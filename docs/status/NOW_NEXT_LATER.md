# Now / Next / Later

**Last updated:** 2026-05-21
**Status:** **Product Proof required-gate policy promotion complete; release remains unauthorized** — `adze-swarm` remains the operating repo for follow-up non-release proof work, public promotion PR #795 has merged into public `adze`, public status sync PR #797 has synced the latest install-gap receipts, and release/publish blockers in [`PRODUCT_OBJECTIVE_AUDIT.md`](./PRODUCT_OBJECTIVE_AUDIT.md) must stay explicitly proved or bounded before any tag, publish, or release-workflow work. The latest completed non-release lanes are closed out in [`../../plans/product-proof-result-readiness/closeout.md`](../../plans/product-proof-result-readiness/closeout.md), [`../../plans/product-proof-required-gate/implementation-plan.md`](../../plans/product-proof-required-gate/implementation-plan.md), [`../../plans/ci-lane-policy-hygiene/implementation-plan.md`](../../plans/ci-lane-policy-hygiene/implementation-plan.md), [`../../plans/parser-runtime-maintainability/closeout.md`](../../plans/parser-runtime-maintainability/closeout.md), [`../../plans/cli-parse-surface/closeout.md`](../../plans/cli-parse-surface/closeout.md), [`../../plans/cli-static-sexp/closeout.md`](../../plans/cli-static-sexp/closeout.md), and [`../../plans/cli-static-json-dot/closeout.md`](../../plans/cli-static-json-dot/closeout.md). The active manifest is complete with no active, ready, or blocked non-release work items; release/publish remains blocked on explicit authorization.

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

### Parser recovery real-grammar coverage
- [x] Replace the paused query/tooling closeout manifest with a focused
      real-grammar parser recovery lane in `adze-swarm`.
- [x] Broaden generated external-scanner malformed-input recovery coverage
      with focused parse/document diagnostic receipts.
- [x] Add or refresh parser-v4 scanner recovery smoke so malformed input stays
      source-bounded and no-panic.
- [x] Refresh support-tier or product-audit wording only after new proof
      commands exist.

### Query/tooling expansion
- [x] Replace the paused no-active-lane manifest with a focused query/tooling
      expansion lane in `adze-swarm`.
- [x] Refresh query examples and CLI/tooling smoke receipts for the supported
      subset without making full query parity or stable CLI schema claims.
- [x] Add or refresh query gap-matrix receipts for supported behavior and
      explicit known gaps.
- [x] Refresh support-tier or product-audit wording only after new proof
      commands exist.

### Product gap burn-down
- [x] Replace the completed release-promotion active manifest with an active product gap burn-down queue.
- [x] Keep `adze-swarm` as the operating repo and public `adze` as release/public-intake unless a deliberate promotion PR opens.
- [x] Refresh the stable-product receipt from current `adze-swarm/main` and update the product objective audit if the result changed.
- [x] Fix the dangling-else selected-tree gap with generated selected-AST and ambiguity-summary proof.
- [x] Fix generated reduce/reduce preservation and typed extraction with generated conflict-cell, selected-AST, and document ambiguity-summary proof.
- [x] Revisit the public promotion decision now that the named GLR product gaps are fixed; outcome remains proceed conditionally with no public PR opened by default.
- [x] Fix parser-v4 external-scanner emitted-token spans and record the focused dispatch/range plus diagnostic-document canaries.
- [x] Add first parser-generated external-token grammar diagnostic-document proof while keeping full parser-generated external-scanner recovery explicitly future work.
- [x] Expand the generated external-token diagnostic-document proof into a malformed-input matrix in `adze-swarm` PR #316 while keeping external scanners Experimental.
- [x] Extend the generated external-token malformed-input matrix in
      `adze-swarm` PR #343 so `parse()` errors and `parse_document()`
      diagnostics agree on spans and expected-token names for multibyte,
      body, and newline-boundary cases.
- [x] Register the focused external-scanner parser-v4 and generated matrix
      commands in the advisory `ci-product.sh` lane in `adze-swarm` PR #345,
      with Product Proof routing for future edits to that script.
- [x] Supersede public promotion PR #794 with refreshed public promotion PR #795 after `adze-swarm` advanced through the residual product-trust fixes and promotion-receipt refreshes.
- [x] Merge public promotion PR #795 into public `adze` after the required public branch-protection context was corrected to `Rust Small Result` and the legacy `ci-supported` dispatch receipt passed.

### Toolkit excellence campaign
- [x] Consolidate the source-of-truth guardrails into one PR and close duplicate source-of-truth PRs.
- [x] Defer the CX33 backfill routing PR until a CX33 execution smoke exists; keep runner topology separate from product work.
- [x] Close wrong-target public `adze` swarm PRs or direct useful work back to `adze-swarm`.
- [x] Define the product acceptance matrix for first-use, document, diagnostics, GLR ambiguity, Tree-sitter, query, JSON, CLI, WASM, and performance workflows.
- [x] Harden the starter project, downstream fixture, API choice guide, examples, compatibility matrix, and performance receipts.
- [x] Finish the support-tier promotion pass without adding new Stable claims.

### External scanner recovery hardening
- [x] Open a focused active lane for parser-generated external-token recovery proof.
- [x] Expand the generated external-token malformed-input matrix.
- [x] Harden direct parser-v4 external-scanner diagnostic-detail canaries.
- [x] Refresh support-tier and product-audit wording after proof exists.

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
      2026-05-20 from current `adze-swarm/main` at commit `4f3d451c` after
      residual product-trust PRs #318-#322. This refreshes the advisory
      README-stable, clean-room quickstart, downstream fixture, typed AST,
      precedence, serialization, and bounded CLI install-claim proof from the
      current swarm state. It was refreshed again on 2026-05-20 from
      `adze-swarm/main` at commit `5498967c` after release-blocker tracker PRs
      #324-#327, and again at commit `99dd12b0` after the 0.9.0 workspace
      version bump in PR #330. It was refreshed again at commit `45e40f16`
      after PRs #332-#334 bounded root README, book, and live co-release
      dependency snippets, adding
      `co_release_dependency_snippets_stay_release_surface_bounded` to the
      stable product lane. It was refreshed again at commit `72d2faa7` after
      PRs #336-#338 routed claim-boundary docs into Product Proof, clarified
      the query expansion tail, and moved remaining active artifact receipt
      actions to Node 24-compatible artifact action versions. It was refreshed
      again at commit `8d4cc1cd` after PRs #339-#341 refreshed the stable
      receipt status, path-routed this rolling status file, and added the
      Product Proof stable-surface routing canary.
      `Product Proof` is now also path-routed on this status file because it
      carries stable-product receipt and release-boundary wording.
      The path list is guarded by
      `product_proof_workflow_routes_stable_claim_surfaces`.
- [x] Objective-level completion audit exists in
      [`PRODUCT_OBJECTIVE_AUDIT.md`](./PRODUCT_OBJECTIVE_AUDIT.md), including
      the remaining `cargo install adze-cli`, required aggregate
      `Product Proof Result` gate, path-selected stable-product canaries, and
      support-tier limitation gaps.
- [x] Latest release-surface package receipts: `just package-local adze-cli`
      and `just check-publishable` passed on 2026-05-19 from `adze-swarm`;
      `just check-publishable` was refreshed again on 2026-05-20 at commits
      `e965cba2` after residual product-trust PRs #309-#310 and `464a32a9`
      after PR #311, then `just package-local adze-cli` and
      `just check-publishable` were refreshed on 2026-05-20 from
      `adze-swarm/main` at commit `390ab76f`. `just check-publishable` was
      refreshed again on 2026-05-20 from current `adze-swarm/main` at commit
      `fc959ec1` after the stable-product receipt status update.
      Both `just package-local adze-cli` and `just check-publishable` were
      refreshed again from `adze-swarm/main` at commit `99dd12b0` after the
      0.9.0 workspace version bump.
      These are package verification receipts, not crates.io install or publish
      claims.
- [x] Latest crates.io CLI install-boundary receipt:
      `cargo info --registry crates-io adze-cli` was refreshed on 2026-05-20
      and again on 2026-05-21 from `adze-swarm/main` at commit `0df9f420`.
      It reported that `adze-cli` is not present in crates.io. The explicit
      registry flag prevents Cargo from resolving the local workspace package,
      and PRs #319-#320 hardened the post-publish verifier so both metadata
      lookup and `cargo install` use the explicit `crates-io` registry. The
      verifier dry-run was refreshed on 2026-05-20 from `adze-swarm/main` at
      commit `df4be63a` and again at commit `fc959ec1`, printing the fully
      qualified command plan. The crates.io metadata check was also refreshed
      from commit `99dd12b0` after the 0.9.0 workspace version bump and still
      reported that `adze-cli` is not present in crates.io. The active manifest
      is complete with no ready non-release work; the install receipt remains
      release-surface work blocked on explicit authorization.
- [x] Latest root README dependency-boundary receipt:
      `cargo info --registry crates-io adze` reports published `adze` 0.8.0,
      while `cargo info --registry crates-io adze-tool` reports no registry
      package; both were refreshed on 2026-05-21 from `adze-swarm/main` at
      commit `0df9f420`. The root README install block now explicitly says the
      dependency block is a release-surface dependency shape, not a crates.io
      install receipt for every co-release crate.
- [x] Live co-release dependency snippets that name `adze-tool` or
      registry-shaped `cargo add --build adze-tool` commands are guarded by
      `cargo test -p adze-cli co_release_dependency_snippets_stay_release_surface_bounded -- --exact --nocapture`.
      The canary proves claim-boundary wording in live README/FAQ/tutorial/book
      docs; it is not crates.io dependency-resolution or install proof.
      The `Product Proof` workflow is path-routed across the same live
      claim-boundary docs so edits to those files wake the stable product
      canaries without changing branch protection.
- [x] External scanner recovery hardening closed out with no support-tier
      promotion. Future routine non-release work should open a fresh active
      goal in `adze-swarm`; release/publish authorization and post-publish
      crates.io install receipt remain tracked separately in
      [`adze-swarm#325`](https://github.com/EffortlessMetrics/adze-swarm/issues/325).
- [x] CLI parse-surface hardening closed out in PRs #457-#459. Default static
      `adze parse <grammar.rs> <input>` now emits a document-backed selected
      tree through generated `parse_document()`, document-projection modes keep
      schema-envelope/recovery receipts, unsupported non-document static modes
      fail explicitly, and CLI output remains Stabilizing rather than a stable
      CLI/WASM schema claim.
- [x] CLI static S-expression hardening closed out in PRs #461-#463. Static
      `adze parse --output sexp <grammar.rs> <input>` now emits a
      document-backed selected-tree S-expression through generated
      `parse_document()`. `json` and `dot` were intentionally left for a
      separate proof lane that later closed in PRs #464-#466, and CLI output
      remains Stabilizing.
- [x] CLI static JSON/DOT hardening closed out in PRs #464-#466. Static
      `adze parse --output json <grammar.rs> <input>` now emits generated
      document JSON, and static `adze parse --output dot <grammar.rs> <input>`
      renders a document-backed selected-tree Graphviz graph through generated
      `parse_document()`. Dynamic parse output and stable CLI/WASM schemas
      remain outside the Stable product contract.

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
- [x] Add an always-present `Product Proof Result` context so the stable-product lane can be promoted later without missing-check hazards.
- [x] Open the Product Proof required-gate burn-in lane and record the first selected Stable-canary receipt in `adze-swarm` PR #386.
- [x] Burn in `Product Proof Result` with distinct green selected/skipped receipts before any required-check promotion.
- [x] Promote `Product Proof Result` to required after burn-in receipts passed and branch-protection policy was updated deliberately.
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
- Broaden query compatibility only through
  [`ADZE-SPEC-0013`](../specs/ADZE-SPEC-0013-query-compatibility.md):
  field constraints, anchors, and the source-aware predicate subset are already
  documented as covered; remaining work is broader alternation/directive,
  imported-fixture, differential-corpus, and GLR-forest matching proof.
- Open fresh CLI/tooling work only for a material selected gap such as dynamic
  parse output, static `json`/`dot` output, or stable schema promotion.
- Stabilize the LSP generator and related developer tooling for broader use.
