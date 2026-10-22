# Symbol Normalization in adze

## Overview

Symbol normalization is a preprocessing step in the adze grammar pipeline.
It rewrites complex grammar symbols (`Optional`, `Repeat`, `RepeatOne`,
`Choice`, `Sequence`) into productions that only contain `Terminal`,
`NonTerminal`, `External`, and `Epsilon` symbols, which is the form the
FIRST/FOLLOW computation and LR item generation consume.

This page describes the implementation as it exists today. Claims are
labelled by evidence state: **current** behavior is implemented and tested;
**known defect** entries are tracked bugs with reproduction witnesses, not
guarantees.

## How Normalization Works

`Grammar::normalize(&mut self) -> Vec<Rule>` (in `ir/src/lib.rs`) rewrites
rules in repeated passes until no complex symbols remain, so nested
constructs such as `Optional(Repeat(x))` are expanded one layer per pass.
It returns the grammar's full rule list after rewriting.

| Construct | Rewrite |
|---|---|
| `Optional(inner)` | New helper rule: `aux -> inner` and `aux -> ` (empty) |
| `Repeat(inner)` | New helper rule: `aux -> aux inner` (left-recursive) and `aux -> ` (empty) |
| `RepeatOne(inner)` | New helper rule: `aux -> aux inner` and `aux -> inner` |
| `Choice([a, b, ...])` | New helper rule with one production per alternative |
| `Sequence([a, b, ...])` | Flattened inline into the parent production; no helper rule |

The parent production keeps its position: the complex symbol is replaced
by `NonTerminal(aux)` (or, for `Sequence`, by the flattened children).

### Helper symbol IDs (known defect)

**Current:** helper IDs start at `max_rule_lhs + 1000` and increment. The
maximum is computed over rule left-hand sides only, and the arithmetic is
unchecked.

**Known defect:** a token, external, or metadata symbol above the maximum
rule LHS can therefore collide with a helper ID, and a grammar whose
maximum rule LHS is within 1000 of `u16::MAX` overflows (panic in debug
builds, wraparound in release builds, potentially onto the reserved EOF
ID `SymbolId(0)`). Tracked in
[#978](https://github.com/EffortlessMetrics/adze-swarm/issues/978) with
reproduction witnesses; a full-domain allocation repair is in review but
not yet landed. Until then, do not rely on helper IDs being disjoint from
the rest of the symbol space, and do not use the `id > 1000` band as an
auxiliary-symbol heuristic in new code.

### Rule names and production identity

Normalization does **not** add entries to `Grammar::rule_names` for helper
symbols, and every helper rule is created with `ProductionId(0)`; helper
productions do not get unique production IDs today.

`ProductionId` is a grammar-local production slot that distinct rules may
share. `RuleId` is the separate dense index of a rule in
`Grammar::all_rules()` order, and `Grammar::production_ids` is the
`RuleId -> ProductionId` map. On the builder path that map is currently
left empty, so it is not yet an authoritative total map; giving it that
contract is tracked in
[#980](https://github.com/EffortlessMetrics/adze-swarm/issues/980).

### Positional metadata (known defect)

**Current:** `Sequence` flattening changes the arity of a production, and
positional metadata such as `Rule.fields` is copied unchanged.

**Known defect:** a field that designated the child after a flattened
sequence silently designates a different child afterwards. Tracked in
[#981](https://github.com/EffortlessMetrics/adze-swarm/issues/981); an
origin-map repair is in review but not yet landed.

## API Usage

Normalization is infallible by signature; it does not return a `Result`.

```rust
use adze_ir::Grammar;

let mut grammar = create_grammar_with_complex_symbols();
let all_rules: Vec<adze_ir::Rule> = grammar.normalize();
```

FIRST/FOLLOW entry points in `adze-glr-core`:

- `FirstFollowSets::compute(&Grammar)` clones the grammar and normalizes
  the clone; the caller's grammar is unchanged.
- `FirstFollowSets::compute_normalized(&mut Grammar)` normalizes the
  caller's grammar in place first.

Both return `Result<FirstFollowSets, GLRError>` for set-computation
failures (for example the checked-capacity failures guarded by
[#982](https://github.com/EffortlessMetrics/adze-swarm/issues/982)).

### Idempotency

Normalization is idempotent: once no complex symbols remain, a second
call allocates no new helpers and returns the same rules. This is covered
by the `adze-ir` normalization suites.

## Testing

```bash
# Normalization behavior (adze-ir)
cargo test -p adze-ir normalize

# FIRST/FOLLOW integration (adze-glr-core)
cargo test -p adze-glr-core first_follow

# This page's accuracy contract
cargo test -p adze-ir --test normalization_doc_contract
```

## Error Handling

`Grammar::normalize(&mut self) -> Vec<Rule>` cannot fail by signature.
The one failure mode today is the helper-ID overflow described above,
which panics (debug) or wraps (release) instead of returning an error;
[#978](https://github.com/EffortlessMetrics/adze-swarm/issues/978) tracks
making that a typed error. `FirstFollowSets::compute(&Grammar)` and
`compute_normalized(&mut Grammar)` return typed `GLRError`s for
set-computation problems.

## Performance Considerations

- Each complex symbol creates one helper rule with one to three
  productions (`Choice` creates one production per alternative).
- More rules mean larger parse tables, with equivalent accepted language.
- Normalization runs at grammar-processing time, not per parse.

## Compatibility Notes

- Grammars with only simple symbols are unchanged by normalization.
- No identical-behavior guarantee is proved beyond the tested rewrites
  above; in particular the helper-ID collision and positional-metadata
  defects mean normalized output must not be assumed collision-free until
  [#978](https://github.com/EffortlessMetrics/adze-swarm/issues/978) and
  [#981](https://github.com/EffortlessMetrics/adze-swarm/issues/981) land.
