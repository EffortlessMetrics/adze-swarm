//! Probe fixed-bridge vs streaming ambiguity summaries for ambiguous_expr (#892).

#![cfg(all(feature = "pure-rust", feature = "glr", feature = "runtime-e2e"))]

use adze::__private::{align_true_glr_parse_table_to_language_symbols, lex_with_language_fn};
use adze::decoder::{decode_grammar, decode_parse_table};
use adze::glr_parser::GLRParser;
use adze::glr_streaming_runtime::parse_with_streaming_driver;
use adze_example::ambiguous_expr::grammar;

#[test]
fn ambiguous_expr_fixed_bridge_exposes_ambiguity_summary() {
    let language = grammar::language();
    let source = "1 + 2 + 3";
    let bytes = source.as_bytes();
    let mut parse_table = decode_parse_table(language);
    let grammar_ir = decode_grammar(language);
    align_true_glr_parse_table_to_language_symbols(language, &mut parse_table);
    let mut parser = GLRParser::new(parse_table, grammar_ir);

    let lex_fn = language.lex_fn.expect("lex_fn");
    for token in lex_with_language_fn(language, lex_fn, bytes).expect("lex") {
        parser.process_token(token.symbol_id, &token.text, token.byte_offset);
    }
    parser.process_eof(bytes.len());
    let summary = parser
        .finish_ambiguity_summary()
        .expect("ambiguity summary should not error");
    assert!(
        summary.is_some(),
        "fixed bridge should retain ambiguity summary for {source:?}, got {summary:?}"
    );
}

#[test]
fn ambiguous_expr_streaming_exposes_ambiguity_summary() {
    let language = grammar::language();
    let source = "1 + 2 + 3";
    let parse_table = decode_parse_table(language);
    let grammar_ir = decode_grammar(language);
    let parsed = parse_with_streaming_driver(source, language, parse_table, &grammar_ir)
        .expect("streaming parse should succeed");
    assert!(
        parsed.ambiguities.is_some(),
        "streaming forest should expose native ambiguity summaries for {source:?}: {:?}",
        parsed.ambiguities
    );

    let document = grammar::parse_document(source).expect("parse_document should succeed");
    assert!(
        !document.ambiguities().is_empty(),
        "parse_document should expose ambiguity summaries for {source:?}: {:?}",
        document.ambiguities()
    );
}
