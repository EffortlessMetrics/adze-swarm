# Contributing to Adze

## Choose the repository and work item

External contributions enter public `EffortlessMetrics/adze`. Active swarm
implementation, testing, documentation and CI work belongs in
`EffortlessMetrics/adze-swarm`. Do not move ordinary swarm work into the public
repository or publish a release as part of an implementation task.

For adze-swarm, read [AGENTS.md](AGENTS.md),
[the source-of-truth system](docs/reference/SPEC_SYSTEM.md), and
[the operating model](docs/reference/adze-swarm-operating-model.md).
#853 is the selected 0.10.0 product roadmap. Issues/PRs hold live coordination;
accepted specs/ADRs define behavior. There is no singleton active-goal requirement
for selecting an eligible issue. One work item per PR does not mean one active
PR for the entire repository.

Before starting, inspect the current owning issue, related PRs, source and review
findings. Reuse a same-scope PR instead of duplicating it. Define acceptance,
owned/shared/forbidden seams and focused proof before implementation. Independent
writers use separate worktrees; shared interfaces need explicit coordination.

## Prerequisites and setup

- The Rust toolchain selected by `rust-toolchain.toml`, including rustfmt/clippy.
- Git and `just`; `jq` for scripts that require it, and optionally ripgrep.
- Tree-sitter/Clang system dependencies only for the selected bridge/backend work.

Verify tools are actually available. A pinned toolchain file is not an execution
receipt. Use the selected repository/branch, not a different checkout's code.

For an existing adze-swarm clone and a new owned slice:

```bash
git fetch origin
gh pr list --repo EffortlessMetrics/adze-swarm --state open
git worktree add -b swarm/<task> ../adze-wt-<task> origin/main
```

### Git hooks

The portable hook configuration works with linked worktrees:

```bash
git config core.hooksPath .githooks
```

`.githooks/install.sh` is also available in a standard clone; its `.git/hooks`
assumption does not apply to linked worktrees. Inspect the existing hooks before
changing them. This guide does not disable an enforced hook or authorize bypass.
If a hook conflicts with the selected CI-efficiency contract, record that concrete
migration dependency rather than silently skipping it.

Automation lives in `xtask`; list current commands with `cargo xtask --help`.

## Implementation and proof

Use focused red/green/refactor cycles. Assert useful independently expected
values, tree facts, spans and errors; agreement between two projections alone
can preserve a shared bug. Do not accept a snapshot merely because it is the new
output. Invalid input must not become a plausible default value.

Typical focused commands (replace the selected package/target):

```bash
cargo test -p <crate> --test <target> -- --list
cargo test -p <crate> --test <target> -- --nocapture
cargo clippy -p <crate> --all-targets -- -D warnings
cargo fmt --all --check
```

Use the feature profile required by that behavior. Enumerate full test IDs before
using `--exact`; a helper name, cfg-disabled target, zero selected tests or an
ignored-only run is not proof. `--all-features` is not proof of each separately
advertised profile and is not mandatory for every PR.

`just ci-supported` is the existing aggregate for the seven core pipeline crates.
Run it when required by the selected supported-proof obligation; do not also run
its full lint/test workload immediately beforehand by habit. CLI, packaged
consumers and other release packages can need distinct proof outside that core
list. Read the selected release graph and support ledger for their scope.

Use `cargo insta review` for intentional snapshot changes. Example/bridge/fuzz
packages excluded from workspace commands require their actual manifest and
feature configuration, for example `--manifest-path example/Cargo.toml`; do not
assume a workspace `-p` selector can reach an excluded package.

Bound property/fuzz runs explicitly. Keep original findings and minimized ordinary
regressions. Coverage, full matrices, golden corpora, performance and mutation
campaigns are selected evidence, not automatic fanout for every edit. Concurrency
limits such as `CARGO_BUILD_JOBS` and `RUST_TEST_THREADS` bound local resource use.

### Record what actually happened

A receipt names source/build identity, toolchain/features, full command, exit
status and selected/executed/ignored counts. Preserve child exit status; no
`FAILED` text does not establish success. Record unavailable proof and the exact
qualification blocker. A source analysis or model cross-check is not a compiled
Rust test. Run named status generators rather than editing generated results.

## Pull request process

Use a descriptive branch and Conventional Commit title (`fix:`, `feat:`, `test:`,
`docs:`, `refactor:`, `perf:` or `chore:`). PRs are normally squash-merged.

Fill out the PR template with the issue/spec/ADR, requirement delta, seams,
non-goals, proof and rollback. Keep a coherent semantic change per PR; an approved
instruction-reconciliation slice can update several documentation entrypoints.
Substantive review and repair happen before expensive qualification. Previous
clean reviews do not suppress new findings.

### Qualification is not every available workflow

Follow the actual protected path and #1025–#1028's deployment readback. The target
is focused local proof/review, cheap exact-head admission, one qualification on
the actual integration composition, then protected landing and cheap readback.
The October 22, 2026 deployed #1070 workaround still qualifies non-draft PRs and
the queue separately. Do not treat this as the final spending policy or remove
required producers before a protected replacement works.

The observed required contexts are `Rust Small Result` and `Product Proof Result`;
recheck effective protection before landing. Individual routed jobs can be
intentionally unselected; selected proof cannot pass through unexpected skip,
cancellation, absence or failure. Do not relax protection, manufacture green
statuses or assume a manual dispatch satisfies native enqueue.

Cancel only explicitly owned run IDs after checking event/ref/head/workflow
identity. Never cancel unknown runs by repository-wide workflow-name sweeps.
Keep pending current attempts visible rather than selecting an old success.

### Close a slice, not the campaign

After landing, reconcile the actual child acceptance. Leave a broader parent
open when requirements remain. Select the next eligible issue rather than stop
because the batch or PR list is empty. A blocked lane states its exact blocker;
independent useful work continues. No arbitrary session percentage establishes
product readiness.

Clean only owned worktrees/cache after changes and receipts are retained. Inspect
registrations and state before removal:

```bash
./scripts/cleanup-worktrees.sh list
./scripts/cleanup-worktrees.sh status <owned-worktree-path>
```

Run the cleanup helper only after validating ownership and preserving uncommitted
work. On Windows, use the corresponding `just` recipes or Git Bash as appropriate.

## Code and architecture conventions

Follow nearby patterns. Public APIs need useful rustdoc, including errors/panics
where applicable. Use workspace dependencies, `thiserror` for library errors and
`anyhow` for application context. Preserve workspace lints. Test assertions remain
oracles; intentional production panic-family exceptions require policy review.

The core pipeline spans macro/tool/common, IR, GLR generation, tablegen and the
runtime. `parse()` is the typed-value fast path; `parse_document()` owns canonical
tooling facts. Document/CST/AST/query/JSON/CLI projections do not invent separate
parse semantics. runtime2 is a development-only proving ground, not the supported
runtime. Declared features are not automatically supported product claims.

For new grammars and user-facing recipes, start from existing compile-tested
examples and the selected accepted contract rather than an unverified copied
snippet. Register the fixture and prove exact values/tree facts, invalid input,
and the required feature profiles. Consult `docs/status/SUPPORT_TIERS.md` and
`docs/status/KNOWN_RED.md` before describing support or exclusions.

## Release boundary

Implementation and candidate qualification do not authorize publication. #325 and
[the publish checklist](docs/reference/PUBLISH_CHECKLIST.md) own the separate
promotion/release decision. Retain exact source/archive/install receipts for
selected platforms; do not turn checkout-based success into a crates.io claim.

## Getting help and conduct

Use the current owning issue and review thread for reproducible findings and
concrete blockers. `cargo xtask --help`, the justfile and accepted specs provide
command/behavior references; CLAUDE points to the same canonical AGENTS guidance.

All participants are expected to follow the
[Contributor Covenant Code of Conduct](CODE_OF_CONDUCT.md).
