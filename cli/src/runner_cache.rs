//! Content-addressed cache for compiled static parse runners (issue #1067).
//!
//! A cold supported parse builds one runner through the existing Cargo
//! project generation; a warm parse of (possibly different) input executes
//! the matching binary directly with zero Cargo invocations. Only compiled
//! runners are cached, never parse results, so input text always parses fresh.
//!
//! # Safe reuse envelope
//!
//! The cache key is a content hash over the complete build identity:
//!
//! - grammar file bytes plus every reachable sibling (`mod foo;`,
//!   `#[path = "..."] mod ...;`, literal `include!`/`include_str!`/
//!   `include_bytes!` targets), recorded as `(relative-path, bytes)` pairs;
//! - the rendered runner `Cargo.toml`, `build.rs` and `main.rs` (template +
//!   protocol shape; protocol version [`RUNNER_PROTOCOL_VERSION`] is also
//!   keyed and stored separately);
//! - dependency identity: for in-workspace builds the recursive content of
//!   every `path =` dependency manifest, `build.rs` and `src/**` tree
//!   (covers `runtime`, `tool` and their transitive workspace members); for
//!   published installs the rendered manifest requirement text;
//! - toolchain (`rustc -vV` release/host), host target triple, `debug`
//!   profile, the fixed runner feature set, `CARGO_PKG_VERSION`, and the
//!   `RUSTFLAGS` / `CARGO_BUILD_TARGET` environment values.
//!
//! Neither grammar mtime nor CLI semver alone identifies the executable, so
//! neither is trusted: same mtime with changed bytes is always a miss.
//!
//! # Fail-safe boundaries (explicit non-goals of the key)
//!
//! Anything that cannot be identified reliably disables direct reuse and
//! falls back to the existing temporary-project `cargo run` path with
//! correct output, never a silent stale hit:
//!
//! - single-file grammars cache on the root file bytes; module-backed
//!   grammars additionally hash every resolved sibling file, so different
//!   source paths with identical root bytes but different relative
//!   resolution never collide;
//! - dynamic `include!` arguments, unresolvable modules, unreadable files,
//!   closure/file-count budgets being exceeded, or a missing `rustc`
//!   fingerprint all refuse caching;
//! - registry drift for floating version requirements, `CC`/`CFLAGS`-style
//!   build-script inputs, and arbitrary user build environment beyond the
//!   keyed variables are outside the envelope: entries additionally expire
//!   after [`ENTRY_TTL_SECS`] and eviction keeps the cache bounded.
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
//! #1066 (private runner contract) is still open, so this cache implements
//! the *current* static-runner shape only: the runner prints the selected
//! document projection to stdout and the CLI consumes it exactly as today.
//! [`RUNNER_PROTOCOL_VERSION`] records that shape; a future #1066 contract
//! must bump it, which invalidates every older entry by construction. No new
//! transport is invented here.

use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// Current private runner shape: document projection printed to stdout,
/// consumed with the existing success/exit-status handling. Bump when the
/// #1066 transport contract lands; older entries then miss by construction.
pub(crate) const RUNNER_PROTOCOL_VERSION: u32 = 1;

/// Maximum entries retained; enforced (best-effort) after each publish.
const MAX_ENTRIES_DEFAULT: usize = 32;
/// Entry directory names are 64-character lowercase hex digests.
const ENTRY_NAME_LEN: usize = 64;
/// Entries older than this are treated as misses (bounds registry drift).
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
const MAX_DEP_FILES: usize = 4000;
const MAX_DEP_BYTES: u64 = 128 << 20;
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

/// Reason caching was refused; the caller falls back to the legacy path.
#[derive(Debug)]
struct CacheSkip(String);

/// Resolved cache configuration, or `None` when caching is disabled or the
/// root is unusable (callers then use the legacy temp-project path).
struct CacheConfig {
    root: PathBuf,
    max_entries: usize,
}

/// Look up a verified cached executable without invoking Cargo.
///
/// Returns `None` for disabled cache, unsupported envelopes, misses,
/// stale/corrupt entries, or any cache-internal I/O problem. A returned
/// path names a complete entry whose executable hash was just verified.
pub(crate) fn lookup(spec: &RunnerSpec<'_>) -> Option<PathBuf> {
    let config = cache_config()?;
    let key = build_key(spec)?;
    validate_entry(&config.root, &key).map(|entry| entry.exe)
}

/// Build the runner, publish it atomically, and return its executable.
///
/// `Ok(CachedBuild::Unavailable)` means the caller should use the legacy
/// temp-project path (disabled/unsupported envelope, lost race, stale lock,
/// or any cache-internal I/O failure). `Err` carries either [`CargoFailed`]
/// (downcastable, rendered by the caller) or an unexpected staging failure.
pub(crate) fn build_cached(spec: &RunnerSpec<'_>) -> anyhow::Result<CachedBuild> {
    let Some(config) = cache_config() else {
        return Ok(CachedBuild::Unavailable);
    };
    let Some(key) = build_key(spec) else {
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
    let staging = claim_staging_dir(&config.root);
    let outcome = build_and_publish(spec, &config, &key, &staging);
    let _ = std::fs::remove_file(&lock_path);
    let _ = std::fs::remove_dir_all(&staging);
    outcome
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
    if std::fs::create_dir_all(&root).is_err() {
        return None;
    }
    let max_entries = std::env::var("ADZE_RUNNER_CACHE_MAX_ENTRIES")
        .ok()
        .and_then(|raw| raw.trim().parse::<usize>().ok())
        .filter(|count| *count > 0)
        .unwrap_or(MAX_ENTRIES_DEFAULT);
    Some(CacheConfig { root, max_entries })
}

/// Compute the content-addressed key, or `None` outside the safe envelope.
fn build_key(spec: &RunnerSpec<'_>) -> Option<String> {
    let grammar_abs = spec.grammar.canonicalize().ok()?;
    let grammar_base = grammar_abs.parent()?.to_path_buf();
    let mut closure = Vec::new();
    let mut seen = HashSet::new();
    let mut budget = MAX_CLOSURE_BYTES;
    if let Err(skip) = collect_closure(
        &grammar_abs,
        &grammar_base,
        &mut closure,
        &mut seen,
        0,
        &mut budget,
    ) {
        log::debug!("runner cache skipped: {}", skip.0);
        return None;
    }

    let Some(toolchain) = rustc_fingerprint() else {
        log::debug!("runner cache skipped: rustc fingerprint unavailable");
        return None;
    };
    let mut hasher = Sha256::new();
    hasher.update(b"adze-runner-cache-v1\0");
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
    if hash_dependency_identity(&mut hasher).is_err() {
        log::debug!("runner cache skipped: dependency identity unavailable");
        return None;
    }
    hasher.update(b"\0rustc=");
    hasher.update(toolchain.as_bytes());
    hash_opt_env(&mut hasher, "RUSTFLAGS");
    hash_opt_env(&mut hasher, "CARGO_BUILD_TARGET");
    Some(hex_digest(hasher.finalize()))
}

/// Hash the selected Adze/runtime/tool sources (or version requirements).
fn hash_dependency_identity(hasher: &mut Sha256) -> Result<(), CacheSkip> {
    let cli_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let runtime = cli_dir.join("../runtime").canonicalize().ok();
    let tool = cli_dir.join("../tool").canonicalize().ok();
    let (Some(runtime), Some(tool)) = (runtime, tool) else {
        // Published install: only version requirements are visible; the
        // rendered manifest text (already keyed) plus entry TTL bound drift.
        hasher.update(b"\0deps=version\0");
        return Ok(());
    };
    if !runtime.join("Cargo.toml").is_file() || !tool.join("Cargo.toml").is_file() {
        hasher.update(b"\0deps=version\0");
        return Ok(());
    }
    hasher.update(b"\0deps=path\0");
    let mut visited = HashSet::new();
    let mut budget = FileBudget::new(MAX_DEP_FILES, MAX_DEP_BYTES);
    hash_path_dep_tree(&runtime, &mut budget, &mut visited, 0)?;
    hash_path_dep_tree(&tool, &mut budget, &mut visited, 0)?;
    Ok(())
}

/// Best-effort content walk of one `path =` dependency tree.
fn hash_path_dep_tree(
    dir: &Path,
    budget: &mut FileBudget,
    visited: &mut HashSet<PathBuf>,
    depth: u8,
) -> Result<(), CacheSkip> {
    if depth > 16 {
        return Err(CacheSkip("dependency tree too deep".to_string()));
    }
    let canonical = dir.canonicalize().map_err(|error| {
        CacheSkip(format!(
            "cannot canonicalize dep dir {}: {error}",
            dir.display()
        ))
    })?;
    if !visited.insert(canonical.clone()) {
        return Ok(());
    }
    let manifest_path = canonical.join("Cargo.toml");
    let manifest = std::fs::read_to_string(&manifest_path)
        .map_err(|error| CacheSkip(format!("cannot read {}: {error}", manifest_path.display())))?;
    budget.consume(manifest.len() as u64)?;
    let mut hasher = Sha256::new();
    hasher.update(b"manifest\0");
    hasher.update(manifest.as_bytes());
    let build_rs = canonical.join("build.rs");
    if build_rs.is_file() {
        let bytes = std::fs::read(&build_rs)
            .map_err(|error| CacheSkip(format!("cannot read {}: {error}", build_rs.display())))?;
        budget.consume(bytes.len() as u64)?;
        hasher.update(b"build.rs\0");
        hasher.update(hash_bytes(&bytes));
    }
    let src = canonical.join("src");
    if src.is_dir() {
        let mut files = Vec::new();
        collect_rs_files(&src, &mut files)
            .map_err(|error| CacheSkip(format!("cannot walk {}: {error}", src.display())))?;
        files.sort();
        for file in files {
            let bytes = std::fs::read(&file)
                .map_err(|error| CacheSkip(format!("cannot read {}: {error}", file.display())))?;
            budget.consume(bytes.len() as u64)?;
            hasher.update(b"src-file\0");
            hasher.update(path_suffix(&file, &canonical).as_bytes());
            hasher.update(b"\0sha256=");
            hasher.update(hash_bytes(&bytes));
        }
    }
    // Recurse into further `path = "..."` manifests (workspace members).
    for dep in path_dep_targets(&manifest) {
        let target = canonical.join(&dep);
        let target_dir = if target.is_file() {
            target.parent().map(|parent| parent.to_path_buf())
        } else {
            Some(target)
        };
        if let Some(target_dir) = target_dir
            && target_dir.join("Cargo.toml").is_file()
        {
            hash_path_dep_tree(&target_dir, budget, visited, depth + 1)?;
        }
    }
    Ok(())
}

/// Minimal `path = "..."` extractor for first-party manifests.
///
/// Double-quoted values only; anything else is ignored (never trusted).
fn path_dep_targets(manifest: &str) -> Vec<String> {
    let mut targets = Vec::new();
    for line in manifest.lines() {
        let Some(eq) = line.find('=') else { continue };
        let (left, right) = line.split_at(eq);
        if !left.trim_end().ends_with("path") {
            continue;
        }
        let value = right[1..].trim_start();
        if let Some(quoted) = value.strip_prefix('"')
            && let Some(end) = quoted.find('"')
        {
            targets.push(quoted[..end].to_string());
        }
    }
    targets
}

/// Collect one grammar file plus every reachable sibling module/include.
fn collect_closure(
    file: &Path,
    root_base: &Path,
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
    let bytes = std::fs::read(&canonical).map_err(|error| {
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
    let mut nested: Vec<Vec<PathBuf>> = Vec::new();
    collect_mods(&parsed.items, &base_dir, &mut nested);
    for group in &mut nested {
        group.sort();
        group.dedup();
    }
    nested.sort();
    nested.dedup();
    for group in nested {
        let target = group
            .iter()
            .find(|candidate| candidate.is_file() || candidate.is_dir())
            .ok_or_else(|| {
                CacheSkip(format!(
                    "grammar module has no resolvable file among {}",
                    group
                        .iter()
                        .map(|path| path.display().to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                ))
            })?;
        collect_closure(target, root_base, closure, seen, depth + 1, budget)?;
    }
    scan_includes(&parsed, &base_dir, root_base, closure, seen, depth, budget)?;
    closure.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(())
}

/// Find `mod ...;` targets (plain or `#[path]`-redirected) among items.
///
/// Each module contributes one candidate group in preference order: a
/// `#[path]` literal resolves to exactly that file (or its `mod.rs`), while
/// a plain `mod name;` tries `name.rs` before `name/mod.rs`. Callers use the
/// first candidate that exists; an empty group is unresolvable.
fn collect_mods(items: &[syn::Item], base_dir: &Path, nested: &mut Vec<Vec<PathBuf>>) {
    for item in items {
        let syn::Item::Mod(module) = item else {
            continue;
        };
        if let Some(content) = &module.content {
            collect_mods(&content.1, base_dir, nested);
            continue;
        }
        if let Some(literal) = path_attr_literal(&module.attrs) {
            let target = base_dir.join(&literal);
            nested.push(vec![if target.is_dir() {
                target.join("mod.rs")
            } else {
                target
            }]);
        } else {
            let name = module.ident.to_string();
            nested.push(vec![
                base_dir.join(format!("{name}.rs")),
                base_dir.join(&name).join("mod.rs"),
            ]);
        }
    }
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

/// Fold literal `include!` targets into the closure; dynamic arguments
/// refuse caching because their inputs cannot be identified.
fn scan_includes(
    file: &syn::File,
    base_dir: &Path,
    root_base: &Path,
    closure: &mut Vec<(String, Vec<u8>)>,
    seen: &mut HashSet<PathBuf>,
    depth: u8,
    budget: &mut u64,
) -> Result<(), CacheSkip> {
    struct Includes {
        literals: Vec<String>,
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
                    Ok(literal) => self.literals.push(literal.value()),
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
    for literal in found.literals {
        // `include!` paths resolve relative to the current file.
        collect_closure(
            &base_dir.join(&literal),
            root_base,
            closure,
            seen,
            depth + 1,
            budget,
        )?;
    }
    Ok(())
}

/// `rustc -vV` release/host identity, computed once per process.
fn rustc_fingerprint() -> Option<String> {
    static FINGERPRINT: OnceLock<Option<String>> = OnceLock::new();
    FINGERPRINT
        .get_or_init(|| {
            let output = std::process::Command::new("rustc")
                .arg("-vV")
                .output()
                .ok()?;
            if !output.status.success() {
                return None;
            }
            let text = String::from_utf8_lossy(&output.stdout);
            let mut selected = String::new();
            for line in text.lines() {
                if line.starts_with("release:") || line.starts_with("host:") {
                    selected.push_str(line.trim());
                    selected.push('\n');
                }
            }
            if selected.is_empty() {
                None
            } else {
                Some(selected)
            }
        })
        .clone()
}

/// A complete, verified cache entry.
struct ValidEntry {
    exe: PathBuf,
}

/// Validate a cache entry before executing anything from it.
///
/// Every check fails closed: wrong name shape, unreadable or corrupt
/// manifest, protocol mismatch, key mismatch, stale TTL, missing or
/// hash-mismatched executable all yield `None` without side effects.
fn validate_entry(root: &Path, key: &str) -> Option<ValidEntry> {
    if key.len() != ENTRY_NAME_LEN || !key.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    let dir = root.join(key);
    let manifest_bytes = std::fs::read(dir.join("manifest.json")).ok()?;
    if manifest_bytes.len() > 1_048_576 {
        return None;
    }
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
    let expected_sha = manifest
        .get("exe")
        .and_then(|exe| exe.get("sha256"))
        .and_then(serde_json::Value::as_str)?;
    let exe = dir.join(exe_name);
    let bytes = std::fs::read(&exe).ok()?;
    if bytes.len() as u64 != expected_len || hex_digest(Sha256::digest(&bytes)) != expected_sha {
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
fn claim_staging_dir(root: &Path) -> PathBuf {
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
            return candidate;
        }
    }
    // Degrade to a process-private temp dir rather than touching anything
    // shared; the build then proceeds without any cache publication.
    std::env::temp_dir().join(format!(
        ".adze-runner-staging-{pid}-{}",
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ))
}

/// Build in owned staging, verify, and publish atomically.
fn build_and_publish(
    spec: &RunnerSpec<'_>,
    config: &CacheConfig,
    key: &str,
    staging: &Path,
) -> anyhow::Result<CachedBuild> {
    let src_dir = staging.join("src");
    std::fs::create_dir_all(&src_dir)?;
    std::fs::write(staging.join("Cargo.toml"), &spec.cargo_toml)?;
    std::fs::write(staging.join("build.rs"), &spec.build_rs)?;
    std::fs::write(src_dir.join("main.rs"), &spec.main_rs)?;

    note_cargo_spawn();
    let output = std::process::Command::new("cargo")
        .arg("build")
        .arg("--manifest-path")
        .arg(staging.join("Cargo.toml"))
        .env("ADZE_USE_PURE_RUST", "1")
        .output()?;
    if !output.status.success() {
        return Err(anyhow::Error::new(CargoFailed {
            status: output.status,
            stdout: output.stdout,
            stderr: output.stderr,
        }));
    }
    let exe_name = format!("adze-cli-parse-runner{}", std::env::consts::EXE_SUFFIX);
    let built = cargo_target_dir(staging).join("debug").join(&exe_name);
    let bytes = std::fs::read(&built).map_err(|error| {
        anyhow::anyhow!(
            "cached runner build produced no executable {}: {error}",
            built.display()
        )
    })?;
    if bytes.is_empty() {
        anyhow::bail!("cached runner build produced an empty executable");
    }
    let exe_sha = hex_digest(Sha256::digest(&bytes));

    // Assemble the complete entry inside staging and commit it with a single
    // rename, so readers only ever see absent-or-complete entries.
    let entry = staging.join("entry");
    std::fs::create_dir_all(&entry)?;
    std::fs::write(entry.join(&exe_name), &bytes)?;
    let built_unix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or(0);
    let manifest = serde_json::json!({
        "protocol": RUNNER_PROTOCOL_VERSION,
        "key": key,
        "cli_version": env!("CARGO_PKG_VERSION"),
        "built_unix": built_unix,
        "exe": {"file": exe_name, "sha256": exe_sha, "len": bytes.len() as u64},
    });
    std::fs::write(
        entry.join("manifest.json"),
        serde_json::to_string_pretty(&manifest)?,
    )?;
    let lock_src = staging.join("Cargo.lock");
    if lock_src.is_file() {
        let _ = std::fs::copy(&lock_src, entry.join("Cargo.lock"));
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
        if name.starts_with(".trash-") {
            trash.push(entry.path());
            continue;
        }
        if name.ends_with(".lock") {
            if lock_is_stale(&entry.path()) {
                let _ = std::fs::remove_file(entry.path());
            }
            continue;
        }
        if name.starts_with(".staging-") || name.len() != ENTRY_NAME_LEN {
            continue;
        }
        let built = std::fs::read(entry.path().join("manifest.json"))
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
        let fresh = std::fs::read(path.join("manifest.json"))
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

/// Move an entry aside before deleting so a concurrent validator never sees
/// a half-removed directory; deletion failures are left for a later pass.
fn retire_entry(root: &Path, path: &Path) {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let pid = std::process::id();
    let trash = root.join(format!(
        ".trash-{pid}-{}",
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    if std::fs::rename(path, &trash).is_ok() {
        let _ = std::fs::remove_dir_all(&trash);
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

/// Where Cargo places build artifacts for a staging project.
///
/// Defaults to the staging-local `target/` directory; an explicit
/// `CARGO_TARGET_DIR` is honored for placement only and never enters the
/// cache key (a relative value resolves against the CLI's working directory,
/// matching Cargo's own behavior).
fn cargo_target_dir(staging: &Path) -> PathBuf {
    if let Ok(dir) = std::env::var("CARGO_TARGET_DIR") {
        let dir = PathBuf::from(dir);
        if dir.is_absolute() {
            return dir;
        }
        return std::env::current_dir()
            .unwrap_or_else(|_| staging.to_path_buf())
            .join(dir);
    }
    staging.join("target")
}
/// Capped consumption budget; exhaustion refuses caching (fail safe).
struct FileBudget {
    files: usize,
    bytes: u64,
}

impl FileBudget {
    fn new(files: usize, bytes: u64) -> Self {
        Self { files, bytes }
    }

    fn consume(&mut self, bytes: u64) -> Result<(), CacheSkip> {
        if self.files == 0 {
            return Err(CacheSkip("dependency walk exceeds file budget".to_string()));
        }
        self.files -= 1;
        self.bytes = self
            .bytes
            .checked_sub(bytes)
            .ok_or_else(|| CacheSkip("dependency walk exceeds byte budget".to_string()))?;
        Ok(())
    }
}

/// Recursively collect `*.rs` files (allocation-light pre-sort).
fn collect_rs_files(dir: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            collect_rs_files(&path, out)?;
        } else if file_type.is_file() && path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
    Ok(())
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

    fn write_grammar(dir: &Path, name: &str, content: &str) -> PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, content).expect("write grammar fixture");
        path
    }

    const SINGLE: &str = "#[adze::grammar(\"tiny\")]\npub mod grammar {\n}\n";

    #[test]
    fn key_is_stable_and_content_addressed() {
        let temp = tempfile::tempdir().expect("tempdir");
        let grammar = write_grammar(temp.path(), "grammar.rs", SINGLE);
        let first = super::build_key(&test_spec(&grammar, "grammar")).expect("key");
        let second = super::build_key(&test_spec(&grammar, "grammar")).expect("key");
        assert_eq!(first, second, "same bytes must produce the same key");
        assert_eq!(first.len(), 64, "key must be a sha256 hex digest");
    }

    #[test]
    fn key_ignores_mtime_and_follows_bytes() {
        let temp = tempfile::tempdir().expect("tempdir");
        let grammar = write_grammar(temp.path(), "grammar.rs", SINGLE);
        let before = super::build_key(&test_spec(&grammar, "grammar")).expect("key");
        // Rewrite identical bytes (mtime changes); then change one byte.
        write_grammar(temp.path(), "grammar.rs", SINGLE);
        let same_bytes = super::build_key(&test_spec(&grammar, "grammar")).expect("key");
        assert_eq!(before, same_bytes, "mtime alone must not change the key");
        write_grammar(temp.path(), "grammar.rs", &format!("{SINGLE}// v2\n"));
        let changed = super::build_key(&test_spec(&grammar, "grammar")).expect("key");
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
        let key_a = super::build_key(&test_spec(&grammar_a, "grammar")).expect("key a");
        let key_b = super::build_key(&test_spec(&grammar_b, "grammar")).expect("key b");
        assert_ne!(
            key_a, key_b,
            "identical root bytes with different sibling resolution must not collide"
        );
        // Identical layout and bytes share the key (same binary either way).
        std::fs::write(root_b.join("shared.rs"), "pub const V: u32 = 1;\n").expect("align sibling");
        let key_b_aligned = super::build_key(&test_spec(&grammar_b, "grammar")).expect("key b2");
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
            super::build_key(&test_spec(&grammar, "grammar")).is_none(),
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
            super::build_key(&test_spec(&grammar, "grammar")).is_none(),
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
        std::fs::create_dir_all(root.join(".staging-999-0")).expect("foreign staging");
        std::fs::write(root.join(".staging-999-0").join("wip"), "wip").expect("wip");
        let live_lock = root.join(format!("{}.lock", "b".repeat(64)));
        std::fs::write(&live_lock, "pid=999").expect("live lock");

        super::enforce_bounds(&root, 2);

        let remaining: Vec<String> = std::fs::read_dir(&root)
            .expect("read cache")
            .flatten()
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .filter(|name| name.len() == 64)
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
        let claimed = super::claim_staging_dir(&root);
        assert_ne!(claimed, taken);
        assert!(claimed.is_dir());
        assert_eq!(
            std::fs::read_to_string(taken.join("sentinel")).expect("sentinel"),
            "foreign"
        );
    }
}
