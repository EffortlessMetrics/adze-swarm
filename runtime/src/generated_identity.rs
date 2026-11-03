//! Producer-owned native symbol identity for checked generated entrypoints.
//!
//! This Rust sidecar does not change the Tree-sitter ABI. Native pointers retain
//! the usual immutable `TSLanguage` allocation contract; validation checks the
//! metadata pairing and namespaces, not arbitrary foreign-pointer validity.

use crate::pure_parser::TSLanguage;
use adze_ir::SymbolId;

/// Versioned identity emitted beside one immutable generated language.
#[derive(Debug, Clone, Copy)]
pub struct NativeLanguageIdentityV1 {
    /// Exact native language whose tables were used to emit this identity.
    pub language: &'static TSLanguage,
    /// Sidecar format version (currently one).
    pub version: u32,
    /// Expected table-column to public-grammar mapping, including EOF.
    pub public_symbols: &'static [u16],
    /// Public symbol accepted by the generated analysis table.
    pub analysis_start: u16,
    /// Public document owner explicitly recorded by the source producer.
    pub document_start: u16,
}

/// Initialization failures detected before parsing or document materialization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeIdentityError {
    /// No authoritative sidecar was supplied.
    Unavailable,
    /// Unsupported metadata format.
    UnsupportedVersion(u32),
    /// Metadata belongs to a different native language allocation.
    LanguageMismatch,
    /// Public map missing, wrong-sized, duplicate, or different from the sidecar.
    InvalidPublicMap,
    /// A start is absent, a terminal, or has no emitted syntax rule.
    InvalidStart { role: &'static str, symbol: u16 },
}

impl std::fmt::Display for NativeIdentityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid generated native identity: {self:?}")
    }
}
impl std::error::Error for NativeIdentityError {}

/// Exact language pairing authority, shared with generated field attachments.
#[must_use]
pub fn matches_language(left: &'static TSLanguage, right: &'static TSLanguage) -> bool {
    std::ptr::eq(left, right)
}

impl NativeLanguageIdentityV1 {
    /// Validate native pairing, the public map and both explicit root roles.
    ///
    /// The normal `TSLanguage` array-allocation contract remains required.
    pub fn validate_for(&self, language: &'static TSLanguage) -> Result<(), NativeIdentityError> {
        if self.version != 1 {
            return Err(NativeIdentityError::UnsupportedVersion(self.version));
        }
        if !matches_language(self.language, language) {
            return Err(NativeIdentityError::LanguageMismatch);
        }
        let count = language.symbol_count as usize;
        if count == 0
            || count > u16::MAX as usize + 1
            || count != self.public_symbols.len()
            || language.public_symbol_map.is_null()
        {
            return Err(NativeIdentityError::InvalidPublicMap);
        }
        // SAFETY: the paired immutable native language supplies symbol_count cells.
        let native = unsafe { std::slice::from_raw_parts(language.public_symbol_map, count) };
        let mut seen = std::collections::BTreeSet::new();
        if native != self.public_symbols || native.iter().any(|symbol| !seen.insert(*symbol)) {
            return Err(NativeIdentityError::InvalidPublicMap);
        }
        let terminal_count = language.token_count as usize + language.external_token_count as usize;
        if terminal_count > count || language.rules.is_null() || language.rule_count == 0 {
            return Err(NativeIdentityError::InvalidPublicMap);
        }
        // SAFETY: the native ABI advertises rule_count immutable syntax-rule cells.
        let rules =
            unsafe { std::slice::from_raw_parts(language.rules, language.rule_count as usize) };
        for (role, symbol) in [
            ("analysis", self.analysis_start),
            ("document", self.document_start),
        ] {
            let column = native.iter().position(|id| *id == symbol);
            if !column.is_some_and(|column| {
                column >= terminal_count && rules.iter().any(|rule| usize::from(rule.lhs) == column)
            }) {
                return Err(NativeIdentityError::InvalidStart { role, symbol });
            }
        }
        Ok(())
    }

    /// Decode once with the explicit public analysis start, without start inference.
    pub fn decode_for(
        &self,
        language: &'static TSLanguage,
    ) -> Result<adze_glr_core::ParseTable, NativeIdentityError> {
        self.validate_for(language)?;
        let mut table =
            crate::decoder::decode_parse_table_with_start(language, SymbolId(self.analysis_start));
        table.grammar.document_start_symbol = Some(SymbolId(self.document_start));
        Ok(table)
    }
}
