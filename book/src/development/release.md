# Release Process

Release orchestration is now handled by [`.github/workflows/release.yml`](../../../.github/workflows/release.yml).

Use workflow dispatch with these inputs to perform the full, canonical path:
- validation
- full test suite
- artifact builds
- version/tags updates
- publish
- GitHub release creation

Workflow inputs:
- `version` (required): release version (e.g. `0.10.0`).
- `release_surface_mode` (default: `fixed`): `fixed` (use committed release graph) or `auto` (recompute publishable crates from metadata).
- `release_crate_file` (optional): override derived list path used by shell helpers (defaults to `scripts/release-crates.txt`).
- `dry_run` (default: `true`): run all validation and checks without publishing.
- `strict_publish_surface` (default: `false`): in `fixed` mode, fail release if extra publishable crates are not in the release graph.

## Release graph authority

One graph drives every release consumer:

```text
policy/package-boundary.toml
  -> policy/release-graph.toml
  -> scripts/release-crates.txt (derived; do not hand-edit)
```

Regenerate and verify:

```bash
cargo run -q -p xtask -- generate-release-graph
cargo run -q -p xtask -- check-release-graph
./scripts/check-release-consumers.sh
```

See also `docs/reference/PUBLISH_CHECKLIST.md`.

Legacy local helpers are kept for ad-hoc use only:
- [`scripts/update-versions.sh`](../../../scripts/update-versions.sh)
- [`scripts/release.sh`](../../../scripts/release.sh)
- [`scripts/dry-run-publish.sh`](../../../scripts/dry-run-publish.sh)

For local helper runs, set:
- `RELEASE_SURFACE_MODE=fixed|auto`
- `RELEASE_GRAPH_ARTIFACT=<path>` (default: `policy/release-graph.toml`)
- `RELEASE_CRATE_FILE=<path>` (default: `scripts/release-crates.txt`)
- `STRICT_PUBLISH_SURFACE=true|false` (fixed mode only; default `false`)
- `RELEASE_CRATE_SYNC=true` (only meaningful in `auto` mode)

`release.toml` is still used for changelog/version replacement metadata.

`release.sh` and `dry-run-publish.sh` support two release-surface modes:
- `RELEASE_SURFACE_MODE=fixed` (default): read crates from `policy/release-graph.toml` via `cargo xtask print-release-graph`.
- `RELEASE_SURFACE_MODE=auto`: compute publishable workspace crates and auto-resolve dependency order.
- `RELEASE_CRATE_SYNC=true` with `RELEASE_SURFACE_MODE=auto`: regenerate `RELEASE_CRATE_FILE` from metadata.
