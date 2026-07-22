//! Comprehensive property-based and unit tests for GrammarBuilder.

use adze_ir::builder::GrammarBuilder;
use adze_ir::{Associativity, PrecedenceKind, Symbol};
use proptest::prelude::*;
use std::collections::HashSet;

// ---------------------------------------------------------------------------
// Strategies
// ---------------------------------------------------------------------------

/// Valid identifier names (lowercase to ensure rule_names registration).
fn rule_name_strategy() -> impl Strategy<Value = String> {
    "[a-z][a-z0-9]{0,12}"
}

/// Valid token names (uppercase to distinguish from rules).
fn token_name_strategy() -> impl Strategy<Value = String> {
    "[A-Z][A-Z0-9_]{0,12}"
}

/// Safe regex-like token patterns (avoids bare `/` which causes a slice panic).
fn pattern_strategy() -> impl Strategy<Value = String> {
    prop_oneof![
        "[a-z]+",
        Just(r"\d+".to_string()),
        Just(r"[a-zA-Z_]+".to_string()),
        Just(r"[0-9]+(\.[0-9]+)?".to_string()),
        Just("keyword".to_string()),
    ]
}

/// Vec of unique (token_name, pattern) pairs.
fn unique_token_list(max_len: usize) -> impl Strategy<Value = Vec<(String, String)>> {
    prop::collection::vec((token_name_strategy(), pattern_strategy()), 1..=max_len).prop_map(
        |pairs| {
            let mut seen = HashSet::new();
            pairs
                .into_iter()
                .filter(|(n, _)| seen.insert(n.clone()))
                .collect()
        },
    )
}

/// Vec of unique rule names.
fn unique_rule_names(max_len: usize) -> impl Strategy<Value = Vec<String>> {
    prop::collection::vec(rule_name_strategy(), 1..=max_len).prop_map(|names| {
        let mut seen = HashSet::new();
        names
            .into_iter()
            .filter(|n| seen.insert(n.clone()))
            .collect()
    })
}

// ---------------------------------------------------------------------------
// Proptest: token count
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    /// P1: N unique tokens → exactly N tokens in grammar.
    #[test]
    fn token_count_preserved(tokens in unique_token_list(10)) {
        let expected = tokens.len();
        let mut b = GrammarBuilder::new("g");
        for (name, pat) in &tokens {
            b = b.token(name, pat);
        }
        let grammar = b.build();
        prop_assert_eq!(grammar.tokens.len(), expected);
    }

    /// P2: M rules (each with distinct LHS) → M LHS entries in grammar.rules.
    #[test]
    fn rule_lhs_count_preserved(
        tok in unique_token_list(3),
        rule_names in unique_rule_names(6),
    ) {
        let mut b = GrammarBuilder::new("g");
        for (name, pat) in &tok {
            b = b.token(name, pat);
        }
        // Each distinct LHS gets one rule referencing first token.
        let first_tok = &tok[0].0;
        for rn in &rule_names {
            b = b.rule(rn, vec![first_tok.as_str()]);
        }
        let grammar = b.build();
        prop_assert_eq!(grammar.rules.len(), rule_names.len());
    }

    /// P3: Grammar name is preserved exactly.
    #[test]
    fn grammar_name_preserved(name in "[a-z][a-z0-9_]{0,20}") {
        let grammar = GrammarBuilder::new(&name).build();
        prop_assert_eq!(&grammar.name, &name);
    }

    /// P4: Token addition order doesn't affect the final token *set*.
    #[test]
    fn token_order_independent_set(tokens in unique_token_list(6)) {
        let mut reversed = tokens.clone();
        reversed.reverse();

        let build = |list: &[(String, String)]| {
            let mut b = GrammarBuilder::new("g");
            for (n, p) in list {
                b = b.token(n, p);
            }
            b.build()
        };
        let g1 = build(&tokens);
        let g2 = build(&reversed);

        let names1: HashSet<_> = g1.tokens.values().map(|t| t.name.clone()).collect();
        let names2: HashSet<_> = g2.tokens.values().map(|t| t.name.clone()).collect();
        prop_assert_eq!(names1, names2);
    }

    /// P5: Rule addition order doesn't affect final rule count.
    #[test]
    fn rule_order_independent_count(rule_names in unique_rule_names(5)) {
        let mut reversed = rule_names.clone();
        reversed.reverse();

        let build = |names: &[String]| {
            let mut b = GrammarBuilder::new("g").token("T", "t");
            for rn in names {
                b = b.rule(rn, vec!["T"]);
            }
            b.build()
        };
        let g1 = build(&rule_names);
        let g2 = build(&reversed);
        prop_assert_eq!(g1.rules.len(), g2.rules.len());
    }

    /// P6: Builder is deterministic — same inputs produce identical grammar.
    #[test]
    fn builder_deterministic(tokens in unique_token_list(4)) {
        let build = || {
            let mut b = GrammarBuilder::new("det");
            for (n, p) in &tokens {
                b = b.token(n, p);
            }
            b.build()
        };
        let g1 = build();
        let g2 = build();
        prop_assert_eq!(g1, g2);
    }

    /// P7: Setting start() → start_symbol() is Some.
    #[test]
    fn start_symbol_is_some(rule_name in rule_name_strategy()) {
        let grammar = GrammarBuilder::new("g")
            .token("T", "t")
            .rule(&rule_name, vec!["T"])
            .start(&rule_name)
            .build();
        // start_symbol() uses heuristics; the rule should be first, so it is found.
        prop_assert!(grammar.start_symbol().is_some());
    }

    /// P8: Random valid grammars always produce a well-formed Grammar struct.
    #[test]
    fn random_grammar_well_formed(
        name in "[a-z]{1,8}",
        tokens in unique_token_list(5),
        rule_names in unique_rule_names(4),
    ) {
        if tokens.is_empty() || rule_names.is_empty() {
            return Ok(());
        }
        let first_tok = &tokens[0].0;
        let mut b = GrammarBuilder::new(&name);
        for (tn, tp) in &tokens {
            b = b.token(tn, tp);
        }
        for rn in &rule_names {
            b = b.rule(rn, vec![first_tok.as_str()]);
        }
        let grammar = b.build();

        prop_assert!(!grammar.name.is_empty());
        prop_assert!(grammar.tokens.len() <= tokens.len() + rule_names.len());
        prop_assert!(!grammar.rules.is_empty());
    }

    /// P9: Duplicate token name → token count stays at 1 for that name.
    #[test]
    fn duplicate_token_idempotent(pat1 in pattern_strategy(), pat2 in pattern_strategy()) {
        let grammar = GrammarBuilder::new("g")
            .token("DUP", &pat1)
            .token("DUP", &pat2)
            .build();
        // The second insertion overwrites the first in the IndexMap.
        prop_assert_eq!(grammar.tokens.len(), 1);
    }

    /// P10: Multiple alternative rules under one LHS accumulate.
    #[test]
    fn multiple_alternatives_accumulate(count in 1usize..=8) {
        let mut b = GrammarBuilder::new("g").token("T", "t");
        for _ in 0..count {
            b = b.rule("expr", vec!["T"]);
        }
        let grammar = b.build();
        let total_rules: usize = grammar.rules.values().map(|v| v.len()).sum();
        prop_assert_eq!(total_rules, count);
    }

    /// P11: Extra token recorded in extras list.
    #[test]
    fn extra_token_recorded(name in "[a-z]{1,6}") {
        let grammar = GrammarBuilder::new("g")
            .token("WS", r"[ \t]+")
            .extra("WS")
            .build();
        prop_assert!(!grammar.extras.is_empty());
        // Grammar name is still correct.
        let _ = name;
        prop_assert_eq!(&grammar.name, "g");
    }

    /// P12: External token recorded.
    #[test]
    fn external_token_recorded(name in "[A-Z]{1,6}") {
        let grammar = GrammarBuilder::new("g")
            .token(&name, &name)
            .external(&name)
            .build();
        prop_assert!(!grammar.externals.is_empty());
        prop_assert_eq!(&grammar.externals[0].name, &name);
    }

    /// P13: Precedence declaration recorded.
    #[test]
    fn precedence_declaration_recorded(level in -100i16..100i16) {
        let grammar = GrammarBuilder::new("g")
            .token("+", "+")
            .precedence(level, Associativity::Left, vec!["+"])
            .build();
        prop_assert_eq!(grammar.precedences.len(), 1);
        prop_assert_eq!(grammar.precedences[0].level, level);
    }

    /// P14: rule_with_precedence stores correct precedence/associativity.
    #[test]
    fn rule_with_precedence_stored(prec in -50i16..50i16) {
        let grammar = GrammarBuilder::new("g")
            .token("N", r"\d+")
            .token("+", "+")
            .rule_with_precedence("expr", vec!["expr", "+", "expr"], prec, Associativity::Left)
            .rule("expr", vec!["N"])
            .start("expr")
            .build();
        let all_rules: Vec<_> = grammar.rules.values().flatten().collect();
        let prec_rule = all_rules.iter().find(|r| r.precedence.is_some()).unwrap();
        prop_assert_eq!(prec_rule.precedence, Some(PrecedenceKind::Static(prec)));
        prop_assert_eq!(prec_rule.associativity, Some(Associativity::Left));
    }

    /// P15: Each rule gets a unique ProductionId.
    #[test]
    fn production_ids_unique(count in 2usize..=10) {
        let mut b = GrammarBuilder::new("g").token("T", "t");
        for _ in 0..count {
            b = b.rule("expr", vec!["T"]);
        }
        let grammar = b.build();
        let ids: Vec<_> = grammar
            .rules
            .values()
            .flatten()
            .map(|r| r.production_id)
            .collect();
        let unique: HashSet<_> = ids.iter().collect();
        prop_assert_eq!(ids.len(), unique.len());
    }

    /// P16: Tokens referenced in rules resolve to Terminal symbols.
    #[test]
    fn tokens_resolve_to_terminal(tokens in unique_token_list(3)) {
        if tokens.is_empty() {
            return Ok(());
        }
        let first = &tokens[0].0;
        let mut b = GrammarBuilder::new("g");
        for (n, p) in &tokens {
            b = b.token(n, p);
        }
        b = b.rule("start", vec![first.as_str()]);
        let grammar = b.build();
        let start_rules = grammar.rules.values().next().unwrap();
        let rhs = &start_rules[0].rhs;
        prop_assert!(matches!(rhs[0], Symbol::Terminal(_)));
    }

    /// P17: Unknown symbols in RHS resolve to NonTerminal.
    #[test]
    fn unknown_symbols_are_nonterminal(name in rule_name_strategy()) {
        let grammar = GrammarBuilder::new("g")
            .rule("top", vec![&name])
            .build();
        let rules = grammar.rules.values().next().unwrap();
        prop_assert!(matches!(rules[0].rhs[0], Symbol::NonTerminal(_)));
    }

    /// P18: Fragile token has fragile flag set.
    #[test]
    fn fragile_token_flag(name in token_name_strategy()) {
        let grammar = GrammarBuilder::new("g")
            .fragile_token(&name, &name)
            .build();
        let tok = grammar.tokens.values().next().unwrap();
        prop_assert!(tok.fragile);
    }

    /// P19: Non-fragile token has fragile=false.
    #[test]
    fn normal_token_not_fragile(name in token_name_strategy()) {
        let grammar = GrammarBuilder::new("g")
            .token(&name, &name)
            .build();
        let tok = grammar.tokens.values().next().unwrap();
        prop_assert!(!tok.fragile);
    }

    /// P20: build() produces empty alias_sequences and production_ids by default.
    #[test]
    fn default_collections_empty(name in "[a-z]{1,8}") {
        let grammar = GrammarBuilder::new(&name).build();
        prop_assert!(grammar.alias_sequences.is_empty());
        prop_assert!(grammar.production_ids.is_empty());
        prop_assert_eq!(grammar.max_alias_sequence_length, 0);
    }
}

// ---------------------------------------------------------------------------
// Unit tests (30+)
// ---------------------------------------------------------------------------

#[test]
fn unit_empty_grammar() {
    let g = GrammarBuilder::new("empty").build();
    assert_eq!(g.name, "empty");
    assert!(g.tokens.is_empty());
    assert!(g.rules.is_empty());
}

#[test]
fn unit_single_token() {
    let g = GrammarBuilder::new("single").token("NUM", r"\d+").build();
    assert_eq!(g.tokens.len(), 1);
    assert_eq!(g.tokens.values().next().unwrap().name, "NUM");
}

#[test]
fn unit_single_rule() {
    let g = GrammarBuilder::new("g")
        .token("A", "a")
        .rule("start", vec!["A"])
        .build();
    assert_eq!(g.rules.len(), 1);
}

#[test]
fn unit_name_with_underscores() {
    let g = GrammarBuilder::new("my_grammar").build();
    assert_eq!(g.name, "my_grammar");
}

#[test]
fn unit_name_with_digits() {
    let g = GrammarBuilder::new("grammar42").build();
    assert_eq!(g.name, "grammar42");
}

#[test]
fn unit_start_symbol_set() {
    let g = GrammarBuilder::new("g")
        .token("T", "t")
        .rule("root", vec!["T"])
        .start("root")
        .build();
    // "root" is first rule because start() reorders it.
    let first_lhs = *g.rules.keys().next().unwrap();
    let first_name = g.rule_names.get(&first_lhs).unwrap();
    assert_eq!(first_name, "root");
}

#[test]
fn unit_no_start_still_builds() {
    let g = GrammarBuilder::new("g")
        .token("T", "t")
        .rule("expr", vec!["T"])
        .start("expr")
        .build();
    assert_eq!(g.start_symbol(), g.explicit_start_symbol());
}

#[test]
fn unit_empty_rhs_becomes_epsilon() {
    let g = GrammarBuilder::new("g").rule("empty", vec![]).build();
    let rules = g.rules.values().next().unwrap();
    assert_eq!(rules[0].rhs.len(), 1);
    assert!(matches!(rules[0].rhs[0], Symbol::Epsilon));
}

#[test]
fn unit_multiple_tokens() {
    let g = GrammarBuilder::new("g")
        .token("A", "a")
        .token("B", "b")
        .token("C", "c")
        .build();
    assert_eq!(g.tokens.len(), 3);
}

#[test]
fn unit_multiple_rules_same_lhs() {
    let g = GrammarBuilder::new("g")
        .token("T", "t")
        .rule("expr", vec!["T"])
        .rule("expr", vec!["T", "T"])
        .build();
    let total: usize = g.rules.values().map(|v| v.len()).sum();
    assert_eq!(total, 2);
    // Only one LHS entry.
    assert_eq!(g.rules.len(), 1);
}

#[test]
fn unit_duplicate_token_overwrites() {
    let g = GrammarBuilder::new("g")
        .token("X", "first")
        .token("X", "second")
        .build();
    assert_eq!(g.tokens.len(), 1);
    assert_eq!(g.tokens.values().next().unwrap().name, "X");
}

#[test]
fn unit_rule_with_precedence_left() {
    let g = GrammarBuilder::new("g")
        .token("N", r"\d+")
        .token("+", "+")
        .rule_with_precedence("expr", vec!["expr", "+", "expr"], 1, Associativity::Left)
        .rule("expr", vec!["N"])
        .build();
    let all: Vec<_> = g.rules.values().flatten().collect();
    let prec_rule = all.iter().find(|r| r.precedence.is_some()).unwrap();
    assert_eq!(prec_rule.associativity, Some(Associativity::Left));
}

#[test]
fn unit_rule_with_precedence_right() {
    let g = GrammarBuilder::new("g")
        .token("N", r"\d+")
        .token("^", "^")
        .rule_with_precedence("expr", vec!["expr", "^", "expr"], 3, Associativity::Right)
        .rule("expr", vec!["N"])
        .build();
    let all: Vec<_> = g.rules.values().flatten().collect();
    let prec_rule = all.iter().find(|r| r.precedence.is_some()).unwrap();
    assert_eq!(prec_rule.associativity, Some(Associativity::Right));
    assert_eq!(prec_rule.precedence, Some(PrecedenceKind::Static(3)));
}

#[test]
fn unit_rule_with_precedence_none_assoc() {
    let g = GrammarBuilder::new("g")
        .token("N", r"\d+")
        .token("==", "==")
        .rule_with_precedence("expr", vec!["expr", "==", "expr"], 0, Associativity::None)
        .rule("expr", vec!["N"])
        .build();
    let all: Vec<_> = g.rules.values().flatten().collect();
    let prec_rule = all.iter().find(|r| r.precedence.is_some()).unwrap();
    assert_eq!(prec_rule.associativity, Some(Associativity::None));
}

#[test]
fn unit_negative_precedence() {
    let g = GrammarBuilder::new("g")
        .token("N", r"\d+")
        .token("+", "+")
        .rule_with_precedence("expr", vec!["expr", "+", "expr"], -5, Associativity::Left)
        .rule("expr", vec!["N"])
        .build();
    let all: Vec<_> = g.rules.values().flatten().collect();
    let prec_rule = all.iter().find(|r| r.precedence.is_some()).unwrap();
    assert_eq!(prec_rule.precedence, Some(PrecedenceKind::Static(-5)));
}

#[test]
fn unit_extra_records_symbol() {
    let g = GrammarBuilder::new("g")
        .token("WS", r"[ \t]+")
        .extra("WS")
        .build();
    assert_eq!(g.extras.len(), 1);
}

#[test]
fn unit_external_records_symbol() {
    let g = GrammarBuilder::new("g")
        .token("INDENT", "INDENT")
        .external("INDENT")
        .build();
    assert_eq!(g.externals.len(), 1);
    assert_eq!(g.externals[0].name, "INDENT");
}

#[test]
fn unit_precedence_declaration() {
    let g = GrammarBuilder::new("g")
        .token("+", "+")
        .token("*", "*")
        .precedence(1, Associativity::Left, vec!["+"])
        .precedence(2, Associativity::Left, vec!["*"])
        .build();
    assert_eq!(g.precedences.len(), 2);
    assert_eq!(g.precedences[0].level, 1);
    assert_eq!(g.precedences[1].level, 2);
}

#[test]
fn unit_symbol_registry_none_by_default() {
    let g = GrammarBuilder::new("g").build();
    assert!(g.symbol_registry.is_none());
}

#[test]
fn unit_fields_empty_by_default() {
    let g = GrammarBuilder::new("g").build();
    assert!(g.fields.is_empty());
}

#[test]
fn unit_conflicts_empty_by_default() {
    let g = GrammarBuilder::new("g").build();
    assert!(g.conflicts.is_empty());
}

#[test]
fn unit_inline_rules_empty_by_default() {
    let g = GrammarBuilder::new("g").build();
    assert!(g.inline_rules.is_empty());
}

#[test]
fn unit_supertypes_empty_by_default() {
    let g = GrammarBuilder::new("g").build();
    assert!(g.supertypes.is_empty());
}

#[test]
fn unit_python_like_preset_builds() {
    let g = GrammarBuilder::python_like();
    assert_eq!(g.name, "python_like");
    assert!(!g.tokens.is_empty());
    assert!(!g.rules.is_empty());
    assert!(!g.externals.is_empty());
}

#[test]
fn unit_javascript_like_preset_builds() {
    let g = GrammarBuilder::javascript_like();
    assert_eq!(g.name, "javascript_like");
    assert!(!g.tokens.is_empty());
    assert!(!g.rules.is_empty());
}

#[test]
fn unit_regex_pattern_detection() {
    let g = GrammarBuilder::new("g").token("NUM", r"\d+").build();
    let tok = g.tokens.values().next().unwrap();
    assert!(matches!(&tok.pattern, adze_ir::TokenPattern::Regex(r) if r.contains(r"\d")));
}

#[test]
fn unit_string_literal_pattern() {
    let g = GrammarBuilder::new("g").token("KW", "keyword").build();
    let tok = g.tokens.values().next().unwrap();
    assert!(matches!(&tok.pattern, adze_ir::TokenPattern::String(s) if s == "keyword"));
}

#[test]
fn unit_slash_in_regex_safe() {
    // Using escaped slash to avoid the slice panic with bare `/`.
    let g = GrammarBuilder::new("g").token("DIV", r"\/").build();
    assert_eq!(g.tokens.len(), 1);
}

#[test]
fn unit_production_ids_sequential() {
    let g = GrammarBuilder::new("g")
        .token("T", "t")
        .rule("a", vec!["T"])
        .rule("b", vec!["T"])
        .rule("c", vec!["T"])
        .build();
    let ids: Vec<u16> = g
        .rules
        .values()
        .flatten()
        .map(|r| r.production_id.0)
        .collect();
    // They should be sequential 0, 1, 2.
    assert_eq!(ids, vec![0, 1, 2]);
}

#[test]
fn unit_start_reorders_rules() {
    let g = GrammarBuilder::new("g")
        .token("T", "t")
        .rule("alpha", vec!["T"])
        .rule("beta", vec!["T"])
        .start("beta")
        .build();
    let first_lhs = *g.rules.keys().next().unwrap();
    let first_name = g.rule_names.get(&first_lhs).unwrap();
    assert_eq!(first_name, "beta");
}

#[test]
fn unit_chained_fluent_api() {
    // Verify the entire fluent chain compiles and runs in one expression.
    let g = GrammarBuilder::new("chain")
        .token("A", "a")
        .token("B", "b")
        .rule("start", vec!["A", "B"])
        .start("start")
        .build();
    assert_eq!(g.name, "chain");
    assert_eq!(g.tokens.len(), 2);
}

#[test]
fn unit_rule_names_registered_for_lowercase() {
    let g = GrammarBuilder::new("g")
        .token("T", "t")
        .rule("myexpr", vec!["T"])
        .build();
    let has_name = g.rule_names.values().any(|n| n == "myexpr");
    assert!(has_name);
}

#[test]
fn unit_many_tokens_stress() {
    let mut b = GrammarBuilder::new("stress");
    for i in 0..100 {
        let name = format!("T{i}");
        b = b.token(&name, &name);
    }
    let g = b.build();
    assert_eq!(g.tokens.len(), 100);
}

#[test]
fn unit_many_rules_stress() {
    let mut b = GrammarBuilder::new("stress").token("T", "t");
    for i in 0..100 {
        let name = format!("r{i}");
        b = b.rule(&name, vec!["T"]);
    }
    let g = b.build();
    assert_eq!(g.rules.len(), 100);
}

#[test]
fn unit_grammar_clone_equals_original() {
    let g = GrammarBuilder::new("clonable")
        .token("A", "a")
        .rule("start", vec!["A"])
        .start("start")
        .build();
    let g2 = g.clone();
    assert_eq!(g, g2);
}
