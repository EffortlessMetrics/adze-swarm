//! Versioned Rust metadata co-generated with a native language.
//!
//! This sidecar transports canonical grammar facts that the native parsing ABI
//! cannot represent. It does not extend or reinterpret `TSLanguage`.

#[cfg(feature = "pure-rust")]
use crate::generated_identity::matches_language;
use crate::pure_parser::TSLanguage;
#[cfg(not(feature = "pure-rust"))]
fn matches_language(left: &'static TSLanguage, right: &'static TSLanguage) -> bool {
    std::ptr::eq(left, right)
}

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
    /// Projection origin references, versions or language/schema binding are invalid.
    #[error("invalid generated field projection")]
    InvalidProjection,
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
        if !matches_language(self.language, schema.language) {
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

/// One source field occurrence in a generated selected-node projection.
#[derive(Clone, Copy, Debug)]
pub struct GeneratedFieldOccurrence {
    /// Canonical field name.
    pub name: &'static str,
    /// Prepared RHS slot, excluding non-RHS extras.
    pub position: usize,
    /// Actual target grammar identity.
    pub target: u16,
    /// Explicit optional carrier transition, if present.
    pub optional: Option<GeneratedOptionalCarrier>,
}
/// A producer-declared optional carrier with retained or unary-elided form.
#[derive(Clone, Copy, Debug)]
pub struct GeneratedOptionalCarrier {
    /// Allocated helper grammar identity.
    pub helper: u16,
    /// Present syntax alternative ordinal (never ProductionId).
    pub present_alternative: usize,
    /// Empty syntax alternative ordinal (never ProductionId).
    pub empty_alternative: usize,
}
/// First-slice program for an owner with exactly one source alternative.
#[derive(Clone, Copy, Debug)]
pub struct GeneratedOwnerProjection {
    /// Source grammar owner.
    pub owner: u16,
    /// Ordered prepared RHS grammar identities.
    pub rhs: &'static [u16],
    /// Ordered source field occurrences.
    pub fields: &'static [GeneratedFieldOccurrence],
}
/// Versioned optional-origin projection sidecar, separate from fact transport.
#[derive(Clone, Copy)]
pub struct GeneratedFieldProjectionV1 {
    /// Projection transport version; currently one.
    pub format_version: u32,
    /// Projection semantics revision. Revision 1 retains positional optional
    /// interpretation; revision 2 additionally certifies direct FIELD observations
    /// on admitted owners without optional transitions. Transport layout is unchanged.
    pub projection_revision: u32,
    /// Co-generated native language identity.
    pub language: &'static TSLanguage,
    /// Canonical schema revision paired with this program.
    pub schema_revision: u32,
    /// Admitted owners. Missing owners explicitly lack this capability.
    pub owners: &'static [GeneratedOwnerProjection],
}
/// Additive generated entry descriptor including selected-node projection.
#[derive(Clone, Copy)]
pub struct GeneratedLanguageDescriptorV2 {
    /// Native parsing tables.
    pub language: &'static TSLanguage,
    /// Canonical source field facts.
    pub field_schema: GeneratedFieldSchemaV1,
    /// Optional explicitly versioned projection capability.
    pub field_projection: Option<GeneratedFieldProjectionV1>,
}
impl GeneratedLanguageDescriptorV2 {
    /// Validate fact transport and the projection's language/schema binding.
    pub fn validate(&self) -> Result<(), GeneratedSchemaError> {
        GeneratedLanguageDescriptor {
            language: self.language,
            field_schema: self.field_schema,
        }
        .validate()?;
        let Some(projection) = &self.field_projection else {
            return Ok(());
        };
        if projection.format_version != 1
            || !matches!(projection.projection_revision, 1 | 2)
            || projection.schema_revision != self.field_schema.schema_revision
            || !matches_language(projection.language, self.language)
        {
            return Err(GeneratedSchemaError::InvalidProjection);
        }
        let public: std::collections::BTreeSet<_> =
            self.field_schema.public_symbols.iter().copied().collect();
        let mut owners = std::collections::BTreeSet::new();
        if projection.owners.is_empty() {
            return Ok(());
        }
        if self.language.rules.is_null() || self.language.rule_count == 0 {
            return Err(GeneratedSchemaError::InvalidProjection);
        }
        // SAFETY: as in the native decoder, a valid immutable language supplies
        // exactly rule_count TSRule entries. No RHS identities are reconstructed
        // from aliases; the sidecar supplies those canonical facts.
        let rules = if self.language.rule_count == 0 {
            &[][..]
        } else {
            unsafe {
                std::slice::from_raw_parts(self.language.rules, self.language.rule_count as usize)
            }
        };
        let widths_for = |owner| {
            rules
                .iter()
                .filter(|rule| {
                    self.field_schema.public_symbols.get(rule.lhs as usize) == Some(&owner)
                })
                .map(|rule| rule.rhs_len as usize)
                .collect::<Vec<_>>()
        };
        for owner in projection.owners {
            if widths_for(owner.owner) != [owner.rhs.len()] {
                return Err(GeneratedSchemaError::InvalidProjection);
            }
            if !public.contains(&owner.owner)
                || !owners.insert(owner.owner)
                || owner.rhs.iter().any(|id| !public.contains(id))
            {
                return Err(GeneratedSchemaError::InvalidProjection);
            }
            let facts: Vec<_> = self
                .field_schema
                .facts
                .iter()
                .filter(|fact| fact.owner == owner.owner)
                .collect();
            if facts.iter().any(|fact| {
                let fields: Vec<_> = owner
                    .fields
                    .iter()
                    .filter(|field| field.name == fact.name)
                    .collect();
                let targets: std::collections::BTreeSet<_> =
                    fields.iter().map(|field| field.target).collect();
                let declared: std::collections::BTreeSet<_> =
                    fact.targets.iter().copied().collect();
                fields.is_empty()
                    || fact.required != fields.iter().any(|field| field.optional.is_none())
                    || fact.multiple != (fields.len() > 1)
                    || targets != declared
            }) {
                return Err(GeneratedSchemaError::InvalidProjection);
            }
            let mut occurrences = std::collections::BTreeSet::new();
            for field in owner.fields {
                let carrier = field
                    .optional
                    .map_or(field.target, |optional| optional.helper);
                if owner.rhs.get(field.position) != Some(&carrier)
                    || !occurrences.insert((field.position, field.name))
                    || !facts
                        .iter()
                        .any(|fact| fact.name == field.name && fact.targets.contains(&field.target))
                    || field.optional.is_some_and(|optional| {
                        let widths = widths_for(optional.helper);
                        optional.present_alternative == optional.empty_alternative
                            || optional.helper == field.target
                            || widths.len() != 2
                            || widths.get(optional.present_alternative) != Some(&1)
                            || widths.get(optional.empty_alternative) != Some(&0)
                    })
                {
                    return Err(GeneratedSchemaError::InvalidProjection);
                }
            }
        }
        Ok(())
    }
}
