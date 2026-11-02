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

## Static runner cache

The first parse of a grammar compiles a dedicated runner; later parses of
the same build reuse the cached binary directly with no Cargo invocation,
and different input text always parses fresh. Only compiled runners are
cached, never parse results, and cached output is byte-identical to an
uncached build (progress chatter goes to stderr; document output owns
stdout).

The cache key is a content hash over the full build identity: grammar bytes
plus reachable sibling modules/includes, the rendered runner project,
dependency sources (in-workspace `path =` trees are hashed recursively;
published installs key version requirements and expire), the toolchain and
target, profile, feature set, and relevant build environment. Grammar mtime
alone never identifies an executable. Anything outside that envelope —
dynamic `include!` arguments, unresolvable modules, unreadable inputs, or an
unusable toolchain fingerprint — falls back to a fresh temporary build with
correct output instead of a silent stale reuse. The private runner transport
contract is still the current document-projection shape; issue #1066 owns
its versioning, and the cache records a protocol version that a future #1066
contract must bump.

```bash
adze parse src/grammar.rs input.txt          # cold: builds once, then reuses
ADZE_RUNNER_CACHE=off adze parse src/grammar.rs input.txt   # always rebuild
ADZE_RUNNER_CACHE_DIR=/path/to/dir adze parse src/grammar.rs input.txt
```

Entries live under `ADZE_RUNNER_CACHE_DIR` (default: a per-user temp
directory), are published atomically so concurrent parses never observe
partial artifacts, expire after 7 days, and are bounded to 32 entries by
default (`ADZE_RUNNER_CACHE_MAX_ENTRIES`). Cleanup only removes
cache-owned entries and never touches another invocation's staging or
unrelated files.

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
