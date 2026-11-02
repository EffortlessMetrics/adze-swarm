//! Query processing for GLR parse forests.
#![cfg_attr(feature = "strict_docs", allow(missing_docs))]

// Query support for GLR parse results
// Implements Tree-sitter's query language for pattern matching on GLR trees

use adze_ir::{Grammar, SymbolId};
use std::collections::HashMap;
use std::fmt;

/// A simple tree representation for query matching
#[derive(Debug, Clone)]
pub struct Subtree {
    pub symbol: SymbolId,
    pub children: Vec<Subtree>,
    #[allow(dead_code)]
    pub start_byte: usize,
    #[allow(dead_code)]
    pub end_byte: usize,
}

/// A query pattern for matching against GLR parse trees
#[derive(Debug, Clone)]
pub struct Query {
    /// The patterns to match
    pub patterns: Vec<Pattern>,
    /// Capture names mapped to indices
    pub capture_names: HashMap<String, u32>,
    /// Predicate functions
    pub predicates: Vec<Predicate>,
}

/// A pattern is a tree structure to match
#[derive(Debug, Clone)]
pub struct Pattern {
    /// The root node of the pattern
    pub root: PatternNode,
    /// Predicates that must be satisfied
    pub predicate_indices: Vec<usize>,
}

/// A node in a pattern tree
#[derive(Debug, Clone)]
pub struct PatternNode {
    /// The symbol to match (None for wildcard)
    symbol: Option<SymbolId>,
    /// Capture name if this node should be captured
    capture: Option<String>,
    /// Child patterns
    children: Vec<PatternChild>,
    /// Whether this is an anchor (must match at root)
    #[allow(dead_code)]
    is_anchor: bool,
}

/// A child in a pattern can be required or have quantifiers
#[derive(Debug, Clone)]
pub struct PatternChild {
    /// The child pattern node
    node: PatternNode,
    /// Quantifier for this child
    quantifier: Quantifier,
    /// Whether this is a field name match
    #[allow(dead_code)]
    field_name: Option<String>,
}

/// Quantifiers for pattern matching
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Quantifier {
    /// Exactly one (default)
    One,
    /// Zero or one (?)
    ZeroOrOne,
    /// Zero or more (*)
    ZeroOrMore,
    /// One or more (+)
    OneOrMore,
}

/// Predicates for additional matching constraints
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum Predicate {
    /// #eq? predicate - captures must be equal
    Equal(Vec<u32>),
    /// #not-eq? predicate - captures must not be equal
    NotEqual(Vec<u32>),
    /// #match? predicate - capture must match regex
    Match(u32, String),
    /// #not-match? predicate - capture must not match regex
    NotMatch(u32, String),
    /// #any-of? predicate - capture must be one of values
    AnyOf(u32, Vec<String>),
}

/// A match found by running a query against a GLR-forest [`Subtree`].
///
/// This is the GLR-forest-specific shape: captures hold `Subtree`s, not
/// `ParseNode`s. It shares its name — but not its definition — with
/// `query::matcher::QueryMatch` (source-free tree matcher) and
/// `query::matcher_v2::QueryMatch` (source-aware; the recommended primary,
/// see #861). The document-backed matcher is the recommended entry point
/// for new code; do not unify these types here.
#[derive(Debug, Clone)]
pub struct QueryMatch {
    /// Index of the pattern that matched
    pub pattern_index: usize,
    /// Captured nodes
    pub captures: Vec<QueryCapture>,
}

/// A captured node from a GLR-forest query match.
///
/// Holds a [`Subtree`], unlike the same-named `query::matcher::QueryCapture`
/// and `query::matcher_v2::QueryCapture` types, which capture recursive-tree
/// nodes instead.
#[derive(Debug, Clone)]
pub struct QueryCapture {
    /// Index of the capture
    pub index: u32,
    /// The captured subtree
    pub subtree: Subtree,
}

/// Query parser for Tree-sitter query syntax
pub struct QueryParser<'a> {
    grammar: &'a Grammar,
    input: &'a str,
    position: usize,
}

impl<'a> QueryParser<'a> {
    pub fn new(grammar: &'a Grammar, input: &'a str) -> Self {
        Self {
            grammar,
            input,
            position: 0,
        }
    }

    /// Parse a query string
    pub fn parse(mut self) -> Result<Query, QueryError> {
        let mut patterns = Vec::new();
        let mut capture_names = HashMap::new();
        let mut predicates = Vec::new();
        let mut next_capture_id = 0;

        self.skip_whitespace();
        while !self.is_at_end() {
            // Parse a pattern
            let (pattern_node, pattern_predicates) =
                self.parse_pattern(&mut capture_names, &mut next_capture_id)?;

            let predicate_start = predicates.len();
            for pred in pattern_predicates {
                predicates.push(pred);
            }
            let predicate_end = predicates.len();

            patterns.push(Pattern {
                root: pattern_node,
                predicate_indices: (predicate_start..predicate_end).collect(),
            });

            self.skip_whitespace();
        }

        if patterns.is_empty() {
            return Err(QueryError::EmptyQuery);
        }

        Ok(Query {
            patterns,
            capture_names,
            predicates,
        })
    }

    /// Parse a single pattern
    fn parse_pattern(
        &mut self,
        capture_names: &mut HashMap<String, u32>,
        next_capture_id: &mut u32,
    ) -> Result<(PatternNode, Vec<Predicate>), QueryError> {
        self.skip_whitespace();

        if !self.consume_char('(') {
            return Err(QueryError::ExpectedOpenParen(self.position));
        }

        let node = self.parse_pattern_node(capture_names, next_capture_id)?;
        let mut predicates = Vec::new();

        // Parse predicates
        self.skip_whitespace();
        while self.peek_char() == Some('(') && self.peek_ahead(1) == Some('#') {
            predicates.push(self.parse_predicate(capture_names)?);
            self.skip_whitespace();
        }

        Ok((node, predicates))
    }

    /// Parse a pattern node
    fn parse_pattern_node(
        &mut self,
        capture_names: &mut HashMap<String, u32>,
        next_capture_id: &mut u32,
    ) -> Result<PatternNode, QueryError> {
        self.skip_whitespace();

        // Check for anchor
        let is_anchor = self.consume_char('.');

        // Parse node type or wildcard
        let symbol = if self.consume_char('_') {
            None // Wildcard
        } else {
            let node_type = self.parse_identifier()?;
            self.find_symbol(&node_type)?
        };

        // Parse children
        let mut children = Vec::new();
        self.skip_whitespace();

        while self.peek_char() != Some(')') && !self.is_at_end() {
            // Check for field name
            let field_name = if self.peek_char() == Some('[') {
                self.advance();
                let name = self.parse_identifier()?;
                if !self.consume_char(']') {
                    return Err(QueryError::ExpectedCloseBracket(self.position));
                }
                self.skip_whitespace();
                if !self.consume_char(':') {
                    return Err(QueryError::ExpectedColon(self.position));
                }
                Some(name)
            } else {
                None
            };

            // Parse child pattern
            self.skip_whitespace();
            if !self.consume_char('(') {
                return Err(QueryError::ExpectedOpenParen(self.position));
            }

            let child_node = self.parse_pattern_node(capture_names, next_capture_id)?;

            // Parse quantifier
            self.skip_whitespace();
            let quantifier = match self.peek_char() {
                Some('?') => {
                    self.advance();
                    Quantifier::ZeroOrOne
                }
                Some('*') => {
                    self.advance();
                    Quantifier::ZeroOrMore
                }
                Some('+') => {
                    self.advance();
                    Quantifier::OneOrMore
                }
                _ => Quantifier::One,
            };

            children.push(PatternChild {
                node: child_node,
                quantifier,
                field_name,
            });

            self.skip_whitespace();
        }

        if !self.consume_char(')') {
            return Err(QueryError::ExpectedCloseParen(self.position));
        }

        // Check for capture after the node
        self.skip_whitespace();
        let capture = if self.peek_char() == Some('@') {
            self.advance();
            let name = self.parse_identifier()?;
            if !capture_names.contains_key(&name) {
                capture_names.insert(name.clone(), *next_capture_id);
                *next_capture_id += 1;
            }
            Some(name)
        } else {
            None
        };

        Ok(PatternNode {
            symbol,
            capture,
            children,
            is_anchor,
        })
    }

    /// Parse a predicate
    fn parse_predicate(
        &mut self,
        capture_names: &HashMap<String, u32>,
    ) -> Result<Predicate, QueryError> {
        self.skip_whitespace();
        if !self.consume_char('(') {
            return Err(QueryError::ExpectedOpenParen(self.position));
        }
        if !self.consume_char('#') {
            return Err(QueryError::ExpectedHash(self.position));
        }

        let predicate_name = self.parse_identifier()?;
        if !self.consume_char('?') {
            return Err(QueryError::ExpectedQuestionMark(self.position));
        }

        self.skip_whitespace();

        let predicate = match predicate_name.as_str() {
            "eq" => {
                let mut captures = Vec::new();
                while self.peek_char() == Some('@') {
                    self.advance();
                    let name = self.parse_identifier()?;
                    let id = capture_names
                        .get(&name)
                        .ok_or(QueryError::UnknownCapture(name))?;
                    captures.push(*id);
                    self.skip_whitespace();
                }
                if captures.len() < 2 {
                    return Err(QueryError::InvalidPredicate(
                        "eq? requires at least 2 captures".into(),
                    ));
                }
                Predicate::Equal(captures)
            }
            "match" => {
                if !self.consume_char('@') {
                    return Err(QueryError::ExpectedAt(self.position));
                }
                let capture_name = self.parse_identifier()?;
                let capture_id = capture_names
                    .get(&capture_name)
                    .ok_or(QueryError::UnknownCapture(capture_name))?;
                self.skip_whitespace();
                let pattern = self.parse_string()?;
                Predicate::Match(*capture_id, pattern)
            }
            _ => return Err(QueryError::UnknownPredicate(predicate_name)),
        };

        self.skip_whitespace();
        if !self.consume_char(')') {
            return Err(QueryError::ExpectedCloseParen(self.position));
        }

        Ok(predicate)
    }

    /// Find a symbol by name in the grammar
    fn find_symbol(&self, name: &str) -> Result<Option<SymbolId>, QueryError> {
        // Check tokens
        for (id, token) in &self.grammar.tokens {
            if token.name == name {
                return Ok(Some(*id));
            }
        }

        // Check non-terminals
        for (id, rule_name) in &self.grammar.rule_names {
            if rule_name == name {
                return Ok(Some(*id));
            }
        }

        Err(QueryError::UnknownNodeType(name.to_string()))
    }

    // Parsing utilities
    fn skip_whitespace(&mut self) {
        while let Some(ch) = self.peek_char() {
            if ch.is_whitespace() || ch == ';' {
                self.advance();
                // Skip comments
                if ch == ';' {
                    while let Some(ch) = self.peek_char() {
                        self.advance();
                        if ch == '\n' {
                            break;
                        }
                    }
                }
            } else {
                break;
            }
        }
    }

    fn peek_char(&self) -> Option<char> {
        self.input.chars().nth(self.position)
    }

    fn peek_ahead(&self, n: usize) -> Option<char> {
        self.input.chars().nth(self.position + n)
    }

    fn advance(&mut self) -> Option<char> {
        let ch = self.peek_char();
        if ch.is_some() {
            self.position += 1;
        }
        ch
    }

    fn consume_char(&mut self, expected: char) -> bool {
        if self.peek_char() == Some(expected) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn is_at_end(&self) -> bool {
        self.position >= self.input.len()
    }

    fn parse_identifier(&mut self) -> Result<String, QueryError> {
        let start = self.position;

        // First character must be letter or underscore
        match self.peek_char() {
            Some(ch) if ch.is_alphabetic() || ch == '_' => self.advance(),
            _ => return Err(QueryError::ExpectedIdentifier(self.position)),
        };

        // Rest can be alphanumeric, underscore, or hyphen
        while let Some(ch) = self.peek_char() {
            if ch.is_alphanumeric() || ch == '_' || ch == '-' {
                self.advance();
            } else {
                break;
            }
        }

        Ok(self.input[start..self.position].to_string())
    }

    fn parse_string(&mut self) -> Result<String, QueryError> {
        if !self.consume_char('"') {
            return Err(QueryError::ExpectedString(self.position));
        }

        let mut result = String::new();
        let mut escaped = false;

        while let Some(ch) = self.advance() {
            if escaped {
                match ch {
                    'n' => result.push('\n'),
                    't' => result.push('\t'),
                    'r' => result.push('\r'),
                    '\\' => result.push('\\'),
                    '"' => result.push('"'),
                    _ => {
                        result.push('\\');
                        result.push(ch);
                    }
                }
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                return Ok(result);
            } else {
                result.push(ch);
            }
        }

        Err(QueryError::UnterminatedString(self.position))
    }
}

/// Query execution engine for GLR-forest [`Subtree`]s.
///
/// This cursor bounds search by maximum depth. It is a different type from
/// the same-named `query::cursor::QueryCursor`, which bounds recursive-tree
/// matching by byte range and root-only flags. For document-backed matching
/// prefer the `matcher_v2` path (see #861).
pub struct QueryCursor {
    /// Maximum depth to search
    max_depth: Option<usize>,
}

impl Default for QueryCursor {
    fn default() -> Self {
        Self::new()
    }
}

impl QueryCursor {
    pub fn new() -> Self {
        Self { max_depth: None }
    }

    /// Set maximum depth for pattern matching
    pub fn set_max_depth(&mut self, depth: usize) {
        self.max_depth = Some(depth);
    }

    /// Execute a query on a subtree
    pub fn matches<'a>(
        &self,
        query: &'a Query,
        root: &'a Subtree,
    ) -> impl Iterator<Item = QueryMatch> + 'a {
        QueryMatches {
            query,
            root,
            pattern_index: 0,
            node_stack: vec![(root, 0)],
            max_depth: self.max_depth,
            captures: Vec::new(),
        }
    }
}

/// Iterator over query matches
struct QueryMatches<'a> {
    query: &'a Query,
    root: &'a Subtree,
    pattern_index: usize,
    node_stack: Vec<(&'a Subtree, usize)>,
    max_depth: Option<usize>,
    captures: Vec<QueryCapture>,
}

impl<'a> Iterator for QueryMatches<'a> {
    type Item = QueryMatch;

    fn next(&mut self) -> Option<Self::Item> {
        while self.pattern_index < self.query.patterns.len() {
            let pattern = &self.query.patterns[self.pattern_index];

            // Try to match pattern at current position
            if let Some(result) = self.find_next_match(pattern) {
                return Some(result);
            }

            // Move to next pattern
            self.pattern_index += 1;
            self.node_stack = vec![(self.root, 0)];
        }

        None
    }
}

impl<'a> QueryMatches<'a> {
    fn find_next_match(&mut self, pattern: &Pattern) -> Option<QueryMatch> {
        while let Some((node, depth)) = self.node_stack.pop() {
            // Check depth limit
            if let Some(max) = self.max_depth
                && depth > max
            {
                continue;
            }

            // Clear captures for new match attempt
            self.captures.clear();

            // Try to match pattern at this node
            if self.match_pattern_node(&pattern.root, node, depth) {
                // Check predicates
                if self.check_predicates(pattern) {
                    // Found a match!
                    let result = QueryMatch {
                        pattern_index: self.pattern_index,
                        captures: self.captures.clone(),
                    };

                    // Continue searching from children
                    self.add_children_to_stack(node, depth + 1);

                    return Some(result);
                }
            }

            // Add children to continue depth-first search
            self.add_children_to_stack(node, depth + 1);
        }

        None
    }

    fn match_pattern_node(
        &mut self,
        pattern: &PatternNode,
        node: &'a Subtree,
        _depth: usize,
    ) -> bool {
        // Check symbol match
        if let Some(expected_symbol) = pattern.symbol
            && node.symbol != expected_symbol
        {
            return false;
        }

        // Capture if needed
        if let Some(ref capture_name) = pattern.capture
            && let Some(&capture_id) = self.query.capture_names.get(capture_name)
        {
            self.captures.push(QueryCapture {
                index: capture_id,
                subtree: node.clone(),
            });
        }

        // Match children
        if !self.match_children(&pattern.children, &node.children) {
            return false;
        }

        true
    }

    fn match_children(
        &mut self,
        pattern_children: &[PatternChild],
        node_children: &'a [Subtree],
    ) -> bool {
        // If no pattern children, match any node
        if pattern_children.is_empty() {
            return true;
        }

        // Try to match pattern children as a subsequence of node children
        // This allows Tree-sitter queries like (parent (child)) to match
        // even if parent has other children besides child
        self.match_children_subsequence(pattern_children, node_children, 0, 0)
    }

    fn match_children_subsequence(
        &mut self,
        pattern_children: &[PatternChild],
        node_children: &'a [Subtree],
        pattern_idx: usize,
        node_idx: usize,
    ) -> bool {
        // All pattern children matched
        if pattern_idx >= pattern_children.len() {
            return true;
        }

        let pattern_child = &pattern_children[pattern_idx];

        match pattern_child.quantifier {
            Quantifier::One => {
                // Find the first matching node child starting from node_idx
                for i in node_idx..node_children.len() {
                    if self.match_pattern_node(&pattern_child.node, &node_children[i], 0) {
                        // Found a match, continue with next pattern child
                        return self.match_children_subsequence(
                            pattern_children,
                            node_children,
                            pattern_idx + 1,
                            i + 1,
                        );
                    }
                }
                false
            }
            Quantifier::ZeroOrOne => {
                // Try matching, but also try skipping this pattern
                for i in node_idx..node_children.len() {
                    if self.match_pattern_node(&pattern_child.node, &node_children[i], 0)
                        && self.match_children_subsequence(
                            pattern_children,
                            node_children,
                            pattern_idx + 1,
                            i + 1,
                        )
                    {
                        return true;
                    }
                }
                // Also try skipping this pattern (zero matches)
                self.match_children_subsequence(
                    pattern_children,
                    node_children,
                    pattern_idx + 1,
                    node_idx,
                )
            }
            Quantifier::ZeroOrMore => {
                // Match as many as possible, but also allow zero matches
                let mut current_node_idx = node_idx;
                loop {
                    // Try continuing with next pattern
                    if self.match_children_subsequence(
                        pattern_children,
                        node_children,
                        pattern_idx + 1,
                        current_node_idx,
                    ) {
                        return true;
                    }

                    // Try matching one more
                    if current_node_idx >= node_children.len() {
                        break;
                    }
                    if self.match_pattern_node(
                        &pattern_child.node,
                        &node_children[current_node_idx],
                        0,
                    ) {
                        current_node_idx += 1;
                    } else {
                        break;
                    }
                }
                false
            }
            Quantifier::OneOrMore => {
                // Must match at least once
                let mut matched = false;
                for i in node_idx..node_children.len() {
                    if self.match_pattern_node(&pattern_child.node, &node_children[i], 0) {
                        matched = true;
                        // Try continuing after this match
                        if self.match_children_subsequence(
                            pattern_children,
                            node_children,
                            pattern_idx + 1,
                            i + 1,
                        ) {
                            return true;
                        }
                        // Try matching more (greedy)
                    } else if matched {
                        // Already matched at least once, try continuing
                        return self.match_children_subsequence(
                            pattern_children,
                            node_children,
                            pattern_idx + 1,
                            i,
                        );
                    } else {
                        break;
                    }
                }
                false
            }
        }
    }

    fn check_predicates(&self, pattern: &Pattern) -> bool {
        for &pred_index in &pattern.predicate_indices {
            if let Some(predicate) = self.query.predicates.get(pred_index)
                && !self.check_predicate(predicate)
            {
                return false;
            }
        }
        true
    }

    fn check_predicate(&self, predicate: &Predicate) -> bool {
        match predicate {
            Predicate::Equal(capture_ids) => {
                if capture_ids.len() < 2 {
                    return true;
                }

                let first_text = self.get_capture_text(capture_ids[0]);
                for &id in &capture_ids[1..] {
                    if self.get_capture_text(id) != first_text {
                        return false;
                    }
                }
                true
            }
            Predicate::NotEqual(capture_ids) => {
                if capture_ids.len() < 2 {
                    return true;
                }

                let first_text = self.get_capture_text(capture_ids[0]);
                for &id in &capture_ids[1..] {
                    if self.get_capture_text(id) == first_text {
                        return false;
                    }
                }
                true
            }
            Predicate::Match(capture_id, pattern) => {
                let text = self.get_capture_text(*capture_id);
                if let Ok(regex) = regex::Regex::new(pattern) {
                    regex.is_match(&text)
                } else {
                    false
                }
            }
            Predicate::NotMatch(capture_id, pattern) => {
                let text = self.get_capture_text(*capture_id);
                if let Ok(regex) = regex::Regex::new(pattern) {
                    !regex.is_match(&text)
                } else {
                    true
                }
            }
            Predicate::AnyOf(capture_id, values) => {
                let text = self.get_capture_text(*capture_id);
                values.iter().any(|v| v == &text)
            }
        }
    }

    fn get_capture_text(&self, capture_id: u32) -> String {
        self.captures
            .iter()
            .find(|c| c.index == capture_id)
            .map(|c| format!("{:?}", c.subtree.symbol)) // Simplified - would need source text
            .unwrap_or_default()
    }

    fn add_children_to_stack(&mut self, node: &'a Subtree, depth: usize) {
        // Add children in reverse order for depth-first traversal
        for child in node.children.iter().rev() {
            self.node_stack.push((child, depth));
        }
    }
}

/// Query parsing and execution errors
#[derive(Debug, Clone)]
pub enum QueryError {
    EmptyQuery,
    ExpectedOpenParen(usize),
    ExpectedCloseParen(usize),
    ExpectedCloseBracket(usize),
    ExpectedColon(usize),
    ExpectedHash(usize),
    ExpectedQuestionMark(usize),
    ExpectedAt(usize),
    ExpectedIdentifier(usize),
    ExpectedString(usize),
    UnterminatedString(usize),
    UnknownNodeType(String),
    UnknownCapture(String),
    UnknownPredicate(String),
    InvalidPredicate(String),
}

impl fmt::Display for QueryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            QueryError::EmptyQuery => write!(f, "Query cannot be empty"),
            QueryError::ExpectedOpenParen(pos) => write!(f, "Expected '(' at position {}", pos),
            QueryError::ExpectedCloseParen(pos) => write!(f, "Expected ')' at position {}", pos),
            QueryError::ExpectedCloseBracket(pos) => write!(f, "Expected ']' at position {}", pos),
            QueryError::ExpectedColon(pos) => write!(f, "Expected ':' at position {}", pos),
            QueryError::ExpectedHash(pos) => write!(f, "Expected '#' at position {}", pos),
            QueryError::ExpectedQuestionMark(pos) => write!(f, "Expected '?' at position {}", pos),
            QueryError::ExpectedAt(pos) => write!(f, "Expected '@' at position {}", pos),
            QueryError::ExpectedIdentifier(pos) => {
                write!(f, "Expected identifier at position {}", pos)
            }
            QueryError::ExpectedString(pos) => write!(f, "Expected string at position {}", pos),
            QueryError::UnterminatedString(pos) => {
                write!(f, "Unterminated string at position {}", pos)
            }
            QueryError::UnknownNodeType(name) => write!(f, "Unknown node type: {}", name),
            QueryError::UnknownCapture(name) => write!(f, "Unknown capture: @{}", name),
            QueryError::UnknownPredicate(name) => write!(f, "Unknown predicate: #{}?", name),
            QueryError::InvalidPredicate(msg) => write!(f, "Invalid predicate: {}", msg),
        }
    }
}

impl std::error::Error for QueryError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_query_parser_simple() {
        let mut grammar = Grammar::new("test".to_string());

        // Add some test symbols
        let expr_id = SymbolId(0);
        grammar.rule_names.insert(expr_id, "expression".to_string());

        let add_id = SymbolId(1);
        grammar.tokens.insert(
            add_id,
            adze_ir::Token {
                name: "plus".to_string(),
                pattern: adze_ir::TokenPattern::String("+".to_string()),
                fragile: false,
            },
        );

        // Parse a simple query
        let parser = QueryParser::new(&grammar, "(expression (plus))");
        let query = parser.parse().unwrap();

        assert_eq!(query.patterns.len(), 1);
        assert_eq!(query.capture_names.len(), 0);
    }

    #[test]
    fn test_query_parser_with_captures() {
        let mut grammar = Grammar::new("test".to_string());

        let expr_id = SymbolId(0);
        grammar.rule_names.insert(expr_id, "expression".to_string());

        // Parse query with captures
        let parser = QueryParser::new(&grammar, "(expression) @expr");
        let query = parser.parse().unwrap();

        assert_eq!(query.patterns.len(), 1);
        assert_eq!(query.capture_names.len(), 1);
        assert_eq!(query.capture_names.get("expr"), Some(&0));
    }

    #[test]
    fn test_query_parser_with_quantifiers() {
        let mut grammar = Grammar::new("test".to_string());

        let list_id = SymbolId(0);
        grammar.rule_names.insert(list_id, "list".to_string());

        let item_id = SymbolId(1);
        grammar.rule_names.insert(item_id, "item".to_string());

        // Parse query with quantifiers
        let parser = QueryParser::new(&grammar, "(list (item)*)");
        let query = parser.parse().unwrap();

        assert_eq!(query.patterns.len(), 1);
        let pattern = &query.patterns[0];
        assert_eq!(pattern.root.children.len(), 1);
        assert_eq!(pattern.root.children[0].quantifier, Quantifier::ZeroOrMore);
    }
}
