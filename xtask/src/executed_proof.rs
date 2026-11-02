//! Executed proof selection (#1058, parent #998).
//!
//! Selection is proven against actual compiled test executables obtained
//! from Cargo (`cargo test --no-run --message-format=json`) and enumerated
//! with the harness's own `--list`, never by scanning source for `fn`
//! names. A receipt binds the command to source/tree, toolchain, profile,
//! features, executable identity, selected test IDs, counts, the terminal
//! child status, and the raw-output file. Zero, helper-only, wrong-module,
//! cfg-disabled, and ignored-only selections are rejected; a failing child
//! cannot be hidden behind a successful wrapper because this runner spawns
//! the test executable directly and checks both its exit status and the
//! parsed `test result:` rows.

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug)]
pub enum ProofError {
    Malformed(String),
    UndeclaredFeature(String),
    BuildFailed(String),
    NoTestExecutable,
    UnsupportedHarness(String),
    UnsupportedFlag(String),
    ZeroSelection(String),
    IgnoredOnly(String),
    ChildFailed { code: Option<i32>, detail: String },
    InconsistentReceipt(String),
    Io(String),
}

impl std::fmt::Display for ProofError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Malformed(m) => write!(f, "malformed proof command: {m}"),
            Self::UndeclaredFeature(m) => write!(f, "undeclared feature: {m}"),
            Self::BuildFailed(m) => write!(f, "cargo build of test executables failed: {m}"),
            Self::NoTestExecutable => write!(f, "cargo produced no test executable"),
            Self::UnsupportedHarness(m) => write!(f, "unsupported test harness: {m}"),
            Self::UnsupportedFlag(m) => write!(
                f,
                "unsupported cargo test flag: {m} (supported: -p/--package, --manifest-path, --features, --test, --lib, --profile; harness: --exact after `--`)"
            ),
            Self::ZeroSelection(m) => write!(f, "zero tests selected: {m}"),
            Self::IgnoredOnly(m) => write!(f, "ignored-only selection: {m}"),
            Self::ChildFailed { code, detail } => {
                write!(f, "test child failed (exit {code:?}): {detail}")
            }
            Self::InconsistentReceipt(m) => write!(f, "inconsistent receipt: {m}"),
            Self::Io(m) => write!(f, "io: {m}"),
        }
    }
}
impl std::error::Error for ProofError {}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Counts {
    pub selected: usize,
    pub executed: usize,
    pub passed: usize,
    pub failed: usize,
    pub ignored: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExecutableIdentity {
    pub path: String,
    pub digest: String,
    pub len: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProofReceipt {
    pub command_argv: Vec<String>,
    pub source_tree: String,
    pub toolchain: String,
    pub profile: Option<String>,
    pub features: Vec<String>,
    pub executables: Vec<ExecutableIdentity>,
    pub selected_test_ids: Vec<String>,
    pub counts: Counts,
    pub child_exit_code: Option<i32>,
    pub terminal_status: String,
    pub raw_output_ref: String,
    #[serde(default)]
    pub include_ignored: bool,
}

impl ProofReceipt {
    /// Re-validate a receipt against its raw output. This is the check a
    /// wrapper cannot bypass: counts must match the terminal result rows
    /// actually present in the retained output.
    pub fn validate_against_output(&self, raw_output: &str) -> Result<(), ProofError> {
        validate_counts(
            self,
            &parse_result_rows(raw_output),
            raw_output,
            self.executables.len(),
        )
    }

    /// Proof for one identity (source/toolchain/profile/features/
    /// executables/command) must not be relabeled for another.
    pub fn matches_identity(&self, other: &ProofReceipt) -> bool {
        self.command_argv == other.command_argv
            && self.source_tree == other.source_tree
            && self.toolchain == other.toolchain
            && self.profile == other.profile
            && self.features == other.features
            && self.executables == other.executables
    }
}

pub struct ProofSpec {
    pub root: PathBuf,
    pub command: String,
    pub raw_output_path: PathBuf,
    pub include_ignored: bool,
}

#[derive(Default, Debug)]
struct Parsed {
    package: Option<String>,
    manifest_path: Option<String>,
    features: Vec<String>,
    profile: Option<String>,
    test_target: Option<String>,
    lib: bool,
    doc: bool,
    filter: Option<String>,
    exact: bool,
}

fn tokenize(command: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    for ch in command.chars() {
        match ch {
            '"' => quoted = !quoted,
            ch if ch.is_whitespace() && !quoted => {
                if !cur.is_empty() {
                    tokens.push(std::mem::take(&mut cur));
                }
            }
            ch => cur.push(ch),
        }
    }
    if !cur.is_empty() {
        tokens.push(cur);
    }
    tokens
}

fn parse(command: &str) -> Result<Parsed, ProofError> {
    let tokens = tokenize(command);
    if tokens.len() < 2 || tokens[0] != "cargo" || tokens[1] != "test" {
        return Err(ProofError::Malformed(command.to_string()));
    }
    let mut p = Parsed::default();
    let mut filters = Vec::new();
    let mut it = tokens.iter().skip(2).peekable();
    while let Some(tok) = it.next() {
        match tok.as_str() {
            "--" => {
                for rest in it.by_ref() {
                    if rest == "--exact" {
                        p.exact = true;
                    } else {
                        // Harness flags affect selection/output; never drop one.
                        return Err(ProofError::UnsupportedFlag(rest.clone()));
                    }
                }
            }
            "-p" | "--package" => {
                p.package = Some(
                    it.next()
                        .ok_or_else(|| ProofError::Malformed("-p".into()))?
                        .clone(),
                )
            }
            "--manifest-path" => {
                p.manifest_path = Some(
                    it.next()
                        .ok_or_else(|| ProofError::Malformed("--manifest-path".into()))?
                        .clone(),
                )
            }
            "--features" => {
                let v = it
                    .next()
                    .ok_or_else(|| ProofError::Malformed("--features".into()))?;
                p.features
                    .extend(v.split(',').map(|s| s.trim().trim_matches('"').to_string()));
            }
            "--test" => {
                p.test_target = Some(
                    it.next()
                        .ok_or_else(|| ProofError::Malformed("--test".into()))?
                        .clone(),
                )
            }
            "--lib" => p.lib = true,
            "--doc" => p.doc = true,
            "--profile" => {
                p.profile = Some(
                    it.next()
                        .ok_or_else(|| ProofError::Malformed("--profile".into()))?
                        .clone(),
                )
            }
            t if t.starts_with("--features=") => {
                let v = t.strip_prefix("--features=").expect("prefix checked");
                if v.is_empty() {
                    return Err(ProofError::Malformed("--features=".into()));
                }
                p.features
                    .extend(v.split(',').map(|s| s.trim().to_string()));
            }
            t if t.starts_with('-') => {
                // `=` forms of the supported value flags above.
                if let Some(v) = t.strip_prefix("--profile=") {
                    if v.is_empty() {
                        return Err(ProofError::Malformed("--profile=".into()));
                    }
                    p.profile = Some(v.to_string());
                } else if let Some(v) = t.strip_prefix("--package=") {
                    if v.is_empty() {
                        return Err(ProofError::Malformed("--package=".into()));
                    }
                    p.package = Some(v.to_string());
                } else if let Some(v) = t.strip_prefix("--manifest-path=") {
                    if v.is_empty() {
                        return Err(ProofError::Malformed("--manifest-path=".into()));
                    }
                    p.manifest_path = Some(v.to_string());
                } else if let Some(v) = t.strip_prefix("--test=") {
                    if v.is_empty() {
                        return Err(ProofError::Malformed("--test=".into()));
                    }
                    p.test_target = Some(v.to_string());
                } else {
                    // Reject, never silently drop: target selectors
                    // (--bin/--bins/--tests/--examples/--example/--benches/
                    // --bench/--all-targets), profile/feature shortcuts
                    // (--release/--all-features/--no-default-features), and
                    // every other unknown `-`/`--*` token (including
                    // `--flag=value` forms) would otherwise change what
                    // Cargo builds/selects while the receipt still claimed
                    // the narrower supported selection above.
                    return Err(ProofError::UnsupportedFlag(t.to_string()));
                }
            }
            t => filters.push(t.trim_matches('"').to_string()),
        }
    }
    if filters.len() > 1 {
        return Err(ProofError::Malformed(format!(
            "cargo accepts one positional test filter, found {}",
            filters.len()
        )));
    }
    if p.exact && filters.is_empty() {
        return Err(ProofError::Malformed("--exact without a filter".into()));
    }
    if p.doc {
        return Err(ProofError::UnsupportedHarness(
            "doctests have no libtest executable; explicit adapter required".into(),
        ));
    }
    p.filter = filters.into_iter().next();
    Ok(p)
}

fn run_capture(
    program: &str,
    args: &[String],
    cwd: &Path,
) -> Result<(Option<i32>, String), ProofError> {
    let out = Command::new(program)
        .args(args)
        .current_dir(cwd)
        .output()
        .map_err(|e| ProofError::Io(format!("spawn {program}: {e}")))?;
    let mut text = String::from_utf8_lossy(&out.stdout).into_owned();
    text.push_str(&String::from_utf8_lossy(&out.stderr));
    Ok((out.status.code(), text))
}

fn cargo_metadata(root: &Path, parsed: &Parsed) -> Result<(PathBuf, Vec<String>), ProofError> {
    let mut args = vec![
        "metadata".to_string(),
        "--no-deps".to_string(),
        "--format-version".to_string(),
        "1".to_string(),
    ];
    if let Some(m) = &parsed.manifest_path {
        args.push("--manifest-path".into());
        args.push(root.join(m).display().to_string());
    }
    let (code, text) = run_capture("cargo", &args, root)?;
    if code != Some(0) {
        return Err(ProofError::BuildFailed(format!("cargo metadata: {text}")));
    }
    let value: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| ProofError::InconsistentReceipt(e.to_string()))?;
    let packages = value["packages"]
        .as_array()
        .ok_or_else(|| ProofError::InconsistentReceipt("metadata packages".into()))?;
    let chosen = match &parsed.package {
        Some(name) => packages
            .iter()
            .find(|p| p["name"].as_str() == Some(name.as_str())),
        None => packages.first(),
    }
    .ok_or_else(|| ProofError::Malformed("package not found by cargo metadata".into()))?;
    let manifest = PathBuf::from(
        chosen["manifest_path"]
            .as_str()
            .ok_or_else(|| ProofError::InconsistentReceipt("manifest_path".into()))?,
    );
    let features = chosen["features"]
        .as_object()
        .map(|m| m.keys().cloned().collect())
        .unwrap_or_default();
    Ok((manifest, features))
}

fn fnv_digest(path: &Path) -> Result<(String, u64), ProofError> {
    let bytes = std::fs::read(path).map_err(|e| ProofError::Io(e.to_string()))?;
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for b in &bytes {
        hash ^= u64::from(*b);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    Ok((format!("fnv1a64:{hash:016x}"), bytes.len() as u64))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListedTest {
    pub name: String,
    pub ignored: bool,
}

fn list_tests(exe: &Path) -> Result<Vec<ListedTest>, ProofError> {
    let list = |extra: &[&str]| -> Result<Vec<String>, ProofError> {
        let mut args: Vec<String> = extra.iter().map(|s| s.to_string()).collect();
        args.push("--list".into());
        let (code, text) = run_capture(&exe.display().to_string(), &args, Path::new("."))?;
        if code != Some(0) {
            return Err(ProofError::UnsupportedHarness(format!(
                "{exe:?} --list exit {code:?}"
            )));
        }
        let names: Vec<String> = text
            .lines()
            .filter_map(|l| l.strip_suffix(": test").map(|s| s.trim().to_string()))
            .collect();
        if names.is_empty() && !text.contains("0 tests") {
            return Err(ProofError::UnsupportedHarness(format!(
                "no libtest listing from {}",
                exe.display()
            )));
        }
        Ok(names)
    };
    let all = list(&[])?;
    let ignored: BTreeSet<String> = list(&["--ignored"])?.into_iter().collect();
    Ok(all
        .into_iter()
        .map(|name| ListedTest {
            ignored: ignored.contains(&name),
            name,
        })
        .collect())
}

fn parse_result_rows(raw: &str) -> Vec<Counts> {
    let mut rows = Vec::new();
    for line in raw.lines() {
        let Some(rest) = line.trim().strip_prefix("test result:") else {
            continue;
        };
        // Parts look like "ok. 1 passed" / "0 failed" / "1 ignored".
        let num_before = |key: &str| -> usize {
            for part in rest.replace('.', " ").split(';') {
                let part = part.trim();
                if let Some(v) = part.strip_suffix(key)
                    && let Some(n) = v.split_whitespace().last()
                {
                    return n.parse().unwrap_or(0);
                }
            }
            0
        };
        rows.push(Counts {
            selected: 0,
            passed: num_before("passed"),
            failed: num_before("failed"),
            ignored: num_before("ignored"),
            executed: num_before("passed") + num_before("failed"),
        });
    }
    rows
}

fn validate_counts(
    receipt: &ProofReceipt,
    rows: &[Counts],
    raw: &str,
    expected_rows: usize,
) -> Result<(), ProofError> {
    if receipt.counts.selected == 0 {
        return Err(ProofError::ZeroSelection("receipt".into()));
    }
    // Selection is bound to the enumerated IDs, not just a bare count: a
    // receipt cannot claim tests it does not name (or vice versa).
    if receipt.selected_test_ids.len() != receipt.counts.selected {
        return Err(ProofError::InconsistentReceipt(format!(
            "selected_test_ids len {} != counts.selected {}",
            receipt.selected_test_ids.len(),
            receipt.counts.selected
        )));
    }
    match receipt.counts.executed.checked_add(receipt.counts.ignored) {
        Some(accounted) if accounted <= receipt.counts.selected => {}
        _ => {
            return Err(ProofError::InconsistentReceipt(format!(
                "executed {} + ignored {} exceeds selected {}",
                receipt.counts.executed, receipt.counts.ignored, receipt.counts.selected
            )));
        }
    }
    if receipt.counts.passed.checked_add(receipt.counts.failed) != Some(receipt.counts.executed) {
        return Err(ProofError::InconsistentReceipt(format!(
            "executed {} != passed {} + failed {}",
            receipt.counts.executed, receipt.counts.passed, receipt.counts.failed
        )));
    }
    if rows.is_empty() || !raw.contains("test result:") {
        return Err(ProofError::InconsistentReceipt(
            "missing/truncated terminal test result".into(),
        ));
    }
    if rows.len() != expected_rows {
        return Err(ProofError::InconsistentReceipt(format!(
            "expected {expected_rows} terminal result rows, found {} (duplicate/inconsistent rows)",
            rows.len()
        )));
    }
    let (passed, failed, ignored) = rows.iter().fold((0, 0, 0), |(p, f, i), r| {
        (p + r.passed, f + r.failed, i + r.ignored)
    });
    if passed != receipt.counts.passed
        || failed != receipt.counts.failed
        || ignored != receipt.counts.ignored
    {
        return Err(ProofError::InconsistentReceipt(
            "receipt counts do not match raw output".into(),
        ));
    }
    if failed > 0 {
        return Err(ProofError::ChildFailed {
            code: receipt.child_exit_code,
            detail: format!("{failed} failed in raw output"),
        });
    }
    if receipt.counts.executed == 0 {
        if receipt.counts.ignored > 0 && !receipt.include_ignored {
            return Err(ProofError::IgnoredOnly("receipt".into()));
        }
        return Err(ProofError::ZeroSelection("executed 0".into()));
    }
    if receipt.child_exit_code != Some(0) {
        return Err(ProofError::ChildFailed {
            code: receipt.child_exit_code,
            detail: "non-zero terminal status".into(),
        });
    }
    Ok(())
}

pub fn run_executed_proof(spec: &ProofSpec) -> Result<ProofReceipt, ProofError> {
    let parsed = parse(&spec.command)?;
    // Feature validation applies to -p and --manifest-path alike (#1058).
    let (_manifest, declared) = cargo_metadata(&spec.root, &parsed)?;
    for feature in &parsed.features {
        let base = feature.split('/').next().unwrap_or(feature);
        if !feature.contains('/') && !declared.iter().any(|f| f == base) {
            return Err(ProofError::UndeclaredFeature(format!(
                "{feature} (declared: {})",
                declared.join(",")
            )));
        }
    }

    // Build actual test executables through Cargo itself.
    let mut build_args: Vec<String> = vec!["test".into()];
    if let Some(p) = &parsed.package {
        build_args.push("-p".into());
        build_args.push(p.clone());
    }
    if let Some(m) = &parsed.manifest_path {
        build_args.push("--manifest-path".into());
        build_args.push(spec.root.join(m).display().to_string());
    }
    if !parsed.features.is_empty() {
        build_args.push("--features".into());
        build_args.push(parsed.features.join(","));
    }
    if let Some(profile) = &parsed.profile {
        build_args.push("--profile".into());
        build_args.push(profile.clone());
    }
    if let Some(t) = &parsed.test_target {
        build_args.push("--test".into());
        build_args.push(t.clone());
    } else if parsed.lib {
        build_args.push("--lib".into());
    }
    build_args.push("--no-run".into());
    build_args.push("--message-format=json-render-diagnostics".into());
    let (build_code, build_out) = run_capture("cargo", &build_args, &spec.root)?;
    if build_code != Some(0) {
        return Err(ProofError::BuildFailed(build_out));
    }
    let mut executables: Vec<PathBuf> = Vec::new();
    for line in build_out.lines() {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if v["reason"].as_str() != Some("compiler-artifact") {
            continue;
        }
        let Some(exe) = v["executable"].as_str() else {
            continue;
        };
        let kinds: Vec<&str> = v["target"]["kind"]
            .as_array()
            .map(|a| a.iter().filter_map(|k| k.as_str()).collect())
            .unwrap_or_default();
        let name = v["target"]["name"].as_str().unwrap_or_default();
        let wanted = if let Some(t) = &parsed.test_target {
            name.replace('-', "_") == t.replace('-', "_")
        } else if parsed.lib {
            kinds.contains(&"lib")
        } else {
            kinds.contains(&"test") || kinds.contains(&"lib")
        };
        if wanted {
            executables.push(PathBuf::from(exe));
        }
    }
    executables.sort();
    executables.dedup();
    if executables.is_empty() {
        return Err(ProofError::NoTestExecutable);
    }

    // Enumerate with the harness's own listing and apply libtest rules.
    let mut selected: Vec<(PathBuf, ListedTest)> = Vec::new();
    let mut identities = Vec::new();
    for exe in &executables {
        let (digest, len) = fnv_digest(exe)?;
        identities.push(ExecutableIdentity {
            path: exe.display().to_string(),
            digest,
            len,
        });
        for test in list_tests(exe)? {
            let matched = match &parsed.filter {
                None => true,
                Some(f) if parsed.exact => &test.name == f,
                Some(f) => test.name.contains(f.as_str()),
            };
            if matched {
                selected.push((exe.clone(), test));
            }
        }
    }
    if selected.is_empty() {
        return Err(ProofError::ZeroSelection(format!(
            "filter {:?} exact={} over {} executable(s)",
            parsed.filter,
            parsed.exact,
            executables.len()
        )));
    }
    if !spec.include_ignored && selected.iter().all(|(_, t)| t.ignored) {
        return Err(ProofError::IgnoredOnly(format!(
            "{} selected, all ignored",
            selected.len()
        )));
    }

    // Execute directly (no shell/tee wrapper) and retain raw output.
    let used_exes: BTreeSet<PathBuf> = selected.iter().map(|(e, _)| e.clone()).collect();
    let mut raw = String::new();
    let mut child_code = Some(0);
    for exe in &used_exes {
        let mut args: Vec<String> = Vec::new();
        if let Some(f) = &parsed.filter {
            args.push(f.clone());
        }
        if parsed.exact {
            args.push("--exact".into());
        }
        if spec.include_ignored {
            args.push("--include-ignored".into());
        }
        let (code, text) = run_capture(&exe.display().to_string(), &args, &spec.root)?;
        raw.push_str(&format!("$ {} {}\n{text}", exe.display(), args.join(" ")));
        if code != Some(0) {
            child_code = code;
        }
    }
    if let Some(parent) = spec.raw_output_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| ProofError::Io(e.to_string()))?;
    }
    std::fs::write(&spec.raw_output_path, &raw).map_err(|e| ProofError::Io(e.to_string()))?;

    let rows = parse_result_rows(&raw);
    let (passed, failed, ignored) = rows.iter().fold((0, 0, 0), |(p, f, i), r| {
        (p + r.passed, f + r.failed, i + r.ignored)
    });
    let (_, toolchain_out) =
        run_capture("rustc", &["--version".into()], &spec.root).unwrap_or((None, String::new()));
    let (_, head_out) = run_capture("git", &["rev-parse".into(), "HEAD".into()], &spec.root)
        .unwrap_or((None, String::new()));
    let (_, status_out) = run_capture("git", &["status".into(), "--porcelain".into()], &spec.root)
        .unwrap_or((None, String::new()));
    let source_tree = format!(
        "{}{}",
        head_out.trim(),
        if status_out.trim().is_empty() {
            " (clean)"
        } else {
            " (dirty)"
        }
    );
    let used_identities: Vec<_> = identities
        .into_iter()
        .filter(|id| used_exes.iter().any(|e| e.display().to_string() == id.path))
        .collect();
    let receipt = ProofReceipt {
        command_argv: tokenize(&spec.command),
        source_tree,
        toolchain: toolchain_out.trim().to_string(),
        profile: parsed.profile.clone(),
        features: parsed.features.clone(),
        executables: used_identities,
        selected_test_ids: selected.iter().map(|(_, t)| t.name.clone()).collect(),
        counts: Counts {
            selected: selected.len(),
            executed: passed + failed,
            passed,
            failed,
            ignored,
        },
        child_exit_code: child_code,
        terminal_status: format!("exit:{child_code:?}"),
        raw_output_ref: spec.raw_output_path.display().to_string(),
        include_ignored: spec.include_ignored,
    };
    if child_code != Some(0) || failed > 0 {
        return Err(ProofError::ChildFailed {
            code: child_code,
            detail: format!(
                "{failed} failed; raw output retained at {}",
                receipt.raw_output_ref
            ),
        });
    }
    receipt.validate_against_output(&raw)?;
    Ok(receipt)
}
