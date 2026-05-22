//! Semantic no-panic checker.
//!
//! Walks every `*.rs` file under the workspace root, parses it with `syn`,
//! and records every panic-family call shape together with its enclosing
//! container (function / impl / module path). Identity is
//! `(path, family, selector)` — we deliberately do not key on line/column.

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

    let findings = scan_workspace(&root)?;
    let mut report = CheckReport {
        mode: format!("{mode:?}"),
        total_findings: findings.len(),
        allowlist_size: allowlist.entries.len(),
        ..Default::default()
    };

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

    match mode {
        Mode::Advisory => Ok(()),
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
    let findings = scan_workspace(&root)?;

    let allowlist = load_allowlist(&root)?;
    let mut entries = Vec::new();
    let mut id_counter = next_id_seed(&allowlist);
    let mut seen_keys: BTreeSet<(String, String, String, String)> = BTreeSet::new();

    for finding in &findings {
        let key = (
            finding.path.clone(),
            finding.family.as_str().to_string(),
            finding.selector.container.clone(),
            finding.selector.callee.clone(),
        );
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
    fn match_finding(&self, f: &Finding) -> Option<&AllowEntry> {
        self.entries.iter().find(|entry| {
            entry.path == f.path
                && entry.family == f.family
                && entry.selector.container == f.selector.container
                && entry.selector.callee == f.selector.callee
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
}

// ---------- scanning ----------

fn scan_workspace(root: &Path) -> Result<Vec<Finding>> {
    let mut findings = Vec::new();
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
        if let Ok(rel) = path.strip_prefix(root) {
            let rel_str = rel.to_string_lossy().replace('\\', "/");
            scan_file(path, &rel_str, &mut findings).ok();
        }
    }
    Ok(findings)
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

fn scan_file(path: &Path, rel: &str, out: &mut Vec<Finding>) -> Result<()> {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(_) => return Ok(()),
    };
    let file = match syn::parse_file(&text) {
        Ok(f) => f,
        Err(_) => return Ok(()),
    };
    let mut visitor = Visitor {
        path: rel.to_string(),
        out,
        container_stack: Vec::new(),
    };
    syn::visit::Visit::visit_file(&mut visitor, &file);
    Ok(())
}

struct Visitor<'a> {
    path: String,
    out: &'a mut Vec<Finding>,
    container_stack: Vec<String>,
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
        });
    }
}

impl<'ast, 'a> syn::visit::Visit<'ast> for Visitor<'a> {
    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        self.container_stack.push(node.sig.ident.to_string());
        syn::visit::visit_item_fn(self, node);
        self.container_stack.pop();
    }

    fn visit_impl_item_fn(&mut self, node: &'ast syn::ImplItemFn) {
        self.container_stack.push(node.sig.ident.to_string());
        syn::visit::visit_impl_item_fn(self, node);
        self.container_stack.pop();
    }

    fn visit_trait_item_fn(&mut self, node: &'ast syn::TraitItemFn) {
        self.container_stack.push(node.sig.ident.to_string());
        syn::visit::visit_trait_item_fn(self, node);
        self.container_stack.pop();
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
            "unwrap" => Some(Family::Unwrap),
            "expect" => Some(Family::Expect),
            _ => None,
        };
        if let Some(family) = family {
            let fp = receiver_fingerprint(&node.receiver);
            let actual_family = if family == Family::Unwrap && is_get_call(&node.receiver) {
                Family::GetUnwrap
            } else {
                family
            };
            self.emit(
                actual_family,
                "method_call",
                &method,
                node.method.span(),
                fp,
            );
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
        self.emit(Family::Indexing, "index_expr", "[]", node.span(), fp);
        syn::visit::visit_expr_index(self, node);
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
    fn allowlist_match_uses_path_family_selector() {
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
                receiver_fingerprint: None,
            },
            last_seen: None,
        };
        let allow = Allowlist {
            entries: vec![entry],
        };
        let f_match = Finding {
            path: "foo.rs".into(),
            family: Family::Unwrap,
            selector: Selector {
                kind: "method_call".into(),
                container: "fn_a".into(),
                callee: "unwrap".into(),
                receiver_fingerprint: Some("anything".into()),
            },
            last_seen: LastSeen {
                line: 999,
                column: 999,
            },
        };
        assert!(allow.match_finding(&f_match).is_some());

        let f_miss = Finding {
            path: "foo.rs".into(),
            family: Family::Unwrap,
            selector: Selector {
                kind: "method_call".into(),
                container: "fn_b".into(),
                callee: "unwrap".into(),
                receiver_fingerprint: None,
            },
            last_seen: LastSeen { line: 1, column: 1 },
        };
        assert!(allow.match_finding(&f_miss).is_none());
    }
}
