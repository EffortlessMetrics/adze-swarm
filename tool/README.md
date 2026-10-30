# adze-tool

Build-time code generation tool for Adze grammars.

Extracts grammar definitions from Rust source files annotated with Adze macros
and generates Tree-sitter JSON grammars and parser code. Typically invoked via
`adze_tool::build_parsers()` in a crate's `build.rs`.

This crate is part of the [Adze](https://github.com/effortlessmetrics/adze)
workspace -- an AST-first grammar toolchain for Rust.

`adze-gen init` is retired and creates or modifies no files. Use the
canonical `adze init` command from the `adze` CLI to create a grammar project.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.
