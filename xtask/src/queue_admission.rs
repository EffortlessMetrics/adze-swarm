//! Cheap merge-queue admission (#1026): bind queue entry to reviewed head H.
//!
//! No candidate-script execution, no token handling, no live queue/worker
//! state stored in the record. Policy arrives as data, never shell input.

use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

pub const SCHEMA_VERSION: &str = "1";
pub const RECORD_KIND: &str = "merge-queue-admission";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionStatus {
    Authorized,
    Revoked,
    Withdrawn,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewVerdict {
    Approved,
    ChangesRequested,
    Commented,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProofReceipt {
    pub command: String,
    pub result_summary: String,
    /// Reference to the receipt artifact (path/URL/id), passed as data.
    pub receipt_ref: String,
    /// Hex SHA-256 of the receipt file when one exists.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub file_digest: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewDisposition {
    pub reviewer: String,
    pub verdict: ReviewVerdict,
    /// Reference to where findings were disposed (review URL / comment id).
    pub findings_disposition_ref: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Decision {
    pub status: DecisionStatus,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub reason: Option<String>,
    /// Volatile: excluded from `record_digest`.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub decided_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdmissionRecord {
    pub schema_version: String,
    pub kind: String,
    pub repository: String,
    pub pr_number: u64,
    /// H: exact reviewed PR source head (full 40-hex SHA).
    pub head_sha: String,
    pub scope_issues: Vec<u64>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub acceptance_ref: Option<String>,
    pub proof_receipts: Vec<ProofReceipt>,
    pub review: ReviewDisposition,
    pub requester: String,
    pub decision: Decision,
    /// G: queue integration composition SHA, recorded separately from H.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub integration_sha: Option<String>,
    /// PR numbers in the queue group; initially at most one.
    #[serde(default)]
    pub group_pr_numbers: Vec<u64>,
    /// Semantic digest over canonical fields, excluding volatile fields.
    pub record_digest: String,
}

/// Live PR state behind a seam so tests use fixtures, never the network.
pub trait LiveStateProvider {
    fn live_state(&self, repository: &str, pr_number: u64) -> Result<LivePrState>;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LivePrState {
    pub repository: String,
    pub pr_number: u64,
    pub head_sha: String,
    pub is_draft: bool,
    /// "open" or "closed".
    pub state: String,
    #[serde(default)]
    pub is_fork: bool,
    /// Labels are observed only; they are never authorization.
    #[serde(default)]
    pub labels: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixtureLiveState(pub LivePrState);
impl LiveStateProvider for FixtureLiveState {
    fn live_state(&self, _r: &str, _p: u64) -> Result<LivePrState> {
        Ok(self.0.clone())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Refusal {
    pub code: &'static str,
    pub message: String,
    pub next_action: String,
}

fn refusal(code: &'static str, msg: impl Into<String>, next: impl Into<String>) -> Refusal {
    Refusal {
        code,
        message: msg.into(),
        next_action: next.into(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AdmissionOk {
    pub head_sha: String,
    pub integration_sha: Option<String>,
    pub record_digest: String,
}

fn is_full_sha(s: &str) -> bool {
    s.len() == 40
        && s.bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}

/// Canonical semantic payload: fixed order, volatile fields excluded.
fn digest_payload(r: &AdmissionRecord) -> String {
    let mut scope = r.scope_issues.clone();
    scope.sort_unstable();
    scope.dedup();
    let mut group = r.group_pr_numbers.clone();
    group.sort_unstable();
    group.dedup();
    let p = serde_json::json!({
        "schema_version": r.schema_version, "kind": r.kind,
        "repository": r.repository, "pr_number": r.pr_number,
        "head_sha": r.head_sha, "scope_issues": scope,
        "acceptance_ref": r.acceptance_ref,
        "proof_receipts": r.proof_receipts, "review": r.review,
        "requester": r.requester,
        "decision_status": r.decision.status, "decision_reason": r.decision.reason,
        "integration_sha": r.integration_sha, "group_pr_numbers": group,
    });
    // serde_json with preserve_order may keep insertion order; sort keys via BTreeMap round-trip.
    let sorted: serde_json::Map<String, serde_json::Value> =
        serde_json::from_value(p.clone()).unwrap_or_default();
    let mut keys: Vec<&String> = sorted.keys().collect();
    keys.sort();
    let mut out = String::from("{");
    for (i, k) in keys.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str(&serde_json::to_string(k).unwrap_or_default());
        out.push(':');
        out.push_str(&serde_json::to_string(&sorted[*k]).unwrap_or_default());
    }
    out.push('}');
    out
}

pub fn compute_digest(r: &AdmissionRecord) -> String {
    sha256_hex(digest_payload(r).as_bytes())
}

#[allow(clippy::too_many_arguments)]
pub fn build_record(
    repository: String,
    pr_number: u64,
    head_sha: String,
    mut scope_issues: Vec<u64>,
    acceptance_ref: Option<String>,
    proof_receipts: Vec<ProofReceipt>,
    review: ReviewDisposition,
    requester: String,
    integration_sha: Option<String>,
    mut group_pr_numbers: Vec<u64>,
    decided_at: Option<String>,
) -> AdmissionRecord {
    scope_issues.sort_unstable();
    scope_issues.dedup();
    group_pr_numbers.sort_unstable();
    group_pr_numbers.dedup();
    if group_pr_numbers.is_empty() {
        group_pr_numbers.push(pr_number);
    }
    let mut r = AdmissionRecord {
        schema_version: SCHEMA_VERSION.into(),
        kind: RECORD_KIND.into(),
        repository,
        pr_number,
        head_sha,
        scope_issues,
        acceptance_ref,
        proof_receipts,
        review,
        requester,
        decision: Decision {
            status: DecisionStatus::Authorized,
            reason: None,
            decided_at,
        },
        integration_sha,
        group_pr_numbers,
        record_digest: String::new(),
    };
    r.record_digest = compute_digest(&r);
    r
}

pub fn revoke(r: &mut AdmissionRecord, reason: &str, at: Option<String>, withdraw: bool) {
    r.decision.status = if withdraw {
        DecisionStatus::Withdrawn
    } else {
        DecisionStatus::Revoked
    };
    r.decision.reason = Some(reason.to_string());
    r.decision.decided_at = at;
    r.record_digest = compute_digest(r);
}

pub fn validate_shape(r: &AdmissionRecord) -> Vec<Refusal> {
    let mut v = Vec::new();
    if r.repository.trim().is_empty() || !r.repository.contains('/') {
        v.push(refusal(
            "repository_missing",
            "record repository is missing or not owner/name",
            "re-emit the record with the exact repository",
        ));
    }
    if r.pr_number == 0 {
        v.push(refusal(
            "pr_number_missing",
            "record PR number is missing",
            "re-emit with the reviewed PR number",
        ));
    }
    if !is_full_sha(&r.head_sha) {
        v.push(refusal("head_sha_invalid", format!("head H '{}' is not a full lowercase 40-hex SHA", r.head_sha), "re-emit with the exact reviewed head SHA; do not infer it from timestamps or queue refs"));
    }
    if r.scope_issues.is_empty() && r.acceptance_ref.as_deref().unwrap_or("").is_empty() {
        v.push(refusal(
            "scope_missing",
            "no scope/acceptance reference (issue numbers or acceptance ref)",
            "add --scope-issue or --acceptance-ref naming the selected scope",
        ));
    }
    if r.proof_receipts.is_empty()
        || r.proof_receipts.iter().any(|p| {
            p.command.trim().is_empty()
                || p.result_summary.trim().is_empty()
                || p.receipt_ref.trim().is_empty()
        })
    {
        v.push(refusal(
            "proof_missing",
            "required local proof receipt missing command, result summary, or reference",
            "attach a proof receipt (command + result + reference) and re-emit",
        ));
    }
    if r.review.reviewer.trim().is_empty() || r.review.findings_disposition_ref.trim().is_empty() {
        v.push(refusal(
            "review_missing",
            "substantive review disposition missing reviewer or findings reference",
            "record reviewer, verdict, and findings disposition reference",
        ));
    }
    if r.review.verdict != ReviewVerdict::Approved {
        v.push(refusal(
            "review_not_approved",
            "review verdict is not approved",
            "obtain a substantive approving review after findings are disposed",
        ));
    }
    if r.requester.trim().is_empty() {
        v.push(refusal(
            "requester_missing",
            "authorized requester is missing",
            "name the authorized requester",
        ));
    }
    match r.decision.status {
        DecisionStatus::Revoked => v.push(refusal(
            "decision_revoked",
            format!(
                "admission revoked: {}",
                r.decision.reason.as_deref().unwrap_or("no reason given")
            ),
            "address the revocation reason, re-review, and emit a new record",
        )),
        DecisionStatus::Withdrawn => v.push(refusal(
            "decision_withdrawn",
            format!(
                "admission withdrawn: {}",
                r.decision.reason.as_deref().unwrap_or("no reason given")
            ),
            "re-enter admission after repair and re-review",
        )),
        DecisionStatus::Authorized => {}
    }
    if r.group_pr_numbers.len() > 1 {
        v.push(refusal(
            "multi_pr_composition_unsupported",
            format!(
                "queue group lists {} PRs {:?}; multi-PR compositions are not yet supported",
                r.group_pr_numbers.len(),
                r.group_pr_numbers
            ),
            "admit PRs singly, or wait for multi-PR composition support (#1027/#1028)",
        ));
    }
    if let Some(g) = &r.integration_sha
        && !is_full_sha(g)
    {
        v.push(refusal(
            "integration_sha_invalid",
            format!("integration G '{g}' is not a full lowercase 40-hex SHA"),
            "record the exact integration composition SHA or omit G",
        ));
    }
    let expected = compute_digest(r);
    if r.record_digest != expected {
        v.push(refusal("record_digest_mismatch", "record digest does not match canonical semantic fields; record may be tampered or stale", "re-emit the record; do not hand-edit digests"));
    }
    v
}

/// Validate-before-spend. Labels are never authorization.
pub fn validate_against_live(
    r: &AdmissionRecord,
    live: &LivePrState,
) -> Result<AdmissionOk, Vec<Refusal>> {
    let mut v = validate_shape(r);
    if live.repository != r.repository {
        v.push(refusal(
            "repository_mismatch",
            format!(
                "live repository '{}' != record '{}'",
                live.repository, r.repository
            ),
            "re-emit against the correct repository",
        ));
    }
    if live.pr_number != r.pr_number {
        v.push(refusal(
            "pr_number_mismatch",
            format!("live PR #{} != record #{}", live.pr_number, r.pr_number),
            "re-emit against the correct PR",
        ));
    }
    if live.head_sha != r.head_sha {
        v.push(refusal(
            "head_mismatch",
            format!(
                "admitted H {} != live head {}; receipts belong to the old head",
                r.head_sha, live.head_sha
            ),
            "re-run local proof and substantive review on the new head, then re-emit",
        ));
    }
    if live.is_draft {
        v.push(refusal(
            "pr_draft",
            "PR is draft",
            "mark ready only after re-review, then re-validate",
        ));
    }
    if live.state != "open" {
        v.push(refusal(
            "pr_closed",
            format!("PR state is '{}'", live.state),
            "reopen or emit a record for an open PR",
        ));
    }
    if live.is_fork {
        v.push(refusal(
            "fork_refused",
            "fork PRs are refused by admission policy",
            "land the change via a branch in the trusted repository",
        ));
    }
    if v.is_empty() {
        Ok(AdmissionOk {
            head_sha: r.head_sha.clone(),
            integration_sha: r.integration_sha.clone(),
            record_digest: r.record_digest.clone(),
        })
    } else {
        Err(v)
    }
}

pub fn validate_via_provider<P: LiveStateProvider>(
    r: &AdmissionRecord,
    p: &P,
) -> Result<AdmissionOk, Vec<Refusal>> {
    match p.live_state(&r.repository, r.pr_number) {
        Ok(live) => validate_against_live(r, &live),
        Err(e) => Err(vec![refusal(
            "live_state_unavailable",
            format!("live PR state unavailable: {e}"),
            "retry when live state can be resolved; admission fails closed",
        )]),
    }
}

// --- Minimal SHA-256 (no new dependency) ---
pub fn sha256_hex(data: &[u8]) -> String {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    let bitlen = (data.len() as u64).wrapping_mul(8);
    let mut msg = data.to_vec();
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bitlen.to_be_bytes());
    for chunk in msg.chunks_exact(64) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([
                chunk[4 * i],
                chunk[4 * i + 1],
                chunk[4 * i + 2],
                chunk[4 * i + 3],
            ]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let (mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh) =
            (h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7]);
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let t1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
        h[5] = h[5].wrapping_add(f);
        h[6] = h[6].wrapping_add(g);
        h[7] = h[7].wrapping_add(hh);
    }
    h.iter().map(|x| format!("{x:08x}")).collect()
}

pub fn file_digest(path: &Path) -> Result<String> {
    let bytes =
        std::fs::read(path).with_context(|| format!("reading receipt file {}", path.display()))?;
    Ok(sha256_hex(&bytes))
}

pub fn run_emit(args: EmitArgs) -> Result<()> {
    let digest = match &args.proof_file {
        Some(p) => Some(file_digest(p)?),
        None => args.proof_digest.clone(),
    };
    let r = build_record(
        args.repository,
        args.pr,
        args.head,
        args.scope_issues,
        args.acceptance_ref,
        vec![ProofReceipt {
            command: args.proof_command,
            result_summary: args.proof_summary,
            receipt_ref: args.proof_ref,
            file_digest: digest,
        }],
        ReviewDisposition {
            reviewer: args.reviewer,
            verdict: args.verdict,
            findings_disposition_ref: args.findings_ref,
        },
        args.requester,
        args.integration_sha,
        args.group_prs,
        args.decided_at,
    );
    let shape = validate_shape(&r);
    // Emit still writes the record but warns; digest mismatch excluded by construction.
    for s in &shape {
        if s.code != "multi_pr_composition_unsupported" {
            eprintln!("warning [{}]: {}", s.code, s.message);
        }
    }
    write_record(&args.out, &r)?;
    println!(
        "wrote admission record pr={} H={} digest={} -> {}",
        r.pr_number,
        r.head_sha,
        r.record_digest,
        args.out.display()
    );
    Ok(())
}

pub fn run_validate(record_path: &Path, live_path: &Path) -> Result<()> {
    let r: AdmissionRecord =
        serde_json::from_str(&std::fs::read_to_string(record_path).context("reading record")?)
            .context("parsing record")?;
    let live: LivePrState =
        serde_json::from_str(&std::fs::read_to_string(live_path).context("reading live fixture")?)
            .context("parsing live fixture")?;
    match validate_via_provider(&r, &FixtureLiveState(live)) {
        Ok(ok) => {
            println!(
                "ADMITTED pr={} H={} G={} digest={}",
                r.pr_number,
                ok.head_sha,
                ok.integration_sha.as_deref().unwrap_or("-"),
                ok.record_digest
            );
            Ok(())
        }
        Err(rs) => {
            for x in &rs {
                eprintln!(
                    "REFUSED [{}]: {} | next: {}",
                    x.code, x.message, x.next_action
                );
            }
            anyhow::bail!("admission refused ({} reason(s))", rs.len())
        }
    }
}

pub fn run_revoke(
    record_path: &Path,
    reason: &str,
    at: Option<String>,
    withdraw: bool,
    out: Option<&Path>,
) -> Result<()> {
    let mut r: AdmissionRecord =
        serde_json::from_str(&std::fs::read_to_string(record_path).context("reading record")?)
            .context("parsing record")?;
    revoke(&mut r, reason, at, withdraw);
    write_record(out.unwrap_or(record_path), &r)?;
    println!(
        "{} pr={} reason={}",
        if withdraw { "WITHDRAWN" } else { "REVOKED" },
        r.pr_number,
        reason
    );
    Ok(())
}

fn write_record(path: &Path, r: &AdmissionRecord) -> Result<()> {
    std::fs::write(
        path,
        serde_json::to_string_pretty(r).context("serializing record")? + "\n",
    )
    .with_context(|| format!("writing {}", path.display()))
}

pub struct EmitArgs {
    pub repository: String,
    pub pr: u64,
    pub head: String,
    pub scope_issues: Vec<u64>,
    pub acceptance_ref: Option<String>,
    pub proof_command: String,
    pub proof_summary: String,
    pub proof_ref: String,
    pub proof_digest: Option<String>,
    pub proof_file: Option<std::path::PathBuf>,
    pub reviewer: String,
    pub verdict: ReviewVerdict,
    pub findings_ref: String,
    pub requester: String,
    pub integration_sha: Option<String>,
    pub group_prs: Vec<u64>,
    pub decided_at: Option<String>,
    pub out: std::path::PathBuf,
}

#[cfg(test)]
mod tests {
    use super::*;
    const H: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const H2: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    const G: &str = "cccccccccccccccccccccccccccccccccccccccc";
    fn rec() -> AdmissionRecord {
        build_record(
            "EffortlessMetrics/adze-swarm".into(),
            42,
            H.into(),
            vec![1026],
            Some("issue-1026".into()),
            vec![ProofReceipt {
                command: "cargo test -p xtask --bin xtask queue_admission".into(),
                result_summary: "12 passed".into(),
                receipt_ref: "target/receipt.json".into(),
                file_digest: Some(sha256_hex(b"x")),
            }],
            ReviewDisposition {
                reviewer: "reviewer-a".into(),
                verdict: ReviewVerdict::Approved,
                findings_disposition_ref: "review:1".into(),
            },
            "maintainer-a".into(),
            None,
            vec![42],
            Some("2026-01-01T00:00:00Z".into()),
        )
    }
    fn live(head: &str) -> LivePrState {
        LivePrState {
            repository: "EffortlessMetrics/adze-swarm".into(),
            pr_number: 42,
            head_sha: head.into(),
            is_draft: false,
            state: "open".into(),
            is_fork: false,
            labels: vec!["ready".into()],
        }
    }
    fn codes(e: &Vec<Refusal>) -> Vec<&'static str> {
        e.iter().map(|r| r.code).collect()
    }

    #[test]
    fn sha256_known_vector() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
    #[test]
    fn emit_is_deterministic_and_timestamp_excluded_from_digest() {
        let a = rec();
        let mut b = rec();
        b.decision.decided_at = Some("2030-01-01T00:00:00Z".into());
        assert_eq!(a.record_digest, compute_digest(&b));
        assert_eq!(
            serde_json::to_string(&a).unwrap(),
            serde_json::to_string(&rec()).unwrap()
        );
    }
    #[test]
    fn valid_record_admits_with_fixture() {
        assert!(validate_against_live(&rec(), &live(H)).is_ok());
    }
    #[test]
    fn head_mismatch_refused() {
        let e = validate_against_live(&rec(), &live(H2)).unwrap_err();
        assert!(codes(&e).contains(&"head_mismatch"));
    }
    #[test]
    fn draft_refused() {
        let mut l = live(H);
        l.is_draft = true;
        assert!(codes(&validate_against_live(&rec(), &l).unwrap_err()).contains(&"pr_draft"));
    }
    #[test]
    fn closed_refused() {
        let mut l = live(H);
        l.state = "closed".into();
        assert!(codes(&validate_against_live(&rec(), &l).unwrap_err()).contains(&"pr_closed"));
    }
    #[test]
    fn fork_refused() {
        let mut l = live(H);
        l.is_fork = true;
        assert!(codes(&validate_against_live(&rec(), &l).unwrap_err()).contains(&"fork_refused"));
    }
    #[test]
    fn labels_are_not_authorization() {
        let mut r = rec();
        r.proof_receipts.clear();
        r.record_digest = compute_digest(&r);
        let e = validate_against_live(&r, &live(H)).unwrap_err();
        assert!(codes(&e).contains(&"proof_missing"));
    }
    #[test]
    fn missing_proof_review_scope_refused() {
        let mut r = rec();
        r.proof_receipts.clear();
        r.scope_issues.clear();
        r.acceptance_ref = None;
        r.review.reviewer.clear();
        r.record_digest = compute_digest(&r);
        let e = validate_against_live(&r, &live(H)).unwrap_err();
        assert!(
            codes(&e).contains(&"proof_missing")
                && codes(&e).contains(&"scope_missing")
                && codes(&e).contains(&"review_missing")
        );
    }
    #[test]
    fn g_may_differ_from_h_and_is_recorded() {
        let mut r = rec();
        r.integration_sha = Some(G.into());
        r.record_digest = compute_digest(&r);
        let ok = validate_against_live(&r, &live(H)).unwrap();
        assert_eq!(ok.integration_sha.as_deref(), Some(G));
    }
    #[test]
    fn multi_pr_composition_refused() {
        let mut r = rec();
        r.group_pr_numbers = vec![42, 43];
        r.record_digest = compute_digest(&r);
        assert!(
            codes(&validate_against_live(&r, &live(H)).unwrap_err())
                .contains(&"multi_pr_composition_unsupported")
        );
    }
    #[test]
    fn revocation_fails_closed_despite_prior_clean_review() {
        let r0 = rec();
        assert!(validate_against_live(&r0, &live(H)).is_ok());
        let mut r = r0;
        revoke(
            &mut r,
            "new blocking finding at unchanged H",
            Some("2026-01-02T00:00:00Z".into()),
            false,
        );
        assert_eq!(r.review.verdict, ReviewVerdict::Approved);
        assert!(
            codes(&validate_against_live(&r, &live(H)).unwrap_err()).contains(&"decision_revoked")
        );
    }
    #[test]
    fn withdrawn_refused() {
        let mut r = rec();
        revoke(&mut r, "author withdrew", None, true);
        assert!(
            codes(&validate_against_live(&r, &live(H)).unwrap_err())
                .contains(&"decision_withdrawn")
        );
    }
    #[test]
    fn wrong_repo_and_pr_refused() {
        let mut l = live(H);
        l.repository = "other/repo".into();
        l.pr_number = 99;
        let e = validate_against_live(&rec(), &l).unwrap_err();
        assert!(
            codes(&e).contains(&"repository_mismatch") && codes(&e).contains(&"pr_number_mismatch")
        );
    }
    #[test]
    fn tampered_digest_refused() {
        let mut r = rec();
        r.requester = "attacker".into();
        assert!(
            codes(&validate_against_live(&r, &live(H)).unwrap_err())
                .contains(&"record_digest_mismatch")
        );
    }
    #[test]
    fn short_or_uppercase_head_refused() {
        let mut r = rec();
        r.head_sha = "ABC123".into();
        r.record_digest = compute_digest(&r);
        assert!(
            codes(&validate_against_live(&r, &live(H)).unwrap_err()).contains(&"head_sha_invalid")
        );
    }
    #[test]
    fn fixture_json_roundtrip_offline() {
        let r = rec();
        let lj = serde_json::to_string(&live(H)).unwrap();
        let parsed: LivePrState = serde_json::from_str(&lj).unwrap();
        assert!(validate_via_provider(&r, &FixtureLiveState(parsed)).is_ok());
        let rj = serde_json::to_string(&r).unwrap();
        let back: AdmissionRecord = serde_json::from_str(&rj).unwrap();
        assert_eq!(back.record_digest, r.record_digest);
    }
}
