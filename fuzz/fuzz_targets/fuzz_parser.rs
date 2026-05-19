#![no_main]

use libfuzzer_sys::fuzz_target;
use syn::{Expr, File, parse_file, parse_str};

fuzz_target!(|data: &[u8]| {
    let input = String::from_utf8_lossy(data);

    // Keep the fuzz target responsive on pathological large buffers.
    if input.len() > 32 * 1024 {
        return;
    }

    // Exercise full Rust-file parsing.
    if let Ok(file) = parse_file(&input) {
        assert_ast_round_trip_file(&file);
    }

    // Also exercise expression parsing for partial snippets.
    if let Ok(expr) = parse_str::<Expr>(&input) {
        assert_ast_round_trip_expr(&expr);
    }
});

fn assert_ast_round_trip_file(file: &File) {
    let tokens = quote::quote!(#file).to_string();
    let reparsed =
        parse_file(&tokens).expect("tokenized syn::File should parse when rendered back to source");

    // Structural invariant: rendering + reparsing should keep item count stable.
    assert_eq!(file.items.len(), reparsed.items.len());
}

fn assert_ast_round_trip_expr(expr: &Expr) {
    let tokens = quote::quote!(#expr).to_string();
    let reparsed = parse_str::<Expr>(&tokens)
        .expect("tokenized syn::Expr should parse when rendered back to source");

    // Structural invariant: formatting/reparse should preserve discrimant variant.
    assert_eq!(
        std::mem::discriminant(expr),
        std::mem::discriminant(&reparsed)
    );
}
