// GLR core may need unsafe for performance-critical parser algorithms
#![forbid(unsafe_op_in_unsafe_fn)]
#![deny(private_interfaces)]
#![cfg_attr(feature = "strict_api", deny(unreachable_pub))]
#![cfg_attr(not(feature = "strict_api"), warn(unreachable_pub))]
#![cfg_attr(feature = "strict_docs", warn(missing_docs))]
#![cfg_attr(not(feature = "strict_docs"), allow(missing_docs))]
// Keep surface stable without big refactors:
#![allow(
    clippy::ptr_arg,
    clippy::explicit_counter_loop,
    clippy::needless_range_loop,
    clippy::unused_enumerate_index
)]

//! GLR parser generation algorithms for Adze
//! This module implements the core GLR state machine generation and conflict resolution
//!
//! ## Contracts & Invariants
//!
//! This crate maintains several critical invariants for correct parsing:
//!
//! ### EOF Symbol Invariants
//! - EOF symbol must be either Tree-sitter's zero sentinel or a terminal sentinel
//!   id at or beyond the terminal boundary
//!   (`token_count + external_token_count`).
//! - EOF symbol must not be the internal ERROR sentinel
//!   (`parse_forest::ERROR_SYMBOL`, currently 0xFFFF).
//! - EOF symbol is always present in the symbol_to_index mapping
//! - EOF column actions are byte-for-byte copies of the TS "end" column,
//!   guaranteeing per-state equality.
//!
//! ### Error Recovery Invariants
//! - `has_error`: true if any error chunks exist in the parse forest
//! - `missing`: count of unique missing terminal symbols inserted
//! - `cost`: total error recovery cost (insertions + deletions)
//! - No double counting: each missing symbol counted exactly once
//! - Extras (whitespace/comments) are never inserted during recovery
//!
//! ### Table Normalization
//! - Action cells are sorted deterministically by action type and value
//! - Duplicate actions are removed from cells
//! - Action ordering: Shift < Reduce < Accept < Error < Recover < Fork
//!
//! ### API Stability
//! - `ForestView` trait is sealed and cannot be implemented outside this crate
//! - `Action` enum is marked `#[non_exhaustive]` for future extensibility
//! - Test-only APIs are gated behind `test-helpers` feature
//!
//! ### Validation
//! Enable the `strict-invariants` feature to validate parse tables at runtime.
//! This adds overhead but catches invariant violations early in development.

use adze_ir::*;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Error types and Result alias for GLR operations.
pub mod error;
/// Convenience result alias for GLR operations.
pub use error::Result as GlrResult;

/// Back-compat alias: prefer `GlrError`; `GLRError` remains for now.
pub use GLRError as GlrError;

/// Conflict inspection API for analyzing GLR parse table conflicts
pub mod conflict_inspection;

mod automaton;
pub use automaton::build_lr1_automaton;

// Re-export key types from adze-ir for API consumers
/// Re-exported IR types used throughout GLR construction.
pub use adze_ir::{Grammar, RuleId, StateId, SymbolId};

/// Stable imports for downstream users during 0.8.0-dev.
pub mod prelude {
    pub use crate::{FirstFollowSets, ParseTable, build_lr1_automaton};
}

// Keep available, but don't promise public docs yet:
#[doc(hidden)]
pub mod advanced_conflict;
#[doc(hidden)]
pub mod conflict_resolution;
#[doc(hidden)]
pub mod conflict_visualizer;
#[doc(hidden)]
pub mod disambiguation;
#[doc(hidden)]
pub mod gss;
#[doc(hidden)]
pub mod gss_arena;
#[doc(hidden)]
pub mod parse_forest;

pub mod driver;
pub mod forest_view;
pub mod stack;
/// Telemetry counters for tracking GLR parser operations.
pub mod telemetry;
/// Tree-sitter compatible lexer interface for GLR parsing.
pub mod ts_lexer;

/// ParseTable serialization for GLR mode
#[cfg(feature = "serialization")]
pub mod serialization;

// Trace macro for debugging GLR conflicts and decisions
/// Internal tracing macro used by the GLR runtime in debug/test builds.
#[cfg(any(feature = "glr_trace", feature = "debug_glr"))]
#[macro_export]
macro_rules! debug_trace {
    ($($t:tt)*) => { eprintln!("[GLR] {}", format!($($t)*)); }
}
#[cfg(not(any(feature = "glr_trace", feature = "debug_glr")))]
#[macro_export]
macro_rules! debug_trace {
    ($($t:tt)*) => {};
}

/// Backward-compatible trace macro.
#[cfg(any(feature = "glr_trace", feature = "debug_glr"))]
#[macro_export]
macro_rules! glr_trace {
    ($($t:tt)*) => { debug_trace!($($t)*); }
}
#[cfg(not(any(feature = "glr_trace", feature = "debug_glr")))]
#[macro_export]
macro_rules! glr_trace {
    ($($t:tt)*) => { debug_trace!($($t)*); }
}

#[doc(hidden)]
pub mod perf_optimizations;
#[doc(hidden)]
pub mod precedence_compare;
#[doc(hidden)]
pub mod symbol_comparison;
#[doc(hidden)]
pub mod version_info;

pub mod lib_v2;

#[cfg(any(test, feature = "test-api"))]
/// Utilities for constructing test parse tables and grammars.
pub mod test_helpers;

#[cfg(test)]
/// Simple symbol allocator used in tests.
pub mod test_symbol_alloc;

#[doc(hidden)]
pub use advanced_conflict::{
    ConflictAnalyzer, ConflictStats, PrecedenceDecision, PrecedenceResolver,
};
#[doc(hidden)]
pub use conflict_resolution::{RuntimeConflictResolver, VecWrapperResolver};
#[doc(hidden)]
pub use conflict_visualizer::{ConflictVisualizer, generate_dot_graph};
#[doc(hidden)]
pub use gss::{GSSStats, GraphStructuredStack, StackNode};
#[doc(hidden)]
pub use parse_forest::{ForestNode, ParseError, ParseForest, ParseNode, ParseTree};
#[doc(hidden)]
pub use perf_optimizations::{ParseTableCache, PerfStats, StackDeduplicator, StackPool};
#[doc(hidden)]
pub use precedence_compare::{
    PrecedenceComparison, PrecedenceInfo, StaticPrecedenceResolver, compare_precedences,
};
#[doc(hidden)]
pub use symbol_comparison::{compare_symbols, compare_versions_with_symbols};
#[doc(hidden)]
pub use version_info::{CompareResult, VersionInfo, compare_versions};

// Public API exports
/// The main GLR parser driver.
pub use driver::Driver;
/// Core parse forest types and views.
pub use forest_view::{Forest, ForestView, Span};

/// Internal performance counters (diagnostics only).
#[cfg(feature = "perf_counters")]
#[cfg_attr(feature = "strict_docs", allow(missing_docs))]
pub mod perf {
    use std::sync::atomic::{AtomicU64, Ordering};

    /// Snapshot of performance counter values.
    #[derive(Clone, Debug, Default)]
    pub struct Counters {
        /// Number of shift operations.
        pub shifts: u64,
        /// Number of reduce operations.
        pub reductions: u64,
        /// Number of parser forks.
        pub forks: u64,
        /// Number of stack merges.
        pub merges: u64,
    }

    static SHIFTS: AtomicU64 = AtomicU64::new(0);
    static REDUCTIONS: AtomicU64 = AtomicU64::new(0);
    static FORKS: AtomicU64 = AtomicU64::new(0);
    static MERGES: AtomicU64 = AtomicU64::new(0);

    /// Increment the shift counter by `n`.
    #[inline]
    pub fn inc_shifts(n: u64) {
        SHIFTS.fetch_add(n, Ordering::Relaxed);
    }

    /// Increment the reduction counter by `n`.
    #[inline]
    pub fn inc_reductions(n: u64) {
        REDUCTIONS.fetch_add(n, Ordering::Relaxed);
    }

    /// Increment the fork counter by `n`.
    #[inline]
    pub fn inc_forks(n: u64) {
        FORKS.fetch_add(n, Ordering::Relaxed);
    }

    /// Increment the merge counter by `n`.
    #[inline]
    pub fn inc_merges(n: u64) {
        MERGES.fetch_add(n, Ordering::Relaxed);
    }

    /// Take a snapshot of the current counter values.
    pub fn snapshot() -> Counters {
        Counters {
            shifts: SHIFTS.load(Ordering::Relaxed),
            reductions: REDUCTIONS.load(Ordering::Relaxed),
            forks: FORKS.load(Ordering::Relaxed),
            merges: MERGES.load(Ordering::Relaxed),
        }
    }

    /// Atomic read-and-clear (consistent snapshot)
    pub fn take() -> Counters {
        Counters {
            shifts: SHIFTS.swap(0, Ordering::Relaxed),
            reductions: REDUCTIONS.swap(0, Ordering::Relaxed),
            forks: FORKS.swap(0, Ordering::Relaxed),
            merges: MERGES.swap(0, Ordering::Relaxed),
        }
    }

    /// Reset all counters to zero.
    pub fn reset() {
        SHIFTS.store(0, Ordering::Relaxed);
        REDUCTIONS.store(0, Ordering::Relaxed);
        FORKS.store(0, Ordering::Relaxed);
        MERGES.store(0, Ordering::Relaxed);
    }
}

/// Internal performance counters (diagnostics only).
#[cfg(not(feature = "perf_counters"))]
#[cfg_attr(feature = "strict_docs", allow(missing_docs))]
pub mod perf {
    /// Snapshot of performance counter values (no-op when disabled).
    #[derive(Clone, Debug, Default)]
    pub struct Counters {
        /// Number of shift operations.
        pub shifts: u64,
        /// Number of reduce operations.
        pub reductions: u64,
        /// Number of parser forks.
        pub forks: u64,
        /// Number of stack merges.
        pub merges: u64,
    }

    /// No-op: increment shift counter.
    #[inline(always)]
    pub fn inc_shifts(_: u64) {}

    /// No-op: increment reduction counter.
    #[inline(always)]
    pub fn inc_reductions(_: u64) {}

    /// No-op: increment fork counter.
    #[inline(always)]
    pub fn inc_forks(_: u64) {}

    /// No-op: increment merge counter.
    #[inline(always)]
    pub fn inc_merges(_: u64) {}

    /// Returns default (zeroed) counters.
    #[inline(always)]
    pub fn snapshot() -> Counters {
        Counters::default()
    }

    /// Present even when disabled so benches/tests compile unchanged.
    #[inline(always)]
    pub fn take() -> Counters {
        Counters::default()
    }

    /// No-op: reset counters.
    #[inline(always)]
    pub fn reset() {}
}

/// FIRST/FOLLOW set computation for GLR grammars.
pub mod first_follow;
pub use first_follow::FirstFollowSets;

/// LR(1) item for GLR parsing
#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct LRItem {
    /// Owning rule for this item/state
    pub rule_id: RuleId,
    /// Position within the rule's RHS
    pub position: usize,
    /// Lookahead symbol for LR(1) parsing
    pub lookahead: SymbolId,
}

impl LRItem {
    /// Construct an `LRItem` from its owning rule, dot position, and lookahead symbol.
    pub fn new(rule_id: RuleId, position: usize, lookahead: SymbolId) -> Self {
        Self {
            rule_id,
            position,
            lookahead,
        }
    }

    /// Check if this item is at the end of the rule (reduce item)
    pub fn is_reduce_item(&self, grammar: &Grammar) -> bool {
        if let Some(rule) = grammar
            .all_rules()
            .find(|r| r.production_id.0 == self.rule_id.0)
        {
            // Special case: epsilon rules (A -> epsilon) are reduce items at position 0
            // because epsilon doesn't need to be "consumed" - it represents empty string
            if rule.rhs.len() == 1 && matches!(rule.rhs[0], Symbol::Epsilon) {
                return true; // Always a reduce item for epsilon rules
            }

            self.position >= rule.rhs.len()
        } else {
            false
        }
    }

    /// Get the symbol after the dot (next symbol to parse)
    pub fn next_symbol<'a>(&self, grammar: &'a Grammar) -> Option<&'a Symbol> {
        if let Some(rule) = grammar
            .all_rules()
            .find(|r| r.production_id.0 == self.rule_id.0)
        {
            rule.rhs.get(self.position)
        } else {
            None
        }
    }
}

/// Set of LR(1) items representing a parser state
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemSet {
    /// The LR(1) item set that defines this state's closure
    pub items: BTreeSet<LRItem>,
    /// Unique identifier for this state in the canonical collection
    pub id: StateId,
}

impl ItemSet {
    /// Create a new empty item set with the given state ID
    pub fn new(id: StateId) -> Self {
        Self {
            items: BTreeSet::new(),
            id,
        }
    }

    /// Add an LR(1) item to this item set
    pub fn add_item(&mut self, item: LRItem) {
        self.items.insert(item);
    }

    /// Compute closure of this item set
    pub fn closure(
        &mut self,
        grammar: &Grammar,
        first_follow: &FirstFollowSets,
    ) -> Result<(), GLRError> {
        let _initial_size = self.items.len();

        let mut added = true;
        let mut _iteration = 0;
        while added {
            added = false;
            _iteration += 1;
            let current_items: Vec<_> = self.items.iter().cloned().collect();

            for item in current_items {
                if let Some(Symbol::NonTerminal(symbol_id)) = item.next_symbol(grammar) {
                    // Find all rules with this symbol as LHS
                    if let Some(rules) = grammar.get_rules_for_symbol(*symbol_id) {
                        for rule in rules {
                            // Compute FIRST of β α where β is the rest of the current rule
                            // and α is the lookahead
                            let mut beta = Vec::new();
                            if let Some(current_rule) = grammar
                                .all_rules()
                                .find(|r| r.production_id.0 == item.rule_id.0)
                            {
                                beta.extend_from_slice(&current_rule.rhs[item.position + 1..]);
                            }
                            beta.push(Symbol::Terminal(item.lookahead));

                            let first_beta_alpha = first_follow.first_of_sequence(&beta)?;

                            // Add new items for each symbol in FIRST(β α)
                            for lookahead_idx in first_beta_alpha.ones() {
                                let new_item = LRItem::new(
                                    RuleId(rule.production_id.0),
                                    0,
                                    SymbolId(lookahead_idx as u16),
                                );

                                if !self.items.contains(&new_item) {
                                    self.items.insert(new_item);
                                    added = true;
                                    if rule.rhs.is_empty() {
                                        // Empty production
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Closure complete
        Ok(())
    }

    /// Compute GOTO for a given symbol
    pub fn goto(
        &self,
        symbol: &Symbol,
        grammar: &Grammar,
        _first_follow: &FirstFollowSets,
    ) -> ItemSet {
        let mut new_set = ItemSet::new(StateId(0)); // ID will be assigned later

        // Add all items where the dot can advance over the given symbol
        for item in &self.items {
            if let Some(next_sym) = item.next_symbol(grammar)
                && std::mem::discriminant(next_sym) == std::mem::discriminant(symbol)
            {
                match (next_sym, symbol) {
                    (Symbol::Terminal(a), Symbol::Terminal(b))
                    | (Symbol::NonTerminal(a), Symbol::NonTerminal(b))
                    | (Symbol::External(a), Symbol::External(b))
                        if a == b =>
                    {
                        let new_item = LRItem::new(item.rule_id, item.position + 1, item.lookahead);
                        new_set.add_item(new_item);
                    }
                    _ => {}
                }
            }
        }

        // Compute closure of the new set
        let _ = new_set.closure(grammar, _first_follow);
        new_set
    }
}

/// Collection of all LR(1) item sets (parser states)
#[derive(Debug, Clone)]
#[cfg_attr(feature = "strict_docs", allow(missing_docs))]
pub struct ItemSetCollection {
    /// All computed LR(1) item sets (parser states).
    pub sets: Vec<ItemSet>,
    /// GOTO transitions: `(from_state, symbol) -> to_state`.
    pub goto_table: IndexMap<(StateId, SymbolId), StateId>,
    /// Track which symbols in goto_table are terminals (true) vs non-terminals (false)
    pub symbol_is_terminal: IndexMap<SymbolId, bool>,
}

impl ItemSetCollection {
    /// Build canonical collection of LR(1) item sets for augmented grammar
    pub fn build_canonical_collection_augmented(
        grammar: &Grammar,
        first_follow: &FirstFollowSets,
        augmented_start: SymbolId,
        _original_start: SymbolId,
        eof_symbol: SymbolId,
    ) -> Self {
        let mut collection = Self {
            sets: Vec::new(),
            goto_table: IndexMap::new(),
            symbol_is_terminal: IndexMap::new(),
        };

        // Create initial state with the augmented start rule S' -> S $
        let mut initial_set = ItemSet::new(StateId(0));

        // Find the augmented start rule
        if let Some(augmented_rules) = grammar.get_rules_for_symbol(augmented_start) {
            for rule in augmented_rules {
                // Add S' -> • S with lookahead $ (EOF)
                let start_item = LRItem::new(
                    RuleId(rule.production_id.0),
                    0,
                    eof_symbol, // EOF symbol
                );
                initial_set.add_item(start_item);
            }
        }

        // Compute closure
        let _ = initial_set.closure(grammar, first_follow);
        debug_trace!(
            "Initial state 0 after closure has {} items:",
            initial_set.items.len()
        );

        // Track what symbols we expect transitions for
        let mut expected_terminals = std::collections::BTreeSet::new();
        let mut expected_nonterminals = std::collections::BTreeSet::new();

        for item in &initial_set.items {
            // Print each item to debug
            if let Some(rule) = grammar
                .all_rules()
                .find(|r| r.production_id.0 == item.rule_id.0)
            {
                let mut rhs_str = String::new();
                for (idx, sym) in rule.rhs.iter().enumerate() {
                    if idx == item.position {
                        rhs_str.push_str(" • ");
                    }
                    match sym {
                        Symbol::Terminal(id) => rhs_str.push_str(&format!("T({}) ", id.0)),
                        Symbol::NonTerminal(id) => rhs_str.push_str(&format!("NT({}) ", id.0)),
                        _ => rhs_str.push_str("? "),
                    }
                }
                if item.position == rule.rhs.len() {
                    rhs_str.push_str(" • ");
                }
                debug_trace!(
                    "  Item: NT({}) -> {}, lookahead={}",
                    rule.lhs.0,
                    rhs_str,
                    item.lookahead.0
                );

                // Track what symbol is next
                if item.position < rule.rhs.len() {
                    match &rule.rhs[item.position] {
                        Symbol::Terminal(t) => {
                            expected_terminals.insert(*t);
                        }
                        Symbol::NonTerminal(nt) => {
                            expected_nonterminals.insert(*nt);
                        }
                        _ => {}
                    }
                }
            }
        }

        debug_trace!("State 0 expects transitions for:");
        debug_trace!("  Terminals: {:?}", expected_terminals);
        debug_trace!("  Nonterminals: {:?}", expected_nonterminals);

        collection.sets.push(initial_set);
        let mut state_counter = 1;

        // Build all reachable states (same as before)
        let mut i = 0;
        while i < collection.sets.len() {
            let current_set = collection.sets[i].clone();

            // Debug: Print all items in this state
            for item in &current_set.items {
                if let Some(rule) = grammar
                    .all_rules()
                    .find(|r| r.production_id.0 == item.rule_id.0)
                {
                    let mut rhs_str = String::new();
                    for (idx, sym) in rule.rhs.iter().enumerate() {
                        if idx == item.position {
                            rhs_str.push_str(" • ");
                        }
                        rhs_str.push_str(&format!("{:?} ", sym));
                    }
                    if item.position == rule.rhs.len() {
                        rhs_str.push_str(" • ");
                    }
                    // "  [{}] {:?} -> {} , lookahead={}"
                }
            }

            // Find all symbols that can be shifted from this state
            let mut symbols = BTreeSet::new();
            let mut _terminal_count = 0;
            let mut _non_terminal_count = 0;
            if i == 0 {
                debug_trace!("\n=== State 0 Analysis ===");
                debug_trace!("State 0 has {} items:", current_set.items.len());
            }
            for (_idx, item) in current_set.items.iter().enumerate() {
                if i == 0 {
                    // Print the item details
                    if let Some(rule) = grammar
                        .all_rules()
                        .find(|r| r.production_id.0 == item.rule_id.0)
                    {
                        let mut item_str = String::new();
                        item_str.push_str(&format!("NT({}) -> ", rule.lhs.0));
                        for (pos, sym) in rule.rhs.iter().enumerate() {
                            if pos == item.position {
                                item_str.push_str("• ");
                            }
                            match sym {
                                Symbol::Terminal(t) => item_str.push_str(&format!("T({}) ", t.0)),
                                Symbol::NonTerminal(nt) => {
                                    item_str.push_str(&format!("NT({}) ", nt.0))
                                }
                                Symbol::External(e) => item_str.push_str(&format!("EXT({}) ", e.0)),
                                _ => item_str.push_str(&format!("{:?} ", sym)),
                            }
                        }
                        if item.position == rule.rhs.len() {
                            item_str.push_str("• ");
                        }
                        debug_trace!("  Item {}: {} (rule_id={})", _idx, item_str, item.rule_id.0);
                    }
                }

                if let Some(symbol) = item.next_symbol(grammar) {
                    match symbol {
                        Symbol::Terminal(_id) => {
                            _terminal_count += 1;
                        }
                        Symbol::NonTerminal(_id) => {
                            _non_terminal_count += 1;
                        }
                        Symbol::External(_id) => {
                            _terminal_count += 1; // Count externals as terminals
                        }
                        _ => {}
                    }
                    symbols.insert(symbol.clone());
                    if i == 0 {
                        debug_trace!("    -> next symbol: {:?}", symbol);
                    }
                }
            }

            if i == 0 {
                debug_trace!("\nState 0 summary:");
                debug_trace!("  Total symbols that can be shifted: {}", symbols.len());
                debug_trace!("  Terminals: {}", _terminal_count);
                debug_trace!("  Non-terminals: {}", _non_terminal_count);
                debug_trace!("  Symbols: {:?}\n", symbols);
            }

            // Debug: symbols.len(), _terminal_count, _non_terminal_count
            // Compute GOTO for each symbol
            for symbol in symbols {
                let goto_set = current_set.goto(&symbol, grammar, first_follow);

                if !goto_set.items.is_empty() {
                    // Check if this set already exists
                    let existing_state = collection
                        .sets
                        .iter()
                        .find(|set| set.items == goto_set.items)
                        .map(|set| set.id);

                    let target_state = if let Some(existing_id) = existing_state {
                        existing_id
                    } else {
                        // Add new state
                        let new_id = StateId(state_counter);
                        let mut new_set = goto_set;
                        new_set.id = new_id;
                        collection.sets.push(new_set);
                        state_counter += 1;
                        new_id
                    };

                    // Add to GOTO table
                    let symbol_id = match symbol {
                        Symbol::Terminal(id) | Symbol::NonTerminal(id) | Symbol::External(id) => id,
                        Symbol::Optional(_)
                        | Symbol::Repeat(_)
                        | Symbol::RepeatOne(_)
                        | Symbol::Choice(_)
                        | Symbol::Sequence(_)
                        | Symbol::Epsilon => {
                            panic!(
                                "Complex symbols should be normalized before LR item generation"
                            );
                        }
                    };
                    if current_set.id.0 == 0 {
                        debug_trace!(
                            "  State 0 GOTO: symbol {:?} -> state {}",
                            symbol_id,
                            target_state.0
                        );
                    }
                    collection
                        .goto_table
                        .insert((current_set.id, symbol_id), target_state);

                    // Track whether this symbol is a terminal or non-terminal
                    let is_terminal = matches!(symbol, Symbol::Terminal(_) | Symbol::External(_));
                    collection.symbol_is_terminal.insert(symbol_id, is_terminal);
                    // "DEBUG: Added goto({}, {}) = {}"
                }
            }

            i += 1;
        }

        collection
    }

    /// Build canonical collection of LR(1) item sets.
    ///
    /// # Examples
    ///
    /// ```
    /// use adze_glr_core::{FirstFollowSets, ItemSetCollection};
    /// use adze_ir::*;
    ///
    /// let mut grammar = Grammar::new("simple".into());
    /// let a = SymbolId(1);
    /// let s = SymbolId(10);
    ///
    /// grammar.tokens.insert(a, Token { name: "a".into(), pattern: TokenPattern::String("a".into()), fragile: false });
    /// grammar.rule_names.insert(s, "S".into());
    /// grammar.rules.insert(s, vec![
    ///     Rule { lhs: s, rhs: vec![Symbol::Terminal(a)], precedence: None, associativity: None, fields: vec![], production_id: ProductionId(0) },
    /// ]);
    ///
    /// let ff = FirstFollowSets::compute(&grammar).unwrap();
    /// let collection = ItemSetCollection::build_canonical_collection(&grammar, &ff);
    /// assert!(!collection.sets.is_empty(), "should have at least one state");
    /// ```
    pub fn build_canonical_collection(grammar: &Grammar, first_follow: &FirstFollowSets) -> Self {
        let mut collection = Self {
            sets: Vec::new(),
            goto_table: IndexMap::new(),
            symbol_is_terminal: IndexMap::new(),
        };

        // Create initial state with augmented start rule
        let mut initial_set = ItemSet::new(StateId(0));

        // Find the start symbol (LHS of the first rule in grammar)
        if let Some(start_symbol) = grammar.start_symbol() {
            // Debug: grammar.rule_names.get(&start_symbol)

            // Add items for ALL rules with the start symbol as LHS
            if let Some(start_rules) = grammar.get_rules_for_symbol(start_symbol) {
                for rule in start_rules.iter() {
                    // Debug: idx, rule.lhs, rule.rhs, rule.production_id.0
                    let start_item = LRItem::new(
                        RuleId(rule.production_id.0),
                        0,
                        SymbolId(0), // EOF symbol
                    );
                    initial_set.add_item(start_item);
                    // Debug: rule.production_id.0
                }
            }

            // Compute closure
            let _ = initial_set.closure(grammar, first_follow);
        }

        // Only add initial set if it has items
        if initial_set.items.is_empty() {
            // Handle empty initial set if needed
        } else {
            for _item in &initial_set.items {
                // Debug: item.rule_id.0, item.position, item.lookahead.0
            }
        }

        collection.sets.push(initial_set);
        let mut state_counter = 1;

        // Build all reachable states
        let mut i = 0;
        while i < collection.sets.len() {
            let current_set = collection.sets[i].clone();

            // Debug: Print all items in this state
            for item in &current_set.items {
                if let Some(rule) = grammar
                    .all_rules()
                    .find(|r| r.production_id.0 == item.rule_id.0)
                {
                    let mut rhs_str = String::new();
                    for (idx, sym) in rule.rhs.iter().enumerate() {
                        if idx == item.position {
                            rhs_str.push_str(" • ");
                        }
                        rhs_str.push_str(&format!("{:?} ", sym));
                    }
                    if item.position == rule.rhs.len() {
                        rhs_str.push_str(" • ");
                    }
                    // "  [{}] {:?} -> {} , lookahead={}"
                }
            }

            // Find all symbols that can be shifted from this state
            let mut symbols = BTreeSet::new();
            let mut _terminal_count = 0;
            let mut _non_terminal_count = 0;
            if i == 0 {
                debug_trace!("\n=== State 0 Analysis ===");
                debug_trace!("State 0 has {} items:", current_set.items.len());
            }
            for (_idx, item) in current_set.items.iter().enumerate() {
                if i == 0 {
                    // Print the item details
                    if let Some(rule) = grammar
                        .all_rules()
                        .find(|r| r.production_id.0 == item.rule_id.0)
                    {
                        let mut item_str = String::new();
                        item_str.push_str(&format!("NT({}) -> ", rule.lhs.0));
                        for (pos, sym) in rule.rhs.iter().enumerate() {
                            if pos == item.position {
                                item_str.push_str("• ");
                            }
                            match sym {
                                Symbol::Terminal(t) => item_str.push_str(&format!("T({}) ", t.0)),
                                Symbol::NonTerminal(nt) => {
                                    item_str.push_str(&format!("NT({}) ", nt.0))
                                }
                                Symbol::External(e) => item_str.push_str(&format!("EXT({}) ", e.0)),
                                _ => item_str.push_str(&format!("{:?} ", sym)),
                            }
                        }
                        if item.position == rule.rhs.len() {
                            item_str.push_str("• ");
                        }
                        debug_trace!("  Item {}: {} (rule_id={})", _idx, item_str, item.rule_id.0);
                    }
                }

                if let Some(symbol) = item.next_symbol(grammar) {
                    match symbol {
                        Symbol::Terminal(_id) => {
                            _terminal_count += 1;
                        }
                        Symbol::NonTerminal(_id) => {
                            _non_terminal_count += 1;
                        }
                        Symbol::External(_id) => {
                            _terminal_count += 1; // Count externals as terminals
                        }
                        _ => {}
                    }
                    symbols.insert(symbol.clone());
                    if i == 0 {
                        debug_trace!("    -> next symbol: {:?}", symbol);
                    }
                }
            }

            if i == 0 {
                debug_trace!("\nState 0 summary:");
                debug_trace!("  Total symbols that can be shifted: {}", symbols.len());
                debug_trace!("  Terminals: {}", _terminal_count);
                debug_trace!("  Non-terminals: {}", _non_terminal_count);
                debug_trace!("  Symbols: {:?}\n", symbols);
            }

            // Debug: symbols.len(), _terminal_count, _non_terminal_count
            for item in &current_set.items {
                if let Some(symbol) = item.next_symbol(grammar) {
                    let _symbol_id = match &symbol {
                        Symbol::Terminal(id) | Symbol::NonTerminal(id) | Symbol::External(id) => id,
                        _ => panic!("Complex symbol"),
                    };
                    // "  Item rule_id={}, position={}, next_symbol={:?} (id={})"
                }
            }

            for symbol in &symbols {
                let _symbol_id = match symbol {
                    Symbol::Terminal(id) | Symbol::NonTerminal(id) | Symbol::External(id) => id,
                    _ => panic!("Complex symbol"),
                };
            }

            // Compute GOTO for each symbol
            for symbol in symbols {
                let goto_set = current_set.goto(&symbol, grammar, first_follow);

                if !goto_set.items.is_empty() {
                    // Check if this set already exists
                    let existing_state = collection
                        .sets
                        .iter()
                        .find(|set| set.items == goto_set.items)
                        .map(|set| set.id);

                    let target_state = if let Some(existing_id) = existing_state {
                        existing_id
                    } else {
                        // Add new state
                        let new_id = StateId(state_counter);
                        let mut new_set = goto_set;
                        new_set.id = new_id;
                        collection.sets.push(new_set);
                        state_counter += 1;
                        new_id
                    };

                    // Add to GOTO table
                    let symbol_id = match symbol {
                        Symbol::Terminal(id) | Symbol::NonTerminal(id) | Symbol::External(id) => id,
                        Symbol::Optional(_)
                        | Symbol::Repeat(_)
                        | Symbol::RepeatOne(_)
                        | Symbol::Choice(_)
                        | Symbol::Sequence(_)
                        | Symbol::Epsilon => {
                            panic!(
                                "Complex symbols should be normalized before LR item generation"
                            );
                        }
                    };
                    if current_set.id.0 == 0 {
                        debug_trace!(
                            "  State 0 GOTO: symbol {:?} -> state {}",
                            symbol_id,
                            target_state.0
                        );
                    }
                    collection
                        .goto_table
                        .insert((current_set.id, symbol_id), target_state);

                    // Track whether this symbol is a terminal or non-terminal
                    let is_terminal = matches!(symbol, Symbol::Terminal(_) | Symbol::External(_));
                    collection.symbol_is_terminal.insert(symbol_id, is_terminal);
                    // "DEBUG: Added goto({}, {}) = {}"
                }
            }

            i += 1;
        }

        collection
    }
}

/// Lexer mode for a parser state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serialization", derive(Serialize, Deserialize))]
pub struct LexMode {
    /// Internal lexer DFA state
    pub lex_state: u16,
    /// State for the external scanner (if any)
    pub external_lex_state: u16,
}

/// How GOTO table columns are indexed
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serialization", derive(Serialize, Deserialize))]
pub enum GotoIndexing {
    /// Use nonterminal_to_index mapping (standard)
    NonterminalMap,
    /// Use SymbolId.0 directly as column index (some table generators)
    DirectSymbolId,
}

/// GLR-compatible parse table supporting multiple actions per state
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serialization", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "strict_docs", allow(missing_docs))]
pub struct ParseTable {
    /// ACTION table: indexed by `[state][terminal]` using symbol_to_index
    pub action_table: Vec<Vec<ActionCell>>,
    /// GOTO table: indexed by `[state][nonterminal]` using nonterminal_to_index or direct ID
    pub goto_table: Vec<Vec<StateId>>,
    /// Metadata (name, visibility, etc.) for each symbol in the grammar.
    pub symbol_metadata: Vec<SymbolMetadata>,
    /// Total number of parser states.
    pub state_count: usize,
    /// Total number of symbols (terminals + non-terminals).
    pub symbol_count: usize,
    /// Maps terminal symbols to ACTION table column indices
    pub symbol_to_index: BTreeMap<SymbolId, usize>,
    /// Index -> SymbolId, perfectly mirroring `symbol_to_index`.
    pub index_to_symbol: Vec<SymbolId>,
    /// For each state, a bitset indicating which external tokens are valid
    pub external_scanner_states: Vec<Vec<bool>>,
    /// Grammar rules for reduction
    pub rules: Vec<ParseRule>,
    /// Maps nonterminal symbols to GOTO table column indices
    pub nonterminal_to_index: BTreeMap<SymbolId, usize>,
    /// How GOTO table columns are indexed
    pub goto_indexing: GotoIndexing,
    /// EOF symbol ID
    pub eof_symbol: SymbolId,
    /// Start symbol ID
    pub start_symbol: SymbolId,
    /// Grammar metadata
    pub grammar: Grammar,
    /// Initial parser state (default: 0, Tree-sitter uses 1)
    pub initial_state: StateId,
    /// Number of tokens (regular terminals)
    pub token_count: usize,
    /// Number of external tokens (from external scanner)
    pub external_token_count: usize,
    /// Lex modes for each state (length == state_count)
    pub lex_modes: Vec<LexMode>,
    /// Terminal symbols to skip as whitespace/comments
    pub extras: Vec<SymbolId>,
    /// Dynamic precedence for each rule (optional)
    pub dynamic_prec_by_rule: Vec<i16>,
    /// Associativity for each rule: -1=Right, 0=None, +1=Left
    pub rule_assoc_by_rule: Vec<i8>,
    /// Alias sequences for rules
    pub alias_sequences: Vec<Vec<Option<SymbolId>>>,
    /// Field names
    pub field_names: Vec<String>,
    /// Map (rule, child_index) -> field_id
    pub field_map: BTreeMap<(RuleId, u16), u16>,
}

/// Parse rule for reduction
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serialization", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "strict_docs", allow(missing_docs))]
pub struct ParseRule {
    /// Left-hand side non-terminal symbol of the rule.
    pub lhs: SymbolId,
    /// Number of symbols on the right-hand side.
    pub rhs_len: u16,
}

impl Default for ParseTable {
    fn default() -> Self {
        Self {
            action_table: vec![],
            goto_table: vec![],
            symbol_metadata: vec![],
            state_count: 0,
            symbol_count: 0,
            symbol_to_index: BTreeMap::new(),
            index_to_symbol: vec![],
            external_scanner_states: vec![],
            rules: vec![],
            nonterminal_to_index: BTreeMap::new(),
            goto_indexing: GotoIndexing::NonterminalMap,
            eof_symbol: SymbolId(0),
            start_symbol: SymbolId(0),
            grammar: Grammar::new("default".to_string()),
            initial_state: StateId(0),
            token_count: 0,
            external_token_count: 0,
            lex_modes: vec![],
            extras: vec![],
            dynamic_prec_by_rule: vec![],
            rule_assoc_by_rule: vec![],
            alias_sequences: vec![],
            field_names: vec![],
            field_map: BTreeMap::new(),
        }
    }
}

impl ParseTable {
    /// Builder method to auto-detect GOTO indexing
    pub fn with_detected_goto_indexing(mut self) -> Self {
        self.detect_goto_indexing();
        self
    }

    /// Normalize EOF symbol to SymbolId(0) for consistency
    /// This ensures compatibility with various table producers
    pub fn normalize_eof_to_zero(mut self) -> Self {
        // If EOF is already 0, nothing to do
        if self.eof_symbol == SymbolId(0) {
            return self;
        }

        let old_eof = self.eof_symbol;
        // Log the normalization for debugging
        #[cfg(debug_assertions)]
        debug_trace!("Normalizing EOF from {:?} to SymbolId(0)", old_eof);

        // Get the indices for remapping
        let old_idx = self.symbol_to_index.get(&old_eof).copied();
        let zero_idx = self.symbol_to_index.get(&SymbolId(0)).copied();

        // Swap columns in ACTION table if both indices exist
        if let (Some(old_idx), Some(zero_idx)) = (old_idx, zero_idx) {
            for row in &mut self.action_table {
                if old_idx < row.len() && zero_idx < row.len() {
                    row.swap(old_idx, zero_idx);
                }
            }
            // Now: 0 → old_idx, old_eof → (removed)
            self.symbol_to_index.insert(SymbolId(0), old_idx);
            self.symbol_to_index.remove(&old_eof);

            // Update index_to_symbol if it exists
            if old_idx < self.index_to_symbol.len() {
                self.index_to_symbol[old_idx] = SymbolId(0);
            }
        } else if let Some(old_idx) = old_idx {
            // Only old EOF existed: move its column mapping to 0
            self.symbol_to_index.remove(&old_eof);
            self.symbol_to_index.insert(SymbolId(0), old_idx);

            // Update index_to_symbol if it exists
            if old_idx < self.index_to_symbol.len() {
                self.index_to_symbol[old_idx] = SymbolId(0);
            }
        } else {
            // Neither mapped: ensure EOF->0 exists so consumers don't panic
            self.symbol_to_index.insert(SymbolId(0), 0);
        }

        // Update EOF symbol
        self.eof_symbol = SymbolId(0);
        self
    }

    /// Auto-detect the GOTO indexing mode based on table contents
    pub fn detect_goto_indexing(&mut self) {
        // Try to determine if the start symbol has a valid GOTO from state 0
        let start_nt = self.start_symbol;

        // Check if start symbol has entry via nonterminal_to_index
        let col_map = self
            .nonterminal_to_index
            .get(&start_nt)
            .and_then(|&c| self.goto_table.first().and_then(|row| row.get(c)))
            .copied();

        // Check if start symbol has entry via direct symbol ID
        let col_direct = self
            .goto_table
            .first()
            .and_then(|row| row.get(start_nt.0 as usize))
            .copied();

        self.goto_indexing = match (col_map, col_direct) {
            (Some(s), _) if s.0 != 0 => GotoIndexing::NonterminalMap,
            (_, Some(s)) if s.0 != 0 => GotoIndexing::DirectSymbolId,
            // Default to nonterminal map; unit tests will catch a mismatch
            _ => GotoIndexing::NonterminalMap,
        };
    }

    /// Get the terminal boundary (tokens + external tokens)
    #[inline]
    pub fn terminal_boundary(&self) -> usize {
        self.token_count + self.external_token_count
    }

    /// Check if a symbol is a terminal
    #[inline]
    pub fn is_terminal(&self, sym: SymbolId) -> bool {
        (sym.0 as usize) < self.terminal_boundary()
    }

    /// Get valid symbols mask for a state (terminals that have actions)
    pub fn valid_symbols(&self, state: StateId) -> Vec<bool> {
        let n = self.terminal_boundary();
        let mut v = vec![false; n];
        let s = state.0 as usize;
        if s < self.action_table.len() {
            for t in 0..n.min(self.action_table[s].len()) {
                v[t] = !self.action_table[s][t].is_empty();
            }
        }
        v
    }

    /// Get actions for a state and symbol.
    ///
    /// Returns the slice of [`Action`]s for the given `(state, terminal)` pair.
    /// Returns an empty slice when no actions exist.
    ///
    /// # Examples
    ///
    /// ```
    /// use adze_glr_core::{FirstFollowSets, build_lr1_automaton, Action};
    /// use adze_ir::*;
    ///
    /// let mut grammar = Grammar::new("act".into());
    /// let a = SymbolId(1);
    /// let s = SymbolId(10);
    /// grammar.tokens.insert(a, Token { name: "a".into(), pattern: TokenPattern::String("a".into()), fragile: false });
    /// grammar.rule_names.insert(s, "S".into());
    /// grammar.rules.insert(s, vec![
    ///     Rule { lhs: s, rhs: vec![Symbol::Terminal(a)], precedence: None, associativity: None, fields: vec![], production_id: ProductionId(0) },
    /// ]);
    ///
    /// let ff = FirstFollowSets::compute(&grammar).unwrap();
    /// let table = build_lr1_automaton(&grammar, &ff).unwrap();
    ///
    /// // Initial state should have a Shift on terminal 'a'
    /// let actions = table.actions(table.initial_state, a);
    /// assert!(actions.iter().any(|a| matches!(a, Action::Shift(_))));
    /// ```
    #[inline]
    pub fn actions(&self, state: StateId, sym: SymbolId) -> &'_ [Action] {
        let s = state.0 as usize;
        let Some(&col) = self.symbol_to_index.get(&sym) else {
            return &[];
        };
        if s >= self.action_table.len() || col >= self.action_table[s].len() {
            return &[];
        }
        &self.action_table[s][col]
    }

    /// Get goto state for a nonterminal.
    ///
    /// Returns the target state after reducing to `nt` in the given `state`,
    /// or `None` if no transition exists.
    ///
    /// # Examples
    ///
    /// ```
    /// use adze_glr_core::{FirstFollowSets, build_lr1_automaton};
    /// use adze_ir::*;
    ///
    /// let mut grammar = Grammar::new("goto".into());
    /// let a = SymbolId(1);
    /// let s = SymbolId(10);
    /// grammar.tokens.insert(a, Token { name: "a".into(), pattern: TokenPattern::String("a".into()), fragile: false });
    /// grammar.rule_names.insert(s, "S".into());
    /// grammar.rules.insert(s, vec![
    ///     Rule { lhs: s, rhs: vec![Symbol::Terminal(a)], precedence: None, associativity: None, fields: vec![], production_id: ProductionId(0) },
    /// ]);
    ///
    /// let ff = FirstFollowSets::compute(&grammar).unwrap();
    /// let table = build_lr1_automaton(&grammar, &ff).unwrap();
    ///
    /// // After shifting 'a' and reducing S→a, goto(0, S) should exist
    /// let target = table.goto(table.initial_state, s);
    /// assert!(target.is_some(), "goto(initial, S) should exist");
    /// ```
    #[inline]
    pub fn goto(&self, state: StateId, nt: SymbolId) -> Option<StateId> {
        let s = state.0 as usize;
        let &col = self.nonterminal_to_index.get(&nt)?;
        // Allow "no edge" to be represented as a sentinel (e.g., u16::MAX)
        let ns = *self.goto_table.get(s)?.get(col)?;
        (ns.0 != u16::MAX).then_some(ns)
    }

    /// Get rule information by ID
    #[inline]
    pub fn rule(&self, id: RuleId) -> (SymbolId, u16) {
        let r = &self.rules[id.0 as usize];
        (r.lhs, r.rhs_len)
    }

    /// Get EOF symbol
    #[inline]
    pub fn eof(&self) -> SymbolId {
        self.eof_symbol
    }

    /// Get start symbol
    #[inline]
    pub fn start_symbol(&self) -> SymbolId {
        self.start_symbol
    }

    /// Get grammar reference
    #[inline]
    pub fn grammar(&self) -> &Grammar {
        &self.grammar
    }

    /// Get the ERROR symbol (by convention, symbol 0 or -1 in Tree-sitter)
    #[inline]
    pub fn error_symbol(&self) -> SymbolId {
        // Tree-sitter convention: ERROR is typically symbol 0
        // We could also check for a symbol named "ERROR" in the grammar
        SymbolId(0)
    }

    /// Get valid symbols mask for a state (terminals that have actions)
    #[inline]
    pub fn valid_symbols_mask(&self, state: StateId) -> Vec<bool> {
        let n = self.terminal_boundary();
        let mut v = vec![false; n];
        let s = state.0 as usize;
        if s < self.action_table.len() {
            for t in 0..n.min(self.action_table[s].len()) {
                v[t] = !self.action_table[s][t].is_empty();
            }
        }
        v
    }

    /// Get lex mode for a state
    #[inline]
    pub fn lex_mode(&self, state: StateId) -> LexMode {
        let idx = state.0 as usize;
        if idx < self.lex_modes.len() {
            self.lex_modes[idx]
        } else {
            LexMode {
                lex_state: 0,
                external_lex_state: 0,
            }
        }
    }

    /// Check if a symbol is an extra (whitespace/comment)
    #[inline]
    pub fn is_extra(&self, sym: SymbolId) -> bool {
        self.extras.contains(&sym)
    }

    /// Validate parse table invariants
    ///
    /// This method checks critical invariants that must hold for correct parsing:
    /// - EOF symbol is not internal ERROR sentinel
    /// - EOF symbol is a proper sentinel:
    ///   Tree-sitter-style `0` or `>= token_count + external_token_count`
    /// - EOF symbol is present in symbol_to_index mapping
    /// - EOF symbol is present in symbol_to_index mapping
    #[must_use = "validation result must be checked"]
    pub fn validate(&self) -> Result<(), TableError> {
        let terminal_boundary = self.token_count + self.external_token_count;

        debug_assert_ne!(
            self.eof_symbol,
            parse_forest::ERROR_SYMBOL,
            "EOF symbol cannot be the ERROR sentinel"
        );

        if self.eof_symbol == parse_forest::ERROR_SYMBOL {
            return Err(TableError::EofIsError);
        }

        let eof_is_zero_sentinel = self.eof_symbol == SymbolId(0);

        // Check EOF is a terminal sentinel. Tree-sitter-compatible tables use
        // symbol 0 for EOF; other producers may place EOF beyond all terminal
        // and external token symbols.
        if !eof_is_zero_sentinel && (self.eof_symbol.0 as usize) < terminal_boundary {
            return Err(TableError::EofNotSentinel {
                eof: self.eof_symbol.0,
                token_count: self.token_count as u32,
                external_count: self.external_token_count as u32,
            });
        }

        // Check EOF is in symbol_to_index
        if !self.symbol_to_index.contains_key(&self.eof_symbol) {
            return Err(TableError::EofMissingFromIndex);
        }

        // Validate terminal partitions
        let tb = self.terminal_boundary();

        // All extras must be regular terminals
        debug_assert!(
            self.extras
                .iter()
                .all(|&sym| (sym.0 as usize) < self.token_count),
            "all extras must be within [0..token_count)"
        );

        // Regular terminals must not be external
        for sym_id in 0..self.token_count {
            let sym = SymbolId(sym_id as u16);
            debug_assert!(self.is_terminal(sym), "0..token_count are terminals");
            // Regular terminals are not external - we verify this by the band
            debug_assert!(
                (sym.0 as usize) < self.token_count,
                "regular terminals are in [0..token_count)"
            );
        }

        // External tokens must be in their band
        for sym_id in self.token_count..tb {
            let sym = SymbolId(sym_id as u16);
            debug_assert!(self.is_terminal(sym), "external tokens are terminals");
            // External tokens are in the external band by definition
            debug_assert!(
                (sym.0 as usize) >= self.token_count && (sym.0 as usize) < tb,
                "external tokens are in [token_count..terminal_boundary)"
            );
        }

        debug_assert!(
            self.symbol_to_index.contains_key(&self.eof_symbol),
            "EOF must exist in ACTION map"
        );

        Ok(())
    }

    /// Remap GOTO table from NonterminalMap layout to DirectSymbolId layout.
    /// No-op if already DirectSymbolId.
    pub fn remap_goto_to_direct_symbol_id(mut self) -> Self {
        if matches!(self.goto_indexing, GotoIndexing::DirectSymbolId) {
            return self;
        }
        // Establish the max symbol id we need to size rows
        let max_sym = self
            .nonterminal_to_index
            .keys()
            .map(|s| s.0 as usize)
            .max()
            .unwrap_or(0);
        let new_width = max_sym + 1;

        for row in &mut self.goto_table {
            // Defensive check: ensure all column indices are valid
            debug_assert!(
                self.nonterminal_to_index.values().all(|&c| c < row.len()),
                "nonterminal_to_index contains a column >= row width"
            );

            let mut new_row = vec![StateId(0); new_width];
            // Move each mapped nonterminal from its old column into the col = symbol id
            for (sym, &old_col) in &self.nonterminal_to_index {
                if old_col < row.len() {
                    new_row[sym.0 as usize] = row[old_col];
                }
            }
            *row = new_row;
        }
        self.goto_indexing = GotoIndexing::DirectSymbolId;
        self
    }

    /// Remap GOTO table from DirectSymbolId layout to NonterminalMap layout.
    /// No-op if already NonterminalMap.
    pub fn remap_goto_to_nonterminal_map(mut self) -> Self {
        if matches!(self.goto_indexing, GotoIndexing::NonterminalMap) {
            return self;
        }
        // Compute width for the map layout
        let width = self
            .nonterminal_to_index
            .values()
            .copied()
            .max()
            .unwrap_or(0)
            + 1;
        for row in &mut self.goto_table {
            // Defensive check: ensure source indices are valid
            debug_assert!(
                self.nonterminal_to_index
                    .keys()
                    .all(|s| (s.0 as usize) < row.len()),
                "nonterminal_to_index contains a symbol id >= row width"
            );

            let mut new_row = vec![StateId(0); width];
            for (sym, &col) in &self.nonterminal_to_index {
                let src = sym.0 as usize;
                if src < row.len() && col < new_row.len() {
                    new_row[col] = row[src];
                }
            }
            *row = new_row;
        }
        self.goto_indexing = GotoIndexing::NonterminalMap;
        self
    }
}

/// Actions in GLR parse table (supporting multiple actions per state)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
#[cfg_attr(feature = "strict_docs", allow(missing_docs))]
pub enum Action {
    /// Shift the current token and transition to the given state.
    Shift(StateId),
    /// Reduce by the given grammar rule.
    Reduce(RuleId),
    /// Accept the input (parsing complete).
    Accept,
    /// No valid action (syntax error).
    Error,
    /// Tree-sitter error recovery — insert missing node.
    Recover,
    /// GLR fork point — multiple valid actions to explore.
    Fork(Vec<Action>),
}

/// Action cell that can hold multiple actions for GLR
pub type ActionCell = Vec<Action>;

/// Symbol metadata for the parse table
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serialization", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "strict_docs", allow(missing_docs))]
pub struct SymbolMetadata {
    /// Human-readable symbol name.
    pub name: String,
    /// Whether the symbol is visible in the syntax tree.
    pub is_visible: bool,
    /// Whether the symbol is a named node (vs anonymous).
    pub is_named: bool,
    /// Whether the symbol is a supertype node.
    pub is_supertype: bool,
    // Additional fields required by API contracts
    /// Whether the symbol is a terminal (leaf token).
    pub is_terminal: bool,
    /// Whether the symbol is an extra (e.g., whitespace, comments).
    pub is_extra: bool,
    /// Whether the symbol is fragile (invalidated by edits).
    pub is_fragile: bool,
    /// Unique identifier for this symbol.
    pub symbol_id: SymbolId,
}

/// Conflict detection and resolution
#[derive(Debug, Clone)]
#[cfg_attr(feature = "strict_docs", allow(missing_docs))]
pub struct ConflictResolver {
    /// All detected parse table conflicts.
    pub conflicts: Vec<Conflict>,
}

/// Conflict information for GLR parsing
#[derive(Debug, Clone)]
#[cfg_attr(feature = "strict_docs", allow(missing_docs))]
pub struct Conflict {
    /// Parser state where the conflict occurs.
    pub state: StateId,
    /// Lookahead symbol that triggers the conflict.
    pub symbol: SymbolId,
    /// Conflicting actions for this state/symbol pair.
    pub actions: Vec<Action>,
    /// Classification of the conflict.
    pub conflict_type: ConflictType,
}

/// Type of parser conflict
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "strict_docs", allow(missing_docs))]
pub enum ConflictType {
    /// Conflict between a shift action and a reduce action.
    ShiftReduce,
    /// Conflict between two different reduce actions.
    ReduceReduce,
}

impl ConflictResolver {
    /// Detect conflicts in the parse table.
    ///
    /// Scans every item set and reports shift/reduce or reduce/reduce conflicts.
    ///
    /// # Examples
    ///
    /// ```
    /// use adze_glr_core::{ConflictResolver, ConflictType, FirstFollowSets, ItemSetCollection};
    /// use adze_ir::*;
    ///
    /// // E → a | E E  (inherently ambiguous)
    /// let mut grammar = Grammar::new("ambig".into());
    /// let a = SymbolId(1);
    /// let e = SymbolId(10);
    /// grammar.tokens.insert(a, Token { name: "a".into(), pattern: TokenPattern::String("a".into()), fragile: false });
    /// grammar.rule_names.insert(e, "E".into());
    /// grammar.rules.insert(e, vec![
    ///     Rule { lhs: e, rhs: vec![Symbol::Terminal(a)], precedence: None, associativity: None, fields: vec![], production_id: ProductionId(0) },
    ///     Rule { lhs: e, rhs: vec![Symbol::NonTerminal(e), Symbol::NonTerminal(e)], precedence: None, associativity: None, fields: vec![], production_id: ProductionId(1) },
    /// ]);
    ///
    /// let ff = FirstFollowSets::compute(&grammar).unwrap();
    /// let collection = ItemSetCollection::build_canonical_collection(&grammar, &ff);
    /// let resolver = ConflictResolver::detect_conflicts(&collection, &grammar, &ff);
    /// // An ambiguous grammar like E → a | E E should have conflicts
    /// assert!(!resolver.conflicts.is_empty(), "should detect conflicts");
    /// ```
    pub fn detect_conflicts(
        item_sets: &ItemSetCollection,
        grammar: &Grammar,
        _first_follow: &FirstFollowSets,
    ) -> Self {
        let mut conflicts = Vec::new();

        for item_set in &item_sets.sets {
            let mut actions_by_symbol: IndexMap<SymbolId, Vec<Action>> = IndexMap::new();

            // Collect all possible actions for each symbol in this state
            for item in &item_set.items {
                if item.is_reduce_item(grammar) {
                    // Check if this is a reduction to the start symbol with EOF lookahead
                    let mut is_accept = false;

                    // Find the rule that corresponds to this rule ID
                    if let Some(start_symbol) = grammar.start_symbol() {
                        // Look through all rules to find the one with this rule ID
                        for rule in grammar.all_rules() {
                            if rule.production_id.0 == item.rule_id.0 {
                                // Check if this rule reduces to the start symbol and we have EOF lookahead
                                is_accept =
                                    rule.lhs == start_symbol && item.lookahead == SymbolId(0);
                                break;
                            }
                        }
                    }

                    let action = if is_accept {
                        Action::Accept
                    } else {
                        Action::Reduce(item.rule_id)
                    };

                    actions_by_symbol
                        .entry(item.lookahead)
                        .or_default()
                        .push(action);
                } else if let Some(symbol) = item.next_symbol(grammar) {
                    // Shift action
                    let symbol_id = match symbol {
                        Symbol::Terminal(id) | Symbol::NonTerminal(id) | Symbol::External(id) => {
                            *id
                        }
                        Symbol::Optional(_)
                        | Symbol::Repeat(_)
                        | Symbol::RepeatOne(_)
                        | Symbol::Choice(_)
                        | Symbol::Sequence(_)
                        | Symbol::Epsilon => {
                            panic!(
                                "Complex symbols should be normalized before LR item generation"
                            );
                        }
                    };

                    if let Some(target_state) = item_sets.goto_table.get(&(item_set.id, symbol_id))
                    {
                        let action = Action::Shift(*target_state);
                        actions_by_symbol.entry(symbol_id).or_default().push(action);
                    }
                }
            }

            // Check for conflicts
            for (symbol_id, actions) in actions_by_symbol {
                if actions.len() > 1 {
                    let conflict_type = if actions.iter().any(|a| matches!(a, Action::Shift(_)))
                        && actions.iter().any(|a| matches!(a, Action::Reduce(_)))
                    {
                        ConflictType::ShiftReduce
                    } else {
                        ConflictType::ReduceReduce
                    };

                    conflicts.push(Conflict {
                        state: item_set.id,
                        symbol: symbol_id,
                        actions,
                        conflict_type,
                    });
                }
            }
        }

        Self { conflicts }
    }

    /// Resolve conflicts using precedence and associativity rules
    pub fn resolve_conflicts(&mut self, grammar: &Grammar) {
        // Clone conflicts to avoid borrowing issues
        let mut conflicts_to_resolve = self.conflicts.clone();
        for conflict in &mut conflicts_to_resolve {
            // Apply Tree-sitter's exact conflict resolution logic
            self.resolve_single_conflict(conflict, grammar);
        }
        self.conflicts = conflicts_to_resolve;
    }

    fn resolve_single_conflict(&self, conflict: &mut Conflict, grammar: &Grammar) {
        // Implement Tree-sitter's exact precedence and associativity resolution
        // This is where we port the C logic for conflict resolution

        match conflict.conflict_type {
            ConflictType::ShiftReduce => {
                // Apply precedence rules between shift and reduce
                // Higher precedence wins, same precedence uses associativity
                self.resolve_shift_reduce_conflict(conflict, grammar);
            }
            ConflictType::ReduceReduce => {
                // Apply precedence rules between multiple reduces
                // Usually choose the rule that appears first in the grammar
                self.resolve_reduce_reduce_conflict(conflict, grammar);
            }
        }
    }

    fn resolve_shift_reduce_conflict(&self, conflict: &mut Conflict, grammar: &Grammar) {
        // Use Tree-sitter's exact precedence comparison logic
        let precedence_resolver = StaticPrecedenceResolver::from_grammar(grammar);

        let mut shift_action = None;
        let mut reduce_action = None;

        // Find shift and reduce actions
        for action in &conflict.actions {
            match action {
                Action::Shift(_) => shift_action = Some(action.clone()),
                Action::Reduce(_) => reduce_action = Some(action.clone()),
                _ => {}
            }
        }

        match (shift_action, reduce_action) {
            (Some(shift), Some(reduce)) => {
                // Get precedence info for shift token
                let shift_prec = precedence_resolver.token_precedence(conflict.symbol);

                // Get precedence info for reduce rule
                let reduce_prec = if let Action::Reduce(rule_id) = &reduce {
                    precedence_resolver.rule_precedence(*rule_id)
                } else {
                    None
                };

                // Compare precedences
                // PRECEDENCE RESOLUTION: When precedence can definitively resolve the conflict,
                // we eliminate the lower-precedence action (not just re-order).
                // This ensures correct parsing for unambiguous grammars.
                match compare_precedences(shift_prec, reduce_prec) {
                    PrecedenceComparison::PreferShift => {
                        // Shift wins - eliminate reduce action
                        conflict.actions = vec![shift];
                    }
                    PrecedenceComparison::PreferReduce => {
                        // Reduce wins - eliminate shift action
                        conflict.actions = vec![reduce];
                    }
                    PrecedenceComparison::Error => {
                        // Non-associative conflict - this is an error
                        // Keep Fork to signal ambiguity for error reporting
                        conflict.actions = vec![Action::Fork(vec![shift, reduce])];
                    }
                    PrecedenceComparison::None => {
                        // No precedence info - use GLR fork to explore all paths
                        conflict.actions = vec![Action::Fork(vec![shift, reduce])];
                    }
                }
            }
            _ => {
                // Should not happen in a shift/reduce conflict
                // Keep original actions
            }
        }
    }

    fn resolve_reduce_reduce_conflict(&self, conflict: &mut Conflict, _grammar: &Grammar) {
        // Choose the rule that appears first in the grammar
        // This is Tree-sitter's default behavior for reduce/reduce conflicts

        let mut best_action = None;
        let mut best_rule_id = u16::MAX;

        for action in &conflict.actions {
            if let Action::Reduce(rule_id) = action
                && rule_id.0 < best_rule_id
            {
                best_rule_id = rule_id.0;
                best_action = Some(action.clone());
            }
        }

        if let Some(action) = best_action {
            conflict.actions = vec![action];
        }
    }
}

/// Error types for GLR processing
#[derive(Debug, thiserror::Error)]
#[cfg_attr(feature = "strict_docs", allow(missing_docs))]
pub enum GLRError {
    /// Error originating from grammar validation.
    #[error("Grammar error: {0}")]
    GrammarError(#[from] GrammarError),

    /// Conflict resolution could not be completed.
    #[error("Conflict resolution failed: {0}")]
    ConflictResolution(String),

    /// State machine construction failed.
    #[error("State machine generation failed: {0}")]
    StateMachine(String),

    /// Parse table failed post-generation validation.
    #[error("Table validation failed: {0}")]
    TableValidation(TableError),

    /// Grammar contains complex symbols that must be normalized first.
    #[error("Complex symbols must be normalized before {operation}")]
    ComplexSymbolsNotNormalized { operation: String },

    /// A complex symbol was found where a simple one was expected.
    #[error("Expected {expected} symbol, found complex symbol")]
    ExpectedSimpleSymbol { expected: String },

    /// A symbol is in an invalid state for the requested operation.
    #[error("Invalid symbol state during {operation}")]
    InvalidSymbolState { operation: String },
}

/// Errors related to parse table validation
#[derive(Debug, thiserror::Error)]
#[cfg_attr(feature = "strict_docs", allow(missing_docs))]
pub enum TableError {
    /// The EOF symbol ID collides with the built-in ERROR symbol.
    #[error("EOF symbol collides with ERROR")]
    EofIsError,

    /// EOF symbol ID is too low; it must be zero or a sentinel beyond all tokens.
    #[error(
        "EOF symbol must be >= token_count + external_token_count or be 0 (EOF: {eof}, tokens: {token_count}, externals: {external_count})"
    )]
    EofNotSentinel {
        eof: u16,
        token_count: u32,
        external_count: u32,
    },

    /// The EOF symbol is not registered in the symbol-to-index mapping.
    #[error("EOF not present in symbol_to_index")]
    EofMissingFromIndex,

    /// ACTION table EOF column has mismatched accept/reduce entries.
    #[error("EOF column parity mismatch at state {0}")]
    EofParityMismatch(u16),
}

/// Check if a symbol can derive the start symbol through unit productions
#[allow(dead_code)]
fn can_derive_start(grammar: &Grammar, symbol: SymbolId, start: SymbolId) -> bool {
    if symbol == start {
        return true;
    }

    // Check if there's a rule symbol -> start
    if let Some(rules) = grammar.get_rules_for_symbol(symbol) {
        for rule in rules {
            if rule.rhs.len() == 1
                && let Symbol::NonTerminal(target) = &rule.rhs[0]
                && *target == start
            {
                return true;
            }
        }
    }

    false
}

/// Build LR(1) automaton (parse table) from grammar.
///
/// Constructs an augmented grammar, builds the canonical LR(1) collection,
/// and fills the ACTION / GOTO tables.
///
/// # Examples
///
/// ```
/// use adze_glr_core::{FirstFollowSets, build_lr1_automaton, Action};
/// use adze_ir::*;
///
/// let mut grammar = Grammar::new("ab".into());
/// let a = SymbolId(1);
/// let s = SymbolId(10);
///
/// grammar.tokens.insert(a, Token { name: "a".into(), pattern: TokenPattern::String("a".into()), fragile: false });
/// grammar.rule_names.insert(s, "S".into());
/// grammar.rules.insert(s, vec![
///     Rule { lhs: s, rhs: vec![Symbol::Terminal(a)], precedence: None, associativity: None, fields: vec![], production_id: ProductionId(0) },
/// ]);
///
/// let ff = FirstFollowSets::compute(&grammar).unwrap();
/// let table = build_lr1_automaton(&grammar, &ff).unwrap();
///
/// assert!(table.state_count > 0);
/// assert_eq!(table.start_symbol(), s);
/// // The table should contain an Accept action somewhere on EOF
/// let eof = table.eof();
/// let has_accept = (0..table.state_count).any(|st| {
///     table.actions(StateId(st as u16), eof).iter().any(|a| matches!(a, Action::Accept))
/// });
/// assert!(has_accept, "table must have an Accept action");
/// ```
/// Sanity check parse table for correctness
#[must_use = "validation result must be checked"]
pub fn sanity_check_tables(pt: &ParseTable) -> Result<(), String> {
    // 1) ACCEPT must exist on EOF in the state that has S'→S•.
    let eof_col = pt
        .symbol_to_index
        .get(&pt.eof_symbol)
        .ok_or_else(|| format!("EOF symbol {} not in symbol_to_index", pt.eof_symbol.0))?;

    let accept_somewhere = pt.action_table.iter().any(|row| {
        row.get(*eof_col)
            .and_then(|cell| cell.iter().find(|a| matches!(a, Action::Accept)))
            .is_some()
    });
    if !accept_somewhere {
        return Err("No ACCEPT on EOF found in action table".to_string());
    }

    // 2) Every production's LHS must be reachable via some goto.
    for pid in 0..pt.rules.len() {
        let lhs = pt.rules[pid].lhs;
        let lhs_idx = pt
            .symbol_to_index
            .get(&lhs)
            .ok_or_else(|| format!("LHS symbol {} not in symbol_to_index", lhs.0))?;

        // LHS must be a non-terminal column
        if *lhs_idx < pt.token_count {
            return Err(format!(
                "LHS must be a non-terminal column (pid={}, lhs_idx={}, token_count={})",
                pid, lhs_idx, pt.token_count
            ));
        }

        let any = pt
            .goto_table
            .iter()
            .any(|row| row.get(*lhs_idx).is_some_and(|s| s.0 != 0));
        if !any {
            return Err(format!("No goto(state, lhs(pid={})) present", pid));
        }
    }

    // 3) Verify index_to_symbol is consistent with symbol_to_index
    for (sym, &idx) in &pt.symbol_to_index {
        if idx >= pt.index_to_symbol.len() {
            return Err(format!(
                "symbol_to_index has index {} but index_to_symbol has length {}",
                idx,
                pt.index_to_symbol.len()
            ));
        }
        if pt.index_to_symbol[idx] != *sym {
            return Err(format!(
                "index_to_symbol[{}] = {} but should be {}",
                idx, pt.index_to_symbol[idx].0, sym.0
            ));
        }
    }

    Ok(())
}

#[cfg(test)]
use automaton::actions::normalize_action;

/// Build LR(1) automaton using the GlrResult type alias
///
/// This is a convenience wrapper that uses the crate-level Result type.
/// Use this when migrating code to the new error handling pattern.
pub fn build_lr1_automaton_res(
    grammar: &Grammar,
    first_follow: &FirstFollowSets,
) -> GlrResult<ParseTable> {
    build_lr1_automaton(grammar, first_follow)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lr_item_creation() {
        let item = LRItem::new(RuleId(1), 2, SymbolId(3));
        assert_eq!(item.rule_id, RuleId(1));
        assert_eq!(item.position, 2);
        assert_eq!(item.lookahead, SymbolId(3));
    }

    #[test]
    fn test_lr_item_equality() {
        let item1 = LRItem::new(RuleId(1), 2, SymbolId(3));
        let item2 = LRItem::new(RuleId(1), 2, SymbolId(3));
        let item3 = LRItem::new(RuleId(1), 3, SymbolId(3));

        assert_eq!(item1, item2);
        assert_ne!(item1, item3);

        // Test hashing
        let mut set = std::collections::BTreeSet::new();
        set.insert(item1.clone());
        assert!(set.contains(&item1));
        assert!(set.contains(&item2));
        assert!(!set.contains(&item3));
    }

    #[test]
    fn test_item_set_creation() {
        let mut item_set = ItemSet::new(StateId(0));
        let item = LRItem::new(RuleId(1), 0, SymbolId(0));
        item_set.add_item(item.clone());

        assert_eq!(item_set.id, StateId(0));
        assert!(item_set.items.contains(&item));
        assert_eq!(item_set.items.len(), 1);
    }

    #[test]
    fn test_item_set_duplicate_items() {
        let mut item_set = ItemSet::new(StateId(0));
        let item = LRItem::new(RuleId(1), 0, SymbolId(0));

        item_set.add_item(item.clone());
        item_set.add_item(item.clone()); // Add same item again

        // Should only contain one item (no duplicates)
        assert_eq!(item_set.items.len(), 1);
    }

    #[test]
    fn test_first_follow_empty_grammar() {
        let grammar = Grammar::new("test".to_string());
        let first_follow = FirstFollowSets::compute(&grammar).unwrap();

        assert!(first_follow.first.is_empty());
        assert!(first_follow.follow.is_empty());
    }

    #[test]
    fn test_first_follow_simple_grammar() {
        let mut grammar = Grammar::new("test".to_string());

        // Add a simple rule: S -> a
        let rule = Rule {
            lhs: SymbolId(0),                         // S
            rhs: vec![Symbol::Terminal(SymbolId(1))], // a
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(0),
        };
        grammar.rules.entry(SymbolId(0)).or_default().push(rule);

        // Add the terminal token
        let token = Token {
            name: "a".to_string(),
            pattern: TokenPattern::String("a".to_string()),
            fragile: false,
        };
        grammar.tokens.insert(SymbolId(1), token);

        let first_follow = FirstFollowSets::compute(&grammar).unwrap();

        // FIRST(S) should contain 'a'
        assert!(first_follow.first.contains_key(&SymbolId(0)));
        if let Some(first_s) = first_follow.first(SymbolId(0)) {
            assert!(first_s.contains(1)); // Terminal 'a' has id 1
        }

        // S should not be nullable
        assert!(!first_follow.is_nullable(SymbolId(0)));
    }

    #[test]
    fn test_first_follow_nullable_rule() {
        let mut grammar = Grammar::new("test".to_string());

        // Add a rule: S -> ε (empty rule)
        let rule = Rule {
            lhs: SymbolId(0), // S
            rhs: vec![],      // empty
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(0),
        };
        grammar.rules.entry(SymbolId(0)).or_default().push(rule);

        let first_follow = FirstFollowSets::compute(&grammar).unwrap();

        // S should be nullable
        assert!(first_follow.is_nullable(SymbolId(0)));
    }

    #[test]
    fn test_first_of_sequence() {
        let mut grammar = Grammar::new("test".to_string());

        // Add tokens
        let token_a = Token {
            name: "a".to_string(),
            pattern: TokenPattern::String("a".to_string()),
            fragile: false,
        };
        grammar.tokens.insert(SymbolId(1), token_a);

        let token_b = Token {
            name: "b".to_string(),
            pattern: TokenPattern::String("b".to_string()),
            fragile: false,
        };
        grammar.tokens.insert(SymbolId(2), token_b);

        let first_follow = FirstFollowSets::compute(&grammar).unwrap();

        // Test FIRST of sequence [a, b]
        let sequence = vec![Symbol::Terminal(SymbolId(1)), Symbol::Terminal(SymbolId(2))];
        let first_seq = first_follow.first_of_sequence(&sequence).unwrap();

        // Should contain only 'a' (first terminal)
        assert!(first_seq.contains(1));
        assert!(!first_seq.contains(2));
    }

    #[test]
    fn test_action_types() {
        let shift = Action::Shift(StateId(1));
        let reduce = Action::Reduce(RuleId(2));
        let accept = Action::Accept;
        let error = Action::Error;
        let fork = Action::Fork(vec![shift.clone(), reduce.clone()]);

        match shift {
            Action::Shift(StateId(1)) => {}
            _ => panic!("Expected shift action"),
        }

        match reduce {
            Action::Reduce(RuleId(2)) => {}
            _ => panic!("Expected reduce action"),
        }

        match accept {
            Action::Accept => {}
            _ => panic!("Expected accept action"),
        }

        match error {
            Action::Error => {}
            _ => panic!("Expected error action"),
        }

        match fork {
            Action::Fork(actions) => {
                assert_eq!(actions.len(), 2);
                assert_eq!(actions[0], shift);
                assert_eq!(actions[1], reduce);
            }
            _ => panic!("Expected fork action"),
        }
    }

    #[test]
    fn test_action_equality() {
        let shift1 = Action::Shift(StateId(1));
        let shift2 = Action::Shift(StateId(1));
        let shift3 = Action::Shift(StateId(2));

        assert_eq!(shift1, shift2);
        assert_ne!(shift1, shift3);

        let reduce1 = Action::Reduce(RuleId(1));
        let reduce2 = Action::Reduce(RuleId(1));

        assert_eq!(reduce1, reduce2);
        assert_ne!(shift1, reduce1);
    }

    #[test]
    fn test_symbol_metadata() {
        let metadata = SymbolMetadata {
            name: "expression".to_string(),
            is_visible: true,
            is_named: true,
            is_supertype: false,
            // Additional fields required by API contracts
            is_terminal: false,
            is_extra: false,
            is_fragile: false,
            symbol_id: SymbolId(1),
        };

        assert_eq!(metadata.name, "expression");
        assert!(metadata.is_visible);
        assert!(metadata.is_named);
        assert!(!metadata.is_supertype);
        assert!(!metadata.is_terminal);
        assert!(!metadata.is_extra);
        assert!(!metadata.is_fragile);
        assert_eq!(metadata.symbol_id, SymbolId(1));
    }

    #[test]
    fn test_conflict_types() {
        let shift_reduce = ConflictType::ShiftReduce;
        let reduce_reduce = ConflictType::ReduceReduce;

        assert_eq!(shift_reduce, ConflictType::ShiftReduce);
        assert_eq!(reduce_reduce, ConflictType::ReduceReduce);
        assert_ne!(shift_reduce, reduce_reduce);
    }

    #[test]
    fn test_conflict_creation() {
        let conflict = Conflict {
            state: StateId(5),
            symbol: SymbolId(10),
            actions: vec![Action::Shift(StateId(1)), Action::Reduce(RuleId(2))],
            conflict_type: ConflictType::ShiftReduce,
        };

        assert_eq!(conflict.state, StateId(5));
        assert_eq!(conflict.symbol, SymbolId(10));
        assert_eq!(conflict.actions.len(), 2);
        assert_eq!(conflict.conflict_type, ConflictType::ShiftReduce);
    }

    #[test]
    fn test_conflict_resolver_creation() {
        let resolver = ConflictResolver { conflicts: vec![] };

        assert!(resolver.conflicts.is_empty());
    }

    #[test]
    fn test_parse_table_creation() {
        let parse_table = ParseTable {
            action_table: vec![vec![vec![Action::Error]; 5]; 3], // 3 states, 5 symbols
            goto_table: vec![vec![StateId(0); 5]; 3],
            symbol_metadata: vec![],
            state_count: 3,
            symbol_count: 5,
            symbol_to_index: BTreeMap::new(),
            index_to_symbol: vec![],
            external_scanner_states: vec![],
            rules: vec![],
            nonterminal_to_index: BTreeMap::new(),
            goto_indexing: GotoIndexing::NonterminalMap,
            eof_symbol: SymbolId(0),
            start_symbol: SymbolId(1),
            grammar: Grammar::new("test".to_string()),
            initial_state: StateId(0),
            token_count: 3,
            external_token_count: 0,
            lex_modes: vec![
                LexMode {
                    lex_state: 0,
                    external_lex_state: 0
                };
                3
            ],
            extras: vec![],
            dynamic_prec_by_rule: vec![],
            rule_assoc_by_rule: vec![],
            alias_sequences: vec![],
            field_names: vec![],
            field_map: BTreeMap::new(),
        };

        assert_eq!(parse_table.state_count, 3);
        assert_eq!(parse_table.symbol_count, 5);
        assert_eq!(parse_table.action_table.len(), 3);
        assert_eq!(parse_table.goto_table.len(), 3);
        assert_eq!(parse_table.action_table[0].len(), 5);
        assert_eq!(parse_table.goto_table[0].len(), 5);
    }

    #[test]
    fn test_lr_item_reduce_check() {
        let mut grammar = Grammar::new("test".to_string());

        // Add a rule: S -> a b
        let rule = Rule {
            lhs: SymbolId(0),
            rhs: vec![Symbol::Terminal(SymbolId(1)), Symbol::Terminal(SymbolId(2))],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(0),
        };
        grammar.rules.entry(SymbolId(0)).or_default().push(rule);

        // Item at position 0: S -> • a b
        let item1 = LRItem::new(RuleId(0), 0, SymbolId(0));
        assert!(!item1.is_reduce_item(&grammar));

        // Item at position 1: S -> a • b
        let item2 = LRItem::new(RuleId(0), 1, SymbolId(0));
        assert!(!item2.is_reduce_item(&grammar));

        // Item at position 2: S -> a b •
        let item3 = LRItem::new(RuleId(0), 2, SymbolId(0));
        assert!(item3.is_reduce_item(&grammar));
    }

    #[test]
    fn test_lr_item_next_symbol() {
        let mut grammar = Grammar::new("test".to_string());

        // Add a rule: S -> a b
        let rule = Rule {
            lhs: SymbolId(0),
            rhs: vec![Symbol::Terminal(SymbolId(1)), Symbol::Terminal(SymbolId(2))],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(0),
        };
        grammar.rules.entry(SymbolId(0)).or_default().push(rule);

        // Item at position 0: S -> • a b
        let item1 = LRItem::new(RuleId(0), 0, SymbolId(0));
        if let Some(symbol) = item1.next_symbol(&grammar) {
            match symbol {
                Symbol::Terminal(SymbolId(1)) => {}
                _ => panic!("Expected terminal symbol with id 1"),
            }
        } else {
            panic!("Expected next symbol");
        }

        // Item at position 1: S -> a • b
        let item2 = LRItem::new(RuleId(0), 1, SymbolId(0));
        if let Some(symbol) = item2.next_symbol(&grammar) {
            match symbol {
                Symbol::Terminal(SymbolId(2)) => {}
                _ => panic!("Expected terminal symbol with id 2"),
            }
        } else {
            panic!("Expected next symbol");
        }

        // Item at position 2: S -> a b •
        let item3 = LRItem::new(RuleId(0), 2, SymbolId(0));
        assert!(item3.next_symbol(&grammar).is_none());
    }

    #[test]
    fn test_item_set_collection_creation() {
        let collection = ItemSetCollection {
            sets: vec![],
            goto_table: IndexMap::new(),
            symbol_is_terminal: IndexMap::new(),
        };

        assert!(collection.sets.is_empty());
        assert!(collection.goto_table.is_empty());
    }

    #[test]
    fn test_glr_error_types() {
        let grammar_error = GLRError::GrammarError(GrammarError::InvalidFieldOrdering);
        let conflict_error = GLRError::ConflictResolution("Test conflict".to_string());
        let state_error = GLRError::StateMachine("Test state machine error".to_string());

        match grammar_error {
            GLRError::GrammarError(_) => {}
            _ => panic!("Expected grammar error"),
        }

        match conflict_error {
            GLRError::ConflictResolution(msg) => assert_eq!(msg, "Test conflict"),
            _ => panic!("Expected conflict resolution error"),
        }

        match state_error {
            GLRError::StateMachine(msg) => assert_eq!(msg, "Test state machine error"),
            _ => panic!("Expected state machine error"),
        }
    }

    #[test]
    fn test_item_set_equality() {
        let mut set1 = ItemSet::new(StateId(0));
        let mut set2 = ItemSet::new(StateId(1));

        let item1 = LRItem::new(RuleId(1), 0, SymbolId(0));
        let item2 = LRItem::new(RuleId(2), 1, SymbolId(1));

        set1.add_item(item1.clone());
        set1.add_item(item2.clone());

        set2.add_item(item1);
        set2.add_item(item2);

        // Sets should be equal based on items, not ID
        assert_eq!(set1.items, set2.items);
        assert_ne!(set1.id, set2.id);
    }

    #[test]
    fn test_recursive_fork_normalization() {
        // Create a messy nested Fork action
        let mut action = Action::Fork(vec![
            Action::Fork(vec![
                Action::Reduce(RuleId(3)),
                Action::Shift(StateId(2)),
                Action::Reduce(RuleId(1)),
            ]),
            Action::Shift(StateId(1)),
            Action::Fork(vec![
                Action::Accept,
                Action::Shift(StateId(4)),
                Action::Error,
            ]),
        ]);

        // Normalize it
        normalize_action(&mut action);

        // Check that inner forks are sorted
        if let Action::Fork(ref actions) = action {
            // First inner fork should have actions sorted: Shift < Reduce
            if let Action::Fork(ref inner) = actions[0] {
                assert_eq!(inner.len(), 3);
                assert!(matches!(inner[0], Action::Shift(StateId(2))));
                assert!(matches!(inner[1], Action::Reduce(RuleId(1))));
                assert!(matches!(inner[2], Action::Reduce(RuleId(3))));
            }

            // Last inner fork should have actions sorted: Shift < Accept < Error
            if let Action::Fork(ref inner) = actions[2] {
                assert_eq!(inner.len(), 3);
                assert!(matches!(inner[0], Action::Shift(StateId(4))));
                assert!(matches!(inner[1], Action::Accept));
                assert!(matches!(inner[2], Action::Error));
            }
        } else {
            panic!("Expected Fork action");
        }
    }
}
