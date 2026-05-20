# Now / Next / Later

**Last updated:** 2026-05-19
**Status:** **Residual product trust hardening active** — `adze-swarm` is the operating repo, public `adze` remains release/public-intake, and the remaining blockers in [`PRODUCT_OBJECTIVE_AUDIT.md`](./PRODUCT_OBJECTIVE_AUDIT.md) must stay explicitly proved or bounded before any public promotion. Toolkit excellence is complete with closeout recorded in [`../../plans/toolkit-excellence/closeout.md`](../../plans/toolkit-excellence/closeout.md). Release promotion readiness is closed out in [`../../plans/release-promotion/closeout.md`](../../plans/release-promotion/closeout.md), and any public promotion still requires a fresh explicit execution goal using [`../../plans/release-promotion/public-promotion-pr-plan.md`](../../plans/release-promotion/public-promotion-pr-plan.md).

Adze status and rolling execution plan. For recurring pain points, see [`docs/status/FRICTION_LOG.md`](./FRICTION_LOG.md). For API stability guarantees per crate, see [`docs/status/API_STABILITY.md`](./API_STABILITY.md). For support-tier proof commands, see [`docs/status/SUPPORT_TIERS.md`](./SUPPORT_TIERS.md).

---

## Done

### ✅ Baseline landed on `main`
- [x] The supported contract remains `just ci-supported` locally, while `adze-swarm` branch protection requires `Rust Small Result` in GitHub.
- [x] Supported crates compile, format, lint, test, and document cleanly on `main`.
- [x] Feature-matrix coverage no longer carries the prior expected failure in the supported lane.
- [x] PR [#264](https://github.com/EffortlessMetrics/adze/pull/264) merged on 2026-04-03 as commit `2a88deb6e6095682051729290987a78a0565d613`.
- [x] The temporary convergence worktrees/branches used for the PR stack were cleaned up.
- [x] A safety archive of the pre-cleanup dirty checkout was preserved outside `/tmp`.
- [x] Issue #268 worktree cleanup documentation and validation is documented in the developer guide and backed by a helper script.

### ✅ Prior close-out state
- [x] PR `#280` (workflow hardening) merged on 2026-04-06.
- [x] PR `#281` (roadmap/execution-state refresh) merged.
- [x] `main` is aligned with `origin/main` and is now the source of truth for the remaining hardening work.
- [x] A restore audit on 2026-04-11 confirmed that the proof surfaces trimmed during publication are already present again on `main`.

---

## Now

### Product gap burn-down
- [x] Replace the completed release-promotion active manifest with an active product gap burn-down queue.
- [x] Keep `adze-swarm` as the operating repo and public `adze` as release/public-intake unless a deliberate promotion PR opens.
- [x] Refresh the stable-product receipt from current `adze-swarm/main` and update the product objective audit if the result changed.
- [x] Fix the dangling-else selected-tree gap with generated selected-AST and ambiguity-summary proof.
- [x] Fix generated reduce/reduce preservation and typed extraction with generated conflict-cell, selected-AST, and document ambiguity-summary proof.
- [x] Revisit the public promotion decision now that the named GLR product gaps are fixed; outcome remains proceed conditionally with no public PR opened by default.
- [x] Fix parser-v4 external-scanner emitted-token spans and record the focused dispatch/range plus diagnostic-document canaries.
- [x] Add first parser-generated external-token grammar diagnostic-document proof while keeping full parser-generated external-scanner recovery explicitly future work.
- [x] Supersede public promotion PR #794 with refreshed public promotion PR #795 after `adze-swarm` advanced through the residual product-trust fixes and promotion-receipt refreshes.
- [ ] Keep public promotion PR #795 parked for review unless a reviewer approves it; all checks are green and squash auto-merge is enabled, but public promotion has not completed.

### Toolkit excellence campaign
- [x] Consolidate the source-of-truth guardrails into one PR and close duplicate source-of-truth PRs.
- [x] Defer the CX33 backfill routing PR until a CX33 execution smoke exists; keep runner topology separate from product work.
- [x] Close wrong-target public `adze` swarm PRs or direct useful work back to `adze-swarm`.
- [x] Define the product acceptance matrix for first-use, document, diagnostics, GLR ambiguity, Tree-sitter, query, JSON, CLI, WASM, and performance workflows.
- [x] Harden the starter project, downstream fixture, API choice guide, examples, compatibility matrix, and performance receipts.
- [x] Finish the support-tier promotion pass without adding new Stable claims.

### Release promotion readiness
- [x] Open the release-promotion readiness campaign in `adze-swarm`.
- [x] Inventory completed swarm campaigns and release-facing support-tier claims.
- [x] Audit public `adze` drift before preparing a promotion PR.
- [x] Freeze release-facing claims and known limitations.
- [x] Prepare or defer the public promotion PR with proof and rollback.
- [x] Close the release-promotion readiness campaign with an explicit proceed/defer/split decision point.

### Correctness queue baseline
- [x] Refresh live PR state with `gh pr list --state open --limit 20 --json number,title,isDraft,headRefName,updatedAt,url`.
- [x] Confirm the live PR queue is empty after the tablegen/runtime correctness fixes landed.
- [x] Keep the historical queue closed; do not revive stale PR numbers from handoffs unless GitHub shows them open again.
- [ ] For any new correctness PR, keep the one-PR loop: rebase on current `main`, run focused proof, require hosted `Rust Small Result`, and report red checks before merge.

### Product proof alignment
- [x] Keep `just ci-supported` as the fast local supported proof. `adze-swarm`
      PR #157 restored the exact local gate on Windows by routing formatting
      through the portable chunked formatter instead of per-crate `cargo fmt`
      invocations that can exceed command-line limits.
- [x] Convert `scripts/ci-product.sh` from compile-only advisory smoke to bounded behavior canaries where behavior is currently truthful; benchmarks and WASM remain explicit compile/no-run canaries.
- [x] Track GLR product proof in [#460](https://github.com/EffortlessMetrics/adze/issues/460), tablegen ABI completeness in [#461](https://github.com/EffortlessMetrics/adze/issues/461), and parse diagnostics in [#463](https://github.com/EffortlessMetrics/adze/issues/463).
- [x] Close out CLI clean-room quickstart/truthfulness and README/support-tier reconciliation as landed proof work ([#464](https://github.com/EffortlessMetrics/adze/issues/464), [#465](https://github.com/EffortlessMetrics/adze/issues/465)).
- [x] Keep README feature claims aligned with [`SUPPORT_TIERS.md`](./SUPPORT_TIERS.md): no Stable claim without a named proof command; guarded by `readme_stable_claims_are_in_stable_product_lane`.
- [x] Add stable-product canaries for the checked-in downstream demo, README quickstart, and Getting Started tutorial so clean downstream crates prove typed parsing and useful bad-input diagnostics.
- [x] Latest hosted stable-product receipt: GitHub workflow dispatch
      [`Product Proof` run 26104726428](https://github.com/EffortlessMetrics/adze-swarm/actions/runs/26104726428)
      passed on 2026-05-19 from current `adze-swarm/main` after PR #281,
      commit `0b79a36a`. The `ci-product stable canaries` job passed in 3m02s
      and the broad advisory canaries skipped under the stable-only default.
      This is evidence for the README Stable claim lane, not a
      branch-protection change.
- [x] Latest local stable-product receipt: `just ci-product-stable` passed on
      2026-05-20 from current `adze-swarm/main` at commit `e965cba2` after
      residual product-trust PRs #295-#310. This refreshes the advisory
      README-stable, clean-room quickstart, downstream fixture, typed AST,
      precedence, and serialization proof from the current swarm state.
- [x] Objective-level completion audit exists in
      [`PRODUCT_OBJECTIVE_AUDIT.md`](./PRODUCT_OBJECTIVE_AUDIT.md), including
      the remaining `cargo install adze-cli`, branch-protection, and public
      promotion gaps.
- [x] Latest release-surface package receipts: `just package-local adze-cli`
      and `just check-publishable` passed on 2026-05-19 from `adze-swarm`;
      `just check-publishable` was refreshed again on 2026-05-20 at commit
      `e965cba2` after residual product-trust PRs #309-#310.
      These are package verification receipts, not crates.io install or publish
      claims.

### Operational tail
- [x] [Issue #269](https://github.com/EffortlessMetrics/adze/issues/269): pure-rust benchmark-compilation tail is removed from routine PRs; benchmark compile/performance signal remains in explicit performance and benchmark lanes.
- [x] `adze-swarm` PR #284 bounded broad Rust tail jobs, and PR #285 scoped the
      default pure-rust PR test step to supported crates while keeping full
      workspace tests explicit through manual/full-ci. The follow-up PR #285
      checks showed `Rust Small Result`, `Supported Rust Gate`, and `Test Pure
      Rust Implementation` all passing.
- [x] [Issue #268](https://github.com/EffortlessMetrics/adze/issues/268): Worktree cleanup script exists (`scripts/cleanup-worktrees.sh`), `just` exposes list/prune helpers, and contributor guidance documents linked-worktree vs standalone-clone cleanup.
- [x] The prior rustdoc-only `Documentation` lane failure note was stale; the latest completed `Documentation` job observed on `main` succeeded on 2026-05-11.

---

## Next

### Behavior-proof product lane
- [x] Add an advisory stable product lane for README-stable claims.
- [ ] Promote `ci-product-stable` to required only after advisory behavior smokes pass consistently.
- [ ] Keep broad workspace, fuzzing, Miri, sanitizers, browser WASM, grammar corpus, runtime2, and benchmarks scheduled/manual unless explicitly promoted.

---

## Later

### ⚡ Performance optimization
- Arena allocator for parse forest nodes.
- Incremental parsing improvements beyond conservative fallback.
- Benchmark suite with clearer regression detection and less CI noise.

### 🌳 Incremental parsing
- Move from conservative fallback toward active forest-splicing for editor-scale workflows.
- Revisit the currently deferred incremental path once the surrounding runtime contracts are steadier.

### 🔍 Query and tooling expansion
- Implement remaining Tree-sitter query predicates and cookbook coverage.
- Continue CLI/tooling polish now that the basic command surface exists.
- Stabilize the LSP generator and related developer tooling for broader use.
