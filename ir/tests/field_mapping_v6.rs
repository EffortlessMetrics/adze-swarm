//! Field mapping v6 tests for adze-ir.
//!
//! Covers: Grammar fields, production IDs, alias sequences,
//! max_alias_sequence_length, preservation through normalize/optimize,
//! and edge cases.

use adze_ir::builder::GrammarBuilder;
use adze_ir::{AliasSequence, FieldId, Grammar, ProductionId, Rule, RuleId, Symbol, SymbolId};
use indexmap::IndexMap;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn minimal_grammar() -> Grammar {
    GrammarBuilder::new("minimal")
        .token("A", "a")
        .rule("start", vec!["A"])
        .start("start")
        .build()
}

fn expr_grammar() -> Grammar {
    GrammarBuilder::new("expr")
        .token("NUMBER", r"\d+")
        .token("PLUS", r"\+")
        .rule("expr", vec!["expr", "PLUS", "expr"])
        .rule("expr", vec!["NUMBER"])
        .start("expr")
        .build()
}

fn grammar_with_fields() -> Grammar {
    let mut g = expr_grammar();
    g.fields.insert(FieldId(0), "left".to_string());
    g.fields.insert(FieldId(1), "operator".to_string());
    g.fields.insert(FieldId(2), "right".to_string());
    g
}

fn grammar_with_alias_sequences() -> Grammar {
    let mut g = expr_grammar();
    g.alias_sequences.insert(
        ProductionId(0),
        AliasSequence {
            aliases: vec![Some("lhs".to_string()), None, Some("rhs".to_string())],
        },
    );
    g.alias_sequences.insert(
        ProductionId(1),
        AliasSequence {
            aliases: vec![Some("literal".to_string())],
        },
    );
    g.max_alias_sequence_length = 3;
    g
}

fn grammar_with_production_ids() -> Grammar {
    let mut g = expr_grammar();
    g.production_ids.insert(RuleId(0), ProductionId(0));
    g.production_ids.insert(RuleId(1), ProductionId(1));
    g.production_ids.insert(RuleId(2), ProductionId(0));
    g
}

fn grammar_with_rule_fields() -> Grammar {
    let mut g = grammar_with_fields();
    let expr_id = g.find_symbol_by_name("expr").unwrap();
    if let Some(rules) = g.rules.get_mut(&expr_id) {
        for rule in rules.iter_mut() {
            if rule.rhs.len() == 3 {
                rule.fields = vec![(FieldId(0), 0), (FieldId(1), 1), (FieldId(2), 2)];
            }
        }
    }
    g
}

// ===========================================================================
// 1. Grammar with no fields (8 tests)
// ===========================================================================

#[test]
fn test_no_fields_builder_produces_empty_fields() {
    let g = minimal_grammar();
    assert!(g.fields.is_empty());
}

#[test]
fn test_no_fields_expr_grammar_has_empty_fields() {
    let g = expr_grammar();
    assert!(g.fields.is_empty());
}

#[test]
fn test_no_fields_alias_sequences_empty_by_default() {
    let g = minimal_grammar();
    assert!(g.alias_sequences.is_empty());
}

#[test]
fn test_no_fields_production_ids_empty_by_default() {
    let g = minimal_grammar();
    assert_eq!(g.production_ids.len(), g.all_rules().count());
}

#[test]
fn test_no_fields_max_alias_sequence_length_zero() {
    let g = minimal_grammar();
    assert_eq!(g.max_alias_sequence_length, 0);
}

#[test]
fn test_no_fields_rules_still_have_empty_field_vecs() {
    let g = expr_grammar();
    for rule in g.all_rules() {
        assert!(rule.fields.is_empty());
    }
}

#[test]
fn test_no_fields_default_grammar_has_empty_fields() {
    let g = Grammar::default();
    assert!(g.fields.is_empty());
    assert!(g.alias_sequences.is_empty());
    assert!(g.production_ids.is_empty());
}

#[test]
fn test_no_fields_grammar_still_has_rules_and_tokens() {
    let g = expr_grammar();
    assert!(g.fields.is_empty());
    assert!(!g.rules.is_empty());
    assert!(!g.tokens.is_empty());
}

// ===========================================================================
// 2. Grammar with explicit fields (8 tests)
// ===========================================================================

#[test]
fn test_explicit_fields_inserted_correctly() {
    let g = grammar_with_fields();
    assert_eq!(g.fields.len(), 3);
}

#[test]
fn test_explicit_fields_lookup_by_id() {
    let g = grammar_with_fields();
    assert_eq!(g.fields[&FieldId(0)], "left");
    assert_eq!(g.fields[&FieldId(1)], "operator");
    assert_eq!(g.fields[&FieldId(2)], "right");
}

#[test]
fn test_explicit_fields_preserves_insertion_order() {
    let g = grammar_with_fields();
    let names: Vec<&str> = g.fields.values().map(|s| s.as_str()).collect();
    assert_eq!(names, vec!["left", "operator", "right"]);
}

#[test]
fn test_explicit_fields_ids_are_sequential() {
    let g = grammar_with_fields();
    let ids: Vec<u16> = g.fields.keys().map(|fid| fid.0).collect();
    assert_eq!(ids, vec![0, 1, 2]);
}

#[test]
fn test_explicit_fields_rule_field_mapping() {
    let g = grammar_with_rule_fields();
    let expr_id = g.find_symbol_by_name("expr").unwrap();
    let rules = g.get_rules_for_symbol(expr_id).unwrap();
    let ternary = rules.iter().find(|r| r.rhs.len() == 3).unwrap();
    assert_eq!(ternary.fields.len(), 3);
    assert_eq!(ternary.fields[0], (FieldId(0), 0));
    assert_eq!(ternary.fields[2], (FieldId(2), 2));
}

#[test]
fn test_explicit_fields_single_rhs_rule_has_no_fields() {
    let g = grammar_with_rule_fields();
    let expr_id = g.find_symbol_by_name("expr").unwrap();
    let rules = g.get_rules_for_symbol(expr_id).unwrap();
    let unary = rules.iter().find(|r| r.rhs.len() == 1).unwrap();
    assert!(unary.fields.is_empty());
}

#[test]
fn test_explicit_fields_replace_on_same_id() {
    let mut g = minimal_grammar();
    g.fields.insert(FieldId(0), "old".to_string());
    g.fields.insert(FieldId(0), "new".to_string());
    assert_eq!(g.fields.len(), 1);
    assert_eq!(g.fields[&FieldId(0)], "new");
}

#[test]
fn test_explicit_fields_noncontiguous_ids() {
    let mut g = minimal_grammar();
    g.fields.insert(FieldId(0), "first".to_string());
    g.fields.insert(FieldId(10), "tenth".to_string());
    g.fields.insert(FieldId(100), "hundredth".to_string());
    assert_eq!(g.fields.len(), 3);
    assert!(g.fields.contains_key(&FieldId(10)));
}

// ===========================================================================
// 3. Field preservation through normalize (8 tests)
// ===========================================================================

#[test]
fn test_normalize_preserves_field_count() {
    let mut g = grammar_with_fields();
    let count_before = g.fields.len();
    let _ = g.normalize();
    assert_eq!(g.fields.len(), count_before);
}

#[test]
fn test_normalize_preserves_field_names() {
    let mut g = grammar_with_fields();
    let _ = g.normalize();
    assert_eq!(g.fields[&FieldId(0)], "left");
    assert_eq!(g.fields[&FieldId(1)], "operator");
    assert_eq!(g.fields[&FieldId(2)], "right");
}

#[test]
fn test_normalize_preserves_field_ids() {
    let mut g = grammar_with_fields();
    let ids_before: Vec<FieldId> = g.fields.keys().copied().collect();
    let _ = g.normalize();
    let ids_after: Vec<FieldId> = g.fields.keys().copied().collect();
    assert_eq!(ids_before, ids_after);
}

#[test]
fn test_normalize_preserves_alias_sequences() {
    let mut g = grammar_with_alias_sequences();
    let seqs_before = g.alias_sequences.len();
    let _ = g.normalize();
    assert_eq!(g.alias_sequences.len(), seqs_before);
}

#[test]
fn test_normalize_preserves_production_ids_map() {
    let mut g = grammar_with_production_ids();
    let _ = g.normalize();
    // #980: production_ids is a derived total map, rebuilt at this boundary.
    assert_eq!(g.production_ids.len(), g.all_rules().count());
}

#[test]
fn test_normalize_preserves_max_alias_sequence_length() {
    let mut g = grammar_with_alias_sequences();
    let max_before = g.max_alias_sequence_length;
    let _ = g.normalize();
    assert_eq!(g.max_alias_sequence_length, max_before);
}

#[test]
fn test_normalize_empty_fields_remain_empty() {
    let mut g = expr_grammar();
    assert!(g.fields.is_empty());
    let _ = g.normalize();
    assert!(g.fields.is_empty());
}

#[test]
fn test_normalize_preserves_rule_field_positions() {
    let mut g = grammar_with_rule_fields();
    let _ = g.normalize();
    // Fields on rules are preserved (normalize operates on symbol structure)
    let expr_id = g.find_symbol_by_name("expr").unwrap();
    if let Some(rules) = g.get_rules_for_symbol(expr_id) {
        let ternary = rules.iter().find(|r| r.rhs.len() == 3);
        if let Some(rule) = ternary {
            assert_eq!(rule.fields.len(), 3);
        }
    }
}

// ===========================================================================
// 4. Field preservation through optimize (8 tests)
// ===========================================================================

#[test]
fn test_optimize_preserves_field_count() {
    let mut g = grammar_with_fields();
    let count_before = g.fields.len();
    g.optimize();
    assert_eq!(g.fields.len(), count_before);
}

#[test]
fn test_optimize_preserves_field_names() {
    let mut g = grammar_with_fields();
    g.optimize();
    assert_eq!(g.fields[&FieldId(0)], "left");
    assert_eq!(g.fields[&FieldId(1)], "operator");
    assert_eq!(g.fields[&FieldId(2)], "right");
}

#[test]
fn test_optimize_preserves_field_ids() {
    let mut g = grammar_with_fields();
    let ids_before: Vec<FieldId> = g.fields.keys().copied().collect();
    g.optimize();
    let ids_after: Vec<FieldId> = g.fields.keys().copied().collect();
    assert_eq!(ids_before, ids_after);
}

#[test]
fn test_optimize_preserves_alias_sequences() {
    let mut g = grammar_with_alias_sequences();
    let seqs_before = g.alias_sequences.len();
    g.optimize();
    assert_eq!(g.alias_sequences.len(), seqs_before);
}

#[test]
fn test_optimize_preserves_production_ids() {
    let mut g = grammar_with_production_ids();
    g.optimize();
    // #980: production_ids is a derived total map, rebuilt at this boundary.
    assert_eq!(g.production_ids.len(), g.all_rules().count());
}

#[test]
fn test_optimize_preserves_max_alias_sequence_length() {
    let mut g = grammar_with_alias_sequences();
    let max_before = g.max_alias_sequence_length;
    g.optimize();
    assert_eq!(g.max_alias_sequence_length, max_before);
}

#[test]
fn test_optimize_empty_fields_remain_empty() {
    let mut g = expr_grammar();
    assert!(g.fields.is_empty());
    g.optimize();
    assert!(g.fields.is_empty());
}

#[test]
fn test_optimize_preserves_rule_field_positions() {
    let mut g = grammar_with_rule_fields();
    g.optimize();
    let expr_id = g.find_symbol_by_name("expr").unwrap();
    if let Some(rules) = g.get_rules_for_symbol(expr_id) {
        let ternary = rules.iter().find(|r| r.rhs.len() == 3);
        if let Some(rule) = ternary {
            assert_eq!(rule.fields.len(), 3);
        }
    }
}

// ===========================================================================
// 5. Production ID assignment (8 tests)
// ===========================================================================

#[test]
fn test_production_id_builder_assigns_sequential_ids() {
    let g = expr_grammar();
    let ids: Vec<ProductionId> = g.all_rules().map(|r| r.production_id).collect();
    for (i, id) in ids.iter().enumerate() {
        assert_eq!(id.0, i as u16);
    }
}

#[test]
fn test_production_id_map_insert_and_lookup() {
    let mut g = minimal_grammar();
    g.production_ids.insert(RuleId(0), ProductionId(42));
    assert_eq!(g.production_ids[&RuleId(0)], ProductionId(42));
}

#[test]
fn test_production_id_map_multiple_rules_same_production() {
    let g = grammar_with_production_ids();
    // RuleId(0) and RuleId(2) both map to ProductionId(0)
    assert_eq!(g.production_ids[&RuleId(0)], ProductionId(0));
    assert_eq!(g.production_ids[&RuleId(2)], ProductionId(0));
}

#[test]
fn test_production_id_map_different_rules_different_productions() {
    let g = grammar_with_production_ids();
    assert_ne!(g.production_ids[&RuleId(0)], g.production_ids[&RuleId(1)]);
}

#[test]
fn test_production_id_on_rule_struct() {
    let rule = Rule {
        lhs: SymbolId(1),
        rhs: vec![Symbol::Terminal(SymbolId(2))],
        precedence: None,
        associativity: None,
        fields: Vec::new(),
        production_id: ProductionId(99),
    };
    assert_eq!(rule.production_id, ProductionId(99));
}

#[test]
fn test_production_id_copy_semantics() {
    let pid = ProductionId(7);
    let copied = pid;
    assert_eq!(pid, copied);
    assert_eq!(pid.0, 7);
}

#[test]
fn test_production_id_distinct_values() {
    let a = ProductionId(0);
    let b = ProductionId(1);
    let c = ProductionId(u16::MAX);
    assert_ne!(a, b);
    assert_ne!(b, c);
    assert_ne!(a, c);
}

#[test]
fn test_production_id_map_overwrite() {
    let mut g = minimal_grammar();
    g.production_ids.insert(RuleId(0), ProductionId(1));
    g.production_ids.insert(RuleId(0), ProductionId(2));
    assert_eq!(g.production_ids[&RuleId(0)], ProductionId(2));
    assert_eq!(g.production_ids.len(), g.all_rules().count());
}

// ===========================================================================
// 6. Alias sequence construction (8 tests)
// ===========================================================================

#[test]
fn test_alias_sequence_single_alias() {
    let seq = AliasSequence {
        aliases: vec![Some("name".to_string())],
    };
    assert_eq!(seq.aliases.len(), 1);
    assert_eq!(seq.aliases[0].as_deref(), Some("name"));
}

#[test]
fn test_alias_sequence_with_none_positions() {
    let seq = AliasSequence {
        aliases: vec![None, Some("op".to_string()), None],
    };
    assert!(seq.aliases[0].is_none());
    assert_eq!(seq.aliases[1].as_deref(), Some("op"));
    assert!(seq.aliases[2].is_none());
}

#[test]
fn test_alias_sequence_all_none() {
    let seq = AliasSequence {
        aliases: vec![None, None, None],
    };
    assert!(seq.aliases.iter().all(|a| a.is_none()));
}

#[test]
fn test_alias_sequence_all_some() {
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
fn test_alias_sequence_in_grammar() {
    let g = grammar_with_alias_sequences();
    assert_eq!(g.alias_sequences.len(), 2);
    let seq0 = &g.alias_sequences[&ProductionId(0)];
    assert_eq!(seq0.aliases.len(), 3);
}

#[test]
fn test_alias_sequence_grammar_lookup_by_production_id() {
    let g = grammar_with_alias_sequences();
    let seq1 = &g.alias_sequences[&ProductionId(1)];
    assert_eq!(seq1.aliases[0].as_deref(), Some("literal"));
}

#[test]
fn test_alias_sequence_empty() {
    let seq = AliasSequence {
        aliases: Vec::new(),
    };
    assert!(seq.aliases.is_empty());
}

#[test]
fn test_alias_sequence_insert_multiple_productions() {
    let mut g = minimal_grammar();
    for i in 0..5u16 {
        g.alias_sequences.insert(
            ProductionId(i),
            AliasSequence {
                aliases: vec![Some(format!("alias_{i}"))],
            },
        );
    }
    assert_eq!(g.alias_sequences.len(), 5);
    assert_eq!(
        g.alias_sequences[&ProductionId(3)].aliases[0].as_deref(),
        Some("alias_3")
    );
}

// ===========================================================================
// 7. Max alias sequence length tracking (8 tests)
// ===========================================================================

#[test]
fn test_max_alias_len_default_zero() {
    let g = minimal_grammar();
    assert_eq!(g.max_alias_sequence_length, 0);
}

#[test]
fn test_max_alias_len_set_directly() {
    let mut g = minimal_grammar();
    g.max_alias_sequence_length = 5;
    assert_eq!(g.max_alias_sequence_length, 5);
}

#[test]
fn test_max_alias_len_matches_longest_sequence() {
    let g = grammar_with_alias_sequences();
    let actual_max = g
        .alias_sequences
        .values()
        .map(|s| s.aliases.len())
        .max()
        .unwrap_or(0);
    assert_eq!(g.max_alias_sequence_length, actual_max);
}

#[test]
fn test_max_alias_len_single_element() {
    let mut g = minimal_grammar();
    g.alias_sequences.insert(
        ProductionId(0),
        AliasSequence {
            aliases: vec![Some("only".to_string())],
        },
    );
    g.max_alias_sequence_length = 1;
    assert_eq!(g.max_alias_sequence_length, 1);
}

#[test]
fn test_max_alias_len_computed_from_sequences() {
    let mut g = minimal_grammar();
    g.alias_sequences.insert(
        ProductionId(0),
        AliasSequence {
            aliases: vec![None, None],
        },
    );
    g.alias_sequences.insert(
        ProductionId(1),
        AliasSequence {
            aliases: vec![None, None, None, None, None],
        },
    );
    let computed = g
        .alias_sequences
        .values()
        .map(|s| s.aliases.len())
        .max()
        .unwrap_or(0);
    g.max_alias_sequence_length = computed;
    assert_eq!(g.max_alias_sequence_length, 5);
}

#[test]
fn test_max_alias_len_empty_sequences_gives_zero() {
    let g = minimal_grammar();
    let computed = g
        .alias_sequences
        .values()
        .map(|s| s.aliases.len())
        .max()
        .unwrap_or(0);
    assert_eq!(computed, 0);
}

#[test]
fn test_max_alias_len_preserved_through_normalize() {
    let mut g = grammar_with_alias_sequences();
    let len_before = g.max_alias_sequence_length;
    let _ = g.normalize();
    assert_eq!(g.max_alias_sequence_length, len_before);
}

#[test]
fn test_max_alias_len_preserved_through_optimize() {
    let mut g = grammar_with_alias_sequences();
    let len_before = g.max_alias_sequence_length;
    g.optimize();
    assert_eq!(g.max_alias_sequence_length, len_before);
}

// ===========================================================================
// 8. Edge cases (8 tests)
// ===========================================================================

#[test]
fn test_edge_empty_field_name() {
    let mut g = minimal_grammar();
    g.fields.insert(FieldId(0), String::new());
    assert_eq!(g.fields[&FieldId(0)], "");
}

#[test]
fn test_edge_many_fields() {
    let mut g = minimal_grammar();
    for i in 0..200u16 {
        g.fields.insert(FieldId(i), format!("field_{i}"));
    }
    assert_eq!(g.fields.len(), 200);
    assert_eq!(g.fields[&FieldId(199)], "field_199");
}

#[test]
fn test_edge_duplicate_field_names_different_ids() {
    let mut g = minimal_grammar();
    g.fields.insert(FieldId(0), "name".to_string());
    g.fields.insert(FieldId(1), "name".to_string());
    // IndexMap allows duplicate values under different keys
    assert_eq!(g.fields.len(), 2);
    assert_eq!(g.fields[&FieldId(0)], g.fields[&FieldId(1)]);
}

#[test]
fn test_edge_field_id_max_u16() {
    let mut g = minimal_grammar();
    g.fields.insert(FieldId(u16::MAX), "last_field".to_string());
    assert_eq!(g.fields[&FieldId(u16::MAX)], "last_field");
}

#[test]
fn test_edge_alias_sequence_very_long() {
    let mut g = minimal_grammar();
    let long_aliases: Vec<Option<String>> = (0..100).map(|i| Some(format!("pos_{i}"))).collect();
    g.alias_sequences.insert(
        ProductionId(0),
        AliasSequence {
            aliases: long_aliases,
        },
    );
    g.max_alias_sequence_length = 100;
    assert_eq!(g.alias_sequences[&ProductionId(0)].aliases.len(), 100);
    assert_eq!(g.max_alias_sequence_length, 100);
}

#[test]
fn test_edge_production_id_zero() {
    let pid = ProductionId(0);
    assert_eq!(pid.0, 0);
    let mut map: IndexMap<ProductionId, String> = IndexMap::new();
    map.insert(pid, "zero".to_string());
    assert_eq!(map[&ProductionId(0)], "zero");
}

#[test]
fn test_edge_combined_fields_aliases_productions() {
    let mut g = expr_grammar();
    // Add fields
    g.fields.insert(FieldId(0), "left".to_string());
    g.fields.insert(FieldId(1), "right".to_string());
    // Add alias sequences
    g.alias_sequences.insert(
        ProductionId(0),
        AliasSequence {
            aliases: vec![Some("lhs".to_string()), None, Some("rhs".to_string())],
        },
    );
    // Add production_ids
    g.production_ids.insert(RuleId(0), ProductionId(0));
    g.max_alias_sequence_length = 3;

    assert_eq!(g.fields.len(), 2);
    assert_eq!(g.alias_sequences.len(), 1);
    assert_eq!(g.production_ids[&RuleId(0)], ProductionId(0));
    assert_eq!(g.max_alias_sequence_length, 3);
}

#[test]
fn test_edge_fields_survive_normalize_then_optimize() {
    let mut g = grammar_with_fields();
    g.alias_sequences.insert(
        ProductionId(0),
        AliasSequence {
            aliases: vec![Some("x".to_string())],
        },
    );
    g.production_ids.insert(RuleId(0), ProductionId(0));
    g.max_alias_sequence_length = 1;

    let _ = g.normalize();
    g.optimize();

    assert_eq!(g.fields.len(), 3);
    assert_eq!(g.fields[&FieldId(0)], "left");
    assert_eq!(g.alias_sequences.len(), 1);
    assert_eq!(g.production_ids[&RuleId(0)], ProductionId(0));
    assert_eq!(g.max_alias_sequence_length, 1);
}
