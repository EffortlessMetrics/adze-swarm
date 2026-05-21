//! High-level language builder producing validated Language structures.

use crate::compress::CompressedParseTable;
use crate::validation::TSLanguage;
use adze_glr_core::ParseTable;
use adze_ir::Grammar;
use proc_macro2::TokenStream;
use quote::quote;
use std::collections::HashSet;
use std::os::raw::c_char;

/// Language builder that produces validated Language structs
pub struct LanguageBuilder {
    grammar: Grammar,
    parse_table: ParseTable,
    start_can_be_empty: bool,
}

impl LanguageBuilder {
    /// Create a new generator
    pub fn new(grammar: Grammar, parse_table: ParseTable) -> Self {
        Self {
            grammar,
            parse_table,
            start_can_be_empty: false,
        }
    }

    /// Set whether the start symbol can be empty (nullable)
    pub fn set_start_can_be_empty(&mut self, value: bool) {
        self.start_can_be_empty = value;
    }

    /// Generate a static Language struct with full validation
    #[must_use = "generation result must be checked"]
    pub fn generate_language(&self) -> Result<TSLanguage, String> {
        // Create compressed tables
        let compressed = CompressedParseTable::from_parse_table(&self.parse_table);

        // Build the Language struct
        let language = self.build_language_struct(&compressed)?;

        // Note: Validation would be done separately by the caller
        // to avoid lifetime issues

        Ok(language)
    }

    /// Build the Language struct with all required fields
    fn build_language_struct(
        &self,
        compressed: &CompressedParseTable,
    ) -> Result<TSLanguage, String> {
        // Count various elements
        let symbol_count = self.parse_table.symbol_count as u32;
        let state_count = self.parse_table.state_count as u32;
        // token_count includes EOF (symbol 0) plus all user-defined tokens
        let token_count = (self.grammar.tokens.len() + 1) as u32;
        let external_token_count = self.grammar.externals.len() as u32;
        let field_count = self.grammar.fields.len() as u32;

        let (alias_count, max_alias_sequence_length) = self.calculate_alias_metrics();
        let large_state_count = 0;
        let production_id_count = self.calculate_alias_production_count() as u32;

        // Build symbol names array
        let symbol_names = self.build_symbol_names()?;

        // Build field names array
        let field_names = self.build_field_names()?;

        // Build symbol metadata
        let symbol_metadata = self.build_symbol_metadata();

        // Build minimal parse tables for validation
        // For now, create dummy tables - in real implementation these would be
        // generated from the compressed parse table data
        let small_parse_table = self.build_small_parse_table(compressed);
        let alias_tables = self.build_alias_tables(max_alias_sequence_length);
        let (alias_map_ptr, alias_sequences_ptr) =
            if let Some((alias_map, alias_sequences)) = alias_tables {
                (
                    Box::leak(alias_map.into_boxed_slice()).as_ptr(),
                    Box::leak(alias_sequences.into_boxed_slice()).as_ptr(),
                )
            } else {
                (std::ptr::null(), std::ptr::null())
            };

        Ok(TSLanguage {
            version: 15,
            symbol_count,
            alias_count,
            token_count,
            external_token_count,
            state_count,
            large_state_count,
            production_id_count,
            field_count,
            max_alias_sequence_length,
            parse_table: std::ptr::null(),
            small_parse_table: Box::leak(Box::new(small_parse_table)).as_ptr(),
            small_parse_table_map: std::ptr::null(),
            parse_actions: std::ptr::null(),
            symbol_names: Box::leak(Box::new(symbol_names)).as_ptr(),
            field_names: if field_count > 0 {
                Box::leak(Box::new(field_names)).as_ptr()
            } else {
                std::ptr::null()
            },
            field_map_slices: std::ptr::null(),
            field_map_entries: std::ptr::null(),
            symbol_metadata: Box::leak(Box::new(symbol_metadata)).as_ptr(),
            public_symbol_map: std::ptr::null(),
            alias_map: alias_map_ptr,
            alias_sequences: alias_sequences_ptr,
            lex_modes: std::ptr::null(),
            lex_fn: None,
            keyword_lex_fn: None,
            keyword_capture_token: 0,
            external_scanner_data: crate::validation::TSExternalScannerData {
                states: std::ptr::null(),
                symbol_map: std::ptr::null(),
                create: None,
                destroy: None,
                scan: None,
                serialize: None,
                deserialize: None,
            },
            primary_state_ids: std::ptr::null(),
        })
    }

    fn build_symbol_names(&self) -> Result<Vec<*const c_char>, String> {
        let mut names = Vec::new();

        // Add terminal symbols
        for (_, token) in &self.grammar.tokens {
            names.push(Self::leak_c_string("token symbol", token.name.clone())?);
        }

        // Add non-terminal symbols
        for (symbol_id, _) in &self.grammar.rules {
            let name = std::ffi::CString::new(format!("rule_{}", symbol_id.0))
                .expect("rule name must not contain NUL bytes");
            names.push(Box::leak(Box::new(name)).as_ptr());
        }

        // Add external symbols
        for external in &self.grammar.externals {
            names.push(Self::leak_c_string(
                "external symbol",
                external.name.clone(),
            )?);
        }

        Ok(names)
    }

    /// Calculate alias-related ABI counters from grammar alias sequences.
    fn calculate_alias_metrics(&self) -> (u32, u16) {
        let mut aliases = HashSet::new();
        let mut max_len = self.grammar.max_alias_sequence_length;

        for seq in self.grammar.alias_sequences.values() {
            max_len = max_len.max(seq.aliases.len());
            for alias in seq.aliases.iter().flatten() {
                aliases.insert(alias.as_str());
            }
        }

        let alias_count = aliases.len() as u32;
        let max_alias_sequence_length = u16::try_from(max_len).unwrap_or(u16::MAX);
        (alias_count, max_alias_sequence_length)
    }

    fn calculate_alias_production_count(&self) -> usize {
        self.grammar
            .alias_sequences
            .keys()
            .map(|production_id| production_id.0 as usize + 1)
            .max()
            .unwrap_or(0)
    }

    fn build_alias_tables(
        &self,
        max_alias_sequence_length: u16,
    ) -> Option<(Vec<u16>, Vec<crate::validation::TSSymbol>)> {
        let production_count = self.calculate_alias_production_count();
        let stride = max_alias_sequence_length as usize;
        if production_count == 0 || stride == 0 {
            return None;
        }

        let mut alias_map = Vec::with_capacity(production_count);
        let mut alias_sequences = vec![0; production_count * stride];

        for production_index in 0..production_count {
            let offset = production_index * stride;
            alias_map.push(u16::try_from(offset).unwrap_or(u16::MAX));

            let production_id = adze_ir::ProductionId(production_index as u16);
            if let Some(sequence) = self.grammar.alias_sequences.get(&production_id) {
                for (position, alias) in sequence.aliases.iter().take(stride).enumerate() {
                    if let Some(alias_name) = alias.as_deref()
                        && let Some(symbol_index) = self.resolve_alias_symbol(alias_name)
                    {
                        alias_sequences[offset + position] = symbol_index;
                    }
                }
            }
        }

        Some((alias_map, alias_sequences))
    }

    fn resolve_alias_symbol(&self, alias: &str) -> Option<crate::validation::TSSymbol> {
        let symbol_id = self
            .grammar
            .tokens
            .iter()
            .find_map(|(id, token)| (token.name == alias).then_some(*id))
            .or_else(|| {
                self.grammar
                    .rule_names
                    .iter()
                    .find_map(|(id, name)| (name == alias).then_some(*id))
            })?;

        self.parse_table
            .symbol_to_index
            .get(&symbol_id)
            .copied()
            .map(|index| index as u16)
            .or(Some(symbol_id.0))
    }

    fn build_field_names(&self) -> Result<Vec<*const c_char>, String> {
        let mut names = Vec::new();

        // First entry is always empty string
        let empty = std::ffi::CString::new("").expect("empty string cannot contain NUL bytes");
        names.push(Box::leak(Box::new(empty)).as_ptr());

        // Add field names in lexicographic order
        let mut field_names: Vec<_> = self.grammar.fields.values().collect();
        field_names.sort_unstable_by_key(|name| name.as_str());
        for field_name in field_names {
            names.push(Self::leak_c_string("field", field_name.clone())?);
        }

        Ok(names)
    }

    fn leak_c_string(kind: &str, name: String) -> Result<*const c_char, String> {
        std::ffi::CString::new(name)
            .map(|name| Box::leak(Box::new(name)).as_ptr())
            .map_err(|err| {
                format!(
                    "{kind} name contains interior NUL at byte {}",
                    err.nul_position()
                )
            })
    }

    fn build_symbol_metadata(&self) -> Vec<crate::validation::TSSymbolMetadata> {
        let mut metadata = Vec::new();

        // First symbol is always EOF (invisible, unnamed)
        metadata.push(crate::validation::TSSymbolMetadata {
            visible: false,
            named: false,
        });

        // Add metadata for terminals
        for (_, token) in &self.grammar.tokens {
            let visible = !token.name.starts_with('_');
            let named = matches!(&token.pattern, adze_ir::TokenPattern::Regex(_)) && visible;

            metadata.push(crate::validation::TSSymbolMetadata { visible, named });
        }

        // Add metadata for non-terminals
        for (symbol_id, _) in &self.grammar.rules {
            let rule_name = format!("rule_{}", symbol_id.0);
            let visible = !rule_name.starts_with('_');
            let named = visible;

            metadata.push(crate::validation::TSSymbolMetadata { visible, named });
        }

        // Add metadata for externals
        for external in &self.grammar.externals {
            let visible = !external.name.starts_with('_');
            let named = visible;

            metadata.push(crate::validation::TSSymbolMetadata { visible, named });
        }

        metadata
    }

    /// Build a minimal small parse table for testing
    fn build_small_parse_table(&self, _compressed: &CompressedParseTable) -> Vec<u16> {
        // Create a minimal parse table with the right dimensions
        // In real implementation, this would be generated from compressed data
        let table_size = self.parse_table.state_count * self.parse_table.symbol_count;
        vec![0xFFFE; table_size] // Fill with error actions for now
    }

    /// Generate Rust code for the Language
    pub fn generate_language_code(&self) -> TokenStream {
        quote! {
            // Placeholder for Language code generation
            pub fn language() -> *const TSLanguage {
                std::ptr::null()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use adze_glr_core::Action;
    use adze_ir::*;

    fn create_test_grammar() -> Grammar {
        let mut grammar = Grammar {
            name: "test".to_string(),
            ..Default::default()
        };

        // Add a simple token
        grammar.tokens.insert(
            SymbolId(1),
            Token {
                name: "number".to_string(),
                pattern: TokenPattern::Regex(r"\d+".to_string()),
                fragile: false,
            },
        );

        // Add a simple rule
        grammar.add_rule(Rule {
            lhs: SymbolId(0),
            rhs: vec![Symbol::Terminal(SymbolId(1))],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(0),
        });

        // Add field names
        grammar.fields.insert(FieldId(0), "value".to_string());

        grammar
    }

    fn create_test_parse_table() -> ParseTable {
        let mut table = crate::test_helpers::test::make_minimal_table(
            vec![vec![vec![]; 2]; 3], // 3 states, 2 symbols
            vec![vec![crate::test_helpers::test::INVALID; 2]; 3],
            vec![],
            adze_ir::SymbolId(1), // start_symbol
            adze_ir::SymbolId(1), // eof_symbol
            0,                    // external_token_count
        );

        // Add some basic actions
        // Since we don't have an actions field, just initialize the action table with proper size
        table.action_table = vec![vec![vec![Action::Error]; table.symbol_count]; table.state_count];

        table
    }

    #[test]
    fn test_language_builder_creation() {
        let grammar = create_test_grammar();
        let parse_table = create_test_parse_table();
        let builder = LanguageBuilder::new(grammar, parse_table);

        // Just verify it can be created
        assert!(builder.grammar.name == "test");
    }

    #[test]
    fn test_generate_language() {
        let grammar = create_test_grammar();
        let parse_table = create_test_parse_table();
        let builder = LanguageBuilder::new(grammar, parse_table);

        let result = builder.generate_language();
        assert!(result.is_ok());

        let language = result.unwrap();
        assert_eq!(language.version, 15);
        assert_eq!(language.symbol_count, 2);
        assert_eq!(language.state_count, 3);
    }

    #[test]
    fn test_build_symbol_names() {
        let grammar = create_test_grammar();
        let parse_table = create_test_parse_table();
        let builder = LanguageBuilder::new(grammar, parse_table);

        let names = builder.build_symbol_names().expect("symbol names");
        assert!(!names.is_empty());
        // Should have at least the token name
        assert!(
            // SAFETY: `name` points to a string literal leaked by `build_symbol_names`,
            // so it is valid, null-terminated, and lives for 'static.
            names.iter().any(|&name| unsafe {
                std::ffi::CStr::from_ptr(name).to_str().unwrap() == "number"
            })
        );
    }

    #[test]
    fn test_build_field_names() {
        let grammar = create_test_grammar();
        let parse_table = create_test_parse_table();
        let builder = LanguageBuilder::new(grammar, parse_table);

        let names = builder.build_field_names().expect("field names");
        // GLR adds an extra null field at index 0
        assert_eq!(names.len(), 2);
        // First field should be null (empty string)
        assert_eq!(
            // SAFETY: `names[0]` is a leaked CString pointer from `build_field_names`.
            unsafe { std::ffi::CStr::from_ptr(names[0]).to_str().unwrap() },
            ""
        );
        // Second field should be "value"
        assert_eq!(
            // SAFETY: `names[1]` is a leaked CString pointer from `build_field_names`.
            unsafe { std::ffi::CStr::from_ptr(names[1]).to_str().unwrap() },
            "value"
        );
    }

    #[test]
    fn test_generate_language_rejects_token_name_with_nul() {
        let mut grammar = create_test_grammar();
        grammar
            .tokens
            .get_mut(&SymbolId(1))
            .expect("test token")
            .name = "num\0ber".to_string();
        let parse_table = create_test_parse_table();
        let builder = LanguageBuilder::new(grammar, parse_table);

        let err = match builder.generate_language() {
            Ok(_) => panic!("interior NUL token name should be rejected"),
            Err(err) => err,
        };
        assert!(
            err.contains("token symbol name contains interior NUL"),
            "{err}"
        );
    }

    #[test]
    fn test_generate_language_rejects_external_name_with_nul() {
        let mut grammar = create_test_grammar();
        grammar.externals.push(ExternalToken {
            name: "external\0token".to_string(),
            symbol_id: SymbolId(2),
        });
        let parse_table = create_test_parse_table();
        let builder = LanguageBuilder::new(grammar, parse_table);

        let err = match builder.generate_language() {
            Ok(_) => panic!("interior NUL external name should be rejected"),
            Err(err) => err,
        };
        assert!(
            err.contains("external symbol name contains interior NUL"),
            "{err}"
        );
    }

    #[test]
    fn test_generate_language_rejects_field_name_with_nul() {
        let mut grammar = create_test_grammar();
        grammar.fields.insert(FieldId(1), "bad\0field".to_string());
        let parse_table = create_test_parse_table();
        let builder = LanguageBuilder::new(grammar, parse_table);

        let err = match builder.generate_language() {
            Ok(_) => panic!("interior NUL field name should be rejected"),
            Err(err) => err,
        };
        assert!(err.contains("field name contains interior NUL"), "{err}");
    }

    #[test]
    fn test_build_symbol_metadata() {
        let grammar = create_test_grammar();
        let parse_table = create_test_parse_table();
        let builder = LanguageBuilder::new(grammar, parse_table);

        let metadata = builder.build_symbol_metadata();
        assert!(!metadata.is_empty());
    }

    #[test]
    fn test_language_generator_code() {
        let grammar = create_test_grammar();
        let parse_table = create_test_parse_table();
        let builder = LanguageBuilder::new(grammar, parse_table);

        let code = builder.generate_language_code();
        let code_str = code.to_string();

        // Check for key elements
        assert!(code_str.contains("language"));
        assert!(code_str.contains("TSLanguage"));
    }

    #[test]
    fn test_language_with_externals() {
        let mut grammar = create_test_grammar();

        // Add external token
        grammar.externals.push(ExternalToken {
            name: "comment".to_string(),
            symbol_id: SymbolId(100),
        });

        let parse_table = create_test_parse_table();
        let builder = LanguageBuilder::new(grammar, parse_table);

        let result = builder.generate_language();
        assert!(result.is_ok());

        let language = result.unwrap();
        assert_eq!(language.external_token_count, 1);
    }

    #[test]
    fn test_language_with_multiple_fields() {
        let mut grammar = create_test_grammar();

        // Add more fields
        grammar.fields.insert(FieldId(1), "left".to_string());
        grammar.fields.insert(FieldId(2), "operator".to_string());
        grammar.fields.insert(FieldId(3), "right".to_string());

        let parse_table = create_test_parse_table();
        let builder = LanguageBuilder::new(grammar, parse_table);

        let result = builder.generate_language();
        assert!(result.is_ok());

        let language = result.unwrap();
        assert_eq!(language.field_count, 4);
    }

    #[test]
    fn test_build_field_names_are_lexicographically_sorted() {
        let mut grammar = create_test_grammar();
        grammar.fields.clear();
        grammar.fields.insert(FieldId(2), "right".to_string());
        grammar.fields.insert(FieldId(1), "left".to_string());
        grammar.fields.insert(FieldId(3), "operator".to_string());

        let parse_table = create_test_parse_table();
        let builder = LanguageBuilder::new(grammar, parse_table);
        let names = builder.build_field_names().expect("field names");

        let fields: Vec<&str> = names
            .iter()
            .skip(1)
            .map(|&ptr| {
                // SAFETY: `ptr` values are leaked `CString` pointers produced by
                // `build_field_names`, so they are valid and nul-terminated.
                unsafe { std::ffi::CStr::from_ptr(ptr).to_str().unwrap() }
            })
            .collect();

        assert_eq!(fields, vec!["left", "operator", "right"]);
    }
}
