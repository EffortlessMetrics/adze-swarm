//! Custom assertion helpers for grammar and parse-table verification.
//!
//! These supplement the standard `assert!` / `assert_eq!` macros with
//! domain-specific checks that produce clear, actionable error messages.

use adze_glr_core::ParseTable;
use adze_ir::{Grammar, Symbol};
use std::fmt;

/// Fallible test helper result.
///
/// This keeps tests readable while allowing them to return `Result` instead of
/// using panic-family assertions for every precondition.
pub type TestResult<T = ()> = Result<T, String>;

/// Require `condition` to be true.
///
/// This is the fallible counterpart to `assert!`.
pub fn ensure(condition: bool, message: impl Into<String>) -> TestResult {
    if condition {
        Ok(())
    } else {
        Err(message.into())
    }
}

/// Require an [`Option`] to contain a value.
///
/// This is the fallible counterpart to `Option::expect`.
pub fn require_some<T>(value: Option<T>, message: impl Into<String>) -> TestResult<T> {
    value.ok_or_else(|| message.into())
}

/// Require a [`Result`] to be successful and attach a caller-facing context.
///
/// This is the fallible counterpart to `Result::expect`.
pub fn require_ok<T, E>(value: Result<T, E>, context: impl Into<String>) -> TestResult<T>
where
    E: fmt::Display,
{
    value.map_err(|error| {
        let context = context.into();
        if context.is_empty() {
            error.to_string()
        } else {
            format!("{context}: {error}")
        }
    })
}

// ---------------------------------------------------------------------------
// Grammar assertions
// ---------------------------------------------------------------------------

/// Assert that the grammar contains a rule whose LHS has the given name.
///
/// # Panics
///
/// Panics with a descriptive message listing all known rule names if no
/// matching rule is found.
pub fn assert_has_rule(grammar: &Grammar, rule_name: &str) {
    let found = grammar
        .rule_names
        .iter()
        .any(|(_, name)| name.as_str() == rule_name);
    assert!(
        found,
        "expected grammar to contain rule `{rule_name}`, but only found: {:?}",
        grammar
            .rule_names
            .values()
            .map(String::as_str)
            .collect::<Vec<_>>()
    );
}

/// Assert that the grammar contains a token with the given name.
pub fn assert_has_token(grammar: &Grammar, token_name: &str) {
    let found = grammar.tokens.values().any(|t| t.name == token_name);
    assert!(
        found,
        "expected grammar to contain token `{token_name}`, but only found: {:?}",
        grammar
            .tokens
            .values()
            .map(|t| t.name.as_str())
            .collect::<Vec<_>>()
    );
}

/// Assert that the grammar's start symbol resolves to the given name.
pub fn assert_start_symbol(grammar: &Grammar, expected: &str) {
    let start_id = grammar
        .start_symbol()
        .expect("grammar should have a start symbol");
    let actual = grammar
        .rule_names
        .get(&start_id)
        .map(String::as_str)
        .unwrap_or("<unnamed>");
    assert_eq!(
        actual, expected,
        "expected start symbol `{expected}`, got `{actual}`"
    );
}

/// Assert that the grammar has exactly `n` rules for the given non-terminal.
pub fn assert_rule_count(grammar: &Grammar, rule_name: &str, expected: usize) {
    let id = grammar
        .rule_names
        .iter()
        .find(|(_, name)| name.as_str() == rule_name)
        .map(|(id, _)| *id);

    let actual = id.and_then(|id| grammar.rules.get(&id)).map_or(0, Vec::len);

    assert_eq!(
        actual, expected,
        "expected {expected} rule(s) for `{rule_name}`, found {actual}"
    );
}

/// Assert that a specific rule has a production whose RHS matches the given terminal/non-terminal names.
pub fn assert_has_production(grammar: &Grammar, rule_name: &str, expected_rhs: &[&str]) {
    let id = grammar
        .rule_names
        .iter()
        .find(|(_, name)| name.as_str() == rule_name)
        .map(|(id, _)| *id)
        .unwrap_or_else(|| panic!("rule `{rule_name}` not found"));

    let rules = grammar
        .rules
        .get(&id)
        .unwrap_or_else(|| panic!("no productions for rule `{rule_name}`"));

    let rhs_matches = |rule: &adze_ir::Rule| -> bool {
        if rule.rhs.len() != expected_rhs.len() {
            return false;
        }
        rule.rhs.iter().zip(expected_rhs.iter()).all(|(sym, &exp)| {
            let sym_name = match sym {
                Symbol::Terminal(id) => grammar.tokens.get(id).map(|t| t.name.as_str()),
                Symbol::NonTerminal(id) => grammar.rule_names.get(id).map(String::as_str),
                Symbol::Epsilon => Some("ε"),
                _ => None,
            };
            sym_name == Some(exp)
        })
    };

    assert!(
        rules.iter().any(rhs_matches),
        "expected rule `{rule_name}` to have production {:?}",
        expected_rhs
    );
}

// ---------------------------------------------------------------------------
// Parse table assertions
// ---------------------------------------------------------------------------

/// Assert that the parse table has at least `min` states.
pub fn assert_min_states(table: &ParseTable, min: usize) {
    assert!(
        table.state_count >= min,
        "expected at least {min} states, got {}",
        table.state_count
    );
}

/// Assert that the parse table's action table has no completely empty rows
/// (every state should have at least one valid action).
pub fn assert_no_dead_states(table: &ParseTable) {
    for (i, row) in table.action_table.iter().enumerate() {
        let has_action = row.iter().any(|cell| !cell.is_empty());
        assert!(has_action, "state {i} has no actions (dead state)");
    }
}

/// Assert that the parse table dimensions are self-consistent.
///
/// This performs the same style of invariant checks as
/// `glr_test_support::assert_parse_table_invariants`, when available,
/// but can be called stand-alone.
pub fn assert_table_consistent(table: &ParseTable) {
    assert_eq!(
        table.action_table.len(),
        table.state_count,
        "action_table row count != state_count"
    );
    assert_eq!(
        table.goto_table.len(),
        table.state_count,
        "goto_table row count != state_count"
    );
    for (i, row) in table.action_table.iter().enumerate() {
        assert_eq!(
            row.len(),
            table.symbol_count,
            "action_table row {i} width mismatch"
        );
    }
    for (i, row) in table.goto_table.iter().enumerate() {
        assert_eq!(
            row.len(),
            table.symbol_count,
            "goto_table row {i} width mismatch"
        );
    }
}

// ---------------------------------------------------------------------------
// Assertion macros
// ---------------------------------------------------------------------------

/// Assert that parsing the given grammar + input produces a parse table
/// without errors.
///
/// ```ignore
/// assert_grammar_valid!(grammar);
/// ```
#[macro_export]
macro_rules! assert_grammar_valid {
    ($grammar:expr) => {{
        let table = $crate::grammar_helpers::build_parse_table(&$grammar);
        assert!(
            table.is_ok(),
            "grammar failed to produce a valid parse table: {}",
            table.unwrap_err()
        );
        table.unwrap()
    }};
}

/// Assert that the grammar is *invalid* (cannot produce a parse table).
///
/// ```ignore
/// assert_grammar_invalid!(grammar);
/// ```
#[macro_export]
macro_rules! assert_grammar_invalid {
    ($grammar:expr) => {{
        let table = $crate::grammar_helpers::build_parse_table(&$grammar);
        assert!(
            table.is_err(),
            "expected grammar to be invalid, but it produced a parse table with {} states",
            table.unwrap().state_count
        );
    }};
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grammar_helpers::{arithmetic_grammar, test_grammar, trivial_grammar};

    #[test]
    fn ensure_succeeds_when_condition_is_true() {
        assert_eq!(ensure(true, "must be true"), Ok(()));
    }

    #[test]
    fn ensure_returns_message_when_condition_is_false() {
        assert_eq!(
            ensure(false, "condition failed"),
            Err("condition failed".to_string())
        );
    }

    #[test]
    fn require_some_extracts_value() {
        assert_eq!(require_some(Some(42), "missing value"), Ok(42));
    }

    #[test]
    fn require_some_reports_missing_value() {
        assert_eq!(
            require_some::<u8>(None, "missing value"),
            Err("missing value".to_string())
        );
    }

    #[test]
    fn require_ok_extracts_success() {
        assert_eq!(
            require_ok::<_, &str>(Ok("value"), "parse should succeed"),
            Ok("value")
        );
    }

    #[test]
    fn require_ok_reports_context_and_error() {
        assert_eq!(
            require_ok::<(), _>(Err("bad token"), "parse should succeed"),
            Err("parse should succeed: bad token".to_string())
        );
    }

    #[test]
    fn test_assert_has_rule() {
        let g = arithmetic_grammar();
        assert_has_rule(&g, "expr");
    }

    #[test]
    #[should_panic(expected = "expected grammar to contain rule `nonexistent`")]
    fn test_assert_has_rule_fails() {
        let g = trivial_grammar();
        assert_has_rule(&g, "nonexistent");
    }

    #[test]
    fn test_assert_has_token() {
        let g = arithmetic_grammar();
        assert_has_token(&g, "NUMBER");
        assert_has_token(&g, "+");
    }

    #[test]
    fn test_assert_start_symbol() {
        let g = arithmetic_grammar();
        assert_start_symbol(&g, "expr");
    }

    #[test]
    fn test_assert_rule_count() {
        let g = test_grammar(&[("expr", &["NUMBER"]), ("expr", &["expr", "+", "expr"])]);
        assert_rule_count(&g, "expr", 2);
    }

    #[test]
    fn test_assert_has_production() {
        let g = test_grammar(&[("sum", &["NUMBER", "+", "NUMBER"])]);
        assert_has_production(&g, "sum", &["NUMBER", "+", "NUMBER"]);
    }

    #[test]
    fn test_assert_grammar_valid_macro() {
        let g = trivial_grammar();
        let _table = assert_grammar_valid!(g);
    }

    #[test]
    fn test_table_consistency() {
        let g = arithmetic_grammar();
        let table = crate::grammar_helpers::build_parse_table(&g).unwrap();
        assert_table_consistent(&table);
        assert_min_states(&table, 1);
    }
}
