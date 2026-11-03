//! Raw bytes reach the real Windows CLI through a native fake cargo.exe.

#[path = "windows_fixture.rs"]
mod windows_fixture;

use assert_cmd::cargo::cargo_bin_cmd;
use std::process::Output;
use std::time::Duration;

pub(crate) fn run_raw(mode: &str, bytes: &[u8], stderr: &[u8], status: u32) -> Output {
    let fixture = tempfile::Builder::new().prefix("adze Windows ' λ ").tempdir().unwrap();
    let bin = windows_fixture::copy_as_cargo(fixture.path());
    let wire = fixture.path().join("wire.bin");
    let err = fixture.path().join("stderr.bin");
    let grammar = fixture.path().join("grammar ' λ.rs");
    let input = fixture.path().join("input ' λ.txt");
    std::fs::write(&wire, bytes).unwrap();
    std::fs::write(&err, stderr).unwrap();
    std::fs::write(&grammar, "#[adze::grammar(\"fixture\")] mod grammar {}\n").unwrap();
    std::fs::write(&input, "hello").unwrap();
    let mut paths = vec![bin];
    paths.extend(std::env::split_paths(&std::env::var_os("PATH").unwrap()));
    let operation = match mode {
        "tree" | "json" | "sexp" | "dot" => "document-json",
        other => other,
    };
    cargo_bin_cmd!("adze")
        .arg("parse").arg(&grammar).arg(&input).args(["--output", mode])
        .current_dir(fixture.path())
        .env("PATH", std::env::join_paths(paths).unwrap())
        .env("ADZE_WINDOWS_FIXTURE_MODE", "raw")
        .env("EXPECTED_INPUT", &input).env("EXPECTED_OPERATION", operation)
        .env("EXPECTED_CWD", fixture.path().canonicalize().unwrap())
        .env("WIRE_FILE", wire).env("STDERR_FILE", err).env("EXIT_CODE", status.to_string())
        .timeout(Duration::from_secs(20)).output().unwrap()
}
