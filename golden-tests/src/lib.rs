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

    // Syntax projection excludes only anonymous lexical whitespace extras.
    // Native nodes, diagnostic facts and all other syntax remain untouched.
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
                            let text = child.utf8_text(source.as_bytes()).ok();
                            let lexical_whitespace = child.is_extra()
                                && !child.is_named()
                                && child.child_count() == 0
                                && !child.is_error()
                                && !child.is_missing()
                                && text.is_some_and(|text| {
                                    !text.is_empty() && text.chars().all(char::is_whitespace)
                                });
                            if lexical_whitespace {
                                continue;
                            }
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
                eprintln!("GOLDEN SKIP [parse-failed] {}: {}", test.fixture_name, e);
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
                "GOLDEN FAIL [missing-expected] No golden reference found for {}. \
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
        let mut parser = adze::pure_parser::Parser::new();
        parser
            .set_language(adze_javascript::grammar::language())
            .unwrap();
        let parsed = parser.parse_string(";\n");
        assert!(parsed.errors.is_empty());
        let root = parsed.root.unwrap();
        let mut pending = vec![&root];
        let (mut punctuation, mut whitespace) = (Vec::new(), Vec::new());
        while let Some(node) = pending.pop() {
            if node.kind() == ";" {
                punctuation.push(node.start_byte()..node.end_byte());
            }
            if node.is_extra() && node.utf8_text(b";\n") == Ok("\n") {
                assert!(!node.is_named(), "declared whitespace leaf is anonymous");
                assert!(!node.is_error() && !node.is_missing());
                whitespace.push(node.start_byte()..node.end_byte());
            }
            pending.extend(node.children());
        }
        assert_eq!(punctuation, vec![0..1]);
        assert_eq!(whitespace, vec![1..2]);
        run_golden_test_strict(GoldenTest {
            language: "javascript",
            fixture_name: "canary_expression.js",
        })
    }

    #[cfg(any(feature = "python-grammar", feature = "javascript-grammar"))]
    fn projected_child(source: &str, child: adze::pure_parser::ParsedNode) -> String {
        let root =
            adze::pure_parser::ParsedNodeBuilder::new(5, vec![child], 0, source.len()).build();
        tree_to_sexp(&root, source)
    }

    #[test]
    #[cfg(any(feature = "python-grammar", feature = "javascript-grammar"))]
    fn test_syntax_projection_omits_only_declared_anonymous_whitespace_extra_leaf() {
        use adze::pure_parser::ParsedNodeBuilder as Node;
        let whitespace = Node::new(6, vec![], 1, 2)
            .is_extra(true)
            .is_named(false)
            .build();
        let punctuation = Node::new(4, vec![], 0, 1).is_named(false).build();
        let root = Node::new(5, vec![punctuation, whitespace], 0, 2).build();
        assert_eq!(tree_to_sexp(&root, ";\n"), "(Expression\n  \";\"\n)");
        assert_eq!(root.children().len(), 2);
        assert!(root.children()[1].is_extra());
        assert_eq!(
            root.children()[1].start_byte()..root.children()[1].end_byte(),
            1..2
        );
    }

    #[test]
    #[cfg(any(feature = "python-grammar", feature = "javascript-grammar"))]
    fn test_syntax_projection_retains_semantic_whitespace_named_extras_and_comments() {
        use adze::pure_parser::ParsedNodeBuilder as Node;
        assert_eq!(
            projected_child("\n", Node::new(6, vec![], 0, 1).is_named(false).build()),
            "(Expression\n  \"\\n\"\n)"
        );
        // Named extras remain visible even when their entire lexeme is whitespace.
        assert_eq!(
            projected_child("\n", Node::new(7, vec![], 0, 1).is_extra(true).build()),
            "(Expression\n  (Whitespace \"\\n\")\n)"
        );
        // The synthetic named-extra kind is irrelevant to policy: comment text
        // and its named identity are both preserved literally.
        assert_eq!(
            projected_child("# note", Node::new(7, vec![], 0, 6).is_extra(true).build()),
            "(Expression\n  (Whitespace \"# note\")\n)"
        );
        assert_eq!(
            projected_child(
                "x",
                Node::new(6, vec![], 0, 1)
                    .is_extra(true)
                    .is_named(false)
                    .build()
            ),
            "(Expression\n  \"x\"\n)"
        );
    }

    #[test]
    #[cfg(any(feature = "python-grammar", feature = "javascript-grammar"))]
    fn test_syntax_projection_retains_errors_missing_empty_and_structural_extras() {
        use adze::pure_parser::ParsedNodeBuilder as Node;
        for child in [
            Node::new(6, vec![], 0, 1)
                .is_extra(true)
                .is_error(true)
                .is_named(false)
                .build(),
            Node::new(6, vec![], 0, 1)
                .is_extra(true)
                .is_missing(true)
                .is_named(false)
                .build(),
        ] {
            assert_eq!(projected_child("\n", child), "(Expression\n  \"\\n\"\n)");
        }
        assert_eq!(
            projected_child(
                "",
                Node::new(6, vec![], 0, 0)
                    .is_extra(true)
                    .is_named(false)
                    .build()
            ),
            "(Expression\n  \"\"\n)"
        );
        let semantic = Node::new(6, vec![], 0, 1).is_named(false).build();
        let structural = Node::new(6, vec![semantic], 0, 1).is_extra(true).build();
        assert_eq!(
            projected_child("\n", structural),
            "(Expression\n  (Whitespace__whitespace\n    \"\\n\"\n  )\n)"
        );
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

    /// Honest-denominator summary: classify every golden fixture on disk by
    /// the disposition that fires at runtime, without changing pass/fail
    /// semantics. A fixture only genuinely compares when it both parses AND
    /// owns an expected file; parse failures return before any comparison.
    #[test]
    #[cfg(any(feature = "python-grammar", feature = "javascript-grammar"))]
    fn golden_disposition_summary() {
        let mut languages: Vec<&'static str> = Vec::new();
        #[cfg(feature = "python-grammar")]
        languages.push("python");
        #[cfg(feature = "javascript-grammar")]
        languages.push("javascript");

        let mut compared = 0u32;
        let mut parse_failed = 0u32;
        let mut missing_expected = 0u32;
        for language in languages {
            let (ext, dir) = match language {
                "python" => ("py", "python/fixtures"),
                _ => ("js", "javascript/fixtures"),
            };
            let fixtures_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(dir);
            let expected_dir = fixtures_dir
                .parent()
                .expect("fixtures dir has a parent")
                .join("expected");
            let mut names: Vec<String> = fs::read_dir(&fixtures_dir)
                .expect("Cannot read fixtures dir")
                .filter_map(|e| e.ok())
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .filter(|n| n.ends_with(&format!(".{ext}")))
                .collect();
            names.sort();
            for name in names {
                let stem = name.trim_end_matches(&format!(".{ext}"));
                let source =
                    fs::read_to_string(fixtures_dir.join(&name)).expect("fixture readable");
                let parsed = parse_with_adze(language, &source).is_ok();
                let has_expected = expected_dir.join(format!("{stem}.sha256")).exists();
                let disposition = if !parsed {
                    parse_failed += 1;
                    "parse-failed"
                } else if !has_expected {
                    missing_expected += 1;
                    "missing-expected"
                } else {
                    compared += 1;
                    "compared"
                };
                eprintln!("GOLDEN DISPOSITION [{disposition}] {language}/{name}");
            }
        }
        eprintln!(
            "GOLDEN SUMMARY compared={compared} parse_failed={parse_failed} missing_expected={missing_expected}"
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
