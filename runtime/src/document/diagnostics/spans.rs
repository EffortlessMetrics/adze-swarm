use super::*;

pub(super) fn first_error_span(tree: &SyntaxTree) -> Option<Range<usize>> {
    (0..tree.node_count())
        .map(NodeId::new)
        .filter_map(|node_id| tree.node_record(node_id))
        .find(|record| record.flags().is_error())
        .map(|record| record.byte_range())
}
