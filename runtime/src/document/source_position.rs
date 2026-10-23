//! Source point and point-range primitives for native document views.

use std::ops::Range;

/// A zero-based source point in a native parse document.
///
/// Columns are byte offsets within a row, matching Tree-sitter's point model.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DocumentPoint {
    /// Zero-based row number.
    pub row: u32,
    /// Zero-based byte column within the row.
    pub column: u32,
}

impl DocumentPoint {
    /// Compute a document point from a byte offset.
    ///
    /// Out-of-range byte offsets are clamped to the end of `source`.
    #[must_use]
    pub fn from_byte_offset(source: &str, byte: usize) -> Self {
        let end = byte.min(source.len());
        let mut row = 0u32;
        let mut column = 0u32;

        for &source_byte in &source.as_bytes()[..end] {
            if source_byte == b'\n' {
                row = row.saturating_add(1);
                column = 0;
            } else {
                column = column.saturating_add(1);
            }
        }

        Self { row, column }
    }
}

/// A zero-based source point range in a native parse document.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PointRange {
    /// Inclusive start point.
    pub start: DocumentPoint,
    /// Exclusive end point.
    pub end: DocumentPoint,
}

impl PointRange {
    /// Compute a point range from a byte range.
    #[must_use]
    pub fn from_byte_range(source: &str, range: Range<usize>) -> Self {
        Self {
            start: DocumentPoint::from_byte_offset(source, range.start),
            end: DocumentPoint::from_byte_offset(source, range.end),
        }
    }
}

/// Source-position lookup for a single immutable source snapshot.
///
/// Only LF starts a new row, as in `DocumentPoint::from_byte_offset`. Store
/// byte positions immediately after LF; the first row starts implicitly at 0.
/// Construction scans the source once. Queries do not retain or read source
/// bytes and take logarithmic work in the number of line breaks.
pub(super) struct SourceLineIndex {
    source_len: usize,
    line_starts: Vec<usize>,
}

#[cfg(test)]
std::thread_local! {
    static INDEX_BUILDS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static INDEXED_BYTES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
pub(super) fn index_build_stats() -> (usize, usize) {
    (
        INDEX_BUILDS.with(|n| n.get()),
        INDEXED_BYTES.with(|n| n.get()),
    )
}

impl SourceLineIndex {
    pub(super) fn new(source: &str) -> Self {
        #[cfg(test)]
        INDEX_BUILDS.with(|n| n.set(n.get().saturating_add(1)));

        let bytes = source.bytes().enumerate();
        #[cfg(test)]
        let bytes = bytes.inspect(|_| {
            INDEXED_BYTES.with(|n| n.set(n.get().saturating_add(1)));
        });
        let line_starts = bytes
            .filter(|&(_, value)| value == b'\n')
            .map(|(byte, _)| byte + 1)
            .collect();
        Self {
            source_len: source.len(),
            line_starts,
        }
    }

    fn point(&self, byte: usize) -> DocumentPoint {
        let end = byte.min(self.source_len);
        let row = self.line_starts.partition_point(|&start| start <= end);
        let line_start = row
            .checked_sub(1)
            .and_then(|previous| self.line_starts.get(previous))
            .copied()
            .unwrap_or(0);
        DocumentPoint {
            row: saturating_position(row),
            column: saturating_position(end - line_start),
        }
    }

    pub(super) fn point_range(&self, range: Range<usize>) -> PointRange {
        PointRange {
            start: self.point(range.start),
            end: self.point(range.end),
        }
    }
}

fn saturating_position(value: usize) -> u32 {
    value.min(u32::MAX as usize) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_line_index_literal_offsets_preserve_byte_column_contract() {
        // Byte positions 4 and 7..10 include positions inside multibyte text.
        let index = SourceLineIndex::new("a\r\né\t🙂\n");
        let expected = [
            (0, 0),
            (0, 1),
            (0, 2),
            (1, 0),
            (1, 1),
            (1, 2),
            (1, 3),
            (1, 4),
            (1, 5),
            (1, 6),
            (1, 7),
            (2, 0),
        ];
        for (byte, (row, column)) in expected.into_iter().enumerate() {
            assert_eq!(index.point(byte), DocumentPoint { row, column });
        }
        assert_eq!(index.point(usize::MAX), DocumentPoint { row: 2, column: 0 });
    }

    #[test]
    fn test_line_index_empty_cr_nul_and_final_lf_have_literal_points() {
        for (source, expected) in [
            ("", DocumentPoint { row: 0, column: 0 }),
            ("\r", DocumentPoint { row: 0, column: 1 }),
            ("\0\t", DocumentPoint { row: 0, column: 2 }),
            ("\n", DocumentPoint { row: 1, column: 0 }),
            ("\n\n", DocumentPoint { row: 2, column: 0 }),
            ("x\r\ny", DocumentPoint { row: 1, column: 1 }),
        ] {
            let index = SourceLineIndex::new(source);
            assert_eq!(index.point(0), DocumentPoint::default());
            assert_eq!(index.point(source.len()), expected, "{source:?}");
            assert_eq!(index.point(usize::MAX), expected, "{source:?}");
        }
    }

    #[test]
    fn test_line_index_all_short_text_offsets_match_linear_reference() {
        fn check(source: &str) {
            let index = SourceLineIndex::new(source);
            for byte in (0..=source.len() + 1).chain(std::iter::once(usize::MAX)) {
                assert_eq!(
                    index.point(byte),
                    DocumentPoint::from_byte_offset(source, byte),
                    "source={source:?}, byte={byte}"
                );
            }
            assert_eq!(
                index.line_starts.len(),
                source.bytes().filter(|&byte| byte == b'\n').count()
            );
        }

        let mut texts = vec![String::new()];
        check("");
        for _ in 0..4 {
            let mut next = Vec::new();
            for prefix in &texts {
                for suffix in ["a", "\n", "\r", "\t", "é", "🙂", "\0"] {
                    let text = format!("{prefix}{suffix}");
                    check(&text);
                    next.push(text);
                }
            }
            texts = next;
        }
    }

    #[test]
    fn test_line_index_ranges_preserve_endpoint_clamping() {
        let source = "a\né";
        let index = SourceLineIndex::new(source);
        // Do not normalize an inverted caller range or require UTF-8 boundaries.
        for range in [
            0..0,
            0..4,
            2..3,
            Range { start: 4, end: 2 },
            3..usize::MAX,
            usize::MAX..usize::MAX,
        ] {
            assert_eq!(
                index.point_range(range.clone()),
                PointRange::from_byte_range(source, range)
            );
        }
    }

    #[test]
    fn test_line_index_saturation_preserves_u32_limits() {
        assert_eq!(saturating_position(0), 0);
        let limit = u32::MAX as usize;
        assert_eq!(saturating_position(limit), u32::MAX);
        if let Some(over_limit) = limit.checked_add(1) {
            assert_eq!(saturating_position(over_limit), u32::MAX);
        }
    }
}
