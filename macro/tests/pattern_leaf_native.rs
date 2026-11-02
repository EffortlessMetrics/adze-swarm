//! Compile annotated leaf enums through the real native generator and extraction.
#[test]
fn test_generated_pattern_leaf_native_controls() {
    let repository = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap();
    let fixture = tempfile::Builder::new()
        .prefix("adze-pattern-leaf-")
        .tempdir()
        .unwrap();
    let path = fixture.keep();
    std::fs::create_dir_all(path.join("src")).unwrap();
    let manifest = format!(
        r#"[package]
name = "adze-pattern-leaf-consumer"
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
    );
    std::fs::write(path.join("Cargo.toml"), manifest).unwrap();
    std::fs::copy(repository.join("Cargo.lock"), path.join("Cargo.lock")).unwrap();
    std::fs::write(
        path.join("build.rs"),
        include_str!("fixtures/pattern_leaf_native/build.rs"),
    )
    .unwrap();
    std::fs::write(
        path.join("src/lib.rs"),
        include_str!("fixtures/pattern_leaf_native/lib.rs"),
    )
    .unwrap();
    let target = std::env::var_os("CARGO_TARGET_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| repository.join("target"))
        .join("pattern-leaf-native-consumer");
    let output = std::process::Command::new(env!("CARGO"))
        .args(["test", "--offline", "--manifest-path"])
        .arg(path.join("Cargo.toml"))
        .current_dir(&path)
        .env("CARGO_TARGET_DIR", target)
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "fixture retained at {}\nstdout:\n{stdout}\nstderr:\n{}",
        path.display(),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        stdout.contains("12 passed; 0 failed; 0 ignored"),
        "all twelve native controls must execute: {stdout}"
    );
    println!("{stdout}");
    std::fs::remove_dir_all(path).unwrap();
}
