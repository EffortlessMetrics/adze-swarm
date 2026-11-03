//! Dependency-free native producer, compiled by the selected rustc in Windows tests.
//! All workers are finite; production custody must stop them before 30 seconds.
#![cfg(windows)]

use std::ffi::{OsStr, OsString, c_void};
use std::fs;
use std::io::{self, Read, Write};
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::ptr;
use std::time::{Duration, Instant};

type Handle = *mut c_void;
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[link(name = "kernel32")]
unsafe extern "system" {
    #[link_name = "IsProcessInJob"]
    fn is_process_in_job(process: Handle, job: Handle, result: *mut i32) -> i32;
    #[link_name = "GetCurrentProcess"]
    fn current_process() -> Handle;
    #[link_name = "GetStdHandle"]
    fn std_handle(kind: u32) -> Handle;
    #[link_name = "SetStdHandle"]
    fn set_std_handle(kind: u32, handle: Handle) -> i32;
    #[link_name = "CloseHandle"]
    fn close_handle(handle: Handle) -> i32;
    #[link_name = "WriteFile"]
    fn write_file(
        handle: Handle,
        bytes: *const c_void,
        length: u32,
        written: *mut u32,
        overlapped: *mut c_void,
    ) -> i32;
    #[link_name = "SetConsoleCtrlHandler"]
    fn set_console_handler(handler: Option<unsafe extern "system" fn(u32) -> i32>, add: i32)
    -> i32;
    #[link_name = "GenerateConsoleCtrlEvent"]
    fn generate_console_event(event: u32, group: u32) -> i32;
    #[link_name = "FreeConsole"]
    fn free_console() -> i32;
    #[link_name = "AllocConsole"]
    fn alloc_console() -> i32;
    #[link_name = "OpenProcess"]
    fn open_process(access: u32, inherit: i32, pid: u32) -> Handle;
    #[link_name = "WaitForSingleObject"]
    fn wait_object(handle: Handle, milliseconds: u32) -> u32;
    #[link_name = "OpenEventW"]
    fn open_event(access: u32, inherit: i32, name: *const u16) -> Handle;
    #[link_name = "GetHandleInformation"]
    fn get_handle_information(handle: Handle, flags: *mut u32) -> i32;
    #[link_name = "SetHandleInformation"]
    fn set_handle_information(handle: Handle, mask: u32, flags: u32) -> i32;
}

#[link(name = "api-ms-win-core-handle-l1-1-0", kind = "raw-dylib")]
unsafe extern "system" {
    #[link_name = "CompareObjectHandles"]
    fn compare_handles(first: Handle, second: Handle) -> i32;
}

const STDIN: u32 = -10_i32 as u32;
const STDOUT: u32 = -11_i32 as u32;
const STDERR: u32 = -12_i32 as u32;
const WAIT_TIMEOUT: u32 = 258;
const SYNCHRONIZE: u32 = 0x0010_0000;
const MODE: &str = "ADZE_WINDOWS_FIXTURE_MODE";

fn os_hex(value: &OsStr) -> String {
    value
        .encode_wide()
        .map(|unit| format!("{unit:04x}"))
        .collect()
}

unsafe extern "system" fn retain_worker(event: u32) -> i32 {
    i32::from(matches!(event, 0 | 1))
}

fn retain_console_worker() -> Result<()> {
    // SAFETY: static callback; only this finite fixture process is affected.
    if unsafe { set_console_handler(Some(retain_worker), 1) } == 0 {
        return Err(io::Error::last_os_error().into());
    }
    Ok(())
}

fn env_path(name: &str) -> Result<PathBuf> {
    Ok(std::env::var_os(name)
        .ok_or_else(|| io::Error::other(format!("missing {name}")))?
        .into())
}

fn raw_cargo(args: &[OsString], in_job: bool) -> Result<u32> {
    assert!(
        in_job,
        "fake Cargo's first application instruction was outside a job"
    );
    assert_eq!(args.len(), 8, "Cargo argv count changed");
    assert_eq!(&args[..3], &["run", "--quiet", "--manifest-path"]);
    assert!(Path::new(&args[3]).is_file());
    assert_eq!(args[4], "--");
    assert_eq!(args[5], std::env::var_os("EXPECTED_INPUT").unwrap());
    assert_eq!(args[6], std::env::var_os("EXPECTED_OPERATION").unwrap());
    assert_eq!(args[7], "--adze-static-runner-protocol=2");
    assert_eq!(
        std::env::current_dir()?.canonicalize()?,
        env_path("EXPECTED_CWD")?.canonicalize()?
    );
    assert_eq!(std::env::var_os("ADZE_USE_PURE_RUST"), Some("1".into()));
    // No decode/re-encode of either channel: malformed wire bytes stay raw.
    io::stdout().write_all(&fs::read(env_path("WIRE_FILE")?)?)?;
    io::stderr().write_all(&fs::read(env_path("STDERR_FILE")?)?)?;
    io::stdout().flush()?;
    io::stderr().flush()?;
    Ok(std::env::var("EXIT_CODE")?.parse()?)
}

fn inspect(args: &[OsString], in_job: bool) -> Result<u32> {
    println!("job={}", u8::from(in_job));
    for arg in args {
        println!("arg={}", os_hex(arg));
    }
    println!("exe={}", os_hex(std::env::current_exe()?.as_os_str()));
    println!("cwd={}", os_hex(std::env::current_dir()?.as_os_str()));
    for key in [
        "ADZE_WINDOWS_KEEP",
        "ADZE_WINDOWS_REMOVE",
        "ADZE_USE_PURE_RUST",
        "CARGO_TARGET_DIR",
        "SystemRoot",
    ] {
        match std::env::var_os(key) {
            Some(value) => println!("env:{key}={}", os_hex(&value)),
            None => println!("env:{key}=<absent>"),
        }
    }
    let mut byte = [0];
    println!("stdin={}", io::stdin().read(&mut byte)?);
    if let (Some(handle), Some(name)) = (
        std::env::var_os("SENTINEL_HANDLE"),
        std::env::var_os("SENTINEL_NAME"),
    ) {
        let handle = handle.to_str().unwrap().parse::<usize>()? as Handle;
        let mut flags = 0;
        // SAFETY: validate before opening any new handle: OpenEventW could
        // otherwise reuse a free numeric slot and fabricate identity equality.
        if unsafe { get_handle_information(handle, &mut flags) } == 0 {
            println!("sentinel=0");
            return Ok(0);
        }
        let name: Vec<u16> = name.encode_wide().chain(Some(0)).collect();
        // SAFETY: open this test's known named event for identity comparison.
        let expected = unsafe { open_event(SYNCHRONIZE, 0, name.as_ptr()) };
        if expected.is_null() {
            return Err(io::Error::last_os_error().into());
        }
        // SAFETY: the API validates the possibly-uninherited numeric handle;
        // comparison does not take ownership or close that candidate handle.
        let inherited = unsafe { compare_handles(handle, expected) } != 0;
        // SAFETY: expected is this fixture's newly opened event handle.
        unsafe { close_handle(expected) };
        println!("sentinel={}", u8::from(inherited));
    }
    Ok(0)
}

fn finite_write(stdout_bytes: usize, stderr_bytes: usize) -> Result<u32> {
    let other = std::thread::spawn(move || -> io::Result<()> {
        let mut stream = io::stderr().lock();
        let chunk = [b'e'; 8192];
        let mut remaining = stderr_bytes;
        while remaining != 0 {
            let length = remaining.min(chunk.len());
            stream.write_all(&chunk[..length])?;
            remaining -= length;
        }
        stream.flush()
    });
    let mut stream = io::stdout().lock();
    let chunk = [b'o'; 8192];
    let mut remaining = stdout_bytes;
    while remaining != 0 {
        let length = remaining.min(chunk.len());
        stream.write_all(&chunk[..length])?;
        remaining -= length;
    }
    stream.flush()?;
    other.join().unwrap()?;
    Ok(0)
}

fn wait_pid(path: &Path) -> Result<u32> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Ok(text) = fs::read_to_string(path)
            && let Ok(pid) = text.parse()
        {
            return Ok(pid);
        }
        if Instant::now() >= deadline {
            return Err(io::Error::other("fixture PID was not published").into());
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn prepare_child_stdio() -> Result<()> {
    for kind in [STDIN, STDOUT, STDERR] {
        // SAFETY: borrow this fixture's known standard handles and prevent
        // hidden inherited originals from overriding the Command stdio choice.
        let handle = unsafe { std_handle(kind) };
        if handle.is_null() || handle as isize == -1 {
            return Err(io::Error::other("missing fixture standard handle").into());
        }
        if unsafe { set_handle_information(handle, 1, 0) } == 0 {
            return Err(io::Error::last_os_error().into());
        }
    }
    Ok(())
}

fn descendant(path: &Path, pipes: &str) -> Result<()> {
    prepare_child_stdio()?;
    let mut command = Command::new(std::env::current_exe()?);
    command
        .arg("hold")
        .arg(path)
        .env_remove(MODE)
        .stdin(Stdio::null());
    if !matches!(pipes, "pipes" | "stdout") {
        command.stdout(Stdio::null());
    }
    if !matches!(pipes, "pipes" | "stderr") {
        command.stderr(Stdio::null());
    }
    let child = command.spawn()?;
    wait_pid(path)?;
    // Deliberately do not wait/kill: the owning production job must clean up
    // this descendant, including after its immediate leader exits first.
    drop(child);
    Ok(())
}

struct ChildGuard(Child);

impl ChildGuard {
    fn wait_bounded(&mut self) -> Result<u32> {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if let Some(status) = self.0.try_wait()? {
                return Ok(status.code().unwrap() as u32);
            }
            if Instant::now() >= deadline {
                return Err(io::Error::other("fixture child wait timed out").into());
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        if !matches!(self.0.try_wait(), Ok(Some(_))) {
            let _ = self.0.kill();
            let deadline = Instant::now() + Duration::from_secs(1);
            while Instant::now() < deadline {
                if matches!(self.0.try_wait(), Ok(Some(_))) {
                    break;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
        }
    }
}

fn process_handle(pid: u32) -> Result<OwnedHandle> {
    // SAFETY: open the exact finite fixture PID while its readiness file and
    // controller establish ownership. Retain the object handle before killing
    // its parent so later observations do not depend on numeric PID reuse.
    let raw = unsafe { open_process(SYNCHRONIZE, 0, pid) };
    if raw.is_null() {
        return Err(io::Error::last_os_error().into());
    }
    // SAFETY: OpenProcess returned a newly owned, non-pseudo handle.
    Ok(unsafe { OwnedHandle::from_raw_handle(raw) })
}

fn wait_stopped(handle: &OwnedHandle) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        // SAFETY: the stable process object is owned by the controller.
        match unsafe { wait_object(handle.as_raw_handle(), 0) } {
            0 => return Ok(()),
            WAIT_TIMEOUT if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(10))
            }
            _ => {
                return Err(io::Error::other(
                    "owned descendant remained live after parent cleanup",
                )
                .into());
            }
        }
    }
}

fn controller(kind: &str, adze: &Path, directory: &Path) -> Result<u32> {
    let is_console = kind != "death";
    if is_console {
        // SAFETY: only this fixture process detaches and allocates its own
        // console. group0 events below cannot reach the hosted runner console.
        unsafe { free_console() };
        if unsafe { alloc_console() } == 0 {
            return Err(io::Error::last_os_error().into());
        }
        // Deliberately construct the desired inherited disposition inside this
        // isolated controller, never by mutating the hosted test process.
        if unsafe { set_console_handler(None, i32::from(kind == "ignored")) } == 0 {
            return Err(io::Error::last_os_error().into());
        }
        retain_console_worker()?;
    }
    prepare_child_stdio()?;
    let grammar = directory.join("grammar λ.rs");
    let input = directory.join("input λ.txt");
    fs::write(&grammar, "#[adze::grammar(\"fixture\")] mod grammar {}\n")?;
    fs::write(&input, "hello")?;
    let mut paths = vec![directory.join("bin")];
    paths.extend(std::env::split_paths(&std::env::var_os("PATH").unwrap()));
    let mut sentinel = ChildGuard(
        Command::new(std::env::current_exe()?)
            .arg("hold")
            .arg(directory.join("sibling.pid"))
            .env_remove(MODE)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?,
    );
    wait_pid(&directory.join("sibling.pid"))?;
    let mut child = ChildGuard(
        Command::new(adze)
            .arg("parse")
            .arg(&grammar)
            .arg(&input)
            .args(["--output", "document-json"])
            .current_dir(directory)
            .env("PATH", std::env::join_paths(paths)?)
            .env(MODE, "leader-holds")
            .env("DESCENDANT_PID", directory.join("descendant.pid"))
            .env("RUNNER_MANIFEST", directory.join("manifest.txt"))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(fs::File::create(directory.join("parent.stderr"))?)
            .spawn()?,
    );
    let descendant = process_handle(wait_pid(&directory.join("descendant.pid"))?)?;
    // SAFETY: the owned process handle must still be live before intervention.
    assert_eq!(
        unsafe { wait_object(descendant.as_raw_handle(), 0) },
        WAIT_TIMEOUT
    );
    if kind == "death" {
        child.0.kill()?;
    } else {
        let event = if kind == "break" { 1 } else { 0 };
        // SAFETY: this controller created its own console and retains itself
        // through the event. Only that console's controlled processes receive it.
        if unsafe { generate_console_event(event, 0) } == 0 {
            return Err(io::Error::last_os_error().into());
        }
        if kind == "ignored" {
            std::thread::sleep(Duration::from_millis(250));
            assert!(
                child.0.try_wait()?.is_none(),
                "inherited ignored Ctrl-C was changed"
            );
            if unsafe { generate_console_event(1, 0) } == 0 {
                return Err(io::Error::last_os_error().into());
            }
        }
    }
    let status = child.wait_bounded()?;
    if is_console {
        assert_eq!(status, 0xc000013a, "cancellation lost its Windows status");
        let manifest = PathBuf::from(fs::read_to_string(directory.join("manifest.txt"))?);
        assert!(
            !manifest.parent().unwrap().exists(),
            "graceful cancellation did not unwind the actual runner directory"
        );
    }
    wait_stopped(&descendant)?;
    assert!(
        sentinel.0.try_wait()?.is_none(),
        "cleanup killed an unrelated sibling"
    );
    fs::write(
        directory.join("result.txt"),
        format!("status={status}\ndescendant=stopped\nsibling=alive\n"),
    )?;
    Ok(0)
}

fn run(args: &[OsString], in_job: bool) -> Result<u32> {
    if let Some(mode) = std::env::var_os(MODE) {
        match mode.to_str().unwrap() {
            "raw" => return raw_cargo(args, in_job),
            "leader-holds" => {
                assert!(in_job);
                retain_console_worker()?;
                fs::write(env_path("RUNNER_MANIFEST")?, args[3].to_str().unwrap())?;
                descendant(&env_path("DESCENDANT_PID")?, "pipes")?;
                std::thread::sleep(Duration::from_secs(30));
                return Ok(0);
            }
            _ => return Err(io::Error::other("unknown native fixture mode").into()),
        }
    }
    let mode = args
        .first()
        .and_then(|arg| arg.to_str())
        .unwrap_or("inspect");
    match mode {
        "inspect" => inspect(args, in_job),
        "emit" => finite_write(
            args[1].to_str().unwrap().parse()?,
            args[2].to_str().unwrap().parse()?,
        ),
        "exit" => Ok(args[1].to_str().unwrap().parse()?),
        "mark" => {
            fs::write(Path::new(&args[1]), b"external program ran")?;
            Ok(0)
        }
        "quiet" => {
            std::thread::sleep(Duration::from_secs(30));
            Ok(0)
        }
        "closed" => {
            // SAFETY: close only this fixture's inherited standard output
            // handles; no later fixture output is attempted on this path.
            // The bootstrap retains its own diagnostics handles while waiting:
            // this is not a both-capture-pipes-EOF-before-leader-exit oracle.
            unsafe {
                close_handle(std_handle(STDOUT));
                set_std_handle(STDOUT, ptr::null_mut());
                close_handle(std_handle(STDERR));
                set_std_handle(STDERR, ptr::null_mut());
            }
            std::thread::sleep(Duration::from_secs(30));
            Ok(0)
        }
        "zero-write" => {
            let mut count = 0;
            // SAFETY: a real zero-length Win32 write, not Rust write_all([]),
            // which could elide the call. The ordinary payload follows it.
            if unsafe {
                write_file(
                    std_handle(STDOUT),
                    b"".as_ptr().cast(),
                    0,
                    &mut count,
                    ptr::null_mut(),
                )
            } == 0
            {
                return Err(io::Error::last_os_error().into());
            }
            io::stdout().write_all(b"after-zero-write")?;
            Ok(0)
        }
        "delayed-write" => {
            std::thread::sleep(Duration::from_millis(args[1].to_str().unwrap().parse()?));
            io::stdout().write_all(b"after-idle-write")?;
            Ok(0)
        }
        "flood" => {
            let end = Instant::now() + Duration::from_secs(30);
            let other = std::thread::spawn(move || {
                let mut stream = io::stderr().lock();
                while Instant::now() < end {
                    if stream.write_all(&[b'e'; 1024]).is_err() {
                        break;
                    }
                }
            });
            let mut stream = io::stdout().lock();
            while Instant::now() < end {
                stream.write_all(&[b'o'; 1024])?;
            }
            other.join().unwrap();
            Ok(0)
        }
        "progress" => {
            let end = Instant::now() + Duration::from_secs(30);
            while Instant::now() < end {
                io::stdout().write_all(b"p")?;
                io::stdout().flush()?;
                std::thread::sleep(Duration::from_millis(5));
            }
            Ok(0)
        }
        "hold" => {
            retain_console_worker()?;
            fs::write(Path::new(&args[1]), std::process::id().to_string())?;
            std::thread::sleep(Duration::from_secs(30));
            Ok(0)
        }
        "descendant" => {
            descendant(Path::new(&args[1]), args[2].to_str().unwrap())?;
            if args[3] != "0" {
                io::stdout().write_all(b"\xffmalformed success bytes")?;
            }
            Ok(args[3].to_str().unwrap().parse()?)
        }
        "controller" => controller(
            args[1].to_str().unwrap(),
            Path::new(&args[2]),
            Path::new(&args[3]),
        ),
        _ => Err(io::Error::other("unknown native fixture argument").into()),
    }
}

fn main() {
    // This is the first fixture application action, before child creation or
    // user output. Production also verifies membership in its specific job.
    let mut in_job = 0;
    if unsafe { is_process_in_job(current_process(), ptr::null_mut(), &mut in_job) } == 0 {
        eprintln!("fixture job query failed: {}", io::Error::last_os_error());
        std::process::exit(1);
    }
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    match run(&args, in_job != 0) {
        Ok(code) => std::process::exit(code as i32),
        Err(error) => {
            if let Some(path) = std::env::var_os("ADZE_FIXTURE_ERROR") {
                let _ = fs::write(path, error.to_string());
            }
            eprintln!("native fixture failed: {error}");
            std::process::exit(1);
        }
    }
}
