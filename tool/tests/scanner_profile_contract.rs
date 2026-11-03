//! Public scanner build behavior across default and minimal tool features.
use adze_tool::scanner_build::ScannerBuilder;
use std::fs;
use tempfile::TempDir;

#[test]
fn test_build_missing_scanner_preserves_empty_output() {
    let source = TempDir::new().unwrap();
    let output = TempDir::new().unwrap();
    ScannerBuilder::new(
        "fixture",
        source.path().to_path_buf(),
        output.path().to_path_buf(),
    )
    .build()
    .unwrap();
    assert_eq!(fs::read_dir(output.path()).unwrap().count(), 0);
}

#[test]
fn test_build_rust_scanner_writes_registration() {
    let source = TempDir::new().unwrap();
    let output = TempDir::new().unwrap();
    fs::write(
        source.path().join("scanner.rs"),
        r#"use adze::external_scanner::{ExternalScanner, Lexer, ScanResult};
#[derive(Default)]
pub struct FixtureScanner;
impl ExternalScanner for FixtureScanner {
    fn scan(&mut self, _: &mut dyn Lexer, _: &[bool]) -> Option<ScanResult> { None }
    fn serialize(&self, buffer: &mut Vec<u8>) { buffer.push(17); }
    fn deserialize(&mut self, _: &[u8]) {}
}
"#,
    )
    .unwrap();
    ScannerBuilder::new(
        "fixture",
        source.path().to_path_buf(),
        output.path().to_path_buf(),
    )
    .build()
    .unwrap();
    let registration =
        fs::read_to_string(output.path().join("fixture_scanner_registration.rs")).unwrap();
    assert!(registration.contains("register_rust::<FixtureScanner>()"));
    assert!(!output.path().join("fixture_scanner_bindings.rs").exists());
}

#[cfg(not(feature = "build_parsers"))]
#[test]
fn test_build_c_and_cpp_without_feature_refuses_without_partial_artifacts() {
    for extension in ["c", "cc", "cpp"] {
        let source = TempDir::new().unwrap();
        let output = TempDir::new().unwrap();
        let path = source.path().join(format!("scanner.{extension}"));
        fs::write(&path, "int scanner_marker(void) { return 37; }\n").unwrap();
        let error = ScannerBuilder::new(
            "fixture",
            source.path().to_path_buf(),
            output.path().to_path_buf(),
        )
        .build()
        .unwrap_err();
        let diagnostic = error.to_string();
        assert!(
            diagnostic.contains("requires the adze-tool `build_parsers` feature"),
            "{diagnostic}"
        );
        assert!(
            diagnostic.contains(&path.display().to_string()),
            "{diagnostic}"
        );
        assert_eq!(fs::read_dir(output.path()).unwrap().count(), 0);
    }
}
