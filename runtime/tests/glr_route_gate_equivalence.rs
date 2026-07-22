//! Route-gate equivalence proof for conflicted generated grammars (#892 prerequisite).
//!
//! Compares fixed pretokenization bridge vs stack-aware streaming driver for
//! `ambiguous_expr` and `reduce_reduce` before flipping production route gates.

#![cfg(all(feature = "pure-rust", feature = "glr", feature = "runtime-e2e"))]

use adze::decoder::{decode_grammar, decode_parse_table};
use adze::glr_parser::GLRParser;
use adze::glr_streaming_runtime::parse_with_streaming_driver;
use adze::pure_parser::TSLanguage;
use adze::subtree::Subtree;
use adze::__private::{align_true_glr_parse_table_to_language_symbols, lex_with_language_fn};
use adze_glr_core::conflict_inspection::state_has_conflicts;
use adze_ir::StateId;
use std::sync::Arc;

fn parse_table_has_conflicts(language: &'static TSLanguage) -> bool {
    let table = decode_parse_table(language);
    (0..table.state_count).any(|state| state_has_conflicts(&table, StateId(state as u16)))
}

fn parse_via_fixed_bridge(
    input: &str,
    language: &'static TSLanguage,
) -> Result<Arc<Subtree>, Vec<adze::errors::ParseError>> {
    let source = input.as_bytes();
    let mut parse_table = decode_parse_table(language);
    let grammar = decode_grammar(language);
    align_true_glr_parse_table_to_language_symbols(language, &mut parse_table);
    let mut parser = GLRParser::new(parse_table, grammar);

    let lex_fn = language
        .lex_fn
        .expect("generated grammar should expose lex_fn");
    for token in lex_with_language_fn(language, lex_fn, source)? {
        parser.process_token(token.symbol_id, &token.text, token.byte_offset);
    }
    parser.process_eof(source.len());
    parser.finish().map_err(|message| {
        vec![adze::errors::ParseError {
            reason: adze::errors::ParseErrorReason::UnexpectedToken(message),
            start: 0,
            end: source.len(),
            expected: vec![],
        }]
    })
}

fn parse_via_streaming_driver(
    input: &str,
    language: &'static TSLanguage,
) -> Result<Arc<Subtree>, adze_glr_core::driver::GlrError> {
    let parse_table = decode_parse_table(language);
    let grammar = decode_grammar(language);
    parse_with_streaming_driver(input, language, parse_table, &grammar).map(|result| result.root)
}

fn subtree_shape_key(subtree: &Subtree) -> String {
    format!(
        "{}:{}:{}",
        subtree.node.symbol_id.0,
        subtree.node.byte_range.start,
        subtree.node.byte_range.end
    )
}

fn assert_bridge_streaming_subtree_equivalence(
    grammar_name: &str,
    input: &str,
    language: &'static TSLanguage,
) {
    let bridge = parse_via_fixed_bridge(input, language).unwrap_or_else(|errors| {
        panic!("{grammar_name} fixed bridge should parse {input:?}: {errors:?}")
    });
    let streaming = parse_via_streaming_driver(input, language).unwrap_or_else(|error| {
        panic!("{grammar_name} streaming driver should parse {input:?}: {error:?}")
    });

    assert_eq!(
        subtree_shape_key(&bridge),
        subtree_shape_key(&streaming),
        "{grammar_name} selected subtree shape should match between bridge and streaming for input {input:?}"
    );
    assert_eq!(
        bridge.node.byte_range, streaming.node.byte_range,
        "{grammar_name} root byte range should match for input {input:?}"
    );
}

#[test]
fn ambiguous_expr_has_conflicts_precondition() {
    let language = adze_example::ambiguous_expr::grammar::language();
    assert!(
        parse_table_has_conflicts(language),
        "ambiguous_expr must remain a conflicted generated grammar"
    );
}

#[test]
fn reduce_reduce_has_conflicts_precondition() {
    let language = adze_example::reduce_reduce::grammar::language();
    assert!(
        parse_table_has_conflicts(language),
        "reduce_reduce must remain a conflicted generated grammar"
    );
}

#[test]
fn reduce_reduce_subtree_equivalence_on_clean_input() {
    let language = adze_example::reduce_reduce::grammar::language();
    assert_bridge_streaming_subtree_equivalence("reduce_reduce", "x", language);
}

#[test]
fn reduce_reduce_route_gate_stays_on_fixed_bridge_until_ambiguity_parity() {
    let language = adze_example::reduce_reduce::grammar::language();
    let table = decode_parse_table(language);
    assert!(
        !adze::glr_streaming_runtime::should_route_conflict_table_through_streaming_driver(
            language, &table
        ),
        "reduce_reduce stays on fixed bridge until streaming ambiguity parity (#892)"
    );
}

#[test]
fn reduce_reduce_streaming_ambiguity_gap_is_known_failing() {
    let language = adze_example::reduce_reduce::grammar::language();
    let bridge_doc = adze_example::reduce_reduce::grammar::parse_document("x")
        .expect("fixed-bridge parse_document should succeed");
    let streaming_root =
        parse_via_streaming_driver("x", language).expect("streaming subtree should parse");

    assert!(
        !bridge_doc.ambiguities().is_empty(),
        "fixed bridge should retain reduce/reduce ambiguity summary"
    );
    assert_eq!(
        streaming_root.node.byte_range,
        bridge_doc.tree().root().byte_range(),
        "subtree span should match even though streaming ambiguity facts diverge"
    );
}

#[test]
fn ambiguous_expr_bridge_streaming_equivalence_single_token_only() {
    let language = adze_example::ambiguous_expr::grammar::language();
    assert_bridge_streaming_subtree_equivalence("ambiguous_expr", "42", language);
}

#[test]
fn ambiguous_expr_multi_token_streaming_gap_is_known_failing() {
    let language = adze_example::ambiguous_expr::grammar::language();
    for input in ["1+2", "1 + 2"] {
        let bridge =
            parse_via_fixed_bridge(input, language).expect("bridge should parse multi-token input");
        let streaming = parse_via_streaming_driver(input, language);
        assert!(
            streaming.is_err(),
            "known gap (#892): diverged-stack lex modes fail for multi-token ambiguous_expr input {input:?}"
        );
        assert!(
            !bridge.node.is_error,
            "fixed bridge remains production route for multi-token ambiguous_expr until parity lands"
        );
    }
}

#[test]
fn ambiguous_expr_route_gate_stays_on_fixed_bridge_until_spaced_parity() {
    let language = adze_example::ambiguous_expr::grammar::language();
    let table = decode_parse_table(language);

    assert!(
        !adze::glr_streaming_runtime::should_route_conflict_table_through_streaming_driver(
            language, &table
        ),
        "ambiguous_expr stays on fixed bridge until spaced-input streaming parity is proved"
    );
}

#[test]
fn ambiguous_expr_bridge_streaming_equivalence_bad_input() {
    let language = adze_example::ambiguous_expr::grammar::language();
    let input = "1 + @";

    let bridge_err = parse_via_fixed_bridge(input, language).expect_err("bridge should fail");
    let streaming_err =
        parse_via_streaming_driver(input, language).expect_err("streaming should fail");

    assert!(
        !bridge_err.is_empty() && streaming_err.to_string().contains("byte"),
        "both paths should surface structured failure for bad input: bridge={bridge_err:?}, streaming={streaming_err:?}"
    );
}
