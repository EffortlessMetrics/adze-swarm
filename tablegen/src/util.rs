#[inline]
pub(crate) fn unexpected_action(action: &adze_glr_core::Action, where_: &str) {
    debug_assert!(
        matches!(
            action,
            adze_glr_core::Action::Shift(_)
                | adze_glr_core::Action::Reduce(_)
                | adze_glr_core::Action::Accept
                | adze_glr_core::Action::Error
                | adze_glr_core::Action::Recover
                | adze_glr_core::Action::Fork(_)
        ),
        "Unexpected action variant in {where_}: {action:?}"
    );
    // TODO: Add log-warns feature to Cargo.toml if needed
    // #[cfg(feature = "log-warns")]
    // {
    //     use std::sync::Once;
    //     static WARN: Once = Once::new();
    //     WARN.call_once(|| log::warn!("Unknown Action variant seen in {where_}; treating as error"));
    // }
}

#[cfg(test)]
mod tests {
    use super::*;
    use adze_glr_core::Action;
    use adze_ir::{RuleId, StateId};

    #[test]
    fn unexpected_action_known_variant_no_panic() {
        // Should not panic or log in debug; it only warns on unknown variants.
        unexpected_action(&Action::Error, "util-test");
        unexpected_action(&Action::Shift(StateId(0)), "util-test");
        unexpected_action(&Action::Reduce(RuleId(0)), "util-test");
        unexpected_action(&Action::Accept, "util-test");
    }

    #[test]
    fn unexpected_action_recover_variant_no_panic() {
        unexpected_action(&Action::Recover, "util-test");
    }

    #[test]
    fn unexpected_action_fork_variant_no_panic() {
        let inner = vec![Action::Shift(StateId(1)), Action::Reduce(RuleId(2))];
        unexpected_action(&Action::Fork(inner), "util-test");
    }
}
