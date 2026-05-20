use super::Node;

/// Trait for visiting nodes in a parse tree.
///
/// Implement one or more of the provided methods to react to specific node
/// types during traversal. All methods have default no-op implementations so
/// you only need to override the ones you care about.
///
/// # Traversal control
///
/// [`enter_node`](Self::enter_node) returns a [`VisitorAction`] that controls
/// whether traversal continues into children, skips them, or stops entirely.
pub trait Visitor {
    /// Called when a node is first entered during traversal.
    ///
    /// Return [`VisitorAction::Continue`] to visit children,
    /// [`VisitorAction::SkipChildren`] to skip them, or
    /// [`VisitorAction::Stop`] to halt the walk.
    fn enter_node(&mut self, _node: &Node) -> VisitorAction {
        VisitorAction::Continue
    }

    /// Called after all of a node's children have been visited.
    fn leave_node(&mut self, _node: &Node) {}

    /// Called for leaf nodes (nodes with no children). `text` is the source
    /// text spanned by the node.
    fn visit_leaf(&mut self, _node: &Node, _text: &str) {}

    /// Called for nodes that represent parse errors.
    fn visit_error(&mut self, _node: &Node) {}
}

/// Controls how traversal proceeds after visiting a node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VisitorAction {
    /// Continue traversal into this node's children.
    Continue,
    /// Skip this node's children but continue with its siblings.
    SkipChildren,
    /// Stop the entire traversal immediately.
    Stop,
}
