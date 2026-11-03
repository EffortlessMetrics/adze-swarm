//! Content-addressed cache for compiled static parse runners (issue #1067).
//!
//! Implements candidate-key and publication machinery; production reuse is
//! refused until complete build-input custody exists. Only compiled runners
//! are eligible for future caching, never parse results. Input parses fresh.
//!
//! # Admission boundary
//!
//! Direct executable reuse is currently refused for production runners. The
//! generated runner uses mutable Adze path dependencies or floating registry
//! requirements, and this cache does not hold the complete resolved dependency,
//! lock, Cargo configuration and build-script/environment identity. A source
//! tree walk or a time-to-live cannot establish that identity. Every production
//! parse therefore uses the existing Cargo checked temporary-project path.
//!
//! The grammar candidate key and atomic entry machinery are retained for a
//! future fully owned build envelope and exercised with isolated fixtures.
//! Candidate keys alone never authorize execution. #1067 remains open for the
//! complete build-input custody and zero-Cargo warm-hit acceptance.
//!
//! # Atomicity and concurrency
//!
//! Builds happen in an exclusively owned staging directory
//! (`.staging-<pid>-<counter>`, never a colliding pre-existing path).
//! Publication writes a complete entry (manifest + verified executable +
//! lockfile copy) into a staging subdirectory and renames it onto the final
//! `<key>` path only after verification, so concurrent readers never observe
//! partial artifacts: an entry is either absent or complete and verified.
//! Per-key `.lock` files serialize duplicate builds; waiters use only
//! validated entries and degrade to a fresh uncached build on timeout.
//! Cleanup only ever removes cache-owned entry/`.trash` directories and
//! stale `.lock` files; another invocation's staging is never touched, and
//! Windows file-sharing failures degrade to left-behind trash that lookups
//! ignore.
//!
//! # Protocol dependency (#1066)
//!
//! Both execution paths use the shared private version-2 frame and bounded
//! Linux/Windows process boundary. Cargo and rustc output remain raw process
//! output. Capture/setup/cancellation refusals are hard errors, never misses.
//! This transport repair does not authorize reuse or prove the compiled-input
//! envelope. [`RUNNER_PROTOCOL_VERSION`] is the shared protocol authority.

use crate::runner_cache_io;
use crate::runner_invocation::{ProcessFailure, ProcessRunner, RunnerInvocation};
use anyhow::Context;
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, TryLockError};
use std::time::{SystemTime, UNIX_EPOCH};

/// Reuse the private transport authority; old unframed entries fail validation.
pub(crate) const RUNNER_PROTOCOL_VERSION: u32 = crate::runner_protocol::PROTOCOL_VERSION;

/// Maximum entries retained; enforced (best-effort) after each publish.
const MAX_ENTRIES_DEFAULT: usize = 32;
/// Entry directory names are 64-character lowercase hex digests.
const ENTRY_NAME_LEN: usize = 64;
/// Entries older than this are treated as misses (retention policy only).
const ENTRY_TTL_SECS: u64 = 7 * 24 * 3600;
/// Fresh entries are never evicted, so active builds/runs are not raced.
const ACTIVE_GRACE_SECS: u64 = 300;
/// How long a second invocation waits for the lock owner's entry.
const BUILD_WAIT_SECS: u64 = 600;
/// Locks older than this are ignored for waiting (swept by eviction).
const STALE_LOCK_SECS: u64 = 3600;
/// Closure walk budgets; exceeding them refuses caching (fail safe).
const MAX_CLOSURE_FILES: usize = 256;
const MAX_CLOSURE_BYTES: u64 = 64 << 20;
/// Byte caps for dormant entry IO; exceeding one refuses this cache entry.
/// These do not reserve disk space or bound Cargo's build scratch directory.
const MAX_MANIFEST_BYTES: u64 = 1 << 20;
const MAX_EXECUTABLE_BYTES: u64 = 256 << 20;
const MAX_LOCKFILE_BYTES: u64 = 4 << 20;
/// Bounded tail kept in `build.log` for post-mortem diagnosis.
const BUILD_LOG_TAIL: usize = 8192;
/// Test-only hook: when `ADZE_TEST_CARGO_COUNT_FILE` names a file, every
/// spawned Cargo invocation appends one line (acceptance instrumentation).
const COUNT_ENV: &str = "ADZE_TEST_CARGO_COUNT_FILE";

/// Fully rendered inputs for one runner compilation.
pub(crate) struct RunnerSpec<'a> {
    /// Absolute original grammar path (embedded in the runner templates).
    pub grammar: &'a Path,
    /// Top-level `#[adze::grammar]` module name.
    pub module: &'a str,
    /// Rendered runner `Cargo.toml`.
    pub cargo_toml: String,
    /// Rendered runner `build.rs`.
    pub build_rs: String,
    /// Rendered runner `src/main.rs`.
    pub main_rs: String,
}

/// Outcome of [`build_cached`] when no error occurred.
pub(crate) enum CachedBuild {
    /// Execute this executable directly (published entry or adopted rival).
    Ready(PathBuf),
    /// Cache disabled, unsupported, or race lost: use the legacy temp build.
    Unavailable,
}

/// A Cargo build owned by this cache failed; the caller renders it with the
/// existing root-cause formatter so diagnostics stay identical to uncached.
#[derive(Debug)]
pub(crate) struct CargoFailed {
    /// Runner project build exit status.
    pub status: std::process::ExitStatus,
    /// Captured build stdout.
    pub stdout: Vec<u8>,
    /// Captured build stderr.
    pub stderr: Vec<u8>,
}

impl std::fmt::Display for CargoFailed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "cached runner build failed ({})", self.status)
    }
}

impl std::error::Error for CargoFailed {}

/// Only marked cache-local storage/serialization failures permit fallback.
#[derive(Debug)]
struct CacheStorageFailure;

impl std::fmt::Display for CacheStorageFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("runner cache storage is unavailable")
    }
}

/// Reason caching was refused; the caller falls back to the legacy path.
#[derive(Debug)]
struct CacheSkip(String);

/// Resolved cache configuration, or `None` when caching is disabled or the
/// root is unusable (callers then use the legacy temp-project path).
pub(crate) struct CacheConfig {
    pub(crate) root: PathBuf,
    pub(crate) max_entries: usize,
}

/// Look up a verified cached executable without invoking Cargo.
///
/// Returns `None` for disabled cache, unsupported envelopes, misses,
/// stale/corrupt entries, or any cache-internal I/O problem. A returned
/// path names a complete entry whose executable hash was just verified.
pub(crate) fn lookup(spec: &RunnerSpec<'_>) -> Option<PathBuf> {
    let key = build_key(spec)?;
    let config = cache_config()?;
    validate_entry(&config.root, &key).map(|entry| entry.exe)
}

/// Build the runner, publish it atomically, and return its executable.
///
/// `Ok(CachedBuild::Unavailable)` means the caller should use the legacy
/// temp-project path (disabled/unsupported envelope, lost race, stale lock,
/// or a cache-local I/O failure). `Err` preserves [`CargoFailed`] or a typed
/// [`ProcessFailure`] with the concrete capture error still downcastable.
/// Process setup, capture, cancellation and coordination refusals never retry.
pub(crate) fn build_cached(
    spec: &RunnerSpec<'_>,
    process: &ProcessRunner,
) -> anyhow::Result<CachedBuild> {
    let Some(key) = build_key(spec) else {
        return Ok(CachedBuild::Unavailable);
    };
    let Some(config) = cache_config() else {
        return Ok(CachedBuild::Unavailable);
    };
    if let Some(entry) = validate_entry(&config.root, &key) {
        return Ok(CachedBuild::Ready(entry.exe));
    }
    let lock_path = config.root.join(format!("{key}.lock"));
    if !claim_lock(&lock_path) {
        if let Some(entry) = wait_for_entry(&config.root, &lock_path, &key) {
            return Ok(CachedBuild::Ready(entry.exe));
        }
        return Ok(CachedBuild::Unavailable);
    }
    let Some(staging) = claim_staging_dir(&config.root) else {
        let _ = std::fs::remove_file(&lock_path);
        return Ok(CachedBuild::Unavailable);
    };
    let outcome = build_and_publish(spec, &config, &key, &staging, process);
    let _ = std::fs::remove_file(&lock_path);
    let _ = std::fs::remove_dir_all(&staging);
    finish_cached_build(outcome)
}

/// Only cache-local failures permit fresh fallback after owned staging cleanup.
pub(crate) fn finish_cached_build(
    outcome: anyhow::Result<CachedBuild>,
) -> anyhow::Result<CachedBuild> {
    match outcome {
        Err(error)
            if error.downcast_ref::<CacheStorageFailure>().is_some()
                && error.downcast_ref::<CargoFailed>().is_none()
                && error.downcast_ref::<ProcessFailure>().is_none() =>
        {
            log::debug!("runner cache unavailable: {error:#}");
            Ok(CachedBuild::Unavailable)
        }
        other => other,
    }
}

/// Resolve cache configuration from the environment.
fn cache_config() -> Option<CacheConfig> {
    if let Ok(flag) = std::env::var("ADZE_RUNNER_CACHE")
        && matches!(
            flag.trim().to_ascii_lowercase().as_str(),
            "off" | "0" | "false" | "no" | "disabled"
        )
    {
        return None;
    }
    let root = std::env::var_os("ADZE_RUNNER_CACHE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("adze-cli-runner-cache"));
    if !prepare_private_root(&root) {
        log::debug!("runner cache skipped: cache root is not private and trusted");
        return None;
    }
    let max_entries = std::env::var("ADZE_RUNNER_CACHE_MAX_ENTRIES")
        .ok()
        .and_then(|raw| raw.trim().parse::<usize>().ok())
        .filter(|count| *count > 0)
        .unwrap_or(MAX_ENTRIES_DEFAULT);
    Some(CacheConfig { root, max_entries })
}

/// Cache contents are executable code. A self-consistent manifest is not
/// authentication: require a private root owned by this user, reached through
/// trusted ancestors. Unsupported ACL platforms refuse reuse until checked.
#[cfg(unix)]
fn prepare_private_root(root: &Path) -> bool {
    use std::os::unix::fs::{DirBuilderExt, MetadataExt};
    let Ok(probe) = tempfile::tempdir() else {
        return false;
    };
    let Ok(probe_meta) = std::fs::metadata(probe.path()) else {
        return false;
    };
    let owner = probe_meta.uid();
    if std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(root)
        .is_err()
    {
        return false;
    }
    let absolute = if root.is_absolute() {
        root.to_path_buf()
    } else {
        let Ok(cwd) = std::env::current_dir() else {
            return false;
        };
        cwd.join(root)
    };
    for (index, ancestor) in absolute.ancestors().enumerate() {
        let Ok(meta) = std::fs::symlink_metadata(ancestor) else {
            return false;
        };
        if !meta.is_dir() || meta.file_type().is_symlink() {
            return false;
        }
        if index == 0 {
            if meta.uid() != owner || meta.mode() & 0o077 != 0 {
                return false;
            }
        } else if (meta.uid() != owner && meta.uid() != 0)
            || (meta.mode() & 0o022 != 0 && !(meta.uid() == 0 && meta.mode() & 0o1000 != 0))
        {
            return false;
        }
    }
    true
}

#[cfg(not(unix))]
fn prepare_private_root(_root: &Path) -> bool {
    false
}

/// Compute the content-addressed key, or `None` outside the safe envelope.
fn build_key(spec: &RunnerSpec<'_>) -> Option<String> {
    // Do not promote a partial source hash to an executable identity. The
    // selected dependency graph, immutable resolution and build configuration
    // have no custody proof yet, including for published installs.
    let _ = spec;
    log::debug!(
        "runner cache skipped: unresolved dependency/configuration custody; Cargo rebuild required"
    );
    None
}

/// Acquire process identity without consuming capture/refusal errors.
/// This remains a partial candidate, never production reuse authority.
pub(crate) fn grammar_candidate_key(
    spec: &RunnerSpec<'_>,
    process: &ProcessRunner,
) -> anyhow::Result<Option<String>> {
    // Preserve the prior closure preflight before acquiring any child process.
    let Some(hasher) = candidate_hasher(spec) else {
        return Ok(None);
    };
    let Some(toolchain) = rustc_fingerprint(process)? else {
        log::debug!("runner cache skipped: rustc fingerprint unavailable");
        return Ok(None);
    };
    Ok(Some(finish_candidate_key(hasher, &toolchain)))
}

/// Collect candidate bytes without acquiring any subprocess.
fn candidate_hasher(spec: &RunnerSpec<'_>) -> Option<Sha256> {
    let grammar_abs = spec.grammar.canonicalize().ok()?;
    let grammar_base = grammar_abs.parent()?.to_path_buf();
    let mut closure = Vec::new();
    let mut seen = HashSet::new();
    let mut budget = MAX_CLOSURE_BYTES;
    if let Err(skip) = collect_closure(
        &grammar_abs,
        &grammar_base,
        &grammar_base,
        &mut closure,
        &mut seen,
        0,
        &mut budget,
    ) {
        log::debug!("runner cache skipped: {}", skip.0);
        return None;
    }

    let mut hasher = Sha256::new();
    hasher.update(b"adze-runner-candidate-v2\0");
    hasher.update(RUNNER_PROTOCOL_VERSION.to_le_bytes());
    hasher.update(b"profile=debug\0features=serialization,glr\0");
    hasher.update(env!("CARGO_PKG_VERSION").as_bytes());
    hasher.update(b"\0module=");
    hasher.update(spec.module.as_bytes());
    hash_str(&mut hasher, &spec.cargo_toml);
    hash_str(&mut hasher, &spec.build_rs);
    hash_str(&mut hasher, &spec.main_rs);
    // Closure pairs are collected in deterministic (sorted) order; the
    // root-file bytes are always included, so same mtime with changed
    // content is a miss and root-only equality cannot collide across
    // different relative resolutions.
    for (relative, bytes) in &closure {
        hasher.update(b"\0file=");
        hasher.update(relative.as_bytes());
        hasher.update(b"\0sha256=");
        hasher.update(hash_bytes(bytes));
    }
    Some(hasher)
}

/// Finish the same partial key with an explicitly acquired fingerprint.
fn finish_candidate_key(mut hasher: Sha256, toolchain: &str) -> String {
    hasher.update(b"\0rustc=");
    hasher.update(toolchain.as_bytes());
    hash_opt_env(&mut hasher, "RUSTFLAGS");
    hash_opt_env(&mut hasher, "CARGO_BUILD_TARGET");
    hex_digest(hasher.finalize())
}

/// Collect one grammar file plus every reachable sibling module/include.
fn collect_closure(
    file: &Path,
    root_base: &Path,
    module_dir: &Path,
    closure: &mut Vec<(String, Vec<u8>)>,
    seen: &mut HashSet<PathBuf>,
    depth: u8,
    budget: &mut u64,
) -> Result<(), CacheSkip> {
    if depth > 8 {
        return Err(CacheSkip("grammar module nesting too deep".to_string()));
    }
    if closure.len() >= MAX_CLOSURE_FILES {
        return Err(CacheSkip("grammar closure exceeds file budget".to_string()));
    }
    let canonical = file.canonicalize().map_err(|error| {
        CacheSkip(format!(
            "cannot resolve grammar file {}: {error}",
            file.display()
        ))
    })?;
    if !seen.insert(canonical.clone()) {
        return Ok(());
    }
    let bytes = runner_cache_io::read_regular(&canonical, *budget).map_err(|error| {
        CacheSkip(format!(
            "cannot read grammar file {}: {error}",
            canonical.display()
        ))
    })?;
    *budget = budget
        .checked_sub(bytes.len() as u64)
        .ok_or_else(|| CacheSkip("grammar closure exceeds byte budget".to_string()))?;
    let relative = canonical
        .strip_prefix(root_base)
        .map(path_suffix_rooted)
        .unwrap_or_else(|_| canonical.display().to_string());
    let parsed = syn::parse_file(&String::from_utf8_lossy(&bytes)).map_err(|error| {
        CacheSkip(format!(
            "cannot parse {} for module closure: {error}",
            canonical.display()
        ))
    })?;
    closure.push((relative, bytes));
    // Keep the scan order deterministic: sort before hashing at the end.
    let base_dir = canonical
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| root_base.to_path_buf());
    let mut nested = Vec::new();
    collect_mods(&parsed.items, module_dir, &mut nested)?;
    nested.sort();
    nested.dedup();
    for (target, child_dir) in nested {
        collect_closure(
            &target,
            root_base,
            &child_dir,
            closure,
            seen,
            depth + 1,
            budget,
        )?;
    }
    scan_includes(&parsed, &base_dir, root_base, closure, seen, budget)?;
    closure.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(())
}

/// Resolve ordinary and inline modules using Rust's module directories.
/// Explicit paths in inline modules and conditional path overrides are refused.
fn collect_mods(
    items: &[syn::Item],
    module_dir: &Path,
    nested: &mut Vec<(PathBuf, PathBuf)>,
) -> Result<(), CacheSkip> {
    for item in items {
        let syn::Item::Mod(module) = item else {
            continue;
        };
        if module
            .attrs
            .iter()
            .any(|attr| attr.path().is_ident("cfg_attr"))
        {
            return Err(CacheSkip(
                "conditional module paths require Cargo validation".into(),
            ));
        }
        if let Some(content) = &module.content {
            if path_attr_literal(&module.attrs).is_some() {
                return Err(CacheSkip(
                    "inline module path override requires Cargo validation".into(),
                ));
            }
            collect_mods(
                &content.1,
                &module_dir.join(module.ident.to_string()),
                nested,
            )?;
        } else if let Some(literal) = path_attr_literal(&module.attrs) {
            let target = module_dir.join(literal);
            if !target.is_file() {
                return Err(CacheSkip("explicit module path is not a file".into()));
            }
            let child_dir = target
                .parent()
                .ok_or_else(|| CacheSkip("missing module parent".into()))?
                .to_path_buf();
            nested.push((target, child_dir));
        } else {
            let name = module.ident.to_string();
            let child_dir = module_dir.join(&name);
            let candidates = [
                module_dir.join(format!("{name}.rs")),
                child_dir.join("mod.rs"),
            ];
            let found: Vec<_> = candidates
                .into_iter()
                .filter(|path| path.is_file())
                .collect();
            if found.len() != 1 {
                return Err(CacheSkip("missing or ambiguous external module".into()));
            }
            nested.push((found[0].clone(), child_dir));
        }
    }
    Ok(())
}

/// Extract a `#[path = "..."]` literal from attributes, if present.
fn path_attr_literal(attrs: &[syn::Attribute]) -> Option<String> {
    for attr in attrs {
        if !attr.path().is_ident("path") {
            continue;
        }
        if let syn::Meta::NameValue(pair) = &attr.meta
            && let syn::Expr::Lit(syn::ExprLit {
                lit: syn::Lit::Str(lit),
                ..
            }) = &pair.value
        {
            return Some(lit.value());
        }
    }
    None
}

/// Hash literal raw-data includes. Rust include! fragments and dynamic
/// arguments require Cargo validation because their contexts are unresolved.
fn scan_includes(
    file: &syn::File,
    base_dir: &Path,
    root_base: &Path,
    closure: &mut Vec<(String, Vec<u8>)>,
    seen: &mut HashSet<PathBuf>,
    budget: &mut u64,
) -> Result<(), CacheSkip> {
    struct Includes {
        literals: Vec<(String, bool)>,
        dynamic: bool,
    }
    impl<'ast> syn::visit::Visit<'ast> for Includes {
        fn visit_macro(&mut self, node: &'ast syn::Macro) {
            if node.path.segments.last().is_some_and(|segment| {
                matches!(
                    segment.ident.to_string().as_str(),
                    "include" | "include_str" | "include_bytes"
                )
            }) {
                match node.parse_body::<syn::LitStr>() {
                    Ok(literal) => self
                        .literals
                        .push((literal.value(), node.path.is_ident("include"))),
                    Err(_) => self.dynamic = true,
                }
            }
            syn::visit::visit_macro(self, node);
        }
    }
    let mut found = Includes {
        literals: Vec::new(),
        dynamic: false,
    };
    syn::visit::Visit::visit_file(&mut found, file);
    if found.dynamic {
        return Err(CacheSkip(
            "grammar uses dynamic include! arguments that cannot be fingerprinted".to_string(),
        ));
    }
    for (literal, rust_source) in found.literals {
        let target = base_dir.join(literal);
        if rust_source {
            // include! can inject modules at the invocation's module context;
            // expression/type fragments and nested contexts need Cargo validation.
            return Err(CacheSkip("Rust include! requires Cargo validation".into()));
        }
        let canonical = target
            .canonicalize()
            .map_err(|error| CacheSkip(error.to_string()))?;
        if !seen.insert(canonical.clone()) {
            continue;
        }
        if closure.len() >= MAX_CLOSURE_FILES {
            return Err(CacheSkip("grammar closure exceeds file budget".into()));
        }
        let bytes = runner_cache_io::read_regular(&canonical, *budget)
            .map_err(|error| CacheSkip(error.to_string()))?;
        *budget = budget
            .checked_sub(bytes.len() as u64)
            .ok_or_else(|| CacheSkip("grammar closure exceeds byte budget".into()))?;
        closure.push((path_suffix(&canonical, root_base), bytes));
    }
    Ok(())
}

/// Cache only successful release/host text, never errors or unavailable probes.
/// A concurrent first probe refuses immediately instead of waiting without a
/// deadline or starting a second capture owner in the same process.
pub(crate) fn rustc_fingerprint(process: &ProcessRunner) -> anyhow::Result<Option<String>> {
    static FINGERPRINT: Mutex<Option<String>> = Mutex::new(None);
    memoized_fingerprint(&FINGERPRINT, || {
        let invocation = RunnerInvocation {
            program: "rustc".into(),
            args: vec!["-vV".into()],
            environment: Vec::new(),
        };
        let output = process.capture(&invocation).context("rustc fingerprint")?;
        if !output.status.success() {
            // A completed natural nonzero means no candidate identity. This is
            // not a process refusal, and cannot enable production cache reuse.
            return Ok(None);
        }
        let text = String::from_utf8_lossy(&output.stdout);
        let mut selected = String::new();
        for line in text.lines() {
            if line.starts_with("release:") || line.starts_with("host:") {
                selected.push_str(line.trim());
                selected.push('\n');
            }
        }
        Ok((!selected.is_empty()).then_some(selected))
    })
}

fn memoized_fingerprint(
    cache: &Mutex<Option<String>>,
    probe: impl FnOnce() -> anyhow::Result<Option<String>>,
) -> anyhow::Result<Option<String>> {
    let mut fingerprint = cache
        .try_lock()
        .map_err(|error| match error {
            TryLockError::WouldBlock => {
                anyhow::anyhow!("rustc fingerprint is already being acquired")
            }
            TryLockError::Poisoned(_) => {
                anyhow::anyhow!("rustc fingerprint coordination is poisoned")
            }
        })
        .context(ProcessFailure)?;
    if let Some(value) = &*fingerprint {
        return Ok(Some(value.clone()));
    }
    let value = probe()?;
    if value.is_some() {
        *fingerprint = value.clone();
    }
    Ok(value)
}

/// A complete, verified cache entry.
pub(crate) struct ValidEntry {
    exe: PathBuf,
}

/// Validate a cache entry before executing anything from it.
///
/// Every check fails closed: wrong name shape, unreadable or corrupt
/// manifest, protocol mismatch, key mismatch, stale TTL, missing or
/// hash-mismatched executable all yield `None` without side effects.
pub(crate) fn validate_entry(root: &Path, key: &str) -> Option<ValidEntry> {
    if !is_entry_name(key) {
        return None;
    }
    let dir = root.join(key);
    if !std::fs::symlink_metadata(&dir).ok()?.is_dir()
        || std::fs::symlink_metadata(dir.join("manifest.json"))
            .ok()?
            .file_type()
            .is_symlink()
    {
        return None;
    }
    let manifest_bytes =
        runner_cache_io::read_regular(&dir.join("manifest.json"), MAX_MANIFEST_BYTES).ok()?;
    let manifest: serde_json::Value = serde_json::from_slice(&manifest_bytes).ok()?;
    if manifest.get("protocol").and_then(serde_json::Value::as_u64)
        != Some(u64::from(RUNNER_PROTOCOL_VERSION))
    {
        return None;
    }
    if manifest.get("key").and_then(serde_json::Value::as_str) != Some(key) {
        return None;
    }
    let built = manifest
        .get("built_unix")
        .and_then(serde_json::Value::as_u64)?;
    let now = SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_secs();
    if built > now || now - built > ENTRY_TTL_SECS {
        return None;
    }
    let exe_name = manifest
        .get("exe")
        .and_then(|exe| exe.get("file"))
        .and_then(serde_json::Value::as_str)?;
    if exe_name.contains(['/', '\\']) || exe_name.contains("..") {
        return None;
    }
    let expected_len = manifest
        .get("exe")
        .and_then(|exe| exe.get("len"))
        .and_then(serde_json::Value::as_u64)?;
    if expected_len == 0 || expected_len > MAX_EXECUTABLE_BYTES {
        return None;
    }
    let expected_sha = manifest
        .get("exe")
        .and_then(|exe| exe.get("sha256"))
        .and_then(serde_json::Value::as_str)?;
    let exe = dir.join(exe_name);
    if !std::fs::symlink_metadata(&exe).ok()?.is_file() {
        return None;
    }
    let digest = runner_cache_io::digest_regular(&exe, expected_len).ok()?;
    if digest.len != expected_len || hex_digest(digest.sha256) != expected_sha {
        return None;
    }
    Some(ValidEntry { exe })
}

/// Exclusively create the per-key coordination lock.
fn claim_lock(lock_path: &Path) -> bool {
    match std::fs::File::create_new(lock_path) {
        Ok(file) => {
            use std::io::Write as _;
            let _ = writeln!(
                &mut &file,
                "pid={} time={}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|elapsed| elapsed.as_secs())
                    .unwrap_or(0)
            );
            true
        }
        Err(_) => false,
    }
}

/// Wait for another invocation's build, then use its entry if it validates.
fn wait_for_entry(root: &Path, lock_path: &Path, key: &str) -> Option<ValidEntry> {
    if lock_is_stale(lock_path) {
        return validate_entry(root, key);
    }
    let started = std::time::Instant::now();
    while started.elapsed().as_secs() < BUILD_WAIT_SECS {
        if let Some(entry) = validate_entry(root, key) {
            return Some(entry);
        }
        if !lock_path.exists() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(250));
    }
    validate_entry(root, key)
}

/// A missing lock is not stale; only an old one is.
fn lock_is_stale(lock_path: &Path) -> bool {
    let Ok(metadata) = std::fs::symlink_metadata(lock_path) else {
        return false;
    };
    let Ok(modified) = metadata.modified() else {
        return false;
    };
    let Ok(age) = SystemTime::now().duration_since(modified) else {
        return false;
    };
    age.as_secs() > STALE_LOCK_SECS
}

/// Exclusively claim a staging directory owned by this process.
pub(crate) fn claim_staging_dir(root: &Path) -> Option<PathBuf> {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let pid = std::process::id();
    for _ in 0..100 {
        let candidate = root.join(format!(
            ".staging-{pid}-{}",
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        // `create_dir` fails when the path exists, so a colliding path is
        // never removed or reused.
        if std::fs::create_dir(&candidate).is_ok() {
            return Some(candidate);
        }
    }
    None
}

/// Build in owned staging, verify, and publish atomically.
pub(crate) fn build_and_publish(
    spec: &RunnerSpec<'_>,
    config: &CacheConfig,
    key: &str,
    staging: &Path,
    process: &ProcessRunner,
) -> anyhow::Result<CachedBuild> {
    let src_dir = staging.join("src");
    std::fs::create_dir_all(&src_dir).context(CacheStorageFailure)?;
    std::fs::write(staging.join("Cargo.toml"), &spec.cargo_toml).context(CacheStorageFailure)?;
    std::fs::write(staging.join("build.rs"), &spec.build_rs).context(CacheStorageFailure)?;
    std::fs::write(src_dir.join("main.rs"), &spec.main_rs).context(CacheStorageFailure)?;

    eprintln!("compiling parse runner...");
    note_cargo_spawn();
    let invocation = RunnerInvocation {
        program: "cargo".into(),
        args: vec![
            "build".into(),
            "--message-format=json-render-diagnostics".into(),
            "--manifest-path".into(),
            staging.join("Cargo.toml").into_os_string(),
        ],
        environment: vec![
            ("ADZE_USE_PURE_RUST".into(), "1".into()),
            (
                "CARGO_TARGET_DIR".into(),
                staging.join("target").into_os_string(),
            ),
        ],
    };
    let output = process
        .capture(&invocation)
        .context("cached runner Cargo build")?;
    if !output.status.success() {
        return Err(anyhow::Error::new(CargoFailed {
            status: output.status,
            stdout: output.stdout,
            stderr: output.stderr,
        }));
    }
    let exe_name = format!("adze-cli-parse-runner{}", std::env::consts::EXE_SUFFIX);
    let Some(built) = cargo_executable(&output.stdout) else {
        return Ok(CachedBuild::Unavailable);
    };
    // Files can change during Cargo's read. Refuse publication unless the
    // observed grammar/template candidate still matches after compilation.
    if grammar_candidate_key(spec, process)?.as_deref() != Some(key) {
        return Ok(CachedBuild::Unavailable);
    }
    // Assemble the complete entry inside staging and commit it with a single
    // rename, so readers only ever see absent-or-complete entries.
    let entry = staging.join("entry");
    std::fs::create_dir_all(&entry).context(CacheStorageFailure)?;
    let executable = entry.join(&exe_name);
    let copied = runner_cache_io::copy_regular(&built, &executable, MAX_EXECUTABLE_BYTES)
        .context(CacheStorageFailure)?;
    if copied.len == 0
        || runner_cache_io::digest_regular(&executable, MAX_EXECUTABLE_BYTES)
            .context(CacheStorageFailure)?
            != copied
    {
        return Ok(CachedBuild::Unavailable);
    }
    let built_unix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or(0);
    let manifest = serde_json::json!({
        "protocol": RUNNER_PROTOCOL_VERSION,
        "key": key,
        "cli_version": env!("CARGO_PKG_VERSION"),
        "built_unix": built_unix,
        "exe": {"file": exe_name, "sha256": hex_digest(copied.sha256), "len": copied.len},
    });
    std::fs::write(
        entry.join("manifest.json"),
        serde_json::to_string_pretty(&manifest).context(CacheStorageFailure)?,
    )
    .context(CacheStorageFailure)?;
    let lock_src = staging.join("Cargo.lock");
    if lock_src.is_file() {
        runner_cache_io::copy_regular(&lock_src, &entry.join("Cargo.lock"), MAX_LOCKFILE_BYTES)
            .context(CacheStorageFailure)?;
    }
    let tail = tail_bytes(&output.stderr, BUILD_LOG_TAIL);
    let _ = std::fs::write(entry.join("build.log"), tail);

    let target = config.root.join(key);
    match std::fs::rename(&entry, &target) {
        Ok(()) => {}
        Err(_) => {
            // A rival published first (rename onto an existing directory
            // fails): adopt its entry when valid.
            if let Some(rival) = validate_entry(&config.root, key) {
                return Ok(CachedBuild::Ready(rival.exe));
            }
            // Otherwise the target is corrupt cache-owned garbage, not a
            // live build (live builds assemble in staging and this lock
            // serializes publishers): move it aside and publish our
            // verified entry in its place.
            retire_entry(&config.root, &target);
            if std::fs::rename(&entry, &target).is_err() {
                if let Some(rival) = validate_entry(&config.root, key) {
                    return Ok(CachedBuild::Ready(rival.exe));
                }
                return Ok(CachedBuild::Unavailable);
            }
        }
    }
    enforce_bounds(&config.root, config.max_entries);
    Ok(CachedBuild::Ready(target.join(&exe_name)))
}

/// Keep the cache bounded without racing active builds.
///
/// Only cache-owned names are eligible: `<64-hex>` entry directories,
/// `.trash-*` leftovers, and stale `<64-hex>.lock` files. Anything else —
/// including other invocations' `.staging-*` directories — is preserved.
/// Fresh entries are never evicted; removal failures (notably a running
/// executable on Windows) leave trash behind for a later pass.
fn enforce_bounds(root: &Path, max_entries: usize) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    let mut ranked: Vec<(u64, PathBuf)> = Vec::new();
    let mut trash: Vec<PathBuf> = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if is_owned_trash_name(&name) {
            trash.push(entry.path());
            continue;
        }
        if name.strip_suffix(".lock").is_some_and(is_entry_name) {
            if lock_is_stale(&entry.path()) {
                let _ = std::fs::remove_file(entry.path());
            }
            continue;
        }
        if !is_entry_name(&name) {
            continue;
        }
        let built =
            runner_cache_io::read_regular(&entry.path().join("manifest.json"), MAX_MANIFEST_BYTES)
                .ok()
                .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
                .and_then(|manifest| {
                    manifest
                        .get("built_unix")
                        .and_then(serde_json::Value::as_u64)
                })
                .unwrap_or(0);
        ranked.push((built, entry.path()));
    }
    ranked.sort();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or(0);
    let excess = ranked.len().saturating_sub(max_entries);
    for (_, path) in ranked.into_iter().take(excess) {
        let fresh = runner_cache_io::read_regular(&path.join("manifest.json"), MAX_MANIFEST_BYTES)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
            .and_then(|manifest| {
                manifest
                    .get("built_unix")
                    .and_then(serde_json::Value::as_u64)
            })
            .is_some_and(|built| built > now || now - built < ACTIVE_GRACE_SECS);
        if fresh {
            continue;
        }
        retire_entry(root, &path);
    }
    // Retry orphaned trash from earlier sharing races, then only if old.
    for path in trash {
        let old = std::fs::symlink_metadata(&path)
            .ok()
            .and_then(|metadata| metadata.modified().ok())
            .and_then(|modified| SystemTime::now().duration_since(modified).ok())
            .is_some_and(|age| age.as_secs() > ACTIVE_GRACE_SECS);
        if old {
            let _ = std::fs::remove_dir_all(&path);
        }
    }
}

fn is_entry_name(name: &str) -> bool {
    name.len() == ENTRY_NAME_LEN
        && name
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn is_owned_trash_name(name: &str) -> bool {
    let Some(suffix) = name.strip_prefix(".trash-") else {
        return false;
    };
    let Some((pid, counter)) = suffix.split_once('-') else {
        return false;
    };
    !pid.is_empty()
        && !counter.is_empty()
        && pid.bytes().all(|byte| byte.is_ascii_digit())
        && counter.bytes().all(|byte| byte.is_ascii_digit())
}

/// Move an entry aside before deleting so a concurrent validator never sees
/// a half-removed directory; deletion failures are left for a later pass.
fn retire_entry(root: &Path, path: &Path) {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let pid = std::process::id();
    for _ in 0..100 {
        let trash = root.join(format!(
            ".trash-{pid}-{}",
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        // Claim an enclosing directory exclusively before moving anything.
        // rename alone could replace somebody else's existing empty directory.
        if std::fs::create_dir(&trash).is_err() {
            continue;
        }
        if std::fs::rename(path, trash.join("entry")).is_ok() {
            let _ = std::fs::remove_dir_all(&trash);
        } else {
            let _ = std::fs::remove_dir(&trash);
        }
        return;
    }
}

/// Record one Cargo spawn for the test-only invocation counter.
pub(crate) fn note_cargo_spawn() {
    if let Ok(path) = std::env::var(COUNT_ENV)
        && !path.is_empty()
    {
        use std::io::Write as _;
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
        {
            let _ = writeln!(file, "cargo");
        }
    }
}

fn hash_str(hasher: &mut Sha256, value: &str) {
    hasher.update(b"\0str=");
    hasher.update(value.as_bytes());
}

fn hash_bytes(bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher.finalize().into()
}

fn hash_opt_env(hasher: &mut Sha256, name: &str) {
    hasher.update(b"\0env=");
    hasher.update(name.as_bytes());
    hasher.update(b"=");
    if let Ok(value) = std::env::var(name) {
        hasher.update(b"1\0");
        hasher.update(value.as_bytes());
    } else {
        hasher.update(b"0\0");
    }
}

fn hex_digest(digest: impl AsRef<[u8]>) -> String {
    let mut out = String::with_capacity(64);
    for byte in digest.as_ref() {
        out.push(char::from_digit(u32::from(*byte) >> 4, 16).unwrap_or('0'));
        out.push(char::from_digit(u32::from(*byte) & 0xF, 16).unwrap_or('0'));
    }
    out
}

/// Slash-normalized path relative to the grammar base directory.
fn path_suffix_rooted(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// Slash-normalized path of `file` below `base`.
fn path_suffix(file: &Path, base: &Path) -> String {
    file.strip_prefix(base)
        .map(path_suffix_rooted)
        .unwrap_or_else(|_| file.display().to_string())
}

fn tail_bytes(bytes: &[u8], limit: usize) -> Vec<u8> {
    if bytes.len() <= limit {
        return bytes.to_vec();
    }
    bytes[bytes.len() - limit..].to_vec()
}

/// Cargo reports the actual selected executable path, including target triples
/// and target-dir configuration. Never guess the layout or a shared filename.
fn cargo_executable(stdout: &[u8]) -> Option<PathBuf> {
    let mut executable = None;
    for line in stdout.split(|byte| *byte == b'\n') {
        let Ok(message) = serde_json::from_slice::<serde_json::Value>(line) else {
            continue;
        };
        if message["reason"] != "compiler-artifact"
            || message["target"]["name"] != "adze-cli-parse-runner"
            || !message["target"]["kind"]
                .as_array()?
                .iter()
                .any(|kind| kind == "bin")
        {
            continue;
        }
        let path = message["executable"].as_str()?;
        if executable.replace(PathBuf::from(path)).is_some() {
            return None;
        }
    }
    executable
}

#[cfg(test)]
mod tests {
    use super::RUNNER_PROTOCOL_VERSION;
    use std::path::{Path, PathBuf};

    fn test_spec<'a>(grammar: &'a Path, module: &'a str) -> super::RunnerSpec<'a> {
        super::RunnerSpec {
            grammar,
            module,
            cargo_toml: "[package]\nname = \"t\"\n".to_string(),
            build_rs: "fn main() {}".to_string(),
            main_rs: format!("// {module}"),
        }
    }

    // Independent identity input: these tests exercise path/content hashing,
    // while the required native controller acquires real rustc/build output.
    fn candidate_key(spec: &super::RunnerSpec<'_>) -> Option<String> {
        Some(super::finish_candidate_key(
            super::candidate_hasher(spec)?,
            "release: fixture\nhost: fixture\n",
        ))
    }

    fn write_grammar(dir: &Path, name: &str, content: &str) -> PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, content).expect("write grammar fixture");
        path
    }

    const SINGLE: &str = "#[adze::grammar(\"tiny\")]\npub mod grammar {\n}\n";

    #[test]
    fn incomplete_dependency_custody_never_admits_production_reuse() {
        let temp = tempfile::tempdir().expect("tempdir");
        let grammar = write_grammar(temp.path(), "grammar.rs", SINGLE);
        let spec = test_spec(&grammar, "grammar");
        assert!(candidate_key(&spec).is_some());
        assert!(
            super::build_key(&spec).is_none(),
            "partial candidate identity cannot authorize execution"
        );
    }

    #[test]
    fn nested_modules_hash_the_actual_child_not_a_sibling_decoy() {
        let temp = tempfile::tempdir().expect("tempdir");
        let grammar = write_grammar(
            temp.path(),
            "grammar.rs",
            "mod foo;
",
        );
        write_grammar(
            temp.path(),
            "foo.rs",
            "mod bar;
",
        );
        write_grammar(
            temp.path(),
            "bar.rs",
            "const V: u8 = 99;
",
        );
        std::fs::create_dir(temp.path().join("foo")).expect("module dir");
        let child = write_grammar(
            &temp.path().join("foo"),
            "bar.rs",
            "const V: u8 = 1;
",
        );
        let before = candidate_key(&test_spec(&grammar, "grammar")).expect("key");
        std::fs::write(
            &child,
            "const V: u8 = 2;
",
        )
        .expect("edit actual child");
        let after = candidate_key(&test_spec(&grammar, "grammar")).expect("key");
        assert_ne!(before, after);
        write_grammar(
            temp.path(),
            "bar.rs",
            "const V: u8 = 100;
",
        );
        assert_eq!(
            after,
            candidate_key(&test_spec(&grammar, "grammar")).expect("key")
        );
    }

    #[test]
    fn inline_modules_use_their_module_directory() {
        let temp = tempfile::tempdir().expect("tempdir");
        let grammar = write_grammar(
            temp.path(),
            "grammar.rs",
            "mod foo { mod bar; }
",
        );
        std::fs::create_dir(temp.path().join("foo")).expect("module dir");
        let child = write_grammar(
            &temp.path().join("foo"),
            "bar.rs",
            "const V: u8 = 1;
",
        );
        let before = candidate_key(&test_spec(&grammar, "grammar")).expect("key");
        std::fs::write(
            child,
            "const V: u8 = 2;
",
        )
        .expect("edit inline child");
        assert_ne!(
            before,
            candidate_key(&test_spec(&grammar, "grammar")).expect("key")
        );
    }

    #[test]
    fn raw_text_and_binary_includes_hash_bytes_without_rust_parsing() {
        let temp = tempfile::tempdir().expect("tempdir");
        let grammar = write_grammar(
            temp.path(),
            "grammar.rs",
            "const T: &str = include_str!(\"data.txt\"); const B: &[u8] = include_bytes!(\"data.bin\");",
        );
        std::fs::write(temp.path().join("data.txt"), "this is not Rust!").expect("text");
        std::fs::write(temp.path().join("data.bin"), [255, 0, 128]).expect("binary");
        let before = candidate_key(&test_spec(&grammar, "grammar")).expect("raw include key");
        std::fs::write(temp.path().join("data.bin"), [254, 0, 128]).expect("changed binary");
        assert_ne!(
            before,
            candidate_key(&test_spec(&grammar, "grammar")).expect("key")
        );
    }

    #[test]
    fn conditional_path_overrides_refuse_candidate_reuse() {
        let temp = tempfile::tempdir().expect("tempdir");
        let grammar = write_grammar(
            temp.path(),
            "grammar.rs",
            "#[cfg_attr(unix, path = \"other.rs\")] mod foo;",
        );
        write_grammar(temp.path(), "foo.rs", "");
        write_grammar(temp.path(), "other.rs", "");
        assert!(candidate_key(&test_spec(&grammar, "grammar")).is_none());
    }

    #[cfg(unix)]
    #[test]
    fn private_cache_root_rejects_writable_roots_and_symlinks() {
        use std::os::unix::fs::{PermissionsExt, symlink};
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path().join("private");
        assert!(super::prepare_private_root(&root));
        let link = temp.path().join("link");
        symlink(&root, &link).expect("symlink");
        assert!(!super::prepare_private_root(&link));
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o777))
            .expect("permissions");
        assert!(!super::prepare_private_root(&root));
        assert_eq!(
            std::fs::metadata(&root)
                .expect("metadata")
                .permissions()
                .mode()
                & 0o777,
            0o777,
            "never chmod an untrusted existing directory"
        );
        let insecure_parent = temp.path().join("insecure");
        std::fs::create_dir(&insecure_parent).expect("parent");
        std::fs::set_permissions(&insecure_parent, std::fs::Permissions::from_mode(0o777))
            .expect("parent mode");
        assert!(!super::prepare_private_root(&insecure_parent.join("cache")));
    }

    #[cfg(unix)]
    #[test]
    fn self_consistent_entry_symlinks_are_not_trusted() {
        use std::os::unix::fs::symlink;
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path().join("cache");
        std::fs::create_dir(&root).expect("root");
        let key = "d".repeat(64);
        let entry = root.join(&key);
        std::fs::create_dir(&entry).expect("entry");
        let bytes = b"runner bytes";
        std::fs::write(entry.join("runner"), bytes).expect("runner");
        let manifest = serde_json::json!({
            "protocol": RUNNER_PROTOCOL_VERSION, "key": key,
            "built_unix": super::SystemTime::now().duration_since(super::UNIX_EPOCH).expect("now").as_secs(),
            "exe": {"file": "runner", "sha256": super::hex_digest(super::hash_bytes(bytes)), "len": bytes.len()},
        });
        std::fs::write(
            entry.join("manifest.json"),
            serde_json::to_vec(&manifest).expect("json"),
        )
        .expect("manifest");
        assert!(super::validate_entry(&root, &key).is_some());
        let outside = temp.path().join("outside-entry");
        std::fs::rename(&entry, &outside).expect("move entry");
        symlink(&outside, &entry).expect("entry symlink");
        assert!(super::validate_entry(&root, &key).is_none());
        std::fs::remove_file(&entry).expect("unlink entry");
        std::fs::rename(&outside, &entry).expect("restore entry");
        let external_runner = temp.path().join("external-runner");
        std::fs::rename(entry.join("runner"), &external_runner).expect("move runner");
        symlink(&external_runner, entry.join("runner")).expect("runner symlink");
        assert!(super::validate_entry(&root, &key).is_none());
    }

    #[test]
    fn staging_failure_never_returns_an_unclaimed_path() {
        let temp = tempfile::tempdir().expect("tempdir");
        let file = temp.path().join("not-a-directory");
        std::fs::write(&file, "sentinel").expect("file");
        assert!(super::claim_staging_dir(&file).is_none());
        assert_eq!(std::fs::read_to_string(file).expect("file"), "sentinel");
    }

    #[test]
    fn cargo_artifact_messages_select_the_reported_target_executable() {
        let message = serde_json::json!({"reason": "compiler-artifact", "target": {"name": "adze-cli-parse-runner", "kind": ["bin"]}, "executable": "/configured/target/host-triple/debug/adze-cli-parse-runner"});
        let stdout = serde_json::to_vec(&message).expect("json");
        assert_eq!(
            super::cargo_executable(&stdout),
            Some(PathBuf::from(
                "/configured/target/host-triple/debug/adze-cli-parse-runner"
            ))
        );
        assert!(super::cargo_executable(b"not an artifact").is_none());
        let duplicate = [stdout.as_slice(), b"\n", stdout.as_slice()].concat();
        assert!(
            super::cargo_executable(&duplicate).is_none(),
            "ambiguous artifacts require Cargo fallback"
        );
    }

    #[test]
    fn key_is_stable_and_content_addressed() {
        let temp = tempfile::tempdir().expect("tempdir");
        let grammar = write_grammar(temp.path(), "grammar.rs", SINGLE);
        let first = candidate_key(&test_spec(&grammar, "grammar")).expect("key");
        let second = candidate_key(&test_spec(&grammar, "grammar")).expect("key");
        assert_eq!(first, second, "same bytes must produce the same key");
        assert_eq!(first.len(), 64, "key must be a sha256 hex digest");
    }

    #[test]
    fn test_grammar_and_raw_include_reads_share_the_remaining_byte_budget() {
        let temp = tempfile::tempdir().expect("tempdir");
        let grammar = temp.path().join("grammar.rs");
        std::fs::File::create(&grammar)
            .unwrap()
            .set_len(super::MAX_CLOSURE_BYTES + 1)
            .unwrap();
        assert!(candidate_key(&test_spec(&grammar, "grammar")).is_none());

        std::fs::write(
            &grammar,
            format!("{SINGLE}const DATA: &[u8] = include_bytes!(\"data.bin\");\n"),
        )
        .unwrap();
        let included = temp.path().join("data.bin");
        std::fs::File::create(&included)
            .unwrap()
            .set_len(super::MAX_CLOSURE_BYTES)
            .unwrap();
        assert!(candidate_key(&test_spec(&grammar, "grammar")).is_none());
        std::fs::write(&included, b"literal data").unwrap();
        assert!(candidate_key(&test_spec(&grammar, "grammar")).is_some());
        assert!(super::build_key(&test_spec(&grammar, "grammar")).is_none());
    }

    #[test]
    fn test_bounded_entry_validation_refuses_partial_or_oversized_files_without_mutation() {
        let temp = tempfile::tempdir().expect("tempdir");
        let key = "e".repeat(64);
        let entry = temp.path().join(&key);
        std::fs::create_dir(&entry).unwrap();
        let executable = entry.join("runner");
        let manifest_path = entry.join("manifest.json");
        std::fs::write(&executable, b"abc").unwrap();
        let mut manifest = serde_json::json!({
            "protocol": RUNNER_PROTOCOL_VERSION,
            "key": key,
            "built_unix": super::SystemTime::now()
                .duration_since(super::UNIX_EPOCH).unwrap().as_secs(),
            "exe": {
                "file": "runner", "len": 3,
                "sha256": "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
            },
        });
        std::fs::write(&manifest_path, serde_json::to_vec(&manifest).unwrap()).unwrap();
        assert!(super::validate_entry(temp.path(), &key).is_some());
        std::fs::write(&executable, b"abc excess").unwrap();
        assert!(super::validate_entry(temp.path(), &key).is_none());
        assert_eq!(std::fs::read(&executable).unwrap(), b"abc excess");
        std::fs::write(&executable, b"abc").unwrap();

        for invalid_len in [0, super::MAX_EXECUTABLE_BYTES + 1] {
            manifest["exe"]["len"] = invalid_len.into();
            std::fs::write(&manifest_path, serde_json::to_vec(&manifest).unwrap()).unwrap();
            assert!(super::validate_entry(temp.path(), &key).is_none());
        }
        std::fs::write(&manifest_path, b"{\"protocol\":").unwrap();
        assert!(super::validate_entry(temp.path(), &key).is_none());
        assert_eq!(std::fs::read(&manifest_path).unwrap(), b"{\"protocol\":");
        std::fs::File::create(&manifest_path)
            .unwrap()
            .set_len(super::MAX_MANIFEST_BYTES + 1)
            .unwrap();
        assert!(super::validate_entry(temp.path(), &key).is_none());
        assert_eq!(
            std::fs::metadata(&manifest_path).unwrap().len(),
            super::MAX_MANIFEST_BYTES + 1,
        );
        assert_eq!(std::fs::read(&executable).unwrap(), b"abc");
    }

    #[test]
    fn key_ignores_mtime_and_follows_bytes() {
        let temp = tempfile::tempdir().expect("tempdir");
        let grammar = write_grammar(temp.path(), "grammar.rs", SINGLE);
        let before = candidate_key(&test_spec(&grammar, "grammar")).expect("key");
        // Rewrite identical bytes (mtime changes); then change one byte.
        write_grammar(temp.path(), "grammar.rs", SINGLE);
        let same_bytes = candidate_key(&test_spec(&grammar, "grammar")).expect("key");
        assert_eq!(before, same_bytes, "mtime alone must not change the key");
        write_grammar(temp.path(), "grammar.rs", &format!("{SINGLE}// v2\n"));
        let changed = candidate_key(&test_spec(&grammar, "grammar")).expect("key");
        assert_ne!(before, changed, "changed content with any mtime must miss");
    }

    #[test]
    fn key_distinguishes_relative_resolution() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root_a = temp.path().join("a");
        let root_b = temp.path().join("b");
        std::fs::create_dir_all(&root_a).expect("mkdir a");
        std::fs::create_dir_all(&root_b).expect("mkdir b");
        let with_mod = "#[adze::grammar(\"tiny\")]\npub mod grammar {\n}\nmod shared;\n";
        write_grammar(&root_a, "grammar.rs", with_mod);
        write_grammar(&root_b, "grammar.rs", with_mod);
        std::fs::write(root_a.join("shared.rs"), "pub const V: u32 = 1;\n").expect("sibling a");
        std::fs::write(root_b.join("shared.rs"), "pub const V: u32 = 2;\n").expect("sibling b");
        let grammar_a = root_a.join("grammar.rs");
        let grammar_b = root_b.join("grammar.rs");
        let key_a = candidate_key(&test_spec(&grammar_a, "grammar")).expect("key a");
        let key_b = candidate_key(&test_spec(&grammar_b, "grammar")).expect("key b");
        assert_ne!(
            key_a, key_b,
            "identical root bytes with different sibling resolution must not collide"
        );
        // Identical layout and bytes share the key (same binary either way).
        std::fs::write(root_b.join("shared.rs"), "pub const V: u32 = 1;\n").expect("align sibling");
        let key_b_aligned = candidate_key(&test_spec(&grammar_b, "grammar")).expect("key b2");
        assert_eq!(key_a, key_b_aligned);
    }

    #[test]
    fn dynamic_include_refuses_caching() {
        let temp = tempfile::tempdir().expect("tempdir");
        let grammar = write_grammar(
            temp.path(),
            "grammar.rs",
            "#[adze::grammar(\"tiny\")]\npub mod grammar {\n}\nconst H: &str = include!(concat!(\"a\", \".txt\"));\n",
        );
        assert!(
            candidate_key(&test_spec(&grammar, "grammar")).is_none(),
            "dynamic include! inputs cannot be fingerprinted"
        );
    }

    #[test]
    fn missing_module_refuses_caching() {
        let temp = tempfile::tempdir().expect("tempdir");
        let grammar = write_grammar(
            temp.path(),
            "grammar.rs",
            "#[adze::grammar(\"tiny\")]\npub mod grammar {\n}\nmod absent;\n",
        );
        assert!(
            candidate_key(&test_spec(&grammar, "grammar")).is_none(),
            "unresolvable modules must fall back to the legacy build"
        );
    }

    #[test]
    fn corrupt_entry_never_validates() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path().join("cache");
        std::fs::create_dir_all(&root).expect("cache root");
        let key = "a".repeat(64);
        let dir = root.join(&key);
        std::fs::create_dir_all(&dir).expect("entry dir");
        std::fs::write(dir.join("manifest.json"), "not json").expect("garbage manifest");
        std::fs::write(dir.join("runner"), b"bytes").expect("orphan exe");
        assert!(super::validate_entry(&root, &key).is_none());
        // Well-formed manifest with a tampered executable also refuses.
        let manifest = serde_json::json!({
            "protocol": RUNNER_PROTOCOL_VERSION,
            "key": key,
            "built_unix": super::SystemTime::now().duration_since(super::UNIX_EPOCH).expect("now").as_secs(),
            "exe": {"file": "runner", "sha256": "0".repeat(64), "len": 5u64},
        });
        std::fs::write(
            dir.join("manifest.json"),
            serde_json::to_string(&manifest).expect("json"),
        )
        .expect("manifest");
        assert!(super::validate_entry(&root, &key).is_none());
    }

    #[test]
    fn eviction_only_retires_old_owned_entries() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path().join("cache");
        std::fs::create_dir_all(&root).expect("cache root");
        let now = super::SystemTime::now()
            .duration_since(super::UNIX_EPOCH)
            .expect("now")
            .as_secs();
        // Five complete-but-fake entries: four ancient, one fresh.
        for (index, age) in [100u64, 200, 300, 400, 0].into_iter().enumerate() {
            let key = format!("{:064x}", index + 1);
            let dir = root.join(&key);
            std::fs::create_dir_all(&dir).expect("entry dir");
            let manifest = serde_json::json!({
                "protocol": RUNNER_PROTOCOL_VERSION,
                "key": key,
                "built_unix": now.saturating_sub(age + super::ENTRY_TTL_SECS + 10),
                "exe": {"file": "runner", "sha256": "00".repeat(32), "len": 1u64},
            });
            std::fs::write(
                dir.join("manifest.json"),
                serde_json::to_string(&manifest).expect("json"),
            )
            .expect("manifest");
            std::fs::write(dir.join("runner"), b"x").expect("exe");
        }
        // Unrelated files, another invocation's staging, and a fresh lock
        // must all survive eviction.
        std::fs::write(root.join("notes.txt"), "user file").expect("user file");
        let unrelated_hex_length = root.join("z".repeat(64));
        std::fs::create_dir(&unrelated_hex_length).expect("unrelated directory");
        std::fs::write(unrelated_hex_length.join("sentinel"), "keep").expect("sentinel");
        let unrelated_lock = root.join("notes.lock");
        let file = std::fs::File::create(&unrelated_lock).expect("unrelated lock");
        file.set_times(std::fs::FileTimes::new().set_modified(super::UNIX_EPOCH))
            .expect("old timestamp");
        std::fs::create_dir_all(root.join(".staging-999-0")).expect("foreign staging");
        std::fs::write(root.join(".staging-999-0").join("wip"), "wip").expect("wip");
        let live_lock = root.join(format!("{}.lock", "b".repeat(64)));
        std::fs::write(&live_lock, "pid=999").expect("live lock");

        super::enforce_bounds(&root, 2);

        let remaining: Vec<String> = std::fs::read_dir(&root)
            .expect("read cache")
            .flatten()
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .filter(|name| super::is_entry_name(name))
            .collect();
        assert!(
            remaining.len() <= 2,
            "bounded cache must retire excess entries, found {remaining:?}"
        );
        assert!(
            root.join("notes.txt").is_file(),
            "unrelated files are preserved"
        );
        assert!(
            root.join(".staging-999-0").join("wip").is_file(),
            "another invocation's staging is never touched"
        );
        assert!(live_lock.is_file(), "fresh locks are preserved");
        assert!(
            unrelated_hex_length.join("sentinel").is_file(),
            "nonhex names are not entries"
        );
        assert!(
            unrelated_lock.is_file(),
            "unrelated old locks are preserved"
        );
    }

    #[test]
    fn concurrent_same_key_waiters_share_one_entry() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path().join("cache");
        std::fs::create_dir_all(&root).expect("cache root");
        let key = "c".repeat(64);
        let lock = root.join(format!("{key}.lock"));
        assert!(super::claim_lock(&lock), "first claim wins");
        assert!(!super::claim_lock(&lock), "second claim loses while held");

        let waiter_root = root.clone();
        let waiter_key = key.clone();
        let waiter_lock = lock.clone();
        let waiter = std::thread::spawn(move || {
            super::wait_for_entry(&waiter_root, &waiter_lock, &waiter_key)
        });
        // The entry is absent while the owner "builds": the waiter must not
        // observe anything partial, so it keeps waiting.
        std::thread::sleep(std::time::Duration::from_millis(500));
        // Publish a complete, valid entry, then release the lock.
        let dir = root.join(&key);
        std::fs::create_dir_all(&dir).expect("entry dir");
        let bytes = b"fake-runner-bytes";
        std::fs::write(dir.join("runner"), bytes).expect("exe");
        let manifest = serde_json::json!({
            "protocol": super::RUNNER_PROTOCOL_VERSION,
            "key": key,
            "built_unix": super::SystemTime::now().duration_since(super::UNIX_EPOCH).expect("now").as_secs(),
            "exe": {"file": "runner", "sha256": super::hex_digest(super::hash_bytes(bytes)), "len": bytes.len() as u64},
        });
        std::fs::write(
            dir.join("manifest.json"),
            serde_json::to_string(&manifest).expect("json"),
        )
        .expect("manifest");
        std::fs::remove_file(&lock).expect("release lock");

        let shared = waiter
            .join()
            .expect("waiter thread")
            .expect("waiter sees the entry");
        assert_eq!(shared.exe, dir.join("runner"));
        assert!(super::validate_entry(&root, &key).is_some());
    }

    #[test]
    fn staging_claim_never_reuses_collisions() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path().join("cache");
        std::fs::create_dir_all(&root).expect("cache root");
        let pid = std::process::id();
        let taken = root.join(format!(".staging-{pid}-0"));
        std::fs::create_dir_all(&taken).expect("colliding staging");
        std::fs::write(taken.join("sentinel"), "foreign").expect("sentinel");
        let claimed = super::claim_staging_dir(&root).expect("owned staging");
        assert_ne!(claimed, taken);
        assert!(claimed.is_dir());
        assert_eq!(
            std::fs::read_to_string(taken.join("sentinel")).expect("sentinel"),
            "foreign"
        );
    }

    #[test]
    fn test_fingerprint_success_is_the_only_memoized_outcome() {
        use std::cell::Cell;
        use std::sync::Mutex;

        let cache = Mutex::new(None);
        let attempts = Cell::new(0);
        assert_eq!(
            super::memoized_fingerprint(&cache, || {
                attempts.set(attempts.get() + 1);
                Ok(None)
            })
            .unwrap(),
            None
        );
        let error = super::memoized_fingerprint(&cache, || {
            attempts.set(attempts.get() + 1);
            Err(anyhow::anyhow!("fixture refusal").context(super::ProcessFailure))
        })
        .unwrap_err();
        assert!(error.downcast_ref::<super::ProcessFailure>().is_some());
        assert!(cache.try_lock().unwrap().is_none());
        assert_eq!(
            super::memoized_fingerprint(&cache, || {
                attempts.set(attempts.get() + 1);
                Ok(Some("release: fixture\nhost: fixture\n".to_string()))
            })
            .unwrap()
            .as_deref(),
            Some("release: fixture\nhost: fixture\n")
        );
        assert_eq!(
            super::memoized_fingerprint(&cache, || {
                panic!("a successful fingerprint must not launch another probe")
            })
            .unwrap()
            .as_deref(),
            Some("release: fixture\nhost: fixture\n")
        );
        assert_eq!(attempts.get(), 3);
    }

    #[test]
    fn test_fingerprint_busy_coordination_refuses_without_waiting_or_probe() {
        use std::sync::Mutex;

        let cache = Mutex::new(None);
        let _owner = cache.try_lock().unwrap();
        let error = super::memoized_fingerprint(&cache, || {
            panic!("the existing owner excludes a second process probe")
        })
        .unwrap_err();
        assert!(error.downcast_ref::<super::ProcessFailure>().is_some());
        assert!(format!("{error:#}").contains("already being acquired"));
        let Err(error) = super::finish_cached_build(Err(error)) else {
            panic!("fingerprint coordination refusal became cache fallback")
        };
        assert!(error.downcast_ref::<super::ProcessFailure>().is_some());
    }

    #[test]
    fn test_fingerprint_poisoned_coordination_is_a_hard_refusal() {
        use std::sync::{Arc, Mutex};

        let cache = Arc::new(Mutex::new(None));
        let owner = Arc::clone(&cache);
        assert!(
            std::thread::spawn(move || {
                let _held = owner.lock().unwrap();
                panic!("fixture owner panic poisons only this local mutex");
            })
            .join()
            .is_err()
        );
        let error = super::memoized_fingerprint(&cache, || {
            panic!("a poisoned owner must not launch another probe")
        })
        .unwrap_err();
        assert!(error.downcast_ref::<super::ProcessFailure>().is_some());
        assert!(format!("{error:#}").contains("poisoned"));
    }

    #[test]
    fn test_cache_fallback_requires_explicit_storage_failure() {
        let result = super::finish_cached_build(Err(
            anyhow::anyhow!("fixture disk failure").context(super::CacheStorageFailure)
        ))
        .unwrap();
        assert!(matches!(result, super::CachedBuild::Unavailable));
        let Err(unknown) = super::finish_cached_build(Err(anyhow::anyhow!("unknown refusal")))
        else {
            panic!("unknown errors must not become cache fallback")
        };
        assert!(unknown.to_string().contains("unknown refusal"));
        let Err(process) =
            super::finish_cached_build(Err(anyhow::anyhow!("fixture process failure")
                .context(super::ProcessFailure)
                .context(super::CacheStorageFailure)))
        else {
            panic!("outer storage context must not consume a process refusal")
        };
        assert!(process.downcast_ref::<super::ProcessFailure>().is_some());
    }
}
