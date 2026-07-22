// Simple test for incremental parsing with subtree reuse
use adze_glr_core::{FirstFollowSets, build_lr1_automaton};
use adze_ir::{Grammar, ProductionId, Rule, Symbol, SymbolId, Token, TokenPattern};

fn create_simple_grammar() -> Grammar {
    let mut grammar = Grammar::new("simple".to_string());

    // Terminal: identifier
    let id_token = SymbolId(1);
    grammar.tokens.insert(
        id_token,
        Token {
            name: "identifier".to_string(),
            pattern: TokenPattern::Regex(r"[a-z]+".to_string()),
            fragile: false,
        },
    );

    // Non-terminal: S
    let s_id = SymbolId(10);
    grammar.rule_names.insert(s_id, "S".to_string());

    // Rule: S -> identifier
    grammar.rules.entry(s_id).or_default().push(Rule {
        lhs: s_id,
        rhs: vec![Symbol::Terminal(id_token)],
        precedence: None,
        associativity: None,
        production_id: ProductionId(0),
        fields: vec![],
    });

    grammar.set_start_symbol(s_id);
    grammar
}

#[test]
#[cfg_attr(
    not(feature = "incremental_glr"),
    ignore = "incremental parsing not enabled"
)]
fn test_incremental_basic() {
    let grammar = create_simple_grammar();
    let first_follow = FirstFollowSets::compute(&grammar).unwrap();
    let parse_table = build_lr1_automaton(&grammar, &first_follow).unwrap();

    // This test verifies the basic structure is working
    // Full incremental parsing tests would require access to internal modules
    assert!(parse_table.state_count > 0);
    assert!(parse_table.action_table.len() == parse_table.state_count);
}
