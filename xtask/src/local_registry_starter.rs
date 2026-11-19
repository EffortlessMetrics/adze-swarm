//! Journal the existing starter Cargo captures. This uses the existing
//! synchronous capture behavior: no deadline, retention-in-flight or process
//! custody claim is implied. Install and init need the shared #1066 adapter.

use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail};

use super::CapturedCommand;
use crate::local_release_command::{
    CapturePolicy, CommandJournal, CommandObservation, EnvironmentChange, JournalLimits,
    RecordedObservation, ReleaseCommand, SourceBinding,
};

const PLAN: [(&str, &[&str]); 3] = [
    ("starter-cargo-test", &["test"]),
    (
        "starter-parse-valid",
        &["run", "--example", "parse", "--", "1 + 2 * 3"],
    ),
    (
        "starter-parse-invalid",
        &["run", "--example", "parse", "--", "1 + @"],
    ),
];

fn limits() -> JournalLimits {
    JournalLimits {
        commands: PLAN.len() as u32,
        request_bytes: 64 * 1024,
        completion_bytes: 64 * 1024,
    }
}

fn policy() -> CapturePolicy {
    CapturePolicy {
        stdout_bytes: 4 * 1024 * 1024,
        stderr_bytes: 4 * 1024 * 1024,
        total_millis: 600_000,
        drain_millis: 5_000,
        cleanup_millis: 5_000,
    }
}

pub(super) fn run(
    root: &Path,
    source: SourceBinding,
    cargo: &OsStr,
    cwd: &Path,
    cargo_home: &Path,
    target: &Path,
) -> Result<Vec<CapturedCommand>> {
    let mut journal = CommandJournal::create(root, source, limits())?;
    let env = [("CARGO_HOME", cargo_home), ("CARGO_TARGET_DIR", target)].map(|(name, value)| {
        EnvironmentChange::Set {
            name: name.into(),
            value: value.as_os_str().into(),
            record_value: true,
        }
    });
    let mut observations = Vec::new();
    for (name, args) in PLAN {
        let request = ReleaseCommand::new(
            name.into(),
            cargo.into(),
            args.iter().map(OsString::from).collect(),
            cwd.into(),
            env.to_vec(),
            policy(),
        )?;
        observations.push(capture(&mut journal, request)?);
    }
    let commands = project_plan(root, &observations)?;
    crate::local_release_receipt::validate_starter_proof(&commands)?;
    Ok(commands)
}

fn capture(journal: &mut CommandJournal, request: ReleaseCommand) -> Result<RecordedObservation> {
    let pending = journal.begin(request)?;
    let request = pending.request();
    let mut command = Command::new(request.program());
    command.args(request.arguments()).current_dir(request.cwd());
    for change in request.environment() {
        match change {
            EnvironmentChange::Set { name, value, .. } => {
                command.env(name, value);
            }
            EnvironmentChange::Remove(name) => {
                command.env_remove(name);
            }
        }
    }
    match command.output() {
        Ok(output) => {
            let status = output.status;
            pending
                .complete(CommandObservation::natural(
                    status,
                    output.stdout,
                    output.stderr,
                ))
                .with_context(|| format!("retaining starter process observation ({status})"))
        }
        Err(error) => {
            let cause = anyhow::Error::new(error)
                .context(format!("capturing starter command {}", request.name()));
            pending.complete(CommandObservation::failed(
                cause,
                None,
                Vec::new(),
                Vec::new(),
            ))
        }
    }
}

fn project_plan(root: &Path, observations: &[RecordedObservation]) -> Result<Vec<CapturedCommand>> {
    if !observations
        .iter()
        .map(|obs| obs.request().name())
        .eq(PLAN.iter().map(|(name, _)| *name))
    {
        bail!("starter observations do not match the complete ordered plan");
    }
    let commands = fs::canonicalize(root)?.join("commands");
    // begin() owns a fresh namespace. Reconcile disk, not only the returned
    // handles: an orphan intent or an unrelated entry must not disappear.
    if !fs::symlink_metadata(&commands)?.is_dir() {
        bail!("starter journal is not an owned directory");
    }
    let mut seen = 0;
    for entry in fs::read_dir(&commands)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir()
            || !observations
                .iter()
                .any(|o| o.completion_path().parent() == Some(entry.path().as_path()))
        {
            bail!(
                "unexpected entry in starter journal: {}",
                entry.path().display()
            );
        }
        seen += 1;
    }
    if seen != PLAN.len() {
        bail!("missing starter journal entry");
    }
    for (index, observed) in observations.iter().enumerate() {
        if observed.completion_path()
            != commands
                .join(format!("{:08}", index + 1))
                .join("completion.json")
        {
            bail!("starter completion does not belong to its planned journal slot");
        }
    }
    observations
        .iter()
        .map(|obs| {
            project(obs).with_context(|| {
                format!(
                    "projecting starter command {} from retained evidence {}",
                    obs.request().name(),
                    obs.completion_path().display()
                )
            })
        })
        .collect()
}

fn project(obs: &RecordedObservation) -> Result<CapturedCommand> {
    obs.verify_artifacts()
        .with_context(|| format!("verifying {}", obs.completion_path().display()))?;
    let request = obs.request();
    let status = obs
        .natural_status()
        .context("no natural starter process status")?;
    let exit_code = status.code().with_context(|| {
        format!(
            "starter command {} terminated without an exit code: {status}",
            request.name()
        )
    })?;
    #[cfg(windows)]
    if exit_code < 0 {
        bail!(
            "native Windows status {:#x} is not a portable starter exit code",
            exit_code as u32
        );
    }
    // These UTF-8-only fields are the legacy consumer view. Refuse an
    // unrepresentable value rather than feeding replacement characters into
    // the parser oracle. The raw evidence has already been persisted.
    let argv = std::iter::once(request.program())
        .chain(request.arguments().iter().map(OsString::as_os_str))
        .map(|arg| {
            arg.to_str()
                .context("starter argv is not Unicode")
                .map(str::to_owned)
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(CapturedCommand {
        name: request.name().to_owned(),
        argv,
        exit_code,
        stdout: std::str::from_utf8(obs.stdout())
            .context("starter stdout is not UTF-8")?
            .to_owned(),
        stderr: std::str::from_utf8(obs.stderr())
            .context("starter stderr is not UTF-8")?
            .to_owned(),
    })
}

#[cfg(test)]
#[path = "local_registry_starter_tests.rs"]
mod tests;
