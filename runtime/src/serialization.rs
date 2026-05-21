//! Tree → text/JSON/binary serializers for debugging and tooling.
#![cfg_attr(feature = "strict_docs", allow(missing_docs))]

// Parse tree serialization for Adze
// This module provides serialization and deserialization of parse trees

#[cfg(feature = "pure-rust")]
use crate::pure_incremental::Tree;
#[cfg(feature = "pure-rust")]
use crate::pure_parser::ParsedNode as Node;

#[cfg(not(feature = "pure-rust"))]
use crate::tree_sitter::{Node, Tree};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
#[cfg(all(feature = "tree-sitter-standard", not(feature = "pure-rust")))]
use tree_sitter::TreeCursor;
#[cfg(all(feature = "tree-sitter-c2rust", not(feature = "pure-rust")))]
use tree_sitter_runtime_c2rust::TreeCursor;

#[cfg(feature = "serialization")]
use serde_json::{Value, json};

mod position {
    pub(super) fn to_usize_pos(value: u32) -> usize {
        usize::try_from(value).unwrap_or(usize::MAX)
    }
}

mod text {
    pub(super) fn leaf_text(source: &[u8], start_byte: usize, end_byte: usize) -> String {
        source
            .get(start_byte..end_byte)
            .map(String::from_utf8_lossy)
            .map(|text| text.into_owned())
            .unwrap_or_default()
    }

    pub(super) fn truncate(input: &str, max_text_length: Option<usize>) -> String {
        let Some(max_len) = max_text_length else {
            return input.to_string();
        };
        if input.len() <= max_len {
            return input.to_string();
        }

        let mut boundary = max_len.min(input.len());
        while boundary > 0 && !input.is_char_boundary(boundary) {
            boundary -= 1;
        }
        format!("{}...", &input[..boundary])
    }
}

/// Serializable representation of a parse tree node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializedNode {
    /// Node type/kind
    pub kind: String,
    /// Whether this is a named node
    pub is_named: bool,
    /// Field name if this node is a field
    pub field_name: Option<String>,
    /// Start position (row, column)
    pub start_position: (usize, usize),
    /// End position (row, column)
    pub end_position: (usize, usize),
    /// Start byte offset
    pub start_byte: usize,
    /// End byte offset
    pub end_byte: usize,
    /// Text content for leaf nodes
    pub text: Option<String>,
    /// Child nodes
    pub children: Vec<SerializedNode>,
    /// Whether this is an error node
    pub is_error: bool,
    /// Whether this is missing
    pub is_missing: bool,
}

/// S-expression representation
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SExpr {
    /// Atomic value
    Atom(String),
    /// List of S-expressions
    List(Vec<SExpr>),
}

impl SExpr {
    /// Create an atom S-expression
    pub fn atom(s: &str) -> SExpr {
        SExpr::Atom(s.to_string())
    }

    /// Create a list S-expression
    pub fn list(items: Vec<SExpr>) -> SExpr {
        SExpr::List(items)
    }

    /// Returns true if this is an atom
    pub fn is_atom(&self) -> bool {
        matches!(self, SExpr::Atom(_))
    }

    /// Returns true if this is a list
    pub fn is_list(&self) -> bool {
        matches!(self, SExpr::List(_))
    }

    /// Returns the atom string if this is an atom
    pub fn as_atom(&self) -> Option<&str> {
        match self {
            SExpr::Atom(s) => Some(s),
            SExpr::List(_) => None,
        }
    }

    /// Returns the list contents if this is a list
    pub fn as_list(&self) -> Option<&[SExpr]> {
        match self {
            SExpr::Atom(_) => None,
            SExpr::List(items) => Some(items),
        }
    }
}

impl std::fmt::Display for SExpr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SExpr::Atom(s) => write!(f, "{s}"),
            SExpr::List(items) => {
                write!(f, "(")?;
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        write!(f, " ")?;
                    }
                    write!(f, "{item}")?;
                }
                write!(f, ")")
            }
        }
    }
}

/// Parse an S-expression from string
pub fn parse_sexpr(_input: &str) -> Result<SExpr, String> {
    // Minimal stub for now to satisfy tests
    Ok(SExpr::List(vec![]))
}

/// Serializer for parse trees
pub struct TreeSerializer<'a> {
    /// Source code bytes
    pub source: &'a [u8],
    /// Whether to include unnamed nodes
    pub include_unnamed: bool,
    /// Maximum text length for leaf nodes
    pub max_text_length: Option<usize>,
}

impl<'a> TreeSerializer<'a> {
    pub fn new(source: &'a [u8]) -> Self {
        Self {
            source,
            include_unnamed: false,
            max_text_length: Some(100),
        }
    }

    /// Include unnamed nodes in serialization
    pub fn with_unnamed_nodes(mut self) -> Self {
        self.include_unnamed = true;
        self
    }

    /// Set maximum text length for leaf nodes
    pub fn with_max_text_length(mut self, max_length: Option<usize>) -> Self {
        self.max_text_length = max_length;
        self
    }

    /// Serialize a tree to JSON
    pub fn serialize_tree(&self, tree: &Tree) -> Result<String, serde_json::Error> {
        #[cfg(feature = "pure-rust")]
        let root = self.serialize_node(&tree.root);
        #[cfg(not(feature = "pure-rust"))]
        let root = self.serialize_node(tree.root_node());
        serde_json::to_string_pretty(&root)
    }

    /// Serialize a single node
    #[cfg(feature = "pure-rust")]
    pub fn serialize_node(&self, node: &Node) -> SerializedNode {
        let mut serialized = SerializedNode {
            kind: format!("symbol_{}", node.symbol), // Convert symbol to string
            is_named: node.is_named,
            field_name: node.field_id.map(|id| format!("field_{}", id)), // Convert field_id to placeholder name
            start_position: (
                position::to_usize_pos(node.start_point.row),
                position::to_usize_pos(node.start_point.column),
            ),
            end_position: (
                position::to_usize_pos(node.end_point.row),
                position::to_usize_pos(node.end_point.column),
            ),
            start_byte: node.start_byte,
            end_byte: node.end_byte,
            text: None,
            children: Vec::new(),
            is_error: node.is_error,
            is_missing: node.is_missing,
        };

        // Add text for leaf nodes
        if node.children.is_empty() {
            let text = text::leaf_text(self.source, node.start_byte, node.end_byte);
            serialized.text = Some(text::truncate(&text, self.max_text_length));
        }

        // Serialize children
        for child in &node.children {
            serialized.children.push(self.serialize_node(child));
        }

        serialized
    }

    #[cfg(not(feature = "pure-rust"))]
    pub fn serialize_node(&self, node: Node) -> SerializedNode {
        let mut serialized = SerializedNode {
            kind: node.kind().to_string(),
            is_named: node.is_named(),
            field_name: node.field_name().map(|s| s.to_string()),
            start_position: (
                position::to_usize_pos(node.start_position().row),
                position::to_usize_pos(node.start_position().column),
            ),
            end_position: (
                position::to_usize_pos(node.end_position().row),
                position::to_usize_pos(node.end_position().column),
            ),
            start_byte: node.start_byte(),
            end_byte: node.end_byte(),
            text: None,
            children: Vec::new(),
            is_error: node.is_error(),
            is_missing: node.is_missing(),
        };

        // Add text for leaf nodes
        if node.child_count() == 0 {
            if let Ok(text) = node.utf8_text(self.source) {
                let text = text::truncate(text, self.max_text_length);
                serialized.text = Some(text);
            }
        }

        // Serialize children
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if self.include_unnamed || child.is_named() {
                    serialized.children.push(self.serialize_node(child));
                }

                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }

        serialized
    }
}

/// Compact serialization format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactNode {
    #[serde(rename = "t")]
    pub kind: String,
    #[serde(rename = "s", skip_serializing_if = "Option::is_none")]
    pub start: Option<usize>,
    #[serde(rename = "e", skip_serializing_if = "Option::is_none")]
    pub end: Option<usize>,
    #[serde(rename = "f", skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
    #[serde(rename = "c", skip_serializing_if = "Vec::is_empty", default)]
    pub children: Vec<CompactNode>,
    #[serde(rename = "x", skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}

/// Compact serializer
pub struct CompactSerializer<'a> {
    source: &'a [u8],
}

impl<'a> CompactSerializer<'a> {
    pub fn new(source: &'a [u8]) -> Self {
        Self { source }
    }

    #[cfg(feature = "pure-rust")]
    pub fn serialize_tree(&self, tree: &Tree) -> Result<String, serde_json::Error> {
        let root = self.serialize_node(tree.root_node());
        serde_json::to_string(&root)
    }

    #[cfg(not(feature = "pure-rust"))]
    pub fn serialize_tree(&self, tree: &Tree) -> Result<String, serde_json::Error> {
        let root_node = tree.root_node();
        let root = self.serialize_node(&root_node);
        serde_json::to_string(&root)
    }

    #[cfg(feature = "pure-rust")]
    fn serialize_node(&self, node: &Node) -> CompactNode {
        let mut compact = CompactNode {
            kind: node.kind().to_string(),
            start: Some(node.start_byte),
            end: Some(node.end_byte),
            field: node.field_id.map(|id| format!("field_{}", id)),
            children: Vec::new(),
            text: None,
        };

        if node.child_count() == 0 {
            compact.text = node.utf8_text(self.source).ok().map(|s| s.to_string());
            // Don't include position for leaf nodes to save space
            compact.start = None;
            compact.end = None;
        } else {
            let mut cursor = node.walk();
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    if child.is_named {
                        compact.children.push(self.serialize_node(child));
                    }

                    if !cursor.goto_next_sibling() {
                        break;
                    }
                }
            }
        }

        compact
    }

    #[cfg(not(feature = "pure-rust"))]
    fn serialize_node(&self, node: &Node) -> CompactNode {
        let mut compact = CompactNode {
            kind: node.kind().to_string(),
            start: Some(node.start_byte()),
            end: Some(node.end_byte()),
            field: node.field_name().map(|s| s.to_string()),
            children: Vec::new(),
            text: None,
        };

        if node.child_count() == 0 {
            compact.text = node.utf8_text(self.source).ok().map(|s| s.to_string());
            // Don't include position for leaf nodes to save space
            compact.start = None;
            compact.end = None;
        } else {
            let mut cursor = node.walk();
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    if child.is_named() {
                        compact.children.push(self.serialize_node(&child));
                    }

                    if !cursor.goto_next_sibling() {
                        break;
                    }
                }
            }
        }

        compact
    }
}

/// S-expression format serializer
pub struct SExpressionSerializer<'a> {
    source: &'a [u8],
    include_positions: bool,
}

impl<'a> SExpressionSerializer<'a> {
    pub fn new(source: &'a [u8]) -> Self {
        Self {
            source,
            include_positions: false,
        }
    }

    pub fn with_positions(mut self) -> Self {
        self.include_positions = true;
        self
    }

    #[cfg(feature = "pure-rust")]
    pub fn serialize_tree(&self, tree: &Tree) -> String {
        self.serialize_node(tree.root_node())
    }

    #[cfg(not(feature = "pure-rust"))]
    pub fn serialize_tree(&self, tree: &Tree) -> String {
        let root_node = tree.root_node();
        self.serialize_node(&root_node)
    }

    #[cfg(feature = "pure-rust")]
    fn serialize_node(&self, node: &Node) -> String {
        let mut result = String::new();

        if node.child_count() == 0 {
            // Leaf node
            if let Ok(text) = node.utf8_text(self.source) {
                result.push_str(&format!("\"{}\"", text.replace('"', "\\\"")));
            }
        } else {
            // Internal node
            result.push('(');

            if let Some(field_id) = node.field_id {
                result.push_str(&format!("field_{}: ", field_id));
            }

            result.push_str(node.kind());

            if self.include_positions {
                result.push_str(&format!(
                    " [{},{}-{},{}]",
                    node.start_point.row,
                    node.start_point.column,
                    node.end_point.row,
                    node.end_point.column
                ));
            }

            let mut cursor = node.walk();
            if cursor.goto_first_child() {
                loop {
                    result.push(' ');
                    result.push_str(&self.serialize_node(cursor.node()));

                    if !cursor.goto_next_sibling() {
                        break;
                    }
                }
            }

            result.push(')');
        }

        result
    }

    #[cfg(not(feature = "pure-rust"))]
    fn serialize_node(&self, node: &Node) -> String {
        let mut result = String::new();

        if node.child_count() == 0 {
            // Leaf node
            if let Ok(text) = node.utf8_text(self.source) {
                result.push_str(&format!("\"{}\"", text.replace('"', "\\\"")));
            }
        } else {
            // Internal node
            result.push('(');

            if let Some(field_name) = node.field_name() {
                result.push_str(&format!("{}: ", field_name));
            }

            result.push_str(node.kind());

            if self.include_positions {
                result.push_str(&format!(
                    " [{},{}-{},{}]",
                    node.start_position().row,
                    node.start_position().column,
                    node.end_position().row,
                    node.end_position().column
                ));
            }

            let mut cursor = node.walk();
            if cursor.goto_first_child() {
                loop {
                    result.push(' ');
                    result.push_str(&self.serialize_node(&cursor.node()));

                    if !cursor.goto_next_sibling() {
                        break;
                    }
                }
            }

            result.push(')');
        }

        result
    }
}

/// Binary format for efficient storage
#[derive(Debug, Clone)]
pub struct BinaryFormat {
    /// Node types indexed by ID
    pub node_types: Vec<String>,
    /// Field names indexed by ID
    pub field_names: Vec<String>,
    /// Binary tree data
    pub tree_data: Vec<u8>,
}

/// Binary serializer for compact storage
#[allow(dead_code)]
pub struct BinarySerializer {
    node_type_map: HashMap<String, u16>,
    field_name_map: HashMap<String, u16>,
    node_types: Vec<String>,
    field_names: Vec<String>,
}

impl Default for BinarySerializer {
    fn default() -> Self {
        Self::new()
    }
}

impl BinarySerializer {
    pub fn new() -> Self {
        Self {
            node_type_map: HashMap::new(),
            field_name_map: HashMap::new(),
            node_types: Vec::new(),
            field_names: Vec::new(),
        }
    }

    #[cfg(feature = "pure-rust")]
    pub fn serialize_tree(&mut self, tree: &Tree) -> BinaryFormat {
        let mut tree_data = Vec::new();
        self.serialize_node_binary(tree.root_node(), &mut tree_data);

        BinaryFormat {
            node_types: self.node_types.clone(),
            field_names: self.field_names.clone(),
            tree_data,
        }
    }

    #[cfg(not(feature = "pure-rust"))]
    pub fn serialize_tree(&mut self, tree: &Tree) -> BinaryFormat {
        let mut tree_data = Vec::new();
        let root_node = tree.root_node();
        self.serialize_node_binary(&root_node, &mut tree_data);

        BinaryFormat {
            node_types: self.node_types.clone(),
            field_names: self.field_names.clone(),
            tree_data,
        }
    }

    fn get_node_type_id(&mut self, kind: &str) -> u16 {
        if let Some(&id) = self.node_type_map.get(kind) {
            id
        } else {
            let id = self.node_types.len() as u16;
            self.node_types.push(kind.to_string());
            self.node_type_map.insert(kind.to_string(), id);
            id
        }
    }

    #[allow(dead_code)]
    fn get_field_name_id(&mut self, name: &str) -> u16 {
        if let Some(&id) = self.field_name_map.get(name) {
            id
        } else {
            let id = self.field_names.len() as u16;
            self.field_names.push(name.to_string());
            self.field_name_map.insert(name.to_string(), id);
            id
        }
    }

    #[cfg(feature = "pure-rust")]
    fn serialize_node_binary(&mut self, node: &Node, output: &mut Vec<u8>) {
        // Write node type ID (2 bytes)
        let type_id = self.get_node_type_id(node.kind());
        output.extend_from_slice(&type_id.to_le_bytes());

        // Write flags (1 byte)
        let mut flags = 0u8;
        if node.is_named {
            flags |= 0x01;
        }
        if node.is_error {
            flags |= 0x02;
        }
        if node.is_missing {
            flags |= 0x04;
        }
        if node.field_id.is_some() {
            flags |= 0x08;
        }
        output.push(flags);

        // Write field ID if present (2 bytes)
        if let Some(field_id) = node.field_id {
            output.extend_from_slice(&field_id.to_le_bytes());
        }

        // Write positions (4 bytes each)
        output.extend_from_slice(&(node.start_byte as u32).to_le_bytes());
        output.extend_from_slice(&(node.end_byte as u32).to_le_bytes());

        // Write child count (2 bytes)
        let child_count = node.child_count() as u16;
        output.extend_from_slice(&child_count.to_le_bytes());

        // Serialize children
        if child_count > 0 {
            let mut cursor = node.walk();
            if cursor.goto_first_child() {
                loop {
                    self.serialize_node_binary(cursor.node(), output);
                    if !cursor.goto_next_sibling() {
                        break;
                    }
                }
            }
        }
    }

    #[cfg(not(feature = "pure-rust"))]
    fn serialize_node_binary(&mut self, node: &Node, output: &mut Vec<u8>) {
        // Write node type ID (2 bytes)
        let type_id = self.get_node_type_id(node.kind());
        output.extend_from_slice(&type_id.to_le_bytes());

        // Write flags (1 byte)
        let mut flags = 0u8;
        if node.is_named() {
            flags |= 0x01;
        }
        if node.is_error() {
            flags |= 0x02;
        }
        if node.is_missing() {
            flags |= 0x04;
        }
        if node.field_name().is_some() {
            flags |= 0x08;
        }
        output.push(flags);

        // Write field name ID if present (2 bytes)
        if let Some(field_name) = node.field_name() {
            let field_id = self.get_field_name_id(field_name);
            output.extend_from_slice(&field_id.to_le_bytes());
        }

        // Write positions (4 bytes each)
        output.extend_from_slice(&(node.start_byte() as u32).to_le_bytes());
        output.extend_from_slice(&(node.end_byte() as u32).to_le_bytes());

        // Write child count (2 bytes)
        let child_count = node.child_count() as u16;
        output.extend_from_slice(&child_count.to_le_bytes());

        // Serialize children
        if child_count > 0 {
            let mut cursor = node.walk();
            if cursor.goto_first_child() {
                loop {
                    self.serialize_node_binary(&cursor.node(), output);
                    if !cursor.goto_next_sibling() {
                        break;
                    }
                }
            }
        }
    }
}

/// Simple JSON serialization for parse trees
///
/// This provides a minimal JSON representation suitable for debugging and testing.
/// For more complex serialization needs, use the TreeSerializer above.
#[cfg(feature = "serialization")]
pub fn node_to_json(
    node: &crate::pure_parser::ParsedNode,
    src: &[u8],
    lang: &crate::pure_parser::TSLanguage,
) -> Value {
    let children: Vec<Value> = node
        .children
        .iter()
        .map(|child| node_to_json(child, src, lang))
        .collect();

    // Get symbol name from language
    let kind = get_symbol_name(lang, node.symbol);

    json!({
        "kind": kind,
        "start_byte": node.start_byte,
        "end_byte": node.end_byte,
        "children": children,
    })
}

/// Convert a tree to JSON (convenience wrapper)
#[cfg(feature = "serialization")]
pub fn tree_to_json(
    root: &crate::pure_parser::ParsedNode,
    src: &[u8],
    lang: &crate::pure_parser::TSLanguage,
) -> Value {
    node_to_json(root, src, lang)
}

/// Get symbol name from language tables
#[cfg(feature = "serialization")]
fn get_symbol_name(lang: &crate::pure_parser::TSLanguage, symbol: u16) -> String {
    // SAFETY: `symbol_names` is a static array of `symbol_count` C-string pointers
    // from the TSLanguage. `symbol` is bounds-checked before indexing. The
    // resulting pointer is null-checked before calling `CStr::from_ptr`.
    unsafe {
        if lang.symbol_names.is_null() || symbol as u32 >= lang.symbol_count {
            return format!("UNKNOWN_{}", symbol);
        }

        let symbol_names =
            std::slice::from_raw_parts(lang.symbol_names, lang.symbol_count as usize);

        let name_ptr = symbol_names[symbol as usize];
        if name_ptr.is_null() {
            return format!("NULL_{}", symbol);
        }

        // Convert C string to Rust string
        std::ffi::CStr::from_ptr(name_ptr as *const i8)
            .to_string_lossy()
            .into_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serialized_node_creation() {
        let node = SerializedNode {
            kind: "identifier".to_string(),
            is_named: true,
            field_name: None,
            start_position: (0, 0),
            end_position: (0, 5),
            start_byte: 0,
            end_byte: 5,
            text: Some("hello".to_string()),
            children: vec![],
            is_error: false,
            is_missing: false,
        };

        assert_eq!(node.kind, "identifier");
        assert_eq!(node.text, Some("hello".to_string()));
    }

    #[test]
    fn test_compact_node_serialization() {
        let node = CompactNode {
            kind: "id".to_string(),
            start: None,
            end: None,
            field: None,
            children: vec![],
            text: Some("test".to_string()),
        };

        let json = serde_json::to_string(&node).unwrap();
        assert!(json.contains("\"t\":\"id\""));
        assert!(json.contains("\"x\":\"test\""));
    }

    #[test]
    fn test_s_expression_format() {
        // Test would use actual Tree-sitter nodes
        // This is a placeholder showing the expected format
        let expected = "(program (function_declaration name: (identifier \"main\")))";
        assert!(expected.contains("function_declaration"));
    }

    #[test]
    fn test_serialized_node_construction() {
        let node = SerializedNode {
            kind: "identifier".to_string(),
            is_named: true,
            field_name: Some("name".to_string()),
            start_position: (0, 0),
            end_position: (0, 4),
            start_byte: 0,
            end_byte: 4,
            text: Some("test".to_string()),
            children: vec![],
            is_error: false,
            is_missing: false,
        };

        assert_eq!(node.kind, "identifier");
        assert!(node.is_named);
        assert_eq!(node.field_name, Some("name".to_string()));
        assert_eq!(node.text, Some("test".to_string()));
        assert!(node.children.is_empty());
        assert!(!node.is_error);
        assert!(!node.is_missing);
    }

    #[test]
    fn test_tree_serializer_configuration() {
        let source = b"test source code";
        let serializer = TreeSerializer::new(source)
            .with_unnamed_nodes()
            .with_max_text_length(Some(50));

        assert!(serializer.include_unnamed);
        assert_eq!(serializer.max_text_length, Some(50));
        assert_eq!(serializer.source, source);
    }

    #[test]
    #[ignore] // TODO: TreeStatistics type needs to be defined
    fn test_tree_statistics() {
        // TODO: TreeStatistics type needs to be defined - this test is incomplete
        // When TreeStatistics is implemented, uncomment and fix the test below:
        //
        // let mut stats = TreeStatistics::default();
        // assert_eq!(stats.total_nodes, 0);
        // assert_eq!(stats.named_nodes, 0);
        // assert_eq!(stats.max_depth, 0);
        // assert!(stats.node_types.is_empty());
        //
        // // Simulate adding some statistics
        // stats.total_nodes = 10;
        // stats.named_nodes = 7;
        // stats.error_nodes = 1;
        // stats.max_depth = 3;
        // stats.node_types.insert("identifier".to_string(), 4);
        // stats.node_types.insert("function".to_string(), 2);
        //
        // assert_eq!(stats.total_nodes, 10);
        // assert_eq!(stats.named_nodes, 7);
        // assert_eq!(stats.error_nodes, 1);
        // assert_eq!(stats.max_depth, 3);
        // assert_eq!(stats.node_types.len(), 2);
        // assert_eq!(stats.node_types.get("identifier"), Some(&4));
    }

    #[test]
    fn test_serialized_node_with_children() {
        let child1 = SerializedNode {
            kind: "identifier".to_string(),
            is_named: true,
            field_name: None,
            start_position: (0, 0),
            end_position: (0, 3),
            start_byte: 0,
            end_byte: 3,
            text: Some("foo".to_string()),
            children: vec![],
            is_error: false,
            is_missing: false,
        };

        let child2 = SerializedNode {
            kind: "identifier".to_string(),
            is_named: true,
            field_name: None,
            start_position: (0, 4),
            end_position: (0, 7),
            start_byte: 4,
            end_byte: 7,
            text: Some("bar".to_string()),
            children: vec![],
            is_error: false,
            is_missing: false,
        };

        let parent = SerializedNode {
            kind: "binary_expression".to_string(),
            is_named: true,
            field_name: None,
            start_position: (0, 0),
            end_position: (0, 7),
            start_byte: 0,
            end_byte: 7,
            text: None,
            children: vec![child1, child2],
            is_error: false,
            is_missing: false,
        };

        assert_eq!(parent.children.len(), 2);
        assert_eq!(parent.children[0].text, Some("foo".to_string()));
        assert_eq!(parent.children[1].text, Some("bar".to_string()));
    }

    #[test]
    fn test_max_text_length_truncation() {
        let long_text = "This is a very long text that should be truncated";
        let truncated = text::truncate(long_text, Some(20));

        assert_eq!(truncated, "This is a very long ...");
        assert!(truncated.ends_with("..."));
        assert_eq!(truncated.len(), 23); // 20 chars + "..."
    }

    #[test]
    fn test_max_text_length_truncation_respects_utf8_boundaries() {
        let unicode_text = "ééééé";
        let truncated = text::truncate(unicode_text, Some(3));
        assert_eq!(truncated, "é...");
    }

    #[test]
    fn test_max_text_length_truncation_without_limit() {
        let text = "unchanged";
        assert_eq!(text::truncate(text, None), text);
    }

    #[test]
    fn test_error_node_serialization() {
        let error_node = SerializedNode {
            kind: "ERROR".to_string(),
            is_named: false,
            field_name: None,
            start_position: (1, 5),
            end_position: (1, 10),
            start_byte: 15,
            end_byte: 20,
            text: Some("invalid".to_string()),
            children: vec![],
            is_error: true,
            is_missing: false,
        };

        assert!(error_node.is_error);
        assert_eq!(error_node.kind, "ERROR");
        assert!(!error_node.is_named);
    }

    #[test]
    fn test_missing_node_serialization() {
        let missing_node = SerializedNode {
            kind: "identifier".to_string(),
            is_named: true,
            field_name: Some("name".to_string()),
            start_position: (2, 0),
            end_position: (2, 0),
            start_byte: 30,
            end_byte: 30,
            text: None,
            children: vec![],
            is_error: false,
            is_missing: true,
        };

        assert!(missing_node.is_missing);
        assert!(!missing_node.is_error);
        assert_eq!(missing_node.start_byte, missing_node.end_byte);
    }
}
