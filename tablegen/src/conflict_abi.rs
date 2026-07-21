//! GLR conflict action helpers for ABI emission.
//!
//! Tree-sitter small tables represent GLR conflicts as duplicate `(symbol, action)`
//! pairs. `Action::Fork` wrappers must be flattened before u16 encoding.

use crate::TableGenError;
use adze_glr_core::Action;
use adze_glr_core::conflict_inspection::effective_actions;

/// Returns true when `action` contains a nested `Action::Fork` beyond the ABI bound.
#[must_use]
pub(crate) fn action_contains_nested_fork(action: &Action) -> bool {
    match action {
        Action::Fork(inner) => inner.iter().any(|child| matches!(child, Action::Fork(_))),
        _ => false,
    }
}

/// Reject action cells whose fork nesting exceeds the ABI bound.
pub(crate) fn validate_abi_cell(action_cell: &[Action]) -> Result<(), TableGenError> {
    if action_cell.iter().any(action_contains_nested_fork) {
        return Err(TableGenError::Compression(
            "nested Action::Fork values must be flattened before ABI emission".to_string(),
        ));
    }
    Ok(())
}

/// Encode one leaf parse action for Tree-sitter small-table ABI emission.
///
/// `Action::Fork` must be flattened with [`effective_actions`] before calling this.
pub(crate) fn encode_leaf_action(action: &Action) -> Result<u16, String> {
    match action {
        Action::Shift(state) => Ok(state.0),
        Action::Reduce(rule) => Ok(0x8000 | (rule.0 + 1)),
        Action::Accept => Ok(0xFFFF),
        Action::Error => Ok(0),
        Action::Recover => Ok(0xFFFD),
        Action::Fork(_) => Err(
            "Fork actions must be flattened with effective_actions before ABI encoding".to_string(),
        ),
        other => {
            crate::util::unexpected_action(other, "encode_leaf_action");
            Err(format!(
                "unsupported action variant for ABI encoding: {other:?}"
            ))
        }
    }
}

/// Expand one action cell into deterministic leaf actions suitable for ABI emission.
#[must_use]
pub(crate) fn abi_leaf_actions(action_cell: &[Action]) -> Vec<Action> {
    let mut out = Vec::new();
    for action in action_cell {
        match action {
            Action::Error => {}
            Action::Fork(inner) => out.extend(effective_actions(inner)),
            other => out.push(other.clone()),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use adze_ir::{RuleId, StateId};

    #[test]
    fn fork_flattens_to_duplicate_symbol_pairs() {
        let cell = vec![Action::Fork(vec![
            Action::Shift(StateId(1)),
            Action::Reduce(RuleId(0)),
        ])];
        let leaves = abi_leaf_actions(&cell);
        assert_eq!(
            leaves,
            vec![Action::Shift(StateId(1)), Action::Reduce(RuleId(0))]
        );
    }

    #[test]
    fn encode_leaf_action_rejects_fork() {
        let fork = Action::Fork(vec![Action::Shift(StateId(1))]);
        assert!(encode_leaf_action(&fork).is_err());
    }

    #[test]
    fn validate_abi_cell_rejects_nested_fork() {
        let nested = vec![Action::Fork(vec![Action::Fork(vec![
            Action::Shift(StateId(1)),
            Action::Reduce(RuleId(0)),
        ])])];
        assert!(validate_abi_cell(&nested).is_err());
    }

    #[test]
    fn validate_abi_cell_accepts_single_level_fork() {
        let cell = vec![Action::Fork(vec![
            Action::Shift(StateId(1)),
            Action::Reduce(RuleId(0)),
        ])];
        assert!(validate_abi_cell(&cell).is_ok());
    }
}
