//! Native parse document alpha.
//!
//! This module is the first implementation slice of the `AdzeDocument`
//! contract. It intentionally exposes only the current parser tree, source
//! text, basic diagnostics, and metadata. Richer projections such as full typed
//! CST generation and GLR forest summaries are future slices.

use crate::parser_v4::ParseNode;
use adze_glr_core::{ParseTable, SymbolMetadata as TableSymbolMetadata};
use adze_ir::{Grammar, SymbolId};
use std::ffi::CStr;
use std::num::NonZeroU16;
use std::ops::Range;

/// Nonzero public field identifier used by native document field metadata.
///
/// This matches Tree-sitter's public field-id convention: ID 0 is the
/// sentinel and real field names start at 1.
pub type FieldId = NonZeroU16;

/// Experimental schema identifier for native document JSON output.
#[cfg(feature = "serialization")]
pub const ADZE_DOCUMENT_JSON_SCHEMA: &str = "adze.document.v1";

/// A native parse-product document.
///
/// `AdzeDocument` owns the source text and the parser's selected concrete
/// syntax tree. Additional views should project from this document instead of
/// reparsing or constructing a separate parse truth.
#[derive(Clone, Debug)]
pub struct AdzeDocument {
    source: String,
    root: ParseNode,
    node_index: Vec<NodeIndex>,
    syntax: SyntaxTree,
    language: LanguageMetadata,
    diagnostics: Vec<ParseDiagnostic>,
    ambiguities: Vec<crate::glr_parser::AmbiguitySummary>,
    metadata: ParseMetadata,
    pure_language: Option<&'static crate::pure_parser::TSLanguage>,
}

impl AdzeDocument {
    pub(crate) fn from_parse_result(
        source: &str,
        root: ParseNode,
        error_count: usize,
        language_name: &str,
        grammar: &Grammar,
        parse_table: &ParseTable,
    ) -> Self {
        let diagnostics = build_diagnostics(&root, error_count, source);
        let runtime = DocumentRuntime {
            language_name,
            grammar,
            parse_table,
            pure_language: None,
        };
        Self::from_parse_result_with_diagnostics(source, root, error_count, runtime, diagnostics)
    }

    pub(crate) fn from_parse_result_with_diagnostics(
        source: &str,
        root: ParseNode,
        error_count: usize,
        runtime: DocumentRuntime<'_>,
        diagnostics: Vec<ParseDiagnostic>,
    ) -> Self {
        Self::from_parse_result_with_diagnostics_and_ambiguities(
            source,
            root,
            error_count,
            runtime,
            diagnostics,
            Vec::new(),
        )
    }

    pub(crate) fn from_parse_result_with_diagnostics_and_ambiguities(
        source: &str,
        root: ParseNode,
        error_count: usize,
        runtime: DocumentRuntime<'_>,
        diagnostics: Vec<ParseDiagnostic>,
        ambiguities: Vec<crate::glr_parser::AmbiguitySummary>,
    ) -> Self {
        let node_index = build_node_index(&root);
        let language = LanguageMetadata::from_runtime(
            runtime.language_name,
            runtime.grammar,
            runtime.parse_table,
        );
        let syntax =
            SyntaxTree::from_parse_root(&root, source, &language, error_count, &node_index);
        let mut diagnostics = diagnostics;
        attach_related_nodes(&root, &mut diagnostics);
        Self {
            source: source.to_string(),
            root,
            node_index,
            syntax,
            language,
            diagnostics,
            ambiguities,
            metadata: ParseMetadata { error_count },
            pure_language: runtime.pure_language,
        }
    }

    /// Return the generic native CST view.
    pub fn tree(&self) -> AdzeTree<'_> {
        AdzeTree { document: self }
    }

    /// Return language metadata recorded for this document.
    pub fn language(&self) -> &LanguageMetadata {
        &self.language
    }

    /// Return structured diagnostics recorded for this parse.
    pub fn diagnostics(&self) -> &[ParseDiagnostic] {
        &self.diagnostics
    }

    /// Return GLR ambiguity summaries recorded for this document.
    ///
    /// The alpha document path records summaries only when generated
    /// `parse_document()` routes through the true-GLR runtime. Conflict-free
    /// documents return an empty slice.
    pub fn ambiguities(&self) -> &[crate::glr_parser::AmbiguitySummary] {
        &self.ambiguities
    }

    /// Return parse metadata recorded for this document.
    pub fn metadata(&self) -> &ParseMetadata {
        &self.metadata
    }

    /// Return the original source text.
    pub fn source_text(&self) -> &str {
        &self.source
    }

    /// Return the original source bytes.
    pub fn source_bytes(&self) -> &[u8] {
        self.source.as_bytes()
    }

    /// Return a UTF-8 source slice for a byte range.
    ///
    /// Returns `None` if the range is outside the document source or does not
    /// align to UTF-8 character boundaries.
    pub fn source_slice(&self, range: Range<usize>) -> Option<&str> {
        self.source.get(range)
    }

    /// Return a schema-versioned JSON value for this native document.
    ///
    /// This experimental projection is intended for canaries and future CLI or
    /// WASM output work. It serializes the selected generic CST, diagnostics,
    /// ambiguity summaries, and metadata from the same document; it is not a
    /// stable `adze-json` contract.
    #[cfg(feature = "serialization")]
    pub fn to_json_value(&self) -> serde_json::Value {
        serde_json::json!({
            "schema": ADZE_DOCUMENT_JSON_SCHEMA,
            "source": {
                "byte_len": self.source_bytes().len(),
            },
            "language": {
                "name": self.language().name(),
            },
            "metadata": {
                "error_count": self.metadata().error_count,
            },
            "diagnostics": self
                .diagnostics()
                .iter()
                .map(diagnostic_to_json)
                .collect::<Vec<_>>(),
            "ambiguities": self
                .ambiguities()
                .iter()
                .map(ambiguity_to_json)
                .collect::<Vec<_>>(),
            "tree": {
                "root": node_to_document_json(self.tree().root()),
            },
        })
    }

    /// Return diagnostics directly related to a document-local node id.
    pub fn diagnostics_for_node(
        &self,
        node_id: NodeId,
    ) -> impl Iterator<Item = &ParseDiagnostic> + '_ {
        self.diagnostics
            .iter()
            .filter(move |diagnostic| diagnostic.related_nodes.contains(&node_id))
    }

    /// Extract a typed AST from this document's selected tree.
    ///
    /// This alpha view is available for generated pure-Rust documents, where
    /// the document retains the generated language metadata needed by
    /// [`Extract`](crate::Extract). Documents with parser diagnostics return
    /// those diagnostics as parse errors instead of extracting from recovered
    /// syntax.
    pub fn ast<T>(&self) -> Result<T, Vec<crate::errors::ParseError>>
    where
        T: crate::Extract<T>,
    {
        self.ast_with_provenance().map(TypedAst::into_value)
    }

    /// Extract a typed AST and record the document syntax it came from.
    ///
    /// This is an alpha document-level provenance view. It records the selected
    /// document node used as the typed AST extraction root; it does not yet
    /// attempt per-AST-node provenance.
    pub fn ast_with_provenance<T>(&self) -> Result<TypedAst<T>, Vec<crate::errors::ParseError>>
    where
        T: crate::Extract<T>,
    {
        let language = self.typed_ast_language()?;

        let parsed_root = document_node_to_parsed_node(&self.root, language, self.source_bytes());
        let extraction_target = self.typed_ast_extraction_target();
        let extract_node = extraction_target
            .child_index
            .and_then(|child_index| parsed_root.children.get(child_index))
            .unwrap_or(&parsed_root);

        let value =
            <T as crate::Extract<_>>::extract(Some(extract_node), self.source_bytes(), 0, None);

        Ok(TypedAst {
            value,
            provenance: extraction_target.provenance,
        })
    }

    /// Extract a typed AST from a specific document node.
    ///
    /// This supports typed CST handles as cheap syntax views: generated wrappers
    /// validate their node kind, then ask the backing document to extract the
    /// semantic value from that exact node. The returned provenance records the
    /// supplied node id. Documents with parser diagnostics return those
    /// diagnostics as parse errors instead of extracting from recovered syntax.
    pub fn ast_from_node<T>(
        &self,
        node_id: NodeId,
    ) -> Result<TypedAst<T>, Vec<crate::errors::ParseError>>
    where
        T: crate::Extract<T>,
    {
        let language = self.typed_ast_language()?;
        let Some(node) = self.node_by_id(node_id) else {
            return Err(vec![crate::errors::ParseError {
                reason: crate::errors::ParseErrorReason::UnexpectedToken(format!(
                    "typed AST extraction node {} does not exist in document",
                    node_id.as_usize()
                )),
                start: 0,
                end: 0,
                expected: Vec::new(),
            }]);
        };

        let parsed_node = document_node_to_parsed_node(node, language, self.source_bytes());
        let value =
            <T as crate::Extract<_>>::extract(Some(&parsed_node), self.source_bytes(), 0, None);

        Ok(TypedAst {
            value,
            provenance: Provenance::Node(node_id),
        })
    }

    #[cfg(feature = "ts-compat")]
    pub(crate) fn root_parse_node(&self) -> &ParseNode {
        &self.root
    }

    fn node_by_id(&self, node_id: NodeId) -> Option<&ParseNode> {
        let index = self.node_index.get(node_id.as_usize())?;
        let mut node = &self.root;

        for &child_index in &index.path {
            node = node.children.get(child_index)?;
        }

        Some(node)
    }

    fn typed_ast_extraction_target(&self) -> TypedAstExtractionTarget {
        let root = self.tree().root();
        if root.kind_name() != Some("source_file") {
            return TypedAstExtractionTarget {
                child_index: None,
                provenance: Provenance::Node(root.node_id()),
            };
        }

        let non_extra_children: Vec<_> = root
            .child_edges()
            .filter_map(|edge| {
                let child = edge.child()?;
                (!child.is_extra()).then_some((edge.child_index(), child.node_id()))
            })
            .collect();

        if let [(child_index, child_id)] = non_extra_children.as_slice() {
            return TypedAstExtractionTarget {
                child_index: Some(*child_index),
                provenance: Provenance::Node(*child_id),
            };
        }

        TypedAstExtractionTarget {
            child_index: None,
            provenance: Provenance::Node(root.node_id()),
        }
    }

    fn typed_ast_language(
        &self,
    ) -> Result<&'static crate::pure_parser::TSLanguage, Vec<crate::errors::ParseError>> {
        if !self.diagnostics.is_empty() {
            return Err(self
                .diagnostics
                .iter()
                .map(ParseDiagnostic::to_parse_error)
                .collect());
        }

        self.pure_language.ok_or_else(|| {
            vec![crate::errors::ParseError {
                reason: crate::errors::ParseErrorReason::UnexpectedToken(
                    "typed AST extraction requires generated pure-Rust language metadata"
                        .to_string(),
                ),
                start: 0,
                end: 0,
                expected: Vec::new(),
            }]
        })
    }
}

pub(crate) struct DocumentRuntime<'a> {
    pub(crate) language_name: &'a str,
    pub(crate) grammar: &'a Grammar,
    pub(crate) parse_table: &'a ParseTable,
    pub(crate) pure_language: Option<&'static crate::pure_parser::TSLanguage>,
}

/// A typed AST value paired with document-level syntax provenance.
///
/// The provenance describes the selected document syntax used as the extraction
/// root. It is intentionally coarser than per-AST-node provenance, which needs
/// a separate contract because semantic AST values may combine, omit, or
/// synthesize concrete syntax.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypedAst<T> {
    value: T,
    provenance: Provenance,
}

impl<T> TypedAst<T> {
    /// Return the extracted typed AST value.
    pub fn value(&self) -> &T {
        &self.value
    }

    /// Return document-level syntax provenance for this typed AST.
    pub fn provenance(&self) -> &Provenance {
        &self.provenance
    }

    /// Consume this wrapper and return the typed AST value.
    pub fn into_value(self) -> T {
        self.value
    }

    /// Consume this wrapper and return both value and provenance.
    pub fn into_parts(self) -> (T, Provenance) {
        (self.value, self.provenance)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TypedAstExtractionTarget {
    child_index: Option<usize>,
    provenance: Provenance,
}

/// Syntax provenance for a typed AST projection.
///
/// The alpha implementation currently records [`Provenance::Node`] for the
/// extraction root. The other variants define the intended shape for future
/// semantic AST values that are span-based, combine multiple syntax nodes, or
/// are synthesized from recovery/defaulting.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Provenance {
    /// The value came from one document-local node.
    Node(NodeId),
    /// The value came from a source byte span without a single owning node.
    Span(Range<usize>),
    /// The value came from multiple document-local nodes.
    Nodes(Vec<NodeId>),
    /// The value was synthesized by extraction or recovery.
    Synthetic {
        /// Source byte span associated with the synthetic value.
        span: Range<usize>,
        /// Reason the value was synthesized.
        reason: SyntheticReason,
    },
}

/// Reason a provenance entry was synthesized.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SyntheticReason {
    /// Parser recovery supplied missing syntax.
    MissingSyntax,
    /// Typed extraction supplied a default value.
    ExtractionDefault,
    /// The value was generated by a parser or runtime fallback.
    RuntimeFallback,
}

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
struct NodeIndex {
    path: Vec<usize>,
    parent_id: Option<NodeId>,
    child_ids: Vec<NodeId>,
}

/// Direct selected-tree storage for an [`AdzeDocument`].
///
/// This is the alpha v2 storage layer behind the borrowed [`AdzeTree`] view.
/// It records document-local node and edge facts directly so future typed CST,
/// diagnostics, JSON, and Tree-sitter compatibility projections do not need to
/// infer structure from recursive tree path replay.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxTree {
    root_id: NodeId,
    nodes: Vec<NodeRecord>,
    edges: Vec<EdgeRecord>,
    parents: Vec<Option<NodeId>>,
}

impl SyntaxTree {
    fn from_parse_root(
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
    visible_id: SymbolId,
    grammar_id: SymbolId,
    byte_range: Range<usize>,
    point_range: PointRange,
    edge_range: EdgeRange,
    alias_symbol_id: Option<SymbolId>,
    flags: NodeFlags,
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
    parent_id: NodeId,
    child_id: NodeId,
    child_index: usize,
    field_id: Option<FieldId>,
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
    symbol_id: SymbolId,
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
    visible_id: SymbolId,
    grammar_id: SymbolId,
    visible_name: Option<&'doc str>,
    grammar_name: Option<&'doc str>,
    alias_symbol_id: Option<SymbolId>,
    visible_is_named: bool,
    grammar_is_named: bool,
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
    named: bool,
    visible: bool,
    extra: bool,
    terminal: bool,
    supertype: bool,
    error: bool,
    missing: bool,
    has_error: bool,
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

/// Basic parse metadata for a native document.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParseMetadata {
    /// Number of parser recovery/error events recorded for this parse.
    pub error_count: usize,
}

/// A structured parse diagnostic attached to a native document.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParseDiagnostic {
    /// Byte offset where the diagnostic begins.
    pub start_byte: usize,
    /// Byte offset where the diagnostic ends.
    pub end_byte: usize,
    /// Zero-based row/column range covered by the diagnostic.
    pub point_range: PointRange,
    /// Human-readable found token or symbol name, when known.
    pub found: Option<String>,
    /// Human-readable expected token or symbol names, when known.
    pub expected: Vec<String>,
    /// Document-local nodes related to this diagnostic.
    pub related_nodes: Vec<NodeId>,
    /// Human-readable diagnostic summary.
    pub message: String,
}

impl ParseDiagnostic {
    /// Return the byte span covered by this diagnostic.
    #[must_use]
    pub fn byte_span(&self) -> Range<usize> {
        self.start_byte..self.end_byte
    }

    /// Return a formatter that includes source location and context.
    #[must_use]
    pub fn display_with_source<'a>(&'a self, source: &'a str) -> ParseDiagnosticWithSource<'a> {
        ParseDiagnosticWithSource {
            diagnostic: self,
            source,
        }
    }

    fn to_parse_error(&self) -> crate::errors::ParseError {
        crate::errors::ParseError {
            reason: crate::errors::ParseErrorReason::UnexpectedToken(self.message.clone()),
            start: self.start_byte,
            end: self.end_byte,
            expected: self.expected.clone(),
        }
    }
}

impl std::fmt::Display for ParseDiagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} at {}:{} (bytes {}..{})",
            self.message,
            self.point_range.start.row + 1,
            self.point_range.start.column + 1,
            self.start_byte,
            self.end_byte
        )
    }
}

/// Display helper returned by [`ParseDiagnostic::display_with_source`].
pub struct ParseDiagnosticWithSource<'a> {
    diagnostic: &'a ParseDiagnostic,
    source: &'a str,
}

impl std::fmt::Display for ParseDiagnosticWithSource<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.diagnostic)?;

        if let Some(line) = source_line(self.source, self.diagnostic.start_byte) {
            let range = self.diagnostic.point_range;
            let marker_width = if range.start.row == range.end.row {
                range.end.column.saturating_sub(range.start.column).max(1)
            } else {
                1
            };
            let marker =
                " ".repeat(range.start.column as usize) + &"^".repeat(marker_width as usize);
            write!(f, "\n{line}\n{marker}")?;
        }

        Ok(())
    }
}

/// A zero-based source point in a native parse document.
///
/// Columns are byte offsets within a row, matching Tree-sitter's point model.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DocumentPoint {
    /// Zero-based row number.
    pub row: u32,
    /// Zero-based byte column within the row.
    pub column: u32,
}

impl DocumentPoint {
    /// Compute a document point from a byte offset.
    ///
    /// Out-of-range byte offsets are clamped to the end of `source`.
    #[must_use]
    pub fn from_byte_offset(source: &str, byte: usize) -> Self {
        let end = byte.min(source.len());
        let mut row = 0u32;
        let mut column = 0u32;

        for &source_byte in &source.as_bytes()[..end] {
            if source_byte == b'\n' {
                row = row.saturating_add(1);
                column = 0;
            } else {
                column = column.saturating_add(1);
            }
        }

        Self { row, column }
    }
}

/// A zero-based source point range in a native parse document.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PointRange {
    /// Inclusive start point.
    pub start: DocumentPoint,
    /// Exclusive end point.
    pub end: DocumentPoint,
}

impl PointRange {
    /// Compute a point range from a byte range.
    #[must_use]
    pub fn from_byte_range(source: &str, range: Range<usize>) -> Self {
        Self {
            start: DocumentPoint::from_byte_offset(source, range.start),
            end: DocumentPoint::from_byte_offset(source, range.end),
        }
    }
}

/// Borrowed generic CST view for an [`AdzeDocument`].
#[derive(Clone, Copy, Debug)]
pub struct AdzeTree<'doc> {
    document: &'doc AdzeDocument,
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
    document: &'doc AdzeDocument,
    node: &'doc ParseNode,
    id: NodeId,
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
    document: &'doc AdzeDocument,
    record: &'doc EdgeRecord,
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

#[cfg(feature = "serialization")]
fn diagnostic_to_json(diagnostic: &ParseDiagnostic) -> serde_json::Value {
    serde_json::json!({
        "start_byte": diagnostic.start_byte,
        "end_byte": diagnostic.end_byte,
        "point_range": point_range_to_json(diagnostic.point_range),
        "found": diagnostic.found.as_deref(),
        "expected": &diagnostic.expected,
        "related_nodes": diagnostic
            .related_nodes
            .iter()
            .map(|node_id| node_id.as_usize())
            .collect::<Vec<_>>(),
        "message": diagnostic.message.as_str(),
    })
}

#[cfg(feature = "serialization")]
fn ambiguity_to_json(ambiguity: &crate::glr_parser::AmbiguitySummary) -> serde_json::Value {
    serde_json::json!({
        "span": {
            "start_byte": ambiguity.span.start,
            "end_byte": ambiguity.span.end,
        },
        "selected": ambiguity.selected,
        "selection_reason": format!("{:?}", ambiguity.selection_reason),
        "alternatives": ambiguity
            .alternatives
            .iter()
            .map(alternative_to_json)
            .collect::<Vec<_>>(),
    })
}

#[cfg(feature = "serialization")]
fn alternative_to_json(alternative: &crate::glr_parser::AlternativeSummary) -> serde_json::Value {
    serde_json::json!({
        "index": alternative.index,
        "root_symbol": alternative.root_symbol.0,
        "span": {
            "start_byte": alternative.span.start,
            "end_byte": alternative.span.end,
        },
        "dynamic_precedence": alternative.dynamic_precedence,
        "in_error": alternative.in_error,
        "cost": alternative.cost,
        "node_count": alternative.node_count,
    })
}

#[cfg(feature = "serialization")]
fn node_to_document_json(node: AdzeNode<'_>) -> serde_json::Value {
    let identity = node.identity();
    let flags = node.flags();
    let text = if node.child_count() == 0 {
        node.utf8_text().ok()
    } else {
        None
    };
    let children = node
        .child_edges()
        .filter_map(|edge| {
            edge.child().map(|child| {
                serde_json::json!({
                    "child_index": edge.child_index(),
                    "field_name": edge.field_name(),
                    "field_id": edge.field_id().map(|field_id| field_id.get()),
                    "node": node_to_document_json(child),
                })
            })
        })
        .collect::<Vec<_>>();

    serde_json::json!({
        "id": node.node_id().as_usize(),
        "kind": identity.visible_name(),
        "kind_id": identity.visible_id().0,
        "grammar_kind": identity.grammar_name(),
        "grammar_id": identity.grammar_id().0,
        "alias_symbol_id": identity.alias_symbol_id().map(|symbol_id| symbol_id.0),
        "has_alias": identity.has_alias(),
        "range": {
            "start_byte": node.start_byte(),
            "end_byte": node.end_byte(),
            "start_point": point_to_json(node.point_range().start),
            "end_point": point_to_json(node.point_range().end),
        },
        "flags": {
            "named": flags.is_named(),
            "visible": flags.is_visible(),
            "extra": flags.is_extra(),
            "terminal": flags.is_terminal(),
            "supertype": flags.is_supertype(),
            "error": flags.is_error(),
            "missing": flags.is_missing(),
            "has_error": flags.has_error(),
        },
        "text": text,
        "children": children,
    })
}

#[cfg(feature = "serialization")]
fn point_range_to_json(range: PointRange) -> serde_json::Value {
    serde_json::json!({
        "start": point_to_json(range.start),
        "end": point_to_json(range.end),
    })
}

#[cfg(feature = "serialization")]
fn point_to_json(point: DocumentPoint) -> serde_json::Value {
    serde_json::json!({
        "row": point.row,
        "column": point.column,
    })
}

fn build_diagnostics(root: &ParseNode, error_count: usize, source: &str) -> Vec<ParseDiagnostic> {
    if error_count == 0 {
        return Vec::new();
    }

    let span = first_error_span(root).unwrap_or(root.start_byte..root.end_byte);
    let start_byte = span.start.min(source.len());
    let end_byte = span.end.min(source.len()).max(start_byte);
    let point_range = PointRange::from_byte_range(source, start_byte..end_byte);

    vec![ParseDiagnostic {
        start_byte,
        end_byte,
        point_range,
        found: None,
        expected: Vec::new(),
        related_nodes: Vec::new(),
        message: format!("parser recorded {error_count} recovery/error event(s)"),
    }]
}

fn attach_related_nodes(root: &ParseNode, diagnostics: &mut [ParseDiagnostic]) {
    for diagnostic in diagnostics {
        diagnostic.related_nodes = related_nodes_for_diagnostic(root, diagnostic);
    }
}

fn related_nodes_for_diagnostic(root: &ParseNode, diagnostic: &ParseDiagnostic) -> Vec<NodeId> {
    let mut related_errors = Vec::new();
    let mut next_id = 0;
    collect_related_error_nodes(root, diagnostic, &mut next_id, &mut related_errors);
    if !related_errors.is_empty() {
        return related_errors;
    }

    let mut best = None;
    let mut next_id = 0;
    collect_smallest_covering_node(root, diagnostic, &mut next_id, &mut best);
    best.map(|(node_id, _)| vec![node_id]).unwrap_or_default()
}

fn collect_related_error_nodes(
    node: &ParseNode,
    diagnostic: &ParseDiagnostic,
    next_id: &mut usize,
    related: &mut Vec<NodeId>,
) {
    let node_id = NodeId::new(*next_id);
    *next_id += 1;

    if is_error_parse_node(node) && node_range_touches_diagnostic(node, diagnostic) {
        related.push(node_id);
    }

    for child in &node.children {
        collect_related_error_nodes(child, diagnostic, next_id, related);
    }
}

fn collect_smallest_covering_node(
    node: &ParseNode,
    diagnostic: &ParseDiagnostic,
    next_id: &mut usize,
    best: &mut Option<(NodeId, usize)>,
) {
    let node_id = NodeId::new(*next_id);
    *next_id += 1;

    if node_covers_diagnostic(node, diagnostic) {
        let width = node.end_byte.saturating_sub(node.start_byte);
        if best
            .map(|(_, best_width)| width < best_width)
            .unwrap_or(true)
        {
            *best = Some((node_id, width));
        }
    }

    for child in &node.children {
        collect_smallest_covering_node(child, diagnostic, next_id, best);
    }
}

fn is_error_parse_node(node: &ParseNode) -> bool {
    node.symbol.0 == 0 && node.children.is_empty()
}

fn node_range_touches_diagnostic(node: &ParseNode, diagnostic: &ParseDiagnostic) -> bool {
    if diagnostic.start_byte == diagnostic.end_byte {
        node.start_byte <= diagnostic.start_byte && diagnostic.start_byte <= node.end_byte
    } else {
        node.start_byte < diagnostic.end_byte && diagnostic.start_byte < node.end_byte
    }
}

fn node_covers_diagnostic(node: &ParseNode, diagnostic: &ParseDiagnostic) -> bool {
    if diagnostic.start_byte == diagnostic.end_byte {
        node.start_byte <= diagnostic.start_byte && diagnostic.start_byte <= node.end_byte
    } else {
        node.start_byte <= diagnostic.start_byte && diagnostic.end_byte <= node.end_byte
    }
}

fn first_error_span(node: &ParseNode) -> Option<Range<usize>> {
    if node.symbol.0 == 0 && node.children.is_empty() {
        return Some(node.start_byte..node.end_byte);
    }

    node.children.iter().find_map(first_error_span)
}

fn source_line(source: &str, byte_offset: usize) -> Option<&str> {
    if source.is_empty() {
        return None;
    }

    let bytes = source.as_bytes();
    let offset = byte_offset.min(bytes.len());
    let mut start = offset;
    while start > 0 && bytes[start - 1] != b'\n' && bytes[start - 1] != b'\r' {
        start -= 1;
    }

    let mut end = offset;
    while end < bytes.len() && bytes[end] != b'\n' && bytes[end] != b'\r' {
        end += 1;
    }

    source.get(start..end)
}

fn build_node_index(root: &ParseNode) -> Vec<NodeIndex> {
    let mut index = Vec::new();
    let mut path = Vec::new();
    collect_node_index(root, &mut path, &mut index);
    index
}

fn collect_node_records(
    node: &ParseNode,
    source: &str,
    language: &LanguageMetadata,
    error_count: usize,
    is_root: bool,
    nodes: &mut Vec<NodeRecord>,
) -> bool {
    let id = NodeId::new(nodes.len());
    let grammar_id = node.symbol_id;
    let alias_symbol_id = node.alias_symbol_id;
    let visible_id = alias_symbol_id.unwrap_or(grammar_id);

    nodes.push(NodeRecord {
        visible_id,
        grammar_id,
        byte_range: node.start_byte..node.end_byte,
        point_range: PointRange::from_byte_range(source, node.start_byte..node.end_byte),
        edge_range: EdgeRange::default(),
        alias_symbol_id,
        flags: NodeFlags::default(),
    });

    let mut child_has_error = false;
    for child in &node.children {
        child_has_error |= collect_node_records(child, source, language, error_count, false, nodes);
    }

    let error = is_error_parse_node(node);
    let missing = node.start_byte == node.end_byte && error;
    let has_error = error || child_has_error || (is_root && error_count > 0);
    let kind = language.symbol(visible_id);
    nodes[id.as_usize()].flags = NodeFlags {
        named: kind.map(NodeKind::is_named).unwrap_or(false),
        visible: kind.map(NodeKind::is_visible).unwrap_or(false),
        extra: kind.map(NodeKind::is_extra).unwrap_or(false),
        terminal: kind.map(NodeKind::is_terminal).unwrap_or(false),
        supertype: kind.map(NodeKind::is_supertype).unwrap_or(false),
        error,
        missing,
        has_error,
    };

    has_error
}

fn collect_edge_records(
    node: &ParseNode,
    node_id: NodeId,
    node_index: &[NodeIndex],
    language: &LanguageMetadata,
    tree: &mut SyntaxTree,
) {
    let edge_start = tree.edges.len();
    let child_ids = node_index
        .get(node_id.as_usize())
        .map(|index| index.child_ids.as_slice())
        .unwrap_or(&[]);

    for (child_index, child) in node.children.iter().enumerate() {
        let Some(child_id) = child_ids.get(child_index).copied() else {
            continue;
        };
        tree.edges.push(EdgeRecord {
            parent_id: node_id,
            child_id,
            child_index,
            field_id: child
                .field_name
                .as_deref()
                .and_then(|field_name| language.field_id_for_name(field_name)),
        });
    }

    let edge_len = tree.edges.len().saturating_sub(edge_start);
    if let Some(record) = tree.nodes.get_mut(node_id.as_usize()) {
        record.edge_range = EdgeRange::new(edge_start, edge_len);
    }

    for (child_index, child) in node.children.iter().enumerate() {
        let Some(child_id) = child_ids.get(child_index).copied() else {
            continue;
        };
        collect_edge_records(child, child_id, node_index, language, tree);
    }
}

fn collect_node_index(
    node: &ParseNode,
    path: &mut Vec<usize>,
    index: &mut Vec<NodeIndex>,
) -> NodeId {
    collect_node_index_with_parent(node, path, index, None)
}

fn collect_node_index_with_parent(
    node: &ParseNode,
    path: &mut Vec<usize>,
    index: &mut Vec<NodeIndex>,
    parent_id: Option<NodeId>,
) -> NodeId {
    let id = NodeId::new(index.len());
    index.push(NodeIndex {
        path: path.clone(),
        parent_id,
        child_ids: Vec::with_capacity(node.children.len()),
    });

    let mut child_ids = Vec::with_capacity(node.children.len());
    for (child_index, child) in node.children.iter().enumerate() {
        path.push(child_index);
        child_ids.push(collect_node_index_with_parent(child, path, index, Some(id)));
        path.pop();
    }
    index[id.as_usize()].child_ids = child_ids;

    id
}

fn insert_symbol(symbols: &mut Vec<NodeKind>, symbol: NodeKind) {
    if let Some(existing) = symbols
        .iter_mut()
        .find(|existing| existing.symbol_id == symbol.symbol_id)
    {
        *existing = symbol;
    } else {
        symbols.push(symbol);
    }
}

fn document_node_to_parsed_node(
    node: &ParseNode,
    language: &'static crate::pure_parser::TSLanguage,
    source: &[u8],
) -> crate::pure_parser::ParsedNode {
    let symbol = table_symbol_for_public_id(language, node.symbol_id);
    let children = node
        .children
        .iter()
        .map(|child| document_node_to_parsed_node(child, language, source))
        .collect();
    let (is_named, is_extra) = symbol_flags(language, symbol);
    let is_empty_error_node =
        node.symbol_id.0 == 0 && node.children.is_empty() && node.start_byte == node.end_byte;

    crate::pure_parser::ParsedNode {
        symbol,
        children,
        start_byte: node.start_byte,
        end_byte: node.end_byte,
        start_point: byte_to_point(source, node.start_byte),
        end_point: byte_to_point(source, node.end_byte),
        is_extra,
        is_error: symbol_name(language, symbol) == Some("ERROR") || is_empty_error_node,
        is_missing: is_empty_error_node,
        is_named,
        field_id: node
            .field_name
            .as_deref()
            .and_then(|field_name| field_id_for_name(language, field_name)),
        language: Some(language as *const _),
    }
}

fn table_symbol_for_public_id(
    language: &crate::pure_parser::TSLanguage,
    public_symbol: SymbolId,
) -> crate::pure_parser::TSSymbol {
    if !language.public_symbol_map.is_null() {
        // SAFETY: `public_symbol_map` has one entry per generated table symbol.
        let public_symbols = unsafe {
            std::slice::from_raw_parts(language.public_symbol_map, language.symbol_count as usize)
        };
        if let Some(index) = public_symbols
            .iter()
            .position(|candidate| *candidate == public_symbol.0)
        {
            return index as crate::pure_parser::TSSymbol;
        }
    }

    public_symbol.0
}

fn symbol_flags(
    language: &crate::pure_parser::TSLanguage,
    symbol: crate::pure_parser::TSSymbol,
) -> (bool, bool) {
    if language.symbol_metadata.is_null() || u32::from(symbol) >= language.symbol_count {
        return (true, false);
    }

    // SAFETY: `symbol` is bounds-checked above, and `symbol_metadata` has one
    // entry per generated table symbol.
    let metadata = unsafe { *language.symbol_metadata.add(usize::from(symbol)) };
    let is_named = (metadata & 0x02) != 0;
    let is_extra = (metadata & 0x04) != 0;
    (is_named, is_extra)
}

fn field_id_for_name(language: &crate::pure_parser::TSLanguage, field_name: &str) -> Option<u16> {
    if language.field_count == 0 || language.field_names.is_null() {
        return None;
    }

    // SAFETY: `field_names` points to a static array of `field_count` C string
    // pointers generated with the language table.
    let field_names =
        unsafe { std::slice::from_raw_parts(language.field_names, language.field_count as usize) };
    field_names
        .iter()
        .enumerate()
        .find_map(|(index, name_ptr)| {
            c_str_to_str(*name_ptr)
                .filter(|candidate| *candidate == field_name)
                .map(|_| index as u16)
        })
}

fn symbol_name(
    language: &crate::pure_parser::TSLanguage,
    symbol: crate::pure_parser::TSSymbol,
) -> Option<&'static str> {
    if language.symbol_names.is_null() || u32::from(symbol) >= language.symbol_count {
        return None;
    }

    // SAFETY: `symbol` is bounds-checked above, and `symbol_names` has one C
    // string pointer per generated table symbol.
    let symbol_names = unsafe {
        std::slice::from_raw_parts(language.symbol_names, language.symbol_count as usize)
    };
    c_str_to_str(symbol_names[usize::from(symbol)])
}

fn c_str_to_str(ptr: *const u8) -> Option<&'static str> {
    if ptr.is_null() {
        return None;
    }

    // SAFETY: generated language tables store static NUL-terminated strings.
    unsafe { CStr::from_ptr(ptr.cast()).to_str().ok() }
}

fn byte_to_point(source: &[u8], byte: usize) -> crate::pure_parser::Point {
    let point = DocumentPoint::from_byte_offset(std::str::from_utf8(source).unwrap_or(""), byte);
    crate::pure_parser::Point {
        row: point.row,
        column: point.column,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn symbol_metadata(name: &str, symbol_id: SymbolId, is_named: bool) -> TableSymbolMetadata {
        TableSymbolMetadata {
            name: name.to_string(),
            is_visible: true,
            is_named,
            is_supertype: false,
            is_terminal: !is_named,
            is_extra: false,
            is_fragile: false,
            symbol_id,
        }
    }

    fn fielded_language() -> (Grammar, ParseTable) {
        let mut table = ParseTable::default();
        table.symbol_metadata = vec![
            symbol_metadata("ERROR", SymbolId(0), true),
            symbol_metadata("source_file", SymbolId(1), true),
            symbol_metadata("expression", SymbolId(2), true),
            symbol_metadata("number", SymbolId(3), true),
            symbol_metadata("-", SymbolId(4), false),
        ];
        table.symbol_count = table.symbol_metadata.len();
        table.index_to_symbol = table
            .symbol_metadata
            .iter()
            .map(|metadata| metadata.symbol_id)
            .collect();
        table.field_names = vec![
            "left".to_string(),
            "operator".to_string(),
            "right".to_string(),
        ];

        (Grammar::new("fielded".to_string()), table)
    }

    #[test]
    fn field_lookup_resolves_missing_error_child() {
        let (grammar, table) = fielded_language();
        let source_file = SymbolId(1);
        let expression = SymbolId(2);
        let number = SymbolId(3);
        let operator = SymbolId(4);
        let error = SymbolId(0);
        let source = "1-";

        let root = ParseNode {
            symbol: source_file,
            symbol_id: source_file,
            start_byte: 0,
            end_byte: source.len(),
            field_name: None,
            alias_symbol_id: None,
            children: vec![ParseNode {
                symbol: expression,
                symbol_id: expression,
                start_byte: 0,
                end_byte: source.len(),
                field_name: None,
                alias_symbol_id: None,
                children: vec![
                    ParseNode {
                        symbol: number,
                        symbol_id: number,
                        start_byte: 0,
                        end_byte: 1,
                        field_name: Some("left".to_string()),
                        alias_symbol_id: None,
                        children: Vec::new(),
                    },
                    ParseNode {
                        symbol: operator,
                        symbol_id: operator,
                        start_byte: 1,
                        end_byte: 2,
                        field_name: Some("operator".to_string()),
                        alias_symbol_id: None,
                        children: Vec::new(),
                    },
                    ParseNode {
                        symbol: error,
                        symbol_id: error,
                        start_byte: 2,
                        end_byte: 2,
                        field_name: Some("right".to_string()),
                        alias_symbol_id: None,
                        children: Vec::new(),
                    },
                ],
            }],
        };
        let document =
            AdzeDocument::from_parse_result(source, root, 1, "fielded", &grammar, &table);

        let expression = document
            .tree()
            .root()
            .child(0)
            .expect("root should expose expression child");
        let right_field = document
            .language()
            .field_id_for_name("right")
            .expect("right field should resolve");
        let right_edge = expression
            .edge_by_field_name("right")
            .expect("right edge should resolve even when its child is missing");
        let right_child = right_edge.child().expect("right edge child should resolve");

        assert_eq!(expression.field_name_for_child(2), Some("right"));
        assert_eq!(expression.field_id_for_child(2), Some(right_field));
        assert_eq!(right_edge.field_name(), Some("right"));
        assert_eq!(right_edge.field_id(), Some(right_field));
        assert_eq!(
            expression
                .child_by_field_name("right")
                .expect("right field lookup should return the missing child")
                .node_id(),
            right_child.node_id()
        );
        assert_eq!(
            expression
                .child_by_field_id(right_field)
                .expect("right field-id lookup should return the missing child")
                .node_id(),
            right_child.node_id()
        );
        assert_eq!(right_child.field_name(), Some("right"));
        assert_eq!(right_child.field_id(), Some(right_field));
        assert!(right_child.is_error());
        assert!(right_child.is_missing());
        assert!(right_child.has_error());
        assert!(document.tree().has_errors());
    }
}
