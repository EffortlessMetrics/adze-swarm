//! #930: streaming Forest → selected AdzeDocument materializer proof.
//!
//! These tests call the adapter on constructed forests without production
//! parser routing / lexer commitment.

#![cfg(all(feature = "pure-rust", feature = "glr"))]

use adze::glr_parser::SelectionReason;
use adze::glr_streaming_runtime::{
    materialize_streaming_forest, materialize_streaming_forest_document,
};
use adze::pure_parser::{ExternalScanner, TSLanguage, TSLexState};
use adze_glr_core::parse_forest::{ErrorMeta, ForestAlternative};
use adze_glr_core::{Forest, ForestNode, ParseForest, ParseTable};
use adze_ir::builder::GrammarBuilder;
use adze_ir::{Grammar, StateId, SymbolId};
use std::collections::HashMap;

#[repr(transparent)]
struct SymbolNames([*const u8; 3]);
// SAFETY: pointers refer to static NUL-terminated byte literals.
unsafe impl Sync for SymbolNames {}

static SYMBOL_EOF: &[u8] = b"end\0";
static SYMBOL_A: &[u8] = b"a\0";
static SYMBOL_S: &[u8] = b"s\0";
static SYMBOL_NAMES: SymbolNames =
    SymbolNames([SYMBOL_EOF.as_ptr(), SYMBOL_A.as_ptr(), SYMBOL_S.as_ptr()]);
static LEX_MODES: [TSLexState; 1] = [TSLexState {
    lex_state: 0,
    external_lex_state: 0,
}];
static SYMBOL_METADATA: [u8; 3] = [0x01, 0x01, 0x03];

static TEST_LANGUAGE: TSLanguage = TSLanguage {
    version: 15,
    symbol_count: 3,
    alias_count: 0,
    token_count: 2,
    external_token_count: 0,
    state_count: 1,
    large_state_count: 0,
    production_id_count: 0,
    field_count: 0,
    max_alias_sequence_length: 0,
    eof_symbol: 0,
    rules: core::ptr::null(),
    rule_count: 0,
    production_count: 0,
    production_lhs_index: core::ptr::null(),
    production_id_map: core::ptr::null(),
    parse_table: core::ptr::null(),
    small_parse_table: core::ptr::null(),
    small_parse_table_map: core::ptr::null(),
    parse_actions: core::ptr::null(),
    symbol_names: SYMBOL_NAMES.0.as_ptr(),
    field_names: core::ptr::null(),
    field_map_slices: core::ptr::null(),
    field_map_entries: core::ptr::null(),
    symbol_metadata: SYMBOL_METADATA.as_ptr(),
    public_symbol_map: core::ptr::null(),
    alias_map: core::ptr::null(),
    alias_sequences: core::ptr::null(),
    lex_modes: LEX_MODES.as_ptr() as *const _,
    lex_fn: None,
    keyword_lex_fn: None,
    keyword_capture_token: 0,
    external_scanner: ExternalScanner {
        states: core::ptr::null(),
        symbol_map: core::ptr::null(),
        create: None,
        destroy: None,
        scan: None,
        serialize: None,
        deserialize: None,
    },
    primary_state_ids: core::ptr::null(),
};

fn tiny_grammar() -> Grammar {
    GrammarBuilder::new("streaming_forest_document")
        .token("a", "a")
        .rule("s", vec!["a"])
        .start("s")
        .build()
}

fn leaf(id: usize, symbol: SymbolId, span: (usize, usize)) -> ForestNode {
    ForestNode {
        id,
        symbol,
        span,
        alternatives: vec![ForestAlternative { children: vec![] }],
        error_meta: ErrorMeta::default(),
    }
}

/// Two complete roots with distinct structural keys so selection is forced.
fn constructed_ambiguous_forest(grammar: Grammar) -> Forest {
    let start = grammar.start_symbol().expect("start symbol");
    let token_a = grammar.find_symbol_by_name("a").expect("token a");

    // Prefer the shorter/earlier structural key: root 0 wins over root 1.
    let root0 = ForestNode {
        id: 0,
        symbol: start,
        span: (0, 1),
        alternatives: vec![ForestAlternative { children: vec![2] }],
        error_meta: ErrorMeta::default(),
    };
    let root1 = ForestNode {
        id: 1,
        symbol: start,
        span: (0, 2),
        alternatives: vec![ForestAlternative { children: vec![3] }],
        error_meta: ErrorMeta::default(),
    };
    let child0 = leaf(2, token_a, (0, 1));
    let child1 = leaf(3, token_a, (0, 2));

    let mut nodes = HashMap::new();
    nodes.insert(0, root0.clone());
    nodes.insert(1, root1.clone());
    nodes.insert(2, child0);
    nodes.insert(3, child1);

    Forest::from_parse_forest_for_test(ParseForest {
        roots: vec![root0, root1],
        nodes,
        grammar,
        source: "aa".to_string(),
        next_node_id: 4,
    })
}

fn empty_parse_table(grammar: Grammar) -> ParseTable {
    let mut symbol_ids: Vec<SymbolId> = grammar.tokens.keys().copied().collect();
    symbol_ids.extend(grammar.rules.keys().copied());
    symbol_ids.sort_by_key(|id| id.0);
    symbol_ids.dedup();

    let symbol_count = symbol_ids.len().max(1);
    let mut symbol_to_index = std::collections::BTreeMap::new();
    for (index, id) in symbol_ids.iter().enumerate() {
        symbol_to_index.insert(*id, index);
    }

    ParseTable {
        action_table: vec![vec![vec![]; symbol_count]],
        goto_table: vec![vec![StateId(0); symbol_count]],
        state_count: 1,
        symbol_count,
        symbol_to_index,
        index_to_symbol: symbol_ids,
        token_count: grammar.tokens.len(),
        external_token_count: 0,
        start_symbol: grammar.start_symbol().unwrap_or(SymbolId(0)),
        grammar,
        rules: Vec::new(),
        dynamic_prec_by_rule: Vec::new(),
        ..ParseTable::default()
    }
}

#[test]
fn test_materialize_streaming_forest_constructed_roots_selects_deterministically() {
    let grammar = tiny_grammar();
    let parse_table = empty_parse_table(grammar.clone());
    let forest = constructed_ambiguous_forest(grammar.clone());

    let first = materialize_streaming_forest(&forest, &TEST_LANGUAGE, &parse_table, &grammar)
        .expect("constructed forest should materialize");
    let second = materialize_streaming_forest(&forest, &TEST_LANGUAGE, &parse_table, &grammar)
        .expect("constructed forest should materialize again");

    let first_summary = first
        .ambiguities
        .as_ref()
        .expect("two roots should produce an ambiguity summary");
    let second_summary = second
        .ambiguities
        .as_ref()
        .expect("second materialization should retain ambiguity");

    assert_eq!(first_summary.alternatives.len(), 2);
    // wrap_forest orders roots by largest span first, then stable structural
    // selection chooses the shorter complete root (byte range 0..1) at index 1.
    assert_eq!(first_summary.selected, Some(1));
    assert_eq!(
        first_summary.selection_reason,
        SelectionReason::StableStructuralTieBreak
    );
    assert_eq!(first_summary.selected, second_summary.selected);
    assert_eq!(first_summary.alternatives, second_summary.alternatives);
    assert_eq!(first.root.node.byte_range, 0..1);
    assert_eq!(second.root.node.byte_range, first.root.node.byte_range);
    assert_eq!(second.root.node.symbol_id, first.root.node.symbol_id);
}

#[test]
fn test_materialize_streaming_forest_document_preserves_selected_and_ambiguity_facts() {
    let grammar = tiny_grammar();
    let parse_table = empty_parse_table(grammar.clone());
    let forest = constructed_ambiguous_forest(grammar.clone());
    let source = "aa";

    let first = materialize_streaming_forest_document(
        source,
        &forest,
        &TEST_LANGUAGE,
        "streaming_forest_document",
        &grammar,
        &parse_table,
    )
    .expect("document materialization should succeed");
    let second = materialize_streaming_forest_document(
        source,
        &forest,
        &TEST_LANGUAGE,
        "streaming_forest_document",
        &grammar,
        &parse_table,
    )
    .expect("document materialization should be repeatable");

    assert!(first.diagnostics().is_empty());
    assert_eq!(first.ambiguities().len(), 1);
    assert_eq!(second.ambiguities(), first.ambiguities());

    let summary = &first.ambiguities()[0];
    assert_eq!(summary.selected, Some(1));
    assert_eq!(summary.alternatives.len(), 2);
    assert_eq!(
        summary.selection_reason,
        SelectionReason::StableStructuralTieBreak
    );

    assert_eq!(first.tree().root().byte_range(), 0..1);
    assert_eq!(
        second.tree().root().byte_range(),
        first.tree().root().byte_range()
    );
}
