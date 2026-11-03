//! Shared `cargo test` flag table (#1058 unification).
//!
//! Canonical list of the `cargo test` flags the proof tooling treats as
//! selecting a supported compiled set. This mirrors the table embedded in
//! `executed_proof.rs` (the executed runner owns its inline table; that file
//! is read-only for this slice, so agreement between this module and the
//! runner is pinned by
//! `test_shared_flag_table_agrees_with_executed_runner` in
//! `xtask/tests/support_tier_proof_commands.rs` until the runner itself
//! imports this module).
//!
//! Known intentional gap (filed separately): the executed runner additionally
//! accepts `--flag=value` forms of the supported value flags below
//! (`--features=`, `--profile=`, `--package=`, `--manifest-path=`,
//! `--test=`), while the static validator conservatively rejects every
//! unknown `-`/`--*` token. That is the safe direction (quarantine, never
//! false proof). Post-`--` harness passthroughs other than `--exact` are a
//! second known gap owned by the same follow-up: the static validator ignores
//! them (they do not change selection) while the executed runner rejects
//! them.

/// Human-readable summary of the supported selectors, kept identical to the
/// executed runner's `UnsupportedFlag` message.
pub const SUPPORTED_SUMMARY: &str =
    "-p/--package, --manifest-path, --features, --test, --lib, --profile";

/// Value flags selecting a supported compiled set: `-p`/`--package`,
/// `--manifest-path`, `--features`, `--test`, `--profile`.
pub const SUPPORTED_VALUE_FLAGS: &[&str] = &[
    "-p",
    "--package",
    "--manifest-path",
    "--features",
    "--test",
    "--profile",
];

/// Boolean flag selecting a supported compiled set: `--lib`.
pub const SUPPORTED_BOOL_FLAGS: &[&str] = &["--lib"];

/// Harness-only selector with no libtest executable: doctests need an
/// explicit adapter, so this is reported as `UNSUPPORTED[harness]`, never
/// silently accepted.
pub const HARNESS_ONLY_FLAG: &str = "--doc";

/// Target selectors the executed runner rejects: they would change what
/// Cargo builds/selects while a receipt still claimed the narrower supported
/// selection above.
pub const REJECTED_TARGET_SELECTORS: &[&str] = &[
    "--bin",
    "--bins",
    "--tests",
    "--examples",
    "--example",
    "--benches",
    "--bench",
    "--all-targets",
];

/// Profile/feature shortcuts the executed runner rejects, for the same
/// selection-narrowness reason.
pub const REJECTED_PROFILE_SHORTCUTS: &[&str] =
    &["--release", "--all-features", "--no-default-features"];

/// Whether a discrete flag token selects a supported compiled set.
pub fn is_supported_flag(flag: &str) -> bool {
    SUPPORTED_VALUE_FLAGS.contains(&flag) || SUPPORTED_BOOL_FLAGS.contains(&flag)
}

/// Whether a discrete flag token is rejected by name by the executed runner.
pub fn is_rejected_flag(flag: &str) -> bool {
    REJECTED_TARGET_SELECTORS.contains(&flag) || REJECTED_PROFILE_SHORTCUTS.contains(&flag)
}
