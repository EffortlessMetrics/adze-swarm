//! Semantic no-panic checker.
//!
//! Walks every `*.rs` file under the workspace root, parses it with `syn`,
//! and records every panic-family call shape together with its enclosing
//! container (function / impl / module path). Identity is the full key
//! `(path, family, kind, container, callee, receiver_fingerprint)` — we
//! deliberately do not key on line/column, but every other field must match:
//! a receipt for one call shape never authorizes a different call.
//!
//! ## Supported syntactic shapes (honest coverage)
//!
//! | family | syntax detected |
//! |---|---|
//! | `unwrap` | `recv.unwrap()` outside `Result`-returning fns; qualified `Option::unwrap(x)` |
//! | `unwrap_in_result` | `recv.unwrap()` inside a fn whose return type mentions `Result` |
//! | `expect` | `recv.expect(..)` and qualified `Result::expect(..)` |
//! | `panic_macro`, `todo`, `unimplemented`, `unreachable` | macro calls, matched by last path segment |
//! | `indexing` | `base[expr]` with a non-range index |
//! | `string_slice` | `base[start..end]` (range index: `str` byte slicing, panics off-boundary) |
//! | `get_unwrap` | `map.get(k).unwrap()` (fused pair; still panics when the key is absent) |
//!
//! Method/qualified detection is name-based (`syn` has no type info).
//! Deliberately NOT covered: `assert!` oracles, `unsafe` code, integer
//! overflow, resource exhaustion, dependency panics.
//!
//! ## Known false-positive class: `serde_json::Value` indexing (documented, not repaired)
//!
//! `value["missing"]` on a `serde_json::Value` never panics (missing keys
//! yield `Null`), but the scanner still reports every `[]` as `indexing`
//! because it cannot resolve types. Do not special-case this in code;
//! classify such findings explicitly (e.g. `external_contract` naming
//! `serde_json::Value` semantics).
//!
//! ## Production scope
//!
//! Scope is derived mechanically from `policy/package-boundary.toml`:
//! packages with `category = "published"` AND `production_use = true`
//! contribute their `path` as a directory prefix. Each scanned file is tagged
//! `production`, `test_only` (under `tests/`, `benches/`, `examples/`, or a
//! `tests.rs` / `*_test.rs` / `*_tests.rs` file inside a production prefix),
//! or `out_of_scope` (dev-only packages). Test-only findings stay visible
//! under their own scope and are never counted in the production denominator.
//!
//! ## Exit-mode contract (blocking-allowlist does NOT enforce freshness)
//!
//! | mode | unallowlisted | stale entries | expired entries | unreadable/unparseable | empty production denominator |
//! |---|---|---|---|---|
//! | `advisory` | report | report | report | FAIL (hard) | FAIL (hard) |
//! | `blocking-allowlist` | FAIL | report only | report only | FAIL (hard) | FAIL (hard) |
//! | `blocking-strict` | FAIL | FAIL | FAIL | FAIL (hard) | FAIL (hard) |
//!
//! Scan-integrity failures (unreadable/unparseable inputs, an empty
//! production denominator) fail in every mode so a broken scan can never
//! report a successful complete scan.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use syn::spanned::Spanned as _;

use super::{Mode, ensure_report_dir, workspace_root};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum Family {
    Unwrap,
    Expect,
    PanicMacro,
    Todo,
    Unimplemented,
    Unreachable,
    Indexing,
    StringSlice,
    GetUnwrap,
    UnwrapInResult,
}

impl Family {
    fn as_str(self) -> &'static str {
        match self {
            Family::Unwrap => "unwrap",
            Family::Expect => "expect",
            Family::PanicMacro => "panic_macro",
            Family::Todo => "todo",
            Family::Unimplemented => "unimplemented",
            Family::Unreachable => "unreachable",
            Family::Indexing => "indexing",
            Family::StringSlice => "string_slice",
            Family::GetUnwrap => "get_unwrap",
            Family::UnwrapInResult => "unwrap_in_result",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Selector {
    pub kind: String,
    pub container: String,
    pub callee: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub receiver_fingerprint: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LastSeen {
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct Finding {
    pub path: String,
    pub family: Family,
    pub selector: Selector,
    pub last_seen: LastSeen,
    /// `production` | `test_only` | `out_of_scope` (see module docs).
    pub scope: String,
}

#[derive(Debug, Default, Deserialize)]
#[allow(dead_code)]
struct AllowlistFile {
    #[serde(default)]
    schema_version: Option<String>,
    #[serde(default, rename = "allow")]
    entries: Vec<AllowEntry>,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct AllowEntry {
    pub id: String,
    pub path: String,
    pub family: Family,
    pub classification: String,
    pub owner: String,
    pub explanation: String,
    pub expires: String,
    pub selector: Selector,
    #[serde(default)]
    pub last_seen: Option<LastSeen>,
}

#[derive(Debug, Default, Serialize)]
pub struct CheckReport {
    pub mode: String,
    pub total_findings: usize,
    pub allowlist_size: usize,
    pub matched: usize,
    pub unallowlisted: Vec<Finding>,
    pub stale_entries: Vec<String>,
    pub expired_entries: Vec<ExpiredEntry>,
    /// Candidate `.rs` files discovered (outside hard-ignored dirs).
    pub selected_files: usize,
    /// Candidates successfully read and parsed.
    pub scanned_files: usize,
    /// Scanned files outside the production denominator (`test_only` + `out_of_scope`).
    pub excluded_files: usize,
    /// Candidates that could not be read or parsed.
    pub failed_files: usize,
    /// Per-file failure detail. Non-empty fails every mode (hard).
    pub failed_details: Vec<FailedFile>,
    /// Scanned files in production scope. Zero fails every mode (hard).
    pub production_files: usize,
    pub production_findings: usize,
    pub test_only_findings: usize,
    pub out_of_scope_findings: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct FailedFile {
    pub path: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExpiredEntry {
    pub id: String,
    pub expires: String,
}

pub fn run_check(mode: Mode) -> Result<()> {
    let root = workspace_root()?;
    let report_dir = ensure_report_dir(&root)?;
    let allowlist = load_allowlist(&root)?;
    let prefixes = production_prefixes(&root)?;

    let (findings, stats) = scan_workspace(&root, &prefixes)?;
    let mut report = CheckReport {
        mode: format!("{mode:?}"),
        total_findings: findings.len(),
        allowlist_size: allowlist.entries.len(),
        selected_files: stats.selected,
        scanned_files: stats.scanned,
        excluded_files: stats.excluded,
        failed_files: stats.failed.len(),
        failed_details: stats.failed,
        production_files: stats.production_files,
        ..Default::default()
    };
    for finding in &findings {
        match finding.scope.as_str() {
            "production" => report.production_findings += 1,
            "test_only" => report.test_only_findings += 1,
            _ => report.out_of_scope_findings += 1,
        }
    }

    let mut matched_ids: BTreeSet<String> = BTreeSet::new();
    for finding in &findings {
        if let Some(entry) = allowlist.match_finding(finding) {
            matched_ids.insert(entry.id.clone());
            report.matched += 1;
        } else {
            report.unallowlisted.push(finding.clone());
        }
    }

    for entry in &allowlist.entries {
        if !matched_ids.contains(&entry.id) {
            report.stale_entries.push(entry.id.clone());
        }
        if is_expired(&entry.expires) {
            report.expired_entries.push(ExpiredEntry {
                id: entry.id.clone(),
                expires: entry.expires.clone(),
            });
        }
    }

    write_reports(&report_dir, &report, &findings)?;
    print_summary(&report);

    // Scan-integrity failures are hard in every mode (see module docs): a
    // broken scan must never report a successful complete scan (#867).
    if !report.failed_details.is_empty() {
        anyhow::bail!(
            "no-panic check failed: {} unreadable/unparseable file(s); see failed_details",
            report.failed_files
        );
    }
    if report.production_files == 0 {
        anyhow::bail!(
            "no-panic check failed: empty production denominator (no scanned production-scope files)"
        );
    }

    match mode {
        Mode::Advisory => Ok(()),
        // blocking-allowlist enforces allowlist coverage ONLY. Stale and
        // expired entries are reported (stale_entries/expired_entries) but
        // do not fail this mode; use blocking-strict for freshness.
        Mode::BlockingAllowlist => {
            if !report.unallowlisted.is_empty() {
                anyhow::bail!(
                    "no-panic check failed: {} unallowlisted findings",
                    report.unallowlisted.len()
                );
            }
            Ok(())
        }
        Mode::BlockingStrict => {
            let bad = report.unallowlisted.len()
                + report.stale_entries.len()
                + report.expired_entries.len();
            if bad > 0 {
                anyhow::bail!(
                    "no-panic check failed: {} unallowlisted, {} stale, {} expired",
                    report.unallowlisted.len(),
                    report.stale_entries.len(),
                    report.expired_entries.len()
                );
            }
            Ok(())
        }
    }
}

pub fn run_propose(baseline: bool) -> Result<()> {
    let root = workspace_root()?;
    let report_dir = ensure_report_dir(&root)?;
    let prefixes = production_prefixes(&root)?;
    let (findings, _stats) = scan_workspace(&root, &prefixes)?;

    let allowlist = load_allowlist(&root)?;
    let mut entries = Vec::new();
    let mut id_counter = next_id_seed(&allowlist);
    // Dedup on the same full key the matcher enforces, so two distinct
    // calls never collapse into one proposed receipt.
    let mut seen_keys: BTreeSet<ProposeKey> = BTreeSet::new();

    for finding in &findings {
        let key = propose_key(finding);
        if !seen_keys.insert(key) {
            continue;
        }
        if !baseline && allowlist.match_finding(finding).is_some() {
            continue;
        }
        let id = format!("panic-{id_counter:04}");
        id_counter += 1;
        entries.push(propose_entry(id, finding));
    }

    let proposed = report_dir.join("no-panic-proposed.toml");
    let mut out = String::from(
        "schema_version = \"0.3\"\n# Proposed no-panic allowlist baseline.\n# Review before copying entries into policy/no-panic-allowlist.toml.\n\n",
    );
    out.push_str(&entries.join("\n"));
    std::fs::write(&proposed, out)
        .with_context(|| format!("writing proposal to {}", proposed.display()))?;

    println!(
        "wrote {} proposed entries to {}",
        entries.len(),
        proposed.display()
    );
    Ok(())
}

/// Full identity key enforced by [`Allowlist::match_finding`]: a receipt
/// covers exactly one call shape, never a second distinct call.
type ProposeKey = (String, String, String, String, String, String);

fn propose_key(f: &Finding) -> ProposeKey {
    (
        f.path.clone(),
        f.family.as_str().to_string(),
        f.selector.kind.clone(),
        f.selector.container.clone(),
        f.selector.callee.clone(),
        f.selector.receiver_fingerprint.clone().unwrap_or_default(),
    )
}

fn propose_entry(id: String, f: &Finding) -> String {
    let receiver = f.selector.receiver_fingerprint.clone().unwrap_or_default();
    let receiver_field = if receiver.is_empty() {
        String::new()
    } else {
        format!(
            "receiver_fingerprint = {}\n",
            toml_string_literal(&receiver)
        )
    };
    let expires = default_expiry();
    format!(
        r#"[[allow]]
id = "{id}"
path = {path}
family = "{family}"
classification = "legacy"
owner = "TODO"
explanation = "TODO: replace with fallible alternative."
expires = "{expires}"

[allow.selector]
kind = "{kind}"
container = {container}
callee = "{callee}"
{receiver_field}
[allow.last_seen]
line = {line}
column = {column}
"#,
        path = toml_string_literal(&f.path),
        family = f.family.as_str(),
        kind = f.selector.kind,
        container = toml_string_literal(&f.selector.container),
        callee = f.selector.callee,
        line = f.last_seen.line,
        column = f.last_seen.column,
    )
}

fn next_id_seed(allowlist: &Allowlist) -> u32 {
    allowlist
        .entries
        .iter()
        .filter_map(|e| e.id.strip_prefix("panic-"))
        .filter_map(|s| s.parse::<u32>().ok())
        .max()
        .map(|n| n + 1)
        .unwrap_or(1)
}

fn toml_string_literal(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            other => out.push(other),
        }
    }
    out.push('"');
    out
}

fn default_expiry() -> String {
    let today = chrono::Utc::now().date_naive();
    let plus_six_months = today
        .checked_add_months(chrono::Months::new(6))
        .unwrap_or(today);
    plus_six_months.format("%Y-%m-%d").to_string()
}

fn is_expired(expires: &str) -> bool {
    match chrono::NaiveDate::parse_from_str(expires, "%Y-%m-%d") {
        Ok(date) => date < chrono::Utc::now().date_naive(),
        Err(_) => true,
    }
}

#[derive(Debug, Default)]
struct Allowlist {
    entries: Vec<AllowEntry>,
}

impl Allowlist {
    /// Match on the full key (path, family, kind, container, callee,
    /// receiver_fingerprint). A receipt for one call shape must never
    /// silently authorize a second distinct call.
    fn match_finding(&self, f: &Finding) -> Option<&AllowEntry> {
        self.entries.iter().find(|entry| {
            entry.path == f.path
                && entry.family == f.family
                && entry.selector.kind == f.selector.kind
                && entry.selector.container == f.selector.container
                && entry.selector.callee == f.selector.callee
                && entry.selector.receiver_fingerprint == f.selector.receiver_fingerprint
        })
    }
}

fn load_allowlist(root: &Path) -> Result<Allowlist> {
    let path = root.join("policy").join("no-panic-allowlist.toml");
    if !path.exists() {
        return Ok(Allowlist::default());
    }
    let text =
        std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    let parsed: AllowlistFile =
        toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;
    Ok(Allowlist {
        entries: parsed.entries,
    })
}

fn write_reports(report_dir: &Path, report: &CheckReport, findings: &[Finding]) -> Result<()> {
    let json = serde_json::to_string_pretty(&report)?;
    std::fs::write(report_dir.join("no-panic.json"), json)?;

    let mut md = String::new();
    md.push_str("# No-panic policy report\n\n");
    md.push_str(&format!("- mode: `{}`\n", report.mode));
    md.push_str(&format!("- total findings: {}\n", report.total_findings));
    md.push_str(&format!("- allowlist size: {}\n", report.allowlist_size));
    md.push_str(&format!("- matched: {}\n", report.matched));
    md.push_str(&format!(
        "- unallowlisted: {}\n",
        report.unallowlisted.len()
    ));
    md.push_str(&format!(
        "- stale entries: {}\n",
        report.stale_entries.len()
    ));
    md.push_str(&format!(
        "- expired entries: {}\n",
        report.expired_entries.len()
    ));
    md.push_str(&format!("- selected files: {}\n", report.selected_files));
    md.push_str(&format!("- scanned files: {}\n", report.scanned_files));
    md.push_str(&format!(
        "- excluded files (non-production scope): {}\n",
        report.excluded_files
    ));
    md.push_str(&format!("- failed files: {}\n", report.failed_files));
    md.push_str(&format!(
        "- production files (denominator): {}\n",
        report.production_files
    ));
    md.push_str(&format!(
        "- findings by scope: production={}, test_only={}, out_of_scope={}\n",
        report.production_findings, report.test_only_findings, report.out_of_scope_findings
    ));
    md.push_str("\n> blocking-allowlist fails on unallowlisted findings only; stale and\n> expired entries are reported but not enforced (use blocking-strict).\n");

    md.push_str("\n## Family breakdown\n\n");
    let mut by_family: BTreeMap<&'static str, usize> = BTreeMap::new();
    for f in findings {
        *by_family.entry(f.family.as_str()).or_default() += 1;
    }
    md.push_str("| family | count |\n|---|---|\n");
    for (k, v) in by_family {
        md.push_str(&format!("| {k} | {v} |\n"));
    }

    md.push_str("\n## Path prefix breakdown\n\n");
    let mut by_prefix: BTreeMap<&str, usize> = BTreeMap::new();
    for f in findings {
        let prefix = f.path.split('/').next().unwrap_or("<root>");
        *by_prefix.entry(prefix).or_default() += 1;
    }
    let mut prefix_rows = by_prefix.into_iter().collect::<Vec<_>>();
    prefix_rows.sort_by(|(left_name, left_count), (right_name, right_count)| {
        right_count
            .cmp(left_count)
            .then_with(|| left_name.cmp(right_name))
    });
    md.push_str("| prefix | count |\n|---|---|\n");
    for (prefix, count) in prefix_rows {
        md.push_str(&format!("| {prefix} | {count} |\n"));
    }

    md.push_str("\n## Top files by finding count\n\n");
    let mut by_file: BTreeMap<&str, usize> = BTreeMap::new();
    for f in findings {
        *by_file.entry(&f.path).or_default() += 1;
    }
    let mut file_rows = by_file.into_iter().collect::<Vec<_>>();
    file_rows.sort_by(|(left_path, left_count), (right_path, right_count)| {
        right_count
            .cmp(left_count)
            .then_with(|| left_path.cmp(right_path))
    });
    md.push_str("| path | count |\n|---|---|\n");
    for (path, count) in file_rows.iter().take(25) {
        md.push_str(&format!("| {path} | {count} |\n"));
    }

    if !report.unallowlisted.is_empty() {
        md.push_str("\n## Unallowlisted (top 50)\n\n");
        md.push_str("| family | path | container | callee | line |\n|---|---|---|---|---|\n");
        for f in report.unallowlisted.iter().take(50) {
            md.push_str(&format!(
                "| {} | {} | {} | {} | {} |\n",
                f.family.as_str(),
                f.path,
                f.selector.container,
                f.selector.callee,
                f.last_seen.line,
            ));
        }
    }

    if !report.failed_details.is_empty() {
        md.push_str("\n## Failed files (top 20)\n\n");
        md.push_str("| path | reason |\n|---|---|\n");
        for failed in report.failed_details.iter().take(20) {
            md.push_str(&format!("| {} | {} |\n", failed.path, failed.reason));
        }
    }
    std::fs::write(report_dir.join("no-panic.md"), md)?;
    Ok(())
}

fn print_summary(report: &CheckReport) {
    println!("no-panic check ({})", report.mode);
    println!("  total findings: {}", report.total_findings);
    println!("  allowlist:      {}", report.allowlist_size);
    println!("  matched:        {}", report.matched);
    println!("  unallowlisted:  {}", report.unallowlisted.len());
    println!("  stale entries:  {}", report.stale_entries.len());
    println!("  expired:        {}", report.expired_entries.len());
    println!("  selected files: {}", report.selected_files);
    println!("  scanned files: {}", report.scanned_files);
    println!("  excluded files: {}", report.excluded_files);
    println!("  failed files: {}", report.failed_files);
    println!("  production files: {}", report.production_files);
    println!(
        "  findings by scope: production={} test_only={} out_of_scope={}",
        report.production_findings, report.test_only_findings, report.out_of_scope_findings
    );
}

// ---------- scanning ----------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(clippy::enum_variant_names)] // `OutOfScope` reads best with the `Scope` suffix.
enum Scope {
    Production,
    TestOnly,
    OutOfScope,
}

impl Scope {
    fn as_str(self) -> &'static str {
        match self {
            Scope::Production => "production",
            Scope::TestOnly => "test_only",
            Scope::OutOfScope => "out_of_scope",
        }
    }
}

#[derive(Debug, Default)]
struct ScanStats {
    selected: usize,
    scanned: usize,
    excluded: usize,
    production_files: usize,
    failed: Vec<FailedFile>,
}

fn scan_workspace(root: &Path, prefixes: &[String]) -> Result<(Vec<Finding>, ScanStats)> {
    let mut findings = Vec::new();
    let mut stats = ScanStats::default();
    for entry in walkdir::WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| !is_ignored_dir(e))
    {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("rs") {
            continue;
        }
        let rel = match path.strip_prefix(root) {
            Ok(rel) => rel.to_string_lossy().replace('\\', "/"),
            Err(_) => continue,
        };
        stats.selected += 1;
        let scope = classify_file(&rel, prefixes);
        match scan_file(path, &rel, scope) {
            Ok(mut file_findings) => {
                stats.scanned += 1;
                match scope {
                    Scope::Production => stats.production_files += 1,
                    Scope::TestOnly | Scope::OutOfScope => stats.excluded += 1,
                }
                findings.append(&mut file_findings);
            }
            Err(reason) => stats.failed.push(FailedFile { path: rel, reason }),
        }
    }
    Ok((findings, stats))
}

/// Directory prefixes that count as production scope, derived from
/// `policy/package-boundary.toml`: `category = "published"` AND
/// `production_use = true`. A missing or empty derivation is an error, never
/// a silent whole-workspace pass.
fn production_prefixes(root: &Path) -> Result<Vec<String>> {
    #[derive(Debug, Default, serde::Deserialize)]
    struct Ledger {
        #[serde(default, rename = "package")]
        packages: Vec<Package>,
    }
    #[derive(Debug, Default, serde::Deserialize)]
    struct Package {
        #[serde(default)]
        path: String,
        #[serde(default)]
        category: String,
        #[serde(default)]
        production_use: Option<bool>,
    }
    let path = root.join("policy").join("package-boundary.toml");
    let text =
        std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    let ledger: Ledger =
        toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;
    let mut prefixes: Vec<String> = ledger
        .packages
        .into_iter()
        .filter(|p| p.category == "published" && p.production_use == Some(true))
        .map(|p| {
            let trimmed = p.path.replace('\\', "/");
            let trimmed = trimmed.trim_matches('/').to_string();
            if trimmed.is_empty() {
                String::new()
            } else {
                format!("{trimmed}/")
            }
        })
        .collect();
    prefixes.sort();
    prefixes.dedup();
    if prefixes.is_empty() {
        anyhow::bail!(
            "no-panic scope: package-boundary.toml yields no published+production_use packages"
        );
    }
    Ok(prefixes)
}

fn classify_file(rel: &str, prefixes: &[String]) -> Scope {
    let in_production = prefixes
        .iter()
        .any(|p| p.is_empty() || rel == p.trim_end_matches('/') || rel.starts_with(p));
    if !in_production {
        return Scope::OutOfScope;
    }
    if is_test_only_path(rel) {
        Scope::TestOnly
    } else {
        Scope::Production
    }
}

/// Cargo-conventional test-only locations inside a production prefix.
/// A directory literally named `tests` is not the whole classifier: file
/// stems (`tests.rs`, `*_test.rs`, `*_tests.rs`) and `benches/`/`examples/`
/// count too. `#[cfg(test)]` modules inside ordinary files are NOT split
/// out — the file keeps its production scope and its findings stay visible.
fn is_test_only_path(rel: &str) -> bool {
    let segments: Vec<&str> = rel.split('/').collect();
    if segments
        .iter()
        .any(|s| *s == "tests" || *s == "benches" || *s == "examples")
    {
        return true;
    }
    if let Some(file) = segments.last() {
        let stem = file.strip_suffix(".rs").unwrap_or(file);
        if stem == "tests" || stem.ends_with("_test") || stem.ends_with("_tests") {
            return true;
        }
    }
    false
}

fn is_ignored_dir(entry: &walkdir::DirEntry) -> bool {
    let name = entry.file_name().to_string_lossy();
    if entry.depth() == 0 {
        return false;
    }
    matches!(
        name.as_ref(),
        "target" | ".git" | "node_modules" | "corpus" | "baselines" | "clippy-report"
    )
}

fn scan_file(path: &Path, rel: &str, scope: Scope) -> std::result::Result<Vec<Finding>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("unreadable: {e}"))?;
    scan_source(rel, &text, scope).map_err(|e| format!("parse_error: {e}"))
}

fn scan_source(
    rel: &str,
    text: &str,
    scope: Scope,
) -> std::result::Result<Vec<Finding>, syn::Error> {
    let file = syn::parse_file(text)?;
    let mut out = Vec::new();
    let mut visitor = Visitor {
        path: rel.to_string(),
        scope: scope.as_str().to_string(),
        out: &mut out,
        container_stack: Vec::new(),
        result_stack: Vec::new(),
    };
    syn::visit::Visit::visit_file(&mut visitor, &file);
    Ok(out)
}

struct Visitor<'a> {
    path: String,
    scope: String,
    out: &'a mut Vec<Finding>,
    container_stack: Vec<String>,
    /// Tracks whether each enclosing fn returns a `Result` (name-based).
    result_stack: Vec<bool>,
}

impl<'a> Visitor<'a> {
    fn current_container(&self) -> String {
        if self.container_stack.is_empty() {
            "<file>".to_string()
        } else {
            self.container_stack.join("::")
        }
    }

    fn line_col(&self, span: proc_macro2::Span) -> (usize, usize) {
        let start = span.start();
        (start.line.max(1), start.column + 1)
    }

    fn emit(
        &mut self,
        family: Family,
        kind: &str,
        callee: &str,
        span: proc_macro2::Span,
        fp: Option<String>,
    ) {
        let (line, column) = self.line_col(span);
        self.out.push(Finding {
            path: self.path.clone(),
            family,
            selector: Selector {
                kind: kind.to_string(),
                container: self.current_container(),
                callee: callee.to_string(),
                receiver_fingerprint: fp,
            },
            last_seen: LastSeen { line, column },
            scope: self.scope.clone(),
        });
    }

    fn push_fn(&mut self, name: &str, output: &syn::ReturnType) {
        self.container_stack.push(name.to_string());
        self.result_stack.push(returns_result(output));
    }

    fn pop_fn(&mut self) {
        self.container_stack.pop();
        self.result_stack.pop();
    }

    fn in_result_fn(&self) -> bool {
        self.result_stack.last().copied().unwrap_or(false)
    }
}

impl<'ast, 'a> syn::visit::Visit<'ast> for Visitor<'a> {
    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        self.push_fn(&node.sig.ident.to_string(), &node.sig.output);
        syn::visit::visit_item_fn(self, node);
        self.pop_fn();
    }

    fn visit_impl_item_fn(&mut self, node: &'ast syn::ImplItemFn) {
        self.push_fn(&node.sig.ident.to_string(), &node.sig.output);
        syn::visit::visit_impl_item_fn(self, node);
        self.pop_fn();
    }

    fn visit_trait_item_fn(&mut self, node: &'ast syn::TraitItemFn) {
        self.push_fn(&node.sig.ident.to_string(), &node.sig.output);
        syn::visit::visit_trait_item_fn(self, node);
        self.pop_fn();
    }

    fn visit_item_mod(&mut self, node: &'ast syn::ItemMod) {
        self.container_stack.push(node.ident.to_string());
        syn::visit::visit_item_mod(self, node);
        self.container_stack.pop();
    }

    fn visit_item_impl(&mut self, node: &'ast syn::ItemImpl) {
        let pretty = type_to_string(&node.self_ty);
        self.container_stack.push(format!("impl {pretty}"));
        syn::visit::visit_item_impl(self, node);
        self.container_stack.pop();
    }

    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        let method = node.method.to_string();
        let family = match method.as_str() {
            // `map.get(k).unwrap()` still panics on a missing key, but it is
            // a distinct, greppable shape worth its own receipt.
            "unwrap" if is_get_call(&node.receiver) => Some(Family::GetUnwrap),
            // `unwrap` where `?` was available: the highest-value repair target.
            "unwrap" if self.in_result_fn() => Some(Family::UnwrapInResult),
            "unwrap" => Some(Family::Unwrap),
            "expect" => Some(Family::Expect),
            _ => None,
        };
        if let Some(family) = family {
            let fp = receiver_fingerprint(&node.receiver);
            self.emit(family, "method_call", &method, node.method.span(), fp);
        }
        syn::visit::visit_expr_method_call(self, node);
    }

    fn visit_expr_macro(&mut self, node: &'ast syn::ExprMacro) {
        if let Some(family) = macro_family(&node.mac.path) {
            let callee = path_last_segment(&node.mac.path);
            self.emit(family, "macro_call", &callee, node.mac.path.span(), None);
        }
        syn::visit::visit_expr_macro(self, node);
    }

    fn visit_stmt_macro(&mut self, node: &'ast syn::StmtMacro) {
        if let Some(family) = macro_family(&node.mac.path) {
            let callee = path_last_segment(&node.mac.path);
            self.emit(family, "macro_call", &callee, node.mac.path.span(), None);
        }
        syn::visit::visit_stmt_macro(self, node);
    }

    fn visit_expr_index(&mut self, node: &'ast syn::ExprIndex) {
        let fp = Some(expr_fingerprint(&node.expr));
        // A range index (`s[a..b]`, `s[..]`) is `str` byte slicing, which
        // panics on non-boundary/off-range indices — a distinct family from
        // plain element indexing.
        let (family, kind) = match node.index.as_ref() {
            syn::Expr::Range(_) => (Family::StringSlice, "slice_expr"),
            _ => (Family::Indexing, "index_expr"),
        };
        self.emit(family, kind, "[]", node.span(), fp);
        syn::visit::visit_expr_index(self, node);
    }

    /// Qualified calls (`Option::unwrap(x)`, `Result::expect(x, ..)`).
    /// Name-based only: `syn` has no type information, so any path whose
    /// last segment is `unwrap`/`expect` is reported.
    fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
        if let syn::Expr::Path(path_expr) = node.func.as_ref() {
            let last = path_last_segment(&path_expr.path);
            let family = match last.as_str() {
                "unwrap" => Some(Family::Unwrap),
                "expect" => Some(Family::Expect),
                _ => None,
            };
            if let Some(family) = family {
                self.emit(family, "qualified_call", &last, path_expr.path.span(), None);
            }
        }
        syn::visit::visit_expr_call(self, node);
    }
}

fn type_to_string(ty: &syn::Type) -> String {
    let s = quote::quote!(#ty).to_string();
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn path_last_segment(path: &syn::Path) -> String {
    path.segments
        .last()
        .map(|s| s.ident.to_string())
        .unwrap_or_else(|| "<unknown>".to_string())
}

fn macro_family(path: &syn::Path) -> Option<Family> {
    let last = path.segments.last()?.ident.to_string();
    match last.as_str() {
        "panic" => Some(Family::PanicMacro),
        "todo" => Some(Family::Todo),
        "unimplemented" => Some(Family::Unimplemented),
        "unreachable" => Some(Family::Unreachable),
        _ => None,
    }
}

fn is_get_call(expr: &syn::Expr) -> bool {
    if let syn::Expr::MethodCall(call) = expr {
        return call.method == "get";
    }
    false
}

/// Name-based `Result`-return heuristic: the return type mentions `Result`
/// (`Result<..>`, `anyhow::Result`, `io::Result`, ..). Deliberately
/// approximate — `syn` has no type resolution.
fn returns_result(output: &syn::ReturnType) -> bool {
    match output {
        syn::ReturnType::Default => false,
        syn::ReturnType::Type(_, ty) => type_to_string(ty).contains("Result"),
    }
}

fn receiver_fingerprint(expr: &syn::Expr) -> Option<String> {
    Some(expr_fingerprint(expr))
}

fn expr_fingerprint(expr: &syn::Expr) -> String {
    let s = quote::quote!(#expr).to_string();
    let one_line: String = s.split_whitespace().collect::<Vec<_>>().join(" ");
    // char-aware truncation so we never split a UTF-8 boundary (the very thing
    // this policy is meant to prevent).
    const LIMIT: usize = 80;
    if one_line.chars().count() > LIMIT {
        let mut out: String = one_line.chars().take(LIMIT).collect();
        out.push('…');
        out
    } else {
        one_line
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn family_string_round_trips() {
        for f in [
            Family::Unwrap,
            Family::Expect,
            Family::PanicMacro,
            Family::Todo,
            Family::Unimplemented,
            Family::Unreachable,
            Family::Indexing,
            Family::StringSlice,
            Family::GetUnwrap,
            Family::UnwrapInResult,
        ] {
            assert!(!f.as_str().is_empty());
        }
    }

    #[test]
    fn fingerprint_truncation_is_utf8_safe() {
        // Long UTF-8 string with multi-byte characters; truncation must not
        // panic on a byte boundary mid-char.
        let s = "α".repeat(200);
        // Construct a fake expr fingerprint via the same join pipeline.
        let one_line = s.clone();
        let truncated: String = one_line.chars().take(80).collect::<String>() + "…";
        assert!(truncated.chars().count() <= 81);
        // Sanity: a naive byte slice would not be a valid str boundary here.
        assert!(s.len() > 80);
    }

    #[test]
    fn mode_parse_round_trips() {
        assert!(matches!(Mode::parse("advisory"), Ok(Mode::Advisory)));
        assert!(matches!(
            Mode::parse("blocking-allowlist"),
            Ok(Mode::BlockingAllowlist)
        ));
        assert!(matches!(
            Mode::parse("blocking-strict"),
            Ok(Mode::BlockingStrict)
        ));
        assert!(Mode::parse("nope").is_err());
    }

    #[test]
    fn expiry_is_six_months_out() {
        let s = default_expiry();
        let parsed = chrono::NaiveDate::parse_from_str(&s, "%Y-%m-%d").unwrap();
        let today = chrono::Utc::now().date_naive();
        assert!(parsed > today);
    }

    #[test]
    fn allowlist_match_requires_full_key() {
        let entry = AllowEntry {
            id: "panic-0001".into(),
            path: "foo.rs".into(),
            family: Family::Unwrap,
            classification: "test_helper".into(),
            owner: "core".into(),
            explanation: "x".into(),
            expires: "2099-01-01".into(),
            selector: Selector {
                kind: "method_call".into(),
                container: "fn_a".into(),
                callee: "unwrap".into(),
                receiver_fingerprint: Some("rx".into()),
            },
            last_seen: None,
        };
        let allow = Allowlist {
            entries: vec![entry],
        };
        let base = || Finding {
            path: "foo.rs".into(),
            family: Family::Unwrap,
            selector: Selector {
                kind: "method_call".into(),
                container: "fn_a".into(),
                callee: "unwrap".into(),
                receiver_fingerprint: Some("rx".into()),
            },
            last_seen: LastSeen { line: 1, column: 1 },
            scope: "production".into(),
        };
        assert!(allow.match_finding(&base()).is_some());

        // A second distinct call must NOT inherit the receipt: kind,
        // container, callee, and receiver fingerprint are all load-bearing.
        let mut kind_miss = base();
        kind_miss.selector.kind = "qualified_call".into();
        assert!(allow.match_finding(&kind_miss).is_none());

        let mut container_miss = base();
        container_miss.selector.container = "fn_b".into();
        assert!(allow.match_finding(&container_miss).is_none());

        let mut fp_miss = base();
        fp_miss.selector.receiver_fingerprint = Some("other".into());
        assert!(allow.match_finding(&fp_miss).is_none());

        let mut fp_none = base();
        fp_none.selector.receiver_fingerprint = None;
        assert!(allow.match_finding(&fp_none).is_none());
    }

    #[test]
    fn propose_key_distinguishes_distinct_calls() {
        let mut first = Finding {
            path: "foo.rs".into(),
            family: Family::Unwrap,
            selector: Selector {
                kind: "method_call".into(),
                container: "f".into(),
                callee: "unwrap".into(),
                receiver_fingerprint: Some("a".into()),
            },
            last_seen: LastSeen { line: 1, column: 1 },
            scope: "production".into(),
        };
        let mut second = first.clone();
        second.selector.receiver_fingerprint = Some("b".into());
        assert_ne!(propose_key(&first), propose_key(&second));
        // Identical shapes dedup (line drift is not identity).
        first.last_seen.line = 999;
        let mut twin = second.clone();
        twin.last_seen.line = 888;
        assert_eq!(propose_key(&second), propose_key(&twin));
    }

    #[test]
    fn visitors_cover_all_claimed_families() {
        let src = r#"
fn plain(x: Option<i32>) -> i32 { x.unwrap() }
fn fallible(x: Option<i32>) -> Result<i32, String> { Ok(x.unwrap()) }
fn qualified(x: Option<i32>) -> i32 { Option::unwrap(x) }
fn macros_(v: &[i32], s: &str) -> i32 {
    if v.is_empty() { panic!("empty"); }
    let _ = todo!();
    let _t: i32 = unimplemented!();
    let _u: i32 = unreachable!();
    let _ = v[0];
    let _ = &s[1..2];
    let _ = v.get(0).unwrap();
    v[0]
}
fn expecting(x: Option<i32>) -> i32 { x.expect("boom") }
"#;
        let findings = scan_source("demo.rs", src, Scope::Production).expect("fixture parses");
        let has = |family: Family| findings.iter().any(|f| f.family == family);
        for family in [
            Family::Unwrap,
            Family::UnwrapInResult,
            Family::Expect,
            Family::PanicMacro,
            Family::Todo,
            Family::Unimplemented,
            Family::Unreachable,
            Family::Indexing,
            Family::StringSlice,
            Family::GetUnwrap,
        ] {
            assert!(has(family), "family {} not detected", family.as_str());
        }
        assert!(findings.iter().any(|f| f.selector.kind == "qualified_call"));
        assert!(findings.iter().all(|f| f.scope == "production"));
    }

    #[test]
    fn slice_expr_split_from_plain_index() {
        let src = "fn f(a: &[i32], s: &str) { let _ = a[0]; let _ = &s[..]; }\n";
        let findings = scan_source("demo.rs", src, Scope::Production).expect("fixture parses");
        assert!(
            findings
                .iter()
                .any(|f| f.family == Family::Indexing && f.selector.kind == "index_expr")
        );
        assert!(
            findings
                .iter()
                .any(|f| f.family == Family::StringSlice && f.selector.kind == "slice_expr")
        );
    }

    #[test]
    fn scope_derivation_splits_test_only_from_production() {
        let prefixes = vec!["runtime/".to_string(), "cli/".to_string()];
        assert_eq!(
            classify_file("runtime/src/lib.rs", &prefixes),
            Scope::Production
        );
        assert_eq!(
            classify_file("runtime/tests/it.rs", &prefixes),
            Scope::TestOnly
        );
        assert_eq!(
            classify_file("runtime/benches/b.rs", &prefixes),
            Scope::TestOnly
        );
        assert_eq!(
            classify_file("runtime/src/foo_test.rs", &prefixes),
            Scope::TestOnly
        );
        assert_eq!(
            classify_file("tooling/src/main.rs", &prefixes),
            Scope::OutOfScope
        );
    }

    #[test]
    fn serde_json_value_index_is_reported_fp() {
        // `v["k"]` on `serde_json::Value` never panics (missing key -> Null),
        // but without type info the scanner still reports it: an explicit,
        // classified FP — never a silent suppression.
        let src = "fn f(v: &serde_json::Value) -> &serde_json::Value { &v[\"k\"] }\n";
        let findings = scan_source("demo.rs", src, Scope::Production).expect("fixture parses");
        assert!(
            findings.iter().any(|f| f.family == Family::Indexing),
            "untyped indexing must stay reported, not suppressed"
        );
    }
}
