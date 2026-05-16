//! Node representation for syntax tree nodes
//!
//! Contract: docs/specs/NODE_API_CONTRACT.md

use crate::{Language, tree::TreeNode};
use std::fmt;

/// A node in the syntax tree
///
/// Provides read-only access to tree node data with lifetime tied to parent Tree.
///
/// # Contract
///
/// - Nodes are Copy and read-only (no mutation)
/// - Lifetime `'tree` tied to parent Tree
/// - Child access is always safe (returns None if out of bounds)
/// - Byte ranges are always valid (start <= end)
///
/// See: docs/specs/NODE_API_CONTRACT.md
#[derive(Clone, Copy)]
pub struct Node<'tree> {
    /// Reference to internal tree node data
    data: &'tree TreeNode,
    /// Language reference for symbol metadata
    language: Option<&'tree Language>,
}

impl<'tree> Node<'tree> {
    /// Create a new node (internal use)
    ///
    /// # Contract
    ///
    /// - `node` must be a valid TreeNode with valid ranges
    /// - `language` is optional (GLR mode may not have Language)
    pub(crate) fn new(node: &'tree TreeNode, language: Option<&'tree Language>) -> Self {
        Self {
            data: node,
            language,
        }
    }

    /// Get the node's symbol type name
    ///
    /// Returns the symbol name from the language's symbol_names array.
    /// Falls back to "unknown" if language is not set or symbol ID is out of bounds.
    ///
    /// # Phase 3.3: Now uses Language.symbol_names for actual symbol resolution
    pub fn kind(&self) -> &str {
        if let Some(language) = self.language {
            let symbol_id = self.data.symbol as usize;
            language
                .symbol_names
                .get(symbol_id)
                .map(|s| s.as_str())
                .unwrap_or("unknown")
        } else {
            "unknown"
        }
    }

    /// Get the node's symbol ID
    ///
    /// # Contract
    ///
    /// - Returns `data.symbol as u16`
    /// - Maps to grammar symbol IDs
    pub fn kind_id(&self) -> u16 {
        self.data.symbol as u16
    }

    /// Get the node's byte range
    ///
    /// # Contract
    ///
    /// - Returns `data.start_byte..data.end_byte`
    /// - Range is always valid: start <= end
    /// - Measured in bytes, not characters
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.data.start_byte..self.data.end_byte
    }

    /// Get the node's start byte
    pub fn start_byte(&self) -> usize {
        self.data.start_byte
    }

    /// Get the node's end byte
    pub fn end_byte(&self) -> usize {
        self.data.end_byte
    }

    /// Get the node's start position
    ///
    /// # Phase 1: Returns dummy (0, 0)
    /// # Phase 2: Will calculate from byte positions
    pub fn start_position(&self) -> Point {
        Point { row: 0, column: 0 }
    }

    /// Get the node's end position
    ///
    /// # Phase 1: Returns dummy (0, 0)
    /// # Phase 2: Will calculate from byte positions
    pub fn end_position(&self) -> Point {
        Point { row: 0, column: 0 }
    }

    /// Check if this node is named (visible in the tree)
    ///
    /// # Phase 1: Always returns true
    /// # Phase 2: Will use symbol_metadata.visible
    pub fn is_named(&self) -> bool {
        true
    }

    /// Check if this node is missing (error recovery)
    ///
    /// # Contract
    ///
    /// - Returns false (error recovery not implemented)
    pub fn is_missing(&self) -> bool {
        false
    }

    /// Check if this node is an error node
    ///
    /// # Contract
    ///
    /// - Returns false (error nodes not implemented)
    pub fn is_error(&self) -> bool {
        false
    }

    /// Get the number of children
    ///
    /// # Contract
    ///
    /// - Returns `data.children.len()`
    /// - Includes both named and anonymous children
    /// - Returns 0 for terminal nodes
    pub fn child_count(&self) -> usize {
        self.data.children.len()
    }

    /// Get the number of named children
    ///
    /// # Phase 1: Returns child_count() (no filtering)
    /// # Phase 2: Will filter by symbol_metadata.visible
    pub fn named_child_count(&self) -> usize {
        self.child_count()
    }

    /// Get a child by index
    ///
    /// # Contract
    ///
    /// - Returns Some(child) if index < child_count()
    /// - Returns None if index out of bounds
    /// - Child inherits parent's language
    pub fn child(&self, index: usize) -> Option<Node<'tree>> {
        self.data.children.get(index).map(|child| Node {
            data: child,
            language: self.language,
        })
    }

    /// Get a named child by index
    ///
    /// # Phase 1: Same as child(index) (no filtering)
    /// # Phase 2: Will skip unnamed children
    pub fn named_child(&self, index: usize) -> Option<Node<'tree>> {
        self.child(index)
    }

    /// Get a child by field name
    ///
    /// # Contract
    ///
    /// - Returns None (field access not implemented)
    pub fn child_by_field_name(&self, field_name: &str) -> Option<Node<'tree>> {
        let _ = field_name;
        None
    }

    /// Get the parent node
    ///
    /// # Contract
    ///
    /// - Returns None (parent links not stored)
    pub fn parent(&self) -> Option<Node<'tree>> {
        None
    }

    /// Get the next sibling
    ///
    /// # Contract
    ///
    /// - Returns None (sibling links not stored)
    pub fn next_sibling(&self) -> Option<Node<'tree>> {
        None
    }

    /// Get the previous sibling
    ///
    /// # Contract
    ///
    /// - Returns None (sibling links not stored)
    pub fn prev_sibling(&self) -> Option<Node<'tree>> {
        None
    }

    /// Get the next named sibling
    ///
    /// # Contract
    ///
    /// - Returns None (sibling links not stored)
    pub fn next_named_sibling(&self) -> Option<Node<'tree>> {
        None
    }

    /// Get the previous named sibling
    ///
    /// # Contract
    ///
    /// - Returns None (sibling links not stored)
    pub fn prev_named_sibling(&self) -> Option<Node<'tree>> {
        None
    }

    /// Get the UTF-8 text of this node
    ///
    /// # Contract
    ///
    /// - Extracts source[self.byte_range()]
    /// - Validates UTF-8 and returns error if invalid
    /// - Lifetime 'a independent of 'tree
    pub fn utf8_text<'a>(&self, source: &'a [u8]) -> Result<&'a str, std::str::Utf8Error> {
        let range = self.byte_range();
        std::str::from_utf8(&source[range])
    }
}

impl fmt::Debug for Node<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Node {{ kind: {}, range: {:?} }}",
            self.kind(),
            self.byte_range()
        )
    }
}

/// A point in the source text
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Point {
    /// Zero-indexed row
    pub row: usize,
    /// Zero-indexed column (in bytes)
    pub column: usize,
}

impl Point {
    /// Create a new point
    pub const fn new(row: usize, column: usize) -> Self {
        Self { row, column }
    }
}

impl fmt::Display for Point {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.row + 1, self.column + 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tree::TreeNode;

    fn leaf(symbol: u32, start: usize, end: usize) -> TreeNode {
        TreeNode::new_with_children(symbol, start, end, vec![])
    }

    fn parent_with(symbol: u32, start: usize, end: usize, children: Vec<TreeNode>) -> TreeNode {
        TreeNode::new_with_children(symbol, start, end, children)
    }

    #[test]
    fn point_new_stores_row_and_column() {
        let p = Point::new(3, 7);
        assert_eq!(p.row, 3);
        assert_eq!(p.column, 7);
    }

    #[test]
    fn point_display_is_one_indexed() {
        let p = Point::new(0, 0);
        assert_eq!(format!("{}", p), "1:1");
        let p = Point::new(4, 10);
        assert_eq!(format!("{}", p), "5:11");
    }

    #[test]
    fn point_derives_equality_and_copy() {
        let a = Point::new(2, 3);
        let b = a; // Copy
        assert_eq!(a, b);
        let c = Point::new(2, 4);
        assert_ne!(a, c);
    }

    #[test]
    fn point_ord_compares_row_then_column() {
        let a = Point::new(0, 5);
        let b = Point::new(1, 0);
        assert!(a < b);
        let c = Point::new(1, 1);
        assert!(b < c);
    }

    #[test]
    fn node_kind_without_language_is_unknown() {
        let data = leaf(0, 0, 4);
        let node = Node::new(&data, None);
        assert_eq!(node.kind(), "unknown");
    }

    #[test]
    fn node_kind_id_returns_symbol_as_u16() {
        let data = leaf(42, 0, 1);
        let node = Node::new(&data, None);
        assert_eq!(node.kind_id(), 42u16);
    }

    #[test]
    fn node_byte_range_matches_data() {
        let data = leaf(1, 3, 10);
        let node = Node::new(&data, None);
        assert_eq!(node.byte_range(), 3..10);
        assert_eq!(node.start_byte(), 3);
        assert_eq!(node.end_byte(), 10);
    }

    #[test]
    fn node_byte_range_zero_width() {
        let data = leaf(1, 5, 5);
        let node = Node::new(&data, None);
        let range = node.byte_range();
        assert_eq!(range.start, range.end);
        assert_eq!(node.start_byte(), 5);
        assert_eq!(node.end_byte(), 5);
    }

    #[test]
    fn node_phase1_positions_are_zero() {
        let data = leaf(1, 0, 4);
        let node = Node::new(&data, None);
        assert_eq!(node.start_position(), Point::new(0, 0));
        assert_eq!(node.end_position(), Point::new(0, 0));
    }

    #[test]
    fn node_default_flags_match_contract() {
        let data = leaf(1, 0, 4);
        let node = Node::new(&data, None);
        assert!(node.is_named());
        assert!(!node.is_missing());
        assert!(!node.is_error());
    }

    #[test]
    fn node_child_count_for_leaf_is_zero() {
        let data = leaf(1, 0, 4);
        let node = Node::new(&data, None);
        assert_eq!(node.child_count(), 0);
        assert_eq!(node.named_child_count(), 0);
        assert!(node.child(0).is_none());
        assert!(node.named_child(0).is_none());
    }

    #[test]
    fn node_child_returns_some_in_bounds_none_out_of_bounds() {
        let data = parent_with(0, 0, 4, vec![leaf(1, 0, 2), leaf(2, 2, 4)]);
        let node = Node::new(&data, None);
        assert_eq!(node.child_count(), 2);
        assert_eq!(node.named_child_count(), 2);
        let first = node.child(0).expect("first child");
        let second = node.child(1).expect("second child");
        assert_eq!(first.kind_id(), 1);
        assert_eq!(second.kind_id(), 2);
        assert_eq!(first.byte_range(), 0..2);
        assert_eq!(second.byte_range(), 2..4);
        assert!(node.child(2).is_none());
        assert!(node.named_child(2).is_none());
    }

    #[test]
    fn node_named_child_mirrors_child_phase1() {
        let data = parent_with(0, 0, 4, vec![leaf(7, 0, 2)]);
        let node = Node::new(&data, None);
        let via_child = node.child(0).unwrap();
        let via_named = node.named_child(0).unwrap();
        assert_eq!(via_child.kind_id(), via_named.kind_id());
        assert_eq!(via_child.byte_range(), via_named.byte_range());
    }

    #[test]
    fn node_field_and_link_methods_return_none() {
        let data = leaf(1, 0, 4);
        let node = Node::new(&data, None);
        assert!(node.child_by_field_name("anything").is_none());
        assert!(node.parent().is_none());
        assert!(node.next_sibling().is_none());
        assert!(node.prev_sibling().is_none());
        assert!(node.next_named_sibling().is_none());
        assert!(node.prev_named_sibling().is_none());
    }

    #[test]
    fn node_utf8_text_extracts_byte_range() {
        let data = leaf(1, 6, 11);
        let node = Node::new(&data, None);
        let source = b"hello world";
        let text = node.utf8_text(source).expect("valid utf8");
        assert_eq!(text, "world");
    }

    #[test]
    fn node_utf8_text_invalid_utf8_returns_err() {
        let data = leaf(1, 0, 2);
        let node = Node::new(&data, None);
        // 0xFF, 0xFE are not valid UTF-8 start bytes.
        let source: [u8; 2] = [0xFF, 0xFE];
        let res = node.utf8_text(&source);
        assert!(res.is_err());
    }

    #[test]
    fn node_debug_format_mentions_kind_and_range() {
        let data = leaf(0, 2, 5);
        let node = Node::new(&data, None);
        let dbg = format!("{:?}", node);
        assert!(dbg.contains("Node"), "debug output: {}", dbg);
        assert!(dbg.contains("unknown"), "debug output: {}", dbg);
        assert!(dbg.contains("2..5"), "debug output: {}", dbg);
    }

    #[test]
    fn node_is_copy_and_clone() {
        fn assert_copy<T: Copy>() {}
        fn assert_clone<T: Clone>() {}
        assert_copy::<Node<'_>>();
        assert_clone::<Node<'_>>();

        let data = leaf(3, 0, 2);
        let node = Node::new(&data, None);
        let copy = node; // Copy semantics keep original usable.
        assert_eq!(node.kind_id(), 3);
        assert_eq!(copy.kind_id(), 3);
    }
}
