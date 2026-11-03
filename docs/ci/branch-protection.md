# Branch protection

## Today

`adze-swarm` branch protection requires two aggregate GitHub status checks:

```text
Rust Small Result
Product Proof Result
```

`Rust Small Result` is emitted by `.github/workflows/em-ci-routed-rust.yml`
after the routed Rust Small lane runs on CPX42, CX43, CX33, or explicit
fallback. CX53 is logged but not selected for this required base route while
`adze-swarm#598` remains blocked.

`Product Proof Result` is emitted by `.github/workflows/product-proof.yml`.
It is always present on Product Proof PR events, passes when no Stable product
surface changed, and fails when Stable product canaries are selected but do not
pass.

`just ci-supported` remains the local supported/product proof command. It is
not the required GitHub branch-protection context in `adze-swarm`.

`Product Proof Result` was promoted only after the burn-in criteria below were
met by merged PR receipts and this policy update changed `.github/settings.yml`,
this file, and [CI_LANES.md](../../.github/CI_LANES.md) together.

Conversation resolution is intentionally disabled in `adze-swarm`. Review bots
can still leave useful comments, but unresolved advisory threads must not block
the single-operator swarm merge path after the required gates are green.

## Promotion History And Future Changes

### Product Proof Result promotion

`Product Proof Result` is now the required context for Stable README claim
proof. It was added alongside `Rust Small Result`; it did not replace the Rust
base gate.

Promotion was gated on:

| Criterion | Target |
| --- | --- |
| Distinct merged PRs with `Product Proof Result` present and green | >= 5 |
| Receipts where `ci-product stable canaries` were selected and green | >= 2 |
| Receipts where Stable canaries skipped with an explicit reason | >= 2 |
| Unexplained `Product Proof Result` flakes | 0 open |
| Product-audit wording updated from advisory to required | same PR as settings change |

The promotion PR only:

1. added `Product Proof Result` to `.github/settings.yml` required contexts;
2. updated [CI_LANES.md](../../.github/CI_LANES.md),
   [KNOWN_RED.md](../status/KNOWN_RED.md), and
   [PRODUCT_OBJECTIVE_AUDIT.md](../status/PRODUCT_OBJECTIVE_AUDIT.md); and
3. recorded rollback to `Rust Small Result` only.

### Optional PR planning result (#1027)

`.github/workflows/pr-gate.yml` now emits **PR Planning Result** on
`merge_group` and manual dispatch. It retains `PR Plan`, selected docs-only
`cargo fmt --all -- --check`, and `ci-actuals` telemetry. The duplicate
`Supported Rust Gate` is retired; the existing required routed workflow
continues to execute supported proof on the integration composition.

The optional result requires a successful plan and an exact `docs_only`
output. `true` requires a successful Docs Gate; `false` requires that job
to be skipped. Missing, malformed, failed, or cancelled prerequisites do
not pass. This result describes planning and selected formatting only.

`Rust Small Result` and `Product Proof Result` remain the two required
contexts. Neither their producer jobs nor `.github/settings.yml` changes
as part of this retirement. An external consumer of the historical
`PR Gate Success` name must migrate to the appropriate current obligation;
the planning result is not a replacement Rust qualification check.

## Retired PR 17 promotion criteria

The public-era proposal to promote `PR Gate Success` used this burn-in table:

| Criterion | Target |
| --- | --- |
| `PR Gate Success` job has run on every PR for | ≥ 14 calendar days |
| `PR Gate Success` flake rate | < 1% |
| Number of distinct PRs that exercised both `Supported Rust Gate` and `Docs Gate` paths | ≥ 5 each |
| `ci-actuals.json` artifacts uploaded | ≥ 30 PRs |
| Manual review of band/LEM accuracy | passes |

This table is retained as historical rollout context. It cannot authorize
promotion of `PR Planning Result`, whose proof scope is deliberately
different. Any future required-check change needs its own reviewed contract
and cutover evidence; planning success does not replace required Rust or
Product Proof.

## Rollback

If a future required-check promotion causes problems, the rollback is to
remove `Product Proof Result` from `.github/settings.yml`, restore branch
protection to require only `Rust Small Result`, and update this file plus
[CI_LANES.md](../../.github/CI_LANES.md), [KNOWN_RED.md](../status/KNOWN_RED.md),
and [PRODUCT_OBJECTIVE_AUDIT.md](../status/PRODUCT_OBJECTIVE_AUDIT.md) to match.
