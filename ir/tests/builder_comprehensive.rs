//! Comprehensive tests for the GrammarBuilder API.
//!
//! Exercises every public method of `GrammarBuilder` including edge cases,
//! complex grammar patterns, and builder method chaining.

use adze_ir::builder::GrammarBuilder;
use adze_ir::*;

// ── 1. GrammarBuilder::new — various name formats ──────────────────────────

#[test]
fn new_simple_name() {
    let g = GrammarBuilder::new("json").build();
    assert_eq!(g.name, "json");
}

#[test]
fn new_snake_case_name() {
    let g = GrammarBuilder::new("my_grammar_v2").build();
    assert_eq!(g.name, "my_grammar_v2");
}

#[test]
fn new_hyphenated_name() {
    let g = GrammarBuilder::new("tree-sitter-rust").build();
    assert_eq!(g.name, "tree-sitter-rust");
}

#[test]
fn new_empty_name() {
    let g = GrammarBuilder::new("").build();
    assert_eq!(g.name, "");
}

#[test]
fn new_unicode_name() {
    let g = GrammarBuilder::new("語法").build();
    assert_eq!(g.name, "語法");
}

// ── 2. .token — simple tokens, regex patterns ──────────────────────────────

#[test]
fn token_string_literal() {
    let g = GrammarBuilder::new("t").token("if", "if").build();

    let tok = g.tokens.values().find(|t| t.name == "if").unwrap();
    assert_eq!(tok.pattern, TokenPattern::String("if".to_string()));
    assert!(!tok.fragile);
}

#[test]
fn token_regex_pattern() {
    let g = GrammarBuilder::new("t")
        .token("NUMBER", r"\d+(\.\d+)?")
        .build();

    let tok = g.tokens.values().find(|t| t.name == "NUMBER").unwrap();
    assert!(matches!(&tok.pattern, TokenPattern::Regex(_)));
}

#[test]
fn token_operator_symbols() {
    let g = GrammarBuilder::new("t")
        .token("+", "+")
        .token("==", "==")
        .token("!=", "!=")
        .build();

    assert_eq!(g.tokens.len(), 3);
}

#[test]
fn token_explicit_regex_delimiters() {
    let g = GrammarBuilder::new("t").token("ID", "/[a-z]+/").build();

    let tok = g.tokens.values().next().unwrap();
    // The builder strips the / delimiters for regex
    assert_eq!(tok.pattern, TokenPattern::Regex("[a-z]+".to_string()));
}

// ── 3. .fragile_token — keyword tokens ─────────────────────────────────────

#[test]
fn fragile_token_marked_fragile() {
    let g = GrammarBuilder::new("t")
        .fragile_token("class", "class")
        .build();

    let tok = g.tokens.values().next().unwrap();
    assert!(tok.fragile);
    assert_eq!(tok.pattern, TokenPattern::String("class".to_string()));
}

#[test]
fn fragile_token_regex_pattern() {
    let g = GrammarBuilder::new("t")
        .fragile_token("KW", r"(if|else|while)")
        .build();

    let tok = g.tokens.values().next().unwrap();
    assert!(tok.fragile);
    assert_eq!(
        tok.pattern,
        TokenPattern::Regex("(if|else|while)".to_string())
    );
}

#[test]
fn mixed_normal_and_fragile_tokens() {
    let g = GrammarBuilder::new("t")
        .token("NUMBER", r"\d+")
        .fragile_token("return", "return")
        .token("IDENT", r"[a-z]+")
        .fragile_token("break", "break")
        .build();

    assert_eq!(g.tokens.len(), 4);
    let fragile_count = g.tokens.values().filter(|t| t.fragile).count();
    assert_eq!(fragile_count, 2);
}

// ── 4. .rule — single and multi-symbol rules ───────────────────────────────

#[test]
fn rule_single_terminal() {
    let g = GrammarBuilder::new("t")
        .token("A", "a")
        .rule("start", vec!["A"])
        .build();

    let rules = g.rules.values().next().unwrap();
    assert_eq!(rules.len(), 1);
    assert_eq!(rules[0].rhs.len(), 1);
    assert!(matches!(rules[0].rhs[0], Symbol::Terminal(_)));
}

#[test]
fn rule_multiple_symbols() {
    let g = GrammarBuilder::new("t")
        .token("A", "a")
        .token("B", "b")
        .token("C", "c")
        .rule("seq", vec!["A", "B", "C"])
        .build();

    let rules = g.rules.values().next().unwrap();
    assert_eq!(rules[0].rhs.len(), 3);
}

#[test]
fn rule_empty_rhs_produces_epsilon() {
    let g = GrammarBuilder::new("t").rule("empty", vec![]).build();

    let rules = g.rules.values().next().unwrap();
    assert_eq!(rules[0].rhs, vec![Symbol::Epsilon]);
}

#[test]
fn rule_nonterminal_reference() {
    let g = GrammarBuilder::new("t")
        .token("A", "a")
        .rule("inner", vec!["A"])
        .rule("outer", vec!["inner"])
        .build();

    let outer_id = g.find_symbol_by_name("outer").unwrap();
    let outer_rules = g.get_rules_for_symbol(outer_id).unwrap();
    // "inner" is not a token, so it should be NonTerminal
    assert!(matches!(outer_rules[0].rhs[0], Symbol::NonTerminal(_)));
}

#[test]
fn rule_multiple_alternatives_same_lhs() {
    let g = GrammarBuilder::new("t")
        .token("A", "a")
        .token("B", "b")
        .rule("choice", vec!["A"])
        .rule("choice", vec!["B"])
        .rule("choice", vec!["A", "B"])
        .build();

    let choice_id = g.find_symbol_by_name("choice").unwrap();
    let rules = g.get_rules_for_symbol(choice_id).unwrap();
    assert_eq!(rules.len(), 3);
    // Each alternative gets a unique production ID
    let ids: std::collections::HashSet<_> = rules.iter().map(|r| r.production_id).collect();
    assert_eq!(ids.len(), 3);
}

// ── 5. .rule_with_precedence — all associativities ─────────────────────────

#[test]
fn rule_with_precedence_left() {
    let g = GrammarBuilder::new("t")
        .token("N", r"\d+")
        .token("+", "+")
        .rule_with_precedence("e", vec!["e", "+", "e"], 1, Associativity::Left)
        .rule("e", vec!["N"])
        .build();

    let e_id = g.find_symbol_by_name("e").unwrap();
    let rules = g.get_rules_for_symbol(e_id).unwrap();
    let add = &rules[0];
    assert_eq!(add.precedence, Some(PrecedenceKind::Static(1)));
    assert_eq!(add.associativity, Some(Associativity::Left));
    // The plain rule has no precedence
    assert_eq!(rules[1].precedence, None);
}

#[test]
fn rule_with_precedence_right() {
    let g = GrammarBuilder::new("t")
        .token("N", r"\d+")
        .token("^", "^")
        .rule_with_precedence("e", vec!["e", "^", "e"], 3, Associativity::Right)
        .build();

    let rules = g.rules.values().next().unwrap();
    assert_eq!(rules[0].associativity, Some(Associativity::Right));
    assert_eq!(rules[0].precedence, Some(PrecedenceKind::Static(3)));
}

#[test]
fn rule_with_precedence_none() {
    let g = GrammarBuilder::new("t")
        .token("N", r"\d+")
        .token("<", "<")
        .rule_with_precedence("cmp", vec!["cmp", "<", "cmp"], 0, Associativity::None)
        .build();

    let rules = g.rules.values().next().unwrap();
    assert_eq!(rules[0].associativity, Some(Associativity::None));
}

#[test]
fn rule_with_precedence_negative_level() {
    let g = GrammarBuilder::new("t")
        .token("X", "x")
        .rule_with_precedence("low", vec!["X"], -5, Associativity::Left)
        .build();

    let rules = g.rules.values().next().unwrap();
    assert_eq!(rules[0].precedence, Some(PrecedenceKind::Static(-5)));
}

// ── 6. .start — explicit start symbol ──────────────────────────────────────

#[test]
fn start_reorders_rules() {
    let g = GrammarBuilder::new("t")
        .token("A", "a")
        .rule("alpha", vec!["A"])
        .rule("beta", vec!["A"])
        .rule("gamma", vec!["A"])
        .start("gamma")
        .build();

    let first_key = g.rules.keys().next().unwrap();
    assert_eq!(g.rule_names[first_key], "gamma");
}

#[test]
fn start_without_call_uses_insertion_order() {
    let g = GrammarBuilder::new("t")
        .token("A", "a")
        .rule("first", vec!["A"])
        .rule("second", vec!["A"])
        .build();

    let first_key = g.rules.keys().next().unwrap();
    assert_eq!(g.rule_names[first_key], "first");
}

// ── 7. .extra — whitespace/comment extras ──────────────────────────────────

#[test]
fn extra_registers_symbol() {
    let g = GrammarBuilder::new("t")
        .token("WS", r"[ \t]+")
        .extra("WS")
        .build();

    assert_eq!(g.extras.len(), 1);
}

#[test]
fn multiple_extras() {
    let g = GrammarBuilder::new("t")
        .token("WS", r"[ \t]+")
        .token("COMMENT", r"//[^\n]*")
        .token("BLOCK_COMMENT", r"/\*[^*]*\*/")
        .extra("WS")
        .extra("COMMENT")
        .extra("BLOCK_COMMENT")
        .build();

    assert_eq!(g.extras.len(), 3);
}

// ── 8. .external — external scanner tokens ─────────────────────────────────

#[test]
fn external_creates_entry() {
    let g = GrammarBuilder::new("t")
        .external("INDENT")
        .external("DEDENT")
        .build();

    assert_eq!(g.externals.len(), 2);
    assert_eq!(g.externals[0].name, "INDENT");
    assert_eq!(g.externals[1].name, "DEDENT");
    // Symbol IDs should be distinct
    assert_ne!(g.externals[0].symbol_id, g.externals[1].symbol_id);
}

// ── 9. .precedence — precedence declarations ───────────────────────────────

#[test]
fn precedence_declaration_single() {
    let g = GrammarBuilder::new("t")
        .token("+", "+")
        .precedence(1, Associativity::Left, vec!["+"])
        .build();

    assert_eq!(g.precedences.len(), 1);
    assert_eq!(g.precedences[0].level, 1);
    assert_eq!(g.precedences[0].associativity, Associativity::Left);
    assert_eq!(g.precedences[0].symbols.len(), 1);
}

#[test]
fn precedence_declaration_multiple_levels() {
    let g = GrammarBuilder::new("t")
        .token("+", "+")
        .token("-", "-")
        .token("*", "*")
        .token("/", "/")
        .precedence(1, Associativity::Left, vec!["+", "-"])
        .precedence(2, Associativity::Left, vec!["*", "/"])
        .build();

    assert_eq!(g.precedences.len(), 2);
    assert_eq!(g.precedences[0].symbols.len(), 2);
    assert_eq!(g.precedences[1].symbols.len(), 2);
    assert!(g.precedences[0].level < g.precedences[1].level);
}

// ── 10. .build — grammar construction ──────────────────────────────────────

#[test]
fn build_sets_default_fields() {
    let g = GrammarBuilder::new("t")
        .token("A", "a")
        .rule("r", vec!["A"])
        .build();

    assert!(g.conflicts.is_empty());
    assert!(g.fields.is_empty());
    assert!(g.supertypes.is_empty());
    assert!(g.inline_rules.is_empty());
    assert!(g.alias_sequences.is_empty());
    assert_eq!(g.production_ids.len(), g.all_rules().count());
    assert_eq!(g.max_alias_sequence_length, 0);
    assert!(g.symbol_registry.is_none());
}

// ── 11. Builder method chaining ────────────────────────────────────────────

#[test]
fn chaining_order_independent() {
    // Call methods in an unconventional order; the result should still be valid.
    let g = GrammarBuilder::new("chain")
        .start("root")
        .extra("WS")
        .external("EXT")
        .precedence(1, Associativity::Left, vec!["op"])
        .fragile_token("ERR", "err")
        .token("WS", r"\s+")
        .token("op", "+")
        .token("NUM", r"\d+")
        .rule("root", vec!["NUM"])
        .rule("root", vec!["root", "op", "root"])
        .build();

    assert_eq!(g.name, "chain");
    let first_key = g.rules.keys().next().unwrap();
    assert_eq!(g.rule_names[first_key], "root");
    assert_eq!(g.extras.len(), 1);
    assert_eq!(g.externals.len(), 1);
    assert_eq!(g.precedences.len(), 1);
    assert!(g.tokens.values().any(|t| t.fragile));
}

// ── 12. Multiple tokens and rules ──────────────────────────────────────────

#[test]
fn many_tokens_and_rules() {
    let g = GrammarBuilder::new("big")
        .token("A", "a")
        .token("B", "b")
        .token("C", "c")
        .token("D", "d")
        .token("E", "e")
        .rule("s1", vec!["A", "B"])
        .rule("s2", vec!["C", "D"])
        .rule("s3", vec!["s1", "E"])
        .rule("s3", vec!["s2", "E"])
        .start("s3")
        .build();

    assert_eq!(g.tokens.len(), 5);
    assert_eq!(g.rules.len(), 3);

    let s3_id = g.find_symbol_by_name("s3").unwrap();
    assert_eq!(g.get_rules_for_symbol(s3_id).unwrap().len(), 2);
}

// ── 13. Complex grammar: arithmetic ────────────────────────────────────────

#[test]
fn complex_arithmetic_grammar() {
    let g = GrammarBuilder::new("arithmetic")
        .token("NUMBER", r"\d+")
        .token("+", "+")
        .token("-", "-")
        .token("*", "*")
        .token("/", "/")
        .token("(", "(")
        .token(")", ")")
        .token("WS", r"[ \t]+")
        .extra("WS")
        .rule_with_precedence("expr", vec!["expr", "+", "expr"], 1, Associativity::Left)
        .rule_with_precedence("expr", vec!["expr", "-", "expr"], 1, Associativity::Left)
        .rule_with_precedence("expr", vec!["expr", "*", "expr"], 2, Associativity::Left)
        .rule_with_precedence("expr", vec!["expr", "/", "expr"], 2, Associativity::Left)
        .rule("expr", vec!["(", "expr", ")"])
        .rule("expr", vec!["NUMBER"])
        .start("expr")
        .precedence(1, Associativity::Left, vec!["+", "-"])
        .precedence(2, Associativity::Left, vec!["*", "/"])
        .build();

    assert_eq!(g.name, "arithmetic");
    assert_eq!(g.tokens.len(), 8);
    assert_eq!(g.extras.len(), 1);
    assert_eq!(g.precedences.len(), 2);

    let expr_id = g.find_symbol_by_name("expr").unwrap();
    let rules = g.get_rules_for_symbol(expr_id).unwrap();
    assert_eq!(rules.len(), 6); // 4 binops + parens + NUMBER

    // Verify mul has higher prec than add
    let add = rules
        .iter()
        .find(|r| {
            r.rhs.len() == 3
                && r.rhs
                    .iter()
                    .any(|s| matches!(s, Symbol::Terminal(id) if g.tokens[id].name == "+"))
        })
        .unwrap();
    let mul = rules
        .iter()
        .find(|r| {
            r.rhs.len() == 3
                && r.rhs
                    .iter()
                    .any(|s| matches!(s, Symbol::Terminal(id) if g.tokens[id].name == "*"))
        })
        .unwrap();
    assert!(matches!((add.precedence, mul.precedence),
        (Some(PrecedenceKind::Static(a)), Some(PrecedenceKind::Static(m))) if a < m
    ));
}

// ── 14. Complex grammar: if-else ───────────────────────────────────────────

#[test]
fn complex_if_else_grammar() {
    let g = GrammarBuilder::new("conditional")
        .token("if", "if")
        .token("else", "else")
        .token("true", "true")
        .token("false", "false")
        .token("(", "(")
        .token(")", ")")
        .token("{", "{")
        .token("}", "}")
        .token(";", ";")
        .fragile_token("IDENT", r"[a-z]+")
        .rule("program", vec!["stmt"])
        .rule("program", vec!["program", "stmt"])
        .rule("stmt", vec!["if_stmt"])
        .rule("stmt", vec!["expr_stmt"])
        .rule("if_stmt", vec!["if", "(", "expr", ")", "block"])
        .rule(
            "if_stmt",
            vec!["if", "(", "expr", ")", "block", "else", "block"],
        )
        .rule("block", vec!["{", "}"])
        .rule("block", vec!["{", "program", "}"])
        .rule("expr_stmt", vec!["expr", ";"])
        .rule("expr", vec!["true"])
        .rule("expr", vec!["false"])
        .rule("expr", vec!["IDENT"])
        .start("program")
        .build();

    assert_eq!(g.tokens.len(), 10);
    // Rules for: program, stmt, if_stmt, block, expr_stmt, expr
    assert_eq!(g.rules.len(), 6);

    let if_id = g.find_symbol_by_name("if_stmt").unwrap();
    let if_rules = g.get_rules_for_symbol(if_id).unwrap();
    assert_eq!(if_rules.len(), 2); // if and if-else

    // Verify the if-else alternative has 7 symbols
    assert!(if_rules.iter().any(|r| r.rhs.len() == 7));
}

// ── 15. Complex grammar: comma-separated lists ─────────────────────────────

#[test]
fn complex_list_grammar() {
    let g = GrammarBuilder::new("lists")
        .token("NUMBER", r"\d+")
        .token(",", ",")
        .token("[", "[")
        .token("]", "]")
        // list -> [ ] | [ items ]
        .rule("list", vec!["[", "]"])
        .rule("list", vec!["[", "items", "]"])
        // items -> NUMBER | items , NUMBER
        .rule("items", vec!["NUMBER"])
        .rule("items", vec!["items", ",", "NUMBER"])
        .start("list")
        .build();

    assert_eq!(g.tokens.len(), 4);
    assert_eq!(g.rules.len(), 2); // list, items

    let items_id = g.find_symbol_by_name("items").unwrap();
    let items_rules = g.get_rules_for_symbol(items_id).unwrap();
    assert_eq!(items_rules.len(), 2);

    // Recursive rule should have 3 symbols: items , NUMBER
    assert!(items_rules.iter().any(|r| r.rhs.len() == 3));
}

// ── 16. Edge case: single-token grammar ────────────────────────────────────

#[test]
fn single_token_grammar() {
    let g = GrammarBuilder::new("minimal")
        .token("X", "x")
        .rule("root", vec!["X"])
        .start("root")
        .build();

    assert_eq!(g.tokens.len(), 1);
    assert_eq!(g.rules.len(), 1);
    let rules = g.rules.values().next().unwrap();
    assert_eq!(rules.len(), 1);
    assert_eq!(rules[0].rhs.len(), 1);
}

// ── 17. Edge case: deeply nested non-terminals ─────────────────────────────

#[test]
fn deeply_nested_nonterminals() {
    let g = GrammarBuilder::new("deep")
        .token("X", "x")
        .rule("a", vec!["b"])
        .rule("b", vec!["c"])
        .rule("c", vec!["d"])
        .rule("d", vec!["e"])
        .rule("e", vec!["X"])
        .start("a")
        .build();

    assert_eq!(g.rules.len(), 5);
    // All intermediate symbols should be non-terminals
    let a_id = g.find_symbol_by_name("a").unwrap();
    let a_rules = g.get_rules_for_symbol(a_id).unwrap();
    assert!(matches!(a_rules[0].rhs[0], Symbol::NonTerminal(_)));
}

// ── 18. Serialization roundtrip ────────────────────────────────────────────

#[test]
fn serialization_roundtrip() {
    let g = GrammarBuilder::new("roundtrip")
        .token("A", "a")
        .token("B", r"\d+")
        .fragile_token("C", "c")
        .rule("s", vec!["A", "B"])
        .rule("s", vec!["C"])
        .rule("s", vec![])
        .extra("A")
        .external("EXT")
        .precedence(1, Associativity::Right, vec!["A"])
        .start("s")
        .build();

    let json = serde_json::to_string(&g).expect("serialize");
    let g2: Grammar = serde_json::from_str(&json).expect("deserialize");

    assert_eq!(g2.name, g.name);
    assert_eq!(g2.tokens.len(), g.tokens.len());
    assert_eq!(g2.rules.len(), g.rules.len());
    assert_eq!(g2.extras.len(), g.extras.len());
    assert_eq!(g2.externals.len(), g.externals.len());
    assert_eq!(g2.precedences.len(), g.precedences.len());
}

// ── 19. Token pattern auto-detection logic ─────────────────────────────────

#[test]
fn token_pattern_auto_detection() {
    let g = GrammarBuilder::new("t")
        .token("plain", "hello") // pure alpha → String
        .token("+", "+") // operator, name==pattern → String
        .token("RE", r"\d+") // has backslash → Regex
        .token("BRACKET", r"[a-z]") // has brackets → Regex
        .token("SLASH_RE", "/foo/") // explicit /.../ → Regex
        .build();

    let plain = g.tokens.values().find(|t| t.name == "plain").unwrap();
    assert!(matches!(&plain.pattern, TokenPattern::String(s) if s == "hello"));

    let plus = g.tokens.values().find(|t| t.name == "+").unwrap();
    assert!(matches!(&plus.pattern, TokenPattern::String(s) if s == "+"));

    let re = g.tokens.values().find(|t| t.name == "RE").unwrap();
    assert!(matches!(&re.pattern, TokenPattern::Regex(_)));

    let bracket = g.tokens.values().find(|t| t.name == "BRACKET").unwrap();
    assert!(matches!(&bracket.pattern, TokenPattern::Regex(_)));

    let slash = g.tokens.values().find(|t| t.name == "SLASH_RE").unwrap();
    assert_eq!(slash.pattern, TokenPattern::Regex("foo".to_string()));
}

// ── 20. Symbol ID uniqueness across all symbols ────────────────────────────

#[test]
fn symbol_ids_are_unique_across_all_symbols() {
    let g = GrammarBuilder::new("t")
        .token("A", "a")
        .token("B", "b")
        .external("EXT")
        .rule("r1", vec!["A"])
        .rule("r2", vec!["B"])
        .extra("WS")
        .token("WS", r"\s+")
        .build();

    let mut all_ids: Vec<SymbolId> = Vec::new();
    all_ids.extend(g.tokens.keys());
    all_ids.extend(g.rules.keys());
    all_ids.extend(g.externals.iter().map(|e| e.symbol_id));
    all_ids.extend(&g.extras);

    let unique: std::collections::HashSet<_> = all_ids.iter().collect();
    // Each distinct name should map to exactly one ID
    // (some IDs may appear in multiple collections, e.g. a token used as extra)
    // At minimum, token keys and rule keys should be disjoint
    let token_ids: std::collections::HashSet<_> = g.tokens.keys().collect();
    let rule_ids: std::collections::HashSet<_> = g.rules.keys().collect();
    assert!(token_ids.is_disjoint(&rule_ids));
    // We have at least 5 distinct names (A, B, EXT, r1, r2, WS)
    assert!(unique.len() >= 5);
}

// ── 21. Preset grammars: python_like and javascript_like ───────────────────

#[test]
fn preset_python_like_structure() {
    let g = GrammarBuilder::python_like();

    assert_eq!(g.name, "python_like");
    assert!(!g.externals.is_empty());
    assert!(!g.extras.is_empty());

    // Module should have an epsilon production (nullable start)
    let module_id = g.find_symbol_by_name("module").unwrap();
    let module_rules = g.get_rules_for_symbol(module_id).unwrap();
    assert!(module_rules.iter().any(|r| r.rhs == vec![Symbol::Epsilon]));

    // First rule should be for module (it's the start symbol)
    let first_key = g.rules.keys().next().unwrap();
    assert_eq!(*first_key, module_id);
}

#[test]
fn preset_javascript_like_structure() {
    let g = GrammarBuilder::javascript_like();

    assert_eq!(g.name, "javascript_like");
    assert!(g.externals.is_empty());
    assert!(!g.extras.is_empty());

    // Program should NOT have an epsilon production
    let program_id = g.find_symbol_by_name("program").unwrap();
    let program_rules = g.get_rules_for_symbol(program_id).unwrap();
    assert!(!program_rules.iter().any(|r| r.rhs == vec![Symbol::Epsilon]));

    // Should have rules with precedence
    assert!(g.all_rules().any(|r| r.precedence.is_some()));
}
