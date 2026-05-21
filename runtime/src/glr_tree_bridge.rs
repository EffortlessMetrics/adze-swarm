//! Bridge between GLR forests and Tree-sitter trees.
#![cfg_attr(feature = "strict_docs", allow(missing_docs))]

// Bridge between GLR Subtree and Tree-sitter Node/Tree structures
// This module provides conversion and compatibility layer

use crate::subtree::Subtree;
use adze_ir::Grammar;
use std::collections::HashMap;
use std::sync::Arc;

/// A Tree-sitter compatible tree structure built from GLR Subtree
pub struct GLRTree {
    /// Root subtree from GLR parser
    root: Arc<Subtree>,
    /// Source text
    source: Vec<u8>,
    /// Grammar for symbol information
    grammar: Grammar,
    /// Map from Subtree pointer to node ID
    node_map: HashMap<usize, usize>,
    /// Next node ID
    next_node_id: usize,
}

impl GLRTree {
    /// Create a new GLR tree from a subtree
    pub fn new(root: Arc<Subtree>, source: Vec<u8>, grammar: Grammar) -> Self {
        let mut tree = Self {
            root,
            source,
            grammar,
            node_map: HashMap::new(),
            next_node_id: 0,
        };

        // Build node map
        tree.build_node_map(&tree.root.clone());
        tree
    }

    /// Build a map from subtree pointers to node IDs
    fn build_node_map(&mut self, subtree: &Arc<Subtree>) {
        let ptr = Arc::as_ptr(subtree) as usize;
        if !self.node_map.contains_key(&ptr) {
            self.node_map.insert(ptr, self.next_node_id);
            self.next_node_id += 1;

            for edge in &subtree.children {
                self.build_node_map(&edge.subtree);
            }
        }
    }

    /// Get root node
    pub fn root_node(&self) -> GLRNode<'_> {
        GLRNode {
            subtree: self.root.clone(),
            tree: self,
        }
    }

    /// Get the language (grammar)
    pub fn language(&self) -> &Grammar {
        &self.grammar
    }

    /// Get source text
    pub fn text(&self) -> &[u8] {
        &self.source
    }
}

/// A node in the GLR tree that provides Tree-sitter-like API
pub struct GLRNode<'tree> {
    subtree: Arc<Subtree>,
    tree: &'tree GLRTree,
}

impl<'tree> GLRNode<'tree> {
    /// Get the node's type (symbol name)
    pub fn kind(&self) -> &str {
        // Look up symbol name from grammar
        if let Some(name) = self
            .tree
            .grammar
            .rule_names
            .get(&self.subtree.node.symbol_id)
        {
            name
        } else if let Some(token) = self.tree.grammar.tokens.get(&self.subtree.node.symbol_id) {
            &token.name
        } else {
            "unknown"
        }
    }

    /// Get the node's symbol ID
    pub fn symbol(&self) -> u16 {
        self.subtree.node.symbol_id.0
    }

    /// Get start byte
    pub fn start_byte(&self) -> usize {
        self.subtree.node.byte_range.start
    }

    /// Get end byte
    pub fn end_byte(&self) -> usize {
        self.subtree.node.byte_range.end
    }

    /// Get byte range
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.subtree.node.byte_range.clone()
    }

    /// Check if this node is an error
    pub fn is_error(&self) -> bool {
        self.subtree.node.is_error
    }

    /// Check if this node has errors (including descendants)
    pub fn has_error(&self) -> bool {
        if self.is_error() {
            return true;
        }

        self.subtree.children.iter().any(|edge| {
            GLRNode {
                subtree: edge.subtree.clone(),
                tree: self.tree,
            }
            .has_error()
        })
    }

    /// Get child count
    pub fn child_count(&self) -> usize {
        self.subtree.children.len()
    }

    /// Get child at index
    pub fn child(&self, index: usize) -> Option<GLRNode<'tree>> {
        self.subtree.children.get(index).map(|edge| GLRNode {
            subtree: edge.subtree.clone(),
            tree: self.tree,
        })
    }

    /// Get child at index with field ID
    pub fn child_with_field(&self, index: usize) -> Option<(GLRNode<'tree>, u16)> {
        self.subtree.children.get(index).map(|edge| {
            (
                GLRNode {
                    subtree: edge.subtree.clone(),
                    tree: self.tree,
                },
                edge.field_id,
            )
        })
    }

    /// Get all children
    pub fn children(&self) -> impl Iterator<Item = GLRNode<'tree>> {
        let tree = self.tree;
        self.subtree.children.iter().map(move |edge| GLRNode {
            subtree: edge.subtree.clone(),
            tree,
        })
    }

    /// Get the field name for this node
    pub fn field_name(&self) -> Option<&str> {
        // Would need parent tracking to determine field name
        None
    }

    /// Convert node to S-expression format
    pub fn to_sexp(&self) -> String {
        self.to_sexp_internal(0)
    }

    fn to_sexp_internal(&self, depth: usize) -> String {
        let indent = "  ".repeat(depth);

        if self.child_count() == 0 {
            self.format_leaf_node(&indent)
        } else {
            self.format_non_leaf_node(depth, &indent)
        }
    }

    fn format_leaf_node(&self, indent: &str) -> String {
        format!("{}{}", indent, self.kind())
    }

    fn format_non_leaf_node(&self, depth: usize, indent: &str) -> String {
        let mut result = format!("{}({}", indent, self.kind());

        for edge in &self.subtree.children {
            result.push('\n');
            self.push_formatted_child(&mut result, edge, depth, indent);
        }

        result.push_str(&format!("\n{})", indent));
        result
    }

    fn push_formatted_child(
        &self,
        result: &mut String,
        edge: &crate::subtree::ChildEdge,
        depth: usize,
        indent: &str,
    ) {
        if let Some(field_name) = self.lookup_field_name(edge.field_id) {
            result.push_str(&format!("{}  {}: ", indent, field_name));
            result.push_str(self.child_for_edge(edge).to_sexp_internal(0).trim_start());
        } else {
            result.push_str(&self.child_for_edge(edge).to_sexp_internal(depth + 1));
        }
    }

    fn lookup_field_name(&self, field_id: u16) -> Option<&str> {
        if field_id == crate::subtree::FIELD_NONE {
            return None;
        }
        self.tree
            .grammar
            .fields
            .iter()
            .find(|(id, _)| id.0 == field_id)
            .map(|(_, field_name)| field_name.as_str())
    }

    fn child_for_edge(&self, edge: &crate::subtree::ChildEdge) -> GLRNode<'tree> {
        GLRNode {
            subtree: edge.subtree.clone(),
            tree: self.tree,
        }
    }

    /// Get child by field name
    pub fn child_by_field_name(&self, field_name: &str) -> Option<GLRNode<'tree>> {
        // Find the field ID for this name
        let field_id = self
            .tree
            .grammar
            .fields
            .iter()
            .find(|(_, name)| name.as_str() == field_name)
            .map(|(id, _)| id.0)?;

        // Find child with this field ID
        self.subtree
            .children
            .iter()
            .find(|edge| edge.field_id == field_id)
            .map(|edge| GLRNode {
                subtree: edge.subtree.clone(),
                tree: self.tree,
            })
    }

    /// Get text for this node
    pub fn utf8_text<'a>(&self, source: &'a [u8]) -> Result<&'a str, std::str::Utf8Error> {
        let range = self.byte_range();
        std::str::from_utf8(&source[range])
    }

    /// Get parent node (not implemented - would require parent tracking)
    pub fn parent(&self) -> Option<GLRNode<'tree>> {
        // Tree-sitter nodes track parent pointers, but our Subtrees don't
        // This would require a different tree structure or parent map
        None
    }

    /// Create a tree cursor starting at this node
    pub fn walk(&self) -> GLRTreeCursor<'tree> {
        GLRTreeCursor::new(self.clone())
    }

    /// Get node ID (for comparison)
    pub fn id(&self) -> usize {
        let ptr = Arc::as_ptr(&self.subtree) as usize;
        *self.tree.node_map.get(&ptr).unwrap_or(&0)
    }
}

impl<'tree> Clone for GLRNode<'tree> {
    fn clone(&self) -> Self {
        Self {
            subtree: self.subtree.clone(),
            tree: self.tree,
        }
    }
}

impl<'tree> PartialEq for GLRNode<'tree> {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.subtree, &other.subtree)
    }
}

impl<'tree> std::fmt::Debug for GLRNode<'tree> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GLRNode")
            .field("kind", &self.kind())
            .field("symbol", &self.symbol())
            .field("range", &self.byte_range())
            .field("children", &self.child_count())
            .finish()
    }
}

/// Tree cursor for traversing GLR trees
pub struct GLRTreeCursor<'tree> {
    /// Stack of (node, child_index) for traversal
    stack: Vec<(GLRNode<'tree>, usize)>,
    root: GLRNode<'tree>,
}

impl<'tree> GLRTreeCursor<'tree> {
    /// Create a new cursor at the given node
    pub fn new(node: GLRNode<'tree>) -> Self {
        let root = node.clone();
        Self {
            stack: vec![(node, 0)],
            root,
        }
    }

    /// Get current node
    pub fn node(&self) -> GLRNode<'tree> {
        self.stack
            .last()
            .map(|(node, _)| node.clone())
            .unwrap_or_else(|| self.root.clone())
    }

    /// Go to first child
    pub fn goto_first_child(&mut self) -> bool {
        if let Some((current, _)) = self.stack.last()
            && current.child_count() > 0
            && let Some(child) = current.child(0)
        {
            self.stack.push((child, 0));
            return true;
        }
        false
    }

    /// Go to next sibling
    pub fn goto_next_sibling(&mut self) -> bool {
        if self.stack.len() <= 1 {
            return false;
        }

        if let Some((_, index)) = self.stack.pop()
            && let Some((parent, _)) = self.stack.last()
        {
            let next_index = index + 1;
            if next_index < parent.child_count()
                && let Some(sibling) = parent.child(next_index)
            {
                self.stack.push((sibling, next_index));
                return true;
            }
        }
        false
    }

    /// Go to parent
    pub fn goto_parent(&mut self) -> bool {
        if self.stack.len() > 1 {
            self.stack.pop();
            true
        } else {
            false
        }
    }

    /// Reset cursor to a node
    pub fn reset(&mut self, node: GLRNode<'tree>) {
        let node = node.clone();
        self.stack.clear();
        self.stack.push((node.clone(), 0));
        self.root = node;
    }

    /// Get field name of current node (not implemented - would require field tracking)
    pub fn field_name(&self) -> Option<&str> {
        // This would require tracking field information during parsing
        None
    }
}

/// Convert a GLR Subtree to a Tree-sitter compatible tree
pub fn subtree_to_tree(subtree: Arc<Subtree>, source: Vec<u8>, grammar: Grammar) -> GLRTree {
    GLRTree::new(subtree, source, grammar)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::subtree::SubtreeNode;
    use adze_ir::SymbolId;

    #[test]
    fn test_glr_node_api() {
        // Create a simple subtree
        let root = Arc::new(Subtree::new_with_fields(
            SubtreeNode {
                symbol_id: SymbolId(1),
                is_error: false,
                byte_range: 0..10,
            },
            vec![
                crate::subtree::ChildEdge {
                    subtree: Arc::new(Subtree::new(
                        SubtreeNode {
                            symbol_id: SymbolId(2),
                            is_error: false,
                            byte_range: 0..5,
                        },
                        vec![],
                    )),
                    field_id: 0, // Field "left"
                },
                crate::subtree::ChildEdge {
                    subtree: Arc::new(Subtree::new(
                        SubtreeNode {
                            symbol_id: SymbolId(3),
                            is_error: false,
                            byte_range: 5..10,
                        },
                        vec![],
                    )),
                    field_id: 1, // Field "right"
                },
            ],
        ));

        let source = b"hello world".to_vec();
        let mut grammar = Grammar::new("test".to_string());
        grammar.rule_names.insert(SymbolId(1), "root".to_string());
        grammar.rule_names.insert(SymbolId(2), "left".to_string());
        grammar.rule_names.insert(SymbolId(3), "right".to_string());

        // Add field names
        grammar
            .fields
            .insert(adze_ir::FieldId(0), "left".to_string());
        grammar
            .fields
            .insert(adze_ir::FieldId(1), "right".to_string());

        let tree = GLRTree::new(root, source, grammar);
        let root_node = tree.root_node();

        // Test node API
        assert_eq!(root_node.kind(), "root");
        assert_eq!(root_node.symbol(), 1);
        assert_eq!(root_node.start_byte(), 0);
        assert_eq!(root_node.end_byte(), 10);
        assert!(!root_node.is_error());
        assert_eq!(root_node.child_count(), 2);

        // Test children
        let child1 = root_node.child(0).unwrap();
        assert_eq!(child1.kind(), "left");
        assert_eq!(child1.byte_range(), 0..5);

        let child2 = root_node.child(1).unwrap();
        assert_eq!(child2.kind(), "right");
        assert_eq!(child2.byte_range(), 5..10);

        // Test field access
        let left_child = root_node.child_by_field_name("left").unwrap();
        assert_eq!(left_child.kind(), "left");

        let right_child = root_node.child_by_field_name("right").unwrap();
        assert_eq!(right_child.kind(), "right");
    }

    #[test]
    fn test_child_edge_size() {
        // Ensure ChildEdge doesn't bloat the tree structure too much
        // On 64-bit: Arc<Subtree> is 8 bytes, field_id is 2 bytes, total should be <= 16 with padding
        let expected_max_size = if cfg!(target_pointer_width = "64") {
            16
        } else {
            8
        };
        let actual_size = std::mem::size_of::<crate::subtree::ChildEdge>();
        assert!(
            actual_size <= expected_max_size,
            "ChildEdge size {} exceeds expected maximum {}",
            actual_size,
            expected_max_size
        );
    }

    #[test]
    fn test_tree_cursor() {
        let root = Arc::new(Subtree::new(
            SubtreeNode {
                symbol_id: SymbolId(1),
                is_error: false,
                byte_range: 0..20,
            },
            vec![
                Arc::new(Subtree::new(
                    SubtreeNode {
                        symbol_id: SymbolId(2),
                        is_error: false,
                        byte_range: 0..10,
                    },
                    vec![Arc::new(Subtree::new(
                        SubtreeNode {
                            symbol_id: SymbolId(4),
                            is_error: false,
                            byte_range: 0..5,
                        },
                        vec![],
                    ))],
                )),
                Arc::new(Subtree::new(
                    SubtreeNode {
                        symbol_id: SymbolId(3),
                        is_error: false,
                        byte_range: 10..20,
                    },
                    vec![],
                )),
            ],
        ));

        let tree = GLRTree::new(root, vec![], Grammar::new("test".to_string()));
        let mut cursor = tree.root_node().walk();

        // Test cursor navigation
        assert_eq!(cursor.node().symbol(), 1);

        // Go to first child
        assert!(cursor.goto_first_child());
        assert_eq!(cursor.node().symbol(), 2);

        // Go to grandchild
        assert!(cursor.goto_first_child());
        assert_eq!(cursor.node().symbol(), 4);

        // Can't go deeper
        assert!(!cursor.goto_first_child());

        // Go back to parent
        assert!(cursor.goto_parent());
        assert_eq!(cursor.node().symbol(), 2);

        // Go to sibling
        assert!(cursor.goto_next_sibling());
        assert_eq!(cursor.node().symbol(), 3);

        // No more siblings
        assert!(!cursor.goto_next_sibling());
    }
}
