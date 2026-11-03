//! Native Windows ownership/capture contracts for #1066.
//!
//! The actual Cargo-built adze.exe is always the bootstrap. The standalone
//! producer is a finite fake child; original generated CLI canaries remain
//! separate product proof. The ignored owner registration is fixture plumbing.
#![cfg(windows)]

#[path = "../src/runner_windows.rs"]
mod runner_windows;
#[path = "../src/runner_windows_handles.rs"]
mod runner_windows_handles;
#[path = "../src/runner_windows_request.rs"]
mod runner_windows_request;
#[path = "support/windows_fixture.rs"]
mod windows_fixture;

use runner_windows::{CaptureCause, CapturePolicy, Channel};
use runner_windows_handles::{OwnedChild, ReadResult};
use runner_windows_request::{
    EnvironmentChange, EnvironmentPolicy, MAX_REQUEST_BYTES, RunnerRequest,
};
use std::ffi::{OsStr, OsString};
use std::io;
use std::mem::{size_of, zeroed};
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::os::windows::process::ExitStatusExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Output, Stdio};
use std::ptr;
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};
use windows_sys::Win32::Foundation::{
    ERROR_ACCESS_DENIED, ERROR_INVALID_PARAMETER, WAIT_OBJECT_0, WAIT_TIMEOUT,
};
use windows_sys::Win32::Security::SECURITY_ATTRIBUTES;
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_ACTIVE_PROCESS,
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
    SetInformationJobObject,
};
use windows_sys::Win32::System::Memory::{FILE_MAP_WRITE, MapViewOfFile, UnmapViewOfFile};
use windows_sys::Win32::System::Threading::{
    CreateEventW, GetCurrentProcess, GetProcessHandleCount, OpenProcess, PROCESS_SYNCHRONIZE,
    WaitForSingleObject,
};

static SERIAL: Mutex<()> = Mutex::new(());

fn serial() -> MutexGuard<'static, ()> {
    SERIAL.lock().unwrap_or_else(|error| error.into_inner())
}

fn adze() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_adze"))
}

fn policy() -> CapturePolicy {
    CapturePolicy {
        stdout_bytes: 128 * 1024,
        stderr_bytes: 128 * 1024,
        total_timeout: Duration::from_secs(8),
        drain_timeout: Duration::from_millis(500),
        ..runner_windows::CARGO_RUN_POLICY
    }
}

fn request(args: &[&str]) -> RunnerRequest {
    RunnerRequest {
        program: windows_fixture::fixture_exe().as_os_str().to_owned(),
        args: args.iter().map(|arg| OsString::from(*arg)).collect(),
        environment: EnvironmentPolicy::Inherit,
        changes: Vec::new(),
        cwd: None,
    }
}

fn capture(
    request: &RunnerRequest,
    policy: CapturePolicy,
) -> Result<Output, runner_windows::CaptureError> {
    runner_windows::capture(request, &adze(), policy)
}

fn assert_cleanup(error: &runner_windows::CaptureError) {
    assert!(error.cleanup.job_terminated, "{error:?}");
    assert!(error.cleanup.leader_closed, "{error:?}");
    assert_eq!(error.cleanup.active_processes, Some(0), "{error:?}");
    assert!(error.cleanup.problem.is_none(), "{error:?}");
}

fn hex(value: &OsStr) -> String {
    value
        .encode_wide()
        .map(|unit| format!("{unit:04x}"))
        .collect()
}

fn unhex(value: &str) -> OsString {
    let units: Vec<u16> = value
        .as_bytes()
        .chunks_exact(4)
        .map(|part| u16::from_str_radix(std::str::from_utf8(part).unwrap(), 16).unwrap())
        .collect();
    OsString::from_wide(&units)
}

fn wait_pid(path: &Path) -> u32 {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Ok(text) = std::fs::read_to_string(path)
            && let Ok(pid) = text.parse()
        {
            return pid;
        }
        assert!(
            Instant::now() < deadline,
            "fixture did not publish {}",
            path.display()
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn opened_process(pid: u32) -> io::Result<OwnedHandle> {
    // SAFETY: the PID comes only from this test's owned fixture registration.
    let raw = unsafe { OpenProcess(PROCESS_SYNCHRONIZE, 0, pid) };
    if raw.is_null() {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: OpenProcess returned a new valid object handle.
    Ok(unsafe { OwnedHandle::from_raw_handle(raw) })
}

fn assert_stopped(pid: u32) {
    let process = match opened_process(pid) {
        Ok(process) => process,
        Err(error) if error.raw_os_error() == Some(ERROR_INVALID_PARAMETER as i32) => return,
        Err(error) => panic!("could not establish fixture liveness: {error}"),
    };
    // SAFETY: retain the object handle, so this observation cannot hit a reused PID.
    assert_eq!(
        unsafe { WaitForSingleObject(process.as_raw_handle(), 1000) },
        WAIT_OBJECT_0,
        "30-second fixture survived owned cleanup"
    );
}

struct FixtureOwner(Child);

impl FixtureOwner {
    fn wait(&mut self) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while self.0.try_wait().unwrap().is_none() {
            assert!(Instant::now() < deadline, "owned fixture did not exit");
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

impl Drop for FixtureOwner {
    fn drop(&mut self) {
        if !matches!(self.0.try_wait(), Ok(Some(_))) {
            let _ = self.0.kill();
            let deadline = Instant::now() + Duration::from_secs(1);
            while Instant::now() < deadline && !matches!(self.0.try_wait(), Ok(Some(_))) {
                std::thread::sleep(Duration::from_millis(10));
            }
        }
    }
}

fn raw_bootstrap(bytes: &[u8]) -> Output {
    let mut child = OwnedChild::spawn(&adze(), bytes, || Ok(())).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let mut closed = [false, false];
    let mut status = None;
    loop {
        assert!(Instant::now() < deadline, "raw bootstrap did not finish");
        if status.is_none() {
            status = child.status().unwrap();
        }
        for (index, pipe, bytes) in [
            (0, &child.stdout, &mut stdout),
            (1, &child.stderr, &mut stderr),
        ] {
            if closed[index] {
                continue;
            }
            let mut buffer = [0; 1024];
            match pipe.read(&mut buffer).unwrap() {
                ReadResult::Bytes(count) => {
                    assert!(
                        bytes.len() + count <= 8192,
                        "raw bootstrap diagnostic exceeded test bound"
                    );
                    bytes.extend_from_slice(&buffer[..count]);
                }
                ReadResult::Idle => {}
                ReadResult::Closed => closed[index] = true,
            }
        }
        if status.is_some() && closed == [true, true] {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    child.terminate().unwrap();
    assert!(child.leader_closed());
    let cleanup_deadline = Instant::now() + Duration::from_secs(2);
    while child.active_processes().unwrap() != 0 {
        assert!(
            Instant::now() < cleanup_deadline,
            "raw bootstrap job not empty"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    Output {
        status: ExitStatus::from_raw(status.unwrap()),
        stdout,
        stderr,
    }
}

#[test]
fn test_request_wire_has_literal_version_policy_and_ordered_records() {
    let request = RunnerRequest {
        program: "x".into(),
        args: Vec::new(),
        environment: EnvironmentPolicy::Clear,
        cwd: None,
        changes: vec![
            EnvironmentChange::Remove("K".into()),
            EnvironmentChange::Set("k".into(), "v".into()),
        ],
    };
    let expected: Vec<u8> = "41 44 5a 57 52 51 30 31 01 00 00 00 40 00 00 00
        01 00 00 00 00 00 00 00 01 00 00 00 78 00 00 00 00 00 02 00 00 00
        00 00 00 00 01 00 00 00 4b 00 01 00 00 00 01 00 00 00 6b 00
        01 00 00 00 76 00"
        .split_whitespace()
        .map(|byte| u8::from_str_radix(byte, 16).unwrap())
        .collect();
    assert_eq!(request.encode().unwrap(), expected);
    assert_eq!(RunnerRequest::decode(&expected).unwrap(), request);
}

#[test]
fn test_request_cap_is_explicit_and_checked_before_child_creation() {
    let _serial = serial();
    let mut request = request(&["inspect", ""]);
    let base = request.encode().unwrap().len();
    request.args[1] = OsString::from_wide(&vec![b'a' as u16; (MAX_REQUEST_BYTES - base) / 2]);
    let encoded = request.encode().unwrap();
    assert_eq!(encoded.len(), MAX_REQUEST_BYTES);
    assert_eq!(RunnerRequest::decode(&encoded).unwrap(), request);
    request.args[1].push("a");
    let error = capture(&request, policy()).unwrap_err();
    assert!(
        matches!(&error.cause, CaptureCause::Setup("request", _)),
        "{error:?}"
    );
    assert!(!error.cleanup.job_terminated);
    assert!(error.natural_status.is_none());
}

#[test]
fn test_request_rejects_bad_counts_versions_extents_and_nul_without_unicode_loss() {
    let _serial = serial();
    let mut request = request(&["inspect"]);
    request
        .args
        .push(OsString::from_wide(&[0xd800, b'x' as u16, 0xdc00]));
    assert_eq!(
        RunnerRequest::decode(&request.encode().unwrap()).unwrap(),
        request
    );
    let valid = RunnerRequest {
        program: "x".into(),
        args: Vec::new(),
        environment: EnvironmentPolicy::Inherit,
        changes: Vec::new(),
        cwd: None,
    }
    .encode()
    .unwrap();
    for (offset, value) in [
        (8, 2),
        (12, u32::MAX),
        (16, 2),
        (20, 2),
        (24, u32::MAX),
        (30, u32::MAX),
        (34, u32::MAX),
    ] {
        let mut bad = valid.clone();
        bad[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        assert!(
            RunnerRequest::decode(&bad).is_err(),
            "accepted field {offset}"
        );
    }
    let mut trailing = valid.clone();
    trailing.push(0);
    let length = trailing.len() as u32;
    trailing[12..16].copy_from_slice(&length.to_le_bytes());
    assert!(RunnerRequest::decode(&trailing).is_err());
    for length in [0, 7, 15, valid.len() - 1] {
        assert!(RunnerRequest::decode(&valid[..length]).is_err());
    }
    request.args.push(OsString::from_wide(&[b'x' as u16, 0]));
    assert!(request.encode().is_err());
}

#[test]
fn test_native_readonly_mapping_and_malformed_bootstrap_refusal() {
    let _serial = serial();
    let bytes = request(&["inspect"]).encode().unwrap();
    let mapping = runner_windows_handles::request_mapping(&bytes).unwrap();
    // SAFETY: this test owns the mapping; an unexpected writable view is
    // immediately unmapped before the assertion reports the regression.
    let writable = unsafe { MapViewOfFile(mapping.raw(), FILE_MAP_WRITE, 0, 0, bytes.len()) };
    let error = io::Error::last_os_error();
    if !writable.Value.is_null() {
        unsafe { UnmapViewOfFile(writable) };
    }
    assert!(
        writable.Value.is_null(),
        "request handle retained writable access"
    );
    assert_eq!(error.raw_os_error(), Some(ERROR_ACCESS_DENIED as i32));
    for change in ["version", "oversize", "short mapping", "count", "trailing"] {
        let mut malformed = bytes.clone();
        match change {
            "version" => malformed[8..12].copy_from_slice(&2_u32.to_le_bytes()),
            "oversize" => {
                malformed[12..16].copy_from_slice(&((MAX_REQUEST_BYTES + 1) as u32).to_le_bytes())
            }
            "short mapping" => {
                let length = malformed.len() as u32 + 65536;
                malformed[12..16].copy_from_slice(&length.to_le_bytes());
            }
            "count" => malformed[24..28].copy_from_slice(&u32::MAX.to_le_bytes()),
            "trailing" => {
                malformed.push(0);
                let length = malformed.len() as u32;
                malformed[12..16].copy_from_slice(&length.to_le_bytes());
            }
            _ => unreachable!(),
        }
        let output = raw_bootstrap(&malformed);
        assert!(!output.status.success(), "{change}");
        assert!(output.stdout.is_empty(), "{change}");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("request decode:"),
            "{change}: {output:?}"
        );
    }
}

#[test]
fn test_bootstrap_argument_and_handle_failures_are_non_success() {
    let _serial = serial();
    assert!(runner_windows::bootstrap_if_requested().is_none());
    let pseudo_handle = (usize::MAX - 1).to_string();
    for args in [
        vec!["--adze-runner-bootstrap-v1"],
        vec!["--adze-runner-bootstrap-v1", "not-a-handle"],
        vec!["--adze-runner-bootstrap-v1", "0"],
        vec!["--adze-runner-bootstrap-v1", "1"],
        vec!["--adze-runner-bootstrap-v1", pseudo_handle.as_str()],
        vec!["--adze-runner-bootstrap-v1", "1", "extra"],
    ] {
        let output = assert_cmd::Command::new(adze())
            .args(args)
            .timeout(Duration::from_secs(3))
            .output()
            .unwrap();
        assert!(!output.status.success(), "{output:?}");
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8_lossy(&output.stderr).contains("static runner bootstrap failed"));
    }
    assert_eq!(
        runner_windows::Cancellation::Interrupt.exit_code() as u32,
        0xc000013a
    );
    assert_eq!(
        runner_windows::Cancellation::Break.exit_code() as u32,
        0xc000013a
    );
}

#[test]
fn test_native_arguments_environment_cwd_and_closed_stdin_are_preserved() {
    let _serial = serial();
    let directory = tempfile::Builder::new()
        .prefix("adze cwd ' λ ")
        .tempdir()
        .unwrap();
    let mut request = request(&["inspect", "", "a b", "λ🙂", "quote\"inside", "trailing\\"]);
    request
        .args
        .push(OsString::from_wide(&[0xd800, b'x' as u16, 0xdc00]));
    request.args.push(
        directory
            .path()
            .join("unresolved")
            .join("..")
            .join("input λ.txt")
            .into_os_string(),
    );
    request.cwd = Some(directory.path().to_owned());
    request.changes = vec![
        EnvironmentChange::Set("ADZE_USE_PURE_RUST".into(), "1".into()),
        EnvironmentChange::Set(
            "CARGO_TARGET_DIR".into(),
            directory.path().join("target λ").into_os_string(),
        ),
    ];
    let output = capture(&request, policy()).unwrap();
    assert!(output.status.success(), "{output:?}");
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.starts_with("job=1\n"), "{text}");
    let actual: Vec<&str> = text
        .lines()
        .filter_map(|line| line.strip_prefix("arg="))
        .collect();
    let expected: Vec<String> = request.args.iter().map(|arg| hex(arg)).collect();
    assert_eq!(actual, expected);
    assert!(text.contains("stdin=0\n"), "{text}");
    assert!(text.contains(&format!(
        "env:ADZE_USE_PURE_RUST={}\n",
        hex(OsStr::new("1"))
    )));
    assert!(text.contains(&format!(
        "env:CARGO_TARGET_DIR={}\n",
        hex(directory.path().join("target λ").as_os_str())
    )));
    let cwd = text
        .lines()
        .find_map(|line| line.strip_prefix("cwd="))
        .unwrap();
    assert_eq!(
        PathBuf::from(unhex(cwd)).canonicalize().unwrap(),
        directory.path().canonicalize().unwrap()
    );
}

#[test]
fn test_child_path_resolution_and_cwd_match_independent_stable_command() {
    let _serial = serial();
    let directory = tempfile::tempdir().unwrap();
    let bin = directory.path().join("selected bin λ");
    let cwd = directory.path().join("different cwd");
    std::fs::create_dir(&bin).unwrap();
    std::fs::create_dir(&cwd).unwrap();
    let executable = bin.join("adze-probe.exe");
    std::fs::copy(windows_fixture::fixture_exe(), &executable).unwrap();
    let paths = std::env::join_paths([bin.as_path()]).unwrap();
    let request = RunnerRequest {
        program: "adze-probe.exe".into(),
        args: vec!["inspect".into()],
        environment: EnvironmentPolicy::Inherit,
        changes: vec![EnvironmentChange::Set("PATH".into(), paths.clone())],
        cwd: Some(cwd.clone()),
    };
    let output = capture(&request, policy()).unwrap();
    assert!(output.status.success(), "{output:?}");
    let control = Command::new("adze-probe.exe")
        .arg("inspect")
        .env("PATH", paths)
        .current_dir(&cwd)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(control.status.success(), "{control:?}");
    let text = String::from_utf8(output.stdout).unwrap();
    let expected = String::from_utf8(control.stdout).unwrap();
    for key in ["exe=", "cwd="] {
        let actual = text
            .lines()
            .find_map(|line| line.strip_prefix(key))
            .unwrap();
        let expected = expected
            .lines()
            .find_map(|line| line.strip_prefix(key))
            .unwrap();
        assert_eq!(actual, expected);
        let expected_path = if key == "exe=" { &executable } else { &cwd };
        assert_eq!(
            PathBuf::from(unhex(actual)).canonicalize().unwrap(),
            expected_path.canonicalize().unwrap()
        );
    }
}

#[test]
fn test_batch_file_command_handling_remains_the_standard_library_contract() {
    let _serial = serial();
    let directory = tempfile::Builder::new()
        .prefix("adze batch λ ")
        .tempdir()
        .unwrap();
    let script = directory.path().join("probe script.cmd");
    std::fs::write(&script, "@echo off\r\nif not \"%ADZE_USE_PURE_RUST%\"==\"1\" exit /b 19\r\necho %~1\r\necho %~2\r\necho %~3\r\nexit /b 7\r\n").unwrap();
    let args = ["hello world", "lambda", "trailing\\"];
    let request = RunnerRequest {
        program: script.clone().into_os_string(),
        args: args.iter().map(|arg| OsString::from(*arg)).collect(),
        environment: EnvironmentPolicy::Inherit,
        changes: vec![EnvironmentChange::Set(
            "ADZE_USE_PURE_RUST".into(),
            "1".into(),
        )],
        cwd: Some(directory.path().to_owned()),
    };
    let output = capture(&request, policy()).unwrap();
    let control = Command::new(&script)
        .args(args)
        .env("ADZE_USE_PURE_RUST", "1")
        .current_dir(directory.path())
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(7), "{output:?}");
    assert_eq!(output.status, control.status);
    assert_eq!(output.stdout, control.stdout);
    assert_eq!(output.stderr, control.stderr);
}

#[test]
fn test_clear_and_ordered_case_variant_environment_changes_match_stable_command() {
    let _serial = serial();
    let mut request = request(&["inspect"]);
    request.environment = EnvironmentPolicy::Clear;
    request.changes = vec![
        EnvironmentChange::Set("ADZE_WINDOWS_KEEP".into(), "first".into()),
        EnvironmentChange::Set("adze_windows_keep".into(), "last λ".into()),
        EnvironmentChange::Set("ADZE_WINDOWS_REMOVE".into(), "present".into()),
        EnvironmentChange::Remove("adze_windows_remove".into()),
    ];
    let output = capture(&request, policy()).unwrap();
    assert!(output.status.success(), "{output:?}");
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains(&format!(
        "env:ADZE_WINDOWS_KEEP={}\n",
        hex(OsStr::new("last λ"))
    )));
    assert!(text.contains("env:ADZE_WINDOWS_REMOVE=<absent>\n"));
    // Independently construct the standard-library control, not request.command().
    let control = Command::new(windows_fixture::fixture_exe())
        .arg("inspect")
        .env_clear()
        .env("ADZE_WINDOWS_KEEP", "first")
        .env("adze_windows_keep", "last λ")
        .env("ADZE_WINDOWS_REMOVE", "present")
        .env_remove("adze_windows_remove")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(control.status.success(), "{control:?}");
    let expected = String::from_utf8(control.stdout).unwrap();
    assert_eq!(
        text.lines()
            .filter(|line| line.starts_with("env:"))
            .collect::<Vec<_>>(),
        expected
            .lines()
            .filter(|line| line.starts_with("env:"))
            .collect::<Vec<_>>()
    );
}

#[test]
fn test_explicit_handle_allowlist_excludes_an_inheritable_sibling_handle() {
    let _serial = serial();
    let name = format!("Local\\adze-sentinel-{}", std::process::id());
    let wide_name: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
    let attributes = SECURITY_ATTRIBUTES {
        nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: ptr::null_mut(),
        bInheritHandle: 1,
    };
    // SAFETY: this test owns a uniquely named event intentionally inheritable
    // by the independent standard-Command control, but absent from our allowlist.
    let raw = unsafe { CreateEventW(&attributes, 1, 0, wide_name.as_ptr()) };
    assert!(!raw.is_null());
    // SAFETY: newly created, valid event handle.
    let event = unsafe { OwnedHandle::from_raw_handle(raw) };
    let value = (event.as_raw_handle() as usize).to_string();
    let control = Command::new(windows_fixture::fixture_exe())
        .arg("inspect")
        .env("SENTINEL_HANDLE", &value)
        .env("SENTINEL_NAME", &name)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(control.status.success(), "{control:?}");
    assert!(
        String::from_utf8_lossy(&control.stdout).contains("sentinel=1\n"),
        "sentinel control did not inherit its event"
    );
    let mut request = request(&["inspect"]);
    request.changes = vec![
        EnvironmentChange::Set("SENTINEL_HANDLE".into(), value.into()),
        EnvironmentChange::Set("SENTINEL_NAME".into(), name.into()),
    ];
    let output = capture(&request, policy()).unwrap();
    assert!(output.status.success(), "{output:?}");
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("sentinel=0\n"),
        "{output:?}"
    );
}

#[test]
fn test_successfully_drains_both_streams_larger_than_pipe_capacity() {
    let _serial = serial();
    let bytes = 1024 * 1024;
    let output = capture(
        &request(&["emit", &bytes.to_string(), &bytes.to_string()]),
        CapturePolicy {
            stdout_bytes: bytes,
            stderr_bytes: bytes,
            ..policy()
        },
    )
    .unwrap();
    assert!(output.status.success(), "{output:?}");
    assert_eq!(output.stdout, vec![b'o'; bytes]);
    assert_eq!(output.stderr, vec![b'e'; bytes]);
}

#[test]
fn test_stdout_and_stderr_accept_exact_caps_and_refuse_one_more_byte() {
    let _serial = serial();
    for limit in [0, 17, 8192] {
        let limits = CapturePolicy {
            stdout_bytes: limit,
            stderr_bytes: limit,
            ..policy()
        };
        let output = capture(
            &request(&["emit", &limit.to_string(), &limit.to_string()]),
            limits,
        )
        .unwrap();
        assert!(output.status.success());
        assert_eq!((output.stdout.len(), output.stderr.len()), (limit, limit));
        for channel in [Channel::Stdout, Channel::Stderr] {
            let count = (limit + 1).to_string();
            let args = if channel == Channel::Stdout {
                ["emit", count.as_str(), "0"]
            } else {
                ["emit", "0", count.as_str()]
            };
            let error = capture(&request(&args), limits).unwrap_err();
            assert!(
                matches!(&error.cause, CaptureCause::Limit(found, size) if *found == channel && *size == limit),
                "{error:?}"
            );
            assert!(error.stdout.len() <= limit && error.stderr.len() <= limit);
            assert_cleanup(&error);
        }
    }
}

#[test]
fn test_zero_write_and_late_write_close_never_discard_queued_bytes() {
    let _serial = serial();
    let output = capture(&request(&["zero-write"]), policy()).unwrap();
    assert!(output.status.success());
    assert_eq!(output.stdout, b"after-zero-write");
    for delay in [0, 1, 5, 9, 10, 11, 19] {
        let output = capture(&request(&["delayed-write", &delay.to_string()]), policy()).unwrap();
        assert!(output.status.success());
        assert_eq!(output.stdout, b"after-idle-write");
    }
}

#[test]
fn test_no_progress_and_closed_external_streams_cannot_outlive_absolute_deadline() {
    let _serial = serial();
    for mode in ["quiet", "closed"] {
        let started = Instant::now();
        let error = capture(
            &request(&[mode]),
            CapturePolicy {
                total_timeout: Duration::from_millis(500),
                ..policy()
            },
        )
        .unwrap_err();
        assert!(
            matches!(&error.cause, CaptureCause::Deadline),
            "{mode}: {error:?}"
        );
        assert!(
            error.natural_status.is_none(),
            "forced termination became a natural exit"
        );
        assert_cleanup(&error);
        assert!(started.elapsed() < Duration::from_secs(4));
    }
}

#[test]
fn test_continuous_progress_does_not_restart_deadline_and_flood_is_bounded() {
    let _serial = serial();
    let error = capture(
        &request(&["progress"]),
        CapturePolicy {
            total_timeout: Duration::from_millis(500),
            ..policy()
        },
    )
    .unwrap_err();
    assert!(matches!(&error.cause, CaptureCause::Deadline), "{error:?}");
    assert!(!error.stdout.is_empty(), "progress control never executed");
    assert_cleanup(&error);
    let error = capture(
        &request(&["flood"]),
        CapturePolicy {
            stdout_bytes: 128,
            stderr_bytes: 128,
            ..policy()
        },
    )
    .unwrap_err();
    assert!(
        matches!(&error.cause, CaptureCause::Limit(_, 128)),
        "{error:?}"
    );
    assert!(error.stdout.len() <= 128 && error.stderr.len() <= 128);
    assert_cleanup(&error);
}

#[test]
fn test_leader_first_descendant_held_streams_have_finite_drain_and_cleanup() {
    let _serial = serial();
    for stream in ["stdout", "stderr"] {
        let directory = tempfile::tempdir().unwrap();
        let pid_file = directory.path().join("descendant.pid");
        let mut request = request(&["descendant"]);
        request
            .args
            .extend([pid_file.clone().into_os_string(), stream.into(), "0".into()]);
        let error = capture(&request, policy()).unwrap_err();
        assert!(
            matches!(&error.cause, CaptureCause::DrainDeadline),
            "{error:?}"
        );
        assert_eq!(error.natural_status.unwrap().code(), Some(0));
        assert_cleanup(&error);
        assert_stopped(wait_pid(&pid_file));
    }
}

#[test]
fn test_known_nonzero_status_survives_incomplete_capture_and_cleanup() {
    let _serial = serial();
    let directory = tempfile::tempdir().unwrap();
    let pid_file = directory.path().join("descendant.pid");
    let mut request = request(&["descendant"]);
    request.args.extend([
        pid_file.clone().into_os_string(),
        "pipes".into(),
        "7".into(),
    ]);
    let error = capture(&request, policy()).unwrap_err();
    assert!(
        matches!(&error.cause, CaptureCause::DrainDeadline),
        "{error:?}"
    );
    assert_eq!(error.natural_status.unwrap().code(), Some(7));
    assert!(error.to_string().starts_with("static runner exited with"));
    assert!(error.stdout.starts_with(b"\xff"));
    assert_cleanup(&error);
    assert_stopped(wait_pid(&pid_file));
}

#[test]
fn test_success_also_stops_descendants_which_closed_both_pipes() {
    let _serial = serial();
    let directory = tempfile::tempdir().unwrap();
    let pid_file = directory.path().join("descendant.pid");
    let mut request = request(&["descendant"]);
    request.args.extend([
        pid_file.clone().into_os_string(),
        "closed".into(),
        "0".into(),
    ]);
    let output = capture(&request, policy()).unwrap();
    assert!(output.status.success(), "{output:?}");
    assert_stopped(wait_pid(&pid_file));
}

#[test]
fn test_terminal_259_and_high_bit_codes_are_forwarded_without_narrowing() {
    let _serial = serial();
    for code in [0_u32, 7, 259, 0x8000ffff, 0xc000013a] {
        let output = capture(&request(&["exit", &code.to_string()]), policy()).unwrap();
        assert_eq!(output.status.code().unwrap() as u32, code);
    }
}

#[test]
fn test_creation_and_external_spawn_failures_keep_their_stage() {
    let _serial = serial();
    let directory = tempfile::tempdir().unwrap();
    let missing = directory.path().join("missing.exe");
    let error = runner_windows::capture(&request(&["inspect"]), &missing, policy()).unwrap_err();
    assert!(matches!(&error.cause, CaptureCause::Spawn(_)), "{error:?}");
    assert!(error.natural_status.is_none());
    let mut request = request(&["inspect"]);
    request.program = missing.into_os_string();
    let output = capture(&request, policy()).unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("external spawn:"),
        "{output:?}"
    );
    assert!(output.stdout.is_empty());
}

#[test]
fn test_repeated_capture_does_not_inherit_stale_pipe_handles() {
    let _serial = serial();
    let warmup = capture(&request(&["emit", "0", "0"]), policy()).unwrap();
    assert!(warmup.status.success());
    let count = || {
        let mut value = 0;
        // SAFETY: current-process pseudo handle is borrowed, not closed.
        assert_ne!(
            unsafe { GetProcessHandleCount(GetCurrentProcess(), &mut value) },
            0
        );
        value
    };
    let before = count();
    for _ in 0..12 {
        let output = capture(&request(&["emit", "0", "0"]), policy()).unwrap();
        assert!(output.status.success());
        assert!(output.stdout.is_empty() && output.stderr.is_empty());
    }
    // The two libtest worker handles may turn over while unrelated codec tests
    // run. A per-invocation process/pipe/mapping leak across twelve runs cannot
    // hide in this fixed harness allowance; the wake event was warmed above.
    assert!(
        count() <= before + 2,
        "native handles accumulated across captures"
    );
}

fn controlled_case(kind: &str) {
    let directory = tempfile::Builder::new()
        .prefix("adze controller λ ")
        .tempdir()
        .unwrap();
    windows_fixture::copy_as_cargo(directory.path());
    let mut request = request(&["controller", kind]);
    request.args.extend([
        adze().into_os_string(),
        directory.path().as_os_str().to_owned(),
    ]);
    request.changes.push(EnvironmentChange::Set(
        "ADZE_FIXTURE_ERROR".into(),
        directory.path().join("controller.error").into_os_string(),
    ));
    let result = capture(
        &request,
        CapturePolicy {
            total_timeout: Duration::from_secs(20),
            ..policy()
        },
    );
    let details =
        std::fs::read_to_string(directory.path().join("controller.error")).unwrap_or_default();
    let parent =
        std::fs::read_to_string(directory.path().join("parent.stderr")).unwrap_or_default();
    let output = result.unwrap_or_else(|error| panic!("{kind}: {error:?}\n{details}\n{parent}"));
    assert!(
        output.status.success(),
        "{kind}: {output:?}\n{details}\n{parent}"
    );
    let receipt = std::fs::read_to_string(directory.path().join("result.txt")).unwrap();
    assert!(
        receipt.contains("descendant=stopped\nsibling=alive\n"),
        "{receipt}"
    );
}

#[test]
fn test_running_parent_death_closes_noninherited_job_without_killing_sibling() {
    let _serial = serial();
    controlled_case("death");
}

#[test]
fn test_isolated_ctrl_c_cleans_job_and_unwinds_actual_runner_directory() {
    let _serial = serial();
    controlled_case("interrupt");
}

#[test]
fn test_isolated_ctrl_break_cleans_job_and_unwinds_actual_runner_directory() {
    let _serial = serial();
    controlled_case("break");
}

#[test]
fn test_inherited_ignored_ctrl_c_remains_ignored_until_ctrl_break() {
    let _serial = serial();
    controlled_case("ignored");
}

#[test]
fn test_parent_death_at_post_creation_checkpoint_releases_bootstrap_job() {
    let _serial = serial();
    let directory = tempfile::tempdir().unwrap();
    let receipt = directory.path().join("bootstrap.pid");
    let mut owner = FixtureOwner(
        Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "fixture_owner", "--ignored", "--nocapture"])
            .env("ADZE_WINDOWS_OWNER", "1")
            .env("ADZE_OWNER_BOOTSTRAP", adze())
            .env("ADZE_OWNER_PROGRAM", windows_fixture::fixture_exe())
            .env("ADZE_OWNER_RECEIPT", &receipt)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let bootstrap = opened_process(wait_pid(&receipt)).unwrap();
    // SAFETY: the creator has just published its owned process object ID,
    // without waiting for external-program readiness; retain the object now.
    assert_eq!(
        unsafe { WaitForSingleObject(bootstrap.as_raw_handle(), 0) },
        WAIT_TIMEOUT
    );
    owner.0.kill().unwrap();
    owner.wait();
    assert_eq!(
        unsafe { WaitForSingleObject(bootstrap.as_raw_handle(), 5000) },
        WAIT_OBJECT_0
    );
}

#[test]
fn test_outer_job_process_limit_refuses_creation_without_external_execution() {
    let _serial = serial();
    let directory = tempfile::tempdir().unwrap();
    let marker = directory.path().join("external.marker");
    let output = assert_cmd::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "fixture_owner", "--ignored", "--nocapture"])
        .env("ADZE_WINDOWS_OWNER", "restricted")
        .env("ADZE_OWNER_BOOTSTRAP", adze())
        .env("ADZE_OWNER_PROGRAM", windows_fixture::fixture_exe())
        .env("ADZE_OWNER_RECEIPT", &marker)
        .timeout(Duration::from_secs(10))
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    assert!(
        !marker.exists(),
        "job rejection fell back to an unowned launch"
    );
}

#[test]
#[ignore = "subprocess registration used only by the parent-death control"]
fn fixture_owner() {
    let mode = std::env::var("ADZE_WINDOWS_OWNER").unwrap();
    if mode == "restricted" {
        // This nested job permits only its current controller process. Native
        // child creation must fail, not retry outside the requested job tree.
        let raw = unsafe { CreateJobObjectW(ptr::null(), ptr::null()) };
        assert!(!raw.is_null());
        // SAFETY: the new job is valid and owned. It deliberately has no
        // kill-on-close flag because this fixture itself becomes its member.
        let job = unsafe { OwnedHandle::from_raw_handle(raw) };
        // SAFETY: this Win32 scalar structure has a valid all-zero value.
        let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { zeroed() };
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_ACTIVE_PROCESS;
        limits.BasicLimitInformation.ActiveProcessLimit = 1;
        assert_ne!(
            unsafe {
                SetInformationJobObject(
                    job.as_raw_handle(),
                    JobObjectExtendedLimitInformation,
                    (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                    size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                )
            },
            0
        );
        assert_ne!(
            unsafe { AssignProcessToJobObject(job.as_raw_handle(), GetCurrentProcess()) },
            0
        );
        let request = RunnerRequest {
            program: std::env::var_os("ADZE_OWNER_PROGRAM").unwrap(),
            args: vec![
                "mark".into(),
                std::env::var_os("ADZE_OWNER_RECEIPT").unwrap(),
            ],
            environment: EnvironmentPolicy::Inherit,
            changes: Vec::new(),
            cwd: None,
        };
        let error = runner_windows::capture(
            &request,
            &PathBuf::from(std::env::var_os("ADZE_OWNER_BOOTSTRAP").unwrap()),
            policy(),
        )
        .unwrap_err();
        assert!(matches!(&error.cause, CaptureCause::Spawn(_)), "{error:?}");
        assert!(error.natural_status.is_none());
        return;
    }
    assert_eq!(mode, "1");
    let request = RunnerRequest {
        program: std::env::var_os("ADZE_OWNER_PROGRAM").unwrap(),
        args: vec!["quiet".into()],
        environment: EnvironmentPolicy::Inherit,
        changes: Vec::new(),
        cwd: None,
    };
    let child = OwnedChild::spawn(
        &PathBuf::from(std::env::var_os("ADZE_OWNER_BOOTSTRAP").unwrap()),
        &request.encode().unwrap(),
        || Ok(()),
    )
    .unwrap();
    std::fs::write(
        PathBuf::from(std::env::var_os("ADZE_OWNER_RECEIPT").unwrap()),
        child.process_id.to_string(),
    )
    .unwrap();
    std::thread::sleep(Duration::from_secs(30));
    drop(child);
}
