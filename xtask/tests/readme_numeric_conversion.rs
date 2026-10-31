//! Compile the actual README grammar, then probe numeric conversion boundaries.

use std::fs;
use std::path::Path;
use std::process::Command;

fn toml_path(path: &Path) -> String {
    path.to_str()
        .expect("repository path must be UTF-8")
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
}

#[test]
fn readme_numeric_conversion_is_fallible() {
    // Normalize documentation checkout line endings, never parser input.
    let readme = include_str!("../../README.md").replace("\r\n", "\n");
    let grammar_block = readme
        .split("```rust\n")
        .skip(1)
        .filter_map(|block| block.split_once("\n```"))
        .map(|(code, _)| code)
        .find(|code| code.starts_with("#[adze::grammar"))
        .expect("README grammar block");
    let (grammar, _) = grammar_block
        .split_once("\nlet expr =")
        .expect("README grammar and usage example");
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("repository root");
    let temp = tempfile::tempdir().expect("temporary downstream project");
    let project = temp.path();
    fs::create_dir_all(project.join("src")).expect("source directory");
    fs::create_dir_all(project.join("tests")).expect("test directory");
    fs::write(
        project.join("Cargo.toml"),
        format!(
            r#"[package]
name = "readme-conversion-probe"
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
            toml_path(&root.join("runtime")),
            toml_path(&root.join("tool")),
        ),
    )
    .expect("write manifest");
    fs::write(project.join("src/lib.rs"), grammar).expect("write README grammar");
    fs::write(
        project.join("build.rs"),
        r#"fn main() {
    println!("cargo::rustc-check-cfg=cfg(adze_unsafe_attrs)");
    adze_tool::build_parsers(&std::path::PathBuf::from("src/lib.rs"));
}
"#,
    )
    .expect("write build script");
    fs::write(
        project.join("tests/conversion_contract.rs"),
        r#"use adze::errors::ParseErrorReason;
use readme_conversion_probe::grammar::{self, Expr};

fn assert_conversion_failure(source: &str, expected: std::ops::Range<usize>) {
    let result = std::panic::catch_unwind(|| grammar::parse(source));
    let errors = result.expect("conversion must not panic").expect_err("must not default to success");
    let first = errors.first().expect("structured conversion diagnostic");
    assert!(matches!(first.reason, ParseErrorReason::ConversionFailed(_)), "{first:?}");
    assert_eq!(first.byte_span(), expected);
}

#[test]
fn valid_numeric_boundaries_keep_exact_values() {
    assert_eq!(grammar::parse("0").expect("zero"), Expr::Number(0));
    assert_eq!(grammar::parse("2147483647").expect("i32 maximum"), Expr::Number(i32::MAX));
}

#[test]
fn one_past_maximum_is_a_structured_error() {
    assert_conversion_failure("2147483648", 0..10);
}

#[test]
fn embedded_overflow_keeps_its_token_range() {
    assert_conversion_failure("1 + 2147483648", 4..14);
}

#[test]
fn long_numeric_token_does_not_panic_or_default() {
    let source = "9".repeat(80);
    assert_conversion_failure(&source, 0..80);
}
"#,
    )
    .expect("write conversion contract");
    let output = Command::new("cargo")
        .args([
            "test",
            "--test",
            "conversion_contract",
            "--",
            "--test-threads=1",
        ])
        .current_dir(project)
        .env("CARGO_TARGET_DIR", project.join("target"))
        .output()
        .expect("run downstream conversion tests");
    assert!(
        output.status.success(),
        "README numeric conversion failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    assert!(
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .any(|line| line.starts_with("test result: ok. 4 passed; 0 failed; 0 ignored;")),
        "all four conversion controls must execute: {:?}",
        output.stdout,
    );
}
