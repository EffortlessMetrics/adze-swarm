//! Compiled enumeration for the 5 smallest package-wide proof-inventory
//! rows (issue #1058 follow-up, advisory scope).
//!
//! The static validator in `support_tier_proof_commands.rs` marks every
//! package-wide row (no `--test`/`--lib` scope) as
//! `UNSUPPORTED[package-wide]` because a source scan cannot prove selection.
//! This test closes the gap for the cheapest subset with the real Cargo
//! harness: for each enumerated row it runs
//! `cargo test <same args> --no-run`, collects the built test executables
//! from `--message-format=json`, runs each executable with the ledger filter
//! plus `--list`, and asserts at least one test is selected in total.
//!
//! Scope is exactly [`ENUMERATED_ROWS`]. Do not extend this list without
//! ledger-lane coordination: the pinned quarantine counts in
//! `support_tier_proof_commands.rs` still count these rows as quarantined
//! until the ledger lane repairs them.
//!
//! Enumeration alone is not execution (#1058): a listed test may still be
//! ignored-only or failing. This test proves nonzero selection only.

use std::path::{Path, PathBuf};
use std::process::Command;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask lives directly under the workspace root")
        .to_path_buf()
}

/// Exact ledger commands (`docs/status/SUPPORT_TIERS.md`), smallest packages
/// first. Byte-for-byte ledger strings; the ledger lane owns repairing these
/// rows from `UNSUPPORTED[package-wide]` to enumerated.
const ENUMERATED_ROWS: &[&str] = &[
    "cargo test -p downstream-demo -- --nocapture",
    "cargo test -p adze-python test_python_language_exists -- --exact --nocapture",
    "cargo test --manifest-path example/Cargo.toml external_word_example::tests::generated_external_grammar_bad_input_matrix_returns_diagnostic_document --features pure-rust -- --exact --nocapture",
    "cargo test -p adze-golden-tests javascript_canary_expression_golden --features javascript-grammar -- --nocapture",
    "cargo test -p adze-cli test_parse_help_documents_available_modes -- --exact --nocapture",
];

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

/// Split a ledger `cargo test ...` command into cargo args (before `--`) and
/// harness args (after `--`). The leading `cargo test` is dropped.
fn split_command(command: &str) -> (Vec<String>, Vec<String>) {
    let tokens = tokenize(command);
    assert!(
        tokens.len() >= 2 && tokens[0] == "cargo" && tokens[1] == "test",
        "not a cargo test command: {command}"
    );
    let mut cargo_args = Vec::new();
    let mut harness_args = Vec::new();
    let mut seen_dashdash = false;
    for token in tokens.into_iter().skip(2) {
        if token == "--" {
            seen_dashdash = true;
            continue;
        }
        if seen_dashdash {
            harness_args.push(token);
        } else {
            cargo_args.push(token);
        }
    }
    (cargo_args, harness_args)
}

/// Positional test filters are the cargo args that are not flags and not
/// flag values. Package-wide rows carry at most one.
fn positional_filters(cargo_args: &[String]) -> Result<Vec<String>, String> {
    const VALUE_FLAGS: &[&str] = &[
        "-p",
        "--package",
        "--features",
        "--manifest-path",
        "--profile",
    ];
    let mut filters = Vec::new();
    let mut skip_next = false;
    for arg in cargo_args {
        if skip_next {
            skip_next = false;
            continue;
        }
        if arg == "--test" || arg == "--lib" {
            return Err(format!(
                "enumerated rows must be package-wide (no --test/--lib scope): {arg}"
            ));
        }
        if VALUE_FLAGS.iter().any(|flag| flag == arg) {
            skip_next = true;
            continue;
        }
        if arg.starts_with('-') {
            return Err(format!("unsupported cargo flag in enumerated row: {arg}"));
        }
        filters.push(arg.clone());
    }
    if filters.len() > 1 {
        return Err(format!(
            "cargo accepts one positional test filter, found {}: {}",
            filters.len(),
            filters.join(" ")
        ));
    }
    Ok(filters)
}

/// Build the row's test executables without running them, then list the
/// selected tests in each. Returns the total number of listed tests.
fn enumerate_row(root: &Path, command: &str) -> Result<usize, String> {
    let (cargo_args, harness_args) = split_command(command);
    let filters = positional_filters(&cargo_args)?;

    let mut build = Command::new("cargo");
    build
        .current_dir(root)
        .arg("test")
        .args(&cargo_args)
        .arg("--no-run")
        .arg("--message-format=json");
    let build_out = build
        .output()
        .map_err(|e| format!("failed to spawn cargo: {e}"))?;
    if !build_out.status.success() {
        let stderr = String::from_utf8_lossy(&build_out.stderr);
        let tail: Vec<&str> = stderr.lines().rev().take(15).collect();
        return Err(format!(
            "cargo test --no-run failed for {command}:\n{}",
            tail.into_iter().rev().collect::<Vec<_>>().join("\n")
        ));
    }
    let stdout = String::from_utf8_lossy(&build_out.stdout);
    let mut executables: Vec<PathBuf> = Vec::new();
    for line in stdout.lines() {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if value.get("reason").and_then(|reason| reason.as_str()) != Some("compiler-artifact") {
            continue;
        }
        let is_test = value
            .get("profile")
            .and_then(|profile| profile.get("test"))
            .and_then(|test| test.as_bool())
            .unwrap_or(false);
        if !is_test {
            continue;
        }
        if let Some(executable) = value.get("executable").and_then(|exe| exe.as_str()) {
            let path = PathBuf::from(executable);
            if !executables.contains(&path) {
                executables.push(path);
            }
        }
    }
    if executables.is_empty() {
        return Err(format!(
            "cargo test --no-run built no test executables for {command}"
        ));
    }

    let mut list_args = filters;
    for harness in &harness_args {
        if harness == "--nocapture" {
            continue;
        }
        list_args.push(harness.clone());
    }
    list_args.push("--list".to_string());

    let mut total = 0usize;
    for executable in &executables {
        let list_out = Command::new(executable)
            .args(&list_args)
            .output()
            .map_err(|e| format!("failed to run {}: {e}", executable.display()))?;
        if !list_out.status.success() {
            return Err(format!(
                "{} --list failed for {command}:\n{}",
                executable.display(),
                String::from_utf8_lossy(&list_out.stderr)
            ));
        }
        let listed = String::from_utf8_lossy(&list_out.stdout)
            .lines()
            .filter(|line| line.trim_end().ends_with(": test"))
            .count();
        println!(
            "  {}: {listed} listed test(s) [{}]",
            executable.display(),
            list_args.join(" ")
        );
        total += listed;
    }
    if total == 0 {
        return Err(format!(
            "filter selects no test in any built executable for {command} [{}]",
            list_args.join(" ")
        ));
    }
    Ok(total)
}

#[test]
fn test_package_wide_rows_enumerate_at_least_one_test() {
    let root = workspace_root();
    assert_eq!(
        ENUMERATED_ROWS.len(),
        5,
        "advisory scope is exactly the 5 smallest package-wide rows"
    );
    for command in ENUMERATED_ROWS {
        match enumerate_row(&root, command) {
            Ok(total) => println!("ENUMERATED: {command} -> {total} listed test(s)"),
            Err(error) => panic!("enumeration failed for {command}: {error}"),
        }
    }
}

#[test]
fn test_enumeration_rejects_unknown_package() {
    // A bogus package fails at resolve time, before any compilation, so this
    // stays cheap while pinning the failure path.
    let root = workspace_root();
    let error = enumerate_row(
        &root,
        "cargo test -p adze-no-such-package-xyz -- --nocapture",
    )
    .expect_err("a bogus package must fail enumeration, not select zero tests");
    assert!(
        error.contains("adze-no-such-package-xyz") || error.contains("no test executables"),
        "unexpected error shape: {error}"
    );
}
