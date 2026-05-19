use super::types::{ExternalScanner, Lexer, ScanResult};

/// Example external scanner for string literals with escape sequences
#[derive(Default)]
pub struct StringScanner {
    /// Whether we're inside a string
    in_string: bool,
    /// The quote character used
    quote_char: Option<u8>,
}

impl StringScanner {
    pub fn new() -> Self {
        StringScanner {
            in_string: false,
            quote_char: None,
        }
    }
}

impl ExternalScanner for StringScanner {
    fn scan(&mut self, lexer: &mut dyn Lexer, valid_symbols: &[bool]) -> Option<ScanResult> {
        const STRING_START: usize = 0;
        const STRING_CONTENT: usize = 1;
        const STRING_END: usize = 2;

        if lexer.is_eof() {
            return None;
        }

        let current = lexer.lookahead()?;

        if !self.in_string {
            if valid_symbols.get(STRING_START) == Some(&true)
                && (current == b'"' || current == b'\'')
            {
                self.in_string = true;
                self.quote_char = Some(current);
                return Some(ScanResult {
                    symbol: STRING_START as u16,
                    length: 1,
                });
            }
        } else if let Some(quote) = self.quote_char {
            if current == quote {
                if valid_symbols.get(STRING_END) == Some(&true) {
                    self.in_string = false;
                    self.quote_char = None;
                    return Some(ScanResult {
                        symbol: STRING_END as u16,
                        length: 1,
                    });
                }
            } else if valid_symbols.get(STRING_CONTENT) == Some(&true) {
                let mut length = 0;

                while !lexer.is_eof() {
                    if let Some(ch) = lexer.lookahead() {
                        if ch == quote {
                            break;
                        }
                        lexer.advance(1);
                        length += 1;
                        if ch == b'\\' && !lexer.is_eof() {
                            lexer.advance(1);
                            length += 1;
                        }
                    } else {
                        break;
                    }
                }

                if length > 0 {
                    return Some(ScanResult {
                        symbol: STRING_CONTENT as u16,
                        length,
                    });
                }
            }
        }

        None
    }

    fn serialize(&self, buffer: &mut Vec<u8>) {
        buffer.push(if self.in_string { 1 } else { 0 });
        buffer.push(self.quote_char.unwrap_or(0));
    }

    fn deserialize(&mut self, buffer: &[u8]) {
        if buffer.len() >= 2 {
            self.in_string = buffer[0] != 0;
            self.quote_char = if buffer[1] != 0 {
                Some(buffer[1])
            } else {
                None
            };
        }
    }
}

/// External scanner for multi-line comments
pub struct CommentScanner {
    /// Nesting depth for nested comments
    depth: u32,
}

impl Default for CommentScanner {
    fn default() -> Self {
        Self::new()
    }
}

impl CommentScanner {
    pub fn new() -> Self {
        CommentScanner { depth: 0 }
    }
}

impl ExternalScanner for CommentScanner {
    fn scan(&mut self, lexer: &mut dyn Lexer, valid_symbols: &[bool]) -> Option<ScanResult> {
        const COMMENT_START: usize = 0;
        const COMMENT_CONTENT: usize = 1;
        const COMMENT_END: usize = 2;

        if lexer.is_eof() {
            return None;
        }

        let current = lexer.lookahead()?;
        lexer.advance(1);
        let next = lexer.lookahead().unwrap_or(0);
        lexer.advance(usize::MAX);

        if self.depth == 0 {
            if valid_symbols.get(COMMENT_START) == Some(&true) && current == b'/' && next == b'*' {
                self.depth = 1;
                return Some(ScanResult {
                    symbol: COMMENT_START as u16,
                    length: 2,
                });
            }
        } else if current == b'/' && next == b'*' {
            self.depth += 1;
            if valid_symbols.get(COMMENT_CONTENT) == Some(&true) {
                return Some(ScanResult {
                    symbol: COMMENT_CONTENT as u16,
                    length: 2,
                });
            }
        } else if current == b'*' && next == b'/' {
            self.depth -= 1;
            if self.depth == 0 && valid_symbols.get(COMMENT_END) == Some(&true) {
                return Some(ScanResult {
                    symbol: COMMENT_END as u16,
                    length: 2,
                });
            } else if valid_symbols.get(COMMENT_CONTENT) == Some(&true) {
                return Some(ScanResult {
                    symbol: COMMENT_CONTENT as u16,
                    length: 2,
                });
            }
        } else if valid_symbols.get(COMMENT_CONTENT) == Some(&true) {
            let mut length = 0;

            while !lexer.is_eof() {
                let ch = lexer.lookahead().unwrap_or(0);
                lexer.advance(1);
                if !lexer.is_eof() {
                    let next_ch = lexer.lookahead().unwrap_or(0);
                    if (ch == b'/' && next_ch == b'*') || (ch == b'*' && next_ch == b'/') {
                        break;
                    }
                }
                length += 1;
            }

            if length > 0 {
                return Some(ScanResult {
                    symbol: COMMENT_CONTENT as u16,
                    length,
                });
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
