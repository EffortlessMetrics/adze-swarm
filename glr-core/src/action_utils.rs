use crate::Action;

/// Return true when two parse-table actions represent the same parser operation.
pub(crate) fn action_eq(a: &Action, b: &Action) -> bool {
    match (a, b) {
        (Action::Shift(s1), Action::Shift(s2)) => s1 == s2,
        (Action::Reduce(r1), Action::Reduce(r2)) => r1 == r2,
        (Action::Accept, Action::Accept)
        | (Action::Error, Action::Error)
        | (Action::Recover, Action::Recover) => true,
        (Action::Fork(a1), Action::Fork(a2)) => {
            a1.len() == a2.len() && a1.iter().zip(a2).all(|(x, y)| action_eq(x, y))
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use adze_ir::{RuleId, StateId};

    #[test]
    fn action_eq_matches_scalar_actions_by_variant_and_id() {
        assert!(action_eq(
            &Action::Shift(StateId(1)),
            &Action::Shift(StateId(1))
        ));
        assert!(!action_eq(
            &Action::Shift(StateId(1)),
            &Action::Shift(StateId(2))
        ));
        assert!(action_eq(
            &Action::Reduce(RuleId(3)),
            &Action::Reduce(RuleId(3))
        ));
        assert!(action_eq(&Action::Accept, &Action::Accept));
        assert!(action_eq(&Action::Error, &Action::Error));
        assert!(action_eq(&Action::Recover, &Action::Recover));
        assert!(!action_eq(&Action::Accept, &Action::Recover));
    }

    #[test]
    fn action_eq_recurses_through_forks() {
        let left = Action::Fork(vec![
            Action::Shift(StateId(1)),
            Action::Fork(vec![Action::Reduce(RuleId(2)), Action::Recover]),
        ]);
        let same = Action::Fork(vec![
            Action::Shift(StateId(1)),
            Action::Fork(vec![Action::Reduce(RuleId(2)), Action::Recover]),
        ]);
        let different = Action::Fork(vec![
            Action::Shift(StateId(1)),
            Action::Fork(vec![Action::Reduce(RuleId(2))]),
        ]);

        assert!(action_eq(&left, &same));
        assert!(!action_eq(&left, &different));
    }
}
