//! Adversarial fixtures for explicit compiler identity (#862 PR5).
//!
//! These cases are named to break legacy name/order heuristics. They prove
//! explicit `start_symbol` and `wrapper_token_relations` remain authoritative
//! under reordering and overlapping symbol names.

use adze_ir::builder::GrammarBuilder;
use adze_ir::{Grammar, ProductionId, Rule, Symbol, SymbolId};
use proptest::prelude::*;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

fn shuffle_rule_map(grammar: &mut Grammar, seed: u64) {
    let entries: Vec<_> = grammar.rules.drain(..).collect();
    let mut order: Vec<usize> = (0..entries.len()).collect();
    let mut hasher = DefaultHasher::new();
    seed.hash(&mut hasher);
    let mut state = hasher.finish();
    for i in (1..order.len()).rev() {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        let j = (state as usize) % (i + 1);
        order.swap(i, j);
    }
    for idx in order {
        let (id, rules) = &entries[idx];
        grammar.rules.insert(*id, rules.clone());
    }
}

#[test]
fn test_start_symbol_with_legacy_names_returns_root9() {
    let mut grammar = Grammar::new("adversarial".to_string());
    let source_id = SymbolId(10);
    let expression_id = SymbolId(11);
    let root_id = SymbolId(12);

    for (id, name) in [
        (source_id, "source_file"),
        (expression_id, "Expression"),
        (root_id, "Root9"),
    ] {
        grammar.rule_names.insert(id, name.to_string());
        grammar.rules.entry(id).or_default().push(Rule {
            lhs: id,
            rhs: vec![Symbol::Epsilon],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(0),
        });
    }
    grammar.set_start_symbol(root_id);

    assert_eq!(grammar.start_symbol(), Some(root_id));
}

#[test]
fn test_start_symbol_under_rule_shuffle_stays_root9() {
    let grammar = GrammarBuilder::new("shuffle")
        .token("t", "t")
        .rule("noise_a", vec!["t"])
        .rule("noise_b", vec!["t"])
        .rule("Root9", vec!["t"])
        .start("Root9")
        .build();
    let root_id = grammar.find_symbol_by_name("Root9").unwrap();

    for seed in 0..32 {
        let mut shuffled = grammar.clone();
        shuffle_rule_map(&mut shuffled, seed);
        assert_eq!(
            shuffled.start_symbol(),
            Some(root_id),
            "seed {seed} must not change explicit start"
        );
    }
}

#[test]
fn test_wrapper_relations_with_overlapping_tokens_match_explicit() {
    let grammar = GrammarBuilder::new("overlap")
        .token("id", "id")
        .token("identifier", r"[a-zA-Z_][a-zA-Z0-9_]*")
        .token("identifier_suffix", "_suffix")
        .wrapper_token("IdWrap", "id")
        .wrapper_token("IdentifierWrap", "identifier")
        .wrapper_token("SuffixWrap", "identifier_suffix")
        .rule("Root9", vec!["IdWrap", "IdentifierWrap", "SuffixWrap"])
        .start("Root9")
        .build();

    let id_token = grammar
        .tokens
        .iter()
        .find(|(_, token)| token.name == "id")
        .map(|(id, _)| *id)
        .expect("id token");
    let identifier_token = grammar
        .tokens
        .iter()
        .find(|(_, token)| token.name == "identifier")
        .map(|(id, _)| *id)
        .expect("identifier token");
    let suffix_token = grammar
        .tokens
        .iter()
        .find(|(_, token)| token.name == "identifier_suffix")
        .map(|(id, _)| *id)
        .expect("identifier_suffix token");

    let id_wrap = grammar.find_symbol_by_name("IdWrap").unwrap();
    let identifier_wrap = grammar.find_symbol_by_name("IdentifierWrap").unwrap();
    let suffix_wrap = grammar.find_symbol_by_name("SuffixWrap").unwrap();

    assert_eq!(grammar.wrapper_token_for(id_wrap), Some(id_token));
    assert_eq!(
        grammar.wrapper_token_for(identifier_wrap),
        Some(identifier_token)
    );
    assert_eq!(grammar.wrapper_token_for(suffix_wrap), Some(suffix_token));

    // Substring heuristics would attach IdWrap to `identifier` or `identifier_suffix`.
    assert_ne!(grammar.wrapper_token_for(id_wrap), Some(identifier_token));
    assert_ne!(grammar.wrapper_token_for(id_wrap), Some(suffix_token));
}

#[test]
fn test_wrapper_relations_with_misleading_names_match_explicit() {
    let grammar = GrammarBuilder::new("numeric_names")
        .token("letters", r"[A-Za-z]+")
        .token("digits", r"[0-9]+")
        .wrapper_token("NumberLike", "letters")
        .wrapper_token("AlphaWrapper", "digits")
        .rule("Root9", vec!["NumberLike", "AlphaWrapper"])
        .start("Root9")
        .build();

    let letters_token = grammar
        .tokens
        .iter()
        .find(|(_, token)| token.name == "letters")
        .map(|(id, _)| *id)
        .expect("letters token");
    let digits_token = grammar
        .tokens
        .iter()
        .find(|(_, token)| token.name == "digits")
        .map(|(id, _)| *id)
        .expect("digits token");

    let number_like = grammar.find_symbol_by_name("NumberLike").unwrap();
    let alpha_wrapper = grammar.find_symbol_by_name("AlphaWrapper").unwrap();

    assert_eq!(grammar.wrapper_token_for(number_like), Some(letters_token));
    assert_eq!(grammar.wrapper_token_for(alpha_wrapper), Some(digits_token));
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(16))]

    #[test]
    fn test_start_symbol_under_proptest_shuffle_stays_root9(seed in any::<u64>()) {
        let grammar = GrammarBuilder::new("prop_shuffle")
            .token("t", "t")
            .rule("helper", vec!["t"])
            .rule("Root9", vec!["t"])
            .start("Root9")
            .build();
        let root_id = grammar.find_symbol_by_name("Root9").unwrap();

        let mut shuffled = grammar;
        shuffle_rule_map(&mut shuffled, seed);
        prop_assert_eq!(shuffled.start_symbol(), Some(root_id));
    }
}
