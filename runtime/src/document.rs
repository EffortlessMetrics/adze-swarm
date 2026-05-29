//! Native parse document alpha.
//!
//! This module is the first implementation slice of the `AdzeDocument`
//! contract. It intentionally exposes only the current parser tree, source
//! text, basic diagnostics, and metadata. Richer projections such as full typed
//! CST generation and GLR forest summaries are future slices.

use crate::parser_v4::ParseNode;
use adze_glr_core::{ParseTable, SymbolMetadata as TableSymbolMetadata};

mod change_tracking;
mod diagnostic_model;
mod diagnostics;
#[cfg(feature = "serialization")]
mod json_projection;
mod position;
mod runtime_metadata;
mod syntax_model;
mod tree_builder;
mod typed_ast;
mod typed_conversion;
mod views;

use change_tracking::conservative_changed_ranges;
pub use diagnostic_model::{ParseDiagnostic, ParseDiagnosticWithSource};
use diagnostics::{attach_related_nodes, build_diagnostics};
#[cfg(feature = "serialization")]
use json_projection::{ambiguity_to_json, diagnostic_to_json, node_to_document_json};
pub use position::{DocumentPoint, PointRange};
pub(crate) use runtime_metadata::DocumentRuntime;
pub use runtime_metadata::{IncrementalFallbackReason, ParseMetadata};
pub(in crate::document) use syntax_model::NodeIndex;
pub use syntax_model::{
    EdgeRange, EdgeRecord, LanguageMetadata, NodeFlags, NodeId, NodeIdentity, NodeKind, NodeRecord,
    SyntaxTree,
};
use tree_builder::build_node_index;
pub(in crate::document) use typed_ast::TypedAstExtractionTarget;
pub use typed_ast::{Provenance, SyntheticReason, TypedAst};
use typed_conversion::document_node_to_parsed_node;
pub use views::{AdzeEdge, AdzeNode, AdzeTree, SyntaxNode};

use adze_ir::{Grammar, SymbolId};
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
            metadata: ParseMetadata::new(error_count),
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

    /// Return this document with metadata showing an incremental request fell
    /// back to a full reparse.
    ///
    /// This is an experimental lifecycle hook for parser/runtime integration.
    /// It records metadata only; it does not claim incremental reuse happened.
    #[must_use]
    pub fn with_full_reparse_fallback_metadata(
        mut self,
        reason: IncrementalFallbackReason,
    ) -> Self {
        self.metadata.incremental_requested = true;
        self.metadata.incremental_used = false;
        self.metadata.fallback_reason = Some(reason);
        self
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

    /// Return conservative changed byte ranges between this document and a
    /// newer document.
    ///
    /// The returned ranges are byte ranges in `newer`'s source text. This is
    /// an experimental document-lifecycle helper: it reports a source-text
    /// range that changed, but does not claim node reuse, stable cross-document
    /// node identity, or incremental performance.
    pub fn changed_ranges(&self, newer: &Self) -> impl Iterator<Item = Range<usize>> {
        conservative_changed_ranges(&self.source, &newer.source).into_iter()
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
                "incremental_requested": self.metadata().incremental_requested,
                "incremental_used": self.metadata().incremental_used,
                "fallback_reason": self
                    .metadata()
                    .fallback_reason
                    .as_ref()
                    .map(IncrementalFallbackReason::as_str),
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

    fn fielded_document(source: &str) -> AdzeDocument {
        let (grammar, table) = fielded_language();
        let source_file = SymbolId(1);
        let expression = SymbolId(2);
        let number = SymbolId(3);

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
                children: vec![ParseNode {
                    symbol: number,
                    symbol_id: number,
                    start_byte: 0,
                    end_byte: source.len(),
                    field_name: Some("left".to_string()),
                    alias_symbol_id: None,
                    children: Vec::new(),
                }],
            }],
        };

        AdzeDocument::from_parse_result(source, root, 0, "fielded", &grammar, &table)
    }

    #[test]
    fn reparse_fallback_metadata_records_full_reparse_fallback() {
        let (grammar, table) = fielded_language();
        let source_file = SymbolId(1);
        let expression = SymbolId(2);
        let number = SymbolId(3);
        let source = "1";

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
                children: vec![ParseNode {
                    symbol: number,
                    symbol_id: number,
                    start_byte: 0,
                    end_byte: source.len(),
                    field_name: Some("left".to_string()),
                    alias_symbol_id: None,
                    children: Vec::new(),
                }],
            }],
        };
        let document =
            AdzeDocument::from_parse_result(source, root, 0, "fielded", &grammar, &table);

        assert!(!document.metadata().incremental_requested);
        assert!(!document.metadata().incremental_used);
        assert!(!document.metadata().full_reparse_fallback());

        let document = document
            .with_full_reparse_fallback_metadata(IncrementalFallbackReason::FullReparseOnly);

        assert!(document.metadata().incremental_requested);
        assert!(!document.metadata().incremental_used);
        assert_eq!(
            document.metadata().fallback_reason.as_ref(),
            Some(&IncrementalFallbackReason::FullReparseOnly)
        );
        assert!(document.metadata().full_reparse_fallback());
    }

    #[test]
    fn changed_ranges_returns_empty_when_sources_match() {
        let old = fielded_document("1 + 2");
        let newer = fielded_document("1 + 2");

        let ranges: Vec<Range<usize>> = old.changed_ranges(&newer).collect();

        assert!(ranges.is_empty());
    }

    #[test]
    fn changed_ranges_reports_new_document_byte_range() {
        let old = fielded_document("1 + 2");
        let newer = fielded_document("1 + 3");

        let ranges: Vec<Range<usize>> = old.changed_ranges(&newer).collect();

        assert_eq!(ranges, vec![4..5]);
        assert_eq!(newer.source_slice(ranges[0].clone()), Some("3"));
    }

    #[test]
    fn changed_ranges_preserves_utf8_boundaries() {
        let old = fielded_document("a é c");
        let newer = fielded_document("a ê c");

        let ranges: Vec<Range<usize>> = old.changed_ranges(&newer).collect();

        assert_eq!(ranges, vec![2..4]);
        assert_eq!(newer.source_slice(ranges[0].clone()), Some("ê"));
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
