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
/// Lexical metadata and pattern validation for the generated-lexer contract.
pub mod lexical;
/// Lexical metadata types and validation helpers.
pub use lexical::{
    LexicalMetadata, LexicalPatternError, TOKEN_WRAPPER_PRIORITY, compare_lexical_priority,
    sorted_lexical_metadata, validate_token_pattern,
};
/// Compiler-significant start-symbol and wrapper-token identity metadata.
pub mod compiler_identity;
/// Compiler identity metadata types and helpers.
pub use compiler_identity::{
    WrapperTokenRelation, remap_compiler_identity, sorted_wrapper_token_relations,
};

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
    /// Total many-to-one map from rule IDs to production IDs (#980).
    ///
    /// A [`RuleId`] is the dense flattened index of [`all_rules`](Grammar::all_rules)
    /// in insertion order; a [`ProductionId`] is a grammar-local production
    /// slot that distinct rules may legitimately share. The map is a
    /// derived cache: rebuild it with
    /// [`refresh_production_ids`](Grammar::refresh_production_ids) after
    /// mutating rules outside the builder/normalize/optimize boundaries.
    pub production_ids: IndexMap<RuleId, ProductionId>,
    /// Maximum alias sequence length
    pub max_alias_sequence_length: usize,
    /// Maps symbol IDs to rule names
    pub rule_names: IndexMap<SymbolId, String>,
    /// Centralized symbol registry
    pub symbol_registry: Option<SymbolRegistry>,
    /// Authoritative word-token symbol for keyword-versus-identifier resolution.
    #[serde(default)]
    pub word_token: Option<SymbolId>,
    /// Per-token lexical metadata (immediate flag, lexical priority).
    #[serde(default)]
    pub lexical_metadata: IndexMap<SymbolId, LexicalMetadata>,
    /// Authoritative start symbol declared at grammar-definition or import time.
    #[serde(default)]
    pub start_symbol: Option<SymbolId>,
    /// Explicit pattern-wrapper nonterminal to backing-token relations.
    #[serde(default)]
    pub wrapper_token_relations: IndexMap<SymbolId, SymbolId>,
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

    /// Rebuild [`production_ids`](Self::production_ids) from the current
    /// rules so the `RuleId -> ProductionId` map is total (#980).
    ///
    /// `RuleId(index)` denotes the `index`-th rule of
    /// [`all_rules`](Self::all_rules). The mapping is many-to-one:
    /// production IDs are grammar-local slots and are never renumbered.
    pub fn refresh_production_ids(&mut self) {
        let entries: Vec<(RuleId, ProductionId)> = self
            .all_rules()
            .enumerate()
            .map(|(index, rule)| {
                let rule_id =
                    RuleId(u16::try_from(index).expect("rule count exceeds the RuleId range"));
                (rule_id, rule.production_id)
            })
            .collect();
        self.production_ids.clear();
        self.production_ids.extend(entries);
    }

    /// First production slot shared by rules whose field/alias metadata
    /// disagree, if any (#980 PR3). Sharing a slot is legitimate only when
    /// every sharing rule carries identical `fields` and, when an alias
    /// sequence is recorded for the slot, every sharing rule has the same
    /// RHS length as that sequence; otherwise a consumer that inverts the
    /// slot to one rule's metadata would pick arbitrarily.
    pub(crate) fn conflicting_production_share(&self) -> Option<(ProductionId, Vec<RuleId>)> {
        let mut groups: IndexMap<ProductionId, Vec<(RuleId, &Rule)>> = IndexMap::new();
        for (index, rule) in self.all_rules().enumerate() {
            let rule_id =
                RuleId(u16::try_from(index).expect("rule count exceeds the RuleId range"));
            groups
                .entry(rule.production_id)
                .or_default()
                .push((rule_id, rule));
        }
        for (production_id, sharing) in groups {
            if sharing.len() < 2 {
                continue;
            }
            let first = sharing[0].1;
            let fields_conflict = sharing.iter().any(|(_, rule)| rule.fields != first.fields);
            let alias_conflict = self.alias_sequences.get(&production_id).is_some_and(|seq| {
                sharing
                    .iter()
                    .any(|(_, rule)| rule.rhs.len() != seq.aliases.len())
            });
            if fields_conflict || alias_conflict {
                return Some((
                    production_id,
                    sharing.into_iter().map(|(rule_id, _)| rule_id).collect(),
                ));
            }
        }
        None
    }

    /// Production slot for one rule, when `rule_id` is in the current
    /// rule domain. Refresh with
    /// [`refresh_production_ids`](Self::refresh_production_ids) after
    /// mutating rules directly.
    pub fn production_id_for_rule(&self, rule_id: RuleId) -> Option<ProductionId> {
        self.production_ids.get(&rule_id).copied()
    }

    /// Every rule that shares one production slot, in rule order. This is
    /// the legitimate reverse direction of the many-to-one map; a
    /// production must never be inverted to a single rule.
    pub fn rule_ids_for_production(
        &self,
        production_id: ProductionId,
    ) -> impl Iterator<Item = RuleId> + '_ {
        self.production_ids
            .iter()
            .filter(move |(_, candidate)| **candidate == production_id)
            .map(|(rule_id, _)| *rule_id)
    }

    /// Get the start symbol for this grammar.
    ///
    /// Returns the explicit [`start_symbol`](Self::start_symbol) field when set and
    /// the symbol still has rules. Legacy name/order heuristics were removed in
    /// #862 PR6; callers must set `start_symbol` explicitly.
    pub fn start_symbol(&self) -> Option<SymbolId> {
        if let Some(explicit) = self.start_symbol
            && self.rules.contains_key(&explicit)
        {
            return Some(explicit);
        }

        None
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
            word_token: None,
            lexical_metadata: IndexMap::new(),
            start_symbol: None,
            wrapper_token_relations: IndexMap::new(),
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

        // Reject production shares whose field/alias metadata conflict (#980 PR3)
        if let Some((production_id, rule_ids)) = self.conflicting_production_share() {
            return Err(GrammarError::ConflictingProductionShare {
                production_id,
                rule_ids,
            });
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
        self.refresh_production_ids();
    }

    /// Every symbol identity this grammar already carries, across all
    /// categories that share the `SymbolId(u16)` space (#978).
    fn occupied_symbol_ids(&self) -> Vec<bool> {
        fn mark(occupied: &mut [bool], id: SymbolId) {
            occupied[id.0 as usize] = true;
        }
        fn mark_symbol(occupied: &mut [bool], symbol: &Symbol) {
            match symbol {
                Symbol::Terminal(id) | Symbol::NonTerminal(id) | Symbol::External(id) => {
                    mark(occupied, *id);
                }
                Symbol::Optional(inner) | Symbol::Repeat(inner) | Symbol::RepeatOne(inner) => {
                    mark_symbol(occupied, inner)
                }
                Symbol::Choice(choices) | Symbol::Sequence(choices) => {
                    for choice in choices {
                        mark_symbol(occupied, choice);
                    }
                }
                Symbol::Epsilon => {}
            }
        }

        let mut occupied = vec![false; usize::from(u16::MAX) + 1];
        // SymbolId(0) is reserved for EOF and is never handed out.
        occupied[0] = true;
        for (lhs, rules) in &self.rules {
            mark(&mut occupied, *lhs);
            for rule in rules {
                for symbol in &rule.rhs {
                    mark_symbol(&mut occupied, symbol);
                }
            }
        }
        for id in self.tokens.keys() {
            mark(&mut occupied, *id);
        }
        for external in &self.externals {
            mark(&mut occupied, external.symbol_id);
        }
        for id in self.rule_names.keys() {
            mark(&mut occupied, *id);
        }
        for id in self
            .extras
            .iter()
            .chain(&self.supertypes)
            .chain(&self.inline_rules)
        {
            mark(&mut occupied, *id);
        }
        if let Some(start) = self.start_symbol {
            mark(&mut occupied, start);
        }
        if let Some(word) = self.word_token {
            mark(&mut occupied, word);
        }
        for id in self.lexical_metadata.keys() {
            mark(&mut occupied, *id);
        }
        for (wrapper, backing) in &self.wrapper_token_relations {
            mark(&mut occupied, *wrapper);
            mark(&mut occupied, *backing);
        }
        for precedence in &self.precedences {
            for id in &precedence.symbols {
                mark(&mut occupied, *id);
            }
        }
        for conflict in &self.conflicts {
            for id in &conflict.symbols {
                mark(&mut occupied, *id);
            }
        }
        if let Some(registry) = &self.symbol_registry {
            for (_, info) in registry.iter() {
                mark(&mut occupied, info.id);
            }
        }
        occupied
    }

    /// Upper bound on helpers normalization will allocate: one per
    /// `Optional`/`Repeat`/`RepeatOne`/`Choice` node in the current rules.
    fn count_complex_symbols(symbols: &[Symbol]) -> usize {
        symbols
            .iter()
            .map(|symbol| match symbol {
                Symbol::Optional(inner) | Symbol::Repeat(inner) | Symbol::RepeatOne(inner) => {
                    1 + Self::count_complex_symbols(std::slice::from_ref(&**inner))
                }
                Symbol::Choice(choices) | Symbol::Sequence(choices) => {
                    Self::count_complex_symbols(choices)
                        + usize::from(matches!(symbol, Symbol::Choice(_)))
                }
                Symbol::Terminal(_)
                | Symbol::NonTerminal(_)
                | Symbol::External(_)
                | Symbol::Epsilon => 0,
            })
            .sum()
    }

    /// Normalize complex symbols by creating auxiliary rules, returning a
    /// typed error instead of panicking, wrapping, or colliding when the
    /// symbol ID space cannot supply a free helper (#978).
    ///
    /// Helpers take unoccupied IDs from the band above the maximum
    /// occupied ID, falling back to the lowest unoccupied IDs when that
    /// band would leave the ID space; the reserved EOF id `SymbolId(0)`
    /// is never handed out, so identical input normalizes identically.
    ///
    /// # Errors
    ///
    /// Returns [`GrammarError::SymbolSpaceExhausted`] before mutating the
    /// grammar when fewer free IDs remain than helpers required.
    pub fn try_normalize(&mut self) -> Result<Vec<Rule>, GrammarError> {
        let mut occupied = self.occupied_symbol_ids();
        let helpers_needed: usize = self
            .rules
            .values()
            .flat_map(|rules| rules.iter())
            .map(|rule| Self::count_complex_symbols(&rule.rhs))
            .sum();
        let free = occupied.iter().filter(|taken| !**taken).count();
        if helpers_needed > free {
            return Err(GrammarError::SymbolSpaceExhausted);
        }
        // Note: a pre-existing grammar may assign one numeric ID to both
        // a rule and a token (the corpus encodes such dual-use grammars
        // as valid normalization input). Normalization must never create
        // or worsen such a collision: helpers come only from the
        // unoccupied set computed above (#978 falsifier 8).
        // Allocation order is a deterministic function of the occupied
        // set: helpers first take the band above the maximum occupied ID
        // (offset by 1000, the historical auxiliary band), and only when
        // that band would leave the `u16` space do they fall back to the
        // lowest unoccupied IDs. Either way a helper is always actually
        // unoccupied, never the reserved EOF ID, and identical input
        // yields identical helpers.
        let max_occupied = occupied.iter().rposition(|taken| *taken).unwrap_or(0) as u32;
        let mut aux_cursor = max_occupied + 1000;
        let mut aux_fallback_cursor: u32 = 1;
        let mut next_aux = move |occupied: &mut [bool]| -> Result<SymbolId, GrammarError> {
            while aux_cursor <= u32::from(u16::MAX) {
                let candidate = aux_cursor;
                aux_cursor += 1;
                if !occupied[candidate as usize] {
                    occupied[candidate as usize] = true;
                    return Ok(SymbolId(
                        u16::try_from(candidate).expect("candidate is within u16 range"),
                    ));
                }
            }
            while aux_fallback_cursor <= u32::from(u16::MAX) {
                let candidate = aux_fallback_cursor;
                aux_fallback_cursor += 1;
                if !occupied[candidate as usize] {
                    occupied[candidate as usize] = true;
                    return Ok(SymbolId(
                        u16::try_from(candidate).expect("candidate is within u16 range"),
                    ));
                }
            }
            Err(GrammarError::SymbolSpaceExhausted)
        };

        // Helper rules get fresh production slots (#980 PR3): each helper
        // alternative is a distinct rule, so it must not collapse into the
        // shared ProductionId(0) slot. Slots stay shareable elsewhere; this
        // only stops normalize() from manufacturing an ambiguous share.
        let mut next_production_id = self
            .all_rules()
            .map(|rule| rule.production_id.0)
            .max()
            .map_or(0, |max| {
                max.checked_add(1).expect("production id range exhausted")
            });
        let mut fresh_production_id = move || {
            let id = ProductionId(next_production_id);
            next_production_id = next_production_id
                .checked_add(1)
                .expect("production id range exhausted");
            id
        };

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
                            let aux_id = next_aux(&mut occupied)?;

                            // aux -> inner (recursively normalize the inner symbol)
                            let inner_rule = Rule {
                                lhs: aux_id,
                                rhs: vec![*inner.clone()],
                                precedence: None,
                                associativity: None,
                                fields: vec![],
                                production_id: fresh_production_id(),
                            };
                            self.add_rule(inner_rule);

                            // aux -> ε
                            self.add_rule(Rule {
                                lhs: aux_id,
                                rhs: vec![Symbol::Epsilon],
                                precedence: None,
                                associativity: None,
                                fields: vec![],
                                production_id: fresh_production_id(),
                            });

                            new_rhs.push(Symbol::NonTerminal(aux_id));
                        }
                        Symbol::Repeat(inner) => {
                            found_complex = true;
                            // Create aux rule: aux -> aux inner | ε
                            let aux_id = next_aux(&mut occupied)?;

                            // aux -> aux inner (recursively normalize)
                            self.add_rule(Rule {
                                lhs: aux_id,
                                rhs: vec![Symbol::NonTerminal(aux_id), *inner.clone()],
                                precedence: None,
                                associativity: None,
                                fields: vec![],
                                production_id: fresh_production_id(),
                            });

                            // aux -> ε
                            self.add_rule(Rule {
                                lhs: aux_id,
                                rhs: vec![Symbol::Epsilon],
                                precedence: None,
                                associativity: None,
                                fields: vec![],
                                production_id: fresh_production_id(),
                            });

                            new_rhs.push(Symbol::NonTerminal(aux_id));
                        }
                        Symbol::RepeatOne(inner) => {
                            found_complex = true;
                            // Create aux rule: aux -> aux inner | inner
                            let aux_id = next_aux(&mut occupied)?;

                            // aux -> aux inner
                            self.add_rule(Rule {
                                lhs: aux_id,
                                rhs: vec![Symbol::NonTerminal(aux_id), *inner.clone()],
                                precedence: None,
                                associativity: None,
                                fields: vec![],
                                production_id: fresh_production_id(),
                            });

                            // aux -> inner
                            self.add_rule(Rule {
                                lhs: aux_id,
                                rhs: vec![*inner],
                                precedence: None,
                                associativity: None,
                                fields: vec![],
                                production_id: fresh_production_id(),
                            });

                            new_rhs.push(Symbol::NonTerminal(aux_id));
                        }
                        Symbol::Choice(choices) => {
                            found_complex = true;
                            // Create aux rules: aux -> choice1 | choice2 | ...
                            let aux_id = next_aux(&mut occupied)?;

                            for choice in choices {
                                self.add_rule(Rule {
                                    lhs: aux_id,
                                    rhs: vec![choice],
                                    precedence: None,
                                    associativity: None,
                                    fields: vec![],
                                    production_id: fresh_production_id(),
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
        self.refresh_production_ids();
        Ok(self.rules.values().flatten().cloned().collect())
    }

    /// Normalize complex symbols by creating auxiliary rules.
    /// This expands Optional, Repeat, Choice, etc. into standard rules.
    ///
    /// # Panics
    ///
    /// Panics only when the entire non-EOF symbol ID space is occupied and
    /// a helper cannot be allocated. Use [`try_normalize`](Self::try_normalize)
    /// for the typed-error form; this infallible signature is retained for
    /// the existing consumer base (#978).
    pub fn normalize(&mut self) -> Vec<Rule> {
        self.try_normalize()
            .unwrap_or_else(|error| panic!("grammar normalization failed: {error}"))
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

    /// Rules sharing one production slot carry conflicting field/alias metadata
    #[error(
        "Production {production_id} is shared by rules {rule_ids:?} with conflicting field/alias metadata"
    )]
    ConflictingProductionShare {
        /// The ambiguously shared production slot
        production_id: ProductionId,
        /// Rules sharing the slot, in rule order
        rule_ids: Vec<RuleId>,
    },

    /// Conflict in grammar
    #[error("Conflict in grammar: {0}")]
    ConflictError(String),

    /// No unoccupied symbol ID remains for a normalization helper
    #[error("symbol ID space exhausted during normalization: no unoccupied SymbolId remains")]
    SymbolSpaceExhausted,

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
