//! Parser implementation with Tree-sitter-compatible API.
//!
//! The parser facade is intentionally small; parsing backends, pure-Rust GLR
//! state, and `.parsetable` loading live in focused sibling modules.

#[cfg(feature = "pure-rust")]
mod glr;
mod language_mode;
#[cfg(all(feature = "pure-rust", feature = "serialization"))]
mod parsetable;

use crate::{error::ParseError, language::Language, tree::Tree};
#[cfg(all(feature = "pure-rust", feature = "serialization"))]
use adze_parsetable_metadata::ParsetableMetadata;
#[cfg(feature = "pure-rust")]
use glr::GLRState;
use std::time::Duration;

/// A parser that can parse text into a syntax [`Tree`] using a [`Language`].
///
/// The parser supports two modes:
///
/// - **Language mode** (default): Set a [`Language`] via [`set_language`](Self::set_language),
///   then call [`parse`](Self::parse) or [`parse_utf8`](Self::parse_utf8).
/// - **GLR mode** (requires `pure-rust` feature): Set a parse table via
///   `set_glr_table` for pure-Rust GLR parsing.
///
/// # Examples
///
/// ```ignore
/// use adze_runtime::Parser;
///
/// let mut parser = Parser::new();
/// parser.set_language(language)?;
/// let tree = parser.parse(b"1 + 2", None)?;
/// println!("{:?}", tree.root_node());
/// ```
#[derive(Debug)]
pub struct Parser {
    language: Option<Language>,
    timeout: Option<Duration>,
    #[cfg(feature = "arenas")]
    arena: Option<bumpalo::Bump>,
    /// GLR mode state (Phase 3.1)
    #[cfg(feature = "pure-rust")]
    glr_state: Option<GLRState>,
    /// Parsed metadata from `.parsetable` load.
    #[cfg(all(feature = "pure-rust", feature = "serialization"))]
    parsetable_metadata: Option<ParsetableMetadata>,
}

impl Parser {
    /// Create a new parser.
    pub fn new() -> Self {
        Self {
            language: None,
            timeout: None,
            #[cfg(feature = "arenas")]
            arena: None,
            #[cfg(feature = "pure-rust")]
            glr_state: None,
            #[cfg(all(feature = "pure-rust", feature = "serialization"))]
            parsetable_metadata: None,
        }
    }

    /// Set the language for parsing.
    ///
    /// In GLR mode, validates that the language provides a parse table and tokenizer.
    pub fn set_language(&mut self, language: Language) -> Result<(), ParseError> {
        #[cfg(feature = "glr")]
        {
            if language.parse_table.is_none() {
                return Err(ParseError::with_msg("Language has no parse table"));
            }
            if language.tokenize.is_none() {
                return Err(ParseError::with_msg("Language has no tokenizer"));
            }
        }
        if language.symbol_metadata.is_empty() {
            return Err(ParseError::with_msg("Language has no symbol metadata"));
        }
        // TODO: Validate language version compatibility
        self.language = Some(language);
        #[cfg(feature = "pure-rust")]
        {
            self.glr_state = None;
        }
        #[cfg(all(feature = "pure-rust", feature = "serialization"))]
        {
            self.parsetable_metadata = None;
        }
        Ok(())
    }

    /// Get the current language.
    pub fn language(&self) -> Option<&Language> {
        self.language.as_ref()
    }

    /// Set a timeout for parsing operations.
    pub fn set_timeout(&mut self, timeout: Duration) {
        self.timeout = Some(timeout);
    }

    /// Get the current timeout.
    pub fn timeout(&self) -> Option<Duration> {
        self.timeout
    }

    /// Parse the given input text.
    ///
    /// If `old_tree` is provided, performs incremental parsing.
    ///
    /// # Mode Selection
    ///
    /// - If GLR mode is active (`set_glr_table()` was called), uses pure-Rust GLR engine
    /// - Otherwise, uses language-based parsing (`set_language()` was called)
    pub fn parse(
        &mut self,
        input: impl AsRef<[u8]>,
        old_tree: Option<&Tree>,
    ) -> Result<Tree, ParseError> {
        let input = input.as_ref();

        #[cfg(feature = "pure-rust")]
        if self.glr_state.is_some() {
            return self.parse_glr(input, old_tree);
        }

        self.parse_language(input, old_tree)
    }

    /// Parse a UTF-8 string input.
    ///
    /// Convenience wrapper around [`parse`](Self::parse) that accepts `&str`.
    pub fn parse_utf8(&mut self, input: &str, old_tree: Option<&Tree>) -> Result<Tree, ParseError> {
        self.parse(input.as_bytes(), old_tree)
    }

    /// Reset the parser state, clearing any internal caches or arenas.
    pub fn reset(&mut self) {
        #[cfg(feature = "arenas")]
        if let Some(arena) = &mut self.arena {
            arena.reset();
        }
    }
}

impl Default for Parser {
    fn default() -> Self {
        Self::new()
    }
}
