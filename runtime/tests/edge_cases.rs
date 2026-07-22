//! Edge case tests for the adze runtime crate.
//!
//! Tests empty input, large input, Unicode edge cases, concurrency,
//! memory pressure, invalid UTF-8, null bytes, and deep nesting.

use adze::glr_lexer::GLRLexer;
use adze::glr_parser::GLRParser;
use adze::lexer::{ErrorRecoveringLexer, GrammarLexer};
use adze_glr_core::{FirstFollowSets, build_lr1_automaton};
use adze_ir::{Grammar, ProductionId, Rule, Symbol, SymbolId, Token, TokenPattern};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// 1. Empty input handling
// ---------------------------------------------------------------------------

#[test]
fn empty_input_glr_lexer_produces_no_tokens() {
    let g = number_add_grammar();
    let mut lexer = GLRLexer::new(&g, String::new()).unwrap();
    let tokens = lexer.tokenize_all();
    assert!(tokens.is_empty(), "empty input should produce no tokens");
}

#[test]
fn empty_input_grammar_lexer_no_meaningful_token() {
    let g = number_add_grammar();
    let patterns: Vec<_> = g
        .tokens
        .iter()
        .map(|(id, t)| (*id, t.pattern.clone(), 0))
        .collect();
    let mut lexer = GrammarLexer::new(&patterns);
    // The lexer may return an EOF-like token; verify no content token is produced.
    match lexer.next_token(b"", 0) {
        None => {} // ideal
        Some(tok) => {
            // If something is returned it should be zero-length or EOF.
            assert_eq!(
                tok.start, tok.end,
                "empty input should not produce a content token"
            );
        }
    }
}

#[test]
fn empty_input_glr_parser_rejects() {
    let g = number_add_grammar();
    let mut parser = build_parser(&g);
    let result = parse_input(&mut parser, &g, "");
    assert!(result.is_err(), "empty input should not parse successfully");
}

#[test]
fn whitespace_only_input_rejects() {
    let g = number_add_grammar();
    let mut parser = build_parser(&g);
    let result = parse_input(&mut parser, &g, "   \t\n  ");
    assert!(result.is_err(), "whitespace-only should not parse");
}

// ---------------------------------------------------------------------------
// 2. Very long input (≥ 10 MB)
// ---------------------------------------------------------------------------

#[test]
#[ignore = "slow: generates ~10 MB input"]
fn large_input_does_not_crash() {
    let g = number_add_grammar();
    let mut parser = build_parser(&g);

    // Build "1+1+1+...+1" ≈ 10 MB
    let repeat = 5_000_000;
    let mut input = String::with_capacity(repeat * 2);
    input.push('1');
    for _ in 1..repeat {
        input.push('+');
        input.push('1');
    }
    assert!(input.len() >= 10_000_000);

    // We only care that it doesn't crash or OOM; parse result is not checked.
    let _ = parse_input(&mut parser, &g, &input);
}

// ---------------------------------------------------------------------------
// 3. Unicode edge cases
// ---------------------------------------------------------------------------

fn unicode_id_grammar() -> Grammar {
    let mut g = Grammar::new("unicode".into());
    let id = SymbolId(1);
    let expr = SymbolId(10);

    // Token that matches any non-whitespace run (covers emoji, CJK, RTL, etc.)
    g.tokens.insert(
        id,
        Token {
            name: "id".into(),
            pattern: TokenPattern::Regex(r"[^\s]+".into()),
            fragile: false,
        },
    );
    g.rules.entry(expr).or_default().push(Rule {
        lhs: expr,
        rhs: vec![Symbol::Terminal(id)],
        precedence: None,
        associativity: None,
        production_id: ProductionId(0),
        fields: vec![],
    });
    g.rule_names.insert(expr, "expression".into());
    g.set_start_symbol(expr);
    g
}

#[test]
fn unicode_emoji_tokenizes() {
    let g = unicode_id_grammar();
    let mut lexer = GLRLexer::new(&g, "😀🎉🚀".into()).unwrap();
    let tokens = lexer.tokenize_all();
    assert!(!tokens.is_empty());
    assert_eq!(tokens[0].text, "😀🎉🚀");
}

#[test]
fn unicode_cjk_tokenizes() {
    let g = unicode_id_grammar();
    let mut lexer = GLRLexer::new(&g, "日本語テスト".into()).unwrap();
    let tokens = lexer.tokenize_all();
    assert!(!tokens.is_empty());
    assert_eq!(tokens[0].text, "日本語テスト");
}

#[test]
fn unicode_rtl_tokenizes() {
    let g = unicode_id_grammar();
    let mut lexer = GLRLexer::new(&g, "مرحبا".into()).unwrap();
    let tokens = lexer.tokenize_all();
    assert!(!tokens.is_empty());
}

#[test]
fn unicode_combining_characters_tokenizes() {
    let g = unicode_id_grammar();
    // e + combining acute accent
    let input = "e\u{0301}";
    let mut lexer = GLRLexer::new(&g, input.into()).unwrap();
    let tokens = lexer.tokenize_all();
    assert!(!tokens.is_empty());
    assert_eq!(tokens[0].text, input);
}

#[test]
fn unicode_zero_width_joiner_tokenizes() {
    let g = unicode_id_grammar();
    // Family emoji with ZWJ
    let input = "👨\u{200D}👩\u{200D}👧\u{200D}👦";
    let mut lexer = GLRLexer::new(&g, input.into()).unwrap();
    let tokens = lexer.tokenize_all();
    assert!(!tokens.is_empty());
}

#[test]
fn unicode_emoji_parses() {
    let g = unicode_id_grammar();
    let mut parser = build_parser(&g);
    let result = parse_input(&mut parser, &g, "🦀");
    assert!(result.is_ok(), "single emoji should parse: {:?}", result);
}

// ---------------------------------------------------------------------------
// 4. Concurrent parse requests (multi-threaded safety)
// ---------------------------------------------------------------------------

#[test]
fn concurrent_parses_do_not_interfere() {
    use std::thread;

    let g = number_add_grammar();
    let ff = FirstFollowSets::compute(&g).unwrap();
    let table = build_lr1_automaton(&g, &ff).unwrap();

    let handles: Vec<_> = (0..4)
        .map(|i| {
            let grammar = g.clone();
            let tbl = table.clone();
            thread::spawn(move || {
                let mut parser = GLRParser::new(tbl, grammar.clone());
                let input = format!("{i}+{}", i + 1);
                let mut lexer = GLRLexer::new(&grammar, input.clone()).unwrap();
                let tokens = lexer.tokenize_all();
                for t in &tokens {
                    parser.process_token(t.symbol_id, &t.text, t.byte_offset);
                }
                parser.process_eof(input.len());
                parser.finish().map(|_| ())
            })
        })
        .collect();

    for h in handles {
        let result = h.join().expect("thread panicked");
        assert!(result.is_ok(), "concurrent parse failed: {:?}", result);
    }
}

// ---------------------------------------------------------------------------
// 5. Memory pressure: many small parses
// ---------------------------------------------------------------------------

#[test]
fn many_small_parses_do_not_leak() {
    let g = number_add_grammar();
    let mut parser = build_parser(&g);

    for i in 0..1000 {
        let input = format!("{}", i);
        let result = parse_input(&mut parser, &g, &input);
        assert!(result.is_ok(), "parse {i} failed: {:?}", result);
    }
}

// ---------------------------------------------------------------------------
// 6. Invalid UTF-8 handling (GrammarLexer operates on &[u8])
// ---------------------------------------------------------------------------

#[test]
fn invalid_utf8_grammar_lexer_does_not_panic() {
    let g = number_add_grammar();
    let patterns: Vec<_> = g
        .tokens
        .iter()
        .map(|(id, t)| (*id, t.pattern.clone(), 0))
        .collect();
    let mut lexer = GrammarLexer::new(&patterns);

    // 0xFF 0xFE are not valid UTF-8 start bytes
    let bad = &[0xFF_u8, 0xFE, b'1', b'+', b'2'];
    let mut pos = 0;
    let mut tokens = Vec::new();
    while pos < bad.len() {
        match lexer.next_token(bad, pos) {
            Some(tok) => {
                tokens.push(tok.clone());
                pos = tok.end;
            }
            None => {
                pos += 1; // skip unrecognized byte
            }
        }
    }
    // Should have found number and plus tokens despite leading garbage.
    assert!(
        tokens.iter().any(|t| t.symbol == SymbolId(1)),
        "should find at least one number token"
    );
}

#[test]
fn error_recovering_lexer_handles_invalid_bytes() {
    let g = number_add_grammar();
    let patterns: Vec<_> = g
        .tokens
        .iter()
        .map(|(id, t)| (*id, t.pattern.clone(), 0))
        .collect();
    let base = GrammarLexer::new(&patterns);
    let mut lexer = ErrorRecoveringLexer::new(base, SymbolId(999));

    let bad = &[0x80_u8, 0x81, b'4', b'2'];
    let mut pos = 0;
    let mut tokens = Vec::new();
    while pos < bad.len() {
        match lexer.next_token(bad, pos) {
            Some(tok) => {
                tokens.push(tok.clone());
                pos = tok.end;
            }
            None => break,
        }
    }
    // Should contain an error token for the invalid bytes and a number token.
    let has_error = tokens.iter().any(|t| t.symbol == SymbolId(999));
    let has_number = tokens.iter().any(|t| t.symbol == SymbolId(1));
    assert!(has_error, "expected error token for invalid bytes");
    assert!(has_number, "expected number token after error recovery");
}

// ---------------------------------------------------------------------------
// 7. Null byte handling
// ---------------------------------------------------------------------------

#[test]
fn null_byte_in_grammar_lexer_does_not_panic() {
    let g = number_add_grammar();
    let patterns: Vec<_> = g
        .tokens
        .iter()
        .map(|(id, t)| (*id, t.pattern.clone(), 0))
        .collect();
    let mut lexer = GrammarLexer::new(&patterns);

    let input = b"1\x002";
    let mut pos = 0;
    let mut tokens = Vec::new();
    while pos < input.len() {
        match lexer.next_token(input, pos) {
            Some(tok) => {
                tokens.push(tok.clone());
                pos = tok.end;
            }
            None => {
                pos += 1;
            }
        }
    }
    // At minimum, "1" should tokenize as a number.
    assert!(
        tokens.iter().any(|t| t.symbol == SymbolId(1)),
        "should tokenize number before null byte"
    );
}

#[test]
fn null_byte_in_glr_lexer_does_not_panic() {
    let g = number_add_grammar();
    // Null byte embedded in the string (valid Rust &str, but unusual)
    let input = "1\x002";
    let mut lexer = GLRLexer::new(&g, input.into()).unwrap();
    let tokens = lexer.tokenize_all();
    // At minimum the leading "1" should tokenize.
    assert!(
        tokens.iter().any(|t| t.text == "1"),
        "should tokenize '1' before null byte"
    );
}

// ---------------------------------------------------------------------------
// 8. Maximum nesting depth
// ---------------------------------------------------------------------------

fn paren_grammar() -> Grammar {
    let mut g = Grammar::new("paren".into());

    let num = SymbolId(1);
    let lp = SymbolId(2);
    let rp = SymbolId(3);
    let plus = SymbolId(4);
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
    g.tokens.insert(
        plus,
        Token {
            name: "plus".into(),
            pattern: TokenPattern::String("+".into()),
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
    g.rules.entry(expr).or_default().push(Rule {
        lhs: expr,
        rhs: vec![
            Symbol::NonTerminal(expr),
            Symbol::Terminal(plus),
            Symbol::NonTerminal(expr),
        ],
        precedence: None,
        associativity: None,
        production_id: ProductionId(2),
        fields: vec![],
    });

    g.rule_names.insert(expr, "expression".into());
    g.set_start_symbol(expr);
    g
}

#[test]
fn moderate_nesting_depth_parses() {
    let g = paren_grammar();
    let mut parser = build_parser(&g);

    let depth = 50;
    let mut input = String::new();
    for _ in 0..depth {
        input.push('(');
    }
    input.push('1');
    for _ in 0..depth {
        input.push(')');
    }

    let result = parse_input(&mut parser, &g, &input);
    assert!(
        result.is_ok(),
        "depth-{depth} nesting should parse: {:?}",
        result
    );
}

#[test]
#[ignore = "slow: tests deep nesting up to 500 levels"]
fn deep_nesting_does_not_crash() {
    let g = paren_grammar();
    let mut parser = build_parser(&g);

    let depth = 500;
    let mut input = String::new();
    for _ in 0..depth {
        input.push('(');
    }
    input.push('1');
    for _ in 0..depth {
        input.push(')');
    }

    // We only care that it doesn't crash / stack-overflow.
    let _ = parse_input(&mut parser, &g, &input);
}

// ---------------------------------------------------------------------------
// Additional edge cases
// ---------------------------------------------------------------------------

#[test]
fn single_character_input_parses() {
    let g = number_add_grammar();
    let mut parser = build_parser(&g);
    let result = parse_input(&mut parser, &g, "0");
    assert!(result.is_ok(), "single digit should parse: {:?}", result);
}

#[test]
fn trailing_operator_rejects() {
    let g = number_add_grammar();
    let mut parser = build_parser(&g);
    let result = parse_input(&mut parser, &g, "1+");
    assert!(result.is_err(), "trailing operator should fail");
}

#[test]
fn leading_operator_handled() {
    // GLR parsers may accept or reject depending on error recovery; just don't panic.
    let g = number_add_grammar();
    let mut parser = build_parser(&g);
    let _ = parse_input(&mut parser, &g, "+1");
}

#[test]
fn double_operator_handled() {
    // GLR parsers may accept or reject depending on error recovery; just don't panic.
    let g = number_add_grammar();
    let mut parser = build_parser(&g);
    let _ = parse_input(&mut parser, &g, "1++2");
}

#[test]
fn unmatched_paren_rejects() {
    let g = paren_grammar();
    let mut parser = build_parser(&g);
    assert!(
        parse_input(&mut parser, &g, "(1").is_err(),
        "unmatched open paren"
    );
    assert!(
        parse_input(&mut parser, &g, "1)").is_err(),
        "unmatched close paren"
    );
}

#[test]
fn very_large_number_literal_tokenizes() {
    let g = number_add_grammar();
    let big = "9".repeat(10_000);
    let mut lexer = GLRLexer::new(&g, big.clone()).unwrap();
    let tokens = lexer.tokenize_all();
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].text, big);
}

#[test]
fn repeated_resets_are_safe() {
    let g = number_add_grammar();
    let mut parser = build_parser(&g);
    for _ in 0..100 {
        parser.reset();
    }
    let result = parse_input(&mut parser, &g, "1+2");
    assert!(result.is_ok());
}
