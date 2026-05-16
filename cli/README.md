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
adze init my-language
adze check src/grammar.rs
adze stats src/grammar.rs
adze build .
```

Static `adze parse` output is still developing. Use generated Rust parsers and
the `parse()` / `parse_document()` APIs for product parsing contracts until the
CLI parse surface is promoted with proof.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.
