//! Durable receipt writer/reader proofs (#1058 fix item 5).
//!
//! Exercises `xtask::proof` against a tiny fixture crate built by Cargo
//! itself (never the workspace build): a positive run retains a rereadable
//! receipt, non-execution/failure retains no success receipt, and a raw log
//! substituted from an unrelated success fails re-validation.

use std::path::PathBuf;
use std::sync::OnceLock;
use xtask::proof::{RAW_FILE_NAME, RECEIPT_FILE_NAME, STABLE_CANARY_COMMAND, run_command};

fn fixture() -> &'static PathBuf {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(|| {
        let dir = std::env::temp_dir().join(format!("adze-proof-receipt-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::write(
            dir.join("Cargo.toml"),
            "[package]\nname = \"proof-receipt-fixture\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        )
        .unwrap();
        std::fs::write(
            dir.join("src/lib.rs"),
            r#"
#[cfg(test)]
mod alpha {
    #[test]
    fn real_canary() { assert_eq!(2 + 2, 4); }

    #[test]
    fn other_canary() { assert_eq!(3 + 3, 6); }

    #[test]
    #[ignore = "intentionally ignored"]
    fn ignored_canary() {}

    #[test]
    fn failing_canary() { panic!("intentional fixture failure"); }
}
"#,
        )
        .unwrap();
        dir
    })
}

fn command(tail: &str) -> String {
    format!(
        "cargo test --manifest-path {} {tail}",
        fixture().join("Cargo.toml").display()
    )
}

fn out_dir(name: &str) -> PathBuf {
    let dir = fixture().join("receipts").join(name);
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn run_fixture(out_name: &str, tail: &str) -> Result<PathBuf, String> {
    run_command(fixture(), &command(tail), &out_dir(out_name), None).map_err(|e| e.to_string())
}

#[test]
fn writer_retains_rereadable_receipt_for_fixture_canary() {
    let out = out_dir("positive");
    let receipt_path =
        run_fixture("positive", "alpha::real_canary -- --exact").expect("canary must pass");
    assert_eq!(receipt_path, out.join(RECEIPT_FILE_NAME));
    assert!(receipt_path.is_file(), "receipt JSON missing");
    assert!(out.join(RAW_FILE_NAME).is_file(), "raw log missing");

    let receipt = xtask::proof::read_receipt(&receipt_path).expect("receipt must parse");
    assert_eq!(receipt.selected_test_ids, vec!["alpha::real_canary"]);
    assert_eq!(receipt.counts.selected, 1);
    assert_eq!(receipt.counts.executed, 1);
    assert_eq!(receipt.counts.passed, 1);
    assert_eq!(receipt.counts.failed, 0);
    assert_eq!(receipt.child_exit_code, Some(0));
    xtask::proof::validate_receipt_file(&receipt_path).expect("disk receipt must validate");
}

#[test]
fn writer_honors_ledger_path_for_receipt_json() {
    let out = out_dir("ledger");
    let ledger = out.join("nested").join("ledger.json");
    let written = run_command(
        fixture(),
        &command("alpha::real_canary -- --exact"),
        &out,
        Some(ledger.as_path()),
    )
    .expect("canary must pass");
    assert_eq!(written, ledger);
    assert!(ledger.is_file(), "ledger JSON missing");
    assert!(out.join(RAW_FILE_NAME).is_file(), "raw log missing");
    xtask::proof::validate_receipt_file(&ledger).expect("ledger receipt must validate");
}

#[test]
fn zero_selection_fixture_fails_without_receipt() {
    // Negative control per the #995/#1058 contract: a filter matching no
    // compiled test must fail, never retain a vacuous receipt.
    let out = out_dir("zero");
    let err = run_fixture("zero", "audit_no_such_test_9ef921 -- --exact")
        .expect_err("nonexistent filter must fail");
    assert!(
        err.contains("zero tests"),
        "expected ZeroSelection, got: {err}"
    );
    assert!(
        !out.join(RECEIPT_FILE_NAME).is_file(),
        "zero-selection must not retain a receipt"
    );
}

#[test]
fn ignored_only_fixture_fails_without_receipt() {
    let out = out_dir("ignored");
    let err = run_fixture("ignored", "alpha::ignored_canary -- --exact")
        .expect_err("ignored-only selection must fail");
    assert!(err.contains("ignored-only selection"), "{err}");
    assert!(!out.join(RECEIPT_FILE_NAME).exists());
}

#[test]
fn failed_fixture_preserves_raw_log_but_not_success_receipt() {
    let out = out_dir("failed");
    let err = run_fixture("failed", "alpha::failing_canary -- --exact")
        .expect_err("failing child must fail");
    assert!(err.contains("test child failed"), "{err}");
    assert!(!out.join(RECEIPT_FILE_NAME).exists());
    let raw = std::fs::read_to_string(out.join(RAW_FILE_NAME)).unwrap();
    assert!(raw.contains("alpha::failing_canary"), "{raw}");
    assert!(raw.contains("test result: FAILED."), "{raw}");
}

#[test]
fn same_count_log_from_another_real_canary_fails_revalidation() {
    let receipt_path =
        run_fixture("original-log", "alpha::real_canary -- --exact").expect("canary must pass");
    let other_path =
        run_fixture("other-log", "alpha::other_canary -- --exact").expect("other canary must pass");
    let receipt = xtask::proof::read_receipt(&receipt_path).unwrap();
    let other = xtask::proof::read_receipt(&other_path).unwrap();
    assert_eq!(receipt.counts, other.counts);
    assert_eq!(receipt.counts.passed, 1);
    assert_ne!(receipt.selected_test_ids, other.selected_test_ids);

    std::fs::copy(&other.raw_output_ref, &receipt.raw_output_ref).unwrap();
    let err = xtask::proof::validate_receipt_file(&receipt_path)
        .expect_err("a passing raw log from the other selection must not validate");
    assert!(
        err.to_string().contains("raw output SHA-256 mismatch"),
        "{err}"
    );
}

#[test]
fn legacy_receipt_without_raw_binding_is_rejected() {
    let receipt_path =
        run_fixture("legacy", "alpha::real_canary -- --exact").expect("canary must pass");
    let mut json: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&receipt_path).unwrap()).unwrap();
    assert!(
        json.as_object_mut()
            .unwrap()
            .remove("raw_output_sha256")
            .is_some()
    );
    std::fs::write(&receipt_path, serde_json::to_vec(&json).unwrap()).unwrap();
    let err = xtask::proof::validate_receipt_file(&receipt_path)
        .expect_err("a count-only legacy receipt must be regenerated");
    assert!(format!("{err:#}").contains("missing field `raw_output_sha256`"));
}

#[test]
fn tampered_raw_log_fails_revalidation() {
    let receipt_path =
        run_fixture("tampered", "alpha::real_canary -- --exact").expect("canary must pass");
    let receipt = xtask::proof::read_receipt(&receipt_path).expect("receipt must parse");
    let raw_path = PathBuf::from(&receipt.raw_output_ref);
    std::fs::write(
        &raw_path,
        "running 1 test\ntest result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out\n",
    )
    .unwrap();
    let err = xtask::proof::validate_receipt_file(&receipt_path)
        .expect_err("tampered raw output must fail");
    assert!(
        err.to_string().contains("invalid"),
        "expected invalid receipt, got: {err}"
    );
}

#[test]
fn stable_canary_command_is_runner_supported() {
    // The default canary must survive the runner's strict flag parsing:
    // `--test` + one filter + `--exact` only, and the `ts-compat` feature the
    // file-level cfg requires (omitting it compiles to zero tests).
    assert!(
        STABLE_CANARY_COMMAND.contains("--test projection_equivalence"),
        "{STABLE_CANARY_COMMAND}"
    );
    assert!(
        STABLE_CANARY_COMMAND.contains("--exact"),
        "{STABLE_CANARY_COMMAND}"
    );
    assert!(
        !STABLE_CANARY_COMMAND.contains("--nocapture"),
        "runner rejects harness flags besides --exact"
    );
    for feature in ["pure-rust", "glr", "ts-compat", "serialization"] {
        assert!(
            STABLE_CANARY_COMMAND.contains(feature),
            "missing feature {feature}: {STABLE_CANARY_COMMAND}"
        );
    }
}
