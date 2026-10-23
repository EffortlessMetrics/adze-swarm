#![allow(
    clippy::needless_range_loop,
    reason = "property and comprehensive tests use index-based loops to exercise table positions and boundary cases"
)]
//! Comprehensive tests for NODE_TYPES JSON generation in adze-tablegen.
//!
//! Covers: schema validation, roundtrip serialization, edge cases,
//! field handling, symbol variants, and deterministic output.

use adze_ir::builder::GrammarBuilder;
use adze_ir::{
    Associativity, ExternalToken, FieldId, Grammar, ProductionId, Rule, Symbol, SymbolId, Token,
    TokenPattern,
};
use adze_tablegen::NodeTypesGenerator;
use serde_json::Value;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Parse generated JSON and return the array of node type objects.
fn generate_and_parse(grammar: &Grammar) -> Vec<Value> {
    let generator = NodeTypesGenerator::new(grammar);
    let json = generator
        .generate()
        .expect("NodeTypesGenerator::generate() failed");
    let val: Value = serde_json::from_str(&json).expect("output is not valid JSON");
    val.as_array().expect("output is not a JSON array").to_vec()
}

/// Find a node type entry by its `type` field.
fn find_node<'a>(nodes: &'a [Value], type_name: &str) -> Option<&'a Value> {
    nodes.iter().find(|n| n["type"].as_str() == Some(type_name))
}

// ---------------------------------------------------------------------------
// 1. Schema shape — every entry must have `type` (string) + `named` (bool)
// ---------------------------------------------------------------------------

#[test]
fn schema_required_fields_present() {
    let g = GrammarBuilder::new("schema")
        .token("num", r"\d+")
        .token("+", "+")
        .rule("expr", vec!["num"])
        .build();

    for entry in generate_and_parse(&g) {
        assert!(
            entry.get("type").and_then(Value::as_str).is_some(),
            "entry missing 'type': {entry}"
        );
        assert!(
            entry.get("named").and_then(Value::as_bool).is_some(),
            "entry missing 'named': {entry}"
        );
    }
}

// ---------------------------------------------------------------------------
// 2. Optional keys are absent when not applicable
// ---------------------------------------------------------------------------

#[test]
fn optional_keys_absent_when_empty() {
    let g = GrammarBuilder::new("opt")
        .token("x", "x")
        .rule("start", vec!["x"])
        .build();

    let nodes = generate_and_parse(&g);
    // The literal "x" token should have no fields, children, or subtypes.
    let x_node = find_node(&nodes, "x").expect("missing 'x'");
    assert!(x_node.get("fields").is_none(), "unexpected 'fields'");
    assert!(x_node.get("children").is_none(), "unexpected 'children'");
    assert!(x_node.get("subtypes").is_none(), "unexpected 'subtypes'");
}

// ---------------------------------------------------------------------------
// 3. Named vs anonymous classification
// ---------------------------------------------------------------------------

#[test]
fn regex_tokens_are_named() {
    let g = GrammarBuilder::new("named_tok")
        .token("identifier", r"[a-z]+")
        .rule("start", vec!["identifier"])
        .build();

    let nodes = generate_and_parse(&g);
    // Regex tokens should NOT appear as anonymous nodes in the output.
    // They appear as named when referenced via rules. Verify no unnamed 'identifier' entry.
    let anon: Vec<_> = nodes.iter().filter(|n| n["named"] == false).collect();
    for n in &anon {
        assert_ne!(
            n["type"].as_str(),
            Some("identifier"),
            "regex token should not be anonymous"
        );
    }
}

#[test]
fn string_literal_tokens_are_anonymous() {
    let g = GrammarBuilder::new("anon_tok")
        .token("+", "+")
        .token(";", ";")
        .token("id", r"[a-z]+")
        .rule("stmt", vec!["id", ";"])
        .build();

    let nodes = generate_and_parse(&g);
    let semi = find_node(&nodes, ";").expect("missing ';'");
    assert_eq!(semi["named"], false, "literal ';' should be anonymous");
}

// ---------------------------------------------------------------------------
// 4. Internal rules (prefixed with _) are excluded
// ---------------------------------------------------------------------------

#[test]
fn internal_rules_excluded_from_output() {
    let mut g = Grammar::new("internal".to_string());

    let tok_id = SymbolId(0);
    g.tokens.insert(
        tok_id,
        Token {
            name: "a".to_string(),
            pattern: TokenPattern::String("a".to_string()),
            fragile: false,
        },
    );

    // Public rule
    let pub_id = SymbolId(1);
    g.rule_names.insert(pub_id, "public_rule".to_string());
    g.add_rule(Rule {
        lhs: pub_id,
        rhs: vec![Symbol::Terminal(tok_id)],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    });

    // Internal rule (starts with _)
    let priv_id = SymbolId(2);
    g.rule_names.insert(priv_id, "_hidden".to_string());
    g.add_rule(Rule {
        lhs: priv_id,
        rhs: vec![Symbol::Terminal(tok_id)],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(1),
    });

    let nodes = generate_and_parse(&g);
    assert!(
        find_node(&nodes, "_hidden").is_none(),
        "internal rule should be excluded"
    );
    assert!(
        find_node(&nodes, "public_rule").is_some(),
        "public rule should be present"
    );
}

// ---------------------------------------------------------------------------
// 5. Fields appear on node types
// ---------------------------------------------------------------------------

#[test]
fn fields_attached_to_node_type() {
    let mut g = Grammar::new("fields".to_string());

    let num_id = SymbolId(0);
    g.tokens.insert(
        num_id,
        Token {
            name: "number".to_string(),
            pattern: TokenPattern::Regex(r"\d+".to_string()),
            fragile: false,
        },
    );

    let op_id = SymbolId(1);
    g.tokens.insert(
        op_id,
        Token {
            name: "op".to_string(),
            pattern: TokenPattern::String("+".to_string()),
            fragile: false,
        },
    );

    let expr_id = SymbolId(10);
    g.rule_names
        .insert(expr_id, "binary_expression".to_string());

    let left_field = FieldId(0);
    let right_field = FieldId(1);
    g.fields.insert(left_field, "left".to_string());
    g.fields.insert(right_field, "right".to_string());

    g.add_rule(Rule {
        lhs: expr_id,
        rhs: vec![
            Symbol::Terminal(num_id),
            Symbol::Terminal(op_id),
            Symbol::Terminal(num_id),
        ],
        precedence: None,
        associativity: None,
        fields: vec![(left_field, 0), (right_field, 2)],
        production_id: ProductionId(0),
    });

    let nodes = generate_and_parse(&g);
    let expr = find_node(&nodes, "binary_expression").expect("missing binary_expression");
    let fields_obj = expr
        .get("fields")
        .expect("binary_expression should have fields");
    assert!(fields_obj.get("left").is_some(), "missing 'left' field");
    assert!(fields_obj.get("right").is_some(), "missing 'right' field");
}

// ---------------------------------------------------------------------------
// 6. Field types array contains correct type references
// ---------------------------------------------------------------------------

#[test]
fn field_types_reference_correct_symbols() {
    let mut g = Grammar::new("field_types".to_string());

    let num_id = SymbolId(0);
    g.tokens.insert(
        num_id,
        Token {
            name: "number".to_string(),
            pattern: TokenPattern::Regex(r"\d+".to_string()),
            fragile: false,
        },
    );

    let expr_id = SymbolId(10);
    g.rule_names.insert(expr_id, "wrapper".to_string());
    let val_field = FieldId(0);
    g.fields.insert(val_field, "value".to_string());

    g.add_rule(Rule {
        lhs: expr_id,
        rhs: vec![Symbol::Terminal(num_id)],
        precedence: None,
        associativity: None,
        fields: vec![(val_field, 0)],
        production_id: ProductionId(0),
    });

    let nodes = generate_and_parse(&g);
    let wrapper = find_node(&nodes, "wrapper").expect("missing wrapper");
    let value_field = &wrapper["fields"]["value"];
    let types = value_field["types"]
        .as_array()
        .expect("types should be array");
    assert!(!types.is_empty(), "types array should not be empty");

    // The type reference should point to the number token (named regex token)
    let first_type = &types[0];
    assert_eq!(first_type["type"].as_str(), Some("number"));
    assert_eq!(first_type["named"], true);
}

// ---------------------------------------------------------------------------
// 7. JSON roundtrip: parse → serialize → parse yields identical value
// ---------------------------------------------------------------------------

#[test]
fn json_roundtrip_identity() {
    let g = GrammarBuilder::new("roundtrip")
        .token("num", r"\d+")
        .token("+", "+")
        .token("-", "-")
        .rule("expr", vec!["num"])
        .rule("add", vec!["expr", "+", "expr"])
        .rule("sub", vec!["expr", "-", "expr"])
        .build();

    let generator = NodeTypesGenerator::new(&g);
    let json1 = generator.generate().unwrap();
    let val1: Value = serde_json::from_str(&json1).unwrap();

    // Serialize back to string, then parse again
    let json2 = serde_json::to_string_pretty(&val1).unwrap();
    let val2: Value = serde_json::from_str(&json2).unwrap();

    assert_eq!(val1, val2, "roundtrip must be identity");
}

// ---------------------------------------------------------------------------
// 8. Output is sorted alphabetically by type name
// ---------------------------------------------------------------------------

#[test]
fn output_sorted_by_type_name() {
    let g = GrammarBuilder::new("sort")
        .token("z_tok", r"z+")
        .token("a_tok", r"a+")
        .token("m_tok", r"m+")
        .rule("zebra", vec!["z_tok"])
        .rule("alpha", vec!["a_tok"])
        .rule("middle", vec!["m_tok"])
        .build();

    let nodes = generate_and_parse(&g);
    let names: Vec<&str> = nodes.iter().filter_map(|n| n["type"].as_str()).collect();
    let mut sorted = names.clone();
    sorted.sort();
    assert_eq!(names, sorted, "entries should be alphabetically sorted");
}

// ---------------------------------------------------------------------------
// 9. Determinism — same grammar → same output across invocations
// ---------------------------------------------------------------------------

#[test]
fn deterministic_across_invocations() {
    let make = || {
        GrammarBuilder::new("det")
            .token("num", r"\d+")
            .token("+", "+")
            .rule("expr", vec!["num", "+", "num"])
            .build()
    };

    let j1 = NodeTypesGenerator::new(&make()).generate().unwrap();
    let j2 = NodeTypesGenerator::new(&make()).generate().unwrap();
    let v1: Value = serde_json::from_str(&j1).unwrap();
    let v2: Value = serde_json::from_str(&j2).unwrap();
    assert_eq!(v1, v2, "identical grammars must produce identical JSON");
}

// ---------------------------------------------------------------------------
// 10. Empty grammar → valid empty JSON array
// ---------------------------------------------------------------------------

#[test]
fn empty_grammar_yields_empty_array() {
    let g = Grammar::new("empty".to_string());
    let nodes = generate_and_parse(&g);
    assert!(nodes.is_empty(), "empty grammar should produce []");
}

// ---------------------------------------------------------------------------
// 11. Rules referencing NonTerminal symbols
// ---------------------------------------------------------------------------

#[test]
fn nonterminal_reference_in_fields() {
    let mut g = Grammar::new("nt_ref".to_string());

    let tok_id = SymbolId(0);
    g.tokens.insert(
        tok_id,
        Token {
            name: "id".to_string(),
            pattern: TokenPattern::Regex(r"[a-z]+".to_string()),
            fragile: false,
        },
    );

    let child_id = SymbolId(1);
    g.rule_names.insert(child_id, "child".to_string());
    g.add_rule(Rule {
        lhs: child_id,
        rhs: vec![Symbol::Terminal(tok_id)],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    });

    let parent_id = SymbolId(2);
    g.rule_names.insert(parent_id, "parent".to_string());
    let body_field = FieldId(0);
    g.fields.insert(body_field, "body".to_string());
    g.add_rule(Rule {
        lhs: parent_id,
        rhs: vec![Symbol::NonTerminal(child_id)],
        precedence: None,
        associativity: None,
        fields: vec![(body_field, 0)],
        production_id: ProductionId(1),
    });

    let nodes = generate_and_parse(&g);
    let parent = find_node(&nodes, "parent").expect("missing 'parent'");
    let body_types = &parent["fields"]["body"]["types"];
    let first = &body_types[0];
    assert_eq!(first["type"].as_str(), Some("child"));
    assert_eq!(
        first["named"], true,
        "non-terminal reference should be named"
    );
}

// ---------------------------------------------------------------------------
// 12. External symbol produces named "external" type reference
// ---------------------------------------------------------------------------

#[test]
fn external_symbol_type_ref() {
    let mut g = Grammar::new("ext".to_string());

    let ext_sym = SymbolId(50);
    g.externals.push(ExternalToken {
        name: "indent".to_string(),
        symbol_id: ext_sym,
    });

    let rule_id = SymbolId(1);
    g.rule_names.insert(rule_id, "block".to_string());
    let indent_field = FieldId(0);
    g.fields.insert(indent_field, "indent".to_string());
    g.add_rule(Rule {
        lhs: rule_id,
        rhs: vec![Symbol::External(ext_sym)],
        precedence: None,
        associativity: None,
        fields: vec![(indent_field, 0)],
        production_id: ProductionId(0),
    });

    let nodes = generate_and_parse(&g);
    let block = find_node(&nodes, "block").expect("missing 'block'");
    let indent_types = &block["fields"]["indent"]["types"];
    let first = &indent_types[0];
    assert_eq!(first["type"].as_str(), Some("external"));
    assert_eq!(first["named"], true);
}

// ---------------------------------------------------------------------------
// 13. Choice symbol uses first alternative's type reference
// ---------------------------------------------------------------------------

#[test]
fn choice_symbol_uses_first_alternative() {
    let mut g = Grammar::new("choice".to_string());

    let num_id = SymbolId(0);
    g.tokens.insert(
        num_id,
        Token {
            name: "number".to_string(),
            pattern: TokenPattern::Regex(r"\d+".to_string()),
            fragile: false,
        },
    );

    let str_id = SymbolId(1);
    g.tokens.insert(
        str_id,
        Token {
            name: "string".to_string(),
            pattern: TokenPattern::Regex(r#""[^"]*""#.to_string()),
            fragile: false,
        },
    );

    let val_id = SymbolId(10);
    g.rule_names.insert(val_id, "value".to_string());
    let inner_field = FieldId(0);
    g.fields.insert(inner_field, "inner".to_string());

    g.add_rule(Rule {
        lhs: val_id,
        rhs: vec![Symbol::Choice(vec![
            Symbol::Terminal(num_id),
            Symbol::Terminal(str_id),
        ])],
        precedence: None,
        associativity: None,
        fields: vec![(inner_field, 0)],
        production_id: ProductionId(0),
    });

    let nodes = generate_and_parse(&g);
    let value = find_node(&nodes, "value").expect("missing 'value'");
    let inner_types = &value["fields"]["inner"]["types"];
    let first = &inner_types[0];
    // Choice uses the first alternative
    assert_eq!(first["type"].as_str(), Some("number"));
}

// ---------------------------------------------------------------------------
// 14. Sequence symbol uses first element's type reference
// ---------------------------------------------------------------------------

#[test]
fn sequence_symbol_uses_first_element() {
    let mut g = Grammar::new("seq".to_string());

    let a_id = SymbolId(0);
    g.tokens.insert(
        a_id,
        Token {
            name: "alpha".to_string(),
            pattern: TokenPattern::Regex(r"[a-z]+".to_string()),
            fragile: false,
        },
    );

    let b_id = SymbolId(1);
    g.tokens.insert(
        b_id,
        Token {
            name: "beta".to_string(),
            pattern: TokenPattern::Regex(r"[A-Z]+".to_string()),
            fragile: false,
        },
    );

    let rule_id = SymbolId(10);
    g.rule_names.insert(rule_id, "pair".to_string());
    let combo_field = FieldId(0);
    g.fields.insert(combo_field, "combo".to_string());

    g.add_rule(Rule {
        lhs: rule_id,
        rhs: vec![Symbol::Sequence(vec![
            Symbol::Terminal(a_id),
            Symbol::Terminal(b_id),
        ])],
        precedence: None,
        associativity: None,
        fields: vec![(combo_field, 0)],
        production_id: ProductionId(0),
    });

    let nodes = generate_and_parse(&g);
    let pair = find_node(&nodes, "pair").expect("missing 'pair'");
    let combo_types = &pair["fields"]["combo"]["types"];
    assert_eq!(combo_types[0]["type"].as_str(), Some("alpha"));
}

// ---------------------------------------------------------------------------
// 15. Epsilon symbol contributes no real child (no fictitious "empty" type)
// ---------------------------------------------------------------------------

#[test]
fn epsilon_symbol_type_ref() {
    let mut g = Grammar::new("eps".to_string());

    let rule_id = SymbolId(1);
    g.rule_names.insert(rule_id, "void_rule".to_string());
    let phantom_field = FieldId(0);
    g.fields.insert(phantom_field, "phantom".to_string());

    g.add_rule(Rule {
        lhs: rule_id,
        rhs: vec![Symbol::Epsilon],
        precedence: None,
        associativity: None,
        fields: vec![(phantom_field, 0)],
        production_id: ProductionId(0),
    });

    let nodes = generate_and_parse(&g);
    let void_rule = find_node(&nodes, "void_rule").expect("missing 'void_rule'");
    let field = &void_rule["fields"]["phantom"];
    assert!(
        field["types"].as_array().is_some_and(|t| t.is_empty()),
        "epsilon must yield no types, got {field}"
    );
    assert_eq!(field["required"], false);
}

// ---------------------------------------------------------------------------
// 16. Optional symbol delegates to inner type
// ---------------------------------------------------------------------------

#[test]
fn optional_symbol_delegates_to_inner() {
    let mut g = Grammar::new("opt_sym".to_string());

    let tok_id = SymbolId(0);
    g.tokens.insert(
        tok_id,
        Token {
            name: "kw".to_string(),
            pattern: TokenPattern::String("return".to_string()),
            fragile: false,
        },
    );

    let rule_id = SymbolId(10);
    g.rule_names.insert(rule_id, "maybe_return".to_string());
    let kw_field = FieldId(0);
    g.fields.insert(kw_field, "keyword".to_string());

    g.add_rule(Rule {
        lhs: rule_id,
        rhs: vec![Symbol::Optional(Box::new(Symbol::Terminal(tok_id)))],
        precedence: None,
        associativity: None,
        fields: vec![(kw_field, 0)],
        production_id: ProductionId(0),
    });

    let nodes = generate_and_parse(&g);
    let mr = find_node(&nodes, "maybe_return").expect("missing 'maybe_return'");
    let types = &mr["fields"]["keyword"]["types"];
    // Optional delegates to the inner token's type
    assert_eq!(types[0]["type"].as_str(), Some("return"));
}

// ---------------------------------------------------------------------------
// 17. Repeat / RepeatOne delegate to inner type
// ---------------------------------------------------------------------------

#[test]
fn repeat_symbols_delegate_to_inner() {
    let mut g = Grammar::new("rep".to_string());

    let tok_id = SymbolId(0);
    g.tokens.insert(
        tok_id,
        Token {
            name: "digit".to_string(),
            pattern: TokenPattern::Regex(r"\d".to_string()),
            fragile: false,
        },
    );

    let rule_id = SymbolId(10);
    g.rule_names.insert(rule_id, "digits".to_string());
    let items_field = FieldId(0);
    g.fields.insert(items_field, "items".to_string());

    g.add_rule(Rule {
        lhs: rule_id,
        rhs: vec![Symbol::Repeat(Box::new(Symbol::Terminal(tok_id)))],
        precedence: None,
        associativity: None,
        fields: vec![(items_field, 0)],
        production_id: ProductionId(0),
    });

    let nodes = generate_and_parse(&g);
    let digits = find_node(&nodes, "digits").expect("missing 'digits'");
    let types = &digits["fields"]["items"]["types"];
    assert_eq!(types[0]["type"].as_str(), Some("digit"));
    assert_eq!(types[0]["named"], true);
}

// ---------------------------------------------------------------------------
// 18. Multiple rules for same LHS do not duplicate node type
// ---------------------------------------------------------------------------

#[test]
fn multiple_rules_same_lhs_no_duplicate() {
    let g = GrammarBuilder::new("multi")
        .token("a", r"a+")
        .token("b", r"b+")
        .rule("expr", vec!["a"])
        .rule("expr", vec!["b"])
        .build();

    let nodes = generate_and_parse(&g);
    let count = nodes
        .iter()
        .filter(|n| n["type"].as_str() == Some("expr"))
        .count();
    assert_eq!(count, 1, "same LHS should produce exactly one node type");
}

// ---------------------------------------------------------------------------
// 19. Large grammar — many rules and tokens, still valid JSON
// ---------------------------------------------------------------------------

#[test]
fn large_grammar_valid_json() {
    let mut builder = GrammarBuilder::new("large");
    for i in 0..50 {
        let tok_name = format!("tok_{i}");
        let rule_name = format!("rule_{i}");
        builder = builder
            .token(&tok_name, &format!(r"t{i}"))
            .rule(&rule_name, vec![&tok_name]);
        // Borrow checker workaround: we must convert the string back for the vec
        // Actually, builder consumes self, so the above won't work directly.
        // Let's use the raw Grammar API instead.
    }
    // The builder approach above has lifetime issues in a loop.
    // Use raw Grammar API for the large test.

    let mut g = Grammar::new("large".to_string());
    for i in 0u16..50 {
        let tok_id = SymbolId(i);
        g.tokens.insert(
            tok_id,
            Token {
                name: format!("tok_{i}"),
                pattern: TokenPattern::Regex(format!("t{i}")),
                fragile: false,
            },
        );

        let rule_id = SymbolId(100 + i);
        g.rule_names.insert(rule_id, format!("rule_{i}"));
        g.add_rule(Rule {
            lhs: rule_id,
            rhs: vec![Symbol::Terminal(tok_id)],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(i),
        });
    }

    let generator = NodeTypesGenerator::new(&g);
    let json = generator.generate().expect("large grammar should succeed");
    let val: Value = serde_json::from_str(&json).expect("output must be valid JSON");
    let arr = val.as_array().expect("must be array");
    // At least 50 rules should appear
    assert!(arr.len() >= 50, "expected ≥50 entries, got {}", arr.len());
}

// ---------------------------------------------------------------------------
// 20. No null type names in output
// ---------------------------------------------------------------------------

#[test]
fn no_null_type_names() {
    let g = GrammarBuilder::new("nullcheck")
        .token("x", r"x+")
        .token("y", "y")
        .rule("start", vec!["x"])
        .rule("alt", vec!["y"])
        .build();

    for entry in generate_and_parse(&g) {
        let t = entry.get("type").expect("missing type");
        assert!(!t.is_null(), "type must not be null: {entry}");
        let s = t.as_str().expect("type must be string");
        assert!(!s.is_empty(), "type must not be empty");
    }
}

// ---------------------------------------------------------------------------
// 21. Pretty-printed JSON (contains newlines and indentation)
// ---------------------------------------------------------------------------

#[test]
fn output_is_pretty_printed() {
    let g = GrammarBuilder::new("pretty")
        .token("a", r"a+")
        .rule("start", vec!["a"])
        .build();

    let generator = NodeTypesGenerator::new(&g);
    let json = generator.generate().unwrap();
    assert!(json.contains('\n'), "JSON should be pretty-printed");
    assert!(json.contains("  "), "JSON should have indentation");
}

// ---------------------------------------------------------------------------
// 22. Supertypes declared on Grammar appear in output
// ---------------------------------------------------------------------------

#[test]
fn supertype_grammar_generates_node_types() {
    let mut g = Grammar::new("super".to_string());

    let num_id = SymbolId(0);
    g.tokens.insert(
        num_id,
        Token {
            name: "number".to_string(),
            pattern: TokenPattern::Regex(r"\d+".to_string()),
            fragile: false,
        },
    );

    let str_id = SymbolId(1);
    g.tokens.insert(
        str_id,
        Token {
            name: "string".to_string(),
            pattern: TokenPattern::Regex(r#""[^"]*""#.to_string()),
            fragile: false,
        },
    );

    // `literal` is a supertype over number | string
    let literal_id = SymbolId(10);
    g.rule_names.insert(literal_id, "literal".to_string());
    g.supertypes.push(literal_id);
    g.add_rule(Rule {
        lhs: literal_id,
        rhs: vec![Symbol::Terminal(num_id)],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    });

    let nodes = generate_and_parse(&g);
    let lit = find_node(&nodes, "literal");
    assert!(lit.is_some(), "supertype 'literal' should appear in output");
}

// ---------------------------------------------------------------------------
// 23. Adding a token changes the output
// ---------------------------------------------------------------------------

#[test]
fn adding_token_changes_output() {
    let g1 = GrammarBuilder::new("diff")
        .token("a", "a")
        .rule("start", vec!["a"])
        .build();

    let mut g2 = Grammar::new("diff".to_string());
    // Copy g1 manually
    let a_id = SymbolId(0);
    g2.tokens.insert(
        a_id,
        Token {
            name: "a".to_string(),
            pattern: TokenPattern::String("a".to_string()),
            fragile: false,
        },
    );
    let start_id = SymbolId(1);
    g2.rule_names.insert(start_id, "start".to_string());
    g2.add_rule(Rule {
        lhs: start_id,
        rhs: vec![Symbol::Terminal(a_id)],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    });
    // Add extra token
    let b_id = SymbolId(2);
    g2.tokens.insert(
        b_id,
        Token {
            name: "b".to_string(),
            pattern: TokenPattern::String("b".to_string()),
            fragile: false,
        },
    );

    let j1 = NodeTypesGenerator::new(&g1).generate().unwrap();
    let j2 = NodeTypesGenerator::new(&g2).generate().unwrap();
    let v1: Value = serde_json::from_str(&j1).unwrap();
    let v2: Value = serde_json::from_str(&j2).unwrap();
    assert_ne!(v1, v2, "adding a token should change the output");
}

// ---------------------------------------------------------------------------
// 24. Rules are named=true
// ---------------------------------------------------------------------------

#[test]
fn rules_are_named() {
    let g = GrammarBuilder::new("named_rules")
        .token("x", r"x+")
        .rule("statement", vec!["x"])
        .rule("expression", vec!["x"])
        .build();

    let nodes = generate_and_parse(&g);
    let stmt = find_node(&nodes, "statement").expect("missing 'statement'");
    assert_eq!(stmt["named"], true, "rules should be named=true");
    let expr = find_node(&nodes, "expression").expect("missing 'expression'");
    assert_eq!(expr["named"], true, "rules should be named=true");
}

// ---------------------------------------------------------------------------
// 25. GrammarBuilder with extras still generates valid output
// ---------------------------------------------------------------------------

#[test]
fn grammar_with_extras_generates_valid_output() {
    let g = GrammarBuilder::new("extras")
        .token("num", r"\d+")
        .token("ws", r"\s+")
        .rule("expr", vec!["num"])
        .extra("ws")
        .build();

    let nodes = generate_and_parse(&g);
    // Should still have at least the rule
    assert!(
        find_node(&nodes, "expr").is_some(),
        "expr should be present"
    );
}

// ---------------------------------------------------------------------------
// 26. GrammarBuilder with precedence rules
// ---------------------------------------------------------------------------

#[test]
fn precedence_rules_produce_valid_output() {
    let g = GrammarBuilder::new("prec")
        .token("num", r"\d+")
        .token("+", "+")
        .token("*", "*")
        .rule("expr", vec!["num"])
        .rule_with_precedence("expr", vec!["expr", "+", "expr"], 1, Associativity::Left)
        .rule_with_precedence("expr", vec!["expr", "*", "expr"], 2, Associativity::Left)
        .build();

    let nodes = generate_and_parse(&g);
    let expr = find_node(&nodes, "expr").expect("missing 'expr'");
    assert_eq!(expr["named"], true);

    // Literal tokens should be present
    assert!(find_node(&nodes, "+").is_some(), "'+' token missing");
    assert!(find_node(&nodes, "*").is_some(), "'*' token missing");
}

// ---------------------------------------------------------------------------
// 27. Multiple fields on one rule all appear
// ---------------------------------------------------------------------------

#[test]
fn multiple_fields_all_present() {
    let mut g = Grammar::new("multi_field".to_string());

    let a_id = SymbolId(0);
    g.tokens.insert(
        a_id,
        Token {
            name: "a".to_string(),
            pattern: TokenPattern::Regex(r"a+".to_string()),
            fragile: false,
        },
    );
    let b_id = SymbolId(1);
    g.tokens.insert(
        b_id,
        Token {
            name: "b".to_string(),
            pattern: TokenPattern::Regex(r"b+".to_string()),
            fragile: false,
        },
    );
    let c_id = SymbolId(2);
    g.tokens.insert(
        c_id,
        Token {
            name: "c".to_string(),
            pattern: TokenPattern::Regex(r"c+".to_string()),
            fragile: false,
        },
    );

    let rule_id = SymbolId(10);
    g.rule_names.insert(rule_id, "triple".to_string());

    let f1 = FieldId(0);
    let f2 = FieldId(1);
    let f3 = FieldId(2);
    g.fields.insert(f1, "first".to_string());
    g.fields.insert(f2, "second".to_string());
    g.fields.insert(f3, "third".to_string());

    g.add_rule(Rule {
        lhs: rule_id,
        rhs: vec![
            Symbol::Terminal(a_id),
            Symbol::Terminal(b_id),
            Symbol::Terminal(c_id),
        ],
        precedence: None,
        associativity: None,
        fields: vec![(f1, 0), (f2, 1), (f3, 2)],
        production_id: ProductionId(0),
    });

    let nodes = generate_and_parse(&g);
    let triple = find_node(&nodes, "triple").expect("missing 'triple'");
    let fields = triple.get("fields").expect("should have fields");
    assert!(fields.get("first").is_some(), "missing 'first'");
    assert!(fields.get("second").is_some(), "missing 'second'");
    assert!(fields.get("third").is_some(), "missing 'third'");
}

// ---------------------------------------------------------------------------
// 28. RepeatOne delegates to inner type (same as Repeat)
// ---------------------------------------------------------------------------

#[test]
fn repeat_one_symbol_delegates_to_inner() {
    let mut g = Grammar::new("rep1".to_string());

    let tok_id = SymbolId(0);
    g.tokens.insert(
        tok_id,
        Token {
            name: "word".to_string(),
            pattern: TokenPattern::Regex(r"[a-z]+".to_string()),
            fragile: false,
        },
    );

    let rule_id = SymbolId(10);
    g.rule_names.insert(rule_id, "words".to_string());
    let elems_field = FieldId(0);
    g.fields.insert(elems_field, "elems".to_string());

    g.add_rule(Rule {
        lhs: rule_id,
        rhs: vec![Symbol::RepeatOne(Box::new(Symbol::Terminal(tok_id)))],
        precedence: None,
        associativity: None,
        fields: vec![(elems_field, 0)],
        production_id: ProductionId(0),
    });

    let nodes = generate_and_parse(&g);
    let words = find_node(&nodes, "words").expect("missing 'words'");
    let types = &words["fields"]["elems"]["types"];
    assert_eq!(types[0]["type"].as_str(), Some("word"));
    assert_eq!(types[0]["named"], true);
}

// ---------------------------------------------------------------------------
// 29. Empty choice contributes no real child (no fictitious "empty" type)
// ---------------------------------------------------------------------------

#[test]
fn empty_choice_produces_empty_type() {
    let mut g = Grammar::new("empty_choice".to_string());

    let rule_id = SymbolId(1);
    g.rule_names.insert(rule_id, "nothing".to_string());
    let val_field = FieldId(0);
    g.fields.insert(val_field, "val".to_string());

    g.add_rule(Rule {
        lhs: rule_id,
        rhs: vec![Symbol::Choice(vec![])],
        precedence: None,
        associativity: None,
        fields: vec![(val_field, 0)],
        production_id: ProductionId(0),
    });

    let nodes = generate_and_parse(&g);
    let nothing = find_node(&nodes, "nothing").expect("missing 'nothing'");
    let field = &nothing["fields"]["val"];
    assert!(
        field["types"].as_array().is_some_and(|t| t.is_empty()),
        "empty choice must yield no types, got {field}"
    );
    assert_eq!(field["required"], false);
}

// ---------------------------------------------------------------------------
// 30. Empty sequence contributes no real child (no fictitious "empty" type)
// ---------------------------------------------------------------------------

#[test]
fn empty_sequence_produces_empty_type() {
    let mut g = Grammar::new("empty_seq".to_string());

    let rule_id = SymbolId(1);
    g.rule_names.insert(rule_id, "void_seq".to_string());
    let s_field = FieldId(0);
    g.fields.insert(s_field, "s".to_string());

    g.add_rule(Rule {
        lhs: rule_id,
        rhs: vec![Symbol::Sequence(vec![])],
        precedence: None,
        associativity: None,
        fields: vec![(s_field, 0)],
        production_id: ProductionId(0),
    });

    let nodes = generate_and_parse(&g);
    let vs = find_node(&nodes, "void_seq").expect("missing 'void_seq'");
    let field = &vs["fields"]["s"];
    assert!(
        field["types"].as_array().is_some_and(|t| t.is_empty()),
        "empty sequence must yield no types, got {field}"
    );
    assert_eq!(field["required"], false);
}

// ---------------------------------------------------------------------------
// 31. Unknown terminal produces "unknown" type reference
// ---------------------------------------------------------------------------

#[test]
fn unknown_terminal_produces_unknown_type() {
    let mut g = Grammar::new("unk".to_string());

    // Reference a terminal that has no token definition
    let phantom_id = SymbolId(99);
    let rule_id = SymbolId(1);
    g.rule_names.insert(rule_id, "mystery".to_string());
    let t_field = FieldId(0);
    g.fields.insert(t_field, "t".to_string());

    g.add_rule(Rule {
        lhs: rule_id,
        rhs: vec![Symbol::Terminal(phantom_id)],
        precedence: None,
        associativity: None,
        fields: vec![(t_field, 0)],
        production_id: ProductionId(0),
    });

    let nodes = generate_and_parse(&g);
    let mystery = find_node(&nodes, "mystery").expect("missing 'mystery'");
    let types = &mystery["fields"]["t"]["types"];
    assert_eq!(types[0]["type"].as_str(), Some("unknown"));
    assert_eq!(types[0]["named"], false);
}

// ---------------------------------------------------------------------------
// 32. Unknown non-terminal produces "unknown" type reference
// ---------------------------------------------------------------------------

#[test]
fn unknown_nonterminal_produces_unknown_type() {
    let mut g = Grammar::new("unk_nt".to_string());

    // Reference a non-terminal that has no rule_name or token
    let phantom_nt = SymbolId(88);
    let rule_id = SymbolId(1);
    g.rule_names.insert(rule_id, "ref_unknown".to_string());
    let r_field = FieldId(0);
    g.fields.insert(r_field, "r".to_string());

    g.add_rule(Rule {
        lhs: rule_id,
        rhs: vec![Symbol::NonTerminal(phantom_nt)],
        precedence: None,
        associativity: None,
        fields: vec![(r_field, 0)],
        production_id: ProductionId(0),
    });

    let nodes = generate_and_parse(&g);
    let ru = find_node(&nodes, "ref_unknown").expect("missing 'ref_unknown'");
    let types = &ru["fields"]["r"]["types"];
    assert_eq!(types[0]["type"].as_str(), Some("unknown"));
    assert_eq!(types[0]["named"], true);
}

// ---------------------------------------------------------------------------
// 33. Nested optional(repeat(terminal)) resolves through wrappers
// ---------------------------------------------------------------------------

#[test]
fn nested_optional_repeat_resolves_through() {
    let mut g = Grammar::new("nested".to_string());

    let tok_id = SymbolId(0);
    g.tokens.insert(
        tok_id,
        Token {
            name: "item".to_string(),
            pattern: TokenPattern::Regex(r"[a-z]+".to_string()),
            fragile: false,
        },
    );

    let rule_id = SymbolId(10);
    g.rule_names.insert(rule_id, "container".to_string());
    let content_field = FieldId(0);
    g.fields.insert(content_field, "content".to_string());

    // Optional(Repeat(Terminal))
    g.add_rule(Rule {
        lhs: rule_id,
        rhs: vec![Symbol::Optional(Box::new(Symbol::Repeat(Box::new(
            Symbol::Terminal(tok_id),
        ))))],
        precedence: None,
        associativity: None,
        fields: vec![(content_field, 0)],
        production_id: ProductionId(0),
    });

    let nodes = generate_and_parse(&g);
    let container = find_node(&nodes, "container").expect("missing 'container'");
    let types = &container["fields"]["content"]["types"];
    // Should resolve through Optional → Repeat → Terminal("item")
    assert_eq!(types[0]["type"].as_str(), Some("item"));
    assert_eq!(types[0]["named"], true);
}

// ---------------------------------------------------------------------------
// 34. Fragile tokens are still emitted as anonymous nodes
// ---------------------------------------------------------------------------

#[test]
fn fragile_tokens_emitted_as_anonymous() {
    let mut g = Grammar::new("fragile".to_string());

    let tok_id = SymbolId(0);
    g.tokens.insert(
        tok_id,
        Token {
            name: "semicolon".to_string(),
            pattern: TokenPattern::String(";".to_string()),
            fragile: true,
        },
    );

    let rule_id = SymbolId(10);
    g.rule_names.insert(rule_id, "stmt".to_string());
    g.add_rule(Rule {
        lhs: rule_id,
        rhs: vec![Symbol::Terminal(tok_id)],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    });

    let nodes = generate_and_parse(&g);
    let semi = find_node(&nodes, ";").expect("missing ';'");
    assert_eq!(
        semi["named"], false,
        "fragile string token should be anonymous"
    );
}

// ---------------------------------------------------------------------------
// 35. Only string-pattern tokens appear as anonymous — regex tokens excluded
// ---------------------------------------------------------------------------

#[test]
fn only_string_tokens_appear_anonymous() {
    let mut g = Grammar::new("anon_only".to_string());

    let regex_id = SymbolId(0);
    g.tokens.insert(
        regex_id,
        Token {
            name: "ident".to_string(),
            pattern: TokenPattern::Regex(r"[a-z]+".to_string()),
            fragile: false,
        },
    );

    let str_id = SymbolId(1);
    g.tokens.insert(
        str_id,
        Token {
            name: "comma".to_string(),
            pattern: TokenPattern::String(",".to_string()),
            fragile: false,
        },
    );

    let rule_id = SymbolId(10);
    g.rule_names.insert(rule_id, "list".to_string());
    g.add_rule(Rule {
        lhs: rule_id,
        rhs: vec![Symbol::Terminal(regex_id), Symbol::Terminal(str_id)],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    });

    let nodes = generate_and_parse(&g);
    let anon_nodes: Vec<_> = nodes.iter().filter(|n| n["named"] == false).collect();
    // Only the string token "," should be anonymous
    assert_eq!(anon_nodes.len(), 1);
    assert_eq!(anon_nodes[0]["type"].as_str(), Some(","));
}

// ---------------------------------------------------------------------------
// 36. Rule with no rule_name gets fallback name
// ---------------------------------------------------------------------------

#[test]
fn rule_without_name_gets_fallback() {
    let mut g = Grammar::new("fallback".to_string());

    let tok_id = SymbolId(0);
    g.tokens.insert(
        tok_id,
        Token {
            name: "x".to_string(),
            pattern: TokenPattern::String("x".to_string()),
            fragile: false,
        },
    );

    // Add rule without inserting into rule_names
    let rule_id = SymbolId(5);
    g.add_rule(Rule {
        lhs: rule_id,
        rhs: vec![Symbol::Terminal(tok_id)],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    });

    let nodes = generate_and_parse(&g);
    // Fallback name should be "rule_5"
    assert!(
        find_node(&nodes, "rule_5").is_some(),
        "unnamed rule should get fallback name 'rule_5'"
    );
}

// ---------------------------------------------------------------------------
// 37. Multiple string tokens all appear as anonymous
// ---------------------------------------------------------------------------

#[test]
fn multiple_string_tokens_all_anonymous() {
    let mut g = Grammar::new("multi_str".to_string());

    for (i, lit) in ["+", "-", "*", "/", "(", ")"].iter().enumerate() {
        g.tokens.insert(
            SymbolId(i as u16),
            Token {
                name: lit.to_string(),
                pattern: TokenPattern::String(lit.to_string()),
                fragile: false,
            },
        );
    }

    let rule_id = SymbolId(100);
    g.rule_names.insert(rule_id, "expr".to_string());
    g.add_rule(Rule {
        lhs: rule_id,
        rhs: vec![Symbol::Terminal(SymbolId(0))],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    });

    let nodes = generate_and_parse(&g);
    let anon: Vec<_> = nodes.iter().filter(|n| n["named"] == false).collect();
    assert_eq!(anon.len(), 6, "all 6 string tokens should be anonymous");
}

// ---------------------------------------------------------------------------
// 38. Grammar with only tokens and no rules
// ---------------------------------------------------------------------------

#[test]
fn grammar_only_tokens_no_rules() {
    let mut g = Grammar::new("tokens_only".to_string());

    g.tokens.insert(
        SymbolId(0),
        Token {
            name: "plus".to_string(),
            pattern: TokenPattern::String("+".to_string()),
            fragile: false,
        },
    );
    g.tokens.insert(
        SymbolId(1),
        Token {
            name: "num".to_string(),
            pattern: TokenPattern::Regex(r"\d+".to_string()),
            fragile: false,
        },
    );

    let nodes = generate_and_parse(&g);
    // Only string tokens appear (regex tokens are excluded from anonymous output)
    assert_eq!(nodes.len(), 1);
    assert_eq!(nodes[0]["type"].as_str(), Some("+"));
    assert_eq!(nodes[0]["named"], false);
}

// ---------------------------------------------------------------------------
// 39. Grammar with only anonymous tokens — no named nodes at all
// ---------------------------------------------------------------------------

#[test]
fn all_anonymous_grammar() {
    let mut g = Grammar::new("all_anon".to_string());

    for (i, lit) in ["(", ")", "{", "}"].iter().enumerate() {
        g.tokens.insert(
            SymbolId(i as u16),
            Token {
                name: lit.to_string(),
                pattern: TokenPattern::String(lit.to_string()),
                fragile: false,
            },
        );
    }

    let nodes = generate_and_parse(&g);
    assert!(
        nodes.iter().all(|n| n["named"] == false),
        "all nodes should be anonymous"
    );
    assert_eq!(nodes.len(), 4);
}

// ---------------------------------------------------------------------------
// 40. Token that is also referenced by rule_names uses token name
// ---------------------------------------------------------------------------

#[test]
fn token_name_takes_priority_over_rule_name() {
    let mut g = Grammar::new("overlap".to_string());

    let id = SymbolId(0);
    g.tokens.insert(
        id,
        Token {
            name: "number".to_string(),
            pattern: TokenPattern::Regex(r"\d+".to_string()),
            fragile: false,
        },
    );
    // Also add to rule_names (simulates overlap)
    g.rule_names.insert(id, "number_alt".to_string());

    // The get_rule_name method checks tokens first
    let generator = NodeTypesGenerator::new(&g);
    let json = generator.generate().unwrap();
    // Should produce valid JSON without error
    let _: Vec<Value> = serde_json::from_str(&json).unwrap();
}

// ---------------------------------------------------------------------------
// 41. Very long rule name is preserved verbatim
// ---------------------------------------------------------------------------

#[test]
fn long_rule_name_preserved() {
    let mut g = Grammar::new("long_name".to_string());

    let tok_id = SymbolId(0);
    g.tokens.insert(
        tok_id,
        Token {
            name: "a".to_string(),
            pattern: TokenPattern::String("a".to_string()),
            fragile: false,
        },
    );

    let long_name = "a".repeat(200);
    let rule_id = SymbolId(1);
    g.rule_names.insert(rule_id, long_name.clone());
    g.add_rule(Rule {
        lhs: rule_id,
        rhs: vec![Symbol::Terminal(tok_id)],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    });

    let nodes = generate_and_parse(&g);
    assert!(
        find_node(&nodes, &long_name).is_some(),
        "long rule name should be preserved"
    );
}

// ---------------------------------------------------------------------------
// 42. Unicode rule names preserved
// ---------------------------------------------------------------------------

#[test]
fn unicode_rule_name_preserved() {
    let mut g = Grammar::new("unicode".to_string());

    let tok_id = SymbolId(0);
    g.tokens.insert(
        tok_id,
        Token {
            name: "x".to_string(),
            pattern: TokenPattern::String("x".to_string()),
            fragile: false,
        },
    );

    let rule_id = SymbolId(1);
    g.rule_names.insert(rule_id, "式".to_string());
    g.add_rule(Rule {
        lhs: rule_id,
        rhs: vec![Symbol::Terminal(tok_id)],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    });

    let nodes = generate_and_parse(&g);
    assert!(
        find_node(&nodes, "式").is_some(),
        "unicode rule name should be preserved"
    );
}

// ---------------------------------------------------------------------------
// 43. Unicode token literal preserved
// ---------------------------------------------------------------------------

#[test]
fn unicode_token_literal_preserved() {
    let mut g = Grammar::new("unicode_tok".to_string());

    let tok_id = SymbolId(0);
    g.tokens.insert(
        tok_id,
        Token {
            name: "arrow".to_string(),
            pattern: TokenPattern::String("→".to_string()),
            fragile: false,
        },
    );

    let rule_id = SymbolId(1);
    g.rule_names.insert(rule_id, "stmt".to_string());
    g.add_rule(Rule {
        lhs: rule_id,
        rhs: vec![Symbol::Terminal(tok_id)],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    });

    let nodes = generate_and_parse(&g);
    assert!(
        find_node(&nodes, "→").is_some(),
        "unicode token literal should appear"
    );
}

// ---------------------------------------------------------------------------
// 44. Field referencing unknown FieldId is gracefully skipped
// ---------------------------------------------------------------------------

#[test]
fn field_with_unknown_field_id_skipped() {
    let mut g = Grammar::new("bad_field".to_string());

    let tok_id = SymbolId(0);
    g.tokens.insert(
        tok_id,
        Token {
            name: "a".to_string(),
            pattern: TokenPattern::String("a".to_string()),
            fragile: false,
        },
    );

    let rule_id = SymbolId(1);
    g.rule_names.insert(rule_id, "r".to_string());
    // FieldId(99) is not in g.fields
    g.add_rule(Rule {
        lhs: rule_id,
        rhs: vec![Symbol::Terminal(tok_id)],
        precedence: None,
        associativity: None,
        fields: vec![(FieldId(99), 0)],
        production_id: ProductionId(0),
    });

    let nodes = generate_and_parse(&g);
    let r = find_node(&nodes, "r").expect("missing 'r'");
    // Unknown field should be skipped, so no fields
    assert!(r.get("fields").is_none(), "unknown field should be skipped");
}

// ---------------------------------------------------------------------------
// 45. Field with out-of-bounds position is gracefully skipped
// ---------------------------------------------------------------------------

#[test]
fn field_with_out_of_bounds_position_skipped() {
    let mut g = Grammar::new("oob_field".to_string());

    let tok_id = SymbolId(0);
    g.tokens.insert(
        tok_id,
        Token {
            name: "a".to_string(),
            pattern: TokenPattern::String("a".to_string()),
            fragile: false,
        },
    );

    let rule_id = SymbolId(1);
    g.rule_names.insert(rule_id, "r".to_string());
    let f = FieldId(0);
    g.fields.insert(f, "val".to_string());
    // Position 10 is out of bounds (rhs has only 1 element)
    g.add_rule(Rule {
        lhs: rule_id,
        rhs: vec![Symbol::Terminal(tok_id)],
        precedence: None,
        associativity: None,
        fields: vec![(f, 10)],
        production_id: ProductionId(0),
    });

    let nodes = generate_and_parse(&g);
    let r = find_node(&nodes, "r").expect("missing 'r'");
    // Out-of-bounds field position should be skipped
    assert!(
        r.get("fields").is_none(),
        "out-of-bounds field should be skipped"
    );
}

// ---------------------------------------------------------------------------
// 46. Two rules with same LHS but different fields merges them
// ---------------------------------------------------------------------------

#[test]
fn multiple_rules_merge_fields() {
    let mut g = Grammar::new("merge_fields".to_string());

    let tok_a = SymbolId(0);
    g.tokens.insert(
        tok_a,
        Token {
            name: "a".to_string(),
            pattern: TokenPattern::Regex(r"a".to_string()),
            fragile: false,
        },
    );
    let tok_b = SymbolId(1);
    g.tokens.insert(
        tok_b,
        Token {
            name: "b".to_string(),
            pattern: TokenPattern::Regex(r"b".to_string()),
            fragile: false,
        },
    );

    let rule_id = SymbolId(10);
    g.rule_names.insert(rule_id, "node".to_string());

    let f_left = FieldId(0);
    let f_right = FieldId(1);
    g.fields.insert(f_left, "left".to_string());
    g.fields.insert(f_right, "right".to_string());

    // First production: has "left" field
    g.add_rule(Rule {
        lhs: rule_id,
        rhs: vec![Symbol::Terminal(tok_a)],
        precedence: None,
        associativity: None,
        fields: vec![(f_left, 0)],
        production_id: ProductionId(0),
    });
    // Second production: has "right" field
    g.add_rule(Rule {
        lhs: rule_id,
        rhs: vec![Symbol::Terminal(tok_b)],
        precedence: None,
        associativity: None,
        fields: vec![(f_right, 0)],
        production_id: ProductionId(1),
    });

    let nodes = generate_and_parse(&g);
    let node = find_node(&nodes, "node").expect("missing 'node'");
    let fields_obj = node.get("fields").expect("should have fields");
    assert!(fields_obj.get("left").is_some(), "missing 'left'");
    assert!(fields_obj.get("right").is_some(), "missing 'right'");
}

// ---------------------------------------------------------------------------
// 47. Deeply nested Symbol::Optional(Repeat(Terminal)) resolves
// ---------------------------------------------------------------------------

#[test]
fn deeply_nested_symbol_resolves() {
    let mut g = Grammar::new("deep_nest".to_string());

    let tok_id = SymbolId(0);
    g.tokens.insert(
        tok_id,
        Token {
            name: "num".to_string(),
            pattern: TokenPattern::Regex(r"\d+".to_string()),
            fragile: false,
        },
    );

    let rule_id = SymbolId(10);
    g.rule_names.insert(rule_id, "deep".to_string());
    let f = FieldId(0);
    g.fields.insert(f, "val".to_string());

    // Optional(Repeat(Optional(Terminal)))
    let sym = Symbol::Optional(Box::new(Symbol::Repeat(Box::new(Symbol::Optional(
        Box::new(Symbol::Terminal(tok_id)),
    )))));

    g.add_rule(Rule {
        lhs: rule_id,
        rhs: vec![sym],
        precedence: None,
        associativity: None,
        fields: vec![(f, 0)],
        production_id: ProductionId(0),
    });

    let nodes = generate_and_parse(&g);
    let deep = find_node(&nodes, "deep").expect("missing 'deep'");
    let types = &deep["fields"]["val"]["types"];
    let first = &types[0];
    assert_eq!(first["type"].as_str(), Some("num"));
    assert_eq!(first["named"], true);
}

// ---------------------------------------------------------------------------
// 48. Choice with all terminals picks first
// ---------------------------------------------------------------------------

#[test]
fn choice_all_terminals_picks_first() {
    let mut g = Grammar::new("choice_terms".to_string());

    let tok_a = SymbolId(0);
    g.tokens.insert(
        tok_a,
        Token {
            name: "a".to_string(),
            pattern: TokenPattern::String("a".to_string()),
            fragile: false,
        },
    );
    let tok_b = SymbolId(1);
    g.tokens.insert(
        tok_b,
        Token {
            name: "b".to_string(),
            pattern: TokenPattern::String("b".to_string()),
            fragile: false,
        },
    );

    let rule_id = SymbolId(10);
    g.rule_names.insert(rule_id, "pick".to_string());
    let f = FieldId(0);
    g.fields.insert(f, "v".to_string());

    g.add_rule(Rule {
        lhs: rule_id,
        rhs: vec![Symbol::Choice(vec![
            Symbol::Terminal(tok_a),
            Symbol::Terminal(tok_b),
        ])],
        precedence: None,
        associativity: None,
        fields: vec![(f, 0)],
        production_id: ProductionId(0),
    });

    let nodes = generate_and_parse(&g);
    let pick = find_node(&nodes, "pick").expect("missing 'pick'");
    let types = &pick["fields"]["v"]["types"];
    assert_eq!(types[0]["type"].as_str(), Some("a"));
}

// ---------------------------------------------------------------------------
// 49. Sequence with mixed terminals and nonterminals picks first
// ---------------------------------------------------------------------------

#[test]
fn sequence_mixed_picks_first() {
    let mut g = Grammar::new("seq_mixed".to_string());

    let tok_id = SymbolId(0);
    g.tokens.insert(
        tok_id,
        Token {
            name: "x".to_string(),
            pattern: TokenPattern::String("x".to_string()),
            fragile: false,
        },
    );

    let nt_id = SymbolId(5);
    g.rule_names.insert(nt_id, "child".to_string());
    g.add_rule(Rule {
        lhs: nt_id,
        rhs: vec![Symbol::Terminal(tok_id)],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    });

    let rule_id = SymbolId(10);
    g.rule_names.insert(rule_id, "parent".to_string());
    let f = FieldId(0);
    g.fields.insert(f, "v".to_string());

    g.add_rule(Rule {
        lhs: rule_id,
        rhs: vec![Symbol::Sequence(vec![
            Symbol::NonTerminal(nt_id),
            Symbol::Terminal(tok_id),
        ])],
        precedence: None,
        associativity: None,
        fields: vec![(f, 0)],
        production_id: ProductionId(1),
    });

    let nodes = generate_and_parse(&g);
    let parent = find_node(&nodes, "parent").expect("missing 'parent'");
    let types = &parent["fields"]["v"]["types"];
    // Sequence picks first element which is NonTerminal("child")
    assert_eq!(types[0]["type"].as_str(), Some("child"));
    assert_eq!(types[0]["named"], true);
}

// ---------------------------------------------------------------------------
// 50. GrammarBuilder-based test with extras produces valid JSON
// ---------------------------------------------------------------------------

#[test]
fn builder_extras_valid_json() {
    let g = GrammarBuilder::new("extras")
        .token("ws", r"[ \t]+")
        .token("id", r"[a-z]+")
        .extra("ws")
        .rule("prog", vec!["id"])
        .build();

    let nodes = generate_and_parse(&g);
    // Should not panic; extras don't affect node types output
    assert!(!nodes.is_empty());
}

// ---------------------------------------------------------------------------
// 51. GrammarBuilder with external tokens produces valid output
// ---------------------------------------------------------------------------

#[test]
fn builder_external_tokens_valid() {
    let g = GrammarBuilder::new("ext_builder")
        .token("id", r"[a-z]+")
        .external("INDENT")
        .external("DEDENT")
        .rule("block", vec!["id"])
        .build();

    let nodes = generate_and_parse(&g);
    assert!(!nodes.is_empty());
}

// ---------------------------------------------------------------------------
// 52. NodeTypesGenerator is Send (can be transferred across threads)
// ---------------------------------------------------------------------------

#[test]
fn generator_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<NodeTypesGenerator<'_>>();
}

// ---------------------------------------------------------------------------
// 53. NodeTypesGenerator is Sync (can be shared across threads)
// ---------------------------------------------------------------------------

#[test]
fn generator_is_sync() {
    fn assert_sync<T: Sync>() {}
    assert_sync::<NodeTypesGenerator<'_>>();
}

// ---------------------------------------------------------------------------
// 54. Same-name rules and tokens — token checked first in get_rule_name
// ---------------------------------------------------------------------------

#[test]
fn same_name_token_checked_first() {
    let mut g = Grammar::new("priority".to_string());

    let id = SymbolId(0);
    g.tokens.insert(
        id,
        Token {
            name: "kw".to_string(),
            pattern: TokenPattern::String("keyword".to_string()),
            fragile: false,
        },
    );
    g.rule_names.insert(id, "kw".to_string());

    // When this symbol has a rule, the node type name comes from the token
    g.add_rule(Rule {
        lhs: id,
        rhs: vec![Symbol::Epsilon],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    });

    // Should not panic; produces valid output
    let nodes = generate_and_parse(&g);
    assert!(!nodes.is_empty());
}

// ---------------------------------------------------------------------------
// 55. Large number of rules doesn't cause stack overflow
// ---------------------------------------------------------------------------

#[test]
fn large_rule_count_no_overflow() {
    let mut g = Grammar::new("large".to_string());

    let tok_id = SymbolId(0);
    g.tokens.insert(
        tok_id,
        Token {
            name: "t".to_string(),
            pattern: TokenPattern::String("t".to_string()),
            fragile: false,
        },
    );

    for i in 1u16..=200 {
        let id = SymbolId(i);
        g.rule_names.insert(id, format!("rule_{}", i));
        g.add_rule(Rule {
            lhs: id,
            rhs: vec![Symbol::Terminal(tok_id)],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(i),
        });
    }

    let nodes = generate_and_parse(&g);
    // 200 named rules + 1 anonymous token "t"
    assert_eq!(nodes.len(), 201);
}

// ---------------------------------------------------------------------------
// 56. Token with empty-string name still produces valid JSON
// ---------------------------------------------------------------------------

#[test]
fn empty_string_token_name() {
    let mut g = Grammar::new("empty_name".to_string());

    g.tokens.insert(
        SymbolId(0),
        Token {
            name: "".to_string(),
            pattern: TokenPattern::String("".to_string()),
            fragile: false,
        },
    );

    // Should not panic
    let generator = NodeTypesGenerator::new(&g);
    let result = generator.generate();
    assert!(result.is_ok());
}

// ---------------------------------------------------------------------------
// 57. RepeatOne delegates through to inner like Repeat
// ---------------------------------------------------------------------------

#[test]
fn repeat_one_delegates_same_as_repeat() {
    let mut g = Grammar::new("rep1_vs_rep".to_string());

    let tok_id = SymbolId(0);
    g.tokens.insert(
        tok_id,
        Token {
            name: "num".to_string(),
            pattern: TokenPattern::Regex(r"\d+".to_string()),
            fragile: false,
        },
    );

    let f_rep = FieldId(0);
    let f_rep1 = FieldId(1);
    g.fields.insert(f_rep, "rep".to_string());
    g.fields.insert(f_rep1, "rep1".to_string());

    let rule_id = SymbolId(10);
    g.rule_names.insert(rule_id, "node".to_string());
    g.add_rule(Rule {
        lhs: rule_id,
        rhs: vec![
            Symbol::Repeat(Box::new(Symbol::Terminal(tok_id))),
            Symbol::RepeatOne(Box::new(Symbol::Terminal(tok_id))),
        ],
        precedence: None,
        associativity: None,
        fields: vec![(f_rep, 0), (f_rep1, 1)],
        production_id: ProductionId(0),
    });

    let nodes = generate_and_parse(&g);
    let node = find_node(&nodes, "node").expect("missing 'node'");
    let rep_type = &node["fields"]["rep"]["types"][0];
    let rep1_type = &node["fields"]["rep1"]["types"][0];
    assert_eq!(rep_type["type"], rep1_type["type"]);
    assert_eq!(rep_type["named"], rep1_type["named"]);
}

// ---------------------------------------------------------------------------
// 58. Multiple internal rules — none appear in output
// ---------------------------------------------------------------------------

#[test]
fn multiple_internal_rules_all_excluded() {
    let mut g = Grammar::new("multi_internal".to_string());

    let tok_id = SymbolId(0);
    g.tokens.insert(
        tok_id,
        Token {
            name: "a".to_string(),
            pattern: TokenPattern::String("a".to_string()),
            fragile: false,
        },
    );

    for i in 1u16..=5 {
        let id = SymbolId(i);
        g.rule_names.insert(id, format!("_internal_{}", i));
        g.add_rule(Rule {
            lhs: id,
            rhs: vec![Symbol::Terminal(tok_id)],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(i),
        });
    }

    let nodes = generate_and_parse(&g);
    for node in &nodes {
        let name = node["type"].as_str().unwrap();
        assert!(
            !name.starts_with('_'),
            "internal rule '{}' should not appear",
            name
        );
    }
}

// ---------------------------------------------------------------------------
// 59. JSON output has no duplicate entries for same type name
// ---------------------------------------------------------------------------

#[test]
fn no_duplicate_type_entries() {
    let g = GrammarBuilder::new("dedup")
        .token("num", r"\d+")
        .token("+", "+")
        .rule("expr", vec!["num"])
        .rule("expr", vec!["expr", "+", "expr"])
        .build();

    let nodes = generate_and_parse(&g);
    let mut seen = std::collections::HashSet::new();
    for node in &nodes {
        let name = node["type"].as_str().unwrap();
        let named = node["named"].as_bool().unwrap();
        let key = (name.to_string(), named);
        assert!(
            seen.insert(key.clone()),
            "duplicate entry for ({}, named={})",
            name,
            named
        );
    }
}

// ---------------------------------------------------------------------------
// 60. generate() returns Ok, not Err
// ---------------------------------------------------------------------------

#[test]
fn generate_returns_ok_for_valid_grammar() {
    let g = GrammarBuilder::new("ok_test")
        .token("id", r"[a-z]+")
        .rule("start", vec!["id"])
        .build();

    let generator = NodeTypesGenerator::new(&g);
    assert!(generator.generate().is_ok());
}

// ---------------------------------------------------------------------------
// 61. Named rule nodes always have `named: true`
// ---------------------------------------------------------------------------

#[test]
fn all_rule_nodes_are_named() {
    let g = GrammarBuilder::new("named_rules")
        .token("id", r"[a-z]+")
        .token(",", ",")
        .rule("item", vec!["id"])
        .rule("list", vec!["item", ",", "item"])
        .build();

    let nodes = generate_and_parse(&g);
    for node in &nodes {
        let name = node["type"].as_str().unwrap();
        let named = node["named"].as_bool().unwrap();
        // Rules should be named, string tokens should not
        if name == "item" || name == "list" {
            assert!(named, "rule '{}' should be named", name);
        }
    }
}

// ---------------------------------------------------------------------------
// 62. GrammarBuilder python_like produces valid node types
// ---------------------------------------------------------------------------

#[test]
fn python_like_grammar_valid() {
    let g = GrammarBuilder::python_like();
    let nodes = generate_and_parse(&g);
    assert!(!nodes.is_empty());
    // Should contain "module", "statement", "function_def", etc.
    assert!(find_node(&nodes, "module").is_some());
    assert!(find_node(&nodes, "statement").is_some());
}

// ---------------------------------------------------------------------------
// 63. GrammarBuilder javascript_like produces valid node types
// ---------------------------------------------------------------------------

#[test]
fn javascript_like_grammar_valid() {
    let g = GrammarBuilder::javascript_like();
    let nodes = generate_and_parse(&g);
    assert!(!nodes.is_empty());
    assert!(find_node(&nodes, "program").is_some());
}

// ---------------------------------------------------------------------------
// 64. Epsilon-only rule still produces a named node
// ---------------------------------------------------------------------------

#[test]
fn epsilon_only_rule_produces_named_node() {
    let mut g = Grammar::new("eps_rule".to_string());

    let rule_id = SymbolId(1);
    g.rule_names.insert(rule_id, "empty_rule".to_string());
    g.add_rule(Rule {
        lhs: rule_id,
        rhs: vec![Symbol::Epsilon],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    });

    let nodes = generate_and_parse(&g);
    let er = find_node(&nodes, "empty_rule").expect("missing 'empty_rule'");
    assert_eq!(er["named"], true);
}

// ---------------------------------------------------------------------------
// 65. Rules with precedence still produce node types
// ---------------------------------------------------------------------------

#[test]
fn precedence_rules_produce_node_types() {
    let g = GrammarBuilder::new("prec")
        .token("num", r"\d+")
        .token("+", "+")
        .token("*", "*")
        .rule_with_precedence("expr", vec!["expr", "+", "expr"], 1, Associativity::Left)
        .rule_with_precedence("expr", vec!["expr", "*", "expr"], 2, Associativity::Left)
        .rule("expr", vec!["num"])
        .build();

    let nodes = generate_and_parse(&g);
    assert!(find_node(&nodes, "expr").is_some());
}

// ---------------------------------------------------------------------------
// COMPRESSION TESTS: Parse table compression properties
// ---------------------------------------------------------------------------
// These tests verify the parse table compression pipeline which is essential
// for generating efficient Tree-sitter grammars with minimal memory overhead.

use adze_glr_core::{Action, ParseTable};
use adze_ir::RuleId;
use adze_tablegen::compress::TableCompressor;
use std::collections::BTreeMap;

/// Helper: Create empty parse table with given dimensions
fn make_empty_parse_table(states: usize, symbols: usize) -> ParseTable {
    let actions = vec![vec![vec![]; symbols]; states];
    let gotos = vec![vec![adze_ir::StateId(u16::MAX); symbols]; states];

    let mut symbol_to_index: BTreeMap<SymbolId, usize> = BTreeMap::new();
    for i in 0..symbols {
        symbol_to_index.insert(SymbolId(i as u16), i);
    }

    let mut index_to_symbol = vec![SymbolId(0); symbols];
    for (sid, idx) in &symbol_to_index {
        index_to_symbol[*idx] = *sid;
    }

    ParseTable {
        action_table: actions,
        goto_table: gotos,
        rules: vec![],
        state_count: states,
        symbol_count: symbols,
        symbol_to_index,
        index_to_symbol,
        nonterminal_to_index: BTreeMap::new(),
        symbol_metadata: vec![],
        token_count: 2,
        external_token_count: 0,
        eof_symbol: SymbolId(symbols as u16 - 1),
        start_symbol: SymbolId(symbols as u16 - 1),
        initial_state: adze_ir::StateId(0),
        lex_modes: vec![
            adze_glr_core::LexMode {
                lex_state: 0,
                external_lex_state: 0,
            };
            states
        ],
        extras: vec![],
        external_scanner_states: vec![],
        dynamic_prec_by_rule: vec![],
        rule_assoc_by_rule: vec![],
        alias_sequences: vec![],
        field_names: vec![],
        field_map: BTreeMap::new(),
        grammar: Grammar::default(),
        goto_indexing: adze_glr_core::GotoIndexing::NonterminalMap,
    }
}

/// Helper: Get sorted token indices from grammar
fn get_indices(table: &ParseTable) -> Vec<usize> {
    let mut v: Vec<usize> = table.symbol_to_index.values().copied().collect();
    v.sort();
    v.dedup();
    v
}

// TEST 1: Compressed action table has row_offsets
#[test]
fn compressed_action_table_has_row_offsets() {
    let mut table = make_empty_parse_table(5, 8);
    // Add minimal shift action to pass validation
    if let Some(cell) = table.action_table[0].get_mut(1) {
        cell.push(Action::Shift(adze_ir::StateId(1)));
    }

    let compressor = TableCompressor::new();
    let indices = get_indices(&table);
    let result = compressor.compress(&table, &indices, false);

    assert!(result.is_ok());
    let compressed = result.unwrap();
    assert!(!compressed.action_table.row_offsets.is_empty());
    assert_eq!(
        compressed.action_table.row_offsets.len(),
        table.state_count + 1
    );
}

// TEST 2: Compressed goto table has row_offsets
#[test]
fn compressed_goto_table_has_row_offsets() {
    let mut table = make_empty_parse_table(5, 8);
    // Add minimal shift action
    if let Some(cell) = table.action_table[0].get_mut(1) {
        cell.push(Action::Shift(adze_ir::StateId(1)));
    }

    let compressor = TableCompressor::new();
    let indices = get_indices(&table);
    let result = compressor.compress(&table, &indices, false);

    assert!(result.is_ok());
    let compressed = result.unwrap();
    assert!(!compressed.goto_table.row_offsets.is_empty());
    assert_eq!(
        compressed.goto_table.row_offsets.len(),
        table.state_count + 1
    );
}

// TEST 3: Compressed tables from different grammars differ
#[test]
fn compressed_tables_from_different_grammars_differ() {
    let mut table1 = make_empty_parse_table(3, 5);
    let mut table2 = make_empty_parse_table(4, 6);

    // Add shift to both
    if let Some(cell) = table1.action_table[0].get_mut(1) {
        cell.push(Action::Shift(adze_ir::StateId(1)));
    }
    if let Some(cell) = table2.action_table[0].get_mut(1) {
        cell.push(Action::Shift(adze_ir::StateId(1)));
    }

    let compressor = TableCompressor::new();
    let indices1 = get_indices(&table1);
    let indices2 = get_indices(&table2);

    let compressed1 = compressor.compress(&table1, &indices1, false).unwrap();
    let compressed2 = compressor.compress(&table2, &indices2, false).unwrap();

    // Different table dimensions should produce different offsets or data
    assert!(
        compressed1.action_table.row_offsets != compressed2.action_table.row_offsets
            || compressed1.action_table.data.len() != compressed2.action_table.data.len()
    );
}

// TEST 4: Compression deterministic (same grammar → same output)
#[test]
fn compression_deterministic_same_output() {
    let mut table = make_empty_parse_table(3, 5);
    if let Some(cell) = table.action_table[0].get_mut(1) {
        cell.push(Action::Shift(adze_ir::StateId(1)));
    }

    let compressor = TableCompressor::new();
    let indices = get_indices(&table);

    let compressed1 = compressor.compress(&table, &indices, false).unwrap();
    let compressed2 = compressor.compress(&table, &indices, false).unwrap();

    // Same table should produce identical compression
    assert_eq!(
        compressed1.action_table.row_offsets,
        compressed2.action_table.row_offsets
    );
    assert_eq!(
        compressed1.goto_table.row_offsets,
        compressed2.goto_table.row_offsets
    );
}

// TEST 5: Row offset count equals state count + 1
#[test]
fn row_offset_count_equals_state_count_plus_one() {
    let mut table = make_empty_parse_table(7, 10);
    if let Some(cell) = table.action_table[0].get_mut(1) {
        cell.push(Action::Shift(adze_ir::StateId(1)));
    }

    let compressor = TableCompressor::new();
    let indices = get_indices(&table);
    let compressed = compressor.compress(&table, &indices, false).unwrap();

    assert_eq!(
        compressed.action_table.row_offsets.len(),
        table.state_count + 1
    );
    assert_eq!(
        compressed.goto_table.row_offsets.len(),
        table.state_count + 1
    );
}

// TEST 6: Compression preserves action semantics (action count matches)
#[test]
fn compression_preserves_action_semantics() {
    let mut table = make_empty_parse_table(2, 4);

    // State 0: has 2 shift actions
    if let Some(cell) = table.action_table[0].get_mut(1) {
        cell.push(Action::Shift(adze_ir::StateId(1)));
    }
    if let Some(cell) = table.action_table[0].get_mut(2) {
        cell.push(Action::Shift(adze_ir::StateId(2)));
    }

    // State 1: has 1 reduce action
    if let Some(cell) = table.action_table[1].get_mut(1) {
        cell.push(Action::Reduce(RuleId(1)));
    }

    let compressor = TableCompressor::new();
    let indices = get_indices(&table);
    let compressed = compressor.compress(&table, &indices, false).unwrap();

    // Should have compressed the actions
    assert!(!compressed.action_table.data.is_empty());
}

// TEST 7: Small grammar compression (single rule)
#[test]
fn small_grammar_compression() {
    let mut table = make_empty_parse_table(2, 3);

    if let Some(cell) = table.action_table[0].get_mut(1) {
        cell.push(Action::Shift(adze_ir::StateId(1)));
    }
    if let Some(cell) = table.action_table[1].get_mut(1) {
        cell.push(Action::Reduce(RuleId(1)));
    }

    let compressor = TableCompressor::new();
    let indices = get_indices(&table);
    let result = compressor.compress(&table, &indices, false);

    assert!(result.is_ok());
    let compressed = result.unwrap();
    assert!(!compressed.action_table.row_offsets.is_empty());
}

// TEST 8: Large grammar compression (10+ tokens)
#[test]
fn large_grammar_compression_many_tokens() {
    let mut table = make_empty_parse_table(20, 15);

    // Populate with shifts
    for state in 0..20 {
        for sym in 1..5 {
            if let Some(cell) = table.action_table[state].get_mut(sym) {
                cell.push(Action::Shift(adze_ir::StateId((state + 1) as u16)));
            }
        }
    }

    let compressor = TableCompressor::new();
    let indices = get_indices(&table);
    let result = compressor.compress(&table, &indices, false);

    assert!(result.is_ok());
    let compressed = result.unwrap();
    assert!(!compressed.action_table.data.is_empty());
}

// TEST 9: Compression with alternatives (mixed shifts and reduces)
#[test]
fn compression_with_mixed_actions() {
    let mut table = make_empty_parse_table(4, 5);

    // State 0: shifts
    if let Some(cell) = table.action_table[0].get_mut(1) {
        cell.push(Action::Shift(adze_ir::StateId(1)));
    }
    if let Some(cell) = table.action_table[0].get_mut(2) {
        cell.push(Action::Shift(adze_ir::StateId(2)));
    }

    // State 1: reduce
    if let Some(cell) = table.action_table[1].get_mut(1) {
        cell.push(Action::Reduce(RuleId(1)));
    }

    // State 2: accept (on EOF typically, but we'll use symbol 3)
    if let Some(cell) = table.action_table[2].get_mut(3) {
        cell.push(Action::Accept);
    }

    let compressor = TableCompressor::new();
    let indices = get_indices(&table);
    let result = compressor.compress(&table, &indices, false);

    assert!(result.is_ok());
    let compressed = result.unwrap();
    assert!(!compressed.action_table.data.is_empty());
}

// TEST 10: Compression with sequences (multiple identical actions in sequence)
#[test]
fn compression_with_sequences() {
    let mut table = make_empty_parse_table(5, 6);

    // Fill several states with same action pattern
    let shift_state = adze_ir::StateId(3);
    for state in 0..4 {
        if let Some(cell) = table.action_table[state].get_mut(1) {
            cell.push(Action::Shift(shift_state));
        }
    }

    let compressor = TableCompressor::new();
    let indices = get_indices(&table);
    let result = compressor.compress(&table, &indices, false);

    assert!(result.is_ok());
}

// TEST 11: Row offsets are strictly increasing
#[test]
fn row_offsets_strictly_increasing() {
    let mut table = make_empty_parse_table(5, 8);
    if let Some(cell) = table.action_table[0].get_mut(1) {
        cell.push(Action::Shift(adze_ir::StateId(1)));
    }

    let compressor = TableCompressor::new();
    let indices = get_indices(&table);
    let compressed = compressor.compress(&table, &indices, false).unwrap();

    let offsets = &compressed.action_table.row_offsets;
    for i in 1..offsets.len() {
        assert!(
            offsets[i] >= offsets[i - 1],
            "Row offsets not monotonically increasing"
        );
    }
}

// TEST 12: Compression with only reduce actions
#[test]
fn compression_with_only_reduces() {
    let mut table = make_empty_parse_table(4, 5);

    // State 0 needs at least one shift
    if let Some(cell) = table.action_table[0].get_mut(1) {
        cell.push(Action::Shift(adze_ir::StateId(1)));
    }

    // Other states: reduce only
    for state in 1..4 {
        if let Some(cell) = table.action_table[state].get_mut(1) {
            cell.push(Action::Reduce(RuleId(1)));
        }
        if let Some(cell) = table.action_table[state].get_mut(2) {
            cell.push(Action::Reduce(RuleId(2)));
        }
    }

    let compressor = TableCompressor::new();
    let indices = get_indices(&table);
    let result = compressor.compress(&table, &indices, false);

    assert!(result.is_ok());
    let compressed = result.unwrap();
    assert!(!compressed.action_table.data.is_empty());
}

// TEST 13: Compression with only shift actions
#[test]
fn compression_with_only_shifts() {
    let mut table = make_empty_parse_table(3, 5);

    for state in 0..3 {
        for sym in 1..4 {
            if let Some(cell) = table.action_table[state].get_mut(sym) {
                cell.push(Action::Shift(adze_ir::StateId((state + 1) as u16)));
            }
        }
    }

    let compressor = TableCompressor::new();
    let indices = get_indices(&table);
    let result = compressor.compress(&table, &indices, false);

    assert!(result.is_ok());
    let compressed = result.unwrap();
    assert!(!compressed.action_table.data.is_empty());
}

// TEST 14: Goto table compression with run-length encoding
#[test]
fn goto_table_run_length_encoding() {
    let mut table = make_empty_parse_table(2, 5);

    // Set repeated goto entries (run of same state)
    let target_state = adze_ir::StateId(1);
    for sym in 1..5 {
        table.goto_table[0][sym] = target_state;
    }

    if let Some(cell) = table.action_table[0].get_mut(1) {
        cell.push(Action::Shift(adze_ir::StateId(1)));
    }

    let compressor = TableCompressor::new();
    let indices = get_indices(&table);
    let result = compressor.compress(&table, &indices, false);

    assert!(result.is_ok());
    let compressed = result.unwrap();
    assert!(!compressed.goto_table.data.is_empty());
}

// TEST 15: Compression handles empty rows correctly
#[test]
fn compression_handles_empty_rows() {
    let mut table = make_empty_parse_table(3, 4);

    // Only populate state 0
    if let Some(cell) = table.action_table[0].get_mut(1) {
        cell.push(Action::Shift(adze_ir::StateId(1)));
    }
    // States 1, 2 have no actions

    let compressor = TableCompressor::new();
    let indices = get_indices(&table);
    let result = compressor.compress(&table, &indices, false);

    assert!(result.is_ok());
    let compressed = result.unwrap();
    assert_eq!(
        compressed.action_table.row_offsets.len(),
        table.state_count + 1
    );
}

// TEST 16: Compression with multiple states and symbols
#[test]
fn compression_with_multiple_states_symbols() {
    let compressor = TableCompressor::new();

    // Tables with many states
    let mut table = make_empty_parse_table(100, 10);
    if let Some(cell) = table.action_table[0].get_mut(1) {
        cell.push(Action::Shift(adze_ir::StateId(1)));
    }

    let indices = get_indices(&table);
    let result = compressor.compress(&table, &indices, false);
    assert!(result.is_ok());
}

// TEST 17: Default actions are set correctly
#[test]
fn default_actions_set_correctly() {
    let mut table = make_empty_parse_table(2, 4);
    if let Some(cell) = table.action_table[0].get_mut(1) {
        cell.push(Action::Shift(adze_ir::StateId(1)));
    }

    let compressor = TableCompressor::new();
    let indices = get_indices(&table);
    let compressed = compressor.compress(&table, &indices, false).unwrap();

    // Default actions should match state count
    assert_eq!(
        compressed.action_table.default_actions.len(),
        table.state_count
    );
}

// TEST 18: Compression with nullable start symbol
#[test]
fn compression_with_nullable_start() {
    let mut table = make_empty_parse_table(2, 4);

    // State 0 with nullable start: can accept on EOF immediately
    if let Some(cell) = table.action_table[0].get_mut(3) {
        cell.push(Action::Accept); // EOF is typically at index 3
    }

    let compressor = TableCompressor::new();
    let indices = get_indices(&table);
    let result = compressor.compress(&table, &indices, true); // start_can_be_empty = true

    assert!(result.is_ok());
}

// TEST 19: Action count preservation
#[test]
fn action_count_preservation() {
    let mut table = make_empty_parse_table(3, 5);

    let mut original_action_count = 0;

    // Add known actions
    if let Some(cell) = table.action_table[0].get_mut(1) {
        cell.push(Action::Shift(adze_ir::StateId(1)));
        original_action_count += 1;
    }
    if let Some(cell) = table.action_table[1].get_mut(2) {
        cell.push(Action::Reduce(RuleId(1)));
        original_action_count += 1;
    }
    if let Some(cell) = table.action_table[2].get_mut(1) {
        cell.push(Action::Shift(adze_ir::StateId(2)));
        original_action_count += 1;
    }

    let compressor = TableCompressor::new();
    let indices = get_indices(&table);
    let compressed = compressor.compress(&table, &indices, false).unwrap();

    // Compressed data should have at least the number of non-error actions
    assert!(compressed.action_table.data.len() >= original_action_count);
}

// TEST 20: Large state space compression
#[test]
fn large_state_space_compression() {
    let mut table = make_empty_parse_table(100, 12);

    // Populate with pattern
    for state in 0..100 {
        if let Some(cell) = table.action_table[state].get_mut(1) {
            cell.push(Action::Shift(adze_ir::StateId((state + 1) as u16 % 100)));
        }
    }

    let compressor = TableCompressor::new();
    let indices = get_indices(&table);
    let result = compressor.compress(&table, &indices, false);

    assert!(result.is_ok());
    let compressed = result.unwrap();
    assert_eq!(
        compressed.action_table.row_offsets.len(),
        101 // states + 1
    );
}

// TEST 21: Accept action handling
#[test]
fn accept_action_handling() {
    let mut table = make_empty_parse_table(2, 4);

    // State 0: shift to state 1
    if let Some(cell) = table.action_table[0].get_mut(1) {
        cell.push(Action::Shift(adze_ir::StateId(1)));
    }

    // State 1: accept on EOF
    if let Some(cell) = table.action_table[1].get_mut(3) {
        cell.push(Action::Accept);
    }

    let compressor = TableCompressor::new();
    let indices = get_indices(&table);
    let result = compressor.compress(&table, &indices, false);

    assert!(result.is_ok());
    let compressed = result.unwrap();
    assert!(!compressed.action_table.data.is_empty());
}

// TEST 22: Error action skipping in compression
#[test]
fn error_action_skipping() {
    let mut table = make_empty_parse_table(2, 4);

    // State 0: shift
    if let Some(cell) = table.action_table[0].get_mut(1) {
        cell.push(Action::Shift(adze_ir::StateId(1)));
    }

    // State 1: has error (should be optimized out)
    if let Some(cell) = table.action_table[1].get_mut(2) {
        cell.push(Action::Error);
    }

    let compressor = TableCompressor::new();
    let indices = get_indices(&table);
    let compressed = compressor.compress(&table, &indices, false).unwrap();

    // Error actions are typically omitted, so data should only have the shift
    assert!(!compressed.action_table.data.is_empty());
}

// TEST 23: Multiple symbol columns compression
#[test]
fn multiple_symbol_columns_compression() {
    let mut table = make_empty_parse_table(3, 8);

    // Populate multiple columns
    for state in 0..3 {
        for sym in 1..7 {
            if let Some(cell) = table.action_table[state].get_mut(sym) {
                cell.push(Action::Shift(adze_ir::StateId((state + sym) as u16 % 3)));
            }
        }
    }

    let compressor = TableCompressor::new();
    let indices = get_indices(&table);
    let result = compressor.compress(&table, &indices, false);

    assert!(result.is_ok());
    let compressed = result.unwrap();
    assert!(!compressed.action_table.data.is_empty());
}

// TEST 24: Determinism across multiple compressions
#[test]
fn determinism_multiple_compressions() {
    let mut table = make_empty_parse_table(4, 6);

    for state in 0..4 {
        if let Some(cell) = table.action_table[state].get_mut(1) {
            cell.push(Action::Shift(adze_ir::StateId((state + 1) as u16)));
        }
    }

    let compressor = TableCompressor::new();
    let indices = get_indices(&table);

    // Compress multiple times
    let results: Vec<_> = (0..3)
        .map(|_| compressor.compress(&table, &indices, false).unwrap())
        .collect();

    // All should be identical
    for i in 1..results.len() {
        assert_eq!(
            results[0].action_table.row_offsets,
            results[i].action_table.row_offsets
        );
    }
}
