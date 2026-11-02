# Cookbook

This cookbook answers common user tasks with complete, compile-tested
examples. Every recipe below is a runnable grammar plus tests in the
`examples/cookbook` crate — there are no prose-only snippets here, so the
code cannot drift from its proof. The task-oriented reference with full
recipe text lives in the [parser cookbook reference](https://github.com/EffortlessMetrics/adze-swarm/blob/main/docs/reference/parser-cookbook.md).

Start with the [Quick Start](getting-started/quickstart.md) if you have not
built a first parser yet; the cookbook links back to it instead of redoing
the tutorial path.

## Available now

| Task | Recipe (crate module) | Proof test |
| --- | --- | --- |
| Arithmetic with typed values and precedence, plus `parse()` versus `parse_document()` | `arithmetic` | `cookbook_arithmetic_*` |
| CSV-like delimited repetition | `csv` | `cookbook_csv_*` |
| Object-like grammar with named fields, repetition, keywords versus identifiers, whitespace extras | `config` | `cookbook_config_*` |
| Fallible numeric conversion with structured failure | `fallible` | `cookbook_fallible_*` |

Each recipe states its prerequisites, support tier, expected output,
limitations, and exact proof command in its module header. Support tiers
for every surface live in `docs/status/SUPPORT_TIERS.md`.

Run the proof:

```bash
cargo test --manifest-path examples/cookbook/Cargo.toml -- --test-threads=1 --nocapture
cargo test --manifest-path examples/cookbook/Cargo.toml --features serialization -- --test-threads=1 --nocapture
```

## Follow-ups (owned by [#882](https://github.com/EffortlessMetrics/adze-swarm/issues/882))

Remaining required recipes stay out until their behavior is proved:
optional-field syntax, comment extras, richer leaf patterns, empty delimited
lists, ambiguity summaries, ts-compat selected trees, document-backed
queries, external scanners, and performance receipts. The reference page's
"Not Yet Promoted" table records each blocker with its failing-input
evidence.
