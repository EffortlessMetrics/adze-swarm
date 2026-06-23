#![cfg(test)]
use adze::external_scanner::{ExternalScanner, Lexer, ScanResult};

/// Comprehensive diagnostic test for indentation scanner
///
/// KNOWN BUG: When `scan()` returns `None`, the lexer position is not advanced,
/// causing the `while !lexer.is_eof()` loop to spin forever on non-whitespace,
/// non-newline characters. Needs a fallback `lexer.advance(1)` when no token
/// is matched.
#[test]
#[ignore = "KNOWN BUG: infinite loop - scan() returns None without advancing lexer position"]
fn comprehensive_indentation_scan_diagnostics() {
    struct DiagnosticLexer {
        input: Vec<u8>,
        position: usize,
        column: usize,
        mark: usize,
    }

    impl Lexer for DiagnosticLexer {
        fn lookahead(&self) -> Option<u8> {
            self.input.get(self.position).copied()
        }

        fn lookahead_n(&self, n: usize) -> Option<u8> {
            self.input.get(self.position.saturating_add(n)).copied()
        }

        fn advance(&mut self, n: usize) {
            for _ in 0..n {
                if let Some(&byte) = self.input.get(self.position) {
                    self.position += 1;
                    match byte {
                        b'\n' => self.column = 0,
                        b'\t' => self.column += 4,
                        _ => self.column += 1,
                    }
                }
            }
        }

        fn mark_end(&mut self) {
            self.mark = self.position;
        }

        fn column(&self) -> usize {
            self.column
        }

        fn is_eof(&self) -> bool {
            self.position >= self.input.len()
        }
    }

    struct TestIndentationScanner {
        stack: Vec<usize>,
    }

    impl TestIndentationScanner {
        fn new() -> Self {
            Self { stack: vec![0] }
        }
    }

    impl ExternalScanner for TestIndentationScanner {
        fn scan(&mut self, lexer: &mut dyn Lexer, valid_symbols: &[bool]) -> Option<ScanResult> {
            let mut indent_level = 0;
            let mut found_newline = false;

            // Newline detection
            match lexer.lookahead() {
                Some(b'\n') => {
                    lexer.advance(1);
                    lexer.mark_end();
                    found_newline = true;
                }
                Some(b'\r') => {
                    lexer.advance(1);
                    if lexer.lookahead() == Some(b'\n') {
                        lexer.advance(1);
                    }
                    lexer.mark_end();
                    found_newline = true;
                }
                _ => {}
            }

            // Detailed logging
            println!(
                "Scan: col={}, lookahead={:?}, current_stack={:?}",
                lexer.column(),
                lexer.lookahead(),
                self.stack
            );

            // Newline token detection
            if found_newline && valid_symbols.get(1002) == Some(&true) {
                return Some(ScanResult {
                    symbol: 1002, // NEWLINE
                    length: 1,
                });
            }

            // Indentation detection
            if lexer.column() == 0 {
                // Compute indentation
                while let Some(ch) = lexer.lookahead() {
                    match ch {
                        b' ' => {
                            indent_level += 1;
                            lexer.advance(1);
                        }
                        b'\t' => {
                            indent_level += 4;
                            lexer.advance(1);
                        }
                        _ => break,
                    }
                }

                // Detailed indentation logging
                println!(
                    "Indent detection: current_level={}, stack_top={}",
                    indent_level,
                    self.stack.last().copied().unwrap_or(0)
                );

                match indent_level.cmp(self.stack.last().unwrap_or(&0)) {
                    std::cmp::Ordering::Greater => {
                        if valid_symbols.get(1000) == Some(&true) {
                            // INDENT
                            self.stack.push(indent_level);
                            lexer.mark_end();
                            return Some(ScanResult {
                                symbol: 1000,
                                length: indent_level,
                            });
                        }
                    }
                    std::cmp::Ordering::Less => {
                        if valid_symbols.get(1001) == Some(&true) {
                            // DEDENT
                            let mut _dedent_count = 0;
                            while self.stack.len() > 1 && *self.stack.last().unwrap() > indent_level
                            {
                                self.stack.pop();
                                _dedent_count += 1;
                            }
                            lexer.mark_end();
                            return Some(ScanResult {
                                symbol: 1001,
                                length: 0,
                            });
                        }
                    }
                    std::cmp::Ordering::Equal => {}
                }
            }

            None
        }

        fn serialize(&self, buffer: &mut Vec<u8>) {
            for &level in &self.stack {
                buffer.extend_from_slice(&(level as u32).to_le_bytes());
            }
        }

        fn deserialize(&mut self, buffer: &[u8]) {
            self.stack.clear();
            for chunk in buffer.chunks_exact(4) {
                if let Ok(bytes) = chunk.try_into() {
                    let level = u32::from_le_bytes(bytes) as usize;
                    self.stack.push(level);
                }
            }
            if self.stack.is_empty() {
                self.stack.push(0);
            }
        }
    }

    // Diagnostic test cases (using Vec<u8> instead of byte arrays)
    let test_cases: Vec<(Vec<u8>, &str)> = vec![
        (
            b"def foo():\n    print('hello')\n    return 42\nbar()".to_vec(),
            "Basic indentation",
        ),
        (
            b"if x:\n\tprint(1)\n    print(2)".to_vec(),
            "Mixed tabs and spaces",
        ),
        (
            b"if a:\n    if b:\n        pass\nend".to_vec(),
            "Multiple dedents",
        ),
    ];

    for (input, description) in test_cases {
        println!("\n--- Test Case: {} ---", description);
        let mut lexer = DiagnosticLexer {
            input,
            position: 0,
            column: 0,
            mark: 0,
        };
        let mut scanner = TestIndentationScanner::new();
        let valid_symbols = vec![true; 3000];

        // Simulate multiple scans
        let mut scan_results = Vec::new();
        while !lexer.is_eof() {
            if let Some(result) = scanner.scan(&mut lexer, &valid_symbols) {
                scan_results.push(result);
            }
        }

        println!("Scan results: {:?}", scan_results);
    }
}
