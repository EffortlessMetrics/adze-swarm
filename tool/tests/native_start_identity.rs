//! Execute checked generated starts, retained roots and typed engine controls.
#[test]
fn test_native_start_identity_controls() {
    let repository = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap();
    let fixture = tempfile::Builder::new()
        .prefix("adze-imported-leaf-")
        .tempdir()
        .unwrap();
    let path = fixture.keep();
    std::fs::create_dir_all(path.join("src")).unwrap();
    let manifest = format!(
        r#"[package]
name = "adze-native-start-consumer"
version = "0.0.0"
edition = "2024"
[workspace]
[features]
default = ["pure-rust"]
pure-rust = ["adze/pure-rust"]
glr = ["adze/glr"]
[dependencies]
adze = {{ path = {:?}, default-features = false }}
[build-dependencies]
adze-tool = {{ path = {:?} }}
adze-ir = {{ path = {:?} }}
serde_json = "1.0"
"#,
        repository.join("runtime"),
        repository.join("tool"),
        repository.join("ir")
    );
    std::fs::write(path.join("Cargo.toml"), manifest).unwrap();
    std::fs::copy(repository.join("Cargo.lock"), path.join("Cargo.lock")).unwrap();
    std::fs::write(
        path.join("build.rs"),
        include_str!("fixtures/native_start_identity/build.rs"),
    )
    .unwrap();
    std::fs::write(
        path.join("src/lib.rs"),
        include_str!("fixtures/native_start_identity/lib.rs"),
    )
    .unwrap();
    let target = std::env::var_os("CARGO_TARGET_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| repository.join("target"))
        .join("native-start-identity-consumer");
    for (features, expected_count) in [(None, 11), (Some("glr"), 15)] {
        let mut command = std::process::Command::new(env!("CARGO"));
        command
            .args(["test", "--offline", "--manifest-path"])
            .arg(path.join("Cargo.toml"))
            .arg("--lib")
            .current_dir(&path)
            .env("CARGO_TARGET_DIR", &target);
        if let Some(features) = features {
            command.args(["--features", features]);
        }
        let output = command.output().unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            output.status.success(),
            "fixture retained at {} with features {features:?}\nstdout:\n{stdout}\nstderr:\n{}",
            path.display(),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            stdout.contains(&format!("{expected_count} passed; 0 failed; 0 ignored")),
            "all generated native identity controls must execute: {stdout}"
        );
        println!("features={features:?}\n{stdout}");
    }
    std::fs::remove_dir_all(path).unwrap();
}
