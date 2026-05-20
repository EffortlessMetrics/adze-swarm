use super::{Node, Visitor, VisitorAction};
use std::collections::VecDeque;

/// Walks a parse tree in depth-first, pre-order fashion.
///
/// The walker invokes a [`Visitor`] at each node and uses the provided source
/// bytes to report text for leaf nodes.
pub struct TreeWalker<'a> {
    pub(crate) source: &'a [u8],
}

impl<'a> TreeWalker<'a> {
    /// Creates a new depth-first walker for the given source bytes.
    pub fn new(source: &'a [u8]) -> Self {
        Self { source }
    }

    #[cfg(feature = "pure-rust")]
    fn get_node_text(&self, node: &Node) -> String {
        let start = node.start_byte();
        let end = node.end_byte();
        if start < self.source.len() && end <= self.source.len() && start < end {
            std::str::from_utf8(&self.source[start..end])
                .unwrap_or("")
                .to_string()
        } else {
            String::new()
        }
    }

    /// Walk the tree depth-first with the given visitor.
    #[cfg(not(feature = "pure-rust"))]
    pub fn walk<V: Visitor>(&self, root: Node, visitor: &mut V) {
        self.walk_node(root, visitor);
    }

    /// Walk the tree depth-first with the given visitor.
    #[cfg(feature = "pure-rust")]
    pub fn walk<V: Visitor>(&self, root: &Node, visitor: &mut V) {
        self.walk_node(root, visitor);
    }

    #[cfg(not(feature = "pure-rust"))]
    fn walk_node<V: Visitor>(&self, node: Node, visitor: &mut V) {
        if node.is_error() {
            visitor.visit_error(&node);
            return;
        }
        match visitor.enter_node(&node) {
            VisitorAction::Stop => return,
            VisitorAction::SkipChildren => {
                visitor.leave_node(&node);
                return;
            }
            VisitorAction::Continue => {}
        }
        if node.child_count() == 0 {
            if let Ok(text) = node.utf8_text(self.source) {
                visitor.visit_leaf(&node, text);
            }
        } else {
            let mut cursor = node.walk();
            if cursor.goto_first_child() {
                loop {
                    self.walk_node(cursor.node(), visitor);
                    if !cursor.goto_next_sibling() {
                        break;
                    }
                }
            }
        }
        visitor.leave_node(&node);
    }

    #[cfg(feature = "pure-rust")]
    fn walk_node<V: Visitor>(&self, node: &Node, visitor: &mut V) {
        if node.is_error() {
            visitor.visit_error(node);
            return;
        }
        match visitor.enter_node(node) {
            VisitorAction::Stop => return,
            VisitorAction::SkipChildren => {
                visitor.leave_node(node);
                return;
            }
            VisitorAction::Continue => {}
        }
        if node.child_count() == 0 {
            let text = self.get_node_text(node);
            visitor.visit_leaf(node, &text);
        } else {
            for child in node.children() {
                self.walk_node(child, visitor);
            }
        }
        visitor.leave_node(node);
    }
}

/// Walks a parse tree in breadth-first, level-order fashion.
pub struct BreadthFirstWalker<'a> {
    pub(crate) source: &'a [u8],
}
impl<'a> BreadthFirstWalker<'a> {
    /// Creates a new breadth-first walker for the given source bytes.
    pub fn new(source: &'a [u8]) -> Self {
        Self { source }
    }

    /// Walk the tree breadth-first with the given visitor.
    #[cfg(not(feature = "pure-rust"))]
    pub fn walk<V: Visitor>(&self, root: Node, visitor: &mut V) {
        let mut queue = VecDeque::new();
        queue.push_back(root);
        while let Some(node) = queue.pop_front() {
            if node.is_error() {
                visitor.visit_error(&node);
                continue;
            }
            match visitor.enter_node(&node) {
                VisitorAction::Stop => return,
                VisitorAction::SkipChildren => continue,
                VisitorAction::Continue => {}
            }
            if node.child_count() == 0 {
                if let Ok(text) = node.utf8_text(self.source) {
                    visitor.visit_leaf(&node, text);
                }
            } else {
                let mut cursor = node.walk();
                if cursor.goto_first_child() {
                    loop {
                        queue.push_back(cursor.node());
                        if !cursor.goto_next_sibling() {
                            break;
                        }
                    }
                }
            }
        }
    }

    /// Walk the tree breadth-first with the given visitor.
    #[cfg(feature = "pure-rust")]
    pub fn walk<V: Visitor>(&self, root: &Node, visitor: &mut V) {
        let mut queue = VecDeque::new();
        queue.push_back(root);
        while let Some(node) = queue.pop_front() {
            if node.is_error() {
                visitor.visit_error(node);
                continue;
            }
            match visitor.enter_node(node) {
                VisitorAction::Stop => return,
                VisitorAction::SkipChildren => continue,
                VisitorAction::Continue => {}
            }
            if node.child_count() == 0 {
                let text = &self.source[node.start_byte()..node.end_byte()];
                if let Ok(text_str) = std::str::from_utf8(text) {
                    visitor.visit_leaf(node, text_str);
                }
            } else {
                for child in node.children() {
                    queue.push_back(child);
                }
            }
        }
    }
}
