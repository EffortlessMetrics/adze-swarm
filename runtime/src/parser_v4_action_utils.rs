use adze_glr_core::{Action, ParseTable};

const ACCEPT_PRIORITY: i32 = 3_000_000;
const SHIFT_PRIORITY: i32 = 2_000_000;
const REDUCE_BASE_PRIORITY: i32 = 1_500_000;

pub(crate) fn action_priority(parse_table: &ParseTable, action: &Action) -> i32 {
    match action {
        Action::Accept => accept_priority(),
        Action::Reduce(rule_id) => reduce_priority(parse_table, rule_id.0 as usize),
        Action::Shift(_) => SHIFT_PRIORITY,
        _ => 0,
    }
}

fn accept_priority() -> i32 {
    ACCEPT_PRIORITY
}

fn reduce_priority(parse_table: &ParseTable, rule_index: usize) -> i32 {
    let precedence = reduce_precedence(parse_table, rule_index);
    if precedence > 0 {
        SHIFT_PRIORITY + precedence
    } else {
        REDUCE_BASE_PRIORITY + precedence
    }
}

fn reduce_precedence(parse_table: &ParseTable, rule_index: usize) -> i32 {
    let dynamic_precedence = dynamic_rule_precedence(parse_table, rule_index);
    let assoc_bias = rule_associativity_bias(parse_table, rule_index);
    dynamic_precedence.saturating_add(assoc_bias)
}

fn dynamic_rule_precedence(parse_table: &ParseTable, rule_index: usize) -> i32 {
    parse_table
        .dynamic_prec_by_rule
        .get(rule_index)
        .copied()
        .map(i32::from)
        .unwrap_or_default()
}

fn rule_associativity_bias(parse_table: &ParseTable, rule_index: usize) -> i32 {
    parse_table
        .rule_assoc_by_rule
        .get(rule_index)
        .copied()
        .map(i32::from)
        .unwrap_or_default()
}
