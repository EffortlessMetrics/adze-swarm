//! Journal the existing starter Cargo captures. This uses the existing
//! synchronous capture behavior: no deadline, retention-in-flight or process
//! custody claim is implied. Install and init need the shared #1066 adapter.

use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail};

use super::CapturedCommand;
use super::starter_inputs::StarterInputs;
use crate::local_release_command::{
    CapturePolicy, CommandJournal, CommandObservation, EnvironmentChange, JournalLimits,
    RecordedObservation, ReleaseCommand,
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
    inputs: &mut StarterInputs,
    cargo: &OsStr,
    cargo_home: &Path,
    target: &Path,
) -> Result<Vec<CapturedCommand>> {
    let before = inputs.observe(None, "not_started");
    if inputs.changed || before.is_err() {
        let reason = if inputs.changed {
            "starter inputs changed before the first command"
        } else {
            "starter input preflight unavailable"
        };
        let all = PLAN.map(|(name, _)| name);
        let recorded = with_evidence(before, inputs.stopped(reason, &all));
        return with_evidence(
            Err(anyhow::anyhow!("{reason}; no starter command started")),
            recorded,
        );
    }
    let result = run_commands(inputs, cargo, cargo_home, target);
    let verified = inputs.verify(result.is_ok());
    with_evidence(result, verified)
}

fn with_evidence<T>(result: Result<T>, evidence: Result<()>) -> Result<T> {
    match (result, evidence) {
        (Ok(value), Ok(())) => Ok(value),
        (Err(error), Ok(())) | (Ok(_), Err(error)) => Err(error),
        (Err(original), Err(secondary)) => Err(original.context(format!(
            "also failed to retain/verify starter inputs: {secondary:#}"
        ))),
    }
}

fn run_commands(
    inputs: &mut StarterInputs,
    cargo: &OsStr,
    cargo_home: &Path,
    target: &Path,
) -> Result<Vec<CapturedCommand>> {
    let mut journal = CommandJournal::create(&inputs.root, inputs.source.clone(), limits())?;
    let env = [("CARGO_HOME", cargo_home), ("CARGO_TARGET_DIR", target)].map(|(name, value)| {
        EnvironmentChange::Set {
            name: name.into(),
            value: value.as_os_str().into(),
            record_value: true,
        }
    });
    let mut observations = Vec::new();
    let mut snapshots = Ok(());
    for (index, (name, args)) in PLAN.into_iter().enumerate() {
        let request = ReleaseCommand::new(
            name.into(),
            cargo.into(),
            args.iter().map(OsString::from).collect(),
            inputs.project.clone(),
            env.to_vec(),
            policy(),
        )?;
        let observation = capture(&mut journal, request);
        let outcome = match &observation {
            Ok(obs) if obs.natural_status().is_some_and(|s| s.success()) => "status_success",
            Ok(_) => "status_unsuccessful",
            Err(_) => "capture_or_recording_error",
        };
        let evidence = inputs.observe(Some((index + 1, name)), outcome);
        snapshots = with_evidence(snapshots, evidence);
        match observation {
            Err(original) => {
                let remaining: Vec<_> = PLAN[index + 1..].iter().map(|(name, _)| *name).collect();
                let reason = if inputs.changed {
                    "command capture or recording failed; input change observed"
                } else {
                    "command capture or recording failed"
                };
                let stopped = inputs.stopped(reason, &remaining);
                return with_evidence(Err(original), with_evidence(snapshots, stopped));
            }
            Ok(observed) => observations.push(observed),
        }

        // Confirmed drift or unreadable inputs stop further launches. A
        // failed sidecar write after stable inputs were observed still vetoes
        // the final result, without hiding an existing Cargo diagnostic.
        if inputs.changed || !inputs.inputs_read {
            let reason = if inputs.changed {
                "starter inputs changed"
            } else {
                "starter input observation unavailable"
            };
            let remaining: Vec<_> = PLAN[index + 1..].iter().map(|(name, _)| *name).collect();
            let stopped = inputs.stopped(reason, &remaining);
            let actual = observations
                .iter()
                .map(|obs| {
                    let diagnostic = std::str::from_utf8(obs.stderr())
                        .ok()
                        .unwrap_or("[see binary stderr]");
                    format!(
                        "{} ({}); {}: {}",
                        obs.request().name(),
                        obs.natural_status()
                            .map_or_else(|| "no natural status".into(), |s| s.to_string()),
                        obs.completion_path().display(),
                        diagnostic.chars().take(400).collect::<String>()
                    )
                })
                .collect::<Vec<_>>()
                .join("; ");
            return with_evidence(
                Err(anyhow::anyhow!(
                    "{reason} after {name}; not run: {remaining:?}; recorded: {actual}"
                )),
                with_evidence(snapshots, stopped),
            );
        }
    }
    let journey = project_plan(&inputs.root, &observations).and_then(|commands| {
        crate::local_release_receipt::validate_starter_proof(&commands)?;
        Ok(commands)
    });
    with_evidence(journey, snapshots)
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
