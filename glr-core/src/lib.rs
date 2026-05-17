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

/// Error types and Result alias for GLR operations.
pub mod error;
/// Convenience result alias for GLR operations.
pub use error::Result as GlrResult;

/// Back-compat alias: prefer `GlrError`; `GLRError` remains for now.
pub use GLRError as GlrError;

mod action_utils;

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

mod conflicts;
pub mod first_follow;
mod lr_items;
mod parse_table;

pub use conflicts::{Conflict, ConflictResolver, ConflictType};
pub use first_follow::FirstFollowSets;
pub use lr_items::{ItemSet, ItemSetCollection, LRItem};
pub use parse_table::{
    Action, ActionCell, GotoIndexing, LexMode, ParseRule, ParseTable, SymbolMetadata,
};

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
    use indexmap::IndexMap;
    use std::collections::BTreeMap;

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
