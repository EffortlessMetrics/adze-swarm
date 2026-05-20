use super::Node;

/// Trait for bottom-up tree transformations.
///
/// Unlike [`super::Visitor`], a `TransformVisitor` produces a value at every
/// node. Children are transformed first and their results are passed to
/// [`transform_node`](Self::transform_node).
pub trait TransformVisitor {
    /// The type produced by each node transformation.
    type Output;

    /// Transforms an interior node given its already-transformed children.
    fn transform_node(&mut self, node: &Node, children: Vec<Self::Output>) -> Self::Output;

    /// Transforms a leaf node with its source text.
    fn transform_leaf(&mut self, node: &Node, text: &str) -> Self::Output;

    /// Transforms an error node.
    fn transform_error(&mut self, node: &Node) -> Self::Output;
}

/// Applies a [`TransformVisitor`] to a parse tree in post-order.
pub struct TransformWalker<'a> {
    source: &'a [u8],
}
impl<'a> TransformWalker<'a> {
    /// Creates a new transform walker for the given source bytes.
    pub fn new(source: &'a [u8]) -> Self {
        Self { source }
    }

    #[cfg(not(feature = "pure-rust"))]
    pub fn walk<T: TransformVisitor>(&self, root: Node, visitor: &mut T) -> T::Output {
        self.transform_node(root, visitor)
    }
    #[cfg(feature = "pure-rust")]
    pub fn walk<T: TransformVisitor>(&self, root: &Node, visitor: &mut T) -> T::Output {
        self.transform_node(root, visitor)
    }

    #[cfg(not(feature = "pure-rust"))]
    fn transform_node<T: TransformVisitor>(&self, node: Node, visitor: &mut T) -> T::Output {
        if node.is_error() {
            return visitor.transform_error(&node);
        }
        if node.child_count() == 0 {
            return visitor.transform_leaf(&node, node.utf8_text(self.source).unwrap_or(""));
        }
        let mut children = Vec::new();
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                children.push(self.transform_node(cursor.node(), visitor));
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
        visitor.transform_node(&node, children)
    }

    #[cfg(feature = "pure-rust")]
    fn transform_node<T: TransformVisitor>(&self, node: &Node, visitor: &mut T) -> T::Output {
        if node.is_error() {
            return visitor.transform_error(node);
        }
        if node.child_count() == 0 {
            let text = &self.source[node.start_byte()..node.end_byte()];
            return visitor.transform_leaf(node, std::str::from_utf8(text).unwrap_or(""));
        }
        let mut children = Vec::new();
        for child in node.children() {
            children.push(self.transform_node(child, visitor));
        }
        visitor.transform_node(node, children)
    }
}
