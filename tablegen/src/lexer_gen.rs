// Lexer generation for pure-Rust parser (#926).
//
// Generates a mode-aware, non-destructive maximal-munch lexer. Failed candidates
// never mutate the shared cursor; lexical priority and length break ties.
//
// NOTE: Generated code contains `unsafe` blocks that dereference `*mut TsLexer`.
// The runtime guarantees the pointer is valid for the duration of `lexer_fn`.

use adze_glr_core::{Action, ParseTable};
use adze_ir::{Grammar, LexicalMetadata, SymbolId, TokenPattern};
use proc_macro2::TokenStream;
use quote::quote;
use std::collections::{BTreeMap, BTreeSet};

/// Prepared lexer candidate at tablegen time.
#[derive(Debug, Clone)]
struct LexCandidate {
    symbol_index: u16,
    pattern: CandidatePattern,
    metadata: LexicalMetadata,
}

#[derive(Debug, Clone)]
enum CandidatePattern {
    Literal(Vec<u8>),
    DigitPlus,
    WordPlus,
    LowerPlus,
    Identifier,
    CharClass(&'static [u8]),
    Whitespace,
}

/// Generate a mode-aware lexer function for the grammar.
pub fn generate_lexer(
    grammar: &Grammar,
    symbol_to_index: &BTreeMap<SymbolId, usize>,
) -> TokenStream {
    generate_lexer_with_table(grammar, symbol_to_index, None)
}

/// Generate a mode-aware lexer with optional parse-table mode eligibility.
pub fn generate_lexer_with_table(
    grammar: &Grammar,
    symbol_to_index: &BTreeMap<SymbolId, usize>,
    parse_table: Option<&ParseTable>,
) -> TokenStream {
    let word_token = grammar.word_token_symbol();
    let mode_candidates = build_mode_candidates(grammar, symbol_to_index, parse_table, word_token);
    let helper_needs = helper_needs_for(&mode_candidates);

    let mode_match_arms = mode_candidates
        .iter()
        .map(|(lex_state, candidates)| generate_mode_arm(*lex_state, candidates))
        .collect::<Vec<_>>();

    let unknown_mode_arm = quote! { _ => false, };

    let is_word_char_fn = if helper_needs.word_char {
        quote! {
            #[inline]
            fn is_word_char(byte: u32) -> bool {
                byte != 0 && ((byte as u8).is_ascii_alphanumeric() || byte == b'_' as u32)
            }
        }
    } else {
        quote! {}
    };

    let is_identifier_start_fn = if helper_needs.identifier_start {
        quote! {
            #[inline]
            fn is_identifier_start(byte: u32) -> bool {
                byte != 0 && ((byte as u8).is_ascii_alphabetic() || byte == b'_' as u32)
            }
        }
    } else {
        quote! {}
    };

    quote! {
        /// SAFETY: Runtime adapters store `TsLexer.data` as a pointer to a backing
        /// struct whose first fields are `(input: &[u8], pos: usize, ...)`.
        #[repr(C)]
        struct LexerBackingView {
            input_ptr: *const u8,
            input_len: usize,
            pos: usize,
        }

        /// SAFETY: `lexer` is a valid `TsLexer` for the duration of the call.
        unsafe fn lexer_view(lexer: *mut adze::lex::TsLexer) -> *mut LexerBackingView {
            unsafe { (*lexer).data as *mut LexerBackingView }
        }

        /// SAFETY: `lexer` is valid and its backing view matches `LexerBackingView`.
        unsafe fn lexer_byte_at_rel(lexer: *mut adze::lex::TsLexer, rel: usize) -> u32 {
            unsafe {
                let view = &*lexer_view(lexer);
                let idx = view.pos.saturating_add(rel);
                if idx < view.input_len {
                    *view.input_ptr.add(idx) as u32
                } else {
                    0
                }
            }
        }

        /// SAFETY: `lexer` is valid and its backing view matches `LexerBackingView`.
        unsafe fn lexer_pos(lexer: *mut adze::lex::TsLexer) -> usize {
            unsafe { (*lexer_view(lexer)).pos }
        }

        /// SAFETY: `lexer` is valid and its backing view matches `LexerBackingView`.
        unsafe fn lexer_set_pos(lexer: *mut adze::lex::TsLexer, pos: usize) {
            unsafe {
                (*lexer_view(lexer)).pos = pos;
            }
        }

        #is_word_char_fn

        #is_identifier_start_fn

        #[inline]
        fn better_match(
            new_sym: u16,
            new_len: usize,
            new_pri: i16,
            old_sym: u16,
            old_len: usize,
            old_pri: i16,
        ) -> bool {
            if new_len != old_len {
                return new_len > old_len;
            }
            if new_pri != old_pri {
                return new_pri > old_pri;
            }
            new_sym < old_sym
        }

        // SAFETY: Called by the GLR runtime which guarantees `state_ptr` is a valid
        // `*mut TsLexer` for the duration of the call.
        unsafe extern "C" fn lexer_fn(
            state_ptr: *mut ::std::ffi::c_void,
            lex_mode: adze::pure_parser::TSLexState,
        ) -> bool {
            if state_ptr.is_null() {
                return false;
            }

            let lexer = state_ptr as *mut adze::lex::TsLexer;
            match lex_mode.lex_state {
                #(#mode_match_arms)*
                #unknown_mode_arm
            }
        }
    }
}

#[derive(Debug, Default)]
struct HelperNeeds {
    word_char: bool,
    identifier_start: bool,
}

fn helper_needs_for(mode_candidates: &BTreeMap<u16, Vec<LexCandidate>>) -> HelperNeeds {
    let mut needs = HelperNeeds::default();
    for candidates in mode_candidates.values() {
        for candidate in candidates {
            match &candidate.pattern {
                CandidatePattern::WordPlus => needs.word_char = true,
                CandidatePattern::Identifier => {
                    needs.word_char = true;
                    needs.identifier_start = true;
                }
                CandidatePattern::Literal(bytes) => {
                    let is_keyword = bytes.iter().all(|b| b.is_ascii_alphabetic() || *b == b'_')
                        && bytes.len() > 1;
                    if is_keyword {
                        needs.word_char = true;
                    }
                }
                CandidatePattern::DigitPlus
                | CandidatePattern::LowerPlus
                | CandidatePattern::CharClass(_)
                | CandidatePattern::Whitespace => {}
            }
        }
    }
    needs
}

fn build_mode_candidates(
    grammar: &Grammar,
    symbol_to_index: &BTreeMap<SymbolId, usize>,
    parse_table: Option<&ParseTable>,
    word_token: Option<SymbolId>,
) -> BTreeMap<u16, Vec<LexCandidate>> {
    let mut all_candidates = collect_candidates(grammar, symbol_to_index, word_token);
    all_candidates.sort_by(|left, right| {
        right
            .metadata
            .lexical_priority
            .cmp(&left.metadata.lexical_priority)
            .then_with(|| right.pattern.max_length().cmp(&left.pattern.max_length()))
            .then_with(|| left.symbol_index.cmp(&right.symbol_index))
    });

    let Some(parse_table) = parse_table else {
        let mut modes = BTreeMap::new();
        modes.insert(0, all_candidates);
        return modes;
    };

    let eligibility = build_lex_state_eligibility(parse_table);
    if eligibility.is_empty() {
        let mut modes = BTreeMap::new();
        modes.insert(0, all_candidates);
        return modes;
    }

    let mut modes: BTreeMap<u16, Vec<LexCandidate>> = BTreeMap::new();
    for (lex_state, columns) in eligibility {
        let mode_candidates = all_candidates
            .iter()
            .filter(|candidate| {
                matches!(
                    candidate.pattern,
                    CandidatePattern::Literal(_) | CandidatePattern::CharClass(_)
                ) || columns.contains(&(candidate.symbol_index as usize))
            })
            .cloned()
            .collect::<Vec<_>>();
        if !mode_candidates.is_empty() {
            modes.insert(lex_state, mode_candidates);
        }
    }

    if modes.is_empty() {
        let mut fallback = BTreeMap::new();
        fallback.insert(0, all_candidates);
        return fallback;
    }

    modes
}

fn collect_candidates(
    grammar: &Grammar,
    symbol_to_index: &BTreeMap<SymbolId, usize>,
    word_token: Option<SymbolId>,
) -> Vec<LexCandidate> {
    let mut candidates = Vec::new();
    for (symbol_id, token) in &grammar.tokens {
        let Some(&idx) = symbol_to_index.get(symbol_id) else {
            continue;
        };
        let is_word = word_token == Some(*symbol_id);
        let pattern = match pattern_to_candidate(&token.pattern, is_word) {
            Some(pattern) => pattern,
            None => continue,
        };
        candidates.push(LexCandidate {
            symbol_index: idx as u16,
            pattern,
            metadata: grammar.lexical_metadata_for(*symbol_id),
        });
    }
    candidates
}

fn pattern_to_candidate(pattern: &TokenPattern, is_word_token: bool) -> Option<CandidatePattern> {
    match pattern {
        TokenPattern::String(value) => Some(CandidatePattern::Literal(value.as_bytes().to_vec())),
        TokenPattern::Regex(regex) => {
            if is_word_token || regex == r"[a-zA-Z_][a-zA-Z0-9_]*" {
                return Some(CandidatePattern::Identifier);
            }
            if regex.len() == 1 {
                return Some(CandidatePattern::Literal(regex.as_bytes().to_vec()));
            }
            match regex.as_str() {
                r"\d+" => Some(CandidatePattern::DigitPlus),
                r"\w+" => Some(CandidatePattern::WordPlus),
                r"[a-z]+" => Some(CandidatePattern::LowerPlus),
                r"[-+*/]" => Some(CandidatePattern::CharClass(b"-+*/")),
                r"\s" | r"\s+" | r"\s*" => Some(CandidatePattern::Whitespace),
                _ => None,
            }
        }
    }
}

fn build_lex_state_eligibility(parse_table: &ParseTable) -> BTreeMap<u16, BTreeSet<usize>> {
    let mut map: BTreeMap<u16, BTreeSet<usize>> = BTreeMap::new();
    for (state_idx, row) in parse_table.action_table.iter().enumerate() {
        let lex_state = parse_table
            .lex_modes
            .get(state_idx)
            .map(|mode| mode.lex_state)
            .unwrap_or(0);
        let entry = map.entry(lex_state).or_default();
        for (col_idx, cell) in row.iter().enumerate() {
            if cell.iter().any(|action| matches!(action, Action::Shift(_))) {
                entry.insert(col_idx);
            }
        }
    }
    map
}

impl CandidatePattern {
    fn max_length(&self) -> usize {
        match self {
            CandidatePattern::Literal(bytes) => bytes.len(),
            CandidatePattern::DigitPlus
            | CandidatePattern::WordPlus
            | CandidatePattern::LowerPlus
            | CandidatePattern::Identifier
            | CandidatePattern::Whitespace => usize::MAX,
            CandidatePattern::CharClass(_) => 1,
        }
    }
}

fn generate_mode_arm(lex_state: u16, candidates: &[LexCandidate]) -> TokenStream {
    let candidate_blocks = candidates
        .iter()
        .map(generate_candidate_block)
        .collect::<Vec<_>>();

    quote! {
        #lex_state => {
            let mut best_sym: Option<u16> = None;
            let mut best_len: usize = 0;
            let mut best_pri: i16 = i16::MIN;

            #(
                {
                    #candidate_blocks
                }
            )*

            let _ = best_pri;

            if let Some(sym) = best_sym {
                let start = unsafe { lexer_pos(lexer) };
                unsafe {
                    lexer_set_pos(lexer, start + best_len);
                    (*lexer).result_symbol = sym;
                    ((*lexer).mark_end)(lexer);
                }
                return true;
            }
            false
        }
    }
}

fn generate_candidate_block(candidate: &LexCandidate) -> TokenStream {
    let sym = candidate.symbol_index;
    let pri = candidate.metadata.lexical_priority;
    let match_expr = generate_match_expr(&candidate.pattern);

    quote! {
        match (|| unsafe { #match_expr })() {
            Some(len) if len > 0 => {
                let replace = match best_sym {
                    None => true,
                    Some(old_sym) => better_match(#sym, len, #pri, old_sym, best_len, best_pri),
                };
                if replace {
                    best_sym = Some(#sym);
                    best_len = len;
                    best_pri = #pri;
                }
            }
            _ => {}
        }
    }
}

fn generate_match_expr(pattern: &CandidatePattern) -> TokenStream {
    match pattern {
        CandidatePattern::Literal(bytes) => {
            let byte_checks = bytes.iter().enumerate().map(|(idx, byte)| {
                let b = *byte as u32;
                let offset = idx;
                quote! {
                    if lexer_byte_at_rel(lexer, #offset) != #b {
                        return None;
                    }
                }
            });
            let len = bytes.len();
            let is_keyword =
                bytes.iter().all(|b| b.is_ascii_alphabetic() || *b == b'_') && bytes.len() > 1;
            if is_keyword {
                quote! {
                    #(#byte_checks)*
                    let next = lexer_byte_at_rel(lexer, #len);
                    if next != 0 && is_word_char(next) {
                        return None;
                    }
                    Some(#len)
                }
            } else {
                quote! {
                    #(#byte_checks)*
                    Some(#len)
                }
            }
        }
        CandidatePattern::DigitPlus => quote! {
            if !((lexer_byte_at_rel(lexer, 0) as u8).is_ascii_digit()) {
                return None;
            }
            let mut len = 1usize;
            while (lexer_byte_at_rel(lexer, len) as u8).is_ascii_digit() {
                len += 1;
            }
            Some(len)
        },
        CandidatePattern::WordPlus => quote! {
            if !is_word_char(lexer_byte_at_rel(lexer, 0)) {
                return None;
            }
            let mut len = 1usize;
            while is_word_char(lexer_byte_at_rel(lexer, len)) {
                len += 1;
            }
            Some(len)
        },
        CandidatePattern::LowerPlus => quote! {
            if !(lexer_byte_at_rel(lexer, 0) as u8).is_ascii_lowercase() {
                return None;
            }
            let mut len = 1usize;
            while (lexer_byte_at_rel(lexer, len) as u8).is_ascii_lowercase() {
                len += 1;
            }
            Some(len)
        },
        CandidatePattern::Identifier => quote! {
            if !is_identifier_start(lexer_byte_at_rel(lexer, 0)) {
                return None;
            }
            let mut len = 1usize;
            while is_word_char(lexer_byte_at_rel(lexer, len)) {
                len += 1;
            }
            Some(len)
        },
        CandidatePattern::CharClass(chars) => {
            let checks = chars.iter().map(|ch| {
                let c = *ch as u32;
                quote! { first == #c }
            });
            quote! {
                let first = lexer_byte_at_rel(lexer, 0);
                if first == 0 {
                    return None;
                }
                if #(#checks)||* {
                    Some(1)
                } else {
                    None
                }
            }
        }
        CandidatePattern::Whitespace => quote! {
            if !(lexer_byte_at_rel(lexer, 0) as u8).is_ascii_whitespace() {
                return None;
            }
            let mut len = 1usize;
            while (lexer_byte_at_rel(lexer, len) as u8).is_ascii_whitespace() {
                len += 1;
            }
            Some(len)
        },
    }
}
