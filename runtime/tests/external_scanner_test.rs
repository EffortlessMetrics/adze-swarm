//! External Scanner Tests
//!
//! Tests for external scanner integration including Python-style indentation,
//! nested comments, and stateful scanning.

#![cfg(test)]

use adze::external_scanner::{ExternalScanner, Lexer, ScanResult};

/// Python-style indentation scanner
#[derive(Debug, Default)]
struct IndentationScanner {
    indent_stack: Vec<u32>,
}

impl ExternalScanner for IndentationScanner {
    fn scan(&mut self, lexer: &mut dyn Lexer, valid_symbols: &[bool]) -> Option<ScanResult> {
        const INDENT: usize = 0;
        const DEDENT: usize = 1;
        const NEWLINE: usize = 2;

        // Skip whitespace except newlines
        while lexer.lookahead() == Some(b' ') || lexer.lookahead() == Some(b'\t') {
            lexer.advance(1);
        }

        if lexer.lookahead() == Some(b'\n') {
            if valid_symbols[NEWLINE] {
                lexer.advance(1);
                lexer.mark_end();
                return Some(ScanResult {
                    symbol: NEWLINE as u16,
                    length: 1,
                });
            }

            lexer.advance(1);

            // Count indentation
            let mut indent = 0;
            while lexer.lookahead() == Some(b' ') {
                indent += 1;
                lexer.advance(1);
            }

            let current_indent = *self.indent_stack.last().unwrap_or(&0);

            if indent > current_indent && valid_symbols[INDENT] {
                self.indent_stack.push(indent);
                lexer.mark_end();
                return Some(ScanResult {
                    symbol: INDENT as u16,
                    length: (indent - current_indent) as usize,
                });
            }

            if indent < current_indent && valid_symbols[DEDENT] {
                while let Some(&level) = self.indent_stack.last() {
                    if level <= indent {
                        break;
                    }
                    self.indent_stack.pop();
                }
                lexer.mark_end();
                return Some(ScanResult {
                    symbol: DEDENT as u16,
                    length: (current_indent - indent) as usize,
                });
            }
        }

        None
    }

    fn serialize(&self, buffer: &mut Vec<u8>) {
        for &indent in &self.indent_stack {
            buffer.extend_from_slice(&indent.to_le_bytes());
        }
    }

    fn deserialize(&mut self, buffer: &[u8]) {
        self.indent_stack.clear();
        let mut consumed = 0;

        while consumed + 4 <= buffer.len() {
            let bytes = [
                buffer[consumed],
                buffer[consumed + 1],
                buffer[consumed + 2],
                buffer[consumed + 3],
            ];
            self.indent_stack.push(u32::from_le_bytes(bytes));
            consumed += 4;
        }
    }
}

/// Nested comment scanner (OCaml-style)
#[derive(Debug, Default)]
struct NestedCommentScanner {
    depth: u32,
}

impl ExternalScanner for NestedCommentScanner {
    fn scan(&mut self, lexer: &mut dyn Lexer, valid_symbols: &[bool]) -> Option<ScanResult> {
        const COMMENT: usize = 0;

        if !valid_symbols[COMMENT] {
            return None;
        }

        // Look for (* to start
        if self.depth == 0 && lexer.lookahead() == Some(b'(') {
            lexer.advance(1);
            if lexer.lookahead() == Some(b'*') {
                lexer.advance(1);
                self.depth = 1;
            }
        }

        let mut length = 0;
        // Scan until we find matching *)
        while self.depth > 0 {
            match lexer.lookahead() {
                Some(b'(') => {
                    lexer.advance(1);
                    length += 1;
                    if lexer.lookahead() == Some(b'*') {
                        lexer.advance(1);
                        length += 1;
                        self.depth += 1;
                    }
                }
                Some(b'*') => {
                    lexer.advance(1);
                    length += 1;
                    if lexer.lookahead() == Some(b')') {
                        lexer.advance(1);
                        length += 1;
                        self.depth -= 1;
                        if self.depth == 0 {
                            lexer.mark_end();
                            return Some(ScanResult {
                                symbol: COMMENT as u16,
                                length,
                            });
                        }
                    }
                }
                Some(_) => {
                    lexer.advance(1);
                    length += 1;
                }
                None => return None, // EOF
            }
        }

        None
    }

    fn serialize(&self, buffer: &mut Vec<u8>) {
        buffer.extend_from_slice(&self.depth.to_le_bytes());
    }

    fn deserialize(&mut self, buffer: &[u8]) {
        if buffer.len() >= 4 {
            self.depth = u32::from_le_bytes([buffer[0], buffer[1], buffer[2], buffer[3]]);
        }
    }
}

#[test]
fn test_indentation_scanner() {
    let mut scanner = IndentationScanner::default();

    // Create a mock lexer for testing
    struct MockLexer {
        input: Vec<u8>,
        position: usize,
        marked_end: usize,
    }

    impl Lexer for MockLexer {
        fn lookahead(&self) -> Option<u8> {
            self.input.get(self.position).copied()
        }

        fn lookahead_n(&self, n: usize) -> Option<u8> {
            self.input.get(self.position.saturating_add(n)).copied()
        }

        fn advance(&mut self, n: usize) {
            self.position = (self.position + n).min(self.input.len());
        }

        fn mark_end(&mut self) {
            self.marked_end = self.position;
        }

        fn column(&self) -> usize {
            // Simplified: just return position
            self.position
        }

        fn is_eof(&self) -> bool {
            self.position >= self.input.len()
        }
    }

    // Test newline detection
    let mut lexer = MockLexer {
        input: b"\n    ".to_vec(),
        position: 0,
        marked_end: 0,
    };

    let valid_symbols = vec![false, false, true]; // Only NEWLINE is valid
    let result = scanner.scan(&mut lexer, &valid_symbols);
    assert!(result.is_some());

    if let Some(scan_result) = result {
        assert_eq!(scan_result.symbol, 2); // NEWLINE
        assert_eq!(scan_result.length, 1);
    }
}

#[test]
fn test_nested_comment_scanner() {
    let mut scanner = NestedCommentScanner::default();

    struct MockLexer {
        input: Vec<u8>,
        position: usize,
        marked_end: usize,
    }

    impl Lexer for MockLexer {
        fn lookahead(&self) -> Option<u8> {
            self.input.get(self.position).copied()
        }

        fn lookahead_n(&self, n: usize) -> Option<u8> {
            self.input.get(self.position.saturating_add(n)).copied()
        }

        fn advance(&mut self, n: usize) {
            self.position = (self.position + n).min(self.input.len());
        }

        fn mark_end(&mut self) {
            self.marked_end = self.position;
        }

        fn column(&self) -> usize {
            self.position
        }

        fn is_eof(&self) -> bool {
            self.position >= self.input.len()
        }
    }

    // Test simple comment
    let mut lexer = MockLexer {
        input: b"(* comment *)".to_vec(),
        position: 0,
        marked_end: 0,
    };

    let valid_symbols = vec![true]; // COMMENT is valid
    let result = scanner.scan(&mut lexer, &valid_symbols);
    assert!(result.is_some());

    if let Some(scan_result) = result {
        assert_eq!(scan_result.symbol, 0); // COMMENT
        assert_eq!(lexer.marked_end, 13); // Should have consumed entire comment
    }
}

#[test]
#[cfg(feature = "external_scanners")]
fn test_scanner_state_persistence() {
    let scanner = IndentationScanner {
        indent_stack: vec![0, 4, 8],
    };

    // Serialize state
    let mut buffer = Vec::new();
    scanner.serialize(&mut buffer);

    // Create new scanner and deserialize
    let mut new_scanner = IndentationScanner::default();
    new_scanner.deserialize(&buffer);

    assert_eq!(new_scanner.indent_stack, vec![0, 4, 8]);
}
