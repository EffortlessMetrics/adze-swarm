//! Typed metadata contract for `.parsetable` artifacts.
//!
//! This crate owns the serialization model used by tablegen and runtime2 when
//! emitting/parsing parsetable metadata payloads.

#![forbid(unsafe_op_in_unsafe_fn)]
#![cfg_attr(feature = "strict_api", deny(unreachable_pub))]
#![cfg_attr(not(feature = "strict_api"), warn(unreachable_pub))]
#![cfg_attr(feature = "strict_docs", deny(missing_docs))]
#![cfg_attr(not(feature = "strict_docs"), allow(missing_docs))]

/// Governance metadata embedded in `.parsetable` metadata payloads.
pub mod governance;

pub use governance::{GovernanceMetadata, ParserFeatureProfileSnapshot};
use serde::{Deserialize, Serialize};

mod parse;

/// Magic number identifying .parsetable files: "RSPT".
pub const MAGIC_NUMBER: [u8; 4] = [0x52, 0x53, 0x50, 0x54];

/// Current .parsetable format version.
pub const FORMAT_VERSION: u32 = 1;

/// Metadata schema version.
pub const METADATA_SCHEMA_VERSION: &str = "1.0";

/// Error type for metadata and table container operations.
#[derive(Debug, thiserror::Error)]
pub enum ParsetableError {
    /// I/O error during file operations.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Serialization error.
    #[error("Serialization error: {0}")]
    Serialization(String),

    /// Invalid metadata payload.
    #[error("Invalid metadata: {0}")]
    InvalidMetadata(String),

    /// Grammar hash computation failed.
    #[error("Grammar hash computation failed: {0}")]
    HashError(String),
}

/// Metadata embedded in .parsetable files.
///
/// # Examples
///
/// ```
/// use adze_parsetable_metadata::*;
///
/// let metadata = ParsetableMetadata {
///     schema_version: METADATA_SCHEMA_VERSION.to_string(),
///     grammar: GrammarInfo {
///         name: "json".into(),
///         version: "1.0.0".into(),
///         language: "json".into(),
///     },
///     generation: GenerationInfo {
///         timestamp: "2025-01-01T00:00:00Z".into(),
///         tool_version: "0.1.0".into(),
///         rust_version: "1.95.0".into(),
///         host_triple: "x86_64-unknown-linux-gnu".into(),
///     },
///     statistics: TableStatistics {
///         state_count: 10, symbol_count: 5, rule_count: 3,
///         conflict_count: 0, multi_action_cells: 0,
///     },
///     features: FeatureFlags {
///         glr_enabled: false, external_scanner: false, incremental: false,
///     },
///     feature_profile: None,
///     governance: None,
/// };
/// let json = serde_json::to_string(&metadata).unwrap();
/// let parsed = ParsetableMetadata::parse_json(&json).unwrap();
/// assert_eq!(metadata, parsed);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ParsetableMetadata {
    /// Metadata schema version.
    pub schema_version: String,
    /// Grammar information.
    pub grammar: GrammarInfo,
    /// Generation information.
    pub generation: GenerationInfo,
    /// Parse table statistics.
    pub statistics: TableStatistics,
    /// Feature flags.
    pub features: FeatureFlags,
    /// Feature profile snapshot for this build artifact.
    #[serde(default)]
    pub feature_profile: Option<ParserFeatureProfileSnapshot>,
    /// BDD progress snapshot attached at generation time.
    #[serde(default)]
    pub governance: Option<GovernanceMetadata>,
}

impl ParsetableMetadata {
    /// Parse metadata from a JSON payload.
    ///
    /// # Examples
    ///
    /// ```
    /// use adze_parsetable_metadata::FeatureFlags;
    ///
    /// let flags = FeatureFlags { glr_enabled: true, external_scanner: false, incremental: false };
    /// let bytes = serde_json::to_vec(&flags).unwrap();
    /// let parsed: FeatureFlags = serde_json::from_slice(&bytes).unwrap();
    /// assert_eq!(flags, parsed);
    /// ```
    #[must_use = "parsing may fail; the Result should be checked"]
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, serde_json::Error> {
        parse::parse_metadata_bytes(bytes)
    }

    /// Parse metadata from a UTF-8 JSON string.
    #[must_use = "parsing may fail; the Result should be checked"]
    pub fn parse_json(payload: &str) -> Result<Self, serde_json::Error> {
        parse::parse_metadata_json(payload)
    }
}

/// Grammar identification information.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GrammarInfo {
    /// Grammar name.
    pub name: String,
    /// Grammar version.
    pub version: String,
    /// Language name.
    pub language: String,
}

/// Generation metadata.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GenerationInfo {
    /// ISO8601 timestamp.
    pub timestamp: String,
    /// Tool version.
    pub tool_version: String,
    /// Rust compiler version.
    pub rust_version: String,
    /// Host triple.
    pub host_triple: String,
}

/// Parse table statistics.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct TableStatistics {
    /// Number of parser states.
    pub state_count: usize,
    /// Number of symbols.
    pub symbol_count: usize,
    /// Number of production rules.
    pub rule_count: usize,
    /// Number of GLR conflicts.
    pub conflict_count: usize,
    /// Number of action table cells with multiple actions.
    pub multi_action_cells: usize,
}

/// Parser feature flags for metadata export.
///
/// # Examples
///
/// ```
/// use adze_parsetable_metadata::FeatureFlags;
///
/// let flags = FeatureFlags {
///     glr_enabled: true,
///     external_scanner: false,
///     incremental: true,
/// };
/// assert!(flags.glr_enabled);
/// assert!(!flags.external_scanner);
/// ```
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct FeatureFlags {
    /// GLR parsing feature flag.
    pub glr_enabled: bool,
    /// External scanner support flag.
    pub external_scanner: bool,
    /// Incremental parsing support flag.
    pub incremental: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn magic_number_is_rspt() {
        assert_eq!(&MAGIC_NUMBER, b"RSPT");
    }

    #[test]
    fn format_version_is_positive() {
        const { assert!(FORMAT_VERSION > 0) };
    }

    #[test]
    fn metadata_schema_version_is_non_empty() {
        assert!(!METADATA_SCHEMA_VERSION.is_empty());
    }

    #[test]
    fn parsetable_error_display() {
        let err = ParsetableError::Serialization("bad json".to_string());
        let msg = format!("{err}");
        assert!(msg.contains("bad json"));

        let err = ParsetableError::InvalidMetadata("missing field".to_string());
        assert!(format!("{err}").contains("missing field"));

        let err = ParsetableError::HashError("sha256 failed".to_string());
        assert!(format!("{err}").contains("sha256 failed"));
    }

    #[test]
    fn feature_flags_serde_roundtrip() {
        let flags = FeatureFlags {
            glr_enabled: true,
            external_scanner: false,
            incremental: true,
        };
        let json = serde_json::to_string(&flags).unwrap();
        let deserialized: FeatureFlags = serde_json::from_str(&json).unwrap();
        assert_eq!(flags, deserialized);
    }

    #[test]
    fn table_statistics_serde_roundtrip() {
        let stats = TableStatistics {
            state_count: 100,
            symbol_count: 50,
            rule_count: 30,
            conflict_count: 2,
            multi_action_cells: 5,
        };
        let json = serde_json::to_string(&stats).unwrap();
        let deserialized: TableStatistics = serde_json::from_str(&json).unwrap();
        assert_eq!(stats, deserialized);
    }

    #[test]
    fn full_metadata_serde_roundtrip() {
        let metadata = ParsetableMetadata {
            schema_version: METADATA_SCHEMA_VERSION.to_string(),
            grammar: GrammarInfo {
                name: "test_grammar".to_string(),
                version: "1.0.0".to_string(),
                language: "test".to_string(),
            },
            generation: GenerationInfo {
                timestamp: "2025-01-01T00:00:00Z".to_string(),
                tool_version: "0.1.0".to_string(),
                rust_version: "1.95.0".to_string(),
                host_triple: "x86_64-unknown-linux-gnu".to_string(),
            },
            statistics: TableStatistics {
                state_count: 10,
                symbol_count: 5,
                rule_count: 3,
                conflict_count: 0,
                multi_action_cells: 0,
            },
            features: FeatureFlags {
                glr_enabled: false,
                external_scanner: false,
                incremental: false,
            },
            feature_profile: None,
            governance: None,
        };
        let json = serde_json::to_string_pretty(&metadata).unwrap();
        let deserialized = ParsetableMetadata::parse_json(&json).unwrap();
        assert_eq!(metadata, deserialized);
    }

    #[test]
    fn metadata_from_bytes() {
        let metadata = ParsetableMetadata {
            schema_version: "1.0".to_string(),
            grammar: GrammarInfo {
                name: "json".to_string(),
                version: "0.1.0".to_string(),
                language: "json".to_string(),
            },
            generation: GenerationInfo {
                timestamp: "2025-01-01T00:00:00Z".to_string(),
                tool_version: "0.1.0".to_string(),
                rust_version: "1.95.0".to_string(),
                host_triple: "x86_64-unknown-linux-gnu".to_string(),
            },
            statistics: TableStatistics {
                state_count: 1,
                symbol_count: 1,
                rule_count: 1,
                conflict_count: 0,
                multi_action_cells: 0,
            },
            features: FeatureFlags {
                glr_enabled: false,
                external_scanner: false,
                incremental: false,
            },
            feature_profile: Some(ParserFeatureProfileSnapshot::new(false, false, true, false)),
            governance: Some(GovernanceMetadata::with_counts("core", 5, 8, "core:5/8")),
        };
        let bytes = serde_json::to_vec(&metadata).unwrap();
        let deserialized = ParsetableMetadata::from_bytes(&bytes).unwrap();
        assert_eq!(metadata, deserialized);
    }

    #[test]
    fn reexported_governance_types_accessible() {
        let _snap = ParserFeatureProfileSnapshot::new(false, false, false, false);
        let _meta = GovernanceMetadata::default();
    }

    // --- ParsetableError comprehensive tests ---

    #[test]
    fn error_is_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}
        assert_send::<ParsetableError>();
        assert_sync::<ParsetableError>();
    }

    #[test]
    fn error_io_display() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file missing");
        let err = ParsetableError::Io(io_err);
        let msg = err.to_string();
        assert!(msg.contains("I/O error"), "got: {msg}");
        assert!(msg.contains("file missing"), "got: {msg}");
    }

    #[test]
    fn error_from_io() {
        let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "no access");
        let err: ParsetableError = io_err.into();
        assert!(matches!(err, ParsetableError::Io(_)));
        assert!(err.to_string().contains("no access"));
    }

    #[test]
    fn error_io_source_chain() {
        use std::error::Error;
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "gone");
        let err = ParsetableError::Io(io_err);
        let src = err.source().expect("Io variant should have a source");
        assert!(src.to_string().contains("gone"));
    }

    #[test]
    fn error_serialization_no_source() {
        use std::error::Error;
        let err = ParsetableError::Serialization("bad payload".into());
        assert!(
            err.source().is_none(),
            "Serialization variant wraps no inner error"
        );
    }

    #[test]
    fn error_invalid_metadata_no_source() {
        use std::error::Error;
        let err = ParsetableError::InvalidMetadata("corrupt".into());
        assert!(
            err.source().is_none(),
            "InvalidMetadata variant wraps no inner error"
        );
    }

    #[test]
    fn error_hash_no_source() {
        use std::error::Error;
        let err = ParsetableError::HashError("sha broke".into());
        assert!(
            err.source().is_none(),
            "HashError variant wraps no inner error"
        );
    }

    #[test]
    fn error_construct_all_variants() {
        let _ = ParsetableError::Io(std::io::Error::other("x"));
        let _ = ParsetableError::Serialization("x".into());
        let _ = ParsetableError::InvalidMetadata("x".into());
        let _ = ParsetableError::HashError("x".into());
    }

    #[test]
    fn error_debug_format() {
        let err = ParsetableError::InvalidMetadata("dbg test".into());
        let dbg = format!("{err:?}");
        assert!(
            dbg.contains("InvalidMetadata"),
            "Debug should name the variant: {dbg}"
        );
        assert!(
            dbg.contains("dbg test"),
            "Debug should include payload: {dbg}"
        );
    }
}
