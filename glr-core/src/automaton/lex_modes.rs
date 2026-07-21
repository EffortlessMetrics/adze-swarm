use crate::{Action, LexMode};
use std::collections::BTreeMap;

/// Assign one internal lex state per distinct shiftable-terminal signature and
/// pack external-scanner validity into `external_lex_state` bitmasks.
pub fn build_lex_modes(
    action_table: &[Vec<Vec<Action>>],
    external_scanner_states: &[Vec<bool>],
) -> Vec<LexMode> {
    let mut signature_to_lex_state: BTreeMap<Vec<u16>, u16> = BTreeMap::new();
    let mut next_lex_state = 0u16;

    action_table
        .iter()
        .zip(external_scanner_states.iter())
        .map(|(row, ext_states)| {
            let mut shiftable = row
                .iter()
                .enumerate()
                .filter_map(|(idx, cell)| {
                    cell.iter()
                        .any(|a| matches!(a, Action::Shift(_)))
                        .then_some(idx as u16)
                })
                .collect::<Vec<_>>();
            shiftable.sort_unstable();

            let lex_state = *signature_to_lex_state.entry(shiftable).or_insert_with(|| {
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
}
