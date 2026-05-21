//! .parsetable binary file format writer
//!
//! This module implements the .parsetable file format as specified in
//! docs/specs/PARSETABLE_FILE_FORMAT_SPEC.md
//!
//! ## File Format
//!
//! ```text
//! ┌────────────────────────────────────────────┐
//! │ Magic Number (4 bytes): "RSPT"            │
//! ├────────────────────────────────────────────┤
//! │ Format Version (4 bytes): u32 LE          │
//! ├────────────────────────────────────────────┤
//! │ Grammar Hash (32 bytes): SHA256           │
//! ├────────────────────────────────────────────┤
//! │ Metadata Length (4 bytes): u32 LE         │
//! ├────────────────────────────────────────────┤
//! │ Metadata JSON (variable length)           │
//! ├────────────────────────────────────────────┤
//! │ Table Data Length (4 bytes): u32 LE       │
//! ├────────────────────────────────────────────┤
//! │ ParseTable Bincode (variable length)      │
//! └────────────────────────────────────────────┘
//! ```

use adze_bdd_governance_core::{BddPhase, GLR_CONFLICT_PRESERVATION_GRID};
use adze_glr_core::ParseTable;
use adze_ir::Grammar;
use std::convert::TryFrom;
use std::fs::File;
use std::io::Write;
use std::path::Path;

pub use adze_parsetable_metadata::{
    FORMAT_VERSION, FeatureFlags, GenerationInfo, GovernanceMetadata, GrammarInfo, MAGIC_NUMBER,
    METADATA_SCHEMA_VERSION, ParserFeatureProfileSnapshot, ParsetableError, ParsetableMetadata,
    TableStatistics,
};

fn checked_u32_len(len: usize, section: &str) -> Result<u32, ParsetableError> {
    u32::try_from(len).map_err(|_| {
        ParsetableError::Serialization(format!(
            "{section} length ({len}) exceeds u32::MAX; cannot encode .parsetable payload"
        ))
    })
}

/// Writer for .parsetable binary files
pub struct ParsetableWriter<'a> {
    grammar: &'a Grammar,
    parse_table: &'a ParseTable,
    metadata: ParsetableMetadata,
}

impl<'a> ParsetableWriter<'a> {
    /// Create a new .parsetable writer
    ///
    /// # Arguments
    ///
    /// * `grammar` - Grammar definition
    /// * `parse_table` - Generated parse table
    /// * `grammar_name` - Name of the grammar (e.g., "python")
    /// * `grammar_version` - Semantic version of the grammar
    pub fn new(
        grammar: &'a Grammar,
        parse_table: &'a ParseTable,
        grammar_name: impl Into<String>,
        grammar_version: impl Into<String>,
    ) -> Self {
        let metadata = Self::build_metadata(grammar, parse_table, grammar_name, grammar_version);

        Self {
            grammar,
            parse_table,
            metadata,
        }
    }

    /// Build metadata from grammar and parse table
    fn build_metadata(
        grammar: &Grammar,
        parse_table: &ParseTable,
        grammar_name: impl Into<String>,
        grammar_version: impl Into<String>,
    ) -> ParsetableMetadata {
        // Count multi-action cells.
        let multi_action_cells = parse_table
            .action_table
            .iter()
            .flat_map(|row| row.iter())
            .filter(|cell| cell.len() > 1)
            .count();

        // Count conflicts (simplified - actual conflict detection is more complex).
        let conflict_count = multi_action_cells;

        // BDD progress is emitted as operational metadata for tooling diagnostics.
        let feature_profile = ParserFeatureProfileSnapshot::from_env();
        let governance = GovernanceMetadata::for_grid(
            BddPhase::Runtime,
            GLR_CONFLICT_PRESERVATION_GRID,
            feature_profile.as_profile(),
        );

        ParsetableMetadata {
            schema_version: METADATA_SCHEMA_VERSION.to_string(),
            grammar: GrammarInfo {
                name: grammar_name.into(),
                version: grammar_version.into(),
                language: grammar.name.clone(),
            },
            generation: GenerationInfo {
                timestamp: chrono::Utc::now().to_rfc3339(),
                tool_version: env!("CARGO_PKG_VERSION").to_string(),
                rust_version: rustc_version::version()
                    .map(|version| version.to_string())
                    .unwrap_or_else(|_| "unknown".to_string()),
                host_triple: std::env::var("TARGET").unwrap_or_else(|_| "unknown".to_string()),
            },
            statistics: TableStatistics {
                state_count: parse_table.state_count,
                symbol_count: parse_table.symbol_count,
                rule_count: parse_table.rules.len(),
                conflict_count,
                multi_action_cells,
            },
            features: FeatureFlags {
                glr_enabled: multi_action_cells > 0,
                external_scanner: !parse_table.external_scanner_states.is_empty(),
                incremental: cfg!(feature = "incremental_glr"),
            },
            feature_profile: Some(feature_profile),
            governance: Some(governance),
        }
    }

    /// Compute SHA-256 hash of grammar source
    ///
    /// This is a placeholder - actual implementation will hash grammar definition.
    fn compute_grammar_hash(&self) -> [u8; 32] {
        use sha2::{Digest, Sha256};

        let mut hasher = Sha256::new();

        // Hash grammar name and rules as a simple identifier.
        hasher.update(self.grammar.name.as_bytes());
        for rule in &self.parse_table.rules {
            hasher.update((rule.lhs.0 as u32).to_le_bytes());
            hasher.update(rule.rhs_len.to_le_bytes());
        }

        hasher.finalize().into()
    }

    /// Write .parsetable file
    ///
    /// # Arguments
    ///
    /// * `path` - Output file path
    ///
    /// # Returns
    ///
    /// `Ok(())` on success, `Err` if file writing fails
    pub fn write_file<P: AsRef<Path>>(&self, path: P) -> Result<(), ParsetableError> {
        let mut file = File::create(path)?;

        // 1. Write magic number.
        file.write_all(&MAGIC_NUMBER)?;

        // 2. Write format version (little-endian).
        file.write_all(&FORMAT_VERSION.to_le_bytes())?;

        // 3. Write grammar hash.
        let hash = self.compute_grammar_hash();
        file.write_all(&hash)?;

        // 4. Write metadata.
        let metadata_json = serde_json::to_string_pretty(&self.metadata).map_err(|e| {
            ParsetableError::Serialization(format!("Metadata JSON serialization failed: {}", e))
        })?;
        let metadata_bytes = metadata_json.as_bytes();
        let metadata_len = checked_u32_len(metadata_bytes.len(), "Metadata JSON")?;
        file.write_all(&metadata_len.to_le_bytes())?;
        file.write_all(metadata_bytes)?;

        // 5. Write parse table.
        #[cfg(feature = "serialization")]
        {
            let table_bytes = self.parse_table.to_bytes().map_err(|e| {
                ParsetableError::Serialization(format!("ParseTable serialization failed: {}", e))
            })?;
            let table_len = checked_u32_len(table_bytes.len(), "ParseTable binary")?;
            file.write_all(&table_len.to_le_bytes())?;
            file.write_all(&table_bytes)?;
        }

        #[cfg(not(feature = "serialization"))]
        {
            return Err(ParsetableError::Serialization(
                "ParseTable serialization requires 'serialization' feature".to_string(),
            ));
        }

        file.flush()?;
        Ok(())
    }

    /// Get reference to metadata
    pub fn metadata(&self) -> &ParsetableMetadata {
        &self.metadata
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_magic_number() {
        assert_eq!(&MAGIC_NUMBER, b"RSPT");
    }

    #[test]
    fn test_format_version() {
        assert_eq!(FORMAT_VERSION, 1);
    }

    #[test]
    fn test_metadata_schema_version() {
        assert_eq!(METADATA_SCHEMA_VERSION, "1.0");
    }

    #[test]
    fn test_checked_u32_len_accepts_max_u32() {
        let len = usize::try_from(u32::MAX).unwrap_or(usize::MAX);
        assert_eq!(checked_u32_len(len, "metadata").ok(), Some(u32::MAX));
    }

    #[test]
    fn test_checked_u32_len_rejects_oversized_payload() {
        let too_large = usize::try_from(u32::MAX)
            .ok()
            .and_then(|max| max.checked_add(1))
            .unwrap_or(usize::MAX);

        let err = checked_u32_len(too_large, "metadata").err();
        assert!(
            matches!(err, Some(ParsetableError::Serialization(message)) if message.contains("exceeds u32::MAX"))
        );
    }
}
