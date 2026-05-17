# Adze Friction Log

**Last updated:** 2026-05-17

If it happens twice, it's not "user error". It's friction we own until we remove it or document it well enough that it stops recurring.

---

## Active Friction

| ID | Area | Symptom | Impact | Status | Link |
|---:|------|---------|--------|--------|------|
| FR-001 | Docs | Docs drift from dev head (README/book/guides disagree) | Users follow dead paths | Resolved (Wave 16, 2026-03-28) | (issue) |
| FR-002 | CI | Too many workflows fail/cancel simultaneously on PRs | Signal is noisy | Mitigated | (issue) |
| FR-003 | Dev loop | Supported gate is still heavy on constrained machines | Local iteration cost | Mitigated | (issue) |
| FR-004 | Status | Supported-lane exclusions aren't obvious | Confusing contributor loop | Mitigated | (issue) |
| FR-005 | Macro | Leaf `transform` closures are captured but never executed | Type conversions (e.g. string to i32) fail silently | Resolved | [Issue #74](https://github.com/EffortlessMetrics/adze/issues/74) |
| FR-006 | Macro | `Extract` trait signature mismatch in `pure-rust` mode | Compilation errors (E0053, E0308) in user code | Resolved | - |
| FR-007 | Runtime | Lexer state pointer layout mismatch in `pure-rust` mode | Runtime `UnexpectedToken("end")` errors | Resolved | - |
| FR-008 | Tooling | `just` has permission issues on some systems | Commands fail with `/run/user/1000/just` errors | Mitigated | - |
| FR-009 | Dev loop | Workspace build is very slow (10+ min for full check) | Developers avoid full validation locally | Open | - |
| FR-010 | Runtime | `runtime/src/pure_parser.rs` has parse errors | Blocks `cargo fmt` on entire workspace | Resolved | - |
| FR-011 | Docs | `rustdoc::private_intra_doc_links` warning in runtime | Cosmetic noise in doc build | Resolved | - |
| FR-012 | Publishing | No `cargo package` dry-run in CI | Broken publishes not caught early | Resolved | - |
| FR-013 | Tooling | No CLI binary yet (`adze check`, `adze stats`) | Grammar validation requires writing Rust | Resolved | - |
| FR-014 | Runtime | Some `adze` runtime integration tests fail to compile | Stale API references in test files (Node, etc) | Resolved | - |
| FR-015 | Testing | Feature matrix expected failure (`feature_profile_resolve_backend`) | 11/12 pass, 1 expected failure | Resolved | - |
| FR-016 | Testing | Compiler ICE in feature policy contract tests | Blocks test compilation under specific macro/control-flow combinations | Resolved | - |
| FR-017 | Testing | Backend-selection expectations drift across feature-unified test surfaces | Head-specific CI red and ad hoc panic matching | Resolved | [Issue #267](https://github.com/EffortlessMetrics/adze/issues/267) |
| FR-018 | CI | Pure-rust benchmark compilation tail in PRs | Routine PRs no longer block on low-signal benchmark compilation | Resolved for routine PRs | [Issue #269](https://github.com/EffortlessMetrics/adze/issues/269) |
| FR-019 | Tooling | Temp worktree cleanup can drift when a `/tmp` path becomes a standalone repo | Cleanup requires manual removal and prune steps | Resolved | [Issue #268](https://github.com/EffortlessMetrics/adze/issues/268) |
| FR-020 | CI | `just ci-supported` formatting can hit Windows command-line length limits | Blocks the local supported proof on Windows | Resolved | `adze-swarm#157` |

---

## Detailed Entries

### FR-006 - Extract Trait Signature Mismatch

**Area:** macro
**Symptom:** Users enabling the `pure-rust` feature encounter compilation errors like `method extract has an incompatible type for trait`.
**Expected:** The macro automatically generates the correct signature based on enabled features.
**Actual:** The macro was emitting `Option<Node>` instead of `Option<&ParsedNode>` because it wasn't correctly detecting the target crate's features.
**Fix:** Updated `macro/src/expansion.rs` to use `cfg!(feature = "pure-rust")` at macro-expansion time to choose the correct tokens.
**Status:** Resolved

### FR-007 - Lexer State Pointer Mismatch

**Area:** runtime
**Symptom:** Parsers built with `ADZE_USE_PURE_RUST=1` fail at runtime with `UnexpectedToken("end")` even for valid input.
**Expected:** The generated lexer correctly tokenizes the input.
**Actual:** The `adze-tool` was generating a lexer that cast the state pointer to a custom `LexerState` struct that didn't match the `TsLexer` struct passed by the runtime.
**Fix:** Updated `tablegen/src/lexer_gen.rs` to generate a lexer that uses the standard `TsLexer` ABI (function pointers for lookahead/advance).
**Status:** Resolved

### FR-001 - Documentation Drift

**Area:** docs
**Symptom:** README.md and book examples refer to old `rust-sitter` naming or outdated macro syntax.
**Expected:** Documentation matches the current `adze` release state.
**Actual:** Users encounter compilation errors when copying examples.
**Fix:** Repository-wide documentation audit and sync completed in two phases:
- **Phase 1:** Updated version strings and critical references (2 files)
- **Phase 2:** Fixed feature flag consistency across book/ directory (8 files, 17 changes)
  - `glr-core` → `glr`
  - `incremental` → `incremental_glr`
  - Removed outdated feature references
- **Phase 3 (Wave 16, 2026-03-28):** Final documentation sync completed:
  - Updated version strings from v0.5.0-beta to the 0.8 release line
  - Fixed feature flags from `["glr-core", "incremental"]` to `["glr", "incremental_glr"]`
  - Updated crate name references from `adze-runtime` to `adze`
  - Updated API usage examples
  - All documentation now aligned with completed PR #2 (Feature Flag Standardization)
**Status:** Resolved (Wave 16, 2026-03-28)

### FR-002 - CI Workflow Noise

**Area:** ci
**Symptom:** PRs trigger dozens of overlapping workflows (benchmarks, tests, lints) that often conflict or cancel each other.
**Expected:** Clear, non-redundant signal on PR status.
**Actual:** Hard to tell if a failure is real or a CI glitch.
**Fix:** Added concurrency groups (`cancel-in-progress`) and feature matrix job. Lint/test jobs gated by event type to reduce noise.
**Status:** Mitigated

### FR-003 - Heavy Local Dev Loop

**Area:** tooling
**Symptom:** Running the full test suite (`xtask test`) requires significant memory and CPU, causing OOMs on 16GB machines.
**Expected:** Developers can iterate on features without crashing their environment.
**Actual:** CI is often the only place to run full validation.
**Fix:** Optimized `pure-rust` builder and split tests into smaller bundles.
**Status:** Mitigated

### FR-004 - Undocumented Support Lanes

**Area:** publishing
**Symptom:** Some crates are excluded from the default workspace build (via `exclude` in Cargo.toml) but the reason isn't documented.
**Expected:** Contributors know which crates require special toolchains (Node.js, C compilers).
**Actual:** Confusion when `cargo build --workspace` skips important crates.
**Fix:** [`DEVELOPER_GUIDE.md`](../DEVELOPER_GUIDE.md) added. [`KNOWN_RED.md`](./KNOWN_RED.md) documents exclusions. READMEs added to `crates/` microcrates.
**Status:** Mitigated

### FR-005 - Transform Closure Capture Bug

**Area:** macro
**Symptom:** Using `#[adze::leaf(transform = ...)]` has no effect; the raw string is always returned.
**Expected:** The closure is executed during the `extract` phase to convert the value.
**Actual:** `adze-macro` generates code that captures the closure but never calls it.
**Repro:** Define a leaf with `transform = |s| s.len()`, observe it still returns the string.
**Fix:** Update `macro/src/expansion.rs` to generate call sites for captured closures.
**Status:** Resolved
**Resolution:** Code analysis confirms the closure IS being called. The `WithLeaf<L>::extract()` implementation in `runtime/src/lib.rs` correctly invokes the closure when provided. Macro expansion properly passes the closure to `extract()`. Snapshot tests verify correct code generation. The original issue may have been fixed in a prior commit or was a misunderstanding of the code flow.
**Links:** [Issue #74](https://github.com/EffortlessMetrics/adze/issues/74)

### FR-008 - `just` Permission Issues

**Area:** tooling
**Symptom:** Running `just` commands fails with permission errors related to `/run/user/1000/just` on some Linux systems.
**Expected:** `just` recipes execute without filesystem permission issues.
**Actual:** Users see permission denied errors; workaround is to set `JUST_TMPDIR` or use `cargo` directly.
**Fix:** Workaround documented. `just` runtime dir permission fix applied. `cargo` commands work as primary alternative.
**Status:** Mitigated

### FR-009 - Slow Workspace Build

**Area:** dev loop
**Symptom:** `cargo check --workspace` or `cargo build` historically took 10+
minutes on standard hardware when the workspace included 47 governance/support
microcrates plus the full core pipeline.
**Expected:** Developers can iterate quickly on individual crates.
**Actual:** The 0.9 microcrate-to-SRP collapse reduced the workspace to 29
packages, but full workspace builds can still be heavier than focused
iteration because grammar/tooling/golden surfaces remain outside the core
supported lane.
**Fix:** Use per-crate `cargo check -p <crate>` and `just ci-supported` for
routine iteration. Keep the package-boundary release gate green so temporary
microcrates do not return before release.
**Status:** Mitigated

### FR-010 - `pure_parser.rs` Parse Errors

**Area:** runtime
**Symptom:** `runtime/src/pure_parser.rs` contained Rust parse errors that prevented `cargo fmt` from formatting the file.
**Expected:** All `.rs` files parse cleanly.
**Actual:** The file had syntax-level issues blocking formatting and compilation.
**Fix:** All 20 compile errors in the runtime crate resolved. `cargo fmt` and `cargo check` now pass.
**Status:** Resolved

### FR-011 - Rustdoc Private Intra-Doc Links Warning

**Area:** docs
**Symptom:** `cargo doc -p adze` emits a `rustdoc::private_intra_doc_links` warning.
**Expected:** Clean doc build with no warnings.
**Actual:** One warning about private intra-doc links in the runtime crate.
**Fix:** Doc links updated to reference public items only.
**Status:** Resolved (Wave 6)

### FR-012 - No `cargo package` Dry-Run in CI

**Area:** publishing
**Symptom:** Publishing errors (missing files, bad metadata) are only discovered at `cargo publish` time.
**Expected:** CI catches packaging issues before merge.
**Actual:** No `cargo package` step in the CI pipeline.
**Fix:** Added `package-validation` job to `.github/workflows/ci.yml` that runs `cargo package --no-verify` for all publishable crates in the core release surface. Also updated `scripts/release-crates.txt` to remove non-publishable crates (`adze-ir`, `adze-glr-core`, `adze-tablegen` have `publish = false`).
**Status:** Resolved

### FR-013 - No CLI Binary

**Area:** tooling
**Symptom:** To validate a grammar, users must write a full Rust program with `build.rs` integration.
**Expected:** A CLI command like `adze check grammar.rs` validates grammars without a full project.
**Actual:** `adze check`, `adze stats`, `adze init`, `adze build`, `adze test`, and `adze doc` exist. `adze parse` is present as a command shape, but static and dynamic parse output currently fail explicitly as unimplemented.
**Fix:** Implement `adze check` and `adze stats` subcommands; keep parse-mode documentation and errors explicit until parse output is behavior-backed.
**Status:** Resolved
**Discovered:** Wave 14
**Resolved:** Wave 15 (2026-03-25) - CLI validation and project scaffolding exist in `cli/`. Parse output remains a developing surface, tracked separately under CLI truthfulness/product-proof work.

### FR-014 - Stale Runtime Test API References

**Area:** runtime
**Symptom:** Several `adze` runtime integration test files fail to compile with `use of undeclared type Node` and similar errors.
**Expected:** All test files compile and run.
**Actual:** Tests like `lexer_tests`, `simd_lexer_test`, `test_glr_integration`, `test_abi_contract`, `error_recovery_tests` reference APIs (`Node`, etc.) that were removed or renamed during the pure-Rust runtime refactor.
**Fix:** Update test files to use current API surface or remove tests that duplicate coverage already in the supported lane.
**Status:** Resolved
**Discovered:** Wave 14
**Resolved:** Wave 15 (2026-03-16) - Verified with `cargo build -p adze --tests` and `cargo test -p adze --no-run` - all tests compile successfully.

### FR-015 - Feature Matrix Expected Failure

**Area:** testing
**Symptom:** A historical feature-policy facade test panicked with "Grammar has shift/reduce or reduce/reduce conflicts, but the GLR feature is not enabled."
**Expected:** Feature matrix: 12/12 pass.
**Actual:** 11/12 pass; 1 expected failure due to intentional GLR feature gating logic being tested without the GLR feature enabled.
**Fix:** Added conditional guard `if profile.has_glr()` in the test to only call `resolve_backend(true)` when GLR is available, avoiding the panic in pure-rust-without-GLR configuration.
**Status:** Resolved
**Discovered:** Wave 14
**Resolved:** Wave 15 (2026-03-25) - Test passed with all feature combinations before the facade was later retired during the 0.9 microcrate transition.

### FR-016 - Compiler ICE in Feature Policy Contract

**Area:** testing
**Symptom:** Compiler internal compiler error (ICE) when running tests in a historical feature-policy facade with `proptest!` macro and complex control flow.
**Expected:** All tests compile and run without compiler errors.
**Actual:** ICE triggered by combination of `proptest!` macro, `const fn` with `unreachable!()`, and nested `if` statements.
**Fix:**
- Replaced `proptest!` macro with regular test functions
- Made `ParserFeatureProfile::resolve_backend()` a `const fn`
- Replaced nested `if` statements with `match` for better compile-time evaluation
- Replaced `unreachable!()` with `panic!()` to avoid ICE in const contexts
- Made `ParserBackend::select()` a `const fn` to fix const fn compilation errors
**Status:** Resolved (Wave 16, 2026-03-28)

### FR-017 - Backend-Selection Contract Drift Across Feature Surfaces

**Area:** testing
**Symptom:** Backend-selection tests in parser/governance microcrates fail differently depending on the effective feature-unified surface of the crate under test.
**Expected:** Conflict-backend assertions should follow one repository-level contract regardless of which representative crate is proving it.
**Actual:** Some tests assume `profile.has_glr()` and actual backend selection always agree, while other lanes can legitimately surface either a selected backend or the expected no-GLR panic.
**Repro:** Recent head-specific failures on PR #264 in the retired parser-backend contract tests, `crates/parser-feature-contract/tests/bdd_parser.rs`, and `crates/runtime-governance/tests/integration_chain.rs`.
**Fix:** Define one authoritative contract for conflict-backend behavior and centralize the assertion/helper used across representative test crates.
**Status:** Resolved (Wave 17, 2026-04-05)
**Links:** [Issue #267](https://github.com/EffortlessMetrics/adze/issues/267)

### FR-018 - Windows Pure-Rust Benchmark Tail

**Area:** ci
**Symptom:** Windows pure-rust CI lanes spend a long time in benchmark-compilation after the meaningful test/build signal has already completed.
**Expected:** Required merge-blocking jobs should either provide clear signal or finish quickly once build/test are done.
**Actual:** The long pole on the final green path for PR #264 was the `Run benchmarks (check compilation)` step in `.github/workflows/pure-rust-ci.yml` on `windows-latest`. Later PRs still spent about 40 minutes in the pure-rust job because the default Ubuntu/stable PR lane ran `cargo bench --no-run` across all crates after the meaningful formatting, clippy, build, and test signal had completed.
**Repro:** PR #264 merged green only after the `Test Pure Rust Implementation (windows-latest, stable)` and `(..., nightly)` jobs eventually completed their final `cargo bench --no-run` step. PRs #631/#632 showed the same low-signal tail shape on the default Ubuntu/stable pull-request lane.
**Fix:** OS-segmented benchmark compile checks added in `.github/workflows/pure-rust-ci.yml` (PR #276). Windows PR benchmark compile checks were later skipped. Routine pull requests now also skip the all-crate pure-rust benchmark compile check; benchmark compile/performance ownership lives in `performance.yml`, `benchmarks.yml`, and explicit `ci:perf`/`benchmarks`/`full-ci` opt-in lanes. Push and workflow-dispatch pure-rust runs still keep the all-crate compile check.
**Status:** Resolved for routine PRs
**Links:** [Issue #269](https://github.com/EffortlessMetrics/adze/issues/269), [PR #276](https://github.com/EffortlessMetrics/adze/pull/276), [PR #280](https://github.com/EffortlessMetrics/adze/pull/280)

### FR-019 - Worktree Metadata Drift During Cleanup

**Area:** tooling
**Symptom:** Temporary PR worktrees can no longer be removed with `git worktree remove` because the temp path has drifted into standalone-repo form while the main checkout still carries stale worktree metadata.
**Expected:** Temporary worktree cleanup should be predictable and safe after PR closeout.
**Actual:** During PR #264 cleanup, `/tmp/adze-local-improvements` had a real `.git/` directory, `git worktree remove` failed validation, and cleanup required manual deletion plus `git worktree prune`.
**Repro:** `fatal: validation failed, cannot remove working tree: '/tmp/adze-local-improvements/.git' is not a .git file, error code 2`
**Fix:** Added `scripts/cleanup-worktrees.sh` with classification, safe cleanup, stale listing, and stale metadata pruning helpers. `just worktree-list` and `just worktree-prune-stale` expose the common commands, and [`DEVELOPER_GUIDE.md`](../DEVELOPER_GUIDE.md) documents the closeout workflow for linked worktrees versus standalone clones.
**Status:** Resolved
**Links:** [Issue #268](https://github.com/EffortlessMetrics/adze/issues/268), `scripts/cleanup-worktrees.sh`

### FR-020 - Windows Supported-Gate Formatter Length

**Area:** ci
**Symptom:** Running `just ci-supported` on Windows failed during formatting
with `The filename or extension is too long. (os error 206)`.
**Expected:** The local supported proof should run on the same Windows checkout
used for swarm work.
**Actual:** Per-crate `cargo fmt`/Cargo-driven rustfmt invocation could exceed
the Windows command-line limit before reaching clippy and tests.
**Repro:** `just ci-supported` from `C:\Code\Rust2\adze-swarm` on 2026-05-17.
**Fix:** `just ci-supported` now invokes `scripts/ci-supported.sh` directly,
and the supported lane formats normal crate source roots through
`scripts/fmt-workspace.sh`, which chunks direct `rustfmt` calls.
**Status:** Resolved
**Links:** `adze-swarm#157`

---

## Entry Template

### FR-XXX - <short title>

**Area:** docs / ci / tooling / runtime / publishing
**Symptom:** what the user experiences
**Expected:** what they thought would happen
**Actual:** what happened
**Repro:** exact commands + environment
**Fix:** what removes this friction
**Status:** Open / Mitigated / Resolved
**Links:** issue, PR, related docs
