# ADR-0007: Runtime2 GLR Integration with Examples

**Status**: Accepted
**Date**: 2025-11-19
**Context**: Phase 3.3 Integration Testing
**Related**: Phase 3.2 Complete, PHASE_3.3_INTEGRATION_TESTING.md

---

Current support-tier note: this ADR records a historical runtime2 integration
decision. `runtime2/` is now treated as an experimental proving ground rather
than the public-primary runtime contract; see
[`docs/status/SUPPORT_TIERS.md`](../status/SUPPORT_TIERS.md).

## Context

During Phase 3.3 implementation (GLR integration testing with example grammars), we discovered an architectural consideration regarding the relationship between `runtime` and `runtime2` packages.

### Current Architecture

**Two Runtime Packages**:
- `runtime/` - Original runtime crate (used by example grammars)
- `runtime2/` - GLR proving-ground runtime from Phase 3.1-3.2 work

**Example Dependencies**:
```toml
# example/Cargo.toml
[dependencies]
adze = { path = "../runtime", default-features = false }
```

**Problem**: Example grammars depend on `runtime`, but GLR implementation lives in `runtime2`.

---

## Decision

### Phase 3.3 Approach

We will proceed with Phase 3.3 using **runtime2** directly for GLR testing, with the following strategy:

#### Option A: Create Runtime2-Specific Examples (CHOSEN)

**Rationale**:
1. **Isolation**: Keep runtime2 GLR development independent
2. **Testing**: Can test GLR without affecting existing examples
3. **Migration Path**: Allows gradual transition
4. **Safety**: No risk of breaking existing functionality

**Implementation**:
1. Create `runtime2/examples/` directory
2. Port key grammars (arithmetic, ambiguous_expr, dangling_else)
3. Write GLR-specific tests in runtime2/tests/
4. Document runtime2 examples separately

#### Option B: Merge Runtime2 into Runtime (FUTURE)

Once runtime2 is fully stable and validated:
1. Merge runtime2 GLR features into runtime
2. Update example dependencies
3. Deprecate runtime2 as separate package
4. **Timeline**: Post-Phase 3.4 (not Phase 3.3)

---

## Consequences

### Positive

✅ **Clear Separation**: GLR testing isolated in runtime2
✅ **No Regressions**: Existing examples unchanged
✅ **Fast Iteration**: Can modify runtime2 freely
✅ **Better Testing**: GLR-specific test infrastructure
✅ **Documentation**: Clear examples for each runtime

### Negative

⚠️ **Duplication**: Some example code duplicated
⚠️ **Maintenance**: Two sets of examples to maintain
⚠️ **User Confusion**: Two runtimes might confuse users

### Mitigations

**Documentation**:
- Clear README in runtime2/examples/
- CLAUDE.md explains runtime vs runtime2
- Migration guide for when they merge

**Timeline**:
- Phase 3.3: runtime2 examples and tests
- Phase 3.4: Documentation and stabilization
- Phase 4+: Consider runtime merge

---

## Implementation Plan

### Phase 3.3 Immediate Actions

1. ✅ Create `runtime2/examples/` directory structure
2. ✅ Port arithmetic grammar to runtime2/examples/
3. ✅ Port ambiguous_expr grammar to runtime2/examples/
4. ✅ Port dangling_else grammar to runtime2/examples/
5. ✅ Create runtime2-specific integration tests
6. ✅ Update PHASE_3.3_INTEGRATION_TESTING.md with approach

### File Structure

```
runtime2/
├── examples/
│   ├── README.md                    # Explains GLR examples
│   ├── arithmetic.rs                # Unambiguous with precedence
│   ├── ambiguous_expr.rs            # Ambiguous expression
│   └── dangling_else.rs             # Classic ambiguity
├── tests/
│   ├── phase_3_3_e2e_integration_test.rs  # E2E scenarios
│   ├── glr_lr_parity_test.rs        # Parity testing
│   └── glr_performance_test.rs      # Performance benchmarks
└── benches/
    └── glr_benchmarks.rs            # Criterion benchmarks
```

### Testing Strategy

**runtime2 Tests**:
- Phase 3.2 completion tests (62 passing)
- Phase 3.3 integration tests (new)
- GLR-specific functionality
- Performance baselines

**example Tests** (unchanged):
- Continue testing with `runtime`
- No changes needed for Phase 3.3
- Will migrate post-merge

---

## Alternatives Considered

### Alt 1: Update Example to Use Runtime2

**Rejected Because**:
- Breaks existing tests
- Risky for Phase 3.3
- No clear rollback

### Alt 2: Backport GLR to Runtime

**Rejected Because**:
- Runtime2 was the focused GLR proving-ground runtime for this phase
- Would duplicate work
- Runtime2 has better architecture

### Alt 3: Feature Flag in Runtime

**Rejected Because**:
- Complex conditional compilation
- Hard to maintain two implementations
- Merge is cleaner long-term

---

## References

- [PHASE_3_PURE_RUST_GLR_RUNTIME.md](../specs/PHASE_3_PURE_RUST_GLR_RUNTIME.md)
- [PHASE_3.3_INTEGRATION_TESTING.md](../specs/PHASE_3.3_INTEGRATION_TESTING.md)
- [CLAUDE.md](../../CLAUDE.md) - Runtime2 architecture

---

## Status

**Current**: Implementing Phase 3.3 with runtime2 examples
**Next**: Create runtime2/examples/ directory and port grammars
**Future**: Merge runtime2 into runtime post-stabilization

---

**Decision**: Proceed with runtime2-specific examples for Phase 3.3
**Approved**: 2025-11-19
**Review Date**: Post-Phase 3.4
