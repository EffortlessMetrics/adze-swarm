//! Public diagnostic value types and source-aware formatting.

use super::diagnostics::source_line;
use super::*;

/// A structured parse diagnostic attached to a native document.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParseDiagnostic {
    /// Byte offset where the diagnostic begins.
    pub start_byte: usize,
    /// Byte offset where the diagnostic ends.
    pub end_byte: usize,
    /// Zero-based row/column range covered by the diagnostic.
    pub point_range: PointRange,
    /// Human-readable found token or symbol name, when known.
    pub found: Option<String>,
    /// Human-readable expected token or symbol names, when known.
    pub expected: Vec<String>,
    /// Document-local nodes related to this diagnostic.
    pub related_nodes: Vec<NodeId>,
    /// Human-readable diagnostic summary.
    pub message: String,
}

impl ParseDiagnostic {
    /// Return the byte span covered by this diagnostic.
    #[must_use]
    pub fn byte_span(&self) -> Range<usize> {
        self.start_byte..self.end_byte
    }

    /// Return a formatter that includes source location and context.
    #[must_use]
    pub fn display_with_source<'a>(&'a self, source: &'a str) -> ParseDiagnosticWithSource<'a> {
        ParseDiagnosticWithSource {
            diagnostic: self,
            source,
        }
    }

    pub(in crate::document) fn to_parse_error(&self) -> crate::errors::ParseError {
        crate::errors::ParseError {
            reason: crate::errors::ParseErrorReason::UnexpectedToken(self.message.clone()),
            start: self.start_byte,
            end: self.end_byte,
            expected: self.expected.clone(),
        }
    }
}

impl std::fmt::Display for ParseDiagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} at {}:{} (bytes {}..{})",
            self.message,
            self.point_range.start.row + 1,
            self.point_range.start.column + 1,
            self.start_byte,
            self.end_byte
        )
    }
}

/// Display helper returned by [`ParseDiagnostic::display_with_source`].
pub struct ParseDiagnosticWithSource<'a> {
    diagnostic: &'a ParseDiagnostic,
    source: &'a str,
}

impl std::fmt::Display for ParseDiagnosticWithSource<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.diagnostic)?;

        if let Some(line) = source_line(self.source, self.diagnostic.start_byte) {
            let range = self.diagnostic.point_range;
            let marker_width = if range.start.row == range.end.row {
                range.end.column.saturating_sub(range.start.column).max(1)
            } else {
                1
            };
            let marker =
                " ".repeat(range.start.column as usize) + &"^".repeat(marker_width as usize);
            write!(f, "\n{line}\n{marker}")?;
        }

        Ok(())
    }
}
