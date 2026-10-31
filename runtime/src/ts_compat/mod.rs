//! Minimal Tree-sitter compatibility shims (edits, points, language wrapper).
#![cfg_attr(feature = "strict_docs", allow(missing_docs))]

//! Tree-sitter compatibility API
//!
//! This module provides a compatibility layer that mimics the Tree-sitter API,
//! allowing existing Tree-sitter code to work with adze with minimal changes.

use crate::document::{AdzeDocument, NodeId, SourceLineIndex};
use crate::parser_v4::Parser as CoreParser;
use crate::pure_incremental::Edit as CoreEdit;
use crate::pure_parser;
use adze_glr_core::ParseTable;
use adze_ir::Grammar;
use std::num::NonZeroU16;
use std::sync::Arc;

mod tree_cursor_nav;

/// Document-backed tree state for the ts_compat layer.
///
/// The selected tree is the owning [`AdzeDocument`]'s canonical flat records
/// (no cloned recursive tree, no root-pointer searches); `positions` is the
/// single line index for point lookups. `Tree::from_document` clones the
/// source document once per parse, replacing the old recursive-root and
/// source-byte clones.
#[derive(Clone, Debug)]
pub(crate) struct OwnedCoreTree {
    /// Owning parse document with canonical flat node/edge records.
    pub document: AdzeDocument,
    /// Line index built once per tree for point lookups.
    pub positions: SourceLineIndex,
}

impl OwnedCoreTree {
    /// Get the root symbol ID (grammar identity, ignoring aliases).
    pub(crate) fn root_kind(&self) -> u16 {
        self.document
            .syntax()
            .node_record(self.document.syntax().root_id())
            .map(|record| record.grammar_id().0)
            .unwrap_or(0)
    }

    /// Get the error count
    pub(crate) fn error_count(&self) -> usize {
        self.document.metadata().error_count
    }
}

/// A position in a document, identified by row and column.
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct Point {
    pub row: u32,
    pub column: u32,
}

impl From<(u32, u32)> for Point {
    fn from((row, column): (u32, u32)) -> Self {
        Point { row, column }
    }
}

impl From<Point> for (u32, u32) {
    fn from(p: Point) -> Self {
        (p.row, p.column)
    }
}

/// A byte and point range in a parsed document.
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct Range {
    pub start_byte: usize,
    pub end_byte: usize,
    pub start_point: Point,
    pub end_point: Point,
}

/// A nonzero Tree-sitter-compatible field identifier.
pub type FieldId = NonZeroU16;

/// An edit to a document.
#[derive(Clone, Debug, Default)]
pub struct InputEdit {
    pub start_byte: usize,
    pub old_end_byte: usize,
    pub new_end_byte: usize,
    pub start_position: Point,
    pub old_end_position: Point,
    pub new_end_position: Point,
}

impl From<InputEdit> for CoreEdit {
    fn from(e: InputEdit) -> Self {
        CoreEdit {
            start_byte: e.start_byte,
            old_end_byte: e.old_end_byte,
            new_end_byte: e.new_end_byte,
            start_point: pure_parser::Point {
                row: e.start_position.row,
                column: e.start_position.column,
            },
            old_end_point: pure_parser::Point {
                row: e.old_end_position.row,
                column: e.old_end_position.column,
            },
            new_end_point: pure_parser::Point {
                row: e.new_end_position.row,
                column: e.new_end_position.column,
            },
        }
    }
}

/// A language definition containing grammar and parse tables.
#[derive(Clone, Debug)]
pub struct Language {
    pub name: String,
    pub grammar: Grammar,
    pub table: ParseTable,
    /// Original generated language, when this compatibility language was
    /// built from one. `Parser::parse` uses it to route through the same
    /// native document pipeline as generated `parse_document` functions;
    /// decoded-only languages (via `new`) cannot, because token patterns
    /// are not recoverable from the decoded grammar alone.
    ts_language: Option<&'static pure_parser::TSLanguage>,
}

impl Language {
    pub fn new(name: impl Into<String>, grammar: Grammar, table: ParseTable) -> Self {
        Self {
            name: name.into(),
            grammar,
            table,
            ts_language: None,
        }
    }

    /// Build a compatibility language directly from a generated `TSLanguage`.
    ///
    /// Prefer this over decoding grammar and parse table by hand:
    /// `Parser::parse` on a language built this way produces the same tree
    /// as `Tree::from_document` over the generated `parse_document` output.
    #[cfg(feature = "pure-rust")]
    pub fn from_ts_language(
        name: impl Into<String>,
        language: &'static pure_parser::TSLanguage,
    ) -> Self {
        Self {
            name: name.into(),
            grammar: crate::decoder::decode_grammar(language),
            table: crate::decoder::decode_parse_table(language),
            ts_language: Some(language),
        }
    }

    fn symbol_metadata_for_id(&self, id: u16) -> Option<&adze_glr_core::SymbolMetadata> {
        let symbol = adze_ir::SymbolId(id);

        self.table
            .symbol_metadata
            .get(id as usize)
            .filter(|metadata| metadata.symbol_id == symbol)
            .or_else(|| {
                self.table
                    .symbol_metadata
                    .iter()
                    .find(|metadata| metadata.symbol_id == symbol)
            })
    }

    /// Get the number of distinct node kinds in this language.
    pub fn node_kind_count(&self) -> usize {
        self.table.symbol_count
    }

    /// Get the node kind name for the given numeric symbol id.
    pub fn node_kind_for_id(&self, id: u16) -> Option<&str> {
        self.symbol_metadata_for_id(id)
            .map(|metadata| metadata.name.as_str())
    }

    /// Get the numeric symbol id for the given node kind and namedness.
    ///
    /// Returns `0` when no matching symbol exists, matching Tree-sitter's
    /// sentinel convention.
    pub fn id_for_node_kind(&self, kind: &str, named: bool) -> u16 {
        self.table
            .symbol_metadata
            .iter()
            .find(|metadata| metadata.name == kind && metadata.is_named == named)
            .map(|metadata| metadata.symbol_id.0)
            .unwrap_or(0)
    }

    /// Check whether the given node kind id is named.
    pub fn node_kind_is_named(&self, id: u16) -> bool {
        self.symbol_metadata_for_id(id)
            .map(|metadata| metadata.is_named)
            .unwrap_or(false)
    }

    /// Check whether the given node kind id is visible.
    pub fn node_kind_is_visible(&self, id: u16) -> bool {
        self.symbol_metadata_for_id(id)
            .map(|metadata| metadata.is_visible)
            .unwrap_or(false)
    }

    /// Check whether the given node kind id is a supertype.
    pub fn node_kind_is_supertype(&self, id: u16) -> bool {
        self.symbol_metadata_for_id(id)
            .map(|metadata| metadata.is_supertype)
            .unwrap_or(false)
    }

    /// Get the number of distinct field names in this language.
    pub fn field_count(&self) -> usize {
        self.table.field_names.len()
    }

    /// Get the field name for a nonzero Tree-sitter-style field id.
    pub fn field_name_for_id(&self, field_id: u16) -> Option<&str> {
        let index = usize::from(field_id.checked_sub(1)?);
        self.table.field_names.get(index).map(String::as_str)
    }

    /// Get the nonzero Tree-sitter-style field id for the given field name.
    pub fn field_id_for_name(&self, field_name: impl AsRef<[u8]>) -> Option<FieldId> {
        let field_name = field_name.as_ref();
        self.table
            .field_names
            .iter()
            .position(|name| name.as_bytes() == field_name)
            .and_then(|index| index.checked_add(1))
            .and_then(|id| u16::try_from(id).ok())
            .and_then(FieldId::new)
    }

    /// Generate advisory Tree-sitter-style `node-types.json` metadata.
    ///
    /// This projection is generated from the language grammar metadata attached
    /// to this compatibility language. It is intentionally narrower than full
    /// Tree-sitter node-types parity: alias-visible node-types and
    /// query-compatible alias metadata remain future work.
    pub fn node_types_json(&self) -> String {
        adze_tablegen::StaticLanguageGenerator::new(self.grammar.clone(), self.table.clone())
            .generate_node_types()
    }
}

/// A parser that can parse source code using a language.
pub struct Parser {
    core: Option<CoreParser>,
    lang: Option<Arc<Language>>,
}

impl Parser {
    /// Create a new parser.
    pub fn new() -> Self {
        Self {
            core: None,
            lang: None,
        }
    }

    /// Set the language for this parser.
    pub fn set_language(&mut self, lang: Arc<Language>) -> Result<(), String> {
        self.lang = Some(Arc::clone(&lang));
        self.core = Some(CoreParser::new(
            lang.grammar.clone(),
            lang.table.clone(),
            lang.name.clone(),
        ));
        Ok(())
    }

    /// Parse source code, optionally reusing an old tree for incremental parsing.
    ///
    /// Note: Incremental parsing is currently disabled and falls back to fresh parsing
    /// for consistency. The `old` parameter is accepted for API compatibility but ignored.
    pub fn parse(&mut self, source: &str, _old: Option<&Tree>) -> Option<Tree> {
        let lang = self.lang.as_ref()?;

        // Languages built from a generated TSLanguage parse through the same
        // native document pipeline as generated parse_document functions.
        // The decoded-grammar CoreParser below cannot lex generated languages
        // (token patterns are absent after decoding) and would return a
        // degenerate zero-width "end" root instead.
        #[cfg(feature = "pure-rust")]
        if let Some(ts_language) = lang.ts_language {
            let document =
                crate::__private::parse_document(source, || ts_language, &lang.name).ok()?;
            return Some(Tree::from_document(Arc::clone(lang), &document));
        }

        let core_parser = self.core.as_mut()?;
        match core_parser.parse_document(source) {
            Ok(document) => Some(Tree::from_document(Arc::clone(lang), &document)),
            Err(_) => None,
        }
    }

    /// Get the current language.
    pub fn language(&self) -> Option<&Arc<Language>> {
        self.lang.as_ref()
    }
}

impl Default for Parser {
    fn default() -> Self {
        Self::new()
    }
}

/// A parsed syntax tree.
#[derive(Clone, Debug)]
pub struct Tree {
    pub(crate) core: OwnedCoreTree,
    pub(crate) last_edit: Option<CoreEdit>,
    pub(crate) language: Arc<Language>,
}

impl Tree {
    /// Build a Tree-sitter-compatible tree from a native parse document.
    ///
    /// Clones the source document once (replacing the old recursive-root and
    /// source-byte clones) and builds one line index shared by every node in
    /// this tree. Keeping `&AdzeDocument` (instead of taking ownership or an
    /// `Arc`) preserves the public signature; revisit under #871 if facade
    /// ownership changes.
    pub fn from_document(language: Arc<Language>, document: &AdzeDocument) -> Self {
        Self {
            core: OwnedCoreTree {
                positions: SourceLineIndex::new(document.source_text()),
                document: document.clone(),
            },
            last_edit: None,
            language,
        }
    }

    /// Get the language that was used to parse this tree.
    pub fn language(&self) -> &Language {
        self.language.as_ref()
    }

    /// Apply an edit to this tree.
    pub fn edit(&mut self, edit: &InputEdit) {
        let core_edit = CoreEdit::from(edit.clone());
        // Store the edit for later incremental parsing
        // Note: parser_v4::Tree doesn't have apply_edit, edits are tracked separately
        self.last_edit = Some(core_edit);
    }

    /// Get the root node of this tree.
    pub fn root_node(&self) -> Node<'_> {
        Node::new(self, self.core.document.syntax().root_id())
    }

    /// Get the root kind as a string.
    pub fn root_kind(&self) -> &str {
        self.kind_for_symbol(self.core.root_kind())
    }

    fn kind_for_symbol(&self, sym: u16) -> &str {
        if let Some(metadata) = self.language.symbol_metadata_for_id(sym) {
            return metadata.name.as_str();
        }
        // Try direct rule name mapping first
        if let Some(name) = self
            .language
            .grammar
            .rule_names
            .get(&adze_ir::SymbolId(sym))
        {
            return name.as_str();
        }
        // Fallback: if index_to_symbol is populated, prefer that
        if let Some(name) = self
            .language
            .table
            .index_to_symbol
            .get(sym as usize)
            .and_then(|sid| self.language.grammar.rule_names.get(sid))
        {
            return name.as_str();
        }
        "unknown"
    }

    fn visible_symbol_for_id(&self, alias: Option<adze_ir::SymbolId>, symbol: u16) -> u16 {
        alias.unwrap_or(adze_ir::SymbolId(symbol)).0
    }

    /// Get the number of errors in this tree.
    pub fn error_count(&self) -> usize {
        self.core.error_count()
    }

    /// Check if the tree has errors.
    pub fn has_errors(&self) -> bool {
        self.error_count() > 0
    }
}

/// A node in a syntax tree.
///
/// Nodes are lightweight `(tree, id)` handles over the owning document's
/// canonical flat records: no cloned subtrees, no pointer-identity searches.
#[derive(Debug, Clone, Copy)]
pub struct Node<'a> {
    tree: &'a Tree,
    id: NodeId,
}

impl<'a> Node<'a> {
    fn new(tree: &'a Tree, id: NodeId) -> Self {
        Self { tree, id }
    }

    fn record(&self) -> Option<&'a crate::document::NodeRecord> {
        self.tree.core.document.syntax().node_record(self.id)
    }

    fn point_for(&self, byte: usize) -> Point {
        let point = self.tree.core.positions.point(byte);
        Point {
            row: point.row,
            column: point.column,
        }
    }

    fn child_id(&self, index: usize) -> Option<NodeId> {
        self.tree.core.document.syntax().child_id(self.id, index)
    }

    fn parent_id(&self) -> Option<NodeId> {
        self.tree.core.document.syntax().parent_id(self.id)
    }

    fn edge_child_index(&self) -> Option<usize> {
        self.tree
            .core
            .document
            .syntax()
            .parent_edge_record(self.id)
            .map(|edge| edge.child_index())
    }

    fn field_name_for_id(&self, field_id: crate::document::FieldId) -> Option<&'a str> {
        self.tree
            .core
            .document
            .language()
            .field_name_for_id(field_id.get())
    }

    fn point_le(left: Point, right: Point) -> bool {
        left.row < right.row || (left.row == right.row && left.column <= right.column)
    }

    /// Get the language that was used to parse this node's tree.
    pub fn language(&self) -> &Language {
        self.tree.language()
    }

    /// Get the kind of this node as a string.
    pub fn kind(&self) -> &str {
        let (alias, visible) = self
            .record()
            .map(|record| (record.alias_symbol_id(), record.visible_id().0))
            .unwrap_or((None, 0));
        self.tree
            .kind_for_symbol(self.tree.visible_symbol_for_id(alias, visible))
    }

    /// Get this node's kind as a numerical symbol id.
    pub fn kind_id(&self) -> u16 {
        self.record()
            .map(|record| record.visible_id().0)
            .unwrap_or(0)
    }

    /// Get this node's grammar symbol id, ignoring aliases.
    ///
    /// Alias-visible `kind_id()` may differ from this value when production
    /// alias metadata applies.
    pub fn grammar_id(&self) -> u16 {
        self.record()
            .map(|record| record.grammar_id().0)
            .unwrap_or(0)
    }

    /// Get this node's grammar symbol name, ignoring aliases.
    ///
    /// Alias-visible `kind()` may differ from this value when production alias
    /// metadata applies.
    pub fn grammar_name(&self) -> &str {
        self.tree.kind_for_symbol(self.grammar_id())
    }

    /// Get the start byte of this node.
    pub fn start_byte(&self) -> usize {
        self.record()
            .map(|record| record.byte_range().start)
            .unwrap_or(0)
    }

    /// Get the end byte of this node.
    pub fn end_byte(&self) -> usize {
        self.record()
            .map(|record| record.byte_range().end)
            .unwrap_or(0)
    }

    /// Get the start position of this node.
    pub fn start_position(&self) -> Point {
        self.point_for(self.start_byte())
    }

    /// Get the end position of this node.
    pub fn end_position(&self) -> Point {
        self.point_for(self.end_byte())
    }

    /// Get the full byte and point range of this node.
    pub fn range(&self) -> Range {
        Range {
            start_byte: self.start_byte(),
            end_byte: self.end_byte(),
            start_point: self.start_position(),
            end_point: self.end_position(),
        }
    }

    /// Get the number of children.
    pub fn child_count(&self) -> usize {
        self.record()
            .map(|record| record.edge_range().len())
            .unwrap_or(0)
    }

    /// Get this node's descendant count, including the node itself.
    pub fn descendant_count(&self) -> usize {
        let syntax = self.tree.core.document.syntax();
        let mut count = 0;
        let mut stack = vec![self.id];
        while let Some(node_id) = stack.pop() {
            let Some(record) = syntax.node_record(node_id) else {
                continue;
            };
            count += 1;
            for offset in (0..record.edge_range().len()).rev() {
                if let Some(child_id) = syntax.child_id(node_id, offset) {
                    stack.push(child_id);
                }
            }
        }
        count
    }

    /// Get a child by index.
    pub fn child(&self, index: usize) -> Option<Node<'a>> {
        self.child_id(index).map(|id| Node::new(self.tree, id))
    }

    /// Get this node's first child that contains or starts after the given byte.
    pub fn first_child_for_byte(&self, byte: usize) -> Option<Node<'a>> {
        let syntax = self.tree.core.document.syntax();
        (0..self.child_count()).find_map(|index| {
            let child_id = syntax.child_id(self.id, index)?;
            let record = syntax.node_record(child_id)?;
            (record.byte_range().end > byte).then(|| Node::new(self.tree, child_id))
        })
    }

    /// Get this node's first named child that contains or starts after the given byte.
    pub fn first_named_child_for_byte(&self, byte: usize) -> Option<Node<'a>> {
        let syntax = self.tree.core.document.syntax();
        (0..self.child_count()).find_map(|index| {
            let child_id = syntax.child_id(self.id, index)?;
            let child = Node::new(self.tree, child_id);
            let record = syntax.node_record(child_id)?;
            (record.byte_range().end > byte && child.is_named()).then_some(child)
        })
    }

    /// Get this node's parent, if it is not the root node.
    pub fn parent(&self) -> Option<Node<'a>> {
        self.parent_id().map(|id| Node::new(self.tree, id))
    }

    /// Get this node's next sibling, if any.
    pub fn next_sibling(&self) -> Option<Node<'a>> {
        let parent = self.parent()?;
        let next_index = self.edge_child_index()? + 1;
        parent.child(next_index)
    }

    /// Get this node's previous sibling, if any.
    pub fn prev_sibling(&self) -> Option<Node<'a>> {
        let parent = self.parent()?;
        let prev_index = self.edge_child_index()?.checked_sub(1)?;
        parent.child(prev_index)
    }

    /// Check if this node is named after applying alias-visible identity.
    pub fn is_named(&self) -> bool {
        let visible_symbol = self.kind_id();
        self.tree
            .language
            .table
            .symbol_metadata
            .get(visible_symbol as usize)
            .map(|metadata| metadata.is_named)
            .unwrap_or_else(|| {
                !self
                    .tree
                    .language
                    .grammar
                    .tokens
                    .contains_key(&adze_ir::SymbolId(visible_symbol))
            })
    }

    /// Check if this node is extra after applying alias-visible identity.
    pub fn is_extra(&self) -> bool {
        let visible_symbol = self.kind_id();
        self.tree
            .language
            .table
            .symbol_metadata
            .get(visible_symbol as usize)
            .map(|metadata| metadata.is_extra)
            .unwrap_or_else(|| {
                self.tree
                    .language
                    .table
                    .extras
                    .contains(&adze_ir::SymbolId(visible_symbol))
            })
    }

    /// Get the number of named children.
    pub fn named_child_count(&self) -> usize {
        (0..self.child_count())
            .filter_map(|index| self.child(index))
            .filter(|child| child.is_named())
            .count()
    }

    /// Get a named child by named-child index.
    pub fn named_child(&self, index: usize) -> Option<Node<'a>> {
        (0..self.child_count())
            .filter_map(|child_index| self.child(child_index))
            .filter(|child| child.is_named())
            .nth(index)
    }

    fn contains_byte_range(&self, start_byte: usize, end_byte: usize) -> bool {
        start_byte <= end_byte && self.start_byte() <= start_byte && end_byte <= self.end_byte()
    }

    /// Get the smallest descendant that contains the given byte range.
    pub fn descendant_for_byte_range(
        &self,
        start_byte: usize,
        end_byte: usize,
    ) -> Option<Node<'a>> {
        if !self.contains_byte_range(start_byte, end_byte) {
            return None;
        }

        let mut current = *self;
        loop {
            let mut descended = false;
            for index in 0..current.child_count() {
                let Some(child) = current.child(index) else {
                    continue;
                };
                if child.contains_byte_range(start_byte, end_byte) {
                    current = child;
                    descended = true;
                    break;
                }
            }
            if !descended {
                return Some(current);
            }
        }
    }

    /// Get the smallest named descendant that contains the given byte range.
    pub fn named_descendant_for_byte_range(
        &self,
        start_byte: usize,
        end_byte: usize,
    ) -> Option<Node<'a>> {
        if !self.contains_byte_range(start_byte, end_byte) {
            return None;
        }

        // Deepest named node on the first-containing-child chain; a level
        // with no containing child stops the descent.
        let mut best = None;
        let mut current = *self;
        loop {
            if current.is_named() {
                best = Some(current);
            }
            let mut next = None;
            for index in 0..current.child_count() {
                if let Some(child) = current.child(index)
                    && child.contains_byte_range(start_byte, end_byte)
                {
                    next = Some(child);
                    break;
                }
            }
            match next {
                Some(child) => current = child,
                None => return best,
            }
        }
    }

    fn contains_point_range(&self, start_point: Point, end_point: Point) -> bool {
        Self::point_le(start_point, end_point)
            && Self::point_le(self.start_position(), start_point)
            && Self::point_le(end_point, self.end_position())
    }

    /// Get the smallest descendant that contains the given point range.
    pub fn descendant_for_point_range(
        &self,
        start_point: Point,
        end_point: Point,
    ) -> Option<Node<'a>> {
        if !self.contains_point_range(start_point, end_point) {
            return None;
        }

        let mut current = *self;
        loop {
            let mut descended = false;
            for index in 0..current.child_count() {
                let Some(child) = current.child(index) else {
                    continue;
                };
                if child.contains_point_range(start_point, end_point) {
                    current = child;
                    descended = true;
                    break;
                }
            }
            if !descended {
                return Some(current);
            }
        }
    }

    /// Get the smallest named descendant that contains the given point range.
    pub fn named_descendant_for_point_range(
        &self,
        start_point: Point,
        end_point: Point,
    ) -> Option<Node<'a>> {
        if !self.contains_point_range(start_point, end_point) {
            return None;
        }

        // Deepest named node on the first-containing-child chain.
        let mut best = None;
        let mut current = *self;
        loop {
            if current.is_named() {
                best = Some(current);
            }
            let mut next = None;
            for index in 0..current.child_count() {
                if let Some(child) = current.child(index)
                    && child.contains_point_range(start_point, end_point)
                {
                    next = Some(child);
                    break;
                }
            }
            match next {
                Some(child) => current = child,
                None => return best,
            }
        }
    }

    /// Get this node's next named sibling, skipping anonymous siblings.
    pub fn next_named_sibling(&self) -> Option<Node<'a>> {
        let parent = self.parent()?;
        let next_index = self.edge_child_index()? + 1;
        (next_index..parent.child_count()).find_map(|index| {
            let child = parent.child(index)?;
            child.is_named().then_some(child)
        })
    }

    /// Get this node's previous named sibling, skipping anonymous siblings.
    pub fn prev_named_sibling(&self) -> Option<Node<'a>> {
        let parent = self.parent()?;
        let prev_index = self.edge_child_index()?;
        (0..prev_index).rev().find_map(|index| {
            let child = parent.child(index)?;
            child.is_named().then_some(child)
        })
    }

    /// Convert this node and its named descendants to a Tree-sitter-style S-expression.
    pub fn to_sexp(&self) -> String {
        let mut result = String::new();
        self.write_sexp(&mut result);
        result
    }

    fn write_sexp(&self, result: &mut String) {
        result.push('(');
        if self.is_missing() {
            result.push_str("MISSING");
        } else if self.is_error() {
            result.push_str("ERROR");
        } else {
            result.push_str(self.kind());
        }

        let syntax = self.tree.core.document.syntax();
        for index in 0..self.child_count() {
            let Some(child_id) = syntax.child_id(self.id, index) else {
                continue;
            };
            let child_node = Node::new(self.tree, child_id);
            if !child_node.is_named() && !child_node.is_error() {
                continue;
            }

            result.push(' ');
            if let Some(field_name) = syntax
                .parent_edge_record(child_id)
                .and_then(|edge| edge.field_id())
                .and_then(|field_id| self.field_name_for_id(field_id))
            {
                result.push_str(field_name);
                result.push_str(": ");
            }
            child_node.write_sexp(result);
        }

        result.push(')');
    }

    /// Create a cursor rooted at this node.
    pub fn walk(&self) -> TreeCursor<'a> {
        TreeCursor::new(self.tree, self.id)
    }

    /// Get the field name attached to this node's edge from its parent.
    pub fn field_name(&self) -> Option<&str> {
        self.tree
            .core
            .document
            .syntax()
            .parent_edge_record(self.id)
            .and_then(|edge| edge.field_id())
            .and_then(|field_id| self.field_name_for_id(field_id))
    }

    /// Get the field name for a child edge by child index.
    pub fn field_name_for_child(&self, index: usize) -> Option<&str> {
        let child_id = self.child_id(index)?;
        self.tree
            .core
            .document
            .syntax()
            .parent_edge_record(child_id)
            .and_then(|edge| edge.field_id())
            .and_then(|field_id| self.field_name_for_id(field_id))
    }

    /// Get the nonzero Tree-sitter-style field id for a child edge by child index.
    pub fn field_id_for_child(&self, index: usize) -> Option<FieldId> {
        let field_name = self.field_name_for_child(index)?;
        self.language().field_id_for_name(field_name)
    }

    /// Get the first child attached through the given field name.
    pub fn child_by_field_name(&self, field_name: &str) -> Option<Node<'a>> {
        let syntax = self.tree.core.document.syntax();
        (0..self.child_count()).find_map(|index| {
            let child_id = syntax.child_id(self.id, index)?;
            let edge = syntax.parent_edge_record(child_id)?;
            let edge_field = edge.field_id()?;
            (self.field_name_for_id(edge_field) == Some(field_name))
                .then(|| Node::new(self.tree, child_id))
        })
    }

    /// Get the first child attached through the given Tree-sitter-style field id.
    pub fn child_by_field_id(&self, field_id: u16) -> Option<Node<'a>> {
        let field_name = self.language().field_name_for_id(field_id)?;
        self.child_by_field_name(field_name)
    }

    /// Check if this node is an error node.
    pub fn is_error(&self) -> bool {
        self.record()
            .map(|record| record.grammar_id().0 == 0 && record.edge_range().is_empty())
            .unwrap_or(false)
    }

    /// Check if this node is missing (was expected but not found).
    pub fn is_missing(&self) -> bool {
        self.record()
            .map(|record| {
                let range = record.byte_range();
                range.start == range.end && self.is_error()
            })
            .unwrap_or(false)
    }

    /// Check if this node or any descendant is an error node.
    pub fn has_error(&self) -> bool {
        // Flat error flags already fold in descendant errors (and the ambient
        // error count at the document root), matching the recursive walk.
        self.record()
            .map(|record| record.flags().has_error())
            .unwrap_or(false)
    }

    /// Get the byte range of this node.
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.record()
            .map(|record| record.byte_range())
            .unwrap_or(0..0)
    }

    /// Get the text content of this node.
    pub fn utf8_text<'b>(&self, source: &'b [u8]) -> Result<&'b str, std::str::Utf8Error> {
        let range = self.byte_range();
        let slice = source.get(range).unwrap_or(&[]);
        std::str::from_utf8(slice)
    }

    /// Get the text content of this node as a string.
    pub fn text(&self, source: &[u8]) -> String {
        self.utf8_text(source).unwrap_or("").to_string()
    }
}

#[derive(Debug, Clone, Copy)]
struct CursorFrame {
    pub(super) node: NodeId,
    pub(super) child_index: usize,
}

/// A cursor for walking a syntax tree without allocating child vectors.
#[derive(Debug, Clone)]
pub struct TreeCursor<'a> {
    tree: &'a Tree,
    current: NodeId,
    parents: Vec<CursorFrame>,
}

impl<'a> TreeCursor<'a> {
    fn new(tree: &'a Tree, current: NodeId) -> Self {
        Self {
            tree,
            current,
            parents: Vec::new(),
        }
    }

    fn syntax(&self) -> &'a crate::document::SyntaxTree {
        self.tree.core.document.syntax()
    }

    /// Get the cursor's current node.
    pub fn node(&self) -> Node<'a> {
        Node::new(self.tree, self.current)
    }

    /// Get the cursor depth relative to the node used to create it.
    pub fn depth(&self) -> usize {
        self.parents.len()
    }

    fn cursor_root(&self) -> NodeId {
        self.parents
            .first()
            .map(|frame| frame.node)
            .unwrap_or(self.current)
    }

    /// Get the current node's preorder descendant index relative to the cursor root.
    ///
    /// Preorder flat ids are dense, so the index is the id offset from the
    /// cursor root.
    pub fn descendant_index(&self) -> usize {
        self.current
            .as_usize()
            .saturating_sub(self.cursor_root().as_usize())
    }

    /// Move to the node at the given preorder descendant index, if it exists.
    pub fn goto_descendant(&mut self, descendant_index: usize) {
        let root = self.cursor_root();
        let syntax = self.syntax();
        // Preorder flat ids are dense: the index is an offset from the root.
        let target = NodeId::new(root.as_usize().saturating_add(descendant_index));
        if syntax.node_record(target).is_none() {
            return;
        }
        // Confirm the target descends from the cursor root by walking up.
        let mut chain = vec![target];
        let mut probe = target;
        while probe != root {
            let Some(parent) = syntax.parent_id(probe) else {
                return;
            };
            probe = parent;
            chain.push(probe);
        }
        chain.pop();
        let mut parents = Vec::with_capacity(chain.len());
        let mut current = root;
        for descendant in chain.iter().rev() {
            let Some(edge) = syntax.parent_edge_record(*descendant) else {
                return;
            };
            if edge.parent_id() != current {
                return;
            }
            parents.push(CursorFrame {
                node: current,
                child_index: edge.child_index(),
            });
            current = *descendant;
        }

        self.current = current;
        self.parents = parents;
    }

    /// Move to the first child of the current node.
    pub fn goto_first_child(&mut self) -> bool {
        let Some(child) = self.syntax().child_id(self.current, 0) else {
            return false;
        };

        self.parents.push(CursorFrame {
            node: self.current,
            child_index: 0,
        });
        self.current = child;
        true
    }

    /// Move to the last child of the current node.
    pub fn goto_last_child(&mut self) -> bool {
        let syntax = self.syntax();
        let Some(record) = syntax.node_record(self.current) else {
            return false;
        };
        let Some(child_index) = record.edge_range().len().checked_sub(1) else {
            return false;
        };
        let Some(child) = syntax.child_id(self.current, child_index) else {
            return false;
        };

        self.parents.push(CursorFrame {
            node: self.current,
            child_index,
        });
        self.current = child;
        true
    }

    /// Move to the first child that contains or starts after the given byte.
    pub fn goto_first_child_for_byte(&mut self, byte: usize) -> Option<usize> {
        let syntax = self.syntax();
        let record = syntax.node_record(self.current)?;
        let child_index = (0..record.edge_range().len()).find(|index| {
            syntax
                .child_id(self.current, *index)
                .and_then(|child| syntax.node_record(child))
                .is_some_and(|child| child.byte_range().end > byte)
        })?;
        let child = syntax.child_id(self.current, child_index)?;

        self.parents.push(CursorFrame {
            node: self.current,
            child_index,
        });
        self.current = child;
        Some(child_index)
    }

    fn point_gt(left: Point, right: Point) -> bool {
        left.row > right.row || (left.row == right.row && left.column > right.column)
    }

    /// Move to the first child that contains or starts after the given point.
    pub fn goto_first_child_for_point(&mut self, point: Point) -> Option<usize> {
        let syntax = self.syntax();
        let current = self.current;
        let record = syntax.node_record(current)?;
        let child_index = (0..record.edge_range().len()).find(|index| {
            syntax.child_id(current, *index).is_some_and(|child| {
                let end = self.tree.core.positions.point(
                    syntax
                        .node_record(child)
                        .map(|record| record.byte_range().end)
                        .unwrap_or(0),
                );
                Self::point_gt(
                    Point {
                        row: end.row,
                        column: end.column,
                    },
                    point,
                )
            })
        })?;
        let child = syntax.child_id(current, child_index)?;

        self.parents.push(CursorFrame {
            node: current,
            child_index,
        });
        self.current = child;
        Some(child_index)
    }

    /// Move to the next sibling of the current node.
    pub fn goto_next_sibling(&mut self) -> bool {
        tree_cursor_nav::goto_next_sibling(self.tree, &mut self.parents, &mut self.current)
    }

    /// Move to the previous sibling of the current node.
    pub fn goto_previous_sibling(&mut self) -> bool {
        tree_cursor_nav::goto_previous_sibling(self.tree, &mut self.parents, &mut self.current)
    }

    /// Move to the parent of the current node.
    pub fn goto_parent(&mut self) -> bool {
        let Some(parent) = self.parents.pop() else {
            return false;
        };

        self.current = parent.node;
        true
    }

    /// Get the field name attached to the current node's parent edge.
    pub fn field_name(&self) -> Option<&str> {
        if self.parents.is_empty() {
            return None;
        }

        let document = &self.tree.core.document;
        document
            .syntax()
            .parent_edge_record(self.current)
            .and_then(|edge| edge.field_id())
            .and_then(|field_id| document.language().field_name_for_id(field_id.get()))
    }

    /// Get the nonzero Tree-sitter-style field id for the current parent edge.
    pub fn field_id(&self) -> Option<FieldId> {
        let field_name = self.field_name()?;
        self.tree.language().field_id_for_name(field_name)
    }

    /// Reset this cursor to the given node and clear parent traversal state.
    pub fn reset(&mut self, node: Node<'a>) {
        self.tree = node.tree;
        self.current = node.id;
        self.parents.clear();
    }

    /// Reset this cursor to another cursor's node and parent traversal state.
    pub fn reset_to(&mut self, cursor: &Self) {
        self.tree = cursor.tree;
        self.current = cursor.current;
        self.parents.clone_from(&cursor.parents);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser_v4::ParseNode;
    use adze_glr_core::Action;
    use adze_ir::SymbolId;
    use std::collections::BTreeMap;

    fn empty_parse_table_language() -> Arc<Language> {
        Arc::new(Language::new(
            "ts_compat_empty_parse_table",
            Grammar::default(),
            ParseTable::default(),
        ))
    }

    fn accept_on_eof_language() -> Arc<Language> {
        let parse_table = ParseTable {
            symbol_to_index: BTreeMap::from([(SymbolId(0), 0)]),
            action_table: vec![vec![vec![Action::Accept]]],
            ..Default::default()
        };

        Arc::new(Language::new(
            "ts_compat_accept_on_eof",
            Grammar::default(),
            parse_table,
        ))
    }

    fn parse_node(symbol: u16, start_byte: usize, end_byte: usize) -> ParseNode {
        ParseNode {
            symbol: SymbolId(symbol),
            symbol_id: SymbolId(symbol),
            start_byte,
            end_byte,
            field_name: None,
            alias_symbol_id: None,
            children: Vec::new(),
        }
    }

    fn compat_tree(root: ParseNode, source: &str, error_count: usize) -> Tree {
        let language = empty_parse_table_language();
        let document = AdzeDocument::from_parse_result(
            source,
            root,
            error_count,
            &language.name,
            &language.grammar,
            &language.table,
        );
        Tree::from_document(Arc::clone(&language), &document)
    }

    #[test]
    fn parse_ignores_old_tree_source() {
        let mut parser = Parser::new();
        parser.set_language(empty_parse_table_language()).unwrap();

        let old_tree = parser.parse("old", None).unwrap();
        let new_source = "incrementally updated";

        let reparsed = parser.parse(new_source, Some(&old_tree)).unwrap();
        assert_eq!(reparsed.core.document.source_bytes(), new_source.as_bytes());
        assert_ne!(
            reparsed.core.document.source_bytes(),
            old_tree.core.document.source_bytes()
        );
    }

    #[test]
    fn parse_returns_none_on_core_parse_error() {
        let mut parser = Parser::new();
        parser.set_language(accept_on_eof_language()).unwrap();

        let tree = parser.parse("any input", None);

        assert!(tree.is_none());
    }

    #[test]
    fn node_has_error_propagates_from_descendant_error_node() {
        let error_child = parse_node(0, 1, 1);
        let root = ParseNode {
            symbol: SymbolId(1),
            symbol_id: SymbolId(1),
            start_byte: 0,
            end_byte: 1,
            field_name: None,
            alias_symbol_id: None,
            children: vec![error_child],
        };
        let tree = compat_tree(root, "x", 0);

        let root = tree.root_node();
        let child = root.child(0).expect("root should expose error child");

        assert!(!root.is_error());
        assert!(root.has_error());
        assert!(child.is_error());
        assert!(child.is_missing());
        assert!(child.has_error());
    }

    #[test]
    fn node_is_missing_reports_only_zero_width_error_nodes() {
        let zero_width_error = parse_node(0, 1, 1);
        let spanning_error = parse_node(0, 2, 3);
        let zero_width_non_error = parse_node(2, 3, 3);
        let root = ParseNode {
            symbol: SymbolId(1),
            symbol_id: SymbolId(1),
            start_byte: 0,
            end_byte: 3,
            field_name: None,
            alias_symbol_id: None,
            children: vec![zero_width_error, spanning_error, zero_width_non_error],
        };
        let tree = compat_tree(root, "abc", 0);

        let root = tree.root_node();
        let missing = root.child(0).expect("missing child should exist");
        let error = root.child(1).expect("spanning error child should exist");
        let empty_regular = root
            .child(2)
            .expect("zero-width regular child should exist");

        assert!(missing.is_error());
        assert!(missing.is_missing());
        assert!(error.is_error());
        assert!(!error.is_missing());
        assert!(!empty_regular.is_error());
        assert!(!empty_regular.is_missing());
    }

    #[test]
    fn node_to_sexp_renders_error_and_missing_nodes() {
        let missing_child = parse_node(0, 1, 1);
        let spanning_error_child = parse_node(0, 2, 3);
        let root = ParseNode {
            symbol: SymbolId(1),
            symbol_id: SymbolId(1),
            start_byte: 0,
            end_byte: 3,
            field_name: None,
            alias_symbol_id: None,
            children: vec![missing_child, spanning_error_child],
        };
        let tree = compat_tree(root, "abc", 0);

        assert_eq!(tree.root_node().to_sexp(), "(unknown (MISSING) (ERROR))");
    }
}
