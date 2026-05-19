//! TreeNodeData - Arena-allocated parse tree node data
//!
//! This module provides the core data structure for parse tree nodes that are
//! allocated in the arena. TreeNodeData is designed to be memory-efficient,
//! cache-friendly, and support all tree-sitter node capabilities.
//!
//! # Design
//!
//! - **Size**: ≤64 bytes for cache efficiency
//! - **SmallVec optimization**: 0-3 children inline, heap for larger
//! - **Handle-based**: Child references via NodeHandle (arena indirection)
//! - **Packed flags**: 8 boolean flags in 1 byte
//!
//! # Example
//!
//! ```
//! use adze::tree_node_data::TreeNodeData;
//! use adze::arena_allocator::NodeHandle;
//!
//! // Create a leaf node
//! let leaf = TreeNodeData::leaf(5, 0, 10);
//!
//! // Create a branch node
//! let children = vec![NodeHandle::new(0, 0), NodeHandle::new(0, 1)];
//! let branch = TreeNodeData::branch(10, 0, 50, children);
//! ```
//!
//! Related: docs/specs/TREE_NODE_DATA_SPEC.md

use crate::arena_allocator::NodeHandle;
use smallvec::SmallVec;

/// Arena-allocated parse tree node data
///
/// This struct represents the data for a single node in the parse tree.
/// It is designed to be allocated in the arena for efficient memory usage.
///
/// Size: 56 bytes (with 8 bytes padding to 64-byte alignment)
#[derive(Clone, Debug)]
pub struct TreeNodeData {
    /// Symbol/kind ID from grammar
    symbol: u16,

    /// Byte range in source text
    start_byte: u32,
    end_byte: u32,

    /// Child node handles
    /// SmallVec optimizes for 0-3 children inline (no heap allocation)
    /// Larger child counts spill to heap automatically
    children: SmallVec<[NodeHandle; 3]>,

    /// Number of named children (subset of children)
    named_child_count: u16,

    /// Field ID (if this node is a named field)
    /// Uses niche optimization: None = 0xFFFF
    field_id: Option<u16>,

    /// Packed boolean flags
    flags: NodeFlags,
}

/// Packed node flags (8 flags in 1 byte)
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
struct NodeFlags {
    bits: u8,
}

impl NodeFlags {
    const IS_NAMED: u8 = 1 << 0; // Is this a named node?
    const IS_MISSING: u8 = 1 << 1; // Is this node missing (error recovery)?
    const IS_ERROR: u8 = 1 << 2; // Is this an error node?
    const IS_EXTRA: u8 = 1 << 3; // Is this an extra node?
    #[allow(dead_code)] // Reserved for incremental parsing
    const HAS_CHANGES: u8 = 1 << 4; // Has this node changed (incremental)?
    // 3 bits reserved for future use

    fn new() -> Self {
        NodeFlags { bits: 0 }
    }

    fn get(&self, mask: u8) -> bool {
        (self.bits & mask) != 0
    }

    fn set(&mut self, mask: u8, value: bool) {
        if value {
            self.bits |= mask;
        } else {
            self.bits &= !mask;
        }
    }
}

macro_rules! node_flag_accessors {
    ($(#[$get_doc:meta] $get:ident, #[$set_doc:meta] $set:ident, $flag:ident;)+) => {
        $(
            #[$get_doc]
            pub fn $get(&self) -> bool {
                self.flags.get(NodeFlags::$flag)
            }

            #[$set_doc]
            pub fn $set(&mut self, value: bool) {
                self.flags.set(NodeFlags::$flag, value);
            }
        )+
    };
}

impl TreeNodeData {
    /// Create a new node with symbol and byte range
    ///
    /// # Example
    ///
    /// ```
    /// use adze::tree_node_data::TreeNodeData;
    ///
    /// let node = TreeNodeData::new(42, 0, 10);
    /// assert_eq!(node.symbol(), 42);
    /// assert_eq!(node.byte_range(), (0, 10));
    /// ```
    pub fn new(symbol: u16, start_byte: u32, end_byte: u32) -> Self {
        TreeNodeData {
            symbol,
            start_byte,
            end_byte,
            children: SmallVec::new(),
            named_child_count: 0,
            field_id: None,
            flags: NodeFlags::new(),
        }
    }

    /// Create a leaf node (no children)
    ///
    /// This is equivalent to `new()` but makes intent clearer.
    ///
    /// # Example
    ///
    /// ```
    /// use adze::tree_node_data::TreeNodeData;
    ///
    /// let leaf = TreeNodeData::leaf(5, 10, 20);
    /// assert!(leaf.is_leaf());
    /// ```
    pub fn leaf(symbol: u16, start_byte: u32, end_byte: u32) -> Self {
        Self::new(symbol, start_byte, end_byte)
    }

    /// Create a branch node with children
    ///
    /// # Example
    ///
    /// ```
    /// use adze::tree_node_data::TreeNodeData;
    /// use adze::arena_allocator::NodeHandle;
    ///
    /// let children = vec![NodeHandle::new(0, 0), NodeHandle::new(0, 1)];
    /// let branch = TreeNodeData::branch(10, 0, 50, children);
    /// assert!(!branch.is_leaf());
    /// assert_eq!(branch.child_count(), 2);
    /// ```
    pub fn branch(
        symbol: u16,
        start_byte: u32,
        end_byte: u32,
        children: impl IntoIterator<Item = NodeHandle>,
    ) -> Self {
        let mut node = Self::new(symbol, start_byte, end_byte);
        node.children = children.into_iter().collect();
        node
    }

    // ========================================================================
    // Basic Accessors
    // ========================================================================

    /// Get the symbol/kind ID
    pub fn symbol(&self) -> u16 {
        self.symbol
    }

    /// Get start byte position
    pub fn start_byte(&self) -> u32 {
        self.start_byte
    }

    /// Get end byte position
    pub fn end_byte(&self) -> u32 {
        self.end_byte
    }

    /// Get byte range (start, end)
    pub fn byte_range(&self) -> (u32, u32) {
        (self.start_byte, self.end_byte)
    }

    /// Get text length in bytes
    pub fn byte_len(&self) -> u32 {
        self.end_byte.saturating_sub(self.start_byte)
    }

    // ========================================================================
    // Children
    // ========================================================================

    /// Get child count
    pub fn child_count(&self) -> usize {
        self.children.len()
    }

    /// Get named child count
    pub fn named_child_count(&self) -> usize {
        self.named_child_count as usize
    }

    /// Check if node has children
    pub fn is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    /// Get child by index
    ///
    /// Returns `None` if index is out of bounds.
    pub fn child(&self, index: usize) -> Option<NodeHandle> {
        self.children.get(index).copied()
    }

    /// Get all children as a slice
    pub fn children(&self) -> &[NodeHandle] {
        &self.children
    }

    /// Add a child node
    ///
    /// This adds an unnamed child to the node.
    pub fn add_child(&mut self, child: NodeHandle) {
        self.children.push(child);
    }

    /// Add a named child node
    ///
    /// This adds a named child and increments the named child count.
    pub fn add_named_child(&mut self, child: NodeHandle) {
        self.children.push(child);
        self.named_child_count = self.named_child_count.saturating_add(1);
    }

    // ========================================================================
    // Flags
    // ========================================================================

    node_flag_accessors! {
        /// Check if node is named
        is_named,
        /// Set named flag
        set_named,
        IS_NAMED;

        /// Check if node is error
        is_error,
        /// Set error flag
        set_error,
        IS_ERROR;

        /// Check if node is missing
        is_missing,
        /// Set missing flag
        set_missing,
        IS_MISSING;

        /// Check if node is extra
        is_extra,
        /// Set extra flag
        set_extra,
        IS_EXTRA;
    }

    // ========================================================================
    // Fields
    // ========================================================================

    /// Get field ID
    ///
    /// Returns `None` if this node is not a named field.
    pub fn field_id(&self) -> Option<u16> {
        self.field_id
    }

    /// Set field ID
    pub fn set_field_id(&mut self, id: Option<u16>) {
        self.field_id = id;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(debug_assertions))]
    macro_rules! debug_trace {
        ($($arg:tt)*) => {};
    }

    #[cfg(debug_assertions)]
    macro_rules! debug_trace {
        ($($arg:tt)*) => {
            if std::env::var("RUST_LOG")
                .ok()
                .unwrap_or_default()
                .contains("debug")
            {
                println!($($arg)*);
            }
        };
    }

    #[test]
    fn test_size_constraint() {
        use std::mem;

        let size = mem::size_of::<TreeNodeData>();
        assert!(
            size <= 64,
            "TreeNodeData is {} bytes, exceeds 64-byte target",
            size
        );

        debug_trace!("TreeNodeData size: {} bytes", size);
        debug_trace!("NodeHandle size: {} bytes", mem::size_of::<NodeHandle>());
        debug_trace!(
            "SmallVec size: {} bytes",
            mem::size_of::<SmallVec<[NodeHandle; 3]>>()
        );
    }

    #[test]
    fn test_flags_size() {
        use std::mem;

        assert_eq!(mem::size_of::<NodeFlags>(), 1);
    }

    #[test]
    fn test_new_and_leaf_initialize_empty_unnamed_nodes() {
        let node = TreeNodeData::new(42, 10, 25);
        assert_eq!(node.symbol(), 42);
        assert_eq!(node.start_byte(), 10);
        assert_eq!(node.end_byte(), 25);
        assert_eq!(node.byte_range(), (10, 25));
        assert_eq!(node.byte_len(), 15);
        assert_eq!(node.child_count(), 0);
        assert_eq!(node.named_child_count(), 0);
        assert!(node.is_leaf());
        assert!(node.children().is_empty());
        assert_eq!(node.field_id(), None);
        assert!(!node.is_named());
        assert!(!node.is_error());
        assert!(!node.is_missing());
        assert!(!node.is_extra());

        let leaf = TreeNodeData::leaf(7, 3, 3);
        assert_eq!(leaf.symbol(), 7);
        assert_eq!(leaf.byte_range(), (3, 3));
        assert_eq!(leaf.byte_len(), 0);
        assert!(leaf.is_leaf());
    }

    #[test]
    fn test_byte_len_saturates_when_end_precedes_start() {
        let node = TreeNodeData::new(1, 20, 5);

        assert_eq!(node.byte_range(), (20, 5));
        assert_eq!(node.byte_len(), 0);
    }

    #[test]
    fn test_branch_preserves_child_order_and_supports_heap_spill() {
        let children = [
            NodeHandle::new(0, 0),
            NodeHandle::new(0, 1),
            NodeHandle::new(0, 2),
            NodeHandle::new(0, 3),
        ];

        let branch = TreeNodeData::branch(10, 0, 50, children);

        assert!(!branch.is_leaf());
        assert_eq!(branch.child_count(), 4);
        assert_eq!(branch.named_child_count(), 0);
        assert_eq!(branch.children(), children);
        assert_eq!(branch.child(0), Some(NodeHandle::new(0, 0)));
        assert_eq!(branch.child(3), Some(NodeHandle::new(0, 3)));
        assert_eq!(branch.child(4), None);
    }

    #[test]
    fn test_add_child_and_add_named_child_update_counts() {
        let mut node = TreeNodeData::new(1, 0, 0);
        let unnamed = NodeHandle::new(1, 0);
        let named = NodeHandle::new(1, 1);

        node.add_child(unnamed);
        assert_eq!(node.child_count(), 1);
        assert_eq!(node.named_child_count(), 0);
        assert_eq!(node.child(0), Some(unnamed));
        assert!(!node.is_leaf());

        node.add_named_child(named);
        assert_eq!(node.child_count(), 2);
        assert_eq!(node.named_child_count(), 1);
        assert_eq!(node.child(1), Some(named));
        assert_eq!(node.children(), &[unnamed, named]);
    }

    #[test]
    fn test_add_named_child_saturates_named_count() {
        let mut node = TreeNodeData::new(1, 0, 0);
        node.named_child_count = u16::MAX;

        node.add_named_child(NodeHandle::new(0, 0));

        assert_eq!(node.named_child_count(), usize::from(u16::MAX));
        assert_eq!(node.child_count(), 1);
    }

    #[test]
    fn test_flags_can_be_toggled_independently() {
        let mut node = TreeNodeData::new(1, 0, 0);

        node.set_named(true);
        node.set_error(true);
        node.set_missing(true);
        node.set_extra(true);
        assert!(node.is_named());
        assert!(node.is_error());
        assert!(node.is_missing());
        assert!(node.is_extra());

        node.set_error(false);
        assert!(node.is_named());
        assert!(!node.is_error());
        assert!(node.is_missing());
        assert!(node.is_extra());

        node.set_named(false);
        node.set_missing(false);
        node.set_extra(false);
        assert!(!node.is_named());
        assert!(!node.is_error());
        assert!(!node.is_missing());
        assert!(!node.is_extra());
    }

    #[test]
    fn test_field_id_round_trips_and_clears() {
        let mut node = TreeNodeData::new(1, 0, 0);

        assert_eq!(node.field_id(), None);
        node.set_field_id(Some(17));
        assert_eq!(node.field_id(), Some(17));
        node.set_field_id(None);
        assert_eq!(node.field_id(), None);
    }
}
