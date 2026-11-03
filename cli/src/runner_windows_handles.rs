//! Owned Win32 objects for the private Windows runner. No parser interpretation.

use crate::runner_windows_request::{
    HEADER_BYTES, MAX_REQUEST_BYTES, RunnerRequest, request_length,
};
use std::ffi::OsStr;
use std::io;
use std::mem::{size_of, size_of_val, zeroed};
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::path::Path;
use std::ptr;
use std::sync::atomic::{AtomicU64, Ordering};
use windows_sys::Win32::Foundation::{
    CloseHandle, DuplicateHandle, ERROR_BROKEN_PIPE, ERROR_INSUFFICIENT_BUFFER, ERROR_NO_DATA,
    ERROR_PIPE_CONNECTED, ERROR_PIPE_NOT_CONNECTED, GENERIC_READ, GENERIC_WRITE, HANDLE,
    HANDLE_FLAG_INHERIT, INVALID_HANDLE_VALUE, SetHandleInformation, WAIT_OBJECT_0, WAIT_TIMEOUT,
};
use windows_sys::Win32::Security::SECURITY_ATTRIBUTES;
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_FLAG_FIRST_PIPE_INSTANCE, FILE_SHARE_READ,
    FILE_SHARE_WRITE, OPEN_EXISTING, PIPE_ACCESS_INBOUND, ReadFile,
};
use windows_sys::Win32::System::Console::{
    GetStdHandle, STD_ERROR_HANDLE, STD_INPUT_HANDLE, STD_OUTPUT_HANDLE,
};
use windows_sys::Win32::System::JobObjects::{
    CreateJobObjectW, IsProcessInJob, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    JOBOBJECT_BASIC_ACCOUNTING_INFORMATION, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
    JobObjectBasicAccountingInformation, JobObjectExtendedLimitInformation,
    QueryInformationJobObject, SetInformationJobObject, TerminateJobObject,
};
use windows_sys::Win32::System::Memory::{
    CreateFileMappingW, FILE_MAP_READ, FILE_MAP_WRITE, GetProcessHeap, HeapAlloc, HeapFree,
    MEMORY_MAPPED_VIEW_ADDRESS, MapViewOfFile, PAGE_READWRITE, UnmapViewOfFile,
};
use windows_sys::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, PIPE_NOWAIT, PIPE_READMODE_BYTE,
    PIPE_REJECT_REMOTE_CLIENTS, PIPE_TYPE_BYTE,
};
use windows_sys::Win32::System::Threading::{
    CREATE_UNICODE_ENVIRONMENT, CreateProcessW, DeleteProcThreadAttributeList,
    EXTENDED_STARTUPINFO_PRESENT, GetCurrentProcess, GetExitCodeProcess,
    InitializeProcThreadAttributeList, LPPROC_THREAD_ATTRIBUTE_LIST,
    PROC_THREAD_ATTRIBUTE_HANDLE_LIST, PROC_THREAD_ATTRIBUTE_JOB_LIST, PROCESS_INFORMATION,
    STARTF_USESTDHANDLES, STARTUPINFOEXW, TerminateProcess, UpdateProcThreadAttribute,
    WaitForSingleObject,
};

pub(crate) const BOOTSTRAP_MODE: &str = "--adze-runner-bootstrap-v1";
pub(crate) const FORCED_EXIT: u32 = 0xc000013a;

pub(crate) fn prepare_bootstrap_stdio() -> io::Result<()> {
    for kind in [STD_INPUT_HANDLE, STD_OUTPUT_HANDLE, STD_ERROR_HANDLE] {
        // SAFETY: the private native parent supplied these three standard
        // handles. Borrow them without taking ownership or closing diagnostics.
        let handle = unsafe { GetStdHandle(kind) };
        if handle.is_null() || handle == INVALID_HANDLE_VALUE {
            return Err(io::Error::other(
                "missing inherited bootstrap standard handle",
            ));
        }
        // SAFETY: change only this process's borrowed handle inheritance flag.
        // Stable Command creates the intended child stdio duplicates; the
        // original handles must not also become hidden inherited pipe writers.
        if unsafe { SetHandleInformation(handle, HANDLE_FLAG_INHERIT, 0) } == 0 {
            return Err(io::Error::last_os_error());
        }
    }
    Ok(())
}

pub(crate) struct Handle(OwnedHandle);

impl Handle {
    fn from_created(raw: HANDLE) -> io::Result<Self> {
        if raw.is_null() || raw == INVALID_HANDLE_VALUE {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: every caller transfers a newly created, non-pseudo handle;
        // null and INVALID_HANDLE_VALUE were excluded above.
        Ok(Self(unsafe { OwnedHandle::from_raw_handle(raw) }))
    }

    pub(crate) fn raw(&self) -> HANDLE {
        self.0.as_raw_handle()
    }
}

struct View {
    address: MEMORY_MAPPED_VIEW_ADDRESS,
    length: usize,
}

impl View {
    fn read(handle: &Handle, length: usize) -> io::Result<Self> {
        // SAFETY: the kernel validates the requested mapping extent and access.
        let address = unsafe { MapViewOfFile(handle.raw(), FILE_MAP_READ, 0, 0, length) };
        if address.Value.is_null() {
            return Err(io::Error::last_os_error());
        }
        Ok(Self { address, length })
    }

    fn bytes(&self) -> &[u8] {
        // SAFETY: a successful MapViewOfFile established this exact extent;
        // the view remains owned, read-only and mapped for the borrow.
        unsafe { std::slice::from_raw_parts(self.address.Value.cast(), self.length) }
    }

    fn close(mut self) -> io::Result<()> {
        // SAFETY: this is the live mapping owned by this view. On failure it
        // remains in the guard for a final best-effort unmap, and is reported.
        if unsafe { UnmapViewOfFile(self.address) } == 0 {
            return Err(io::Error::last_os_error());
        }
        self.address.Value = ptr::null_mut();
        Ok(())
    }
}

impl Drop for View {
    fn drop(&mut self) {
        if !self.address.Value.is_null() {
            // SAFETY: this guard owns the still-mapped view. No I/O is pending.
            unsafe { UnmapViewOfFile(self.address) };
        }
    }
}

pub(crate) fn request_mapping(bytes: &[u8]) -> io::Result<Handle> {
    if !(HEADER_BYTES..=MAX_REQUEST_BYTES).contains(&bytes.len()) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "request mapping extent is outside its bounded limit",
        ));
    }
    let length =
        u32::try_from(bytes.len()).map_err(|_| io::Error::other("mapping size overflow"))?;
    // SAFETY: an unnamed pagefile mapping of the checked, nonzero request size.
    // No inherited handle or executable mapping is requested.
    let writable = Handle::from_created(unsafe {
        CreateFileMappingW(
            INVALID_HANDLE_VALUE,
            ptr::null(),
            PAGE_READWRITE,
            0,
            length,
            ptr::null(),
        )
    })?;
    // SAFETY: this new mapping has PAGE_READWRITE and the requested extent.
    let address = unsafe { MapViewOfFile(writable.raw(), FILE_MAP_WRITE, 0, 0, bytes.len()) };
    if address.Value.is_null() {
        return Err(io::Error::last_os_error());
    }
    let view = View {
        address,
        length: bytes.len(),
    };
    // SAFETY: the private writable view has bytes.len() available bytes and no
    // aliases in a child; source and destination cannot overlap.
    unsafe { ptr::copy_nonoverlapping(bytes.as_ptr(), view.address.Value.cast(), bytes.len()) };
    let mut read_only = ptr::null_mut();
    // SAFETY: duplicate the owned mapping within this process with read-only
    // access. Only this duplicate is inheritable; the job never is.
    if unsafe {
        DuplicateHandle(
            GetCurrentProcess(),
            writable.raw(),
            GetCurrentProcess(),
            &mut read_only,
            FILE_MAP_READ,
            1,
            0,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    let read_only = Handle::from_created(read_only)?;
    view.close()?;
    drop(writable);
    Ok(read_only)
}

pub(crate) fn decode_inherited_request(value: usize) -> io::Result<RunnerRequest> {
    let inherited = value as HANDLE;
    if inherited.is_null() || inherited == INVALID_HANDLE_VALUE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid request handle",
        ));
    }
    let mut duplicate = ptr::null_mut();
    // SAFETY: the OS validates the untrusted numeric argument. Do not construct
    // an OwnedHandle directly from argv: only a successful duplicate is newly
    // owned here. MapViewOfFile below also establishes that it is a mapping.
    if unsafe {
        DuplicateHandle(
            GetCurrentProcess(),
            inherited,
            GetCurrentProcess(),
            &mut duplicate,
            FILE_MAP_READ,
            0,
            0,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    let handle = Handle::from_created(duplicate)?;
    let header = View::read(&handle, HEADER_BYTES)?;
    let length = request_length(header.bytes());
    header.close()?;
    let view = View::read(&handle, length?)?;
    let request = RunnerRequest::decode(view.bytes());
    view.close()?;
    // Neither the view nor its inheritable handle may reach the external child.
    drop(handle);
    // SAFETY: successful mapping established the original inherited object's
    // type. This private bootstrap owns that original, but used a validated
    // duplicate for Rust ownership even for malformed direct invocations.
    if unsafe { CloseHandle(inherited) } == 0 {
        return Err(io::Error::last_os_error());
    }
    request
}

pub(crate) enum ReadResult {
    Bytes(usize),
    Idle,
    Closed,
}

pub(crate) struct Pipe(Handle);

static PIPE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn wide(value: &OsStr) -> io::Result<Vec<u16>> {
    let mut result: Vec<u16> = value.encode_wide().collect();
    if result.contains(&0) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Win32 string contains NUL",
        ));
    }
    result.push(0);
    Ok(result)
}

fn inheritable_attributes() -> SECURITY_ATTRIBUTES {
    SECURITY_ATTRIBUTES {
        nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: ptr::null_mut(),
        bInheritHandle: 1,
    }
}

impl Pipe {
    fn pair() -> io::Result<(Self, Handle)> {
        let sequence = PIPE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let name = wide(OsStr::new(&format!(
            r"\\.\pipe\adze-runner-{}-{sequence}",
            std::process::id()
        )))?;
        // SAFETY: all arguments describe a new local byte pipe. FIRST_INSTANCE
        // makes a name collision fail; no existing pipe is silently reused.
        let read = Handle::from_created(unsafe {
            CreateNamedPipeW(
                name.as_ptr(),
                PIPE_ACCESS_INBOUND | FILE_FLAG_FIRST_PIPE_INSTANCE,
                PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_NOWAIT | PIPE_REJECT_REMOTE_CLIENTS,
                1,
                0,
                64 * 1024,
                0,
                ptr::null(),
            )
        })?;
        let attributes = inheritable_attributes();
        // SAFETY: connect the sole client locally before spawning. Only this
        // write endpoint is inheritable; its default PIPE_WAIT mode preserves
        // ordinary blocking stdout/stderr writes in the child.
        let write = Handle::from_created(unsafe {
            CreateFileW(
                name.as_ptr(),
                GENERIC_WRITE,
                0,
                &attributes,
                OPEN_EXISTING,
                FILE_ATTRIBUTE_NORMAL,
                ptr::null_mut(),
            )
        })?;
        let pipe = Self(read);
        match pipe.connection_state()? {
            ReadResult::Idle => Ok((pipe, write)),
            _ => Err(io::Error::other(
                "new runner pipe did not establish its connection",
            )),
        }
    }

    fn connection_state(&self) -> io::Result<ReadResult> {
        // SAFETY: this server handle is PIPE_NOWAIT. After initial connection
        // it is never disconnected/reused; this call always completes now.
        let result = unsafe { ConnectNamedPipe(self.0.raw(), ptr::null_mut()) };
        if result != 0 {
            return Err(io::Error::other(
                "runner pipe unexpectedly became reconnectable",
            ));
        }
        match io::Error::last_os_error()
            .raw_os_error()
            .map(|code| code as u32)
        {
            Some(ERROR_PIPE_CONNECTED) => Ok(ReadResult::Idle),
            Some(ERROR_NO_DATA) => Ok(ReadResult::Closed),
            _ => Err(io::Error::last_os_error()),
        }
    }

    pub(crate) fn read(&self, bytes: &mut [u8]) -> io::Result<ReadResult> {
        let count =
            u32::try_from(bytes.len()).map_err(|_| io::Error::other("pipe read size overflow"))?;
        let mut closure_observed = false;
        loop {
            let mut read = 0;
            // SAFETY: the mutable buffer has count bytes and is used only during
            // this synchronous PIPE_NOWAIT call. There is no pending OVERLAPPED
            // operation, reader thread or buffer lifetime beyond the return.
            let result = unsafe {
                ReadFile(
                    self.0.raw(),
                    bytes.as_mut_ptr(),
                    count,
                    &mut read,
                    ptr::null_mut(),
                )
            };
            if result != 0 {
                if read != 0 {
                    return Ok(ReadResult::Bytes(read as usize));
                }
                // A successful zero-byte read can consume a zero-byte write
                // while later bytes are queued. It is never an EOF oracle.
                return Ok(ReadResult::Idle);
            }
            let error = io::Error::last_os_error();
            match error.raw_os_error().map(|code| code as u32) {
                Some(ERROR_NO_DATA) if closure_observed => return Ok(ReadResult::Closed),
                Some(ERROR_NO_DATA) => match self.connection_state()? {
                    ReadResult::Idle => return Ok(ReadResult::Idle),
                    ReadResult::Closed => {
                        // A writer could append and close between the empty
                        // read and the connection probe. Read once more after
                        // known closure before declaring the buffer drained.
                        closure_observed = true;
                    }
                    ReadResult::Bytes(_) => {
                        return Err(io::Error::other("invalid pipe connection state"));
                    }
                },
                Some(ERROR_BROKEN_PIPE | ERROR_PIPE_NOT_CONNECTED) => {
                    return Ok(ReadResult::Closed);
                }
                _ => return Err(error),
            }
        }
    }
}

struct Attributes {
    raw: LPPROC_THREAD_ATTRIBUTE_LIST,
    initialized: bool,
}

impl Attributes {
    fn new() -> io::Result<Self> {
        let mut bytes = 0;
        // SAFETY: documented size query, with no attribute storage yet.
        let result =
            unsafe { InitializeProcThreadAttributeList(ptr::null_mut(), 2, 0, &mut bytes) };
        if result != 0
            || io::Error::last_os_error().raw_os_error() != Some(ERROR_INSUFFICIENT_BUFFER as i32)
            || bytes == 0
        {
            return Err(io::Error::other("could not size process attribute list"));
        }
        // SAFETY: the Windows heap supplies the alignment required by this
        // opaque Win32 structure; allocation is owned until HeapFree below.
        let raw = unsafe { HeapAlloc(GetProcessHeap(), 0, bytes) };
        if raw.is_null() {
            return Err(io::Error::other(
                "could not allocate process attribute list",
            ));
        }
        let mut attributes = Self {
            raw,
            initialized: false,
        };
        // SAFETY: raw has the exact queried size/alignment and is exclusively owned.
        if unsafe { InitializeProcThreadAttributeList(raw, 2, 0, &mut bytes) } == 0 {
            return Err(io::Error::last_os_error());
        }
        attributes.initialized = true;
        Ok(attributes)
    }

    fn handles(&mut self, key: u32, handles: &[HANDLE]) -> io::Result<()> {
        // SAFETY: the caller retains the slices and their live handles through
        // CreateProcessW; UpdateProcThreadAttribute borrows these values.
        if unsafe {
            UpdateProcThreadAttribute(
                self.raw,
                0,
                key as usize,
                handles.as_ptr().cast(),
                std::mem::size_of_val(handles),
                ptr::null_mut(),
                ptr::null(),
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }
}

impl Drop for Attributes {
    fn drop(&mut self) {
        // SAFETY: this allocation belongs to the process heap; Delete is used
        // only after successful initialization. Neither call waits on a child.
        unsafe {
            if self.initialized {
                DeleteProcThreadAttributeList(self.raw);
            }
            HeapFree(GetProcessHeap(), 0, self.raw);
        }
    }
}

pub(crate) struct OwnedChild {
    job: Handle,
    process: Option<Handle>,
    pub(crate) stdout: Pipe,
    pub(crate) stderr: Pipe,
    pub(crate) process_id: u32,
}

impl std::fmt::Debug for OwnedChild {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OwnedChild")
            .field("process_id", &self.process_id)
            .field("leader_closed", &self.leader_closed())
            .finish_non_exhaustive()
    }
}

impl OwnedChild {
    pub(crate) fn spawn(
        bootstrap: &Path,
        request: &[u8],
        before_create: impl FnOnce() -> io::Result<()>,
    ) -> io::Result<Self> {
        let mapping = request_mapping(request)?;
        let (stdout, stdout_write) = Pipe::pair()?;
        let (stderr, stderr_write) = Pipe::pair()?;
        let attributes = inheritable_attributes();
        let null = wide(OsStr::new("NUL"))?;
        // SAFETY: a read-only NUL handle supplies closed/EOF stdin.
        let stdin = Handle::from_created(unsafe {
            CreateFileW(
                null.as_ptr(),
                GENERIC_READ,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                &attributes,
                OPEN_EXISTING,
                FILE_ATTRIBUTE_NORMAL,
                ptr::null_mut(),
            )
        })?;
        // SAFETY: unnamed job, default security, explicitly non-inheritable.
        let job = Handle::from_created(unsafe { CreateJobObjectW(ptr::null(), ptr::null()) })?;
        // SAFETY: these Win32 structures consist of zero-valid scalar/pointer
        // fields; documented fields are filled before each API consumes them.
        let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { zeroed() };
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        // SAFETY: pointer and byte size describe the live limits structure.
        if unsafe {
            SetInformationJobObject(
                job.raw(),
                JobObjectExtendedLimitInformation,
                (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                size_of_val(&limits) as u32,
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        let jobs = [job.raw()];
        let handles = [
            stdin.raw(),
            stdout_write.raw(),
            stderr_write.raw(),
            mapping.raw(),
        ];
        let mut attributes = Attributes::new()?;
        attributes.handles(PROC_THREAD_ATTRIBUTE_JOB_LIST, &jobs)?;
        attributes.handles(PROC_THREAD_ATTRIBUTE_HANDLE_LIST, &handles)?;
        let executable = wide(bootstrap.as_os_str())?;
        if !bootstrap.is_absolute() || executable.contains(&(b'"' as u16)) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "bootstrap executable must be an absolute native path",
            ));
        }
        let mut command_line = vec![b'"' as u16];
        command_line.extend_from_slice(&executable[..executable.len() - 1]);
        command_line.push(b'"' as u16);
        command_line.extend(format!(" {BOOTSTRAP_MODE} {}", mapping.raw() as usize).encode_utf16());
        command_line.push(0);
        if command_line.len() > 32767 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "bootstrap executable command line is too long",
            ));
        }
        // SAFETY: zero-valid Win32 startup/result structures; all supplied
        // handles/buffers and the opaque attributes remain alive through spawn.
        let mut startup: STARTUPINFOEXW = unsafe { zeroed() };
        startup.StartupInfo.cb = size_of::<STARTUPINFOEXW>() as u32;
        startup.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
        startup.StartupInfo.hStdInput = stdin.raw();
        startup.StartupInfo.hStdOutput = stdout_write.raw();
        startup.StartupInfo.hStdError = stderr_write.raw();
        startup.lpAttributeList = attributes.raw;
        // SAFETY: PROCESS_INFORMATION has no invalid Rust scalar bit patterns.
        let mut process: PROCESS_INFORMATION = unsafe { zeroed() };
        before_create()?;
        // SAFETY: JOB_LIST assigns the new process before its initial thread
        // runs. HANDLE_LIST contains only the four intentional inherited
        // handles, never the kill-on-close job. NULL env/cwd preserve the CLI
        // context; the helper applies request changes only to its Command.
        if unsafe {
            CreateProcessW(
                executable.as_ptr(),
                command_line.as_mut_ptr(),
                ptr::null(),
                ptr::null(),
                1,
                EXTENDED_STARTUPINFO_PRESENT | CREATE_UNICODE_ENVIRONMENT,
                ptr::null(),
                ptr::null(),
                &startup.StartupInfo,
                &mut process,
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        let process_handle = Handle::from_created(process.hProcess)?;
        let thread = Handle::from_created(process.hThread)?;
        drop(thread);
        let child = Self {
            job,
            process: Some(process_handle),
            stdout,
            stderr,
            process_id: process.dwProcessId,
        };
        drop(stdin);
        drop(stdout_write);
        drop(stderr_write);
        drop(mapping);
        let mut member = 0;
        // SAFETY: both handles identify objects this guard owns; no PID lookup.
        if unsafe {
            IsProcessInJob(
                child.process.as_ref().expect("new process").raw(),
                child.job.raw(),
                &mut member,
            )
        } == 0
            || member == 0
        {
            return Err(io::Error::other(
                "atomic runner job membership could not be confirmed",
            ));
        }
        Ok(child)
    }

    pub(crate) fn status(&mut self) -> io::Result<Option<u32>> {
        let Some(process) = &self.process else {
            return Ok(None);
        };
        // SAFETY: this guard owns the process object, even after leader exit.
        match unsafe { WaitForSingleObject(process.raw(), 0) } {
            WAIT_TIMEOUT => Ok(None),
            WAIT_OBJECT_0 => {
                let mut code = 0;
                // SAFETY: the process is signaled. 259 is now a valid terminal
                // exit code, not a liveness sentinel.
                if unsafe { GetExitCodeProcess(process.raw(), &mut code) } == 0 {
                    return Err(io::Error::last_os_error());
                }
                self.process.take();
                Ok(Some(code))
            }
            _ => Err(io::Error::last_os_error()),
        }
    }

    pub(crate) fn terminate(&self) -> io::Result<()> {
        // SAFETY: only the private job created for this invocation is affected.
        if unsafe { TerminateJobObject(self.job.raw(), FORCED_EXIT) } == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    pub(crate) fn active_processes(&self) -> io::Result<u32> {
        // SAFETY: zero-valid output structure and its exact writable extent.
        let mut accounting: JOBOBJECT_BASIC_ACCOUNTING_INFORMATION = unsafe { zeroed() };
        if unsafe {
            QueryInformationJobObject(
                self.job.raw(),
                JobObjectBasicAccountingInformation,
                (&mut accounting as *mut JOBOBJECT_BASIC_ACCOUNTING_INFORMATION).cast(),
                size_of_val(&accounting) as u32,
                ptr::null_mut(),
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        Ok(accounting.ActiveProcesses)
    }

    pub(crate) fn leader_closed(&self) -> bool {
        self.process.is_none()
    }
}

impl Drop for OwnedChild {
    fn drop(&mut self) {
        // SAFETY: this guard owns both objects. These are finite best-effort
        // requests, never a hidden wait. Explicit cleanup reports its errors.
        // The non-inherited job handle then closes and retains kill-on-close.
        unsafe {
            TerminateJobObject(self.job.raw(), FORCED_EXIT);
            if let Some(process) = &self.process {
                TerminateProcess(process.raw(), FORCED_EXIT);
            }
        }
    }
}
