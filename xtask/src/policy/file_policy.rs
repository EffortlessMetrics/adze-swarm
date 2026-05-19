//! Non-Rust file policy checker.
//!
//! Walks every git-tracked file (or every file under the workspace root if
//! we are not in a git checkout), filters out Rust sources, and reports
//! anything that is not matched by an `[[allow]]` entry in
//! `policy/non-rust-allowlist.toml`.

use anyhow::{Context, Result};
use globset::{Glob, GlobSetBuilder};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::Path;

use super::{Mode, ensure_report_dir, workspace_root};

#[derive(Debug, Default, Deserialize)]
#[allow(dead_code)]
struct AllowlistFile {
    #[serde(default)]
    schema_version: Option<String>,
    #[serde(default, rename = "allow")]
    entries: Vec<AllowEntry>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AllowEntry {
    #[serde(default)]
    pub glob: Option<String>,
    #[serde(default)]
    pub path: Option<String>,
    pub kind: String,
    pub owner: String,
    pub surface: String,
    pub classification: String,
    pub reason: String,
    #[serde(default)]
    pub covered_by: Vec<String>,
    #[serde(default)]
    pub expires: Option<String>,
    #[serde(default)]
    pub retired: Option<bool>,
    #[serde(default)]
    pub generated_by: Option<String>,
}

#[derive(Debug, Default, Serialize)]
pub struct FileReport {
    pub mode: String,
    pub total_non_rust: usize,
    pub matched: usize,
    pub allowlist_size: usize,
    pub unallowlisted: Vec<String>,
    pub unused_entries: Vec<UnusedEntry>,
    pub rust_migration_candidates: Vec<RustMigrationCandidate>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RustMigrationCandidate {
    pub path: String,
    pub kind: String,
    pub owner: String,
    pub current_surface: String,
    pub migration_target: String,
    pub reason: String,
    pub covered_by: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct UnusedEntry {
    pub glob: String,
    pub kind: String,
    pub owner: String,
}

pub fn run_check(mode: Mode) -> Result<()> {
    let root = workspace_root()?;
    let report_dir = ensure_report_dir(&root)?;
    let entries = load_allowlist(&root)?;

    let mut builders = Vec::new();
    for (entry_idx, entry) in entries.iter().enumerate() {
        if let Some(glob) = entry.glob.as_deref().or(entry.path.as_deref()) {
            match Glob::new(glob) {
                Ok(g) => builders.push((entry_idx, g)),
                Err(err) => {
                    eprintln!("warning: invalid glob `{glob}` in non-rust-allowlist.toml: {err}");
                }
            }
        }
    }
    let pattern_to_entry_idx: Vec<usize> = builders.iter().map(|(idx, _)| *idx).collect();
    let mut gsb = GlobSetBuilder::new();
    for (_, g) in &builders {
        gsb.add(g.clone());
    }
    let set = gsb.build()?;

    let files = enumerate_files(&root)?;
    let mut report = FileReport {
        mode: format!("{mode:?}"),
        allowlist_size: entries.len(),
        ..Default::default()
    };

    let mut used: BTreeSet<usize> = BTreeSet::new();
    for path in &files {
        if !is_non_rust_candidate(path) {
            continue;
        }
        report.total_non_rust += 1;
        let matches = set.matches(path);
        if matches.is_empty() {
            report.unallowlisted.push(path.clone());
        } else {
            report.matched += 1;
            let mut matched_entries = Vec::new();
            for m in matches {
                if let Some(entry_idx) = pattern_to_entry_idx.get(m).copied() {
                    used.insert(entry_idx);
                    matched_entries.push((entry_idx, &entries[entry_idx]));
                }
            }
            if let Some(candidate) = rust_migration_candidate(path, &matched_entries) {
                report.rust_migration_candidates.push(candidate);
            }
        }
    }

    for (idx, _g) in &builders {
        if !used.contains(idx) {
            let entry = &entries[*idx];
            if entry.retired.unwrap_or(false) {
                continue;
            }
            report.unused_entries.push(UnusedEntry {
                glob: entry
                    .glob
                    .clone()
                    .or_else(|| entry.path.clone())
                    .unwrap_or_default(),
                kind: entry.kind.clone(),
                owner: entry.owner.clone(),
            });
        }
    }

    write_reports(&report_dir, &report)?;
    print_summary(&report);

    match mode {
        Mode::Advisory => Ok(()),
        Mode::BlockingAllowlist => {
            if !report.unallowlisted.is_empty() {
                anyhow::bail!(
                    "file-policy check failed: {} unallowlisted files",
                    report.unallowlisted.len()
                );
            }
            Ok(())
        }
        Mode::BlockingStrict => {
            if !report.unallowlisted.is_empty() || !report.unused_entries.is_empty() {
                anyhow::bail!(
                    "file-policy check failed: {} unallowlisted, {} unused entries",
                    report.unallowlisted.len(),
                    report.unused_entries.len()
                );
            }
            Ok(())
        }
    }
}

fn is_non_rust_candidate(rel: &str) -> bool {
    if rel.ends_with(".rs") {
        return false;
    }
    if rel.starts_with("target/") || rel == "target" {
        return false;
    }
    if rel.starts_with(".git/") {
        return false;
    }
    true
}

fn load_allowlist(root: &Path) -> Result<Vec<AllowEntry>> {
    let path = root.join("policy").join("non-rust-allowlist.toml");
    if !path.exists() {
        return Ok(Vec::new());
    }
    let text =
        std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    let parsed: AllowlistFile =
        toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;
    Ok(parsed.entries)
}

fn enumerate_files(root: &Path) -> Result<Vec<String>> {
    if let Some(files) = git_ls_files(root) {
        return Ok(files);
    }
    let mut out = Vec::new();
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
        if let Ok(rel) = entry.path().strip_prefix(root) {
            out.push(rel.to_string_lossy().replace('\\', "/"));
        }
    }
    Ok(out)
}

fn is_ignored_dir(entry: &walkdir::DirEntry) -> bool {
    if entry.depth() == 0 {
        return false;
    }
    matches!(
        entry.file_name().to_string_lossy().as_ref(),
        "target" | ".git" | "node_modules"
    )
}

fn git_ls_files(root: &Path) -> Option<Vec<String>> {
    let output = std::process::Command::new("git")
        .args(["ls-files", "-z"])
        .current_dir(root)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let mut files = Vec::new();
    for chunk in output.stdout.split(|b| *b == 0) {
        if chunk.is_empty() {
            continue;
        }
        match std::str::from_utf8(chunk) {
            Ok(s) => files.push(s.replace('\\', "/")),
            Err(_) => continue,
        }
    }
    Some(files)
}

fn write_reports(dir: &Path, report: &FileReport) -> Result<()> {
    std::fs::write(
        dir.join("file-policy.json"),
        serde_json::to_string_pretty(report)?,
    )?;
    let mut md = String::new();
    md.push_str("# File policy report\n\n");
    md.push_str(&format!("- mode: `{}`\n", report.mode));
    md.push_str(&format!(
        "- non-rust files scanned: {}\n",
        report.total_non_rust
    ));
    md.push_str(&format!("- matched: {}\n", report.matched));
    md.push_str(&format!("- allowlist entries: {}\n", report.allowlist_size));
    md.push_str(&format!(
        "- unallowlisted: {}\n",
        report.unallowlisted.len()
    ));
    md.push_str(&format!(
        "- unused allowlist entries: {}\n",
        report.unused_entries.len()
    ));
    md.push_str(&format!(
        "- rust migration candidates: {}\n",
        report.rust_migration_candidates.len()
    ));

    if !report.unallowlisted.is_empty() {
        md.push_str("\n## Unallowlisted (top 100)\n\n");
        for p in report.unallowlisted.iter().take(100) {
            md.push_str(&format!("- `{p}`\n"));
        }
    }
    if !report.unused_entries.is_empty() {
        md.push_str("\n## Unused allowlist entries\n\n");
        md.push_str("| glob | kind | owner |\n|---|---|---|\n");
        for e in &report.unused_entries {
            md.push_str(&format!("| `{}` | {} | {} |\n", e.glob, e.kind, e.owner));
        }
    }

    if !report.rust_migration_candidates.is_empty() {
        md.push_str("\n## Rust migration candidates\n\n");
        md.push_str("These matched non-Rust tooling surfaces are good candidates to move into `xtask` or a core Rust crate. Fixture, generated, docs, and platform-required configuration files are intentionally excluded.\n\n");
        md.push_str(
            "| path | kind | owner | surface | target | reason | covered by |\n|---|---|---|---|---|---|---|\n",
        );
        for c in &report.rust_migration_candidates {
            md.push_str(&format!(
                "| `{}` | {} | {} | {} | {} | {} | {} |\n",
                c.path,
                c.kind,
                c.owner,
                c.current_surface,
                c.migration_target,
                c.reason,
                c.covered_by.join("<br>")
            ));
        }
    }

    std::fs::write(dir.join("file-policy.md"), md)?;
    Ok(())
}

fn rust_migration_candidate(
    path: &str,
    matched_entries: &[(usize, &AllowEntry)],
) -> Option<RustMigrationCandidate> {
    let entry = matched_entries
        .iter()
        .map(|(_, entry)| *entry)
        .find(|entry| is_migratable_entry(path, entry))?;
    Some(RustMigrationCandidate {
        path: path.to_owned(),
        kind: entry.kind.clone(),
        owner: entry.owner.clone(),
        current_surface: entry.surface.clone(),
        migration_target: migration_target_for(path, entry).to_owned(),
        reason: entry.reason.clone(),
        covered_by: entry.covered_by.clone(),
    })
}

fn is_migratable_entry(path: &str, entry: &AllowEntry) -> bool {
    if entry.retired.unwrap_or(false) || entry.classification == "generated" {
        return false;
    }
    if matches!(
        entry.classification.as_str(),
        "docs" | "fixtures" | "test" | "config"
    ) {
        return false;
    }
    if is_grammar_definition(path) {
        return entry.surface == "grammar" || entry.kind.contains("grammar");
    }
    let has_tool_extension = matches!(
        Path::new(path).extension().and_then(|ext| ext.to_str()),
        Some("sh" | "py" | "js")
    ) || is_hook_script(path);
    let is_tool_surface = matches!(
        entry.surface.as_str(),
        "tooling" | "release" | "ci" | "build"
    );
    let is_tool_kind = entry.kind.contains("tooling")
        || entry.kind.contains("hook")
        || entry.kind.contains("orchestrator")
        || entry.kind == "web_demo";
    has_tool_extension && is_tool_surface && is_tool_kind
}

fn is_grammar_definition(path: &str) -> bool {
    path.ends_with("grammar.js")
}

fn is_hook_script(path: &str) -> bool {
    (path.starts_with(".githooks/") || path.starts_with("hooks/"))
        && Path::new(path).extension().and_then(|ext| ext.to_str()) != Some("md")
}

fn migration_target_for(path: &str, entry: &AllowEntry) -> &'static str {
    if is_grammar_definition(path) {
        "Rust grammar crate using Adze annotations"
    } else if path.starts_with(".githooks/") || path.starts_with("hooks/") {
        "xtask lint/preflight subcommand plus thin hook shim"
    } else if path.starts_with("scripts/ci/") {
        "xtask CI planning/reporting module"
    } else if path.starts_with("scripts/") {
        "xtask policy/build subcommand"
    } else if path.starts_with("runtime/build-wasm") {
        "runtime/core WASM build subcommand"
    } else if path.starts_with("golden-tests/") {
        "xtask golden-test command"
    } else if path.starts_with("tools/ts-bridge/") {
        "ts-bridge Rust crate"
    } else if entry.surface == "demo" {
        "Rust-backed playground/demo command"
    } else {
        "core Rust tooling surface"
    }
}

fn print_summary(report: &FileReport) {
    println!("file-policy check ({})", report.mode);
    println!("  non-rust scanned: {}", report.total_non_rust);
    println!("  matched:          {}", report.matched);
    println!("  unallowlisted:    {}", report.unallowlisted.len());
    println!(
        "  rust migrations:  {}",
        report.rust_migration_candidates.len()
    );
    println!("  unused entries:   {}", report.unused_entries.len());
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(kind: &str, surface: &str, classification: &str) -> AllowEntry {
        AllowEntry {
            glob: Some("scripts/**".to_string()),
            path: None,
            kind: kind.to_string(),
            owner: "core/build".to_string(),
            surface: surface.to_string(),
            classification: classification.to_string(),
            reason: "test entry".to_string(),
            covered_by: vec!["cargo test -p xtask".to_string()],
            expires: None,
            retired: None,
            generated_by: None,
        }
    }

    #[test]
    fn rust_migration_candidate_accepts_tooling_scripts() {
        let entries = [entry("shell_tooling", "tooling", "tooling")];
        let candidate = rust_migration_candidate("scripts/check.sh", &[(4, &entries[0])])
            .expect("tooling script should be a migration candidate");

        assert_eq!(candidate.current_surface, "tooling");
        assert_eq!(candidate.migration_target, "xtask policy/build subcommand");
        assert_eq!(candidate.covered_by, ["cargo test -p xtask"]);
    }

    #[test]
    fn rust_migration_candidate_skips_fixture_scripts() {
        let entries = [entry("language_fixture", "fixtures", "test")];
        let candidate = rust_migration_candidate("fixtures/example.py", &[(2, &entries[0])]);

        assert!(candidate.is_none());
    }

    #[test]
    fn rust_migration_candidate_routes_grammar_js_to_rust_grammar() {
        let entries = [entry("grammar_input", "grammar", "production")];
        let candidate =
            rust_migration_candidate("grammars/example/grammar.js", &[(7, &entries[0])])
                .expect("production grammar.js should be a migration candidate");

        assert_eq!(
            candidate.migration_target,
            "Rust grammar crate using Adze annotations"
        );
    }

    #[test]
    fn rust_migration_candidate_routes_extensionless_hooks_to_xtask() {
        let entries = [entry("git_hook", "tooling", "tooling")];
        let candidate = rust_migration_candidate(".githooks/pre-push", &[(0, &entries[0])])
            .expect("extensionless hook should be a migration candidate");

        assert_eq!(
            candidate.migration_target,
            "xtask lint/preflight subcommand plus thin hook shim"
        );
    }
}
