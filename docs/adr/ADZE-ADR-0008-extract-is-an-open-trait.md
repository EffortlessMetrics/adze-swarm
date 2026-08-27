# ADZE-ADR-0008: `Extract` Is an Open Trait

Status: accepted
Date: 2026-08-27
Owner: runtime/api
Linked proposal: ../proposals/ADZE-PROP-0002-api-foundation.md
Linked issue: https://github.com/EffortlessMetrics/adze-swarm/issues/865

## Context

`adze::Extract` carried this doc comment:

> This trait is sealed and cannot be implemented outside this crate, allowing
> us to add new methods in the future without breaking changes.

The supertrait that was supposed to enforce that claim was
`adze::sealed::Sealed`, declared in a **public** module with a blanket
implementation:

```rust
pub mod sealed {
    pub trait Sealed {}
    impl<T> Sealed for T {}
}
```

Every type in every crate already satisfied `Sealed`, so the bound prevented
nothing. `docs/status/API_STABILITY.md` repeated the claim ("Sealed traits:
`Extract` requires `sealed::Sealed`, preventing external implementations"),
and two runtime tests asserted the blanket impl exists — that is, they proved
the trait was *not* sealed while the docs said it was.

The blanket impl was not an oversight. `#[adze::grammar]` expands in the
**caller's** crate and emits `impl Extract<..> for TheirType`. A genuine seal
would reject exactly that expansion, so the blanket impl was added to keep
generated code compiling. The type system and the documentation had therefore
disagreed since the marker was introduced.

`adze-glr-core` shows the pattern that does work: `ForestView: sealed::Sealed`
with `pub(crate) mod sealed`. Downstream crates cannot name that marker, so
that trait is genuinely sealed. `Extract` cannot adopt the same shape without
breaking macro-generated downstream implementations.

## Decision

`Extract` is **intentionally open**. Downstream crates may implement it, both
through `#[adze::grammar]` expansion and by hand.

1. The "sealed and cannot be implemented outside this crate" wording is
   removed, along with the false compatibility guarantee it implied.
2. The `sealed::Sealed` supertrait bound is removed from `Extract`. It
   enforced nothing, so removing it rejects no implementation that compiled
   before.
3. `adze::sealed` remains, deprecated, so existing `T: adze::sealed::Sealed`
   bounds keep compiling. It is scheduled for removal in a later pre-1.0
   release.
4. **Trait-evolution rule.** Because downstream implementations are supported,
   every new item added to `Extract` must carry a default — as
   `HAS_CONFLICTS`, `GRAMMAR_NAME`, and `GRAMMAR_JSON` already do. Adding a
   *required* item is a breaking change and may only land in a pre-1.0
   breaking release. It may never land in a patch or minor release.

The same rule governs `ExtractDefault`, which inherits `Extract` and made no
independent sealing claim.

## Consequences

- `runtime/tests/extract_open_contract.rs` compiles as its own crate against
  `adze` and implements `Extract` for a locally declared type. It is the
  compile-pass proof of this decision: if `Extract` is ever genuinely sealed,
  that test stops building and this ADR must be revisited first.
- `docs/status/API_STABILITY.md` records `Extract` as open and states the
  evolution rule instead of the sealing claim.
- `adze::sealed` is `Internal` and deprecated. Naming it emits a deprecation
  warning; the module is a no-op marker with no stability guarantee.
- Adding a required method to `Extract` is now visibly a breaking change
  rather than something the (false) seal appeared to permit.

## Alternatives considered

**Genuinely seal `Extract` (make `sealed` `pub(crate)`).** Rejected: it breaks
`#[adze::grammar]`, whose whole purpose is to emit `Extract` implementations
in downstream crates. Workarounds that keep the macro working — re-exporting
the marker under `#[doc(hidden)]`, or a token type the macro passes through —
leave the marker nameable and so leave the trait open in practice, restoring
the same documentation/behavior mismatch this ADR removes.

**Delete `adze::sealed` outright.** Rejected for this release: it is a public
module, and any downstream `T: adze::sealed::Sealed` bound would stop
compiling. Deprecating it now and removing it in a later pre-1.0 release gives
that break a visible warning first.

## Non-Goals

- No change to `Extract`'s method signatures, associated types, or constants.
- No support-tier promotion. `Extract` keeps its existing stability row.
- No 1.0 declaration or permanent API freeze.
- No change to `adze-glr-core`'s `ForestView`, which is genuinely sealed and
  stays that way.
