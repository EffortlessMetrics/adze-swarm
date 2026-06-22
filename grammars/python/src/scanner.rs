// Python indentation scanner for adze

use adze::external_scanner::{ExternalScanner, Lexer, ScanResult};

// These are the actual symbol IDs from the Python grammar
// Found from test output: Valid externals for state 0: {SymbolId(203-211)}
// The valid_symbols array uses indices 0-8, not the symbol IDs
#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(u16)]
pub enum TokenType {
    Newline = 203,
    Indent = 204,
    Dedent = 205,
    StringStart = 206,
    StringEnd = 207,
    StringContent = 208,
    Comment = 209,
    LineJoining = 210,
    ErrorRecovery = 211,
}

// Indices in the valid_symbols array
const NEWLINE_INDEX: usize = 0;
const INDENT_INDEX: usize = 1;
const DEDENT_INDEX: usize = 2;
const STRING_START_INDEX: usize = 3;
const STRING_END_INDEX: usize = 4;
const STRING_CONTENT_INDEX: usize = 5;
const _COMMENT_INDEX: usize = 6;
const _LINE_JOINING_INDEX: usize = 7;
const _ERROR_RECOVERY_INDEX: usize = 8;

#[derive(Debug, Clone)]
pub struct PythonScanner {
    indent_stack: Vec<u16>,
    inside_string: bool,
    string_delimiter: Option<StringDelimiter>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[expect(
    clippy::enum_variant_names,
    reason = "grammar scanner enum variants share a common prefix that is part of the token naming convention"
)]
enum StringDelimiter {
    SingleQuote,
    DoubleQuote,
    TripleSingleQuote,
    TripleDoubleQuote,
}

impl PythonScanner {
    pub fn new() -> Self {
        PythonScanner {
            indent_stack: vec![0], // Start with zero indentation
            inside_string: false,
            string_delimiter: None,
        }
    }
}

impl Default for PythonScanner {
    fn default() -> Self {
        Self::new()
    }
}

impl ExternalScanner for PythonScanner {
    fn scan(&mut self, lexer: &mut dyn Lexer, valid_symbols: &[bool]) -> Option<ScanResult> {
        // Special case: handle NEWLINE when we're at column 0 and it's valid
        // But only if we're actually at a newline character, not just at the start of input
        if lexer.column() == 0
            && valid_symbols.len() > NEWLINE_INDEX
            && valid_symbols[NEWLINE_INDEX]
        {
            // Check if we're actually at a newline character
            if let Some(b'\n') = lexer.lookahead() {
                lexer.advance(1); // Consume the newline
                lexer.mark_end();
                return Some(ScanResult {
                    symbol: TokenType::Newline as u16,
                    length: 1,
                });
            }
            // Otherwise, don't emit a NEWLINE just because we're at column 0
        }

        if lexer.is_eof() {
            return None;
        }

        // Handle string scanning
        if self.inside_string && valid_symbols.get(STRING_END_INDEX) == Some(&true) {
            if let Some(delimiter) = self.string_delimiter {
                match delimiter {
                    StringDelimiter::SingleQuote => {
                        if let Some(b'\'') = lexer.lookahead() {
                            self.inside_string = false;
                            self.string_delimiter = None;
                            lexer.mark_end();
                            return Some(ScanResult {
                                symbol: TokenType::StringEnd as u16,
                                length: 1,
                            });
                        }
                    }
                    StringDelimiter::DoubleQuote => {
                        if let Some(b'"') = lexer.lookahead() {
                            self.inside_string = false;
                            self.string_delimiter = None;
                            lexer.mark_end();
                            return Some(ScanResult {
                                symbol: TokenType::StringEnd as u16,
                                length: 1,
                            });
                        }
                    }
                    StringDelimiter::TripleSingleQuote => {
                        if self.match_triple_at_lexer(lexer, b'\'') {
                            self.inside_string = false;
                            self.string_delimiter = None;
                            lexer.mark_end();
                            return Some(ScanResult {
                                symbol: TokenType::StringEnd as u16,
                                length: 3,
                            });
                        }
                    }
                    StringDelimiter::TripleDoubleQuote => {
                        if self.match_triple_at_lexer(lexer, b'"') {
                            self.inside_string = false;
                            self.string_delimiter = None;
                            lexer.mark_end();
                            return Some(ScanResult {
                                symbol: TokenType::StringEnd as u16,
                                length: 3,
                            });
                        }
                    }
                }
            }

            // If we're in a string but can't find the end, consume content
            if valid_symbols.get(STRING_CONTENT_INDEX) == Some(&true) {
                let mut length = 0;

                while !lexer.is_eof() {
                    // Check if we hit the string delimiter
                    match self.string_delimiter {
                        Some(StringDelimiter::SingleQuote) if lexer.lookahead() == Some(b'\'') => {
                            break;
                        }
                        Some(StringDelimiter::DoubleQuote) if lexer.lookahead() == Some(b'"') => {
                            break;
                        }
                        Some(StringDelimiter::TripleSingleQuote)
                            if self.match_triple_at_lexer(lexer, b'\'') =>
                        {
                            break;
                        }
                        Some(StringDelimiter::TripleDoubleQuote)
                            if self.match_triple_at_lexer(lexer, b'"') =>
                        {
                            break;
                        }
                        _ => {}
                    }

                    // Handle escape sequences
                    if lexer.lookahead() == Some(b'\\') {
                        lexer.advance(1);
                        length += 1;
                        if !lexer.is_eof() {
                            lexer.advance(1);
                            length += 1;
                        }
                    } else {
                        lexer.advance(1);
                        length += 1;
                    }
                }

                if length > 0 {
                    lexer.mark_end();
                    return Some(ScanResult {
                        symbol: TokenType::StringContent as u16,
                        length,
                    });
                }
            }

            return None;
        }

        // Check for string start
        if valid_symbols.get(STRING_START_INDEX) == Some(&true) {
            // Check for triple quotes first
            if self.match_triple_at_lexer(lexer, b'\'') {
                self.inside_string = true;
                self.string_delimiter = Some(StringDelimiter::TripleSingleQuote);
                lexer.mark_end();
                return Some(ScanResult {
                    symbol: TokenType::StringStart as u16,
                    length: 3,
                });
            }

            if self.match_triple_at_lexer(lexer, b'"') {
                self.inside_string = true;
                self.string_delimiter = Some(StringDelimiter::TripleDoubleQuote);
                lexer.mark_end();
                return Some(ScanResult {
                    symbol: TokenType::StringStart as u16,
                    length: 3,
                });
            }

            // Check for single quotes
            if lexer.lookahead() == Some(b'\'') {
                self.inside_string = true;
                self.string_delimiter = Some(StringDelimiter::SingleQuote);
                lexer.advance(1);
                lexer.mark_end();
                return Some(ScanResult {
                    symbol: TokenType::StringStart as u16,
                    length: 1,
                });
            }

            if lexer.lookahead() == Some(b'"') {
                self.inside_string = true;
                self.string_delimiter = Some(StringDelimiter::DoubleQuote);
                lexer.advance(1);
                lexer.mark_end();
                return Some(ScanResult {
                    symbol: TokenType::StringStart as u16,
                    length: 1,
                });
            }
        }

        // Handle newlines and indentation
        if valid_symbols.get(NEWLINE_INDEX) == Some(&true) && lexer.lookahead() == Some(b'\n') {
            lexer.advance(1);
            lexer.mark_end();
            return Some(ScanResult {
                symbol: TokenType::Newline as u16,
                length: 1,
            });
        }

        // Handle indentation at the beginning of a line (column 0)
        if lexer.column() == 0 {
            let mut indent_length = 0;

            // Count leading whitespace
            while !lexer.is_eof() {
                match lexer.lookahead() {
                    Some(b' ') => {
                        indent_length += 1;
                        lexer.advance(1);
                    }
                    Some(b'\t') => {
                        indent_length += 8; // Tabs count as 8 spaces
                        lexer.advance(1);
                    }
                    _ => break,
                }
            }

            // Don't emit indentation tokens for blank lines or comments
            if !lexer.is_eof() {
                let next_char = lexer.lookahead();
                if next_char != Some(b'\n') && next_char != Some(b'#') {
                    // Get current indentation level
                    let current_indent = self.indent_stack.last().copied().unwrap_or(0);

                    if valid_symbols.get(INDENT_INDEX) == Some(&true)
                        && indent_length > current_indent
                    {
                        self.indent_stack.push(indent_length);
                        lexer.mark_end();
                        return Some(ScanResult {
                            symbol: TokenType::Indent as u16,
                            length: 0, // Indents don't consume characters
                        });
                    }

                    if valid_symbols.get(DEDENT_INDEX) == Some(&true)
                        && indent_length < current_indent
                    {
                        // Pop all indentation levels greater than the current line's indentation
                        while let Some(&last_indent) = self.indent_stack.last() {
                            if last_indent <= indent_length {
                                break;
                            }
                            self.indent_stack.pop();
                        }

                        lexer.mark_end();
                        return Some(ScanResult {
                            symbol: TokenType::Dedent as u16,
                            length: 0, // Dedents don't consume characters
                        });
                    }
                }
            }
        }

        None
    }

    fn serialize(&self, buffer: &mut Vec<u8>) {
        // Serialize the indent stack
        buffer.extend_from_slice(&(self.indent_stack.len() as u16).to_le_bytes());
        for &indent in &self.indent_stack {
            buffer.extend_from_slice(&indent.to_le_bytes());
        }

        // Serialize string state
        buffer.push(if self.inside_string { 1 } else { 0 });
        buffer.push(match self.string_delimiter {
            None => 0,
            Some(StringDelimiter::SingleQuote) => 1,
            Some(StringDelimiter::DoubleQuote) => 2,
            Some(StringDelimiter::TripleSingleQuote) => 3,
            Some(StringDelimiter::TripleDoubleQuote) => 4,
        });
    }

    fn deserialize(&mut self, buffer: &[u8]) {
        self.indent_stack.clear();

        if buffer.len() < 2 {
            return;
        }

        // Deserialize indent stack
        let stack_len = u16::from_le_bytes([buffer[0], buffer[1]]) as usize;
        let mut offset = 2;

        for _ in 0..stack_len {
            if offset + 2 > buffer.len() {
                break;
            }
            let indent = u16::from_le_bytes([buffer[offset], buffer[offset + 1]]);
            self.indent_stack.push(indent);
            offset += 2;
        }

        // Deserialize string state
        if offset < buffer.len() {
            self.inside_string = buffer[offset] != 0;
            offset += 1;
        }

        if offset < buffer.len() {
            self.string_delimiter = match buffer[offset] {
                1 => Some(StringDelimiter::SingleQuote),
                2 => Some(StringDelimiter::DoubleQuote),
                3 => Some(StringDelimiter::TripleSingleQuote),
                4 => Some(StringDelimiter::TripleDoubleQuote),
                _ => None,
            };
        }
    }
}

impl PythonScanner {
    fn match_triple_at_lexer(&self, lexer: &mut dyn Lexer, quote: u8) -> bool {
        // Non-advancing triple-quote check using multi-byte lookahead.
        // All three bytes must equal `quote`; otherwise the lexer position is
        // untouched and the caller can fall through to single-quote handling.
        lexer.lookahead() == Some(quote)
            && lexer.lookahead_n(1) == Some(quote)
            && lexer.lookahead_n(2) == Some(quote)
    }
}

// Export the scanner creation function
pub fn create_scanner() -> Box<dyn ExternalScanner> {
    Box::new(PythonScanner::new())
}
