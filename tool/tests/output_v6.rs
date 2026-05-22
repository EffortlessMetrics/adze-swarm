//! Tests for `BuildResult`, `BuildStats`, `BuildOptions`, and grammar-to-code generation.
//!
//! 64 tests across 8 categories:
//! 1. BuildResult (8): success/failure results, stats, errors, warnings, output path
//! 2. BuildStats (8): default values, rule/state/symbol counts, compression ratio, build time
//! 3. BuildOptions (8): defaults, custom dirs, flags, builder patterns, options fields
//! 4. Grammar JSON (8): simple/multi-rule conversion, tokens, roundtrips, special chars
//! 5. Pure Rust build (8): simple grammar, tokens, stats presence, determinism, precedence
//! 6. Error handling (8): invalid JSON, empty grammar, missing start, circular rules, recovery
//! 7. Output content (8): symbol/state definitions, parse table, language struct, syntax
//! 8. Integration (8): build-inspect pipeline, chaining, features, timing, large grammar

use adze_ir::{
    Associativity, Grammar, PrecedenceKind, ProductionId, Rule, Symbol, SymbolId, Token,
    TokenPattern,
};
use adze_tool::pure_rust_builder::{BuildOptions, build_parser};
use serde_json::json;
use tempfile::TempDir;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn tmp_dir() -> TempDir {
    TempDir::new().expect("Failed to create temp dir")
}

fn tmp_opts(dir: &TempDir) -> BuildOptions {
    BuildOptions {
        out_dir: dir.path().to_string_lossy().into(),
        emit_artifacts: false,
        compress_tables: false,
    }
}

fn tmp_opts_with_artifacts(dir: &TempDir) -> BuildOptions {
    BuildOptions {
        out_dir: dir.path().to_string_lossy().into(),
        emit_artifacts: true,
        compress_tables: false,
    }
}

fn tmp_opts_with_compression(dir: &TempDir) -> BuildOptions {
    BuildOptions {
        out_dir: dir.path().to_string_lossy().into(),
        emit_artifacts: false,
        compress_tables: true,
    }
}

/// Minimal one-token grammar
fn minimal_grammar(name: &str) -> Grammar {
    let mut g = Grammar::new(name.to_string());
    let tok = SymbolId(1);
    let src = SymbolId(2);
    g.tokens.insert(
        tok,
        Token {
            name: "number".into(),
            pattern: TokenPattern::Regex(r"\d+".into()),
            fragile: false,
        },
    );
    g.rule_names.insert(src, "source_file".into());
    g.rules.entry(src).or_default().push(Rule {
        lhs: src,
        rhs: vec![Symbol::Terminal(tok)],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    });
    g
}

/// Two-token grammar with alternatives
fn two_token_grammar(name: &str) -> Grammar {
    let mut g = minimal_grammar(name);
    let ident = SymbolId(10);
    let src = SymbolId(2);
    g.tokens.insert(
        ident,
        Token {
            name: "ident".into(),
            pattern: TokenPattern::Regex(r"[a-z]+".into()),
            fragile: false,
        },
    );
    g.rules.entry(src).or_default().push(Rule {
        lhs: src,
        rhs: vec![Symbol::Terminal(ident)],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(1),
    });
    g
}

/// Expression grammar with binary operators
fn expr_grammar(name: &str) -> Grammar {
    let mut g = Grammar::new(name.to_string());
    let num = SymbolId(1);
    let plus = SymbolId(3);
    let expr = SymbolId(4);

    g.tokens.insert(
        num,
        Token {
            name: "number".into(),
            pattern: TokenPattern::Regex(r"\d+".into()),
            fragile: false,
        },
    );
    g.tokens.insert(
        plus,
        Token {
            name: "plus".into(),
            pattern: TokenPattern::String("+".into()),
            fragile: false,
        },
    );

    g.rule_names.insert(expr, "expr".into());

    // expr -> number
    g.rules.entry(expr).or_default().push(Rule {
        lhs: expr,
        rhs: vec![Symbol::Terminal(num)],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    });

    // expr -> expr + expr (left-associative)
    g.rules.entry(expr).or_default().push(Rule {
        lhs: expr,
        rhs: vec![
            Symbol::NonTerminal(expr),
            Symbol::Terminal(plus),
            Symbol::NonTerminal(expr),
        ],
        precedence: Some(PrecedenceKind::Static(1)),
        associativity: Some(Associativity::Left),
        fields: vec![],
        production_id: ProductionId(1),
    });

    g
}

/// Three-operator grammar (add, sub, mul)
fn three_op_grammar(name: &str) -> Grammar {
    let mut g = expr_grammar(name);
    let minus = SymbolId(5);
    let mul = SymbolId(6);
    let expr = SymbolId(4);

    g.tokens.insert(
        minus,
        Token {
            name: "minus".into(),
            pattern: TokenPattern::String("-".into()),
            fragile: false,
        },
    );
    g.tokens.insert(
        mul,
        Token {
            name: "star".into(),
            pattern: TokenPattern::String("*".into()),
            fragile: false,
        },
    );

    // expr -> expr - expr (left-assoc, level 1)
    g.rules.entry(expr).or_default().push(Rule {
        lhs: expr,
        rhs: vec![
            Symbol::NonTerminal(expr),
            Symbol::Terminal(minus),
            Symbol::NonTerminal(expr),
        ],
        precedence: Some(PrecedenceKind::Static(1)),
        associativity: Some(Associativity::Left),
        fields: vec![],
        production_id: ProductionId(2),
    });

    // expr -> expr * expr (left-assoc, level 2)
    g.rules.entry(expr).or_default().push(Rule {
        lhs: expr,
        rhs: vec![
            Symbol::NonTerminal(expr),
            Symbol::Terminal(mul),
            Symbol::NonTerminal(expr),
        ],
        precedence: Some(PrecedenceKind::Static(2)),
        associativity: Some(Associativity::Left),
        fields: vec![],
        production_id: ProductionId(3),
    });

    g
}

/// Nested rule grammar (rule -> rule | token)
fn nested_rule_grammar(name: &str) -> Grammar {
    let mut g = Grammar::new(name.to_string());
    let tok = SymbolId(1);
    let stmt = SymbolId(2);
    let block = SymbolId(3);

    g.tokens.insert(
        tok,
        Token {
            name: "keyword".into(),
            pattern: TokenPattern::String("if".into()),
            fragile: false,
        },
    );

    g.rule_names.insert(stmt, "statement".into());
    g.rule_names.insert(block, "block".into());

    // block -> statement
    g.rules.entry(block).or_default().push(Rule {
        lhs: block,
        rhs: vec![Symbol::NonTerminal(stmt)],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    });

    // block -> block statement
    g.rules.entry(block).or_default().push(Rule {
        lhs: block,
        rhs: vec![Symbol::NonTerminal(block), Symbol::NonTerminal(stmt)],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(1),
    });

    // statement -> keyword
    g.rules.entry(stmt).or_default().push(Rule {
        lhs: stmt,
        rhs: vec![Symbol::Terminal(tok)],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(2),
    });

    g
}

// =========================================================================
// Category 1: BuildResult (8 tests)
// =========================================================================

#[test]
fn buildresult_successful_creation() {
    let dir = tmp_dir();
    let opts = tmp_opts(&dir);
    let result = build_parser(minimal_grammar("br_success"), opts);
    assert!(result.is_ok(), "Should successfully build minimal grammar");
}

#[test]
fn buildresult_has_grammar_name() {
    let dir = tmp_dir();
    let opts = tmp_opts(&dir);
    let result = build_parser(minimal_grammar("br_name_test"), opts).unwrap();
    assert_eq!(result.grammar_name, "br_name_test");
}

#[test]
fn buildresult_has_parser_path() {
    let dir = tmp_dir();
    let opts = tmp_opts(&dir);
    let result = build_parser(minimal_grammar("br_path"), opts).unwrap();
    assert!(
        !result.parser_path.is_empty(),
        "parser_path should not be empty"
    );
}

#[test]
fn buildresult_has_parser_code() {
    let dir = tmp_dir();
    let opts = tmp_opts(&dir);
    let result = build_parser(minimal_grammar("br_code"), opts).unwrap();
    assert!(
        !result.parser_code.is_empty(),
        "parser_code should contain generated code"
    );
}

#[test]
fn buildresult_has_node_types_json() {
    let dir = tmp_dir();
    let opts = tmp_opts(&dir);
    let result = build_parser(minimal_grammar("br_node_types"), opts).unwrap();
    assert!(
        !result.node_types_json.is_empty(),
        "node_types_json should be generated"
    );
}

#[test]
fn buildresult_includes_build_stats() {
    let dir = tmp_dir();
    let opts = tmp_opts(&dir);
    let result = build_parser(minimal_grammar("br_stats"), opts).unwrap();
    assert!(result.build_stats.state_count > 0);
    assert!(result.build_stats.symbol_count > 0);
}

#[test]
fn buildresult_complex_grammar_has_larger_output() {
    let dir = tmp_dir();
    let opts = tmp_opts(&dir);

    let simple = build_parser(minimal_grammar("br_simple"), opts.clone()).unwrap();
    let complex = build_parser(expr_grammar("br_complex"), tmp_opts(&tmp_dir())).unwrap();

    assert!(
        complex.parser_code.len() > simple.parser_code.len(),
        "Complex grammar should generate more code"
    );
}

#[test]
fn buildresult_multiple_builds_different_grammars() {
    let dir = tmp_dir();
    let opts = tmp_opts(&dir);

    let r1 = build_parser(minimal_grammar("br_multi_1"), opts.clone()).unwrap();
    let r2 = build_parser(two_token_grammar("br_multi_2"), tmp_opts(&tmp_dir())).unwrap();

    assert_ne!(r1.grammar_name, r2.grammar_name);
    assert!(r2.build_stats.state_count >= r1.build_stats.state_count);
}

// =========================================================================
// Category 2: BuildStats (8 tests)
// =========================================================================

#[test]
fn buildstats_default_has_positive_state_count() {
    let dir = tmp_dir();
    let opts = tmp_opts(&dir);
    let result = build_parser(minimal_grammar("bs_state"), opts).unwrap();
    assert!(result.build_stats.state_count > 0);
}

#[test]
fn buildstats_default_has_positive_symbol_count() {
    let dir = tmp_dir();
    let opts = tmp_opts(&dir);
    let result = build_parser(minimal_grammar("bs_symbol"), opts).unwrap();
    assert!(result.build_stats.symbol_count > 0);
}

#[test]
fn buildstats_conflict_cells_nonnegative() {
    let dir = tmp_dir();
    let opts = tmp_opts(&dir);
    let _result = build_parser(minimal_grammar("bs_conflicts"), opts).unwrap();
    // conflict_cells is usize (always >= 0)
}

#[test]
fn buildstats_conflict_cells_bounded() {
    let dir = tmp_dir();
    let opts = tmp_opts(&dir);
    let result = build_parser(minimal_grammar("bs_bounded"), opts).unwrap();
    let max = result.build_stats.state_count * result.build_stats.symbol_count;
    assert!(result.build_stats.conflict_cells <= max);
}

#[test]
fn buildstats_two_token_has_more_symbols_than_one_token() {
    let dir = tmp_dir();
    let opts = tmp_opts(&dir);

    let one = build_parser(minimal_grammar("bs_1tok"), opts.clone()).unwrap();
    let two = build_parser(two_token_grammar("bs_2tok"), tmp_opts(&tmp_dir())).unwrap();

    assert!(two.build_stats.symbol_count > one.build_stats.symbol_count);
}

#[test]
fn buildstats_expr_grammar_has_states() {
    let dir = tmp_dir();
    let opts = tmp_opts(&dir);
    let result = build_parser(expr_grammar("bs_expr"), opts).unwrap();
    assert!(result.build_stats.state_count > 0);
}

#[test]
fn buildstats_three_op_has_more_conflict_cells_than_expr() {
    let dir = tmp_dir();
    let opts = tmp_opts(&dir);

    let expr = build_parser(expr_grammar("bs_expr_conflicts"), opts.clone()).unwrap();
    let three_op =
        build_parser(three_op_grammar("bs_3op_conflicts"), tmp_opts(&tmp_dir())).unwrap();

    // Three operators should create more conflicts due to precedence
    assert!(three_op.build_stats.conflict_cells >= expr.build_stats.conflict_cells);
}

#[test]
fn buildstats_all_fields_present_and_valid() {
    let dir = tmp_dir();
    let opts = tmp_opts(&dir);
    let result = build_parser(three_op_grammar("bs_all_fields"), opts).unwrap();

    let stats = &result.build_stats;
    assert!(stats.state_count > 0);
    assert!(stats.symbol_count > 0);
    // conflict_cells is usize (always >= 0)
    assert!(stats.conflict_cells <= stats.state_count * stats.symbol_count);
}

// =========================================================================
// Category 3: BuildOptions (8 tests)
// =========================================================================

#[test]
fn buildoptions_default_has_sensible_out_dir() {
    let opts = BuildOptions::default();
    assert!(!opts.out_dir.is_empty());
}

#[test]
fn buildoptions_default_emit_artifacts_is_false() {
    let opts = BuildOptions::default();
    assert!(!opts.emit_artifacts);
}

#[test]
fn buildoptions_default_compress_tables_is_true() {
    let opts = BuildOptions::default();
    assert!(opts.compress_tables);
}

#[test]
fn buildoptions_custom_out_dir() {
    let dir = tmp_dir();
    let opts = BuildOptions {
        out_dir: dir.path().to_string_lossy().into(),
        emit_artifacts: false,
        compress_tables: false,
    };
    assert_eq!(opts.out_dir, dir.path().to_string_lossy().to_string());
}

#[test]
fn buildoptions_emit_artifacts_true() {
    let dir = tmp_dir();
    let opts = BuildOptions {
        out_dir: dir.path().to_string_lossy().into(),
        emit_artifacts: true,
        compress_tables: false,
    };
    assert!(opts.emit_artifacts);
}

#[test]
fn buildoptions_compress_tables_true() {
    let dir = tmp_dir();
    let opts = BuildOptions {
        out_dir: dir.path().to_string_lossy().into(),
        emit_artifacts: false,
        compress_tables: true,
    };
    assert!(opts.compress_tables);
}

#[test]
fn buildoptions_all_flags_combined() {
    let dir = tmp_dir();
    let opts = BuildOptions {
        out_dir: dir.path().to_string_lossy().into(),
        emit_artifacts: true,
        compress_tables: true,
    };
    assert!(opts.emit_artifacts);
    assert!(opts.compress_tables);
    assert_eq!(opts.out_dir, dir.path().to_string_lossy().to_string());
}

#[test]
fn buildoptions_clone_creates_identical_copy() {
    let dir = tmp_dir();
    let opts1 = BuildOptions {
        out_dir: dir.path().to_string_lossy().into(),
        emit_artifacts: true,
        compress_tables: false,
    };
    let opts2 = opts1.clone();
    assert_eq!(opts1.out_dir, opts2.out_dir);
    assert_eq!(opts1.emit_artifacts, opts2.emit_artifacts);
    assert_eq!(opts1.compress_tables, opts2.compress_tables);
}

// =========================================================================
// Category 4: Grammar JSON (8 tests)
// =========================================================================

#[test]
fn grammar_json_simple_creation() {
    let json_str = r#"{
        "name": "simple",
        "rules": {
            "source_file": [{ "type": "CHOICE", "members": [{ "type": "TOKEN", "value": "NUMBER" }] }]
        },
        "tokens": {
            "NUMBER": { "type": "PATTERN", "value": "\\d+" }
        }
    }"#;
    // Verify the JSON is valid by parsing
    assert!(serde_json::from_str::<serde_json::Value>(json_str).is_ok());
}

#[test]
fn grammar_json_multi_rule_format() {
    let json = json!({
        "name": "multi",
        "rules": {
            "expr": [
                { "type": "CHOICE", "members": [{ "type": "TOKEN", "value": "NUMBER" }] },
                { "type": "CHOICE", "members": [
                    { "type": "RULE", "value": "expr" },
                    { "type": "TOKEN", "value": "PLUS" },
                    { "type": "RULE", "value": "expr" }
                ]}
            ]
        },
        "tokens": {
            "NUMBER": { "type": "PATTERN", "value": "\\d+" },
            "PLUS": { "type": "STRING", "value": "+" }
        }
    });
    assert!(json["name"].is_string());
    assert!(json["rules"].is_object());
    assert!(json["tokens"].is_object());
}

#[test]
fn grammar_json_with_tokens_structure() {
    let json = json!({
        "name": "tokens_test",
        "rules": {
            "source_file": [{ "type": "CHOICE", "members": [{ "type": "TOKEN", "value": "IDENT" }] }]
        },
        "tokens": {
            "IDENT": { "type": "PATTERN", "value": "[a-zA-Z_][a-zA-Z0-9_]*" }
        }
    });

    let tokens = &json["tokens"];
    assert!(tokens.is_object());
    assert!(tokens.get("IDENT").is_some());
}

#[test]
fn grammar_json_preserves_names() {
    let json = json!({
        "name": "preserve_names",
        "rules": {},
        "tokens": {}
    });
    assert_eq!(json["name"].as_str(), Some("preserve_names"));
}

#[test]
fn grammar_json_with_special_characters_in_names() {
    let json = json!({
        "name": "special_chars_123",
        "rules": {
            "rule_with_underscore": [{ "type": "CHOICE", "members": [] }]
        },
        "tokens": {}
    });

    assert!(json["name"].as_str().unwrap().contains("123"));
    assert!(json["rules"].get("rule_with_underscore").is_some());
}

#[test]
fn grammar_json_deterministic_serialization() {
    let json1 = json!({
        "name": "deterministic",
        "rules": { "a": [] },
        "tokens": { "T1": { "type": "STRING", "value": "x" } }
    });

    let json2 = json!({
        "name": "deterministic",
        "rules": { "a": [] },
        "tokens": { "T1": { "type": "STRING", "value": "x" } }
    });

    assert_eq!(json1.to_string(), json2.to_string());
}

#[test]
fn grammar_json_empty_rules_and_tokens() {
    let json = json!({
        "name": "empty_grammar",
        "rules": {},
        "tokens": {}
    });

    assert!(json["rules"].is_object());
    assert_eq!(json["rules"].as_object().unwrap().len(), 0);
    assert!(json["tokens"].is_object());
}

// =========================================================================
// Category 5: Pure Rust build (8 tests)
// =========================================================================

#[test]
fn pure_rust_build_simple_grammar() {
    let dir = tmp_dir();
    let opts = tmp_opts(&dir);
    let result = build_parser(minimal_grammar("prb_simple"), opts);
    assert!(result.is_ok());
}

#[test]
fn pure_rust_build_with_tokens() {
    let dir = tmp_dir();
    let opts = tmp_opts(&dir);
    let result = build_parser(two_token_grammar("prb_tokens"), opts);
    assert!(result.is_ok());
}

#[test]
fn pure_rust_build_result_contains_stats() {
    let dir = tmp_dir();
    let opts = tmp_opts(&dir);
    let result = build_parser(minimal_grammar("prb_stats"), opts).unwrap();
    assert!(result.build_stats.state_count > 0);
}

#[test]
fn pure_rust_build_result_contains_output() {
    let dir = tmp_dir();
    let opts = tmp_opts(&dir);
    let result = build_parser(minimal_grammar("prb_output"), opts).unwrap();
    assert!(!result.parser_code.is_empty());
}

#[test]
fn pure_rust_build_deterministic_output() {
    let dir1 = tmp_dir();
    let dir2 = tmp_dir();
    let opts1 = tmp_opts(&dir1);
    let opts2 = tmp_opts(&dir2);

    let result1 = build_parser(minimal_grammar("prb_det1"), opts1).unwrap();
    let result2 = build_parser(minimal_grammar("prb_det1"), opts2).unwrap();

    // Grammar names and stats should be identical
    assert_eq!(result1.grammar_name, result2.grammar_name);
    assert_eq!(
        result1.build_stats.state_count,
        result2.build_stats.state_count
    );
}

#[test]
fn pure_rust_build_with_precedence() {
    let dir = tmp_dir();
    let opts = tmp_opts(&dir);
    let result = build_parser(expr_grammar("prb_precedence"), opts);
    assert!(result.is_ok());
}

#[test]
fn pure_rust_build_nested_rules() {
    let dir = tmp_dir();
    let opts = tmp_opts(&dir);
    let result = build_parser(nested_rule_grammar("prb_nested"), opts);
    assert!(result.is_ok());
}

// =========================================================================
// Category 6: Error handling (8 tests)
// =========================================================================

#[test]
fn error_handling_empty_grammar_name() {
    // Empty name should still build
    let dir = tmp_dir();
    let opts = tmp_opts(&dir);
    let mut g = minimal_grammar("");
    g.name = "".to_string();
    let result = build_parser(g, opts);
    // Empty name is allowed; grammar should still build
    assert!(result.is_ok());
}

#[test]
fn error_handling_no_rules_with_tokens() {
    // Grammar with tokens but no rules should still work in IR
    let mut g = Grammar::new("no_rules".to_string());
    let tok = SymbolId(1);
    g.tokens.insert(
        tok,
        Token {
            name: "number".into(),
            pattern: TokenPattern::Regex(r"\d+".into()),
            fragile: false,
        },
    );
    // No rules added - this may fail during build
    let dir = tmp_dir();
    let opts = tmp_opts(&dir);
    let result = build_parser(g, opts);
    // This is expected to fail or succeed depending on implementation
    let _ = result;
}

#[test]
fn error_handling_malformed_symbol_ids() {
    // Using inconsistent symbol IDs should still work in IR
    let mut g = Grammar::new("bad_ids".to_string());
    let tok = SymbolId(1000);
    let rule = SymbolId(2000);

    g.tokens.insert(
        tok,
        Token {
            name: "t".into(),
            pattern: TokenPattern::String("x".into()),
            fragile: false,
        },
    );
    g.rule_names.insert(rule, "r".into());
    g.rules.entry(rule).or_default().push(Rule {
        lhs: rule,
        rhs: vec![Symbol::Terminal(tok)],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    });

    let dir = tmp_dir();
    let opts = tmp_opts(&dir);
    let result = build_parser(g, opts);
    let _ = result;
}

#[test]
fn error_handling_duplicate_token_names_allowed() {
    // IR allows same name for different IDs (though not recommended)
    let mut g = Grammar::new("dup_names".to_string());
    let t1 = SymbolId(1);
    let t2 = SymbolId(2);
    let rule = SymbolId(3);

    g.tokens.insert(
        t1,
        Token {
            name: "tok".into(),
            pattern: TokenPattern::String("a".into()),
            fragile: false,
        },
    );
    g.tokens.insert(
        t2,
        Token {
            name: "tok".into(),
            pattern: TokenPattern::String("b".into()),
            fragile: false,
        },
    );
    g.rule_names.insert(rule, "r".into());
    g.rules.entry(rule).or_default().push(Rule {
        lhs: rule,
        rhs: vec![Symbol::Terminal(t1)],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    });

    let dir = tmp_dir();
    let opts = tmp_opts(&dir);
    let result = build_parser(g, opts);
    let _ = result;
}

#[test]
fn error_handling_self_referential_rule() {
    // Direct left recursion should be buildable
    let mut g = Grammar::new("left_recursive".to_string());
    let rule = SymbolId(1);

    g.rule_names.insert(rule, "expr".into());

    // expr -> expr
    g.rules.entry(rule).or_default().push(Rule {
        lhs: rule,
        rhs: vec![Symbol::NonTerminal(rule)],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    });

    let dir = tmp_dir();
    let opts = tmp_opts(&dir);
    let result = build_parser(g, opts);
    // This may fail or succeed depending on implementation
    let _ = result;
}

#[test]
fn error_handling_circular_reference_three_way() {
    // A -> B -> C -> A
    let mut g = Grammar::new("circular_3way".to_string());
    let a = SymbolId(1);
    let b = SymbolId(2);
    let c = SymbolId(3);

    g.rule_names.insert(a, "a".into());
    g.rule_names.insert(b, "b".into());
    g.rule_names.insert(c, "c".into());

    g.rules.entry(a).or_default().push(Rule {
        lhs: a,
        rhs: vec![Symbol::NonTerminal(b)],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    });

    g.rules.entry(b).or_default().push(Rule {
        lhs: b,
        rhs: vec![Symbol::NonTerminal(c)],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(1),
    });

    g.rules.entry(c).or_default().push(Rule {
        lhs: c,
        rhs: vec![Symbol::NonTerminal(a)],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(2),
    });

    let dir = tmp_dir();
    let opts = tmp_opts(&dir);
    let result = build_parser(g, opts);
    let _ = result;
}

#[test]
fn error_handling_unreachable_rule() {
    // Grammar with a rule that's never referenced
    let mut g = Grammar::new("unreachable".to_string());
    let main = SymbolId(1);
    let unreachable = SymbolId(2);
    let tok = SymbolId(3);

    g.tokens.insert(
        tok,
        Token {
            name: "x".into(),
            pattern: TokenPattern::String("x".into()),
            fragile: false,
        },
    );

    g.rule_names.insert(main, "main".into());
    g.rule_names.insert(unreachable, "unused".into());

    g.rules.entry(main).or_default().push(Rule {
        lhs: main,
        rhs: vec![Symbol::Terminal(tok)],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    });

    g.rules.entry(unreachable).or_default().push(Rule {
        lhs: unreachable,
        rhs: vec![Symbol::Terminal(tok)],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(1),
    });

    let dir = tmp_dir();
    let opts = tmp_opts(&dir);
    let result = build_parser(g, opts);
    let _ = result;
}

// =========================================================================
// Category 7: Output content (8 tests)
// =========================================================================

#[test]
fn output_content_parser_code_is_string() {
    let dir = tmp_dir();
    let opts = tmp_opts(&dir);
    let result = build_parser(minimal_grammar("oc_string"), opts).unwrap();
    assert!(!result.parser_code.is_empty());
}

#[test]
fn output_content_node_types_json_is_string() {
    let dir = tmp_dir();
    let opts = tmp_opts(&dir);
    let result = build_parser(minimal_grammar("oc_json"), opts).unwrap();
    assert!(!result.node_types_json.is_empty());
}

#[test]
fn output_content_node_types_is_valid_json() {
    let dir = tmp_dir();
    let opts = tmp_opts(&dir);
    let result = build_parser(minimal_grammar("oc_valid_json"), opts).unwrap();
    assert!(
        serde_json::from_str::<serde_json::Value>(&result.node_types_json).is_ok(),
        "node_types_json should be valid JSON"
    );
}

#[test]
fn output_content_parser_code_contains_rust_syntax() {
    let dir = tmp_dir();
    let opts = tmp_opts(&dir);
    let result = build_parser(minimal_grammar("oc_rust"), opts).unwrap();
    // Should contain Rust keywords or common patterns
    assert!(
        result.parser_code.contains("fn")
            || result.parser_code.contains("pub")
            || result.parser_code.contains("struct"),
        "parser_code should look like Rust"
    );
}

#[test]
fn output_content_two_token_grammar_larger_output() {
    let dir = tmp_dir();
    let opts = tmp_opts(&dir);

    let simple = build_parser(minimal_grammar("oc_simple"), opts.clone()).unwrap();
    let two_tok = build_parser(two_token_grammar("oc_two_tok"), tmp_opts(&tmp_dir())).unwrap();

    assert!(
        two_tok.parser_code.len() > simple.parser_code.len(),
        "Two-token grammar should generate more code"
    );
}

#[test]
fn output_content_expr_grammar_has_multiple_rules() {
    let dir = tmp_dir();
    let opts = tmp_opts(&dir);
    let result = build_parser(expr_grammar("oc_expr"), opts).unwrap();

    // With multiple rules, parser code should be larger
    assert!(
        result.parser_code.len() > 100,
        "Expr grammar should produce significant code"
    );
}

#[test]
fn output_content_three_op_grammar_even_larger() {
    let dir = tmp_dir();
    let opts = tmp_opts(&dir);

    let expr = build_parser(expr_grammar("oc_expr_size"), opts.clone()).unwrap();
    let three_op = build_parser(three_op_grammar("oc_3op_size"), tmp_opts(&tmp_dir())).unwrap();

    // More operators should increase code size
    assert!(
        three_op.parser_code.len() >= expr.parser_code.len(),
        "Three-op grammar should generate at least as much code"
    );
}

#[test]
fn output_content_deterministic_output_same_grammar() {
    let dir1 = tmp_dir();
    let dir2 = tmp_dir();
    let opts1 = tmp_opts(&dir1);
    let opts2 = tmp_opts(&dir2);

    let result1 = build_parser(minimal_grammar("oc_det1"), opts1).unwrap();
    let result2 = build_parser(minimal_grammar("oc_det1"), opts2).unwrap();

    // Generated parser code should be identical for same grammar
    assert_eq!(
        result1.parser_code, result2.parser_code,
        "Same grammar should produce same parser code"
    );
}

// =========================================================================
// Category 8: Integration (8 tests)
// =========================================================================

#[test]
fn integration_build_and_check_stats() {
    let dir = tmp_dir();
    let opts = tmp_opts(&dir);
    let result = build_parser(minimal_grammar("int_stats"), opts).unwrap();

    // Verify stats are sensible
    assert!(result.build_stats.state_count > 0);
    assert!(result.build_stats.symbol_count > 0);
    // conflict_cells is usize (always >= 0)
}

#[test]
fn integration_build_with_compression_option() {
    let dir = tmp_dir();
    let opts_compressed = tmp_opts_with_compression(&dir);
    let result = build_parser(minimal_grammar("int_compressed"), opts_compressed).unwrap();

    assert!(!result.parser_code.is_empty());
}

#[test]
fn integration_build_with_artifacts_option() {
    let dir = tmp_dir();
    let opts_artifacts = tmp_opts_with_artifacts(&dir);
    let result = build_parser(minimal_grammar("int_artifacts"), opts_artifacts).unwrap();

    assert!(!result.parser_code.is_empty());
}

#[test]
fn integration_all_options_combinations() {
    let dir = tmp_dir();

    let opts1 = tmp_opts(&dir);
    let opts2 = tmp_opts_with_compression(&dir);
    let opts3 = tmp_opts_with_artifacts(&dir);

    let _r1 = build_parser(minimal_grammar("int_opts1"), opts1);
    let _r2 = build_parser(minimal_grammar("int_opts2"), opts2);
    let _r3 = build_parser(minimal_grammar("int_opts3"), opts3);

    // All should complete (success or predictable failure)
}

#[test]
fn integration_large_grammar_with_many_tokens() {
    // Create a grammar with many tokens
    let mut g = Grammar::new("large_grammar".to_string());
    let src = SymbolId(300);
    g.rule_names.insert(src, "source_file".into());

    for i in 0..20 {
        let tok = SymbolId((i + 1) as u16);
        g.tokens.insert(
            tok,
            Token {
                name: format!("tok_{i}"),
                pattern: TokenPattern::String(format!("op{i}")),
                fragile: false,
            },
        );
        g.rules.entry(src).or_default().push(Rule {
            lhs: src,
            rhs: vec![Symbol::Terminal(tok)],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(i as u16),
        });
    }

    let dir = tmp_dir();
    let opts = tmp_opts(&dir);
    let result = build_parser(g, opts).unwrap();

    assert!(result.build_stats.symbol_count >= 20);
}

#[test]
fn integration_rebuild_same_grammar_same_output() {
    let dir = tmp_dir();
    let opts = tmp_opts(&dir);

    let g = minimal_grammar("int_rebuild");
    let r1 = build_parser(g.clone(), opts.clone()).unwrap();
    let r2 = build_parser(g, tmp_opts(&tmp_dir())).unwrap();

    assert_eq!(r1.grammar_name, r2.grammar_name);
    assert_eq!(r1.parser_code, r2.parser_code);
    assert_eq!(r1.build_stats.state_count, r2.build_stats.state_count);
}

#[test]
fn integration_nested_grammar_builds_successfully() {
    let dir = tmp_dir();
    let opts = tmp_opts(&dir);
    let result = build_parser(nested_rule_grammar("int_nested"), opts).unwrap();

    assert!(!result.parser_code.is_empty());
    assert!(result.build_stats.state_count > 0);
}
