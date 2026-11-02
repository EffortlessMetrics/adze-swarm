//! Execute generated extraction guards; expansion-string checks are separate.
#[test]
fn test_generated_none_native_extraction_controls() -> Result<(), Box<dyn std::error::Error>> {
    let repository = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or("repository parent")?;
    let fixture = tempfile::Builder::new()
        .prefix("adze-none-extraction-")
        .tempdir()?;
    let path = fixture.keep();
    std::fs::create_dir(path.join("src"))?;
    std::fs::write(
        path.join("Cargo.toml"),
        format!(
            r#"[package]
name = "adze-none-extraction-consumer"
version = "0.0.0"
edition = "2024"
[workspace]
[features]
default = ["pure-rust"]
pure-rust = ["adze/pure-rust"]
[dependencies]
adze = {{ path = {:?}, default-features = false }}
[build-dependencies]
adze-tool = {{ path = {:?} }}
"#,
            repository.join("runtime"),
            repository.join("tool")
        ),
    )?;
    std::fs::copy(repository.join("Cargo.lock"), path.join("Cargo.lock"))?;
    std::fs::write(
        path.join("build.rs"),
        include_str!("fixtures/none_node_native/build.rs"),
    )?;
    std::fs::write(
        path.join("src/lib.rs"),
        include_str!("fixtures/none_node_native/lib.rs"),
    )?;
    let target = std::env::var_os("CARGO_TARGET_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| repository.join("target"))
        .join("none-extraction-native-consumer");
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
        stdout.contains("7 passed; 0 failed; 0 ignored"),
        "seven native controls must execute: {stdout}"
    );
    println!("{stdout}");
    std::fs::remove_dir_all(path)?;
    Ok(())
}
