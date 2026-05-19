# Product Gap Burn-Down

Status: complete
Owner: runtime/product
Created: 2026-05-19
Active goal: ../../.adze/goals/active.toml

This lane owns the next execution queue after the completed toolkit excellence
and release-promotion readiness campaigns. It burns down the remaining blockers
named in `docs/status/PRODUCT_OBJECTIVE_AUDIT.md` without broadening public
claims.

Start with [`implementation-plan.md`](./implementation-plan.md). The closeout is
recorded in [`closeout.md`](./closeout.md).

## Boundaries

- Work in `EffortlessMetrics/adze-swarm`.
- Keep public `EffortlessMetrics/adze` as release/public-intake until an
  explicit promotion PR opens.
- Do not claim `cargo install adze-cli` until a crates.io install receipt
  exists.
- Do not treat `ci-product-stable` as a required branch-protection gate until
  policy records that promotion.
