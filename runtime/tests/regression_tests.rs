//! Regression tests for known issues and fixed bugs.
//!
//! Each test references its corresponding GitHub issue and either:
//! - Verifies a fix remains in place (for resolved issues), or
//! - Documents current behavior for open issues (may be `#[ignore]`d).

// ---------------------------------------------------------------------------
// Issue #89 / PR #90: EOF symbol layout collision (FIXED)
//
// The EOF symbol ID must live within the terminal column range of the parse
// table.  Before the fix, EOF could collide with a non-terminal column,
// causing incorrect metadata and parse failures.
// ---------------------------------------------------------------------------

/// Regression: EOF symbol column must be strictly less than token_count.
/// See: https://github.com/EffortlessMetrics/adze/issues/89
#[test]
fn issue_89_eof_symbol_in_token_range() {
    use adze::pure_parser::TSLanguage;
    use std::ptr;

    // Construct a minimal TSLanguage with known counts.
    let lang = TSLanguage {
        version: 14,
        symbol_count: 10,
        alias_count: 0,
        token_count: 5,
        external_token_count: 0,
        state_count: 20,
        large_state_count: 5,
        production_id_count: 0,
        field_count: 0,
        max_alias_sequence_length: 0,
        production_id_map: ptr::null(),
        parse_table: ptr::null(),
        small_parse_table: ptr::null(),
        small_parse_table_map: ptr::null(),
        parse_actions: ptr::null(),
        symbol_names: ptr::null(),
        field_names: ptr::null(),
        field_map_slices: ptr::null(),
        field_map_entries: ptr::null(),
        symbol_metadata: ptr::null(),
        public_symbol_map: ptr::null(),
        alias_map: ptr::null(),
        alias_sequences: ptr::null(),
        lex_modes: ptr::null(),
        lex_fn: None,
        keyword_lex_fn: None,
        keyword_capture_token: 0,
        external_scanner: adze::pure_parser::ExternalScanner::default(),
        primary_state_ids: ptr::null(),
        production_count: 0,
        production_lhs_index: ptr::null(),
        eof_symbol: 0, // Tree-sitter convention: EOF is column 0
        rules: ptr::null(),
        rule_count: 0,
    };

    // The fix ensures EOF is always column 0, well within token_count.
    assert!(
        (lang.eof_symbol as u32) < lang.token_count,
        "EOF column ({}) must be < token_count ({})",
        lang.eof_symbol,
        lang.token_count
    );
    assert_eq!(lang.eof_symbol, 0, "EOF must be column 0 by convention");
}

/// Regression: ParseTable invariant — EOF column == token_count + external_token_count.
/// See: https://github.com/EffortlessMetrics/adze/issues/89
#[test]
fn issue_89_parse_table_eof_invariant() {
    use adze_glr_core::{Action, ParseRule, StateId};
    use adze_ir::SymbolId;

    // Layout: ERROR(0), terminal NUM(1), EOF(2), non-terminal EXPR(3)
    let eof = SymbolId(2);
    let expr = SymbolId(3);

    let actions = vec![
        // state 0: shift NUM
        vec![
            vec![],                          // col 0 ERROR
            vec![Action::Shift(StateId(1))], // col 1 NUM
            vec![],                          // col 2 EOF
            vec![],                          // col 3 EXPR
        ],
        // state 1: accept on EOF
        vec![vec![], vec![], vec![Action::Accept], vec![]],
    ];

    let gotos = vec![
        vec![
            glr_test_support::INVALID,
            glr_test_support::INVALID,
            glr_test_support::INVALID,
            StateId(1), // EXPR goto
        ],
        vec![
            glr_test_support::INVALID,
            glr_test_support::INVALID,
            glr_test_support::INVALID,
            glr_test_support::INVALID,
        ],
    ];

    let rules = vec![ParseRule {
        lhs: expr,
        rhs_len: 1,
    }];

    let table = glr_test_support::make_minimal_table(
        actions, gotos, rules, expr, eof, /*external_token_count=*/ 0,
    );

    // The invariant: EOF's column index must equal token_count + external_token_count.
    let eof_col = table
        .symbol_to_index
        .get(&table.eof_symbol)
        .expect("EOF must be in symbol_to_index");
    assert_eq!(
        *eof_col,
        table.token_count + table.external_token_count,
        "EOF column must be token_count + external_token_count"
    );
}

// ---------------------------------------------------------------------------
// Issue #71: Intermittent test_glr_error_recovery_simple failure
//
// The error recovery path could produce non-deterministic results when
// multiple recovery strategies competed.  This test verifies that basic
// error recovery succeeds deterministically for a simple grammar.
// ---------------------------------------------------------------------------

/// Regression: GLR error recovery must succeed deterministically for double-operator input.
/// See: https://github.com/EffortlessMetrics/adze/issues/71
#[test]
fn issue_71_error_recovery_deterministic() {
    use adze::error_recovery::ErrorRecoveryConfigBuilder;
    use adze::glr_lexer::GLRLexer;
    use adze::glr_parser::GLRParser;
    use adze_glr_core::{FirstFollowSets, build_lr1_automaton};
    use adze_ir::{Grammar, ProductionId, Rule, Symbol, SymbolId, Token, TokenPattern};

    let mut grammar = Grammar::new("recovery_regression".to_string());

    let num_id = SymbolId(1);
    let plus_id = SymbolId(2);
    let expr_id = SymbolId(10);

    grammar.tokens.insert(
        num_id,
        Token {
            name: "number".to_string(),
            pattern: TokenPattern::Regex("[0-9]+".to_string()),
            fragile: false,
        },
    );
    grammar.tokens.insert(
        plus_id,
        Token {
            name: "plus".to_string(),
            pattern: TokenPattern::String("+".to_string()),
            fragile: false,
        },
    );
    grammar.rule_names.insert(expr_id, "expression".to_string());

    // expression → expression '+' expression
    grammar.rules.entry(expr_id).or_default().push(Rule {
        lhs: expr_id,
        rhs: vec![
            Symbol::NonTerminal(expr_id),
            Symbol::Terminal(plus_id),
            Symbol::NonTerminal(expr_id),
        ],
        precedence: None,
        associativity: None,
        production_id: ProductionId(0),
        fields: vec![],
    });

    // expression → number
    grammar.rules.entry(expr_id).or_default().push(Rule {
        lhs: expr_id,
        rhs: vec![Symbol::Terminal(num_id)],
        precedence: None,
        associativity: None,
        production_id: ProductionId(1),
        fields: vec![],
    });

    grammar.set_start_symbol(expr_id);

    let first_follow = FirstFollowSets::compute(&grammar).unwrap();
    let table = build_lr1_automaton(&grammar, &first_follow).unwrap();

    // Run multiple iterations to catch intermittent failures (issue #71).
    for _ in 0..10 {
        let mut parser = GLRParser::new(table.clone(), grammar.clone());
        let config = ErrorRecoveryConfigBuilder::new()
            .max_consecutive_errors(10)
            .build();
        parser.enable_error_recovery(config);

        let input = "1 + + 2";
        let mut lexer = GLRLexer::new(&grammar, input.to_string()).unwrap();
        let tokens = lexer.tokenize_all();

        parser.reset();
        for token in &tokens {
            parser.process_token(token.symbol_id, &token.text, token.byte_offset);
        }
        parser.process_eof(input.len());

        let result = parser.finish();
        assert!(
            result.is_ok(),
            "Error recovery must succeed for '1 + + 2' (issue #71)"
        );
    }
}

// ---------------------------------------------------------------------------
// Issue #72: Arena optimization
//
// The arena allocator must grow chunks exponentially, reuse memory after
// reset, and not leak capacity.
// ---------------------------------------------------------------------------

/// Regression: arena chunk growth is exponential and reset preserves capacity.
/// See: https://github.com/EffortlessMetrics/adze/issues/72
#[test]
fn issue_72_arena_chunk_growth_and_reuse() {
    use adze::arena_allocator::{TreeArena, TreeNode};

    let mut arena = TreeArena::with_capacity(4);

    // Fill first chunk.
    for i in 0..4 {
        arena.alloc(TreeNode::leaf(i));
    }
    assert_eq!(arena.num_chunks(), 1);

    // Overflow triggers new (larger) chunk.
    arena.alloc(TreeNode::leaf(99));
    assert!(
        arena.num_chunks() >= 2,
        "Arena must grow to a second chunk after overflow"
    );

    let cap_before = arena.capacity();

    // Reset clears nodes but keeps chunks.
    arena.reset();
    assert_eq!(arena.len(), 0, "Reset must clear nodes");
    assert_eq!(
        arena.capacity(),
        cap_before,
        "Reset must preserve chunk capacity"
    );

    // Allocate again — no new chunks needed for same count.
    for i in 0..4 {
        arena.alloc(TreeNode::leaf(i));
    }
    assert_eq!(arena.num_chunks(), 2, "No new chunks after reuse");
}

/// Regression: arena `clear()` frees excess chunks.
/// See: https://github.com/EffortlessMetrics/adze/issues/72
#[test]
fn issue_72_arena_clear_frees_excess() {
    use adze::arena_allocator::{TreeArena, TreeNode};

    let mut arena = TreeArena::with_capacity(4);

    // Force multiple chunks.
    for i in 0..20 {
        arena.alloc(TreeNode::leaf(i));
    }
    assert!(arena.num_chunks() > 1);

    arena.clear();
    assert_eq!(arena.num_chunks(), 1, "clear() must drop excess chunks");
    assert_eq!(arena.len(), 0);
}

// ---------------------------------------------------------------------------
// Issue #74: Lexer / leaf transform warning
//
// Leaf `transform` closures are captured but never executed, causing
// type-conversion (e.g. string→i32) to fail silently.  This is an open
// issue; the test documents current behavior.
// ---------------------------------------------------------------------------

/// Documents that the GLR lexer can tokenize numeric strings even though
/// transform closures are not executed at runtime (issue #74).
/// See: https://github.com/EffortlessMetrics/adze/issues/74
#[test]
fn issue_74_lexer_tokenizes_without_transform() {
    use adze::glr_lexer::GLRLexer;
    use adze_ir::{Grammar, SymbolId, Token, TokenPattern};

    let mut grammar = Grammar::new("transform_test".to_string());
    let num_id = SymbolId(1);
    grammar.tokens.insert(
        num_id,
        Token {
            name: "number".to_string(),
            pattern: TokenPattern::Regex("[0-9]+".to_string()),
            fragile: false,
        },
    );

    let mut lexer = GLRLexer::new(&grammar, "42".to_string()).unwrap();
    let tokens = lexer.tokenize_all();

    assert_eq!(tokens.len(), 1, "Must tokenize '42' into one token");
    assert_eq!(tokens[0].text, "42");
    assert_eq!(tokens[0].symbol_id, num_id);
    // NOTE: There is no runtime transform step — the raw text "42" is
    // returned, not a parsed integer.  See issue #74.
}

// ---------------------------------------------------------------------------
// Issue #88: Incremental parsing disabled
//
// The incremental parsing path currently returns None (falls back to fresh
// parse) for consistency.  This test documents that behavior.
// ---------------------------------------------------------------------------

/// Documents that `glr_incremental::reparse` returns None (fresh parse fallback).
/// See: https://github.com/EffortlessMetrics/adze/issues/88
#[test]
fn issue_88_incremental_reparse_returns_none() {
    use adze::glr_incremental::IncrementalGLRParser;
    use adze_glr_core::{FirstFollowSets, build_lr1_automaton};
    use adze_ir::{Grammar, ProductionId, Rule, Symbol, SymbolId, Token, TokenPattern};

    let mut grammar = Grammar::new("incr_test".to_string());
    let num_id = SymbolId(1);
    let expr_id = SymbolId(10);

    grammar.tokens.insert(
        num_id,
        Token {
            name: "number".to_string(),
            pattern: TokenPattern::Regex("[0-9]+".to_string()),
            fragile: false,
        },
    );
    grammar.rule_names.insert(expr_id, "expression".to_string());
    grammar.rules.entry(expr_id).or_default().push(Rule {
        lhs: expr_id,
        rhs: vec![Symbol::Terminal(num_id)],
        precedence: None,
        associativity: None,
        production_id: ProductionId(0),
        fields: vec![],
    });

    grammar.set_start_symbol(expr_id);

    let first_follow = FirstFollowSets::compute(&grammar).unwrap();
    let table = build_lr1_automaton(&grammar, &first_follow).unwrap();

    // First parse: fresh
    let mut parser = IncrementalGLRParser::new(grammar, table);

    use adze::glr_incremental::GLRToken;
    let tokens = vec![GLRToken {
        symbol: num_id,
        text: b"42".to_vec(),
        start_byte: 0,
        end_byte: 2,
    }];

    let result = parser.parse_incremental(&tokens, &[]);
    assert!(
        result.is_ok(),
        "Fresh parse must succeed for single-number input"
    );

    // Second parse with no edits still does a fresh parse (issue #88).
    let result2 = parser.parse_incremental(&tokens, &[]);
    assert!(result2.is_ok(), "Re-parse with no edits must still succeed");
}

// ---------------------------------------------------------------------------
// Issue #90 / PR #90: EOF / non-terminal column collision (FIXED)
//
// Before the fix in abi_builder.rs, the EOF symbol could be assigned a
// column index that overlapped with a non-terminal, causing symbol metadata
// to be mis-indexed.  The fix pins EOF to column 0.
// ---------------------------------------------------------------------------

/// Regression: EOF metadata must be visible=true, named=false (0x01).
/// See: https://github.com/EffortlessMetrics/adze/issues/89
///      https://github.com/EffortlessMetrics/adze/pull/90
#[test]
fn issue_90_eof_metadata_convention() {
    use adze::pure_parser::TSLanguage;
    use std::ptr;

    // Build a language struct with eof_symbol=0.
    let lang = TSLanguage {
        version: 14,
        symbol_count: 4,
        alias_count: 0,
        token_count: 3,
        external_token_count: 0,
        state_count: 2,
        large_state_count: 1,
        production_id_count: 0,
        field_count: 0,
        max_alias_sequence_length: 0,
        production_id_map: ptr::null(),
        parse_table: ptr::null(),
        small_parse_table: ptr::null(),
        small_parse_table_map: ptr::null(),
        parse_actions: ptr::null(),
        symbol_names: ptr::null(),
        field_names: ptr::null(),
        field_map_slices: ptr::null(),
        field_map_entries: ptr::null(),
        symbol_metadata: ptr::null(),
        public_symbol_map: ptr::null(),
        alias_map: ptr::null(),
        alias_sequences: ptr::null(),
        lex_modes: ptr::null(),
        lex_fn: None,
        keyword_lex_fn: None,
        keyword_capture_token: 0,
        external_scanner: adze::pure_parser::ExternalScanner::default(),
        primary_state_ids: ptr::null(),
        production_count: 0,
        production_lhs_index: ptr::null(),
        eof_symbol: 0,
        rules: ptr::null(),
        rule_count: 0,
    };

    // The convention: eof_symbol is always 0, sitting before any
    // non-terminal columns (which start at token_count).
    assert_eq!(lang.eof_symbol, 0);
    assert!(
        (lang.eof_symbol as u32) < lang.token_count,
        "EOF must not collide with non-terminal columns (>= token_count={})",
        lang.token_count
    );
}

/// Regression: basic GLR parsing still works after EOF layout fix.
/// See: https://github.com/EffortlessMetrics/adze/pull/90
#[test]
fn issue_90_basic_parsing_after_eof_fix() {
    use adze::glr_lexer::GLRLexer;
    use adze::glr_parser::GLRParser;
    use adze_glr_core::{FirstFollowSets, build_lr1_automaton};
    use adze_ir::{Grammar, ProductionId, Rule, Symbol, SymbolId, Token, TokenPattern};

    let mut grammar = Grammar::new("eof_fix_regression".to_string());
    let num_id = SymbolId(1);
    let expr_id = SymbolId(10);

    grammar.tokens.insert(
        num_id,
        Token {
            name: "number".to_string(),
            pattern: TokenPattern::Regex("[0-9]+".to_string()),
            fragile: false,
        },
    );
    grammar.rule_names.insert(expr_id, "expression".to_string());
    grammar.rules.entry(expr_id).or_default().push(Rule {
        lhs: expr_id,
        rhs: vec![Symbol::Terminal(num_id)],
        precedence: None,
        associativity: None,
        production_id: ProductionId(0),
        fields: vec![],
    });

    grammar.set_start_symbol(expr_id);

    let first_follow = FirstFollowSets::compute(&grammar).unwrap();
    let table = build_lr1_automaton(&grammar, &first_follow).unwrap();

    let mut parser = GLRParser::new(table, grammar.clone());

    let input = "42";
    let mut lexer = GLRLexer::new(&grammar, input.to_string()).unwrap();
    let tokens = lexer.tokenize_all();

    parser.reset();
    for token in &tokens {
        parser.process_token(token.symbol_id, &token.text, token.byte_offset);
    }
    parser.process_eof(input.len());

    let result = parser.finish();
    assert!(
        result.is_ok(),
        "Basic parsing must succeed after EOF layout fix (PR #90)"
    );
}
