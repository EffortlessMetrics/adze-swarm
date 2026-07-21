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
    pub crate_path: PathBuf,
    pub checksum: String,
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
            let manifest = packaged_manifest_text(&crate_path)?;
            if manifest_contains_path_dependency(&manifest) {
                bail!(
                    "packaged crate `{crate_name}` still contains a path dependency; local-registry publish blocked"
                );
            }

            let checksum = sha256_hex_file(&crate_path)?;
            let dest = self
                .crate_dir
                .join(format!("{crate_name}-{}.crate", package.version));
            fs::copy(&crate_path, &dest).with_context(|| {
                format!("copying packaged crate `{}` into local registry", dest.display())
            })?;
            write_sparse_index_entry(&self.index_dir, package, &published_set, &checksum)?;
            self.published.push(PublishedCrate {
                name: crate_name.clone(),
                version: package.version.clone(),
                crate_path: dest,
                checksum: checksum.clone(),
            });
            println!(
                "published to {REGISTRY_NAME}: {crate_name} {} ({checksum})",
                package.version
            );
        }

        Ok(())
    }
}

fn write_registry_config(
    cargo_home: &TempDir,
    index_dir: &Path,
    crate_dir: &Path,
) -> Result<()> {
    let index_url = path_to_sparse_file_url(index_dir)?;
    let dl_url = path_to_file_url(crate_dir)?;
    let config = format!(
        r#"[registries.{REGISTRY_NAME}]
index = "{index_url}"
"#
    );
    let config_path = cargo_home.path().join("config.toml");
    fs::write(&config_path, config)
        .with_context(|| format!("writing {}", config_path.display()))?;

    let config_json = format!(
        "{{\"dl\":\"{dl_url}\",\"api\":null}}\n",
        dl_url = dl_url
    );
    fs::write(index_dir.join("config.json"), config_json)
        .context("writing sparse registry config.json")?;
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

fn path_to_file_url(path: &Path) -> Result<String> {
    let absolute = fs::canonicalize(path)
        .with_context(|| format!("canonicalizing {}", path.display()))?;
    let mut url = absolute.display().to_string().replace('\\', "/");
    if url
        .as_bytes()
        .get(1)
        .is_some_and(|byte| *byte == b':')
    {
        url = format!("/{url}");
    }
    if !url.ends_with('/') {
        url.push('/');
    }
    Ok(format!("file://{url}"))
}

fn path_to_sparse_file_url(path: &Path) -> Result<String> {
    Ok(format!("sparse+{}", path_to_file_url(path)?))
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
        .args([
            "package",
            "-p",
            crate_name,
            "--allow-dirty",
            "--no-verify",
        ])
        .current_dir(workspace_root)
        .env("CARGO_HOME", cargo_home)
        .env("CARGO_TARGET_DIR", target_dir);

    for patch_arg in package_patch_config_args(workspace_root, crate_name, ordered_crates, metadata)? {
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
            format!("packaged .crate for `{crate_name}` not found under {}", package_dir.display())
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
        let relative = package_dir
            .strip_prefix(workspace_root)
            .with_context(|| {
                format!(
                    "package path {} is outside workspace {}",
                    package_dir.display(),
                    workspace_root.display()
                )
            })?;
        let relative = relative.to_string_lossy().replace('\\', "/");
        args.push(format!(
            "patch.crates-io.{}.path=\"{relative}\"",
            sibling
        ));
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
    Ok(String::from_utf8(output.stdout).context("decoding extracted Cargo.toml")?)
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

fn sha256_hex_file(path: &Path) -> Result<String> {
    let mut file = File::open(path)
        .with_context(|| format!("opening {} for checksum", path.display()))?;
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
    let output = child.wait_with_output().context("waiting for openssl dgst")?;
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
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_ascii_lowercase())
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
    registry: Option<String>,
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
    file.write_all(b"\n").context("terminating sparse index entry")?;
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
        dep_entry.insert("registry".to_string(), Value::String(REGISTRY_NAME.to_string()));
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
    fn sha256_hex_matches_known_vector() {
        let digest = sha256_hex(b"abc").expect("sha256");
        assert_eq!(
            digest,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
