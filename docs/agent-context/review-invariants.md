# Adze Review Invariants

Invariants that must hold in all code submitted to this repository.

## Type System
1. **Extract trait usage**: All grammar types implement `Extract` correctly
2. **No orphaned derives**: Macro derives are paired with type definitions
3. **MSRV compliance**: Code compiles with Rust 1.95.0 (check `rust-version` in Cargo.toml)
4. **Edition consistency**: All crates use Rust 2024 edition

## Parser Behavior
1. **No panics on untrusted input**: Parser returns Result, never panics
2. **Error recovery**: Parse failures produce usable error messages with location info
3. **Incremental safety**: Fresh parsing and incremental parsing produce identical results
4. **Table correctness**: Generated parse tables match Tree-sitter ABI exactly

## Memory Safety
1. **Unsafe boundaries**: All `unsafe` blocks have documented invariants
2. **No memory leaks**: Resources held by Parser/Extract are properly released
3. **Bounds checking**: Array/slice access is checked
4. **FFI correctness**: Language struct layout matches C ABI requirements

## Testing Coverage
1. **Snapshot consistency**: New snapshots reflect intended grammar changes
2. **Error path coverage**: Tests verify both success and failure modes
3. **Golden parity**: adze-golden-tests validates Tree-sitter compatibility
4. **Feature matrix**: Feature combinations tested via `scripts/test-matrix.sh`

## Workspace Hygiene
1. **Unused imports removed**: No `#[allow(unused)]` suppression
2. **Workspace deps preferred**: Use `[workspace]` dependency versions
3. **No direct semver**: Pin exact versions or use workspace specifications
4. **Doc comments**: Public items have doc comments; internal details are inline

## Performance Assumptions
1. **Compile-time overhead**: Build time must not regress >5% on workspace
2. **Runtime parsing**: Parser performance on Python/Go/JavaScript grammars maintained
3. **Memory allocation**: GLR forest must not exceed ~2x input size for typical code
4. **Test runtime**: `just test` completes in <60s on 4-core machine

## CI Compliance
1. **Hosted PR gate pass**: `Rust Small Result` is green for `adze-swarm` PRs
2. **Local supported proof**: `just ci-supported` runs clean when the change
   affects supported/release-facing surfaces
3. **No clippy warnings**: All core crates pass `cargo clippy -D warnings`
4. **Format compliance**: All code passes `cargo fmt --all --check`
5. **Policy compliance**: No new panics without allowlist exception

## PR Metadata
1. **Title clarity**: PR title describes the change, not the result
2. **Description completeness**: Linked issues, test validation, migration notes (if applicable)
3. **Commit messages**: Imperative mood, reference issues/PRs where relevant
4. **Validation evidence**: Test runs, snapshot reviews, or operational smoke tests

## Documentation
1. **AGENTS.md current**: Updates to tooling reflected in AGENTS.md
2. **CLAUDE.md accurate**: Architecture changes documented
3. **Snapshot intent clear**: Snapshot diffs have explanatory commits
4. **Example grammars valid**: Grammar changes validated with `example/` tests
