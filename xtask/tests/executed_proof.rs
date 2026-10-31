//! Focused falsifiers for #1058 executed proof selection.
//! Uses a tiny fixture crate built by Cargo itself; selection comes from
//! the compiled harness listing, never source scanning.

use std::path::PathBuf;
use std::sync::OnceLock;
use xtask::executed_proof::{ProofError, ProofSpec, run_executed_proof};

fn fixture() -> &'static PathBuf {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(|| {
        let dir = std::env::temp_dir().join(format!("adze-executed-proof-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::write(
            dir.join("Cargo.toml"),
            "[package]\nname = \"proof-fixture\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[features]\nspecial = []\n",
        )
        .unwrap();
        std::fs::write(
            dir.join("src/lib.rs"),
            r#"
pub fn test_helper_lookalike() {}

#[cfg(test)]
mod alpha {
    #[test]
    fn real_canary() { assert_eq!(2 + 2, 4); }

    #[test]
    #[ignore = "fixture ignored canary"]
    fn ignored_canary() {}

    #[test]
    fn failing_canary() { panic!("fixture failure"); }

    #[cfg(feature = "special")]
    #[test]
    fn cfg_gated_canary() {}
}
"#,
        )
        .unwrap();
        dir
    })
}

fn spec(command: String, out_name: &str) -> ProofSpec {
    ProofSpec {
        root: fixture().clone(),
        command,
        raw_output_path: fixture().join("target").join(out_name),
        include_ignored: false,
    }
}

fn manifest_command(tail: &str) -> String {
    format!(
        "cargo test --manifest-path {} {tail}",
        fixture().join("Cargo.toml").display()
    )
}

#[test]
fn positive_real_selection_executes_and_receipt_binds_identity() {
    let cmd = manifest_command("alpha::real_canary -- --exact");
    let receipt = run_executed_proof(&spec(cmd, "positive.txt")).expect("real canary must pass");
    assert_eq!(receipt.selected_test_ids, vec!["alpha::real_canary"]);
    assert_eq!(receipt.counts.selected, 1);
    assert_eq!(receipt.counts.executed, 1);
    assert_eq!(receipt.counts.passed, 1);
    assert_eq!(receipt.counts.failed, 0);
    assert_eq!(receipt.child_exit_code, Some(0));
    assert!(!receipt.executables.is_empty() && !receipt.executables[0].digest.is_empty());
    assert!(!receipt.toolchain.is_empty() && !receipt.source_tree.is_empty());
    let raw = std::fs::read_to_string(&receipt.raw_output_ref).unwrap();
    receipt.validate_against_output(&raw).unwrap();
    // Relabeling for a different feature tuple must not match.
    let mut other = receipt.clone();
    other.features.push("special".into());
    assert!(!receipt.matches_identity(&other));
}

#[test]
fn helper_function_resembling_test_is_not_selected() {
    let err = run_executed_proof(&spec(
        manifest_command("test_helper_lookalike -- --exact"),
        "helper.txt",
    ))
    .expect_err("helper fn is not a compiled test");
    assert!(matches!(err, ProofError::ZeroSelection(_)), "{err}");
}

#[test]
fn wrong_module_path_is_rejected_while_correct_id_runs() {
    let err = run_executed_proof(&spec(
        manifest_command("wrong_module::real_canary -- --exact"),
        "wrong.txt",
    ))
    .expect_err("wrong module must select zero");
    assert!(matches!(err, ProofError::ZeroSelection(_)), "{err}");
}

#[test]
fn nonexistent_package_wide_filter_is_rejected() {
    // No --test/--lib: package-wide filters are enumerated over every
    // compiled executable, not skipped.
    let err = run_executed_proof(&spec(
        manifest_command("audit_no_such_test_9ef921 -- --exact"),
        "package-wide.txt",
    ))
    .expect_err("nonexistent package-wide filter must be rejected");
    assert!(matches!(err, ProofError::ZeroSelection(_)), "{err}");
}

#[test]
fn cfg_disabled_target_is_zero_without_feature() {
    let err = run_executed_proof(&spec(
        manifest_command("alpha::cfg_gated_canary -- --exact"),
        "cfg.txt",
    ))
    .expect_err("cfg-disabled test must select zero");
    assert!(matches!(err, ProofError::ZeroSelection(_)), "{err}");
}

#[test]
fn manifest_selected_undeclared_feature_is_rejected() {
    let err = run_executed_proof(&spec(
        manifest_command("--features audit_missing_feature alpha::real_canary -- --exact"),
        "feature.txt",
    ))
    .expect_err("undeclared manifest feature must be rejected");
    assert!(matches!(err, ProofError::UndeclaredFeature(_)), "{err}");
}

#[test]
fn ignored_only_selection_is_rejected() {
    let err = run_executed_proof(&spec(
        manifest_command("alpha::ignored_canary -- --exact"),
        "ignored.txt",
    ))
    .expect_err("ignored-only must not count as executed proof");
    assert!(matches!(err, ProofError::IgnoredOnly(_)), "{err}");
}

#[test]
fn failing_child_is_not_hidden_by_successful_wrapper() {
    let err = run_executed_proof(&spec(
        manifest_command("alpha::failing_canary -- --exact"),
        "failing.txt",
    ))
    .expect_err("failing child must fail proof");
    assert!(matches!(err, ProofError::ChildFailed { .. }), "{err}");

    // A receipt claiming exit 0 while raw output records a failure is
    // rejected on re-validation (the wrapper-success case).
    let ok = run_executed_proof(&spec(
        manifest_command("alpha::real_canary -- --exact"),
        "wrapper.txt",
    ))
    .unwrap();
    let tampered_raw = "running 1 test\ntest result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out\n";
    assert!(ok.validate_against_output(tampered_raw).is_err());
    // Truncated output (no terminal row) also cannot pass.
    assert!(ok.validate_against_output("running 1 test\n").is_err());
}

#[test]
fn bin_flag_is_rejected_even_when_lib_test_would_pass() {
    // Probe regression: `--bin` used to be silently dropped, so a command
    // Cargo would have scoped to a binary was "proven" as a lib test.
    let err = run_executed_proof(&spec(
        manifest_command("--bin proof-fixture --lib alpha::real_canary -- --exact"),
        "bin-flag.txt",
    ))
    .expect_err("--bin must be rejected, not dropped");
    assert!(matches!(err, ProofError::UnsupportedFlag(_)), "{err}");
    assert!(err.to_string().contains("--bin"), "{err}");
}

#[test]
fn unsupported_cargo_flags_are_rejected_not_dropped() {
    for flag in [
        "--bins",
        "--tests",
        "--examples",
        "--example demo",
        "--benches",
        "--bench demo",
        "--all-targets",
        "--release",
        "--all-features",
        "--no-default-features",
        "--workspace",
        "--offline",
        "--bin=proof-fixture",
        "--unknown-flag",
        "--unknown-flag=value",
        "-q",
    ] {
        let err = run_executed_proof(&spec(
            manifest_command(&format!("{flag} alpha::real_canary -- --exact")),
            "unsupported-flag.txt",
        ))
        .expect_err(&format!("{flag} must be rejected, not dropped"));
        assert!(
            matches!(err, ProofError::UnsupportedFlag(_)),
            "{flag}: {err}"
        );
    }
    // Unknown harness flags after `--` are likewise not dropped.
    let err = run_executed_proof(&spec(
        manifest_command("alpha::real_canary -- --ignored"),
        "unsupported-harness-flag.txt",
    ))
    .expect_err("--ignored harness flag must be rejected, not dropped");
    assert!(matches!(err, ProofError::UnsupportedFlag(_)), "{err}");
}

#[test]
fn all_features_is_rejected_so_cfg_selection_is_explicit() {
    // `--all-features` used to be dropped: the cfg-gated test was then
    // silently absent (ZeroSelection) or the receipt understated features.
    // Now it must fail explicitly; callers use `--features special`.
    let err = run_executed_proof(&spec(
        manifest_command("--all-features alpha::cfg_gated_canary -- --exact"),
        "all-features.txt",
    ))
    .expect_err("--all-features must be rejected, not dropped");
    assert!(matches!(err, ProofError::UnsupportedFlag(_)), "{err}");
    assert!(err.to_string().contains("--all-features"), "{err}");
}

#[test]
fn cfg_gated_test_runs_with_explicit_feature() {
    let receipt = run_executed_proof(&spec(
        manifest_command("--features special alpha::cfg_gated_canary -- --exact"),
        "cfg-enabled.txt",
    ))
    .expect("cfg-gated canary must run with --features special");
    assert_eq!(receipt.selected_test_ids, vec!["alpha::cfg_gated_canary"]);
    assert_eq!(receipt.counts.selected, 1);
    assert_eq!(receipt.counts.executed, 1);
    assert_eq!(receipt.counts.passed, 1);
    assert_eq!(receipt.features, vec!["special"]);
}

#[test]
fn tampered_receipt_selection_does_not_validate() {
    let receipt = run_executed_proof(&spec(
        manifest_command("alpha::real_canary -- --exact"),
        "tampered-receipt.txt",
    ))
    .unwrap();
    let raw = std::fs::read_to_string(&receipt.raw_output_ref).unwrap();
    receipt.validate_against_output(&raw).unwrap();

    // IDs cleared while counts still claim a selection.
    let mut cleared = receipt.clone();
    cleared.selected_test_ids.clear();
    let err = cleared
        .validate_against_output(&raw)
        .expect_err("cleared selected_test_ids must fail");
    assert!(matches!(err, ProofError::InconsistentReceipt(_)), "{err}");

    // Inflated selected count not backed by IDs.
    let mut inflated = receipt.clone();
    inflated.counts.selected = 999;
    let err = inflated
        .validate_against_output(&raw)
        .expect_err("selected=999 without IDs must fail");
    assert!(matches!(err, ProofError::InconsistentReceipt(_)), "{err}");

    // Executed + ignored accounting for more tests than were selected.
    let mut over_accounted = receipt.clone();
    over_accounted.counts.ignored = 1;
    let err = over_accounted
        .validate_against_output(&raw)
        .expect_err("executed+ignored > selected must fail");
    assert!(matches!(err, ProofError::InconsistentReceipt(_)), "{err}");
}

#[test]
fn missing_target_produces_explicit_non_success() {
    let result = run_executed_proof(&spec(
        manifest_command("--test no_such_target alpha::real_canary -- --exact"),
        "missing-target.txt",
    ));
    assert!(
        result.is_err(),
        "deleted/missing target must not succeed: {result:?}"
    );
}
