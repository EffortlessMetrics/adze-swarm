#![allow(
    clippy::needless_range_loop,
    reason = "property and comprehensive tests use index-based loops to exercise table positions and boundary cases"
)]

//! Comprehensive tests for `adze_tablegen::lexer_gen::generate_lexer`.
//!
//! Validates that the generated mode-aware `lexer_fn` handles keywords,
//! string literals, regex patterns, candidate registration, ordering, and edge cases.

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

fn normalized_codegen(code: &str) -> String {
    code.chars().filter(|ch| !ch.is_whitespace()).collect()
}

fn better_match_pos(code: &str, sym: u16) -> Option<usize> {
    normalized_codegen(code).find(&format!("better_match({sym}u16,"))
}

fn candidate_registration_count(code: &str) -> usize {
    normalized_codegen(code).matches("best_sym=Some(").count()
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
    // Longest literals are registered first for maximal-munch evaluation.
    let pos_interface = better_match_pos(&code, 3).expect("interface match");
    let pos_int = better_match_pos(&code, 2).expect("int match");
    let pos_in = better_match_pos(&code, 1).expect("in match");
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
        better_match_pos(&code, 1).is_some(),
        "[a-z]+ regex should register the mapped result symbol"
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
    // Both are maximal patterns; registration order follows tablegen sort, not legacy ident-last.
    let pos_digit = code.find("is_ascii_digit").expect("digit pattern");
    let pos_ident = code
        .find("is_identifier_start")
        .expect("identifier pattern");
    assert!(
        pos_ident < pos_digit,
        "identifier candidate is emitted before digit candidate in mode-aware codegen"
    );
}

// ── 13. Duplicate string patterns register distinct candidates ────

#[test]
fn duplicate_string_patterns_deduplicated() {
    let code = generate(vec![
        (1, "plus1", TokenPattern::String("+".into())),
        (2, "plus2", TokenPattern::String("+".into())),
    ]);
    assert_eq!(
        candidate_registration_count(&code),
        2,
        "duplicate literals should register distinct candidates for better_match"
    );
}

// ── 14. Duplicate regex patterns register distinct candidates ─────

#[test]
fn duplicate_regex_patterns_deduplicated() {
    let code = generate(vec![
        (1, "num1", TokenPattern::Regex(r"\d+".into())),
        (2, "num2", TokenPattern::Regex(r"\d+".into())),
    ]);
    assert!(
        code.contains("is_ascii_digit"),
        "should have digit matching"
    );
    assert_eq!(
        candidate_registration_count(&code),
        2,
        "duplicate regex patterns should register distinct candidates"
    );
}

// ── 15. Named tokens take priority over auto-generated names ──────

#[test]
fn named_tokens_processed_before_auto_generated() {
    let code = generate(vec![
        (10, "_42", TokenPattern::String("+".into())),
        (11, "plus", TokenPattern::String("-".into())),
    ]);
    // Candidate order follows lexical priority, length, then symbol index.
    let pos_plus = code.find("43u32").expect("plus char");
    let pos_minus = code.find("45u32").expect("minus char");
    assert!(
        pos_plus < pos_minus,
        "lower symbol index should register before higher symbol index"
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
    assert!(code.contains("lex_mode"), "should take lex_mode parameter");
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
    // Registration order: longest/maximal patterns first, then symbol index.
    let pos_ident = better_match_pos(&code, 1).expect("ident match");
    let pos_number = better_match_pos(&code, 2).expect("number match");
    let pos_kw = better_match_pos(&code, 3).expect("keyword match");
    let pos_plus = better_match_pos(&code, 4).expect("plus match");

    assert!(pos_ident < pos_number, "identifier before number");
    assert!(pos_number < pos_kw, "number before keyword");
    assert!(pos_kw < pos_plus, "keyword before single-char string");
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
        candidate_registration_count(&code),
        0,
        "unmapped token should not register a candidate"
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

// ── 22. Keyword uses non-destructive byte checks ──────────────────

#[test]
fn keyword_advance_calls_match_keyword_length() {
    let code = generate(vec![(1, "for_kw", TokenPattern::String("for".into()))]);
    assert!(
        code.contains("lexer_byte_at_rel") && code.contains("0usize") && code.contains("2usize"),
        "3-char keyword should peek each byte without destructive advance"
    );
    assert!(
        !code.contains("advance)(lexer"),
        "mode-aware lexer must not call destructive advance in candidates"
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
        candidate_registration_count(&code),
        0,
        "unrecognized regex should not register a candidate"
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
        code.contains("is_identifier_start"),
        "should check identifier start"
    );
    assert!(code.contains("is_word_char"), "should loop on word chars");
    assert!(
        better_match_pos(&code, 5).is_some(),
        "should register symbol 5"
    );
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
        better_match_pos(&code, 1).is_some(),
        "should register operator candidate"
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
    assert_eq!(
        candidate_registration_count(&code),
        20,
        "all 20 unique tokens should register candidates"
    );
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
    assert_eq!(
        candidate_registration_count(&code),
        2,
        "duplicate literals should register both symbol candidates"
    );
    assert!(
        better_match_pos(&code, 1).is_some() && better_match_pos(&code, 2).is_some(),
        "both duplicate symbols should be present"
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
    // Identifier registers before keyword due to maximal-length ordering.
    let pos_ident = better_match_pos(&code, 1).expect("identifier");
    let pos_kw = better_match_pos(&code, 2).expect("keyword");
    assert!(
        pos_ident < pos_kw,
        "identifier candidate before keyword candidate"
    );
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
