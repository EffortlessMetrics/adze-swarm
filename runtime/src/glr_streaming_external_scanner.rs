//! Generated external-scanner adapter for stack-aware GLR streaming (#857 / #890).
//!
//! Invokes a generated `TSLanguage::external_scanner` at a byte position with the
//! union of valid external symbols supplied by the streaming driver.

#![cfg(all(feature = "glr", feature = "pure-rust", feature = "external_scanners"))]

use adze_glr_core::LexMode;
use adze_glr_core::ts_lexer::NextToken;
use core::ffi::c_void;

use crate::lex::TsLexer;
use crate::pure_parser::TSLanguage;

/// Structured failure from a single generated external-scanner invocation.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum StreamingExternalScanError {
    /// The language does not expose an external scanner.
    #[error("generated language is missing external_scanner.scan")]
    MissingScannerFn,
    /// No valid external symbols were supplied for this position.
    #[error("no valid external symbols at byte {pos}")]
    NoValidSymbols {
        /// Byte offset where scanning was attempted.
        pos: usize,
    },
    /// The scanner made no progress at the requested position.
    #[error("external scanner made no progress at byte {pos}")]
    NoProgress {
        /// Byte offset where scanning stalled.
        pos: usize,
    },
    /// The scanner emitted a symbol that is not currently valid.
    #[error("external scanner emitted invalid symbol {symbol} at byte {pos}")]
    InvalidSymbol {
        /// Emitted external symbol index.
        symbol: u16,
        /// Byte offset where scanning started.
        pos: usize,
    },
    /// The scanner produced a zero-width token.
    #[error("external scanner produced a zero-width token at byte {pos}")]
    ZeroWidth {
        /// Byte offset where scanning started.
        pos: usize,
    },
}

/// Persistent generated external scanner state for streaming lexing.
pub struct GeneratedExternalStreamingScanner<'a> {
    language: &'a TSLanguage,
    scanner_instance: *mut c_void,
}

impl<'a> GeneratedExternalStreamingScanner<'a> {
    /// Create a scanner wrapper for the given generated language.
    pub fn new(language: &'a TSLanguage) -> Self {
        let scanner_instance = language
            .external_scanner
            .create
            .map(|create| unsafe { create() })
            .unwrap_or(core::ptr::null_mut());

        Self {
            language,
            scanner_instance,
        }
    }

    /// Scan at `pos` when `valid_symbols` is the union mask from active GLR stacks.
    pub fn scan_at(
        &mut self,
        input: &str,
        pos: usize,
        valid_symbols: &[bool],
        _mode: LexMode,
    ) -> Result<Option<NextToken>, StreamingExternalScanError> {
        let scan_fn = self
            .language
            .external_scanner
            .scan
            .ok_or(StreamingExternalScanError::MissingScannerFn)?;

        if pos >= input.len() {
            return Ok(None);
        }

        if !valid_symbols.iter().any(|valid| *valid) {
            return Err(StreamingExternalScanError::NoValidSymbols { pos });
        }

        let ts_valid_symbols = to_tree_sitter_valid_symbols(valid_symbols);
        let source = input.as_bytes();
        let start = pos;
        let token = scan_once(
            self.language,
            scan_fn,
            self.scanner_instance,
            source,
            start,
            &ts_valid_symbols,
        )?;

        let Some((external_index, end)) = token else {
            return Err(StreamingExternalScanError::NoProgress { pos: start });
        };

        if end <= start {
            return Err(StreamingExternalScanError::ZeroWidth { pos: start });
        }

        let driver_index = external_index.saturating_sub(1) as usize;
        if !valid_symbols.get(driver_index).copied().unwrap_or(false) {
            return Err(StreamingExternalScanError::InvalidSymbol {
                symbol: external_index,
                pos: start,
            });
        }

        let kind = map_external_symbol(self.language, external_index);
        Ok(Some(NextToken {
            kind: kind as u32,
            start: start as u32,
            end: end as u32,
        }))
    }
}

impl Drop for GeneratedExternalStreamingScanner<'_> {
    fn drop(&mut self) {
        if !self.scanner_instance.is_null()
            && let Some(destroy) = self.language.external_scanner.destroy
        {
            // SAFETY: `scanner_instance` was created by the language `create` hook
            // and is destroyed exactly once here.
            unsafe {
                destroy(self.scanner_instance);
            }
        }
    }
}

/// Build a `Driver::parse_streaming` external-scanner closure for a generated language.
pub fn make_generated_external_streaming_scanner<'a>(
    language: &'a TSLanguage,
) -> GeneratedExternalStreamingScanner<'a> {
    GeneratedExternalStreamingScanner::new(language)
}

fn map_external_symbol(language: &TSLanguage, external_index: u16) -> u16 {
    if language.external_scanner.symbol_map.is_null() {
        return external_index;
    }

    // SAFETY: Tree-sitter external scanners use 1-based indices into `symbol_map`.
    unsafe {
        *language
            .external_scanner
            .symbol_map
            .add(external_index as usize)
    }
}

fn to_tree_sitter_valid_symbols(valid_symbols: &[bool]) -> Vec<bool> {
    let mut ts_valid = vec![false; valid_symbols.len() + 1];
    for (idx, valid) in valid_symbols.iter().enumerate() {
        ts_valid[idx + 1] = *valid;
    }
    ts_valid
}

fn scan_once(
    _language: &TSLanguage,
    scan_fn: unsafe extern "C" fn(*mut c_void, *mut c_void, *const bool) -> bool,
    scanner_instance: *mut c_void,
    source: &[u8],
    pos: usize,
    valid_symbols: &[bool],
) -> Result<Option<(u16, usize)>, StreamingExternalScanError> {
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

    unsafe extern "C" fn advance(lex: *mut TsLexer, skip: bool) {
        unsafe {
            if lex.is_null() || (*lex).data.is_null() {
                return;
            }
            let backing = &mut *((*lex).data as *mut Backing);
            if backing.pos < backing.input.len() {
                backing.pos += 1;
                if !skip {
                    backing.mark = backing.pos;
                }
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
        input: &source[pos..],
        pos: 0,
        mark: 0,
    };
    let mut ts_lexer = TsLexer {
        lookahead,
        advance,
        mark_end,
        result_symbol: u16::MAX,
        data: &mut backing as *mut _ as *mut c_void,
    };

    // SAFETY: `scan_fn` is the generated external scanner. `valid_symbols` is a
    // live slice for the duration of the call. `scanner_instance` may be null when
    // the grammar does not allocate scanner state.
    let ok = unsafe {
        scan_fn(
            scanner_instance,
            &mut ts_lexer as *mut _ as *mut c_void,
            valid_symbols.as_ptr(),
        )
    };

    if !ok || ts_lexer.result_symbol == u16::MAX || ts_lexer.result_symbol == 0 {
        return Ok(None);
    }

    let end = if backing.mark > 0 {
        pos + backing.mark
    } else {
        pos + backing.pos
    };

    Ok(Some((ts_lexer.result_symbol, end)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pure_parser::{ExternalScanner, SyncPtr, TSParseAction, TSRule};
    use core::ptr;

    unsafe extern "C" fn stub_scan_fn(
        _scanner: *mut c_void,
        lexer: *mut c_void,
        valid_symbols: *const bool,
    ) -> bool {
        if lexer.is_null() || valid_symbols.is_null() {
            return false;
        }
        let lexer = lexer as *mut TsLexer;
        let valid = unsafe { *valid_symbols.add(1) };
        if !valid {
            return false;
        }
        let first = unsafe { ((*lexer).lookahead)(lexer) };
        if first == b'x' as u32 {
            unsafe {
                ((*lexer).advance)(lexer, false);
                (*lexer).result_symbol = 1;
                ((*lexer).mark_end)(lexer);
            }
            return true;
        }
        false
    }

    fn stub_language(
        scan: Option<unsafe extern "C" fn(*mut c_void, *mut c_void, *const bool) -> bool>,
    ) -> TSLanguage {
        static SYMBOL_MAP: [u16; 2] = [0, 42];
        TSLanguage {
            version: 15,
            symbol_count: 8,
            alias_count: 0,
            token_count: 4,
            external_token_count: 1,
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
            symbol_metadata: ptr::null(),
            public_symbol_map: ptr::null(),
            alias_map: ptr::null(),
            alias_sequences: ptr::null(),
            lex_modes: ptr::null(),
            lex_fn: None,
            keyword_lex_fn: None,
            keyword_capture_token: 0,
            external_scanner: ExternalScanner {
                states: ptr::null(),
                symbol_map: SYMBOL_MAP.as_ptr(),
                create: None,
                destroy: None,
                scan,
                serialize: None,
                deserialize: None,
            },
            primary_state_ids: ptr::null(),
            production_lhs_index: ptr::null(),
            production_count: 0,
            eof_symbol: 0,
            rules: ptr::null::<TSRule>(),
            rule_count: 0,
        }
    }

    #[test]
    fn external_scanner_adapter_honors_valid_symbol_mask() {
        let language = stub_language(Some(stub_scan_fn));
        let mut scanner = GeneratedExternalStreamingScanner::new(&language);
        let valid = [true];

        let token = scanner
            .scan_at(
                "x",
                0,
                &valid,
                LexMode {
                    lex_state: 0,
                    external_lex_state: 0b001,
                },
            )
            .expect("scan should succeed")
            .expect("token expected");

        assert_eq!(token.kind, 42);
        assert_eq!(token.start, 0);
        assert_eq!(token.end, 1);
    }

    #[test]
    fn external_scanner_adapter_rejects_invalid_symbol_emission() {
        let language = stub_language(Some(stub_scan_fn));
        let mut scanner = GeneratedExternalStreamingScanner::new(&language);
        let valid = [false];

        let err = scanner
            .scan_at(
                "x",
                0,
                &valid,
                LexMode {
                    lex_state: 0,
                    external_lex_state: 0,
                },
            )
            .expect_err("invalid mask should fail");

        assert_eq!(
            err,
            StreamingExternalScanError::NoValidSymbols { pos: 0 },
            "unexpected error: {err:?}"
        );
    }
}
