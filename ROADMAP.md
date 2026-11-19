# Adze Roadmap

When syntax changes, the parser and the Rust types that consume its output
need to stay in sync. Adze lets you put grammar rules on those types and
generate a parser at build time. The current goal is to make that path easy to
start, easy to diagnose, and repeatable from packaged artifacts.

## What Works From a Checkout

The generated pure-Rust parser is the default path. Start with the
[checkout starter](docs/tutorials/quickstart-10-minutes.md#create-the-project)
on Rust 1.95.0 or newer. Use `grammar::parse()` for typed Rust values and
`grammar::parse_document()` for tooling that needs diagnostics or a selected
syntax tree.

[Support Tiers](docs/status/SUPPORT_TIERS.md) records what is tested and where
the limits are. The generated pure-Rust and typed extraction paths are Stable
within their documented scope. Documents, structured errors, GLR conflict
routing, and the selected-tree and query subsets are Stabilizing. Incremental
parsing is Experimental. WASM has an Advisory compile check; it does not
establish browser behavior or support for every crate.

Earlier milestones introduced the pure-Rust runtime, precedence handling,
and GLR conflict routing. See the [changelog](CHANGELOG.md) for that history.
Table size and generation cost depend on the grammar. Use the
[performance evidence](docs/status/PERFORMANCE.md) for measured workloads
rather than assuming a fixed compression ratio.

## Next Release Candidate: 0.10.0

The [0.10.0 product milestone](https://github.com/EffortlessMetrics/adze-swarm/issues/853)
tracks the remaining acceptance work. This is a candidate plan, not a release
announcement.

- **Make diagnostics and tooling output useful.** Keep the first parser
  runnable from a checkout, expand the tested grammar and malformed-input
  cases, and state clearly which document, query, and projection behavior is
  covered. The [product acceptance matrix](docs/product/ACCEPTANCE_MATRIX.md)
  describes these workflows and their proof.
- **Prove the packaged starter.** Verify the same candidate on Linux and
  Windows through the [package-first milestone](https://github.com/EffortlessMetrics/adze-swarm/issues/880).
  A checkout build or an isolated local-registry check does not prove a
  crates.io install.
- **Prepare the public promotion decision.** Record the exact source and target
  in the [promotion packet](https://github.com/EffortlessMetrics/adze-swarm/issues/881).
  The [package-boundary ledger](policy/package-boundary.toml) and
  [release graph](policy/release-graph.toml) determine which crates belong in the
  candidate and their dependency order. The earlier crate consolidation is
  recorded in the [migration plan](plans/0.9.0/microcrate-collapse.md); keep
  temporary migration crates out of the release graph.

Use the checkout flow for both the runtime and the build-time tool
(`adze-tool`). Release-graph membership does not mean a package is available
from crates.io. Public promotion, publication, and the real registry install
receipt are tracked separately in
[release decision #325](https://github.com/EffortlessMetrics/adze-swarm/issues/325).

## After the Candidate

Measure parser and IDE workflows before making stronger performance or
incremental-reuse claims. Expand platform and editor support where repeatable
tests justify it. The support tiers, not a milestone label, define when an API
or platform is ready to rely on.
