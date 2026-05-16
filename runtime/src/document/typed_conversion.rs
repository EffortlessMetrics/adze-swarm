//! Conversion helpers from document nodes back into generated pure-parser nodes.

use super::*;
use std::ffi::CStr;

pub(super) fn document_node_to_parsed_node(
    node: &ParseNode,
    language: &'static crate::pure_parser::TSLanguage,
    source: &[u8],
) -> crate::pure_parser::ParsedNode {
    let symbol = table_symbol_for_public_id(language, node.symbol_id);
    let children = node
        .children
        .iter()
        .map(|child| document_node_to_parsed_node(child, language, source))
        .collect();
    let (is_named, is_extra) = symbol_flags(language, symbol);
    let is_empty_error_node =
        node.symbol_id.0 == 0 && node.children.is_empty() && node.start_byte == node.end_byte;

    crate::pure_parser::ParsedNode {
        symbol,
        children,
        start_byte: node.start_byte,
        end_byte: node.end_byte,
        start_point: byte_to_point(source, node.start_byte),
        end_point: byte_to_point(source, node.end_byte),
        is_extra,
        is_error: symbol_name(language, symbol) == Some("ERROR") || is_empty_error_node,
        is_missing: is_empty_error_node,
        is_named,
        field_id: node
            .field_name
            .as_deref()
            .and_then(|field_name| field_id_for_name(language, field_name)),
        language: Some(language as *const _),
    }
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

fn byte_to_point(source: &[u8], byte: usize) -> crate::pure_parser::Point {
    let point = DocumentPoint::from_byte_offset(std::str::from_utf8(source).unwrap_or(""), byte);
    crate::pure_parser::Point {
        row: point.row,
        column: point.column,
    }
}
