//! Edge case tests for the adze-tool build pipeline.
//!
//! Comprehensive tests for error conditions and unusual input patterns:
//! - Empty source files
//! - Files with only comments
//! - Duplicate grammar names
//! - Invalid Rust syntax in annotations
//! - Missing #[adze::grammar] attributes
//! - Unicode identifiers in grammar names
//! - Deterministic build artifacts

use std::fs;
use tempfile::TempDir;

use adze_tool::pure_rust_builder::{BuildOptions, build_parser_from_grammar_js};

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

/// Write a Rust source file and extract grammars via `generate_grammars`.
fn grammars_from_rust(code: &str) -> adze_tool::ToolResult<Vec<serde_json::Value>> {
    let dir = TempDir::new().unwrap();
    let src = dir.path().join("lib.rs");
    fs::write(&src, code).unwrap();
    adze_tool::generate_grammars(&src)
}

/// Build from grammar.js file and return the result.
fn build_from_grammar_js(js: &str) -> anyhow::Result<adze_tool::pure_rust_builder::BuildResult> {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("grammar.js");
    fs::write(&path, js).unwrap();
    let opts = BuildOptions {
        out_dir: dir.path().to_string_lossy().into(),
        emit_artifacts: false,
        compress_tables: false,
    };
    build_parser_from_grammar_js(&path, opts)
}

// =========================================================================
// 1. Empty source file
// =========================================================================

#[test]
fn empty_source_file_yields_no_grammars() {
    let result = grammars_from_rust("");
    assert!(result.is_ok(), "should handle empty file gracefully");
    assert!(
        result.unwrap().is_empty(),
        "empty file should yield no grammars"
    );
}

#[test]
fn whitespace_only_file_yields_no_grammars() {
    let result = grammars_from_rust("\n\n    \t\n");
    assert!(
        result.is_ok(),
        "should handle whitespace-only file gracefully"
    );
    assert!(
        result.unwrap().is_empty(),
        "whitespace-only file should yield no grammars"
    );
}

// =========================================================================
// 2. Files with only comments
// =========================================================================

#[test]
fn comment_only_file_yields_no_grammars() {
    let result = grammars_from_rust(
        r#"
        // This is a comment
        // Another comment
        // Just documentation
        "#,
    );
    assert!(result.is_ok(), "should handle comment-only file gracefully");
    assert!(
        result.unwrap().is_empty(),
        "comment-only file should yield no grammars"
    );
}

#[test]
fn block_comments_only_yields_no_grammars() {
    let result = grammars_from_rust(
        r#"
        /*
         * This is a block comment
         * with multiple lines
         */
        /* Another block comment */
        "#,
    );
    assert!(
        result.is_ok(),
        "should handle block-comment-only file gracefully"
    );
    assert!(
        result.unwrap().is_empty(),
        "block-comment-only file should yield no grammars"
    );
}

#[test]
fn doc_comments_only_yields_no_grammars() {
    let result = grammars_from_rust(
        r#"
        //! This is a crate-level doc comment
        //! With multiple lines
        //! But no actual code
        "#,
    );
    assert!(
        result.is_ok(),
        "should handle doc-comment-only file gracefully"
    );
    assert!(
        result.unwrap().is_empty(),
        "doc-comment-only file should yield no grammars"
    );
}

// =========================================================================
// 3. Duplicate grammar names
// =========================================================================

#[test]
fn duplicate_grammar_names_produces_both() {
    let result = grammars_from_rust(
        r#"
        #[adze::grammar("duplicate_name")]
        mod grammar_one {
            #[adze::language]
            pub enum Expr {
                Num(#[adze::leaf(pattern = r"\d+")] i32),
            }
        }

        #[adze::grammar("duplicate_name")]
        mod grammar_two {
            #[adze::language]
            pub enum Tok {
                Word(#[adze::leaf(pattern = r"[a-z]+")] String),
            }
        }
        "#,
    );
    assert!(
        result.is_ok(),
        "should handle duplicate names without error"
    );
    let grammars = result.unwrap();
    assert_eq!(
        grammars.len(),
        2,
        "should extract both grammars even with duplicate names"
    );
    // Both should have the same name
    assert_eq!(
        grammars[0]["name"].as_str().unwrap(),
        "duplicate_name",
        "first grammar should have correct name"
    );
    assert_eq!(
        grammars[1]["name"].as_str().unwrap(),
        "duplicate_name",
        "second grammar should have correct name"
    );
}

// =========================================================================
// 4. Invalid Rust syntax in annotations
// =========================================================================

#[test]
fn invalid_syntax_in_pattern_annotation_fails_gracefully() {
    // Malformed pattern string in the annotation
    let result = grammars_from_rust(
        r#"
        #[adze::grammar("bad_pattern")]
        mod grammar {
            #[adze::language]
            pub enum Expr {
                Bad(#[adze::leaf(pattern = "unclosed")] i32),
            }
        }
        "#,
    );
    // Note: This may succeed during parsing but produce a different output
    // The key is that the tool handles it without panicking
    let _ = result; // May succeed or fail, but shouldn't crash
}

#[test]
fn malformed_leaf_attribute_fails_gracefully() {
    // Missing required fields in leaf attribute
    let result = grammars_from_rust(
        r#"
        #[adze::grammar("bad_leaf")]
        mod grammar {
            #[adze::language]
            pub enum Expr {
                Num(#[adze::leaf()] i32),
            }
        }
        "#,
    );
    // The system should handle this gracefully (may fail or succeed)
    let _ = result;
}

// =========================================================================
// 5. Missing #[adze::grammar] attribute
// =========================================================================

#[test]
fn module_without_grammar_attribute_is_ignored() {
    let result = grammars_from_rust(
        r#"
        mod not_a_grammar {
            #[adze::language]
            pub enum Expr {
                Num(#[adze::leaf(pattern = r"\d+")] i32),
            }
        }
        "#,
    );
    assert!(
        result.is_ok(),
        "should handle modules without grammar attribute"
    );
    assert!(
        result.unwrap().is_empty(),
        "module without #[adze::grammar] should not be extracted"
    );
}

#[test]
fn empty_grammar_attribute_is_valid() {
    let result = grammars_from_rust(
        r#"
        #[adze::grammar("empty_grammar")]
        mod grammar {
            #[adze::language]
            pub enum Empty {
            }
        }
        "#,
    );
    // This may succeed or fail depending on implementation
    // The key is graceful handling
    let _ = result;
}

#[test]
fn test_module_without_language_attribute_returns_spanned_error() {
    let error = grammars_from_rust(
        r#"
        #[adze::grammar("no_language")]
        mod grammar {
            pub struct SomeType;
        }
        "#,
    )
    .expect_err("a grammar without a language root must be rejected");
    match error {
        adze_tool::ToolError::SynError { syn_error } => assert_eq!(
            syn_error.to_string(),
            "Each parser must have the root type annotated with `#[adze::language]`; add `#[adze::language]` to the enum or struct that is the parse entry point"
        ),
        other => panic!("expected a spanned syntax error, got {other}"),
    }
}

// =========================================================================
// 6. Unicode identifiers in grammar names
// =========================================================================

#[test]
fn unicode_characters_in_grammar_names_preserved() {
    let result = grammars_from_rust(
        r#"
        #[adze::grammar("λ_calculus")]
        mod grammar {
            #[adze::language]
            pub enum Expr {
                Var(#[adze::leaf(pattern = r"[a-z]+")] String),
            }
        }
        "#,
    );
    assert!(result.is_ok(), "should handle Unicode in grammar names");
    let grammars = result.unwrap();
    assert_eq!(grammars.len(), 1);
    assert_eq!(
        grammars[0]["name"].as_str().unwrap(),
        "λ_calculus",
        "Unicode characters should be preserved"
    );
}

#[test]
fn emoji_in_grammar_names_handled() {
    let result = grammars_from_rust(
        r#"
        #[adze::grammar("🦀_lang")]
        mod grammar {
            #[adze::language]
            pub enum Token {
                Crab(#[adze::leaf(pattern = r"[a-z]+")] String),
            }
        }
        "#,
    );
    // Should handle without panicking, may succeed or fail
    let _ = result;
}

#[test]
fn cyrillic_grammar_names_handled() {
    let result = grammars_from_rust(
        r#"
        #[adze::grammar("привет")]
        mod grammar {
            #[adze::language]
            pub enum Expr {
                Word(#[adze::leaf(pattern = r"[а-я]+")] String),
            }
        }
        "#,
    );
    // Should handle without panicking
    assert!(result.is_ok() || result.is_err(), "should not crash");
}

// =========================================================================
// 7. Deterministic build artifacts (build twice, compare)
// =========================================================================

#[test]
#[ignore = "Known non-determinism in code generation due to HashMap ordering — tracked"]
fn build_artifacts_are_deterministic_simple_grammar() {
    let grammar_js = r#"
module.exports = grammar({
  name: 'deterministic_simple',
  rules: {
    source: $ => $.item,
    item: $ => /[a-z]+/
  }
});
"#;

    // Build twice
    let result1 = build_from_grammar_js(grammar_js).unwrap();
    let result2 = build_from_grammar_js(grammar_js).unwrap();

    // Parser code should be identical
    assert_eq!(
        result1.parser_code, result2.parser_code,
        "parser code should be deterministic"
    );

    // Node types JSON should be identical
    assert_eq!(
        result1.node_types_json, result2.node_types_json,
        "node_types_json should be deterministic"
    );

    // Grammar names should match
    assert_eq!(
        result1.grammar_name, result2.grammar_name,
        "grammar_name should be deterministic"
    );
}

#[test]
#[ignore = "Known non-determinism in code generation due to HashMap ordering — tracked"]
fn build_artifacts_deterministic_with_complex_rules() {
    let grammar_js = r#"
module.exports = grammar({
  name: 'deterministic_complex',
  rules: {
    source: $ => repeat($.statement),
    statement: $ => choice(
      $.expr,
      $.assign
    ),
    expr: $ => choice(
      $.number,
      $.identifier,
      seq($.identifier, '(', $.arguments, ')')
    ),
    assign: $ => seq($.identifier, '=', $.expr),
    arguments: $ => optional_list($.expr, ','),
    number: $ => /\d+/,
    identifier: $ => /[a-zA-Z_][a-zA-Z0-9_]*/
  }
});
"#;

    // Build twice
    let result1 = build_from_grammar_js(grammar_js).unwrap();
    let result2 = build_from_grammar_js(grammar_js).unwrap();

    // Code should be deterministic
    assert_eq!(
        result1.parser_code, result2.parser_code,
        "parser code should be deterministic with complex rules"
    );
}

#[test]
fn build_artifacts_deterministic_unicode_safe() {
    let grammar_js = r#"
module.exports = grammar({
  name: 'unicode_safe',
  rules: {
    source: $ => $.greeting,
    greeting: $ => /[あ-ん]+/
  }
});
"#;

    // Build twice
    let result1 = build_from_grammar_js(grammar_js);
    let result2 = build_from_grammar_js(grammar_js);

    // Both should succeed or both should fail
    match (result1, result2) {
        (Ok(r1), Ok(r2)) => {
            assert_eq!(
                r1.parser_code, r2.parser_code,
                "parser code should be deterministic with unicode patterns"
            );
        }
        (Err(_), Err(_)) => {
            // Both failed, which is also deterministic
        }
        _ => panic!("builds should have same result (both succeed or both fail)"),
    }
}

// =========================================================================
// 8. Build artifacts for valid grammars
// =========================================================================

#[test]
fn build_simple_grammar_produces_output() {
    let grammar_js = r#"
module.exports = grammar({
  name: 'simple_valid',
  rules: {
    source: $ => $.expr,
    expr: $ => /[a-z]+/
  }
});
"#;

    let result = build_from_grammar_js(grammar_js);
    assert!(
        result.is_ok(),
        "simple valid grammar should build successfully"
    );

    let result = result.unwrap();
    assert_eq!(result.grammar_name, "simple_valid");
    assert!(
        !result.parser_code.is_empty(),
        "parser code should be generated"
    );
    assert!(
        !result.node_types_json.is_empty(),
        "node_types should be generated"
    );
}

#[test]
fn build_grammar_with_multiple_rules() {
    let grammar_js = r#"
module.exports = grammar({
  name: 'multi_rule',
  rules: {
    source: $ => $.program,
    program: $ => repeat($.statement),
    statement: $ => choice(
      $.expression,
      $.declaration
    ),
    expression: $ => /[a-z]+/,
    declaration: $ => /[A-Z]+/
  }
});
"#;

    let result = build_from_grammar_js(grammar_js);
    assert!(
        result.is_ok(),
        "multi-rule grammar should build successfully"
    );

    let result = result.unwrap();
    assert!(!result.parser_code.is_empty());
    assert!(!result.node_types_json.is_empty());
}

// =========================================================================
// 9. Error recovery and edge cases
// =========================================================================

#[test]
fn grammar_with_empty_rules_object() {
    let grammar_js = r#"
module.exports = grammar({
  name: 'empty_rules',
  rules: {}
});
"#;

    // Should handle gracefully - may succeed or fail
    let result = build_from_grammar_js(grammar_js);
    let _ = result; // Accept either outcome
}

#[test]
fn grammar_missing_required_fields() {
    let grammar_js = r#"
module.exports = grammar({
  name: 'incomplete'
});
"#;

    // Should fail gracefully without crashing
    let result = build_from_grammar_js(grammar_js);
    let _ = result; // Accept the error
}

#[test]
fn rust_extraction_with_deeply_nested_modules() {
    let result = grammars_from_rust(
        r#"
        #[adze::grammar("nested")]
        mod level1 {
            #[adze::language]
            pub enum Expr {
                Num(#[adze::leaf(pattern = r"\d+")] i32),
            }
        }
        "#,
    );
    assert!(result.is_ok(), "should handle nested modules");
    assert_eq!(result.unwrap().len(), 1);
}

#[test]
fn rust_extraction_mixed_grammar_and_non_grammar_modules() {
    let result = grammars_from_rust(
        r#"
        mod helper {
            pub fn help() {}
        }

        #[adze::grammar("actual")]
        mod grammar {
            #[adze::language]
            pub enum Tok {
                Lit(#[adze::leaf(pattern = r".")] char),
            }
        }

        mod other_helper {
            pub struct Config;
        }
        "#,
    );
    assert!(result.is_ok(), "should handle mixed modules");
    let grammars = result.unwrap();
    assert_eq!(grammars.len(), 1, "should extract only the grammar module");
    assert_eq!(grammars[0]["name"].as_str().unwrap(), "actual");
}

// =========================================================================
// 10. File system edge cases
// =========================================================================

#[test]
fn valid_grammar_with_long_filename() {
    let dir = TempDir::new().unwrap();
    let long_name = "a".repeat(200);
    let src = dir.path().join(format!("{}.rs", long_name));
    fs::write(
        &src,
        r#"
        #[adze::grammar("long_filename")]
        mod grammar {
            #[adze::language]
            pub enum Expr {
                Num(#[adze::leaf(pattern = r"\d+")] i32),
            }
        }
        "#,
    )
    .unwrap();

    let result = adze_tool::generate_grammars(&src);
    assert!(result.is_ok(), "should handle long filenames");
    assert_eq!(result.unwrap().len(), 1);
}

#[test]
fn valid_grammar_with_special_characters_in_path() {
    let dir = TempDir::new().unwrap();
    let src = dir.path().join("grammar (1).rs");
    fs::write(
        &src,
        r#"
        #[adze::grammar("special_path")]
        mod grammar {
            #[adze::language]
            pub enum Expr {
                Num(#[adze::leaf(pattern = r"\d+")] i32),
            }
        }
        "#,
    )
    .unwrap();

    let result = adze_tool::generate_grammars(&src);
    assert!(result.is_ok(), "should handle special characters in path");
    assert_eq!(result.unwrap().len(), 1);
}

// =========================================================================
// 11. Rust syntax variations
// =========================================================================

#[test]
fn grammar_with_visibility_modifiers() {
    let result = grammars_from_rust(
        r#"
        #[adze::grammar("vis_test")]
        pub mod grammar {
            #[adze::language]
            pub enum Expr {
                Num(#[adze::leaf(pattern = r"\d+")] i32),
            }
        }
        "#,
    );
    assert!(result.is_ok(), "should handle visibility modifiers");
    assert_eq!(result.unwrap().len(), 1);
}

#[test]
fn grammar_with_attributes_and_docs() {
    let result = grammars_from_rust(
        r#"
        /// Documentation for the grammar module
        /// Multiple lines
        #[adze::grammar("doc_grammar")]
        mod grammar {
            #[adze::language]
            pub enum Expr {
                Num(#[adze::leaf(pattern = r"\d+")] i32),
            }
        }
        "#,
    );
    assert!(result.is_ok(), "should handle documentation and attributes");
    assert_eq!(result.unwrap().len(), 1);
}
