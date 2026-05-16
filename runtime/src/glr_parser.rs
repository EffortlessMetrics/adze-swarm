//! GLR (Generalized LR) Parser Implementation
//!
//! This module implements a GLR parser that can handle ambiguous grammars by maintaining
//! multiple parse stacks simultaneously. When the parser encounters a shift/reduce or
//! reduce/reduce conflict, it forks the parse stack and explores both possibilities.
#![cfg_attr(feature = "strict_docs", allow(missing_docs))]
//! ## Algorithm Overview
//!
//! The parser uses a two-phase approach for processing tokens:
//!
//! ### Phase 1: Reduction Saturation
//! Before consuming any token, the parser performs all possible reductions on all active
//! stacks. This is crucial because:
//! - Reductions can cascade (one reduction enables another)
//! - We must complete all reductions before shifting to maintain correctness
//! - This prevents tokens from being consumed prematurely or processed multiple times
//!
//! ### Phase 2: Token Processing  
//! After all reductions are complete, the parser:
//! - Processes shift actions for the current token
//! - Handles fork actions (creating new stacks for conflicts)
//! - Processes error recovery if no valid actions exist
//!
//! ## Fork/Merge Strategy
//!
//! When conflicts occur, the parser:
//! 1. Forks the current stack into multiple stacks (one per conflicting action)
//! 2. Processes each fork independently
//! 3. Merges stacks that reach the same state with the same parse tree structure
//! 4. Uses dynamic precedence to resolve ambiguities when possible
//!
//! ## Error Recovery
//!
//! The parser supports configurable error recovery strategies:
//! - Token deletion (skip unexpected tokens)
//! - Token insertion (insert missing tokens)
//! - Panic mode (skip to synchronization points)
//!
//! ## Configuration Constants
//!
//! ### Safe Deduplication Threshold
//! Only perform pointer-based deduplication when new_stacks.len() exceeds this threshold.
//! This prevents performance overhead for small stack sets while ensuring correctness
//! for larger sets where duplicate stacks could impact performance.
//! Default: 10. Override with env var ADZE_SAFE_DEDUP_N for testing.
//!
//! ## Example Usage
//!
//! ```rust,ignore
//! use adze::glr_parser::GLRParser;
//! use adze::glr_lexer::GLRLexer;
//! use adze_ir::{Grammar, SymbolId};
//! use adze_glr_core::ParseTable;
//!
//! // Create parser with grammar and parse table (grammar and parse_table provided by your app)
//! # fn example(grammar: Grammar, parse_table: ParseTable) {
//! let mut parser = GLRParser::new(parse_table, grammar);
//!
//! // Create lexer and tokenize input
//! let mut lexer = GLRLexer::new(&grammar);
//! let tokens = lexer.tokenize("1 + 2 * 3").unwrap();
//!
//! // Process each token
//! for token in tokens {
//!     parser.process_token(token.symbol, &token.text, token.start_byte);
//! }
//!
//! // Process EOF and get result
//! parser.process_eof();
//! match parser.finish() {
//!     Ok(tree) => println!("Parse successful!"),
//!     Err(e) => println!("Parse failed: {}", e),
//! }
//! # }
//! ```

use crate::error_recovery::{ErrorRecoveryConfig, ErrorRecoveryState, RecoveryAction};
use crate::subtree::{Subtree, SubtreeNode};
use adze_glr_core::{Action, CompareResult, ParseTable, compare_versions};
use adze_glr_core::{FirstFollowSets, VecWrapperResolver};
use adze_ir::{Grammar, PrecedenceKind, Rule, Symbol};
use adze_ir::{RuleId, StateId, SymbolId};
use std::collections::VecDeque;
use std::sync::Arc;

/// Error types specific to GLR parsing operations.
#[derive(Debug, thiserror::Error)]
pub enum GLRError {
    /// Complex symbol found in rule that should have been normalized during grammar preprocessing.
    ///
    /// GLR parsing requires that all complex symbols (Optional, Repeat, RepeatOne, Choice, Sequence, Epsilon)
    /// are normalized into simpler forms during grammar compilation. If this error occurs, it indicates
    /// that the grammar preprocessing step did not complete properly.
    ///
    /// ## Resolution
    /// Ensure that grammar normalization is run before GLR parsing. Complex symbols should be
    /// expanded into equivalent rules with only Terminal, NonTerminal, and External symbols.
    #[error(
        "Complex symbol '{symbol_type}' not normalized in rule {production_id:?} at position {position}. Complex symbols must be normalized before GLR parsing."
    )]
    ComplexSymbolNotNormalized {
        /// The type of complex symbol that was encountered
        symbol_type: String,
        /// The production ID where the symbol was found
        production_id: adze_ir::ProductionId,
        /// The position within the rule's RHS where the symbol occurred
        position: usize,
    },
}

/// Result type for GLR parsing operations.
pub type GLRResult<T> = Result<T, GLRError>;

// Debug macro for GLR parser
#[cfg(feature = "debug_glr")]
macro_rules! debug_glr {
    ($($arg:tt)*) => {
        if std::env::var("RUST_LOG")
            .ok()
            .unwrap_or_default()
            .contains("debug")
        {
            println!($($arg)*);
        }
    };
}

#[cfg(not(feature = "debug_glr"))]
macro_rules! debug_glr {
    ($($arg:tt)*) => {};
}

#[path = "glr_parser/ambiguity.rs"]
mod ambiguity;
#[path = "glr_parser/config.rs"]
mod config;
#[path = "glr_parser/parse_stack.rs"]
mod parse_stack;
#[path = "glr_parser/recovery.rs"]
mod recovery;
#[path = "glr_parser/telemetry.rs"]
mod telemetry;

pub use ambiguity::{AlternativeSummary, AmbiguitySummary, SelectionReason};
#[allow(unused_imports)]
pub use config::{DEFAULT_SAFE_DEDUP_THRESHOLD, safe_dedup_threshold};
pub use parse_stack::ParseStack;
#[cfg(feature = "glr_telemetry")]
use telemetry::TelemetryCounters;

use ambiguity::{
    SelectedCompleteStack, subtree_node_count, subtree_selection_key, version_selection_reason,
};

/// Recovery event for tracking what recovery actions were taken
#[derive(Clone, Copy, Debug)]
#[allow(dead_code)]
enum RecoveryEvent {
    /// Synthesized a token (zero-width insertion)
    Insert(SymbolId),
    /// Dropped the current lookahead
    Delete(SymbolId),
    /// Popped N symbols from stacks
    Pop(usize),
}

/// GLR parser engine
pub struct GLRParser {
    /// Parse table
    table: ParseTable,

    /// Grammar for reductions
    grammar: Grammar,

    /// Active parse stacks
    stacks: Vec<ParseStack>,

    /// Next stack ID
    next_stack_id: usize,

    /// Stacks to process in the next step
    pending_stacks: VecDeque<usize>,

    /// Error recovery configuration
    error_recovery: Option<ErrorRecoveryConfig>,

    /// Error recovery state
    recovery_state: Option<ErrorRecoveryState>,

    /// Conflict resolver for vec wrapper conflicts
    #[allow(dead_code)]
    vec_wrapper_resolver: Option<VecWrapperResolver>,

    /// Total input length in bytes (set when process_eof is called)
    input_length: usize,

    /// Number of tokens deleted in a row for recovery
    deleted_in_row: usize,

    /// Number of tokens inserted in a row for recovery
    inserted_in_row: usize,

    /// Pending synthetic tokens to process
    pending_synthetic_tokens: VecDeque<SymbolId>,

    /// Telemetry counters for performance monitoring
    #[cfg(feature = "glr_telemetry")]
    telemetry: TelemetryCounters,
}

#[allow(dead_code)]
impl GLRParser {
    /// Calculate priority for an action based on precedence
    #[inline]
    fn action_priority(&self, action: &Action) -> i32 {
        use Action::*;

        // Highest: Accept
        if matches!(action, Accept) {
            return 3_000_000;
        }

        // Pull dynamic precedence if this is a reduce
        let mut prec = 0i32;
        if let Reduce(rid) = action {
            // Get dynamic precedence for this rule
            if (rid.0 as usize) < self.table.dynamic_prec_by_rule.len() {
                prec = self.table.dynamic_prec_by_rule[rid.0 as usize] as i32;
            }

            // Get associativity from the rule: +1 left, -1 right, 0 none
            let assoc_bias = if (rid.0 as usize) < self.table.rule_assoc_by_rule.len() {
                self.table.rule_assoc_by_rule[rid.0 as usize] as i32
            } else {
                0
            };

            // Combine precedence and associativity
            prec = prec.saturating_add(assoc_bias);

            // Bump reduces with positive precedence above plain shift
            if prec > 0 {
                return 2_000_000 + prec;
            }
            // Neutral reduce (slightly below shift to prefer shift in S/R conflicts)
            return 1_500_000 + prec;
        }

        // Plain Shift (default TS policy prefers shift over no-prec reduce)
        if matches!(action, Shift(_)) {
            return 2_000_000;
        }

        0 // Error/other
    }

    /// Get a rule by its ID
    #[allow(dead_code)]
    fn get_rule(&self, rule_id: RuleId) -> Option<&Rule> {
        let mut rule_counter = 0;
        for rules in self.grammar.rules.values() {
            for rule in rules {
                if rule_counter == rule_id.0 as usize {
                    return Some(rule);
                }
                rule_counter += 1;
            }
        }
        None
    }

    pub fn new(table: ParseTable, grammar: Grammar) -> Self {
        let initial_stack = ParseStack::new(StateId(0), 0);

        // Compute FIRST/FOLLOW sets for the resolver
        let vec_wrapper_resolver = FirstFollowSets::compute(&grammar)
            .ok()
            .map(|first_follow| VecWrapperResolver::new(&grammar, &first_follow));

        Self {
            table,
            grammar,
            stacks: vec![initial_stack],
            next_stack_id: 1,
            pending_stacks: VecDeque::from([0]),
            error_recovery: None,
            recovery_state: None,
            vec_wrapper_resolver,
            input_length: 0,
            deleted_in_row: 0,
            inserted_in_row: 0,
            pending_synthetic_tokens: VecDeque::new(),
            #[cfg(feature = "glr_telemetry")]
            telemetry: TelemetryCounters::default(),
        }
    }

    /// Get the goto state for a nonterminal after a reduction
    #[inline]
    fn goto_next_state(&self, state: StateId, lhs: SymbolId) -> Option<StateId> {
        let row = self.table.goto_table.get(state.0 as usize)?;
        let col = match self.table.goto_indexing {
            adze_glr_core::GotoIndexing::NonterminalMap => {
                *self.table.nonterminal_to_index.get(&lhs)?
            }
            adze_glr_core::GotoIndexing::DirectSymbolId => lhs.0 as usize,
        };
        row.get(col).copied().filter(|&s| s.0 != 0)
    }

    /// Get the start symbol from the grammar
    /// This is the LHS of the first production (production_id 0), or the grammar's start symbol
    #[inline]
    pub fn start_symbol_id(&self) -> SymbolId {
        self.grammar
            .rules
            .values()
            .flat_map(|rules| rules.iter())
            .find(|r| r.production_id.0 == 0)
            .map(|r| r.lhs)
            .or_else(|| self.grammar.start_symbol())
            .unwrap_or(SymbolId(1)) // Neutral fallback, not EOF(0)
    }

    /// Enable error recovery with the given configuration
    pub fn enable_error_recovery(&mut self, config: ErrorRecoveryConfig) {
        self.recovery_state = Some(ErrorRecoveryState::new(config.clone()));
        self.error_recovery = Some(config);
    }

    /// Builder method to enable error recovery with the given configuration
    pub fn with_error_recovery(mut self, config: ErrorRecoveryConfig) -> Self {
        self.enable_error_recovery(config);
        self
    }

    /// Process a synthetic token (from recovery)
    fn process_synthetic_token(&mut self, token: SymbolId) {
        // Process synthetic token exactly like a real one but with zero width
        let stacks = std::mem::take(&mut self.stacks);
        let stacks = self.reduce_until_saturated(stacks, token, self.input_length);
        self.stacks = stacks;
        self.shift_synthetic_token(token);
    }

    /// Process one token through all active stacks
    ///
    /// This is the main entry point for processing tokens. It implements the two-phase
    /// approach described in the module documentation:
    ///
    /// 1. First, it performs all possible reductions on all active stacks using
    ///    `reduce_until_saturated()`. This ensures all cascading reductions complete
    ///    before any shifts occur.
    ///
    /// 2. Then, it processes the token by examining shift and fork actions on the
    ///    reduced stacks.
    ///
    /// # Arguments
    /// * `token` - The symbol ID of the token to process
    /// * `text` - The text content of the token
    /// * `byte_offset` - The byte position of the token in the input
    pub fn process_token(&mut self, token: SymbolId, text: &str, byte_offset: usize) {
        // Processing token

        // First process any pending synthetic tokens from recovery
        while let Some(synthetic) = self.pending_synthetic_tokens.pop_front() {
            self.process_synthetic_token(synthetic);
        }

        // Phase 1: Perform all possible reductions until saturation
        let mut stacks_to_process = std::mem::take(&mut self.stacks);
        self.pending_stacks.clear();

        stacks_to_process =
            self.reduce_until_saturated(stacks_to_process, token, byte_offset + text.len());

        // In true GLR, we may have both shift and reduce actions in the same cell
        // This is expected behavior for handling ambiguous grammars

        // Check if any stack has an action for the current token
        // If not, try recovery before Phase 2
        if self.error_recovery.is_some()
            && !stacks_to_process.is_empty()
            && !self.any_stack_has_action_in(&stacks_to_process, token)
        {
            // Restore stacks before attempting recovery
            self.stacks = stacks_to_process;
            if let Some(evt) = self.try_recover(token, false) {
                // Recovery performed: it modified stacks and/or input
                debug_glr!("Recovery performed: {:?}", evt);
                // For deletion, we should just skip this token and continue
                if matches!(evt, RecoveryEvent::Delete(_)) {
                    // Mark all stacks as having encountered an error
                    for stack in &mut self.stacks {
                        stack.version.enter_error();
                    }
                    // Token was deleted, caller should advance input
                    return;
                }
                // For insertion or pop, continue processing
                return;
            } else if self.stacks.is_empty() {
                // No stacks left, parsing failed
                debug_glr!("Recovery failed: no stacks remaining");
                return;
            } else {
                // Recovery couldn't help but we still have stacks
                // Create error node and continue
                debug_glr!("Recovery failed but stacks remain, creating error node");
                // Keep stacks but mark as error
                for stack in &mut self.stacks {
                    stack.version.enter_error();
                }
                return;
            }
        }

        // Phase 2: Process shifts and other actions on all post-reduction stacks
        let mut new_stacks = Vec::new();
        let mut accepted_any = false;
        let mut accept_stacks = Vec::new();

        for stack in stacks_to_process {
            let state = stack.current_state();

            // Debug: Print current state and token being processed
            debug_glr!(
                "DEBUG: Processing token {} (symbol_idx: {:?}) in state {}",
                token.0,
                self.table.symbol_to_index.get(&token),
                state.0
            );

            if let Some(symbol_idx) = self.table.symbol_to_index.get(&token) {
                let action_cell = self.table.action_table[state.0 as usize][*symbol_idx].clone();

                // Debug: Print action cell contents
                if action_cell.len() > 1 {
                    debug_glr!(
                        "DEBUG: Found multi-action cell at state {} for token {}: {} actions",
                        state.0,
                        token.0,
                        action_cell.len()
                    );
                    #[cfg(feature = "debug_glr")]
                    for (i, act) in action_cell.iter().enumerate() {
                        debug_glr!("  Action {}: {:?}", i, act);
                    }
                }

                // Process ALL actions in the cell without collapsing
                // This ensures true GLR behavior by exploring all alternatives
                let mut processed_any = false;

                for action in &action_cell {
                    match action {
                        Action::Shift(new_state) => {
                            let mut new_stack = stack.clone();
                            new_stack.push(
                                *new_state,
                                Arc::new(Subtree::new(
                                    SubtreeNode {
                                        symbol_id: token,
                                        is_error: false,
                                        byte_range: byte_offset..byte_offset + text.len(),
                                    },
                                    vec![],
                                )),
                            );
                            new_stacks.push(new_stack);
                            processed_any = true;
                        }

                        Action::Accept => {
                            // Collect accepting stacks for aggregation
                            accepted_any = true;
                            accept_stacks.push(stack.clone());
                            processed_any = true;
                        }

                        Action::Reduce(rule_id) => {
                            // Apply the reduction directly
                            let mut reduced_stack = stack.clone();
                            self.perform_reduction_on_stack(
                                &mut reduced_stack,
                                *rule_id,
                                byte_offset + text.len(),
                            );

                            // Re-saturate with the SAME lookahead to reach fixed point
                            // This ensures cascaded reduces and accepts are discovered
                            let closed = self.reduce_until_saturated(
                                vec![reduced_stack],
                                token,
                                byte_offset + text.len(),
                            );
                            new_stacks.extend(closed);
                            processed_any = true;
                        }

                        Action::Fork(actions) => {
                            // TRUE GLR FORKING! Always fork for ambiguity preservation
                            // Note: we ignore the conflict resolver to maintain all parse alternatives
                            // This is the critical part where we maintain ambiguity by forking stacks
                            debug_glr!(
                                "DEBUG: GLR Fork! Creating {} stacks for state {} with token {}",
                                actions.len(),
                                state.0,
                                token.0
                            );

                            // Fork the stack for EACH action to explore all parse paths
                            #[allow(unused_variables)]
                            for (i, fork_action) in actions.iter().enumerate() {
                                match fork_action {
                                    Action::Shift(new_state) => {
                                        let mut forked = stack.fork(self.next_stack_id);
                                        self.next_stack_id += 1;
                                        self.record_runtime_fork();

                                        forked.push(
                                            *new_state,
                                            Arc::new(Subtree::new(
                                                SubtreeNode {
                                                    symbol_id: token,
                                                    is_error: false,
                                                    byte_range: byte_offset
                                                        ..byte_offset + text.len(),
                                                },
                                                vec![],
                                            )),
                                        );
                                        debug_glr!("  Fork {}: Shift to state {}", i, new_state.0);
                                        new_stacks.push(forked);
                                    }

                                    Action::Reduce(rule_id) => {
                                        // Reductions should have been handled in phase 1, but if not, handle them
                                        let mut forked = stack.fork(self.next_stack_id);
                                        self.next_stack_id += 1;
                                        self.record_runtime_fork();
                                        self.perform_reduction_on_stack(
                                            &mut forked,
                                            *rule_id,
                                            byte_offset + text.len(),
                                        );
                                        debug_glr!("  Fork {}: Reduce by rule {}", i, rule_id.0);
                                        new_stacks.push(forked);
                                    }

                                    Action::Fork(nested_actions) => {
                                        // Handle nested Fork recursively
                                        debug_glr!(
                                            "  Fork {}: Nested fork with {} actions",
                                            i,
                                            nested_actions.len()
                                        );
                                        for nested_action in nested_actions {
                                            let mut nested_fork = stack.fork(self.next_stack_id);
                                            self.next_stack_id += 1;
                                            self.record_runtime_fork();

                                            match nested_action {
                                                Action::Shift(new_state) => {
                                                    nested_fork.push(
                                                        *new_state,
                                                        Arc::new(Subtree::new(
                                                            SubtreeNode {
                                                                symbol_id: token,
                                                                is_error: false,
                                                                byte_range: byte_offset
                                                                    ..byte_offset + text.len(),
                                                            },
                                                            vec![],
                                                        )),
                                                    );
                                                    new_stacks.push(nested_fork);
                                                }
                                                Action::Reduce(rule_id) => {
                                                    self.perform_reduction_on_stack(
                                                        &mut nested_fork,
                                                        *rule_id,
                                                        byte_offset + text.len(),
                                                    );
                                                    new_stacks.push(nested_fork);
                                                }
                                                _ => {
                                                    new_stacks.push(nested_fork);
                                                }
                                            }
                                        }
                                    }

                                    _ => {
                                        debug_glr!("  Fork {}: Other action", i);
                                    }
                                }
                            }

                            processed_any = true;
                        }

                        Action::Recover => {
                            // Handle Recover action - similar to Error but with specific recovery
                            // For now, treat it as an error
                            let mut error_stack = stack.clone();
                            error_stack.version.enter_error();
                            new_stacks.push(error_stack);
                            processed_any = true;
                        }

                        Action::Error => {
                            // println!("    Action: Error");

                            // Try error recovery if enabled
                            if let Some(recovery_state) = &mut self.recovery_state
                                && let Some(recovery_action) = recovery_state.suggest_recovery(
                                    state,
                                    token,
                                    &self.table,
                                    &self.grammar,
                                )
                            {
                                match recovery_action {
                                    RecoveryAction::InsertToken(missing_token) => {
                                        // Insert the missing token and continue processing
                                        if let Some(&missing_idx) =
                                            self.table.symbol_to_index.get(&missing_token)
                                        {
                                            let missing_action_cell = &self.table.action_table
                                                [state.0 as usize][missing_idx];
                                            // Find shift action in cell
                                            let shift_action = missing_action_cell
                                                .iter()
                                                .find(|a| matches!(a, Action::Shift(_)));
                                            if let Some(Action::Shift(new_state)) = shift_action {
                                                let mut recovery_stack = stack.clone();
                                                // Create error node for inserted token
                                                let error_node = Arc::new(Subtree::new(
                                                    SubtreeNode {
                                                        symbol_id: missing_token,
                                                        is_error: true,
                                                        byte_range: byte_offset..byte_offset, // Zero width for insertion
                                                    },
                                                    vec![], // Empty children for inserted token
                                                ));
                                                recovery_stack.push(*new_state, error_node);
                                                recovery_stack.version.enter_error();

                                                // Now process the current token against the new state
                                                let new_state_id = *new_state;
                                                if let Some(&token_idx) =
                                                    self.table.symbol_to_index.get(&token)
                                                {
                                                    let new_action_cell = &self.table.action_table
                                                        [new_state_id.0 as usize][token_idx];

                                                    // Try processing each action in the cell
                                                    for action in new_action_cell {
                                                        match action {
                                                            Action::Shift(shift_state) => {
                                                                let mut updated_stack =
                                                                    recovery_stack.clone();
                                                                let node = Arc::new(Subtree::new(
                                                                    SubtreeNode {
                                                                        symbol_id: token,
                                                                        is_error: false,
                                                                        byte_range: byte_offset
                                                                            ..byte_offset
                                                                                + text.len(),
                                                                    },
                                                                    vec![],
                                                                ));
                                                                updated_stack
                                                                    .push(*shift_state, node);
                                                                new_stacks.push(updated_stack);
                                                            }
                                                            _ => {
                                                                // Handle other actions (reduce, etc.) - add stack for processing
                                                                new_stacks
                                                                    .push(recovery_stack.clone());
                                                            }
                                                        }
                                                    }
                                                } else {
                                                    // Token not found in table, keep recovery stack
                                                    new_stacks.push(recovery_stack);
                                                }
                                                continue;
                                            }
                                        }
                                    }
                                    RecoveryAction::DeleteToken => {
                                        // Delete this token - add stack without processing token
                                        let mut recovery_stack = stack.clone();
                                        recovery_stack.version.enter_error();
                                        // Mark stack as having handled this token by deletion
                                        recovery_stack.version.dynamic_prec -= 1; // Penalize for token deletion

                                        // Create an error node for the deleted token to maintain parse tree structure
                                        let error_node = Arc::new(Subtree::new(
                                            SubtreeNode {
                                                symbol_id: token,
                                                is_error: true,
                                                byte_range: byte_offset..byte_offset + text.len(),
                                            },
                                            vec![], // No children for deleted token
                                        ));
                                        recovery_stack.nodes.push(error_node);
                                        new_stacks.push(recovery_stack);
                                        continue;
                                    }
                                    RecoveryAction::CreateErrorNode(_) => {
                                        // Create an error node containing the unexpected token
                                        let error_node = Arc::new(Subtree {
                                            node: SubtreeNode {
                                                symbol_id: token,
                                                is_error: true,
                                                byte_range: byte_offset..byte_offset + text.len(),
                                            },
                                            dynamic_prec: 0,
                                            children: vec![],
                                            alternatives: smallvec::SmallVec::new(),
                                        });
                                        let mut error_stack = stack.clone();
                                        // Just add the error node without changing state
                                        error_stack.nodes.push(error_node);
                                        error_stack.version.enter_error();
                                        new_stacks.push(error_stack);
                                        continue;
                                    }
                                    _ => {} // Other recovery actions not implemented yet
                                }
                            }

                            // Default error handling - mark stack as errored
                            let mut error_stack = stack.clone();
                            error_stack.version.enter_error();
                            new_stacks.push(error_stack);
                            processed_any = true;
                        }

                        _ => {
                            // Unknown action type // Expected: V for Recover
                            let mut error_stack = stack.clone();
                            error_stack.version.enter_error();
                            new_stacks.push(error_stack);
                            processed_any = true;
                        }
                    }
                }

                // If no actions were processed, keep the original stack
                if !processed_any {
                    new_stacks.push(stack);
                }
            } else {
                // No symbol in index - keep the stack
                new_stacks.push(stack);
            }
        }

        // Skip merging to preserve all forks for true GLR behavior
        // This allows maintaining multiple parse paths even if they reach the same state
        // self.merge_stacks(&mut new_stacks);

        // If we have accepting stacks, use only those
        // This aggregates all accepts for the current token
        if accepted_any {
            self.stacks = accept_stacks;
            // Don't process further for this token - we've accepted
            self.pending_stacks.clear();
            return;
        }

        // Safe deduplication: remove exact duplicates (same state and same top node pointer)
        // This keeps ambiguities intact while removing inflated stack counts
        // NOTE: Only dedup if we have many stacks to avoid collapsing necessary ambiguity forks
        if new_stacks.len() > safe_dedup_threshold() {
            use std::ptr;
            new_stacks.dedup_by(|a, b| {
                a.current_state() == b.current_state()
                    && match (a.nodes.last(), b.nodes.last()) {
                        (Some(a_last), Some(b_last)) => ptr::eq(a_last.as_ref(), b_last.as_ref()),
                        _ => false,
                    }
            });
        }

        // Compress stacks with identical tops to prevent explosion
        new_stacks = self.compress_identical_tops(new_stacks);

        // EOF finalization: prefer Accept or start symbol stacks
        if token == self.table.eof_symbol && !new_stacks.is_empty() {
            // EOF processing - prefer stacks that have Accept action or start symbol
            if let Some(&eof_idx) = self.table.symbol_to_index.get(&token) {
                // First, prefer stacks with Accept action
                let (accepted, rest): (Vec<_>, Vec<_>) = new_stacks.into_iter().partition(|st| {
                    let state_idx = st.current_state().0 as usize;
                    self.table
                        .action_table
                        .get(state_idx)
                        .and_then(|row| row.get(eof_idx))
                        .is_some_and(|actions| actions.iter().any(|a| matches!(a, Action::Accept)))
                });

                new_stacks = if !accepted.is_empty() {
                    accepted
                } else {
                    // Otherwise, prefer stacks whose top symbol is the start symbol
                    let start_symbol = self.start_symbol_id();

                    let (start_tops, others): (Vec<_>, Vec<_>) = rest.into_iter().partition(|st| {
                        st.nodes
                            .last()
                            .is_some_and(|n| n.node.symbol_id == start_symbol)
                    });

                    if !start_tops.is_empty() {
                        start_tops
                    } else {
                        others
                    }
                };
            }
        }

        // Update active stacks
        self.stacks = new_stacks;
        self.pending_stacks = (0..self.stacks.len()).collect();
    }

    /// Perform all possible reductions on the given stacks until no more reductions apply
    ///
    /// This method implements the reduction saturation phase of GLR parsing using a
    /// fixed-point worklist algorithm. It repeatedly applies all possible reductions
    /// until no new stack states are reachable for the given lookahead.
    ///
    /// This is essential for correctness because:
    ///
    /// 1. **Cascading Reductions**: One reduction may enable another. After a reduction
    ///    and GOTO, the new state may have additional reductions for the same lookahead.
    ///
    /// 2. **Completeness**: We must reach a fixed point where no new reductions are
    ///    possible before shifting, to ensure we don't miss valid parses.
    ///
    /// 3. **Fork Handling**: When a fork action contains both reduce and shift actions,
    ///    we process all reductions in this phase and defer shifts to phase 2.
    ///
    /// # Arguments
    /// * `stacks` - The parse stacks to process
    /// * `token` - The lookahead token (used to determine which reductions apply)
    ///
    /// # Returns
    /// A vector of stacks with all reductions applied to fixed point
    fn reduce_until_saturated(
        &mut self,
        stacks: Vec<ParseStack>,
        token: SymbolId,
        lookahead_end: usize,
    ) -> Vec<ParseStack> {
        use std::collections::{HashSet, VecDeque};

        // Track which reductions we've already applied at a given position
        // to avoid infinite epsilon loops
        #[derive(Clone, Copy, Debug, Hash, Eq, PartialEq)]
        struct RedStamp {
            state: StateId,
            rule: RuleId,
            start: usize, // start byte position for precise stamping
            end: usize,   // end byte position to prevent epsilon re-fires at same position
        }
        let mut seen_reductions = HashSet::<RedStamp>::new();

        // Track which (stack_id, state_id) tops we've already expanded for this lookahead.
        // This bounds the worklist and prevents infinite exploration.
        let mut seen_tops = HashSet::<(u16, usize)>::new(); // (state, top_ptr)

        // Worklist of stacks to try reduces from
        let mut worklist = VecDeque::new();

        // Result stacks that are fully saturated (no more reductions possible)
        let mut saturated_stacks = Vec::new();

        // Stacks that have shift actions (need to be preserved for phase 2)
        let mut shift_stacks = Vec::new();

        // Initialize worklist with input stacks
        for stack in stacks {
            let state = stack.current_state();
            // For empty stacks, use stack ID as discriminator to avoid collapsing all empty stacks
            let top_ptr = stack
                .nodes
                .last()
                .map(|n| Arc::as_ptr(n) as usize)
                .unwrap_or(stack.id);
            if seen_tops.insert((state.0, top_ptr)) {
                worklist.push_back(stack);
            }
        }

        // Get the column index for the lookahead token
        // For epsilon reductions, we still need to process even if token is not in table
        let symbol_idx = self.table.symbol_to_index.get(&token).copied();

        // Fixed-point iteration: process stacks until no new tops appear
        // Add iteration cap to prevent pathological grammars from hanging
        let mut steps = 0usize;
        const MAX_STEPS: usize = 64; // very conservative; never reached in sane grammars
        while let Some(stack) = worklist.pop_front() {
            if steps >= MAX_STEPS {
                debug_glr!(
                    "  Warning: Reached max epsilon closure steps ({})",
                    MAX_STEPS
                );
                break;
            }
            steps += 1;
            let state = stack.current_state();

            // Get actions for this state and lookahead
            // If symbol_idx is None (e.g. EOF not in table), still check for epsilon reductions
            let action_cell = if let Some(idx) = symbol_idx {
                self.table.action_table[state.0 as usize][idx].clone()
            } else {
                // No specific lookahead - check for epsilon reductions across all columns
                // This handles EOF and other unmapped symbols
                let mut all_reduces = Vec::new();
                for actions in &self.table.action_table[state.0 as usize] {
                    for action in actions {
                        if let Action::Reduce(rid) = action {
                            // Check if this is an epsilon reduction
                            let rhs_len = self.table.rules[rid.0 as usize].rhs_len as usize;
                            if rhs_len == 0 && !all_reduces.contains(action) {
                                all_reduces.push(action.clone());
                            }
                        }
                    }
                }
                all_reduces
            };

            debug_glr!(
                "DEBUG reduce_closure: Processing state {} for token {} ({} actions)",
                state.0,
                token.0,
                action_cell.len()
            );

            // Extract reduce actions from the specific column
            let mut reduces: Vec<(Action, i32)> = action_cell
                .iter()
                .filter_map(|a| match a {
                    Action::Reduce(_rid) => Some((a.clone(), self.action_priority(a))),
                    _ => None,
                })
                .collect();

            // At EOF, if column lacks epsilon reductions, also pull them from entire row
            // This ensures cascading epsilon reductions complete to the start symbol
            let is_eof = token == self.table.eof_symbol;
            if is_eof {
                let has_eps_in_col = reduces.iter().any(|(a, _)| {
                    matches!(a, Action::Reduce(rid) if self.table.rules[rid.0 as usize].rhs_len == 0)
                });

                if !has_eps_in_col {
                    // Include row-wide epsilon reductions, dedup by rule id
                    let mut _added_count = 0;
                    for actions in &self.table.action_table[state.0 as usize] {
                        for a in actions {
                            if let Action::Reduce(rid) = a
                                && self.table.rules[rid.0 as usize].rhs_len == 0
                                && !reduces
                                    .iter()
                                    .any(|(b, _)| matches!(b, Action::Reduce(r2) if r2.0 == rid.0))
                            {
                                reduces.push((a.clone(), self.action_priority(a)));
                                _added_count += 1;
                            }
                        }
                    }
                }
            }

            // Sort by priority (highest first)
            reduces.sort_by_key(|(_, prio)| -prio);

            // Check if this stack also has shift actions (needs to be preserved)
            let has_shift = action_cell.iter().any(|a| matches!(a, Action::Shift(_)));
            if has_shift {
                debug_glr!("  Stack has shift action - preserving for phase 2");
                shift_stacks.push(stack.clone());
            }

            // Check for other non-reduce actions
            let has_accept = action_cell.iter().any(|a| matches!(a, Action::Accept));
            if has_accept {
                debug_glr!("  Stack has accept action - preserving");
                saturated_stacks.push(stack.clone());
            }

            if reduces.is_empty() {
                // No reduces available - this stack is saturated
                if !has_shift && !has_accept {
                    saturated_stacks.push(stack);
                }
                continue;
            }

            // Apply each reduce action
            let mut any_reduction_applied = false;
            for (reduce_action, _) in reduces {
                let rule_id = match reduce_action {
                    Action::Reduce(rid) => rid,
                    _ => continue,
                };

                // Guard against repeated application at same position (epsilon loop prevention)
                // Use the parse table to check if this is an epsilon reduction
                let rhs_len = self.table.rules[rule_id.0 as usize].rhs_len as usize;

                // Only stamp epsilon reductions to prevent loops
                if rhs_len == 0 {
                    let (start_byte, end_byte) = if let Some(n) = stack.nodes.last() {
                        (n.node.byte_range.start, n.node.byte_range.end)
                    } else {
                        (lookahead_end, lookahead_end)
                    };
                    let stamp = RedStamp {
                        state: stack.current_state(),
                        rule: rule_id,
                        start: start_byte,
                        end: end_byte,
                    };

                    if !seen_reductions.insert(stamp) {
                        debug_glr!(
                            "  Skipping epsilon re-fire: state {} rule {} at {}..{}",
                            stamp.state.0,
                            rule_id.0,
                            stamp.start,
                            stamp.end
                        );
                        continue;
                    }
                }
                // No stamping for non-epsilon reductions

                debug_glr!("  Applying reduction: rule {}", rule_id.0);

                // Fork the stack for this reduction
                let mut reduced_stack = stack.fork(self.next_stack_id);
                self.next_stack_id += 1;
                self.record_runtime_fork();

                // Apply the reduction (this will pop symbols and push via GOTO)
                self.perform_reduction_on_stack(&mut reduced_stack, rule_id, lookahead_end);

                // The new top state after GOTO might have more reduces for the same lookahead
                let new_state = reduced_stack.current_state();

                // Use pointer-based key to match closure-local dedup
                // For empty stacks, use stack ID to avoid collapsing them
                let top_ptr = reduced_stack
                    .nodes
                    .last()
                    .map(|n| Arc::as_ptr(n) as usize)
                    .unwrap_or(reduced_stack.id);
                let key = (new_state.0, top_ptr);

                if seen_tops.insert(key) {
                    // This is a new top we haven't explored - add to worklist for cascading
                    debug_glr!(
                        "  New top reached: state {} - adding to worklist",
                        new_state.0
                    );
                    worklist.push_back(reduced_stack);
                    any_reduction_applied = true;
                } else {
                    // We've already processed this top - it's saturated
                    debug_glr!("  Top already seen: state {} - saturated", new_state.0);
                    saturated_stacks.push(reduced_stack);
                }
            }

            // If no reductions were applied from this stack and it has no shift/accept,
            // then this stack is fully saturated
            if !any_reduction_applied && !has_shift && !has_accept {
                saturated_stacks.push(stack);
            }
        }

        // Combine saturated stacks and shift stacks
        let mut result = saturated_stacks;
        result.extend(shift_stacks);

        // Skip merging to preserve all reduction paths for true GLR behavior
        // self.merge_stacks(&mut result);

        // Closure-local deduplication: drop exact duplicates by (state, top-node pointer)
        // This prevents epsilon chains from blowing up
        use std::ptr;
        // Don't remove stacks with no nodes - they may be valid initial states
        result.sort_by_key(|s| {
            (
                s.current_state().0,
                s.nodes.last().map(|n| Arc::as_ptr(n) as usize).unwrap_or(0),
            )
        });
        result.dedup_by(|a, b| {
            // Only dedup if both stacks have the same state AND the same top node
            a.current_state() == b.current_state()
                && match (a.nodes.last(), b.nodes.last()) {
                    (Some(node_a), Some(node_b)) => ptr::eq(node_a.as_ref(), node_b.as_ref()),
                    (None, None) => true, // Two stacks with no nodes and same state are duplicates
                    _ => false,           // Different node counts means different stacks
                }
        });

        debug_glr!(
            "DEBUG reduce_closure: Fixed point reached with {} stacks",
            result.len()
        );

        result
    }

    // ================================================================================
    // Stack compression to prevent explosion
    // ================================================================================

    /// Compress stacks with identical tops to prevent explosion
    /// This preserves all derivations by packing alternatives at the top
    fn compress_identical_tops(&mut self, mut stacks: Vec<ParseStack>) -> Vec<ParseStack> {
        use std::collections::HashMap;

        // If we have few stacks, no need to compress
        if stacks.len() <= 10 {
            return stacks;
        }

        #[derive(Hash, Eq, PartialEq)]
        struct TopKey {
            state: StateId,
            symbol: SymbolId,
            start: usize,
            end: usize,
        }

        // Map from top key to index in output vector
        let mut keep: HashMap<TopKey, usize> = HashMap::new();
        let mut out: Vec<ParseStack> = Vec::new();
        #[cfg(any(feature = "glr_telemetry", feature = "debug_glr"))]
        let mut packed_count = 0usize;

        #[cfg(any(feature = "glr_telemetry", feature = "debug_glr"))]
        let input_count = stacks.len();

        for mut stack in stacks.drain(..) {
            // Get the top node info, if any
            let key = if let Some(top) = stack.nodes.last() {
                TopKey {
                    state: stack.current_state(),
                    symbol: top.node.symbol_id,
                    start: top.node.byte_range.start,
                    end: top.node.byte_range.end,
                }
            } else {
                // Stack with no nodes - just check state
                TopKey {
                    state: stack.current_state(),
                    symbol: SymbolId(u16::MAX),
                    start: 0,
                    end: 0,
                }
            };

            if let Some(&idx) = keep.get(&key) {
                // We already have a stack with this top - merge ambiguity
                let kept = &mut out[idx];

                // Pop the tops from both stacks
                if let (Some(new_top), Some(kept_top)) = (stack.nodes.pop(), kept.nodes.pop()) {
                    // Merge the two tops, preserving all alternatives
                    let merged_subtree = Arc::try_unwrap(kept_top)
                        .unwrap_or_else(|arc| (*arc).clone())
                        .merge_ambiguous(new_top);

                    // Push the merged top back
                    kept.nodes.push(Arc::new(merged_subtree));
                    #[cfg(any(feature = "glr_telemetry", feature = "debug_glr"))]
                    {
                        packed_count += 1;
                    }

                    // Keep the highest dynamic precedence
                    if stack.version.dynamic_prec > kept.version.dynamic_prec {
                        kept.version.dynamic_prec = stack.version.dynamic_prec;
                    }
                }
                // Otherwise stack has no nodes, just drop it
            } else {
                // First time seeing this top
                keep.insert(key, out.len());
                out.push(stack);
            }
        }

        #[cfg(feature = "glr_telemetry")]
        {
            self.bump_telemetry(|t| {
                t.tops_before_compress += input_count;
                t.tops_after_compress += out.len();
                t.alts_packed += packed_count;
            });
        }

        debug_glr!(
            "Compressed {} stacks down to {} unique tops ({} packed)",
            input_count,
            out.len(),
            packed_count
        );

        out
    }

    /// Perform a reduction on a specific stack
    fn perform_reduction_on_stack(
        &mut self,
        stack: &mut ParseStack,
        rule_id: RuleId,
        lookahead_end: usize,
    ) {
        debug_glr!(
            "DEBUG: Performing reduction with rule {} on stack in state {}",
            rule_id.0,
            stack.current_state().0
        );
        debug_glr!("  Stack has {} nodes before reduction", stack.nodes.len());

        // Perform reduction
        // Find the rule in the grammar
        if let Some(rule) = self
            .grammar
            .rules
            .values()
            .flat_map(|rules| rules.iter())
            .find(|r| r.production_id.0 == rule_id.0)
        {
            debug_glr!(
                "  Rule: {:?} -> {:?} ({} symbols)",
                rule.lhs,
                rule.rhs,
                rule.rhs.len()
            );
            let children = stack.pop(rule.rhs.len());
            debug_glr!(
                "  Popped {} children, stack now has {} nodes",
                children.len(),
                stack.nodes.len()
            );

            // Create new subtree for the reduction
            // For epsilon reductions (empty RHS), use lookahead_end as the position
            let byte_range = if let (Some(first), Some(last)) = (children.first(), children.last())
            {
                // Normal: span from first child start to last child end
                first.node.byte_range.start..last.node.byte_range.end
            } else {
                // Epsilon: zero-width span at the current lookahead end
                lookahead_end..lookahead_end
            };

            let node = SubtreeNode {
                symbol_id: rule.lhs,
                is_error: false,
                byte_range,
            };

            // Apply field mappings to children
            let children_with_fields = if rule.fields.is_empty() {
                // No fields, use FIELD_NONE for all children
                children
                    .into_iter()
                    .map(crate::subtree::ChildEdge::new_without_field)
                    .collect()
            } else {
                // Apply field mappings based on rule.fields
                let mut result = Vec::with_capacity(children.len());
                for (idx, child) in children.into_iter().enumerate() {
                    // Find field ID for this child position
                    let field_id = rule
                        .fields
                        .iter()
                        .find(|(_, pos)| *pos == idx)
                        .map(|(field_id, _)| field_id.0)
                        .unwrap_or(crate::subtree::FIELD_NONE);

                    result.push(crate::subtree::ChildEdge::new(child, field_id));
                }
                result
            };

            // Check if this rule has precedence (static or dynamic)
            let dynamic_prec = match &rule.precedence {
                Some(adze_ir::PrecedenceKind::Dynamic(prec)) => *prec as i32,
                Some(adze_ir::PrecedenceKind::Static(prec)) => *prec as i32, // Add support for static precedence
                None => 0,
            };

            let subtree = Arc::new(Subtree::with_dynamic_prec_and_fields(
                node,
                children_with_fields,
                dynamic_prec,
            ));

            // Look up goto state for the nonterminal after reduction
            let current_state = stack.current_state();
            if let Some(new_state) = self.goto_next_state(current_state, rule.lhs) {
                debug_glr!(
                    "  GOTO: state {} -> state {} for symbol {}",
                    current_state.0,
                    new_state.0,
                    rule.lhs.0
                );
                stack.push(new_state, subtree);
            } else {
                debug_glr!(
                    "  ERROR: No GOTO found for symbol {} from state {}",
                    rule.lhs.0,
                    current_state.0
                );
                // Can't continue with this reduction path
            }
        } else {
            debug_glr!("  ERROR: Rule {} not found in grammar", rule_id.0);
        }
    }

    /// Merge stacks that have reached the same state
    ///
    /// In GLR parsing, multiple parse stacks can reach the same parser state through
    /// different paths. When this happens, we can merge these stacks to avoid exponential
    /// growth in the number of stacks.
    ///
    /// The merging process:
    /// 1. Identifies stacks with identical state sequences
    /// 2. Compares their parse trees using dynamic precedence and other criteria
    /// 3. Keeps the best parse according to the comparison rules
    /// 4. Handles ambiguity by potentially keeping multiple parses if they're equally valid
    ///
    /// This is a key optimization that makes GLR parsing practical for real grammars.
    #[allow(dead_code)]
    fn merge_stacks(&mut self, stacks: &mut Vec<ParseStack>) {
        let mut merged = Vec::new();
        let mut processed = vec![false; stacks.len()];

        for i in 0..stacks.len() {
            if processed[i] {
                continue;
            }

            let mut best_stack = stacks[i].clone();
            processed[i] = true;

            // Find all stacks with the same state and node count
            for j in (i + 1)..stacks.len() {
                if processed[j] {
                    continue;
                }

                if stacks[j].states == best_stack.states {
                    // NEW: Check if parse trees are structurally equivalent
                    if stacks[j].has_equivalent_parse_tree(&best_stack) {
                        // Trees are identical - safe to merge using version comparison
                        match compare_versions(&best_stack.version, &stacks[j].version) {
                            CompareResult::TakeLeft => {
                                // Keep best_stack
                            }
                            CompareResult::TakeRight => {
                                best_stack = stacks[j].clone();
                            }
                            CompareResult::PreferLeft => {
                                // Keep the preferred one
                            }
                            CompareResult::PreferRight => {
                                best_stack = stacks[j].clone();
                            }
                            CompareResult::Tie => {
                                // Keep the first one
                            }
                        }
                        processed[j] = true;
                    }
                    // If parse trees differ, DON'T merge - keep both stacks!
                    // This preserves ambiguity in GLR parsing
                }
            }

            merged.push(best_stack);
        }

        // Add any unprocessed stacks (those with different parse trees)
        for (i, stack) in stacks.iter().enumerate() {
            if !processed[i] {
                merged.push(stack.clone());
            }
        }

        if merged.len() > 1 && merged.len() != stacks.len() {
            debug_glr!(
                "DEBUG merge_stacks: {} stacks -> {} stacks after conservative merge",
                stacks.len(),
                merged.len()
            );
        }

        *stacks = merged;
    }

    /// Get the best parse tree from active stacks
    pub fn get_best_parse(&self) -> Option<Arc<Subtree>> {
        debug_glr!("get_best_parse: {} stacks available", self.stacks.len());

        // Get best parse from available stacks
        if self.stacks.is_empty() {
            return None;
        }

        // Find the best stack according to version comparison
        let mut best_idx = 0;
        for i in 1..self.stacks.len() {
            match compare_versions(&self.stacks[best_idx].version, &self.stacks[i].version) {
                CompareResult::TakeRight | CompareResult::PreferRight => {
                    best_idx = i;
                }
                _ => {}
            }
        }

        debug_glr!(
            "Best stack has {} nodes, current state: {}",
            self.stacks[best_idx].nodes.len(),
            self.stacks[best_idx].current_state().0
        );

        // Return best parse
        self.stacks[best_idx].nodes.last().cloned()
    }

    /// Try to reach a state that can accept EOF by repeatedly applying
    /// insertion/pop recovery. Never attempt deletion at EOF.
    fn drive_recovery_until_eof_action(&mut self) {
        if self.error_recovery.is_none() || self.stacks.is_empty() {
            return;
        }
        let eof = self.table.eof_symbol;
        debug_glr!(
            "drive_recovery_until_eof_action: starting with {} stacks",
            self.stacks.len()
        );

        // Hard bound: at most a few iterations (guards infinite loops).
        #[allow(unused_variables)] // Used in debug_glr! macro calls
        for i in 0..8 {
            // Close first with EOF as lookahead to expose any reduces
            let stacks = std::mem::take(&mut self.stacks);
            let stacks = self.reduce_until_saturated(stacks, eof, self.input_length);
            self.stacks = stacks;

            // Now check if any stack can handle EOF
            if self.any_stack_has_action(eof) {
                debug_glr!(
                    "drive_recovery_until_eof_action: found action for EOF after {} iterations",
                    i
                );
                break;
            }

            debug_glr!(
                "drive_recovery_until_eof_action: iteration {} with {} stacks",
                i,
                self.stacks.len()
            );

            // Try recovery (insert/pop only, never delete at EOF)
            match self.try_recover(eof, /*eof=*/ true) {
                Some(RecoveryEvent::Insert(_tok)) => {
                    debug_glr!("drive_recovery_until_eof_action: inserted {:?}", _tok);
                    // Process pending synthetic tokens
                    while let Some(synthetic) = self.pending_synthetic_tokens.pop_front() {
                        self.process_synthetic_token(synthetic);
                    }
                    continue;
                }
                Some(RecoveryEvent::Pop(_n)) => {
                    debug_glr!("drive_recovery_until_eof_action: popped {} symbols", _n);
                    continue;
                }
                _ => {
                    debug_glr!("drive_recovery_until_eof_action: no recovery possible");
                    break;
                }
            }
        }
    }

    /// Process EOF to complete parsing
    pub fn process_eof(&mut self, total_bytes: usize) {
        // Store the total input length for validation in finish_all_alternatives
        self.input_length = total_bytes;

        // Give recovery a chance to make EOF shiftable/reduceable
        self.drive_recovery_until_eof_action();

        // Process EOF token using the table's EOF symbol
        let eof_symbol = self.table.eof_symbol;
        self.process_token(eof_symbol, "", total_bytes);
    }

    /// Get number of active stacks (for debugging)
    pub fn stack_count(&self) -> usize {
        self.stacks.len()
    }

    fn complete_stack_indices(&self) -> Vec<usize> {
        self.stacks
            .iter()
            .enumerate()
            .filter_map(|(index, stack)| {
                let node = stack.nodes.first()?;
                (stack.nodes.len() == 1 && node.node.byte_range.end == self.input_length)
                    .then_some(index)
            })
            .collect()
    }

    fn incomplete_stack_states(&self) -> Vec<(StateId, usize, Vec<SymbolId>)> {
        self.stacks
            .iter()
            .map(|stack| {
                let state = stack.states.last().copied().unwrap_or(StateId(0));
                (
                    state,
                    stack.nodes.len(),
                    stack
                        .nodes
                        .iter()
                        .map(|node| node.node.symbol_id)
                        .collect::<Vec<_>>(),
                )
            })
            .collect()
    }

    fn incomplete_parse_message(&self) -> String {
        format!(
            "Parse incomplete. Stack states: {:?}",
            self.incomplete_stack_states()
        )
    }

    fn select_best_complete_stack(
        &self,
        complete_stacks: &[usize],
    ) -> Option<SelectedCompleteStack> {
        let mut best_stack_idx = *complete_stacks.first()?;
        let mut best_tree_key = subtree_selection_key(&self.stacks[best_stack_idx].nodes[0]);
        let mut reason = if complete_stacks.len() == 1 {
            SelectionReason::SingleParse
        } else {
            SelectionReason::StableStructuralTieBreak
        };

        for &stack_idx in &complete_stacks[1..] {
            match compare_versions(
                &self.stacks[best_stack_idx].version,
                &self.stacks[stack_idx].version,
            ) {
                CompareResult::TakeRight | CompareResult::PreferRight => {
                    reason = version_selection_reason(
                        &self.stacks[best_stack_idx],
                        &self.stacks[stack_idx],
                    );
                    best_stack_idx = stack_idx;
                    best_tree_key = subtree_selection_key(&self.stacks[best_stack_idx].nodes[0]);
                }
                CompareResult::Tie => {
                    let tree_key = subtree_selection_key(&self.stacks[stack_idx].nodes[0]);
                    if tree_key < best_tree_key {
                        best_stack_idx = stack_idx;
                        best_tree_key = tree_key;
                        reason = SelectionReason::StableStructuralTieBreak;
                    }
                }
                _ => {}
            }
        }

        Some(SelectedCompleteStack {
            stack_index: best_stack_idx,
            reason,
        })
    }

    /// Return an ambiguity summary for retained complete alternatives.
    ///
    /// This is a summary view, not a raw forest API. It is available after EOF
    /// processing. `Ok(None)` means the runtime retained only one complete
    /// parse; `Ok(Some(_))` means multiple complete alternatives were retained
    /// and one was selected by the same policy used by [`Self::finish`].
    pub fn finish_ambiguity_summary(&self) -> Result<Option<AmbiguitySummary>, String> {
        let complete_stacks = self.complete_stack_indices();
        if complete_stacks.is_empty() {
            return Err(self.incomplete_parse_message());
        }
        if complete_stacks.len() == 1 {
            return Ok(None);
        }

        let selected = self.select_best_complete_stack(&complete_stacks);
        let selected_stack_index = selected.map(|selection| selection.stack_index);
        let selected_index = selected_stack_index.and_then(|stack_index| {
            complete_stacks
                .iter()
                .position(|candidate| *candidate == stack_index)
        });

        let mut span_start = usize::MAX;
        let mut span_end = 0usize;
        let alternatives = complete_stacks
            .iter()
            .enumerate()
            .map(|(index, stack_index)| {
                let stack = &self.stacks[*stack_index];
                let node = &stack.nodes[0];
                span_start = span_start.min(node.node.byte_range.start);
                span_end = span_end.max(node.node.byte_range.end);
                AlternativeSummary {
                    index,
                    root_symbol: node.node.symbol_id,
                    span: node.node.byte_range.clone(),
                    dynamic_precedence: stack.version.dynamic_prec,
                    in_error: stack.version.in_error,
                    cost: stack.version.cost,
                    node_count: subtree_node_count(node),
                }
            })
            .collect();

        Ok(Some(AmbiguitySummary {
            span: span_start..span_end,
            alternatives,
            selected: selected_index,
            selection_reason: selected
                .map(|selection| selection.reason)
                .unwrap_or(SelectionReason::StableStructuralTieBreak),
        }))
    }

    /// Finish parsing and get the result
    ///
    /// This method is called after all tokens have been processed (including EOF) to
    /// extract the final parse tree. It examines all remaining stacks and returns the
    /// parse tree from a successfully completed parse.
    ///
    /// A successful parse is identified by:
    /// 1. Having exactly one node on the stack (the root of the parse tree)
    /// 2. That node representing a non-terminal symbol (not a raw token)
    ///
    /// # Returns
    /// * `Ok(Arc<Subtree>)` - The root of the parse tree if parsing succeeded
    /// * `Err(String)` - An error message with debugging information if parsing failed
    ///
    /// # Note
    /// In case of ambiguous parses where multiple stacks complete successfully, this
    /// returns one selected tree. Selection first compares GLR stack versions using
    /// dynamic-precedence and recovery state, then applies a stable structural
    /// tie-break. Use [`Self::finish_all_alternatives`] to inspect all complete
    /// parse alternatives retained by the runtime.
    pub fn finish(&self) -> Result<Arc<Subtree>, String> {
        // Find a successfully parsed stack
        // Success criteria:
        // 1. Has exactly one node (the root of the parse tree)
        // 2. That node represents the start symbol (we'll accept any non-terminal for now)

        // Debug: Print all available stacks before choosing
        debug_glr!("finish: have {} stacks to consider", self.stacks.len());
        #[allow(unused_variables)]
        for (i, stack) in self.stacks.iter().enumerate() {
            debug_glr!("Debug stack index: {}", i);
            debug_glr!(
                "Stack {}: {} nodes, state {}",
                i,
                stack.nodes.len(),
                stack.current_state().0
            );
            if stack.nodes.len() == 1 {
                #[allow(unused_variables)]
                let node = &stack.nodes[0];
                debug_glr!("Debug stack node symbol: {:?}", node.node.symbol_id);
                debug_glr!(
                    "  Node: symbol={:?}, range={:?}",
                    node.node.symbol_id,
                    node.node.byte_range
                );
            }
        }

        // Collect all complete stacks (same logic as finish_all_alternatives)
        let mut complete_stacks = Vec::new();
        #[allow(unused_variables)]
        for (i, stack) in self.stacks.iter().enumerate() {
            debug_glr!("Debug stack index: {}", i);
            debug_glr!("finish: stack has {} nodes", stack.nodes.len());
            if stack.nodes.len() == 1 {
                #[allow(unused_variables)]
                let node = &stack.nodes[0];
                debug_glr!("Debug stack node symbol: {:?}", node.node.symbol_id);

                // CRITICAL: Check that the parse consumed all input
                // This fix ensures we don't return incomplete parses like "1+2" when input is "1+2*3"
                if node.node.byte_range.end == self.input_length {
                    debug_glr!("finish: Stack {} is complete (spans full input)", i);
                    complete_stacks.push(i);
                } else {
                    debug_glr!(
                        "finish: Rejecting incomplete stack {} - ends at byte {} but input is {} bytes",
                        i,
                        node.node.byte_range.end,
                        self.input_length
                    );
                }
            }
        }

        if complete_stacks.is_empty() {
            // No complete stacks, check for error recovery cases
            for stack in &self.stacks {
                if !stack.nodes.is_empty() {
                    // If we have multiple nodes but error recovery was used,
                    // return a partial tree wrapped in an error node
                    let has_error =
                        stack.nodes.iter().any(|n| n.node.is_error) || stack.version.in_error;
                    if has_error && self.error_recovery.is_some() {
                        // Create an error node containing all remaining nodes
                        let error_node = Arc::new(Subtree::new(
                            SubtreeNode {
                                symbol_id: SymbolId(u16::MAX), // Special error symbol
                                is_error: true,
                                byte_range: 0..self.input_length,
                            },
                            stack.nodes.clone(),
                        ));
                        return Ok(error_node);
                    }
                }
            }
        } else {
            // We have complete stacks! Choose the best one using version comparison
            debug_glr!(
                "finish: Found {} complete stacks, choosing best",
                complete_stacks.len()
            );

            let mut best_stack_idx = complete_stacks[0];
            let mut best_tree_key = subtree_selection_key(&self.stacks[best_stack_idx].nodes[0]);
            for &stack_idx in &complete_stacks[1..] {
                match compare_versions(
                    &self.stacks[best_stack_idx].version,
                    &self.stacks[stack_idx].version,
                ) {
                    CompareResult::TakeRight | CompareResult::PreferRight => {
                        debug_glr!(
                            "finish: Stack {} is better than stack {}",
                            stack_idx,
                            best_stack_idx
                        );
                        best_stack_idx = stack_idx;
                        best_tree_key =
                            subtree_selection_key(&self.stacks[best_stack_idx].nodes[0]);
                    }
                    CompareResult::Tie => {
                        let tree_key = subtree_selection_key(&self.stacks[stack_idx].nodes[0]);
                        if tree_key < best_tree_key {
                            debug_glr!(
                                "finish: Stack {} wins stable structural tie-break over stack {}",
                                stack_idx,
                                best_stack_idx
                            );
                            best_stack_idx = stack_idx;
                            best_tree_key = tree_key;
                        }
                    }
                    _ => {
                        debug_glr!(
                            "finish: Stack {} is not better than stack {}",
                            stack_idx,
                            best_stack_idx
                        );
                    }
                }
            }

            let best_stack = &self.stacks[best_stack_idx];
            let node = &best_stack.nodes[0];

            debug_glr!(
                "finish: Choosing stack {} with dynamic_prec={}",
                best_stack_idx,
                best_stack.version.dynamic_prec
            );

            // Check if we encountered errors during parsing (e.g., deleted tokens)
            if best_stack.version.in_error && self.error_recovery.is_some() {
                // Wrap in error node to indicate parse had errors
                let error_node = Arc::new(Subtree::new(
                    SubtreeNode {
                        symbol_id: SymbolId(u16::MAX), // Special error symbol
                        is_error: true,
                        byte_range: node.node.byte_range.clone(),
                    },
                    vec![node.clone()],
                ));
                return Ok(error_node);
            }
            return Ok(node.clone());
        }

        // If no accepted stack, return error with debugging info
        let states: Vec<_> = self
            .stacks
            .iter()
            .map(|s| {
                let state = s.states.last().copied().unwrap_or(StateId(0));
                (
                    state,
                    s.nodes.len(),
                    s.nodes.iter().map(|n| n.node.symbol_id).collect::<Vec<_>>(),
                )
            })
            .collect();
        Err(format!("Parse incomplete. Stack states: {:?}", states))
    }

    /// Get all successful parse alternatives (for ambiguous grammars)
    pub fn finish_all_alternatives(&self) -> Result<Vec<Arc<Subtree>>, String> {
        debug_glr!(
            "DEBUG finish_all_alternatives: have {} stacks",
            self.stacks.len()
        );
        #[allow(unused_variables)]
        #[allow(unused_variables)]
        for (i, stack) in self.stacks.iter().enumerate() {
            debug_glr!("Debug stack index: {}", i);
            debug_glr!(
                "  Stack {}: {} nodes, state {}",
                i,
                stack.nodes.len(),
                stack.current_state().0
            );
            // Print parse tree structure for debugging
            if stack.nodes.len() == 1 {
                ParseStack::print_tree_structure(&stack.nodes[0], 0);
            }
        }

        let mut alternatives = Vec::new();

        // Collect all successfully parsed stacks
        for stack in &self.stacks {
            if stack.nodes.len() == 1 {
                // Accept if we have exactly one node after EOF processing
                // This should be the root of the parse tree (the start symbol)
                #[allow(unused_variables)]
                let node = &stack.nodes[0];
                debug_glr!("Debug stack node symbol: {:?}", node.node.symbol_id);

                // CRITICAL: Check that the parse consumed all input
                if node.node.byte_range.end == self.input_length {
                    alternatives.push(node.clone());
                } else {
                    debug_glr!(
                        "DEBUG: Rejecting incomplete stack - ends at byte {} but input is {} bytes",
                        node.node.byte_range.end,
                        self.input_length
                    );
                }
            }
        }

        if alternatives.is_empty() {
            // If no accepted stack, return error with debugging info
            let states: Vec<_> = self
                .stacks
                .iter()
                .map(|s| {
                    let state = s.states.last().copied().unwrap_or(StateId(0));
                    (
                        state,
                        s.nodes.len(),
                        s.nodes.iter().map(|n| n.node.symbol_id).collect::<Vec<_>>(),
                    )
                })
                .collect();
            Err(format!("Parse incomplete. Stack states: {:?}", states))
        } else {
            debug_glr!("DEBUG: Found {} parse alternatives", alternatives.len());
            Ok(alternatives)
        }
    }

    /// Reset parser state for reuse
    pub fn reset(&mut self) {
        self.stacks.clear();
        let initial_stack = ParseStack::new(StateId(0), self.next_stack_id);
        self.next_stack_id += 1;
        self.stacks.push(initial_stack);
        self.pending_stacks.clear();
        self.pending_stacks.push_back(0);

        // Reset error recovery state if present
        if let Some(ref mut recovery_state) = self.recovery_state {
            recovery_state.reset_consecutive_errors();
            recovery_state.clear_errors();
        }
    }

    /// Get expected symbols at current parse state
    pub fn expected_symbols(&self) -> Vec<SymbolId> {
        let mut symbols = Vec::new();

        for stack in &self.stacks {
            let state = stack.current_state();

            // Check all possible actions from this state
            for symbol in self.table.symbol_to_index.keys() {
                if let Some(_action) = self.get_action(state, *symbol)
                    && !symbols.contains(symbol)
                {
                    symbols.push(*symbol);
                }
            }
        }

        symbols
    }

    /// Return the grammar name for a symbol when one is available.
    pub fn symbol_name(&self, symbol: SymbolId) -> Option<&str> {
        self.grammar
            .tokens
            .get(&symbol)
            .map(|token| token.name.as_str())
            .or_else(|| self.grammar.rule_names.get(&symbol).map(String::as_str))
    }

    /// Get display names for symbols expected at the current parse state.
    pub fn expected_symbol_names(&self) -> Vec<String> {
        let mut symbols = self
            .expected_symbols()
            .into_iter()
            .map(|symbol| {
                self.symbol_name(symbol)
                    .map(str::to_string)
                    .unwrap_or_else(|| format!("{symbol:?}"))
            })
            .collect::<Vec<_>>();

        symbols.sort();
        symbols.dedup();
        symbols
    }

    /// Perform all possible reductions on a stack until no more are possible
    ///
    /// # Errors
    ///
    /// Returns [`GLRError::ComplexSymbolNotNormalized`] if any rule contains unnormalized complex symbols.
    #[allow(dead_code)]
    fn perform_all_reductions(&self, stack: ParseStack) -> GLRResult<Vec<ParseStack>> {
        let mut result_stacks = vec![];
        let mut work_list = vec![stack];

        while let Some(current_stack) = work_list.pop() {
            let _state = current_stack.current_state();
            let mut has_reduction = false;

            // Check all possible reductions in this state
            for (_symbol_id, rules) in &self.grammar.rules {
                for rule in rules {
                    // Check if we can reduce by this rule
                    match self.can_reduce(&current_stack, rule) {
                        Ok(true) => {
                            // After reduction, we need to find the goto state
                            // First get the state we'll be in after popping the RHS symbols
                            let base_state_idx = if current_stack.states.len() > rule.rhs.len() {
                                current_stack.states
                                    [current_stack.states.len() - rule.rhs.len() - 1]
                                    .0 as usize
                            } else {
                                0
                            };

                            // Get the nonterminal index for the LHS non-terminal
                            if let Some(&lhs_idx) = self.table.nonterminal_to_index.get(&rule.lhs) {
                                // Look up the goto state
                                if base_state_idx < self.table.goto_table.len()
                                    && lhs_idx < self.table.goto_table[base_state_idx].len()
                                {
                                    let goto_state = self.table.goto_table[base_state_idx][lhs_idx];
                                    if goto_state.0 != 0 {
                                        // Valid goto state
                                        has_reduction = true;

                                        // Perform the reduction
                                        let mut reduced_stack = current_stack.clone();
                                        let children: Vec<Arc<Subtree>> = (0..rule.rhs.len())
                                            .filter_map(|_| reduced_stack.nodes.pop())
                                            .collect::<Vec<_>>()
                                            .into_iter()
                                            .rev()
                                            .collect();

                                        // Also pop the corresponding states
                                        for _ in 0..rule.rhs.len() {
                                            reduced_stack.states.pop();
                                        }

                                        // Create new subtree for the reduction
                                        let byte_range = if let (Some(first), Some(last)) =
                                            (children.first(), children.last())
                                        {
                                            first.node.byte_range.start..last.node.byte_range.end
                                        } else {
                                            0..0 // Empty production
                                        };

                                        let node = SubtreeNode {
                                            symbol_id: rule.lhs,
                                            is_error: false,
                                            byte_range,
                                        };

                                        let dynamic_prec = rule
                                            .precedence
                                            .map(|p| match p {
                                                PrecedenceKind::Static(prec) => prec as i32,
                                                PrecedenceKind::Dynamic(idx) => {
                                                    // For dynamic precedence, use child's precedence
                                                    let idx_usize = idx as usize;
                                                    if idx_usize < children.len() {
                                                        children[idx_usize].dynamic_prec
                                                    } else {
                                                        0
                                                    }
                                                }
                                            })
                                            .unwrap_or(0);

                                        let parent = Arc::new(Subtree::with_dynamic_prec(
                                            node,
                                            children,
                                            dynamic_prec,
                                        ));

                                        // Push the new subtree
                                        reduced_stack.push(goto_state, parent);

                                        // Continue reducing from this new state
                                        work_list.push(reduced_stack);
                                    }
                                }
                            }
                        }
                        Ok(false) => {
                            // Cannot reduce with this rule, continue to next rule
                        }
                        Err(err) => {
                            // Return error immediately if normalization check fails
                            return Err(err);
                        }
                    }
                }
            }

            // If no reductions were possible, this stack is done
            if !has_reduction {
                result_stacks.push(current_stack);
            }
        }

        Ok(result_stacks)
    }

    /// Check if we can reduce by a rule
    ///
    /// This function validates that all symbols in the rule are normalized (Terminal, NonTerminal, or External)
    /// and checks if the stack contents match the rule's right-hand side.
    ///
    /// # Errors
    ///
    /// Returns [`GLRError::ComplexSymbolNotNormalized`] if any symbol in the rule's RHS is not normalized.
    /// Complex symbols (Optional, Repeat, RepeatOne, Choice, Sequence, Epsilon) must be preprocessed
    /// and normalized before GLR parsing.
    #[allow(dead_code)]
    fn can_reduce(&self, stack: &ParseStack, rule: &Rule) -> GLRResult<bool> {
        if stack.nodes.len() < rule.rhs.len() {
            return Ok(false);
        }

        // Check if the top of the stack matches the rule's RHS
        let start_idx = stack.nodes.len() - rule.rhs.len();
        for (i, symbol) in rule.rhs.iter().enumerate() {
            let node_symbol = match symbol {
                Symbol::Terminal(id) | Symbol::NonTerminal(id) => *id,
                Symbol::External(id) => *id,
                Symbol::Optional(_) => {
                    return Err(GLRError::ComplexSymbolNotNormalized {
                        symbol_type: "Optional".to_string(),
                        production_id: rule.production_id,
                        position: i,
                    });
                }
                Symbol::Repeat(_) => {
                    return Err(GLRError::ComplexSymbolNotNormalized {
                        symbol_type: "Repeat".to_string(),
                        production_id: rule.production_id,
                        position: i,
                    });
                }
                Symbol::RepeatOne(_) => {
                    return Err(GLRError::ComplexSymbolNotNormalized {
                        symbol_type: "RepeatOne".to_string(),
                        production_id: rule.production_id,
                        position: i,
                    });
                }
                Symbol::Choice(_) => {
                    return Err(GLRError::ComplexSymbolNotNormalized {
                        symbol_type: "Choice".to_string(),
                        production_id: rule.production_id,
                        position: i,
                    });
                }
                Symbol::Sequence(_) => {
                    return Err(GLRError::ComplexSymbolNotNormalized {
                        symbol_type: "Sequence".to_string(),
                        production_id: rule.production_id,
                        position: i,
                    });
                }
                Symbol::Epsilon => {
                    return Err(GLRError::ComplexSymbolNotNormalized {
                        symbol_type: "Epsilon".to_string(),
                        production_id: rule.production_id,
                        position: i,
                    });
                }
            };
            if stack.nodes[start_idx + i].node.symbol_id != node_symbol {
                return Ok(false);
            }
        }

        Ok(true)
    }

    /// Get action from parse table for state and symbol
    fn get_action(&self, state: StateId, symbol: SymbolId) -> Option<Action> {
        let state_idx = state.0 as usize;

        if state_idx < self.table.action_table.len()
            && let Some(&symbol_idx) = self.table.symbol_to_index.get(&symbol)
            && symbol_idx < self.table.action_table[state_idx].len()
        {
            let action_cell = &self.table.action_table[state_idx][symbol_idx];
            if action_cell.is_empty() {
                return Some(Action::Error);
            } else if action_cell.len() == 1 {
                return Some(action_cell[0].clone());
            } else {
                // Multiple actions - create a Fork with sorted actions
                let mut sorted_actions = action_cell.clone();
                sorted_actions.sort_by_key(|a| -self.action_priority(a));
                return Some(Action::Fork(sorted_actions));
            }
        }

        None
    }

    // Methods for incremental parsing state management

    /// Get the current GSS (Graph-Structured Stack) state for snapshots
    pub fn get_gss_state(&self) -> Vec<ParseStack> {
        self.stacks.clone()
    }

    /// Restore the GSS state from a snapshot
    pub fn set_gss_state(&mut self, stacks: Vec<ParseStack>) {
        self.stacks = stacks;
        self.pending_stacks.clear();
        // Re-populate pending stacks with all current stack indices
        for i in 0..self.stacks.len() {
            self.pending_stacks.push_back(i);
        }
    }

    /// Restore GSS state selectively - only restore the most promising stacks
    /// This is a performance optimization for incremental parsing
    pub fn set_gss_state_selective(&mut self, stacks: Vec<ParseStack>) {
        if stacks.is_empty() {
            self.stacks = stacks;
            self.pending_stacks.clear();
            return;
        }

        // AGGRESSIVE OPTIMIZATION: Only keep the single deepest stack
        // This dramatically reduces the work needed to process remaining tokens
        // If the middle chunk is ambiguous, the GLR mechanism will naturally
        // re-create forks as needed
        let best_stack = if let Some(best_stack) = stacks.into_iter().max_by_key(|s| s.states.len())
        {
            best_stack
        } else {
            self.pending_stacks.clear();
            return;
        };

        self.stacks = vec![best_stack];
        self.pending_stacks.clear();
        self.pending_stacks.push_back(0);
    }

    /// Get the next stack ID for restoring fork tracking
    pub fn get_next_stack_id(&self) -> usize {
        self.next_stack_id
    }

    /// Set the next stack ID for restoring fork tracking
    pub fn set_next_stack_id(&mut self, id: usize) {
        self.next_stack_id = id;
    }

    /// Inject multiple alternative subtrees (for ambiguous parses)
    /// This is used for incremental GLR parsing to preserve ambiguity
    pub fn inject_ambiguous_subtrees(&mut self, subtrees: Vec<Arc<Subtree>>) -> Result<(), String> {
        if self.stacks.is_empty() {
            return Err("No active stacks to inject subtrees into".to_string());
        }

        if subtrees.is_empty() {
            return Err("No subtrees to inject".to_string());
        }

        // For each subtree alternative, create potential parse stacks
        let mut new_stacks = Vec::new();

        for subtree in subtrees {
            for stack in &self.stacks {
                let new_stack = stack.clone();

                // Get the current state
                let current_state = new_stack.current_state();

                // Look up the goto state after shifting this symbol
                let symbol = subtree.node.symbol_id;
                if let Some(&symbol_idx) = self.table.symbol_to_index.get(&symbol) {
                    let state_idx = current_state.0 as usize;

                    // Check if there's a shift action for this symbol
                    if state_idx < self.table.action_table.len()
                        && symbol_idx < self.table.action_table[state_idx].len()
                    {
                        let action_cell = &self.table.action_table[state_idx][symbol_idx];

                        // Look for shift actions
                        for action in action_cell {
                            if let Action::Shift(next_state) = action {
                                // Push the subtree and advance to the next state
                                let mut forked_stack = new_stack.clone();
                                forked_stack.push(*next_state, subtree.clone());
                                new_stacks.push(forked_stack);
                            }
                        }
                    }
                }
            }
        }

        if new_stacks.is_empty() {
            return Err("Cannot inject any subtrees in current state".to_string());
        }

        self.stacks = new_stacks;
        Ok(())
    }

    /// Inject a pre-parsed subtree at the current position
    /// This is used for incremental parsing to reuse unchanged portions
    pub fn inject_subtree(&mut self, subtree: Arc<Subtree>) -> Result<(), String> {
        if self.stacks.is_empty() {
            return Err("No active stacks to inject subtree into".to_string());
        }

        // For each active stack, inject the subtree
        let mut new_stacks = Vec::new();
        for stack in &self.stacks {
            let mut new_stack = stack.clone();

            // Get the current state
            let current_state = new_stack.current_state();

            // Look up the goto state after shifting this symbol
            let symbol = subtree.node.symbol_id;
            if let Some(&symbol_idx) = self.table.symbol_to_index.get(&symbol) {
                let state_idx = current_state.0 as usize;

                // First check if there's a shift action for this symbol
                if state_idx < self.table.action_table.len()
                    && symbol_idx < self.table.action_table[state_idx].len()
                {
                    let action_cell = &self.table.action_table[state_idx][symbol_idx];

                    // Look for a shift action
                    for action in action_cell {
                        if let Action::Shift(next_state) = action {
                            // Push the subtree and advance to the next state
                            new_stack.push(*next_state, subtree.clone());
                            new_stacks.push(new_stack.clone());
                            break;
                        }
                    }
                }
            }
        }

        if new_stacks.is_empty() {
            return Err(format!(
                "Cannot inject subtree with symbol {:?} in current state",
                subtree.node.symbol_id
            ));
        }

        self.stacks = new_stacks;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_stack_creation() {
        let stack = ParseStack::new(StateId(0), 0);
        assert_eq!(stack.current_state(), StateId(0));
        assert_eq!(stack.nodes.len(), 0);
        assert_eq!(stack.version.dynamic_prec, 0);
    }

    #[test]
    fn test_parse_stack_fork() {
        let mut stack = ParseStack::new(StateId(0), 0);

        // Add a node
        let node = Arc::new(Subtree::new(
            SubtreeNode {
                symbol_id: SymbolId(1),
                is_error: false,
                byte_range: 0..5,
            },
            vec![],
        ));
        stack.push(StateId(1), node);

        // Fork the stack
        let forked = stack.fork(1);
        assert_eq!(forked.states, stack.states);
        assert_eq!(forked.nodes.len(), stack.nodes.len());
        assert_ne!(forked.id, stack.id);
    }

    #[test]
    fn test_dynamic_precedence_accumulation() {
        let mut stack = ParseStack::new(StateId(0), 0);

        // Add nodes with dynamic precedence
        let node1 = Arc::new(Subtree::with_dynamic_prec(
            SubtreeNode {
                symbol_id: SymbolId(1),
                is_error: false,
                byte_range: 0..5,
            },
            vec![],
            3,
        ));
        stack.push(StateId(1), node1);
        assert_eq!(stack.version.dynamic_prec, 3);

        let node2 = Arc::new(Subtree::with_dynamic_prec(
            SubtreeNode {
                symbol_id: SymbolId(2),
                is_error: false,
                byte_range: 5..10,
            },
            vec![],
            2,
        ));
        stack.push(StateId(2), node2);
        assert_eq!(stack.version.dynamic_prec, 5); // 3 + 2
    }
}
