//! Source point and point-range value types.

use super::*;

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
