//! External scanner support for custom lexing logic

/// Result of scanning for an external token
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScanResult {
    /// The token type that was found (index into external_tokens array)
    pub token_type: u32,
    /// Number of bytes consumed
    pub bytes_consumed: usize,
}

/// Trait for external scanners (pure Rust version)
pub trait ExternalScanner: Send + Sync {
    /// Initialize the scanner
    fn init(&mut self);

    /// Scan for a token
    ///
    /// # Arguments
    /// * `valid_symbols` - Bitset of valid external tokens at this position
    /// * `input` - Input bytes available for scanning
    ///
    /// # Returns
    /// * `Some(ScanResult)` if a token was found
    /// * `None` if no external token matches
    fn scan(&mut self, valid_symbols: &[bool], input: &[u8]) -> Option<ScanResult>;

    /// Serialize scanner state for incremental parsing
    fn serialize(&self) -> Vec<u8>;

    /// Deserialize scanner state for incremental parsing
    fn deserialize(&mut self, data: &[u8]);
}

/// FFI-compatible external scanner interface
#[cfg(feature = "external_scanners")]
#[repr(C)]
pub struct TSExternalScanner {
    /// Private data pointer
    pub data: *mut std::os::raw::c_void,
    /// Function pointers for scanner operations
    pub vtable: TSExternalScannerVTable,
}

#[cfg(feature = "external_scanners")]
#[repr(C)]
/// Function pointers bridging a Rust scanner to the C ABI expected by Tree-sitter.
pub struct TSExternalScannerVTable {
    /// Create a new scanner instance
    pub create: unsafe extern "C" fn() -> *mut std::os::raw::c_void,
    /// Destroy a scanner instance
    pub destroy: unsafe extern "C" fn(*mut std::os::raw::c_void),
    /// Scan for a token
    pub scan: unsafe extern "C" fn(
        *mut std::os::raw::c_void,
        *const u32,  // lexer
        *const bool, // valid_symbols
    ) -> bool,
    /// Serialize scanner state
    pub serialize: unsafe extern "C" fn(
        *const std::os::raw::c_void,
        *mut u8, // buffer
    ) -> u32, // bytes written
    /// Deserialize scanner state
    pub deserialize: unsafe extern "C" fn(
        *mut std::os::raw::c_void,
        *const u8, // buffer
        u32,       // length
    ),
}

/// Example external scanner for indentation-based languages
#[cfg(test)]
pub struct IndentationScanner {
    indent_stack: Vec<u32>,
}

#[cfg(test)]
impl ExternalScanner for IndentationScanner {
    fn init(&mut self) {
        self.indent_stack.clear();
        self.indent_stack.push(0);
    }

    fn scan(&mut self, _valid_symbols: &[bool], input: &[u8]) -> Option<ScanResult> {
        // Simple example: count leading spaces
        let indent = input.iter().take_while(|&&b| b == b' ').count() as u32;

        if indent > *self.indent_stack.last()? {
            // INDENT token
            self.indent_stack.push(indent);
            Some(ScanResult {
                token_type: 0, // INDENT
                bytes_consumed: 0,
            })
        } else if indent < *self.indent_stack.last()? {
            // DEDENT token(s)
            while self.indent_stack.len() > 1 && indent < *self.indent_stack.last()? {
                self.indent_stack.pop();
            }
            Some(ScanResult {
                token_type: 1, // DEDENT
                bytes_consumed: 0,
            })
        } else {
            None
        }
    }

    fn serialize(&self) -> Vec<u8> {
        // Serialize indent stack
        let mut data = Vec::new();
        data.extend_from_slice(&(self.indent_stack.len() as u32).to_le_bytes());
        for &indent in &self.indent_stack {
            data.extend_from_slice(&indent.to_le_bytes());
        }
        data
    }

    fn deserialize(&mut self, data: &[u8]) {
        // Deserialize indent stack
        if data.len() >= 4 {
            let len = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;
            self.indent_stack.clear();
            for i in 0..len {
                let offset = 4 + i * 4;
                if offset + 4 <= data.len() {
                    let indent = u32::from_le_bytes([
                        data[offset],
                        data[offset + 1],
                        data[offset + 2],
                        data[offset + 3],
                    ]);
                    self.indent_stack.push(indent);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh_scanner() -> IndentationScanner {
        let mut s = IndentationScanner {
            indent_stack: Vec::new(),
        };
        s.init();
        s
    }

    #[test]
    fn scan_result_derives_equality_and_copy() {
        let a = ScanResult {
            token_type: 0,
            bytes_consumed: 4,
        };
        let b = a; // Copy
        assert_eq!(a, b);
        let c = ScanResult {
            token_type: 1,
            bytes_consumed: 4,
        };
        assert_ne!(a, c);
    }

    #[test]
    fn scan_result_debug_includes_fields() {
        let r = ScanResult {
            token_type: 7,
            bytes_consumed: 9,
        };
        let dbg = format!("{:?}", r);
        assert!(dbg.contains("token_type"), "debug output: {}", dbg);
        assert!(dbg.contains('7'), "debug output: {}", dbg);
        assert!(dbg.contains("bytes_consumed"), "debug output: {}", dbg);
        assert!(dbg.contains('9'), "debug output: {}", dbg);
    }

    #[test]
    fn init_resets_stack_to_zero_baseline() {
        let mut s = IndentationScanner {
            indent_stack: vec![1, 2, 3],
        };
        s.init();
        assert_eq!(s.indent_stack, vec![0]);
    }

    #[test]
    fn scan_equal_indent_returns_none() {
        let mut s = fresh_scanner();
        // No leading spaces matches the baseline 0.
        assert!(s.scan(&[], b"foo").is_none());
        assert_eq!(s.indent_stack, vec![0]);
    }

    #[test]
    fn scan_increased_indent_emits_indent_token() {
        let mut s = fresh_scanner();
        let result = s.scan(&[], b"    foo").expect("indent emitted");
        assert_eq!(result.token_type, 0); // INDENT
        assert_eq!(result.bytes_consumed, 0);
        assert_eq!(s.indent_stack, vec![0, 4]);
    }

    #[test]
    fn scan_decreased_indent_emits_dedent_and_pops() {
        let mut s = fresh_scanner();
        // First indent to depth 4.
        let _ = s.scan(&[], b"    a").unwrap();
        // Then indent further to depth 8.
        let _ = s.scan(&[], b"        b").unwrap();
        assert_eq!(s.indent_stack, vec![0, 4, 8]);
        // Now dedent back to 0.
        let dedent = s.scan(&[], b"x").expect("dedent emitted");
        assert_eq!(dedent.token_type, 1); // DEDENT
        assert_eq!(dedent.bytes_consumed, 0);
        assert_eq!(s.indent_stack, vec![0]);
    }

    #[test]
    fn scan_partial_dedent_keeps_intermediate_level() {
        let mut s = fresh_scanner();
        let _ = s.scan(&[], b"  a").unwrap(); // indent to 2
        let _ = s.scan(&[], b"    b").unwrap(); // indent to 4
        assert_eq!(s.indent_stack, vec![0, 2, 4]);
        // Dedent to 2 should pop only the top level.
        let dedent = s.scan(&[], b"  c").expect("dedent emitted");
        assert_eq!(dedent.token_type, 1);
        assert_eq!(s.indent_stack, vec![0, 2]);
    }

    #[test]
    fn scan_empty_input_returns_none_after_init() {
        let mut s = fresh_scanner();
        // Empty input means 0 indent, matches baseline 0, returns None.
        assert!(s.scan(&[], b"").is_none());
    }

    #[test]
    fn serialize_empty_stack_records_zero_length() {
        let s = IndentationScanner {
            indent_stack: Vec::new(),
        };
        let data = s.serialize();
        assert_eq!(data.len(), 4);
        let len = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        assert_eq!(len, 0);
    }

    #[test]
    fn serialize_baseline_encodes_one_zero_entry() {
        let s = fresh_scanner();
        let data = s.serialize();
        assert_eq!(data.len(), 4 + 4);
        let len = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        assert_eq!(len, 1);
        let first = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
        assert_eq!(first, 0);
    }

    #[test]
    fn deserialize_short_buffer_is_noop() {
        let mut s = IndentationScanner {
            indent_stack: vec![0, 4],
        };
        s.deserialize(&[]);
        // Less than 4 bytes: stack untouched.
        assert_eq!(s.indent_stack, vec![0, 4]);
        s.deserialize(&[0x01, 0x02, 0x03]);
        assert_eq!(s.indent_stack, vec![0, 4]);
    }

    #[test]
    fn deserialize_clears_then_restores_levels() {
        let mut original = fresh_scanner();
        let _ = original.scan(&[], b"  a").unwrap();
        let _ = original.scan(&[], b"    b").unwrap();
        assert_eq!(original.indent_stack, vec![0, 2, 4]);
        let bytes = original.serialize();

        let mut other = IndentationScanner {
            indent_stack: vec![99, 100],
        };
        other.deserialize(&bytes);
        assert_eq!(other.indent_stack, vec![0, 2, 4]);
    }

    #[test]
    fn roundtrip_serialize_then_deserialize_preserves_stack() {
        let mut s = fresh_scanner();
        let _ = s.scan(&[], b"  a").unwrap();
        let _ = s.scan(&[], b"      b").unwrap();
        let bytes = s.serialize();

        let mut restored = IndentationScanner {
            indent_stack: Vec::new(),
        };
        restored.deserialize(&bytes);
        assert_eq!(restored.indent_stack, s.indent_stack);
    }

    #[test]
    fn deserialize_truncated_entries_stops_safely() {
        // Encode header claiming 3 entries but only provide bytes for 1.
        let mut data = Vec::new();
        data.extend_from_slice(&3u32.to_le_bytes());
        data.extend_from_slice(&42u32.to_le_bytes());
        // Only 4 bytes of body — second/third entries are missing.
        let mut s = IndentationScanner {
            indent_stack: vec![0],
        };
        s.deserialize(&data);
        // Should have cleared and pushed only the present entry.
        assert_eq!(s.indent_stack, vec![42]);
    }

    #[test]
    fn scan_returns_none_when_stack_is_empty() {
        // Without init(), indent_stack is empty: `last()` is None,
        // so the `?` short-circuits and scan returns None.
        let mut s = IndentationScanner {
            indent_stack: Vec::new(),
        };
        assert!(s.scan(&[], b"  a").is_none());
        assert!(s.indent_stack.is_empty());
    }
}
