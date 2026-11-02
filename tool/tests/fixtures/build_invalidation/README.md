# Tiny generated-parser build invalidation control

This is a manually selected real-Cargo control for build-script invalidation
under #869/CI economics. It is separate from #1067/#1160 executable-cache
identity and makes no parser throughput or named-runner performance claim.

Run from a configured Rust 1.95.0 environment with the locked dependencies
already available offline:

```sh
python3 tool/tests/fixtures/build_invalidation/check.py
```

The driver creates an owned external target/counter/receipt directory. It runs
exactly two offline, locked Cargo commands with unchanged fixture inputs, jobs
2, incremental compilation disabled, debug information disabled and a 180-second
limit per command. On timeout it kills only the owned POSIX process group and
reaps the direct Cargo child. Logs and a JSON receipt survive failure. This
initial control has not established Windows behavior.

The root source loads the grammar from `src/authored.rs`, so the fixture can
later discriminate a root-file-only invalidation repair. The binary independently
asserts `parse("hello") == Word { value: () }` on both runs. The build script
increments a counter outside all source paths before invoking the existing
`adze_tool::build_parsers`. It adds no input or output watch of its own.

The expected count is one after the unchanged second Cargo run. At the
unmodified `ab1bcde` tool source, both binaries establish correct parsing, but
actual build-script invocations increase from one to two. Cargo's fingerprint
log names the generated OUT_DIR parser as newer than its own build-script
output reference. The separate original receipt is retained in the coordination
handoff; the checked-in driver creates a fresh target for each reproduction.

This fixture is preparation for a bounded watch-graph repair. No production
code, default test admission or CLI behavior is changed here.

## Proposed implementation boundary

Keep the public `generate_grammars` and `build_parser_for_crate` return APIs
unchanged. A private traced-loading helper can reuse
`syn_inline_mod::InlinerBuilder::inline_with_callback` and return grammar JSON
plus actual loaded paths and unresolved input information. A private builder
wrapper propagates that information to the build-script adapter. Only the
build-script adapter emits Cargo directives; the public CLI must not receive
`cargo:` lines.

Do not watch generated parser outputs as generator inputs. Root-only watching
is insufficient for this fixture. Loaded-file callbacks also do not prove an
arbitrary Rust include/environment/dependency closure: failed or alternative
module resolution candidates, material environment choices and deletion/recovery
behavior need explicit treatment. The existing best-effort inlining error policy
must not be silently changed as a performance fix. Standard Cargo invalidation
is not the complete content identity required for direct executable reuse.

The first repair can leave parser writing and debug-artifact cleanup unchanged.
Content-preserving writes are a possible later optimization; `emit_artifacts`
currently removes/recreates its grammar directory, so changing writes also needs
an explicit obsolete-artifact policy.

Before production edits, coordinate with #1169's `tool/src/expansion.rs`
diagnostic repair and #1184's `tool/src/build_parsers/mod.rs` named failure
boundary. Prefer leaving both files untouched, preserving their diagnostics.

Required follow-up controls include root and nested-module edits, relevant
configuration/backend changes, unresolved/missing module behavior, output
removal, and a public CLI stdout check. Those controls are not claimed by this
initial two-run red.
