# Publish Checklist

How to publish the Adze release surface to crates.io.

Current release/publish authorization and the post-publish crates.io install
receipt are tracked in
[`adze-swarm#325`](https://github.com/EffortlessMetrics/adze-swarm/issues/325).
That issue is a tracker, not authorization by itself.

## Publish Order

Crates **must** be published in dependency order. The source of truth for
membership and order is the committed release graph:

```text
policy/release-graph.toml
```

Regenerate and verify it from the ledger-published set:

```bash
cargo run -q -p xtask -- generate-release-graph
cargo run -q -p xtask -- check-release-graph
```

Shell helpers and derived artifacts read that graph; they do not define their
own lists:

| Role | Command / path |
|------|----------------|
| Print ordered crate names | `cargo run -q -p xtask -- print-release-graph` |
| Shell reader | `scripts/release-graph-crates.sh` |
| Derived one-name-per-line list | `scripts/release-crates.txt` (generated; do not hand-edit) |
| Publishability metadata check | `just check-publishable` (`scripts/check-publish.sh`) |
| Release surface validation | `RELEASE_SURFACE_MODE=fixed ./scripts/validate-release-surface.sh` |

The 0.9 microcrate-to-SRP transition is complete. Temporary
`owner-module-migration-target` packages are not allowed in the release surface;
the release gate must pass before publishing.

```bash
cargo run -q -p xtask -- check-package-boundary --release-gate
cargo run -q -p xtask -- check-release-graph
PACKAGE_BOUNDARY_RELEASE_GATE=1 ./scripts/validate-release-surface.sh
```

Durable support crates that remain standalone are recorded by
`docs/adr/ADZE-ADR-0005-durable-published-support-crates.md` and are included
in the release graph while core crates depend on them:

- `adze-bdd-governance-core`
- `adze-common-type-ops-core`
- `adze-linecol-core`
- `adze-parsetable-metadata`

To inspect the current ledger-selected publish order:

```bash
cargo run -q -p xtask -- print-release-graph
```

Do not maintain a separate hand-written crate list in documentation or scripts.
The graph above is the complete 12-crate release surface for 0.10.0 work.

## Pre-publish verification

Publishing happens from public `EffortlessMetrics/adze`, not from
`EffortlessMetrics/adze-swarm`. Before any tag or publish command, first
confirm that public `adze/main` has received the selected `adze-swarm` release
candidate through an explicit public promotion PR. If the trees differ, stop
and run the public promotion plan before continuing. Run this comparison from
the `adze-swarm` preflight checkout with `origin` pointing at
`EffortlessMetrics/adze-swarm` and `public` pointing at
`EffortlessMetrics/adze`:

```bash
git fetch origin --prune
git fetch public --prune
git diff --quiet public/main..origin/main
```

Treat a non-empty diff as a release blocker, not as a reason to publish from
`adze-swarm` or to move release secrets there.

```bash
# Run the automated check (metadata + cargo package --list)
just check-publishable

# Validate the release surface and the microcrate-to-SRP release gate
cargo run -q -p xtask -- check-release-graph
./scripts/check-release-consumers.sh
PACKAGE_BOUNDARY_RELEASE_GATE=1 ./scripts/validate-release-surface.sh

# Full packaging test (requires all deps on crates.io already)
cargo package --allow-dirty -p <crate>
```

## Per-crate checklist

For each crate, before running `cargo publish`:

- [ ] Version bumped from `-dev` to release (e.g., `0.8.0`)
- [ ] All path dependencies also have their version bumped
- [ ] `cargo package -p <crate>` succeeds (no `--allow-dirty`)
- [ ] README.md is present and accurate
- [ ] LICENSE-MIT and LICENSE-APACHE are present
- [ ] `description` is meaningful (not a placeholder)
- [ ] `license = "Apache-2.0 OR MIT"` matches workspace
- [ ] `repository` points to the correct GitHub URL
- [ ] `publish = true` is set (workspace default is `publish = false`)
- [ ] `include` directive lists all needed files
- [ ] No secrets or large binaries in the package (`cargo package --list`)

## Publishing a release

```bash
# 1. Work from public EffortlessMetrics/adze after the selected adze-swarm
#    release candidate has been promoted into public main.

# 2. Ensure clean working tree
git status  # should be clean

# 3. Update versions for the release you are cutting
#    For example: 0.8.0 -> 0.9.0, including Cargo.toml files and cross-references.

# 4. Run the publish and release-surface checks
./scripts/check-publish.sh
cargo run -q -p xtask -- check-release-graph
./scripts/check-release-consumers.sh
PACKAGE_BOUNDARY_RELEASE_GATE=1 ./scripts/validate-release-surface.sh

# 5. Commit the version bump
git commit -am "release: vX.Y.Z"
git tag vX.Y.Z

# 6. Publish in release-graph order (`policy/release-graph.toml`).
#    Prefer the release helper; otherwise publish each graph crate manually and
#    wait for each to appear on crates.io before publishing dependents.
./scripts/release.sh
# or:
# while read -r crate; do
#   cargo publish -p "$crate"
# done < <(cargo run -q -p xtask -- print-release-graph)

# 7. Verify the published CLI installs from crates.io in an isolated temp root.
#    Run this only after the crate is visible on crates.io.
cargo run -q -p xtask -- verify-crates-io-install adze-cli --bin adze --version X.Y.Z --locked

# 8. Push tags
git push origin main --tags
```

The crates.io install verifier is the post-publish receipt for any user-facing
`cargo install adze-cli` quickstart claim. Before publishing, inspect the command
plan without touching crates.io. The verifier checks package metadata with the
explicit `crates-io` registry and also installs with `--registry crates-io` so
the receipt cannot be satisfied by the local workspace package or another
configured default registry:

```bash
cargo run -q -p xtask -- verify-crates-io-install adze-cli --bin adze --version X.Y.Z --locked --dry-run
```

Do not treat the dry run as registry proof. It only confirms the post-publish
receipt command shape.

## Troubleshooting

### "no matching package named X found"

This means a path dependency hasn't been published yet. Publish dependencies
first, in the order listed above.

### "failed to verify package tarball"

The crate's `include` directive may be too restrictive. Check that all
referenced source files are included with `cargo package --list -p <crate>`.

### Version mismatch

All inter-workspace path dependencies must have matching version strings.
For example, if `adze-ir` is `0.8.0`, then `adze-glr-core`'s dep on
`adze-ir` must also say `version = "0.8.0"`.
