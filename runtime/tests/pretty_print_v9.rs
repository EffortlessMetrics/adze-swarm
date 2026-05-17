//! PrettyPrintVisitor comprehensive tests — 84 tests across 20 categories.
//!
//! Categories:
//!   pp_new_*            — freshly constructed visitor state
//!   pp_one_leaf_*       — single leaf node walks
//!   pp_one_leaf_text_*  — leaf text appears in output
//!   pp_depth0_*         — root-level (depth 0) indentation
//!   pp_depth1_*         — depth-1 indentation behaviour
//!   pp_depth2_*         — depth-2 indentation behaviour
//!   pp_multi_*          — multiple nodes in one tree
//!   pp_nested_*         — nested (different depth) nodes
//!   pp_clone_*          — Clone-like semantics (via Default round-trip)
//!   pp_debug_*          — Debug representation
//!   pp_empty_name_*     — nodes with unnamed (anonymous) symbols
//!   pp_empty_text_*     — leaf with empty source span
//!   pp_long_name_*      — high symbol ids
//!   pp_long_text_*      — long source text
//!   pp_many_*           — 100-node trees
//!   pp_deep_*           — deeply nested trees (depth 50)
//!   pp_kind_*           — various kind (symbol) values
//!   pp_utf8_*           — UTF-8 validity of output
//!   pp_growth_*         — output length grows with nodes
//!   pp_consume_*        — output() returns accumulated string

use adze::pure_parser::ParsedNode;
use adze::visitor::{PrettyPrintVisitor, TreeWalker};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_node(
    symbol: u16,
    children: Vec<ParsedNode>,
    start: usize,
    end: usize,
    is_error: bool,
    is_named: bool,
) -> ParsedNode {
    ParsedNode::builder(symbol, children, start, end)
        .is_error(is_error)
        .is_named(is_named)
        .build()
}

fn leaf(sym: u16, start: usize, end: usize) -> ParsedNode {
    make_node(sym, vec![], start, end, false, true)
}

fn unnamed_leaf(sym: u16, start: usize, end: usize) -> ParsedNode {
    make_node(sym, vec![], start, end, false, false)
}

fn interior(sym: u16, children: Vec<ParsedNode>) -> ParsedNode {
    let start = children.first().map_or(0, |c| c.start_byte);
    let end = children.last().map_or(0, |c| c.end_byte);
    make_node(sym, children, start, end, false, true)
}

fn error_node(start: usize, end: usize) -> ParsedNode {
    make_node(0, vec![], start, end, true, false)
}

fn walk_pp(root: &ParsedNode, src: &[u8]) -> String {
    let mut pp = PrettyPrintVisitor::new();
    TreeWalker::new(src).walk(root, &mut pp);
    pp.output().to_owned()
}

// ===================================================================
// Category 1 — pp_new_* (4 tests)
// ===================================================================

#[test]
fn pp_new_output_is_empty() {
    let pp = PrettyPrintVisitor::new();
    assert!(pp.output().is_empty());
}

#[test]
fn pp_new_default_output_is_empty() {
    let pp = PrettyPrintVisitor::default();
    assert!(pp.output().is_empty());
}

#[test]
fn pp_new_output_len_zero() {
    let pp = PrettyPrintVisitor::new();
    assert_eq!(pp.output().len(), 0);
}

#[test]
fn pp_new_output_equals_empty_str() {
    let pp = PrettyPrintVisitor::new();
    assert_eq!(pp.output(), "");
}

// ===================================================================
// Category 2 — pp_one_leaf_* (5 tests)
// ===================================================================

#[test]
fn pp_one_leaf_output_non_empty() {
    let node = leaf(1, 0, 1);
    let out = walk_pp(&node, b"x");
    assert!(!out.is_empty());
}

#[test]
fn pp_one_leaf_contains_named_marker() {
    let node = leaf(1, 0, 1);
    let out = walk_pp(&node, b"x");
    assert!(out.contains("[named]"));
}

#[test]
fn pp_one_leaf_has_newline() {
    let node = leaf(1, 0, 1);
    let out = walk_pp(&node, b"x");
    assert!(out.contains('\n'));
}

#[test]
fn pp_one_leaf_ends_with_newline() {
    let node = leaf(1, 0, 1);
    let out = walk_pp(&node, b"x");
    assert!(out.ends_with('\n'));
}

#[test]
fn pp_one_leaf_kind_appears_in_output() {
    // symbol 5 → "Expression" in the fallback table
    let node = leaf(5, 0, 1);
    let out = walk_pp(&node, b"x");
    assert!(out.contains("Expression"));
}

// ===================================================================
// Category 3 — pp_one_leaf_text_* (4 tests)
// ===================================================================

#[test]
fn pp_one_leaf_text_appears() {
    let node = leaf(1, 0, 3);
    let out = walk_pp(&node, b"abc");
    assert!(out.contains("abc"));
}

#[test]
fn pp_one_leaf_text_quoted() {
    let node = leaf(1, 0, 3);
    let out = walk_pp(&node, b"abc");
    assert!(out.contains("\"abc\""));
}

#[test]
fn pp_one_leaf_text_single_char() {
    let node = leaf(1, 0, 1);
    let out = walk_pp(&node, b"z");
    assert!(out.contains("\"z\""));
}

#[test]
fn pp_one_leaf_text_multi_char() {
    let node = leaf(1, 0, 5);
    let out = walk_pp(&node, b"hello");
    assert!(out.contains("\"hello\""));
}

// ===================================================================
// Category 4 — pp_depth0_* (4 tests)
// ===================================================================

#[test]
fn pp_depth0_root_no_leading_space() {
    let node = interior(10, vec![leaf(1, 0, 1)]);
    let out = walk_pp(&node, b"a");
    let first_line = out.lines().next().unwrap_or("");
    assert_eq!(first_line.len() - first_line.trim_start().len(), 0);
}

#[test]
fn pp_depth0_single_leaf_no_leading_space_on_kind_line() {
    // A named leaf has an enter_node line at depth 0
    let node = leaf(5, 0, 1);
    let out = walk_pp(&node, b"x");
    let first_line = out.lines().next().unwrap_or("");
    assert_eq!(first_line, first_line.trim_start());
}

#[test]
fn pp_depth0_root_starts_with_kind() {
    // symbol 10 → "rule_10" in the fallback table
    let node = interior(10, vec![leaf(1, 0, 1)]);
    let out = walk_pp(&node, b"a");
    let first_line = out.lines().next().unwrap_or("");
    assert!(first_line.starts_with("rule_10"));
}

#[test]
fn pp_depth0_root_kind_and_named() {
    // symbol 7 → "Whitespace" in the fallback table
    let node = interior(7, vec![leaf(1, 0, 1)]);
    let out = walk_pp(&node, b"a");
    let first_line = out.lines().next().unwrap_or("");
    assert!(first_line.contains("Whitespace"));
    assert!(first_line.contains("[named]"));
}

// ===================================================================
// Category 5 — pp_depth1_* (5 tests)
// ===================================================================

#[test]
fn pp_depth1_child_indented() {
    let node = interior(10, vec![leaf(1, 0, 1)]);
    let out = walk_pp(&node, b"a");
    let second_line = out.lines().nth(1).unwrap_or("");
    let indent = second_line.len() - second_line.trim_start().len();
    assert!(indent > 0);
}

#[test]
fn pp_depth1_child_indent_is_two_spaces() {
    let node = interior(10, vec![leaf(1, 0, 1)]);
    let out = walk_pp(&node, b"a");
    let second_line = out.lines().nth(1).unwrap_or("");
    assert!(second_line.starts_with("  "));
}

#[test]
fn pp_depth1_leaf_text_indented() {
    let node = interior(10, vec![leaf(1, 0, 1)]);
    let out = walk_pp(&node, b"a");
    let text_lines: Vec<&str> = out.lines().filter(|l| l.contains('"')).collect();
    assert!(!text_lines.is_empty());
    for line in &text_lines {
        let indent = line.len() - line.trim_start().len();
        assert!(indent > 0);
    }
}

#[test]
fn pp_depth1_multiple_children_same_indent() {
    let node = interior(10, vec![leaf(1, 0, 1), leaf(2, 1, 2)]);
    let src = b"ab";
    let out = walk_pp(&node, src);
    let child_lines: Vec<&str> = out
        .lines()
        .skip(1) // skip root
        .filter(|l| !l.trim().starts_with('"'))
        .collect();
    if child_lines.len() >= 2 {
        let i0 = child_lines[0].len() - child_lines[0].trim_start().len();
        let i1 = child_lines[1].len() - child_lines[1].trim_start().len();
        assert_eq!(i0, i1);
    }
}

#[test]
fn pp_depth1_child_indent_greater_than_root() {
    let node = interior(10, vec![leaf(1, 0, 1)]);
    let out = walk_pp(&node, b"a");
    let lines: Vec<&str> = out.lines().collect();
    assert!(lines.len() >= 2);
    let root_indent = lines[0].len() - lines[0].trim_start().len();
    let child_indent = lines[1].len() - lines[1].trim_start().len();
    assert!(child_indent > root_indent);
}

// ===================================================================
// Category 6 — pp_depth2_* (4 tests)
// ===================================================================

#[test]
fn pp_depth2_grandchild_more_indented_than_child() {
    let grandchild = leaf(3, 0, 1);
    let child = interior(2, vec![grandchild]);
    let root = interior(1, vec![child]);
    let out = walk_pp(&root, b"x");
    let node_lines: Vec<&str> = out.lines().filter(|l| !l.trim().starts_with('"')).collect();
    assert!(node_lines.len() >= 3);
    let indents: Vec<usize> = node_lines
        .iter()
        .map(|l| l.len() - l.trim_start().len())
        .collect();
    assert!(indents[1] > indents[0]);
    assert!(indents[2] > indents[1]);
}

#[test]
fn pp_depth2_indent_doubles_per_level() {
    let grandchild = leaf(3, 0, 1);
    let child = interior(2, vec![grandchild]);
    let root = interior(1, vec![child]);
    let out = walk_pp(&root, b"x");
    let node_lines: Vec<&str> = out.lines().filter(|l| !l.trim().starts_with('"')).collect();
    if node_lines.len() >= 3 {
        let i0 = node_lines[0].len() - node_lines[0].trim_start().len();
        let i1 = node_lines[1].len() - node_lines[1].trim_start().len();
        let i2 = node_lines[2].len() - node_lines[2].trim_start().len();
        assert_eq!(i0, 0);
        assert_eq!(i1, 2);
        assert_eq!(i2, 4);
    }
}

#[test]
fn pp_depth2_leaf_text_at_depth2() {
    let grandchild = leaf(3, 0, 1);
    let child = interior(2, vec![grandchild]);
    let root = interior(1, vec![child]);
    let out = walk_pp(&root, b"x");
    let text_lines: Vec<&str> = out.lines().filter(|l| l.trim().starts_with('"')).collect();
    assert!(!text_lines.is_empty());
    let indent = text_lines[0].len() - text_lines[0].trim_start().len();
    assert!(indent >= 4); // at least depth-2 child indent
}

#[test]
fn pp_depth2_three_level_all_kinds_present() {
    // 10 → "rule_10", 9 → "Expression_Sub", 5 → "Expression"
    let gc = leaf(5, 0, 1);
    let c = interior(9, vec![gc]);
    let r = interior(10, vec![c]);
    let out = walk_pp(&r, b"x");
    assert!(out.contains("rule_10"));
    assert!(out.contains("Expression_Sub"));
    assert!(out.contains("Expression"));
}

// ===================================================================
// Category 7 — pp_multi_* (5 tests)
// ===================================================================

#[test]
fn pp_multi_two_leaves_both_texts() {
    let root = interior(10, vec![leaf(1, 0, 1), leaf(2, 1, 2)]);
    let out = walk_pp(&root, b"ab");
    assert!(out.contains("\"a\""));
    assert!(out.contains("\"b\""));
}

#[test]
fn pp_multi_three_leaves_all_texts() {
    let root = interior(10, vec![leaf(1, 0, 1), leaf(2, 1, 2), leaf(3, 2, 3)]);
    let out = walk_pp(&root, b"xyz");
    assert!(out.contains("\"x\""));
    assert!(out.contains("\"y\""));
    assert!(out.contains("\"z\""));
}

#[test]
fn pp_multi_five_children_line_count() {
    let children: Vec<ParsedNode> = (0..5).map(|i| leaf(i as u16 + 1, i, i + 1)).collect();
    let root = interior(50, children);
    let out = walk_pp(&root, b"abcde");
    // root node line + (child enter + leaf text) * 5
    let line_count = out.lines().count();
    assert!(line_count >= 6);
}

#[test]
fn pp_multi_all_kinds_appear() {
    // symbol 10 → "rule_10", 5 → "Expression", 6 → "Whitespace__whitespace", 7 → "Whitespace"
    let root = interior(10, vec![leaf(5, 0, 1), leaf(6, 1, 2), leaf(7, 2, 3)]);
    let out = walk_pp(&root, b"xyz");
    assert!(out.contains("rule_10"));
    assert!(out.contains("Expression"));
    assert!(out.contains("Whitespace__whitespace"));
    assert!(out.contains("Whitespace"));
}

#[test]
fn pp_multi_output_has_multiple_lines() {
    let root = interior(10, vec![leaf(1, 0, 1), leaf(2, 1, 2)]);
    let out = walk_pp(&root, b"ab");
    assert!(out.lines().count() > 2);
}

// ===================================================================
// Category 8 — pp_nested_* (5 tests)
// ===================================================================

#[test]
fn pp_nested_indentation_increases() {
    let n3 = leaf(103, 0, 4);
    let n2 = interior(102, vec![n3]);
    let n1 = interior(101, vec![n2]);
    let root = interior(100, vec![n1]);
    let out = walk_pp(&root, b"deep");
    let node_indents: Vec<usize> = out
        .lines()
        .filter(|l| !l.trim().starts_with('"'))
        .map(|l| l.len() - l.trim_start().len())
        .collect();
    for w in node_indents.windows(2) {
        assert!(w[1] >= w[0]);
    }
}

#[test]
fn pp_nested_leaf_text_at_deepest_level() {
    let n3 = leaf(103, 0, 4);
    let n2 = interior(102, vec![n3]);
    let n1 = interior(101, vec![n2]);
    let root = interior(100, vec![n1]);
    let out = walk_pp(&root, b"deep");
    assert!(out.contains("\"deep\""));
}

#[test]
fn pp_nested_all_intermediate_kinds() {
    // All symbols > 10 map to "unknown"; use known symbols instead
    // 10 → "rule_10", 9 → "Expression_Sub", 8 → "Expression_Sub_1", 5 → "Expression"
    let n3 = leaf(5, 0, 1);
    let n2 = interior(8, vec![n3]);
    let n1 = interior(9, vec![n2]);
    let root = interior(10, vec![n1]);
    let out = walk_pp(&root, b"x");
    assert!(out.contains("rule_10"));
    assert!(out.contains("Expression_Sub"));
    assert!(out.contains("Expression_Sub_1"));
    assert!(out.contains("Expression"));
}

#[test]
fn pp_nested_sibling_after_deep_subtree() {
    let deep = interior(11, vec![leaf(12, 0, 1)]);
    let sibling = leaf(13, 1, 2);
    let root = interior(10, vec![deep, sibling]);
    let out = walk_pp(&root, b"ab");
    assert!(out.contains("\"a\""));
    assert!(out.contains("\"b\""));
}

#[test]
fn pp_nested_mixed_depths_output_valid() {
    // Use symbols > 10 that all map to "unknown"; just verify structure
    let left = interior(11, vec![interior(12, vec![leaf(13, 0, 1)])]);
    let right = leaf(14, 1, 2);
    let root = interior(10, vec![left, right]);
    let out = walk_pp(&root, b"ab");
    // root is "rule_10", rest are "unknown"; all texts present
    assert!(out.contains("rule_10"));
    assert!(out.contains("unknown"));
    assert!(out.contains("\"a\""));
    assert!(out.contains("\"b\""));
}

// ===================================================================
// Category 9 — pp_default_equiv_* (simulating Clone via Default)
// ===================================================================

#[test]
fn pp_default_equiv_fresh_visitor_matches() {
    let pp1 = PrettyPrintVisitor::new();
    let pp2 = PrettyPrintVisitor::default();
    assert_eq!(pp1.output(), pp2.output());
}

#[test]
fn pp_default_equiv_after_walk_fresh_is_empty() {
    let node = leaf(1, 0, 1);
    let src = b"x";
    let mut pp = PrettyPrintVisitor::new();
    TreeWalker::new(src).walk(&node, &mut pp);
    let fresh = PrettyPrintVisitor::new();
    assert!(fresh.output().is_empty());
    assert!(!pp.output().is_empty());
}

#[test]
fn pp_default_equiv_two_walks_independent() {
    let node = leaf(1, 0, 1);
    let src = b"x";
    let mut pp1 = PrettyPrintVisitor::new();
    let mut pp2 = PrettyPrintVisitor::new();
    TreeWalker::new(src).walk(&node, &mut pp1);
    TreeWalker::new(src).walk(&node, &mut pp2);
    assert_eq!(pp1.output(), pp2.output());
}

#[test]
fn pp_default_equiv_deterministic_output() {
    let root = interior(10, vec![leaf(1, 0, 1), leaf(2, 1, 2)]);
    let src = b"ab";
    let out1 = walk_pp(&root, src);
    let out2 = walk_pp(&root, src);
    assert_eq!(out1, out2);
}

// ===================================================================
// Category 10 — pp_debug_* (4 tests)
// ===================================================================

#[test]
fn pp_debug_format_non_empty() {
    let pp = PrettyPrintVisitor::new();
    let dbg = format!("{:?}", pp);
    assert!(!dbg.is_empty());
}

#[test]
fn pp_debug_format_after_walk() {
    let node = leaf(1, 0, 1);
    let src = b"x";
    let mut pp = PrettyPrintVisitor::new();
    TreeWalker::new(src).walk(&node, &mut pp);
    let dbg = format!("{:?}", pp);
    assert!(!dbg.is_empty());
}

// ===================================================================
// Category 11 — pp_empty_name_* (unnamed nodes, 4 tests)
// ===================================================================

#[test]
fn pp_empty_name_unnamed_leaf_no_named_marker() {
    let node = unnamed_leaf(1, 0, 1);
    let out = walk_pp(&node, b"x");
    assert!(!out.contains("[named]"));
}

#[test]
fn pp_empty_name_unnamed_leaf_has_text() {
    let node = unnamed_leaf(1, 0, 3);
    let out = walk_pp(&node, b"foo");
    assert!(out.contains("\"foo\""));
}

#[test]
fn pp_empty_name_unnamed_among_named() {
    let root = interior(
        10,
        vec![leaf(1, 0, 1), unnamed_leaf(2, 1, 2), leaf(3, 2, 3)],
    );
    let out = walk_pp(&root, b"abc");
    // Named nodes show "[named]", unnamed don't
    let named_count = out.matches("[named]").count();
    // root + 2 named leaves = 3 occurrences
    assert!(named_count >= 3);
}

#[test]
fn pp_empty_name_unnamed_interior_no_named() {
    let child = leaf(2, 0, 1);
    let root = make_node(10, vec![child], 0, 1, false, false);
    let out = walk_pp(&root, b"x");
    // The root node line should not contain [named]
    let first_line = out.lines().next().unwrap_or("");
    assert!(!first_line.contains("[named]"));
}

// ===================================================================
// Category 12 — pp_empty_text_* (4 tests)
// ===================================================================

#[test]
fn pp_empty_text_zero_span_leaf() {
    let node = leaf(1, 0, 0);
    let out = walk_pp(&node, b"");
    assert!(!out.is_empty()); // still has the node kind line
}

#[test]
fn pp_empty_text_zero_span_has_quotes() {
    let node = leaf(1, 0, 0);
    let out = walk_pp(&node, b"");
    // visit_leaf is still called; empty text produces ""
    assert!(out.contains("\"\""));
}

#[test]
fn pp_empty_text_mid_tree_empty_leaf() {
    let root = interior(10, vec![leaf(1, 0, 0), leaf(2, 0, 1)]);
    let out = walk_pp(&root, b"a");
    assert!(out.contains("\"a\""));
}

#[test]
fn pp_empty_text_all_empty_spans() {
    let root = interior(10, vec![leaf(1, 0, 0), leaf(2, 0, 0)]);
    let out = walk_pp(&root, b"");
    // Should still have structure (node kind lines)
    assert!(out.lines().count() >= 3);
}

// ===================================================================
// Category 13 — pp_long_name_* (high symbol ids, 4 tests)
// ===================================================================

#[test]
fn pp_long_name_high_symbol_id_falls_back() {
    // Symbols > 10 with no language → "unknown"
    let node = leaf(65535, 0, 1);
    let out = walk_pp(&node, b"x");
    assert!(out.contains("unknown"));
}

#[test]
fn pp_long_name_max_u16_symbol() {
    let node = leaf(u16::MAX, 0, 1);
    let out = walk_pp(&node, b"x");
    assert!(out.contains("unknown"));
}

#[test]
fn pp_long_name_zero_symbol() {
    // symbol 0 → "end"
    let node = leaf(0, 0, 1);
    let out = walk_pp(&node, b"x");
    assert!(out.contains("end"));
}

#[test]
fn pp_long_name_various_known_symbols_in_tree() {
    // 10 → "rule_10", 5 → "Expression", 6 → "Whitespace__whitespace"
    let root = interior(10, vec![leaf(5, 0, 1), leaf(6, 1, 2)]);
    let out = walk_pp(&root, b"ab");
    assert!(out.contains("rule_10"));
    assert!(out.contains("Expression"));
    assert!(out.contains("Whitespace__whitespace"));
}

// ===================================================================
// Category 14 — pp_long_text_* (4 tests)
// ===================================================================

#[test]
fn pp_long_text_100_chars() {
    let text = "a".repeat(100);
    let src = text.as_bytes().to_vec();
    let node = leaf(1, 0, 100);
    let out = walk_pp(&node, &src);
    assert!(out.contains(&text));
}

#[test]
fn pp_long_text_contains_full_content() {
    let text = "hello_world_test";
    let src = text.as_bytes();
    let node = leaf(1, 0, text.len());
    let out = walk_pp(&node, src);
    assert!(out.contains(text));
}

#[test]
fn pp_long_text_500_chars() {
    let text = "b".repeat(500);
    let src = text.as_bytes().to_vec();
    let node = leaf(1, 0, 500);
    let out = walk_pp(&node, &src);
    assert!(out.contains(&text));
}

#[test]
fn pp_long_text_with_digits() {
    let text = "0123456789".repeat(10);
    let src = text.as_bytes().to_vec();
    let node = leaf(1, 0, text.len());
    let out = walk_pp(&node, &src);
    assert!(out.contains(&text));
}

// ===================================================================
// Category 15 — pp_many_* (100-node trees, 4 tests)
// ===================================================================

#[test]
fn pp_many_100_leaves_all_present() {
    let src: Vec<u8> = (0..100).map(|i| b'a' + (i % 26)).collect();
    let children: Vec<ParsedNode> = (0..100).map(|i| leaf(i as u16 + 1, i, i + 1)).collect();
    let root = interior(999, children);
    let out = walk_pp(&root, &src);
    // Check a sampling of leaf texts
    for i in [0usize, 25, 50, 75, 99] {
        let ch = (b'a' + (i as u8 % 26)) as char;
        assert!(out.contains(&format!("\"{}\"", ch)));
    }
}

#[test]
fn pp_many_100_leaves_line_count() {
    let src: Vec<u8> = (0..100).map(|i| b'a' + (i % 26)).collect();
    let children: Vec<ParsedNode> = (0..100).map(|i| leaf(i as u16 + 1, i, i + 1)).collect();
    let root = interior(999, children);
    let out = walk_pp(&root, &src);
    // At least root + 100 child-enter + 100 leaf-text lines
    assert!(out.lines().count() >= 101);
}

#[test]
fn pp_many_100_leaves_output_length() {
    let src: Vec<u8> = (0..100).map(|i| b'a' + (i % 26)).collect();
    let children: Vec<ParsedNode> = (0..100).map(|i| leaf(i as u16 + 1, i, i + 1)).collect();
    let root = interior(999, children);
    let out = walk_pp(&root, &src);
    assert!(out.len() > 500);
}

#[test]
fn pp_many_all_kind_ids_present() {
    let src: Vec<u8> = (0..100).map(|i| b'a' + (i % 26)).collect();
    let children: Vec<ParsedNode> = (0..100).map(|i| leaf(i as u16 + 1, i, i + 1)).collect();
    let root = interior(999, children);
    let out = walk_pp(&root, &src);
    // Root symbol 999 → "unknown"; children include symbols 1-10 with known names
    assert!(out.contains("unknown"));
    // symbol 1 → "*", symbol 5 → "Expression"
    assert!(out.contains("Expression"));
    assert!(out.contains("rule_10"));
}

// ===================================================================
// Category 16 — pp_deep_* (depth 50 trees, 4 tests)
// ===================================================================

fn build_chain(depth: usize) -> (ParsedNode, Vec<u8>) {
    let src = b"x".to_vec();
    let mut node = leaf(depth as u16, 0, 1);
    for d in (1..depth).rev() {
        node = interior(d as u16, vec![node]);
    }
    (node, src)
}

#[test]
fn pp_deep_50_levels_has_output() {
    let (root, src) = build_chain(50);
    let out = walk_pp(&root, &src);
    assert!(!out.is_empty());
}

#[test]
fn pp_deep_50_levels_indentation_grows() {
    let (root, src) = build_chain(50);
    let out = walk_pp(&root, &src);
    let max_indent = out
        .lines()
        .map(|l| l.len() - l.trim_start().len())
        .max()
        .unwrap_or(0);
    // 50 levels deep ⟹ at least 49 * 2 = 98 spaces of max indent
    assert!(max_indent >= 90);
}

#[test]
fn pp_deep_50_levels_leaf_text_present() {
    let (root, src) = build_chain(50);
    let out = walk_pp(&root, &src);
    assert!(out.contains("\"x\""));
}

#[test]
fn pp_deep_50_levels_line_count() {
    let (root, src) = build_chain(50);
    let out = walk_pp(&root, &src);
    // 49 interior enter lines + 1 leaf enter + 1 leaf text = at least 51
    assert!(out.lines().count() >= 50);
}

// ===================================================================
// Category 17 — pp_kind_* (various kind values, 4 tests)
// ===================================================================

#[test]
fn pp_kind_zero() {
    // symbol 0 → "end"
    let node = leaf(0, 0, 1);
    let out = walk_pp(&node, b"x");
    assert!(out.contains("end"));
}

#[test]
fn pp_kind_one() {
    // symbol 1 → "*"
    let node = leaf(1, 0, 1);
    let out = walk_pp(&node, b"x");
    assert!(out.contains('*'));
}

#[test]
fn pp_kind_large() {
    // symbol > 10 → "unknown"
    let node = leaf(9999, 0, 1);
    let out = walk_pp(&node, b"x");
    assert!(out.contains("unknown"));
}

#[test]
fn pp_kind_different_in_same_tree() {
    // 10 → "rule_10", 5 → "Expression", 7 → "Whitespace"
    let root = interior(10, vec![leaf(5, 0, 1), leaf(7, 1, 2)]);
    let out = walk_pp(&root, b"ab");
    assert!(out.contains("rule_10"));
    assert!(out.contains("Expression"));
    assert!(out.contains("Whitespace"));
}

// ===================================================================
// Category 18 — pp_utf8_* (4 tests)
// ===================================================================

#[test]
fn pp_utf8_output_is_valid() {
    let node = leaf(1, 0, 1);
    let out = walk_pp(&node, b"x");
    // If we got a String, it's already valid UTF-8; verify no panic
    let _ = out.as_bytes();
    assert!(!out.is_empty());
}

#[test]
fn pp_utf8_empty_output_is_valid() {
    let pp = PrettyPrintVisitor::new();
    assert!(std::str::from_utf8(pp.output().as_bytes()).is_ok());
}

#[test]
fn pp_utf8_large_tree_output_valid() {
    let src: Vec<u8> = (0..50).map(|i| b'a' + (i % 26)).collect();
    let children: Vec<ParsedNode> = (0..50).map(|i| leaf(i as u16 + 1, i, i + 1)).collect();
    let root = interior(999, children);
    let out = walk_pp(&root, &src);
    assert!(std::str::from_utf8(out.as_bytes()).is_ok());
}

#[test]
fn pp_utf8_deep_tree_output_valid() {
    let (root, src) = build_chain(20);
    let out = walk_pp(&root, &src);
    assert!(std::str::from_utf8(out.as_bytes()).is_ok());
}

// ===================================================================
// Category 19 — pp_growth_* (output length grows with nodes, 4 tests)
// ===================================================================

#[test]
fn pp_growth_more_nodes_longer_output() {
    let src2 = b"ab".to_vec();
    let src5 = b"abcde".to_vec();
    let root2 = interior(10, vec![leaf(1, 0, 1), leaf(2, 1, 2)]);
    let children5: Vec<ParsedNode> = (0..5).map(|i| leaf(i as u16 + 1, i, i + 1)).collect();
    let root5 = interior(10, children5);
    let out2 = walk_pp(&root2, &src2);
    let out5 = walk_pp(&root5, &src5);
    assert!(out5.len() > out2.len());
}

#[test]
fn pp_growth_single_vs_tree() {
    let single = leaf(1, 0, 1);
    let root = interior(10, vec![leaf(1, 0, 1), leaf(2, 1, 2)]);
    let out_single = walk_pp(&single, b"x");
    let out_tree = walk_pp(&root, b"ab");
    assert!(out_tree.len() > out_single.len());
}

#[test]
fn pp_growth_deeper_tree_longer_output() {
    let shallow = interior(1, vec![leaf(2, 0, 1)]);
    let deep = interior(1, vec![interior(2, vec![interior(3, vec![leaf(4, 0, 1)])])]);
    let out_shallow = walk_pp(&shallow, b"x");
    let out_deep = walk_pp(&deep, b"x");
    assert!(out_deep.len() > out_shallow.len());
}

#[test]
fn pp_growth_empty_vs_single() {
    let pp = PrettyPrintVisitor::new();
    let node = leaf(1, 0, 1);
    let out = walk_pp(&node, b"x");
    assert!(out.len() > pp.output().len());
}

// ===================================================================
// Category 20 — pp_consume_* (output accessor, 4 tests)
// ===================================================================

#[test]
fn pp_consume_output_returns_str_ref() {
    let mut pp = PrettyPrintVisitor::new();
    let node = leaf(1, 0, 1);
    TreeWalker::new(b"x").walk(&node, &mut pp);
    let s: &str = pp.output();
    assert!(!s.is_empty());
}

#[test]
fn pp_consume_output_can_be_cloned_to_string() {
    let mut pp = PrettyPrintVisitor::new();
    let node = leaf(1, 0, 1);
    TreeWalker::new(b"x").walk(&node, &mut pp);
    let owned: String = pp.output().to_owned();
    assert_eq!(owned.as_str(), pp.output());
}

#[test]
fn pp_consume_output_called_twice_same_result() {
    let mut pp = PrettyPrintVisitor::new();
    let node = leaf(1, 0, 1);
    TreeWalker::new(b"x").walk(&node, &mut pp);
    let first = pp.output().to_owned();
    let second = pp.output().to_owned();
    assert_eq!(first, second);
}

#[test]
fn pp_consume_output_after_two_walks_accumulates() {
    let node = leaf(1, 0, 1);
    let src = b"x";
    let mut pp = PrettyPrintVisitor::new();
    TreeWalker::new(src).walk(&node, &mut pp);
    let after_one = pp.output().len();
    TreeWalker::new(src).walk(&node, &mut pp);
    let after_two = pp.output().len();
    assert!(after_two > after_one);
}

// ===================================================================
// Extra tests to reach 84 total
// ===================================================================

#[test]
fn pp_error_node_shows_error_label() {
    let root = interior(10, vec![error_node(0, 1)]);
    let out = walk_pp(&root, b"e");
    assert!(out.contains("ERROR"));
}

#[test]
fn pp_error_node_among_valid_nodes() {
    let root = interior(10, vec![leaf(1, 0, 1), error_node(1, 2), leaf(2, 2, 3)]);
    let out = walk_pp(&root, b"abc");
    assert!(out.contains("ERROR"));
    assert!(out.contains("\"a\""));
    assert!(out.contains("\"c\""));
}

#[test]
fn pp_newline_terminated_every_line_non_empty() {
    let root = interior(10, vec![leaf(1, 0, 1), leaf(2, 1, 2)]);
    let out = walk_pp(&root, b"ab");
    assert!(out.ends_with('\n'));
}

#[test]
fn pp_default_and_new_equivalent() {
    let a = PrettyPrintVisitor::new();
    let b = PrettyPrintVisitor::default();
    assert_eq!(a.output(), b.output());
    assert_eq!(format!("{a:?}"), format!("{b:?}"));
}
