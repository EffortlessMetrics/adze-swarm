use crate::LineCol;

impl LineCol {
    /// Compute line metadata for a byte position in `input`.
    ///
    /// If `position` is beyond `input.len()`, the end of input is used.
    ///
    /// # Examples
    ///
    /// ```
    /// use adze_linecol_core::LineCol;
    ///
    /// let lc = LineCol::at_position(b"hello\nworld\n", 6);
    /// assert_eq!(lc.line, 1);
    /// assert_eq!(lc.line_start, 6);
    /// assert_eq!(lc.column(8), 2);
    /// ```
    #[must_use]
    pub fn at_position(input: &[u8], position: usize) -> Self {
        let mut tracker = Self::new();
        let end = position.min(input.len());

        for i in 0..end {
            if input[i] == b'\n' {
                tracker.advance_line(i + 1);
            } else if input[i] == b'\r' {
                // CRLF is counted on the LF byte, not the CR byte.
                if i + 1 < input.len() && input[i + 1] == b'\n' {
                    continue;
                }
                tracker.advance_line(i + 1);
            }
        }

        tracker
    }

    /// Process one byte while scanning a stream and update line metadata.
    ///
    /// Returns `true` if the byte advanced to a new line.
    ///
    /// Note: for CRLF, this returns `false` for the CR byte and `true` for the LF byte.
    ///
    /// # Examples
    ///
    /// ```
    /// use adze_linecol_core::LineCol;
    ///
    /// let mut lc = LineCol::new();
    /// assert!(!lc.process_byte(b'a', None, 0));
    /// assert!(lc.process_byte(b'\n', None, 1));
    /// assert_eq!(lc.line, 1);
    /// assert_eq!(lc.line_start, 2);
    /// ```
    pub fn process_byte(&mut self, byte: u8, next_byte: Option<u8>, current_offset: usize) -> bool {
        match byte {
            b'\n' => {
                self.advance_line(current_offset + 1);
                true
            }
            b'\r' => {
                if next_byte == Some(b'\n') {
                    false
                } else {
                    self.advance_line(current_offset + 1);
                    true
                }
            }
            _ => false,
        }
    }
}
