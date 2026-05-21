//! Property-based tests for the `adze_tablegen::generate` module.
//!
//! Properties verified:
//!  1.  generate_language is deterministic (same input → same output)
//!  2.  ABI version is always 15
//!  3.  token_count == grammar.tokens.len() + 1 (EOF)
//!  4.  external_token_count == grammar.externals.len()
//!  5.  field_count == grammar.fields.len()
//!  6.  field_names null iff field_count == 0
//!  7.  field_names first entry is the first real sorted field name when fields exist
//!  8.  symbol_names is never null
//!  9.  symbol_metadata starts with invisible unnamed EOF
//! 10.  hidden tokens (_prefix) have visible == false
//! 11.  generate_language_code output is deterministic
//! 12.  generate_language_code always contains "language" and "TSLanguage"
//! 13.  symbol_metadata length follows the parse-table symbol count
//! 14.  small_parse_table is non-null after generation
//! 15.  set_start_can_be_empty does not affect generated ABI version
//! 16.  multiple externals are all counted

use adze_glr_core::{LexMode, ParseTable};
use adze_ir::builder::GrammarBuilder;
use adze_ir::{ExternalToken, FieldId, Grammar, StateId, SymbolId, Token, TokenPattern};
use adze_tablegen::generate::LanguageBuilder;
use proptest::prelude::*;
use std::collections::{BTreeMap, BTreeSet};
use std::ffi::CStr;

// ---------------------------------------------------------------------------
// Strategies
// ---------------------------------------------------------------------------

/// Generate a valid token name (ASCII alphanumeric, non-empty, no NUL).
fn token_name_strategy() -> impl Strategy<Value = String> {
    "[a-z][a-z0-9_]{0,15}".prop_filter("must not be empty", |s| !s.is_empty())
}

/// Generate a token name starting with underscore (hidden).
fn hidden_token_name_strategy() -> impl Strategy<Value = String> {
    "_[a-z][a-z0-9_]{0,14}".prop_filter("must start with _", |s| s.starts_with('_'))
}

/// Generate a simple regex pattern for a token.
fn token_pattern_strategy() -> impl Strategy<Value = TokenPattern> {
    prop_oneof![
        "[a-z]{1,8}".prop_map(TokenPattern::String),
        Just(TokenPattern::Regex(r"\d+".to_string())),
        Just(TokenPattern::Regex(r"[a-z]+".to_string())),
    ]
}

/// Generate a field name (ASCII alpha, non-empty, no NUL).
fn field_name_strategy() -> impl Strategy<Value = String> {
    "[a-z][a-z_]{0,15}"
}

/// Build a Grammar with a random number of visible tokens.
fn grammar_with_tokens(count: usize) -> Grammar {
    let mut builder = GrammarBuilder::new("proptest");
    for i in 0..count {
        builder = builder.token(&format!("tok{i}"), &format!("t{i}"));
    }
    // Need at least one token for a valid rule
    if count == 0 {
        builder = builder.token("tok0", "t0");
    }
    builder = builder.rule("root", vec!["tok0"]).start("root");
    builder.build()
}

/// Build a Grammar with specified tokens and fields.
fn grammar_with_tokens_and_fields(token_count: usize, field_names: Vec<String>) -> Grammar {
    let mut grammar = grammar_with_tokens(token_count);
    for (i, name) in field_names.into_iter().enumerate() {
        grammar.fields.insert(FieldId(i as u16), name);
    }
    grammar
}

/// Build a Grammar with external tokens appended.
fn grammar_with_externals(base_tokens: usize, external_names: Vec<String>) -> Grammar {
    let mut grammar = grammar_with_tokens(base_tokens);
    for (i, name) in external_names.into_iter().enumerate() {
        grammar.externals.push(ExternalToken {
            name,
            symbol_id: SymbolId(200 + i as u16),
        });
    }
    grammar
}

/// Build a Grammar with hidden tokens.
fn grammar_with_hidden_tokens(visible: usize, hidden_names: Vec<String>) -> Grammar {
    let mut grammar = grammar_with_tokens(visible);
    for (i, name) in hidden_names.into_iter().enumerate() {
        grammar.tokens.insert(
            SymbolId(100 + i as u16),
            Token {
                name,
                pattern: TokenPattern::String("h".to_string()),
                fragile: false,
            },
        );
    }
    grammar
}

/// Read a C-string pointer into a Rust &str.
///
/// # Safety
/// `ptr` must be valid and null-terminated (from leaked CString).
unsafe fn cstr_from_ptr(ptr: *const i8) -> &'static str {
    unsafe { CStr::from_ptr(ptr).to_str().expect("valid UTF-8") }
}

fn parse_table_for_grammar(grammar: &Grammar) -> ParseTable {
    let mut seen = BTreeSet::new();
    let mut symbols = Vec::new();
    push_symbol(&mut symbols, &mut seen, SymbolId(0));

    for symbol_id in grammar.tokens.keys() {
        push_symbol(&mut symbols, &mut seen, *symbol_id);
    }
    for external in &grammar.externals {
        push_symbol(&mut symbols, &mut seen, external.symbol_id);
    }
    for symbol_id in grammar.rule_names.keys() {
        push_symbol(&mut symbols, &mut seen, *symbol_id);
    }

    let symbol_count = symbols.len();
    let symbol_to_index: BTreeMap<_, _> = symbols
        .iter()
        .copied()
        .enumerate()
        .map(|(index, symbol_id)| (symbol_id, index))
        .collect();
    let start_symbol = grammar
        .start_symbol()
        .unwrap_or_else(|| symbols.last().copied().unwrap_or(SymbolId(0)));

    ParseTable {
        action_table: vec![vec![vec![]; symbol_count]],
        goto_table: vec![vec![StateId(u16::MAX); symbol_count]],
        state_count: 1,
        symbol_count,
        symbol_to_index,
        index_to_symbol: symbols,
        eof_symbol: SymbolId(0),
        start_symbol,
        token_count: grammar.tokens.len() + 1,
        external_token_count: grammar.externals.len(),
        lex_modes: vec![LexMode {
            lex_state: 0,
            external_lex_state: 0,
        }],
        ..ParseTable::default()
    }
}

fn push_symbol(symbols: &mut Vec<SymbolId>, seen: &mut BTreeSet<SymbolId>, symbol_id: SymbolId) {
    if seen.insert(symbol_id) {
        symbols.push(symbol_id);
    }
}

// ---------------------------------------------------------------------------
// Property tests
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    // 1. generate_language is deterministic
    #[test]
    fn deterministic_generation(token_count in 1usize..6) {
        let grammar = grammar_with_tokens(token_count);
        let table = ParseTable::default();

        let b1 = LanguageBuilder::new(grammar.clone(), table.clone());
        let b2 = LanguageBuilder::new(grammar, table);

        let l1 = b1.generate_language().unwrap();
        let l2 = b2.generate_language().unwrap();

        prop_assert_eq!(l1.version, l2.version);
        prop_assert_eq!(l1.symbol_count, l2.symbol_count);
        prop_assert_eq!(l1.token_count, l2.token_count);
        prop_assert_eq!(l1.external_token_count, l2.external_token_count);
        prop_assert_eq!(l1.field_count, l2.field_count);
        prop_assert_eq!(l1.state_count, l2.state_count);
        prop_assert_eq!(l1.production_id_count, l2.production_id_count);
    }

    // 2. ABI version is always 15
    #[test]
    fn abi_version_always_15(token_count in 1usize..8) {
        let grammar = grammar_with_tokens(token_count);
        let table = ParseTable::default();
        let builder = LanguageBuilder::new(grammar, table);
        let lang = builder.generate_language().unwrap();
        prop_assert_eq!(lang.version, 15);
    }

    // 3. token_count == grammar.tokens.len() + 1 (for EOF)
    #[test]
    fn token_count_includes_eof(token_count in 1usize..10) {
        let grammar = grammar_with_tokens(token_count);
        let expected = grammar.tokens.len() as u32 + 1;
        let table = ParseTable::default();
        let builder = LanguageBuilder::new(grammar, table);
        let lang = builder.generate_language().unwrap();
        prop_assert_eq!(lang.token_count, expected);
    }

    // 4. external_token_count == grammar.externals.len()
    #[test]
    fn external_count_matches(
        ext_names in prop::collection::vec(token_name_strategy(), 0..5),
    ) {
        let expected = ext_names.len() as u32;
        let grammar = grammar_with_externals(1, ext_names);
        let table = ParseTable::default();
        let builder = LanguageBuilder::new(grammar, table);
        let lang = builder.generate_language().unwrap();
        prop_assert_eq!(lang.external_token_count, expected);
    }

    // 5. field_count == grammar.fields.len()
    #[test]
    fn field_count_matches(
        field_names in prop::collection::vec(field_name_strategy(), 0..6),
    ) {
        let expected = field_names.len() as u32;
        let grammar = grammar_with_tokens_and_fields(1, field_names);
        let table = ParseTable::default();
        let builder = LanguageBuilder::new(grammar, table);
        let lang = builder.generate_language().unwrap();
        prop_assert_eq!(lang.field_count, expected);
    }

    // 6. field_names null iff field_count == 0
    #[test]
    fn field_names_null_iff_no_fields(
        field_names in prop::collection::vec(field_name_strategy(), 0..4),
    ) {
        let has_fields = !field_names.is_empty();
        let grammar = grammar_with_tokens_and_fields(1, field_names);
        let table = ParseTable::default();
        let builder = LanguageBuilder::new(grammar, table);
        let lang = builder.generate_language().unwrap();

        if has_fields {
            prop_assert!(!lang.field_names.is_null(), "field_names should be non-null when fields exist");
        } else {
            prop_assert!(lang.field_names.is_null(), "field_names should be null when no fields");
        }
    }

    // 7. field_names first entry is the first real sorted field name when fields exist
    #[test]
    fn field_names_first_entry_empty(
        field_names in prop::collection::vec(field_name_strategy(), 1..5),
    ) {
        let mut expected = field_names.clone();
        expected.sort();
        let grammar = grammar_with_tokens_and_fields(1, field_names);
        let table = ParseTable::default();
        let builder = LanguageBuilder::new(grammar, table);
        let lang = builder.generate_language().unwrap();

        prop_assert!(!lang.field_names.is_null());
        let first = unsafe { cstr_from_ptr(*lang.field_names) };
        prop_assert_eq!(first, expected[0].as_str(), "first field name entry must be the first sorted field");
    }

    // 8. symbol_names is never null
    #[test]
    fn symbol_names_never_null(token_count in 1usize..6) {
        let grammar = grammar_with_tokens(token_count);
        let table = ParseTable::default();
        let builder = LanguageBuilder::new(grammar, table);
        let lang = builder.generate_language().unwrap();
        prop_assert!(!lang.symbol_names.is_null(), "symbol_names must never be null");
    }

    // 9. symbol_metadata starts with invisible unnamed EOF
    #[test]
    fn metadata_starts_with_eof(token_count in 1usize..6) {
        let grammar = grammar_with_tokens(token_count);
        let table = parse_table_for_grammar(&grammar);
        let builder = LanguageBuilder::new(grammar, table);
        let lang = builder.generate_language().unwrap();

        prop_assert!(!lang.symbol_metadata.is_null());
        let eof_meta = unsafe { &*lang.symbol_metadata };
        prop_assert!(!eof_meta.visible, "EOF must be invisible");
        prop_assert!(!eof_meta.named, "EOF must be unnamed");
    }

    // 10. hidden tokens (_prefix) have visible == false in metadata
    #[test]
    fn hidden_tokens_invisible(
        hidden_names in prop::collection::vec(hidden_token_name_strategy(), 1..4),
    ) {
        let hidden_count = hidden_names.len();
        let grammar = grammar_with_hidden_tokens(1, hidden_names);
        let table = parse_table_for_grammar(&grammar);
        let builder = LanguageBuilder::new(grammar.clone(), table);
        let lang = builder.generate_language().unwrap();

        // Walk metadata: first is EOF, then the grammar symbols from the parse table.
        let total_meta = lang.symbol_count as usize;
        let mut hidden_invisible_count = 0;
        for i in 0..total_meta {
            let meta = unsafe { &*lang.symbol_metadata.add(i) };
            if !meta.visible {
                hidden_invisible_count += 1;
            }
        }
        // At minimum: EOF + hidden tokens should be invisible
        prop_assert!(
            hidden_invisible_count > hidden_count,
            "expected at least {} invisible symbols (EOF + hidden), got {}",
            hidden_count + 1,
            hidden_invisible_count
        );
    }

    // 11. generate_language_code output is deterministic
    #[test]
    fn code_generation_deterministic(token_count in 1usize..5) {
        let grammar = grammar_with_tokens(token_count);
        let table = ParseTable::default();

        let b1 = LanguageBuilder::new(grammar.clone(), table.clone());
        let b2 = LanguageBuilder::new(grammar, table);

        let c1 = b1.generate_language_code().to_string();
        let c2 = b2.generate_language_code().to_string();

        prop_assert_eq!(c1, c2, "code generation must be deterministic");
    }

    // 12. generate_language_code always contains "language" and "TSLanguage"
    #[test]
    fn code_contains_required_tokens(token_count in 1usize..5) {
        let grammar = grammar_with_tokens(token_count);
        let table = ParseTable::default();
        let builder = LanguageBuilder::new(grammar, table);
        let code = builder.generate_language_code().to_string();

        prop_assert!(code.contains("language"), "code must contain 'language'");
        prop_assert!(code.contains("TSLanguage"), "code must contain 'TSLanguage'");
    }

    // 13. symbol_metadata length >= 1 (at least EOF entry)
    #[test]
    fn metadata_has_at_least_eof(token_count in 0usize..6) {
        let grammar = grammar_with_tokens(token_count);
        let table = parse_table_for_grammar(&grammar);
        let builder = LanguageBuilder::new(grammar.clone(), table);
        let lang = builder.generate_language().unwrap();

        prop_assert!(!lang.symbol_metadata.is_null());
        prop_assert!(lang.symbol_count >= 1, "metadata must have at least EOF entry");
        // The grammar-level expectation has 1 (EOF) + tokens + rules + externals entries.
        let expected_len = 1 + grammar.tokens.len() + grammar.rules.len() + grammar.externals.len();
        prop_assert_eq!(lang.symbol_count as usize, expected_len);
    }

    // 14. small_parse_table is non-null after generation
    #[test]
    fn small_parse_table_non_null(token_count in 1usize..5) {
        let grammar = grammar_with_tokens(token_count);
        let table = ParseTable::default();
        let builder = LanguageBuilder::new(grammar, table);
        let lang = builder.generate_language().unwrap();

        prop_assert!(
            !lang.small_parse_table.is_null(),
            "small_parse_table must be non-null after generation"
        );
    }

    // 15. set_start_can_be_empty does not affect ABI version or counts
    #[test]
    fn start_can_be_empty_no_abi_effect(
        token_count in 1usize..5,
        empty_flag in proptest::bool::ANY,
    ) {
        let grammar = grammar_with_tokens(token_count);
        let table = ParseTable::default();
        let mut builder = LanguageBuilder::new(grammar, table);
        builder.set_start_can_be_empty(empty_flag);
        let lang = builder.generate_language().unwrap();

        prop_assert_eq!(lang.version, 15, "ABI version must remain 15 regardless of start_can_be_empty");
    }

    // 16. multiple externals all counted correctly
    #[test]
    fn multiple_externals_counted(ext_count in 0usize..8) {
        let ext_names: Vec<String> = (0..ext_count).map(|i| format!("ext{i}")).collect();
        let grammar = grammar_with_externals(1, ext_names);
        let table = ParseTable::default();
        let builder = LanguageBuilder::new(grammar, table);
        let lang = builder.generate_language().unwrap();

        prop_assert_eq!(lang.external_token_count, ext_count as u32);
    }

    // 17. field_names array has exactly field_count real field entries
    #[test]
    fn field_names_array_length(
        field_names in prop::collection::vec(field_name_strategy(), 1..5),
    ) {
        let field_count = field_names.len();
        let mut expected = field_names.clone();
        expected.sort();
        let grammar = grammar_with_tokens_and_fields(1, field_names);
        let table = ParseTable::default();
        let builder = LanguageBuilder::new(grammar, table);
        let lang = builder.generate_language().unwrap();

        prop_assert!(!lang.field_names.is_null());
        let mut fields = Vec::with_capacity(field_count);
        for i in 0..field_count {
            let ptr = unsafe { *lang.field_names.add(i) };
            prop_assert!(!ptr.is_null(), "field_names[{}] must not be null", i);
            fields.push(unsafe { cstr_from_ptr(ptr) });
        }
        prop_assert_eq!(fields, expected);
    }

    // 18. token names with different patterns produce valid symbol_names
    #[test]
    fn different_patterns_valid_names(
        pattern in token_pattern_strategy(),
        name in token_name_strategy(),
    ) {
        let mut grammar = GrammarBuilder::new("pat_test")
            .token("base_tok", "b")
            .rule("root", vec!["base_tok"])
            .start("root")
            .build();

        grammar.tokens.insert(
            SymbolId(50),
            Token {
                name: name.clone(),
                pattern,
                fragile: false,
            },
        );

        let table = ParseTable::default();
        let builder = LanguageBuilder::new(grammar, table);
        let lang = builder.generate_language().unwrap();

        prop_assert!(!lang.symbol_names.is_null());
    }
}

// ---------------------------------------------------------------------------
// Non-proptest edge cases
// ---------------------------------------------------------------------------

#[test]
fn zero_tokens_grammar_still_succeeds() {
    // grammar_with_tokens(0) adds a fallback token, so generation should work.
    let grammar = grammar_with_tokens(0);
    let table = ParseTable::default();
    let builder = LanguageBuilder::new(grammar, table);
    let lang = builder.generate_language().unwrap();
    assert_eq!(lang.version, 15);
}

#[test]
fn many_fields_generation() {
    let field_names: Vec<String> = (0..20).map(|i| format!("field_{i}")).collect();
    let grammar = grammar_with_tokens_and_fields(2, field_names);
    let table = ParseTable::default();
    let builder = LanguageBuilder::new(grammar, table);
    let lang = builder.generate_language().unwrap();
    assert_eq!(lang.field_count, 20);
}

#[test]
fn many_externals_generation() {
    let ext_names: Vec<String> = (0..15).map(|i| format!("ext_{i}")).collect();
    let grammar = grammar_with_externals(1, ext_names);
    let table = ParseTable::default();
    let builder = LanguageBuilder::new(grammar, table);
    let lang = builder.generate_language().unwrap();
    assert_eq!(lang.external_token_count, 15);
}
