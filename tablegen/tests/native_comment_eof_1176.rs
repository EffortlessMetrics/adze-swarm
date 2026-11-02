//! Execute exact generated EOF-comment source ranges through the native runtime.
#[test]
fn test_native_eof_comments_retain_exact_source_ranges() -> Result<(), Box<dyn std::error::Error>> {
    let repository = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap();
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "adze-native-comment-eof-{}-{nonce}",
        std::process::id()
    ));
    std::fs::create_dir(&path)?;
    std::fs::create_dir(path.join("src"))?;
    let manifest = format!(
        r#"[package]
name = "adze-native-comment-eof-consumer"
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
    std::fs::write(path.join("Cargo.toml"), manifest)?;
    std::fs::copy(repository.join("Cargo.lock"), path.join("Cargo.lock"))?;
    std::fs::write(
        path.join("grammar.rs"),
        include_str!("fixtures/native_comment_eof/grammar.rs"),
    )?;
    std::fs::write(
        path.join("build.rs"),
        include_str!("fixtures/native_comment_eof/build.rs"),
    )?;
    std::fs::write(
        path.join("src/lib.rs"),
        include_str!("fixtures/native_comment_eof/lib.rs"),
    )?;
    let target = std::env::var_os("CARGO_TARGET_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| repository.join("target"))
        .join("native-comment-eof-consumer");
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
        stdout.contains("1 passed; 0 failed; 0 ignored"),
        "the native EOF/NUL control must execute: {stdout}"
    );
    println!("{stdout}");
    std::fs::remove_dir_all(path)?;
    Ok(())
}
