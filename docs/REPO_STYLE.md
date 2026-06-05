# Repo style

Adze-swarm is operated as an evidence machine.

Rust and `xtask` are the default construction material. Non-Rust files,
unsafe, panic paths, lint suppressions, generated files, workflow behavior,
process/network access, expensive CI lanes, and release claims must be owned
and receipted.

Static evidence runs first:

- `cargo-allow` for source exceptions;
- `ripr` for static mutation-exposure analysis;
- `unsafe-review` for unsafe-contract review;
- rustc and Clippy for code-shape policy.

Runtime evidence runs where it pays:

- focused tests on PRs;
- targeted mutation for risk PRs;
- broader mutation, Miri, fuzzing, and coverage on nightly and release lanes.

CI is designed for proof per Linux-equivalent minute (LEM). Default PRs are
cheap, deterministic, and high-signal. Deep validation is preserved, but routed
by risk pack, label, main, nightly, or release.

Agents work one review-fast PR at a time. Review-fast does not mean tiny; it
means coherent seam, nearby proof, efficient verification, and honest claim
boundary. Do not broaden scope to satisfy CI. Do not add invisible exceptions.

## Tool-role split

| Tool | Role | Does not do |
| --- | --- | --- |
| `cargo-allow` | Durable source-exception ledger for syntax-visible retained exceptions. | Replace unsafe reviewability, Miri, or behavior proof. |
| `ripr` | Static mutation-exposure analysis for PR-time weak-oracle signal. | Run mutants, report killed/survived outcomes, or replace runtime mutation. |
| `unsafe-review` | Unsafe-contract reviewability: safety contract, guard, test reach, and witness route. | Prove memory safety without a concrete witness receipt. |
| `xtask` | Repo control plane: wrappers, receipts, reports, CI planning, release readiness, and policy glue. | Reimplement specialized upstream tools. |
| `cargo-mutants` | Runtime mutation backstop for targeted, nightly, and release lanes. | Serve as a cheap default check for every PR. |
| Miri | Concrete UB execution witness for selected code paths. | Prove all unsafe code sound by default. |
| Codecov | Execution-surface telemetry. | Prove correctness, mutation adequacy, or release readiness. |

## Exception rule

There are no invisible source exceptions. Retained exceptions need an owner,
reason, coverage or evidence pointer, review date, and expiry when temporary.
`cargo-allow` is the preferred durable ledger for source-tree exceptions;
specialized ledgers remain only where they add semantics that the source ledger
cannot express.

## CI economics rule

We are not reducing CI because we want less verification. We are reducing
wasted CI so we can afford more verification where it matters. Optional deep
lanes are allowed to be expensive only when their trigger, proof value, and
claim boundary are explicit.

## Agent rule

The good path should be easiest:

```text
change code
run one command
see exception diffs
see weak-oracle gaps
see unsafe review cards
add focused proof
keep receipts
merge when green
```

If a change needs a new exception, broadens a public claim, changes CI spend, or
alters unsafe behavior, the PR must say what it proves, what it does not prove,
and which receipt carries the claim.

## Related operating docs

- [`docs/reference/SPEC_SYSTEM.md`](reference/SPEC_SYSTEM.md) defines the source-of-truth stack.
- [`docs/reference/adze-swarm-operating-model.md`](reference/adze-swarm-operating-model.md) defines the swarm/public repo boundary.
- [`docs/ci/tooling-substrate.md`](ci/tooling-substrate.md) defines the upstream tool substrate and `xtask` wrapper model.
- [`docs/ci/cost-and-verification-policy.md`](ci/cost-and-verification-policy.md) defines CI economics.
- [`docs/ci/ripr.md`](ci/ripr.md) defines the static mutation-exposure lane.
- [`docs/ci/unsafe-review.md`](ci/unsafe-review.md) defines the unsafe-contract review lane.
