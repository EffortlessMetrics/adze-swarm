//! Execute shared-mode overlap through generated lexer, native CST and driver.
#[test]
fn test_generated_native_modes_preserve_state_specific_lookahead()
-> Result<(), Box<dyn std::error::Error>> {
    let repository = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap();
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "adze-mode-lookahead-{}-{nonce}",
        std::process::id()
    ));
    std::fs::create_dir(&path)?;
    std::fs::create_dir(path.join("src"))?;
    let manifest = format!(
        r#"[package]
name = "adze-mode-lookahead-consumer"
version = "0.0.0"
edition = "2024"
[workspace]
[features]
default = ["pure-rust"]
pure-rust = ["adze/pure-rust"]
[dependencies]
adze = {{ path = {:?}, default-features = false, features = ["glr"] }}
adze-ir = {{ path = {:?} }}
adze-glr-core = {{ path = {:?} }}
[build-dependencies]
adze-tool = {{ path = {:?} }}
adze-ir = {{ path = {:?} }}
"#,
        repository.join("runtime"),
        repository.join("ir"),
        repository.join("glr-core"),
        repository.join("tool"),
        repository.join("ir")
    );
    std::fs::write(path.join("Cargo.toml"), manifest)?;
    std::fs::copy(repository.join("Cargo.lock"), path.join("Cargo.lock"))?;
    std::fs::write(
        path.join("grammar.rs"),
        include_str!("fixtures/lex_mode_lookahead/grammar.rs"),
    )?;
    std::fs::write(
        path.join("build.rs"),
        include_str!("fixtures/lex_mode_lookahead/build.rs"),
    )?;
    std::fs::write(
        path.join("src/lib.rs"),
        include_str!("fixtures/lex_mode_lookahead/lib.rs"),
    )?;
    let target = std::env::var_os("CARGO_TARGET_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| repository.join("target"))
        .join("native-mode-lookahead-consumer");
    let output = std::process::Command::new(env!("CARGO"))
        .args(["test", "--offline", "--manifest-path"])
        .arg(path.join("Cargo.toml"))
        .current_dir(&path)
        .env("CARGO_TARGET_DIR", target)
        .output()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "fixture retained at {}\nstdout:\n{stdout}\nstderr:\n{}",
        path.display(),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        stdout.contains("5 passed; 0 failed; 0 ignored"),
        "all five native controls must execute: {stdout}"
    );
    println!("{stdout}");
    std::fs::remove_dir_all(path)?;
    Ok(())
}
