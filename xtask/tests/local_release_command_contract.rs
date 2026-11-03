//! Data/file-persistence controls for the unwired release journal.
//!
//! ExitStatus values below are explicit protocol fixtures. These tests do not
//! launch a child and cannot qualify process creation, custody or cancellation.

#[path = "../src/local_release_command.rs"]
mod local_release_command;

use local_release_command::{
    CapturePolicy, CommandJournal, CommandObservation, EnvironmentChange, JournalLimits,
    ReleaseCommand, SourceBinding,
};
use serde_json::Value;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::Path;
use std::process::ExitStatus;

fn binding() -> SourceBinding {
    SourceBinding {
        source_commit: "1234567890abcdef1234567890abcdef12345678".into(),
        source_tree: "abcdef1234567890abcdef1234567890abcdef12".into(),
        selected_version: "0.10.0".into(),
    }
}

fn limits() -> JournalLimits {
    JournalLimits {
        commands: 8,
        request_bytes: 64 * 1024,
        completion_bytes: 64 * 1024,
    }
}

fn policy() -> CapturePolicy {
    CapturePolicy {
        stdout_bytes: 64,
        stderr_bytes: 64,
        total_millis: 200,
        drain_millis: 20,
        cleanup_millis: 30,
    }
}

fn request(root: &Path) -> ReleaseCommand {
    ReleaseCommand::new(
        "fixture-command".into(),
        "fixture-tool".into(),
        vec!["one argument".into(), "λ".into(), "".into()],
        root.join("space λ"),
        Vec::new(),
        policy(),
    )
    .expect("valid fixture request")
}

fn status(code: u32) -> ExitStatus {
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        ExitStatus::from_raw((code as i32) << 8)
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::ExitStatusExt;
        ExitStatus::from_raw(code)
    }
}

fn read_json(path: &Path) -> Value {
    serde_json::from_slice(&fs::read(path).expect("record bytes")).expect("complete JSON record")
}

fn assert_unknown(record: &Value) {
    assert_eq!(record["package_first_qualified"], false);
    assert!(
        record["launch_evidence"]
            .as_str()
            .unwrap()
            .starts_with("unknown")
    );
    assert!(
        record["custody_evidence"]
            .as_str()
            .unwrap()
            .starts_with("unknown")
    );
}

#[test]
fn test_intent_owns_native_inputs_and_never_asserts_launch() {
    let root = tempfile::tempdir().unwrap();
    let mut journal = CommandJournal::create(root.path(), binding(), limits()).unwrap();
    let pending = journal.begin(request(root.path())).unwrap();
    assert_eq!(pending.request().program(), OsStr::new("fixture-tool"));
    assert_eq!(
        pending.request().arguments(),
        ["one argument", "λ", ""].map(OsString::from).as_slice()
    );
    assert_eq!(
        pending.request().cwd(),
        root.path().join("space λ").as_path()
    );
    assert_eq!(pending.request().policy().total_millis, 200);
    let path = root.path().join("commands/00000001/intent.json");
    let record = read_json(&path);
    assert_eq!(record["state"], "intent_recorded");
    assert_eq!(
        record["source"]["source_tree"],
        "abcdef1234567890abcdef1234567890abcdef12"
    );
    assert_eq!(record["request"]["arguments"][0]["display"], "one argument");
    assert_eq!(record["request"]["arguments"][1]["display"], "λ");
    assert_eq!(record["request"]["arguments"][2]["display"], "");
    assert_eq!(
        record["request"]["inherited_environment_values"],
        "not_recorded"
    );
    assert_eq!(
        record["request"]["inherited_environment_identity"],
        "unknown"
    );
    assert_unknown(&record);
    drop(pending);
    drop(journal);
    assert_eq!(read_json(&path), record);
    assert!(
        !root
            .path()
            .join("commands/00000001/completion.json")
            .exists()
    );
}

#[test]
fn test_raw_binary_outputs_and_natural_nonzero_status_are_preserved() {
    let root = tempfile::tempdir().unwrap();
    let mut journal = CommandJournal::create(root.path(), binding(), limits()).unwrap();
    let result = journal
        .begin(request(root.path()))
        .unwrap()
        .complete(CommandObservation::natural(
            status(37),
            b"\x00\xff\nx".to_vec(),
            b"abc".to_vec(),
        ))
        .unwrap();
    assert_eq!(result.natural_status().unwrap().code(), Some(37));
    assert_eq!(result.stdout(), b"\x00\xff\nx");
    assert_eq!(result.stderr(), b"abc");
    assert_eq!(result.request().program(), OsStr::new("fixture-tool"));
    result.verify_artifacts().unwrap();
    let record = read_json(result.completion_path());
    assert_eq!(record["state"], "observation_recorded");
    assert_eq!(record["natural_status"]["code"], 37);
    assert_eq!(
        record["stdout"]["sha256"],
        "578d6126089a1e285e430aa755bb637bbc1f23d08e0938a2bf3c9b06916cb62c"
    );
    assert_eq!(
        record["stderr"]["sha256"],
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    assert_eq!(record["stdout"]["retained_bytes"], 4);
    assert_eq!(record["stdout"]["truncated_by_journal"], false);
    assert_eq!(
        fs::read(root.path().join(record["stdout"]["path"].as_str().unwrap())).unwrap(),
        b"\x00\xff\nx"
    );
    assert_eq!(
        fs::read(root.path().join(record["stderr"]["path"].as_str().unwrap())).unwrap(),
        b"abc"
    );
    assert_unknown(&record);
    assert_eq!(
        record["capture_enforcement_evidence"],
        "unknown_pending_backend_report"
    );
}

#[derive(Debug)]
struct OriginalFailure;

impl std::fmt::Display for OriginalFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("original fixture capture failure")
    }
}

impl std::error::Error for OriginalFailure {}

#[test]
fn test_failed_capture_retains_partial_bytes_and_original_error_type() {
    let root = tempfile::tempdir().unwrap();
    let mut journal = CommandJournal::create(root.path(), binding(), limits()).unwrap();
    let error = journal
        .begin(request(root.path()))
        .unwrap()
        .complete(CommandObservation::failed(
            OriginalFailure.into(),
            Some(status(23)),
            b"partial\x00".to_vec(),
            b"cause".to_vec(),
        ))
        .err()
        .expect("capture failure must propagate");
    assert!(error.downcast_ref::<OriginalFailure>().is_some());
    let record = read_json(&root.path().join("commands/00000001/completion.json"));
    assert_eq!(record["natural_status"]["code"], 23);
    assert_eq!(
        record["observed_failure"],
        "original fixture capture failure"
    );
    assert_eq!(
        fs::read(root.path().join("commands/00000001/stdout.bin")).unwrap(),
        b"partial\x00"
    );
    assert_unknown(&record);
}

#[test]
fn test_recording_failure_cannot_erase_original_capture_error() {
    let root = tempfile::tempdir().unwrap();
    let mut journal = CommandJournal::create(root.path(), binding(), limits()).unwrap();
    let pending = journal.begin(request(root.path())).unwrap();
    let blocker = root.path().join("commands/00000001/stdout.bin");
    fs::write(&blocker, b"must not overwrite").unwrap();
    let error = pending
        .complete(CommandObservation::failed(
            OriginalFailure.into(),
            None,
            b"new output".to_vec(),
            Vec::new(),
        ))
        .err()
        .expect("recording/capture failure must propagate");
    assert!(error.downcast_ref::<OriginalFailure>().is_some());
    assert!(format!("{error:#}").contains("also failed to retain release command observation"));
    assert_eq!(fs::read(blocker).unwrap(), b"must not overwrite");
    assert!(
        !root
            .path()
            .join("commands/00000001/completion.json")
            .exists()
    );
    assert_eq!(
        read_json(&root.path().join("commands/00000001/intent.json"))["state"],
        "intent_recorded"
    );
}

#[test]
fn test_success_cannot_overwrite_an_existing_completion_record() {
    let root = tempfile::tempdir().unwrap();
    let mut journal = CommandJournal::create(root.path(), binding(), limits()).unwrap();
    let pending = journal.begin(request(root.path())).unwrap();
    let blocker = root.path().join("commands/00000001/completion.json");
    fs::write(&blocker, b"existing artifact").unwrap();
    assert!(
        pending
            .complete(CommandObservation::natural(
                status(0),
                b"output".to_vec(),
                Vec::new()
            ))
            .is_err()
    );
    assert_eq!(fs::read(blocker).unwrap(), b"existing artifact");
    assert_eq!(
        fs::read(root.path().join("commands/00000001/stdout.bin")).unwrap(),
        b"output"
    );
}

#[test]
fn test_oversized_observation_retains_bounded_prefixes_and_refuses_success() {
    let root = tempfile::tempdir().unwrap();
    let mut journal = CommandJournal::create(root.path(), binding(), limits()).unwrap();
    let request = ReleaseCommand::new(
        "bounded-fixture".into(),
        "tool".into(),
        Vec::new(),
        root.path().into(),
        Vec::new(),
        CapturePolicy {
            stdout_bytes: 3,
            stderr_bytes: 2,
            ..policy()
        },
    )
    .unwrap();
    let error = journal
        .begin(request)
        .unwrap()
        .complete(CommandObservation::natural(
            status(0),
            b"abcde".to_vec(),
            b"xyz".to_vec(),
        ))
        .err()
        .expect("a zero status does not excuse an oversized observation");
    assert!(
        error
            .to_string()
            .contains("exceeded its recorded capture policy")
    );
    let record = read_json(&root.path().join("commands/00000001/completion.json"));
    assert_eq!(record["stdout"]["observed_bytes"], 5);
    assert_eq!(record["stdout"]["retained_bytes"], 3);
    assert_eq!(record["stdout"]["truncated_by_journal"], true);
    assert_eq!(record["stderr"]["observed_bytes"], 3);
    assert_eq!(record["stderr"]["retained_bytes"], 2);
    assert_eq!(record["stderr"]["truncated_by_journal"], true);
    assert_eq!(
        fs::read(root.path().join("commands/00000001/stdout.bin")).unwrap(),
        b"abc"
    );
    assert_eq!(
        fs::read(root.path().join("commands/00000001/stderr.bin")).unwrap(),
        b"xy"
    );
    assert_unknown(&record);
}

#[test]
fn test_environment_order_and_explicit_redaction_do_not_change_memory_request() {
    let root = tempfile::tempdir().unwrap();
    let mut journal = CommandJournal::create(root.path(), binding(), limits()).unwrap();
    let secret = "sample_secret_must_not_appear_in_artifacts";
    let request = ReleaseCommand::new(
        "environment-fixture".into(),
        "tool".into(),
        Vec::new(),
        root.path().into(),
        vec![
            EnvironmentChange::Set {
                name: "MODE".into(),
                value: "first".into(),
                record_value: true,
            },
            EnvironmentChange::Remove("MODE".into()),
            EnvironmentChange::Set {
                name: "MODE".into(),
                value: "final".into(),
                record_value: true,
            },
            EnvironmentChange::Set {
                name: "PRIVATE".into(),
                value: secret.into(),
                record_value: false,
            },
        ],
        policy(),
    )
    .unwrap();
    let pending = journal.begin(request).unwrap();
    match &pending.request().environment()[3] {
        EnvironmentChange::Set {
            value,
            record_value,
            ..
        } => {
            assert_eq!(value.as_os_str(), OsStr::new(secret));
            assert!(!*record_value);
        }
        EnvironmentChange::Remove(_) => panic!("ordered environment mutation changed"),
    }
    let raw = fs::read_to_string(root.path().join("commands/00000001/intent.json")).unwrap();
    assert!(!raw.contains(secret));
    let record: Value = serde_json::from_str(&raw).unwrap();
    let changes = record["request"]["environment"].as_array().unwrap();
    assert_eq!(
        changes
            .iter()
            .map(|v| v["operation"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["set", "remove", "set", "set"]
    );
    assert_eq!(changes[0]["value"]["display"], "first");
    assert_eq!(changes[2]["value"]["display"], "final");
    assert_eq!(changes[3]["value_redacted"], true);
    assert!(changes[3]["value"].is_null());
}

#[test]
fn test_command_and_request_limits_leave_no_false_completion() {
    let root = tempfile::tempdir().unwrap();
    let mut journal = CommandJournal::create(
        root.path(),
        binding(),
        JournalLimits {
            commands: 1,
            ..limits()
        },
    )
    .unwrap();
    let too_large = ReleaseCommand::new(
        "too-large".into(),
        "tool".into(),
        vec![OsString::from("a".repeat(70_000))],
        root.path().into(),
        Vec::new(),
        policy(),
    )
    .unwrap();
    assert!(journal.begin(too_large).is_err());
    assert_eq!(
        fs::read_dir(root.path().join("commands")).unwrap().count(),
        0
    );
    drop(journal.begin(request(root.path())).unwrap());
    assert!(journal.begin(request(root.path())).is_err());
    assert!(!root.path().join("commands/00000002").exists());
    assert!(
        !root
            .path()
            .join("commands/00000001/completion.json")
            .exists()
    );
}

#[test]
fn test_namespace_reuse_and_recording_failure_refuse_further_work() {
    let root = tempfile::tempdir().unwrap();
    let mut journal = CommandJournal::create(root.path(), binding(), limits()).unwrap();
    assert!(CommandJournal::create(root.path(), binding(), limits()).is_err());
    let blocker = root.path().join("commands/00000001");
    fs::create_dir(&blocker).unwrap();
    assert!(journal.begin(request(root.path())).is_err());
    fs::remove_dir(blocker).unwrap();
    let error = journal
        .begin(request(root.path()))
        .err()
        .expect("failed recorder remains refused");
    assert!(error.to_string().contains("incomplete recording failure"));
}

#[test]
fn test_completion_byte_limit_retains_logs_but_publishes_no_partial_json() {
    let root = tempfile::tempdir().unwrap();
    let mut journal = CommandJournal::create(
        root.path(),
        binding(),
        JournalLimits {
            completion_bytes: 64,
            ..limits()
        },
    )
    .unwrap();
    let result =
        journal
            .begin(request(root.path()))
            .unwrap()
            .complete(CommandObservation::natural(
                status(0),
                b"retained".to_vec(),
                Vec::new(),
            ));
    assert!(result.is_err());
    assert_eq!(
        fs::read(root.path().join("commands/00000001/stdout.bin")).unwrap(),
        b"retained"
    );
    assert!(
        !root
            .path()
            .join("commands/00000001/completion.json")
            .exists()
    );
}

#[test]
fn test_failure_description_is_utf8_bounded_without_erasing_original_error() {
    let root = tempfile::tempdir().unwrap();
    let mut journal = CommandJournal::create(root.path(), binding(), limits()).unwrap();
    let description = "λ".repeat(6000);
    let error = journal
        .begin(request(root.path()))
        .unwrap()
        .complete(CommandObservation::failed(
            anyhow::anyhow!(description.clone()),
            None,
            Vec::new(),
            Vec::new(),
        ))
        .err()
        .expect("original failure");
    assert_eq!(error.to_string(), description);
    let record = read_json(&root.path().join("commands/00000001/completion.json"));
    assert_eq!(record["observed_failure_text_truncated"], true);
    assert_eq!(
        record["observed_failure"].as_str().unwrap(),
        "λ".repeat(4096)
    );
}

#[test]
fn test_invalid_source_or_request_is_rejected_before_intent() {
    let root = tempfile::tempdir().unwrap();
    let mut invalid = binding();
    invalid.source_commit = "short".into();
    assert!(CommandJournal::create(root.path(), invalid, limits()).is_err());
    let mut invalid = binding();
    invalid.selected_version = "^0.10".into();
    assert!(CommandJournal::create(root.path(), invalid, limits()).is_err());
    assert!(!root.path().join("commands").exists());
    assert!(
        ReleaseCommand::new(
            "bad".into(),
            "tool".into(),
            Vec::new(),
            "relative".into(),
            Vec::new(),
            policy()
        )
        .is_err()
    );
    assert!(
        ReleaseCommand::new(
            "bad".into(),
            "tool".into(),
            Vec::new(),
            root.path().into(),
            Vec::new(),
            CapturePolicy {
                total_millis: 0,
                ..policy()
            }
        )
        .is_err()
    );
}

#[cfg(unix)]
#[test]
fn test_unix_native_bytes_are_not_replaced_with_utf8_replacement_characters() {
    use std::os::unix::ffi::OsStringExt;
    let root = tempfile::tempdir().unwrap();
    let mut journal = CommandJournal::create(root.path(), binding(), limits()).unwrap();
    let command = ReleaseCommand::new(
        "native-unix".into(),
        OsString::from_vec(b"tool\xff".to_vec()),
        vec![OsString::from_vec(b"\xf0".to_vec())],
        root.path().into(),
        Vec::new(),
        policy(),
    )
    .unwrap();
    let pending = journal.begin(command).unwrap();
    assert_eq!(
        pending.request().program(),
        OsString::from_vec(b"tool\xff".to_vec()).as_os_str()
    );
    let record = read_json(&root.path().join("commands/00000001/intent.json"));
    assert_eq!(record["request"]["program"]["encoding"], "unix_bytes");
    assert_eq!(record["request"]["program"]["units_hex"], "746f6f6cff");
    assert!(record["request"]["program"]["display"].is_null());
    assert_eq!(record["request"]["arguments"][0]["units_hex"], "f0");
}

#[cfg(unix)]
#[test]
fn test_unix_signal_status_has_no_fabricated_exit_code() {
    use std::os::unix::process::ExitStatusExt;
    let root = tempfile::tempdir().unwrap();
    let mut journal = CommandJournal::create(root.path(), binding(), limits()).unwrap();
    let recorded = journal
        .begin(request(root.path()))
        .unwrap()
        .complete(CommandObservation::natural(
            ExitStatus::from_raw(9),
            Vec::new(),
            Vec::new(),
        ))
        .unwrap();
    let record = read_json(recorded.completion_path());
    assert!(record["natural_status"]["code"].is_null());
    assert_eq!(record["natural_status"]["signal"], 9);
    assert_eq!(record["natural_status"]["core_dumped"], false);
    assert_unknown(&record);
}

#[cfg(windows)]
#[test]
fn test_windows_native_unpaired_utf16_units_are_preserved() {
    use std::os::windows::ffi::OsStringExt;
    let root = tempfile::tempdir().unwrap();
    let mut journal = CommandJournal::create(root.path(), binding(), limits()).unwrap();
    let command = ReleaseCommand::new(
        "native-windows".into(),
        OsString::from_wide(&[0x0061, 0xd800, 0x0062]),
        Vec::new(),
        root.path().into(),
        Vec::new(),
        policy(),
    )
    .unwrap();
    drop(journal.begin(command).unwrap());
    let record = read_json(&root.path().join("commands/00000001/intent.json"));
    assert_eq!(record["request"]["program"]["encoding"], "windows_utf16_le");
    assert_eq!(record["request"]["program"]["units_hex"], "610000d86200");
    assert!(record["request"]["program"]["display"].is_null());
}

#[cfg(windows)]
#[test]
fn test_windows_exit_status_keeps_all_unsigned_bits() {
    let root = tempfile::tempdir().unwrap();
    let mut journal = CommandJournal::create(root.path(), binding(), limits()).unwrap();
    let recorded = journal
        .begin(request(root.path()))
        .unwrap()
        .complete(CommandObservation::natural(
            status(0xc000_013a),
            Vec::new(),
            Vec::new(),
        ))
        .unwrap();
    let record = read_json(recorded.completion_path());
    assert_eq!(record["natural_status"]["code"], 3_221_225_786_u64);
    assert_eq!(record["natural_status"]["platform"], "windows");
    assert_unknown(&record);
}

#[test]
fn test_changed_intent_is_rejected_before_observation_is_published() {
    let root = tempfile::tempdir().unwrap();
    let mut journal = CommandJournal::create(root.path(), binding(), limits()).unwrap();
    let pending = journal.begin(request(root.path())).unwrap();
    fs::write(root.path().join("commands/00000001/intent.json"), b"{}").unwrap();
    assert!(
        pending
            .complete(CommandObservation::natural(
                status(0),
                Vec::new(),
                Vec::new()
            ))
            .is_err()
    );
    assert!(
        !root
            .path()
            .join("commands/00000001/completion.json")
            .exists()
    );
}

#[test]
fn test_changed_retained_artifacts_are_detected_before_receipt_projection() {
    let root = tempfile::tempdir().unwrap();
    let mut journal = CommandJournal::create(root.path(), binding(), limits()).unwrap();
    let result = journal
        .begin(request(root.path()))
        .unwrap()
        .complete(CommandObservation::natural(
            status(0),
            b"stdout".to_vec(),
            b"stderr".to_vec(),
        ))
        .unwrap();
    for name in ["intent.json", "stdout.bin", "stderr.bin", "completion.json"] {
        let path = root.path().join("commands/00000001").join(name);
        let original = fs::read(&path).unwrap();
        let mut changed = original.clone();
        changed[0] ^= 1;
        fs::write(&path, &changed).unwrap();
        assert!(
            result.verify_artifacts().is_err(),
            "same-length tampering of {name} went undetected"
        );
        fs::write(&path, &original).unwrap();
        result.verify_artifacts().unwrap();
    }
}

#[test]
fn test_later_observation_does_not_close_an_earlier_dropped_intent() {
    let root = tempfile::tempdir().unwrap();
    let mut journal = CommandJournal::create(root.path(), binding(), limits()).unwrap();
    drop(journal.begin(request(root.path())).unwrap());
    let later = journal
        .begin(request(root.path()))
        .unwrap()
        .complete(CommandObservation::natural(
            status(0),
            Vec::new(),
            Vec::new(),
        ))
        .unwrap();
    assert!(
        !root
            .path()
            .join("commands/00000001/completion.json")
            .exists()
    );
    assert_eq!(
        read_json(&root.path().join("commands/00000001/intent.json"))["state"],
        "intent_recorded"
    );
    assert_eq!(read_json(later.completion_path())["id"], 2);
    assert_unknown(&read_json(later.completion_path()));
    later.verify_artifacts().unwrap();
}
