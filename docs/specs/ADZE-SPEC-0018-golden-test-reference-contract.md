# ADZE-SPEC-0018: golden-test reference contract

Status: accepted
Owner: grammar/fixtures
Created: 2026-06-22
Linked proposal: ../proposals/ADZE-PROP-0003-glr-toolkit-productization.md
Linked ADRs: ../adr/ADZE-ADR-0006-user-defined-grammar-is-stable-product.md
Linked plan: ../../plans/glr-toolkit/productization-plan.md
Linked issues: #783
Support-tier impact: ../status/SUPPORT_TIERS.md

## Problem

The golden-test harness (`golden-tests/`) validates that the adze parser produces
expected parse trees for fixture inputs. However, there was no spec defining the
reference format (S-expression shape), the generation procedure, or the refresh
policy. This led to stale tree-sitter-format `.sexp` files that could never match
adze parser output (#783, fixed by deleting them in #818).

## Behavior

### B1. Golden-test references use adze parse-tree format

Reference `.sexp` files must reflect the **adze parser's actual output shape**:
camelCase node types (e.g. `Program`, `FunctionDeclaration`, `NumberLiteral`),
no `[row, col]` position annotations, and the structural nesting the adze
pure-rust parser produces.

Tree-sitter-format references (snake_case node types, position annotations) are
**invalid** and must not be checked in. The `generate_references.sh` script
(which imports upstream tree-sitter output) must not be used to generate adze
references.

### B2. Strict vs non-strict mode

- `run_golden_test_strict`: the test **fails** if the parse tree does not match
  the reference. Use this for canary fixtures that prove the parser works.
- `run_golden_test` (non-strict): the test **soft-skips** (prints "Skipping..."
  and returns `Ok(())`) if parsing fails. Use this only for fixtures where the
  grammar is known-incomplete and the reference is aspirational.

Non-strict tests that always soft-skip provide no proof value. They should be
either promoted to strict (once the grammar can parse the fixture) or removed.

### B3. Reference generation procedure

To generate a correct adze-format reference:
1. Write the fixture input (`.js`, `.py`, etc.)
2. Run the adze parser on it
3. Capture the S-expression output from the adze parser (NOT tree-sitter)
4. Verify the output manually
5. Commit the `.sexp` + `.sha256`

### B4. Non-Goals

This spec does not:
- Require golden tests for every grammar (they are Advisory per SUPPORT_TIERS.md).
- Define the S-expression serialization format (that is an implementation detail
  of the `Forest`/`ParsedNode` display).
- Change the `golden-tests` crate's tier (it remains Advisory).

## Acceptance examples

| Scenario | Expected |
|---|---|
| `.sexp` file uses camelCase node types | valid (adze format) |
| `.sexp` file uses snake_case + `[row, col]` | invalid (tree-sitter format) — must be deleted or regenerated |
| Test uses `run_golden_test_strict` | fails on mismatch |
| Test uses `run_golden_test` (non-strict) | soft-skips on parse failure |
| Non-strict test always soft-skips | should be promoted or removed |

## Test mapping

| Behavior | Proof |
|---|---|
| B1 (adze format) | The canary `.sexp` uses camelCase types |
| B2 (strict/non-strict) | `run_golden_test_strict` is used for the canary |

## CI Proof

Golden tests are Advisory (per SUPPORT_TIERS.md:70). The `golden-tests.yml`
workflow runs them on path-routed PRs. They are not a required gate.
