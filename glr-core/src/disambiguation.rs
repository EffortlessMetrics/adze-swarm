//! Disambiguation of parse forests into single parse trees.

use crate::parse_forest::{ForestNode, ParseError, ParseForest, ParseNode, ParseTree};

impl ParseForest {
    /// Convert forest to single tree - for now just pick first complete parse
    pub fn to_single_tree(self) -> Result<ParseTree, ParseError> {
        // Find all complete parses (reached EOF at root symbol)
        let complete_parses: Vec<_> = self
            .roots
            .iter()
            .filter(|r| {
                if let Some(start) = self.grammar.start_symbol() {
                    r.symbol == start && r.is_complete()
                } else {
                    false
                }
            })
            .collect();

        if complete_parses.is_empty() {
            return Err(ParseError::Incomplete);
        }

        // For now: pick first. Later: scoring heuristics
        Ok(self.extract_tree(complete_parses[0]))
    }

    fn extract_tree(&self, root: &ForestNode) -> ParseTree {
        ParseTree {
            root: self.build_tree_node(root),
            source: self.source.clone(),
        }
    }

    fn build_tree_node(&self, forest_node: &ForestNode) -> ParseNode {
        // Take first alternative (later: scoring)
        let alt = &forest_node.alternatives[0];

        ParseNode {
            symbol: forest_node.symbol,
            span: forest_node.span,
            children: alt
                .children
                .iter()
                .map(|child_id| self.build_tree_node(&self.nodes[child_id]))
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse_forest::{ErrorMeta, ForestAlternative, ForestNode};
    use adze_ir::builder::GrammarBuilder;
    use adze_ir::{Grammar, SymbolId};
    use std::collections::HashMap;

    /// Build a tiny grammar with a single rule `s -> a` so `start_symbol()`
    /// returns the symbol id for `s`.
    fn tiny_grammar() -> (Grammar, SymbolId) {
        let grammar = GrammarBuilder::new("disambig_test")
            .token("a", "a")
            .rule("s", vec!["a"])
            .start("s")
            .build();
        let start = grammar.start_symbol().expect("start symbol present");
        (grammar, start)
    }

    fn leaf_node(id: usize, symbol: SymbolId, span: (usize, usize)) -> ForestNode {
        ForestNode {
            id,
            symbol,
            span,
            alternatives: vec![ForestAlternative { children: vec![] }],
            error_meta: ErrorMeta::default(),
        }
    }

    #[test]
    fn to_single_tree_returns_first_complete_root() {
        let (grammar, start) = tiny_grammar();
        let leaf = leaf_node(0, start, (0, 1));
        let mut nodes = HashMap::new();
        nodes.insert(0usize, leaf.clone());
        let forest = ParseForest {
            roots: vec![leaf],
            nodes,
            grammar,
            source: "a".to_string(),
            next_node_id: 1,
        };

        let tree = forest.to_single_tree().expect("complete parse");
        assert_eq!(tree.source, "a");
        assert_eq!(tree.root.symbol, start);
        assert_eq!(tree.root.span, (0, 1));
        assert!(tree.root.children.is_empty());
    }

    #[test]
    fn to_single_tree_recurses_into_children() {
        let (grammar, start) = tiny_grammar();
        // child node with id=1 (a leaf)
        let child = leaf_node(1, start, (0, 1));
        // root node with alternative pointing to child id=1
        let root = ForestNode {
            id: 0,
            symbol: start,
            span: (0, 1),
            alternatives: vec![ForestAlternative { children: vec![1] }],
            error_meta: ErrorMeta::default(),
        };
        let mut nodes = HashMap::new();
        nodes.insert(0usize, root.clone());
        nodes.insert(1usize, child);

        let forest = ParseForest {
            roots: vec![root],
            nodes,
            grammar,
            source: "a".to_string(),
            next_node_id: 2,
        };

        let tree = forest.to_single_tree().expect("complete parse");
        assert_eq!(tree.root.children.len(), 1);
        assert_eq!(tree.root.children[0].symbol, start);
        assert_eq!(tree.root.children[0].span, (0, 1));
        assert!(tree.root.children[0].children.is_empty());
    }

    #[test]
    fn to_single_tree_incomplete_when_no_roots_match_start() {
        let (grammar, start) = tiny_grammar();
        // Use a symbol that is not the start symbol.
        let other = SymbolId(start.0.wrapping_add(7));
        let root = leaf_node(0, other, (0, 1));
        let mut nodes = HashMap::new();
        nodes.insert(0usize, root.clone());
        let forest = ParseForest {
            roots: vec![root],
            nodes,
            grammar,
            source: "a".to_string(),
            next_node_id: 1,
        };

        match forest.to_single_tree() {
            Err(ParseError::Incomplete) => {}
            other => panic!("expected Incomplete, got {other:?}"),
        }
    }

    #[test]
    fn to_single_tree_incomplete_when_root_has_no_alternatives() {
        let (grammar, start) = tiny_grammar();
        // Root matches start symbol but has no alternatives → is_complete() == false.
        let root = ForestNode {
            id: 0,
            symbol: start,
            span: (0, 0),
            alternatives: vec![],
            error_meta: ErrorMeta::default(),
        };
        let mut nodes = HashMap::new();
        nodes.insert(0usize, root.clone());
        let forest = ParseForest {
            roots: vec![root],
            nodes,
            grammar,
            source: String::new(),
            next_node_id: 1,
        };

        assert!(matches!(
            forest.to_single_tree(),
            Err(ParseError::Incomplete)
        ));
    }

    #[test]
    fn to_single_tree_incomplete_when_grammar_has_no_start_symbol() {
        // Default (empty) grammar has no rules and therefore no start symbol.
        let grammar = Grammar::default();
        assert!(grammar.start_symbol().is_none());

        let forest = ParseForest {
            roots: vec![],
            nodes: HashMap::new(),
            grammar,
            source: String::new(),
            next_node_id: 0,
        };

        assert!(matches!(
            forest.to_single_tree(),
            Err(ParseError::Incomplete)
        ));
    }

    #[test]
    fn to_single_tree_incomplete_when_roots_are_empty() {
        let (grammar, _start) = tiny_grammar();
        let forest = ParseForest {
            roots: vec![],
            nodes: HashMap::new(),
            grammar,
            source: String::new(),
            next_node_id: 0,
        };

        assert!(matches!(
            forest.to_single_tree(),
            Err(ParseError::Incomplete)
        ));
    }

    #[test]
    fn to_single_tree_picks_first_complete_root_when_multiple_present() {
        let (grammar, start) = tiny_grammar();
        let first = ForestNode {
            id: 0,
            symbol: start,
            span: (0, 1),
            alternatives: vec![ForestAlternative { children: vec![] }],
            error_meta: ErrorMeta::default(),
        };
        let second = ForestNode {
            id: 1,
            symbol: start,
            span: (2, 3),
            alternatives: vec![ForestAlternative { children: vec![] }],
            error_meta: ErrorMeta::default(),
        };
        let mut nodes = HashMap::new();
        nodes.insert(0usize, first.clone());
        nodes.insert(1usize, second.clone());

        let forest = ParseForest {
            roots: vec![first, second],
            nodes,
            grammar,
            source: "a a".to_string(),
            next_node_id: 2,
        };

        let tree = forest.to_single_tree().expect("complete parse");
        // The first complete root is preferred; observed by its span.
        assert_eq!(tree.root.span, (0, 1));
    }
}
