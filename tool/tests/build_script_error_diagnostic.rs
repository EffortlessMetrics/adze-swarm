//! Exercise the existing unit-returning build-script failure boundary.
#[test]
fn test_build_script_grammar_error_names_exact_source_path_and_cause() -> anyhow::Result<()> {
    let repository = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or_else(|| anyhow::anyhow!("tool belongs to repository"))?;
    let fixture = tempfile::Builder::new()
        .prefix("adze-build-error-")
        .tempdir()?;
    std::fs::create_dir(fixture.path().join("src"))?;
    std::fs::write(fixture.path().join("src/lib.rs"), "")?;
    let grammar = fixture.path().join("invalid grammar é.rs");
    std::fs::write(
        &grammar,
        r#"#[adze::grammar("diagnostic")]
mod grammar {
    #[adze::language]
    pub struct Root {}
}
"#,
    )?;
    std::fs::write(
        fixture.path().join("Cargo.toml"),
        format!(
            r#"[package]
name = "adze-build-error-consumer"
version = "0.0.0"
edition = "2024"
[workspace]
[build-dependencies]
adze-tool = {{ path = {:?} }}
"#,
            repository.join("tool")
        ),
    )?;
    std::fs::copy(
        repository.join("Cargo.lock"),
        fixture.path().join("Cargo.lock"),
    )?;
    std::fs::write(
        fixture.path().join("build.rs"),
        format!(
            "fn main() {{ adze_tool::build_parsers(std::path::Path::new({:?})); }}",
            grammar
        ),
    )?;
    let target = std::env::var_os("CARGO_TARGET_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| repository.join("target"))
        .join("build-script-error-consumer");
    let output = std::process::Command::new(env!("CARGO"))
        .args(["check", "--offline", "--manifest-path"])
        .arg(fixture.path().join("Cargo.toml"))
        .current_dir(fixture.path())
        .env("CARGO_TARGET_DIR", target)
        .env_remove("CARGO_FEATURE_PURE_RUST")
        .env_remove("ADZE_USE_PURE_RUST")
        .output()?;
    assert!(
        !output.status.success(),
        "malformed annotated grammar must fail its real build script"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    let expected = format!(
        "adze: failed to generate grammars from {}: struct Root has no non-skipped fields",
        grammar.display()
    );
    assert!(
        stderr.contains(&expected),
        "expected exact build-script source path and cause {expected:?}; stderr:\n{stderr}"
    );
    Ok(())
}
