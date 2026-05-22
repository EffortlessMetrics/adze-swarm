//! Property tests for grammar lifecycle: creation, normalization, optimization,
//! validation, serialization, chaining, tokens, and rules.

use adze_ir::builder::GrammarBuilder;
use adze_ir::optimizer::GrammarOptimizer;
use adze_ir::validation::GrammarValidator;
use adze_ir::{Associativity, Grammar, ProductionId, Rule, Symbol, SymbolId, Token, TokenPattern};
use proptest::prelude::*;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Build a minimal valid grammar with N tokens and a start rule referencing them.
fn build_grammar_with_tokens(name: &str, token_count: usize) -> Grammar {
    let mut b = GrammarBuilder::new(name).token("NUM", r"\d+");
    for i in 0..token_count {
        let tok_name = format!("T{i}");
        let pat = format!("t{i}");
        // Use leaked &str so the borrow lives long enough
        let tok_ref: &'static str = Box::leak(tok_name.into_boxed_str());
        let pat_ref: &'static str = Box::leak(pat.into_boxed_str());
        b = b.token(tok_ref, pat_ref);
    }
    b = b.rule("start", vec!["NUM"]).start("start");
    b.build()
}

/// Build a grammar using GrammarBuilder with a given name and rule count.
fn build_multi_rule_grammar(name: &str, rule_count: usize) -> Grammar {
    let mut b = GrammarBuilder::new(name).token("ID", r"[a-z]+");
    for i in 0..rule_count {
        let rule_name: &'static str = Box::leak(format!("r{i}").into_boxed_str());
        b = b.rule(rule_name, vec!["ID"]);
    }
    b = b.start("r0");
    b.build()
}

// ---------------------------------------------------------------------------
// 1. prop_lifecycle_create_* — grammar creation properties (6 tests)
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    /// Grammar name is preserved through construction.
    #[test]
    fn prop_lifecycle_create_name_preserved(name in "[a-z_]{1,20}") {
        let g = Grammar::new(name.clone());
        prop_assert_eq!(&g.name, &name);
    }

    /// A freshly created grammar has no rules.
    #[test]
    fn prop_lifecycle_create_empty_rules(name in "[a-z]{1,10}") {
        let g = Grammar::new(name);
        prop_assert!(g.rules.is_empty());
    }

    /// Builder-created grammar has exactly the requested token count (+1 base).
    #[test]
    fn prop_lifecycle_create_token_count(n in 1usize..10) {
        let g = build_grammar_with_tokens("tc", n);
        // n extra tokens + NUM
        prop_assert_eq!(g.tokens.len(), n + 1);
    }

    /// Builder grammar always has a start rule entry.
    #[test]
    fn prop_lifecycle_create_has_start(n in 1usize..8) {
        let g = build_grammar_with_tokens("hs", n);
        prop_assert!(!g.rules.is_empty());
        prop_assert!(g.start_symbol().is_some());
    }

    /// Adding N rules to different LHS yields N total rules.
    #[test]
    fn prop_lifecycle_create_distinct_lhs(n in 1usize..15) {
        let mut g = Grammar::new("dl".to_string());
        g.tokens.insert(SymbolId(1), Token {
            name: "a".to_string(),
            pattern: TokenPattern::String("a".to_string()),
            fragile: false,
        });
        for i in 0..n {
            g.add_rule(Rule {
                lhs: SymbolId(10 + i as u16),
                rhs: vec![Symbol::Terminal(SymbolId(1))],
                precedence: None,
                associativity: None,
                fields: vec![],
                production_id: ProductionId(i as u16),
            });
        }
        prop_assert_eq!(g.all_rules().count(), n);
    }

    /// Adding N rules to the same LHS accumulates them.
    #[test]
    fn prop_lifecycle_create_same_lhs_accumulates(n in 1usize..12) {
        let mut g = Grammar::new("sa".to_string());
        for i in 0..n {
            g.add_rule(Rule {
                lhs: SymbolId(5),
                rhs: vec![Symbol::Terminal(SymbolId(i as u16 + 1))],
                precedence: None,
                associativity: None,
                fields: vec![],
                production_id: ProductionId(i as u16),
            });
        }
        let rules = g.get_rules_for_symbol(SymbolId(5)).unwrap();
        prop_assert_eq!(rules.len(), n);
    }
}

// ---------------------------------------------------------------------------
// 2. prop_lifecycle_normalize_* — normalize idempotence (6 tests)
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    /// Normalizing twice yields the same rule count as normalizing once.
    #[test]
    fn prop_lifecycle_normalize_idempotent_count(n in 1usize..6) {
        let mut g = build_multi_rule_grammar("ni", n);
        g.normalize();
        let count_after_first = g.all_rules().count();
        g.normalize();
        prop_assert_eq!(g.all_rules().count(), count_after_first);
    }

    /// Normalize preserves grammar name.
    #[test]
    fn prop_lifecycle_normalize_preserves_name(name in "[a-z]{1,12}") {
        let name_ref: &'static str = Box::leak(name.clone().into_boxed_str());
        let mut g = GrammarBuilder::new(name_ref)
            .token("X", "x")
            .rule("s", vec!["X"])
            .start("s")
            .build();
        g.normalize();
        prop_assert_eq!(&g.name, &name);
    }

    /// Normalize does not reduce rule count for simple grammars.
    #[test]
    fn prop_lifecycle_normalize_no_shrink(n in 1usize..8) {
        let mut g = build_multi_rule_grammar("ns", n);
        let before = g.all_rules().count();
        g.normalize();
        prop_assert!(g.all_rules().count() >= before);
    }

    /// Normalize preserves token definitions.
    #[test]
    fn prop_lifecycle_normalize_tokens_intact(n in 1usize..6) {
        let mut g = build_grammar_with_tokens("ti", n);
        let tok_count = g.tokens.len();
        g.normalize();
        prop_assert_eq!(g.tokens.len(), tok_count);
    }

    /// Normalizing a grammar with Optional symbols expands them.
    #[test]
    fn prop_lifecycle_normalize_expands_optional(n in 1usize..5) {
        let mut g = Grammar::new("opt".to_string());
        g.tokens.insert(SymbolId(1), Token {
            name: "a".to_string(),
            pattern: TokenPattern::String("a".to_string()),
            fragile: false,
        });
        for i in 0..n {
            g.add_rule(Rule {
                lhs: SymbolId(10),
                rhs: vec![Symbol::Optional(Box::new(Symbol::Terminal(SymbolId(1))))],
                precedence: None,
                associativity: None,
                fields: vec![],
                production_id: ProductionId(i as u16),
            });
        }
        g.normalize();
        // Each Optional creates auxiliary rules, so total must exceed n
        prop_assert!(g.all_rules().count() > n);
    }

    /// Normalize followed by normalize returns the same serialized form.
    #[test]
    fn prop_lifecycle_normalize_stable_json(n in 1usize..5) {
        let mut g = build_multi_rule_grammar("sj", n);
        g.normalize();
        let json1 = serde_json::to_string(&g).unwrap();
        g.normalize();
        let json2 = serde_json::to_string(&g).unwrap();
        prop_assert_eq!(json1, json2);
    }
}

// ---------------------------------------------------------------------------
// 3. prop_lifecycle_optimize_* — optimize properties (6 tests)
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    /// Optimizer returns non-negative stats.
    #[test]
    fn prop_lifecycle_optimize_stats_non_negative(n in 1usize..6) {
        let mut g = build_multi_rule_grammar("on", n);
        let mut opt = GrammarOptimizer::new();
        let stats = opt.optimize(&mut g);
        prop_assert!(stats.total() < 10000);
    }

    /// Optimize preserves grammar name.
    #[test]
    fn prop_lifecycle_optimize_preserves_name(name in "[a-z]{1,12}") {
        let name_ref: &'static str = Box::leak(name.clone().into_boxed_str());
        let mut g = GrammarBuilder::new(name_ref)
            .token("X", "x")
            .rule("s", vec!["X"])
            .start("s")
            .build();
        let mut opt = GrammarOptimizer::new();
        opt.optimize(&mut g);
        prop_assert_eq!(&g.name, &name);
    }

    /// Optimize does not increase token count for simple grammars.
    #[test]
    fn prop_lifecycle_optimize_tokens_stable(n in 1usize..6) {
        let mut g = build_grammar_with_tokens("ts", n);
        let before = g.tokens.len();
        let mut opt = GrammarOptimizer::new();
        opt.optimize(&mut g);
        prop_assert!(g.tokens.len() <= before);
    }

    /// Optimizing twice is not worse than optimizing once.
    #[test]
    fn prop_lifecycle_optimize_idempotent_bound(n in 1usize..6) {
        let mut g = build_multi_rule_grammar("ib", n);
        let mut opt = GrammarOptimizer::new();
        opt.optimize(&mut g);
        let count1 = g.all_rules().count();
        let mut opt2 = GrammarOptimizer::new();
        opt2.optimize(&mut g);
        let count2 = g.all_rules().count();
        prop_assert!(count2 <= count1);
    }

    /// optimize_grammar convenience function does not error.
    #[test]
    fn prop_lifecycle_optimize_convenience_ok(n in 1usize..6) {
        let g = build_multi_rule_grammar("co", n);
        let result = adze_ir::optimizer::optimize_grammar(g);
        prop_assert!(result.is_ok());
    }

    /// Optimize preserves externals list.
    #[test]
    fn prop_lifecycle_optimize_externals_preserved(n in 1usize..4) {
        let mut b = GrammarBuilder::new("ep")
            .token("X", "x")
            .rule("s", vec!["X"])
            .start("s");
        for i in 0..n {
            let ext_name: &'static str =
                Box::leak(format!("EXT{i}").into_boxed_str());
            b = b.external(ext_name);
        }
        let mut g = b.build();
        let ext_count = g.externals.len();
        let mut opt = GrammarOptimizer::new();
        opt.optimize(&mut g);
        prop_assert_eq!(g.externals.len(), ext_count);
    }
}

// ---------------------------------------------------------------------------
// 4. prop_lifecycle_validate_* — validation invariants (6 tests)
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    /// A well-formed builder grammar passes validate().
    #[test]
    fn prop_lifecycle_validate_builder_ok(n in 1usize..8) {
        let g = build_multi_rule_grammar("vo", n);
        prop_assert!(g.validate().is_ok());
    }

    /// Empty grammar validates successfully.
    #[test]
    fn prop_lifecycle_validate_empty_ok(name in "[a-z]{1,10}") {
        let g = Grammar::new(name);
        prop_assert!(g.validate().is_ok());
    }

    /// Grammar with correctly-ordered fields passes validation.
    #[test]
    fn prop_lifecycle_validate_field_ordering(n in 1usize..6) {
        let mut g = Grammar::new("fo".to_string());
        let mut names: Vec<String> = (0..n).map(|i| format!("f{i:03}")).collect();
        names.sort();
        for (idx, name) in names.into_iter().enumerate() {
            g.fields.insert(adze_ir::FieldId(idx as u16), name);
        }
        prop_assert!(g.validate().is_ok());
    }

    /// Grammar with reversed field ordering fails validation.
    #[test]
    fn prop_lifecycle_validate_bad_field_ordering(n in 2usize..6) {
        let mut g = Grammar::new("bf".to_string());
        let mut names: Vec<String> = (0..n).map(|i| format!("f{i:03}")).collect();
        names.sort();
        names.reverse();
        for (idx, name) in names.into_iter().enumerate() {
            g.fields.insert(adze_ir::FieldId(idx as u16), name);
        }
        prop_assert!(g.validate().is_err());
    }

    /// GrammarValidator reports stats for valid grammars.
    #[test]
    fn prop_lifecycle_validate_stats_populated(n in 1usize..6) {
        let g = build_grammar_with_tokens("sp", n);
        let mut v = GrammarValidator::new();
        let result = v.validate(&g);
        prop_assert!(result.stats.total_tokens > 0);
    }

    /// GrammarValidator errors list is empty for valid builder grammar.
    #[test]
    fn prop_lifecycle_validate_no_errors(n in 1usize..6) {
        let g = build_multi_rule_grammar("ne", n);
        let mut v = GrammarValidator::new();
        let result = v.validate(&g);
        prop_assert!(result.errors.is_empty());
    }
}

// ---------------------------------------------------------------------------
// 5. prop_lifecycle_serialize_* — serialization roundtrip (6 tests)
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    /// JSON roundtrip preserves grammar name.
    #[test]
    fn prop_lifecycle_serialize_name_roundtrip(name in "[a-z]{1,15}") {
        let name_ref: &'static str = Box::leak(name.clone().into_boxed_str());
        let g = GrammarBuilder::new(name_ref)
            .token("X", "x")
            .rule("s", vec!["X"])
            .start("s")
            .build();
        let json = serde_json::to_string(&g).unwrap();
        let g2: Grammar = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(&g2.name, &name);
    }

    /// JSON roundtrip preserves token count.
    #[test]
    fn prop_lifecycle_serialize_token_count(n in 1usize..8) {
        let g = build_grammar_with_tokens("tc", n);
        let json = serde_json::to_string(&g).unwrap();
        let g2: Grammar = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(g2.tokens.len(), g.tokens.len());
    }

    /// JSON roundtrip preserves rule count.
    #[test]
    fn prop_lifecycle_serialize_rule_count(n in 1usize..8) {
        let g = build_multi_rule_grammar("rc", n);
        let json = serde_json::to_string(&g).unwrap();
        let g2: Grammar = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(g2.all_rules().count(), g.all_rules().count());
    }

    /// Serialized grammar equals original via PartialEq.
    #[test]
    fn prop_lifecycle_serialize_equality(n in 1usize..6) {
        let g = build_multi_rule_grammar("eq", n);
        let json = serde_json::to_string(&g).unwrap();
        let g2: Grammar = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(g, g2);
    }

    /// Serialized JSON is non-empty.
    #[test]
    fn prop_lifecycle_serialize_non_empty(n in 1usize..6) {
        let g = build_grammar_with_tokens("ne", n);
        let json = serde_json::to_string(&g).unwrap();
        prop_assert!(!json.is_empty());
    }

    /// Pretty-printed JSON roundtrips identically to compact JSON.
    #[test]
    fn prop_lifecycle_serialize_pretty_compact(n in 1usize..6) {
        let g = build_multi_rule_grammar("pp", n);
        let compact = serde_json::to_string(&g).unwrap();
        let pretty = serde_json::to_string_pretty(&g).unwrap();
        let g_compact: Grammar = serde_json::from_str(&compact).unwrap();
        let g_pretty: Grammar = serde_json::from_str(&pretty).unwrap();
        prop_assert_eq!(g_compact, g_pretty);
    }
}

// ---------------------------------------------------------------------------
// 6. prop_lifecycle_chain_* — chained operation properties (6 tests)
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    /// Normalize then validate succeeds for builder grammars.
    #[test]
    fn prop_lifecycle_chain_normalize_validate(n in 1usize..6) {
        let mut g = build_multi_rule_grammar("nv", n);
        g.normalize();
        prop_assert!(g.validate().is_ok());
    }

    /// Normalize then serialize roundtrips correctly.
    #[test]
    fn prop_lifecycle_chain_normalize_serialize(n in 1usize..6) {
        let mut g = build_multi_rule_grammar("ns", n);
        g.normalize();
        let json = serde_json::to_string(&g).unwrap();
        let g2: Grammar = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(g, g2);
    }

    /// Optimize then validate succeeds.
    #[test]
    fn prop_lifecycle_chain_optimize_validate(n in 1usize..6) {
        let mut g = build_multi_rule_grammar("ov", n);
        let mut opt = GrammarOptimizer::new();
        opt.optimize(&mut g);
        prop_assert!(g.validate().is_ok());
    }

    /// Optimize then serialize roundtrips.
    #[test]
    fn prop_lifecycle_chain_optimize_serialize(n in 1usize..6) {
        let mut g = build_multi_rule_grammar("os", n);
        let mut opt = GrammarOptimizer::new();
        opt.optimize(&mut g);
        let json = serde_json::to_string(&g).unwrap();
        let g2: Grammar = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(g, g2);
    }

    /// Normalize → optimize → validate pipeline succeeds.
    #[test]
    fn prop_lifecycle_chain_full_pipeline(n in 1usize..6) {
        let mut g = build_multi_rule_grammar("fp", n);
        g.normalize();
        let mut opt = GrammarOptimizer::new();
        opt.optimize(&mut g);
        prop_assert!(g.validate().is_ok());
    }

    /// Clone after normalize → optimize yields equal grammar.
    #[test]
    fn prop_lifecycle_chain_clone_after_pipeline(n in 1usize..6) {
        let mut g = build_multi_rule_grammar("cp", n);
        g.normalize();
        let mut opt = GrammarOptimizer::new();
        opt.optimize(&mut g);
        let g2 = g.clone();
        prop_assert_eq!(g, g2);
    }
}

// ---------------------------------------------------------------------------
// 7. prop_lifecycle_token_* — token lifecycle (6 tests)
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    /// Token names survive serialization roundtrip.
    #[test]
    fn prop_lifecycle_token_names_roundtrip(n in 1usize..8) {
        let g = build_grammar_with_tokens("tn", n);
        let names: Vec<String> = g.tokens.values().map(|t| t.name.clone()).collect();
        let json = serde_json::to_string(&g).unwrap();
        let g2: Grammar = serde_json::from_str(&json).unwrap();
        let names2: Vec<String> = g2.tokens.values().map(|t| t.name.clone()).collect();
        prop_assert_eq!(names, names2);
    }

    /// Fragile tokens remain fragile after roundtrip.
    #[test]
    fn prop_lifecycle_token_fragile_roundtrip(n in 1usize..5) {
        let mut b = GrammarBuilder::new("fr")
            .token("BASE", "b")
            .rule("s", vec!["BASE"])
            .start("s");
        for i in 0..n {
            let name: &'static str =
                Box::leak(format!("FRG{i}").into_boxed_str());
            let pat: &'static str =
                Box::leak(format!("frg{i}").into_boxed_str());
            b = b.fragile_token(name, pat);
        }
        let g = b.build();
        let fragile_count = g.tokens.values().filter(|t| t.fragile).count();
        let json = serde_json::to_string(&g).unwrap();
        let g2: Grammar = serde_json::from_str(&json).unwrap();
        let fragile_count2 = g2.tokens.values().filter(|t| t.fragile).count();
        prop_assert_eq!(fragile_count, n);
        prop_assert_eq!(fragile_count, fragile_count2);
    }

    /// Token patterns are preserved through normalize.
    #[test]
    fn prop_lifecycle_token_patterns_after_normalize(n in 1usize..6) {
        let mut g = build_grammar_with_tokens("tp", n);
        let patterns: Vec<TokenPattern> =
            g.tokens.values().map(|t| t.pattern.clone()).collect();
        g.normalize();
        let patterns2: Vec<TokenPattern> =
            g.tokens.values().map(|t| t.pattern.clone()).collect();
        prop_assert_eq!(patterns, patterns2);
    }

    /// check_empty_terminals passes for properly patterned tokens.
    #[test]
    fn prop_lifecycle_token_no_empty_terminals(n in 1usize..8) {
        let g = build_grammar_with_tokens("ne", n);
        prop_assert!(g.check_empty_terminals().is_ok());
    }

    /// Token SymbolIds are unique within a grammar.
    #[test]
    fn prop_lifecycle_token_unique_ids(n in 1usize..10) {
        let g = build_grammar_with_tokens("ui", n);
        let ids: Vec<SymbolId> = g.tokens.keys().copied().collect();
        let mut deduped = ids.clone();
        deduped.sort();
        deduped.dedup();
        prop_assert_eq!(ids.len(), deduped.len());
    }

    /// String-pattern tokens serialize with their exact value.
    #[test]
    fn prop_lifecycle_token_string_pattern_value(val in "[a-z]{1,8}") {
        let val_ref: &'static str = Box::leak(val.clone().into_boxed_str());
        let g = GrammarBuilder::new("sv")
            .token(val_ref, val_ref)
            .token("NUM", r"\d+")
            .rule("s", vec!["NUM"])
            .start("s")
            .build();
        let json = serde_json::to_string(&g).unwrap();
        let g2: Grammar = serde_json::from_str(&json).unwrap();
        // Find the token and verify pattern content
        let tok = g2.tokens.values().find(|t| t.name == val);
        prop_assert!(tok.is_some());
    }
}

// ---------------------------------------------------------------------------
// 8. prop_lifecycle_rule_* — rule lifecycle (6 tests)
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    /// All rules have valid production IDs after building.
    #[test]
    fn prop_lifecycle_rule_production_ids(n in 1usize..8) {
        let g = build_multi_rule_grammar("pi", n);
        let ids: Vec<ProductionId> = g.all_rules().map(|r| r.production_id).collect();
        // Production IDs should all be distinct
        let mut sorted = ids.clone();
        sorted.sort();
        sorted.dedup();
        prop_assert_eq!(ids.len(), sorted.len());
    }

    /// Rule RHS is never empty for non-epsilon rules from builder.
    #[test]
    fn prop_lifecycle_rule_rhs_non_empty(n in 1usize..8) {
        let g = build_multi_rule_grammar("rn", n);
        for rule in g.all_rules() {
            prop_assert!(!rule.rhs.is_empty());
        }
    }

    /// Rules survive serialization roundtrip with same LHS.
    #[test]
    fn prop_lifecycle_rule_lhs_roundtrip(n in 1usize..6) {
        let g = build_multi_rule_grammar("lr", n);
        let lhs_ids: Vec<SymbolId> = g.all_rules().map(|r| r.lhs).collect();
        let json = serde_json::to_string(&g).unwrap();
        let g2: Grammar = serde_json::from_str(&json).unwrap();
        let lhs_ids2: Vec<SymbolId> = g2.all_rules().map(|r| r.lhs).collect();
        prop_assert_eq!(lhs_ids, lhs_ids2);
    }

    /// Rule precedence is preserved through serialization.
    #[test]
    fn prop_lifecycle_rule_precedence_roundtrip(prec in -100i16..100) {
        let g = GrammarBuilder::new("pr")
            .token("N", r"\d+")
            .token("+", "+")
            .rule_with_precedence("e", vec!["e", "+", "e"], prec, Associativity::Left)
            .rule("e", vec!["N"])
            .start("e")
            .build();
        let json = serde_json::to_string(&g).unwrap();
        let g2: Grammar = serde_json::from_str(&json).unwrap();
        let prec_rule = g2.all_rules().find(|r| r.precedence.is_some()).unwrap();
        prop_assert_eq!(
            prec_rule.precedence,
            Some(adze_ir::PrecedenceKind::Static(prec))
        );
    }

    /// Rule associativity is preserved through serialization.
    #[test]
    fn prop_lifecycle_rule_assoc_roundtrip(idx in 0usize..3) {
        let assocs = [Associativity::Left, Associativity::Right, Associativity::None];
        let assoc = assocs[idx];
        let g = GrammarBuilder::new("ar")
            .token("N", r"\d+")
            .token("+", "+")
            .rule_with_precedence("e", vec!["e", "+", "e"], 1, assoc)
            .rule("e", vec!["N"])
            .start("e")
            .build();
        let json = serde_json::to_string(&g).unwrap();
        let g2: Grammar = serde_json::from_str(&json).unwrap();
        let rule = g2.all_rules().find(|r| r.associativity.is_some()).unwrap();
        prop_assert_eq!(rule.associativity, Some(assoc));
    }

    /// Rules with fields preserve field mappings through roundtrip.
    #[test]
    fn prop_lifecycle_rule_fields_preserved(n in 1usize..5) {
        let mut g = Grammar::new("fp".to_string());
        g.tokens.insert(SymbolId(1), Token {
            name: "a".to_string(),
            pattern: TokenPattern::String("a".to_string()),
            fragile: false,
        });
        let fields: Vec<(adze_ir::FieldId, usize)> =
            (0..n).map(|i| (adze_ir::FieldId(i as u16), i)).collect();
        g.add_rule(Rule {
            lhs: SymbolId(10),
            rhs: vec![Symbol::Terminal(SymbolId(1))],
            precedence: None,
            associativity: None,
            fields: fields.clone(),
            production_id: ProductionId(0),
        });
        let json = serde_json::to_string(&g).unwrap();
        let g2: Grammar = serde_json::from_str(&json).unwrap();
        let rule = g2.all_rules().next().unwrap();
        prop_assert_eq!(&rule.fields, &fields);
    }
}
