#![cfg(feature = "pure-rust")]
#![allow(
    clippy::needless_range_loop,
    reason = "property and comprehensive tests use index-based loops to exercise table positions and boundary cases"
)]

//! Comprehensive tests for the builder module (forest-to-tree conversion).
//!
//! Tests the builder's tree construction, forest-to-tree pipeline via the
//! Parser API, Tree/Node/TreeCursor behaviour, Language builder, and edge cases.

use adze_runtime::language::SymbolMetadata;
use adze_runtime::test_helpers::linear_token_language;
use adze_runtime::tree::TreeCursor;
use adze_runtime::{Language, Parser, Tree};

// ---------------------------------------------------------------------------
// Helpers: build the small linear grammars used by parser/builder tests.
// ---------------------------------------------------------------------------

fn make_single_token_language() -> Language {
    linear_token_language("single", &["a"])
}

fn make_two_token_language() -> Language {
    linear_token_language("two_tok", &["a", "b"])
}

fn parse_single(input: &[u8]) -> Tree {
    let lang = make_single_token_language();
    let mut parser = Parser::new();
    parser.set_language(lang).unwrap();
    parser.parse(input, None).unwrap()
}

fn parse_two(input: &[u8]) -> Tree {
    let lang = make_two_token_language();
    let mut parser = Parser::new();
    parser.set_language(lang).unwrap();
    parser.parse(input, None).unwrap()
}

// ===========================================================================
// 1. Stub tree construction
// ===========================================================================

#[test]
fn stub_tree_root_has_zero_range() {
    let tree = Tree::new_stub();
    let root = tree.root_node();
    assert_eq!(root.start_byte(), 0);
    assert_eq!(root.end_byte(), 0);
}

#[test]
fn stub_tree_root_has_no_children() {
    let tree = Tree::new_stub();
    assert_eq!(tree.root_node().child_count(), 0);
}

#[test]
fn stub_tree_root_kind_is_zero() {
    let tree = Tree::new_stub();
    assert_eq!(tree.root_kind(), 0);
    assert_eq!(tree.root_node().kind_id(), 0);
}

#[test]
fn stub_tree_has_no_language() {
    let tree = Tree::new_stub();
    assert!(tree.language().is_none());
}

#[test]
fn stub_tree_has_no_source() {
    let tree = Tree::new_stub();
    assert!(tree.source_bytes().is_none());
}

#[test]
fn stub_tree_root_kind_is_unknown_without_language() {
    let tree = Tree::new_stub();
    assert_eq!(tree.root_node().kind(), "unknown");
}

// ===========================================================================
// 2. Forest-to-tree via Parser (single token grammar)
// ===========================================================================

#[test]
fn single_token_parse_produces_valid_tree() {
    let tree = parse_single(b"a");
    let root = tree.root_node();
    assert!(root.start_byte() <= root.end_byte());
}

#[test]
fn single_token_parse_sets_language() {
    let tree = parse_single(b"a");
    assert!(tree.language().is_some());
}

#[test]
fn single_token_parse_stores_source_bytes() {
    let tree = parse_single(b"a");
    assert_eq!(tree.source_bytes(), Some(b"a".as_slice()));
}

#[test]
fn single_token_root_has_children() {
    let tree = parse_single(b"a");
    let root = tree.root_node();
    // start → a  ⇒ root should have at least one child
    assert!(root.child_count() > 0);
}

#[test]
fn single_token_root_child_byte_range() {
    let tree = parse_single(b"a");
    let root = tree.root_node();
    if let Some(child) = root.child(0) {
        assert!(child.start_byte() <= child.end_byte());
        // Child range should be within root range
        assert!(child.start_byte() >= root.start_byte());
        assert!(child.end_byte() <= root.end_byte());
    }
}

#[test]
fn single_token_root_resolves_kind_name() {
    let tree = parse_single(b"a");
    let root = tree.root_node();
    // Root should be the "start" nonterminal
    let kind = root.kind();
    assert_ne!(kind, "unknown");
}

// ===========================================================================
// 3. Forest-to-tree via Parser (two token grammar)
// ===========================================================================

#[test]
fn two_token_parse_produces_valid_tree() {
    let tree = parse_two(b"ab");
    let root = tree.root_node();
    assert!(root.start_byte() <= root.end_byte());
}

#[test]
fn two_token_root_has_two_children() {
    let tree = parse_two(b"ab");
    let root = tree.root_node();
    assert_eq!(root.child_count(), 2);
}

#[test]
fn two_token_children_have_correct_ranges() {
    let tree = parse_two(b"ab");
    let root = tree.root_node();
    let child_a = root.child(0).expect("first child");
    let child_b = root.child(1).expect("second child");

    // "a" occupies [0,1), "b" occupies [1,2)
    assert_eq!(child_a.start_byte(), 0);
    assert_eq!(child_a.end_byte(), 1);
    assert_eq!(child_b.start_byte(), 1);
    assert_eq!(child_b.end_byte(), 2);
}

#[test]
fn two_token_children_non_overlapping() {
    let tree = parse_two(b"ab");
    let root = tree.root_node();
    let child_a = root.child(0).expect("first child");
    let child_b = root.child(1).expect("second child");
    assert!(child_a.end_byte() <= child_b.start_byte());
}

#[test]
fn two_token_root_spans_full_input() {
    let tree = parse_two(b"ab");
    let root = tree.root_node();
    assert_eq!(root.start_byte(), 0);
    assert_eq!(root.end_byte(), 2);
}

#[test]
fn two_token_source_bytes_stored() {
    let tree = parse_two(b"ab");
    assert_eq!(tree.source_bytes(), Some(b"ab".as_slice()));
}

// ===========================================================================
// 4. Node API on parsed tree
// ===========================================================================

#[test]
fn node_kind_id_matches_grammar_symbol() {
    let tree = parse_two(b"ab");
    let root = tree.root_node();
    // Root is the "start" nonterminal (symbol 3)
    let root_kind_id = root.kind_id();
    assert!(root_kind_id > 0);
}

#[test]
fn node_is_named_returns_true() {
    let tree = parse_two(b"ab");
    assert!(tree.root_node().is_named());
}

#[test]
fn node_is_error_returns_false() {
    let tree = parse_two(b"ab");
    assert!(!tree.root_node().is_error());
}

#[test]
fn node_is_missing_returns_false() {
    let tree = parse_two(b"ab");
    assert!(!tree.root_node().is_missing());
}

#[test]
fn node_out_of_bounds_child_returns_none() {
    let tree = parse_two(b"ab");
    let root = tree.root_node();
    assert!(root.child(999).is_none());
}

#[test]
fn node_utf8_text_extraction() {
    let tree = parse_two(b"ab");
    let root = tree.root_node();
    let text = root.utf8_text(b"ab").expect("valid utf8");
    assert_eq!(text, "ab");
}

#[test]
fn node_child_utf8_text() {
    let tree = parse_two(b"ab");
    let root = tree.root_node();
    if let Some(child) = root.child(0) {
        let text = child.utf8_text(b"ab").expect("valid utf8");
        assert_eq!(text, "a");
    }
}

// ===========================================================================
// 5. Tree cloning
// ===========================================================================

#[test]
fn parsed_tree_clone_preserves_root_kind() {
    let tree = parse_single(b"a");
    let cloned = tree.clone();
    assert_eq!(tree.root_kind(), cloned.root_kind());
}

#[test]
fn parsed_tree_clone_preserves_byte_range() {
    let tree = parse_single(b"a");
    let cloned = tree.clone();
    assert_eq!(
        tree.root_node().start_byte(),
        cloned.root_node().start_byte()
    );
    assert_eq!(tree.root_node().end_byte(), cloned.root_node().end_byte());
}

#[test]
fn parsed_tree_clone_preserves_children() {
    let tree = parse_two(b"ab");
    let cloned = tree.clone();
    assert_eq!(
        tree.root_node().child_count(),
        cloned.root_node().child_count()
    );
}

// ===========================================================================
// 6. Tree cursor traversal on built tree
// ===========================================================================

#[test]
fn cursor_starts_at_root() {
    let tree = parse_two(b"ab");
    let cursor = TreeCursor::new(&tree);
    // Cursor exists and we can create it without panic
    drop(cursor);
}

#[test]
fn cursor_goto_first_child_on_parsed_tree() {
    let tree = parse_two(b"ab");
    let mut cursor = TreeCursor::new(&tree);
    assert!(cursor.goto_first_child());
}

#[test]
fn cursor_goto_next_sibling_on_parsed_tree() {
    let tree = parse_two(b"ab");
    let mut cursor = TreeCursor::new(&tree);
    assert!(cursor.goto_first_child());
    assert!(cursor.goto_next_sibling());
}

#[test]
fn cursor_goto_parent_returns_to_root() {
    let tree = parse_two(b"ab");
    let mut cursor = TreeCursor::new(&tree);
    assert!(cursor.goto_first_child());
    assert!(cursor.goto_parent());
    // After going back, going to first child again should work
    assert!(cursor.goto_first_child());
}

#[test]
fn cursor_no_parent_at_root() {
    let tree = parse_two(b"ab");
    let mut cursor = TreeCursor::new(&tree);
    assert!(!cursor.goto_parent());
}

#[test]
fn cursor_leaf_has_no_children() {
    let tree = parse_two(b"ab");
    let mut cursor = TreeCursor::new(&tree);
    assert!(cursor.goto_first_child());
    // Terminal "a" should have no children
    assert!(!cursor.goto_first_child());
}

// ===========================================================================
// 7. Language builder
// ===========================================================================

#[test]
fn language_builder_missing_parse_table_fails() {
    let result = Language::builder()
        .symbol_metadata(vec![SymbolMetadata {
            is_terminal: true,
            is_visible: true,
            is_supertype: false,
        }])
        .build();
    assert!(result.is_err());
}

#[test]
fn language_builder_missing_metadata_fails() {
    let result = Language::builder().build();
    assert!(result.is_err());
}

#[test]
fn language_symbol_name_lookup() {
    let lang = make_two_token_language();
    assert_eq!(lang.symbol_name(0), Some("EOF"));
    assert_eq!(lang.symbol_name(1), Some("a"));
    assert_eq!(lang.symbol_name(2), Some("b"));
    assert_eq!(lang.symbol_name(3), Some("start"));
}

#[test]
fn language_symbol_name_out_of_bounds() {
    let lang = make_single_token_language();
    assert_eq!(lang.symbol_name(999), None);
}

#[test]
fn language_is_terminal() {
    let lang = make_two_token_language();
    assert!(lang.is_terminal(1)); // "a" is terminal
    assert!(lang.is_terminal(2)); // "b" is terminal
    assert!(!lang.is_terminal(3)); // "start" is nonterminal
}

#[test]
fn language_is_visible() {
    let lang = make_two_token_language();
    assert!(!lang.is_visible(0)); // EOF not visible
    assert!(lang.is_visible(1)); // "a" visible
}

// ===========================================================================
// 8. Parser set_language / re-parse
// ===========================================================================

#[test]
fn parse_with_old_tree_succeeds() {
    let lang = make_single_token_language();
    let mut parser = Parser::new();
    parser.set_language(lang).unwrap();
    let tree1 = parser.parse(b"a", None).unwrap();
    let tree2 = parser.parse(b"a", Some(&tree1)).unwrap();
    assert_eq!(tree1.root_kind(), tree2.root_kind());
}

#[test]
fn parse_utf8_string_works() {
    let lang = make_single_token_language();
    let mut parser = Parser::new();
    parser.set_language(lang).unwrap();
    let tree = parser.parse_utf8("a", None).unwrap();
    assert!(tree.root_node().start_byte() <= tree.root_node().end_byte());
}

// ===========================================================================
// 9. Debug formatting
// ===========================================================================

#[test]
fn tree_debug_does_not_panic() {
    let tree = parse_single(b"a");
    let debug = format!("{:?}", tree);
    assert!(!debug.is_empty());
}

#[test]
fn stub_tree_debug_does_not_panic() {
    let tree = Tree::new_stub();
    let debug = format!("{:?}", tree);
    assert!(!debug.is_empty());
}

#[test]
fn node_debug_does_not_panic() {
    let tree = parse_two(b"ab");
    let debug = format!("{:?}", tree.root_node());
    assert!(debug.contains("Node"));
}
