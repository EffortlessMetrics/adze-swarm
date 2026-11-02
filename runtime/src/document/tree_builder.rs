//! Syntax-tree record and edge construction helpers.
//!
//! Canonical flat construction uses explicit heap traversal frames so document
//! building does not consume call-stack depth proportional to tree depth.
//! Parents and edges derive directly from the selected parse tree; the
//! construction input is consumed once and no persistent recursive tree is
//! retained (storage retirement completed under #900b/#860).

use super::diagnostics::is_error_parse_node;
use super::source_position::SourceLineIndex;
use super::*;

/// Flat node/edge/parent records built directly from one selected parse tree.
///
/// Every child yields exactly one edge, so construction cannot silently drop
/// structural facts; [`SyntaxTree::validate`] remains the explicit checked
/// path for corrupted flat records.
pub(in crate::document) struct FlatTreeBuild {
    pub(in crate::document) nodes: Vec<NodeRecord>,
    pub(in crate::document) edges: Vec<EdgeRecord>,
    pub(in crate::document) parents: Vec<Option<NodeId>>,
}

pub(in crate::document) fn build_flat_tree(
    root: &ParseNode,
    source: &str,
    language: &LanguageMetadata,
    error_count: usize,
) -> FlatTreeBuild {
    let positions = SourceLineIndex::new(source);

    // Phase 1: preorder layout on an explicit heap stack. Preorder numbering
    // guarantees every parent id is smaller than its descendants' ids, so a
    // single reverse pass later resolves bottom-up error flags.
    let mut order: Vec<&ParseNode> = Vec::new();
    let mut parents: Vec<Option<NodeId>> = Vec::new();
    let mut child_ids: Vec<Vec<NodeId>> = Vec::new();
    {
        let mut stack: Vec<(&ParseNode, Option<NodeId>)> = vec![(root, None)];
        while let Some((node, parent)) = stack.pop() {
            let id = NodeId::new(order.len());
            if let Some(parent_id) = parent {
                child_ids[parent_id.as_usize()].push(id);
            }
            order.push(node);
            parents.push(parent);
            child_ids.push(Vec::with_capacity(node.children.len()));
            for child in node.children.iter().rev() {
                stack.push((child, Some(id)));
            }
            #[cfg(test)]
            preorder_visits().with(|visits| visits.set(visits.get().saturating_add(1)));
        }
    }

    // Phase 2: node records with point ranges from the single line index.
    let mut nodes: Vec<NodeRecord> = Vec::with_capacity(order.len());
    for node in &order {
        let grammar_id = node.symbol_id;
        let alias_symbol_id = node.alias_symbol_id;
        nodes.push(NodeRecord {
            visible_id: alias_symbol_id.unwrap_or(grammar_id),
            grammar_id,
            byte_range: node.start_byte..node.end_byte,
            point_range: positions.point_range(node.start_byte..node.end_byte),
            edge_range: EdgeRange::default(),
            alias_symbol_id,
            flags: NodeFlags::default(),
        });
    }

    // Phase 3: contiguous per-parent edges in node-id order.
    let mut edges: Vec<EdgeRecord> = Vec::new();
    for (node_id, node) in order.iter().enumerate() {
        let edge_start = edges.len();
        for (child_index, child) in node.children.iter().enumerate() {
            let child_id = child_ids[node_id][child_index];
            edges.push(EdgeRecord {
                parent_id: NodeId::new(node_id),
                child_id,
                child_index,
                field_id: child
                    .field_name
                    .as_deref()
                    .and_then(|field_name| language.field_id_for_name(field_name)),
            });
        }
        let edge_len = edges.len() - edge_start;
        nodes[node_id].edge_range = EdgeRange::new(edge_start, edge_len);
    }

    // Phase 4: bottom-up error flags in reverse preorder. Preorder numbering
    // places every descendant after its ancestors, so one reverse pass
    // resolves child error state before its parents.
    let mut has_error = vec![false; order.len()];
    for (node_id, node) in order.iter().enumerate().rev() {
        let error = is_error_parse_node(node);
        let missing = node.start_byte == node.end_byte && error;
        let child_has_error = child_ids[node_id]
            .iter()
            .any(|child_id| has_error[child_id.as_usize()]);
        // Only the build root (element 0) absorbs the ambient error count;
        // subtree builds pass `error_count == 0` via `collect_node_records`.
        let node_has_error = error || child_has_error || (node_id == 0 && error_count > 0);
        has_error[node_id] = node_has_error;
        let kind = language.symbol(nodes[node_id].visible_id);
        nodes[node_id].flags = NodeFlags {
            named: kind.map(NodeKind::is_named).unwrap_or(false),
            visible: kind.map(NodeKind::is_visible).unwrap_or(false),
            extra: kind.map(NodeKind::is_extra).unwrap_or(false),
            terminal: kind.map(NodeKind::is_terminal).unwrap_or(false),
            supertype: kind.map(NodeKind::is_supertype).unwrap_or(false),
            error,
            missing,
            has_error: node_has_error,
        };
    }

    FlatTreeBuild {
        nodes,
        edges,
        parents,
    }
}

/// Compatibility entry point appending subtree node records to `nodes`.
///
/// Returns the subtree root's bottom-up error state. Only document-root calls
/// (`is_root == true`) absorb the ambient `error_count`; edge/parent tables
/// are owned by the combined [`build_flat_tree`] path, not by this function.
///
/// Test-only until #900 migrates a consumer onto flat records.
#[cfg(test)]
pub(in crate::document) fn collect_node_records(
    node: &ParseNode,
    source: &str,
    language: &LanguageMetadata,
    error_count: usize,
    is_root: bool,
    nodes: &mut Vec<NodeRecord>,
) -> bool {
    // Subtree builds never absorb the ambient document error count; only the
    // document root does. Node records carry no ids of their own (identity is
    // positional), so appending a subtree under an existing prefix stays
    // consistent; edge/parent tables are owned by the combined flat builder.
    let effective_error_count = if is_root { error_count } else { 0 };
    let built = build_flat_tree(node, source, language, effective_error_count);
    // Every selected tree has a root record, so the first record always exists.
    let root_has_error = built.nodes[0].flags().has_error();
    nodes.extend(built.nodes);
    root_has_error
}

#[cfg(test)]
std::thread_local! {
    static PREORDER_VISITS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
fn preorder_visits() -> &'static std::thread::LocalKey<std::cell::Cell<usize>> {
    &PREORDER_VISITS
}

#[cfg(test)]
pub(in crate::document) fn preorder_visit_stats() -> usize {
    preorder_visits().with(|visits| visits.get())
}

pub(in crate::document) fn insert_symbol(symbols: &mut Vec<NodeKind>, symbol: NodeKind) {
    if let Some(existing) = symbols
        .iter_mut()
        .find(|existing| existing.symbol_id == symbol.symbol_id)
    {
        *existing = symbol;
    } else {
        symbols.push(symbol);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::source_position::index_build_stats;

    fn test_language() -> LanguageMetadata {
        test_language_with_fields(&[])
    }

    fn test_language_with_fields(fields: &[&str]) -> LanguageMetadata {
        LanguageMetadata {
            name: "index-test".to_string(),
            symbols: Vec::new(),
            fields: fields.iter().map(ToString::to_string).collect(),
            field_semantics: Default::default(),
        }
    }

    fn leaf(symbol: u16, start: usize, end: usize) -> ParseNode {
        ParseNode {
            symbol: SymbolId(symbol),
            symbol_id: SymbolId(symbol),
            start_byte: start,
            end_byte: end,
            field_name: None,
            alias_symbol_id: None,
            children: Vec::new(),
        }
    }

    #[test]
    fn test_node_records_many_nodes_index_source_once() {
        let source = "a\n".repeat(64);
        let children = (0..64)
            .map(|row| ParseNode {
                symbol: SymbolId(1),
                symbol_id: SymbolId(1),
                start_byte: row * 2,
                end_byte: row * 2 + 1,
                field_name: None,
                alias_symbol_id: None,
                children: Vec::new(),
            })
            .collect();
        let root = ParseNode {
            symbol: SymbolId(1),
            symbol_id: SymbolId(1),
            start_byte: 0,
            end_byte: source.len(),
            field_name: None,
            alias_symbol_id: None,
            children,
        };
        let language = test_language();
        let mut records = Vec::new();
        let before = index_build_stats();
        collect_node_records(&root, &source, &language, 0, true, &mut records);
        let after = index_build_stats();
        assert_eq!(after.0 - before.0, 1, "one index per record-building call");
        assert_eq!(
            after.1 - before.1,
            source.len(),
            "one visit per source byte"
        );
        assert_eq!(records.len(), 65);
        for (row, record) in records.iter().skip(1).enumerate() {
            assert_eq!(
                record.point_range(),
                PointRange {
                    start: DocumentPoint {
                        row: row as u32,
                        column: 0,
                    },
                    end: DocumentPoint {
                        row: row as u32,
                        column: 1,
                    },
                }
            );
        }
    }

    #[test]
    fn test_flat_builder_visits_each_node_once() {
        let source = "a\n".repeat(32);
        let children = (0..32).map(|row| leaf(1, row * 2, row * 2 + 1)).collect();
        let root = ParseNode {
            symbol: SymbolId(1),
            symbol_id: SymbolId(1),
            start_byte: 0,
            end_byte: source.len(),
            field_name: None,
            alias_symbol_id: None,
            children,
        };
        let language = test_language();
        let before = preorder_visit_stats();
        let built = build_flat_tree(&root, &source, &language, 0);
        let after = preorder_visit_stats();
        assert_eq!(built.nodes.len(), 33);
        assert_eq!(
            after - before,
            33,
            "explicit preorder pass must visit every node exactly once"
        );
    }

    #[test]
    fn test_flat_builder_wide_tree_preserves_facts_in_order() {
        let width = 8_192;
        let mut source = String::new();
        let mut field_names: Vec<String> = Vec::new();
        let mut children = Vec::with_capacity(width);
        for index in 0..width {
            source.push('a');
            source.push('\n');
            let mut child = leaf(1, index * 2, index * 2 + 1);
            if index % 3 == 0 {
                child.field_name = Some(format!("field{index}"));
                field_names.push(format!("field{index}"));
            }
            if index % 5 == 0 {
                child.alias_symbol_id = Some(SymbolId(7));
            }
            children.push(child);
        }
        let root = ParseNode {
            symbol: SymbolId(1),
            symbol_id: SymbolId(1),
            start_byte: 0,
            end_byte: source.len(),
            field_name: None,
            alias_symbol_id: None,
            children,
        };
        let field_refs: Vec<&str> = field_names.iter().map(String::as_str).collect();
        let language = test_language_with_fields(&field_refs);
        let built = build_flat_tree(&root, &source, &language, 0);

        assert_eq!(built.nodes.len(), width + 1);
        assert_eq!(built.edges.len(), width);
        assert_eq!(built.parents.len(), width + 1);
        assert_eq!(built.parents[0], None);
        // Root owns one contiguous edge range covering every child in order.
        assert_eq!(
            built.nodes[0].edge_range(),
            EdgeRange::new(0, width),
            "root edge range must be contiguous in child order"
        );
        for (offset, edge) in built.edges.iter().enumerate() {
            assert_eq!(edge.parent_id(), NodeId::new(0));
            assert_eq!(edge.child_id(), NodeId::new(offset + 1));
            assert_eq!(edge.child_index(), offset);
        }
        for (node_id, parent) in built.parents.iter().enumerate().skip(1) {
            assert_eq!(*parent, Some(NodeId::new(0)), "node {node_id}");
        }
        // Fields survive the flat pass and map to the language field table;
        // aliases survive untouched.
        let mut field_position = 0usize;
        for index in 0..width {
            let record = &built.nodes[index + 1];
            assert_eq!(record.byte_range(), index * 2..index * 2 + 1);
            if index % 3 == 0 {
                field_position += 1;
                assert_eq!(
                    built.edges[index].field_id(),
                    FieldId::new(field_position as u16),
                    "field at child {index}"
                );
            } else {
                assert_eq!(built.edges[index].field_id(), None, "child {index}");
            }
            assert_eq!(
                record.alias_symbol_id(),
                (index % 5 == 0).then_some(SymbolId(7)),
                "alias at child {index}"
            );
        }
        assert!(!built.nodes[0].flags().has_error());
    }

    #[test]
    fn test_flat_builder_deep_chain_without_recursion() {
        // A single-child chain built bottom-up with a loop, so even the test
        // setup/teardown never recurses through tree depth.
        const DEPTH: usize = 4_000;
        let mut current = leaf(1, 0, 1);
        for _ in 1..DEPTH {
            current = ParseNode {
                symbol: SymbolId(1),
                symbol_id: SymbolId(1),
                start_byte: 0,
                end_byte: 1,
                field_name: None,
                alias_symbol_id: None,
                children: vec![current],
            };
        }
        let source = "x".to_string();
        let language = test_language();
        let built = build_flat_tree(&current, &source, &language, 0);

        assert_eq!(built.nodes.len(), DEPTH);
        assert_eq!(built.edges.len(), DEPTH - 1);
        assert_eq!(built.parents.len(), DEPTH);
        for id in 0..DEPTH {
            let record = &built.nodes[id];
            if id + 1 < DEPTH {
                assert_eq!(record.edge_range(), EdgeRange::new(id, 1), "node {id}");
                assert_eq!(built.parents[id + 1], Some(NodeId::new(id)));
                assert_eq!(built.edges[id].child_id(), NodeId::new(id + 1));
            } else {
                assert!(record.edge_range().is_empty(), "deep leaf");
            }
        }
        assert!(!built.nodes[0].flags().has_error());
    }

    #[test]
    fn test_flat_builder_propagates_error_flags_bottom_up() {
        // Error leaf at the bottom of a chain: every ancestor must report
        // has_error while the clean sibling subtree stays clean.
        let mut error_leaf = leaf(1, 0, 0);
        error_leaf.symbol = SymbolId(0);
        error_leaf.symbol_id = SymbolId(0);
        let error_branch = ParseNode {
            symbol: SymbolId(1),
            symbol_id: SymbolId(1),
            start_byte: 0,
            end_byte: 0,
            field_name: None,
            alias_symbol_id: None,
            children: vec![error_leaf],
        };
        let clean_branch = leaf(1, 0, 1);
        let root = ParseNode {
            symbol: SymbolId(1),
            symbol_id: SymbolId(1),
            start_byte: 0,
            end_byte: 1,
            field_name: None,
            alias_symbol_id: None,
            children: vec![error_branch, clean_branch],
        };
        let language = test_language();
        let built = build_flat_tree(&root, "x", &language, 0);

        assert_eq!(built.nodes.len(), 4);
        assert!(built.nodes[2].flags().is_error(), "error leaf");
        assert!(built.nodes[2].flags().is_missing(), "zero-width error");
        assert!(built.nodes[1].flags().has_error(), "error ancestor");
        assert!(built.nodes[0].flags().has_error(), "root");
        assert!(!built.nodes[3].flags().has_error(), "clean sibling");
        assert!(built.nodes[0].flags().has_error(), "flat root flag");
    }
}
