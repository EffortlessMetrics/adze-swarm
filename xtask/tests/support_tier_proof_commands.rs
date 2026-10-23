//! Mechanical validation of the `cargo test` proof commands advertised in
//! `docs/status/SUPPORT_TIERS.md` (issue #998).
//!
//! Every documented command must be syntactically valid for Cargo (at most
//! one positional test filter), reference an existing package and test
//! target, use features the package actually declares, and select at least
//! one real test (an `--exact` name must exist; a prefix filter must match
//! at least one test function). This is a static selection check: it does
//! not execute the suites.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

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
    test_target: Option<String>,
    lib: bool,
    filters: Vec<String>,
    exact: bool,
}

fn parse_command(command: &str) -> Result<ParsedCommand, String> {
    let tokens = tokenize(command);
    let mut parsed = ParsedCommand::default();
    let mut iter = tokens.iter().skip(2).peekable();
    while let Some(token) = iter.next() {
        match token.as_str() {
            "--" => {
                for rest in iter.by_ref() {
                    if rest == "--exact" {
                        parsed.exact = true;
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
            "--lib" => parsed.lib = true,
            token if token.starts_with('-') => {
                // Flags we do not model (e.g. --tests, --bins, --doc,
                // --offline, --profile <v>, --workspace) are accepted as-is;
                // --profile consumes a value.
                if token == "--profile" {
                    iter.next();
                }
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

fn collect_test_fn_names(source: &str, names: &mut Vec<String>) {
    for line in source.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("fn ") {
            let name: String = rest
                .chars()
                .take_while(|ch| ch.is_alphanumeric() || *ch == '_')
                .collect();
            if !name.is_empty() {
                names.push(name);
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
///
/// Cargo matches non-exact filters as substrings of the full test path and
/// `--exact` filters against the full path; statically we approximate the
/// path with the leaf function name plus module declarations, which is
/// exact for the flat test layouts this inventory uses.
fn filter_matches(filter: &str, exact: bool, fn_names: &[String], source: &str) -> bool {
    let leaf = filter.rsplit("::").next().unwrap_or(filter);
    if exact {
        fn_names.iter().any(|name| name == leaf)
            || (filter.contains("::") && source.contains(&format!("mod {leaf}")))
    } else {
        fn_names.iter().any(|name| name.contains(leaf))
            || (filter.contains("::") && source.contains(&format!("mod {leaf}")))
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

    if let Some(package) = &parsed.package {
        let declared = declared_features(&package_dir);
        for feature in &parsed.features {
            if !declared.iter().any(|f| f == feature) {
                return Err(format!(
                    "package {package} does not declare feature {feature}"
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
        if let Some(filter) = parsed.filters.first() {
            // Test names live in the target root file plus its same-stem
            // module directory (tests/<target>/**), when present.
            let mut names = Vec::new();
            let source = std::fs::read_to_string(&target_file)
                .map_err(|e| format!("read {}: {e}", target_file.display()))?;
            collect_test_fn_names(&source, &mut names);
            let module_dir = target_file.with_extension("");
            if module_dir.is_dir() {
                names.extend(test_fn_names_in(&module_dir));
            }
            if !filter_matches(filter, parsed.exact, &names, &source) {
                return Err(format!(
                    "filter {filter} selects no test in target {target}"
                ));
            }
        }
    } else if parsed.lib
        && let Some(filter) = parsed.filters.first()
    {
        let src_dir = package_dir.join("src");
        let names = test_fn_names_in(&src_dir);
        let mut all_source = String::new();
        collect_source_text(&src_dir, &mut all_source);
        if !filter_matches(filter, parsed.exact, &names, &all_source) {
            return Err(format!("filter {filter} selects no lib test"));
        }
    }

    Ok(())
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
    for command in &commands {
        if let Err(error) = validate_command(&root, command) {
            failures.push(format!("{command}\n  -> {error}"));
        }
    }
    assert!(
        failures.is_empty(),
        "invalid support-tier proof commands:\n{}",
        failures.join("\n")
    );
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
fn test_validator_rejects_missing_test_target() {
    let root = workspace_root();
    let error = validate_command(
        &root,
        "cargo test -p adze --test no_such_target -- --nocapture",
    )
    .expect_err("a missing test target must be rejected");
    assert!(error.contains("not found"), "{error}");
}
