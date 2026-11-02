//! Lexer implementation and token processing.

// Lexer implementation for the Adze runtime
// This module provides lexical analysis capabilities

use adze_ir::{SymbolId, TokenPattern};
use regex::Regex;
use std::collections::HashMap;

/// Advanced lexer that uses token patterns from the grammar
pub struct GrammarLexer {
    /// Token patterns indexed by symbol ID
    patterns: HashMap<SymbolId, CompiledPattern>,
    /// Priority order for matching (higher priority first)
    priority_order: Vec<SymbolId>,
    /// Symbol IDs that should be skipped (like whitespace)
    skip_symbols: Vec<SymbolId>,
}

/// A compiled pattern ready for matching
#[derive(Debug)]
enum CompiledPattern {
    /// Literal string match
    Literal(String),
    /// Regular expression match
    Regex(Regex),
}

impl GrammarLexer {
    /// Create a new lexer from token patterns
    pub fn new(tokens: &[(SymbolId, TokenPattern, i32)]) -> Self {
        let mut patterns = HashMap::new();
        let mut priority_order = Vec::new();

        // Sort by priority (higher first)
        let mut sorted_tokens = tokens.to_vec();
        sorted_tokens.sort_by_key(|(_, _, priority)| -priority);

        for (symbol_id, pattern, _) in sorted_tokens {
            priority_order.push(symbol_id);

            let compiled = match pattern {
                TokenPattern::String(s) => CompiledPattern::Literal(s.clone()),
                TokenPattern::Regex(r) => {
                    // Add anchoring to ensure we match from the beginning
                    let anchored = format!("^{}", r);
                    match Regex::new(&anchored) {
                        Ok(regex) => CompiledPattern::Regex(regex),
                        Err(_) => continue, // Skip invalid regexes
                    }
                }
            };

            patterns.insert(symbol_id, compiled);
        }

        Self {
            patterns,
            priority_order,
            skip_symbols: Vec::new(),
        }
    }

    /// Mark certain symbols as skip tokens (like whitespace)
    pub fn set_skip_symbols(&mut self, symbols: Vec<SymbolId>) {
        self.skip_symbols = symbols;
    }

    /// Get the next token from the input
    pub fn next_token(&mut self, input: &[u8], mut position: usize) -> Option<Token> {
        // Skip any skip symbols first
        loop {
            let skipped = self.try_skip_tokens(input, position);
            if skipped == position {
                break;
            }
            position = skipped;
        }

        // Check if we're at EOF
        if position >= input.len() {
            return Some(Token {
                symbol: SymbolId(0), // EOF
                text: vec![],
                start: position,
                end: position,
            });
        }

        // Try to match patterns in priority order
        for symbol_id in &self.priority_order {
            if let Some(pattern) = self.patterns.get(symbol_id)
                && let Some(token) = self.try_match(pattern, *symbol_id, input, position)
            {
                return Some(token);
            }
        }

        // No match found - return error token
        None
    }

    /// Try to skip tokens at the current position
    fn try_skip_tokens(&self, input: &[u8], position: usize) -> usize {
        let mut pos = position;

        loop {
            let mut skipped_any = false;

            for skip_symbol in &self.skip_symbols {
                if let Some(pattern) = self.patterns.get(skip_symbol)
                    && let Some(token) = self.try_match(pattern, *skip_symbol, input, pos)
                {
                    pos = token.end;
                    skipped_any = true;
                    break;
                }
            }

            if !skipped_any {
                break;
            }
        }

        pos
    }

    /// Try to match a pattern at the current position
    fn try_match(
        &self,
        pattern: &CompiledPattern,
        symbol_id: SymbolId,
        input: &[u8],
        position: usize,
    ) -> Option<Token> {
        let remaining = &input[position..];

        match pattern {
            CompiledPattern::Literal(s) => {
                let bytes = s.as_bytes();
                if remaining.starts_with(bytes) {
                    Some(Token {
                        symbol: symbol_id,
                        text: bytes.to_vec(),
                        start: position,
                        end: position + bytes.len(),
                    })
                } else {
                    None
                }
            }

            CompiledPattern::Regex(regex) => {
                // Convert to string for regex matching
                // This is not ideal for binary input, but works for UTF-8
                if let Ok(text) = std::str::from_utf8(remaining) {
                    if let Some(mat) = regex.find(text) {
                        let matched_bytes = mat.as_str().as_bytes();
                        Some(Token {
                            symbol: symbol_id,
                            text: matched_bytes.to_vec(),
                            start: position,
                            end: position + matched_bytes.len(),
                        })
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
        }
    }
}

/// A token produced by the lexer
#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    /// Symbol ID for this token
    pub symbol: SymbolId,
    /// Token text
    pub text: Vec<u8>,
    /// Start position in the input
    pub start: usize,
    /// End position in the input
    pub end: usize,
}

/// Error recovery mode for the lexer
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ErrorRecoveryMode {
    /// Skip single character and try again
    SkipChar,
    /// Skip until a known token is found
    SkipToKnown,
    /// Fail immediately
    Fail,
}

/// Lexer with error recovery capabilities
pub struct ErrorRecoveringLexer {
    /// Base lexer
    base: GrammarLexer,
    /// Error recovery mode
    recovery_mode: ErrorRecoveryMode,
    /// Symbol ID for error tokens
    error_symbol: SymbolId,
}

impl ErrorRecoveringLexer {
    /// Create a lexer that delegates to `base` and synthesizes error tokens
    /// tagged with `error_symbol`, starting in [`ErrorRecoveryMode::SkipChar`].
    pub fn new(base: GrammarLexer, error_symbol: SymbolId) -> Self {
        Self {
            base,
            recovery_mode: ErrorRecoveryMode::SkipChar,
            error_symbol,
        }
    }

    /// Switch the recovery strategy used when `base` yields no token.
    pub fn set_recovery_mode(&mut self, mode: ErrorRecoveryMode) {
        self.recovery_mode = mode;
    }

    /// Return the next token at `position`, applying the selected recovery mode.
    ///
    /// Delegates to the base lexer first, including its EOF token (symbol 0).
    /// At end of input, that token is returned unchanged in every recovery mode.
    /// For an unrecognized in-bounds byte, [`ErrorRecoveryMode::SkipChar`]
    /// emits a one-byte error token, [`ErrorRecoveryMode::SkipToKnown`] emits
    /// an error token up to the next recognized token boundary, and
    /// [`ErrorRecoveryMode::Fail`] returns `None`.
    pub fn next_token(&mut self, input: &[u8], position: usize) -> Option<Token> {
        // Try normal lexing first
        if let Some(token) = self.base.next_token(input, position) {
            return Some(token);
        }

        // Handle error recovery
        match self.recovery_mode {
            ErrorRecoveryMode::SkipChar => {
                // Skip one character and return error token
                if position < input.len() {
                    Some(Token {
                        symbol: self.error_symbol,
                        text: vec![input[position]],
                        start: position,
                        end: position + 1,
                    })
                } else {
                    None
                }
            }

            ErrorRecoveryMode::SkipToKnown => {
                // Skip characters until we find a known token
                let mut end = position + 1;
                while end < input.len() {
                    if self.base.next_token(input, end).is_some() {
                        break;
                    }
                    end += 1;
                }

                if end > position {
                    Some(Token {
                        symbol: self.error_symbol,
                        text: input[position..end].to_vec(),
                        start: position,
                        end,
                    })
                } else {
                    None
                }
            }

            ErrorRecoveryMode::Fail => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_literal_pattern() {
        let tokens = vec![
            (SymbolId(1), TokenPattern::String("+".to_string()), 0),
            (SymbolId(2), TokenPattern::String("-".to_string()), 0),
        ];

        let mut lexer = GrammarLexer::new(&tokens);

        let token = lexer.next_token(b"+", 0).unwrap();
        assert_eq!(token.symbol, SymbolId(1));
        assert_eq!(token.text, b"+");

        let token = lexer.next_token(b"-", 0).unwrap();
        assert_eq!(token.symbol, SymbolId(2));
        assert_eq!(token.text, b"-");
    }

    #[test]
    fn test_regex_pattern() {
        let tokens = vec![
            (SymbolId(1), TokenPattern::Regex(r"\d+".to_string()), 0),
            (
                SymbolId(2),
                TokenPattern::Regex(r"[a-zA-Z_]\w*".to_string()),
                0,
            ),
        ];

        let mut lexer = GrammarLexer::new(&tokens);

        let token = lexer.next_token(b"123", 0).unwrap();
        assert_eq!(token.symbol, SymbolId(1));
        assert_eq!(token.text, b"123");

        let token = lexer.next_token(b"hello", 0).unwrap();
        assert_eq!(token.symbol, SymbolId(2));
        assert_eq!(token.text, b"hello");
    }

    #[test]
    fn test_priority_order() {
        let tokens = vec![
            (SymbolId(1), TokenPattern::Regex(r"\w+".to_string()), 1), // Lower priority
            (SymbolId(2), TokenPattern::String("if".to_string()), 10), // Higher priority
        ];

        let mut lexer = GrammarLexer::new(&tokens);

        // "if" should match as keyword (SymbolId(2)) not identifier (SymbolId(1))
        let token = lexer.next_token(b"if", 0).unwrap();
        assert_eq!(token.symbol, SymbolId(2));
    }

    #[test]
    fn test_skip_symbols() {
        let tokens = vec![
            (SymbolId(1), TokenPattern::Regex(r"\d+".to_string()), 0),
            (SymbolId(2), TokenPattern::Regex(r"\s+".to_string()), 0),
        ];

        let mut lexer = GrammarLexer::new(&tokens);
        lexer.set_skip_symbols(vec![SymbolId(2)]); // Skip whitespace

        let token = lexer.next_token(b"  123", 0).unwrap();
        assert_eq!(token.symbol, SymbolId(1));
        assert_eq!(token.text, b"123");
        assert_eq!(token.start, 2); // Skipped 2 spaces
    }

    #[test]
    fn test_error_recovery_skip_char() {
        let tokens = vec![(SymbolId(1), TokenPattern::Regex(r"\d+".to_string()), 0)];

        let base = GrammarLexer::new(&tokens);
        let mut lexer = ErrorRecoveringLexer::new(base, SymbolId(999));

        // '@' is not a valid token
        let token = lexer.next_token(b"@123", 0).unwrap();
        assert_eq!(token.symbol, SymbolId(999)); // Error token
        assert_eq!(token.text, b"@");
        assert_eq!(token.end, 1);

        // Next token should be the number
        let token = lexer.next_token(b"@123", 1).unwrap();
        assert_eq!(token.symbol, SymbolId(1));
        assert_eq!(token.text, b"123");
    }
}
