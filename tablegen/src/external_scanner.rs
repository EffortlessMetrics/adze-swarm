#![cfg_attr(feature = "strict_docs", allow(missing_docs))]
//! External scanner code generation for Tree-sitter.

use adze_ir::{ExternalToken, Grammar, SymbolId};
use quote::quote;
use std::collections::HashMap;

/// Generates external scanner data and interface for Tree-sitter
pub struct ExternalScannerGenerator {
    #[allow(dead_code)]
    grammar: Grammar,
    external_tokens: Vec<ExternalToken>,
    /// Maps symbol IDs to their indices in the external scanner
    #[allow(dead_code)]
    symbol_map: HashMap<SymbolId, usize>,
}

impl ExternalScannerGenerator {
    pub fn new(grammar: Grammar) -> Self {
        let external_tokens = grammar.externals.clone();
        let mut symbol_map = HashMap::new();

        for (index, token) in external_tokens.iter().enumerate() {
            symbol_map.insert(token.symbol_id, index);
        }

        Self {
            grammar,
            external_tokens,
            symbol_map,
        }
    }

    /// Generates the external scanner state bitmap
    /// Each state has a boolean array indicating which external tokens are valid
    pub fn generate_state_bitmap(&self, state_count: usize) -> Vec<Vec<bool>> {
        // For now, return a simple bitmap where all external tokens are valid in all states
        // TODO: This needs to be computed from the parse table
        let external_count = self.external_tokens.len();
        vec![vec![true; external_count]; state_count]
    }

    /// Generates the symbol map array that maps external scanner indices to symbol IDs
    pub fn generate_symbol_map(&self) -> Vec<u16> {
        let mut map = vec![0u16; self.external_tokens.len()];

        for (token_index, token) in self.external_tokens.iter().enumerate() {
            map[token_index] = token.symbol_id.0;
        }

        map
    }

    /// Generates the external scanner FFI interface code
    pub fn generate_scanner_interface(&self) -> proc_macro2::TokenStream {
        if self.external_tokens.is_empty() {
            return quote! {};
        }

        // Generate external scanner state data
        let state_bitmap = self.generate_state_bitmap(100); // TODO: Get actual state count
        let mut state_data = Vec::new();

        for state in &state_bitmap {
            for &valid in state {
                state_data.push(valid);
            }
        }

        // Generate symbol map
        let symbol_map = self.generate_symbol_map();

        quote! {
            // External scanner state bitmap
            static EXTERNAL_SCANNER_STATES: &[bool] = &[#(#state_data),*];

            // External scanner symbol map
            static EXTERNAL_SCANNER_SYMBOL_MAP: &[u16] = &[#(#symbol_map),*];

            // External scanner data
            #[allow(dead_code)]
            static EXTERNAL_SCANNER_DATA: adze::ffi::TSExternalScannerData = adze::ffi::TSExternalScannerData {
                states: EXTERNAL_SCANNER_STATES.as_ptr(),
                symbol_map: EXTERNAL_SCANNER_SYMBOL_MAP.as_ptr(),
                create: None, // TODO: Link to user scanner
                destroy: None,
                scan: None,
                serialize: None,
                deserialize: None,
            };
        }
    }

    /// Returns whether the grammar has external tokens
    pub fn has_external_tokens(&self) -> bool {
        !self.external_tokens.is_empty()
    }

    /// Returns the number of external tokens
    pub fn external_token_count(&self) -> usize {
        self.external_tokens.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_external_scanner_empty() {
        let grammar = Grammar::new("test".to_string());
        let generator = ExternalScannerGenerator::new(grammar);

        assert!(!generator.has_external_tokens());
        assert_eq!(generator.external_token_count(), 0);
        let interface = generator.generate_scanner_interface();
        assert_eq!(interface.to_string(), "");
    }

    #[test]
    fn test_external_scanner_with_tokens() {
        let mut grammar = Grammar::new("test".to_string());

        // Add some external tokens
        grammar.externals.push(ExternalToken {
            name: "HEREDOC".to_string(),
            symbol_id: SymbolId(100),
        });

        grammar.externals.push(ExternalToken {
            name: "TEMPLATE_STRING".to_string(),
            symbol_id: SymbolId(101),
        });

        let generator = ExternalScannerGenerator::new(grammar);

        assert!(generator.has_external_tokens());
        assert_eq!(generator.external_token_count(), 2);

        let symbol_map = generator.generate_symbol_map();
        assert_eq!(symbol_map, vec![100, 101]);

        let interface = generator.generate_scanner_interface();
        let interface_str = interface.to_string();
        assert!(interface_str.contains("EXTERNAL_SCANNER_STATES"));
        assert!(interface_str.contains("EXTERNAL_SCANNER_SYMBOL_MAP"));
        assert!(interface_str.contains("TSExternalScannerData"));
    }

    #[test]
    fn test_state_bitmap_generation() {
        let mut grammar = Grammar::new("test".to_string());

        grammar.externals.push(ExternalToken {
            name: "TOKEN1".to_string(),
            symbol_id: SymbolId(200),
        });

        grammar.externals.push(ExternalToken {
            name: "TOKEN2".to_string(),
            symbol_id: SymbolId(201),
        });

        let generator = ExternalScannerGenerator::new(grammar);
        let bitmap = generator.generate_state_bitmap(3); // 3 states

        assert_eq!(bitmap.len(), 3); // 3 states
        assert_eq!(bitmap[0].len(), 2); // 2 external tokens

        // Currently all tokens are valid in all states
        assert!(bitmap[0][0] && bitmap[0][1]);
        assert!(bitmap[1][0] && bitmap[1][1]);
        assert!(bitmap[2][0] && bitmap[2][1]);
    }

    // ---------------------------------------------------------------------
    // Additional coverage: edge cases, every public branch, and
    // serialization-equivalent round trips through the TokenStream output.
    // Tests below are pure unit tests — no production changes.
    // ---------------------------------------------------------------------

    fn token(name: &str, id: u16) -> ExternalToken {
        ExternalToken {
            name: name.to_string(),
            symbol_id: SymbolId(id),
        }
    }

    /// Empty grammar: `generate_symbol_map` returns an empty vector.
    #[test]
    fn test_generate_symbol_map_empty() {
        let grammar = Grammar::new("empty".to_string());
        let generator = ExternalScannerGenerator::new(grammar);

        let map = generator.generate_symbol_map();
        assert!(map.is_empty());
        assert_eq!(map.len(), generator.external_token_count());
    }

    /// `generate_state_bitmap` with `state_count = 0` returns an empty outer
    /// vector regardless of how many external tokens exist.
    #[test]
    fn test_state_bitmap_zero_states() {
        let mut grammar = Grammar::new("g".to_string());
        grammar.externals.push(token("A", 1));
        grammar.externals.push(token("B", 2));

        let generator = ExternalScannerGenerator::new(grammar);
        let bitmap = generator.generate_state_bitmap(0);

        assert_eq!(bitmap.len(), 0);
        assert!(bitmap.is_empty());
    }

    /// `generate_state_bitmap` when no external tokens are declared: each
    /// inner row is empty but the outer length matches `state_count`.
    #[test]
    fn test_state_bitmap_zero_externals() {
        let grammar = Grammar::new("g".to_string());
        let generator = ExternalScannerGenerator::new(grammar);

        let bitmap = generator.generate_state_bitmap(5);
        assert_eq!(bitmap.len(), 5);
        for row in &bitmap {
            assert!(row.is_empty(), "row must be empty when no externals");
        }
    }

    /// Every cell in the generated bitmap is `true` — the stub returns a
    /// fully-valid matrix. Verifies the contract for many states.
    #[test]
    fn test_state_bitmap_all_cells_true() {
        let mut grammar = Grammar::new("g".to_string());
        grammar.externals.push(token("X", 10));
        grammar.externals.push(token("Y", 11));
        grammar.externals.push(token("Z", 12));

        let generator = ExternalScannerGenerator::new(grammar);
        let bitmap = generator.generate_state_bitmap(7);

        assert_eq!(bitmap.len(), 7);
        for row in &bitmap {
            assert_eq!(row.len(), 3);
            assert!(row.iter().all(|&b| b));
        }
    }

    /// Edge symbol IDs (`0` and `u16::MAX`) round-trip through the symbol map
    /// unchanged. Order matches `external_tokens` insertion order.
    #[test]
    fn test_generate_symbol_map_edge_ids() {
        let mut grammar = Grammar::new("g".to_string());
        grammar.externals.push(token("LO", 0));
        grammar.externals.push(token("HI", u16::MAX));
        grammar.externals.push(token("MID", 32_768));

        let generator = ExternalScannerGenerator::new(grammar);
        let map = generator.generate_symbol_map();

        assert_eq!(map, vec![0u16, u16::MAX, 32_768]);
    }

    /// Duplicate `symbol_id` values are preserved in `external_tokens` even
    /// though the internal `symbol_map` HashMap dedupes by key. The public
    /// `generate_symbol_map` therefore still emits every duplicate.
    #[test]
    fn test_generate_symbol_map_preserves_duplicate_symbol_ids() {
        let mut grammar = Grammar::new("g".to_string());
        grammar.externals.push(token("FIRST", 42));
        grammar.externals.push(token("SECOND", 42));
        grammar.externals.push(token("THIRD", 7));

        let generator = ExternalScannerGenerator::new(grammar);

        // Public surface: the count and the emitted map both honor order.
        assert_eq!(generator.external_token_count(), 3);
        assert_eq!(generator.generate_symbol_map(), vec![42, 42, 7]);
    }

    /// `has_external_tokens` flips with exactly one external present.
    #[test]
    fn test_has_external_tokens_single_token() {
        let mut grammar = Grammar::new("g".to_string());
        grammar.externals.push(token("ONLY", 5));

        let generator = ExternalScannerGenerator::new(grammar);

        assert!(generator.has_external_tokens());
        assert_eq!(generator.external_token_count(), 1);
        assert_eq!(generator.generate_symbol_map(), vec![5]);
    }

    /// The default state count baked into `generate_scanner_interface` is
    /// 100. With N external tokens the emitted `EXTERNAL_SCANNER_STATES`
    /// slice therefore contains exactly `100 * N` boolean literals, all
    /// `true`. We count the `true` tokens in the rendered output.
    #[test]
    fn test_scanner_interface_states_slice_size() {
        let mut grammar = Grammar::new("g".to_string());
        grammar.externals.push(token("A", 1));
        grammar.externals.push(token("B", 2));
        grammar.externals.push(token("C", 3));

        let generator = ExternalScannerGenerator::new(grammar);
        let rendered = generator.generate_scanner_interface().to_string();

        // 100 states * 3 tokens = 300 booleans, all currently `true`.
        let true_count = rendered.matches("true").count();
        assert_eq!(true_count, 300);
        // And no `false` literals should appear in the states slice.
        assert!(!rendered.contains("false"));
    }

    /// The emitted FFI struct names every Tree-sitter scanner hook so that
    /// downstream linkers can wire them in. None should be elided.
    #[test]
    fn test_scanner_interface_lists_all_ffi_hooks() {
        let mut grammar = Grammar::new("g".to_string());
        grammar.externals.push(token("TOK", 1));

        let generator = ExternalScannerGenerator::new(grammar);
        let rendered = generator.generate_scanner_interface().to_string();

        for hook in &[
            "create",
            "destroy",
            "scan",
            "serialize",
            "deserialize",
            "states",
            "symbol_map",
        ] {
            assert!(
                rendered.contains(hook),
                "expected hook `{hook}` in rendered interface"
            );
        }
    }

    /// The symbol IDs the caller put in are visible verbatim in the rendered
    /// `EXTERNAL_SCANNER_SYMBOL_MAP` slice (as decimal literals).
    #[test]
    fn test_scanner_interface_contains_symbol_ids() {
        let mut grammar = Grammar::new("g".to_string());
        grammar.externals.push(token("ALPHA", 1234));
        grammar.externals.push(token("BETA", 5678));

        let generator = ExternalScannerGenerator::new(grammar);
        let rendered = generator.generate_scanner_interface().to_string();

        assert!(rendered.contains("1234"));
        assert!(rendered.contains("5678"));
        // Sanity: the static is named correctly.
        assert!(rendered.contains("EXTERNAL_SCANNER_SYMBOL_MAP"));
    }

    /// Calling generators repeatedly is pure: same input -> same output for
    /// every public method (round-trip / referential transparency).
    #[test]
    fn test_generators_are_pure() {
        let mut grammar = Grammar::new("g".to_string());
        grammar.externals.push(token("X", 9));
        grammar.externals.push(token("Y", 11));

        let generator = ExternalScannerGenerator::new(grammar);

        assert_eq!(
            generator.generate_symbol_map(),
            generator.generate_symbol_map()
        );
        assert_eq!(
            generator.generate_state_bitmap(4),
            generator.generate_state_bitmap(4)
        );
        assert_eq!(
            generator.generate_scanner_interface().to_string(),
            generator.generate_scanner_interface().to_string()
        );
    }

    /// Empty grammar yields the empty TokenStream branch — verified via
    /// `is_empty()` rather than just `""` string comparison, exercising the
    /// `quote!{}` early-return path more directly.
    #[test]
    fn test_scanner_interface_empty_is_empty_token_stream() {
        let grammar = Grammar::new("g".to_string());
        let generator = ExternalScannerGenerator::new(grammar);

        let stream = generator.generate_scanner_interface();
        assert!(stream.is_empty());
        assert_eq!(stream.to_string(), "");
    }

    /// State bitmap rows are independent: mutating one row in a clone of the
    /// result does not affect another. Guards against accidental sharing if
    /// the implementation ever changes to use `Rc`/shared storage.
    #[test]
    fn test_state_bitmap_rows_are_independent() {
        let mut grammar = Grammar::new("g".to_string());
        grammar.externals.push(token("A", 1));
        grammar.externals.push(token("B", 2));

        let generator = ExternalScannerGenerator::new(grammar);
        let mut bitmap = generator.generate_state_bitmap(2);

        bitmap[0][0] = false;
        assert!(!bitmap[0][0]);
        assert!(bitmap[1][0], "row 1 must not alias row 0");
    }

    /// Verifies the rendered interface declares both static slices and wires
    /// them into the FFI struct via `as_ptr`. Spacing in `quote!` output is
    /// implementation-defined, so we compare against a whitespace-normalized
    /// form rather than literal substrings.
    #[test]
    fn test_scanner_interface_uses_slice_literals() {
        let mut grammar = Grammar::new("g".to_string());
        grammar.externals.push(token("ONLY", 1));

        let rendered = ExternalScannerGenerator::new(grammar)
            .generate_scanner_interface()
            .to_string();

        let collapsed: String = rendered.split_whitespace().collect();
        assert!(collapsed.contains("&["), "expected slice literal `&[`");
        assert!(
            collapsed.contains("as_ptr()"),
            "expected `as_ptr()` call after normalization"
        );
        assert!(collapsed.contains("TSExternalScannerData"));
    }

    /// A large symbol-map run-length test: confirm that 64 external tokens
    /// all flow through `generate_symbol_map` and `external_token_count`.
    #[test]
    fn test_many_external_tokens() {
        let mut grammar = Grammar::new("g".to_string());
        for i in 0..64u16 {
            grammar.externals.push(token(&format!("T{i}"), 1000 + i));
        }

        let generator = ExternalScannerGenerator::new(grammar);

        assert_eq!(generator.external_token_count(), 64);
        let map = generator.generate_symbol_map();
        assert_eq!(map.len(), 64);
        assert_eq!(map[0], 1000);
        assert_eq!(map[63], 1063);
        // First and last bitmap rows still match.
        let bitmap = generator.generate_state_bitmap(2);
        assert_eq!(bitmap[0].len(), 64);
        assert_eq!(bitmap[1].len(), 64);
    }
}
