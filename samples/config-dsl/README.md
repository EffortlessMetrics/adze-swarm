# A typed configuration DSL

This example turns a small service retry policy into ordinary Rust values:

```text
retries = 3;
targets = [api, worker];
timeout = 30;
```

`Policy` owns a `u16`, a `Vec<String>` and an `Option<Timeout>`. Its annotated
fields are the grammar: there is no hand-written tree-to-configuration mapper.
The language deliberately has fixed field order and is not TOML or a general
configuration format. Targets are nonempty ASCII identifiers. The optional
timeout follows the target list. A `#` comment ends at the next line ending.

## Qualification boundary

This is a source-path, non-published downstream fixture under issues #1129 and
#882. Its new feature composition and nine tests still require Rust execution
before acceptance. It does not establish a registry install, a release, or a
support-tier promotion. The isolated workspace keeps it out of the release graph
and out of default workspace test selection.

## Run from the repository root

Use the repository's declared Rust toolchain. These commands are the intended
qualification steps, not a receipt that they have already passed:

```sh
cargo test --manifest-path samples/config-dsl/Cargo.toml
cargo run --quiet --manifest-path samples/config-dsl/Cargo.toml -- samples/config-dsl/fixtures/valid.policy
```

The expected standard output is exactly:

```text
retries=3
targets=api,worker
timeout_seconds=30
```

With `fixtures/overflow.policy`, the program must emit a diagnostic on standard
error, emit no success output, and exit 1. Missing/extra arguments or an unreadable
file exit 2. Input paths are accepted as operating-system strings.

## Read the implementation

`src/lib.rs` contains the complete annotated grammar. `build.rs` generates the
parser during a normal Cargo build. `src/main.rs` reads one file, calls
`grammar::parse`, consumes the typed result, and renders source-aware errors.
`tests/contract.rs` checks exact values and ordering, optional-field meaning,
numeric boundaries, overflow spans, malformed inputs, comments/line endings and
the actual binary's output/exit behavior.

Both numeric leaves use `try_transform`. A matching digit token can still exceed
`u16`; that must return a structured conversion error, not panic or become zero.
Missing timeout and `timeout=0;` have different meanings and are tested separately.

## Change, break, and repair

Start by changing the fixture to five retries, three targets and no timeout. The
expected values are `5`, `[api, worker, scheduler]`, and `None`; add that assertion
to `tests/contract.rs` before changing the grammar. Then deliberately add a trailing
comma to the target list. The parser must reject it, and removing the comma must
restore the same values. These are data-edit exercises, not evidence of grammar-
change compatibility.

The next #1129 slice adds a tested grammar-extension walkthrough and runs this
application through the installed-package harness. Do not replace these tests
with accepted error snapshots if the current parser cannot handle a valid case;
retain the witness and repair its owning grammar/extraction issue.
