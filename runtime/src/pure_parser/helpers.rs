//! Helper functions for the pure-Rust parser.
//!
//! These are free functions that don't depend on `Parser`'s private state.

use super::{ParsedNode, Point, Subtree, TSLanguage};

/// Advance a source position point by the bytes in `text`.
pub(super) fn advance_point(mut point: Point, text: &[u8]) -> Point {
    for &byte in text {
        if byte == b'\n' {
            point.row += 1;
            point.column = 0;
        } else {
            point.column += 1;
        }
    }
    point
}

/// Convert an internal [`Subtree`] to a public [`ParsedNode`].
pub(super) fn subtree_to_node(subtree: Subtree, language: Option<*const TSLanguage>) -> ParsedNode {
    // Determine if the node is named based on symbol metadata
    let is_named = if let Some(lang_ptr) = language {
        // SAFETY: `lang_ptr` comes from a valid `TSLanguage` that outlives the
        // parse result. `symbol_metadata` is bounds-checked via `symbol_count`.
        unsafe {
            let lang = &*lang_ptr;
            if subtree.symbol < lang.symbol_count as u16 {
                let metadata = *lang.symbol_metadata.add(subtree.symbol as usize);
                metadata >= 2
            } else {
                false
            }
        }
    } else {
        true // Default to named if no language info
    };

    ParsedNode {
        symbol: subtree.symbol,
        children: subtree
            .children
            .into_iter()
            .map(|s| subtree_to_node(s, language))
            .collect(),
        start_byte: subtree.start_byte,
        end_byte: subtree.end_byte,
        start_point: subtree.start_point,
        end_point: subtree.end_point,
        is_extra: subtree.is_extra,
        is_error: subtree.is_error,
        is_missing: subtree.is_missing,
        is_named,
        field_id: subtree.field_id,
        language,
    }
}
