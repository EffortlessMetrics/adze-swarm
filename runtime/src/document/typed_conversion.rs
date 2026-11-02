//! Conversion helpers from document nodes back into generated pure-parser nodes.

use super::source_position::SourceLineIndex;
use super::*;
use std::ffi::CStr;

/// Materialize one flat-record subtree as generated pure-parser nodes.
///
/// This is the flat-record counterpart of the retired recursive
/// `document_node_to_parsed_node`: same symbol mapping, flags, field
/// resolution, and byte/point contracts, but resolved from canonical
/// [`SyntaxTree`] records with one shared line index instead of recursive
/// walks and per-node prefix rescans. Only the target subtree is
/// materialized; ancestors and siblings are never converted. Returns `None`
/// when `root_id` has no record.
pub(super) fn flat_subtree_to_parsed_node(
    tree: &SyntaxTree,
    language: &LanguageMetadata,
    parsed_language: &crate::pure_parser::TSLanguage,
    source: &str,
    root_id: NodeId,
) -> Option<crate::pure_parser::ParsedNode> {
    let positions = SourceLineIndex::new(source);

    // Preorder ids of the target subtree on an explicit heap stack.
    // Preorder numbering makes every subtree contiguous, so a built node's
    // slot is its id minus the subtree root id.
    let mut order: Vec<NodeId> = Vec::new();
    {
        let mut stack = vec![root_id];
        while let Some(node_id) = stack.pop() {
            let record = tree.node_record(node_id)?;
            order.push(node_id);
            for child_index in (0..record.edge_range().len()).rev() {
                stack.push(tree.child_id(node_id, child_index)?);
            }
        }
    }

    let base = root_id.as_usize();
    let mut slots: Vec<Option<crate::pure_parser::ParsedNode>> =
        (0..order.len()).map(|_| None).collect();
    for node_id in order.iter().rev() {
        let record = tree
            .node_record(*node_id)
            .expect("visited ids have records");
        let range = record.byte_range();
        let mut children = Vec::with_capacity(record.edge_range().len());
        for child_index in 0..record.edge_range().len() {
            let child_id = tree
                .child_id(*node_id, child_index)
                .expect("visited edges resolve");
            children.push(
                slots[child_id.as_usize() - base]
                    .take()
                    .expect("reverse preorder builds children first"),
            );
        }
        let symbol = table_symbol_for_public_id(parsed_language, record.grammar_id());
        let (is_named, is_extra) = symbol_flags(parsed_language, symbol);
        let is_empty_error_node =
            record.grammar_id().0 == 0 && children.is_empty() && range.start == range.end;
        let start_point = positions.point(range.start);
        let end_point = positions.point(range.end);
        slots[node_id.as_usize() - base] = Some(crate::pure_parser::ParsedNode {
            symbol,
            children,
            start_byte: range.start,
            end_byte: range.end,
            start_point: crate::pure_parser::Point {
                row: start_point.row,
                column: start_point.column,
            },
            end_point: crate::pure_parser::Point {
                row: end_point.row,
                column: end_point.column,
            },
            is_extra,
            is_error: symbol_name(parsed_language, symbol) == Some("ERROR")
                || is_empty_error_node
                || record.grammar_id().0 == 0,
            is_missing: is_empty_error_node,
            is_named,
            field_id: tree
                .parent_edge_record(*node_id)
                .and_then(|edge| edge.field_id())
                .and_then(|document_field| {
                    language
                        .field_name_for_id(document_field.get())
                        .and_then(|name| field_id_for_name(parsed_language, name))
                }),
            language: Some(parsed_language as *const _),
        });
    }
    slots.into_iter().next().flatten()
}

fn table_symbol_for_public_id(
    language: &crate::pure_parser::TSLanguage,
    public_symbol: SymbolId,
) -> crate::pure_parser::TSSymbol {
    if !language.public_symbol_map.is_null() {
        // SAFETY: `public_symbol_map` has one entry per generated table symbol.
        let public_symbols = unsafe {
            std::slice::from_raw_parts(language.public_symbol_map, language.symbol_count as usize)
        };
        if let Some(index) = public_symbols
            .iter()
            .position(|candidate| *candidate == public_symbol.0)
        {
            return index as crate::pure_parser::TSSymbol;
        }
    }

    public_symbol.0
}

fn symbol_flags(
    language: &crate::pure_parser::TSLanguage,
    symbol: crate::pure_parser::TSSymbol,
) -> (bool, bool) {
    if language.symbol_metadata.is_null() || u32::from(symbol) >= language.symbol_count {
        return (true, false);
    }

    // SAFETY: `symbol` is bounds-checked above, and `symbol_metadata` has one
    // entry per generated table symbol.
    let metadata = unsafe { *language.symbol_metadata.add(usize::from(symbol)) };
    let is_named = (metadata & 0x02) != 0;
    let is_extra = (metadata & 0x04) != 0;
    (is_named, is_extra)
}

fn field_id_for_name(language: &crate::pure_parser::TSLanguage, field_name: &str) -> Option<u16> {
    if language.field_count == 0 || language.field_names.is_null() {
        return None;
    }

    // SAFETY: `field_names` points to a static array of `field_count` C string
    // pointers generated with the language table.
    let field_names =
        unsafe { std::slice::from_raw_parts(language.field_names, language.field_count as usize) };
    field_names
        .iter()
        .enumerate()
        .find_map(|(index, name_ptr)| {
            c_str_to_str(*name_ptr)
                .filter(|candidate| *candidate == field_name)
                .map(|_| index as u16)
        })
}

fn symbol_name(
    language: &crate::pure_parser::TSLanguage,
    symbol: crate::pure_parser::TSSymbol,
) -> Option<&'static str> {
    if language.symbol_names.is_null() || u32::from(symbol) >= language.symbol_count {
        return None;
    }

    // SAFETY: `symbol` is bounds-checked above, and `symbol_names` has one C
    // string pointer per generated table symbol.
    let symbol_names = unsafe {
        std::slice::from_raw_parts(language.symbol_names, language.symbol_count as usize)
    };
    c_str_to_str(symbol_names[usize::from(symbol)])
}

fn c_str_to_str(ptr: *const u8) -> Option<&'static str> {
    if ptr.is_null() {
        return None;
    }

    // SAFETY: generated language tables store static NUL-terminated strings.
    unsafe { CStr::from_ptr(ptr.cast()).to_str().ok() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::tree_builder::build_flat_tree;

    fn conversion_language() -> LanguageMetadata {
        LanguageMetadata {
            name: "conversion-test".to_string(),
            symbols: Vec::new(),
            fields: vec!["left".to_string(), "right".to_string()],
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

    // A `TSLanguage` is only read through guarded raw pointers here; the
    // converter never dereferences a null table. A zeroed table exercises
    // the no-metadata fallback paths exactly.
    fn null_language() -> crate::pure_parser::TSLanguage {
        unsafe { std::mem::zeroed() }
    }

    #[test]
    fn test_flat_subtree_materializes_only_the_target() {
        // root(0..8) > [left-field leaf(0..3), mid(3..8) > leaf(3..8)];
        // converting `mid` must not include the left sibling.
        let mut left = leaf(1, 0, 3);
        left.field_name = Some("left".to_string());
        let root = ParseNode {
            symbol: SymbolId(1),
            symbol_id: SymbolId(1),
            start_byte: 0,
            end_byte: 8,
            field_name: None,
            alias_symbol_id: None,
            children: vec![
                left,
                ParseNode {
                    symbol: SymbolId(1),
                    symbol_id: SymbolId(1),
                    start_byte: 3,
                    end_byte: 8,
                    field_name: None,
                    alias_symbol_id: None,
                    children: vec![leaf(1, 3, 8)],
                },
            ],
        };
        let source = "12345678";
        let language = conversion_language();
        let built = build_flat_tree(&root, source, &language, 0);
        let tree = SyntaxTree {
            root_id: NodeId::new(0),
            nodes: built.nodes,
            edges: built.edges,
            parents: built.parents,
        };

        let converted = {
            let tables = null_language();
            flat_subtree_to_parsed_node(&tree, &language, &tables, source, NodeId::new(2))
                .expect("target record exists")
        };
        assert_eq!(converted.start_byte, 3);
        assert_eq!(converted.end_byte, 8);
        assert_eq!(converted.children.len(), 1);
        assert_eq!(converted.children[0].start_byte, 3);
        assert_eq!(converted.children[0].end_byte, 8);
        // Points come from the shared line index, not per-node rescans.
        assert_eq!(converted.start_point.row, 0);
        assert_eq!(converted.start_point.column, 3);
        // No metadata tables: symbol falls back to the public id.
        assert_eq!(converted.symbol, 1);
    }

    #[test]
    fn test_flat_subtree_missing_id_returns_none() {
        let root = leaf(1, 0, 1);
        let language = conversion_language();
        let built = build_flat_tree(&root, "x", &language, 0);
        let tree = SyntaxTree {
            root_id: NodeId::new(0),
            nodes: built.nodes,
            edges: built.edges,
            parents: built.parents,
        };
        let tables = null_language();
        assert!(
            flat_subtree_to_parsed_node(&tree, &language, &tables, "x", NodeId::new(7)).is_none()
        );
    }
}
