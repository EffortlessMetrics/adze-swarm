//! Direct syntax storage, node identity, and language metadata types.

use super::tree_builder::{collect_edge_records, collect_node_records, insert_symbol};
use super::*;

/// Stable node identifier within one [`AdzeDocument`].
///
/// Node IDs are assigned in preorder over the selected parse tree. They are
/// stable for the lifetime of a document but are not meaningful across parses.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct NodeId(usize);

impl NodeId {
    /// Construct a node id from its raw preorder index.
    pub fn new(index: usize) -> Self {
        Self(index)
    }

    /// Return this node id as a raw preorder index.
    pub fn as_usize(self) -> usize {
        self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::document) struct NodeIndex {
    pub(in crate::document) path: Vec<usize>,
    pub(in crate::document) parent_id: Option<NodeId>,
    pub(in crate::document) child_ids: Vec<NodeId>,
}

/// Direct selected-tree storage for an [`AdzeDocument`].
///
/// This is the alpha v2 storage layer behind the borrowed [`AdzeTree`] view.
/// It records document-local node and edge facts directly so future typed CST,
/// diagnostics, JSON, and Tree-sitter compatibility projections do not need to
/// infer structure from recursive tree path replay.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxTree {
    pub(in crate::document) root_id: NodeId,
    pub(in crate::document) nodes: Vec<NodeRecord>,
    pub(in crate::document) edges: Vec<EdgeRecord>,
    pub(in crate::document) parents: Vec<Option<NodeId>>,
}

impl SyntaxTree {
    pub(in crate::document) fn from_parse_root(
        root: &ParseNode,
        source: &str,
        language: &LanguageMetadata,
        error_count: usize,
        node_index: &[NodeIndex],
    ) -> Self {
        let mut nodes = Vec::new();
        collect_node_records(root, source, language, error_count, true, &mut nodes);

        let mut tree = Self {
            root_id: NodeId::new(0),
            nodes,
            edges: Vec::new(),
            parents: node_index.iter().map(|index| index.parent_id).collect(),
        };
        collect_edge_records(root, tree.root_id, node_index, language, &mut tree);
        tree
    }

    /// Return the document-local root node id.
    pub fn root_id(&self) -> NodeId {
        self.root_id
    }

    /// Return the number of direct node records in this tree.
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Return the number of direct parent-to-child edge records.
    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    /// Return the direct record for a document-local node id.
    pub fn node_record(&self, node_id: NodeId) -> Option<&NodeRecord> {
        self.nodes.get(node_id.as_usize())
    }

    /// Return the direct record for an edge index.
    pub fn edge_record(&self, edge_index: usize) -> Option<&EdgeRecord> {
        self.edges.get(edge_index)
    }

    /// Return the parent id for a document-local node id.
    pub fn parent_id(&self, node_id: NodeId) -> Option<NodeId> {
        self.parents.get(node_id.as_usize()).copied().flatten()
    }

    /// Return a child edge record by parent node id and child index.
    pub fn child_edge_record(&self, parent_id: NodeId, child_index: usize) -> Option<&EdgeRecord> {
        let parent = self.node_record(parent_id)?;
        self.edges
            .get(parent.edge_range().get(child_index)?)
            .filter(|edge| edge.parent_id == parent_id && edge.child_index == child_index)
    }

    /// Return the edge record connecting a parent to a child, if one exists.
    pub fn parent_edge_record(&self, child_id: NodeId) -> Option<&EdgeRecord> {
        let parent_id = self.parent_id(child_id)?;
        let parent = self.node_record(parent_id)?;
        self.edges[parent.edge_range().as_range()]
            .iter()
            .find(|edge| edge.child_id == child_id)
    }
}

/// Contiguous edge range owned by one [`NodeRecord`].
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct EdgeRange {
    start: usize,
    len: usize,
}

impl EdgeRange {
    /// Construct an edge range from a start index and edge count.
    pub fn new(start: usize, len: usize) -> Self {
        Self { start, len }
    }

    /// Return the first edge index in this range.
    pub fn start(self) -> usize {
        self.start
    }

    /// Return the number of edges in this range.
    pub fn len(self) -> usize {
        self.len
    }

    /// Return whether this range contains no edges.
    pub fn is_empty(self) -> bool {
        self.len == 0
    }

    /// Return the exclusive end edge index.
    pub fn end(self) -> usize {
        self.start + self.len
    }

    /// Return this edge range as a standard Rust range.
    pub fn as_range(self) -> Range<usize> {
        self.start()..self.end()
    }

    fn get(self, child_index: usize) -> Option<usize> {
        (child_index < self.len).then_some(self.start + child_index)
    }
}

/// Direct facts for one selected-tree node.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NodeRecord {
    pub(in crate::document) visible_id: SymbolId,
    pub(in crate::document) grammar_id: SymbolId,
    pub(in crate::document) byte_range: Range<usize>,
    pub(in crate::document) point_range: PointRange,
    pub(in crate::document) edge_range: EdgeRange,
    pub(in crate::document) alias_symbol_id: Option<SymbolId>,
    pub(in crate::document) flags: NodeFlags,
}

impl NodeRecord {
    /// Return the alias-visible kind id for this node.
    pub fn visible_id(&self) -> SymbolId {
        self.visible_id
    }

    /// Return the grammar kind id for this node, ignoring aliases.
    pub fn grammar_id(&self) -> SymbolId {
        self.grammar_id
    }

    /// Return the source byte range for this node.
    pub fn byte_range(&self) -> Range<usize> {
        self.byte_range.clone()
    }

    /// Return the zero-based point range for this node.
    pub fn point_range(&self) -> PointRange {
        self.point_range
    }

    /// Return this node's contiguous child-edge range.
    pub fn edge_range(&self) -> EdgeRange {
        self.edge_range
    }

    /// Return the alias symbol id applied to this node, if one exists.
    pub fn alias_symbol_id(&self) -> Option<SymbolId> {
        self.alias_symbol_id
    }

    /// Return direct structural flags for this node.
    pub fn flags(&self) -> NodeFlags {
        self.flags
    }
}

/// Direct facts for one selected-tree parent-to-child edge.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EdgeRecord {
    pub(in crate::document) parent_id: NodeId,
    pub(in crate::document) child_id: NodeId,
    pub(in crate::document) child_index: usize,
    pub(in crate::document) field_id: Option<FieldId>,
}

impl EdgeRecord {
    /// Return the parent node id for this edge.
    pub fn parent_id(&self) -> NodeId {
        self.parent_id
    }

    /// Return the child node id for this edge.
    pub fn child_id(&self) -> NodeId {
        self.child_id
    }

    /// Return the child index within the parent.
    pub fn child_index(&self) -> usize {
        self.child_index
    }

    /// Return the public field id attached to this edge, if any.
    pub fn field_id(&self) -> Option<FieldId> {
        self.field_id
    }
}

/// Native language metadata attached to a parse document.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LanguageMetadata {
    name: String,
    symbols: Vec<NodeKind>,
    fields: Vec<String>,
}

impl LanguageMetadata {
    pub(crate) fn from_runtime(
        language_name: &str,
        grammar: &Grammar,
        parse_table: &ParseTable,
    ) -> Self {
        let mut symbols = Vec::new();

        for metadata in &parse_table.symbol_metadata {
            insert_symbol(&mut symbols, NodeKind::from_table_metadata(metadata));
        }

        for (symbol_id, name) in &grammar.rule_names {
            if !symbols.iter().any(|symbol| symbol.symbol_id == *symbol_id) {
                let is_terminal = grammar.tokens.contains_key(symbol_id);
                insert_symbol(
                    &mut symbols,
                    NodeKind {
                        symbol_id: *symbol_id,
                        name: name.clone(),
                        is_visible: !name.starts_with('_'),
                        is_named: !is_terminal,
                        is_supertype: grammar.supertypes.contains(symbol_id),
                        is_terminal,
                        is_extra: grammar.extras.contains(symbol_id),
                    },
                );
            }
        }

        for (symbol_id, token) in &grammar.tokens {
            if !symbols.iter().any(|symbol| symbol.symbol_id == *symbol_id) {
                insert_symbol(
                    &mut symbols,
                    NodeKind {
                        symbol_id: *symbol_id,
                        name: token.name.clone(),
                        is_visible: !token.name.starts_with('_'),
                        is_named: false,
                        is_supertype: grammar.supertypes.contains(symbol_id),
                        is_terminal: true,
                        is_extra: grammar.extras.contains(symbol_id),
                    },
                );
            }
        }

        symbols.sort_by_key(|symbol| symbol.symbol_id.0);

        Self {
            name: language_name.to_string(),
            symbols,
            fields: parse_table.field_names.clone(),
        }
    }

    /// Return the language name used to create this document.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Return all known node kinds for this language.
    pub fn symbols(&self) -> &[NodeKind] {
        &self.symbols
    }

    /// Return metadata for a symbol id.
    pub fn symbol(&self, symbol_id: SymbolId) -> Option<&NodeKind> {
        self.symbols
            .iter()
            .find(|symbol| symbol.symbol_id == symbol_id)
    }

    /// Return the display name for a symbol id.
    pub fn symbol_name(&self, symbol_id: SymbolId) -> Option<&str> {
        self.symbol(symbol_id).map(NodeKind::name)
    }

    /// Return the number of public fields in this language.
    pub fn field_count(&self) -> usize {
        self.fields.len()
    }

    /// Return all public field names in their zero-based table order.
    ///
    /// Public field IDs are one-based, so `fields()[0]` corresponds to
    /// [`field_name_for_id(1)`](Self::field_name_for_id).
    pub fn fields(&self) -> &[String] {
        &self.fields
    }

    /// Return a field name for a nonzero public field id.
    pub fn field_name_for_id(&self, field_id: u16) -> Option<&str> {
        let index = field_id.checked_sub(1)? as usize;
        self.fields.get(index).map(String::as_str)
    }

    /// Return the nonzero public field id for a field name.
    pub fn field_id_for_name(&self, field_name: impl AsRef<[u8]>) -> Option<FieldId> {
        let field_name = field_name.as_ref();
        self.fields
            .iter()
            .position(|candidate| candidate.as_bytes() == field_name)
            .and_then(|index| {
                let field_id = u16::try_from(index.checked_add(1)?).ok()?;
                FieldId::new(field_id)
            })
    }
}

/// Native metadata for one grammar symbol.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NodeKind {
    pub(in crate::document) symbol_id: SymbolId,
    name: String,
    is_visible: bool,
    is_named: bool,
    is_supertype: bool,
    is_terminal: bool,
    is_extra: bool,
}

impl NodeKind {
    fn from_table_metadata(metadata: &TableSymbolMetadata) -> Self {
        Self {
            symbol_id: metadata.symbol_id,
            name: metadata.name.clone(),
            is_visible: metadata.is_visible,
            is_named: metadata.is_named,
            is_supertype: metadata.is_supertype,
            is_terminal: metadata.is_terminal,
            is_extra: metadata.is_extra,
        }
    }

    /// Return the symbol id for this node kind.
    pub fn symbol_id(&self) -> SymbolId {
        self.symbol_id
    }

    /// Return the display name for this node kind.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Return whether this node kind is visible in syntax output.
    pub fn is_visible(&self) -> bool {
        self.is_visible
    }

    /// Return whether this node kind is named.
    pub fn is_named(&self) -> bool {
        self.is_named
    }

    /// Return whether this node kind is a supertype.
    pub fn is_supertype(&self) -> bool {
        self.is_supertype
    }

    /// Return whether this node kind is a terminal token.
    pub fn is_terminal(&self) -> bool {
        self.is_terminal
    }

    /// Return whether this node kind is extra syntax such as trivia.
    pub fn is_extra(&self) -> bool {
        self.is_extra
    }
}

/// Native identity for one document node.
///
/// Tree-sitter-compatible output distinguishes visible identity from grammar
/// identity. The alpha document keeps those identities separate so production
/// aliases can populate visible identity without forcing compatibility adapters
/// to infer aliases from grammar names.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NodeIdentity<'doc> {
    pub(in crate::document) visible_id: SymbolId,
    pub(in crate::document) grammar_id: SymbolId,
    pub(in crate::document) visible_name: Option<&'doc str>,
    pub(in crate::document) grammar_name: Option<&'doc str>,
    pub(in crate::document) alias_symbol_id: Option<SymbolId>,
    pub(in crate::document) visible_is_named: bool,
    pub(in crate::document) grammar_is_named: bool,
}

impl<'doc> NodeIdentity<'doc> {
    /// Return the alias-visible symbol id for this node.
    ///
    /// This differs from [`grammar_id`](Self::grammar_id) when production alias
    /// metadata changes the node's visible identity.
    pub fn visible_id(&self) -> SymbolId {
        self.visible_id
    }

    /// Return the alias-visible node name for this node.
    ///
    /// This differs from [`grammar_name`](Self::grammar_name) when production
    /// alias metadata changes the node's visible identity.
    pub fn visible_name(&self) -> Option<&'doc str> {
        self.visible_name
    }

    /// Return the original grammar symbol id for this node.
    pub fn grammar_id(&self) -> SymbolId {
        self.grammar_id
    }

    /// Return the original grammar symbol name for this node.
    pub fn grammar_name(&self) -> Option<&'doc str> {
        self.grammar_name
    }

    /// Return the alias symbol id applied to this node, when one is known.
    pub fn alias_symbol_id(&self) -> Option<SymbolId> {
        self.alias_symbol_id
    }

    /// Return whether this node has an alias entry or alias-visible identity
    /// distinct from its grammar identity.
    pub fn has_alias(&self) -> bool {
        self.alias_symbol_id.is_some()
            || self.visible_id != self.grammar_id
            || self.visible_name != self.grammar_name
            || self.visible_is_named != self.grammar_is_named
    }

    /// Return whether the alias-visible identity is named.
    pub fn visible_is_named(&self) -> bool {
        self.visible_is_named
    }

    /// Return whether the original grammar identity is named.
    pub fn grammar_is_named(&self) -> bool {
        self.grammar_is_named
    }
}

/// Native structural flags for one document node.
///
/// These flags are computed from the selected document tree and language
/// metadata. They are the native source for generated typed CST wrappers and
/// Tree-sitter-compatible projections that need named/extra/error/missing
/// behavior without inventing local adapter state.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct NodeFlags {
    pub(in crate::document) named: bool,
    pub(in crate::document) visible: bool,
    pub(in crate::document) extra: bool,
    pub(in crate::document) terminal: bool,
    pub(in crate::document) supertype: bool,
    pub(in crate::document) error: bool,
    pub(in crate::document) missing: bool,
    pub(in crate::document) has_error: bool,
}

impl NodeFlags {
    /// Return whether the node is named in visible syntax.
    pub fn is_named(&self) -> bool {
        self.named
    }

    /// Return whether the node is visible in syntax output.
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Return whether the node is extra syntax such as trivia.
    pub fn is_extra(&self) -> bool {
        self.extra
    }

    /// Return whether the node is a terminal token.
    pub fn is_terminal(&self) -> bool {
        self.terminal
    }

    /// Return whether the node is a supertype.
    pub fn is_supertype(&self) -> bool {
        self.supertype
    }

    /// Return whether the node is a node-local synthetic error.
    pub fn is_error(&self) -> bool {
        self.error
    }

    /// Return whether the node is a zero-width synthetic missing node.
    pub fn is_missing(&self) -> bool {
        self.missing
    }

    /// Return whether this node or its descendants carry error state.
    pub fn has_error(&self) -> bool {
        self.has_error
    }
}
