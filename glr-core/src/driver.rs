//! Public driver that runs the GLR engine and returns a trait-object forest.

use crate::debug_trace;
use crate::forest_view::{Forest, ForestView, Span};
use crate::lexical_commitment::{CandidateOrigin, TokenCandidate, select_global_lexical_candidate};
use crate::parse_forest::{ForestAlternative, ForestNode, ParseForest};
use crate::{Action, ParseTable, RuleId, StateId, SymbolId};
use smallvec::SmallVec;
use std::collections::HashMap;

#[cfg(feature = "perf_counters")]
use crate::perf;

#[cfg(feature = "glr_telemetry")]
use crate::telemetry::Telemetry;

/// Helper function to safely convert usize spans to u32, avoiding overflow on giant buffers
#[inline]
fn u32_span(start: usize, end: usize) -> (u32, u32) {
    (
        start.min(u32::MAX as usize) as u32,
        end.min(u32::MAX as usize) as u32,
    )
}

/// Error type for GLR parsing operations
#[derive(thiserror::Error, Debug)]
pub enum GlrError {
    /// Lexer error
    #[error("lexer error: {0}")]
    Lex(String),
    /// Parse error
    #[error("parse error: {0}")]
    Parse(String),
    /// Other error
    #[error("{0}")]
    Other(String),
}

fn reduce_actions_for(actions: &[Action]) -> Vec<RuleId> {
    let mut reduce_rules = Vec::new();
    for action in actions {
        match *action {
            Action::Reduce(rid) => reduce_rules.push(rid),
            Action::Fork(ref xs) => {
                for nested in xs {
                    if let Action::Reduce(rid) = *nested {
                        reduce_rules.push(rid);
                    }
                }
            }
            _ => {}
        }
    }
    reduce_rules
}

/// GLR parser driver that executes the parsing algorithm
pub struct Driver<'t> {
    /// LR tables used by the driver
    #[allow(dead_code)]
    tables: &'t ParseTable,
    /// Telemetry instance for performance monitoring
    #[cfg(feature = "glr_telemetry")]
    telemetry: Option<&'t Telemetry>,
}

/// A GLR parse stack
#[derive(Debug, Clone, Default)]
struct ParseStack {
    states: Vec<StateId>,
    nodes: Vec<usize>, // Node IDs in the forest
    pos: usize,        // Current byte position (end of last consumed token)
    /// Total recovery cost accumulated on this path.
    error_cost: u32,
}

impl ParseStack {
    /// Get the top state from the stack, returning an error if empty.
    fn top_state(&self) -> Result<StateId, GlrError> {
        self.states
            .last()
            .copied()
            .ok_or_else(|| GlrError::Parse("Empty parse stack: no state available".to_string()))
    }
}

/// GLR parser state
struct GlrState {
    stacks: Vec<ParseStack>,
    forest: ParseForest,
    next_node_id: usize,
}

impl<'t> Driver<'t> {
    /// Maximum insertions allowed at a single position before forcing skip
    const MAX_INSERTS_PER_POS: u32 = 3;
    /// Hard cap on the number of active stacks to prevent exponential GLR blow-ups.
    const MAX_ACTIVE_STACKS: usize = 1024;

    /// Create a new driver with the given parse tables
    pub fn new(tables: &'t ParseTable) -> Self {
        // EOF must be present in symbol_to_index mapping
        debug_assert!(
            tables.symbol_to_index.contains_key(&tables.eof_symbol),
            "EOF symbol {} must be present in symbol_to_index/action table",
            tables.eof_symbol.0
        );

        // Validate tables when strict-invariants feature is enabled
        #[cfg(feature = "strict-invariants")]
        {
            if let Err(e) = tables.validate() {
                panic!("Invalid parse table: {}", e);
            }
        }

        Self {
            tables,
            #[cfg(feature = "glr_telemetry")]
            telemetry: None,
        }
    }

    /// Set telemetry instance for performance monitoring
    #[cfg(feature = "glr_telemetry")]
    pub fn with_telemetry(mut self, telemetry: &'t Telemetry) -> Self {
        self.telemetry = Some(telemetry);
        self
    }

    /// Build union of valid external symbols across all active stacks
    fn union_valid_external_symbols(&self, stacks: &[ParseStack]) -> Result<Vec<bool>, GlrError> {
        let mut mask = vec![false; self.tables.external_token_count];
        for stack in stacks {
            let top_state = stack.top_state()?;
            for ext_idx in 0..self.tables.external_token_count {
                let sym = SymbolId((self.tables.token_count + ext_idx) as u16);
                if !self.tables.actions(top_state, sym).is_empty() {
                    mask[ext_idx] = true;
                }
            }
        }
        Ok(mask)
    }

    /// Parse input with Tree-sitter compatible streaming lexer.
    ///
    /// This implements the full GLR-aware lexing algorithm:
    /// - Lexes at each position based on active stack states
    /// - Handles multiple lex modes when stacks diverge
    /// - Integrates external scanners when present
    pub fn parse_streaming<L, E>(
        &mut self,
        input: &str,
        mut internal_lexer: L,
        mut external_scanner: Option<E>,
    ) -> Result<Forest, GlrError>
    where
        L: FnMut(&str, usize, crate::LexMode) -> Option<crate::ts_lexer::NextToken>,
        E: FnMut(&str, usize, &[bool], crate::LexMode) -> Option<crate::ts_lexer::NextToken>,
    {
        // Initialize state with starting stack
        // Pre-size HashMap based on input length heuristic: roughly 1 node per 10 bytes
        let estimated_nodes = (input.len() / 10).max(64);
        let mut state = GlrState {
            stacks: vec![ParseStack {
                states: vec![self.tables.initial_state],
                nodes: vec![],
                pos: 0,
                error_cost: 0,
            }],
            forest: ParseForest {
                roots: vec![],
                nodes: HashMap::with_capacity(estimated_nodes),
                grammar: self.tables.grammar().clone(),
                source: input.to_string(),
                next_node_id: 0,
            },
            next_node_id: 0,
        };

        let mut pos = 0usize;
        let mut inserts_at_pos: u32 = 0;

        // Main parse loop - lex at each position based on active stacks
        while pos <= input.len() && !state.stacks.is_empty() {
            // If we're at EOF, use EOF token
            let lookahead = if pos >= input.len() {
                crate::ts_lexer::NextToken {
                    kind: self.tables.eof_symbol.0 as u32,
                    start: pos as u32,
                    end: pos as u32,
                }
            } else {
                // Gather distinct lex modes from all active stacks
                // Use SmallVec since there are typically only 1-2 distinct modes
                let mut modes: SmallVec<[crate::LexMode; 2]> = SmallVec::new();
                for stk in &state.stacks {
                    let top = stk.top_state()?;
                    let mode = self.tables.lex_mode(top);
                    if !modes.contains(&mode) {
                        modes.push(mode);
                    }
                }

                // Collect candidate tokens from all modes
                let mut candidates = Vec::new();

                for mode in modes {
                    // Try internal lexer (it handles extras/whitespace internally via advance)
                    if let Some(token) = internal_lexer(input, pos, mode) {
                        candidates.push((token, false)); // false = internal
                    }

                    // Try external scanner if applicable
                    if mode.external_lex_state != 0
                        && let Some(ref mut ext) = external_scanner
                    {
                        // Build union of valid external symbols across all stacks
                        let valid_ext = self.union_valid_external_symbols(&state.stacks)?;

                        // Only call external scanner if at least one symbol is valid
                        if valid_ext.iter().any(|&b| b)
                            && let Some(token) = ext(input, pos, &valid_ext, mode)
                        {
                            candidates.push((token, true)); // true = external
                        }
                    }
                }

                // Choose best candidate using the documented global lexical commitment policy (#928)
                if candidates.is_empty() {
                    return Err(GlrError::Parse(format!(
                        "cannot lex at byte {}: no valid tokens",
                        pos
                    )));
                }

                let stack_tops: Vec<StateId> = state
                    .stacks
                    .iter()
                    .filter_map(|stk| stk.top_state().ok())
                    .collect();
                let token_candidates: Vec<TokenCandidate> = candidates
                    .iter()
                    .map(|(token, is_external)| TokenCandidate {
                        token: *token,
                        origin: if *is_external {
                            CandidateOrigin::External
                        } else {
                            CandidateOrigin::Internal
                        },
                    })
                    .collect();

                select_global_lexical_candidate(self.tables, &token_candidates, &stack_tops)?.token
            };

            // Process this token through the GLR parser
            let token_sym = SymbolId(lookahead.kind as u16);
            let token_start = lookahead.start as usize;
            let token_end = lookahead.end as usize;

            // Process all stacks with this token
            let prev_stacks = std::mem::take(&mut state.stacks); // Take ownership to avoid clone
            // state.stacks is now empty, ready for filling with new_stacks
            let mut new_stacks = Vec::new();
            let mut has_any_real_action = false;

            for stk in prev_stacks.iter().cloned() {
                let top = stk.top_state()?;
                let pre_actions = self.tables.actions(top, token_sym);
                let reduce_rules = reduce_actions_for(pre_actions);
                let has_shift = pre_actions
                    .iter()
                    .any(|action| matches!(action, Action::Shift(_)));

                let stacks_to_process = if has_shift {
                    // Shift/reduce conflicts must branch from the conflict state; do not
                    // pre-reduce away the shift path before reading the action cell.
                    vec![stk]
                } else if reduce_rules.len() > 1 {
                    self.reduce_closure_stacks(&mut state, vec![stk], token_sym)?
                } else {
                    let mut stk = stk;
                    self.reduce_closure(&mut state, &mut stk, token_sym)?;
                    vec![stk]
                };

                for stk in stacks_to_process {
                    // Get actions and filter Recover if real actions exist
                    let top = stk.top_state()?;
                    let all_actions = self.tables.actions(top, token_sym);

                    // Check if we have any real actions without collecting into Vec
                    let has_real_action = all_actions
                        .iter()
                        .any(|a| !matches!(*a, Action::Recover | Action::Error));

                    if has_real_action {
                        has_any_real_action = true;
                    }

                    // Then apply shifts/accepts
                    // Use iterator filtering to avoid intermediate Vec allocation
                    for action in all_actions.iter().filter(|a| {
                        if has_real_action {
                            !matches!(**a, Action::Recover | Action::Error)
                        } else {
                            true
                        }
                    }) {
                        match *action {
                            Action::Shift(ns) => {
                                #[cfg(feature = "perf_counters")]
                                perf::inc_shifts(1);
                                let node_id = self.push_terminal(
                                    &mut state,
                                    token_sym,
                                    (token_start, token_end),
                                );
                                let mut s2 = stk.clone();
                                s2.states.push(ns);
                                s2.nodes.push(node_id);
                                s2.pos = token_end;
                                self.push_limited_stack(&mut new_stacks, s2, "shifting a token")?;
                            }
                            Action::Accept => {
                                if let Some(&root_id) = stk.nodes.last()
                                    && let Some(root) = state.forest.nodes.get(&root_id).cloned()
                                {
                                    state.forest.roots.push(root);
                                }
                                if token_sym == self.tables.eof_symbol {
                                    break;
                                }
                                return Ok(Self::wrap_forest(state.forest));
                            }
                            Action::Reduce(rid) => {
                                #[cfg(feature = "perf_counters")]
                                perf::inc_reductions(1);
                                #[cfg(feature = "glr_telemetry")]
                                if let Some(t) = self.telemetry {
                                    t.inc_reduce();
                                }
                                // Handle reduce+shift conflicts
                                let s2 = self.reduce_once(&mut state, stk.clone(), rid)?;
                                let mut s2_clone = s2.clone();
                                self.reduce_closure(&mut state, &mut s2_clone, token_sym)?;
                                // Try shift after reduce
                                let s2_top = s2_clone.top_state()?;
                                for a2 in self.tables.actions(s2_top, token_sym) {
                                    if let Action::Shift(ns) = *a2 {
                                        let node_id = self.push_terminal(
                                            &mut state,
                                            token_sym,
                                            (token_start, token_end),
                                        );
                                        let mut s3 = s2_clone.clone();
                                        s3.states.push(ns);
                                        s3.nodes.push(node_id);
                                        s3.pos = token_end;
                                        self.push_limited_stack(
                                            &mut new_stacks,
                                            s3,
                                            "branching after a reduction",
                                        )?;
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }

            // If no stack had any real action, try recovery
            if !has_any_real_action && new_stacks.is_empty() {
                // Restore previous stacks for recovery
                state.stacks = prev_stacks;

                // Try insertion first, but cap insertions per position
                if inserts_at_pos < Self::MAX_INSERTS_PER_POS
                    && self.try_insertion(&mut state, token_sym, pos)?
                {
                    inserts_at_pos = inserts_at_pos.saturating_add(1);
                    continue; // Re-lex at current position with new stacks
                }

                // Otherwise skip one byte as error (forces progress)
                if self.try_skip_one_byte(&mut state, input, &mut pos) {
                    inserts_at_pos = 0; // Reset counter after skip
                    continue;
                }

                // If we can't recover, fail
                return Err(GlrError::Parse(
                    "input not accepted: no valid parse and recovery failed".to_string(),
                ));
            } else {
                // Track forks when we have multiple stacks from a single parent
                #[cfg(feature = "glr_telemetry")]
                if let Some(t) = self.telemetry
                    && new_stacks.len() > 1
                {
                    t.inc_fork_by((new_stacks.len() - 1) as u64);
                }
                // Commit the new frontier
                if new_stacks.len() > Self::MAX_ACTIVE_STACKS {
                    return Err(GlrError::Parse(format!(
                        "parse aborted: active stack limit ({}) exceeded while processing byte {}",
                        Self::MAX_ACTIVE_STACKS,
                        pos,
                    )));
                }
                state.stacks = new_stacks;
                inserts_at_pos = 0; // Reset counter on successful real token
            }

            // Check for EOF acceptance
            if token_sym == self.tables.eof_symbol {
                break;
            }

            // Advance position if we consumed input
            if token_end > pos {
                pos = token_end;
                inserts_at_pos = 0; // Reset counter when position advances
            } else if pos < input.len() {
                // Ensure we make progress even with zero-width tokens
                pos += 1;
                inserts_at_pos = 0; // Reset counter when position advances
            }
        }

        self.complete_eof_phase(state)
    }

    fn push_limited_stack(
        &self,
        stacks: &mut Vec<ParseStack>,
        stack: ParseStack,
        context: &str,
    ) -> Result<(), GlrError> {
        if stacks.len() >= Self::MAX_ACTIVE_STACKS {
            return Err(GlrError::Parse(format!(
                "parse aborted: active stack limit ({}) exceeded while {}",
                Self::MAX_ACTIVE_STACKS,
                context,
            )));
        }
        stacks.push(stack);
        Ok(())
    }

    /// Parse from a token stream.
    ///
    /// The token stream should already have extras (whitespace/comments) filtered out.
    /// For Tree-sitter compatibility, use parse_streaming instead which handles per-position lexing.
    pub fn parse_tokens<I>(&mut self, tokens: I) -> Result<Forest, GlrError>
    where
        I: IntoIterator<
            Item = (
                u32, /* kind */
                u32, /* start */
                u32, /* end */
            ),
        >,
    {
        // Initialize state with grammar from parse table
        // Use initial_state from ParseTable (default 0, Tree-sitter uses 1)
        let mut state = GlrState {
            stacks: vec![ParseStack {
                states: vec![self.tables.initial_state],
                nodes: vec![],
                pos: 0,
                error_cost: 0,
            }],
            forest: ParseForest {
                roots: vec![],
                nodes: HashMap::new(),
                grammar: self.tables.grammar().clone(),
                source: String::new(),
                next_node_id: 0,
            },
            next_node_id: 0,
        };

        // Main token loop
        for (kind, start, end) in tokens.into_iter() {
            // Add debug assert for token width
            debug_assert!(kind <= u16::MAX as u32, "terminal id overflow");
            let lookahead = SymbolId(kind as u16);

            let prev_stacks = std::mem::take(&mut state.stacks); // Take ownership to avoid clone
            // state.stacks is now empty, ready for filling with new_stacks
            let mut new_stacks = Vec::with_capacity(prev_stacks.len());

            for mut stk in prev_stacks.iter().cloned() {
                // 1) Closure: apply all reduces available on this lookahead BEFORE any shift
                self.reduce_closure(&mut state, &mut stk, lookahead)?;

                // 2) Get actions and filter Recover if real actions exist
                let top = stk.top_state()?;
                let all_actions = self.tables.actions(top, lookahead);

                // Check if we have any real actions without collecting into Vec
                let has_real_action = all_actions
                    .iter()
                    .any(|a| !matches!(*a, Action::Recover | Action::Error));

                // 3) Then apply shifts for this lookahead
                // Use iterator filtering to avoid intermediate Vec allocation
                for action in all_actions.iter().filter(|a| {
                    if has_real_action {
                        !matches!(**a, Action::Recover | Action::Error)
                    } else {
                        true
                    }
                }) {
                    match *action {
                        Action::Shift(ns) => {
                            #[cfg(feature = "perf_counters")]
                            perf::inc_shifts(1);
                            let node_id = self.push_terminal(
                                &mut state,
                                lookahead,
                                (start as usize, end as usize),
                            );
                            let mut s2 = stk.clone();
                            s2.states.push(ns);
                            s2.nodes.push(node_id);
                            s2.pos = end as usize; // Update position to token end
                            self.push_limited_stack(&mut new_stacks, s2, "shifting a token")?;
                        }
                        Action::Accept => {
                            // Accept on lookahead (rare, usually on EOF)
                            if let Some(&root_id) = stk.nodes.last()
                                && let Some(root) = state.forest.nodes.get(&root_id).cloned()
                            {
                                // Assert the accepted root is the start symbol (catches table/config drift)
                                debug_assert_eq!(
                                    root.symbol,
                                    self.tables.start_symbol(),
                                    "accepted non-start symbol: {:?} != {:?}",
                                    root.symbol,
                                    self.tables.start_symbol()
                                );
                                state.forest.roots.push(root);
                            }
                            return Ok(Self::wrap_forest(state.forest));
                        }
                        Action::Reduce(rid) => {
                            #[cfg(feature = "perf_counters")]
                            perf::inc_reductions(1);
                            #[cfg(feature = "glr_telemetry")]
                            if let Some(t) = self.telemetry {
                                t.inc_reduce();
                            }
                            // If your table encodes reduce+shift conflicts, we still need to try the reduce path
                            let s2 = self.reduce_once(&mut state, stk.clone(), rid)?;
                            // After a single reduce, we can still be able to shift this lookahead
                            let mut s2_clone = s2.clone();
                            self.reduce_closure(&mut state, &mut s2_clone, lookahead)?;
                            let s2_top = s2_clone.top_state()?;
                            for a2 in self.tables.actions(s2_top, lookahead) {
                                if let Action::Shift(ns) = *a2 {
                                    let node_id = self.push_terminal(
                                        &mut state,
                                        lookahead,
                                        (start as usize, end as usize),
                                    );
                                    let mut s3 = s2_clone.clone();
                                    s3.states.push(ns);
                                    s3.nodes.push(node_id);
                                    s3.pos = end as usize; // Update position to token end
                                    self.push_limited_stack(
                                        &mut new_stacks,
                                        s3,
                                        "branching after a reduction",
                                    )?;
                                }
                            }
                        }
                        Action::Error => { /* drop path */ }
                        Action::Recover => {
                            // Tree-sitter error recovery: insert a missing/error node
                            // For now, treat as Error until we implement proper recovery
                            // TODO: Insert ERROR node and continue parsing
                        }
                        Action::Fork(ref xs) => {
                            #[cfg(feature = "perf_counters")]
                            perf::inc_forks(1);
                            // If your generator emits Fork, just treat as a set of actions
                            for a in xs {
                                if let Action::Shift(ns) = *a {
                                    let node_id = self.push_terminal(
                                        &mut state,
                                        lookahead,
                                        (start as usize, end as usize),
                                    );
                                    let mut s2 = stk.clone();
                                    s2.states.push(ns);
                                    s2.nodes.push(node_id);
                                    s2.pos = end as usize; // Update position to token end
                                    self.push_limited_stack(
                                        &mut new_stacks,
                                        s2,
                                        "shifting a token",
                                    )?;
                                } else if let Action::Reduce(rid) = *a {
                                    let mut s2 = self.reduce_once(&mut state, stk.clone(), rid)?;
                                    self.reduce_closure(&mut state, &mut s2, lookahead)?;
                                    // After closure, check if we can shift
                                    let s2_top = s2.top_state()?;
                                    for a2 in self.tables.actions(s2_top, lookahead) {
                                        if let Action::Shift(ns) = *a2 {
                                            let node_id = self.push_terminal(
                                                &mut state,
                                                lookahead,
                                                (start as usize, end as usize),
                                            );
                                            let mut s3 = s2.clone();
                                            s3.states.push(ns);
                                            s3.nodes.push(node_id);
                                            s3.pos = end as usize;
                                            self.push_limited_stack(
                                                &mut new_stacks,
                                                s3,
                                                "branching after a reduction",
                                            )?;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            if new_stacks.is_empty() {
                // Restore previous stacks for recovery
                state.stacks = prev_stacks;

                // Try insertion recovery for token streams (no skip since we can't re-lex)
                if self.try_insertion(&mut state, lookahead, start as usize)? {
                    // Continue with recovered stacks
                    continue;
                }

                // If insertion didn't help, fail
                let top_state = if !state.stacks.is_empty() {
                    *state.stacks[0]
                        .states
                        .last()
                        .unwrap_or(&self.tables.initial_state)
                } else {
                    self.tables.initial_state
                };
                return Err(GlrError::Parse(format!(
                    "no valid parse paths at byte {} (state={}, symbol={})",
                    start, top_state.0, lookahead.0
                )));
            } else {
                if new_stacks.len() > Self::MAX_ACTIVE_STACKS {
                    return Err(GlrError::Parse(format!(
                        "parse aborted: active stack limit ({}) exceeded at byte {}",
                        Self::MAX_ACTIVE_STACKS,
                        start,
                    )));
                }
                state.stacks = new_stacks;
            }
        }

        self.complete_eof_phase(state)
    }

    /// Collect complete parse roots from surviving stacks at EOF.
    fn complete_eof_phase(&mut self, mut state: GlrState) -> Result<Forest, GlrError> {
        let eof = self.tables.eof();
        #[cfg(feature = "debug_glr")]
        debug_trace!(
            "DEBUG: EOF phase starting with {} stack(s)",
            state.stacks.len()
        );

        let stacks = std::mem::take(&mut state.stacks);
        for stk in stacks {
            for stk in self.reduce_closure_stacks(&mut state, vec![stk], eof)? {
                let _top_after_reduce = stk.top_state()?;
                debug_trace!(
                    "DEBUG: After reduce_closure, checking actions for state {} on EOF",
                    _top_after_reduce.0
                );

                // Check if we have the start symbol on top of the stack
                if let Some(&root_id) = stk.nodes.last()
                    && let Some(root) = state.forest.nodes.get(&root_id)
                {
                    debug_trace!("DEBUG: Top node has symbol {}", root.symbol.0);
                    if root.symbol == self.tables.start_symbol() {
                        debug_trace!("DEBUG: Found start symbol! Adding as root");
                        state.forest.roots.push(root.clone());
                    }
                }

                let top_state = stk.states.last().expect("GLR stack must never be empty");
                for action in self.tables.actions(*top_state, eof) {
                    debug_trace!("DEBUG: EOF action: {:?}", action);
                    match *action {
                        Action::Accept => {
                            debug_trace!("DEBUG: Accept action found");
                            if let Some(&root_id) = stk.nodes.last()
                                && let Some(root) = state.forest.nodes.get(&root_id).cloned()
                            {
                                // Assert the accepted root is the start symbol (catches table/config drift)
                                debug_assert_eq!(
                                    root.symbol,
                                    self.tables.start_symbol(),
                                    "accepted non-start symbol: {:?} != {:?}",
                                    root.symbol,
                                    self.tables.start_symbol()
                                );
                                state.forest.roots.push(root);
                            }
                            break;
                        }
                        Action::Reduce(rid) => {
                            debug_trace!("DEBUG: Reduce action found, rule {}", rid.0);
                            let s2 = self.reduce_once(&mut state, stk.clone(), rid)?;

                            // Check if reduction produced start symbol
                            if let Some(&root_id) = s2.nodes.last()
                                && let Some(root) = state.forest.nodes.get(&root_id)
                            {
                                #[cfg(feature = "debug_glr")]
                                debug_trace!(
                                    "DEBUG: After reduction, top symbol is {}",
                                    root.symbol.0
                                );
                                if root.symbol == self.tables.start_symbol() {
                                    #[cfg(feature = "debug_glr")]
                                    debug_trace!("DEBUG: Reduced to start symbol! Adding as root");
                                    state.forest.roots.push(root.clone());
                                }
                            }

                            // Try accept after reduce
                            let s2_top = s2.states.last().expect("GLR stack must never be empty");
                            for a2 in self.tables.actions(*s2_top, eof) {
                                if let Action::Accept = *a2 {
                                    if let Some(&root_id) = s2.nodes.last()
                                        && let Some(root) =
                                            state.forest.nodes.get(&root_id).cloned()
                                    {
                                        // Assert the accepted root is the start symbol (catches table/config drift)
                                        debug_assert_eq!(
                                            root.symbol,
                                            self.tables.start_symbol(),
                                            "accepted non-start symbol: {:?} != {:?}",
                                            root.symbol,
                                            self.tables.start_symbol()
                                        );
                                        state.forest.roots.push(root);
                                    }
                                    break;
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        // If we found any roots with the start symbol, accept the parse
        if !state.forest.roots.is_empty() {
            debug_trace!(
                "DEBUG: Accepting parse with {} root(s)",
                state.forest.roots.len()
            );
            return Ok(Self::wrap_forest(state.forest));
        }

        Err(GlrError::Parse(format!(
            "input not accepted: EOF phase failed (expected start symbol {}, got {} root(s))",
            self.tables.start_symbol().0,
            state.forest.roots.len()
        )))
    }

    #[inline]
    fn push_terminal(&self, st: &mut GlrState, sym: SymbolId, span: (usize, usize)) -> usize {
        Self::push_terminal_with_meta_static(st, sym, span, Default::default())
    }

    #[inline]
    fn push_terminal_with_meta_static(
        st: &mut GlrState,
        sym: SymbolId,
        span: (usize, usize),
        meta: crate::parse_forest::ErrorMeta,
    ) -> usize {
        let id = st.next_node_id;
        st.next_node_id += 1;
        st.forest.nodes.insert(
            id,
            ForestNode {
                id,
                symbol: sym,
                span,
                alternatives: vec![ForestAlternative { children: vec![] }],
                error_meta: meta,
            },
        );
        id
    }

    /// Apply exactly one reduce(rid) to `stack`; return the new stack with a pushed goto state.
    fn reduce_once(
        &self,
        st: &mut GlrState,
        mut stack: ParseStack,
        rid: RuleId,
    ) -> Result<ParseStack, GlrError> {
        let (lhs, rhs_len) = self.tables.rule(rid);
        if rhs_len as usize > stack.nodes.len()
            || rhs_len as usize > stack.states.len().saturating_sub(1)
        {
            return Err(GlrError::Parse(format!(
                "reduce underflow: rule {} requires {} symbols but stack has {}",
                rid.0,
                rhs_len,
                stack.nodes.len()
            )));
        }

        // Pop rhs_len nodes/states (states pop rhs_len; bottom state remains)
        let child_ids: Vec<usize> = stack.nodes.split_off(stack.nodes.len() - rhs_len as usize);
        let goto_from = *stack
            .states
            .get(stack.states.len() - 1 - rhs_len as usize)
            .ok_or_else(|| {
                GlrError::Parse(format!(
                    "stack underflow: cannot find goto state for rule {}",
                    rid.0
                ))
            })?;
        stack.states.truncate(stack.states.len() - rhs_len as usize);

        // Span = [first_child.start, last_child.end], or current position if empty production
        let (start, end) = if child_ids.is_empty() {
            // Empty production - use current position
            (stack.pos, stack.pos)
        } else {
            let first_id = child_ids
                .first()
                .expect("child_ids verified non-empty above");
            let last_id = child_ids
                .last()
                .expect("child_ids verified non-empty above");
            let first = st
                .forest
                .nodes
                .get(first_id)
                .ok_or_else(|| GlrError::Parse(format!("missing forest node {first_id}")))?
                .span
                .0;
            let last = st
                .forest
                .nodes
                .get(last_id)
                .ok_or_else(|| GlrError::Parse(format!("missing forest node {last_id}")))?
                .span
                .1;
            (first, last)
        };

        // Build nonterminal node
        let id = st.next_node_id;
        st.next_node_id += 1;
        st.forest.nodes.insert(
            id,
            ForestNode {
                id,
                symbol: lhs,
                span: (start, end),
                alternatives: vec![ForestAlternative {
                    children: child_ids,
                }],
                error_meta: Default::default(), // Non-terminals have no error metadata
            },
        );

        // Goto
        let Some(ns) = self.tables.goto(goto_from, lhs) else {
            return Err(GlrError::Parse(format!(
                "missing goto: no transition from state {} on symbol {}",
                goto_from.0, lhs.0
            )));
        };
        stack.states.push(ns);
        stack.nodes.push(id);
        Ok(stack)
    }

    /// Keep reducing as long as there is at least one reduce for (top, lookahead).
    ///
    /// When multiple reduce actions are available (reduce/reduce), fork into one stack
    /// per reduction so ambiguity survives through EOF materialization.
    fn reduce_closure_stacks(
        &self,
        st: &mut GlrState,
        stacks: Vec<ParseStack>,
        lookahead: SymbolId,
    ) -> Result<Vec<ParseStack>, GlrError> {
        let mut frontier = stacks;
        loop {
            let mut next = Vec::new();
            let mut progressed = false;

            for stk in frontier {
                let state = *stk.states.last().expect("GLR stack must never be empty");
                let reduce_rules = reduce_actions_for(self.tables.actions(state, lookahead));

                if reduce_rules.is_empty() {
                    next.push(stk);
                    continue;
                }

                progressed = true;
                for rid in reduce_rules {
                    next.push(self.reduce_once(st, stk.clone(), rid)?);
                }
            }

            if !progressed {
                return Ok(next);
            }
            frontier = next;
        }
    }

    fn reduce_closure(
        &self,
        st: &mut GlrState,
        stack: &mut ParseStack,
        lookahead: SymbolId,
    ) -> Result<(), GlrError> {
        let stacks = self.reduce_closure_stacks(st, vec![std::mem::take(stack)], lookahead)?;
        *stack = stacks
            .into_iter()
            .next()
            .expect("reduce_closure must retain at least one stack");
        Ok(())
    }

    /// Recovery beam width - keep stacks within best_cost + RECOVERY_BEAM
    pub const RECOVERY_BEAM: u32 = 3;

    /// Try inserting a zero-width terminal that is actionable in the top state.
    /// Returns true if we made progress (i.e., at least one new stack advanced).
    fn try_insertion(
        &self,
        state: &mut GlrState,
        next_lookahead: SymbolId,
        pos: usize,
    ) -> Result<bool, GlrError> {
        let mut progressed = false;
        let mut next = Vec::new();

        // Take stacks to avoid clone
        let stacks = std::mem::take(&mut state.stacks);

        for stk in &stacks {
            let top = *stk.states.last().expect("GLR stack must never be empty");

            // Find terminals with any real (non-Recover) action from this state
            // Iterate to terminal_boundary (excludes EOF by definition)
            for sym_id in 0..self.tables.terminal_boundary() {
                let sym = SymbolId(sym_id as u16);

                // Skip EOF (redundant now but harmless for clarity)
                if sym == self.tables.eof_symbol {
                    continue;
                }

                // Skip extras (whitespace/comments) - never insert extras
                if self.tables.is_extra(sym) {
                    continue;
                }

                let acts = self.tables.actions(top, sym);

                // Skip if only has Recover/Error actions
                if acts
                    .iter()
                    .all(|a| matches!(a, Action::Recover | Action::Error))
                {
                    continue;
                }

                // Insert zero-width terminal (missing)
                let node_id = Self::push_terminal_with_meta_static(
                    state,
                    sym,
                    (pos, pos),
                    crate::parse_forest::ErrorMeta {
                        missing: true,
                        cost: 1,
                        ..Default::default()
                    },
                );

                let mut s2 = stk.clone();
                s2.nodes.push(node_id);

                // Apply actions as usual (shift/reduce/accept) on this inserted symbol
                for a in acts {
                    match *a {
                        Action::Shift(ns) => {
                            let mut s3 = s2.clone();
                            s3.states.push(ns);
                            s3.error_cost = s3.error_cost.saturating_add(1);
                            // Run closure with the real lookahead after insertion shift
                            self.reduce_closure(state, &mut s3, next_lookahead)?;
                            next.push(s3);
                            progressed = true;
                        }
                        Action::Reduce(rid) => {
                            let mut s3 = self.reduce_once(state, s2.clone(), rid)?;
                            // Use real lookahead for closure
                            self.reduce_closure(state, &mut s3, next_lookahead)?;
                            s3.error_cost = s3.error_cost.saturating_add(1);
                            next.push(s3);
                            progressed = true;
                        }
                        Action::Accept => {
                            // Valid when at EOF; treat as success
                            let mut s3 = s2.clone();
                            s3.error_cost = s3.error_cost.saturating_add(1);
                            next.push(s3);
                            progressed = true;
                        }
                        Action::Recover | Action::Error | Action::Fork(_) => { /* ignore here */ }
                    }
                }
            }
        }

        if progressed {
            state.stacks = Self::prune_by_cost(next);
        } else {
            // Restore stacks if no progress
            state.stacks = stacks;
        }
        Ok(progressed)
    }

    /// If insertion cannot help, consume one byte into an ERROR chunk.
    #[must_use]
    fn try_skip_one_byte(&self, state: &mut GlrState, input: &str, pos: &mut usize) -> bool {
        if *pos < input.len() {
            let start = *pos;
            *pos += 1;

            // Create a proper error node outside the grammar's symbol space
            let node_id = state.forest.push_error_chunk((start, *pos));
            // Keep state's next_node_id in sync
            state.next_node_id = state.forest.next_node_id;

            for stk in &mut state.stacks {
                stk.nodes.push(node_id);
                stk.error_cost = stk.error_cost.saturating_add(1);
                stk.pos = *pos;
            }
            state.stacks = Self::prune_by_cost(std::mem::take(&mut state.stacks));
            true
        } else {
            false
        }
    }

    fn prune_by_cost(mut stacks: Vec<ParseStack>) -> Vec<ParseStack> {
        if stacks.is_empty() {
            return stacks;
        }
        let best = stacks.iter().map(|s| s.error_cost).min().unwrap_or(0);
        stacks.retain(|s| s.error_cost <= best.saturating_add(Self::RECOVERY_BEAM));
        stacks
    }

    /// Convert internal parse forest to public Forest
    pub(crate) fn wrap_forest(mut forest: ParseForest) -> Forest {
        // Deterministic root selection: prefer largest span, then earliest start position
        forest.roots.sort_by_key(|n| {
            (
                std::cmp::Reverse(n.span.1.saturating_sub(n.span.0)), // Largest span first
                n.span.0, // Then earliest start position
            )
        });
        // Remove duplicate roots by ID
        forest.roots.dedup_by_key(|n| n.id);

        #[cfg(any(test, feature = "test-api", feature = "test_helpers"))]
        let error_stats = forest.debug_error_stats();

        let view = Box::new(ParseForestView::new(forest));
        Forest {
            view,
            #[cfg(any(test, feature = "test-api", feature = "test_helpers"))]
            test_hooks: Some(crate::forest_view::ForestTestHooks { error_stats }),
        }
    }
}

struct ParseForestView {
    forest: ParseForest,
    roots_cache: Vec<u32>,
    children_cache: HashMap<u32, Vec<u32>>,
}

impl ParseForestView {
    fn new(forest: ParseForest) -> Self {
        let roots_cache = forest.roots.iter().map(|n| n.id as u32).collect();

        // Pre-compute children cache
        let mut children_cache = HashMap::new();
        for (&id, node) in &forest.nodes {
            if !node.alternatives.is_empty() {
                // Take first alternative (best)
                let children: Vec<u32> = node.alternatives[0]
                    .children
                    .iter()
                    .map(|&c| c as u32)
                    .collect();
                children_cache.insert(id as u32, children);
            }
        }

        Self {
            forest,
            roots_cache,
            children_cache,
        }
    }
}

impl crate::forest_view::sealed::Sealed for ParseForestView {}

impl ForestView for ParseForestView {
    fn roots(&self) -> &[u32] {
        &self.roots_cache
    }

    fn kind(&self, id: u32) -> u32 {
        if let Some(node) = self.forest.nodes.get(&(id as usize)) {
            node.symbol.0 as u32
        } else {
            0
        }
    }

    fn span(&self, id: u32) -> Span {
        if let Some(node) = self.forest.nodes.get(&(id as usize)) {
            let (start, end) = u32_span(node.span.0, node.span.1);
            Span { start, end }
        } else {
            Span { start: 0, end: 0 }
        }
    }

    fn best_children(&self, id: u32) -> &[u32] {
        self.children_cache
            .get(&id)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    fn error_meta(&self, id: u32) -> crate::parse_forest::ErrorMeta {
        self.forest
            .nodes
            .get(&(id as usize))
            .map(|node| node.error_meta)
            .unwrap_or_default()
    }
}
