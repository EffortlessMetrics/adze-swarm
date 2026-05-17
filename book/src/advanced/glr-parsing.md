# GLR Parsing

GLR parsing lets Adze handle grammar conflicts that ordinary LR parsing cannot
represent with one action per state. In Adze, GLR is a product feature only
where the support-tier ledger lists proof for the grammar class and behavior.

The user-facing rule is:

```text
grammar::parse(source) returns the selected typed AST.
grammar::parse_document(source) returns the canonical document.
GLR ambiguity summaries belong to the document.
Tree-sitter-compatible output exposes one selected tree.
Raw forest export is future work.
```

See `docs/specs/ADZE-SPEC-0012-glr-toolkit-product-contract.md` and
`docs/specs/ADZE-SPEC-0007-glr-ambiguity-summary.md` for the product contract.

## What GLR Solves

An LR parse table normally has one action for each state and lookahead symbol.
Ambiguous grammars can require multiple actions:

- shift/reduce conflicts;
- reduce/reduce conflicts;
- nested fork conflicts;
- expression grammars with multiple valid groupings;
- dangling-else-style selection problems.

GLR keeps those alternatives available during parsing. Adze then exposes a
deterministic selected tree plus ambiguity summaries for native tooling.

## How Users See GLR

Most users should not instantiate a low-level GLR parser. Use the generated
grammar APIs:

```rust
let ast = grammar::parse("1 + 2 * 3")?;
```

When tooling needs parse facts, use the document API:

```rust
let report = grammar::parse_document("1 + 2 * 3");
let document = report.document();
let ambiguities = document.ambiguities();
```

The typed AST path should agree with the selected tree in the document:

```text
grammar::parse(source)
  == grammar::parse_document(source).document().ast()
```

for the covered generated grammar path.

## Selected Tree And Ambiguity Summary

Tree-sitter-compatible output exposes one selected tree:

```rust
let tree = document.as_tree_sitter();
let root = tree.root_node();
```

Native Adze tooling should inspect ambiguity summaries:

```text
ambiguity site:
  byte range
  selected alternative
  retained alternative count
  selection reason when available
```

This is intentionally summary-first. Raw forest export is not the stable native
API.

## Designing GLR Grammars

Use GLR when ambiguity is intentional or hard to avoid. Keep ordinary grammar
design conservative:

- add precedence and associativity where they express the language;
- keep ambiguity localized;
- add fixture tests for every intended conflict;
- document which selected tree the grammar expects;
- inspect ambiguity summaries before exposing the surface as stable.

For expression grammars, prefer explicit precedence when the language has one:

```rust
#[adze::prec_left(10)]
struct Add {
    left: Box<Expr>,
    plus: Plus,
    right: Box<Expr>,
}

#[adze::prec_left(20)]
struct Multiply {
    left: Box<Expr>,
    star: Star,
    right: Box<Expr>,
}
```

Leave a grammar ambiguous only when the language really permits multiple valid
interpretations or when a later semantic phase must inspect ambiguity.

## Diagnostics And Errors

GLR does not remove the need for useful parse errors. Bad input should return a
structured error or a diagnostic document rather than panic:

```rust
let report = grammar::parse_document("1 +");
let document = report.document();

assert!(!document.diagnostics().is_empty());
```

Diagnostics, error nodes, selected tree facts, and ambiguity summaries should
all describe the same source snapshot.

## Tree-Sitter Compatibility

Tree-sitter-compatible output is an adapter over document data:

```rust
let tree = document.as_tree_sitter();
```

It should be used when a tool expects Tree-sitter-shaped traversal. It is not
the core parse product, and it does not expose every GLR alternative.

## Performance

GLR cost depends on conflict shape:

```text
unambiguous regions -> ordinary parser cost
localized ambiguity -> bounded extra work
wide ambiguity -> potentially expensive
```

Do not make performance claims without benchmark fixtures and receipts. The
product benchmark plan measures parse, document construction, typed projection,
Tree-sitter-compatible projection, query matching, diagnostics, JSON, ambiguity
summaries, tablegen, and ABI decode separately.

## Testing GLR Behavior

Every GLR product claim needs a fixture-backed proof. Useful tests assert:

- conflict cells survive table generation and ABI decode;
- shift/reduce and reduce/reduce conflicts route through GLR;
- selected trees are deterministic;
- ambiguity summaries report retained alternatives;
- `parse_document().ast()` matches the selected parse;
- bad input returns structured errors or diagnostic documents;
- Tree-sitter-compatible output reflects the selected tree only.

Representative proof commands:

```bash
cargo test -p adze-glr-core conflict ambiguity -- --nocapture
cargo test -p adze --features "pure-rust,glr,runtime-e2e" --test test_e2e_ambiguous_grammar_glr -- --nocapture
cargo test -p adze --features "pure-rust,glr,ts-compat" --test tablegen_abi_decode_roundtrip -- --nocapture
git diff --check
```

## What Not To Claim

Do not claim:

- full GLR forest stability;
- query matching over every GLR alternative;
- full Tree-sitter parity for ambiguous parses;
- stable raw forest serialization;
- stable selected-tree behavior for a grammar class without fixtures;
- performance wins without benchmark receipts.

## Current Status

GLR conflict routing is Stabilizing for documented grammar classes with
canaries. Broader product claims still need fixture matrices, selected-tree
resolution proof, compatibility proof, and support-tier promotion.
