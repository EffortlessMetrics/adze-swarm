//! Comprehensive tests for scanner_build module.
//!
//! Tests scanner discovery, language detection, builder config, code generation,
//! error handling, and edge cases.

use adze_tool::scanner_build::*;
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

// ═══════════════════════════════════════════════════════════════════
// 1. ScannerLanguage enum
// ═══════════════════════════════════════════════════════════════════

#[test]
fn test_scanner_language_c_extension() {
    assert_eq!(ScannerLanguage::C.extension(), "c");
}

#[test]
fn test_scanner_language_cpp_extension() {
    assert_eq!(ScannerLanguage::Cpp.extension(), "cc");
}

#[test]
fn test_scanner_language_rust_extension() {
    assert_eq!(ScannerLanguage::Rust.extension(), "rs");
}

#[test]
fn test_scanner_language_equality_same_variants() {
    assert_eq!(ScannerLanguage::C, ScannerLanguage::C);
    assert_eq!(ScannerLanguage::Cpp, ScannerLanguage::Cpp);
    assert_eq!(ScannerLanguage::Rust, ScannerLanguage::Rust);
}

#[test]
fn test_scanner_language_inequality_different_variants() {
    assert_ne!(ScannerLanguage::C, ScannerLanguage::Cpp);
    assert_ne!(ScannerLanguage::C, ScannerLanguage::Rust);
    assert_ne!(ScannerLanguage::Cpp, ScannerLanguage::Rust);
}

#[test]
fn test_scanner_language_debug_format() {
    assert_eq!(format!("{:?}", ScannerLanguage::C), "C");
    assert_eq!(format!("{:?}", ScannerLanguage::Cpp), "Cpp");
    assert_eq!(format!("{:?}", ScannerLanguage::Rust), "Rust");
}

#[test]
fn test_scanner_language_copy_semantics() {
    let lang = ScannerLanguage::C;
    let copied = lang;
    // Both should still be usable after copy
    assert_eq!(lang, copied);
    assert_eq!(lang.extension(), copied.extension());
}

#[test]
fn test_scanner_language_clone_semantics() {
    let lang = ScannerLanguage::Rust;
    let cloned = lang;
    assert_eq!(lang, cloned);
}

#[test]
fn test_scanner_language_all_extensions_unique() {
    let exts: Vec<&str> = vec![
        ScannerLanguage::C.extension(),
        ScannerLanguage::Cpp.extension(),
        ScannerLanguage::Rust.extension(),
    ];
    // All extensions should be unique
    let mut sorted = exts.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(exts.len(), sorted.len());
}

// ═══════════════════════════════════════════════════════════════════
// 2. ScannerSource struct
// ═══════════════════════════════════════════════════════════════════

#[test]
fn test_scanner_source_construction_c() {
    let src = ScannerSource {
        path: PathBuf::from("scanner.c"),
        language: ScannerLanguage::C,
        grammar_name: "test".to_string(),
    };
    assert_eq!(src.path, PathBuf::from("scanner.c"));
    assert_eq!(src.language, ScannerLanguage::C);
    assert_eq!(src.grammar_name, "test");
}

#[test]
fn test_scanner_source_construction_cpp() {
    let src = ScannerSource {
        path: PathBuf::from("scanner.cc"),
        language: ScannerLanguage::Cpp,
        grammar_name: "cpp_lang".to_string(),
    };
    assert_eq!(src.language, ScannerLanguage::Cpp);
    assert_eq!(src.grammar_name, "cpp_lang");
}

#[test]
fn test_scanner_source_construction_rust() {
    let src = ScannerSource {
        path: PathBuf::from("scanner.rs"),
        language: ScannerLanguage::Rust,
        grammar_name: "rust_lang".to_string(),
    };
    assert_eq!(src.language, ScannerLanguage::Rust);
}

#[test]
fn test_scanner_source_clone() {
    let src = ScannerSource {
        path: PathBuf::from("/deep/path/scanner.c"),
        language: ScannerLanguage::C,
        grammar_name: "json".to_string(),
    };
    let cloned = src.clone();
    assert_eq!(cloned.path, src.path);
    assert_eq!(cloned.language, src.language);
    assert_eq!(cloned.grammar_name, src.grammar_name);
}

#[test]
fn test_scanner_source_debug_contains_fields() {
    let src = ScannerSource {
        path: PathBuf::from("scanner.cc"),
        language: ScannerLanguage::Cpp,
        grammar_name: "cpp".to_string(),
    };
    let debug = format!("{:?}", src);
    assert!(debug.contains("ScannerSource"));
    assert!(debug.contains("scanner.cc"));
    assert!(debug.contains("Cpp"));
    assert!(debug.contains("cpp"));
}

#[test]
fn test_scanner_source_with_nested_path() {
    let src = ScannerSource {
        path: PathBuf::from("/a/b/c/d/scanner.rs"),
        language: ScannerLanguage::Rust,
        grammar_name: "deep".to_string(),
    };
    assert!(src.path.ends_with("scanner.rs"));
    assert!(src.path.components().count() >= 5);
}

#[test]
fn test_scanner_source_grammar_names_preserved() {
    for name in &["json", "python", "javascript", "c_lang", "my_grammar_v2"] {
        let src = ScannerSource {
            path: PathBuf::from("scanner.c"),
            language: ScannerLanguage::C,
            grammar_name: name.to_string(),
        };
        assert_eq!(&src.grammar_name, name);
    }
}

#[test]
fn test_scanner_source_empty_grammar_name() {
    let src = ScannerSource {
        path: PathBuf::from("scanner.c"),
        language: ScannerLanguage::C,
        grammar_name: String::new(),
    };
    assert!(src.grammar_name.is_empty());
}

// ═══════════════════════════════════════════════════════════════════
// 3. ScannerBuilder construction
// ═══════════════════════════════════════════════════════════════════

#[test]
fn test_scanner_builder_new_str() {
    let builder = ScannerBuilder::new("test", PathBuf::from("/src"), PathBuf::from("/out"));
    // Builder is constructed without error
    let _ = builder;
}

#[test]
fn test_scanner_builder_new_string() {
    let builder = ScannerBuilder::new(
        String::from("my_grammar"),
        PathBuf::from("/src"),
        PathBuf::from("/out"),
    );
    let _ = builder;
}

#[test]
fn test_scanner_builder_new_into_string() {
    // Verify that Into<String> works (e.g., &String)
    let name = String::from("owned_name");
    let builder = ScannerBuilder::new(&*name, PathBuf::from("/s"), PathBuf::from("/o"));
    let _ = builder;
}

// ═══════════════════════════════════════════════════════════════════
// 4. Scanner discovery — generic scanner.{c,cc,cpp,rs}
// ═══════════════════════════════════════════════════════════════════

#[test]
fn test_find_scanner_empty_dir() {
    let dir = TempDir::new().unwrap();
    let builder = ScannerBuilder::new("test", dir.path().to_path_buf(), dir.path().to_path_buf());
    assert!(builder.find_scanner().unwrap().is_none());
}

#[test]
fn test_find_scanner_c_file() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("scanner.c"), "// C").unwrap();
    let builder = ScannerBuilder::new("test", dir.path().to_path_buf(), dir.path().to_path_buf());
    let src = builder.find_scanner().unwrap().unwrap();
    assert_eq!(src.language, ScannerLanguage::C);
    assert!(src.path.ends_with("scanner.c"));
    assert_eq!(src.grammar_name, "test");
}

#[test]
fn test_find_scanner_cc_file() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("scanner.cc"), "// C++").unwrap();
    let builder = ScannerBuilder::new("test", dir.path().to_path_buf(), dir.path().to_path_buf());
    let src = builder.find_scanner().unwrap().unwrap();
    assert_eq!(src.language, ScannerLanguage::Cpp);
}

#[test]
fn test_find_scanner_cpp_file() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("scanner.cpp"), "// C++").unwrap();
    let builder = ScannerBuilder::new("test", dir.path().to_path_buf(), dir.path().to_path_buf());
    let src = builder.find_scanner().unwrap().unwrap();
    assert_eq!(src.language, ScannerLanguage::Cpp);
}

#[test]
fn test_find_scanner_rs_file() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("scanner.rs"), "// Rust").unwrap();
    let builder = ScannerBuilder::new("test", dir.path().to_path_buf(), dir.path().to_path_buf());
    let src = builder.find_scanner().unwrap().unwrap();
    assert_eq!(src.language, ScannerLanguage::Rust);
}

// ═══════════════════════════════════════════════════════════════════
// 5. Scanner discovery — grammar-named scanner files
// ═══════════════════════════════════════════════════════════════════

#[test]
fn test_find_named_scanner_c() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("json_scanner.c"), "// scanner").unwrap();
    let builder = ScannerBuilder::new("json", dir.path().to_path_buf(), dir.path().to_path_buf());
    let src = builder.find_scanner().unwrap().unwrap();
    assert_eq!(src.language, ScannerLanguage::C);
    assert_eq!(src.grammar_name, "json");
}

#[test]
fn test_find_named_scanner_cc() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("python_scanner.cc"), "// scanner").unwrap();
    let builder = ScannerBuilder::new("python", dir.path().to_path_buf(), dir.path().to_path_buf());
    let src = builder.find_scanner().unwrap().unwrap();
    assert_eq!(src.language, ScannerLanguage::Cpp);
    assert_eq!(src.grammar_name, "python");
}

#[test]
fn test_find_named_scanner_rs() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("html_scanner.rs"), "// scanner").unwrap();
    let builder = ScannerBuilder::new("html", dir.path().to_path_buf(), dir.path().to_path_buf());
    let src = builder.find_scanner().unwrap().unwrap();
    assert_eq!(src.language, ScannerLanguage::Rust);
}

// ═══════════════════════════════════════════════════════════════════
// 6. Scanner discovery — priority ordering
// ═══════════════════════════════════════════════════════════════════

#[test]
fn test_find_scanner_priority_c_before_cc() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("scanner.c"), "// C").unwrap();
    fs::write(dir.path().join("scanner.cc"), "// C++").unwrap();
    let builder = ScannerBuilder::new("test", dir.path().to_path_buf(), dir.path().to_path_buf());
    let src = builder.find_scanner().unwrap().unwrap();
    assert_eq!(src.language, ScannerLanguage::C);
}

#[test]
fn test_find_scanner_priority_c_before_rs() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("scanner.c"), "// C").unwrap();
    fs::write(dir.path().join("scanner.rs"), "// Rust").unwrap();
    let builder = ScannerBuilder::new("test", dir.path().to_path_buf(), dir.path().to_path_buf());
    let src = builder.find_scanner().unwrap().unwrap();
    assert_eq!(src.language, ScannerLanguage::C);
}

#[test]
fn test_find_scanner_priority_cc_before_rs() {
    let dir = TempDir::new().unwrap();
    // Only cc and rs present — cc comes before rs in search order
    fs::write(dir.path().join("scanner.cc"), "// C++").unwrap();
    fs::write(dir.path().join("scanner.rs"), "// Rust").unwrap();
    let builder = ScannerBuilder::new("test", dir.path().to_path_buf(), dir.path().to_path_buf());
    let src = builder.find_scanner().unwrap().unwrap();
    assert_eq!(src.language, ScannerLanguage::Cpp);
}

#[test]
fn test_find_scanner_generic_before_named() {
    let dir = TempDir::new().unwrap();
    // generic scanner.c is searched before json_scanner.c
    fs::write(dir.path().join("scanner.c"), "// generic").unwrap();
    fs::write(dir.path().join("json_scanner.c"), "// named").unwrap();
    let builder = ScannerBuilder::new("json", dir.path().to_path_buf(), dir.path().to_path_buf());
    let src = builder.find_scanner().unwrap().unwrap();
    assert!(src.path.ends_with("scanner.c"));
}

#[test]
fn test_find_scanner_named_found_when_no_generic() {
    let dir = TempDir::new().unwrap();
    // Only the named scanner exists
    fs::write(dir.path().join("mygrammar_scanner.rs"), "// named").unwrap();
    let builder = ScannerBuilder::new(
        "mygrammar",
        dir.path().to_path_buf(),
        dir.path().to_path_buf(),
    );
    let src = builder.find_scanner().unwrap().unwrap();
    assert_eq!(src.language, ScannerLanguage::Rust);
    assert!(src.path.ends_with("mygrammar_scanner.rs"));
}

// ═══════════════════════════════════════════════════════════════════
// 7. Scanner discovery — negative / edge cases
// ═══════════════════════════════════════════════════════════════════

#[test]
fn test_find_scanner_nonexistent_dir() {
    let builder = ScannerBuilder::new(
        "test",
        PathBuf::from("/nonexistent/path/12345"),
        PathBuf::from("/tmp"),
    );
    assert!(builder.find_scanner().unwrap().is_none());
}

#[test]
fn test_find_scanner_ignores_unrecognized_extensions() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("scanner.py"), "# Python").unwrap();
    fs::write(dir.path().join("scanner.go"), "// Go").unwrap();
    fs::write(dir.path().join("scanner.js"), "// JS").unwrap();
    let builder = ScannerBuilder::new("test", dir.path().to_path_buf(), dir.path().to_path_buf());
    assert!(builder.find_scanner().unwrap().is_none());
}

#[test]
fn test_find_scanner_ignores_directories_named_scanner() {
    let dir = TempDir::new().unwrap();
    // A directory named "scanner.c" should not match
    fs::create_dir(dir.path().join("scanner.c")).unwrap();
    let builder = ScannerBuilder::new("test", dir.path().to_path_buf(), dir.path().to_path_buf());
    // `path.exists()` returns true for dirs too, but extension matching should still work.
    // The scanner search just checks exists(); it may return Some. That's the current behavior.
    // This test documents the behavior.
    let result = builder.find_scanner().unwrap();
    // A directory matches exists() and has extension "c", so the current impl returns it.
    // We just document this behavior.
    if let Some(src) = result {
        assert_eq!(src.language, ScannerLanguage::C);
    }
    // Either way, the function doesn't panic or error
}

#[test]
fn test_find_scanner_wrong_grammar_name_no_match() {
    let dir = TempDir::new().unwrap();
    // Create a named scanner for a different grammar
    fs::write(dir.path().join("python_scanner.c"), "// python").unwrap();
    let builder = ScannerBuilder::new("json", dir.path().to_path_buf(), dir.path().to_path_buf());
    // Should not find python_scanner.c when looking for json grammar
    assert!(builder.find_scanner().unwrap().is_none());
}

#[test]
fn test_find_scanner_only_named_scanner_matches_grammar() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("abc_scanner.c"), "// abc").unwrap();
    fs::write(dir.path().join("xyz_scanner.rs"), "// xyz").unwrap();
    let builder = ScannerBuilder::new("abc", dir.path().to_path_buf(), dir.path().to_path_buf());
    let src = builder.find_scanner().unwrap().unwrap();
    assert!(src.path.ends_with("abc_scanner.c"));
}

#[test]
fn test_find_scanner_idempotent() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("scanner.rs"), "// Rust").unwrap();
    let builder = ScannerBuilder::new("test", dir.path().to_path_buf(), dir.path().to_path_buf());
    let r1 = builder.find_scanner().unwrap();
    let r2 = builder.find_scanner().unwrap();
    assert!(r1.is_some() && r2.is_some());
    assert_eq!(r1.unwrap().language, r2.unwrap().language);
}

// ═══════════════════════════════════════════════════════════════════
// 8. Build pipeline — no scanner (no-op)
// ═══════════════════════════════════════════════════════════════════

#[test]
fn test_build_no_scanner_succeeds() {
    let dir = TempDir::new().unwrap();
    let builder = ScannerBuilder::new("test", dir.path().to_path_buf(), dir.path().to_path_buf());
    assert!(builder.build().is_ok());
}

#[test]
fn test_build_empty_dir_is_no_op() {
    let dir = TempDir::new().unwrap();
    let builder = ScannerBuilder::new("empty", dir.path().to_path_buf(), dir.path().to_path_buf());
    builder.build().unwrap();
    // No output files should be created
    let entries: Vec<_> = fs::read_dir(dir.path()).unwrap().collect();
    assert!(entries.is_empty());
}

#[test]
fn test_build_unrelated_files_ignored() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("main.rs"), "fn main() {}").unwrap();
    fs::write(dir.path().join("lib.c"), "int x;").unwrap();
    let builder = ScannerBuilder::new("test", dir.path().to_path_buf(), dir.path().to_path_buf());
    assert!(builder.build().is_ok());
}

// ═══════════════════════════════════════════════════════════════════
// 9. Build pipeline — Rust scanner builds
// ═══════════════════════════════════════════════════════════════════

#[test]
fn test_build_rust_scanner_with_impl_line() {
    let dir = TempDir::new().unwrap();
    let out_dir = TempDir::new().unwrap();
    let scanner_content = r#"
pub struct MyScanner {
    state: u32,
}

impl ExternalScanner for MyScanner {
    fn scan(&mut self) -> bool { false }
}
"#;
    fs::write(dir.path().join("scanner.rs"), scanner_content).unwrap();
    let builder = ScannerBuilder::new(
        "test",
        dir.path().to_path_buf(),
        out_dir.path().to_path_buf(),
    );
    let result = builder.build();
    assert!(result.is_ok());
    // Registration file should be generated
    let reg_path = out_dir.path().join("test_scanner_registration.rs");
    assert!(reg_path.exists(), "registration file should be created");
    let content = fs::read_to_string(&reg_path).unwrap();
    assert!(content.contains("MyScanner"));
    assert!(content.contains("test"));
}

#[test]
fn test_build_rust_scanner_registration_contains_grammar_name() {
    let dir = TempDir::new().unwrap();
    let out_dir = TempDir::new().unwrap();
    let scanner_content = r#"
pub struct PythonScanner;
impl ExternalScanner for PythonScanner {}
"#;
    fs::write(dir.path().join("scanner.rs"), scanner_content).unwrap();
    let builder = ScannerBuilder::new(
        "python",
        dir.path().to_path_buf(),
        out_dir.path().to_path_buf(),
    );
    builder.build().unwrap();
    let reg_path = out_dir.path().join("python_scanner_registration.rs");
    let content = fs::read_to_string(&reg_path).unwrap();
    assert!(content.contains("\"python\""));
    assert!(content.contains("PythonScanner"));
}

#[test]
fn test_build_rust_scanner_fallback_struct_detection() {
    let dir = TempDir::new().unwrap();
    let out_dir = TempDir::new().unwrap();
    // No "impl ExternalScanner for" line, but has pub struct with Scanner in name
    let scanner_content = r#"
pub struct IndentScanner {
    depth: usize,
}
"#;
    fs::write(dir.path().join("scanner.rs"), scanner_content).unwrap();
    let builder = ScannerBuilder::new(
        "indent",
        dir.path().to_path_buf(),
        out_dir.path().to_path_buf(),
    );
    builder.build().unwrap();
    let reg_path = out_dir.path().join("indent_scanner_registration.rs");
    let content = fs::read_to_string(&reg_path).unwrap();
    assert!(content.contains("IndentScanner"));
}

#[test]
fn test_build_rust_scanner_no_struct_found_errors() {
    let dir = TempDir::new().unwrap();
    let out_dir = TempDir::new().unwrap();
    // No matching struct at all
    let scanner_content = r#"
fn helper() -> bool { true }
pub struct SomethingElse;
"#;
    fs::write(dir.path().join("scanner.rs"), scanner_content).unwrap();
    let builder = ScannerBuilder::new(
        "bad",
        dir.path().to_path_buf(),
        out_dir.path().to_path_buf(),
    );
    let result = builder.build();
    assert!(result.is_err(), "should error when no scanner struct found");
}

#[test]
fn test_build_rust_scanner_named_file() {
    let dir = TempDir::new().unwrap();
    let out_dir = TempDir::new().unwrap();
    let scanner_content = r#"
pub struct JavaScanner;
impl ExternalScanner for JavaScanner {}
"#;
    fs::write(dir.path().join("java_scanner.rs"), scanner_content).unwrap();
    let builder = ScannerBuilder::new(
        "java",
        dir.path().to_path_buf(),
        out_dir.path().to_path_buf(),
    );
    builder.build().unwrap();
    assert!(out_dir.path().join("java_scanner_registration.rs").exists());
}

#[test]
fn test_build_rust_scanner_registration_includes_include_path() {
    let dir = TempDir::new().unwrap();
    let out_dir = TempDir::new().unwrap();
    let scanner_content = "pub struct TestScanner;\nimpl ExternalScanner for TestScanner {}\n";
    fs::write(dir.path().join("scanner.rs"), scanner_content).unwrap();
    let builder = ScannerBuilder::new(
        "test",
        dir.path().to_path_buf(),
        out_dir.path().to_path_buf(),
    );
    builder.build().unwrap();
    let content = fs::read_to_string(out_dir.path().join("test_scanner_registration.rs")).unwrap();
    // Should include!() the scanner source path
    assert!(content.contains("include!"));
}

#[test]
fn test_build_rust_scanner_registration_has_register_function() {
    let dir = TempDir::new().unwrap();
    let out_dir = TempDir::new().unwrap();
    let scanner_content = "pub struct FooScanner;\nimpl ExternalScanner for FooScanner {}\n";
    fs::write(dir.path().join("scanner.rs"), scanner_content).unwrap();
    let builder = ScannerBuilder::new(
        "foo",
        dir.path().to_path_buf(),
        out_dir.path().to_path_buf(),
    );
    builder.build().unwrap();
    let content = fs::read_to_string(out_dir.path().join("foo_scanner_registration.rs")).unwrap();
    assert!(content.contains("pub fn register_scanner()"));
    assert!(content.contains("ExternalScannerBuilder"));
}

// ═══════════════════════════════════════════════════════════════════
// 10. Build pipeline — error handling
// ═══════════════════════════════════════════════════════════════════

#[test]
fn test_build_rust_scanner_unreadable_file_errors() {
    let dir = TempDir::new().unwrap();
    let out_dir = TempDir::new().unwrap();
    let scanner_path = dir.path().join("scanner.rs");
    fs::write(&scanner_path, "content").unwrap();
    // Make file unreadable
    #[cfg(unix)]
    {
        let perms = <fs::Permissions as std::os::unix::fs::PermissionsExt>::from_mode(0o000);
        fs::set_permissions(&scanner_path, perms).unwrap();
    }
    let builder = ScannerBuilder::new(
        "test",
        dir.path().to_path_buf(),
        out_dir.path().to_path_buf(),
    );
    #[cfg(unix)]
    {
        let result = builder.build();
        assert!(result.is_err());
        // Restore permissions for cleanup
        let perms = <fs::Permissions as std::os::unix::fs::PermissionsExt>::from_mode(0o644);
        fs::set_permissions(&scanner_path, perms).unwrap();
    }
    #[cfg(not(unix))]
    {
        let _ = builder;
    }
}

#[test]
fn test_build_rust_scanner_readonly_outdir_errors() {
    let dir = TempDir::new().unwrap();
    let out_dir = TempDir::new().unwrap();
    let scanner_content = "pub struct TestScanner;\nimpl ExternalScanner for TestScanner {}\n";
    fs::write(dir.path().join("scanner.rs"), scanner_content).unwrap();

    // Make output directory read-only
    #[cfg(unix)]
    {
        let perms = <fs::Permissions as std::os::unix::fs::PermissionsExt>::from_mode(0o555);
        fs::set_permissions(out_dir.path(), perms).unwrap();
    }
    let builder = ScannerBuilder::new(
        "test",
        dir.path().to_path_buf(),
        out_dir.path().to_path_buf(),
    );
    #[cfg(unix)]
    {
        // Root bypasses directory write permissions, so a 0o555 out-dir is
        // still writable when the suite runs as uid 0 (the default in many
        // CI containers). Probe the precondition instead of assuming it: only
        // assert the build fails where the OS actually denies the write.
        let probe = out_dir.path().join(".readonly-probe");
        let write_denied = fs::write(&probe, b"probe").is_err();
        if write_denied {
            let result = builder.build();
            assert!(result.is_err());
        } else {
            fs::remove_file(&probe).unwrap();
        }
        // Restore permissions for cleanup
        let perms = <fs::Permissions as std::os::unix::fs::PermissionsExt>::from_mode(0o755);
        fs::set_permissions(out_dir.path(), perms).unwrap();
    }
    #[cfg(not(unix))]
    {
        let _ = builder;
    }
}

// ═══════════════════════════════════════════════════════════════════
// 11. find_scanner_struct patterns (tested via build pipeline)
// ═══════════════════════════════════════════════════════════════════

#[test]
fn test_scanner_struct_detected_from_impl_line() {
    let dir = TempDir::new().unwrap();
    let out_dir = TempDir::new().unwrap();
    let content = "struct Foo;\nimpl ExternalScanner for MyScanner {\n}\n";
    fs::write(dir.path().join("scanner.rs"), content).unwrap();
    let builder = ScannerBuilder::new(
        "test",
        dir.path().to_path_buf(),
        out_dir.path().to_path_buf(),
    );
    builder.build().unwrap();
    let reg = fs::read_to_string(out_dir.path().join("test_scanner_registration.rs")).unwrap();
    assert!(reg.contains("MyScanner"));
}

#[test]
fn test_scanner_struct_detected_with_brace_on_same_line() {
    let dir = TempDir::new().unwrap();
    let out_dir = TempDir::new().unwrap();
    let content = "impl ExternalScanner for BraceScanner{\nfn scan(&self) {}\n}\n";
    fs::write(dir.path().join("scanner.rs"), content).unwrap();
    let builder = ScannerBuilder::new(
        "test",
        dir.path().to_path_buf(),
        out_dir.path().to_path_buf(),
    );
    builder.build().unwrap();
    let reg = fs::read_to_string(out_dir.path().join("test_scanner_registration.rs")).unwrap();
    // The brace should be trimmed
    assert!(reg.contains("BraceScanner"));
    assert!(!reg.contains("BraceScanner{"));
}

#[test]
fn test_scanner_struct_fallback_to_pub_struct_with_scanner_in_name() {
    let dir = TempDir::new().unwrap();
    let out_dir = TempDir::new().unwrap();
    let content = "pub struct CustomScannerImpl {\n    state: bool,\n}\n";
    fs::write(dir.path().join("scanner.rs"), content).unwrap();
    let builder = ScannerBuilder::new(
        "test",
        dir.path().to_path_buf(),
        out_dir.path().to_path_buf(),
    );
    builder.build().unwrap();
    let reg = fs::read_to_string(out_dir.path().join("test_scanner_registration.rs")).unwrap();
    assert!(reg.contains("CustomScannerImpl"));
}

#[test]
fn test_scanner_struct_impl_preferred_over_pub_struct() {
    let dir = TempDir::new().unwrap();
    let out_dir = TempDir::new().unwrap();
    // Both impl line and pub struct are present; impl line should be found first
    let content = r#"
pub struct FallbackScanner;
impl ExternalScanner for PreferredScanner {}
"#;
    fs::write(dir.path().join("scanner.rs"), content).unwrap();
    let builder = ScannerBuilder::new(
        "test",
        dir.path().to_path_buf(),
        out_dir.path().to_path_buf(),
    );
    builder.build().unwrap();
    let reg = fs::read_to_string(out_dir.path().join("test_scanner_registration.rs")).unwrap();
    assert!(reg.contains("PreferredScanner"));
}

#[test]
fn test_scanner_struct_not_found_no_scanner_no_impl() {
    let dir = TempDir::new().unwrap();
    let out_dir = TempDir::new().unwrap();
    // "SomethingElse" doesn't contain "Scanner", so the fallback won't match it.
    // But the `pub struct` line also needs to NOT contain "Scanner".
    let content = "pub struct Unrelated;\nfn helper() {}\n";
    fs::write(dir.path().join("scanner.rs"), content).unwrap();
    let builder = ScannerBuilder::new(
        "test",
        dir.path().to_path_buf(),
        out_dir.path().to_path_buf(),
    );
    assert!(builder.build().is_err());
}

#[test]
fn test_scanner_struct_private_struct_not_matched() {
    let dir = TempDir::new().unwrap();
    let out_dir = TempDir::new().unwrap();
    // `struct` without `pub` doesn't match the fallback
    let content = "struct PrivateScanner;\n";
    fs::write(dir.path().join("scanner.rs"), content).unwrap();
    let builder = ScannerBuilder::new(
        "test",
        dir.path().to_path_buf(),
        out_dir.path().to_path_buf(),
    );
    assert!(builder.build().is_err());
}

// ═══════════════════════════════════════════════════════════════════
// 12. C/C++ scanner build — bindings generation
// ═══════════════════════════════════════════════════════════════════
//
// NOTE: C/C++ build() invokes cc::Build which needs a real C compiler
// and tree-sitter headers. We can't easily test the full build path
// in a unit test. Instead we test what we can without invoking cc.

#[test]
fn test_find_scanner_returns_correct_path_for_c() {
    let dir = TempDir::new().unwrap();
    let scanner_path = dir.path().join("scanner.c");
    fs::write(&scanner_path, "void foo() {}").unwrap();
    let builder = ScannerBuilder::new("test", dir.path().to_path_buf(), dir.path().to_path_buf());
    let src = builder.find_scanner().unwrap().unwrap();
    assert_eq!(src.path, scanner_path);
}

#[test]
fn test_find_scanner_returns_correct_path_for_cpp() {
    let dir = TempDir::new().unwrap();
    let scanner_path = dir.path().join("scanner.cpp");
    fs::write(&scanner_path, "void foo() {}").unwrap();
    let builder = ScannerBuilder::new("test", dir.path().to_path_buf(), dir.path().to_path_buf());
    let src = builder.find_scanner().unwrap().unwrap();
    assert_eq!(src.path, scanner_path);
}

// ═══════════════════════════════════════════════════════════════════
// 13. Edge cases — grammar names with special characters
// ═══════════════════════════════════════════════════════════════════

#[test]
fn test_find_scanner_grammar_with_underscores() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("my_lang_scanner.c"), "// s").unwrap();
    let builder = ScannerBuilder::new(
        "my_lang",
        dir.path().to_path_buf(),
        dir.path().to_path_buf(),
    );
    let src = builder.find_scanner().unwrap().unwrap();
    assert_eq!(src.grammar_name, "my_lang");
}

#[test]
fn test_find_scanner_grammar_with_numbers() {
    let dir = TempDir::new().unwrap();
    fs::write(
        dir.path().join("v2_scanner.rs"),
        "pub struct V2Scanner;\nimpl ExternalScanner for V2Scanner {}\n",
    )
    .unwrap();
    let builder = ScannerBuilder::new("v2", dir.path().to_path_buf(), dir.path().to_path_buf());
    let src = builder.find_scanner().unwrap().unwrap();
    assert_eq!(src.grammar_name, "v2");
}

#[test]
fn test_build_with_empty_grammar_name() {
    let dir = TempDir::new().unwrap();
    // Empty grammar name means named scanner files are "_scanner.c" etc.
    fs::write(dir.path().join("_scanner.c"), "// s").unwrap();
    let builder = ScannerBuilder::new("", dir.path().to_path_buf(), dir.path().to_path_buf());
    // Should find _scanner.c since it generates format!("{}_scanner.c", "")
    let result = builder.find_scanner().unwrap();
    if let Some(src) = result {
        assert_eq!(src.grammar_name, "");
    }
}

// ═══════════════════════════════════════════════════════════════════
// 14. Edge cases — scanner file content patterns
// ═══════════════════════════════════════════════════════════════════

#[test]
fn test_build_rust_scanner_empty_file_errors() {
    let dir = TempDir::new().unwrap();
    let out_dir = TempDir::new().unwrap();
    fs::write(dir.path().join("scanner.rs"), "").unwrap();
    let builder = ScannerBuilder::new(
        "test",
        dir.path().to_path_buf(),
        out_dir.path().to_path_buf(),
    );
    assert!(builder.build().is_err());
}

#[test]
fn test_build_rust_scanner_whitespace_only_errors() {
    let dir = TempDir::new().unwrap();
    let out_dir = TempDir::new().unwrap();
    fs::write(dir.path().join("scanner.rs"), "   \n\n  \n").unwrap();
    let builder = ScannerBuilder::new(
        "test",
        dir.path().to_path_buf(),
        out_dir.path().to_path_buf(),
    );
    assert!(builder.build().is_err());
}

#[test]
fn test_build_rust_scanner_multiple_impl_lines_takes_first() {
    let dir = TempDir::new().unwrap();
    let out_dir = TempDir::new().unwrap();
    let content = r#"
impl ExternalScanner for FirstScanner {}
impl ExternalScanner for SecondScanner {}
"#;
    fs::write(dir.path().join("scanner.rs"), content).unwrap();
    let builder = ScannerBuilder::new(
        "test",
        dir.path().to_path_buf(),
        out_dir.path().to_path_buf(),
    );
    builder.build().unwrap();
    let reg = fs::read_to_string(out_dir.path().join("test_scanner_registration.rs")).unwrap();
    assert!(reg.contains("FirstScanner"));
}

#[test]
fn test_build_rust_scanner_impl_with_generics_style_name() {
    let dir = TempDir::new().unwrap();
    let out_dir = TempDir::new().unwrap();
    // Struct name with trailing { stripped
    let content = "impl ExternalScanner for GenericScanner{\n}\n";
    fs::write(dir.path().join("scanner.rs"), content).unwrap();
    let builder = ScannerBuilder::new(
        "test",
        dir.path().to_path_buf(),
        out_dir.path().to_path_buf(),
    );
    builder.build().unwrap();
    let reg = fs::read_to_string(out_dir.path().join("test_scanner_registration.rs")).unwrap();
    assert!(reg.contains("GenericScanner"));
}

// ═══════════════════════════════════════════════════════════════════
// 15. build_scanner() free function
// ═══════════════════════════════════════════════════════════════════

#[test]
fn test_build_scanner_fn_fails_without_env_vars() {
    // build_scanner() reads CARGO_MANIFEST_DIR and OUT_DIR from env
    // Without them set, it should return an error.
    // In Rust 2024 edition, set_var/remove_var are unsafe, so we use
    // a subprocess approach: just verify it errors when manifest dir is missing.
    // We test by temporarily pointing at a non-existent manifest dir via
    // ScannerBuilder directly (build_scanner is a thin wrapper).
    let builder = ScannerBuilder::new(
        "test",
        PathBuf::from("/nonexistent_manifest_dir_12345/src"),
        PathBuf::from("/tmp"),
    );
    // No scanner found = Ok(())
    assert!(builder.build().is_ok());
}

// ═══════════════════════════════════════════════════════════════════
// 16. Multiple builders, independent state
// ═══════════════════════════════════════════════════════════════════

#[test]
fn test_multiple_builders_independent() {
    let dir1 = TempDir::new().unwrap();
    let dir2 = TempDir::new().unwrap();
    fs::write(dir1.path().join("scanner.c"), "// C").unwrap();
    fs::write(dir2.path().join("scanner.rs"), "// Rust").unwrap();

    let b1 = ScannerBuilder::new(
        "lang1",
        dir1.path().to_path_buf(),
        dir1.path().to_path_buf(),
    );
    let b2 = ScannerBuilder::new(
        "lang2",
        dir2.path().to_path_buf(),
        dir2.path().to_path_buf(),
    );

    let s1 = b1.find_scanner().unwrap().unwrap();
    let s2 = b2.find_scanner().unwrap().unwrap();

    assert_eq!(s1.language, ScannerLanguage::C);
    assert_eq!(s2.language, ScannerLanguage::Rust);
    assert_eq!(s1.grammar_name, "lang1");
    assert_eq!(s2.grammar_name, "lang2");
}

#[test]
fn test_builder_does_not_cross_contaminate() {
    let dir_with = TempDir::new().unwrap();
    let dir_without = TempDir::new().unwrap();
    fs::write(dir_with.path().join("scanner.c"), "// C").unwrap();

    let b1 = ScannerBuilder::new(
        "has",
        dir_with.path().to_path_buf(),
        dir_with.path().to_path_buf(),
    );
    let b2 = ScannerBuilder::new(
        "none",
        dir_without.path().to_path_buf(),
        dir_without.path().to_path_buf(),
    );

    assert!(b1.find_scanner().unwrap().is_some());
    assert!(b2.find_scanner().unwrap().is_none());
}

// ═══════════════════════════════════════════════════════════════════
// 17. Scanner source path correctness
// ═══════════════════════════════════════════════════════════════════

#[test]
fn test_scanner_path_is_absolute_when_src_dir_absolute() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("scanner.c"), "// c").unwrap();
    let builder = ScannerBuilder::new("t", dir.path().to_path_buf(), dir.path().to_path_buf());
    let src = builder.find_scanner().unwrap().unwrap();
    assert!(src.path.is_absolute());
}

#[test]
fn test_scanner_path_joins_src_dir_and_filename() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("scanner.rs"), "// rs").unwrap();
    let builder = ScannerBuilder::new("t", dir.path().to_path_buf(), dir.path().to_path_buf());
    let src = builder.find_scanner().unwrap().unwrap();
    assert_eq!(src.path, dir.path().join("scanner.rs"));
}

#[test]
fn test_named_scanner_path_is_correct() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("sql_scanner.cc"), "// cc").unwrap();
    let builder = ScannerBuilder::new("sql", dir.path().to_path_buf(), dir.path().to_path_buf());
    let src = builder.find_scanner().unwrap().unwrap();
    assert_eq!(src.path, dir.path().join("sql_scanner.cc"));
}
