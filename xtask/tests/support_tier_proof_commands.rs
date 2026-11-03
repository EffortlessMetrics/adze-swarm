//! Mechanical validation of the `cargo test` proof commands advertised in
//! `docs/status/SUPPORT_TIERS.md` (issue #998).
//!
//! Every documented command must be syntactically valid for Cargo (at most
//! one positional test filter), use features the package actually declares,
//! and select at least one real test (an `--exact` name must exist; a prefix
//! filter must match at least one test function). Only `#[test]`-attributed
//! functions count: plain helpers never satisfy a filter, exact multi-segment
//! paths require every leading segment to be a declared module (no leaf
//! reduction), and crate-level `#![cfg(...)]` gates are evaluated against the
//! selected features instead of assuming the target is enabled (#1058).
//!
//! Rows this static check cannot prove are marked `UNSUPPORTED[...]` with an
//! explicit reason and counted, never passed vacuously: package-wide rows
//! without `--test`/`--lib` need compiled `cargo test --no-run` + `-- --list`
//! enumeration, and flags the executed runner (`xtask/src/executed_proof.rs`)
//! rejects are rejected here via the shared `xtask::cargo_flags` table.
//! `UNSUPPORTED` is a quarantine, not proof. Historical enumeration and a
//! matching command string do not establish selection on the current source.
//! Package-wide rows remain quarantined here; actual compiled selection and
//! execution belong to the executed proof runner, not this source screen.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use xtask::cargo_flags as flag_table;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask lives directly under the workspace root")
        .to_path_buf()
}

/// Package name -> workspace-relative directory for packages the support
/// ledger is allowed to cite. An unmapped package fails validation so the
/// inventory stays explicit.
fn package_dirs() -> BTreeMap<&'static str, &'static str> {
    BTreeMap::from([
        ("adze", "runtime"),
        ("adze-benchmarks", "benchmarks"),
        ("adze-cli", "cli"),
        ("adze-common", "common"),
        ("adze-glr-core", "glr-core"),
        ("adze-golden-tests", "golden-tests"),
        ("adze-ir", "ir"),
        ("adze-macro", "macro"),
        ("adze-python", "grammars/python"),
        ("adze-tablegen", "tablegen"),
        ("adze-tool", "tool"),
        ("downstream-demo", "samples/downstream-demo"),
        ("xtask", "xtask"),
    ])
}

fn extract_cargo_test_commands(markdown: &str) -> Vec<String> {
    let mut commands = Vec::new();
    for span in markdown.split('`') {
        let span = span.trim();
        if span.starts_with("cargo test") {
            commands.push(span.to_string());
        }
    }
    commands
}

/// Split on whitespace, keeping double-quoted sections together.
fn tokenize(command: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    for ch in command.chars() {
        match ch {
            '"' => in_quotes = !in_quotes,
            ch if ch.is_whitespace() && !in_quotes => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
            }
            ch => current.push(ch),
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

#[derive(Debug, Default)]
struct ParsedCommand {
    package: Option<String>,
    manifest_path: Option<String>,
    features: Vec<String>,
    profile: Option<String>,
    test_target: Option<String>,
    lib: bool,
    filters: Vec<String>,
    exact: bool,
}

/// Flag table imported from `xtask::cargo_flags` (single source for this
/// validator; agreement with `xtask/src/executed_proof.rs` is pinned by
/// `test_shared_flag_table_agrees_with_executed_runner`): only
/// `-p/--package`, `--manifest-path`, `--features`, `--test`, `--lib`
/// and `--profile` select a supported compiled set. `--doc` needs an
/// explicit doctest adapter; every other target/profile selector or unknown
/// `-`/`--*` token is rejected, never silently dropped. After `--`, only
/// `--exact` and the display-only `--nocapture` are screened here. Every
/// other argument is quarantined: ignored/skip/list options and extra filters
/// must not silently change selection or replace execution with enumeration.
/// This static screen is not a compiled-selection or execution receipt.
fn parse_command(command: &str) -> Result<ParsedCommand, String> {
    let tokens = tokenize(command);
    let mut parsed = ParsedCommand::default();
    let mut iter = tokens.iter().skip(2).peekable();
    while let Some(token) = iter.next() {
        match token.as_str() {
            "--" => {
                for rest in iter.by_ref() {
                    match rest.as_str() {
                        "--exact" => parsed.exact = true,
                        "--nocapture" => {}
                        _ => {
                            return Err(format!(
                                "UNSUPPORTED[flag]: unmodeled test-harness argument {rest}; \
                                only --exact and --nocapture are screened statically"
                            ));
                        }
                    }
                }
                break;
            }
            "-p" | "--package" => {
                parsed.package = Some(
                    iter.next()
                        .ok_or("missing value for -p")?
                        .trim_matches('"')
                        .to_string(),
                );
            }
            "--features" => {
                let value = iter.next().ok_or("missing value for --features")?;
                parsed
                    .features
                    .extend(value.split(',').map(|f| f.trim().to_string()));
            }
            "--test" => {
                parsed.test_target = Some(
                    iter.next()
                        .ok_or("missing value for --test")?
                        .trim_matches('"')
                        .to_string(),
                );
            }
            "--manifest-path" => {
                parsed.manifest_path = Some(
                    iter.next()
                        .ok_or("missing value for --manifest-path")?
                        .trim_matches('"')
                        .to_string(),
                );
            }
            "--profile" => {
                parsed.profile = Some(
                    iter.next()
                        .ok_or("missing value for --profile")?
                        .trim_matches('"')
                        .to_string(),
                );
            }
            "--lib" => parsed.lib = true,
            "--doc" => {
                return Err(
                    "UNSUPPORTED[harness]: doctests have no libtest executable; \
                    explicit adapter or the executed runner required"
                        .to_string(),
                );
            }
            token if flag_table::is_rejected_flag(token) => {
                return Err(format!(
                    "UNSUPPORTED[flag]: {token} changes the selected build/test set \
                    (supported: {})",
                    flag_table::SUPPORTED_SUMMARY
                ));
            }
            token if token.starts_with('-') => {
                return Err(format!(
                    "UNSUPPORTED[flag]: unknown cargo test flag {token} (supported: {})",
                    flag_table::SUPPORTED_SUMMARY
                ));
            }
            token => parsed.filters.push(token.trim_matches('"').to_string()),
        }
    }
    if parsed.filters.len() > 1 {
        return Err(format!(
            "cargo accepts one positional test filter, found {}: {}",
            parsed.filters.len(),
            parsed.filters.join(" ")
        ));
    }
    if parsed.exact && parsed.filters.is_empty() {
        return Err("--exact given without a test name filter".to_string());
    }
    Ok(parsed)
}

fn declared_features(package_dir: &Path) -> Vec<String> {
    let manifest = std::fs::read_to_string(package_dir.join("Cargo.toml"))
        .unwrap_or_else(|e| panic!("read {}: {e}", package_dir.join("Cargo.toml").display()));
    let mut features = Vec::new();
    let mut in_features = false;
    for line in manifest.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_features = line == "[features]";
            continue;
        }
        if in_features && let Some((name, _)) = line.split_once('=') {
            features.push(name.trim().to_string());
        }
    }
    features
}

/// Only `#[test]`-attributed functions are collected: plain helpers and
/// fixtures never count (#1058). `mod` lines neither arm nor disarm, so an
/// attribute above a module item applies to the functions inside it.
fn collect_test_fn_names(source: &str, names: &mut Vec<String>) {
    fn push(rest: &str, names: &mut Vec<String>) {
        let name: String = rest
            .chars()
            .take_while(|ch| ch.is_alphanumeric() || *ch == '_')
            .collect();
        if !name.is_empty() {
            names.push(name);
        }
    }
    fn is_test_attr(t: &str) -> bool {
        let inner = t.trim_start_matches("#[").trim_end_matches(']');
        inner == "test" || inner.starts_with("test(") || inner.ends_with("::test")
    }
    let mut armed = false;
    for line in source.lines() {
        let t = line.trim();
        if t.starts_with("#[") {
            if is_test_attr(t) {
                if let Some(idx) = t.find("fn ") {
                    push(&t[idx + 3..], names);
                } else {
                    armed = true;
                }
            }
            continue;
        }
        if let Some(rest) = t.strip_prefix("fn ") {
            if armed {
                push(rest, names);
            }
            armed = false;
            continue;
        }
        if t.starts_with('}')
            || [
                "mod ", "struct ", "enum ", "union ", "impl", "trait ", "type ", "const ",
                "static ", "use ", "extern ", "macro", "pub ",
            ]
            .iter()
            .any(|kw| t.starts_with(kw))
        {
            armed = false;
        }
    }
}

/// Declared `mod <name>` items in one source blob. Single-line and
/// self-contained, so concatenating files before scanning is safe.
fn collect_mod_names(source: &str, mods: &mut BTreeSet<String>) {
    for line in source.lines() {
        let mut t = line.trim();
        if let Some(rest) = t.strip_prefix("pub") {
            t = rest.trim_start();
            if let Some(inner) = t.strip_prefix('(') {
                match inner.find(')') {
                    Some(end) => t = inner[end + 1..].trim_start(),
                    None => continue,
                }
            }
        }
        if let Some(rest) = t.strip_prefix("mod ") {
            let name: String = rest
                .chars()
                .take_while(|ch| ch.is_alphanumeric() || *ch == '_')
                .collect();
            if !name.is_empty() {
                mods.insert(name);
            }
        }
    }
}

fn test_fn_names_in(dir: &Path) -> Vec<String> {
    let mut names = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(path) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&path) else {
            continue;
        };
        for entry in entries.flatten() {
            let entry_path = entry.path();
            if entry_path.is_dir() {
                stack.push(entry_path);
            } else if entry_path.extension().is_some_and(|ext| ext == "rs")
                && let Ok(source) = std::fs::read_to_string(&entry_path)
            {
                collect_test_fn_names(&source, &mut names);
            }
        }
    }
    names
}

/// Whether a Cargo test filter selects at least one known test.
/// Non-exact filters match substrings of the full test path. `--exact`
/// filters match the full path: a bare name must equal a `#[test]` function,
/// and a multi-segment path additionally requires every leading segment to
/// be a declared module. Leaf reduction is rejected: `wrong::real_test` never
/// matches `real_test` or `mod real_test` (#1058).
fn filter_matches(filter: &str, exact: bool, fn_names: &[String], mods: &BTreeSet<String>) -> bool {
    if exact {
        match filter.rsplit_once("::") {
            None => fn_names.iter().any(|name| name == filter),
            Some((path, leaf)) => {
                fn_names.iter().any(|name| name == leaf)
                    && path.split("::").all(|seg| mods.contains(seg))
            }
        }
    } else {
        let leaf = filter.rsplit("::").next().unwrap_or(filter);
        fn_names.iter().any(|name| name.contains(leaf))
            || (filter.contains("::") && filter.split("::").all(|seg| mods.contains(seg)))
    }
}

fn collect_source_text(dir: &Path, out: &mut String) {
    let mut stack = vec![dir.to_path_buf()];
    while let Some(path) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&path) else {
            continue;
        };
        for entry in entries.flatten() {
            let entry_path = entry.path();
            if entry_path.is_dir() {
                stack.push(entry_path);
            } else if entry_path.extension().is_some_and(|ext| ext == "rs")
                && let Ok(source) = std::fs::read_to_string(&entry_path)
            {
                out.push_str(&source);
                out.push('\n');
            }
        }
    }
}

/// Crate-level `#![cfg(...)]` gates of a test-target root file. Only plain
/// `#![cfg(EXPR)]` inners are joined; `#![cfg_attr(..)]` and `#![allow(..)]`
/// are skipped (the former needs the real cfg evaluator).
fn target_cfg_gates(source: &str) -> Vec<String> {
    let mut gates = Vec::new();
    let mut rest = source;
    while let Some(idx) = rest.find("#![cfg") {
        let after = &rest[idx + "#![cfg".len()..];
        if !after.trim_start().starts_with('(') {
            rest = after;
            continue;
        }
        let mut depth = 0usize;
        let mut end = None;
        for (i, byte) in after.bytes().enumerate() {
            if byte == b'(' {
                depth += 1;
            } else if byte == b')' {
                if depth == 0 {
                    break;
                }
                depth -= 1;
                if depth == 0 {
                    end = Some(i + 1);
                    break;
                }
            }
        }
        match end {
            Some(len) => {
                gates.push(after[..len].trim().to_string());
                rest = &after[len..];
            }
            None => break,
        }
    }
    gates
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CfgVerdict {
    Yes,
    No,
    Unknown,
}

fn cfg_tokens(expr: &str) -> Vec<String> {
    let mut toks = Vec::new();
    let mut cur = String::new();
    let mut chars = expr.chars();
    while let Some(ch) = chars.next() {
        match ch {
            '"' => {
                if !cur.is_empty() {
                    toks.push(std::mem::take(&mut cur));
                }
                let mut lit = String::from("\"");
                for c in chars.by_ref() {
                    lit.push(c);
                    if c == '"' {
                        break;
                    }
                }
                toks.push(lit);
            }
            '(' | ')' | ',' | '=' => {
                if !cur.is_empty() {
                    toks.push(std::mem::take(&mut cur));
                }
                toks.push(ch.to_string());
            }
            c if c.is_whitespace() => {
                if !cur.is_empty() {
                    toks.push(std::mem::take(&mut cur));
                }
            }
            c => cur.push(c),
        }
    }
    if !cur.is_empty() {
        toks.push(cur);
    }
    toks
}

/// Evaluate a `#![cfg(EXPR)]` gate against the selected features. Only
/// positive feature conjunctions (`all(test, feature = "a", ...)` shape)
/// plus `not(feature = "x")` are decided statically: `test` holds under
/// `cargo test`, `feature = "x"` follows the feature set, and a negated
/// feature holds when absent. Anything else (`any(...)`, platform or custom
/// predicates, complex nesting) is Unknown — the compiled runner decides.
/// Unknown is never claimed as selected.
fn eval_cfg_gate(gate: &str, features: &[String]) -> CfgVerdict {
    use CfgVerdict::{No, Unknown, Yes};
    let mut expr = gate.trim();
    expr = expr.strip_prefix('(').unwrap_or(expr);
    expr = expr.strip_suffix(')').unwrap_or(expr);
    let toks = cfg_tokens(expr);
    let mut required: Vec<(String, bool)> = Vec::new();
    let mut i = 0;
    while i < toks.len() {
        match toks[i].as_str() {
            "all" | "(" | ")" | "," | "test" | "true" => i += 1,
            "false" => return No,
            "feature" => {
                let lit = toks.get(i + 2).cloned().unwrap_or_default();
                if toks.get(i + 1).is_some_and(|t| t == "=") && lit.starts_with('"') {
                    required.push((lit.trim_matches('"').to_string(), false));
                    i += 3;
                } else {
                    return Unknown;
                }
            }
            "not" => {
                let lit = toks.get(i + 4).cloned().unwrap_or_default();
                if toks.get(i + 1).is_some_and(|t| t == "(")
                    && toks.get(i + 2).is_some_and(|t| t == "feature")
                    && toks.get(i + 3).is_some_and(|t| t == "=")
                    && lit.starts_with('"')
                    && toks.get(i + 5).is_some_and(|t| t == ")")
                {
                    required.push((lit.trim_matches('"').to_string(), true));
                    i += 6;
                } else {
                    return Unknown;
                }
            }
            _ => return Unknown,
        }
    }
    for (name, negated) in &required {
        let present = features.iter().any(|f| f == name);
        if present == *negated {
            return No;
        }
    }
    Yes
}

fn validate_command(root: &Path, command: &str) -> Result<(), String> {
    let parsed = parse_command(command)?;

    let package_dir = if let Some(manifest_path) = &parsed.manifest_path {
        root.join(manifest_path)
            .parent()
            .expect("manifest path has a parent")
            .to_path_buf()
    } else if let Some(package) = &parsed.package {
        let dirs = package_dirs();
        let rel = dirs
            .get(package.as_str())
            .ok_or_else(|| format!("package {package} is not in the validated inventory"))?;
        root.join(rel)
    } else {
        root.to_path_buf()
    };

    // Feature validation applies to --manifest-path commands too (#1058):
    // a manifest-selected undeclared feature must not slip through merely
    // because no -p package was named.
    if parsed.package.is_some() || parsed.manifest_path.is_some() {
        let declared = declared_features(&package_dir);
        for feature in &parsed.features {
            let base = feature.split('/').next().unwrap_or(feature);
            if !feature.contains('/') && !declared.iter().any(|f| f == base) {
                return Err(format!(
                    "package at {} does not declare feature {feature}",
                    package_dir.display()
                ));
            }
        }
    }

    if let Some(target) = &parsed.test_target {
        let target_file = package_dir.join("tests").join(format!("{target}.rs"));
        if !target_file.is_file() {
            return Err(format!(
                "test target {target} not found at {}",
                target_file.display()
            ));
        }
        let source = std::fs::read_to_string(&target_file)
            .map_err(|e| format!("read {}: {e}", target_file.display()))?;
        // A file-level `#![cfg(...)]` gate can compile the target to zero
        // tests; never claim such a target as selected (#1058).
        for gate in target_cfg_gates(&source) {
            match eval_cfg_gate(&gate, &parsed.features) {
                CfgVerdict::Yes => {}
                CfgVerdict::No => {
                    return Err(format!(
                        "UNSUPPORTED[cfg]: test target {target} is cfg-disabled under features [{}] (gate {gate}); compiled enumeration required",
                        parsed.features.join(",")
                    ));
                }
                CfgVerdict::Unknown => {
                    return Err(format!(
                        "UNSUPPORTED[cfg]: test target {target} has a cfg gate this static check cannot evaluate ({gate}); compiled enumeration required"
                    ));
                }
            }
        }
        // Test names live in the target root file plus its same-stem
        // module directory (tests/<target>/**), when present.
        let mut names = Vec::new();
        collect_test_fn_names(&source, &mut names);
        let mut mods = BTreeSet::new();
        collect_mod_names(&source, &mut mods);
        let module_dir = target_file.with_extension("");
        if module_dir.is_dir() {
            names.extend(test_fn_names_in(&module_dir));
            let mut extra = String::new();
            collect_source_text(&module_dir, &mut extra);
            collect_mod_names(&extra, &mut mods);
        }
        if let Some(filter) = parsed.filters.first() {
            if !filter_matches(filter, parsed.exact, &names, &mods) {
                return Err(format!(
                    "filter {filter} selects no test in target {target}"
                ));
            }
        } else if names.is_empty() {
            return Err(format!(
                "filter selects no test in target {target}: no #[test] functions found"
            ));
        }
    } else if parsed.lib {
        let src_dir = package_dir.join("src");
        if let Some(filter) = parsed.filters.first() {
            let names = test_fn_names_in(&src_dir);
            let mut all_source = String::new();
            collect_source_text(&src_dir, &mut all_source);
            let mut mods = BTreeSet::new();
            collect_mod_names(&all_source, &mut mods);
            if !filter_matches(filter, parsed.exact, &names, &mods) {
                return Err(format!("filter {filter} selects no lib test"));
            }
        } else {
            return Err("UNSUPPORTED[package-wide]: --lib without a filter selects the whole library; \
                compiled `cargo test --no-run` + `-- --list` enumeration with >=1 selected test required"
                .to_string());
        }
    } else {
        // Package-wide rows (no --test/--lib scope): a bare filter or no
        // filter at all proves nothing statically (#1058 item 1). Mark
        // Unsupported — explicit and counted — instead of passing vacuously.
        // A historical enumeration or matching command string cannot prove
        // the selected methods still exist on this candidate.
        return Err(
            "UNSUPPORTED[package-wide]: package-wide selection without --test/--lib scope; \
            compiled `cargo test --no-run` + `-- --list` enumeration with >=1 selected test required"
                .to_string(),
        );
    }

    Ok(())
}

/// Historical false-accept inputs retained only as regression fixtures.
/// This is not an admission list or a copy of the live enumeration inventory.
/// No validation branch consults these strings to authorize selection.
const FORMER_ALLOWLIST_ROWS: &[&str] = &[
    "cargo test -p downstream-demo -- --nocapture",
    "cargo test -p adze-python test_python_language_exists -- --exact --nocapture",
    "cargo test --manifest-path example/Cargo.toml external_word_example::tests::generated_external_grammar_bad_input_matrix_returns_diagnostic_document --features pure-rust -- --exact --nocapture",
    "cargo test -p adze-golden-tests javascript_canary_expression_golden --features javascript-grammar -- --nocapture",
    "cargo test -p adze-cli test_parse_help_documents_available_modes -- --exact --nocapture",
];

/// Pinned quarantine counts (#1058 items 1-2). Rows the static check cannot
/// prove are explicit and counted here instead of passing vacuously. If the
/// ledger changes, update these counts deliberately — a mismatch fails
/// loudly with the full row list.
const EXPECTED_UNSUPPORTED: &[(&str, usize)] = &[
    ("package-wide", 39),
    ("flag", 4),
    ("harness", 1),
    ("cfg", 0),
];

fn unsupported_kind(error: &str) -> Option<&str> {
    error.strip_prefix("UNSUPPORTED[")?.split(']').next()
}

#[test]
fn test_support_tier_cargo_commands_are_valid_and_select_tests() {
    let root = workspace_root();
    let markdown = std::fs::read_to_string(root.join("docs/status/SUPPORT_TIERS.md"))
        .expect("read SUPPORT_TIERS.md");

    let commands = extract_cargo_test_commands(&markdown);
    assert!(
        commands.len() > 20,
        "expected a substantial proof inventory, found {}",
        commands.len()
    );

    let mut failures = Vec::new();
    let mut unsupported: Vec<String> = Vec::new();
    for command in &commands {
        if let Err(error) = validate_command(&root, command) {
            if unsupported_kind(&error).is_some() {
                unsupported.push(format!("{command}\n  -> {error}"));
            } else {
                failures.push(format!("{command}\n  -> {error}"));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "invalid support-tier proof commands:\n{}",
        failures.join("\n")
    );
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for row in &unsupported {
        println!("QUARANTINED: {row}");
        let kind = row
            .split("UNSUPPORTED[")
            .nth(1)
            .and_then(|rest| rest.split(']').next())
            .unwrap_or("other");
        *counts.entry(kind).or_insert(0) += 1;
    }
    let count = |kind: &str| counts.get(kind).copied().unwrap_or(0);
    println!(
        "inventory: {} commands, {} supported, {} unsupported (package-wide={}, flag={}, harness={}, cfg={})",
        commands.len(),
        commands.len() - unsupported.len() - failures.len(),
        unsupported.len(),
        count("package-wide"),
        count("flag"),
        count("harness"),
        count("cfg"),
    );
    for (kind, expected) in EXPECTED_UNSUPPORTED {
        assert_eq!(
            count(kind),
            *expected,
            "{kind} quarantine changed; update the count deliberately:\n{}",
            unsupported.join("\n")
        );
    }
}

#[test]
fn test_validator_rejects_multiple_positional_filters() {
    let root = workspace_root();
    let error = validate_command(
        &root,
        "cargo test -p adze-glr-core conflict ambiguity -- --nocapture",
    )
    .expect_err("two positional filters must be rejected");
    assert!(error.contains("one positional test filter"), "{error}");
}

#[test]
fn test_validator_rejects_unknown_exact_test_name() {
    let root = workspace_root();
    let error = validate_command(
        &root,
        "cargo test -p adze --features \"pure-rust,glr\" --test generated_parse_errors no_such_test_exists -- --exact --nocapture",
    )
    .expect_err("a nonexistent exact test must be rejected");
    assert!(error.contains("selects no test"), "{error}");
}

#[test]
fn test_validator_rejects_undeclared_feature() {
    let root = workspace_root();
    let error = validate_command(
        &root,
        "cargo test -p adze --features \"pure-rust,not-a-feature\" --test generated_parse_errors -- --nocapture",
    )
    .expect_err("an undeclared feature must be rejected");
    assert!(error.contains("does not declare feature"), "{error}");
}

#[test]
fn test_validator_rejects_manifest_path_undeclared_feature() {
    let root = workspace_root();
    let error = validate_command(
        &root,
        "cargo test --manifest-path runtime/Cargo.toml --features audit_missing_feature --test projection_equivalence",
    )
    .expect_err("manifest-selected undeclared feature must be rejected (#1058)");
    assert!(error.contains("does not declare feature"), "{error}");
}

#[test]
fn test_validator_rejects_missing_test_target() {
    let root = workspace_root();
    let error = validate_command(
        &root,
        "cargo test -p adze --test no_such_target -- --nocapture",
    )
    .expect_err("a missing test target must be rejected");
    assert!(error.contains("not found"), "{error}");
}

#[test]
fn test_validator_rejects_unsupported_cargo_flags() {
    // Flag table mirrored from executed_proof.rs UnsupportedFlag, plus the
    // doctest harness (#1058 item 2). Each entry names its quarantine tag.
    let root = workspace_root();
    for (flag, tag) in [
        ("--bins", "flag"),
        ("--tests", "flag"),
        ("--examples", "flag"),
        ("--example", "flag"),
        ("--benches", "flag"),
        ("--bench", "flag"),
        ("--all-targets", "flag"),
        ("--release", "flag"),
        ("--all-features", "flag"),
        ("--no-default-features", "flag"),
        ("--workspace", "flag"),
        ("--audit-bogus-flag", "flag"),
        ("--doc", "harness"),
    ] {
        let mut command = format!("cargo test -p adze-glr-core {flag}");
        if tag == "harness" {
            command.push_str(" --features serialization");
        }
        let error = validate_command(&root, &command)
            .expect_err(&format!("{flag} must be rejected, not silently accepted"));
        assert!(
            error.starts_with(&format!("UNSUPPORTED[{tag}]")),
            "{flag}: {error}"
        );
    }
}

#[test]
fn test_validator_marks_package_wide_selection_unsupported() {
    // No --test/--lib scope: requires compiled enumeration (#1058 item 1).
    let root = workspace_root();
    for command in [
        "cargo test -p adze-glr-core conflict -- --nocapture",
        "cargo test -p adze-cli test_init_generates_buildable_project -- --exact --nocapture",
        "cargo test -p adze --features external_scanners",
    ] {
        let error = validate_command(&root, command)
            .expect_err("package-wide rows must not pass vacuously");
        assert!(
            error.starts_with("UNSUPPORTED[package-wide]"),
            "{command}: {error}"
        );
    }
}

/// Scratch fixture tree so falsifier tests do not depend on the ledger.
fn fixture_tree(name: &str, features: &str, files: &[(&str, &str)]) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("adze-validator-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create fixture root");
    let manifest = format!(
        "[package]\nname = \"fixture-pkg\"\nversion = \"0.0.0\"\nedition = \"2021\"\n\n[features]\n{features}\n"
    );
    std::fs::write(dir.join("Cargo.toml"), manifest).expect("write fixture manifest");
    for (rel, contents) in files {
        let path = dir.join(rel);
        std::fs::create_dir_all(path.parent().expect("fixture file has a parent"))
            .expect("create fixture dir");
        std::fs::write(&path, contents).expect("write fixture file");
    }
    dir
}

#[test]
fn test_validator_rejects_helpers_and_wrong_module_paths() {
    // Helpers without #[test] never count; exact multi-segment paths need
    // every leading segment declared as a module (#1058 item 3).
    let root = fixture_tree(
        "helpers",
        "",
        &[(
            "tests/picked.rs",
            "fn looks_like_a_test_but_is_helper() {}\n\
             #[test]\n\
             fn real_test_in_target() {}\n\
             mod inner {\n\
                 #[test]\n\
                 fn nested_real_test() {}\n\
             }\n",
        )],
    );
    let manifest = "cargo test --manifest-path Cargo.toml --test picked";
    let error = validate_command(
        &root,
        &format!("{manifest} looks_like_a_test_but_is_helper -- --exact"),
    )
    .expect_err("a helper without #[test] must not be selected");
    assert!(error.contains("selects no test"), "{error}");
    validate_command(&root, &format!("{manifest} real_test_in_target -- --exact"))
        .expect("exact #[test] name must be selected");
    let error = validate_command(
        &root,
        &format!("{manifest} wrong_module::nested_real_test -- --exact"),
    )
    .expect_err("wrong_module::existing_test --exact must be rejected");
    assert!(error.contains("selects no test"), "{error}");
    validate_command(
        &root,
        &format!("{manifest} inner::nested_real_test -- --exact"),
    )
    .expect("correct full module path must be selected");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn test_validator_marks_cfg_disabled_targets_unsupported() {
    // A crate-level cfg gate the features do not satisfy compiles to zero
    // tests; the static check must not claim it as selected (#1058 item 3).
    let root = fixture_tree(
        "cfg",
        "needed = []\n",
        &[
            (
                "tests/gated.rs",
                "#![cfg(feature = \"needed\")]\n\
                 #[test]\n\
                 fn gated_test() {}\n",
            ),
            (
                "tests/plat.rs",
                "#![cfg(unix)]\n\
                 #[test]\n\
                 fn plat_test() {}\n",
            ),
        ],
    );
    validate_command(
        &root,
        "cargo test --manifest-path Cargo.toml --features needed --test gated",
    )
    .expect("satisfied cfg gate must be selected");
    let error = validate_command(&root, "cargo test --manifest-path Cargo.toml --test gated")
        .expect_err("cfg-disabled target must not be claimed as selected");
    assert!(error.starts_with("UNSUPPORTED[cfg]"), "{error}");
    let error = validate_command(&root, "cargo test --manifest-path Cargo.toml --test plat")
        .expect_err("platform gate needs the real runner, not a static claim");
    assert!(error.starts_with("UNSUPPORTED[cfg]"), "{error}");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn test_historical_enumeration_does_not_lift_package_wide_quarantine() {
    let root = workspace_root();
    let markdown = std::fs::read_to_string(root.join("docs/status/SUPPORT_TIERS.md"))
        .expect("read SUPPORT_TIERS.md");
    assert_eq!(FORMER_ALLOWLIST_ROWS.len(), 5);
    for command in FORMER_ALLOWLIST_ROWS {
        assert!(
            markdown.contains(*command),
            "missing regression row: {command}"
        );
        let error = validate_command(&root, command)
            .expect_err("a command string is not current-candidate compiled proof");
        assert!(
            error.starts_with("UNSUPPORTED[package-wide]"),
            "{command}: {error}"
        );
    }
}

#[test]
fn test_unchanged_package_command_stays_quarantined_when_source_changes() {
    // Preserve the exact formerly allowlisted command while the source moves
    // through present, cfg-disabled and removed test states. This checks the
    // static refusal boundary; it does not claim to compile these fixtures.
    let root = fixture_tree(
        "package-source-drift",
        "",
        &[(
            "samples/downstream-demo/Cargo.toml",
            "[package]\nname = \"downstream-demo\"\nversion = \"0.0.0\"\n",
        )],
    );
    let source_path = root.join("samples/downstream-demo/src/lib.rs");
    std::fs::create_dir_all(source_path.parent().expect("source parent"))
        .expect("create source directory");
    let command = "cargo test -p downstream-demo -- --nocapture";
    for source in [
        "#[test]\nfn previously_listed() {}\n",
        "#[cfg(any())]\n#[test]\nfn previously_listed() {}\n",
        "// The previously listed test has been removed.\n",
    ] {
        std::fs::write(&source_path, source).expect("write changed source");
        let error = validate_command(&root, command)
            .expect_err("unchanged argv must not authorize changed source");
        assert!(error.starts_with("UNSUPPORTED[package-wide]"), "{error}");
    }
    std::fs::remove_dir_all(&root).expect("remove owned fixture");
}

#[test]
fn test_validator_rejects_unmodeled_harness_arguments() {
    // These probes stop during parsing, before any source read or child run.
    for suffix in [
        "--ignored",
        "--include-ignored",
        "--skip selected",
        "--skip=selected",
        "--list",
        "--bench",
        "--test",
        "--exclude-should-panic",
        "--format json",
        "--format=json",
        "--test-threads=1",
        "--audit-bogus-harness-flag",
        "another_filter",
    ] {
        let command = format!(
            "cargo test -p xtask --test picked selected -- --exact --nocapture {suffix}"
        );
        let error = parse_command(&command)
            .expect_err("unmodeled harness arguments must never be silently dropped");
        assert!(error.starts_with("UNSUPPORTED[flag]"), "{suffix}: {error}");
        assert!(error.contains("test-harness argument"), "{suffix}: {error}");
    }
}

#[test]
fn test_validator_preserves_supported_harness_arguments() {
    for (suffix, exact) in [
        ("--exact --nocapture", true),
        ("--nocapture --exact", true),
        ("--exact", true),
        ("--nocapture", false),
    ] {
        let parsed = parse_command(&format!(
            "cargo test -p xtask --test picked selected -- {suffix}"
        ))
        .expect("supported static harness options must remain accepted");
        assert_eq!(parsed.exact, exact);
        assert_eq!(parsed.filters, vec!["selected".to_string()]);
        assert_eq!(parsed.test_target.as_deref(), Some("picked"));
    }
    let error = parse_command("cargo test -p xtask --test picked -- --exact --nocapture")
        .expect_err("--exact still requires a named filter");
    assert!(error.contains("without a test name filter"), "{error}");
}

#[test]
fn test_shared_flag_table_agrees_with_executed_runner() {
    // Pin the shared `xtask::cargo_flags` table against the executed
    // runner's own inline table in `xtask/src/executed_proof.rs` (read-only
    // for this slice). Probing runs the runner's parse only: a rejected flag
    // fails parse before any Cargo invocation, and `--doc` terminates parse
    // for accepted flags, so every probe is hermetic (no builds, no reads).
    use xtask::executed_proof::{ProofSpec, run_executed_proof};
    fn probe_error(command: &str) -> String {
        let spec = ProofSpec {
            root: PathBuf::from("probe-root-unused-by-parse"),
            command: command.to_string(),
            raw_output_path: PathBuf::from("probe-output-unused-by-parse"),
            include_ignored: false,
        };
        match run_executed_proof(&spec) {
            Ok(_) => panic!("probe unexpectedly proved: {command}"),
            Err(error) => error.to_string(),
        }
    }
    // Every rejected-by-name flag must be UnsupportedFlag in the runner too.
    for flag in flag_table::REJECTED_TARGET_SELECTORS
        .iter()
        .chain(flag_table::REJECTED_PROFILE_SHORTCUTS)
    {
        assert!(flag_table::is_rejected_flag(flag), "{flag}");
        assert!(!flag_table::is_supported_flag(flag), "{flag}");
        let error = probe_error(&format!("cargo test -p adze-glr-core {flag}"));
        assert!(
            error.starts_with("unsupported cargo test flag:"),
            "{flag}: {error}"
        );
    }
    // Unknown flags: rejected by both, never silently dropped.
    for flag in ["--workspace", "--audit-bogus-flag"] {
        assert!(!flag_table::is_supported_flag(flag), "{flag}");
        assert!(!flag_table::is_rejected_flag(flag), "{flag}");
        let error = probe_error(&format!("cargo test -p adze-glr-core {flag}"));
        assert!(
            error.starts_with("unsupported cargo test flag:"),
            "{flag}: {error}"
        );
    }
    // Every supported flag must survive the runner's parse: appending `--doc`
    // makes parse terminate with UnsupportedHarness instead, proving the flag
    // itself was accepted.
    let supported: Vec<String> = flag_table::SUPPORTED_VALUE_FLAGS
        .iter()
        .map(|flag| format!("{flag} probe-value"))
        .chain(
            flag_table::SUPPORTED_BOOL_FLAGS
                .iter()
                .map(|flag| (*flag).to_string()),
        )
        .collect();
    assert_eq!(
        supported.len(),
        7,
        "supported table shape changed; review the probes"
    );
    for flag in &supported {
        let bare = flag.split_whitespace().next().expect("flag has a name");
        assert!(flag_table::is_supported_flag(bare), "{flag}");
        let error = probe_error(&format!("cargo test -p adze-glr-core {flag} --doc"));
        assert!(
            error.starts_with("unsupported test harness:"),
            "{flag}: {error}"
        );
    }
    // Harness-only flag and summary string stay aligned with the runner.
    assert_eq!(flag_table::HARNESS_ONLY_FLAG, "--doc");
    let error = probe_error("cargo test -p adze-glr-core --doc");
    assert!(error.starts_with("unsupported test harness:"), "{error}");
    assert!(
        probe_error("cargo test -p adze-glr-core --bins").contains(flag_table::SUPPORTED_SUMMARY),
        "SUPPORTED_SUMMARY drifted from the runner message"
    );
}
