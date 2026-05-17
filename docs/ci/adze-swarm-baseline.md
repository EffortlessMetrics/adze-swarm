# adze-swarm CI baseline

`adze-swarm` is the swarm working repo for Adze. It shares Adze project history and keeps the normal project-local instructions.

## Active base lane

The current durable base lane is:

- `EM CI Routed Rust`
- final check: `Rust Small Result`

This lane routes to the CX43 self-hosted runner when idle and falls back to GitHub-hosted when the runner is busy.

Runner-class policy is defined in [`runner-classes.md`](./runner-classes.md).
The short version is: CX43 owns the current `rust-small` base gate,
GitHub-hosted is scoped fallback plus Windows/macOS/public-fork/release
surface, and CX53 is the planned `rust-large` capacity tier once it is
registered and burned in.

The inherited public `ci.yml` full-CI workflow is retained for scheduled and
manual verification only. It does not run on ordinary swarm PRs or every merge
to `main`; that keeps the swarm base loop bounded while preserving an opt-in
full-CI path.

Inherited public push-triggered validation workflows such as pure-Rust matrix,
microcrate groups, golden tests, and test-policy runtime caps are also kept off
ordinary `main` merges in `adze-swarm`. They remain available through
path-routed/labeled PRs or manual dispatch where useful.

Microcrate CI is path-routed at both levels: crate-group tests run only for the
matching owner-module surface, and receipt jobs such as formatting, `cargo doc`,
WASM compile checks, and strict features run only for matching Rust/package
paths. Markdown-only docs changes should rely on the base gate and docs/policy
receipts instead of paying for workspace `cargo doc`.

Workflows that listen for `labeled` events guard their setup jobs so unrelated
labels do not restart path detectors or implementation lanes. Only labels that
request that workflow's evidence, such as `full-ci`, `platform-matrix`,
`coverage`, `ci:golden`, or `ci:microcrate`, should wake the matching routed
lane.

The Pure Rust workflow is code-path gated for ordinary PRs. Docs-only,
policy-only, CI-doc-only, and excluded tool-island changes such as
`tools/ts-bridge/**` should get the base `Rust Small Result` gate and their
focused receipts without also paying for the Ubuntu/stable Pure Rust lane.
Rust/package/runtime/workspace-tooling paths, the Pure Rust workflow itself,
`full-ci`, `platform-matrix`, and manual dispatch still run the Pure Rust jobs.

The `tools/ts-bridge/**` island is owned by `ts-bridge-smoke.yml`. Ordinary
bridge PRs run the Linux smoke only; `platform-matrix`, `full-ci`, and manual
dispatch can still request the macOS/Windows smoke matrix.

Coverage is split into `coverage-lite` and `coverage-full`. Lite coverage is
path-routed or label-routed for PRs and starts with the primary runtime package
so it stays cheap enough for PR evidence. Full coverage is manual or `full-ci`
evidence. In both modes, the LCOV artifact is the proof and Codecov upload is
non-blocking publication.

GLR/tablegen invariant proof is owned by the `GLR Invariants` policy lane. It
runs the `xtask` GOTO-indexing guard and its tests for runtime parser,
`glr-core`, and `tablegen` changes so GOTO-column remapping and `SymbolId(0)`
conventions stay attached to the parser proof surface.

## Public/release workflows

Release, publish, signing, Droid review, Droid security scan, and deploy-style workflows remain on public `EffortlessMetrics/adze` unless explicitly reintroduced here.

These workflows are not day-one `adze-swarm` workflows because they involve release authority, external credentials, deploy surfaces, or separate review policy.

## Staged later

The following are real CI lanes, but are staged pending routing or policy decisions:

- benchmarks
- performance
- fuzz

They should be restored deliberately once their runner class, trigger policy, and cost profile are clear.

## Operating rule

`adze-swarm` should carry swarm-safe verification. Public release and external-contribution workflows stay on public `adze` unless explicitly promoted into this repo.

## Cutover checkpoint

New swarm work targets `EffortlessMetrics/adze-swarm`. Keep public `EffortlessMetrics/adze` side by side as the release/public-history repo, and do not retarget its `origin` remote during swarm work.

Branches for swarm tasks should be created from `adze-swarm/main`, pushed to `origin`, and opened as same-repo PRs against `adze-swarm/main`. Do not push directly to `main`, do not target `adze-dev`, and do not use `em-ci` labels for swarm routing.
