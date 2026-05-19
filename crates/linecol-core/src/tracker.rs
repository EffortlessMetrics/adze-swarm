/// Tracks a zero-based line index and the byte offset where that line starts.
///
/// # Examples
///
/// ```
/// use adze_linecol_core::LineCol;
///
/// let lc = LineCol::at_position(b"hello\nworld", 8);
/// assert_eq!(lc.line, 1);
/// assert_eq!(lc.column(8), 2);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LineCol {
    /// Zero-based line index.
    pub line: usize,
    /// Byte offset for the start of the current line.
    pub line_start: usize,
}

impl LineCol {
    /// Create a new tracker at line `0`, byte offset `0`.
    ///
    /// # Examples
    ///
    /// ```
    /// use adze_linecol_core::LineCol;
    ///
    /// let lc = LineCol::new();
    /// assert_eq!(lc.line, 0);
    /// assert_eq!(lc.line_start, 0);
    /// ```
    #[must_use]
    pub const fn new() -> Self {
        Self {
            line: 0,
            line_start: 0,
        }
    }

    /// Advance to a new line, setting the new line's starting byte offset.
    ///
    /// # Examples
    ///
    /// ```
    /// use adze_linecol_core::LineCol;
    ///
    /// let mut lc = LineCol::new();
    /// lc.advance_line(5);
    /// assert_eq!(lc.line, 1);
    /// assert_eq!(lc.line_start, 5);
    /// ```
    pub fn advance_line(&mut self, new_line_start: usize) {
        self.line += 1;
        self.line_start = new_line_start;
    }

    /// Compute a byte-based column for `position`.
    ///
    /// # Examples
    ///
    /// ```
    /// use adze_linecol_core::LineCol;
    ///
    /// let lc = LineCol::at_position(b"ab\ncd", 3);
    /// assert_eq!(lc.column(3), 0); // start of line
    /// assert_eq!(lc.column(4), 1); // one byte into line
    /// ```
    #[must_use]
    pub fn column(&self, position: usize) -> usize {
        position.saturating_sub(self.line_start)
    }
}

impl std::fmt::Display for LineCol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "line {}, col {}", self.line, self.line_start)
    }
}

impl Default for LineCol {
    fn default() -> Self {
        Self::new()
    }
}
