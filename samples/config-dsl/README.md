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

This is a source-path, non-published downstream fixture under issues #1129,
#1138 and #882. Its feature composition, nine application tests, one edit-anchor
check and one explicitly selected edit/debug journey are **authored, not yet
Rust-qualified**. It does not establish a registry install, a release, or a
support-tier promotion. Its isolated workspace is outside the release graph
and default root-workspace test selection.

## Run from the repository root

Use the repository's declared Rust toolchain. These are qualification commands,
not receipts that they have already passed:

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

## Change the data

Try five retries, three targets and no timeout. The expected values are `5`,
`[api, worker, scheduler]`, and `None`. Add a trailing comma to the target list;
the parser must reject it. Removing the comma must restore the same values.
These are data changes, not changes to the grammar.

## Extend, break, and repair the grammar

The separate journey in `tests/edit_debug.rs` copies this sample's actual grammar
and build script into a temporary downstream project. It never edits your sample.
Run it explicitly because it invokes Cargo four times:

```sh
cargo test --manifest-path samples/config-dsl/Cargo.toml --test edit_debug -- --ignored --exact grammar_edit_break_repair --nocapture
```

An ordinary `cargo test` **does not execute this journey**. An ignored test is
not evidence that the walkthrough works.

| Phase | Change | Required result |
| --- | --- | --- |
| Baseline | Use the original grammar. | The old policy retains exact values; a `backoff=7;` clause is rejected. |
| Extend | Add `pub backoff: Option<Backoff>` after `timeout` and the annotated `Backoff` type. | Backoff values `0`, `7` and `65535` are exact; absence remains `None`; old inputs still work; `65536` produces a ranged conversion error. |
| Break | Change the new field's type to `Option<MissingBackoffType1138>`. | Cargo fails with a diagnostic naming that missing type; an old successful test binary cannot qualify. |
| Repair | Restore `Option<Backoff>`. | The extension assertions pass again without deleting the target directory. |

`FIELD_ANCHOR`, `BACKOFF_TYPE` and `EXTENSION_TEST` in that test are the exact
editable source and expected-value contract; this guide does not maintain a second
copy of the grammar. Each successful phase must execute the named
`journey_contract` test, not merely report a successful Cargo exit. The same
owned target directory is reused across all phases. The test also checks that the
original source/build script and an unrelated file remain unchanged.

For a manual exercise, make those edits in your own copy and add the new typed
assertions before accepting the change. The sample CLI is intentionally unchanged
by the automated journey: it demonstrates the new AST field through tests, not a
new printed field.

## Remaining proof

The journey still needs actual Rust execution, formatting/lint checks and selected
Windows/Linux runs. Installed-package execution belongs to the existing package
harness; these checkout-path dependencies are not package-install evidence.

Do not replace valid-input assertions with accepted error snapshots, discard a
failing grammar construct, or turn overflow into a default value to make the
sample look green. Retain the witness and repair the owning implementation.
