//! Property-based tests (v5) for `GrammarBuilder` — 45+ properties across 9 categories.

use adze_ir::builder::GrammarBuilder;
use adze_ir::{Grammar, Symbol};
use proptest::prelude::*;

// ---------------------------------------------------------------------------
// Strategies
// ---------------------------------------------------------------------------

/// Alpha-only names 1..10 chars, lowercase prefix to land in `rule_names`.
fn alpha_name() -> impl Strategy<Value = String> {
    "[a-z][a-zA-Z]{0,8}"
}

/// Grammar-level name (uppercase start).
fn grammar_name() -> impl Strategy<Value = String> {
    "[A-Z][a-zA-Z]{0,8}"
}

/// Regex-safe token patterns (no special regex chars).
fn safe_pattern() -> impl Strategy<Value = String> {
    prop_oneof![
        "[a-z]{1,6}",
        Just("keyword".to_string()),
        Just("literal".to_string()),
        Just("value".to_string()),
    ]
}

/// Vec of `(name, pattern)` with **unique** names, length in `1..=max_len`.
fn unique_tokens(max_len: usize) -> impl Strategy<Value = Vec<(String, String)>> {
    prop::collection::vec((alpha_name(), safe_pattern()), 1..=max_len).prop_map(|pairs| {
        let mut seen = std::collections::HashSet::new();
        pairs
            .into_iter()
            .filter(|(n, _)| seen.insert(n.clone()))
            .collect()
    })
}

/// Exactly `n` unique token pairs (retried until we get `n` distinct names).
#[allow(dead_code)]
fn exact_tokens(n: usize) -> impl Strategy<Value = Vec<(String, String)>> {
    prop::collection::vec((alpha_name(), safe_pattern()), n..=n + 10).prop_map(move |pairs| {
        let mut seen = std::collections::HashSet::new();
        pairs
            .into_iter()
            .filter(|(name, _)| seen.insert(name.clone()))
            .take(n)
            .collect::<Vec<_>>()
    })
}

/// Build a grammar from token pairs, an optional rule, and an optional start.
fn build_grammar(
    gname: &str,
    tokens: &[(String, String)],
    rule_name: Option<&str>,
    start: Option<&str>,
) -> Grammar {
    let mut b = GrammarBuilder::new(gname);
    for (tname, pat) in tokens {
        b = b.token(tname, pat);
    }
    if let Some(lhs) = rule_name {
        let rhs: Vec<&str> = tokens.iter().map(|(n, _)| n.as_str()).collect();
        b = b.rule(lhs, rhs);
    }
    if let Some(s) = start {
        b = b.start(s);
    }
    b.build()
}

// ===========================================================================
// Category 1 — Builder always succeeds with valid inputs (5 properties)
// ===========================================================================
proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn cat1_build_empty_grammar(gname in grammar_name()) {
        let g = GrammarBuilder::new(&gname).build();
        prop_assert_eq!(&g.name, &gname);
    }

    #[test]
    fn cat1_build_tokens_only(tokens in unique_tokens(6)) {
        let mut b = GrammarBuilder::new("TokOnly");
        for (n, p) in &tokens {
            b = b.token(n, p);
        }
        let _g = b.build(); // must not panic
    }

    #[test]
    fn cat1_build_tokens_and_rule(tokens in unique_tokens(4)) {
        if tokens.is_empty() { return Ok(()); }
        let rhs: Vec<&str> = tokens.iter().map(|(n, _)| n.as_str()).collect();
        let g = GrammarBuilder::new("WithRule")
            .token(&tokens[0].0, &tokens[0].1)
            .rule("root", rhs)
            .start("root")
            .build();
        prop_assert!(!g.rules.is_empty());
    }

    #[test]
    fn cat1_build_multiple_rules(n in 1usize..=6) {
        let mut b = GrammarBuilder::new("Multi");
        b = b.token("x", "x");
        for i in 0..n {
            b = b.rule(&format!("r{i}"), vec!["x"]);
        }
        let _g = b.build();
    }

    #[test]
    fn cat1_build_with_start(start in alpha_name()) {
        let g = GrammarBuilder::new("StartTest")
            .token("a", "a")
            .rule(&start, vec!["a"])
            .start(&start)
            .build();
        prop_assert!(!g.rules.is_empty());
    }
}

// ===========================================================================
// Category 2 — Built grammar has expected token count (5 properties)
// ===========================================================================
proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn cat2_token_count_matches(tokens in unique_tokens(8)) {
        let mut b = GrammarBuilder::new("Cnt");
        for (n, p) in &tokens {
            b = b.token(n, p);
        }
        let g = b.build();
        prop_assert_eq!(g.tokens.len(), tokens.len());
    }

    #[test]
    fn cat2_single_token_count(_dummy in 0..1i32) {
        let g = GrammarBuilder::new("One").token("tok", "t").build();
        prop_assert_eq!(g.tokens.len(), 1);
    }

    #[test]
    fn cat2_duplicate_token_count(
        tname in alpha_name(),
        p1 in safe_pattern(),
        p2 in safe_pattern(),
    ) {
        let g = GrammarBuilder::new("Dup")
            .token(&tname, &p1)
            .token(&tname, &p2)
            .build();
        prop_assert_eq!(g.tokens.len(), 1);
    }

    #[test]
    fn cat2_many_tokens_count(n in 1usize..=20) {
        let mut b = GrammarBuilder::new("Many");
        for i in 0..n {
            b = b.token(&format!("t{i}"), &format!("p{i}"));
        }
        let g = b.build();
        prop_assert_eq!(g.tokens.len(), n);
    }

    #[test]
    fn cat2_tokens_independent_of_rules(tokens in unique_tokens(5)) {
        let len = tokens.len();
        let mut b = GrammarBuilder::new("Ind");
        for (n, p) in &tokens {
            b = b.token(n, p);
        }
        b = b.rule("r", vec![tokens[0].0.as_str()]);
        let g = b.build();
        prop_assert_eq!(g.tokens.len(), len);
    }
}

// ===========================================================================
// Category 3 — Built grammar has expected rule count (5 properties)
// ===========================================================================
proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn cat3_single_rule_count(_dummy in 0..1i32) {
        let g = GrammarBuilder::new("R")
            .token("a", "a")
            .rule("r", vec!["a"])
            .build();
        let total: usize = g.rules.values().map(|v| v.len()).sum();
        prop_assert_eq!(total, 1);
    }

    #[test]
    fn cat3_n_distinct_rules(n in 1usize..=8) {
        let mut b = GrammarBuilder::new("Dist");
        b = b.token("x", "x");
        for i in 0..n {
            b = b.rule(&format!("r{i}"), vec!["x"]);
        }
        let g = b.build();
        prop_assert_eq!(g.rules.len(), n);
    }

    #[test]
    fn cat3_alternatives_accumulate(alts in 2usize..=6) {
        let mut b = GrammarBuilder::new("Alt");
        for i in 0..alts {
            b = b.token(&format!("t{i}"), &format!("v{i}"));
        }
        for i in 0..alts {
            b = b.rule("expr", vec![&format!("t{i}")]);
        }
        let g = b.build();
        let expr_id = g.rule_names.iter()
            .find(|(_, name)| name.as_str() == "expr")
            .map(|(id, _)| *id)
            .unwrap();
        prop_assert_eq!(g.rules[&expr_id].len(), alts);
    }

    #[test]
    fn cat3_total_rule_count(
        num_lhs in 1usize..=4,
        alts in 1usize..=3,
    ) {
        let mut b = GrammarBuilder::new("TRC");
        b = b.token("x", "x");
        for i in 0..num_lhs {
            for _ in 0..alts {
                b = b.rule(&format!("r{i}"), vec!["x"]);
            }
        }
        let g = b.build();
        let total: usize = g.rules.values().map(|v| v.len()).sum();
        prop_assert_eq!(total, num_lhs * alts);
    }

    #[test]
    fn cat3_no_rules_when_none_added(gname in grammar_name()) {
        let g = GrammarBuilder::new(&gname).token("a", "a").build();
        prop_assert!(g.rules.is_empty());
    }
}

// ===========================================================================
// Category 4 — Name is preserved (5 properties)
// ===========================================================================
proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn cat4_grammar_name_exact(name in grammar_name()) {
        let g = GrammarBuilder::new(&name).build();
        prop_assert_eq!(&g.name, &name);
    }

    #[test]
    fn cat4_token_names_preserved(tokens in unique_tokens(6)) {
        let mut b = GrammarBuilder::new("NP");
        for (n, p) in &tokens {
            b = b.token(n, p);
        }
        let g = b.build();
        let stored: std::collections::HashSet<&str> =
            g.tokens.values().map(|t| t.name.as_str()).collect();
        for (n, _) in &tokens {
            prop_assert!(stored.contains(n.as_str()));
        }
    }

    #[test]
    fn cat4_rule_name_in_rule_names(rname in alpha_name()) {
        let g = GrammarBuilder::new("RN")
            .token("a", "a")
            .rule(&rname, vec!["a"])
            .build();
        let names: Vec<&str> = g.rule_names.values().map(|s| s.as_str()).collect();
        prop_assert!(names.contains(&rname.as_str()));
    }

    #[test]
    fn cat4_name_survives_rules(name in grammar_name(), n in 1usize..=5) {
        let mut b = GrammarBuilder::new(&name);
        b = b.token("x", "x");
        for i in 0..n {
            b = b.rule(&format!("r{i}"), vec!["x"]);
        }
        let g = b.build();
        prop_assert_eq!(&g.name, &name);
    }

    #[test]
    fn cat4_name_nonempty(name in "[a-zA-Z][a-zA-Z0-9]{0,12}") {
        let g = GrammarBuilder::new(&name).build();
        prop_assert!(!g.name.is_empty());
    }
}

// ===========================================================================
// Category 5 — Start symbol is set correctly (5 properties)
// ===========================================================================
proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn cat5_start_is_first_rule(start in alpha_name()) {
        let g = GrammarBuilder::new("S")
            .token("a", "a")
            .rule("other", vec!["a"])
            .rule(&start, vec!["a"])
            .start(&start)
            .build();
        let first_lhs = g.rules.keys().next().unwrap();
        let first_name = g.rule_names.get(first_lhs).map(|s| s.as_str());
        prop_assert_eq!(first_name, Some(start.as_str()));
    }

    #[test]
    fn cat5_start_symbol_lookup(start in alpha_name()) {
        let g = GrammarBuilder::new("SL")
            .token("a", "a")
            .rule(&start, vec!["a"])
            .start(&start)
            .build();
        let sid = g.find_symbol_by_name(&start);
        prop_assert!(sid.is_some());
    }

    #[test]
    fn cat5_start_among_rules(start in alpha_name()) {
        let g = GrammarBuilder::new("SR")
            .token("a", "a")
            .rule(&start, vec!["a"])
            .start(&start)
            .build();
        let sid = g.find_symbol_by_name(&start).unwrap();
        prop_assert!(g.rules.contains_key(&sid));
    }

    #[test]
    fn cat5_no_start_uses_first_rule_lhs(_dummy in 0..1i32) {
        let g = GrammarBuilder::new("NS")
            .token("x", "x")
            .rule("alpha", vec!["x"])
            .build();
        let sym = g.start_symbol();
        prop_assert!(sym.is_some());
    }

    #[test]
    fn cat5_start_reorder_preserves_all(n in 2usize..=5) {
        let mut b = GrammarBuilder::new("Reord");
        b = b.token("a", "a");
        for i in 0..n {
            b = b.rule(&format!("r{i}"), vec!["a"]);
        }
        let last = format!("r{}", n - 1);
        b = b.start(&last);
        let g = b.build();
        prop_assert_eq!(g.rules.len(), n);
    }
}

// ===========================================================================
// Category 6 — Normalize preserves token count (5 properties)
// ===========================================================================
proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]

    #[test]
    fn cat6_normalize_preserves_tokens(tokens in unique_tokens(5)) {
        let mut g = build_grammar("Norm", &tokens, None, None);
        let before = g.tokens.len();
        let _new_rules = g.normalize();
        prop_assert_eq!(g.tokens.len(), before);
    }

    #[test]
    fn cat6_normalize_preserves_name(name in grammar_name()) {
        let mut g = GrammarBuilder::new(&name)
            .token("a", "a")
            .rule("root", vec!["a"])
            .start("root")
            .build();
        let _new = g.normalize();
        prop_assert_eq!(&g.name, &name);
    }

    #[test]
    fn cat6_normalize_empty_grammar(name in grammar_name()) {
        let mut g = GrammarBuilder::new(&name).build();
        let new_rules = g.normalize();
        prop_assert!(new_rules.is_empty());
    }

    #[test]
    fn cat6_normalize_single_token(_dummy in 0..1i32) {
        let mut g = GrammarBuilder::new("ST")
            .token("a", "a")
            .rule("root", vec!["a"])
            .start("root")
            .build();
        let before = g.tokens.len();
        let _new = g.normalize();
        prop_assert_eq!(g.tokens.len(), before);
    }

    #[test]
    fn cat6_normalize_many_rules(n in 1usize..=6) {
        let mut b = GrammarBuilder::new("MR");
        b = b.token("x", "x");
        for i in 0..n {
            b = b.rule(&format!("r{i}"), vec!["x"]);
        }
        b = b.start("r0");
        let mut g = b.build();
        let tok_before = g.tokens.len();
        let _new = g.normalize();
        prop_assert_eq!(g.tokens.len(), tok_before);
    }
}

// ===========================================================================
// Category 7 — Validate passes for well-formed grammars (5 properties)
// ===========================================================================
proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn cat7_validate_simple(tokens in unique_tokens(4)) {
        if tokens.is_empty() { return Ok(()); }
        let g = build_grammar("Val", &tokens, Some("root"), Some("root"));
        prop_assert!(g.validate().is_ok());
    }

    #[test]
    fn cat7_validate_empty_grammar(name in grammar_name()) {
        let g = GrammarBuilder::new(&name).build();
        prop_assert!(g.validate().is_ok());
    }

    #[test]
    fn cat7_validate_tokens_only(tokens in unique_tokens(5)) {
        let mut b = GrammarBuilder::new("TO");
        for (n, p) in &tokens {
            b = b.token(n, p);
        }
        let g = b.build();
        prop_assert!(g.validate().is_ok());
    }

    #[test]
    fn cat7_validate_multiple_alternatives(alts in 2usize..=5) {
        let mut b = GrammarBuilder::new("MA");
        for i in 0..alts {
            b = b.token(&format!("t{i}"), &format!("v{i}"));
        }
        for i in 0..alts {
            b = b.rule("expr", vec![&format!("t{i}")]);
        }
        b = b.start("expr");
        let g = b.build();
        prop_assert!(g.validate().is_ok());
    }

    #[test]
    fn cat7_validate_epsilon_rule(rname in alpha_name()) {
        let g = GrammarBuilder::new("Eps")
            .rule(&rname, vec![])
            .start(&rname)
            .build();
        prop_assert!(g.validate().is_ok());
    }
}

// ===========================================================================
// Category 8 — Serialize / deserialize roundtrip (5 properties)
// ===========================================================================
proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]

    #[test]
    fn cat8_json_roundtrip_name(name in grammar_name()) {
        let g = GrammarBuilder::new(&name).token("a", "a").build();
        let json = serde_json::to_string(&g).unwrap();
        let g2: Grammar = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(&g.name, &g2.name);
    }

    #[test]
    fn cat8_json_roundtrip_tokens(tokens in unique_tokens(5)) {
        let g = build_grammar("RT", &tokens, None, None);
        let json = serde_json::to_string(&g).unwrap();
        let g2: Grammar = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(g.tokens.len(), g2.tokens.len());
    }

    #[test]
    fn cat8_json_roundtrip_rules(n in 1usize..=6) {
        let mut b = GrammarBuilder::new("RR");
        b = b.token("x", "x");
        for i in 0..n {
            b = b.rule(&format!("r{i}"), vec!["x"]);
        }
        let g = b.build();
        let json = serde_json::to_string(&g).unwrap();
        let g2: Grammar = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(g.rules.len(), g2.rules.len());
    }

    #[test]
    fn cat8_json_roundtrip_full(tokens in unique_tokens(4)) {
        if tokens.is_empty() { return Ok(()); }
        let g = build_grammar("Full", &tokens, Some("root"), Some("root"));
        let json = serde_json::to_string(&g).unwrap();
        let g2: Grammar = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(&g, &g2);
    }

    #[test]
    fn cat8_json_roundtrip_epsilon(_dummy in 0..1i32) {
        let g = GrammarBuilder::new("Eps")
            .rule("empty", vec![])
            .start("empty")
            .build();
        let json = serde_json::to_string(&g).unwrap();
        let g2: Grammar = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(&g, &g2);
    }
}

// ===========================================================================
// Category 9 — Edge cases (6 properties)
// ===========================================================================
proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]

    #[test]
    fn cat9_single_token_grammar(_dummy in 0..1i32) {
        let g = GrammarBuilder::new("Single").token("t", "t").build();
        prop_assert_eq!(g.tokens.len(), 1);
        prop_assert!(g.rules.is_empty());
    }

    #[test]
    fn cat9_many_tokens(n in 10usize..=30) {
        let mut b = GrammarBuilder::new("Bulk");
        for i in 0..n {
            b = b.token(&format!("tok{i}"), &format!("p{i}"));
        }
        let g = b.build();
        prop_assert_eq!(g.tokens.len(), n);
    }

    #[test]
    fn cat9_empty_rhs_epsilon(lhs in alpha_name()) {
        let g = GrammarBuilder::new("EmptyRHS")
            .rule(&lhs, vec![])
            .start(&lhs)
            .build();
        let lhs_id = g.find_symbol_by_name(&lhs).unwrap();
        let rules = &g.rules[&lhs_id];
        prop_assert_eq!(rules[0].rhs.len(), 1);
        prop_assert!(matches!(rules[0].rhs[0], Symbol::Epsilon));
    }

    #[test]
    fn cat9_production_ids_unique(n in 1usize..=10) {
        let mut b = GrammarBuilder::new("ProdUniq");
        b = b.token("x", "x");
        for i in 0..n {
            b = b.rule(&format!("r{i}"), vec!["x"]);
        }
        let g = b.build();
        let mut ids = std::collections::HashSet::new();
        for rule in g.all_rules() {
            prop_assert!(ids.insert(rule.production_id));
        }
    }

    #[test]
    fn cat9_extras_tracked(tname in alpha_name()) {
        let g = GrammarBuilder::new("Ext")
            .token(&tname, "ws")
            .extra(&tname)
            .build();
        prop_assert_eq!(g.extras.len(), 1);
    }

    #[test]
    fn cat9_fragile_token_flag(tname in alpha_name()) {
        let g = GrammarBuilder::new("Frag")
            .fragile_token(&tname, "err")
            .build();
        let tok = g.tokens.values().next().unwrap();
        prop_assert!(tok.fragile);
        prop_assert_eq!(&tok.name, &tname);
    }
}
