// Lexer generation for pure-Rust parser
//
// NOTE: This module generates `quote!` token streams containing `unsafe` blocks.
// The generated unsafe code dereferences a `*mut TsLexer` raw pointer passed by the
// GLR runtime. The safety contract is: the runtime guarantees the pointer is valid
// and exclusively borrowed for the duration of the `lexer_fn` call.
use adze_ir::{Grammar, SymbolId, TokenPattern};
use proc_macro2::TokenStream;
use quote::quote;
use std::collections::BTreeMap;

/// Generate a simple lexer function for the grammar
pub fn generate_lexer(
    grammar: &Grammar,
    symbol_to_index: &BTreeMap<SymbolId, usize>,
) -> TokenStream {
    // The lexer needs to return symbol indices that match the parse table.
    // We use the symbol_to_index mapping from the parse table to ensure consistency.

    // Collect all tokens and categorize them
    let mut keywords = Vec::new();
    let mut other_strings = Vec::new();
    let mut regex_patterns = Vec::new();
    let mut identifier_pattern = None;

    // Track which patterns we've already seen to avoid duplicates
    let mut seen_string_patterns = std::collections::HashSet::new();
    let mut seen_regex_patterns = std::collections::HashSet::new();

    // Sort tokens by name to process primary tokens (with meaningful names) first
    let mut sorted_tokens: Vec<_> = grammar.tokens.iter().collect();
    sorted_tokens.sort_by_key(|(_, token)| {
        if token.name.starts_with('_') && token.name[1..].chars().all(|c| c.is_ascii_digit()) {
            (1, token.name.clone())
        } else {
            (0, token.name.clone())
        }
    });

    for (id, token) in sorted_tokens {
        if let Some(&idx) = symbol_to_index.get(id) {
            let symbol_index = idx as u16;
            match &token.pattern {
                TokenPattern::String(s) => {
                    if seen_string_patterns.contains(s) {
                        continue;
                    }
                    seen_string_patterns.insert(s.clone());

                    if s.chars().all(|c| c.is_ascii_alphabetic() || c == '_') && s.len() > 1 {
                        keywords.push((symbol_index, s.clone()));
                    } else {
                        other_strings.push((symbol_index, s.clone()));
                    }
                }
                TokenPattern::Regex(pattern) => {
                    if seen_regex_patterns.contains(pattern) {
                        continue;
                    }
                    seen_regex_patterns.insert(pattern.clone());

                    if pattern == r"[a-zA-Z_][a-zA-Z0-9_]*" {
                        identifier_pattern = Some(symbol_index);
                    } else {
                        regex_patterns.push((symbol_index, pattern.clone()));
                    }
                }
            }
        }
    }

    // Sort keywords by length (longest first)
    keywords.sort_by_key(|(_, keyword)| std::cmp::Reverse(keyword.len()));

    let mut token_matches = Vec::new();

    // First: Add keyword matching
    for (symbol_index, keyword) in keywords {
        let bytes = keyword.as_bytes();
        let mut checks = Vec::new();
        for byte in bytes {
            let b = *byte as u32;
            checks.push(quote! {
                if ((*lexer).lookahead)(lexer) == #b {
                    ((*lexer).advance)(lexer, false);
                } else {
                    return false;
                }
            });
        }

        token_matches.push(quote! {
            if (|| unsafe {
                #(#checks)*
                let next = ((*lexer).lookahead)(lexer);
                if next != 0 && ((next as u8).is_ascii_alphanumeric() || next == b'_' as u32) {
                    return false;
                }
                true
            })() {
                unsafe {
                    (*lexer).result_symbol = #symbol_index;
                    ((*lexer).mark_end)(lexer);
                }
                return true;
            }
        });
    }

    // Second: Add other string patterns
    for (symbol_index, s) in other_strings {
        if s.len() == 1 {
            let ch = s.chars().next().expect("s.len() == 1 guarantees a char") as u32;
            token_matches.push(quote! {
                if unsafe { ((*lexer).lookahead)(lexer) == #ch } {
                    unsafe {
                        ((*lexer).advance)(lexer, false);
                        (*lexer).result_symbol = #symbol_index;
                        ((*lexer).mark_end)(lexer);
                    }
                    return true;
                }
            });
        } else {
            let bytes = s.as_bytes();
            let mut checks = Vec::new();
            for byte in bytes {
                let b = *byte as u32;
                checks.push(quote! {
                    if ((*lexer).lookahead)(lexer) == #b {
                        ((*lexer).advance)(lexer, false);
                    } else {
                        return false;
                    }
                });
            }
            token_matches.push(quote! {
                if (|| unsafe {
                    #(#checks)*
                    true
                })() {
                    unsafe {
                        (*lexer).result_symbol = #symbol_index;
                        ((*lexer).mark_end)(lexer);
                    }
                    return true;
                }
            });
        }
    }

    // Third: Add regex patterns
    for (symbol_index, pattern) in regex_patterns {
        if pattern == r"\d+" {
            token_matches.push(quote! {
                let first = unsafe { ((*lexer).lookahead)(lexer) };
                if first != 0 && (first as u8).is_ascii_digit() {
                    unsafe {
                        ((*lexer).advance)(lexer, false);
                        while {
                            let next = ((*lexer).lookahead)(lexer);
                            next != 0 && (next as u8).is_ascii_digit()
                        } {
                            ((*lexer).advance)(lexer, false);
                        }
                        (*lexer).result_symbol = #symbol_index;
                        ((*lexer).mark_end)(lexer);
                    }
                    return true;
                }
            });
        } else if pattern == r"\w+" {
            token_matches.push(quote! {
                let first = unsafe { ((*lexer).lookahead)(lexer) };
                if first != 0 && ((first as u8).is_ascii_alphanumeric() || first == b'_' as u32) {
                    unsafe {
                        ((*lexer).advance)(lexer, false);
                        while {
                            let next = ((*lexer).lookahead)(lexer);
                            next != 0 && ((next as u8).is_ascii_alphanumeric() || next == b'_' as u32)
                        } {
                            ((*lexer).advance)(lexer, false);
                        }
                        (*lexer).result_symbol = #symbol_index;
                        ((*lexer).mark_end)(lexer);
                    }
                    return true;
                }
            });
        } else if pattern == r"[a-z]+" {
            token_matches.push(quote! {
                let first = unsafe { ((*lexer).lookahead)(lexer) };
                if first != 0 && (first as u8).is_ascii_lowercase() {
                    unsafe {
                        ((*lexer).advance)(lexer, false);
                        while {
                            let next = ((*lexer).lookahead)(lexer);
                            next != 0 && (next as u8).is_ascii_lowercase()
                        } {
                            ((*lexer).advance)(lexer, false);
                        }
                        (*lexer).result_symbol = #symbol_index;
                        ((*lexer).mark_end)(lexer);
                    }
                    return true;
                }
            });
        } else if pattern == r"[-+*/]" {
            token_matches.push(quote! {
                let first = unsafe { ((*lexer).lookahead)(lexer) };
                if first == b'-' as u32 || first == b'+' as u32 || first == b'*' as u32 || first == b'/' as u32 {
                    unsafe {
                        ((*lexer).advance)(lexer, false);
                        (*lexer).result_symbol = #symbol_index;
                        ((*lexer).mark_end)(lexer);
                    }
                    return true;
                }
            });
        } else if pattern == r"\s" || pattern == r"\s+" || pattern == r"\s*" {
            token_matches.push(quote! {
                let first = unsafe { ((*lexer).lookahead)(lexer) };
                if first != 0 && (first as u8).is_ascii_whitespace() {
                    unsafe {
                        ((*lexer).advance)(lexer, false);
                        while {
                            let next = ((*lexer).lookahead)(lexer);
                            next != 0 && (next as u8).is_ascii_whitespace()
                        } {
                            ((*lexer).advance)(lexer, false);
                        }
                        (*lexer).result_symbol = #symbol_index;
                        ((*lexer).mark_end)(lexer);
                    }
                    return true;
                }
            });
        }
    }

    // Fourth: Add identifier pattern last
    if let Some(symbol_index) = identifier_pattern {
        token_matches.push(quote! {
            let first = unsafe { ((*lexer).lookahead)(lexer) };
            if first != 0 && ((first as u8).is_ascii_alphabetic() || first == b'_' as u32) {
                unsafe {
                    ((*lexer).advance)(lexer, false);
                    while {
                        let next = ((*lexer).lookahead)(lexer);
                        next != 0 && ((next as u8).is_ascii_alphanumeric() || next == b'_' as u32)
                    } {
                        ((*lexer).advance)(lexer, false);
                    }
                    (*lexer).result_symbol = #symbol_index;
                    ((*lexer).mark_end)(lexer);
                }
                return true;
            }
        });
    }

    quote! {
        // SAFETY: Called by the GLR runtime which guarantees `state_ptr` is a valid
        // `*mut TsLexer` for the duration of the call. The null check below guards
        // against the degenerate case.
        unsafe extern "C" fn lexer_fn(state_ptr: *mut ::std::ffi::c_void, _lex_mode: adze::pure_parser::TSLexState) -> bool {
            if state_ptr.is_null() {
                return false;
            }

            let lexer = state_ptr as *mut adze::lex::TsLexer;

            #(#token_matches)*

            false
        }
    }
}
