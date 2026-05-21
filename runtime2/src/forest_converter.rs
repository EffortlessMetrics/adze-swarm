//! Forest-to-Tree Conversion for GLR Parsing (Phase 3.2, Component 2)
//!
//! Contract: docs/specs/FOREST_CONVERTER_CONTRACT.md
//!
//! This module converts ParseForest (potentially containing multiple parse trees)
//! into a single Tree structure using disambiguation strategies.

use crate::Tree;
use crate::error::ParseError;
use crate::glr_engine::{ForestNode, ForestNodeId, ParseForest};
use crate::tree::TreeNode;
use adze_glr_core::SymbolId;
use std::collections::HashSet;
use std::fmt;

/// Disambiguation strategies for ambiguous parses
///
/// Contract: Determines which alternative to select when forest has
/// multiple valid parse trees
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisambiguationStrategy {
    /// Prefer shift over reduce (Tree-sitter default)
    ///
    /// Creates right-associative trees
    PreferShift,

    /// Prefer reduce over shift
    ///
    /// Creates left-associative trees
    PreferReduce,

    /// Use precedence from grammar (Phase 3.3)
    #[allow(dead_code)]
    Precedence,

    /// Take first alternative (fast but arbitrary)
    First,

    /// Reject ambiguity (return error)
    RejectAmbiguity,
}

/// Converts ParseForest to single Tree
///
/// Contract:
/// - Selects one parse tree from potentially multiple valid parses
/// - Applies disambiguation strategy consistently
/// - Preserves all node metadata
#[derive(Debug)]
pub struct ForestConverter {
    /// Disambiguation strategy
    strategy: DisambiguationStrategy,
}

/// Forest conversion errors
#[derive(Debug)]
pub enum ConversionError {
    /// Forest has no root nodes
    NoRoots,

    /// Ambiguous forest with multiple valid parses
    AmbiguousForest {
        /// Number of alternative parse trees in the forest
        count: usize,
    },

    /// Invalid forest structure
    InvalidForest {
        /// Description of the structural problem
        reason: String,
    },

    /// Invalid node reference
    InvalidNodeId {
        /// The invalid node ID that was referenced
        node_id: usize,
    },

    /// Cycle detected in forest
    #[allow(dead_code)]
    CycleDetected {
        /// The node ID where the cycle was detected
        node_id: usize,
    },
}

impl fmt::Display for ConversionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConversionError::NoRoots => write!(f, "Forest has no root nodes"),
            ConversionError::AmbiguousForest { count } => {
                write!(f, "Ambiguous forest: {} valid parses", count)
            }
            ConversionError::InvalidForest { reason } => {
                write!(f, "Invalid forest structure: {}", reason)
            }
            ConversionError::InvalidNodeId { node_id } => {
                write!(f, "Invalid node reference: {}", node_id)
            }
            ConversionError::CycleDetected { node_id } => {
                write!(f, "Cycle detected in forest at node {}", node_id)
            }
        }
    }
}

impl std::error::Error for ConversionError {}

impl From<ConversionError> for ParseError {
    fn from(err: ConversionError) -> Self {
        ParseError::with_msg(&err.to_string())
    }
}

impl ForestConverter {
    /// Create converter with strategy
    ///
    /// # Contract
    ///
    /// ## Postconditions
    /// - Converter ready to convert forests
    ///
    pub fn new(strategy: DisambiguationStrategy) -> Self {
        Self { strategy }
    }

    /// Convert ParseForest to Tree
    ///
    /// # Contract
    ///
    /// ## Preconditions
    /// - `forest.roots` is non-empty
    /// - Forest nodes form valid tree structure
    /// - All ForestNodeIds reference valid nodes
    ///
    /// ## Postconditions
    /// - Tree has single root node
    /// - Node ranges are consistent
    ///
    /// ## Algorithm
    ///
    /// Phase 1: Select root (disambiguation if multiple)
    /// Phase 2: Build tree via DFS traversal
    ///
    pub fn to_tree(&self, forest: &ParseForest, input: &[u8]) -> Result<Tree, ConversionError> {
        // Phase 1: Select root
        if forest.roots.is_empty() {
            return Err(ConversionError::NoRoots);
        }

        let selected_root = if forest.roots.len() == 1 {
            forest.roots[0]
        } else {
            // Multiple roots - apply disambiguation
            self.disambiguate_roots(&forest.roots, forest)?
        };

        // Phase 2: Build tree
        let mut visited = HashSet::new();
        let root_node = self.build_node(selected_root, forest, input, &mut visited)?;

        // Create tree
        let mut tree = Tree::new(root_node);
        tree.set_source(input.to_vec());

        Ok(tree)
    }

    /// Detect ambiguity in forest
    ///
    /// # Contract
    ///
    /// ## Returns
    /// - `None`: Unambiguous (single parse)
    /// - `Some(count)`: `count` alternative parses
    ///
    pub fn detect_ambiguity(&self, forest: &ParseForest) -> Option<usize> {
        // Check multiple roots
        if forest.roots.len() > 1 {
            return Some(forest.roots.len());
        }

        // Current struct-based ForestNode doesn't support Packed nodes yet
        // This will be added in Phase 3.3 when we refactor to enum
        // For now, only check multiple roots
        None
    }

    /// Disambiguate multiple roots
    fn disambiguate_roots(
        &self,
        roots: &[ForestNodeId],
        _forest: &ParseForest,
    ) -> Result<ForestNodeId, ConversionError> {
        self.select_disambiguated_node(roots, "root")
    }

    fn select_disambiguated_node(
        &self,
        nodes: &[ForestNodeId],
        node_kind: &str,
    ) -> Result<ForestNodeId, ConversionError> {
        let Some(first_node) = nodes.first().copied() else {
            return Err(ConversionError::InvalidForest {
                reason: format!("{node_kind} set is empty"),
            });
        };

        match self.strategy {
            DisambiguationStrategy::RejectAmbiguity if nodes.len() > 1 => {
                Err(ConversionError::AmbiguousForest { count: nodes.len() })
            }
            _ => Ok(first_node),
        }
    }

    /// Build node recursively
    fn build_node(
        &self,
        node_id: ForestNodeId,
        forest: &ParseForest,
        input: &[u8],
        visited: &mut HashSet<usize>,
    ) -> Result<TreeNode, ConversionError> {
        // Validate node ID
        if node_id.0 >= forest.nodes.len() {
            return Err(ConversionError::InvalidNodeId { node_id: node_id.0 });
        }

        // Cycle detection (commented out for now - can cause false positives in valid DAGs)
        // if visited.contains(&node_id.0) {
        //     return Err(ConversionError::CycleDetected { node_id: node_id.0 });
        // }
        visited.insert(node_id.0);

        let forest_node = &forest.nodes[node_id.0];

        // Current ForestNode is a struct (not enum)
        // Distinguish terminals from nonterminals by checking children
        if forest_node.children.is_empty() {
            // Terminal (leaf) node - no children
            Ok(TreeNode::new_with_children(
                forest_node.symbol.0 as u32,
                forest_node.range.start,
                forest_node.range.end,
                vec![],
            ))
        } else {
            // Nonterminal (internal) node - has children
            let mut child_nodes = Vec::new();
            for child_id in &forest_node.children {
                let child_node = self.build_node(*child_id, forest, input, visited)?;
                child_nodes.push(child_node);
            }

            // Use range from forest node (already calculated by GLR engine)
            Ok(TreeNode::new_with_children(
                forest_node.symbol.0 as u32,
                forest_node.range.start,
                forest_node.range.end,
                child_nodes,
            ))
        }
    }

    /// Disambiguate alternatives in Packed node
    fn disambiguate_alternatives(
        &self,
        alternatives: &[ForestNodeId],
        _forest: &ParseForest,
    ) -> Result<ForestNodeId, ConversionError> {
        if alternatives.is_empty() {
            return Err(ConversionError::InvalidForest {
                reason: "Packed node has no alternatives".to_string(),
            });
        }

        // For MVP, we don't have metadata about shift/reduce or precedence;
        // these strategies currently fall back to deterministic first-choice.
        self.select_disambiguated_node(alternatives, "alternative")
    }
}

// TreeNode accessor methods are defined in tree.rs

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glr_engine::ForestNodeId;

    #[test]
    fn test_disambiguation_strategy_equality() {
        assert_eq!(DisambiguationStrategy::First, DisambiguationStrategy::First);
        assert_ne!(
            DisambiguationStrategy::First,
            DisambiguationStrategy::PreferShift
        );
    }

    fn single_terminal_forest(symbol: u16, range: std::ops::Range<usize>) -> ParseForest {
        ParseForest {
            nodes: vec![ForestNode {
                symbol: SymbolId(symbol),
                children: vec![],
                range,
            }],
            roots: vec![ForestNodeId(0)],
        }
    }

    #[test]
    fn given_single_root_forest_when_converting_then_tree_preserves_root_symbol_range_and_source() {
        // Given
        let forest = single_terminal_forest(42, 1..3);
        let converter = ForestConverter::new(DisambiguationStrategy::First);

        // When
        let tree = converter
            .to_tree(&forest, b"xyz")
            .expect("conversion should succeed");

        // Then
        let root = tree.root_node();
        assert_eq!(root.kind_id(), 42);
        assert_eq!(root.byte_range(), 1..3);
        assert_eq!(tree.source_bytes(), Some("xyz".as_bytes()));
    }

    #[test]
    fn given_forest_with_no_roots_when_converting_then_returns_no_roots_error() {
        // Given
        let forest = ParseForest {
            nodes: vec![],
            roots: vec![],
        };
        let converter = ForestConverter::new(DisambiguationStrategy::First);

        // When
        let err = converter
            .to_tree(&forest, b"")
            .expect_err("forest without roots should fail");

        // Then
        assert!(matches!(err, ConversionError::NoRoots));
    }

    #[test]
    fn given_multiple_roots_and_reject_strategy_when_converting_then_returns_ambiguity_error() {
        // Given
        let forest = ParseForest {
            nodes: vec![
                ForestNode {
                    symbol: SymbolId(1),
                    children: vec![],
                    range: 0..1,
                },
                ForestNode {
                    symbol: SymbolId(2),
                    children: vec![],
                    range: 0..1,
                },
            ],
            roots: vec![ForestNodeId(0), ForestNodeId(1)],
        };
        let converter = ForestConverter::new(DisambiguationStrategy::RejectAmbiguity);

        // When
        let err = converter
            .to_tree(&forest, b"a")
            .expect_err("ambiguity should be rejected");

        // Then
        assert!(matches!(err, ConversionError::AmbiguousForest { count: 2 }));
    }

    #[test]
    fn given_multiple_roots_and_first_strategy_when_converting_then_first_root_is_selected() {
        // Given
        let forest = ParseForest {
            nodes: vec![
                ForestNode {
                    symbol: SymbolId(7),
                    children: vec![],
                    range: 0..1,
                },
                ForestNode {
                    symbol: SymbolId(8),
                    children: vec![],
                    range: 1..2,
                },
            ],
            roots: vec![ForestNodeId(0), ForestNodeId(1)],
        };
        let converter = ForestConverter::new(DisambiguationStrategy::First);

        // When
        let tree = converter
            .to_tree(&forest, b"ab")
            .expect("first strategy should select one root");

        // Then
        assert_eq!(tree.root_node().kind_id(), 7);
    }

    #[test]
    fn given_forest_with_invalid_child_reference_when_converting_then_returns_invalid_node_id() {
        // Given
        let forest = ParseForest {
            nodes: vec![ForestNode {
                symbol: SymbolId(9),
                children: vec![ForestNodeId(99)],
                range: 0..1,
            }],
            roots: vec![ForestNodeId(0)],
        };
        let converter = ForestConverter::new(DisambiguationStrategy::First);

        // When
        let err = converter
            .to_tree(&forest, b"a")
            .expect_err("invalid child reference should fail");

        // Then
        assert!(matches!(
            err,
            ConversionError::InvalidNodeId { node_id: 99 }
        ));
    }

    #[test]
    fn given_converter_when_detecting_ambiguity_then_multiple_roots_are_reported() {
        // Given
        let converter = ForestConverter::new(DisambiguationStrategy::First);
        let ambiguous = ParseForest {
            nodes: vec![
                ForestNode {
                    symbol: SymbolId(1),
                    children: vec![],
                    range: 0..1,
                },
                ForestNode {
                    symbol: SymbolId(2),
                    children: vec![],
                    range: 0..1,
                },
            ],
            roots: vec![ForestNodeId(0), ForestNodeId(1)],
        };
        let unambiguous = single_terminal_forest(1, 0..1);

        // When / Then
        assert_eq!(converter.detect_ambiguity(&ambiguous), Some(2));
        assert_eq!(converter.detect_ambiguity(&unambiguous), None);
    }
    #[test]
    fn given_empty_alternatives_when_disambiguating_then_returns_invalid_forest() {
        let converter = ForestConverter::new(DisambiguationStrategy::First);

        let err = converter
            .disambiguate_alternatives(
                &[],
                &ParseForest {
                    nodes: vec![],
                    roots: vec![],
                },
            )
            .expect_err("empty alternatives should fail");

        assert!(matches!(err, ConversionError::InvalidForest { .. }));
    }

    #[test]
    fn given_ambiguous_alternatives_and_reject_strategy_when_disambiguating_then_returns_ambiguity_error()
     {
        let converter = ForestConverter::new(DisambiguationStrategy::RejectAmbiguity);
        let alternatives = [ForestNodeId(0), ForestNodeId(1)];

        let err = converter
            .disambiguate_alternatives(
                &alternatives,
                &ParseForest {
                    nodes: vec![],
                    roots: vec![],
                },
            )
            .expect_err("reject strategy should fail for ambiguous alternatives");

        assert!(matches!(err, ConversionError::AmbiguousForest { count: 2 }));
    }
}
