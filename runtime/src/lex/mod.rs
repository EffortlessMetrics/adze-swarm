//! Lexing utilities for the runtime.

/// Character-based scanner implementation.
pub mod char_scanner;
/// Token source trait and data types.
pub mod token_source;
/// Adapter for Tree-sitter lexer functions.
pub mod ts_lexfn_adapter;

pub use char_scanner::CharScanner;
pub use token_source::{Token, TokenSource};
pub use ts_lexfn_adapter::{TSLexState, TsLexFnAdapter, TsLexer};

/// Friendly name for [`TSLexState`]; both names denote the same type.
pub use ts_lexfn_adapter::TSLexState as LexState;
