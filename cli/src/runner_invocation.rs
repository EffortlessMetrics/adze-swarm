//! Explicit subprocess inputs for the static runner and its dormant cache.
//!
//! These four call sites inherit the CLI's cwd and environment and apply only
//! ordered environment sets. This is deliberately narrower than `Command`:
//! there is no `env_clear`, removal, raw argument or arbitrary creation option.
//! Linux and Windows use the existing bounded custody implementation. Other
//! platforms retain the existing output path without claiming those bounds.

use anyhow::{Context, Result};
use std::ffi::OsString;
use std::fmt;
use std::path::PathBuf;
use std::process::Output;

/// Marks a process refusal which must never become a cache miss or retry.
///
/// Use this as an anyhow context, rather than wrapping the source error in an
/// opaque type: the CLI must still downcast the concrete platform CaptureError
/// to observe and restore its cancellation exit behavior.
#[derive(Debug)]
pub(crate) struct ProcessFailure;

impl fmt::Display for ProcessFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("static runner process boundary failed")
    }
}

/// Explicit data; no subprocess configuration is recovered from Command getters.
pub(crate) struct RunnerInvocation {
    pub(crate) program: OsString,
    pub(crate) args: Vec<OsString>,
    pub(crate) environment: Vec<(OsString, OsString)>,
}

/// The real CLI supplies its actual executable. Integration tests supply the
/// exact Cargo-built CLI artifact; no environment-controlled bootstrap exists.
pub(crate) struct ProcessRunner {
    #[cfg(windows)]
    bootstrap: PathBuf,
}

impl ProcessRunner {
    pub(crate) fn new(bootstrap: PathBuf) -> Self {
        #[cfg(windows)]
        {
            Self { bootstrap }
        }
        #[cfg(not(windows))]
        {
            let _ = bootstrap;
            Self {}
        }
    }

    pub(crate) fn capture(&self, invocation: &RunnerInvocation) -> Result<Output> {
        self.capture_inner(invocation).context(ProcessFailure)
    }

    fn capture_inner(&self, invocation: &RunnerInvocation) -> Result<Output> {
        #[cfg(windows)]
        {
            use crate::runner_windows_request::{
                EnvironmentChange, EnvironmentPolicy, RunnerRequest,
            };

            let request = RunnerRequest {
                program: invocation.program.clone(),
                args: invocation.args.clone(),
                environment: EnvironmentPolicy::Inherit,
                changes: invocation
                    .environment
                    .iter()
                    .map(|(key, value)| EnvironmentChange::Set(key.clone(), value.clone()))
                    .collect(),
                // Preserve Cargo's inherited cwd/config discovery.
                cwd: None,
            };
            Ok(crate::runner_windows::capture(
                &request,
                &self.bootstrap,
                crate::runner_windows::CARGO_RUN_POLICY,
            )?)
        }
        #[cfg(not(windows))]
        {
            let mut command = std::process::Command::new(&invocation.program);
            command.args(&invocation.args);
            for (key, value) in &invocation.environment {
                command.env(key, value);
            }
            #[cfg(target_os = "linux")]
            {
                Ok(crate::runner_process::capture(
                    &mut command,
                    crate::runner_process::CARGO_RUN_POLICY,
                )?)
            }
            #[cfg(not(target_os = "linux"))]
            {
                Ok(command.output()?)
            }
        }
    }
}
