# AdzeDocument Design Contract

**Status:** Draft design contract; not an implemented stable API.

`AdzeDocument` is the planned native parse-product boundary for Adze. It is
the source of truth that future Tree-sitter-compatible output, typed CST,
typed AST extraction, diagnostics, and GLR ambiguity views should project from.

The design goal is one parse truth with multiple views, not parallel trees that
can drift.

```text
source
  -> Adze parser/runtime
  -> AdzeDocument
       -> generic CST
       -> typed CST
       -> typed AST
       -> Tree-sitter-compatible Tree/Node/Cursor
       -> diagnostics
       -> GLR ambiguity summaries / forest data
```

## Core Rule

`AdzeDocument` must be monomorphic.

```rust
pub struct AdzeDocument {
    source: SourceText,
    tree: AdzeTree,
    diagnostics: Vec<ParseDiagnostic>,
    ambiguities: AmbiguitySet,
    metadata: ParseMetadata,
}
```

Typed ASTs and typed CSTs are projections:

```rust
let doc = grammar::parse_document(source)?;

let tree = doc.tree();
let syntax: syntax::SourceFile = doc.syntax()?;
let ast: ast::Module = doc.ast()?;
let ts_tree = adze::ts_compat::Tree::from_document(language.clone(), &doc);
```

They are not fields on the document and must not define separate parse truths.

## Document Versus Failure Semantics

`parse_document` should distinguish parse facts from infrastructure failures.

Syntax errors, recovery, and ambiguity should generally produce an
`AdzeDocument` with diagnostics, node flags, and metadata:

```rust
let doc = grammar::parse_document("1 +")?;
assert!(!doc.diagnostics().is_empty());
```

Hard failures are reserved for cases where no trustworthy document can be
produced:

```rust
pub enum ParseFailure {
    NoLanguage,
    Cancelled,
    InternalInvariant,
}
```

This keeps native parsing useful for editors, LSPs, formatters, and agents that
must inspect incomplete source text. Tree-sitter-compatible projections can
render the same state as `is_error()`, `has_error()`, and `is_missing()`, while
native APIs expose the structured diagnostics that explain what happened.

## Native Tree Model

The generic native CST should be lossless enough to support formatting,
refactoring, diagnostics, Tree-sitter-compatible projection, and typed CST
wrappers.

```rust
pub struct AdzeTree {
    root: NodeId,
    nodes: NodeArena,
    language: LanguageMetadata,
}

pub struct AdzeNode {
    id: NodeId,
    kind: NodeKind,
    span: ByteRange,
    point_range: PointRange,
    parent: Option<NodeId>,
    children: Vec<Edge>,
    production_id: Option<ProductionId>,
    rule_id: Option<RuleId>,
    flags: NodeFlags,
}
```

Node identity is explicit even before alias-aware projection lands:

```rust
pub struct NodeIdentity {
    visible_id: SymbolId,
    grammar_id: SymbolId,
    visible_name: Option<SymbolName>,
    grammar_name: Option<SymbolName>,
    alias_symbol_id: Option<SymbolId>,
    visible_is_named: bool,
    grammar_is_named: bool,
}
```

The current alpha populates visible and grammar identity from the same raw
parsed symbol and leaves `alias_symbol_id` empty. That is intentional: the
native model now has separate slots for alias-aware identity, but the
Tree-sitter compatibility layer must not claim alias parity until parser nodes
actually carry alias sequence entries and canaries prove the projection.

Node flags are explicit native data as well:

```rust
pub struct NodeFlags {
    named: bool,
    visible: bool,
    extra: bool,
    terminal: bool,
    supertype: bool,
    error: bool,
    missing: bool,
    has_error: bool,
}
```

The current alpha computes these flags from the selected document tree,
language metadata, and parser recovery/error count. It does not yet claim
alias-adjusted namedness, recovered-node classification, or per-node ambiguity
flags.

Fields are edge metadata:

```rust
pub struct Edge {
    child: NodeId,
    field_id: Option<FieldId>,
    field_name: Option<Arc<str>>,
}
```

A child is the `left` child of a particular parent; it is not globally `left`.
This is required for both Tree-sitter-compatible field APIs and generated typed
CST accessors.

## Projections

### Simple Typed AST

The existing simple API remains the front door for users who only want typed
semantic values:

```rust
let ast: Module = grammar::parse(source)?;
```

Generated pure-Rust documents now expose an alpha projection for this path:

```rust
let ast: Module = grammar::parse_document(source)?.ast()?;
```

That projection extracts from the document's selected tree and returns document
diagnostics as parse errors when the source recovered or failed. It is still an
experimental native-document view, not a replacement for the stable
`grammar::parse(...)` front door.

### Typed CST

Typed CST is a future generated view over `AdzeTree`, not a second tree.

Typed CST wrappers should be cheap handles:

```rust
pub struct FunctionDecl<'doc> {
    doc: &'doc AdzeDocument,
    id: NodeId,
}
```

The first implementation should stay narrow: generated node wrappers, field
accessors, token wrappers where needed, span access, and text access. Visitors,
rewriters, typed queries, trivia classification, and JSON output are later
surfaces that require separate proof.

### Tree-sitter Compatibility

Tree-sitter compatibility is a conformance adapter over the native document.

```rust
let ts_tree = adze::ts_compat::Tree::from_document(language.clone(), &doc);
let root = ts_tree.root_node();
```

The adapter must not invent missing semantics locally. If a Tree-sitter method
cannot be implemented from `AdzeDocument` data, the native document model is
missing required information.

Examples:

| Tree-sitter-compatible API | Native invariant |
|---|---|
| `Node::child(i)` | Stable child edges exist. |
| `Node::field_name_for_child(i)` | Field names live on edges. |
| `Node::child_by_field_id(id)` | Public field IDs translate from edge metadata. |
| `Node::kind()` | Visible node identity exists. |
| `Node::grammar_name()` | Original grammar identity exists. |
| `Node::is_error()` | Node-local error flags exist. |
| `Node::has_error()` | Diagnostics or recovery state propagate through the tree. |
| `Node::is_missing()` | Recovery can represent zero-width inserted structure. |
| `Node::to_sexp()` | Tree shape and field labels are serializable. |

### Diagnostics

Tree-sitter-compatible output exposes structural flags. Native Adze output must
also expose diagnostic data.

```rust
pub struct ParseDiagnostic {
    span: ByteRange,
    point_range: PointRange,
    expected: Vec<ExpectedSymbol>,
    found: Option<FoundSymbol>,
    recovery: Option<RecoveryAction>,
    related_nodes: Vec<NodeId>,
}
```

Text rendering is a view over this data, not the canonical representation. The
alpha runtime exposes `ParseDiagnostic::display_with_source(source)` for
source-context rendering while keeping byte spans, point ranges, expected
symbols, found symbols, and related document node IDs as the document facts.
`AdzeDocument::diagnostics_for_node(...)` and `AdzeNode::diagnostics()` expose
the same related-node mapping from the tree side without forcing callers to scan
all diagnostics manually.

### GLR Ambiguity

Tree-sitter-compatible output should expose one selected tree. Native Adze
output should expose ambiguity summaries first and raw forest internals only
after the summary contract is proven.

```rust
pub struct Ambiguity {
    span: ByteRange,
    alternatives: Vec<AlternativeSummary>,
    selected: Option<AlternativeId>,
    selection_reason: SelectionReason,
}
```

Default parsing should not eagerly collect expensive forest or trace data unless
the user opts in through explicit parse options.

The true-GLR runtime now has an alpha parser-level summary for retained complete
alternatives: it reports the alternative count, selected alternative, root spans,
structural node counts, and whether selection came from dynamic-precedence/error-cost
version comparison or the stable structural tie-break. Generated `parse_document()` now routes
conflicted parse tables through that true-GLR runtime and records the same
summary on `AdzeDocument::ambiguities()`. GLR lexing and finish errors on this
document route are converted into structured diagnostics with a synthetic error
root instead of escaping as hard parse failures. This is still a selected-tree
summary, not raw forest export or typed extraction from alternatives.

## Parse Options

`parse_document` should leave room for staged cost:

```rust
pub struct ParseOptions {
    recover: bool,
    collect_diagnostics: bool,
    collect_ambiguities: bool,
    collect_forest: bool,
    collect_trace: bool,
}
```

The common path should be cheap: parse source, retain the generic CST, expose
diagnostics and metadata, and compute richer projections lazily.

## Serialized Outputs

Any future native JSON output must be schema-versioned.

Examples:

```json
{ "schema": "adze.document.v1" }
```

Planned schema families include:

- `adze.document.v1`
- `adze.tree.v1`
- `adze.diagnostics.v1`
- `adze.typed-cst.v1`
- `adze.forest.v1`

The current alpha implements only `AdzeDocument::to_json_value()` under the
`serialization` feature. It emits an experimental `adze.document.v1` envelope
for the selected generic CST, source byte length, language name, metadata,
structured diagnostics, and ambiguity summaries. This is a document canary for
future output work; snapshot fixtures pin representative clean fielded-edge,
EOF diagnostic, multibyte diagnostic, multiline diagnostic, and ambiguous GLR
documents. The clean fixture also cross-checks serialized child-edge indexes,
field names, field IDs, and nested child node IDs against the native
`AdzeEdge` projection, but this is not a stable CLI/WASM `adze-json` contract.

No JSON schema should be treated as stable until it has a fixture, snapshot, and
support-tier entry.

## Non-Goals For The First Implementation

The first implementation must not attempt all projections at once.

Out of scope for the alpha document:

- full typed CST generation,
- typed CST visitors or rewriters,
- full Tree-sitter query execution,
- raw GLR forest export,
- typed extraction from ambiguity alternatives,
- stable `adze-json`,
- WASM document bindings,
- support-tier promotion.

The first useful slice is:

```text
AdzeDocument
  -> tree()
  -> source_slice()
  -> NodeId lookup
  -> edge and parent lookup
  -> SyntaxNode handle helpers
  -> language()
  -> diagnostics()
  -> metadata()
  -> ts_compat::Tree::from_document(...)
```

The current alpha also preserves expected/found token names for generated
pure-Rust parser diagnostics, maps diagnostics back to related document-local
nodes when the selected tree or synthetic error tree carries one, and can return
a synthetic error document for truncated source when the parser records
diagnostics but cannot select a root. It also exposes
an explicit `NodeIdentity` view with separate visible and grammar identity slots
currently populated from the same raw parsed symbol, and
`AdzeDocument::ast_with_provenance()` as an alpha document-level typed AST
projection that pairs the extracted value with the document node used as the
extraction root. The same selected-tree extraction path is proven for generated
true-GLR documents, so conflicted generated grammars can project a typed AST
from `parse_document()` without creating a second parse truth. This is still an
experimental document proof, not a stable native diagnostics or per-AST-node
provenance schema. The current alpha also exposes a schema-tagged
`AdzeDocument::to_json_value()` projection for the same selected generic CST,
diagnostic, metadata, and ambiguity facts; it remains experimental and is not a
CLI/WASM output contract.

## Proof Requirements

Before any part of this surface is promoted beyond draft/advisory, it needs a
small contract test and a product proof command.

Minimum proof map:

| Surface | Required proof |
|---|---|
| Generic CST | Root, child edges, fields, spans, and flags are populated from one parse. |
| Tree-sitter projection | `ts_compat` methods read native data, not local guesses. |
| Typed CST | Generated wrappers access the same node IDs and edge fields as the generic CST. |
| Typed AST | Extraction walks the same document and records honest provenance. |
| Diagnostics | Structured diagnostics map to source spans and related nodes. |
| Ambiguity | Selected-tree summaries record alternatives and selection reasons. |
| JSON output | Schema snapshots include explicit version strings. |

Passing tests are not enough by themselves; each test must cover the stated
contract rather than a proxy.

## Support Status

This document does not change Adze support tiers. It records the intended native
API direction so implementation PRs can stay small and reviewable:

1. contract first,
2. minimal document alpha,
3. Tree-sitter projection over the document,
4. typed CST spike, generated-style wrapper scaffold, and tablegen generator target,
5. generated parser-module typed CST wiring,
6. generated `parse_document()` helper and typed CST runtime canary,
7. generated `parse_document()` diagnostics that preserve expected/found token
   names, byte spans, zero-based point ranges, source-context display, and
   partial document facts for truncated or multiline bad source,
8. diagnostic related-node IDs that resolve back into the same document tree,
9. document/node diagnostic lookup over related-node IDs,
10. typed AST extraction from generated pure-Rust documents,
11. document-level typed AST extraction provenance,
12. GLR ambiguity summaries,
13. schema-versioned `AdzeDocument::to_json_value()` alpha,
14. schema-versioned CLI/WASM outputs.
