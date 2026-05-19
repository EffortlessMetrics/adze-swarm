#![allow(dead_code, unused_imports)]

use super::*;

#[test]
fn test_string_scanner_start_token() {
    let mut scanner = StringScanner::new();
    let valid = vec![true, true, true];

    struct TestLexer<'a> {
        input: &'a [u8],
        position: usize,
    }

    impl<'a> Lexer for TestLexer<'a> {
        fn advance(&mut self, n: usize) {
            self.position = (self.position + n).min(self.input.len());
        }

        fn lookahead(&self) -> Option<u8> {
            self.input.get(self.position).copied()
        }

        fn mark_end(&mut self) {}

        fn column(&self) -> usize {
            self.position
        }

        fn is_eof(&self) -> bool {
            self.position >= self.input.len()
        }
    }

    let mut lexer = TestLexer {
        input: b"\"hello\"",
        position: 0,
    };

    let result = scanner.scan(&mut lexer, &valid);
    assert_eq!(
        result,
        Some(ScanResult {
            symbol: 0,
            length: 1
        })
    );
}
