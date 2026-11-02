# No-Panic Policy

The Adze workspace targets **no unreceipted panic-family behavior in
production or tests**. Panic-family means any call shape that can abort the
process: `unwrap`, `expect`, `panic!`, `todo!`, `unimplemented!`, `unreachable!`,
unchecked indexing, byte-boundary string slicing, `Result::unwrap` on
`Result`-returning functions, and similar.

Test assertion macros (`assert!`, `assert_eq!`, `assert_ne!`) are **not**
panic-family for v1. They are oracles. We may revisit this once a fallible
assertion helper exists.

## The ledger

Exceptions are recorded in [`policy/no-panic-allowlist.toml`](../policy/no-panic-allowlist.toml).
Identity is `(path, family, kind, container, callee, receiver_fingerprint)`.
Line and column drift do not invalidate an entry; every identity field must
match, including the presence or absence of a receiver fingerprint.

Each entry must specify:

```toml
[[allow]]
id = "panic-0001"
path = "crates/example/src/parser.rs"
family = "unwrap"
classification = "test_helper"
owner = "parser"
explanation = "Fixture setup; migrate to fallible fixture builder."
expires = "2026-07-01"

[allow.selector]
kind = "method_call"
container = "parses_boundary_fixture"
callee = "unwrap"
receiver_fingerprint = "std::fs::read_to_string(path)"

[allow.last_seen]
line = 42
column = 17
```

`last_seen` is advisory only. The checker updates it via
`cargo xtask no-panic-propose --update-last-seen` (planned).

## Workflow

1. **Inspect debt.** Run:
   ```bash
   cargo xtask check-no-panic-family
   ```
   The checker reports unallowlisted findings, expired entries, and stale
   entries (allowlist entries with no matching code).

2. **Propose receipts.** For new debt, run:
   ```bash
   cargo xtask no-panic-propose
   ```
   This writes `target/policy/no-panic-proposed.toml`. **It never edits
   `policy/no-panic-allowlist.toml` automatically.** Review the proposal,
   set owner/reason/expires, and copy entries in by hand.

3. **Burn down.** Cleanup happens by replacing the panic-family call with a
   `Result`-returning equivalent. Update or remove the allowlist entry once
   the call is gone.

## Classifications

| Classification     | Meaning                                                         |
| ------------------ | --------------------------------------------------------------- |
| `test_helper`      | Setup/teardown plumbing inside a test module.                   |
| `fixture_setup`    | Deterministic fixture construction, e.g. `Path::new`.           |
| `invariant`        | Genuinely unreachable; documented invariant nearby.             |
| `external_contract`| Forced by an external API that does not return `Result`.        |
| `legacy`           | Pre-existing debt; must have an `expires` date.                 |
| `placeholder`      | `todo!` / `unimplemented!` for in-flight work.                  |
| `governance`       | Required by a policy framework or proc macro contract.          |

`legacy` entries must expire within 6 months. `placeholder` entries must
expire within 3 months.

## What the checker does

`cargo xtask check-no-panic-family` walks the workspace, parses Rust source
with `syn`, finds panic-family calls, builds a selector for each, and
matches against the allowlist. It emits Markdown + JSON reports in
`target/policy/`:

- `target/policy/no-panic.md` — human-readable summary.
- `target/policy/no-panic.json` — machine-readable findings.

The default **advisory** mode reports panic-family debt without enforcing
allowlist coverage. Every mode fails on selected unreadable/unparseable files or an
empty production-file denominator. `no-panic-propose` applies these same
integrity checks before writing its proposal, including with `--baseline`.
Proposals require individual review and never update the allowlist.

Directory traversal errors are not yet included in the failed-file report.
The integrity checks cover files the walker selected; they do not prove that
every unreadable directory was visited. Completing that coverage remains part
of the scanner work in #867.

Production prefixes come from published packages marked `production_use` in
`policy/package-boundary.toml`. Counts classify files: conventional test,
benchmark, and example paths are separate, but inline `#[cfg(test)]` modules
retain their containing file's scope. The production-finding count therefore
does not mean every reported call is compiled into a production artifact.

Return-context classification is syntactic. An explicit function or closure
return type mentioning `Result` selects `unwrap_in_result`; a closure never
inherits its enclosing function's return type. Inferred closure results are
unknown and remain ordinary `unwrap` findings. The scanner does not resolve
types or prove that replacing any particular call with `?` would compile.

## Exit modes

| Mode                              | Description                                  |
| --------------------------------- | -------------------------------------------- |
| `advisory` (current default)      | Report debt; fail on scan-integrity errors. |
| `blocking-allowlist`              | Also fail on unallowlisted findings; freshness is reported. |
| `blocking-strict`                 | Also fail on unallowlisted, stale, or expired entries. |

The mode is chosen via `--mode` on the command line.
