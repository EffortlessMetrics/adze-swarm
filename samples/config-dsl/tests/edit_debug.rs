//! Explicit downstream grammar-edit journey. Never edits the checked-in sample.

use std::fs;
use std::path::Path;
use std::process::Command;

const BASE_GRAMMAR: &str = include_str!("../src/lib.rs");
const BUILD_SCRIPT: &str = include_str!("../build.rs");
#[path = "support/edit_debug_contract.rs"]
mod edit_debug_contract;

use edit_debug_contract::{FIELD_ANCHOR, Phase, extended_grammar};

fn toml_path(path: &Path) -> String {
    path.to_str()
        .expect("fixture dependency path must be UTF-8")
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
}

fn run_phase(project: &Path, phase: Phase) {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let output = Command::new(cargo)
        .args(Phase::cargo_args(false))
        .current_dir(project)
        // Cleaning this directory between phases would hide stale generation.
        .env("CARGO_TARGET_DIR", project.join("target"))
        .output()
        .expect("execute downstream Cargo; a missing tool is not passing proof");
    if let Err(error) = phase.audit_output(output.status.code(), &output.stdout, &output.stderr) {
        panic!(
            "{error}\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
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
    let owned_target = project.join("target");
    fs::create_dir(&owned_target).expect("owned target");
    let target_sentinel = owned_target.join("keep-target.txt");
    fs::write(&target_sentinel, b"same target across phases\n").expect("target sentinel");
    let manifest = fs::read(project.join("Cargo.toml")).expect("prepared manifest");
    let mut lock = None;
    for phase in Phase::ORDERED {
        let (grammar, oracle) = phase.inputs(BASE_GRAMMAR);
        fs::write(project.join("src/lib.rs"), grammar).expect("phase grammar");
        fs::write(project.join("tests/journey_contract.rs"), oracle).expect("phase oracle");
        run_phase(project, phase);
        let current_lock = fs::read(project.join("Cargo.lock")).expect("resolved project lock");
        assert_eq!(
            lock.get_or_insert_with(|| current_lock.clone()),
            &current_lock
        );
        assert_eq!(
            fs::read(project.join("Cargo.toml")).expect("manifest preserved"),
            manifest
        );
        assert_eq!(
            fs::read(project.join("build.rs")).expect("build script preserved"),
            BUILD_SCRIPT.as_bytes()
        );
        assert_eq!(
            fs::read(&target_sentinel).expect("target was not cleared"),
            b"same target across phases\n"
        );
    }
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
