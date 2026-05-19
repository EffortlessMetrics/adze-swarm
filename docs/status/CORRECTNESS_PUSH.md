# Correctness Push Plan

**Last updated:** 2026-05-19
**Scope:** current parser/runtime, GLR, tablegen ABI, CLI, and product-proof convergence.

This is the execution playbook for moving Adze from "bounded core lane is green" to "the product claims are behavior-proven." It is intentionally narrower than a roadmap: keep the required lane bounded, land focused correctness work only when it has receipts, and track remaining product gaps without hiding them inside broad policy or infrastructure PRs.

## Baseline

- Required `adze-swarm` GitHub gate is `Rust Small Result`.
- Fast local supported proof stays `just ci-supported`.
- `ci-supported` covers the seven core crates: `adze`, `adze-macro`, `adze-tool`, `adze-common`, `adze-ir`, `adze-glr-core`, and `adze-tablegen`.
- As of `adze-swarm` PR #157, the supported proof uses
  `scripts/fmt-workspace.sh` for formatting so Windows local runs avoid
  Cargo/rustfmt command-line length failures while keeping the same supported
  crate set and clippy/test/doc-test scope.
- The broader product lane is advisory until each canary proves real behavior instead of only compile/no-run smoke.
- README-stable claims must map to a proof command in `docs/status/SUPPORT_TIERS.md`.
- Runtime2 remains an experimental proving ground unless a later promotion plan gives it required behavior tests and a public support contract.
- Live GitHub state is the execution baseline. As of 2026-05-19
  after `adze-swarm` PR #248, live `gh pr list` checks showed no open PRs in
  `EffortlessMetrics/adze-swarm` or public `EffortlessMetrics/adze`.

## Live-State Refresh

Before merging any queued PR, refresh the real GitHub state:

```bash
gh pr list --state open --limit 50 \
  --json number,title,mergeable,isDraft,headRefName,baseRefName,updatedAt,url
```

If GitHub API access is rate-limited, do not guess mergeability. Continue with local rebases and tests, but report that live PR count could not be refreshed.

## Current Queue State

The historical correctness merge queue is closed. Do not resurrect old PR numbers from handoff notes unless `gh pr view <PR>` shows that the PR is still open and relevant.

For any new correctness PR:

```bash
git checkout main
git pull --ff-only
gh pr checkout <PR>
git fetch origin main
git rebase origin/main
cargo fmt --all -- --check
just ci-supported
```

Apply the same rules that worked for the closed queue:

- Keep each PR narrow and parser/tablegen/runtime focused.
- Rebase on current `main` and use hosted `Rust Small Result` as the required merge gate.
- Add a focused behavior canary before claiming a product surface is proven.
- Do not weaken strict canaries to make a dashboard green. The closed #501 JavaScript canary-ignore PR is the example to avoid.
- Do not let broad policy, coverage, or governance work consume the parser-correctness lane.

## Post-Queue Issues

The post-queue work is tracked as focused issues instead of broad catch-all implementation PRs:

- [#460](https://github.com/EffortlessMetrics/adze/issues/460) GLR product proof: conflict-preserving end-to-end typed extraction.
- [#461](https://github.com/EffortlessMetrics/adze/issues/461) Tablegen ABI completeness: conflict encoding/routing, field maps, symbol/state invariants.
- [#463](https://github.com/EffortlessMetrics/adze/issues/463) Parse diagnostics: spans, expected token sets, line/column mapping, excerpts, no panic.
- [#464](https://github.com/EffortlessMetrics/adze/issues/464) CLI clean-room quickstart and parse command truthfulness — closed after behavior canaries landed; reopen or replace only for new CLI parser behavior work.
- [#465](https://github.com/EffortlessMetrics/adze/issues/465) README/support-tier reconciliation — closed after the proof map and stable product lane landed; keep the invariant in future docs edits.
- [#629](https://github.com/EffortlessMetrics/adze/pull/629), [#630](https://github.com/EffortlessMetrics/adze/pull/630), and [#631](https://github.com/EffortlessMetrics/adze/pull/631) extended the stable quickstart proof to a checked-in downstream demo, the Getting Started tutorial, and tutorial bad-input diagnostics.
- [#73](https://github.com/EffortlessMetrics/adze/issues/73) and [#75](https://github.com/EffortlessMetrics/adze/issues/75) Benchmark truthfulness: real parser work vs infrastructure-only measurements.

## Native Product Contract Lane

The next product work should keep Tree-sitter compatibility as a conformance
adapter and Adze-native output as the parse-product API. Contract docs already
pin the intended shape:

- [`docs/design/adze-document.md`](../design/adze-document.md): monomorphic
  `AdzeDocument`, generic CST, diagnostics, metadata, ambiguity summaries, and
  lazy projections.
- [`docs/design/typed-cst.md`](../design/typed-cst.md): generated typed CST
  handles over `AdzeDocument`, not a second parse tree.
- [`docs/reference/ts-compat-alias-semantics.md`](../reference/ts-compat-alias-semantics.md):
  current alias-visible identity behavior and remaining alias parity gaps for
  the compatibility adapter.

Keep implementation slices small:

1. Minimal `AdzeDocument` alpha now exists for `tree()`, document-local node
   IDs/lookups, explicit `NodeIdentity` slots for alias-visible and raw grammar
   identity projection, alias-adjusted `NodeFlags` for the current
   named/visible/extra/token/error/missing/aggregate-error state, edge/parent lookup,
   language/node-kind metadata, `diagnostics()`, `metadata()`, and
   `ts_compat::Tree::from_document()` over the same parse data. It now also
   has an experimental `AdzeDocument::to_json_value()` projection under
   `serialization` that emits schema-tagged `adze.document.v1` facts for the
   selected generic CST, diagnostics, metadata, and ambiguity summaries, with
   representative clean, EOF diagnostic, multibyte diagnostic, multiline
   diagnostic, and ambiguous GLR document snapshots. Keep expanding it in small
   proof-backed slices, and do not treat this as a stable CLI/WASM `adze-json`
   contract.
2. A typed CST arithmetic spike now proves a generated-style fixture module,
   the runtime `SyntaxNode` handle contract, typed field accessors, spans,
   text, and recovery flags over document node IDs. Tablegen also has an alpha
   typed-CST generator target that emits the same wrapper/accessor shape from
   `Grammar` metadata, and Pure-Rust generated parser modules now append that
   alpha `syntax` module plus a generated `parse_document()` helper. The runtime
   canary now proves that helper feeds generated wrappers from the same
   `AdzeDocument` and that retained typed wrappers agree with the generic CST on
   node IDs, kind names, byte ranges, and text. A generated fielded-struct
   canary now proves FIELD metadata survives Rust expansion into terminal-backed
   converter `SEQ` productions, ABI field maps, `AdzeDocument` edge metadata,
   and generated typed CST `left`/`right` accessors. A follow-on generated
   precedence enum canary now proves explicit `left`/`operator`/`right` fields
   survive precedence operator inlining into native edge metadata and generated
   typed CST accessors. The same runtime canary now proves a validated typed
   CST wrapper can extract a semantic typed AST from its own document node while
   preserving document-level node provenance, without expanding into visitors,
   rewriters, typed queries, or JSON output.
3. Initial alias-visible compatibility canaries now prove native document node
   identity and parsed `ts_compat` node/S-expression projection for known
   production alias sequence entries. Remaining alias work is node-types output,
   query-compatible metadata/execution, anonymous alias named-child filtering,
   GLR/generated-route parity, and imported corpus fixtures.

Do not promote any native document, typed CST, alias-visible compatibility, or
JSON output surface without an exact proof command and support-tier entry.

## Green Ladder

Rung 0 is the current local supported proof:

```bash
just ci-supported
```

The current `adze-swarm` required GitHub merge gate is `Rust Small Result`.

Rung 1 is advisory product behavior. Convert `scripts/ci-product.sh` from compile-only smoke to bounded behavior smokes, but keep it non-blocking until stable.

Rung 2 is a stable product lane. A candidate `just ci-product-stable` lane now exists for README-stable claims, but it remains advisory until branch protection promotes it:

```bash
just ci-supported
just ci-product-stable
```

The stable product lane covers README stable proof-map alignment, clean-room README and Getting Started quickstarts, the checked-in downstream demo library and binary run, typed extraction exact-value and repeated-parse determinism tests, operator precedence, serialization doctests, and serialization roundtrip canaries. GLR ambiguity and broad structured parse-error diagnostics remain in the wider advisory lane until those surfaces graduate from Stabilizing.

Latest receipt: GitHub workflow dispatch
[`Product Proof` run 26104726428](https://github.com/EffortlessMetrics/adze-swarm/actions/runs/26104726428)
passed on 2026-05-19 from current `adze-swarm/main` after PR #281. The
`ci-product stable canaries` job passed in 3m02s and the broad advisory
canaries skipped under the stable-only default. This remains advisory until
branch protection explicitly promotes it.

Rung 3 remains scheduled/manual: full workspace all-features, fuzzing, Miri, sanitizers, benchmarks, grammar corpus, runtime2, and browser WASM execution.

## Reporting Format

After each merge or failed merge attempt, report:

- PR handled.
- Proof commands run.
- Current open PR count, or why it could not be refreshed.
- Red checks.
- Next blocking PR.
