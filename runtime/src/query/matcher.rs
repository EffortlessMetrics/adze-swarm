//! Source-free compatibility adapter to the canonical query engine.
use super::ast::*;
use crate::parser_v4::ParseNode;

/// A match of a query pattern against a source-free parse tree.
///
/// This is the legacy tree-matcher shape: captures hold [`ParseNode`]s and
/// predicates that need source text fail closed. It shares its name — but not
/// its definition — with [`matcher_v2::QueryMatch`](super::matcher_v2::QueryMatch)
/// (source-aware; the recommended primary, see #861) and with
/// [`glr_query::QueryMatch`](crate::glr_query::QueryMatch) (GLR-forest
/// `Subtree` captures). Do not unify these types here; unification is #861's scope.
#[derive(Debug, Clone)]
pub struct QueryMatch {
    /// Pattern index that matched
    pub pattern_index: usize,
    /// Captured nodes
    pub captures: Vec<QueryCapture>,
}

/// A captured node in a source-free query match.
///
/// Pairs a capture index with its [`ParseNode`]. Distinct from
/// [`matcher_v2::QueryCapture`](super::matcher_v2::QueryCapture) and
/// [`glr_query::QueryCapture`](crate::glr_query::QueryCapture), which share
/// the name but capture different node representations.
#[derive(Debug, Clone)]
pub struct QueryCapture {
    /// Capture index
    pub index: u32,
    /// The captured node
    pub node: ParseNode,
}

/// Source-free compatibility matcher over caller-owned recursive nodes.
///
/// Matching, occurrence ordering and rollback use the canonical engine. No
/// source text or named/extra metadata is fabricated. Text predicates, including
/// capture-to-capture equality/inequality, fail closed: spans are not text.
/// Use `matcher_v2::QueryMatcher` with source or `matches_document` for text.
/// Result shapes retain owned node payloads for public API compatibility only.
/// The two matchers (and their `QueryMatch`/`QueryMatches` types) are
/// intentionally distinct; do not merge them in this module (#861 scope).
pub struct QueryMatcher<'a> {
    query: &'a Query,
}

impl<'a> QueryMatcher<'a> {
    /// Create a source-free compatibility matcher.
    pub fn new(query: &'a Query) -> Self {
        Self { query }
    }

    /// Match patterns using canonical occurrence semantics without source text.
    pub fn matches(&self, root: &ParseNode) -> Vec<QueryMatch> {
        self.matches_with_options(root, None, false)
    }

    pub(crate) fn matches_with_options(
        &self,
        root: &ParseNode,
        byte_range: Option<&std::ops::Range<usize>>,
        match_root: bool,
    ) -> Vec<QueryMatch> {
        super::matcher_v2::QueryMatcher::source_free(self.query)
            .matches_with_options(root, byte_range, match_root)
            .into_iter()
            .map(|matched| QueryMatch {
                pattern_index: matched.pattern_index,
                captures: matched
                    .captures
                    .into_iter()
                    .map(|capture| QueryCapture {
                        index: capture.index,
                        node: capture.node,
                    })
                    .collect(),
            })
            .collect()
    }
}

/// Iterator over source-free query matches.
///
/// Yields [`QueryMatch`] items produced by [`QueryMatcher`]. Distinct from
/// [`matcher_v2::QueryMatches`](super::matcher_v2::QueryMatches)
/// (source-aware) and the private GLR-forest iterator in
/// [`glr_query`](crate::glr_query); the shared name is a known collision
/// tracked by #871 §d, not an alias.
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
