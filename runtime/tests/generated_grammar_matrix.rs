//! Generated-grammar feature matrix proof (issue #876, PR-1 proved subset).
//!
//! Each test maps to one row of
//! `docs/product/GENERATED_GRAMMAR_FEATURE_MATRIX.md`. Only constructs with
//! exact-value assertions belong here; unproved constructs stay `Gap` rows in
//! the matrix document and must not be asserted as supported.

use adze_example::fielded_precedence_typed_cst_contract::grammar::Expr as FieldedExpr;
use adze_example::typed_ast_contract::grammar::Expr;

/// Row: regex leaf + infallible transform (`\d+` -> `i32`).
#[test]
fn matrix_regex_leaf_transform_exact_value() {
    let parsed =
        adze_example::typed_ast_contract::grammar::parse("42").expect("number should parse");
    assert_eq!(parsed, Expr::Number(42));
}

/// Row: tuple enum variant + `Box<T>` recursion + `#[adze::prec_left(1)]`.
/// Left associativity is asserted by exact tree shape, not parse success.
#[test]
fn matrix_tuple_variant_box_recursion_prec_left_exact_shape() {
    let parsed = adze_example::typed_ast_contract::grammar::parse("1 + 2 + 3")
        .expect("addition chain should parse");
    assert_eq!(
        parsed,
        Expr::Add(
            Box::new(Expr::Add(
                Box::new(Expr::Number(1)),
                (),
                Box::new(Expr::Number(2)),
            )),
            (),
            Box::new(Expr::Number(3)),
        )
    );
}

/// Row: extras (whitespace) are skipped around tokens and operators.
#[test]
fn matrix_extras_whitespace_skipped_exact_value() {
    let parsed = adze_example::typed_ast_contract::grammar::parse("  1   +   2  ")
        .expect("spaced addition should parse");
    assert_eq!(
        parsed,
        Expr::Add(Box::new(Expr::Number(1)), (), Box::new(Expr::Number(2)),)
    );
}

/// Row: named-field struct variants + two precedence levels.
/// `*` binds tighter than `+`, asserted by exact tree shape.
#[test]
fn matrix_named_fields_precedence_levels_exact_shape() {
    let parsed = adze_example::fielded_precedence_typed_cst_contract::grammar::parse("1 + 2 * 3")
        .expect("precedence expression should parse");
    assert_eq!(
        parsed,
        FieldedExpr::Add {
            left: Box::new(FieldedExpr::Number(1)),
            operator: (),
            right: Box::new(FieldedExpr::Mul {
                left: Box::new(FieldedExpr::Number(2)),
                operator: (),
                right: Box::new(FieldedExpr::Number(3)),
            }),
        }
    );
}

/// Row: `parse()` and `parse_document().ast()` agree on the same input
/// (typed extraction vs canonical document truth, SPEC-0012).
#[test]
fn matrix_parse_and_document_ast_agree() {
    let source = "1 + 2 * 3";
    let parsed = adze_example::fielded_precedence_typed_cst_contract::grammar::parse(source)
        .expect("typed parse should succeed");
    let document =
        adze_example::fielded_precedence_typed_cst_contract::grammar::parse_document(source)
            .expect("document parse should succeed");
    let from_document: FieldedExpr = document
        .ast()
        .expect("document should extract the same typed AST");
    assert_eq!(from_document, parsed);
}
