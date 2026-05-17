// Heredoc scanner for shell-like languages
use crate::external_scanner::{ExternalScanner, Lexer, ScanResult};

/// Scanner for heredoc strings in shell-like languages
#[derive(Debug, Clone, Default)]
pub struct HeredocScanner {
    delimiter: Vec<u8>,
    in_heredoc: bool,
}

impl HeredocScanner {
    pub fn new() -> Self {
        HeredocScanner {
            delimiter: Vec::new(),
            in_heredoc: false,
        }
    }
}

impl ExternalScanner for HeredocScanner {
    fn scan(&mut self, lexer: &mut dyn Lexer, valid_symbols: &[bool]) -> Option<ScanResult> {
        const HEREDOC_START: usize = 0;
        const HEREDOC_BODY: usize = 1;
        const HEREDOC_END: usize = 2;

        if lexer.is_eof() {
            return None;
        }

        if !self.in_heredoc {
            // Look for heredoc start (<<DELIMITER)
            if valid_symbols.get(HEREDOC_START) == Some(&true) {
                // Check for <<
                if lexer.lookahead() == Some(b'<') {
                    lexer.advance(1);
                    if lexer.lookahead() == Some(b'<') {
                        lexer.advance(1);

                        // Skip optional whitespace
                        while lexer.lookahead() == Some(b' ') || lexer.lookahead() == Some(b'\t') {
                            lexer.advance(1);
                        }

                        // Read delimiter
                        self.delimiter.clear();
                        while !lexer.is_eof() {
                            if let Some(ch) = lexer.lookahead() {
                                if ch == b'\n' || ch == b' ' || ch == b'\t' {
                                    break;
                                }
                                self.delimiter.push(ch);
                                lexer.advance(1);
                            } else {
                                break;
                            }
                        }

                        if !self.delimiter.is_empty() {
                            self.in_heredoc = true;
                            lexer.mark_end();
                            return Some(ScanResult {
                                symbol: HEREDOC_START as u16,
                                length: 2 + self.delimiter.len(),
                            });
                        }
                    }
                }
            }
        } else {
            // Inside heredoc - look for delimiter or body
            if valid_symbols.get(HEREDOC_END) == Some(&true) {
                // Check if current line starts with delimiter
                if lexer.column() == 0 {
                    let mut matches = true;
                    let mut temp_pos = 0;

                    for &expected in &self.delimiter {
                        if lexer.lookahead() != Some(expected) {
                            matches = false;
                            break;
                        }
                        lexer.advance(1);
                        temp_pos += 1;
                    }

                    if matches && (lexer.lookahead() == Some(b'\n') || lexer.is_eof()) {
                        self.in_heredoc = false;
                        self.delimiter.clear();
                        lexer.mark_end();
                        return Some(ScanResult {
                            symbol: HEREDOC_END as u16,
                            length: temp_pos,
                        });
                    }

                    // Rewind if not a match
                    // Note: This is simplified - proper implementation would need better lookahead
                }
            }

            if valid_symbols.get(HEREDOC_BODY) == Some(&true) {
                // Consume heredoc body until end of line
                let mut length = 0;
                while !lexer.is_eof() && lexer.lookahead() != Some(b'\n') {
                    lexer.advance(1);
                    length += 1;
                }

                if lexer.lookahead() == Some(b'\n') {
                    lexer.advance(1);
                    length += 1;
                }

                if length > 0 {
                    lexer.mark_end();
                    return Some(ScanResult {
                        symbol: HEREDOC_BODY as u16,
                        length,
                    });
                }
            }
        }

        None
    }

    fn serialize(&self, buffer: &mut Vec<u8>) {
        // Serialize delimiter length and content
        buffer.extend_from_slice(&(self.delimiter.len() as u16).to_le_bytes());
        buffer.extend_from_slice(&self.delimiter);

        // Serialize state
        buffer.push(if self.in_heredoc { 1 } else { 0 });
    }

    fn deserialize(&mut self, buffer: &[u8]) {
        if buffer.len() < 2 {
            return;
        }

        // Deserialize delimiter
        let delimiter_len = u16::from_le_bytes([buffer[0], buffer[1]]) as usize;
        self.delimiter.clear();

        let offset = 2;
        if offset + delimiter_len <= buffer.len() {
            self.delimiter
                .extend_from_slice(&buffer[offset..offset + delimiter_len]);
        }

        // Deserialize state
        let state_offset = offset + delimiter_len;
        if state_offset < buffer.len() {
            self.in_heredoc = buffer[state_offset] != 0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Minimal in-memory Lexer for driving HeredocScanner from tests.
    struct MockLexer {
        input: Vec<u8>,
        pos: usize,
        end_mark: usize,
        column_for_pos_zero: usize,
    }

    impl MockLexer {
        fn new(input: &[u8]) -> Self {
            MockLexer {
                input: input.to_vec(),
                pos: 0,
                end_mark: 0,
                column_for_pos_zero: 0,
            }
        }

        fn with_column(mut self, column: usize) -> Self {
            self.column_for_pos_zero = column;
            self
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
            // Simplified: when at byte 0 use the configured column; otherwise
            // approximate column as bytes-since-last-newline.
            if self.pos == 0 {
                return self.column_for_pos_zero;
            }
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

    #[test]
    fn new_yields_empty_clean_state() {
        let scanner = HeredocScanner::new();
        assert!(scanner.delimiter.is_empty());
        assert!(!scanner.in_heredoc);
    }

    #[test]
    fn default_matches_new() {
        let new_scanner = HeredocScanner::new();
        let default_scanner = HeredocScanner::default();
        assert_eq!(new_scanner.delimiter, default_scanner.delimiter);
        assert_eq!(new_scanner.in_heredoc, default_scanner.in_heredoc);
    }

    #[test]
    fn scan_returns_none_at_eof() {
        let mut scanner = HeredocScanner::new();
        let mut lexer = MockLexer::new(b"");
        let result = scanner.scan(&mut lexer, &[true, true, true]);
        assert!(result.is_none());
    }

    #[test]
    fn scan_recognizes_heredoc_start_and_records_delimiter() {
        let mut scanner = HeredocScanner::new();
        let mut lexer = MockLexer::new(b"<<EOF\n");
        let result = scanner.scan(&mut lexer, &[true, true, true]);
        assert!(result.is_some());
        let res = result.unwrap();
        assert_eq!(res.symbol, 0); // HEREDOC_START
        assert_eq!(res.length, 2 + 3); // "<<" plus "EOF"
        assert!(scanner.in_heredoc);
        assert_eq!(scanner.delimiter, b"EOF");
    }

    #[test]
    fn scan_skips_whitespace_after_double_lt_before_delimiter() {
        let mut scanner = HeredocScanner::new();
        let mut lexer = MockLexer::new(b"<<  END\n");
        let result = scanner.scan(&mut lexer, &[true, true, true]);
        assert!(result.is_some());
        assert!(scanner.in_heredoc);
        assert_eq!(scanner.delimiter, b"END");
    }

    #[test]
    fn scan_does_not_start_heredoc_when_start_symbol_invalid() {
        let mut scanner = HeredocScanner::new();
        let mut lexer = MockLexer::new(b"<<EOF\n");
        // HEREDOC_START is index 0 -> mark invalid
        let result = scanner.scan(&mut lexer, &[false, true, true]);
        assert!(result.is_none());
        assert!(!scanner.in_heredoc);
    }

    #[test]
    fn scan_does_not_start_heredoc_on_single_lt() {
        let mut scanner = HeredocScanner::new();
        let mut lexer = MockLexer::new(b"<EOF\n");
        let result = scanner.scan(&mut lexer, &[true, true, true]);
        assert!(result.is_none());
        assert!(!scanner.in_heredoc);
    }

    #[test]
    fn scan_does_not_start_when_delimiter_is_empty() {
        let mut scanner = HeredocScanner::new();
        let mut lexer = MockLexer::new(b"<<\n");
        let result = scanner.scan(&mut lexer, &[true, true, true]);
        assert!(result.is_none());
        assert!(!scanner.in_heredoc);
    }

    #[test]
    fn scan_consumes_body_line_when_in_heredoc() {
        let mut scanner = HeredocScanner::new();
        scanner.in_heredoc = true;
        scanner.delimiter = b"EOF".to_vec();
        let mut lexer = MockLexer::new(b"some body text\n");
        // Only HEREDOC_BODY valid -> consume the line
        let result = scanner.scan(&mut lexer, &[false, true, false]);
        assert!(result.is_some());
        let res = result.unwrap();
        assert_eq!(res.symbol, 1); // HEREDOC_BODY
        assert_eq!(res.length, b"some body text\n".len());
        // Scanner should remain inside the heredoc.
        assert!(scanner.in_heredoc);
    }

    #[test]
    fn scan_recognizes_heredoc_end_at_column_zero() {
        let mut scanner = HeredocScanner::new();
        scanner.in_heredoc = true;
        scanner.delimiter = b"EOF".to_vec();
        let mut lexer = MockLexer::new(b"EOF\n");
        let result = scanner.scan(&mut lexer, &[false, false, true]);
        assert!(result.is_some());
        let res = result.unwrap();
        assert_eq!(res.symbol, 2); // HEREDOC_END
        assert!(!scanner.in_heredoc);
        assert!(scanner.delimiter.is_empty());
    }

    #[test]
    fn serialize_then_deserialize_roundtrips_state() {
        let mut original = HeredocScanner::new();
        original.in_heredoc = true;
        original.delimiter = b"DELIM".to_vec();

        let mut buf = Vec::new();
        original.serialize(&mut buf);

        let mut restored = HeredocScanner::new();
        restored.deserialize(&buf);
        assert_eq!(restored.delimiter, b"DELIM");
        assert!(restored.in_heredoc);
    }

    #[test]
    fn serialize_clean_state_is_minimal() {
        let scanner = HeredocScanner::new();
        let mut buf = Vec::new();
        scanner.serialize(&mut buf);
        // 2 bytes for length (zero) + 1 byte for state.
        assert_eq!(buf, vec![0, 0, 0]);
    }

    #[test]
    fn deserialize_short_buffer_is_noop() {
        let mut scanner = HeredocScanner::new();
        scanner.in_heredoc = true;
        scanner.delimiter = b"PRE".to_vec();
        scanner.deserialize(&[]);
        // No mutation when buffer is below the 2-byte header.
        assert_eq!(scanner.delimiter, b"PRE");
        assert!(scanner.in_heredoc);
    }

    #[test]
    fn deserialize_truncated_delimiter_leaves_delimiter_empty() {
        let mut scanner = HeredocScanner::new();
        // Length says 5 bytes but buffer has only 3 -> delimiter stays empty.
        let buf = [5u8, 0, b'a', b'b', b'c'];
        scanner.deserialize(&buf);
        assert!(scanner.delimiter.is_empty());
    }

    #[test]
    fn mock_lexer_column_uses_configured_value_at_pos_zero() {
        // Sanity check on the helper: covers the column() == 0 branch in
        // HEREDOC_END detection.
        let mut scanner = HeredocScanner::new();
        scanner.in_heredoc = true;
        scanner.delimiter = b"E".to_vec();
        let mut lexer = MockLexer::new(b"E\n").with_column(0);
        let result = scanner.scan(&mut lexer, &[false, false, true]);
        assert!(result.is_some());
        assert!(!scanner.in_heredoc);
    }
}
