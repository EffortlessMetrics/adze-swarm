// Common test utilities to reduce boilerplate
use adze_glr_core::{FirstFollowSets, ParseTable, build_lr1_automaton};
use adze_ir::Grammar;

/// Build a parse table from a grammar - centralizes the construction logic
#[allow(dead_code)]
pub fn build_table(grammar: &Grammar) -> ParseTable {
    let ff = FirstFollowSets::compute(grammar).unwrap();
    build_lr1_automaton(grammar, &ff).expect("Failed to build automaton")
}

/// Build parse table and wrap in Result for tests that need error handling
#[allow(dead_code)]
pub fn build_table_result(grammar: &Grammar) -> anyhow::Result<ParseTable> {
    let ff = FirstFollowSets::compute(grammar).unwrap();
    Ok(build_lr1_automaton(grammar, &ff)?)
}

// ParsedNode fixtures used by visitor and ParsedNode integration tests.
//
// `ParsedNode::language` is intentionally crate-private, so external tests need
// one carefully-contained constructor that initializes every field.
#[allow(dead_code)]
pub fn parsed_point(row: u32, col: u32) -> adze::pure_parser::Point {
    adze::pure_parser::Point { row, column: col }
}

#[allow(dead_code)]
#[expect(
    clippy::too_many_arguments,
    reason = "test fixture constructor mirrors ParsedNode fields"
)]
pub fn make_parsed_node(
    symbol: u16,
    children: Vec<adze::pure_parser::ParsedNode>,
    start: usize,
    end: usize,
    start_pt: adze::pure_parser::Point,
    end_pt: adze::pure_parser::Point,
    is_extra: bool,
    is_error: bool,
    is_missing: bool,
    is_named: bool,
    field_id: Option<u16>,
) -> adze::pure_parser::ParsedNode {
    use adze::pure_parser::ParsedNode;
    use std::mem::MaybeUninit;

    let mut uninit = MaybeUninit::<ParsedNode>::uninit();
    let ptr = uninit.as_mut_ptr();
    unsafe {
        std::ptr::write_bytes(ptr, 0, 1);
        std::ptr::addr_of_mut!((*ptr).symbol).write(symbol);
        std::ptr::addr_of_mut!((*ptr).children).write(children);
        std::ptr::addr_of_mut!((*ptr).start_byte).write(start);
        std::ptr::addr_of_mut!((*ptr).end_byte).write(end);
        std::ptr::addr_of_mut!((*ptr).start_point).write(start_pt);
        std::ptr::addr_of_mut!((*ptr).end_point).write(end_pt);
        std::ptr::addr_of_mut!((*ptr).is_extra).write(is_extra);
        std::ptr::addr_of_mut!((*ptr).is_error).write(is_error);
        std::ptr::addr_of_mut!((*ptr).is_missing).write(is_missing);
        std::ptr::addr_of_mut!((*ptr).is_named).write(is_named);
        std::ptr::addr_of_mut!((*ptr).field_id).write(field_id);
        uninit.assume_init()
    }
}

#[allow(dead_code)]
pub fn make_test_node(
    symbol: u16,
    children: Vec<adze::pure_parser::ParsedNode>,
    start: usize,
    end: usize,
    is_error: bool,
    is_named: bool,
) -> adze::pure_parser::ParsedNode {
    make_parsed_node(
        symbol,
        children,
        start,
        end,
        parsed_point(0, start as u32),
        parsed_point(0, end as u32),
        false,
        is_error,
        false,
        is_named,
        None,
    )
}

#[allow(dead_code)]
pub fn named_leaf(symbol: u16, start: usize, end: usize) -> adze::pure_parser::ParsedNode {
    make_test_node(symbol, vec![], start, end, false, true)
}

#[allow(dead_code)]
pub fn unnamed_leaf(symbol: u16, start: usize, end: usize) -> adze::pure_parser::ParsedNode {
    make_test_node(symbol, vec![], start, end, false, false)
}

#[allow(dead_code)]
pub fn interior_node(
    symbol: u16,
    children: Vec<adze::pure_parser::ParsedNode>,
) -> adze::pure_parser::ParsedNode {
    let start = children.first().map_or(0, |c| c.start_byte);
    let end = children.last().map_or(0, |c| c.end_byte);
    make_test_node(symbol, children, start, end, false, true)
}

#[allow(dead_code)]
pub fn error_node(start: usize, end: usize) -> adze::pure_parser::ParsedNode {
    make_test_node(0, vec![], start, end, true, false)
}

#[allow(dead_code)]
pub fn count_nodes(node: &adze::pure_parser::ParsedNode) -> usize {
    1 + node.children().iter().map(count_nodes).sum::<usize>()
}

#[allow(dead_code)]
pub fn tree_depth(node: &adze::pure_parser::ParsedNode) -> usize {
    if node.children().is_empty() {
        1
    } else {
        1 + node.children().iter().map(tree_depth).max().unwrap_or(0)
    }
}

#[allow(dead_code)]
pub fn count_leaves(node: &adze::pure_parser::ParsedNode) -> usize {
    if node.children().is_empty() {
        1
    } else {
        node.children().iter().map(count_leaves).sum()
    }
}
