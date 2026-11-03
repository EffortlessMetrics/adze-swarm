//! Actual Linux child-process contracts for #1066.
//!
//! Controller subprocesses isolate process-global signal ownership. Workers
//! are finite shell fixtures; no fake child is a grammar/projection receipt.
//! The unchanged cli_test envelope and recovery canaries must run separately.
#![cfg(target_os = "linux")]

#[path = "../src/runner_process.rs"]
mod runner_process;

use runner_process::{CaptureCause, CapturePolicy, Channel};
use rustix::process::{Pid, Signal, getpid, kill_process};
use std::io::Read;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::time::{Duration, Instant};

fn policy() -> CapturePolicy {
    CapturePolicy {
        stdout_bytes: 1024,
        stderr_bytes: 1024,
        total_timeout: Duration::from_secs(2),
        drain_timeout: Duration::from_millis(200),
        ..runner_process::CARGO_RUN_POLICY
    }
}

fn shell(script: &str) -> Command {
    let mut command = Command::new("/bin/sh");
    command.arg("-c").arg(script);
    command
}

fn controller(case: &str) -> Output {
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args(["--exact", "fixture_controller", "--ignored", "--nocapture"])
        .env("ADZE_PROCESS_CASE", case)
        .process_group(0);
    assert_cmd::Command::from_std(command)
        .timeout(Duration::from_secs(15))
        .output()
        .unwrap()
}

fn passes(case: &str) {
    let output = controller(case);
    assert!(output.status.success(), "{case}: {output:?}");
}

#[test]
fn test_exact_bytes_and_natural_nonzero_status_are_preserved() {
    passes("bytes");
}

#[test]
fn test_stdout_limit_accepts_exact_bound_and_rejects_one_more_byte() {
    passes("stdout_limit");
}

#[test]
fn test_stderr_limit_accepts_exact_bound_and_rejects_one_more_byte() {
    passes("stderr_limit");
}

#[test]
fn test_concurrent_channel_floods_are_bounded_and_reaped() {
    passes("flood");
}

#[test]
fn test_large_concurrent_channels_are_completely_drained() {
    passes("large_channels");
}

#[test]
fn test_progress_does_not_restart_the_absolute_deadline() {
    passes("progress");
}

#[test]
fn test_closed_pipes_do_not_substitute_for_child_exit() {
    passes("closed_pipes");
}

#[test]
fn test_leader_exit_does_not_hide_descendant_held_pipes() {
    passes("held_pipes");
}

#[test]
fn test_success_also_terminates_remaining_owned_group_members() {
    passes("background");
}

#[test]
fn test_failed_setup_and_spawn_restore_ownership_and_dispositions() {
    passes("setup");
}

#[test]
fn test_group_teardown_leaves_an_unrelated_child_running() {
    passes("unrelated");
}

#[test]
fn test_cancellation_restores_disposition_before_conventional_relay() {
    let output = controller("cancel");
    assert!(
        output.status.signal() == Some(libc::SIGTERM)
            || output.status.code() == Some(128 + libc::SIGTERM),
        "{output:?}"
    );
}

#[test]
fn test_real_cli_sigint_cleans_group_and_unwinds_runner_directory() {
    cli_cancellation(Signal::Int, libc::SIGINT);
}

#[test]
fn test_real_cli_sigterm_cleans_group_and_unwinds_runner_directory() {
    cli_cancellation(Signal::Term, libc::SIGTERM);
}

/// Selected only by the outer tests in a fresh controller process. Its
/// ignored status is fixture plumbing, never a counted acceptance test.
#[test]
#[ignore = "subprocess fixture selected explicitly by runner process contracts"]
fn fixture_controller() {
    let case = std::env::var("ADZE_PROCESS_CASE").expect("controller case");
    match case.as_str() {
        "bytes" => {
            let temp = tempfile::Builder::new()
                .prefix("adze cwd ' λ ")
                .tempdir()
                .unwrap();
            let cwd = temp.path().canonicalize().unwrap();
            let argument = "one argument ' λ";
            let mut command = shell(
                "printf '%s\\n' \"$1\" \"$PWD\" \"$ADZE_USE_PURE_RUST\"; printf '\\377' >&2; exit 7",
            );
            command
                .arg("fixture")
                .arg(argument)
                .current_dir(&cwd)
                .env("ADZE_USE_PURE_RUST", "1");
            let output = runner_process::capture(&mut command, policy()).unwrap();
            assert_eq!(output.status.code(), Some(7));
            assert_eq!(
                output.stdout,
                format!("{argument}\n{}\n1\n", cwd.display()).as_bytes()
            );
            assert_eq!(output.stderr, [0xff]);
            assert_eq!(read_action(libc::SIGINT).sa_sigaction, libc::SIG_DFL);
            assert_eq!(read_action(libc::SIGTERM).sa_sigaction, libc::SIG_DFL);
        }
        "stdout_limit" | "stderr_limit" => {
            let stdout = case == "stdout_limit";
            let mut limits = policy();
            if stdout {
                limits.stdout_bytes = 8;
            } else {
                limits.stderr_bytes = 8;
            }
            let suffix = if stdout { "" } else { " >&2" };
            let output =
                runner_process::capture(&mut shell(&format!("printf 12345678{suffix}")), limits)
                    .unwrap();
            assert_eq!(
                if stdout { output.stdout } else { output.stderr },
                b"12345678"
            );
            let error =
                runner_process::capture(&mut shell(&format!("printf 123456789{suffix}")), limits)
                    .unwrap_err();
            let expected = if stdout {
                Channel::Stdout
            } else {
                Channel::Stderr
            };
            assert!(matches!(error.cause, CaptureCause::Limit(channel, 8) if channel == expected));
            assert_eq!(
                if stdout {
                    error.stdout.len()
                } else {
                    error.stderr.len()
                },
                8
            );
            assert!(error.cleanup.leader_reaped, "{error:?}");
            assert!(error.cleanup.problem.is_none(), "{error:?}");
            assert!(error.cancellation().is_none());
        }
        "flood" => {
            let limits = CapturePolicy {
                stdout_bytes: 128,
                stderr_bytes: 128,
                ..policy()
            };
            // Finite even if a regression ignores the capture policy.
            let mut command = shell(
                "i=0; while [ \"$i\" -lt 10000 ]; do printf 12345678; printf ABCDEFGH >&2; i=$((i+1)); done",
            );
            let error = runner_process::capture(&mut command, limits).unwrap_err();
            assert!(matches!(error.cause, CaptureCause::Limit(_, 128)), "{error:?}");
            assert!(error.stdout.len() <= 128 && error.stderr.len() <= 128);
            assert!(
                error.cleanup.group_signal_sent && error.cleanup.leader_reaped,
                "{error:?}"
            );
        }
        "large_channels" => {
            // Independent finite writers each exceed the Linux runner's
            // ordinary pipe capacity. A serial wait/read implementation
            // cannot rely on either channel fitting in its pipe.
            let bytes = 1024 * 1024;
            let limits = CapturePolicy {
                stdout_bytes: bytes,
                stderr_bytes: bytes,
                total_timeout: Duration::from_secs(10),
                ..runner_process::CARGO_RUN_POLICY
            };
            let mut command =
                shell("head -c 1048576 /dev/zero & head -c 1048576 /dev/zero >&2 & wait");
            let output = runner_process::capture(&mut command, limits).unwrap();
            assert!(output.status.success());
            assert_eq!(output.stdout.len(), bytes);
            assert_eq!(output.stderr.len(), bytes);
            assert!(output.stdout.iter().all(|byte| *byte == 0));
            assert!(output.stderr.iter().all(|byte| *byte == 0));
        }
        "progress" => {
            let limits = CapturePolicy {
                total_timeout: Duration::from_secs(1),
                ..policy()
            };
            let mut command =
                shell("i=0; while [ \"$i\" -lt 80 ]; do printf x; sleep 0.05; i=$((i+1)); done");
            let error = runner_process::capture(&mut command, limits).unwrap_err();
            assert!(matches!(error.cause, CaptureCause::Deadline), "{error:?}");
            assert!(!error.stdout.is_empty(), "the worker must have made progress");
            assert!(
                error.natural_status.is_none(),
                "our kill is not a natural exit: {error:?}"
            );
            assert!(error.cleanup.leader_reaped, "{error:?}");
        }
        "closed_pipes" => {
            let mut command = shell("exec >/dev/null 2>&1; exec sleep 30");
            let error = runner_process::capture(&mut command, policy()).unwrap_err();
            assert!(matches!(error.cause, CaptureCause::Deadline), "{error:?}");
            assert!(error.stdout.is_empty() && error.stderr.is_empty());
            assert!(error.natural_status.is_none());
            assert!(error.cleanup.leader_reaped, "{error:?}");
        }
        "held_pipes" => {
            for (redirect, status) in [("2>/dev/null", 0), (">/dev/null", 7)] {
                let temp = tempfile::tempdir().unwrap();
                let pid_file = temp.path().join("descendant.pid");
                let mut command = shell(&format!(
                    "sleep 30 {redirect} & printf '%s' \"$!\" > \"$PID_FILE\"; printf '\\377'; printf 'Error: controlled exit\\n' >&2; exit {status}"
                ));
                command.env("PID_FILE", &pid_file);
                let error = runner_process::capture(&mut command, policy()).unwrap_err();
                assert!(matches!(error.cause, CaptureCause::DrainDeadline), "{error:?}");
                assert_eq!(
                    error.natural_status.and_then(|exit| exit.code()),
                    Some(status)
                );
                assert!(
                    error.cleanup.group_signal_sent && error.cleanup.leader_reaped,
                    "{error:?}"
                );
                if status == 7 {
                    let message = error.to_string();
                    assert!(message.starts_with("static runner exited with"), "{message}");
                    assert!(!message.contains("UTF-8"), "no payload decoder runs here");
                }
                assert_not_running(read_pid(&pid_file));
            }
        }
        "background" => {
            let temp = tempfile::tempdir().unwrap();
            let pid_file = temp.path().join("descendant.pid");
            let mut command = shell(
                "sleep 30 >/dev/null 2>&1 & printf '%s' \"$!\" > \"$PID_FILE\"; printf done; exit 0",
            );
            command.env("PID_FILE", &pid_file);
            let output = runner_process::capture(&mut command, policy()).unwrap();
            assert!(output.status.success());
            assert_eq!(output.stdout, b"done");
            assert_not_running(read_pid(&pid_file));
        }
        "setup" => setup_controls(),
        "unrelated" => {
            let mut outsider = ReapOnDrop(Command::new("sleep").arg("30").spawn().unwrap());
            let limits = CapturePolicy {
                stdout_bytes: 8,
                ..policy()
            };
            let error = runner_process::capture(&mut shell("printf 123456789; sleep 30"), limits)
                .unwrap_err();
            assert!(matches!(
                error.cause,
                CaptureCause::Limit(Channel::Stdout, 8)
            ));
            assert!(
                outsider.0.try_wait().unwrap().is_none(),
                "unrelated child was terminated"
            );
        }
        "cancel" => {
            let temp = tempfile::tempdir().unwrap();
            let ready = temp.path().join("ready");
            let sender_ready = ready.clone();
            let sender = std::thread::spawn(move || {
                wait_for_file(&sender_ready);
                kill_process(getpid(), Signal::Term).unwrap();
            });
            let mut command = shell(": > \"$READY\"; exec sleep 30");
            command.env("READY", &ready);
            let error = runner_process::capture(&mut command, policy()).unwrap_err();
            sender.join().unwrap();
            assert!(matches!(error.cause, CaptureCause::Cancelled), "{error:?}");
            assert!(error.cleanup.leader_reaped, "{error:?}");
            assert_eq!(read_action(libc::SIGTERM).sa_sigaction, libc::SIG_DFL);
            runner_process::exit_after_cancellation(error.cancellation().unwrap());
        }
        other => panic!("unknown controller case {other}"),
    }
}

fn setup_controls() {
    let temp = tempfile::tempdir().unwrap();
    let marker = temp.path().join("must-not-spawn");
    let original_int = read_action(libc::SIGINT);
    let original_term = read_action(libc::SIGTERM);
    let original_child = read_action(libc::SIGCHLD);

    set_handler(libc::SIGINT, libc::SIG_IGN);
    set_handler(libc::SIGTERM, libc::SIG_IGN);
    let output = runner_process::capture(&mut shell("printf ignored"), policy()).unwrap();
    assert_eq!(output.stdout, b"ignored");
    assert_eq!(read_action(libc::SIGINT).sa_sigaction, libc::SIG_IGN);
    assert_eq!(read_action(libc::SIGTERM).sa_sigaction, libc::SIG_IGN);
    restore_action(libc::SIGINT, &original_int);
    restore_action(libc::SIGTERM, &original_term);

    extern "C" fn custom_handler(_: libc::c_int) {}
    let custom = custom_handler as *const () as libc::sighandler_t;
    set_handler(libc::SIGTERM, custom);
    let mut command = shell(": > \"$MARKER\"");
    command.env("MARKER", &marker);
    let error = runner_process::capture(&mut command, policy()).unwrap_err();
    assert!(matches!(error.cause, CaptureCause::Setup(_)), "{error:?}");
    assert!(!marker.exists());
    assert_eq!(read_action(libc::SIGTERM).sa_sigaction, custom);
    assert_eq!(
        read_action(libc::SIGINT).sa_sigaction,
        original_int.sa_sigaction
    );
    restore_action(libc::SIGTERM, &original_term);

    set_handler(libc::SIGCHLD, libc::SIG_IGN);
    let error = runner_process::capture(&mut command, policy()).unwrap_err();
    assert!(matches!(error.cause, CaptureCause::Setup(_)), "{error:?}");
    assert!(!marker.exists());
    restore_action(libc::SIGCHLD, &original_child);

    let mut missing = Command::new(temp.path().join("missing-executable"));
    let error = runner_process::capture(&mut missing, policy()).unwrap_err();
    assert!(matches!(error.cause, CaptureCause::Spawn(_)), "{error:?}");
    assert_eq!(
        read_action(libc::SIGINT).sa_sigaction,
        original_int.sa_sigaction
    );
    assert_eq!(
        read_action(libc::SIGTERM).sa_sigaction,
        original_term.sa_sigaction
    );
    let output = runner_process::capture(&mut shell("printf after-failure"), policy()).unwrap();
    assert_eq!(output.stdout, b"after-failure");
}

fn read_action(signal: i32) -> libc::sigaction {
    // SAFETY: a valid writable sigaction is initialized by this query.
    let mut action: libc::sigaction = unsafe { std::mem::zeroed() };
    assert_eq!(
        unsafe { libc::sigaction(signal, std::ptr::null(), &mut action) },
        0
    );
    action
}

fn restore_action(signal: i32, action: &libc::sigaction) {
    // SAFETY: action is initialized and remains live for the syscall.
    assert_eq!(
        unsafe { libc::sigaction(signal, action, std::ptr::null_mut()) },
        0
    );
}

fn set_handler(signal: i32, handler: libc::sighandler_t) {
    let mut action = read_action(signal);
    action.sa_sigaction = handler;
    action.sa_flags = 0;
    // SAFETY: action owns a valid writable signal mask.
    assert_eq!(unsafe { libc::sigemptyset(&mut action.sa_mask) }, 0);
    restore_action(signal, &action);
}

fn wait_for_file(path: &Path) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !path.exists() {
        assert!(
            Instant::now() < deadline,
            "fixture did not become ready: {}",
            path.display()
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn read_pid(path: &Path) -> u32 {
    std::fs::read_to_string(path).unwrap().trim().parse().unwrap()
}

fn assert_not_running(pid: u32) {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        match std::fs::read_to_string(format!("/proc/{pid}/stat")) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return,
            Ok(stat) => {
                let state = stat.rsplit_once(") ").unwrap().1.chars().next().unwrap();
                if matches!(state, 'Z' | 'X') {
                    // The CLI owns/reaps its leader, not the grandchild's
                    // adoption. A zombie has terminated and holds no pipes.
                    return;
                }
            }
            other => panic!("cannot inspect descendant {pid}: {other:?}"),
        }
        assert!(Instant::now() < deadline, "descendant {pid} is still running");
        std::thread::sleep(Duration::from_millis(10));
    }
}

struct ReapOnDrop(Child);

impl Drop for ReapOnDrop {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn cli_cancellation(signal: Signal, raw_signal: i32) {
    let temp = tempfile::Builder::new()
        .prefix("adze process ' λ ")
        .tempdir()
        .unwrap();
    let bin = temp.path().join("bin");
    std::fs::create_dir(&bin).unwrap();
    let cargo = bin.join("cargo");
    std::fs::write(&cargo, "#!/bin/sh\nset -eu\n[ \"$1\" = run ]\n[ \"$2\" = --quiet ]\n[ \"$3\" = --manifest-path ]\n[ \"$5\" = -- ]\n[ \"$6\" = \"$EXPECTED_INPUT\" ]\n[ \"$7\" = document-json ]\n[ \"$PWD\" = \"$EXPECTED_CWD\" ]\n[ \"$ADZE_USE_PURE_RUST\" = 1 ]\nprintf '%s' \"$4\" > \"$MANIFEST_FILE\"\nsleep 30 &\nprintf '%s' \"$!\" > \"$DESCENDANT_FILE\"\nprintf '%s' \"$$\" > \"$LEADER_FILE\"\n: > \"$READY\"\nwait\n").unwrap();
    std::fs::set_permissions(&cargo, std::fs::Permissions::from_mode(0o755)).unwrap();
    let grammar = temp.path().join("grammar ' λ.rs");
    let input = temp.path().join("input ' λ.txt");
    std::fs::write(&grammar, "#[adze::grammar(\"fixture\")] mod grammar {}\n").unwrap();
    std::fs::write(&input, "hello").unwrap();
    let expected_cwd = temp.path().canonicalize().unwrap();
    let ready = temp.path().join("ready");
    let leader = temp.path().join("leader.pid");
    let descendant = temp.path().join("descendant.pid");
    let manifest = temp.path().join("manifest");
    let mut paths = vec![bin];
    paths.extend(std::env::split_paths(&std::env::var_os("PATH").unwrap()));
    let mut command = Command::new(assert_cmd::cargo::cargo_bin!("adze"));
    command
        .arg("parse")
        .arg(&grammar)
        .arg(&input)
        .args(["--output", "document-json"])
        .current_dir(temp.path())
        .env("PATH", std::env::join_paths(paths).unwrap())
        .env("EXPECTED_INPUT", &input)
        .env("EXPECTED_CWD", &expected_cwd)
        .env("READY", &ready)
        .env("LEADER_FILE", &leader)
        .env("DESCENDANT_FILE", &descendant)
        .env("MANIFEST_FILE", &manifest)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut cli = ReapOnDrop(command.spawn().unwrap());
    wait_for_file(&ready);
    kill_process(Pid::from_child(&cli.0), signal).unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    let status = loop {
        if let Some(status) = cli.0.try_wait().unwrap() {
            break status;
        }
        assert!(Instant::now() < deadline, "CLI cancellation did not finish");
        std::thread::sleep(Duration::from_millis(10));
    };
    assert!(
        status.signal() == Some(raw_signal) || status.code() == Some(128 + raw_signal),
        "unexpected cancellation status: {status}"
    );
    // Observe termination before any pipe collection. The worker's 30-second
    // lifetime exceeds this fixture's entire 10-second CLI wait plus checks.
    assert_not_running(read_pid(&leader));
    assert_not_running(read_pid(&descendant));
    let runner_manifest = PathBuf::from(std::fs::read_to_string(manifest).unwrap());
    assert!(
        !runner_manifest.parent().unwrap().exists(),
        "runner directory was not unwound before CLI termination"
    );
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    cli.0
        .stdout
        .take()
        .unwrap()
        .read_to_end(&mut stdout)
        .unwrap();
    cli.0
        .stderr
        .take()
        .unwrap()
        .read_to_end(&mut stderr)
        .unwrap();
    assert!(stdout.is_empty(), "cancelled output leaked: {stdout:?}");
    assert!(String::from_utf8_lossy(&stderr).contains("cancellation"));
}
