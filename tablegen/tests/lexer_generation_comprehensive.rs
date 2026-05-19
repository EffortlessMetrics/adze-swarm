#![allow(
    clippy::needless_range_loop,
    reason = "property and comprehensive tests use index-based loops to exercise table positions and boundary cases"
)]

//! Comprehensive tests for `adze_tablegen::lexer_gen::generate_lexer`.
//!
//! Validates that the generated `lexer_fn` token stream handles keywords,
//! string literals, regex patterns, deduplication, ordering, and edge cases.

use adze_ir::{Grammar, SymbolId, Token, TokenPattern};
use adze_tablegen::lexer_gen::generate_lexer;
use std::collections::BTreeMap;

// ── Helpers ──────────────────────────────────────────────────────────

/// Build a grammar with given tokens and a trivial symbol_to_index map.
fn grammar_with_tokens(
    tokens: Vec<(u16, &str, TokenPattern)>,
) -> (Grammar, BTreeMap<SymbolId, usize>) {
    let mut grammar = Grammar::new("test".to_string());
    let mut symbol_to_index = BTreeMap::new();

    for (id, name, pattern) in tokens {
        grammar.tokens.insert(
            SymbolId(id),
            Token {
                name: name.to_string(),
                pattern,
                fragile: false,
            },
        );
        symbol_to_index.insert(SymbolId(id), id as usize);
    }

    (grammar, symbol_to_index)
}

/// Generate the lexer and return its string representation.
fn generate(tokens: Vec<(u16, &str, TokenPattern)>) -> String {
    let (grammar, map) = grammar_with_tokens(tokens);
    generate_lexer(&grammar, &map).to_string()
}

// ── 1. Empty grammar ────────────────────────────────────────────────

#[test]
fn empty_grammar_produces_lexer_fn() {
    let code = generate(vec![]);
    assert!(code.contains("lexer_fn"), "should define lexer_fn");
    assert!(code.contains("false"), "empty lexer should return false");
}

// ── 2. Single keyword ──────────────────────────────────────────────

#[test]
fn single_keyword_generates_match() {
    let code = generate(vec![(1, "if_kw", TokenPattern::String("if".into()))]);
    assert!(code.contains("result_symbol"), "should set result_symbol");
    assert!(code.contains("mark_end"), "should call mark_end");
}

// ── 3. Multiple keywords sorted longest-first ──────────────────────

#[test]
fn keywords_sorted_longest_first() {
    let code = generate(vec![
        (1, "in_kw", TokenPattern::String("in".into())),
        (2, "int_kw", TokenPattern::String("int".into())),
        (3, "interface_kw", TokenPattern::String("interface".into())),
    ]);
    // "interface" (9 chars) should appear before "int" (3 chars) and "in" (2 chars)
    let pos_interface = code.find("result_symbol = 3u16").expect("interface match");
    let pos_int = code.find("result_symbol = 2u16").expect("int match");
    let pos_in = code.find("result_symbol = 1u16").expect("in match");
    assert!(
        pos_interface < pos_int,
        "interface should precede int in output"
    );
    assert!(pos_int < pos_in, "int should precede in in output");
}

// ── 4. Single-char string token ────────────────────────────────────

#[test]
fn single_char_string_uses_direct_lookahead() {
    let code = generate(vec![(1, "plus", TokenPattern::String("+".into()))]);
    // Single-char pattern uses a simple `lookahead == ch` without closure
    assert!(code.contains("result_symbol"), "should set result_symbol");
    // '+' is 43
    assert!(code.contains("43u32"), "should compare against '+' (43)");
}

// ── 5. Multi-char non-keyword string ───────────────────────────────

#[test]
fn multi_char_non_keyword_string() {
    // Contains digits, so not classified as keyword
    let code = generate(vec![(1, "arrow", TokenPattern::String("=>".into()))]);
    assert!(code.contains("result_symbol"), "should produce a match arm");
}

// ── 6. Digit regex (\d+) ──────────────────────────────────────────

#[test]
fn digit_regex_generates_loop() {
    let code = generate(vec![(1, "number", TokenPattern::Regex(r"\d+".into()))]);
    assert!(
        code.contains("is_ascii_digit"),
        "digit regex should use is_ascii_digit"
    );
}

// ── 7. Word regex (\w+) ───────────────────────────────────────────

#[test]
fn word_regex_generates_alphanumeric_loop() {
    let code = generate(vec![(1, "word", TokenPattern::Regex(r"\w+".into()))]);
    assert!(
        code.contains("is_ascii_alphanumeric"),
        "word regex should use is_ascii_alphanumeric"
    );
}

#[test]
fn lowercase_alpha_regex_generates_lowercase_loop() {
    let code = generate(vec![(1, "ident", TokenPattern::Regex(r"[a-z]+".into()))]);
    assert!(
        code.contains("is_ascii_lowercase"),
        "[a-z]+ regex should use is_ascii_lowercase"
    );
    assert!(
        code.contains("result_symbol = 1u16"),
        "[a-z]+ regex should set the mapped result symbol"
    );
}

// ── 8. Whitespace regex (\s+) ─────────────────────────────────────

#[test]
fn whitespace_regex_generates_whitespace_loop() {
    let code = generate(vec![(1, "ws", TokenPattern::Regex(r"\s+".into()))]);
    assert!(
        code.contains("is_ascii_whitespace"),
        "whitespace regex should use is_ascii_whitespace"
    );
}

// ── 9. Whitespace regex variant (\s) ──────────────────────────────

#[test]
fn whitespace_single_regex_also_works() {
    let code = generate(vec![(1, "ws", TokenPattern::Regex(r"\s".into()))]);
    assert!(
        code.contains("is_ascii_whitespace"),
        r"\s should also generate whitespace matching"
    );
}

// ── 10. Whitespace regex variant (\s*) ────────────────────────────

#[test]
fn whitespace_star_regex_also_works() {
    let code = generate(vec![(1, "ws", TokenPattern::Regex(r"\s*".into()))]);
    assert!(
        code.contains("is_ascii_whitespace"),
        r"\s* should also generate whitespace matching"
    );
}

// ── 11. Operator character class regex ────────────────────────────

#[test]
fn operator_char_class_regex() {
    let code = generate(vec![(1, "op", TokenPattern::Regex(r"[-+*/]".into()))]);
    // Should match '-', '+', '*', '/'
    assert!(
        code.contains("b'-'") || code.contains("45"),
        "should match minus"
    );
    assert!(
        code.contains("b'+'") || code.contains("43"),
        "should match plus"
    );
}

// ── 12. Identifier regex ─────────────────────────────────────────

#[test]
fn identifier_regex_is_emitted_last() {
    let code = generate(vec![
        (1, "number", TokenPattern::Regex(r"\d+".into())),
        (
            2,
            "ident",
            TokenPattern::Regex(r"[a-zA-Z_][a-zA-Z0-9_]*".into()),
        ),
    ]);
    // Identifier should be after digit regex
    let pos_digit = code.find("is_ascii_digit").expect("digit pattern");
    let pos_ident = code
        .find("is_ascii_alphabetic")
        .expect("identifier pattern");
    assert!(
        pos_digit < pos_ident,
        "identifier pattern should come after digit pattern"
    );
}

// ── 13. Duplicate string patterns are deduplicated ────────────────

#[test]
fn duplicate_string_patterns_deduplicated() {
    let code = generate(vec![
        (1, "plus1", TokenPattern::String("+".into())),
        (2, "plus2", TokenPattern::String("+".into())),
    ]);
    let count = code.matches("result_symbol").count();
    // Only one match for '+', not two
    assert_eq!(count, 1, "duplicate string patterns should be deduplicated");
}

// ── 14. Duplicate regex patterns are deduplicated ─────────────────

#[test]
fn duplicate_regex_patterns_deduplicated() {
    let code = generate(vec![
        (1, "num1", TokenPattern::Regex(r"\d+".into())),
        (2, "num2", TokenPattern::Regex(r"\d+".into())),
    ]);
    let count = code.matches("is_ascii_digit").count();
    // Only one set of digit-checking code
    assert!(count > 0, "should have digit matching");
    // The second duplicate should not add another block
    let result_count = code.matches("result_symbol").count();
    assert_eq!(result_count, 1, "duplicate regex should be deduplicated");
}

// ── 15. Named tokens take priority over auto-generated names ──────

#[test]
fn named_tokens_processed_before_auto_generated() {
    // Named token "plus" should appear before auto-generated "_42"
    let code = generate(vec![
        (10, "_42", TokenPattern::String("+".into())),
        (11, "plus", TokenPattern::String("-".into())),
    ]);
    // "plus" (named) should be processed first; "-" is 45, "+" is 43
    let pos_minus = code.find("45u32").expect("minus char");
    let pos_plus = code.find("43u32").expect("plus char");
    assert!(
        pos_minus < pos_plus,
        "named token '-' should appear before auto-generated '+'"
    );
}

// ── 16. Keyword word-boundary checking ────────────────────────────

#[test]
fn keyword_has_word_boundary_check() {
    let code = generate(vec![(
        1,
        "return_kw",
        TokenPattern::String("return".into()),
    )]);
    // Keywords check that the next char is not alphanumeric or underscore
    assert!(
        code.contains("is_ascii_alphanumeric"),
        "keyword should check word boundary"
    );
}

// ── 17. Null pointer check ────────────────────────────────────────

#[test]
fn null_pointer_guard_in_output() {
    let code = generate(vec![]);
    assert!(
        code.contains("is_null"),
        "should guard against null state_ptr"
    );
}

// ── 18. Lexer signature ───────────────────────────────────────────

#[test]
fn lexer_fn_has_correct_signature() {
    let code = generate(vec![]);
    assert!(
        code.contains("unsafe extern \"C\" fn lexer_fn"),
        "should be unsafe extern C"
    );
    assert!(
        code.contains("state_ptr"),
        "should take state_ptr parameter"
    );
    assert!(
        code.contains("_lex_mode"),
        "should take _lex_mode parameter"
    );
    assert!(code.contains("-> bool"), "should return bool");
}

// ── 19. Mixed token types maintain correct order ──────────────────

#[test]
fn mixed_tokens_order_keywords_then_strings_then_regex_then_ident() {
    let code = generate(vec![
        (
            1,
            "ident",
            TokenPattern::Regex(r"[a-zA-Z_][a-zA-Z0-9_]*".into()),
        ),
        (2, "number", TokenPattern::Regex(r"\d+".into())),
        (3, "if_kw", TokenPattern::String("if".into())),
        (4, "plus", TokenPattern::String("+".into())),
    ]);
    // Order: keywords, then other strings, then regex patterns, then identifier
    let pos_kw = code.find("result_symbol = 3u16").expect("keyword match");
    let pos_plus = code.find("result_symbol = 4u16").expect("plus match");
    let pos_number = code.find("result_symbol = 2u16").expect("number match");
    let pos_ident = code.find("result_symbol = 1u16").expect("ident match");

    assert!(pos_kw < pos_plus, "keyword before string");
    assert!(pos_plus < pos_number, "string before regex");
    assert!(pos_number < pos_ident, "regex before identifier");
}

// ── 20. Token not in symbol_to_index is skipped ───────────────────

#[test]
fn token_not_in_symbol_map_is_skipped() {
    let mut grammar = Grammar::new("test".to_string());
    grammar.tokens.insert(
        SymbolId(1),
        Token {
            name: "plus".to_string(),
            pattern: TokenPattern::String("+".into()),
            fragile: false,
        },
    );
    // Empty map — symbol 1 is NOT in symbol_to_index
    let map = BTreeMap::new();
    let code = generate_lexer(&grammar, &map).to_string();
    // Should still produce lexer_fn but with no match arms
    assert!(code.contains("lexer_fn"));
    assert_eq!(
        code.matches("result_symbol").count(),
        0,
        "unmapped token should not produce match"
    );
}

// ── 21. Multiple single-char tokens ───────────────────────────────

#[test]
fn multiple_single_char_tokens() {
    let code = generate(vec![
        (1, "lparen", TokenPattern::String("(".into())),
        (2, "rparen", TokenPattern::String(")".into())),
        (3, "comma", TokenPattern::String(",".into())),
    ]);
    // '(' = 40, ')' = 41, ',' = 44
    assert!(code.contains("40u32"), "should match '('");
    assert!(code.contains("41u32"), "should match ')'");
    assert!(code.contains("44u32"), "should match ','");
}

// ── 22. Keyword advance calls match length ────────────────────────

#[test]
fn keyword_advance_calls_match_keyword_length() {
    let code = generate(vec![(1, "for_kw", TokenPattern::String("for".into()))]);
    // "for" has 3 bytes, so there should be 3 advance calls in the keyword closure
    // Each byte becomes a lookahead check + advance
    let advance_count = code.matches("advance").count();
    // At least 3 advance calls for the 3-char keyword
    assert!(
        advance_count >= 3,
        "3-char keyword needs at least 3 advance calls, got {advance_count}"
    );
}

// ── 23. Multi-char string non-keyword advance ─────────────────────

#[test]
fn multi_char_nonkeyword_advance_calls() {
    // "=>" has digits/punctuation, not all alphabetic, length 2
    let code = generate(vec![(1, "arrow", TokenPattern::String("=>".into()))]);
    // Should have 2 lookahead checks for '=' and '>'
    // '=' = 61, '>' = 62
    assert!(code.contains("61u32"), "should check for '='");
    assert!(code.contains("62u32"), "should check for '>'");
}

// ── 24. Unrecognized regex produces no match ──────────────────────

#[test]
fn unrecognized_regex_produces_no_extra_match() {
    let code = generate(vec![(
        1,
        "custom",
        TokenPattern::Regex(r"[0-9a-f]+".into()),
    )]);
    // The function only handles specific known patterns (\d+, \w+, \s+, etc.)
    assert_eq!(
        code.matches("result_symbol").count(),
        0,
        "unrecognized regex should not produce match"
    );
}

// ── 25. Identifier regex alone ────────────────────────────────────

#[test]
fn identifier_regex_standalone() {
    let code = generate(vec![(
        5,
        "ident",
        TokenPattern::Regex(r"[a-zA-Z_][a-zA-Z0-9_]*".into()),
    )]);
    assert!(
        code.contains("is_ascii_alphabetic"),
        "should check alpha start"
    );
    assert!(
        code.contains("is_ascii_alphanumeric"),
        "should loop on alphanumeric"
    );
    assert!(code.contains("result_symbol = 5u16"), "should use symbol 5");
}

// ── 26. Keywords with underscores are still keywords ──────────────

#[test]
fn keyword_with_underscore_classified_as_keyword() {
    // "my_func" is all alphabetic + underscore, length > 1 → keyword
    let code = generate(vec![(1, "kw", TokenPattern::String("my_func".into()))]);
    // Keywords have word-boundary checking
    assert!(
        code.contains("is_ascii_alphanumeric"),
        "keyword with underscore should get word-boundary check"
    );
}

// ── 27. Single alphabetic char is NOT keyword ─────────────────────

#[test]
fn single_alpha_char_is_not_keyword() {
    // Length 1 — should be treated as a single-char string, not a keyword
    let code = generate(vec![(1, "a_tok", TokenPattern::String("a".into()))]);
    // 'a' = 97
    assert!(code.contains("97u32"), "should match 'a' as single char");
}

// ── 28. String with digit is not keyword ──────────────────────────

#[test]
fn string_with_digit_is_not_keyword() {
    // "a1" has a digit, so not classified as keyword
    let code = generate(vec![(1, "a1_tok", TokenPattern::String("a1".into()))]);
    // Should be treated as a multi-char non-keyword string
    // No word-boundary check (that's keyword-specific)
    // 'a' = 97, '1' = 49
    assert!(code.contains("97u32"), "should check 'a'");
    assert!(code.contains("49u32"), "should check '1'");
}

// ── 29. Operator regex (\[-+*/\]) matches four operators ──────────

#[test]
fn operator_regex_matches_all_operators() {
    let code = generate(vec![(1, "op", TokenPattern::Regex(r"[-+*/]".into()))]);
    // Should produce comparisons for '-', '+', '*', '/'
    assert!(
        code.contains("result_symbol = 1u16"),
        "should set result_symbol"
    );
}

// ── 30. Grammar with only regex tokens (no strings) ───────────────

#[test]
fn grammar_with_only_regex_tokens() {
    let code = generate(vec![
        (1, "number", TokenPattern::Regex(r"\d+".into())),
        (2, "ws", TokenPattern::Regex(r"\s+".into())),
    ]);
    assert!(
        code.contains("is_ascii_digit"),
        "should have digit matching"
    );
    assert!(
        code.contains("is_ascii_whitespace"),
        "should have whitespace matching"
    );
}

// ── 31. Grammar with only string tokens (no regex) ────────────────

#[test]
fn grammar_with_only_string_tokens() {
    let code = generate(vec![
        (1, "plus", TokenPattern::String("+".into())),
        (2, "if_kw", TokenPattern::String("if".into())),
    ]);
    assert!(!code.contains("is_ascii_digit"), "no regex patterns");
    assert!(code.contains("result_symbol"), "should produce matches");
}

// ── 32. Large number of tokens ────────────────────────────────────

#[test]
fn many_tokens_all_present() {
    let tokens: Vec<_> = (0u16..20)
        .map(|i| {
            let ch = (b'a' + (i as u8 % 26)) as char;
            let name = format!("tok_{ch}{i}");
            let pattern = format!("{ch}{ch}");
            (i, name.as_str().to_string(), TokenPattern::String(pattern))
        })
        .collect();

    let mut grammar = Grammar::new("test".to_string());
    let mut symbol_to_index = BTreeMap::new();
    for (id, name, pattern) in &tokens {
        grammar.tokens.insert(
            SymbolId(*id),
            Token {
                name: name.clone(),
                pattern: pattern.clone(),
                fragile: false,
            },
        );
        symbol_to_index.insert(SymbolId(*id), *id as usize);
    }
    let code = generate_lexer(&grammar, &symbol_to_index).to_string();
    // All 20 tokens should have result_symbol assignments
    // (2-char alphabetic strings are keywords, and they're all unique)
    let count = code.matches("result_symbol").count();
    assert_eq!(count, 20, "all 20 unique tokens should produce matches");
}

// ── 33. Deduplication prefers first occurrence (named) ────────────

#[test]
fn deduplication_uses_first_named_occurrence() {
    // Named token "plus" sorts before auto-generated "_99"
    // Both map to "+", so only the first (named) produces a match
    let code = generate(vec![
        (1, "_99", TokenPattern::String("+".into())),
        (2, "plus", TokenPattern::String("+".into())),
    ]);
    let count = code.matches("result_symbol").count();
    assert_eq!(count, 1, "deduplicated to one match");
    // Named "plus" (id=2) should be used since named tokens sort first
    assert!(
        code.contains("result_symbol = 2u16"),
        "named token should win"
    );
}

// ── 34. Identifier with keywords: keyword match precedes ident ───

#[test]
fn keyword_precedes_identifier_match() {
    let code = generate(vec![
        (
            1,
            "ident",
            TokenPattern::Regex(r"[a-zA-Z_][a-zA-Z0-9_]*".into()),
        ),
        (2, "while_kw", TokenPattern::String("while".into())),
    ]);
    // Keyword "while" should be checked before identifier
    let pos_kw = code.find("result_symbol = 2u16").expect("keyword");
    let pos_ident = code.find("result_symbol = 1u16").expect("identifier");
    assert!(pos_kw < pos_ident, "keyword before identifier");
}

// ── 35. Lexer returns false at end ────────────────────────────────

#[test]
fn lexer_returns_false_at_end() {
    let code = generate(vec![(1, "plus", TokenPattern::String("+".into()))]);
    // The function body ends with `false`
    assert!(
        code.contains("false"),
        "lexer should return false as fallback"
    );
}
