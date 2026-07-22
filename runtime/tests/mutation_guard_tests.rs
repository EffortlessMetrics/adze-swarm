//! Mutation-guard tests for the adze runtime crate.
//!
//! Each test is designed to fail if a common mutation (the kind `cargo-mutants`
//! would generate) is applied. Categories covered:
//!
//! 1. Off-by-one errors in byte/position calculations
//! 2. Wrong comparison operators (< vs <=, > vs >=)
//! 3. Missing null/empty checks
//! 4. Wrong arithmetic operators (+/-)
//! 5. Boolean logic inversions
//! 6. Missing break/continue in loops
//! 7. Return value substitutions (Ok vs Err, Some vs None)
//! 8. Wrong field access (start vs end, row vs column)

// ---------------------------------------------------------------------------
// Imports
// ---------------------------------------------------------------------------

use adze::arena_allocator::{NodeHandle, TreeArena, TreeNode};
use adze::linecol::LineCol;
use adze::tree_node_data::TreeNodeData;

// ===================================================================
// 1. Off-by-one errors in byte position calculations
// ===================================================================

#[test]
fn off_by_one_tree_node_data_start_byte() {
    let data = TreeNodeData::new(1, 5, 10);
    assert_eq!(
        data.start_byte(),
        5,
        "start_byte must be exactly 5, not 4 or 6"
    );
}

#[test]
fn off_by_one_tree_node_data_end_byte() {
    let data = TreeNodeData::new(1, 5, 10);
    assert_eq!(
        data.end_byte(),
        10,
        "end_byte must be exactly 10, not 9 or 11"
    );
}

#[test]
fn off_by_one_tree_node_data_byte_len() {
    let data = TreeNodeData::new(1, 10, 20);
    assert_eq!(data.byte_len(), 10, "byte_len = end - start = 10");
}

#[test]
fn off_by_one_tree_node_data_zero_length() {
    let data = TreeNodeData::new(1, 5, 5);
    assert_eq!(data.byte_len(), 0, "zero-length node must have byte_len 0");
}

#[test]
fn off_by_one_tree_node_data_single_byte() {
    let data = TreeNodeData::new(1, 7, 8);
    assert_eq!(data.byte_len(), 1, "single-byte span");
    assert_eq!(data.start_byte(), 7);
    assert_eq!(data.end_byte(), 8);
}

#[test]
fn off_by_one_linecol_column_boundary() {
    // "ab\ncd" => line 1 starts at byte 3
    let lc = LineCol::at_position(b"ab\ncd", 4);
    assert_eq!(lc.column(3), 0, "column at line start is 0, not 1");
    assert_eq!(lc.column(4), 1, "one byte past line start is column 1");
}

#[test]
fn off_by_one_linecol_just_before_newline() {
    let lc = LineCol::at_position(b"abc\ndef", 3);
    assert_eq!(lc.line, 0, "position of the newline itself is still line 0");
}

#[test]
fn off_by_one_linecol_just_after_newline() {
    let lc = LineCol::at_position(b"abc\ndef", 4);
    assert_eq!(lc.line, 1, "position right after newline is line 1");
    assert_eq!(lc.line_start, 4);
}

#[test]
fn off_by_one_linecol_process_byte_offset() {
    let mut lc = LineCol::new();
    // process a newline at offset 5 => new line starts at 6
    lc.process_byte(b'\n', None, 5);
    assert_eq!(lc.line_start, 6, "line_start = offset + 1");
    assert_eq!(lc.line, 1);
}

#[test]
fn off_by_one_arena_len_after_alloc() {
    let mut arena = TreeArena::new();
    assert_eq!(arena.len(), 0);
    arena.alloc(TreeNode::leaf(1));
    assert_eq!(arena.len(), 1, "len must be exactly 1 after one alloc");
    arena.alloc(TreeNode::leaf(2));
    assert_eq!(arena.len(), 2, "len must be exactly 2 after two allocs");
}

// ===================================================================
// 2. Wrong comparison operators (< vs <=, > vs >=)
// ===================================================================

#[test]
fn comparison_tree_node_data_child_boundary() {
    let h0 = NodeHandle::new(0, 0);
    let h1 = NodeHandle::new(0, 1);
    let data = TreeNodeData::branch(1, 0, 10, vec![h0, h1]);
    assert!(data.child(0).is_some());
    assert!(data.child(1).is_some());
    assert!(
        data.child(2).is_none(),
        "index 2 out of bounds for 2 children"
    );
}

#[test]
fn comparison_tree_node_data_child_zero_on_leaf() {
    let data = TreeNodeData::leaf(1, 0, 5);
    assert!(data.child(0).is_none(), "leaf has no children at index 0");
}

#[test]
fn comparison_linecol_empty_input() {
    let lc = LineCol::at_position(b"", 0);
    assert_eq!(lc.line, 0);
    assert_eq!(lc.line_start, 0);
    assert_eq!(lc.column(0), 0);
}

#[test]
fn comparison_arena_is_full_boundary() {
    let mut arena = TreeArena::with_capacity(2);
    arena.alloc(TreeNode::leaf(1));
    arena.alloc(TreeNode::leaf(2));
    // Third alloc should trigger new chunk, not panic
    let h = arena.alloc(TreeNode::leaf(3));
    assert_eq!(arena.get(h).value(), 3);
    assert_eq!(arena.len(), 3);
}

#[test]
fn comparison_linecol_at_position_exactly_at_end() {
    let input = b"ab";
    let lc = LineCol::at_position(input, 2); // exactly at end
    assert_eq!(lc.line, 0);
    assert_eq!(lc.column(2), 2);
}

#[test]
fn comparison_linecol_at_position_beyond_end() {
    let input = b"ab";
    let lc = LineCol::at_position(input, 100); // beyond end
    assert_eq!(lc.line, 0, "no newlines, so still line 0");
}

// ===================================================================
// 3. Missing null/empty checks
// ===================================================================

#[test]
fn empty_check_tree_node_data_leaf() {
    let data = TreeNodeData::leaf(1, 0, 5);
    assert!(data.is_leaf());
    assert_eq!(data.child_count(), 0);
    assert!(data.child(0).is_none());
    assert!(data.children().is_empty());
}

#[test]
fn empty_check_tree_arena_fresh() {
    let arena = TreeArena::new();
    assert!(arena.is_empty());
    assert_eq!(arena.len(), 0);
}

#[test]
fn empty_check_tree_arena_after_reset() {
    let mut arena = TreeArena::new();
    arena.alloc(TreeNode::leaf(42));
    assert!(!arena.is_empty());
    arena.reset();
    assert!(arena.is_empty());
    assert_eq!(arena.len(), 0);
}

#[test]
fn empty_check_field_id_none_by_default() {
    let data = TreeNodeData::new(1, 0, 10);
    assert_eq!(data.field_id(), None, "field_id should be None by default");
}

#[test]
fn empty_check_named_child_count_zero_initially() {
    let data = TreeNodeData::new(1, 0, 10);
    assert_eq!(data.named_child_count(), 0);
}

#[test]
fn empty_check_tree_node_data_no_children_initially() {
    let data = TreeNodeData::new(1, 0, 10);
    assert_eq!(data.child_count(), 0);
    assert!(data.children().is_empty());
}

#[test]
fn empty_check_arena_clear_works() {
    let mut arena = TreeArena::new();
    arena.alloc(TreeNode::leaf(1));
    arena.alloc(TreeNode::leaf(2));
    arena.clear();
    assert!(arena.is_empty());
}

// ===================================================================
// 4. Wrong arithmetic operators (+/-)
// ===================================================================

#[test]
fn arithmetic_byte_len_is_difference_not_sum() {
    let data = TreeNodeData::new(1, 10, 30);
    assert_eq!(
        data.byte_len(),
        20,
        "byte_len = 30 - 10 = 20, not 30 + 10 = 40"
    );
}

#[test]
fn arithmetic_byte_len_saturates_not_wraps() {
    let data = TreeNodeData::new(1, 20, 10);
    assert_eq!(
        data.byte_len(),
        0,
        "saturating_sub should yield 0, not underflow"
    );
}

#[test]
fn arithmetic_linecol_column_is_subtraction() {
    let lc = LineCol {
        line: 1,
        line_start: 10,
    };
    assert_eq!(lc.column(15), 5, "column = position - line_start = 5");
}

#[test]
fn arithmetic_linecol_column_saturates() {
    let lc = LineCol {
        line: 1,
        line_start: 10,
    };
    assert_eq!(
        lc.column(5),
        0,
        "position < line_start should saturate to 0"
    );
}

#[test]
fn arithmetic_linecol_advance_increments_line() {
    let mut lc = LineCol::new();
    lc.advance_line(10);
    assert_eq!(lc.line, 1, "advance_line should add 1, not subtract");
    lc.advance_line(20);
    assert_eq!(lc.line, 2, "second advance should give line 2");
}

#[test]
fn arithmetic_linecol_line_count_after_multiple_newlines() {
    let input = b"a\nb\nc\nd\n";
    let lc = LineCol::at_position(input, input.len());
    assert_eq!(lc.line, 4, "4 newlines = 4 line advances");
}

#[test]
fn arithmetic_arena_len_matches_alloc_count() {
    let mut arena = TreeArena::new();
    for i in 0..10 {
        arena.alloc(TreeNode::leaf(i));
    }
    assert_eq!(arena.len(), 10, "len must equal number of allocs");
}

// ===================================================================
// 5. Boolean logic inversions
// ===================================================================

#[test]
fn bool_tree_node_data_is_error_true_when_set() {
    let mut data = TreeNodeData::new(1, 0, 10);
    data.set_error(true);
    assert!(
        data.is_error(),
        "is_error must be true after set_error(true)"
    );
}

#[test]
fn bool_tree_node_data_is_error_false_when_clear() {
    let data = TreeNodeData::new(1, 0, 10);
    assert!(!data.is_error(), "is_error must be false by default");
}

#[test]
fn bool_tree_node_data_is_missing_true_when_set() {
    let mut data = TreeNodeData::new(1, 0, 10);
    data.set_missing(true);
    assert!(data.is_missing());
}

#[test]
fn bool_tree_node_data_is_missing_false_when_clear() {
    let data = TreeNodeData::new(1, 0, 10);
    assert!(!data.is_missing());
}

#[test]
fn bool_tree_node_data_is_extra_true_when_set() {
    let mut data = TreeNodeData::new(1, 0, 10);
    data.set_extra(true);
    assert!(data.is_extra());
}

#[test]
fn bool_tree_node_data_is_extra_false_when_clear() {
    let data = TreeNodeData::new(1, 0, 10);
    assert!(!data.is_extra());
}

#[test]
fn bool_tree_node_data_is_named_true_when_set() {
    let mut data = TreeNodeData::new(1, 0, 10);
    data.set_named(true);
    assert!(data.is_named());
}

#[test]
fn bool_tree_node_data_is_named_false_when_clear() {
    let data = TreeNodeData::new(1, 0, 10);
    assert!(!data.is_named());
}

#[test]
fn bool_tree_node_data_flags_independent() {
    let mut data = TreeNodeData::new(1, 0, 10);
    data.set_error(true);
    assert!(data.is_error());
    assert!(!data.is_named(), "setting error should not affect named");
    assert!(
        !data.is_missing(),
        "setting error should not affect missing"
    );
    assert!(!data.is_extra(), "setting error should not affect extra");
}

#[test]
fn bool_tree_node_data_flag_toggle() {
    let mut data = TreeNodeData::new(1, 0, 10);
    data.set_named(true);
    assert!(data.is_named());
    data.set_named(false);
    assert!(!data.is_named(), "toggling flag back to false must work");
}

#[test]
fn bool_tree_node_data_is_leaf_true_for_no_children() {
    let data = TreeNodeData::leaf(1, 0, 5);
    assert!(data.is_leaf());
}

#[test]
fn bool_tree_node_data_is_leaf_false_for_children() {
    let data = TreeNodeData::branch(1, 0, 5, vec![NodeHandle::new(0, 0)]);
    assert!(!data.is_leaf());
}

#[test]
fn bool_linecol_process_byte_returns_true_for_newline() {
    let mut lc = LineCol::new();
    assert!(lc.process_byte(b'\n', None, 0), "newline must return true");
}

#[test]
fn bool_linecol_process_byte_returns_false_for_regular() {
    let mut lc = LineCol::new();
    assert!(
        !lc.process_byte(b'a', None, 0),
        "regular char must return false"
    );
}

#[test]
fn bool_linecol_process_byte_crlf_cr_returns_false() {
    let mut lc = LineCol::new();
    // CR followed by LF: the CR should return false (CRLF counted on LF)
    assert!(
        !lc.process_byte(b'\r', Some(b'\n'), 0),
        "CR in CRLF must return false"
    );
}

#[test]
fn bool_linecol_process_byte_cr_alone_returns_true() {
    let mut lc = LineCol::new();
    assert!(
        lc.process_byte(b'\r', Some(b'x'), 0),
        "lone CR must return true"
    );
}

#[test]
fn bool_arena_is_empty_inversions() {
    let arena = TreeArena::new();
    assert!(arena.is_empty());
    assert_eq!(arena.is_empty(), arena.is_empty());
}

// ===================================================================
// 6. Missing break/continue in loops (iterator traversal)
// ===================================================================

#[test]
fn loop_linecol_process_byte_all_newlines() {
    let mut lc = LineCol::new();
    let input = b"a\nb\nc";
    for i in 0..input.len() {
        let next = input.get(i + 1).copied();
        lc.process_byte(input[i], next, i);
    }
    assert_eq!(lc.line, 2, "process_byte must not skip newlines");
}

#[test]
fn loop_linecol_process_consecutive_newlines() {
    let mut lc = LineCol::new();
    let input = b"\n\n\n";
    for i in 0..input.len() {
        let next = input.get(i + 1).copied();
        lc.process_byte(input[i], next, i);
    }
    assert_eq!(lc.line, 3, "3 newlines must give line 3");
}

#[test]
fn loop_arena_children_iter() {
    let mut arena = TreeArena::new();
    let c1 = arena.alloc(TreeNode::leaf(10));
    let c2 = arena.alloc(TreeNode::leaf(20));
    let c3 = arena.alloc(TreeNode::leaf(30));
    let parent = arena.alloc(TreeNode::branch(vec![c1, c2, c3]));

    let parent_ref = arena.get(parent);
    let children = parent_ref.children();
    assert_eq!(children.len(), 3, "must iterate all 3 children");
    // Verify each child has the correct value
    assert_eq!(arena.get(children[0]).value(), 10);
    assert_eq!(arena.get(children[1]).value(), 20);
    assert_eq!(arena.get(children[2]).value(), 30);
}

#[test]
fn loop_tree_node_data_children_slice_complete() {
    let mut data = TreeNodeData::new(1, 0, 10);
    for i in 0..5 {
        data.add_child(NodeHandle::new(0, i));
    }
    assert_eq!(data.children().len(), 5, "children slice must have all 5");
    for (idx, handle) in data.children().iter().enumerate() {
        assert_eq!(*handle, NodeHandle::new(0, idx as u32));
    }
}

// ===================================================================
// 7. Return value substitutions (Ok vs Err, Some vs None)
// ===================================================================

#[test]
fn return_tree_node_data_child_some_for_valid() {
    let h = NodeHandle::new(0, 0);
    let data = TreeNodeData::branch(1, 0, 5, vec![h]);
    assert!(data.child(0).is_some(), "child(0) must be Some");
}

#[test]
fn return_tree_node_data_child_none_for_invalid() {
    let h = NodeHandle::new(0, 0);
    let data = TreeNodeData::branch(1, 0, 5, vec![h]);
    assert!(
        data.child(1).is_none(),
        "child(1) must be None for single-child"
    );
}

#[test]
fn return_tree_node_data_child_none_on_leaf() {
    let data = TreeNodeData::leaf(1, 0, 5);
    assert!(data.child(0).is_none());
    assert!(data.child(usize::MAX).is_none());
}

#[test]
fn return_tree_node_data_field_id_round_trip() {
    let mut data = TreeNodeData::new(1, 0, 10);
    assert_eq!(data.field_id(), None);
    data.set_field_id(Some(42));
    assert_eq!(data.field_id(), Some(42));
    data.set_field_id(None);
    assert_eq!(data.field_id(), None);
}

#[test]
fn return_tree_node_data_field_id_some_value() {
    let mut data = TreeNodeData::new(1, 0, 10);
    data.set_field_id(Some(7));
    assert_eq!(data.field_id(), Some(7), "field_id must return Some(7)");
    assert_ne!(data.field_id(), None, "field_id must not return None");
}

#[test]
fn return_arena_get_correct_value() {
    let mut arena = TreeArena::new();
    let h = arena.alloc(TreeNode::leaf(99));
    assert_eq!(
        arena.get(h).value(),
        99,
        "get must return the allocated value"
    );
}

// ===================================================================
// 8. Wrong field access (start vs end, row vs column)
// ===================================================================

#[test]
fn field_tree_node_data_start_vs_end() {
    let data = TreeNodeData::new(1, 100, 200);
    assert_eq!(data.start_byte(), 100);
    assert_eq!(data.end_byte(), 200);
    assert_ne!(data.start_byte(), data.end_byte());
}

#[test]
fn field_tree_node_data_byte_range_order() {
    let data = TreeNodeData::new(1, 5, 15);
    let (s, e) = data.byte_range();
    assert!(s <= e, "start must be <= end in byte_range tuple");
    assert_eq!(s, data.start_byte());
    assert_eq!(e, data.end_byte());
}

#[test]
fn field_tree_node_data_byte_range_matches_accessors() {
    let data = TreeNodeData::new(1, 42, 99);
    assert_eq!(data.byte_range(), (data.start_byte(), data.end_byte()));
}

#[test]
fn field_linecol_line_vs_line_start() {
    let lc = LineCol::at_position(b"hello\nworld", 8);
    assert_eq!(lc.line, 1, "line index should be 1");
    assert_eq!(lc.line_start, 6, "line_start byte should be 6");
    assert_ne!(lc.line, lc.line_start, "line != line_start");
}

#[test]
fn field_symbol_identity() {
    let data = TreeNodeData::new(42, 0, 10);
    assert_eq!(
        data.symbol(),
        42,
        "symbol must be the value passed to new()"
    );
}

#[test]
fn field_symbol_distinct_from_start_byte() {
    let data = TreeNodeData::new(1, 42, 99);
    assert_eq!(data.symbol(), 1);
    assert_eq!(data.start_byte(), 42);
    assert_ne!(
        data.symbol() as u32,
        data.start_byte(),
        "symbol != start_byte"
    );
}

#[test]
fn field_linecol_column_uses_position_not_line() {
    let lc = LineCol {
        line: 5,
        line_start: 50,
    };
    assert_eq!(lc.column(55), 5, "column = 55 - 50, not line");
}

// ===================================================================
// Additional mutation guards (cross-cutting)
// ===================================================================

#[test]
fn arena_alloc_returns_valid_handle() {
    let mut arena = TreeArena::new();
    let h = arena.alloc(TreeNode::leaf(7));
    let node_ref = arena.get(h);
    assert_eq!(node_ref.value(), 7);
}

#[test]
fn arena_multiple_allocs_distinct_handles() {
    let mut arena = TreeArena::new();
    let h1 = arena.alloc(TreeNode::leaf(1));
    let h2 = arena.alloc(TreeNode::leaf(2));
    assert_ne!(h1, h2, "distinct allocs must give distinct handles");
    assert_eq!(arena.get(h1).value(), 1);
    assert_eq!(arena.get(h2).value(), 2);
}

#[test]
fn tree_node_data_add_child_increments_count() {
    let mut data = TreeNodeData::new(1, 0, 10);
    assert_eq!(data.child_count(), 0);
    data.add_child(NodeHandle::new(0, 0));
    assert_eq!(data.child_count(), 1);
    data.add_child(NodeHandle::new(0, 1));
    assert_eq!(data.child_count(), 2);
}

#[test]
fn tree_node_data_add_named_child_increments_named_count() {
    let mut data = TreeNodeData::new(1, 0, 10);
    assert_eq!(data.named_child_count(), 0);
    data.add_named_child(NodeHandle::new(0, 0));
    assert_eq!(data.named_child_count(), 1);
    data.add_named_child(NodeHandle::new(0, 1));
    assert_eq!(data.named_child_count(), 2);
}

#[test]
fn tree_node_data_named_count_unaffected_by_unnamed() {
    let mut data = TreeNodeData::new(1, 0, 10);
    data.add_child(NodeHandle::new(0, 0)); // unnamed
    data.add_named_child(NodeHandle::new(0, 1)); // named
    assert_eq!(data.child_count(), 2, "total includes both");
    assert_eq!(data.named_child_count(), 1, "only named children counted");
}

#[test]
fn linecol_crlf_counted_as_single_newline() {
    let input = b"ab\r\ncd";
    let lc = LineCol::at_position(input, 5);
    assert_eq!(lc.line, 1, "CRLF is one newline, not two");
}

#[test]
fn linecol_position_beyond_input_clamped() {
    let input = b"hi";
    let lc = LineCol::at_position(input, 1000);
    assert_eq!(lc.line, 0, "no newlines in 'hi'");
}

#[test]
fn arena_branch_children_preserved() {
    let mut arena = TreeArena::new();
    let c = arena.alloc(TreeNode::leaf(5));
    let p = arena.alloc(TreeNode::branch(vec![c]));
    assert!(arena.get(p).is_branch());
    assert!(arena.get(c).is_leaf());
    let p_ref = arena.get(p);
    let children = p_ref.children();
    assert_eq!(children.len(), 1);
    assert_eq!(arena.get(children[0]).value(), 5);
}

#[test]
fn arena_memory_usage_positive() {
    let mut arena = TreeArena::new();
    arena.alloc(TreeNode::leaf(1));
    assert!(
        arena.memory_usage() > 0,
        "memory_usage must be positive after alloc"
    );
}

// ===================================================================
// GLR-specific mutation guards
// ===================================================================

#[cfg(feature = "glr")]
mod glr_tests {
    use adze::glr_lexer::GLRLexer;
    use adze::glr_parser::GLRParser;
    use adze::subtree::{ChildEdge, FIELD_NONE, Subtree, SubtreeNode};
    use adze_glr_core::{FirstFollowSets, build_lr1_automaton};
    use adze_ir::{Grammar, ProductionId, Rule, Symbol, SymbolId, Token, TokenPattern};
    use std::sync::Arc;

    fn simple_grammar() -> Grammar {
        let mut g = Grammar::new("simple".into());
        let num = SymbolId(1);
        let plus = SymbolId(2);
        let expr = SymbolId(10);

        g.tokens.insert(
            num,
            Token {
                name: "number".into(),
                pattern: TokenPattern::Regex(r"\d+".into()),
                fragile: false,
            },
        );
        g.tokens.insert(
            plus,
            Token {
                name: "plus".into(),
                pattern: TokenPattern::String("+".into()),
                fragile: false,
            },
        );

        g.rules.entry(expr).or_default().push(Rule {
            lhs: expr,
            rhs: vec![Symbol::Terminal(num)],
            precedence: None,
            associativity: None,
            production_id: ProductionId(0),
            fields: vec![],
        });
        g.rules.entry(expr).or_default().push(Rule {
            lhs: expr,
            rhs: vec![
                Symbol::NonTerminal(expr),
                Symbol::Terminal(plus),
                Symbol::NonTerminal(expr),
            ],
            precedence: None,
            associativity: None,
            production_id: ProductionId(1),
            fields: vec![],
        });

        g.rule_names.insert(expr, "expression".into());
        g.set_start_symbol(expr);
        g
    }

    fn build_parser(grammar: &Grammar) -> GLRParser {
        let ff = FirstFollowSets::compute(grammar).expect("FIRST/FOLLOW");
        let table = build_lr1_automaton(grammar, &ff).expect("LR(1) automaton");
        GLRParser::new(table, grammar.clone())
    }

    fn parse_input(
        parser: &mut GLRParser,
        grammar: &Grammar,
        input: &str,
    ) -> Result<Arc<Subtree>, String> {
        parser.reset();
        let mut lexer = GLRLexer::new(grammar, input.to_string()).map_err(|e| e.to_string())?;
        let tokens = lexer.tokenize_all();
        for t in &tokens {
            parser.process_token(t.symbol_id, &t.text, t.byte_offset);
        }
        parser.process_eof(input.len());
        parser.finish()
    }

    #[test]
    fn glr_subtree_byte_range_start_not_end() {
        let node = SubtreeNode {
            symbol_id: SymbolId(1),
            is_error: false,
            byte_range: 5..15,
        };
        let st = Subtree::new(node, vec![]);
        assert_eq!(st.byte_range().start, 5);
        assert_eq!(st.byte_range().end, 15);
        assert_ne!(st.byte_range().start, st.byte_range().end);
    }

    #[test]
    fn glr_subtree_is_error_reflects_node() {
        let err_node = SubtreeNode {
            symbol_id: SymbolId(1),
            is_error: true,
            byte_range: 0..5,
        };
        let ok_node = SubtreeNode {
            symbol_id: SymbolId(1),
            is_error: false,
            byte_range: 0..5,
        };
        assert!(Subtree::new(err_node, vec![]).is_error());
        assert!(!Subtree::new(ok_node, vec![]).is_error());
    }

    #[test]
    fn glr_subtree_symbol_correct() {
        let node = SubtreeNode {
            symbol_id: SymbolId(42),
            is_error: false,
            byte_range: 0..10,
        };
        let st = Subtree::new(node, vec![]);
        assert_eq!(st.symbol(), 42);
    }

    #[test]
    fn glr_subtree_no_alts_by_default() {
        let node = SubtreeNode {
            symbol_id: SymbolId(1),
            is_error: false,
            byte_range: 0..5,
        };
        let st = Subtree::new(node, vec![]);
        assert!(!st.is_ambiguous());
        assert!(!st.has_alts());
        assert_eq!(st.alternatives_iter().count(), 0);
    }

    #[test]
    fn glr_subtree_push_alt_makes_ambiguous() {
        let node = SubtreeNode {
            symbol_id: SymbolId(1),
            is_error: false,
            byte_range: 0..5,
        };
        let st = Subtree::new(node.clone(), vec![]);
        let alt = Arc::new(Subtree::new(node, vec![]));
        let st = st.push_alt(alt);
        assert!(st.is_ambiguous(), "push_alt must make it ambiguous");
        assert!(st.has_alts());
    }

    #[test]
    fn glr_field_none_constant() {
        assert_eq!(FIELD_NONE, u16::MAX, "FIELD_NONE must be u16::MAX");
    }

    #[test]
    fn glr_child_edge_field_id() {
        let node = SubtreeNode {
            symbol_id: SymbolId(1),
            is_error: false,
            byte_range: 0..5,
        };
        let child = Arc::new(Subtree::new(node, vec![]));
        let edge_with = ChildEdge::new(child.clone(), 7);
        let edge_without = ChildEdge::new_without_field(child);
        assert_eq!(edge_with.field_id, 7);
        assert_eq!(edge_without.field_id, FIELD_NONE);
    }

    #[test]
    fn glr_parse_valid_input_succeeds() {
        let g = simple_grammar();
        let mut parser = build_parser(&g);
        let result = parse_input(&mut parser, &g, "1+2");
        assert!(result.is_ok(), "valid input must parse: {:?}", result);
    }

    #[test]
    fn glr_parse_empty_input_fails() {
        let g = simple_grammar();
        let mut parser = build_parser(&g);
        let result = parse_input(&mut parser, &g, "");
        assert!(result.is_err(), "empty input should fail");
    }

    #[test]
    fn glr_lexer_token_offsets_monotonic() {
        let g = simple_grammar();
        let mut lexer = GLRLexer::new(&g, "1+2+3".into()).unwrap();
        let tokens = lexer.tokenize_all();
        assert!(tokens.len() >= 3, "should have multiple tokens");
        for pair in tokens.windows(2) {
            assert!(
                pair[1].byte_offset >= pair[0].byte_offset,
                "token offsets must be non-decreasing"
            );
        }
    }

    #[test]
    fn glr_parser_reset_allows_reuse() {
        let g = simple_grammar();
        let mut parser = build_parser(&g);
        let r1 = parse_input(&mut parser, &g, "1");
        assert!(r1.is_ok());
        let r2 = parse_input(&mut parser, &g, "2+3");
        assert!(r2.is_ok());
    }
}
