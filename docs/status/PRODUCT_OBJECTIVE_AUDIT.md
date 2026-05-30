# Product Objective Audit

**Last updated:** 2026-05-22
**Status:** incomplete; use this as an audit checklist, not as a support-tier
promotion. Routine product-proof, query/tooling, recovery, user-experience,
parser/runtime maintainability, CLI parse-surface, static S-expression, static
JSON/DOT, and dynamic parse boundary lanes are closed out. The current
`active.toml` records the CLI dynamic parse boundary lane as complete with no
active, ready, or blocked non-release work items. Release/publish work remains
separate and blocked on explicit authorization.
**Source of truth:** [`SUPPORT_TIERS.md`](./SUPPORT_TIERS.md) remains the
authoritative support-tier ledger.

This audit maps the current product objective to concrete repo evidence. It is
intentionally stricter than "the manifest is complete" or "tests passed": every
claim below needs a file, command, CI receipt, or named gap.

## Objective Restated As Deliverables

Adze should be release-readable as a Rust parser generator where:

1. Rust types define the grammar and generated parsers return typed ASTs
   directly.
2. The quickstart works in a clean downstream crate without repo archaeology.
3. The core pure-Rust pipeline is green, bounded, and boring.
4. Tablegen emits valid tables.
5. GLR handles real conflicts honestly.
6. Typed extraction is deterministic.
7. Parse errors are useful instead of incidental.
8. Every Stable README claim maps to concrete proof.
9. Experimental or developing surfaces remain labeled unless promoted with
   receipts.
10. The advertised product works under ordinary user pressure and fails clearly.

## Prompt-To-Artifact Checklist

| Requirement | Evidence inspected | Current result | Gap / next action |
| --- | --- | --- | --- |
| Rust types define grammar and generated parsers return typed ASTs. | README core example; `SUPPORT_TIERS.md` Stable `Typed extraction` row; `PRODUCT_PROOF_MAP.md` Stable typed extraction claim. | Covered for the supported generated-parser contract. | Keep Stable claim limited to proof rows; do not broaden to every grammar shape. |
| Quickstart works in a clean downstream crate. | `testing/downstream-starter/`; `docs/product/ACCEPTANCE_MATRIX.md`; `SUPPORT_TIERS.md` Pure-Rust parser row; user-experience hardening closeout. | Covered for path-dependency downstream wiring, generated starter shape, local `adze-cli` package verification, and the post-closeout starter README/local dependency polish from PR #352. | Published `cargo install adze-cli` is not proven until `adze-cli` is published and an install receipt exists. |
| Core pure-Rust pipeline is green, bounded, and boring. | `just ci-supported`; `Rust Small Result`; `Product Proof Result`; `KNOWN_RED.md` supported-lane description; `adze-swarm` PRs #284, #285, #329, #354, #381, #383, Product Proof required-gate burn-in receipts #386-#391, and parser/runtime maintainability closeout #455. | Covered as the required swarm gate plus supported proof. PR #284 bounds the broad Rust tail, PR #285 scopes the default pure-rust PR test step to supported crates while keeping full workspace tests explicit through manual/full-ci, PRs #329/#354/#381 reduce Windows supported-gate friction, PR #383 adds the always-present Product Proof result context, the burn-in receipts prove selected/skipped Stable-canary paths, and #455 records the post-maintainability closeout with `Supported Rust Gate` green. | Keep broad feature matrices and advisory product canaries outside branch protection unless separately promoted. |
| Tablegen emits valid tables. | `SUPPORT_TIERS.md` Tablegen `TSLanguage` ABI row; `PRODUCT_PROOF_MAP.md` tablegen ABI claim; parser/runtime maintainability PRs #444-#446 and #451-#454. | Stabilizing with compressed decode, field metadata, aliases, externals, lex modes, conflict-cell proof, and recent guardrails for invalid names, null field names, zero-symbol metadata, exact field-name counts, generated ABI name arrays, generated API reads, and exact-array validation fixtures. | Broader generated-language roundtrip and full Tree-sitter parity remain future work. |
| GLR handles real conflicts honestly. | `SUPPORT_TIERS.md` GLR conflict routing row; `docs/product/ACCEPTANCE_MATRIX.md` GLR ambiguity row. | Stabilizing with generated shift/reduce conflict preservation, generated reduce/reduce preservation and selected typed-AST extraction, dangling-else nearest-else selected typed AST proof, retained alternatives, deterministic selected output, ambiguity summaries, and no-panic bad-input guardrails. | Broader conflict-class coverage and any Stable GLR promotion still require support-tier proof review. |
| Typed extraction is deterministic. | `typed_ast_contract_left_associative_addition`; `typed_ast_contract_repeated_parse_is_deterministic`; `readme_arithmetic_quickstart_builds_and_runs`. | Covered for Stable typed extraction rows. | Keep determinism claims scoped to supported generated-parser shapes. |
| Parse errors are useful instead of incidental. | `SUPPORT_TIERS.md` Structured parse errors and External scanners rows; `PRODUCT_PROOF_MAP.md` parse-error claim; CLI recovery diagnostics proof; `docs/reference/diagnostics-and-recovery.md`. | Stabilizing with spans, excerpts, expected tokens, UTF-8, EOF, multiline, no-panic, generated-parser matrix canaries, object-like `parse_document()`/JSON recovery proof, and user-facing diagnostics/recovery guidance from PR #353. External-scanner dispatch now has focused parser-v4 proof for emitted-token byte spans/text, rejection of scanner tokens that are invalid in the parser state without advancing input position, direct parser-v4 `parse_document()` diagnostic-document behavior for bad input in an external-scanner grammar shape with rendered source context, and generated external-token grammar diagnostic-document matrix behavior for malformed root, empty/whitespace input, keyword/missing-condition, missing-colon, trailing-token, multibyte expression, multibyte body-token, invalid body, newline/CRLF boundary, and nested invalid-expression inputs. The generated matrix also compares `parse()` errors with `parse_document()` diagnostics for span and expected-token agreement. | Corpus-wide external-scanner recovery parity remains future work; any Stable promotion still needs support-tier review. |
| Every Stable README claim maps to proof. | README capability table; `SUPPORT_TIERS.md`; `readme_stable_claims_are_in_stable_product_lane`; `scripts/ci-product-stable.sh`; `Product Proof Result`. | Covered by current proof map and stable-product canaries. PR #383 proves the Product Proof workflow emits an always-present aggregate result while selecting Stable canaries only for relevant product surfaces, schedule, or manual dispatch. Product Proof required-gate burn-in receipts #386-#391 proved enough selected/skipped paths for the deliberate required-gate policy update. | `Product Proof Result` is now required; keep the path-selected `ci-product stable canaries` implementation job out of branch protection. |
| Experimental/developing surfaces are clearly labeled. | README capability table; `SUPPORT_TIERS.md`; `KNOWN_RED.md`; `PRODUCT_PROOF_MAP.md`; query/tooling closeout. | Covered for runtime2, broader grammars, WASM, Tree-sitter interop, CLI, benchmarks, typed CST, incremental, JSON, and the documented query subset. Query remains Stabilizing for the documented subset, not full Tree-sitter query parity. | Re-check after any README, support-tier, or release-facing wording change. |
| Product works under ordinary user pressure and fails clearly. | Downstream starter fixture; README/tutorial/book quickstart canaries; CLI selected-tree, S-expression, JSON, DOT, and document JSON recovery diagnostics; public promotion PR #795; user-experience hardening closeout; query/tooling closeout; parser/runtime maintainability closeout; CLI parse-surface closeout; CLI static S-expression closeout; CLI static JSON/DOT closeout; CLI dynamic parse boundary closeout. | Covered for local/downstream fixtures, CLI recovery smoke, document-backed static selected-tree output, document-backed static S-expression output, document-backed static JSON output, document-backed static DOT output, explicit dynamic `--dynamic` feature-gate and missing-library boundaries, public repository promotion, starter README polish, diagnostics/recovery guidance, performance receipt boundaries, query example/differential receipts, local proof-loop friction mitigation, and post-closeout tablegen/supported-gate receipts. | Published CLI install, full dynamic parse output, stable CLI/WASM schemas, and any future crates.io release surface need fresh receipts. |

## Commands And Receipts

Current stable product receipts:

```bash
just ci-supported
just ci-product-stable
cargo test -p adze-cli product_proof_workflow_routes_stable_claim_surfaces -- --exact --nocapture
cargo test -p adze-cli cargo_install_adze_cli_claims_stay_release_surface_bounded -- --exact --nocapture
cargo test -p adze-cli readme_stable_claims_are_in_stable_product_lane -- --exact --nocapture
cargo test -p adze-cli test_parse_static_tree_mode_emits_document_backed_tree -- --exact --nocapture
cargo test -p adze-cli test_parse_static_sexp_mode_emits_document_backed_sexp -- --exact --nocapture
cargo test -p adze-cli test_parse_static_json_mode_emits_document_json -- --exact --nocapture
cargo test -p adze-cli test_parse_static_dot_mode_emits_document_backed_graph -- --exact --nocapture
cargo test -p adze-cli test_parse_document_projection_modes_emit_schema_envelopes -- --exact --nocapture
cargo test -p adze-cli parse_document_json_modes_emit_recovery_diagnostics -- --exact --nocapture
cargo test --manifest-path testing/downstream-starter/Cargo.toml
cargo run --manifest-path testing/downstream-starter/Cargo.toml --example parse
```

Receipt-era note: the older `ci-product-stable` receipts below predate the
later `Product Proof Result` required-gate promotion. Current branch protection
requires `Product Proof Result`; the path-selected `ci-product stable canaries`
implementation job remains selected by path, schedule, or manual dispatch.

GitHub workflow dispatch
[`Product Proof` run 26104726428](https://github.com/EffortlessMetrics/adze-swarm/actions/runs/26104726428)
passed on 2026-05-19 from current `adze-swarm/main` after PR #281, commit
`0b79a36a`. The `ci-product stable canaries` job passed in 3m02s and the broad
advisory canaries skipped under the stable-only default. This is evidence for
the README Stable claim lane from the pre-promotion receipt era.

Local receipt after residual product-trust PRs #295-#311: `just
ci-product-stable` passed on 2026-05-20 from `adze-swarm/main` at commit
`464a32a9`, and the refreshed public promotion PR later passed the hosted
`ci-product stable canaries` job. These receipts cover README stable proof
alignment, bounded published CLI install-claim wording, clean-room README/
Getting Started/book quickstarts, checked-in downstream demo, standalone
downstream starter fixture, typed AST determinism, operator precedence, and
core table serialization canaries. This receipt predates the later aggregate
required-gate promotion.

Latest local receipt after install-receipt verifier hardening PRs #318-#322:
`just ci-product-stable` passed on 2026-05-20 from `adze-swarm/main` at commit
`4f3d451c`. This refreshes the bounded README-stable and CLI install-claim
boundary canaries from the pre-promotion swarm state.

Latest local receipt after release-blocker tracker PRs #324-#327: `just
ci-product-stable` passed on 2026-05-20 from `adze-swarm/main` at commit
`5498967c`. This reran the same advisory README-stable, clean-room quickstart,
downstream fixture, typed AST determinism, precedence, serialization, and
bounded CLI install-claim canaries after the release tracker links were added.
This receipt predates the later aggregate required-gate promotion.

Latest local receipt after the 0.9.0 workspace version bump: `just
ci-product-stable` passed on 2026-05-20 from `adze-swarm/main` at commit
`99dd12b0` after PR #330. This reran the same advisory README-stable,
clean-room quickstart, downstream fixture, typed AST determinism, precedence,
serialization, and bounded CLI install-claim canaries after the workspace
version moved to 0.9.0. This receipt predates the later aggregate
required-gate promotion.

Latest local receipt after release-surface claim-boundary PRs #332-#334:
`just ci-product-stable` passed on 2026-05-20 from `adze-swarm/main` at commit
`45e40f16`. This refreshes README-stable proof alignment, bounded
`cargo install adze-cli` wording, the co-release dependency snippet boundary
canary, clean-room README/Getting Started/book quickstarts, checked-in and
standalone downstream fixtures, typed AST determinism, precedence, and core
table serialization from the pre-promotion swarm state.

Latest local receipt after product-proof routing, Node 24 artifact-action
maintenance, and routing-canary PRs #336-#341: `just ci-product-stable` passed
on 2026-05-20 from `adze-swarm/main` at commit `8d4cc1cd`. This reran the same
advisory README-stable, install/dependency claim-boundary, stable-surface
routing, clean-room quickstart, downstream fixture, typed AST determinism,
precedence, and serialization canaries from the current swarm state. It is
the final local pre-promotion stable-product receipt in this sequence.

Latest current-main receipt after CLI static JSON/DOT closeout and release
boundary refresh PRs #464-#469: `just ci-supported` and
`just ci-product-stable` passed on 2026-05-21 from `adze-swarm/main` at commit
`ae317e42`. This refreshes the supported core pipeline and the Stable README
claim canaries after the static `tree`, `sexp`, `json`, and `dot` CLI output
lanes closed and after the latest crates.io install-boundary status updates.
It is still non-release product proof, not a crates.io install receipt.

Latest current-main receipt after CLI dynamic parse boundary closeout and audit
alignment PRs #471-#474: `just ci-supported` and `just ci-product-stable`
passed on 2026-05-22 from `adze-swarm/main` at commit `3a2f83e6`. This
refreshes the supported core pipeline and the Stable README claim canaries
after the dynamic parse boundary lane closed and after this audit was aligned
with that closeout. It is still non-release product proof, not a crates.io
install receipt or release authorization.

Latest current-main receipt after the current supported-product receipt PR
#475: `just ci-supported` and `just ci-product-stable` passed on 2026-05-22
from `adze-swarm/main` at commit `e6aa7ea0`. This reran the supported
format/clippy/test/doc surface and the Stable README claim canaries from the
actual post-receipt `main` tree. Source-of-truth checks also passed:
`cargo run -q -p xtask -- check-active-goal --mode blocking`, `cargo run -q -p
xtask -- check-doc-artifacts --mode blocking`, and `git diff --check`. This is
still non-release product proof, not a crates.io install receipt or release
authorization.

Latest Adze Adoption Hardening closeout: PRs #562-#574 completed the
non-release adoption-hardening lane and archived it in
[`../../plans/adze-adoption-hardening/closeout.md`](../../plans/adze-adoption-hardening/closeout.md).
The closeout records downstream starter fixture proof, API choice guidance,
GLR ambiguity and diagnostics/recovery walkthroughs, query cookbook receipts,
Tree-sitter selected-tree guidance, advisory benchmark receipt guidance, and
the public release-boundary checklist. GitHub receipts across the closing PRs
included `Rust Small Result`, `Product Proof Result`, Source of Truth, CI Lane
Whitelist, and GLR Invariants. The post-closeout `active.toml` state is paused
forge standby with release/publish authorization still blocked on #325.

Latest user-experience hardening closeout: PRs #350-#356 completed the
non-release adoption polish lane and archived it in
[`../../plans/user-experience-hardening/closeout.md`](../../plans/user-experience-hardening/closeout.md).
The closeout records starter README/local dependency polish, API navigation,
diagnostics/recovery guidance, performance receipt boundaries, and the Windows
supported-gate PDB-pressure mitigation. GitHub receipts across the closeout PRs
included `Rust Small Result`, Source of Truth, CI Lane Whitelist, GLR
Invariants, Docs Gate, PR Gate Success, Product Proof where relevant, and
`Supported Rust Gate` on the final closeout. The post-closeout `active.toml`
state is paused with no selected non-release lane.

Latest query/tooling expansion closeout: PRs #371-#374 completed the
non-release query subset proof refresh and archived it in
[`../../plans/query-tooling-expansion/closeout.md`](../../plans/query-tooling-expansion/closeout.md).
The closeout records the `query_highlighting` example receipt and test, the
expanded `query_differential` matrix, refreshed support-tier/product-proof
wording, and the explicit boundary that query remains Stabilizing for the
documented subset rather than full Tree-sitter query parity. GitHub receipts
included `Rust Small Result`, Source of Truth, CI Lane Whitelist, GLR
Invariants, and `ci-product stable canaries` where relevant. The post-closeout
`active.toml` state is paused with no selected non-release lane.

Latest parser/runtime maintainability closeout: PRs #443-#455 completed the
non-release parser/runtime maintainability lane and archived it in
[`../../plans/parser-runtime-maintainability/closeout.md`](../../plans/parser-runtime-maintainability/closeout.md).
The lane recorded the supported-surface audit, landed focused tablegen
validation hardening, mitigated CI linker/no-output friction, repaired exact
field-name ABI test drift discovered by Microcrate CI, and kept release/publish
work blocked on #325. Hosted receipts on #454 included `Rust Small Result`,
`Supported Rust Gate`, `Product Proof Result`,
`Test Core Crates (ir, glr-core, tablegen)`, and
`Test Pure Rust Implementation (ubuntu-latest, stable)`. Hosted receipts on
#455 included `Rust Small Result`, `Source of Truth`, `Product Proof Result`,
`Supported Rust Gate`, and `PR Gate Success`. The post-closeout `active.toml`
state is complete with no active, ready, or blocked non-release work items.

Latest CLI parse-surface closeout: PRs #457-#459 completed the non-release CLI
parse-surface hardening lane and archived it in
[`../../plans/cli-parse-surface/closeout.md`](../../plans/cli-parse-surface/closeout.md).
PR #458 made default static `adze parse <grammar.rs> <input>` emit a
document-backed selected-tree receipt from generated `parse_document()` while
keeping `json`, `sexp`, and `dot` explicitly unsupported until separate proof
lands. Local proof included:

```bash
cargo test -p adze-cli test_parse_static_tree_mode_emits_document_backed_tree -- --exact --nocapture
cargo test -p adze-cli test_parse_document_projection_modes_emit_schema_envelopes -- --exact --nocapture
cargo test -p adze-cli parse_document_json_modes_emit_recovery_diagnostics -- --exact --nocapture
```

The #458 explicit unsupported-mode canary was retired after static `sexp`,
`json`, and `dot` output gained document-backed receipts.

Hosted receipts on #458 included `Rust Small Result`, `Product Proof Result`,
`Source of Truth`, `Supported Rust Gate`, `PR Gate Success`, and
`Test Pure Rust Implementation (ubuntu-latest, stable)`. Hosted receipts on
#459 included `Rust Small Result`, `Product Proof Result`, and Source of Truth
checks. The post-closeout `active.toml` state is complete with no active,
ready, or blocked non-release work items.

Latest CLI static S-expression closeout: PRs #461-#463 completed the
non-release CLI static S-expression lane and archived it in
[`../../plans/cli-static-sexp/closeout.md`](../../plans/cli-static-sexp/closeout.md).
PR #462 made static `adze parse --output sexp <grammar.rs> <input>` emit a
document-backed selected-tree S-expression from generated `parse_document()`
while keeping `json` and `dot` explicitly unsupported until separate proof
lands. Local proof included:

```bash
cargo test -p adze-cli test_parse_static_sexp_mode_emits_document_backed_sexp -- --exact --nocapture
cargo test -p adze-cli test_parse_static_tree_mode_emits_document_backed_tree -- --exact --nocapture
cargo test -p adze-cli test_parse_document_projection_modes_emit_schema_envelopes -- --exact --nocapture
cargo test -p adze-cli parse_document_json_modes_emit_recovery_diagnostics -- --exact --nocapture
```

The #462 explicit unsupported-mode canary was retired after static `json` and
`dot` output gained document-backed receipts.

Hosted receipts on #462 included `Rust Small Result`, `Product Proof Result`,
`Source of Truth`, `Supported Rust Gate`, `PR Gate Success`,
`Test Pure Rust Implementation (ubuntu-latest, stable)`, `Test Runtime Crates`,
and `ci-product stable canaries`. The post-closeout `active.toml` state is
complete with no active, ready, or blocked non-release work items.

Latest CLI static JSON/DOT closeout: PRs #464-#466 completed the non-release
CLI static JSON and DOT lane and archived it in
[`../../plans/cli-static-json-dot/closeout.md`](../../plans/cli-static-json-dot/closeout.md).
PR #465 made static `adze parse --output json <grammar.rs> <input>` emit the
generated document JSON and made static `adze parse --output dot <grammar.rs>
<input>` render a selected-tree Graphviz graph from generated
`parse_document()` document facts. Local proof included:

```bash
cargo test -p adze-cli test_parse_static_json_mode_emits_document_json -- --exact --nocapture
cargo test -p adze-cli test_parse_static_dot_mode_emits_document_backed_graph -- --exact --nocapture
cargo test -p adze-cli test_parse_static_tree_mode_emits_document_backed_tree -- --exact --nocapture
cargo test -p adze-cli test_parse_static_sexp_mode_emits_document_backed_sexp -- --exact --nocapture
cargo test -p adze-cli test_parse_document_projection_modes_emit_schema_envelopes -- --exact --nocapture
cargo test -p adze-cli parse_document_json_modes_emit_recovery_diagnostics -- --exact --nocapture
```

Hosted receipts on #465 included `Rust Small Result`, `Product Proof Result`,
`Source of Truth`, `Test Pure Rust Implementation (ubuntu-latest, stable)`,
`Test Runtime Crates`, and `ci-product stable canaries`. The post-closeout
`active.toml` state is complete with no active, ready, or blocked non-release
work items.

Latest CLI dynamic parse boundary closeout: PRs #471-#473 completed the
non-release CLI dynamic parse boundary lane and archived it in
[`../../plans/cli-dynamic-parse/closeout.md`](../../plans/cli-dynamic-parse/closeout.md).
PR #472 added executable receipts for the no-feature `adze parse --dynamic`
gate, the feature-enabled missing-library boundary, and helper-level symbol
handling without requiring a system grammar library. It also tightened the
dynamic-loading guide, CLI README, and support-tier wording so dynamic parse
output remains experimental and unimplemented rather than a supported parse
output path. Local proof included:

```bash
cargo test -p adze-cli test_parse_dynamic_without_feature_reports_feature_gate -- --exact --nocapture
cargo test -p adze-cli --features dynamic dynamic -- --nocapture
cargo test -p adze-cli cargo_install_adze_cli_claims_stay_release_surface_bounded -- --exact --nocapture
cargo test -p adze-cli test_parse_help_documents_available_modes -- --exact --nocapture
cargo test -p adze-cli test_parse_reports_available_modes -- --exact --nocapture
cargo fmt -p adze-cli -- --check
cargo clippy -p adze-cli --all-targets --features dynamic -- -D warnings
just ci-product-stable
```

Hosted receipts on #472 included `Rust Small Result`,
`Product Proof Result`, `Source of Truth`, `PR Plan`,
`Test Runtime Crates`, and `ci-product stable canaries`. Hosted receipts on
#473 included `Rust Small Result`, `Product Proof Result`, `Source of Truth`,
`PR Plan`, and `ci-product stable canaries`. The post-closeout `active.toml`
state is complete with no active, ready, or blocked non-release work items.

Latest claim-boundary closeout after public promotion: PRs #526-#532 tightened
release-readable wording for surfaces outside the Stable generated-parser
contract. The sequence bounded `runtime2/`, `tools/ts-bridge`, `wasm-demo`,
bundled grammar crates, `adze-lsp-generator`, `adze-playground`, and tutorial
example wording so they remain experimental, advisory, prototype, or fixture
surfaces unless support tiers promote a named slice.

Local proof across the sequence included targeted command checks for changed
surfaces plus:

```bash
cargo run -q -p xtask -- check-doc-artifacts --mode blocking
cargo run -q -p xtask -- check-active-goal --mode blocking
git diff --check
cargo check -p adze-lsp-generator -p adze-playground
cargo run -q -p adze-lsp-generator --bin adze-lsp-gen -- --help
cargo run -q -p adze-playground --bin adze-playground -- --help
cargo check --manifest-path wasm-demo/Cargo.toml --target wasm32-unknown-unknown
cargo test --manifest-path wasm-demo/Cargo.toml --target wasm32-unknown-unknown --no-run
cargo test --manifest-path tools/ts-bridge/Cargo.toml --test basic
cargo run -q --manifest-path tools/ts-bridge/Cargo.toml --bin tsb-abi-check
```

Hosted receipts on the claim-boundary PRs included `Rust Small Result`,
Source of Truth, CI Lane Whitelist, GLR Invariants, PR Gate, Product Proof
where selected, and the path-routed docs/tooling/grammar receipts relevant to
each change. These PRs did not tag, publish, mutate signing/Cargo-token
workflows, or promote any experimental surface to Stable.

Latest Product Proof result-readiness receipt: `adze-swarm` PR #383 made
`.github/workflows/product-proof.yml` emit `Detect Product Proof Paths`,
`ci-product stable canaries`, and `Product Proof Result`. In GitHub run
[`26201525347`](https://github.com/EffortlessMetrics/adze-swarm/actions/runs/26201525347),
the detector passed, the Stable canaries passed, and `Product Proof Result`
passed. This proved the result context existed and could aggregate the selected
Stable canary lane before the later required-gate promotion.

Latest Product Proof required-gate burn-in receipts: `adze-swarm` PR #386 opened
the burn-in lane without changing branch protection. The PR check rollup showed
`Rust Small Result`, `ci-product stable canaries`, and `Product Proof Result`
green. This counts as a selected Stable-canary burn-in receipt; skipped-canary
receipts still need to be collected before any required-check promotion.
Follow-up PRs #387 through #390 completed the receipt mix: #387 added another
selected Stable-canary receipt, while #388 through #390 exercised the
skipped-canary path with `Product Proof Result` and `Rust Small Result` green.
PR #391 recorded the complete receipt set and moved the policy promotion item
to ready after another selected Stable-canary Product Proof run passed. PR #392
is the promotion policy PR that adds `Product Proof Result` to branch
protection while keeping `Rust Small Result` required.

Current CI-tail receipts:

- PR #284 bounded `pure-rust-ci` and `pr-gate` Rust tail steps so advisory
  Rust jobs fail clearly instead of hanging indefinitely.
- PR #285 scoped the default `pure-rust-ci` PR test step to the supported
  pipeline crates and kept full workspace tests available through
  `workflow_dispatch` / `full-ci`.
- On PR #285, `Rust Small Result` passed in 6s, `Supported Rust Gate` passed in
  22m56s, and `Test Pure Rust Implementation (ubuntu-latest, stable)` passed
  in 23m10s after running the scoped supported-crate test step.

Current structured parse-error receipts:

```bash
cargo test -p adze --features "pure-rust,glr,serialization,ts-compat" --test recovery_matrix generated_object_like_bad_input_matrix_preserves_document_diagnostics_and_json -- --exact --nocapture
cargo test -p adze --features "pure-rust,glr,serialization,ts-compat" --test recovery_matrix -- --nocapture
```

PR #293 added object-like `parse_document()` and document JSON recovery proof
for missing separators, multibyte invalid identifier continuations, multiline
invalid values, and multiline EOF. This narrows the invalid-span product gap;
it is not a support-tier promotion.

Current external-scanner dispatch receipts:

```bash
cargo test -p adze --features "pure-rust,external_scanners" parser_v4::tests::test_parser_with_external_scanner -- --exact --nocapture
cargo test -p adze --features "pure-rust,external_scanners" parser_v4::tests::test_external_scanner_rejects_token_not_in_valid_symbols -- --exact --nocapture
cargo test -p adze --features "pure-rust,external_scanners" parser_v4::tests::test_external_scanner_parse_document_bad_input_returns_diagnostic_document -- --exact --nocapture
cargo test --manifest-path example/Cargo.toml external_word_example::tests::generated_external_grammar_bad_input_matrix_returns_diagnostic_document --features pure-rust -- --exact --nocapture
scripts/ci-product.sh --dry-run
cargo test -p adze --features "pure-rust,external_scanners" parser_v4::tests -- --nocapture
cargo test -p adze --features external_scanners
```

PR #298 fixed parser-v4 external scanner token spans so emitted tokens use the
pre-scan byte position and preserve token text. This closes the focused
dispatch/span gap for the parser-v4 scanner canaries. The follow-up parser-v4
document canary proves bad input in a direct external-scanner grammar shape
returns a diagnostic document with error facts. PR #316 expanded the generated
external-token example matrix proving generated `parse_document()` returns
diagnostic documents with bounded byte spans, matching point ranges,
selected-tree error facts, and public expected-token names for malformed root,
keyword, missing-colon, and trailing-token inputs. PR #343 adds multibyte
expression, invalid body, and newline-boundary body inputs and compares
generated `parse()` errors with `parse_document()` diagnostics for span and
expected-token agreement. PR #345 added the focused parser-v4 and generated
external-token proof commands to the broad advisory `ci-product.sh` lane and
routed edits to that script through Product Proof. PR #359 adds empty-source,
whitespace-only, missing-condition, multibyte body-token, CRLF boundary, and
nested invalid-expression generated cases. PR #360 adds parser-v4 canaries for
rejected-token input-position safety and rendered source diagnostics. Hosted PR
#345 passed `Rust Small Result`, `Source of Truth`, `ci-product stable
canaries`, `Supported Rust Gate`, and the broad Pure Rust implementation tail;
PRs #359 and #360 passed `Rust Small Result`, Source of Truth, and the relevant
path-routed proof receipts. PR #377 broadened the generated external-token
matrix again with invalid body, nested invalid-expression, CRLF, multibyte, and
empty/whitespace cases; PR #379 expanded the direct parser-v4 diagnostic smoke
across invalid root, UTF-8, newline, extra-newline, and CRLF shapes. PR #379
passed `Rust Small Result`, Source of Truth, GLR Invariants, PR Gate, Coverage
Lite, `ci-product stable canaries`, Golden-Master smoke, and the relevant
path-routed runtime/product receipts. These receipts close the targeted
real-grammar recovery lane, but do not promote external scanners out of
Experimental, prove corpus-wide external-scanner recovery parity, or create a
stable public scanner API claim.

Current release-surface readiness receipts:

```bash
just check-publishable
```

`just check-publishable` passed on 2026-05-19 from `adze-swarm/main` after
PR #253, again at commit `b613ebbb` after residual product-trust PRs
#295-#301, again on 2026-05-20 at commit `e965cba2` after PRs #309-#310, and
again on 2026-05-20 at commit `464a32a9` after PR #311. It was refreshed again
on 2026-05-20 from current `adze-swarm/main` at commit `fc959ec1`, after the
stable-product receipt status update. It was refreshed again on 2026-05-20 from
`adze-swarm/main` at commit `99dd12b0`, after PR #330 bumped the publishable
workspace crates to 0.9.0, and again on 2026-05-21 from `adze-swarm/main` at
commit `ae317e42` after PRs #468-#469. It was refreshed again on 2026-05-22
from `adze-swarm/main` at commit `e6aa7ea0` after the current supported and
Stable-product receipts were recorded.
It verifies publish-order metadata and `cargo package --list` for
the core publish surface (`adze-common`, `adze-ir`, `adze-glr-core`,
`adze-tablegen`, `adze-macro`, `adze-tool`, `adze-cli`, and `adze`). This is
package metadata/file-list evidence only; it does not publish crates or prove
registry installation.

Current public promotion receipt:

Public `EffortlessMetrics/adze#795` supersedes closed/unmerged public PR #794.
It was opened from the explicit public promotion execution decision, refreshed
to `adze-swarm/main` commit `464a32a9`, and merged into public `main` on
2026-05-20 as squash commit `a0d593e8`. The promoted public tree matches the
`adze-swarm/main` tree at `464a32a9`. The refreshed public PR #795 check set
passed on 2026-05-20, including `Rust Small Result`, `Supported Rust Gate`, `PR Gate
Success`, `Source of Truth`, `CI Lane Whitelist`, `GLR Invariants`, `Coverage
Lite`, `ci-product stable canaries`, `Test Core Crates`, `Test Runtime Crates`,
and `Test Pure Rust Implementation`.

The public `main` branch-protection context was corrected from stale
`ci-supported` to `Rust Small Result`, matching `.github/settings.yml` in the
promoted tree. Before that correction, a manual `CI` workflow dispatch on the
promotion branch also passed the legacy `ci-supported` job. No release tag,
crate publish, signing, or Cargo-token workflow change happened as part of the
promotion.

Current first-use / CLI boundary receipts:

```bash
cargo test -p adze-cli test_init -- --nocapture
cargo test -p adze-cli getting_started_quickstart_builds_parses_and_reports_diagnostics -- --exact --nocapture
cargo test -p adze-cli cargo_install_adze_cli_claims_stay_release_surface_bounded -- --exact --nocapture
cargo test -p adze-cli co_release_dependency_snippets_stay_release_surface_bounded -- --exact --nocapture
cargo test -p adze-cli test_parse_dynamic_without_feature_reports_feature_gate -- --exact --nocapture
cargo test -p adze-cli --features dynamic dynamic -- --nocapture
just package-local adze-cli
cargo info --registry crates-io adze
cargo info --registry crates-io adze-tool
cargo info --registry crates-io adze-cli
cargo run -q -p xtask -- verify-crates-io-install adze-cli --bin adze --version X.Y.Z --locked
```

The root README install block is now explicitly bounded as a release-surface
dependency shape rather than a crates.io install receipt for every co-release
crate. On 2026-05-20, `cargo info --registry crates-io adze` reported published
`adze` 0.8.0, while `cargo info --registry crates-io adze-tool` reported that
`adze-tool` could not be found in crates.io. Until the coordinated publish
receipt exists, the README dependency block must not be treated as proof that
the current repo can be consumed directly from crates.io.

`just package-local adze-cli` packages and verifies the CLI crate with local
patches for unpublished co-release crates. It passed on 2026-05-19 from
`adze-swarm`, producing and verifying `adze-cli v0.9.0`, and passed again
on 2026-05-20 from `adze-swarm/main` at commit `390ab76f`. It was refreshed
again from `adze-swarm/main` at commit `99dd12b0` after the workspace version
bump, and again on 2026-05-22 from `adze-swarm/main` at commit `e6aa7ea0`.
The current refresh packaged and verified `adze-cli v0.9.0`; Cargo printed
non-fatal unused patch warnings for packages not used in the `adze-cli`
verification graph. This is local publish-readiness evidence, not a crates.io
install receipt.

`cargo_install_adze_cli_claims_stay_release_surface_bounded` keeps live
beginner/status/spec docs from presenting `cargo install adze-cli` as a
release-surface quickstart until a crates.io receipt exists. It is a
claim-boundary canary, not registry installation proof.

`co_release_dependency_snippets_stay_release_surface_bounded` keeps live
README/FAQ/tutorial/book dependency snippets that name `adze-tool` or
registry-shaped `cargo add --build adze-tool` commands explicitly bounded as
release-surface shapes until co-release crates have crates.io receipts. It is a
claim-boundary canary, not dependency-resolution or install proof.
The `Product Proof` workflow path filter covers these live claim-boundary docs
so edits to the scanned docs route to the stable product canaries while the lane
remains advisory. The `product_proof_workflow_routes_stable_claim_surfaces`
canary keeps the workflow path filter aligned with the stable product and
claim-boundary surfaces.

Hosted PR #334 also passed `Rust Small Result`, `ci-product stable canaries`,
`Source of Truth`, `Supported Rust Gate`, `Test Pure Rust Implementation
(ubuntu-latest, stable)`, and the relevant docs/runtime receipt jobs before the
post-merge check set finished green. No release tag, crate publish,
signing/Cargo-token workflow change, or support-tier promotion happened in that
PR.

Use `cargo info --registry crates-io adze-cli` when verifying registry
publication. The explicit registry flag avoids resolving the local workspace
package. It reported on 2026-05-20 that `adze-cli` could not be found in
crates.io, including current-main refreshes at commits `fc959ec1` and
`99dd12b0`. It was refreshed again on 2026-05-21 from `adze-swarm/main` at
commit `0df9f420` after the CLI static JSON/DOT status alignment, and still
reported that `adze-cli` could not be found in crates.io. In the same registry
check, `cargo info --registry crates-io adze` resolved published `adze 0.8.0`,
while `cargo info --registry crates-io adze-tool` reported no registry package.
The registry absence check was refreshed again on 2026-05-21 from
`adze-swarm/main` at commit `ae317e42`; `adze-cli` and `adze-tool` were still
absent from crates.io.
The crates.io metadata check was refreshed again on 2026-05-22 from
`adze-swarm/main` after PR #476; `adze-cli` and `adze-tool` were still absent
from the explicit `crates-io` registry, while `adze` resolved to published
`adze 0.8.0`.
`cargo install adze-cli` remains a release-surface target rather than current
product proof.

`cargo run -q -p xtask -- verify-crates-io-install adze-cli --bin adze --version
X.Y.Z --locked` is the post-publish receipt hook for the missing crates.io
install proof. The verifier's metadata check uses the explicit `crates-io`
registry, and the install command also passes `--registry crates-io`, so the
receipt cannot pass by resolving the local workspace package or another
configured default registry. It installs from crates.io into an isolated
temporary root and runs
`adze --version`. The `--dry-run` mode is pre-publish command-shape evidence
only; it does not contact crates.io and does not close the install-receipt gap.
The dry-run command shape was refreshed on 2026-05-20 from `adze-swarm/main` at
commit `df4be63a` after PRs #319-#320 and refreshed again from current
`adze-swarm/main` at commit `fc959ec1`. It was refreshed again on 2026-05-21
from `adze-swarm/main` at commit `ae317e42`; it printed:

```text
cargo info --registry crates-io adze-cli
cargo install --registry crates-io adze-cli --root <temp-root> --version X.Y.Z --locked
<temp-root>/bin/adze.exe --version
```

The dry-run command shape was refreshed again on 2026-05-22 from
`adze-swarm/main` at commit `e6aa7ea0` and printed the same explicit
`crates-io` command sequence. It remains pre-publish command-shape evidence
only.

Latest public release-surface drift refreshes are tracked on
[`adze-swarm#325`](https://github.com/EffortlessMetrics/adze-swarm/issues/325).
Do not rely on hard-coded drift counts in this audit: a status PR that records
fresh receipts changes the swarm tree and can make the exact diff count stale
as soon as it merges.

The stable invariant is that a non-empty public `adze/main` versus
`adze-swarm/main` diff is a release blocker until maintainers select a release
candidate and promote it into public `EffortlessMetrics/adze` with an explicit
public promotion PR. It is not a reason to tag, publish, or move release
secrets into `adze-swarm`.

## Current Non-Completion Reasons

Do not mark the product objective complete while any of these are true:

- `cargo install adze-cli` has no crates.io install receipt.
- The root README dependency block is release-surface-bounded because
  `adze-tool` does not yet have a crates.io metadata receipt.
- Public `adze/main` is not tree-identical to current `adze-swarm/main`; after
  the post-promotion claim-boundary cleanup, `origin/main` is
  `81db54aa4986a36bf4c24d545cffc877e749f01f` and `public/main` is
  `6263c6a80046d13fb98e3ad319dfe726f32f1010` as of the 2026-05-22 refresh.
  An explicit public promotion PR is required before any authorized publish
  from the public release surface.
- Corpus-wide external-scanner recovery parity remains future work and is not
  a Stable claim.
- GLR conflict routing, structured parse errors, Tree-sitter compatibility,
  query compatibility, CLI selected-tree/S-expression/document output, and
  `AdzeDocument` are not all Stable; their current tiers and limitations are recorded in
  `SUPPORT_TIERS.md`.

## Next Concrete Actions

The routine product-proof, Adze adoption hardening, user-experience,
external-scanner recovery, parser-recovery real-grammar, query/tooling
expansion, parser/runtime maintainability, CLI parse-surface, CLI static
S-expression, CLI static JSON/DOT, and CLI dynamic parse boundary lanes are
closed out:
[`../../plans/adze-adoption-hardening/closeout.md`](../../plans/adze-adoption-hardening/closeout.md),
[`../../plans/user-experience-hardening/closeout.md`](../../plans/user-experience-hardening/closeout.md),
[`../../plans/external-scanner-recovery/closeout.md`](../../plans/external-scanner-recovery/closeout.md),
[`../../plans/parser-recovery-real-grammar/closeout.md`](../../plans/parser-recovery-real-grammar/closeout.md),
and
[`../../plans/query-tooling-expansion/closeout.md`](../../plans/query-tooling-expansion/closeout.md),
plus
[`../../plans/parser-runtime-maintainability/closeout.md`](../../plans/parser-runtime-maintainability/closeout.md)
and
[`../../plans/cli-parse-surface/closeout.md`](../../plans/cli-parse-surface/closeout.md),
plus
[`../../plans/cli-static-sexp/closeout.md`](../../plans/cli-static-sexp/closeout.md),
[`../../plans/cli-static-json-dot/closeout.md`](../../plans/cli-static-json-dot/closeout.md),
and
[`../../plans/cli-dynamic-parse/closeout.md`](../../plans/cli-dynamic-parse/closeout.md).
Release authorization and post-publish crates.io install receipt work remain
separate and tracked in
[`adze-swarm#325`](https://github.com/EffortlessMetrics/adze-swarm/issues/325).

1. If release/publish is authorized, refresh public `main`, rerun release
   preflight, follow `docs/reference/PUBLISH_CHECKLIST.md`, then run the
   crates.io install receipt after publish and before any doc claims
   `cargo install adze-cli` as the supported quickstart.
2. If no release/publish authorization exists, do not tag, publish, mutate
   signing/Cargo-token workflows, or claim `cargo install adze-cli`.
3. If Product Proof required-gate behavior flakes, roll back by removing
   `Product Proof Result` from `.github/settings.yml` and restoring the
   advisory wording in `.github/CI_LANES.md`, `KNOWN_RED.md`, and this audit.
4. For future non-release work, open a fresh active goal in `adze-swarm`; do
   not promote external scanners or query compatibility beyond their proven
   support-tier slices.
5. If no release/publish authorization exists and no material product/proof gap
   is selected, leave the repo without an invented active lane rather than
   creating routine status-refresh PRs.
