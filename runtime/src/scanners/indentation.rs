// Indentation-based scanning for languages like Python
use crate::external_scanner::{ExternalScanner, Lexer, ScanResult};

/// Scanner for tracking indentation levels
#[derive(Debug, Clone, Default)]
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
            MockLexer {
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
                .position(|&b| b == b'\n')
                .unwrap_or(preceding.len())
        }

        fn is_eof(&self) -> bool {
            self.pos >= self.input.len()
        }
    }

    fn all_valid() -> [bool; 3] {
        [true, true, true]
    }

    #[test]
    fn new_pushes_baseline_zero_indent() {
        let scanner = IndentationScanner::new();
        assert_eq!(scanner.indent_stack, vec![0]);
        assert!(scanner.at_line_start);
        assert_eq!(scanner.pending_dedents, 0);
    }

    #[test]
    fn default_is_clean_state_with_no_stack() {
        // Note: Default derives directly, so it does NOT push a 0 baseline.
        let scanner = IndentationScanner::default();
        assert!(scanner.indent_stack.is_empty());
    }

    #[test]
    fn scan_returns_none_at_eof_when_no_pending_dedents() {
        let mut scanner = IndentationScanner::new();
        let mut lexer = MockLexer::new(b"");
        assert!(scanner.scan(&mut lexer, &all_valid()).is_none());
    }

    #[test]
    fn newline_advances_lexer_and_emits_newline_token() {
        let mut scanner = IndentationScanner::new();
        scanner.at_line_start = false;
        let mut lexer = MockLexer::new(b"\n");
        let result = scanner.scan(&mut lexer, &all_valid());
        let res = result.expect("expected NEWLINE");
        assert_eq!(res.symbol, 0);
        assert_eq!(res.length, 1);
        assert!(scanner.at_line_start);
    }

    #[test]
    fn newline_symbol_gating_disables_newline_emit() {
        let mut scanner = IndentationScanner::new();
        scanner.at_line_start = false;
        let mut lexer = MockLexer::new(b"\n");
        // NEWLINE is index 0 -> disabled.
        let result = scanner.scan(&mut lexer, &[false, true, true]);
        assert!(result.is_none());
    }

    #[test]
    fn indent_increase_pushes_stack_and_emits_indent() {
        let mut scanner = IndentationScanner::new();
        // Stack is [0]; encountering "    x" produces INDENT to push 4.
        let mut lexer = MockLexer::new(b"    x");
        let result = scanner.scan(&mut lexer, &all_valid());
        let res = result.expect("expected INDENT");
        assert_eq!(res.symbol, 1);
        assert_eq!(scanner.indent_stack.last(), Some(&4));
        assert!(!scanner.at_line_start);
    }

    #[test]
    fn tab_counts_as_eight_spaces_for_indent() {
        let mut scanner = IndentationScanner::new();
        let mut lexer = MockLexer::new(b"\tx");
        let result = scanner.scan(&mut lexer, &all_valid());
        assert!(result.is_some());
        assert_eq!(scanner.indent_stack.last(), Some(&8));
    }

    #[test]
    fn dedent_pops_stack_and_returns_dedent() {
        let mut scanner = IndentationScanner::new();
        scanner.indent_stack = vec![0, 4, 8];
        // After "    x" we should pop 8 -> emit one DEDENT, no pending.
        let mut lexer = MockLexer::new(b"    x");
        let result = scanner.scan(&mut lexer, &all_valid());
        let res = result.expect("expected DEDENT");
        assert_eq!(res.symbol, 2);
        assert_eq!(scanner.indent_stack, vec![0, 4]);
        assert_eq!(scanner.pending_dedents, 0);
    }

    #[test]
    fn multiple_dedents_emit_one_and_queue_pending() {
        let mut scanner = IndentationScanner::new();
        scanner.indent_stack = vec![0, 4, 8, 12];
        // Drop all the way to column 0 -> three dedents: emit 1 now, queue 2.
        let mut lexer = MockLexer::new(b"x");
        let result = scanner.scan(&mut lexer, &all_valid());
        let res = result.expect("expected first DEDENT");
        assert_eq!(res.symbol, 2);
        assert_eq!(scanner.pending_dedents, 2);
        assert_eq!(scanner.indent_stack, vec![0]);

        // Subsequent calls (even on a no-content lexer) should drain pending dedents.
        let mut empty = MockLexer::new(b"");
        let next = scanner.scan(&mut empty, &all_valid()).unwrap();
        assert_eq!(next.symbol, 2);
        assert_eq!(scanner.pending_dedents, 1);
        let next = scanner.scan(&mut empty, &all_valid()).unwrap();
        assert_eq!(next.symbol, 2);
        assert_eq!(scanner.pending_dedents, 0);
    }

    #[test]
    fn pending_dedents_blocked_when_dedent_symbol_invalid() {
        let mut scanner = IndentationScanner::new();
        scanner.pending_dedents = 2;
        let mut lexer = MockLexer::new(b"");
        // DEDENT (index 2) disabled -> falls through to EOF check -> None.
        let result = scanner.scan(&mut lexer, &[true, true, false]);
        assert!(result.is_none());
        assert_eq!(scanner.pending_dedents, 2);
    }

    #[test]
    fn blank_line_does_not_modify_indent_stack() {
        let mut scanner = IndentationScanner::new();
        let start_stack = scanner.indent_stack.clone();
        // Indented blank line.
        let mut lexer = MockLexer::new(b"    ");
        let result = scanner.scan(&mut lexer, &all_valid());
        assert!(result.is_none());
        assert_eq!(scanner.indent_stack, start_stack);
    }

    #[test]
    fn comment_line_does_not_modify_indent_stack() {
        let mut scanner = IndentationScanner::new();
        let start_stack = scanner.indent_stack.clone();
        let mut lexer = MockLexer::new(b"    # comment");
        let result = scanner.scan(&mut lexer, &all_valid());
        assert!(result.is_none());
        assert_eq!(scanner.indent_stack, start_stack);
    }

    #[test]
    fn equal_indent_emits_nothing_but_consumes_state_flag() {
        let mut scanner = IndentationScanner::new();
        scanner.indent_stack = vec![0, 4];
        let mut lexer = MockLexer::new(b"    x");
        let result = scanner.scan(&mut lexer, &all_valid());
        assert!(result.is_none());
        // Same indent level: at_line_start should flip to false now that the
        // significant token is reached.
        assert!(!scanner.at_line_start);
    }

    #[test]
    fn indent_symbol_invalid_does_not_push_stack() {
        let mut scanner = IndentationScanner::new();
        let mut lexer = MockLexer::new(b"    x");
        // INDENT (index 1) disabled.
        let result = scanner.scan(&mut lexer, &[true, false, true]);
        assert!(result.is_none());
        assert_eq!(scanner.indent_stack, vec![0]);
    }

    #[test]
    fn serialize_then_deserialize_roundtrips_state() {
        let mut original = IndentationScanner::new();
        original.indent_stack = vec![0, 4, 8];
        original.at_line_start = false;
        original.pending_dedents = 3;

        let mut buf = Vec::new();
        original.serialize(&mut buf);

        let mut restored = IndentationScanner::new();
        restored.deserialize(&buf);
        assert_eq!(restored.indent_stack, vec![0, 4, 8]);
        assert!(!restored.at_line_start);
        assert_eq!(restored.pending_dedents, 3);
    }

    #[test]
    fn deserialize_short_buffer_is_noop() {
        let mut scanner = IndentationScanner::new();
        let original = scanner.indent_stack.clone();
        scanner.deserialize(&[]);
        assert_eq!(scanner.indent_stack, original);
    }

    #[test]
    fn deserialize_truncated_stack_stops_early() {
        let mut scanner = IndentationScanner::new();
        // Claim 3 entries in the stack but only provide 1 (followed by an
        // unrelated trailing byte that should not be misread as a stack entry).
        let mut buf = Vec::new();
        buf.extend_from_slice(&3u16.to_le_bytes()); // stack length = 3
        buf.extend_from_slice(&4u16.to_le_bytes()); // 1st entry = 4
        // Buffer ends mid-stack -> remaining entries are skipped.
        scanner.deserialize(&buf);
        assert_eq!(scanner.indent_stack, vec![4]);
    }
}
