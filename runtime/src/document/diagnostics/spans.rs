use super::*;

pub(super) fn first_error_span(node: &ParseNode) -> Option<Range<usize>> {
    if node.symbol.0 == 0 && node.children.is_empty() {
        return Some(node.start_byte..node.end_byte);
    }

    node.children.iter().find_map(first_error_span)
}
