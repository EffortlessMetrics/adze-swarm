# Error Recovery

When someone is still typing, you may need to show an error without requiring
a complete program. For a generated pure-Rust parser, use `grammar::parse()`
when the caller needs a typed AST and `grammar::parse_document()` when it needs
diagnostics and the selected syntax tree.

## Report Diagnostics From the Document

`parse_document()` returns a `Result`. A recovered document can contain both a
selected tree and diagnostics. Handle the separate error return when parsing
cannot produce a document. An `Ok(document)` is not a guarantee that the input
is valid.

This example assumes `grammar` is the generated module in your project:

```rust,ignore
match grammar::parse_document(source) {
    Ok(document) => {
        for diagnostic in document.diagnostics() {
            eprintln!("{}", diagnostic.display_with_source(source));
        }
    }
    Err(errors) => {
        for error in errors {
            eprintln!("{error}");
        }
    }
}
```

Use `diagnostic.byte_span()` and `diagnostic.point_range` to place a message in
your tool. Found and expected tokens can be included when the parser knows
them. For the covered GLR grammar shapes, `document.ambiguities()` describes
alternatives and the selection; the document's tree is the selected one, not
the whole forest.

## Slice Source Safely

A span is a pair of byte offsets, not character indices. For a recovered
document, use `document.source_slice(range)`; it returns `None` for reversed
ranges, out-of-bounds offsets, or offsets that split a UTF-8 character. If you
have an `adze::Spanned<T>` and the original source but no document, take the
offsets from `spanned.span` and use `source.get(start..end)`. Indexing the
string directly can panic for an invalid range.

## Handle Edits Conservatively

Be prepared to parse the full updated source after an edit. Incremental
parsing is Experimental. A document marked as using a full-reparse fallback
does not imply subtree reuse, and changed ranges are conservative. See
[Incremental Parsing](incremental-parsing.md) for the lifecycle and limits.

## Check Your Grammar's Recovery Behavior

Exercise the actual malformed inputs your tool needs to handle, such as an
unfinished expression, an unexpected token, and an edit near a multibyte
character. Check whether the call returns a document or errors, then assert the
diagnostic ranges and selected-tree facts your tool consumes.

The workspace has a focused canary for generated object-like grammar recovery:

```bash
cargo test -p adze --features "pure-rust,glr,serialization,ts-compat" --test recovery_matrix generated_object_like_bad_input_matrix_preserves_document_diagnostics_and_json -- --exact --nocapture
```

Measure recovery latency on your own grammar and inputs rather than assuming
that recovery is free. See [Support Tiers](https://github.com/EffortlessMetrics/adze-swarm/blob/main/docs/status/SUPPORT_TIERS.md)
for the covered diagnostic and recovery cases.
