use crate::{Action, ActionCell, action_utils::action_eq};
use std::collections::BTreeMap;

pub(super) fn normalize_action_table(action_table: &mut Vec<Vec<ActionCell>>) {
    for row in action_table.iter_mut() {
        for cell in row.iter_mut() {
            normalize_action_cell(cell);
        }
    }
}

fn normalize_action_cell(cell: &mut ActionCell) {
    for action in cell.iter_mut() {
        normalize_action(action);
    }
    cell.sort_by_key(action_sort_key);
    cell.dedup();
}

pub(crate) fn normalize_action(action: &mut Action) {
    if let Action::Fork(inner) = action {
        for inner_action in inner.iter_mut() {
            normalize_action(inner_action);
        }
        inner.sort_by_key(action_sort_key);
        inner.dedup();
    }
}

fn action_sort_key(action: &Action) -> (u8, u16, u16, u16) {
    match action {
        Action::Shift(s) => (0, s.0, 0, 0),
        Action::Reduce(r) => (1, r.0, 0, 0),
        Action::Accept => (2, 0, 0, 0),
        Action::Error => (3, 0, 0, 0),
        Action::Recover => (4, 0, 0, 0),
        Action::Fork(inner) => {
            let first = inner.first().map(action_sort_key).unwrap_or((0, 0, 0, 0));
            (5, first.1, first.2, inner.len() as u16)
        }
    }
}

pub(super) fn add_action_with_conflict(
    action_table: &mut Vec<Vec<ActionCell>>,
    conflicts_by_state: &mut BTreeMap<(usize, usize), Vec<Action>>,
    state_idx: usize,
    symbol_idx: usize,
    new_action: Action,
) {
    if state_idx >= action_table.len() || symbol_idx >= action_table[0].len() {
        panic!(
            "Index out of bounds in add_action_with_conflict: state_idx={}, symbol_idx={}, table_size={}x{}",
            state_idx,
            symbol_idx,
            action_table.len(),
            if action_table.is_empty() {
                0
            } else {
                action_table[0].len()
            }
        );
    }

    let current_cell = &mut action_table[state_idx][symbol_idx];

    if !current_cell.iter().any(|a| action_eq(a, &new_action)) {
        current_cell.push(new_action.clone());

        if current_cell.len() > 1 {
            let entry = conflicts_by_state
                .entry((state_idx, symbol_idx))
                .or_default();
            *entry = current_cell.clone();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use adze_ir::{RuleId, StateId};

    #[test]
    fn normalize_action_leaves_shift_unchanged() {
        let mut action = Action::Shift(StateId(7));
        normalize_action(&mut action);
        assert_eq!(action, Action::Shift(StateId(7)));
    }

    #[test]
    fn normalize_action_leaves_reduce_unchanged() {
        let mut action = Action::Reduce(RuleId(3));
        normalize_action(&mut action);
        assert_eq!(action, Action::Reduce(RuleId(3)));
    }

    #[test]
    fn normalize_action_leaves_accept_error_recover_unchanged() {
        let mut accept = Action::Accept;
        normalize_action(&mut accept);
        assert_eq!(accept, Action::Accept);

        let mut error = Action::Error;
        normalize_action(&mut error);
        assert_eq!(error, Action::Error);

        let mut recover = Action::Recover;
        normalize_action(&mut recover);
        assert_eq!(recover, Action::Recover);
    }

    #[test]
    fn normalize_action_sorts_fork_contents() {
        // Sort key tier order: Shift(0) < Reduce(1) < Accept(2) < Error(3) < Recover(4) < Fork(5)
        let mut action = Action::Fork(vec![
            Action::Recover,
            Action::Accept,
            Action::Reduce(RuleId(2)),
            Action::Shift(StateId(5)),
            Action::Error,
        ]);
        normalize_action(&mut action);
        assert_eq!(
            action,
            Action::Fork(vec![
                Action::Shift(StateId(5)),
                Action::Reduce(RuleId(2)),
                Action::Accept,
                Action::Error,
                Action::Recover,
            ])
        );
    }

    #[test]
    fn normalize_action_sorts_within_same_tier_by_id() {
        let mut action = Action::Fork(vec![
            Action::Shift(StateId(10)),
            Action::Shift(StateId(3)),
            Action::Shift(StateId(7)),
        ]);
        normalize_action(&mut action);
        assert_eq!(
            action,
            Action::Fork(vec![
                Action::Shift(StateId(3)),
                Action::Shift(StateId(7)),
                Action::Shift(StateId(10)),
            ])
        );
    }

    #[test]
    fn normalize_action_dedups_fork_contents() {
        let mut action = Action::Fork(vec![
            Action::Shift(StateId(1)),
            Action::Shift(StateId(1)),
            Action::Reduce(RuleId(2)),
            Action::Reduce(RuleId(2)),
            Action::Accept,
        ]);
        normalize_action(&mut action);
        assert_eq!(
            action,
            Action::Fork(vec![
                Action::Shift(StateId(1)),
                Action::Reduce(RuleId(2)),
                Action::Accept,
            ])
        );
    }

    #[test]
    fn normalize_action_recurses_into_nested_forks() {
        let mut action = Action::Fork(vec![
            Action::Fork(vec![Action::Reduce(RuleId(5)), Action::Shift(StateId(1))]),
            Action::Shift(StateId(0)),
        ]);
        normalize_action(&mut action);
        // Inner fork is sorted; the outer ordering pushes the inner Fork
        // (tier 5) past the bare Shift (tier 0).
        assert_eq!(
            action,
            Action::Fork(vec![
                Action::Shift(StateId(0)),
                Action::Fork(vec![Action::Shift(StateId(1)), Action::Reduce(RuleId(5))]),
            ])
        );
    }

    #[test]
    fn normalize_action_table_normalizes_every_cell() {
        let cell_a: ActionCell = vec![Action::Fork(vec![
            Action::Reduce(RuleId(4)),
            Action::Shift(StateId(2)),
            Action::Shift(StateId(2)),
        ])];
        let cell_b: ActionCell = vec![
            Action::Shift(StateId(9)),
            Action::Shift(StateId(9)),
            Action::Reduce(RuleId(1)),
        ];
        let mut table: Vec<Vec<ActionCell>> = vec![vec![cell_a, cell_b]];

        normalize_action_table(&mut table);

        // Cell A: inner Fork was sorted + deduped.
        assert_eq!(
            table[0][0],
            vec![Action::Fork(vec![
                Action::Shift(StateId(2)),
                Action::Reduce(RuleId(4)),
            ])]
        );
        // Cell B: dup removed and entries sorted by tier/id.
        assert_eq!(
            table[0][1],
            vec![Action::Shift(StateId(9)), Action::Reduce(RuleId(1))]
        );
    }

    #[test]
    fn add_action_inserts_when_cell_is_empty() {
        let mut table: Vec<Vec<ActionCell>> = vec![vec![Vec::new(), Vec::new()]];
        let mut conflicts: BTreeMap<(usize, usize), Vec<Action>> = BTreeMap::new();

        add_action_with_conflict(&mut table, &mut conflicts, 0, 0, Action::Shift(StateId(1)));
        assert_eq!(table[0][0], vec![Action::Shift(StateId(1))]);
        // Single-entry cells do NOT register a conflict.
        assert!(conflicts.is_empty());
    }

    #[test]
    fn add_action_skips_duplicate_shift() {
        let mut table: Vec<Vec<ActionCell>> = vec![vec![vec![Action::Shift(StateId(1))]]];
        let mut conflicts: BTreeMap<(usize, usize), Vec<Action>> = BTreeMap::new();

        add_action_with_conflict(&mut table, &mut conflicts, 0, 0, Action::Shift(StateId(1)));
        assert_eq!(table[0][0], vec![Action::Shift(StateId(1))]);
        assert!(conflicts.is_empty());
    }

    #[test]
    fn add_action_skips_duplicate_reduce() {
        let mut table: Vec<Vec<ActionCell>> = vec![vec![vec![Action::Reduce(RuleId(2))]]];
        let mut conflicts: BTreeMap<(usize, usize), Vec<Action>> = BTreeMap::new();

        add_action_with_conflict(&mut table, &mut conflicts, 0, 0, Action::Reduce(RuleId(2)));
        assert_eq!(table[0][0], vec![Action::Reduce(RuleId(2))]);
        assert!(conflicts.is_empty());
    }

    #[test]
    fn add_action_skips_duplicate_accept_and_error() {
        let mut table: Vec<Vec<ActionCell>> = vec![vec![vec![Action::Accept]]];
        let mut conflicts: BTreeMap<(usize, usize), Vec<Action>> = BTreeMap::new();
        add_action_with_conflict(&mut table, &mut conflicts, 0, 0, Action::Accept);
        assert_eq!(table[0][0], vec![Action::Accept]);
        assert!(conflicts.is_empty());

        let mut table: Vec<Vec<ActionCell>> = vec![vec![vec![Action::Error]]];
        let mut conflicts: BTreeMap<(usize, usize), Vec<Action>> = BTreeMap::new();
        add_action_with_conflict(&mut table, &mut conflicts, 0, 0, Action::Error);
        assert_eq!(table[0][0], vec![Action::Error]);
        assert!(conflicts.is_empty());
    }

    #[test]
    fn add_action_records_conflict_when_cell_grows_past_one() {
        let mut table: Vec<Vec<ActionCell>> = vec![vec![vec![Action::Shift(StateId(1))]]];
        let mut conflicts: BTreeMap<(usize, usize), Vec<Action>> = BTreeMap::new();

        add_action_with_conflict(&mut table, &mut conflicts, 0, 0, Action::Reduce(RuleId(2)));

        assert_eq!(
            table[0][0],
            vec![Action::Shift(StateId(1)), Action::Reduce(RuleId(2))]
        );
        let recorded = conflicts.get(&(0, 0)).expect("conflict must be recorded");
        assert_eq!(
            *recorded,
            vec![Action::Shift(StateId(1)), Action::Reduce(RuleId(2))]
        );
    }

    #[test]
    fn add_action_extends_existing_conflict_entry() {
        let mut table: Vec<Vec<ActionCell>> = vec![vec![vec![Action::Shift(StateId(1))]]];
        let mut conflicts: BTreeMap<(usize, usize), Vec<Action>> = BTreeMap::new();

        add_action_with_conflict(&mut table, &mut conflicts, 0, 0, Action::Reduce(RuleId(2)));
        add_action_with_conflict(&mut table, &mut conflicts, 0, 0, Action::Reduce(RuleId(3)));

        // Both reduces should land in the cell along with the original shift.
        assert_eq!(
            table[0][0],
            vec![
                Action::Shift(StateId(1)),
                Action::Reduce(RuleId(2)),
                Action::Reduce(RuleId(3)),
            ]
        );
        // Conflicts entry mirrors the current cell after the second add.
        let recorded = conflicts.get(&(0, 0)).expect("conflict must be recorded");
        assert_eq!(
            *recorded,
            vec![
                Action::Shift(StateId(1)),
                Action::Reduce(RuleId(2)),
                Action::Reduce(RuleId(3)),
            ]
        );
    }

    #[test]
    fn add_action_treats_equal_forks_as_duplicates() {
        let existing = Action::Fork(vec![Action::Shift(StateId(1)), Action::Reduce(RuleId(2))]);
        let mut table: Vec<Vec<ActionCell>> = vec![vec![vec![existing.clone()]]];
        let mut conflicts: BTreeMap<(usize, usize), Vec<Action>> = BTreeMap::new();

        // Same fork content => treated as equal, so no push.
        add_action_with_conflict(&mut table, &mut conflicts, 0, 0, existing.clone());
        assert_eq!(table[0][0], vec![existing]);
        assert!(conflicts.is_empty());
    }

    #[test]
    fn add_action_distinguishes_fork_from_shift() {
        let fork = Action::Fork(vec![Action::Shift(StateId(1))]);
        let mut table: Vec<Vec<ActionCell>> = vec![vec![vec![fork.clone()]]];
        let mut conflicts: BTreeMap<(usize, usize), Vec<Action>> = BTreeMap::new();

        // A bare Shift is not equal to a Fork containing the same Shift.
        add_action_with_conflict(&mut table, &mut conflicts, 0, 0, Action::Shift(StateId(1)));
        assert_eq!(table[0][0], vec![fork.clone(), Action::Shift(StateId(1))]);
        let recorded = conflicts.get(&(0, 0)).expect("conflict must be recorded");
        assert_eq!(*recorded, vec![fork, Action::Shift(StateId(1))]);
    }

    #[test]
    fn add_action_distinguishes_forks_of_different_length() {
        let short_fork = Action::Fork(vec![Action::Shift(StateId(1))]);
        let long_fork = Action::Fork(vec![Action::Shift(StateId(1)), Action::Reduce(RuleId(2))]);
        let mut table: Vec<Vec<ActionCell>> = vec![vec![vec![short_fork.clone()]]];
        let mut conflicts: BTreeMap<(usize, usize), Vec<Action>> = BTreeMap::new();

        add_action_with_conflict(&mut table, &mut conflicts, 0, 0, long_fork.clone());
        assert_eq!(table[0][0], vec![short_fork, long_fork]);
        assert!(conflicts.contains_key(&(0, 0)));
    }

    #[test]
    #[should_panic(expected = "Index out of bounds in add_action_with_conflict")]
    fn add_action_panics_on_state_out_of_bounds() {
        let mut table: Vec<Vec<ActionCell>> = vec![vec![Vec::new()]];
        let mut conflicts: BTreeMap<(usize, usize), Vec<Action>> = BTreeMap::new();
        add_action_with_conflict(&mut table, &mut conflicts, 5, 0, Action::Accept);
    }

    #[test]
    #[should_panic(expected = "Index out of bounds in add_action_with_conflict")]
    fn add_action_panics_on_symbol_out_of_bounds() {
        let mut table: Vec<Vec<ActionCell>> = vec![vec![Vec::new()]];
        let mut conflicts: BTreeMap<(usize, usize), Vec<Action>> = BTreeMap::new();
        add_action_with_conflict(&mut table, &mut conflicts, 0, 5, Action::Accept);
    }
}
