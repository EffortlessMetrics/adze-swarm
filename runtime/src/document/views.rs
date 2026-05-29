//! Borrowed document tree, node, edge, and typed syntax handles.

use super::*;

/// Borrowed generic CST view for an [`AdzeDocument`].
#[derive(Clone, Copy, Debug)]
pub struct AdzeTree<'doc> {
    pub(in crate::document) document: &'doc AdzeDocument,
}

impl<'doc> AdzeTree<'doc> {
    /// Return language metadata for this tree.
    pub fn language(&self) -> &'doc LanguageMetadata {
        self.document.language()
    }

    /// Return the root node id.
    pub fn root_id(&self) -> NodeId {
        self.document.syntax.root_id()
    }

    /// Return the number of indexed nodes in this tree.
    pub fn node_count(&self) -> usize {
        self.document.syntax.node_count()
    }

    /// Return the number of direct parent-to-child edges in this tree.
    pub fn edge_count(&self) -> usize {
        self.document.syntax.edge_count()
    }

    /// Return the direct selected-tree record for a node id.
    pub fn node_record(&self, node_id: NodeId) -> Option<&'doc NodeRecord> {
        self.document.syntax.node_record(node_id)
    }

    /// Return the direct selected-tree record for an edge index.
    pub fn edge_record(&self, edge_index: usize) -> Option<&'doc EdgeRecord> {
        self.document.syntax.edge_record(edge_index)
    }

    /// Return a node by document-local id.
    pub fn node(&self, node_id: NodeId) -> Option<AdzeNode<'doc>> {
        self.document.node_by_id(node_id).map(|node| AdzeNode {
            document: self.document,
            node,
            id: node_id,
        })
    }

    /// Return the root node.
    pub fn root(&self) -> AdzeNode<'doc> {
        AdzeNode {
            document: self.document,
            node: &self.document.root,
            id: self.root_id(),
        }
    }

    /// Return whether this tree has parser errors.
    pub fn has_errors(&self) -> bool {
        self.document.metadata.error_count > 0
    }

    /// Return the number of parser recovery/error events.
    pub fn error_count(&self) -> usize {
        self.document.metadata.error_count
    }
}

/// Borrowed generic CST node view.
#[derive(Clone, Copy, Debug)]
pub struct AdzeNode<'doc> {
    pub(in crate::document) document: &'doc AdzeDocument,
    pub(in crate::document) node: &'doc ParseNode,
    pub(in crate::document) id: NodeId,
}

impl<'doc> AdzeNode<'doc> {
    /// Return this node's document-local id.
    pub fn node_id(&self) -> NodeId {
        self.id
    }

    /// Return this node's direct selected-tree record.
    pub fn record(&self) -> &'doc NodeRecord {
        self.document
            .syntax
            .node_record(self.id)
            .expect("AdzeNode ids are created only from indexed document records")
    }

    /// Return this node's parent id, if it is not the root.
    pub fn parent_id(&self) -> Option<NodeId> {
        self.document.syntax.parent_id(self.id)
    }

    /// Return this node's parent, if it is not the root.
    pub fn parent(&self) -> Option<AdzeNode<'doc>> {
        self.parent_id()
            .and_then(|parent_id| self.document.tree().node(parent_id))
    }

    /// Return the edge that connects this node to its parent.
    pub fn parent_edge(&self) -> Option<AdzeEdge<'doc>> {
        self.document
            .syntax
            .parent_edge_record(self.id)
            .map(|record| AdzeEdge {
                document: self.document,
                record,
            })
    }

    /// Return metadata for this node's kind, when known.
    pub fn kind(&self) -> Option<&'doc NodeKind> {
        self.document.language.symbol(self.symbol_id())
    }

    fn visible_kind(&self) -> Option<&'doc NodeKind> {
        let visible_id = self.identity().visible_id();
        self.document.language.symbol(visible_id)
    }

    /// Return this node's native identity.
    ///
    /// The document keeps raw grammar identity separate from alias-visible
    /// identity so compatibility projections can expose Tree-sitter-style
    /// aliases without losing the original grammar symbol.
    pub fn identity(&self) -> NodeIdentity<'doc> {
        let record = self.record();
        let grammar_kind = self.document.language.symbol(record.grammar_id());
        let grammar_name = grammar_kind.map(NodeKind::name);
        let grammar_is_named = grammar_kind.map(NodeKind::is_named).unwrap_or(false);
        let alias_kind = record
            .alias_symbol_id()
            .and_then(|alias_symbol_id| self.document.language.symbol(alias_symbol_id));
        let visible_kind = alias_kind.or(grammar_kind);
        let visible_id = record.visible_id();
        let visible_name = visible_kind.map(NodeKind::name);
        let visible_is_named = visible_kind
            .map(NodeKind::is_named)
            .unwrap_or(grammar_is_named);

        NodeIdentity {
            visible_id,
            grammar_id: record.grammar_id(),
            visible_name,
            grammar_name,
            alias_symbol_id: record.alias_symbol_id(),
            visible_is_named,
            grammar_is_named,
        }
    }

    /// Return this node's display kind name, when known.
    pub fn kind_name(&self) -> Option<&'doc str> {
        self.identity().visible_name()
    }

    /// Return this node's grammar symbol name, ignoring aliases.
    pub fn grammar_name(&self) -> Option<&'doc str> {
        self.identity().grammar_name()
    }

    /// Return this node's visible kind id.
    pub fn kind_id(&self) -> SymbolId {
        self.identity().visible_id()
    }

    /// Return this node's grammar symbol id, ignoring aliases.
    pub fn grammar_id(&self) -> SymbolId {
        self.identity().grammar_id()
    }

    /// Return the node's grammar symbol id.
    pub fn symbol_id(&self) -> SymbolId {
        self.record().grammar_id()
    }

    /// Return the start byte for this node.
    pub fn start_byte(&self) -> usize {
        self.record().byte_range().start
    }

    /// Return the end byte for this node.
    pub fn end_byte(&self) -> usize {
        self.record().byte_range().end
    }

    /// Return the byte range for this node.
    pub fn byte_range(&self) -> Range<usize> {
        self.record().byte_range()
    }

    /// Return this node's zero-based point range.
    pub fn point_range(&self) -> PointRange {
        self.record().point_range()
    }

    /// Return this node's source text if the byte range is valid UTF-8.
    pub fn utf8_text(&self) -> Result<&'doc str, std::str::Utf8Error> {
        let slice = self
            .document
            .source_bytes()
            .get(self.byte_range())
            .unwrap_or(&[]);
        std::str::from_utf8(slice)
    }

    /// Return the field name attached to this node's parent edge, if any.
    pub fn field_name(&self) -> Option<&'doc str> {
        self.parent_edge().and_then(|edge| edge.field_name())
    }

    /// Return the public field id attached to this node's parent edge, if any.
    pub fn field_id(&self) -> Option<FieldId> {
        self.parent_edge().and_then(|edge| edge.field_id())
    }

    /// Return the number of direct children.
    pub fn child_count(&self) -> usize {
        self.node.children.len()
    }

    /// Return a child by index.
    pub fn child(&self, index: usize) -> Option<AdzeNode<'doc>> {
        self.child_edge(index)?.child()
    }

    /// Return a child edge by index.
    pub fn child_edge(&self, index: usize) -> Option<AdzeEdge<'doc>> {
        self.node.children.get(index)?;
        let record = self.document.syntax.child_edge_record(self.id, index)?;
        Some(AdzeEdge {
            document: self.document,
            record,
        })
    }

    /// Return direct child edges in source order.
    pub fn child_edges(&self) -> impl Iterator<Item = AdzeEdge<'doc>> + '_ {
        (0..self.child_count()).filter_map(|index| self.child_edge(index))
    }

    /// Return the field name for a child edge by index.
    pub fn field_name_for_child(&self, index: usize) -> Option<&'doc str> {
        self.child_edge(index).and_then(|edge| edge.field_name())
    }

    /// Return the public field id for a child edge by index.
    pub fn field_id_for_child(&self, index: usize) -> Option<FieldId> {
        self.child_edge(index).and_then(|edge| edge.field_id())
    }

    /// Return the first child edge attached through the given field name.
    pub fn edge_by_field_name(&self, field_name: &str) -> Option<AdzeEdge<'doc>> {
        self.child_edges()
            .find(|edge| edge.field_name() == Some(field_name))
    }

    /// Return the first child attached through the given field name.
    pub fn child_by_field_name(&self, field_name: &str) -> Option<AdzeNode<'doc>> {
        self.edge_by_field_name(field_name)?.child()
    }

    /// Return the first child attached through the given public field id.
    pub fn child_by_field_id(&self, field_id: FieldId) -> Option<AdzeNode<'doc>> {
        self.child_edges()
            .find(|edge| edge.field_id() == Some(field_id))?
            .child()
    }

    /// Return native structural flags for this node.
    pub fn flags(&self) -> NodeFlags {
        self.record().flags()
    }

    /// Return whether this node is named according to language metadata.
    pub fn is_named(&self) -> bool {
        self.identity().visible_is_named()
    }

    /// Return whether this node is visible according to language metadata.
    pub fn is_visible(&self) -> bool {
        self.visible_kind()
            .map(NodeKind::is_visible)
            .unwrap_or(false)
    }

    /// Return whether this node is an extra syntax node according to metadata.
    pub fn is_extra(&self) -> bool {
        self.visible_kind().map(NodeKind::is_extra).unwrap_or(false)
    }

    /// Return whether this node is a terminal token according to metadata.
    pub fn is_terminal(&self) -> bool {
        self.visible_kind()
            .map(NodeKind::is_terminal)
            .unwrap_or(false)
    }

    /// Return whether this node is a supertype according to metadata.
    pub fn is_supertype(&self) -> bool {
        self.visible_kind()
            .map(NodeKind::is_supertype)
            .unwrap_or(false)
    }

    /// Return whether this node is a local synthetic error node.
    pub fn is_error(&self) -> bool {
        self.flags().is_error()
    }

    /// Return whether this node is a zero-width synthetic missing node.
    pub fn is_missing(&self) -> bool {
        self.flags().is_missing()
    }

    /// Return whether this node or its descendants carry error state.
    pub fn has_error(&self) -> bool {
        self.flags().has_error()
    }

    /// Return diagnostics directly related to this node.
    pub fn diagnostics(&self) -> impl Iterator<Item = &'doc ParseDiagnostic> + 'doc {
        let node_id = self.id;
        self.document
            .diagnostics
            .iter()
            .filter(move |diagnostic| diagnostic.related_nodes.contains(&node_id))
    }
}

/// Borrowed parent-to-child CST edge view.
///
/// Field labels belong to edges, not globally to child nodes. `AdzeEdge`
/// makes that relationship explicit for native syntax tooling and future
/// generated typed CST accessors.
#[derive(Clone, Copy, Debug)]
pub struct AdzeEdge<'doc> {
    pub(in crate::document) document: &'doc AdzeDocument,
    pub(in crate::document) record: &'doc EdgeRecord,
}

impl<'doc> AdzeEdge<'doc> {
    /// Return this edge's direct selected-tree record.
    pub fn record(&self) -> &'doc EdgeRecord {
        self.record
    }

    /// Return the parent node id for this edge.
    pub fn parent_id(&self) -> NodeId {
        self.record.parent_id()
    }

    /// Return this edge's child index within its parent.
    pub fn child_index(&self) -> usize {
        self.record.child_index()
    }

    /// Return the child node id for this edge.
    pub fn child_id(&self) -> NodeId {
        self.record.child_id()
    }

    /// Return the child node for this edge.
    pub fn child(&self) -> Option<AdzeNode<'doc>> {
        self.document.tree().node(self.record.child_id())
    }

    /// Return the field name attached to this edge, if any.
    pub fn field_name(&self) -> Option<&'doc str> {
        self.record
            .field_id()
            .and_then(|field_id| self.document.language().field_name_for_id(field_id.get()))
    }

    /// Return the public field id attached to this edge, if any.
    pub fn field_id(&self) -> Option<FieldId> {
        self.record.field_id()
    }
}

/// Common handle contract for generated typed CST wrappers.
///
/// Typed CST wrappers should be cheap views over [`AdzeDocument`] node IDs.
/// The default helpers are fallible so a wrapper can preserve honest behavior
/// when constructed around stale, recovered, or dynamically discovered syntax.
/// Generated wrappers are expected to validate their kind in their own `cast`
/// constructors before implementing typed field accessors.
pub trait SyntaxNode<'doc>: Copy {
    /// Return the document backing this typed CST handle.
    fn document(&self) -> &'doc AdzeDocument;

    /// Return the document-local node id represented by this handle.
    fn node_id(&self) -> NodeId;

    /// Return the generic CST node for this typed handle.
    fn node(&self) -> Option<AdzeNode<'doc>> {
        self.document().tree().node(self.node_id())
    }

    /// Return this handle's display kind name, when the node resolves.
    fn kind_name(&self) -> Option<&'doc str> {
        self.node().and_then(|node| node.kind_name())
    }

    /// Return this handle's byte range, when the node resolves.
    fn byte_range(&self) -> Option<Range<usize>> {
        self.node().map(|node| node.byte_range())
    }

    /// Return this handle's zero-based point range, when the node resolves.
    fn point_range(&self) -> Option<PointRange> {
        self.node().map(|node| node.point_range())
    }

    /// Return this handle's source text, when the range is a valid UTF-8 slice.
    fn text(&self) -> Option<&'doc str> {
        self.byte_range()
            .and_then(|range| self.document().source_slice(range))
    }

    /// Return a child node by index, when the node and child resolve.
    fn child(&self, index: usize) -> Option<AdzeNode<'doc>> {
        self.node()?.child(index)
    }

    /// Return a child edge by index, when the node and edge resolve.
    fn child_edge(&self, index: usize) -> Option<AdzeEdge<'doc>> {
        self.node()?.child_edge(index)
    }

    /// Return a child edge by native field name.
    fn edge_by_field_name(&self, field_name: &str) -> Option<AdzeEdge<'doc>> {
        self.node()?.edge_by_field_name(field_name)
    }

    /// Return a child node by native field name.
    fn child_by_field_name(&self, field_name: &str) -> Option<AdzeNode<'doc>> {
        self.node()?.child_by_field_name(field_name)
    }

    /// Return whether this handle resolves to a node-local synthetic error.
    fn is_error(&self) -> bool {
        self.node().map(|node| node.is_error()).unwrap_or(false)
    }

    /// Return whether this handle resolves to a zero-width missing node.
    fn is_missing(&self) -> bool {
        self.node().map(|node| node.is_missing()).unwrap_or(false)
    }

    /// Return whether this handle resolves to syntax that carries error state.
    fn has_error(&self) -> bool {
        self.node().map(|node| node.has_error()).unwrap_or(false)
    }

    /// Extract a typed AST from this typed CST handle's node.
    ///
    /// This is an alpha bridge between typed CST and typed AST views. The
    /// wrapper remains a cheap node handle; extraction still happens through
    /// the backing [`AdzeDocument`] and records this handle's node id as
    /// document-level provenance.
    fn ast<T>(&self) -> Result<TypedAst<T>, Vec<crate::errors::ParseError>>
    where
        T: crate::Extract<T>,
    {
        self.document().ast_from_node(self.node_id())
    }
}
