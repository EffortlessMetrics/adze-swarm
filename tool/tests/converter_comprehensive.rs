#![allow(
    clippy::needless_range_loop,
    reason = "property and comprehensive tests use index-based loops to exercise table positions and boundary cases"
)]

//! Comprehensive tests for grammar conversion functionality.
//!
//! Covers `GrammarConverter` (IR-level), `from_tree_sitter_json` (JSON→GrammarJs),
//! `GrammarJs::validate`, `GrammarJsConverter` (GrammarJs→IR), and `Grammar`
//! normalization / validation round-trips.

use adze_ir::{
    Associativity, FieldId, Grammar, PrecedenceKind, ProductionId, Rule as IrRule, Symbol,
    SymbolId, Token, TokenPattern,
};
use adze_tool::grammar_js::json_converter::from_tree_sitter_json;
use adze_tool::grammar_js::{GrammarJs, Rule};
use adze_tool::{GrammarConverter, GrammarJsConverter};
use serde_json::json;

// ===================================================================
// Helper builders
// ===================================================================

fn minimal_json(name: &str) -> serde_json::Value {
    json!({
        "name": name,
        "rules": {
            "source": { "type": "BLANK" }
        }
    })
}

fn arith_json() -> serde_json::Value {
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
                        "type": "PREC_LEFT",
                        "value": 2,
                        "content": {
                            "type": "SEQ",
                            "members": [
                                { "type": "SYMBOL", "name": "expression" },
                                { "type": "STRING", "value": "*" },
                                { "type": "SYMBOL", "name": "expression" }
                            ]
                        }
                    }
                ]
            },
            "number": {
                "type": "PATTERN",
                "value": "[0-9]+"
            }
        }
    })
}

// ===================================================================
// 1. GrammarConverter – IR-level sample grammar
// ===================================================================

#[test]
fn sample_grammar_name_is_sample() {
    let g = GrammarConverter::create_sample_grammar();
    assert_eq!(g.name, "sample");
}

#[test]
fn sample_grammar_has_three_tokens() {
    let g = GrammarConverter::create_sample_grammar();
    assert_eq!(g.tokens.len(), 3);
}

#[test]
fn sample_grammar_token_patterns_correct() {
    let g = GrammarConverter::create_sample_grammar();
    let id_tok = g.tokens.get(&SymbolId(1)).expect("id token");
    assert!(matches!(&id_tok.pattern, TokenPattern::Regex(r) if r.contains("[a-zA-Z_]")));
    let plus_tok = g.tokens.get(&SymbolId(3)).expect("plus token");
    assert!(matches!(&plus_tok.pattern, TokenPattern::String(s) if s == "+"));
}

#[test]
fn sample_grammar_expr_has_three_alternatives() {
    let g = GrammarConverter::create_sample_grammar();
    let expr_rules = g.get_rules_for_symbol(SymbolId(4)).expect("expr rules");
    assert_eq!(expr_rules.len(), 3);
}

#[test]
fn sample_grammar_addition_rule_has_left_assoc() {
    let g = GrammarConverter::create_sample_grammar();
    let rules = g.get_rules_for_symbol(SymbolId(4)).unwrap();
    let add_rule = rules
        .iter()
        .find(|r| r.rhs.len() == 3)
        .expect("addition rule");
    assert_eq!(add_rule.associativity, Some(Associativity::Left));
    assert_eq!(add_rule.precedence, Some(PrecedenceKind::Static(1)));
}

#[test]
fn sample_grammar_fields_are_left_right() {
    let g = GrammarConverter::create_sample_grammar();
    let mut names: Vec<&str> = g.fields.values().map(|s| s.as_str()).collect();
    names.sort();
    assert_eq!(names, vec!["left", "right"]);
}

#[test]
fn sample_grammar_start_symbol_is_expr() {
    let g = GrammarConverter::create_sample_grammar();
    // The only non-terminal with rules is SymbolId(4)
    assert_eq!(g.start_symbol(), Some(SymbolId(4)));
}

// ===================================================================
// 2. Grammar::normalize – complex symbol expansion
// ===================================================================

#[test]
fn normalize_optional_creates_auxiliary_rules() {
    let mut g = Grammar::new("opt_test".into());
    let s1 = SymbolId(1);
    let s2 = SymbolId(2);
    g.tokens.insert(
        s2,
        Token {
            name: "a".into(),
            pattern: TokenPattern::String("a".into()),
            fragile: false,
        },
    );
    g.add_rule(IrRule {
        lhs: s1,
        rhs: vec![Symbol::Optional(Box::new(Symbol::Terminal(s2)))],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    });
    let all = g.normalize();
    // The auxiliary symbol should produce two rules (inner | ε)
    assert!(
        all.len() >= 3,
        "expected at least 3 rules after normalizing optional, got {}",
        all.len()
    );
}

#[test]
fn normalize_repeat_creates_recursive_and_epsilon() {
    let mut g = Grammar::new("rep_test".into());
    let s1 = SymbolId(1);
    let s2 = SymbolId(2);
    g.tokens.insert(
        s2,
        Token {
            name: "b".into(),
            pattern: TokenPattern::String("b".into()),
            fragile: false,
        },
    );
    g.add_rule(IrRule {
        lhs: s1,
        rhs: vec![Symbol::Repeat(Box::new(Symbol::Terminal(s2)))],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    });
    let all = g.normalize();
    let has_epsilon = all.iter().any(|r| r.rhs.contains(&Symbol::Epsilon));
    assert!(
        has_epsilon,
        "repeat normalization should produce an epsilon rule"
    );
}

#[test]
fn normalize_repeat_one_has_no_epsilon() {
    let mut g = Grammar::new("rep1_test".into());
    let s1 = SymbolId(1);
    let s2 = SymbolId(2);
    g.tokens.insert(
        s2,
        Token {
            name: "c".into(),
            pattern: TokenPattern::String("c".into()),
            fragile: false,
        },
    );
    g.add_rule(IrRule {
        lhs: s1,
        rhs: vec![Symbol::RepeatOne(Box::new(Symbol::Terminal(s2)))],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    });
    let all = g.normalize();
    let has_epsilon = all.iter().any(|r| r.rhs.contains(&Symbol::Epsilon));
    assert!(
        !has_epsilon,
        "repeat-one normalization should NOT produce an epsilon rule"
    );
}

#[test]
fn normalize_choice_creates_per_alternative_rules() {
    let mut g = Grammar::new("choice_test".into());
    let s1 = SymbolId(1);
    let s2 = SymbolId(2);
    let s3 = SymbolId(3);
    g.tokens.insert(
        s2,
        Token {
            name: "x".into(),
            pattern: TokenPattern::String("x".into()),
            fragile: false,
        },
    );
    g.tokens.insert(
        s3,
        Token {
            name: "y".into(),
            pattern: TokenPattern::String("y".into()),
            fragile: false,
        },
    );
    g.add_rule(IrRule {
        lhs: s1,
        rhs: vec![Symbol::Choice(vec![
            Symbol::Terminal(s2),
            Symbol::Terminal(s3),
        ])],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    });
    let all = g.normalize();
    // The aux symbol should have two alternatives
    let aux_rules: Vec<_> = all.iter().filter(|r| r.lhs != s1).collect();
    assert_eq!(
        aux_rules.len(),
        2,
        "choice should expand into two aux rules"
    );
}

#[test]
fn normalize_sequence_is_flattened() {
    let mut g = Grammar::new("seq_test".into());
    let s1 = SymbolId(1);
    let s2 = SymbolId(2);
    let s3 = SymbolId(3);
    g.tokens.insert(
        s2,
        Token {
            name: "a".into(),
            pattern: TokenPattern::String("a".into()),
            fragile: false,
        },
    );
    g.tokens.insert(
        s3,
        Token {
            name: "b".into(),
            pattern: TokenPattern::String("b".into()),
            fragile: false,
        },
    );
    g.add_rule(IrRule {
        lhs: s1,
        rhs: vec![Symbol::Sequence(vec![
            Symbol::Terminal(s2),
            Symbol::Terminal(s3),
        ])],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    });
    let all = g.normalize();
    let main = all.iter().find(|r| r.lhs == s1).unwrap();
    assert_eq!(
        main.rhs.len(),
        2,
        "sequence should be flattened into the parent rule"
    );
}

#[test]
fn normalize_preserves_precedence() {
    let mut g = GrammarConverter::create_sample_grammar();
    g.normalize();
    let rules = g.get_rules_for_symbol(SymbolId(4)).unwrap();
    let add = rules.iter().find(|r| r.rhs.len() == 3).unwrap();
    assert_eq!(add.precedence, Some(PrecedenceKind::Static(1)));
}

// ===================================================================
// 3. Grammar::validate
// ===================================================================

#[test]
fn validate_sample_grammar_succeeds() {
    let g = GrammarConverter::create_sample_grammar();
    assert!(g.validate().is_ok());
}

#[test]
fn validate_empty_grammar_succeeds() {
    let g = Grammar::new("empty".into());
    assert!(g.validate().is_ok());
}

#[test]
fn validate_bad_field_order_fails() {
    let mut g = Grammar::new("bad_order".into());
    g.fields.insert(FieldId(0), "zebra".into());
    g.fields.insert(FieldId(1), "alpha".into());
    assert!(g.validate().is_err());
}

#[test]
fn check_empty_terminals_catches_empty_string() {
    let mut g = Grammar::new("t".into());
    g.tokens.insert(
        SymbolId(1),
        Token {
            name: "blank".into(),
            pattern: TokenPattern::String("".into()),
            fragile: false,
        },
    );
    assert!(g.check_empty_terminals().is_err());
}

#[test]
fn check_empty_terminals_passes_nonempty() {
    let g = GrammarConverter::create_sample_grammar();
    assert!(g.check_empty_terminals().is_ok());
}

// ===================================================================
// 4. from_tree_sitter_json – JSON → GrammarJs
// ===================================================================

#[test]
fn json_blank_rule_parsed() {
    let g = from_tree_sitter_json(&minimal_json("blank")).unwrap();
    assert!(matches!(g.rules["source"], Rule::Blank));
}

#[test]
fn json_optional_parsed() {
    let v = json!({
        "name": "opt",
        "rules": {
            "maybe": {
                "type": "OPTIONAL",
                "value": { "type": "STRING", "value": "x" }
            }
        }
    });
    let g = from_tree_sitter_json(&v).unwrap();
    assert!(matches!(g.rules["maybe"], Rule::Optional { .. }));
}

#[test]
fn json_token_and_immediate_token_parsed() {
    let v = json!({
        "name": "tok",
        "rules": {
            "t": { "type": "TOKEN", "content": { "type": "STRING", "value": "a" } },
            "it": { "type": "IMMEDIATE_TOKEN", "content": { "type": "STRING", "value": "b" } }
        }
    });
    let g = from_tree_sitter_json(&v).unwrap();
    assert!(matches!(g.rules["t"], Rule::Token { .. }));
    assert!(matches!(g.rules["it"], Rule::ImmediateToken { .. }));
}

#[test]
fn json_nested_seq_in_choice_parsed() {
    let v = json!({
        "name": "nested",
        "rules": {
            "r": {
                "type": "CHOICE",
                "members": [
                    {
                        "type": "SEQ",
                        "members": [
                            { "type": "STRING", "value": "(" },
                            { "type": "STRING", "value": ")" }
                        ]
                    },
                    { "type": "BLANK" }
                ]
            }
        }
    });
    let g = from_tree_sitter_json(&v).unwrap();
    if let Rule::Choice { members } = &g.rules["r"] {
        assert_eq!(members.len(), 2);
        assert!(matches!(&members[0], Rule::Seq { .. }));
    } else {
        panic!("expected CHOICE");
    }
}

#[test]
fn json_externals_parsed() {
    let v = json!({
        "name": "ext",
        "externals": [
            { "type": "SYMBOL", "name": "indent" },
            { "type": "SYMBOL", "name": "dedent" }
        ],
        "rules": {
            "source": { "type": "BLANK" }
        }
    });
    let g = from_tree_sitter_json(&v).unwrap();
    assert_eq!(g.externals.len(), 2);
    assert_eq!(g.externals[0].name, "indent");
    assert_eq!(g.externals[1].name, "dedent");
}

#[test]
fn json_unknown_rule_type_is_error() {
    let v = json!({
        "name": "bad",
        "rules": {
            "r": { "type": "FOOBAR", "value": "?" }
        }
    });
    // #1076: unknown rule types are rejected, not silently skipped.
    let err = from_tree_sitter_json(&v).expect_err("unknown rule type must fail");
    assert!(
        format!("{err:#}").contains("FOOBAR"),
        "error should name the unknown type: {err:#}"
    );
}

#[test]
fn json_missing_rules_key_produces_empty_grammar() {
    let v = json!({ "name": "no_rules" });
    let g = from_tree_sitter_json(&v).unwrap();
    assert!(g.rules.is_empty());
}

// ===================================================================
// 5. GrammarJs::validate
// ===================================================================

#[test]
fn validate_grammar_with_valid_symbol_ref_passes() {
    let mut g = GrammarJs::new("ok".into());
    g.rules
        .insert("a".into(), Rule::Pattern { value: "x".into() });
    g.rules
        .insert("b".into(), Rule::Symbol { name: "a".into() });
    assert!(g.validate().is_ok());
}

#[test]
fn validate_grammar_with_external_symbol_passes() {
    let mut g = GrammarJs::new("ext_ok".into());
    g.externals.push(adze_tool::grammar_js::ExternalToken {
        name: "indent".into(),
        symbol: "external_0".into(),
    });
    g.rules.insert(
        "start".into(),
        Rule::Symbol {
            name: "indent".into(),
        },
    );
    assert!(g.validate().is_ok());
}

#[test]
fn validate_conflict_referencing_existing_rules_passes() {
    let mut g = GrammarJs::new("c".into());
    g.rules.insert("a".into(), Rule::Blank);
    g.rules.insert("b".into(), Rule::Blank);
    g.conflicts.push(vec!["a".into(), "b".into()]);
    assert!(g.validate().is_ok());
}

// ===================================================================
// 6. GrammarJsConverter – GrammarJs → IR round-trip
// ===================================================================

#[test]
fn converter_arith_grammar_name() {
    let gjs = from_tree_sitter_json(&arith_json()).unwrap();
    let ir = GrammarJsConverter::new(gjs).convert().unwrap();
    assert_eq!(ir.name, "arith");
}

#[test]
fn converter_arith_has_rules_and_tokens() {
    let gjs = from_tree_sitter_json(&arith_json()).unwrap();
    let ir = GrammarJsConverter::new(gjs).convert().unwrap();
    assert!(!ir.rules.is_empty(), "should have IR rules");
    assert!(!ir.tokens.is_empty(), "should have IR tokens");
}

#[test]
fn converter_arith_token_for_plus() {
    let gjs = from_tree_sitter_json(&arith_json()).unwrap();
    let ir = GrammarJsConverter::new(gjs).convert().unwrap();
    let has_plus = ir
        .tokens
        .values()
        .any(|t| matches!(&t.pattern, TokenPattern::String(s) if s == "+"));
    assert!(has_plus, "should contain a '+' string token");
}

#[test]
fn converter_arith_token_for_star() {
    let gjs = from_tree_sitter_json(&arith_json()).unwrap();
    let ir = GrammarJsConverter::new(gjs).convert().unwrap();
    let has_star = ir
        .tokens
        .values()
        .any(|t| matches!(&t.pattern, TokenPattern::String(s) if s == "*"));
    assert!(has_star, "should contain a '*' string token");
}

#[test]
fn converter_simple_string_rule_creates_token() {
    let v = json!({
        "name": "hello",
        "rules": {
            "greeting": { "type": "STRING", "value": "hi" }
        }
    });
    let gjs = from_tree_sitter_json(&v).unwrap();
    let ir = GrammarJsConverter::new(gjs).convert().unwrap();
    let has_hi = ir
        .tokens
        .values()
        .any(|t| matches!(&t.pattern, TokenPattern::String(s) if s == "hi"));
    assert!(has_hi, "STRING rule should create a literal token");
}

#[test]
fn converter_pattern_rule_creates_regex_token() {
    let v = json!({
        "name": "nums",
        "rules": {
            "number": { "type": "PATTERN", "value": "[0-9]+" }
        }
    });
    let gjs = from_tree_sitter_json(&v).unwrap();
    let ir = GrammarJsConverter::new(gjs).convert().unwrap();
    let has_regex = ir
        .tokens
        .values()
        .any(|t| matches!(&t.pattern, TokenPattern::Regex(r) if r == "[0-9]+"));
    assert!(has_regex, "PATTERN rule should create a regex token");
}

#[test]
fn converter_optional_creates_empty_alternative() {
    let v = json!({
        "name": "opt",
        "rules": {
            "maybe": {
                "type": "OPTIONAL",
                "value": { "type": "STRING", "value": "x" }
            }
        }
    });
    let gjs = from_tree_sitter_json(&v).unwrap();
    let ir = GrammarJsConverter::new(gjs).convert().unwrap();
    // An OPTIONAL should produce at least one empty-rhs alternative
    let has_empty = ir.all_rules().any(|r| r.rhs.is_empty());
    assert!(has_empty, "OPTIONAL should create an empty alternative");
}

#[test]
fn converter_repeat_creates_empty_alternative() {
    let v = json!({
        "name": "rep",
        "rules": {
            "many": {
                "type": "REPEAT",
                "content": { "type": "STRING", "value": "y" }
            }
        }
    });
    let gjs = from_tree_sitter_json(&v).unwrap();
    let ir = GrammarJsConverter::new(gjs).convert().unwrap();
    let has_empty = ir.all_rules().any(|r| r.rhs.is_empty());
    assert!(has_empty, "REPEAT should create an empty alternative");
}

#[test]
fn converter_handles_whitespace_extras() {
    let v = json!({
        "name": "ws",
        "extras": [{ "type": "PATTERN", "value": "\\s" }],
        "rules": {
            "source": { "type": "BLANK" }
        }
    });
    let gjs = from_tree_sitter_json(&v).unwrap();
    let ir = GrammarJsConverter::new(gjs).convert().unwrap();
    // The converter adds a _WHITESPACE token when extras contain \s
    let has_ws = ir.tokens.values().any(|t| t.name.contains("WHITESPACE"));
    assert!(
        has_ws,
        "whitespace extras should generate a whitespace token"
    );
}

#[test]
fn converter_inline_rules_propagated() {
    let v = json!({
        "name": "inl",
        "inline": ["_helper"],
        "rules": {
            "start": { "type": "SYMBOL", "name": "_helper" },
            "_helper": { "type": "STRING", "value": "z" }
        }
    });
    let gjs = from_tree_sitter_json(&v).unwrap();
    let ir = GrammarJsConverter::new(gjs).convert().unwrap();
    assert!(
        !ir.inline_rules.is_empty(),
        "inline rules should be propagated to IR"
    );
}

#[test]
fn converter_conflicts_propagated() {
    let v = json!({
        "name": "conf",
        "conflicts": [["a", "b"]],
        "rules": {
            "a": { "type": "STRING", "value": "a" },
            "b": { "type": "STRING", "value": "b" }
        }
    });
    let gjs = from_tree_sitter_json(&v).unwrap();
    let ir = GrammarJsConverter::new(gjs).convert().unwrap();
    assert!(
        !ir.conflicts.is_empty(),
        "conflicts should be propagated to IR"
    );
}

#[test]
fn converter_supertypes_propagated() {
    let v = json!({
        "name": "sup",
        "supertypes": ["expr"],
        "rules": {
            "expr": { "type": "STRING", "value": "e" }
        }
    });
    let gjs = from_tree_sitter_json(&v).unwrap();
    let ir = GrammarJsConverter::new(gjs).convert().unwrap();
    assert!(
        !ir.supertypes.is_empty(),
        "supertypes should be propagated to IR"
    );
}

// ===================================================================
// 7. GrammarJs serde round-trip
// ===================================================================

#[test]
fn grammar_js_serde_roundtrip() {
    let v = arith_json();
    let gjs = from_tree_sitter_json(&v).unwrap();
    let serialized = serde_json::to_string(&gjs).unwrap();
    let deserialized: GrammarJs = serde_json::from_str(&serialized).unwrap();
    assert_eq!(deserialized.name, gjs.name);
    assert_eq!(deserialized.rules.len(), gjs.rules.len());
}

#[test]
fn rule_serde_roundtrip_string() {
    let rule = Rule::String {
        value: "hello".into(),
    };
    let s = serde_json::to_string(&rule).unwrap();
    let back: Rule = serde_json::from_str(&s).unwrap();
    assert!(matches!(back, Rule::String { value } if value == "hello"));
}

#[test]
fn rule_serde_roundtrip_prec_left() {
    let rule = Rule::PrecLeft {
        value: 5,
        content: Box::new(Rule::Blank),
    };
    let s = serde_json::to_string(&rule).unwrap();
    let back: Rule = serde_json::from_str(&s).unwrap();
    assert!(matches!(back, Rule::PrecLeft { value: 5, .. }));
}
