//! Linux custody for the static runner's existing process/output contract.
//!
//! This module owns one child process group, nonblocking pipe reads and finite
//! wait/drain/cleanup phases. It does not interpret JSON, parser outcomes or
//! cache entries. Descendants which deliberately leave the group are outside
//! this local execution boundary; this is not a sandbox.

use rustix::fs::{OFlags, fcntl_getfl, fcntl_setfl};
use rustix::process::{
    Pid, Signal, WaitId, WaitIdOptions, getpid, kill_process, kill_process_group, waitid,
};
use std::fmt;
use std::io::{self, Read};
use std::os::fd::AsFd;
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::process::{Child, ChildStderr, ChildStdout, Command, ExitStatus, Output, Stdio};
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::time::{Duration, Instant};

const READ_CHUNK: usize = 8 * 1024;
const READ_TURN: usize = 64 * 1024;
const POLL_INTERVAL: Duration = Duration::from_millis(10);
const MAX_STDOUT: usize = 64 * 1024 * 1024;
const MAX_STDERR: usize = 1024 * 1024;

/// Initial limits for one Cargo-build-and-run invocation, in bytes and seconds.
/// The total deadline never restarts when bytes arrive or the leader exits.
pub(crate) const CARGO_RUN_POLICY: CapturePolicy = CapturePolicy {
    stdout_bytes: MAX_STDOUT,
    stderr_bytes: MAX_STDERR,
    total_timeout: Duration::from_secs(600),
    drain_timeout: Duration::from_secs(2),
    cleanup_timeout: Duration::from_secs(2),
};

#[derive(Clone, Copy, Debug)]
pub(crate) struct CapturePolicy {
    pub(crate) stdout_bytes: usize,
    pub(crate) stderr_bytes: usize,
    pub(crate) total_timeout: Duration,
    pub(crate) drain_timeout: Duration,
    pub(crate) cleanup_timeout: Duration,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Channel {
    Stdout,
    Stderr,
}

impl fmt::Display for Channel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Stdout => "stdout",
            Self::Stderr => "stderr",
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Cancellation {
    Interrupt,
    Terminate,
}

impl Cancellation {
    fn from_raw(signal: i32) -> Option<Self> {
        match signal {
            libc::SIGINT => Some(Self::Interrupt),
            libc::SIGTERM => Some(Self::Terminate),
            _ => None,
        }
    }

    fn raw(self) -> i32 {
        match self {
            Self::Interrupt => libc::SIGINT,
            Self::Terminate => libc::SIGTERM,
        }
    }

    fn signal(self) -> Signal {
        match self {
            Self::Interrupt => Signal::INT,
            Self::Terminate => Signal::TERM,
        }
    }
}

impl fmt::Display for Cancellation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Interrupt => "SIGINT",
            Self::Terminate => "SIGTERM",
        })
    }
}

#[derive(Debug)]
pub(crate) enum CaptureCause {
    Setup(String),
    UnsupportedInitProcess,
    Spawn(io::Error),
    Read(Channel, io::Error),
    Observe(io::Error),
    Limit(Channel, usize),
    Deadline,
    DrainDeadline,
    Cancelled,
    Cleanup,
    Restore,
}

/// A process failure keeps capture/cleanup causes separate from an exit which
/// was observed before intervention. A SIGKILL we send is never a natural exit.
pub(crate) struct CaptureError {
    pub(crate) cause: CaptureCause,
    pub(crate) natural_status: Option<ExitStatus>,
    pub(crate) stdout: Vec<u8>,
    pub(crate) stderr: Vec<u8>,
    pub(crate) cleanup: Box<Cleanup>,
    cancellation: Option<Cancellation>,
    restore_error: Option<io::Error>,
}

impl CaptureError {
    fn before_spawn(cause: CaptureCause) -> Self {
        Self {
            cause,
            natural_status: None,
            stdout: Vec::new(),
            stderr: Vec::new(),
            cleanup: Box::new(Cleanup::not_spawned()),
            cancellation: None,
            restore_error: None,
        }
    }

    pub(crate) fn cancellation(&self) -> Option<Cancellation> {
        self.cancellation
    }
}

impl fmt::Debug for CaptureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CaptureError")
            .field("cause", &self.cause)
            .field("natural_status", &self.natural_status)
            .field("stdout_bytes", &self.stdout.len())
            .field("stderr_bytes", &self.stderr.len())
            .field("cleanup", &self.cleanup)
            .field("cancellation", &self.cancellation)
            .field("restore_error", &self.restore_error)
            .finish()
    }
}

impl fmt::Display for CaptureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(status) = self.natural_status.filter(|status| !status.success()) {
            write!(f, "static runner exited with {status}; ")?;
        }
        match &self.cause {
            CaptureCause::Setup(message) => {
                write!(f, "static runner capture setup failed: {message}")?
            }
            CaptureCause::UnsupportedInitProcess => f.write_str(
                "static runner signal custody is unavailable when the CLI is Linux PID 1",
            )?,
            CaptureCause::Spawn(error) => write!(f, "static runner could not start: {error}")?,
            CaptureCause::Read(channel, error) => {
                write!(f, "static runner {channel} read failed: {error}")?
            }
            CaptureCause::Observe(error) => {
                write!(f, "static runner child observation failed: {error}")?
            }
            CaptureCause::Limit(channel, limit) => write!(
                f,
                "static runner {channel} exceeded its {limit}-byte capture limit"
            )?,
            CaptureCause::Deadline => {
                f.write_str("static runner exceeded its absolute process deadline")?
            }
            CaptureCause::DrainDeadline => {
                f.write_str("static runner pipe drain exceeded its post-exit deadline")?
            }
            CaptureCause::Cancelled => f.write_str("static runner was cancelled")?,
            CaptureCause::Cleanup => f.write_str("static runner cleanup could not be confirmed")?,
            CaptureCause::Restore => {
                f.write_str("static runner signal dispositions could not be restored")?
            }
        }
        if let Some(signal) = self.cancellation {
            write!(f, "; cancellation: {signal}")?;
        }
        if let Some(problem) = &self.cleanup.problem {
            write!(f, "; cleanup: {problem}")?;
        }
        if let Some(error) = &self.restore_error {
            write!(f, "; signal restore: {error}")?;
        }
        if !self.stderr.is_empty() {
            let excerpt = &self.stderr[..self.stderr.len().min(4096)];
            write!(f, "; captured stderr: {}", String::from_utf8_lossy(excerpt))?;
        }
        Ok(())
    }
}

impl std::error::Error for CaptureError {}

pub(crate) struct Cleanup {
    pub(crate) group_signal_sent: bool,
    pub(crate) leader_reaped: bool,
    pub(crate) problem: Option<String>,
}

impl fmt::Debug for Cleanup {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Cleanup")
            .field("group_signal_sent", &self.group_signal_sent)
            .field("leader_reaped", &self.leader_reaped)
            .field("problem", &self.problem)
            .finish()
    }
}

impl Cleanup {
    fn not_spawned() -> Self {
        Self {
            group_signal_sent: false,
            leader_reaped: false,
            problem: None,
        }
    }
}

static SIGNAL_OWNER: AtomicBool = AtomicBool::new(false);
const SIGNAL_CLOSED: i32 = 0;
const SIGNAL_ACTIVE: i32 = 1 << 8;
const SIGNAL_POISONED: i32 = 1 << 9;
const EVENT_INTERRUPT: i32 = 1;
const EVENT_TERMINATE: i32 = 2;
const EVENT_MASK: i32 = 3;
static SIGNAL_STATE: AtomicI32 = AtomicI32::new(SIGNAL_CLOSED);

extern "C" fn record_cancellation(signal: libc::c_int) {
    let Some(cancellation) = Cancellation::from_raw(signal) else {
        return;
    };
    // SAFETY: Linux errno is thread-local, and this pointer remains valid
    // throughout the handler. Preserve it if a successful late relay returns.
    let errno = unsafe { libc::__errno_location() };
    let saved_errno = unsafe { *errno };
    #[cfg(test)]
    signal_probe::before_record();
    let event = match cancellation {
        Cancellation::Interrupt => EVENT_INTERRUPT,
        Cancellation::Terminate => EVENT_TERMINATE,
    };
    match SIGNAL_STATE.compare_exchange(
        SIGNAL_ACTIVE,
        SIGNAL_ACTIVE | event,
        Ordering::AcqRel,
        Ordering::Acquire,
    ) {
        Ok(_) => {}
        Err(state) if state & SIGNAL_ACTIVE != 0 => {
            // Another event already won. The active finalizer owns it.
        }
        Err(SIGNAL_CLOSED) => {
            #[cfg(test)]
            signal_probe::before_relay();
            // A callback dispatched on another thread can reach this point
            // after exact disposition restoration and atomic closure. Relay
            // through the current disposition instead of consuming it. A
            // later capture may own the relay and must be allowed to clean up.
            // SAFETY: getpid, kill and _exit are async-signal-safe. Setup
            // refuses PID 1, whose default dispositions need different policy.
            let pid = unsafe { libc::getpid() };
            if pid == 1 || unsafe { libc::kill(pid, signal) } != 0 {
                unsafe { libc::_exit(128 + signal) };
            }
        }
        Err(_) => {
            // Restoration failed. Do not recursively signal a handler which
            // may still be installed. No later owner may reset this poison.
            // SAFETY: _exit is async-signal-safe and does not unwind.
            unsafe { libc::_exit(128 + signal) };
        }
    }
    // SAFETY: restore the interrupted thread's errno before returning.
    unsafe { *errno = saved_errno };
}

fn cancellation_from_state(state: i32) -> Option<Cancellation> {
    match state & EVENT_MASK {
        EVENT_INTERRUPT => Some(Cancellation::Interrupt),
        EVENT_TERMINATE => Some(Cancellation::Terminate),
        _ => None,
    }
}

fn cancellation() -> Option<Cancellation> {
    cancellation_from_state(SIGNAL_STATE.load(Ordering::Acquire))
}

/// This private CLI boundary serializes signal ownership, not process work.
/// Contention fails before spawning instead of waiting behind a long capture.
struct SignalGuard {
    installed: Vec<(i32, libc::sigaction)>,
    active: bool,
}

impl SignalGuard {
    fn acquire() -> io::Result<Self> {
        if SIGNAL_OWNER
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            return Err(io::Error::other(
                "another static runner owns signal handling",
            ));
        }
        if SIGNAL_STATE
            .compare_exchange(
                SIGNAL_CLOSED,
                SIGNAL_ACTIVE,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_err()
        {
            SIGNAL_OWNER.store(false, Ordering::Release);
            return Err(io::Error::other(
                "static runner signal ownership is poisoned after failed restoration",
            ));
        }
        Ok(Self {
            installed: Vec::with_capacity(2),
            active: true,
        })
    }

    fn install(&mut self) -> io::Result<()> {
        // WNOWAIT custody requires a waitable leader and no competing reaper.
        // The CLI parse path has no other child reaper. Refuse an inherited
        // auto-reap/custom SIGCHLD disposition before starting any child.
        let child_action = read_action(libc::SIGCHLD)?;
        if child_action.sa_sigaction != libc::SIG_DFL
            || child_action.sa_flags & libc::SA_NOCLDWAIT != 0
        {
            return Err(io::Error::other(
                "SIGCHLD does not preserve exclusive child wait custody",
            ));
        }

        let previous = [
            (libc::SIGINT, read_action(libc::SIGINT)?),
            (libc::SIGTERM, read_action(libc::SIGTERM)?),
        ];
        for (signal, action) in &previous {
            if action.sa_sigaction != libc::SIG_DFL && action.sa_sigaction != libc::SIG_IGN {
                return Err(io::Error::other(format!(
                    "signal {signal} already has a custom handler"
                )));
            }
        }
        for (signal, previous) in previous {
            if previous.sa_sigaction == libc::SIG_IGN {
                continue;
            }
            #[cfg(test)]
            if signal == libc::SIGTERM {
                signal_probe::before_second_install()?;
            }
            // SAFETY: zero initialization is valid for Linux sigaction and
            // sigemptyset initializes its mask before sigaction reads it.
            let mut action: libc::sigaction = unsafe { std::mem::zeroed() };
            action.sa_sigaction = record_cancellation as *const () as libc::sighandler_t;
            // SAFETY: action owns a valid, writable sigset_t.
            if unsafe { libc::sigemptyset(&mut action.sa_mask) } != 0 {
                return Err(io::Error::last_os_error());
            }
            // SAFETY: both pointers refer to initialized actions for this call;
            // the static handler records or relays through the reviewed
            // atomic close protocol without allocation or locking.
            if unsafe { libc::sigaction(signal, &action, std::ptr::null_mut()) } != 0 {
                return Err(io::Error::last_os_error());
            }
            self.installed.push((signal, previous));
        }
        Ok(())
    }

    fn restore(&mut self) -> Option<io::Error> {
        let mut failure = None;
        for (signal, action) in self.installed.drain(..).rev() {
            if let Err(error) = restore_action(signal, &action)
                && failure.is_none()
            {
                failure = Some(error);
            }
        }
        failure
    }

    fn close(&mut self) -> (Option<Cancellation>, Option<io::Error>) {
        let failure = self.restore();
        // This single transition owns every previously dispatched callback:
        // its CAS either precedes closure and is returned here, or observes a
        // closed state and relays. A plain load after restore is insufficient
        // when the callback runs on another thread.
        let previous = SIGNAL_STATE.swap(
            if failure.is_some() {
                SIGNAL_POISONED
            } else {
                SIGNAL_CLOSED
            },
            Ordering::AcqRel,
        );
        let signal = cancellation_from_state(previous);
        self.active = false;
        SIGNAL_OWNER.store(false, Ordering::Release);
        #[cfg(test)]
        signal_probe::FINISH_COUNT.fetch_add(1, Ordering::Release);
        (signal, failure)
    }

    fn finish(mut self) -> (Option<Cancellation>, Option<io::Error>) {
        self.close()
    }
}

impl Drop for SignalGuard {
    fn drop(&mut self) {
        if self.active {
            let (signal, _) = self.close();
            if let Some(signal) = signal {
                // OwnedChild's finite cleanup has already unwound. Reuse the
                // closed-state relay rather than discarding this event. A
                // poisoned state takes the non-recursive exit fallback.
                record_cancellation(signal.raw());
            }
        }
    }
}

fn restore_action(signal: i32, action: &libc::sigaction) -> io::Result<()> {
    #[cfg(test)]
    if signal_probe::FAIL_RESTORE
        .compare_exchange(signal, 0, Ordering::AcqRel, Ordering::Acquire)
        .is_ok()
    {
        // Deliberate fixture-only failure leaves the handler installed so the
        // poisoned late-callback path is exercised, not merely a fake error.
        return Err(io::Error::from_raw_os_error(libc::EINVAL));
    }
    // SAFETY: restore the exact initialized action saved for this signal.
    // The singleton remains held through restoration.
    if unsafe { libc::sigaction(signal, action, std::ptr::null_mut()) } != 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

fn read_action(signal: i32) -> io::Result<libc::sigaction> {
    // SAFETY: sigaction initializes the pointed-to object; a null action only
    // reads the current disposition and does not install one.
    let mut action: libc::sigaction = unsafe { std::mem::zeroed() };
    if unsafe { libc::sigaction(signal, std::ptr::null(), &mut action) } != 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(action)
    }
}

/// Call only after the CLI command stack has unwound its temporary resources.
/// A failed restoration cannot turn cancellation into a swallowed signal: the
/// conventional 128+signal exit remains a fallback if self-signalling returns.
pub(crate) fn exit_after_cancellation(signal: Cancellation) -> ! {
    let _ = kill_process(getpid(), signal.signal());
    std::process::exit(128 + signal.raw());
}

struct OwnedChild {
    child: Child,
    pid: Pid,
    natural_status: Option<ExitStatus>,
    wait_custody: bool,
    cleanup_attempted: bool,
    cleanup_timeout: Duration,
}

impl OwnedChild {
    fn new(child: Child, cleanup_timeout: Duration) -> Self {
        Self {
            pid: Pid::from_child(&child),
            child,
            natural_status: None,
            wait_custody: true,
            cleanup_attempted: false,
            cleanup_timeout,
        }
    }

    fn observe(&mut self) -> io::Result<Option<ExitStatus>> {
        let flags = WaitIdOptions::EXITED | WaitIdOptions::NOHANG | WaitIdOptions::NOWAIT;
        match waitid(WaitId::Pid(self.pid), flags) {
            Ok(Some(status)) => {
                let raw = if let Some(code) = status.exit_status() {
                    code << 8
                } else if let Some(signal) = status.terminating_signal() {
                    signal | if status.dumped() { 0x80 } else { 0 }
                } else {
                    return Err(io::Error::other("waitid returned a nonterminal status"));
                };
                let status = ExitStatus::from_raw(raw);
                self.natural_status = Some(status);
                Ok(Some(status))
            }
            Ok(None) => Ok(None),
            Err(rustix::io::Errno::INTR) => Ok(None),
            Err(error) => {
                if error == rustix::io::Errno::CHILD {
                    // A recycled numeric PID/group must never be signalled.
                    self.wait_custody = false;
                }
                Err(error.into())
            }
        }
    }

    fn cleanup(&mut self) -> (Cleanup, Option<ExitStatus>) {
        self.cleanup_attempted = true;
        let mut report = Cleanup::not_spawned();
        if !self.wait_custody || self.pid.is_init() {
            report.problem = Some(
                "exclusive leader wait custody was lost; no numeric group was signalled".into(),
            );
            return (report, None);
        }
        // No wait/try_wait has reaped the leader. Even a leader that exited
        // first retains the process-group identity until this group signal.
        match kill_process_group(self.pid, Signal::KILL) {
            Ok(()) => report.group_signal_sent = true,
            Err(rustix::io::Errno::SRCH) => {
                // An empty group needs no signal. Still request termination of
                // the owned, unreaped leader in case it left its own group.
                if self.natural_status.is_none()
                    && let Err(error) = self.child.kill()
                {
                    report.problem = Some(format!("group absent; leader termination: {error}"));
                }
            }
            Err(error) => {
                report.problem = Some(format!("owned group termination failed: {error}"));
                let _ = self.child.kill();
            }
        }
        let Some(deadline) = Instant::now().checked_add(self.cleanup_timeout) else {
            report.problem = Some("cleanup deadline overflow".into());
            return (report, None);
        };
        loop {
            // Reaping occurs only after the final group signal; never signal
            // this numeric group again after try_wait has released its leader.
            match self.child.try_wait() {
                Ok(Some(status)) => {
                    report.leader_reaped = true;
                    return (report, Some(status));
                }
                Ok(None) => {}
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                Err(error) => {
                    report.problem = Some(format!("leader reap failed: {error}"));
                    return (report, None);
                }
            }
            if Instant::now() >= deadline {
                report.problem =
                    Some("leader reap was not confirmed within the cleanup grace".into());
                return (report, None);
            }
            pause_until(deadline);
        }
    }
}

impl Drop for OwnedChild {
    fn drop(&mut self) {
        if !self.cleanup_attempted {
            // Emergency setup/unwind path uses the same finite cleanup; there
            // is no blocking wait, pipe read or reader-thread join in Drop.
            let _ = self.cleanup();
        }
    }
}

struct Streams {
    stdout: Option<ChildStdout>,
    stderr: Option<ChildStderr>,
    out: Vec<u8>,
    err: Vec<u8>,
}

impl Streams {
    fn new(child: &mut Child) -> Self {
        Self {
            stdout: child.stdout.take(),
            stderr: child.stderr.take(),
            out: Vec::new(),
            err: Vec::new(),
        }
    }

    fn prepare(&self) -> io::Result<()> {
        nonblocking(
            self.stdout
                .as_ref()
                .ok_or_else(|| io::Error::other("missing stdout pipe"))?,
        )?;
        nonblocking(
            self.stderr
                .as_ref()
                .ok_or_else(|| io::Error::other("missing stderr pipe"))?,
        )
    }

    fn close(&mut self) {
        self.stdout = None;
        self.stderr = None;
    }
}

fn nonblocking(pipe: &impl AsFd) -> io::Result<()> {
    let flags = fcntl_getfl(pipe)?;
    fcntl_setfl(pipe, flags | OFlags::NONBLOCK)?;
    Ok(())
}

fn read_turn<R: Read>(
    pipe: &mut Option<R>,
    bytes: &mut Vec<u8>,
    limit: usize,
    channel: Channel,
    deadline: Instant,
) -> Result<bool, CaptureCause> {
    let mut progress = false;
    let mut read_this_turn = 0;
    let mut buffer = [0; READ_CHUNK];
    while let Some(reader) = pipe.as_mut() {
        if cancellation().is_some() {
            return Err(CaptureCause::Cancelled);
        }
        if Instant::now() >= deadline {
            return Err(CaptureCause::Deadline);
        }
        let remaining = limit.saturating_sub(bytes.len());
        let width = buffer
            .len()
            .min(remaining.saturating_add(1))
            .min(READ_TURN - read_this_turn);
        match reader.read(&mut buffer[..width]) {
            Ok(0) => {
                *pipe = None;
                return Ok(true);
            }
            Ok(count) => {
                let keep = count.min(remaining);
                let wanted = bytes.len() + keep;
                if wanted > bytes.capacity() {
                    let capacity = bytes
                        .capacity()
                        .max(READ_CHUNK)
                        .saturating_mul(2)
                        .min(limit)
                        .max(wanted);
                    bytes
                        .try_reserve_exact(capacity - bytes.len())
                        .map_err(|error| {
                            CaptureCause::Read(channel, io::Error::other(error.to_string()))
                        })?;
                }
                bytes.extend_from_slice(&buffer[..keep]);
                if count > remaining {
                    return Err(CaptureCause::Limit(channel, limit));
                }
                progress = true;
                read_this_turn += count;
                if read_this_turn >= READ_TURN {
                    break;
                }
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(CaptureCause::Read(channel, error)),
        }
    }
    Ok(progress)
}

fn pause_until(deadline: Instant) {
    std::thread::sleep(POLL_INTERVAL.min(deadline.saturating_duration_since(Instant::now())));
}

fn pump(
    child: &mut OwnedChild,
    streams: &mut Streams,
    policy: CapturePolicy,
    deadline: Instant,
) -> Option<CaptureCause> {
    let mut drain_deadline = None;
    loop {
        if cancellation().is_some() {
            return Some(CaptureCause::Cancelled);
        }
        match child.observe() {
            Ok(Some(_)) if drain_deadline.is_none() => {
                drain_deadline = Instant::now().checked_add(policy.drain_timeout);
            }
            Ok(_) => {}
            Err(error) => return Some(CaptureCause::Observe(error)),
        }
        let now = Instant::now();
        if now >= deadline {
            return Some(CaptureCause::Deadline);
        }
        if drain_deadline.is_some_and(|drain| now >= drain) {
            return Some(CaptureCause::DrainDeadline);
        }
        let read_deadline = drain_deadline.map_or(deadline, |drain| drain.min(deadline));
        let out = read_turn(
            &mut streams.stdout,
            &mut streams.out,
            policy.stdout_bytes,
            Channel::Stdout,
            read_deadline,
        );
        let err = read_turn(
            &mut streams.stderr,
            &mut streams.err,
            policy.stderr_bytes,
            Channel::Stderr,
            read_deadline,
        );
        let (out, err) = match (out, err) {
            (Ok(out), Ok(err)) => (out, err),
            (Err(CaptureCause::Deadline), _) | (_, Err(CaptureCause::Deadline))
                if drain_deadline
                    .is_some_and(|drain| drain < deadline && Instant::now() >= drain) =>
            {
                return Some(CaptureCause::DrainDeadline);
            }
            (Err(cause), _) | (_, Err(cause)) => return Some(cause),
        };
        if child.natural_status.is_some() && streams.stdout.is_none() && streams.stderr.is_none() {
            return None;
        }
        if !out && !err {
            pause_until(read_deadline);
        }
    }
}

fn capture_owned(
    command: &mut Command,
    policy: CapturePolicy,
    deadline: Instant,
) -> Result<(Output, Cleanup), CaptureError> {
    if cancellation().is_some() {
        return Err(CaptureError::before_spawn(CaptureCause::Cancelled));
    }
    if Instant::now() >= deadline {
        return Err(CaptureError::before_spawn(CaptureCause::Deadline));
    }
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0);
    let child = command
        .spawn()
        .map_err(|error| CaptureError::before_spawn(CaptureCause::Spawn(error)))?;
    // Take ownership immediately, before any fallible post-spawn setup.
    let mut child = OwnedChild::new(child, policy.cleanup_timeout);
    let mut streams = Streams::new(&mut child.child);
    let mut cause = match streams.prepare() {
        Ok(()) => pump(&mut child, &mut streams, policy, deadline),
        Err(error) => Some(CaptureCause::Setup(error.to_string())),
    };
    // Preserve an exit which was already real BEFORE any pipe close or kill.
    if let Err(error) = child.observe()
        && cause.is_none()
    {
        cause = Some(CaptureCause::Observe(error));
    }
    let (cleanup, status) = child.cleanup();
    streams.close();
    if cleanup.problem.is_some() || !cleanup.leader_reaped {
        cause.get_or_insert(CaptureCause::Cleanup);
    }
    match (cause, status) {
        (None, Some(status)) => Ok((
            Output {
                status,
                stdout: streams.out,
                stderr: streams.err,
            },
            cleanup,
        )),
        (cause, _) => Err(CaptureError {
            cause: cause.unwrap_or(CaptureCause::Cleanup),
            natural_status: child.natural_status,
            stdout: streams.out,
            stderr: streams.err,
            cleanup: Box::new(cleanup),
            cancellation: None,
            restore_error: None,
        }),
    }
}

/// Capture one command while preserving its argv, environment and cwd.
/// Stdin is closed, matching Command::output. Only this boundary's pipes and
/// process group are changed. Policies may narrow, not enlarge, capture caps.
pub(crate) fn capture(
    command: &mut Command,
    policy: CapturePolicy,
) -> Result<Output, CaptureError> {
    if getpid().is_init() {
        return Err(CaptureError::before_spawn(
            CaptureCause::UnsupportedInitProcess,
        ));
    }
    let now = Instant::now();
    let deadline = now.checked_add(policy.total_timeout);
    if policy.stdout_bytes > MAX_STDOUT
        || policy.stderr_bytes > MAX_STDERR
        || policy.total_timeout.is_zero()
        || policy.drain_timeout.is_zero()
        || policy.cleanup_timeout.is_zero()
        || deadline.is_none()
        || now.checked_add(policy.drain_timeout).is_none()
        || now.checked_add(policy.cleanup_timeout).is_none()
    {
        return Err(CaptureError::before_spawn(CaptureCause::Setup(
            "invalid finite capture policy".into(),
        )));
    }
    let mut guard = SignalGuard::acquire()
        .map_err(|error| CaptureError::before_spawn(CaptureCause::Setup(error.to_string())))?;
    // Even partial installation/setup failure reaches explicit restoration and
    // final cancellation sampling; Drop is only an emergency unwind fallback.
    let result = match guard.install() {
        Ok(()) => capture_owned(command, policy, deadline.unwrap_or(now)),
        Err(error) => Err(CaptureError::before_spawn(CaptureCause::Setup(
            error.to_string(),
        ))),
    };
    let (signal, restore_error) = guard.finish();
    match result {
        Ok((output, _)) if signal.is_none() && restore_error.is_none() => Ok(output),
        Ok((output, cleanup)) => Err(CaptureError {
            cause: if signal.is_some() {
                CaptureCause::Cancelled
            } else {
                CaptureCause::Restore
            },
            natural_status: Some(output.status),
            stdout: output.stdout,
            stderr: output.stderr,
            cleanup: Box::new(cleanup),
            cancellation: signal,
            restore_error,
        }),
        Err(mut error) => {
            error.cancellation = signal;
            error.restore_error = restore_error;
            Err(error)
        }
    }
}

/// Only compiled into test harnesses. Every operation used by the actual
/// signal callback is atomic or async-signal-safe; no mutex/condvar rendezvous.
#[cfg(test)]
pub(crate) mod signal_probe {
    use std::io;
    use std::sync::atomic::{AtomicBool, AtomicI32, AtomicUsize, Ordering};
    use std::time::{Duration, Instant};

    pub(crate) static PAUSE_BEFORE_RECORD: AtomicBool = AtomicBool::new(false);
    pub(crate) static ENTERED: AtomicBool = AtomicBool::new(false);
    pub(crate) static RELEASE: AtomicBool = AtomicBool::new(false);
    pub(crate) static PAUSE_BEFORE_RELAY: AtomicBool = AtomicBool::new(false);
    pub(crate) static RELAY_ENTERED: AtomicBool = AtomicBool::new(false);
    pub(crate) static RELAY_RELEASE: AtomicBool = AtomicBool::new(false);
    pub(crate) static FINISH_COUNT: AtomicUsize = AtomicUsize::new(0);
    pub(crate) static FAIL_RESTORE: AtomicI32 = AtomicI32::new(0);
    pub(crate) static PAUSE_INSTALL: AtomicBool = AtomicBool::new(false);
    pub(crate) static INSTALL_ENTERED: AtomicBool = AtomicBool::new(false);
    pub(crate) static INSTALL_RELEASE: AtomicBool = AtomicBool::new(false);
    pub(crate) static FAIL_INSTALL: AtomicBool = AtomicBool::new(false);
    pub(crate) static PANIC_INSTALL: AtomicBool = AtomicBool::new(false);

    pub(super) fn before_second_install() -> io::Result<()> {
        if !PAUSE_INSTALL.swap(false, Ordering::AcqRel) {
            return Ok(());
        }
        INSTALL_ENTERED.store(true, Ordering::Release);
        let until = Instant::now() + Duration::from_secs(4);
        while !INSTALL_RELEASE.load(Ordering::Acquire) {
            assert!(
                Instant::now() < until,
                "signal fixture install rendezvous expired"
            );
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(
            !PANIC_INSTALL.swap(false, Ordering::AcqRel),
            "deliberate signal fixture partial-install panic"
        );
        if FAIL_INSTALL.swap(false, Ordering::AcqRel) {
            return Err(io::Error::other(
                "deliberate signal fixture partial-install failure",
            ));
        }
        Ok(())
    }

    pub(super) fn before_record() {
        pause(&PAUSE_BEFORE_RECORD, &ENTERED, &RELEASE);
    }

    pub(super) fn before_relay() {
        pause(&PAUSE_BEFORE_RELAY, &RELAY_ENTERED, &RELAY_RELEASE);
    }

    fn pause(armed: &AtomicBool, entered: &AtomicBool, release: &AtomicBool) {
        if !armed.swap(false, Ordering::AcqRel) {
            return;
        }
        // SAFETY: clock_gettime initializes this valid timespec, and is
        // async-signal-safe. Failure has an unmistakable fixture-only status.
        let mut start: libc::timespec = unsafe { std::mem::zeroed() };
        if unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut start) } != 0 {
            unsafe { libc::_exit(220) };
        }
        entered.store(true, Ordering::Release);
        while !release.load(Ordering::Acquire) {
            let mut now: libc::timespec = unsafe { std::mem::zeroed() };
            if unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut now) } != 0
                || now.tv_sec.saturating_sub(start.tv_sec) >= 5
            {
                // An outer controller also has a finite process watchdog.
                unsafe { libc::_exit(221) };
            }
            std::hint::spin_loop();
        }
    }
}
