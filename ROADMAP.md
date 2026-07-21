# Adze Roadmap

**Current Version:** 0.8.0
**MSRV:** 1.95 (Rust 2024 edition)

Adze (formerly `rust-sitter`) is a Rust-native grammar toolchain that turns Rust type definitions into high-performance GLR parse machinery.

---

## ✅ Milestone 0.6.0: Core Stability (Completed)
- **Pure-Rust Runtime**: Initial zero-dependency parsing engine.
- **Precedence & Associativity**: Basic support for operator binding.
- **Tree-sitter Parity**: Core grammar features parity.

## ✅ Milestone 0.7.0: GLR & Ambiguity (Completed)
- **GLR Engine**: Generalized LR parsing for inherently ambiguous grammars (C++, JS).
- **Conflict Handling**: Automatic stack forking and merging (SPPF).
- **External Scanners**: Support for custom lexing (e.g. Python indentation).

## ✅ Milestone 0.8.0: The Publishable Baseline (Complete)
- ✅ **Supported CI Contract Green**: `just ci-supported` / `CI / ci-supported` is green on `main`. Full workspace status docs and supported-lane boundaries are documented in `docs/status/`.
- ✅ **Safety Audit**: SAFETY comments on all `unsafe` blocks in supported crates.
- ✅ **Testing Buildout**: 2,460+ tests — property, integration, snapshot, GLR-core, fuzzing, mutation guards, ABI matrix. Supported feature matrix is green. Mutation testing is configured.
- ✅ **Example Grammars**: 10 example grammars (arithmetic, optionals, repetitions, words, boolean, json, csv, lambda, regex, ini).
- ✅ **API Documentation**: Crate-level doc comments; `cargo doc` builds with 0 warnings. Book: 6+ chapters. Architecture chapter added.
- ✅ **WASM Compatibility**: All core crates verified for `wasm32-unknown-unknown`. WASM CI verification job.
- ✅ **Security Audit**: `cargo-audit` clean — 0 known vulnerabilities.
- ✅ **Error Message Quality**: Actionable diagnostics across parser, IR, and tablegen. Error display formatting tests.
- ✅ **Fuzzing Targets**: 22 fuzz targets covering parser, lexer, external scanners, stack pool, and concurrency.
- ✅ **CI Feature Matrix**: Crate × feature-flag test combinations with concurrency caps. Cross-platform advisory jobs for macOS/Windows.
- ✅ **Cargo.toml Metadata**: Publish-ready metadata across workspace. Publish order documented in `policy/release-graph.toml` (`just check-release-graph`, `just check-publishable`).
- ✅ **Workspace Structure**: governance/support crates under `crates/`, benchmarks, fuzzing, golden-tests, and book scaffolding.
- ✅ **Table Compression**: Optimized parse tables using Tree-sitter format (>10x reduction).
- ✅ **Cross-Platform**: Linux verified, macOS/Windows CI advisory jobs in place.
- ✅ **Parallel Agent Work**: 14 waves of parallel agent work, 85+ commits driving the 0.8.0 release.
- ✅ **Backlog Convergence**: Final live branch [#264](https://github.com/EffortlessMetrics/adze/pull/264) merged into `main` on 2026-04-03.
- ✅ **Workflow Hardening**: PR [#280](https://github.com/EffortlessMetrics/adze/pull/280) merged on 2026-04-06 with CI lane hardening and backend-contract stabilization. Backend-selection contract ([issue #267](https://github.com/EffortlessMetrics/adze/issues/267)) resolved.
- ✅ **Core Crates Publishable**: PR [#275](https://github.com/EffortlessMetrics/adze/pull/275) made core crates publishable with correct metadata.
- ✅ **Crates.io Release Landed**: `adze` 0.8.0 and `adze-tool` 0.8.0 are published on crates.io as of 2026-04-08.
- ✅ **Windows benchmark tail**: [Issue #269](https://github.com/EffortlessMetrics/adze/issues/269) routine PR benchmark compile tails are gated out of the required path; benchmark/performance signal lives in explicit opt-in lanes.
- ✅ **Worktree cleanup docs**: [Issue #268](https://github.com/EffortlessMetrics/adze/issues/268) has helper-backed contributor guidance for linked worktrees, standalone clones, and stale metadata pruning.

## 🚀 Milestone 0.9.0: Ecosystem & Tooling (Current)

### Completed prerequisite: microcrate-to-SRP collapse

The release-blocking microcrate-to-SRP transition is complete. The post-collapse
workspace has 28 packages and zero `owner-module-migration-target` entries in
`policy/package-boundary.toml`.

This remains a release gate, not just a historical cleanup. Before 0.9.0 ships,
the release candidate must keep both package-boundary commands green:

```bash
cargo run -q -p xtask -- check-package-boundary
cargo run -q -p xtask -- check-package-boundary --release-gate
```

The release surface should stay tight: implementation-only support code belongs
in SRP submodules under the crate or xtask tooling that actually owns it. A
standalone crate remains acceptable only when it is a published public surface,
a durable published support surface recorded by ADR, or genuine dev-only
tooling.

The exact ledger-published release graph lives in `policy/release-graph.toml`
(twelve crates as of #855). Regenerate with `cargo xtask generate-release-graph`
and verify with `cargo xtask check-release-graph`. Do not maintain competing
crate lists in docs or scripts.

| Crate | Role | Current status | Target review |
|-------|------|----------------|---------------|
| `adze` (runtime) | parsing, typed extraction, documents, diagnostics, ts-compat | publishable supported core | keep public |
| `adze-macro` | proc-macro attributes | publishable supported core | keep public |
| `adze-tool` | build-time code generation | publishable supported core | keep public/tooling |
| `adze-cli` | CLI scaffolding and parse projections | publishable product shell | keep public |
| `adze-common` | shared grammar expansion (consumed by macro + tool) | publishable supported core | evaluate internal seam after consumer audit |
| `adze-ir` | grammar IR (consumed by glr-core + tablegen) | publishable supported core | evaluate internal seam after tablegen/glr-core boundary review |
| `adze-glr-core` | GLR parser generation | publishable supported core | evaluate internal seam after public API/release impact review |
| `adze-tablegen` | table compression, FFI generation | publishable supported core | evaluate internal seam after codegen/release impact review |
| durable support crates | see `ADZE-ADR-0005` | publishable support surface | keep public while core depends on them |

The collapse record lives in `plans/0.9.0/microcrate-collapse.md`. The durable
architecture rule lives in
`docs/adr/ADZE-ADR-0002-no-durable-unpublished-production-crates.md`: there is
no release-state category for unpublished production microcrates.

### Release blockers

- [x] Microcrate collapse/audit complete with Cargo metadata, CI routing, and support docs updated
- [x] MSRV bump to 1.95 (toolchain, Cargo.toml, CI, docs, xtask doctor)
- [ ] Clippy planned-lint activation (6 lints gated on 1.94/1.95)
- [x] Non-Rust file allowlist reconciled against new structure
- [x] CI economics update (LEM estimates reflect new workspace shape)
- [x] Product-proof refresh maps stable README claims to current proof commands

### Other 0.9.0 work
- **Post-release hardening**: Finish narrowing workflow-only red and restore any proof surfaces trimmed only for publication.
- **Close remaining operational issues**: Keep routine PR gate tails bounded; move any further worktree or benchmark-policy cleanup into focused follow-up issues instead of the old post-#264 queue.
- **CI Hardening Beyond the Supported Gate**: Reduce advisory-lane churn and make broader workflow behavior easier to interpret.
- **CLI Polish**: Improve the already-landed CLI surface (`adze check`, `adze stats`, etc.).
- **Performance Optimization**: Arena allocator for parse forest nodes; benchmark suite with regression detection.
- **Incremental Parsing**: Stabilize forest-splicing for real-time editor performance.
- **Query Predicates**: Full compatibility with Tree-sitter `.scm` query files.
- **LSP Refinement**: Move LSP generator from prototype to "useful for production".
- **More Book Content**: End-to-end tutorials, attribute reference, migration guide from Tree-sitter.

## 🎯 Milestone 1.0.0: The Stability Contract
- **API Freeze**: Stable public API surface for `adze` and `adze-macro`.
- **Performance Baseline**: Documented benchmarks and complexity envelopes.
- **Multi-platform Stability**: Tier 1 support for Linux, macOS, Windows, and WASM.

---

## Non-Goals
- **Replacing Tree-sitter**: Adze aims for interoperability, not total replacement of the ecosystem.
- **Universal Grammar Support**: Focus is on a repeatable, safe pipeline for Rust developers.
