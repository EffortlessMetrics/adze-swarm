//! Comprehensive tests for production ID management and alias sequences in adze-ir Grammar.

use adze_ir::builder::GrammarBuilder;
use adze_ir::{AliasSequence, Associativity, Grammar, ProductionId, RuleId, SymbolId};
use std::collections::HashSet;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn build_minimal(name: &str) -> Grammar {
    GrammarBuilder::new(name)
        .token("NUM", r"\d+")
        .rule("start", vec!["NUM"])
        .start("start")
        .build()
}

fn build_two_rules(name: &str) -> Grammar {
    GrammarBuilder::new(name)
        .token("NUM", r"\d+")
        .token("+", "+")
        .rule("expr", vec!["NUM"])
        .rule("expr", vec!["expr", "+", "expr"])
        .start("expr")
        .build()
}

fn build_n_rules(name: &str, n: usize) -> Grammar {
    let mut b = GrammarBuilder::new(name);
    b = b.token("a", "a");
    for i in 0..n {
        let rule_name: &'static str = Box::leak(format!("r{i}").into_boxed_str());
        b = b.rule(rule_name, vec!["a"]);
    }
    if n > 0 {
        b = b.start("r0");
    }
    b.build()
}

fn populate_aliases(g: &mut Grammar, entries: Vec<(u16, Vec<Option<String>>)>) {
    for (id, aliases) in entries {
        g.alias_sequences
            .insert(ProductionId(id), AliasSequence { aliases });
    }
    g.max_alias_sequence_length = g
        .alias_sequences
        .values()
        .map(|s| s.aliases.len())
        .max()
        .unwrap_or(0);
}

fn populate_prod_ids(g: &mut Grammar, count: u16) {
    for i in 0..count {
        g.production_ids.insert(RuleId(i), ProductionId(i));
    }
}

// ===========================================================================
// 1. Grammar default has empty production_ids
// ===========================================================================

#[test]
fn default_grammar_has_empty_production_ids() {
    let g = Grammar::default();
    assert_eq!(g.production_ids.len(), g.all_rules().count());
}

#[test]
fn grammar_new_has_empty_production_ids() {
    let g = Grammar::new("pi_v10_new_empty_pid".to_string());
    assert_eq!(g.production_ids.len(), g.all_rules().count());
}

// ===========================================================================
// 2. Grammar default has empty alias_sequences
// ===========================================================================

#[test]
fn default_grammar_has_empty_alias_sequences() {
    let g = Grammar::default();
    assert!(g.alias_sequences.is_empty());
}

#[test]
fn grammar_new_has_empty_alias_sequences() {
    let g = Grammar::new("pi_v10_new_empty_alias".to_string());
    assert!(g.alias_sequences.is_empty());
}

// ===========================================================================
// 3. Grammar default max_alias_sequence_length is 0
// ===========================================================================

#[test]
fn default_grammar_max_alias_sequence_length_is_zero() {
    let g = Grammar::default();
    assert_eq!(g.max_alias_sequence_length, 0);
}

#[test]
fn grammar_new_max_alias_sequence_length_is_zero() {
    let g = Grammar::new("pi_v10_new_max_zero".to_string());
    assert_eq!(g.max_alias_sequence_length, 0);
}

// ===========================================================================
// 4. Built grammar has production_ids on rules
// ===========================================================================

#[test]
fn built_grammar_rules_have_production_ids() {
    let g = build_minimal("pi_v10_built_has_pids");
    let ids: Vec<ProductionId> = g.all_rules().map(|r| r.production_id).collect();
    assert!(!ids.is_empty());
}

#[test]
fn built_grammar_every_rule_has_a_production_id() {
    let g = build_two_rules("pi_v10_built_every_rule");
    let count = g.all_rules().count();
    let ids: Vec<ProductionId> = g.all_rules().map(|r| r.production_id).collect();
    assert_eq!(ids.len(), count);
}

#[test]
fn built_grammar_production_ids_start_at_zero() {
    let g = build_minimal("pi_v10_built_start_zero");
    let min_id = g.all_rules().map(|r| r.production_id.0).min().unwrap();
    assert_eq!(min_id, 0);
}

// ===========================================================================
// 5. Production IDs count relates to rule count
// ===========================================================================

#[test]
fn production_id_count_equals_rule_count_minimal() {
    let g = build_minimal("pi_v10_count_min");
    let rule_count = g.all_rules().count();
    let id_count: HashSet<u16> = g.all_rules().map(|r| r.production_id.0).collect();
    assert_eq!(id_count.len(), rule_count);
}

#[test]
fn production_id_count_equals_rule_count_two_rules() {
    let g = build_two_rules("pi_v10_count_two");
    let rule_count = g.all_rules().count();
    let id_count: HashSet<u16> = g.all_rules().map(|r| r.production_id.0).collect();
    assert_eq!(id_count.len(), rule_count);
}

#[test]
fn production_id_count_equals_rule_count_n_rules() {
    let g = build_n_rules("pi_v10_count_n", 8);
    let rule_count = g.all_rules().count();
    let id_count: HashSet<u16> = g.all_rules().map(|r| r.production_id.0).collect();
    assert_eq!(id_count.len(), rule_count);
}

// ===========================================================================
// 6. Alias sequences for simple grammar
// ===========================================================================

#[test]
fn simple_grammar_alias_sequences_empty_by_default() {
    let g = build_minimal("pi_v10_simple_alias_empty");
    assert!(g.alias_sequences.is_empty());
}

#[test]
fn simple_grammar_supports_alias_population() {
    let mut g = build_minimal("pi_v10_simple_alias_pop");
    populate_aliases(&mut g, vec![(0, vec![Some("node".to_string())])]);
    assert_eq!(g.alias_sequences.len(), 1);
    assert_eq!(
        g.alias_sequences[&ProductionId(0)].aliases[0].as_deref(),
        Some("node")
    );
}

#[test]
fn two_rule_grammar_alias_sequences_empty_by_default() {
    let g = build_two_rules("pi_v10_two_alias_empty");
    assert!(g.alias_sequences.is_empty());
}

// ===========================================================================
// 7. Clone preserves production_ids
// ===========================================================================

#[test]
fn clone_preserves_production_ids_map() {
    let mut g = Grammar::new("pi_v10_clone_pid_map".to_string());
    populate_prod_ids(&mut g, 5);
    let cloned = g.clone();
    assert_eq!(g.production_ids, cloned.production_ids);
}

#[test]
fn clone_preserves_rule_production_ids() {
    let g = build_two_rules("pi_v10_clone_rule_pids");
    let cloned = g.clone();
    let orig: Vec<ProductionId> = g.all_rules().map(|r| r.production_id).collect();
    let copy: Vec<ProductionId> = cloned.all_rules().map(|r| r.production_id).collect();
    assert_eq!(orig, copy);
}

// ===========================================================================
// 8. Clone preserves alias_sequences
// ===========================================================================

#[test]
fn clone_preserves_alias_sequences() {
    let mut g = build_minimal("pi_v10_clone_alias");
    populate_aliases(
        &mut g,
        vec![
            (0, vec![Some("a".to_string())]),
            (1, vec![None, Some("b".to_string())]),
        ],
    );
    let cloned = g.clone();
    assert_eq!(g.alias_sequences, cloned.alias_sequences);
}

#[test]
fn clone_alias_sequences_independent() {
    let mut g = build_minimal("pi_v10_clone_alias_indep");
    populate_aliases(&mut g, vec![(0, vec![Some("orig".to_string())])]);
    let mut cloned = g.clone();
    cloned.alias_sequences.insert(
        ProductionId(99),
        AliasSequence {
            aliases: vec![Some("new".to_string())],
        },
    );
    assert_eq!(g.alias_sequences.len(), 1);
    assert_eq!(cloned.alias_sequences.len(), 2);
}

// ===========================================================================
// 9. Clone preserves max_alias_sequence_length
// ===========================================================================

#[test]
fn clone_preserves_max_alias_sequence_length() {
    let mut g = build_minimal("pi_v10_clone_max");
    populate_aliases(&mut g, vec![(0, vec![None; 7])]);
    let cloned = g.clone();
    assert_eq!(
        g.max_alias_sequence_length,
        cloned.max_alias_sequence_length
    );
}

#[test]
fn clone_max_alias_zero_when_empty() {
    let g = build_minimal("pi_v10_clone_max_zero");
    let cloned = g.clone();
    assert_eq!(cloned.max_alias_sequence_length, 0);
}

// ===========================================================================
// 10. Debug includes production info
// ===========================================================================

#[test]
fn debug_format_includes_production_ids_field() {
    let g = build_minimal("pi_v10_debug_pid");
    let dbg = format!("{g:?}");
    assert!(dbg.contains("production_ids"));
}

#[test]
fn debug_format_includes_alias_sequences_field() {
    let g = build_minimal("pi_v10_debug_alias");
    let dbg = format!("{g:?}");
    assert!(dbg.contains("alias_sequences"));
}

#[test]
fn debug_format_includes_max_alias_length() {
    let g = build_minimal("pi_v10_debug_max");
    let dbg = format!("{g:?}");
    assert!(dbg.contains("max_alias_sequence_length"));
}

#[test]
fn debug_format_shows_populated_alias_values() {
    let mut g = build_minimal("pi_v10_debug_pop");
    populate_aliases(&mut g, vec![(0, vec![Some("visible_v10".to_string())])]);
    let dbg = format!("{g:?}");
    assert!(dbg.contains("visible_v10"));
}

// ===========================================================================
// 11. Normalize updates production_ids
// ===========================================================================

#[test]
fn normalize_preserves_alias_sequences() {
    let mut g = build_two_rules("pi_v10_norm_alias");
    populate_aliases(&mut g, vec![(0, vec![Some("kept".to_string())])]);
    let _ = g.normalize();
    assert_eq!(g.alias_sequences.len(), 1);
    assert_eq!(
        g.alias_sequences[&ProductionId(0)].aliases[0].as_deref(),
        Some("kept")
    );
}

#[test]
fn normalize_preserves_max_alias_length() {
    let mut g = build_two_rules("pi_v10_norm_max");
    populate_aliases(&mut g, vec![(0, vec![None; 4])]);
    let _ = g.normalize();
    assert_eq!(g.max_alias_sequence_length, 4);
}

#[test]
fn normalize_empty_alias_sequences_remain_empty() {
    let mut g = build_minimal("pi_v10_norm_empty");
    let _ = g.normalize();
    assert!(g.alias_sequences.is_empty());
    assert_eq!(g.max_alias_sequence_length, 0);
}

#[test]
fn normalize_does_not_add_alias_sequences() {
    let mut g = build_two_rules("pi_v10_norm_no_add");
    let _ = g.normalize();
    assert!(g.alias_sequences.is_empty());
}

// ===========================================================================
// 12. Optimize preserves production_ids
// ===========================================================================

#[test]
fn optimize_preserves_production_ids_on_rules() {
    let mut g = build_two_rules("pi_v10_opt_pid");
    let ids_before: Vec<ProductionId> = g.all_rules().map(|r| r.production_id).collect();
    g.optimize();
    let ids_after: Vec<ProductionId> = g.all_rules().map(|r| r.production_id).collect();
    assert_eq!(ids_before, ids_after);
}

#[test]
fn optimize_preserves_alias_sequences() {
    let mut g = build_two_rules("pi_v10_opt_alias");
    populate_aliases(&mut g, vec![(0, vec![Some("kept".to_string())])]);
    g.optimize();
    assert_eq!(
        g.alias_sequences[&ProductionId(0)].aliases[0].as_deref(),
        Some("kept")
    );
}

#[test]
fn optimize_preserves_max_alias_length() {
    let mut g = build_two_rules("pi_v10_opt_max");
    populate_aliases(&mut g, vec![(0, vec![None; 3])]);
    g.optimize();
    assert_eq!(g.max_alias_sequence_length, 3);
}

#[test]
fn optimize_empty_grammar_no_alias_change() {
    let mut g = Grammar::new("pi_v10_opt_empty".to_string());
    g.optimize();
    assert!(g.alias_sequences.is_empty());
    assert_eq!(g.max_alias_sequence_length, 0);
}

// ===========================================================================
// 13. Grammar with 1 rule → 1+ production IDs
// ===========================================================================

#[test]
fn one_rule_grammar_has_one_production_id() {
    let g = build_minimal("pi_v10_one_rule");
    assert_eq!(g.all_rules().count(), 1);
    assert_eq!(g.all_rules().next().unwrap().production_id, ProductionId(0));
}

#[test]
fn one_rule_grammar_unique_ids() {
    let g = build_minimal("pi_v10_one_unique");
    let ids: HashSet<u16> = g.all_rules().map(|r| r.production_id.0).collect();
    assert_eq!(ids.len(), 1);
}

// ===========================================================================
// 14. Grammar with 5 rules → 5+ production IDs
// ===========================================================================

#[test]
fn five_rule_grammar_has_five_production_ids() {
    let g = build_n_rules("pi_v10_five_rules", 5);
    assert_eq!(g.all_rules().count(), 5);
    let ids: HashSet<u16> = g.all_rules().map(|r| r.production_id.0).collect();
    assert_eq!(ids.len(), 5);
}

#[test]
fn five_rule_grammar_ids_contiguous() {
    let g = build_n_rules("pi_v10_five_contig", 5);
    let mut ids: Vec<u16> = g.all_rules().map(|r| r.production_id.0).collect();
    ids.sort();
    for (i, id) in ids.iter().enumerate() {
        assert_eq!(*id, i as u16);
    }
}

// ===========================================================================
// 15. Grammar with 10 rules → 10+ production IDs
// ===========================================================================

#[test]
fn ten_rule_grammar_has_ten_production_ids() {
    let g = build_n_rules("pi_v10_ten_rules", 10);
    assert_eq!(g.all_rules().count(), 10);
    let ids: HashSet<u16> = g.all_rules().map(|r| r.production_id.0).collect();
    assert_eq!(ids.len(), 10);
}

#[test]
fn ten_rule_grammar_ids_ascending() {
    let g = build_n_rules("pi_v10_ten_asc", 10);
    let ids: Vec<u16> = g.all_rules().map(|r| r.production_id.0).collect();
    for window in ids.windows(2) {
        assert!(window[0] < window[1]);
    }
}

#[test]
fn ten_rule_grammar_alias_population() {
    let mut g = build_n_rules("pi_v10_ten_alias", 10);
    let entries: Vec<(u16, Vec<Option<String>>)> = (0..10)
        .map(|i| (i, vec![Some(format!("alias_{i}"))]))
        .collect();
    populate_aliases(&mut g, entries);
    assert_eq!(g.alias_sequences.len(), 10);
    assert_eq!(g.max_alias_sequence_length, 1);
}

// ===========================================================================
// 16. Production IDs deterministic
// ===========================================================================

#[test]
fn production_ids_deterministic_across_builds() {
    let g1 = build_two_rules("pi_v10_determ_1");
    let g2 = build_two_rules("pi_v10_determ_1");
    let ids1: Vec<u16> = g1.all_rules().map(|r| r.production_id.0).collect();
    let ids2: Vec<u16> = g2.all_rules().map(|r| r.production_id.0).collect();
    assert_eq!(ids1, ids2);
}

#[test]
fn production_ids_deterministic_n_rules() {
    let g1 = build_n_rules("pi_v10_determ_n1", 7);
    let g2 = build_n_rules("pi_v10_determ_n1", 7);
    let ids1: Vec<u16> = g1.all_rules().map(|r| r.production_id.0).collect();
    let ids2: Vec<u16> = g2.all_rules().map(|r| r.production_id.0).collect();
    assert_eq!(ids1, ids2);
}

#[test]
fn production_ids_deterministic_with_precedence() {
    let build = || {
        GrammarBuilder::new("pi_v10_determ_prec")
            .token("NUM", r"\d+")
            .token("+", "+")
            .rule_with_precedence("expr", vec!["expr", "+", "expr"], 1, Associativity::Left)
            .rule("expr", vec!["NUM"])
            .start("expr")
            .build()
    };
    let ids1: Vec<u16> = build().all_rules().map(|r| r.production_id.0).collect();
    let ids2: Vec<u16> = build().all_rules().map(|r| r.production_id.0).collect();
    assert_eq!(ids1, ids2);
}

// ===========================================================================
// 17. Grammar with alternatives → more production IDs
// ===========================================================================

#[test]
fn alternatives_produce_unique_production_ids() {
    let g = GrammarBuilder::new("pi_v10_alt_unique")
        .token("A", "a")
        .token("B", "b")
        .token("C", "c")
        .rule("item", vec!["A"])
        .rule("item", vec!["B"])
        .rule("item", vec!["C"])
        .start("item")
        .build();
    let ids: HashSet<u16> = g.all_rules().map(|r| r.production_id.0).collect();
    assert_eq!(ids.len(), 3);
}

#[test]
fn alternatives_ids_are_sequential() {
    let g = GrammarBuilder::new("pi_v10_alt_seq")
        .token("A", "a")
        .token("B", "b")
        .token("C", "c")
        .token("D", "d")
        .rule("item", vec!["A"])
        .rule("item", vec!["B"])
        .rule("item", vec!["C"])
        .rule("item", vec!["D"])
        .start("item")
        .build();
    let ids: Vec<u16> = g.all_rules().map(|r| r.production_id.0).collect();
    for (i, id) in ids.iter().enumerate() {
        assert_eq!(*id, i as u16);
    }
}

#[test]
fn alternatives_more_rules_more_ids() {
    let g2 = build_n_rules("pi_v10_alt_more_2", 2);
    let g5 = build_n_rules("pi_v10_alt_more_5", 5);
    let count2: HashSet<u16> = g2.all_rules().map(|r| r.production_id.0).collect();
    let count5: HashSet<u16> = g5.all_rules().map(|r| r.production_id.0).collect();
    assert!(count5.len() > count2.len());
}

// ===========================================================================
// 18. Grammar with precedence → production IDs
// ===========================================================================

#[test]
fn precedence_rules_get_production_ids() {
    let g = GrammarBuilder::new("pi_v10_prec_pids")
        .token("NUM", r"\d+")
        .token("+", "+")
        .token("*", "*")
        .rule_with_precedence("expr", vec!["expr", "+", "expr"], 1, Associativity::Left)
        .rule_with_precedence("expr", vec!["expr", "*", "expr"], 2, Associativity::Left)
        .rule("expr", vec!["NUM"])
        .start("expr")
        .build();
    let ids: HashSet<u16> = g.all_rules().map(|r| r.production_id.0).collect();
    assert_eq!(ids.len(), 3);
}

#[test]
fn precedence_rules_ids_sequential() {
    let g = GrammarBuilder::new("pi_v10_prec_seq")
        .token("NUM", r"\d+")
        .token("+", "+")
        .rule_with_precedence("expr", vec!["expr", "+", "expr"], 1, Associativity::Left)
        .rule("expr", vec!["NUM"])
        .start("expr")
        .build();
    let ids: Vec<u16> = g.all_rules().map(|r| r.production_id.0).collect();
    assert_eq!(ids[0], 0);
    assert_eq!(ids[1], 1);
}

#[test]
fn precedence_grammar_alias_sequences_empty() {
    let g = GrammarBuilder::new("pi_v10_prec_alias_empty")
        .token("NUM", r"\d+")
        .token("+", "+")
        .rule_with_precedence("expr", vec!["expr", "+", "expr"], 1, Associativity::Left)
        .rule("expr", vec!["NUM"])
        .start("expr")
        .build();
    assert!(g.alias_sequences.is_empty());
}

#[test]
fn precedence_grammar_supports_alias_population() {
    let mut g = GrammarBuilder::new("pi_v10_prec_alias_pop")
        .token("NUM", r"\d+")
        .token("+", "+")
        .rule_with_precedence("expr", vec!["expr", "+", "expr"], 1, Associativity::Left)
        .rule("expr", vec!["NUM"])
        .start("expr")
        .build();
    populate_aliases(
        &mut g,
        vec![(
            0,
            vec![
                Some("left".to_string()),
                Some("op".to_string()),
                Some("right".to_string()),
            ],
        )],
    );
    assert_eq!(g.max_alias_sequence_length, 3);
}

// ===========================================================================
// 19. Grammar with inline → production IDs after normalize
// ===========================================================================

#[test]
fn inline_grammar_has_production_ids_before_normalize() {
    let g = GrammarBuilder::new("pi_v10_inline_before")
        .token("A", "a")
        .token("B", "b")
        .rule("start", vec!["helper"])
        .rule("helper", vec!["A"])
        .rule("helper", vec!["B"])
        .inline("helper")
        .start("start")
        .build();
    let ids: Vec<u16> = g.all_rules().map(|r| r.production_id.0).collect();
    assert_eq!(ids.len(), 3);
}

#[test]
fn inline_grammar_production_ids_after_normalize() {
    let mut g = GrammarBuilder::new("pi_v10_inline_after")
        .token("A", "a")
        .token("B", "b")
        .rule("start", vec!["helper"])
        .rule("helper", vec!["A"])
        .rule("helper", vec!["B"])
        .inline("helper")
        .start("start")
        .build();
    let _ = g.normalize();
    let ids: Vec<ProductionId> = g.all_rules().map(|r| r.production_id).collect();
    assert!(!ids.is_empty());
}

#[test]
fn inline_grammar_aliases_preserved_after_normalize() {
    let mut g = GrammarBuilder::new("pi_v10_inline_alias_norm")
        .token("A", "a")
        .rule("start", vec!["helper"])
        .rule("helper", vec!["A"])
        .inline("helper")
        .start("start")
        .build();
    populate_aliases(&mut g, vec![(0, vec![Some("inlined".to_string())])]);
    let _ = g.normalize();
    assert_eq!(
        g.alias_sequences[&ProductionId(0)].aliases[0].as_deref(),
        Some("inlined")
    );
}

// ===========================================================================
// 20. All grammar fields consistent after production ID assignment
// ===========================================================================

#[test]
fn all_fields_consistent_after_build() {
    let g = GrammarBuilder::new("pi_v10_consistent")
        .token("A", "a")
        .token("B", "b")
        .rule("start", vec!["A"])
        .rule("start", vec!["B"])
        .start("start")
        .build();
    assert_eq!(g.all_rules().count(), 2);
    assert_eq!(g.production_ids.len(), g.all_rules().count());
    assert!(g.alias_sequences.is_empty());
    assert_eq!(g.max_alias_sequence_length, 0);
    assert!(g.fields.is_empty());
}

#[test]
fn fields_consistent_after_manual_population() {
    let mut g = build_two_rules("pi_v10_consist_manual");
    populate_prod_ids(&mut g, 3);
    populate_aliases(
        &mut g,
        vec![
            (0, vec![Some("x".to_string()), None]),
            (1, vec![Some("y".to_string())]),
        ],
    );
    assert_eq!(g.production_ids.len(), 3);
    assert_eq!(g.alias_sequences.len(), 2);
    assert_eq!(g.max_alias_sequence_length, 2);
}

#[test]
fn fields_consistent_after_normalize_and_optimize() {
    let mut g = build_two_rules("pi_v10_consist_norm_opt");
    populate_aliases(&mut g, vec![(0, vec![Some("z".to_string())])]);
    let _ = g.normalize();
    g.optimize();
    assert_eq!(g.alias_sequences.len(), 1);
    assert_eq!(g.max_alias_sequence_length, 1);
}

// ===========================================================================
// 21. Production IDs on Rule structs
// ===========================================================================

#[test]
fn rule_struct_production_id_accessible() {
    let g = build_minimal("pi_v10_rule_struct_access");
    let rule = g.all_rules().next().unwrap();
    assert_eq!(rule.production_id, ProductionId(0));
}

#[test]
fn rule_struct_production_id_varies_per_rule() {
    let g = build_two_rules("pi_v10_rule_struct_varies");
    let rules: Vec<_> = g.all_rules().collect();
    assert_ne!(rules[0].production_id, rules[1].production_id);
}

// ===========================================================================
// 22. ProductionId Copy and comparison semantics
// ===========================================================================

#[test]
fn production_id_copy_semantics() {
    let id = ProductionId(42);
    let id2 = id;
    assert_eq!(id, id2);
}

#[test]
fn production_id_ordering() {
    assert!(ProductionId(0) < ProductionId(1));
    assert!(ProductionId(100) > ProductionId(50));
}

#[test]
fn production_id_hash_in_set() {
    let mut set = HashSet::new();
    set.insert(ProductionId(1));
    set.insert(ProductionId(2));
    set.insert(ProductionId(1));
    assert_eq!(set.len(), 2);
}

// ===========================================================================
// 23. AliasSequence construction and properties
// ===========================================================================

#[test]
fn alias_sequence_empty_construction() {
    let seq = AliasSequence { aliases: vec![] };
    assert!(seq.aliases.is_empty());
}

#[test]
fn alias_sequence_all_some() {
    let seq = AliasSequence {
        aliases: vec![
            Some("a".to_string()),
            Some("b".to_string()),
            Some("c".to_string()),
        ],
    };
    assert!(seq.aliases.iter().all(|a| a.is_some()));
}

#[test]
fn alias_sequence_all_none() {
    let seq = AliasSequence {
        aliases: vec![None, None, None],
    };
    assert!(seq.aliases.iter().all(|a| a.is_none()));
}

#[test]
fn alias_sequence_mixed() {
    let seq = AliasSequence {
        aliases: vec![Some("x".to_string()), None, Some("z".to_string())],
    };
    assert!(seq.aliases[0].is_some());
    assert!(seq.aliases[1].is_none());
    assert!(seq.aliases[2].is_some());
}

#[test]
fn alias_sequence_clone_equality() {
    let seq = AliasSequence {
        aliases: vec![Some("test".to_string()), None],
    };
    let cloned = seq.clone();
    assert_eq!(seq, cloned);
}

// ===========================================================================
// 24. max_alias_sequence_length tracking
// ===========================================================================

#[test]
fn max_alias_tracks_single_entry() {
    let mut g = build_minimal("pi_v10_max_single");
    populate_aliases(&mut g, vec![(0, vec![Some("a".to_string())])]);
    assert_eq!(g.max_alias_sequence_length, 1);
}

#[test]
fn max_alias_tracks_longest_sequence() {
    let mut g = build_minimal("pi_v10_max_longest");
    populate_aliases(
        &mut g,
        vec![(0, vec![None]), (1, vec![None; 5]), (2, vec![None; 3])],
    );
    assert_eq!(g.max_alias_sequence_length, 5);
}

#[test]
fn max_alias_zero_for_empty_alias_vectors() {
    let mut g = build_minimal("pi_v10_max_empty_vecs");
    populate_aliases(&mut g, vec![(0, vec![]), (1, vec![])]);
    assert_eq!(g.max_alias_sequence_length, 0);
}

#[test]
fn max_alias_consistent_with_all_same_size() {
    let mut g = build_minimal("pi_v10_max_same");
    populate_aliases(
        &mut g,
        vec![(0, vec![None; 3]), (1, vec![None; 3]), (2, vec![None; 3])],
    );
    assert_eq!(g.max_alias_sequence_length, 3);
}

// ===========================================================================
// 25. Serde roundtrip
// ===========================================================================

#[test]
fn alias_sequence_serde_roundtrip() {
    let seq = AliasSequence {
        aliases: vec![Some("alias".to_string()), None, Some("other".to_string())],
    };
    let json = serde_json::to_string(&seq).unwrap();
    let restored: AliasSequence = serde_json::from_str(&json).unwrap();
    assert_eq!(seq, restored);
}

#[test]
fn grammar_with_aliases_serde_roundtrip() {
    let mut g = build_minimal("pi_v10_serde_full");
    populate_aliases(
        &mut g,
        vec![
            (0, vec![Some("a".to_string())]),
            (1, vec![None, Some("b".to_string())]),
        ],
    );
    let json = serde_json::to_string(&g).unwrap();
    let restored: Grammar = serde_json::from_str(&json).unwrap();
    assert_eq!(g.alias_sequences, restored.alias_sequences);
    assert_eq!(
        g.max_alias_sequence_length,
        restored.max_alias_sequence_length
    );
}

#[test]
fn production_ids_serde_roundtrip() {
    let mut g = build_minimal("pi_v10_serde_pid");
    populate_prod_ids(&mut g, 5);
    let json = serde_json::to_string(&g).unwrap();
    let restored: Grammar = serde_json::from_str(&json).unwrap();
    assert_eq!(g.production_ids, restored.production_ids);
}

#[test]
fn production_id_json_roundtrip() {
    let id = ProductionId(42);
    let json = serde_json::to_string(&id).unwrap();
    let restored: ProductionId = serde_json::from_str(&json).unwrap();
    assert_eq!(id, restored);
}

// ===========================================================================
// 26. Edge cases — boundary conditions
// ===========================================================================

#[test]
fn alias_sequence_with_empty_string_alias() {
    let mut g = build_minimal("pi_v10_edge_empty_str");
    populate_aliases(&mut g, vec![(0, vec![Some(String::new())])]);
    assert_eq!(
        g.alias_sequences[&ProductionId(0)].aliases[0].as_deref(),
        Some("")
    );
}

#[test]
fn alias_sequence_large_production_id() {
    let mut g = build_minimal("pi_v10_edge_large_pid");
    g.alias_sequences.insert(
        ProductionId(u16::MAX),
        AliasSequence {
            aliases: vec![Some("max".to_string())],
        },
    );
    assert!(g.alias_sequences.contains_key(&ProductionId(u16::MAX)));
}

#[test]
fn production_ids_map_large_rule_id() {
    let mut g = build_minimal("pi_v10_edge_large_rid");
    g.production_ids
        .insert(RuleId(u16::MAX), ProductionId(u16::MAX));
    assert_eq!(g.production_ids[&RuleId(u16::MAX)], ProductionId(u16::MAX));
}

#[test]
fn alias_sequence_many_none_entries() {
    let mut g = build_minimal("pi_v10_edge_many_none");
    populate_aliases(&mut g, vec![(0, vec![None; 50])]);
    assert_eq!(g.max_alias_sequence_length, 50);
    assert!(
        g.alias_sequences[&ProductionId(0)]
            .aliases
            .iter()
            .all(|a| a.is_none())
    );
}

#[test]
fn alias_sequence_remove_then_reinsert() {
    let mut g = build_minimal("pi_v10_edge_reinsert");
    populate_aliases(&mut g, vec![(0, vec![Some("first".to_string())])]);
    g.alias_sequences.shift_remove(&ProductionId(0));
    assert!(g.alias_sequences.is_empty());
    populate_aliases(&mut g, vec![(0, vec![Some("second".to_string())])]);
    assert_eq!(
        g.alias_sequences[&ProductionId(0)].aliases[0].as_deref(),
        Some("second")
    );
}

// ===========================================================================
// 27. production_ids map — insertion order preserved by IndexMap
// ===========================================================================

#[test]
fn production_ids_map_preserves_insertion_order() {
    let mut g = Grammar::new("pi_v10_indexmap_order".to_string());
    g.production_ids.insert(RuleId(5), ProductionId(50));
    g.production_ids.insert(RuleId(2), ProductionId(20));
    g.production_ids.insert(RuleId(8), ProductionId(80));
    let keys: Vec<u16> = g.production_ids.keys().map(|r| r.0).collect();
    assert_eq!(keys, vec![5, 2, 8]);
}

#[test]
fn production_ids_overwrite_on_same_rule_id() {
    let mut g = Grammar::new("pi_v10_indexmap_overwrite".to_string());
    g.production_ids.insert(RuleId(0), ProductionId(1));
    g.production_ids.insert(RuleId(0), ProductionId(99));
    assert_eq!(g.production_ids.len(), 1);
    assert_eq!(g.production_ids[&RuleId(0)], ProductionId(99));
}

// ===========================================================================
// 28. Interaction between alias_sequences and all_rules()
// ===========================================================================

#[test]
fn all_rules_iterator_unaffected_by_alias_sequences() {
    let mut g = build_two_rules("pi_v10_iter_alias");
    let count_before = g.all_rules().count();
    populate_aliases(&mut g, vec![(0, vec![Some("x".to_string())])]);
    let count_after = g.all_rules().count();
    assert_eq!(count_before, count_after);
}

#[test]
fn all_rules_production_ids_can_match_alias_keys() {
    let mut g = build_two_rules("pi_v10_iter_match");
    let rule_pids: Vec<u16> = g.all_rules().map(|r| r.production_id.0).collect();
    for pid in &rule_pids {
        g.alias_sequences.insert(
            ProductionId(*pid),
            AliasSequence {
                aliases: vec![Some(format!("alias_{pid}"))],
            },
        );
    }
    for rule in g.all_rules() {
        assert!(g.alias_sequences.contains_key(&rule.production_id));
    }
}

// ===========================================================================
// 29. Validate with alias data
// ===========================================================================

#[test]
fn validate_succeeds_with_alias_sequences() {
    let mut g = build_minimal("pi_v10_validate_alias");
    populate_aliases(&mut g, vec![(0, vec![Some("ok".to_string())])]);
    assert!(g.validate().is_ok());
}

#[test]
fn validate_succeeds_with_production_ids_map() {
    let mut g = build_minimal("pi_v10_validate_pid");
    populate_prod_ids(&mut g, 3);
    assert!(g.validate().is_ok());
}

#[test]
fn validate_succeeds_empty_alias_sequences() {
    let g = build_minimal("pi_v10_validate_empty");
    assert!(g.validate().is_ok());
}

// ===========================================================================
// 30. Grammar equality with alias data
// ===========================================================================

#[test]
fn grammars_equal_with_same_aliases() {
    let make = || {
        let mut g = Grammar::new("pi_v10_eq_alias".to_string());
        populate_aliases(&mut g, vec![(0, vec![Some("a".to_string())])]);
        g
    };
    assert_eq!(make(), make());
}

#[test]
fn grammars_not_equal_different_aliases() {
    let mut g1 = Grammar::new("pi_v10_ne_alias".to_string());
    let mut g2 = Grammar::new("pi_v10_ne_alias".to_string());
    populate_aliases(&mut g1, vec![(0, vec![Some("x".to_string())])]);
    populate_aliases(&mut g2, vec![(0, vec![Some("y".to_string())])]);
    assert_ne!(g1, g2);
}

#[test]
fn grammars_not_equal_different_max_alias_length() {
    let mut g1 = Grammar::new("pi_v10_ne_max".to_string());
    let mut g2 = Grammar::new("pi_v10_ne_max".to_string());
    g1.max_alias_sequence_length = 3;
    g2.max_alias_sequence_length = 5;
    assert_ne!(g1, g2);
}

// ===========================================================================
// 31. Multiple LHS symbols — globally unique IDs
// ===========================================================================

#[test]
fn multiple_lhs_production_ids_globally_unique() {
    let g = GrammarBuilder::new("pi_v10_multi_lhs")
        .token("a", "a")
        .token("b", "b")
        .rule("start", vec!["a"])
        .rule("start", vec!["b"])
        .rule("other", vec!["a"])
        .start("start")
        .build();
    let ids: HashSet<u16> = g.all_rules().map(|r| r.production_id.0).collect();
    assert_eq!(ids.len(), 3);
}

#[test]
fn same_lhs_different_production_ids() {
    let g = GrammarBuilder::new("pi_v10_same_lhs")
        .token("a", "a")
        .token("b", "b")
        .token("c", "c")
        .rule("start", vec!["a"])
        .rule("start", vec!["b"])
        .rule("start", vec!["c"])
        .start("start")
        .build();
    let ids: HashSet<u16> = g.all_rules().map(|r| r.production_id.0).collect();
    assert_eq!(ids.len(), 3);
}

// ===========================================================================
// 32. Token-only grammar
// ===========================================================================

#[test]
fn token_only_grammar_has_no_production_ids() {
    let g = GrammarBuilder::new("pi_v10_tok_only")
        .token("NUM", r"\d+")
        .token("IDENT", r"[a-z]+")
        .build();
    assert_eq!(g.production_ids.len(), g.all_rules().count());
    assert!(g.alias_sequences.is_empty());
    assert_eq!(g.max_alias_sequence_length, 0);
}

#[test]
fn token_only_grammar_all_rules_empty() {
    let g = GrammarBuilder::new("pi_v10_tok_only_rules")
        .token("NUM", r"\d+")
        .build();
    assert_eq!(g.all_rules().count(), 0);
}

// ===========================================================================
// 33. Empty rule (epsilon) gets a production ID
// ===========================================================================

#[test]
fn empty_rule_gets_production_id() {
    let g = GrammarBuilder::new("pi_v10_empty_rule")
        .rule("empty", vec![])
        .start("empty")
        .build();
    let rules: Vec<_> = g.all_rules().collect();
    assert_eq!(rules.len(), 1);
    assert_eq!(rules[0].production_id, ProductionId(0));
}

// ===========================================================================
// 34. Grammar with conflicts — production ID preservation
// ===========================================================================

#[test]
fn conflict_grammar_alias_sequences_empty() {
    let mut g = build_two_rules("pi_v10_conflict_empty");
    g.conflicts.push(adze_ir::ConflictDeclaration {
        symbols: vec![SymbolId(1)],
        resolution: adze_ir::ConflictResolution::GLR,
    });
    assert!(g.alias_sequences.is_empty());
}

#[test]
fn conflict_grammar_production_ids_intact() {
    let mut g = build_two_rules("pi_v10_conflict_pid");
    g.conflicts.push(adze_ir::ConflictDeclaration {
        symbols: vec![SymbolId(1)],
        resolution: adze_ir::ConflictResolution::GLR,
    });
    let ids: HashSet<u16> = g.all_rules().map(|r| r.production_id.0).collect();
    assert_eq!(ids.len(), 2);
}

// ===========================================================================
// 35. Start symbol unaffected by alias data
// ===========================================================================

#[test]
fn start_symbol_unaffected_by_alias_sequences() {
    let mut g = build_minimal("pi_v10_start_alias");
    let start_before = g.start_symbol();
    populate_aliases(&mut g, vec![(0, vec![Some("renamed".to_string())])]);
    let start_after = g.start_symbol();
    assert_eq!(start_before, start_after);
}

// ===========================================================================
// 36. Grammar with external tokens — production IDs
// ===========================================================================

#[test]
fn external_token_grammar_production_ids() {
    let g = GrammarBuilder::new("pi_v10_external")
        .token("NUM", r"\d+")
        .rule("start", vec!["NUM"])
        .external("INDENT")
        .start("start")
        .build();
    let ids: Vec<ProductionId> = g.all_rules().map(|r| r.production_id).collect();
    assert_eq!(ids.len(), 1);
    assert_eq!(ids[0], ProductionId(0));
}

// ===========================================================================
// 37. Grammar with extras — production IDs
// ===========================================================================

#[test]
fn extra_token_grammar_production_ids() {
    let g = GrammarBuilder::new("pi_v10_extras")
        .token("NUM", r"\d+")
        .token("WS", r"\s+")
        .rule("start", vec!["NUM"])
        .extra("WS")
        .start("start")
        .build();
    let ids: Vec<ProductionId> = g.all_rules().map(|r| r.production_id).collect();
    assert_eq!(ids.len(), 1);
}

// ===========================================================================
// 38. Grammar with supertypes — production IDs
// ===========================================================================

#[test]
fn supertype_grammar_production_ids() {
    let g = GrammarBuilder::new("pi_v10_supertype")
        .token("NUM", r"\d+")
        .token("STR", r#""[^"]*""#)
        .rule("expr", vec!["NUM"])
        .rule("expr", vec!["STR"])
        .supertype("expr")
        .start("expr")
        .build();
    let ids: HashSet<u16> = g.all_rules().map(|r| r.production_id.0).collect();
    assert_eq!(ids.len(), 2);
}

// ===========================================================================
// 39. Large grammar alias population
// ===========================================================================

#[test]
fn large_grammar_20_rules_alias_population() {
    let mut g = build_n_rules("pi_v10_large_20", 20);
    let entries: Vec<(u16, Vec<Option<String>>)> = (0..20)
        .map(|i| (i, vec![Some(format!("alias_{i}"))]))
        .collect();
    populate_aliases(&mut g, entries);
    assert_eq!(g.alias_sequences.len(), 20);
    assert_eq!(g.max_alias_sequence_length, 1);
}

#[test]
fn large_grammar_20_production_ids_map() {
    let mut g = build_n_rules("pi_v10_large_20_pid", 20);
    populate_prod_ids(&mut g, 20);
    assert_eq!(g.production_ids.len(), 20);
}

// ===========================================================================
// 40. Alias sequences with varying lengths
// ===========================================================================

#[test]
fn alias_sequences_varying_lengths() {
    let mut g = build_minimal("pi_v10_vary_len");
    populate_aliases(
        &mut g,
        vec![
            (0, vec![Some("short".to_string())]),
            (1, vec![None, None, Some("mid".to_string())]),
            (2, vec![None, None, None, None, Some("long".to_string())]),
        ],
    );
    assert_eq!(g.max_alias_sequence_length, 5);
    assert_eq!(g.alias_sequences.len(), 3);
}

#[test]
fn alias_sequence_length_consistent_with_max() {
    let mut g = build_minimal("pi_v10_vary_consistent");
    populate_aliases(
        &mut g,
        vec![(0, vec![None; 2]), (1, vec![None; 7]), (2, vec![None; 4])],
    );
    let actual_max = g
        .alias_sequences
        .values()
        .map(|s| s.aliases.len())
        .max()
        .unwrap();
    assert_eq!(g.max_alias_sequence_length, actual_max);
}
