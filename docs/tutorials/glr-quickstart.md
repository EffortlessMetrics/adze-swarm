# GLR And .parsetable Quickstart

> **Doc status:** GLR conflict routing is Stabilizing. `.parsetable`
> serialization and tablegen ABI proof are support-tiered, but dynamic
> table-loading APIs are not the ordinary user entry point. This page is an
> advanced orientation note, not a Stable production API contract.

For most users, GLR does not change the first-use path:

```rust
let ast = grammar::parse(source)?;
```

Use the generated document path when you need GLR facts for tooling:

```rust
let document = grammar::parse_document(source)?;
let ambiguities = document.ambiguities();
let diagnostics = document.diagnostics();
```

`AdzeDocument` remains the one parse truth. Typed AST, generic CST, typed CST,
diagnostics, ambiguity summaries, Tree-sitter-compatible output, JSON, CLI, and
future WASM/editor views are projections from the generated document path.

## When GLR Matters

GLR is useful when a grammar has real ambiguity or when conflict cells must be
preserved instead of rejected as ordinary LR conflicts. Typical examples are:

- expression grammars with multiple associativity or precedence sites;
- dangling-else style shift/reduce conflicts;
- grammars with reduce/reduce conflict cells;
- language subsets where the selected tree must be deterministic but ambiguity
  should remain visible to native tooling.

The selected tree is deterministic for the proven generated-parser slices.
Native Adze APIs expose ambiguity summaries separately. Tree-sitter-compatible
views expose the selected tree only.

## The Supported User Ladder

| Need | Start here | Support posture |
|---|---|---|
| Typed Rust values | `grammar::parse(source)` | Stable front door |
| Diagnostics, ranges, ambiguity, JSON, compatibility views | `grammar::parse_document(source)` | Experimental/Stabilizing by surface |
| Tree-sitter-shaped selected-tree traversal | `adze::ts_compat::Tree::from_document(...)` | Advisory selected-tree subset |
| Serialized parse table ABI proof | tablegen/parsetable tests | Stabilizing proof surface |
| Dynamic `.parsetable` loading in an application | implementation-specific integration | Advisory |

See [Support Tiers](../status/SUPPORT_TIERS.md) before treating any GLR,
document, Tree-sitter compatibility, query, JSON, CLI, or `.parsetable` behavior
as stable.

## Minimal Generated GLR Shape

An ambiguous grammar still starts as Rust types:

```rust
#[adze::grammar("expr")]
pub mod grammar {
    #[adze::language]
    #[derive(Debug, PartialEq)]
    pub enum Expr {
        Number(
            #[adze::leaf(pattern = r"\d+", transform = |v| v.parse().unwrap())]
            i32,
        ),

        #[adze::prec_left(1)]
        Add(Box<Expr>, #[adze::leaf(text = "+")] (), Box<Expr>),

        #[adze::prec_left(2)]
        Mul(Box<Expr>, #[adze::leaf(text = "*")] (), Box<Expr>),
    }

    #[adze::extra]
    #[allow(dead_code)]
    struct Whitespace {
        #[adze::leaf(pattern = r"\s+")]
        _ws: (),
    }
}
```

Use `parse()` when you only need the typed result:

```rust
let ast = grammar::parse("1 + 2 * 3")?;
```

Use `parse_document()` when the application needs document facts:

```rust
let document = grammar::parse_document("1 + 2 * 3")?;

println!("diagnostics: {}", document.diagnostics().len());
println!("ambiguities: {}", document.ambiguities().len());
println!("root: {:?}", document.tree().root().kind_name());
```

## .parsetable Files

`.parsetable` files are an advanced distribution format for pre-generated parse
table data. They are useful when a build or deployment pipeline needs to carry
table artifacts separately from source generation.

Use them for:

- ABI roundtrip proof;
- build artifact inspection;
- controlled internal tooling;
- future dynamic grammar distribution experiments.

Do not use them as the default beginner path. A new user should not need to
hand-load parse tables to parse input with Adze.

## Advanced Pipeline Shape

The high-level `.parsetable` flow is:

```text
Rust grammar / grammar JSON
  -> adze-tool / adze-tablegen
  -> generated parser module
  -> optional .parsetable artifact
  -> validation / ABI roundtrip proof
```

If an integration stores `.parsetable` artifacts, keep these receipts:

- exact Adze crate versions;
- grammar fingerprint or equivalent build identity;
- tablegen ABI proof command;
- generated parser proof command;
- regeneration instructions for version upgrades.

## Proof Commands

Use focused GLR and tablegen proof before relying on serialized table behavior:

```bash
cargo test -p adze-glr-core conflict ambiguity -- --nocapture
cargo test -p adze-tablegen --all-features
cargo test -p adze --features "pure-rust,glr,runtime-e2e" --test test_e2e_ambiguous_grammar_glr -- --nocapture
```

Use the repository-supported gate before submitting product changes:

```bash
just ci-supported
```

## Known Limits

- GLR conflict routing is Stabilizing, not broadly Stable for every grammar
  class.
- `AdzeDocument` is still Experimental even though it is the native parse
  product boundary.
- Tree-sitter compatibility is a selected-tree adapter, not a full
  Tree-sitter runtime parity claim.
- Query compatibility is a documented subset.
- Dynamic `.parsetable` loading is not the recommended beginner API.
- Full performance claims require benchmark receipts, not example prose.

## Troubleshooting

If table artifacts fail to decode or behave unexpectedly:

1. Regenerate artifacts with the same Adze version used by the runtime.
2. Confirm the grammar fingerprint/build identity matches the expected source.
3. Run the tablegen and GLR proof commands above.
4. Prefer generated `parse()` / `parse_document()` until the dynamic table path
   has a dedicated support-tier receipt.

## Next Steps

- [Quickstart: First Parser In 10 Minutes](./quickstart-10-minutes.md)
- [Mental Model](../explanations/mental-model.md)
- [API Reference](../reference/api.md)
- [Tree-sitter Compatibility](../reference/tree-sitter-compatibility.md)
- [Support Tiers](../status/SUPPORT_TIERS.md)
