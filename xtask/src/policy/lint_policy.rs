//! Lint-policy checker.
//!
//! Verifies that:
//! 1. The workspace MSRV in `Cargo.toml` matches `policy/clippy-lints.toml`.
//! 2. No `clippy.toml` introduces panic-family test carveouts.
//! 3. Active lint policy mirrors `[workspace.lints]`.
//! 4. No `[[planned]]` lint is activated before its `activate_when_msrv`.
//! 5. No `[[planned]]` lint remains overdue after its `activate_when_msrv`.
//!
//! Runs in advisory mode by default — failures are reported but do not stop
//! CI. Once we are confident the manifest matches reality everywhere, this
//! will graduate to blocking.

use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::Path;

use super::{Mode, ensure_report_dir, workspace_root};

#[derive(Debug, Deserialize)]
struct PolicyFile {
    msrv: String,
    #[serde(default)]
    active: Active,
    #[serde(default, rename = "planned")]
    planned: Vec<Planned>,
}

#[derive(Debug, Default, Deserialize)]
struct Active {
    #[serde(default)]
    rust: BTreeMap<String, String>,
    #[serde(default)]
    clippy: BTreeMap<String, String>,
}

#[derive(Debug, Deserialize)]
#[expect(
    dead_code,
    reason = "schema fields are deserialized for policy validation, not all are read directly"
)]
struct Planned {
    name: String,
    level: String,
    activate_when_msrv: String,
    reason: String,
}

#[derive(Debug, Default)]
struct Findings {
    issues: Vec<String>,
    bare_allows: Vec<BareAllowFinding>,
}

#[derive(Debug, Clone)]
struct BareAllowFinding {
    path: String,
    line: usize,
    attribute: String,
}

pub fn run_check(mode: Mode) -> Result<()> {
    let root = workspace_root()?;
    let report_dir = ensure_report_dir(&root)?;

    let policy_path = root.join("policy").join("clippy-lints.toml");
    if !policy_path.exists() {
        anyhow::bail!("policy/clippy-lints.toml is missing; cannot run lint-policy check");
    }
    let policy: PolicyFile = toml::from_str(&std::fs::read_to_string(&policy_path)?)
        .with_context(|| format!("parsing {}", policy_path.display()))?;

    let mut findings = Findings::default();

    check_msrv_consistency(&root, &policy, &mut findings)?;
    check_no_test_carveouts(&root, &mut findings)?;
    check_active_lints_match_cargo(&root, &policy, &mut findings)?;
    check_planned_not_active_early(&root, &policy, &mut findings)?;
    collect_bare_allow_attributes(&root, &mut findings)?;

    let summary = format!(
        "lint-policy check ({mode:?}): {} issue(s), {} bare allow attribute(s)",
        findings.issues.len(),
        findings.bare_allows.len()
    );
    println!("{summary}");
    for issue in &findings.issues {
        println!("  - {issue}");
    }

    let mut md = String::from("# Lint policy report\n\n");
    md.push_str(&format!("- mode: `{mode:?}`\n"));
    md.push_str(&format!("- issues: {}\n\n", findings.issues.len()));
    if findings.issues.is_empty() {
        md.push_str("No issues found.\n");
    } else {
        md.push_str("## Issues\n\n");
        for issue in &findings.issues {
            md.push_str(&format!("- {issue}\n"));
        }
    }
    append_bare_allow_section(&mut md, &findings.bare_allows);
    std::fs::write(report_dir.join("lint-policy.md"), md)?;

    match mode {
        Mode::Advisory => Ok(()),
        Mode::BlockingAllowlist | Mode::BlockingStrict => {
            if !findings.issues.is_empty() {
                anyhow::bail!(
                    "lint-policy check failed with {} issue(s)",
                    findings.issues.len()
                );
            }
            Ok(())
        }
    }
}

fn check_msrv_consistency(root: &Path, policy: &PolicyFile, findings: &mut Findings) -> Result<()> {
    let cargo = std::fs::read_to_string(root.join("Cargo.toml"))?;
    let toolchain_path = root.join("rust-toolchain.toml");
    let toolchain = if toolchain_path.exists() {
        std::fs::read_to_string(&toolchain_path)?
    } else {
        String::new()
    };

    let cargo_msrv = extract_kv(&cargo, "rust-version");
    let toolchain_msrv = extract_kv(&toolchain, "channel");

    if let Some(cm) = cargo_msrv.as_deref() {
        if !cm.starts_with(&policy.msrv) {
            findings.issues.push(format!(
                "Cargo.toml rust-version `{cm}` does not match policy MSRV `{}`",
                policy.msrv
            ));
        }
    } else {
        findings
            .issues
            .push("Cargo.toml is missing workspace.package.rust-version".into());
    }

    if let Some(tm) = toolchain_msrv.as_deref()
        && !tm.starts_with(&policy.msrv)
    {
        findings.issues.push(format!(
            "rust-toolchain.toml channel `{tm}` does not match policy MSRV `{}`",
            policy.msrv
        ));
    }

    Ok(())
}

fn collect_bare_allow_attributes(root: &Path, findings: &mut Findings) -> Result<()> {
    for entry in walkdir::WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| !is_ignored_dir(e))
    {
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => continue,
        };
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("rs") {
            continue;
        }
        let rel = path.strip_prefix(root).unwrap_or(path);
        let rel_str = rel.to_string_lossy().replace('\\', "/");
        if rel_str.starts_with("docs/archive/") {
            continue;
        }
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(_) => continue,
        };
        for (idx, line) in text.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("#[allow(") && !trimmed.contains("reason") {
                findings.bare_allows.push(BareAllowFinding {
                    path: rel_str.clone(),
                    line: idx + 1,
                    attribute: trimmed.to_string(),
                });
            }
        }
    }
    Ok(())
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

fn append_bare_allow_section(md: &mut String, bare_allows: &[BareAllowFinding]) {
    md.push_str("\n## Bare allow attributes\n\n");
    md.push_str(&format!("- total: {}\n", bare_allows.len()));
    if bare_allows.is_empty() {
        md.push_str("\nNo bare `#[allow(...)]` attributes found.\n");
        return;
    }

    md.push_str("\n### Path prefix breakdown\n\n");
    let mut by_prefix: BTreeMap<&str, usize> = BTreeMap::new();
    for finding in bare_allows {
        let prefix = finding.path.split('/').next().unwrap_or("<root>");
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

    md.push_str("\n### Top files by bare allow count\n\n");
    let mut by_file: BTreeMap<&str, usize> = BTreeMap::new();
    for finding in bare_allows {
        *by_file.entry(&finding.path).or_default() += 1;
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

    md.push_str("\n### Bare allow samples\n\n");
    md.push_str("| path | line | attribute |\n|---|---:|---|\n");
    for finding in bare_allows.iter().take(50) {
        md.push_str(&format!(
            "| {} | {} | `{}` |\n",
            finding.path,
            finding.line,
            finding.attribute.replace('`', "\\`")
        ));
    }
}

fn extract_kv(text: &str, key: &str) -> Option<String> {
    for line in text.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix(key) {
            let value = rest
                .trim_start()
                .strip_prefix('=')?
                .trim()
                .trim_matches('"')
                .to_string();
            if !value.is_empty() {
                return Some(value);
            }
        }
    }
    None
}

fn check_no_test_carveouts(root: &Path, findings: &mut Findings) -> Result<()> {
    let banned: &[&str] = &[
        "allow-unwrap-in-tests",
        "allow-expect-in-tests",
        "allow-panic-in-tests",
        "allow-indexing-slicing-in-tests",
        "allow-dbg-in-tests",
    ];
    for entry in walkdir::WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if !entry.file_type().is_file() {
            continue;
        }
        if entry.file_name() != "clippy.toml" {
            continue;
        }
        let rel = entry.path().strip_prefix(root).unwrap_or(entry.path());
        let text = match std::fs::read_to_string(entry.path()) {
            Ok(t) => t,
            Err(_) => continue,
        };
        for bad in banned {
            if text.contains(bad) {
                findings.issues.push(format!(
                    "{}: forbidden test carveout `{}` (no-panic policy bans this)",
                    rel.display(),
                    bad
                ));
            }
        }
    }
    Ok(())
}

#[derive(Debug, Default, PartialEq, Eq)]
struct WorkspaceLintTables {
    rust: BTreeMap<String, String>,
    clippy: BTreeMap<String, String>,
}

fn check_active_lints_match_cargo(
    root: &Path,
    policy: &PolicyFile,
    findings: &mut Findings,
) -> Result<()> {
    let cargo = std::fs::read_to_string(root.join("Cargo.toml"))?;
    let workspace_lints = workspace_lints_from_cargo(&cargo)?;

    compare_lint_table("rust", &policy.active.rust, &workspace_lints.rust, findings);
    compare_lint_table(
        "clippy",
        &policy.active.clippy,
        &workspace_lints.clippy,
        findings,
    );

    Ok(())
}

fn workspace_lints_from_cargo(cargo: &str) -> Result<WorkspaceLintTables> {
    let value: toml::Value = toml::from_str(cargo).context("parsing Cargo.toml as TOML")?;
    let Some(workspace) = value.get("workspace").and_then(toml::Value::as_table) else {
        return Ok(WorkspaceLintTables::default());
    };
    let Some(lints) = workspace.get("lints").and_then(toml::Value::as_table) else {
        return Ok(WorkspaceLintTables::default());
    };

    Ok(WorkspaceLintTables {
        rust: extract_lint_table(lints.get("rust")),
        clippy: extract_lint_table(lints.get("clippy")),
    })
}

fn extract_lint_table(value: Option<&toml::Value>) -> BTreeMap<String, String> {
    let Some(table) = value.and_then(toml::Value::as_table) else {
        return BTreeMap::new();
    };

    table
        .iter()
        .filter_map(|(name, value)| lint_level(value).map(|level| (name.clone(), level)))
        .collect()
}

fn lint_level(value: &toml::Value) -> Option<String> {
    if let Some(level) = value.as_str() {
        return Some(level.to_string());
    }
    value
        .as_table()
        .and_then(|table| table.get("level"))
        .and_then(toml::Value::as_str)
        .map(str::to_string)
}

fn compare_lint_table(
    kind: &str,
    policy: &BTreeMap<String, String>,
    cargo: &BTreeMap<String, String>,
    findings: &mut Findings,
) {
    for (name, expected_level) in policy {
        match cargo.get(name) {
            Some(actual_level) if actual_level == expected_level => {}
            Some(actual_level) => findings.issues.push(format!(
                "policy active {kind} lint `{name}` is `{expected_level}` but Cargo.toml has `{actual_level}`"
            )),
            None => findings.issues.push(format!(
                "policy active {kind} lint `{name}` is missing from Cargo.toml [workspace.lints.{kind}]"
            )),
        }
    }

    for (name, actual_level) in cargo {
        if !policy.contains_key(name) {
            findings.issues.push(format!(
                "Cargo.toml [workspace.lints.{kind}] contains `{name}` = `{actual_level}` but policy/clippy-lints.toml does not list it as active"
            ));
        }
    }
}

fn check_planned_not_active_early(
    root: &Path,
    policy: &PolicyFile,
    findings: &mut Findings,
) -> Result<()> {
    let cargo = std::fs::read_to_string(root.join("Cargo.toml"))?;
    let workspace_lints = workspace_lints_from_cargo(&cargo)?;
    let active_msrv = extract_kv(&cargo, "rust-version").unwrap_or_else(|| policy.msrv.clone());
    for planned in &policy.planned {
        let target = &planned.activate_when_msrv;
        if version_geq(&active_msrv, target) {
            findings.issues.push(format!(
                "planned lint `{}` is due at MSRV {target}; move it to active policy or reschedule it with a new reason",
                planned.name
            ));
            continue;
        }

        let lint_key = planned_lint_key(&planned.name);
        let active_early = workspace_lints.rust.contains_key(lint_key)
            || workspace_lints.clippy.contains_key(lint_key);
        if active_early {
            findings.issues.push(format!(
                "planned lint `{}` referenced in Cargo.toml before MSRV {target}",
                planned.name
            ));
        }
    }
    Ok(())
}

fn planned_lint_key(name: &str) -> &str {
    name.strip_prefix("clippy::")
        .or_else(|| name.strip_prefix("rust::"))
        .unwrap_or(name)
}

fn version_geq(a: &str, b: &str) -> bool {
    parse_version(a)
        .and_then(|av| parse_version(b).map(|bv| av >= bv))
        .unwrap_or(false)
}

fn parse_version(s: &str) -> Option<(u32, u32, u32)> {
    let mut parts = s.split('.').filter_map(|p| p.parse::<u32>().ok());
    let major = parts.next()?;
    let minor = parts.next().unwrap_or(0);
    let patch = parts.next().unwrap_or(0);
    Some((major, minor, patch))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_compare_works() {
        assert!(version_geq("1.96.0", "1.95"));
        assert!(version_geq("1.95.0", "1.95"));
        assert!(!version_geq("1.94.0", "1.95"));
    }

    #[test]
    fn extract_kv_handles_spaced_assignment() {
        let text = "rust-version = \"1.95.0\"\nother = 1\n";
        assert_eq!(extract_kv(text, "rust-version").as_deref(), Some("1.95.0"));
    }

    #[test]
    fn extract_kv_handles_tight_assignment() {
        let text = "channel=\"1.95.0\"\n";
        assert_eq!(extract_kv(text, "channel").as_deref(), Some("1.95.0"));
    }

    #[test]
    fn workspace_lints_from_cargo_extracts_string_and_table_levels() {
        let cargo = r#"
[workspace]

[workspace.lints.rust]
missing_docs = "warn"

[workspace.lints.clippy]
allow_attributes_without_reason = "deny"
manual_ilog2 = { level = "warn", priority = -1 }
"#;

        let tables = workspace_lints_from_cargo(cargo).unwrap();

        assert_eq!(
            tables.rust.get("missing_docs").map(String::as_str),
            Some("warn")
        );
        assert_eq!(
            tables
                .clippy
                .get("allow_attributes_without_reason")
                .map(String::as_str),
            Some("deny")
        );
        assert_eq!(
            tables.clippy.get("manual_ilog2").map(String::as_str),
            Some("warn")
        );
    }

    #[test]
    fn planned_lint_key_strips_known_tool_prefixes() {
        assert_eq!(planned_lint_key("clippy::manual_ilog2"), "manual_ilog2");
        assert_eq!(planned_lint_key("rust::missing_docs"), "missing_docs");
        assert_eq!(planned_lint_key("custom_lint"), "custom_lint");
    }

    #[test]
    fn bare_allow_report_summarizes_prefixes_files_and_samples() {
        let bare_allows = vec![
            BareAllowFinding {
                path: "runtime/src/lib.rs".to_string(),
                line: 10,
                attribute: "#[allow(dead_code)]".to_string(),
            },
            BareAllowFinding {
                path: "runtime/src/lib.rs".to_string(),
                line: 20,
                attribute: "#[allow(unused_imports)]".to_string(),
            },
            BareAllowFinding {
                path: "tool/src/lib.rs".to_string(),
                line: 30,
                attribute: "#[allow(dead_code)]".to_string(),
            },
        ];

        let mut md = String::new();
        append_bare_allow_section(&mut md, &bare_allows);

        assert!(md.contains("- total: 3"));
        assert!(md.contains("| runtime | 2 |"));
        assert!(md.contains("| tool | 1 |"));
        assert!(md.contains("| runtime/src/lib.rs | 2 |"));
        assert!(md.contains("| runtime/src/lib.rs | 10 | `#[allow(dead_code)]` |"));
    }
}
