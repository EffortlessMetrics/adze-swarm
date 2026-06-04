# Tooling substrate standard

Status: proposed operating standard
Owner: repo governance
Created: 2026-06-03
Scope: repo-facing CI, policy, proof, and source-hygiene tooling
Support-tier impact: none; this document does not broaden product claims
Policy impact: establishes selection guidance; it does not add exceptions

Adze should standardize around a small upstream substrate and expose it through
repo-owned `xtask` commands. Upstream tools are the engine room; the repo
contract should remain `xtask`, policy ledgers, and checked-in receipts.

```text
Do not make upstream tools the public control surface.
Make xtask the repo surface.
Make upstream tools the engine room.
```

This keeps agent and maintainer workflows stable even when an underlying tool,
install method, or CI topology changes.

## Control-plane rule

| Layer | Owns | Examples |
| --- | --- | --- |
| Repo surface | stable command names, policy interpretation, receipts, summaries | `cargo xtask check-pr`, `cargo xtask policy-report`, `cargo xtask ripr-pr` |
| Policy ledgers | exceptions, owners, review dates, expiry, covered-by links | `policy/*.toml` |
| Upstream tools | fast specialized analysis or execution | `ast-grep`, `cargo-nextest`, `cargo-llvm-cov`, `cargo-deny` |

The repo should prefer stable wrappers even when the wrapper initially delegates
to a small script or a direct tool invocation. CI YAML should call the wrapper
where practical, not encode long-lived policy in job shell blocks.

## Standard upstream substrate

| Plane | Standard upstream tools | Repo-facing role |
| --- | --- | --- |
| Syntax and codemods | `ast-grep`; rust-analyzer crates for Rust-specific authority | find syntactic candidates, generate worklists, help codemods |
| Workspace graph | `cargo_metadata`, `guppy` | package inventory, dependency closure, risk-pack expansion, lane selection |
| Test execution | `cargo-nextest`; `cargo test --doc` | fast PR tests, partitioning, retries, JUnit artifacts, separate doctests |
| Coverage | `cargo-llvm-cov`, Codecov | execution-surface evidence and coverage artifacts |
| Static mutation exposure | `ripr` | PR advisory weak-oracle packet and repair guidance |
| Runtime mutation | `cargo-mutants` | targeted PR backstop, nightly/release mutation evidence |
| Unsafe and UB | `unsafe-review`, Miri | unsafe reviewability card plus targeted concrete UB witness |
| Source exceptions | `cargo-allow` | durable source exception ledger and diff receipts |
| Dependency trust | `cargo-deny`, `cargo-vet`, RustSec/`cargo-audit`, `cargo-auditable` | license/advisory/source policy, mature audits, shipped-binary auditability |
| Public API and release | `cargo-semver-checks`, rustdoc JSON | semver compatibility receipts and custom public-surface inventories |
| Workflow policy | `actionlint`, `zizmor` | Actions correctness and security posture |
| Text and config hygiene | `taplo`, `typos`, Markdown link/style tooling | TOML, spelling, Markdown structure, link health |
| Workspace hygiene | `cargo-udeps` scheduled; `cargo-hakari` only when measured duplicate-build pain exists | dependency cleanup and optional large-workspace build consolidation |
| CI cache | `Swatinem/rust-cache` by default; `sccache` only when cache economics justify it | cheap restore/save policy and optional large-runner acceleration |

## Authority boundaries

### Syntax scanning

`ast-grep` is the default syntax-pattern substrate for fast structural search,
rewrites, non-Rust policy probes, and agent worklists. It should find
candidates, not make final Rust-semantic decisions.

```text
ast-grep finds candidates.
Rust-aware tooling decides authority.
```

For durable Rust selectors, suppression identity, panic-family checks, or API
facts, use Rust-aware inputs such as rust-analyzer syntax crates, Cargo
metadata, `guppy`, rustdoc JSON, or a repo-owned Rust checker.

### Workspace graph

Use `cargo_metadata` for basic workspace, package, target, and manifest
inventory. Use `guppy` when the repo needs graph closure: changed crate to
reverse dependencies, feature graph routing, publish graph checks, CI lane
selection, or workspace package partitioning.

### Tests and doctests

`cargo-nextest` is the preferred serious test runner for PR and risk-pack test
execution. Keep doctests as a separate command because the nextest lane does
not replace `cargo test --doc`.

### Coverage

Coverage is execution-surface evidence only. `cargo-llvm-cov` and Codecov can
show which code ran, but they do not prove parser correctness, mutation
adequacy, UB freedom, release readiness, or public API stability.

### Mutation

`ripr` shifts mutation-style signal left as a static exposure check. It should
produce PR packets and review guidance for Rust behavior changes. It does not
run mutants or report killed/survived outcomes.

`cargo-mutants` remains the runtime backstop for targeted PR cases, nightly
calibration, and release readiness. Full-workspace mutation should not be a
default tax on ordinary PRs, README-only changes, or policy-only changes.

### Unsafe and UB

`unsafe-review` asks whether unsafe changes have reviewable evidence: contract,
guard, test reach, and witness route. It is not a proof of memory safety. Miri
is the targeted/nightly/release concrete UB witness lane for executions that
need it.

A normal unsafe flow should be:

```text
unsafe-review card
→ safety contract / guard / test reach / witness route
→ optional Miri witness
→ cargo-allow evidence link when an exception is needed
```

### Dependency and release trust

`cargo-deny` is the normal dependency policy gate for advisories, licenses,
bans, sources, and duplicate-version pressure. `cargo-vet` is a maturity layer
for public release trains and high-risk dependency graphs, not a requirement for
every small crate on day one. RustSec/`cargo-audit` remains the advisory source
and simple scheduled audit path. `cargo-auditable` belongs to shipped binaries,
not pure libraries.

`cargo-semver-checks` is the default semver gate for public crates and release
preparation. Rustdoc JSON is the substrate for custom public API inventories,
support-tier maps, and release-surface reports.

### Workflow, text, and config hygiene

Use `actionlint` for GitHub Actions correctness and `zizmor` for Actions
security posture. `zizmor` should be advisory until the baseline is understood,
then promoted selectively for release/publish workflows.

Use `taplo` for TOML, `typos` for source/docs spelling, and one Markdown style
and link-checking stack per repo family. Do not overinvest in prose linting
before broken links and obvious Markdown structure are stable.

## Source inventory rule

Source and exception policy should scan tracked files by default:

```bash
git ls-files -z
```

Use broader filesystem walking only when a tool intentionally includes ignored,
generated, or local artifact paths.

## Default wrapper surface

These command names are the desired stable repo-facing surface. Existing
commands may be added, renamed, or shimmed toward this surface incrementally.

```bash
cargo xtask check-pr
cargo xtask fix-pr
cargo xtask pr-summary

cargo xtask allow-check
cargo xtask allow-diff
cargo xtask ripr-pr
cargo xtask unsafe-review-pr

cargo xtask test-pr
cargo xtask test-risk-pack <pack>
cargo xtask test-docs
cargo xtask coverage
cargo xtask mutation-targeted
cargo xtask miri-targeted

cargo xtask check-deps
cargo xtask check-supply-chain
cargo xtask semver-check
cargo xtask check-workflows
cargo xtask check-toml
cargo xtask policy-report
```

The wrapper names are the repo contract. Implementations may call upstream
binaries directly, use pinned release installers, or switch engines without
changing maintainer and agent entry points.

## Install set guidance

Baseline local and CI tool installation can use cargo installs for Rust tools
and pinned binaries or installer actions for external tools where install time
matters.

```bash
cargo install cargo-allow --locked
cargo install ripr --locked
cargo install unsafe-review --locked
cargo install cargo-nextest --locked
cargo install cargo-deny --locked
cargo install cargo-llvm-cov --locked
cargo install cargo-semver-checks --locked
cargo install cargo-mutants --locked
cargo install cargo-audit --locked
cargo install taplo-cli --locked
cargo install typos-cli --locked
```

External binary substrate:

```text
ast-grep
actionlint
zizmor
markdownlint-cli2
lychee or markdown-link-check
```

Nightly-only and scheduled tools:

```bash
cargo +nightly install cargo-udeps --locked
rustup +nightly component add miri
```

Use `taiki-e/install-action` or pinned release binaries in CI where cold install
cost would dominate the lane.

## Non-standards

Do not globally standardize these as default control-plane requirements:

- Semgrep as the main local code scanner. Prefer `ast-grep` plus Rust-aware
  repo tooling; Semgrep may remain an external security layer where useful.
- Nix for every repo. Use it only where environment determinism justifies the
  added surface.
- Docker for default Rust PR CI. Keep containers in service, integration, or
  release lanes unless the product is a container.
- Full mutation on every PR. Use `ripr` by default and route `cargo-mutants` by
  risk.
- Full Miri on every PR. Use `unsafe-review` by default and route Miri by unsafe
  risk, nightly schedules, or release readiness.

## Adoption rule

Adopt upstream engines behind stable wrappers, one semantic lane at a time.
Each wrapper promotion should document:

1. the source-of-truth artifact it implements,
2. the policy ledger or exception receipt it consumes,
3. the proof command and artifact it emits,
4. whether it is blocking, advisory, scheduled, manual, or release-only,
5. rollback if the upstream tool becomes unavailable or too expensive.

Bottom line:

```text
Standardize upstream engines.
Standardize repo-facing wrappers.
Do not standardize every heavyweight lane as default CI.
```

## Upstream references

- `ast-grep`: <https://ast-grep.github.io/>
- `cargo_metadata`: <https://docs.rs/cargo_metadata/latest/cargo_metadata/>
- `cargo-nextest`: <https://nexte.st/>
- `cargo-llvm-cov`: <https://github.com/taiki-e/cargo-llvm-cov>
- `cargo-mutants`: <https://mutants.rs/>
- Miri: <https://github.com/rust-lang/miri>
- `cargo-deny`: <https://embarkstudios.github.io/cargo-deny/>
- `cargo-vet`: <https://mozilla.github.io/cargo-vet/>
- RustSec / `cargo-audit`: <https://rustsec.org/>
- `cargo-semver-checks`: <https://docs.rs/cargo-semver-checks/latest/cargo_semver_checks/>
- Rustdoc JSON helper crate: <https://docs.rs/rustdoc-json/latest/rustdoc_json/>
- `actionlint`: <https://github.com/rhysd/actionlint>
- `taplo`: <https://taplo.tamasfe.dev/>
- `typos`: <https://github.com/crate-ci/typos>
- `cargo-udeps`: <https://github.com/est31/cargo-udeps>
- `Swatinem/rust-cache`: <https://github.com/Swatinem/rust-cache>
