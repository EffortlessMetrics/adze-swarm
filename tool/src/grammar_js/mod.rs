//! Grammar.js compatibility layer for parsing Tree-sitter grammar definitions
//!
//! This module provides parsing and conversion of JavaScript-based grammar.js files
//! to Adze's internal representation.

use anyhow::{Result, bail};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub mod converter;
pub mod helpers;
pub mod json_converter;
pub mod parser;
pub mod parser_v2;
pub mod parser_v2_test;
pub mod parser_v3;

pub use converter::GrammarJsConverter;
pub use helpers::hidden_pattern_token_name;
pub use parser::parse_grammar_js;
pub use parser_v2::parse_grammar_js_v2;
pub use parser_v3::GrammarJsParserV3;

/// Represents a Tree-sitter grammar.js file structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrammarJs {
    /// The name of the language
    pub name: String,

    /// The word token (typically identifier)
    pub word: Option<String>,

    /// Rules that should be made inline
    pub inline: Vec<String>,

    /// Conflict sets
    pub conflicts: Vec<Vec<String>>,

    /// Extra tokens (usually whitespace and comments)
    pub extras: Vec<Rule>,

    /// External tokens defined in external scanner
    pub externals: Vec<ExternalToken>,

    /// Precedence levels
    pub precedences: Vec<Vec<(String, i32)>>,

    /// Grammar rules
    pub rules: IndexMap<String, Rule>,

    /// Supertypes for the grammar
    pub supertypes: Vec<String>,

    /// Explicit start symbol from `#[adze::language]` or imported grammar metadata.
    #[serde(default)]
    pub start_symbol: Option<String>,

    /// Explicit pattern-wrapper nonterminal to backing-token relations by name.
    #[serde(default)]
    pub wrapper_token_relations: IndexMap<String, String>,
}

/// Represents a grammar rule
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Rule {
    /// A literal string
    #[serde(rename = "STRING")]
    String { value: String },

    /// A regular expression pattern
    #[serde(rename = "PATTERN")]
    Pattern { value: String },

    /// A reference to another rule
    #[serde(rename = "SYMBOL")]
    Symbol { name: String },

    /// A blank rule (no content)
    #[serde(rename = "BLANK")]
    Blank,

    /// A sequence of rules
    #[serde(rename = "SEQ")]
    Seq { members: Vec<Rule> },

    /// A choice between rules
    #[serde(rename = "CHOICE")]
    Choice { members: Vec<Rule> },

    /// An optional rule
    #[serde(rename = "OPTIONAL")]
    Optional { value: Box<Rule> },

    /// Zero or more repetitions
    #[serde(rename = "REPEAT")]
    Repeat { content: Box<Rule> },

    /// One or more repetitions
    #[serde(rename = "REPEAT1")]
    Repeat1 { content: Box<Rule> },

    /// Immediate token (no whitespace)
    #[serde(rename = "IMMEDIATE_TOKEN")]
    ImmediateToken { content: Box<Rule> },

    /// Token with precedence
    #[serde(rename = "TOKEN")]
    Token { content: Box<Rule> },

    /// Rule with precedence
    #[serde(rename = "PREC")]
    Prec { value: i32, content: Box<Rule> },

    /// Rule with dynamic precedence
    #[serde(rename = "PREC_DYNAMIC")]
    PrecDynamic { value: i32, content: Box<Rule> },

    /// Rule with left associativity
    #[serde(rename = "PREC_LEFT")]
    PrecLeft { value: i32, content: Box<Rule> },

    /// Rule with right associativity
    #[serde(rename = "PREC_RIGHT")]
    PrecRight { value: i32, content: Box<Rule> },

    /// Aliased rule
    #[serde(rename = "ALIAS")]
    Alias {
        content: Box<Rule>,
        value: String,
        named: bool,
    },

    /// Field assignment
    #[serde(rename = "FIELD")]
    Field { name: String, content: Box<Rule> },
}

/// Represents an external token
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalToken {
    pub name: String,
    pub symbol: String,
}

impl GrammarJs {
    /// Create a new empty grammar
    pub fn new(name: String) -> Self {
        Self {
            name,
            word: None,
            inline: Vec::new(),
            conflicts: Vec::new(),
            extras: Vec::new(),
            externals: Vec::new(),
            precedences: Vec::new(),
            rules: IndexMap::new(),
            supertypes: Vec::new(),
            start_symbol: None,
            wrapper_token_relations: IndexMap::new(),
        }
    }

    /// Validate the grammar structure
    pub fn validate(&self) -> Result<()> {
        // Check that all referenced symbols exist
        for (rule_name, rule) in &self.rules {
            self.validate_rule(rule, rule_name)?;
        }

        // Check that word token exists if specified
        if let Some(word) = &self.word
            && !self.rules.contains_key(word)
        {
            bail!("Word token '{}' not found in rules", word);
        }

        // Check inline rules exist
        for inline in &self.inline {
            if !self.rules.contains_key(inline) {
                bail!("Inline rule '{}' not found in rules", inline);
            }
        }

        // Check conflict rules exist
        for conflict_set in &self.conflicts {
            for rule in conflict_set {
                if !self.rules.contains_key(rule) {
                    bail!("Conflict rule '{}' not found in rules", rule);
                }
            }
        }

        Ok(())
    }

    fn validate_rule(&self, rule: &Rule, context: &str) -> Result<()> {
        match rule {
            Rule::Symbol { name } if !self.rules.contains_key(name) && !self.is_external(name) => {
                bail!("Symbol '{}' referenced in '{}' not found", name, context);
            }
            Rule::Symbol { .. } => {}
            Rule::Seq { members } | Rule::Choice { members } => {
                for member in members {
                    self.validate_rule(member, context)?;
                }
            }
            Rule::Optional { value }
            | Rule::Repeat { content: value }
            | Rule::Repeat1 { content: value }
            | Rule::ImmediateToken { content: value }
            | Rule::Token { content: value }
            | Rule::Prec { content: value, .. }
            | Rule::PrecDynamic { content: value, .. }
            | Rule::PrecLeft { content: value, .. }
            | Rule::PrecRight { content: value, .. } => {
                self.validate_rule(value, context)?;
            }
            Rule::Alias { content, .. } | Rule::Field { content, .. } => {
                self.validate_rule(content, context)?;
            }
            _ => {}
        }
        Ok(())
    }

    fn is_external(&self, name: &str) -> bool {
        self.externals.iter().any(|ext| ext.name == name)
    }
}

/// Convert a JSON value to a GrammarJs structure
pub fn from_json(value: &Value) -> Result<GrammarJs> {
    // Use the Tree-sitter JSON converter
    json_converter::from_tree_sitter_json(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_grammar_validation() {
        let mut grammar = GrammarJs::new("test".to_string());

        // Add a simple rule
        grammar.rules.insert(
            "identifier".to_string(),
            Rule::Pattern {
                value: r"[a-zA-Z_]\w*".to_string(),
            },
        );

        // Valid reference
        grammar.rules.insert(
            "variable".to_string(),
            Rule::Symbol {
                name: "identifier".to_string(),
            },
        );

        assert!(grammar.validate().is_ok());

        // Invalid reference
        grammar.rules.insert(
            "invalid".to_string(),
            Rule::Symbol {
                name: "nonexistent".to_string(),
            },
        );

        assert!(grammar.validate().is_err());
    }
}
