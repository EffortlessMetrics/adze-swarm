//! Comprehensive tests for the grammar_js module's public API.
//!
//! Covers JSON parsing (`from_json`, `from_tree_sitter_json`), grammar validation,
//! helper function evaluation, converter to IR, and edge/error cases.

use adze_tool::grammar_js::helpers::HelperFunctions;
use adze_tool::grammar_js::json_converter::from_tree_sitter_json;
use adze_tool::grammar_js::{ExternalToken, GrammarJs, GrammarJsConverter, Rule, from_json};
use serde_json::json;

// ---------------------------------------------------------------------------
// 1. JSON grammar parsing — basic & structural
// ---------------------------------------------------------------------------

#[test]
fn json_parse_minimal_grammar() {
    let v = json!({
        "name": "minimal",
        "rules": {
            "start": { "type": "STRING", "value": "x" }
        }
    });
    let g = from_json(&v).unwrap();
    assert_eq!(g.name, "minimal");
    assert_eq!(g.rules.len(), 1);
    assert!(g.word.is_none());
    assert!(g.extras.is_empty());
    assert!(g.externals.is_empty());
    assert!(g.inline.is_empty());
    assert!(g.conflicts.is_empty());
    assert!(g.supertypes.is_empty());
}

#[test]
fn json_parse_word_token() {
    let v = json!({
        "name": "lang",
        "word": "identifier",
        "rules": {
            "identifier": { "type": "PATTERN", "value": "[a-z]+" }
        }
    });
    let g = from_json(&v).unwrap();
    assert_eq!(g.word, Some("identifier".to_string()));
}

#[test]
fn json_parse_extras() {
    let v = json!({
        "name": "lang",
        "rules": { "s": { "type": "BLANK" } },
        "extras": [
            { "type": "PATTERN", "value": "\\s" },
            { "type": "SYMBOL", "name": "comment" }
        ]
    });
    let g = from_json(&v).unwrap();
    assert_eq!(g.extras.len(), 2);
}

#[test]
fn json_parse_inline_rules() {
    let v = json!({
        "name": "lang",
        "inline": ["_expr", "_stmt"],
        "rules": {
            "_expr": { "type": "BLANK" },
            "_stmt": { "type": "BLANK" }
        }
    });
    let g = from_json(&v).unwrap();
    assert_eq!(g.inline, vec!["_expr", "_stmt"]);
}

#[test]
fn json_parse_conflicts() {
    let v = json!({
        "name": "lang",
        "conflicts": [["a", "b"], ["c"]],
        "rules": {
            "a": { "type": "BLANK" },
            "b": { "type": "BLANK" },
            "c": { "type": "BLANK" }
        }
    });
    let g = from_json(&v).unwrap();
    assert_eq!(g.conflicts.len(), 2);
    assert_eq!(g.conflicts[0], vec!["a", "b"]);
}

#[test]
fn json_parse_supertypes() {
    let v = json!({
        "name": "lang",
        "supertypes": ["_expression", "_statement"],
        "rules": {
            "_expression": { "type": "BLANK" },
            "_statement": { "type": "BLANK" }
        }
    });
    let g = from_json(&v).unwrap();
    assert_eq!(g.supertypes, vec!["_expression", "_statement"]);
}

#[test]
fn json_parse_externals() {
    let v = json!({
        "name": "lang",
        "externals": [
            { "name": "indent", "type": "SYMBOL" },
            { "name": "dedent", "type": "SYMBOL" }
        ],
        "rules": { "s": { "type": "BLANK" } }
    });
    let g = from_json(&v).unwrap();
    assert_eq!(g.externals.len(), 2);
    assert_eq!(g.externals[0].name, "indent");
    assert_eq!(g.externals[1].name, "dedent");
}

// ---------------------------------------------------------------------------
// 2. Rule type parsing — every variant via JSON
// ---------------------------------------------------------------------------

#[test]
fn json_parse_all_simple_rule_types() {
    let v = json!({
        "name": "all_rules",
        "rules": {
            "r_string": { "type": "STRING", "value": "hello" },
            "r_pattern": { "type": "PATTERN", "value": "\\d+" },
            "r_blank": { "type": "BLANK" },
            "r_symbol": { "type": "SYMBOL", "name": "r_blank" }
        }
    });
    let g = from_json(&v).unwrap();
    assert!(matches!(g.rules["r_string"], Rule::String { ref value } if value == "hello"));
    assert!(matches!(g.rules["r_pattern"], Rule::Pattern { ref value } if value == "\\d+"));
    assert!(matches!(g.rules["r_blank"], Rule::Blank));
    assert!(matches!(g.rules["r_symbol"], Rule::Symbol { ref name } if name == "r_blank"));
}

#[test]
fn json_parse_seq_and_choice() {
    let v = json!({
        "name": "g",
        "rules": {
            "r_seq": {
                "type": "SEQ",
                "members": [
                    { "type": "STRING", "value": "a" },
                    { "type": "STRING", "value": "b" }
                ]
            },
            "r_choice": {
                "type": "CHOICE",
                "members": [
                    { "type": "STRING", "value": "x" },
                    { "type": "STRING", "value": "y" }
                ]
            }
        }
    });
    let g = from_json(&v).unwrap();
    if let Rule::Seq { ref members } = g.rules["r_seq"] {
        assert_eq!(members.len(), 2);
    } else {
        panic!("expected Seq");
    }
    if let Rule::Choice { ref members } = g.rules["r_choice"] {
        assert_eq!(members.len(), 2);
    } else {
        panic!("expected Choice");
    }
}

#[test]
fn json_parse_repeat_repeat1_optional() {
    let v = json!({
        "name": "g",
        "rules": {
            "r_rep": {
                "type": "REPEAT",
                "content": { "type": "STRING", "value": "a" }
            },
            "r_rep1": {
                "type": "REPEAT1",
                "content": { "type": "STRING", "value": "b" }
            },
            "r_opt": {
                "type": "OPTIONAL",
                "value": { "type": "STRING", "value": "c" }
            }
        }
    });
    let g = from_json(&v).unwrap();
    assert!(matches!(g.rules["r_rep"], Rule::Repeat { .. }));
    assert!(matches!(g.rules["r_rep1"], Rule::Repeat1 { .. }));
    assert!(matches!(g.rules["r_opt"], Rule::Optional { .. }));
}

#[test]
fn json_parse_precedence_rules() {
    let v = json!({
        "name": "g",
        "rules": {
            "p": {
                "type": "PREC",
                "value": 5,
                "content": { "type": "BLANK" }
            },
            "pl": {
                "type": "PREC_LEFT",
                "value": 3,
                "content": { "type": "BLANK" }
            },
            "pr": {
                "type": "PREC_RIGHT",
                "value": -1,
                "content": { "type": "BLANK" }
            },
            "pd": {
                "type": "PREC_DYNAMIC",
                "value": 2,
                "content": { "type": "BLANK" }
            }
        }
    });
    let g = from_json(&v).unwrap();
    assert!(matches!(g.rules["p"], Rule::Prec { value: 5, .. }));
    assert!(matches!(g.rules["pl"], Rule::PrecLeft { value: 3, .. }));
    assert!(matches!(g.rules["pr"], Rule::PrecRight { value: -1, .. }));
    assert!(matches!(g.rules["pd"], Rule::PrecDynamic { value: 2, .. }));
}

#[test]
fn json_parse_token_and_immediate_token() {
    let v = json!({
        "name": "g",
        "rules": {
            "t": {
                "type": "TOKEN",
                "content": { "type": "PATTERN", "value": "\\w+" }
            },
            "it": {
                "type": "IMMEDIATE_TOKEN",
                "content": { "type": "STRING", "value": "." }
            }
        }
    });
    let g = from_json(&v).unwrap();
    assert!(matches!(g.rules["t"], Rule::Token { .. }));
    assert!(matches!(g.rules["it"], Rule::ImmediateToken { .. }));
}

#[test]
fn json_parse_field_rule() {
    let v = json!({
        "name": "g",
        "rules": {
            "assignment": {
                "type": "FIELD",
                "name": "left",
                "content": { "type": "SYMBOL", "name": "ident" }
            },
            "ident": { "type": "PATTERN", "value": "[a-z]+" }
        }
    });
    let g = from_json(&v).unwrap();
    if let Rule::Field { ref name, .. } = g.rules["assignment"] {
        assert_eq!(name, "left");
    } else {
        panic!("expected Field");
    }
}

#[test]
fn json_parse_alias_rule() {
    let v = json!({
        "name": "g",
        "rules": {
            "r": {
                "type": "ALIAS",
                "content": { "type": "STRING", "value": "fn" },
                "value": "function_keyword",
                "named": true
            }
        }
    });
    let g = from_json(&v).unwrap();
    if let Rule::Alias {
        ref value, named, ..
    } = g.rules["r"]
    {
        assert_eq!(value, "function_keyword");
        assert!(named);
    } else {
        panic!("expected Alias");
    }
}

// ---------------------------------------------------------------------------
// 3. JSON parsing — error / edge cases
// ---------------------------------------------------------------------------

#[test]
fn json_parse_error_not_object() {
    let v = json!("not an object");
    assert!(from_json(&v).is_err());
}

#[test]
fn json_parse_error_missing_name() {
    let v = json!({ "rules": {} });
    assert!(from_json(&v).is_err());
}

#[test]
fn json_parse_unknown_rule_type_skipped() {
    // #1076: unknown rule types are rejected, not silently skipped.
    let v = json!({
        "name": "g",
        "rules": {
            "good": { "type": "BLANK" },
            "bad":  { "type": "TOTALLY_INVENTED" }
        }
    });
    let err = from_json(&v).expect_err("unknown rule type must fail");
    assert!(
        format!("{err:#}").contains("bad"),
        "error should name the malformed rule: {err:#}"
    );
}

#[test]
fn json_parse_empty_rules_object() {
    let v = json!({ "name": "empty", "rules": {} });
    let g = from_json(&v).unwrap();
    assert_eq!(g.rules.len(), 0);
}

#[test]
fn json_parse_no_rules_key_at_all() {
    // If "rules" is absent entirely the grammar should still parse (empty rules map)
    let v = json!({ "name": "norules" });
    let g = from_json(&v).unwrap();
    assert!(g.rules.is_empty());
}

// ---------------------------------------------------------------------------
// 4. GrammarJs validation
// ---------------------------------------------------------------------------

#[test]
fn validate_valid_grammar() {
    let mut g = GrammarJs::new("ok".into());
    g.rules.insert(
        "start".into(),
        Rule::Symbol {
            name: "ident".into(),
        },
    );
    g.rules
        .insert("ident".into(), Rule::Pattern { value: "x".into() });
    assert!(g.validate().is_ok());
}

#[test]
fn validate_missing_symbol_reference() {
    let mut g = GrammarJs::new("bad".into());
    g.rules.insert(
        "start".into(),
        Rule::Symbol {
            name: "nonexistent".into(),
        },
    );
    let err = g.validate().unwrap_err();
    assert!(
        err.to_string().contains("nonexistent"),
        "error should mention missing symbol: {err}"
    );
}

#[test]
fn validate_word_token_missing() {
    let mut g = GrammarJs::new("bad".into());
    g.word = Some("missing_word".into());
    g.rules
        .insert("start".into(), Rule::String { value: "x".into() });
    let err = g.validate().unwrap_err();
    assert!(err.to_string().contains("missing_word"));
}

#[test]
fn validate_inline_rule_missing() {
    let mut g = GrammarJs::new("bad".into());
    g.inline.push("ghost".into());
    g.rules
        .insert("start".into(), Rule::String { value: "x".into() });
    let err = g.validate().unwrap_err();
    assert!(err.to_string().contains("ghost"));
}

#[test]
fn validate_conflict_rule_missing() {
    let mut g = GrammarJs::new("bad".into());
    g.conflicts.push(vec!["phantom".into()]);
    g.rules
        .insert("start".into(), Rule::String { value: "x".into() });
    let err = g.validate().unwrap_err();
    assert!(err.to_string().contains("phantom"));
}

#[test]
fn validate_external_token_allows_symbol() {
    let mut g = GrammarJs::new("ext".into());
    g.externals.push(ExternalToken {
        name: "indent".into(),
        symbol: "external_0".into(),
    });
    g.rules.insert(
        "start".into(),
        Rule::Symbol {
            name: "indent".into(),
        },
    );
    // "indent" is external, so this should be valid
    assert!(g.validate().is_ok());
}

#[test]
fn validate_nested_missing_symbol() {
    let mut g = GrammarJs::new("nested".into());
    g.rules.insert(
        "start".into(),
        Rule::Seq {
            members: vec![
                Rule::String { value: "a".into() },
                Rule::Optional {
                    value: Box::new(Rule::Symbol {
                        name: "missing".into(),
                    }),
                },
            ],
        },
    );
    assert!(g.validate().is_err());
}

// ---------------------------------------------------------------------------
// 5. HelperFunctions
// ---------------------------------------------------------------------------

#[test]
fn helper_is_helper_function() {
    assert!(HelperFunctions::is_helper_function("commaSep"));
    assert!(HelperFunctions::is_helper_function("commaSep1"));
    assert!(HelperFunctions::is_helper_function("sep"));
    assert!(HelperFunctions::is_helper_function("sep1"));
    assert!(HelperFunctions::is_helper_function("parens"));
    assert!(HelperFunctions::is_helper_function("brackets"));
    assert!(HelperFunctions::is_helper_function("braces"));
    assert!(!HelperFunctions::is_helper_function("unknown_fn"));
    assert!(!HelperFunctions::is_helper_function(""));
}

#[test]
fn helper_comma_sep() {
    let rule = Rule::Symbol {
        name: "item".into(),
    };
    let result = HelperFunctions::evaluate_helper("commaSep", vec![rule]).unwrap();
    // commaSep produces Optional(Seq([item, Repeat(Seq([",", item]))]))
    assert!(matches!(result, Rule::Optional { .. }));
}

#[test]
fn helper_comma_sep1() {
    let rule = Rule::Symbol {
        name: "item".into(),
    };
    let result = HelperFunctions::evaluate_helper("commaSep1", vec![rule]).unwrap();
    // commaSep1 produces Seq([item, Repeat(Seq([",", item]))])
    assert!(matches!(result, Rule::Seq { .. }));
}

#[test]
fn helper_sep_with_custom_separator() {
    let rule = Rule::Symbol {
        name: "item".into(),
    };
    let sep = Rule::String { value: ";".into() };
    let result = HelperFunctions::evaluate_helper("sep", vec![rule, sep]).unwrap();
    assert!(matches!(result, Rule::Optional { .. }));
}

#[test]
fn helper_parens_wraps_in_parens() {
    let inner = Rule::Symbol {
        name: "expr".into(),
    };
    let result = HelperFunctions::evaluate_helper("parens", vec![inner]).unwrap();
    if let Rule::Seq { ref members } = result {
        assert_eq!(members.len(), 3);
        assert!(matches!(&members[0], Rule::String { value } if value == "("));
        assert!(matches!(&members[2], Rule::String { value } if value == ")"));
    } else {
        panic!("expected Seq for parens");
    }
}

#[test]
fn helper_brackets_wraps_in_brackets() {
    let inner = Rule::Symbol { name: "arr".into() };
    let result = HelperFunctions::evaluate_helper("brackets", vec![inner]).unwrap();
    if let Rule::Seq { ref members } = result {
        assert!(matches!(&members[0], Rule::String { value } if value == "["));
        assert!(matches!(&members[2], Rule::String { value } if value == "]"));
    } else {
        panic!("expected Seq for brackets");
    }
}

#[test]
fn helper_braces_wraps_in_braces() {
    let inner = Rule::Symbol {
        name: "block".into(),
    };
    let result = HelperFunctions::evaluate_helper("braces", vec![inner]).unwrap();
    if let Rule::Seq { ref members } = result {
        assert!(matches!(&members[0], Rule::String { value } if value == "{"));
        assert!(matches!(&members[2], Rule::String { value } if value == "}"));
    } else {
        panic!("expected Seq for braces");
    }
}

#[test]
fn helper_wrong_arg_count_errors() {
    // commaSep needs exactly 1 arg
    assert!(HelperFunctions::evaluate_helper("commaSep", vec![]).is_err());
    assert!(HelperFunctions::evaluate_helper("commaSep", vec![Rule::Blank, Rule::Blank]).is_err());

    // sep needs exactly 2 args
    assert!(HelperFunctions::evaluate_helper("sep", vec![Rule::Blank]).is_err());

    // parens needs exactly 1 arg
    assert!(HelperFunctions::evaluate_helper("parens", vec![]).is_err());
}

#[test]
fn helper_unknown_function_errors() {
    assert!(HelperFunctions::evaluate_helper("doesNotExist", vec![]).is_err());
}

// ---------------------------------------------------------------------------
// 6. GrammarJsConverter — round-trip from JSON → GrammarJs → IR
// ---------------------------------------------------------------------------

#[test]
fn converter_minimal_grammar_to_ir() {
    let v = json!({
        "name": "tiny",
        "rules": {
            "source": { "type": "STRING", "value": "hello" }
        }
    });
    let g = from_tree_sitter_json(&v).unwrap();
    let converter = GrammarJsConverter::new(g);
    let ir = converter.convert();
    // Conversion may succeed or fail depending on IR requirements,
    // but it must not panic.
    let _ = ir;
}

#[test]
fn converter_grammar_with_multiple_rules() {
    let v = json!({
        "name": "multi",
        "rules": {
            "program": {
                "type": "REPEAT",
                "content": { "type": "SYMBOL", "name": "statement" }
            },
            "statement": {
                "type": "CHOICE",
                "members": [
                    { "type": "SYMBOL", "name": "number" },
                    { "type": "SYMBOL", "name": "word" }
                ]
            },
            "number": { "type": "PATTERN", "value": "\\d+" },
            "word":   { "type": "PATTERN", "value": "[a-z]+" }
        }
    });
    let g = from_tree_sitter_json(&v).unwrap();
    assert!(g.validate().is_ok());
    let converter = GrammarJsConverter::new(g);
    let _ = converter.convert();
}

// ---------------------------------------------------------------------------
// 7. Rule serialization round-trip via serde
// ---------------------------------------------------------------------------

#[test]
fn rule_serde_roundtrip() {
    let rule = Rule::Seq {
        members: vec![
            Rule::String { value: "if".into() },
            Rule::Field {
                name: "condition".into(),
                content: Box::new(Rule::Symbol {
                    name: "expression".into(),
                }),
            },
            Rule::Optional {
                value: Box::new(Rule::Seq {
                    members: vec![
                        Rule::String {
                            value: "else".into(),
                        },
                        Rule::Symbol {
                            name: "expression".into(),
                        },
                    ],
                }),
            },
        ],
    };
    let serialized = serde_json::to_string(&rule).unwrap();
    let deserialized: Rule = serde_json::from_str(&serialized).unwrap();
    // Re-serialize and compare to ensure stable round-trip
    let re_serialized = serde_json::to_string(&deserialized).unwrap();
    assert_eq!(serialized, re_serialized);
}

// ---------------------------------------------------------------------------
// 8. GrammarJs::new defaults
// ---------------------------------------------------------------------------

#[test]
fn grammar_js_new_defaults() {
    let g = GrammarJs::new("test_lang".into());
    assert_eq!(g.name, "test_lang");
    assert!(g.word.is_none());
    assert!(g.inline.is_empty());
    assert!(g.conflicts.is_empty());
    assert!(g.extras.is_empty());
    assert!(g.externals.is_empty());
    assert!(g.precedences.is_empty());
    assert!(g.rules.is_empty());
    assert!(g.supertypes.is_empty());
}
