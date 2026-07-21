//! Lexical metadata and pattern validation for the 0.10 generated-lexer contract.
//!
//! Compiler-significant lexical facts live here rather than being inferred inside
//! `tablegen::lexer_gen`. Matcher generation consumes this metadata in #926.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::fmt;

use crate::{Grammar, SymbolId, TokenPattern};

/// Per-token lexical metadata preserved through IR normalization and serialization.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LexicalMetadata {
    /// When true, leading extras (whitespace) are not skipped before matching.
    #[serde(default)]
    pub immediate: bool,
    /// Lexical precedence for maximal-munch tie-breaking. Higher wins.
    #[serde(default)]
    pub lexical_priority: i16,
}

/// Structured error when a token pattern is outside the supported 0.10 subset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LexicalPatternError {
    /// Token or rule owner name.
    pub owner: String,
    /// The rejected pattern text.
    pub pattern: String,
    /// Human-readable reason and correction guidance.
    pub message: String,
}

impl fmt::Display for LexicalPatternError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "unsupported lexical pattern for '{}': {} (pattern: {:?})",
            self.owner, self.message, self.pattern
        )
    }
}

impl std::error::Error for LexicalPatternError {}

/// Default lexical priority assigned to `token(...)` wrapped rules in Tree-sitter JSON.
pub const TOKEN_WRAPPER_PRIORITY: i16 = 1;

/// Validate a token pattern against the supported 0.10 lexical subset.
///
/// Unsupported constructs fail generation with structured guidance rather than
/// silently omitting matchers at tablegen time.
#[must_use = "validation result must be checked"]
pub fn validate_token_pattern(
    owner: &str,
    pattern: &TokenPattern,
) -> Result<(), LexicalPatternError> {
    match pattern {
        TokenPattern::String(value) => validate_pattern_text(owner, value),
        TokenPattern::Regex(value) => validate_pattern_text(owner, value),
    }
}

fn validate_pattern_text(owner: &str, pattern: &str) -> Result<(), LexicalPatternError> {
    if pattern.is_empty() {
        return Err(LexicalPatternError {
            owner: owner.to_string(),
            pattern: pattern.to_string(),
            message: "zero-width patterns are rejected; use a non-empty literal or regex"
                .to_string(),
        });
    }

    let unsupported = [
        ("(?<=", "positive lookbehind"),
        ("(?<!", "negative lookbehind"),
        ("(?=", "positive lookahead"),
        ("(?!", "negative lookahead"),
        ("(?P<", "named capture groups"),
        ("(?<", "named capture groups"),
        ("\\1", "backreferences"),
        ("\\2", "backreferences"),
        ("\\3", "backreferences"),
    ];

    for (needle, construct) in unsupported {
        if pattern.contains(needle) {
            return Err(LexicalPatternError {
                owner: owner.to_string(),
                pattern: pattern.to_string(),
                message: format!(
                    "{construct} is not in the supported 0.10 regex subset; simplify the pattern"
                ),
            });
        }
    }

    Ok(())
}

impl Grammar {
    /// Returns lexical metadata for a token, or default metadata when unset.
    #[must_use]
    pub fn lexical_metadata_for(&self, symbol: SymbolId) -> LexicalMetadata {
        self.lexical_metadata
            .get(&symbol)
            .cloned()
            .unwrap_or_default()
    }

    /// Set or replace lexical metadata for a token symbol.
    pub fn set_lexical_metadata(&mut self, symbol: SymbolId, metadata: LexicalMetadata) {
        self.lexical_metadata.insert(symbol, metadata);
    }

    /// Mark a token as immediate (no leading extras).
    pub fn mark_token_immediate(&mut self, symbol: SymbolId) {
        let mut meta = self.lexical_metadata_for(symbol);
        meta.immediate = true;
        self.lexical_metadata.insert(symbol, meta);
    }

    /// Apply the default lexical-priority boost used by Tree-sitter `token(...)` wrappers.
    pub fn boost_token_lexical_priority(&mut self, symbol: SymbolId, priority: i16) {
        let mut meta = self.lexical_metadata_for(symbol);
        meta.lexical_priority = meta.lexical_priority.max(priority);
        self.lexical_metadata.insert(symbol, meta);
    }

    /// Resolve the authoritative word-token symbol, if declared.
    #[must_use]
    pub fn word_token_symbol(&self) -> Option<SymbolId> {
        self.word_token
    }

    /// Validate all token patterns in this grammar.
    #[must_use = "validation result must be checked"]
    pub fn validate_lexical_patterns(&self) -> Result<(), Vec<LexicalPatternError>> {
        let mut errors = Vec::new();
        for (symbol_id, token) in &self.tokens {
            if let Err(err) = validate_token_pattern(&token.name, &token.pattern) {
                errors.push(LexicalPatternError {
                    owner: self
                        .rule_names
                        .get(symbol_id)
                        .cloned()
                        .unwrap_or_else(|| token.name.clone()),
                    pattern: err.pattern,
                    message: err.message,
                });
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

/// Deterministic lexical tie-breaking: higher priority first, then lower symbol id.
#[must_use]
pub fn compare_lexical_priority(
    left: (&SymbolId, &LexicalMetadata),
    right: (&SymbolId, &LexicalMetadata),
) -> std::cmp::Ordering {
    right
        .1
        .lexical_priority
        .cmp(&left.1.lexical_priority)
        .then_with(|| left.0.0.cmp(&right.0.0))
}

/// Collect lexical metadata entries sorted for deterministic iteration.
#[must_use]
pub fn sorted_lexical_metadata(
    metadata: &IndexMap<SymbolId, LexicalMetadata>,
) -> Vec<(SymbolId, LexicalMetadata)> {
    let mut entries: Vec<_> = metadata
        .iter()
        .map(|(id, meta)| (*id, meta.clone()))
        .collect();
    entries.sort_by(|(left_id, left_meta), (right_id, right_meta)| {
        right_meta
            .lexical_priority
            .cmp(&left_meta.lexical_priority)
            .then_with(|| left_id.0.cmp(&right_id.0))
    });
    entries
}
