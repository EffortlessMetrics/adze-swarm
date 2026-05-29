# Non-Publish Preflight Receipts

Status: current receipt
Owner: release/product
Updated: 2026-05-29
Linked proposal: ../../docs/proposals/ADZE-PROP-0017-release-candidate-bundle.md
Linked checklist: ../../docs/reference/RELEASE_CANDIDATE_BUNDLE.md
Linked active goal: ../../.adze/goals/active.toml
Release authorization tracker: https://github.com/EffortlessMetrics/adze-swarm/issues/325

## Scope

These receipts were captured from `EffortlessMetrics/adze-swarm` for the
release-candidate bundle lane. They are non-publish evidence only.

They do not authorize:

- public `adze` promotion;
- release tags;
- crate publishing;
- signing workflow changes;
- Cargo-token use;
- real crates.io install verification;
- a `cargo install adze-cli` public claim.

## Candidate Commit

```text
802fc63d6ed2b120a66c77484d671f1c00f3b61e
docs(release): add candidate bundle checklist (#556)
```

## Receipts

### Supported Gate

Command:

```bash
just ci-supported
```

Result:

```text
passed
```

Observed coverage:

- supported formatting completed for runtime, macro, tool, common, ir,
  glr-core, and tablegen;
- supported test/doc/clippy phases completed successfully;
- no local Windows PDB/disk-pressure failure was observed in this run.

### Stable Product Canaries

Command:

```bash
just ci-product-stable
```

Result:

```text
passed
```

Observed stable canaries included:

- README stable proof alignment;
- Product Proof stable surface routing;
- published CLI install claim boundary;
- co-release dependency claim boundary;
- typed extraction exact value and repeated-parse determinism;
- README, Getting Started, and book quickstart clean-room parse/diagnostics;
- checked-in downstream demo test and binary run;
- standalone downstream starter test and binary run;
- operator precedence core shape;
- core parse-table serialization doctests and roundtrip.

### Publishability Check

Command:

```bash
just check-publishable
```

Result:

```text
passed
```

Publish order checked:

1. `adze-common`
2. `adze-ir`
3. `adze-glr-core`
4. `adze-tablegen`
5. `adze-macro`
6. `adze-tool`
7. `adze-cli`
8. `adze`

Each crate passed `cargo package --list` and metadata checks.

### Crates.io Install Verifier Dry Run

Command:

```bash
cargo run -q -p xtask -- verify-crates-io-install adze-cli --bin adze --version X.Y.Z --locked --dry-run
```

Result:

```text
crates.io install receipt plan
status: dry-run
package: adze-cli
binary: adze
version: X.Y.Z
locked: true
commands:
  cargo info --registry crates-io adze-cli
  cargo install --registry crates-io adze-cli --root <temp-root> --version X.Y.Z --locked
  <temp-root>/bin/adze.exe --version

non-claim: dry-run does not contact crates.io or prove registry installation
```

## Claim Boundary

These receipts show current non-publish readiness from `adze-swarm`.

They do not prove `cargo install adze-cli` works from crates.io. That claim
requires an authorized public release/publish path and a real post-publish
install verifier run without `--dry-run`.

Public promotion and release execution remain blocked on #325.
