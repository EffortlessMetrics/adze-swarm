// IR crate should be safe - no unsafe needed for grammar representation
#![forbid(unsafe_code)]
#![cfg_attr(feature = "strict_docs", deny(missing_docs))]
#![cfg_attr(not(feature = "strict_docs"), warn(missing_docs))]

//! Grammar Intermediate Representation for Adze
//! This module provides GLR-aware data structures for representing grammars

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Error types and Result alias for IR operations.
pub mod error;
/// Error types for grammar IR operations.
pub use error::{IrError, Result as IrResult};

/// Grammar optimization utilities
pub mod optimizer;
/// Grammar optimization utilities and statistics.
pub use optimizer::{GrammarOptimizer, OptimizationStats, optimize_grammar};

/// Grammar validation utilities
pub mod validation;
/// Grammar validation types and results.
pub use validation::{GrammarValidator, ValidationError, ValidationResult, ValidationWarning};

/// Debug macros for development
pub mod debug_macros;
/// Symbol registry for managing grammar symbols
pub mod symbol_registry;
/// Symbol registry for deterministic ID assignment.
pub use symbol_registry::{SymbolInfo, SymbolRegistry};
/// Builder API for programmatically constructing grammars
pub mod builder;

/// Core grammar representation supporting all Tree-sitter features including GLR
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Grammar {
    /// Grammar name
    pub name: String,
    /// Production rules indexed by left-hand side symbol
    pub rules: IndexMap<SymbolId, Vec<Rule>>,
    /// Token definitions
    pub tokens: IndexMap<SymbolId, Token>,
    /// Precedence declarations
    pub precedences: Vec<Precedence>,
    /// Conflict resolution declarations
    pub conflicts: Vec<ConflictDeclaration>,
    /// External scanner tokens
    pub externals: Vec<ExternalToken>,
    /// Extra tokens (e.g., whitespace, comments)
    pub extras: Vec<SymbolId>,
    /// Field names maintained in lexicographic order
    pub fields: IndexMap<FieldId, String>,
    /// Supertype symbols
    pub supertypes: Vec<SymbolId>,
    /// Rules to inline during generation
    pub inline_rules: Vec<SymbolId>,
    /// Alias sequences for productions
    pub alias_sequences: IndexMap<ProductionId, AliasSequence>,
    /// Maps rule IDs to production IDs
    pub production_ids: IndexMap<RuleId, ProductionId>,
    /// Maximum alias sequence length
    pub max_alias_sequence_length: usize,
    /// Maps symbol IDs to rule names
    pub rule_names: IndexMap<SymbolId, String>,
    /// Centralized symbol registry
    pub symbol_registry: Option<SymbolRegistry>,
}

impl Grammar {
    /// Add a rule to the grammar
    pub fn add_rule(&mut self, rule: Rule) {
        self.rules.entry(rule.lhs).or_default().push(rule);
    }

    /// Get all rules for a given LHS symbol
    pub fn get_rules_for_symbol(&self, symbol: SymbolId) -> Option<&Vec<Rule>> {
        self.rules.get(&symbol)
    }

    /// Iterate over all rules in the grammar
    pub fn all_rules(&self) -> impl Iterator<Item = &Rule> {
        self.rules.values().flat_map(|rules| rules.iter())
    }

    /// Get the start symbol (LHS of the first rule)
    pub fn start_symbol(&self) -> Option<SymbolId> {
        // For Tree-sitter compatibility, look for "source_file" symbol
        if let Some(source_file_id) = self.find_symbol_by_name("source_file")
            && self.rules.contains_key(&source_file_id)
        {
            return Some(source_file_id);
        }

        // In adze, source_file is often just a reference to the actual language type
        // So let's look for the language type that's marked with #[adze::language]
        // This is typically the first non-terminal that has rules

        // Try common patterns first
        for name in &["Expression", "Statement", "Program", "Module"] {
            if let Some(symbol_id) = self.find_symbol_by_name(name)
                && self.rules.contains_key(&symbol_id)
            {
                return Some(symbol_id);
            }
        }

        // Otherwise, use the first symbol that has rules and isn't a leaf/token
        for (symbol_id, rules) in &self.rules {
            // Skip symbols that look like internal/generated names
            if let Some(name) = self.rule_names.get(symbol_id)
                && !name.contains('_')
                && !rules.is_empty()
            {
                return Some(*symbol_id);
            }
        }

        // Final fallback: just use the first symbol with rules
        self.rules.keys().next().copied()
    }

    /// Find a symbol by its name in rule_names
    pub fn find_symbol_by_name(&self, name: &str) -> Option<SymbolId> {
        for (symbol_id, symbol_name) in &self.rule_names {
            if symbol_name == name {
                return Some(*symbol_id);
            }
        }
        None
    }

    /// Build or get the symbol registry
    pub fn get_or_build_registry(&mut self) -> &SymbolRegistry {
        match self.symbol_registry {
            Some(ref registry) => registry,
            None => {
                let registry = self.build_registry();
                self.symbol_registry.insert(registry)
            }
        }
    }

    /// Check for empty string terminals (separate from main validate)
    #[must_use = "validation result must be checked"]
    pub fn check_empty_terminals(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        // Check for empty string terminals
        for (id, token) in &self.tokens {
            match &token.pattern {
                TokenPattern::String(s) if s.is_empty() => {
                    errors.push(format!(
                        "Token '{}' (id={}) has empty string pattern",
                        token.name, id.0
                    ));
                }
                TokenPattern::Regex(r) if r.is_empty() => {
                    errors.push(format!(
                        "Token '{}' (id={}) has empty regex pattern",
                        token.name, id.0
                    ));
                }
                _ => {}
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Build a new symbol registry from the grammar
    pub fn build_registry(&self) -> SymbolRegistry {
        let mut registry = SymbolRegistry::new();

        // Sort tokens deterministically: underscore-prefixed last
        let mut token_entries: Vec<_> = self.tokens.iter().collect();
        token_entries.sort_by(|(_, left), (_, right)| {
            left.name
                .starts_with('_')
                .cmp(&right.name.starts_with('_'))
                .then_with(|| left.name.cmp(&right.name))
        });

        // Register all tokens
        for (symbol_id, token) in token_entries {
            let metadata = SymbolMetadata {
                visible: !token.name.starts_with('_'),
                named: false,
                hidden: self.extras.contains(symbol_id),
                terminal: true,
            };
            registry.register(&token.name, metadata);
        }

        // Sort non-terminals deterministically
        let mut rule_entries: Vec<_> = self.rule_names.iter().collect();
        rule_entries.sort_by_key(|(_, name)| *name);

        // Register all non-terminals
        for (symbol_id, name) in rule_entries {
            if !self.tokens.contains_key(symbol_id) {
                let metadata = SymbolMetadata {
                    visible: !name.starts_with('_'),
                    named: true,
                    hidden: name.starts_with('_'),
                    terminal: false,
                };
                registry.register(name, metadata);
            }
        }

        // Register externals
        for external in &self.externals {
            let metadata = SymbolMetadata {
                visible: true,
                named: false,
                hidden: false,
                terminal: true,
            };
            registry.register(&external.name, metadata);
        }

        registry
    }
}

/// Grammar rule supporting GLR multiple actions per state
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rule {
    /// Left-hand side symbol
    pub lhs: SymbolId,
    /// Right-hand side symbols
    pub rhs: Vec<Symbol>,
    /// Precedence if specified
    pub precedence: Option<PrecedenceKind>,
    /// Associativity if specified
    pub associativity: Option<Associativity>,
    /// Field to position mapping
    pub fields: Vec<(FieldId, usize)>,
    /// Production ID
    pub production_id: ProductionId,
}

/// Precedence supporting both static and dynamic precedence (PREC_DYNAMIC)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PrecedenceKind {
    /// Static precedence
    Static(i16),
    /// Dynamic precedence
    Dynamic(i16),
}

/// Token with fragile flag for lexical vs parse conflicts
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Token {
    /// Token name
    pub name: String,
    /// Token pattern (string or regex)
    pub pattern: TokenPattern,
    /// TSFragile flag for lexical vs parse conflicts
    pub fragile: bool,
}

/// Token pattern representation
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TokenPattern {
    /// String literal pattern
    String(String),
    /// Regular expression pattern
    Regex(String),
}

/// Grammar symbol types
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Symbol {
    /// Terminal symbol
    Terminal(SymbolId),
    /// Non-terminal symbol
    NonTerminal(SymbolId),
    /// External scanner symbol
    External(SymbolId),
    /// Optional symbol (zero or one)
    Optional(Box<Symbol>),
    /// Zero or more repetitions
    Repeat(Box<Symbol>),
    /// One or more repetitions
    RepeatOne(Box<Symbol>),
    /// Choice between symbols
    Choice(Vec<Symbol>),
    /// Sequence of symbols
    Sequence(Vec<Symbol>),
    /// Empty production
    Epsilon,
}

/// Alias sequence for node renaming
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AliasSequence {
    /// Aliases for each position
    pub aliases: Vec<Option<String>>,
}

/// Precedence declaration
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Precedence {
    /// Precedence level
    pub level: i16,
    /// Associativity for this level
    pub associativity: Associativity,
    /// Symbols at this precedence level
    pub symbols: Vec<SymbolId>,
}

/// Associativity for conflict resolution
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Associativity {
    /// Left associative
    Left,
    /// Right associative
    Right,
    /// Non-associative
    None,
}

/// Conflict declaration for GLR handling
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConflictDeclaration {
    /// Conflicting symbols
    pub symbols: Vec<SymbolId>,
    /// Conflict resolution strategy
    pub resolution: ConflictResolution,
}

/// How to resolve conflicts
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConflictResolution {
    /// Resolve by precedence
    Precedence(PrecedenceKind),
    /// Resolve by associativity
    Associativity(Associativity),
    /// Allow GLR fork/merge
    GLR,
}

/// External token declaration
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalToken {
    /// External token name
    pub name: String,
    /// Symbol ID for the external token
    pub symbol_id: SymbolId,
}

// Type-safe IDs
/// Symbol identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SymbolId(pub u16);

/// Rule identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct RuleId(pub u16);

/// State identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct StateId(pub u16);

/// Field identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FieldId(pub u16);

/// Production identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ProductionId(pub u16);

// Display implementations for debugging
impl fmt::Display for SymbolId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Symbol({})", self.0)
    }
}

impl fmt::Display for RuleId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Rule({})", self.0)
    }
}

impl fmt::Display for StateId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "State({})", self.0)
    }
}

impl fmt::Display for FieldId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Field({})", self.0)
    }
}

impl fmt::Display for ProductionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Production({})", self.0)
    }
}

/// Metadata for a symbol in the language
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SymbolMetadata {
    /// Whether the symbol is visible
    pub visible: bool,
    /// Whether the symbol is named
    pub named: bool,
    /// Whether the symbol is hidden
    pub hidden: bool,
    /// Whether the symbol is a terminal
    pub terminal: bool,
}

/// Grammar validation and processing
impl Grammar {
    /// Create a new empty grammar
    pub fn new(name: String) -> Self {
        Self {
            name,
            rules: IndexMap::new(),
            tokens: IndexMap::new(),
            precedences: Vec::new(),
            conflicts: Vec::new(),
            externals: Vec::new(),
            extras: Vec::new(),
            fields: IndexMap::new(),
            supertypes: Vec::new(),
            inline_rules: Vec::new(),
            alias_sequences: IndexMap::new(),
            production_ids: IndexMap::new(),
            max_alias_sequence_length: 0,
            rule_names: IndexMap::new(),
            symbol_registry: None,
        }
    }

    /// Extract IR from procedural macro data
    #[must_use = "parsing result must be checked"]
    pub fn from_macro_output(data: &str) -> Result<Self, GrammarError> {
        // This will be implemented to parse the output from adze macros
        serde_json::from_str(data).map_err(GrammarError::ParseError)
    }

    /// Helper to validate a symbol recursively
    fn validate_symbol(&self, symbol: &Symbol) -> Result<(), GrammarError> {
        match symbol {
            Symbol::Terminal(id) | Symbol::NonTerminal(id) => {
                if !self.rules.contains_key(id) && !self.tokens.contains_key(id) {
                    return Err(GrammarError::UnresolvedSymbol(*id));
                }
            }
            Symbol::External(id) => {
                if !self.externals.iter().any(|ext| ext.symbol_id == *id) {
                    return Err(GrammarError::UnresolvedExternalSymbol(*id));
                }
            }
            Symbol::Optional(inner) | Symbol::Repeat(inner) | Symbol::RepeatOne(inner) => {
                self.validate_symbol(inner)?;
            }
            Symbol::Choice(choices) => {
                for s in choices {
                    self.validate_symbol(s)?;
                }
            }
            Symbol::Sequence(seq) => {
                for s in seq {
                    self.validate_symbol(s)?;
                }
            }
            Symbol::Epsilon => {}
        }
        Ok(())
    }

    /// Validate grammar consistency and detect issues
    #[must_use = "validation result must be checked"]
    pub fn validate(&self) -> Result<(), GrammarError> {
        // Validate field name ordering (must be lexicographic)
        let mut field_names: Vec<_> = self.fields.values().collect();
        field_names.sort();
        let expected_order: Vec<_> = self.fields.values().collect();
        if field_names != expected_order {
            return Err(GrammarError::InvalidFieldOrdering);
        }

        // Validate symbol references
        for rule in self.all_rules() {
            for symbol in &rule.rhs {
                self.validate_symbol(symbol)?;
            }
        }

        Ok(())
    }

    /// Apply grammar transformations for better table generation
    pub fn optimize(&mut self) {
        // Remove unused rules
        // Inline simple rules where beneficial
        // Optimize precedence declarations
        // This will be implemented based on Tree-sitter's optimization strategies
    }

    /// Normalize complex symbols by creating auxiliary rules
    /// This expands Optional, Repeat, Choice, etc. into standard rules
    pub fn normalize(&mut self) -> Vec<Rule> {
        let max_id = self.rules.keys().map(|id| id.0).max().unwrap_or(0);
        let mut aux_counter = max_id + 1000; // Start auxiliary IDs well above existing ones

        // We need to keep processing until no complex symbols remain
        loop {
            let mut found_complex = false;
            let mut all_rules = Vec::new();

            // Collect all current rules
            for (_lhs, rules) in &self.rules {
                for rule in rules {
                    all_rules.push(rule.clone());
                }
            }

            // Clear existing rules
            self.rules.clear();

            // Process each rule
            for mut rule in all_rules {
                let mut new_rhs = Vec::new();

                for symbol in rule.rhs {
                    match symbol {
                        Symbol::Optional(inner) => {
                            found_complex = true;
                            // Create aux rule: aux -> inner | ε
                            let aux_id = SymbolId(aux_counter);
                            aux_counter += 1;

                            // aux -> inner (recursively normalize the inner symbol)
                            let inner_rule = Rule {
                                lhs: aux_id,
                                rhs: vec![*inner.clone()],
                                precedence: None,
                                associativity: None,
                                fields: vec![],
                                production_id: ProductionId(0),
                            };
                            self.add_rule(inner_rule);

                            // aux -> ε
                            self.add_rule(Rule {
                                lhs: aux_id,
                                rhs: vec![Symbol::Epsilon],
                                precedence: None,
                                associativity: None,
                                fields: vec![],
                                production_id: ProductionId(0),
                            });

                            new_rhs.push(Symbol::NonTerminal(aux_id));
                        }
                        Symbol::Repeat(inner) => {
                            found_complex = true;
                            // Create aux rule: aux -> aux inner | ε
                            let aux_id = SymbolId(aux_counter);
                            aux_counter += 1;

                            // aux -> aux inner (recursively normalize)
                            self.add_rule(Rule {
                                lhs: aux_id,
                                rhs: vec![Symbol::NonTerminal(aux_id), *inner.clone()],
                                precedence: None,
                                associativity: None,
                                fields: vec![],
                                production_id: ProductionId(0),
                            });

                            // aux -> ε
                            self.add_rule(Rule {
                                lhs: aux_id,
                                rhs: vec![Symbol::Epsilon],
                                precedence: None,
                                associativity: None,
                                fields: vec![],
                                production_id: ProductionId(0),
                            });

                            new_rhs.push(Symbol::NonTerminal(aux_id));
                        }
                        Symbol::RepeatOne(inner) => {
                            found_complex = true;
                            // Create aux rule: aux -> aux inner | inner
                            let aux_id = SymbolId(aux_counter);
                            aux_counter += 1;

                            // aux -> aux inner
                            self.add_rule(Rule {
                                lhs: aux_id,
                                rhs: vec![Symbol::NonTerminal(aux_id), *inner.clone()],
                                precedence: None,
                                associativity: None,
                                fields: vec![],
                                production_id: ProductionId(0),
                            });

                            // aux -> inner
                            self.add_rule(Rule {
                                lhs: aux_id,
                                rhs: vec![*inner],
                                precedence: None,
                                associativity: None,
                                fields: vec![],
                                production_id: ProductionId(0),
                            });

                            new_rhs.push(Symbol::NonTerminal(aux_id));
                        }
                        Symbol::Choice(choices) => {
                            found_complex = true;
                            // Create aux rules: aux -> choice1 | choice2 | ...
                            let aux_id = SymbolId(aux_counter);
                            aux_counter += 1;

                            for choice in choices {
                                self.add_rule(Rule {
                                    lhs: aux_id,
                                    rhs: vec![choice],
                                    precedence: None,
                                    associativity: None,
                                    fields: vec![],
                                    production_id: ProductionId(0),
                                });
                            }

                            new_rhs.push(Symbol::NonTerminal(aux_id));
                        }
                        Symbol::Sequence(seq) => {
                            found_complex = true;
                            // Flatten sequence into the current rule
                            new_rhs.extend(seq);
                        }
                        other => new_rhs.push(other),
                    }
                }

                rule.rhs = new_rhs;
                self.add_rule(rule);
            }

            // If no complex symbols were found in this iteration, we're done
            if !found_complex {
                break;
            }
        }

        // Return all rules for compatibility (though caller probably doesn't need this)
        self.rules.values().flatten().cloned().collect()
    }
}

/// Grammar processing errors
#[derive(Debug, thiserror::Error)]
pub enum GrammarError {
    /// Failed to parse grammar
    #[error("Failed to parse grammar: {0}")]
    ParseError(#[from] serde_json::Error),

    /// Invalid field ordering
    #[error("Invalid field ordering - fields must be in lexicographic order")]
    InvalidFieldOrdering,

    /// Unresolved symbol reference
    #[error("Unresolved symbol reference: {0}")]
    UnresolvedSymbol(SymbolId),

    /// Unresolved external symbol reference
    #[error("Unresolved external symbol reference: {0}")]
    UnresolvedExternalSymbol(SymbolId),

    /// Conflict in grammar
    #[error("Conflict in grammar: {0}")]
    ConflictError(String),

    /// Invalid precedence declaration
    #[error("Invalid precedence declaration: {0}")]
    InvalidPrecedence(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_grammar_creation() {
        let grammar = Grammar::new("test".to_string());
        assert_eq!(grammar.name, "test");
        assert!(grammar.rules.is_empty());
        assert!(grammar.tokens.is_empty());
        assert!(grammar.precedences.is_empty());
        assert!(grammar.conflicts.is_empty());
        assert!(grammar.externals.is_empty());
        assert!(grammar.fields.is_empty());
        assert!(grammar.supertypes.is_empty());
        assert!(grammar.inline_rules.is_empty());
        assert!(grammar.alias_sequences.is_empty());
        assert!(grammar.production_ids.is_empty());
        assert_eq!(grammar.max_alias_sequence_length, 0);
    }

    #[test]
    fn test_field_ordering_validation() {
        let mut grammar = Grammar::new("test".to_string());

        // Add fields in non-lexicographic order
        grammar.fields.insert(FieldId(1), "zebra".to_string());
        grammar.fields.insert(FieldId(0), "alpha".to_string());

        // Validation should fail
        assert!(grammar.validate().is_err());

        // Fix the ordering
        grammar.fields.clear();
        grammar.fields.insert(FieldId(0), "alpha".to_string());
        grammar.fields.insert(FieldId(1), "zebra".to_string());

        // Validation should now pass
        assert!(grammar.validate().is_ok());
    }

    #[test]
    fn test_symbol_id_display() {
        let symbol_id = SymbolId(42);
        assert_eq!(format!("{}", symbol_id), "Symbol(42)");

        let rule_id = RuleId(10);
        assert_eq!(format!("{}", rule_id), "Rule(10)");

        let state_id = StateId(5);
        assert_eq!(format!("{}", state_id), "State(5)");

        let field_id = FieldId(3);
        assert_eq!(format!("{}", field_id), "Field(3)");

        let production_id = ProductionId(7);
        assert_eq!(format!("{}", production_id), "Production(7)");
    }

    #[test]
    fn test_precedence_kinds() {
        let static_prec = PrecedenceKind::Static(5);
        let dynamic_prec = PrecedenceKind::Dynamic(10);

        match static_prec {
            PrecedenceKind::Static(level) => assert_eq!(level, 5),
            _ => panic!("Expected static precedence"),
        }

        match dynamic_prec {
            PrecedenceKind::Dynamic(level) => assert_eq!(level, 10),
            _ => panic!("Expected dynamic precedence"),
        }
    }

    #[test]
    fn test_symbol_types() {
        let terminal = Symbol::Terminal(SymbolId(1));
        let non_terminal = Symbol::NonTerminal(SymbolId(2));
        let external = Symbol::External(SymbolId(3));

        match terminal {
            Symbol::Terminal(SymbolId(1)) => {}
            _ => panic!("Expected terminal symbol"),
        }

        match non_terminal {
            Symbol::NonTerminal(SymbolId(2)) => {}
            _ => panic!("Expected non-terminal symbol"),
        }

        match external {
            Symbol::External(SymbolId(3)) => {}
            _ => panic!("Expected external symbol"),
        }

        // Test equality and hashing
        assert_eq!(terminal, Symbol::Terminal(SymbolId(1)));
        assert_ne!(terminal, non_terminal);

        let mut set = std::collections::HashSet::new();
        set.insert(terminal.clone());
        assert!(set.contains(&terminal));
        assert!(!set.contains(&non_terminal));
    }

    #[test]
    fn test_token_patterns() {
        let string_pattern = TokenPattern::String("hello".to_string());
        let regex_pattern = TokenPattern::Regex(r"\d+".to_string());

        match string_pattern {
            TokenPattern::String(s) => assert_eq!(s, "hello"),
            _ => panic!("Expected string pattern"),
        }

        match regex_pattern {
            TokenPattern::Regex(r) => assert_eq!(r, r"\d+"),
            _ => panic!("Expected regex pattern"),
        }
    }

    #[test]
    fn test_associativity() {
        let left = Associativity::Left;
        let right = Associativity::Right;
        let none = Associativity::None;

        assert_eq!(left, Associativity::Left);
        assert_eq!(right, Associativity::Right);
        assert_eq!(none, Associativity::None);

        assert_ne!(left, right);
        assert_ne!(left, none);
        assert_ne!(right, none);
    }

    #[test]
    fn test_conflict_resolution() {
        let precedence_resolution = ConflictResolution::Precedence(PrecedenceKind::Static(5));
        let associativity_resolution = ConflictResolution::Associativity(Associativity::Left);
        let glr_resolution = ConflictResolution::GLR;

        match precedence_resolution {
            ConflictResolution::Precedence(PrecedenceKind::Static(5)) => {}
            _ => panic!("Expected precedence resolution"),
        }

        match associativity_resolution {
            ConflictResolution::Associativity(Associativity::Left) => {}
            _ => panic!("Expected associativity resolution"),
        }

        match glr_resolution {
            ConflictResolution::GLR => {}
            _ => panic!("Expected GLR resolution"),
        }
    }

    #[test]
    fn test_grammar_with_rules_and_tokens() {
        let mut grammar = Grammar::new("test_grammar".to_string());

        // Add a rule: S -> NUMBER
        let rule = Rule {
            lhs: SymbolId(0),                         // S
            rhs: vec![Symbol::Terminal(SymbolId(1))], // NUMBER
            precedence: Some(PrecedenceKind::Static(1)),
            associativity: Some(Associativity::Left),
            fields: vec![(FieldId(0), 0)],
            production_id: ProductionId(0),
        };
        grammar.add_rule(rule);

        // Add a token
        let token = Token {
            name: "NUMBER".to_string(),
            pattern: TokenPattern::Regex(r"\d+".to_string()),
            fragile: false,
        };
        grammar.tokens.insert(SymbolId(1), token);

        // Add fields in correct order
        grammar.fields.insert(FieldId(0), "left".to_string());
        grammar.fields.insert(FieldId(1), "right".to_string());

        // Validation should pass
        match grammar.validate() {
            Ok(_) => {}
            Err(e) => panic!("Grammar validation failed: {:?}", e),
        }

        assert_eq!(grammar.rules.len(), 1);
        assert_eq!(grammar.tokens.len(), 1);
        assert_eq!(grammar.fields.len(), 2);
    }

    #[test]
    fn test_grammar_validation_unresolved_symbol() {
        let mut grammar = Grammar::new("test".to_string());

        // Add a rule that references a non-existent symbol
        let rule = Rule {
            lhs: SymbolId(0),
            rhs: vec![Symbol::Terminal(SymbolId(999))], // Non-existent symbol
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(0),
        };
        grammar.add_rule(rule);

        // Validation should fail
        assert!(grammar.validate().is_err());

        match grammar.validate() {
            Err(GrammarError::UnresolvedSymbol(SymbolId(999))) => {}
            _ => panic!("Expected unresolved symbol error"),
        }
    }

    #[test]
    fn test_grammar_validation_unresolved_external() {
        let mut grammar = Grammar::new("test".to_string());

        // Add a rule that references a non-existent external symbol
        let rule = Rule {
            lhs: SymbolId(0),
            rhs: vec![Symbol::External(SymbolId(999))], // Non-existent external
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(0),
        };
        grammar.add_rule(rule);

        // Validation should fail
        assert!(grammar.validate().is_err());

        match grammar.validate() {
            Err(GrammarError::UnresolvedExternalSymbol(SymbolId(999))) => {}
            _ => panic!("Expected unresolved external symbol error"),
        }
    }

    #[test]
    fn test_alias_sequence() {
        let alias_seq = AliasSequence {
            aliases: vec![Some("alias1".to_string()), None, Some("alias2".to_string())],
        };

        assert_eq!(alias_seq.aliases.len(), 3);
        assert_eq!(alias_seq.aliases[0], Some("alias1".to_string()));
        assert_eq!(alias_seq.aliases[1], None);
        assert_eq!(alias_seq.aliases[2], Some("alias2".to_string()));
    }

    #[test]
    fn test_external_token() {
        let external_token = ExternalToken {
            name: "HERE_STRING".to_string(),
            symbol_id: SymbolId(42),
        };

        assert_eq!(external_token.name, "HERE_STRING");
        assert_eq!(external_token.symbol_id, SymbolId(42));
    }

    #[test]
    fn test_precedence() {
        let precedence = Precedence {
            level: 10,
            associativity: Associativity::Right,
            symbols: vec![SymbolId(1), SymbolId(2), SymbolId(3)],
        };

        assert_eq!(precedence.level, 10);
        assert_eq!(precedence.associativity, Associativity::Right);
        assert_eq!(precedence.symbols.len(), 3);
        assert!(precedence.symbols.contains(&SymbolId(2)));
    }

    #[test]
    fn test_conflict_declaration() {
        let conflict = ConflictDeclaration {
            symbols: vec![SymbolId(1), SymbolId(2)],
            resolution: ConflictResolution::GLR,
        };

        assert_eq!(conflict.symbols.len(), 2);
        assert!(conflict.symbols.contains(&SymbolId(1)));
        assert!(conflict.symbols.contains(&SymbolId(2)));

        match conflict.resolution {
            ConflictResolution::GLR => {}
            _ => panic!("Expected GLR resolution"),
        }
    }
}
