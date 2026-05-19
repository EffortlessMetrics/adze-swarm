use super::{ParsedChild, ParsedNode, Point, TSLanguage};
use crate::ffi::TSSymbol;
use std::sync::Arc;

/// Builder functions for creating nodes during parsing
impl ParsedNode {
    /// Create a node from a reduce action
    pub fn from_reduction(
        symbol: TSSymbol,
        production_id: u16,
        children: Vec<ParsedNode>,
        language: Arc<TSLanguage>,
    ) -> Self {
        // Get field mappings for this production
        let field_map = language.production_fields(production_id);

        // Build children with field IDs
        let parsed_children: Vec<ParsedChild> = children
            .into_iter()
            .enumerate()
            .map(|(i, child)| ParsedChild {
                node: child,
                field_id: field_map.get(i).and_then(|&f| f),
            })
            .collect();

        // Compute byte ranges from children
        let (start_byte, end_byte) = if parsed_children.is_empty() {
            (0, 0)
        } else {
            let start = parsed_children[0].node.start_byte;
            let end = parsed_children
                .last()
                .map(|child| child.node.end_byte)
                .unwrap_or(start);
            (start, end)
        };

        // Compute point ranges from children
        let (start_point, end_point) = if parsed_children.is_empty() {
            (Point::new(0, 0), Point::new(0, 0))
        } else {
            let start = parsed_children[0].node.start_point;
            let end = parsed_children
                .last()
                .map(|child| child.node.end_point)
                .unwrap_or(start);
            (start, end)
        };

        ParsedNode {
            symbol,
            children: parsed_children,
            start_byte,
            end_byte,
            start_point,
            end_point,
            is_extra: false,
            is_error: false,
            is_missing: false,
            is_named: true, // TODO: Get from symbol metadata
            language: Some(language),
        }
    }
}
