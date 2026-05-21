//! Pure Rust external scanner implementation.
#![cfg_attr(feature = "strict_docs", allow(missing_docs))]

// External scanner support for pure-Rust parser
use std::collections::HashMap;
use std::ffi::c_void;

/// External scanner state
pub const MAX_EXTERNAL_SCANNER_STATE_LENGTH: usize = 32;

/// External scanner interface for custom lexing
pub trait ExternalScanner: Send + Sync {
    /// Scan for a token
    fn scan(&mut self, lexer: &mut Lexer, valid_symbols: &[bool]) -> bool;

    /// Serialize scanner state
    fn serialize(&self, _buffer: &mut [u8]) -> usize {
        0 // Default: no state
    }

    /// Deserialize scanner state
    fn deserialize(&mut self, _buffer: &[u8]) {
        // Default: ignore state
    }
}

/// Lexer interface for external scanners
pub struct Lexer<'a> {
    input: &'a [u8],
    position: usize,
    token_end: usize,
    result_symbol: u16,
}

impl<'a> Lexer<'a> {
    /// Create a new lexer
    pub fn new(input: &'a [u8], position: usize) -> Self {
        Lexer {
            input,
            position,
            token_end: position,
            result_symbol: 0,
        }
    }

    /// Advance the lexer by one character
    pub fn advance(&mut self, skip: bool) -> Option<u8> {
        if self.position < self.input.len() {
            let ch = self.input[self.position];
            self.position += 1;

            if !skip {
                self.token_end = self.position;
            }

            Some(ch)
        } else {
            None
        }
    }

    /// Skip whitespace
    pub fn skip_whitespace(&mut self) {
        while self.position < self.input.len() {
            match self.input[self.position] {
                b' ' | b'\t' | b'\r' | b'\n' => {
                    self.position += 1;
                }
                _ => break,
            }
        }
        self.token_end = self.position;
    }

    /// Mark the end of a token
    pub fn mark_end(&mut self) {
        self.token_end = self.position;
    }

    /// Get the current column
    pub fn get_column(&self) -> usize {
        // Count from last newline
        let mut column = 0;
        for i in (0..self.position).rev() {
            if self.input[i] == b'\n' {
                break;
            }
            column += 1;
        }
        column
    }

    /// Check if at end of input
    pub fn eof(&self) -> bool {
        self.position >= self.input.len()
    }

    /// Peek at the next character
    pub fn lookahead(&self) -> Option<u8> {
        if self.position < self.input.len() {
            Some(self.input[self.position])
        } else {
            None
        }
    }

    /// Set the result symbol
    pub fn result(&mut self, symbol: u16) {
        self.result_symbol = symbol;
    }

    /// Get the token length
    pub fn token_length(&self) -> usize {
        self.token_end - (self.position - self.token_end)
    }
}

/// Registry for external scanners
pub struct ExternalScannerRegistry {
    scanners: HashMap<String, Box<dyn ExternalScanner>>,
}

impl Default for ExternalScannerRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ExternalScannerRegistry {
    /// Create a new registry
    pub fn new() -> Self {
        ExternalScannerRegistry {
            scanners: HashMap::new(),
        }
    }

    /// Register an external scanner
    pub fn register(&mut self, name: String, scanner: Box<dyn ExternalScanner>) {
        self.scanners.insert(name, scanner);
    }

    /// Get a scanner by name
    pub fn get(&self, name: &str) -> Option<&dyn ExternalScanner> {
        self.scanners.get(name).map(|s| s.as_ref())
    }

    /// Get a mutable scanner by name
    pub fn get_mut(&mut self, name: &str) -> Option<&mut (dyn ExternalScanner + 'static)> {
        self.scanners.get_mut(name).map(|s| s.as_mut())
    }
}

/// C FFI bridge for external scanners
pub mod ffi {
    use super::*;
    use std::slice;

    /// Create a scanner instance
    pub unsafe extern "C" fn external_scanner_create() -> *mut c_void {
        let registry = Box::new(ExternalScannerRegistry::new());
        Box::into_raw(registry) as *mut c_void
    }

    /// Destroy a scanner instance
    pub unsafe extern "C" fn external_scanner_destroy(scanner: *mut c_void) {
        if !scanner.is_null() {
            unsafe {
                let _ = Box::from_raw(scanner as *mut ExternalScannerRegistry);
            }
        }
    }

    /// Scan for a token
    pub unsafe extern "C" fn external_scanner_scan(
        scanner: *mut c_void,
        lexer: *mut c_void,
        valid_symbols: *const bool,
        valid_symbol_count: u32,
    ) -> bool {
        if scanner.is_null() || lexer.is_null() || valid_symbols.is_null() {
            return false;
        }

        let _registry = unsafe { &mut *(scanner as *mut ExternalScannerRegistry) };
        let _valid_symbols =
            unsafe { slice::from_raw_parts(valid_symbols, valid_symbol_count as usize) };

        // In a real implementation, this would:
        // 1. Cast lexer to the appropriate type
        // 2. Call the appropriate scanner based on valid_symbols
        // 3. Return whether a token was found

        false
    }

    /// Serialize scanner state
    pub unsafe extern "C" fn external_scanner_serialize(
        scanner: *mut c_void,
        buffer: *mut u8,
        buffer_size: u32,
    ) -> u32 {
        if scanner.is_null() || buffer.is_null() {
            return 0;
        }

        let _registry = unsafe { &*(scanner as *mut ExternalScannerRegistry) };
        let _buffer = unsafe { slice::from_raw_parts_mut(buffer, buffer_size as usize) };

        // In a real implementation, serialize the current scanner state
        0
    }

    /// Deserialize scanner state
    pub unsafe extern "C" fn external_scanner_deserialize(
        scanner: *mut c_void,
        buffer: *const u8,
        length: u32,
    ) {
        if scanner.is_null() || buffer.is_null() {
            return;
        }

        let _registry = unsafe { &mut *(scanner as *mut ExternalScannerRegistry) };
        let _buffer = unsafe { slice::from_raw_parts(buffer, length as usize) };

        // In a real implementation, deserialize the scanner state
    }
}

/// Example: String scanner for handling multi-line strings
pub struct StringScanner {
    delimiter: Option<String>,
}

impl Default for StringScanner {
    fn default() -> Self {
        Self::new()
    }
}

impl StringScanner {
    pub fn new() -> Self {
        StringScanner { delimiter: None }
    }
}

impl ExternalScanner for StringScanner {
    fn scan(&mut self, lexer: &mut Lexer, valid_symbols: &[bool]) -> bool {
        // Example: Scan for triple-quoted strings
        const STRING_START: usize = 0;
        const STRING_CONTENT: usize = 1;
        const STRING_END: usize = 2;

        if symbol_is_valid(valid_symbols, STRING_START)
            && self.scan_string_start(lexer, STRING_START as u16)
        {
            return true;
        }

        if symbol_is_valid(valid_symbols, STRING_END)
            && let Some(delim) = &self.delimiter
            && self.scan_string_end(lexer, delim, STRING_END as u16)
        {
            self.delimiter = None;
            return true;
        }

        if symbol_is_valid(valid_symbols, STRING_CONTENT)
            && self.delimiter.is_some()
            && self.scan_string_content(lexer, STRING_CONTENT as u16)
        {
            return true;
        }

        false
    }

    fn serialize(&self, buffer: &mut [u8]) -> usize {
        if let Some(delim) = &self.delimiter {
            let bytes = delim.as_bytes();
            let len = bytes.len().min(buffer.len());
            buffer[..len].copy_from_slice(&bytes[..len]);
            len
        } else {
            0
        }
    }

    fn deserialize(&mut self, buffer: &[u8]) {
        if !buffer.is_empty() {
            self.delimiter = Some(String::from_utf8_lossy(buffer).to_string());
        } else {
            self.delimiter = None;
        }
    }
}

impl StringScanner {
    fn scan_string_start(&mut self, lexer: &mut Lexer, symbol: u16) -> bool {
        if !advance_if_matches(lexer, b'"') {
            return false;
        }
        if !advance_if_matches(lexer, b'"') {
            return false;
        }
        if !advance_if_matches(lexer, b'"') {
            return false;
        }

        lexer.mark_end();
        lexer.result(symbol);
        self.delimiter = Some("\"\"\"".to_string());
        true
    }

    fn scan_string_end(&self, lexer: &mut Lexer, delimiter: &str, symbol: u16) -> bool {
        for &byte in delimiter.as_bytes() {
            if !advance_if_matches(lexer, byte) {
                return false;
            }
        }

        lexer.mark_end();
        lexer.result(symbol);
        true
    }

    fn scan_string_content(&self, lexer: &mut Lexer, symbol: u16) -> bool {
        while !lexer.eof() {
            if lexer.lookahead() == Some(b'"') {
                break;
            }
            lexer.advance(false);
        }

        if lexer.token_length() > 0 {
            lexer.result(symbol);
            return true;
        }

        false
    }
}

fn symbol_is_valid(valid_symbols: &[bool], index: usize) -> bool {
    valid_symbols.get(index).copied().unwrap_or(false)
}

fn advance_if_matches(lexer: &mut Lexer, expected: u8) -> bool {
    if lexer.lookahead() != Some(expected) {
        return false;
    }
    lexer.advance(false);
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lexer_advance() {
        let input = b"hello world";
        let mut lexer = Lexer::new(input, 0);

        assert_eq!(lexer.advance(false), Some(b'h'));
        assert_eq!(lexer.advance(false), Some(b'e'));
        assert_eq!(lexer.position, 2);
        assert_eq!(lexer.token_end, 2);
    }

    #[test]
    fn test_lexer_skip() {
        let input = b"  \t\nhello";
        let mut lexer = Lexer::new(input, 0);

        lexer.skip_whitespace();
        assert_eq!(lexer.position, 4);
        assert_eq!(lexer.lookahead(), Some(b'h'));
    }

    #[test]
    fn test_string_scanner() {
        let mut scanner = StringScanner::new();
        let input = b"\"\"\"hello world\"\"\"";
        let mut lexer = Lexer::new(input, 0);

        // Test scanning start
        let valid_symbols = vec![true, false, false]; // STRING_START
        assert!(scanner.scan(&mut lexer, &valid_symbols));
        assert_eq!(lexer.result_symbol, 0);

        // Test serialization
        let mut buffer = vec![0u8; 32];
        let len = scanner.serialize(&mut buffer);
        assert_eq!(&buffer[..len], b"\"\"\"");
    }
}
