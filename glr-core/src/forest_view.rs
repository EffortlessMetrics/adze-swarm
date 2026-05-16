//! Object-safe view over a GLR forest/SPPF used by downstream runtimes.
#![cfg_attr(feature = "strict_docs", allow(missing_docs))]

pub(crate) mod sealed {
    pub trait Sealed {}
}

/// Numeric symbol id (terminals and nonterminals share the space).
pub type SymbolId = u32;

/// Byte span in input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: u32,
    pub end: u32,
}

/// Object-safe view of a forest/SPPF.
///
/// Notes:
/// - We keep ambiguity handling simple for now: `best_children` returns one
///   chosen family (e.g., first/longest/leftmost). You can extend this later
///   with explicit "families" if you want full ambiguity exposure.
/// - This trait's shape is stable across all build configurations.
/// - This trait is sealed and cannot be implemented outside this crate.
pub trait ForestView: sealed::Sealed + Send + Sync {
    /// Root candidate nodes (usually 1).
    fn roots(&self) -> &[u32];
    /// Symbol kind for a node id.
    fn kind(&self, id: u32) -> SymbolId;
    /// Byte span for a node id.
    fn span(&self, id: u32) -> Span;
    /// Children chosen for the best family.
    fn best_children(&self, id: u32) -> &[u32];
}

/// Test hooks for Forest (only available in test builds).
#[cfg(any(test, feature = "test-api", feature = "test_helpers"))]
pub struct ForestTestHooks {
    /// Cached error stats from the forest.
    /// (has_error_chunks, missing_terminals, total_error_cost).
    pub error_stats: (bool, usize, u32),
}

/// Opaque forest handle exported to consumers via trait object.
pub struct Forest {
    pub(crate) view: Box<dyn ForestView>,
    #[cfg(any(test, feature = "test-api", feature = "test_helpers"))]
    pub(crate) test_hooks: Option<ForestTestHooks>,
}

impl Forest {
    /// Returns a read-only view of the parse forest.
    pub fn view(&self) -> &dyn ForestView {
        &*self.view
    }

    /// Test helper: returns (has_error_chunks, missing_terminals, total_error_cost)
    /// Only available in test builds. Not part of the stable runtime API.
    #[cfg(any(test, feature = "test-api", feature = "test_helpers"))]
    pub fn debug_error_stats(&self) -> (bool, usize, u32) {
        let hooks = self
            .test_hooks
            .as_ref()
            .expect("Forest built without test hooks");
        hooks.error_stats
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::driver::Driver;
    use crate::parse_forest::{
        ERROR_SYMBOL, ErrorMeta, ForestAlternative, ForestNode, ParseForest,
    };
    use adze_ir::builder::GrammarBuilder;
    use adze_ir::{Grammar, SymbolId as IrSymbolId};
    use std::collections::HashMap;

    fn tiny_grammar() -> (Grammar, IrSymbolId) {
        let grammar = GrammarBuilder::new("forest_view_test")
            .token("a", "a")
            .rule("s", vec!["a"])
            .start("s")
            .build();
        let start = grammar.start_symbol().expect("start symbol present");
        (grammar, start)
    }

    fn leaf(id: usize, symbol: IrSymbolId, span: (usize, usize)) -> ForestNode {
        ForestNode {
            id,
            symbol,
            span,
            alternatives: vec![ForestAlternative { children: vec![] }],
            error_meta: ErrorMeta::default(),
        }
    }

    /// Build a minimal forest with one root node and one child, then wrap it.
    fn wrap_minimal_forest() -> Forest {
        let (grammar, start) = tiny_grammar();
        let child = leaf(1, start, (0, 1));
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

        Driver::wrap_forest(forest)
    }

    #[test]
    fn span_struct_supports_equality_copy_and_debug() {
        let a = Span { start: 0, end: 5 };
        let b = a;
        assert_eq!(a, b);
        assert_ne!(a, Span { start: 1, end: 5 });
        // Debug should mention both fields.
        let dbg = format!("{a:?}");
        assert!(dbg.contains("start"), "debug missing start: {dbg}");
        assert!(dbg.contains("end"), "debug missing end: {dbg}");
        // Clone roundtrip preserves fields.
        let c = a;
        assert_eq!(c.start, 0);
        assert_eq!(c.end, 5);
    }

    #[test]
    fn forest_view_returns_root_id() {
        let forest = wrap_minimal_forest();
        let view = forest.view();
        let roots = view.roots();
        assert_eq!(roots, &[0u32]);
    }

    #[test]
    fn forest_view_kind_and_span_match_underlying_node() {
        let (_grammar, start) = tiny_grammar();
        let forest = wrap_minimal_forest();
        let view = forest.view();

        // Root node id == 0.
        assert_eq!(view.kind(0), u32::from(start.0));
        assert_eq!(view.span(0), Span { start: 0, end: 1 });
        // Child node id == 1.
        assert_eq!(view.kind(1), u32::from(start.0));
        assert_eq!(view.span(1), Span { start: 0, end: 1 });
    }

    #[test]
    fn forest_view_unknown_node_returns_zero_kind_and_empty_span() {
        let forest = wrap_minimal_forest();
        let view = forest.view();
        // No node with id 999.
        assert_eq!(view.kind(999), 0u32);
        assert_eq!(view.span(999), Span { start: 0, end: 0 });
        // best_children returns an empty slice for unknown ids.
        assert!(view.best_children(999).is_empty());
    }

    #[test]
    fn forest_view_best_children_returns_first_alternative() {
        let forest = wrap_minimal_forest();
        let view = forest.view();
        // Root has a single alternative pointing to child id 1.
        assert_eq!(view.best_children(0), &[1u32]);
        // Leaf child has no children.
        assert!(view.best_children(1).is_empty());
    }

    #[test]
    fn forest_debug_error_stats_returns_zero_for_clean_forest() {
        let forest = wrap_minimal_forest();
        assert_eq!(forest.debug_error_stats(), (false, 0, 0));
    }

    #[test]
    fn forest_debug_error_stats_surfaces_error_chunks() {
        let (grammar, _start) = tiny_grammar();
        // A single error chunk node serving as the root.
        let err_node = ForestNode {
            id: 0,
            symbol: ERROR_SYMBOL,
            span: (0, 1),
            alternatives: vec![ForestAlternative { children: vec![] }],
            error_meta: ErrorMeta {
                is_error: true,
                missing: false,
                cost: 3,
            },
        };
        let mut nodes = HashMap::new();
        nodes.insert(0usize, err_node.clone());

        let forest = ParseForest {
            roots: vec![err_node],
            nodes,
            grammar,
            source: "x".to_string(),
            next_node_id: 1,
        };

        let wrapped = Driver::wrap_forest(forest);
        let (has_err, missing, cost) = wrapped.debug_error_stats();
        assert!(has_err, "expected error chunk to be reported");
        assert_eq!(missing, 0);
        assert_eq!(cost, 3);
    }

    #[test]
    fn forest_view_handles_empty_forest() {
        let grammar = Grammar::default();
        let forest = ParseForest {
            roots: vec![],
            nodes: HashMap::new(),
            grammar,
            source: String::new(),
            next_node_id: 0,
        };

        let wrapped = Driver::wrap_forest(forest);
        let view = wrapped.view();
        assert!(view.roots().is_empty());
        assert_eq!(view.kind(0), 0);
        assert_eq!(view.span(0), Span { start: 0, end: 0 });
        assert!(view.best_children(0).is_empty());
    }
}
