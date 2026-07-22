// Simple test to debug GLR parse table generation

use adze_glr_core::{ConflictResolver, FirstFollowSets, build_lr1_automaton};
use adze_ir::{Grammar, ProductionId, Rule, Symbol, SymbolId, Token, TokenPattern};

#[test]
fn test_minimal_grammar() {
    let mut grammar = Grammar::new("minimal".to_string());

    // Single token
    grammar.tokens.insert(
        SymbolId(0),
        Token {
            name: "a".to_string(),
            pattern: TokenPattern::String("a".to_string()),
            fragile: false,
        },
    );

    // Single rule: S -> a
    let s_id = SymbolId(10);
    grammar.rules.entry(s_id).or_default().push(Rule {
        lhs: s_id,
        rhs: vec![Symbol::Terminal(SymbolId(0))],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    });

    grammar.set_start_symbol(s_id);

    println!(
        "Grammar created with {} rules and {} tokens",
        grammar.rules.len(),
        grammar.tokens.len()
    );

    // Compute FIRST/FOLLOW sets
    let first_follow = FirstFollowSets::compute(&grammar).unwrap();
    println!("FIRST/FOLLOW sets computed");

    // Build parse table
    match build_lr1_automaton(&grammar, &first_follow) {
        Ok(table) => {
            println!("Parse table built successfully!");
            println!("  States: {}", table.state_count);
            println!("  Symbols: {}", table.symbol_count);
            println!(
                "  Action table size: {}x{}",
                table.action_table.len(),
                if table.action_table.is_empty() {
                    0
                } else {
                    table.action_table[0].len()
                }
            );
        }
        Err(e) => {
            eprintln!("Failed to build parse table: {:?}", e);
            panic!("Parse table generation failed");
        }
    }
}

#[test]
fn test_simple_expression() {
    let mut grammar = Grammar::new("expr".to_string());

    // Tokens
    grammar.tokens.insert(
        SymbolId(0),
        Token {
            name: "num".to_string(),
            pattern: TokenPattern::Regex(r"\d+".to_string()),
            fragile: false,
        },
    );

    grammar.tokens.insert(
        SymbolId(1),
        Token {
            name: "plus".to_string(),
            pattern: TokenPattern::String("+".to_string()),
            fragile: false,
        },
    );

    // Rules
    let expr_id = SymbolId(10);

    // expr -> num
    grammar.rules.entry(expr_id).or_default().push(Rule {
        lhs: expr_id,
        rhs: vec![Symbol::Terminal(SymbolId(0))],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(0),
    });

    // expr -> expr + expr
    grammar.rules.entry(expr_id).or_default().push(Rule {
        lhs: expr_id,
        rhs: vec![
            Symbol::NonTerminal(expr_id),
            Symbol::Terminal(SymbolId(1)),
            Symbol::NonTerminal(expr_id),
        ],
        precedence: None,
        associativity: None,
        fields: vec![],
        production_id: ProductionId(1),
    });

    grammar.set_start_symbol(expr_id);

    println!("\nExpression grammar:");
    println!("  Rules: {}", grammar.rules.len());
    println!("  Tokens: {}", grammar.tokens.len());

    let first_follow = FirstFollowSets::compute(&grammar).unwrap();

    match build_lr1_automaton(&grammar, &first_follow) {
        Ok(table) => {
            println!("Parse table built!");
            println!("  States: {}", table.state_count);

            // Check for conflicts
            let item_sets = adze_glr_core::ItemSetCollection::build_canonical_collection(
                &grammar,
                &first_follow,
            );
            let resolver = ConflictResolver::detect_conflicts(&item_sets, &grammar, &first_follow);

            if !resolver.conflicts.is_empty() {
                println!("  Conflicts detected: {}", resolver.conflicts.len());
                for conflict in &resolver.conflicts {
                    println!(
                        "    - State {} on symbol {}: {:?}",
                        conflict.state.0, conflict.symbol.0, conflict.conflict_type
                    );
                }
            }
        }
        Err(e) => {
            eprintln!("Failed to build parse table: {:?}", e);
        }
    }
}
