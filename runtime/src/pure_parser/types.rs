//! Type definitions for the pure-Rust parser.
//!
//! These types are used internally by the parser and externally by callers
//! that receive [`ParseResult`].

use super::{Point, TSLanguage, TSStateId, TSSymbol};
use std::ffi::c_void;

/// Stack entry for LR parsing
#[derive(Debug, Clone)]
pub(super) struct StackEntry {
    pub(super) state: TSStateId,
    pub(super) subtree: Option<Subtree>,
    #[allow(dead_code)]
    pub(super) position: usize,
}

/// Lexer state
#[derive(Debug)]
pub(super) struct Lexer {
    pub(super) input: Vec<u8>,
    #[allow(dead_code)]
    pub(super) position: usize,
    #[allow(dead_code)]
    pub(super) external_scanner: Option<*mut c_void>,
}

/// Internal node representation during parsing
#[derive(Debug, Clone)]
pub(crate) struct Subtree {
    pub(crate) symbol: TSSymbol,
    pub(crate) children: Vec<Subtree>,
    pub(crate) start_byte: usize,
    pub(crate) end_byte: usize,
    pub(crate) start_point: Point,
    pub(crate) end_point: Point,
    pub(crate) is_extra: bool,
    pub(crate) is_error: bool,
    pub(crate) is_missing: bool,
    #[allow(dead_code)]
    pub(crate) production_id: u16,
    /// Optional field identifier for this node within its parent
    pub(crate) field_id: Option<u16>,
}

/// The result of a parse operation.
///
/// On success, `root` contains the root [`ParsedNode`] of the concrete syntax
/// tree and `errors` is empty. On failure, `root` may be `None` and `errors`
/// will contain one or more [`ParseError`]s.
pub struct ParseResult {
    /// Root node of the parse tree, or `None` if parsing failed completely.
    pub root: Option<ParsedNode>,
    /// Errors encountered during parsing.
    pub errors: Vec<ParseError>,
}

/// A node in the concrete syntax tree produced by the parser.
///
/// Each node records its symbol, byte range, child nodes, and various
/// classification flags. Use the accessor methods (e.g., [`kind`](Self::kind),
/// [`children`](Self::children), [`utf8_text`](Self::utf8_text)) rather than
/// accessing fields directly.
#[derive(Debug, Clone)]
pub struct ParsedNode {
    /// Numeric symbol identifier from the grammar.
    pub symbol: TSSymbol,
    /// Ordered child nodes.
    pub children: Vec<ParsedNode>,
    /// Inclusive start byte offset in the source.
    pub start_byte: usize,
    /// Exclusive end byte offset in the source.
    pub end_byte: usize,
    /// Start position as row/column.
    pub start_point: Point,
    /// End position as row/column.
    pub end_point: Point,
    /// Whether this node represents an "extra" token (e.g., whitespace or comment).
    pub is_extra: bool,
    /// Whether this node represents a parse error.
    pub is_error: bool,
    /// Whether the parser inserted this node to represent a missing token.
    pub is_missing: bool,
    /// Whether this is a named (as opposed to anonymous) grammar symbol.
    pub is_named: bool,
    /// Optional field identifier for this node within its parent.
    pub field_id: Option<u16>,
    pub(crate) language: Option<*const TSLanguage>,
}

/// Builder for constructing [`ParsedNode`] values outside the parser.
///
/// Parser internals usually create nodes while reducing parse-table actions,
/// but tests and adapters often need to assemble small synthetic trees. The
/// builder centralizes the default node metadata so those callers do not need
/// to duplicate struct literals or reach into crate-private fields.
#[derive(Debug, Clone)]
pub struct ParsedNodeBuilder {
    symbol: TSSymbol,
    children: Vec<ParsedNode>,
    start_byte: usize,
    end_byte: usize,
    start_point: Point,
    end_point: Point,
    is_extra: bool,
    is_error: bool,
    is_missing: bool,
    is_named: bool,
    field_id: Option<u16>,
}

impl ParsedNodeBuilder {
    /// Creates a builder with byte-range-derived points and standard node flags.
    pub fn new(
        symbol: TSSymbol,
        children: Vec<ParsedNode>,
        start_byte: usize,
        end_byte: usize,
    ) -> Self {
        Self {
            symbol,
            children,
            start_byte,
            end_byte,
            start_point: Point {
                row: 0,
                column: start_byte as u32,
            },
            end_point: Point {
                row: 0,
                column: end_byte as u32,
            },
            is_extra: false,
            is_error: false,
            is_missing: false,
            is_named: true,
            field_id: None,
        }
    }

    /// Overrides the start point.
    pub fn start_point(mut self, point: Point) -> Self {
        self.start_point = point;
        self
    }

    /// Overrides the end point.
    pub fn end_point(mut self, point: Point) -> Self {
        self.end_point = point;
        self
    }

    /// Marks whether this node is an extra token.
    pub fn is_extra(mut self, is_extra: bool) -> Self {
        self.is_extra = is_extra;
        self
    }

    /// Marks whether this node is an error node.
    pub fn is_error(mut self, is_error: bool) -> Self {
        self.is_error = is_error;
        self
    }

    /// Marks whether this node was inserted as missing.
    pub fn is_missing(mut self, is_missing: bool) -> Self {
        self.is_missing = is_missing;
        self
    }

    /// Marks whether this node is named.
    pub fn is_named(mut self, is_named: bool) -> Self {
        self.is_named = is_named;
        self
    }

    /// Sets the optional field identifier for this node.
    pub fn field_id(mut self, field_id: Option<u16>) -> Self {
        self.field_id = field_id;
        self
    }

    /// Builds the parsed node with no associated language pointer.
    pub fn build(self) -> ParsedNode {
        ParsedNode {
            symbol: self.symbol,
            children: self.children,
            start_byte: self.start_byte,
            end_byte: self.end_byte,
            start_point: self.start_point,
            end_point: self.end_point,
            is_extra: self.is_extra,
            is_error: self.is_error,
            is_missing: self.is_missing,
            is_named: self.is_named,
            field_id: self.field_id,
            language: None,
        }
    }
}

/// A parse error at a specific position in the source.
#[derive(Debug, Clone)]
pub struct ParseError {
    /// Byte offset where the error occurred.
    pub position: usize,
    /// Row/column position of the error.
    pub point: Point,
    /// Symbol IDs that the parser expected at this position.
    pub expected: Vec<TSSymbol>,
    /// Symbol ID that the parser actually found.
    pub found: TSSymbol,
}
