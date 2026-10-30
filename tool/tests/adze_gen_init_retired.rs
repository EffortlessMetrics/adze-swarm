//! Regression tests for #1080: `adze-gen init` is retired so it can never
//! overwrite user files or escape the selected destination.

use std::fs;
use std::process::Command;

fn run_init(cwd: &std::path::Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_adze-gen"))
        .arg("init")
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("run adze-gen init")
}

fn assert_retired(output: &std::process::Output) {
    assert!(
        !output.status.success(),
        "retired init must fail, stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        combined.contains("adze init"),
        "error must direct users to canonical `adze init`: {combined}"
    );
    assert!(
        !combined.contains("Created grammar project"),
        "retired init must not claim success: {combined}"
    );
}

#[test]
fn test_adze_gen_init_rerun_preserves_existing_project_files() {
    let dir = tempfile::tempdir().unwrap();
    let project = dir.path().join("tree-sitter-calc");
    fs::create_dir_all(project.join("src")).unwrap();
    let files = [
        (project.join("grammar.js"), "SENTINEL-GRAMMAR"),
        (project.join("Cargo.toml"), "SENTINEL-CARGO"),
        (project.join("build.rs"), "SENTINEL-BUILD"),
        (project.join("src/lib.rs"), "SENTINEL-LIB"),
    ];
    for (path, sentinel) in &files {
        fs::write(path, sentinel).unwrap();
    }
    let output = run_init(dir.path(), &["Calc"]);
    assert_retired(&output);
    for (path, sentinel) in &files {
        assert_eq!(fs::read_to_string(path).unwrap(), *sentinel);
    }
}

#[test]
fn test_adze_gen_init_in_place_preserves_existing_files() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("grammar.js"), "SENTINEL-GRAMMAR").unwrap();
    fs::write(dir.path().join("Cargo.toml"), "SENTINEL-CARGO").unwrap();
    let output = run_init(dir.path(), &["Calc", "--in-place"]);
    assert_retired(&output);
    assert_eq!(
        fs::read_to_string(dir.path().join("grammar.js")).unwrap(),
        "SENTINEL-GRAMMAR"
    );
    assert_eq!(
        fs::read_to_string(dir.path().join("Cargo.toml")).unwrap(),
        "SENTINEL-CARGO"
    );
    assert!(!dir.path().join("build.rs").exists());
    assert!(!dir.path().join("src").exists());
}

#[test]
fn test_adze_gen_init_rejects_path_escape_without_writes() {
    let dir = tempfile::tempdir().unwrap();
    let caller = dir.path().join("caller");
    fs::create_dir_all(&caller).unwrap();
    for name in [
        "../../../escaped",
        "..\\escaped",
        "/absolute",
        "a/b",
        "1bad",
        "x';evil",
    ] {
        let output = run_init(&caller, &[name]);
        assert_retired(&output);
    }
    assert!(!dir.path().join("escaped").exists());
    assert!(fs::read_dir(&caller).unwrap().next().is_none());
}

#[test]
fn test_adze_gen_init_help_directs_to_canonical_init() {
    let output = Command::new(env!("CARGO_BIN_EXE_adze-gen"))
        .args(["init", "--help"])
        .output()
        .expect("run adze-gen init --help");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("adze init"),
        "init help must direct users to canonical `adze init`: {stdout}"
    );
}
