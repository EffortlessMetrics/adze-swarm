use crate::{Action, LexMode};
use std::collections::BTreeMap;

/// Assign one internal mode per usable-lookahead signature, matching lexing.
pub(crate) fn build_lex_modes(
    action_table: &[Vec<Vec<Action>>],
    external_scanner_states: &[Vec<bool>],
) -> Vec<LexMode> {
    build_lex_modes_matching(
        action_table,
        external_scanner_states,
        Action::uses_lookahead,
    )
}

/// Preserve the historical direct-Shift-only public fixture contract.
pub(crate) fn build_shift_lex_modes(
    action_table: &[Vec<Vec<Action>>],
    external_scanner_states: &[Vec<bool>],
) -> Vec<LexMode> {
    build_lex_modes_matching(action_table, external_scanner_states, |action| {
        matches!(action, Action::Shift(_))
    })
}

fn build_lex_modes_matching(
    action_table: &[Vec<Vec<Action>>],
    external_scanner_states: &[Vec<bool>],
    eligible: impl Fn(&Action) -> bool,
) -> Vec<LexMode> {
    let mut signature_to_lex_state: BTreeMap<Vec<u16>, u16> = BTreeMap::new();
    let mut next_lex_state = 0u16;

    action_table
        .iter()
        .zip(external_scanner_states.iter())
        .map(|(row, ext_states)| {
            let mut lookahead = row
                .iter()
                .enumerate()
                .filter_map(|(idx, cell)| cell.iter().any(&eligible).then_some(idx as u16))
                .collect::<Vec<_>>();
            lookahead.sort_unstable();

            let lex_state = *signature_to_lex_state.entry(lookahead).or_insert_with(|| {
                let id = next_lex_state;
                next_lex_state = next_lex_state.saturating_add(1);
                id
            });

            let external_lex_state =
                ext_states
                    .iter()
                    .enumerate()
                    .fold(
                        0u16,
                        |mask, (idx, active)| {
                            if *active { mask | (1u16 << idx) } else { mask }
                        },
                    );

            LexMode {
                lex_state,
                external_lex_state,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use adze_ir::StateId;

    #[test]
    fn distinct_shiftable_terminal_sets_receive_distinct_lex_states() {
        let action_table = vec![
            vec![
                vec![],
                vec![Action::Shift(StateId(1))],
                vec![Action::Shift(StateId(2))],
            ],
            vec![vec![], vec![Action::Shift(StateId(2))], vec![]],
            vec![vec![], vec![], vec![Action::Accept]],
        ];
        let external_scanner_states = vec![vec![false; 0]; 3];

        let modes = build_lex_modes(&action_table, &external_scanner_states);
        assert_eq!(modes.len(), 3);
        assert_eq!(modes[0].lex_state, 0);
        assert_eq!(modes[1].lex_state, 1);
        assert_eq!(modes[2].lex_state, 2);
    }

    #[test]
    fn external_scanner_validity_is_packed_into_lex_mode_mask() {
        let action_table = vec![vec![vec![]]];
        let external_scanner_states = vec![vec![true, false, true]];

        let modes = build_lex_modes(&action_table, &external_scanner_states);
        assert_eq!(modes[0].external_lex_state, 0b101);
    }

    #[test]
    fn test_usable_modes_follow_reductions_acceptance_and_recursive_forks() {
        use adze_ir::RuleId;
        let shift = vec![Action::Shift(StateId(1))];
        let rows = vec![
            vec![vec![Action::Error], vec![Action::Error], shift.clone()],
            vec![
                vec![Action::Error],
                vec![Action::Reduce(RuleId(0))],
                shift.clone(),
            ],
            vec![vec![Action::Error], vec![Action::Accept], shift.clone()],
            vec![
                vec![Action::Error],
                vec![Action::Fork(vec![
                    Action::Recover,
                    Action::Fork(vec![Action::Error, Action::Reduce(RuleId(0))]),
                ])],
                shift.clone(),
            ],
            vec![
                vec![Action::Error],
                vec![Action::Fork(vec![Action::Error, Action::Recover])],
                shift.clone(),
            ],
            vec![vec![Action::Error]; 3],
            vec![
                vec![Action::Recover],
                vec![Action::Fork(vec![])],
                vec![Action::Error],
            ],
            vec![
                vec![Action::Error],
                vec![Action::Fork(vec![Action::Fork(vec![Action::Shift(
                    StateId(1),
                )])])],
                shift,
            ],
        ];
        let modes = build_lex_modes(&rows, &vec![vec![]; rows.len()]);
        assert_ne!(modes[0].lex_state, modes[1].lex_state);
        for state in [2, 3, 7] {
            assert_eq!(modes[state].lex_state, modes[1].lex_state);
        }
        assert_eq!(modes[4].lex_state, modes[0].lex_state);
        assert_eq!(modes[5].lex_state, modes[6].lex_state);
        assert_ne!(modes[5].lex_state, modes[0].lex_state);
    }

    #[test]
    fn test_shift_specific_public_modes_keep_their_original_contract() {
        use adze_ir::RuleId;
        let rows = vec![
            vec![vec![Action::Shift(StateId(1))], vec![Action::Error]],
            vec![
                vec![Action::Shift(StateId(1))],
                vec![Action::Reduce(RuleId(0))],
            ],
        ];
        let external = vec![vec![]; rows.len()];
        let shift = crate::build_lex_modes_from_shiftable_terminals(&rows, &external);
        assert_eq!(shift[0].lex_state, shift[1].lex_state);
        let lookahead = crate::build_lex_modes_from_usable_lookahead(&rows, &external);
        assert_ne!(lookahead[0].lex_state, lookahead[1].lex_state);
    }
}
