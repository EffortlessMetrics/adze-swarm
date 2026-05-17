//! # adze-tablegen
//!
//! Generate and compress LR(1) parse tables for Adze grammars.

// Table generation requires unsafe for FFI-compatible Language struct generation
#![forbid(unsafe_op_in_unsafe_fn)]
#![deny(private_interfaces)]
#![cfg_attr(feature = "strict_api", deny(unreachable_pub))]
#![cfg_attr(not(feature = "strict_api"), warn(unreachable_pub))]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![cfg_attr(feature = "strict_docs", deny(missing_docs))]
#![cfg_attr(not(feature = "strict_docs"), allow(missing_docs))]

mod test_helpers;
mod util;

#[cfg(test)]
pub use crate::test_helpers::test::{make_empty_table, make_minimal_table};

/// Tree-sitter ABI type definitions and constants.
pub mod abi;
/// Builder for generating ABI-compatible Language structs.
pub mod abi_builder;
/// Parse table compression algorithms.
pub mod compress;
/// Additional compression utilities and strategies.
pub mod compression;
/// Error types for table generation
pub mod error;
/// External scanner code generation (v1).
pub mod external_scanner;
/// External scanner code generation (v2, improved).
pub mod external_scanner_v2;
/// Language builder for generating static parsers
pub mod generate;
/// Helper utilities for table generation
pub mod helpers;
/// Language code generation from parse tables.
pub mod language_gen;
/// Lexer generation utilities
pub mod lexer_gen;
/// NODE_TYPES JSON metadata generation.
pub mod node_types;
/// Parser template and parse table code generation.
pub mod parser;
/// .parsetable binary file format writer
#[cfg(feature = "serialization")]
pub mod parsetable_writer;
/// Schema validation for parse tables
pub mod schema;
/// Parse table serialization to various output formats.
pub mod serializer;
/// Typed CST wrapper code generation.
pub mod typed_cst;
/// Parse table and language struct validation.
pub mod validation;

/// Error and Result types for table generation.
pub use error::{Result, TableGenError};

// Re-export commonly used helpers at crate root for ergonomics
/// Utility functions for querying parse tables.
pub use helpers::{collect_token_indices, eof_accepts_or_reduces};

// Re-export key types
/// ABI-compatible language struct builder.
pub use abi_builder::AbiLanguageBuilder;
/// Compressed table types and compressor.
pub use compress::{
    ActionEntry, CompressedActionEntry, CompressedActionTable, CompressedGotoEntry,
    CompressedGotoTable, CompressedParseTable, CompressedTables, GotoEntry, TableCompressor,
};
/// External scanner code generator.
pub use external_scanner::ExternalScannerGenerator;
/// High-level language builder.
pub use generate::LanguageBuilder;
/// NODE_TYPES metadata generator.
pub use node_types::NodeTypesGenerator;
#[cfg(feature = "serialization")]
/// Binary `.parsetable` file format writer and types.
pub use parsetable_writer::{
    FORMAT_VERSION, FeatureFlags, GenerationInfo, GovernanceMetadata, GrammarInfo, MAGIC_NUMBER,
    METADATA_SCHEMA_VERSION, ParserFeatureProfileSnapshot, ParsetableError, ParsetableMetadata,
    ParsetableWriter, TableStatistics,
};
/// Typed CST wrapper code generator.
pub use typed_cst::TypedCstGenerator;
/// ABI language validator and validation error types.
pub use validation::{LanguageValidator, ValidationError};

/// Static language code generation facade.
pub mod static_language;

/// Static Language generator that produces Rust code.
pub use static_language::StaticLanguageGenerator;
