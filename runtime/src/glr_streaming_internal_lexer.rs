//! Generated internal-lexer adapter for stack-aware GLR streaming (#857 / #889).
//!
//! Invokes a generated `TSLanguage::lex_fn` at a single byte position with the
//! caller-supplied [`LexMode`]. Unlike the fixed-mode pretokenization bridge, this
//! adapter does not pre-scan the whole input or hard-code ASCII whitespace skips.

#![cfg(all(feature = "glr", feature = "pure-rust"))]

use adze_glr_core::LexMode;
use adze_glr_core::ts_lexer::NextToken;
use core::ffi::c_void;

use crate::lex::TsLexer;
use crate::pure_parser::{TSLanguage, TSLexState};

/// Structured failure from a single generated-lexer invocation.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum StreamingInternalLexError {
    /// The language does not expose a generated lexer function.
    #[error("generated language is missing lex_fn")]
    MissingLexerFn,
    /// The lexer made no progress at the requested position.
    #[error("generated lexer made no progress at byte {pos}")]
    NoProgress {
        /// Byte offset where lexing stalled.
        pos: usize,
    },
    /// The lexer produced a zero-width token.
    #[error("generated lexer produced a zero-width token at byte {pos}")]
    ZeroWidth {
        /// Byte offset where lexing started.
        pos: usize,
    },
    /// Exceeded the bounded extra-skip loop while searching for a real token.
    #[error("generated lexer exceeded extra-skip budget at byte {pos}")]
    ExtraSkipBudgetExceeded {
        /// Byte offset where the budget was exhausted.
        pos: usize,
    },
}

/// Maximum number of consecutive grammar extras the adapter will skip at one call.
const MAX_EXTRA_SKIPS: usize = 256;

fn lex_mode_to_ts_lex_state(mode: LexMode) -> TSLexState {
    TSLexState {
        lex_state: mode.lex_state,
        external_lex_state: mode.external_lex_state,
    }
}

fn is_extra_symbol(language: &TSLanguage, symbol: u16) -> bool {
    if symbol >= language.symbol_count as u16 || language.symbol_metadata.is_null() {
        return false;
    }
    // SAFETY: `symbol < symbol_count` and generated languages expose a
    // `symbol_metadata` array with `symbol_count` entries.
    unsafe { (*language.symbol_metadata.add(symbol as usize) & 0x04) != 0 }
}

/// Lex the next non-extra token at `pos` using the generated internal lexer.
pub fn lex_generated_internal_at(
    language: &TSLanguage,
    input: &str,
    pos: usize,
    mode: LexMode,
) -> Result<Option<NextToken>, StreamingInternalLexError> {
    let lex_fn = language
        .lex_fn
        .ok_or(StreamingInternalLexError::MissingLexerFn)?;
    let source = input.as_bytes();
    if pos >= source.len() {
        return Ok(None);
    }

    let mut cursor = pos;
    let mut extra_skips = 0usize;

    loop {
        if cursor >= source.len() {
            return Ok(None);
        }
        if extra_skips > MAX_EXTRA_SKIPS {
            return Err(StreamingInternalLexError::ExtraSkipBudgetExceeded { pos });
        }

        let start = cursor;
        let token = lex_once(
            language,
            lex_fn,
            source,
            start,
            lex_mode_to_ts_lex_state(mode),
        )?;

        let Some((symbol, end)) = token else {
            return Err(StreamingInternalLexError::NoProgress { pos: start });
        };

        if end <= start {
            return Err(StreamingInternalLexError::ZeroWidth { pos: start });
        }

        if is_extra_symbol(language, symbol) {
            cursor = end;
            extra_skips += 1;
            continue;
        }

        return Ok(Some(NextToken {
            kind: symbol as u32,
            start: start as u32,
            end: end as u32,
        }));
    }
}

fn lex_once(
    _language: &TSLanguage,
    lex_fn: unsafe extern "C" fn(*mut c_void, TSLexState) -> bool,
    source: &[u8],
    pos: usize,
    lex_mode: TSLexState,
) -> Result<Option<(u16, usize)>, StreamingInternalLexError> {
    #[repr(C)]
    struct Backing<'a> {
        input: &'a [u8],
        pos: usize,
        mark: usize,
    }

    unsafe extern "C" fn lookahead(lex: *mut TsLexer) -> u32 {
        unsafe {
            if lex.is_null() || (*lex).data.is_null() {
                return 0;
            }
            let backing = &*((*lex).data as *const Backing);
            if backing.pos < backing.input.len() {
                backing.input[backing.pos] as u32
            } else {
                0
            }
        }
    }

    unsafe extern "C" fn advance(lex: *mut TsLexer, _skip: bool) {
        unsafe {
            if lex.is_null() || (*lex).data.is_null() {
                return;
            }
            let backing = &mut *((*lex).data as *mut Backing);
            if backing.pos < backing.input.len() {
                backing.pos += 1;
            }
        }
    }

    unsafe extern "C" fn mark_end(lex: *mut TsLexer) {
        unsafe {
            if lex.is_null() || (*lex).data.is_null() {
                return;
            }
            let backing = &mut *((*lex).data as *mut Backing);
            backing.mark = backing.pos;
        }
    }

    let mut backing = Backing {
        input: source,
        pos,
        mark: pos,
    };
    let mut ts_lexer = TsLexer {
        lookahead,
        advance,
        mark_end,
        result_symbol: u16::MAX,
        data: &mut backing as *mut _ as *mut c_void,
    };

    // SAFETY: `lex_fn` is the generated language lexer. `ts_lexer` uses the
    // same `TsLexer` ABI layout that generated lexers expect.
    let ok = unsafe { lex_fn(&mut ts_lexer as *mut _ as *mut c_void, lex_mode) };
    if !ok || ts_lexer.result_symbol == u16::MAX {
        return Ok(None);
    }

    let end = if backing.mark > pos {
        backing.mark
    } else {
        backing.pos
    };

    Ok(Some((ts_lexer.result_symbol, end)))
}

/// Build a `Driver::parse_streaming` internal-lexer closure for a generated language.
pub fn make_generated_internal_streaming_lexer(
    language: &'static TSLanguage,
) -> impl FnMut(&str, usize, LexMode) -> Option<NextToken> + '_ {
    move |input, pos, mode| {
        lex_generated_internal_at(language, input, pos, mode)
            .ok()
            .flatten()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pure_parser::{ExternalScanner, SyncPtr, TSParseAction, TSRule};
    use core::ptr;

    unsafe extern "C" fn stub_lex_fn(state_ptr: *mut c_void, _lex_mode: TSLexState) -> bool {
        if state_ptr.is_null() {
            return false;
        }
        let lexer = state_ptr as *mut TsLexer;
        let first = unsafe { ((*lexer).lookahead)(lexer) };
        if first == b'a' as u32 {
            unsafe {
                ((*lexer).advance)(lexer, false);
                (*lexer).result_symbol = 7;
                ((*lexer).mark_end)(lexer);
            }
            return true;
        }
        false
    }

    fn stub_language(
        lex_fn: Option<unsafe extern "C" fn(*mut c_void, TSLexState) -> bool>,
    ) -> TSLanguage {
        static SYMBOL_METADATA: [u8; 8] = [0, 0, 0, 0, 0x04, 0, 0, 0];
        TSLanguage {
            version: 15,
            symbol_count: 8,
            alias_count: 0,
            token_count: 4,
            external_token_count: 0,
            state_count: 1,
            large_state_count: 0,
            production_id_count: 0,
            field_count: 0,
            max_alias_sequence_length: 0,
            production_id_map: ptr::null(),
            parse_table: ptr::null(),
            small_parse_table: ptr::null(),
            small_parse_table_map: ptr::null(),
            parse_actions: ptr::null::<TSParseAction>(),
            symbol_names: ptr::null(),
            field_names: ptr::null(),
            field_map_slices: ptr::null(),
            field_map_entries: ptr::null(),
            symbol_metadata: SYMBOL_METADATA.as_ptr(),
            public_symbol_map: ptr::null(),
            alias_map: ptr::null(),
            alias_sequences: ptr::null(),
            lex_modes: ptr::null(),
            lex_fn,
            keyword_lex_fn: None,
            keyword_capture_token: 0,
            external_scanner: ExternalScanner::default(),
            primary_state_ids: ptr::null(),
            production_lhs_index: ptr::null(),
            production_count: 0,
            eof_symbol: 0,
            rules: ptr::null::<TSRule>(),
            rule_count: 0,
        }
    }

    #[test]
    fn internal_adapter_lexes_at_position_with_supplied_mode() {
        let language = stub_language(Some(stub_lex_fn));
        let token = lex_generated_internal_at(
            &language,
            "za",
            1,
            LexMode {
                lex_state: 3,
                external_lex_state: 0,
            },
        )
        .expect("lexing should succeed")
        .expect("token expected");

        assert_eq!(token.kind, 7);
        assert_eq!(token.start, 1);
        assert_eq!(token.end, 2);
    }

    #[test]
    fn internal_adapter_is_deterministic_for_same_position_and_mode() {
        let language = stub_language(Some(stub_lex_fn));
        let mode = LexMode {
            lex_state: 1,
            external_lex_state: 0,
        };
        let first =
            lex_generated_internal_at(&language, "a", 0, mode).expect("first call should succeed");
        let second =
            lex_generated_internal_at(&language, "a", 0, mode).expect("second call should succeed");
        assert_eq!(first, second);
    }

    #[test]
    fn internal_adapter_reports_no_progress_structured_error() {
        let language = stub_language(Some(stub_lex_fn));
        let err = lex_generated_internal_at(
            &language,
            "z",
            0,
            LexMode {
                lex_state: 0,
                external_lex_state: 0,
            },
        )
        .expect_err("no-progress input should fail");

        assert_eq!(err, StreamingInternalLexError::NoProgress { pos: 0 });
    }
}
