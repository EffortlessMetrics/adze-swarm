//! Common `proptest` strategies for grammar-related types.
//!
//! These strategies produce random but structurally valid grammars, rule
//! names, token patterns, and symbol sequences that can be used in
//! property-based tests across all crates.

use adze_ir::builder::GrammarBuilder;
use adze_ir::{Associativity, Grammar, SymbolId};
use proptest::prelude::*;

// ---------------------------------------------------------------------------
// Primitive strategies
// ---------------------------------------------------------------------------

/// Strategy that generates valid identifier-style names (e.g. `"expr"`, `"a2b"`).
pub fn ident_strategy() -> impl Strategy<Value = String> {
    prop::string::string_regex("[a-z][a-z0-9_]{0,7}")
        .unwrap()
        .prop_filter("non-empty", |s| !s.is_empty())
}

/// Strategy that generates UPPER_CASE token names (e.g. `"NUMBER"`, `"ID"`).
pub fn token_name_strategy() -> impl Strategy<Value = String> {
    prop::string::string_regex("[A-Z][A-Z0-9_]{0,7}")
        .unwrap()
        .prop_filter("non-empty", |s| !s.is_empty())
}

/// Strategy for valid SymbolId values (1..=1000, avoiding EOF at 0).
pub fn symbol_id_strategy() -> impl Strategy<Value = SymbolId> {
    (1u16..=1000).prop_map(SymbolId)
}

/// Strategy for associativity values.
pub fn associativity_strategy() -> impl Strategy<Value = Associativity> {
    prop_oneof![Just(Associativity::Left), Just(Associativity::Right),]
}

/// Strategy for precedence levels (-10..=10).
pub fn precedence_strategy() -> impl Strategy<Value = i16> {
    -10i16..=10
}

// ---------------------------------------------------------------------------
// Grammar strategies
// ---------------------------------------------------------------------------

/// Strategy that generates a small but valid grammar.
///
/// The grammar will have 1–3 tokens and 1–4 rules, with the first rule's
/// LHS used as the start symbol. All generated grammars can successfully
/// produce parse tables.
pub fn small_grammar_strategy() -> impl Strategy<Value = Grammar> {
    // Generate 1-3 distinct token names, then build rules from them.
    prop::collection::hash_set(token_name_strategy(), 1..=3).prop_flat_map(|token_names| {
        let tokens: Vec<String> = token_names.into_iter().collect();
        let n_tokens = tokens.len();

        // Generate 1-4 rules, each with an RHS of 1-3 symbols drawn from the token set.
        let tokens_for_rules = tokens.clone();
        prop::collection::vec(prop::collection::vec(0..n_tokens, 1..=3), 1..=4).prop_map(
            move |rule_indices| {
                let mut builder = GrammarBuilder::new("proptest");

                // Register tokens.
                for name in &tokens_for_rules {
                    builder = builder.token(name, name);
                }

                // Add rules.  Use "start" as the only non-terminal to keep
                // the grammar simple and valid.
                for rhs_indices in &rule_indices {
                    let rhs: Vec<&str> = rhs_indices
                        .iter()
                        .map(|&i| tokens_for_rules[i].as_str())
                        .collect();
                    builder = builder.rule("start", rhs);
                }

                builder.start("start").build()
            },
        )
    })
}

/// Strategy for source-code-like strings (ASCII alphanum + whitespace).
pub fn source_text_strategy() -> impl Strategy<Value = String> {
    prop::string::string_regex("[0-9a-zA-Z \\t\\n]{1,100}").unwrap()
}

/// Strategy for edit operations: (offset, delete_len, insert_text).
pub fn edit_strategy() -> impl Strategy<Value = (usize, usize, String)> {
    (0usize..50, 0usize..10, "[0-9a-z ]{0,10}").prop_map(|(pos, del, ins)| (pos, del, ins))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grammar_helpers::build_parse_table;

    proptest! {
        #[test]
        fn small_grammars_are_valid(g in small_grammar_strategy()) {
            prop_assert!(!g.rules.is_empty());
            prop_assert!(!g.tokens.is_empty());
            prop_assert_eq!(g.name, "proptest");
        }

        #[test]
        fn ident_is_valid(s in ident_strategy()) {
            prop_assert!(!s.is_empty());
            prop_assert!(s.chars().next().unwrap().is_ascii_lowercase());
        }

        #[test]
        fn token_name_is_upper(s in token_name_strategy()) {
            prop_assert!(!s.is_empty());
            prop_assert!(s.chars().next().unwrap().is_ascii_uppercase());
        }

        #[test]
        fn symbol_id_is_nonzero(id in symbol_id_strategy()) {
            prop_assert!(id.0 > 0);
        }

        #[test]
        fn small_grammar_builds_table(g in small_grammar_strategy()) {
            // Not all random grammars produce valid tables, but this should
            // not panic. We just verify the function doesn't crash.
            let _ = build_parse_table(&g);
        }


        #[test]
        fn precedence_within_expected_range(value in precedence_strategy()) {
            prop_assert!((-10..=10).contains(&value));
        }

        #[test]
        fn source_text_is_non_empty_ascii(text in source_text_strategy()) {
            prop_assert!(!text.is_empty());
            prop_assert!(text.len() <= 100);
            prop_assert!(text.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, ' ' | '\t' | '\n')));
        }

        #[test]
        fn edit_strategy_stays_in_bounds((pos, del, ins) in edit_strategy()) {
            prop_assert!(pos < 50);
            prop_assert!(del < 10);
            prop_assert!(ins.len() <= 10);
            prop_assert!(ins.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == ' '));
        }

        #[test]
        fn associativity_strategy_returns_only_supported_values(assoc in associativity_strategy()) {
            prop_assert!(matches!(assoc, Associativity::Left | Associativity::Right));
        }
    }
}
