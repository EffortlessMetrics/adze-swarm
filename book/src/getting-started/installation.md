# Set up Adze from a checkout

This page uses the runtime and build tool from an Adze checkout. It does not
require published Adze packages.

Use Rust 1.95.0 or newer for this crate. A sibling crate does not inherit the
Adze checkout's toolchain file, so make sure your own Cargo uses that version
before continuing. For example, with rustup you can select it for a command
with `cargo +1.95.0`.

## Add the dependencies

The example assumes your crate sits next to a checkout named `adze-swarm`.
Adjust the paths if the checkout is elsewhere. Merge the entries into your
existing `Cargo.toml` tables. If your crate already lists default features, add
`"pure-rust"` without removing them.

```toml
[dependencies]
adze = { path = "../adze-swarm/runtime", default-features = false }

[build-dependencies]
adze-tool = { path = "../adze-swarm/tool" }

[features]
default = ["pure-rust"]
pure-rust = ["adze/pure-rust"]
```

## Build configuration

Create `build.rs` in the project root:

```rust
use std::path::PathBuf;

fn main() {
    println!("cargo::rustc-check-cfg=cfg(adze_unsafe_attrs)");
    adze_tool::build_parsers(&PathBuf::from("src/lib.rs"));
}
```

The path in `build_parsers` should point to the file with the grammar module.
For a binary crate with its grammar in the entry point, use `src/main.rs`.

Follow [Quick Start](quickstart.md) to add a grammar and run the parser tests.
Check [Support tiers](https://github.com/EffortlessMetrics/adze-swarm/blob/main/docs/status/SUPPORT_TIERS.md)
before enabling features outside the typed pure-Rust path.
