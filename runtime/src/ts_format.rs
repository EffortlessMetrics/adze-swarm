//! Shared constants and helpers for Tree-sitter-compatible parsing tables.

use adze_glr_core::{Action, ParseTable};

/// Tree-sitter action type tags.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum TSActionTag {
    /// Error action tag.
    Error = 0,
    /// Shift action tag.
    Shift = 1,
    /// Recover action tag.
    Recover = 2,
    /// Reduce action tag.
    Reduce = 3,
    /// Accept action tag.
    Accept = 4,
}

/// Friendly name for [`TSActionTag`]; both names denote the same type.
pub type ActionTag = TSActionTag;

/// Choose a single action from a GLR cell deterministically.
///
/// Priority is `Accept > Shift > Reduce > Error`, matching existing runtime
/// behavior.
#[must_use]
pub fn choose_action(cell: &[Action]) -> Option<Action> {
    if cell.is_empty() {
        return None;
    }

    if let Some(a) = cell.iter().find(|a| matches!(a, Action::Accept)) {
        return Some(a.clone());
    }
    if let Some(a) = cell.iter().find(|a| matches!(a, Action::Shift(_))) {
        return Some(a.clone());
    }
    if let Some(a) = cell.iter().find(|a| matches!(a, Action::Reduce(_))) {
        return Some(a.clone());
    }
    Some(Action::Error)
}

/// Choose a single action from a GLR cell using precedence-aware scoring.
///
/// This is used when grammar/runtime policy needs tie-breaking with dynamic
/// rule precedence.
#[must_use]
pub fn choose_action_with_precedence(cell: &[Action], parse_table: &ParseTable) -> Option<Action> {
    if cell.is_empty() {
        return None;
    }

    let mut sorted = cell.to_vec();
    sorted.sort_by_key(|a| -action_priority(a, parse_table));
    sorted.first().cloned()
}

#[inline]
fn action_priority(action: &Action, parse_table: &ParseTable) -> i32 {
    use Action::*;

    if matches!(action, Accept) {
        return 3_000_000;
    }

    let mut prec = 0i32;
    if let Reduce(rid) = action {
        if (rid.0 as usize) < parse_table.dynamic_prec_by_rule.len() {
            prec = parse_table.dynamic_prec_by_rule[rid.0 as usize] as i32;
        }

        if (rid.0 as usize) < parse_table.rule_assoc_by_rule.len() {
            prec = prec.saturating_add(parse_table.rule_assoc_by_rule[rid.0 as usize] as i32);
        }

        if prec > 0 {
            return 2_000_000 + prec;
        }
        return 1_500_000 + prec;
    }

    if matches!(action, Shift(_)) {
        return 2_000_000;
    }

    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_tag_values_match_tree_sitter_abi() {
        assert_eq!(TSActionTag::Error as u8, 0);
        assert_eq!(TSActionTag::Shift as u8, 1);
        assert_eq!(TSActionTag::Recover as u8, 2);
        assert_eq!(TSActionTag::Reduce as u8, 3);
        assert_eq!(TSActionTag::Accept as u8, 4);
    }

    #[test]
    fn choose_action_priority_order_is_stable() {
        use adze_glr_core::{RuleId, StateId};

        let cell = vec![
            Action::Error,
            Action::Reduce(RuleId(0)),
            Action::Shift(StateId(1)),
            Action::Accept,
        ];
        assert_eq!(choose_action(&cell), Some(Action::Accept));

        let cell = vec![
            Action::Error,
            Action::Reduce(RuleId(0)),
            Action::Shift(StateId(1)),
        ];
        assert_eq!(choose_action(&cell), Some(Action::Shift(StateId(1))));

        let cell = vec![Action::Error, Action::Reduce(RuleId(0))];
        assert_eq!(choose_action(&cell), Some(Action::Reduce(RuleId(0))));

        let cell = vec![Action::Error];
        assert_eq!(choose_action(&cell), Some(Action::Error));
    }
}
