# Independent Python reference — `cfg.nullable_first_follow.v1`

Pure-Python (standard library only, Python >= 3.11) reference for the
nullable / FIRST / FOLLOW conformance profile, plus a bounded second
observer that challenges it with a different algorithm.

## Profile and spec revision used

- Profile: `cfg.nullable_first_follow.v1`, `schema_version` 1.
- Contract: `ADZE-SPEC-0019` (status: proposed, created 2026-10-20) as held
  on branch `swarm/conformance-profile-974` (PR #1050, unmerged at the time
  of writing), read via `git show FETCH_HEAD:...` without merging it.
- Corpus: `conformance/cases/cfg.nullable_first_follow.v1/corpus.json`
  from the same branch, `required_denominator` 13. The corpus lands with
  #1050; until then corpus-dependent tests skip with an explicit reason
  (see `tests/test_corpus.py`). The reference itself is fully
  self-contained and does not read the corpus at runtime.

## Independence statement

This reference was written from SPEC-0019 (input/output contracts B1/B2,
rejection contract B5, bounds, canonical identity B4) — not by translating
the Rust `FirstFollowBuilder`. The author did not inspect the production
Rust implementation before the first version of this reference, and no
Rust output was used as an expectation. Expectations in tests are either
the hand-reasoned (`reasoned_semantic`) corpus expectations or small
hand-computed expectations written alongside each mutant case. No changes
were made after observing a Rust mismatch, because no Rust run is part of
this lane.

The code never imports, invokes, links to, or falls back to Rust: no
`subprocess`, no FFI, no Cargo/rustc, no network. Reference execution is
subprocess-free. `tests/test_reference.py` enforces this by auditing the
package's import graph against the standard library.

## Layout

```text
reference/python/
  README.md
  adze_reference/
    __init__.py    public API re-exports
    __main__.py    thin runner: python -m adze_reference <request.json>
    profile.py     strict profile validation, typed results
    engine.py      nullable/FIRST/FOLLOW worklist reference
    observer.py    bounded sentential-form enumeration observer
  tests/
    test_reference.py  validation, typed results, reorder invariance,
                       determinism, independence (stdlib-only) audit
    test_corpus.py     all 13 corpus cases + invalid cases (skips until
                       the corpus lands with #1050)
    test_observer.py   observer agreement over its complete envelope,
                       inconclusive states
    test_mutants.py    six faulty variants, each asserted caught
    generated_cases.json  deterministic bounded acyclic cases retained
                       as concrete inputs (seed 975 generator output)
```

## Engine contract

Input is an engine request only (a corpus case minus its expectation).
Requests carrying `expectation`/`expected`-style fields are rejected, as
are unknown fields. Results are typed:

| Status | Meaning |
|---|---|
| `ok` | canonical semantic result |
| `invalid` | malformed document, bad name/case ID, reserved EOF name, duplicate name/production, terminal/nonterminal collision, undefined symbol, unknown start, nonterminal without production, unexpected field |
| `unsupported` | a different `profile` string or `schema_version` |
| `bound_exceeded` | more than 64 symbols, 256 productions, 32 RHS symbols, 4096 total RHS occurrences, or an over-long name/case ID |

Reject codes match the corpus `reject_class` values where the corpus
declares them (`duplicate_production`, `undefined_symbol_reference`,
`missing_or_unknown_start`, `terminal_nonterminal_collision`).

## Representation and algorithm differences from the Rust approach

Per the issue's independence protocol, this reference deliberately does
not mirror production structure:

- Grammar symbols are ordinary Python strings (symbolic names) in plain
  `set`/`dict` structures — no numeric symbol IDs, no bitsets.
- Nullable is a production-counter worklist (remaining non-nullable
  nonterminal occurrences per production, with a reverse dependency
  index); a terminal anywhere in an RHS blocks that production.
- FIRST is a dependency worklist over productions, using a
  first-of-sequence helper that continues only while the prefix is
  nullable. FIRST is over sentential forms, so a nonproductive
  nonterminal can still contribute the terminal prefix before it.
- FOLLOW is a propagation worklist seeded with EOF only on the explicit
  start symbol (never inferred from declaration order). Every occurrence
  of a nonterminal in an RHS is visited, so repeated occurrences and
  nullable suffixes both contribute.
- Output is canonical by construction: every declared nonterminal
  appears exactly once in `first`/`follow` (including unreachable ones),
  and every name list is sorted, so results are byte-for-byte
  deterministic under declaration/production reordering.

## Bounded second observer

`observer.py` enumerates sentential forms breadth-first from the start
symbol and from every nonterminal — a materially different method from
set-equation propagation. From the enumerated forms it reads off
nullable (the empty form is derivable), FIRST (terminals heading a
derivable form), and FOLLOW (terminals/nonterminals that can follow an
occurrence in a start-rooted form, with EOF when the occurrence can
reach the end of the form).

### Envelope and budgets

Default budgets (`ObserverBudgets`): `max_depth` 10 derivation steps,
`max_states` 8192 sentential forms, `max_expansions` 8192 expansions,
`max_form_length` 12 symbols.

An observation is `conclusive` only when every enumeration drains its
queue inside all budgets, no form exceeds the length cap, and every
declared nonterminal is reachable from the start (an unreachable
nonterminal's FOLLOW row is unobservable in start-rooted forms).
Recursive grammars (for example left recursion) normally exhaust a
budget and return `inconclusive`. Exhaustion, pruning, or an unsupported
shape never counts as agreement: `verdict` returns `conclusive_agree`,
`conclusive_disagree`, or `inconclusive` with reasons, and only
`conclusive_agree` is agreement.

## Running locally (no Cargo, rustc, or network needed)

```bash
python --version            # >= 3.11
python -m compileall reference/python
python -m unittest discover -s reference/python/tests
python -m adze_reference <request.json>   # from reference/python/
```

Repeated runs over the same grammar produce byte-identical canonical
JSON (`SemanticResult.canonical_json`), including under declaration and
production reordering; the test suite covers both.
