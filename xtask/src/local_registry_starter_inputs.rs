//! File observations around the existing starter commands. They are not proof
//! of lock consumption, offline closure, or a qualified release.
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail, ensure};
use serde::Serialize;

use crate::local_release_command::{SourceBinding, sha256, write_atomic_new};

const MANIFEST_LIMIT: usize = 1024 * 1024;
const LOCK_LIMIT: usize = 4 * 1024 * 1024;

#[derive(Clone, Serialize)]
struct Artifact {
    path: String, // relative to starter-inputs/
    bytes: usize,
    sha256: String,
}

pub(super) struct StarterInputs {
    pub(super) root: PathBuf,
    pub(super) project: PathBuf,
    pub(super) source: SourceBinding,
    directory: PathBuf,
    retained: Vec<Artifact>,
    effective: Option<Artifact>,
    lock: Option<String>,
    pub(super) changed: bool,
    pub(super) inputs_read: bool,
    last_lock_present: bool,
}

// These are the only two source files the harness observes. No directory
// traversal, Cargo-home import, configuration or environment is collected.
fn read_regular(path: &Path, limit: usize) -> Result<Option<Vec<u8>>> {
    let meta = match fs::symlink_metadata(path) {
        Ok(meta) => meta,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    ensure!(
        meta.is_file() && meta.len() <= limit as u64,
        "expected a bounded regular file: {}",
        path.display()
    );
    let mut bytes = Vec::new();
    File::open(path)?
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() <= limit && bytes.len() as u64 == meta.len(),
        "starter input length changed: {}",
        path.display()
    );
    Ok(Some(bytes))
}

impl StarterInputs {
    pub(super) fn create(
        root: &Path,
        source: SourceBinding,
        project: &Path,
    ) -> Result<(Self, String)> {
        ensure!(
            fs::symlink_metadata(project)?.is_dir(),
            "expected an owned starter directory"
        );
        let project = fs::canonicalize(project)?;
        let root = fs::canonicalize(root)?;
        let directory = root.join("starter-inputs");
        fs::create_dir(&directory)
            .context("starter inputs directory must be new and exclusively owned")?;
        let mut this = Self {
            root,
            project,
            source,
            directory,
            retained: Vec::new(),
            effective: None,
            lock: None,
            changed: false,
            inputs_read: false,
            last_lock_present: false,
        };
        let manifest = this.manifest()?;
        let text =
            String::from_utf8(manifest).context("generated starter manifest is not UTF-8")?;
        let generated = this.store("generated-Cargo.toml".into(), text.as_bytes())?;
        let lock = this.observe_lock(true)?; // establish the generated-state baseline
        this.record(
            "generated.json",
            serde_json::json!({
                "stage": "after_installed_cli_init_before_registry_pinning",
                "manifest": generated,
                "lock": lock,
            }),
        )?;
        Ok((this, text))
    }

    /// Seal the exact manifest accepted by the existing registry-isolation
    /// checks; a later observation cannot substitute a different baseline.
    pub(super) fn expect_effective(&mut self, prepared: &[u8]) -> Result<()> {
        ensure!(
            self.effective.is_none(),
            "effective starter manifest already bound"
        );
        ensure!(
            self.manifest()? == prepared,
            "starter manifest differs from the prepared bytes"
        );
        self.effective = Some(self.store("effective-Cargo.toml".into(), prepared)?);
        Ok(())
    }

    fn manifest(&self) -> Result<Vec<u8>> {
        read_regular(&self.project.join("Cargo.toml"), MANIFEST_LIMIT)?
            .context("generated starter Cargo.toml is absent")
    }

    fn store(&mut self, name: String, raw: &[u8]) -> Result<Artifact> {
        let artifact = Artifact {
            path: name,
            bytes: raw.len(),
            sha256: sha256(raw),
        };
        if !self
            .retained
            .iter()
            .any(|old| old.path == artifact.path && old.sha256 == artifact.sha256)
        {
            write_atomic_new(&self.directory.join(&artifact.path), raw)?;
            self.retained.push(artifact.clone());
        }
        Ok(artifact)
    }

    fn record(&mut self, name: &str, mut observation: serde_json::Value) -> Result<()> {
        observation["schema_version"] = 1.into();
        observation["source"] = serde_json::to_value(&self.source)?;
        observation["kind"] = "local-release-starter-input-observation".into();
        observation["package_first_qualified"] = false.into();
        observation["lock_consumption"] = "not_proved".into();
        let bytes = serde_json::to_vec_pretty(&observation)?;
        self.store(name.to_string(), &bytes)?;
        Ok(())
    }

    fn observe_lock(&mut self, may_first_appear: bool) -> Result<serde_json::Value> {
        let observed_lock = read_regular(&self.project.join("Cargo.lock"), LOCK_LIMIT)?;
        self.last_lock_present = observed_lock.is_some();
        Ok(if let Some(raw) = observed_lock {
            let digest = sha256(&raw);
            if let Some(previous) = &self.lock {
                self.changed |= previous != &digest;
            } else {
                self.changed |= !may_first_appear;
                self.lock = Some(digest.clone());
            }
            let artifact = self.store(format!("lock-{digest}.lock"), &raw)?;
            serde_json::json!({"state":"present","artifact":artifact})
        } else {
            self.changed |= self.lock.is_some();
            serde_json::json!({"state":"absent"})
        })
    }

    pub(super) fn observe(&mut self, slot: Option<(usize, &str)>, outcome: &str) -> Result<()> {
        self.inputs_read = false;
        ensure!(
            fs::symlink_metadata(&self.project)?.is_dir(),
            "starter directory is no longer a regular directory"
        );
        ensure!(
            fs::symlink_metadata(&self.directory)?.is_dir(),
            "starter inputs namespace is no longer a regular directory"
        );
        let expected = self
            .effective
            .as_ref()
            .context("effective starter manifest was not bound")?
            .clone();
        let observed = read_regular(&self.project.join("Cargo.toml"), MANIFEST_LIMIT)?;
        let observed_sha = observed.as_deref().map(sha256);
        let manifest_matches = observed_sha.as_ref() == Some(&expected.sha256);
        self.changed |= !manifest_matches;
        // Between init and the first Cargo invocation even an absent lock is
        // a bound fact. A first lock may be observed after a natural status,
        // not silently adopted before any command or on an unknown attempt.
        let may_first_appear =
            slot.is_some() && matches!(outcome, "status_success" | "status_unsuccessful");
        let lock = self.observe_lock(may_first_appear)?;
        self.inputs_read = true;
        let label = slot.map_or_else(
            || "before.json".into(),
            |(id, _)| format!("after-{id:02}.json"),
        );
        self.record(
            &label,
            serde_json::json!({
                "stage": if slot.is_some() { "after_command_attempt" } else { "before_first_command" },
                "journal_slot": slot.map(|s| s.0), "command": slot.map(|s| s.1),
                "command_outcome": outcome,
                "effective_manifest": expected,
                "observed_manifest_sha256": observed_sha,
                "effective_manifest_matches": manifest_matches,
                "lock": lock,
                "known_input_change": self.changed
            }),
        )
    }

    pub(super) fn stopped(&mut self, reason: &str, not_run: &[&str]) -> Result<()> {
        self.record(
            "stopped.json",
            serde_json::json!({
                "stage":"stopped", "reason":reason, "not_run":not_run
            }),
        )
    }

    pub(super) fn verify(&self, journey_passed: bool) -> Result<()> {
        ensure!(
            fs::symlink_metadata(&self.directory)?.is_dir(),
            "starter inputs namespace changed"
        );
        for saved in &self.retained {
            let bytes = read_regular(&self.directory.join(&saved.path), saved.bytes)?
                .with_context(|| format!("retained starter input missing: {}", saved.path))?;
            ensure!(
                bytes.len() == saved.bytes && sha256(&bytes) == saved.sha256,
                "retained starter input changed: {}",
                saved.path
            );
        }
        if journey_passed && self.changed {
            bail!("effective starter manifest or observed lock changed during the journey");
        }
        ensure!(
            !journey_passed || self.last_lock_present,
            "successful starter journey lacks an observed Cargo.lock"
        );
        Ok(())
    }
}
