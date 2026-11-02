//! Error reporting and diagnostics.
#![cfg_attr(feature = "strict_docs", allow(missing_docs))]

// User-friendly error reporting for the GLR parser
use crate::glr_parser::GLRParser;
use crate::subtree::Subtree;
use adze_ir::SymbolId;
use std::fmt;
use std::sync::Arc;

/// Parse error with location and context (display-oriented diagnostic shape).
///
/// This is a different type from the facade-canonical
/// [`errors::ParseError`](crate::errors::ParseError) (reason + byte range),
/// despite the shared name — a known collision tracked by #871 §d. In-repo
/// query/display tests import this type directly, so no alias is introduced
/// here; unification belongs to the #871 facade decision, not this module.
#[derive(Debug, Clone)]
pub struct ParseError {
    /// Line number (1-indexed)
    pub line: usize,
    /// Column number (1-indexed)
    pub column: usize,
    /// The unexpected token
    pub unexpected_token: Option<String>,
    /// Expected tokens/symbols
    pub expected: Vec<String>,
    /// Additional context
    pub context: String,
}

/// Structured parse diagnostic with byte-span and display context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseDiagnostic {
    /// Line number (1-indexed)
    pub line: usize,
    /// Column number (1-indexed)
    pub column: usize,
    /// Start byte offset of the unexpected token or EOF position.
    pub start_byte: usize,
    /// End byte offset of the unexpected token or EOF position.
    pub end_byte: usize,
    /// The unexpected token.
    pub unexpected_token: Option<String>,
    /// Expected tokens/symbols.
    pub expected: Vec<String>,
    /// Additional source context.
    pub context: String,
}

impl From<ParseDiagnostic> for ParseError {
    fn from(diagnostic: ParseDiagnostic) -> Self {
        Self {
            line: diagnostic.line,
            column: diagnostic.column,
            unexpected_token: diagnostic.unexpected_token,
            expected: diagnostic.expected,
            context: diagnostic.context,
        }
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Parse error at {}:{}: ", self.line, self.column)?;

        if let Some(ref token) = self.unexpected_token {
            write!(f, "unexpected token '{}'", token)?;
        } else {
            write!(f, "unexpected end of input")?;
        }

        if !self.expected.is_empty() {
            write!(f, ", expected one of: {}", self.expected.join(", "))?;
        }

        if !self.context.is_empty() {
            write!(f, " ({})", self.context)?;
        }

        Ok(())
    }
}

impl fmt::Display for ParseDiagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Parse error at {}:{} (bytes {}..{}): ",
            self.line, self.column, self.start_byte, self.end_byte
        )?;

        if let Some(ref token) = self.unexpected_token {
            write!(f, "unexpected token '{}'", token)?;
        } else {
            write!(f, "unexpected end of input")?;
        }

        if !self.expected.is_empty() {
            write!(f, ", expected one of: {}", self.expected.join(", "))?;
        }

        if !self.context.is_empty() {
            write!(f, " ({})", self.context)?;
        }

        Ok(())
    }
}

/// Error reporter that tracks parse state and generates helpful messages
pub struct ErrorReporter {
    /// Input text for error context
    input: String,
    /// Current line number
    current_line: usize,
    /// Current column number
    current_column: usize,
    /// Current byte offset
    current_byte: usize,
    /// Token positions
    token_positions: Vec<(usize, usize, usize, usize)>, // (start_line, start_col, end_line, end_col)
}

impl ErrorReporter {
    pub fn new(input: String) -> Self {
        Self {
            input,
            current_line: 1,
            current_column: 1,
            current_byte: 0,
            token_positions: Vec::new(),
        }
    }

    /// Record a token position
    pub fn record_token(&mut self, token: &str, byte_offset: usize) {
        let start_line = self.current_line;
        let start_col = self.current_column;
        let start_byte = if byte_offset == 0 && self.current_byte != 0 {
            self.current_byte
        } else {
            byte_offset
        };

        // Update position based on token content
        for ch in token.chars() {
            if ch == '\n' {
                self.current_line += 1;
                self.current_column = 1;
            } else {
                self.current_column += 1;
            }
        }

        let end_line = self.current_line;
        let end_col = self.current_column;
        self.current_byte = start_byte + token.len();

        self.token_positions
            .push((start_line, start_col, end_line, end_col));
    }

    /// Generate error at current position
    pub fn error_at_current(&self, parser: &GLRParser, unexpected: Option<String>) -> ParseError {
        self.diagnostic_at_current(parser, unexpected).into()
    }

    /// Generate a structured diagnostic at current position.
    pub fn diagnostic_at_current(
        &self,
        parser: &GLRParser,
        unexpected: Option<String>,
    ) -> ParseDiagnostic {
        let expected = self.get_expected_tokens(parser);
        let start_byte = self.current_byte;
        let end_byte = start_byte + unexpected.as_ref().map_or(0, |token| token.len());

        ParseDiagnostic {
            line: self.current_line,
            column: self.current_column,
            start_byte,
            end_byte,
            unexpected_token: unexpected,
            expected,
            context: self.get_context(),
        }
    }

    /// Get expected tokens from parser state
    fn get_expected_tokens(&self, parser: &GLRParser) -> Vec<String> {
        expected_token_names(parser)
    }

    /// Get context around the error
    fn get_context(&self) -> String {
        // Extract a line or snippet around the error position
        let lines: Vec<&str> = self.input.lines().collect();
        if self.current_line > 0 && self.current_line <= lines.len() {
            let line = lines[self.current_line - 1];
            let marker = " ".repeat(self.current_column.saturating_sub(1)) + "^";
            format!("\n{}\n{}", line, marker)
        } else {
            String::new()
        }
    }
}

/// Extension trait for GLRParser to add error reporting
pub trait ErrorReportingExt {
    fn parse_with_errors(
        &mut self,
        tokens: Vec<(SymbolId, String)>,
    ) -> Result<Subtree, Vec<ParseError>>;
}

impl ErrorReportingExt for GLRParser {
    fn parse_with_errors(
        &mut self,
        tokens: Vec<(SymbolId, String)>,
    ) -> Result<Subtree, Vec<ParseError>> {
        self.parse_with_diagnostics(tokens)
            .map_err(|errors| errors.into_iter().map(ParseError::from).collect())
    }
}

impl GLRParser {
    /// Parse a token stream and return structured diagnostics on failure.
    pub fn parse_with_diagnostics(
        &mut self,
        tokens: Vec<(SymbolId, String)>,
    ) -> Result<Subtree, Vec<ParseDiagnostic>> {
        let mut errors = Vec::new();
        let source = tokens
            .iter()
            .map(|(_, token_text)| token_text.as_str())
            .collect::<String>();
        let mut reporter = ErrorReporter::new(source);
        let mut byte_offset = 0;

        for (symbol_id, token_text) in tokens {
            let expected_before_token = expected_token_names(self);
            if !expected_before_token.is_empty() && !self.expected_symbols().contains(&symbol_id) {
                let mut error = reporter.diagnostic_at_current(self, Some(token_text));
                if error.expected.is_empty() {
                    error.expected = expected_before_token;
                }
                errors.push(error);
                return Err(errors);
            }

            // Try to process the token
            let initial_stack_count = self.stack_count();
            self.process_token(symbol_id, &token_text, byte_offset);

            // Check if all stacks died (parse error)
            if self.stack_count() == 0 && initial_stack_count > 0 {
                let mut error = reporter.diagnostic_at_current(self, Some(token_text.clone()));
                if error.expected.is_empty() {
                    error.expected = expected_before_token;
                }
                errors.push(error);
                return Err(errors);
            }

            reporter.record_token(&token_text, byte_offset);
            byte_offset += token_text.len();
        }

        let expected_before_eof = expected_token_names(self);
        self.process_eof(byte_offset);

        if let Some(tree) = self.get_best_parse() {
            Ok(Arc::try_unwrap(tree).unwrap_or_else(|arc| (*arc).clone()))
        } else {
            let mut error = reporter.diagnostic_at_current(self, None);
            if error.expected.is_empty() {
                error.expected = expected_before_eof;
            }
            errors.push(error);
            Err(errors)
        }
    }
}

fn expected_token_names(parser: &GLRParser) -> Vec<String> {
    parser.expected_symbol_names()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let error = ParseError {
            line: 3,
            column: 15,
            unexpected_token: Some("foo".to_string()),
            expected: vec!["number".to_string(), "string".to_string()],
            context: "in object member".to_string(),
        };

        let display = format!("{}", error);
        assert!(display.contains("3:15"));
        assert!(display.contains("unexpected token 'foo'"));
        assert!(display.contains("expected one of: number, string"));
        assert!(display.contains("(in object member)"));
    }

    #[test]
    fn test_diagnostic_display_includes_byte_span() {
        let error = ParseDiagnostic {
            line: 3,
            column: 15,
            start_byte: 20,
            end_byte: 23,
            unexpected_token: Some("foo".to_string()),
            expected: vec!["number".to_string()],
            context: String::new(),
        };

        let display = format!("{}", error);
        assert!(display.contains("3:15"));
        assert!(display.contains("bytes 20..23"));
        assert!(display.contains("unexpected token 'foo'"));
    }

    #[test]
    fn test_error_reporter() {
        let mut reporter = ErrorReporter::new("{\n  \"key\": \n}".to_string());

        reporter.record_token("{", 0);
        assert_eq!(reporter.current_line, 1);
        assert_eq!(reporter.current_column, 2);

        reporter.record_token("\n", 1);
        assert_eq!(reporter.current_line, 2);
        assert_eq!(reporter.current_column, 1);

        reporter.record_token("\"key\"", 4);
        assert_eq!(reporter.current_line, 2);
        assert_eq!(reporter.current_column, 6);
    }
}
