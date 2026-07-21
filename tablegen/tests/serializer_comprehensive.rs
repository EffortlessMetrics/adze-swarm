//! Comprehensive tests for `adze_tablegen::serializer`.
//!
//! 50+ tests covering:
//! 1. Serialization/deserialization of parse tables
//! 2. JSON output format details
//! 3. Compressed table serialization
//! 4. Edge cases with empty/large tables
//! 5. Round-trip properties

use std::collections::BTreeMap;

use adze_glr_core::{
    Action, FirstFollowSets, GotoIndexing, LexMode, ParseTable, build_lr1_automaton,
};
use adze_ir::builder::GrammarBuilder;
use adze_ir::{
    ExternalToken, FieldId, Grammar, ProductionId, Rule, RuleId, StateId, Symbol, SymbolId, Token,
    TokenPattern,
};
use adze_tablegen::compress::{
    CompressedActionEntry, CompressedActionTable, CompressedGotoEntry, CompressedGotoTable,
    CompressedTables,
};
use adze_tablegen::serializer::{SerializableLanguage, SerializableLexState, serialize_language};
use adze_tablegen::{TableCompressor, collect_token_indices, eof_accepts_or_reduces};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const INVALID: StateId = StateId(u16::MAX);

/// Construct a minimal ParseTable suitable for unit-level tests.
fn make_empty_table(states: usize, terms: usize, nonterms: usize, externals: usize) -> ParseTable {
    let states = states.max(1);
    let eof_idx = 1 + terms + externals;
    let nonterms_eff = if nonterms == 0 { 1 } else { nonterms };
    let symbol_count = eof_idx + 1 + nonterms_eff;

    let actions = vec![vec![vec![]; symbol_count]; states];
    let gotos = vec![vec![INVALID; symbol_count]; states];

    let start_symbol = SymbolId((eof_idx + 1) as u16);
    let eof_symbol = SymbolId(eof_idx as u16);
    let token_count = eof_idx - externals;

    let mut symbol_to_index: BTreeMap<SymbolId, usize> = BTreeMap::new();
    for i in 0..symbol_count {
        symbol_to_index.insert(SymbolId(i as u16), i);
    }
    let mut nonterminal_to_index: BTreeMap<SymbolId, usize> = BTreeMap::new();
    nonterminal_to_index.insert(start_symbol, start_symbol.0 as usize);

    let mut index_to_symbol = vec![SymbolId(0); symbol_count];
    for (symbol_id, index) in &symbol_to_index {
        index_to_symbol[*index] = *symbol_id;
    }

    let lex_modes = vec![
        LexMode {
            lex_state: 0,
            external_lex_state: 0,
        };
        states
    ];

    ParseTable {
        action_table: actions,
        goto_table: gotos,
        rules: vec![],
        state_count: states,
        symbol_count,
        symbol_to_index,
        index_to_symbol,
        nonterminal_to_index,
        symbol_metadata: vec![],
        token_count,
        external_token_count: externals,
        eof_symbol,
        start_symbol,
        initial_state: StateId(0),
        lex_modes,
        extras: vec![],
        external_scanner_states: vec![],
        dynamic_prec_by_rule: vec![],
        rule_assoc_by_rule: vec![],
        alias_sequences: vec![],
        field_names: vec![],
        field_map: BTreeMap::new(),
        grammar: Grammar::default(),
        goto_indexing: GotoIndexing::NonterminalMap,
    }
}

fn grammar_with(token_names: &[&str], rule_names: &[&str]) -> Grammar {
    let mut grammar = Grammar::new("test".to_string());
    for (i, tname) in token_names.iter().enumerate() {
        grammar.tokens.insert(
            SymbolId((i + 1) as u16),
            Token {
                name: tname.to_string(),
                pattern: TokenPattern::String(tname.to_string()),
                fragile: false,
            },
        );
    }
    let base = token_names.len() + 1;
    for (i, rname) in rule_names.iter().enumerate() {
        let sid = SymbolId((base + i) as u16);
        grammar.rules.entry(sid).or_default().push(Rule {
            lhs: sid,
            rhs: vec![],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(i as u16),
        });
        grammar.rule_names.insert(sid, rname.to_string());
    }
    grammar
}

/// Build a `CompressedTables` with given action entries, goto entries, and offsets.
fn make_compressed(
    action_entries: Vec<CompressedActionEntry>,
    action_offsets: Vec<u16>,
    default_actions: Vec<Action>,
    goto_entries: Vec<CompressedGotoEntry>,
    goto_offsets: Vec<u16>,
) -> CompressedTables {
    CompressedTables {
        action_table: CompressedActionTable {
            data: action_entries,
            row_offsets: action_offsets,
            default_actions,
        },
        goto_table: CompressedGotoTable {
            data: goto_entries,
            row_offsets: goto_offsets,
        },
        small_table_threshold: 32768,
    }
}

// =========================================================================
// 1. serialize_language — basic serialization
// =========================================================================

#[test]
fn serialize_empty_grammar_returns_ok() {
    let grammar = Grammar::new("empty".to_string());
    let pt = make_empty_table(1, 0, 0, 0);
    assert!(serialize_language(&grammar, &pt, None).is_ok());
}

#[test]
fn serialize_returns_valid_json() {
    let grammar = grammar_with(&["a"], &["r"]);
    let pt = make_empty_table(1, 1, 1, 0);
    let json = serialize_language(&grammar, &pt, None).unwrap();
    let _: serde_json::Value = serde_json::from_str(&json).unwrap();
}

#[test]
fn serialize_with_no_compressed_yields_empty_parse_table_data() {
    let grammar = grammar_with(&["tok"], &[]);
    let pt = make_empty_table(1, 1, 0, 0);
    let json = serialize_language(&grammar, &pt, None).unwrap();
    let deser: SerializableLanguage = serde_json::from_str(&json).unwrap();
    assert!(deser.parse_table.is_empty());
    assert!(deser.small_parse_table_map.is_empty());
}

#[test]
fn serialize_with_compressed_tables_includes_action_data() {
    let grammar = grammar_with(&["x"], &["r"]);
    let pt = make_empty_table(1, 1, 1, 0);
    let compressed = make_compressed(
        vec![CompressedActionEntry::new(0, Action::Shift(StateId(1)))],
        vec![0, 1],
        vec![Action::Error],
        vec![],
        vec![0],
    );
    let json = serialize_language(&grammar, &pt, Some(&compressed)).unwrap();
    let deser: SerializableLanguage = serde_json::from_str(&json).unwrap();
    assert!(!deser.parse_table.is_empty());
}

#[test]
fn serialize_shift_action_encoding() {
    let compressed = make_compressed(
        vec![CompressedActionEntry::new(5, Action::Shift(StateId(42)))],
        vec![0, 1],
        vec![Action::Error],
        vec![],
        vec![0],
    );
    let grammar = grammar_with(&["t"], &[]);
    let pt = make_empty_table(1, 1, 0, 0);
    let json = serialize_language(&grammar, &pt, Some(&compressed)).unwrap();
    let deser: SerializableLanguage = serde_json::from_str(&json).unwrap();
    // Shift encodes symbol then state: [5, 42]
    assert_eq!(deser.parse_table[0], 5);
    assert_eq!(deser.parse_table[1], 42);
}

#[test]
fn serialize_reduce_action_encoding() {
    let compressed = make_compressed(
        vec![CompressedActionEntry::new(3, Action::Reduce(RuleId(0)))],
        vec![0, 1],
        vec![Action::Error],
        vec![],
        vec![0],
    );
    let grammar = grammar_with(&["t"], &[]);
    let pt = make_empty_table(1, 1, 0, 0);
    let json = serialize_language(&grammar, &pt, Some(&compressed)).unwrap();
    let deser: SerializableLanguage = serde_json::from_str(&json).unwrap();
    // Reduce uses 0x8000 | (rule_id + 1)
    assert_eq!(deser.parse_table[0], 3); // symbol
    assert_eq!(deser.parse_table[1], 0x8000 | 0x0001); // reduce rule 0 → 1-based
}

#[test]
fn serialize_accept_action_encoding() {
    let compressed = make_compressed(
        vec![CompressedActionEntry::new(0, Action::Accept)],
        vec![0, 1],
        vec![Action::Error],
        vec![],
        vec![0],
    );
    let grammar = Grammar::new("t".to_string());
    let pt = make_empty_table(1, 0, 0, 0);
    let json = serialize_language(&grammar, &pt, Some(&compressed)).unwrap();
    let deser: SerializableLanguage = serde_json::from_str(&json).unwrap();
    assert_eq!(deser.parse_table[1], 0xFFFF);
}

#[test]
fn serialize_error_action_encoding() {
    let compressed = make_compressed(
        vec![CompressedActionEntry::new(1, Action::Error)],
        vec![0, 1],
        vec![Action::Error],
        vec![],
        vec![0],
    );
    let grammar = Grammar::new("t".to_string());
    let pt = make_empty_table(1, 0, 0, 0);
    let json = serialize_language(&grammar, &pt, Some(&compressed)).unwrap();
    let deser: SerializableLanguage = serde_json::from_str(&json).unwrap();
    assert!(
        deser.parse_table.is_empty(),
        "error-only ABI rows are omitted from serialized small tables"
    );
}

#[test]
fn serialize_recover_action_encoding() {
    let compressed = make_compressed(
        vec![CompressedActionEntry::new(2, Action::Recover)],
        vec![0, 1],
        vec![Action::Error],
        vec![],
        vec![0],
    );
    let grammar = Grammar::new("t".to_string());
    let pt = make_empty_table(1, 0, 0, 0);
    let json = serialize_language(&grammar, &pt, Some(&compressed)).unwrap();
    let deser: SerializableLanguage = serde_json::from_str(&json).unwrap();
    assert_eq!(deser.parse_table[1], 0xFFFD);
}

#[test]
fn serialize_fork_action_encoding() {
    let fork = Action::Fork(vec![Action::Shift(StateId(1)), Action::Reduce(RuleId(0))]);
    let compressed = make_compressed(
        vec![CompressedActionEntry::new(4, fork)],
        vec![0, 1],
        vec![Action::Error],
        vec![],
        vec![0],
    );
    let grammar = Grammar::new("t".to_string());
    let pt = make_empty_table(1, 0, 0, 0);
    let json = serialize_language(&grammar, &pt, Some(&compressed)).unwrap();
    let deser: SerializableLanguage = serde_json::from_str(&json).unwrap();
    assert_eq!(
        deser.parse_table.len(),
        4,
        "fork must flatten to two ABI pairs"
    );
    assert_eq!(deser.parse_table[0], 4);
    assert_eq!(deser.parse_table[1], 1);
    assert_eq!(deser.parse_table[2], 4);
    assert_eq!(deser.parse_table[3], 0x8001);
}

// =========================================================================
// 2. JSON output format
// =========================================================================

#[test]
fn json_is_pretty_printed() {
    let grammar = Grammar::new("pp".to_string());
    let pt = make_empty_table(1, 0, 0, 0);
    let json = serialize_language(&grammar, &pt, None).unwrap();
    assert!(json.contains('\n'));
    assert!(json.contains("  ")); // indentation
}

#[test]
fn json_top_level_is_object() {
    let grammar = Grammar::new("obj".to_string());
    let pt = make_empty_table(1, 0, 0, 0);
    let json = serialize_language(&grammar, &pt, None).unwrap();
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert!(v.is_object());
}

#[test]
fn json_version_field_is_integer_15() {
    let grammar = Grammar::new("v".to_string());
    let pt = make_empty_table(1, 0, 0, 0);
    let json = serialize_language(&grammar, &pt, None).unwrap();
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(v["version"], 15);
}

#[test]
fn json_symbol_names_is_string_array() {
    let grammar = grammar_with(&["a", "b"], &["r"]);
    let pt = make_empty_table(1, 2, 1, 0);
    let json = serialize_language(&grammar, &pt, None).unwrap();
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    let arr = v["symbol_names"].as_array().unwrap();
    for item in arr {
        assert!(item.is_string());
    }
}

#[test]
fn json_lex_modes_is_array_of_objects() {
    let grammar = Grammar::new("lm".to_string());
    let pt = make_empty_table(3, 0, 0, 0);
    let json = serialize_language(&grammar, &pt, None).unwrap();
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    let arr = v["lex_modes"].as_array().unwrap();
    assert_eq!(arr.len(), 3);
    for item in arr {
        assert!(item.is_object());
        assert!(item["lex_state"].is_u64());
        assert!(item["external_lex_state"].is_u64());
    }
}

#[test]
fn json_parse_table_is_u16_array() {
    let compressed = make_compressed(
        vec![
            CompressedActionEntry::new(0, Action::Shift(StateId(1))),
            CompressedActionEntry::new(1, Action::Accept),
        ],
        vec![0, 2],
        vec![Action::Error],
        vec![],
        vec![0],
    );
    let grammar = Grammar::new("pt".to_string());
    let pt = make_empty_table(1, 0, 0, 0);
    let json = serialize_language(&grammar, &pt, Some(&compressed)).unwrap();
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    let arr = v["parse_table"].as_array().unwrap();
    for item in arr {
        assert!(item.is_u64());
        assert!(item.as_u64().unwrap() <= u16::MAX as u64);
    }
}

#[test]
fn json_small_parse_table_map_is_u32_array() {
    let compressed = make_compressed(vec![], vec![0, 0], vec![Action::Error], vec![], vec![0]);
    let grammar = Grammar::new("map".to_string());
    let pt = make_empty_table(1, 0, 0, 0);
    let json = serialize_language(&grammar, &pt, Some(&compressed)).unwrap();
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    let arr = v["small_parse_table_map"].as_array().unwrap();
    for item in arr {
        assert!(item.is_u64());
    }
}

#[test]
fn json_metadata_is_u8_array() {
    let grammar = grammar_with(&["a"], &["r"]);
    let pt = make_empty_table(1, 1, 1, 0);
    let json = serialize_language(&grammar, &pt, None).unwrap();
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    let arr = v["symbol_metadata"].as_array().unwrap();
    for item in arr {
        assert!(item.is_u64());
        assert!(item.as_u64().unwrap() <= 255);
    }
}

#[test]
fn json_contains_all_15_keys() {
    let grammar = Grammar::new("keys".to_string());
    let pt = make_empty_table(1, 0, 0, 0);
    let json = serialize_language(&grammar, &pt, None).unwrap();
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    let obj = v.as_object().unwrap();
    let expected = [
        "version",
        "symbol_count",
        "alias_count",
        "token_count",
        "external_token_count",
        "state_count",
        "large_state_count",
        "production_id_count",
        "field_count",
        "symbol_names",
        "field_names",
        "symbol_metadata",
        "parse_table",
        "small_parse_table_map",
        "lex_modes",
    ];
    assert_eq!(obj.len(), expected.len());
    for key in &expected {
        assert!(obj.contains_key(*key), "missing key: {key}");
    }
}

// =========================================================================
// 3. serialize_compressed_tables
// =========================================================================

#[test]
fn serialize_compressed_tables_empty() {
    let tables = make_compressed(vec![], vec![0], vec![], vec![], vec![0]);
    let json = adze_tablegen::serializer::serialize_compressed_tables(&tables).unwrap();
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert!(v["action_table"]["entries"].as_array().unwrap().is_empty());
    assert!(v["goto_table"]["entries"].as_array().unwrap().is_empty());
}

#[test]
fn serialize_compressed_tables_shift_action_format() {
    let tables = make_compressed(
        vec![CompressedActionEntry::new(1, Action::Shift(StateId(7)))],
        vec![0, 1],
        vec![Action::Error],
        vec![],
        vec![0],
    );
    let json = adze_tablegen::serializer::serialize_compressed_tables(&tables).unwrap();
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    let entries = v["action_table"]["entries"].as_array().unwrap();
    assert_eq!(entries.len(), 1);
    // Each entry is [symbol, action_str]
    let entry = entries[0].as_array().unwrap();
    assert_eq!(entry[0].as_u64().unwrap(), 1);
    assert_eq!(entry[1].as_str().unwrap(), "Shift(7)");
}

#[test]
fn serialize_compressed_tables_reduce_action_format() {
    let tables = make_compressed(
        vec![CompressedActionEntry::new(2, Action::Reduce(RuleId(3)))],
        vec![0, 1],
        vec![Action::Error],
        vec![],
        vec![0],
    );
    let json = adze_tablegen::serializer::serialize_compressed_tables(&tables).unwrap();
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    let entries = v["action_table"]["entries"].as_array().unwrap();
    let entry = entries[0].as_array().unwrap();
    assert_eq!(entry[1].as_str().unwrap(), "Reduce(3)");
}

#[test]
fn serialize_compressed_tables_accept_format() {
    let tables = make_compressed(
        vec![CompressedActionEntry::new(0, Action::Accept)],
        vec![0, 1],
        vec![Action::Accept],
        vec![],
        vec![0],
    );
    let json = adze_tablegen::serializer::serialize_compressed_tables(&tables).unwrap();
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    let entries = v["action_table"]["entries"].as_array().unwrap();
    assert_eq!(
        entries[0].as_array().unwrap()[1].as_str().unwrap(),
        "Accept"
    );
    let defaults = v["action_table"]["default_actions"].as_array().unwrap();
    assert_eq!(defaults[0].as_str().unwrap(), "Accept");
}

#[test]
fn serialize_compressed_tables_error_format() {
    let tables = make_compressed(
        vec![CompressedActionEntry::new(0, Action::Error)],
        vec![0, 1],
        vec![Action::Error],
        vec![],
        vec![0],
    );
    let json = adze_tablegen::serializer::serialize_compressed_tables(&tables).unwrap();
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    let entries = v["action_table"]["entries"].as_array().unwrap();
    assert_eq!(entries[0].as_array().unwrap()[1].as_str().unwrap(), "Error");
}

#[test]
fn serialize_compressed_tables_recover_format() {
    let tables = make_compressed(
        vec![CompressedActionEntry::new(0, Action::Recover)],
        vec![0, 1],
        vec![Action::Recover],
        vec![],
        vec![0],
    );
    let json = adze_tablegen::serializer::serialize_compressed_tables(&tables).unwrap();
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    let entries = v["action_table"]["entries"].as_array().unwrap();
    assert_eq!(
        entries[0].as_array().unwrap()[1].as_str().unwrap(),
        "Recover"
    );
}

#[test]
fn serialize_compressed_tables_fork_format() {
    let fork = Action::Fork(vec![
        Action::Shift(StateId(1)),
        Action::Reduce(RuleId(2)),
        Action::Accept,
    ]);
    let tables = make_compressed(
        vec![CompressedActionEntry::new(0, fork)],
        vec![0, 1],
        vec![Action::Error],
        vec![],
        vec![0],
    );
    let json = adze_tablegen::serializer::serialize_compressed_tables(&tables).unwrap();
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    let entries = v["action_table"]["entries"].as_array().unwrap();
    // Fork(3) because 3 sub-actions
    assert_eq!(
        entries[0].as_array().unwrap()[1].as_str().unwrap(),
        "Fork(3)"
    );
}

#[test]
fn serialize_compressed_tables_goto_single() {
    let tables = make_compressed(
        vec![],
        vec![0],
        vec![],
        vec![CompressedGotoEntry::Single(5)],
        vec![0, 1],
    );
    let json = adze_tablegen::serializer::serialize_compressed_tables(&tables).unwrap();
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    let entries = v["goto_table"]["entries"].as_array().unwrap();
    assert_eq!(entries[0].as_str().unwrap(), "Single(5)");
}

#[test]
fn serialize_compressed_tables_goto_run_length() {
    let tables = make_compressed(
        vec![],
        vec![0],
        vec![],
        vec![CompressedGotoEntry::RunLength {
            state: 3,
            count: 10,
        }],
        vec![0, 1],
    );
    let json = adze_tablegen::serializer::serialize_compressed_tables(&tables).unwrap();
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    let entries = v["goto_table"]["entries"].as_array().unwrap();
    assert_eq!(entries[0].as_str().unwrap(), "RunLength(3, 10)");
}

#[test]
fn serialize_compressed_tables_row_offsets_preserved() {
    let tables = make_compressed(
        vec![
            CompressedActionEntry::new(0, Action::Shift(StateId(1))),
            CompressedActionEntry::new(1, Action::Shift(StateId(2))),
            CompressedActionEntry::new(2, Action::Accept),
        ],
        vec![0, 2, 3],
        vec![Action::Error, Action::Error],
        vec![],
        vec![0],
    );
    let json = adze_tablegen::serializer::serialize_compressed_tables(&tables).unwrap();
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    let offsets = v["action_table"]["row_offsets"].as_array().unwrap();
    assert_eq!(offsets.len(), 3);
    assert_eq!(offsets[0].as_u64().unwrap(), 0);
    assert_eq!(offsets[1].as_u64().unwrap(), 2);
    assert_eq!(offsets[2].as_u64().unwrap(), 3);
}

#[test]
fn serialize_compressed_tables_small_table_threshold() {
    let tables = CompressedTables {
        action_table: CompressedActionTable {
            data: vec![],
            row_offsets: vec![0],
            default_actions: vec![],
        },
        goto_table: CompressedGotoTable {
            data: vec![],
            row_offsets: vec![0],
        },
        small_table_threshold: 99999,
    };
    let json = adze_tablegen::serializer::serialize_compressed_tables(&tables).unwrap();
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(v["small_table_threshold"].as_u64().unwrap(), 99999);
}

#[test]
fn serialize_compressed_tables_is_valid_json() {
    let tables = make_compressed(
        vec![CompressedActionEntry::new(0, Action::Accept)],
        vec![0, 1],
        vec![Action::Error],
        vec![CompressedGotoEntry::Single(0)],
        vec![0, 1],
    );
    let json = adze_tablegen::serializer::serialize_compressed_tables(&tables).unwrap();
    let _: serde_json::Value = serde_json::from_str(&json).unwrap();
}

#[test]
fn serialize_compressed_tables_is_pretty_printed() {
    let tables = make_compressed(vec![], vec![0], vec![], vec![], vec![0]);
    let json = adze_tablegen::serializer::serialize_compressed_tables(&tables).unwrap();
    assert!(json.contains('\n'));
}

// =========================================================================
// 4. Edge cases — empty and large tables
// =========================================================================

#[test]
fn empty_grammar_symbol_count_is_one() {
    // Only EOF symbol
    let grammar = Grammar::new("e".to_string());
    let pt = make_empty_table(1, 0, 0, 0);
    let json = serialize_language(&grammar, &pt, None).unwrap();
    let deser: SerializableLanguage = serde_json::from_str(&json).unwrap();
    // 1 for "end" (EOF)
    assert_eq!(deser.symbol_count, 1);
    assert_eq!(deser.symbol_names, vec!["end"]);
}

#[test]
fn empty_grammar_metadata_has_one_entry() {
    let grammar = Grammar::new("em".to_string());
    let pt = make_empty_table(1, 0, 0, 0);
    let json = serialize_language(&grammar, &pt, None).unwrap();
    let deser: SerializableLanguage = serde_json::from_str(&json).unwrap();
    assert_eq!(deser.symbol_metadata.len(), 1);
}

#[test]
fn single_state_lex_modes() {
    let grammar = Grammar::new("s".to_string());
    let pt = make_empty_table(1, 0, 0, 0);
    let json = serialize_language(&grammar, &pt, None).unwrap();
    let deser: SerializableLanguage = serde_json::from_str(&json).unwrap();
    assert_eq!(deser.lex_modes.len(), 1);
    assert_eq!(deser.lex_modes[0].lex_state, 0);
}

#[test]
fn many_tokens_serialization() {
    let mut grammar = Grammar::new("many".to_string());
    for i in 1..=500 {
        grammar.tokens.insert(
            SymbolId(i),
            Token {
                name: format!("t{i}"),
                pattern: TokenPattern::String(format!("{i}")),
                fragile: false,
            },
        );
    }
    let pt = make_empty_table(1, 500, 0, 0);
    let json = serialize_language(&grammar, &pt, None).unwrap();
    let deser: SerializableLanguage = serde_json::from_str(&json).unwrap();
    assert_eq!(deser.token_count, 500);
    // 1 (end) + 500 tokens
    assert!(deser.symbol_names.len() >= 501);
}

#[test]
fn many_rules_serialization() {
    let mut grammar = Grammar::new("rules".to_string());
    for i in 0..100 {
        let sid = SymbolId((i + 10) as u16);
        grammar.rules.entry(sid).or_default().push(Rule {
            lhs: sid,
            rhs: vec![],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(i as u16),
        });
        grammar.rule_names.insert(sid, format!("rule_{i}"));
    }
    let pt = make_empty_table(1, 0, 100, 0);
    let json = serialize_language(&grammar, &pt, None).unwrap();
    let deser: SerializableLanguage = serde_json::from_str(&json).unwrap();
    assert_eq!(deser.production_id_count, 100);
}

#[test]
fn many_fields_serialization() {
    let mut grammar = Grammar::new("fields".to_string());
    for i in 0..50 {
        grammar.fields.insert(FieldId(i), format!("field_{i:03}"));
    }
    let pt = make_empty_table(1, 0, 0, 0);
    let json = serialize_language(&grammar, &pt, None).unwrap();
    let deser: SerializableLanguage = serde_json::from_str(&json).unwrap();
    assert_eq!(deser.field_count, 50);
    assert_eq!(deser.field_names.len(), 50);
    // Fields are sorted lexicographically
    let mut sorted = deser.field_names.clone();
    sorted.sort();
    assert_eq!(deser.field_names, sorted);
}

#[test]
fn many_states_lex_modes_preserve_parse_table_values() {
    let grammar = Grammar::new("st".to_string());
    let mut pt = make_empty_table(1000, 0, 0, 0);
    for (i, mode) in pt.lex_modes.iter_mut().enumerate() {
        *mode = LexMode {
            lex_state: (i % 17) as u16,
            external_lex_state: (i % 5) as u16,
        };
    }
    let json = serialize_language(&grammar, &pt, None).unwrap();
    let deser: SerializableLanguage = serde_json::from_str(&json).unwrap();
    assert_eq!(deser.state_count, 1000);
    assert_eq!(deser.lex_modes.len(), 1000);
    for (i, mode) in deser.lex_modes.iter().enumerate() {
        assert_eq!(mode.lex_state, pt.lex_modes[i].lex_state);
        assert_eq!(mode.external_lex_state, pt.lex_modes[i].external_lex_state);
    }
}

#[test]
fn large_compressed_action_table() {
    let mut entries = Vec::new();
    for i in 0..200u16 {
        entries.push(CompressedActionEntry::new(i, Action::Shift(StateId(i))));
    }
    let tables = make_compressed(entries, vec![0, 200], vec![Action::Error], vec![], vec![0]);
    let json = adze_tablegen::serializer::serialize_compressed_tables(&tables).unwrap();
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(v["action_table"]["entries"].as_array().unwrap().len(), 200);
}

#[test]
fn large_goto_table_with_run_lengths() {
    let mut goto_entries = Vec::new();
    for i in 0..50u16 {
        goto_entries.push(CompressedGotoEntry::RunLength {
            state: i,
            count: 100,
        });
    }
    let tables = make_compressed(vec![], vec![0], vec![], goto_entries, vec![0, 50]);
    let json = adze_tablegen::serializer::serialize_compressed_tables(&tables).unwrap();
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(v["goto_table"]["entries"].as_array().unwrap().len(), 50);
}

// =========================================================================
// 5. Round-trip properties
// =========================================================================

#[test]
fn roundtrip_serializable_language_equality() {
    let lang = SerializableLanguage {
        version: 15,
        symbol_count: 5,
        alias_count: 1,
        token_count: 2,
        external_token_count: 1,
        state_count: 3,
        large_state_count: 0,
        production_id_count: 2,
        field_count: 2,
        symbol_names: vec![
            "end".into(),
            "a".into(),
            "b".into(),
            "r1".into(),
            "ext".into(),
        ],
        field_names: vec!["left".into(), "right".into()],
        symbol_metadata: vec![1, 3, 1, 3, 3],
        parse_table: vec![0, 1, 2, 0x8001, 0xFFFF],
        small_parse_table_map: vec![0, 5],
        lex_modes: vec![
            SerializableLexState {
                lex_state: 0,
                external_lex_state: 0,
            },
            SerializableLexState {
                lex_state: 1,
                external_lex_state: 1,
            },
            SerializableLexState {
                lex_state: 2,
                external_lex_state: 0,
            },
        ],
    };
    let json = serde_json::to_string(&lang).unwrap();
    let deser: SerializableLanguage = serde_json::from_str(&json).unwrap();
    assert_eq!(lang, deser);
}

#[test]
fn roundtrip_lex_state_all_zeros() {
    let state = SerializableLexState {
        lex_state: 0,
        external_lex_state: 0,
    };
    let json = serde_json::to_string(&state).unwrap();
    let deser: SerializableLexState = serde_json::from_str(&json).unwrap();
    assert_eq!(state, deser);
}

#[test]
fn roundtrip_lex_state_max_values() {
    let state = SerializableLexState {
        lex_state: u16::MAX,
        external_lex_state: u16::MAX,
    };
    let json = serde_json::to_string(&state).unwrap();
    let deser: SerializableLexState = serde_json::from_str(&json).unwrap();
    assert_eq!(state, deser);
}

#[test]
fn roundtrip_language_with_max_u32_counts() {
    let lang = SerializableLanguage {
        version: u32::MAX,
        symbol_count: u32::MAX,
        alias_count: u32::MAX,
        token_count: u32::MAX,
        external_token_count: u32::MAX,
        state_count: u32::MAX,
        large_state_count: u32::MAX,
        production_id_count: u32::MAX,
        field_count: u32::MAX,
        symbol_names: vec![],
        field_names: vec![],
        symbol_metadata: vec![],
        parse_table: vec![],
        small_parse_table_map: vec![],
        lex_modes: vec![],
    };
    let json = serde_json::to_string(&lang).unwrap();
    let deser: SerializableLanguage = serde_json::from_str(&json).unwrap();
    assert_eq!(lang, deser);
}

#[test]
fn roundtrip_language_empty_vectors() {
    let lang = SerializableLanguage {
        version: 15,
        symbol_count: 0,
        alias_count: 0,
        token_count: 0,
        external_token_count: 0,
        state_count: 0,
        large_state_count: 0,
        production_id_count: 0,
        field_count: 0,
        symbol_names: vec![],
        field_names: vec![],
        symbol_metadata: vec![],
        parse_table: vec![],
        small_parse_table_map: vec![],
        lex_modes: vec![],
    };
    let json = serde_json::to_string(&lang).unwrap();
    let deser: SerializableLanguage = serde_json::from_str(&json).unwrap();
    assert_eq!(lang, deser);
}

#[test]
fn triple_roundtrip_serialize_language() {
    let grammar = grammar_with(&["if", "else", "for"], &["stmt", "block"]);
    let pt = make_empty_table(3, 3, 2, 0);
    let json1 = serialize_language(&grammar, &pt, None).unwrap();
    let d1: SerializableLanguage = serde_json::from_str(&json1).unwrap();
    let json2 = serde_json::to_string_pretty(&d1).unwrap();
    let d2: SerializableLanguage = serde_json::from_str(&json2).unwrap();
    let json3 = serde_json::to_string_pretty(&d2).unwrap();
    assert_eq!(json2, json3);
    assert_eq!(d1, d2);
}

#[test]
fn deterministic_serialization_repeated() {
    let grammar = grammar_with(&["x", "y"], &["r"]);
    let pt = make_empty_table(2, 2, 1, 0);
    let results: Vec<String> = (0..5)
        .map(|_| serialize_language(&grammar, &pt, None).unwrap())
        .collect();
    for r in &results[1..] {
        assert_eq!(&results[0], r);
    }
}

// =========================================================================
// 6. Symbol metadata details
// =========================================================================

#[test]
fn eof_metadata_is_visible_not_named() {
    let grammar = Grammar::new("eof".to_string());
    let pt = make_empty_table(1, 0, 0, 0);
    let json = serialize_language(&grammar, &pt, None).unwrap();
    let deser: SerializableLanguage = serde_json::from_str(&json).unwrap();
    let eof_meta = deser.symbol_metadata[0];
    // visible=true → bit 0x01 set
    assert_ne!(eof_meta & 0x01, 0, "EOF should be visible");
    // named=false → bit 0x02 not set
    assert_eq!(eof_meta & 0x02, 0, "EOF should not be named");
}

#[test]
fn serialize_preserves_accept_on_eof_in_compressed_rows() {
    let mut grammar = GrammarBuilder::new("ser_acc_eof")
        .token("a", "a")
        .rule("start", vec!["a"])
        .start("start")
        .build();
    let ff = FirstFollowSets::compute_normalized(&mut grammar).expect("first/follow");
    let table = build_lr1_automaton(&grammar, &ff).expect("automaton");
    let token_indices = collect_token_indices(&grammar, &table);
    let start_empty = eof_accepts_or_reduces(&table);
    let compressed = TableCompressor::new()
        .compress(&table, &token_indices, start_empty)
        .expect("compress");

    let json = serialize_language(&grammar, &table, Some(&compressed)).expect("serialize");
    let deser: SerializableLanguage = serde_json::from_str(&json).expect("deserialize json");

    let eof_col = *table
        .symbol_to_index
        .get(&table.eof_symbol)
        .expect("EOF in symbol_to_index");
    let accepting_states: Vec<usize> = table
        .action_table
        .iter()
        .enumerate()
        .filter_map(|(state, row)| {
            row.get(eof_col).and_then(|cell| {
                cell.iter()
                    .any(|a| matches!(a, Action::Accept))
                    .then_some(state)
            })
        })
        .collect();

    assert!(
        !accepting_states.is_empty(),
        "source table must accept on EOF"
    );

    for state in accepting_states {
        let row_start = deser.small_parse_table_map[state] as usize;
        let row_end = deser.small_parse_table_map[state + 1] as usize;
        let row_words = &deser.parse_table[row_start..row_end];

        let has_accept_on_eof = row_words
            .chunks_exact(2)
            .any(|pair| pair[0] as usize == eof_col && pair[1] == 0xFFFF);
        assert!(
            has_accept_on_eof,
            "serialized table lost Accept-on-EOF for state {state}"
        );
    }
}

#[test]
fn visible_string_token_metadata() {
    let mut grammar = Grammar::new("vis".to_string());
    grammar.tokens.insert(
        SymbolId(1),
        Token {
            name: "plus".to_string(),
            pattern: TokenPattern::String("+".to_string()),
            fragile: false,
        },
    );
    let pt = make_empty_table(1, 1, 0, 0);
    let json = serialize_language(&grammar, &pt, None).unwrap();
    let deser: SerializableLanguage = serde_json::from_str(&json).unwrap();
    let idx = deser.symbol_names.iter().position(|n| n == "plus").unwrap();
    let meta = deser.symbol_metadata[idx];
    // visible=true, named=false (string pattern)
    assert_ne!(meta & 0x01, 0, "plus should be visible");
    assert_eq!(meta & 0x02, 0, "string token should not be named");
}

#[test]
fn hidden_token_metadata_underscore() {
    let mut grammar = Grammar::new("hid".to_string());
    grammar.tokens.insert(
        SymbolId(1),
        Token {
            name: "_ws".to_string(),
            pattern: TokenPattern::String(" ".to_string()),
            fragile: false,
        },
    );
    let pt = make_empty_table(1, 1, 0, 0);
    let json = serialize_language(&grammar, &pt, None).unwrap();
    let deser: SerializableLanguage = serde_json::from_str(&json).unwrap();
    let idx = deser.symbol_names.iter().position(|n| n == "_ws").unwrap();
    let meta = deser.symbol_metadata[idx];
    assert_eq!(meta & 0x01, 0, "_ws should not be visible");
}

#[test]
fn regex_token_is_named_and_visible() {
    let mut grammar = Grammar::new("rn".to_string());
    grammar.tokens.insert(
        SymbolId(1),
        Token {
            name: "identifier".to_string(),
            pattern: TokenPattern::Regex(r"[a-z]+".to_string()),
            fragile: false,
        },
    );
    let pt = make_empty_table(1, 1, 0, 0);
    let json = serialize_language(&grammar, &pt, None).unwrap();
    let deser: SerializableLanguage = serde_json::from_str(&json).unwrap();
    let idx = deser
        .symbol_names
        .iter()
        .position(|n| n == "identifier")
        .unwrap();
    let meta = deser.symbol_metadata[idx];
    assert_ne!(meta & 0x01, 0, "regex token should be visible");
    assert_ne!(meta & 0x02, 0, "regex token should be named");
}

#[test]
fn rule_metadata_visible_named() {
    let grammar = grammar_with(&[], &["expression"]);
    let pt = make_empty_table(1, 0, 1, 0);
    let json = serialize_language(&grammar, &pt, None).unwrap();
    let deser: SerializableLanguage = serde_json::from_str(&json).unwrap();
    let idx = deser
        .symbol_names
        .iter()
        .position(|n| n == "expression")
        .unwrap();
    let meta = deser.symbol_metadata[idx];
    assert_ne!(meta & 0x01, 0, "rule should be visible");
    assert_ne!(meta & 0x02, 0, "rule should be named");
}

#[test]
fn hidden_rule_metadata() {
    let grammar = grammar_with(&[], &["_hidden_rule"]);
    let pt = make_empty_table(1, 0, 1, 0);
    let json = serialize_language(&grammar, &pt, None).unwrap();
    let deser: SerializableLanguage = serde_json::from_str(&json).unwrap();
    let idx = deser
        .symbol_names
        .iter()
        .position(|n| n == "_hidden_rule")
        .unwrap();
    let meta = deser.symbol_metadata[idx];
    assert_eq!(meta & 0x01, 0, "hidden rule should not be visible");
    assert_eq!(meta & 0x02, 0, "hidden rule should not be named");
}

#[test]
fn supertype_rule_metadata() {
    let mut grammar = Grammar::new("st".to_string());
    let sid = SymbolId(10);
    grammar.rules.entry(sid).or_default().push(Rule {
        lhs: sid,
        rhs: vec![],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    });
    grammar.rule_names.insert(sid, "type_node".to_string());
    grammar.supertypes.push(sid);
    let pt = make_empty_table(1, 0, 1, 0);
    let json = serialize_language(&grammar, &pt, None).unwrap();
    let deser: SerializableLanguage = serde_json::from_str(&json).unwrap();
    let idx = deser
        .symbol_names
        .iter()
        .position(|n| n == "type_node")
        .unwrap();
    let meta = deser.symbol_metadata[idx];
    assert_ne!(meta & 0x10, 0, "supertype bit should be set");
}

#[test]
fn external_token_metadata() {
    let mut grammar = Grammar::new("ext".to_string());
    grammar.externals.push(ExternalToken {
        name: "indent".to_string(),
        symbol_id: SymbolId(100),
    });
    let pt = make_empty_table(1, 0, 0, 1);
    let json = serialize_language(&grammar, &pt, None).unwrap();
    let deser: SerializableLanguage = serde_json::from_str(&json).unwrap();
    let idx = deser
        .symbol_names
        .iter()
        .position(|n| n == "indent")
        .unwrap();
    let meta = deser.symbol_metadata[idx];
    assert_ne!(meta & 0x01, 0, "visible external should be visible");
    assert_ne!(meta & 0x02, 0, "visible external should be named");
}

#[test]
fn hidden_external_token_metadata() {
    let mut grammar = Grammar::new("hext".to_string());
    grammar.externals.push(ExternalToken {
        name: "_newline".to_string(),
        symbol_id: SymbolId(100),
    });
    let pt = make_empty_table(1, 0, 0, 1);
    let json = serialize_language(&grammar, &pt, None).unwrap();
    let deser: SerializableLanguage = serde_json::from_str(&json).unwrap();
    let idx = deser
        .symbol_names
        .iter()
        .position(|n| n == "_newline")
        .unwrap();
    let meta = deser.symbol_metadata[idx];
    assert_eq!(meta & 0x01, 0, "_newline should not be visible");
}

// =========================================================================
// 7. Counts consistency
// =========================================================================

#[test]
fn symbol_count_equals_names_length() {
    let grammar = grammar_with(&["a", "b", "c"], &["r1", "r2"]);
    let pt = make_empty_table(1, 3, 2, 0);
    let json = serialize_language(&grammar, &pt, None).unwrap();
    let deser: SerializableLanguage = serde_json::from_str(&json).unwrap();
    assert_eq!(deser.symbol_count as usize, deser.symbol_names.len());
}

#[test]
fn symbol_count_equals_metadata_length() {
    let grammar = grammar_with(&["a", "b"], &["r"]);
    let pt = make_empty_table(1, 2, 1, 0);
    let json = serialize_language(&grammar, &pt, None).unwrap();
    let deser: SerializableLanguage = serde_json::from_str(&json).unwrap();
    assert_eq!(deser.symbol_count as usize, deser.symbol_metadata.len());
}

#[test]
fn field_count_equals_field_names_length() {
    let mut grammar = Grammar::new("fc".to_string());
    grammar.fields.insert(FieldId(0), "a".to_string());
    grammar.fields.insert(FieldId(1), "b".to_string());
    grammar.fields.insert(FieldId(2), "c".to_string());
    let pt = make_empty_table(1, 0, 0, 0);
    let json = serialize_language(&grammar, &pt, None).unwrap();
    let deser: SerializableLanguage = serde_json::from_str(&json).unwrap();
    assert_eq!(deser.field_count as usize, deser.field_names.len());
}

#[test]
fn state_count_equals_lex_modes_length() {
    let grammar = Grammar::new("sc".to_string());
    let pt = make_empty_table(7, 0, 0, 0);
    let json = serialize_language(&grammar, &pt, None).unwrap();
    let deser: SerializableLanguage = serde_json::from_str(&json).unwrap();
    assert_eq!(deser.state_count as usize, deser.lex_modes.len());
}

#[test]
fn token_count_matches_grammar_tokens() {
    let grammar = grammar_with(&["a", "b", "c", "d"], &[]);
    let pt = make_empty_table(1, 4, 0, 0);
    let json = serialize_language(&grammar, &pt, None).unwrap();
    let deser: SerializableLanguage = serde_json::from_str(&json).unwrap();
    assert_eq!(deser.token_count, 4);
}

#[test]
fn external_token_count_matches() {
    let mut grammar = Grammar::new("etc".to_string());
    grammar.externals.push(ExternalToken {
        name: "ext1".to_string(),
        symbol_id: SymbolId(50),
    });
    grammar.externals.push(ExternalToken {
        name: "ext2".to_string(),
        symbol_id: SymbolId(51),
    });
    grammar.externals.push(ExternalToken {
        name: "ext3".to_string(),
        symbol_id: SymbolId(52),
    });
    let pt = make_empty_table(1, 0, 0, 3);
    let json = serialize_language(&grammar, &pt, None).unwrap();
    let deser: SerializableLanguage = serde_json::from_str(&json).unwrap();
    assert_eq!(deser.external_token_count, 3);
}

// =========================================================================
// 8. Symbol ordering
// =========================================================================

#[test]
fn tokens_ordered_by_symbol_id() {
    let mut grammar = Grammar::new("ord".to_string());
    // Insert out of order
    grammar.tokens.insert(
        SymbolId(5),
        Token {
            name: "five".to_string(),
            pattern: TokenPattern::String("5".to_string()),
            fragile: false,
        },
    );
    grammar.tokens.insert(
        SymbolId(1),
        Token {
            name: "one".to_string(),
            pattern: TokenPattern::String("1".to_string()),
            fragile: false,
        },
    );
    grammar.tokens.insert(
        SymbolId(3),
        Token {
            name: "three".to_string(),
            pattern: TokenPattern::String("3".to_string()),
            fragile: false,
        },
    );
    let pt = make_empty_table(1, 3, 0, 0);
    let json = serialize_language(&grammar, &pt, None).unwrap();
    let deser: SerializableLanguage = serde_json::from_str(&json).unwrap();
    assert_eq!(deser.symbol_names[0], "end");
    assert_eq!(deser.symbol_names[1], "one");
    assert_eq!(deser.symbol_names[2], "three");
    assert_eq!(deser.symbol_names[3], "five");
}

#[test]
fn rules_ordered_by_symbol_id() {
    let mut grammar = Grammar::new("rord".to_string());
    for (id, name) in [(20, "zeta"), (10, "alpha"), (15, "mu")] {
        let sid = SymbolId(id);
        grammar.rules.entry(sid).or_default().push(Rule {
            lhs: sid,
            rhs: vec![],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(0),
        });
        grammar.rule_names.insert(sid, name.to_string());
    }
    let pt = make_empty_table(1, 0, 3, 0);
    let json = serialize_language(&grammar, &pt, None).unwrap();
    let deser: SerializableLanguage = serde_json::from_str(&json).unwrap();
    // After "end", rules should be sorted by SymbolId: 10, 15, 20
    let rule_start = 1; // after "end"
    assert_eq!(deser.symbol_names[rule_start], "alpha");
    assert_eq!(deser.symbol_names[rule_start + 1], "mu");
    assert_eq!(deser.symbol_names[rule_start + 2], "zeta");
}

#[test]
fn fields_sorted_lexicographically() {
    let mut grammar = Grammar::new("fsort".to_string());
    grammar.fields.insert(FieldId(0), "zebra".to_string());
    grammar.fields.insert(FieldId(1), "apple".to_string());
    grammar.fields.insert(FieldId(2), "mango".to_string());
    let pt = make_empty_table(1, 0, 0, 0);
    let json = serialize_language(&grammar, &pt, None).unwrap();
    let deser: SerializableLanguage = serde_json::from_str(&json).unwrap();
    assert_eq!(deser.field_names, vec!["apple", "mango", "zebra"]);
}

// =========================================================================
// 9. Rule names fallback
// =========================================================================

#[test]
fn unnamed_rule_gets_generated_name() {
    let mut grammar = Grammar::new("unnamed".to_string());
    let sid = SymbolId(42);
    grammar.rules.entry(sid).or_default().push(Rule {
        lhs: sid,
        rhs: vec![],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    });
    // Don't insert into rule_names — should fallback to "rule_42"
    let pt = make_empty_table(1, 0, 1, 0);
    let json = serialize_language(&grammar, &pt, None).unwrap();
    let deser: SerializableLanguage = serde_json::from_str(&json).unwrap();
    assert!(deser.symbol_names.contains(&"rule_42".to_string()));
}

// =========================================================================
// 10. Multiple action types in compressed tables
// =========================================================================

#[test]
fn compressed_mixed_actions_serialization() {
    let entries = vec![
        CompressedActionEntry::new(0, Action::Shift(StateId(1))),
        CompressedActionEntry::new(1, Action::Reduce(RuleId(0))),
        CompressedActionEntry::new(2, Action::Accept),
        CompressedActionEntry::new(3, Action::Error),
        CompressedActionEntry::new(4, Action::Recover),
    ];
    let tables = make_compressed(entries, vec![0, 5], vec![Action::Error], vec![], vec![0]);
    let json = adze_tablegen::serializer::serialize_compressed_tables(&tables).unwrap();
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    let action_entries = v["action_table"]["entries"].as_array().unwrap();
    assert_eq!(action_entries.len(), 5);
    let actions: Vec<&str> = action_entries
        .iter()
        .map(|e| e.as_array().unwrap()[1].as_str().unwrap())
        .collect();
    assert_eq!(actions[0], "Shift(1)");
    assert_eq!(actions[1], "Reduce(0)");
    assert_eq!(actions[2], "Accept");
    assert_eq!(actions[3], "Error");
    assert_eq!(actions[4], "Recover");
}

#[test]
fn compressed_mixed_goto_entries() {
    let goto_entries = vec![
        CompressedGotoEntry::Single(1),
        CompressedGotoEntry::Single(2),
        CompressedGotoEntry::RunLength {
            state: 5,
            count: 20,
        },
        CompressedGotoEntry::Single(0),
    ];
    let tables = make_compressed(vec![], vec![0], vec![], goto_entries, vec![0, 4]);
    let json = adze_tablegen::serializer::serialize_compressed_tables(&tables).unwrap();
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    let entries = v["goto_table"]["entries"].as_array().unwrap();
    assert_eq!(entries.len(), 4);
    assert_eq!(entries[0].as_str().unwrap(), "Single(1)");
    assert_eq!(entries[1].as_str().unwrap(), "Single(2)");
    assert_eq!(entries[2].as_str().unwrap(), "RunLength(5, 20)");
    assert_eq!(entries[3].as_str().unwrap(), "Single(0)");
}

// =========================================================================
// 11. Multiple productions per rule
// =========================================================================

#[test]
fn multiple_productions_counted() {
    let mut grammar = Grammar::new("multi".to_string());
    let sid = SymbolId(10);
    for i in 0..5 {
        grammar.rules.entry(sid).or_default().push(Rule {
            lhs: sid,
            rhs: vec![Symbol::Terminal(SymbolId(1))],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(i),
        });
    }
    grammar.rule_names.insert(sid, "stmt".to_string());
    let pt = make_empty_table(1, 0, 1, 0);
    let json = serialize_language(&grammar, &pt, None).unwrap();
    let deser: SerializableLanguage = serde_json::from_str(&json).unwrap();
    assert_eq!(deser.production_id_count, 5);
}

// =========================================================================
// 12. SerializableLexState edge cases
// =========================================================================

#[test]
fn lex_state_json_format() {
    let state = SerializableLexState {
        lex_state: 10,
        external_lex_state: 3,
    };
    let json = serde_json::to_string(&state).unwrap();
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(v["lex_state"], 10);
    assert_eq!(v["external_lex_state"], 3);
}

#[test]
fn lex_state_debug_impl() {
    let state = SerializableLexState {
        lex_state: 1,
        external_lex_state: 2,
    };
    let debug = format!("{:?}", state);
    assert!(debug.contains("lex_state"));
    assert!(debug.contains("external_lex_state"));
}

// =========================================================================
// 13. Alias count is always zero
// =========================================================================

#[test]
fn alias_count_is_always_zero() {
    let grammar = grammar_with(&["a", "b"], &["r1", "r2"]);
    let pt = make_empty_table(2, 2, 2, 0);
    let json = serialize_language(&grammar, &pt, None).unwrap();
    let deser: SerializableLanguage = serde_json::from_str(&json).unwrap();
    assert_eq!(deser.alias_count, 0);
}

#[test]
fn large_state_count_is_always_zero() {
    let grammar = grammar_with(&["a"], &["r"]);
    let pt = make_empty_table(100, 1, 1, 0);
    let json = serialize_language(&grammar, &pt, None).unwrap();
    let deser: SerializableLanguage = serde_json::from_str(&json).unwrap();
    assert_eq!(deser.large_state_count, 0);
}
