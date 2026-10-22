# Conformance receipt - cfg.nullable_first_follow.v1

- Receipt version: 1
- Schema version: 1
- Corpus digest (sha256, canonical corpus JSON): `c0e4a3302e230c06bde7ea64cfa4a581d3c258733bc1dc001cd920cb99761347`
- Expectation digest: `7323dc7046ed360c361fe1e1fe7c6757a17d8af2edee56043cff0699150f6553`
- Semantic digest: `bac8995e566c77dfd96473106e327743032fe0befe00d93c7360c93db91b90d2`
- Aggregate disposition: **agrees**
- Selected cases (13): single_terminal, direct_epsilon, nullable_prefix, nullable_suffix, fully_nullable_production, nullable_chain, left_recursion, mutual_recursion, repeated_occurrence, terminal_stopping, explicit_start_not_first_declared, unreachable_component, nonproductive_terminal_prefix

## Components

- corpus: {"commit": "748903cc51c1e7bb00bbe9a9e6b72d6acddd27c9", "obtained_via": "git -C <corpus checkout> rev-parse HEAD", "source": "conformance/cases/cfg.nullable_first_follow.v1/corpus.json"}
- python_reference: {"commit": "ca3ed5b9df902cd071bd16274a908524f75223e0", "interpreter": "3.14.6", "interpreter_implementation": "CPython", "obtained_via": "git -C <python reference checkout> rev-parse HEAD", "obtained_via_interpreter": "platform.python_version()"}
- rust_candidate: {"build": "cargo build -p adze-glr-core --example conformance_sets (once, batched request)", "commit": "017bdc93ae2efb5126816a03f1e79daa9a06528a", "driver": "glr-core/examples/conformance_sets.rs", "obtained_via": "git -C <rust driver checkout> rev-parse HEAD"}
- engine/python: {"engine": "adze_reference", "interpreter": "3.14.6", "interpreter_implementation": "CPython", "package_parent": "python"}
- engine/rust: {"binary_sha256": "9b9ade5b2b1e833bd1770947af5c37ac4f2d261882018f14fe3949dedd15df7e", "driver": "glr-core/examples/conformance_sets.rs", "engine": "adze-glr-core", "engine_version": "0.9.0"}

## Counts

- python: selected=13 executed=13 ok=13 failed=0 missing=0
- rust: selected=13 executed=13 ok=13 failed=0 missing=0

## Per-case dispositions

| Case | Disposition | Engine statuses | Detail |
|---|---|---|---|
| single_terminal | agrees | python=ok, rust=ok |  |
| direct_epsilon | agrees | python=ok, rust=ok |  |
| nullable_prefix | agrees | python=ok, rust=ok |  |
| nullable_suffix | agrees | python=ok, rust=ok |  |
| fully_nullable_production | agrees | python=ok, rust=ok |  |
| nullable_chain | agrees | python=ok, rust=ok |  |
| left_recursion | agrees | python=ok, rust=ok |  |
| mutual_recursion | agrees | python=ok, rust=ok |  |
| repeated_occurrence | agrees | python=ok, rust=ok |  |
| terminal_stopping | agrees | python=ok, rust=ok |  |
| explicit_start_not_first_declared | agrees | python=ok, rust=ok |  |
| unreachable_component | agrees | python=ok, rust=ok |  |
| nonproductive_terminal_prefix | agrees | python=ok, rust=ok |  |
