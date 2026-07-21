//! Tree-sitter FFI lexer wrapper for calling grammar's lex_fn
#![cfg_attr(feature = "strict_docs", allow(missing_docs))]

use crate::LexMode;
use std::ffi::{CStr, c_void};
use std::os::raw::c_char;
use std::ptr;

/// Tree-sitter lexer struct passed to lex function
#[repr(C)]
pub struct TSLexer {
    pub lookahead: i32,
    pub result_symbol: u16,
    pub advance: extern "C" fn(lexer: *mut TSLexer, is_skipped: bool),
    pub mark_end: extern "C" fn(lexer: *mut TSLexer),
    pub get_column: extern "C" fn(lexer: *mut TSLexer) -> u32,
    pub is_at_included_range_start: extern "C" fn(lexer: *const TSLexer) -> bool,
    pub eof: extern "C" fn(lexer: *const TSLexer) -> bool,
    pub log: extern "C" fn(lexer: *const TSLexer, message: *const c_char),
}

/// Function pointer type for lexer functions
pub type LexFn = unsafe extern "C" fn(lexer: *mut TSLexer, state: u16) -> bool;

/// Tree-sitter lex mode struct
#[repr(C)]
pub struct TSLexMode {
    pub lex_state: u16,
    pub external_lex_state: u16,
}

/// Tree-sitter language struct (partial - only fields we need)
#[repr(C)]
pub struct TSLanguage {
    pub version: u32,
    pub symbol_count: u32,
    pub alias_count: u32,
    pub token_count: u32,
    pub external_token_count: u32,
    pub state_count: u32,
    pub large_state_count: u32,
    pub production_id_count: u32,
    pub field_count: u32,
    pub max_alias_sequence_length: u16,
    // Skip parse table pointers we don't need
    _parse_table: *const u16,
    _small_parse_table: *const u16,
    _small_parse_table_map: *const u32,
    _parse_actions: *const c_void,
    _symbol_names: *const *const c_char,
    _field_names: *const *const c_char,
    _field_map_slices: *const c_void,
    _field_map_entries: *const c_void,
    _symbol_metadata: *const c_void,
    _public_symbol_map: *const u16,
    _alias_map: *const u16,
    _alias_sequences: *const u16,
    /// Lex modes for each state
    pub lex_modes: *const TSLexMode,
    /// Lex function
    pub lex_fn: Option<LexFn>,
    /// Keyword lex function
    pub keyword_lex_fn: Option<LexFn>,
    /// Capture function for keywords
    pub keyword_capture_token: u16,
    /// External scanner functions
    pub external_scanner: ExternalScanner,
}

/// External scanner functions
#[repr(C)]
pub struct ExternalScanner {
    pub states: *const bool,
    pub symbol_map: *const u16,
    pub create: Option<extern "C" fn() -> *mut c_void>,
    pub destroy: Option<extern "C" fn(scanner: *mut c_void)>,
    pub scan: Option<
        extern "C" fn(
            scanner: *mut c_void,
            lexer: *mut TSLexer,
            valid_symbols: *const bool,
        ) -> bool,
    >,
    pub serialize: Option<extern "C" fn(scanner: *mut c_void, buffer: *mut c_char) -> u32>,
    pub deserialize:
        Option<extern "C" fn(scanner: *mut c_void, buffer: *const c_char, length: u32)>,
}

/// Token produced by the lexer
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NextToken {
    pub kind: u32,
    pub start: u32,
    pub end: u32,
}

#[repr(C)]
struct TSLexerWrapper<'a> {
    lexer: TSLexer,
    host: TsLexerHost<'a>,
}

impl<'a> TSLexerWrapper<'a> {
    fn from_mut_lexer(lexer: *mut TSLexer) -> &'a mut Self {
        // SAFETY: `TSLexerWrapper` is `#[repr(C)]` with `lexer` as its first field,
        // so a pointer to `lexer` has the same address as the wrapper.
        unsafe { &mut *(lexer.cast::<Self>()) }
    }

    fn from_const_lexer(lexer: *const TSLexer) -> &'a Self {
        // SAFETY: Same layout guarantee as `from_mut_lexer`, but for shared access.
        unsafe { &*(lexer.cast::<Self>()) }
    }

    fn refresh_lookahead(&mut self) {
        self.lexer.lookahead = self
            .host
            .input
            .get(self.host.pos)
            .map_or(0, |byte| i32::from(*byte));
    }
}

/// Host struct for callbacks from the C lexer
pub struct TsLexerHost<'a> {
    input: &'a [u8],
    pos: usize,
    end_mark: usize,
}

impl<'a> TsLexerHost<'a> {
    #[must_use]
    fn current_column(&self) -> u32 {
        let mut line_start = 0usize;
        let mut i = 0usize;
        let limit = self.pos.min(self.input.len());
        while i < limit {
            match self.input[i] {
                b'\n' => {
                    line_start = i + 1;
                }
                b'\r' => {
                    if i + 1 < limit && self.input[i + 1] == b'\n' {
                        i += 1;
                    }
                    line_start = i + 1;
                }
                _ => {}
            }
            i += 1;
        }
        (limit - line_start) as u32
    }

    // C callbacks — invoked by the Tree-sitter lex_fn during `GrammarLexer::next()`.
    // SAFETY: Tree-sitter passes the same `TSLexer *` pointer that `GrammarLexer::next()`
    // provided to `lex_fn`. `TSLexerWrapper` is `#[repr(C)]` with `lexer` as the first field,
    // so casting that pointer back to the wrapper is valid for the duration of the call.
    extern "C" fn eof(lexer: *const TSLexer) -> bool {
        let wrapper = TSLexerWrapper::from_const_lexer(lexer);
        wrapper.host.pos >= wrapper.host.input.len()
    }

    extern "C" fn advance(lexer: *mut TSLexer, skip: bool) {
        let wrapper = TSLexerWrapper::from_mut_lexer(lexer);
        if wrapper.host.pos < wrapper.host.input.len() {
            wrapper.host.pos += 1;
            if !skip {
                wrapper.host.end_mark = wrapper.host.pos;
            }
        }
        wrapper.refresh_lookahead();
    }

    extern "C" fn mark_end(lexer: *mut TSLexer) {
        let wrapper = TSLexerWrapper::from_mut_lexer(lexer);
        wrapper.host.end_mark = wrapper.host.pos;
    }

    extern "C" fn get_column(lexer: *mut TSLexer) -> u32 {
        let wrapper = TSLexerWrapper::from_mut_lexer(lexer);
        wrapper.host.current_column()
    }

    extern "C" fn is_at_included_range_start(lexer: *const TSLexer) -> bool {
        let wrapper = TSLexerWrapper::from_const_lexer(lexer);
        // Included-range APIs are not implemented in this wrapper yet.
        // Model the whole input as a single included range starting at byte 0.
        wrapper.host.pos == 0
    }

    extern "C" fn log(_lexer: *const TSLexer, message: *const c_char) {
        if message.is_null() {
            return;
        }

        // Tree-sitter's `log` callback is variadic in C, but generated lexers typically
        // pass a plain C string literal. Ignore invalid UTF-8 to preserve behavior.
        let _ = unsafe { CStr::from_ptr(message) }.to_str();
    }
}

/// Grammar lexer that calls the compiled Tree-sitter lex function
pub struct GrammarLexer {
    lang: *const TSLanguage,
}

impl GrammarLexer {
    /// Create a lexer for a specific Tree-sitter language
    ///
    /// # Safety
    ///
    /// `lang` must be a valid, non-null pointer to a live [`TSLanguage`]
    /// from Tree-sitter. It must remain valid for the lifetime of the
    /// returned wrapper. Passing an invalid pointer or one that outlives
    /// the wrapper is undefined behavior.
    pub unsafe fn new(lang: *const TSLanguage) -> Self {
        Self { lang }
    }

    /// Get the next token from the input
    pub fn next(
        &self,
        input: &str,
        pos: usize,
        mode: LexMode,
        _valid_symbols: &[bool], // TODO: Use for external scanner
    ) -> Option<NextToken> {
        let mut wrapper = TSLexerWrapper {
            lexer: TSLexer {
                lookahead: 0,
                result_symbol: 0,
                advance: TsLexerHost::advance,
                mark_end: TsLexerHost::mark_end,
                get_column: TsLexerHost::get_column,
                is_at_included_range_start: TsLexerHost::is_at_included_range_start,
                eof: TsLexerHost::eof,
                log: TsLexerHost::log,
            },
            host: TsLexerHost {
                input: input.as_bytes(),
                pos,
                end_mark: pos,
            },
        };
        wrapper.refresh_lookahead();

        // SAFETY: `self.lang` was required to be a valid, non-null pointer to a live
        // TSLanguage by the safety contract of `GrammarLexer::new()`.
        let lex_fn = unsafe { (*self.lang).lex_fn }?;
        // SAFETY: `lex_fn` is a Tree-sitter-generated C function that expects a valid
        // TSLexer pointer; `c_lexer` is a stack-local struct with valid callbacks.
        let ok = unsafe { lex_fn(ptr::addr_of_mut!(wrapper.lexer), mode.lex_state) };

        if !ok || wrapper.lexer.result_symbol == 0 {
            return None;
        }

        Some(NextToken {
            kind: wrapper.lexer.result_symbol as u32,
            start: pos as u32,
            end: wrapper.host.end_mark as u32,
        })
    }
}

// Example of how to get a Tree-sitter language function
// This would be linked from the compiled grammar library
#[allow(dead_code)]
unsafe extern "C" {
    // Example: Link to tree-sitter-json
    // fn tree_sitter_json() -> *const TSLanguage;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::offset_of;

    #[test]
    fn test_ts_lexer_layout_matches_tree_sitter_abi() {
        assert_eq!(offset_of!(TSLexer, lookahead), 0);
        assert_eq!(offset_of!(TSLexer, result_symbol), 4);
        assert_eq!(offset_of!(TSLexer, advance), 8);
        assert_eq!(offset_of!(TSLexer, mark_end), 16);
        assert_eq!(offset_of!(TSLexer, get_column), 24);
        assert_eq!(offset_of!(TSLexer, is_at_included_range_start), 32);
        assert_eq!(offset_of!(TSLexer, eof), 40);
        assert_eq!(offset_of!(TSLexer, log), 48);
        assert_eq!(std::mem::size_of::<TSLexer>(), 56);
    }

    unsafe extern "C" fn test_lex_fn(lexer: *mut TSLexer, _state: u16) -> bool {
        let lexer = unsafe { &mut *lexer };
        if (lexer.eof)(lexer) {
            return false;
        }

        assert!(
            (lexer.is_at_included_range_start)(lexer),
            "should start at the whole-input included range"
        );
        assert_eq!((lexer.get_column)(lexer), 0, "stubbed get_column returns 0");
        let log_message = c"parser callback test".as_ptr();
        (lexer.log)(lexer, log_message);
        (lexer.mark_end)(lexer);
        (lexer.advance)(lexer, false);
        lexer.result_symbol = 7;
        true
    }

    #[test]
    fn test_grammar_lexer_uses_tree_sitter_callback_abi() {
        let lex_modes = [TSLexMode {
            lex_state: 0,
            external_lex_state: 0,
        }];
        let lang = TSLanguage {
            version: 0,
            symbol_count: 0,
            alias_count: 0,
            token_count: 0,
            external_token_count: 0,
            state_count: 0,
            large_state_count: 0,
            production_id_count: 0,
            field_count: 0,
            max_alias_sequence_length: 0,
            _parse_table: ptr::null(),
            _small_parse_table: ptr::null(),
            _small_parse_table_map: ptr::null(),
            _parse_actions: ptr::null(),
            _symbol_names: ptr::null(),
            _field_names: ptr::null(),
            _field_map_slices: ptr::null(),
            _field_map_entries: ptr::null(),
            _symbol_metadata: ptr::null(),
            _public_symbol_map: ptr::null(),
            _alias_map: ptr::null(),
            _alias_sequences: ptr::null(),
            lex_modes: lex_modes.as_ptr(),
            lex_fn: Some(test_lex_fn),
            keyword_lex_fn: None,
            keyword_capture_token: 0,
            external_scanner: ExternalScanner {
                states: ptr::null(),
                symbol_map: ptr::null(),
                create: None,
                destroy: None,
                scan: None,
                serialize: None,
                deserialize: None,
            },
        };

        let lexer = unsafe { GrammarLexer::new(&lang) };
        let token = lexer.next(
            "ab",
            0,
            LexMode {
                lex_state: 0,
                external_lex_state: 0,
            },
            &[],
        );

        assert!(token.is_some());
        let token = token.unwrap();
        assert_eq!(token.kind, 7);
        assert_eq!(token.start, 0);
        assert_eq!(token.end, 1);
    }

    #[test]
    #[ignore = "external scanner plumbing missing: GrammarLexer::next currently does not dispatch external scanners with valid_symbols"]
    fn contract_external_scanner_must_respect_valid_symbols() {
        // Contract marker test:
        // once GrammarLexer::next dispatches language.external_scanner.scan,
        // this test should assert that a scanner-emitted token is rejected
        // whenever its valid_symbols entry is false for the current state.
        panic!("not yet implemented");
    }

    #[test]
    fn get_column_tracks_newlines_and_crlf() {
        let mut wrapper = TSLexerWrapper {
            lexer: TSLexer {
                lookahead: 0,
                result_symbol: 0,
                advance: TsLexerHost::advance,
                mark_end: TsLexerHost::mark_end,
                get_column: TsLexerHost::get_column,
                is_at_included_range_start: TsLexerHost::is_at_included_range_start,
                eof: TsLexerHost::eof,
                log: TsLexerHost::log,
            },
            host: TsLexerHost {
                input: b"ab\ncd\r\nef",
                pos: 2,
                end_mark: 0,
            },
        };
        assert_eq!(TsLexerHost::get_column(&mut wrapper.lexer as *mut _), 2);
        wrapper.host.pos = 5;
        assert_eq!(TsLexerHost::get_column(&mut wrapper.lexer as *mut _), 2);
        wrapper.host.pos = 9;
        assert_eq!(TsLexerHost::get_column(&mut wrapper.lexer as *mut _), 2);
    }

    #[test]
    fn is_at_included_range_start_tracks_whole_input_start() {
        let mut wrapper = TSLexerWrapper {
            lexer: TSLexer {
                lookahead: 0,
                result_symbol: 0,
                advance: TsLexerHost::advance,
                mark_end: TsLexerHost::mark_end,
                get_column: TsLexerHost::get_column,
                is_at_included_range_start: TsLexerHost::is_at_included_range_start,
                eof: TsLexerHost::eof,
                log: TsLexerHost::log,
            },
            host: TsLexerHost {
                input: b"hello",
                pos: 0,
                end_mark: 0,
            },
        };
        assert!(TsLexerHost::is_at_included_range_start(
            &wrapper.lexer as *const _
        ));
        wrapper.host.pos = 3;
        assert!(!TsLexerHost::is_at_included_range_start(
            &wrapper.lexer as *const _
        ));
    }
}
