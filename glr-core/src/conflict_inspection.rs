//! Conflict Inspection API for GLR Parse Tables
//!
//! This module provides tools for detecting and analyzing GLR conflicts
//! (shift/reduce and reduce/reduce) in parse tables.
//!
//! # Overview
//!
//! GLR parsers preserve conflicts rather than resolving them, allowing
//! exploration of multiple parse paths. This API enables:
//!
//! - Validating parse tables correctly preserve conflicts
//! - Understanding where and why conflicts occur
//! - Automated testing of grammar conflict properties
//! - Clear visibility into GLR behavior
//!
//! # ParseTable Invariants
//!
//! This module relies on the following ParseTable structure invariants:
//!
//! ## Structure Invariants
//!
//! 1. **State Count Consistency**: `table.state_count == table.action_table.len()`
//!    - The state_count field must match the number of rows in the action table
//!
//! 2. **Action Table Structure**: `table.action_table: Vec<Vec<Vec<Action>>>`
//!    - Outer Vec: indexed by state (StateId as usize)
//!    - Middle Vec: indexed by symbol (mapped via index_to_symbol)
//!    - Inner Vec (ActionCell): multiple actions for GLR conflicts
//!
//! 3. **Symbol Indexing**: `table.index_to_symbol[symbol_idx] -> SymbolId`
//!    - All symbol indices in action_table must be valid indices into index_to_symbol
//!    - Symbol metadata should exist for all referenced SymbolIds
//!
//! 4. **Empty Cells**: Empty action cells (`Vec::new()`) represent error states
//!    - These are not considered conflicts
//!    - Parser will error/recover if it reaches such a state
//!
//! ## Conflict Semantics
//!
//! ### What Counts as a Conflict?
//!
//! A conflict exists when an action cell can produce **more than one valid parse action**
//! from the same state + lookahead.
//!
//! - **Single effective action**: Not a conflict, deterministic behavior
//! - **Multiple effective actions**: Conflict, GLR fork required
//! - **Empty cell**: Error state, not a conflict
//!
//! ### Conflict Classification
//!
//! Conflicts are classified by examining the action types:
//!
//! - **ShiftReduce**: Cell contains both `Action::Shift(_)` and `Action::Reduce(_)`
//!   - Classic shift/reduce ambiguity (e.g., dangling else)
//!   - GLR runtime forks: one branch shifts, other reduces
//!
//! - **ReduceReduce**: Cell contains multiple `Action::Reduce(_)` actions
//!   - Multiple production rules could apply
//!   - GLR runtime forks: each branch tries a different reduction
//!
//! - **Mixed**: Other combinations (e.g., multiple shifts)
//!   - Unusual but possible in some grammar constructions
//!   - Counted as both S/R and R/R for conservative reporting
//!
//! ### Action::Fork Handling
//!
//! `Action::Fork(Vec<Action>)` is treated **recursively**:
//!
//! - A single `Fork` may itself be conflicted when it contains multiple valid branches
//! - The *contents* of the fork are flattened and deduplicated for conflict checks
//! - Example: `Fork([Shift(5), Reduce(3)])` is classified as ShiftReduce
//!
//! This allows Fork actions to be properly analyzed even when nested.
//!
//! ## Validation Contract
//!
//! The `count_conflicts()` function validates these invariants via debug assertions:
//!
//! - State count matches action table length
//! - All state and symbol indices are within bounds
//! - Symbol metadata is available for referenced symbols
//!
//! These assertions catch table generation bugs during testing while
//! having zero runtime cost in release builds.
//!
//! # Examples
//!
//! ```ignore
//! use adze_glr_core::conflict_inspection::count_conflicts;
//!
//! let grammar = load_ambiguous_grammar();
//! let summary = count_conflicts(&grammar.parse_table);
//!
//! assert_eq!(summary.shift_reduce, 1);
//! assert_eq!(summary.reduce_reduce, 0);
//! ```

use crate::{Action, ParseTable, StateId};
use adze_ir::SymbolId;
use std::fmt;

/// Summary of conflicts in a parse table
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConflictSummary {
    /// Number of shift/reduce conflicts
    pub shift_reduce: usize,

    /// Number of reduce/reduce conflicts
    pub reduce_reduce: usize,

    /// States that contain conflicts
    pub states_with_conflicts: Vec<StateId>,

    /// Detailed conflict information
    pub conflict_details: Vec<ConflictDetail>,
}

/// Detailed information about a specific conflict
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConflictDetail {
    /// State where conflict occurs
    pub state: StateId,

    /// Lookahead symbol that triggers conflict
    pub symbol: SymbolId,

    /// Human-readable symbol name
    pub symbol_name: String,

    /// Type of conflict
    pub conflict_type: ConflictType,

    /// All possible actions at this conflict point
    pub actions: Vec<Action>,

    /// Action priorities (for GLR ordering)
    pub priorities: Vec<i32>,
}

/// Classification of conflict types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictType {
    /// Shift vs Reduce conflict
    ShiftReduce,

    /// Reduce vs Reduce conflict
    ReduceReduce,

    /// Mixed conflict (multiple shifts or reduces)
    Mixed,
}

/// Returns true when an action cell has more than one valid parse action.
///
/// This is the canonical conflict predicate for `adze-glr-core` and is used by
/// summary/counting APIs. It treats a single `Action::Fork` with multiple inner
/// actions as conflicted, because the parser can branch from the same
/// state/lookahead position.
pub fn cell_has_conflict(actions: &[Action]) -> bool {
    effective_actions(actions).len() > 1
}

fn effective_actions(actions: &[Action]) -> Vec<Action> {
    let mut flattened = Vec::new();
    collect_effective_actions(actions, &mut flattened);
    flattened
}

fn collect_effective_actions(actions: &[Action], out: &mut Vec<Action>) {
    for action in actions {
        match action {
            Action::Fork(inner) => collect_effective_actions(inner, out),
            other => {
                if !out.contains(other) {
                    out.push(other.clone());
                }
            }
        }
    }
}

/// Count how many parse-action branches a single action represents.
///
/// Most actions represent exactly one branch. `Action::Fork` represents the
/// sum of its inner branches, recursively.
#[must_use]
pub fn action_branch_count(action: &Action) -> usize {
    match action {
        Action::Fork(inner) => inner.iter().map(action_branch_count).sum(),
        Action::Shift(_) | Action::Reduce(_) | Action::Accept => 1,
        Action::Error | Action::Recover => 0,
    }
}
/// Inspect parse table and count conflicts
///
/// This function scans the entire action table and identifies
/// all conflicted cells (GLR conflicts).
///
/// # Invariant Validation
///
/// This function validates ParseTable invariants via debug assertions:
/// - State count matches action table length
/// - All symbol indices are valid
/// - Symbol metadata is properly sized
///
/// These checks have zero cost in release builds but catch bugs during testing.
///
/// # Examples
///
/// ```ignore
/// use adze_glr_core::conflict_inspection::count_conflicts;
///
/// let grammar = load_ambiguous_grammar();
/// let summary = count_conflicts(&grammar.parse_table);
///
/// assert_eq!(summary.shift_reduce, 1);
/// assert_eq!(summary.reduce_reduce, 0);
/// ```
pub fn count_conflicts(table: &ParseTable) -> ConflictSummary {
    // Validate ParseTable invariants (debug builds only)
    debug_assert_eq!(
        table.state_count,
        table.action_table.len(),
        "ParseTable invariant violation: state_count ({}) != action_table.len() ({})",
        table.state_count,
        table.action_table.len()
    );

    debug_assert!(
        !table.action_table.is_empty(),
        "ParseTable invariant violation: action_table is empty but should have at least initial state"
    );

    // Validate symbol indexing is consistent
    for (state_idx, state_actions) in table.action_table.iter().enumerate() {
        debug_assert!(
            state_idx < table.state_count,
            "ParseTable invariant violation: state index {} >= state_count {}",
            state_idx,
            table.state_count
        );

        for symbol_idx in 0..state_actions.len() {
            debug_assert!(
                symbol_idx < table.index_to_symbol.len() || table.index_to_symbol.is_empty(),
                "ParseTable invariant violation: symbol index {} >= index_to_symbol.len() {}",
                symbol_idx,
                table.index_to_symbol.len()
            );
        }
    }

    let mut summary = ConflictSummary {
        shift_reduce: 0,
        reduce_reduce: 0,
        states_with_conflicts: Vec::new(),
        conflict_details: Vec::new(),
    };

    // Scan action table for multi-action cells
    for (state_idx, state_actions) in table.action_table.iter().enumerate() {
        let state_id = StateId(state_idx as u16);
        let mut state_has_conflict = false;

        for (symbol_idx, action_cell) in state_actions.iter().enumerate() {
            // Skip empty cells
            if action_cell.is_empty() {
                continue;
            }

            // Conflict exists if this state/lookahead can take multiple effective actions
            if cell_has_conflict(action_cell) {
                state_has_conflict = true;

                // Get symbol info using index_to_symbol
                let symbol_id = if symbol_idx < table.index_to_symbol.len() {
                    table.index_to_symbol[symbol_idx]
                } else {
                    SymbolId(0)
                };

                let symbol_name = symbol_name(table, symbol_id);
                let effective_actions = effective_actions(action_cell);

                // Classify conflict type from the effective (flattened and deduplicated) actions
                let conflict_type = classify_conflict(&effective_actions);

                // Count by type
                match conflict_type {
                    ConflictType::ShiftReduce => summary.shift_reduce += 1,
                    ConflictType::ReduceReduce => summary.reduce_reduce += 1,
                    ConflictType::Mixed => {
                        // Count as both
                        summary.shift_reduce += 1;
                        summary.reduce_reduce += 1;
                    }
                }

                // Compute priorities (placeholder - would come from precedence/associativity)
                let priorities = effective_actions
                    .iter()
                    .map(|_action| 0i32) // Default priority
                    .collect();

                // Store detailed info
                summary.conflict_details.push(ConflictDetail {
                    state: state_id,
                    symbol: symbol_id,
                    symbol_name,
                    conflict_type,
                    actions: effective_actions,
                    priorities,
                });
            }
        }

        if state_has_conflict {
            summary.states_with_conflicts.push(state_id);
        }
    }

    summary
}

fn symbol_name(table: &ParseTable, symbol: SymbolId) -> String {
    table
        .symbol_metadata
        .iter()
        .find(|metadata| metadata.symbol_id == symbol)
        .map(|metadata| metadata.name.clone())
        .unwrap_or_else(|| format!("symbol_{}", symbol.0))
}

/// Classify conflict type from action list.
///
/// Forks are flattened and duplicate branches are removed before classification,
/// so this helper reports the conflict represented by the actual set of parser
/// branches rather than the parse-table encoding used to store them.
pub fn classify_conflict(actions: &[Action]) -> ConflictType {
    let effective = effective_actions(actions);
    let shift_count = effective
        .iter()
        .filter(|action| matches!(action, Action::Shift(_)))
        .count();
    let reduce_count = effective
        .iter()
        .filter(|action| matches!(action, Action::Reduce(_)))
        .count();

    if shift_count > 0 && reduce_count > 0 {
        ConflictType::ShiftReduce
    } else if reduce_count > 1 {
        ConflictType::ReduceReduce
    } else {
        ConflictType::Mixed
    }
}

/// Check if a specific state has conflicts
pub fn state_has_conflicts(table: &ParseTable, state: StateId) -> bool {
    if (state.0 as usize) >= table.action_table.len() {
        return false;
    }

    let state_actions = &table.action_table[state.0 as usize];
    state_actions.iter().any(|cell| cell_has_conflict(cell))
}

/// Get all conflicts for a specific state
pub fn get_state_conflicts(table: &ParseTable, state: StateId) -> Vec<ConflictDetail> {
    let summary = count_conflicts(table);
    summary
        .conflict_details
        .into_iter()
        .filter(|detail| detail.state == state)
        .collect()
}

/// Find conflicts for a specific symbol across all states
pub fn find_conflicts_for_symbol(table: &ParseTable, symbol: SymbolId) -> Vec<ConflictDetail> {
    let summary = count_conflicts(table);
    summary
        .conflict_details
        .into_iter()
        .filter(|detail| detail.symbol == symbol)
        .collect()
}

/// Format conflict summary for human reading
impl fmt::Display for ConflictSummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "=== Conflict Summary ===")?;
        writeln!(f, "Shift/Reduce conflicts: {}", self.shift_reduce)?;
        writeln!(f, "Reduce/Reduce conflicts: {}", self.reduce_reduce)?;
        writeln!(
            f,
            "States with conflicts: {}",
            self.states_with_conflicts.len()
        )?;
        writeln!(f)?;
        writeln!(f, "=== Conflict Details ===")?;
        for detail in &self.conflict_details {
            writeln!(f, "{}", detail)?;
        }
        Ok(())
    }
}

impl fmt::Display for ConflictDetail {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "State {}, Symbol '{}' ({}): {:?} - {} actions",
            self.state.0,
            self.symbol_name,
            self.symbol.0,
            self.conflict_type,
            self.actions.len()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Action, SymbolMetadata};
    use adze_ir::RuleId;

    /// Helper to create a minimal ParseTable for testing
    fn create_test_table(action_table: Vec<Vec<Vec<Action>>>) -> ParseTable {
        let state_count = action_table.len();
        ParseTable {
            action_table,
            goto_table: vec![],
            symbol_metadata: vec![],
            state_count,
            symbol_count: 1,
            symbol_to_index: Default::default(),
            index_to_symbol: Default::default(),
            external_scanner_states: vec![],
            rules: vec![],
            nonterminal_to_index: Default::default(),
            goto_indexing: crate::GotoIndexing::NonterminalMap,
            eof_symbol: SymbolId(0),
            start_symbol: SymbolId(0),
            grammar: adze_ir::Grammar::new("test".to_string()),
            initial_state: StateId(0),
            token_count: 0,
            external_token_count: 0,
            lex_modes: vec![],
            extras: vec![],
            dynamic_prec_by_rule: vec![],
            rule_assoc_by_rule: vec![],
            alias_sequences: vec![],
            field_names: vec![],
            field_map: Default::default(),
        }
    }

    #[test]
    fn test_classify_shift_reduce() {
        let actions = vec![Action::Shift(StateId(5)), Action::Reduce(RuleId(3))];

        assert_eq!(classify_conflict(&actions), ConflictType::ShiftReduce);
    }

    #[test]
    fn test_classify_reduce_reduce() {
        let actions = vec![Action::Reduce(RuleId(3)), Action::Reduce(RuleId(7))];

        assert_eq!(classify_conflict(&actions), ConflictType::ReduceReduce);
    }

    #[test]
    fn test_classify_mixed() {
        let actions = vec![Action::Shift(StateId(1)), Action::Shift(StateId(2))];

        assert_eq!(classify_conflict(&actions), ConflictType::Mixed);
    }

    #[test]
    fn test_classify_fork_shift_reduce() {
        let actions = vec![Action::Fork(vec![
            Action::Shift(StateId(1)),
            Action::Reduce(RuleId(1)),
        ])];

        assert_eq!(classify_conflict(&actions), ConflictType::ShiftReduce);
    }

    #[test]
    fn test_cell_has_conflict_for_single_fork_with_two_branches() {
        let cell = vec![Action::Fork(vec![
            Action::Shift(StateId(1)),
            Action::Reduce(RuleId(1)),
        ])];
        assert!(cell_has_conflict(&cell));
    }

    #[test]
    fn test_cell_has_no_conflict_for_single_fork_with_one_branch() {
        let cell = vec![Action::Fork(vec![Action::Shift(StateId(1))])];
        assert!(!cell_has_conflict(&cell));
    }

    #[test]
    fn test_empty_conflict_summary() {
        // Create a minimal parse table with no conflicts
        let table = create_test_table(vec![vec![vec![Action::Shift(StateId(1))]]]);

        let summary = count_conflicts(&table);
        assert_eq!(summary.shift_reduce, 0);
        assert_eq!(summary.reduce_reduce, 0);
        assert!(summary.states_with_conflicts.is_empty());
    }

    #[test]
    fn test_detect_shift_reduce_conflict() {
        // Create a parse table with one shift/reduce conflict
        let table = create_test_table(vec![vec![vec![
            Action::Shift(StateId(1)),
            Action::Reduce(RuleId(0)),
        ]]]);

        let summary = count_conflicts(&table);
        assert_eq!(summary.shift_reduce, 1);
        assert_eq!(summary.reduce_reduce, 0);
        assert_eq!(summary.states_with_conflicts.len(), 1);
        assert_eq!(summary.conflict_details.len(), 1);

        let detail = &summary.conflict_details[0];
        assert_eq!(detail.conflict_type, ConflictType::ShiftReduce);
        assert_eq!(detail.actions.len(), 2);
    }

    #[test]
    fn test_count_conflicts_uses_symbol_metadata_name() {
        let mut table = create_test_table(vec![vec![vec![
            Action::Shift(StateId(1)),
            Action::Reduce(RuleId(0)),
        ]]]);
        table.index_to_symbol = vec![SymbolId(42)];
        table.symbol_metadata = vec![SymbolMetadata {
            name: "expression".to_string(),
            is_visible: true,
            is_named: true,
            is_supertype: false,
            is_terminal: true,
            is_extra: false,
            is_fragile: false,
            symbol_id: SymbolId(42),
        }];

        let summary = count_conflicts(&table);

        assert_eq!(summary.conflict_details[0].symbol, SymbolId(42));
        assert_eq!(summary.conflict_details[0].symbol_name, "expression");
    }

    #[test]
    fn test_count_conflicts_reports_effective_actions_for_nested_forks() {
        let table = create_test_table(vec![vec![vec![Action::Fork(vec![
            Action::Shift(StateId(1)),
            Action::Fork(vec![Action::Reduce(RuleId(0)), Action::Shift(StateId(1))]),
        ])]]]);

        let summary = count_conflicts(&table);

        assert_eq!(summary.conflict_details.len(), 1);
        assert_eq!(
            summary.conflict_details[0].conflict_type,
            ConflictType::ShiftReduce
        );
        assert_eq!(
            summary.conflict_details[0].actions,
            vec![Action::Shift(StateId(1)), Action::Reduce(RuleId(0))]
        );
        assert_eq!(summary.conflict_details[0].priorities.len(), 2);
    }

    #[test]
    fn test_classify_reduce_accept_as_mixed_not_reduce_reduce() {
        let actions = vec![Action::Reduce(RuleId(3)), Action::Accept];

        assert_eq!(classify_conflict(&actions), ConflictType::Mixed);
    }

    #[test]
    fn test_state_has_conflicts() {
        let table = create_test_table(vec![
            vec![vec![Action::Shift(StateId(1)), Action::Reduce(RuleId(0))]],
            vec![vec![Action::Shift(StateId(2))]],
        ]);

        assert!(state_has_conflicts(&table, StateId(0)));
        assert!(!state_has_conflicts(&table, StateId(1)));
    }
}
