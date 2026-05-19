// Grammar validation and diagnostics for Adze
//! Grammar validation checking for undefined, unreachable, and non-productive symbols.

// This module provides comprehensive validation and diagnostic capabilities

mod symbol_walk;

use crate::{FieldId, Grammar, Symbol, SymbolId};
use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt;
use symbol_walk::{add_reachable_from_symbol, collect_used_in_symbol, is_symbol_productive};

/// Grammar validation errors
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationError {
    /// Undefined symbol referenced
    UndefinedSymbol {
        /// The undefined symbol
        symbol: SymbolId,
        /// Where it was referenced
        location: String,
    },
    /// Unreachable symbol (not reachable from start)
    UnreachableSymbol {
        /// The unreachable symbol
        symbol: SymbolId,
        /// Symbol name
        name: String,
    },
    /// Non-productive symbol (can't derive terminal strings)
    NonProductiveSymbol {
        /// The non-productive symbol
        symbol: SymbolId,
        /// Symbol name
        name: String,
    },
    /// Cyclic rule without base case
    CyclicRule {
        /// Symbols involved in the cycle
        symbols: Vec<SymbolId>,
    },
    /// Duplicate rule definition
    DuplicateRule {
        /// Symbol with duplicate rules
        symbol: SymbolId,
        /// Number of existing definitions
        existing_count: usize,
    },
    /// Invalid field mapping
    InvalidField {
        /// Invalid field ID
        field_id: FieldId,
        /// Symbol containing the invalid field
        rule_symbol: SymbolId,
    },
    /// Empty grammar
    EmptyGrammar,
    /// Grammar has no explicit start rule
    NoExplicitStartRule,
    /// Conflicting precedence declarations
    ConflictingPrecedence {
        /// Symbol with conflicting precedences
        symbol: SymbolId,
        /// Conflicting precedence values
        precedences: Vec<i16>,
    },
    /// Invalid regex pattern
    InvalidRegex {
        /// Token with invalid regex
        token: SymbolId,
        /// The invalid pattern
        pattern: String,
        /// Error message
        error: String,
    },
    /// External token conflict
    ExternalTokenConflict {
        /// First conflicting token
        token1: String,
        /// Second conflicting token
        token2: String,
    },
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ValidationError::UndefinedSymbol { symbol, location } => {
                write!(
                    f,
                    "Undefined symbol {:?} referenced in {}",
                    symbol, location
                )
            }
            ValidationError::UnreachableSymbol { symbol, name } => {
                format_symbol_property_error(f, name, symbol, "is unreachable from start symbol")
            }
            ValidationError::NonProductiveSymbol { symbol, name } => {
                format_symbol_property_error(f, name, symbol, "cannot derive any terminal strings")
            }
            ValidationError::CyclicRule { symbols } => {
                write!(f, "Cyclic dependency detected: {:?}", symbols)
            }
            ValidationError::DuplicateRule {
                symbol,
                existing_count,
            } => {
                write!(
                    f,
                    "Symbol {:?} has {} rule definitions (expected 1)",
                    symbol, existing_count
                )
            }
            ValidationError::InvalidField {
                field_id,
                rule_symbol,
            } => {
                write!(
                    f,
                    "Invalid field {:?} in rule for symbol {:?}",
                    field_id, rule_symbol
                )
            }
            ValidationError::EmptyGrammar => {
                write!(f, "Grammar has no rules defined")
            }
            ValidationError::NoExplicitStartRule => {
                write!(
                    f,
                    "No explicit start rule defined (first rule will be used)"
                )
            }
            ValidationError::ConflictingPrecedence {
                symbol,
                precedences,
            } => {
                write!(
                    f,
                    "Symbol {:?} has conflicting precedences: {:?}",
                    symbol, precedences
                )
            }
            ValidationError::InvalidRegex {
                token,
                pattern,
                error,
            } => {
                write!(
                    f,
                    "Invalid regex pattern for token {:?}: '{}' - {}",
                    token, pattern, error
                )
            }
            ValidationError::ExternalTokenConflict { token1, token2 } => {
                write!(f, "External tokens '{}' and '{}' conflict", token1, token2)
            }
        }
    }
}

fn format_symbol_property_error(
    f: &mut fmt::Formatter<'_>,
    name: &str,
    symbol: &SymbolId,
    detail: &str,
) -> fmt::Result {
    write!(f, "Symbol '{}' ({:?}) {}", name, symbol, detail)
}

/// Grammar validator
pub struct GrammarValidator {
    errors: Vec<ValidationError>,
    warnings: Vec<ValidationWarning>,
}

/// Grammar validation warnings (non-fatal issues)
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationWarning {
    /// Unused token
    UnusedToken {
        /// The unused token
        token: SymbolId,
        /// Token name
        name: String,
    },
    /// Duplicate token pattern
    DuplicateTokenPattern {
        /// Tokens with duplicate pattern
        tokens: Vec<SymbolId>,
        /// The duplicate pattern
        pattern: String,
    },
    /// Ambiguous grammar (may need GLR)
    AmbiguousGrammar {
        /// Ambiguity description
        message: String,
    },
    /// Missing field names
    MissingFieldNames {
        /// Symbol missing field names
        rule_symbol: SymbolId,
    },
    /// Inefficient rule structure
    InefficientRule {
        /// Symbol with inefficient rule
        symbol: SymbolId,
        /// Optimization suggestion
        suggestion: String,
    },
}

impl fmt::Display for ValidationWarning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ValidationWarning::UnusedToken { token, name } => {
                write!(
                    f,
                    "Token '{}' ({:?}) is defined but never used",
                    name, token
                )
            }
            ValidationWarning::DuplicateTokenPattern { tokens, pattern } => {
                write!(
                    f,
                    "Multiple tokens have the same pattern '{}': {:?}",
                    pattern, tokens
                )
            }
            ValidationWarning::AmbiguousGrammar { message } => {
                write!(f, "Grammar ambiguity detected: {}", message)
            }
            ValidationWarning::MissingFieldNames { rule_symbol } => {
                write!(f, "Rule for symbol {:?} has no field names", rule_symbol)
            }
            ValidationWarning::InefficientRule { symbol, suggestion } => {
                write!(
                    f,
                    "Inefficient rule for symbol {:?}: {}",
                    symbol, suggestion
                )
            }
        }
    }
}

/// Validation result
pub struct ValidationResult {
    /// Validation errors found
    pub errors: Vec<ValidationError>,
    /// Validation warnings found
    pub warnings: Vec<ValidationWarning>,
    /// Validation statistics
    pub stats: ValidationStats,
}

/// Statistics gathered during validation
#[derive(Debug, Clone, Default)]
pub struct ValidationStats {
    /// Total number of symbols
    pub total_symbols: usize,
    /// Total number of tokens
    pub total_tokens: usize,
    /// Total number of rules
    pub total_rules: usize,
    /// Number of reachable symbols
    pub reachable_symbols: usize,
    /// Number of productive symbols
    pub productive_symbols: usize,
    /// Number of external tokens
    pub external_tokens: usize,
    /// Maximum rule length
    pub max_rule_length: usize,
    /// Average rule length
    pub avg_rule_length: f64,
}

impl Default for GrammarValidator {
    fn default() -> Self {
        Self::new()
    }
}

impl GrammarValidator {
    /// Create a new validator
    pub fn new() -> Self {
        Self {
            errors: Vec::new(),
            warnings: Vec::new(),
        }
    }

    /// Validate a grammar and return results
    pub fn validate(&mut self, grammar: &Grammar) -> ValidationResult {
        self.errors.clear();
        self.warnings.clear();

        let mut stats = ValidationStats::default();

        // Basic checks
        self.check_empty_grammar(grammar);

        // Symbol analysis
        let defined_symbols = self.collect_defined_symbols(grammar);
        let used_symbols = self.collect_used_symbols(grammar);

        // Check for undefined symbols
        self.check_undefined_symbols(&defined_symbols, &used_symbols, grammar);

        // Reachability analysis
        let reachable = self.find_reachable_symbols(grammar);
        self.check_unreachable_symbols(&reachable, &defined_symbols, grammar);

        // Productivity analysis
        let productive = self.find_productive_symbols(grammar);
        self.check_non_productive_symbols(&productive, &defined_symbols, grammar);

        // Token validation
        self.validate_tokens(grammar);

        // Field validation
        self.validate_fields(grammar);

        // Precedence validation
        self.validate_precedences(grammar);

        // External token validation
        self.validate_external_tokens(grammar);

        // Check for cycles
        self.check_cycles(grammar);

        // Check for inefficiencies
        self.check_inefficiencies(grammar);

        // Gather statistics
        stats.total_symbols = defined_symbols.len();
        stats.total_tokens = grammar.tokens.len();
        stats.total_rules = grammar.rules.values().map(|v| v.len()).sum();
        stats.reachable_symbols = reachable.len();
        stats.productive_symbols = productive.len();
        stats.external_tokens = grammar.externals.len();

        if !grammar.rules.is_empty() {
            let all_rules: Vec<_> = grammar.all_rules().collect();
            let total_length: usize = all_rules.iter().map(|r| r.rhs.len()).sum();
            stats.max_rule_length = all_rules.iter().map(|r| r.rhs.len()).max().unwrap_or(0);
            stats.avg_rule_length = total_length as f64 / all_rules.len() as f64;
        }

        ValidationResult {
            errors: self.errors.clone(),
            warnings: self.warnings.clone(),
            stats,
        }
    }

    fn check_empty_grammar(&mut self, grammar: &Grammar) {
        if grammar.rules.is_empty() {
            self.errors.push(ValidationError::EmptyGrammar);
        }
    }

    fn collect_defined_symbols(&self, grammar: &Grammar) -> HashSet<SymbolId> {
        let mut defined = HashSet::new();

        // All tokens are defined
        defined.extend(grammar.tokens.keys());

        // All rule LHS are defined
        defined.extend(grammar.rules.keys());

        // External tokens are defined
        for external in &grammar.externals {
            defined.insert(external.symbol_id);
        }

        defined
    }

    fn collect_used_symbols(&self, grammar: &Grammar) -> HashSet<SymbolId> {
        let mut used = HashSet::new();

        // First rule's LHS is implicitly the start symbol
        if let Some(start_symbol) = grammar.start_symbol() {
            used.insert(start_symbol);
        }

        // Symbols in rule RHS are used
        for rule in grammar.all_rules() {
            for symbol in &rule.rhs {
                collect_used_in_symbol(symbol, &mut used);
            }
        }

        used
    }

    fn check_undefined_symbols(
        &mut self,
        defined: &HashSet<SymbolId>,
        used: &HashSet<SymbolId>,
        grammar: &Grammar,
    ) {
        for symbol in used {
            if !defined.contains(symbol) {
                // Find where it's used
                let mut location = String::from("unknown");
                for (rule_sym, rules) in &grammar.rules {
                    for rule in rules {
                        for rhs_sym in &rule.rhs {
                            match rhs_sym {
                                Symbol::Terminal(id) | Symbol::NonTerminal(id) if id == symbol => {
                                    location = format!("rule for {:?}", rule_sym);
                                    break;
                                }
                                _ => {}
                            }
                        }
                    }
                }

                self.errors.push(ValidationError::UndefinedSymbol {
                    symbol: *symbol,
                    location,
                });
            }
        }
    }

    fn find_reachable_symbols(&self, grammar: &Grammar) -> HashSet<SymbolId> {
        let mut reachable = HashSet::new();
        let mut queue = VecDeque::new();

        // Start from the first rule (implicit start symbol)
        if let Some(start) = grammar.start_symbol() {
            queue.push_back(start);
            reachable.insert(start);
        }

        // BFS to find all reachable symbols
        while let Some(symbol) = queue.pop_front() {
            if let Some(rules) = grammar.rules.get(&symbol) {
                for rule in rules {
                    for rhs_symbol in &rule.rhs {
                        add_reachable_from_symbol(rhs_symbol, &mut reachable, &mut queue);
                    }
                }
            }
        }

        reachable
    }

    fn check_unreachable_symbols(
        &mut self,
        reachable: &HashSet<SymbolId>,
        defined: &HashSet<SymbolId>,
        grammar: &Grammar,
    ) {
        let start_symbol = grammar.start_symbol();

        for symbol in defined {
            if !reachable.contains(symbol) && Some(*symbol) != start_symbol {
                let name = self.get_symbol_name(*symbol, grammar);
                self.warnings.push(ValidationWarning::UnusedToken {
                    token: *symbol,
                    name,
                });
            }
        }
    }

    fn find_productive_symbols(&self, grammar: &Grammar) -> HashSet<SymbolId> {
        let mut productive = HashSet::new();
        let mut changed = true;

        // All tokens are productive
        productive.extend(grammar.tokens.keys());

        // External tokens are productive
        for external in &grammar.externals {
            productive.insert(external.symbol_id);
        }

        // Fixed-point iteration to find productive non-terminals
        while changed {
            changed = false;

            for (symbol, rules) in &grammar.rules {
                if !productive.contains(symbol) {
                    // Check if any rule for this symbol is productive
                    let any_productive = rules.iter().any(|rule| {
                        // Check if all RHS symbols are productive
                        rule.rhs
                            .iter()
                            .all(|rhs_sym| is_symbol_productive(rhs_sym, &productive))
                    });

                    if any_productive {
                        productive.insert(*symbol);
                        changed = true;
                    }
                }
            }
        }

        productive
    }

    fn check_non_productive_symbols(
        &mut self,
        productive: &HashSet<SymbolId>,
        defined: &HashSet<SymbolId>,
        grammar: &Grammar,
    ) {
        for symbol in defined {
            if !productive.contains(symbol) {
                let name = self.get_symbol_name(*symbol, grammar);
                self.errors.push(ValidationError::NonProductiveSymbol {
                    symbol: *symbol,
                    name,
                });
            }
        }
    }

    fn validate_tokens(&mut self, grammar: &Grammar) {
        let mut pattern_map: HashMap<String, Vec<SymbolId>> = HashMap::new();

        for (symbol, token) in &grammar.tokens {
            // Check regex validity
            if let crate::TokenPattern::Regex(pattern) = &token.pattern {
                // In a real implementation, we'd compile the regex
                // For now, just check for basic issues
                if pattern.is_empty() {
                    self.errors.push(ValidationError::InvalidRegex {
                        token: *symbol,
                        pattern: pattern.clone(),
                        error: "Empty regex pattern".to_string(),
                    });
                }
            }

            // Check for duplicate patterns
            let pattern_str = match &token.pattern {
                crate::TokenPattern::String(s) => s.clone(),
                crate::TokenPattern::Regex(r) => r.clone(),
            };

            pattern_map
                .entry(pattern_str.clone())
                .or_default()
                .push(*symbol);
        }

        // Report duplicate patterns
        for (pattern, symbols) in pattern_map {
            if symbols.len() > 1 {
                self.warnings
                    .push(ValidationWarning::DuplicateTokenPattern {
                        tokens: symbols,
                        pattern,
                    });
            }
        }
    }

    fn validate_fields(&mut self, grammar: &Grammar) {
        for (symbol, rules) in &grammar.rules {
            for rule in rules {
                // Check that field indices are valid
                for (field_id, index) in &rule.fields {
                    if *index >= rule.rhs.len() {
                        self.errors.push(ValidationError::InvalidField {
                            field_id: *field_id,
                            rule_symbol: *symbol,
                        });
                    }
                }

                // Warn about missing field names
                if rule.fields.is_empty() && rule.rhs.len() > 1 {
                    self.warnings.push(ValidationWarning::MissingFieldNames {
                        rule_symbol: *symbol,
                    });
                }
            }
        }
    }

    fn validate_precedences(&mut self, grammar: &Grammar) {
        let mut symbol_precedences: HashMap<SymbolId, Vec<i16>> = HashMap::new();

        // Collect precedences from declarations
        for prec in &grammar.precedences {
            for symbol in &prec.symbols {
                symbol_precedences
                    .entry(*symbol)
                    .or_default()
                    .push(prec.level);
            }
        }

        // Check for conflicts
        for (symbol, precedences) in symbol_precedences {
            let unique_precs: HashSet<_> = precedences.iter().cloned().collect();
            if unique_precs.len() > 1 {
                self.errors.push(ValidationError::ConflictingPrecedence {
                    symbol,
                    precedences: unique_precs.into_iter().collect(),
                });
            }
        }
    }

    fn validate_external_tokens(&mut self, grammar: &Grammar) {
        let mut names = HashSet::new();

        for external in &grammar.externals {
            if !names.insert(&external.name) {
                self.errors.push(ValidationError::ExternalTokenConflict {
                    token1: external.name.clone(),
                    token2: external.name.clone(),
                });
            }
        }
    }

    fn check_cycles(&mut self, grammar: &Grammar) {
        // Simple cycle detection using DFS
        let mut visited = HashSet::new();
        let mut rec_stack = HashSet::new();
        let mut path = Vec::new();

        for symbol in grammar.rules.keys() {
            if !visited.contains(symbol)
                && self.has_cycle(*symbol, grammar, &mut visited, &mut rec_stack, &mut path)
            {
                self.errors.push(ValidationError::CyclicRule {
                    symbols: path.clone(),
                });
            }
        }
    }

    fn has_cycle(
        &self,
        symbol: SymbolId,
        grammar: &Grammar,
        visited: &mut HashSet<SymbolId>,
        rec_stack: &mut HashSet<SymbolId>,
        path: &mut Vec<SymbolId>,
    ) -> bool {
        visited.insert(symbol);
        rec_stack.insert(symbol);
        path.push(symbol);

        if let Some(rules) = grammar.rules.get(&symbol) {
            for rule in rules {
                for rhs_symbol in &rule.rhs {
                    if let Symbol::NonTerminal(id) = rhs_symbol {
                        if !visited.contains(id) {
                            if self.has_cycle(*id, grammar, visited, rec_stack, path) {
                                return true;
                            }
                        } else if rec_stack.contains(id) {
                            // Found a cycle
                            return true;
                        }
                    }
                }
            }
        }

        path.pop();
        rec_stack.remove(&symbol);
        false
    }

    fn check_inefficiencies(&mut self, grammar: &Grammar) {
        for (symbol, rules) in &grammar.rules {
            for rule in rules {
                // Check for trivial rules (A -> B)
                if rule.rhs.len() == 1
                    && let Symbol::NonTerminal(_) = &rule.rhs[0]
                {
                    self.warnings.push(ValidationWarning::InefficientRule {
                        symbol: *symbol,
                        suggestion: "Consider inlining trivial rules".to_string(),
                    });
                }

                // Check for very long rules
                if rule.rhs.len() > 10 {
                    self.warnings.push(ValidationWarning::InefficientRule {
                        symbol: *symbol,
                        suggestion: format!(
                            "Rule has {} symbols, consider breaking it down",
                            rule.rhs.len()
                        ),
                    });
                }
            }
        }
    }

    fn get_symbol_name(&self, symbol: SymbolId, grammar: &Grammar) -> String {
        if let Some(token) = grammar.tokens.get(&symbol) {
            return token.name.clone();
        }

        if let Some(_rule) = grammar.rules.get(&symbol) {
            return format!("rule_{}", symbol.0);
        }

        for external in &grammar.externals {
            if external.symbol_id == symbol {
                return external.name.clone();
            }
        }

        format!("symbol_{}", symbol.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        FieldId, Grammar, Precedence, ProductionId, Rule, Symbol, SymbolId, Token, TokenPattern,
    };

    #[test]
    fn test_empty_grammar_validation() {
        let grammar = Grammar::new("test".to_string());
        let mut validator = GrammarValidator::new();
        let result = validator.validate(&grammar);

        assert!(
            result
                .errors
                .iter()
                .any(|e| matches!(e, ValidationError::EmptyGrammar))
        );
    }

    #[test]
    fn test_undefined_symbol() {
        let mut grammar = Grammar::new("test".to_string());
        let expr = SymbolId(1);
        let undefined = SymbolId(99);

        grammar.add_rule(Rule {
            lhs: expr,
            rhs: vec![Symbol::NonTerminal(undefined)],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(0),
        });

        let mut validator = GrammarValidator::new();
        let result = validator.validate(&grammar);

        assert!(result.errors.iter().any(|e| {
            matches!(e, ValidationError::UndefinedSymbol { symbol, .. } if *symbol == undefined)
        }));
    }

    #[test]
    fn test_non_productive_symbol() {
        let mut grammar = Grammar::new("test".to_string());
        let a = SymbolId(1);
        let b = SymbolId(2);

        // A -> B
        grammar.add_rule(Rule {
            lhs: a,
            rhs: vec![Symbol::NonTerminal(b)],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(0),
        });

        // B -> A (circular, non-productive)
        grammar.add_rule(Rule {
            lhs: b,
            rhs: vec![Symbol::NonTerminal(a)],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(1),
        });

        let mut validator = GrammarValidator::new();
        let result = validator.validate(&grammar);

        assert!(
            result
                .errors
                .iter()
                .any(|e| { matches!(e, ValidationError::NonProductiveSymbol { .. }) })
        );
    }

    #[test]
    fn test_valid_grammar() {
        let mut grammar = Grammar::new("test".to_string());
        let expr = SymbolId(1);
        let num = SymbolId(2);

        // Add token
        grammar.tokens.insert(
            num,
            Token {
                name: "number".to_string(),
                pattern: TokenPattern::Regex(r"\d+".to_string()),
                fragile: false,
            },
        );

        // expr -> num
        grammar.add_rule(Rule {
            lhs: expr,
            rhs: vec![Symbol::Terminal(num)],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(0),
        });

        // No explicit start symbol field, first rule is implicit start

        let mut validator = GrammarValidator::new();
        let result = validator.validate(&grammar);

        assert!(result.errors.is_empty());
        assert_eq!(result.stats.total_symbols, 2);
        assert_eq!(result.stats.productive_symbols, 2);
    }

    #[test]
    fn test_duplicate_token_patterns() {
        let mut grammar = Grammar::new("test".to_string());

        // Add two tokens with the same pattern
        grammar.tokens.insert(
            SymbolId(1),
            Token {
                name: "plus1".to_string(),
                pattern: TokenPattern::String("+".to_string()),
                fragile: false,
            },
        );

        grammar.tokens.insert(
            SymbolId(2),
            Token {
                name: "plus2".to_string(),
                pattern: TokenPattern::String("+".to_string()),
                fragile: false,
            },
        );

        let mut validator = GrammarValidator::new();
        let result = validator.validate(&grammar);

        assert!(result.warnings.iter().any(|w| {
            matches!(w, ValidationWarning::DuplicateTokenPattern { pattern, .. } if pattern == "+")
        }));
    }

    #[test]
    fn test_invalid_field_index() {
        let mut grammar = Grammar::new("test".to_string());
        let expr = SymbolId(1);
        let num = SymbolId(2);

        grammar.tokens.insert(
            num,
            Token {
                name: "number".to_string(),
                pattern: TokenPattern::Regex(r"\d+".to_string()),
                fragile: false,
            },
        );

        // Rule with invalid field index
        grammar.add_rule(Rule {
            lhs: expr,
            rhs: vec![Symbol::Terminal(num)],
            precedence: None,
            associativity: None,
            fields: vec![(FieldId(0), 5)], // Index 5 is out of bounds
            production_id: ProductionId(0),
        });

        let mut validator = GrammarValidator::new();
        let result = validator.validate(&grammar);

        assert!(
            result
                .errors
                .iter()
                .any(|e| { matches!(e, ValidationError::InvalidField { .. }) })
        );
    }

    #[test]
    fn test_cyclic_rules() {
        let mut grammar = Grammar::new("test".to_string());
        let a = SymbolId(1);
        let b = SymbolId(2);
        let c = SymbolId(3);

        // Create a cycle: A -> B -> C -> A
        grammar.add_rule(Rule {
            lhs: a,
            rhs: vec![Symbol::NonTerminal(b)],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(0),
        });

        grammar.add_rule(Rule {
            lhs: b,
            rhs: vec![Symbol::NonTerminal(c)],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(1),
        });

        grammar.add_rule(Rule {
            lhs: c,
            rhs: vec![Symbol::NonTerminal(a)],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(2),
        });

        let mut validator = GrammarValidator::new();
        let result = validator.validate(&grammar);

        assert!(
            result
                .errors
                .iter()
                .any(|e| { matches!(e, ValidationError::CyclicRule { .. }) })
        );
    }

    #[test]
    fn test_conflicting_precedence() {
        let mut grammar = Grammar::new("test".to_string());
        let plus = SymbolId(1);

        // Add conflicting precedence declarations
        grammar.precedences.push(Precedence {
            level: 1,
            associativity: crate::Associativity::Left,
            symbols: vec![plus],
        });

        grammar.precedences.push(Precedence {
            level: 2,
            associativity: crate::Associativity::Right,
            symbols: vec![plus],
        });

        let mut validator = GrammarValidator::new();
        let result = validator.validate(&grammar);

        assert!(result.errors.iter().any(|e| {
            matches!(e, ValidationError::ConflictingPrecedence { symbol, .. } if *symbol == plus)
        }));
    }
}
