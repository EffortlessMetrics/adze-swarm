//! Generated GLR conflict matrix canaries.
//!
//! This consolidates currently generated GLR fixtures into one product-facing
//! proof: generated parse tables retain conflict cells, selected-tree output is
//! deterministic, `parse()` and `parse_document().ast()` agree, ambiguity
//! summaries are exposed for ambiguous inputs, and bad input returns diagnostic
//! documents instead of panicking.

#![cfg(all(test, feature = "pure-rust", feature = "glr"))]

use adze::{decoder, document::AdzeDocument, pure_parser::TSLanguage};
use adze_glr_core::conflict_inspection::{ConflictType, count_conflicts};
use std::ops::Range;

#[derive(Clone, Copy)]
enum CountExpectation {
    Exact(usize),
    AtLeast(usize),
}

struct GeneratedConflictCase {
    id: &'static str,
    language: &'static TSLanguage,
    shift_reduce: CountExpectation,
    reduce_reduce: CountExpectation,
    required_symbol_fragment: Option<&'static str>,
}

#[derive(Debug, Eq, PartialEq)]
struct SelectedTreeShape {
    kind_name: Option<String>,
    grammar_name: Option<String>,
    byte_range: Range<usize>,
    child_count: usize,
    has_error: bool,
}

#[test]
fn generated_conflict_matrix_preserves_table_cells() {
    use adze_example::{ambiguous_expr, dangling_else, fielded_precedence_typed_cst_contract};

    let cases = [
        GeneratedConflictCase {
            id: "glr-shift-reduce-expression",
            language: ambiguous_expr::grammar::language(),
            shift_reduce: CountExpectation::AtLeast(1),
            reduce_reduce: CountExpectation::Exact(0),
            required_symbol_fragment: None,
        },
        GeneratedConflictCase {
            id: "glr-dangling-else",
            language: dangling_else::grammar::language(),
            shift_reduce: CountExpectation::Exact(1),
            reduce_reduce: CountExpectation::Exact(0),
            required_symbol_fragment: None,
        },
        GeneratedConflictCase {
            id: "glr-precedence-control",
            language: fielded_precedence_typed_cst_contract::grammar::language(),
            shift_reduce: CountExpectation::Exact(0),
            reduce_reduce: CountExpectation::Exact(0),
            required_symbol_fragment: None,
        },
    ];

    for case in cases {
        let table = decoder::decode_parse_table(case.language);
        let summary = count_conflicts(&table);

        assert_count(
            case.id,
            "shift/reduce",
            summary.shift_reduce,
            case.shift_reduce,
        );
        assert_count(
            case.id,
            "reduce/reduce",
            summary.reduce_reduce,
            case.reduce_reduce,
        );

        if let Some(fragment) = case.required_symbol_fragment {
            assert!(
                summary.conflict_details.iter().any(|conflict| {
                    conflict.conflict_type == ConflictType::ShiftReduce
                        && conflict.symbol_name.contains(fragment)
                        && conflict.actions.len() >= 2
                }),
                "{id} should retain a shift/reduce conflict on symbol fragment {fragment:?}; got {details:?}",
                id = case.id,
                details = summary
                    .conflict_details
                    .iter()
                    .map(|conflict| {
                        (
                            &conflict.symbol_name,
                            conflict.conflict_type,
                            conflict.actions.len(),
                        )
                    })
                    .collect::<Vec<_>>()
            );
        }
    }
}

#[test]
fn generated_conflict_matrix_selects_deterministic_document_trees() {
    assert_ambiguous_expr_document("1 + 2 + 3", 1);
    assert_ambiguous_expr_document("1 + 2 * 3 - 4", 1);
    assert_precedence_control_document("1+2*3");
}

#[test]
fn generated_conflict_matrix_bad_input_returns_diagnostic_documents() {
    let ambiguous = adze_example::ambiguous_expr::grammar::parse_document("1 + @")
        .expect("ambiguous_expr should return partial document facts for bad input");
    assert_diagnostic_document("glr-shift-reduce-expression", ambiguous);

    let precedence =
        adze_example::fielded_precedence_typed_cst_contract::grammar::parse_document("1+")
            .expect("precedence control should return partial document facts for bad input");
    assert_diagnostic_document("glr-precedence-control", precedence);
}

fn assert_ambiguous_expr_document(source: &str, min_ambiguities: usize) {
    use adze_example::ambiguous_expr::grammar::{self, Expr};

    let selected = grammar::parse(source).expect("GLR parse should select a typed AST");
    let document = grammar::parse_document(source)
        .expect("GLR parse_document should return the selected document");
    let document_ast: Expr = document
        .ast()
        .expect("document selected tree should extract the same typed AST");
    assert_eq!(document_ast, selected);
    assert_clean_document(&document, source);
    assert!(
        document.ambiguities().len() >= min_ambiguities,
        "ambiguous expression should expose ambiguity summaries for {source:?}: {:?}",
        document.ambiguities()
    );
    assert_deterministic_document(|| {
        grammar::parse_document(source).expect("repeat GLR parse_document should succeed")
    });
}

fn assert_precedence_control_document(source: &str) {
    use adze_example::fielded_precedence_typed_cst_contract::grammar::{self, Expr};

    let selected = grammar::parse(source).expect("precedence control parse should succeed");
    let document = grammar::parse_document(source)
        .expect("precedence control parse_document should return a document");
    let document_ast: Expr = document
        .ast()
        .expect("document selected tree should extract the same typed AST");
    assert_eq!(document_ast, selected);
    assert_clean_document(&document, source);
    assert!(
        document.ambiguities().is_empty(),
        "precedence-resolved control should not expose ambiguity summaries: {:?}",
        document.ambiguities()
    );
}

fn assert_clean_document(document: &AdzeDocument, source: &str) {
    assert!(document.diagnostics().is_empty());
    assert!(!document.tree().has_errors());
    assert_eq!(document.tree().root().byte_range(), 0..source.len());
}

fn assert_diagnostic_document(id: &str, document: AdzeDocument) {
    assert!(
        document.tree().has_errors(),
        "{id} bad input should set tree error state"
    );
    assert!(
        !document.diagnostics().is_empty(),
        "{id} bad input should expose structured diagnostics"
    );
    assert!(
        document.ambiguities().is_empty(),
        "{id} bad input should not claim ambiguity summaries for an error document"
    );
}

fn assert_deterministic_document(parse: impl Fn() -> AdzeDocument) {
    let first = parse();
    let second = parse();
    assert_eq!(
        collect_selected_tree_shape(first.tree().root()),
        collect_selected_tree_shape(second.tree().root()),
        "repeated parse_document calls should produce the same selected-tree shape"
    );
}

fn collect_selected_tree_shape(node: adze::document::AdzeNode<'_>) -> Vec<SelectedTreeShape> {
    let mut out = Vec::new();
    collect_selected_tree_shape_into(node, &mut out);
    out
}

fn collect_selected_tree_shape_into(
    node: adze::document::AdzeNode<'_>,
    out: &mut Vec<SelectedTreeShape>,
) {
    out.push(SelectedTreeShape {
        kind_name: node.kind_name().map(str::to_owned),
        grammar_name: node.grammar_name().map(str::to_owned),
        byte_range: node.byte_range(),
        child_count: node.child_count(),
        has_error: node.has_error(),
    });
    for child_index in 0..node.child_count() {
        collect_selected_tree_shape_into(
            node.child(child_index)
                .expect("child should resolve while collecting selected-tree shape"),
            out,
        );
    }
}

fn assert_count(id: &str, kind: &str, actual: usize, expected: CountExpectation) {
    match expected {
        CountExpectation::Exact(value) => assert_eq!(
            actual, value,
            "{id} should have exactly {value} {kind} conflicts"
        ),
        CountExpectation::AtLeast(value) => assert!(
            actual >= value,
            "{id} should have at least {value} {kind} conflicts, got {actual}"
        ),
    }
}
