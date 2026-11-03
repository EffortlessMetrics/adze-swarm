//! Fixed-capacity artifact storage for the disabled runner cache.
//!
//! Permanent coordination files are never renamed or removed. Every mutation
//! acquires namespace, then an independently opened nonblocking slot lock.
//! Filesystem writes occur only after their full artifact allowance is recorded.
//! Drop releases OS handles only; durable owners survive uncertain completion.
//! Compiler scratch, immutable build inputs and platform release authority are
//! separate gates. This module does not enable runtime reuse.

use crate::runner_cache_io;
use crate::runner_invocation::OwnedCleanupReceipt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fs::{File, OpenOptions, TryLockError};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

const VERSION: u32 = 1;
const DIRECTORY: &str = "owned-store-v1";
const NAMESPACE_FILE: &str = "namespace.lock";
const RECORD_FILE: &str = "namespace.json";
const NEXT_FILE: &str = "namespace.next";
const MAX_SLOTS: usize = 32;
const MAX_RUNS: usize = 128;
const RECORD_LIMIT: u64 = 64 << 10;
pub(crate) const TOTAL_ARTIFACT_LIMIT: u64 = 1 << 30;
pub(crate) const ENTRY_ALLOWANCE: u64 = (256 << 20) + (4 << 20) + (1 << 20) + 8192;

#[derive(Debug)]
pub(crate) enum StoreError {
    Busy(&'static str),
    Capacity,
    Invalid(String),
    Io(&'static str, io::Error),
    OwnershipUnknown,
}

impl std::fmt::Display for StoreError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Busy(scope) => write!(formatter, "runner cache {scope} is busy"),
            Self::Capacity => formatter.write_str("runner cache owned capacity is exhausted"),
            Self::Invalid(reason) => write!(formatter, "runner cache refused: {reason}"),
            Self::Io(operation, error) => write!(formatter, "runner cache {operation}: {error}"),
            Self::OwnershipUnknown => formatter.write_str(
                "runner cache execution ownership is unproved; reservation retained; no retry",
            ),
        }
    }
}

impl std::error::Error for StoreError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(_, error) => Some(error),
            _ => None,
        }
    }
}

type Result<T> = std::result::Result<T, StoreError>;

fn invalid(reason: impl Into<String>) -> StoreError {
    StoreError::Invalid(reason.into())
}

fn disk<T>(operation: &'static str, result: io::Result<T>) -> Result<T> {
    result.map_err(|error| StoreError::Io(operation, error))
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileIdentity {
    device: u64,
    file: u64,
}

fn file_identity(file: &File) -> Result<FileIdentity> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let metadata = disk("identify file", file.metadata())?;
        Ok(FileIdentity {
            device: metadata.dev(),
            file: metadata.ino(),
        })
    }
    #[cfg(windows)]
    {
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::Storage::FileSystem::{
            BY_HANDLE_FILE_INFORMATION, GetFileInformationByHandle,
        };
        // SAFETY: zero-valid Win32 output record and a live owned file handle.
        let mut information: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
        if unsafe { GetFileInformationByHandle(file.as_raw_handle().cast(), &mut information) } == 0
        {
            return Err(StoreError::Io("identify file", io::Error::last_os_error()));
        }
        Ok(FileIdentity {
            device: u64::from(information.dwVolumeSerialNumber),
            file: (u64::from(information.nFileIndexHigh) << 32)
                | u64::from(information.nFileIndexLow),
        })
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = file;
        Err(invalid(
            "store file identity is unsupported on this platform",
        ))
    }
}

fn regular(metadata: &std::fs::Metadata) -> bool {
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return false;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        use windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT;
        if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return false;
        }
    }
    true
}

fn plain_directory(metadata: &std::fs::Metadata) -> bool {
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return false;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        use windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT;
        if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return false;
        }
    }
    true
}

fn open_coordination(path: &Path, create: bool) -> Result<File> {
    if !create
        && !regular(&disk(
            "inspect coordination",
            std::fs::symlink_metadata(path),
        )?)
    {
        return Err(invalid("coordination path is not a regular file"));
    }
    let mut options = OpenOptions::new();
    options.read(true).write(true).create_new(create);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        use windows_sys::Win32::Storage::FileSystem::FILE_FLAG_OPEN_REPARSE_POINT;
        options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    }
    let file = disk("open coordination", options.open(path))?;
    let metadata = disk("inspect open coordination", file.metadata())?;
    if !regular(&metadata) || metadata.len() != 0 {
        return Err(invalid(
            "coordination file must remain an empty regular file",
        ));
    }
    Ok(file)
}

fn lock(file: File, shared: bool, scope: &'static str) -> Result<File> {
    let result = if shared {
        file.try_lock_shared()
    } else {
        file.try_lock()
    };
    match result {
        Ok(()) => Ok(file),
        Err(TryLockError::WouldBlock) => Err(StoreError::Busy(scope)),
        Err(TryLockError::Error(error)) => Err(StoreError::Io("lock coordination", error)),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Limits {
    pub(crate) slots: usize,
    pub(crate) artifact_bytes: u64,
}

impl Limits {
    fn validate(self) -> Result<()> {
        if self.slots == 0
            || self.slots > MAX_SLOTS
            || self.artifact_bytes == 0
            || self.artifact_bytes > TOTAL_ARTIFACT_LIMIT
        {
            return Err(invalid(
                "store limits are outside the declared finite bounds",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Artifact {
    Executable,
    Manifest,
    Lockfile,
    BuildLog,
}

impl Artifact {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Executable if cfg!(windows) => "runner.exe",
            Self::Executable => "runner",
            Self::Manifest => "manifest.json",
            Self::Lockfile => "Cargo.lock",
            Self::BuildLog => "build.log",
        }
    }

    pub(crate) fn limit(self) -> u64 {
        match self {
            Self::Executable => 256 << 20,
            Self::Manifest => 1 << 20,
            Self::Lockfile => 4 << 20,
            Self::BuildLog => 8192,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Token {
    store: FileIdentity,
    slot: usize,
    generation: u64,
    id: u64,
}

impl Token {
    fn directory(self) -> String {
        format!(
            "entry-{:016x}-{:016x}-{}-g{}-o{}",
            self.store.device, self.store.file, self.slot, self.generation, self.id,
        )
    }

    fn binding(self) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(b"adze-cache-run-owner-v1\0");
        for value in [
            self.store.device,
            self.store.file,
            self.slot as u64,
            self.generation,
            self.id,
        ] {
            hasher.update(value.to_le_bytes());
        }
        hasher.finalize().into()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ArtifactRecord {
    kind: Artifact,
    len: u64,
    sha256: [u8; 32],
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RunRecord {
    token: Token,
    started: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "phase", rename_all = "snake_case", deny_unknown_fields)]
enum Entry {
    Reserved {
        token: Token,
        key: String,
        allowance: u64,
    },
    Published {
        token: Token,
        key: String,
        files: Vec<ArtifactRecord>,
        runs: Vec<RunRecord>,
    },
    Retiring {
        token: Token,
        key: String,
        files: Vec<ArtifactRecord>,
        retirer: Token,
    },
}

impl Entry {
    fn token(&self) -> Token {
        match self {
            Self::Reserved { token, .. }
            | Self::Published { token, .. }
            | Self::Retiring { token, .. } => *token,
        }
    }

    fn key(&self) -> &str {
        match self {
            Self::Reserved { key, .. }
            | Self::Published { key, .. }
            | Self::Retiring { key, .. } => key,
        }
    }

    fn bytes(&self) -> Result<u64> {
        match self {
            Self::Reserved { allowance, .. } => Ok(*allowance),
            Self::Published { files, .. } | Self::Retiring { files, .. } => file_total(files),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Slot {
    lock_identity: FileIdentity,
    generation: u64,
    entry: Option<Entry>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Namespace {
    version: u32,
    identity: FileIdentity,
    limits: Limits,
    next_token: u64,
    slots: Vec<Slot>,
}

fn valid_key(key: &str) -> bool {
    key.len() == 64
        && key
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn file_total(files: &[ArtifactRecord]) -> Result<u64> {
    if files.len() > 4 {
        return Err(invalid("too many entry artifacts"));
    }
    let mut kinds = HashSet::new();
    let mut total = 0_u64;
    for file in files {
        if !kinds.insert(file.kind) || file.len > file.kind.limit() {
            return Err(invalid("duplicate or oversized entry artifact"));
        }
        total = total.checked_add(file.len).ok_or(StoreError::Capacity)?;
    }
    if !files
        .iter()
        .any(|file| file.kind == Artifact::Executable && file.len > 0)
        || !files
            .iter()
            .any(|file| file.kind == Artifact::Manifest && file.len > 0)
    {
        return Err(invalid(
            "complete entry needs a nonempty executable and manifest",
        ));
    }
    Ok(total)
}

impl Namespace {
    fn validate(&self, identity: FileIdentity, limits: Limits) -> Result<()> {
        self.limits.validate()?;
        if self.version != VERSION
            || self.identity != identity
            || self.limits != limits
            || self.slots.len() != limits.slots
            || self.next_token == 0
        {
            return Err(invalid(
                "unsupported or inconsistent namespace identity/policy",
            ));
        }
        let mut ids = HashSet::new();
        let mut lock_ids = HashSet::from([identity]);
        let mut keys = HashSet::new();
        let mut runs = 0_usize;
        let mut bytes = 0_u64;
        let check_token = |token: Token, index: usize, generation: u64, ids: &mut HashSet<u64>| {
            if token.store != identity
                || token.slot != index
                || token.generation != generation
                || generation == 0
                || token.id == 0
                || token.id >= self.next_token
                || !ids.insert(token.id)
            {
                return Err(invalid("stale, duplicate or foreign namespace token"));
            }
            Ok(())
        };
        for (index, slot) in self.slots.iter().enumerate() {
            if !lock_ids.insert(slot.lock_identity) {
                return Err(invalid("duplicate namespace/slot coordination identity"));
            }
            if let Some(entry) = &slot.entry {
                check_token(entry.token(), index, slot.generation, &mut ids)?;
                if !valid_key(entry.key()) || !keys.insert(entry.key()) {
                    return Err(invalid("invalid or duplicate entry key"));
                }
                match entry {
                    Entry::Reserved { allowance, .. }
                        if *allowance == 0 || *allowance > ENTRY_ALLOWANCE =>
                    {
                        return Err(invalid("invalid artifact reservation"));
                    }
                    Entry::Published { runs: active, .. } => {
                        runs = runs.checked_add(active.len()).ok_or(StoreError::Capacity)?;
                        if runs > MAX_RUNS {
                            return Err(StoreError::Capacity);
                        }
                        for run in active {
                            check_token(run.token, index, slot.generation, &mut ids)?;
                        }
                    }
                    Entry::Retiring { retirer, .. } => {
                        check_token(*retirer, index, slot.generation, &mut ids)?
                    }
                    _ => {}
                }
                bytes = bytes
                    .checked_add(entry.bytes()?)
                    .ok_or(StoreError::Capacity)?;
            }
        }
        if bytes > limits.artifact_bytes {
            return Err(StoreError::Capacity);
        }
        Ok(())
    }

    fn token(&mut self, slot: usize, generation: u64) -> Result<Token> {
        let id = self.next_token;
        self.next_token = id.checked_add(1).ok_or(StoreError::Capacity)?;
        Ok(Token {
            store: self.identity,
            slot,
            generation,
            id,
        })
    }

    fn bytes(&self) -> Result<u64> {
        self.slots.iter().try_fold(0_u64, |sum, slot| {
            sum.checked_add(
                slot.entry
                    .as_ref()
                    .map(Entry::bytes)
                    .transpose()?
                    .unwrap_or(0),
            )
            .ok_or(StoreError::Capacity)
        })
    }
}

struct StoreInner {
    root: PathBuf,
    identity: FileIdentity,
    limits: Limits,
    // This identity anchor is never locked or cloned. Keeping the original
    // object open prevents a replacement store from reusing its file identity.
    _anchor: File,
}

#[derive(Clone)]
pub(crate) struct Store(Arc<StoreInner>);

struct NamespaceGuard {
    store: Store,
    _lock: File,
    state: Namespace,
}

fn create_private_directory(path: &Path) -> io::Result<()> {
    let mut builder = std::fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(path)
}

fn sync_directory(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        let directory = disk("open directory for sync", File::open(path))?;
        disk("sync directory", directory.sync_all())
    }
    #[cfg(not(unix))]
    {
        // Windows controls must qualify atomic visibility and process-death
        // persistence. This claims no directory-flush/power-loss guarantee.
        let _ = path;
        Ok(())
    }
}

fn write_new(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = disk("create owned file", options.open(path))?;
    disk("write owned file", file.write_all(bytes))?;
    disk("sync owned file", file.sync_all())
}

impl Store {
    pub(crate) fn open(parent: &Path, limits: Limits) -> Result<Self> {
        limits.validate()?;
        let root = parent.join(DIRECTORY);
        let created = match create_private_directory(&root) {
            Ok(()) => true,
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => false,
            Err(error) => return Err(StoreError::Io("create store", error)),
        };
        let metadata = disk("inspect store root", std::fs::symlink_metadata(&root))?;
        if !plain_directory(&metadata) {
            return Err(invalid("store root is not an owned directory"));
        }
        let anchor = open_coordination(&root.join(NAMESPACE_FILE), created)?;
        let identity = file_identity(&anchor)?;
        let store = Self(Arc::new(StoreInner {
            root,
            identity,
            limits,
            _anchor: anchor,
        }));
        if created {
            let namespace_lock = store.namespace_lock()?;
            let mut slots = Vec::with_capacity(limits.slots);
            for index in 0..limits.slots {
                let file =
                    open_coordination(&store.0.root.join(format!("slot-{index}.lock")), true)?;
                slots.push(Slot {
                    lock_identity: file_identity(&file)?,
                    generation: 0,
                    entry: None,
                });
                disk("sync slot coordination", file.sync_all())?;
            }
            let state = Namespace {
                version: VERSION,
                identity,
                limits,
                next_token: 1,
                slots,
            };
            let mut guard = NamespaceGuard {
                store: store.clone(),
                _lock: namespace_lock,
                state,
            };
            guard.persist(true)?;
        } else {
            let _guard = store.namespace()?;
        }
        Ok(store)
    }

    fn namespace_lock(&self) -> Result<File> {
        let file = self.current_coordination()?;
        lock(file, false, "namespace")
    }

    // Identity inspection does not acquire a lock or reverse lock order. The
    // store is private; administrative replacement requires quiescence. A
    // surviving old handle additionally refuses all operations on a new store.
    fn current_coordination(&self) -> Result<File> {
        let file = open_coordination(&self.0.root.join(NAMESPACE_FILE), false)?;
        if file_identity(&file)? != self.0.identity {
            return Err(invalid("store coordination identity was replaced"));
        }
        Ok(file)
    }

    fn namespace(&self) -> Result<NamespaceGuard> {
        let file = self.namespace_lock()?;
        if self
            .0
            .root
            .join(NEXT_FILE)
            .try_exists()
            .map_err(|e| StoreError::Io("inspect next record", e))?
        {
            return Err(invalid(
                "unfinished namespace replacement requires administrative recovery",
            ));
        }
        let bytes = disk(
            "read namespace",
            runner_cache_io::read_regular(&self.0.root.join(RECORD_FILE), RECORD_LIMIT),
        )?;
        let state: Namespace = serde_json::from_slice(&bytes)
            .map_err(|error| invalid(format!("namespace record: {error}")))?;
        state.validate(self.0.identity, self.0.limits)?;
        let guard = NamespaceGuard {
            store: self.clone(),
            _lock: file,
            state,
        };
        guard.validate_names()?;
        Ok(guard)
    }

    fn entry_path(&self, token: Token) -> PathBuf {
        self.0.root.join(token.directory())
    }

    pub(crate) fn reserve(&self, key: &str, allowance: u64) -> Result<ArtifactReservation> {
        if !valid_key(key)
            || allowance == 0
            || allowance > ENTRY_ALLOWANCE
            || allowance > self.0.limits.artifact_bytes
        {
            return Err(invalid(
                "key or requested artifact allowance is outside policy",
            ));
        }
        let mut namespace = self.namespace()?;
        if namespace
            .state
            .slots
            .iter()
            .any(|slot| slot.entry.as_ref().is_some_and(|entry| entry.key() == key))
        {
            return Err(StoreError::Busy("entry key"));
        }
        // At most one retirement pass per fixed slot, with no lock wait.
        for _ in 0..=self.0.limits.slots {
            if namespace
                .state
                .bytes()?
                .checked_add(allowance)
                .is_some_and(|bytes| bytes <= self.0.limits.artifact_bytes)
            {
                for index in 0..namespace.state.slots.len() {
                    if namespace.state.slots[index].entry.is_some() {
                        continue;
                    }
                    let slot_lock = match namespace.slot_lock(index, false) {
                        Err(StoreError::Busy(_)) => continue,
                        other => other?,
                    };
                    let generation = namespace.state.slots[index]
                        .generation
                        .checked_add(1)
                        .ok_or(StoreError::Capacity)?;
                    let token = namespace.state.token(index, generation)?;
                    namespace.state.slots[index].generation = generation;
                    namespace.state.slots[index].entry = Some(Entry::Reserved {
                        token,
                        key: key.into(),
                        allowance,
                    });
                    namespace.persist(false)?;
                    let directory = self.entry_path(token);
                    // The complete allowance is durable before any directory
                    // or artifact write. Collision never grants cleanup rights.
                    disk(
                        "claim artifact staging",
                        create_private_directory(&directory),
                    )?;
                    sync_directory(&self.0.root)?;
                    drop(namespace);
                    return Ok(ArtifactReservation {
                        store: self.clone(),
                        token,
                        lock: slot_lock,
                        allowance,
                        files: Vec::new(),
                        claimed: Vec::new(),
                        failed: false,
                    });
                }
            }
            if !namespace.retire_one()? {
                return Err(StoreError::Capacity);
            }
        }
        Err(StoreError::Capacity)
    }

    pub(crate) fn acquire(&self, key: &str) -> Result<Option<UnstartedLease>> {
        if !valid_key(key) {
            return Err(invalid("invalid entry key"));
        }
        let mut namespace = self.namespace()?;
        let total_runs = namespace
            .state
            .slots
            .iter()
            .map(|slot| match &slot.entry {
                Some(Entry::Published { runs, .. }) => runs.len(),
                _ => 0,
            })
            .sum::<usize>();
        if total_runs >= MAX_RUNS {
            return Err(StoreError::Capacity);
        }
        let Some(index) = namespace.state.slots.iter().position(|slot| {
            matches!(&slot.entry, Some(Entry::Published { key: existing, .. }) if existing == key)
        }) else {
            return Ok(None);
        };
        let slot_lock = namespace.slot_lock(index, true)?;
        let generation = namespace.state.slots[index].generation;
        let token = namespace.state.token(index, generation)?;
        let Some(Entry::Published {
            token: entry_token,
            files,
            runs,
            ..
        }) = &mut namespace.state.slots[index].entry
        else {
            return Err(invalid("entry publication changed under namespace"));
        };
        let entry = *entry_token;
        let files = files.clone();
        runs.push(RunRecord {
            token,
            started: false,
        });
        namespace.persist(false)?;
        drop(namespace);
        let lease = UnstartedLease {
            store: self.clone(),
            token,
            entry,
            files,
            lock: slot_lock,
        };
        if let Err(error) = lease.verify() {
            let release = lease.cancel();
            return Err(match release {
                Ok(()) => error,
                Err(release) => invalid(format!(
                    "entry validation failed ({error}); reservation retained ({release})"
                )),
            });
        }
        Ok(Some(lease))
    }

    #[cfg(test)]
    fn inspect(&self, key: &str) -> Result<Option<Vec<u8>>> {
        let Some(lease) = self.acquire(key)? else {
            return Ok(None);
        };
        let manifest = lease.manifest();
        // This lease has never exposed an executable or launched a child.
        lease.cancel()?;
        manifest.map(Some)
    }
}

impl NamespaceGuard {
    fn slot_lock(&self, index: usize, shared: bool) -> Result<File> {
        let slot = self
            .state
            .slots
            .get(index)
            .ok_or_else(|| invalid("invalid slot"))?;
        let file = open_coordination(&self.store.0.root.join(format!("slot-{index}.lock")), false)?;
        if file_identity(&file)? != slot.lock_identity {
            return Err(invalid("slot coordination identity was replaced"));
        }
        lock(file, shared, "slot")
    }

    fn validate_names(&self) -> Result<()> {
        let mut allowed = HashSet::from([NAMESPACE_FILE.to_string(), RECORD_FILE.to_string()]);
        for (index, slot) in self.state.slots.iter().enumerate() {
            allowed.insert(format!("slot-{index}.lock"));
            if let Some(entry) = &slot.entry {
                allowed.insert(entry.token().directory());
            }
        }
        let directory = disk("list namespace", std::fs::read_dir(&self.store.0.root))?;
        for (count, entry) in directory.enumerate() {
            if count >= 2 + MAX_SLOTS * 2 {
                return Err(StoreError::Capacity);
            }
            let entry = disk("read namespace name", entry)?;
            let name = entry
                .file_name()
                .into_string()
                .map_err(|_| invalid("unknown namespace name"))?;
            if !allowed.remove(&name) {
                return Err(invalid(
                    "unrelated or duplicate namespace entry is preserved",
                ));
            }
        }
        // A Reserved generation may not yet have created its directory.
        for name in allowed {
            if !self.state.slots.iter().any(|slot| {
                matches!(&slot.entry,
                Some(Entry::Reserved { token, .. }) if token.directory() == name)
            }) {
                return Err(invalid("required namespace object is missing"));
            }
        }
        Ok(())
    }

    fn persist(&mut self, initial: bool) -> Result<()> {
        self.state
            .validate(self.store.0.identity, self.store.0.limits)?;
        let bytes = serde_json::to_vec(&self.state).map_err(|error| invalid(error.to_string()))?;
        if bytes.len() as u64 > RECORD_LIMIT {
            return Err(StoreError::Capacity);
        }
        let next = self.store.0.root.join(NEXT_FILE);
        write_new(&next, &bytes)?;
        let record = self.store.0.root.join(RECORD_FILE);
        if initial
            && record
                .try_exists()
                .map_err(|e| StoreError::Io("inspect initial record", e))?
        {
            return Err(invalid("initial namespace record already exists"));
        }
        disk("publish namespace", std::fs::rename(next, record))?;
        sync_directory(&self.store.0.root)
    }

    fn matching_reserved(&self, token: Token) -> Result<u64> {
        if token.store != self.store.0.identity {
            return Err(invalid("foreign store token"));
        }
        match self
            .state
            .slots
            .get(token.slot)
            .and_then(|slot| slot.entry.as_ref())
        {
            Some(Entry::Reserved {
                token: current,
                allowance,
                ..
            }) if *current == token => Ok(*allowance),
            _ => Err(invalid("stale artifact owner/generation")),
        }
    }

    fn retire_one(&mut self) -> Result<bool> {
        let mut eligible: Vec<_> = self
            .state
            .slots
            .iter()
            .enumerate()
            .filter_map(|(index, slot)| match &slot.entry {
                Some(Entry::Published { token, runs, .. }) if runs.is_empty() => {
                    Some((token.id, index))
                }
                _ => None,
            })
            .collect();
        eligible.sort_unstable();
        for (_, index) in eligible {
            let slot_lock = match self.slot_lock(index, false) {
                Err(StoreError::Busy(_)) => continue,
                other => other?,
            };
            let Some(Entry::Published {
                token,
                key,
                files,
                runs,
            }) = self.state.slots[index].entry.clone()
            else {
                return Err(invalid("retirement state changed under namespace"));
            };
            if !runs.is_empty() {
                return Err(invalid("retirement encountered durable execution owners"));
            }
            verify_names(&self.store.entry_path(token), &files)?;
            let retirer = self.state.token(index, token.generation)?;
            self.state.slots[index].entry = Some(Entry::Retiring {
                token,
                key,
                files: files.clone(),
                retirer,
            });
            self.persist(false)?;
            remove_known_files(&self.store.entry_path(token), &files)?;
            self.state.slots[index].entry = None;
            self.persist(false)?;
            drop(slot_lock);
            return Ok(true);
        }
        Ok(false)
    }

    fn run_mut(&mut self, token: Token, started: bool) -> Result<&mut Vec<RunRecord>> {
        if token.store != self.store.0.identity {
            return Err(invalid("foreign execution store token"));
        }
        let Some(Entry::Published { runs, .. }) = self
            .state
            .slots
            .get_mut(token.slot)
            .and_then(|slot| slot.entry.as_mut())
        else {
            return Err(invalid("execution entry is no longer published"));
        };
        if !runs
            .iter()
            .any(|run| run.token == token && run.started == started)
        {
            return Err(invalid("stale execution owner/generation/phase"));
        }
        Ok(runs)
    }
}

fn verify_names(directory: &Path, files: &[ArtifactRecord]) -> Result<()> {
    let metadata = disk(
        "inspect artifact directory",
        std::fs::symlink_metadata(directory),
    )?;
    if !plain_directory(&metadata) {
        return Err(invalid(
            "artifact directory is not an owned regular directory",
        ));
    }
    let mut expected: HashSet<_> = files.iter().map(|file| file.kind.name()).collect();
    let entries = disk("list artifact directory", std::fs::read_dir(directory))?;
    for (count, entry) in entries.enumerate() {
        if count >= 4 {
            return Err(StoreError::Capacity);
        }
        let entry = disk("read artifact name", entry)?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| invalid("unknown artifact filename"))?;
        if !expected.remove(name.as_str()) {
            return Err(invalid("unrelated artifact is preserved"));
        }
        if !regular(&disk(
            "inspect artifact",
            std::fs::symlink_metadata(entry.path()),
        )?) {
            return Err(invalid("nonregular artifact is preserved"));
        }
    }
    if !expected.is_empty() {
        return Err(invalid("published artifact is missing"));
    }
    Ok(())
}

fn remove_known_files(directory: &Path, files: &[ArtifactRecord]) -> Result<()> {
    verify_names(directory, files)?;
    for file in files {
        disk(
            "retire owned artifact",
            std::fs::remove_file(directory.join(file.kind.name())),
        )?;
    }
    disk("retire owned generation", std::fs::remove_dir(directory))?;
    sync_directory(
        directory
            .parent()
            .ok_or_else(|| invalid("missing store parent"))?,
    )
}

fn verify_artifacts(directory: &Path, files: &[ArtifactRecord]) -> Result<()> {
    verify_names(directory, files)?;
    for file in files {
        let digest = disk(
            "validate owned artifact",
            runner_cache_io::digest_regular(&directory.join(file.kind.name()), file.len),
        )?;
        if digest.len != file.len || digest.sha256 != file.sha256 {
            return Err(invalid("owned artifact identity differs"));
        }
    }
    Ok(())
}

pub(crate) struct ArtifactReservation {
    store: Store,
    token: Token,
    lock: File,
    allowance: u64,
    files: Vec<ArtifactRecord>,
    claimed: Vec<(Artifact, FileIdentity)>,
    failed: bool,
}

impl ArtifactReservation {
    fn claim(&mut self, kind: Artifact) -> Result<File> {
        let _identity = self.store.current_coordination()?;
        let path = self.store.entry_path(self.token).join(kind.name());
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let file = disk("claim artifact file", options.open(path))?;
        self.claimed.push((kind, file_identity(&file)?));
        Ok(file)
    }

    fn remaining(&self, kind: Artifact) -> Result<u64> {
        if self.failed || self.files.iter().any(|file| file.kind == kind) {
            return Err(invalid(
                "artifact writer already failed or file already exists",
            ));
        }
        let used = self.files.iter().try_fold(0_u64, |sum, file| {
            sum.checked_add(file.len).ok_or(StoreError::Capacity)
        })?;
        Ok(self
            .allowance
            .checked_sub(used)
            .ok_or(StoreError::Capacity)?
            .min(kind.limit()))
    }

    pub(crate) fn write(&mut self, kind: Artifact, bytes: &[u8]) -> Result<()> {
        let remaining = self.remaining(kind)?;
        if bytes.len() as u64 > remaining {
            return Err(StoreError::Capacity);
        }
        self.failed = true;
        let mut file = self.claim(kind)?;
        disk("write owned artifact", file.write_all(bytes))?;
        disk("sync owned artifact", file.sync_all())?;
        self.files.push(ArtifactRecord {
            kind,
            len: bytes.len() as u64,
            sha256: Sha256::digest(bytes).into(),
        });
        self.failed = false;
        Ok(())
    }

    pub(crate) fn copy(
        &mut self,
        kind: Artifact,
        source: &Path,
    ) -> Result<runner_cache_io::FileDigest> {
        let remaining = self.remaining(kind)?;
        self.failed = true;
        let path = self.store.entry_path(self.token).join(kind.name());
        let mut file = self.claim(kind)?;
        let digest = disk(
            "copy owned artifact",
            runner_cache_io::copy_to_owned_file(source, &mut file, remaining),
        )?;
        if disk(
            "verify owned copy",
            runner_cache_io::digest_regular(&path, remaining),
        )? != digest
        {
            return Err(invalid("copied artifact readback differs"));
        }
        self.files.push(ArtifactRecord {
            kind,
            len: digest.len,
            sha256: digest.sha256,
        });
        self.failed = false;
        Ok(digest)
    }

    pub(crate) fn publish(self) -> Result<()> {
        let _identity = self.store.current_coordination()?;
        if self.failed || file_total(&self.files)? > self.allowance {
            return Err(invalid("incomplete or oversized artifact assembly"));
        }
        for (kind, identity) in &self.claimed {
            let file = disk(
                "open claimed publication artifact",
                runner_cache_io::open_regular(
                    &self.store.entry_path(self.token).join(kind.name()),
                    kind.limit(),
                ),
            )?;
            if file_identity(&file)? != *identity {
                return Err(invalid("claimed publication artifact was replaced"));
            }
        }
        verify_artifacts(&self.store.entry_path(self.token), &self.files)?;
        sync_directory(&self.store.entry_path(self.token))?;
        let Self {
            store,
            token,
            lock,
            files,
            ..
        } = self;
        // Durable reservation protects this gap. Never acquire namespace while
        // still holding the previous slot handle.
        drop(lock);
        let mut namespace = store.namespace()?;
        let _slot = namespace.slot_lock(token.slot, false)?;
        namespace.matching_reserved(token)?;
        let Some(Entry::Reserved { key, .. }) = namespace.state.slots[token.slot].entry.take()
        else {
            return Err(invalid("artifact reservation disappeared"));
        };
        namespace.state.slots[token.slot].entry = Some(Entry::Published {
            token,
            key,
            files,
            runs: Vec::new(),
        });
        namespace.persist(false)
    }

    pub(crate) fn abort(self) -> Result<()> {
        let Self {
            store,
            token,
            lock,
            claimed,
            ..
        } = self;
        drop(lock);
        let mut namespace = store.namespace()?;
        let _slot = namespace.slot_lock(token.slot, false)?;
        namespace.matching_reserved(token)?;
        let directory = store.entry_path(token);
        let names: Vec<_> = claimed
            .iter()
            .map(|(kind, _)| ArtifactRecord {
                kind: *kind,
                len: 0,
                sha256: [0; 32],
            })
            .collect();
        verify_names(&directory, &names)?;
        // ArtifactReservation never exposes this directory or launches a child.
        // Only this live, exact owner may discard its known file-IO assembly.
        for (kind, expected_identity) in claimed {
            let path = directory.join(kind.name());
            let file = disk(
                "open aborted artifact",
                runner_cache_io::open_regular(&path, kind.limit()),
            )?;
            if file_identity(&file)? != expected_identity {
                return Err(invalid(
                    "aborted artifact was replaced; reservation retained",
                ));
            }
            drop(file);
            disk("remove aborted artifact", std::fs::remove_file(path))?;
        }
        disk("remove aborted generation", std::fs::remove_dir(directory))?;
        sync_directory(&store.0.root)?;
        namespace.state.slots[token.slot].entry = None;
        namespace.persist(false)
    }
}

pub(crate) struct UnstartedLease {
    store: Store,
    token: Token,
    entry: Token,
    files: Vec<ArtifactRecord>,
    lock: File,
}

impl UnstartedLease {
    fn verify(&self) -> Result<()> {
        let directory = self.store.entry_path(self.entry);
        verify_artifacts(&directory, &self.files)
    }

    pub(crate) fn executable_identity(&self) -> Result<runner_cache_io::FileDigest> {
        let file = self
            .files
            .iter()
            .find(|file| file.kind == Artifact::Executable)
            .ok_or_else(|| invalid("leased executable record is missing"))?;
        Ok(runner_cache_io::FileDigest {
            len: file.len,
            sha256: file.sha256,
        })
    }

    pub(crate) fn manifest(&self) -> Result<Vec<u8>> {
        let _identity = self.store.current_coordination()?;
        disk(
            "read leased manifest",
            runner_cache_io::read_regular(
                &self
                    .store
                    .entry_path(self.entry)
                    .join(Artifact::Manifest.name()),
                Artifact::Manifest.limit(),
            ),
        )
    }

    pub(crate) fn cancel(self) -> Result<()> {
        let Self {
            store, token, lock, ..
        } = self;
        drop(lock);
        let mut namespace = store.namespace()?;
        let _slot = namespace.slot_lock(token.slot, true)?;
        namespace
            .run_mut(token, false)?
            .retain(|run| run.token != token);
        namespace.persist(false)
    }

    pub(crate) fn start(self) -> Result<ActiveLease> {
        let Self {
            store,
            token,
            entry,
            lock,
            ..
        } = self;
        drop(lock);
        let mut namespace = store.namespace()?;
        let slot_lock = namespace.slot_lock(token.slot, true)?;
        let runs = namespace.run_mut(token, false)?;
        let run = runs
            .iter_mut()
            .find(|run| run.token == token)
            .ok_or_else(|| invalid("execution token disappeared"))?;
        run.started = true;
        namespace.persist(false)?;
        drop(namespace);
        Ok(ActiveLease {
            store,
            token,
            entry,
            lock: slot_lock,
        })
    }
}

pub(crate) struct ActiveLease {
    store: Store,
    token: Token,
    entry: Token,
    lock: File,
}

impl ActiveLease {
    pub(crate) fn binding(&self) -> [u8; 32] {
        self.token.binding()
    }

    pub(crate) fn executable(&self) -> Result<PathBuf> {
        let _identity = self.store.current_coordination()?;
        Ok(self
            .store
            .entry_path(self.entry)
            .join(Artifact::Executable.name()))
    }

    /// Only platform custody may construct a matching positive receipt. None
    /// means unknown, including today's generic Output API. No destructor,
    /// output status, PID/age, or test-only positive constructor substitutes.
    pub(crate) fn finish(self, receipt: Option<OwnedCleanupReceipt>) -> Result<()> {
        let Some(receipt) = receipt else {
            return Err(StoreError::OwnershipUnknown);
        };
        if !receipt.is_for(&self.binding()) {
            return Err(invalid(
                "cleanup receipt belongs to another execution owner",
            ));
        }
        let Self {
            store, token, lock, ..
        } = self;
        drop(lock);
        let mut namespace = store.namespace()?;
        let _slot = namespace.slot_lock(token.slot, true)?;
        namespace
            .run_mut(token, true)?
            .retain(|run| run.token != token);
        namespace.persist(false)
    }
}

#[cfg(test)]
#[path = "runner_cache_store_tests.rs"]
mod tests;
