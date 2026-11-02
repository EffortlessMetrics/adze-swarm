//! Versioned Rust metadata co-generated with a native language.
//!
//! This sidecar transports canonical grammar facts that the native parsing ABI
//! cannot represent. It does not extend or reinterpret `TSLanguage`.

use crate::pure_parser::TSLanguage;

/// Supported generated field-schema transport format.
pub const FIELD_SCHEMA_FORMAT_VERSION: u32 = 1;

/// A canonical owner/field fact emitted by the grammar generator.
#[derive(Clone, Copy, Debug)]
pub struct GeneratedFieldFact {
    /// Public grammar identity of the owning node kind.
    pub owner: u16,
    /// Canonical field name.
    pub name: &'static str,
    /// Whether every alternative requires this field.
    pub required: bool,
    /// Whether this field admits multiple children.
    pub multiple: bool,
    /// Canonical leaf targets, in public grammar identity space.
    pub targets: &'static [u16],
}

/// Canonical field facts bound to one immutable generated native language.
///
/// Constructed by generated Rust code, alongside the language and its tables.
/// The version and revision describe this schema, not a whole-grammar digest.
#[derive(Clone, Copy)]
pub struct GeneratedFieldSchemaV1 {
    /// Transport format version.
    pub format_version: u32,
    /// Revision of the canonical generator's schema semantics.
    pub schema_revision: u32,
    /// Native language whose complete grammar produced these facts.
    pub language: &'static TSLanguage,
    /// Public grammar identities in native table-column order.
    pub public_symbols: &'static [u16],
    /// Field names in native field-name table order (document field ids are one-based).
    pub field_names: &'static [&'static str],
    /// Canonical facts; never reconstructed from native production metadata.
    pub facts: &'static [GeneratedFieldFact],
}

/// Native language and its co-generated canonical metadata.
#[derive(Clone, Copy)]
pub struct GeneratedLanguageDescriptor {
    /// Native parsing tables.
    pub language: &'static TSLanguage,
    /// Canonical schema emitted from the same prepared grammar.
    pub field_schema: GeneratedFieldSchemaV1,
}

/// A descriptor does not match the language or supported schema contract.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum GeneratedSchemaError {
    /// The transport format is unsupported.
    #[error("unsupported generated field-schema format {0}")]
    UnsupportedFormat(u32),
    /// The generator's semantic revision is unsupported.
    #[error("unsupported canonical field-schema revision {0}")]
    UnsupportedRevision(u32),
    /// The sidecar belongs to another native language object.
    #[error("generated field schema belongs to a different native language")]
    LanguageIdentity,
    /// The native/public identity table differs from the generated snapshot.
    #[error("generated field schema public-symbol table mismatch")]
    PublicSymbols,
    /// The field-id/name table differs from the generated snapshot.
    #[error("generated field schema field-name table mismatch")]
    FieldNames,
    /// A fact references an unknown owner, field or target, or duplicates a fact.
    #[error("invalid generated field fact for owner {owner}, field {field:?}")]
    InvalidFact {
        /// Public owner identity.
        owner: u16,
        /// Canonical field name.
        field: &'static str,
    },
}

impl GeneratedLanguageDescriptor {
    /// Check language identity, versions and the identity tables before use.
    ///
    /// As with native parsing, the supplied language must obey `TSLanguage`'s
    /// immutable, valid static table contract. This validates metadata pairing;
    /// it cannot validate arbitrary raw pointers supplied by unsafe code.
    pub fn validate(&self) -> Result<(), GeneratedSchemaError> {
        use GeneratedSchemaError as E;
        let schema = &self.field_schema;
        if schema.format_version != FIELD_SCHEMA_FORMAT_VERSION {
            return Err(E::UnsupportedFormat(schema.format_version));
        }
        if schema.schema_revision != adze_tablegen::field_schema::FIELD_SCHEMA_REVISION {
            return Err(E::UnsupportedRevision(schema.schema_revision));
        }
        if !std::ptr::eq(self.language, schema.language) {
            return Err(E::LanguageIdentity);
        }
        let language = self.language;
        if schema.public_symbols.len() != language.symbol_count as usize
            || language.public_symbol_map.is_null()
        {
            return Err(E::PublicSymbols);
        }
        // SAFETY: the native language contract provides symbol_count static entries.
        let native_symbols = unsafe {
            std::slice::from_raw_parts(language.public_symbol_map, language.symbol_count as usize)
        };
        if native_symbols != schema.public_symbols {
            return Err(E::PublicSymbols);
        }
        let public: std::collections::BTreeSet<_> = native_symbols.iter().copied().collect();
        if public.len() != native_symbols.len() {
            return Err(E::PublicSymbols);
        }
        if schema.field_names.len() != language.field_count as usize
            || (language.field_count != 0 && language.field_names.is_null())
        {
            return Err(E::FieldNames);
        }
        for (index, expected) in schema.field_names.iter().enumerate() {
            // SAFETY: native field_names has field_count static, NUL-terminated
            // names; this native table is zero-based.
            let pointer = unsafe { *language.field_names.add(index) };
            if pointer.is_null()
                || unsafe { std::ffi::CStr::from_ptr(pointer.cast()) }.to_bytes()
                    != expected.as_bytes()
            {
                return Err(E::FieldNames);
            }
        }
        let mut seen = std::collections::BTreeSet::new();
        for fact in schema.facts {
            let owner_column = native_symbols.iter().position(|id| *id == fact.owner);
            if owner_column.is_none_or(|column| column < language.token_count as usize)
                || !schema.field_names.contains(&fact.name)
                || !seen.insert((fact.owner, fact.name))
                || fact.targets.iter().any(|target| !public.contains(target))
                || fact
                    .targets
                    .iter()
                    .collect::<std::collections::BTreeSet<_>>()
                    .len()
                    != fact.targets.len()
            {
                return Err(E::InvalidFact {
                    owner: fact.owner,
                    field: fact.name,
                });
            }
        }
        Ok(())
    }
}
