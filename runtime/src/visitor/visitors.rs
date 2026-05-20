use super::{Node, Visitor, VisitorAction};

/// A [`Visitor`] that collects statistics about the parse tree.
///
/// After a walk, inspect [`total_nodes`](Self::total_nodes),
/// [`leaf_nodes`](Self::leaf_nodes), [`error_nodes`](Self::error_nodes),
/// [`max_depth`](Self::max_depth), and per-kind counts in
/// [`node_counts`](Self::node_counts).
#[derive(Debug, Default)]
pub struct StatsVisitor {
    /// Total number of nodes visited.
    pub total_nodes: usize,
    /// Number of leaf, childless nodes.
    pub leaf_nodes: usize,
    /// Number of error nodes.
    pub error_nodes: usize,
    /// Maximum depth reached during traversal.
    pub max_depth: usize,
    /// Per-kind node counts.
    pub node_counts: std::collections::HashMap<String, usize>,
    current_depth: usize,
}
impl Visitor for StatsVisitor {
    fn enter_node(&mut self, node: &Node) -> VisitorAction {
        self.total_nodes += 1;
        self.current_depth += 1;
        self.max_depth = self.max_depth.max(self.current_depth);
        *self.node_counts.entry(node.kind().to_string()).or_insert(0) += 1;
        VisitorAction::Continue
    }
    fn leave_node(&mut self, _node: &Node) {
        self.current_depth -= 1;
    }
    fn visit_leaf(&mut self, _node: &Node, _text: &str) {
        self.leaf_nodes += 1;
    }
    fn visit_error(&mut self, _node: &Node) {
        self.error_nodes += 1;
    }
}

/// A [`Visitor`] that records nodes matching a user-supplied predicate.
///
/// After the walk, matching nodes are stored in [`matches`](Self::matches) as
/// `(start_byte, end_byte, kind)` tuples.
pub struct SearchVisitor<F> {
    predicate: F,
    /// Matched nodes as `(start_byte, end_byte, kind)` tuples.
    pub matches: Vec<(usize, usize, String)>,
}
impl<F> SearchVisitor<F>
where
    F: Fn(&Node) -> bool,
{
    /// Creates a new search visitor with the given predicate.
    pub fn new(predicate: F) -> Self {
        Self {
            predicate,
            matches: Vec::new(),
        }
    }
}
impl<F> Visitor for SearchVisitor<F>
where
    F: Fn(&Node) -> bool,
{
    fn enter_node(&mut self, node: &Node) -> VisitorAction {
        if (self.predicate)(node) {
            self.matches
                .push((node.start_byte(), node.end_byte(), node.kind().to_string()));
        }
        VisitorAction::Continue
    }
}

/// A [`Visitor`] that produces an indented, human-readable representation of
/// the parse tree.
///
/// After the walk, call [`output`](Self::output) to retrieve the formatted
/// string.
#[derive(Debug, Clone)]
pub struct PrettyPrintVisitor {
    indent: usize,
    output: String,
}
impl Default for PrettyPrintVisitor {
    fn default() -> Self {
        Self::new()
    }
}
impl PrettyPrintVisitor {
    /// Creates a new pretty-print visitor with no accumulated output.
    pub fn new() -> Self {
        Self {
            indent: 0,
            output: String::new(),
        }
    }

    /// Returns the accumulated pretty-printed output.
    pub fn output(&self) -> &str {
        &self.output
    }
}
impl Visitor for PrettyPrintVisitor {
    fn enter_node(&mut self, node: &Node) -> VisitorAction {
        self.output
            .push_str(&format!("{}{}", "  ".repeat(self.indent), node.kind()));
        if node.is_named() {
            self.output.push_str(" [named]");
        }
        self.output.push('\n');
        self.indent += 1;
        VisitorAction::Continue
    }
    fn leave_node(&mut self, _node: &Node) {
        self.indent -= 1;
    }
    fn visit_leaf(&mut self, _node: &Node, text: &str) {
        self.output
            .push_str(&format!("{}\"{}\"\n", "  ".repeat(self.indent), text));
    }
    fn visit_error(&mut self, node: &Node) {
        self.output.push_str(&format!(
            "{}ERROR: {}\n",
            "  ".repeat(self.indent),
            node.kind()
        ));
    }
}
