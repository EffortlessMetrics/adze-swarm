// Query pattern matching implementation
use super::ast::*;
use crate::parser_v4::ParseNode;
use std::collections::HashMap;

/// A match of a query pattern
#[derive(Debug, Clone)]
pub struct QueryMatch {
    /// Pattern index that matched
    pub pattern_index: usize,
    /// Captured nodes
    pub captures: Vec<QueryCapture>,
}

/// A captured node in a query match
#[derive(Debug, Clone)]
pub struct QueryCapture {
    /// Capture index
    pub index: u32,
    /// The captured node
    pub node: ParseNode,
}

/// State for matching a pattern
#[derive(Debug)]
struct MatchState {
    /// Current captures
    captures: HashMap<u32, ParseNode>,
    /// Whether the match succeeded
    #[allow(dead_code)]
    success: bool,
}

/// Query pattern matcher
pub struct QueryMatcher<'a> {
    query: &'a Query,
}

fn node_overlaps_range(node: &ParseNode, range: &std::ops::Range<usize>) -> bool {
    node.start_byte <= range.end && node.end_byte >= range.start
}

fn match_overlaps_range(
    captures: &HashMap<u32, ParseNode>,
    node: &ParseNode,
    range: &std::ops::Range<usize>,
) -> bool {
    if captures.is_empty() {
        return node_overlaps_range(node, range);
    }

    captures
        .values()
        .any(|capture| node_overlaps_range(capture, range))
}

impl<'a> QueryMatcher<'a> {
    /// Create a new query matcher
    pub fn new(query: &'a Query) -> Self {
        QueryMatcher { query }
    }

    /// Match all patterns in the query against a parse tree
    pub fn matches(&self, root: &ParseNode) -> Vec<QueryMatch> {
        self.matches_with_options(root, None, false)
    }

    /// Match all patterns in the query against a parse tree using cursor options.
    pub(crate) fn matches_with_options(
        &self,
        root: &ParseNode,
        byte_range: Option<&std::ops::Range<usize>>,
        match_root: bool,
    ) -> Vec<QueryMatch> {
        let mut matches = Vec::new();

        // Try each pattern
        for (pattern_index, pattern) in self.query.patterns.iter().enumerate() {
            self.match_pattern(
                pattern_index,
                pattern,
                root,
                byte_range,
                match_root,
                &mut matches,
            );
        }

        matches
    }

    /// Match a single pattern against the tree
    fn match_pattern(
        &self,
        pattern_index: usize,
        pattern: &Pattern,
        root: &ParseNode,
        byte_range: Option<&std::ops::Range<usize>>,
        match_root: bool,
        matches: &mut Vec<QueryMatch>,
    ) {
        // Walk the tree and try to match at each node
        self.match_pattern_at_node(
            pattern_index,
            pattern,
            root,
            byte_range,
            match_root,
            matches,
        );
    }

    /// Try to match pattern starting at a specific node
    fn match_pattern_at_node(
        &self,
        pattern_index: usize,
        pattern: &Pattern,
        node: &ParseNode,
        byte_range: Option<&std::ops::Range<usize>>,
        match_root: bool,
        matches: &mut Vec<QueryMatch>,
    ) {
        if let Some(range) = byte_range
            && !node_overlaps_range(node, range)
        {
            return;
        }

        // Try to match the pattern at this node
        let mut state = MatchState {
            captures: HashMap::new(),
            success: false,
        };

        if self.match_node(&pattern.root, node, &mut state)
            && byte_range.is_none_or(|range| match_overlaps_range(&state.captures, node, range))
        {
            // Check predicates
            if self.check_predicates(&pattern.predicates, &state.captures) {
                // Create match
                let mut captures: Vec<_> = state
                    .captures
                    .into_iter()
                    .map(|(index, node)| QueryCapture { index, node })
                    .collect();
                captures.sort_by_key(|c| c.index);

                matches.push(QueryMatch {
                    pattern_index,
                    captures,
                });
            }
        }

        if match_root {
            return;
        }

        // Recursively try to match in children
        for child in &node.children {
            self.match_pattern_at_node(pattern_index, pattern, child, byte_range, false, matches);
        }
    }

    /// Match a pattern node against a parse node
    fn match_node(&self, pattern: &PatternNode, node: &ParseNode, state: &mut MatchState) -> bool {
        // Check symbol match
        if pattern.symbol != node.symbol {
            return false;
        }

        // Capture if needed
        if let Some(capture_id) = pattern.capture {
            state.captures.insert(capture_id, node.clone());
        }

        // Match children based on quantifier
        match pattern.quantifier {
            Quantifier::One => self.match_children_one(pattern, node, state),
            Quantifier::Optional => self.match_children_optional(pattern, node, state),
            Quantifier::Plus => self.match_children_plus(pattern, node, state),
            Quantifier::Star => self.match_children_star(pattern, node, state),
        }
    }

    /// Match children with One quantifier
    fn match_children_one(
        &self,
        pattern: &PatternNode,
        node: &ParseNode,
        state: &mut MatchState,
    ) -> bool {
        // Check field assertions
        for (field_name, field_pattern) in &pattern.fields {
            // Find child with this field name
            let field_node = node
                .children
                .iter()
                .find(|child| child.field_name.as_ref() == Some(field_name));

            if let Some(field_node) = field_node {
                if !self.match_node(field_pattern, field_node, state) {
                    return false;
                }
            } else {
                return false; // Required field not found
            }
        }

        // If pattern has explicit children, match them
        if !pattern.children.is_empty() {
            return self.match_child_sequence(&pattern.children, &node.children, 0, 0, state);
        }

        true
    }

    /// Match children with Optional quantifier
    fn match_children_optional(
        &self,
        pattern: &PatternNode,
        node: &ParseNode,
        state: &mut MatchState,
    ) -> bool {
        // Optional always matches, but we try to match if possible
        self.match_children_one(pattern, node, state);
        true
    }

    /// Match children with Plus quantifier
    fn match_children_plus(
        &self,
        pattern: &PatternNode,
        node: &ParseNode,
        state: &mut MatchState,
    ) -> bool {
        // Must match at least once
        if !self.match_children_one(pattern, node, state) {
            return false;
        }

        // Try to match more (simplified - in reality would need backtracking)
        true
    }

    /// Match children with Star quantifier
    fn match_children_star(
        &self,
        pattern: &PatternNode,
        node: &ParseNode,
        state: &mut MatchState,
    ) -> bool {
        // Star always matches (zero or more)
        self.match_children_plus(pattern, node, state);
        true
    }

    /// Match a sequence of child patterns
    fn match_child_sequence(
        &self,
        patterns: &[PatternChild],
        nodes: &[ParseNode],
        pattern_idx: usize,
        node_idx: usize,
        state: &mut MatchState,
    ) -> bool {
        // Base case: all patterns matched
        if pattern_idx >= patterns.len() {
            return node_idx >= nodes.len(); // All nodes must be consumed
        }

        // Base case: no more nodes but patterns remain
        if node_idx >= nodes.len() {
            // Check if remaining patterns are all optional
            for i in pattern_idx..patterns.len() {
                if let PatternChild::Node(ref pattern_node) = patterns[i]
                    && pattern_node.quantifier != Quantifier::Optional
                    && pattern_node.quantifier != Quantifier::Star
                {
                    return false;
                }
            }
            return true;
        }

        match &patterns[pattern_idx] {
            PatternChild::Token(_) => false,
            PatternChild::Node(pattern_node) => {
                // Try to match this pattern node
                if self.match_node(pattern_node, &nodes[node_idx], state) {
                    self.match_child_sequence(patterns, nodes, pattern_idx + 1, node_idx + 1, state)
                } else {
                    false
                }
            }
        }
    }

    /// Check if predicates are satisfied
    fn check_predicates(
        &self,
        predicates: &[Predicate],
        captures: &HashMap<u32, ParseNode>,
    ) -> bool {
        for predicate in predicates {
            if !self.check_predicate(predicate, captures) {
                return false;
            }
        }
        true
    }

    /// Check a single predicate
    fn check_predicate(&self, predicate: &Predicate, captures: &HashMap<u32, ParseNode>) -> bool {
        match predicate {
            Predicate::Eq {
                capture1,
                capture2,
                value,
            } => {
                if let Some(node1) = captures.get(capture1) {
                    if let Some(capture2) = capture2 {
                        if let Some(node2) = captures.get(capture2) {
                            // Compare node texts (simplified)
                            return node1.start_byte == node2.start_byte
                                && node1.end_byte == node2.end_byte;
                        }
                    } else if let Some(_value) = value {
                        // Compare node text with value
                        // In real implementation, would extract actual text
                        return true;
                    }
                }
                false
            }
            Predicate::NotEq {
                capture1,
                capture2,
                value,
            } => !self.check_predicate(
                &Predicate::Eq {
                    capture1: *capture1,
                    capture2: *capture2,
                    value: value.clone(),
                },
                captures,
            ),
            Predicate::Match {
                capture: _,
                regex: _,
            } => {
                // In real implementation, would compile regex and match
                true
            }
            Predicate::NotMatch { capture, regex } => !self.check_predicate(
                &Predicate::Match {
                    capture: *capture,
                    regex: regex.clone(),
                },
                captures,
            ),
            _ => {
                // Other predicates not implemented yet
                true
            }
        }
    }
}

/// Iterator over query matches
pub struct QueryMatches<'a> {
    #[allow(dead_code)]
    matcher: QueryMatcher<'a>,
    #[allow(dead_code)]
    root: &'a ParseNode,
    #[allow(dead_code)]
    pattern_index: usize,
    matches: Vec<QueryMatch>,
    current_index: usize,
}

impl<'a> QueryMatches<'a> {
    /// Create a new query matches iterator
    pub fn new(query: &'a Query, root: &'a ParseNode) -> Self {
        Self::new_with_options(query, root, None, false)
    }

    /// Create a new query matches iterator using cursor options.
    pub(crate) fn new_with_options(
        query: &'a Query,
        root: &'a ParseNode,
        byte_range: Option<&std::ops::Range<usize>>,
        match_root: bool,
    ) -> Self {
        let matcher = QueryMatcher::new(query);
        let matches = matcher.matches_with_options(root, byte_range, match_root);
        QueryMatches {
            matcher,
            root,
            pattern_index: 0,
            matches,
            current_index: 0,
        }
    }
}

impl<'a> Iterator for QueryMatches<'a> {
    type Item = QueryMatch;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current_index < self.matches.len() {
            let match_item = self.matches[self.current_index].clone();
            self.current_index += 1;
            Some(match_item)
        } else {
            None
        }
    }
}
