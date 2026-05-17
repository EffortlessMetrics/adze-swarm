// Query AST representation
use adze_ir::SymbolId;
use std::collections::HashMap;
use thiserror::Error;

/// A compiled tree-sitter query
#[derive(Debug, Clone)]
pub struct Query {
    /// The source query string
    pub source: String,
    /// Compiled patterns
    pub patterns: Vec<Pattern>,
    /// Capture names mapped to indices
    pub capture_names: HashMap<String, u32>,
    /// Property settings
    pub property_settings: Vec<PropertySetting>,
    /// Property predicates
    pub property_predicates: Vec<PropertyPredicate>,
}

/// A single pattern in a query
#[derive(Debug, Clone)]
pub struct Pattern {
    /// The root node of the pattern
    pub root: PatternNode,
    /// Predicates associated with this pattern
    pub predicates: Vec<Predicate>,
    /// Start byte offset in the query source
    pub start_byte: usize,
}

/// A node in a query pattern
#[derive(Debug, Clone)]
pub struct PatternNode {
    /// Symbol ID (node type)
    pub symbol: SymbolId,
    /// Child patterns
    pub children: Vec<PatternChild>,
    /// Field assertions
    pub fields: HashMap<String, PatternNode>,
    /// Capture name (if this node is captured)
    pub capture: Option<u32>,
    /// Whether this is a named node
    pub is_named: bool,
    /// Quantifier
    pub quantifier: Quantifier,
}

/// Child pattern (can be a node or anonymous token)
#[derive(Debug, Clone)]
pub enum PatternChild {
    Node(PatternNode),
    Token(String),
    Anchor,
}

/// Quantifiers for pattern nodes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Quantifier {
    One,      // Default
    Optional, // ?
    Plus,     // +
    Star,     // *
}

/// Predicates that can be used in patterns
#[derive(Debug, Clone)]
pub enum Predicate {
    /// #eq? predicate
    Eq {
        capture1: u32,
        capture2: Option<u32>,
        value: Option<String>,
    },
    /// #not-eq? predicate
    NotEq {
        capture1: u32,
        capture2: Option<u32>,
        value: Option<String>,
    },
    /// #match? predicate
    Match { capture: u32, regex: String },
    /// #not-match? predicate
    NotMatch { capture: u32, regex: String },
    /// #set! directive
    Set {
        property: String,
        capture: Option<u32>,
        value: Option<String>,
    },
    /// #is? predicate
    Is {
        property: String,
        capture: Option<u32>,
        value: Option<String>,
    },
    /// #is-not? predicate
    IsNot {
        property: String,
        capture: Option<u32>,
        value: Option<String>,
    },
    /// #any-of? predicate
    AnyOf { capture: u32, values: Vec<String> },
    /// Custom predicates
    Custom {
        name: String,
        args: Vec<PredicateArg>,
    },
}

/// Argument to a predicate
#[derive(Debug, Clone)]
pub enum PredicateArg {
    Capture(u32),
    String(String),
}

/// Property settings from #set! directives
#[derive(Debug, Clone)]
pub struct PropertySetting {
    pub key: String,
    pub value: Option<String>,
    pub capture: Option<u32>,
}

/// Property predicates from #is? and #is-not? directives
#[derive(Debug, Clone)]
pub struct PropertyPredicate {
    pub key: String,
    pub value: Option<String>,
    pub capture: Option<u32>,
    pub is_positive: bool,
}

/// Errors that can occur during query compilation or execution
#[derive(Debug, Error)]
pub enum QueryError {
    #[error("Syntax error at position {position}: {message}")]
    SyntaxError { position: usize, message: String },

    #[error("Undefined node type: {0}")]
    UndefinedNodeType(String),

    #[error("Invalid field name: {0}")]
    InvalidField(String),

    #[error("Invalid capture name: {0}")]
    InvalidCapture(String),

    #[error("Invalid predicate: {0}")]
    InvalidPredicate(String),

    #[error("Regex error: {0}")]
    RegexError(String),

    #[error("Capture index out of bounds: {0}")]
    CaptureIndexOutOfBounds(u32),
}

impl Query {
    /// Create a new empty query
    pub fn new() -> Self {
        Query {
            source: String::new(),
            patterns: Vec::new(),
            capture_names: HashMap::new(),
            property_settings: Vec::new(),
            property_predicates: Vec::new(),
        }
    }

    /// Get the index for a capture name
    pub fn capture_index(&self, name: &str) -> Option<u32> {
        self.capture_names.get(name).copied()
    }

    /// Get all capture names
    pub fn capture_names(&self) -> Vec<&str> {
        let mut names: Vec<_> = self.capture_names.iter().collect();
        names.sort_by_key(|&(_, &index)| index);
        names.into_iter().map(|(name, _)| name.as_str()).collect()
    }

    /// Get the number of captures
    pub fn capture_count(&self) -> u32 {
        self.capture_names.len() as u32
    }

    /// Get the number of patterns
    pub fn pattern_count(&self) -> usize {
        self.patterns.len()
    }
}

impl PatternNode {
    /// Create a new pattern node
    pub fn new(symbol: SymbolId, is_named: bool) -> Self {
        PatternNode {
            symbol,
            children: Vec::new(),
            fields: HashMap::new(),
            capture: None,
            is_named,
            quantifier: Quantifier::One,
        }
    }

    /// Set the capture for this node
    pub fn with_capture(mut self, capture: u32) -> Self {
        self.capture = Some(capture);
        self
    }

    /// Set the quantifier
    pub fn with_quantifier(mut self, quantifier: Quantifier) -> Self {
        self.quantifier = quantifier;
        self
    }

    /// Add a child pattern
    pub fn add_child(&mut self, child: PatternChild) {
        self.children.push(child);
    }

    /// Add a field assertion
    pub fn add_field(&mut self, field_name: String, node: PatternNode) {
        self.fields.insert(field_name, node);
    }
}

impl Default for Query {
    fn default() -> Self {
        Self::new()
    }
}
