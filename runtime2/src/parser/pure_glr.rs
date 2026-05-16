//! Pure-Rust GLR parser mode.
//!
//! This module owns GLR-specific parser state, parse-table validation, token
//! scanning, forest conversion, and the synthetic [`Language`] used for symbol
//! name lookup.

use crate::{error::ParseError, language::Language, tree::Tree};

use super::Parser;

/// GLR parsing state (pure-Rust mode, bypasses TSLanguage).
#[derive(Debug)]
pub(super) struct GlrState {
    /// Direct reference to ParseTable from glr-core.
    pub(super) parse_table: &'static adze_glr_core::ParseTable,
    /// Symbol metadata for tree construction.
    pub(super) symbol_metadata: Vec<crate::language::SymbolMetadata>,
    /// Token patterns for tokenizer (Phase 3.2).
    pub(super) token_patterns: Option<Vec<crate::tokenizer::TokenPattern>>,
}

impl Parser {
    /// Parse using pure-Rust GLR engine (Phase 3.1).
    ///
    /// This method is called when the parser is in GLR mode (via `set_glr_table()`).
    pub(super) fn parse_glr(
        &mut self,
        input: &[u8],
        _old_tree: Option<&Tree>,
    ) -> Result<Tree, ParseError> {
        use crate::forest_converter::{DisambiguationStrategy, ForestConverter};
        use crate::glr_engine::{GLRConfig, GLREngine};
        use crate::tokenizer::{Tokenizer, WhitespaceMode};

        let glr_state = self
            .glr_state
            .as_ref()
            .ok_or_else(|| ParseError::with_msg("No GLR state"))?;

        let tokens = if let Some(ref patterns) = glr_state.token_patterns {
            let tokenizer = Tokenizer::new(patterns.clone(), WhitespaceMode::Skip);
            tokenizer
                .scan(input)
                .map_err(|e| ParseError::with_msg(&e.to_string()))?
        } else {
            vec![crate::Token {
                kind: 0,
                start: input.len() as u32,
                end: input.len() as u32,
            }]
        };

        let config = GLRConfig::default();
        let mut engine = GLREngine::new(glr_state.parse_table, config);
        let forest = engine.parse(&tokens)?;

        let converter = ForestConverter::new(DisambiguationStrategy::PreferShift);
        let mut tree = converter
            .to_tree(&forest, input)
            .map_err(|e| ParseError::with_msg(&e.to_string()))?;

        tree.set_language(Self::build_language_from_parse_table(glr_state.parse_table));
        tree.set_source(input.to_vec());

        Ok(tree)
    }

    /// Build a Language from ParseTable for symbol name resolution (Phase 3.3).
    fn build_language_from_parse_table(
        parse_table: &'static adze_glr_core::ParseTable,
    ) -> Language {
        let mut symbol_names = symbol_names_from_parse_table(parse_table);
        if symbol_names.len() < parse_table.symbol_count {
            symbol_names.resize(parse_table.symbol_count, String::from("unknown"));
        }

        Language {
            version: 1,
            symbol_count: parse_table.symbol_count as u32,
            field_count: 0,
            max_alias_sequence_length: 0,
            #[cfg(feature = "glr")]
            parse_table: Some(parse_table),
            #[cfg(not(feature = "glr"))]
            parse_table: crate::language::ParseTable::default(),
            #[cfg(feature = "glr")]
            tokenize: None,
            symbol_names,
            symbol_metadata: Vec::new(),
            field_names: Vec::new(),
            #[cfg(feature = "external_scanners")]
            external_scanner: None,
        }
    }

    /// Set the GLR parse table directly (pure-Rust mode, bypasses TSLanguage).
    ///
    /// This is the Phase 3.1 API that enables GLR parsing without TSLanguage encoding.
    ///
    /// # Errors
    ///
    /// Returns an error when `table` violates basic parse-table invariants.
    #[cfg_attr(docsrs, doc(cfg(feature = "pure-rust")))]
    pub fn set_glr_table(
        &mut self,
        table: &'static adze_glr_core::ParseTable,
    ) -> Result<(), ParseError> {
        validate_parse_table(table)?;

        self.glr_state = Some(GlrState {
            parse_table: table,
            symbol_metadata: Vec::new(),
            token_patterns: None,
        });

        #[cfg(all(feature = "pure-rust", feature = "serialization"))]
        {
            self.parsetable_metadata = None;
        }

        self.language = None;

        Ok(())
    }

    /// Set symbol metadata for GLR mode.
    ///
    /// Symbol metadata is needed for tree construction in GLR mode.
    ///
    /// # Errors
    ///
    /// Returns an error if `set_glr_table()` was not called first.
    #[cfg_attr(docsrs, doc(cfg(feature = "pure-rust")))]
    pub fn set_symbol_metadata(
        &mut self,
        metadata: Vec<crate::language::SymbolMetadata>,
    ) -> Result<(), ParseError> {
        let glr_state = self
            .glr_state
            .as_mut()
            .ok_or_else(|| ParseError::with_msg("No GLR state: call set_glr_table() first"))?;

        glr_state.symbol_metadata = metadata;
        Ok(())
    }

    /// Set token patterns for GLR mode tokenizer (Phase 3.2).
    ///
    /// Token patterns define how to scan input into tokens for the GLR parser.
    ///
    /// # Errors
    ///
    /// Returns an error if `set_glr_table()` was not called first.
    #[cfg_attr(docsrs, doc(cfg(feature = "pure-rust")))]
    pub fn set_token_patterns(
        &mut self,
        patterns: Vec<crate::tokenizer::TokenPattern>,
    ) -> Result<(), ParseError> {
        let glr_state = self
            .glr_state
            .as_mut()
            .ok_or_else(|| ParseError::with_msg("No GLR state: call set_glr_table() first"))?;

        glr_state.token_patterns = Some(patterns);
        Ok(())
    }

    /// Check if parser is in GLR mode.
    ///
    /// Returns `true` if `set_glr_table()` was called and GLR mode is active.
    #[cfg_attr(docsrs, doc(cfg(feature = "pure-rust")))]
    pub fn is_glr_mode(&self) -> bool {
        self.glr_state.is_some()
    }
}

fn validate_parse_table(table: &adze_glr_core::ParseTable) -> Result<(), ParseError> {
    if table.state_count == 0 {
        return Err(ParseError::with_msg("ParseTable has 0 states"));
    }

    if table.action_table.len() != table.state_count {
        return Err(ParseError::with_msg(&format!(
            "ParseTable invariant violation: state_count ({}) != action_table.len() ({})",
            table.state_count,
            table.action_table.len()
        )));
    }

    Ok(())
}

fn symbol_names_from_parse_table(parse_table: &adze_glr_core::ParseTable) -> Vec<String> {
    let max_terminal_id = parse_table
        .grammar
        .tokens
        .keys()
        .map(|id| id.0 as usize)
        .max()
        .unwrap_or(0);
    let max_nonterminal_id = parse_table
        .grammar
        .rule_names
        .keys()
        .map(|id| id.0 as usize)
        .max()
        .unwrap_or(0);
    let vec_size = max_terminal_id.max(max_nonterminal_id) + 1;
    let mut symbol_names = vec![String::from("unknown"); vec_size];

    for (symbol_id, token) in &parse_table.grammar.tokens {
        symbol_names[symbol_id.0 as usize] = token.name.clone();
    }

    for (symbol_id, name) in &parse_table.grammar.rule_names {
        symbol_names[symbol_id.0 as usize] = name.clone();
    }

    symbol_names
}
