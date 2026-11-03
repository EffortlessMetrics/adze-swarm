//! Request-owned command evidence for the local release harness (#1065).
//!
//! This data seam is deliberately unwired. It records an immutable native
//! request and an observation; it neither launches processes nor infers custody
//! from an exit status. The existing CapturedCommand view will be projected
//! from this authority when the reviewed process adapter is connected.
//!
//! Individual records do not establish whole-run completeness. The eventual
//! orchestrator must propagate completion recording errors and reject any
//! aggregate with an incomplete/dropped intent. `failed_recording` only stops
//! reuse after a `begin` recording failure; pending handles are independent.

use std::ffi::{OsStr, OsString};
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitStatus;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};
use serde::Serialize;
use sha2::{Digest, Sha256};

#[derive(Clone)]
pub(crate) enum EnvironmentChange {
    Set {
        name: OsString,
        value: OsString,
        record_value: bool,
    },
    Remove(OsString),
}

/// Requested limits only. Recording these numbers does not prove enforcement.
#[derive(Clone, Copy, Debug, Serialize)]
pub(crate) struct CapturePolicy {
    pub(crate) stdout_bytes: usize,
    pub(crate) stderr_bytes: usize,
    pub(crate) total_millis: u64,
    pub(crate) drain_millis: u64,
    pub(crate) cleanup_millis: u64,
}

#[derive(Clone)]
pub(crate) struct ReleaseCommand {
    name: String,
    program: OsString,
    arguments: Vec<OsString>,
    cwd: PathBuf,
    environment: Vec<EnvironmentChange>,
    policy: CapturePolicy,
}

impl ReleaseCommand {
    pub(crate) fn new(
        name: String,
        program: OsString,
        arguments: Vec<OsString>,
        cwd: PathBuf,
        environment: Vec<EnvironmentChange>,
        policy: CapturePolicy,
    ) -> Result<Self> {
        if name.is_empty() || name.len() > 128 || program.is_empty() {
            bail!("release command requires a bounded name and a nonempty program");
        }
        if !cwd.is_absolute() {
            bail!("release command cwd must be an explicit absolute path");
        }
        if policy.total_millis == 0 || policy.drain_millis == 0 || policy.cleanup_millis == 0 {
            bail!("release command deadlines must be nonzero");
        }
        Ok(Self {
            name,
            program,
            arguments,
            cwd,
            environment,
            policy,
        })
    }

    pub(crate) fn program(&self) -> &OsStr {
        &self.program
    }

    pub(crate) fn arguments(&self) -> &[OsString] {
        &self.arguments
    }

    pub(crate) fn cwd(&self) -> &Path {
        &self.cwd
    }

    pub(crate) fn environment(&self) -> &[EnvironmentChange] {
        &self.environment
    }

    pub(crate) fn policy(&self) -> CapturePolicy {
        self.policy
    }

    fn check_record_budget(&self, limit: usize) -> Result<()> {
        // Bound construction of the lossless recorded view before allocating
        // its hex/display strings. The later JSON writer enforces exact bytes.
        let mut size = self.name.len();
        let mut account = |value: &OsStr| -> Result<()> {
            size = size.checked_add(value.as_encoded_bytes().len())
                .and_then(|value| value.checked_add(32))
                .context("release command request size overflow")?;
            if size > limit {
                bail!("release command request exceeds its journal input budget");
            }
            Ok(())
        };
        account(&self.program)?;
        account(self.cwd.as_os_str())?;
        for value in &self.arguments { account(value)?; }
        for change in &self.environment {
            match change {
                EnvironmentChange::Set { name, value, .. } => {
                    account(name)?;
                    account(value)?;
                }
                EnvironmentChange::Remove(name) => account(name)?,
            }
        }
        Ok(())
    }

    fn recorded(&self) -> RecordedRequest {
        RecordedRequest {
            name: self.name.clone(),
            program: NativeString::from_os(&self.program),
            arguments: self.arguments.iter().map(|v| NativeString::from_os(v)).collect(),
            cwd: NativeString::from_os(self.cwd.as_os_str()),
            environment_policy: "inherit_with_ordered_changes",
            inherited_environment_values: "not_recorded",
            inherited_environment_identity: "unknown",
            environment: self.environment.iter().map(RecordedEnvironmentChange::from).collect(),
            requested_policy: self.policy,
        }
    }
}

/// Lossless native units are authority; display is optional and never lossy.
#[derive(Debug, Serialize)]
struct NativeString {
    encoding: &'static str,
    units_hex: String,
    display: Option<String>,
}

impl NativeString {
    fn from_os(value: &OsStr) -> Self {
        #[cfg(unix)]
        let (encoding, units_hex) = {
            use std::os::unix::ffi::OsStrExt;
            ("unix_bytes", hex(value.as_bytes()))
        };
        #[cfg(windows)]
        let (encoding, units_hex) = {
            use std::os::windows::ffi::OsStrExt;
            let bytes: Vec<_> = value.encode_wide().flat_map(u16::to_le_bytes).collect();
            ("windows_utf16_le", hex(&bytes))
        };
        #[cfg(not(any(unix, windows)))]
        compile_error!("release command native-string evidence supports Unix and Windows");
        Self {
            encoding,
            units_hex,
            display: value.to_str().map(str::to_owned),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
enum RecordedEnvironmentChange {
    Set {
        name: NativeString,
        value: Option<NativeString>,
        value_redacted: bool,
    },
    Remove {
        name: NativeString,
    },
}

impl From<&EnvironmentChange> for RecordedEnvironmentChange {
    fn from(change: &EnvironmentChange) -> Self {
        match change {
            EnvironmentChange::Set { name, value, record_value } => Self::Set {
                name: NativeString::from_os(name),
                value: (*record_value).then(|| NativeString::from_os(value)),
                value_redacted: !*record_value,
            },
            EnvironmentChange::Remove(name) => Self::Remove {
                name: NativeString::from_os(name),
            },
        }
    }
}

#[derive(Debug, Serialize)]
struct RecordedRequest {
    name: String,
    program: NativeString,
    arguments: Vec<NativeString>,
    cwd: NativeString,
    environment_policy: &'static str,
    inherited_environment_values: &'static str,
    inherited_environment_identity: &'static str,
    environment: Vec<RecordedEnvironmentChange>,
    requested_policy: CapturePolicy,
}

/// Supplied by the existing source owner; this journal does not re-prove Git.
#[derive(Clone, Debug, Serialize)]
pub(crate) struct SourceBinding {
    pub(crate) source_commit: String,
    pub(crate) source_tree: String,
    pub(crate) selected_version: String,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct JournalLimits {
    pub(crate) commands: u32,
    pub(crate) request_bytes: usize,
    pub(crate) completion_bytes: usize,
}

#[derive(Debug, Serialize)]
struct Timestamp {
    unix_seconds: u64,
    subsecond_nanoseconds: u32,
}

impl Timestamp {
    fn now() -> Result<Self> {
        let elapsed = SystemTime::now().duration_since(UNIX_EPOCH)
            .context("recording release command time")?;
        Ok(Self { unix_seconds: elapsed.as_secs(), subsecond_nanoseconds: elapsed.subsec_nanos() })
    }
}

#[derive(Debug, Serialize)]
struct IntentRecord<'a> {
    schema_version: u32,
    kind: &'static str,
    state: &'static str,
    id: u32,
    recorded_at: Timestamp,
    source: &'a SourceBinding,
    request: RecordedRequest,
    launch_evidence: &'static str,
    custody_evidence: &'static str,
    package_first_qualified: bool,
}

pub(crate) struct CommandJournal {
    root: PathBuf,
    source: SourceBinding,
    limits: JournalLimits,
    next_id: u32,
    failed_recording: bool,
}

impl CommandJournal {
    /// `artifact_root` must already be the provenance owner's retained root.
    /// A new namespace prevents accidental append/reuse of another execution.
    pub(crate) fn create(
        artifact_root: &Path,
        source: SourceBinding,
        limits: JournalLimits,
    ) -> Result<Self> {
        if limits.commands == 0 || limits.request_bytes == 0 || limits.completion_bytes == 0 {
            bail!("release command journal limits must be nonzero");
        }
        for identity in [&source.source_commit, &source.source_tree] {
            if identity.len() != 40 || !identity.bytes().all(|b| b.is_ascii_hexdigit()) {
                bail!("release command source binding requires full Git identities");
            }
        }
        if source.selected_version.len() > 256 {
            bail!("selected release version exceeds the journal input budget");
        }
        semver::Version::parse(&source.selected_version)
            .context("release command source binding requires an exact selected version")?;
        let artifact_root = fs::canonicalize(artifact_root)
            .context("canonicalizing retained release artifact root")?;
        let root = artifact_root.join("commands");
        fs::create_dir(&root).context("release command journal must be newly owned")?;
        Ok(Self { root, source, limits, next_id: 1, failed_recording: false })
    }

    pub(crate) fn begin(&mut self, request: ReleaseCommand) -> Result<PendingCommand> {
        if self.failed_recording {
            bail!("release command journal has an incomplete recording failure");
        }
        if self.next_id > self.limits.commands {
            bail!("release command journal reached its command limit");
        }
        request.check_record_budget(self.limits.request_bytes)?;
        let id = self.next_id;
        let intent = IntentRecord {
            schema_version: 1,
            kind: "local-release-command",
            state: "intent_recorded",
            id,
            recorded_at: Timestamp::now()?,
            source: &self.source,
            request: request.recorded(),
            launch_evidence: "unknown",
            custody_evidence: "unknown",
            package_first_qualified: false,
        };
        let raw = json_bytes(&intent, self.limits.request_bytes)?;
        let directory = self.root.join(format!("{id:08}"));
        self.failed_recording = true;
        fs::create_dir(&directory).context("creating release command entry")?;
        write_atomic_new(&directory.join("intent.json"), &raw)?;
        self.next_id = self.next_id.checked_add(1).context("release command id overflow")?;
        self.failed_recording = false;
        Ok(PendingCommand {
            id,
            directory,
            request,
            intent_sha256: sha256(&raw),
            intent_bytes: raw.len(),
            completion_limit: self.limits.completion_bytes,
            intent_time: Instant::now(),
        })
    }
}

/// Owns the exact request supplied to `begin`; adapters receive only a borrow.
/// Dropping this value leaves intent, with no affirmative launch/completion fact.
pub(crate) struct PendingCommand {
    id: u32,
    directory: PathBuf,
    request: ReleaseCommand,
    intent_sha256: String,
    intent_bytes: usize,
    completion_limit: usize,
    intent_time: Instant,
}

pub(crate) struct CommandObservation {
    natural_status: Option<ExitStatus>,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    failure: Option<anyhow::Error>,
}

impl CommandObservation {
    pub(crate) fn natural(status: ExitStatus, stdout: Vec<u8>, stderr: Vec<u8>) -> Self {
        Self { natural_status: Some(status), stdout, stderr, failure: None }
    }

    pub(crate) fn failed(
        error: anyhow::Error,
        natural_status: Option<ExitStatus>,
        stdout: Vec<u8>,
        stderr: Vec<u8>,
    ) -> Self {
        Self { natural_status, stdout, stderr, failure: Some(error) }
    }
}

#[derive(Debug, Serialize)]
#[serde(tag = "platform", rename_all = "snake_case")]
enum NativeStatus {
    #[cfg(unix)]
    Unix { code: Option<i32>, signal: Option<i32>, core_dumped: bool },
    #[cfg(windows)]
    Windows { code: u32 },
}

impl NativeStatus {
    fn from_status(status: ExitStatus) -> Self {
        #[cfg(unix)]
        {
            use std::os::unix::process::ExitStatusExt;
            Self::Unix { code: status.code(), signal: status.signal(), core_dumped: status.core_dumped() }
        }
        #[cfg(windows)]
        {
            // Windows always has an exit code. Preserve its unsigned bit pattern.
            Self::Windows { code: status.code().expect("Windows exit status has a code") as u32 }
        }
    }
}

#[derive(Debug, Serialize)]
struct OutputArtifact {
    path: String,
    observed_bytes: usize,
    retained_bytes: usize,
    sha256: String,
    truncated_by_journal: bool,
}

#[derive(Debug, Serialize)]
struct CompletionRecord {
    schema_version: u32,
    kind: &'static str,
    state: &'static str,
    id: u32,
    // Integrity of the public/redacted intent file, not an execution-custody key.
    intent_sha256: String,
    recorded_at: Timestamp,
    elapsed_since_intent_millis: u128,
    natural_status: Option<NativeStatus>,
    observed_failure: Option<String>,
    observed_failure_text_truncated: bool,
    stdout: OutputArtifact,
    stderr: OutputArtifact,
    launch_evidence: &'static str,
    custody_evidence: &'static str,
    capture_enforcement_evidence: &'static str,
    package_first_qualified: bool,
}

pub(crate) struct RecordedObservation {
    request: ReleaseCommand,
    completion_path: PathBuf,
    natural_status: Option<ExitStatus>,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    artifacts: Vec<RetainedArtifact>,
}

impl RecordedObservation {
    pub(crate) fn request(&self) -> &ReleaseCommand { &self.request }
    pub(crate) fn completion_path(&self) -> &Path { &self.completion_path }
    pub(crate) fn natural_status(&self) -> Option<ExitStatus> { self.natural_status }
    pub(crate) fn stdout(&self) -> &[u8] { &self.stdout }
    pub(crate) fn stderr(&self) -> &[u8] { &self.stderr }

    pub(crate) fn verify_artifacts(&self) -> Result<()> {
        for artifact in &self.artifacts { artifact.verify()?; }
        Ok(())
    }
}

struct RetainedArtifact {
    path: PathBuf,
    bytes: usize,
    sha256: String,
}

impl RetainedArtifact {
    fn verify(&self) -> Result<()> {
        let metadata = fs::symlink_metadata(&self.path)
            .with_context(|| format!("reading release command artifact {}", self.path.display()))?;
        let expected_bytes = u64::try_from(self.bytes).context("command artifact byte count overflow")?;
        if !metadata.file_type().is_file() || metadata.len() != expected_bytes {
            bail!("release command artifact type/length changed: {}", self.path.display());
        }
        let limit = expected_bytes.checked_add(1).context("command artifact read bound overflow")?;
        let mut input = File::open(&self.path)?.take(limit);
        let mut digest = Sha256::new();
        let mut bytes = 0_u64;
        let mut buffer = [0_u8; 8192];
        loop {
            let count = input.read(&mut buffer)?;
            if count == 0 { break; }
            bytes += count as u64;
            digest.update(&buffer[..count]);
        }
        if bytes != expected_bytes || hex(&digest.finalize()) != self.sha256 {
            bail!("release command artifact bytes changed: {}", self.path.display());
        }
        Ok(())
    }
}

impl PendingCommand {
    pub(crate) fn request(&self) -> &ReleaseCommand { &self.request }

    /// No backend report exists in this independent seam. Even a natural zero
    /// status therefore records unknown launch/custody/enforcement evidence.
    pub(crate) fn complete(self, mut observation: CommandObservation) -> Result<RecordedObservation> {
        let persisted = self.persist(&observation);
        if let Some(original) = observation.failure.take() {
            return match persisted {
                Ok(_) => Err(original),
                Err(recording) => Err(original.context(format!(
                    "also failed to retain release command observation: {recording:#}"
                ))),
            };
        }
        let artifacts = persisted?;
        Ok(RecordedObservation {
            request: self.request,
            completion_path: self.directory.join("completion.json"),
            natural_status: observation.natural_status,
            stdout: observation.stdout,
            stderr: observation.stderr,
            artifacts,
        })
    }

    fn persist(&self, observation: &CommandObservation) -> Result<Vec<RetainedArtifact>> {
        let intent = RetainedArtifact {
            path: self.directory.join("intent.json"),
            bytes: self.intent_bytes,
            sha256: self.intent_sha256.clone(),
        };
        intent.verify()?;
        let stdout = self.write_output("stdout.bin", &observation.stdout, self.request.policy.stdout_bytes)?;
        let stderr = self.write_output("stderr.bin", &observation.stderr, self.request.policy.stderr_bytes)?;
        let exceeds_policy = stdout.truncated_by_journal || stderr.truncated_by_journal;
        let mut artifacts = vec![intent, RetainedArtifact {
            path: self.directory.join("stdout.bin"),
            bytes: stdout.retained_bytes,
            sha256: stdout.sha256.clone(),
        }, RetainedArtifact {
            path: self.directory.join("stderr.bin"),
            bytes: stderr.retained_bytes,
            sha256: stderr.sha256.clone(),
        }];
        let failure_text = observation.failure.as_ref().map(bounded_error_text);
        let record = CompletionRecord {
            schema_version: 1,
            kind: "local-release-command",
            state: "observation_recorded",
            id: self.id,
            intent_sha256: self.intent_sha256.clone(),
            recorded_at: Timestamp::now()?,
            elapsed_since_intent_millis: self.intent_time.elapsed().as_millis(),
            natural_status: observation.natural_status.map(NativeStatus::from_status),
            observed_failure: failure_text.as_ref().map(|(text, _)| text.clone()),
            observed_failure_text_truncated: failure_text.as_ref().is_some_and(|(_, truncated)| *truncated),
            stdout,
            stderr,
            launch_evidence: "unknown_pending_backend_report",
            custody_evidence: "unknown_pending_backend_report",
            capture_enforcement_evidence: "unknown_pending_backend_report",
            package_first_qualified: false,
        };
        let raw = json_bytes(&record, self.completion_limit)?;
        write_atomic_new(&self.directory.join("completion.json"), &raw)?;
        artifacts.push(RetainedArtifact {
            path: self.directory.join("completion.json"),
            bytes: raw.len(),
            sha256: sha256(&raw),
        });
        if exceeds_policy {
            bail!("release command observation exceeded its recorded capture policy; bounded raw evidence retained");
        }
        Ok(artifacts)
    }

    fn write_output(&self, name: &str, raw: &[u8], limit: usize) -> Result<OutputArtifact> {
        let retained = &raw[..raw.len().min(limit)];
        write_exclusive(&self.directory.join(name), retained)?;
        Ok(OutputArtifact {
            path: format!("commands/{:08}/{name}", self.id),
            observed_bytes: raw.len(),
            retained_bytes: retained.len(),
            sha256: sha256(retained),
            truncated_by_journal: retained.len() != raw.len(),
        })
    }
}

fn json_bytes(value: &impl Serialize, limit: usize) -> Result<Vec<u8>> {
    struct BoundedBytes { bytes: Vec<u8>, limit: usize }
    impl Write for BoundedBytes {
        fn write(&mut self, data: &[u8]) -> io::Result<usize> {
            if data.len() > self.limit.saturating_sub(self.bytes.len()) {
                return Err(io::Error::other("release command record exceeds its journal byte limit"));
            }
            self.bytes.extend_from_slice(data);
            Ok(data.len())
        }
        fn flush(&mut self) -> io::Result<()> { Ok(()) }
    }
    let mut output = BoundedBytes { bytes: Vec::new(), limit };
    serde_json::to_writer_pretty(&mut output, value).context("serializing release command record")?;
    output.write_all(b"\n").context("terminating release command record")?;
    Ok(output.bytes)
}

fn bounded_error_text(error: &anyhow::Error) -> (String, bool) {
    struct BoundedText { text: String, truncated: bool }
    impl fmt::Write for BoundedText {
        fn write_str(&mut self, text: &str) -> fmt::Result {
            let capacity = 8192_usize.saturating_sub(self.text.len());
            if text.len() <= capacity {
                self.text.push_str(text);
                return Ok(());
            }
            let mut end = capacity;
            while !text.is_char_boundary(end) { end -= 1; }
            self.text.push_str(&text[..end]);
            self.truncated = true;
            Err(fmt::Error)
        }
    }
    let mut output = BoundedText { text: String::new(), truncated: false };
    let _ = fmt::write(&mut output, format_args!("{error:#}"));
    (output.text, output.truncated)
}

fn write_atomic_new(path: &Path, raw: &[u8]) -> Result<()> {
    let parent = path.parent().context("release command artifact has no parent")?;
    let mut pending = tempfile::NamedTempFile::new_in(parent)
        .context("creating pending release command record")?;
    pending.write_all(raw).context("writing pending release command record")?;
    pending.as_file().sync_all().context("flushing pending release command record")?;
    pending.persist_noclobber(path)
        .with_context(|| format!("publishing new release command record {}", path.display()))?;
    Ok(())
}

fn write_exclusive(path: &Path, raw: &[u8]) -> Result<()> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)
        .with_context(|| format!("creating new release command artifact {}", path.display()))?;
    file.write_all(raw).with_context(|| format!("writing release command artifact {}", path.display()))?;
    file.sync_all().with_context(|| format!("flushing release command artifact {}", path.display()))?;
    Ok(())
}

fn sha256(raw: &[u8]) -> String {
    hex(&Sha256::digest(raw))
}

fn hex(raw: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut text = String::with_capacity(raw.len() * 2);
    for &byte in raw {
        text.push(char::from(DIGITS[usize::from(byte >> 4)]));
        text.push(char::from(DIGITS[usize::from(byte & 15)]));
    }
    text
}
