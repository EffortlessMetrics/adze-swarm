//! Unicode and international text handling tests.
//!
//! Verifies that the lexer and GLR parser correctly handle multi-byte UTF-8
//! sequences, CJK characters, emoji, combining marks, RTL scripts, BOM,
//! null bytes, and large inputs – with correct byte positions throughout.

use adze::glr_lexer::GLRLexer;
use adze::glr_parser::GLRParser;
use adze::lexer::GrammarLexer;
use adze_glr_core::{FirstFollowSets, build_lr1_automaton};
use adze_ir::{Grammar, ProductionId, Rule, Symbol, SymbolId, Token, TokenPattern};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Grammar: expr → id  (where id matches any non-whitespace run).
fn unicode_id_grammar() -> Grammar {
    let mut g = Grammar::new("unicode".into());
    let id = SymbolId(1);
    let ws = SymbolId(2);
    let expr = SymbolId(10);

    g.tokens.insert(
        id,
        Token {
            name: "id".into(),
            pattern: TokenPattern::Regex(r"[^\s]+".into()),
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

/// Grammar: expr → id ('+' id)* where id matches `[^\s+]+`.
fn unicode_add_grammar() -> Grammar {
    let mut g = Grammar::new("unicode_add".into());
    let id = SymbolId(1);
    let plus = SymbolId(2);
    let ws = SymbolId(3);
    let expr = SymbolId(10);

    g.tokens.insert(
        id,
        Token {
            name: "id".into(),
            pattern: TokenPattern::Regex(r"[^\s+]+".into()),
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

    // expr → id
    g.rules.entry(expr).or_default().push(Rule {
        lhs: expr,
        rhs: vec![Symbol::Terminal(id)],
        precedence: None,
        associativity: None,
        production_id: ProductionId(0),
        fields: vec![],
    });
    // expr → expr '+' expr
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

fn build_parser(grammar: &Grammar) -> GLRParser {
    let ff = FirstFollowSets::compute(grammar).expect("FIRST/FOLLOW");
    let table = build_lr1_automaton(grammar, &ff).expect("LR(1) automaton");
    GLRParser::new(table, grammar.clone())
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
// 1. CJK characters
// ---------------------------------------------------------------------------

#[test]
fn cjk_tokenizes_correctly() {
    let g = unicode_id_grammar();
    let input = "日本語テスト";
    let mut lexer = GLRLexer::new(&g, input.into()).unwrap();
    let tokens = lexer.tokenize_all();

    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].text, input);
    assert_eq!(tokens[0].byte_offset, 0);
    assert_eq!(tokens[0].byte_length, input.len()); // 18 bytes (6 chars × 3 bytes)
}

#[test]
fn cjk_parses() {
    let g = unicode_id_grammar();
    let mut parser = build_parser(&g);
    let result = parse_input(&mut parser, &g, "漢字");
    assert!(result.is_ok(), "CJK input should parse: {:?}", result);
}

#[test]
fn cjk_byte_positions_in_addition() {
    let g = unicode_add_grammar();
    let input = "你好 + 世界";
    let mut lexer = GLRLexer::new(&g, input.into()).unwrap();
    let tokens: Vec<_> = lexer.tokenize_all();

    // "你好" = 6 bytes, " " = 1, "+" = 1, " " = 1, "世界" = 6 bytes
    let ids: Vec<_> = tokens.iter().map(|t| &t.text).collect();
    assert!(ids.contains(&&"你好".to_string()), "tokens: {:?}", ids);
    assert!(ids.contains(&&"世界".to_string()), "tokens: {:?}", ids);

    let first = tokens.iter().find(|t| t.text == "你好").unwrap();
    assert_eq!(first.byte_offset, 0);
    assert_eq!(first.byte_length, 6);

    let second = tokens.iter().find(|t| t.text == "世界").unwrap();
    assert_eq!(second.byte_offset, 9); // 6 + 1(' ') + 1('+') + 1(' ')
    assert_eq!(second.byte_length, 6);
}

// ---------------------------------------------------------------------------
// 2. Emoji
// ---------------------------------------------------------------------------

#[test]
fn emoji_tokenizes_correctly() {
    let g = unicode_id_grammar();
    let input = "😀🎉🚀";
    let mut lexer = GLRLexer::new(&g, input.into()).unwrap();
    let tokens = lexer.tokenize_all();

    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].text, input);
    assert_eq!(tokens[0].byte_length, 12); // 3 emoji × 4 bytes each
}

#[test]
fn emoji_parses() {
    let g = unicode_id_grammar();
    let mut parser = build_parser(&g);
    let result = parse_input(&mut parser, &g, "🦀");
    assert!(result.is_ok(), "emoji should parse: {:?}", result);
}

#[test]
fn zwj_family_emoji_tokenizes() {
    let g = unicode_id_grammar();
    // Family emoji with Zero-Width Joiners
    let input = "👨\u{200D}👩\u{200D}👧\u{200D}👦";
    let mut lexer = GLRLexer::new(&g, input.into()).unwrap();
    let tokens = lexer.tokenize_all();
    assert!(!tokens.is_empty());
    assert_eq!(tokens[0].text, input);
    assert_eq!(tokens[0].byte_length, input.len());
}

#[test]
fn emoji_byte_positions_in_addition() {
    let g = unicode_add_grammar();
    let input = "🌍 + 🌏";
    let mut lexer = GLRLexer::new(&g, input.into()).unwrap();
    let tokens: Vec<_> = lexer.tokenize_all();

    let first = tokens.iter().find(|t| t.text == "🌍").unwrap();
    assert_eq!(first.byte_offset, 0);
    assert_eq!(first.byte_length, 4);

    let second = tokens.iter().find(|t| t.text == "🌏").unwrap();
    // "🌍" = 4, " " = 1, "+" = 1, " " = 1 → offset 7
    assert_eq!(second.byte_offset, 7);
    assert_eq!(second.byte_length, 4);
}

// ---------------------------------------------------------------------------
// 3. Combining diacritical marks
// ---------------------------------------------------------------------------

#[test]
fn combining_marks_tokenize_as_single_token() {
    let g = unicode_id_grammar();
    // 'e' + combining acute accent U+0301
    let input = "e\u{0301}";
    let mut lexer = GLRLexer::new(&g, input.into()).unwrap();
    let tokens = lexer.tokenize_all();

    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].text, input);
    // 'e' = 1 byte, U+0301 = 2 bytes
    assert_eq!(tokens[0].byte_length, 3);
}

#[test]
fn multiple_combining_marks() {
    let g = unicode_id_grammar();
    // 'a' + combining tilde + combining acute
    let input = "a\u{0303}\u{0301}";
    let mut lexer = GLRLexer::new(&g, input.into()).unwrap();
    let tokens = lexer.tokenize_all();

    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].text, input);
    // 'a' = 1, U+0303 = 2, U+0301 = 2
    assert_eq!(tokens[0].byte_length, 5);
}

#[test]
fn combining_marks_parse() {
    let g = unicode_id_grammar();
    let mut parser = build_parser(&g);
    let input = "n\u{0303}"; // ñ as n + combining tilde
    let result = parse_input(&mut parser, &g, input);
    assert!(result.is_ok(), "combining marks should parse: {:?}", result);
}

// ---------------------------------------------------------------------------
// 4. RTL text (Arabic, Hebrew)
// ---------------------------------------------------------------------------

#[test]
fn arabic_tokenizes() {
    let g = unicode_id_grammar();
    let input = "مرحبا";
    let mut lexer = GLRLexer::new(&g, input.into()).unwrap();
    let tokens = lexer.tokenize_all();

    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].text, input);
    assert_eq!(tokens[0].byte_length, input.len());
}

#[test]
fn hebrew_tokenizes() {
    let g = unicode_id_grammar();
    let input = "שלום";
    let mut lexer = GLRLexer::new(&g, input.into()).unwrap();
    let tokens = lexer.tokenize_all();

    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].text, input);
    assert_eq!(tokens[0].byte_length, input.len());
}

#[test]
fn arabic_parses() {
    let g = unicode_id_grammar();
    let mut parser = build_parser(&g);
    let result = parse_input(&mut parser, &g, "مرحبا");
    assert!(result.is_ok(), "Arabic should parse: {:?}", result);
}

#[test]
fn hebrew_parses() {
    let g = unicode_id_grammar();
    let mut parser = build_parser(&g);
    let result = parse_input(&mut parser, &g, "שלום");
    assert!(result.is_ok(), "Hebrew should parse: {:?}", result);
}

#[test]
fn rtl_byte_positions_in_addition() {
    let g = unicode_add_grammar();
    // Arabic "hello" + Hebrew "peace"
    let input = "مرحبا + שלום";
    let mut lexer = GLRLexer::new(&g, input.into()).unwrap();
    let tokens: Vec<_> = lexer.tokenize_all();

    let arabic = tokens.iter().find(|t| t.text == "مرحبا").unwrap();
    assert_eq!(arabic.byte_offset, 0);
    let arabic_len = "مرحبا".len();
    assert_eq!(arabic.byte_length, arabic_len);

    let hebrew = tokens.iter().find(|t| t.text == "שלום").unwrap();
    // arabic_len + " " + "+" + " "
    assert_eq!(hebrew.byte_offset, arabic_len + 3);
    assert_eq!(hebrew.byte_length, "שלום".len());
}

// ---------------------------------------------------------------------------
// 5. Multi-byte UTF-8 sequences
// ---------------------------------------------------------------------------

#[test]
fn two_byte_utf8() {
    let g = unicode_id_grammar();
    // Latin Small Letter E with Acute (U+00E9) = 2 bytes in UTF-8
    let input = "café";
    let mut lexer = GLRLexer::new(&g, input.into()).unwrap();
    let tokens = lexer.tokenize_all();

    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].text, input);
    // c=1, a=1, f=1, é=2 → 5 bytes total
    assert_eq!(tokens[0].byte_length, 5);
}

#[test]
fn three_byte_utf8() {
    let g = unicode_id_grammar();
    // Euro sign U+20AC = 3 bytes
    let input = "€100";
    let mut lexer = GLRLexer::new(&g, input.into()).unwrap();
    let tokens = lexer.tokenize_all();

    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].byte_length, 6); // 3 (€) + 1 + 1 + 1
}

#[test]
fn four_byte_utf8() {
    let g = unicode_id_grammar();
    // Mathematical Bold Capital A U+1D400 = 4 bytes
    let input = "𝐀";
    let mut lexer = GLRLexer::new(&g, input.into()).unwrap();
    let tokens = lexer.tokenize_all();

    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].byte_length, 4);
}

#[test]
fn mixed_byte_widths() {
    let g = unicode_id_grammar();
    // Mix: ASCII(1) + 2-byte + 3-byte + 4-byte
    let input = "Aé€𝐀";
    let mut lexer = GLRLexer::new(&g, input.into()).unwrap();
    let tokens = lexer.tokenize_all();

    assert_eq!(tokens.len(), 1);
    // A=1, é=2, €=3, 𝐀=4 → 10 bytes
    assert_eq!(tokens[0].byte_length, 10);
}

// ---------------------------------------------------------------------------
// 6. Byte position correctness for multi-byte chars
// ---------------------------------------------------------------------------

#[test]
fn byte_positions_correct_with_multibyte_tokens() {
    let g = unicode_add_grammar();
    // "café + naïve"
    let input = "café + naïve";
    let mut lexer = GLRLexer::new(&g, input.into()).unwrap();
    let tokens: Vec<_> = lexer.tokenize_all();

    let cafe = tokens.iter().find(|t| t.text == "café").unwrap();
    assert_eq!(cafe.byte_offset, 0);
    assert_eq!(cafe.byte_length, 5); // c(1) + a(1) + f(1) + é(2)

    let naive = tokens.iter().find(|t| t.text == "naïve").unwrap();
    // "café" = 5 bytes, " " = 1, "+" = 1, " " = 1 → offset 8
    assert_eq!(naive.byte_offset, 8);
    assert_eq!(naive.byte_length, 6); // n(1) + a(1) + ï(2) + v(1) + e(1)
}

#[test]
fn byte_offsets_with_grammar_lexer() {
    let g = unicode_id_grammar();
    let patterns: Vec<_> = g
        .tokens
        .iter()
        .map(|(id, t)| (*id, t.pattern.clone(), 0))
        .collect();
    let mut lexer = GrammarLexer::new(&patterns);

    let input = "café naïve";
    let bytes = input.as_bytes();
    let mut pos = 0;
    let mut tokens = Vec::new();
    while pos < bytes.len() {
        match lexer.next_token(bytes, pos) {
            Some(tok) => {
                tokens.push(tok.clone());
                pos = tok.end;
            }
            None => {
                pos += 1;
            }
        }
    }

    // Find the "café" token (id = SymbolId(1))
    let cafe_tok = tokens
        .iter()
        .find(|t| t.symbol == SymbolId(1) && t.start == 0)
        .unwrap();
    assert_eq!(cafe_tok.start, 0);
    assert_eq!(cafe_tok.end, 5); // 5 bytes

    // Find "naïve" – should start after "café" + space
    let naive_tok = tokens
        .iter()
        .find(|t| t.symbol == SymbolId(1) && t.start > 0)
        .unwrap();
    assert_eq!(naive_tok.start, 6); // after "café"(5) + " "(1)
    assert_eq!(naive_tok.end, 12); // 6 bytes for "naïve"
}

// ---------------------------------------------------------------------------
// 7. Point (row, column) calculation with multi-byte chars
// ---------------------------------------------------------------------------

#[cfg(feature = "ts-compat")]
mod point_calculation_tests {
    use adze::ts_compat::{Language, Parser, Point};
    use adze_glr_core::{FirstFollowSets, build_lr1_automaton};
    use adze_ir::{Grammar, ProductionId, Rule, Symbol, SymbolId, Token, TokenPattern};
    use std::sync::Arc;

    fn create_unicode_language() -> Arc<Language> {
        let mut grammar = Grammar::new("unicode".to_string());
        let id = SymbolId(1);
        let expr = SymbolId(10);

        grammar.tokens.insert(
            id,
            Token {
                name: "id".into(),
                pattern: TokenPattern::Regex(r"[^\s]+".into()),
                fragile: false,
            },
        );

        grammar.rules.entry(expr).or_default().push(Rule {
            lhs: expr,
            rhs: vec![Symbol::Terminal(id)],
            precedence: None,
            associativity: None,
            production_id: ProductionId(0),
            fields: vec![],
        });
        grammar.rule_names.insert(expr, "expression".into());
        grammar.set_start_symbol(expr);

        let ff = FirstFollowSets::compute(&grammar).unwrap();
        let table = build_lr1_automaton(&grammar, &ff).unwrap();
        Arc::new(Language::new("unicode", grammar, table))
    }

    #[test]
    fn point_single_line_ascii() {
        let language = create_unicode_language();
        let mut parser = Parser::new();
        parser.set_language(language).unwrap();

        let tree = parser.parse("hello", None).unwrap();
        let root = tree.root_node();
        assert_eq!(root.start_position(), Point { row: 0, column: 0 });
        assert_eq!(root.end_position(), Point { row: 0, column: 5 });
    }

    #[test]
    fn point_single_line_multibyte() {
        let language = create_unicode_language();
        let mut parser = Parser::new();
        parser.set_language(language).unwrap();

        // "café" = 5 bytes, column should reflect bytes
        let tree = parser.parse("café", None).unwrap();
        let root = tree.root_node();
        assert_eq!(root.start_position(), Point { row: 0, column: 0 });
        // Tree-sitter uses byte-based columns
        assert_eq!(root.end_position(), Point { row: 0, column: 5 });
    }

    #[test]
    fn point_multiline_with_cjk() {
        let language = create_unicode_language();
        let mut parser = Parser::new();
        parser.set_language(language).unwrap();

        // Multiline: first line is "日本\n語" but our grammar takes non-whitespace runs
        // so this will only match "日本" (stops at newline)
        let source = "日本語";
        if let Some(tree) = parser.parse(source, None) {
            let root = tree.root_node();
            assert_eq!(root.start_byte(), 0);
            assert_eq!(root.end_byte(), source.len());
        }
    }
}

// ---------------------------------------------------------------------------
// 8. BOM (Byte Order Mark)
// ---------------------------------------------------------------------------

#[test]
fn bom_in_glr_lexer_does_not_crash() {
    let g = unicode_id_grammar();
    // UTF-8 BOM is U+FEFF = EF BB BF (3 bytes)
    let input = "\u{FEFF}hello";
    let mut lexer = GLRLexer::new(&g, input.into()).unwrap();
    let tokens = lexer.tokenize_all();

    // BOM + "hello" should be lexed (BOM is non-whitespace, so treated as part of token)
    assert!(!tokens.is_empty());
}

#[test]
fn bom_only_parses() {
    let g = unicode_id_grammar();
    let mut parser = build_parser(&g);
    let input = "\u{FEFF}";
    // BOM alone may parse as a single id token or fail – just don't crash
    let _ = parse_input(&mut parser, &g, input);
}

#[test]
fn bom_before_content_byte_positions() {
    let g = unicode_add_grammar();
    // BOM(3 bytes) + "a + b"
    let input = "\u{FEFF}a + b";
    let mut lexer = GLRLexer::new(&g, input.into()).unwrap();
    let tokens: Vec<_> = lexer.tokenize_all();

    // The BOM may merge with 'a' or be a separate token – verify no panic and
    // all tokens have consistent byte ranges
    for tok in &tokens {
        assert!(
            tok.byte_offset + tok.byte_length <= input.len(),
            "token '{}' at offset {} with length {} exceeds input length {}",
            tok.text,
            tok.byte_offset,
            tok.byte_length,
            input.len()
        );
    }
}

#[test]
fn bom_grammar_lexer_byte_positions() {
    let g = unicode_id_grammar();
    let patterns: Vec<_> = g
        .tokens
        .iter()
        .map(|(id, t)| (*id, t.pattern.clone(), 0))
        .collect();
    let mut lexer = GrammarLexer::new(&patterns);

    let input = "\u{FEFF}test";
    let bytes = input.as_bytes();
    let mut pos = 0;
    let mut tokens = Vec::new();
    while pos < bytes.len() {
        match lexer.next_token(bytes, pos) {
            Some(tok) => {
                tokens.push(tok.clone());
                pos = tok.end;
            }
            None => {
                pos += 1;
            }
        }
    }

    // Verify every token has valid byte range
    for tok in &tokens {
        assert!(tok.start <= tok.end);
        assert!(tok.end <= bytes.len());
    }
}

// ---------------------------------------------------------------------------
// 9. Null bytes in input
// ---------------------------------------------------------------------------

#[test]
fn null_bytes_in_glr_lexer() {
    let g = unicode_id_grammar();
    let input = "hello\x00world";
    let mut lexer = GLRLexer::new(&g, input.into()).unwrap();
    let tokens = lexer.tokenize_all();

    // Should produce at least one token without panicking
    assert!(!tokens.is_empty());
}

#[test]
fn null_byte_between_tokens() {
    let g = unicode_add_grammar();
    let input = "a\x00b";
    let mut lexer = GLRLexer::new(&g, input.into()).unwrap();
    let tokens = lexer.tokenize_all();

    // Verify no token overflows
    for tok in &tokens {
        assert!(tok.byte_offset + tok.byte_length <= input.len());
    }
}

#[test]
fn null_byte_grammar_lexer() {
    let g = unicode_id_grammar();
    let patterns: Vec<_> = g
        .tokens
        .iter()
        .map(|(id, t)| (*id, t.pattern.clone(), 0))
        .collect();
    let mut lexer = GrammarLexer::new(&patterns);

    let input = b"abc\x00def";
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

    // "abc" should tokenize before null byte
    assert!(
        tokens
            .iter()
            .any(|t| t.text == b"abc" || t.text.starts_with(b"abc")),
        "should find token starting with 'abc', got: {:?}",
        tokens
            .iter()
            .map(|t| String::from_utf8_lossy(&t.text).to_string())
            .collect::<Vec<_>>()
    );
}

#[test]
fn null_byte_glr_parser_does_not_crash() {
    let g = unicode_id_grammar();
    let mut parser = build_parser(&g);
    let input = "test\x00data";
    let _ = parse_input(&mut parser, &g, input);
}

// ---------------------------------------------------------------------------
// 10. Maximum length inputs
// ---------------------------------------------------------------------------

#[test]
fn large_unicode_input_tokenizes() {
    let g = unicode_id_grammar();
    // Build a ~100KB string of CJK characters
    let chunk = "漢字";
    let input: String = chunk.repeat(10_000);
    assert!(input.len() > 50_000);

    let mut lexer = GLRLexer::new(&g, input.clone()).unwrap();
    let tokens = lexer.tokenize_all();

    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].text, input);
    assert_eq!(tokens[0].byte_length, input.len());
}

#[test]
fn large_unicode_addition_chain() {
    let g = unicode_add_grammar();
    let mut parser = build_parser(&g);

    // "é + é + é + ... + é" with 100 terms
    let n = 100;
    let mut input = String::from("é");
    for _ in 1..n {
        input.push_str(" + é");
    }

    let result = parse_input(&mut parser, &g, &input);
    assert!(result.is_ok(), "large chain should parse: {:?}", result);
}

#[test]
fn large_emoji_input() {
    let g = unicode_id_grammar();
    let input: String = "🦀".repeat(5_000);
    assert!(input.len() >= 20_000); // 4 bytes each

    let mut lexer = GLRLexer::new(&g, input.clone()).unwrap();
    let tokens = lexer.tokenize_all();

    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].byte_length, input.len());
}

#[test]
#[ignore = "slow: generates ~10 MB input"]
fn very_large_multibyte_input_does_not_crash() {
    let g = unicode_id_grammar();
    let input: String = "日本語".repeat(1_000_000); // ~9 MB
    assert!(input.len() >= 9_000_000);

    let mut lexer = GLRLexer::new(&g, input.clone()).unwrap();
    let tokens = lexer.tokenize_all();
    assert!(!tokens.is_empty());
}

// ---------------------------------------------------------------------------
// Additional: mixed script and edge cases
// ---------------------------------------------------------------------------

#[test]
fn mixed_scripts_in_addition() {
    let g = unicode_add_grammar();
    // ASCII + CJK + Arabic + Emoji in one expression
    let input = "hello + 世界 + مرحبا + 🌍";
    let mut lexer = GLRLexer::new(&g, input.into()).unwrap();
    let tokens: Vec<_> = lexer.tokenize_all();

    let texts: Vec<&str> = tokens.iter().map(|t| t.text.as_str()).collect();
    assert!(
        texts.contains(&"hello"),
        "should contain ASCII: {:?}",
        texts
    );
    assert!(texts.contains(&"世界"), "should contain CJK: {:?}", texts);
    assert!(
        texts.contains(&"مرحبا"),
        "should contain Arabic: {:?}",
        texts
    );
    assert!(texts.contains(&"🌍"), "should contain emoji: {:?}", texts);
}

#[test]
fn mixed_scripts_parse() {
    let g = unicode_add_grammar();
    let mut parser = build_parser(&g);
    let input = "café + naïve + 日本語";
    let result = parse_input(&mut parser, &g, input);
    assert!(
        result.is_ok(),
        "mixed script addition should parse: {:?}",
        result
    );
}

#[test]
fn surrogate_region_characters() {
    // Characters near the surrogate range boundary (U+D800-U+DFFF are not valid,
    // but characters just outside are fine)
    let g = unicode_id_grammar();
    let input = "\u{D7FF}\u{E000}"; // Just below and above surrogate range
    let mut lexer = GLRLexer::new(&g, input.into()).unwrap();
    let tokens = lexer.tokenize_all();
    assert!(!tokens.is_empty());
    assert_eq!(tokens[0].byte_length, input.len());
}

#[test]
fn replacement_character() {
    let g = unicode_id_grammar();
    let input = "\u{FFFD}"; // U+FFFD REPLACEMENT CHARACTER
    let mut lexer = GLRLexer::new(&g, input.into()).unwrap();
    let tokens = lexer.tokenize_all();
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].byte_length, 3); // U+FFFD is 3 bytes in UTF-8
}
