//! Syntax-tree record and edge construction helpers.

use super::diagnostics::is_error_parse_node;
use super::*;

pub(super) fn build_node_index(root: &ParseNode) -> Vec<NodeIndex> {
    let mut index = Vec::new();
    let mut path = Vec::new();
    collect_node_index(root, &mut path, &mut index);
    index
}

pub(in crate::document) fn collect_node_records(
    node: &ParseNode,
    source: &str,
    language: &LanguageMetadata,
    error_count: usize,
    is_root: bool,
    nodes: &mut Vec<NodeRecord>,
) -> bool {
    let id = NodeId::new(nodes.len());
    let grammar_id = node.symbol_id;
    let alias_symbol_id = node.alias_symbol_id;
    let visible_id = alias_symbol_id.unwrap_or(grammar_id);

    nodes.push(NodeRecord {
        visible_id,
        grammar_id,
        byte_range: node.start_byte..node.end_byte,
        point_range: PointRange::from_byte_range(source, node.start_byte..node.end_byte),
        edge_range: EdgeRange::default(),
        alias_symbol_id,
        flags: NodeFlags::default(),
    });

    let mut child_has_error = false;
    for child in &node.children {
        child_has_error |= collect_node_records(child, source, language, error_count, false, nodes);
    }

    let error = is_error_parse_node(node);
    let missing = node.start_byte == node.end_byte && error;
    let has_error = error || child_has_error || (is_root && error_count > 0);
    let kind = language.symbol(visible_id);
    nodes[id.as_usize()].flags = NodeFlags {
        named: kind.map(NodeKind::is_named).unwrap_or(false),
        visible: kind.map(NodeKind::is_visible).unwrap_or(false),
        extra: kind.map(NodeKind::is_extra).unwrap_or(false),
        terminal: kind.map(NodeKind::is_terminal).unwrap_or(false),
        supertype: kind.map(NodeKind::is_supertype).unwrap_or(false),
        error,
        missing,
        has_error,
    };

    has_error
}

pub(in crate::document) fn collect_edge_records(
    node: &ParseNode,
    node_id: NodeId,
    node_index: &[NodeIndex],
    language: &LanguageMetadata,
    tree: &mut SyntaxTree,
) {
    let edge_start = tree.edges.len();
    let child_ids = node_index
        .get(node_id.as_usize())
        .map(|index| index.child_ids.as_slice())
        .unwrap_or(&[]);

    for (child_index, child) in node.children.iter().enumerate() {
        let Some(child_id) = child_ids.get(child_index).copied() else {
            continue;
        };
        tree.edges.push(EdgeRecord {
            parent_id: node_id,
            child_id,
            child_index,
            field_id: child
                .field_name
                .as_deref()
                .and_then(|field_name| language.field_id_for_name(field_name)),
        });
    }

    let edge_len = tree.edges.len().saturating_sub(edge_start);
    if let Some(record) = tree.nodes.get_mut(node_id.as_usize()) {
        record.edge_range = EdgeRange::new(edge_start, edge_len);
    }

    for (child_index, child) in node.children.iter().enumerate() {
        let Some(child_id) = child_ids.get(child_index).copied() else {
            continue;
        };
        collect_edge_records(child, child_id, node_index, language, tree);
    }
}

fn collect_node_index(
    node: &ParseNode,
    path: &mut Vec<usize>,
    index: &mut Vec<NodeIndex>,
) -> NodeId {
    collect_node_index_with_parent(node, path, index, None)
}

fn collect_node_index_with_parent(
    node: &ParseNode,
    path: &mut Vec<usize>,
    index: &mut Vec<NodeIndex>,
    parent_id: Option<NodeId>,
) -> NodeId {
    let id = NodeId::new(index.len());
    index.push(NodeIndex {
        path: path.clone(),
        parent_id,
        child_ids: Vec::with_capacity(node.children.len()),
    });

    let mut child_ids = Vec::with_capacity(node.children.len());
    for (child_index, child) in node.children.iter().enumerate() {
        path.push(child_index);
        child_ids.push(collect_node_index_with_parent(child, path, index, Some(id)));
        path.pop();
    }
    index[id.as_usize()].child_ids = child_ids;

    id
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
