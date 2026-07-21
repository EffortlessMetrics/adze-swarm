//! #930: streaming Forest → selected AdzeDocument materializer proof.
//!
//! These tests call the adapter on constructed forests without production
//! parser routing / lexer commitment.

#![cfg(all(feature = "pure-rust", feature = "glr"))]

use adze::__private::align_true_glr_parse_table_to_language_symbols;
use adze::decoder::{decode_grammar, decode_parse_table};
use adze::glr_parser::SelectionReason;
use adze::glr_streaming_runtime::{
    materialize_streaming_forest, materialize_streaming_forest_document,
};
use adze::pure_parser::{ExternalScanner, TSLanguage, TSLexState, TSParseAction, TSRule};
use adze_glr_core::parse_forest::{ErrorMeta, ForestAlternative};
use adze_glr_core::{Forest, ForestNode, ParseForest, ParseTable};
use adze_ir::builder::GrammarBuilder;
use adze_ir::{Grammar, StateId, Symbol, SymbolId};
use std::collections::HashMap;
use std::ptr;

mod alias_field_fixture {
    use super::*;

    static SYMBOL_NAME_END: &[u8] = b"end\0";
    static SYMBOL_NAME_NUM: &[u8] = b"num\0";
    static SYMBOL_NAME_START: &[u8] = b"start\0";
    static FIELD_NAME_VALUE: &[u8] = b"value\0";

    #[repr(transparent)]
    struct SymbolNames([*const u8; 3]);
    unsafe impl Sync for SymbolNames {}

    static SYMBOL_NAMES: SymbolNames = SymbolNames([
        SYMBOL_NAME_END.as_ptr(),
        SYMBOL_NAME_NUM.as_ptr(),
        SYMBOL_NAME_START.as_ptr(),
    ]);

    #[repr(transparent)]
    struct FieldNames([*const u8; 1]);
    unsafe impl Sync for FieldNames {}

    static FIELD_NAMES: FieldNames = FieldNames([FIELD_NAME_VALUE.as_ptr()]);

    static SYMBOL_METADATA: [u8; 3] = [0x00, 0x03, 0x02];
    static SMALL_PARSE_TABLE: [u16; 8] = [1, 1, 2, 2, 0, 0x8001, 0, 0xFFFF];
    static SMALL_PARSE_TABLE_MAP: [u32; 4] = [0, 4, 6, 8];
    static LEX_MODES: [TSLexState; 3] = [
        TSLexState {
            lex_state: 0,
            external_lex_state: 0,
        },
        TSLexState {
            lex_state: 7,
            external_lex_state: 0,
        },
        TSLexState {
            lex_state: 3,
            external_lex_state: 0,
        },
    ];
    static PUBLIC_SYMBOL_MAP: [u16; 3] = [0, 5, 9];
    static PRIMARY_STATE_IDS: [u16; 3] = [0, 1, 2];
    static PRODUCTION_ID_MAP: [u16; 1] = [0];
    static PRODUCTION_LHS_INDEX: [u16; 1] = [2];
    static TS_RULES: [TSRule; 1] = [TSRule {
        lhs: 2,
        rhs_len: 1,
        _pad: 0,
    }];
    static ALIAS_MAP: [u16; 1] = [0];
    static ALIAS_SEQUENCES: [u16; 1] = [1];
    static FIELD_MAP_SLICES: [u16; 2] = [0, 1];
    static FIELD_MAP_ENTRIES: [u16; 2] = [0, 0];
    static PARSE_ACTIONS: [TSParseAction; 1] = [TSParseAction {
        action_type: 3,
        extra: 0,
        child_count: 1,
        dynamic_precedence: 0,
        symbol: 0,
    }];

    pub static LANGUAGE: TSLanguage = TSLanguage {
        version: 15,
        symbol_count: 3,
        alias_count: 1,
        token_count: 2,
        external_token_count: 0,
        state_count: 3,
        large_state_count: 0,
        production_id_count: 1,
        field_count: 1,
        max_alias_sequence_length: 1,
        production_id_map: PRODUCTION_ID_MAP.as_ptr(),
        parse_table: ptr::null(),
        small_parse_table: SMALL_PARSE_TABLE.as_ptr(),
        small_parse_table_map: SMALL_PARSE_TABLE_MAP.as_ptr(),
        parse_actions: PARSE_ACTIONS.as_ptr(),
        symbol_names: SYMBOL_NAMES.0.as_ptr(),
        field_names: FIELD_NAMES.0.as_ptr(),
        field_map_slices: FIELD_MAP_SLICES.as_ptr(),
        field_map_entries: FIELD_MAP_ENTRIES.as_ptr(),
        symbol_metadata: SYMBOL_METADATA.as_ptr(),
        public_symbol_map: PUBLIC_SYMBOL_MAP.as_ptr(),
        alias_map: ALIAS_MAP.as_ptr(),
        alias_sequences: ALIAS_SEQUENCES.as_ptr(),
        lex_modes: LEX_MODES.as_ptr() as *const _,
        lex_fn: None,
        keyword_lex_fn: None,
        keyword_capture_token: 0,
        external_scanner: ExternalScanner {
            states: ptr::null(),
            symbol_map: ptr::null(),
            create: None,
            destroy: None,
            scan: None,
            serialize: None,
            deserialize: None,
        },
        primary_state_ids: PRIMARY_STATE_IDS.as_ptr(),
        production_lhs_index: PRODUCTION_LHS_INDEX.as_ptr(),
        production_count: 1,
        eof_symbol: 0,
        rules: TS_RULES.as_ptr(),
        rule_count: 1,
    };
}

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

fn aligned_parse_table(language: &'static TSLanguage) -> (Grammar, ParseTable) {
    let grammar = decode_grammar(language);
    let mut parse_table = decode_parse_table(language);
    align_true_glr_parse_table_to_language_symbols(language, &mut parse_table);
    (grammar, parse_table)
}

fn production_rhs_symbols(rule: &adze_ir::Rule) -> Vec<SymbolId> {
    rule.rhs
        .iter()
        .filter_map(|symbol| match symbol {
            Symbol::Terminal(id) | Symbol::NonTerminal(id) | Symbol::External(id) => Some(*id),
            _ => None,
        })
        .collect()
}

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

fn symbol_by_name(parse_table: &ParseTable, name: &str) -> SymbolId {
    parse_table
        .symbol_metadata
        .iter()
        .find(|metadata| metadata.name == name)
        .map(|metadata| metadata.symbol_id)
        .unwrap_or_else(|| panic!("fixture should expose symbol '{name}'"))
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

fn constructed_fielded_pair_forest(source: &str) -> (Forest, Grammar, ParseTable) {
    let language = adze_example::fielded_typed_cst_contract::grammar::language();
    let grammar = decode_grammar(language);
    let parse_table = decode_parse_table(language);
    let reference = adze_example::fielded_typed_cst_contract::grammar::parse_document(source)
        .expect("reference parse_document should succeed for fielded pair fixture");

    let pair = find_node_by_kind(reference.tree().root(), "Pair")
        .expect("reference document should expose Pair node");
    let left = pair
        .edge_by_field_name("left")
        .and_then(|edge| edge.child())
        .expect("reference Pair should expose left field");
    let right = pair
        .edge_by_field_name("right")
        .and_then(|edge| edge.child())
        .expect("reference Pair should expose right field");

    let source_root_sym = reference.tree().root().grammar_id();
    let pair_sym = pair.grammar_id();
    let left_sym = left.grammar_id();
    let right_sym = right.grammar_id();

    let source_root = ForestNode {
        id: 0,
        symbol: source_root_sym,
        span: (0, source.len()),
        alternatives: vec![ForestAlternative { children: vec![1] }],
        error_meta: ErrorMeta::default(),
    };
    let pair_node = ForestNode {
        id: 1,
        symbol: pair_sym,
        span: (0, source.len()),
        alternatives: vec![ForestAlternative {
            children: vec![2, 3],
        }],
        error_meta: ErrorMeta::default(),
    };
    let left_child = leaf(
        2,
        left_sym,
        (left.byte_range().start, left.byte_range().end),
    );
    let right_child = leaf(
        3,
        right_sym,
        (right.byte_range().start, right.byte_range().end),
    );

    let mut nodes = HashMap::new();
    nodes.insert(0, source_root.clone());
    nodes.insert(1, pair_node);
    nodes.insert(2, left_child);
    nodes.insert(3, right_child);

    let forest = Forest::from_parse_forest_for_test(ParseForest {
        roots: vec![source_root],
        nodes,
        grammar: grammar.clone(),
        source: source.to_string(),
        next_node_id: 4,
    });

    (forest, grammar, parse_table)
}

fn constructed_alias_field_forest(source: &str) -> (Forest, Grammar, ParseTable) {
    let language = &alias_field_fixture::LANGUAGE;
    let (grammar, parse_table) = aligned_parse_table(language);

    let start = parse_table.rules[0].lhs;
    let num = symbol_by_name(&parse_table, "num");
    let root = ForestNode {
        id: 0,
        symbol: start,
        span: (0, source.len()),
        alternatives: vec![ForestAlternative { children: vec![1] }],
        error_meta: ErrorMeta::default(),
    };
    let child = leaf(1, num, (0, source.len()));

    let mut nodes = HashMap::new();
    nodes.insert(0, root.clone());
    nodes.insert(1, child);

    let forest = Forest::from_parse_forest_for_test(ParseForest {
        roots: vec![root],
        nodes,
        grammar: grammar.clone(),
        source: source.to_string(),
        next_node_id: 2,
    });

    (forest, grammar, parse_table)
}

fn constructed_error_only_forest(grammar: Grammar) -> Forest {
    let root = ForestNode {
        id: 0,
        symbol: adze_glr_core::parse_forest::ERROR_SYMBOL,
        span: (0, 1),
        alternatives: vec![ForestAlternative { children: vec![] }],
        error_meta: ErrorMeta {
            is_error: true,
            missing: false,
            cost: 1,
        },
    };
    let mut nodes = HashMap::new();
    nodes.insert(0, root.clone());

    Forest::from_parse_forest_for_test(ParseForest {
        roots: vec![root],
        nodes,
        grammar,
        source: "x".to_string(),
        next_node_id: 1,
    })
}

fn find_node_by_kind<'a>(
    node: adze::document::AdzeNode<'a>,
    kind: &str,
) -> Option<adze::document::AdzeNode<'a>> {
    if node.kind_name() == Some(kind) {
        return Some(node);
    }
    for index in 0..node.child_count() {
        if let Some(child) = node.child(index) {
            if let Some(found) = find_node_by_kind(child, kind) {
                return Some(found);
            }
        }
    }
    None
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

#[test]
fn test_materialize_streaming_forest_document_preserves_fields_and_aliases() {
    let source = "42";
    let (forest, grammar, parse_table) = constructed_alias_field_forest(source);
    let language = &alias_field_fixture::LANGUAGE;

    let parsed = materialize_streaming_forest(&forest, language, &parse_table, &grammar)
        .expect("alias/field forest should materialize subtree facts");
    assert_ne!(
        parsed.root.children[0].field_id,
        adze::subtree::FIELD_NONE,
        "forest materializer should preserve field ids on child edges"
    );

    let document = materialize_streaming_forest_document(
        source,
        &forest,
        language,
        "alias_field_fixture",
        &grammar,
        &parse_table,
    )
    .expect("alias/field forest should materialize");

    let root = document.tree().root();
    let child = root
        .child(0)
        .expect("start production should expose one child");
    let child_identity = child.identity();

    assert_eq!(
        root.edge_by_field_name("value")
            .map(|edge| edge.field_name()),
        Some(Some("value"))
    );
    let alias_symbol = parse_table.alias_sequences[0][0].expect("alias sequence");
    assert_eq!(child_identity.alias_symbol_id(), Some(alias_symbol));
    assert!(child_identity.has_alias());
    assert_eq!(child_identity.visible_id(), alias_symbol);
}

#[test]
fn test_materialize_streaming_forest_document_fielded_pair_preserves_native_edges() {
    let source = "123+";
    let (forest, grammar, parse_table) = constructed_fielded_pair_forest(source);
    let language = adze_example::fielded_typed_cst_contract::grammar::language();

    let document = materialize_streaming_forest_document(
        source,
        &forest,
        language,
        "fielded_typed_cst_contract",
        &grammar,
        &parse_table,
    )
    .expect("fielded pair forest should materialize");

    let parsed = materialize_streaming_forest(&forest, language, &parse_table, &grammar)
        .expect("fielded pair forest should materialize subtree facts");
    let pair_subtree = parsed
        .root
        .children
        .first()
        .map(|edge| edge.subtree.as_ref())
        .expect("source_file root should wrap Pair");
    assert_ne!(
        pair_subtree.children[0].field_id,
        adze::subtree::FIELD_NONE,
        "Pair left child should retain field id in materialized subtree"
    );
    assert_ne!(
        pair_subtree.children[1].field_id,
        adze::subtree::FIELD_NONE,
        "Pair right child should retain field id in materialized subtree"
    );

    let pair = find_node_by_kind(document.tree().root(), "Pair")
        .or_else(|| {
            document
                .tree()
                .root()
                .child(0)
                .filter(|node| node.kind_name() == Some("Pair"))
        })
        .expect("materialized document should expose Pair node");

    let left_range = pair_subtree.children[0].subtree.node.byte_range.clone();
    let right_range = pair_subtree.children[1].subtree.node.byte_range.clone();
    assert_eq!(&source[left_range], "123");
    assert_eq!(&source[right_range], "+");
}

fn forest_from_reference_document(
    reference: &adze::document::AdzeDocument,
    grammar: Grammar,
) -> Forest {
    let source = reference.source_text().to_string();
    let mut nodes = HashMap::new();
    let mut next_id = 1usize;

    fn build_node(
        node: adze::document::AdzeNode<'_>,
        id: usize,
        nodes: &mut HashMap<usize, ForestNode>,
        next_id: &mut usize,
    ) -> ForestNode {
        let mut children = Vec::new();
        for index in 0..node.child_count() {
            let child = node.child(index).expect("child should exist");
            let child_id = *next_id;
            *next_id += 1;
            children.push(child_id);
            let child_node = build_node(child, child_id, nodes, next_id);
            nodes.insert(child_id, child_node);
        }
        let range = node.byte_range();
        ForestNode {
            id,
            symbol: node.grammar_id(),
            span: (range.start, range.end),
            alternatives: vec![ForestAlternative { children }],
            error_meta: ErrorMeta::default(),
        }
    }

    let root_node = build_node(reference.tree().root(), 0, &mut nodes, &mut next_id);
    nodes.insert(0, root_node.clone());
    Forest::from_parse_forest_for_test(ParseForest {
        roots: vec![root_node],
        nodes,
        grammar,
        source,
        next_node_id: next_id,
    })
}

#[test]
fn test_materialize_streaming_forest_document_preserves_reference_tree_shape() {
    let source = "1 + 2 + 3";
    let language = adze_example::typed_ast_contract::grammar::language();
    let grammar = decode_grammar(language);
    let parse_table = decode_parse_table(language);
    let reference = adze_example::typed_ast_contract::grammar::parse_document(source)
        .expect("reference parse_document should succeed for typed_ast fixture");
    let forest = forest_from_reference_document(&reference, grammar.clone());

    let document = materialize_streaming_forest_document(
        source,
        &forest,
        language,
        "typed_ast_contract",
        &grammar,
        &parse_table,
    )
    .expect("typed_ast forest should materialize into a document");

    assert_eq!(document.tree().node_count(), reference.tree().node_count());
    assert_eq!(
        document.tree().root().byte_range(),
        reference.tree().root().byte_range()
    );
    assert_eq!(
        document
            .tree()
            .root()
            .child(0)
            .and_then(|node| node.utf8_text().ok()),
        reference
            .tree()
            .root()
            .child(0)
            .and_then(|node| node.utf8_text().ok())
    );
    assert_eq!(document.source_text(), reference.source_text());
}

#[test]
fn test_materialize_streaming_forest_rejects_empty_roots() {
    let grammar = tiny_grammar();
    let parse_table = empty_parse_table(grammar.clone());
    let forest = Forest::from_parse_forest_for_test(ParseForest {
        roots: vec![],
        nodes: HashMap::new(),
        grammar: grammar.clone(),
        source: String::new(),
        next_node_id: 0,
    });

    let error = materialize_streaming_forest(&forest, &TEST_LANGUAGE, &parse_table, &grammar)
        .err()
        .expect("empty forest should not masquerade as a complete parse");

    assert!(
        error.to_string().contains("no complete parse roots"),
        "unexpected error: {error}"
    );
}

#[test]
fn test_materialize_streaming_forest_rejects_error_only_root() {
    let grammar = tiny_grammar();
    let parse_table = empty_parse_table(grammar.clone());
    let forest = constructed_error_only_forest(grammar.clone());

    let error = materialize_streaming_forest(&forest, &TEST_LANGUAGE, &parse_table, &grammar)
        .err()
        .expect("error-only root should not masquerade as a recovered complete parse");

    assert!(
        error
            .to_string()
            .contains("error-only root without a complete parse"),
        "unexpected error: {error}"
    );
}

#[test]
fn test_materialize_streaming_forest_document_error_forest_blocks_typed_extraction() {
    let grammar = tiny_grammar();
    let parse_table = empty_parse_table(grammar.clone());
    let forest = constructed_error_only_forest(grammar.clone());

    let materialize_error = materialize_streaming_forest_document(
        "x",
        &forest,
        &TEST_LANGUAGE,
        "tiny",
        &grammar,
        &parse_table,
    )
    .err()
    .expect("error-only forest should fail document materialization");

    assert!(
        materialize_error
            .to_string()
            .contains("error-only root without a complete parse"),
        "unexpected error: {materialize_error}"
    );
}
