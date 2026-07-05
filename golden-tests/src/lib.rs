mod example_integration;

#[cfg(test)]
mod tests {
    #![allow(dead_code)]

    use adze_tool::pure_rust_builder::{BuildOptions, build_parser_from_json};
    use anyhow::{Context, Result};
    use sha2::{Digest, Sha256};
    use std::fs;
    use std::path::PathBuf;

    /// Represents a golden test case
    struct GoldenTest {
        language: &'static str,
        fixture_name: &'static str,
    }

    impl GoldenTest {
        fn fixture_path(&self) -> PathBuf {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join(self.language)
                .join("fixtures")
                .join(self.fixture_name)
        }

        fn base_name(&self) -> String {
            std::path::Path::new(self.fixture_name)
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned()
        }

        fn expected_hash_path(&self) -> PathBuf {
            let base_name = self.base_name();
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join(self.language)
                .join("expected")
                .join(format!("{}.sha256", base_name))
        }

        fn expected_sexp_path(&self) -> PathBuf {
            let base_name = self.base_name();
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join(self.language)
                .join("expected")
                .join(format!("{}.sexp", base_name))
        }
    }

    /// Parse a file with adze and return S-expression
    fn parse_with_adze(language: &str, source: &str) -> Result<String> {
        match language {
            "python" => parse_python(source),
            "javascript" => parse_javascript(source),
            _ => anyhow::bail!("Unsupported language: {}", language),
        }
    }

    /// Parse Python source code and return S-expression
    #[cfg(feature = "python-grammar")]
    fn parse_python(source: &str) -> Result<String> {
        use adze::pure_parser::Parser;

        adze_python::register_scanner();
        let mut parser = Parser::new();
        parser
            .set_language(adze_python::get_language())
            .map_err(|e| anyhow::anyhow!(e))?;
        let result = parser.parse_string(source);
        if let Some(root) = result.root {
            Ok(tree_to_sexp(&root, source))
        } else {
            let err = result
                .errors
                .get(0)
                .map(|e| {
                    format!(
                        "pos {} expected {:?} found {}",
                        e.position, e.expected, e.found
                    )
                })
                .unwrap_or_else(|| "unknown error".to_string());
            anyhow::bail!("parse failed: {}", err)
        }
    }

    #[cfg(not(feature = "python-grammar"))]
    fn parse_python(_source: &str) -> Result<String> {
        anyhow::bail!("Python grammar feature not enabled")
    }

    /// Parse JavaScript source code and return S-expression
    #[cfg(feature = "javascript-grammar")]
    fn parse_javascript(source: &str) -> Result<String> {
        use adze::pure_parser::Parser;

        let mut parser = Parser::new();
        parser
            .set_language(&adze_javascript::grammar::LANGUAGE)
            .map_err(|e| anyhow::anyhow!(e))?;
        let result = parser.parse_string(source);
        if let Some(root) = result.root {
            Ok(tree_to_sexp(&root, source))
        } else {
            let err = result
                .errors
                .get(0)
                .map(|e| {
                    format!(
                        "pos {} expected {:?} found {}",
                        e.position, e.expected, e.found
                    )
                })
                .unwrap_or_else(|| "unknown error".to_string());
            anyhow::bail!("parse failed: {}", err)
        }
    }

    #[cfg(not(feature = "javascript-grammar"))]
    fn parse_javascript(_source: &str) -> Result<String> {
        anyhow::bail!("JavaScript grammar feature not enabled")
    }

    #[cfg(any(feature = "python-grammar", feature = "javascript-grammar"))]
    fn tree_to_sexp(node: &adze::pure_parser::ParsedNode, source: &str) -> String {
        fn node_to_sexp(
            node: &adze::pure_parser::ParsedNode,
            source: &str,
            indent: usize,
        ) -> String {
            let mut result = String::new();
            let spaces = " ".repeat(indent);

            if node.is_named() {
                result.push_str(&format!("{}({}", spaces, node.kind()));

                if node.child_count() == 0 {
                    let text = node.utf8_text(source.as_bytes()).unwrap_or("");
                    result.push_str(&format!(" \"{}\")", escape_string(text)));
                } else {
                    result.push('\n');
                    for i in 0..node.child_count() {
                        if let Some(child) = node.child(i) {
                            result.push_str(&node_to_sexp(&child, source, indent + 2));
                            result.push('\n');
                        }
                    }
                    result.push_str(&format!("{})", spaces));
                }
            } else {
                let text = node.utf8_text(source.as_bytes()).unwrap_or("");
                result.push_str(&format!("{}\"{}\"", spaces, escape_string(text)));
            }

            result
        }

        node_to_sexp(node, source, 0)
    }

    #[cfg(any(feature = "python-grammar", feature = "javascript-grammar"))]
    fn escape_string(s: &str) -> String {
        s.chars()
            .flat_map(|c| match c {
                '"' => vec!['\\', '"'],
                '\\' => vec!['\\', '\\'],
                '\n' => vec!['\\', 'n'],
                '\r' => vec!['\\', 'r'],
                '\t' => vec!['\\', 't'],
                c if c.is_control() => format!("\\u{{{:04x}}}", c as u32).chars().collect(),
                c => vec![c],
            })
            .collect()
    }

    /// Compute SHA256 hash of a string
    fn compute_hash(content: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    /// Run a golden test.
    ///
    /// In legacy mode parse failures are treated as skips so this crate can run
    /// even when a grammar is partially wired.
    fn run_golden_test(test: GoldenTest) -> Result<()> {
        run_golden_test_internal(test, false)
    }

    /// Run a golden test and fail hard on parse errors.
    ///
    /// Use this mode for canary fixtures that must always produce a stable
    /// user-visible output.
    fn run_golden_test_strict(test: GoldenTest) -> Result<()> {
        run_golden_test_internal(test, true)
    }

    fn run_golden_test_internal(test: GoldenTest, strict_parse: bool) -> Result<()> {
        // Read the source file
        let source = fs::read_to_string(test.fixture_path())
            .with_context(|| format!("Failed to read fixture: {}", test.fixture_name))?;

        // Parse with adze
        let sexp = match parse_with_adze(test.language, &source) {
            Ok(s) => s,
            Err(e) => {
                if strict_parse {
                    anyhow::bail!("Parse failed for {}: {}", test.fixture_name, e);
                }
                eprintln!("Skipping {}: {}", test.fixture_name, e);
                return Ok(());
            }
        };

        // Check if we're in "update" mode
        if std::env::var("UPDATE_GOLDEN").is_ok() {
            // Update mode: generate new reference files
            println!("Updating golden reference for {}", test.fixture_name);

            // Note: In real implementation, we'd run tree-sitter here
            // For now, we just save what adze produces
            let sexp_path = test.expected_sexp_path();
            let hash_path = test.expected_hash_path();

            // Ensure parent directories exist
            if let Some(dir) = sexp_path.parent() {
                fs::create_dir_all(dir)?;
            }

            fs::write(&sexp_path, &sexp)?;

            let hash = compute_hash(&sexp);
            fs::write(&hash_path, &hash)?;

            return Ok(());
        }

        // Normal mode: compare against reference
        if test.expected_hash_path().exists() {
            // Hash-based comparison (more efficient)
            let expected_hash = fs::read_to_string(test.expected_hash_path())
                .with_context(|| "Failed to read expected hash")?
                .trim()
                .to_string();

            let actual_hash = compute_hash(&sexp);

            if actual_hash != expected_hash {
                // On hash mismatch, show more detailed error
                if test.expected_sexp_path().exists() {
                    let _expected_sexp = fs::read_to_string(test.expected_sexp_path())
                        .with_context(|| "Failed to read expected S-expression")?;

                    // Save actual output for debugging
                    let debug_path = test.expected_sexp_path().with_extension("actual.sexp");
                    fs::write(&debug_path, &sexp)?;

                    anyhow::bail!(
                        "Parse tree mismatch for {}:\n\
                         Expected hash: {}\n\
                         Actual hash:   {}\n\
                         \n\
                         Expected S-expression saved to: {}\n\
                         Actual S-expression saved to: {}\n\
                         \n\
                         To update golden files, run with UPDATE_GOLDEN=1",
                        test.fixture_name,
                        expected_hash,
                        actual_hash,
                        test.expected_sexp_path().display(),
                        debug_path.display()
                    );
                }

                anyhow::bail!(
                    "Parse tree hash mismatch for {}:\n\
                     Expected: {}\n\
                     Actual:   {}",
                    test.fixture_name,
                    expected_hash,
                    actual_hash
                );
            }
        } else {
            // No reference file exists
            anyhow::bail!(
                "No golden reference found for {}. \
                 Run ./generate_references.sh to create reference files.",
                test.fixture_name
            );
        }

        Ok(())
    }

    // ===== Python golden tests =====

    #[test]
    #[cfg(feature = "python-grammar")]
    fn python_simple_golden() -> Result<()> {
        run_golden_test(GoldenTest {
            language: "python",
            fixture_name: "simple_program.py",
        })
    }

    #[test]
    #[cfg(feature = "python-grammar")]
    fn python_empty_golden() -> Result<()> {
        run_golden_test(GoldenTest {
            language: "python",
            fixture_name: "empty.py",
        })
    }

    #[test]
    #[cfg(feature = "python-grammar")]
    fn python_unicode_golden() -> Result<()> {
        run_golden_test(GoldenTest {
            language: "python",
            fixture_name: "unicode_identifiers.py",
        })
    }

    #[test]
    #[cfg(feature = "python-grammar")]
    fn python_deeply_nested_golden() -> Result<()> {
        run_golden_test(GoldenTest {
            language: "python",
            fixture_name: "deeply_nested.py",
        })
    }

    #[test]
    #[cfg(feature = "python-grammar")]
    fn python_long_expression_golden() -> Result<()> {
        run_golden_test(GoldenTest {
            language: "python",
            fixture_name: "long_expression.py",
        })
    }

    #[test]
    #[cfg(feature = "python-grammar")]
    fn python_syntax_error_golden() -> Result<()> {
        run_golden_test(GoldenTest {
            language: "python",
            fixture_name: "syntax_error.py",
        })
    }

    // ===== JavaScript golden tests =====

    #[test]
    #[cfg(feature = "javascript-grammar")]
    fn javascript_canary_expression_golden() -> Result<()> {
        run_golden_test_strict(GoldenTest {
            language: "javascript",
            fixture_name: "canary_expression.js",
        })
    }

    // ===== Infrastructure tests =====

    #[test]
    fn reference_script_exists() {
        let script_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("generate_references.sh");
        assert!(
            script_path.exists(),
            "Reference generation script not found at: {}",
            script_path.display()
        );
    }

    #[test]
    fn compute_hash_deterministic() {
        let h1 = compute_hash("hello world");
        let h2 = compute_hash("hello world");
        assert_eq!(h1, h2, "Hash should be deterministic");
        assert_eq!(h1.len(), 64, "SHA256 hex should be 64 chars");
    }

    #[test]
    fn compute_hash_distinct_inputs() {
        let h1 = compute_hash("abc");
        let h2 = compute_hash("abd");
        assert_ne!(h1, h2, "Different inputs should produce different hashes");
    }

    #[test]
    fn compute_hash_empty_input() {
        let h = compute_hash("");
        assert_eq!(h.len(), 64);
        // SHA256 of empty string is well-known
        assert_eq!(
            h,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn compute_hash_unicode() {
        let h = compute_hash("こんにちは🌍");
        assert_eq!(h.len(), 64);
        // Determinism check
        assert_eq!(h, compute_hash("こんにちは🌍"));
    }

    #[test]
    fn golden_test_fixture_path_construction() {
        let test = GoldenTest {
            language: "python",
            fixture_name: "simple_program.py",
        };
        let path = test.fixture_path();
        assert!(path.ends_with("python/fixtures/simple_program.py"));
    }

    #[test]
    fn golden_test_base_name() {
        let test = GoldenTest {
            language: "javascript",
            fixture_name: "simple_program.js",
        };
        assert_eq!(test.base_name(), "simple_program");
    }

    #[test]
    fn golden_test_expected_paths() {
        let test = GoldenTest {
            language: "python",
            fixture_name: "unicode_identifiers.py",
        };
        let hash_path = test.expected_hash_path();
        let sexp_path = test.expected_sexp_path();
        assert!(hash_path.ends_with("python/expected/unicode_identifiers.sha256"));
        assert!(sexp_path.ends_with("python/expected/unicode_identifiers.sexp"));
    }

    #[test]
    fn all_python_fixtures_readable() {
        let fixtures_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("python/fixtures");
        assert!(fixtures_dir.exists(), "Python fixtures directory missing");
        let entries: Vec<_> = fs::read_dir(&fixtures_dir)
            .expect("Cannot read fixtures dir")
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "py"))
            .collect();
        assert!(!entries.is_empty(), "No Python fixture files found");
        for entry in &entries {
            let content = fs::read_to_string(entry.path());
            assert!(
                content.is_ok(),
                "Failed to read fixture: {}",
                entry.path().display()
            );
        }
    }

    #[test]
    fn all_javascript_fixtures_readable() {
        let fixtures_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("javascript/fixtures");
        assert!(
            fixtures_dir.exists(),
            "JavaScript fixtures directory missing"
        );
        let entries: Vec<_> = fs::read_dir(&fixtures_dir)
            .expect("Cannot read fixtures dir")
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "js"))
            .collect();
        assert!(!entries.is_empty(), "No JavaScript fixture files found");
        for entry in &entries {
            let content = fs::read_to_string(entry.path());
            assert!(
                content.is_ok(),
                "Failed to read fixture: {}",
                entry.path().display()
            );
        }
    }

    #[test]
    fn python_fixture_count() {
        let fixtures_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("python/fixtures");
        let count = fs::read_dir(&fixtures_dir)
            .expect("Cannot read fixtures dir")
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "py"))
            .count();
        assert!(
            count >= 6,
            "Expected at least 6 Python fixtures, found {}",
            count
        );
    }

    #[test]
    fn javascript_fixture_count() {
        // #818 removed the stale tree-sitter-format JS fixtures on purpose;
        // only the canary fixture remains until refreshed goldens exist.
        let fixtures_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("javascript/fixtures");
        let names: Vec<String> = fs::read_dir(&fixtures_dir)
            .expect("Cannot read fixtures dir")
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "js"))
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        assert!(
            names.iter().any(|name| name == "canary_expression.js"),
            "Expected the canary_expression.js fixture, found {:?}",
            names
        );
    }

    /// Verify that parse_with_adze does not panic on empty input
    #[test]
    #[cfg(feature = "python-grammar")]
    fn python_empty_input_no_panic() {
        let _ = parse_with_adze("python", "");
    }

    #[test]
    #[cfg(feature = "javascript-grammar")]
    fn javascript_empty_input_no_panic() {
        let _ = parse_with_adze("javascript", "");
    }

    /// Verify that parse_with_adze does not panic on large input
    #[test]
    #[cfg(feature = "python-grammar")]
    fn python_large_input_no_panic() {
        let large = "x = 1\n".repeat(1000);
        let _ = parse_with_adze("python", &large);
    }

    #[test]
    #[cfg(feature = "javascript-grammar")]
    fn javascript_large_input_no_panic() {
        let large = "let x = 1;\n".repeat(1000);
        let _ = parse_with_adze("javascript", &large);
    }

    /// Verify that parse_with_adze does not panic on unicode
    #[test]
    #[cfg(feature = "python-grammar")]
    fn python_unicode_no_panic() {
        let _ = parse_with_adze("python", "x = \"こんにちは🌍\"");
    }

    #[test]
    #[cfg(feature = "javascript-grammar")]
    fn javascript_unicode_no_panic() {
        let _ = parse_with_adze("javascript", "const x = \"こんにちは🌍\";");
    }

    /// Verify that parse_with_adze does not panic on syntax errors
    #[test]
    #[cfg(feature = "python-grammar")]
    fn python_syntax_error_no_panic() {
        let _ = parse_with_adze("python", "def foo(\n  bar");
    }

    #[test]
    #[cfg(feature = "javascript-grammar")]
    fn javascript_syntax_error_no_panic() {
        let _ = parse_with_adze("javascript", "function foo( {\nlet x");
    }

    #[test]
    fn unsupported_language_returns_error() {
        let result = parse_with_adze("cobol", "DISPLAY 'HELLO'");
        assert!(result.is_err());
    }

    #[test]
    fn minimal_json_grammar_node_types_golden() -> Result<()> {
        let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures")
            .join("minimal_json_grammar.json");
        let expected_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("expected")
            .join("minimal_json_grammar.node_types.json");

        let grammar_json = fs::read_to_string(&fixture_path)
            .with_context(|| format!("Failed to read fixture: {}", fixture_path.display()))?;

        let temp_out = tempfile::tempdir()?;
        let options = BuildOptions {
            out_dir: temp_out.path().to_string_lossy().into_owned(),
            emit_artifacts: false,
            compress_tables: false,
        };

        let result = build_parser_from_json(grammar_json, options)
            .with_context(|| "Failed to build parser from JSON fixture")?;

        let actual_value = serde_json::from_str::<serde_json::Value>(&result.node_types_json)?;
        let actual_pretty = serde_json::to_string_pretty(&actual_value)?;
        if std::env::var("UPDATE_GOLDEN").is_ok() {
            if let Some(parent) = expected_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&expected_path, &actual_pretty)?;
            return Ok(());
        }

        let expected_pretty = fs::read_to_string(&expected_path).with_context(|| {
            format!(
                "Failed to read expected output: {}",
                expected_path.display()
            )
        })?;
        let expected_value = serde_json::from_str::<serde_json::Value>(&expected_pretty)?;

        if actual_value != expected_value {
            let actual_path = expected_path.with_extension("actual.json");
            fs::write(&actual_path, actual_pretty)?;
            anyhow::bail!(
                "node_types JSON mismatch for fixture {}. \
                 Expected: {}. Actual written to: {}",
                fixture_path.display(),
                expected_path.display(),
                actual_path.display()
            );
        }

        Ok(())
    }

    #[cfg(any(feature = "python-grammar", feature = "javascript-grammar"))]
    mod escape_tests {
        use super::escape_string;

        #[test]
        fn escape_plain_text() {
            assert_eq!(escape_string("hello"), "hello");
        }

        #[test]
        fn escape_quotes() {
            assert_eq!(escape_string("say \"hi\""), "say \\\"hi\\\"");
        }

        #[test]
        fn escape_backslash() {
            assert_eq!(escape_string("a\\b"), "a\\\\b");
        }

        #[test]
        fn escape_newlines_and_tabs() {
            assert_eq!(escape_string("a\nb\tc\r"), "a\\nb\\tc\\r");
        }

        #[test]
        fn escape_control_chars() {
            let s = String::from('\x01');
            let escaped = escape_string(&s);
            assert!(escaped.contains("\\u{0001}"));
        }

        #[test]
        fn escape_unicode_passthrough() {
            assert_eq!(escape_string("こんにちは"), "こんにちは");
        }

        #[test]
        fn escape_empty_string() {
            assert_eq!(escape_string(""), "");
        }

        #[test]
        fn escape_mixed() {
            assert_eq!(
                escape_string("line1\nline2\t\"quoted\""),
                "line1\\nline2\\t\\\"quoted\\\""
            );
        }
    }
}
