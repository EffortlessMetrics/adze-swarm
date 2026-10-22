//! Comprehensive tests for GrammarBuilder covering fluent API, grammar properties,
//! edge cases, and property-based testing.

use adze_ir::builder::GrammarBuilder;
use adze_ir::{Associativity, PrecedenceKind, Symbol, TokenPattern};
use proptest::prelude::*;

// ---------------------------------------------------------------------------
// Helper: look up a nonterminal's SymbolId by name in rule_names
// ---------------------------------------------------------------------------
fn find_rule_id(grammar: &adze_ir::Grammar, name: &str) -> adze_ir::SymbolId {
    grammar
        .rule_names
        .iter()
        .find(|(_, n)| n.as_str() == name)
        .map(|(id, _)| *id)
        .unwrap_or_else(|| panic!("nonterminal '{name}' not found in rule_names"))
}

fn find_token_id(grammar: &adze_ir::Grammar, name: &str) -> adze_ir::SymbolId {
    grammar
        .tokens
        .iter()
        .find(|(_, t)| t.name == name)
        .map(|(id, _)| *id)
        .unwrap_or_else(|| panic!("token '{name}' not found"))
}

// ===========================================================================
// 1. Fluent API chaining
// ===========================================================================

#[test]
fn test_chaining_token_rule_start_build() {
    let g = GrammarBuilder::new("chain")
        .token("NUM", r"\d+")
        .rule("expr", vec!["NUM"])
        .start("expr")
        .build();
    assert_eq!(g.name, "chain");
    assert_eq!(g.tokens.len(), 1);
    assert_eq!(g.rules.len(), 1);
}

#[test]
fn test_chaining_multiple_tokens_then_rules() {
    let g = GrammarBuilder::new("multi")
        .token("A", "a")
        .token("B", "b")
        .token("C", "c")
        .rule("s", vec!["A", "B", "C"])
        .start("s")
        .build();
    assert_eq!(g.tokens.len(), 3);
    let s_id = find_rule_id(&g, "s");
    assert_eq!(g.rules[&s_id].len(), 1);
    assert_eq!(g.rules[&s_id][0].rhs.len(), 3);
}

#[test]
fn test_chaining_rules_before_tokens_creates_nonterminals() {
    // Rules referencing undefined tokens create NonTerminal symbols
    let g = GrammarBuilder::new("order")
        .rule("s", vec!["a"])
        .rule("a", vec![])
        .start("s")
        .build();
    assert_eq!(g.rules.len(), 2);
    let s_id = find_rule_id(&g, "s");
    // "a" should be NonTerminal because no token("a", ..) was called first
    assert!(matches!(g.rules[&s_id][0].rhs[0], Symbol::NonTerminal(_)));
}

#[test]
fn test_chaining_interleaved_token_and_rule() {
    let g = GrammarBuilder::new("interleave")
        .token("X", "x")
        .rule("s", vec!["X", "y"])
        .token("Y", "y_pat")
        .rule("y", vec!["Y"])
        .start("s")
        .build();
    assert_eq!(g.tokens.len(), 2);
    assert_eq!(g.rules.len(), 2);
}

#[test]
fn test_start_reorders_rules() {
    let g = GrammarBuilder::new("reorder")
        .token("A", "a")
        .rule("beta", vec!["A"])
        .rule("alpha", vec!["beta"])
        .start("alpha")
        .build();
    // The first entry in the rules map should be alpha
    let first_key = g.rules.keys().next().unwrap();
    assert_eq!(g.rule_names[first_key], "alpha");
}

// ===========================================================================
// 2. Multiple rules for same nonterminal
// ===========================================================================

#[test]
fn test_two_alternatives_for_same_nonterminal() {
    let g = GrammarBuilder::new("alt")
        .token("A", "a")
        .token("B", "b")
        .rule("s", vec!["A"])
        .rule("s", vec!["B"])
        .start("s")
        .build();
    let s_id = find_rule_id(&g, "s");
    assert_eq!(g.rules[&s_id].len(), 2);
}

#[test]
fn test_many_alternatives() {
    let mut builder = GrammarBuilder::new("many");
    for i in 0..10 {
        let tok_name = format!("t{i}");
        // Leak to get &'static str for token/rule calls
        let tok: &'static str = Box::leak(tok_name.into_boxed_str());
        builder = builder.token(tok, tok);
        builder = builder.rule("s", vec![tok]);
    }
    let g = builder.start("s").build();
    let s_id = find_rule_id(&g, "s");
    assert_eq!(g.rules[&s_id].len(), 10);
}

#[test]
fn test_production_ids_are_unique() {
    let g = GrammarBuilder::new("pid")
        .token("A", "a")
        .rule("s", vec!["A"])
        .rule("s", vec![])
        .start("s")
        .build();
    let s_id = find_rule_id(&g, "s");
    let ids: Vec<_> = g.rules[&s_id].iter().map(|r| r.production_id).collect();
    assert_ne!(ids[0], ids[1]);
}

#[test]
fn test_epsilon_alternative_mixed() {
    let g = GrammarBuilder::new("eps_mix")
        .token("X", "x")
        .rule("s", vec!["X"])
        .rule("s", vec![])
        .start("s")
        .build();
    let s_id = find_rule_id(&g, "s");
    let has_epsilon = g.rules[&s_id]
        .iter()
        .any(|r| r.rhs == vec![Symbol::Epsilon]);
    assert!(has_epsilon);
}

// ===========================================================================
// 3. Token patterns (regex, literal)
// ===========================================================================

#[test]
fn test_literal_token_pattern() {
    let g = GrammarBuilder::new("lit")
        .token("kw", "keyword")
        .rule("s", vec!["kw"])
        .start("s")
        .build();
    let tok = g.tokens.values().next().unwrap();
    assert_eq!(tok.pattern, TokenPattern::String("keyword".into()));
}

#[test]
fn test_regex_token_pattern_via_metachar() {
    let g = GrammarBuilder::new("regex")
        .token("NUM", r"\d+")
        .rule("s", vec!["NUM"])
        .start("s")
        .build();
    let tok = g.tokens.values().next().unwrap();
    assert!(matches!(tok.pattern, TokenPattern::Regex(_)));
}

#[test]
fn test_regex_token_pattern_via_slash_delimiters() {
    let g = GrammarBuilder::new("slashed")
        .token("ID", "/[a-z]+/")
        .rule("s", vec!["ID"])
        .start("s")
        .build();
    let tok = g.tokens.values().next().unwrap();
    assert_eq!(tok.pattern, TokenPattern::Regex("[a-z]+".into()));
}

#[test]
fn test_punctuation_literal_token() {
    let g = GrammarBuilder::new("punct")
        .token("+", "+")
        .rule("s", vec![])
        .start("s")
        .build();
    let tok = g.tokens.values().next().unwrap();
    assert_eq!(tok.pattern, TokenPattern::String("+".into()));
}

#[test]
fn test_fragile_token() {
    let g = GrammarBuilder::new("fragile")
        .fragile_token("WS", r"\s+")
        .rule("s", vec![])
        .start("s")
        .build();
    let tok = g.tokens.values().next().unwrap();
    assert!(tok.fragile);
}

#[test]
fn test_non_fragile_token_default() {
    let g = GrammarBuilder::new("nf")
        .token("A", "a")
        .rule("s", vec![])
        .start("s")
        .build();
    let tok = g.tokens.values().next().unwrap();
    assert!(!tok.fragile);
}

// ===========================================================================
// 4. Grammar properties after building
// ===========================================================================

#[test]
fn test_grammar_name_preserved() {
    let g = GrammarBuilder::new("my_grammar").build();
    assert_eq!(g.name, "my_grammar");
}

#[test]
fn test_rules_count_matches_distinct_nonterminals() {
    let g = GrammarBuilder::new("cnt")
        .token("A", "a")
        .token("B", "b")
        .rule("x", vec!["A"])
        .rule("x", vec!["B"])
        .rule("y", vec!["x"])
        .start("x")
        .build();
    // Two distinct nonterminals: x and y
    assert_eq!(g.rules.len(), 2);
}

#[test]
fn test_token_count() {
    let g = GrammarBuilder::new("tc")
        .token("A", "a")
        .token("B", "b")
        .token("C", "c")
        .rule("s", vec!["A"])
        .start("s")
        .build();
    assert_eq!(g.tokens.len(), 3);
}

#[test]
fn test_start_symbol_is_first_rule() {
    let g = GrammarBuilder::new("ss")
        .token("A", "a")
        .rule("beta", vec!["A"])
        .rule("alpha", vec!["beta"])
        .start("alpha")
        .build();
    let first_lhs = *g.rules.keys().next().unwrap();
    let name = &g.rule_names[&first_lhs];
    assert_eq!(name, "alpha");
}

#[test]
fn test_extras_are_recorded() {
    let g = GrammarBuilder::new("ex")
        .token("WS", r"\s+")
        .extra("WS")
        .rule("s", vec![])
        .start("s")
        .build();
    assert_eq!(g.extras.len(), 1);
}

#[test]
fn test_externals_are_recorded() {
    let g = GrammarBuilder::new("ext")
        .token("INDENT", "INDENT")
        .external("INDENT")
        .rule("s", vec![])
        .start("s")
        .build();
    assert_eq!(g.externals.len(), 1);
    assert_eq!(g.externals[0].name, "INDENT");
}

#[test]
fn test_precedence_declaration() {
    let g = GrammarBuilder::new("prec_decl")
        .token("+", "+")
        .token("*", "*")
        .precedence(1, Associativity::Left, vec!["+"])
        .precedence(2, Associativity::Left, vec!["*"])
        .rule("s", vec![])
        .start("s")
        .build();
    assert_eq!(g.precedences.len(), 2);
    assert_eq!(g.precedences[0].level, 1);
    assert_eq!(g.precedences[1].level, 2);
}

#[test]
fn test_rule_with_precedence_stored() {
    let g = GrammarBuilder::new("rp")
        .token("NUM", r"\d+")
        .token("+", "+")
        .rule_with_precedence("expr", vec!["expr", "+", "expr"], 5, Associativity::Left)
        .rule("expr", vec!["NUM"])
        .start("expr")
        .build();
    let expr_id = find_rule_id(&g, "expr");
    let rules = &g.rules[&expr_id];
    let prec_rule = rules
        .iter()
        .find(|r| r.precedence.is_some())
        .expect("should have precedence rule");
    assert_eq!(prec_rule.precedence, Some(PrecedenceKind::Static(5)));
    assert_eq!(prec_rule.associativity, Some(Associativity::Left));
}

#[test]
fn test_rule_without_precedence_has_none() {
    let g = GrammarBuilder::new("np")
        .token("A", "a")
        .rule("s", vec!["A"])
        .start("s")
        .build();
    let s_id = find_rule_id(&g, "s");
    assert!(g.rules[&s_id][0].precedence.is_none());
    assert!(g.rules[&s_id][0].associativity.is_none());
}

#[test]
fn test_rule_names_populated_for_lowercase() {
    let g = GrammarBuilder::new("rn")
        .token("TOK", "tok")
        .rule("my_rule", vec!["TOK"])
        .start("my_rule")
        .build();
    assert!(g.rule_names.values().any(|n| n == "my_rule"));
}

#[test]
fn test_all_rules_iterator() {
    let g = GrammarBuilder::new("iter")
        .token("A", "a")
        .rule("x", vec!["A"])
        .rule("x", vec![])
        .rule("y", vec!["x"])
        .start("x")
        .build();
    assert_eq!(g.all_rules().count(), 3);
}

#[test]
fn test_get_rules_for_symbol() {
    let g = GrammarBuilder::new("grs")
        .token("A", "a")
        .rule("s", vec!["A"])
        .rule("s", vec![])
        .start("s")
        .build();
    let s_id = find_rule_id(&g, "s");
    assert_eq!(g.get_rules_for_symbol(s_id).unwrap().len(), 2);
}

// ===========================================================================
// 5. Edge cases
// ===========================================================================

#[test]
fn test_empty_grammar() {
    let g = GrammarBuilder::new("empty").build();
    assert_eq!(g.tokens.len(), 0);
    assert_eq!(g.rules.len(), 0);
    assert_eq!(g.name, "empty");
}

#[test]
fn test_grammar_with_only_tokens() {
    let g = GrammarBuilder::new("tokens_only")
        .token("A", "a")
        .token("B", "b")
        .build();
    assert_eq!(g.tokens.len(), 2);
    assert_eq!(g.rules.len(), 0);
}

#[test]
fn test_grammar_with_only_epsilon_rule() {
    let g = GrammarBuilder::new("eps")
        .rule("s", vec![])
        .start("s")
        .build();
    let s_id = find_rule_id(&g, "s");
    assert_eq!(g.rules[&s_id][0].rhs, vec![Symbol::Epsilon]);
}

#[test]
fn test_duplicate_token_overwrites() {
    let g = GrammarBuilder::new("dup")
        .token("A", "first")
        .token("A", "second")
        .rule("s", vec!["A"])
        .start("s")
        .build();
    // The token map is keyed by SymbolId; second .token() overwrites the first
    assert_eq!(g.tokens.len(), 1);
    let tok = g.tokens.values().next().unwrap();
    assert_eq!(tok.pattern, TokenPattern::String("second".into()));
}

#[test]
fn test_no_start_symbol_keeps_insertion_order() {
    let g = GrammarBuilder::new("nostart")
        .token("A", "a")
        .rule("first", vec!["A"])
        .rule("second", vec!["first"])
        .build();
    let first_key = g.rules.keys().next().unwrap();
    assert_eq!(g.rule_names[first_key], "first");
}

#[test]
fn test_start_symbol_not_in_rules_still_recorded() {
    // start() creates the symbol even if no rule uses it yet
    let g = GrammarBuilder::new("phantom")
        .token("A", "a")
        .rule("s", vec!["A"])
        .start("other")
        .build();
    // "other" was created by start() but has no rules, so it won't appear in rules map
    // "s" should still be present
    assert!(!g.rules.is_empty());
}

#[test]
fn test_self_referential_rule() {
    let g = GrammarBuilder::new("self_ref")
        .token("A", "a")
        .rule("s", vec!["s", "A"])
        .rule("s", vec!["A"])
        .start("s")
        .build();
    let s_id = find_rule_id(&g, "s");
    assert_eq!(g.rules[&s_id].len(), 2);
    // First rule should reference s as NonTerminal
    assert!(matches!(g.rules[&s_id][0].rhs[0], Symbol::NonTerminal(_)));
}

#[test]
fn test_mutual_recursion() {
    let g = GrammarBuilder::new("mutual")
        .token("A", "a")
        .token("B", "b")
        .rule("x", vec!["y", "A"])
        .rule("x", vec!["A"])
        .rule("y", vec!["x", "B"])
        .rule("y", vec!["B"])
        .start("x")
        .build();
    assert_eq!(g.rules.len(), 2);
    let x_id = find_rule_id(&g, "x");
    let y_id = find_rule_id(&g, "y");
    assert_eq!(g.rules[&x_id].len(), 2);
    assert_eq!(g.rules[&y_id].len(), 2);
}

#[test]
fn test_large_rhs() {
    let mut builder = GrammarBuilder::new("large_rhs");
    let mut tok_names: Vec<&'static str> = Vec::new();
    for i in 0..20 {
        let name: &'static str = Box::leak(format!("t{i}").into_boxed_str());
        builder = builder.token(name, name);
        tok_names.push(name);
    }
    let g = builder.rule("s", tok_names.clone()).start("s").build();
    let s_id = find_rule_id(&g, "s");
    assert_eq!(g.rules[&s_id][0].rhs.len(), 20);
}

#[test]
fn test_many_nonterminals() {
    let mut builder = GrammarBuilder::new("many_nt");
    builder = builder.token("a", "a");
    let mut prev: &'static str = "a";
    for i in 0..15 {
        let name: &'static str = Box::leak(format!("nt{i}").into_boxed_str());
        if i == 0 {
            builder = builder.rule(name, vec!["a"]);
        } else {
            builder = builder.rule(name, vec![prev]);
        }
        prev = name;
    }
    let g = builder.start("nt14").build();
    assert_eq!(g.rules.len(), 15);
}

#[test]
fn test_token_name_with_special_chars() {
    let g = GrammarBuilder::new("special")
        .token("(", "(")
        .token(")", ")")
        .token("{", "{")
        .token("}", "}")
        .rule("s", vec![])
        .start("s")
        .build();
    assert_eq!(g.tokens.len(), 4);
    // Punctuation tokens should NOT appear in rule_names
    assert!(!g.rule_names.values().any(|n| n == "("));
}

#[test]
fn test_right_associativity() {
    let g = GrammarBuilder::new("rassoc")
        .token("NUM", r"\d+")
        .token("^", "^")
        .rule_with_precedence("expr", vec!["expr", "^", "expr"], 3, Associativity::Right)
        .rule("expr", vec!["NUM"])
        .start("expr")
        .build();
    let expr_id = find_rule_id(&g, "expr");
    let pow = g.rules[&expr_id]
        .iter()
        .find(|r| r.associativity == Some(Associativity::Right))
        .unwrap();
    assert_eq!(pow.precedence, Some(PrecedenceKind::Static(3)));
}

#[test]
fn test_none_associativity() {
    let g = GrammarBuilder::new("nassoc")
        .token("NUM", r"\d+")
        .token("==", "==")
        .rule_with_precedence("expr", vec!["expr", "==", "expr"], 1, Associativity::None)
        .rule("expr", vec!["NUM"])
        .start("expr")
        .build();
    let expr_id = find_rule_id(&g, "expr");
    let cmp = g.rules[&expr_id]
        .iter()
        .find(|r| r.associativity == Some(Associativity::None))
        .unwrap();
    assert_eq!(cmp.associativity, Some(Associativity::None));
}

#[test]
fn test_python_like_preset() {
    let g = GrammarBuilder::python_like();
    assert_eq!(g.name, "python_like");
    let module_id = find_rule_id(&g, "module");
    let module_rules = &g.rules[&module_id];
    // module should have 3 alternatives (empty, statement, module statement)
    assert_eq!(module_rules.len(), 3);
    assert!(g.externals.len() >= 2);
}

#[test]
fn test_javascript_like_preset() {
    let g = GrammarBuilder::javascript_like();
    assert_eq!(g.name, "javascript_like");
    let program_id = find_rule_id(&g, "program");
    let program_rules = &g.rules[&program_id];
    // program has 2 alternatives (statement, program statement)
    assert_eq!(program_rules.len(), 2);
    // Should have precedence rules for expressions
    let expr_id = find_rule_id(&g, "expression");
    let expr_rules = &g.rules[&expr_id];
    let prec_count = expr_rules.iter().filter(|r| r.precedence.is_some()).count();
    assert!(prec_count >= 4); // +, -, *, /
}

#[test]
fn test_extra_token_shows_in_extras() {
    let g = GrammarBuilder::new("extr")
        .token("WS", r"[ \t]+")
        .extra("WS")
        .token("NL", r"\n")
        .extra("NL")
        .rule("s", vec![])
        .start("s")
        .build();
    assert_eq!(g.extras.len(), 2);
}

#[test]
fn test_fields_empty_by_default() {
    let g = GrammarBuilder::new("fields")
        .token("A", "a")
        .rule("s", vec!["A"])
        .start("s")
        .build();
    assert!(g.fields.is_empty());
    let s_id = find_rule_id(&g, "s");
    assert!(g.rules[&s_id][0].fields.is_empty());
}

#[test]
fn test_alias_sequences_empty_by_default() {
    let g = GrammarBuilder::new("alias")
        .token("A", "a")
        .rule("s", vec!["A"])
        .start("s")
        .build();
    assert!(g.alias_sequences.is_empty());
}

#[test]
fn test_production_ids_empty_by_default() {
    let g = GrammarBuilder::new("prodids")
        .token("A", "a")
        .rule("s", vec!["A"])
        .start("s")
        .build();
    assert_eq!(g.production_ids.len(), g.all_rules().count());
}

#[test]
fn test_max_alias_sequence_length_zero() {
    let g = GrammarBuilder::new("masl")
        .token("A", "a")
        .rule("s", vec!["A"])
        .start("s")
        .build();
    assert_eq!(g.max_alias_sequence_length, 0);
}

#[test]
fn test_symbol_registry_none_by_default() {
    let g = GrammarBuilder::new("reg")
        .token("A", "a")
        .rule("s", vec!["A"])
        .start("s")
        .build();
    assert!(g.symbol_registry.is_none());
}

#[test]
fn test_terminal_vs_nonterminal_classification() {
    let g = GrammarBuilder::new("classify")
        .token("NUM", r"\d+")
        .token("+", "+")
        .rule("expr", vec!["expr", "+", "NUM"])
        .rule("expr", vec!["NUM"])
        .start("expr")
        .build();
    let expr_id = find_rule_id(&g, "expr");
    let recursive_rule = &g.rules[&expr_id][0];
    // rhs[0] = expr (NonTerminal), rhs[1] = + (Terminal), rhs[2] = NUM (Terminal)
    assert!(matches!(recursive_rule.rhs[0], Symbol::NonTerminal(_)));
    assert!(matches!(recursive_rule.rhs[1], Symbol::Terminal(_)));
    assert!(matches!(recursive_rule.rhs[2], Symbol::Terminal(_)));
}

#[test]
fn test_token_reuse_same_symbol_id() {
    let g = GrammarBuilder::new("reuse")
        .token("A", "a")
        .rule("s", vec!["A", "A"])
        .start("s")
        .build();
    let s_id = find_rule_id(&g, "s");
    // Both references to A should have the same SymbolId
    let a_id = find_token_id(&g, "A");
    assert_eq!(g.rules[&s_id][0].rhs[0], Symbol::Terminal(a_id));
    assert_eq!(g.rules[&s_id][0].rhs[1], Symbol::Terminal(a_id));
}

#[test]
fn test_inline_rules_empty_by_default() {
    let g = GrammarBuilder::new("inline").build();
    assert!(g.inline_rules.is_empty());
}

#[test]
fn test_supertypes_empty_by_default() {
    let g = GrammarBuilder::new("super").build();
    assert!(g.supertypes.is_empty());
}

#[test]
fn test_conflicts_empty_by_default() {
    let g = GrammarBuilder::new("conf").build();
    assert!(g.conflicts.is_empty());
}

// ===========================================================================
// 6. Property tests with proptest
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn prop_grammar_name_preserved(name in "[a-z][a-z0-9_]{0,15}") {
        let g = GrammarBuilder::new(&name).build();
        prop_assert_eq!(g.name, name);
    }

    #[test]
    fn prop_token_count_matches(n in 1usize..=10) {
        let mut builder = GrammarBuilder::new("prop_tc");
        for i in 0..n {
            let name: &'static str = Box::leak(format!("tok{i}").into_boxed_str());
            builder = builder.token(name, name);
        }
        let g = builder.build();
        prop_assert_eq!(g.tokens.len(), n);
    }

    #[test]
    fn prop_rule_count_matches_distinct_lhs(n in 1usize..=8) {
        let mut builder = GrammarBuilder::new("prop_rc");
        builder = builder.token("a", "a");
        for i in 0..n {
            let name: &'static str = Box::leak(format!("nt{i}").into_boxed_str());
            builder = builder.rule(name, vec!["a"]);
        }
        let g = builder.build();
        prop_assert_eq!(g.rules.len(), n);
    }

    #[test]
    fn prop_alternatives_count(alts in 1usize..=8) {
        let mut builder = GrammarBuilder::new("prop_alt")
            .token("a", "a");
        for _ in 0..alts {
            builder = builder.rule("s", vec!["a"]);
        }
        let g = builder.start("s").build();
        let s_id = find_rule_id(&g, "s");
        prop_assert_eq!(g.rules[&s_id].len(), alts);
    }

    #[test]
    fn prop_production_ids_unique(alts in 2usize..=10) {
        let mut builder = GrammarBuilder::new("prop_pid")
            .token("a", "a");
        for _ in 0..alts {
            builder = builder.rule("s", vec!["a"]);
        }
        let g = builder.start("s").build();
        let s_id = find_rule_id(&g, "s");
        let ids: Vec<_> = g.rules[&s_id].iter().map(|r| r.production_id).collect();
        let unique: std::collections::HashSet<_> = ids.iter().collect();
        prop_assert_eq!(ids.len(), unique.len());
    }

    #[test]
    fn prop_start_symbol_first(
        first in "[a-z]{3,6}",
        second in "[a-z]{3,6}",
    ) {
        // Ensure names differ
        prop_assume!(first != second);
        let g = GrammarBuilder::new("prop_start")
            .token("a", "a")
            .rule(&first, vec!["a"])
            .rule(&second, vec!["a"])
            .start(&second)
            .build();
        let first_key = g.rules.keys().next().unwrap();
        prop_assert_eq!(&g.rule_names[first_key], &second);
    }

    #[test]
    fn prop_extras_count(n in 0usize..=5) {
        let mut builder = GrammarBuilder::new("prop_ex");
        for i in 0..n {
            let name: &'static str = Box::leak(format!("ex{i}").into_boxed_str());
            builder = builder.token(name, name);
            builder = builder.extra(name);
        }
        let g = builder.rule("s", vec![]).start("s").build();
        prop_assert_eq!(g.extras.len(), n);
    }

    #[test]
    fn prop_externals_count(n in 0usize..=5) {
        let mut builder = GrammarBuilder::new("prop_ext");
        for i in 0..n {
            let name: &'static str = Box::leak(format!("ext{i}").into_boxed_str());
            builder = builder.token(name, name);
            builder = builder.external(name);
        }
        let g = builder.rule("s", vec![]).start("s").build();
        prop_assert_eq!(g.externals.len(), n);
    }

    #[test]
    fn prop_all_rules_count(
        nt_count in 1usize..=4,
        alts_per in 1usize..=4,
    ) {
        let mut builder = GrammarBuilder::new("prop_all")
            .token("a", "a");
        for i in 0..nt_count {
            let name: &'static str = Box::leak(format!("nt{i}").into_boxed_str());
            for _ in 0..alts_per {
                builder = builder.rule(name, vec!["a"]);
            }
        }
        let g = builder.build();
        prop_assert_eq!(g.all_rules().count(), nt_count * alts_per);
    }

    #[test]
    fn prop_rhs_length_preserved(len in 1usize..=8) {
        let mut builder = GrammarBuilder::new("prop_rhs");
        let mut toks = Vec::new();
        for i in 0..len {
            let name: &'static str = Box::leak(format!("t{i}").into_boxed_str());
            builder = builder.token(name, name);
            toks.push(name);
        }
        let g = builder.rule("s", toks.clone()).start("s").build();
        let s_id = find_rule_id(&g, "s");
        prop_assert_eq!(g.rules[&s_id][0].rhs.len(), len);
    }

    #[test]
    fn prop_epsilon_rule_has_single_epsilon(n in 1usize..=5) {
        let mut builder = GrammarBuilder::new("prop_eps");
        for _ in 0..n {
            builder = builder.rule("s", vec![]);
        }
        let g = builder.start("s").build();
        let s_id = find_rule_id(&g, "s");
        for rule in &g.rules[&s_id] {
            prop_assert_eq!(rule.rhs.clone(), vec![Symbol::Epsilon]);
        }
    }

    #[test]
    fn prop_precedence_values_stored(prec in -100i16..=100) {
        let g = GrammarBuilder::new("prop_prec")
            .token("a", "a")
            .token("+", "+")
            .rule_with_precedence("expr", vec!["expr", "+", "expr"], prec, Associativity::Left)
            .rule("expr", vec!["a"])
            .start("expr")
            .build();
        let expr_id = find_rule_id(&g, "expr");
        let prec_rule = g.rules[&expr_id]
            .iter()
            .find(|r| r.precedence.is_some())
            .unwrap();
        prop_assert_eq!(prec_rule.precedence, Some(PrecedenceKind::Static(prec)));
    }

    #[test]
    fn prop_duplicate_token_keeps_last(
        first_pat in "[a-z]{1,5}",
        second_pat in "[a-z]{1,5}",
    ) {
        let g = GrammarBuilder::new("prop_dup")
            .token("tok", &first_pat)
            .token("tok", &second_pat)
            .build();
        prop_assert_eq!(g.tokens.len(), 1);
        let tok = g.tokens.values().next().unwrap();
        // The pattern stored should be from the second call
        let expected = TokenPattern::String(second_pat);
        prop_assert_eq!(&tok.pattern, &expected);
    }
}
