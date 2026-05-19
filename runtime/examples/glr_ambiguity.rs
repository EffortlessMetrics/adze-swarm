//! Runnable GLR ambiguity example.
//!
//! This example intentionally uses the `ambiguous_expr` generated grammar. It
//! teaches the product boundary:
//!
//! - `grammar::parse()` returns one deterministic selected typed AST.
//! - `grammar::parse_document()` returns the same selected parse plus native
//!   ambiguity summaries and diagnostics.
//! - Tree-sitter-compatible projections expose the selected tree; raw GLR
//!   forest alternatives are not the stable product API.

use adze_example::ambiguous_expr::grammar::{self, Expr};

fn main() {
    let source = "1 + 2 + 3";

    let selected_ast = grammar::parse(source).expect("ambiguous expression should parse");
    let document =
        grammar::parse_document(source).expect("ambiguous expression should produce a document");
    let document_ast: Expr = document
        .ast()
        .expect("document selected tree should extract typed AST");
    assert_eq!(
        document_ast, selected_ast,
        "typed parse and document AST projection should select the same tree"
    );

    let root = document.tree().root();
    let ambiguity = document
        .ambiguities()
        .first()
        .expect("ambiguous expression should expose an ambiguity summary");

    println!("source: {source}");
    println!("selected typed AST: {selected_ast:?}");
    println!(
        "selected document root: kind={:?} bytes={:?}",
        root.kind_name(),
        root.byte_range()
    );
    println!(
        "ambiguity summary: span={:?} alternatives={} selected={:?} reason={:?}",
        ambiguity.span,
        ambiguity.alternatives.len(),
        ambiguity.selected,
        ambiguity.selection_reason
    );

    for alternative in &ambiguity.alternatives {
        println!(
            "  alternative #{}: root={:?} span={:?} nodes={} cost={} error={}",
            alternative.index,
            alternative.root_symbol,
            alternative.span,
            alternative.node_count,
            alternative.cost,
            alternative.in_error
        );
    }

    let bad_input = "1 + @";
    let diagnostic_document =
        grammar::parse_document(bad_input).expect("bad input should produce diagnostic document");
    assert!(
        diagnostic_document.tree().has_errors(),
        "bad ambiguous input should keep error facts on the document tree"
    );
    assert!(
        diagnostic_document.ambiguities().is_empty(),
        "bad input should not claim a complete ambiguity summary"
    );

    println!("bad input: {bad_input}");
    for diagnostic in diagnostic_document.diagnostics() {
        println!(
            "  diagnostic: {} expected={:?}",
            diagnostic.display_with_source(bad_input),
            diagnostic.expected
        );
    }

    println!(
        "compatibility note: Tree-sitter-shaped output exposes the selected tree; \
         native AdzeDocument ambiguity summaries keep the GLR ambiguity facts."
    );
}
