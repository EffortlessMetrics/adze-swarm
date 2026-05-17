//! Span-aware parsed values and span validation helpers.

/// A wrapper that pairs a parsed value with its byte-offset span in the source.
///
/// `Spanned<T>` implements [`Deref<Target = T>`](std::ops::Deref) so the
/// inner value can be accessed transparently. The span can also be used to
/// index into the original source string.
///
/// # Examples
///
/// ```
/// use adze::Spanned;
///
/// let spanned = Spanned { value: 42, span: (0, 2) };
/// // Deref to the inner value:
/// assert_eq!(*spanned, 42);
/// // Use the span to slice source text:
/// let source = "42 + 1";
/// assert_eq!(&source[spanned], "42");
/// ```
#[derive(Clone, Debug)]
pub struct Spanned<T> {
    /// The underlying parsed value.
    pub value: T,
    /// Byte-offset span `(start, end)` where `start` is inclusive and `end` is
    /// exclusive.
    pub span: (usize, usize),
}

impl<T> std::ops::Deref for Spanned<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.value
    }
}

impl<T: crate::Extract<U>, U> crate::Extract<Spanned<U>> for Spanned<T> {
    type LeafFn = T::LeafFn;

    #[cfg(not(feature = "pure-rust"))]
    fn extract(
        node: Option<crate::tree_sitter::Node>,
        source: &[u8],
        last_idx: usize,
        leaf_fn: Option<&Self::LeafFn>,
    ) -> Spanned<U> {
        Spanned {
            value: <T as crate::Extract<U>>::extract(node, source, last_idx, leaf_fn),
            span: node
                .map(|n| (n.start_byte(), n.end_byte()))
                .unwrap_or((last_idx, last_idx)),
        }
    }

    #[cfg(feature = "pure-rust")]
    fn extract(
        node: Option<&crate::pure_parser::ParsedNode>,
        source: &[u8],
        last_idx: usize,
        leaf_fn: Option<&Self::LeafFn>,
    ) -> Spanned<U> {
        Spanned {
            value: <T as crate::Extract<U>>::extract(node, source, last_idx, leaf_fn),
            span: node
                .map(|n| (n.start_byte, n.end_byte))
                .unwrap_or((last_idx, last_idx)),
        }
    }
}

/// Error type for invalid span operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpanError {
    /// The invalid span that was used.
    pub span: (usize, usize),
    /// The length of the source being indexed.
    pub source_len: usize,
    /// Detailed description of what went wrong.
    pub reason: SpanErrorReason,
}

/// Specific reasons for span validation failures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpanErrorReason {
    /// Start position is greater than end position.
    StartGreaterThanEnd,
    /// Start position exceeds source length.
    StartOutOfBounds,
    /// End position exceeds source length.
    EndOutOfBounds,
}

impl std::fmt::Display for SpanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (start, end) = self.span;
        match self.reason {
            SpanErrorReason::StartGreaterThanEnd => {
                write!(
                    f,
                    "Invalid span {start}..{end}: start ({start}) > end ({end})"
                )
            }
            SpanErrorReason::StartOutOfBounds => {
                write!(
                    f,
                    "Invalid span {start}..{end}: start ({start}) > source length ({})",
                    self.source_len
                )
            }
            SpanErrorReason::EndOutOfBounds => {
                write!(
                    f,
                    "Invalid span {start}..{end}: end ({end}) > source length ({})",
                    self.source_len
                )
            }
        }
    }
}

impl std::error::Error for SpanError {}

/// Validates a span against a source of given length.
///
/// Performs comprehensive bounds checking before any memory access.
/// Returns `Ok(())` if the span is valid, or `Err(SpanError)` with detailed
/// error information if the span is invalid.
fn validate_span(span: (usize, usize), source_len: usize) -> Result<(), SpanError> {
    let (start, end) = span;

    // Check if start > end
    if start > end {
        return Err(SpanError {
            span,
            source_len,
            reason: SpanErrorReason::StartGreaterThanEnd,
        });
    }

    // Check if start > source_len
    if start > source_len {
        return Err(SpanError {
            span,
            source_len,
            reason: SpanErrorReason::StartOutOfBounds,
        });
    }

    // Check if end > source_len
    if end > source_len {
        return Err(SpanError {
            span,
            source_len,
            reason: SpanErrorReason::EndOutOfBounds,
        });
    }

    Ok(())
}

impl<T> std::ops::Index<Spanned<T>> for str {
    type Output = str;

    fn index(&self, span: Spanned<T>) -> &Self::Output {
        let (start, end) = span.span;
        let source_len = self.len();

        // Proactive span validation before any memory access
        if let Err(error) = validate_span(span.span, source_len) {
            panic!("Invalid span: {}", error);
        }

        // Safe to access since we've validated the span
        // Using direct indexing here is safe because we've already validated bounds
        &self[start..end]
    }
}

impl<T> std::ops::IndexMut<Spanned<T>> for str {
    fn index_mut(&mut self, span: Spanned<T>) -> &mut Self::Output {
        let (start, end) = span.span;
        let source_len = self.len();

        // Proactive span validation before any memory access
        if let Err(error) = validate_span(span.span, source_len) {
            panic!("Invalid span: {}", error);
        }

        // Safe to access since we've validated the span
        // Using direct indexing here is safe because we've already validated bounds
        &mut self[start..end]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn index_valid_span() {
        let source = "hello world";
        let span = Spanned {
            value: (),
            span: (0, 5),
        };
        assert_eq!(&source[span], "hello");
    }

    #[test]
    fn index_edge_cases() {
        let source = "hello";

        // Empty span at start
        let span = Spanned {
            value: (),
            span: (0, 0),
        };
        assert_eq!(&source[span], "");

        // Empty span at end
        let span = Spanned {
            value: (),
            span: (5, 5),
        };
        assert_eq!(&source[span], "");

        // Full string
        let span = Spanned {
            value: (),
            span: (0, 5),
        };
        assert_eq!(&source[span], "hello");
    }

    #[test]
    #[should_panic(expected = "Invalid span")]
    fn index_invalid_span_panics() {
        let source = "hello";
        let span = Spanned {
            value: (),
            span: (0, 10),
        };
        let _ = &source[span];
    }

    #[test]
    fn index_mut_valid_span() {
        let mut source = String::from("hello world");
        let span = Spanned {
            value: (),
            span: (6, 11),
        };
        source.as_mut_str()[span].make_ascii_uppercase();
        assert_eq!(source, "hello WORLD");
    }

    #[test]
    #[should_panic(expected = "Invalid span")]
    fn index_mut_invalid_span_panics() {
        let mut source = String::from("hello");
        let span = Spanned {
            value: (),
            span: (6, 7),
        };
        let s = source.as_mut_str();
        let _ = &mut s[span];
    }

    // New comprehensive span validation tests

    #[test]
    fn validate_span_valid() {
        assert!(validate_span((0, 5), 10).is_ok());
        assert!(validate_span((0, 0), 5).is_ok());
        assert!(validate_span((5, 5), 5).is_ok());
        assert!(validate_span((2, 8), 10).is_ok());
    }

    #[test]
    fn validate_span_start_greater_than_end() {
        let result = validate_span((5, 3), 10);
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert_eq!(error.reason, SpanErrorReason::StartGreaterThanEnd);
        assert_eq!(error.span, (5, 3));
        assert_eq!(error.source_len, 10);
    }

    #[test]
    fn validate_span_start_out_of_bounds() {
        let result = validate_span((11, 12), 10);
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert_eq!(error.reason, SpanErrorReason::StartOutOfBounds);
        assert_eq!(error.span, (11, 12));
        assert_eq!(error.source_len, 10);
    }

    #[test]
    fn validate_span_end_out_of_bounds() {
        let result = validate_span((5, 11), 10);
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert_eq!(error.reason, SpanErrorReason::EndOutOfBounds);
        assert_eq!(error.span, (5, 11));
        assert_eq!(error.source_len, 10);
    }

    #[test]
    fn span_error_display() {
        let error = SpanError {
            span: (5, 3),
            source_len: 10,
            reason: SpanErrorReason::StartGreaterThanEnd,
        };
        assert_eq!(error.to_string(), "Invalid span 5..3: start (5) > end (3)");

        let error = SpanError {
            span: (11, 12),
            source_len: 10,
            reason: SpanErrorReason::StartOutOfBounds,
        };
        assert_eq!(
            error.to_string(),
            "Invalid span 11..12: start (11) > source length (10)"
        );

        let error = SpanError {
            span: (5, 11),
            source_len: 10,
            reason: SpanErrorReason::EndOutOfBounds,
        };
        assert_eq!(
            error.to_string(),
            "Invalid span 5..11: end (11) > source length (10)"
        );
    }

    #[test]
    #[should_panic(expected = "Invalid span: Invalid span 12..15: start (12) > source length (5)")]
    fn index_start_out_of_bounds_detailed_error() {
        let source = "hello";
        let span = Spanned {
            value: (),
            span: (12, 15),
        };
        let _ = &source[span];
    }

    #[test]
    #[should_panic(expected = "Invalid span: Invalid span 2..10: end (10) > source length (5)")]
    fn index_end_out_of_bounds_detailed_error() {
        let source = "hello";
        let span = Spanned {
            value: (),
            span: (2, 10),
        };
        let _ = &source[span];
    }

    #[test]
    #[should_panic(expected = "Invalid span: Invalid span 5..3: start (5) > end (3)")]
    fn index_start_greater_than_end_detailed_error() {
        let source = "hello world";
        let span = Spanned {
            value: (),
            span: (5, 3),
        };
        let _ = &source[span];
    }

    #[test]
    #[should_panic(expected = "Invalid span: Invalid span 7..9: start (7) > source length (5)")]
    fn index_mut_start_out_of_bounds_detailed_error() {
        let mut source = String::from("hello");
        let span = Spanned {
            value: (),
            span: (7, 9),
        };
        let s = source.as_mut_str();
        let _ = &mut s[span];
    }
}
