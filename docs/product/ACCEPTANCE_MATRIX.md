# Adze Product Acceptance Matrix

Status: active
Owner: runtime/product
Created: 2026-05-18
Linked proposal: ../proposals/ADZE-PROP-0004-toolkit-excellence.md
Linked plan: ../../plans/toolkit-excellence/implementation-plan.md
Support-tier map: ../status/SUPPORT_TIERS.md

This matrix defines what "Adze is easy to use and product-ready" means for the
toolkit excellence campaign. It is not a support-tier promotion by itself.
Support tiers remain the source of truth for public stability claims.

## Rules

- Every row names a user workflow, not just an internal subsystem.
- Every workflow has a repeatable proof command or an explicit next proof.
- Beginner workflows use `grammar::parse` and `grammar::parse_document`.
- Advanced workflows project from `AdzeDocument`; they do not create a second
  parse truth.
- Known gaps stay visible until support tiers and proof commands justify
  stronger claims.
- Performance rows record receipts; they do not create speed guarantees.

## Matrix

| Workflow | User promise | Required proof | Claim boundary | Support-tier impact |
| --- | --- | --- | --- | --- |
| Initialize project | A new user can run the generated starter flow from a repo-built CLI now, and from `cargo install adze-cli` only after the CLI is published. | `cargo test -p adze-cli test_init_default_cwd_generates_buildable_project -- --exact --nocapture`; `cargo test -p adze-cli test_init_generates_buildable_project -- --exact --nocapture`; `just package-local adze-cli` | Proves the generated starter path and local CLI package verification, not crates.io CLI installation or every install environment. | Keeps starter path eligible for public docs; no tier promotion by itself. |
| Downstream starter | A clean user-shaped crate can depend on local Adze crates, run `build.rs`, parse a typed AST, report diagnostics, and run a parse example. | `cargo test --manifest-path testing/downstream-starter/Cargo.toml`; `cargo run --manifest-path testing/downstream-starter/Cargo.toml --example parse` | Proves path-dependency downstream wiring and generated parser shape; publish/install behavior remains separate. | Adds product acceptance evidence for the stable typed-parser claim. |
| Parse typed AST | A library user can call `grammar::parse(source)` and get typed Rust values for exact input. | `cargo test -p adze --features pure-rust --test typed_ast_contract -- --nocapture` | Applies to generated parser contracts covered by support tiers. | Supports existing typed extraction Stable claim. |
| Parse document | A tooling user can call `grammar::parse_document(source)` and inspect the same parse product. | `cargo test -p adze --features "pure-rust,ts-compat" --test adze_document_alpha -- --nocapture` | `AdzeDocument` remains the canonical parse product; API stability still follows support tiers. | Candidate evidence for `AdzeDocument` promotion only after docs and limitations align. |
| Diagnostics | Bad input reports structured diagnostics with spans, excerpts, and expected tokens where available. | `cargo test -p adze --features "pure-rust,glr" --test generated_parse_errors -- --nocapture` | Does not promise perfect recovery for every grammar or external scanner. | Supports structured parse-error rows and future stable generated-matrix claims. |
| GLR ambiguity | Ambiguous input exposes deterministic selected output plus native ambiguity summaries. | `cargo test -p adze --features "pure-rust,glr,runtime-e2e" --test glr_conflict_matrix -- --nocapture` | Full raw forest export remains experimental unless support tiers say otherwise. | Supports GLR conflict-routing and ambiguity-summary proof. |
| Tree-sitter selected tree | Tree-sitter-shaped traversal works for the documented selected-tree subset. | `cargo test -p adze --features "pure-rust,glr,ts-compat" --test ts_compat_selected_tree -- --nocapture` | Not a full Tree-sitter API, query, or grammar-corpus parity claim. | Candidate evidence for selected-tree subset promotion. |
| Query subset | Supported query patterns behave predictably, including source-aware predicate boundaries. | `cargo test -p adze --features "pure-rust,ts-compat,query" --test query_differential -- --nocapture` | Unsupported Tree-sitter query features remain known gaps, not hidden failures. | Candidate evidence for documented query subset promotion. |
| JSON projection | Document JSON exposes schema-versioned document facts from the same parse. | `cargo test -p adze --features "pure-rust,serialization" --test adze_document_json -- --nocapture` | Stable serialized ABI is separate from advisory JSON proof. | Supports JSON projection rows without creating stable schema claims. |
| CLI parse/check | CLI commands expose useful parse, document, diagnostics, and ambiguity output. | `cargo test -p adze-cli test_parse_document_projection_modes_emit_schema_envelopes -- --exact --nocapture`; `cargo test -p adze-cli parse_document_json_modes_emit_recovery_diagnostics -- --exact --nocapture` | CLI remains scoped to proven command behavior and documented formats. | Candidate evidence for CLI document output promotion. |
| WASM compile | WASM-facing code compiles as an advisory integration surface. | `rustup target add wasm32-unknown-unknown`; `cargo check --manifest-path wasm-demo/Cargo.toml --target wasm32-unknown-unknown` | Compile proof is not a stable JS API claim. | Keeps WASM advisory until smoke behavior and docs exist. |
| Performance receipt | Benchmarks produce reproducible receipts for parse, document, projection, query, diagnostics, and table paths. | `cargo run -q -p xtask -- perf-receipt --profile product-smoke`; `cargo bench -p adze-benchmarks --no-run` | No speed claims or thresholds without baseline history and support-tier policy. | Supports advisory performance evidence only. |

## Acceptance Flow

1. Start with the starter path and downstream fixture.
2. Align README, book, quickstart, and API-choice docs with the proven path.
3. Add examples for ambiguity, query/highlighting, diagnostics, and recovery.
4. Publish compatibility matrices and known gaps for Tree-sitter and query.
5. Record performance receipts without promoting speed claims.
6. Update `docs/status/SUPPORT_TIERS.md` only for slices with repeatable proof,
   limitations, and user-facing wording.

## Stop Conditions

- A workflow cannot name a proof command.
- A public claim would exceed `docs/status/SUPPORT_TIERS.md`.
- A PR changes runtime behavior while presenting as docs-only.
- Work starts in public `EffortlessMetrics/adze` instead of `adze-swarm`.
- A compatibility row implies full Tree-sitter or query parity.
