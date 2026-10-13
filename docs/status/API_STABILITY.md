# API Stability Matrix

**Last updated:** 2026-08-27
**Workspace version:** 0.9.0

This document catalogs the stability status of every public API surface in the adze workspace.
It is an API-semver inventory, not the product support-tier proof map.
For feature support tiers and repeatable proof commands, use [`SUPPORT_TIERS.md`](./SUPPORT_TIERS.md).

## Stability Levels

| Level | Meaning |
|-------|---------|
| **Stable** | Covered by semver. Will not break without a major version bump. |
| **Unstable** | Functional and tested, but signature or behavior may change in minor releases. |
| **Experimental** | Works in limited cases. May be removed or radically redesigned. |
| **Deprecated** | Scheduled for removal. Use the noted replacement instead. |
| **Internal** | Exposed for macro/codegen use only. Not part of the public contract. |

Stable rows here identify API compatibility guarantees. They do not by themselves promote a feature to the README-stable product contract; product promotion requires a named proof command in [`SUPPORT_TIERS.md`](./SUPPORT_TIERS.md).

## Summary

| Crate | Stable | Unstable | Experimental | Deprecated | Internal | Total |
|-------|--------|----------|-------------|------------|----------|-------|
| `adze` (runtime) | 8 | 18 | 12 | 5 | 5 | 48 |
| `adze-macro` | 10 | 1 | 0 | 0 | 0 | 11 |
| `adze-tool` | 3 | 5 | 2 | 0 | 0 | 10 |
| `adze-common` | 3 | 2 | 0 | 0 | 0 | 5 |
| `adze-ir` | 14 | 7 | 1 | 0 | 0 | 22 |
| `adze-glr-core` | 10 | 14 | 5 | 1 | 8 | 38 |
| `adze-tablegen` | 6 | 10 | 3 | 0 | 0 | 19 |

---

## `adze` (runtime)

The main user-facing crate. Re-exports `adze-macro` proc-macros at the top level.

### Public Types

| API | Kind | Stability | Feature | Docs | Tests |
|-----|------|-----------|---------|------|-------|
| `Extract<Output>` | trait | **Stable** | — | ✅ | ✅ |
| `ExtractDefault<Output>` | trait | **Stable** | — | ✅ | ✅ |
| `WithLeaf<L>` | struct | **Stable** | — | ✅ | ✅ |
| `Spanned<T>` | struct | **Stable** | — | ✅ | ✅ |
| `SpanError` | struct | **Stable** | — | ✅ | ✅ |
| `SpanErrorReason` | enum | **Stable** | — | ✅ | ✅ |
| `SymbolId` (type alias) | type | **Stable** | — | ✅ | — |
| `ParserBackend` | enum | Unstable | — | ✅ | ✅ |
| `ParserFeatureProfile` | struct | Unstable | — | ✅ | — |

### Public Functions / Re-exports

| API | Kind | Stability | Feature | Docs | Tests |
|-----|------|-----------|---------|------|-------|
| `parser_feature_profile_for_runtime()` | fn | Unstable | — | ✅ | — |
| `bdd_progress_report_for_current_profile()` | fn | Unstable | — | ✅ | — |
| `bdd_status_line_for_current_profile()` | fn | Unstable | — | ✅ | — |
| `current_backend_for()` | fn | Unstable | — | ✅ | — |
| `runtime_governance_snapshot()` | fn | Unstable | — | ✅ | — |

### Public Modules

| Module | Stability | Feature | Docs | Tests |
|--------|-----------|---------|------|-------|
| `errors` (ParseError, ParseErrorReason) | **Stable** | — | ✅ | ✅ |
| `ffi` | **Internal** | — | ✅ | ✅ |
| `__private` | **Internal** | — | — | — |
| `sealed` | **Deprecated** | — | ✅ | — |
| `tree_sitter` (compat shim) | Unstable | `pure-rust` | ✅ | ✅ |
| `parser` (→ `parser_v4`) | Unstable | `pure-rust` | ✅ | ✅ |
| `pure_parser` | Unstable | — | ✅ | ✅ |
| `pure_incremental` | Unstable | — | ✅ | ✅ |
| `glr_forest` | Experimental | `pure-rust` | ✅ | ✅ |
| `glr_incremental` | Experimental | `pure-rust` | ✅ | ✅ |
| `glr_parser` | Unstable | — | ✅ | ✅ |
| `glr_lexer` | Unstable | — | ✅ | ✅ |
| `glr_query` | Experimental | — | ✅ | ✅ |
| `glr_tree_bridge` | Unstable | — | ✅ | ✅ |
| `glr_validation` | Unstable | — | ✅ | ✅ |
| `tree_bridge` | Experimental | `pure-rust` | ✅ | ✅ |
| `decoder` | Experimental | `pure-rust` | ✅ | ✅ |
| `document` | Experimental | `pure-rust` | ✅ | ✅ |
| `grammar_json` | Experimental | `pure-rust` | ✅ | ✅ |
| `parser_v4` | Experimental | `pure-rust` | ✅ | ✅ |
| `unified_parser` | Experimental | `pure-rust` | ✅ | — |
| `query` | Experimental | `pure-rust` | ✅ | ✅ |
| `serialization` | Unstable | `serialization` | ✅ | ✅ |
| `visitor` | Unstable | — | ✅ | ✅ |
| `error_recovery` | Unstable | — | ✅ | ✅ |
| `error_reporting` | Unstable | — | ✅ | — |
| `ts_compat` | Experimental | `ts-compat` | ✅ | — |
| `external_scanner` | Unstable | — | ✅ | ✅ |
| `external_scanner_ffi` | **Internal** | — | ✅ | — |
| `scanner_registry` | Unstable | — | ✅ | — |
| `scanners` | Unstable | — | ✅ | — |
| `concurrency_caps` | Unstable | — | ✅ | ✅ |
| `field_tree` | **Internal** | — | ✅ | — |
| `lex` | Unstable | — | ✅ | ✅ |
| `lexer` | Unstable | — | ✅ | ✅ |
| `simd_lexer` | Experimental | — | ✅ | ✅ |
| `linecol` | Unstable | — | ✅ | ✅ |
| `pool` | Unstable | — | ✅ | — |
| `ts_format` | Unstable | — | ✅ | — |
| `arena_allocator` | Unstable | — | ✅ | — |
| `node` | Unstable | — | ✅ | ✅ |
| `subtree` | Unstable | — | ✅ | — |
| `stack_pool` | Unstable | — | ✅ | — |
| `tree_node_data` | Unstable | — | ✅ | — |
| `parser_selection` | Unstable | — | ✅ | ✅ |
| `optimizations` | Unstable | — | ✅ | — |
| `pure_external_scanner` | Unstable | — | ✅ | — |

### Feature-Gated Re-exports

| API | Stability | Feature | Docs |
|-----|-----------|---------|------|
| `Edit`, `GLRToken`, `IncrementalGLRParser` | Experimental | `pure-rust` | ✅ |
| `adze_glr_core` (full crate) | Unstable | `ts-compat` | — |
| `adze_ir` (full crate) | Unstable | `ts-compat` | — |
| `adze_macro::*` (all macros) | **Stable** | — | ✅ |
| `tree_sitter` (external crate) | Deprecated | `tree-sitter-standard` | — |
| `tree_sitter_c2rust` | Deprecated | `tree-sitter-c2rust` | — |

### Deprecated Modules

| Module | Stability | Feature | Replacement |
|--------|-----------|---------|-------------|
| `incremental` | Deprecated | `legacy-parsers` | `glr_incremental` |
| `incremental_v2` | Deprecated | `legacy-parsers` | `glr_incremental` |
| `incremental_v3` | Deprecated | `legacy-parsers` | `glr_incremental` |
| `glr` (legacy) | Deprecated | `legacy-parsers` | `glr_parser` |

### Feature Flags

| Flag | Default | Description |
|------|---------|-------------|
| `pure-rust` | ✅ | Pure-Rust parser backend (WASM-compatible) |
| `glr` | — | Enable GLR parser runtime for ambiguous grammars |
| `serialization` | — | Tree serialization to JSON / S-expressions |
| `simd` | — | SIMD-accelerated lexer |
| `wasm` | — | WASM build support |
| `ts-compat` | — | Tree-sitter compatibility API |
| `legacy-parsers` | — | Access deprecated parser_v2/v3 modules |
| `incremental_glr` | — | GLR incremental parsing support |
| `external_scanners` | — | External scanner support |
| `query` | — | Query predicates support |
| `debug_glr` | — | Debug output for GLR parser |
| `glr_telemetry` | — | GLR telemetry counters |

---

## `adze-macro`

Proc-macro crate providing grammar definition attributes. All macros are pass-through at compile time; actual expansion happens in `adze-common`.

### Proc-Macro Attributes

| Macro | Stability | Docs | Tests |
|-------|-----------|------|-------|
| `#[adze::grammar("name")]` | **Stable** | ✅ | ✅ |
| `#[adze::language]` | **Stable** | ✅ | ✅ |
| `#[adze::leaf(...)]` | **Stable** | ✅ | ✅ |
| `#[adze::extra]` | **Stable** | ✅ | ✅ |
| `#[adze::prec(n)]` | **Stable** | ✅ | ✅ |
| `#[adze::prec_left(n)]` | **Stable** | ✅ | ✅ |
| `#[adze::prec_right(n)]` | **Stable** | ✅ | ✅ |
| `#[adze::delimited(...)]` | **Stable** | ✅ | ✅ |
| `#[adze::repeat(...)]` | **Stable** | ✅ | ✅ |
| `#[adze::skip(value)]` | **Stable** | ✅ | ✅ |
| `#[adze::word]` | Unstable | ✅ | ✅ |
| `#[adze::external]` | Unstable | ✅ | ✅ |

### Feature Flags

| Flag | Default | Description |
|------|---------|-------------|
| `pure-rust` | — | Enables pure-Rust code generation path |
| `strict_docs` | — | Deny missing docs |

---

## `adze-tool`

Build-time code generation. Called from `build.rs` to produce parsers.

### Public Functions

| API | Kind | Stability | Feature | Docs | Tests |
|-----|------|-----------|---------|------|-------|
| `generate_grammars(root_file)` | fn | **Stable** | — | ✅ | ✅ |
| `build_parsers(root_file)` | fn | **Stable** | `build_parsers` | ✅ | ✅ |
| `build_parser(grammar)` | fn | Unstable | — | ✅ | ✅ |
| `build_parser_for_crate(root, opts)` | fn | Unstable | — | ✅ | ✅ |
| `build_parser_from_grammar_js(path)` | fn | Experimental | — | ✅ | — |
| `parse_grammar_js(src)` | fn | Experimental | — | ✅ | — |

### Public Types

| API | Kind | Stability | Docs | Tests |
|-----|------|-----------|------|-------|
| `GrammarConverter` | struct | Unstable | ✅ | ✅ |
| `GrammarVisualizer` | struct | Unstable | ✅ | — |
| `GrammarJsConverter` | struct | Unstable | ✅ | — |
| `BuildOptions` | struct | Unstable | ✅ | ✅ |
| `BuildResult` | struct | Unstable | ✅ | ✅ |
| `ToolError` | enum | **Stable** | ✅ | ✅ |
| `ToolResult` | type alias | **Stable** | ✅ | — |

### Public Modules

| Module | Stability | Docs | Tests |
|--------|-----------|------|-------|
| `visualization` | Unstable | ✅ | — |
| `grammar_js` | Experimental | ✅ | — |
| `pure_rust_builder` | Unstable | ✅ | ✅ |
| `cli` | Unstable | ✅ | — |
| `scanner_build` | Unstable | ✅ | — |
| `error` | **Stable** | ✅ | ✅ |

### Feature Flags

| Flag | Default | Description |
|------|---------|-------------|
| `build_parsers` | ✅ | Enable `build_parsers()` entry point |
| `serialization` | ✅ | Enable serialization support |

---

## `adze-common`

Shared expansion logic and syntax helpers for macro and tool crates.

### Public Types

| API | Kind | Stability | Docs | Tests |
|-----|------|-----------|------|-------|
| `NameValueExpr` | struct | **Stable** | — | ✅ |
| `FieldThenParams` | struct | **Stable** | — | ✅ |

### Public Functions

| API | Kind | Stability | Docs | Tests |
|-----|------|-----------|------|-------|
| `try_extract_inner_type(ty, ident)` | fn | **Stable** | — | ✅ |
| `filter_inner_type(ty, skip_over)` | fn | Unstable | — | ✅ |
| `wrap_leaf_type(ty, skip_over)` | fn | Unstable | — | ✅ |

### Feature Flags

| Flag | Default | Description |
|------|---------|-------------|
| `strict_docs` | — | Enable strict documentation requirements |

---

## `adze-ir`

Grammar Intermediate Representation. Core data structures for representing grammars with GLR support.

### Public Types

| API | Kind | Stability | Docs | Tests |
|-----|------|-----------|------|-------|
| `Grammar` | struct | **Stable** | ✅ | ✅ |
| `Rule` | struct | **Stable** | ✅ | ✅ |
| `Symbol` | enum | **Stable** | ✅ | ✅ |
| `Token` | struct | **Stable** | ✅ | ✅ |
| `TokenPattern` | enum | **Stable** | ✅ | ✅ |
| `SymbolId` | struct (newtype) | **Stable** | ✅ | ✅ |
| `RuleId` | struct (newtype) | **Stable** | ✅ | ✅ |
| `StateId` | struct (newtype) | **Stable** | ✅ | ✅ |
| `FieldId` | struct (newtype) | **Stable** | ✅ | ✅ |
| `ProductionId` | struct (newtype) | **Stable** | ✅ | ✅ |
| `SymbolMetadata` | struct | **Stable** | ✅ | ✅ |
| `PrecedenceKind` | enum | **Stable** | ✅ | — |
| `Associativity` | enum | **Stable** | ✅ | — |
| `Precedence` | struct | **Stable** | ✅ | — |
| `ConflictDeclaration` | struct | Unstable | ✅ | — |
| `ConflictResolution` | enum | Unstable | ✅ | — |
| `ExternalToken` | struct | Unstable | ✅ | — |
| `AliasSequence` | struct | Unstable | ✅ | — |
| `GrammarError` | enum | Unstable | ✅ | ✅ |
| `IrError` | enum | Unstable | ✅ | — |
| `IrResult` | type alias | Unstable | ✅ | — |

### Grammar Methods

| Method | Stability | Docs | Tests |
|--------|-----------|------|-------|
| `Grammar::new(name)` | **Stable** | ✅ | ✅ |
| `Grammar::add_rule(rule)` | **Stable** | ✅ | ✅ |
| `Grammar::get_rules_for_symbol(id)` | **Stable** | ✅ | ✅ |
| `Grammar::all_rules()` | **Stable** | ✅ | ✅ |
| `Grammar::start_symbol()` | Unstable | ✅ | ✅ |
| `Grammar::find_symbol_by_name(name)` | Unstable | ✅ | — |
| `Grammar::get_or_build_registry()` | Unstable | ✅ | — |
| `Grammar::check_empty_terminals()` | Unstable | ✅ | — |
| `Grammar::build_registry()` | Unstable | ✅ | — |
| `Grammar::from_macro_output(data)` | Unstable | ✅ | — |
| `Grammar::validate()` | Unstable | ✅ | ✅ |
| `Grammar::optimize()` | Experimental | ✅ | — |
| `Grammar::normalize()` | Unstable | ✅ | ✅ |

### Re-exported Types

| API | Source | Stability | Docs | Tests |
|-----|--------|-----------|------|-------|
| `GrammarOptimizer` | `optimizer` | Unstable | ✅ | ✅ |
| `OptimizationStats` | `optimizer` | Unstable | ✅ | ✅ |
| `optimize_grammar()` | `optimizer` | Unstable | ✅ | ✅ |
| `GrammarValidator` | `validation` | Unstable | ✅ | ✅ |
| `ValidationError` | `validation` | Unstable | ✅ | ✅ |
| `ValidationResult` | `validation` | Unstable | ✅ | ✅ |
| `ValidationWarning` | `validation` | Unstable | ✅ | ✅ |
| `SymbolInfo` | `symbol_registry` | Unstable | ✅ | — |
| `SymbolRegistry` | `symbol_registry` | Unstable | ✅ | ✅ |

### Public Modules

| Module | Stability | Docs | Tests |
|--------|-----------|------|-------|
| `error` | Unstable | ✅ | — |
| `optimizer` | Unstable | ✅ | ✅ |
| `validation` | Unstable | ✅ | ✅ |
| `debug_macros` | Unstable | ✅ | — |
| `symbol_registry` | Unstable | ✅ | ✅ |
| `builder` | Unstable | ✅ | ✅ |

### Feature Flags

| Flag | Default | Description |
|------|---------|-------------|
| `strict_docs` | — | Strict documentation enforcement |

---

## `adze-glr-core`

GLR parser generation algorithms. Builds LR(1) automata and handles conflict resolution.

### Primary Public Types

| API | Kind | Stability | Docs | Tests |
|-----|------|-----------|------|-------|
| `FirstFollowSets` | struct | **Stable** | ✅ | ✅ |
| `ParseTable` | struct | **Stable** | ✅ | ✅ |
| `ParseRule` | struct | **Stable** | ✅ | ✅ |
| `LRItem` | struct | **Stable** | ✅ | ✅ |
| `ItemSet` | struct | **Stable** | ✅ | ✅ |
| `ItemSetCollection` | struct | **Stable** | ✅ | ✅ |
| `Action` | enum (`#[non_exhaustive]`) | **Stable** | ✅ | ✅ |
| `ActionCell` | type alias | **Stable** | ✅ | — |
| `SymbolMetadata` | struct | **Stable** | ✅ | ✅ |
| `LexMode` | struct | Unstable | ✅ | — |
| `GotoIndexing` | enum | Unstable | ✅ | — |
| `GLRError` | enum | **Stable** | ✅ | ✅ |
| `GlrError` (alias) | type alias | **Stable** | ✅ | — |
| `GlrResult` | type alias | **Stable** | ✅ | — |
| `TableError` | enum | Unstable | ✅ | ✅ |
| `ConflictResolver` | struct | Unstable | ✅ | ✅ |
| `Conflict` | struct | Unstable | ✅ | ✅ |
| `ConflictType` | enum | Unstable | ✅ | ✅ |

### Primary Public Functions

| API | Stability | Docs | Tests |
|-----|-----------|------|-------|
| `build_lr1_automaton(grammar, ff)` | **Stable** | ✅ | ✅ |
| `build_lr1_automaton_res(grammar, ff)` | Unstable | ✅ | — |
| `sanity_check_tables(pt)` | Unstable | ✅ | ✅ |
| `FirstFollowSets::compute(grammar)` | **Stable** | ✅ | ✅ |
| `FirstFollowSets::compute_normalized(grammar)` | **Stable** | ✅ | ✅ |
| `FirstFollowSets::first(symbol)` | **Stable** | ✅ | ✅ |
| `FirstFollowSets::follow(symbol)` | **Stable** | ✅ | ✅ |
| `FirstFollowSets::is_nullable(symbol)` | **Stable** | ✅ | ✅ |
| `FirstFollowSets::first_of_sequence(symbols)` | Unstable | ✅ | ✅ |
| `ConflictResolver::detect_conflicts(...)` | Unstable | ✅ | ✅ |

### Prelude

| API | Stability | Docs |
|-----|-----------|------|
| `prelude::FirstFollowSets` | **Stable** | — |
| `prelude::ParseTable` | **Stable** | — |
| `prelude::build_lr1_automaton` | **Stable** | — |

### `#[doc(hidden)]` Re-exports (Internal)

These are publicly accessible but not part of the stability contract.

| API | Source Module | Stability |
|-----|--------------|-----------|
| `ConflictAnalyzer`, `ConflictStats`, `PrecedenceDecision`, `PrecedenceResolver` | `advanced_conflict` | **Internal** |
| `RuntimeConflictResolver`, `VecWrapperResolver` | `conflict_resolution` | **Internal** |
| `ConflictVisualizer`, `generate_dot_graph` | `conflict_visualizer` | **Internal** |
| `GSSStats`, `GraphStructuredStack`, `StackNode` | `gss` | **Internal** |
| `ForestNode`, `ParseError`, `ParseForest`, `ParseNode`, `ParseTree` | `parse_forest` | **Internal** |
| `ParseTableCache`, `PerfStats`, `StackDeduplicator`, `StackPool` | `perf_optimizations` | **Internal** |
| `PrecedenceComparison`, `PrecedenceInfo`, `StaticPrecedenceResolver`, `compare_precedences` | `precedence_compare` | **Internal** |
| `compare_symbols`, `compare_versions_with_symbols` | `symbol_comparison` | **Internal** |
| `CompareResult`, `VersionInfo`, `compare_versions` | `version_info` | **Internal** |

### Stable Public Modules

| Module | Stability | Docs | Tests |
|--------|-----------|------|-------|
| `driver` (→ `Driver`) | Unstable | ✅ | ✅ |
| `forest_view` (→ `Forest`, `ForestView`, `Span`) | Unstable | ✅ | ✅ |
| `stack` | Unstable | ✅ | ✅ |
| `telemetry` | Unstable | ✅ | — |
| `ts_lexer` | Unstable | ✅ | — |
| `conflict_inspection` | Unstable | ✅ | ✅ |
| `error` | **Stable** | ✅ | — |
| `serialization` | Experimental | `serialization` | ✅ |
| `perf` | Experimental | `perf-counters` | ✅ |

### Macros

| Macro | Stability | Feature | Docs |
|-------|-----------|---------|------|
| `debug_trace!` | **Internal** | `glr-trace` / `debug_glr` | ✅ |
| `glr_trace!` | **Internal** | `glr-trace` / `debug_glr` | ✅ |

### Re-exports from `adze-ir`

| API | Stability |
|-----|-----------|
| `Grammar` | **Stable** |
| `RuleId` | **Stable** |
| `StateId` | **Stable** |
| `SymbolId` | **Stable** |

### Feature Flags

| Flag | Default | Description |
|------|---------|-------------|
| `parallel` | — | Enable rayon-based parallel algorithms |
| `serialization` | — | ParseTable serialization via postcard |
| `perf-counters` | — | Runtime performance counters |
| `glr-trace` | — | Trace macro output for GLR debugging |
| `debug_glr` | — | Alias for `glr-trace` |
| `strict_docs` | — | Strict documentation enforcement |
| `strict_api` | — | Strict API surface checking |

---

## `adze-tablegen`

Table generation and compression. Produces FFI-compatible Language structs.

### Public Types

| API | Kind | Stability | Feature | Docs | Tests |
|-----|------|-----------|---------|------|-------|
| `StaticLanguageGenerator` | struct | Unstable | — | ✅ | ✅ |
| `AbiLanguageBuilder` | struct | Unstable | — | ✅ | ✅ |
| `TableCompressor` | struct | Unstable | — | ✅ | ✅ |
| `CompressedTables` | struct | Unstable | — | ✅ | ✅ |
| `CompressedParseTable` | struct | Unstable | — | ✅ | ✅ |
| `CompressedActionTable` | struct | Unstable | — | ✅ | ✅ |
| `CompressedGotoTable` | struct | Unstable | — | ✅ | ✅ |
| `CompressedActionEntry` | struct | Unstable | — | ✅ | — |
| `CompressedGotoEntry` | struct | Unstable | — | ✅ | — |
| `ActionEntry` | struct | Unstable | — | ✅ | — |
| `GotoEntry` | struct | Unstable | — | ✅ | — |
| `ExternalScannerGenerator` | struct | Unstable | — | ✅ | — |
| `LanguageBuilder` | struct | Unstable | — | ✅ | ✅ |
| `NodeTypesGenerator` | struct | Unstable | — | ✅ | — |
| `LanguageValidator` | struct | **Stable** | — | ✅ | ✅ |
| `ValidationError` (tablegen) | enum | **Stable** | — | ✅ | ✅ |
| `TableGenError` | enum | **Stable** | — | ✅ | ✅ |
| `Result` (type alias) | type | **Stable** | — | ✅ | — |

### Serialization Types (feature-gated)

| API | Kind | Stability | Feature | Docs | Tests |
|-----|------|-----------|---------|------|-------|
| `ParsetableWriter` | struct | Experimental | `serialization` | ✅ | ✅ |
| `ParsetableMetadata` | struct | Experimental | `serialization` | ✅ | ✅ |
| `ParsetableError` | enum | Experimental | `serialization` | ✅ | — |
| `GrammarInfo` | struct | Experimental | `serialization` | ✅ | — |
| `GenerationInfo` | struct | Experimental | `serialization` | ✅ | — |
| `GovernanceMetadata` | struct | Experimental | `serialization` | ✅ | — |
| `ParserFeatureProfileSnapshot` | struct | Experimental | `serialization` | ✅ | — |
| `FeatureFlags` | struct | Experimental | `serialization` | ✅ | — |
| `TableStatistics` | struct | Experimental | `serialization` | ✅ | — |
| `MAGIC_NUMBER` | const | Experimental | `serialization` | ✅ | — |
| `FORMAT_VERSION` | const | Experimental | `serialization` | ✅ | — |
| `METADATA_SCHEMA_VERSION` | const | Experimental | `serialization` | ✅ | — |

### Public Functions

| API | Stability | Docs | Tests |
|-----|-----------|------|-------|
| `StaticLanguageGenerator::new(grammar, table)` | Unstable | ✅ | ✅ |
| `StaticLanguageGenerator::generate_language_code()` | Unstable | ✅ | ✅ |
| `StaticLanguageGenerator::generate_node_types()` | Unstable | ✅ | — |
| `StaticLanguageGenerator::set_start_can_be_empty(bool)` | Unstable | ✅ | — |
| `collect_token_indices(...)` | **Stable** | ✅ | ✅ |
| `eof_accepts_or_reduces(...)` | **Stable** | ✅ | ✅ |

### Public Modules

| Module | Stability | Docs | Tests |
|--------|-----------|------|-------|
| `abi` | Unstable | ✅ | ✅ |
| `abi_builder` | Unstable | ✅ | ✅ |
| `compress` | Unstable | ✅ | ✅ |
| `compression` | Unstable | ✅ | ✅ |
| `error` | **Stable** | ✅ | ✅ |
| `external_scanner` | Unstable | ✅ | — |
| `external_scanner_v2` | Experimental | ✅ | — |
| `generate` | Unstable | ✅ | ✅ |
| `helpers` | **Stable** | ✅ | ✅ |
| `language_gen` | Unstable | ✅ | ✅ |
| `lexer_gen` | Unstable | ✅ | — |
| `node_types` | Unstable | ✅ | — |
| `parser` | Unstable | ✅ | ✅ |
| `parsetable_writer` | Experimental | `serialization` | ✅ |
| `schema` | Unstable | ✅ | ✅ |
| `serializer` | Unstable | ✅ | — |
| `validation` | **Stable** | ✅ | ✅ |

### Test-Only Exports

| API | Kind | Stability | Feature |
|-----|------|-----------|---------|
| `make_empty_table()` | fn | **Internal** | `#[cfg(test)]` |
| `make_minimal_table()` | fn | **Internal** | `#[cfg(test)]` |

### Feature Flags

| Flag | Default | Description |
|------|---------|-------------|
| `tree-sitter-c2rust` | ✅ | c2rust-transpiled TS runtime |
| `serialization` | ✅ | `.parsetable` file generation |
| `tree-sitter-standard` | — | Standard C Tree-sitter runtime |
| `compression` | — | Compression/decompression utilities |
| `small-table` | — | Compressed table generation |
| `incremental_glr` | — | Incremental GLR parsing support |
| `strict_docs` | — | Strict documentation enforcement |
| `strict_api` | — | Strict API surface checking |

---

## Conventions

1. **`#[non_exhaustive]`** is applied to `Action` enum to allow future variants.
2. **Open traits**: `Extract` (and `ExtractDefault`, which inherits it) is intentionally open —
   downstream crates may implement it, and `#[adze::grammar]` relies on that. Every new trait item
   must carry a default; adding a *required* item is a breaking change reserved for a pre-1.0
   breaking release. See [ADZE-ADR-0008](../adr/ADZE-ADR-0008-extract-is-an-open-trait.md).
   `adze-glr-core`'s `ForestView` is genuinely sealed via a `pub(crate)` marker.
3. **`#[doc(hidden)]`** modules in `adze-glr-core` are accessible but carry no stability guarantee.
4. **`#[must_use]`** is applied to validation and computation functions returning `Result`.
5. **All ID newtypes** (`SymbolId`, `RuleId`, `StateId`, `FieldId`, `ProductionId`) derive `Serialize`/`Deserialize` and are stable.

## Related Documents

- [NOW_NEXT_LATER.md](./NOW_NEXT_LATER.md) — Release status and execution plan
- [FRICTION_LOG.md](./FRICTION_LOG.md) — Paper cuts and pain points
- [KNOWN_RED.md](./KNOWN_RED.md) — Known failures and red items
- [PERFORMANCE.md](./PERFORMANCE.md) — Performance benchmarks and targets
