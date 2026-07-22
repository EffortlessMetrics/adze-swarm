//! Stress tests for the adze runtime crate.
//!
//! All tests are gated with `#[cfg(not(miri))]` since they exercise
//! heavy allocation / threading patterns unsuitable for Miri.

#![cfg(not(miri))]

use adze::glr_lexer::GLRLexer;
use adze::glr_parser::GLRParser;
use adze_glr_core::{FirstFollowSets, build_lr1_automaton};
use adze_ir::{Grammar, ProductionId, Rule, Symbol, SymbolId, Token, TokenPattern};

// ---------------------------------------------------------------------------
// Helpers (same minimal grammar used in edge_cases.rs)
// ---------------------------------------------------------------------------

fn number_add_grammar() -> Grammar {
    let mut g = Grammar::new("number_add".into());

    let num = SymbolId(1);
    let plus = SymbolId(2);
    let ws = SymbolId(3);
    let expr = SymbolId(10);

    g.tokens.insert(
        num,
        Token {
            name: "number".into(),
            pattern: TokenPattern::Regex(r"\d+".into()),
            fragile: false,
        },
    );
    g.tokens.insert(
        plus,
        Token {
            name: "plus".into(),
            pattern: TokenPattern::String("+".into()),
            fragile: false,
        },
    );
    g.tokens.insert(
        ws,
        Token {
            name: "ws".into(),
            pattern: TokenPattern::Regex(r"\s+".into()),
            fragile: false,
        },
    );

    g.rules.entry(expr).or_default().push(Rule {
        lhs: expr,
        rhs: vec![Symbol::Terminal(num)],
        precedence: None,
        associativity: None,
        production_id: ProductionId(0),
        fields: vec![],
    });
    g.rules.entry(expr).or_default().push(Rule {
        lhs: expr,
        rhs: vec![
            Symbol::NonTerminal(expr),
            Symbol::Terminal(plus),
            Symbol::NonTerminal(expr),
        ],
        precedence: None,
        associativity: None,
        production_id: ProductionId(1),
        fields: vec![],
    });

    g.rule_names.insert(expr, "expression".into());
    g.set_start_symbol(expr);
    g
}

fn build_parser(grammar: &Grammar) -> Result<GLRParser, String> {
    let ff = FirstFollowSets::compute(grammar)
        .map_err(|error| format!("failed to compute FIRST/FOLLOW sets: {error}"))?;
    let table = build_lr1_automaton(grammar, &ff)
        .map_err(|error| format!("failed to build LR(1) automaton: {error}"))?;
    Ok(GLRParser::new(table, grammar.clone()))
}

fn parse_input(parser: &mut GLRParser, grammar: &Grammar, input: &str) -> Result<(), String> {
    parser.reset();
    let mut lexer = GLRLexer::new(grammar, input.to_string())?;
    let tokens = lexer.tokenize_all();
    for t in &tokens {
        parser.process_token(t.symbol_id, &t.text, t.byte_offset);
    }
    parser.process_eof(input.len());
    parser.finish().map(|_| ())
}

// ---------------------------------------------------------------------------
// 1. Parse 1000 inputs in a loop
// ---------------------------------------------------------------------------

#[test]
fn stress_parse_1000_inputs() {
    let g = number_add_grammar();
    let parser_result = build_parser(&g);
    assert!(
        parser_result.is_ok(),
        "test grammar should build parser: {}",
        parser_result
            .as_ref()
            .err()
            .map_or("<none>", String::as_str)
    );
    let Ok(mut parser) = parser_result else {
        return;
    };

    for i in 0..1000 {
        let input = format!("{}+{}", i, i + 1);
        let result = parse_input(&mut parser, &g, &input);
        assert!(result.is_ok(), "iteration {i} failed: {:?}", result);
    }
}

#[test]
#[ignore = "slow: parses 1000 growing expressions"]
fn stress_parse_1000_growing_expressions() {
    let g = number_add_grammar();
    let parser_result = build_parser(&g);
    assert!(
        parser_result.is_ok(),
        "test grammar should build parser: {}",
        parser_result
            .as_ref()
            .err()
            .map_or("<none>", String::as_str)
    );
    let Ok(mut parser) = parser_result else {
        return;
    };

    for size in 1..=1000 {
        let mut input = String::from("1");
        for j in 2..=size.min(20) {
            input.push_str(&format!("+{j}"));
        }
        let result = parse_input(&mut parser, &g, &input);
        assert!(result.is_ok(), "size {size} failed: {:?}", result);
    }
}

// ---------------------------------------------------------------------------
// 2. Parse with different backend configurations
// ---------------------------------------------------------------------------

#[test]
fn stress_different_grammars_same_input() {
    // Grammar A: expr → number
    let mut ga = Grammar::new("a".into());
    ga.tokens.insert(
        SymbolId(1),
        Token {
            name: "number".into(),
            pattern: TokenPattern::Regex(r"\d+".into()),
            fragile: false,
        },
    );
    let ea = SymbolId(10);
    ga.rules.entry(ea).or_default().push(Rule {
        lhs: ea,
        rhs: vec![Symbol::Terminal(SymbolId(1))],
        precedence: None,
        associativity: None,
        production_id: ProductionId(0),
        fields: vec![],
    });
    ga.rule_names.insert(ea, "expr".into());
    ga.set_start_symbol(ea);

    // Grammar B: expr → number | number '+' number
    let mut gb = Grammar::new("b".into());
    gb.tokens.insert(
        SymbolId(1),
        Token {
            name: "number".into(),
            pattern: TokenPattern::Regex(r"\d+".into()),
            fragile: false,
        },
    );
    gb.tokens.insert(
        SymbolId(2),
        Token {
            name: "plus".into(),
            pattern: TokenPattern::String("+".into()),
            fragile: false,
        },
    );
    let eb = SymbolId(10);
    gb.rules.entry(eb).or_default().push(Rule {
        lhs: eb,
        rhs: vec![Symbol::Terminal(SymbolId(1))],
        precedence: None,
        associativity: None,
        production_id: ProductionId(0),
        fields: vec![],
    });
    gb.rules.entry(eb).or_default().push(Rule {
        lhs: eb,
        rhs: vec![
            Symbol::Terminal(SymbolId(1)),
            Symbol::Terminal(SymbolId(2)),
            Symbol::Terminal(SymbolId(1)),
        ],
        precedence: None,
        associativity: None,
        production_id: ProductionId(1),
        fields: vec![],
    });
    gb.rule_names.insert(eb, "expr".into());
    gb.set_start_symbol(eb);

    let parser_a = build_parser(&ga);
    assert!(
        parser_a.is_ok(),
        "grammar A should build parser: {}",
        parser_a.as_ref().err().map_or("<none>", String::as_str)
    );
    let Ok(mut pa) = parser_a else {
        return;
    };

    let parser_b = build_parser(&gb);
    assert!(
        parser_b.is_ok(),
        "grammar B should build parser: {}",
        parser_b.as_ref().err().map_or("<none>", String::as_str)
    );
    let Ok(mut pb) = parser_b else {
        return;
    };

    // "42" should parse on both grammars
    assert!(parse_input(&mut pa, &ga, "42").is_ok());
    assert!(parse_input(&mut pb, &gb, "42").is_ok());

    // "1+2" should only parse on grammar B
    assert!(parse_input(&mut pa, &ga, "1+2").is_err());
    assert!(parse_input(&mut pb, &gb, "1+2").is_ok());
}

#[test]
fn stress_rebuild_parser_repeatedly() {
    let g = number_add_grammar();
    for _ in 0..50 {
        let parser_result = build_parser(&g);
        assert!(
            parser_result.is_ok(),
            "test grammar should build parser: {}",
            parser_result
                .as_ref()
                .err()
                .map_or("<none>", String::as_str)
        );
        let Ok(mut parser) = parser_result else {
            return;
        };
        let result = parse_input(&mut parser, &g, "1+2+3");
        assert!(result.is_ok());
    }
}

// ---------------------------------------------------------------------------
// 3. Thread safety test with std::thread::spawn
// ---------------------------------------------------------------------------

#[test]
fn stress_thread_spawn_parsing() {
    use std::thread;

    let g = number_add_grammar();
    let ff = FirstFollowSets::compute(&g).unwrap();
    let table = build_lr1_automaton(&g, &ff).unwrap();

    let handles: Vec<_> = (0..8)
        .map(|i| {
            let grammar = g.clone();
            let tbl = table.clone();
            thread::spawn(move || {
                let mut parser = GLRParser::new(tbl, grammar.clone());
                for j in 0..100 {
                    let input = format!("{}+{}", i * 100 + j, i * 100 + j + 1);
                    parser.reset();
                    let mut lexer = GLRLexer::new(&grammar, input.clone()).unwrap();
                    let tokens = lexer.tokenize_all();
                    for t in &tokens {
                        parser.process_token(t.symbol_id, &t.text, t.byte_offset);
                    }
                    parser.process_eof(input.len());
                    let r = parser.finish();
                    assert!(r.is_ok(), "thread {i}, iter {j} failed: {:?}", r);
                }
            })
        })
        .collect();

    for h in handles {
        h.join().expect("thread panicked");
    }
}

#[test]
fn stress_threads_independent_grammars() {
    use std::thread;

    let handles: Vec<_> = (0..4)
        .map(|_| {
            thread::spawn(|| {
                // Each thread builds its own grammar and parser from scratch.
                let g = number_add_grammar();
                let parser_result = build_parser(&g);
                assert!(
                    parser_result.is_ok(),
                    "test grammar should build parser: {}",
                    parser_result
                        .as_ref()
                        .err()
                        .map_or("<none>", String::as_str)
                );
                let Ok(mut parser) = parser_result else {
                    return;
                };
                for i in 0..50 {
                    let input = format!("{i}+{}", i + 1);
                    let result = parse_input(&mut parser, &g, &input);
                    assert!(result.is_ok());
                }
            })
        })
        .collect();

    for h in handles {
        h.join().expect("thread panicked");
    }
}
