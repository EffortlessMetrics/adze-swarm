//! Generation -> compression -> serializer parity for GLR conflict cells (#929 PR2).

use adze_glr_core::{Action, FirstFollowSets, ParseTable, build_lr1_automaton};
use adze_ir::builder::GrammarBuilder;
use adze_ir::{Grammar, RuleId, StateId};
use adze_tablegen::compress::{
    CompressedActionEntry, CompressedActionTable, CompressedGotoTable, CompressedTables,
    TableCompressor,
};
use adze_tablegen::serializer::serialize_language;
use adze_tablegen::{collect_token_indices, eof_accepts_or_reduces};

fn pipeline(g: &Grammar) -> ParseTable {
    let mut grammar = g.clone();
    let ff = FirstFollowSets::compute_normalized(&mut grammar).expect("FIRST/FOLLOW");
    build_lr1_automaton(&grammar, &ff).expect("LR(1)")
}

fn conflict_fixture() -> (Grammar, ParseTable) {
    let grammar = GrammarBuilder::new("conflict_abi_roundtrip")
        .token("t", "t")
        .rule("S", vec!["t"])
        .start("S")
        .build();
    let mut table = pipeline(&grammar);
    table.action_table[0][1] = vec![Action::Fork(vec![
        Action::Shift(StateId(1)),
        Action::Reduce(RuleId(0)),
    ])];
    (grammar, table)
}

#[test]
fn compress_emits_duplicate_symbol_pairs_for_single_level_fork() {
    let (grammar, table) = conflict_fixture();
    let token_indices = collect_token_indices(&grammar, &table);
    let compressed = TableCompressor::new()
        .compress(&table, &token_indices, eof_accepts_or_reduces(&table))
        .expect("single-level fork must compress");

    let state0 = &compressed.action_table.data[compressed.action_table.row_offsets[0] as usize
        ..compressed.action_table.row_offsets[1] as usize];
    assert_eq!(state0.len(), 2, "fork cell must emit two ABI pairs");
    assert_eq!(state0[0].symbol, state0[1].symbol);
    assert!(matches!(state0[0].action, Action::Shift(_)));
    assert!(matches!(state0[1].action, Action::Reduce(_)));
}

#[test]
fn compress_rejects_nested_fork_cells() {
    let (grammar, mut table) = conflict_fixture();
    table.action_table[0][1] = vec![Action::Fork(vec![Action::Fork(vec![
        Action::Shift(StateId(1)),
        Action::Reduce(RuleId(0)),
    ])])];

    let token_indices = collect_token_indices(&grammar, &table);
    match TableCompressor::new().compress(&table, &token_indices, eof_accepts_or_reduces(&table)) {
        Err(err) => assert!(
            err.to_string().contains("nested Action::Fork"),
            "unexpected error: {err}"
        ),
        Ok(_) => panic!("nested fork must be rejected at compression time"),
    }
}

#[test]
fn serializer_matches_abi_encoding_for_conflict_cell() {
    let (grammar, table) = conflict_fixture();
    let token_indices = collect_token_indices(&grammar, &table);
    let compressed = TableCompressor::new()
        .compress(&table, &token_indices, eof_accepts_or_reduces(&table))
        .expect("compress conflict table");

    let json = serialize_language(&grammar, &table, Some(&compressed)).expect("serialize");
    let value: serde_json::Value = serde_json::from_str(&json).expect("valid json");
    let parse_table = value["parse_table"].as_array().expect("parse_table array");
    let map = value["small_parse_table_map"]
        .as_array()
        .expect("small_parse_table_map array");

    let start = map[0].as_u64().unwrap() as usize;
    let end = map[1].as_u64().unwrap() as usize;
    let row: Vec<u16> = parse_table[start..end]
        .iter()
        .map(|v| v.as_u64().unwrap() as u16)
        .collect();

    assert_eq!(
        row.len(),
        4,
        "conflict row must contain two symbol/action pairs"
    );
    assert_eq!(row[0], row[2], "duplicate symbol entries");
    assert_eq!(row[1], 1, "shift encoding");
    assert_eq!(row[3], 0x8001, "reduce encoding");
}

#[test]
fn manual_compressed_conflict_matches_serializer_encoding() {
    let tables = CompressedTables {
        action_table: CompressedActionTable {
            data: vec![
                CompressedActionEntry::new(1, Action::Shift(StateId(1))),
                CompressedActionEntry::new(1, Action::Reduce(RuleId(0))),
            ],
            row_offsets: vec![0, 2, 2],
            default_actions: vec![Action::Error, Action::Error],
        },
        goto_table: CompressedGotoTable {
            data: vec![],
            row_offsets: vec![0, 0, 0],
        },
        small_table_threshold: 32768,
    };

    let (grammar, table) = conflict_fixture();
    let json = serialize_language(&grammar, &table, Some(&tables)).expect("serialize");
    let value: serde_json::Value = serde_json::from_str(&json).expect("valid json");
    let parse_table = value["parse_table"].as_array().expect("parse_table array");

    assert_eq!(parse_table.len(), 4);
    assert_eq!(parse_table[1].as_u64(), Some(1));
    assert_eq!(parse_table[3].as_u64(), Some(0x8001));
}
