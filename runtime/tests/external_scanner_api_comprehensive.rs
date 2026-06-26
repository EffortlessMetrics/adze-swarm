#![allow(
    clippy::needless_range_loop,
    reason = "property and comprehensive tests use index-based loops to exercise table positions and boundary cases"
)]
//! Comprehensive tests for the external scanner API surface.
//!
//! Covers: ScanResult, ExternalScannerState, Lexer trait, ExternalScanner trait,
//! ExternalScannerRuntime, StringScanner, CommentScanner, and serialization round-trips.

use adze::external_scanner::{
    CommentScanner, ExternalScanner, ExternalScannerRuntime, ExternalScannerState, Lexer,
    ScanResult, StringScanner,
};
use std::collections::HashSet;

// ---------------------------------------------------------------------------
// Helper: a simple in-memory Lexer usable by any test
// ---------------------------------------------------------------------------

struct TestLexer {
    data: Vec<u8>,
    pos: usize,
    mark: usize,
}

impl TestLexer {
    fn new(input: &[u8]) -> Self {
        Self {
            data: input.to_vec(),
            pos: 0,
            mark: 0,
        }
    }

    fn at(input: &[u8], position: usize) -> Self {
        Self {
            data: input.to_vec(),
            pos: position,
            mark: position,
        }
    }
}

impl Lexer for TestLexer {
    fn lookahead(&self) -> Option<u8> {
        self.data.get(self.pos).copied()
    }

    fn lookahead_n(&self, n: usize) -> Option<u8> {
        self.data.get(self.pos.saturating_add(n)).copied()
    }

    fn advance(&mut self, n: usize) {
        self.pos = self.pos.saturating_add(n).min(self.data.len());
    }

    fn mark_end(&mut self) {
        self.mark = self.pos;
    }

    fn column(&self) -> usize {
        self.pos
    }

    fn is_eof(&self) -> bool {
        self.pos >= self.data.len()
    }
}

// ---------------------------------------------------------------------------
// Helpers for setting private scanner state via serialize/deserialize
// ---------------------------------------------------------------------------

/// Put a StringScanner into "inside string" state using its serialization format.
/// Format: [in_string: u8, quote_char: u8]
fn set_string_scanner_in_string(scanner: &mut StringScanner, quote: u8) {
    scanner.deserialize(&[1, quote]);
}

/// Check whether a StringScanner is in "inside string" state by serializing it.
fn string_scanner_is_in_string(scanner: &StringScanner) -> (bool, Option<u8>) {
    let mut buf = Vec::new();
    scanner.serialize(&mut buf);
    let in_string = buf.first().copied().unwrap_or(0) != 0;
    let quote = buf.get(1).copied().filter(|&b| b != 0);
    (in_string, quote)
}

/// Set a CommentScanner's depth via its serialization format (u32 LE).
fn set_comment_depth(scanner: &mut CommentScanner, depth: u32) {
    scanner.deserialize(&depth.to_le_bytes());
}

/// Read a CommentScanner's depth by serializing.
fn comment_depth(scanner: &CommentScanner) -> u32 {
    let mut buf = Vec::new();
    scanner.serialize(&mut buf);
    if buf.len() >= 4 {
        u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]])
    } else {
        0
    }
}

// ===========================================================================
// ScanResult
// ===========================================================================

#[test]
fn scan_result_equality_and_inequality() {
    let a = ScanResult {
        symbol: 1,
        length: 5,
    };
    let b = ScanResult {
        symbol: 1,
        length: 5,
    };
    assert_eq!(a, b);

    // Different symbol
    let c = ScanResult {
        symbol: 0,
        length: 5,
    };
    assert_ne!(a, c);

    // Different length
    let d = ScanResult {
        symbol: 1,
        length: 3,
    };
    assert_ne!(a, d);
}

#[test]
fn scan_result_clone_and_debug() {
    let a = ScanResult {
        symbol: 42,
        length: 10,
    };
    let b = a.clone();
    assert_eq!(a, b);

    let dbg = format!("{a:?}");
    assert!(dbg.contains("42"));
    assert!(dbg.contains("10"));
}

// ===========================================================================
// ExternalScannerState
// ===========================================================================

#[test]
fn state_new_and_default_are_empty() {
    let state = ExternalScannerState::new();
    assert!(state.data.is_empty());
    let state_d = ExternalScannerState::default();
    assert!(state_d.serialize().is_empty());
}

#[test]
fn state_serialize_deserialize_roundtrip() {
    let mut state = ExternalScannerState::new();
    state.data = vec![10, 20, 30];

    let bytes = state.serialize();
    let restored = ExternalScannerState::deserialize(bytes);
    assert_eq!(restored.data, vec![10, 20, 30]);

    // Empty roundtrip
    let restored_empty = ExternalScannerState::deserialize(&[]);
    assert!(restored_empty.data.is_empty());
}

// ===========================================================================
// TestLexer basics (exercises the Lexer trait)
// ===========================================================================

#[test]
fn lexer_lookahead_advance_and_eof() {
    let lexer = TestLexer::new(b"abc");
    assert_eq!(lexer.lookahead(), Some(b'a'));
    assert!(!lexer.is_eof());

    let mut lexer = TestLexer::new(b"abc");
    lexer.advance(2);
    assert_eq!(lexer.lookahead(), Some(b'c'));

    // EOF on empty input
    let lexer = TestLexer::new(b"");
    assert!(lexer.is_eof());
    assert_eq!(lexer.lookahead(), None);

    // Advance past end
    let mut lexer = TestLexer::new(b"ab");
    lexer.advance(100);
    assert!(lexer.is_eof());
    assert_eq!(lexer.lookahead(), None);
}

#[test]
fn lexer_column_and_mark() {
    let mut lexer = TestLexer::new(b"hello");
    assert_eq!(lexer.column(), 0);
    lexer.advance(3);
    assert_eq!(lexer.column(), 3);

    lexer.mark_end();
    assert_eq!(lexer.mark, 3);
    lexer.advance(2);
    // mark stays at 3
    assert_eq!(lexer.mark, 3);
}

#[test]
fn lexer_at_specific_position() {
    let lexer = TestLexer::at(b"hello", 3);
    assert_eq!(lexer.lookahead(), Some(b'l'));
    assert_eq!(lexer.column(), 3);
    assert!(!lexer.is_eof());
}

// ===========================================================================
// StringScanner
// ===========================================================================

#[test]
fn string_scanner_double_quote_start() {
    let mut scanner = StringScanner::new();
    let mut lexer = TestLexer::new(b"\"hello\"");
    let valid = vec![true, true, true];

    let result = scanner.scan(&mut lexer, &valid);
    assert_eq!(
        result,
        Some(ScanResult {
            symbol: 0,
            length: 1,
        })
    );
}

#[test]
fn string_scanner_single_quote_start() {
    let mut scanner = StringScanner::new();
    let mut lexer = TestLexer::new(b"'hello'");
    let valid = vec![true, true, true];

    let result = scanner.scan(&mut lexer, &valid);
    assert_eq!(
        result,
        Some(ScanResult {
            symbol: 0,
            length: 1,
        })
    );
}

#[test]
fn string_scanner_content_then_end() {
    let mut scanner = StringScanner::new();
    // Put scanner into "inside double-quoted string" state via deserialize
    set_string_scanner_in_string(&mut scanner, b'"');

    // Scan content
    let mut lexer = TestLexer::new(b"abc\"");
    let valid = vec![false, true, false]; // only STRING_CONTENT valid
    let result = scanner.scan(&mut lexer, &valid);
    assert_eq!(
        result,
        Some(ScanResult {
            symbol: 1,
            length: 3,
        })
    );

    // Now scan end
    let mut lexer = TestLexer::new(b"\"");
    let valid = vec![false, false, true]; // only STRING_END valid
    let result = scanner.scan(&mut lexer, &valid);
    assert_eq!(
        result,
        Some(ScanResult {
            symbol: 2,
            length: 1,
        })
    );
    // Scanner should have left the string state
    let (in_string, _) = string_scanner_is_in_string(&scanner);
    assert!(!in_string);
}

#[test]
fn string_scanner_no_match_on_non_quote() {
    let mut scanner = StringScanner::new();
    let mut lexer = TestLexer::new(b"hello");
    let valid = vec![true, true, true];

    let result = scanner.scan(&mut lexer, &valid);
    assert_eq!(result, None);
}

#[test]
fn string_scanner_eof_returns_none() {
    let mut scanner = StringScanner::new();
    let mut lexer = TestLexer::new(b"");
    let valid = vec![true, true, true];
    assert_eq!(scanner.scan(&mut lexer, &valid), None);
}

#[test]
fn string_scanner_escape_inside_string() {
    let mut scanner = StringScanner::new();
    set_string_scanner_in_string(&mut scanner, b'"');

    let mut lexer = TestLexer::new(b"he\\\"llo");
    let valid = vec![false, true, false];
    let result = scanner.scan(&mut lexer, &valid);
    // Should consume all content including escaped quote
    assert!(result.is_some());
    assert_eq!(result.unwrap().symbol, 1); // STRING_CONTENT
}

#[test]
fn string_scanner_start_not_valid_returns_none() {
    let mut scanner = StringScanner::new();
    let mut lexer = TestLexer::new(b"\"hello\"");
    // STRING_START not valid
    let valid = vec![false, true, true];
    assert_eq!(scanner.scan(&mut lexer, &valid), None);
}

#[test]
fn string_scanner_serialize_roundtrip() {
    let mut scanner = StringScanner::new();
    set_string_scanner_in_string(&mut scanner, b'\'');

    let mut buf = Vec::new();
    scanner.serialize(&mut buf);

    let mut restored = StringScanner::new();
    restored.deserialize(&buf);

    let (in_string, quote) = string_scanner_is_in_string(&restored);
    assert!(in_string);
    assert_eq!(quote, Some(b'\''));
}

#[test]
fn string_scanner_deserialize_short_buffer() {
    let mut scanner = StringScanner::new();
    set_string_scanner_in_string(&mut scanner, b'"');
    // Buffer too short: should not modify state (the deserialize guards on len >= 2)
    scanner.deserialize(&[]);
    // State unchanged because buffer was too short
    let (in_string, _) = string_scanner_is_in_string(&scanner);
    assert!(in_string);
}

#[test]
fn string_scanner_full_double_quote_sequence() {
    // Drive the scanner through start -> content -> end for a complete string
    let mut scanner = StringScanner::new();
    let all_valid = vec![true, true, true];

    // 1. STRING_START on "
    let mut lexer = TestLexer::new(b"\"abc\"");
    let r = scanner.scan(&mut lexer, &all_valid).unwrap();
    assert_eq!(r.symbol, 0);

    // 2. STRING_CONTENT
    let mut lexer = TestLexer::at(b"\"abc\"", 1);
    let r = scanner.scan(&mut lexer, &all_valid).unwrap();
    assert_eq!(r.symbol, 1);
    assert_eq!(r.length, 3);

    // 3. STRING_END
    let mut lexer = TestLexer::at(b"\"abc\"", 4);
    let r = scanner.scan(&mut lexer, &all_valid).unwrap();
    assert_eq!(r.symbol, 2);
}

// ===========================================================================
// CommentScanner
// ===========================================================================

#[test]
fn comment_scanner_start() {
    let mut scanner = CommentScanner::new();
    let mut lexer = TestLexer::new(b"/* comment */");
    let valid = vec![true, true, true];

    let result = scanner.scan(&mut lexer, &valid);
    assert_eq!(
        result,
        Some(ScanResult {
            symbol: 0,
            length: 2,
        })
    );
    assert_eq!(comment_depth(&scanner), 1);
}

#[test]
fn comment_scanner_eof_returns_none() {
    let mut scanner = CommentScanner::new();
    let mut lexer = TestLexer::new(b"");
    let valid = vec![true, true, true];
    assert_eq!(scanner.scan(&mut lexer, &valid), None);
}

#[test]
fn comment_scanner_no_match_on_plain_text() {
    let mut scanner = CommentScanner::new();
    let mut lexer = TestLexer::new(b"hello");
    let valid = vec![true, true, true];
    assert_eq!(scanner.scan(&mut lexer, &valid), None);
}

#[test]
fn comment_scanner_serialize_roundtrip() {
    let mut scanner = CommentScanner::new();
    set_comment_depth(&mut scanner, 3);

    let mut buf = Vec::new();
    scanner.serialize(&mut buf);

    let mut restored = CommentScanner::new();
    restored.deserialize(&buf);

    assert_eq!(comment_depth(&restored), 3);
}

#[test]
fn comment_scanner_deserialize_short_buffer() {
    let mut scanner = CommentScanner::new();
    set_comment_depth(&mut scanner, 5);
    // Buffer too short: should not change depth
    scanner.deserialize(&[]);
    assert_eq!(comment_depth(&scanner), 5);
}

#[test]
fn comment_scanner_no_match_single_slash() {
    let mut scanner = CommentScanner::new();
    let mut lexer = TestLexer::new(b"/x");
    let valid = vec![true, true, true];
    assert_eq!(scanner.scan(&mut lexer, &valid), None);
}

// ===========================================================================
// ExternalScannerRuntime
// ===========================================================================

/// Minimal scanner that always returns a fixed result.
struct FixedScanner {
    result: Option<ScanResult>,
}

impl ExternalScanner for FixedScanner {
    fn scan(&mut self, _lexer: &mut dyn Lexer, _valid_symbols: &[bool]) -> Option<ScanResult> {
        self.result.clone()
    }
    fn serialize(&self, buffer: &mut Vec<u8>) {
        buffer.push(0xAA);
    }
    fn deserialize(&mut self, _buffer: &[u8]) {}
}

#[test]
fn runtime_new_and_get_tokens() {
    let runtime = ExternalScannerRuntime::new(vec![1, 2, 3]);
    assert_eq!(runtime.get_external_tokens(), &[1, 2, 3]);

    let empty = ExternalScannerRuntime::new(vec![]);
    assert!(empty.get_external_tokens().is_empty());
}

#[test]
fn runtime_reset_clears_state() {
    let mut runtime = ExternalScannerRuntime::new(vec![1]);
    let mut scanner = FixedScanner {
        result: Some(ScanResult {
            symbol: 0,
            length: 1,
        }),
    };
    let mut lexer = TestLexer::new(b"x");
    let mut valid = HashSet::new();
    valid.insert(1u16);
    let _ = runtime.scan(&mut scanner, &mut lexer, &valid);
    runtime.reset();
    // After reset a fresh scan still works
    let result = runtime.scan(&mut scanner, &mut lexer, &valid);
    assert!(result.is_some());
}

#[test]
fn runtime_scan_returns_symbol_and_length() {
    let mut runtime = ExternalScannerRuntime::new(vec![10]);
    let mut scanner = FixedScanner {
        result: Some(ScanResult {
            symbol: 0,
            length: 5,
        }),
    };
    let mut lexer = TestLexer::new(b"hello");
    let mut valid = HashSet::new();
    valid.insert(10u16);

    let result = runtime.scan(&mut scanner, &mut lexer, &valid);
    assert_eq!(result, Some((0, 5)));
}

#[test]
fn runtime_scan_returns_none_when_scanner_returns_none() {
    let mut runtime = ExternalScannerRuntime::new(vec![10]);
    let mut scanner = FixedScanner { result: None };
    let mut lexer = TestLexer::new(b"hello");
    let valid = HashSet::new();

    let result = runtime.scan(&mut scanner, &mut lexer, &valid);
    assert_eq!(result, None);
}

#[test]
fn runtime_scan_rejects_zero_length_results() {
    let mut runtime = ExternalScannerRuntime::new(vec![10]);
    let mut scanner = FixedScanner {
        result: Some(ScanResult {
            symbol: 0,
            length: 0,
        }),
    };
    let mut lexer = TestLexer::new(b"hello");
    let mut valid = HashSet::new();
    valid.insert(10u16);

    let result = runtime.scan(&mut scanner, &mut lexer, &valid);
    assert_eq!(result, None);
}

#[test]
fn runtime_scan_builds_valid_symbols_from_tokens() {
    struct CheckingScanner {
        observed_valid: Vec<bool>,
    }
    impl ExternalScanner for CheckingScanner {
        fn scan(&mut self, _lexer: &mut dyn Lexer, valid_symbols: &[bool]) -> Option<ScanResult> {
            self.observed_valid = valid_symbols.to_vec();
            None
        }
        fn serialize(&self, _buffer: &mut Vec<u8>) {}
        fn deserialize(&mut self, _buffer: &[u8]) {}
    }

    let mut runtime = ExternalScannerRuntime::new(vec![5, 10, 15]);
    let mut scanner = CheckingScanner {
        observed_valid: vec![],
    };
    let mut lexer = TestLexer::new(b"x");
    let mut valid = HashSet::new();
    valid.insert(10u16);

    runtime.scan(&mut scanner, &mut lexer, &valid);
    assert_eq!(scanner.observed_valid, vec![false, true, false]);
}

#[test]
fn runtime_persists_state_across_scans() {
    struct CountingScanner {
        counter: u8,
    }
    impl ExternalScanner for CountingScanner {
        fn scan(&mut self, _lexer: &mut dyn Lexer, _valid_symbols: &[bool]) -> Option<ScanResult> {
            self.counter += 1;
            Some(ScanResult {
                symbol: 0,
                length: 1,
            })
        }
        fn serialize(&self, buffer: &mut Vec<u8>) {
            buffer.push(self.counter);
        }
        fn deserialize(&mut self, buffer: &[u8]) {
            if let Some(&b) = buffer.first() {
                self.counter = b;
            }
        }
    }

    let mut runtime = ExternalScannerRuntime::new(vec![1]);
    let mut scanner = CountingScanner { counter: 0 };
    let mut lexer = TestLexer::new(b"abc");
    let mut valid = HashSet::new();
    valid.insert(1u16);

    // First scan: counter 0 -> 1, serialized as [1]
    runtime.scan(&mut scanner, &mut lexer, &valid);
    // Second scan: deserialize restores 1, scan bumps to 2
    runtime.scan(&mut scanner, &mut lexer, &valid);
    assert_eq!(scanner.counter, 2);
}

#[test]
fn runtime_all_tokens_valid() {
    struct AllValidChecker {
        all_true: bool,
    }
    impl ExternalScanner for AllValidChecker {
        fn scan(&mut self, _lexer: &mut dyn Lexer, valid_symbols: &[bool]) -> Option<ScanResult> {
            self.all_true = valid_symbols.iter().all(|&v| v);
            None
        }
        fn serialize(&self, _buffer: &mut Vec<u8>) {}
        fn deserialize(&mut self, _buffer: &[u8]) {}
    }

    let mut runtime = ExternalScannerRuntime::new(vec![1, 2, 3]);
    let mut scanner = AllValidChecker { all_true: false };
    let mut lexer = TestLexer::new(b"x");
    let valid: HashSet<u16> = [1, 2, 3].into_iter().collect();

    runtime.scan(&mut scanner, &mut lexer, &valid);
    assert!(scanner.all_true);
}

// ===========================================================================
// Object-safety: ExternalScanner as dyn trait
// ===========================================================================

#[test]
fn external_scanner_is_object_safe() {
    let scanner: Box<dyn ExternalScanner> = Box::new(StringScanner::new());
    let _ = scanner;
}

// ===========================================================================
// Edge cases
// ===========================================================================

#[test]
fn scanners_default_matches_new() {
    // StringScanner
    let a = StringScanner::new();
    let b = StringScanner::default();
    let (a_in, a_q) = string_scanner_is_in_string(&a);
    let (b_in, b_q) = string_scanner_is_in_string(&b);
    assert!(!a_in);
    assert!(!b_in);
    assert_eq!(a_q, None);
    assert_eq!(b_q, None);

    // CommentScanner
    let c = CommentScanner::new();
    let d = CommentScanner::default();
    assert_eq!(comment_depth(&c), 0);
    assert_eq!(comment_depth(&d), 0);
}
