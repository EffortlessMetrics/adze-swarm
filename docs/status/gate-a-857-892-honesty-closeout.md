# Gate A #857 / #892 Honesty Closeout Receipt

Status: partial — docs/catalog alignment only
Owner: runtime/product
Recorded: 2026-07-25
Base commit: `66c2d894` (`#966` merged on main)
Parent campaign: #853
Active goal: `../../.adze/goals/active.toml`

## Purpose

Record what `adze-swarm/main` proves after the #964/#966 matrix receipts without
claiming #857, #892, or Gate A epic closure. This is a source-of-truth honesty
pass only; it does not implement runtime fixes or close GitHub issues.

## Landed On Main (claimed)

| Surface | Receipt | PR |
| --- | --- | --- |
| `ambiguous_expr` AST/JSON projection equivalence | `projection_equivalence::glr_ambiguous_expr_parse_document_ast_and_json_agree` | #964 |
| `reduce_reduce` AST/JSON ambiguity agreement | `projection_equivalence::glr_reduce_reduce_document_ambiguity_matches_json` | #964 |
| `dangling_else` AST/JSON projection equivalence | `projection_equivalence::glr_dangling_else_parse_document_ast_and_json_agree` | #964 |
| `streaming_lex_modes` AST/JSON projection equivalence | `projection_equivalence::glr_streaming_lex_modes_parse_document_ast_and_json_agree` | #964 |
| `ambiguous_expr` ts-compat selected-tree projection | `projection_equivalence::glr_ambiguous_expr_tree_sitter`, `glr_ambiguous_expr_parser_parse` | #966 |
| `reduce_reduce` ts-compat selected-tree projection | `projection_equivalence::glr_reduce_reduce_tree_sitter` | #966 |
| Fixture catalog honesty | `tests/fixtures/catalog.toml` rows linked to proved commands; stale fixed-bridge gaps retired | #966 + this receipt |

## Explicitly Deferred (not claimed)

| Item | Tracking | Why still open |
| --- | --- | --- |
| Dangling-else nearest-else typed AST structure | #877 | `generated_dangling_else_selects_nearest_else_and_records_ambiguity` remains `#[ignore]` |
| Dangling-else ts-compat selected-tree row | #877 / #892 | No `Tree::from_document` / `Parser::parse` parity canary for `dangling_else` |
| GLR conflict-fixture determinism receipts | #853 / #877 | Not part of this docs pass |
| Full serialization matrix across all GLR conflict fixtures | #892 | Partial AST/JSON rows only |
| #857 production routing / divergent-stack lexical commitment | #857 / #928 / #891 | Streaming route advanced; epic closeout criteria not met |
| #892 old-bridge removal / full equivalence matrix | #892 | Bridge removal landed; matrix and support-tier alignment incomplete |
| #874 first-hour user journey | #874 | Still blocked on #857 closeout per active goal |
| #856 Linux full isolated-registry receipt | #856 | Harness merged; scheduled/manual execution pending |
| GLR conflict routing Stable promotion | support tiers | Remains **Stabilizing** with explicit non-claims |

## Issues Not Closed

Do **not** close #857, #892, #877, or #853 from this receipt. Acceptance for
those issues is not met on main.

## Fresh Receipts

```bash
python -c "import tomllib; tomllib.load(open('tests/fixtures/catalog.toml', 'rb')); tomllib.load(open('.adze/goals/active.toml', 'rb'))"
cargo run -q -p xtask -- check-active-goal --mode blocking
cargo run -q -p xtask -- check-doc-artifacts --mode blocking
git diff --check
```

## Recommended Next After Merge

1. **#877** — un-ignore or replace dangling-else nearest-else typed AST proof; add ts-compat selected-tree row when lexer/matrix criteria are met.
2. **#856** — dispatch Linux full local-registry receipt when workflow lane is available.
3. **#874** — remain blocked until #857 closeout criteria are recorded in #853.
