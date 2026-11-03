# adze-cli

Command-line tools for Adze grammar development and validation.

The `adze` binary can initialize clean grammar projects, build parser artifacts,
validate grammar files, print grammar statistics, and report the current Adze
CLI version. It is a user-facing companion to the `adze` runtime and
`adze-tool` build-time code generation crate.

This crate is part of the [Adze](https://github.com/effortlessmetrics/adze)
workspace, an AST-first grammar toolchain for Rust.

## Common Commands

These commands assume an `adze` binary built from this checkout or a local
package. Published `cargo install adze-cli` usage remains a release-surface
claim blocked on the post-publish install receipt tracked in #325.

```bash
adze init calc
cd calc
cargo test
cargo run --example parse -- "1 + 2 * 3"
```

Other useful commands:

```bash
adze check src/grammar.rs
adze stats src/grammar.rs
adze build .
adze build src/grammar.rs
adze build . --output path/to/artifacts
```

`adze build` works outside a Cargo build script and does not need
`OUT_DIR`. A directory target builds from its `src/lib.rs` entrypoint
(`src/main.rs`, then `src/grammar.rs`, are fallbacks); a file target builds
that file. Generated pure-Rust parser modules are written as
`grammar_<name>/parser_<name>.rs` under `<project>/target/adze-build` by
default, or under the directory given with `--output`/`-o`. A relative
target resolves against the caller's working directory, while the default
output always belongs to the selected project. Missing or invalid input,
a project with no grammar, and an unwritable output are reported as
errors rather than panics. `adze build --watch` uses the same initial
build and keeps watching after a later rebuild error.

Static `adze parse` output is still advisory. The default `tree`, `json`,
`sexp`, `dot`, and document projection modes compile a temporary
single-grammar runner, call the generated `parse_document()` helper, and
project document-backed output. Use generated Rust parsers and the
`parse()` / `parse_document()` APIs for stable product parsing contracts until
the CLI parse surface is promoted with proof.

The parse command exposes the ADZE-SPEC-0008 projection names:

```bash
adze parse src/grammar.rs input.txt
adze parse src/grammar.rs input.txt --output json
adze parse src/grammar.rs input.txt --output sexp
adze parse src/grammar.rs input.txt --output dot
adze parse src/grammar.rs input.txt --output document-json
adze parse src/grammar.rs input.txt --output tree-json
adze parse src/grammar.rs input.txt --output diagnostics-json
adze parse src/grammar.rs input.txt --output ambiguity-json
```

These modes are intended for single-file grammar smoke checks and tooling
receipts. They are not a stable CLI/WASM schema contract.

Format map: `tree` renders the document tree; `json` prints the full
`document-json` envelope (not a minimal tree); `sexp`/`dot` project the
selected tree; `document-json`/`tree-json`/`diagnostics-json`/`ambiguity-json`
emit the named projection verbatim. `--format` and `--output` are aliases.

## Static runner process limits on Linux

One temporary Cargo-build-and-run invocation may capture up to **64 MiB of
stdout** and **1 MiB of stderr** (one MiB is 1,048,576 bytes). Capture stops with
an error if either stream exceeds its limit; incomplete output is never printed
as a successful projection. The stdout budget includes the private response
header (at most 4096 bytes including its LF) and the unchanged public payload.
The invocation has one **600-second deadline**
covering execution and pipe reads. Output progress does not restart it. After
the direct child exits, any remaining pipe drain has at most **2 seconds** and
still cannot exceed the original deadline. These are initial bounded CLI
limits, not a promise that every grammar or cold build fits them.

On completion, a capture failure, or SIGINT/SIGTERM cancellation, the Linux
parent terminates the runner's owned process group and allows at most **2
additional seconds** to confirm reaping its direct child. Cleanup which cannot
be confirmed is an error. A child exit observed before intervention is retained
separately from a timeout, capture limit, or cleanup error. Cancellation restores
the prior signal dispositions, unwinds the temporary runner directory and then
terminates the CLI conventionally. Inherited ignored signals stay ignored;
custom cancellation handlers or a child auto-reap policy are refused before
starting the runner.

This covers ordinary descendants which stay in the inherited process group.
It does not provide a sandbox or custody of deliberately escaped descendants.
The existing argv, input path, environment and inherited Cargo working directory
are preserved. Platforms other than Linux and Windows retain the existing runner
path and are not qualified for these capture, deadline or cleanup bounds.
Public projection schemas and parser outcome semantics are unchanged.

## Static runner process limits on Windows

The Windows path targets Windows 10 / Windows Server 2016 and newer. It uses the
same stdout, stderr, execution and drain limits above, with **2 additional
seconds** to confirm that its owned job is empty. The helper belongs to that job
from process creation, before it starts Cargo. Ordinary inherited descendants
remain owned even if Cargo or the helper exits first; a failed or unconfirmed
cleanup is reported. Closing the CLI also closes the job's non-inherited handle.

The private launch request has a **1 MiB encoded-byte limit**, including its
header, counts and UTF-16 fields. Oversized or malformed requests fail before
external execution. Arguments preserve their Windows code units, input paths
keep their spelling, and environment changes retain their order. Cargo still
uses Rust's standard command handling and the CLI's inherited working directory,
including its Cargo configuration discovery. Cache reuse remains disabled.

Ctrl-C and Ctrl-Break cancel capture, clean up the job and unwind the temporary
runner directory before this CLI exits with Windows status `0xC000013A`.
Inherited ignored Ctrl-C remains ignored. The CLI does not broadcast a console
event to its caller. Forced termination or console closure can prevent Rust
unwinding; the job still provides descendant custody, while temporary-directory
removal is not guaranteed in those cases. This remains trusted local grammar
execution, with the same public schemas and parser outcomes.

Native qualification is specific to the tested Windows image, architecture and
toolchain. A Linux receipt or a Windows cross-compilation is not Windows process,
console or installed-package proof.

## Static runner builds and cache admission

Static parsing currently builds a temporary runner through Cargo on every
invocation. Changing the input always parses fresh. Progress messages go to
stderr; document output owns stdout.

Direct cached-executable reuse is disabled while issue #1067 establishes
complete build-input custody. The generated runner currently depends on
mutable workspace packages or floating registry requirements, and a grammar
hash plus a source-tree walk does not identify the resolved lock state,
transitive dependencies, Cargo configuration, compiler wrappers or arbitrary
build-script inputs. A seven-day expiry cannot make an incomplete key safe.
The CLI therefore uses the checked temporary-project path instead of executing
a potentially stale cache entry. This also applies to existing cache entries.

```bash
adze parse src/grammar.rs input.txt
ADZE_RUNNER_CACHE=off adze parse src/grammar.rs input.txt
```

The candidate-key, atomic publication and integrity-check machinery remains
covered by isolated fixtures for future admission work. It does not yet provide
a supported zero-Cargo warm-hit path. Candidate scanning handles ordinary and
inline modules and literal raw-data includes; uncertain conditional module
paths and Rust `include!` fragments require Cargo validation. Production
admission requires a separate complete dependency/configuration proof.

Cache-root handling requires a private owned directory with trusted ancestors
on Unix; other ACL platforms refuse cache use until equivalent checks exist.
The dormant owned-store layout has at most 32 permanent slots, 128 durable run
tokens, and 1 GiB of reserved or published artifact bytes. A complete artifact
allowance is recorded before writing; each namespace record and its one
replacement are limited to 64 KiB. Nonblocking namespace/slot locks and tokens
bound to the physical store, generation and owner replace pathname/age-based
cleanup. Retirement requires an exclusive slot and no durable owners. Old
cache layouts, unrelated files and permanent coordination files are preserved.
Executable hashing has no measured performance guarantee; #869 owns reporting.

Dormant cache IO now refuses over-budget files while reading: manifests are
limited to 1 MiB, executables to 256 MiB, retained lockfiles to 4 MiB, and the
grammar closure to 64 MiB across at most 256 files. Executable hashing and
exclusive publication copies stream through an 8 KiB buffer. A length reported
by the filesystem is only an early check; the read loop enforces the limit if
a file grows. These are refusal thresholds, with no claim that every generated
runner fits them. The artifact reservation bounds only internally copied
artifacts; Cargo never receives the store's staging path. Compiler scratch,
filesystem latency and immutable compiled inputs remain separate unqualified
requirements. Scratch is retained when process ownership is unproved.

A stored executable is exposed through a durable execution lease. Destruction,
PID, age, timeout, output status and successful decoding never release its
reservation. The shared process layer has no qualified empty-owner receipt
producer yet, so completion retains that capacity and returns an error. A
semantic result followed by store-finalization failure cannot trigger another
parse. Orphan reservations require explicit administrative recovery after old
owners and descendants are proved absent. None of this enables production
reuse or qualifies Windows ACLs, network filesystems or power-loss durability.

The dormant direct-runner and Cargo-build paths, fresh Cargo execution and
rustc fingerprint acquisition share the same process boundary. On Linux and
Windows its existing 64 MiB stdout / 1 MiB stderr capture limits, 600-second
absolute deadline and 2-second drain/cleanup phases also apply to these sites.
Process setup, cancellation, capture and coordination refusals propagate as
errors; they never trigger a fresh fallback. A concurrent first fingerprint
probe refuses immediately, and only a successful fingerprint is memoized.
Completed nonzero rustc output leaves the partial candidate unavailable.

Both runner execution paths request private protocol version 2 and preserve
actual failed-child status before decoding. Cargo and rustc bytes are raw
process output. All four sites inherit the CLI cwd and environment; only the
dormant cache build additionally sets its owned CARGO_TARGET_DIR. This does
not extend cache admission, input/config custody or platform support claims.

## Windows notes

First `adze parse` compiles a temporary runner and may look hung on Windows;
wait for the `compiling temporary parse runner` line. Color/emoji output may
render as boxes in older consoles. Grammar discovery matches `*grammar*.rs`.

Dynamic parse mode is unavailable in 0.10.0. `adze parse --dynamic` fails
immediately without loading any shared library; parse statically with
`adze parse <grammar.rs> <input>` instead. See the dynamic-loading guide for
the current non-claim boundary.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.
