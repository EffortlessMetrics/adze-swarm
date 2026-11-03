//! Actual two-thread Linux signal finalization controls for #1066.
//!
//! The callback receives pthread-directed OS signals. A fixture-only atomic
//! rendezvous holds it immediately before the same CAS used in production.
//! Existing runner_process_contract and real CLI cancellation controls remain
//! separate and unchanged. No PID-1 or complete descendant-owner claim follows.
#![cfg(target_os = "linux")]

#[path = "../src/runner_process.rs"]
mod runner_process;

use runner_process::signal_probe;
use runner_process::{CaptureCause, CapturePolicy};
use std::io::Write;
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

fn policy() -> CapturePolicy {
    CapturePolicy {
        stdout_bytes: 4096,
        stderr_bytes: 4096,
        total_timeout: Duration::from_secs(4),
        drain_timeout: Duration::from_millis(200),
        ..runner_process::CARGO_RUN_POLICY
    }
}

fn controller(case: &str) -> Output {
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args([
            "--exact",
            "fixture_signal_controller",
            "--ignored",
            "--nocapture",
        ])
        .env("ADZE_SIGNAL_CASE", case)
        .process_group(0);
    assert_cmd::Command::from_std(command)
        .timeout(Duration::from_secs(15))
        .output()
        .unwrap()
}

fn conventional(case: &str, signal: i32) {
    let output = controller(case);
    assert!(
        output.status.signal() == Some(signal) || output.status.code() == Some(128 + signal),
        "{case}: code={:?}, signal={:?}; {output:?}",
        output.status.code(),
        output.status.signal()
    );
    let marker = if matches!(case, "next_owner" | "relay_next_owner") {
        "second owned cleanup checked"
    } else {
        "owned cleanup checked"
    };
    assert!(
        String::from_utf8_lossy(&output.stdout).contains(marker),
        "the callback must not be released before real cleanup: {output:?}"
    );
}

#[test]
fn test_late_dispatched_sigint_relays_after_atomic_close() {
    conventional("late_int", libc::SIGINT);
}

#[test]
fn test_late_dispatched_sigterm_relays_after_atomic_close() {
    conventional("late_term", libc::SIGTERM);
}

#[test]
fn test_signal_winning_before_close_is_returned_after_cleanup() {
    conventional("early", libc::SIGTERM);
}

#[test]
fn test_delayed_callback_is_owned_by_a_subsequent_capture() {
    conventional("next_owner", libc::SIGTERM);
}

#[test]
fn test_successful_late_relay_allows_a_new_owner_to_clean_up() {
    conventional("relay_next_owner", libc::SIGTERM);
}

#[test]
fn test_restore_poison_refuses_reuse_and_late_callback_exits_once() {
    let output = controller("poison");
    assert_eq!(
        output.status.code(),
        Some(128 + libc::SIGTERM),
        "{output:?}"
    );
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.contains("owned cleanup checked"), "{output:?}");
    assert!(
        text.contains("poisoned reuse refused before spawn"),
        "{output:?}"
    );
}

#[test]
fn test_partial_setup_error_keeps_delayed_cancellation() {
    conventional("setup_error", libc::SIGTERM);
}

#[test]
fn test_partial_setup_drop_does_not_discard_delayed_cancellation() {
    conventional("setup_drop", libc::SIGTERM);
}

/// Deliberately ignored fixture plumbing, selected only by the eight controls.
#[test]
#[ignore = "actual signal subprocess selected by runner_linux_signal_contract"]
fn fixture_signal_controller() {
    let case = std::env::var("ADZE_SIGNAL_CASE").expect("signal fixture case");
    let signal = if case == "late_int" {
        libc::SIGINT
    } else {
        libc::SIGTERM
    };
    let temp = tempfile::tempdir().unwrap();
    let first = Paths::new(temp.path(), "first");
    let second = Paths::new(temp.path(), "second");
    let stop_receiver = Arc::new(AtomicBool::new(false));
    let receiver_done = Arc::new(AtomicBool::new(false));
    let (tid, receiver) = receiver_thread(stop_receiver.clone(), receiver_done.clone());

    if case != "early" {
        signal_probe::PAUSE_BEFORE_RECORD.store(true, Ordering::Release);
    }
    if case == "poison" {
        signal_probe::FAIL_RESTORE.store(libc::SIGTERM, Ordering::Release);
    }
    if case == "relay_next_owner" {
        signal_probe::PAUSE_BEFORE_RELAY.store(true, Ordering::Release);
    }

    let sender_case = case.clone();
    let sender_first = first.clone();
    let sender_second = second.clone();
    let sender_stop = stop_receiver.clone();
    let sender_done = receiver_done.clone();
    let sender = std::thread::spawn(move || {
        wait_for_file(&sender_first.ready);
        // SAFETY: the receiver remains live until this sender releases it.
        // pthread_kill targets that exact live thread, not the caller's group.
        assert_eq!(unsafe { libc::pthread_kill(tid, signal) }, 0);
        if sender_case == "early" {
            sender_stop.store(true, Ordering::Release);
            return;
        }
        wait_for_flag(&signal_probe::ENTERED);
        std::fs::write(&sender_first.finish, b"finish").unwrap();
        let until = Instant::now() + Duration::from_secs(4);
        while signal_probe::FINISH_COUNT.load(Ordering::Acquire) < 1 {
            assert!(Instant::now() < until, "first capture did not finalize");
            std::thread::sleep(Duration::from_millis(1));
        }
        assert_not_running(read_pid(&sender_first.descendant));

        if sender_case == "relay_next_owner" {
            // Move the first callback past its CLOSED observation, then start
            // a second capture before permitting the actual kill() relay.
            signal_probe::RELEASE.store(true, Ordering::Release);
            wait_for_flag(&signal_probe::RELAY_ENTERED);
            wait_for_file(&sender_second.ready);
            sender_stop.store(true, Ordering::Release);
            signal_probe::RELAY_RELEASE.store(true, Ordering::Release);
            return;
        }

        match sender_case.as_str() {
            "next_owner" => {
                wait_for_file(&sender_second.ready);
            }
            "setup_error" | "setup_drop" => {
                wait_for_flag(&signal_probe::INSTALL_ENTERED);
            }
            "poison" => {
                wait_for_file(&sender_first.checked);
            }
            _ => {}
        }
        println!("owned cleanup checked");
        std::io::stdout().flush().unwrap();
        sender_stop.store(true, Ordering::Release);
        signal_probe::RELEASE.store(true, Ordering::Release);
        if matches!(sender_case.as_str(), "setup_error" | "setup_drop") {
            // The old callback must record into the new ACTIVE guard before
            // the deliberately failed/panicking installation continues.
            wait_for_flag(&sender_done);
            signal_probe::INSTALL_RELEASE.store(true, Ordering::Release);
        }
    });

    let first_result = runner_process::capture(&mut first.command(), policy());
    match case.as_str() {
        "early" => {
            let error = first_result.unwrap_err();
            assert!(matches!(error.cause, CaptureCause::Cancelled), "{error:?}");
            assert!(error.cleanup.leader_reaped, "{error:?}");
            assert!(error.cleanup.problem.is_none(), "{error:?}");
            assert_not_running(read_pid(&first.descendant));
            sender.join().unwrap();
            receiver.join().unwrap();
            println!("owned cleanup checked");
            std::io::stdout().flush().unwrap();
            runner_process::exit_after_cancellation(error.cancellation().unwrap());
        }
        "next_owner" | "relay_next_owner" => {
            assert_eq!(first_result.unwrap().stdout, b"done");
            if case == "relay_next_owner" {
                wait_for_flag(&signal_probe::RELAY_ENTERED);
            }
            let error = runner_process::capture(&mut second.command(), policy()).unwrap_err();
            assert!(matches!(error.cause, CaptureCause::Cancelled), "{error:?}");
            assert!(error.cleanup.leader_reaped, "{error:?}");
            assert!(error.cleanup.problem.is_none(), "{error:?}");
            assert_not_running(read_pid(&second.descendant));
            sender.join().unwrap();
            receiver.join().unwrap();
            println!("second owned cleanup checked");
            std::io::stdout().flush().unwrap();
            runner_process::exit_after_cancellation(error.cancellation().unwrap());
        }
        "poison" => {
            let error = first_result.unwrap_err();
            assert!(matches!(error.cause, CaptureCause::Restore), "{error:?}");
            assert!(error.cleanup.leader_reaped, "{error:?}");
            assert!(error.cancellation().is_none());
            assert!(error.to_string().contains("signal restore:"));
            let error = runner_process::capture(&mut second.command(), policy()).unwrap_err();
            assert!(matches!(error.cause, CaptureCause::Setup(_)), "{error:?}");
            assert!(error.to_string().contains("poisoned"), "{error:?}");
            assert!(
                !second.ready.exists(),
                "poison must refuse before child creation"
            );
            println!("poisoned reuse refused before spawn");
            std::io::stdout().flush().unwrap();
            std::fs::write(&first.checked, b"checked").unwrap();
        }
        "setup_error" | "setup_drop" => {
            assert_eq!(first_result.unwrap().stdout, b"done");
            signal_probe::PAUSE_INSTALL.store(true, Ordering::Release);
            if case == "setup_error" {
                signal_probe::FAIL_INSTALL.store(true, Ordering::Release);
            } else {
                signal_probe::PANIC_INSTALL.store(true, Ordering::Release);
            }
            let error = runner_process::capture(&mut second.command(), policy()).unwrap_err();
            assert_eq!(
                case, "setup_error",
                "Drop must relay during the deliberate panic"
            );
            assert!(matches!(error.cause, CaptureCause::Setup(_)), "{error:?}");
            assert!(
                error
                    .to_string()
                    .contains("deliberate signal fixture partial-install failure")
            );
            assert!(
                !second.ready.exists(),
                "partial setup must precede child creation"
            );
            sender.join().unwrap();
            receiver.join().unwrap();
            runner_process::exit_after_cancellation(error.cancellation().unwrap());
        }
        "late_int" | "late_term" => {
            assert_eq!(first_result.unwrap().stdout, b"done");
        }
        other => panic!("unknown signal fixture {other}"),
    }

    sender.join().unwrap();
    receiver.join().unwrap();
    // A consumed callback is a distinct, finite regression failure. The red
    // fixture patch replaces only close's atomic swap with the old plain-load
    // semantics; both late-signal tests must then fail on this status.
    std::process::exit(95);
}

#[derive(Clone)]
struct Paths {
    ready: PathBuf,
    finish: PathBuf,
    descendant: PathBuf,
    checked: PathBuf,
}

impl Paths {
    fn new(root: &Path, name: &str) -> Self {
        Self {
            ready: root.join(format!("{name}.ready")),
            finish: root.join(format!("{name}.finish")),
            descendant: root.join(format!("{name}.descendant")),
            checked: root.join(format!("{name}.checked")),
        }
    }

    fn command(&self) -> Command {
        let mut command = Command::new("/bin/sh");
        // The descendant lives longer than the entire outer fixture watchdog,
        // so a stopped observation cannot be explained by natural expiry.
        command
            .args([
                "-c",
                "sleep 30 >/dev/null 2>&1 & printf '%s' \"$!\" > \"$DESCENDANT\"; : > \"$READY\"; while [ ! -f \"$FINISH\" ]; do sleep 0.01; done; printf done",
            ])
            .env("READY", &self.ready)
            .env("FINISH", &self.finish)
            .env("DESCENDANT", &self.descendant);
        command
    }
}

fn receiver_thread(
    stop: Arc<AtomicBool>,
    done: Arc<AtomicBool>,
) -> (libc::pthread_t, std::thread::JoinHandle<()>) {
    let (send, receive) = std::sync::mpsc::channel();
    let thread = std::thread::spawn(move || {
        // SAFETY: the initialized mask is local and both named signals are
        // valid. Only this fixture thread's mask is changed.
        let mut mask: libc::sigset_t = unsafe { std::mem::zeroed() };
        assert_eq!(unsafe { libc::sigemptyset(&mut mask) }, 0);
        assert_eq!(unsafe { libc::sigaddset(&mut mask, libc::SIGINT) }, 0);
        assert_eq!(unsafe { libc::sigaddset(&mut mask, libc::SIGTERM) }, 0);
        assert_eq!(
            unsafe { libc::pthread_sigmask(libc::SIG_UNBLOCK, &mask, std::ptr::null_mut()) },
            0
        );
        // SAFETY: pthread_self is valid for the currently running thread.
        send.send(unsafe { libc::pthread_self() }).unwrap();
        let until = Instant::now() + Duration::from_secs(10);
        while !stop.load(Ordering::Acquire) {
            assert!(Instant::now() < until, "signal receiver fixture expired");
            std::thread::sleep(Duration::from_millis(1));
        }
        done.store(true, Ordering::Release);
    });
    (
        receive.recv_timeout(Duration::from_secs(3)).unwrap(),
        thread,
    )
}

fn wait_for_flag(flag: &AtomicBool) {
    let until = Instant::now() + Duration::from_secs(4);
    while !flag.load(Ordering::Acquire) {
        assert!(Instant::now() < until, "signal fixture flag did not arrive");
        std::thread::sleep(Duration::from_millis(1));
    }
}

fn wait_for_file(path: &Path) {
    let until = Instant::now() + Duration::from_secs(4);
    while !path.exists() {
        assert!(
            Instant::now() < until,
            "fixture file did not arrive: {path:?}"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
}

fn read_pid(path: &Path) -> i32 {
    std::fs::read_to_string(path).unwrap().parse().unwrap()
}

fn assert_not_running(pid: i32) {
    let until = Instant::now() + Duration::from_secs(1);
    loop {
        let state = std::fs::read_to_string(format!("/proc/{pid}/stat"));
        if state
            .as_ref()
            .is_err_and(|error| error.kind() == std::io::ErrorKind::NotFound)
            || state.as_ref().is_ok_and(|stat| {
                stat.rsplit_once(')').is_some_and(|(_, tail)| {
                    matches!(tail.split_whitespace().next(), Some("Z" | "X"))
                })
            })
        {
            return;
        }
        assert!(
            Instant::now() < until,
            "owned fixture descendant {pid} is still running: {state:?}"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
}
