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

/// Append the lexer's end-of-input sentinel byte to `source`.
///
/// The pure-Rust lexer treats `position >= lexer.input.len() - 1` as EOF, so
/// every input buffer must carry exactly one synthetic trailing byte beyond
/// the caller's real content. `source` is arbitrary bytes (not just UTF-8
/// text), so a real trailing `0x00` byte supplied by the caller must not be
/// mistaken for the sentinel — the sentinel is always appended.
pub(super) fn append_sentinel(source: &[u8]) -> Vec<u8> {
    let mut buf = Vec::with_capacity(source.len() + 1);
    buf.extend_from_slice(source);
    buf.push(0);
    buf
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

#[cfg(test)]
mod tests {
    use super::append_sentinel;

    #[test]
    fn test_append_sentinel_any_input_grows_length_by_one_byte() {
        assert_eq!(append_sentinel(b"").len(), 1);
        assert_eq!(append_sentinel(b"a").len(), 2);
        assert_eq!(append_sentinel(b"hello").len(), 6);
    }

    #[test]
    fn test_append_sentinel_with_trailing_nul_preserves_input_byte() {
        // `source` is arbitrary bytes, so a genuine trailing 0x00 supplied by
        // the caller must be kept intact, with the sentinel appended after it
        // rather than treated as the sentinel itself.
        let with_trailing_nul = append_sentinel(b"a\0");
        assert_eq!(with_trailing_nul, vec![b'a', 0, 0]);
        assert_eq!(with_trailing_nul.len(), 3);
    }
}
