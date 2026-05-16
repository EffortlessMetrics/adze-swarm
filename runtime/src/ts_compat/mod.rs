//! Minimal Tree-sitter compatibility shims (edits, points, language wrapper).
#![cfg_attr(feature = "strict_docs", allow(missing_docs))]

//! Tree-sitter compatibility API
//!
//! This module provides a compatibility layer that mimics the Tree-sitter API,
//! allowing existing Tree-sitter code to work with adze with minimal changes.

use crate::document::AdzeDocument;
use crate::parser_v4::{ParseNode, Parser as CoreParser};
use crate::pure_incremental::Edit as CoreEdit;
use crate::pure_parser;
use adze_glr_core::ParseTable;
use adze_ir::Grammar;
use std::num::NonZeroU16;
use std::sync::Arc;

/// An owned tree representation for ts_compat layer.
/// This provides the interface expected by ts_compat::Tree without lifetime constraints.
#[derive(Clone, Debug)]
pub(crate) struct OwnedCoreTree {
    /// The root parse node
    pub root: ParseNode,
    /// Source text that was parsed
    pub source: Vec<u8>,
    /// Number of parse errors
    pub error_count: usize,
}

impl OwnedCoreTree {
    /// Get the root symbol ID
    pub(crate) fn root_kind(&self) -> u16 {
        self.root.symbol.0
    }

    /// Get the error count
    pub(crate) fn error_count(&self) -> usize {
        self.error_count
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
}

impl Language {
    pub fn new(name: impl Into<String>, grammar: Grammar, table: ParseTable) -> Self {
        Self {
            name: name.into(),
            grammar,
            table,
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
        let core_parser = self.core.as_mut()?;
        let lang = self.lang.as_ref()?;

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
    pub fn from_document(language: Arc<Language>, document: &AdzeDocument) -> Self {
        Self {
            core: OwnedCoreTree {
                root: document.root_parse_node().clone(),
                source: document.source_bytes().to_vec(),
                error_count: document.metadata().error_count,
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
        Node::new(self, &self.core.root)
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

    fn visible_symbol_for_node(&self, node: &ParseNode) -> u16 {
        node.alias_symbol_id.unwrap_or(node.symbol).0
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
#[derive(Debug, Clone)]
pub struct Node<'a> {
    tree: &'a Tree,
    node: &'a ParseNode,
}

impl<'a> Node<'a> {
    fn new(tree: &'a Tree, node: &'a ParseNode) -> Self {
        Self { tree, node }
    }

    /// Convert byte position to Point (row, column)
    fn byte_to_point(source: &[u8], byte_pos: usize) -> Point {
        let mut row = 0;
        let mut column = 0;

        for (i, &byte) in source.iter().enumerate() {
            if i >= byte_pos {
                break;
            }
            if byte == b'\n' {
                row += 1;
                column = 0;
            } else {
                column += 1;
            }
        }

        Point { row, column }
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
        self.tree
            .kind_for_symbol(self.tree.visible_symbol_for_node(self.node))
    }

    /// Get this node's kind as a numerical symbol id.
    pub fn kind_id(&self) -> u16 {
        self.tree.visible_symbol_for_node(self.node)
    }

    /// Get this node's grammar symbol id, ignoring aliases.
    ///
    /// Alias-visible `kind_id()` may differ from this value when production
    /// alias metadata applies.
    pub fn grammar_id(&self) -> u16 {
        self.node.symbol.0
    }

    /// Get this node's grammar symbol name, ignoring aliases.
    ///
    /// Alias-visible `kind()` may differ from this value when production alias
    /// metadata applies.
    pub fn grammar_name(&self) -> &str {
        self.tree.kind_for_symbol(self.node.symbol.0)
    }

    /// Get the start byte of this node.
    pub fn start_byte(&self) -> usize {
        self.node.start_byte
    }

    /// Get the end byte of this node.
    pub fn end_byte(&self) -> usize {
        self.node.end_byte
    }

    /// Get the start position of this node.
    pub fn start_position(&self) -> Point {
        Self::byte_to_point(&self.tree.core.source, self.node.start_byte)
    }

    /// Get the end position of this node.
    pub fn end_position(&self) -> Point {
        Self::byte_to_point(&self.tree.core.source, self.node.end_byte)
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
        self.node.children.len()
    }

    /// Get this node's descendant count, including the node itself.
    pub fn descendant_count(&self) -> usize {
        1 + self
            .node
            .children
            .iter()
            .map(|child| Node::new(self.tree, child).descendant_count())
            .sum::<usize>()
    }

    /// Get a child by index.
    pub fn child(&self, index: usize) -> Option<Node<'a>> {
        self.node
            .children
            .get(index)
            .map(|child| Node::new(self.tree, child))
    }

    /// Get this node's first child that contains or starts after the given byte.
    pub fn first_child_for_byte(&self, byte: usize) -> Option<Node<'a>> {
        self.node
            .children
            .iter()
            .find(|child| child.end_byte > byte)
            .map(|child| Node::new(self.tree, child))
    }

    /// Get this node's first named child that contains or starts after the given byte.
    pub fn first_named_child_for_byte(&self, byte: usize) -> Option<Node<'a>> {
        self.node
            .children
            .iter()
            .map(|child| Node::new(self.tree, child))
            .find(|child| child.end_byte() > byte && child.is_named())
    }

    /// Get this node's parent, if it is not the root node.
    pub fn parent(&self) -> Option<Node<'a>> {
        Self::find_parent(&self.tree.core.root, self.node)
            .map(|parent| Node::new(self.tree, parent))
    }

    fn find_parent(current: &'a ParseNode, target: *const ParseNode) -> Option<&'a ParseNode> {
        for child in &current.children {
            if std::ptr::eq(child, target) {
                return Some(current);
            }
            if let Some(parent) = Self::find_parent(child, target) {
                return Some(parent);
            }
        }
        None
    }

    fn sibling_index(parent: &'a ParseNode, target: *const ParseNode) -> Option<usize> {
        parent
            .children
            .iter()
            .position(|child| std::ptr::eq(child, target))
    }

    /// Get this node's next sibling, if any.
    pub fn next_sibling(&self) -> Option<Node<'a>> {
        let parent = self.parent()?;
        let next_index = Self::sibling_index(parent.node, self.node)? + 1;
        parent
            .node
            .children
            .get(next_index)
            .map(|child| Node::new(self.tree, child))
    }

    /// Get this node's previous sibling, if any.
    pub fn prev_sibling(&self) -> Option<Node<'a>> {
        let parent = self.parent()?;
        let prev_index = Self::sibling_index(parent.node, self.node)?.checked_sub(1)?;
        parent
            .node
            .children
            .get(prev_index)
            .map(|child| Node::new(self.tree, child))
    }

    /// Check if this node is named after applying alias-visible identity.
    pub fn is_named(&self) -> bool {
        let visible_symbol = self.tree.visible_symbol_for_node(self.node);
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
        let visible_symbol = self.tree.visible_symbol_for_node(self.node);
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
        self.node
            .children
            .iter()
            .filter(|child| Node::new(self.tree, child).is_named())
            .count()
    }

    /// Get a named child by named-child index.
    pub fn named_child(&self, index: usize) -> Option<Node<'a>> {
        self.node
            .children
            .iter()
            .filter(|child| Node::new(self.tree, child).is_named())
            .nth(index)
            .map(|child| Node::new(self.tree, child))
    }

    fn contains_byte_range(&self, start_byte: usize, end_byte: usize) -> bool {
        start_byte <= end_byte
            && self.node.start_byte <= start_byte
            && end_byte <= self.node.end_byte
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

        for child in &self.node.children {
            let child_node = Node::new(self.tree, child);
            if child_node.contains_byte_range(start_byte, end_byte) {
                return child_node.descendant_for_byte_range(start_byte, end_byte);
            }
        }

        Some(self.clone())
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

        for child in &self.node.children {
            let child_node = Node::new(self.tree, child);
            if child_node.contains_byte_range(start_byte, end_byte) {
                if let Some(named_child) =
                    child_node.named_descendant_for_byte_range(start_byte, end_byte)
                {
                    return Some(named_child);
                }
                break;
            }
        }

        self.is_named().then(|| self.clone())
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

        for child in &self.node.children {
            let child_node = Node::new(self.tree, child);
            if child_node.contains_point_range(start_point, end_point) {
                return child_node.descendant_for_point_range(start_point, end_point);
            }
        }

        Some(self.clone())
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

        for child in &self.node.children {
            let child_node = Node::new(self.tree, child);
            if child_node.contains_point_range(start_point, end_point) {
                if let Some(named_child) =
                    child_node.named_descendant_for_point_range(start_point, end_point)
                {
                    return Some(named_child);
                }
                break;
            }
        }

        self.is_named().then(|| self.clone())
    }

    /// Get this node's next named sibling, skipping anonymous siblings.
    pub fn next_named_sibling(&self) -> Option<Node<'a>> {
        let parent = self.parent()?;
        let next_index = Self::sibling_index(parent.node, self.node)? + 1;
        parent
            .node
            .children
            .iter()
            .skip(next_index)
            .map(|child| Node::new(self.tree, child))
            .find(|child| child.is_named())
    }

    /// Get this node's previous named sibling, skipping anonymous siblings.
    pub fn prev_named_sibling(&self) -> Option<Node<'a>> {
        let parent = self.parent()?;
        let prev_index = Self::sibling_index(parent.node, self.node)?;
        parent
            .node
            .children
            .iter()
            .take(prev_index)
            .rev()
            .map(|child| Node::new(self.tree, child))
            .find(|child| child.is_named())
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

        for child in &self.node.children {
            let child_node = Node::new(self.tree, child);
            if !child_node.is_named() && !child_node.is_error() {
                continue;
            }

            result.push(' ');
            if let Some(field_name) = child.field_name.as_deref() {
                result.push_str(field_name);
                result.push_str(": ");
            }
            child_node.write_sexp(result);
        }

        result.push(')');
    }

    /// Create a cursor rooted at this node.
    pub fn walk(&self) -> TreeCursor<'a> {
        TreeCursor::new(self.tree, self.node)
    }

    /// Get the field name attached to this node's edge from its parent.
    pub fn field_name(&self) -> Option<&str> {
        self.node.field_name.as_deref()
    }

    /// Get the field name for a child edge by child index.
    pub fn field_name_for_child(&self, index: usize) -> Option<&str> {
        self.node
            .children
            .get(index)
            .and_then(|child| child.field_name.as_deref())
    }

    /// Get the nonzero Tree-sitter-style field id for a child edge by child index.
    pub fn field_id_for_child(&self, index: usize) -> Option<FieldId> {
        let field_name = self.field_name_for_child(index)?;
        self.language().field_id_for_name(field_name)
    }

    /// Get the first child attached through the given field name.
    pub fn child_by_field_name(&self, field_name: &str) -> Option<Node<'a>> {
        self.node
            .children
            .iter()
            .find(|child| child.field_name.as_deref() == Some(field_name))
            .map(|child| Node::new(self.tree, child))
    }

    /// Get the first child attached through the given Tree-sitter-style field id.
    pub fn child_by_field_id(&self, field_id: u16) -> Option<Node<'a>> {
        let field_name = self.language().field_name_for_id(field_id)?;
        self.child_by_field_name(field_name)
    }

    /// Check if this node is an error node.
    pub fn is_error(&self) -> bool {
        self.node.symbol.0 == 0 && self.node.children.is_empty()
    }

    /// Check if this node is missing (was expected but not found).
    pub fn is_missing(&self) -> bool {
        self.node.start_byte == self.node.end_byte && self.is_error()
    }

    /// Check if this node or any descendant is an error node.
    pub fn has_error(&self) -> bool {
        self.is_error()
            || (std::ptr::eq(self.node, &self.tree.core.root) && self.tree.error_count() > 0)
            || self
                .node
                .children
                .iter()
                .any(|child| Node::new(self.tree, child).has_error())
    }

    /// Get the byte range of this node.
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.node.start_byte..self.node.end_byte
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

#[derive(Debug, Clone)]
struct CursorFrame<'a> {
    node: &'a ParseNode,
    child_index: usize,
}

/// A cursor for walking a syntax tree without allocating child vectors.
#[derive(Debug, Clone)]
pub struct TreeCursor<'a> {
    tree: &'a Tree,
    current: &'a ParseNode,
    parents: Vec<CursorFrame<'a>>,
}

impl<'a> TreeCursor<'a> {
    fn new(tree: &'a Tree, current: &'a ParseNode) -> Self {
        Self {
            tree,
            current,
            parents: Vec::new(),
        }
    }

    /// Get the cursor's current node.
    pub fn node(&self) -> Node<'a> {
        Node::new(self.tree, self.current)
    }

    /// Get the cursor depth relative to the node used to create it.
    pub fn depth(&self) -> usize {
        self.parents.len()
    }

    fn cursor_root(&self) -> &'a ParseNode {
        self.parents
            .first()
            .map(|frame| frame.node)
            .unwrap_or(self.current)
    }

    fn descendant_index_for(
        node: &'a ParseNode,
        target: *const ParseNode,
        next_index: &mut usize,
    ) -> Option<usize> {
        if std::ptr::eq(node, target) {
            return Some(*next_index);
        }

        for child in &node.children {
            *next_index += 1;
            if let Some(index) = Self::descendant_index_for(child, target, next_index) {
                return Some(index);
            }
        }

        None
    }

    fn path_for_descendant_index(
        node: &'a ParseNode,
        target_index: usize,
        next_index: &mut usize,
        path: &mut Vec<usize>,
    ) -> bool {
        if *next_index == target_index {
            return true;
        }

        for (child_index, child) in node.children.iter().enumerate() {
            *next_index += 1;
            path.push(child_index);
            if Self::path_for_descendant_index(child, target_index, next_index, path) {
                return true;
            }
            path.pop();
        }

        false
    }

    /// Get the current node's preorder descendant index relative to the cursor root.
    pub fn descendant_index(&self) -> usize {
        let mut next_index = 0;
        Self::descendant_index_for(
            self.cursor_root(),
            self.current as *const ParseNode,
            &mut next_index,
        )
        .unwrap_or(0)
    }

    /// Move to the node at the given preorder descendant index, if it exists.
    pub fn goto_descendant(&mut self, descendant_index: usize) {
        let root = self.cursor_root();
        let mut next_index = 0;
        let mut path = Vec::new();

        if !Self::path_for_descendant_index(root, descendant_index, &mut next_index, &mut path) {
            return;
        }

        let mut current = root;
        let mut parents = Vec::with_capacity(path.len());
        for child_index in path {
            let Some(child) = current.children.get(child_index) else {
                return;
            };
            parents.push(CursorFrame {
                node: current,
                child_index,
            });
            current = child;
        }

        self.current = current;
        self.parents = parents;
    }

    /// Move to the first child of the current node.
    pub fn goto_first_child(&mut self) -> bool {
        let Some(child) = self.current.children.first() else {
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
        let Some(child_index) = self.current.children.len().checked_sub(1) else {
            return false;
        };
        let Some(child) = self.current.children.get(child_index) else {
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
        let child_index = self
            .current
            .children
            .iter()
            .position(|child| child.end_byte > byte)?;
        let child = self.current.children.get(child_index)?;

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
        let child_index = self.current.children.iter().position(|child| {
            let child_node = Node::new(self.tree, child);
            Self::point_gt(child_node.end_position(), point)
        })?;
        let child = self.current.children.get(child_index)?;

        self.parents.push(CursorFrame {
            node: self.current,
            child_index,
        });
        self.current = child;
        Some(child_index)
    }

    /// Move to the next sibling of the current node.
    pub fn goto_next_sibling(&mut self) -> bool {
        let Some(parent) = self.parents.last_mut() else {
            return false;
        };

        let next_index = parent.child_index + 1;
        let Some(next) = parent.node.children.get(next_index) else {
            return false;
        };

        parent.child_index = next_index;
        self.current = next;
        true
    }

    /// Move to the previous sibling of the current node.
    pub fn goto_previous_sibling(&mut self) -> bool {
        let Some(parent) = self.parents.last_mut() else {
            return false;
        };
        let Some(previous_index) = parent.child_index.checked_sub(1) else {
            return false;
        };
        let Some(previous) = parent.node.children.get(previous_index) else {
            return false;
        };

        parent.child_index = previous_index;
        self.current = previous;
        true
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

        self.current.field_name.as_deref()
    }

    /// Get the nonzero Tree-sitter-style field id for the current parent edge.
    pub fn field_id(&self) -> Option<FieldId> {
        let field_name = self.field_name()?;
        self.tree.language().field_id_for_name(field_name)
    }

    /// Reset this cursor to the given node and clear parent traversal state.
    pub fn reset(&mut self, node: Node<'a>) {
        self.tree = node.tree;
        self.current = node.node;
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

    #[test]
    fn parse_ignores_old_tree_source() {
        let mut parser = Parser::new();
        parser.set_language(empty_parse_table_language()).unwrap();

        let old_tree = parser.parse("old", None).unwrap();
        let new_source = "incrementally updated";

        let reparsed = parser.parse(new_source, Some(&old_tree)).unwrap();
        assert_eq!(reparsed.core.source, new_source.as_bytes().to_vec());
        assert_ne!(reparsed.core.source, old_tree.core.source);
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
        let tree = Tree {
            core: OwnedCoreTree {
                root,
                source: b"x".to_vec(),
                error_count: 0,
            },
            last_edit: None,
            language: empty_parse_table_language(),
        };

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
        let tree = Tree {
            core: OwnedCoreTree {
                root,
                source: b"abc".to_vec(),
                error_count: 0,
            },
            last_edit: None,
            language: empty_parse_table_language(),
        };

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
        let tree = Tree {
            core: OwnedCoreTree {
                root,
                source: b"abc".to_vec(),
                error_count: 0,
            },
            last_edit: None,
            language: empty_parse_table_language(),
        };

        assert_eq!(tree.root_node().to_sexp(), "(unknown (MISSING) (ERROR))");
    }
}
