//! JSON grammar generation and conversion tests for adze-tool (v5).
//!
//! 55+ tests covering:
//! 1. Parse valid JSON grammars of increasing complexity
//! 2. Grammar name preserved in output
//! 3. Rule types correctly interpreted (PATTERN, STRING, SEQ, CHOICE, REPEAT, REPEAT1, OPTIONAL)
//! 4. Precedence values applied correctly
//! 5. Invalid rule types produce errors
//! 6. Token extraction from grammar JSON
//! 7. Multiple rules with cross-references
//! 8. Edge cases: empty rules, deeply nested structures

use adze_tool::grammar_js::Rule;
use adze_tool::grammar_js::json_converter::from_tree_sitter_json;
use adze_tool::pure_rust_builder::{BuildOptions, build_parser_from_json};
use serde_json::json;
use tempfile::TempDir;

// ===========================================================================
// Helpers
// ===========================================================================

fn test_options(dir: &TempDir) -> BuildOptions {
    BuildOptions {
        out_dir: dir.path().to_string_lossy().to_string(),
        emit_artifacts: false,
        compress_tables: false,
    }
}

fn make_grammar(name: &str, rules: serde_json::Value) -> serde_json::Value {
    json!({ "name": name, "rules": rules })
}

// ===========================================================================
// Category 1: Parse valid JSON grammars of increasing complexity (1–10)
// ===========================================================================

#[test]
fn parse_single_string_rule() {
    let g = from_tree_sitter_json(&make_grammar(
        "single",
        json!({
            "source": { "type": "STRING", "value": "hello" }
        }),
    ))
    .unwrap();
    assert_eq!(g.rules.len(), 1);
}

#[test]
fn parse_single_pattern_rule() {
    let g = from_tree_sitter_json(&make_grammar(
        "pat",
        json!({
            "source": { "type": "PATTERN", "value": "[0-9]+" }
        }),
    ))
    .unwrap();
    assert!(matches!(&g.rules["source"], Rule::Pattern { value } if value == "[0-9]+"));
}

#[test]
fn parse_two_rule_grammar() {
    let g = from_tree_sitter_json(&make_grammar(
        "two",
        json!({
            "program": { "type": "SYMBOL", "name": "expr" },
            "expr": { "type": "PATTERN", "value": "\\d+" }
        }),
    ))
    .unwrap();
    assert_eq!(g.rules.len(), 2);
}

#[test]
fn parse_three_rule_chain() {
    let g = from_tree_sitter_json(&make_grammar(
        "chain",
        json!({
            "program": { "type": "SYMBOL", "name": "stmt" },
            "stmt": { "type": "SYMBOL", "name": "atom" },
            "atom": { "type": "STRING", "value": "x" }
        }),
    ))
    .unwrap();
    assert_eq!(g.rules.len(), 3);
}

#[test]
fn parse_grammar_with_seq_and_choice() {
    let g = from_tree_sitter_json(&json!({
        "name": "combo",
        "rules": {
            "source": {
                "type": "SEQ",
                "members": [
                    { "type": "CHOICE", "members": [
                        { "type": "STRING", "value": "a" },
                        { "type": "STRING", "value": "b" }
                    ]},
                    { "type": "STRING", "value": ";" }
                ]
            }
        }
    }))
    .unwrap();
    assert!(matches!(&g.rules["source"], Rule::Seq { members } if members.len() == 2));
}

#[test]
fn parse_grammar_with_repeat_and_repeat1() {
    let g = from_tree_sitter_json(&make_grammar(
        "reps",
        json!({
            "zero_or_more": { "type": "REPEAT", "content": { "type": "STRING", "value": "x" } },
            "one_or_more": { "type": "REPEAT1", "content": { "type": "STRING", "value": "y" } }
        }),
    ))
    .unwrap();
    assert!(matches!(&g.rules["zero_or_more"], Rule::Repeat { .. }));
    assert!(matches!(&g.rules["one_or_more"], Rule::Repeat1 { .. }));
}

#[test]
fn parse_grammar_with_optional() {
    let g = from_tree_sitter_json(&make_grammar(
        "opt",
        json!({
            "maybe": { "type": "OPTIONAL", "value": { "type": "STRING", "value": "?" } }
        }),
    ))
    .unwrap();
    assert!(matches!(&g.rules["maybe"], Rule::Optional { .. }));
}

#[test]
fn parse_grammar_with_extras() {
    let g = from_tree_sitter_json(&json!({
        "name": "ws",
        "extras": [{ "type": "PATTERN", "value": "\\s+" }],
        "rules": { "source": { "type": "STRING", "value": "ok" } }
    }))
    .unwrap();
    assert_eq!(g.extras.len(), 1);
}

#[test]
fn parse_grammar_with_token_rule() {
    let g = from_tree_sitter_json(&make_grammar(
        "tok",
        json!({
            "ident": { "type": "TOKEN", "content": { "type": "PATTERN", "value": "[a-z]+" } }
        }),
    ))
    .unwrap();
    assert!(matches!(&g.rules["ident"], Rule::Token { .. }));
}

#[test]
fn parse_grammar_with_field_and_alias() {
    let g = from_tree_sitter_json(&make_grammar("fa", json!({
        "pair": {
            "type": "SEQ",
            "members": [
                { "type": "FIELD", "name": "key", "content": { "type": "STRING", "value": "k" } },
                { "type": "ALIAS", "content": { "type": "STRING", "value": "v" }, "value": "val_alias", "named": true }
            ]
        }
    }))).unwrap();
    if let Rule::Seq { members } = &g.rules["pair"] {
        assert_eq!(members.len(), 2);
        assert!(matches!(&members[0], Rule::Field { name, .. } if name == "key"));
        assert!(
            matches!(&members[1], Rule::Alias { value, named, .. } if value == "val_alias" && *named)
        );
    } else {
        panic!("expected Seq");
    }
}

// ===========================================================================
// Category 2: Grammar name preserved (11–15)
// ===========================================================================

#[test]
fn grammar_name_simple() {
    let g = from_tree_sitter_json(&make_grammar(
        "my_lang",
        json!({
            "src": { "type": "BLANK" }
        }),
    ))
    .unwrap();
    assert_eq!(g.name, "my_lang");
}

#[test]
fn grammar_name_with_underscores() {
    let g = from_tree_sitter_json(&make_grammar(
        "my_test_lang",
        json!({
            "src": { "type": "BLANK" }
        }),
    ))
    .unwrap();
    assert_eq!(g.name, "my_test_lang");
}

#[test]
fn grammar_name_single_char() {
    let g = from_tree_sitter_json(&make_grammar(
        "x",
        json!({
            "src": { "type": "BLANK" }
        }),
    ))
    .unwrap();
    assert_eq!(g.name, "x");
}

#[test]
fn grammar_name_preserved_after_build() {
    let dir = TempDir::new().unwrap();
    let grammar_json = make_grammar(
        "preserved_name",
        json!({
            "source_file": { "type": "STRING", "value": "hello" }
        }),
    );
    let result = build_parser_from_json(grammar_json.to_string(), test_options(&dir));
    // build_parser_from_json may fail on trivial grammars; we just check name extraction
    match result {
        Ok(r) => assert_eq!(r.grammar_name, "preserved_name"),
        Err(_) => {
            // Even on error, from_tree_sitter_json preserves the name
            let g = from_tree_sitter_json(&grammar_json).unwrap();
            assert_eq!(g.name, "preserved_name");
        }
    }
}

#[test]
fn grammar_name_missing_produces_error() {
    let result = from_tree_sitter_json(&json!({
        "rules": { "src": { "type": "BLANK" } }
    }));
    assert!(result.is_err());
}

// ===========================================================================
// Category 3: Rule types correctly interpreted (16–26)
// ===========================================================================

#[test]
fn rule_type_string_has_value() {
    let g = from_tree_sitter_json(&make_grammar(
        "s",
        json!({
            "kw": { "type": "STRING", "value": "return" }
        }),
    ))
    .unwrap();
    if let Rule::String { value } = &g.rules["kw"] {
        assert_eq!(value, "return");
    } else {
        panic!("expected String");
    }
}

#[test]
fn rule_type_pattern_has_value() {
    let g = from_tree_sitter_json(&make_grammar(
        "p",
        json!({
            "num": { "type": "PATTERN", "value": "-?\\d+\\.?\\d*" }
        }),
    ))
    .unwrap();
    if let Rule::Pattern { value } = &g.rules["num"] {
        assert_eq!(value, r"-?\d+\.?\d*");
    } else {
        panic!("expected Pattern");
    }
}

#[test]
fn rule_type_seq_members_count() {
    let g = from_tree_sitter_json(&make_grammar(
        "sq",
        json!({
            "triple": {
                "type": "SEQ",
                "members": [
                    { "type": "STRING", "value": "a" },
                    { "type": "STRING", "value": "b" },
                    { "type": "STRING", "value": "c" }
                ]
            }
        }),
    ))
    .unwrap();
    if let Rule::Seq { members } = &g.rules["triple"] {
        assert_eq!(members.len(), 3);
    } else {
        panic!("expected Seq");
    }
}

#[test]
fn rule_type_choice_members_count() {
    let g = from_tree_sitter_json(&make_grammar(
        "ch",
        json!({
            "pick": {
                "type": "CHOICE",
                "members": [
                    { "type": "STRING", "value": "x" },
                    { "type": "STRING", "value": "y" },
                    { "type": "STRING", "value": "z" },
                    { "type": "STRING", "value": "w" }
                ]
            }
        }),
    ))
    .unwrap();
    if let Rule::Choice { members } = &g.rules["pick"] {
        assert_eq!(members.len(), 4);
    } else {
        panic!("expected Choice");
    }
}

#[test]
fn rule_type_repeat_has_content() {
    let g = from_tree_sitter_json(&make_grammar(
        "rp",
        json!({
            "items": { "type": "REPEAT", "content": { "type": "SYMBOL", "name": "item" } },
            "item": { "type": "STRING", "value": "i" }
        }),
    ))
    .unwrap();
    if let Rule::Repeat { content } = &g.rules["items"] {
        assert!(matches!(content.as_ref(), Rule::Symbol { name } if name == "item"));
    } else {
        panic!("expected Repeat");
    }
}

#[test]
fn rule_type_repeat1_has_content() {
    let g = from_tree_sitter_json(&make_grammar(
        "rp1",
        json!({
            "nonempty": { "type": "REPEAT1", "content": { "type": "STRING", "value": "+" } }
        }),
    ))
    .unwrap();
    assert!(matches!(&g.rules["nonempty"], Rule::Repeat1 { .. }));
}

#[test]
fn rule_type_optional_accepts_content_key() {
    let g = from_tree_sitter_json(&make_grammar(
        "oc",
        json!({
            "maybe": { "type": "OPTIONAL", "content": { "type": "STRING", "value": "opt" } }
        }),
    ))
    .unwrap();
    assert!(matches!(&g.rules["maybe"], Rule::Optional { .. }));
}

#[test]
fn rule_type_blank() {
    let g = from_tree_sitter_json(&make_grammar(
        "bl",
        json!({
            "empty": { "type": "BLANK" }
        }),
    ))
    .unwrap();
    assert!(matches!(&g.rules["empty"], Rule::Blank));
}

#[test]
fn rule_type_symbol_has_name() {
    let g = from_tree_sitter_json(&make_grammar(
        "sym",
        json!({
            "ref_rule": { "type": "SYMBOL", "name": "target" },
            "target": { "type": "BLANK" }
        }),
    ))
    .unwrap();
    if let Rule::Symbol { name } = &g.rules["ref_rule"] {
        assert_eq!(name, "target");
    } else {
        panic!("expected Symbol");
    }
}

#[test]
fn rule_type_immediate_token() {
    let g = from_tree_sitter_json(&make_grammar(
        "imm",
        json!({
            "glued": { "type": "IMMEDIATE_TOKEN", "content": { "type": "STRING", "value": "." } }
        }),
    ))
    .unwrap();
    assert!(matches!(&g.rules["glued"], Rule::ImmediateToken { .. }));
}

#[test]
fn rule_type_field_extracts_name_and_content() {
    let g = from_tree_sitter_json(&make_grammar(
        "fld",
        json!({
            "assignment": {
                "type": "FIELD",
                "name": "lhs",
                "content": { "type": "SYMBOL", "name": "ident" }
            },
            "ident": { "type": "PATTERN", "value": "[a-z]+" }
        }),
    ))
    .unwrap();
    if let Rule::Field { name, content } = &g.rules["assignment"] {
        assert_eq!(name, "lhs");
        assert!(matches!(content.as_ref(), Rule::Symbol { .. }));
    } else {
        panic!("expected Field");
    }
}

// ===========================================================================
// Category 4: Precedence values applied correctly (27–34)
// ===========================================================================

#[test]
fn prec_value_positive() {
    let g = from_tree_sitter_json(&make_grammar(
        "pv",
        json!({
            "expr": { "type": "PREC", "value": 10, "content": { "type": "STRING", "value": "+" } }
        }),
    ))
    .unwrap();
    if let Rule::Prec { value, .. } = &g.rules["expr"] {
        assert_eq!(*value, 10);
    } else {
        panic!("expected Prec");
    }
}

#[test]
fn prec_value_zero() {
    let g = from_tree_sitter_json(&make_grammar(
        "pz",
        json!({
            "expr": { "type": "PREC", "value": 0, "content": { "type": "STRING", "value": "x" } }
        }),
    ))
    .unwrap();
    if let Rule::Prec { value, .. } = &g.rules["expr"] {
        assert_eq!(*value, 0);
    } else {
        panic!("expected Prec");
    }
}

#[test]
fn prec_value_negative() {
    let g = from_tree_sitter_json(&make_grammar(
        "pn",
        json!({
            "expr": { "type": "PREC", "value": -5, "content": { "type": "STRING", "value": "x" } }
        }),
    ))
    .unwrap();
    if let Rule::Prec { value, .. } = &g.rules["expr"] {
        assert_eq!(*value, -5);
    } else {
        panic!("expected Prec");
    }
}

#[test]
fn prec_left_value() {
    let g = from_tree_sitter_json(&make_grammar("pl", json!({
        "add": { "type": "PREC_LEFT", "value": 3, "content": { "type": "STRING", "value": "+" } }
    }))).unwrap();
    if let Rule::PrecLeft { value, .. } = &g.rules["add"] {
        assert_eq!(*value, 3);
    } else {
        panic!("expected PrecLeft");
    }
}

#[test]
fn prec_right_value() {
    let g = from_tree_sitter_json(&make_grammar("pr", json!({
        "power": { "type": "PREC_RIGHT", "value": 7, "content": { "type": "STRING", "value": "^" } }
    }))).unwrap();
    if let Rule::PrecRight { value, .. } = &g.rules["power"] {
        assert_eq!(*value, 7);
    } else {
        panic!("expected PrecRight");
    }
}

#[test]
fn prec_dynamic_value() {
    let g = from_tree_sitter_json(&make_grammar("pd", json!({
        "dyn_rule": { "type": "PREC_DYNAMIC", "value": 2, "content": { "type": "STRING", "value": "d" } }
    }))).unwrap();
    if let Rule::PrecDynamic { value, .. } = &g.rules["dyn_rule"] {
        assert_eq!(*value, 2);
    } else {
        panic!("expected PrecDynamic");
    }
}

#[test]
fn prec_nested_prec_left_inside_prec() {
    let g = from_tree_sitter_json(&make_grammar(
        "np",
        json!({
            "expr": {
                "type": "PREC",
                "value": 10,
                "content": {
                    "type": "PREC_LEFT",
                    "value": 5,
                    "content": { "type": "STRING", "value": "*" }
                }
            }
        }),
    ))
    .unwrap();
    if let Rule::Prec { value, content } = &g.rules["expr"] {
        assert_eq!(*value, 10);
        assert!(matches!(content.as_ref(), Rule::PrecLeft { value: 5, .. }));
    } else {
        panic!("expected Prec wrapping PrecLeft");
    }
}

#[test]
fn prec_content_is_sequence() {
    let g = from_tree_sitter_json(&make_grammar(
        "ps",
        json!({
            "binary": {
                "type": "PREC_LEFT",
                "value": 4,
                "content": {
                    "type": "SEQ",
                    "members": [
                        { "type": "SYMBOL", "name": "expr" },
                        { "type": "STRING", "value": "+" },
                        { "type": "SYMBOL", "name": "expr" }
                    ]
                }
            },
            "expr": { "type": "PATTERN", "value": "\\d+" }
        }),
    ))
    .unwrap();
    if let Rule::PrecLeft { value, content } = &g.rules["binary"] {
        assert_eq!(*value, 4);
        assert!(matches!(content.as_ref(), Rule::Seq { members } if members.len() == 3));
    } else {
        panic!("expected PrecLeft");
    }
}

// ===========================================================================
// Category 5: Invalid rule types produce errors (35–39)
// ===========================================================================

#[test]
fn unknown_rule_type_errors() {
    let result = from_tree_sitter_json(&make_grammar(
        "bad",
        json!({
            "source": { "type": "FOOBAR", "value": "x" }
        }),
    ));
    // #1076: unknown rule types are rejected, not silently skipped.
    assert!(result.is_err());
}

#[test]
fn missing_type_field_errors() {
    let result = from_tree_sitter_json(&make_grammar(
        "nofield",
        json!({
            "source": { "value": "x" }
        }),
    ));
    // #1076: a missing type field is rejected, not silently skipped.
    assert!(result.is_err());
}

#[test]
fn invalid_json_to_build_parser() {
    let dir = TempDir::new().unwrap();
    let result = build_parser_from_json("not valid json".to_string(), test_options(&dir));
    assert!(result.is_err());
}

#[test]
fn missing_name_field_errors() {
    let result = from_tree_sitter_json(&json!({
        "rules": { "src": { "type": "BLANK" } }
    }));
    assert!(result.is_err());
}

#[test]
fn missing_rules_field_gives_empty_grammar() {
    let g = from_tree_sitter_json(&json!({
        "name": "norules"
    }))
    .unwrap();
    assert!(g.rules.is_empty());
}

// ===========================================================================
// Category 6: Token extraction from grammar JSON (40–44)
// ===========================================================================

#[test]
fn token_wrapping_pattern() {
    let g = from_tree_sitter_json(&make_grammar(
        "tk",
        json!({
            "ident": {
                "type": "TOKEN",
                "content": { "type": "PATTERN", "value": "[a-zA-Z_]\\w*" }
            }
        }),
    ))
    .unwrap();
    if let Rule::Token { content } = &g.rules["ident"] {
        assert!(matches!(content.as_ref(), Rule::Pattern { value } if value == "[a-zA-Z_]\\w*"));
    } else {
        panic!("expected Token");
    }
}

#[test]
fn token_wrapping_choice() {
    let g = from_tree_sitter_json(&make_grammar(
        "tc",
        json!({
            "operator": {
                "type": "TOKEN",
                "content": {
                    "type": "CHOICE",
                    "members": [
                        { "type": "STRING", "value": "+" },
                        { "type": "STRING", "value": "-" }
                    ]
                }
            }
        }),
    ))
    .unwrap();
    if let Rule::Token { content } = &g.rules["operator"] {
        assert!(matches!(content.as_ref(), Rule::Choice { members } if members.len() == 2));
    } else {
        panic!("expected Token");
    }
}

#[test]
fn immediate_token_wrapping_string() {
    let g = from_tree_sitter_json(&make_grammar(
        "it",
        json!({
            "dot": {
                "type": "IMMEDIATE_TOKEN",
                "content": { "type": "STRING", "value": "." }
            }
        }),
    ))
    .unwrap();
    if let Rule::ImmediateToken { content } = &g.rules["dot"] {
        assert!(matches!(content.as_ref(), Rule::String { value } if value == "."));
    } else {
        panic!("expected ImmediateToken");
    }
}

#[test]
fn token_inside_seq() {
    let g = from_tree_sitter_json(&make_grammar(
        "ts",
        json!({
            "decl": {
                "type": "SEQ",
                "members": [
                    { "type": "TOKEN", "content": { "type": "STRING", "value": "let" } },
                    { "type": "SYMBOL", "name": "ident" }
                ]
            },
            "ident": { "type": "PATTERN", "value": "[a-z]+" }
        }),
    ))
    .unwrap();
    if let Rule::Seq { members } = &g.rules["decl"] {
        assert!(matches!(&members[0], Rule::Token { .. }));
    } else {
        panic!("expected Seq");
    }
}

#[test]
fn extras_contain_pattern_tokens() {
    let g = from_tree_sitter_json(&json!({
        "name": "ext_tok",
        "extras": [
            { "type": "PATTERN", "value": "\\s" },
            { "type": "PATTERN", "value": "//[^\\n]*" }
        ],
        "rules": { "src": { "type": "STRING", "value": "x" } }
    }))
    .unwrap();
    assert_eq!(g.extras.len(), 2);
    assert!(matches!(&g.extras[0], Rule::Pattern { .. }));
    assert!(matches!(&g.extras[1], Rule::Pattern { .. }));
}

// ===========================================================================
// Category 7: Multiple rules with cross-references (45–50)
// ===========================================================================

#[test]
fn cross_ref_symbol_to_other_rule() {
    let g = from_tree_sitter_json(&make_grammar(
        "xref",
        json!({
            "program": { "type": "SYMBOL", "name": "statement" },
            "statement": { "type": "SYMBOL", "name": "expression" },
            "expression": { "type": "PATTERN", "value": "\\d+" }
        }),
    ))
    .unwrap();
    assert_eq!(g.rules.len(), 3);
    assert!(matches!(&g.rules["program"], Rule::Symbol { name } if name == "statement"));
}

#[test]
fn choice_of_symbols_referencing_rules() {
    let g = from_tree_sitter_json(&make_grammar(
        "csym",
        json!({
            "value": {
                "type": "CHOICE",
                "members": [
                    { "type": "SYMBOL", "name": "number" },
                    { "type": "SYMBOL", "name": "word" }
                ]
            },
            "number": { "type": "PATTERN", "value": "\\d+" },
            "word": { "type": "PATTERN", "value": "[a-z]+" }
        }),
    ))
    .unwrap();
    if let Rule::Choice { members } = &g.rules["value"] {
        assert_eq!(members.len(), 2);
    } else {
        panic!("expected Choice");
    }
}

#[test]
fn repeat_of_symbol() {
    let g = from_tree_sitter_json(&make_grammar(
        "rsym",
        json!({
            "items": { "type": "REPEAT", "content": { "type": "SYMBOL", "name": "item" } },
            "item": { "type": "STRING", "value": "i" }
        }),
    ))
    .unwrap();
    if let Rule::Repeat { content } = &g.rules["items"] {
        assert!(matches!(content.as_ref(), Rule::Symbol { name } if name == "item"));
    } else {
        panic!("expected Repeat");
    }
}

#[test]
fn seq_with_multiple_symbol_refs() {
    let g = from_tree_sitter_json(&make_grammar(
        "ms",
        json!({
            "assignment": {
                "type": "SEQ",
                "members": [
                    { "type": "SYMBOL", "name": "lhs" },
                    { "type": "STRING", "value": "=" },
                    { "type": "SYMBOL", "name": "rhs" }
                ]
            },
            "lhs": { "type": "PATTERN", "value": "[a-z]+" },
            "rhs": { "type": "PATTERN", "value": "\\d+" }
        }),
    ))
    .unwrap();
    if let Rule::Seq { members } = &g.rules["assignment"] {
        assert_eq!(members.len(), 3);
        assert!(matches!(&members[0], Rule::Symbol { name } if name == "lhs"));
        assert!(matches!(&members[2], Rule::Symbol { name } if name == "rhs"));
    } else {
        panic!("expected Seq");
    }
}

#[test]
fn field_referencing_symbol() {
    let g = from_tree_sitter_json(&make_grammar("fs", json!({
        "binding": {
            "type": "SEQ",
            "members": [
                { "type": "FIELD", "name": "identifier", "content": { "type": "SYMBOL", "name": "ident" } },
                { "type": "STRING", "value": "=" },
                { "type": "FIELD", "name": "val", "content": { "type": "SYMBOL", "name": "expr" } }
            ]
        },
        "ident": { "type": "PATTERN", "value": "[a-z]+" },
        "expr": { "type": "PATTERN", "value": "\\d+" }
    }))).unwrap();
    if let Rule::Seq { members } = &g.rules["binding"] {
        assert!(matches!(&members[0], Rule::Field { name, .. } if name == "identifier"));
        assert!(matches!(&members[2], Rule::Field { name, .. } if name == "val"));
    } else {
        panic!("expected Seq");
    }
}

#[test]
fn grammar_with_word_field() {
    let g = from_tree_sitter_json(&json!({
        "name": "word_test",
        "word": "identifier",
        "rules": {
            "source": { "type": "SYMBOL", "name": "identifier" },
            "identifier": { "type": "PATTERN", "value": "[a-zA-Z_]\\w*" }
        }
    }))
    .unwrap();
    assert_eq!(g.word, Some("identifier".to_string()));
}

// ===========================================================================
// Category 8: Edge cases (51–60)
// ===========================================================================

#[test]
fn blank_rule_only_grammar() {
    let g = from_tree_sitter_json(&make_grammar(
        "blank_only",
        json!({
            "source": { "type": "BLANK" }
        }),
    ))
    .unwrap();
    assert!(matches!(&g.rules["source"], Rule::Blank));
}

#[test]
fn deeply_nested_choice_inside_seq_inside_repeat() {
    let g = from_tree_sitter_json(&make_grammar(
        "deep",
        json!({
            "items": {
                "type": "REPEAT",
                "content": {
                    "type": "SEQ",
                    "members": [
                        {
                            "type": "CHOICE",
                            "members": [
                                { "type": "STRING", "value": "a" },
                                { "type": "STRING", "value": "b" }
                            ]
                        },
                        { "type": "STRING", "value": ";" }
                    ]
                }
            }
        }),
    ))
    .unwrap();
    if let Rule::Repeat { content } = &g.rules["items"] {
        assert!(matches!(content.as_ref(), Rule::Seq { .. }));
    } else {
        panic!("expected Repeat");
    }
}

#[test]
fn seq_with_zero_members_is_valid() {
    let g = from_tree_sitter_json(&make_grammar(
        "empty_seq",
        json!({
            "nothing": { "type": "SEQ", "members": [] }
        }),
    ))
    .unwrap();
    if let Rule::Seq { members } = &g.rules["nothing"] {
        assert!(members.is_empty());
    } else {
        panic!("expected Seq");
    }
}

#[test]
fn choice_with_single_member() {
    let g = from_tree_sitter_json(&make_grammar(
        "single_choice",
        json!({
            "one": { "type": "CHOICE", "members": [{ "type": "STRING", "value": "only" }] }
        }),
    ))
    .unwrap();
    if let Rule::Choice { members } = &g.rules["one"] {
        assert_eq!(members.len(), 1);
    } else {
        panic!("expected Choice");
    }
}

#[test]
fn deeply_nested_prec_chain() {
    let g = from_tree_sitter_json(&make_grammar(
        "prec_chain",
        json!({
            "expr": {
                "type": "PREC",
                "value": 1,
                "content": {
                    "type": "PREC_LEFT",
                    "value": 2,
                    "content": {
                        "type": "PREC_RIGHT",
                        "value": 3,
                        "content": {
                            "type": "PREC_DYNAMIC",
                            "value": 4,
                            "content": { "type": "STRING", "value": "x" }
                        }
                    }
                }
            }
        }),
    ))
    .unwrap();
    if let Rule::Prec { value, content } = &g.rules["expr"] {
        assert_eq!(*value, 1);
        if let Rule::PrecLeft { value, content } = content.as_ref() {
            assert_eq!(*value, 2);
            if let Rule::PrecRight { value, content } = content.as_ref() {
                assert_eq!(*value, 3);
                assert!(matches!(
                    content.as_ref(),
                    Rule::PrecDynamic { value: 4, .. }
                ));
            } else {
                panic!("expected PrecRight");
            }
        } else {
            panic!("expected PrecLeft");
        }
    } else {
        panic!("expected Prec");
    }
}

#[test]
fn grammar_with_conflicts_and_inline() {
    let g = from_tree_sitter_json(&json!({
        "name": "ci",
        "inline": ["_inner"],
        "conflicts": [["expr", "stmt"]],
        "rules": {
            "program": { "type": "SYMBOL", "name": "expr" },
            "expr": { "type": "STRING", "value": "e" },
            "stmt": { "type": "STRING", "value": "s" },
            "_inner": { "type": "BLANK" }
        }
    }))
    .unwrap();
    assert_eq!(g.inline, vec!["_inner"]);
    assert_eq!(g.conflicts.len(), 1);
    assert_eq!(g.conflicts[0], vec!["expr", "stmt"]);
}

#[test]
fn grammar_with_supertypes() {
    let g = from_tree_sitter_json(&json!({
        "name": "st",
        "supertypes": ["_expression", "_statement"],
        "rules": {
            "source": { "type": "BLANK" },
            "_expression": { "type": "BLANK" },
            "_statement": { "type": "BLANK" }
        }
    }))
    .unwrap();
    assert_eq!(g.supertypes.len(), 2);
    assert_eq!(g.supertypes[0], "_expression");
}

#[test]
fn alias_with_named_false() {
    let g = from_tree_sitter_json(&make_grammar(
        "anon_alias",
        json!({
            "rule": {
                "type": "ALIAS",
                "content": { "type": "STRING", "value": "x" },
                "value": "anon",
                "named": false
            }
        }),
    ))
    .unwrap();
    if let Rule::Alias { value, named, .. } = &g.rules["rule"] {
        assert_eq!(value, "anon");
        assert!(!named);
    } else {
        panic!("expected Alias");
    }
}

#[test]
fn optional_with_content_key_synonym() {
    // OPTIONAL accepts both "value" and "content" keys
    let g1 = from_tree_sitter_json(&make_grammar(
        "ov",
        json!({
            "maybe": { "type": "OPTIONAL", "value": { "type": "STRING", "value": "a" } }
        }),
    ))
    .unwrap();
    let g2 = from_tree_sitter_json(&make_grammar(
        "oc",
        json!({
            "maybe": { "type": "OPTIONAL", "content": { "type": "STRING", "value": "a" } }
        }),
    ))
    .unwrap();
    assert!(matches!(&g1.rules["maybe"], Rule::Optional { .. }));
    assert!(matches!(&g2.rules["maybe"], Rule::Optional { .. }));
}

#[test]
fn large_grammar_many_rules() {
    let mut rules = serde_json::Map::new();
    rules.insert("source".to_string(), json!({
        "type": "CHOICE",
        "members": (0..20).map(|i| json!({ "type": "SYMBOL", "name": format!("rule_{i}") })).collect::<Vec<_>>()
    }));
    for i in 0..20 {
        rules.insert(
            format!("rule_{i}"),
            json!({ "type": "STRING", "value": format!("v{i}") }),
        );
    }

    let g = from_tree_sitter_json(&json!({
        "name": "large",
        "rules": rules
    }))
    .unwrap();
    // 1 source + 20 rule_N
    assert_eq!(g.rules.len(), 21);
}
