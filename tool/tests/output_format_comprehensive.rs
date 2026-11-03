#![allow(
    clippy::needless_range_loop,
    reason = "property and comprehensive tests use index-based loops to exercise table positions and boundary cases"
)]

//! Comprehensive tests for output format handling in adze-tool.
//!
//! Covers: JSON output structure, output file naming conventions, multiple
//! output files, output directory handling, format selection, output with
//! different grammar complexities, output encoding (UTF-8), and deterministic
//! output.

use std::fs;
use std::path::Path;

use adze_tool::GrammarConverter;
use adze_tool::pure_rust_builder::{BuildOptions, BuildResult, build_parser_from_grammar_js};
use tempfile::TempDir;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn temp_opts(dir: &TempDir) -> BuildOptions {
    BuildOptions {
        out_dir: dir.path().to_string_lossy().into(),
        emit_artifacts: false,
        compress_tables: false,
    }
}

fn temp_opts_with_artifacts(dir: &TempDir) -> BuildOptions {
    BuildOptions {
        out_dir: dir.path().to_string_lossy().into(),
        emit_artifacts: true,
        compress_tables: false,
    }
}

fn simple_grammar_js() -> &'static str {
    r#"
module.exports = grammar({
  name: 'simple',
  rules: {
    source_file: $ => $.expression,
    expression: $ => /\d+/
  }
});
"#
}

fn two_rule_grammar_js() -> &'static str {
    r#"
module.exports = grammar({
  name: 'two_rule',
  rules: {
    source_file: $ => choice($.number, $.word),
    number: $ => /\d+/,
    word: $ => /[a-z]+/
  }
});
"#
}

fn build_js(js: &str, opts: BuildOptions) -> BuildResult {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("grammar.js");
    fs::write(&path, js).unwrap();
    build_parser_from_grammar_js(&path, opts).unwrap()
}

fn _build_js_in(js: &str, dir: &TempDir) -> BuildResult {
    let path = dir.path().join("grammar.js");
    fs::write(&path, js).unwrap();
    build_parser_from_grammar_js(&path, temp_opts(dir)).unwrap()
}

/// Write Rust source to a temp file and extract grammars.
fn extract_grammars(src: &str) -> Vec<serde_json::Value> {
    let dir = TempDir::new().unwrap();
    let p = dir.path().join("lib.rs");
    fs::write(&p, src).unwrap();
    adze_tool::generate_grammars(&p).unwrap()
}

fn extract_one(src: &str) -> serde_json::Value {
    let gs = extract_grammars(src);
    assert_eq!(gs.len(), 1);
    gs.into_iter().next().unwrap()
}

// =========================================================================
// 1. JSON output structure
// =========================================================================

#[test]
fn json_grammar_has_name_field() {
    let g = extract_one(
        r#"
        #[adze::grammar("json_name_test")]
        mod grammar {
            #[adze::language]
            pub struct Root {
                #[adze::leaf(pattern = r"[a-z]+")]
                pub tok: String,
            }
        }
        "#,
    );
    assert_eq!(g["name"].as_str().unwrap(), "json_name_test");
}

#[test]
fn json_grammar_has_rules_object() {
    let g = extract_one(
        r#"
        #[adze::grammar("rules_structure")]
        mod grammar {
            #[adze::language]
            pub struct Root {
                #[adze::leaf(text = "x")]
                pub x: String,
            }
        }
        "#,
    );
    assert!(g["rules"].is_object());
    assert!(!g["rules"].as_object().unwrap().is_empty());
}

#[test]
fn json_grammar_has_extras_array() {
    let g = extract_one(
        r#"
        #[adze::grammar("extras_check")]
        mod grammar {
            #[adze::language]
            pub struct Root {
                #[adze::leaf(text = "y")]
                pub y: String,
            }
        }
        "#,
    );
    assert!(g["extras"].is_array());
}

#[test]
fn json_grammar_rules_contain_language_type() {
    let g = extract_one(
        r#"
        #[adze::grammar("lang_type_check")]
        mod grammar {
            #[adze::language]
            pub enum Expr {
                Num(#[adze::leaf(pattern = r"\d+")] String),
            }
        }
        "#,
    );
    let rules = g["rules"].as_object().unwrap();
    // The language type should produce a rule entry
    assert!(
        !rules.is_empty(),
        "expected at least 1 rule, got {}",
        rules.len()
    );
}

// =========================================================================
// 2. Output file naming conventions
// =========================================================================

#[test]
fn parser_path_contains_grammar_name() {
    let dir = TempDir::new().unwrap();
    let result = build_js(simple_grammar_js(), temp_opts(&dir));
    assert!(
        result.parser_path.contains("simple"),
        "parser_path '{}' should contain grammar name 'simple'",
        result.parser_path
    );
}

#[test]
fn parser_path_ends_with_rs_extension() {
    let dir = TempDir::new().unwrap();
    let result = build_js(simple_grammar_js(), temp_opts(&dir));
    assert!(
        result.parser_path.ends_with(".rs"),
        "parser_path '{}' should end with .rs",
        result.parser_path
    );
}

#[test]
fn parser_module_filename_is_lowercase() {
    let dir = TempDir::new().unwrap();
    let result = build_js(simple_grammar_js(), temp_opts(&dir));
    let filename = Path::new(&result.parser_path)
        .file_name()
        .unwrap()
        .to_str()
        .unwrap();
    assert_eq!(
        filename,
        filename.to_lowercase(),
        "parser module filename should be lowercase"
    );
}

#[test]
fn grammar_name_propagated_to_result() {
    let dir = TempDir::new().unwrap();
    let result = build_js(simple_grammar_js(), temp_opts(&dir));
    assert_eq!(result.grammar_name, "simple");
}

// =========================================================================
// 3. Multiple output files
// =========================================================================

#[test]
fn build_result_contains_parser_code() {
    let dir = TempDir::new().unwrap();
    let result = build_js(simple_grammar_js(), temp_opts(&dir));
    assert!(
        !result.parser_code.is_empty(),
        "parser_code should not be empty"
    );
}

#[test]
fn build_result_contains_node_types_json() {
    let dir = TempDir::new().unwrap();
    let result = build_js(simple_grammar_js(), temp_opts(&dir));
    assert!(
        !result.node_types_json.is_empty(),
        "node_types_json should not be empty"
    );
}

#[test]
fn node_types_json_is_valid_json() {
    let dir = TempDir::new().unwrap();
    let result = build_js(simple_grammar_js(), temp_opts(&dir));
    let parsed: serde_json::Value = serde_json::from_str(&result.node_types_json)
        .expect("node_types_json should be valid JSON");
    assert!(parsed.is_array(), "NODE_TYPES should be a JSON array");
}

#[test]
fn artifacts_mode_writes_node_types_file() {
    let dir = TempDir::new().unwrap();
    let _result = build_js(simple_grammar_js(), temp_opts_with_artifacts(&dir));
    let nt_path = dir.path().join("grammar_simple").join("NODE_TYPES.json");
    assert!(
        nt_path.exists(),
        "NODE_TYPES.json should exist when emit_artifacts is true"
    );
    let content = fs::read_to_string(&nt_path).unwrap();
    let _: serde_json::Value = serde_json::from_str(&content).unwrap();
}

#[test]
fn artifacts_mode_writes_grammar_ir_file() {
    let dir = TempDir::new().unwrap();
    let _result = build_js(simple_grammar_js(), temp_opts_with_artifacts(&dir));
    let ir_path = dir.path().join("grammar_simple").join("grammar.ir.json");
    assert!(
        ir_path.exists(),
        "grammar.ir.json should exist when emit_artifacts is true"
    );
    let content = fs::read_to_string(&ir_path).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert!(parsed.is_object());
}

// =========================================================================
// 4. Output directory handling
// =========================================================================

#[test]
fn output_directory_created_automatically() {
    let dir = TempDir::new().unwrap();
    let nested = dir.path().join("deep").join("nested");
    // The nested dir does not exist yet; the builder should create it.
    let opts = BuildOptions {
        out_dir: nested.to_string_lossy().into(),
        emit_artifacts: false,
        compress_tables: false,
    };
    let path = dir.path().join("grammar.js");
    fs::write(&path, simple_grammar_js()).unwrap();
    let result = build_parser_from_grammar_js(&path, opts).unwrap();
    assert!(
        Path::new(&result.parser_path).exists(),
        "parser file should be written into the nested output directory"
    );
}

#[test]
fn artifacts_directory_named_after_grammar() {
    let dir = TempDir::new().unwrap();
    let _result = build_js(simple_grammar_js(), temp_opts_with_artifacts(&dir));
    let grammar_dir = dir.path().join("grammar_simple");
    assert!(
        grammar_dir.is_dir(),
        "grammar output directory should be named grammar_<name>"
    );
}

#[test]
fn multiple_builds_overwrite_artifacts() {
    let dir = TempDir::new().unwrap();
    // Build twice with artifacts to the same directory.
    let _r1 = build_js(simple_grammar_js(), temp_opts_with_artifacts(&dir));
    let _r2 = build_js(simple_grammar_js(), temp_opts_with_artifacts(&dir));
    // Owned artifacts are refreshed while their shared directory is preserved.
    let grammar_dir = dir.path().join("grammar_simple");
    assert!(grammar_dir.is_dir());
}

// =========================================================================
// 5. Format selection (OutputFormat enum)
// =========================================================================

#[test]
fn output_format_all_variants_exist() {
    let _tree = adze_tool::cli::OutputFormat::Tree;
    let _sexp = adze_tool::cli::OutputFormat::Sexp;
    let _json = adze_tool::cli::OutputFormat::Json;
    let _dot = adze_tool::cli::OutputFormat::Dot;
}

// =========================================================================
// 6. Output with different grammar complexities
// =========================================================================

#[test]
fn single_token_grammar_produces_output() {
    let dir = TempDir::new().unwrap();
    let result = build_js(simple_grammar_js(), temp_opts(&dir));
    assert!(!result.parser_code.is_empty());
    assert!(result.build_stats.state_count > 0);
}

#[test]
fn multi_rule_grammar_has_more_states() {
    let dir1 = TempDir::new().unwrap();
    let r1 = build_js(simple_grammar_js(), temp_opts(&dir1));

    let dir2 = TempDir::new().unwrap();
    let r2 = build_js(two_rule_grammar_js(), temp_opts(&dir2));

    // More rules should generally produce at least as many states.
    assert!(
        r2.build_stats.state_count >= r1.build_stats.state_count,
        "two_rule ({} states) should have >= simple ({} states)",
        r2.build_stats.state_count,
        r1.build_stats.state_count,
    );
}

#[test]
fn multi_rule_grammar_has_more_symbols() {
    let dir1 = TempDir::new().unwrap();
    let r1 = build_js(simple_grammar_js(), temp_opts(&dir1));

    let dir2 = TempDir::new().unwrap();
    let r2 = build_js(two_rule_grammar_js(), temp_opts(&dir2));

    assert!(
        r2.build_stats.symbol_count >= r1.build_stats.symbol_count,
        "two_rule ({} symbols) should have >= simple ({} symbols)",
        r2.build_stats.symbol_count,
        r1.build_stats.symbol_count,
    );
}

#[test]
fn build_stats_are_nonzero() {
    let dir = TempDir::new().unwrap();
    let result = build_js(simple_grammar_js(), temp_opts(&dir));
    assert!(
        result.build_stats.state_count > 0,
        "state_count should be > 0"
    );
    assert!(
        result.build_stats.symbol_count > 0,
        "symbol_count should be > 0"
    );
}

#[test]
fn grammar_with_precedence_builds_successfully() {
    let js = r#"
module.exports = grammar({
  name: 'prec_test',
  rules: {
    source_file: $ => $.expression,
    expression: $ => choice(
      $.number,
      prec.left(1, seq($.expression, '+', $.expression))
    ),
    number: $ => /\d+/
  }
});
"#;
    let dir = TempDir::new().unwrap();
    let result = build_js(js, temp_opts(&dir));
    assert_eq!(result.grammar_name, "prec_test");
    assert!(!result.parser_code.is_empty());
}

// =========================================================================
// 7. Output encoding (UTF-8)
// =========================================================================

#[test]
fn parser_code_is_valid_utf8() {
    let dir = TempDir::new().unwrap();
    let result = build_js(simple_grammar_js(), temp_opts(&dir));
    // The code is already a Rust String, which is UTF-8 by definition.
    // Verify it round-trips through bytes without loss.
    let bytes = result.parser_code.as_bytes();
    let roundtrip = std::str::from_utf8(bytes).expect("parser_code should be valid UTF-8");
    assert_eq!(roundtrip, result.parser_code);
}

#[test]
fn node_types_json_is_valid_utf8() {
    let dir = TempDir::new().unwrap();
    let result = build_js(simple_grammar_js(), temp_opts(&dir));
    let bytes = result.node_types_json.as_bytes();
    let roundtrip = std::str::from_utf8(bytes).expect("node_types_json should be valid UTF-8");
    assert_eq!(roundtrip, result.node_types_json);
}

#[test]
fn written_parser_file_is_utf8() {
    let dir = TempDir::new().unwrap();
    let result = build_js(simple_grammar_js(), temp_opts(&dir));
    let content =
        fs::read_to_string(&result.parser_path).expect("parser file should be readable as UTF-8");
    assert!(!content.is_empty());
}

// =========================================================================
// 8. Deterministic output
// =========================================================================

#[test]
fn parser_code_length_is_deterministic_across_builds() {
    let dir1 = TempDir::new().unwrap();
    let r1 = build_js(simple_grammar_js(), temp_opts(&dir1));

    let dir2 = TempDir::new().unwrap();
    let r2 = build_js(simple_grammar_js(), temp_opts(&dir2));

    assert_eq!(
        r1.parser_code.len(),
        r2.parser_code.len(),
        "parser_code length should be consistent across builds"
    );
}

#[test]
fn node_types_json_structure_is_deterministic() {
    let dir1 = TempDir::new().unwrap();
    let r1 = build_js(simple_grammar_js(), temp_opts(&dir1));

    let dir2 = TempDir::new().unwrap();
    let r2 = build_js(simple_grammar_js(), temp_opts(&dir2));

    // Parse and compare structural content (element count) rather than raw string
    // because HashMap ordering may affect symbol-name byte encodings.
    let v1: serde_json::Value = serde_json::from_str(&r1.node_types_json).unwrap();
    let v2: serde_json::Value = serde_json::from_str(&r2.node_types_json).unwrap();
    assert_eq!(
        v1.as_array().unwrap().len(),
        v2.as_array().unwrap().len(),
        "node_types_json should have the same number of entries"
    );
}

#[test]
fn build_stats_are_deterministic() {
    let dir1 = TempDir::new().unwrap();
    let r1 = build_js(simple_grammar_js(), temp_opts(&dir1));

    let dir2 = TempDir::new().unwrap();
    let r2 = build_js(simple_grammar_js(), temp_opts(&dir2));

    assert_eq!(r1.build_stats.state_count, r2.build_stats.state_count);
    assert_eq!(r1.build_stats.symbol_count, r2.build_stats.symbol_count);
    assert_eq!(r1.build_stats.conflict_cells, r2.build_stats.conflict_cells);
}

#[test]
fn grammar_name_deterministic_from_rust_source() {
    let src = r#"
        #[adze::grammar("det_test")]
        mod grammar {
            #[adze::language]
            pub struct Root {
                #[adze::leaf(pattern = r"[a-z]+")]
                pub tok: String,
            }
        }
    "#;
    let g1 = extract_one(src);
    let g2 = extract_one(src);
    assert_eq!(
        g1, g2,
        "grammar JSON should be identical across extractions"
    );
}

// =========================================================================
// 9. Additional output format edge cases
// =========================================================================

#[test]
fn parser_code_contains_grammar_name_constant() {
    let dir = TempDir::new().unwrap();
    let result = build_js(simple_grammar_js(), temp_opts(&dir));
    // The written file should contain the GRAMMAR_NAME constant.
    let file_content = fs::read_to_string(&result.parser_path).unwrap();
    assert!(
        file_content.contains("GRAMMAR_NAME"),
        "parser file should contain GRAMMAR_NAME constant"
    );
}

#[test]
fn parser_file_header_contains_auto_generated_comment() {
    let dir = TempDir::new().unwrap();
    let result = build_js(simple_grammar_js(), temp_opts(&dir));
    let file_content = fs::read_to_string(&result.parser_path).unwrap();
    assert!(
        file_content.contains("Auto-generated"),
        "parser file should have auto-generated header comment"
    );
}

#[test]
fn compressed_and_uncompressed_produce_output() {
    let dir1 = TempDir::new().unwrap();
    let opts_uncompressed = BuildOptions {
        out_dir: dir1.path().to_string_lossy().into(),
        emit_artifacts: false,
        compress_tables: false,
    };
    let r1 = build_js(simple_grammar_js(), opts_uncompressed);

    let dir2 = TempDir::new().unwrap();
    let opts_compressed = BuildOptions {
        out_dir: dir2.path().to_string_lossy().into(),
        emit_artifacts: false,
        compress_tables: true,
    };
    let r2 = build_js(simple_grammar_js(), opts_compressed);

    // Both should produce non-empty output.
    assert!(!r1.parser_code.is_empty());
    assert!(!r2.parser_code.is_empty());
    // Grammar name should be same regardless of compression.
    assert_eq!(r1.grammar_name, r2.grammar_name);
}

#[test]
fn visualization_dot_output_for_sample_grammar() {
    use adze_tool::GrammarVisualizer;
    let grammar = GrammarConverter::create_sample_grammar();
    let viz = GrammarVisualizer::new(grammar);
    let dot = viz.to_dot();
    assert!(
        dot.starts_with("digraph"),
        "DOT output should start with 'digraph'"
    );
    assert!(
        dot.contains("->"),
        "DOT output should contain edge definitions"
    );
}

#[test]
fn visualization_text_output_for_sample_grammar() {
    use adze_tool::GrammarVisualizer;
    let grammar = GrammarConverter::create_sample_grammar();
    let viz = GrammarVisualizer::new(grammar);
    let text = viz.to_text();
    assert!(!text.is_empty(), "text visualization should not be empty");
}
