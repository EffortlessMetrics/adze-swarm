# GLR Toolkit Fixture Taxonomy

Status: active
Owner: runtime/product
Linked proposal: ../proposals/ADZE-PROP-0003-glr-toolkit-productization.md
Linked spec: ../specs/ADZE-SPEC-0012-glr-toolkit-product-contract.md
Linked plan: ../../plans/glr-toolkit/productization-plan.md

This taxonomy names the fixture classes used to prove Adze as a GLR parser
toolkit. It is intentionally product-shaped: each fixture must say which user
surface it protects and which projection facts are expected.

## Fixture Roots

```text
tests/fixtures/glr/
tests/fixtures/ts-compat/
tests/fixtures/query/
tests/fixtures/recovery/
```

Each fixture family should eventually contain grammar sources, input files, and
expected facts. Until the projection harness exists, `tests/fixtures/catalog.toml`
is the lightweight registry for planned fixtures.

## Fixture Record Shape

Each fixture record should name:

```text
id
family
grammar
source input
expected selected-tree shape
expected ambiguity summary
expected diagnostics
expected Tree-sitter-compatible projection
expected query behavior, when relevant
expected node-types metadata, when relevant
support-tier relevance
proof command
known gaps
```

## GLR Fixture Classes

| Class | Purpose | Required facts |
| --- | --- | --- |
| valid deterministic grammar | Baseline non-ambiguous parse behavior. | selected tree, no ambiguity, typed AST agreement |
| single shift/reduce conflict | Prove conflict cell generation and deterministic selection. | conflict retained, selected tree, ambiguity summary |
| single reduce/reduce conflict | Prove multiple reductions survive tablegen/runtime. | retained alternatives, deterministic selection |
| nested fork conflict | Prove stack splitting and merging across nested ambiguity. | selected tree, alternative count, no panic |
| multi-conflict expression grammar | Prove precedence/associativity across interacting conflicts. | selected tree, typed AST agreement |
| dangling-else grammar | Prove selected-tree policy for a known ambiguity. | selected tree, ambiguity summary |
| ambiguous list grammar | Prove repeated ambiguous structure does not collapse incorrectly. | alternative count, selected tree |
| ambiguous prefix/postfix grammar | Prove adjacent operator ambiguity behavior. | selected tree, retained alternatives |
| bad input inside ambiguous grammar | Prove recovery does not panic in GLR paths. | diagnostics, document tree, error flags |

## Tree-sitter Compatibility Fixture Classes

| Class | Purpose | Required facts |
| --- | --- | --- |
| field-heavy grammar | Edge field metadata and lookup parity. | field names, field IDs, child lookup |
| alias-heavy grammar | Visible vs grammar identity parity. | kind, grammar name, node-types facts |
| hidden/extra node grammar | Namedness and extra-node behavior. | named count, extra flags |
| missing/error node grammar | Recovery projection parity. | is_error, is_missing, has_error |
| S-expression grammar | Selected-tree rendering parity. | stable S-expression |
| node-types metadata grammar | Editor metadata readiness. | fields, children, aliases, namedness |

## Query Fixture Classes

| Class | Purpose | Required facts |
| --- | --- | --- |
| named node patterns | Baseline query matching. | captures and match ranges |
| anonymous token patterns | Source-aware literal behavior. | fail-closed without source |
| child quantifiers | Repeat/backtrack behavior. | `?`, `*`, `+` matches |
| sibling sequences | Ordered child matching. | capture order |
| field constraints | Field-aware query behavior. | field match and mismatch |
| anchors | Positional query behavior. | first/last/adjacent constraints |
| predicates | Source-aware predicate behavior. | `#eq?`, `#not-eq?`, `#match?`, `#any-of?` |
| byte range and root-only | Cursor option behavior. | filtered matches only |

## Recovery Fixture Classes

| Class | Purpose | Required facts |
| --- | --- | --- |
| bad token | Invalid-token diagnostics. | byte span, point range, expected tokens |
| unexpected EOF | Zero-width or end-span diagnostics. | EOF span and expected set |
| missing close delimiter | Missing-node recovery. | missing flag and diagnostic |
| bad separator | Local recovery without cascade. | diagnostic and selected tree |
| multibyte bad token | UTF-8 byte and point accounting. | byte span and point range |
| multiline bad token | Multi-line excerpt rendering. | line/column and caret/excerpt |
| ambiguous input with error | GLR recovery safety. | no panic, ambiguity and diagnostics |
| external scanner error | Scanner integration behavior. | scanner diagnostic or known gap |

## Promotion Rule

A support-tier row may cite a fixture class only after there is:

- at least one concrete fixture for the class;
- a proof command that exercises it;
- a known-gaps entry for unsupported behavior;
- a support-tier row that names the protected user surface.
