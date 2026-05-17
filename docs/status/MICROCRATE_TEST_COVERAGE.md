# Microcrate Test Coverage Analysis

**Generated:** 2026-03-26
**Last Updated:** 2026-05-16
**Total Crates:** 5

## Summary

| Category | Count | Percentage |
|----------|-------|------------|
| Workspace support crates with BDD + Property | 4 | 100% |
| Durable support crates with contract locks | 4 | 100% |
| Excluded harness crates | 1 | 100% classified |

The remaining workspace support surfaces have BDD/property coverage where
applicable. The package-boundary release gate is now the source of truth for
whether any temporary microcrate remains.

## Workspace Coverage (BDD + Property Tests)

All 4 remaining workspace support crates have BDD tests and property-based
tests:

| Crate | BDD File | Property File | Contract Lock |
|-------|----------|---------------|---------------|
| `bdd-governance-core` | ✓ | ✓ | ✓ |
| `common-type-ops-core` | ✓ | ✓ | ✓ |
| `linecol-core` | ✓ | ✓ | ✓ |
| `parsetable-metadata` | ✓ | ✓ | ✓ |

### Excluded Harness

| Crate | BDD File | Property File | Contract Lock |
|-------|----------|---------------|---------------|
| `ts-c-harness` | - | - | excluded harness |

## Contract Lock Files

The following durable support crates have `contract_lock.rs` test files
(contract verification):

- `bdd-governance-core`
- `common-type-ops-core`
- `linecol-core`
- `parsetable-metadata`

### Crates Without Contract Lock Tests

The following crates do not have contract lock tests (by design):

- `ts-c-harness` - FFI test harness (excluded from workspace)

## Test Coverage Milestones

| Date | Milestone |
|------|-----------|
| 2026-03-26 | Initial coverage analysis (20 complete, 23 partial, 4 missing) |
| 2026-03-27 | **100% BDD + Property coverage achieved** - All 47 crates now have both test types |
| 2026-03-27 | Contract lock tests expanded to 45+ crates |

## Overlapping Responsibilities Analysis

### Potential Consolidation Opportunities

1. **Runtime Governance Crates:**
   The runtime governance facade/matrix stack has been collapsed into runtime owner modules and `bdd-governance-core::runtime`.

2. **Concurrency Init Crates:**
   Standalone concurrency crates have been collapsed. Classifier and bootstrap
   helpers now live as SRP owner submodules under the runtime surface.

## Documentation Status

All crates have proper module-level documentation (`//!` comments) except:

| Crate | Status |
|-------|--------|
| `ts-c-harness` | Missing documentation |

This is acceptable as `ts-c-harness` is an FFI test harness (excluded from workspace).

## Next Steps

1. ✅ ~~Add property tests to high-priority crates missing them~~ - **COMPLETE**
2. ✅ ~~Add BDD + Property tests to crates with no coverage~~ - **COMPLETE**
3. ✅ ~~Review overlapping crates for potential consolidation~~ - **COMPLETE for 0.9 package-boundary release gate**
4. ✅ Documentation check complete - all workspace crates documented
5. Keep release-facing support crate claims mapped through `SUPPORT_TIERS.md`
   before promoting any product behavior.
