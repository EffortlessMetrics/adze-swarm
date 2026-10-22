# Merge-queue admission (#1026)

Cheap, validate-before-spend admission for the merge queue. Enqueue is the
authorized decision this record produces; live queue enablement is owned by
#1027/#1028. Implemented as `cargo xtask queue-admission` in `xtask`.

## H / G / M lifecycle

- **H** — exact PR source head (full SHA) that was reviewed and locally
  validated. The admission record binds H. Nothing infers H from
  `github.sha`, a queue-ref suffix, commit dates, or commit counts.
- **G** — actual GitHub integration composition being qualified. G may
  differ from H while H is unchanged; the record stores G separately
  (`integration_sha`) when provided. Multi-PR compositions (a group listing
  more than one PR) are initially **refused** with an actionable reason.
- **M** — actual protected squash commit subsequently landed. M is produced
  by landing, not by this record; passing G conditionally authorizes landing
  under the pre-queue decision, with no routine second manual approval.

## Issuing admission

```sh
cargo xtask queue-admission emit \
  --repository EffortlessMetrics/adze-swarm --pr 42 \
  --head <full-40-hex-H> --scope-issue 1026 --acceptance-ref issue-1026 \
  --proof-command "cargo test -p xtask --bin xtask queue_admission" \
  --proof-summary "17 passed" --proof-ref target/receipt.json \
  --reviewer <reviewer> --verdict approved --findings-ref <review-ref> \
  --requester <maintainer> --out target/queue-admission/pr-42.json
```

Use `--proof-file <path>` to record the receipt file's SHA-256 digest, and
`--integration-sha <G>` / `--group-pr <n>` when G is known. The JSON record
is deterministic (canonical field order); the volatile `decided_at`
timestamp is excluded from `record_digest`.

## Validate before spending

```sh
cargo xtask queue-admission validate \
  --record target/queue-admission/pr-42.json --live target/live-pr-42.json
```

`--live` is a fixture/seam JSON (`repository`, `pr_number`, `head_sha`,
`is_draft`, `state`, `is_fork`, `labels`). Tests run offline against
fixtures — they never call the network or `gh`. Labels are observed only
and are never authorization. Validation fails closed and prints typed
refusals (`head_mismatch`, `pr_draft`, `pr_closed`, `fork_refused`,
`proof_missing`, `review_missing`, `scope_missing`,
`multi_pr_composition_unsupported`, `record_digest_mismatch`, …), each with
the next permitted action.

## Revoking

```sh
cargo xtask queue-admission revoke --record <record> \
  --reason "new blocking finding at unchanged H" [--withdraw]
```

Revocation updates the decision state and digest; a prior clean review
never suppresses it — validation of a revoked/withdrawn record refuses.
Repair, re-review, and emit a new record to re-enter.

## Boundaries

No candidate-script execution, no token handling, and no live queue/worker
state stored in the record. Policy is passed as data, never interpolated
into a shell. This tool does not enable the live queue — that is #1028.
