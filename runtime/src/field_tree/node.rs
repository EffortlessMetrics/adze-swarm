use super::{Point, TSLanguage};
use crate::ffi::TSSymbol;
use std::sync::Arc;

/// A child node with an optional field identifier
#[derive(Debug, Clone)]
pub struct ParsedChild {
    /// The child node
    pub node: ParsedNode,
    /// Optional field ID for this child (None if not a named field)
    pub field_id: Option<u16>,
}

impl ParsedChild {
    /// Get the field name for this child if it has one
    pub fn field_name<'a>(&self, language: &'a TSLanguage) -> Option<&'a str> {
        self.field_id.and_then(|id| language.field_name(id))
    }
}

/// A parsed node in the syntax tree
#[derive(Clone)]
pub struct ParsedNode {
    /// Symbol/kind ID for this node
    pub symbol: TSSymbol,
    /// Child nodes with their field information
    pub children: Vec<ParsedChild>,
    /// Byte range in source text
    pub start_byte: usize,
    pub end_byte: usize,
    /// Position in lines/columns
    pub start_point: Point,
    pub end_point: Point,
    /// Node flags
    pub is_extra: bool,
    pub is_error: bool,
    pub is_missing: bool,
    pub is_named: bool,
    /// Reference to the language for symbol/field name lookups
    pub language: Option<Arc<TSLanguage>>,
}

impl ParsedNode {
    /// Get a child by field name
    pub fn child_by_field_name<'a>(&'a self, name: &str) -> Option<&'a ParsedNode> {
        let language = self.language.as_ref()?;
        let field_id = language.field_id_for_name(name)?;

        self.children
            .iter()
            .find(|c| c.field_id == Some(field_id))
            .map(|c| &c.node)
    }

    /// Get all children with a specific field name
    pub fn children_by_field_name<'a>(&'a self, name: &str) -> Vec<&'a ParsedNode> {
        let language = match self.language.as_ref() {
            Some(l) => l,
            None => return Vec::new(),
        };

        let field_id = match language.field_id_for_name(name) {
            Some(id) => id,
            None => return Vec::new(),
        };

        self.children
            .iter()
            .filter(|c| c.field_id == Some(field_id))
            .map(|c| &c.node)
            .collect()
    }

    /// Get the node's kind/type name
    pub fn kind<'a>(&self, language: &'a TSLanguage) -> &'a str {
        language.symbol_name(self.symbol)
    }

    /// Iterate over all named children (skip anonymous/extra nodes)
    pub fn named_children(&self) -> impl Iterator<Item = &ParsedChild> {
        self.children.iter().filter(|c| c.node.is_named)
    }

    /// Get the Nth child (if it exists)
    pub fn child(&self, index: usize) -> Option<&ParsedNode> {
        self.children.get(index).map(|c| &c.node)
    }

    /// Get the number of children
    pub fn child_count(&self) -> usize {
        self.children.len()
    }
}

impl std::fmt::Debug for ParsedNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ParsedNode")
            .field("symbol", &self.symbol)
            .field("children", &self.children)
            .field("start_byte", &self.start_byte)
            .field("end_byte", &self.end_byte)
            .field("start_point", &self.start_point)
            .field("end_point", &self.end_point)
            .field("is_extra", &self.is_extra)
            .field("is_error", &self.is_error)
            .field("is_missing", &self.is_missing)
            .field("is_named", &self.is_named)
            .field("has_language", &self.language.is_some())
            .finish()
    }
}
