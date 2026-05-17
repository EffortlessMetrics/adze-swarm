# Product Fixture Catalog

These fixtures are for the GLR toolkit productization campaign. They are not a
new test harness by themselves; they define the shared fixture surface that GLR,
document projection, Tree-sitter compatibility, query, recovery, and benchmark
tests should use.

Start with:

- `catalog.toml` for the machine-readable registry.
- `glr/` for ambiguity and conflict fixtures.
- `ts-compat/` for selected-tree and metadata compatibility fixtures.
- `query/` for Tree-sitter query subset fixtures.
- `recovery/` for bad-input and diagnostic fixtures.

Each concrete fixture should name its support-tier relevance and proof command
before it is used for promotion.
