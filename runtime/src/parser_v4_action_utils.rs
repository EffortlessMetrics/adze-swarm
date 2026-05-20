use adze_glr_core::{Action, ParseTable};

pub(crate) fn action_priority(parse_table: &ParseTable, action: &Action) -> i32 {
    use Action::*;

    if matches!(action, Accept) {
        return 3_000_000;
    }

    let mut prec = 0i32;
    if let Reduce(rid) = action {
        if (rid.0 as usize) < parse_table.dynamic_prec_by_rule.len() {
            prec = parse_table.dynamic_prec_by_rule[rid.0 as usize] as i32;
        }

        let assoc_bias = if (rid.0 as usize) < parse_table.rule_assoc_by_rule.len() {
            parse_table.rule_assoc_by_rule[rid.0 as usize] as i32
        } else {
            0
        };

        prec = prec.saturating_add(assoc_bias);

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
