# Publish Checklist

How to publish the Adze release surface to crates.io.

## Publish Order

Crates **must** be published in dependency order. The source of truth for the
release surface and publish order is:

```text
scripts/release-crates.txt
```

The 0.9 microcrate-to-SRP transition is complete. Temporary
`owner-module-migration-target` packages are not allowed in the release surface;
the release gate must pass before publishing.

```bash
cargo run -q -p xtask -- check-package-boundary --release-gate
PACKAGE_BOUNDARY_RELEASE_GATE=1 ./scripts/validate-release-surface.sh
```

Durable support crates that remain standalone are recorded by
`docs/adr/ADZE-ADR-0005-durable-published-support-crates.md` and must remain in
the release surface while core crates depend on them:

- `adze-bdd-governance-core`
- `adze-linecol-core`
- `adze-parsetable-metadata`

The following table is a compact dependency reminder for the core pipeline, not
the complete release surface.

| Step | Crate | Directory | Key deps |
|------|-------|-----------|----------|
| 1 | `adze-common` | `common/` | *(external only)* |
| 2 | `adze-ir` | `ir/` | *(external only)* |
| 3 | durable support crates | `crates/*` | see `ADZE-ADR-0005` |
| 4 | `adze-glr-core` | `glr-core/` | `adze-ir` |
| 5 | `adze-tablegen` | `tablegen/` | `adze-ir`, `adze-glr-core`, durable support crates |
| 6 | `adze-macro` | `macro/` | `adze-common` |
| 7 | `adze-tool` | `tool/` | `adze-common`, `adze-ir`, `adze-glr-core`, `adze-tablegen` |
| 8 | `adze` | `runtime/` | `adze-macro`, `adze-ir`, `adze-glr-core`, `adze-tablegen`, durable support crates |

## Pre-publish verification

```bash
# Run the automated check (metadata + cargo package --list)
just check-publishable

# Validate the release surface and the microcrate-to-SRP release gate
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
# 1. Ensure clean working tree
git status  # should be clean

# 2. Update versions for the release you are cutting
#    For example: 0.8.0 -> 0.9.0, including Cargo.toml files and cross-references.

# 3. Run the publish and release-surface checks
./scripts/check-publish.sh
PACKAGE_BOUNDARY_RELEASE_GATE=1 ./scripts/validate-release-surface.sh

# 4. Commit the version bump
git commit -am "release: vX.Y.Z"
git tag vX.Y.Z

# 5. Publish in scripts/release-crates.txt order.
#    Prefer the release helper; otherwise publish each listed crate manually and
#    wait for each to appear on crates.io before publishing dependents.
./scripts/release.sh
# or:
# while read -r crate; do
#   [[ -z "$crate" || "$crate" == \#* ]] && continue
#   cargo publish -p "$crate"
# done < scripts/release-crates.txt

# 6. Verify the published CLI installs from crates.io in an isolated temp root.
#    Run this only after the crate is visible on crates.io.
cargo run -q -p xtask -- verify-crates-io-install adze-cli --bin adze --version X.Y.Z

# 7. Push tags
git push origin main --tags
```

The crates.io install verifier is the post-publish receipt for any user-facing
`cargo install adze-cli` quickstart claim. Before publishing, inspect the command
plan without touching crates.io:

```bash
cargo run -q -p xtask -- verify-crates-io-install adze-cli --bin adze --version X.Y.Z --dry-run
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
