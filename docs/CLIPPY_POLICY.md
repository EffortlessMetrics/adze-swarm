# Clippy Policy

This document describes the lint-policy stack for the Adze workspace. The
authoritative manifest is [`policy/clippy-lints.toml`](../policy/clippy-lints.toml);
this file explains what it means and how to extend it.

## Why a policy stack

Clippy is fast and visible in the editor, but it cannot carry the full
governance receipt for an exception (owner, reason, expiry, location identity).
We therefore use a **dual-rail** model:

```
Clippy            -> catches bad code shapes locally and in CI.
xtask checks      -> own the exception ledger (owner / reason / expiry / selector).
```

The two rails complement each other. Clippy denies a shape; the xtask checker
records the receipt for the few legitimate exceptions.

## Lanes

| Lane                              | Purpose                                                   | Source                              |
| --------------------------------- | --------------------------------------------------------- | ----------------------------------- |
| `[workspace.lints]` in `Cargo.toml` | Active Clippy/Rust lints applied to every workspace member | `Cargo.toml`                       |
| `policy/clippy-lints.toml`        | Manifest of active + staged + planned lints               | this repo                           |
| `cargo xtask check-lint-policy`   | Verifies inheritance and consistency                      | `xtask/src/policy/lint_policy.rs`   |
| `cargo xtask check-no-panic-family` | Owns receipted exceptions to panic-family lints          | `xtask/src/policy/no_panic.rs`      |

## Suppression style

Bare `#[allow(...)]` is forbidden. The supported style is:

```rust
#[expect(clippy::unwrap_used, reason = "policy:no-panic:panic-0042")]
let value = lookup().unwrap();
```

The reason must reference either a no-panic allowlist id (`policy:no-panic:<id>`)
or a documented invariant (`invariant: …`). Any other reason should be reviewed
before merging.

`clippy.toml` test carveouts (`allow-unwrap-in-tests`, `allow-expect-in-tests`,
`allow-panic-in-tests`, etc.) are **not allowed** anywhere in the workspace.

`cargo xtask check-lint-policy` records the current bare-allow migration debt
in `target/policy/lint-policy.md`. That receipt is advisory: it shows the
path-prefix breakdown, top files, and samples so cleanup can happen in small
reviewable batches before any blocking enforcement is enabled.

## Staged lints

Several lints are gated on debt cleanup. They live in
`policy/clippy-lints.toml` under `[staged.clippy]`:

```
unwrap_used, expect_used, panic, todo, unimplemented, unreachable,
get_unwrap, unwrap_in_result, indexing_slicing, string_slice, dbg_macro
```

These are currently **warn** in `[workspace.lints]` (where applied) and
**advisory** in the semantic checker. The staged plan is:

1. Inventory existing debt with `cargo xtask no-panic-propose --baseline`.
2. Receipt or fix every finding.
3. Promote the lint to `deny` in `[workspace.lints]`.

## Planned lints

`[[planned]]` entries name lints that we intend to enable when the MSRV bumps.
The lint-policy checker fails if any planned lint is activated before its
`activate_when_msrv` window.

## Adding a new active lint

1. Decide if the lint is a hard ban or a quality warning.
2. Add it to `[workspace.lints]` in the root `Cargo.toml` at the chosen level.
3. Mirror the entry in `policy/clippy-lints.toml` under `[active.clippy]` with
   the same level.
4. Run `cargo xtask check-lint-policy` to confirm everything stays in sync.

## Adding a temporary exception

Prefer fixing the code. If you must suppress, use:

```rust
#[expect(clippy::indexing_slicing, reason = "invariant: bounds checked above")]
```

If the suppression is panic-family adjacent, also add a `policy/no-panic-allowlist.toml`
entry (see [`docs/NO_PANIC_POLICY.md`](NO_PANIC_POLICY.md)).
