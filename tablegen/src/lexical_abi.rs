//! Lexical fact propagation into Tree-sitter ABI artifacts (#924 PR3).
//!
//! Compiler-significant lexical metadata from `adze_ir::Grammar` is encoded into
//! generated ABI fields here rather than being inferred at matcher generation time.

use adze_ir::{Grammar, SymbolId};
use std::collections::BTreeMap;

use crate::abi::create_symbol_metadata;

/// Resolve the ABI `keyword_capture_token` index from explicit `word_token` metadata.
#[must_use]
pub(crate) fn keyword_capture_index(
    grammar: &Grammar,
    symbol_to_index: &BTreeMap<SymbolId, usize>,
) -> u16 {
    grammar
        .word_token_symbol()
        .and_then(|symbol| symbol_to_index.get(&symbol).copied())
        .map(|index| index as u16)
        .unwrap_or(0)
}

/// Build symbol-metadata flags including lexical immediate semantics.
#[must_use]
pub(crate) fn symbol_metadata_byte(
    grammar: &Grammar,
    symbol_id: SymbolId,
    visible: bool,
    named: bool,
    hidden: bool,
    supertype: bool,
) -> u8 {
    let immediate = grammar.lexical_metadata_for(symbol_id).immediate;
    create_symbol_metadata(visible, named, hidden, immediate, supertype)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::abi::symbol_metadata;
    use adze_ir::{Grammar, LexicalMetadata, SymbolId, Token, TokenPattern};

    #[test]
    fn keyword_capture_index_uses_word_token_mapping() {
        let mut grammar = Grammar::new("word".to_string());
        let ident = SymbolId(3);
        grammar.tokens.insert(
            ident,
            Token {
                name: "identifier".to_string(),
                pattern: TokenPattern::Regex(r"[a-z]+".to_string()),
                fragile: false,
            },
        );
        grammar.word_token = Some(ident);

        let mut symbol_to_index = BTreeMap::new();
        symbol_to_index.insert(SymbolId(0), 0);
        symbol_to_index.insert(ident, 2);

        assert_eq!(keyword_capture_index(&grammar, &symbol_to_index), 2);
    }

    #[test]
    fn symbol_metadata_byte_sets_auxiliary_for_immediate_tokens() {
        let mut grammar = Grammar::new("immediate".to_string());
        let dot = SymbolId(1);
        grammar.tokens.insert(
            dot,
            Token {
                name: ".".to_string(),
                pattern: TokenPattern::String(".".to_string()),
                fragile: false,
            },
        );
        grammar.set_lexical_metadata(
            dot,
            LexicalMetadata {
                immediate: true,
                lexical_priority: 0,
            },
        );

        let meta = symbol_metadata_byte(&grammar, dot, true, false, false, false);
        assert_ne!(meta & symbol_metadata::AUXILIARY, 0);
    }
}
