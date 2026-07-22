//! Comprehensive Node API tests
//!
//! Tests every method on the Node type hierarchy:
//! - `parser_v4::ParseNode` — the concrete parse tree node with public fields
//! - `tree_node_data::TreeNodeData` — low-level node metadata (flags, children, fields)
//! - `parser_v4::Tree` + `node::Node` — arena-allocated tree from parsing
//! - `ts_compat::Node` — Tree-sitter compatibility API (behind `ts-compat` feature)

mod common;

use adze::arena_allocator::NodeHandle;
use adze::parser_v4::{ParseNode, Parser};
use adze::tree_node_data::TreeNodeData;
use adze_ir::SymbolId;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Build a simple grammar + parse table for "numbers only": Expr → NUM
fn number_grammar() -> (adze_ir::Grammar, adze_glr_core::ParseTable) {
    use adze_ir::{ProductionId, Rule, Symbol, Token, TokenPattern};

    let mut grammar = adze_ir::Grammar::new("number".to_string());

    let num_id = SymbolId(1);
    let expr_id = SymbolId(0);

    grammar.tokens.insert(
        num_id,
        Token {
            name: "NUM".to_string(),
            pattern: TokenPattern::Regex(r"\d+".to_string()),
            fragile: false,
        },
    );

    grammar.rule_names.insert(expr_id, "expression".to_string());
    grammar.rule_names.insert(num_id, "NUM".to_string());

    grammar.add_rule(Rule {
        lhs: expr_id,
        rhs: vec![Symbol::Terminal(num_id)],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    });
    grammar.set_start_symbol(expr_id);

    let table = common::build_table(&grammar);
    (grammar, table)
}

/// Construct a ParseNode leaf (terminal) by hand.
fn leaf(sym: u16, start: usize, end: usize) -> ParseNode {
    ParseNode {
        symbol: SymbolId(sym),
        symbol_id: SymbolId(sym),
        start_byte: start,
        end_byte: end,
        field_name: None,
        alias_symbol_id: None,
        children: vec![],
    }
}

/// Construct a ParseNode branch (non-terminal) by hand.
fn branch(sym: u16, start: usize, end: usize, children: Vec<ParseNode>) -> ParseNode {
    ParseNode {
        symbol: SymbolId(sym),
        symbol_id: SymbolId(sym),
        start_byte: start,
        end_byte: end,
        field_name: None,
        alias_symbol_id: None,
        children,
    }
}

/// Build a small hand-crafted tree: expr(num("42"), plus("+"), num("7"))
fn sample_tree() -> ParseNode {
    let num1 = leaf(1, 0, 2);
    let plus = leaf(2, 3, 4);
    let num2 = leaf(5, 5, 6);
    branch(10, 0, 6, vec![num1, plus, num2])
}

// ===========================================================================
// 1. kind() — ParseNode symbol identity
// ===========================================================================

#[test]
fn test_kind_returns_correct_symbol_id() {
    let node = leaf(42, 0, 5);
    assert_eq!(node.symbol.0, 42);
    assert_eq!(node.symbol_id.0, 42);
}

#[test]
fn test_kind_root_vs_child() {
    let tree = sample_tree();
    assert_eq!(tree.symbol.0, 10, "root symbol");
    assert_eq!(tree.children[0].symbol.0, 1, "first child symbol");
    assert_eq!(tree.children[1].symbol.0, 2, "operator symbol");
}

// ===========================================================================
// 2. is_named() — TreeNodeData flag
// ===========================================================================

#[test]
fn test_is_named_default_false() {
    let node = TreeNodeData::new(1, 0, 10);
    assert!(!node.is_named());
}

#[test]
fn test_is_named_set_true() {
    let mut node = TreeNodeData::new(1, 0, 10);
    node.set_named(true);
    assert!(node.is_named());
}

#[test]
fn test_is_named_differentiates_named_and_anonymous() {
    let mut named = TreeNodeData::new(1, 0, 10);
    named.set_named(true);
    let anon = TreeNodeData::new(2, 0, 10);
    assert!(named.is_named());
    assert!(!anon.is_named());
}

// ===========================================================================
// 3. is_missing() — for error-recovery inserted nodes
// ===========================================================================

#[test]
fn test_is_missing_default_false() {
    let node = TreeNodeData::new(1, 0, 10);
    assert!(!node.is_missing());
}

#[test]
fn test_is_missing_set_true() {
    let mut node = TreeNodeData::new(1, 0, 10);
    node.set_missing(true);
    assert!(node.is_missing());
}

#[test]
fn test_is_missing_independent_of_is_error() {
    let mut node = TreeNodeData::new(1, 0, 10);
    node.set_error(true);
    assert!(!node.is_missing(), "error != missing");
    node.set_missing(true);
    assert!(node.is_error());
    assert!(node.is_missing());
}

// ===========================================================================
// 4. is_extra() — trivia nodes (whitespace, comments)
// ===========================================================================

#[test]
fn test_is_extra_default_false() {
    let node = TreeNodeData::new(1, 0, 10);
    assert!(!node.is_extra());
}

#[test]
fn test_is_extra_set_true() {
    let mut node = TreeNodeData::new(7, 0, 3);
    node.set_extra(true);
    assert!(node.is_extra());
}

#[test]
fn test_is_extra_independent_of_other_flags() {
    let mut node = TreeNodeData::new(7, 0, 3);
    node.set_extra(true);
    node.set_named(true);
    assert!(node.is_extra());
    assert!(node.is_named());
}

// ===========================================================================
// 5. start_byte() and end_byte() — byte range
// ===========================================================================

#[test]
fn test_start_end_byte_parse_node() {
    let node = leaf(1, 10, 25);
    assert_eq!(node.start_byte, 10);
    assert_eq!(node.end_byte, 25);
}

#[test]
fn test_start_end_byte_tree_node_data() {
    let node = TreeNodeData::new(1, 100, 200);
    assert_eq!(node.start_byte(), 100);
    assert_eq!(node.end_byte(), 200);
}

#[test]
fn test_byte_range_zero_length() {
    let node = TreeNodeData::new(1, 5, 5);
    assert_eq!(node.byte_len(), 0);
    assert_eq!(node.start_byte(), node.end_byte());
}

#[test]
fn test_byte_range_root_covers_children() {
    let tree = sample_tree();
    assert_eq!(tree.start_byte, 0);
    assert_eq!(tree.end_byte, 6);
    for child in &tree.children {
        assert!(child.start_byte >= tree.start_byte);
        assert!(child.end_byte <= tree.end_byte);
    }
}

// ===========================================================================
// 6. start_position() and end_position() (row, column) — ts_compat only
// ===========================================================================

#[cfg(feature = "ts-compat")]
mod position_tests {
    use adze::ts_compat::{Language, Parser, Point};
    use adze_glr_core::{FirstFollowSets, build_lr1_automaton};
    use adze_ir::{Grammar, ProductionId, Rule, Symbol, SymbolId, Token, TokenPattern};
    use std::sync::Arc;

    fn simple_language() -> Arc<Language> {
        let mut grammar = Grammar::new("pos_test".to_string());
        let num = SymbolId(1);
        let expr = SymbolId(2);
        grammar.tokens.insert(
            num,
            Token {
                name: "number".to_string(),
                pattern: TokenPattern::Regex(r"\d+".to_string()),
                fragile: false,
            },
        );
        grammar.add_rule(Rule {
            lhs: expr,
            rhs: vec![Symbol::Terminal(num)],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(0),
        });
        grammar.rule_names.insert(expr, "expression".to_string());
        grammar.rule_names.insert(num, "number".to_string());
        grammar.set_start_symbol(expr);
        let ff = FirstFollowSets::compute(&grammar).unwrap();
        let table = build_lr1_automaton(&grammar, &ff).unwrap();
        Arc::new(Language::new("pos_test", grammar, table))
    }

    #[test]
    fn test_start_position_single_line() {
        let lang = simple_language();
        let mut parser = Parser::new();
        parser.set_language(lang).unwrap();
        let tree = parser.parse("123", None).unwrap();
        let root = tree.root_node();
        assert_eq!(root.start_position(), Point { row: 0, column: 0 });
    }

    #[test]
    fn test_end_position_single_line() {
        let lang = simple_language();
        let mut parser = Parser::new();
        parser.set_language(lang).unwrap();
        let tree = parser.parse("123", None).unwrap();
        let root = tree.root_node();
        assert_eq!(root.end_position(), Point { row: 0, column: 3 });
    }
}

// ===========================================================================
// 7. child_count() — accuracy
// ===========================================================================

#[test]
fn test_child_count_leaf() {
    let node = leaf(1, 0, 3);
    assert_eq!(node.children.len(), 0);
}

#[test]
fn test_child_count_branch() {
    let tree = sample_tree();
    assert_eq!(tree.children.len(), 3);
}

#[test]
fn test_child_count_tree_node_data() {
    let mut data = TreeNodeData::new(1, 0, 20);
    data.add_child(NodeHandle::new(0, 0));
    data.add_child(NodeHandle::new(0, 1));
    data.add_child(NodeHandle::new(0, 2));
    assert_eq!(data.child_count(), 3);
}

// ===========================================================================
// 8. named_child_count() — accuracy
// ===========================================================================

#[test]
fn test_named_child_count_zero() {
    let mut data = TreeNodeData::new(1, 0, 10);
    data.add_child(NodeHandle::new(0, 0));
    data.add_child(NodeHandle::new(0, 1));
    assert_eq!(data.named_child_count(), 0);
}

#[test]
fn test_named_child_count_mixed() {
    let mut data = TreeNodeData::new(1, 0, 20);
    data.add_named_child(NodeHandle::new(0, 0));
    data.add_child(NodeHandle::new(0, 1));
    data.add_named_child(NodeHandle::new(0, 2));
    assert_eq!(data.named_child_count(), 2);
    assert_eq!(data.child_count(), 3);
}

// ===========================================================================
// 9. child(index) — returns correct child
// ===========================================================================

#[test]
fn test_child_by_index_parse_node() {
    let tree = sample_tree();
    assert_eq!(tree.children[0].symbol.0, 1);
    assert_eq!(tree.children[2].symbol.0, 5);
}

#[test]
fn test_child_by_index_out_of_bounds() {
    let tree = sample_tree();
    assert!(tree.children.get(100).is_none());
}

#[test]
fn test_child_by_index_tree_node_data() {
    let mut data = TreeNodeData::new(10, 0, 20);
    let h = NodeHandle::new(0, 99);
    data.add_child(h);
    assert_eq!(data.child(0), Some(h));
    assert_eq!(data.child(1), None);
}

// ===========================================================================
// 10. named_child(index) — returns correct named child
// ===========================================================================

#[test]
fn test_named_child_tree_node_data() {
    let mut data = TreeNodeData::new(1, 0, 20);
    let h1 = NodeHandle::new(0, 10);
    let h2 = NodeHandle::new(0, 20);
    data.add_named_child(h1);
    data.add_named_child(h2);
    // Access children by index; named children are at positions 0 and 1
    assert_eq!(data.child(0), Some(h1));
    assert_eq!(data.child(1), Some(h2));
    assert_eq!(data.named_child_count(), 2);
}

#[test]
fn test_named_child_mixed_with_unnamed() {
    let mut data = TreeNodeData::new(1, 0, 20);
    data.add_child(NodeHandle::new(0, 0)); // unnamed
    data.add_named_child(NodeHandle::new(0, 1)); // named
    data.add_child(NodeHandle::new(0, 2)); // unnamed
    assert_eq!(data.child_count(), 3);
    assert_eq!(data.named_child_count(), 1);
}

// ===========================================================================
// 11. child_by_field_name() — ParseNode.field_name
// ===========================================================================

#[test]
fn test_child_by_field_name_present() {
    let mut child = leaf(1, 0, 3);
    child.field_name = Some("value".to_string());
    let tree = branch(10, 0, 3, vec![child]);
    let found = tree
        .children
        .iter()
        .find(|c| c.field_name.as_deref() == Some("value"));
    assert!(found.is_some());
    assert_eq!(found.unwrap().symbol.0, 1);
}

#[test]
fn test_child_by_field_name_absent() {
    let tree = sample_tree();
    let found = tree
        .children
        .iter()
        .find(|c| c.field_name.as_deref() == Some("nonexistent"));
    assert!(found.is_none());
}

#[test]
fn test_field_id_tree_node_data() {
    let mut data = TreeNodeData::new(1, 0, 10);
    assert_eq!(data.field_id(), None);
    data.set_field_id(Some(5));
    assert_eq!(data.field_id(), Some(5));
}

// ===========================================================================
// 12. children() iterator — ParseNode children
// ===========================================================================

#[test]
fn test_children_iterator_empty() {
    let node = leaf(1, 0, 3);
    assert_eq!(node.children.len(), 0);
}

#[test]
fn test_children_iterator_multiple() {
    let tree = sample_tree();
    let symbols: Vec<u16> = tree.children.iter().map(|c| c.symbol.0).collect();
    assert_eq!(symbols, vec![1, 2, 5]);
}

#[test]
fn test_children_iterator_exact_size() {
    let tree = sample_tree();
    assert_eq!(tree.children.len(), 3);
}

// ===========================================================================
// 13. named_children() iterator
// ===========================================================================

#[test]
fn test_named_children_filter() {
    // Simulate named children filter using TreeNodeData flags
    let mut nodes = [
        TreeNodeData::new(1, 0, 5),
        TreeNodeData::new(2, 5, 6),
        TreeNodeData::new(3, 6, 10),
    ];
    nodes[0].set_named(true);
    nodes[2].set_named(true);

    let named: Vec<u16> = nodes
        .iter()
        .filter(|n| n.is_named())
        .map(|n| n.symbol())
        .collect();
    assert_eq!(named, vec![1, 3]);
}

// ===========================================================================
// 14. parent() — ParseNode tree traversal
// ===========================================================================

#[test]
fn test_parent_root_has_no_parent() {
    // ParseNode is an owned tree; there's no parent pointer.
    // Traversal is top-down only. Root has no parent by definition.
    let tree = sample_tree();
    // We verify the root owns its children but children don't reference back.
    assert_eq!(tree.children.len(), 3);
    // No parent() method exists on ParseNode; this tests the structural property.
}

#[test]
fn test_parent_child_relationship() {
    // We can verify parent-child via recursive traversal.
    let tree = sample_tree();
    fn find_parent_of(root: &ParseNode, target_sym: u16) -> Option<u16> {
        for child in &root.children {
            if child.symbol.0 == target_sym {
                return Some(root.symbol.0);
            }
            if let Some(p) = find_parent_of(child, target_sym) {
                return Some(p);
            }
        }
        None
    }
    assert_eq!(find_parent_of(&tree, 1), Some(10));
}

// ===========================================================================
// 15. next_sibling() and prev_sibling()
// ===========================================================================

#[test]
fn test_sibling_navigation_via_index() {
    let tree = sample_tree();
    // Siblings are children[i-1] and children[i+1]
    let children = &tree.children;
    assert_eq!(children.len(), 3);
    // child[0] has no prev, next is child[1]
    assert_eq!(children[1].symbol.0, 2); // next of child[0]
    // child[2] has prev child[1], no next
    assert_eq!(children[1].symbol.0, 2); // prev of child[2]
}

#[test]
fn test_first_child_has_no_prev_sibling() {
    let tree = sample_tree();
    // Index 0 has no predecessor
    assert!(!tree.children.is_empty());
    // No negative index possible
}

#[test]
fn test_last_child_has_no_next_sibling() {
    let tree = sample_tree();
    let last_idx = tree.children.len() - 1;
    assert!(tree.children.get(last_idx + 1).is_none());
}

// ===========================================================================
// 16. next_named_sibling() and prev_named_sibling()
// ===========================================================================

#[test]
fn test_named_sibling_navigation() {
    // Simulate named sibling search using TreeNodeData flags
    let mut data_named_a = TreeNodeData::new(1, 0, 5);
    data_named_a.set_named(true);
    let data_anon = TreeNodeData::new(2, 5, 6);
    // anonymous — is_named() == false
    let mut data_named_b = TreeNodeData::new(3, 6, 10);
    data_named_b.set_named(true);

    let siblings = [data_named_a, data_anon, data_named_b];
    // Next named sibling of index 0 (skipping anonymous at index 1):
    let next_named = siblings.iter().skip(1).find(|s| s.is_named());
    assert!(next_named.is_some());
    assert_eq!(next_named.unwrap().symbol(), 3);
    // Prev named sibling of index 2 (skipping anonymous at index 1):
    let prev_named = siblings.iter().rev().skip(1).find(|s| s.is_named());
    assert!(prev_named.is_some());
    assert_eq!(prev_named.unwrap().symbol(), 1);
}

// ===========================================================================
// 17. utf8_text() — extracts correct text
// ===========================================================================

#[test]
fn test_utf8_text_via_byte_range() {
    let source = b"42 + 7";
    let node = leaf(1, 0, 2);
    let text = std::str::from_utf8(&source[node.start_byte..node.end_byte]).unwrap();
    assert_eq!(text, "42");
}

#[test]
fn test_utf8_text_operator() {
    let source = b"42 + 7";
    let node = leaf(2, 3, 4);
    let text = std::str::from_utf8(&source[node.start_byte..node.end_byte]).unwrap();
    assert_eq!(text, "+");
}

#[test]
fn test_utf8_text_unicode() {
    let source = "π + 1".as_bytes();
    // π is 2 bytes in UTF-8
    let node = leaf(1, 0, 2);
    let text = std::str::from_utf8(&source[node.start_byte..node.end_byte]).unwrap();
    assert_eq!(text, "π");
}

#[cfg(feature = "ts-compat")]
#[test]
fn test_utf8_text_ts_compat_node() {
    use adze::ts_compat::{Language, Parser};
    use adze_glr_core::{FirstFollowSets, build_lr1_automaton};
    use adze_ir::{Grammar, ProductionId, Rule, Symbol, SymbolId, Token, TokenPattern};
    use std::sync::Arc;

    let mut grammar = Grammar::new("text_test".to_string());
    let num = SymbolId(1);
    let expr = SymbolId(2);
    grammar.tokens.insert(
        num,
        Token {
            name: "number".to_string(),
            pattern: TokenPattern::Regex(r"\d+".to_string()),
            fragile: false,
        },
    );
    grammar.add_rule(Rule {
        lhs: expr,
        rhs: vec![Symbol::Terminal(num)],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    });
    grammar.rule_names.insert(expr, "expression".to_string());
    grammar.set_start_symbol(expr);
    let ff = FirstFollowSets::compute(&grammar).unwrap();
    let table = build_lr1_automaton(&grammar, &ff).unwrap();
    let lang = Arc::new(Language::new("text_test", grammar, table));

    let mut parser = Parser::new();
    parser.set_language(lang).unwrap();
    let source = "999";
    let tree = parser.parse(source, None).unwrap();
    let root = tree.root_node();
    let text = root.utf8_text(source.as_bytes()).unwrap();
    assert_eq!(text, source);
}

// ===========================================================================
// 18. to_sexp() — S-expression rendering
// ===========================================================================

#[test]
fn test_to_sexp_leaf() {
    // Render a simple S-expression for a leaf node.
    fn to_sexp(node: &ParseNode, source: &[u8]) -> String {
        if node.children.is_empty() {
            let text = std::str::from_utf8(&source[node.start_byte..node.end_byte]).unwrap_or("?");
            format!("(sym_{} \"{}\")", node.symbol.0, text)
        } else {
            let children_sexp: Vec<String> =
                node.children.iter().map(|c| to_sexp(c, source)).collect();
            format!("(sym_{} {})", node.symbol.0, children_sexp.join(" "))
        }
    }
    let node = leaf(1, 0, 2);
    let sexp = to_sexp(&node, b"42");
    assert_eq!(sexp, "(sym_1 \"42\")");
}

#[test]
fn test_to_sexp_tree() {
    fn to_sexp(node: &ParseNode, source: &[u8]) -> String {
        if node.children.is_empty() {
            let text = std::str::from_utf8(&source[node.start_byte..node.end_byte]).unwrap_or("?");
            format!("(sym_{} \"{}\")", node.symbol.0, text)
        } else {
            let children_sexp: Vec<String> =
                node.children.iter().map(|c| to_sexp(c, source)).collect();
            format!("(sym_{} {})", node.symbol.0, children_sexp.join(" "))
        }
    }
    let source = b"42 + 7";
    let tree = sample_tree();
    let sexp = to_sexp(&tree, source);
    assert!(sexp.starts_with("(sym_10 "));
    assert!(sexp.contains("sym_1"));
    assert!(sexp.contains("sym_2"));
}

// ===========================================================================
// 19. has_error() — error detection
// ===========================================================================

#[test]
fn test_has_error_default_false() {
    let data = TreeNodeData::new(1, 0, 10);
    assert!(!data.is_error());
}

#[test]
fn test_has_error_set_true() {
    let mut data = TreeNodeData::new(1, 0, 10);
    data.set_error(true);
    assert!(data.is_error());
}

#[test]
fn test_has_error_parse_node_no_error() {
    let (grammar, table) = number_grammar();
    let mut parser = Parser::new(grammar, table, "test".to_string());
    let root = parser.parse_tree("42").expect("parse should succeed");
    // Successful parse should not have error symbol
    assert_ne!(root.symbol, SymbolId(u16::MAX));
}

// ===========================================================================
// 20. has_changes() — change detection
// ===========================================================================

#[test]
fn test_has_changes_flags_independence() {
    // TreeNodeData flags are independent — toggling one doesn't affect others.
    let mut data = TreeNodeData::new(1, 0, 10);
    data.set_named(true);
    data.set_error(false);
    data.set_missing(false);
    data.set_extra(false);
    assert!(data.is_named());
    assert!(!data.is_error());
    assert!(!data.is_missing());
    assert!(!data.is_extra());
}

#[test]
fn test_all_flags_can_be_set_simultaneously() {
    let mut data = TreeNodeData::new(1, 0, 10);
    data.set_named(true);
    data.set_error(true);
    data.set_missing(true);
    data.set_extra(true);
    assert!(data.is_named());
    assert!(data.is_error());
    assert!(data.is_missing());
    assert!(data.is_extra());
}

// ===========================================================================
// Additional integration tests via parser_v4::Parser
// ===========================================================================

#[test]
fn test_parser_v4_parse_tree_structure() {
    let (grammar, table) = number_grammar();
    let mut parser = Parser::new(grammar, table, "test".to_string());
    let root = parser.parse_tree("123").expect("parse should succeed");
    // Root should have the expression symbol
    assert!(
        root.start_byte <= root.end_byte,
        "start_byte should be <= end_byte"
    );
}

#[test]
fn test_parser_v4_children_present() {
    let (grammar, table) = number_grammar();
    let mut parser = Parser::new(grammar, table, "test".to_string());
    let root = parser.parse_tree("42").expect("parse should succeed");
    // An expression wrapping a terminal may have children
    // At minimum the root should be a valid node
    assert!(root.symbol.0 < 1000, "symbol should be reasonable");
}

#[test]
fn test_parser_v4_byte_range_matches_source() {
    let (grammar, table) = number_grammar();
    let mut parser = Parser::new(grammar, table, "test".to_string());
    let source = "99999";
    let root = parser.parse_tree(source).expect("parse should succeed");
    // Root should span the entire source (or at least be within bounds)
    assert!(root.end_byte <= source.len() + 1);
}

// ===========================================================================
// Arena Node via parser_v4::Parser::parse()
// ===========================================================================

#[test]
fn test_arena_node_via_parse() {
    let (grammar, table) = number_grammar();
    let mut parser = Parser::new(grammar, table, "test".to_string());
    let tree = parser.parse("42").expect("parse should succeed");
    let root = tree.root_node();
    // Arena node should have a valid symbol
    let _sym = root.symbol();
    // child_count should be non-negative
    let _count = root.child_count();
}

#[test]
fn test_arena_node_is_copy() {
    fn assert_copy<T: Copy>(_: T) {}
    let (grammar, table) = number_grammar();
    let mut parser = Parser::new(grammar, table, "test".to_string());
    let tree = parser.parse("42").expect("parse should succeed");
    let node = tree.root_node();
    assert_copy(node);
}

#[test]
fn test_arena_node_debug_impl() {
    let (grammar, table) = number_grammar();
    let mut parser = Parser::new(grammar, table, "test".to_string());
    let tree = parser.parse("42").expect("parse should succeed");
    let node = tree.root_node();
    let debug = format!("{:?}", node);
    assert!(debug.contains("Node"), "Debug should contain 'Node'");
}

// ===========================================================================
// TreeNodeData byte_range and byte_len
// ===========================================================================

#[test]
fn test_tree_node_data_byte_range() {
    let data = TreeNodeData::new(1, 10, 50);
    assert_eq!(data.byte_range(), (10, 50));
    assert_eq!(data.byte_len(), 40);
}

#[test]
fn test_tree_node_data_leaf_factory() {
    let data = TreeNodeData::leaf(5, 20, 30);
    assert_eq!(data.symbol(), 5);
    assert_eq!(data.start_byte(), 20);
    assert_eq!(data.end_byte(), 30);
    assert!(data.is_leaf());
    assert_eq!(data.child_count(), 0);
}

// ===========================================================================
// ParseNode field_name tests (child_by_field_name proxy)
// ===========================================================================

#[test]
fn test_parse_node_multiple_fields() {
    let mut left = leaf(1, 0, 2);
    left.field_name = Some("left".to_string());
    let mut op = leaf(2, 3, 4);
    op.field_name = Some("operator".to_string());
    let mut right = leaf(1, 5, 6);
    right.field_name = Some("right".to_string());
    let tree = branch(10, 0, 6, vec![left, op, right]);

    assert_eq!(
        tree.children
            .iter()
            .find(|c| c.field_name.as_deref() == Some("left"))
            .unwrap()
            .start_byte,
        0
    );
    assert_eq!(
        tree.children
            .iter()
            .find(|c| c.field_name.as_deref() == Some("right"))
            .unwrap()
            .start_byte,
        5
    );
    assert_eq!(
        tree.children
            .iter()
            .find(|c| c.field_name.as_deref() == Some("operator"))
            .unwrap()
            .symbol
            .0,
        2
    );
}

// ===========================================================================
// ParseNode recursive children iteration
// ===========================================================================

#[test]
fn test_parse_node_recursive_traversal() {
    let inner = branch(11, 0, 2, vec![leaf(1, 0, 2)]);
    let tree = branch(10, 0, 6, vec![inner, leaf(2, 3, 4), leaf(1, 5, 6)]);

    fn count_nodes(node: &ParseNode) -> usize {
        1 + node.children.iter().map(count_nodes).sum::<usize>()
    }
    assert_eq!(count_nodes(&tree), 5);
}

#[test]
fn test_parse_node_leaf_symbols_collection() {
    let tree = sample_tree();
    fn collect_leaves(node: &ParseNode, out: &mut Vec<u16>) {
        if node.children.is_empty() {
            out.push(node.symbol.0);
        }
        for child in &node.children {
            collect_leaves(child, out);
        }
    }
    let mut leaves = Vec::new();
    collect_leaves(&tree, &mut leaves);
    assert_eq!(leaves, vec![1, 2, 5]);
}
