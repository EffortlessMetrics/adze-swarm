// Test for Python-style indentation external scanner
#![cfg(all(test, feature = "external_scanners"))]

use adze::external_scanner::{ExternalScanner, Lexer, ScanResult};

/// Token types for indentation
const INDENT: u16 = 1000;
const DEDENT: u16 = 1001;
const NEWLINE: u16 = 1002;

/// Python-style indentation scanner
struct IndentationScanner {
    indent_stack: std::sync::Mutex<Vec<usize>>,
}

impl IndentationScanner {
    fn new() -> Self {
        Self {
            indent_stack: std::sync::Mutex::new(vec![0]), // Start with indent level 0
        }
    }
}

impl ExternalScanner for IndentationScanner {
    fn scan(&mut self, lexer: &mut dyn Lexer, valid_symbols: &[bool]) -> Option<ScanResult> {
        // Precise indentation tracking with comprehensive symbol detection
        let mut indent_level = 0;
        let mut found_newline = false;

        // Controlled newline handling
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

        // Selective newline token detection
        if found_newline && valid_symbols.get(NEWLINE as usize) == Some(&true) {
            return Some(ScanResult {
                symbol: NEWLINE,
                length: 1,
            });
        }

        // Granular indentation detection
        if lexer.column() == 0 {
            // Whitespace computation with precision
            while let Some(ch) = lexer.lookahead() {
                match ch {
                    b' ' => {
                        indent_level += 1;
                        lexer.advance(1);
                    }
                    b'\t' => {
                        indent_level += 4; // Consistent tab handling
                        lexer.advance(1);
                    }
                    _ => break, // Non-whitespace stops tracking
                }
            }

            // Synchronized indent stack management
            let mut stack = self.indent_stack.lock().unwrap();
            let current_indent = *stack.last().unwrap_or(&0);

            // Smart indent/dedent logic
            match indent_level.cmp(&current_indent) {
                std::cmp::Ordering::Greater => {
                    if valid_symbols.get(INDENT as usize) == Some(&true) {
                        stack.push(indent_level);
                        lexer.mark_end();
                        return Some(ScanResult {
                            symbol: INDENT,
                            length: indent_level,
                        });
                    }
                }
                std::cmp::Ordering::Less => {
                    if valid_symbols.get(DEDENT as usize) == Some(&true) {
                        if stack.len() > 1 && *stack.last().unwrap() > indent_level {
                            stack.pop();
                        }
                        lexer.mark_end();
                        return Some(ScanResult {
                            symbol: DEDENT,
                            length: 0, // Strict zero-length dedent
                        });
                    }
                }
                std::cmp::Ordering::Equal => {
                    // Special case: newline after same indent level
                    if found_newline && valid_symbols.get(NEWLINE as usize) == Some(&true) {
                        return Some(ScanResult {
                            symbol: NEWLINE,
                            length: 1,
                        });
                    }
                }
            }
        }

        None
    }

    fn serialize(&self, buffer: &mut Vec<u8>) {
        // Serialize indent stack
        let stack = self.indent_stack.lock().unwrap();
        for &level in stack.iter() {
            buffer.extend_from_slice(&(level as u32).to_le_bytes());
        }
    }

    fn deserialize(&mut self, buffer: &[u8]) {
        // Deserialize indent stack
        let mut stack = self.indent_stack.lock().unwrap();
        stack.clear();
        for chunk in buffer.chunks_exact(4) {
            if let Ok(bytes) = chunk.try_into() {
                let level = u32::from_le_bytes(bytes) as usize;
                stack.push(level);
            }
        }
        if stack.is_empty() {
            stack.push(0);
        }
    }
}

#[test]
fn test_basic_indentation() {
    let input = b"def foo():\n    print('hello')\n    return 42\nbar()";
    let mut scanner = IndentationScanner::new();

    // Mock lexer for testing
    struct TestLexer<'a> {
        input: &'a [u8],
        position: usize,
        column: usize,
        mark: usize,
    }

    impl<'a> Lexer for TestLexer<'a> {
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
                    if byte == b'\n' {
                        self.column = 0;
                    } else {
                        self.column += 1;
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

    // Test sequence:
    // After "def foo():\n" we should get NEWLINE
    // At start of "    print..." we should get INDENT
    // After "return 42\n" we should get NEWLINE
    // At start of "bar()" we should get DEDENT

    let mut lexer = TestLexer {
        input,
        position: 11, // After "def foo():\n"
        column: 0,
        mark: 11,
    };

    // All symbols valid for testing
    let valid_symbols = vec![true; 2000];

    // Should detect indent at start of line with spaces
    let result = scanner.scan(&mut lexer, &valid_symbols);
    assert_eq!(
        result,
        Some(ScanResult {
            symbol: INDENT,
            length: 4
        })
    );
    assert_eq!(*scanner.indent_stack.lock().unwrap(), vec![0, 4]);

    // Move to next line with same indent
    lexer.position = 30; // After "print('hello')\n"
    lexer.column = 0;
    let result = scanner.scan(&mut lexer, &valid_symbols);
    assert_eq!(result, None); // Same indent level, no token

    // Move to dedent position
    lexer.position = 44; // After "return 42\n"
    lexer.column = 0;
    let result = scanner.scan(&mut lexer, &valid_symbols);
    assert_eq!(
        result,
        Some(ScanResult {
            symbol: DEDENT,
            length: 0
        })
    );
    assert_eq!(*scanner.indent_stack.lock().unwrap(), vec![0]);
}

#[test]
fn test_mixed_spaces_tabs() {
    let input = b"if x:\n\tprint(1)\n    print(2)";
    let mut scanner = IndentationScanner::new();

    struct TestLexer<'a> {
        input: &'a [u8],
        position: usize,
        column: usize,
        mark: usize,
    }

    impl<'a> Lexer for TestLexer<'a> {
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
                    if byte == b'\n' {
                        self.column = 0;
                    } else {
                        self.column += 1;
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

    let valid_symbols = vec![true; 2000];

    // After "if x:\n" with tab
    let mut lexer = TestLexer {
        input,
        position: 6,
        column: 0,
        mark: 6,
    };

    // Tab counts as 4 spaces
    let result = scanner.scan(&mut lexer, &valid_symbols);
    assert_eq!(
        result,
        Some(ScanResult {
            symbol: INDENT,
            length: 4
        })
    );

    // After "print(1)\n" with 4 spaces
    lexer.position = 16;
    lexer.column = 0;
    let result = scanner.scan(&mut lexer, &valid_symbols);
    assert_eq!(result, None); // Same indent level (4 spaces = 1 tab)
}

#[test]
fn test_serialization() {
    let scanner = IndentationScanner::new();
    *scanner.indent_stack.lock().unwrap() = vec![0, 4, 8];

    let mut buffer = Vec::new();
    scanner.serialize(&mut buffer);
    assert_eq!(buffer.len(), 12); // 3 levels * 4 bytes

    let mut scanner2 = IndentationScanner::new();
    scanner2.deserialize(&buffer);
    assert_eq!(*scanner2.indent_stack.lock().unwrap(), vec![0, 4, 8]);
}

#[test]
fn test_multi_dedent() {
    // Test handling multiple dedent levels at once
    // Python code:
    // if x:           # indent to 4
    //     if y:       # indent to 8
    //         pass    # indent to 12
    // z = 1           # dedent back to 0 (3 levels)
    let input = b"if x:\n    if y:\n        pass\nz = 1";
    let mut scanner = IndentationScanner::new();

    struct TestLexer<'a> {
        input: &'a [u8],
        position: usize,
        column: usize,
        mark: usize,
    }

    impl<'a> Lexer for TestLexer<'a> {
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
                    if byte == b'\n' {
                        self.column = 0;
                    } else {
                        self.column += 1;
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

    let mut lexer = TestLexer {
        input,
        position: 0,
        column: 0,
        mark: 0,
    };

    let mut valid_symbols = vec![false; 3000];
    valid_symbols[INDENT as usize] = true;
    valid_symbols[DEDENT as usize] = true;
    valid_symbols[NEWLINE as usize] = true;

    // Skip to first newline (after "if x:")
    lexer.position = 6;
    lexer.column = 0;

    // Should detect first INDENT (4 spaces)
    let result = scanner.scan(&mut lexer, &valid_symbols);
    assert_eq!(
        result,
        Some(ScanResult {
            symbol: INDENT,
            length: 4
        })
    );
    assert_eq!(*scanner.indent_stack.lock().unwrap(), vec![0, 4]);

    // Skip to second newline (after "if y:")
    lexer.position = 16;
    lexer.column = 0;

    // Should detect second INDENT (8 spaces)
    let result = scanner.scan(&mut lexer, &valid_symbols);
    assert_eq!(
        result,
        Some(ScanResult {
            symbol: INDENT,
            length: 8
        })
    );
    assert_eq!(*scanner.indent_stack.lock().unwrap(), vec![0, 4, 8]);

    // Skip to third newline (after "pass")
    lexer.position = 29;
    lexer.column = 0;

    // Now we're at column 0, should detect DEDENT
    // The scanner should produce one DEDENT at a time
    let result = scanner.scan(&mut lexer, &valid_symbols);
    assert_eq!(
        result,
        Some(ScanResult {
            symbol: DEDENT,
            length: 0
        })
    );

    // Stack should have popped one level
    assert_eq!(*scanner.indent_stack.lock().unwrap(), vec![0, 4]);

    // Call scan again for second DEDENT
    let result = scanner.scan(&mut lexer, &valid_symbols);
    assert_eq!(
        result,
        Some(ScanResult {
            symbol: DEDENT,
            length: 0
        })
    );

    // Stack should have popped another level
    assert_eq!(*scanner.indent_stack.lock().unwrap(), vec![0]);
}

#[test]
fn test_dedent_sequence() {
    // Test that multiple consecutive DEDENTs work correctly
    let input = b"if a:\n    if b:\n        if c:\n            pass\nend";
    let mut scanner = IndentationScanner::new();

    // Set up the indent stack as if we've indented 3 times
    *scanner.indent_stack.lock().unwrap() = vec![0, 4, 8, 12];

    struct TestLexer<'a> {
        input: &'a [u8],
        position: usize,
        column: usize,
        mark: usize,
    }

    impl<'a> Lexer for TestLexer<'a> {
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
                    if byte == b'\n' {
                        self.column = 0;
                    } else {
                        self.column += 1;
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

    // Position at the start of "end" (column 0)
    let mut lexer = TestLexer {
        input,
        position: 47, // Position at 'e' in "end"
        column: 0,
        mark: 0,
    };

    let mut valid_symbols = vec![false; 3000];
    valid_symbols[DEDENT as usize] = true;

    // Should produce 3 consecutive DEDENTs
    for expected_stack_size in [3, 2, 1] {
        let result = scanner.scan(&mut lexer, &valid_symbols);
        assert_eq!(
            result,
            Some(ScanResult {
                symbol: DEDENT,
                length: 0
            })
        );
        assert_eq!(
            scanner.indent_stack.lock().unwrap().len(),
            expected_stack_size
        );
    }

    // Final stack should be [0]
    assert_eq!(*scanner.indent_stack.lock().unwrap(), vec![0]);
}
