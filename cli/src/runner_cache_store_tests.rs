//! Real filesystem and bounded child-process controls for artifact ownership.
//! These do not construct platform cleanup authority, run a cached parser, or
//! qualify compiler scratch, immutable inputs, Windows ACLs or network storage.

use super::*;
use std::io::Write;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

fn key(value: u64) -> String {
    format!("{value:064x}")
}

fn open_test_store(root: &Path, slots: usize, artifact_bytes: u64) -> Store {
    Store::open(root, Limits { slots, artifact_bytes }).unwrap()
}

fn publish(store: &Store, value: u64) -> Token {
    let mut writer = store.reserve(&key(value), 64).unwrap();
    let token = writer.token;
    writer.write(Artifact::Executable, b"abc").unwrap();
    writer.write(Artifact::Manifest, b"literal manifest").unwrap();
    writer.publish().unwrap();
    token
}

fn record(store: &Store) -> Vec<u8> {
    std::fs::read(store.0.root.join(RECORD_FILE)).unwrap()
}

#[test]
fn test_fixed_limits_and_reserved_bytes_precede_artifact_writes() {
    let root = tempfile::tempdir().unwrap();
    for limits in [
        Limits { slots: 0, artifact_bytes: 1 },
        Limits { slots: 33, artifact_bytes: 1 },
        Limits { slots: 1, artifact_bytes: 0 },
        Limits { slots: 1, artifact_bytes: TOTAL_ARTIFACT_LIMIT + 1 },
    ] {
        assert!(Store::open(root.path(), limits).is_err());
        assert!(!root.path().join(DIRECTORY).exists());
    }
    let store = open_test_store(root.path(), 2, 10);
    let mut writer = store.reserve(&key(1), 6).unwrap();
    let token = writer.token;
    let directory = store.entry_path(token);
    let state: Namespace = serde_json::from_slice(&record(&store)).unwrap();
    assert!(matches!(&state.slots[0].entry, Some(Entry::Reserved { allowance: 6, .. })));
    assert_eq!(state.bytes().unwrap(), 6);
    assert_eq!(std::fs::read_dir(&directory).unwrap().count(), 0);
    writer.write(Artifact::Executable, b"abcd").unwrap();
    assert!(matches!(writer.write(Artifact::Manifest, b"123"), Err(StoreError::Capacity)));
    assert!(!directory.join(Artifact::Manifest.name()).exists());
    writer.write(Artifact::Manifest, b"12").unwrap();
    writer.publish().unwrap();
    let leased = store.acquire(&key(1)).unwrap().unwrap();
    assert!(matches!(store.reserve(&key(2), 5), Err(StoreError::Capacity)));
    let other = store.reserve(&key(2), 4).unwrap();
    assert_eq!(store.namespace().unwrap().state.bytes().unwrap(), 10);
    other.abort().unwrap();
    leased.cancel().unwrap();

    let separate = tempfile::tempdir().unwrap();
    let bounded = open_test_store(separate.path(), 1, 20_000);
    let mut writer = bounded.reserve(&key(1), 20_000).unwrap();
    let log = bounded.entry_path(writer.token).join(Artifact::BuildLog.name());
    assert!(matches!(writer.write(Artifact::BuildLog, &[0; 8193]), Err(StoreError::Capacity)));
    assert!(!log.exists());
    writer.write(Artifact::BuildLog, &[0; 8192]).unwrap();
    assert_eq!(std::fs::metadata(&log).unwrap().len(), 8192);
    writer.abort().unwrap();
}

#[test]
fn test_partial_artifacts_are_invisible_and_same_key_is_nonblocking() {
    let root = tempfile::tempdir().unwrap();
    let store = open_test_store(root.path(), 2, 1024);
    let mut owner = store.reserve(&key(1), 64).unwrap();
    assert!(store.acquire(&key(1)).unwrap().is_none());
    assert!(matches!(store.reserve(&key(1), 64), Err(StoreError::Busy("entry key"))));
    owner.write(Artifact::Executable, b"abc").unwrap();
    assert!(store.acquire(&key(1)).unwrap().is_none());
    let different = store.reserve(&key(2), 64).unwrap();
    different.abort().unwrap();
    owner.write(Artifact::Manifest, b"literal manifest").unwrap();
    owner.publish().unwrap();
    assert_eq!(store.inspect(&key(1)).unwrap(), Some(b"literal manifest".to_vec()));
}

#[test]
fn test_owned_copy_preserves_literal_bytes_and_refuses_replacement() {
    let root = tempfile::tempdir().unwrap();
    let store = open_test_store(root.path(), 2, 1024);
    let source = root.path().join("source space λ");
    std::fs::write(&source, b"abc").unwrap();
    let mut writer = store.reserve(&key(1), 64).unwrap();
    let token = writer.token;
    let copied = writer.copy(Artifact::Executable, &source).unwrap();
    assert_eq!(copied.len, 3);
    assert_eq!(format!("{:x}", Sha256::digest(b"abc")), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
    assert_eq!(copied.sha256, <[u8; 32]>::from(Sha256::digest(b"abc")));
    writer.write(Artifact::Manifest, b"manifest").unwrap();
    let executable = store.entry_path(token).join(Artifact::Executable.name());
    let moved = root.path().join("preserved original file");
    std::fs::rename(&executable, &moved).unwrap();
    std::fs::write(&executable, b"abc").unwrap();
    assert!(matches!(writer.publish(), Err(StoreError::Invalid(_))));
    assert_eq!(std::fs::read(moved).unwrap(), b"abc");
    assert_eq!(std::fs::read(executable).unwrap(), b"abc");
    assert!(store.acquire(&key(1)).unwrap().is_none());
}

#[test]
fn test_abort_deletes_only_files_this_exact_reservation_claimed() {
    let root = tempfile::tempdir().unwrap();
    let store = open_test_store(root.path(), 2, 1024);
    let mut writer = store.reserve(&key(1), 64).unwrap();
    let directory = store.entry_path(writer.token);
    let sentinel = directory.join(Artifact::Executable.name());
    std::fs::write(&sentinel, b"unclaimed existing bytes").unwrap();
    assert!(writer.write(Artifact::Executable, b"abc").is_err());
    assert!(writer.abort().is_err());
    assert_eq!(std::fs::read(&sentinel).unwrap(), b"unclaimed existing bytes");
    assert!(matches!(store.reserve(&key(1), 64), Err(StoreError::Busy(_))));

    let mut failed_copy = store.reserve(&key(2), 64).unwrap();
    let owned = store.entry_path(failed_copy.token);
    assert!(failed_copy.copy(Artifact::Executable, &root.path().join("missing source")).is_err());
    assert!(owned.join(Artifact::Executable.name()).is_file());
    failed_copy.abort().unwrap();
    assert!(!owned.exists());
    assert_eq!(std::fs::read(sentinel).unwrap(), b"unclaimed existing bytes");
}

#[test]
fn test_unrelated_files_and_live_leases_survive_retirement() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("notes.txt"), b"user file").unwrap();
    let foreign = root.path().join(".staging-999-0");
    std::fs::create_dir(&foreign).unwrap();
    std::fs::write(foreign.join("wip"), b"wip").unwrap();
    let legacy = root.path().join(key(99));
    std::fs::create_dir(&legacy).unwrap();
    std::fs::write(legacy.join("sentinel"), b"legacy entry is not imported").unwrap();
    let unrelated_hex_length = root.path().join("z".repeat(64));
    std::fs::create_dir(&unrelated_hex_length).unwrap();
    std::fs::write(unrelated_hex_length.join("sentinel"), b"keep").unwrap();
    let unrelated_lock = root.path().join("notes.lock");
    let file = File::create(&unrelated_lock).unwrap();
    file.set_times(std::fs::FileTimes::new().set_modified(std::time::UNIX_EPOCH)).unwrap();
    drop(file);
    let live_legacy_lock = root.path().join(format!("{}.lock", key(100)));
    std::fs::write(&live_legacy_lock, b"pid=999").unwrap();

    let store = open_test_store(root.path(), 2, 1024);
    let first = publish(&store, 1);
    let second = publish(&store, 2);
    let active = store.acquire(&key(1)).unwrap().unwrap();
    let third = publish(&store, 3);
    assert!(store.entry_path(first).is_dir());
    assert!(!store.entry_path(second).exists());
    assert!(store.entry_path(third).is_dir());
    assert_eq!(active.manifest().unwrap(), b"literal manifest");
    active.cancel().unwrap();
    assert_eq!(std::fs::read(root.path().join("notes.txt")).unwrap(), b"user file");
    assert_eq!(std::fs::read(foreign.join("wip")).unwrap(), b"wip");
    assert_eq!(std::fs::read(legacy.join("sentinel")).unwrap(), b"legacy entry is not imported");
    assert_eq!(std::fs::read(unrelated_hex_length.join("sentinel")).unwrap(), b"keep");
    assert!(unrelated_lock.is_file());
    assert_eq!(std::fs::read(live_legacy_lock).unwrap(), b"pid=999");
    assert_eq!(store.namespace().unwrap().state.slots.iter().filter(|slot| slot.entry.is_some()).count(), 2);

    let unknown = store.0.root.join("unrelated internal file");
    std::fs::write(&unknown, b"preserve and refuse").unwrap();
    assert!(store.reserve(&key(4), 64).is_err());
    assert_eq!(std::fs::read(unknown).unwrap(), b"preserve and refuse");
}

#[test]
fn test_unknown_execution_retains_durable_owner_after_handle_close() {
    let root = tempfile::tempdir().unwrap();
    let store = open_test_store(root.path(), 1, 1024);
    let entry = publish(&store, 1);
    let active = store.acquire(&key(1)).unwrap().unwrap().start().unwrap();
    let token = active.token;
    assert_eq!(active.executable().unwrap(), store.entry_path(entry).join(Artifact::Executable.name()));
    assert!(matches!(active.finish(None), Err(StoreError::OwnershipUnknown)));
    let reopened = Store::open(root.path(), store.0.limits).unwrap();
    let namespace = reopened.namespace().unwrap();
    let Some(Entry::Published { runs, .. }) = &namespace.state.slots[0].entry else {
        panic!("entry was retired without release authority")
    };
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].token, token);
    assert!(runs[0].started);
    drop(namespace);
    assert!(matches!(reopened.reserve(&key(2), 64), Err(StoreError::Capacity)));
    assert_eq!(std::fs::read(store.entry_path(entry).join(Artifact::Executable.name())).unwrap(), b"abc");
}

#[test]
fn test_generation_and_run_tokens_cannot_clear_replacements() {
    let root = tempfile::tempdir().unwrap();
    let store = open_test_store(root.path(), 1, 1024);
    let first = store.reserve(&key(1), 64).unwrap();
    let stale_artifact = first.token;
    first.abort().unwrap();
    let second = store.reserve(&key(2), 64).unwrap();
    assert!(second.token.generation > stale_artifact.generation);
    let current_artifact = second.token;
    drop(second);
    let mut namespace = store.namespace().unwrap();
    assert!(namespace.matching_reserved(stale_artifact).is_err());
    assert_eq!(namespace.matching_reserved(current_artifact).unwrap(), 64);
    assert!(namespace.run_mut(stale_artifact, false).is_err());
    drop(namespace);

    let separate = tempfile::tempdir().unwrap();
    let other = open_test_store(separate.path(), 1, 1024);
    publish(&other, 1);
    let old = other.acquire(&key(1)).unwrap().unwrap();
    let old_token = old.token;
    old.cancel().unwrap();
    let new = other.acquire(&key(1)).unwrap().unwrap();
    let new_token = new.token;
    assert_ne!(old_token, new_token);
    drop(new);
    let mut namespace = other.namespace().unwrap();
    assert!(namespace.run_mut(old_token, false).is_err());
    assert_eq!(namespace.run_mut(new_token, false).unwrap().len(), 1);
}

#[test]
fn test_old_store_handles_refuse_a_replacement_store() {
    let parent = tempfile::tempdir().unwrap();
    let old = open_test_store(parent.path(), 1, 1024);
    let mut writer = old.reserve(&key(1), 64).unwrap();
    let old_token = writer.token;
    let moved = parent.path().join("administratively isolated old store");
    // Deliberately violate the administrative quiescence rule to exercise a
    // surviving stale handle. The old namespace object stays anchored open.
    std::fs::rename(&old.0.root, &moved).unwrap();
    let replacement = open_test_store(parent.path(), 1, 1024);
    assert_ne!(old.0.identity, replacement.0.identity);
    let new_token = publish(&replacement, 1);
    assert_ne!(old_token.directory(), new_token.directory());
    let before = record(&replacement);
    assert!(writer.write(Artifact::BuildLog, b"must not reach replacement").is_err());
    assert!(writer.abort().is_err());
    assert_eq!(record(&replacement), before);
    assert_eq!(replacement.inspect(&key(1)).unwrap(), Some(b"literal manifest".to_vec()));
    assert!(moved.join(old_token.directory()).is_dir());
}

#[test]
fn test_run_capacity_and_counter_overflow_refuse_without_mutation() {
    let root = tempfile::tempdir().unwrap();
    let store = open_test_store(root.path(), 1, 1024);
    publish(&store, 1);
    let mut owners = Vec::new();
    for _ in 0..128 {
        owners.push(store.acquire(&key(1)).unwrap().unwrap());
    }
    let before = record(&store);
    assert!(matches!(store.acquire(&key(1)), Err(StoreError::Capacity)));
    assert_eq!(record(&store), before);

    let separate = tempfile::tempdir().unwrap();
    let generations = open_test_store(separate.path(), 1, 1024);
    let mut namespace = generations.namespace().unwrap();
    namespace.state.slots[0].generation = u64::MAX;
    namespace.persist(false).unwrap();
    drop(namespace);
    let before = record(&generations);
    assert!(matches!(generations.reserve(&key(1), 64), Err(StoreError::Capacity)));
    assert_eq!(record(&generations), before);
    for owner in owners {
        owner.cancel().unwrap();
    }
    let mut namespace = store.namespace().unwrap();
    namespace.state.next_token = u64::MAX;
    namespace.persist(false).unwrap();
    drop(namespace);
    let before = record(&store);
    assert!(matches!(store.acquire(&key(1)), Err(StoreError::Capacity)));
    assert_eq!(record(&store), before);
}

#[test]
fn test_partial_duplicate_oversized_and_unknown_records_are_never_reset() {
    for kind in ["partial", "duplicate", "oversized", "unknown", "foreign", "next"] {
        let root = tempfile::tempdir().unwrap();
        let store = open_test_store(root.path(), 1, 1024);
        let path = store.0.root.join(RECORD_FILE);
        let original = record(&store);
        match kind {
            "partial" => std::fs::write(&path, b"{\"version\":").unwrap(),
            "duplicate" => {
                let text = String::from_utf8(original).unwrap();
                std::fs::write(&path, text.replacen("\"version\":1", "\"version\":1,\"version\":1", 1)).unwrap();
            }
            "oversized" => File::create(&path).unwrap().set_len(RECORD_LIMIT + 1).unwrap(),
            "unknown" | "foreign" => {
                let mut value: serde_json::Value = serde_json::from_slice(&original).unwrap();
                if kind == "unknown" {
                    value["unexpected"] = true.into();
                } else {
                    value["identity"]["file"] = (store.0.identity.file ^ 0b1).into();
                }
                std::fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
            }
            "next" => {
                let mut file = File::create_new(store.0.root.join(NEXT_FILE)).unwrap();
                file.write_all(b"{\"interrupted\":").unwrap();
                file.sync_all().unwrap();
                file.set_times(std::fs::FileTimes::new().set_modified(std::time::UNIX_EPOCH)).unwrap();
            }
            _ => unreachable!(),
        }
        let corrupt = record(&store);
        assert!(Store::open(root.path(), store.0.limits).is_err(), "{kind}");
        assert!(store.reserve(&key(1), 64).is_err(), "{kind}");
        assert_eq!(record(&store), corrupt, "{kind}");
        if kind == "next" {
            assert_eq!(std::fs::read(store.0.root.join(NEXT_FILE)).unwrap(), b"{\"interrupted\":");
        }
    }
}

#[test]
fn test_permanent_lock_identity_and_policy_are_not_rewritten() {
    let root = tempfile::tempdir().unwrap();
    let store = open_test_store(root.path(), 1, 1024);
    let namespace = store.namespace().unwrap();
    let identity = namespace.state.slots[0].lock_identity;
    drop(namespace);
    for value in 1..=3 {
        publish(&store, value);
    }
    assert_eq!(store.namespace().unwrap().state.slots[0].lock_identity, identity);
    let before = record(&store);
    assert!(Store::open(root.path(), Limits { slots: 2, artifact_bytes: 1024 }).is_err());
    assert_eq!(record(&store), before);
    let lock_path = store.0.root.join("slot-0.lock");
    let original = root.path().join("old slot lock");
    std::fs::rename(&lock_path, &original).unwrap();
    File::create_new(&lock_path).unwrap();
    assert!(store.acquire(&key(3)).is_err());
    assert!(original.is_file());
    assert_eq!(record(&store), before);
}

// This controller launches only this exact libtest binary and a known helper
// that never spawns descendants. Its bounded wait/kill proves fixture control,
// not the product's arbitrary-child/descendant ownership contract.
struct LockChild {
    child: Option<Child>,
    directory: PathBuf,
}

fn retained_logs(directory: &Path) -> String {
    let mut output = format!("retained fixture {}", directory.display());
    for name in ["stdout.log", "stderr.log"] {
        match runner_cache_io::read_regular(&directory.join(name), 64 << 10) {
            Ok(bytes) => output.push_str(&format!("\n{name}:\n{}", String::from_utf8_lossy(&bytes))),
            Err(error) => output.push_str(&format!("\n{name}: bounded read refused: {error}")),
        }
    }
    output
}

impl LockChild {
    fn start(root: &Path, mode: &str) -> Self {
        let directory = root.join(format!("controller-{mode}"));
        std::fs::create_dir(&directory).unwrap();
        let stdout = File::create_new(directory.join("stdout.log")).unwrap();
        let stderr = File::create_new(directory.join("stderr.log")).unwrap();
        let child = Command::new(std::env::current_exe().unwrap())
            .args(["--ignored", "--exact", "runner_cache_store::tests::test_store_process_helper", "--nocapture"])
            .env("ADZE_TEST_STORE_ROOT", root)
            .env("ADZE_TEST_STORE_CONTROLLER", &directory)
            .env("ADZE_TEST_STORE_MODE", mode)
            .stdout(Stdio::from(stdout))
            .stderr(Stdio::from(stderr))
            .spawn().unwrap();
        Self { child: Some(child), directory }
    }

    fn wait_ready(&mut self) {
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            if self.directory.join("ready").try_exists().unwrap() {
                assert_eq!(std::fs::read(self.directory.join("ready")).unwrap(), b"ready");
                return;
            }
            if let Some(status) = self.child.as_mut().unwrap().try_wait().unwrap() {
                self.child = None;
                panic!("lock helper exited before ready ({status}); {}", retained_logs(&self.directory));
            }
            assert!(Instant::now() < deadline, "lock helper readiness deadline; {}", retained_logs(&self.directory));
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    fn wait_status(&mut self) -> std::process::ExitStatus {
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            if let Some(status) = self.child.as_mut().unwrap().try_wait().unwrap() {
                self.child = None;
                return status;
            }
            assert!(Instant::now() < deadline, "lock helper exit deadline; {}", retained_logs(&self.directory));
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    fn release(mut self) {
        std::fs::write(self.directory.join("release"), b"release").unwrap();
        let status = self.wait_status();
        assert!(status.success(), "lock helper {status}; {}", retained_logs(&self.directory));
        let stdout = runner_cache_io::read_regular(&self.directory.join("stdout.log"), 64 << 10).unwrap();
        assert!(String::from_utf8_lossy(&stdout).contains("test result: ok. 1 passed; 0 failed; 0 ignored;"));
    }

    fn kill(mut self) {
        self.child.as_mut().unwrap().kill().unwrap();
        let status = self.wait_status();
        assert!(!status.success());
    }
}

impl Drop for LockChild {
    fn drop(&mut self) {
        let Some(child) = self.child.as_mut() else { return };
        let _ = child.kill();
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            match child.try_wait() {
                Ok(Some(_)) => return,
                Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(10)),
                _ => {
                    eprintln!("UNCONFIRMED lock-fixture cleanup; {}", retained_logs(&self.directory));
                    return;
                }
            }
        }
    }
}

fn hold(controller: &Path) {
    write_new(&controller.join("ready.next"), b"ready").unwrap();
    std::fs::rename(controller.join("ready.next"), controller.join("ready")).unwrap();
    let deadline = Instant::now() + Duration::from_secs(45);
    while !controller.join("release").try_exists().unwrap() {
        assert!(Instant::now() < deadline, "lock-fixture controller did not release owner");
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
#[ignore = "controller-only helper; selected exactly by the real process test"]
fn test_store_process_helper() {
    let root = PathBuf::from(std::env::var_os("ADZE_TEST_STORE_ROOT").expect("controller root"));
    let controller = PathBuf::from(std::env::var_os("ADZE_TEST_STORE_CONTROLLER").expect("controller directory"));
    let mode = std::env::var("ADZE_TEST_STORE_MODE").expect("controller mode");
    let slots = if mode == "reserve" { 2 } else { 1 };
    let store = open_test_store(&root, slots, 1024);
    match mode.as_str() {
        "reserve" => {
            let writer = store.reserve(&key(1), 64).unwrap();
            hold(&controller);
            writer.abort().unwrap();
        }
        "namespace" => {
            let _namespace = store.namespace().unwrap();
            hold(&controller);
        }
        "lease" => {
            let lease = store.acquire(&key(1)).unwrap().unwrap();
            hold(&controller);
            lease.cancel().unwrap();
        }
        "partial-artifact" => {
            let mut writer = store.reserve(&key(1), 64).unwrap();
            let mut file = writer.claim(Artifact::Executable).unwrap();
            file.write_all(b"partial").unwrap();
            file.sync_all().unwrap();
            hold(&controller);
            panic!("partial writer must be terminated by its controller");
        }
        "partial-record" => {
            let namespace = store.namespace().unwrap();
            let serialized = serde_json::to_vec(&namespace.state).unwrap();
            let mut file = File::create_new(store.0.root.join(NEXT_FILE)).unwrap();
            file.write_all(&serialized[..serialized.len() / 2]).unwrap();
            file.sync_all().unwrap();
            hold(&controller);
            panic!("partial record writer must be terminated by its controller");
        }
        _ => panic!("unrecognized lock-fixture mode"),
    }
}

#[test]
fn test_real_process_contention_and_interrupted_owners_fail_closed() {
    let fixture = tempfile::Builder::new().prefix("adze store process λ ").tempdir().unwrap();
    let result = std::panic::catch_unwind(|| {
        for mode in ["reserve", "namespace", "lease", "partial-artifact", "partial-record"] {
            let root = fixture.path().join(mode);
            std::fs::create_dir(&root).unwrap();
            let slots = if mode == "reserve" { 2 } else { 1 };
            let store = open_test_store(&root, slots, 1024);
            if mode == "lease" { publish(&store, 1); }
            let mut child = LockChild::start(&root, mode);
            child.wait_ready();
            match mode {
                "reserve" => {
                    assert!(matches!(store.reserve(&key(1), 64), Err(StoreError::Busy(_))));
                    assert!(store.acquire(&key(1)).unwrap().is_none());
                    let different = store.reserve(&key(2), 64).unwrap();
                    different.abort().unwrap();
                    child.release();
                    store.reserve(&key(1), 64).unwrap().abort().unwrap();
                }
                "namespace" => {
                    let before = record(&store);
                    assert!(matches!(store.reserve(&key(1), 64), Err(StoreError::Busy("namespace"))));
                    assert_eq!(record(&store), before);
                    child.release();
                    store.reserve(&key(1), 64).unwrap().abort().unwrap();
                }
                "lease" => {
                    assert!(matches!(store.reserve(&key(2), 64), Err(StoreError::Capacity)));
                    store.acquire(&key(1)).unwrap().unwrap().cancel().unwrap();
                    child.kill();
                    assert!(matches!(store.reserve(&key(2), 64), Err(StoreError::Capacity)));
                    let namespace = store.namespace().unwrap();
                    let Some(Entry::Published { runs, .. }) = &namespace.state.slots[0].entry else { panic!("lost leased entry") };
                    assert_eq!(runs.len(), 1);
                    assert!(!runs[0].started);
                }
                "partial-artifact" => {
                    child.kill();
                    assert!(store.acquire(&key(1)).unwrap().is_none());
                    assert!(matches!(store.reserve(&key(2), 64), Err(StoreError::Capacity)));
                    let namespace = store.namespace().unwrap();
                    let token = namespace.state.slots[0].entry.as_ref().unwrap().token();
                    assert_eq!(std::fs::read(store.entry_path(token).join(Artifact::Executable.name())).unwrap(), b"partial");
                }
                "partial-record" => {
                    child.kill();
                    let incomplete = std::fs::read(store.0.root.join(NEXT_FILE)).unwrap();
                    assert!(serde_json::from_slice::<Namespace>(&incomplete).is_err());
                    assert!(store.reserve(&key(1), 64).is_err());
                    assert_eq!(std::fs::read(store.0.root.join(NEXT_FILE)).unwrap(), incomplete);
                }
                _ => unreachable!(),
            }
            eprintln!("actual store process case complete: {mode}");
        }
    });
    if let Err(error) = result {
        let retained = fixture.keep();
        eprintln!("retained failed store fixture: {}", retained.display());
        std::panic::resume_unwind(error);
    }
}
