//! Language-backed parser flow.
//!
//! This module owns the standard parser mode that routes through a configured
//! [`Language`] and optionally uses incremental GLR support.

#[cfg(feature = "glr")]
use crate::builder::forest_to_tree;
#[cfg(feature = "glr")]
use crate::engine::parse_full as engine_parse_full;
#[cfg(all(feature = "glr", feature = "incremental_glr"))]
use crate::engine::parse_incremental as engine_parse_incremental;
use crate::{error::ParseError, language::Language, tree::Tree};

use super::Parser;

impl Parser {
    pub(super) fn parse_language(
        &mut self,
        input: &[u8],
        old_tree: Option<&Tree>,
    ) -> Result<Tree, ParseError> {
        let language_ptr =
            self.language.as_ref().ok_or(ParseError::no_language())? as *const Language;

        // SAFETY: we only read from the language while holding an immutable reference.
        let language = unsafe { &*language_ptr };

        let mut tree = if let Some(old) = old_tree {
            self.parse_incremental(language, input, old)?
        } else {
            self.parse_full(language, input)?
        };
        tree.set_language(language.clone());
        tree.set_source(input.to_vec());
        Ok(tree)
    }

    fn parse_full(&mut self, language: &Language, input: &[u8]) -> Result<Tree, ParseError> {
        #[cfg(feature = "glr")]
        {
            let forest = engine_parse_full(language, input)?;
            Ok(forest_to_tree(forest))
        }

        #[cfg(not(feature = "glr"))]
        {
            let _ = (language, input);
            Err(ParseError::with_msg("GLR core feature not enabled"))
        }
    }

    #[cfg(feature = "incremental_glr")]
    fn parse_incremental(
        &mut self,
        language: &Language,
        input: &[u8],
        old_tree: &Tree,
    ) -> Result<Tree, ParseError> {
        #[cfg(all(feature = "glr", feature = "incremental_glr"))]
        {
            // Optimization: return early if input hasn't changed.
            if let Some(old_src) = old_tree.source_bytes()
                && old_src == input
            {
                return Ok(old_tree.clone());
            }
            let forest = engine_parse_incremental(language, input, old_tree)?;
            Ok(forest_to_tree(forest))
        }

        #[cfg(not(feature = "glr"))]
        {
            let _ = (language, input, old_tree);
            Err(ParseError::with_msg("GLR core feature not enabled"))
        }
    }

    #[cfg(not(feature = "incremental_glr"))]
    fn parse_incremental(
        &mut self,
        language: &Language,
        input: &[u8],
        _old_tree: &Tree,
    ) -> Result<Tree, ParseError> {
        // Fall back to full parse when incremental is disabled.
        self.parse_full(language, input)
    }
}
