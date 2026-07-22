//! GrammarBuilder edge-case and pattern test suite (v9) — 85 tests across 10 categories.
//!
//! Categories:
//!  1. Single-element basics (token, rule, start)
//!  2. Many-token scaling (10, 50+ tokens)
//!  3. Multi-alternative rules (same LHS)
//!  4. Epsilon / empty-RHS productions
//!  5. Long RHS productions
//!  6. Builder ordering permutations
//!  7. Duplicate / overwrite semantics
//!  8. Inline, supertype, external, extra markers
//!  9. Grammar query methods (find_symbol_by_name, get_rules_for_symbol, start_symbol, all_rules)
//! 10. All-features-combined integration

use adze_ir::builder::GrammarBuilder;
use adze_ir::{Associativity, PrecedenceKind, Symbol, SymbolId};

// ═══════════════════════════════════════════════════════════════════════════
// Category 1 — Single-element basics (8 tests)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn single_01_builder_new_preserves_name() {
    let g = GrammarBuilder::new("my_lang").build();
    assert_eq!(g.name, "my_lang");
}

#[test]
fn single_02_empty_grammar_has_no_tokens() {
    let g = GrammarBuilder::new("empty").build();
    assert!(g.tokens.is_empty());
}

#[test]
fn single_03_empty_grammar_has_no_rules() {
    let g = GrammarBuilder::new("empty").build();
    assert!(g.rules.is_empty());
}

#[test]
fn single_04_one_token_recorded() {
    let g = GrammarBuilder::new("t").token("NUM", r"\d+").build();
    assert_eq!(g.tokens.len(), 1);
}

#[test]
fn single_05_one_rule_recorded() {
    let g = GrammarBuilder::new("t")
        .token("NUM", r"\d+")
        .rule("expr", vec!["NUM"])
        .build();
    assert_eq!(g.rules.len(), 1);
}

#[test]
fn single_06_start_moves_rule_first() {
    let g = GrammarBuilder::new("t")
        .token("A", "a")
        .rule("beta", vec!["A"])
        .rule("alpha", vec!["beta"])
        .start("alpha")
        .build();
    let first_lhs = *g.rules.keys().next().unwrap();
    assert_eq!(g.rule_names[&first_lhs], "alpha");
}

#[test]
fn single_07_token_name_stored_in_token_struct() {
    let g = GrammarBuilder::new("t").token("PLUS", "+").build();
    let tok = g.tokens.values().next().unwrap();
    assert_eq!(tok.name, "PLUS");
}

#[test]
fn single_08_rule_lhs_matches_rule_names() {
    let g = GrammarBuilder::new("t")
        .token("A", "a")
        .rule("root", vec!["A"])
        .build();
    let (&lhs_id, rules) = g.rules.iter().next().unwrap();
    assert_eq!(g.rule_names[&lhs_id], "root");
    assert_eq!(rules.len(), 1);
}

// ═══════════════════════════════════════════════════════════════════════════
// Category 2 — Many-token scaling (9 tests)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn many_tok_01_ten_tokens() {
    let mut b = GrammarBuilder::new("t");
    for i in 0..10 {
        b = b.token(&format!("T{i}"), &format!("t{i}"));
    }
    let g = b.build();
    assert_eq!(g.tokens.len(), 10);
}

#[test]
fn many_tok_02_fifty_tokens() {
    let mut b = GrammarBuilder::new("t");
    for i in 0..50 {
        b = b.token(&format!("T{i}"), &format!("t{i}"));
    }
    let g = b.build();
    assert_eq!(g.tokens.len(), 50);
}

#[test]
fn many_tok_03_hundred_tokens() {
    let mut b = GrammarBuilder::new("t");
    for i in 0..100 {
        b = b.token(&format!("T{i}"), &format!("t{i}"));
    }
    let g = b.build();
    assert_eq!(g.tokens.len(), 100);
}

#[test]
fn many_tok_04_all_tokens_have_distinct_ids() {
    let mut b = GrammarBuilder::new("t");
    for i in 0..20 {
        b = b.token(&format!("T{i}"), &format!("t{i}"));
    }
    let g = b.build();
    let ids: Vec<SymbolId> = g.tokens.keys().copied().collect();
    for (i, a) in ids.iter().enumerate() {
        for b_id in &ids[i + 1..] {
            assert_ne!(a, b_id);
        }
    }
}

#[test]
fn many_tok_05_token_order_preserved() {
    let g = GrammarBuilder::new("t")
        .token("FIRST", "f")
        .token("SECOND", "s")
        .token("THIRD", "t")
        .build();
    let names: Vec<&str> = g.tokens.values().map(|t| t.name.as_str()).collect();
    assert_eq!(names, vec!["FIRST", "SECOND", "THIRD"]);
}

#[test]
fn many_tok_06_token_with_regex_pattern() {
    let g = GrammarBuilder::new("t")
        .token("IDENT", r"[a-zA-Z_]\w*")
        .build();
    let tok = g.tokens.values().next().unwrap();
    assert_eq!(
        tok.pattern,
        adze_ir::TokenPattern::Regex(r"[a-zA-Z_]\w*".to_string())
    );
}

#[test]
fn many_tok_07_token_with_string_literal_pattern() {
    let g = GrammarBuilder::new("t").token("kw_if", "if").build();
    let tok = g.tokens.values().next().unwrap();
    assert_eq!(tok.pattern, adze_ir::TokenPattern::String("if".to_string()));
}

#[test]
fn many_tok_08_fragile_token_flag() {
    let g = GrammarBuilder::new("t")
        .fragile_token("ERR", "error")
        .build();
    let tok = g.tokens.values().next().unwrap();
    assert!(tok.fragile);
}

#[test]
fn many_tok_09_normal_token_not_fragile() {
    let g = GrammarBuilder::new("t").token("OK", "ok").build();
    let tok = g.tokens.values().next().unwrap();
    assert!(!tok.fragile);
}

// ═══════════════════════════════════════════════════════════════════════════
// Category 3 — Multi-alternative rules for the same LHS (9 tests)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn multi_alt_01_two_alternatives() {
    let g = GrammarBuilder::new("t")
        .token("A", "a")
        .token("B", "b")
        .rule("r", vec!["A"])
        .rule("r", vec!["B"])
        .build();
    assert_eq!(g.rules.len(), 1);
    let rules = g.rules.values().next().unwrap();
    assert_eq!(rules.len(), 2);
}

#[test]
fn multi_alt_02_five_alternatives() {
    let mut b = GrammarBuilder::new("t");
    for i in 0..5 {
        let tok_name = format!("T{i}");
        b = b.token(&tok_name, &format!("v{i}"));
    }
    for i in 0..5 {
        let tok_name = format!("T{i}");
        b = b.rule("r", vec![&*Box::leak(tok_name.into_boxed_str())]);
    }
    let g = b.build();
    let rules = g.rules.values().next().unwrap();
    assert_eq!(rules.len(), 5);
}

#[test]
fn multi_alt_03_each_alternative_gets_unique_production_id() {
    let g = GrammarBuilder::new("t")
        .token("A", "a")
        .token("B", "b")
        .rule("r", vec!["A"])
        .rule("r", vec!["B"])
        .build();
    let rules = g.rules.values().next().unwrap();
    assert_ne!(rules[0].production_id, rules[1].production_id);
}

#[test]
fn multi_alt_04_alternatives_preserve_order() {
    let g = GrammarBuilder::new("t")
        .token("X", "x")
        .token("Y", "y")
        .token("Z", "z")
        .rule("r", vec!["X"])
        .rule("r", vec!["Y"])
        .rule("r", vec!["Z"])
        .build();
    let rules = g.rules.values().next().unwrap();
    assert_eq!(rules.len(), 3);
    // First alternative references X, last references Z
    assert_ne!(rules[0].rhs, rules[2].rhs);
}

#[test]
fn multi_alt_05_different_lhs_are_separate_entries() {
    let g = GrammarBuilder::new("t")
        .token("A", "a")
        .rule("alpha", vec!["A"])
        .rule("beta", vec!["A"])
        .build();
    assert_eq!(g.rules.len(), 2);
}

#[test]
fn multi_alt_06_mixed_lhs_and_shared_lhs() {
    let g = GrammarBuilder::new("t")
        .token("A", "a")
        .token("B", "b")
        .rule("expr", vec!["A"])
        .rule("expr", vec!["B"])
        .rule("stmt", vec!["expr"])
        .build();
    assert_eq!(g.rules.len(), 2);
    let expr_rules: Vec<_> = g
        .rules
        .iter()
        .filter(|(id, _)| g.rule_names.get(*id).map(|n| n.as_str()) == Some("expr"))
        .flat_map(|(_, rs)| rs)
        .collect();
    assert_eq!(expr_rules.len(), 2);
}

#[test]
fn multi_alt_07_ten_alternatives_same_lhs() {
    let mut b = GrammarBuilder::new("t").token("T", "t");
    for _ in 0..10 {
        b = b.rule("r", vec!["T"]);
    }
    let g = b.build();
    let rules = g.rules.values().next().unwrap();
    assert_eq!(rules.len(), 10);
}

#[test]
fn multi_alt_08_recursive_alternatives() {
    let g = GrammarBuilder::new("t")
        .token("A", "a")
        .rule("list", vec!["A"])
        .rule("list", vec!["list", "A"])
        .build();
    let rules = g.rules.values().next().unwrap();
    assert_eq!(rules.len(), 2);
    // Second alternative has two symbols in RHS
    assert_eq!(rules[1].rhs.len(), 2);
}

#[test]
fn multi_alt_09_left_and_right_recursion() {
    let g = GrammarBuilder::new("t")
        .token("X", "x")
        .rule("lr", vec!["lr", "X"]) // left recursive
        .rule("lr", vec!["X", "lr"]) // right recursive
        .rule("lr", vec!["X"]) // base case
        .build();
    let rules = g.rules.values().next().unwrap();
    assert_eq!(rules.len(), 3);
}

// ═══════════════════════════════════════════════════════════════════════════
// Category 4 — Epsilon / empty-RHS productions (8 tests)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn epsilon_01_empty_vec_produces_epsilon() {
    let g = GrammarBuilder::new("t").rule("empty", vec![]).build();
    let rules = g.rules.values().next().unwrap();
    assert_eq!(rules[0].rhs.len(), 1);
    assert!(matches!(rules[0].rhs[0], Symbol::Epsilon));
}

#[test]
fn epsilon_02_epsilon_alongside_nonempty_alt() {
    let g = GrammarBuilder::new("t")
        .token("A", "a")
        .rule("opt", vec![])
        .rule("opt", vec!["A"])
        .build();
    let rules = g.rules.values().next().unwrap();
    assert_eq!(rules.len(), 2);
    let has_epsilon = rules
        .iter()
        .any(|r| r.rhs.len() == 1 && matches!(r.rhs[0], Symbol::Epsilon));
    assert!(has_epsilon);
}

#[test]
fn epsilon_03_multiple_epsilon_alts() {
    let g = GrammarBuilder::new("t")
        .rule("e", vec![])
        .rule("e", vec![])
        .build();
    let rules = g.rules.values().next().unwrap();
    assert_eq!(rules.len(), 2);
    for r in rules {
        assert!(matches!(r.rhs[0], Symbol::Epsilon));
    }
}

#[test]
fn epsilon_04_epsilon_production_id_distinct() {
    let g = GrammarBuilder::new("t")
        .rule("e", vec![])
        .rule("e", vec![])
        .build();
    let rules = g.rules.values().next().unwrap();
    assert_ne!(rules[0].production_id, rules[1].production_id);
}

#[test]
fn epsilon_05_nullable_start_like_python() {
    let g = GrammarBuilder::python_like();
    let module_id = g
        .rule_names
        .iter()
        .find(|(_, name)| name.as_str() == "module")
        .map(|(id, _)| *id)
        .unwrap();
    let rules = &g.rules[&module_id];
    assert!(
        rules
            .iter()
            .any(|r| r.rhs.len() == 1 && matches!(r.rhs[0], Symbol::Epsilon))
    );
}

#[test]
fn epsilon_06_nonnullable_start_like_javascript() {
    let g = GrammarBuilder::javascript_like();
    let program_id = g
        .rule_names
        .iter()
        .find(|(_, name)| name.as_str() == "program")
        .map(|(id, _)| *id)
        .unwrap();
    let rules = &g.rules[&program_id];
    assert!(
        !rules
            .iter()
            .any(|r| r.rhs.len() == 1 && matches!(r.rhs[0], Symbol::Epsilon))
    );
}

#[test]
fn epsilon_07_epsilon_lhs_has_name() {
    let g = GrammarBuilder::new("t").rule("nullable", vec![]).build();
    let lhs = *g.rules.keys().next().unwrap();
    assert_eq!(g.rule_names[&lhs], "nullable");
}

#[test]
fn epsilon_08_epsilon_with_start() {
    let g = GrammarBuilder::new("t")
        .rule("root", vec![])
        .start("root")
        .build();
    let first_lhs = *g.rules.keys().next().unwrap();
    assert_eq!(g.rule_names[&first_lhs], "root");
}

// ═══════════════════════════════════════════════════════════════════════════
// Category 5 — Long RHS productions (8 tests)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn long_rhs_01_ten_symbols() {
    let mut b = GrammarBuilder::new("t");
    let names: Vec<String> = (0..10).map(|i| format!("t{i}")).collect();
    for n in &names {
        b = b.token(n, n);
    }
    let refs: Vec<&str> = names.iter().map(|s| s.as_str()).collect();
    b = b.rule("long", refs);
    let g = b.build();
    let rules = g.rules.values().next().unwrap();
    assert_eq!(rules[0].rhs.len(), 10);
}

#[test]
fn long_rhs_02_twenty_symbols() {
    let mut b = GrammarBuilder::new("t");
    let names: Vec<String> = (0..20).map(|i| format!("t{i}")).collect();
    for n in &names {
        b = b.token(n, n);
    }
    let refs: Vec<&str> = names.iter().map(|s| s.as_str()).collect();
    b = b.rule("long", refs);
    let g = b.build();
    let rules = g.rules.values().next().unwrap();
    assert_eq!(rules[0].rhs.len(), 20);
}

#[test]
fn long_rhs_03_all_terminal() {
    let g = GrammarBuilder::new("t")
        .token("A", "a")
        .token("B", "b")
        .token("C", "c")
        .token("D", "d")
        .token("E", "e")
        .rule("seq", vec!["A", "B", "C", "D", "E"])
        .build();
    let rules = g.rules.values().next().unwrap();
    for sym in &rules[0].rhs {
        assert!(matches!(sym, Symbol::Terminal(_)));
    }
}

#[test]
fn long_rhs_04_mixed_terminal_nonterminal() {
    let g = GrammarBuilder::new("t")
        .token("A", "a")
        .rule("inner", vec!["A"])
        .rule("outer", vec!["A", "inner", "A", "inner"])
        .build();
    let outer_rules: Vec<_> = g
        .rules
        .iter()
        .filter(|(id, _)| g.rule_names.get(*id).map(|n| n.as_str()) == Some("outer"))
        .flat_map(|(_, rs)| rs)
        .collect();
    assert_eq!(outer_rules[0].rhs.len(), 4);
    let has_terminal = outer_rules[0]
        .rhs
        .iter()
        .any(|s| matches!(s, Symbol::Terminal(_)));
    let has_nonterminal = outer_rules[0]
        .rhs
        .iter()
        .any(|s| matches!(s, Symbol::NonTerminal(_)));
    assert!(has_terminal);
    assert!(has_nonterminal);
}

#[test]
fn long_rhs_05_function_def_pattern() {
    let g = GrammarBuilder::new("t")
        .token("def", "def")
        .token("IDENT", r"[a-z]+")
        .token("LPAREN", "(")
        .token("RPAREN", ")")
        .token("COLON", ":")
        .token("pass", "pass")
        .rule(
            "funcdef",
            vec!["def", "IDENT", "LPAREN", "RPAREN", "COLON", "pass"],
        )
        .build();
    let rules = g.rules.values().next().unwrap();
    assert_eq!(rules[0].rhs.len(), 6);
}

#[test]
fn long_rhs_06_deeply_nested_nonterminals() {
    let g = GrammarBuilder::new("t")
        .token("X", "x")
        .rule("a", vec!["X"])
        .rule("b", vec!["a"])
        .rule("c", vec!["b"])
        .rule("d", vec!["c"])
        .rule("e", vec!["d"])
        .build();
    assert_eq!(g.rules.len(), 5);
}

#[test]
fn long_rhs_07_repeated_same_token_in_rhs() {
    let g = GrammarBuilder::new("t")
        .token("X", "x")
        .rule("rep", vec!["X", "X", "X", "X", "X"])
        .build();
    let rules = g.rules.values().next().unwrap();
    assert_eq!(rules[0].rhs.len(), 5);
    for sym in &rules[0].rhs {
        match sym {
            Symbol::Terminal(id) => assert_eq!(g.tokens[id].name, "X"),
            _ => panic!("expected terminal"),
        }
    }
}

#[test]
fn long_rhs_08_if_else_chain() {
    let g = GrammarBuilder::new("t")
        .token("kw_if", "if")
        .token("kw_else", "else")
        .token("kw_elif", "elif")
        .token("EXPR", r"\w+")
        .token("COLON", ":")
        .token("BODY", r".+")
        .rule(
            "if_stmt",
            vec![
                "kw_if", "EXPR", "COLON", "BODY", "kw_elif", "EXPR", "COLON", "BODY", "kw_else",
                "COLON", "BODY",
            ],
        )
        .build();
    let rules = g.rules.values().next().unwrap();
    assert_eq!(rules[0].rhs.len(), 11);
}

// ═══════════════════════════════════════════════════════════════════════════
// Category 6 — Builder ordering permutations (9 tests)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn order_01_token_then_rule_then_start() {
    let g = GrammarBuilder::new("t")
        .token("A", "a")
        .rule("r", vec!["A"])
        .start("r")
        .build();
    assert_eq!(g.tokens.len(), 1);
    assert_eq!(g.rules.len(), 1);
}

#[test]
fn order_02_start_then_rule_then_token() {
    // start before rule definition is fine — symbol is lazily created
    let g = GrammarBuilder::new("t")
        .start("r")
        .rule("r", vec!["A"])
        .token("A", "a")
        .build();
    assert_eq!(g.rules.len(), 1);
}

#[test]
fn order_03_rule_before_token_refs_nonterminal() {
    // When a rule references a name that hasn't been registered as a token yet,
    // the builder treats it as NonTerminal
    let g = GrammarBuilder::new("t")
        .rule("r", vec!["X"])
        .token("X", "x")
        .build();
    let rules = g.rules.values().next().unwrap();
    // X was not a token at the time the rule was built, so it's NonTerminal
    assert!(matches!(rules[0].rhs[0], Symbol::NonTerminal(_)));
}

#[test]
fn order_04_token_before_rule_refs_terminal() {
    let g = GrammarBuilder::new("t")
        .token("X", "x")
        .rule("r", vec!["X"])
        .build();
    let rules = g.rules.values().next().unwrap();
    assert!(matches!(rules[0].rhs[0], Symbol::Terminal(_)));
}

#[test]
fn order_05_interleaved_tokens_and_rules() {
    let g = GrammarBuilder::new("t")
        .token("A", "a")
        .rule("r1", vec!["A"])
        .token("B", "b")
        .rule("r2", vec!["B"])
        .build();
    assert_eq!(g.tokens.len(), 2);
    assert_eq!(g.rules.len(), 2);
}

#[test]
fn order_06_extras_before_tokens() {
    let g = GrammarBuilder::new("t")
        .extra("WS")
        .token("WS", r"\s+")
        .token("A", "a")
        .rule("r", vec!["A"])
        .build();
    assert!(!g.extras.is_empty());
}

#[test]
fn order_07_inline_before_rule() {
    let g = GrammarBuilder::new("t")
        .token("A", "a")
        .inline("helper")
        .rule("helper", vec!["A"])
        .rule("main", vec!["helper"])
        .build();
    assert!(!g.inline_rules.is_empty());
}

#[test]
fn order_08_multiple_start_calls_last_wins() {
    let g = GrammarBuilder::new("t")
        .token("A", "a")
        .rule("first", vec!["A"])
        .rule("second", vec!["A"])
        .start("first")
        .start("second")
        .build();
    let first_lhs = *g.rules.keys().next().unwrap();
    assert_eq!(g.rule_names[&first_lhs], "second");
}

#[test]
fn order_09_precedence_declaration_order() {
    let g = GrammarBuilder::new("t")
        .token("A", "a")
        .token("B", "b")
        .precedence(1, Associativity::Left, vec!["A"])
        .precedence(2, Associativity::Right, vec!["B"])
        .build();
    assert_eq!(g.precedences.len(), 2);
    assert_eq!(g.precedences[0].level, 1);
    assert_eq!(g.precedences[1].level, 2);
}

// ═══════════════════════════════════════════════════════════════════════════
// Category 7 — Duplicate / overwrite semantics (8 tests)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn dup_01_duplicate_token_overwrites() {
    let g = GrammarBuilder::new("t")
        .token("A", "first")
        .token("A", "second")
        .build();
    // Same symbol name → same symbol id → token map overwrites
    assert_eq!(g.tokens.len(), 1);
    let tok = g.tokens.values().next().unwrap();
    assert_eq!(
        tok.pattern,
        adze_ir::TokenPattern::String("second".to_string())
    );
}

#[test]
fn dup_02_duplicate_token_keeps_same_symbol_id() {
    let g = GrammarBuilder::new("t")
        .token("A", "first")
        .token("A", "second")
        .build();
    // Only one key in tokens
    assert_eq!(g.tokens.len(), 1);
}

#[test]
fn dup_03_same_rule_added_twice_creates_two_alternatives() {
    let g = GrammarBuilder::new("t")
        .token("A", "a")
        .rule("r", vec!["A"])
        .rule("r", vec!["A"])
        .build();
    let rules = g.rules.values().next().unwrap();
    assert_eq!(rules.len(), 2);
}

#[test]
fn dup_04_duplicate_extra_adds_twice() {
    let g = GrammarBuilder::new("t").extra("WS").extra("WS").build();
    assert_eq!(g.extras.len(), 2);
}

#[test]
fn dup_05_duplicate_inline_adds_twice() {
    let g = GrammarBuilder::new("t")
        .inline("helper")
        .inline("helper")
        .build();
    assert_eq!(g.inline_rules.len(), 2);
}

#[test]
fn dup_06_duplicate_supertype_adds_twice() {
    let g = GrammarBuilder::new("t")
        .supertype("base")
        .supertype("base")
        .build();
    assert_eq!(g.supertypes.len(), 2);
}

#[test]
fn dup_07_duplicate_external_adds_twice() {
    let g = GrammarBuilder::new("t")
        .external("INDENT")
        .external("INDENT")
        .build();
    assert_eq!(g.externals.len(), 2);
}

#[test]
fn dup_08_token_and_rule_same_name() {
    // A name used for both token and rule shares the same SymbolId
    let g = GrammarBuilder::new("t")
        .token("item", "item")
        .rule("item", vec!["item"])
        .build();
    // Token and rule share the id
    let tok_id = *g.tokens.keys().next().unwrap();
    let rule_lhs = *g.rules.keys().next().unwrap();
    assert_eq!(tok_id, rule_lhs);
}

// ═══════════════════════════════════════════════════════════════════════════
// Category 8 — Inline, supertype, external, extra markers (10 tests)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn marker_01_inline_populates_inline_rules() {
    let g = GrammarBuilder::new("t")
        .token("A", "a")
        .rule("helper", vec!["A"])
        .inline("helper")
        .build();
    assert!(!g.inline_rules.is_empty());
}

#[test]
fn marker_02_inline_id_matches_rule_id() {
    let g = GrammarBuilder::new("t")
        .token("A", "a")
        .rule("helper", vec!["A"])
        .inline("helper")
        .build();
    let rule_lhs = *g.rules.keys().next().unwrap();
    assert!(g.inline_rules.contains(&rule_lhs));
}

#[test]
fn marker_03_supertype_populates_supertypes() {
    let g = GrammarBuilder::new("t")
        .token("A", "a")
        .rule("base", vec!["A"])
        .supertype("base")
        .build();
    assert!(!g.supertypes.is_empty());
}

#[test]
fn marker_04_supertype_id_matches_rule_id() {
    let g = GrammarBuilder::new("t")
        .token("A", "a")
        .rule("base", vec!["A"])
        .supertype("base")
        .build();
    let rule_lhs = *g.rules.keys().next().unwrap();
    assert!(g.supertypes.contains(&rule_lhs));
}

#[test]
fn marker_05_external_populates_externals() {
    let g = GrammarBuilder::new("t").external("INDENT").build();
    assert_eq!(g.externals.len(), 1);
    assert_eq!(g.externals[0].name, "INDENT");
}

#[test]
fn marker_06_multiple_externals() {
    let g = GrammarBuilder::new("t")
        .external("INDENT")
        .external("DEDENT")
        .external("NEWLINE")
        .build();
    assert_eq!(g.externals.len(), 3);
    let names: Vec<&str> = g.externals.iter().map(|e| e.name.as_str()).collect();
    assert!(names.contains(&"INDENT"));
    assert!(names.contains(&"DEDENT"));
    assert!(names.contains(&"NEWLINE"));
}

#[test]
fn marker_07_extra_populates_extras() {
    let g = GrammarBuilder::new("t").extra("WS").build();
    assert!(!g.extras.is_empty());
}

#[test]
fn marker_08_multiple_extras() {
    let g = GrammarBuilder::new("t")
        .extra("WS")
        .extra("COMMENT")
        .build();
    assert_eq!(g.extras.len(), 2);
}

#[test]
fn marker_09_inline_and_supertype_coexist() {
    let g = GrammarBuilder::new("t")
        .token("A", "a")
        .rule("helper", vec!["A"])
        .rule("base", vec!["A"])
        .inline("helper")
        .supertype("base")
        .build();
    assert!(!g.inline_rules.is_empty());
    assert!(!g.supertypes.is_empty());
}

#[test]
fn marker_10_external_has_symbol_id() {
    let g = GrammarBuilder::new("t").external("SCAN").build();
    // External token should have a non-zero symbol id
    assert_ne!(g.externals[0].symbol_id, SymbolId(0));
}

// ═══════════════════════════════════════════════════════════════════════════
// Category 9 — Grammar query methods (14 tests)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn query_01_find_symbol_by_name_for_rule() {
    let g = GrammarBuilder::new("t")
        .token("A", "a")
        .rule("expr", vec!["A"])
        .build();
    let found = g.find_symbol_by_name("expr");
    assert!(found.is_some());
}

#[test]
fn query_02_find_symbol_by_name_nonexistent_returns_none() {
    let g = GrammarBuilder::new("t")
        .token("A", "a")
        .rule("expr", vec!["A"])
        .build();
    assert!(g.find_symbol_by_name("nonexistent").is_none());
}

#[test]
fn query_03_find_symbol_by_name_uppercase_token_not_in_rule_names() {
    // All-uppercase names are not added to rule_names by the builder
    let g = GrammarBuilder::new("t").token("NUMBER", r"\d+").build();
    assert!(g.find_symbol_by_name("NUMBER").is_none());
}

#[test]
fn query_04_find_symbol_by_name_lowercase_token_in_rule_names() {
    // Mixed/lowercase names ARE added to rule_names
    let g = GrammarBuilder::new("t").token("kw_if", "if").build();
    assert!(g.find_symbol_by_name("kw_if").is_some());
}

#[test]
fn query_05_get_rules_for_symbol_returns_correct_rules() {
    let g = GrammarBuilder::new("t")
        .token("A", "a")
        .token("B", "b")
        .rule("expr", vec!["A"])
        .rule("expr", vec!["B"])
        .build();
    let id = g.find_symbol_by_name("expr").unwrap();
    let rules = g.get_rules_for_symbol(id).unwrap();
    assert_eq!(rules.len(), 2);
}

#[test]
fn query_06_get_rules_for_symbol_nonexistent_returns_none() {
    let g = GrammarBuilder::new("t")
        .token("A", "a")
        .rule("expr", vec!["A"])
        .build();
    assert!(g.get_rules_for_symbol(SymbolId(999)).is_none());
}

#[test]
fn query_07_all_rules_count() {
    let g = GrammarBuilder::new("t")
        .token("A", "a")
        .token("B", "b")
        .rule("expr", vec!["A"])
        .rule("expr", vec!["B"])
        .rule("stmt", vec!["expr"])
        .build();
    assert_eq!(g.all_rules().count(), 3);
}

#[test]
fn query_08_all_rules_empty_grammar() {
    let g = GrammarBuilder::new("t").build();
    assert_eq!(g.all_rules().count(), 0);
}

#[test]
fn query_09_start_symbol_first_rule_key() {
    let g = GrammarBuilder::new("t")
        .token("A", "a")
        .rule("root", vec!["A"])
        .start("root")
        .build();
    // start_symbol() has complex logic; verify the first rule key is "root"
    let first_key = *g.rules.keys().next().unwrap();
    assert_eq!(g.rule_names[&first_key], "root");
}

#[test]
fn query_10_start_symbol_without_start_call() {
    let g = GrammarBuilder::new("t")
        .token("A", "a")
        .rule("expr", vec!["A"])
        .build();
    // Builder pins the first rule LHS as explicit start metadata.
    assert_eq!(g.start_symbol(), g.explicit_start_symbol());
    assert!(g.start_symbol().is_some());
}

#[test]
fn query_11_rule_names_map_contains_nonterminals() {
    let g = GrammarBuilder::new("t")
        .token("A", "a")
        .rule("expr", vec!["A"])
        .rule("stmt", vec!["expr"])
        .build();
    let names: Vec<&str> = g.rule_names.values().map(|s| s.as_str()).collect();
    assert!(names.contains(&"expr"));
    assert!(names.contains(&"stmt"));
}

#[test]
fn query_12_symbol_id_is_copy() {
    let g = GrammarBuilder::new("t")
        .token("A", "a")
        .rule("expr", vec!["A"])
        .build();
    let id = g.find_symbol_by_name("expr").unwrap();
    // SymbolId is Copy — use it twice without clone
    let _a = id;
    let _b = id;
    assert_eq!(_a, _b);
}

#[test]
fn query_13_rule_rhs_terminal_resolves_to_token() {
    let g = GrammarBuilder::new("t")
        .token("NUM", r"\d+")
        .rule("expr", vec!["NUM"])
        .build();
    let rules = g.rules.values().next().unwrap();
    match &rules[0].rhs[0] {
        Symbol::Terminal(id) => {
            assert!(g.tokens.contains_key(id));
            assert_eq!(g.tokens[id].name, "NUM");
        }
        _ => panic!("expected Terminal"),
    }
}

#[test]
fn query_14_rule_rhs_nonterminal_resolves_to_rule() {
    let g = GrammarBuilder::new("t")
        .token("A", "a")
        .rule("inner", vec!["A"])
        .rule("outer", vec!["inner"])
        .build();
    let outer_id = g.find_symbol_by_name("outer").unwrap();
    let outer_rules = g.get_rules_for_symbol(outer_id).unwrap();
    match &outer_rules[0].rhs[0] {
        Symbol::NonTerminal(id) => {
            assert!(g.rules.contains_key(id));
        }
        _ => panic!("expected NonTerminal"),
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Category 10 — Precedence and associativity (10 tests)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn prec_01_left_assoc() {
    let g = GrammarBuilder::new("t")
        .token("N", r"\d+")
        .token("PLUS", "+")
        .rule_with_precedence("expr", vec!["expr", "PLUS", "expr"], 1, Associativity::Left)
        .rule("expr", vec!["N"])
        .build();
    let id = g.find_symbol_by_name("expr").unwrap();
    let rules = g.get_rules_for_symbol(id).unwrap();
    let prec_rule = rules.iter().find(|r| r.precedence.is_some()).unwrap();
    assert_eq!(prec_rule.associativity, Some(Associativity::Left));
}

#[test]
fn prec_02_right_assoc() {
    let g = GrammarBuilder::new("t")
        .token("N", r"\d+")
        .token("POW", "**")
        .rule_with_precedence("expr", vec!["expr", "POW", "expr"], 3, Associativity::Right)
        .rule("expr", vec!["N"])
        .build();
    let id = g.find_symbol_by_name("expr").unwrap();
    let rules = g.get_rules_for_symbol(id).unwrap();
    let prec_rule = rules.iter().find(|r| r.precedence.is_some()).unwrap();
    assert_eq!(prec_rule.associativity, Some(Associativity::Right));
}

#[test]
fn prec_03_none_assoc() {
    let g = GrammarBuilder::new("t")
        .token("N", r"\d+")
        .token("EQ", "==")
        .rule_with_precedence("cmp", vec!["cmp", "EQ", "cmp"], 0, Associativity::None)
        .rule("cmp", vec!["N"])
        .build();
    let id = g.find_symbol_by_name("cmp").unwrap();
    let rules = g.get_rules_for_symbol(id).unwrap();
    let prec_rule = rules.iter().find(|r| r.precedence.is_some()).unwrap();
    assert_eq!(prec_rule.associativity, Some(Associativity::None));
}

#[test]
fn prec_04_static_precedence_value() {
    let g = GrammarBuilder::new("t")
        .token("N", r"\d+")
        .token("STAR", "*")
        .rule_with_precedence("expr", vec!["expr", "STAR", "expr"], 5, Associativity::Left)
        .rule("expr", vec!["N"])
        .build();
    let id = g.find_symbol_by_name("expr").unwrap();
    let rules = g.get_rules_for_symbol(id).unwrap();
    let prec_rule = rules.iter().find(|r| r.precedence.is_some()).unwrap();
    assert_eq!(prec_rule.precedence, Some(PrecedenceKind::Static(5)));
}

#[test]
fn prec_05_negative_precedence() {
    let g = GrammarBuilder::new("t")
        .token("N", r"\d+")
        .token("OP", "~")
        .rule_with_precedence("expr", vec!["OP", "expr"], -1, Associativity::Right)
        .rule("expr", vec!["N"])
        .build();
    let id = g.find_symbol_by_name("expr").unwrap();
    let rules = g.get_rules_for_symbol(id).unwrap();
    let prec_rule = rules.iter().find(|r| r.precedence.is_some()).unwrap();
    assert_eq!(prec_rule.precedence, Some(PrecedenceKind::Static(-1)));
}

#[test]
fn prec_06_zero_precedence() {
    let g = GrammarBuilder::new("t")
        .token("N", r"\d+")
        .token("OP", "?")
        .rule_with_precedence("expr", vec!["expr", "OP"], 0, Associativity::Left)
        .rule("expr", vec!["N"])
        .build();
    let id = g.find_symbol_by_name("expr").unwrap();
    let rules = g.get_rules_for_symbol(id).unwrap();
    let prec_rule = rules.iter().find(|r| r.precedence.is_some()).unwrap();
    assert_eq!(prec_rule.precedence, Some(PrecedenceKind::Static(0)));
}

#[test]
fn prec_07_multiple_precedence_levels() {
    let g = GrammarBuilder::new("t")
        .token("N", r"\d+")
        .token("PLUS", "+")
        .token("STAR", "*")
        .rule_with_precedence("expr", vec!["expr", "PLUS", "expr"], 1, Associativity::Left)
        .rule_with_precedence("expr", vec!["expr", "STAR", "expr"], 2, Associativity::Left)
        .rule("expr", vec!["N"])
        .build();
    let id = g.find_symbol_by_name("expr").unwrap();
    let rules = g.get_rules_for_symbol(id).unwrap();
    let prec_values: Vec<i16> = rules
        .iter()
        .filter_map(|r| match r.precedence {
            Some(PrecedenceKind::Static(v)) => Some(v),
            _ => None,
        })
        .collect();
    assert!(prec_values.contains(&1));
    assert!(prec_values.contains(&2));
}

#[test]
fn prec_08_rule_without_prec_has_none() {
    let g = GrammarBuilder::new("t")
        .token("A", "a")
        .rule("r", vec!["A"])
        .build();
    let rules = g.rules.values().next().unwrap();
    assert!(rules[0].precedence.is_none());
    assert!(rules[0].associativity.is_none());
}

#[test]
fn prec_09_precedence_declaration_symbols() {
    let g = GrammarBuilder::new("t")
        .token("PLUS", "+")
        .token("STAR", "*")
        .precedence(1, Associativity::Left, vec!["PLUS"])
        .precedence(2, Associativity::Left, vec!["STAR"])
        .build();
    assert_eq!(g.precedences.len(), 2);
    assert_eq!(g.precedences[0].associativity, Associativity::Left);
    assert_eq!(g.precedences[1].level, 2);
}

#[test]
fn prec_10_precedence_declaration_multiple_symbols() {
    let g = GrammarBuilder::new("t")
        .token("PLUS", "+")
        .token("MINUS", "-")
        .precedence(1, Associativity::Left, vec!["PLUS", "MINUS"])
        .build();
    assert_eq!(g.precedences[0].symbols.len(), 2);
}

// ═══════════════════════════════════════════════════════════════════════════
// Category 11 — All-features-combined integration (5 tests)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn integration_01_kitchen_sink_grammar() {
    let g = GrammarBuilder::new("kitchen_sink")
        .token("NUM", r"\d+")
        .token("PLUS", "+")
        .token("STAR", "*")
        .token("LPAREN", "(")
        .token("RPAREN", ")")
        .token("WS", r"\s+")
        .extra("WS")
        .external("INDENT")
        .external("DEDENT")
        .rule("expr", vec!["NUM"])
        .rule_with_precedence("expr", vec!["expr", "PLUS", "expr"], 1, Associativity::Left)
        .rule_with_precedence("expr", vec!["expr", "STAR", "expr"], 2, Associativity::Left)
        .rule("expr", vec!["LPAREN", "expr", "RPAREN"])
        .rule("program", vec!["expr"])
        .inline("expr")
        .supertype("program")
        .precedence(1, Associativity::Left, vec!["PLUS"])
        .precedence(2, Associativity::Left, vec!["STAR"])
        .start("program")
        .build();

    // Verify all features are present
    assert_eq!(g.name, "kitchen_sink");
    assert_eq!(g.tokens.len(), 6);
    assert!(!g.extras.is_empty());
    assert_eq!(g.externals.len(), 2);
    assert!(!g.inline_rules.is_empty());
    assert!(!g.supertypes.is_empty());
    assert_eq!(g.precedences.len(), 2);
    assert_eq!(g.all_rules().count(), 5);

    // Start symbol comes first in rules map
    let first_key = *g.rules.keys().next().unwrap();
    assert_eq!(g.rule_names[&first_key], "program");
}

#[test]
fn integration_02_python_preset_has_externals() {
    let g = GrammarBuilder::python_like();
    assert!(!g.externals.is_empty());
}

#[test]
fn integration_03_javascript_preset_has_extras() {
    let g = GrammarBuilder::javascript_like();
    assert!(!g.extras.is_empty());
}

#[test]
fn integration_04_grammar_name_special_chars() {
    let g = GrammarBuilder::new("my-lang_v2.0").build();
    assert_eq!(g.name, "my-lang_v2.0");
}

#[test]
fn integration_05_grammar_fields_default_empty() {
    let g = GrammarBuilder::new("t")
        .token("A", "a")
        .rule("r", vec!["A"])
        .build();
    assert!(g.fields.is_empty());
    assert!(g.alias_sequences.is_empty());
    assert!(g.production_ids.is_empty());
    assert_eq!(g.max_alias_sequence_length, 0);
}
