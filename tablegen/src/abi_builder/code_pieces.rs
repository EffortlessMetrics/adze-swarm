#![cfg_attr(feature = "strict_docs", allow(missing_docs))]
//! Conditional `TokenStream` fragments emitted by
//! `AbiLanguageBuilder::generate`.
//!
//! The generation orchestrator threads several optional fragments through its
//! final `quote!` template: the external-scanner interface, the alias-table
//! statics, and the field-name pointer array. Each fragment has two shapes
//! depending on whether the grammar uses that feature; isolating the
//! branching here keeps `generate` close to pure assembly.

use proc_macro2::TokenStream;
use quote::quote;

use super::{AbiLanguageBuilder, LanguageCounts};

/// The three `TokenStream` fragments needed for the alias-table portion of
/// the generated language module: the static-array definitions, and the two
/// pointer expressions placed into the `TSLanguage` struct.
pub(super) struct AliasTablePieces {
    pub(super) tables: TokenStream,
    pub(super) map_ptr: TokenStream,
    pub(super) sequences_ptr: TokenStream,
}

impl<'a> AbiLanguageBuilder<'a> {
    /// Build the external-scanner code block and the `ExternalScanner`
    /// struct literal placed into `TSLanguage`.
    ///
    /// When the grammar has no external tokens both fragments are empty/null.
    pub(super) fn build_external_scanner_pieces(&self) -> (TokenStream, TokenStream) {
        if self.grammar.externals.is_empty() {
            return (
                quote! {},
                quote! {
                    ExternalScanner {
                        states: std::ptr::null(),
                        symbol_map: std::ptr::null(),
                        create: None,
                        destroy: None,
                        scan: None,
                        serialize: None,
                        deserialize: None,
                    }
                },
            );
        }

        use crate::external_scanner_v2::ExternalScannerGenerator;

        let scanner_gen =
            ExternalScannerGenerator::new(self.grammar.clone(), self.parse_table.clone());
        let scanner_interface = scanner_gen.generate_scanner_interface();

        // Grammars with external scanners provide their own FFI functions; we
        // only emit the static interface tables here.
        let scanner_functions = quote! {};

        let scanner_struct = quote! {
            ExternalScanner {
                states: EXTERNAL_SCANNER_STATES.as_ptr() as *const u8,
                symbol_map: EXTERNAL_SCANNER_SYMBOL_MAP.as_ptr(),
                create: None,
                destroy: None,
                scan: None,
                serialize: None,
                deserialize: None,
            }
        };

        (
            quote! {
                #scanner_interface
                #scanner_functions
            },
            scanner_struct,
        )
    }

    /// Build the alias-table statics plus the two raw-pointer expressions
    /// referenced by the `TSLanguage` struct.
    ///
    /// When the grammar carries no aliases (or the longest sequence is empty)
    /// the table definitions collapse to `quote!{}` and the pointers fall
    /// back to typed null pointers.
    pub(super) fn build_alias_table_pieces(
        &self,
        counts: &LanguageCounts,
        alias_map: &[TokenStream],
        alias_sequences: &[TokenStream],
    ) -> AliasTablePieces {
        let has_aliases = counts.alias_count > 0 && counts.max_alias_sequence_length > 0;
        if has_aliases {
            AliasTablePieces {
                tables: quote! {
                    static ALIAS_MAP: &[u16] = &[#(#alias_map),*];
                    static ALIAS_SEQUENCES: &[u16] = &[#(#alias_sequences),*];
                },
                map_ptr: quote! { ALIAS_MAP.as_ptr() },
                sequences_ptr: quote! { ALIAS_SEQUENCES.as_ptr() },
            }
        } else {
            AliasTablePieces {
                tables: quote! {},
                map_ptr: quote! { std::ptr::null() },
                sequences_ptr: quote! { std::ptr::null::<u16>() },
            }
        }
    }

    /// Build the `FIELD_NAME_PTRS` static declaration.
    ///
    /// Grammars without fields emit a zero-length array (the typed array
    /// still lets the language struct take a non-null `as_ptr`); otherwise we
    /// emit a sized array whose length is bound to the field count constant.
    pub(super) fn build_field_names_array(
        &self,
        counts: &LanguageCounts,
        field_name_ptrs: &[TokenStream],
    ) -> TokenStream {
        let field_count = counts.field_count;
        if field_count == 0 {
            quote! {
                static FIELD_NAME_PTRS: [SyncPtr; 0] = [];
            }
        } else {
            quote! {
                const FIELD_NAME_PTRS_LEN: usize = #field_count as usize;
                static FIELD_NAME_PTRS: [SyncPtr; FIELD_NAME_PTRS_LEN] = [
                    #(#field_name_ptrs),*
                ];
            }
        }
    }
}
