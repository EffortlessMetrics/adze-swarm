# Visualizing GLR Conflicts

Adze's current GLR visualization support is conflict-level, not a stable
runtime trace API.

Use this guide when you need to inspect why a grammar produced shift/reduce or
reduce/reduce conflicts during table generation. Runtime fork/merge tracing and
full forest visualization are future work.

## Current Scope

Supported today:

- text reports for conflict lists;
- optional item-set detail for the conflicting LR state;
- DOT graphs for parse automaton states;
- red highlighting for conflict states in generated DOT output.

Not currently a stable user API:

- step-by-step runtime stack tracing;
- partial parse tree visualization during parsing;
- full GLR forest visualization;
- production profiling of fork and merge behavior.

## Conflict Reports

The low-level conflict visualizer lives in `adze-glr-core`:

```rust
use adze_glr_core::conflict_visualizer::ConflictVisualizer;

let visualizer = ConflictVisualizer::new(&grammar, &conflicts);
let report = visualizer.generate_report();

println!("{report}");
```

The report includes:

- total conflict count;
- shift/reduce conflict count;
- reduce/reduce conflict count;
- state ID for each conflict;
- lookahead symbol for each conflict;
- shift and reduce actions involved in each conflict.

When item sets are available, attach them for more context:

```rust
let visualizer = ConflictVisualizer::new(&grammar, &conflicts)
    .with_item_sets(&item_sets);

let report = visualizer.generate_report();
```

That adds the LR items involved in the conflict state. This is usually the most
useful view when deciding whether to add precedence, refactor a grammar rule, or
accept the ambiguity and rely on GLR behavior.

## DOT Automaton Graphs

For a graph-level view of the LR automaton:

```rust
use adze_glr_core::conflict_visualizer::generate_dot_graph;

let dot = generate_dot_graph(&item_sets, &conflicts, &grammar);
std::fs::write("parse_automaton.dot", dot)?;
```

Render the graph with Graphviz:

```bash
dot -Tpng parse_automaton.dot -o parse_automaton.png
```

The DOT output is an automaton visualization. It is not a selected parse tree
and it is not a GLR forest export.

## Interpreting Conflicts

### Shift/Reduce

A shift/reduce conflict means the parser can either consume the lookahead token
or reduce an existing rule at the same state.

Typical causes:

- expression precedence or associativity;
- optional suffixes;
- dangling-else style grammar shape;
- list separators with ambiguous trailing forms.

### Reduce/Reduce

A reduce/reduce conflict means two or more completed productions can reduce on
the same lookahead.

Typical causes:

- overlapping rules with the same visible shape;
- hidden or wrapper rules that erase useful distinction;
- grammar aliases that need clearer ownership;
- intentionally ambiguous language constructs.

## Debugging Workflow

1. Generate or collect the grammar conflicts.
2. Print a conflict report.
3. If the report is too shallow, add item-set detail.
4. Generate the DOT automaton graph only when state relationships matter.
5. Decide whether the ambiguity is accidental or intentional.
6. If intentional, add a GLR fixture that proves the selected tree and ambiguity
   summary behavior.

## How This Relates to `AdzeDocument`

Conflict visualization explains table-generation facts. It does not replace the
runtime parse product.

Runtime users should inspect GLR behavior through:

```rust
let report = grammar::parse_document(source);
let document = report.document();

let ambiguities = document.ambiguities();
let selected_tree = document.root();
```

That is the user-facing path for selected-tree behavior and ambiguity summaries.
The conflict visualizer is lower-level tooling for grammar and table debugging.

## Proof Commands

The conflict visualizer is covered by focused `adze-glr-core` tests:

```bash
cargo test -p adze-glr-core --test conflict_visualizer_tests --features test-api
cargo test -p adze-glr-core --test conflict_visualizer_comprehensive
git diff --check
```

## Known Gaps

These remain future work:

- document-backed runtime trace export;
- full GLR forest visualization;
- source-range-aware ambiguity diagrams;
- CLI command for conflict reports;
- stable public visualization API.
