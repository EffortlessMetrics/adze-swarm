//! Explicit downstream grammar-edit journey. Never edits the checked-in sample.

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

const BASE_GRAMMAR: &str = include_str!("../src/lib.rs");
const BUILD_SCRIPT: &str = include_str!("../build.rs");
const FIELD_ANCHOR: &str = "        pub timeout: Option<Timeout>,";
const TYPE_ANCHOR: &str = "    #[adze::extra]";
const BACKOFF_TYPE: &str = r#"    /// Optional delay between retries; absence and zero are distinct.
    #[derive(Debug, PartialEq)]
    pub struct Backoff {
        #[adze::leaf(text = "backoff")]
        _keyword: (),
        #[adze::leaf(text = "=")]
        _equals: (),
        #[adze::leaf(pattern = r"[0-9]+", try_transform = |text| text.parse::<u16>())]
        pub seconds: u16,
        #[adze::leaf(text = ";")]
        _end: (),
    }

"#;

const BASELINE_TEST: &str = r#"use edit_debug_probe::grammar;

#[test]
fn journey_contract() {
    let old = grammar::parse("retries=3;targets=[api,worker];timeout=30;").expect("old input");
    assert_eq!(old.retries, 3);
    assert_eq!(old.targets, vec!["api", "worker"]);
    assert_eq!(old.timeout.expect("timeout").seconds, 30);
    assert!(grammar::parse("retries=3;targets=[api];backoff=7;").is_err());
}
"#;

const EXTENSION_TEST: &str = r#"use adze::errors::ParseErrorReason;
use edit_debug_probe::grammar;

#[test]
fn journey_contract() {
    for value in [0, 7, u16::MAX] {
        let source = format!("retries=3;targets=[api,worker];backoff={value};");
        let policy = grammar::parse(&source).expect("extended input");
        assert_eq!(policy.retries, 3);
        assert_eq!(policy.targets, vec!["api", "worker"]);
        assert_eq!(policy.timeout, None);
        assert_eq!(policy.backoff.expect("explicit backoff").seconds, value);
    }
    let old = grammar::parse("retries=3;targets=[api,worker];timeout=30;").expect("old input still works");
    assert_eq!(old.retries, 3);
    assert_eq!(old.targets, vec!["api", "worker"]);
    assert_eq!(old.timeout.expect("old timeout").seconds, 30);
    assert_eq!(old.backoff, None);
    let source = "retries=3;targets=[api];backoff=65536;";
    let errors = grammar::parse(source).expect_err("overflow is not success");
    let error = errors.first().expect("conversion diagnostic");
    assert!(matches!(error.reason, ParseErrorReason::ConversionFailed(_)));
    let start = source.find("65536").expect("token");
    assert_eq!(error.byte_span(), start..start + 5);
}
"#;

fn extended_grammar(base: &str) -> String {
    assert_eq!(
        base.matches(FIELD_ANCHOR).count(),
        1,
        "sample field anchor drifted"
    );
    assert!(base.contains(TYPE_ANCHOR), "sample type anchor drifted");
    base.replacen(
        FIELD_ANCHOR,
        &format!("{FIELD_ANCHOR}\n        pub backoff: Option<Backoff>,"),
        1,
    )
    .replacen(TYPE_ANCHOR, &format!("{BACKOFF_TYPE}{TYPE_ANCHOR}"), 1)
}

fn toml_path(path: &Path) -> String {
    path.to_str()
        .expect("fixture dependency path must be UTF-8")
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
}

fn run_phase(project: &Path) -> Output {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    Command::new(cargo)
        .args([
            "test",
            "--test",
            "journey_contract",
            "--",
            "--exact",
            "journey_contract",
            "--test-threads=1",
        ])
        .current_dir(project)
        // Cleaning this directory between phases would hide stale generation.
        .env("CARGO_TARGET_DIR", project.join("target"))
        .output()
        .expect("execute downstream Cargo; a missing tool is not passing proof")
}

fn assert_one_passed(phase: &str, output: &Output) {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "{phase}: {}\n{stdout}\n{stderr}",
        output.status
    );
    let terminal_rows: Vec<_> = stdout
        .lines()
        .filter(|line| line.starts_with("test result:"))
        .collect();
    assert_eq!(
        terminal_rows.len(),
        1,
        "{phase}: expected one terminal row\n{stdout}"
    );
    assert!(
        terminal_rows[0].starts_with("test result: ok. 1 passed; 0 failed; 0 ignored;"),
        "{phase}: the intended test must actually execute\n{stdout}",
    );
    assert!(
        stdout.lines().any(|line| line == "test journey_contract ... ok"),
        "{phase}: missing named test\n{stdout}"
    );
}

#[test]
fn walkthrough_edit_has_unique_anchors() {
    let extended = extended_grammar(BASE_GRAMMAR);
    assert_eq!(extended.matches("pub backoff: Option<Backoff>").count(), 1);
    assert_eq!(extended.matches("pub struct Backoff").count(), 1);
    assert!(extended.contains(FIELD_ANCHOR));
    assert!(extended.contains("pub struct Timeout"));
}

#[test]
#[ignore = "explicit downstream Cargo journey; run with --ignored --exact grammar_edit_break_repair"]
fn grammar_edit_break_repair() {
    let sample = Path::new(env!("CARGO_MANIFEST_DIR"));
    let repo = sample
        .parent()
        .and_then(Path::parent)
        .expect("repository root");
    let original_source = fs::read(sample.join("src/lib.rs")).expect("read original source");
    let original_build = fs::read(sample.join("build.rs")).expect("read original build script");
    let temp = tempfile::tempdir().expect("owned downstream workspace");
    let project = temp.path();
    fs::create_dir_all(project.join("src")).expect("source directory");
    fs::create_dir_all(project.join("tests")).expect("tests directory");
    fs::write(project.join("preserve-me.txt"), b"unrelated user file\n").expect("sentinel");
    fs::write(project.join("build.rs"), BUILD_SCRIPT).expect("copy actual build script");
    fs::write(
        project.join("Cargo.toml"),
        format!(
            r#"[package]
name = "edit-debug-probe"
version = "0.0.0"
edition = "2024"
publish = false

[workspace]

[dependencies]
adze = {{ path = "{}", default-features = false }}

[build-dependencies]
adze-tool = {{ path = "{}" }}

[features]
default = ["pure-rust"]
pure-rust = ["adze/pure-rust"]
"#,
            toml_path(&repo.join("runtime")),
            toml_path(&repo.join("tool"))
        ),
    )
    .expect("manifest");
    fs::write(project.join("src/lib.rs"), BASE_GRAMMAR).expect("baseline grammar");
    fs::write(project.join("tests/journey_contract.rs"), BASELINE_TEST).expect("baseline oracle");
    assert_one_passed("baseline", &run_phase(project));

    let extended = extended_grammar(BASE_GRAMMAR);
    fs::write(project.join("src/lib.rs"), &extended).expect("edit grammar");
    fs::write(project.join("tests/journey_contract.rs"), EXTENSION_TEST).expect("extension oracle");
    assert_one_passed("extension", &run_phase(project));

    // Break the new grammar field, not a copied data fixture. Either grammar
    // generation or rustc may diagnose the missing type; both must name it.
    let broken = extended.replacen(
        "pub backoff: Option<Backoff>",
        "pub backoff: Option<MissingBackoffType1138>",
        1,
    );
    fs::write(project.join("src/lib.rs"), broken).expect("break user source");
    let failure = run_phase(project);
    let stderr = String::from_utf8_lossy(&failure.stderr);
    assert!(!failure.status.success(), "broken source reused stale success");
    assert!(
        stderr.contains("MissingBackoffType1138"),
        "failure must be the deliberate missing type, not network/tool setup: {stderr}"
    );
    assert!(!String::from_utf8_lossy(&failure.stdout).contains("test result: ok."));

    fs::write(project.join("src/lib.rs"), extended).expect("repair source");
    assert_one_passed("repair", &run_phase(project));
    assert_eq!(
        fs::read(project.join("preserve-me.txt"))
            .expect("sentinel preserved")
            .as_slice(),
        b"unrelated user file\n"
    );
    assert_eq!(
        fs::read(sample.join("src/lib.rs")).expect("original source intact"),
        original_source
    );
    assert_eq!(
        fs::read(sample.join("build.rs")).expect("original build intact"),
        original_build
    );
}
