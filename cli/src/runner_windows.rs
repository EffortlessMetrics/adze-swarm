//! Windows custody for one explicit static-runner request (#1066).
//!
//! Native creation assigns a private kill-on-close job before the trusted
//! same-executable bootstrap runs. The bootstrap uses stable Command for the
//! external invocation; this module owns capture, deadlines and job cleanup.
//! It does not interpret document JSON, parser outcomes or cache admission.

use crate::runner_windows_handles::{BOOTSTRAP_MODE, FORCED_EXIT, OwnedChild, Pipe, ReadResult, decode_inherited_request, prepare_bootstrap_stdio};
use crate::runner_windows_request::RunnerRequest;
use std::ffi::c_void;
use std::fmt;
use std::io;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::os::windows::process::ExitStatusExt;
use std::path::Path;
use std::process::{ExitStatus, Output};
use std::ptr;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicPtr, AtomicU32, Ordering};
use std::time::{Duration, Instant};
use windows_sys::Win32::Foundation::{WAIT_OBJECT_0, WAIT_TIMEOUT};
use windows_sys::Win32::System::Console::{CTRL_BREAK_EVENT, CTRL_C_EVENT, SetConsoleCtrlHandler};
use windows_sys::Win32::System::Threading::{CreateEventW, ResetEvent, SetEvent, WaitForSingleObject};

const READ_CHUNK: usize = 8 * 1024;
const READ_TURN: usize = 64 * 1024;
const POLL_INTERVAL: Duration = Duration::from_millis(10);

pub(crate) const CARGO_RUN_POLICY: CapturePolicy = CapturePolicy {
    stdout_bytes: 64 * 1024 * 1024,
    stderr_bytes: 1024 * 1024,
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
pub(crate) enum Channel { Stdout, Stderr }

impl fmt::Display for Channel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self { Self::Stdout => "stdout", Self::Stderr => "stderr" })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Cancellation { Interrupt, Break }

impl Cancellation {
    fn from_encoded(value: u32) -> Option<Self> {
        match value & EVENT_MASK { 1 => Some(Self::Interrupt), 2 => Some(Self::Break), _ => None }
    }

    pub(crate) fn exit_code(self) -> i32 { FORCED_EXIT as i32 }
}

impl fmt::Display for Cancellation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self { Self::Interrupt => "Ctrl-C", Self::Break => "Ctrl-Break" })
    }
}

#[derive(Debug)]
pub(crate) enum CaptureCause {
    Setup(&'static str, io::Error),
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

#[derive(Debug, Default)]
pub(crate) struct Cleanup {
    pub(crate) job_terminated: bool,
    pub(crate) leader_closed: bool,
    pub(crate) active_processes: Option<u32>,
    pub(crate) problem: Option<String>,
}

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
        Self { cause, natural_status: None, stdout: Vec::new(), stderr: Vec::new(),
            cleanup: Box::default(), cancellation: None, restore_error: None }
    }

    pub(crate) fn cancellation(&self) -> Option<Cancellation> { self.cancellation }
}

impl fmt::Debug for CaptureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CaptureError").field("cause", &self.cause)
            .field("natural_status", &self.natural_status)
            .field("stdout_bytes", &self.stdout.len()).field("stderr_bytes", &self.stderr.len())
            .field("cleanup", &self.cleanup).field("cancellation", &self.cancellation)
            .field("restore_error", &self.restore_error).finish()
    }
}

impl fmt::Display for CaptureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(status) = self.natural_status.filter(|status| !status.success()) {
            write!(f, "static runner exited with {status}; ")?;
        }
        match &self.cause {
            CaptureCause::Setup(stage, error) => write!(f, "static runner {stage} setup failed: {error}")?,
            CaptureCause::Spawn(error) => write!(f, "static runner could not start in its Windows job: {error}")?,
            CaptureCause::Read(channel, error) => write!(f, "static runner {channel} read failed: {error}")?,
            CaptureCause::Observe(error) => write!(f, "static runner observation failed: {error}")?,
            CaptureCause::Limit(channel, limit) => write!(f, "static runner {channel} exceeded its {limit}-byte capture limit")?,
            CaptureCause::Deadline => f.write_str("static runner exceeded its absolute process deadline")?,
            CaptureCause::DrainDeadline => f.write_str("static runner pipe drain exceeded its post-exit deadline")?,
            CaptureCause::Cancelled => f.write_str("static runner was cancelled")?,
            CaptureCause::Cleanup => f.write_str("static runner Windows job cleanup could not be confirmed")?,
            CaptureCause::Restore => f.write_str("static runner console handler could not be removed")?,
        }
        if let Some(event) = self.cancellation { write!(f, "; cancellation: {event}")?; }
        if let Some(problem) = &self.cleanup.problem { write!(f, "; cleanup: {problem}")?; }
        if let Some(error) = &self.restore_error { write!(f, "; handler removal: {error}")?; }
        if !self.stderr.is_empty() {
            write!(f, "; captured stderr: {}", String::from_utf8_lossy(&self.stderr[..self.stderr.len().min(4096)]))?;
        }
        Ok(())
    }
}

impl std::error::Error for CaptureError {}

const ACTIVE: u32 = 1 << 31;
const EVENT_MASK: u32 = 3;
static CONSOLE_OWNER: AtomicBool = AtomicBool::new(false);
static CONSOLE_STATE: AtomicU32 = AtomicU32::new(0);
// The event has process lifetime: unregistering a callback does not join an
// already-dispatched OS handler thread. Never close a handle it may still use.
static WAKE_EVENT: OnceLock<Result<OwnedHandle, i32>> = OnceLock::new();
static WAKE_HANDLE: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());

unsafe extern "system" fn console_handler(event: u32) -> i32 {
    let encoded = match event { CTRL_C_EVENT => 1, CTRL_BREAK_EVENT => 2, _ => return 0 };
    let mut state = CONSOLE_STATE.load(Ordering::Acquire);
    loop {
        if state & ACTIVE == 0 { return 0; }
        let next = if state & EVENT_MASK == 0 { state | encoded } else { state };
        match CONSOLE_STATE.compare_exchange_weak(state, next, Ordering::AcqRel, Ordering::Acquire) {
            Ok(_) => {
                // SAFETY: the event was published before ACTIVE and is never
                // closed during process lifetime. Failure only loses a wake;
                // the parent also polls the atomic state at bounded cadence.
                unsafe { SetEvent(WAKE_HANDLE.load(Ordering::Acquire)) };
                return 1;
            }
            Err(actual) => state = actual,
        }
    }
}

fn cancellation() -> Option<Cancellation> {
    Cancellation::from_encoded(CONSOLE_STATE.load(Ordering::Acquire))
}

struct ConsoleGuard { installed: bool }

impl ConsoleGuard {
    fn acquire() -> io::Result<Self> {
        if CONSOLE_OWNER.compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire).is_err() {
            return Err(io::Error::other("another runner owns console cancellation"));
        }
        let event = WAKE_EVENT.get_or_init(|| {
            // SAFETY: unnamed, non-inheritable manual-reset event. It remains
            // owned by the process-lifetime OnceLock for callback safety.
            let raw = unsafe { CreateEventW(ptr::null(), 1, 0, ptr::null()) };
            if raw.is_null() { return Err(io::Error::last_os_error().raw_os_error().unwrap_or(31)); }
            // SAFETY: CreateEventW returned a new valid, non-pseudo handle.
            Ok(unsafe { OwnedHandle::from_raw_handle(raw) })
        });
        let event = match event {
            Ok(event) => event,
            Err(code) => {
                CONSOLE_OWNER.store(false, Ordering::Release);
                return Err(io::Error::from_raw_os_error(*code));
            }
        };
        // SAFETY: the event is valid for the process lifetime and no other
        // capture may reset it while this guard owns cancellation.
        if unsafe { ResetEvent(event.as_raw_handle()) } == 0 {
            CONSOLE_OWNER.store(false, Ordering::Release);
            return Err(io::Error::last_os_error());
        }
        WAKE_HANDLE.store(event.as_raw_handle(), Ordering::Release);
        CONSOLE_STATE.store(ACTIVE, Ordering::Release);
        // SAFETY: callback has system ABI and static lifetime. A non-null
        // handler does not toggle the inherited ignore-Ctrl-C disposition.
        if unsafe { SetConsoleCtrlHandler(Some(console_handler), 1) } == 0 {
            CONSOLE_STATE.fetch_and(!ACTIVE, Ordering::AcqRel);
            CONSOLE_OWNER.store(false, Ordering::Release);
            return Err(io::Error::last_os_error());
        }
        Ok(Self { installed: true })
    }

    fn finish(&mut self) -> (Option<Cancellation>, Option<io::Error>) {
        // This is the cancellation linearization point, not a later load.
        // A dispatched callback either wins before closure or returns FALSE
        // to the existing/default chain after losing its ACTIVE-state CAS.
        let recorded = CONSOLE_STATE.fetch_and(!ACTIVE, Ordering::AcqRel);
        let error = if self.installed {
            // SAFETY: remove only this static callback, not any other handler.
            if unsafe { SetConsoleCtrlHandler(Some(console_handler), 0) } == 0 {
                Some(io::Error::last_os_error())
            } else { None }
        } else { None };
        self.installed = false;
        CONSOLE_OWNER.store(false, Ordering::Release);
        (Cancellation::from_encoded(recorded), error)
    }
}

impl Drop for ConsoleGuard {
    fn drop(&mut self) {
        if self.installed { let _ = self.finish(); }
    }
}

#[derive(Clone, Copy)]
enum DeadlineKind { Total, Drain }

fn check(deadline: Instant, kind: DeadlineKind) -> Result<(), CaptureCause> {
    if cancellation().is_some() { return Err(CaptureCause::Cancelled); }
    if Instant::now() >= deadline {
        return Err(match kind { DeadlineKind::Total => CaptureCause::Deadline, DeadlineKind::Drain => CaptureCause::DrainDeadline });
    }
    Ok(())
}

fn read_turn(pipe: &Pipe, output: &mut Vec<u8>, closed: &mut bool, channel: Channel,
    limit: usize, deadline: Instant, kind: DeadlineKind) -> Result<(), CaptureCause> {
    if *closed { return Ok(()); }
    let mut buffer = [0; READ_CHUNK];
    let mut consumed = 0;
    while consumed < READ_TURN {
        check(deadline, kind)?;
        let remaining = limit.saturating_sub(output.len());
        let length = remaining.saturating_add(1).min(READ_CHUNK);
        match pipe.read(&mut buffer[..length]).map_err(|error| CaptureCause::Read(channel, error))? {
            ReadResult::Bytes(count) => {
                output.extend_from_slice(&buffer[..count.min(remaining)]);
                if count > remaining { return Err(CaptureCause::Limit(channel, limit)); }
                consumed += count;
            }
            ReadResult::Idle => return Ok(()),
            ReadResult::Closed => { *closed = true; return Ok(()); }
        }
    }
    Ok(())
}

fn idle_until(deadline: Instant) -> Result<(), CaptureCause> {
    let duration = deadline.saturating_duration_since(Instant::now()).min(POLL_INTERVAL);
    // SAFETY: the static event remains valid. The finite wait never hides a
    // wait for pipe completion or a child; handler wake is only an optimization.
    match unsafe { WaitForSingleObject(WAKE_HANDLE.load(Ordering::Acquire), duration.as_millis() as u32) } {
        WAIT_OBJECT_0 | WAIT_TIMEOUT => Ok(()),
        _ => Err(CaptureCause::Observe(io::Error::last_os_error())),
    }
}

fn cleanup(child: &mut OwnedChild, timeout: Duration) -> Cleanup {
    let mut report = Cleanup::default();
    match child.terminate() {
        Ok(()) => report.job_terminated = true,
        Err(error) => report.problem = Some(format!("TerminateJobObject failed: {error}")),
    }
    let deadline = Instant::now().checked_add(timeout).unwrap_or_else(Instant::now);
    loop {
        if !child.leader_closed() && let Err(error) = child.status() {
            report.problem.get_or_insert_with(|| format!("leader observation failed: {error}"));
            break;
        }
        report.leader_closed = child.leader_closed();
        // Close the signaled leader handle before final job accounting. There
        // is no numeric-PID identity/reap requirement on Windows.
        if report.leader_closed {
            match child.active_processes() {
                Ok(count) => {
                    report.active_processes = Some(count);
                    if count == 0 { break; }
                }
                Err(error) => {
                    report.problem.get_or_insert_with(|| format!("job accounting failed: {error}"));
                    break;
                }
            }
        }
        if Instant::now() >= deadline {
            report.problem.get_or_insert_with(|| "owned job was not confirmed empty within cleanup grace".to_owned());
            break;
        }
        std::thread::sleep(deadline.saturating_duration_since(Instant::now()).min(POLL_INTERVAL));
    }
    report
}

struct Captured { output: Output, cleanup: Box<Cleanup> }

fn capture_owned(request: &RunnerRequest, bootstrap: &Path, policy: CapturePolicy) -> Result<Captured, CaptureError> {
    let deadline = Instant::now().checked_add(policy.total_timeout).ok_or_else(||
        CaptureError::before_spawn(CaptureCause::Setup("deadline", io::Error::other("deadline overflow"))))?;
    let bytes = request.encode().map_err(|error| CaptureError::before_spawn(CaptureCause::Setup("request", error)))?;
    check(deadline, DeadlineKind::Total).map_err(CaptureError::before_spawn)?;
    let mut child = OwnedChild::spawn(bootstrap, &bytes, || {
        if cancellation().is_some() {
            Err(io::Error::new(io::ErrorKind::Interrupted, "runner cancelled before creation"))
        } else if Instant::now() >= deadline {
            Err(io::Error::new(io::ErrorKind::TimedOut, "runner deadline expired before creation"))
        } else { Ok(()) }
    }).map_err(|error| CaptureError::before_spawn(if cancellation().is_some() {
        CaptureCause::Cancelled
    } else if Instant::now() >= deadline { CaptureCause::Deadline } else { CaptureCause::Spawn(error) }))?;
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let mut stdout_closed = false;
    let mut stderr_closed = false;
    let mut natural_status = None;
    let mut drain_deadline = None;
    let mut cause = None;
    loop {
        if natural_status.is_none() {
            match child.status() {
                Ok(Some(code)) => {
                    natural_status = Some(ExitStatus::from_raw(code));
                    drain_deadline = Some(Instant::now().checked_add(policy.drain_timeout).unwrap_or(deadline).min(deadline));
                }
                Ok(None) => {}
                Err(error) => { cause = Some(CaptureCause::Observe(error)); break; }
            }
        }
        let (until, kind) = match drain_deadline {
            Some(drain) if drain < deadline => (drain, DeadlineKind::Drain),
            _ => (deadline, DeadlineKind::Total),
        };
        if let Err(error) = check(until, kind) { cause = Some(error); break; }
        if natural_status.is_some() && stdout_closed && stderr_closed { break; }
        let previous = (stdout.len(), stderr.len(), stdout_closed, stderr_closed);
        if let Err(error) = read_turn(&child.stdout, &mut stdout, &mut stdout_closed, Channel::Stdout,
            policy.stdout_bytes, until, kind).and_then(|()| read_turn(&child.stderr, &mut stderr,
                &mut stderr_closed, Channel::Stderr, policy.stderr_bytes, until, kind)) {
            cause = Some(error); break;
        }
        if previous == (stdout.len(), stderr.len(), stdout_closed, stderr_closed)
            && let Err(error) = idle_until(until) { cause = Some(error); break; }
    }
    // Only status observed before our intervention can be called natural.
    if natural_status.is_none() {
        match child.status() {
            Ok(Some(code)) => natural_status = Some(ExitStatus::from_raw(code)),
            Ok(None) => {}
            Err(error) if cause.is_none() => cause = Some(CaptureCause::Observe(error)),
            Err(_) => {}
        }
    }
    let cleanup = Box::new(cleanup(&mut child, policy.cleanup_timeout));
    if cause.is_none() && (!cleanup.job_terminated || !cleanup.leader_closed
        || cleanup.active_processes != Some(0) || cleanup.problem.is_some()) {
        cause = Some(CaptureCause::Cleanup);
    }
    drop(child);
    if let Some(cause) = cause {
        return Err(CaptureError { cause, natural_status, stdout, stderr, cleanup,
            cancellation: None, restore_error: None });
    }
    Ok(Captured { output: Output { status: natural_status.expect("completed leader"), stdout, stderr }, cleanup })
}

pub(crate) fn capture(request: &RunnerRequest, bootstrap: &Path, policy: CapturePolicy) -> Result<Output, CaptureError> {
    let mut console = ConsoleGuard::acquire().map_err(|error|
        CaptureError::before_spawn(CaptureCause::Setup("console cancellation", error)))?;
    let result = capture_owned(request, bootstrap, policy);
    let (cancellation, restore_error) = console.finish();
    match result {
        Ok(captured) if cancellation.is_none() && restore_error.is_none() => Ok(captured.output),
        Ok(captured) => Err(CaptureError {
            cause: if cancellation.is_some() { CaptureCause::Cancelled } else { CaptureCause::Restore },
            natural_status: Some(captured.output.status), stdout: captured.output.stdout,
            stderr: captured.output.stderr, cleanup: captured.cleanup, cancellation, restore_error,
        }),
        Err(mut error) => {
            error.cancellation = cancellation;
            error.restore_error = restore_error;
            Err(error)
        }
    }
}

/// Private dispatch before normal CLI parsing. The native parent launches this
/// exact binary in its job; no child launch can bypass the explicit decoder.
pub(crate) fn bootstrap_if_requested() -> Option<io::Result<u32>> {
    let mut args = std::env::args_os().skip(1);
    if args.next().as_deref() != Some(std::ffi::OsStr::new(BOOTSTRAP_MODE)) { return None; }
    Some((|| {
        let handle = args.next().and_then(|arg| arg.to_str().and_then(|arg| arg.parse::<usize>().ok()))
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "missing bootstrap request handle"))?;
        if args.next().is_some() {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "unexpected bootstrap argument"));
        }
        let request = decode_inherited_request(handle).map_err(|error|
            io::Error::new(error.kind(), format!("request decode: {error}")))?;
        prepare_bootstrap_stdio().map_err(|error|
            io::Error::new(error.kind(), format!("stdio inheritance: {error}")))?;
        let mut child = request.command().spawn().map_err(|error|
            io::Error::new(error.kind(), format!("external spawn: {error}")))?;
        let status = child.wait().map_err(|error|
            io::Error::new(error.kind(), format!("external wait: {error}")))?;
        // Windows always supplies a u32 process code. Preserve its complete bit
        // pattern through the public signed ExitStatus API, including 259 and
        // high-bit codes; main passes it to process::exit without u8 narrowing.
        Ok(status.code().ok_or_else(|| io::Error::other("missing Windows child status"))? as u32)
    })())
}
