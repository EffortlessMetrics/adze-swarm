# adze

When syntax changes, keep the grammar beside the Rust types your application
reads. Adze generates a parser at build time from rules on those types.
This crate runs the generated parser and extracts typed Rust values.

Start with the default pure-Rust path. Call `grammar::parse(source)` for typed
values, or `grammar::parse_document(source)` when your tool needs diagnostics
and a selected syntax tree. The document API is Stabilizing. Check the support
notes in your checkout before relying on optional runtime features or adapters.

The runtime is part of [Adze](https://github.com/effortlessmetrics/adze).
To try it from source, follow the getting-started guide in your checkout and
use the runtime and build-time tool from the same revision.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.
