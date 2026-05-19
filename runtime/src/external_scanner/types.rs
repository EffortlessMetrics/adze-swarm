use crate::SymbolId;
use std::collections::HashSet;

/// Result of external scanning
#[derive(Debug, Clone, PartialEq)]
pub struct ScanResult {
    pub symbol: u16,
    pub length: usize,
}

/// External scanner state
#[derive(Debug, Clone)]
pub struct ExternalScannerState {
    /// Current state data (serialized)
    pub data: Vec<u8>,
}

impl Default for ExternalScannerState {
    fn default() -> Self {
        Self::new()
    }
}

impl ExternalScannerState {
    pub fn new() -> Self {
        ExternalScannerState { data: Vec::new() }
    }

    /// Serialize the state
    pub fn serialize(&self) -> &[u8] {
        &self.data
    }

    /// Deserialize from bytes
    pub fn deserialize(data: &[u8]) -> Self {
        ExternalScannerState {
            data: data.to_vec(),
        }
    }
}

/// Trait for external scanner lexing interaction
pub trait Lexer {
    /// Get the next byte at the current position
    fn lookahead(&self) -> Option<u8>;

    /// Advance the lexer by n bytes
    fn advance(&mut self, n: usize);

    /// Mark the end of the current token
    fn mark_end(&mut self);

    /// Get the current column position
    fn column(&self) -> usize;

    /// Check if at end of file
    fn is_eof(&self) -> bool;
}

/// Trait for implementing external scanners (object-safe)
pub trait ExternalScanner: Send + Sync {
    /// Scan for external tokens
    fn scan(&mut self, lexer: &mut dyn Lexer, valid_symbols: &[bool]) -> Option<ScanResult>;

    /// Serialize scanner state
    fn serialize(&self, buffer: &mut Vec<u8>);

    /// Deserialize scanner state
    fn deserialize(&mut self, buffer: &[u8]);
}

/// Type alias for dynamic external scanner
pub type DynExternalScanner = dyn ExternalScanner + Send + Sync;

/// Runtime for executing external scanners
pub struct ExternalScannerRuntime {
    /// Map of external token IDs to their valid symbols
    external_tokens: Vec<SymbolId>,
    /// Scanner state
    state: ExternalScannerState,
}

impl ExternalScannerRuntime {
    pub fn new(external_tokens: Vec<SymbolId>) -> Self {
        ExternalScannerRuntime {
            external_tokens,
            state: ExternalScannerState::new(),
        }
    }

    /// Get the external tokens
    pub fn get_external_tokens(&self) -> &[SymbolId] {
        &self.external_tokens
    }

    /// Reset the scanner state
    ///
    /// This clears any accumulated state and prepares the scanner for a fresh parse
    pub fn reset(&mut self) {
        self.state = ExternalScannerState::new();
    }

    /// Execute external scanner
    pub fn scan(
        &mut self,
        scanner: &mut DynExternalScanner,
        lexer: &mut dyn Lexer,
        valid_external_tokens: &HashSet<SymbolId>,
    ) -> Option<(SymbolId, usize)> {
        let valid_symbols: Vec<bool> = self
            .external_tokens
            .iter()
            .map(|token| valid_external_tokens.contains(token))
            .collect();

        scanner.deserialize(&self.state.data);

        if let Some(result) = scanner.scan(lexer, &valid_symbols) {
            let emitted_index = usize::from(result.symbol);
            let emitted_by_index =
                emitted_index < valid_symbols.len() && valid_symbols[emitted_index];
            let emitted_by_symbol_id = self
                .external_tokens
                .iter()
                .enumerate()
                .find_map(|(idx, token)| (*token == result.symbol).then_some(idx))
                .is_some_and(|idx| valid_symbols.get(idx) == Some(&true));

            if !emitted_by_index && !emitted_by_symbol_id {
                return None;
            }
            if result.length == 0 {
                return None;
            }
            self.state.data.clear();
            scanner.serialize(&mut self.state.data);

            return Some((result.symbol, result.length));
        }

        None
    }
}
