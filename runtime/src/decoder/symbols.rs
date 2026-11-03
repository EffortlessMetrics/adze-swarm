use adze_ir::{SymbolId, Token, TokenPattern};
use indexmap::IndexMap;
use std::collections::HashMap;
use std::ffi::{CStr, c_char};

use crate::pure_parser::TSLanguage;

pub(super) fn decode_symbol_names(lang: &TSLanguage) -> Vec<String> {
    let mut symbol_names = Vec::new();

    if lang.symbol_names.is_null() {
        for i in 0..lang.symbol_count as usize {
            symbol_names.push(format!("symbol_{}", i));
        }
        return symbol_names;
    }

    let symbol_count = lang.symbol_count as usize;
    if symbol_count == 0 {
        return symbol_names;
    }

    // SAFETY: `lang.symbol_names` is non-null (branch guard above), and
    // `symbol_count` matches `lang.symbol_count`. The TSLanguage contract
    // guarantees the array has at least `symbol_count` elements.
    let symbol_name_ptrs = unsafe { std::slice::from_raw_parts(lang.symbol_names, symbol_count) };

    for (i, &name_ptr) in symbol_name_ptrs.iter().enumerate() {
        let name = if name_ptr.is_null() {
            format!("symbol_{}", i)
        } else {
            // SAFETY: `name_ptr` is non-null (branch guard above) and points to a
            // null-terminated C string per TSLanguage contract.
            match unsafe { CStr::from_ptr(name_ptr as *const c_char) }.to_str() {
                Ok(valid_str) => valid_str.to_owned(),
                Err(_) => format!("symbol_invalid_{}", i),
            }
        };
        symbol_names.push(name);
    }

    symbol_names
}

pub(super) fn decode_tokens(
    lang: &TSLanguage,
    symbol_names: &[String],
    token_patterns: &HashMap<String, TokenPattern>,
) -> IndexMap<SymbolId, Token> {
    if !lang.symbol_metadata.is_null() {
        return decode_tokens_from_metadata(lang, symbol_names, token_patterns);
    }

    decode_tokens_from_names(lang, symbol_names, token_patterns)
}

fn decode_tokens_from_metadata(
    lang: &TSLanguage,
    symbol_names: &[String],
    token_patterns: &HashMap<String, TokenPattern>,
) -> IndexMap<SymbolId, Token> {
    let mut tokens = IndexMap::new();
    let symbol_count = lang.symbol_count as usize;
    if symbol_count == 0 {
        return tokens;
    }

    // SAFETY: `lang.symbol_metadata` is non-null (caller branch) and the
    // TSLanguage contract guarantees the array has `symbol_count` elements.
    let symbol_metadata_slice =
        unsafe { std::slice::from_raw_parts(lang.symbol_metadata, symbol_count) };

    for (i, &metadata) in symbol_metadata_slice.iter().enumerate() {
        if let Some(name) = symbol_names.get(i)
            && is_terminal(metadata, name)
        {
            insert_token(&mut tokens, SymbolId(i as u16), name, token_patterns);
        }
    }

    tokens
}

fn decode_tokens_from_names(
    lang: &TSLanguage,
    symbol_names: &[String],
    token_patterns: &HashMap<String, TokenPattern>,
) -> IndexMap<SymbolId, Token> {
    let mut tokens = IndexMap::new();

    for i in 0..lang.symbol_count as usize {
        let Some(name) = symbol_names.get(i) else {
            continue;
        };

        if is_likely_terminal_by_name(name) {
            insert_token(&mut tokens, SymbolId(i as u16), name, token_patterns);
        }
    }

    tokens
}

fn insert_token(
    tokens: &mut IndexMap<SymbolId, Token>,
    symbol_id: SymbolId,
    name: &str,
    token_patterns: &HashMap<String, TokenPattern>,
) {
    let pattern = token_patterns
        .get(name)
        .cloned()
        .unwrap_or_else(|| TokenPattern::String(name.to_owned()));

    tokens.insert(
        symbol_id,
        Token {
            name: name.to_owned(),
            pattern,
            fragile: false,
        },
    );
}

/// Determine if a symbol is a terminal based on metadata and name.
fn is_terminal(metadata: u8, name: &str) -> bool {
    if (metadata & 0x04) != 0 {
        return true;
    }

    if (metadata & 0x01) != 0 {
        if name.starts_with('_') && name[1..].chars().all(|c| c.is_ascii_digit()) {
            return false;
        }
        return true;
    }

    is_special_terminal_name(name)
}

/// Heuristic to determine if a symbol is likely a terminal when metadata is unavailable.
fn is_likely_terminal_by_name(name: &str) -> bool {
    if is_special_terminal_name(name) {
        return true;
    }

    if name.starts_with('_') && name[1..].chars().all(|c| c.is_ascii_digit()) {
        return false;
    }

    if name.len() == 1 {
        return true;
    }

    name.len() <= 3
        && name
            .chars()
            .all(|c| !c.is_alphanumeric() && !c.is_whitespace())
}

fn is_special_terminal_name(name: &str) -> bool {
    name.starts_with("anon_sym_")
        || name.starts_with("aux_sym_")
        || name.starts_with("sym_")
        || name == "ERROR"
        || name.starts_with("ts_builtin_sym_")
        || matches!(
            name,
            "identifier"
                | "integer"
                | "float"
                | "string"
                | "comment"
                | "newline"
                | "indent"
                | "dedent"
                | "string_start"
                | "string_content"
                | "string_end"
        )
}

/// Translate decoded column identities to the public identities carried by nodes.
/// External declarations are already public when decoded by `externals`.
pub(super) fn remap_grammar_symbols(grammar: &mut adze_ir::Grammar, public_symbols: &[SymbolId]) {
    let remap = |id: SymbolId| public_symbols.get(id.0 as usize).copied().unwrap_or(id);
    grammar.rules = std::mem::take(&mut grammar.rules)
        .into_iter()
        .map(|(owner, mut rules)| {
            for rule in &mut rules {
                rule.lhs = remap(rule.lhs);
                for symbol in &mut rule.rhs {
                    remap_symbol(symbol, public_symbols);
                }
            }
            (remap(owner), rules)
        })
        .collect();
    grammar.tokens = std::mem::take(&mut grammar.tokens)
        .into_iter()
        .map(|(id, token)| (remap(id), token))
        .collect();
}

fn remap_symbol(symbol: &mut adze_ir::Symbol, public_symbols: &[SymbolId]) {
    use adze_ir::Symbol;
    match symbol {
        Symbol::Terminal(id) | Symbol::NonTerminal(id) | Symbol::External(id) => {
            *id = public_symbols.get(id.0 as usize).copied().unwrap_or(*id);
        }
        Symbol::Optional(inner) | Symbol::Repeat(inner) | Symbol::RepeatOne(inner) => {
            remap_symbol(inner, public_symbols);
        }
        Symbol::Choice(members) | Symbol::Sequence(members) => {
            for member in members {
                remap_symbol(member, public_symbols);
            }
        }
        Symbol::Epsilon => {}
    }
}

/// Derive a dense execution grammar from the canonical public grammar.
/// The caller prepares a clone; public document facts keep their original IDs.
#[cfg(feature = "glr")]
pub(crate) fn remap_grammar_to_table_columns(
    grammar: &mut adze_ir::Grammar,
    public_to_column: &std::collections::BTreeMap<SymbolId, usize>,
) {
    let map_len = public_to_column
        .keys()
        .map(|id| usize::from(id.0) + 1)
        .max()
        .unwrap_or(0);
    let mut inverse: Vec<_> = (0..map_len).map(|id| SymbolId(id as u16)).collect();
    for (&public, &column) in public_to_column {
        if let Ok(column) = u16::try_from(column) {
            inverse[usize::from(public.0)] = SymbolId(column);
        }
    }
    remap_grammar_symbols(grammar, &inverse);
    let remap = |id: SymbolId| inverse.get(usize::from(id.0)).copied().unwrap_or(id);
    for external in &mut grammar.externals {
        external.symbol_id = remap(external.symbol_id);
    }
    for symbol in grammar
        .extras
        .iter_mut()
        .chain(&mut grammar.supertypes)
        .chain(&mut grammar.inline_rules)
    {
        *symbol = remap(*symbol);
    }
    for precedence in &mut grammar.precedences {
        for symbol in &mut precedence.symbols {
            *symbol = remap(*symbol);
        }
    }
    for conflict in &mut grammar.conflicts {
        for symbol in &mut conflict.symbols {
            *symbol = remap(*symbol);
        }
    }
    grammar.rule_names = std::mem::take(&mut grammar.rule_names)
        .into_iter()
        .map(|(id, name)| (remap(id), name))
        .collect();
    grammar.lexical_metadata = std::mem::take(&mut grammar.lexical_metadata)
        .into_iter()
        .map(|(id, metadata)| (remap(id), metadata))
        .collect();
    grammar.wrapper_token_relations = std::mem::take(&mut grammar.wrapper_token_relations)
        .into_iter()
        .map(|(wrapper, token)| (remap(wrapper), remap(token)))
        .collect();
    grammar.start_symbol = grammar.start_symbol.map(remap);
    grammar.document_start_symbol = grammar.document_start_symbol.map(remap);
    grammar.word_token = grammar.word_token.map(remap);
    // A producer registry belongs to its public grammar, never this derived view.
    grammar.symbol_registry = None;
}

#[cfg(test)]
mod identity_tests {
    use super::*;
    use adze_ir::{ExternalToken, FieldId, Grammar, ProductionId, Rule, Symbol};

    #[test]
    fn test_decoded_grammar_maps_public_owner_targets_and_tokens_once() {
        let mut grammar = Grammar::new("decoded_grammar".to_string());
        grammar.tokens.insert(
            SymbolId(1),
            Token {
                name: "number".to_string(),
                pattern: TokenPattern::Regex("[0-9]+".to_string()),
                fragile: false,
            },
        );
        grammar.externals.push(ExternalToken {
            name: "external".to_string(),
            symbol_id: SymbolId(20),
        });
        grammar.add_rule(Rule {
            lhs: SymbolId(3),
            rhs: vec![
                Symbol::Terminal(SymbolId(1)),
                Symbol::NonTerminal(SymbolId(2)),
                Symbol::External(SymbolId(4)),
            ],
            precedence: None,
            associativity: None,
            fields: vec![(FieldId(0), 0)],
            production_id: ProductionId(7),
        });
        remap_grammar_symbols(
            &mut grammar,
            &[
                SymbolId(0),
                SymbolId(11),
                SymbolId(12),
                SymbolId(13),
                SymbolId(20),
            ],
        );
        assert!(!grammar.rules.contains_key(&SymbolId(3)));
        let rule = &grammar.rules[&SymbolId(13)][0];
        assert_eq!(rule.lhs, SymbolId(13));
        assert_eq!(
            rule.rhs,
            vec![
                Symbol::Terminal(SymbolId(11)),
                Symbol::NonTerminal(SymbolId(12)),
                Symbol::External(SymbolId(20))
            ]
        );
        assert_eq!(rule.fields, vec![(FieldId(0), 0)]);
        assert_eq!(rule.production_id, ProductionId(7));
        assert_eq!(grammar.tokens[&SymbolId(11)].name, "number");
        assert_eq!(grammar.externals[0].symbol_id, SymbolId(20));
    }
}
