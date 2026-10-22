//! Tests for alias sequences, production IDs, alias mapping, and edge cases.

use adze_ir::builder::GrammarBuilder;
use adze_ir::{AliasSequence, Grammar, ProductionId, RuleId};
use std::collections::HashSet;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn simple_grammar() -> Grammar {
    GrammarBuilder::new("test")
        .token("NUM", r"\d+")
        .token("+", "+")
        .rule("expr", vec!["NUM"])
        .rule("expr", vec!["expr", "+", "expr"])
        .start("expr")
        .build()
}

fn grammar_with_many_rules(n: usize) -> Grammar {
    let mut b = GrammarBuilder::new("many");
    b = b.token("a", "a");
    for i in 0..n {
        let name: &'static str = Box::leak(format!("r{i}").into_boxed_str());
        b = b.rule(name, vec!["a"]);
    }
    if n > 0 {
        b = b.start("r0");
    }
    b.build()
}

fn populate_alias_sequences(g: &mut Grammar, entries: Vec<(u16, Vec<Option<String>>)>) {
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

fn populate_production_ids(g: &mut Grammar, count: u16) {
    for i in 0..count {
        g.production_ids.insert(RuleId(i), ProductionId(i));
    }
}

// ===========================================================================
// 1. Alias sequences — grammar alias_sequences field, max_alias_sequence_length
// ===========================================================================

#[test]
fn alias_sequences_initially_empty() {
    let g = simple_grammar();
    assert!(g.alias_sequences.is_empty());
    assert_eq!(g.max_alias_sequence_length, 0);
}

#[test]
fn insert_single_alias_sequence() {
    let mut g = simple_grammar();
    let seq = AliasSequence {
        aliases: vec![Some("aliased_expr".to_string())],
    };
    g.alias_sequences.insert(ProductionId(0), seq);
    g.max_alias_sequence_length = 1;
    assert_eq!(g.alias_sequences.len(), 1);
    assert_eq!(g.max_alias_sequence_length, 1);
}

#[test]
fn insert_multiple_alias_sequences() {
    let mut g = simple_grammar();
    populate_alias_sequences(
        &mut g,
        vec![
            (0, vec![Some("a".to_string())]),
            (1, vec![Some("b".to_string()), None]),
            (2, vec![None, None, Some("c".to_string())]),
        ],
    );
    assert_eq!(g.alias_sequences.len(), 3);
    assert_eq!(g.max_alias_sequence_length, 3);
}

#[test]
fn max_alias_sequence_length_tracks_longest() {
    let mut g = simple_grammar();
    populate_alias_sequences(
        &mut g,
        vec![
            (0, vec![Some("x".to_string())]),
            (1, vec![None; 7]),
            (2, vec![Some("y".to_string()), None]),
        ],
    );
    assert_eq!(g.max_alias_sequence_length, 7);
}

#[test]
fn max_alias_sequence_length_zero_when_all_empty() {
    let mut g = simple_grammar();
    populate_alias_sequences(&mut g, vec![(0, vec![]), (1, vec![])]);
    assert_eq!(g.max_alias_sequence_length, 0);
}

#[test]
fn alias_sequence_overwrite_same_production_id() {
    let mut g = simple_grammar();
    let seq1 = AliasSequence {
        aliases: vec![Some("first".to_string())],
    };
    let seq2 = AliasSequence {
        aliases: vec![Some("second".to_string()), None],
    };
    g.alias_sequences.insert(ProductionId(0), seq1);
    g.alias_sequences.insert(ProductionId(0), seq2);
    assert_eq!(g.alias_sequences.len(), 1);
    assert_eq!(
        g.alias_sequences[&ProductionId(0)].aliases[0].as_deref(),
        Some("second")
    );
}

#[test]
fn alias_sequences_preserve_insertion_order() {
    let mut g = simple_grammar();
    populate_alias_sequences(
        &mut g,
        vec![
            (5, vec![Some("fifth".to_string())]),
            (2, vec![Some("second".to_string())]),
            (8, vec![Some("eighth".to_string())]),
        ],
    );
    let keys: Vec<u16> = g.alias_sequences.keys().map(|p| p.0).collect();
    assert_eq!(keys, vec![5, 2, 8]);
}

#[test]
fn alias_sequences_accessible_by_production_id() {
    let mut g = simple_grammar();
    populate_alias_sequences(
        &mut g,
        vec![
            (10, vec![Some("ten".to_string())]),
            (20, vec![Some("twenty".to_string())]),
        ],
    );
    assert_eq!(
        g.alias_sequences[&ProductionId(10)].aliases[0].as_deref(),
        Some("ten")
    );
    assert_eq!(
        g.alias_sequences[&ProductionId(20)].aliases[0].as_deref(),
        Some("twenty")
    );
}

// ===========================================================================
// 2. Production IDs — assignment, uniqueness
// ===========================================================================

#[test]
fn production_ids_populated_by_builder() {
    let g = simple_grammar();
    assert_eq!(g.production_ids.len(), g.all_rules().count());
}

#[test]
fn production_ids_sequential_assignment() {
    let mut g = simple_grammar();
    populate_production_ids(&mut g, 5);
    for i in 0..5u16 {
        assert_eq!(g.production_ids[&RuleId(i)], ProductionId(i));
    }
}

#[test]
fn production_ids_unique_values() {
    let mut g = grammar_with_many_rules(20);
    populate_production_ids(&mut g, 20);
    let values: HashSet<u16> = g.production_ids.values().map(|p| p.0).collect();
    assert_eq!(values.len(), 20);
}

#[test]
fn production_id_from_rule_preserves_order() {
    let g = simple_grammar();
    let pids: Vec<ProductionId> = g.all_rules().map(|r| r.production_id).collect();
    assert!(pids.len() >= 2);
    assert_ne!(pids[0], pids[1]);
}

#[test]
fn builder_assigns_distinct_production_ids() {
    let g = GrammarBuilder::new("test")
        .token("a", "a")
        .token("b", "b")
        .rule("s", vec!["a"])
        .rule("s", vec!["b"])
        .rule("s", vec!["a", "b"])
        .start("s")
        .build();
    let ids: HashSet<ProductionId> = g.all_rules().map(|r| r.production_id).collect();
    assert_eq!(ids.len(), 3);
}

#[test]
fn production_id_values_start_at_zero() {
    let g = GrammarBuilder::new("test")
        .token("x", "x")
        .rule("s", vec!["x"])
        .start("s")
        .build();
    let min_id = g.all_rules().map(|r| r.production_id.0).min().unwrap();
    assert_eq!(min_id, 0);
}

#[test]
fn production_ids_contiguous_range() {
    let g = grammar_with_many_rules(10);
    let mut ids: Vec<u16> = g.all_rules().map(|r| r.production_id.0).collect();
    ids.sort();
    for (i, id) in ids.iter().enumerate() {
        assert_eq!(*id, i as u16);
    }
}

// ===========================================================================
// 3. Alias mapping — symbol aliasing through grammar operations
// ===========================================================================

#[test]
fn alias_map_production_to_single_alias() {
    let mut g = simple_grammar();
    let seq = AliasSequence {
        aliases: vec![Some("renamed_expr".to_string())],
    };
    g.alias_sequences.insert(ProductionId(0), seq);
    assert_eq!(
        g.alias_sequences[&ProductionId(0)].aliases[0].as_deref(),
        Some("renamed_expr")
    );
}

#[test]
fn alias_map_multi_position_aliases() {
    let mut g = simple_grammar();
    let seq = AliasSequence {
        aliases: vec![Some("left".to_string()), None, Some("right".to_string())],
    };
    g.alias_sequences.insert(ProductionId(0), seq);
    let aliases = &g.alias_sequences[&ProductionId(0)].aliases;
    assert_eq!(aliases[0].as_deref(), Some("left"));
    assert!(aliases[1].is_none());
    assert_eq!(aliases[2].as_deref(), Some("right"));
}

#[test]
fn alias_map_different_productions_different_aliases() {
    let mut g = simple_grammar();
    populate_alias_sequences(
        &mut g,
        vec![
            (0, vec![Some("production_zero".to_string())]),
            (1, vec![Some("production_one".to_string())]),
        ],
    );
    assert_ne!(
        g.alias_sequences[&ProductionId(0)].aliases[0],
        g.alias_sequences[&ProductionId(1)].aliases[0],
    );
}

#[test]
fn alias_map_same_name_in_different_positions() {
    let mut g = simple_grammar();
    let seq = AliasSequence {
        aliases: vec![
            Some("dup".to_string()),
            Some("dup".to_string()),
            Some("dup".to_string()),
        ],
    };
    g.alias_sequences.insert(ProductionId(0), seq);
    for alias in &g.alias_sequences[&ProductionId(0)].aliases {
        assert_eq!(alias.as_deref(), Some("dup"));
    }
}

#[test]
fn alias_map_lookup_missing_production_returns_none() {
    let g = simple_grammar();
    assert!(g.alias_sequences.get(&ProductionId(99)).is_none());
}

// ===========================================================================
// 4. Sequence length edge cases — empty, single, many aliases
// ===========================================================================

#[test]
fn sequence_length_zero() {
    let seq = AliasSequence { aliases: vec![] };
    assert_eq!(seq.aliases.len(), 0);
}

#[test]
fn sequence_length_one_some() {
    let seq = AliasSequence {
        aliases: vec![Some("a".to_string())],
    };
    assert_eq!(seq.aliases.len(), 1);
}

#[test]
fn sequence_length_one_none() {
    let seq = AliasSequence {
        aliases: vec![None],
    };
    assert_eq!(seq.aliases.len(), 1);
    assert!(seq.aliases[0].is_none());
}

#[test]
fn sequence_length_large() {
    let seq = AliasSequence {
        aliases: (0..100).map(|i| Some(format!("a{i}"))).collect(),
    };
    assert_eq!(seq.aliases.len(), 100);
}

#[test]
fn sequence_length_matches_rule_rhs_length() {
    let g = GrammarBuilder::new("test")
        .token("a", "a")
        .token("b", "b")
        .token("c", "c")
        .rule("s", vec!["a", "b", "c"])
        .start("s")
        .build();
    let rhs_len = g.all_rules().next().unwrap().rhs.len();
    let seq = AliasSequence {
        aliases: vec![None; rhs_len],
    };
    assert_eq!(seq.aliases.len(), 3);
}

#[test]
fn max_length_updates_correctly_on_remove() {
    let mut g = simple_grammar();
    populate_alias_sequences(&mut g, vec![(0, vec![None; 5]), (1, vec![None; 10])]);
    assert_eq!(g.max_alias_sequence_length, 10);
    g.alias_sequences.shift_remove(&ProductionId(1));
    let new_max = g
        .alias_sequences
        .values()
        .map(|s| s.aliases.len())
        .max()
        .unwrap_or(0);
    assert_eq!(new_max, 5);
}

// ===========================================================================
// 5. Alias determinism — same grammar → same alias sequences
// ===========================================================================

#[test]
fn deterministic_alias_sequences_same_grammar() {
    let build = || {
        let mut g = simple_grammar();
        populate_alias_sequences(
            &mut g,
            vec![
                (0, vec![Some("expr".to_string())]),
                (1, vec![None, Some("op".to_string()), None]),
            ],
        );
        g
    };
    let g1 = build();
    let g2 = build();
    assert_eq!(g1.alias_sequences, g2.alias_sequences);
    assert_eq!(g1.max_alias_sequence_length, g2.max_alias_sequence_length);
}

#[test]
fn deterministic_production_ids_from_builder() {
    let build = || {
        GrammarBuilder::new("det")
            .token("x", "x")
            .token("y", "y")
            .rule("s", vec!["x"])
            .rule("s", vec!["y"])
            .rule("s", vec!["x", "y"])
            .start("s")
            .build()
    };
    let g1 = build();
    let g2 = build();
    let ids1: Vec<ProductionId> = g1.all_rules().map(|r| r.production_id).collect();
    let ids2: Vec<ProductionId> = g2.all_rules().map(|r| r.production_id).collect();
    assert_eq!(ids1, ids2);
}

#[test]
fn deterministic_alias_key_order() {
    let build = || {
        let mut g = simple_grammar();
        populate_alias_sequences(
            &mut g,
            vec![
                (3, vec![Some("c".to_string())]),
                (1, vec![Some("a".to_string())]),
                (2, vec![Some("b".to_string())]),
            ],
        );
        g
    };
    let g1 = build();
    let g2 = build();
    let keys1: Vec<u16> = g1.alias_sequences.keys().map(|p| p.0).collect();
    let keys2: Vec<u16> = g2.alias_sequences.keys().map(|p| p.0).collect();
    assert_eq!(keys1, keys2);
}

#[test]
fn deterministic_max_length_across_builds() {
    let build = || {
        let mut g = simple_grammar();
        populate_alias_sequences(
            &mut g,
            vec![(0, vec![None; 4]), (1, vec![None; 2]), (2, vec![None; 6])],
        );
        g
    };
    for _ in 0..5 {
        let g = build();
        assert_eq!(g.max_alias_sequence_length, 6);
    }
}

#[test]
fn deterministic_alias_values_across_builds() {
    let build = || {
        let mut g = simple_grammar();
        let seq = AliasSequence {
            aliases: vec![Some("alpha".to_string()), None, Some("gamma".to_string())],
        };
        g.alias_sequences.insert(ProductionId(0), seq);
        g
    };
    let g1 = build();
    let g2 = build();
    assert_eq!(
        g1.alias_sequences[&ProductionId(0)],
        g2.alias_sequences[&ProductionId(0)]
    );
}

// ===========================================================================
// 6. Complex alias scenarios — nested aliases, aliases with precedence
// ===========================================================================

#[test]
fn alias_on_grammar_with_precedence() {
    let mut g = GrammarBuilder::new("calc")
        .token("NUM", r"\d+")
        .token("+", "+")
        .token("*", "*")
        .rule_with_precedence(
            "expr",
            vec!["expr", "+", "expr"],
            1,
            adze_ir::Associativity::Left,
        )
        .rule_with_precedence(
            "expr",
            vec!["expr", "*", "expr"],
            2,
            adze_ir::Associativity::Left,
        )
        .rule("expr", vec!["NUM"])
        .start("expr")
        .build();

    populate_alias_sequences(
        &mut g,
        vec![
            (
                0,
                vec![
                    Some("left_operand".to_string()),
                    Some("operator".to_string()),
                    Some("right_operand".to_string()),
                ],
            ),
            (
                1,
                vec![
                    Some("left_operand".to_string()),
                    Some("operator".to_string()),
                    Some("right_operand".to_string()),
                ],
            ),
        ],
    );
    assert_eq!(g.alias_sequences.len(), 2);
    assert_eq!(g.max_alias_sequence_length, 3);
}

#[test]
fn alias_on_epsilon_production() {
    let mut g = GrammarBuilder::new("nullable")
        .token("a", "a")
        .rule("opt", vec![])
        .rule("opt", vec!["a"])
        .start("opt")
        .build();
    populate_alias_sequences(&mut g, vec![(0, vec![])]);
    assert_eq!(g.alias_sequences[&ProductionId(0)].aliases.len(), 0);
}

#[test]
fn alias_with_external_tokens() {
    let mut g = GrammarBuilder::new("ext")
        .token("INDENT", "INDENT")
        .token("DEDENT", "DEDENT")
        .token("pass", "pass")
        .external("INDENT")
        .external("DEDENT")
        .rule("block", vec!["INDENT", "pass", "DEDENT"])
        .start("block")
        .build();
    populate_alias_sequences(
        &mut g,
        vec![(
            0,
            vec![
                Some("begin".to_string()),
                Some("body".to_string()),
                Some("end".to_string()),
            ],
        )],
    );
    assert_eq!(g.max_alias_sequence_length, 3);
}

#[test]
fn alias_across_multiple_lhs_symbols() {
    let mut g = GrammarBuilder::new("multi")
        .token("a", "a")
        .token("b", "b")
        .rule("s", vec!["a"])
        .rule("t", vec!["b"])
        .start("s")
        .build();
    populate_alias_sequences(
        &mut g,
        vec![
            (0, vec![Some("aliased_a".to_string())]),
            (1, vec![Some("aliased_b".to_string())]),
        ],
    );
    assert_eq!(
        g.alias_sequences[&ProductionId(0)].aliases[0].as_deref(),
        Some("aliased_a")
    );
    assert_eq!(
        g.alias_sequences[&ProductionId(1)].aliases[0].as_deref(),
        Some("aliased_b")
    );
}

#[test]
fn alias_with_inline_rules() {
    let mut g = GrammarBuilder::new("inlined")
        .token("a", "a")
        .token("b", "b")
        .rule("s", vec!["helper"])
        .rule("helper", vec!["a", "b"])
        .inline("helper")
        .start("s")
        .build();
    populate_alias_sequences(
        &mut g,
        vec![(
            1,
            vec![Some("first".to_string()), Some("second".to_string())],
        )],
    );
    assert_eq!(g.max_alias_sequence_length, 2);
    assert!(!g.inline_rules.is_empty());
}

#[test]
fn alias_sequences_with_supertypes() {
    let mut g = GrammarBuilder::new("super")
        .token("a", "a")
        .token("b", "b")
        .rule("expression", vec!["a"])
        .rule("expression", vec!["b"])
        .supertype("expression")
        .start("expression")
        .build();
    populate_alias_sequences(
        &mut g,
        vec![
            (0, vec![Some("literal".to_string())]),
            (1, vec![Some("literal".to_string())]),
        ],
    );
    assert!(!g.supertypes.is_empty());
    assert_eq!(g.alias_sequences.len(), 2);
}

// ===========================================================================
// 7. Edge cases — no aliases, all aliases, mixed
// ===========================================================================

#[test]
fn no_aliases_at_all() {
    let g = simple_grammar();
    assert!(g.alias_sequences.is_empty());
    assert_eq!(g.max_alias_sequence_length, 0);
}

#[test]
fn all_positions_aliased() {
    let mut g = simple_grammar();
    let seq = AliasSequence {
        aliases: vec![
            Some("a".to_string()),
            Some("b".to_string()),
            Some("c".to_string()),
        ],
    };
    g.alias_sequences.insert(ProductionId(0), seq);
    assert!(
        g.alias_sequences[&ProductionId(0)]
            .aliases
            .iter()
            .all(|a| a.is_some())
    );
}

#[test]
fn no_positions_aliased() {
    let mut g = simple_grammar();
    let seq = AliasSequence {
        aliases: vec![None, None, None],
    };
    g.alias_sequences.insert(ProductionId(0), seq);
    assert!(
        g.alias_sequences[&ProductionId(0)]
            .aliases
            .iter()
            .all(|a| a.is_none())
    );
}

#[test]
fn mixed_some_none_aliases() {
    let mut g = simple_grammar();
    let seq = AliasSequence {
        aliases: vec![Some("aliased".to_string()), None, Some("also".to_string())],
    };
    g.alias_sequences.insert(ProductionId(0), seq);
    let aliases = &g.alias_sequences[&ProductionId(0)].aliases;
    assert!(aliases[0].is_some());
    assert!(aliases[1].is_none());
    assert!(aliases[2].is_some());
}

#[test]
fn empty_grammar_no_aliases() {
    let g = Grammar::default();
    assert!(g.alias_sequences.is_empty());
    assert_eq!(g.max_alias_sequence_length, 0);
}

#[test]
fn alias_sequence_with_empty_string_alias() {
    let seq = AliasSequence {
        aliases: vec![Some(String::new()), Some("valid".to_string())],
    };
    assert_eq!(seq.aliases[0].as_deref(), Some(""));
    assert_eq!(seq.aliases[1].as_deref(), Some("valid"));
}

#[test]
fn alias_sequence_clone_equality() {
    let seq = AliasSequence {
        aliases: vec![Some("x".to_string()), None, Some("z".to_string())],
    };
    let cloned = seq.clone();
    assert_eq!(seq, cloned);
}

#[test]
fn alias_sequence_debug_format() {
    let seq = AliasSequence {
        aliases: vec![Some("a".to_string()), None],
    };
    let dbg = format!("{seq:?}");
    assert!(dbg.contains("AliasSequence"));
    assert!(dbg.contains("aliases"));
}

#[test]
fn production_id_display() {
    let pid = ProductionId(42);
    let display = format!("{pid}");
    assert!(display.contains("42"));
}

#[test]
fn production_id_copy_semantics() {
    let pid = ProductionId(7);
    let copied = pid;
    assert_eq!(pid, copied);
}

#[test]
fn rule_id_copy_semantics() {
    let rid = RuleId(3);
    let copied = rid;
    assert_eq!(rid, copied);
}

#[test]
fn production_id_ordering() {
    let a = ProductionId(1);
    let b = ProductionId(2);
    assert!(a < b);
}

#[test]
fn production_id_hash_consistent() {
    let mut set = HashSet::new();
    set.insert(ProductionId(5));
    set.insert(ProductionId(5));
    assert_eq!(set.len(), 1);
}

#[test]
fn alias_sequence_serde_roundtrip() {
    let seq = AliasSequence {
        aliases: vec![Some("node".to_string()), None, Some("leaf".to_string())],
    };
    let json = serde_json::to_string(&seq).unwrap();
    let deserialized: AliasSequence = serde_json::from_str(&json).unwrap();
    assert_eq!(seq, deserialized);
}

#[test]
fn alias_sequences_map_serde_roundtrip() {
    let mut g = simple_grammar();
    populate_alias_sequences(
        &mut g,
        vec![
            (0, vec![Some("a".to_string()), None]),
            (1, vec![None, Some("b".to_string())]),
        ],
    );
    let json = serde_json::to_string(&g.alias_sequences).unwrap();
    let deserialized: indexmap::IndexMap<ProductionId, AliasSequence> =
        serde_json::from_str(&json).unwrap();
    assert_eq!(g.alias_sequences, deserialized);
}

#[test]
fn large_grammar_production_id_uniqueness() {
    let g = grammar_with_many_rules(50);
    let ids: HashSet<ProductionId> = g.all_rules().map(|r| r.production_id).collect();
    assert_eq!(ids.len(), 50);
}

#[test]
fn alias_sequence_with_unicode_names() {
    let seq = AliasSequence {
        aliases: vec![
            Some("表达式".to_string()),
            Some("αβγ".to_string()),
            Some("名前".to_string()),
        ],
    };
    assert_eq!(seq.aliases[0].as_deref(), Some("表达式"));
    assert_eq!(seq.aliases[1].as_deref(), Some("αβγ"));
    assert_eq!(seq.aliases[2].as_deref(), Some("名前"));
}

#[test]
fn alias_sequence_with_special_characters() {
    let seq = AliasSequence {
        aliases: vec![
            Some("node-name".to_string()),
            Some("node_name".to_string()),
            Some("node.name".to_string()),
        ],
    };
    for alias in &seq.aliases {
        assert!(alias.is_some());
    }
}

#[test]
fn max_alias_length_with_single_entry() {
    let mut g = simple_grammar();
    populate_alias_sequences(&mut g, vec![(0, vec![Some("only".to_string())])]);
    assert_eq!(g.max_alias_sequence_length, 1);
}

#[test]
fn max_alias_length_with_varying_lengths() {
    let mut g = simple_grammar();
    populate_alias_sequences(
        &mut g,
        vec![
            (0, vec![None]),
            (1, vec![None, None, None]),
            (2, vec![None, None]),
            (3, vec![None, None, None, None, None]),
        ],
    );
    assert_eq!(g.max_alias_sequence_length, 5);
}

#[test]
fn production_ids_map_rule_to_production() {
    let mut g = simple_grammar();
    g.production_ids.insert(RuleId(0), ProductionId(100));
    g.production_ids.insert(RuleId(1), ProductionId(200));
    assert_eq!(g.production_ids[&RuleId(0)], ProductionId(100));
    assert_eq!(g.production_ids[&RuleId(1)], ProductionId(200));
}

#[test]
fn production_ids_non_sequential_mapping() {
    let mut g = simple_grammar();
    // Builder grammars arrive with a total map (#980); this test's subject
    // is manual non-sequential population, so start from an empty map.
    g.production_ids.clear();
    g.production_ids.insert(RuleId(0), ProductionId(10));
    g.production_ids.insert(RuleId(5), ProductionId(50));
    g.production_ids.insert(RuleId(3), ProductionId(30));
    assert_eq!(g.production_ids.len(), 3);
}

#[test]
fn alias_on_grammar_with_extras() {
    let mut g = GrammarBuilder::new("with_extras")
        .token("a", "a")
        .token("WS", r"\s+")
        .extra("WS")
        .rule("s", vec!["a"])
        .start("s")
        .build();
    populate_alias_sequences(&mut g, vec![(0, vec![Some("item".to_string())])]);
    assert!(!g.extras.is_empty());
    assert_eq!(g.alias_sequences.len(), 1);
}

#[test]
fn alias_sequence_equality() {
    let seq1 = AliasSequence {
        aliases: vec![Some("a".to_string()), None],
    };
    let seq2 = AliasSequence {
        aliases: vec![Some("a".to_string()), None],
    };
    let seq3 = AliasSequence {
        aliases: vec![Some("b".to_string()), None],
    };
    assert_eq!(seq1, seq2);
    assert_ne!(seq1, seq3);
}

#[test]
fn alias_sequence_length_inequality() {
    let seq1 = AliasSequence {
        aliases: vec![Some("a".to_string())],
    };
    let seq2 = AliasSequence {
        aliases: vec![Some("a".to_string()), None],
    };
    assert_ne!(seq1, seq2);
}

#[test]
fn grammar_default_has_zero_max_alias_length() {
    let g = Grammar::default();
    assert_eq!(g.max_alias_sequence_length, 0);
}

#[test]
fn grammar_new_has_zero_max_alias_length() {
    let g = Grammar::new("fresh".to_string());
    assert_eq!(g.max_alias_sequence_length, 0);
    assert!(g.alias_sequences.is_empty());
}

#[test]
fn alias_sequences_can_contain_duplicate_production_ids_across_grammars() {
    let mut g1 = simple_grammar();
    let mut g2 = simple_grammar();
    populate_alias_sequences(&mut g1, vec![(0, vec![Some("a".to_string())])]);
    populate_alias_sequences(&mut g2, vec![(0, vec![Some("b".to_string())])]);
    assert_ne!(
        g1.alias_sequences[&ProductionId(0)],
        g2.alias_sequences[&ProductionId(0)]
    );
}

#[test]
fn alias_trailing_none_values() {
    let seq = AliasSequence {
        aliases: vec![Some("start".to_string()), None, None, None],
    };
    assert_eq!(seq.aliases.len(), 4);
    assert!(seq.aliases[0].is_some());
    for i in 1..4 {
        assert!(seq.aliases[i].is_none());
    }
}

#[test]
fn alias_leading_none_values() {
    let seq = AliasSequence {
        aliases: vec![None, None, None, Some("end".to_string())],
    };
    assert_eq!(seq.aliases.len(), 4);
    for i in 0..3 {
        assert!(seq.aliases[i].is_none());
    }
    assert!(seq.aliases[3].is_some());
}

#[test]
fn alias_alternating_some_none() {
    let seq = AliasSequence {
        aliases: vec![
            Some("a".to_string()),
            None,
            Some("c".to_string()),
            None,
            Some("e".to_string()),
        ],
    };
    for (i, alias) in seq.aliases.iter().enumerate() {
        if i % 2 == 0 {
            assert!(alias.is_some(), "position {i} should be Some");
        } else {
            assert!(alias.is_none(), "position {i} should be None");
        }
    }
}

#[test]
fn production_id_as_map_key() {
    let mut map = std::collections::HashMap::new();
    map.insert(ProductionId(1), "first");
    map.insert(ProductionId(2), "second");
    assert_eq!(map[&ProductionId(1)], "first");
    assert_eq!(map[&ProductionId(2)], "second");
}

#[test]
fn rule_id_as_map_key() {
    let mut map = std::collections::HashMap::new();
    map.insert(RuleId(10), ProductionId(100));
    map.insert(RuleId(20), ProductionId(200));
    assert_eq!(map[&RuleId(10)], ProductionId(100));
}

#[test]
fn grammar_alias_sequences_independent_of_rules() {
    let mut g = simple_grammar();
    let rule_count = g.all_rules().count();
    populate_alias_sequences(
        &mut g,
        vec![
            (0, vec![Some("x".to_string())]),
            (99, vec![Some("y".to_string())]),
        ],
    );
    assert_eq!(g.all_rules().count(), rule_count);
    assert_eq!(g.alias_sequences.len(), 2);
}
