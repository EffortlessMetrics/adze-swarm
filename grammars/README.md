# Adze Grammar Crates

The crates in this directory are reference grammars and integration fixtures.
They are useful for exercising Adze's generated-parser pipeline, GLR behavior,
external scanner shape, and Tree-sitter compatibility work, but they are not
stable bundled language packages unless `docs/status/SUPPORT_TIERS.md` promotes
a specific crate and proof lane.

The stable product path remains user-defined Rust grammar types that generate a
pure-Rust parser and return typed Rust values through the main `adze` runtime.
For the user-facing support boundary, see
[`docs/reference/language-support.md`](../docs/reference/language-support.md)
and [`docs/reference/known-limitations.md`](../docs/reference/known-limitations.md).

## Current Roles

| Crate | Role | Stability boundary |
|-------|------|--------------------|
| `adze-python` | External-scanner and indentation-sensitive fixture. | Advisory; external scanner API is experimental. |
| `adze-javascript` | Larger grammar and GLR/golden-test fixture. | Advisory/stabilizing fixture; not full JavaScript ecosystem parity. |
| `adze-go` | Standard grammar-shape smoke fixture. | Advisory; see `grammars/go/STATUS.md` for current blockers. |
| `adze-python-simple` | Small Python-like subset for parser experiments. | Advisory fixture only. |
| `test-vec-wrapper` | Tiny parser smoke and recovery canary. | Canary-level proof; see `grammars/test-vec-wrapper/SMOKE_STATUS.md`. |

## Claim Rules

- Do not describe a grammar crate as production-ready or stable unless the
  support-tier ledger names that exact crate and proof command.
- Do not use these crates to claim full Tree-sitter imported grammar parity.
- Keep status files honest about skipped assertions, permissive parse behavior,
  and known blockers.
- Prefer adding focused fixture proof before expanding README or release claims.

## Local Proof Examples

Run focused crate checks when changing a grammar:

```bash
cargo test -p adze-go
cargo test -p adze-javascript
cargo test -p adze-python
cargo test -p adze-python-simple
cargo test -p test-vec-wrapper
```

If the change affects the stable public path, also run the stable product and
supported gates named by the current active campaign or PR template.
