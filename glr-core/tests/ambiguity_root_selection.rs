//! Root/selection determinism for ambiguous parses (#1031, glr-core slice).
//!
//! The JSON layer already canonicalizes alternative summaries. This target pins
//! the remaining glr-core contract: tied roots — or sibling alternatives —
//! are decided by a documented structural rule, never by vector insertion
//! (derivation-discovery) order.
//!
//! Canonical rule under test (see `ForestNode::selection_rank` /
//! `ForestNode::best_alternative_index`): roots — largest span, then earliest
//! start, then smallest symbol id, then smallest node id. Alternatives —
//! lexicographically smallest children-id sequence, then lowest index.
//!
//! Run with: `cargo test -p adze-glr-core --test ambiguity_root_selection`

use adze_glr_core::forest_view::Forest;
use adze_glr_core::parse_forest::{ErrorMeta, ForestAlternative, ForestNode, ParseForest};
use adze_ir::builder::GrammarBuilder;
use adze_ir::{Grammar, SymbolId};
use std::collections::HashMap;

fn grammar_with_start() -> (Grammar, SymbolId) {
    let grammar = GrammarBuilder::new("ambig_selection")
        .token("a", "a")
        .rule("s", vec!["a"])
        .start("s")
        .build();
    let start = grammar.start_symbol().expect("start symbol present");
    (grammar, start)
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

fn parent(id: usize, symbol: SymbolId, span: (usize, usize), children: Vec<usize>) -> ForestNode {
    ForestNode {
        id,
        symbol,
        span,
        alternatives: vec![ForestAlternative { children }],
        error_meta: ErrorMeta::default(),
    }
}

fn assemble(roots: Vec<ForestNode>, extra: Vec<ForestNode>, grammar: Grammar) -> ParseForest {
    let mut nodes = HashMap::new();
    for node in roots.iter().chain(extra.iter()) {
        nodes.insert(node.id, node.clone());
    }
    ParseForest {
        roots,
        nodes,
        grammar,
        source: "aaa".to_string(),
        next_node_id: 100,
    }
}

/// Two complete roots, identical symbol and span, inserted highest-id-first.
///
/// Before the fix, `wrap_forest` sorted only by (span length, start) and the
/// tie fell through to stable insertion order, so `roots()[0]` was id 7. The
/// documented rule selects the smallest node id (3) instead.
#[test]
fn wrapped_roots_with_identical_span_select_smallest_node_id() {
    let (grammar, start) = grammar_with_start();
    let forest = assemble(
        vec![leaf(7, start, (0, 3)), leaf(3, start, (0, 3))],
        vec![],
        grammar,
    );
    let wrapped = Forest::from_parse_forest_for_test(forest);
    assert_eq!(
        wrapped.view().roots()[0],
        3u32,
        "tied roots must select smallest node id, not insertion order"
    );
}

/// Same setup through `to_single_tree`, observed via differing children.
///
/// Root 7 (inserted first) points at leaf 70 span (0, 1); root 3 points at
/// leaf 30 span (0, 3); both roots claim span (0, 3). The winner must be root
/// 3; the tree proves it by containing leaf 30.
#[test]
fn to_single_tree_with_identical_span_selects_smallest_node_id() {
    let (grammar, start) = grammar_with_start();
    let forest = assemble(
        vec![
            parent(7, start, (0, 3), vec![70]),
            parent(3, start, (0, 3), vec![30]),
        ],
        vec![leaf(70, start, (0, 1)), leaf(30, start, (0, 3))],
        grammar,
    );
    let tree = forest.to_single_tree().expect("complete parse");
    assert_eq!(tree.root.children.len(), 1);
    assert_eq!(
        tree.root.children[0].span,
        (0, 3),
        "to_single_tree must pick root 3 (smallest id), not first-inserted root 7"
    );
}

/// One node, two alternatives, lexicographically larger children stored first.
///
/// Before the fix every consumer took `alternatives[0]`. The documented rule
/// selects the lexicographically smallest children-id sequence ([30] beats
/// [70]) regardless of storage order, through both the view and the tree.
#[test]
fn best_alternative_is_lexicographically_smallest_children_not_first_stored() {
    let (grammar, start) = grammar_with_start();
    let root = ForestNode {
        id: 0,
        symbol: start,
        span: (0, 3),
        alternatives: vec![
            ForestAlternative { children: vec![70] },
            ForestAlternative { children: vec![30] },
        ],
        error_meta: ErrorMeta::default(),
    };
    let forest = assemble(
        vec![root],
        vec![leaf(70, start, (2, 3)), leaf(30, start, (0, 3))],
        grammar,
    );

    // View path (wrap borrows nothing: ParseForest is Clone).
    let wrapped = Forest::from_parse_forest_for_test(forest.clone());
    assert_eq!(
        wrapped.view().best_children(0),
        &[30u32],
        "best_children must pick lexicographically smallest children, not alternatives[0]"
    );

    // Tree path.
    let tree = forest.to_single_tree().expect("complete parse");
    assert_eq!(tree.root.children.len(), 1);
    assert_eq!(
        tree.root.children[0].span,
        (0, 3),
        "extracted tree must follow the canonical alternative, not alternatives[0]"
    );
}

/// Span rule still dominates the id tie-break: a strictly larger span wins
/// even with a larger node id. Guards against over-correcting the tie-break.
#[test]
fn larger_span_still_beats_smaller_node_id() {
    let (grammar, start) = grammar_with_start();
    let forest = assemble(
        vec![leaf(9, start, (0, 5)), leaf(1, start, (0, 1))],
        vec![],
        grammar,
    );
    let wrapped = Forest::from_parse_forest_for_test(forest);
    assert_eq!(wrapped.view().roots()[0], 9u32);
}
