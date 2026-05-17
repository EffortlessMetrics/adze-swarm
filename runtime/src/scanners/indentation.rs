// Indentation-based scanning for languages like Python
use crate::external_scanner::{ExternalScanner, Lexer, ScanResult};

/// Scanner for tracking indentation levels
#[derive(Debug, Clone)]
pub struct IndentationScanner {
    indent_stack: Vec<usize>,
    at_line_start: bool,
    pending_dedents: usize,
}

impl IndentationScanner {
    pub fn new() -> Self {
        IndentationScanner {
            indent_stack: vec![0], // Start with column 0
            at_line_start: true,
            pending_dedents: 0,
        }
    }
}

impl Default for IndentationScanner {
    fn default() -> Self {
        Self::new()
    }
}

impl ExternalScanner for IndentationScanner {
    fn scan(&mut self, lexer: &mut dyn Lexer, valid_symbols: &[bool]) -> Option<ScanResult> {
        const NEWLINE: usize = 0;
        const INDENT: usize = 1;
        const DEDENT: usize = 2;

        // If we have pending dedents, emit them
        if self.pending_dedents > 0 && valid_symbols.get(DEDENT) == Some(&true) {
            self.pending_dedents -= 1;
            return Some(ScanResult {
                symbol: DEDENT as u16,
                length: 0,
            });
        }

        if lexer.is_eof() {
            return None;
        }

        // Check for newline
        if valid_symbols.get(NEWLINE) == Some(&true) && lexer.lookahead() == Some(b'\n') {
            self.at_line_start = true;
            lexer.advance(1);
            lexer.mark_end();
            return Some(ScanResult {
                symbol: NEWLINE as u16,
                length: 1,
            });
        }

        // Handle indentation at start of line
        if self.at_line_start {
            let mut indent_count = 0;

            // Count leading whitespace
            while !lexer.is_eof() {
                match lexer.lookahead() {
                    Some(b' ') => {
                        indent_count += 1;
                        lexer.advance(1);
                    }
                    Some(b'\t') => {
                        indent_count += 8; // Tabs count as 8 spaces
                        lexer.advance(1);
                    }
                    _ => break,
                }
            }

            // Skip blank lines and comment lines
            if !lexer.is_eof() {
                let next = lexer.lookahead();
                if next != Some(b'\n') && next != Some(b'#') {
                    self.at_line_start = false;
                    let &current_indent = self.indent_stack.last()?;

                    if indent_count > current_indent {
                        // Indent
                        if valid_symbols.get(INDENT) == Some(&true) {
                            self.indent_stack.push(indent_count);
                            lexer.mark_end();
                            return Some(ScanResult {
                                symbol: INDENT as u16,
                                length: 0,
                            });
                        }
                    } else if indent_count < current_indent {
                        // Dedent(s)
                        if valid_symbols.get(DEDENT) == Some(&true) {
                            // Count how many dedents are needed
                            let mut dedent_count = 0;
                            let mut temp_stack = self.indent_stack.clone();

                            while let Some(&last) = temp_stack.last() {
                                if last <= indent_count {
                                    break;
                                }
                                temp_stack.pop();
                                dedent_count += 1;
                            }

                            if dedent_count > 0 {
                                // Apply the dedents
                                for _ in 0..dedent_count {
                                    self.indent_stack.pop();
                                }
                                self.pending_dedents = dedent_count - 1;
                                lexer.mark_end();
                                return Some(ScanResult {
                                    symbol: DEDENT as u16,
                                    length: 0,
                                });
                            }
                        }
                    }
                }
            }
        }

        None
    }

    fn serialize(&self, buffer: &mut Vec<u8>) {
        // Serialize indent stack
        buffer.extend_from_slice(&(self.indent_stack.len() as u16).to_le_bytes());
        for &indent in &self.indent_stack {
            buffer.extend_from_slice(&(indent as u16).to_le_bytes());
        }

        // Serialize flags
        buffer.push(if self.at_line_start { 1 } else { 0 });
        buffer.extend_from_slice(&(self.pending_dedents as u16).to_le_bytes());
    }

    fn deserialize(&mut self, buffer: &[u8]) {
        if buffer.len() < 2 {
            return;
        }

        self.indent_stack.clear();

        // Deserialize indent stack
        let stack_len = u16::from_le_bytes([buffer[0], buffer[1]]) as usize;
        let mut offset = 2;

        for _ in 0..stack_len {
            if offset + 2 > buffer.len() {
                break;
            }
            let indent = u16::from_le_bytes([buffer[offset], buffer[offset + 1]]) as usize;
            self.indent_stack.push(indent);
            offset += 2;
        }

        // Deserialize flags
        if offset < buffer.len() {
            self.at_line_start = buffer[offset] != 0;
            offset += 1;
        }

        if offset + 2 <= buffer.len() {
            self.pending_dedents =
                u16::from_le_bytes([buffer[offset], buffer[offset + 1]]) as usize;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockLexer {
        input: Vec<u8>,
        pos: usize,
        end_mark: usize,
    }

    impl MockLexer {
        fn new(input: &[u8]) -> Self {
            Self {
                input: input.to_vec(),
                pos: 0,
                end_mark: 0,
            }
        }
    }

    impl Lexer for MockLexer {
        fn lookahead(&self) -> Option<u8> {
            self.input.get(self.pos).copied()
        }

        fn advance(&mut self, n: usize) {
            self.pos = (self.pos + n).min(self.input.len());
        }

        fn mark_end(&mut self) {
            self.end_mark = self.pos;
        }

        fn column(&self) -> usize {
            let preceding = &self.input[..self.pos];
            preceding
                .iter()
                .rev()
                .position(|&byte| byte == b'\n')
                .unwrap_or(preceding.len())
        }

        fn is_eof(&self) -> bool {
            self.pos >= self.input.len()
        }
    }

    fn all_valid_symbols() -> [bool; 3] {
        [true, true, true]
    }

    #[test]
    fn default_matches_new_baseline_state() {
        let default_scanner = IndentationScanner::default();
        let new_scanner = IndentationScanner::new();

        assert_eq!(default_scanner.indent_stack, new_scanner.indent_stack);
        assert_eq!(default_scanner.at_line_start, new_scanner.at_line_start);
        assert_eq!(default_scanner.pending_dedents, new_scanner.pending_dedents);
        assert_eq!(default_scanner.indent_stack, vec![0]);
    }

    #[test]
    fn default_scanner_can_emit_first_indent() {
        let mut scanner = IndentationScanner::default();
        let mut lexer = MockLexer::new(b"    value");

        let result = scanner
            .scan(&mut lexer, &all_valid_symbols())
            .expect("expected first indentation token");

        assert_eq!(result.symbol, 1);
        assert_eq!(scanner.indent_stack, vec![0, 4]);
        assert!(!scanner.at_line_start);
    }

    #[test]
    fn deserialize_short_buffer_preserves_existing_baseline() {
        let mut scanner = IndentationScanner::default();

        scanner.deserialize(&[]);

        assert_eq!(scanner.indent_stack, vec![0]);
        assert!(scanner.at_line_start);
        assert_eq!(scanner.pending_dedents, 0);
    }
}
