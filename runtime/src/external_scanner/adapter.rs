/// External scanner adapter that bridges between Rust parsers and external scanners
/// Provides the TSLexer interface expected by Tree-sitter external scanners
use std::ops::Range;

/// Adapter that implements the Lexer trait for external scanners
/// Manages source position, line/column tracking, and included ranges
#[cfg(feature = "external_scanners")]
pub struct TSLexerAdapter<'a> {
    /// Source code being parsed
    src: &'a [u8],
    /// Current byte position in source
    cursor: usize,
    /// Position of last marked token end
    mark_end: usize,
    /// Current line number (0-based)
    row: u32,
    /// Current column in codepoints (0-based)
    col: u32,
    /// Precomputed line start positions for efficient column calculation
    line_starts: &'a [usize],
    /// Included ranges for embedded language support
    ranges: Ranges,
}

/// Manages included ranges for parsing embedded languages
struct Ranges {
    /// List of byte ranges that should be parsed
    spans: Box<[Range<usize>]>,
    /// Index of the current range being processed
    next: usize,
}

impl<'a> TSLexerAdapter<'a> {
    /// Create a new lexer adapter
    pub fn new(
        src: &'a [u8],
        cursor: usize,
        line_starts: &'a [usize],
        ranges: Vec<Range<usize>>,
    ) -> Self {
        // Calculate initial row/col from cursor position
        let (row, col) = position_to_line_col(src, cursor, line_starts);

        // Find which range contains the cursor
        let mut next = 0;
        for (i, range) in ranges.iter().enumerate() {
            if cursor < range.end {
                next = i;
                break;
            }
        }

        Self {
            src,
            cursor,
            mark_end: cursor,
            row,
            col,
            line_starts,
            ranges: Ranges {
                spans: ranges.into_boxed_slice(),
                next,
            },
        }
    }

    /// Get the current range being processed
    fn current_range(&self) -> Option<&Range<usize>> {
        self.ranges.spans.get(self.ranges.next)
    }

    /// Update line/column position after advancing
    fn update_position(&mut self, byte: u8) {
        match byte {
            b'\n' => {
                self.row += 1;
                self.col = 0;
            }
            b'\r' => {
                // Handle CRLF: don't update yet, wait for potential LF
            }
            _ => {
                // Count codepoints for column (simplified - assumes ASCII/UTF-8)
                if (byte & 0b11000000) != 0b10000000 {
                    // Not a UTF-8 continuation byte
                    self.col += 1;
                }
            }
        }
    }
}

impl<'a> crate::external_scanner::Lexer for TSLexerAdapter<'a> {
    fn lookahead(&self) -> Option<u8> {
        // Check if we're at the end of current range
        if let Some(range) = self.current_range() {
            if self.cursor >= range.end {
                return None; // EOF for this range
            }
        } else {
            return None; // No more ranges
        }

        // Return current byte or None for EOF
        self.src.get(self.cursor).copied()
    }

    fn lookahead_n(&self, n: usize) -> Option<u8> {
        let range = self.current_range()?;
        let pos = self.cursor.checked_add(n)?;
        if pos >= range.end {
            return None;
        }
        self.src.get(pos).copied()
    }

    fn advance(&mut self, n: usize) {
        for _ in 0..n {
            // Check if we can advance within current range
            if let Some(range) = self.current_range() {
                if self.cursor >= range.end {
                    return; // Can't advance past range end
                }
            } else {
                return; // No more ranges
            }

            // Get current byte before advancing
            if let Some(&byte) = self.src.get(self.cursor) {
                // Handle CRLF sequences
                if byte == b'\r' {
                    if self.src.get(self.cursor + 1) == Some(&b'\n') {
                        // CRLF sequence - advance past both
                        self.cursor += 2;
                        self.row += 1;
                        self.col = 0;
                    } else {
                        // Just CR
                        self.cursor += 1;
                        self.row += 1;
                        self.col = 0;
                    }
                } else {
                    self.cursor += 1;
                    self.update_position(byte);
                }

                // Check if we need to move to next range
                if let Some(range) = self.current_range()
                    && self.cursor >= range.end
                    && self.ranges.next + 1 < self.ranges.spans.len()
                {
                    // Move to next range
                    self.ranges.next += 1;
                    if let Some(next_range) = self.ranges.spans.get(self.ranges.next) {
                        self.cursor = next_range.start;
                        // Recalculate position for new range
                        let (row, col) =
                            position_to_line_col(self.src, self.cursor, self.line_starts);
                        self.row = row;
                        self.col = col;
                    }
                }
            } else {
                return; // EOF
            }
        }
    }

    fn mark_end(&mut self) {
        self.mark_end = self.cursor;
    }

    fn column(&self) -> usize {
        self.col as usize
    }

    fn is_eof(&self) -> bool {
        if let Some(range) = self.current_range() {
            self.cursor >= range.end && self.ranges.next + 1 >= self.ranges.spans.len()
        } else {
            true
        }
    }
}

// Additional methods for extended functionality
impl<'a> TSLexerAdapter<'a> {
    /// Check if at start of an included range (for multi-file support)
    pub fn is_at_included_range_start(&self) -> bool {
        self.ranges
            .spans
            .get(self.ranges.next)
            .map(|r| r.start == self.cursor)
            .unwrap_or(false)
    }

    /// Get marked token length
    pub fn get_marked_length(&self) -> usize {
        self.mark_end
            .saturating_sub(self.cursor.saturating_sub(self.mark_end))
    }
}

/// Convert byte position to line/column
fn position_to_line_col(src: &[u8], pos: usize, line_starts: &[usize]) -> (u32, u32) {
    // Binary search for line containing position
    let line = line_starts
        .binary_search(&pos)
        .unwrap_or_else(|i| i.saturating_sub(1));
    let line_start = line_starts.get(line).copied().unwrap_or(0);

    // Count codepoints from line start to position for column
    let mut col = 0u32;
    for i in line_start..pos.min(src.len()) {
        if let Some(&byte) = src.get(i) {
            // Count non-continuation bytes as codepoints
            if (byte & 0b11000000) != 0b10000000 {
                col += 1;
            }
        }
    }

    (line as u32, col)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::external_scanner::Lexer;

    #[test]
    fn test_advance_crlf() {
        let input = b"hello\r\nworld";
        let line_starts = vec![0, 7]; // "hello\r\n" is 7 bytes
        let ranges = std::iter::once(0..input.len()).collect::<Vec<_>>();
        let mut adapter = TSLexerAdapter::new(input, 0, &line_starts, ranges);

        // Advance through "hello"
        for _ in 0..5 {
            adapter.advance(1);
        }
        assert_eq!(adapter.row, 0);
        assert_eq!(adapter.col, 5);

        // Advance through CRLF
        adapter.advance(1); // Should consume both \r\n
        assert_eq!(adapter.row, 1);
        assert_eq!(adapter.col, 0);

        // Verify we're at 'w'
        assert_eq!(adapter.lookahead(), Some(b'w'));
    }

    #[test]
    fn test_range_boundaries() {
        let input = b"hello world";
        let line_starts = vec![0];
        let ranges = vec![0..5, 6..11]; // Split at space
        let mut adapter = TSLexerAdapter::new(input, 0, &line_starts, ranges);

        // Advance to end of first range
        for _ in 0..5 {
            adapter.advance(1);
        }

        // After advancing 5 times from 0, we should be at position 5 (which is the space)
        // The first range is 0..5, so position 5 is the end (exclusive)
        // The adapter should automatically jump to the next range (starting at 6)
        assert_eq!(adapter.cursor, 6);

        // We're now at the start of the second range, pointing to 'w'
        assert_eq!(adapter.lookahead(), Some(b'w'));

        // Advance one more time
        adapter.advance(1);
        assert_eq!(adapter.cursor, 7); // Should be at 'o'
        assert_eq!(adapter.lookahead(), Some(b'o'));
    }

    #[test]
    fn test_is_at_included_range_start() {
        let input = b"hello world";
        let line_starts = vec![0];
        let ranges = vec![0..5, 6..11];
        let adapter = TSLexerAdapter::new(input, 0, &line_starts, ranges);

        assert!(adapter.is_at_included_range_start()); // At start of first range

        let adapter2 = TSLexerAdapter::new(input, 6, &line_starts, vec![0..5, 6..11]);
        assert!(adapter2.is_at_included_range_start()); // At start of second range
    }
}
