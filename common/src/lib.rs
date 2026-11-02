// Common crate is pure-Rust - no unsafe needed
#![forbid(unsafe_code)]
#![cfg_attr(feature = "strict_docs", deny(missing_docs))]
#![cfg_attr(not(feature = "strict_docs"), allow(missing_docs))]

//! Shared utility entrypoint for macro and tool parsing behavior.

pub mod syntax;

pub use syntax::*;

/// Canonical public token-kind name for an inline regex leaf.
///
/// Generation and typed extraction share this name so dispatch follows the
/// selected native token identity rather than re-evaluating the regex.
pub fn pattern_token_name(pattern: &str) -> String {
    format!("/{pattern}/")
}
