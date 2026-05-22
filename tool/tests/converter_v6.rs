//! Converter v6: 64 tests covering Grammar → JavaScript grammar conversion.
//!
//! Categories (8 × 8):
//!   1. convert_basic_*      – basic Grammar ↔ GrammarJs round-trip scaffolding
//!   2. convert_token_*      – terminal / token conversion
//!   3. convert_rule_*       – rule structure (SEQ, CHOICE, REPEAT, etc.)
//!   4. convert_prec_*       – precedence and associativity
//!   5. convert_json_*       – JSON parse / emit fidelity
//!   6. convert_roundtrip_*  – JSON → IR → JSON idempotency
//!   7. convert_complex_*    – multi-layer / real-world-ish grammars
//!   8. convert_edge_*       – edge cases and error paths

use adze_ir::{
    Associativity, Grammar, PrecedenceKind, Rule, Symbol, SymbolId, TokenPattern,
    builder::GrammarBuilder,
};
use adze_tool::grammar_js::GrammarJsConverter;
use adze_tool::grammar_js::json_converter::from_tree_sitter_json;
use adze_tool::pure_rust_builder::{BuildOptions, BuildResult, build_parser_from_json};
use serde_json::{Value, json};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn opts() -> BuildOptions {
    BuildOptions {
        out_dir: "/tmp/adze-converter-v6".to_string(),
        emit_artifacts: false,
        compress_tables: false,
    }
}

/// Convert a JSON value through from_tree_sitter_json → GrammarJsConverter → Grammar.
fn convert(val: &Value) -> Grammar {
    let gjs = from_tree_sitter_json(val).expect("from_tree_sitter_json failed");
    GrammarJsConverter::new(gjs)
        .convert()
        .expect("convert failed")
}

/// Build a parser directly from a JSON value.
fn build_json(val: &Value) -> anyhow::Result<BuildResult> {
    build_parser_from_json(serde_json::to_string(val).unwrap(), opts())
}

/// Find a token whose pattern matches the given regex string.
fn find_regex_token(g: &Grammar, regex: &str) -> bool {
    g.tokens.values().any(|t| match &t.pattern {
        TokenPattern::Regex(r) => r == regex,
        _ => false,
    })
}

/// Find a token whose pattern matches the given literal string.
fn find_string_token(g: &Grammar, literal: &str) -> bool {
    g.tokens.values().any(|t| match &t.pattern {
        TokenPattern::String(s) => s == literal,
        _ => false,
    })
}

/// Count total IR rules across all LHS symbols.
fn total_rules(g: &Grammar) -> usize {
    g.rules.values().map(|rs| rs.len()).sum()
}

/// Collect rules for the named symbol.
fn rules_for<'a>(g: &'a Grammar, name: &str) -> Vec<&'a Rule> {
    let sid = g
        .rule_names
        .iter()
        .find(|(_, n)| n.as_str() == name)
        .map(|(id, _)| *id)
        .expect("symbol not found");
    g.rules
        .get(&sid)
        .map(|rs| rs.iter().collect())
        .unwrap_or_default()
}

// ===========================================================================
// 1. convert_basic_* — basic Grammar ↔ GrammarJs scaffolding (8 tests)
// ===========================================================================

#[test]
fn convert_basic_grammar_name_preserved() {
    let g = convert(&json!({
        "name": "my_lang",
        "rules": { "root": { "type": "PATTERN", "value": "." } }
    }));
    assert_eq!(g.name, "my_lang");
}

#[test]
fn convert_basic_single_rule_produces_nonempty_grammar() {
    let g = convert(&json!({
        "name": "one",
        "rules": { "root": { "type": "STRING", "value": "x" } }
    }));
    assert!(total_rules(&g) > 0, "should have at least one IR rule");
}

#[test]
fn convert_basic_rule_names_populated() {
    let g = convert(&json!({
        "name": "names",
        "rules": {
            "alpha": { "type": "STRING", "value": "a" },
            "beta":  { "type": "STRING", "value": "b" }
        }
    }));
    let names: Vec<&str> = g.rule_names.values().map(|s| s.as_str()).collect();
    assert!(
        names.contains(&"alpha"),
        "rule_names should contain 'alpha'"
    );
    assert!(names.contains(&"beta"), "rule_names should contain 'beta'");
}

#[test]
fn convert_basic_tokens_map_nonempty() {
    let g = convert(&json!({
        "name": "tok",
        "rules": { "kw": { "type": "STRING", "value": "let" } }
    }));
    assert!(!g.tokens.is_empty(), "tokens map should be non-empty");
}

#[test]
fn convert_basic_empty_extras() {
    let g = convert(&json!({
        "name": "noex",
        "rules": { "a": { "type": "STRING", "value": "x" } }
    }));
    // No extras specified, whitespace extra is only added if a \\s pattern is in extras.
    // We didn't specify any extras, so extras should be empty.
    assert!(g.extras.is_empty(), "no extras specified → empty extras");
}

#[test]
fn convert_basic_whitespace_extra_detected() {
    let g = convert(&json!({
        "name": "ws",
        "rules": { "a": { "type": "STRING", "value": "x" } },
        "extras": [{ "type": "PATTERN", "value": "\\s" }]
    }));
    assert!(!g.extras.is_empty(), "whitespace extra should be detected");
}

#[test]
fn convert_basic_two_rules_distinct_symbol_ids() {
    let g = convert(&json!({
        "name": "two",
        "rules": {
            "foo": { "type": "STRING", "value": "f" },
            "bar": { "type": "STRING", "value": "b" }
        }
    }));
    let ids: Vec<SymbolId> = g.rule_names.keys().copied().collect();
    assert!(ids.len() >= 2, "should have ≥2 distinct symbol IDs");
    assert_ne!(ids[0], ids[1], "symbol IDs must be distinct");
}

#[test]
fn convert_basic_builder_grammar_has_rules() {
    let grammar = GrammarBuilder::new("basic_builder")
        .token("NUM", r"\d+")
        .rule("start", vec!["NUM"])
        .start("start")
        .build();
    assert!(
        !grammar.rules.is_empty(),
        "builder grammar should have rules"
    );
}

// ===========================================================================
// 2. convert_token_* — terminal / token conversion (8 tests)
// ===========================================================================

#[test]
fn convert_token_string_literal_created() {
    let g = convert(&json!({
        "name": "t1",
        "rules": { "semi": { "type": "STRING", "value": ";" } }
    }));
    assert!(find_string_token(&g, ";"));
}

#[test]
fn convert_token_regex_pattern_created() {
    let g = convert(&json!({
        "name": "t2",
        "rules": { "num": { "type": "PATTERN", "value": "[0-9]+" } }
    }));
    assert!(find_regex_token(&g, "[0-9]+"));
}

#[test]
fn convert_token_duplicated_string_shared() {
    let g = convert(&json!({
        "name": "t3",
        "rules": {
            "pair": {
                "type": "SEQ",
                "members": [
                    { "type": "STRING", "value": "+" },
                    { "type": "STRING", "value": "+" }
                ]
            }
        }
    }));
    let plus_count = g
        .tokens
        .values()
        .filter(|t| matches!(&t.pattern, TokenPattern::String(s) if s == "+"))
        .count();
    assert_eq!(
        plus_count, 1,
        "identical STRING literals should share one token"
    );
}

#[test]
fn convert_token_keyword_stored_as_string_pattern() {
    let g = convert(&json!({
        "name": "t4",
        "rules": { "kw": { "type": "STRING", "value": "return" } }
    }));
    let tok = g
        .tokens
        .values()
        .find(|t| matches!(&t.pattern, TokenPattern::String(s) if s == "return"));
    assert!(tok.is_some(), "keyword should be a String pattern");
}

#[test]
fn convert_token_regex_unicode_class() {
    let g = convert(&json!({
        "name": "t5",
        "rules": { "letter": { "type": "PATTERN", "value": "\\p{L}+" } }
    }));
    assert!(find_regex_token(&g, "\\p{L}+"));
}

#[test]
fn convert_token_hex_regex() {
    let g = convert(&json!({
        "name": "t6",
        "rules": { "hex": { "type": "PATTERN", "value": "0x[0-9a-fA-F]+" } }
    }));
    assert!(find_regex_token(&g, "0x[0-9a-fA-F]+"));
}

#[test]
fn convert_token_single_char_operator() {
    let g = convert(&json!({
        "name": "t7",
        "rules": { "star": { "type": "STRING", "value": "*" } }
    }));
    assert!(find_string_token(&g, "*"));
}

#[test]
fn convert_token_multichar_operator() {
    let g = convert(&json!({
        "name": "t8",
        "rules": { "arrow": { "type": "STRING", "value": "=>" } }
    }));
    assert!(find_string_token(&g, "=>"));
}

// ===========================================================================
// 3. convert_rule_* — rule structure (8 tests)
// ===========================================================================

#[test]
fn convert_rule_seq_two_members() {
    let g = convert(&json!({
        "name": "r1",
        "rules": {
            "pair": {
                "type": "SEQ",
                "members": [
                    { "type": "STRING", "value": "(" },
                    { "type": "STRING", "value": ")" }
                ]
            }
        }
    }));
    let rules = rules_for(&g, "pair");
    assert!(rules.iter().any(|r| r.rhs.len() == 2));
}

#[test]
fn convert_rule_choice_creates_alternatives() {
    let g = convert(&json!({
        "name": "r2",
        "rules": {
            "val": {
                "type": "CHOICE",
                "members": [
                    { "type": "STRING", "value": "yes" },
                    { "type": "STRING", "value": "no" }
                ]
            }
        }
    }));
    let rules = rules_for(&g, "val");
    assert!(rules.len() >= 2, "CHOICE with 2 members → ≥2 rules");
}

#[test]
fn convert_rule_repeat_has_empty_production() {
    let g = convert(&json!({
        "name": "r3",
        "rules": {
            "items": {
                "type": "REPEAT",
                "content": { "type": "STRING", "value": "x" }
            }
        }
    }));
    let rules = rules_for(&g, "items");
    assert!(rules.iter().any(|r| r.rhs.is_empty()), "REPEAT → ε rule");
}

#[test]
fn convert_rule_repeat1_has_base_case() {
    let g = convert(&json!({
        "name": "r4",
        "rules": {
            "items": {
                "type": "REPEAT1",
                "content": { "type": "STRING", "value": "y" }
            }
        }
    }));
    let rules = rules_for(&g, "items");
    let has_single = rules
        .iter()
        .any(|r| r.rhs.len() == 1 && matches!(&r.rhs[0], Symbol::Terminal(_)));
    assert!(has_single, "REPEAT1 should have a base-case rule");
}

#[test]
fn convert_rule_optional_both_empty_and_nonempty() {
    let g = convert(&json!({
        "name": "r5",
        "rules": {
            "maybe": {
                "type": "OPTIONAL",
                "content": { "type": "STRING", "value": "z" }
            }
        }
    }));
    let rules = rules_for(&g, "maybe");
    assert!(rules.iter().any(|r| r.rhs.is_empty()), "empty alt");
    assert!(rules.iter().any(|r| !r.rhs.is_empty()), "non-empty alt");
}

#[test]
fn convert_rule_symbol_ref_produces_nonterminal() {
    let g = convert(&json!({
        "name": "r6",
        "rules": {
            "wrapper": { "type": "SYMBOL", "name": "inner" },
            "inner":   { "type": "PATTERN", "value": "[a-z]+" }
        }
    }));
    let rules = rules_for(&g, "wrapper");
    let has_nt = rules
        .iter()
        .any(|r| r.rhs.iter().any(|s| matches!(s, Symbol::NonTerminal(_))));
    assert!(has_nt, "SYMBOL ref should produce NonTerminal");
}

#[test]
fn convert_rule_seq_with_symbol_and_string() {
    let g = convert(&json!({
        "name": "r7",
        "rules": {
            "stmt": {
                "type": "SEQ",
                "members": [
                    { "type": "SYMBOL", "name": "id" },
                    { "type": "STRING", "value": ";" }
                ]
            },
            "id": { "type": "PATTERN", "value": "[a-z]+" }
        }
    }));
    let rules = rules_for(&g, "stmt");
    assert!(rules.iter().any(|r| r.rhs.len() == 2));
}

#[test]
fn convert_rule_blank_creates_no_rhs() {
    let g = convert(&json!({
        "name": "r8",
        "rules": {
            "empty": { "type": "BLANK" }
        }
    }));
    // BLANK may produce an empty rule or be handled as no-op.
    let rules = rules_for(&g, "empty");
    if !rules.is_empty() {
        assert!(rules.iter().any(|r| r.rhs.is_empty()), "BLANK → empty rhs");
    }
}

// ===========================================================================
// 4. convert_prec_* — precedence and associativity (8 tests)
// ===========================================================================

fn arith_json() -> Value {
    json!({
        "name": "arith",
        "rules": {
            "expression": {
                "type": "CHOICE",
                "members": [
                    { "type": "SYMBOL", "name": "number" },
                    {
                        "type": "PREC_LEFT",
                        "value": 1,
                        "content": {
                            "type": "SEQ",
                            "members": [
                                { "type": "SYMBOL", "name": "expression" },
                                { "type": "STRING", "value": "+" },
                                { "type": "SYMBOL", "name": "expression" }
                            ]
                        }
                    },
                    {
                        "type": "PREC_RIGHT",
                        "value": 2,
                        "content": {
                            "type": "SEQ",
                            "members": [
                                { "type": "SYMBOL", "name": "expression" },
                                { "type": "STRING", "value": "^" },
                                { "type": "SYMBOL", "name": "expression" }
                            ]
                        }
                    }
                ]
            },
            "number": { "type": "PATTERN", "value": "[0-9]+" }
        }
    })
}

#[test]
fn convert_prec_left_sets_left_assoc() {
    let g = convert(&arith_json());
    let rules = rules_for(&g, "expression");
    assert!(
        rules
            .iter()
            .any(|r| r.associativity == Some(Associativity::Left)),
        "PREC_LEFT → Left"
    );
}

#[test]
fn convert_prec_right_sets_right_assoc() {
    let g = convert(&arith_json());
    let rules = rules_for(&g, "expression");
    assert!(
        rules
            .iter()
            .any(|r| r.associativity == Some(Associativity::Right)),
        "PREC_RIGHT → Right"
    );
}

#[test]
fn convert_prec_left_stores_precedence_level() {
    let g = convert(&arith_json());
    let rules = rules_for(&g, "expression");
    assert!(
        rules
            .iter()
            .any(|r| r.precedence == Some(PrecedenceKind::Static(1))),
        "PREC_LEFT(1) → Static(1)"
    );
}

#[test]
fn convert_prec_right_stores_precedence_level() {
    let g = convert(&arith_json());
    let rules = rules_for(&g, "expression");
    assert!(
        rules
            .iter()
            .any(|r| r.precedence == Some(PrecedenceKind::Static(2))),
        "PREC_RIGHT(2) → Static(2)"
    );
}

#[test]
fn convert_prec_left_produces_three_rhs() {
    let g = convert(&arith_json());
    let rules = rules_for(&g, "expression");
    let left = rules
        .iter()
        .find(|r| r.associativity == Some(Associativity::Left));
    assert!(left.is_some());
    assert_eq!(left.unwrap().rhs.len(), 3);
}

#[test]
fn convert_prec_different_levels_coexist() {
    let g = convert(&json!({
        "name": "levels",
        "rules": {
            "e": {
                "type": "CHOICE",
                "members": [
                    { "type": "SYMBOL", "name": "n" },
                    {
                        "type": "PREC_LEFT", "value": 5,
                        "content": { "type": "SEQ", "members": [
                            { "type": "SYMBOL", "name": "e" },
                            { "type": "STRING", "value": "+" },
                            { "type": "SYMBOL", "name": "e" }
                        ]}
                    },
                    {
                        "type": "PREC_LEFT", "value": 10,
                        "content": { "type": "SEQ", "members": [
                            { "type": "SYMBOL", "name": "e" },
                            { "type": "STRING", "value": "*" },
                            { "type": "SYMBOL", "name": "e" }
                        ]}
                    }
                ]
            },
            "n": { "type": "PATTERN", "value": "\\d+" }
        }
    }));
    let rules = rules_for(&g, "e");
    assert!(
        rules
            .iter()
            .any(|r| r.precedence == Some(PrecedenceKind::Static(5)))
    );
    assert!(
        rules
            .iter()
            .any(|r| r.precedence == Some(PrecedenceKind::Static(10)))
    );
}

#[test]
fn convert_prec_plain_prec_stores_value() {
    let g = convert(&json!({
        "name": "pp",
        "rules": {
            "e": {
                "type": "CHOICE",
                "members": [
                    { "type": "SYMBOL", "name": "a" },
                    {
                        "type": "PREC", "value": 7,
                        "content": { "type": "SYMBOL", "name": "a" }
                    }
                ]
            },
            "a": { "type": "PATTERN", "value": "\\w+" }
        }
    }));
    let rules = rules_for(&g, "e");
    assert!(
        rules
            .iter()
            .any(|r| r.precedence == Some(PrecedenceKind::Static(7))),
        "plain PREC(7) → Static(7)"
    );
}

#[test]
fn convert_prec_builder_with_precedence() {
    let grammar = GrammarBuilder::new("prec_builder")
        .token("NUM", r"\d+")
        .token("+", "+")
        .token("*", "*")
        .rule_with_precedence("expr", vec!["expr", "+", "expr"], 1, Associativity::Left)
        .rule_with_precedence("expr", vec!["expr", "*", "expr"], 2, Associativity::Left)
        .rule("expr", vec!["NUM"])
        .start("expr")
        .build();
    let expr_id = grammar
        .rule_names
        .iter()
        .find(|(_, n)| n.as_str() == "expr")
        .map(|(id, _)| *id)
        .unwrap();
    let expr_rules = &grammar.rules[&expr_id];
    let has_prec_1 = expr_rules
        .iter()
        .any(|r| r.precedence == Some(PrecedenceKind::Static(1)));
    let has_prec_2 = expr_rules
        .iter()
        .any(|r| r.precedence == Some(PrecedenceKind::Static(2)));
    assert!(has_prec_1, "precedence 1 should be set");
    assert!(has_prec_2, "precedence 2 should be set");
}

// ===========================================================================
// 5. convert_json_* — JSON parse / emit fidelity (8 tests)
// ===========================================================================

#[test]
fn convert_json_minimal_parses() {
    let v = json!({ "name": "j1", "rules": { "r": { "type": "BLANK" } } });
    let gjs = from_tree_sitter_json(&v);
    assert!(gjs.is_ok());
}

#[test]
fn convert_json_word_field_parsed() {
    let v = json!({
        "name": "j2",
        "word": "identifier",
        "rules": { "identifier": { "type": "PATTERN", "value": "[a-z]+" } }
    });
    let gjs = from_tree_sitter_json(&v).unwrap();
    assert_eq!(gjs.word, Some("identifier".to_string()));
}

#[test]
fn convert_json_inline_rules_parsed() {
    let v = json!({
        "name": "j3",
        "inline": ["_helper"],
        "rules": {
            "root": { "type": "SYMBOL", "name": "_helper" },
            "_helper": { "type": "PATTERN", "value": "." }
        }
    });
    let gjs = from_tree_sitter_json(&v).unwrap();
    assert_eq!(gjs.inline, vec!["_helper".to_string()]);
}

#[test]
fn convert_json_conflicts_parsed() {
    let v = json!({
        "name": "j4",
        "conflicts": [["expr", "stmt"]],
        "rules": {
            "expr": { "type": "PATTERN", "value": "\\d+" },
            "stmt": { "type": "PATTERN", "value": "[a-z]+" }
        }
    });
    let gjs = from_tree_sitter_json(&v).unwrap();
    assert_eq!(gjs.conflicts.len(), 1);
    assert_eq!(gjs.conflicts[0], vec!["expr", "stmt"]);
}

#[test]
fn convert_json_extras_parsed() {
    let v = json!({
        "name": "j5",
        "rules": { "a": { "type": "STRING", "value": "x" } },
        "extras": [{ "type": "PATTERN", "value": "\\s" }]
    });
    let gjs = from_tree_sitter_json(&v).unwrap();
    assert_eq!(gjs.extras.len(), 1);
}

#[test]
fn convert_json_externals_parsed() {
    let v = json!({
        "name": "j6",
        "rules": { "a": { "type": "STRING", "value": "x" } },
        "externals": [{ "name": "indent", "type": "SYMBOL" }]
    });
    let gjs = from_tree_sitter_json(&v).unwrap();
    assert_eq!(gjs.externals.len(), 1);
    assert_eq!(gjs.externals[0].name, "indent");
}

#[test]
fn convert_json_supertypes_parsed() {
    let v = json!({
        "name": "j7",
        "supertypes": ["expression"],
        "rules": { "expression": { "type": "PATTERN", "value": "." } }
    });
    let gjs = from_tree_sitter_json(&v).unwrap();
    assert_eq!(gjs.supertypes, vec!["expression".to_string()]);
}

#[test]
fn convert_json_rules_order_preserved() {
    let v = json!({
        "name": "j8",
        "rules": {
            "alpha": { "type": "STRING", "value": "a" },
            "beta":  { "type": "STRING", "value": "b" },
            "gamma": { "type": "STRING", "value": "c" }
        }
    });
    let gjs = from_tree_sitter_json(&v).unwrap();
    let keys: Vec<&String> = gjs.rules.keys().collect();
    assert_eq!(keys, &["alpha", "beta", "gamma"]);
}

// ===========================================================================
// 6. convert_roundtrip_* — JSON → IR → assertions on IR (8 tests)
// ===========================================================================

#[test]
fn convert_roundtrip_name_survives() {
    let v = json!({
        "name": "roundtrip1",
        "rules": { "r": { "type": "STRING", "value": "x" } }
    });
    let g = convert(&v);
    assert_eq!(g.name, "roundtrip1");
}

#[test]
fn convert_roundtrip_string_token_in_ir() {
    let v = json!({
        "name": "roundtrip2",
        "rules": { "k": { "type": "STRING", "value": "fn" } }
    });
    let g = convert(&v);
    assert!(find_string_token(&g, "fn"));
}

#[test]
fn convert_roundtrip_pattern_token_in_ir() {
    let v = json!({
        "name": "roundtrip3",
        "rules": { "n": { "type": "PATTERN", "value": "\\d+" } }
    });
    let g = convert(&v);
    assert!(find_regex_token(&g, "\\d+"));
}

#[test]
fn convert_roundtrip_seq_length_preserved() {
    let v = json!({
        "name": "roundtrip4",
        "rules": {
            "s": {
                "type": "SEQ",
                "members": [
                    { "type": "STRING", "value": "a" },
                    { "type": "STRING", "value": "b" },
                    { "type": "STRING", "value": "c" }
                ]
            }
        }
    });
    let g = convert(&v);
    let rules = rules_for(&g, "s");
    assert!(rules.iter().any(|r| r.rhs.len() == 3));
}

#[test]
fn convert_roundtrip_choice_count() {
    let v = json!({
        "name": "roundtrip5",
        "rules": {
            "c": {
                "type": "CHOICE",
                "members": [
                    { "type": "STRING", "value": "a" },
                    { "type": "STRING", "value": "b" },
                    { "type": "STRING", "value": "c" }
                ]
            }
        }
    });
    let g = convert(&v);
    let rules = rules_for(&g, "c");
    assert!(
        rules.len() >= 3,
        "3-way CHOICE → ≥3 rules, got {}",
        rules.len()
    );
}

#[test]
fn convert_roundtrip_optional_has_two_alts() {
    let v = json!({
        "name": "roundtrip6",
        "rules": {
            "o": { "type": "OPTIONAL", "content": { "type": "STRING", "value": "?" } }
        }
    });
    let g = convert(&v);
    let rules = rules_for(&g, "o");
    assert!(rules.len() >= 2, "OPTIONAL → ≥2 rules (empty + content)");
}

#[test]
fn convert_roundtrip_repeat_has_recursive() {
    let v = json!({
        "name": "roundtrip7",
        "rules": {
            "rep": { "type": "REPEAT", "content": { "type": "STRING", "value": "x" } }
        }
    });
    let g = convert(&v);
    let rules = rules_for(&g, "rep");
    let lhs_id = g
        .rule_names
        .iter()
        .find(|(_, n)| n.as_str() == "rep")
        .map(|(id, _)| *id)
        .unwrap();
    let has_rec = rules.iter().any(|r| {
        r.rhs
            .iter()
            .any(|s| matches!(s, Symbol::NonTerminal(id) if *id == lhs_id))
    });
    assert!(has_rec, "REPEAT should have a self-recursive rule");
}

#[test]
fn convert_roundtrip_prec_preserved() {
    let v = json!({
        "name": "roundtrip8",
        "rules": {
            "e": {
                "type": "CHOICE",
                "members": [
                    { "type": "SYMBOL", "name": "a" },
                    {
                        "type": "PREC_LEFT", "value": 3,
                        "content": { "type": "SEQ", "members": [
                            { "type": "SYMBOL", "name": "e" },
                            { "type": "STRING", "value": "+" },
                            { "type": "SYMBOL", "name": "a" }
                        ]}
                    }
                ]
            },
            "a": { "type": "PATTERN", "value": "\\d+" }
        }
    });
    let g = convert(&v);
    let rules = rules_for(&g, "e");
    assert!(
        rules
            .iter()
            .any(|r| r.precedence == Some(PrecedenceKind::Static(3)))
    );
}

// ===========================================================================
// 7. convert_complex_* — multi-layer / real-world-ish grammars (8 tests)
// ===========================================================================

#[test]
fn convert_complex_nested_choice_in_seq() {
    let g = convert(&json!({
        "name": "cx1",
        "rules": {
            "stmt": {
                "type": "SEQ",
                "members": [
                    { "type": "STRING", "value": "var" },
                    { "type": "SYMBOL", "name": "name" }
                ]
            },
            "name": {
                "type": "CHOICE",
                "members": [
                    { "type": "STRING", "value": "x" },
                    { "type": "STRING", "value": "y" }
                ]
            }
        }
    }));
    let stmt_rules = rules_for(&g, "stmt");
    assert!(!stmt_rules.is_empty());
    let name_rules = rules_for(&g, "name");
    assert!(name_rules.len() >= 2);
}

#[test]
fn convert_complex_three_level_nesting() {
    let g = convert(&json!({
        "name": "cx2",
        "rules": {
            "expr": {
                "type": "CHOICE",
                "members": [
                    { "type": "SYMBOL", "name": "atom" },
                    {
                        "type": "PREC_LEFT", "value": 4,
                        "content": {
                            "type": "SEQ",
                            "members": [
                                { "type": "SYMBOL", "name": "expr" },
                                { "type": "STRING", "value": "-" },
                                { "type": "SYMBOL", "name": "atom" }
                            ]
                        }
                    }
                ]
            },
            "atom": { "type": "PATTERN", "value": "\\d+" }
        }
    }));
    let rules = rules_for(&g, "expr");
    let deep = rules.iter().find(|r| {
        r.precedence == Some(PrecedenceKind::Static(4))
            && r.associativity == Some(Associativity::Left)
            && r.rhs.len() == 3
    });
    assert!(
        deep.is_some(),
        "3-level nesting should produce correct rule"
    );
}

#[test]
fn convert_complex_repeat_of_symbol() {
    let g = convert(&json!({
        "name": "cx3",
        "rules": {
            "list": {
                "type": "REPEAT",
                "content": { "type": "SYMBOL", "name": "item" }
            },
            "item": { "type": "PATTERN", "value": "[a-z]+" }
        }
    }));
    let rules = rules_for(&g, "list");
    assert!(rules.len() >= 2, "REPEAT(SYMBOL) → ≥2 rules");
}

#[test]
fn convert_complex_optional_of_symbol() {
    let g = convert(&json!({
        "name": "cx4",
        "rules": {
            "maybe_num": {
                "type": "OPTIONAL",
                "content": { "type": "SYMBOL", "name": "num" }
            },
            "num": { "type": "PATTERN", "value": "\\d+" }
        }
    }));
    let rules = rules_for(&g, "maybe_num");
    assert!(rules.iter().any(|r| r.rhs.is_empty()));
    assert!(rules.iter().any(|r| !r.rhs.is_empty()));
}

#[test]
fn convert_complex_field_in_rule_body() {
    // FIELD as a top-level rule body creates a field in the IR.
    let g = convert(&json!({
        "name": "cx5",
        "rules": {
            "assign": {
                "type": "FIELD",
                "name": "value",
                "content": { "type": "SYMBOL", "name": "ident" }
            },
            "ident": { "type": "PATTERN", "value": "[a-z]+" }
        }
    }));
    let field_names: Vec<&str> = g.fields.values().map(|s| s.as_str()).collect();
    assert!(field_names.contains(&"value"), "field 'value' should exist");
}

#[test]
fn convert_complex_multi_rule_grammar() {
    let g = convert(&json!({
        "name": "cx6",
        "rules": {
            "program": {
                "type": "REPEAT",
                "content": { "type": "SYMBOL", "name": "stmt" }
            },
            "stmt": {
                "type": "SEQ",
                "members": [
                    { "type": "SYMBOL", "name": "val" },
                    { "type": "STRING", "value": ";" }
                ]
            },
            "val": {
                "type": "CHOICE",
                "members": [
                    { "type": "PATTERN", "value": "\\d+" },
                    { "type": "STRING", "value": "nil" }
                ]
            }
        }
    }));
    assert_eq!(g.name, "cx6");
    assert!(g.rule_names.values().any(|n| n == "program"));
    assert!(g.rule_names.values().any(|n| n == "stmt"));
    assert!(g.rule_names.values().any(|n| n == "val"));
}

#[test]
fn convert_complex_inline_rules_converted() {
    let g = convert(&json!({
        "name": "cx7",
        "inline": ["_helper"],
        "rules": {
            "root": { "type": "SYMBOL", "name": "_helper" },
            "_helper": { "type": "PATTERN", "value": "." }
        }
    }));
    // Inline rules should be recorded in grammar.inline_rules
    // The symbol for _helper should be in inline_rules
    let helper_id = g
        .rule_names
        .iter()
        .find(|(_, n)| n.as_str() == "_helper")
        .map(|(id, _)| *id);
    if let Some(hid) = helper_id {
        assert!(
            g.inline_rules.contains(&hid),
            "inline rule should be in grammar.inline_rules"
        );
    }
}

#[test]
fn convert_complex_conflict_declaration() {
    let g = convert(&json!({
        "name": "cx8",
        "conflicts": [["expr", "stmt"]],
        "rules": {
            "expr": { "type": "PATTERN", "value": "\\d+" },
            "stmt": { "type": "PATTERN", "value": "[a-z]+" }
        }
    }));
    assert!(
        !g.conflicts.is_empty(),
        "conflicts should be propagated to Grammar"
    );
}

// ===========================================================================
// 8. convert_edge_* — edge cases and error paths (8 tests)
// ===========================================================================

#[test]
fn convert_edge_missing_name_fails() {
    let result = from_tree_sitter_json(&json!({
        "rules": { "a": { "type": "BLANK" } }
    }));
    assert!(result.is_err(), "missing 'name' should fail");
}

#[test]
fn convert_edge_empty_json_object_fails() {
    let result = from_tree_sitter_json(&json!({}));
    assert!(result.is_err());
}

#[test]
fn convert_edge_not_an_object_fails() {
    let result = from_tree_sitter_json(&json!("just a string"));
    assert!(result.is_err());
}

#[test]
fn convert_edge_missing_rules_tolerated() {
    let gjs = from_tree_sitter_json(&json!({ "name": "empty" }));
    assert!(gjs.is_ok());
    assert!(gjs.unwrap().rules.is_empty());
}

#[test]
fn convert_edge_rules_not_object_ignored() {
    let gjs = from_tree_sitter_json(&json!({
        "name": "bad",
        "rules": "not an object"
    }));
    assert!(gjs.is_ok());
    assert!(gjs.unwrap().rules.is_empty());
}

#[test]
fn convert_edge_invalid_json_string() {
    let result = build_parser_from_json("{{not valid}}".to_string(), opts());
    assert!(result.is_err());
}

#[test]
fn convert_edge_empty_rules_produces_empty_ir() {
    let v = json!({ "name": "empty", "rules": {} });
    let gjs = from_tree_sitter_json(&v).unwrap();
    let conv_result = GrammarJsConverter::new(gjs).convert();
    if let Ok(g) = conv_result {
        assert!(
            total_rules(&g) == 0 || g.rules.is_empty(),
            "empty rules → no IR rules"
        );
    }
}

#[test]
fn convert_edge_blank_only_grammar_no_panic() {
    // A grammar with only BLANK should not panic.
    let result = build_json(&json!({
        "name": "blank_only",
        "rules": {
            "start": { "type": "BLANK" }
        }
    }));
    // Either error or degenerate build — no panic.
    if let Err(e) = &result {
        assert!(!e.to_string().is_empty());
    }
}
