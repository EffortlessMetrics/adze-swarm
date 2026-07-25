//! Isolated sparse-file local registry helpers for #856 PR2.
//!
//! Packages ledger-published crates in dependency order, writes them into a
//! temporary sparse registry, and verifies packaged manifests do not retain
//! workspace path dependencies.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsStr;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use serde_json::{Map, Value};
use tempfile::TempDir;

pub const REGISTRY_NAME: &str = "adze-local";

pub struct PublishedCrate {
    pub name: String,
    pub version: String,
    pub checksum: String,
}

pub struct LocalCliInstall {
    pub _install_root: TempDir,
    pub binary_path: PathBuf,
}

pub struct StarterProject {
    pub _parent: TempDir,
    pub project_dir: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapturedCommand {
    pub name: String,
    pub argv: Vec<String>,
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

impl CapturedCommand {
    pub fn success(&self) -> bool {
        self.exit_code == 0
    }
}

pub struct IsolatedRegistry {
    _root: TempDir,
    cargo_home: TempDir,
    target_dir: TempDir,
    index_dir: PathBuf,
    crate_dir: PathBuf,
    published: Vec<PublishedCrate>,
}

impl IsolatedRegistry {
    pub fn new() -> Result<Self> {
        let root = tempfile::Builder::new()
            .prefix("adze-local-registry-")
            .tempdir()
            .context("creating isolated local registry root")?;
        let cargo_home = tempfile::Builder::new()
            .prefix("adze-cargo-home-")
            .tempdir_in(root.path())
            .context("creating isolated CARGO_HOME")?;
        let target_dir = tempfile::Builder::new()
            .prefix("adze-target-")
            .tempdir_in(root.path())
            .context("creating isolated CARGO_TARGET_DIR")?;
        let index_dir = root.path().join("index");
        let crate_dir = root.path().join("crates");
        fs::create_dir_all(&index_dir).context("creating sparse index directory")?;
        fs::create_dir_all(&crate_dir).context("creating local crate download directory")?;

        write_registry_config(&cargo_home, &index_dir, &crate_dir)?;
        write_registry_credentials(&cargo_home)?;

        Ok(Self {
            _root: root,
            cargo_home,
            target_dir,
            index_dir,
            crate_dir,
            published: Vec::new(),
        })
    }

    pub fn published_crates(&self) -> &[PublishedCrate] {
        &self.published
    }

    pub fn publish_release_graph(
        &mut self,
        workspace_root: &Path,
        ordered_crates: &[String],
    ) -> Result<()> {
        self.write_index_entries(workspace_root, ordered_crates)
    }

    fn write_index_entries(
        &mut self,
        workspace_root: &Path,
        ordered_crates: &[String],
    ) -> Result<()> {
        let metadata = load_cargo_metadata(workspace_root)?;
        let published_set: BTreeSet<String> = ordered_crates.iter().cloned().collect();

        for crate_name in ordered_crates {
            let package = metadata
                .packages
                .iter()
                .find(|pkg| pkg.name == *crate_name)
                .with_context(|| format!("crate `{crate_name}` missing from cargo metadata"))?;
            let crate_path = package_crate(
                workspace_root,
                self.cargo_home.path(),
                self.target_dir.path(),
                crate_name,
                ordered_crates,
                &metadata,
            )?;
            inject_registry_deps_into_crate(&crate_path, &published_set)?;
            let manifest = packaged_manifest_text(&crate_path)?;
            if manifest_contains_path_dependency(&manifest) {
                bail!(
                    "packaged crate `{crate_name}` still contains a path dependency; local-registry publish blocked"
                );
            }
            if manifest_missing_registry_for_workspace_deps(&manifest, &published_set) {
                bail!(
                    "packaged crate `{crate_name}` is missing `{REGISTRY_NAME}` registry pins for workspace dependencies"
                );
            }

            let checksum = sha256_hex_file(&crate_path)?;
            let dest_dir = self.crate_dir.join(crate_name).join(&package.version);
            fs::create_dir_all(&dest_dir).with_context(|| {
                format!("creating local registry crate dir {}", dest_dir.display())
            })?;
            let dest = dest_dir.join("download");
            fs::copy(&crate_path, &dest).with_context(|| {
                format!(
                    "copying packaged crate `{}` into local registry",
                    dest.display()
                )
            })?;
            write_sparse_index_entry(&self.index_dir, package, &published_set, &checksum)?;
            self.published.push(PublishedCrate {
                name: crate_name.clone(),
                version: package.version.clone(),
                checksum: checksum.clone(),
            });
            println!(
                "published to {REGISTRY_NAME}: {crate_name} {} ({checksum})",
                package.version
            );
        }

        Ok(())
    }

    pub fn install_cli(
        &self,
        cli_crate: &str,
        bin: &str,
        version: &str,
    ) -> Result<LocalCliInstall> {
        let published = self
            .published_crates()
            .iter()
            .find(|published| published.name == cli_crate)
            .with_context(|| {
                format!("CLI crate `{cli_crate}` was not published to {REGISTRY_NAME}")
            })?;
        if published.version != version {
            bail!(
                "CLI crate `{cli_crate}` was published as {} but install requested {version}",
                published.version
            );
        }

        let install_root = tempfile::Builder::new()
            .prefix("adze-local-install-root-")
            .tempdir()
            .context("creating temporary cargo install root")?;
        let cargo = std::env::var_os("CARGO").unwrap_or_else(|| OsStr::new("cargo").to_owned());
        let status = Command::new(cargo)
            .args([
                "install",
                "--registry",
                REGISTRY_NAME,
                cli_crate,
                "--bin",
                bin,
                "--version",
                version,
                "--root",
            ])
            .arg(install_root.path())
            .env("CARGO_HOME", self.cargo_home.path())
            .env("CARGO_TARGET_DIR", self.target_dir.path())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .with_context(|| format!("installing `{cli_crate}` from {REGISTRY_NAME}"))?;
        if !status.success() {
            bail!("cargo install `{cli_crate}` from {REGISTRY_NAME} failed with status {status}");
        }

        let binary_path = installed_binary_path(install_root.path(), bin);
        if !binary_path.is_file() {
            bail!(
                "expected installed binary `{}` was not created",
                binary_path.display()
            );
        }

        Ok(LocalCliInstall {
            _install_root: install_root,
            binary_path,
        })
    }

    pub fn init_starter_project(
        &self,
        adze_binary: &Path,
        workspace_root: &Path,
        project_name: &str,
    ) -> Result<StarterProject> {
        let parent = tempfile::Builder::new()
            .prefix("adze-local-starter-")
            .tempdir()
            .context("creating starter project parent directory")?;
        let parent_path = fs::canonicalize(parent.path())
            .with_context(|| format!("canonicalizing {}", parent.path().display()))?;
        let workspace_root = fs::canonicalize(workspace_root)
            .with_context(|| format!("canonicalizing {}", workspace_root.display()))?;
        if parent_path.starts_with(&workspace_root) {
            bail!("starter project parent must be outside the source workspace");
        }

        let status = Command::new(adze_binary)
            .args(["init", project_name, "--output"])
            .arg(&parent_path)
            .env("CARGO_HOME", self.cargo_home.path())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .with_context(|| format!("running `{}` init", adze_binary.display()))?;
        if !status.success() {
            bail!(
                "`{}` init {project_name} failed with status {status}",
                adze_binary.display()
            );
        }

        let project_dir = parent_path.join(project_name);
        if !project_dir.join("Cargo.toml").is_file() {
            bail!(
                "expected generated starter at {} was not created",
                project_dir.display()
            );
        }
        let manifest = fs::read_to_string(project_dir.join("Cargo.toml")).with_context(|| {
            format!(
                "reading generated {}",
                project_dir.join("Cargo.toml").display()
            )
        })?;
        if manifest_contains_path_dependency(&manifest) {
            bail!(
                "generated starter at {} still resolves path dependencies; expected registry versions",
                project_dir.display()
            );
        }

        Ok(StarterProject {
            _parent: parent,
            project_dir,
        })
    }

    pub fn run_cargo_capture(
        &self,
        cwd: &Path,
        name: &str,
        args: &[&str],
    ) -> Result<CapturedCommand> {
        let cargo = std::env::var_os("CARGO").unwrap_or_else(|| OsStr::new("cargo").to_owned());
        let mut argv = vec!["cargo".to_string()];
        argv.extend(args.iter().map(|arg| (*arg).to_string()));

        let output = Command::new(cargo)
            .args(args)
            .current_dir(cwd)
            .env("CARGO_HOME", self.cargo_home.path())
            .env("CARGO_TARGET_DIR", self.target_dir.path())
            .output()
            .with_context(|| format!("running cargo {} in {}", args.join(" "), cwd.display()))?;

        Ok(CapturedCommand {
            name: name.to_string(),
            argv,
            exit_code: output.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }
}

impl StarterProject {
    pub fn run_starter_proof(&self, registry: &IsolatedRegistry) -> Result<Vec<CapturedCommand>> {
        let mut commands = Vec::new();

        let test =
            registry.run_cargo_capture(&self.project_dir, "starter-cargo-test", &["test"])?;
        commands.push(test);

        let parse_ok = registry.run_cargo_capture(
            &self.project_dir,
            "starter-parse-valid",
            &["run", "--example", "parse", "--", "1 + 2 * 3"],
        )?;
        commands.push(parse_ok);

        let parse_bad = registry.run_cargo_capture(
            &self.project_dir,
            "starter-parse-invalid",
            &["run", "--example", "parse", "--", "1 + @"],
        )?;
        commands.push(parse_bad);

        crate::local_release_receipt::validate_starter_proof(&commands)?;
        Ok(commands)
    }
}

fn installed_binary_path(root: &Path, bin_name: &str) -> PathBuf {
    let mut binary = bin_name.to_owned();
    binary.push_str(std::env::consts::EXE_SUFFIX);
    root.join("bin").join(binary)
}

fn write_registry_config(cargo_home: &TempDir, index_dir: &Path, crate_dir: &Path) -> Result<()> {
    let index_url = sparse_file_url(index_dir)?;
    let dl_url = sparse_file_url(crate_dir)?;
    let config = format!(
        r#"[registries.{REGISTRY_NAME}]
index = "{index_url}"
"#
    );
    let config_path = cargo_home.path().join("config.toml");
    fs::write(&config_path, config)
        .with_context(|| format!("writing {}", config_path.display()))?;

    let config_json = format!("{{\"dl\":\"{dl_url}\",\"api\":null}}\n", dl_url = dl_url);
    fs::write(index_dir.join("config.json"), config_json)
        .context("writing registry config.json")?;
    Ok(())
}

fn finalize_git_index(index_dir: &Path) -> Result<()> {
    if index_dir.join(".git").is_dir() {
        return Ok(());
    }
    run_git(index_dir, ["init", "-b", "master"])?;
    run_git(index_dir, ["add", "."])?;
    run_git(index_dir, ["commit", "-m", "adze-local registry index"])?;
    Ok(())
}

fn run_git(index_dir: &Path, args: impl IntoIterator<Item = &'static str>) -> Result<()> {
    let mut command = Command::new("git");
    command
        .args([
            "-c",
            "user.email=adze-local@example.com",
            "-c",
            "user.name=adze-local",
        ])
        .args(args)
        .current_dir(index_dir)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    let status = command
        .status()
        .context("running git for local registry index")?;
    if !status.success() {
        bail!(
            "git command in {} failed with status {status}",
            index_dir.display()
        );
    }
    Ok(())
}

fn write_registry_credentials(cargo_home: &TempDir) -> Result<()> {
    let credentials = format!(
        r#"[registries.{REGISTRY_NAME}]
token = "local"
"#
    );
    let credentials_path = cargo_home.path().join("credentials.toml");
    fs::write(&credentials_path, credentials)
        .with_context(|| format!("writing {}", credentials_path.display()))?;
    Ok(())
}

fn sparse_file_url(path: &Path) -> Result<String> {
  Ok(format!("sparse+{}", path_to_file_url(path)?))
}

fn path_to_file_url(path: &Path) -> Result<String> {
    let absolute =
        fs::canonicalize(path).with_context(|| format!("canonicalizing {}", path.display()))?;
    let mut url = absolute.display().to_string().replace('\\', "/");
    if let Some(stripped) = url.strip_prefix("//?/") {
        url = stripped.to_string();
    }
    if url.as_bytes().get(1).is_some_and(|byte| *byte == b':') {
        url = format!("/{url}");
    }
    if !url.ends_with('/') {
        url.push('/');
    }
    Ok(format!("file://{url}"))
}

fn legacy_published_crate_path(crate_dir: &Path, crate_name: &str, version: &str) -> PathBuf {
    crate_dir.join(format!("{crate_name}-{version}.crate"))
}

fn package_crate(
    workspace_root: &Path,
    cargo_home: &Path,
    target_dir: &Path,
    crate_name: &str,
    ordered_crates: &[String],
    metadata: &CargoMetadata,
) -> Result<PathBuf> {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| OsStr::new("cargo").to_owned());
    let mut command = Command::new(cargo);
    command
        .args(["package", "-p", crate_name, "--allow-dirty", "--no-verify"])
        .current_dir(workspace_root)
        .env("CARGO_HOME", cargo_home)
        .env("CARGO_TARGET_DIR", target_dir);

    for patch_arg in
        package_patch_config_args(workspace_root, crate_name, ordered_crates, metadata)?
    {
        command.arg("--config").arg(patch_arg);
    }

    let status = command
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .with_context(|| format!("running cargo package -p {crate_name}"))?;
    if !status.success() {
        bail!("cargo package -p {crate_name} failed with status {status}");
    }

    let package_dir = target_dir.join("package");
    let crate_path = fs::read_dir(&package_dir)
        .with_context(|| format!("reading {}", package_dir.display()))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| {
            path.extension().is_some_and(|ext| ext == "crate")
                && path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with(crate_name))
        })
        .with_context(|| {
            format!(
                "packaged .crate for `{crate_name}` not found under {}",
                package_dir.display()
            )
        })?;
    Ok(crate_path)
}

fn package_patch_config_args(
    workspace_root: &Path,
    crate_name: &str,
    ordered_crates: &[String],
    metadata: &CargoMetadata,
) -> Result<Vec<String>> {
    let mut args = Vec::new();
    for sibling in ordered_crates {
        if sibling == crate_name {
            continue;
        }
        let Some(package) = metadata.packages.iter().find(|pkg| pkg.name == *sibling) else {
            continue;
        };
        let manifest_path = Path::new(&package.manifest_path);
        let package_dir = manifest_path
            .parent()
            .with_context(|| format!("missing parent for {}", manifest_path.display()))?;
        let relative = package_dir.strip_prefix(workspace_root).with_context(|| {
            format!(
                "package path {} is outside workspace {}",
                package_dir.display(),
                workspace_root.display()
            )
        })?;
        let relative = relative.to_string_lossy().replace('\\', "/");
        args.push(format!("patch.crates-io.{}.path=\"{relative}\"", sibling));
    }
    Ok(args)
}

fn packaged_manifest_text(crate_path: &Path) -> Result<String> {
    let manifest_member = packaged_manifest_member(crate_path)?;
    let output = Command::new("tar")
        .args(["-xOf"])
        .arg(crate_path)
        .arg(&manifest_member)
        .output()
        .context("extracting Cargo.toml from packaged crate via tar")?;
    if !output.status.success() {
        bail!(
            "failed to extract Cargo.toml from {}: {}",
            crate_path.display(),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    String::from_utf8(output.stdout).context("decoding extracted Cargo.toml")
}

fn packaged_manifest_member(crate_path: &Path) -> Result<String> {
    let output = Command::new("tar")
        .args(["-tzf"])
        .arg(crate_path)
        .output()
        .with_context(|| format!("listing members of {}", crate_path.display()))?;
    if !output.status.success() {
        bail!(
            "failed to list packaged crate {}: {}",
            crate_path.display(),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let manifest_member = String::from_utf8_lossy(&output.stdout)
        .lines()
        .find(|line| line.ends_with("/Cargo.toml"))
        .map(str::to_string)
        .with_context(|| format!("Cargo.toml not found in {}", crate_path.display()))?;
    Ok(manifest_member)
}

fn manifest_contains_path_dependency(manifest: &str) -> bool {
    let mut in_dependency_table = false;
    for line in manifest.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') {
            continue;
        }
        if trimmed.starts_with('[') {
            in_dependency_table = trimmed.starts_with("[dependencies")
                || trimmed.starts_with("[dev-dependencies")
                || trimmed.starts_with("[build-dependencies");
            continue;
        }
        if in_dependency_table && trimmed.contains("path =") {
            return true;
        }
    }
    false
}

fn manifest_missing_registry_for_workspace_deps(
    manifest: &str,
    published_set: &BTreeSet<String>,
) -> bool {
    let Ok(value) = manifest.parse::<toml::Value>() else {
        return true;
    };
    for table_name in ["dependencies", "dev-dependencies", "build-dependencies"] {
        let Some(deps) = value.get(table_name).and_then(toml::Value::as_table) else {
            continue;
        };
        for (name, spec) in deps {
            if !published_set.contains(name) {
                continue;
            }
            if !dependency_spec_has_registry(spec) {
                return true;
            }
        }
    }
    false
}

fn dependency_spec_has_registry(spec: &toml::Value) -> bool {
    match spec {
        toml::Value::Table(table) => table
            .get("registry")
            .and_then(toml::Value::as_str)
            .is_some_and(|registry| registry == REGISTRY_NAME),
        _ => false,
    }
}

fn inject_registry_deps_into_crate(
    crate_path: &Path,
    published_set: &BTreeSet<String>,
) -> Result<()> {
    let manifest_member = packaged_manifest_member(crate_path)?;
    let manifest = packaged_manifest_text(crate_path)?;
    let updated = inject_registry_into_manifest(&manifest, published_set)?;
    if updated == manifest {
        return Ok(());
    }
    repack_crate_with_manifest(crate_path, &manifest_member, &updated)
}

fn inject_registry_into_manifest(
    manifest: &str,
    published_set: &BTreeSet<String>,
) -> Result<String> {
    let mut value: toml::Value = manifest
        .parse()
        .context("parsing packaged manifest for registry injection")?;
    let root = value
        .as_table_mut()
        .context("packaged manifest root must be a table")?;

    for table_name in ["dependencies", "dev-dependencies", "build-dependencies"] {
        let Some(deps) = root.get_mut(table_name).and_then(toml::Value::as_table_mut) else {
            continue;
        };
        for (name, spec) in deps.iter_mut() {
            if !published_set.contains(name) {
                continue;
            }
            *spec = registry_pinned_dependency_spec(spec)?;
        }
    }

    if let Some(target) = root.get_mut("target").and_then(toml::Value::as_table_mut) {
        let target_keys = target.keys().cloned().collect::<Vec<_>>();
        for target_key in target_keys {
            let Some(target_table) = target.get_mut(&target_key).and_then(toml::Value::as_table_mut)
            else {
                continue;
            };
            for table_name in ["dependencies", "dev-dependencies", "build-dependencies"] {
                let Some(deps) = target_table
                    .get_mut(table_name)
                    .and_then(toml::Value::as_table_mut)
                else {
                    continue;
                };
                for (name, spec) in deps.iter_mut() {
                    if !published_set.contains(name) {
                        continue;
                    }
                    *spec = registry_pinned_dependency_spec(spec)?;
                }
            }
        }
    }

    toml::to_string(&value).context("serializing registry-pinned packaged manifest")
}

fn registry_pinned_dependency_spec(spec: &toml::Value) -> Result<toml::Value> {
    let mut table = match spec {
        toml::Value::String(version) => {
            let mut table = toml::map::Map::new();
            table.insert(
                "version".to_string(),
                toml::Value::String(version.clone()),
            );
            table
        }
        toml::Value::Table(table) => table.clone(),
        _ => bail!("unsupported dependency spec in packaged manifest"),
    };
    if table
        .get("registry")
        .and_then(toml::Value::as_str)
        .is_some_and(|registry| registry == REGISTRY_NAME)
    {
        return Ok(toml::Value::Table(table));
    }
    table.insert(
        "registry".to_string(),
        toml::Value::String(REGISTRY_NAME.to_string()),
    );
    Ok(toml::Value::Table(table))
}

fn repack_crate_with_manifest(
    crate_path: &Path,
    manifest_member: &str,
    manifest: &str,
) -> Result<()> {
    let extract_dir = tempfile::Builder::new()
        .prefix("adze-local-crate-repack-")
        .tempdir()
        .context("creating crate repack extraction directory")?;
    let status = Command::new("tar")
        .args(["-xzf"])
        .arg(crate_path)
        .arg("-C")
        .arg(extract_dir.path())
        .status()
        .with_context(|| format!("extracting {} for manifest rewrite", crate_path.display()))?;
    if !status.success() {
        bail!(
            "failed to extract {} for manifest rewrite",
            crate_path.display()
        );
    }

    let manifest_path = extract_dir.path().join(manifest_member);
    fs::write(&manifest_path, manifest).with_context(|| {
        format!(
            "writing registry-pinned manifest to {}",
            manifest_path.display()
        )
    })?;

    let crate_root = manifest_path
        .parent()
        .with_context(|| format!("missing crate root for {}", manifest_path.display()))?;
    let crate_root_name = crate_root
        .file_name()
        .and_then(OsStr::to_str)
        .with_context(|| format!("missing crate root name for {}", crate_root.display()))?;

    let repacked_path = crate_path.with_extension("crate.repacked");
    let status = Command::new("tar")
        .args(["-czf"])
        .arg(&repacked_path)
        .arg("-C")
        .arg(extract_dir.path())
        .arg(crate_root_name)
        .status()
        .with_context(|| format!("repacking {}", crate_path.display()))?;
    if !status.success() {
        bail!("failed to repack {}", crate_path.display());
    }
    fs::rename(&repacked_path, crate_path).with_context(|| {
        format!(
            "replacing {} with registry-pinned package",
            crate_path.display()
        )
    })?;
    Ok(())
}

fn sha256_hex_file(path: &Path) -> Result<String> {
    let mut file =
        File::open(path).with_context(|| format!("opening {} for checksum", path.display()))?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)
        .with_context(|| format!("reading {} for checksum", path.display()))?;
    sha256_hex(&buffer)
}

fn sha256_hex(bytes: &[u8]) -> Result<String> {
    if let Ok(digest) = sha256_hex_openssl(bytes) {
        return Ok(digest);
    }
    sha256_hex_powershell(bytes)
}

fn sha256_hex_openssl(bytes: &[u8]) -> Result<String> {
    use std::io::Write as _;

    let mut child = Command::new("openssl")
        .args(["dgst", "-sha256", "-hex"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .context("spawning openssl dgst for sha256")?;
    {
        let mut stdin = child.stdin.take().context("openssl stdin unavailable")?;
        stdin.write_all(bytes)?;
    }
    let output = child
        .wait_with_output()
        .context("waiting for openssl dgst")?;
    if !output.status.success() {
        bail!("openssl sha256 failed");
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let Some((_prefix, digest)) = stdout.split_once('=') else {
        bail!("unexpected openssl output: {stdout}");
    };
    Ok(digest.trim().to_ascii_lowercase())
}

#[cfg(windows)]
fn sha256_hex_powershell(bytes: &[u8]) -> Result<String> {
    use std::io::Write as _;

    let mut child = Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            "$input = [Console]::In.ReadToEnd(); $bytes = [Text.Encoding]::UTF8.GetBytes($input); $hash = [System.Security.Cryptography.SHA256]::Create().ComputeHash($bytes); -join ($hash | ForEach-Object { $_.ToString('x2') })",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("spawning PowerShell sha256 hasher")?;
    {
        let mut stdin = child.stdin.take().context("PowerShell stdin unavailable")?;
        stdin.write_all(bytes)?;
    }
    let output = child
        .wait_with_output()
        .context("waiting for PowerShell sha256 hasher")?;
    if !output.status.success() {
        bail!(
            "PowerShell sha256 failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .trim()
        .to_ascii_lowercase())
}

#[cfg(not(windows))]
fn sha256_hex_powershell(bytes: &[u8]) -> Result<String> {
    use std::io::Write as _;

    let mut child = Command::new("sha256sum")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("spawning sha256sum")?;
    {
        let mut stdin = child.stdin.take().context("sha256sum stdin unavailable")?;
        stdin.write_all(bytes)?;
    }
    let output = child.wait_with_output().context("waiting for sha256sum")?;
    if !output.status.success() {
        bail!(
            "sha256sum failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let digest = stdout.split_whitespace().next().unwrap_or_default();
    if digest.is_empty() {
        bail!("unexpected sha256sum output: {stdout}");
    }
    Ok(digest.to_ascii_lowercase())
}

#[derive(Debug, Clone, Deserialize)]
struct CargoMetadata {
    packages: Vec<CargoPackage>,
}

#[derive(Debug, Clone, Deserialize)]
struct CargoPackage {
    name: String,
    version: String,
    manifest_path: String,
    features: BTreeMap<String, Vec<String>>,
    dependencies: Vec<CargoDependency>,
}

#[derive(Debug, Clone, Deserialize)]
struct CargoDependency {
    name: String,
    req: String,
    #[serde(default)]
    features: Vec<String>,
    #[serde(default)]
    optional: bool,
    #[serde(default = "default_true")]
    default_features: bool,
    target: Option<String>,
    kind: Option<String>,
}

fn default_true() -> bool {
    true
}

fn load_cargo_metadata(workspace_root: &Path) -> Result<CargoMetadata> {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| OsStr::new("cargo").to_owned());
    let output = Command::new(cargo)
        .args(["metadata", "--format-version", "1", "--locked"])
        .current_dir(workspace_root)
        .output()
        .context("running cargo metadata --format-version 1 --locked")?;
    if !output.status.success() {
        bail!(
            "cargo metadata failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    serde_json::from_slice(&output.stdout).context("parsing cargo metadata output")
}

fn write_sparse_index_entry(
    index_dir: &Path,
    package: &CargoPackage,
    published_set: &BTreeSet<String>,
    checksum: &str,
) -> Result<()> {
    let index_file = sparse_index_file(index_dir, &package.name);
    if let Some(parent) = index_file.parent() {
        fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
    }

    let deps = package
        .dependencies
        .iter()
        .filter(|dep| dependency_counts_for_index(dep))
        .map(|dep| index_dependency(dep, published_set))
        .collect::<Result<Vec<_>>>()?;

    let mut entry = Map::new();
    entry.insert("name".to_string(), Value::String(package.name.clone()));
    entry.insert("vers".to_string(), Value::String(package.version.clone()));
    entry.insert(
        "deps".to_string(),
        serde_json::to_value(deps).context("serializing index dependencies")?,
    );
    entry.insert("cksum".to_string(), Value::String(checksum.to_string()));
    entry.insert(
        "features".to_string(),
        serde_json::to_value(&package.features).context("serializing crate features")?,
    );
    entry.insert("yanked".to_string(), Value::Bool(false));

    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&index_file)
        .with_context(|| format!("opening {}", index_file.display()))?;
    serde_json::to_writer(&mut file, &entry).context("writing sparse index entry")?;
    file.write_all(b"\n")
        .context("terminating sparse index entry")?;
    Ok(())
}

fn sparse_index_file(index_dir: &Path, crate_name: &str) -> PathBuf {
    let prefix = crate_name.get(..2).unwrap_or(crate_name);
    let suffix = crate_name.get(2..4).unwrap_or("");
    index_dir.join(prefix).join(suffix).join(crate_name)
}

fn dependency_counts_for_index(dep: &CargoDependency) -> bool {
    matches!(dep.kind.as_deref(), None | Some("normal") | Some("build"))
}

fn index_dependency(dep: &CargoDependency, published_set: &BTreeSet<String>) -> Result<Value> {
    let mut dep_entry = Map::new();
    dep_entry.insert("name".to_string(), Value::String(dep.name.clone()));
    dep_entry.insert("req".to_string(), Value::String(dep.req.clone()));
    dep_entry.insert(
        "features".to_string(),
        serde_json::to_value(&dep.features).context("serializing dependency features")?,
    );
    dep_entry.insert("optional".to_string(), Value::Bool(dep.optional));
    dep_entry.insert(
        "default_features".to_string(),
        Value::Bool(dep.default_features),
    );
    dep_entry.insert(
        "target".to_string(),
        dep.target
            .as_ref()
            .map(|target| Value::String(target.clone()))
            .unwrap_or(Value::Null),
    );
    dep_entry.insert(
        "kind".to_string(),
        Value::String(dep.kind.clone().unwrap_or_else(|| "normal".to_string())),
    );
    if published_set.contains(&dep.name) {
        dep_entry.insert(
            "registry".to_string(),
            Value::String(REGISTRY_NAME.to_string()),
        );
    } else {
        dep_entry.insert("registry".to_string(), Value::Null);
    }
    dep_entry.insert("package".to_string(), Value::Null);
    Ok(Value::Object(dep_entry))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_path_detection_flags_workspace_paths() {
        let manifest = r#"
[dependencies]
adze = { version = "0.9.0", path = "../runtime" }
"#;
        assert!(manifest_contains_path_dependency(manifest));
    }

    #[test]
    fn manifest_path_detection_allows_registry_versions() {
        let manifest = r#"
[dependencies]
adze = "0.9.0"
"#;
        assert!(!manifest_contains_path_dependency(manifest));
    }

    #[test]
    fn sparse_index_file_uses_crates_io_layout() {
        let path = sparse_index_file(Path::new("/tmp/index"), "adze-cli");
        assert_eq!(path, Path::new("/tmp/index/ad/ze/adze-cli"));
    }

    #[test]
    fn sparse_file_url_uses_sparse_file_protocol() {
        let index_dir = std::env::temp_dir().join("adze-local-index-test-sparse");
        std::fs::create_dir_all(&index_dir).expect("create temp index dir");
        let url = sparse_file_url(&index_dir).expect("sparse file url");
        assert!(url.starts_with("sparse+file://"));
    }

    #[test]
    fn registry_index_url_uses_file_protocol_for_git_index() {
        let index_dir = std::env::temp_dir().join("adze-local-index-test-git");
        std::fs::create_dir_all(&index_dir).expect("create temp index dir");
        let url = path_to_file_url(&index_dir).expect("registry index url");
        assert!(url.starts_with("file://"));
        assert!(url.contains("adze-local-index-test-git"));
    }

    #[test]
    fn inject_registry_into_manifest_pins_workspace_dependencies() {
        let manifest = r#"
[dependencies]
adze = "0.9.0"
clap = "4.5"
"#;
        let published = BTreeSet::from(["adze".to_string()]);
        let updated = inject_registry_into_manifest(manifest, &published).expect("inject");
        assert!(updated.contains(r#"registry = "adze-local""#));
        assert!(updated.contains("adze"));
        assert!(updated.contains(r#"clap = "4.5""#));
    }

    #[test]
    fn manifest_missing_registry_for_workspace_deps_detects_unpinned_workspace_crate() {
        let manifest = r#"
[dependencies]
adze = "0.9.0"
"#;
        let published = BTreeSet::from(["adze".to_string()]);
        assert!(manifest_missing_registry_for_workspace_deps(
            manifest,
            &published
        ));
        let pinned = inject_registry_into_manifest(manifest, &published).expect("inject");
        assert!(!manifest_missing_registry_for_workspace_deps(
            &pinned,
            &published
        ));
    }

    #[test]
    fn sha256_hex_matches_known_vector() {
        let digest = sha256_hex(b"abc").expect("sha256");
        assert_eq!(
            digest,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
