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
    struct Frame {
        subtree: Subtree,
        remaining: std::vec::IntoIter<Subtree>,
        children: Vec<ParsedNode>,
    }
    let frame = |mut subtree: Subtree| {
        let children = std::mem::take(&mut subtree.children);
        Frame {
            subtree,
            children: Vec::with_capacity(children.len()),
            remaining: children.into_iter(),
        }
    };
    let mut frames = vec![frame(subtree)];
    loop {
        let next = frames
            .last_mut()
            .expect("root frame exists")
            .remaining
            .next();
        if let Some(child) = next {
            frames.push(frame(child));
            continue;
        }
        let completed = frames.pop().expect("completed frame exists");
        let node = node_from_subtree(completed.subtree, completed.children, language);
        if let Some(parent) = frames.last_mut() {
            parent.children.push(node);
        } else {
            return node;
        }
    }
}

fn node_from_subtree(
    subtree: Subtree,
    children: Vec<ParsedNode>,
    language: Option<*const TSLanguage>,
) -> ParsedNode {
    // Determine if the node is named based on symbol metadata
    let is_named = if let Some(lang_ptr) = language {
        // SAFETY: `lang_ptr` comes from a valid `TSLanguage` that outlives the
        // parse result. `symbol_metadata` is bounds-checked via `symbol_count`.
        unsafe {
            let lang = &*lang_ptr;
            if (subtree.symbol as usize) < lang.symbol_count as usize {
                let metadata = *lang.symbol_metadata.add(subtree.symbol as usize);
                (metadata & 0x02) != 0
            } else {
                false
            }
        }
    } else {
        true // Default to named if no language info
    };

    ParsedNode {
        symbol: subtree.symbol,
        children,
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

    // Conversion-only ABI fixture: no parser/scanner operation is invoked.
    fn metadata_language(metadata: &[u8]) -> super::TSLanguage {
        super::TSLanguage {
            version: 15,
            symbol_count: metadata.len() as u32,
            alias_count: 0,
            token_count: 0,
            external_token_count: 0,
            state_count: 0,
            large_state_count: 0,
            production_id_count: 0,
            field_count: 0,
            max_alias_sequence_length: 0,
            production_id_map: std::ptr::null(),
            parse_table: std::ptr::null(),
            small_parse_table: std::ptr::null(),
            small_parse_table_map: std::ptr::null(),
            parse_actions: std::ptr::null(),
            symbol_names: std::ptr::null(),
            field_names: std::ptr::null(),
            field_map_slices: std::ptr::null(),
            field_map_entries: std::ptr::null(),
            symbol_metadata: metadata.as_ptr(),
            public_symbol_map: std::ptr::null(),
            alias_map: std::ptr::null(),
            alias_sequences: std::ptr::null(),
            lex_modes: std::ptr::null(),
            lex_fn: None,
            keyword_lex_fn: None,
            keyword_capture_token: 0,
            external_scanner: super::super::ExternalScanner::default(),
            primary_state_ids: std::ptr::null(),
            production_lhs_index: std::ptr::null(),
            production_count: 0,
            eof_symbol: 0,
            rules: std::ptr::null(),
            rule_count: 0,
        }
    }

    #[test]
    fn test_named_metadata_at_maximum_u16_symbol_cardinality() {
        // ABI symbol IDs are u16, while symbol_count can represent all 65536
        // slots. This tests conversion bounds, not a whole maximum-size grammar.
        let mut flags = vec![0; u16::MAX as usize + 1];
        flags[0] = 0x02;
        flags[u16::MAX as usize] = 0x03;
        let language = metadata_language(&flags);
        for symbol in [0, u16::MAX] {
            let subtree = super::Subtree {
                symbol,
                children: Vec::new(),
                start_byte: 0,
                end_byte: 1,
                start_point: super::Point { row: 0, column: 0 },
                end_point: super::Point { row: 0, column: 1 },
                is_extra: false,
                is_error: false,
                is_missing: false,
                production_id: 0,
                field_id: None,
            };
            let node = super::subtree_to_node(subtree, Some(&language));
            assert!(
                node.is_named(),
                "valid ABI symbol {symbol} retains named metadata"
            );
        }
    }

    #[test]
    fn test_extra_and_auxiliary_metadata_do_not_imply_named() {
        // The parser ABI assigns independent bits to visibility, namedness,
        // extras/hidden, supertype and auxiliary flags. Only NAMED matters.
        for (metadata, expected_named) in [
            (0x00, false),
            (0x01, false),
            (0x02, true),
            (0x03, true),
            (0x04, false),
            (0x05, false),
            (0x06, true),
            (0x07, true),
            (0x08, false),
            (0x10, false),
            (0x1d, false),
            (0x1f, true),
        ] {
            let flags = [metadata];
            let language = metadata_language(&flags);
            let subtree = super::Subtree {
                symbol: 0,
                children: Vec::new(),
                start_byte: 0,
                end_byte: 1,
                start_point: super::Point { row: 0, column: 0 },
                end_point: super::Point { row: 0, column: 1 },
                is_extra: metadata & 0x04 != 0,
                is_error: false,
                is_missing: false,
                production_id: 0,
                field_id: None,
            };
            let parsed = super::subtree_to_node(subtree, Some(&language));
            assert_eq!(
                parsed.is_named(),
                expected_named,
                "metadata {metadata:#04x}"
            );
            assert_eq!(parsed.is_extra(), metadata & 0x04 != 0);
        }
    }

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
