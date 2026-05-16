//! Subtree representation and manipulation.
#![cfg_attr(feature = "strict_docs", allow(missing_docs))]

// Subtree representation with dynamic precedence support

use adze_ir::SymbolId;
use smallvec::SmallVec;
use std::sync::Arc;

/// Node information for a subtree
#[derive(Debug, Clone)]
pub struct SubtreeNode {
    /// Symbol ID for this node
    pub symbol_id: SymbolId,

    /// Whether this node is an error node
    pub is_error: bool,

    /// Byte range in source text
    pub byte_range: std::ops::Range<usize>,
}

/// A child edge with optional field information
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct ChildEdge {
    /// The child subtree
    pub subtree: Arc<Subtree>,

    /// Field ID for this child (u16::MAX means no field)
    pub field_id: u16,
}

/// Constant representing "no field" for a child edge
pub const FIELD_NONE: u16 = u16::MAX;

impl ChildEdge {
    /// Create a new ChildEdge
    pub fn new(subtree: Arc<Subtree>, field_id: u16) -> Self {
        Self { subtree, field_id }
    }

    /// Create a ChildEdge without a field
    pub fn new_without_field(subtree: Arc<Subtree>) -> Self {
        Self {
            subtree,
            field_id: FIELD_NONE,
        }
    }
}

/// A subtree in the parse tree, potentially with dynamic precedence
#[derive(Debug, Clone)]
pub struct Subtree {
    /// The tree node data
    pub node: SubtreeNode,

    /// Dynamic precedence value for this subtree
    /// Set by prec.dynamic(n) annotations in the grammar
    pub dynamic_prec: i32,

    /// Child subtrees with optional field information
    pub children: Vec<ChildEdge>,

    /// Alternative parse trees for ambiguous nodes
    /// Empty = single parse, non-empty = ambiguity pack
    pub alternatives: SmallVec<[Arc<Subtree>; 2]>,
}

#[allow(dead_code)]
impl Subtree {
    /// Create a new subtree with the given node and children (no field info)
    pub fn new(node: SubtreeNode, children: Vec<Arc<Subtree>>) -> Self {
        // Convert to ChildEdge with no field
        let children_with_fields = children
            .into_iter()
            .map(|subtree| ChildEdge {
                subtree,
                field_id: FIELD_NONE,
            })
            .collect::<Vec<_>>();

        // Propagate dynamic precedence upward (max of children)
        let max_child_prec = children_with_fields
            .iter()
            .map(|c| c.subtree.dynamic_prec)
            .max()
            .unwrap_or(0);

        Self {
            node,
            dynamic_prec: max_child_prec,
            children: children_with_fields,
            alternatives: SmallVec::new(),
        }
    }

    /// Create a new subtree with field information for children
    pub fn new_with_fields(node: SubtreeNode, children: Vec<ChildEdge>) -> Self {
        // Propagate dynamic precedence upward (max of children)
        let max_child_prec = children
            .iter()
            .map(|c| c.subtree.dynamic_prec)
            .max()
            .unwrap_or(0);

        Self {
            node,
            dynamic_prec: max_child_prec,
            children,
            alternatives: SmallVec::new(),
        }
    }

    /// Create a new subtree with explicit dynamic precedence (no field info)
    pub fn with_dynamic_prec(
        node: SubtreeNode,
        children: Vec<Arc<Subtree>>,
        dynamic_prec: i32,
    ) -> Self {
        // Convert to ChildEdge with no field
        let children_with_fields = children
            .into_iter()
            .map(|subtree| ChildEdge {
                subtree,
                field_id: FIELD_NONE,
            })
            .collect::<Vec<_>>();

        // Take max of explicit precedence and children's precedence
        let max_child_prec = children_with_fields
            .iter()
            .map(|c| c.subtree.dynamic_prec)
            .max()
            .unwrap_or(0);

        Self {
            node,
            dynamic_prec: dynamic_prec.max(max_child_prec),
            children: children_with_fields,
            alternatives: SmallVec::new(),
        }
    }

    /// Create a new subtree with explicit dynamic precedence and field info
    pub fn with_dynamic_prec_and_fields(
        node: SubtreeNode,
        children: Vec<ChildEdge>,
        dynamic_prec: i32,
    ) -> Self {
        // Take max of explicit precedence and children's precedence
        let max_child_prec = children
            .iter()
            .map(|c| c.subtree.dynamic_prec)
            .max()
            .unwrap_or(0);

        Self {
            node,
            dynamic_prec: dynamic_prec.max(max_child_prec),
            children,
            alternatives: SmallVec::new(),
        }
    }

    /// Get the symbol ID for this subtree
    pub fn symbol(&self) -> u16 {
        self.node.symbol_id.0
    }

    /// Check if this subtree is in error
    pub fn is_error(&self) -> bool {
        self.node.is_error
    }

    /// Get the byte range for this subtree
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.node.byte_range.clone()
    }

    /// Check if this subtree has ambiguous alternatives
    pub fn is_ambiguous(&self) -> bool {
        !self.alternatives.is_empty()
    }

    /// Check if this subtree has alternatives
    pub fn has_alts(&self) -> bool {
        !self.alternatives.is_empty()
    }

    /// Get all alternatives (not including the primary tree)
    pub fn alternatives_iter(&self) -> impl Iterator<Item = &Arc<Subtree>> {
        self.alternatives.iter()
    }

    /// Merge two subtrees with the same top, preserving all alternatives
    pub fn merge_ambiguous(mut self, other: Arc<Subtree>) -> Self {
        // If the other tree also has alternatives, merge them all
        if !other.alternatives.is_empty() {
            for alt in &other.alternatives {
                if !self
                    .alternatives
                    .iter()
                    .any(|a: &Arc<Subtree>| Arc::ptr_eq(a, alt))
                {
                    self.alternatives.push(alt.clone());
                }
            }
        }

        // Add the other tree itself as an alternative (if not already present)
        // Need to check by pointer equality since we're moving other
        let other_ptr = Arc::as_ptr(&other);
        if !self
            .alternatives
            .iter()
            .any(|a: &Arc<Subtree>| Arc::as_ptr(a) == other_ptr)
        {
            // Keep the highest dynamic precedence before moving
            self.dynamic_prec = self.dynamic_prec.max(other.dynamic_prec);
            self.alternatives.push(other);
        } else {
            // Still update precedence even if not adding
            self.dynamic_prec = self.dynamic_prec.max(other.dynamic_prec);
        }

        self
    }

    /// Create a new subtree with the given alternative
    pub fn with_alts(mut self, alt: Arc<Subtree>) -> Self {
        if !self.alternatives.iter().any(|a| Arc::ptr_eq(a, &alt)) {
            self.alternatives.push(alt);
        }
        self
    }

    /// Add an alternative to this subtree (deduplicating by pointer)
    pub fn push_alt(mut self, alt: Arc<Subtree>) -> Self {
        let alt_ptr = Arc::as_ptr(&alt);
        if !self.alternatives.iter().any(|a| Arc::as_ptr(a) == alt_ptr) {
            self.dynamic_prec = self.dynamic_prec.max(alt.dynamic_prec);
            self.alternatives.push(alt);
        }
        self
    }

    /// Concatenate alternatives from two subtrees (deduplicating)
    pub fn concat_alts(mut self, other: Arc<Subtree>) -> Self {
        // First add the other tree as an alternative
        self = self.push_alt(other.clone());

        // Then add all of its alternatives
        for alt in &other.alternatives {
            if !self
                .alternatives
                .iter()
                .any(|a: &Arc<Subtree>| Arc::ptr_eq(a, alt))
            {
                self.alternatives.push(alt.clone());
            }
        }

        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mk_node(sym: u16, range: std::ops::Range<usize>, is_error: bool) -> SubtreeNode {
        SubtreeNode {
            symbol_id: SymbolId(sym),
            is_error,
            byte_range: range,
        }
    }

    fn mk_leaf(sym: u16, range: std::ops::Range<usize>) -> Arc<Subtree> {
        Arc::new(Subtree::new(mk_node(sym, range, false), Vec::new()))
    }

    fn mk_leaf_with_prec(sym: u16, range: std::ops::Range<usize>, prec: i32) -> Arc<Subtree> {
        Arc::new(Subtree::with_dynamic_prec(
            mk_node(sym, range, false),
            Vec::new(),
            prec,
        ))
    }

    #[test]
    fn subtree_new_leaf_has_no_children_and_zero_prec() {
        let leaf = Subtree::new(mk_node(1, 0..3, false), Vec::new());
        assert_eq!(leaf.symbol(), 1);
        assert!(leaf.children.is_empty());
        assert_eq!(leaf.dynamic_prec, 0);
        assert!(leaf.alternatives.is_empty());
        assert!(!leaf.is_ambiguous());
        assert!(!leaf.has_alts());
        assert!(!leaf.is_error());
    }

    #[test]
    fn subtree_new_error_node_reports_is_error() {
        let err = Subtree::new(mk_node(7, 5..9, true), Vec::new());
        assert!(err.is_error());
        assert_eq!(err.symbol(), 7);
        assert_eq!(err.byte_range(), 5..9);
    }

    #[test]
    fn subtree_byte_range_returns_clone_of_node_range() {
        let leaf = mk_leaf(2, 10..20);
        let r = leaf.byte_range();
        assert_eq!(r.start, 10);
        assert_eq!(r.end, 20);
    }

    #[test]
    fn subtree_new_propagates_max_child_dynamic_prec() {
        let c1 = mk_leaf_with_prec(1, 0..1, 3);
        let c2 = mk_leaf_with_prec(2, 1..2, 7);
        let c3 = mk_leaf_with_prec(3, 2..3, 5);
        let parent = Subtree::new(mk_node(10, 0..3, false), vec![c1, c2, c3]);
        assert_eq!(parent.dynamic_prec, 7);
        assert_eq!(parent.children.len(), 3);
        // Children created via `new` have FIELD_NONE
        for edge in &parent.children {
            assert_eq!(edge.field_id, FIELD_NONE);
        }
    }

    #[test]
    fn subtree_new_with_fields_preserves_field_ids_and_propagates_prec() {
        let c1 = mk_leaf_with_prec(1, 0..1, 2);
        let c2 = mk_leaf_with_prec(2, 1..2, 4);
        let edges = vec![
            ChildEdge::new(c1, 11),
            ChildEdge::new_without_field(c2.clone()),
        ];
        let parent = Subtree::new_with_fields(mk_node(20, 0..2, false), edges);
        assert_eq!(parent.children.len(), 2);
        assert_eq!(parent.children[0].field_id, 11);
        assert_eq!(parent.children[1].field_id, FIELD_NONE);
        assert_eq!(parent.dynamic_prec, 4);
    }

    #[test]
    fn subtree_new_with_fields_empty_children_zero_prec() {
        let parent = Subtree::new_with_fields(mk_node(42, 0..0, false), Vec::new());
        assert_eq!(parent.dynamic_prec, 0);
        assert!(parent.children.is_empty());
    }

    #[test]
    fn subtree_with_dynamic_prec_uses_max_of_explicit_and_children() {
        // explicit lower than children: child wins
        let child = mk_leaf_with_prec(1, 0..1, 9);
        let s = Subtree::with_dynamic_prec(mk_node(10, 0..1, false), vec![child], 3);
        assert_eq!(s.dynamic_prec, 9);

        // explicit higher than children: explicit wins
        let child2 = mk_leaf_with_prec(2, 0..1, 1);
        let s2 = Subtree::with_dynamic_prec(mk_node(10, 0..1, false), vec![child2], 100);
        assert_eq!(s2.dynamic_prec, 100);
    }

    #[test]
    fn subtree_with_dynamic_prec_no_children_uses_explicit() {
        let s = Subtree::with_dynamic_prec(mk_node(5, 0..0, false), Vec::new(), 42);
        assert_eq!(s.dynamic_prec, 42);
        assert!(s.children.is_empty());
    }

    #[test]
    fn subtree_with_dynamic_prec_negative_explicit() {
        // negative explicit, no children -> explicit (since max of [] is 0,
        // and max(-5, 0) == 0)
        let s = Subtree::with_dynamic_prec(mk_node(5, 0..0, false), Vec::new(), -5);
        assert_eq!(s.dynamic_prec, 0);
    }

    #[test]
    fn subtree_with_dynamic_prec_and_fields_picks_max() {
        let c1 = mk_leaf_with_prec(1, 0..1, 8);
        let edges = vec![ChildEdge::new(c1, 3)];
        let s = Subtree::with_dynamic_prec_and_fields(mk_node(10, 0..1, false), edges, 5);
        assert_eq!(s.dynamic_prec, 8);
        assert_eq!(s.children[0].field_id, 3);

        let c2 = mk_leaf_with_prec(2, 0..1, 1);
        let edges2 = vec![ChildEdge::new(c2, 4)];
        let s2 = Subtree::with_dynamic_prec_and_fields(mk_node(10, 0..1, false), edges2, 50);
        assert_eq!(s2.dynamic_prec, 50);
    }

    #[test]
    fn child_edge_constructors_set_field_correctly() {
        let leaf = mk_leaf(1, 0..1);
        let e1 = ChildEdge::new(leaf.clone(), 7);
        assert_eq!(e1.field_id, 7);
        assert!(Arc::ptr_eq(&e1.subtree, &leaf));

        let e2 = ChildEdge::new_without_field(leaf.clone());
        assert_eq!(e2.field_id, FIELD_NONE);
        assert_eq!(FIELD_NONE, u16::MAX);
        assert!(Arc::ptr_eq(&e2.subtree, &leaf));
    }

    #[test]
    fn child_edge_clone_shares_subtree_arc() {
        let leaf = mk_leaf(1, 0..1);
        let e1 = ChildEdge::new(leaf, 2);
        let e2 = e1.clone();
        assert!(Arc::ptr_eq(&e1.subtree, &e2.subtree));
        assert_eq!(e2.field_id, 2);
    }

    #[test]
    fn subtree_node_clone_and_debug() {
        let n = mk_node(42, 1..2, true);
        let cloned = n.clone();
        assert_eq!(cloned.symbol_id, SymbolId(42));
        assert_eq!(cloned.byte_range, 1..2);
        assert!(cloned.is_error);
        let s = format!("{n:?}");
        assert!(s.contains("SubtreeNode"));
    }

    #[test]
    fn subtree_clone_preserves_fields() {
        let leaf = mk_leaf_with_prec(3, 0..2, 9);
        let s = Subtree::with_dynamic_prec(mk_node(50, 0..2, false), vec![leaf], 4);
        let cloned = s.clone();
        assert_eq!(cloned.symbol(), s.symbol());
        assert_eq!(cloned.dynamic_prec, s.dynamic_prec);
        assert_eq!(cloned.children.len(), s.children.len());
        let dbg = format!("{s:?}");
        assert!(dbg.contains("Subtree"));
    }

    #[test]
    fn merge_ambiguous_adds_other_as_alternative() {
        let primary = Subtree::new(mk_node(1, 0..1, false), Vec::new());
        let alt = mk_leaf(2, 0..1);
        let merged = primary.merge_ambiguous(alt.clone());
        assert!(merged.is_ambiguous());
        assert!(merged.has_alts());
        assert_eq!(merged.alternatives.len(), 1);
        assert!(Arc::ptr_eq(&merged.alternatives[0], &alt));
    }

    #[test]
    fn merge_ambiguous_deduplicates_other_when_already_present() {
        let primary = Subtree::new(mk_node(1, 0..1, false), Vec::new());
        let alt = mk_leaf(2, 0..1);
        let merged = primary.merge_ambiguous(alt.clone());
        // Re-merge the same alt; should not grow alternatives.
        let merged = merged.merge_ambiguous(alt.clone());
        assert_eq!(merged.alternatives.len(), 1);
    }

    #[test]
    fn merge_ambiguous_pulls_in_others_alternatives() {
        let inner_alt = mk_leaf(3, 0..1);
        // Build `other` with an existing alternative.
        let other_with_alts =
            Subtree::new(mk_node(2, 0..1, false), Vec::new()).with_alts(inner_alt.clone());
        let other_arc = Arc::new(other_with_alts);

        let primary = Subtree::new(mk_node(1, 0..1, false), Vec::new());
        let merged = primary.merge_ambiguous(other_arc.clone());

        // Should include both `inner_alt` and `other_arc` itself.
        assert_eq!(merged.alternatives.len(), 2);
        assert!(
            merged
                .alternatives
                .iter()
                .any(|a| Arc::ptr_eq(a, &inner_alt))
        );
        assert!(
            merged
                .alternatives
                .iter()
                .any(|a| Arc::ptr_eq(a, &other_arc))
        );
    }

    #[test]
    fn merge_ambiguous_keeps_highest_dynamic_prec() {
        let primary = Subtree::with_dynamic_prec(mk_node(1, 0..1, false), Vec::new(), 3);
        let other = Arc::new(Subtree::with_dynamic_prec(
            mk_node(2, 0..1, false),
            Vec::new(),
            11,
        ));
        let merged = primary.merge_ambiguous(other);
        assert_eq!(merged.dynamic_prec, 11);
    }

    #[test]
    fn with_alts_dedups_pointer_equal_alternatives() {
        let alt = mk_leaf(2, 0..1);
        let s = Subtree::new(mk_node(1, 0..1, false), Vec::new())
            .with_alts(alt.clone())
            .with_alts(alt.clone());
        assert_eq!(s.alternatives.len(), 1);
        assert!(Arc::ptr_eq(&s.alternatives[0], &alt));
    }

    #[test]
    fn push_alt_dedups_and_updates_prec() {
        let alt = Arc::new(Subtree::with_dynamic_prec(
            mk_node(2, 0..1, false),
            Vec::new(),
            42,
        ));
        let s = Subtree::with_dynamic_prec(mk_node(1, 0..1, false), Vec::new(), 1)
            .push_alt(alt.clone());
        assert_eq!(s.alternatives.len(), 1);
        assert_eq!(s.dynamic_prec, 42);
        // Pushing the same alt again is a no-op.
        let s = s.push_alt(alt);
        assert_eq!(s.alternatives.len(), 1);
    }

    #[test]
    fn alternatives_iter_yields_all_alts_in_order() {
        let a1 = mk_leaf(2, 0..1);
        let a2 = mk_leaf(3, 0..1);
        let s = Subtree::new(mk_node(1, 0..1, false), Vec::new())
            .with_alts(a1.clone())
            .with_alts(a2.clone());
        let collected: Vec<_> = s.alternatives_iter().collect();
        assert_eq!(collected.len(), 2);
        assert!(Arc::ptr_eq(collected[0], &a1));
        assert!(Arc::ptr_eq(collected[1], &a2));
    }

    #[test]
    fn concat_alts_combines_and_dedupes_alternatives() {
        let shared = mk_leaf(9, 0..1);
        let other_inner_alt = mk_leaf(4, 0..1);

        // primary already has `shared` as an alternative.
        let primary = Subtree::new(mk_node(1, 0..1, false), Vec::new()).with_alts(shared.clone());
        // other has both `shared` and `other_inner_alt` as alternatives.
        let other = Arc::new(
            Subtree::new(mk_node(2, 0..1, false), Vec::new())
                .with_alts(shared.clone())
                .with_alts(other_inner_alt.clone()),
        );

        let merged = primary.concat_alts(other.clone());
        // Expect: shared (already there), other itself, other_inner_alt — no
        // duplicates.
        assert_eq!(merged.alternatives.len(), 3);
        let mut count_shared = 0;
        let mut count_other = 0;
        let mut count_inner = 0;
        for a in &merged.alternatives {
            if Arc::ptr_eq(a, &shared) {
                count_shared += 1;
            }
            if Arc::ptr_eq(a, &other) {
                count_other += 1;
            }
            if Arc::ptr_eq(a, &other_inner_alt) {
                count_inner += 1;
            }
        }
        assert_eq!(count_shared, 1);
        assert_eq!(count_other, 1);
        assert_eq!(count_inner, 1);
    }

    #[test]
    fn empty_span_and_root_like_subtree() {
        let s = Subtree::new(mk_node(0, 0..0, false), Vec::new());
        assert_eq!(s.byte_range(), 0..0);
        assert_eq!(s.symbol(), 0);
        assert!(!s.is_error());
        assert!(!s.is_ambiguous());
    }
}
