# ADZE-SPEC-0019: nullable/FIRST/FOLLOW conformance profile `cfg.nullable_first_follow.v1`

Status: proposed
Owner: conformance
Created: 2026-10-20
Linked proposal: none
Linked ADRs: none
Linked plan: none
Linked issues: #973, #974, #975, #976, #977
Support-tier impact: none (no support claim changes)

## Problem

Nullable, FIRST, and FOLLOW sets are computed by the production Rust code
in `adze-glr-core`, but nothing outside that implementation defines what
the correct answers are. Property tests generated inside the same codebase
cannot catch a systematically wrong fixed point, because the generator and
the implementation share assumptions. This spec defines an engine-neutral
contract, `cfg.nullable_first_follow.v1`, plus a small dense corpus with
hand-reasoned expectations, so that an independent reference implementation
([#975](https://github.com/EffortlessMetrics/adze-swarm/issues/975)) and the
Rust implementation (driven through
[#976](https://github.com/EffortlessMetrics/adze-swarm/issues/976)) can be
compared under [#977](https://github.com/EffortlessMetrics/adze-swarm/issues/977)
without either side's output becoming the expectation authority.

This spec owns meaning and cases only. It does not own the Rust
implementation, the Python reference, any adapter, or CI policy.

## Behavior

### B1. Input contract

A case is a JSON object:

```text
profile:      "cfg.nullable_first_follow.v1" (exact string)
case_id:      stable identifier matching [a-z][a-z0-9_]*, at most 64 bytes
start:        name of a declared nonterminal
terminals:    unique names
nonterminals: unique names, disjoint from terminals
productions:  list of { lhs, rhs } where lhs is a declared nonterminal and
              rhs is a sequence of declared terminal/nonterminal names;
              [] is the only epsilon spelling
```

Names match `[A-Za-z_][A-Za-z0-9_]*` and are at most 64 UTF-8 bytes. The
semantic EOF marker is reserved: it cannot be declared as a terminal or
nonterminal name, and engines must refer to it only through the `eof`
boolean in FOLLOW output. Numeric symbol IDs (including `SymbolId(0)` for
EOF) are adapter implementation details, never profile input.

Bounds: at most 64 total declared symbols, at most 256 productions, at
most 32 symbols per RHS, at most 4096 total RHS occurrences.

Every declared nonterminal has at least one production. Unreachable
nonterminals are allowed and still receive exactly one output row.
Production order, declaration order, and map order are not semantic.
Exact duplicate productions are rejected.

### B2. Output contract

For every case, an engine result is:

```text
nullable: sorted set of nonterminal names
first:    map from every declared nonterminal to a sorted set of terminal names
follow:   map from every declared nonterminal to
          { terminals: sorted set of terminal names, eof: boolean }
```

Semantics are the ordinary whole-grammar fixed points:

- FIRST is over sentential forms, not only fully terminating strings; a
  recursive or nonproductive nonterminal may still contribute a terminal
  prefix.
- Nullable propagation continues through an RHS only while its prefix is
  nullable.
- FOLLOW is computed with EOF seeded only on the explicit start symbol;
  declaration or insertion order must not affect the result.
- FOLLOW handles nullable suffixes and repeated occurrences of the same
  nonterminal in one RHS.

### B3. Corpus and expectation provenance

The shared corpus lives at
`conformance/cases/cfg.nullable_first_follow.v1/corpus.json` and declares
its required denominator (`required_denominator`) before any execution.
The v1 denominator is 13, one case per coverage family named in
[#974](https://github.com/EffortlessMetrics/adze-swarm/issues/974): single
terminal, direct epsilon, nullable prefix, nullable suffix, fully nullable
production, nullable chain, left recursion, mutual recursion, repeated
occurrence, terminal stopping propagation, explicit start differing from
declaration order, unreachable component, and recursive/nonproductive
terminal prefix.

Every expectation records exactly one authority class:
`reasoned_semantic`, `curated_gold`, `external_oracle`, `snapshot`, or
`generated_relation`. All v1 expectations are `reasoned_semantic`: derived
by hand from B2, never from running an implementation. Machine-derived
output must never be relabelled as curated gold. Expectations live only in
the corpus; they are never part of an engine request.

### B4. Canonical identity

Set and map ordering is canonicalized (sorted names) before comparison or
hashing. Case ID, exact input digest, profile/schema version, and
expectation digest are bound separately. A result row is invalid when it
omits or adds a nonterminal, names an undeclared terminal, or encodes a
boolean as an integer lookalike.

### B5. Rejection contract

Case documents must be rejected when they have: a missing or unknown
start; duplicate or empty names; the reserved EOF name declared; a
terminal/nonterminal name collision; an undefined symbol reference; a
production LHS that is not a declared nonterminal; a declared nonterminal
without a production; a duplicate production; a malformed case ID; a
profile or schema-version mismatch; or any B1 bound overflow. The corpus
carries representative invalid documents under `invalid_cases`, each with
its rejection class, so validators can be tested without inventing new
malformed inputs.

### B6. Non-goals

This profile does not cover complex Adze symbols (`Optional`, `Repeat`,
`Choice`, `Sequence`) or normalization; engines may normalize internally,
but agreement under this profile proves only primitive-input
nullable/FIRST/FOLLOW semantics. It implies no parser, Tree-sitter,
support-tier, or release claim.

## Acceptance

- [ ] Meaning is reviewable without either implementation (this spec).
- [ ] Corpus covers the 13 dense families with rationale and provenance.
- [ ] Schemas reject malformed, ambiguous, and incomplete data
      (`conformance/schemas/`).
- [ ] Exact nonzero denominator is declared before execution (13).
- [ ] Expected values cannot reach engine requests (expectations live only
      in the corpus document, never in a request shape).
- [ ] Order invariance is stated as a reusable relation (B1/B2).
- [ ] File-policy ownership covers every new non-Rust artifact
      (`policy/non-rust-allowlist.toml`).
- [ ] No normalization/parser/Tree-sitter claim is implied (B6).

## Proof

```bash
cargo test -p adze-glr-core --test conformance_corpus_contract
git diff --check
```

## Rollback

Remove the v1 profile, corpus, schemas, and file-policy registration. No
production parser behavior needs rollback.
