# 0.10.0 Promotion Packet (#881)

> **This packet does NOT authorize promotion or publication.**
> It is evidence assembly only. Promotion to `EffortlessMetrics/adze` and any
> crates.io publication require separate explicit decisions under #325/#881.
> No tag, signing, or publication is implied by this document.

Machine-readable twin: `docs/release/promotion-packet-0.10.0.json`

**Candidate status:** `e0c2a316` is a *proposed* candidate (current swarm
`main` HEAD). No maintainer has selected it under #325; this packet is not a
selection receipt. Tested-tree identity: swarm `main` working tree was clean
at `e0c2a316` when the receipts below were produced. Lock identity:
`Cargo.lock` SHA-256 `7d78a208a886be91c5f3ddb0a77c3b5fc54efb33bd13936aafc1b24df36185f8`.
Original/transformed local-registry archive digests were not retained by the
journey harness; archive-byte identity with any future crates.io artifact is
explicitly **not claimed**.

## Identity

| Field | Value |
|---|---|
| Version | 0.10.0 |
| Source repo | `EffortlessMetrics/adze-swarm` `main` |
| Source commit | `e0c2a316f158eb33732623dd7198de421c26bf7f` |
| Source tree | `1d7d3030fbc7bb56c63eb2e56034b73fd66ef438` |
| Public target repo | `EffortlessMetrics/adze` `main` |
| Public target commit | `6263c6a80046d13fb98e3ad319dfe726f32f1010` |
| Public target tree | `565e5ea130a18cb3afcd2bbc03156de228cb8cba` |

Public target verified 2026-10-30:
`git ls-remote https://github.com/EffortlessMetrics/adze.git refs/heads/main`
returned `6263c6a80046d13fb98e3ad319dfe726f32f1010`. A shallow clone at
`H:/tmp/adze-public` has that HEAD/tree (read-only drift comparison only).

## Drift inventory (public target -> selected source)

Method: in the shallow public clone, fetch the selected swarm commit, then
`git diff --stat/--shortstat/--name-status HEAD FETCH_HEAD`.

**Summary: 973 files changed, 61,153 insertions, 26,289 deletions**
(modified 653, added 249, deleted 69, renamed 2).

Largest top-level areas (files changed): `runtime` 199, `docs` 103, `ir` 71,
`tablegen` 57, `tool` 56, `macro` 54, `glr-core` 48, `plans` 40, `book` 38,
`.adze` 37, `example` 30, `crates` 28, `.github` 26, `golden-tests` 23,
`testing` 22, `scripts` 18, `xtask` 18, `grammars` 17, `reference` 11,
`conformance` 9, `policy` 8. Full per-directory counts are in the JSON twin.

Added-only highlights: `runtime` 48, `plans` 36, `.adze` 36, `docs` 32.
Deleted-only highlights: `testing` 22, `golden-tests` 18, `crates` 11.

### Package / workspace changes

- Source `Cargo.toml` workspace members no longer list `testing` and
  `test-mini` (members in the public target).
- Source `Cargo.toml` exclude no longer lists `crates/ts-c-harness`;
  source exclude is `runtime/fuzz`, `tools/ts-bridge`, `example`.
  `ts-c-harness` remains excluded from default workspace commands and is
  never a release crate.
- All 12 release-graph crates report **0.10.0** in source
  (`cargo metadata --no-deps` at `e0c2a316`). The public target predates the
  0.10.0 bump (swarm PR #1116).
- Source adds the release-graph authority `policy/release-graph.toml`
  (generated from `policy/package-boundary.toml`); the public target has no
  equivalent 0.10.0 graph.

### Swarm-only excluded artifacts (do not bulk-promote)

- `.adze/goals/**` (36 added) — swarm goal/migration artifacts.
- `plans/**` (36 added) — swarm planning artifacts.
- `conformance/**`, `reference/python/**` — swarm proof assets; promote only
  by explicit public decision.
- `policy/**` ledgers, `xtask` queue/policy governance, swarm
  `.github/workflows` routing (cx33/cx43/cx53/cpx42) — swarm CI/governance.
- `crates/ts-c-harness`, `runtime/fuzz`, `tools/ts-bridge`, `example` —
  workspace-excluded; never in the release graph.
- Swarm `docs/status`, `docs/proposals`, `docs/ci` — review individually.

**Caution:** 973-file drift is not a cherry-pick. Promotion must be a reviewed
public promotion PR per `docs/reference/PUBLISH_CHECKLIST.md`, not a tree
copy. A non-empty `public/main..swarm/main` diff blocks publishing from swarm.

## Exact 12-crate release graph at 0.10.0

Source of truth: `policy/release-graph.toml` (generated, ledger-published,
dependency order). Regenerate/check:
`cargo run -q -p xtask -- generate-release-graph | check-release-graph | print-release-graph`

Publish in this exact order:

1. `adze-bdd-governance-core`
2. `adze-common-type-ops-core`
3. `adze-common`
4. `adze-ir`
5. `adze-glr-core`
6. `adze-linecol-core`
7. `adze-macro`
8. `adze-parsetable-metadata`
9. `adze-tablegen`
10. `adze`
11. `adze-tool`
12. `adze-cli`

## Receipts

- **Journey:** `cargo run -p xtask -- verify-local-release-install --version 0.10.0`
  exited 0 on `e0c2a316` (log `H:/tmp/journey-final.log`, completed
  2026-10-30T17:24Z, after the #1118 merge at 17:03Z) — **local-registry claim
  boundary only** (publish/install/`adze init` scaffold proof against an
  isolated local registry). This is NOT a crates.io claim and does not prove
  public install.
- **Focused suites on `e0c2a316`** (all exit 0, nonzero denominators):
  `cargo test -p adze --features "pure-rust,glr,ts-compat" --test generated_parse_errors`
  23 passed; `cargo test --manifest-path example/Cargo.toml --lib repetitions`
  16 passed; `--lib boolean` 6 passed; `--lib csv` 5 passed.
- **PR gates 1101-1121:** verified via `gh pr view` on 2026-10-30. All
  actual PRs are MERGED, including #1119 (#1078 query fix), #1120 (this
  packet's first version), and #1121 (publish-metadata fix). `1107` is an
  issue, not a PR; its fix landed via #1114/#1117.
- **Query suite on candidate:** `cargo test -p adze --features query --lib query::`
  68 passed, exit 0.
- **Publishability:** `just check-publishable` - all checks passed for the 12
  release-graph crates on this candidate, exit 0 (after #1121).

### Explicit non-claims (no receipt exists in this packet)

| Claim | Status | Owner |
|---|---|---|
| Supported-gate (`just ci-supported`) aggregate on the candidate | Not run locally; exact-head CI gates on the merged PRs are the integration evidence | #1028 |
| Corrected all-canaries Stable Product Proof | No current executed receipt | #854 |
| Linux/Windows package-first portability receipts | Windows journey only (this machine); no separate Linux receipt | #880 |
| Parser/GLR/document/projection matrix receipts | Not assembled | #853 |
| No-panic and API-surface receipts | Not assembled | #867/#1072 |
| Fixed-runner performance baseline | Not assembled; no performance claim | #869 |
| Known-red list | Example crate has 15 pre-existing failures on main (arithmetic, ini, json_like, lambda, optionals, regex, words), unchanged from baseline | repo `KNOWN_RED.md` |
| Public API surface diff / detailed Cargo dependency diff | Not generated; dependency identity is the `Cargo.lock` hash above only | this packet (limitation) |

## Open items and deferrals (none are "accepted" without a cited decision)

| Item | State (2026-10-30) | User impact | Future owner |
|---|---|---|---|
| #853 roadmap | OPEN | 0.10.0 endpoint not frozen; candidate is usable-core only | #853 (58 gate owners still open) |
| #1078 query fail-closed | CLOSED via PR #1119, included in this candidate | None for the admitted query subset; full query parity still open under #861 | #861 |
| #325 publish authorization | OPEN, no A-E decision recorded | No promotion, tag, or publish can start | #325 (maintainer decision) |
| #881 this packet | OPEN | Packet exists; promotion itself unauthorized | #881, then #325 |
| macOS | UNPROVED | No macOS support claim | #880 |

## Incident communication plan (for the future authorized run)

1. Any abort after a publish step starts is recorded the same day as a comment
   on #325 naming the crates already published, their versions, and the abort
   criterion that fired.
2. If a defective 0.10.0 crate becomes visible on crates.io, the yank decision
   and reverse-order yank list are posted to #325 before yanking, and the
   public release notes are corrected to withdraw the install claim.
3. User-facing status is updated in public `adze` (release notes + README
   claim boundary) by the release owner named under #325; until that owner is
   named, this plan has no executor and publication stays blocked.

## Release operations plan (for the future, separately authorized run)

Preconditions, in order: (1) explicit promotion decision naming `e0c2a316`;
(2) reviewed public promotion PR resolving the drift above; (3) on public
main, confirm the candidate diff is empty or every remainder is accepted;
(4) re-run `just check-publishable`, release-graph and package-boundary gates,
and focused suites on the public tree.

- **Package order:** the 12-crate graph order above. Wait for each crate to
  appear on crates.io before publishing dependents (`./scripts/release.sh` or
  `cargo publish -p <crate>` in graph order).
- **Tag/changelog:** finalize `CHANGELOG.md` `[0.10.0]` (currently an
  unreleased candidate) with a date on public main only; tag `v0.10.0` on the
  promoted public commit only after the #325 publish decision. No tag is
  created by this packet.
- **Signing / token owners: VISIBLY ABSENT.** No signing identity, crates.io
  token owner, or credential is named in this packet. Publication must not
  proceed until #325 names them via the approved secret channel.
- **Post-publish verify (only after crates.io visibility):**
  `cargo run -q -p xtask -- verify-crates-io-install adze-cli --bin adze --version 0.10.0 --locked`
  (`--dry-run` proves command shape only, not registry availability.)

Abort, in order: (1) swarm-only artifacts in the promotion PR without explicit
acceptance; (2) any release-graph/package/`cargo package` gate fails on the
public tree; (3) any of the 12 crates or inter-crate deps != 0.10.0; (4) a
crates.io dependency not yet visible for its dependent — stop, never skip
order; (5) post-publish verify fails — make no install claim, file the gap
under #325.

Rollback, in order: (1) before any publish, withdraw the promotion PR — no
rollback needed; (2) after partial publish, crates.io cannot unpublish — yank
0.10.0 in reverse graph order (`adze-cli` first) via the #325 token owner and
ship a corrected patch release, never reuse 0.10.0; (3) post-tag abort: move
or delete `v0.10.0` only by explicit release-owner decision recorded in #325,
never silently retag a different tree. Yank/rollback owner is likewise
**ABSENT** until named under #325.
