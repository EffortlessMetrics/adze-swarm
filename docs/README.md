# Adze Documentation

> **Status:** Documentation structured according to the [Diataxis framework](https://diataxis.fr/) for Adze 0.8.0-dev.

Welcome to the Adze documentation. Adze (formerly `rust-sitter`) is a Rust-native grammar toolchain for building high-performance parsers.

---

## 🎓 Tutorials
*Learning-oriented: guided lessons to help you get started.*

- [**Quickstart: First Parser In 10 Minutes**](./tutorials/quickstart-10-minutes.md) - Generate, test, and run the canonical starter parser.
- [**Your First Parser**](./tutorials/getting-started.md) - Build a working calculator parser in 5 minutes.
- [**GLR Quickstart**](./tutorials/glr-quickstart.md) - Understanding and building your first ambiguous grammar.

---

## 🛠️ How-to Guides
*Task-oriented: step-by-step guides to solve specific problems.*

- [**Handling Precedence**](./how-to/handle-precedence.md) - How to resolve operator ambiguity and associativity.
- [**External Scanners**](./how-to/external-scanners.md) - Integrating custom Rust/C logic for indentation and complex tokens.
- [**Testing Grammars**](./how-to/test-grammars.md) - Using unit tests, snapshots, and golden tests.
- [**Incremental Parsing**](./how-to/incremental-parsing.md) - Reparsing partial text changes for IDE performance.
- [**Optimizing Performance**](./how-to/optimize-performance.md) - SIMD, GLR tuning, and profiling your parser.
- [**LSP Generation**](./how-to/generate-lsp.md) - Generating a Language Server for your grammar.
- [**Using the Playground**](./how-to/use-playground.md) - Developing grammars interactively in the browser.
- [**Visualizing GLR**](./how-to/visualize-glr.md) - Inspecting conflict reports and DOT automaton graphs.
- [**Querying with Metadata**](./how-to/query-with-metadata.md) - Using symbol metadata in Tree-sitter queries.
- [**C++ Templates Cookbook**](./how-to/cookbook-cpp-templates.md) - Best practices for parsing complex C++ constructs.

---

## 📚 Reference
*Information-oriented: technical descriptions and specifications.*

- [**API Reference**](./reference/api.md) - Detailed docs for the `adze` crate and macro attributes.
- [**Which API Should I Use?**](./reference/which-api-should-i-use.md) - Decision guide for `parse`, `parse_document`, document projections, Tree-sitter compatibility, queries, JSON, and CLI surfaces.
- [**Grammar Examples**](./reference/grammar-examples.md) - Patterns for common constructs (Choices, Repeats, Optionals).
- [**Parser Cookbook**](./reference/parser-cookbook.md) - Tested recipes for typed parsers, documents, diagnostics, GLR ambiguity, Tree-sitter-compatible output, and query captures.
- [**Usage Examples**](./reference/usage-examples.md) - Practical code snippets for common tasks.
- [**Language Support**](./reference/language-support.md) - Status of built-in grammars (Python, JS, Go).
- [**Known Limitations**](./reference/known-limitations.md) - Current status of experimental features.
- [**Tree-sitter Compatibility**](./reference/tree-sitter-compatibility.md) - Supported selected-tree adapter subset and table-format invariants.
- [**Migrating From Tree-sitter**](./reference/migrating-from-tree-sitter.md) - Mapping from Tree-sitter trees, nodes, fields, node-types, queries, errors, and ambiguity to Adze's document-centered model.
- [**Query Compatibility**](./reference/query-compatibility.md) - Supported Tree-sitter query subset, source-aware behavior, and known gaps.
- [**Tree-sitter Alias Semantics**](./reference/ts-compat-alias-semantics.md) - Draft target contract for alias-visible compatibility behavior.
- [**Empty Rules Reference**](./reference/empty-rules-reference.md) - Quick reference for handling ε-productions.

---

## 💡 Explanations
*Understanding-oriented: conceptual background and architectural theory.*

- [**Mental Model**](./explanations/mental-model.md) - How Rust types, generated parsers, `parse()`, `AdzeDocument`, and projections fit together.
- [**Architecture Overview**](./explanations/architecture.md) - How the Macro, Tool, and Runtime fit together.
- [**AdzeDocument Design Contract**](./design/adze-document.md) - Draft native parse-product contract for future CST, typed AST, diagnostics, and compatibility projections.
- [**Typed CST Design Contract**](./design/typed-cst.md) - Draft generated typed syntax view over the native parse document.
- [**GLR Internals**](./explanations/glr-internals.md) - Deep dive into the Generalized LR engine.
- [**Incremental Theory**](./explanations/incremental-parsing-theory.md) - The Direct Forest Splicing algorithm.
- [**Test Strategy**](./explanations/test-strategy.md) - Why and how we test Adze.
- [**Arena Allocation**](./explanations/arena-allocator.md) - Efficient memory management for parse trees.
- [**Symbol Normalization**](./explanations/symbol-normalization.md) - How Adze simplifies complex grammar rules.
- [**Query Predicates**](./explanations/query-predicates.md) - How #eq?, #match?, etc. are evaluated.
- [**Empty Rules Theory**](./explanations/empty-rules.md) - The challenges of nullable productions in LR(1).
- [**GOTO Indexing**](./explanations/goto-indexing.md) - Mathematical invariants of our table compression.

---

## Project Status

- [**Roadmap**](../ROADMAP.md) - Milestones for 0.8.0, 0.9.0, and 1.0.
- [**Source-Of-Truth System**](./reference/SPEC_SYSTEM.md) - Repo rails for proposals, specs, ADRs, plans, active goals, proof, and policy ledgers.
- [**Proposals**](./proposals/README.md) - PRD-style "why" documents for product and repo-governance campaigns.
- [**Specs**](./specs/README.md) - Behavior contracts, acceptance criteria, and proof requirements.
- [**Architecture Decisions**](./adr/README.md) - Durable architecture decisions and their consequences.
- [**0.9.0 Plans**](../plans/0.9.0/README.md) - PR-sized implementation sequencing and proof commands.
- [**Microcrate To SRP Plan**](../plans/0.9.0/microcrate-collapse.md) - Release-blocking transition from migration-target microcrates to SRP owner submodules.
- [**API Foundation Plan**](../plans/0.9.0/api-foundation.md) - PR-sized sequence for `AdzeDocument` and its typed, diagnostic, GLR, compatibility, JSON, CLI, and WASM projections.
- [**Release Promotion Readiness Plan**](../plans/release-promotion/implementation-plan.md) - Current campaign for inventorying, auditing, and planning any public `adze` promotion from `adze-swarm`.
- [**Active Goals**](../.adze/goals/README.md) - Machine-readable Droid/Codex execution state conventions.
- [**Document Artifact Ledger**](../policy/doc-artifacts.toml) - Machine-readable registry for proposals, specs, ADRs, and implementation plans.
- [**Correctness Push Plan**](./status/CORRECTNESS_PUSH.md) - Current merge/proof sequence for parser, GLR, tablegen ABI, CLI, and product-proof convergence.
- [**Support Tiers**](./status/SUPPORT_TIERS.md) - Feature claims mapped to proof commands and CI lanes.
- [**Product Proof Map**](./status/PRODUCT_PROOF_MAP.md) - Release-readable summary of product claims and their representative proof.
- [**Product Acceptance Matrix**](./product/ACCEPTANCE_MATRIX.md) - User workflows mapped to required proof, claim boundaries, and support-tier impact.
- [**Performance Baselines**](./perf/baselines.md) - Advisory benchmark baseline policy, receipt fields, and non-claims.
- [**Friction Log**](./status/FRICTION_LOG.md) - Current developer pain points we are burning down.
- [**Now / Next / Later**](./status/NOW_NEXT_LATER.md) - Rolling execution plan.
- [**Known Red**](./status/KNOWN_RED.md) - Exclusions from the supported CI lane.
- [**PR Template**](./PR_TEMPLATE.md) - Checklist for contributors.
- [**Verification**](./VERIFICATION.md) - README badge meanings, generated endpoints, and PR evidence boundaries.
- [**GLR Toolkit Fixture Taxonomy**](./testing/glr-fixture-taxonomy.md) - Shared fixture classes for GLR, compatibility, query, recovery, and benchmark proof.
