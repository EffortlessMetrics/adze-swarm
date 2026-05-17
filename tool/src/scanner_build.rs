// Build system integration for external scanners
//! Build system integration for discovering and compiling external scanners.

// This module provides functionality to discover and compile user-provided scanner implementations

use anyhow::{Context, Result, bail};
use std::fs;
use std::path::{Path, PathBuf};

/// Scanner source file information
#[derive(Debug, Clone)]
pub struct ScannerSource {
    /// Path to the scanner source file
    pub path: PathBuf,
    /// Language of the scanner (C, C++, or Rust)
    pub language: ScannerLanguage,
    /// Name of the grammar this scanner belongs to
    pub grammar_name: String,
}

/// Supported scanner implementation languages
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScannerLanguage {
    C,
    Cpp,
    Rust,
}

impl ScannerLanguage {
    /// Get the canonical file extension for this language.
    pub fn extension(&self) -> &'static str {
        match self {
            ScannerLanguage::C => "c",
            ScannerLanguage::Cpp => "cc",
            ScannerLanguage::Rust => "rs",
        }
    }

    /// Infer a scanner language from a source path.
    fn from_path(path: &Path) -> Option<Self> {
        match path.extension().and_then(|s| s.to_str()) {
            Some("c") => Some(ScannerLanguage::C),
            Some("cc" | "cpp") => Some(ScannerLanguage::Cpp),
            Some("rs") => Some(ScannerLanguage::Rust),
            _ => None,
        }
    }
}

/// Scanner builder configuration
pub struct ScannerBuilder {
    /// Grammar name
    grammar_name: String,
    /// Source directory to search for scanner files
    src_dir: PathBuf,
    /// Output directory for compiled scanner
    out_dir: PathBuf,
}

impl ScannerBuilder {
    /// Create a new scanner builder
    pub fn new(grammar_name: impl Into<String>, src_dir: PathBuf, out_dir: PathBuf) -> Self {
        ScannerBuilder {
            grammar_name: grammar_name.into(),
            src_dir,
            out_dir,
        }
    }

    /// Find scanner source file in the source directory
    pub fn find_scanner(&self) -> Result<Option<ScannerSource>> {
        const CANONICAL_SCANNER_NAMES: &[&str] =
            &["scanner.c", "scanner.cc", "scanner.cpp", "scanner.rs"];

        let prefixed_scanner_names = [
            format!("{}_scanner.c", self.grammar_name),
            format!("{}_scanner.cc", self.grammar_name),
            format!("{}_scanner.cpp", self.grammar_name),
            format!("{}_scanner.rs", self.grammar_name),
        ];

        for name in CANONICAL_SCANNER_NAMES
            .iter()
            .copied()
            .chain(prefixed_scanner_names.iter().map(String::as_str))
        {
            let path = self.src_dir.join(name);
            if !path.exists() {
                continue;
            }

            let Some(language) = ScannerLanguage::from_path(&path) else {
                continue;
            };

            return Ok(Some(ScannerSource {
                path,
                language,
                grammar_name: self.grammar_name.clone(),
            }));
        }

        Ok(None)
    }

    /// Build the scanner and generate integration code
    pub fn build(&self) -> Result<()> {
        let scanner = match self.find_scanner()? {
            Some(scanner) => scanner,
            None => {
                // No scanner found - that's OK, not all grammars need external scanners
                return Ok(());
            }
        };

        println!("cargo:rerun-if-changed={}", scanner.path.display());

        match scanner.language {
            ScannerLanguage::C | ScannerLanguage::Cpp => {
                self.build_c_scanner(&scanner)?;
            }
            ScannerLanguage::Rust => {
                self.build_rust_scanner(&scanner)?;
            }
        }

        Ok(())
    }

    /// Build a C/C++ scanner
    fn build_c_scanner(&self, scanner: &ScannerSource) -> Result<()> {
        // Use cc crate to compile the scanner
        let mut build = cc::Build::new();

        build
            .file(&scanner.path)
            .include(&self.src_dir)
            .warnings(false);

        if scanner.language == ScannerLanguage::Cpp {
            build.cpp(true);
        }

        // Set output name based on grammar
        let lib_name = format!("{}_scanner", self.grammar_name);
        build.compile(&lib_name);

        // Generate Rust bindings
        self.generate_c_bindings(scanner)?;

        Ok(())
    }

    /// Generate Rust bindings for C scanner
    fn generate_c_bindings(&self, _scanner: &ScannerSource) -> Result<()> {
        let bindings_path = self
            .out_dir
            .join(format!("{}_scanner_bindings.rs", self.grammar_name));

        let bindings = format!(
            r#"
// Auto-generated bindings for {} scanner
use adze::external_scanner_ffi::{{TSExternalScannerData, CreateFn, DestroyFn, ScanFn, SerializeFn, DeserializeFn}};

extern "C" {{
    fn tree_sitter_{}_external_scanner_create() -> *mut std::ffi::c_void;
    fn tree_sitter_{}_external_scanner_destroy(payload: *mut std::ffi::c_void);
    fn tree_sitter_{}_external_scanner_scan(
        payload: *mut std::ffi::c_void,
        lexer: *mut adze::external_scanner_ffi::TSLexer,
        valid_symbols: *const bool,
    ) -> bool;
    fn tree_sitter_{}_external_scanner_serialize(
        payload: *mut std::ffi::c_void,
        buffer: *mut std::os::raw::c_char,
    ) -> std::os::raw::c_uint;
    fn tree_sitter_{}_external_scanner_deserialize(
        payload: *mut std::ffi::c_void,
        buffer: *const std::os::raw::c_char,
        length: std::os::raw::c_uint,
    );
}}

/// Get the external scanner data for this grammar
pub fn get_external_scanner_data() -> TSExternalScannerData {{
    TSExternalScannerData {{
        states: std::ptr::null(),
        symbol_map: std::ptr::null(),
        create: Some(tree_sitter_{}_external_scanner_create as CreateFn),
        destroy: Some(tree_sitter_{}_external_scanner_destroy as DestroyFn),
        scan: Some(tree_sitter_{}_external_scanner_scan as ScanFn),
        serialize: Some(tree_sitter_{}_external_scanner_serialize as SerializeFn),
        deserialize: Some(tree_sitter_{}_external_scanner_deserialize as DeserializeFn),
    }}
}}

/// Register this scanner with the global registry
pub fn register_scanner(external_tokens: Vec<adze::SymbolId>) {{
    let data = get_external_scanner_data();
    adze::scanner_registry::register_c_scanner(
        "{}",
        data,
        external_tokens,
    );
}}
"#,
            self.grammar_name,
            self.grammar_name,
            self.grammar_name,
            self.grammar_name,
            self.grammar_name,
            self.grammar_name,
            self.grammar_name,
            self.grammar_name,
            self.grammar_name,
            self.grammar_name,
            self.grammar_name,
            self.grammar_name
        );

        fs::write(&bindings_path, bindings)
            .with_context(|| format!("Failed to write scanner bindings to {:?}", bindings_path))?;

        println!(
            "cargo:rustc-env=ADZE_SCANNER_BINDINGS_{}={}",
            self.grammar_name.to_uppercase(),
            bindings_path.display()
        );

        Ok(())
    }

    /// Build a Rust scanner
    fn build_rust_scanner(&self, scanner: &ScannerSource) -> Result<()> {
        // For Rust scanners, generate code to register them
        let registration_path = self
            .out_dir
            .join(format!("{}_scanner_registration.rs", self.grammar_name));

        // Read the scanner file to extract the scanner struct name
        let scanner_content = fs::read_to_string(&scanner.path)
            .with_context(|| format!("Failed to read scanner file {:?}", scanner.path))?;

        // Simple heuristic to find the scanner struct name
        let scanner_struct = self.find_scanner_struct(&scanner_content)?;

        let registration = format!(
            r#"
// Auto-generated registration for {} Rust scanner
use adze::scanner_registry::ExternalScannerBuilder;

include!({:?});

/// Register this scanner with the global registry
pub fn register_scanner() {{
    ExternalScannerBuilder::new("{}")
        .register_rust::<{}>();
}}
"#,
            self.grammar_name,
            scanner.path.display(),
            self.grammar_name,
            scanner_struct
        );

        fs::write(&registration_path, registration).with_context(|| {
            format!(
                "Failed to write scanner registration to {:?}",
                registration_path
            )
        })?;

        Ok(())
    }

    /// Find the scanner struct name in Rust code
    fn find_scanner_struct(&self, content: &str) -> Result<String> {
        // Look for "impl ExternalScanner for StructName"
        for line in content.lines() {
            if line.contains("impl ExternalScanner for") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 4 {
                    return Ok(parts[3].trim_end_matches('{').to_string());
                }
            }
        }

        // Fallback: look for struct definitions with "Scanner" in the name
        for line in content.lines() {
            if line.trim().starts_with("pub struct") && line.contains("Scanner") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 3 {
                    return Ok(parts[2].trim_end_matches('{').to_string());
                }
            }
        }

        bail!("Could not find scanner struct in {:?}", self.src_dir)
    }
}

/// Helper function to build scanners in build.rs
pub fn build_scanner(grammar_name: &str) -> Result<()> {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").context("CARGO_MANIFEST_DIR not set")?;
    let out_dir = std::env::var("OUT_DIR").context("OUT_DIR not set")?;

    let src_dir = Path::new(&manifest_dir).join("src");
    let out_dir = PathBuf::from(out_dir);

    let builder = ScannerBuilder::new(grammar_name, src_dir, out_dir);
    builder.build()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_find_scanner() {
        let temp_dir = TempDir::new().unwrap();
        let src_dir = temp_dir.path().to_path_buf();

        // Create a test scanner file
        fs::write(src_dir.join("scanner.c"), "// test scanner").unwrap();

        let builder = ScannerBuilder::new("test", src_dir, PathBuf::new());
        let scanner = builder.find_scanner().unwrap().unwrap();

        assert_eq!(scanner.language, ScannerLanguage::C);
        assert_eq!(scanner.grammar_name, "test");
    }

    #[test]
    fn test_find_scanner_struct() {
        let builder = ScannerBuilder::new("test", PathBuf::new(), PathBuf::new());

        let content = r#"
pub struct MyScanner {
    state: u32,
}

impl ExternalScanner for MyScanner {
    // implementation
}
"#;

        let struct_name = builder.find_scanner_struct(content).unwrap();
        assert_eq!(struct_name, "MyScanner");
    }

    #[test]
    fn scanner_language_extension_matches_variant() {
        assert_eq!(ScannerLanguage::C.extension(), "c");
        assert_eq!(ScannerLanguage::Cpp.extension(), "cc");
        assert_eq!(ScannerLanguage::Rust.extension(), "rs");
    }

    #[test]
    fn scanner_language_from_path_accepts_supported_extensions() {
        assert_eq!(
            ScannerLanguage::from_path(Path::new("scanner.c")),
            Some(ScannerLanguage::C)
        );
        assert_eq!(
            ScannerLanguage::from_path(Path::new("scanner.cc")),
            Some(ScannerLanguage::Cpp)
        );
        assert_eq!(
            ScannerLanguage::from_path(Path::new("scanner.cpp")),
            Some(ScannerLanguage::Cpp)
        );
        assert_eq!(
            ScannerLanguage::from_path(Path::new("scanner.rs")),
            Some(ScannerLanguage::Rust)
        );
        assert_eq!(ScannerLanguage::from_path(Path::new("scanner.txt")), None);
    }

    #[test]
    fn scanner_language_is_copy_and_eq() {
        let l = ScannerLanguage::C;
        let copied = l;
        assert_eq!(l, copied);
        assert_ne!(ScannerLanguage::C, ScannerLanguage::Cpp);
        assert_ne!(ScannerLanguage::Cpp, ScannerLanguage::Rust);
        // Debug formatting works
        assert!(!format!("{:?}", ScannerLanguage::Cpp).is_empty());
    }

    #[test]
    fn find_scanner_returns_none_when_missing() {
        let temp_dir = TempDir::new().unwrap();
        let builder = ScannerBuilder::new("test", temp_dir.path().to_path_buf(), PathBuf::new());
        assert!(builder.find_scanner().unwrap().is_none());
    }

    #[test]
    fn find_scanner_detects_cpp_via_cc_extension() {
        let temp_dir = TempDir::new().unwrap();
        let src_dir = temp_dir.path().to_path_buf();
        fs::write(src_dir.join("scanner.cc"), "// cc scanner").unwrap();

        let builder = ScannerBuilder::new("g", src_dir, PathBuf::new());
        let scanner = builder.find_scanner().unwrap().unwrap();
        assert_eq!(scanner.language, ScannerLanguage::Cpp);
        assert_eq!(scanner.grammar_name, "g");
        assert!(scanner.path.ends_with("scanner.cc"));
    }

    #[test]
    fn find_scanner_detects_cpp_via_cpp_extension() {
        let temp_dir = TempDir::new().unwrap();
        let src_dir = temp_dir.path().to_path_buf();
        fs::write(src_dir.join("scanner.cpp"), "// cpp scanner").unwrap();

        let builder = ScannerBuilder::new("g", src_dir, PathBuf::new());
        let scanner = builder.find_scanner().unwrap().unwrap();
        assert_eq!(scanner.language, ScannerLanguage::Cpp);
    }

    #[test]
    fn find_scanner_detects_rust() {
        let temp_dir = TempDir::new().unwrap();
        let src_dir = temp_dir.path().to_path_buf();
        fs::write(src_dir.join("scanner.rs"), "// rust scanner").unwrap();

        let builder = ScannerBuilder::new("g", src_dir, PathBuf::new());
        let scanner = builder.find_scanner().unwrap().unwrap();
        assert_eq!(scanner.language, ScannerLanguage::Rust);
    }

    #[test]
    fn find_scanner_detects_grammar_prefixed_name() {
        let temp_dir = TempDir::new().unwrap();
        let src_dir = temp_dir.path().to_path_buf();
        fs::write(src_dir.join("python_scanner.c"), "// python scanner").unwrap();

        let builder = ScannerBuilder::new("python", src_dir, PathBuf::new());
        let scanner = builder.find_scanner().unwrap().unwrap();
        assert_eq!(scanner.language, ScannerLanguage::C);
        assert_eq!(scanner.grammar_name, "python");
        assert!(scanner.path.ends_with("python_scanner.c"));
    }

    #[test]
    fn find_scanner_detects_grammar_prefixed_cpp_name() {
        let temp_dir = TempDir::new().unwrap();
        let src_dir = temp_dir.path().to_path_buf();
        fs::write(src_dir.join("python_scanner.cpp"), "// python scanner").unwrap();

        let builder = ScannerBuilder::new("python", src_dir, PathBuf::new());
        let scanner = builder.find_scanner().unwrap().unwrap();
        assert_eq!(scanner.language, ScannerLanguage::Cpp);
        assert_eq!(scanner.grammar_name, "python");
        assert!(scanner.path.ends_with("python_scanner.cpp"));
    }

    #[test]
    fn find_scanner_prefers_canonical_name_over_prefixed() {
        let temp_dir = TempDir::new().unwrap();
        let src_dir = temp_dir.path().to_path_buf();
        // Both present — canonical scanner.c wins (it is searched first).
        fs::write(src_dir.join("scanner.c"), "// canonical").unwrap();
        fs::write(src_dir.join("py_scanner.c"), "// prefixed").unwrap();

        let builder = ScannerBuilder::new("py", src_dir, PathBuf::new());
        let scanner = builder.find_scanner().unwrap().unwrap();
        assert!(scanner.path.ends_with("scanner.c"));
    }

    #[test]
    fn find_scanner_source_is_clone() {
        // Exercise derive(Clone, Debug) on ScannerSource.
        let temp_dir = TempDir::new().unwrap();
        let src_dir = temp_dir.path().to_path_buf();
        fs::write(src_dir.join("scanner.c"), "").unwrap();

        let builder = ScannerBuilder::new("g", src_dir, PathBuf::new());
        let scanner = builder.find_scanner().unwrap().unwrap();
        let cloned = scanner.clone();
        assert_eq!(cloned.grammar_name, scanner.grammar_name);
        assert_eq!(cloned.language, scanner.language);
        assert!(!format!("{:?}", scanner).is_empty());
    }

    #[test]
    fn find_scanner_struct_falls_back_to_pub_struct_with_scanner() {
        let builder = ScannerBuilder::new("g", PathBuf::new(), PathBuf::new());
        // No `impl ExternalScanner for ...` line, so the fallback branch runs.
        let content = r#"
// Some preamble

pub struct CustomScanner {
    foo: u32,
}
"#;
        let name = builder.find_scanner_struct(content).unwrap();
        assert_eq!(name, "CustomScanner");
    }

    #[test]
    fn find_scanner_struct_strips_trailing_brace() {
        let builder = ScannerBuilder::new("g", PathBuf::new(), PathBuf::new());
        // No whitespace before the opening brace exercises trim_end_matches.
        let content = "impl ExternalScanner for TightScanner{ }\n";
        let name = builder.find_scanner_struct(content).unwrap();
        assert_eq!(name, "TightScanner");
    }

    #[test]
    fn find_scanner_struct_errors_when_missing() {
        let builder = ScannerBuilder::new("g", PathBuf::new(), PathBuf::new());
        let err = builder
            .find_scanner_struct("// no scanner here\nfn unrelated() {}\n")
            .unwrap_err();
        let msg = format!("{:?}", err);
        assert!(msg.contains("Could not find scanner struct"));
    }

    #[test]
    fn find_scanner_struct_ignores_non_scanner_pub_struct() {
        let builder = ScannerBuilder::new("g", PathBuf::new(), PathBuf::new());
        // `pub struct Foo` without "Scanner" in the name should NOT match the
        // fallback branch; with no `impl ExternalScanner` line either, this errors.
        let content = "pub struct Foo {}\n";
        assert!(builder.find_scanner_struct(content).is_err());
    }
}
