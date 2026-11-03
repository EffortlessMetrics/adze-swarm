# ci-actuals.json

`ci-actuals.json` is the per-run telemetry artifact that records what the
PR Plan estimated and what the run actually cost. It is the input for
the future learned-estimates work (PR 18).

## What it contains

```json
{
  "schema_version": 1,
  "repo": "EffortlessMetrics/adze",
  "run_id": "12345",
  "event": "pull_request",
  "ref": "refs/pull/.../merge",
  "pr": "493",
  "plan": { "...full ci-plan.json..." },
  "jobs": [
    {
      "name": "Docs Gate",
      "conclusion": "success",
      "labels": ["ubuntu-latest"],
      "runner_multiplier": 1.0,
      "actual_seconds": 12,
      "actual_lem": 0.2,
      "estimated_lem": null,
      "cache_hit": null
    }
  ],
  "status": "ok"
}
```

## How it is produced

The optional `PR Planning Result` job runs `scripts/ci/emit-ci-actuals.py`
after planning and any selected docs-only formatting, on `merge_group` and
manual dispatch. The script:

1. loads `target/ci/ci-plan.json` (downloaded from the `ci-plan` artifact),
2. queries the GitHub Actions API for the current run's jobs,
3. computes `actual_seconds` from `started_at`/`completed_at`,
4. multiplies by the runner-label multiplier (Linux 1×, Windows 2×,
   macOS 10×, …),
5. writes `target/ci/ci-actuals.json`,
6. uploads the file as the `ci-actuals` artifact.

If the API is unreachable, the token is missing, or the plan artifact is
absent, the script falls back to `"status": "degraded"` and emits a
minimal, valid JSON document. The build is never failed by this step.

## Scope after supported-proof deduplication (#1027)

The artifact name and schema stay unchanged. Current `pr-gate.yml` actuals
contain planning, selected `Docs Gate`, and reporting jobs. The duplicate
`Supported Rust Gate` no longer runs here; the required `Supported Proof`
job in `em-ci-routed-rust.yml` owns that execution.

Historical artifacts may contain `Supported Rust Gate` and `PR Gate Success`.
Current artifacts use `PR Planning Result`. The consumer records the API's
actual job names and does not require either historical name. External
consumers of those names must account for this boundary.

These are per-workflow actuals. They do not include the separate routed Rust
or Product Proof runs and must not be presented as total qualification cost.
The plan remains a broader forecast; a missing job-cost match stays `null`.
Compare historical and current costs by job/obligation, not by assuming the
workflow retained the old executed denominator.

## Why this is enough for now

Until at least 30 days of `ci-actuals.json` artifacts have accumulated,
static `base_lem` from `policy/ci-lane-whitelist.toml` is the better
estimate. The script's purpose today is to *gather* the data; using it
to drive estimates is PR 18 in the rollout.

## Privacy

`ci-actuals.json` only includes job names, runner labels, durations, and
cache-step success. No diff, source, or token material is included.
