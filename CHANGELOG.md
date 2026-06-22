# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased] — 0.9.0

**Focus**: Test infrastructure, quality hardening, documentation, security audit, and RC readiness.

**Summary**: 10 waves of parallel agent work, 75+ commits. 1,700+ tests passing with 0 failures.
All supported crates are fmt clean, clippy clean, and fully tested. WASM verified, security clean.

### Added
- **1,700+ tests** across all supported crates, covering unit, integration, snapshot, property-based, and end-to-end scenarios
- **BDD scenario tests** for runtime crate using declarative test patterns
- **Property-based testing** with `proptest` for grammar and parser invariants
- **Mutation testing** setup with `cargo-mutants` for test-quality verification
- **Feature combination verification** script to validate all feature-flag permutations
- **Benchmark baselines** with `criterion` for parser and table-generation performance
- **20 fuzz targets** covering parser inputs, grammar construction, serialization, external scanners, stack pool, and concurrency
- **Golden test expansion** — additional reference grammars and hash-verified parse trees
- **Cross-crate integration tests** validating the full IR → GLR → tablegen → runtime pipeline
- **Book with 6+ chapters**: grammar design, GLR parsing, external scanners, API reference, and more
- **Cross-platform CI advisory jobs** for macOS and Windows alongside Linux verification
- **Publish order documentation** for crates.io release sequence
- **`.editorconfig` and VS Code settings** for consistent contributor experience
- **Security audit** with `cargo-audit` — 0 known vulnerabilities

### Fixed
- All **clippy warnings resolved** across supported crates
- **Rustdoc warnings eliminated** for clean `cargo doc` builds
- **Runtime compilation errors** fixed for edge-case feature combinations
- **Error messages improved** — 38 diagnostic messages made more actionable
- **Snapshot test assertions corrected** to match current parser output

### Changed
- **WASM build compatibility** verified for all core crates (`wasm32-unknown-unknown`, `wasm32-wasi`)
- **Workspace dependencies centralized** — 9 common deps lifted to `[workspace.dependencies]`
- **`cargo-deny` configuration** updated with current advisory database
- **CI workflow enhanced** with feature matrix covering default, `glr-core`, `incremental`, and `all-features`
- **Concurrency caps** standardized across CI and local testing (RUST_TEST_THREADS=2, RAYON_NUM_THREADS=4)

### Security
- **SAFETY comments** added to all `unsafe` blocks per Rust best practices
- **`cargo-audit` clean** — 0 advisories across the full dependency tree

---

## [0.8.0] - 2026-02-22

**Focus**: Publishable baseline, documentation sync, and governance-as-code.

### Added
- **Governance-as-Code**: Integrated policy enforcement for backend selection (Pure-Rust vs GLR) and progress tracking via 25+ micro-crates in `crates/`.
- **Table Compression**: Optimized parse tables using Tree-sitter's format, achieving >10x reduction in binary size for large grammars.
- **Improved Macro Extraction**: Enhanced `Extract` trait for more robust typed AST construction from both LR(1) and GLR trees.
- **Standardized CI Lane**: Defined the "Supported Lane" (`just ci-supported`) ensuring core reliability across platforms.
- **Comprehensive Documentation**: Complete overhaul of all guides and READMEs to reflect the transition from `rust-sitter` to Adze.

### Changed
- **Project Rename**: Formally transitioned from `rust-sitter` to **Adze**.
- **MSRV**: Updated Minimum Supported Rust Version to **1.92** (Rust 2024 edition).
- **Default Backend**: Pure-Rust LR(1) is now the default and primary recommended path.

### Fixed
- **Precedence Disambiguation**: Corrected operator precedence conflicts in the GLR runtime.
- **EOF Handling**: Fixed proper end-of-input processing in the pure-Rust parser.
- **FFI Hardening**: Eliminated potential segmentation faults in the legacy C bridge.

---

## [0.7.0] - 2025-12-20

**Focus**: GLR Engine Completion and Ambiguity Handling.

### Added
- **GLR Fork/Merge**: Implementation of stack forking and merging (SPPF) for ambiguous grammars.
- **External Scanners**: Support for custom lexing logic in Rust (implemented for Python indentation).
- **Initial Query Support**: Basic Tree-sitter query pattern matching (`.scm` files).

---

## [0.6.1-beta] - 2025-01-22

### Fixed
- Fixed Accept action encoding (0x7FFF to 0xFFFF).
- Corrected decoder check order.
- Fixed token_count calculation to include EOF symbol.
- Added missing GOTO table entries to compressed parse tables.
