#![cfg(test)]

use adze::external_scanner::{ExternalScanner, ExternalScannerRuntime, Lexer, ScanResult};
use std::collections::HashSet;

#[derive(Default)]
struct IgnoringScanner;

impl ExternalScanner for IgnoringScanner {
    fn scan(&mut self, _lexer: &mut dyn Lexer, _valid_symbols: &[bool]) -> Option<ScanResult> {
        Some(ScanResult {
            symbol: 1,
            length: 1,
        })
    }

    fn serialize(&self, _buffer: &mut Vec<u8>) {}

    fn deserialize(&mut self, _buffer: &[u8]) {}
}

struct TestLexer;

impl Lexer for TestLexer {
    fn lookahead(&self) -> Option<u8> {
        Some(b'x')
    }

    fn lookahead_n(&self, _n: usize) -> Option<u8> {
        Some(b'x')
    }

    fn advance(&mut self, _n: usize) {}

    fn mark_end(&mut self) {}

    fn column(&self) -> usize {
        0
    }

    fn is_eof(&self) -> bool {
        false
    }
}

#[test]
fn scanner_runtime_filters_tokens_not_in_valid_symbols() {
    let mut runtime = ExternalScannerRuntime::new(vec![10, 11]);
    let mut scanner = IgnoringScanner;
    let mut lexer = TestLexer;

    let valid_external_tokens = HashSet::from([10]);
    let result = runtime.scan(&mut scanner, &mut lexer, &valid_external_tokens);

    assert!(result.is_none());
}

#[test]
fn scanner_runtime_allows_tokens_that_are_valid_in_state() {
    #[derive(Default)]
    struct RespectingScanner;

    impl ExternalScanner for RespectingScanner {
        fn scan(&mut self, _lexer: &mut dyn Lexer, valid_symbols: &[bool]) -> Option<ScanResult> {
            if valid_symbols.get(1).copied().unwrap_or(false) {
                return Some(ScanResult {
                    symbol: 1,
                    length: 2,
                });
            }
            None
        }

        fn serialize(&self, _buffer: &mut Vec<u8>) {}

        fn deserialize(&mut self, _buffer: &[u8]) {}
    }

    let mut runtime = ExternalScannerRuntime::new(vec![10, 11]);
    let mut scanner = RespectingScanner;
    let mut lexer = TestLexer;
    let valid_external_tokens = HashSet::from([11]);

    let result = runtime.scan(&mut scanner, &mut lexer, &valid_external_tokens);

    assert_eq!(result, Some((1, 2)));
}
