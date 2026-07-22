//! Boundary and limits tests for the adze runtime crate.
//!
//! Tests that the parser handles extremes gracefully: no panics, no infinite
//! loops, and predictable error handling at the edges of input size, grammar
//! complexity, and parse-table dimensions.

use adze::glr_lexer::GLRLexer;
use adze::glr_parser::GLRParser;
use adze_glr_core::{FirstFollowSets, ParseTable, StateId, build_lr1_automaton};
use adze_ir::{Grammar, ProductionId, Rule, Symbol, SymbolId, Token, TokenPattern};

// ===========================================================================
// Helpers
// ===========================================================================

/// Minimal grammar: expr → number | expr '+' expr
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

/// Parenthesized expression grammar: expr → number | '(' expr ')'
fn paren_grammar() -> Grammar {
    let mut g = Grammar::new("paren".into());

    let num = SymbolId(1);
    let lp = SymbolId(2);
    let rp = SymbolId(3);
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
        lp,
        Token {
            name: "lparen".into(),
            pattern: TokenPattern::String("(".into()),
            fragile: false,
        },
    );
    g.tokens.insert(
        rp,
        Token {
            name: "rparen".into(),
            pattern: TokenPattern::String(")".into()),
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
            Symbol::Terminal(lp),
            Symbol::NonTerminal(expr),
            Symbol::Terminal(rp),
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

/// Build a GLR parser from the given grammar.
fn build_parser(grammar: &Grammar) -> GLRParser {
    let ff = FirstFollowSets::compute(grammar).expect("FIRST/FOLLOW");
    let table = build_lr1_automaton(grammar, &ff).expect("LR(1) automaton");
    GLRParser::new(table, grammar.clone())
}

/// Tokenize `input` and feed it into `parser`, then finish.
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

// ===========================================================================
// 1. Parse empty string with various grammars
// ===========================================================================

#[test]
fn empty_string_number_add_grammar() {
    let g = number_add_grammar();
    let mut parser = build_parser(&g);
    let result = parse_input(&mut parser, &g, "");
    assert!(
        result.is_err(),
        "empty input must not parse as an expression"
    );
}

#[test]
fn empty_string_paren_grammar() {
    let g = paren_grammar();
    let mut parser = build_parser(&g);
    let result = parse_input(&mut parser, &g, "");
    assert!(
        result.is_err(),
        "empty input must not parse with paren grammar"
    );
}

#[test]
fn empty_string_lexer_produces_no_tokens() {
    let g = number_add_grammar();
    let mut lexer = GLRLexer::new(&g, String::new()).unwrap();
    let tokens = lexer.tokenize_all();
    assert!(tokens.is_empty());
}

#[test]
fn empty_string_single_token_grammar() {
    // Grammar that only accepts a single literal "a"
    let mut g = Grammar::new("single".into());
    let a = SymbolId(1);
    let s = SymbolId(10);
    g.tokens.insert(
        a,
        Token {
            name: "a".into(),
            pattern: TokenPattern::String("a".into()),
            fragile: false,
        },
    );
    g.rules.entry(s).or_default().push(Rule {
        lhs: s,
        rhs: vec![Symbol::Terminal(a)],
        precedence: None,
        associativity: None,
        production_id: ProductionId(0),
        fields: vec![],
    });
    g.rule_names.insert(s, "start".into());
    g.set_start_symbol(s);

    let mut parser = build_parser(&g);
    let result = parse_input(&mut parser, &g, "");
    assert!(result.is_err());
}

// ===========================================================================
// 2. Parse single character inputs
// ===========================================================================

#[test]
fn single_digit_parses() {
    let g = number_add_grammar();
    let mut parser = build_parser(&g);
    for ch in '0'..='9' {
        let input = ch.to_string();
        let result = parse_input(&mut parser, &g, &input);
        assert!(
            result.is_ok(),
            "single digit '{}' should parse: {:?}",
            ch,
            result
        );
    }
}

#[test]
fn single_operator_rejects() {
    let g = number_add_grammar();
    let mut parser = build_parser(&g);
    // A lone '+' is not a valid expression
    let result = parse_input(&mut parser, &g, "+");
    assert!(result.is_err(), "lone operator should reject");
}

#[test]
fn single_paren_does_not_panic() {
    let g = paren_grammar();
    let mut parser = build_parser(&g);
    // Error recovery may accept partial inputs; we only require no panic.
    let _ = parse_input(&mut parser, &g, "(");
    let _ = parse_input(&mut parser, &g, ")");
}

#[test]
fn single_character_tokenization() {
    let g = number_add_grammar();
    let mut lexer = GLRLexer::new(&g, "5".into()).unwrap();
    let tokens = lexer.tokenize_all();
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].text, "5");
}

// ===========================================================================
// 3. Parse maximum token length inputs
// ===========================================================================

#[test]
fn very_long_single_token() {
    let g = number_add_grammar();
    // A number literal with 100,000 digits
    let big = "9".repeat(100_000);
    let mut lexer = GLRLexer::new(&g, big.clone()).unwrap();
    let tokens = lexer.tokenize_all();
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].text.len(), 100_000);
}

#[test]
fn very_long_single_token_parses() {
    let g = number_add_grammar();
    let big = "1".repeat(50_000);
    let mut parser = build_parser(&g);
    let result = parse_input(&mut parser, &g, &big);
    assert!(
        result.is_ok(),
        "long number literal should parse: {:?}",
        result
    );
}

#[test]
fn many_tokens_in_sequence() {
    let g = number_add_grammar();
    let mut parser = build_parser(&g);
    // "1+2+3+...+1000"
    let mut input = String::from("1");
    for i in 2..=1000 {
        input.push('+');
        input.push_str(&i.to_string());
    }
    let result = parse_input(&mut parser, &g, &input);
    assert!(
        result.is_ok(),
        "1000-element chain should parse: {:?}",
        result
    );
}

// ===========================================================================
// 4. Parse input with maximum nesting depth
// ===========================================================================

#[test]
fn nesting_depth_100() {
    let g = paren_grammar();
    let mut parser = build_parser(&g);
    let depth = 100;
    let input = "(".repeat(depth) + "1" + &")".repeat(depth);
    let result = parse_input(&mut parser, &g, &input);
    assert!(
        result.is_ok(),
        "depth-100 nesting should parse: {:?}",
        result
    );
}

#[test]
fn nesting_depth_200_no_panic() {
    let g = paren_grammar();
    let mut parser = build_parser(&g);
    let depth = 200;
    let input = "(".repeat(depth) + "1" + &")".repeat(depth);
    // Must not panic; result may be Ok or Err depending on stack limits.
    let _ = parse_input(&mut parser, &g, &input);
}

#[test]
fn unbalanced_deep_open_parens_rejects() {
    let g = paren_grammar();
    let mut parser = build_parser(&g);
    let input = "(".repeat(100) + "1";
    let result = parse_input(&mut parser, &g, &input);
    assert!(result.is_err(), "unbalanced deep open parens should reject");
}

#[test]
fn unbalanced_deep_close_parens_rejects() {
    let g = paren_grammar();
    let mut parser = build_parser(&g);
    let input = "1".to_string() + &")".repeat(100);
    let result = parse_input(&mut parser, &g, &input);
    assert!(
        result.is_err(),
        "unbalanced deep close parens should reject"
    );
}

// ===========================================================================
// 5. Parse input that produces maximum number of nodes
// ===========================================================================

#[test]
fn many_additions_produce_many_nodes() {
    let g = number_add_grammar();
    let mut parser = build_parser(&g);
    // 500 additions => ~1000 tokens, many tree nodes
    let mut input = String::from("0");
    for i in 1..500 {
        input.push('+');
        input.push_str(&i.to_string());
    }
    let result = parse_input(&mut parser, &g, &input);
    assert!(
        result.is_ok(),
        "500-addition chain should parse: {:?}",
        result
    );
}

#[test]
fn many_nested_parens_produce_many_nodes() {
    let g = paren_grammar();
    let mut parser = build_parser(&g);
    let depth = 80;
    let input = "(".repeat(depth) + "1" + &")".repeat(depth);
    let result = parse_input(&mut parser, &g, &input);
    assert!(
        result.is_ok(),
        "depth-80 paren nesting should parse: {:?}",
        result
    );
}

#[test]
fn alternating_single_digits_produces_many_tokens() {
    let g = number_add_grammar();
    // "1+2+3+4+..." — each digit is a separate token
    let mut input = String::new();
    for i in 0..200 {
        if i > 0 {
            input.push('+');
        }
        input.push_str(&(i % 10).to_string());
    }
    let mut parser = build_parser(&g);
    let result = parse_input(&mut parser, &g, &input);
    assert!(
        result.is_ok(),
        "200 single-digit additions should parse: {:?}",
        result
    );
}

// ===========================================================================
// 6. Parse with 0-length tokens
// ===========================================================================

#[test]
fn zero_length_regex_token_does_not_infinite_loop() {
    // A regex that can match the empty string (e.g. "\d*")
    let mut g = Grammar::new("zero_len".into());
    let star = SymbolId(1);
    let s = SymbolId(10);

    g.tokens.insert(
        star,
        Token {
            name: "digits_opt".into(),
            pattern: TokenPattern::Regex(r"\d*".into()),
            fragile: false,
        },
    );
    g.rules.entry(s).or_default().push(Rule {
        lhs: s,
        rhs: vec![Symbol::Terminal(star)],
        precedence: None,
        associativity: None,
        production_id: ProductionId(0),
        fields: vec![],
    });
    g.rule_names.insert(s, "start".into());
    g.set_start_symbol(s);

    let err = match GLRLexer::new(&g, "abc".into()) {
        Ok(_) => panic!("zero-length regex must be rejected"),
        Err(err) => err,
    };
    assert!(err.contains("zero-length matches are unsupported"));
}

#[test]
fn empty_literal_token_does_not_infinite_loop() {
    let mut g = Grammar::new("empty_lit".into());
    let empty = SymbolId(1);
    let s = SymbolId(10);

    g.tokens.insert(
        empty,
        Token {
            name: "empty".into(),
            pattern: TokenPattern::String(String::new()),
            fragile: false,
        },
    );
    g.rules.entry(s).or_default().push(Rule {
        lhs: s,
        rhs: vec![Symbol::Terminal(empty)],
        precedence: None,
        associativity: None,
        production_id: ProductionId(0),
        fields: vec![],
    });
    g.rule_names.insert(s, "start".into());
    g.set_start_symbol(s);

    let err = match GLRLexer::new(&g, "hello".into()) {
        Ok(_) => panic!("empty literal must be rejected"),
        Err(err) => err,
    };
    assert!(err.contains("empty literals are unsupported"));
}

// ===========================================================================
// 7. Parser state limits (max states, max symbols)
// ===========================================================================

#[test]
fn parser_with_empty_parse_table_does_not_crash_process() {
    // Construct a degenerate ParseTable with zero states and no actions.
    // The parser may panic (index OOB) but must not cause UB or abort.
    let result = std::panic::catch_unwind(|| {
        let g = Grammar::new("empty".into());
        let table = ParseTable::default();
        let mut parser = GLRParser::new(table, g.clone());
        parser.process_token(SymbolId(1), "x", 0);
        parser.process_eof(1);
        parser.finish()
    });
    // Either a panic or an Err is acceptable for a completely empty table.
    assert!(
        result.is_err() || result.unwrap().is_err(),
        "empty table must not produce a valid parse"
    );
}

#[test]
fn parser_with_single_state_no_actions() {
    let g = Grammar::new("one_state".into());
    let mut table = ParseTable {
        state_count: 1,
        symbol_count: 1,
        action_table: vec![vec![vec![]; 1]; 1],
        goto_table: vec![vec![StateId(0); 1]; 1],
        index_to_symbol: vec![SymbolId(0)],
        eof_symbol: SymbolId(0),
        ..Default::default()
    };
    table.symbol_to_index.insert(SymbolId(0), 0);

    let mut parser = GLRParser::new(table, g);
    parser.process_token(SymbolId(0), "", 0);
    parser.process_eof(0);
    let result = parser.finish();
    assert!(result.is_err());
}

#[test]
fn parser_with_many_symbols() {
    // Grammar with many terminal symbols (a..z) but all rules point to one NT.
    let mut g = Grammar::new("many_sym".into());
    let expr = SymbolId(100);
    g.rule_names.insert(expr, "expr".into());
    g.set_start_symbol(expr);

    for i in 1u16..=26 {
        let sym = SymbolId(i);
        let ch = (b'a' + (i - 1) as u8) as char;
        g.tokens.insert(
            sym,
            Token {
                name: ch.to_string(),
                pattern: TokenPattern::String(ch.to_string()),
                fragile: false,
            },
        );
        g.rules.entry(expr).or_default().push(Rule {
            lhs: expr,
            rhs: vec![Symbol::Terminal(sym)],
            precedence: None,
            associativity: None,
            production_id: ProductionId(i - 1),
            fields: vec![],
        });
    }

    let mut parser = build_parser(&g);
    // Each single letter should parse.
    for ch in 'a'..='z' {
        let result = parse_input(&mut parser, &g, &ch.to_string());
        assert!(result.is_ok(), "'{}' should parse: {:?}", ch, result);
    }
}

// ===========================================================================
// 8. Grammar with maximum number of rules
// ===========================================================================

#[test]
fn grammar_with_100_rules() {
    // Build a grammar with 100 alternative rules for one non-terminal.
    let mut g = Grammar::new("many_rules".into());
    let expr = SymbolId(200);
    g.rule_names.insert(expr, "expr".into());
    g.set_start_symbol(expr);

    for i in 0u16..100 {
        let sym = SymbolId(i + 1);
        // Use fixed-width names to avoid prefix conflicts (e.g. "k000".."k099")
        let literal = format!("k{i:03}");
        g.tokens.insert(
            sym,
            Token {
                name: literal.clone(),
                pattern: TokenPattern::String(literal.clone()),
                fragile: false,
            },
        );
        g.rules.entry(expr).or_default().push(Rule {
            lhs: expr,
            rhs: vec![Symbol::Terminal(sym)],
            precedence: None,
            associativity: None,
            production_id: ProductionId(i),
            fields: vec![],
        });
    }

    // Table construction must succeed.
    let ff = FirstFollowSets::compute(&g).expect("FIRST/FOLLOW for 100-rule grammar");
    let table = build_lr1_automaton(&g, &ff).expect("LR(1) for 100-rule grammar");
    let mut parser = GLRParser::new(table, g.clone());

    // Parse the first and last alternatives.
    let result0 = parse_input(&mut parser, &g, "k000");
    assert!(result0.is_ok(), "rule 0 should parse: {:?}", result0);

    let result99 = parse_input(&mut parser, &g, "k099");
    assert!(result99.is_ok(), "rule 99 should parse: {:?}", result99);

    // An unknown token should fail.
    let result_bad = parse_input(&mut parser, &g, "k100");
    assert!(result_bad.is_err(), "k100 is not in grammar");
}

#[test]
fn grammar_with_chained_nonterminals() {
    // A → B, B → C, C → D, ..., Z → "x"
    // Tests that deeply chained non-terminals don't blow up.
    let mut g = Grammar::new("chain".into());
    let x = SymbolId(1);
    g.tokens.insert(
        x,
        Token {
            name: "x".into(),
            pattern: TokenPattern::String("x".into()),
            fragile: false,
        },
    );

    let chain_len = 20;
    for i in 0..chain_len {
        let nt = SymbolId(100 + i);
        g.rule_names.insert(nt, format!("nt_{}", i));
        if i == chain_len - 1 {
            // Last NT → terminal
            g.rules.entry(nt).or_default().push(Rule {
                lhs: nt,
                rhs: vec![Symbol::Terminal(x)],
                precedence: None,
                associativity: None,
                production_id: ProductionId(i),
                fields: vec![],
            });
        } else {
            // NT_i → NT_{i+1}
            let next = SymbolId(100 + i + 1);
            g.rules.entry(nt).or_default().push(Rule {
                lhs: nt,
                rhs: vec![Symbol::NonTerminal(next)],
                precedence: None,
                associativity: None,
                production_id: ProductionId(i),
                fields: vec![],
            });
        }
    }

    g.set_start_symbol(SymbolId(100));

    let mut parser = build_parser(&g);
    let result = parse_input(&mut parser, &g, "x");
    assert!(result.is_ok(), "chained NTs should parse 'x': {:?}", result);
}

// ===========================================================================
// 9. Parse table with maximum number of entries
// ===========================================================================

#[test]
fn large_parse_table_many_states_does_not_panic() {
    // Construct a parse table with many states but no useful actions.
    let g = Grammar::new("big_table".into());
    let state_count = 500;
    let symbol_count = 10;

    let mut table = ParseTable {
        state_count,
        symbol_count,
        eof_symbol: SymbolId(0),
        action_table: vec![vec![vec![]; symbol_count]; state_count],
        goto_table: vec![vec![StateId(0); symbol_count]; state_count],
        ..Default::default()
    };
    for i in 0..symbol_count {
        let sym = SymbolId(i as u16);
        table.symbol_to_index.insert(sym, i);
        table.index_to_symbol.push(sym);
    }

    let mut parser = GLRParser::new(table, g);
    parser.process_token(SymbolId(1), "x", 0);
    parser.process_eof(1);
    let result = parser.finish();
    assert!(result.is_err(), "empty action table should not accept");
}

#[test]
fn large_parse_table_many_symbols_does_not_panic() {
    let g = Grammar::new("big_sym".into());
    let state_count = 2;
    let symbol_count = 500;

    let mut table = ParseTable {
        state_count,
        symbol_count,
        eof_symbol: SymbolId(0),
        action_table: vec![vec![vec![]; symbol_count]; state_count],
        goto_table: vec![vec![StateId(0); symbol_count]; state_count],
        ..Default::default()
    };
    for i in 0..symbol_count {
        let sym = SymbolId(i as u16);
        table.symbol_to_index.insert(sym, i);
        table.index_to_symbol.push(sym);
    }

    let mut parser = GLRParser::new(table, g);
    parser.process_token(SymbolId(1), "t", 0);
    parser.process_eof(1);
    let result = parser.finish();
    assert!(result.is_err());
}

#[test]
fn parse_table_with_shift_reduce_in_same_cell() {
    // Build a grammar that generates shift-reduce conflicts (GLR).
    // expr → expr '+' expr | number
    let g = number_add_grammar();
    let ff = FirstFollowSets::compute(&g).expect("FIRST/FOLLOW");
    let table = build_lr1_automaton(&g, &ff).expect("LR(1) automaton");

    // Verify the table was built, then parse an ambiguous expression.
    let mut parser = GLRParser::new(table, g.clone());
    let result = parse_input(&mut parser, &g, "1+2+3");
    assert!(
        result.is_ok(),
        "ambiguous expression should parse: {:?}",
        result
    );
}

// ===========================================================================
// 10. Stack depth limits during parsing
// ===========================================================================

#[test]
fn stack_depth_with_deeply_nested_expression() {
    let g = paren_grammar();
    let mut parser = build_parser(&g);
    // 150 levels of nesting: each pushes onto the parser stack.
    let depth = 150;
    let input = "(".repeat(depth) + "1" + &")".repeat(depth);
    // Must not stack-overflow.
    let _ = parse_input(&mut parser, &g, &input);
}

#[test]
fn stack_depth_with_long_right_chain() {
    // expr → number '+' expr | number
    // Parsing "1+2+3+...+N" is right-recursive and pushes deeply.
    let mut g = Grammar::new("right_rec".into());
    let num = SymbolId(1);
    let plus = SymbolId(2);
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

    // expr → number
    g.rules.entry(expr).or_default().push(Rule {
        lhs: expr,
        rhs: vec![Symbol::Terminal(num)],
        precedence: None,
        associativity: None,
        production_id: ProductionId(0),
        fields: vec![],
    });
    // expr → number '+' expr  (right-recursive)
    g.rules.entry(expr).or_default().push(Rule {
        lhs: expr,
        rhs: vec![
            Symbol::Terminal(num),
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

    let mut parser = build_parser(&g);
    // 200 terms in a right-recursive chain — must not panic or hang.
    let mut input = String::from("1");
    for i in 2..=200 {
        input.push('+');
        input.push_str(&i.to_string());
    }
    let _ = parse_input(&mut parser, &g, &input);
}

#[test]
fn repeated_reset_and_parse_does_not_grow_stack() {
    let g = number_add_grammar();
    let mut parser = build_parser(&g);
    for _ in 0..500 {
        let result = parse_input(&mut parser, &g, "1+2");
        assert!(result.is_ok());
    }
}

#[test]
fn parser_reset_after_error_cleans_state() {
    let g = number_add_grammar();
    let mut parser = build_parser(&g);

    // First: a failing parse
    let bad = parse_input(&mut parser, &g, "++");
    assert!(bad.is_err());

    // Second: a valid parse must still work after reset
    let ok = parse_input(&mut parser, &g, "1+2");
    assert!(ok.is_ok(), "parse after error should succeed: {:?}", ok);
}

#[test]
fn parser_finish_without_eof_is_err() {
    let g = number_add_grammar();
    let mut parser = build_parser(&g);
    parser.reset();
    let mut lexer = GLRLexer::new(&g, "1+2".into()).unwrap();
    let tokens = lexer.tokenize_all();
    for t in &tokens {
        parser.process_token(t.symbol_id, &t.text, t.byte_offset);
    }
    // Skip process_eof: the parser should not have an accept state.
    let result = parser.finish();
    assert!(result.is_err(), "finish without EOF should fail");
}
