# unsafe-review advisory

`unsafe-review` is advisory unsafe-contract review. It checks whether changed
unsafe seams have reviewable evidence: a safety contract, a local guard, test
reach, and a witness route.

It does not prove memory safety or UB-free status unless a matching witness
receipt is attached. Miri, sanitizers, fuzzing, and targeted tests remain the
execution-backed witnesses for concrete paths.

## Tool boundary

| Tool | Question answered |
| --- | --- |
| `cargo-allow` | Is this unsafe or source exception allowed, owned, and receipted? |
| `unsafe-review` | Is this unsafe seam reviewable: contract, guard, test reach, and witness route? |
| Miri / sanitizers | Did a concrete execution expose UB or memory misuse? |

Keep those planes separate. A ledger entry can say an unsafe block is allowed;
it cannot by itself show that the unsafe contract is reviewable or that a
concrete UB witness passed.

## Review card expectations

An unsafe-review card should make the seam legible to reviewers without asking
them to reverse-engineer invariants from raw pointers or layout code. The card
should identify:

- the unsafe location and enclosing API;
- the safety contract the caller or callee must uphold;
- the local guard or validation that narrows the unsafe preconditions;
- tests or examples that reach the seam;
- an optional witness route such as Miri, sanitizer, fuzz, or a targeted test;
- any `cargo-allow` or policy ledger ID that owns the retained exception.

## Artifacts

Expected advisory artifacts live under `target/unsafe-review/` when the tool is
available:

```text
target/unsafe-review/
  cards.json
  pr-summary.md
  github-summary.md
  cards.sarif
  comment-plan.json
  witness-plan.md
  lsp.json
  receipt-audit.json
```

When the tool is not available, the wrapper should emit a skipped advisory
receipt rather than blocking ordinary PRs during rollout.

## Suppressions and witnesses

| File | Purpose |
| --- | --- |
| `policy/unsafe-review.toml` | Repo policy for unsafe review cards when enabled. |
| `policy/unsafe-review-suppressions.toml` | Owned, expiring advisory suppressions. |
| `policy/unsafe-witnesses.toml` | Durable witness routes for unsafe seams when enabled. |

A suppression should be rare, owned, dated, and reviewable. It should not hide a
new unsafe seam that lacks a safety contract.

## PR behavior

`unsafe-review` belongs on PRs that touch unsafe, FFI, native ABI, raw pointer,
layout-sensitive parser/table code, GPU/native integration, or related guard
logic. It is advisory first. A later policy may require evidence or a waiver for
changed unsafe seams, but that should happen only after the baseline is clean
and the wrapper emits stable receipts.

## Claim boundary

A green unsafe-review card means the unsafe seam is reviewable. It does not mean
that every unsafe execution is sound. Only cite UB-free or memory-safety claims
when the PR also includes the matching execution witness receipt.

## Related

- [`docs/REPO_STYLE.md`](../REPO_STYLE.md) for the evidence-machine doctrine.
- [`docs/ci/tooling-substrate.md`](tooling-substrate.md) for the upstream tool and `xtask` wrapper split.
- [`policy/ripr-suppressions.toml`](../../policy/ripr-suppressions.toml) as an example of advisory suppression ownership.
