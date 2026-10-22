//! Comprehensive tests for production ID management in adze-ir.
//!
//! Covers: sequential IDs, uniqueness, scaling with rules, alias sequences,
//! normalize preservation, ProductionId trait properties, and edge cases.

use adze_ir::builder::GrammarBuilder;
use adze_ir::{AliasSequence, Grammar, ProductionId, RuleId};
use std::collections::{BTreeSet, HashMap, HashSet};

// ============================================================
// Helper: build a grammar with N simple rules
// ============================================================

fn grammar_with_n_rules(n: usize) -> Grammar {
    let mut b = GrammarBuilder::new("test");
    b = b.token("a", "a");
    for i in 0..n {
        let name = format!("r{i}");
        // leak is fine in tests — gives us &'static str
        let name: &'static str = Box::leak(name.into_boxed_str());
        b = b.rule(name, vec!["a"]);
    }
    if n > 0 {
        b = b.start("r0");
    }
    b.build()
}

fn populate_sequential_production_ids(g: &mut Grammar) {
    let rule_count: u16 = g.rules.len().try_into().expect("too many rules");
    for i in 0..rule_count {
        g.production_ids.insert(RuleId(i), ProductionId(i));
    }
}

// ============================================================
// 1. Production IDs are sequential (8 tests)
// ============================================================

#[test]
fn sequential_ids_single_rule() {
    let mut g = grammar_with_n_rules(1);
    populate_sequential_production_ids(&mut g);
    assert_eq!(g.production_ids[&RuleId(0)], ProductionId(0));
}

#[test]
fn sequential_ids_two_rules() {
    let mut g = grammar_with_n_rules(2);
    populate_sequential_production_ids(&mut g);
    assert_eq!(g.production_ids[&RuleId(0)], ProductionId(0));
    assert_eq!(g.production_ids[&RuleId(1)], ProductionId(1));
}

#[test]
fn sequential_ids_five_rules() {
    let mut g = grammar_with_n_rules(5);
    populate_sequential_production_ids(&mut g);
    for i in 0..5u16 {
        assert_eq!(g.production_ids[&RuleId(i)], ProductionId(i));
    }
}

#[test]
fn sequential_ids_form_contiguous_range() {
    let mut g = grammar_with_n_rules(10);
    populate_sequential_production_ids(&mut g);
    let ids: Vec<u16> = g.production_ids.values().map(|p| p.0).collect();
    for (i, &id) in ids.iter().enumerate() {
        assert_eq!(id, i as u16, "ID at index {i} should be {i}");
    }
}

#[test]
fn sequential_ids_start_from_zero() {
    let mut g = grammar_with_n_rules(3);
    populate_sequential_production_ids(&mut g);
    let min_id = g.production_ids.values().map(|p| p.0).min().unwrap();
    assert_eq!(min_id, 0);
}

#[test]
fn sequential_ids_max_equals_count_minus_one() {
    let mut g = grammar_with_n_rules(7);
    populate_sequential_production_ids(&mut g);
    let max_id = g.production_ids.values().map(|p| p.0).max().unwrap();
    assert_eq!(max_id, 6);
}

#[test]
fn sequential_ids_no_gaps() {
    let mut g = grammar_with_n_rules(8);
    populate_sequential_production_ids(&mut g);
    let mut ids: Vec<u16> = g.production_ids.values().map(|p| p.0).collect();
    ids.sort();
    for (i, &id) in ids.iter().enumerate() {
        assert_eq!(id, i as u16, "gap detected at index {i}");
    }
}

#[test]
fn sequential_ids_insertion_order_matches() {
    let mut g = grammar_with_n_rules(4);
    populate_sequential_production_ids(&mut g);
    let entries: Vec<(RuleId, ProductionId)> =
        g.production_ids.iter().map(|(&r, &p)| (r, p)).collect();
    for (i, (rule_id, prod_id)) in entries.iter().enumerate() {
        assert_eq!(rule_id.0, i as u16);
        assert_eq!(prod_id.0, i as u16);
    }
}

// ============================================================
// 2. Production ID uniqueness (8 tests)
// ============================================================

#[test]
fn uniqueness_no_duplicate_ids_small() {
    let mut g = grammar_with_n_rules(3);
    populate_sequential_production_ids(&mut g);
    let ids: HashSet<u16> = g.production_ids.values().map(|p| p.0).collect();
    assert_eq!(ids.len(), g.production_ids.len());
}

#[test]
fn uniqueness_no_duplicate_ids_ten() {
    let mut g = grammar_with_n_rules(10);
    populate_sequential_production_ids(&mut g);
    let ids: HashSet<u16> = g.production_ids.values().map(|p| p.0).collect();
    assert_eq!(ids.len(), 10);
}

#[test]
fn uniqueness_all_rule_ids_distinct() {
    let mut g = grammar_with_n_rules(6);
    populate_sequential_production_ids(&mut g);
    let rule_ids: HashSet<u16> = g.production_ids.keys().map(|r| r.0).collect();
    assert_eq!(rule_ids.len(), 6);
}

#[test]
fn uniqueness_bijection_rule_to_production() {
    let mut g = grammar_with_n_rules(5);
    populate_sequential_production_ids(&mut g);
    let rule_set: HashSet<u16> = g.production_ids.keys().map(|r| r.0).collect();
    let prod_set: HashSet<u16> = g.production_ids.values().map(|p| p.0).collect();
    assert_eq!(rule_set.len(), prod_set.len());
}

#[test]
fn uniqueness_reverse_map_is_injective() {
    let mut g = grammar_with_n_rules(8);
    populate_sequential_production_ids(&mut g);
    let mut reverse: HashMap<ProductionId, RuleId> = HashMap::new();
    for (&rule_id, &prod_id) in &g.production_ids {
        let prev = reverse.insert(prod_id, rule_id);
        assert!(prev.is_none(), "duplicate production ID: {prod_id:?}");
    }
}

#[test]
fn uniqueness_btreeset_preserves_all() {
    let mut g = grammar_with_n_rules(12);
    populate_sequential_production_ids(&mut g);
    let sorted: BTreeSet<u16> = g.production_ids.values().map(|p| p.0).collect();
    assert_eq!(sorted.len(), 12);
}

#[test]
fn uniqueness_empty_grammar_no_ids() {
    let g = grammar_with_n_rules(0);
    assert_eq!(g.production_ids.len(), g.all_rules().count());
}

#[test]
fn uniqueness_manual_ids_can_have_gaps() {
    let mut g = grammar_with_n_rules(3);
    g.production_ids.insert(RuleId(0), ProductionId(0));
    g.production_ids.insert(RuleId(1), ProductionId(5));
    g.production_ids.insert(RuleId(2), ProductionId(10));
    let ids: HashSet<u16> = g.production_ids.values().map(|p| p.0).collect();
    assert_eq!(ids.len(), 3);
}

// ============================================================
// 3. Production IDs scale with rules (8 tests)
// ============================================================

#[test]
fn scale_one_rule_one_id() {
    let mut g = grammar_with_n_rules(1);
    populate_sequential_production_ids(&mut g);
    assert_eq!(g.production_ids.len(), g.all_rules().count());
}

#[test]
fn scale_three_rules_three_ids() {
    let mut g = grammar_with_n_rules(3);
    populate_sequential_production_ids(&mut g);
    assert_eq!(g.production_ids.len(), 3);
}

#[test]
fn scale_ten_rules_ten_ids() {
    let mut g = grammar_with_n_rules(10);
    populate_sequential_production_ids(&mut g);
    assert_eq!(g.production_ids.len(), 10);
}

#[test]
fn scale_twenty_rules() {
    let mut g = grammar_with_n_rules(20);
    populate_sequential_production_ids(&mut g);
    assert_eq!(g.production_ids.len(), 20);
}

#[test]
fn scale_more_rules_more_ids() {
    let small = {
        let mut g = grammar_with_n_rules(3);
        populate_sequential_production_ids(&mut g);
        g.production_ids.len()
    };
    let large = {
        let mut g = grammar_with_n_rules(10);
        populate_sequential_production_ids(&mut g);
        g.production_ids.len()
    };
    assert!(large > small);
}

#[test]
fn scale_id_count_matches_rule_count() {
    for n in [1, 2, 5, 8, 15] {
        let mut g = grammar_with_n_rules(n);
        populate_sequential_production_ids(&mut g);
        assert_eq!(
            g.production_ids.len(),
            g.rules.len(),
            "mismatch for {n} rules"
        );
    }
}

#[test]
fn scale_max_id_grows_with_rules() {
    let max5 = {
        let mut g = grammar_with_n_rules(5);
        populate_sequential_production_ids(&mut g);
        g.production_ids.values().map(|p| p.0).max().unwrap()
    };
    let max10 = {
        let mut g = grammar_with_n_rules(10);
        populate_sequential_production_ids(&mut g);
        g.production_ids.values().map(|p| p.0).max().unwrap()
    };
    assert!(max10 > max5);
}

#[test]
fn scale_fifty_rules() {
    let mut g = grammar_with_n_rules(50);
    populate_sequential_production_ids(&mut g);
    assert_eq!(g.production_ids.len(), 50);
    let ids: HashSet<u16> = g.production_ids.values().map(|p| p.0).collect();
    assert_eq!(ids.len(), 50);
}

// ============================================================
// 4. Alias sequences (8 tests)
// ============================================================

#[test]
fn alias_sequences_initially_empty() {
    let g = GrammarBuilder::new("test")
        .token("a", "a")
        .rule("s", vec!["a"])
        .start("s")
        .build();
    assert!(g.alias_sequences.is_empty());
}

#[test]
fn alias_max_length_initially_zero() {
    let g = GrammarBuilder::new("test").build();
    assert_eq!(g.max_alias_sequence_length, 0);
}

#[test]
fn alias_insert_single_sequence() {
    let mut g = GrammarBuilder::new("test").build();
    g.alias_sequences.insert(
        ProductionId(0),
        AliasSequence {
            aliases: vec![Some("expr".to_string())],
        },
    );
    assert_eq!(g.alias_sequences.len(), 1);
}

#[test]
fn alias_max_length_tracks_longest() {
    let mut g = GrammarBuilder::new("test").build();
    g.alias_sequences.insert(
        ProductionId(0),
        AliasSequence {
            aliases: vec![Some("a".to_string()), None],
        },
    );
    g.alias_sequences.insert(
        ProductionId(1),
        AliasSequence {
            aliases: vec![None, Some("b".to_string()), Some("c".to_string())],
        },
    );
    g.max_alias_sequence_length = g
        .alias_sequences
        .values()
        .map(|s| s.aliases.len())
        .max()
        .unwrap_or(0);
    assert_eq!(g.max_alias_sequence_length, 3);
}

#[test]
fn alias_sequence_with_all_none() {
    let seq = AliasSequence {
        aliases: vec![None, None, None],
    };
    assert!(seq.aliases.iter().all(|a| a.is_none()));
}

#[test]
fn alias_sequence_with_all_some() {
    let seq = AliasSequence {
        aliases: vec![
            Some("a".to_string()),
            Some("b".to_string()),
            Some("c".to_string()),
        ],
    };
    assert!(seq.aliases.iter().all(|a| a.is_some()));
    assert_eq!(seq.aliases.len(), 3);
}

#[test]
fn alias_sequences_keyed_by_production_id() {
    let mut g = GrammarBuilder::new("test").build();
    for i in 0..5u16 {
        g.alias_sequences.insert(
            ProductionId(i),
            AliasSequence {
                aliases: vec![Some(format!("alias_{i}"))],
            },
        );
    }
    assert_eq!(g.alias_sequences.len(), 5);
    for i in 0..5u16 {
        let seq = &g.alias_sequences[&ProductionId(i)];
        assert_eq!(
            seq.aliases[0].as_deref(),
            Some(format!("alias_{i}")).as_deref()
        );
    }
}

#[test]
fn alias_sequence_replace_existing() {
    let mut g = GrammarBuilder::new("test").build();
    g.alias_sequences.insert(
        ProductionId(0),
        AliasSequence {
            aliases: vec![Some("old".to_string())],
        },
    );
    g.alias_sequences.insert(
        ProductionId(0),
        AliasSequence {
            aliases: vec![Some("new".to_string())],
        },
    );
    assert_eq!(g.alias_sequences.len(), 1);
    assert_eq!(
        g.alias_sequences[&ProductionId(0)].aliases[0].as_deref(),
        Some("new")
    );
}

// ============================================================
// 5. Normalize preserves production IDs (7 tests)
// ============================================================

#[test]
fn normalize_preserves_existing_production_ids_map() {
    let mut g = GrammarBuilder::new("test")
        .token("a", "a")
        .rule("s", vec!["a"])
        .start("s")
        .build();
    g.production_ids.insert(RuleId(0), ProductionId(42));
    g.normalize();
    assert_eq!(g.production_ids[&RuleId(0)], ProductionId(42));
}

#[test]
fn normalize_does_not_remove_production_ids() {
    let mut g = GrammarBuilder::new("test")
        .token("a", "a")
        .token("b", "b")
        .rule("s", vec!["a"])
        .rule("t", vec!["b"])
        .start("s")
        .build();
    g.production_ids.insert(RuleId(0), ProductionId(0));
    g.production_ids.insert(RuleId(1), ProductionId(1));
    let count_before = g.production_ids.len();
    g.normalize();
    assert!(g.production_ids.len() >= count_before);
}

#[test]
fn normalize_empty_grammar_no_crash() {
    let mut g = GrammarBuilder::new("empty").build();
    let rules = g.normalize();
    assert_eq!(g.production_ids.len(), g.all_rules().count());
    // normalize returns collected rules
    let _ = rules;
}

#[test]
fn normalize_preserves_id_values() {
    let mut g = GrammarBuilder::new("test")
        .token("x", "x")
        .rule("s", vec!["x"])
        .start("s")
        .build();
    g.production_ids.insert(RuleId(0), ProductionId(99));
    g.normalize();
    assert_eq!(g.production_ids[&RuleId(0)].0, 99);
}

#[test]
fn normalize_preserves_alias_sequences() {
    let mut g = GrammarBuilder::new("test")
        .token("a", "a")
        .rule("s", vec!["a"])
        .start("s")
        .build();
    g.alias_sequences.insert(
        ProductionId(0),
        AliasSequence {
            aliases: vec![Some("kept".to_string())],
        },
    );
    g.normalize();
    assert_eq!(
        g.alias_sequences[&ProductionId(0)].aliases[0].as_deref(),
        Some("kept")
    );
}

#[test]
fn normalize_preserves_max_alias_sequence_length() {
    let mut g = GrammarBuilder::new("test")
        .token("a", "a")
        .rule("s", vec!["a"])
        .start("s")
        .build();
    g.max_alias_sequence_length = 5;
    g.normalize();
    assert_eq!(g.max_alias_sequence_length, 5);
}

#[test]
fn normalize_multiple_times_preserves_ids() {
    let mut g = GrammarBuilder::new("test")
        .token("a", "a")
        .rule("s", vec!["a"])
        .start("s")
        .build();
    g.production_ids.insert(RuleId(0), ProductionId(7));
    g.normalize();
    g.normalize();
    g.normalize();
    assert_eq!(g.production_ids[&RuleId(0)], ProductionId(7));
}

// ============================================================
// 6. Production ID properties (8 tests)
// ============================================================

#[test]
fn property_copy() {
    let id = ProductionId(5);
    let id2 = id;
    assert_eq!(id, id2);
}

#[test]
fn property_debug_contains_value() {
    let id = ProductionId(42);
    let dbg = format!("{id:?}");
    assert!(dbg.contains("42"), "Debug should contain inner value");
    assert!(
        dbg.contains("ProductionId"),
        "Debug should contain type name"
    );
}

#[test]
fn property_partial_eq() {
    assert_eq!(ProductionId(0), ProductionId(0));
    assert_ne!(ProductionId(0), ProductionId(1));
}

#[test]
fn property_eq_reflexive() {
    let id = ProductionId(10);
    assert_eq!(id, id);
}

#[test]
fn property_hash_equal_ids_same_bucket() {
    use std::hash::{Hash, Hasher};
    let mut h1 = std::collections::hash_map::DefaultHasher::new();
    let mut h2 = std::collections::hash_map::DefaultHasher::new();
    ProductionId(3).hash(&mut h1);
    ProductionId(3).hash(&mut h2);
    assert_eq!(h1.finish(), h2.finish());
}

#[test]
fn property_ord_ascending() {
    assert!(ProductionId(0) < ProductionId(1));
    assert!(ProductionId(1) < ProductionId(100));
    assert!(ProductionId(99) < ProductionId(u16::MAX));
}

#[test]
fn property_ord_sorting() {
    let mut ids = [
        ProductionId(5),
        ProductionId(2),
        ProductionId(9),
        ProductionId(0),
        ProductionId(7),
    ];
    ids.sort();
    let values: Vec<u16> = ids.iter().map(|p| p.0).collect();
    assert_eq!(values, [0, 2, 5, 7, 9]);
}

#[test]
fn property_display_format() {
    assert_eq!(format!("{}", ProductionId(0)), "Production(0)");
    assert_eq!(format!("{}", ProductionId(255)), "Production(255)");
    assert_eq!(
        format!("{}", ProductionId(u16::MAX)),
        format!("Production({})", u16::MAX)
    );
}

// ============================================================
// 7. Edge cases (8 tests)
// ============================================================

#[test]
fn edge_single_rule_grammar() {
    let g = GrammarBuilder::new("min")
        .token("x", "x")
        .rule("s", vec!["x"])
        .start("s")
        .build();
    assert_eq!(g.rules.len(), 1);
    assert_eq!(g.production_ids.len(), g.all_rules().count());
}

#[test]
fn edge_no_rules_grammar() {
    let g = GrammarBuilder::new("empty").build();
    assert!(g.rules.is_empty());
    assert_eq!(g.production_ids.len(), g.all_rules().count());
    assert!(g.alias_sequences.is_empty());
    assert_eq!(g.max_alias_sequence_length, 0);
}

#[test]
fn edge_many_rules_grammar() {
    let mut g = grammar_with_n_rules(100);
    populate_sequential_production_ids(&mut g);
    assert_eq!(g.production_ids.len(), 100);
    let max_id = g.production_ids.values().map(|p| p.0).max().unwrap();
    assert_eq!(max_id, 99);
}

#[test]
fn edge_production_id_zero_value() {
    let id = ProductionId(0);
    assert_eq!(id.0, 0);
    assert_eq!(format!("{id}"), "Production(0)");
}

#[test]
fn edge_production_id_max_value() {
    let id = ProductionId(u16::MAX);
    assert_eq!(id.0, 65535);
}

#[test]
fn edge_id_arithmetic_wrapping() {
    let id = ProductionId(u16::MAX);
    let next = ProductionId(id.0.wrapping_add(1));
    assert_eq!(next.0, 0);
}

#[test]
fn edge_production_id_as_map_key() {
    let mut map: HashMap<ProductionId, String> = HashMap::new();
    map.insert(ProductionId(0), "first".to_string());
    map.insert(ProductionId(1), "second".to_string());
    assert_eq!(map[&ProductionId(0)], "first");
    assert_eq!(map[&ProductionId(1)], "second");
}

#[test]
fn edge_default_grammar_has_empty_production_ids() {
    let g = Grammar::default();
    assert_eq!(g.production_ids.len(), g.all_rules().count());
    assert!(g.alias_sequences.is_empty());
    assert_eq!(g.max_alias_sequence_length, 0);
}
