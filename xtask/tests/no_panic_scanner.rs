//! Scanner-repair fixtures for #867 (defects 1–6).
//!
//! Drives the real `xtask` binary against minimal fixture workspaces so the
//! counting, scope-derivation, full-key matching, and mode-contract behavior
//! is exercised end to end — not just as unit-level selection.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const BOUNDARY: &str = r#"schema_version = "1.0"
policy = "package-boundary"
owner = "test"
status = "advisory"
updated = "2026-11-02"

[[package]]
name = "prod"
path = "runtime"
category = "published"
production_use = true

[[package]]
name = "dev"
path = "tooling"
category = "dev-only"
production_use = false
"#;

const EMPTY_ALLOWLIST: &str = "schema_version = \"0.3\"\n";

fn fixture_root(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("adze-no-panic-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("policy")).unwrap();
    std::fs::write(dir.join("Cargo.toml"), "[workspace]\n").unwrap();
    std::fs::write(dir.join("policy/package-boundary.toml"), BOUNDARY).unwrap();
    std::fs::write(dir.join("policy/no-panic-allowlist.toml"), EMPTY_ALLOWLIST).unwrap();
    dir
}

fn write_file(dir: &Path, rel: &str, contents: &str) {
    let path = dir.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, contents).unwrap();
}

fn write_allowlist(dir: &Path, body: &str) {
    std::fs::write(dir.join("policy/no-panic-allowlist.toml"), body).unwrap();
}

fn run(dir: &Path, mode: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_xtask"))
        .args(["check-no-panic-family", "--mode", mode])
        .current_dir(dir)
        .output()
        .expect("run xtask binary")
}

fn combined(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn report_json(dir: &Path) -> serde_json::Value {
    let text = std::fs::read_to_string(dir.join("target/policy/no-panic.json"))
        .expect("report must always be written");
    serde_json::from_str(&text).unwrap()
}

fn allow_entry(id: &str, container: &str, fingerprint: &str, expires: &str) -> String {
    format!(
        r#"[[allow]]
id = "{id}"
path = "runtime/src/lib.rs"
family = "unwrap"
classification = "legacy"
owner = "test"
explanation = "fixture"
expires = "{expires}"

[allow.selector]
kind = "method_call"
container = "{container}"
callee = "unwrap"
receiver_fingerprint = "{fingerprint}"
"#
    )
}

#[test]
fn advisory_reports_counts_scopes_and_all_families() {
    let dir = fixture_root("counts");
    write_file(
        &dir,
        "runtime/src/lib.rs",
        r#"#![allow(dead_code)]
use std::collections::HashMap;
pub fn shapes(m: &HashMap<String, i32>, v: &[i32], s: &str, x: Option<i32>) -> i32 {
    let a = x.unwrap();
    let b = Option::unwrap(Some(1));
    let c = v[0];
    let d = &s[1..2];
    let e = m.get("k").unwrap();
    let f = x.expect("boom");
    if v.is_empty() { panic!("empty"); }
    let g: i32 = todo!();
    let h: i32 = unimplemented!();
    let i: i32 = unreachable!();
    let _ = d;
    a + b + c + e + f + g + h + i
}
pub fn fallible(x: Option<i32>) -> Result<i32, String> { Ok(x.unwrap()) }
"#,
    );
    write_file(
        &dir,
        "runtime/tests/it.rs",
        "#[test]\nfn t() { let x: Option<i32> = None; let _ = x.unwrap(); }\n",
    );
    write_file(
        &dir,
        "tooling/src/main.rs",
        "fn main() { let x: Option<i32> = None; let _ = x.expect(\"dev\"); }\n",
    );

    let output = run(&dir, "advisory");
    let text = combined(&output);
    assert!(output.status.success(), "advisory must pass: {text}");
    for needle in [
        "selected files: 3",
        "scanned files: 3",
        "excluded files: 2",
        "failed files: 0",
        "production files: 1",
    ] {
        assert!(text.contains(needle), "missing `{needle}` in:\n{text}");
    }

    let report = report_json(&dir);
    assert_eq!(report["selected_files"], 3);
    assert_eq!(report["scanned_files"], 3);
    assert_eq!(report["excluded_files"], 2);
    assert_eq!(report["failed_files"], 0);
    assert_eq!(report["production_files"], 1);
    assert_eq!(report["test_only_findings"], 1);
    assert_eq!(report["out_of_scope_findings"], 1);

    let families: std::collections::BTreeSet<String> = report["unallowlisted"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|f| f["scope"] == "production")
        .map(|f| f["family"].as_str().unwrap().to_string())
        .collect();
    for family in [
        "unwrap",
        "unwrap_in_result",
        "expect",
        "panic_macro",
        "todo",
        "unimplemented",
        "unreachable",
        "indexing",
        "string_slice",
        "get_unwrap",
    ] {
        assert!(
            families.contains(family),
            "family {family} missing: {families:?}"
        );
    }
    let kinds: std::collections::BTreeSet<String> = report["unallowlisted"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["selector"]["kind"].as_str().unwrap().to_string())
        .collect();
    assert!(
        kinds.contains("qualified_call"),
        "qualified calls missing: {kinds:?}"
    );
}

#[test]
fn unparseable_file_fails_every_mode() {
    let dir = fixture_root("unparseable");
    write_file(
        &dir,
        "runtime/src/lib.rs",
        "pub fn f(x: Option<i32>) -> i32 { x.unwrap() }\n",
    );
    write_file(&dir, "runtime/src/broken.rs", "fn broken( {\n");

    for mode in ["advisory", "blocking-allowlist", "blocking-strict"] {
        let output = run(&dir, mode);
        let text = combined(&output);
        assert!(
            !output.status.success(),
            "{mode} must fail on parse error: {text}"
        );
        assert!(text.contains("unreadable/unparseable"), "{mode}: {text}");
    }
    let report = report_json(&dir);
    assert_eq!(report["failed_files"], 1);
    assert_eq!(report["failed_details"].as_array().unwrap().len(), 1);
}

#[test]
fn empty_production_denominator_fails() {
    let dir = fixture_root("empty-denominator");
    write_file(&dir, "tooling/src/main.rs", "fn main() {}\n");

    let output = run(&dir, "advisory");
    let text = combined(&output);
    assert!(
        !output.status.success(),
        "empty denominator must fail: {text}"
    );
    assert!(text.contains("empty production denominator"), "{text}");
    assert_eq!(report_json(&dir)["production_files"], 0);
}

#[test]
fn blocking_allowlist_enforces_full_key_and_second_call() {
    let dir = fixture_root("full-key");
    write_file(
        &dir,
        "runtime/src/lib.rs",
        "pub fn first(x: Option<i32>) -> i32 { x.unwrap() }\npub fn second(y: Option<i32>) -> i32 { y.unwrap() }\n",
    );

    // No receipts: both findings unallowlisted.
    let output = run(&dir, "blocking-allowlist");
    assert!(!output.status.success(), "must fail without receipts");
    assert_eq!(
        report_json(&dir)["unallowlisted"].as_array().unwrap().len(),
        2
    );

    // Receipt for `first` only: `second` (different container + fingerprint)
    // must NOT inherit it.
    write_allowlist(
        &dir,
        &format!(
            "schema_version = \"0.3\"\n{}",
            allow_entry("panic-0001", "first", "x", "2099-01-01")
        ),
    );
    let output = run(&dir, "blocking-allowlist");
    assert!(
        !output.status.success(),
        "second call must stay unallowlisted"
    );
    assert_eq!(
        report_json(&dir)["unallowlisted"].as_array().unwrap().len(),
        1
    );

    // Wrong fingerprint alone must not match either.
    write_allowlist(
        &dir,
        &format!(
            "schema_version = \"0.3\"\n{}",
            allow_entry("panic-0001", "first", "WRONG", "2099-01-01")
        ),
    );
    let output = run(&dir, "blocking-allowlist");
    assert!(!output.status.success(), "wrong fingerprint must not match");

    // Both receipts: green.
    write_allowlist(
        &dir,
        &format!(
            "schema_version = \"0.3\"\n{}{}",
            allow_entry("panic-0001", "first", "x", "2099-01-01"),
            allow_entry("panic-0002", "second", "y", "2099-01-01"),
        ),
    );
    let output = run(&dir, "blocking-allowlist");
    assert!(
        output.status.success(),
        "full receipts must pass: {}",
        combined(&output)
    );
}

#[test]
fn stale_and_expired_reported_but_not_enforced_in_allowlist_mode() {
    let dir = fixture_root("freshness");
    write_file(
        &dir,
        "runtime/src/lib.rs",
        "pub fn f(x: Option<i32>) -> i32 { x.unwrap() }\n",
    );
    write_allowlist(
        &dir,
        &format!(
            "schema_version = \"0.3\"\n{}{}",
            allow_entry("panic-0001", "f", "x", "2000-01-01"),
            allow_entry("panic-0009", "gone", "z", "2099-01-01")
                .replace("runtime/src/lib.rs", "runtime/src/gone.rs"),
        ),
    );

    // blocking-allowlist: reported, not enforced.
    let output = run(&dir, "blocking-allowlist");
    assert!(
        output.status.success(),
        "allowlist mode must not enforce freshness: {}",
        combined(&output)
    );
    let report = report_json(&dir);
    assert!(
        report["stale_entries"]
            .as_array()
            .unwrap()
            .iter()
            .any(|s| s == "panic-0009")
    );
    assert_eq!(report["expired_entries"].as_array().unwrap().len(), 1);

    // blocking-strict: enforced.
    let output = run(&dir, "blocking-strict");
    assert!(
        !output.status.success(),
        "strict must fail on stale+expired"
    );
}
