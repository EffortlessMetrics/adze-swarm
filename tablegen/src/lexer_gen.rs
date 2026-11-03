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
    /// Unicode-aware identifier: first char [`is_unicode_ident_start`], rest
    /// [`is_unicode_ident_continue`], decoded as validated UTF-8. ASCII inputs
    /// match exactly like [`CandidatePattern::Identifier`]; invalid UTF-8 at
    /// the cursor fails closed (`None`, cursor never advances).
    UnicodeIdentifier,
    CharClass(&'static [u8]),
    ByteClassPlus(&'static [u8]),
    LineComment(u8),
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

    let is_unicode_ident_fns = if helper_needs.unicode_ident {
        quote! {
            /// Byte width of the UTF-8 sequence starting with `first`, or `0`
            /// for bytes that can never start a character (continuation bytes
            /// `0x80..=0xBF`, overlong leads `0xC0..=0xC1`, `0xF5..=0xFF`).
            #[inline]
            fn utf8_char_width(first: u32) -> usize {
                match first {
                    0x01..=0x7F => 1,
                    0xC2..=0xDF => 2,
                    0xE0..=0xEF => 3,
                    0xF0..=0xF4 => 4,
                    _ => 0,
                }
            }

            /// SAFETY: `lexer` is valid and its backing view matches `LexerBackingView`.
            /// Decodes the UTF-8 character `rel` bytes past the cursor as
            /// `(byte_len, code_point)`. Returns `(0, 0)` at EOF or on invalid
            /// UTF-8 (bad continuation bytes, overlong forms, surrogates,
            /// `> U+10FFFF`, truncated sequences): callers stop the match
            /// without consuming bytes (fail closed). A NUL byte reads as EOF,
            /// consistent with `lexer_byte_at_rel`.
            unsafe fn lexer_utf8_char_at_rel(
                lexer: *mut adze::lex::TsLexer,
                rel: usize,
            ) -> (usize, u32) {
                unsafe {
                    let b0 = lexer_byte_at_rel(lexer, rel);
                    if b0 < 0x80 {
                        // Covers EOF: `0` reads as end of input everywhere in this lexer.
                        return ((b0 != 0) as usize, b0);
                    }
                    let width = utf8_char_width(b0);
                    if width < 2 {
                        return (0, 0);
                    }
                    let b1 = lexer_byte_at_rel(lexer, rel + 1);
                    if !(0x80..=0xBF).contains(&b1) {
                        return (0, 0);
                    }
                    if width == 2 {
                        return (2, ((b0 & 0x1F) << 6) | (b1 & 0x3F));
                    }
                    let b2 = lexer_byte_at_rel(lexer, rel + 2);
                    if !(0x80..=0xBF).contains(&b2) {
                        return (0, 0);
                    }
                    if width == 3 {
                        let cp = ((b0 & 0x0F) << 12) | ((b1 & 0x3F) << 6) | (b2 & 0x3F);
                        if cp < 0x800 || (0xD800..=0xDFFF).contains(&cp) {
                            return (0, 0);
                        }
                        return (3, cp);
                    }
                    let b3 = lexer_byte_at_rel(lexer, rel + 3);
                    if !(0x80..=0xBF).contains(&b3) {
                        return (0, 0);
                    }
                    let cp =
                        ((b0 & 0x07) << 18) | ((b1 & 0x3F) << 12) | ((b2 & 0x3F) << 6) | (b3 & 0x3F);
                    if !(0x10000..=0x10FFFF).contains(&cp) {
                        return (0, 0);
                    }
                    (4, cp)
                }
            }

            /// Unicode identifier-start predicate (XID_Start approximation).
            /// ASCII fast path is exact (`A-Z`, `a-z`, `_`); non-ASCII defers
            /// to `char::is_alphabetic`. Documented deviation: Alphabetic
            /// admits a handful of marks excluded from XID_Start; the
            /// combining-mark table below only widens Continue, never Start.
            #[inline]
            fn is_unicode_ident_start(cp: u32) -> bool {
                matches!(cp, 0x41..=0x5A | 0x61..=0x7A | 0x5F)
                    || (cp >= 0x80 && char::from_u32(cp).is_some_and(|c| c.is_alphabetic()))
            }

            /// Unicode identifier-continue predicate (XID_Continue approximation).
            /// Start characters and ASCII digits are exact; non-ASCII defers to
            /// `char::is_alphanumeric` plus the Mn/Mc combining-mark ranges
            /// `char` cannot query. Documented deviation: `is_alphanumeric`
            /// also admits Nl/No (e.g. superscripts) which XID_Continue excludes.
            #[inline]
            fn is_unicode_ident_continue(cp: u32) -> bool {
                if is_unicode_ident_start(cp) || matches!(cp, 0x30..=0x39) {
                    return true;
                }
                if cp < 0x80 {
                    return false;
                }
                char::from_u32(cp).is_some_and(|c| c.is_alphanumeric())
                    || matches!(
                        cp,
                        0x0300..=0x036F
                            | 0x1AB0..=0x1AFF
                            | 0x1DC0..=0x1DFF
                            | 0x20D0..=0x20FF
                            | 0xFE20..=0xFE2F
                    )
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

        #is_unicode_ident_fns

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
    unicode_ident: bool,
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
                CandidatePattern::UnicodeIdentifier => needs.unicode_ident = true,
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
                | CandidatePattern::ByteClassPlus(_)
                | CandidatePattern::LineComment(_)
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
            .filter(|candidate| columns.contains(&(candidate.symbol_index as usize)))
            .cloned()
            .collect::<Vec<_>>();
        // An explicitly empty mode rejects all candidates. It must not fall
        // back to the unfiltered lexer used when no table was supplied.
        modes.insert(lex_state, mode_candidates);
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
        // Explicit (never silent) drop: a pattern this rejects has no lexer
        // candidate. [`unsupported_pattern_reason`] returns `Some(reason)` for
        // exactly the patterns skipped here, so codegen and lint entry points
        // can surface the gap instead of dropping the token quietly.
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
            // Unicode spellings win over the word-token fallback below: a word
            // token declared with an XID spelling must stay unicode-aware.
            // Canonical spellings (XID_Start/XID_Continue exclude `_`, so the
            // underscore stays explicit in both classes):
            //   [\p{XID_Start}_][\p{XID_Continue}_]*  |  \p{XID_Start}\p{XID_Continue}*
            // This reorder cannot change ASCII behavior (the fallback still
            // maps every other word-token regex to ASCII `Identifier`).
            if matches!(
                regex.as_str(),
                r"[\p{XID_Start}_][\p{XID_Continue}_]*" | r"\p{XID_Start}\p{XID_Continue}*"
            ) {
                return Some(CandidatePattern::UnicodeIdentifier);
            }
            if is_word_token || regex == r"[a-zA-Z_][a-zA-Z0-9_]*" {
                return Some(CandidatePattern::Identifier);
            }
            if regex.len() == 1 {
                return Some(CandidatePattern::Literal(regex.as_bytes().to_vec()));
            }
            match regex.as_str() {
                r"\d+" | r"[0-9]+" => Some(CandidatePattern::DigitPlus),
                r"\w+" => Some(CandidatePattern::WordPlus),
                r"[a-z]+" => Some(CandidatePattern::LowerPlus),
                r"[-+*/]" => Some(CandidatePattern::CharClass(b"-+*/")),
                r"\s" | r"\s+" | r"\s*" => Some(CandidatePattern::Whitespace),
                r"[ \t\r\n]+" => Some(CandidatePattern::ByteClassPlus(b" \t\r\n")),
                r"#[^\r\n]*" => Some(CandidatePattern::LineComment(b'#')),
                _ => None,
            }
        }
    }
}

/// Explicit reason a [`TokenPattern`] has no lexer candidate.
///
/// Returns `None` exactly when [`pattern_to_candidate`] succeeds, and
/// `Some(reason)` for exactly the patterns [`collect_candidates`] skips, so
/// the drop is queryable and never silent. Callers that need a hard error
/// (grammar lint, codegen entry points) should surface this string; turning
/// the skip into a deny-by-default diagnostic belongs to the tool/ lane and
/// needs the ir validation allowlist (`ir/src/lexical.rs`) kept in sync.
pub fn unsupported_pattern_reason(pattern: &TokenPattern, is_word_token: bool) -> Option<String> {
    if pattern_to_candidate(pattern, is_word_token).is_some() {
        return None;
    }
    Some(match pattern {
        TokenPattern::String(value) => format!(
            "string literal {value:?} has no lexer candidate (unreachable: literals always lower)"
        ),
        TokenPattern::Regex(value) => format!(
            "regex {value:?} has no CandidatePattern arm; add one or use a \
             supported unicode-identifier spelling \
             ([\\p{{XID_Start}}_][\\p{{XID_Continue}}_]*)"
        ),
    })
}

/// Byte width of the UTF-8 sequence starting with `first`, or `None` for bytes
/// that can never start a character (continuation bytes, overlong leads
/// `0xC0..=0xC1`, `0xF5..=0xFF`). Tablegen-side oracle for the generated
/// `utf8_char_width` helper; the two must agree.
#[cfg(test)]
fn utf8_sequence_width(first: u8) -> Option<usize> {
    match first {
        0x00..=0x7F => Some(1),
        0xC2..=0xDF => Some(2),
        0xE0..=0xEF => Some(3),
        0xF0..=0xF4 => Some(4),
        _ => None,
    }
}

/// Decode the character at `input[pos]` with full validation (continuation
/// bytes, overlong forms, surrogates, `> U+10FFFF`, truncation). Returns
/// `(code_point, byte_len)`; `None` fails closed. Oracle for the generated
/// `lexer_utf8_char_at_rel`; the two must agree.
#[cfg(test)]
fn decode_utf8_at(input: &[u8], pos: usize) -> Option<(u32, usize)> {
    let b0 = *input.get(pos)?;
    if b0 < 0x80 {
        return Some((u32::from(b0), 1));
    }
    let width = utf8_sequence_width(b0)?;
    if width < 2 {
        return None;
    }
    let b1 = *input.get(pos + 1)?;
    if !(0x80..=0xBF).contains(&b1) {
        return None;
    }
    if width == 2 {
        return Some((((u32::from(b0) & 0x1F) << 6) | (u32::from(b1) & 0x3F), 2));
    }
    let b2 = *input.get(pos + 2)?;
    if !(0x80..=0xBF).contains(&b2) {
        return None;
    }
    if width == 3 {
        let cp =
            ((u32::from(b0) & 0x0F) << 12) | ((u32::from(b1) & 0x3F) << 6) | (u32::from(b2) & 0x3F);
        if cp < 0x800 || matches!(cp, 0xD800..=0xDFFF) {
            return None;
        }
        return Some((cp, 3));
    }
    let b3 = *input.get(pos + 3)?;
    if !matches!(b3, 0x80..=0xBF) {
        return None;
    }
    let cp = ((u32::from(b0) & 0x07) << 18)
        | ((u32::from(b1) & 0x3F) << 12)
        | ((u32::from(b2) & 0x3F) << 6)
        | (u32::from(b3) & 0x3F);
    if !(0x10000..=0x10FFFF).contains(&cp) {
        return None;
    }
    Some((cp, 4))
}

/// Unicode identifier-start predicate (XID_Start approximation). ASCII fast
/// path is exact (`A-Z`, `a-z`, `_`); non-ASCII defers to
/// `char::is_alphabetic`. See the generated `is_unicode_ident_start` for the
/// documented deviation; the two must agree.
#[cfg(test)]
fn is_unicode_ident_start(cp: u32) -> bool {
    matches!(cp, 0x41..=0x5A | 0x61..=0x7A | 0x5F)
        || (cp >= 0x80 && char::from_u32(cp).is_some_and(|c| c.is_alphabetic()))
}

/// Mn/Mc combining-mark ranges admitted as identifier-continue beyond what
/// `char::is_alphanumeric` covers. Partial table (not full Mn/Mc): grammars
/// needing wider scripts own their candidate spelling.
#[cfg(test)]
fn is_combining_mark(cp: u32) -> bool {
    matches!(
        cp,
        0x0300..=0x036F | 0x1AB0..=0x1AFF | 0x1DC0..=0x1DFF | 0x20D0..=0x20FF | 0xFE20..=0xFE2F
    )
}

/// Unicode identifier-continue predicate (XID_Continue approximation). Oracle
/// for the generated `is_unicode_ident_continue`; the two must agree.
#[cfg(test)]
fn is_unicode_ident_continue(cp: u32) -> bool {
    if is_unicode_ident_start(cp) || matches!(cp, 0x30..=0x39) {
        return true;
    }
    if cp < 0x80 {
        return false;
    }
    char::from_u32(cp).is_some_and(|c| c.is_alphanumeric()) || is_combining_mark(cp)
}

/// Match a unicode identifier at the start of `input`, returning its byte
/// length, or `None` when no valid identifier starts here. Invalid UTF-8 at
/// the cursor fails closed (`None`, cursor never advances). Multi-byte
/// characters are consumed whole: boundaries always fall on char boundaries.
#[cfg(test)]
fn unicode_ident_match_len(input: &[u8]) -> Option<usize> {
    let (cp, width) = decode_utf8_at(input, 0)?;
    if !is_unicode_ident_start(cp) {
        return None;
    }
    let mut len = width;
    while let Some((cp, width)) = decode_utf8_at(input, len) {
        if !is_unicode_ident_continue(cp) {
            break;
        }
        len += width;
    }
    Some(len)
}

/// Keyword-boundary oracle: after a keyword of `len` bytes, `false` means a
/// longer identifier continues here, so the keyword must not match. ASCII
/// word bytes block as before; ANY non-ASCII byte (`>= 0x80`, valid UTF-8 or
/// not) also blocks, so `ifé` never lexes keyword `if`. Invalid UTF-8 fails
/// closed (`false`). Mirrors the generated literal guard; the two must agree.
#[cfg(test)]
fn keyword_boundary_ok(input: &[u8], len: usize) -> bool {
    match input.get(len) {
        None => true,
        Some(&next) if next < 0x80 => !(next.is_ascii_alphanumeric() || next == b'_'),
        Some(_) => false,
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
            // Lexing precedes reduction: a real lookahead may first drive a
            // Reduce action before it becomes shiftable (#1174).
            if cell.iter().any(Action::uses_lookahead) {
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
            | CandidatePattern::UnicodeIdentifier
            | CandidatePattern::ByteClassPlus(_)
            | CandidatePattern::LineComment(_)
            | CandidatePattern::Whitespace => usize::MAX,
            CandidatePattern::CharClass(_) => 1,
        }
    }
}

fn generate_mode_arm(lex_state: u16, candidates: &[LexCandidate]) -> TokenStream {
    if candidates.is_empty() {
        return quote! { #lex_state => false, };
    }
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
                    // Any non-ASCII byte (>= 0x80) also extends the identifier:
                    // it may start or continue a multi-byte (unicode) identifier,
                    // so the keyword must not match its own prefix (e.g. `ifé`
                    // never lexes keyword `if`). Invalid UTF-8 fails closed.
                    if next != 0 && (is_word_char(next) || next >= 0x80) {
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
        CandidatePattern::UnicodeIdentifier => quote! {
            {
                let (mut len, first_cp) = lexer_utf8_char_at_rel(lexer, 0);
                if len == 0 || !is_unicode_ident_start(first_cp) {
                    return None;
                }
                loop {
                    let (width, cp) = lexer_utf8_char_at_rel(lexer, len);
                    if width == 0 || !is_unicode_ident_continue(cp) {
                        break;
                    }
                    len += width;
                }
                Some(len)
            }
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
        CandidatePattern::ByteClassPlus(chars) => {
            let checks = chars.iter().map(|ch| {
                let byte = *ch as u32;
                quote! { byte == #byte }
            });
            quote! {
                let mut len = 0usize;
                loop {
                    let byte = lexer_byte_at_rel(lexer, len);
                    if !(#(#checks)||*) {
                        break;
                    }
                    len += 1;
                }
                if len == 0 {
                    return None;
                }
                Some(len)
            }
        }
        CandidatePattern::LineComment(prefix) => {
            let prefix = *prefix as u32;
            quote! {
                if lexer_byte_at_rel(lexer, 0) != #prefix {
                    return None;
                }
                let mut len = 1usize;
                let remaining = (*lexer_view(lexer)).input_len.saturating_sub(lexer_pos(lexer));
                while len < remaining {
                    let byte = lexer_byte_at_rel(lexer, len);
                    if byte == b'\r' as u32 || byte == b'\n' as u32 {
                        break;
                    }
                    len += 1;
                }
                Some(len)
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

#[cfg(test)]
mod unicode_candidate_tests {
    use super::*;
    use adze_ir::Token;

    fn regex_candidate(pattern: &str) -> Option<CandidatePattern> {
        pattern_to_candidate(&TokenPattern::Regex(pattern.to_string()), false)
    }

    fn discriminant_name(pattern: &CandidatePattern) -> &'static str {
        match pattern {
            CandidatePattern::Literal(_) => "Literal",
            CandidatePattern::DigitPlus => "DigitPlus",
            CandidatePattern::WordPlus => "WordPlus",
            CandidatePattern::LowerPlus => "LowerPlus",
            CandidatePattern::Identifier => "Identifier",
            CandidatePattern::UnicodeIdentifier => "UnicodeIdentifier",
            CandidatePattern::CharClass(_) => "CharClass",
            CandidatePattern::ByteClassPlus(_) => "ByteClassPlus",
            CandidatePattern::LineComment(_) => "LineComment",
            CandidatePattern::Whitespace => "Whitespace",
        }
    }

    #[test]
    fn unicode_identifier_spellings_yield_unicode_identifier() {
        for spelling in [
            r"[\p{XID_Start}_][\p{XID_Continue}_]*",
            r"\p{XID_Start}\p{XID_Continue}*",
        ] {
            let candidate = regex_candidate(spelling);
            assert!(
                matches!(candidate, Some(CandidatePattern::UnicodeIdentifier)),
                "{spelling} must lower to UnicodeIdentifier, got {candidate:?}",
            );
            // Word-token or not, the spelling keeps its candidate.
            assert!(matches!(
                pattern_to_candidate(&TokenPattern::Regex(spelling.to_string()), true),
                Some(CandidatePattern::UnicodeIdentifier)
            ));
        }
    }

    #[test]
    fn ascii_lowerings_unchanged() {
        let cases = [
            (r"[a-zA-Z_][a-zA-Z0-9_]*", "Identifier"),
            (r"\d+", "DigitPlus"),
            (r"[0-9]+", "DigitPlus"),
            (r"\w+", "WordPlus"),
            (r"[a-z]+", "LowerPlus"),
            (r"[-+*/]", "CharClass"),
            (r"\s", "Whitespace"),
            (r"\s+", "Whitespace"),
            (r"\s*", "Whitespace"),
            (r"[ \t\r\n]+", "ByteClassPlus"),
            (r"#[^\r\n]*", "LineComment"),
            ("x", "Literal"),
        ];
        for (spelling, expected) in cases {
            let candidate = regex_candidate(spelling);
            assert_eq!(
                candidate.as_ref().map(discriminant_name),
                Some(expected),
                "{spelling} lowering changed",
            );
        }
        // ASCII identifier still lowers even as the word token.
        assert!(matches!(
            pattern_to_candidate(
                &TokenPattern::Regex(r"[a-zA-Z_][a-zA-Z0-9_]*".to_string()),
                true
            ),
            Some(CandidatePattern::Identifier)
        ));
        // Bare string literals still lower to Literal.
        assert!(matches!(
            pattern_to_candidate(&TokenPattern::String("if".to_string()), false),
            Some(CandidatePattern::Literal(_))
        ));
    }

    #[test]
    fn unsupported_regex_is_explicit_never_silent() {
        let pattern = TokenPattern::Regex(r"[0-9a-f]+".to_string());
        assert!(
            pattern_to_candidate(&pattern, false).is_none(),
            "hex-class regex must still have no candidate"
        );
        let reason = unsupported_pattern_reason(&pattern, false)
            .expect("skipped patterns must carry an explicit reason");
        assert!(
            reason.contains("[0-9a-f]+"),
            "reason must name the offending pattern: {reason}"
        );
        // Supported patterns carry no reason.
        assert!(
            unsupported_pattern_reason(
                &TokenPattern::Regex(r"[a-zA-Z_][a-zA-Z0-9_]*".to_string()),
                false
            )
            .is_none()
        );
        assert!(
            unsupported_pattern_reason(
                &TokenPattern::Regex(r"[\p{XID_Start}_][\p{XID_Continue}_]*".to_string()),
                false
            )
            .is_none()
        );
    }

    #[test]
    fn unicode_identifier_lexes_whole_words() {
        // `café`: 3 ASCII bytes + 2-byte `é` (U+00E9).
        assert_eq!(unicode_ident_match_len("café".as_bytes()), Some(5));
        // Leading multi-byte char: `é`(2) + `x`(1).
        assert_eq!(unicode_ident_match_len("éx".as_bytes()), Some(3));
        // Base char + combining mark U+0301 (2 bytes in UTF-8).
        assert_eq!(unicode_ident_match_len("é".as_bytes()), Some(3));
        // CJK ideographs (Lo, 3 bytes each) lex whole.
        assert_eq!(unicode_ident_match_len("日本語".as_bytes()), Some(9));
        // Trailing ASCII digits continue the identifier.
        assert_eq!(unicode_ident_match_len("café2".as_bytes()), Some(6));
        // Identifier stops at a non-continue char without consuming it.
        assert_eq!(unicode_ident_match_len("café x".as_bytes()), Some(5));
        // Pure ASCII behaves exactly like the old Identifier candidate.
        assert_eq!(unicode_ident_match_len(b"abc"), Some(3));
        assert_eq!(unicode_ident_match_len(b"_x9"), Some(3));
        // No valid start: digits, empty input, invalid UTF-8 fail closed.
        assert_eq!(unicode_ident_match_len(b"9abc"), None);
        assert_eq!(unicode_ident_match_len(b""), None);
        assert_eq!(unicode_ident_match_len(b"\xffabc"), None);
        assert_eq!(unicode_ident_match_len(b"\xc3"), None);
        assert_eq!(unicode_ident_match_len(b"\xc0\xaf"), None);
        assert_eq!(unicode_ident_match_len(b"\xed\xa0\x80"), None);
        // Invalid UTF-8 mid-identifier stops the match without consuming it.
        assert_eq!(unicode_ident_match_len(b"ab\xffc"), Some(2));
    }

    #[test]
    fn keyword_boundary_holds_on_unicode_and_invalid_bytes() {
        // ASCII behavior unchanged.
        assert!(keyword_boundary_ok(b"if x", 2));
        assert!(keyword_boundary_ok(b"if(x", 2));
        assert!(keyword_boundary_ok(b"if", 2));
        assert!(!keyword_boundary_ok(b"ifx", 2));
        assert!(!keyword_boundary_ok(b"if_", 2));
        assert!(!keyword_boundary_ok(b"if9", 2));
        // Non-ASCII always extends the identifier: `ifé` never lexes `if`.
        assert!(!keyword_boundary_ok("ifé".as_bytes(), 2));
        assert!(!keyword_boundary_ok("if日本".as_bytes(), 2));
        // Truncated/invalid UTF-8 fails closed (keyword must not match).
        assert!(!keyword_boundary_ok(b"if\xc3", 2));
        assert!(!keyword_boundary_ok(b"if\xff", 2));
    }

    #[test]
    fn generated_unicode_matcher_decodes_utf8() {
        let tokens = generate_match_expr(&CandidatePattern::UnicodeIdentifier).to_string();
        for needle in [
            "lexer_utf8_char_at_rel",
            "is_unicode_ident_start",
            "is_unicode_ident_continue",
        ] {
            assert!(
                tokens.contains(needle),
                "unicode matcher must call {needle}: {tokens}"
            );
        }
    }

    #[test]
    fn generated_keyword_guard_covers_non_ascii() {
        let tokens = generate_match_expr(&CandidatePattern::Literal(b"if".to_vec())).to_string();
        assert!(
            tokens.contains("0x80"),
            "keyword guard must treat bytes >= 0x80 as identifier-continue: {tokens}"
        );
        assert!(
            tokens.contains("is_word_char"),
            "keyword guard must keep the ASCII word-char check: {tokens}"
        );
    }

    #[test]
    fn helper_needs_tracks_unicode_identifier() {
        let candidate = LexCandidate {
            symbol_index: 0,
            pattern: CandidatePattern::UnicodeIdentifier,
            metadata: LexicalMetadata::default(),
        };
        let mut modes = BTreeMap::new();
        modes.insert(0u16, vec![candidate]);
        let needs = helper_needs_for(&modes);
        assert!(needs.unicode_ident);
        assert!(
            !needs.word_char,
            "unicode matcher must not need the ASCII helper"
        );
        assert!(!needs.identifier_start);
    }

    #[test]
    fn end_to_end_lexer_emits_unicode_helpers() {
        let mut grammar = Grammar::default();
        grammar.tokens.insert(
            SymbolId(1),
            Token {
                name: "identifier".to_string(),
                pattern: TokenPattern::Regex(r"[\p{XID_Start}_][\p{XID_Continue}_]*".to_string()),
                fragile: false,
            },
        );
        grammar.tokens.insert(
            SymbolId(2),
            Token {
                name: "if".to_string(),
                pattern: TokenPattern::String("if".to_string()),
                fragile: false,
            },
        );
        let symbol_to_index = BTreeMap::from([(SymbolId(1), 0usize), (SymbolId(2), 1usize)]);
        let tokens = generate_lexer(&grammar, &symbol_to_index).to_string();
        for needle in [
            "is_unicode_ident_start",
            "is_unicode_ident_continue",
            "lexer_utf8_char_at_rel",
            "utf8_char_width",
            "is_alphabetic",
            "is_alphanumeric",
        ] {
            assert!(
                tokens.contains(needle),
                "generated lexer must emit {needle}"
            );
        }
        assert!(
            tokens.contains("0x80"),
            "generated keyword guard must cover non-ASCII bytes"
        );
    }
}
