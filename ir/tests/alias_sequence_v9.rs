//! Comprehensive tests for alias_sequences and production_ids in adze-ir Grammar.

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
// 1. Empty grammar — alias_sequences defaults
// ===========================================================================

#[test]
fn empty_grammar_has_empty_alias_sequences() {
    let g = Grammar::new("aaseq_v9_empty".to_string());
    assert!(g.alias_sequences.is_empty());
}

#[test]
fn empty_grammar_has_zero_max_alias_length() {
    let g = Grammar::new("aaseq_v9_empty_max".to_string());
    assert_eq!(g.max_alias_sequence_length, 0);
}

#[test]
fn empty_grammar_has_empty_production_ids() {
    let g = Grammar::new("aaseq_v9_empty_pid".to_string());
    assert!(g.production_ids.is_empty());
}

// ===========================================================================
// 2. Grammar with rules — alias_sequences defaults
// ===========================================================================

#[test]
fn builder_grammar_alias_sequences_empty_by_default() {
    let g = build_two_rules("aaseq_v9_two_rules");
    assert!(g.alias_sequences.is_empty());
}

#[test]
fn builder_grammar_max_alias_length_zero_by_default() {
    let g = build_two_rules("aaseq_v9_two_rules_max");
    assert_eq!(g.max_alias_sequence_length, 0);
}

#[test]
fn builder_grammar_production_ids_map_total_by_default() {
    let g = build_two_rules("aaseq_v9_two_rules_pid");
    assert_eq!(g.production_ids.len(), g.all_rules().count());
}

#[test]
fn builder_rules_have_production_ids_on_rule_structs() {
    let g = build_two_rules("aaseq_v9_rule_pids");
    let ids: Vec<ProductionId> = g.all_rules().map(|r| r.production_id).collect();
    assert!(!ids.is_empty());
}

// ===========================================================================
// 3. production_ids defaults for simple grammar
// ===========================================================================

#[test]
fn simple_grammar_production_ids_start_at_zero() {
    let g = build_minimal("aaseq_v9_simple_pid_zero");
    let min_id = g.all_rules().map(|r| r.production_id.0).min().unwrap();
    assert_eq!(min_id, 0);
}

#[test]
fn simple_grammar_production_ids_are_unique() {
    let g = build_two_rules("aaseq_v9_simple_unique");
    let ids: HashSet<u16> = g.all_rules().map(|r| r.production_id.0).collect();
    assert_eq!(ids.len(), g.all_rules().count());
}

#[test]
fn production_ids_contiguous_for_builder_grammar() {
    let g = build_n_rules("aaseq_v9_contiguous", 5);
    let mut ids: Vec<u16> = g.all_rules().map(|r| r.production_id.0).collect();
    ids.sort();
    for (i, id) in ids.iter().enumerate() {
        assert_eq!(*id, i as u16);
    }
}

// ===========================================================================
// 4. max_alias_sequence_length defaults
// ===========================================================================

#[test]
fn max_alias_length_default_is_zero() {
    let g = build_minimal("aaseq_v9_max_default");
    assert_eq!(g.max_alias_sequence_length, 0);
}

#[test]
fn max_alias_length_after_single_insert() {
    let mut g = build_minimal("aaseq_v9_max_single");
    populate_aliases(&mut g, vec![(0, vec![Some("a".to_string())])]);
    assert_eq!(g.max_alias_sequence_length, 1);
}

#[test]
fn max_alias_length_tracks_longest_sequence() {
    let mut g = build_minimal("aaseq_v9_max_longest");
    populate_aliases(
        &mut g,
        vec![(0, vec![None]), (1, vec![None; 5]), (2, vec![None; 3])],
    );
    assert_eq!(g.max_alias_sequence_length, 5);
}

#[test]
fn max_alias_length_zero_for_empty_sequences() {
    let mut g = build_minimal("aaseq_v9_max_empty_seqs");
    populate_aliases(&mut g, vec![(0, vec![]), (1, vec![])]);
    assert_eq!(g.max_alias_sequence_length, 0);
}

// ===========================================================================
// 5. alias_sequences after normalize()
// ===========================================================================

#[test]
fn normalize_preserves_existing_alias_sequences() {
    let mut g = build_two_rules("aaseq_v9_norm_preserve");
    populate_aliases(&mut g, vec![(0, vec![Some("aliased".to_string())])]);
    let _ = g.normalize();
    assert_eq!(g.alias_sequences.len(), 1);
    assert_eq!(
        g.alias_sequences[&ProductionId(0)].aliases[0].as_deref(),
        Some("aliased")
    );
}

#[test]
fn normalize_preserves_max_alias_length() {
    let mut g = build_two_rules("aaseq_v9_norm_max");
    populate_aliases(&mut g, vec![(0, vec![None; 4])]);
    let _ = g.normalize();
    assert_eq!(g.max_alias_sequence_length, 4);
}

#[test]
fn normalize_empty_alias_sequences_remain_empty() {
    let mut g = build_minimal("aaseq_v9_norm_empty");
    let _ = g.normalize();
    assert!(g.alias_sequences.is_empty());
    assert_eq!(g.max_alias_sequence_length, 0);
}

#[test]
fn normalize_does_not_add_alias_sequences() {
    let mut g = build_two_rules("aaseq_v9_norm_no_add");
    let _ = g.normalize();
    assert!(g.alias_sequences.is_empty());
}

#[test]
fn normalize_preserves_multiple_alias_sequences() {
    let mut g = build_two_rules("aaseq_v9_norm_multi");
    populate_aliases(
        &mut g,
        vec![
            (0, vec![Some("first".to_string())]),
            (1, vec![Some("second".to_string()), None]),
        ],
    );
    let _ = g.normalize();
    assert_eq!(g.alias_sequences.len(), 2);
}

// ===========================================================================
// 6. production_ids after optimize()
// ===========================================================================

#[test]
fn optimize_preserves_production_ids_on_rules() {
    let mut g = build_two_rules("aaseq_v9_opt_pid");
    let ids_before: Vec<ProductionId> = g.all_rules().map(|r| r.production_id).collect();
    g.optimize();
    let ids_after: Vec<ProductionId> = g.all_rules().map(|r| r.production_id).collect();
    assert_eq!(ids_before, ids_after);
}

#[test]
fn optimize_preserves_alias_sequences() {
    let mut g = build_two_rules("aaseq_v9_opt_alias");
    populate_aliases(&mut g, vec![(0, vec![Some("kept".to_string())])]);
    g.optimize();
    assert_eq!(
        g.alias_sequences[&ProductionId(0)].aliases[0].as_deref(),
        Some("kept")
    );
}

#[test]
fn optimize_preserves_max_alias_length() {
    let mut g = build_two_rules("aaseq_v9_opt_max");
    populate_aliases(&mut g, vec![(0, vec![None; 3])]);
    g.optimize();
    assert_eq!(g.max_alias_sequence_length, 3);
}

#[test]
fn optimize_empty_grammar_no_alias_change() {
    let mut g = Grammar::new("aaseq_v9_opt_empty".to_string());
    g.optimize();
    assert!(g.alias_sequences.is_empty());
    assert_eq!(g.max_alias_sequence_length, 0);
}

// ===========================================================================
// 7. max_alias_sequence_length consistency with alias_sequences
// ===========================================================================

#[test]
fn max_length_consistent_after_single_entry() {
    let mut g = build_minimal("aaseq_v9_consist_single");
    populate_aliases(&mut g, vec![(0, vec![Some("x".to_string()), None])]);
    assert_eq!(g.max_alias_sequence_length, 2);
    assert_eq!(
        g.alias_sequences
            .values()
            .map(|s| s.aliases.len())
            .max()
            .unwrap(),
        2
    );
}

#[test]
fn max_length_consistent_after_multiple_entries() {
    let mut g = build_minimal("aaseq_v9_consist_multi");
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
    assert_eq!(actual_max, 7);
}

#[test]
fn max_length_consistent_when_all_same_size() {
    let mut g = build_minimal("aaseq_v9_consist_same");
    populate_aliases(
        &mut g,
        vec![(0, vec![None; 3]), (1, vec![None; 3]), (2, vec![None; 3])],
    );
    assert_eq!(g.max_alias_sequence_length, 3);
}

#[test]
fn max_length_zero_when_no_alias_sequences() {
    let g = build_minimal("aaseq_v9_consist_none");
    assert_eq!(g.max_alias_sequence_length, 0);
    assert!(g.alias_sequences.is_empty());
}

// ===========================================================================
// 8. Grammar with tokens — production_ids populated
// ===========================================================================

#[test]
fn token_only_grammar_has_no_production_ids_map() {
    let g = GrammarBuilder::new("aaseq_v9_tok_only")
        .token("NUM", r"\d+")
        .token("IDENT", r"[a-z]+")
        .build();
    assert!(g.production_ids.is_empty());
}

#[test]
fn grammar_with_tokens_and_rules_has_rule_pids() {
    let g = GrammarBuilder::new("aaseq_v9_tok_rules")
        .token("NUM", r"\d+")
        .token("IDENT", r"[a-z]+")
        .rule("start", vec!["NUM"])
        .rule("start", vec!["IDENT"])
        .start("start")
        .build();
    let count = g.all_rules().count();
    assert_eq!(count, 2);
    let ids: HashSet<u16> = g.all_rules().map(|r| r.production_id.0).collect();
    assert_eq!(ids.len(), 2);
}

#[test]
fn production_ids_populated_manually() {
    let mut g = build_minimal("aaseq_v9_pid_manual");
    populate_prod_ids(&mut g, 3);
    assert_eq!(g.production_ids.len(), 3);
    assert_eq!(g.production_ids[&RuleId(0)], ProductionId(0));
    assert_eq!(g.production_ids[&RuleId(1)], ProductionId(1));
    assert_eq!(g.production_ids[&RuleId(2)], ProductionId(2));
}

#[test]
fn production_ids_map_preserves_insertion_order() {
    let mut g = build_minimal("aaseq_v9_pid_order");
    // Builder grammars arrive with a total map (#980); this test's subject
    // is manual insertion order, so start from an empty map.
    g.production_ids.clear();
    g.production_ids.insert(RuleId(5), ProductionId(50));
    g.production_ids.insert(RuleId(2), ProductionId(20));
    g.production_ids.insert(RuleId(8), ProductionId(80));
    let keys: Vec<u16> = g.production_ids.keys().map(|r| r.0).collect();
    assert_eq!(keys, vec![5, 2, 8]);
}

// ===========================================================================
// 9. Grammar with precedence — alias management
// ===========================================================================

#[test]
fn precedence_grammar_alias_sequences_empty_by_default() {
    let g = GrammarBuilder::new("aaseq_v9_prec_empty")
        .token("NUM", r"\d+")
        .token("+", "+")
        .token("*", "*")
        .rule_with_precedence("expr", vec!["expr", "+", "expr"], 1, Associativity::Left)
        .rule_with_precedence("expr", vec!["expr", "*", "expr"], 2, Associativity::Left)
        .rule("expr", vec!["NUM"])
        .start("expr")
        .build();
    assert!(g.alias_sequences.is_empty());
}

#[test]
fn precedence_grammar_supports_alias_population() {
    let mut g = GrammarBuilder::new("aaseq_v9_prec_pop")
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

#[test]
fn precedence_grammar_production_ids_unique() {
    let g = GrammarBuilder::new("aaseq_v9_prec_unique")
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
fn precedence_grammar_alias_after_optimize() {
    let mut g = GrammarBuilder::new("aaseq_v9_prec_opt")
        .token("NUM", r"\d+")
        .token("+", "+")
        .rule_with_precedence("expr", vec!["expr", "+", "expr"], 1, Associativity::Left)
        .rule("expr", vec!["NUM"])
        .start("expr")
        .build();
    populate_aliases(&mut g, vec![(0, vec![Some("alias".to_string())])]);
    g.optimize();
    assert_eq!(
        g.alias_sequences[&ProductionId(0)].aliases[0].as_deref(),
        Some("alias")
    );
}

// ===========================================================================
// 10. Grammar with conflicts — production_ids
// ===========================================================================

#[test]
fn conflict_grammar_has_empty_alias_sequences() {
    let mut g = build_two_rules("aaseq_v9_conflict_empty");
    g.conflicts.push(adze_ir::ConflictDeclaration {
        symbols: vec![SymbolId(1)],
        resolution: adze_ir::ConflictResolution::GLR,
    });
    assert!(g.alias_sequences.is_empty());
}

#[test]
fn conflict_grammar_production_ids_on_rules_intact() {
    let mut g = build_two_rules("aaseq_v9_conflict_pid");
    g.conflicts.push(adze_ir::ConflictDeclaration {
        symbols: vec![SymbolId(1)],
        resolution: adze_ir::ConflictResolution::GLR,
    });
    let ids: HashSet<u16> = g.all_rules().map(|r| r.production_id.0).collect();
    assert_eq!(ids.len(), 2);
}

#[test]
fn conflict_grammar_supports_alias_population() {
    let mut g = build_two_rules("aaseq_v9_conflict_alias");
    g.conflicts.push(adze_ir::ConflictDeclaration {
        symbols: vec![SymbolId(1)],
        resolution: adze_ir::ConflictResolution::Precedence(adze_ir::PrecedenceKind::Static(1)),
    });
    populate_aliases(&mut g, vec![(0, vec![Some("c".to_string())])]);
    assert_eq!(g.alias_sequences.len(), 1);
}

#[test]
fn conflict_grammar_max_alias_length_updates() {
    let mut g = build_two_rules("aaseq_v9_conflict_max");
    g.conflicts.push(adze_ir::ConflictDeclaration {
        symbols: vec![SymbolId(1)],
        resolution: adze_ir::ConflictResolution::GLR,
    });
    populate_aliases(&mut g, vec![(0, vec![None; 6])]);
    assert_eq!(g.max_alias_sequence_length, 6);
}

// ===========================================================================
// 11. Cloned grammar preserves alias_sequences
// ===========================================================================

#[test]
fn clone_preserves_alias_sequences() {
    let mut g = build_minimal("aaseq_v9_clone_alias");
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
fn clone_preserves_max_alias_length() {
    let mut g = build_minimal("aaseq_v9_clone_max");
    populate_aliases(&mut g, vec![(0, vec![None; 5])]);
    let cloned = g.clone();
    assert_eq!(
        g.max_alias_sequence_length,
        cloned.max_alias_sequence_length
    );
}

#[test]
fn clone_preserves_production_ids_map() {
    let mut g = build_minimal("aaseq_v9_clone_pid");
    populate_prod_ids(&mut g, 4);
    let cloned = g.clone();
    assert_eq!(g.production_ids, cloned.production_ids);
}

#[test]
fn clone_is_independent_of_original() {
    let mut g = build_minimal("aaseq_v9_clone_indep");
    populate_aliases(&mut g, vec![(0, vec![Some("original".to_string())])]);
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

#[test]
fn clone_preserves_rule_production_ids() {
    let g = build_two_rules("aaseq_v9_clone_rpid");
    let cloned = g.clone();
    let orig_pids: Vec<ProductionId> = g.all_rules().map(|r| r.production_id).collect();
    let clone_pids: Vec<ProductionId> = cloned.all_rules().map(|r| r.production_id).collect();
    assert_eq!(orig_pids, clone_pids);
}

// ===========================================================================
// 12. Grammar Debug format includes alias info
// ===========================================================================

#[test]
fn debug_format_includes_alias_sequences_field() {
    let g = build_minimal("aaseq_v9_debug_alias");
    let dbg = format!("{g:?}");
    assert!(dbg.contains("alias_sequences"));
}

#[test]
fn debug_format_includes_production_ids_field() {
    let g = build_minimal("aaseq_v9_debug_pid");
    let dbg = format!("{g:?}");
    assert!(dbg.contains("production_ids"));
}

#[test]
fn debug_format_includes_max_alias_length() {
    let g = build_minimal("aaseq_v9_debug_max");
    let dbg = format!("{g:?}");
    assert!(dbg.contains("max_alias_sequence_length"));
}

#[test]
fn debug_format_shows_populated_aliases() {
    let mut g = build_minimal("aaseq_v9_debug_pop");
    populate_aliases(&mut g, vec![(0, vec![Some("visible_alias".to_string())])]);
    let dbg = format!("{g:?}");
    assert!(dbg.contains("visible_alias"));
}

#[test]
fn debug_format_shows_production_id_values() {
    let mut g = build_minimal("aaseq_v9_debug_pidv");
    populate_prod_ids(&mut g, 1);
    let dbg = format!("{g:?}");
    assert!(dbg.contains("ProductionId"));
}

// ===========================================================================
// 13. Multiple rules — production_ids ordering
// ===========================================================================

#[test]
fn multiple_rules_production_ids_ascending() {
    let g = build_n_rules("aaseq_v9_multi_asc", 10);
    let ids: Vec<u16> = g.all_rules().map(|r| r.production_id.0).collect();
    for window in ids.windows(2) {
        assert!(window[0] < window[1]);
    }
}

#[test]
fn multiple_rules_same_lhs_different_pids() {
    let g = GrammarBuilder::new("aaseq_v9_same_lhs")
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

#[test]
fn multiple_lhs_production_ids_globally_unique() {
    let g = GrammarBuilder::new("aaseq_v9_multi_lhs")
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
fn production_ids_map_can_have_non_sequential_values() {
    let mut g = build_minimal("aaseq_v9_non_seq");
    g.production_ids.insert(RuleId(0), ProductionId(10));
    g.production_ids.insert(RuleId(1), ProductionId(20));
    g.production_ids.insert(RuleId(2), ProductionId(30));
    assert_eq!(g.production_ids[&RuleId(1)], ProductionId(20));
}

#[test]
fn production_ids_overwrite_on_same_rule_id() {
    let mut g = build_minimal("aaseq_v9_overwrite");
    g.production_ids.insert(RuleId(0), ProductionId(1));
    g.production_ids.insert(RuleId(0), ProductionId(99));
    assert_eq!(g.production_ids.len(), 1);
    assert_eq!(g.production_ids[&RuleId(0)], ProductionId(99));
}

// ===========================================================================
// 14. Single rule — alias state
// ===========================================================================

#[test]
fn single_rule_grammar_alias_empty() {
    let g = build_minimal("aaseq_v9_single_empty");
    assert!(g.alias_sequences.is_empty());
}

#[test]
fn single_rule_grammar_one_production_id() {
    let g = build_minimal("aaseq_v9_single_pid");
    assert_eq!(g.all_rules().count(), 1);
    assert_eq!(g.all_rules().next().unwrap().production_id, ProductionId(0));
}

#[test]
fn single_rule_alias_population() {
    let mut g = build_minimal("aaseq_v9_single_pop");
    populate_aliases(&mut g, vec![(0, vec![Some("only_alias".to_string())])]);
    assert_eq!(g.alias_sequences.len(), 1);
    assert_eq!(g.max_alias_sequence_length, 1);
}

#[test]
fn single_rule_alias_with_none_entries() {
    let mut g = build_minimal("aaseq_v9_single_none");
    populate_aliases(&mut g, vec![(0, vec![None])]);
    assert!(g.alias_sequences[&ProductionId(0)].aliases[0].is_none());
}

#[test]
fn single_rule_normalize_preserves_alias() {
    let mut g = build_minimal("aaseq_v9_single_norm");
    populate_aliases(&mut g, vec![(0, vec![Some("kept".to_string())])]);
    let _ = g.normalize();
    assert_eq!(
        g.alias_sequences[&ProductionId(0)].aliases[0].as_deref(),
        Some("kept")
    );
}

// ===========================================================================
// 15. Various grammar sizes — 1 to 20 rules
// ===========================================================================

#[test]
fn grammar_size_1_rule() {
    let g = build_n_rules("aaseq_v9_size_1", 1);
    assert_eq!(g.all_rules().count(), 1);
    assert!(g.alias_sequences.is_empty());
}

#[test]
fn grammar_size_2_rules() {
    let g = build_n_rules("aaseq_v9_size_2", 2);
    assert_eq!(g.all_rules().count(), 2);
    let ids: HashSet<u16> = g.all_rules().map(|r| r.production_id.0).collect();
    assert_eq!(ids.len(), 2);
}

#[test]
fn grammar_size_5_rules() {
    let g = build_n_rules("aaseq_v9_size_5", 5);
    assert_eq!(g.all_rules().count(), 5);
}

#[test]
fn grammar_size_10_rules() {
    let g = build_n_rules("aaseq_v9_size_10", 10);
    assert_eq!(g.all_rules().count(), 10);
    let ids: HashSet<u16> = g.all_rules().map(|r| r.production_id.0).collect();
    assert_eq!(ids.len(), 10);
}

#[test]
fn grammar_size_15_rules() {
    let g = build_n_rules("aaseq_v9_size_15", 15);
    assert_eq!(g.all_rules().count(), 15);
}

#[test]
fn grammar_size_20_rules() {
    let g = build_n_rules("aaseq_v9_size_20", 20);
    assert_eq!(g.all_rules().count(), 20);
    let ids: HashSet<u16> = g.all_rules().map(|r| r.production_id.0).collect();
    assert_eq!(ids.len(), 20);
}

#[test]
fn grammar_size_20_alias_population() {
    let mut g = build_n_rules("aaseq_v9_size_20_alias", 20);
    let entries: Vec<(u16, Vec<Option<String>>)> = (0..20)
        .map(|i| (i, vec![Some(format!("alias_{i}"))]))
        .collect();
    populate_aliases(&mut g, entries);
    assert_eq!(g.alias_sequences.len(), 20);
    assert_eq!(g.max_alias_sequence_length, 1);
}

#[test]
fn grammar_size_20_production_ids_map() {
    let mut g = build_n_rules("aaseq_v9_size_20_pid", 20);
    populate_prod_ids(&mut g, 20);
    assert_eq!(g.production_ids.len(), 20);
}

// ===========================================================================
// 16. AliasSequence struct — construction and properties
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
fn alias_sequence_mixed_some_none() {
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

#[test]
fn alias_sequence_debug_format() {
    let seq = AliasSequence {
        aliases: vec![Some("dbg".to_string())],
    };
    let dbg = format!("{seq:?}");
    assert!(dbg.contains("dbg"));
}

// ===========================================================================
// 17. Alias sequences with serde roundtrip
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
    let mut g = build_minimal("aaseq_v9_serde_full");
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
    let mut g = build_minimal("aaseq_v9_serde_pid");
    populate_prod_ids(&mut g, 5);
    let json = serde_json::to_string(&g).unwrap();
    let restored: Grammar = serde_json::from_str(&json).unwrap();
    assert_eq!(g.production_ids, restored.production_ids);
}

// ===========================================================================
// 18. Edge cases — alias_sequences boundary conditions
// ===========================================================================

#[test]
fn alias_sequence_with_empty_string_alias() {
    let mut g = build_minimal("aaseq_v9_edge_empty_str");
    populate_aliases(&mut g, vec![(0, vec![Some(String::new())])]);
    assert_eq!(
        g.alias_sequences[&ProductionId(0)].aliases[0].as_deref(),
        Some("")
    );
}

#[test]
fn alias_sequence_with_long_alias_name() {
    let mut g = build_minimal("aaseq_v9_edge_long");
    let long_name = "a".repeat(1000);
    populate_aliases(&mut g, vec![(0, vec![Some(long_name.clone())])]);
    assert_eq!(
        g.alias_sequences[&ProductionId(0)].aliases[0].as_deref(),
        Some(long_name.as_str())
    );
}

#[test]
fn alias_sequence_large_production_id() {
    let mut g = build_minimal("aaseq_v9_edge_large_pid");
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
    let mut g = build_minimal("aaseq_v9_edge_large_rid");
    g.production_ids
        .insert(RuleId(u16::MAX), ProductionId(u16::MAX));
    assert_eq!(g.production_ids[&RuleId(u16::MAX)], ProductionId(u16::MAX));
}

#[test]
fn alias_sequence_remove_then_reinsert() {
    let mut g = build_minimal("aaseq_v9_edge_reinsert");
    populate_aliases(&mut g, vec![(0, vec![Some("first".to_string())])]);
    g.alias_sequences.shift_remove(&ProductionId(0));
    assert!(g.alias_sequences.is_empty());
    populate_aliases(&mut g, vec![(0, vec![Some("second".to_string())])]);
    assert_eq!(
        g.alias_sequences[&ProductionId(0)].aliases[0].as_deref(),
        Some("second")
    );
}

#[test]
fn alias_sequence_many_none_entries() {
    let mut g = build_minimal("aaseq_v9_edge_many_none");
    populate_aliases(&mut g, vec![(0, vec![None; 50])]);
    assert_eq!(g.max_alias_sequence_length, 50);
    assert!(
        g.alias_sequences[&ProductionId(0)]
            .aliases
            .iter()
            .all(|a| a.is_none())
    );
}

// ===========================================================================
// 19. Interaction between alias_sequences and all_rules()
// ===========================================================================

#[test]
fn all_rules_iterator_not_affected_by_alias_sequences() {
    let mut g = build_two_rules("aaseq_v9_iter_alias");
    let count_before = g.all_rules().count();
    populate_aliases(&mut g, vec![(0, vec![Some("x".to_string())])]);
    let count_after = g.all_rules().count();
    assert_eq!(count_before, count_after);
}

#[test]
fn all_rules_production_ids_match_alias_keys() {
    let mut g = build_two_rules("aaseq_v9_iter_match");
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
// 20. Interaction between alias_sequences and start_symbol()
// ===========================================================================

#[test]
fn start_symbol_unaffected_by_alias_sequences() {
    let mut g = build_minimal("aaseq_v9_start_alias");
    let start_before = g.start_symbol();
    populate_aliases(&mut g, vec![(0, vec![Some("renamed".to_string())])]);
    let start_after = g.start_symbol();
    assert_eq!(start_before, start_after);
}

#[test]
fn start_symbol_exists_with_populated_aliases() {
    let mut g = build_minimal("aaseq_v9_start_exists");
    populate_aliases(&mut g, vec![(0, vec![Some("x".to_string())])]);
    assert!(g.start_symbol().is_some());
}

// ===========================================================================
// 21. Interaction between alias_sequences and validate()
// ===========================================================================

#[test]
fn validate_succeeds_with_alias_sequences() {
    let mut g = build_minimal("aaseq_v9_validate_alias");
    populate_aliases(&mut g, vec![(0, vec![Some("ok".to_string())])]);
    assert!(g.validate().is_ok());
}

#[test]
fn validate_succeeds_with_production_ids_map() {
    let mut g = build_minimal("aaseq_v9_validate_pid");
    populate_prod_ids(&mut g, 3);
    assert!(g.validate().is_ok());
}

#[test]
fn validate_succeeds_empty_alias_sequences() {
    let g = build_minimal("aaseq_v9_validate_empty");
    assert!(g.validate().is_ok());
}

// ===========================================================================
// 22. Grammar::new() constructor defaults
// ===========================================================================

#[test]
fn grammar_new_alias_sequences_is_indexmap() {
    let g = Grammar::new("aaseq_v9_new_indexmap".to_string());
    // IndexMap is empty, supports insertion-order iteration
    assert!(g.alias_sequences.is_empty());
    assert!(g.production_ids.is_empty());
}

#[test]
fn grammar_new_name_preserved() {
    let g = Grammar::new("aaseq_v9_new_name".to_string());
    assert_eq!(g.name, "aaseq_v9_new_name");
}

// ===========================================================================
// 23. Grammar equality with alias_sequences
// ===========================================================================

#[test]
fn grammars_equal_with_same_aliases() {
    let make = || {
        let mut g = Grammar::new("aaseq_v9_eq".to_string());
        populate_aliases(&mut g, vec![(0, vec![Some("a".to_string())])]);
        g
    };
    assert_eq!(make(), make());
}

#[test]
fn grammars_not_equal_different_aliases() {
    let mut g1 = Grammar::new("aaseq_v9_neq".to_string());
    let mut g2 = Grammar::new("aaseq_v9_neq".to_string());
    populate_aliases(&mut g1, vec![(0, vec![Some("a".to_string())])]);
    populate_aliases(&mut g2, vec![(0, vec![Some("b".to_string())])]);
    assert_ne!(g1.alias_sequences, g2.alias_sequences);
}

#[test]
fn grammars_not_equal_different_max_length() {
    let mut g1 = Grammar::new("aaseq_v9_neq_max".to_string());
    let mut g2 = Grammar::new("aaseq_v9_neq_max".to_string());
    populate_aliases(&mut g1, vec![(0, vec![None; 3])]);
    populate_aliases(&mut g2, vec![(0, vec![None; 7])]);
    assert_ne!(g1.max_alias_sequence_length, g2.max_alias_sequence_length);
}

// ===========================================================================
// 24. External tokens and aliases
// ===========================================================================

#[test]
fn grammar_with_externals_alias_empty() {
    let g = GrammarBuilder::new("aaseq_v9_ext_empty")
        .token("a", "a")
        .external("INDENT")
        .rule("start", vec!["a"])
        .start("start")
        .build();
    assert!(g.alias_sequences.is_empty());
}

#[test]
fn grammar_with_externals_supports_aliases() {
    let mut g = GrammarBuilder::new("aaseq_v9_ext_alias")
        .token("a", "a")
        .external("INDENT")
        .rule("start", vec!["a"])
        .start("start")
        .build();
    populate_aliases(&mut g, vec![(0, vec![Some("ext_alias".to_string())])]);
    assert_eq!(g.alias_sequences.len(), 1);
}

// ===========================================================================
// 25. Extras and aliases
// ===========================================================================

#[test]
fn grammar_with_extras_alias_empty() {
    let g = GrammarBuilder::new("aaseq_v9_extras_empty")
        .token("NUM", r"\d+")
        .token("WS", r"\s+")
        .extra("WS")
        .rule("start", vec!["NUM"])
        .start("start")
        .build();
    assert!(g.alias_sequences.is_empty());
}

#[test]
fn grammar_with_extras_supports_alias_population() {
    let mut g = GrammarBuilder::new("aaseq_v9_extras_pop")
        .token("NUM", r"\d+")
        .token("WS", r"\s+")
        .extra("WS")
        .rule("start", vec!["NUM"])
        .start("start")
        .build();
    populate_aliases(&mut g, vec![(0, vec![Some("num_alias".to_string())])]);
    assert_eq!(g.max_alias_sequence_length, 1);
}

// ===========================================================================
// 26. Inline rules and aliases
// ===========================================================================

#[test]
fn grammar_with_inline_rules_alias_empty() {
    let g = GrammarBuilder::new("aaseq_v9_inline_empty")
        .token("a", "a")
        .rule("start", vec!["helper"])
        .rule("helper", vec!["a"])
        .inline("helper")
        .start("start")
        .build();
    assert!(g.alias_sequences.is_empty());
}

#[test]
fn grammar_with_inline_rules_production_ids() {
    let g = GrammarBuilder::new("aaseq_v9_inline_pid")
        .token("a", "a")
        .rule("start", vec!["helper"])
        .rule("helper", vec!["a"])
        .inline("helper")
        .start("start")
        .build();
    let ids: HashSet<u16> = g.all_rules().map(|r| r.production_id.0).collect();
    assert_eq!(ids.len(), 2);
}

// ===========================================================================
// 27. Supertypes and aliases
// ===========================================================================

#[test]
fn grammar_with_supertypes_alias_empty() {
    let g = GrammarBuilder::new("aaseq_v9_super_empty")
        .token("a", "a")
        .token("b", "b")
        .rule("start", vec!["item"])
        .rule("item", vec!["a"])
        .rule("item", vec!["b"])
        .supertype("item")
        .start("start")
        .build();
    assert!(g.alias_sequences.is_empty());
}

#[test]
fn grammar_with_supertypes_supports_aliases() {
    let mut g = GrammarBuilder::new("aaseq_v9_super_alias")
        .token("a", "a")
        .rule("start", vec!["item"])
        .rule("item", vec!["a"])
        .supertype("item")
        .start("start")
        .build();
    populate_aliases(&mut g, vec![(0, vec![Some("super_alias".to_string())])]);
    assert_eq!(g.alias_sequences.len(), 1);
}

// ===========================================================================
// 28. Multiple normalize() calls
// ===========================================================================

#[test]
fn double_normalize_preserves_aliases() {
    let mut g = build_minimal("aaseq_v9_double_norm");
    populate_aliases(&mut g, vec![(0, vec![Some("stable".to_string())])]);
    let _ = g.normalize();
    let _ = g.normalize();
    assert_eq!(
        g.alias_sequences[&ProductionId(0)].aliases[0].as_deref(),
        Some("stable")
    );
}

#[test]
fn double_normalize_preserves_max_length() {
    let mut g = build_minimal("aaseq_v9_double_norm_max");
    populate_aliases(&mut g, vec![(0, vec![None; 3])]);
    let _ = g.normalize();
    let _ = g.normalize();
    assert_eq!(g.max_alias_sequence_length, 3);
}

// ===========================================================================
// 29. Default trait
// ===========================================================================

#[test]
fn default_grammar_alias_sequences_empty() {
    let g = Grammar::default();
    assert!(g.alias_sequences.is_empty());
    assert_eq!(g.max_alias_sequence_length, 0);
    assert!(g.production_ids.is_empty());
}

// ===========================================================================
// 30. Alias sequence iteration patterns
// ===========================================================================

#[test]
fn iterate_alias_sequences_by_production_id() {
    let mut g = build_minimal("aaseq_v9_iter_by_pid");
    populate_aliases(
        &mut g,
        vec![
            (0, vec![Some("first".to_string())]),
            (1, vec![Some("second".to_string())]),
            (2, vec![Some("third".to_string())]),
        ],
    );
    let names: Vec<&str> = g
        .alias_sequences
        .values()
        .filter_map(|s| s.aliases.first()?.as_deref())
        .collect();
    assert_eq!(names, vec!["first", "second", "third"]);
}

#[test]
fn count_non_none_aliases_in_sequence() {
    let seq = AliasSequence {
        aliases: vec![Some("a".to_string()), None, Some("c".to_string()), None],
    };
    let non_none = seq.aliases.iter().filter(|a| a.is_some()).count();
    assert_eq!(non_none, 2);
}

#[test]
fn alias_sequences_keys_are_production_ids() {
    let mut g = build_minimal("aaseq_v9_keys_pid");
    populate_aliases(
        &mut g,
        vec![
            (10, vec![Some("ten".to_string())]),
            (20, vec![Some("twenty".to_string())]),
        ],
    );
    let keys: Vec<ProductionId> = g.alias_sequences.keys().copied().collect();
    assert_eq!(keys, vec![ProductionId(10), ProductionId(20)]);
}
