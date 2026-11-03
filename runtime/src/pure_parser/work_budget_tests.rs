//! Real LR actions exercise productive work and bounded nonprogress (#1211).
//!
//! These small, independently specified tables use the native small-table ABI.
//! Only the immutable tables are static; no result tree is leaked and no test
//! changes the thread stack or the production 10,000-iteration threshold.

use super::*;

pub(crate) const DEEP_TOKEN_COUNT: usize = 12_000;

static NAMES: [SyncPtr; 3] = [
    SyncPtr::new(c"end".as_ptr().cast()),
    SyncPtr::new(c"a".as_ptr().cast()),
    SyncPtr::new(c"sequence".as_ptr().cast()),
];
static METADATA: [u8; 3] = [0, 0x01, 0x03];
static LHS: [u16; 2] = [2, 2];
static ACTION_METADATA: [TSParseAction; 2] = [TSParseAction {
    action_type: 0,
    extra: 0,
    child_count: 0,
    dynamic_precedence: 0,
    symbol: 0,
}; 2];

unsafe extern "C" fn lex_a(raw: *mut c_void, _mode: TSLexState) -> bool {
    // SAFETY: the parser supplies its live TsLexer adapter for this callback.
    unsafe {
        let lexer = &mut *raw.cast::<crate::lex::TsLexer>();
        if (lexer.lookahead)(lexer) != u32::from(b'a') {
            return false;
        }
        (lexer.advance)(lexer, false);
        (lexer.mark_end)(lexer);
        lexer.result_symbol = 1;
        true
    }
}

const fn language(
    pairs: &'static [u16],
    offsets: &'static [u32],
    rules: &'static [TSRule],
) -> TSLanguage {
    TSLanguage {
        version: TREE_SITTER_LANGUAGE_VERSION,
        symbol_count: 3,
        alias_count: 0,
        token_count: 2,
        external_token_count: 0,
        state_count: (offsets.len() - 1) as u32,
        large_state_count: 0,
        production_id_count: rules.len() as u32,
        field_count: 0,
        max_alias_sequence_length: 0,
        production_id_map: std::ptr::null(),
        parse_table: std::ptr::null(),
        small_parse_table: pairs.as_ptr(),
        small_parse_table_map: offsets.as_ptr(),
        parse_actions: ACTION_METADATA.as_ptr(),
        symbol_names: NAMES.as_ptr().cast(),
        field_names: std::ptr::null(),
        field_map_slices: std::ptr::null(),
        field_map_entries: std::ptr::null(),
        symbol_metadata: METADATA.as_ptr(),
        public_symbol_map: std::ptr::null(),
        alias_map: std::ptr::null(),
        alias_sequences: std::ptr::null(),
        lex_modes: std::ptr::null(),
        lex_fn: Some(lex_a),
        keyword_lex_fn: None,
        keyword_capture_token: 0,
        external_scanner: ExternalScanner::default(),
        primary_state_ids: std::ptr::null(),
        production_lhs_index: LHS.as_ptr(),
        production_count: rules.len() as u16,
        eof_symbol: 0,
        rules: rules.as_ptr(),
        rule_count: rules.len() as u16,
    }
}

// S -> a | a S. All a's shift before EOF, then each binary reduction
// decreases the semantic stack by one. The selected tree is right-nested.
static EOF_UNWIND: TSLanguage = language(
    &[1, 1, 2, 3, 0, 0x8001, 1, 1, 2, 2, 0, 0x8002, 0, 0xffff],
    &[0, 4, 10, 12, 14],
    &[
        TSRule {
            lhs: 2,
            rhs_len: 1,
            _pad: 0,
        },
        TSRule {
            lhs: 2,
            rhs_len: 2,
            _pad: 0,
        },
    ],
);

// A deliberately cyclic table: S -> a, then S -> S forever at EOF.
// S is visible, so every unary reduction owns the preceding deep subtree.
static UNARY_CYCLE: TSLanguage = language(
    &[1, 1, 2, 2, 0, 0x8001, 0, 0x8002],
    &[0, 4, 6, 8],
    &[
        TSRule {
            lhs: 2,
            rhs_len: 1,
            _pad: 0,
        },
        TSRule {
            lhs: 2,
            rhs_len: 1,
            _pad: 0,
        },
    ],
);

// S -> epsilon forever. The growing stack is not progress at a fixed cursor.
static EPSILON_CYCLE: TSLanguage = language(
    &[0, 0x8001, 2, 1, 0, 0x8001, 2, 1],
    &[0, 4, 8],
    &[TSRule {
        lhs: 2,
        rhs_len: 0,
        _pad: 0,
    }],
);

pub(crate) fn eof_unwind_language() -> &'static TSLanguage {
    &EOF_UNWIND
}

pub(crate) fn unary_cycle_language() -> &'static TSLanguage {
    &UNARY_CYCLE
}

pub(crate) fn epsilon_cycle_language() -> &'static TSLanguage {
    &EPSILON_CYCLE
}

pub(crate) fn assert_sequence_tree(root: &ParsedNode, length: usize) {
    let mut sequence = root;
    for index in 0..length {
        assert_eq!(sequence.symbol, 2);
        assert_eq!(sequence.kind(), "sequence");
        assert_eq!(sequence.start_byte..sequence.end_byte, index..length);
        assert_eq!(
            sequence.start_point,
            Point {
                row: 0,
                column: index as u32
            }
        );
        assert_eq!(
            sequence.end_point,
            Point {
                row: 0,
                column: length as u32
            }
        );
        assert!(sequence.is_named);
        assert!(!(sequence.is_extra || sequence.is_error || sequence.is_missing));
        assert_eq!(sequence.field_id, None);
        assert_eq!(
            sequence.child_count(),
            if index + 1 == length { 1 } else { 2 }
        );
        let leaf = sequence.child(0).expect("one a at each depth");
        assert_eq!(leaf.symbol, 1);
        assert_eq!(leaf.kind(), "a");
        assert_eq!(leaf.start_byte..leaf.end_byte, index..index + 1);
        assert_eq!(
            leaf.start_point,
            Point {
                row: 0,
                column: index as u32
            }
        );
        assert_eq!(
            leaf.end_point,
            Point {
                row: 0,
                column: (index + 1) as u32
            }
        );
        assert!(!leaf.is_named);
        assert!(!(leaf.is_extra || leaf.is_error || leaf.is_missing));
        assert_eq!(leaf.field_id, None);
        assert_eq!(leaf.child_count(), 0);
        if index + 1 < length {
            sequence = sequence.child(1).expect("remaining sequence");
        }
    }
    assert!(!root.has_error());
}

#[test]
fn test_long_eof_unwind_preserves_tree_clone_and_parser_lifecycle() {
    let input = "a".repeat(DEEP_TOKEN_COUNT);
    let mut parser = Parser::new();
    parser.set_language(eof_unwind_language()).unwrap();
    let result = parser.parse_string(&input);
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    let root = result.root.expect("valid right-recursive input");
    assert_sequence_tree(&root, DEEP_TOKEN_COUNT);
    let cloned = root.clone();
    assert_sequence_tree(&cloned, DEEP_TOKEN_COUNT);
    drop(cloned);
    assert!(
        parser.stack.is_empty(),
        "success releases residual native stack"
    );
    parser.reset();
    let small = parser.parse_string("a");
    assert!(small.errors.is_empty());
    assert_sequence_tree(small.root.as_ref().unwrap(), 1);
    drop(parser);
    drop(root);

    // The ordinary incremental wrapper actively clones a successful root.
    // This does not qualify its separate recursive old-tree/edit traversal.
    let mut incremental = crate::pure_incremental::IncrementalParser::new();
    incremental.set_language(eof_unwind_language()).unwrap();
    let result = incremental.parse(&input, None);
    assert!(result.errors.is_empty());
    assert_sequence_tree(result.root.as_ref().unwrap(), DEEP_TOKEN_COUNT);
    drop(result);
    let small = incremental.parse("a", None);
    assert!(small.errors.is_empty());
    assert_sequence_tree(small.root.as_ref().unwrap(), 1);
    drop(incremental);
}

#[test]
fn test_actual_epsilon_and_unary_cycles_refuse_and_leave_parser_reusable() {
    let mut parser = Parser::new();
    for (language, source) in [
        (epsilon_cycle_language(), ""),
        (unary_cycle_language(), "a"),
    ] {
        parser.set_language(language).unwrap();
        let result = parser.parse_string(source);
        assert!(result.root.is_none());
        assert_eq!(result.errors.len(), 1);
        let error = &result.errors[0];
        assert_eq!(error.position, source.len());
        assert_eq!(
            error.point,
            Point {
                row: 0,
                column: source.len() as u32
            }
        );
        assert!(error.expected.is_empty());
        assert_eq!(
            error.work_limit,
            Some(crate::errors::ParserWorkLimit {
                limit: 10_000,
                completed_iterations: 10_000,
            })
        );
        assert!(
            parser.stack.is_empty(),
            "refusal releases every accumulated tree"
        );
        parser.reset();
        parser.set_language(eof_unwind_language()).unwrap();
        let clean = parser.parse_string("a");
        assert!(clean.errors.is_empty());
        assert_sequence_tree(clean.root.as_ref().unwrap(), 1);
    }
}

#[test]
fn test_clone_and_error_scan_preserve_all_native_facts() {
    let child = ParsedNode::builder(1, vec![], 2, 3)
        .start_point(Point { row: 1, column: 0 })
        .end_point(Point { row: 1, column: 1 })
        .is_extra(true)
        .is_error(true)
        .is_missing(true)
        .is_named(false)
        .field_id(Some(7))
        .build();
    let mut root = ParsedNode::builder(2, vec![child], 0, 3).build();
    root.language = Some(eof_unwind_language() as *const _);
    let cloned = root.clone();
    let mut pending = vec![(&root, &cloned)];
    while let Some((original, copy)) = pending.pop() {
        assert_eq!(original.symbol, copy.symbol);
        assert_eq!(
            original.start_byte..original.end_byte,
            copy.start_byte..copy.end_byte
        );
        assert_eq!(original.start_point, copy.start_point);
        assert_eq!(original.end_point, copy.end_point);
        assert_eq!(original.is_extra, copy.is_extra);
        assert_eq!(original.is_error, copy.is_error);
        assert_eq!(original.is_missing, copy.is_missing);
        assert_eq!(original.is_named, copy.is_named);
        assert_eq!(original.field_id, copy.field_id);
        assert_eq!(original.language, copy.language);
        assert_eq!(original.child_count(), copy.child_count());
        pending.extend(original.children().iter().zip(copy.children()));
    }
    assert!(root.has_error());
    assert!(cloned.has_error());
}

#[cfg(feature = "pure-rust")]
mod document {
    use super::*;
    use crate::__private::{EngineDispatch, engine_dispatch_for_language, parse, parse_document};
    use crate::document::ParseOutcome;
    use crate::errors::ParseErrorReason;

    #[derive(Debug)]
    struct SequenceLength(usize);

    impl crate::Extract<Self> for SequenceLength {
        type LeafFn = ();

        fn extract(
            node: Option<&ParsedNode>,
            source: &[u8],
            _last_idx: usize,
            _leaf_fn: Option<&Self::LeafFn>,
        ) -> Self {
            let root = node.expect("selected sequence");
            assert_sequence_tree(root, source.len());
            Self(source.len())
        }
    }

    #[test]
    fn test_long_eof_unwind_reaches_flat_document_and_small_typed_value() {
        let input = "a".repeat(DEEP_TOKEN_COUNT);
        assert_eq!(
            engine_dispatch_for_language(eof_unwind_language()),
            EngineDispatch::DeterministicLr
        );
        let typed = parse::<SequenceLength>(&input, eof_unwind_language)
            .unwrap_or_else(|errors| panic!("unexpected typed errors: {errors:?}"));
        assert_eq!(typed.0, DEEP_TOKEN_COUNT);
        let document = parse_document(&input, eof_unwind_language, "work_budget_sequence")
            .expect("valid sequence must produce a document");
        assert_eq!(document.outcome(), ParseOutcome::Complete);
        assert!(document.diagnostics().is_empty());
        assert_eq!(document.tree().node_count(), DEEP_TOKEN_COUNT * 2);
        let mut sequence = document.tree().root();
        for index in 0..DEEP_TOKEN_COUNT {
            assert_eq!(sequence.kind_id(), adze_ir::SymbolId(2));
            assert_eq!(sequence.byte_range(), index..DEEP_TOKEN_COUNT);
            assert!(!sequence.has_error());
            assert_eq!(
                sequence.child_count(),
                if index + 1 == DEEP_TOKEN_COUNT { 1 } else { 2 }
            );
            let leaf = sequence.child(0).expect("one a at each depth");
            assert_eq!(leaf.kind_id(), adze_ir::SymbolId(1));
            assert_eq!(leaf.byte_range(), index..index + 1);
            assert_eq!(leaf.utf8_text().unwrap(), "a");
            assert_eq!(leaf.child_count(), 0);
            if index + 1 < DEEP_TOKEN_COUNT {
                sequence = sequence.child(1).expect("remaining sequence");
            }
        }
        // Exercise flat -> temporary ParsedNode -> extraction -> normal drop.
        // The user value is a count, not an arbitrarily deep recursive AST.
        assert_eq!(
            document.ast::<SequenceLength>().unwrap().0,
            DEEP_TOKEN_COUNT
        );
        let cloned = document.clone();
        drop(document);
        assert_eq!(cloned.tree().node_count(), DEEP_TOKEN_COUNT * 2);
        drop(cloned);
    }

    #[test]
    fn test_real_work_refusal_is_hard_typed_document_error_with_prior_errors_preserved() {
        for (language, source, prior_syntax_errors) in [
            (epsilon_cycle_language(), "", 0),
            (unary_cycle_language(), "a", 0),
            (unary_cycle_language(), "!a", 1),
        ] {
            assert_eq!(
                engine_dispatch_for_language(language),
                EngineDispatch::DeterministicLr
            );
            let typed = parse::<SequenceLength>(source, || language)
                .expect_err("cyclic table must refuse before extraction");
            let document = parse_document(source, || language, "work_budget_cycle")
                .expect_err("work refusal must not synthesize a syntax document");
            assert_eq!(typed.len(), prior_syntax_errors + 1);
            assert_eq!(document.len(), typed.len());
            if prior_syntax_errors > 0 {
                for error in [&typed[0], &document[0]] {
                    // UnexpectedToken retains the complete public diagnostic,
                    // including its expected-token suffix, before the refusal.
                    assert!(
                        matches!(
                            &error.reason,
                            ParseErrorReason::UnexpectedToken(message)
                                if message == "!; expected one of: a"
                        ),
                        "{source:?}: prior syntax diagnostic was {error:?}"
                    );
                    assert_eq!(error.byte_span(), 0..1);
                    assert_eq!(error.expected, ["a"]);
                }
            }
            for error in [typed.last().unwrap(), document.last().unwrap()] {
                assert!(matches!(
                    error.reason,
                    ParseErrorReason::ParserWorkLimit(limit)
                        if limit.limit == 10_000 && limit.completed_iterations == 10_000
                ));
                assert_eq!(error.byte_span(), source.len()..source.len());
                assert!(error.expected.is_empty());
            }
        }
    }
}
