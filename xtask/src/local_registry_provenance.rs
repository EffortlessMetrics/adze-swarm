//! Durable source/archive/index evidence for the local release harness (#1065).
//!
//! This is deliberately not an install-resolution or binary receipt. The
//! record remains unqualified even if the existing installed starter succeeds.

use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};
use serde::Serialize;
use tempfile::TempDir;

use super::sha256_hex_file;

const RECEIPT: &str = "archive-provenance.json";

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct Artifact {
    path: String,
    bytes: u64,
    sha256: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct FileIdentity {
    bytes: u64,
    sha256: String,
    unix_mode: Option<u32>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct Gitlink {
    path: String,
    commit: String,
    disposition: &'static str,
}

#[derive(Debug, Serialize)]
struct Source {
    commit: String,
    tree: String,
    clean_tracked_and_untracked: bool,
    ignored_files: &'static str,
    gitlinks: Vec<Gitlink>,
    gitlinks_outside_selected_packages: bool,
    selected_package_roots: BTreeMap<String, String>,
    archive: Artifact,
    inventory: Artifact,
    checked_after_packaging: bool,
}

#[derive(Debug, Serialize)]
struct MemberChange {
    member: String,
    before_sha256: String,
    after_sha256: String,
    reason: &'static str,
}

#[derive(Debug, Serialize)]
struct Archive {
    name: String,
    version: String,
    state: &'static str,
    transformation: &'static str,
    inventory_scope: &'static str,
    original: Option<Artifact>,
    served: Option<Artifact>,
    original_manifest: Option<Artifact>,
    served_manifest: Option<Artifact>,
    original_members: Option<Artifact>,
    served_members: Option<Artifact>,
    changes: Vec<MemberChange>,
    cargo_metadata: Option<Artifact>,
    cargo_metadata_stderr: Option<Artifact>,
    cargo_metadata_exit_code: Option<i32>,
    sparse_index: Option<Artifact>,
    metadata_claim: &'static str,
    lock_claim: &'static str,
}

#[derive(Debug, Serialize)]
struct Receipt {
    schema_version: &'static str,
    kind: &'static str,
    package_first_qualified: bool,
    harness_status: &'static str,
    stage: String,
    error: Option<String>,
    selected_version: String,
    release_graph_authority: &'static str,
    ordered_crates: Vec<String>,
    archive_preparation_complete: bool,
    source: Option<Source>,
    archives: Vec<Archive>,
    pending_claims: [&'static str; 5],
}

pub(super) struct Provenance {
    root: PathBuf,
    source_root: PathBuf,
    snapshot_inventory: BTreeMap<String, FileIdentity>,
    receipt: Receipt,
}

pub(super) struct PreparedArchive {
    pub extracted: TempDir,
    pub manifest_path: PathBuf,
    pub download: PathBuf,
    pub checksum: String,
    pub slot: usize,
    inventory: BTreeMap<String, FileIdentity>,
}

impl PreparedArchive {
    pub(super) fn verify_extraction(&self) -> Result<()> {
        let root = self
            .manifest_path
            .parent()
            .context("served manifest has no parent")?;
        if file_inventory(root)? != self.inventory {
            bail!("served extraction changed before metadata could be bound to its archive");
        }
        Ok(())
    }
}

impl Provenance {
    pub(super) fn create(
        source_root: &Path,
        artifacts_out: Option<&Path>,
        version: &str,
        ordered_crates: &[String],
    ) -> Result<Self> {
        let source_root = fs::canonicalize(source_root).context("canonicalizing source root")?;
        let root = if let Some(requested) = artifacts_out {
            let absolute = if requested.is_absolute() {
                requested.to_path_buf()
            } else {
                std::env::current_dir()?.join(requested)
            };
            let parent =
                fs::canonicalize(absolute.parent().context("artifact path has no parent")?)
                    .context("artifact parent must already exist")?;
            let name = absolute
                .file_name()
                .context("artifact path has no directory name")?;
            let root = parent.join(name);
            if root.starts_with(&source_root) {
                bail!("release artifacts must be outside the source workspace");
            }
            fs::create_dir(&root)
                .context("artifact directory must be new and exclusively owned")?;
            root
        } else {
            let owned = tempfile::Builder::new()
                .prefix("adze-release-artifacts-")
                .tempdir()
                .context("creating persistent release artifact directory")?;
            if fs::canonicalize(owned.path())?.starts_with(&source_root) {
                bail!("temporary release artifacts must be outside the source workspace");
            }
            owned.keep()
        };
        if root.starts_with(&source_root) {
            bail!("temporary release artifact directory must be outside the source workspace");
        }
        let result = Self {
            root,
            source_root,
            snapshot_inventory: BTreeMap::new(),
            receipt: Receipt {
                schema_version: "1",
                kind: "local-release-archive-preparation",
                package_first_qualified: false,
                harness_status: "in_progress",
                stage: "source_preflight".into(),
                error: None,
                selected_version: version.into(),
                release_graph_authority: crate::release_graph::ARTIFACT_PATH,
                ordered_crates: ordered_crates.to_vec(),
                archive_preparation_complete: false,
                source: None,
                archives: Vec::new(),
                pending_claims: [
                    "transformed packaged lock and actual resolved install",
                    "complete executed command, capture, toolchain and environment provenance",
                    "installed executable identity and subsequent tamper checks",
                    "declared packaged target/test-source closure",
                    "same-source Linux/Windows package-first qualification",
                ],
            },
        };
        println!("persistent release artifacts: {}", result.root.display());
        result.checkpoint()?;
        Ok(result)
    }

    pub(super) fn root(&self) -> &Path {
        &self.root
    }

    pub(super) fn source_commit(&self) -> Result<&str> {
        Ok(&self
            .receipt
            .source
            .as_ref()
            .context("source not captured")?
            .commit)
    }

    pub(super) fn stage(&mut self, stage: impl Into<String>) -> Result<()> {
        self.receipt.stage = stage.into();
        self.checkpoint()
    }

    pub(super) fn failed(&mut self, error: &anyhow::Error) -> Result<()> {
        self.receipt.harness_status = "failed";
        self.receipt.error = Some(format!("{error:#}"));
        self.checkpoint()
    }

    pub(super) fn finished(&mut self) -> Result<()> {
        self.receipt.harness_status = "completed_unqualified";
        self.checkpoint()
    }

    fn checkpoint(&self) -> Result<()> {
        let mut pending = tempfile::NamedTempFile::new_in(&self.root)
            .context("creating archive provenance checkpoint")?;
        serde_json::to_writer_pretty(&mut pending, &self.receipt)?;
        pending.write_all(b"\n")?;
        pending.as_file().sync_all()?;
        pending
            .persist(self.root.join(RECEIPT))
            .context("committing archive provenance checkpoint")?;
        Ok(())
    }

    fn artifact(&self, path: &Path) -> Result<Artifact> {
        Ok(Artifact {
            path: relative_name(path, &self.root)?,
            bytes: fs::metadata(path)?.len(),
            sha256: sha256_hex_file(path)?,
        })
    }

    fn write_json(&self, relative: &str, value: &impl Serialize) -> Result<Artifact> {
        let path = self.root.join(relative);
        fs::create_dir_all(path.parent().context("missing artifact parent")?)?;
        let mut file = File::create(&path)?;
        serde_json::to_writer_pretty(&mut file, value)?;
        file.write_all(b"\n")?;
        self.artifact(&path)
    }

    pub(super) fn snapshot(&mut self) -> Result<PathBuf> {
        let (commit, tree) = clean_source_identity(&self.source_root)?;
        let gitlinks = source_gitlinks(&self.source_root, &commit)?;
        self.stage("source_snapshot")?;
        let archive_path = self.root.join("source.tar");
        let status = Command::new("git")
            .env("GIT_NO_REPLACE_OBJECTS", "1")
            .args(["archive", "--format=tar", "--output"])
            .arg(&archive_path)
            .arg(&commit)
            .current_dir(&self.source_root)
            .status()
            .context("exporting exact source commit")?;
        if !status.success() {
            bail!("git archive failed with status {status}");
        }
        let snapshot = self.root.join("source");
        fs::create_dir(&snapshot)?;
        run_archive_helper("extract-source", &archive_path, Some(&snapshot), None)?;
        self.snapshot_inventory = file_inventory(&snapshot)?;
        let inventory = self.write_json("source-files.json", &self.snapshot_inventory)?;
        self.receipt.source = Some(Source {
            commit,
            tree,
            clean_tracked_and_untracked: true,
            ignored_files: "excluded by git archive; no working-tree files copied",
            gitlinks,
            gitlinks_outside_selected_packages: false,
            selected_package_roots: BTreeMap::new(),
            archive: self.artifact(&archive_path)?,
            inventory,
            checked_after_packaging: false,
        });
        self.checkpoint()?;
        Ok(snapshot)
    }

    pub(super) fn check_package_roots(
        &mut self,
        snapshot: &Path,
        package_roots: &BTreeMap<String, PathBuf>,
    ) -> Result<()> {
        let source = self
            .receipt
            .source
            .as_mut()
            .context("source not captured")?;
        source.gitlinks_outside_selected_packages = false;
        source.selected_package_roots.clear();
        let snapshot = fs::canonicalize(snapshot)?;
        for (name, root) in package_roots {
            let root = fs::canonicalize(root)?;
            let relative = root.strip_prefix(&snapshot).with_context(|| {
                format!("selected package `{name}` is outside the source snapshot")
            })?;
            for gitlink in &source.gitlinks {
                let path = Path::new(&gitlink.path);
                if path.starts_with(relative) || relative.starts_with(path) {
                    bail!(
                        "selected package `{name}` intersects unexported gitlink `{}`",
                        gitlink.path
                    );
                }
            }
            source
                .selected_package_roots
                .insert(name.clone(), path_text(relative)?);
        }
        source.gitlinks_outside_selected_packages = true;
        self.checkpoint()
    }

    pub(super) fn verify_source(&mut self, snapshot: &Path) -> Result<()> {
        let (commit, tree) = clean_source_identity(&self.source_root)?;
        let source = self
            .receipt
            .source
            .as_ref()
            .context("source not captured")?;
        if (commit.as_str(), tree.as_str()) != (source.commit.as_str(), source.tree.as_str()) {
            bail!("source commit/tree changed during archive preparation");
        }
        if file_inventory(snapshot)? != self.snapshot_inventory {
            bail!("owned source snapshot changed during archive preparation");
        }
        if self.artifact(&self.root.join(&source.archive.path))? != source.archive {
            bail!("preserved source archive changed during preparation");
        }
        self.receipt
            .source
            .as_mut()
            .expect("checked source")
            .checked_after_packaging = true;
        self.checkpoint()
    }

    pub(super) fn prepare_archive(
        &mut self,
        name: &str,
        version: &str,
        packaged: &Path,
        transform: impl FnOnce(&Path) -> Result<()>,
    ) -> Result<PreparedArchive> {
        let archive_root = format!("{name}-{version}");
        single_component(&archive_root)?;
        let slot = self.receipt.archives.len();
        self.receipt.archives.push(Archive {
            name: name.into(), version: version.into(), state: "preserving_original",
            transformation: "existing exact-version local-registry manifest rewrite; tar/gzip repack if changed",
            inventory_scope: "regular-file contents and Unix modes on Unix; container metadata is not compared",
            original: None, served: None, original_manifest: None, served_manifest: None,
            original_members: None, served_members: None, changes: Vec::new(),
            cargo_metadata: None, cargo_metadata_stderr: None,
            cargo_metadata_exit_code: None, sparse_index: None,
            metadata_claim: "not yet interpreted from the served manifest",
            lock_claim: "preserved archive contents only; no resolution or locked-install claim",
        });
        self.stage(format!("archive:{name}:preserve"))?;
        let original = self
            .root
            .join("original")
            .join(format!("{archive_root}.crate"));
        copy_new(packaged, &original)?;
        self.receipt.archives[slot].original = Some(self.artifact(&original)?);
        self.checkpoint()?;
        let before = extract_archive(&original, &archive_root)?;
        let before_root = before.path().join(&archive_root);
        let before_files = file_inventory(&before_root)?;
        let original_members = self.write_json(
            &format!("metadata/{archive_root}/original-files.json"),
            &before_files,
        )?;
        let original_manifest_path = self
            .root
            .join(format!("metadata/{archive_root}/Cargo.toml.original"));
        copy_new(&before_root.join("Cargo.toml"), &original_manifest_path)?;
        let original_manifest = self.artifact(&original_manifest_path)?;
        self.receipt.archives[slot].original_members = Some(original_members);
        self.receipt.archives[slot].original_manifest = Some(original_manifest);
        self.receipt.archives[slot].state = "transforming_copy";
        self.stage(format!("archive:{name}:transform"))?;
        let download = self
            .root
            .join("registry/crates")
            .join(name)
            .join(version)
            .join("download");
        copy_new(&original, &download)?;
        transform(&download)?;
        let extracted = extract_archive(&download, &archive_root)?;
        let served_root = extracted.path().join(&archive_root);
        let after_files = file_inventory(&served_root)?;
        let changes = manifest_only_changes(&before_files, &after_files)?;
        let served_members = self.write_json(
            &format!("metadata/{archive_root}/served-files.json"),
            &after_files,
        )?;
        let served_manifest_path = self
            .root
            .join(format!("metadata/{archive_root}/Cargo.toml.served"));
        copy_new(&served_root.join("Cargo.toml"), &served_manifest_path)?;
        let served_manifest = self.artifact(&served_manifest_path)?;
        let served = self.artifact(&download)?;
        let checksum = served.sha256.clone();
        let record = &mut self.receipt.archives[slot];
        record.served_members = Some(served_members);
        record.served_manifest = Some(served_manifest);
        record.served = Some(served);
        record.changes = changes;
        record.state = "transformed_members_verified";
        self.checkpoint()?;
        self.verify_archive_artifacts()?;
        Ok(PreparedArchive {
            manifest_path: served_root.join("Cargo.toml"),
            extracted,
            download,
            checksum,
            slot,
            inventory: after_files,
        })
    }

    pub(super) fn record_metadata(
        &mut self,
        slot: usize,
        output: &std::process::Output,
    ) -> Result<()> {
        let archive = &self.receipt.archives[slot];
        let path = self.root.join(format!(
            "metadata/{}-{}/cargo-metadata.json",
            archive.name, archive.version
        ));
        fs::write(&path, &output.stdout)?;
        self.receipt.archives[slot].cargo_metadata = Some(self.artifact(&path)?);
        let stderr_path = path.with_extension("stderr");
        fs::write(&stderr_path, &output.stderr)?;
        self.receipt.archives[slot].cargo_metadata_stderr = Some(self.artifact(&stderr_path)?);
        self.receipt.archives[slot].cargo_metadata_exit_code = output.status.code();
        self.checkpoint()
    }

    pub(super) fn metadata_validated(&mut self, slot: usize) -> Result<()> {
        self.receipt.archives[slot].metadata_claim = "actual cargo metadata --no-deps: exact served root manifest interpretation; resolve absent";
        self.checkpoint()
    }

    pub(super) fn record_index(&mut self, slot: usize, index: &Path) -> Result<()> {
        self.receipt.archives[slot].sparse_index = Some(self.artifact(index)?);
        self.receipt.archives[slot].state = "indexed_from_served_manifest";
        self.checkpoint()
    }

    pub(super) fn verify_archive_artifacts(&self) -> Result<()> {
        if let Some(source) = &self.receipt.source {
            for recorded in [&source.archive, &source.inventory] {
                if self.artifact(&self.root.join(&recorded.path))? != *recorded {
                    bail!("preserved source artifact changed: {}", recorded.path);
                }
            }
        }
        for archive in &self.receipt.archives {
            for recorded in [
                &archive.original,
                &archive.served,
                &archive.original_manifest,
                &archive.served_manifest,
                &archive.original_members,
                &archive.served_members,
                &archive.cargo_metadata,
                &archive.cargo_metadata_stderr,
                &archive.sparse_index,
            ]
            .into_iter()
            .flatten()
            {
                if self.artifact(&self.root.join(&recorded.path))? != *recorded {
                    bail!("preserved release artifact changed: {}", recorded.path);
                }
            }
        }
        Ok(())
    }

    pub(super) fn archives_prepared(&mut self) -> Result<()> {
        if self.receipt.archives.len() != self.receipt.ordered_crates.len()
            || self
                .receipt
                .archives
                .iter()
                .any(|a| a.state != "indexed_from_served_manifest")
        {
            bail!("archive preparation did not cover the complete selected release graph");
        }
        self.verify_archive_artifacts()?;
        self.receipt.archive_preparation_complete = true;
        self.stage("archives_prepared")
    }
}

fn path_text(path: &Path) -> Result<String> {
    Ok(path
        .to_str()
        .context("release evidence requires UTF-8 path names")?
        .replace('\\', "/"))
}

fn relative_name(path: &Path, root: &Path) -> Result<String> {
    path_text(
        path.strip_prefix(root)
            .context("artifact is outside owned evidence root")?,
    )
}

fn single_component(name: &str) -> Result<()> {
    let mut components = Path::new(name).components();
    if !matches!(components.next(), Some(Component::Normal(_)))
        || components.next().is_some()
        || name.contains('\\')
    {
        bail!("invalid release artifact component `{name}`");
    }
    Ok(())
}

fn copy_new(source: &Path, dest: &Path) -> Result<()> {
    fs::create_dir_all(dest.parent().context("copy destination has no parent")?)?;
    let before = sha256_hex_file(source)?;
    let mut input = File::open(source)?;
    let mut output = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(dest)?;
    std::io::copy(&mut input, &mut output)?;
    output.sync_all()?;
    if sha256_hex_file(source)? != before || sha256_hex_file(dest)? != before {
        bail!("archive changed while preserving {}", source.display());
    }
    Ok(())
}

fn git_output(root: &Path, args: &[&str]) -> Result<Vec<u8>> {
    let output = Command::new("git")
        .env("GIT_NO_REPLACE_OBJECTS", "1")
        .args(args)
        .current_dir(root)
        .output()?;
    if !output.status.success() {
        bail!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(output.stdout)
}

fn git_text(root: &Path, args: &[&str]) -> Result<String> {
    Ok(String::from_utf8(git_output(root, args)?)?
        .trim()
        .to_string())
}

fn clean_source_identity(root: &Path) -> Result<(String, String)> {
    let top = git_text(root, &["rev-parse", "--show-toplevel"])?;
    if fs::canonicalize(top)? != fs::canonicalize(root)? {
        bail!("source root must be the Git working-tree root");
    }
    let status = git_output(
        root,
        &[
            "status",
            "--porcelain=v1",
            "-z",
            "--untracked-files=all",
            "--ignore-submodules=none",
        ],
    )?;
    if !status.is_empty() {
        bail!("release qualification requires clean tracked, submodule and untracked source state");
    }
    Ok((
        git_text(root, &["rev-parse", "HEAD"])?,
        git_text(root, &["rev-parse", "HEAD^{tree}"])?,
    ))
}

fn source_gitlinks(root: &Path, commit: &str) -> Result<Vec<Gitlink>> {
    let raw = git_output(root, &["ls-tree", "-r", "-z", commit])?;
    let mut result = Vec::new();
    for entry in raw
        .split(|byte| *byte == 0)
        .filter(|entry| !entry.is_empty())
    {
        let entry = std::str::from_utf8(entry).context("non-UTF-8 Git tree path")?;
        let (header, path) = entry
            .split_once('\t')
            .context("invalid git ls-tree record")?;
        let fields: Vec<_> = header.split_whitespace().collect();
        if fields.len() != 3 {
            bail!("invalid git ls-tree header");
        }
        if fields[0] == "160000" {
            result.push(Gitlink {
                path: path.into(),
                commit: fields[2].into(),
                disposition: "not recursively exported; must be outside selected package roots",
            });
        }
    }
    Ok(result)
}

fn file_inventory(root: &Path) -> Result<BTreeMap<String, FileIdentity>> {
    let mut result = BTreeMap::new();
    for entry in walkdir::WalkDir::new(root).follow_links(false) {
        let entry = entry?;
        if entry.file_type().is_dir() {
            continue;
        }
        if !entry.file_type().is_file() {
            bail!(
                "release snapshot/archive contains unsupported non-file entry {}",
                entry.path().display()
            );
        }
        let metadata = entry.metadata()?;
        #[cfg(unix)]
        let unix_mode = {
            use std::os::unix::fs::PermissionsExt;
            Some(metadata.permissions().mode() & 0o7777)
        };
        #[cfg(not(unix))]
        let unix_mode = None;
        result.insert(
            relative_name(entry.path(), root)?,
            FileIdentity {
                bytes: metadata.len(),
                sha256: sha256_hex_file(entry.path())?,
                unix_mode,
            },
        );
    }
    Ok(result)
}

const ARCHIVE_HELPER: &str = include_str!("local_release_archive.py");

fn run_archive_helper(
    operation: &str,
    archive: &Path,
    destination: Option<&Path>,
    expected_root: Option<&str>,
) -> Result<std::process::Output> {
    for interpreter in ["python3", "python"] {
        let mut command = Command::new(interpreter);
        command
            .args(["-c", ARCHIVE_HELPER, operation])
            .arg(archive)
            .env("PYTHONUTF8", "1");
        if let Some(destination) = destination {
            command.arg(destination);
        }
        if let Some(expected_root) = expected_root {
            command.arg(expected_root);
        }
        match command.output() {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error).context("starting release archive validator"),
            Ok(output) => {
                if !output.status.success() {
                    bail!(
                        "release archive helper {operation} failed with status {}: {}",
                        output.status,
                        String::from_utf8_lossy(&output.stderr)
                    );
                }
                return Ok(output);
            }
        }
    }
    bail!("release archive validation requires Python 3.8+ (python3 or python)")
}

pub(super) fn archive_manifest_member(archive: &Path) -> Result<String> {
    let output = run_archive_helper("inspect-package", archive, None, None)?;
    let manifest = String::from_utf8(output.stdout).context("non-UTF-8 archive helper output")?;
    let manifest = manifest.trim_end_matches('\n');
    if manifest.is_empty() || manifest.contains('\n') || manifest.contains('\r') {
        bail!("archive validator returned an invalid root manifest name");
    }
    Ok(manifest.to_string())
}

pub(super) fn extract_package_to(
    archive: &Path,
    destination: &Path,
    expected_root: &str,
) -> Result<()> {
    run_archive_helper(
        "extract-package",
        archive,
        Some(destination),
        Some(expected_root),
    )?;
    Ok(())
}

fn extract_archive(archive: &Path, expected_root: &str) -> Result<TempDir> {
    let extracted = tempfile::Builder::new()
        .prefix("adze-release-extract-")
        .tempdir()?;
    extract_package_to(archive, extracted.path(), expected_root)?;
    Ok(extracted)
}

fn manifest_only_changes(
    before: &BTreeMap<String, FileIdentity>,
    after: &BTreeMap<String, FileIdentity>,
) -> Result<Vec<MemberChange>> {
    if before.keys().ne(after.keys()) {
        bail!("local-registry transformation added or removed archive members");
    }
    let mut changes = Vec::new();
    for (member, previous) in before {
        let current = &after[member];
        if current != previous {
            if member != "Cargo.toml" || previous.unix_mode != current.unix_mode {
                bail!("local-registry transformation changed unapproved member `{member}`");
            }
            changes.push(MemberChange {
                member: member.clone(),
                before_sha256: previous.sha256.clone(),
                after_sha256: current.sha256.clone(),
                reason: "existing local-registry exact-version manifest rewrite",
            });
        }
    }
    Ok(changes)
}

#[cfg(test)]
#[path = "local_registry_provenance_tests.rs"]
mod tests;
