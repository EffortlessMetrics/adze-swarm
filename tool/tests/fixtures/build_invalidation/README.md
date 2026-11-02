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

The fixture/control files do not change default test admission or CLI behavior.
The private watch-graph implementation is described separately below.

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

## Implemented private watch graph (#1195)

The repair keeps one shared post-load grammar expansion function and preserves
public signatures/error boundaries. Only the Cargo adapter uses the private
traced loader/builder result. It registers actual loaded source files and their
containing directories, plus nearest existing parents for failed resolution.
Directory tracking observes the existing inliner's candidate additions/removals
without independently reproducing its resolution rules.

The adapter declares the consulted `ADZE_USE_PURE_RUST`,
`ADZE_EMIT_ARTIFACTS`, `ADZE_DEBUG_FILE`, `RUST_LOG`, and
`ADZE_REQUIRE_TS_CLI` inputs. Cargo continues to own feature, target, package and
OUT_DIR identity; the C compiler helper owns its compiler environment tracking.
This is standard Cargo invalidation, not arbitrary include/environment closure
or a direct-executable content key.

A source-directory watch that would recursively include OUT_DIR is deliberately
not treated as fresh reuse. An exclusively owned temporary trigger is removed
and registered as a missing dependency, forcing conservative regeneration with
an explicit diagnostic. This avoids recursively scanning Cargo's target tree
and preserves correctness for overlapping layouts at the existing rerun cost.

Run the controlled source/environment/removal cases with:

```sh
python3 tool/tests/fixtures/build_invalidation/exercise_inputs.py
```

This copies the fixture into an owned temporary project and keeps its target
and invocation counter external. It checks initial/no-change builds, root and
child edits, missing and added modules, alternative candidate ambiguity and
recovery, module relocation, an external `#[path]` child edit, debug artifacts,
backend environment selection and unchanged values, and explicit conservative
source/output overlap. It records every case's input hashes and actual build
counter. The total budget is 600 seconds, with at most 180 seconds per Cargo
command; a timeout is a non-qualification result.

For a removed generated Rust module, the current bounded policy is explicit
compiler refusal: no stale binary executes, and a package-scoped `cargo clean`
in the owned target followed by rebuild restores parsing. Automatic regeneration
on manual output deletion is not claimed. Parser writing and debug-artifact
cleanup remain unchanged. A legacy C-backend selection is checked for correct
environment invalidation; unchanged C-backend performance is not claimed here.

Directory watches are deliberately conservative: unrelated files inside a
watched source directory can cause regeneration, as can configured outputs
placed there. This is not a promise of perfect no-change caching for all source
layouts, and it is not a complete arbitrary Rust include/environment closure.

Two independently retained profile failures remain outside this repair:

- `adze-tool --no-default-features` fails because the unchanged public
  `scanner_build` module references the optional `cc` dependency. The new private
  tracking code is gated by `build_parsers`; it does not qualify that existing
  minimal profile.
- A caller with no local `pure-rust` feature, an Adze runtime dependency with
  pure Rust enabled, `ADZE_USE_PURE_RUST` absent and
  `ADZE_EMIT_ARTIFACTS=true` selects legacy C generation after opportunistic Rust
  generation. Existing C debug-artifact cleanup removes that Rust module and
  compilation refuses the missing include. The matrix qualifies the ordinary
  backend switch with artifacts disabled; the failing combination remains a
  separate executed product defect, not a successful parsing case.
