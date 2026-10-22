//! Comprehensive tests for ProductionId and alias sequences in adze-ir.

use adze_ir::builder::GrammarBuilder;
use adze_ir::*;
use indexmap::IndexMap;
use std::collections::BTreeSet;

// ============================================================
// 1. ProductionId construction (8 tests)
// ============================================================

#[test]
fn test_production_id_zero() {
    let id = ProductionId(0);
    assert_eq!(id.0, 0);
}

#[test]
fn test_production_id_max() {
    let id = ProductionId(u16::MAX);
    assert_eq!(id.0, u16::MAX);
}

#[test]
fn test_production_id_inner_value_accessible() {
    let id = ProductionId(42);
    assert_eq!(id.0, 42);
}

#[test]
fn test_production_id_display() {
    let id = ProductionId(7);
    assert_eq!(format!("{id}"), "Production(7)");
}

#[test]
fn test_production_id_display_zero() {
    let id = ProductionId(0);
    assert_eq!(format!("{id}"), "Production(0)");
}

#[test]
fn test_production_id_display_max() {
    let id = ProductionId(u16::MAX);
    assert_eq!(format!("{id}"), format!("Production({})", u16::MAX));
}

#[test]
fn test_production_id_debug() {
    let id = ProductionId(99);
    let dbg = format!("{id:?}");
    assert!(dbg.contains("99"));
    assert!(dbg.contains("ProductionId"));
}

#[test]
fn test_production_id_copy_semantics() {
    let id = ProductionId(10);
    let id2 = id; // Copy, not move
    assert_eq!(id, id2);
}

// ============================================================
// 2. ProductionId comparison and ordering (5 tests)
// ============================================================

#[test]
fn test_production_id_equality() {
    assert_eq!(ProductionId(5), ProductionId(5));
    assert_ne!(ProductionId(5), ProductionId(6));
}

#[test]
fn test_production_id_ordering() {
    assert!(ProductionId(0) < ProductionId(1));
    assert!(ProductionId(100) > ProductionId(50));
}

#[test]
fn test_production_id_ord_consistency() {
    let mut ids = vec![ProductionId(3), ProductionId(1), ProductionId(2)];
    ids.sort();
    assert_eq!(ids, [ProductionId(1), ProductionId(2), ProductionId(3)]);
}

#[test]
fn test_production_id_hash_in_set() {
    let mut set = std::collections::HashSet::new();
    set.insert(ProductionId(1));
    set.insert(ProductionId(2));
    set.insert(ProductionId(1)); // duplicate
    assert_eq!(set.len(), 2);
}

#[test]
fn test_production_id_btreeset_ordering() {
    let mut set = BTreeSet::new();
    set.insert(ProductionId(10));
    set.insert(ProductionId(2));
    set.insert(ProductionId(7));
    let ordered: Vec<_> = set.into_iter().collect();
    assert_eq!(
        ordered,
        [ProductionId(2), ProductionId(7), ProductionId(10)]
    );
}

// ============================================================
// 3. Alias sequences (8 tests)
// ============================================================

#[test]
fn test_alias_sequence_empty() {
    let seq = AliasSequence {
        aliases: Vec::new(),
    };
    assert!(seq.aliases.is_empty());
}

#[test]
fn test_alias_sequence_all_none() {
    let seq = AliasSequence {
        aliases: vec![None, None, None],
    };
    assert_eq!(seq.aliases.len(), 3);
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
    assert_eq!(seq.aliases.len(), 3);
    assert!(seq.aliases.iter().all(|a| a.is_some()));
}

#[test]
fn test_alias_sequence_mixed() {
    let seq = AliasSequence {
        aliases: vec![
            Some("expression".to_string()),
            None,
            Some("body".to_string()),
        ],
    };
    assert_eq!(seq.aliases[0].as_deref(), Some("expression"));
    assert_eq!(seq.aliases[1], None);
    assert_eq!(seq.aliases[2].as_deref(), Some("body"));
}

#[test]
fn test_alias_sequence_clone_equality() {
    let seq = AliasSequence {
        aliases: vec![Some("x".to_string()), None],
    };
    let seq2 = seq.clone();
    assert_eq!(seq, seq2);
}

#[test]
fn test_alias_sequence_inequality() {
    let a = AliasSequence {
        aliases: vec![Some("x".to_string())],
    };
    let b = AliasSequence {
        aliases: vec![Some("y".to_string())],
    };
    assert_ne!(a, b);
}

#[test]
fn test_alias_sequence_single_element() {
    let seq = AliasSequence {
        aliases: vec![Some("alias_name".to_string())],
    };
    assert_eq!(seq.aliases.len(), 1);
    assert_eq!(seq.aliases[0].as_deref(), Some("alias_name"));
}

#[test]
fn test_alias_sequence_debug_output() {
    let seq = AliasSequence {
        aliases: vec![Some("test".to_string()), None],
    };
    let dbg = format!("{seq:?}");
    assert!(dbg.contains("test"));
    assert!(dbg.contains("None"));
}

// ============================================================
// 4. Grammar production_ids (8 tests)
// ============================================================

#[test]
fn test_grammar_new_has_empty_production_ids() {
    let grammar = Grammar::new("test".to_string());
    assert!(grammar.production_ids.is_empty());
}

#[test]
fn test_grammar_production_ids_insert_single() {
    let mut grammar = Grammar::new("test".to_string());
    grammar.production_ids.insert(RuleId(0), ProductionId(100));
    assert_eq!(grammar.production_ids.len(), 1);
    assert_eq!(grammar.production_ids[&RuleId(0)], ProductionId(100));
}

#[test]
fn test_grammar_production_ids_insert_multiple() {
    let mut grammar = Grammar::new("test".to_string());
    grammar.production_ids.insert(RuleId(0), ProductionId(10));
    grammar.production_ids.insert(RuleId(1), ProductionId(20));
    grammar.production_ids.insert(RuleId(2), ProductionId(30));
    assert_eq!(grammar.production_ids.len(), 3);
}

#[test]
fn test_grammar_production_ids_overwrite() {
    let mut grammar = Grammar::new("test".to_string());
    grammar.production_ids.insert(RuleId(0), ProductionId(10));
    grammar.production_ids.insert(RuleId(0), ProductionId(99));
    assert_eq!(grammar.production_ids.len(), 1);
    assert_eq!(grammar.production_ids[&RuleId(0)], ProductionId(99));
}

#[test]
fn test_grammar_production_ids_contains_key() {
    let mut grammar = Grammar::new("test".to_string());
    grammar.production_ids.insert(RuleId(5), ProductionId(55));
    assert!(grammar.production_ids.contains_key(&RuleId(5)));
    assert!(!grammar.production_ids.contains_key(&RuleId(6)));
}

#[test]
fn test_grammar_production_ids_iteration_order() {
    let mut grammar = Grammar::new("test".to_string());
    grammar.production_ids.insert(RuleId(3), ProductionId(30));
    grammar.production_ids.insert(RuleId(1), ProductionId(10));
    grammar.production_ids.insert(RuleId(2), ProductionId(20));
    // IndexMap preserves insertion order
    let keys: Vec<_> = grammar.production_ids.keys().collect();
    assert_eq!(keys, [&RuleId(3), &RuleId(1), &RuleId(2)]);
}

#[test]
fn test_grammar_production_ids_get_returns_none_for_missing() {
    let grammar = Grammar::new("test".to_string());
    assert!(grammar.production_ids.get(&RuleId(999)).is_none());
}

#[test]
fn test_grammar_production_ids_from_builder_are_total() {
    let grammar = GrammarBuilder::new("example")
        .token("NUM", r"\d+")
        .rule("start", vec!["NUM"])
        .start("start")
        .build();
    assert_eq!(grammar.production_ids.len(), grammar.all_rules().count());
    assert_eq!(
        grammar.production_id_for_rule(RuleId(0)),
        grammar.all_rules().next().map(|rule| rule.production_id)
    );
}

// ============================================================
// 5. max_alias_sequence_length (5 tests)
// ============================================================

#[test]
fn test_max_alias_sequence_length_default_zero() {
    let grammar = Grammar::new("test".to_string());
    assert_eq!(grammar.max_alias_sequence_length, 0);
}

#[test]
fn test_max_alias_sequence_length_set_directly() {
    let mut grammar = Grammar::new("test".to_string());
    grammar.max_alias_sequence_length = 5;
    assert_eq!(grammar.max_alias_sequence_length, 5);
}

#[test]
fn test_max_alias_sequence_length_matches_longest_sequence() {
    let mut grammar = Grammar::new("test".to_string());
    grammar.alias_sequences.insert(
        ProductionId(0),
        AliasSequence {
            aliases: vec![None, None],
        },
    );
    grammar.alias_sequences.insert(
        ProductionId(1),
        AliasSequence {
            aliases: vec![None, None, None, None],
        },
    );
    // Manually track the max
    grammar.max_alias_sequence_length = grammar
        .alias_sequences
        .values()
        .map(|s| s.aliases.len())
        .max()
        .unwrap_or(0);
    assert_eq!(grammar.max_alias_sequence_length, 4);
}

#[test]
fn test_max_alias_sequence_length_builder_starts_zero() {
    let grammar = GrammarBuilder::new("test")
        .token("A", "a")
        .rule("r", vec!["A"])
        .start("r")
        .build();
    assert_eq!(grammar.max_alias_sequence_length, 0);
}

#[test]
fn test_max_alias_sequence_length_single_sequence() {
    let mut grammar = Grammar::new("test".to_string());
    grammar.alias_sequences.insert(
        ProductionId(0),
        AliasSequence {
            aliases: vec![Some("name".to_string())],
        },
    );
    grammar.max_alias_sequence_length = 1;
    assert_eq!(grammar.max_alias_sequence_length, 1);
}

// ============================================================
// 6. Serialization roundtrip (5 tests)
// ============================================================

#[test]
fn test_production_id_serde_json_roundtrip() {
    let id = ProductionId(42);
    let json = serde_json::to_string(&id).unwrap();
    let deserialized: ProductionId = serde_json::from_str(&json).unwrap();
    assert_eq!(id, deserialized);
}

#[test]
fn test_production_id_serde_zero_roundtrip() {
    let id = ProductionId(0);
    let json = serde_json::to_string(&id).unwrap();
    let deserialized: ProductionId = serde_json::from_str(&json).unwrap();
    assert_eq!(id, deserialized);
}

#[test]
fn test_alias_sequence_serde_roundtrip() {
    let seq = AliasSequence {
        aliases: vec![Some("node".to_string()), None, Some("leaf".to_string())],
    };
    let json = serde_json::to_string(&seq).unwrap();
    let deserialized: AliasSequence = serde_json::from_str(&json).unwrap();
    assert_eq!(seq, deserialized);
}

#[test]
fn test_grammar_with_alias_data_serde_roundtrip() {
    let mut grammar = Grammar::new("serde_test".to_string());
    grammar.production_ids.insert(RuleId(0), ProductionId(10));
    grammar.production_ids.insert(RuleId(1), ProductionId(20));
    grammar.alias_sequences.insert(
        ProductionId(10),
        AliasSequence {
            aliases: vec![Some("expr".to_string()), None],
        },
    );
    grammar.max_alias_sequence_length = 2;

    let json = serde_json::to_string(&grammar).unwrap();
    let deserialized: Grammar = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.production_ids.len(), 2);
    assert_eq!(deserialized.production_ids[&RuleId(0)], ProductionId(10));
    assert_eq!(deserialized.alias_sequences.len(), 1);
    assert_eq!(deserialized.max_alias_sequence_length, 2);
}

#[test]
fn test_production_id_bincode_roundtrip() {
    let id = ProductionId(255);
    let encoded = postcard::to_allocvec(&id).unwrap();
    let decoded: ProductionId = postcard::from_bytes(&encoded).unwrap();
    assert_eq!(id, decoded);
}

// ============================================================
// 7. Builder alias API (8 tests)
// ============================================================

#[test]
fn test_builder_produces_production_ids_on_rules() {
    let grammar = GrammarBuilder::new("test")
        .token("A", "a")
        .token("B", "b")
        .rule("start", vec!["A"])
        .rule("start", vec!["B"])
        .start("start")
        .build();

    // Builder assigns sequential ProductionIds to rules
    let all_ids: Vec<_> = grammar.all_rules().map(|r| r.production_id).collect();
    assert_eq!(all_ids.len(), 2);
    assert_eq!(all_ids[0], ProductionId(0));
    assert_eq!(all_ids[1], ProductionId(1));
}

#[test]
fn test_builder_production_ids_sequential() {
    let grammar = GrammarBuilder::new("seq")
        .token("X", "x")
        .token("Y", "y")
        .token("Z", "z")
        .rule("a", vec!["X"])
        .rule("b", vec!["Y"])
        .rule("c", vec!["Z"])
        .start("a")
        .build();

    let ids: Vec<_> = grammar.all_rules().map(|r| r.production_id.0).collect();
    // Should be sequential
    for (i, id) in ids.iter().enumerate() {
        assert_eq!(*id as usize, i);
    }
}

#[test]
fn test_builder_alias_sequences_empty_by_default() {
    let grammar = GrammarBuilder::new("test")
        .token("A", "a")
        .rule("r", vec!["A"])
        .start("r")
        .build();
    assert!(grammar.alias_sequences.is_empty());
}

#[test]
fn test_builder_with_precedence_still_assigns_ids() {
    let grammar = GrammarBuilder::new("prec")
        .token("NUM", r"\d+")
        .token("+", "+")
        .rule_with_precedence("expr", vec!["expr", "+", "expr"], 1, Associativity::Left)
        .rule("expr", vec!["NUM"])
        .start("expr")
        .build();

    let ids: Vec<_> = grammar.all_rules().map(|r| r.production_id).collect();
    assert_eq!(ids.len(), 2);
    // First rule gets id 0, second gets id 1
    assert_eq!(ids[0], ProductionId(0));
    assert_eq!(ids[1], ProductionId(1));
}

#[test]
fn test_builder_multiple_alternatives_unique_ids() {
    let grammar = GrammarBuilder::new("alt")
        .token("A", "a")
        .token("B", "b")
        .token("C", "c")
        .rule("item", vec!["A"])
        .rule("item", vec!["B"])
        .rule("item", vec!["C"])
        .start("item")
        .build();

    let ids: Vec<_> = grammar.all_rules().map(|r| r.production_id).collect();
    // All unique
    let unique: std::collections::HashSet<_> = ids.iter().collect();
    assert_eq!(unique.len(), ids.len());
}

#[test]
fn test_builder_javascript_like_has_production_ids() {
    let grammar = GrammarBuilder::javascript_like();
    let count = grammar.all_rules().count();
    assert!(count > 0);
    // Every rule should have a production_id
    let ids: Vec<_> = grammar.all_rules().map(|r| r.production_id).collect();
    assert_eq!(ids.len(), count);
}

#[test]
fn test_builder_python_like_has_production_ids() {
    let grammar = GrammarBuilder::python_like();
    let ids: Vec<_> = grammar.all_rules().map(|r| r.production_id).collect();
    assert!(!ids.is_empty());
    // All sequential from 0
    for (i, id) in ids.iter().enumerate() {
        assert_eq!(id.0 as usize, i);
    }
}

#[test]
fn test_builder_empty_rule_gets_production_id() {
    let grammar = GrammarBuilder::new("nullable")
        .rule("empty", vec![])
        .start("empty")
        .build();

    let rules: Vec<_> = grammar.all_rules().collect();
    assert_eq!(rules.len(), 1);
    assert_eq!(rules[0].production_id, ProductionId(0));
}

// ============================================================
// 8. Edge cases (8 tests)
// ============================================================

#[test]
fn test_production_id_as_indexmap_key() {
    let mut map: IndexMap<ProductionId, String> = IndexMap::new();
    map.insert(ProductionId(0), "first".to_string());
    map.insert(ProductionId(1), "second".to_string());
    assert_eq!(map[&ProductionId(0)], "first");
    assert_eq!(map[&ProductionId(1)], "second");
}

#[test]
fn test_alias_sequence_with_empty_string_alias() {
    let seq = AliasSequence {
        aliases: vec![Some(String::new()), None],
    };
    assert_eq!(seq.aliases[0].as_deref(), Some(""));
}

#[test]
fn test_alias_sequence_with_unicode_alias() {
    let seq = AliasSequence {
        aliases: vec![Some("表达式".to_string()), Some("café".to_string())],
    };
    assert_eq!(seq.aliases[0].as_deref(), Some("表达式"));
    assert_eq!(seq.aliases[1].as_deref(), Some("café"));
}

#[test]
fn test_grammar_alias_sequences_multiple_productions() {
    let mut grammar = Grammar::new("multi".to_string());
    for i in 0..10u16 {
        grammar.alias_sequences.insert(
            ProductionId(i),
            AliasSequence {
                aliases: vec![Some(format!("alias_{i}"))],
            },
        );
    }
    assert_eq!(grammar.alias_sequences.len(), 10);
    assert_eq!(
        grammar.alias_sequences[&ProductionId(5)].aliases[0].as_deref(),
        Some("alias_5")
    );
}

#[test]
fn test_production_id_used_in_rule_struct() {
    let rule = Rule {
        lhs: SymbolId(1),
        rhs: vec![Symbol::Terminal(SymbolId(2))],
        precedence: None,
        associativity: None,
        fields: Vec::new(),
        production_id: ProductionId(42),
    };
    assert_eq!(rule.production_id, ProductionId(42));
}

#[test]
fn test_grammar_default_has_zero_alias_length() {
    let grammar = Grammar::default();
    assert_eq!(grammar.max_alias_sequence_length, 0);
    assert!(grammar.alias_sequences.is_empty());
    assert!(grammar.production_ids.is_empty());
}

#[test]
fn test_alias_sequence_long() {
    let aliases: Vec<Option<String>> = (0..100)
        .map(|i| {
            if i % 2 == 0 {
                Some(format!("pos_{i}"))
            } else {
                None
            }
        })
        .collect();
    let seq = AliasSequence { aliases };
    assert_eq!(seq.aliases.len(), 100);
    assert_eq!(seq.aliases[0].as_deref(), Some("pos_0"));
    assert!(seq.aliases[1].is_none());
    assert_eq!(seq.aliases[98].as_deref(), Some("pos_98"));
}

#[test]
fn test_rule_id_to_production_id_mapping_consistency() {
    let mut grammar = Grammar::new("mapping".to_string());
    // Simulate a mapping where multiple rules map to the same production
    grammar.production_ids.insert(RuleId(0), ProductionId(100));
    grammar.production_ids.insert(RuleId(1), ProductionId(100));
    grammar.production_ids.insert(RuleId(2), ProductionId(200));

    // Two different rules can map to the same production id
    assert_eq!(
        grammar.production_ids[&RuleId(0)],
        grammar.production_ids[&RuleId(1)]
    );
    assert_ne!(
        grammar.production_ids[&RuleId(0)],
        grammar.production_ids[&RuleId(2)]
    );
}
