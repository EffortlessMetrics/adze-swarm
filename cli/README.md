# adze-cli

Command-line tools for Adze grammar development and validation.

The `adze` binary can initialize clean grammar projects, build parser artifacts,
validate grammar files, print grammar statistics, and report the current Adze
CLI version. It is a user-facing companion to the `adze` runtime and
`adze-tool` build-time code generation crate.

This crate is part of the [Adze](https://github.com/effortlessmetrics/adze)
workspace, an AST-first grammar toolchain for Rust.

## Common Commands

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
```

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

Dynamic parse mode is a separate experimental boundary. `adze parse --dynamic`
requires building `adze-cli` with `--features dynamic`; with that feature it can
attempt to load a shared-library symbol, but dynamic parse output is not
implemented or supported yet. See the dynamic-loading guide for the current
non-claim boundary.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.
