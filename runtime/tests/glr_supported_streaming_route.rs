//! Prove supported conflicted generated grammars never fall back to the fixed bridge (#892).

#![cfg(all(feature = "pure-rust", feature = "glr", feature = "runtime-e2e"))]

use adze::decoder::decode_parse_table;
use adze::glr_streaming_runtime::{TrueGlrParseRoute, last_true_glr_parse_route};
use adze::pure_parser::TSLanguage;
use adze_example::{ambiguous_expr, dangling_else, reduce_reduce, streaming_lex_modes};

struct SupportedConflictCase {
    id: &'static str,
    language: &'static TSLanguage,
    source: &'static str,
}

fn assert_routes_and_parses(case: SupportedConflictCase) {
    assert!(
        adze::glr_streaming_runtime::should_route_conflict_table_through_streaming_driver(
            case.language,
            &decode_parse_table(case.language),
        ),
        "{} must route through streaming driver",
        case.id
    );

    let _ = last_true_glr_parse_route();
    let document = match case.id {
        "streaming_lex_modes" => streaming_lex_modes::grammar::parse_document(case.source)
            .expect("parse_document should succeed"),
        "ambiguous_expr" => ambiguous_expr::grammar::parse_document(case.source)
            .expect("parse_document should succeed"),
        "reduce_reduce" => reduce_reduce::grammar::parse_document(case.source)
            .expect("parse_document should succeed"),
        "dangling_else" => dangling_else::grammar::parse_document(case.source)
            .expect("parse_document should succeed"),
        _ => panic!("unsupported case id"),
    };

    assert!(
        document.diagnostics().is_empty(),
        "{} should parse cleanly for {:?}",
        case.id,
        case.source
    );
    assert_eq!(
        last_true_glr_parse_route(),
        Some(TrueGlrParseRoute::StreamingDriver),
        "{} production parse must not use fixed pretokenization bridge",
        case.id
    );
}

#[test]
fn supported_conflict_matrix_routes_through_streaming_driver_only() {
    let cases = [
        SupportedConflictCase {
            id: "ambiguous_expr",
            language: ambiguous_expr::grammar::language(),
            source: "1+2",
        },
        SupportedConflictCase {
            id: "reduce_reduce",
            language: reduce_reduce::grammar::language(),
            source: "x",
        },
        SupportedConflictCase {
            id: "streaming_lex_modes",
            language: streaming_lex_modes::grammar::language(),
            source: "1+2\n",
        },
        SupportedConflictCase {
            id: "dangling_else",
            language: dangling_else::grammar::language(),
            source: "if a then if b then other else other",
        },
    ];

    for case in cases {
        assert_routes_and_parses(case);
    }
}

#[test]
fn supported_conflict_matrix_parse_and_document_ast_agree() {
    use adze_example::ambiguous_expr::grammar::{self, Expr};

    let source = "1 + 2 * 3";
    let selected = grammar::parse(source).expect("typed parse should succeed");
    let document = grammar::parse_document(source).expect("document parse should succeed");
    let document_ast: Expr = document
        .ast()
        .expect("document AST projection should succeed");

    assert_eq!(selected, document_ast);
    assert_eq!(
        last_true_glr_parse_route(),
        Some(TrueGlrParseRoute::StreamingDriver)
    );
}
