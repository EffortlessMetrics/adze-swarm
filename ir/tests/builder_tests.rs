//! Comprehensive builder pattern tests for IR grammar construction.

use adze_ir::builder::GrammarBuilder;
use adze_ir::{Associativity, PrecedenceKind, Symbol, TokenPattern};

// ── 1. Builder creates valid grammar with all field types ───────────────────

#[test]
fn builder_creates_valid_grammar_with_all_field_types() {
    let grammar = GrammarBuilder::new("full")
        .token("NUMBER", r"\d+")
        .token("IDENT", r"[a-z]+")
        .token("+", "+")
        .token(";", ";")
        .token("(", "(")
        .token(")", ")")
        .fragile_token("ERROR_TOK", r"[^\s]+")
        .rule("program", vec!["statement"])
        .rule("program", vec!["program", "statement"])
        .rule("statement", vec!["expression", ";"])
        .rule("expression", vec!["NUMBER"])
        .rule("expression", vec!["IDENT"])
        .rule("expression", vec!["(", "expression", ")"])
        .rule_with_precedence(
            "expression",
            vec!["expression", "+", "expression"],
            1,
            Associativity::Left,
        )
        .external("INDENT")
        .external("DEDENT")
        .extra("WS")
        .token("WS", r"\s+")
        .precedence(1, Associativity::Left, vec!["+"])
        .start("program")
        .build();

    assert_eq!(grammar.name, "full");
    // Tokens: NUMBER, IDENT, +, ;, (, ), ERROR_TOK, WS
    assert_eq!(grammar.tokens.len(), 8);
    // Rules for: program, statement, expression
    assert_eq!(grammar.rules.len(), 3);
    assert_eq!(grammar.externals.len(), 2);
    assert_eq!(grammar.extras.len(), 1);
    assert_eq!(grammar.precedences.len(), 1);

    // Start symbol should be "program" (first in rules because we called .start())
    let first_rule_name = grammar.rule_names.get(grammar.rules.keys().next().unwrap());
    assert_eq!(first_rule_name.unwrap(), "program");
}

// ── 2. Builder handles empty grammar ────────────────────────────────────────

#[test]
fn builder_handles_empty_grammar() {
    let grammar = GrammarBuilder::new("empty").build();

    assert_eq!(grammar.name, "empty");
    assert!(grammar.rules.is_empty());
    assert!(grammar.tokens.is_empty());
    assert!(grammar.precedences.is_empty());
    assert!(grammar.externals.is_empty());
    assert!(grammar.extras.is_empty());
    assert!(grammar.fields.is_empty());
    assert!(grammar.supertypes.is_empty());
    assert!(grammar.inline_rules.is_empty());
    assert!(grammar.rule_names.is_empty());
    assert!(grammar.symbol_registry.is_none());
}

// ── 3. Builder with complex nested symbols ──────────────────────────────────

#[test]
fn builder_with_complex_nested_symbols() {
    // Build a grammar that exercises multiple alternative productions
    // and epsilon (empty) rules
    let grammar = GrammarBuilder::new("nested")
        .token("a", "a")
        .token("b", "b")
        .token("c", "c")
        // nullable rule via empty production
        .rule("opt_list", vec![])
        .rule("opt_list", vec!["list"])
        // recursive rule
        .rule("list", vec!["item"])
        .rule("list", vec!["list", "item"])
        .rule("item", vec!["a"])
        .rule("item", vec!["b"])
        .rule("item", vec!["c"])
        .start("opt_list")
        .build();

    // opt_list should have an epsilon production
    let opt_list_id = grammar.find_symbol_by_name("opt_list").unwrap();
    let opt_list_rules = grammar.get_rules_for_symbol(opt_list_id).unwrap();
    assert_eq!(opt_list_rules.len(), 2);
    assert!(
        opt_list_rules
            .iter()
            .any(|r| r.rhs == vec![Symbol::Epsilon])
    );

    // list should have 2 productions (base + recursive)
    let list_id = grammar.find_symbol_by_name("list").unwrap();
    let list_rules = grammar.get_rules_for_symbol(list_id).unwrap();
    assert_eq!(list_rules.len(), 2);

    // item should have 3 alternatives
    let item_id = grammar.find_symbol_by_name("item").unwrap();
    let item_rules = grammar.get_rules_for_symbol(item_id).unwrap();
    assert_eq!(item_rules.len(), 3);
}

// ── 4. Builder with precedence and associativity ────────────────────────────

#[test]
fn builder_with_precedence_and_associativity() {
    let grammar = GrammarBuilder::new("calc")
        .token("NUM", r"\d+")
        .token("+", "+")
        .token("*", "*")
        .token("^", "^")
        .rule_with_precedence("expr", vec!["expr", "+", "expr"], 1, Associativity::Left)
        .rule_with_precedence("expr", vec!["expr", "*", "expr"], 2, Associativity::Left)
        .rule_with_precedence("expr", vec!["expr", "^", "expr"], 3, Associativity::Right)
        .rule("expr", vec!["NUM"])
        .start("expr")
        .build();

    let expr_id = grammar.find_symbol_by_name("expr").unwrap();
    let expr_rules = grammar.get_rules_for_symbol(expr_id).unwrap();
    assert_eq!(expr_rules.len(), 4);

    // Verify precedence levels and associativity on operator rules
    let add_rule = &expr_rules[0];
    assert_eq!(add_rule.precedence, Some(PrecedenceKind::Static(1)));
    assert_eq!(add_rule.associativity, Some(Associativity::Left));

    let mul_rule = &expr_rules[1];
    assert_eq!(mul_rule.precedence, Some(PrecedenceKind::Static(2)));
    assert_eq!(mul_rule.associativity, Some(Associativity::Left));

    let pow_rule = &expr_rules[2];
    assert_eq!(pow_rule.precedence, Some(PrecedenceKind::Static(3)));
    assert_eq!(pow_rule.associativity, Some(Associativity::Right));

    // Plain rule has no precedence
    let num_rule = &expr_rules[3];
    assert_eq!(num_rule.precedence, None);
    assert_eq!(num_rule.associativity, None);
}

// ── 5. Builder with external tokens ─────────────────────────────────────────

#[test]
fn builder_with_external_tokens() {
    let grammar = GrammarBuilder::new("indented")
        .token("NEWLINE", r"\n")
        .token("pass", "pass")
        .external("INDENT")
        .external("DEDENT")
        .external("NEWLINE_SCANNER")
        .rule("block", vec!["NEWLINE", "statement"])
        .rule("statement", vec!["pass"])
        .start("block")
        .build();

    assert_eq!(grammar.externals.len(), 3);
    assert_eq!(grammar.externals[0].name, "INDENT");
    assert_eq!(grammar.externals[1].name, "DEDENT");
    assert_eq!(grammar.externals[2].name, "NEWLINE_SCANNER");

    // External tokens get unique symbol IDs
    let ids: Vec<_> = grammar.externals.iter().map(|e| e.symbol_id).collect();
    assert_ne!(ids[0], ids[1]);
    assert_ne!(ids[1], ids[2]);
}

// ── 6. Builder with fragile tokens ──────────────────────────────────────────

#[test]
fn builder_with_fragile_tokens() {
    let grammar = GrammarBuilder::new("fragile_test")
        .token("NORMAL", "normal")
        .fragile_token("FRAG", "frag_pattern")
        .fragile_token("FRAG2", r"[a-z]+")
        .rule("root", vec!["NORMAL"])
        .start("root")
        .build();

    // Verify fragile flags
    let normal_tok = grammar
        .tokens
        .values()
        .find(|t| t.name == "NORMAL")
        .unwrap();
    assert!(!normal_tok.fragile);

    let frag_tok = grammar.tokens.values().find(|t| t.name == "FRAG").unwrap();
    assert!(frag_tok.fragile);
    assert_eq!(
        frag_tok.pattern,
        TokenPattern::Regex("frag_pattern".to_string())
    );

    let frag2_tok = grammar.tokens.values().find(|t| t.name == "FRAG2").unwrap();
    assert!(frag2_tok.fragile);
    assert_eq!(frag2_tok.pattern, TokenPattern::Regex("[a-z]+".to_string()));
}

// ── 7. Builder method chaining works correctly ──────────────────────────────

#[test]
fn builder_method_chaining_works_correctly() {
    // All builder methods return Self, so they can be chained in any order.
    // Verify the grammar is correct regardless of call ordering.
    let grammar = GrammarBuilder::new("chain")
        .start("root") // start before rules
        .extra("WS") // extra before its token
        .token("WS", r"\s+")
        .external("EXT") // external before rules
        .precedence(1, Associativity::Left, vec!["op"]) // precedence before token
        .token("op", "+")
        .token("NUM", r"\d+")
        .fragile_token("ERR", "???")
        .rule("root", vec!["NUM"])
        .rule("root", vec!["root", "op", "root"])
        .build();

    assert_eq!(grammar.name, "chain");
    // start symbol causes "root" rules to come first
    let first_key = grammar.rules.keys().next().unwrap();
    assert_eq!(grammar.rule_names[first_key], "root");
    assert_eq!(grammar.extras.len(), 1);
    assert_eq!(grammar.externals.len(), 1);
    assert_eq!(grammar.precedences.len(), 1);
    // fragile token present
    let err_tok = grammar.tokens.values().find(|t| t.name == "ERR").unwrap();
    assert!(err_tok.fragile);
}

// ── 8. Builder produces serializable output ─────────────────────────────────

#[test]
fn builder_produces_serializable_output() {
    let grammar = GrammarBuilder::new("serde_test")
        .token("A", "a")
        .token("B", "b")
        .rule("start", vec!["A", "B"])
        .rule("start", vec!["A"])
        .start("start")
        .build();

    // Serialize to JSON
    let json = serde_json::to_string_pretty(&grammar).expect("serialization should succeed");
    assert!(json.contains("\"name\": \"serde_test\""));
    assert!(json.contains("\"A\""));
    assert!(json.contains("\"B\""));

    // Roundtrip: deserialize back
    let roundtrip: adze_ir::Grammar =
        serde_json::from_str(&json).expect("deserialization should succeed");
    assert_eq!(roundtrip.name, grammar.name);
    assert_eq!(roundtrip.tokens.len(), grammar.tokens.len());
    assert_eq!(roundtrip.rules.len(), grammar.rules.len());
}

// ── 9. Builder with duplicate rule names (adds alternatives) ────────────────

#[test]
fn builder_with_duplicate_rule_names_adds_alternatives() {
    let grammar = GrammarBuilder::new("dupes")
        .token("X", "x")
        .token("Y", "y")
        .token("Z", "z")
        .rule("thing", vec!["X"])
        .rule("thing", vec!["Y"])
        .rule("thing", vec!["Z"])
        .rule("thing", vec!["X", "Y"])
        .start("thing")
        .build();

    // All four productions should be alternatives under the same symbol
    let thing_id = grammar.find_symbol_by_name("thing").unwrap();
    let thing_rules = grammar.get_rules_for_symbol(thing_id).unwrap();
    assert_eq!(thing_rules.len(), 4);

    // Each production gets a unique ProductionId
    let prod_ids: Vec<_> = thing_rules.iter().map(|r| r.production_id).collect();
    let unique: std::collections::HashSet<_> = prod_ids.iter().collect();
    assert_eq!(unique.len(), 4, "each production must have a unique ID");

    // The grammar should only have one entry in the rules map
    assert_eq!(grammar.rules.len(), 1);
}

// ── 10. Builder default values are sensible ─────────────────────────────────

#[test]
fn builder_default_values_are_sensible() {
    let grammar = GrammarBuilder::new("defaults")
        .token("T", "t")
        .rule("r", vec!["T"])
        .build();

    // No start symbol set → rules map uses insertion order
    assert_eq!(grammar.rules.len(), 1);

    // Default empty collections
    assert!(grammar.precedences.is_empty());
    assert!(grammar.conflicts.is_empty());
    assert!(grammar.externals.is_empty());
    assert!(grammar.extras.is_empty());
    assert!(grammar.supertypes.is_empty());
    assert!(grammar.inline_rules.is_empty());
    assert!(grammar.alias_sequences.is_empty());
    assert_eq!(grammar.production_ids.len(), grammar.all_rules().count());
    assert_eq!(grammar.max_alias_sequence_length, 0);
    assert!(grammar.symbol_registry.is_none());
    assert!(grammar.fields.is_empty());

    // Rule should have sane defaults
    let rule = &grammar.rules.values().next().unwrap()[0];
    assert_eq!(rule.precedence, None);
    assert_eq!(rule.associativity, None);
    assert!(rule.fields.is_empty());

    // Token should have default fragile=false
    let tok = grammar.tokens.values().next().unwrap();
    assert!(!tok.fragile);
}
