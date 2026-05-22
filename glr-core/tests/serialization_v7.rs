//! ParseTable serialization v7 comprehensive tests.
//!
//! 64 tests across 8 categories (8 per category):
//!   serial_basic_*         — fundamental serialize/deserialize smoke tests
//!   serial_roundtrip_*     — field-level roundtrip fidelity
//!   serial_version_*       — format version validation
//!   serial_size_*          — byte-size properties and bounds
//!   serial_complex_*       — complex grammars and multi-action cells
//!   serial_deterministic_* — determinism / idempotence
//!   serial_corrupt_*       — corrupted / invalid input handling
//!   serial_edge_*          — boundary and edge-case coverage
//!
//! Run with:
//!   cargo test -p adze-glr-core --test serialization_v7 --features serialization -- --test-threads=2

#![cfg(feature = "serialization")]

use adze_glr_core::serialization::{DeserializationError, PARSE_TABLE_FORMAT_VERSION};
use adze_glr_core::{Action, FirstFollowSets, ParseTable, StateId, build_lr1_automaton};
use adze_ir::builder::GrammarBuilder;
use adze_ir::{Grammar, RuleId, SymbolId};
use std::collections::BTreeMap;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn build_table(grammar: &Grammar) -> ParseTable {
    let ff = FirstFollowSets::compute(grammar).expect("FIRST/FOLLOW");
    build_lr1_automaton(grammar, &ff).expect("automaton construction")
}

fn roundtrip(table: &ParseTable) -> ParseTable {
    let bytes = table.to_bytes().expect("serialize");
    ParseTable::from_bytes(&bytes).expect("deserialize")
}

fn make_action_table(actions: Vec<Action>) -> ParseTable {
    let mut sym_to_idx = BTreeMap::new();
    sym_to_idx.insert(SymbolId(0), 0);
    ParseTable {
        action_table: vec![vec![actions]],
        state_count: 1,
        symbol_count: 1,
        index_to_symbol: vec![SymbolId(0)],
        symbol_to_index: sym_to_idx,
        ..Default::default()
    }
}

fn craft_versioned_bytes(version: u32, inner_data: &[u8]) -> Vec<u8> {
    #[derive(serde::Serialize)]
    struct Wrapper {
        version: u32,
        data: Vec<u8>,
    }
    let w = Wrapper {
        version,
        data: inner_data.to_vec(),
    };
    postcard::to_stdvec(&w).expect("encode wrapper")
}

fn replace_version_in(valid_bytes: &[u8], new_version: u32) -> Vec<u8> {
    #[derive(serde::Deserialize)]
    struct WrapperDe {
        #[expect(
            dead_code,
            reason = "decoded wrapper version preserves the wire shape but is not read"
        )]
        version: u32,
        data: Vec<u8>,
    }
    #[derive(serde::Serialize)]
    struct WrapperSe {
        version: u32,
        data: Vec<u8>,
    }
    let original: WrapperDe = postcard::from_bytes(valid_bytes).expect("decode");
    let replaced = WrapperSe {
        version: new_version,
        data: original.data,
    };
    postcard::to_stdvec(&replaced).expect("re-encode")
}

// ---------------------------------------------------------------------------
// Grammar factories
// ---------------------------------------------------------------------------

/// S -> a
fn single_token_grammar() -> Grammar {
    GrammarBuilder::new("single_tok")
        .token("a", "a")
        .rule("S", vec!["a"])
        .start("S")
        .build()
}

/// S -> a | b
fn two_alt_grammar() -> Grammar {
    GrammarBuilder::new("two_alt")
        .token("a", "a")
        .token("b", "b")
        .rule("S", vec!["a"])
        .rule("S", vec!["b"])
        .start("S")
        .build()
}

/// S -> A, A -> a
fn chain_grammar() -> Grammar {
    GrammarBuilder::new("chain")
        .token("a", "a")
        .rule("S", vec!["A"])
        .rule("A", vec!["a"])
        .start("S")
        .build()
}

/// E -> E + E | n (ambiguous)
fn expr_grammar() -> Grammar {
    GrammarBuilder::new("expr")
        .token("n", r"\d+")
        .token("+", r"\+")
        .rule("E", vec!["E", "+", "E"])
        .rule("E", vec!["n"])
        .start("E")
        .build()
}

/// S -> a b
fn seq_grammar() -> Grammar {
    GrammarBuilder::new("seq")
        .token("a", "a")
        .token("b", "b")
        .rule("S", vec!["a", "b"])
        .start("S")
        .build()
}

/// S -> L R, L -> a, R -> b
fn two_nt_grammar() -> Grammar {
    GrammarBuilder::new("two_nt")
        .token("a", "a")
        .token("b", "b")
        .rule("S", vec!["L", "R"])
        .rule("L", vec!["a"])
        .rule("R", vec!["b"])
        .start("S")
        .build()
}

/// list -> a list | a (right-recursive)
fn right_recursive_grammar() -> Grammar {
    GrammarBuilder::new("right_rec")
        .token("a", "a")
        .rule("list", vec!["a", "list"])
        .rule("list", vec!["a"])
        .start("list")
        .build()
}

/// E -> E + T | T, T -> T * F | F, F -> n | ( E )
fn arithmetic_grammar() -> Grammar {
    GrammarBuilder::new("arith")
        .token("n", r"\d+")
        .token("+", r"\+")
        .token("*", r"\*")
        .token("(", r"\(")
        .token(")", r"\)")
        .rule("E", vec!["E", "+", "T"])
        .rule("E", vec!["T"])
        .rule("T", vec!["T", "*", "F"])
        .rule("T", vec!["F"])
        .rule("F", vec!["n"])
        .rule("F", vec!["(", "E", ")"])
        .start("E")
        .build()
}

/// S -> a | b | c | d | e
fn five_alt_grammar() -> Grammar {
    GrammarBuilder::new("five_alt")
        .token("a", "a")
        .token("b", "b")
        .token("c", "c")
        .token("d", "d")
        .token("e", "e")
        .rule("S", vec!["a"])
        .rule("S", vec!["b"])
        .rule("S", vec!["c"])
        .rule("S", vec!["d"])
        .rule("S", vec!["e"])
        .start("S")
        .build()
}

/// S -> A B C, A -> a, B -> b, C -> c
fn three_nt_chain_grammar() -> Grammar {
    GrammarBuilder::new("three_nt_chain")
        .token("a", "a")
        .token("b", "b")
        .token("c", "c")
        .rule("S", vec!["A", "B", "C"])
        .rule("A", vec!["a"])
        .rule("B", vec!["b"])
        .rule("C", vec!["c"])
        .start("S")
        .build()
}

/// S -> S a | a (left-recursive)
fn left_recursive_grammar() -> Grammar {
    GrammarBuilder::new("left_rec")
        .token("a", "a")
        .rule("S", vec!["S", "a"])
        .rule("S", vec!["a"])
        .start("S")
        .build()
}

/// S -> a S b | epsilon (nested)
fn nested_grammar() -> Grammar {
    GrammarBuilder::new("nested")
        .token("a", "a")
        .token("b", "b")
        .rule("S", vec!["a", "S", "b"])
        .rule("S", vec![])
        .start("S")
        .build()
}

/// stmts -> stmt stmts | stmt, stmt -> ID = NUM ;
fn statement_list_grammar() -> Grammar {
    GrammarBuilder::new("stmt_list")
        .token("ID", r"[a-z]+")
        .token("NUM", r"\d+")
        .token("=", "=")
        .token(";", ";")
        .rule("stmts", vec!["stmt", "stmts"])
        .rule("stmts", vec!["stmt"])
        .rule("stmt", vec!["ID", "=", "NUM", ";"])
        .start("stmts")
        .build()
}

/// S -> A B, A -> a | epsilon, B -> b
fn nullable_prefix_grammar() -> Grammar {
    GrammarBuilder::new("nullable_prefix")
        .token("a", "a")
        .token("b", "b")
        .rule("S", vec!["A", "B"])
        .rule("A", vec!["a"])
        .rule("A", vec![])
        .rule("B", vec!["b"])
        .start("S")
        .build()
}

/// S -> a b c d e f
fn wide_rhs_grammar() -> Grammar {
    GrammarBuilder::new("wide_rhs")
        .token("a", "a")
        .token("b", "b")
        .token("c", "c")
        .token("d", "d")
        .token("e", "e")
        .token("f", "f")
        .rule("S", vec!["a", "b", "c", "d", "e", "f"])
        .start("S")
        .build()
}

/// S -> A, A -> B, B -> C, C -> a (deep chain)
fn deep_chain_grammar() -> Grammar {
    GrammarBuilder::new("deep_chain")
        .token("a", "a")
        .rule("S", vec!["A"])
        .rule("A", vec!["B"])
        .rule("B", vec!["C"])
        .rule("C", vec!["a"])
        .start("S")
        .build()
}

// ===================================================================
// 1. serial_basic_* — fundamental serialize/deserialize smoke (8 tests)
// ===================================================================

#[test]
fn serial_basic_single_token_serializes() {
    let table = build_table(&single_token_grammar());
    let bytes = table.to_bytes().expect("serialize");
    assert!(!bytes.is_empty());
}

#[test]
fn serial_basic_single_token_deserializes() {
    let table = build_table(&single_token_grammar());
    let bytes = table.to_bytes().expect("serialize");
    let _restored = ParseTable::from_bytes(&bytes).expect("deserialize");
}

#[test]
fn serial_basic_default_table_serializes() {
    let table = ParseTable::default();
    let bytes = table.to_bytes().expect("serialize");
    assert!(!bytes.is_empty());
}

#[test]
fn serial_basic_default_table_roundtrips() {
    let table = ParseTable::default();
    let restored = roundtrip(&table);
    assert_eq!(table.state_count, restored.state_count);
    assert_eq!(table.symbol_count, restored.symbol_count);
}

#[test]
fn serial_basic_chain_grammar_roundtrips() {
    let table = build_table(&chain_grammar());
    let restored = roundtrip(&table);
    assert_eq!(table.state_count, restored.state_count);
}

#[test]
fn serial_basic_two_alt_serializes() {
    let table = build_table(&two_alt_grammar());
    let bytes = table.to_bytes().expect("serialize");
    assert!(bytes.len() > 1);
}

#[test]
fn serial_basic_arithmetic_roundtrips() {
    let table = build_table(&arithmetic_grammar());
    let restored = roundtrip(&table);
    assert_eq!(table.state_count, restored.state_count);
    assert_eq!(table.symbol_count, restored.symbol_count);
}

#[test]
fn serial_basic_expr_roundtrips() {
    let table = build_table(&expr_grammar());
    let restored = roundtrip(&table);
    assert_eq!(table.state_count, restored.state_count);
}

// ===================================================================
// 2. serial_roundtrip_* — field-level roundtrip fidelity (8 tests)
// ===================================================================

#[test]
fn serial_roundtrip_action_table_preserved() {
    let table = build_table(&arithmetic_grammar());
    let restored = roundtrip(&table);
    assert_eq!(table.action_table, restored.action_table);
}

#[test]
fn serial_roundtrip_goto_table_preserved() {
    let table = build_table(&two_nt_grammar());
    let restored = roundtrip(&table);
    assert_eq!(table.goto_table, restored.goto_table);
}

#[test]
fn serial_roundtrip_symbol_maps_preserved() {
    let table = build_table(&seq_grammar());
    let restored = roundtrip(&table);
    assert_eq!(table.symbol_to_index, restored.symbol_to_index);
    assert_eq!(table.index_to_symbol, restored.index_to_symbol);
}

#[test]
fn serial_roundtrip_nonterminal_index_preserved() {
    let table = build_table(&three_nt_chain_grammar());
    let restored = roundtrip(&table);
    assert_eq!(table.nonterminal_to_index, restored.nonterminal_to_index);
    assert_eq!(table.goto_indexing, restored.goto_indexing);
}

#[test]
fn serial_roundtrip_rules_preserved() {
    let table = build_table(&expr_grammar());
    let restored = roundtrip(&table);
    assert_eq!(table.rules.len(), restored.rules.len());
    for (a, b) in table.rules.iter().zip(restored.rules.iter()) {
        assert_eq!(a.lhs, b.lhs);
        assert_eq!(a.rhs_len, b.rhs_len);
    }
}

#[test]
fn serial_roundtrip_eof_and_start_preserved() {
    let table = build_table(&five_alt_grammar());
    let restored = roundtrip(&table);
    assert_eq!(table.eof_symbol, restored.eof_symbol);
    assert_eq!(table.start_symbol, restored.start_symbol);
}

#[test]
fn serial_roundtrip_extras_and_lex_modes_preserved() {
    let table = build_table(&right_recursive_grammar());
    let restored = roundtrip(&table);
    assert_eq!(table.extras, restored.extras);
    assert_eq!(table.lex_modes.len(), restored.lex_modes.len());
}

#[test]
fn serial_roundtrip_field_names_and_map_preserved() {
    let table = build_table(&statement_list_grammar());
    let restored = roundtrip(&table);
    assert_eq!(table.field_names, restored.field_names);
    assert_eq!(table.field_map, restored.field_map);
}

// ===================================================================
// 3. serial_version_* — format version validation (8 tests)
// ===================================================================

#[test]
fn serial_version_constant_equals_two() {
    assert_eq!(PARSE_TABLE_FORMAT_VERSION, 2);
}

#[test]
fn serial_version_constant_nonzero() {
    assert_ne!(PARSE_TABLE_FORMAT_VERSION, 0);
}

#[test]
fn serial_version_future_rejected() {
    let table = build_table(&single_token_grammar());
    let bytes = table.to_bytes().expect("serialize");
    let tampered = replace_version_in(&bytes, PARSE_TABLE_FORMAT_VERSION + 1);
    let err = ParseTable::from_bytes(&tampered).unwrap_err();
    match err {
        DeserializationError::IncompatibleVersion { expected, actual } => {
            assert_eq!(expected, PARSE_TABLE_FORMAT_VERSION);
            assert_eq!(actual, PARSE_TABLE_FORMAT_VERSION + 1);
        }
        other => panic!("expected IncompatibleVersion, got: {other}"),
    }
}

#[test]
fn serial_version_past_rejected() {
    let table = build_table(&chain_grammar());
    let bytes = table.to_bytes().expect("serialize");
    let tampered = replace_version_in(&bytes, 1);
    assert!(ParseTable::from_bytes(&tampered).is_err());
}

#[test]
fn serial_version_zero_rejected() {
    let table = build_table(&expr_grammar());
    let bytes = table.to_bytes().expect("serialize");
    let tampered = replace_version_in(&bytes, 0);
    assert!(ParseTable::from_bytes(&tampered).is_err());
}

#[test]
fn serial_version_u32_max_rejected() {
    let table = build_table(&two_alt_grammar());
    let bytes = table.to_bytes().expect("serialize");
    let tampered = replace_version_in(&bytes, u32::MAX);
    assert!(ParseTable::from_bytes(&tampered).is_err());
}

#[test]
fn serial_version_survives_double_roundtrip() {
    let table = build_table(&arithmetic_grammar());
    let b1 = table.to_bytes().expect("r1");
    let t2 = ParseTable::from_bytes(&b1).expect("d1");
    let b2 = t2.to_bytes().expect("r2");
    let t3 = ParseTable::from_bytes(&b2).expect("d2");
    let b3 = t3.to_bytes().expect("r3");
    assert_eq!(b1, b2);
    assert_eq!(b2, b3);
}

#[test]
fn serial_version_hundred_rejected() {
    let table = build_table(&seq_grammar());
    let bytes = table.to_bytes().expect("serialize");
    let tampered = replace_version_in(&bytes, 100);
    assert!(ParseTable::from_bytes(&tampered).is_err());
}

// ===================================================================
// 4. serial_size_* — byte-size properties and bounds (8 tests)
// ===================================================================

#[test]
fn serial_size_single_token_nonempty() {
    let bytes = build_table(&single_token_grammar()).to_bytes().expect("s");
    assert!(!bytes.is_empty());
}

#[test]
fn serial_size_arithmetic_under_100kb() {
    let bytes = build_table(&arithmetic_grammar()).to_bytes().expect("s");
    assert!(
        bytes.len() < 100_000,
        "serialized size {} exceeds 100KB",
        bytes.len()
    );
}

#[test]
fn serial_size_more_states_means_more_bytes() {
    let small = build_table(&single_token_grammar()).to_bytes().expect("s");
    let large = build_table(&arithmetic_grammar()).to_bytes().expect("s");
    assert!(
        large.len() > small.len(),
        "arithmetic ({}) should be larger than single_token ({})",
        large.len(),
        small.len()
    );
}

#[test]
fn serial_size_five_alt_larger_than_two_alt() {
    let two = build_table(&two_alt_grammar()).to_bytes().expect("s");
    let five = build_table(&five_alt_grammar()).to_bytes().expect("s");
    assert!(
        five.len() > two.len(),
        "five_alt ({}) should be larger than two_alt ({})",
        five.len(),
        two.len()
    );
}

#[test]
fn serial_size_default_table_minimal() {
    let bytes = ParseTable::default().to_bytes().expect("s");
    // Default table has no states; should be very small
    assert!(
        bytes.len() < 1000,
        "default table serialized to {} bytes",
        bytes.len()
    );
}

#[test]
fn serial_size_chain_nonempty() {
    let bytes = build_table(&chain_grammar()).to_bytes().expect("s");
    assert!(bytes.len() > 4, "chain grammar must produce >4 bytes");
}

#[test]
fn serial_size_roundtrip_preserves_length() {
    let table = build_table(&expr_grammar());
    let b1 = table.to_bytes().expect("s1");
    let restored = ParseTable::from_bytes(&b1).expect("d");
    let b2 = restored.to_bytes().expect("s2");
    assert_eq!(b1.len(), b2.len());
}

#[test]
fn serial_size_stmt_list_reasonable() {
    let bytes = build_table(&statement_list_grammar())
        .to_bytes()
        .expect("s");
    assert!(bytes.len() > 10);
    assert!(bytes.len() < 500_000);
}

// ===================================================================
// 5. serial_complex_* — complex grammars and multi-action cells (8 tests)
// ===================================================================

#[test]
fn serial_complex_arithmetic_all_fields() {
    let table = build_table(&arithmetic_grammar());
    let restored = roundtrip(&table);
    assert_eq!(table.state_count, restored.state_count);
    assert_eq!(table.symbol_count, restored.symbol_count);
    assert_eq!(table.action_table, restored.action_table);
    assert_eq!(table.goto_table, restored.goto_table);
    assert_eq!(table.rules.len(), restored.rules.len());
}

#[test]
fn serial_complex_stmt_list_all_fields() {
    let table = build_table(&statement_list_grammar());
    let restored = roundtrip(&table);
    assert_eq!(table.state_count, restored.state_count);
    assert_eq!(table.action_table, restored.action_table);
    assert_eq!(table.goto_table, restored.goto_table);
}

#[test]
fn serial_complex_shift_action_preserved() {
    let table = make_action_table(vec![Action::Shift(StateId(42))]);
    let restored = roundtrip(&table);
    assert_eq!(
        restored.action_table[0][0],
        vec![Action::Shift(StateId(42))]
    );
}

#[test]
fn serial_complex_reduce_action_preserved() {
    let table = make_action_table(vec![Action::Reduce(RuleId(7))]);
    let restored = roundtrip(&table);
    assert_eq!(restored.action_table[0][0], vec![Action::Reduce(RuleId(7))]);
}

#[test]
fn serial_complex_accept_action_preserved() {
    let table = make_action_table(vec![Action::Accept]);
    let restored = roundtrip(&table);
    assert_eq!(restored.action_table[0][0], vec![Action::Accept]);
}

#[test]
fn serial_complex_error_action_preserved() {
    let table = make_action_table(vec![Action::Error]);
    let restored = roundtrip(&table);
    assert_eq!(restored.action_table[0][0], vec![Action::Error]);
}

#[test]
fn serial_complex_multi_action_cell_preserved() {
    let table = make_action_table(vec![Action::Shift(StateId(1)), Action::Reduce(RuleId(2))]);
    let restored = roundtrip(&table);
    assert_eq!(restored.action_table[0][0].len(), 2);
    assert_eq!(restored.action_table[0][0][0], Action::Shift(StateId(1)));
    assert_eq!(restored.action_table[0][0][1], Action::Reduce(RuleId(2)));
}

#[test]
fn serial_complex_nested_epsilon_grammar() {
    let table = build_table(&nested_grammar());
    let restored = roundtrip(&table);
    assert_eq!(table.state_count, restored.state_count);
    assert_eq!(table.rules.len(), restored.rules.len());
    assert_eq!(table.action_table, restored.action_table);
}

// ===================================================================
// 6. serial_deterministic_* — determinism / idempotence (8 tests)
// ===================================================================

#[test]
fn serial_deterministic_same_grammar_same_bytes() {
    let b1 = build_table(&arithmetic_grammar()).to_bytes().expect("s1");
    let b2 = build_table(&arithmetic_grammar()).to_bytes().expect("s2");
    assert_eq!(b1, b2);
}

#[test]
fn serial_deterministic_single_token_stable() {
    let b1 = build_table(&single_token_grammar()).to_bytes().expect("s1");
    let b2 = build_table(&single_token_grammar()).to_bytes().expect("s2");
    assert_eq!(b1, b2);
}

#[test]
fn serial_deterministic_chain_stable() {
    let b1 = build_table(&chain_grammar()).to_bytes().expect("s1");
    let b2 = build_table(&chain_grammar()).to_bytes().expect("s2");
    assert_eq!(b1, b2);
}

#[test]
fn serial_deterministic_different_grammars_differ() {
    let b1 = build_table(&single_token_grammar()).to_bytes().expect("s1");
    let b2 = build_table(&two_alt_grammar()).to_bytes().expect("s2");
    assert_ne!(b1, b2);
}

#[test]
fn serial_deterministic_left_vs_right_recursive_differ() {
    let b1 = build_table(&left_recursive_grammar())
        .to_bytes()
        .expect("s1");
    let b2 = build_table(&right_recursive_grammar())
        .to_bytes()
        .expect("s2");
    assert_ne!(b1, b2);
}

#[test]
fn serial_deterministic_triple_roundtrip_stable() {
    let table = build_table(&five_alt_grammar());
    let b1 = table.to_bytes().expect("s1");
    let t2 = ParseTable::from_bytes(&b1).expect("d1");
    let b2 = t2.to_bytes().expect("s2");
    let t3 = ParseTable::from_bytes(&b2).expect("d2");
    let b3 = t3.to_bytes().expect("s3");
    assert_eq!(b1, b2);
    assert_eq!(b2, b3);
}

#[test]
fn serial_deterministic_expr_idempotent() {
    let table = build_table(&expr_grammar());
    let bytes = table.to_bytes().expect("s");
    let restored = ParseTable::from_bytes(&bytes).expect("d");
    let bytes2 = restored.to_bytes().expect("s2");
    assert_eq!(bytes, bytes2);
}

#[test]
fn serial_deterministic_nested_vs_chain_differ() {
    let b1 = build_table(&nested_grammar()).to_bytes().expect("s1");
    let b2 = build_table(&chain_grammar()).to_bytes().expect("s2");
    assert_ne!(b1, b2);
}

// ===================================================================
// 7. serial_corrupt_* — corrupted / invalid input handling (8 tests)
// ===================================================================

#[test]
fn serial_corrupt_empty_bytes() {
    assert!(ParseTable::from_bytes(&[]).is_err());
}

#[test]
fn serial_corrupt_single_byte() {
    assert!(ParseTable::from_bytes(&[0x42]).is_err());
}

#[test]
fn serial_corrupt_all_zeros() {
    assert!(ParseTable::from_bytes(&[0u8; 64]).is_err());
}

#[test]
fn serial_corrupt_all_ones() {
    assert!(ParseTable::from_bytes(&[0xFF; 64]).is_err());
}

#[test]
fn serial_corrupt_pseudorandom_garbage() {
    let garbage: Vec<u8> = (0u8..128)
        .map(|i| i.wrapping_mul(37).wrapping_add(13))
        .collect();
    assert!(ParseTable::from_bytes(&garbage).is_err());
}

#[test]
fn serial_corrupt_truncated_halfway() {
    let table = build_table(&arithmetic_grammar());
    let bytes = table.to_bytes().expect("serialize");
    let half = bytes.len() / 2;
    assert!(ParseTable::from_bytes(&bytes[..half]).is_err());
}

#[test]
fn serial_corrupt_valid_version_garbage_data() {
    let tampered = craft_versioned_bytes(PARSE_TABLE_FORMAT_VERSION, &[0xFF; 50]);
    assert!(ParseTable::from_bytes(&tampered).is_err());
}

#[test]
fn serial_corrupt_flipped_middle_no_panic() {
    let table = build_table(&arithmetic_grammar());
    let mut bytes = table.to_bytes().expect("serialize");
    if bytes.len() > 8 {
        let mid = bytes.len() / 2;
        bytes[mid] ^= 0xFF;
        bytes[mid + 1] ^= 0xAA;
        bytes[mid + 2] ^= 0x55;
    }
    // Must not panic — may succeed or fail
    let _ = ParseTable::from_bytes(&bytes);
}

// ===================================================================
// 8. serial_edge_* — boundary and edge-case coverage (8 tests)
// ===================================================================

#[test]
fn serial_edge_trailing_junk_no_panic() {
    let table = build_table(&single_token_grammar());
    let mut bytes = table.to_bytes().expect("serialize");
    bytes.extend_from_slice(&[0xDE, 0xAD, 0xBE, 0xEF]);
    // postcard may or may not tolerate trailing data — no panic is the contract
    let _ = ParseTable::from_bytes(&bytes);
}

#[test]
fn serial_edge_one_byte_short() {
    let table = build_table(&chain_grammar());
    let bytes = table.to_bytes().expect("serialize");
    assert!(ParseTable::from_bytes(&bytes[..bytes.len() - 1]).is_err());
}

#[test]
fn serial_edge_empty_inner_data_rejected() {
    let tampered = craft_versioned_bytes(PARSE_TABLE_FORMAT_VERSION, &[]);
    assert!(ParseTable::from_bytes(&tampered).is_err());
}

#[test]
fn serial_edge_nullable_prefix_roundtrips() {
    let table = build_table(&nullable_prefix_grammar());
    let restored = roundtrip(&table);
    assert_eq!(table.state_count, restored.state_count);
    assert_eq!(table.action_table, restored.action_table);
}

#[test]
fn serial_edge_wide_rhs_roundtrips() {
    let table = build_table(&wide_rhs_grammar());
    let restored = roundtrip(&table);
    assert_eq!(table.state_count, restored.state_count);
    assert_eq!(table.rules.len(), restored.rules.len());
}

#[test]
fn serial_edge_deep_chain_roundtrips() {
    let table = build_table(&deep_chain_grammar());
    let restored = roundtrip(&table);
    assert_eq!(table.state_count, restored.state_count);
    assert_eq!(table.goto_table, restored.goto_table);
}

#[test]
fn serial_edge_per_state_actions_match() {
    let g = five_alt_grammar();
    let table = build_table(&g);
    let restored = roundtrip(&table);
    for st in 0..table.state_count {
        let sid = StateId(st as u16);
        for (&sym, _) in &g.tokens {
            assert_eq!(
                table.actions(sid, sym),
                restored.actions(sid, sym),
                "mismatch at state {st} sym {sym:?}"
            );
        }
    }
}

#[test]
fn serial_edge_state_count_nonzero_after_roundtrip() {
    let table = build_table(&arithmetic_grammar());
    let restored = roundtrip(&table);
    assert!(restored.state_count > 0);
    assert!(restored.symbol_count > 0);
    assert!(!restored.rules.is_empty());
}
