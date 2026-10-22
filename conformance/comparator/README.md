# Conformance comparator and receipt (#977)

Pure standard-library Python (>= 3.11) comparator for the
`cfg.nullable_first_follow.v1` profile. It does not vendor the corpus
(#974), the Python reference (#975), or the Rust driver (#976); all three
component locations are inputs.

## Law

- The full required case set (denominator 13) is selected before either
  engine is invoked. Zero selected cases is a harness failure.
- Expectations are loaded only by the comparator. Engine requests contain
  profile, schema version, and cases without any expectation keys; a
  guard fails harness-side if an expectation key would be sent.
- Each engine owes exactly one terminal row per case
  (`ok` / `invalid_input` / `unsupported` / `engine_error` / `timeout`).
  Only `ok` participates in agreement for required cases.
- Missing rows are `engine_incomplete`; duplicate/unknown rows, malformed
  or partial JSON, and type-confused results are `harness_failure`; wrong
  input digests and stale adapter identities are `identity_mismatch`.
- Majority agreement is not truth: both engines returning the same wrong
  result against the expectation is `both_disagree_with_expectation`.

## Run locally

```bash
export ADZE_CONF_CORPUS=/path/to/conformance/cases/cfg.nullable_first_follow.v1/corpus.json
export ADZE_CONF_PYREF=/path/to/python-reference-checkout   # or its reference/python dir
export ADZE_CONF_RUST_DRIVER=/path/to/rust-driver-checkout

# Tests (fake engines; the end-to-end test skips without the env vars above)
python -m unittest discover -s conformance/comparator/tests -v

# Real retained receipt (builds the Rust example once, one batched request)
python conformance/comparator/comparator.py \
  --corpus "$ADZE_CONF_CORPUS" --pyref "$ADZE_CONF_PYREF" \
  --rust-checkout "$ADZE_CONF_RUST_DRIVER" \
  --out-dir conformance/receipts/cfg.nullable_first_follow.v1
```

The comparator exits 0 only when the aggregate disposition is `agrees`;
the receipt (JSON + Markdown) is written on success and failure either
way. The retained receipt under
`conformance/receipts/cfg.nullable_first_follow.v1/` records each
component's branch commit (`git rev-parse HEAD` in its checkout), the
Python interpreter version, corpus and expectation digests (sha256 of
canonical JSON), per-case dispositions, and per-engine counts. No
timestamps or elapsed times appear in the receipt, so volatile telemetry
cannot enter the semantic digest.

## CI

CI advisory routing is deferred by campaign instruction: this PR adds no
workflow files and stays outside the required `Rust Small Result` gate.
