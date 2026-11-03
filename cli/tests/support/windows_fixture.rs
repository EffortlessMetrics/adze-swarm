//! Compile the dependency-free native producer with the selected Rust toolchain.
//! This is not the bootstrap: production always uses the actual adze.exe.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

pub(crate) fn fixture_exe() -> &'static Path {
    // Retain the executable for this test process, including concurrent raw
    // output cases. The hosted runner owns its temporary directory afterward.
    static FIXTURE: OnceLock<(tempfile::TempDir, PathBuf)> = OnceLock::new();
    &FIXTURE.get_or_init(|| {
        let directory = tempfile::Builder::new().prefix("adze native fixture ").tempdir().unwrap();
        let executable = directory.path().join("runner-fixture.exe");
        let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/windows_runner_fixture.rs");
        let output = Command::new("rustc")
            .args(["--edition=2024", "-Dwarnings", "--crate-name", "adze_windows_fixture"])
            .arg(source).arg("-o").arg(&executable).output().unwrap();
        assert!(output.status.success(), "native fixture compilation failed: {}",
            String::from_utf8_lossy(&output.stderr));
        (directory, executable)
    }).1
}

pub(crate) fn copy_as_cargo(directory: &Path) -> PathBuf {
    let bin = directory.join("bin");
    std::fs::create_dir(&bin).unwrap();
    let target = bin.join("cargo.exe");
    if std::fs::hard_link(fixture_exe(), &target).is_err() {
        std::fs::copy(fixture_exe(), &target).unwrap();
    }
    bin
}
