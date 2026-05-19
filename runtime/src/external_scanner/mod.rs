//! External scanner runtime for Adze.
//! This module provides the runtime support for custom lexing logic.
#![cfg_attr(feature = "strict_docs", allow(missing_docs))]

#[cfg(feature = "external_scanners")]
pub mod adapter;

#[cfg(feature = "external_scanners")]
pub mod lifecycle;

mod scanners;
mod types;

pub use scanners::{CommentScanner, StringScanner};
pub use types::{
    DynExternalScanner, ExternalScanner, ExternalScannerRuntime, ExternalScannerState, Lexer,
    ScanResult,
};

#[cfg(test)]
mod tests;
