//! Isolated sparse-file local registry helpers for #856 PR2.
//!
//! Packages ledger-published crates in dependency order, writes them into a
//! temporary sparse registry, and verifies packaged manifests do not retain
//! workspace path dependencies.

use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

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
    http_server: Option<HttpServer>,
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

        write_registry_credentials(&cargo_home)?;

        Ok(Self {
            _root: root,
            cargo_home,
            target_dir,
            index_dir,
            crate_dir,
            published: Vec::new(),
            http_server: None,
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
        let expected_versions = expected_release_versions(&metadata, ordered_crates)?;
        let mut staged = Vec::new();

        for crate_name in ordered_crates {
            let package = metadata
                .packages
                .iter()
                .find(|pkg| pkg.name == *crate_name)
                .with_context(|| format!("crate `{crate_name}` missing from cargo metadata"))?
                .clone();
            let crate_path = package_crate(
                workspace_root,
                self.cargo_home.path(),
                self.target_dir.path(),
                crate_name,
                ordered_crates,
                &metadata,
            )?;
            inject_registry_deps_into_crate(&crate_path, &expected_versions)?;
            let manifest = packaged_manifest_text(&crate_path)?;
            let violations = manifest_isolation_violations(&manifest, &expected_versions);
            if !violations.is_empty() {
                bail!(
                    "packaged crate `{crate_name}` failed {REGISTRY_NAME} isolation: {}",
                    violations.join("; ")
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
            staged.push((package.clone(), checksum.clone()));
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

        self.activate_http_registry()?;
        for (package, checksum) in staged {
            write_sparse_index_entry(&self.index_dir, &package, &expected_versions, &checksum)?;
        }
        Ok(())
    }

    fn activate_http_registry(&mut self) -> Result<String> {
        if self.http_server.is_some() {
            bail!("local registry HTTP server already active");
        }
        let registry_root = self
            .index_dir
            .parent()
            .context("missing local registry root")?;
        let (child, port) = start_http_server(registry_root)?;
        let index_url = format!("sparse+http://127.0.0.1:{port}/index/");
        let dl_url = format!("http://127.0.0.1:{port}/crates/");
        // Keep the guard local until all activation steps succeed. A config
        // write error must stop/reap this child and leave activation retryable.
        write_registry_config(&self.cargo_home, &index_url, &self.index_dir, &dl_url)?;
        self.http_server = Some(child);
        Ok(index_url)
    }

    pub fn install_cli(
        &self,
        cli_crate: &str,
        bin: &str,
        version: &str,
    ) -> Result<LocalCliInstall> {
        let cargo = std::env::var_os("CARGO").unwrap_or_else(|| OsStr::new("cargo").to_owned());
        self.install_cli_with_command(cli_crate, bin, version, Command::new(cargo))
    }

    fn install_cli_with_command(
        &self,
        cli_crate: &str,
        bin: &str,
        version: &str,
        mut command: Command,
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
        let status = command
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
        let expected_versions: BTreeMap<String, String> = self
            .published
            .iter()
            .map(|published| (published.name.clone(), published.version.clone()))
            .collect();
        let pinned = inject_registry_into_manifest(&manifest, &expected_versions)?;
        if pinned != manifest {
            fs::write(project_dir.join("Cargo.toml"), &pinned).with_context(|| {
                format!(
                    "pinning generated starter {} to {REGISTRY_NAME}",
                    project_dir.join("Cargo.toml").display()
                )
            })?;
        }
        let violations = manifest_isolation_violations(&pinned, &expected_versions);
        if !violations.is_empty() {
            bail!(
                "generated starter at {} failed {REGISTRY_NAME} isolation: {}",
                project_dir.display(),
                violations.join("; ")
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

fn write_registry_config(
    cargo_home: &TempDir,
    index_url: &str,
    index_dir: &Path,
    dl_url: &str,
) -> Result<()> {
    let config = format!(
        r#"[registries.{REGISTRY_NAME}]
index = "{index_url}"
"#
    );
    let config_path = cargo_home.path().join("config.toml");
    fs::write(&config_path, config)
        .with_context(|| format!("writing {}", config_path.display()))?;

    let config_json = registry_config_json(dl_url);
    fs::write(index_dir.join("config.json"), config_json)
        .context("writing registry config.json")?;
    Ok(())
}

fn registry_config_json(dl_url: &str) -> String {
    format!("{{\"dl\":\"{dl_url}\",\"api\":null}}\n")
}

const HTTP_STARTUP_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);
const HTTP_READER_SETTLE: std::time::Duration = std::time::Duration::from_millis(100);
const HTTP_PORT_LINE_LIMIT: usize = 64;
const HTTP_STDERR_LIMIT: usize = 8192;

#[derive(Clone, Copy)]
struct ServerStartupLimits {
    timeout: std::time::Duration,
    port_line_bytes: usize,
    stderr_bytes: usize,
}

impl Default for ServerStartupLimits {
    fn default() -> Self {
        Self {
            timeout: HTTP_STARTUP_TIMEOUT,
            port_line_bytes: HTTP_PORT_LINE_LIMIT,
            stderr_bytes: HTTP_STDERR_LIMIT,
        }
    }
}

#[derive(Debug, Default, Clone)]
struct ServerStderr {
    bytes: Vec<u8>,
    truncated: bool,
    complete: bool,
    read_error: Option<String>,
}

/// Owns the direct child from spawn through every startup/teardown path.
/// Reader threads retain bounded storage and are never joined while blocked on
/// pipes inherited by another process; cleanup owns only this child.
struct HttpServer {
    child: Child,
    stderr: std::sync::Arc<std::sync::Mutex<ServerStderr>>,
    readers: Vec<std::thread::JoinHandle<()>>,
    stopped: bool,
}

#[derive(Debug)]
enum ServerHandshake {
    Line(Vec<u8>),
    Oversized,
    Eof,
    ReadError(String),
}

#[derive(Debug)]
enum ServerStartupProblem {
    Exited,
    PrematureEof,
    Timeout,
    MalformedPort(Vec<u8>),
    OversizedPort,
    ReadError(String),
}

impl std::fmt::Display for ServerStartupProblem {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Exited => formatter.write_str("child exited before startup completed"),
            Self::PrematureEof => formatter.write_str("premature EOF before port line"),
            Self::Timeout => formatter.write_str("timed out awaiting port line"),
            Self::MalformedPort(line) => write!(
                formatter,
                "malformed port line {:?}",
                String::from_utf8_lossy(line)
            ),
            Self::OversizedPort => formatter.write_str("oversized port line"),
            Self::ReadError(error) => write!(formatter, "reading port line failed: {error}"),
        }
    }
}

#[derive(Debug)]
struct ServerStartupError {
    program: std::ffi::OsString,
    pid: u32,
    problem: ServerStartupProblem,
    status: Option<std::process::ExitStatus>,
    stderr: ServerStderr,
    cleanup_error: Option<String>,
}

impl std::fmt::Display for ServerStartupError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "local registry server {:?} (pid {}) startup failed: {}; exit status: {:?}; stderr{}{}: {}",
            self.program,
            self.pid,
            self.problem,
            self.status,
            if self.stderr.truncated {
                " [truncated]"
            } else {
                ""
            },
            if !self.stderr.complete {
                " [incomplete]"
            } else {
                ""
            },
            String::from_utf8_lossy(&self.stderr.bytes)
        )?;
        if let Some(error) = &self.stderr.read_error {
            write!(formatter, "; stderr read error: {error}")?;
        }
        if let Some(error) = &self.cleanup_error {
            write!(formatter, "; child cleanup error: {error}")?;
        }
        Ok(())
    }
}

impl std::error::Error for ServerStartupError {}

impl HttpServer {
    fn new(
        child: Child,
        limits: ServerStartupLimits,
    ) -> Result<(Self, std::sync::mpsc::Receiver<ServerHandshake>)> {
        let mut server = Self {
            child,
            stderr: Default::default(),
            readers: Vec::new(),
            stopped: false,
        };
        let stdout = server
            .child
            .stdout
            .take()
            .context("missing registry child stdout pipe")?;
        let stderr = server
            .child
            .stderr
            .take()
            .context("missing registry child stderr pipe")?;
        let (sender, receiver) = std::sync::mpsc::sync_channel(1);
        server.readers.push(
            std::thread::Builder::new()
                .name("registry-stdout".into())
                .spawn(move || {
                    drain_server_stdout(stdout, sender, limits.port_line_bytes);
                })
                .context("starting registry stdout reader")?,
        );
        let captured = server.stderr.clone();
        server.readers.push(
            std::thread::Builder::new()
                .name("registry-stderr".into())
                .spawn(move || {
                    drain_server_stderr(stderr, captured, limits.stderr_bytes);
                })
                .context("starting registry stderr reader")?,
        );
        Ok((server, receiver))
    }

    fn stop_and_reap(&mut self) -> (Option<std::process::ExitStatus>, Option<String>) {
        if self.stopped {
            return (None, None);
        }
        let mut errors = Vec::new();
        match self.child.try_wait() {
            Ok(Some(_)) => {}
            Ok(None) => {
                if let Err(error) = self.child.kill() {
                    errors.push(format!("kill: {error}"));
                }
            }
            Err(error) => {
                errors.push(format!("try_wait: {error}"));
                if let Err(error) = self.child.kill() {
                    errors.push(format!("kill: {error}"));
                }
            }
        }
        let status = match self.child.wait() {
            Ok(status) => Some(status),
            Err(error) => {
                errors.push(format!("wait: {error}"));
                None
            }
        };
        self.stopped = true;
        // A descendant could hold either pipe open. Never turn bounded startup
        // into an unbounded join; unfinished readers own no unbounded buffers.
        let deadline = std::time::Instant::now() + HTTP_READER_SETTLE;
        while self.readers.iter().any(|reader| !reader.is_finished())
            && std::time::Instant::now() < deadline
        {
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        for reader in self.readers.drain(..) {
            if reader.is_finished() {
                let _ = reader.join();
            }
        }
        (status, (!errors.is_empty()).then(|| errors.join("; ")))
    }

    fn startup_failure(
        &mut self,
        program: std::ffi::OsString,
        problem: ServerStartupProblem,
    ) -> anyhow::Error {
        let pid = self.child.id();
        let (status, cleanup_error) = self.stop_and_reap();
        let stderr = self
            .stderr
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .clone();
        ServerStartupError {
            program,
            pid,
            problem,
            status,
            stderr,
            cleanup_error,
        }
        .into()
    }
}

impl Drop for HttpServer {
    fn drop(&mut self) {
        self.stop_and_reap();
    }
}

fn drain_server_stdout(
    mut stdout: impl Read,
    sender: std::sync::mpsc::SyncSender<ServerHandshake>,
    limit: usize,
) {
    let mut line = Vec::new();
    let mut pending = true;
    let mut buffer = [0u8; 1024];
    loop {
        match stdout.read(&mut buffer) {
            Ok(0) => {
                if pending {
                    let _ = sender.try_send(ServerHandshake::Eof);
                }
                return;
            }
            Ok(count) if pending => {
                for byte in &buffer[..count] {
                    if *byte == b'\n' {
                        let _ = sender.try_send(ServerHandshake::Line(std::mem::take(&mut line)));
                        pending = false;
                        break;
                    }
                    if line.len() == limit {
                        let _ = sender.try_send(ServerHandshake::Oversized);
                        pending = false;
                        line.clear();
                        break;
                    }
                    line.push(*byte);
                }
            }
            Ok(_) => {} // Keep draining without retaining post-handshake stdout.
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(error) => {
                if pending {
                    let _ = sender.try_send(ServerHandshake::ReadError(error.to_string()));
                }
                return;
            }
        }
    }
}

fn drain_server_stderr(
    mut stderr: impl Read,
    captured: std::sync::Arc<std::sync::Mutex<ServerStderr>>,
    limit: usize,
) {
    let mut buffer = [0u8; 4096];
    loop {
        match stderr.read(&mut buffer) {
            Ok(0) => {
                captured
                    .lock()
                    .unwrap_or_else(|poison| poison.into_inner())
                    .complete = true;
                return;
            }
            Ok(count) => {
                let mut output = captured.lock().unwrap_or_else(|poison| poison.into_inner());
                let retain = count.min(limit.saturating_sub(output.bytes.len()));
                output.bytes.extend_from_slice(&buffer[..retain]);
                output.truncated |= retain != count;
            }
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(error) => {
                captured
                    .lock()
                    .unwrap_or_else(|poison| poison.into_inner())
                    .read_error = Some(error.to_string());
                return;
            }
        }
    }
}

fn wait_for_server(
    mut server: HttpServer,
    receiver: std::sync::mpsc::Receiver<ServerHandshake>,
    program: std::ffi::OsString,
    limits: ServerStartupLimits,
) -> Result<(HttpServer, u16)> {
    let deadline = std::time::Instant::now() + limits.timeout;
    loop {
        match server.child.try_wait() {
            Ok(Some(_)) => {
                return Err(server.startup_failure(program, ServerStartupProblem::Exited));
            }
            Err(error) => {
                return Err(server
                    .startup_failure(program, ServerStartupProblem::ReadError(error.to_string())));
            }
            Ok(None) => {}
        }
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        if remaining.is_zero() {
            return Err(server.startup_failure(program, ServerStartupProblem::Timeout));
        }
        match receiver.recv_timeout(remaining.min(std::time::Duration::from_millis(10))) {
            Ok(ServerHandshake::Line(mut line)) => {
                if line.last() == Some(&b'\r') {
                    line.pop();
                }
                let port = if !line.is_empty() && line.iter().all(u8::is_ascii_digit) {
                    std::str::from_utf8(&line)
                        .ok()
                        .and_then(|text| text.parse::<u16>().ok())
                        .filter(|port| *port != 0)
                } else {
                    None
                };
                let Some(port) = port else {
                    return Err(
                        server.startup_failure(program, ServerStartupProblem::MalformedPort(line))
                    );
                };
                match server.child.try_wait() {
                    Ok(None) => return Ok((server, port)),
                    Ok(Some(_)) => {
                        return Err(server.startup_failure(program, ServerStartupProblem::Exited));
                    }
                    Err(error) => {
                        return Err(server.startup_failure(
                            program,
                            ServerStartupProblem::ReadError(error.to_string()),
                        ));
                    }
                }
            }
            Ok(handshake) => {
                // EOF can race the child's exit notification. Give a process
                // that just closed stdout a short, bounded chance to expose
                // its real exit status before deciding it is still running.
                if matches!(handshake, ServerHandshake::Eof) {
                    let settle = std::time::Instant::now() + std::time::Duration::from_millis(50);
                    loop {
                        match server.child.try_wait() {
                            Ok(Some(status)) if !status.success() => {
                                return Err(
                                    server.startup_failure(program, ServerStartupProblem::Exited)
                                );
                            }
                            Ok(Some(_)) => break,
                            Err(error) => {
                                return Err(server.startup_failure(
                                    program,
                                    ServerStartupProblem::ReadError(error.to_string()),
                                ));
                            }
                            Ok(None) if std::time::Instant::now() < settle => {
                                std::thread::sleep(std::time::Duration::from_millis(1));
                            }
                            Ok(None) => break,
                        }
                    }
                }
                let problem = match handshake {
                    ServerHandshake::Eof => ServerStartupProblem::PrematureEof,
                    ServerHandshake::Oversized => ServerStartupProblem::OversizedPort,
                    ServerHandshake::ReadError(error) => ServerStartupProblem::ReadError(error),
                    ServerHandshake::Line(_) => unreachable!(),
                };
                return Err(server.startup_failure(program, problem));
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                return Err(server.startup_failure(program, ServerStartupProblem::PrematureEof));
            }
        }
    }
}

fn start_server_commands(
    commands: impl IntoIterator<Item = Command>,
    limits: ServerStartupLimits,
) -> Result<(HttpServer, u16)> {
    let mut absent = Vec::new();
    for mut command in commands {
        let program = command.get_program().to_owned();
        command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let child = match command.spawn() {
            Ok(child) => child,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                if let Some(root) = command.get_current_dir()
                    && !root.is_dir()
                {
                    return Err(error).with_context(|| {
                        format!("registry directory {} unavailable", root.display())
                    });
                }
                absent.push(program);
                continue;
            }
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("spawning local registry interpreter {program:?}"));
            }
        };
        let (server, receiver) = HttpServer::new(child, limits)?;
        return wait_for_server(server, receiver, program, limits);
    }
    bail!("local registry interpreter not found; tried {absent:?} (python3 or python required)")
}

fn start_http_server(registry_root: &Path) -> Result<(HttpServer, u16)> {
    let registry_root = fs::canonicalize(registry_root)
        .with_context(|| format!("canonicalizing {}", registry_root.display()))?;
    // The root stays filesystem data. No display/lossy conversion or source
    // interpolation; current_dir preserves native platform path boundaries.
    const SCRIPT: &str = r#"import http.server, socketserver
with socketserver.TCPServer(("127.0.0.1", 0), http.server.SimpleHTTPRequestHandler) as httpd:
    print(httpd.server_address[1], flush=True)
    httpd.serve_forever()
"#;
    start_server_commands(
        ["python3", "python"].map(|python| {
            let mut command = Command::new(python);
            command
                .arg("-u")
                .arg("-c")
                .arg(SCRIPT)
                .current_dir(&registry_root);
            command
        }),
        ServerStartupLimits::default(),
    )
}

impl Drop for IsolatedRegistry {
    fn drop(&mut self) {
        // Stop the server before the registry directories are removed.
        self.http_server.take();
    }
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

    let expected_version = metadata
        .packages
        .iter()
        .find(|pkg| pkg.name == crate_name)
        .with_context(|| format!("crate `{crate_name}` missing from cargo metadata"))?
        .version
        .clone();
    let expected_file = format!("{crate_name}-{expected_version}.crate");
    let package_dir = target_dir.join("package");
    let crate_path = fs::read_dir(&package_dir)
        .with_context(|| format!("reading {}", package_dir.display()))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name == expected_file)
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

const DEPENDENCY_TABLE_NAMES: [&str; 3] =
    ["dependencies", "dev-dependencies", "build-dependencies"];

/// The package a dependency spec actually refers to: the `package` field
/// for renamed dependencies, otherwise the TOML key.
fn dependency_actual_package_name<'a>(key: &'a str, spec: &'a toml::Value) -> &'a str {
    spec.as_table()
        .and_then(|table| table.get("package"))
        .and_then(toml::Value::as_str)
        .unwrap_or(key)
}

/// Every dependency table in a manifest: the top-level normal/build/dev
/// tables plus target-specific ones.
fn dependency_tables(
    root: &toml::map::Map<String, toml::Value>,
) -> Result<Vec<&toml::map::Map<String, toml::Value>>> {
    let mut tables = Vec::new();
    for name in DEPENDENCY_TABLE_NAMES {
        if let Some(value) = root.get(name) {
            tables.push(
                value
                    .as_table()
                    .with_context(|| format!("`{name}` must be a dependency table"))?,
            );
        }
    }
    if let Some(value) = root.get("target") {
        let targets = value.as_table().context("`target` must be a table")?;
        for (target, value) in targets {
            let target_table = value
                .as_table()
                .with_context(|| format!("`target.{target}` must be a table"))?;
            for name in DEPENDENCY_TABLE_NAMES {
                if let Some(value) = target_table.get(name) {
                    tables.push(value.as_table().with_context(|| {
                        format!("`target.{target}.{name}` must be a dependency table")
                    })?);
                }
            }
        }
    }
    for table in &tables {
        for (name, spec) in *table {
            match spec {
                toml::Value::String(_) => {}
                toml::Value::Table(fields) => {
                    for key in ["package", "version", "registry", "path"] {
                        if let Some(value) = fields.get(key)
                            && !value.is_str()
                        {
                            bail!("dependency `{name}` field `{key}` must be a string");
                        }
                    }
                }
                _ => bail!("dependency `{name}` must be a version string or table"),
            }
        }
    }
    Ok(tables)
}

fn manifest_contains_path_dependency(manifest: &str) -> bool {
    // Structural scan over every dependency table. Unparseable manifests
    // fail closed: a receipt must never pass on content it cannot inspect.
    let Ok(value) = manifest.parse::<toml::Value>() else {
        return true;
    };
    let Some(root) = value.as_table() else {
        return true;
    };
    let Ok(tables) = dependency_tables(root) else {
        return true;
    };
    tables
        .into_iter()
        .flat_map(|deps| deps.values())
        .any(|spec| {
            spec.as_table()
                .is_some_and(|table| table.contains_key("path"))
        })
}

/// Expected co-release pins: package name to the exact published version
/// from the single release graph (via cargo metadata, not a second list).
fn expected_release_versions(
    metadata: &CargoMetadata,
    ordered_crates: &[String],
) -> Result<BTreeMap<String, String>> {
    ordered_crates
        .iter()
        .map(|crate_name| {
            let version = metadata
                .packages
                .iter()
                .find(|pkg| pkg.name == *crate_name)
                .with_context(|| format!("crate `{crate_name}` missing from cargo metadata"))?
                .version
                .clone();
            Ok((crate_name.clone(), version))
        })
        .collect()
}

/// Structural isolation verdict over a parsed manifest: every dependency
/// table (normal, build, dev, and target-specific) is inspected. Co-release
/// edges are resolved by actual package name (`package`, else the TOML key)
/// and must carry no source-checkout path plus the exact local
/// version/registry. Unparseable manifests fail closed. Empty means isolated.
fn manifest_isolation_violations(
    manifest: &str,
    expected: &BTreeMap<String, String>,
) -> Vec<String> {
    let Ok(value) = manifest.parse::<toml::Value>() else {
        return vec!["manifest is not parseable TOML; failing closed".to_string()];
    };
    let Some(root) = value.as_table() else {
        return vec!["manifest root is not a TOML table; failing closed".to_string()];
    };
    let mut violations = Vec::new();
    let tables = match dependency_tables(root) {
        Ok(tables) => tables,
        Err(error) => return vec![format!("invalid dependency structure: {error:#}")],
    };
    for deps in tables {
        for (name, spec) in deps {
            if spec
                .as_table()
                .is_some_and(|table| table.contains_key("path"))
            {
                violations.push(format!(
                    "dependency `{name}` retains a path dependency; expected an `{REGISTRY_NAME}` registry pin"
                ));
            }
            let actual = dependency_actual_package_name(name, spec);
            let Some(expected_version) = expected.get(actual) else {
                continue;
            };
            if let Err(error) = reject_git_source_for_registry_pin(spec) {
                violations.push(format!("dependency `{name}` (package `{actual}`): {error}"));
                continue;
            }
            let (registry, version) = dependency_registry_and_version(spec);
            let exact_requirement = format!("={expected_version}");
            if registry == Some(REGISTRY_NAME) && version == Some(exact_requirement.as_str()) {
                continue;
            }
            violations.push(format!(
                "dependency `{name}` (package `{actual}`) must pin version \
                 \"={expected_version}\" from registry \"{REGISTRY_NAME}\"; found registry {} version {}",
                registry.map_or("missing".to_string(), |r| format!("\"{r}\"")),
                version.map_or("missing".to_string(), |v| format!("\"{v}\"")),
            ));
        }
    }
    violations
}

fn dependency_registry_and_version(spec: &toml::Value) -> (Option<&str>, Option<&str>) {
    match spec {
        toml::Value::String(version) => (None, Some(version.as_str())),
        toml::Value::Table(table) => (
            table.get("registry").and_then(toml::Value::as_str),
            table.get("version").and_then(toml::Value::as_str),
        ),
        _ => (None, None),
    }
}

fn inject_registry_deps_into_crate(
    crate_path: &Path,
    expected_versions: &BTreeMap<String, String>,
) -> Result<()> {
    let manifest_member = packaged_manifest_member(crate_path)?;
    let manifest = packaged_manifest_text(crate_path)?;
    let updated = inject_registry_into_manifest(&manifest, expected_versions)?;
    if updated == manifest {
        return Ok(());
    }
    repack_crate_with_manifest(crate_path, &manifest_member, &updated)
}

fn inject_registry_into_manifest(
    manifest: &str,
    expected_versions: &BTreeMap<String, String>,
) -> Result<String> {
    let mut value: toml::Value = manifest
        .parse()
        .context("parsing packaged manifest for registry injection")?;
    let root = value
        .as_table_mut()
        .context("packaged manifest root must be a table")?;

    dependency_tables(root)?;
    for table_name in DEPENDENCY_TABLE_NAMES {
        let Some(deps) = root.get_mut(table_name).and_then(toml::Value::as_table_mut) else {
            continue;
        };
        for (name, spec) in deps.iter_mut() {
            let actual = dependency_actual_package_name(name, spec).to_string();
            if let Some(version) = expected_versions.get(&actual) {
                *spec = registry_pinned_dependency_spec(spec, version)?;
            }
        }
    }

    if let Some(target) = root.get_mut("target").and_then(toml::Value::as_table_mut) {
        let target_keys = target.keys().cloned().collect::<Vec<_>>();
        for target_key in target_keys {
            let Some(target_table) = target
                .get_mut(&target_key)
                .and_then(toml::Value::as_table_mut)
            else {
                continue;
            };
            for table_name in DEPENDENCY_TABLE_NAMES {
                let Some(deps) = target_table
                    .get_mut(table_name)
                    .and_then(toml::Value::as_table_mut)
                else {
                    continue;
                };
                for (name, spec) in deps.iter_mut() {
                    let actual = dependency_actual_package_name(name, spec).to_string();
                    if let Some(version) = expected_versions.get(&actual) {
                        *spec = registry_pinned_dependency_spec(spec, version)?;
                    }
                }
            }
        }
    }

    toml::to_string(&value).context("serializing registry-pinned packaged manifest")
}

fn reject_git_source_for_registry_pin(spec: &toml::Value) -> Result<()> {
    if let Some(table) = spec.as_table() {
        for field in ["git", "rev", "branch", "tag"] {
            if table.contains_key(field) {
                bail!("co-release registry pin cannot retain Git source field `{field}`");
            }
        }
    }
    Ok(())
}

fn registry_pinned_dependency_spec(
    spec: &toml::Value,
    expected_version: &str,
) -> Result<toml::Value> {
    reject_git_source_for_registry_pin(spec)?;
    let mut table = match spec {
        toml::Value::String(version) => {
            let mut table = toml::map::Map::new();
            table.insert("version".to_string(), toml::Value::String(version.clone()));
            table
        }
        toml::Value::Table(table) => table.clone(),
        _ => bail!("unsupported dependency spec in packaged manifest"),
    };
    let original = table
        .get("version")
        .and_then(toml::Value::as_str)
        .context("co-release dependency must declare a version requirement")?;
    let requirement =
        semver::VersionReq::parse(original).context("invalid co-release version requirement")?;
    let version =
        semver::Version::parse(expected_version).context("invalid metadata package version")?;
    if !requirement.matches(&version) {
        bail!(
            "co-release requirement `{original}` does not admit selected local version `{expected_version}`"
        );
    }
    table.insert(
        "version".to_string(),
        toml::Value::String(format!("={expected_version}")),
    );
    // A registry pin must not carry a source-checkout path: the rewritten
    // edge resolves from the local registry, never from a relative path.
    table.remove("path");
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
    #[serde(default)]
    links: Option<String>,
    features: BTreeMap<String, Vec<String>>,
    dependencies: Vec<CargoDependency>,
}

#[derive(Debug, Clone, Deserialize)]
struct CargoDependency {
    name: String,
    #[serde(default)]
    rename: Option<String>,
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

fn split_index_features(
    features: &BTreeMap<String, Vec<String>>,
) -> (BTreeMap<String, Vec<String>>, BTreeMap<String, Vec<String>>) {
    let mut v1 = BTreeMap::new();
    let mut v2 = BTreeMap::new();
    for (name, values) in features {
        if values
            .iter()
            .any(|v| v.contains("dep:") || v.contains("?/"))
        {
            v2.insert(name.clone(), values.clone());
        } else {
            v1.insert(name.clone(), values.clone());
        }
    }
    (v1, v2)
}

fn write_sparse_index_entry(
    index_dir: &Path,
    package: &CargoPackage,
    expected_versions: &BTreeMap<String, String>,
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
        .map(|dep| index_dependency(dep, expected_versions))
        .collect::<Result<Vec<_>>>()?;

    let mut entry = Map::new();
    entry.insert("name".to_string(), Value::String(package.name.clone()));
    entry.insert("vers".to_string(), Value::String(package.version.clone()));
    entry.insert(
        "deps".to_string(),
        serde_json::to_value(deps).context("serializing index dependencies")?,
    );
    entry.insert("cksum".to_string(), Value::String(checksum.to_string()));
    let (features, features2) = split_index_features(&package.features);
    entry.insert(
        "features".to_string(),
        serde_json::to_value(&features).context("serializing crate features")?,
    );
    if !features2.is_empty() {
        entry.insert(
            "features2".to_string(),
            serde_json::to_value(&features2).context("serializing crate features2")?,
        );
    }
    if let Some(links) = &package.links {
        entry.insert("links".to_string(), Value::String(links.clone()));
    }
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
    let lower = crate_name.to_ascii_lowercase();
    match lower.len() {
        1 => index_dir.join("1").join(&lower),
        2 => index_dir.join("2").join(&lower),
        3 => index_dir.join("3").join(&lower[..1]).join(&lower),
        _ => index_dir.join(&lower[..2]).join(&lower[2..4]).join(&lower),
    }
}

fn dependency_counts_for_index(dep: &CargoDependency) -> bool {
    // The index must list dev deps too: features may reference them
    // (e.g. `ts-compat = ["adze-example/ts-compat"]`), and Cargo rejects
    // an entry whose features name a dep that is not in `deps`.
    matches!(
        dep.kind.as_deref(),
        None | Some("normal") | Some("build") | Some("dev")
    )
}

fn index_dependency(
    dep: &CargoDependency,
    expected_versions: &BTreeMap<String, String>,
) -> Result<Value> {
    let mut dep_entry = Map::new();
    let index_name = dep.rename.as_ref().unwrap_or(&dep.name);
    dep_entry.insert("name".to_string(), Value::String(index_name.clone()));
    let requirement = if let Some(version) = expected_versions.get(&dep.name) {
        let expected =
            semver::Version::parse(version).context("invalid metadata package version")?;
        if !semver::VersionReq::parse(&dep.req)
            .context("invalid index dependency requirement")?
            .matches(&expected)
        {
            bail!(
                "index requirement `{}` does not admit selected local version `{version}`",
                dep.req
            );
        }
        format!("={version}")
    } else {
        dep.req.clone()
    };
    dep_entry.insert("req".to_string(), Value::String(requirement));
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
    if expected_versions.contains_key(&dep.name) {
        dep_entry.insert("registry".to_string(), Value::Null);
    } else {
        dep_entry.insert(
            "registry".to_string(),
            Value::String("https://github.com/rust-lang/crates.io-index".to_string()),
        );
    }
    if dep.rename.is_some() {
        dep_entry.insert("package".to_string(), Value::String(dep.name.clone()));
    } else {
        dep_entry.insert("package".to_string(), Value::Null);
    }
    Ok(Value::Object(dep_entry))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_install_executes_locked_request_with_isolated_roots() {
        let mut registry = IsolatedRegistry::new().expect("isolated registry");
        registry.published.push(PublishedCrate {
            name: "adze-cli".into(),
            version: "0.10.0".into(),
            checksum: "fixture".into(),
        });
        // Exercise the real install method without packaging or running Cargo.
        // A child checks argv boundaries and creates the expected installed file.
        let mut command = fake_server_command(
            r#"import os, pathlib, sys
args = sys.argv[1:]
assert args[:-1] == ['install', '--locked', '--registry', 'adze-local', 'adze-cli', '--bin', 'adze fixture', '--version', '0.10.0', '--root'], args
assert os.environ['CARGO_HOME'] == os.environ['EXPECTED_CARGO_HOME']
assert os.environ['CARGO_TARGET_DIR'] == os.environ['EXPECTED_TARGET_DIR']
binary = pathlib.Path(args[-1]) / 'bin' / ('adze fixture.exe' if os.name == 'nt' else 'adze fixture')
binary.parent.mkdir()
binary.write_text('installed fixture')
"#,
        );
        command
            .env("EXPECTED_CARGO_HOME", registry.cargo_home.path())
            .env("EXPECTED_TARGET_DIR", registry.target_dir.path());
        let install = registry
            .install_cli_with_command("adze-cli", "adze fixture", "0.10.0", command)
            .expect("locked install request");
        assert_eq!(fs::read_to_string(install.binary_path).unwrap(), "installed fixture");

        let error = registry
            .install_cli_with_command(
                "adze-cli",
                "adze fixture",
                "0.10.0",
                fake_server_command("import sys; sys.exit(23)"),
            )
            .err()
            .expect("nonzero installer must fail");
        assert!(error.to_string().contains("23"), "{error:#}");
    }

    fn fake_server_command(script: &str) -> Command {
        for python in ["python3", "python"] {
            match Command::new(python).arg("--version").output() {
                Ok(output) if output.status.success() => {
                    let mut command = Command::new(python);
                    command.args(["-u", "-c", script]);
                    return command;
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                outcome => panic!("Python probe failed: {outcome:?}"),
            }
        }
        panic!("Python is required for registry process controls");
    }

    fn server_test_limits() -> ServerStartupLimits {
        ServerStartupLimits {
            timeout: std::time::Duration::from_secs(2),
            port_line_bytes: 16,
            stderr_bytes: 1024,
        }
    }

    fn failed_server(script: &str) -> ServerStartupError {
        failed_server_with_limits(script, server_test_limits())
    }

    fn failed_server_with_limits(script: &str, limits: ServerStartupLimits) -> ServerStartupError {
        let error = match start_server_commands([fake_server_command(script)], limits) {
            Ok(_) => panic!("fake startup unexpectedly succeeded"),
            Err(error) => error,
        };
        let error = error
            .downcast::<ServerStartupError>()
            .expect("structured started-child failure");
        assert_child_reaped(error.pid);
        error
    }

    fn assert_child_reaped(pid: u32) {
        #[cfg(target_os = "linux")]
        {
            // Query the child relationship through the same process API as
            // Child::wait. /proc may describe a different PID namespace.
            unsafe extern "C" {
                fn waitpid(pid: i32, status: *mut i32, options: i32) -> i32;
            }
            let mut status = 0;
            // SAFETY: status is writable; WNOHANG (1 on Linux) never blocks.
            let result = unsafe { waitpid(pid as i32, &mut status, 1) };
            assert_eq!(result, -1, "owned child {pid} was not already reaped");
            assert_eq!(
                std::io::Error::last_os_error().raw_os_error(),
                Some(10),
                "ECHILD establishes the child was already reaped"
            );
        }
        #[cfg(not(target_os = "linux"))]
        let _ = pid;
    }

    #[test]
    fn test_http_server_nonzero_exit_retains_real_status_and_stderr() {
        let error = failed_server(
            "import sys; sys.stderr.write('startup exploded'); sys.stderr.flush(); sys.exit(73)",
        );
        assert!(matches!(error.problem, ServerStartupProblem::Exited));
        assert_eq!(error.status.unwrap().code(), Some(73));
        assert_eq!(error.stderr.bytes, b"startup exploded");
        assert!(error.stderr.complete);
        let rendered = error.to_string();
        assert!(rendered.contains("startup exploded"));
        assert!(rendered.contains("exit status"));
        assert!(!rendered.contains("interpreter not found"));
    }

    #[test]
    fn test_http_server_malformed_and_zero_ports_fail_closed() {
        for port in ["not-a-port", "0", "65536", "+123", " 123", "12 34"] {
            let script = format!("import time; print({port:?}, flush=True); time.sleep(10)");
            let error = failed_server(&script);
            assert!(
                matches!(error.problem, ServerStartupProblem::MalformedPort(_)),
                "{error}"
            );
            assert!(error.status.is_some());
        }
    }

    #[test]
    fn test_http_server_oversized_unterminated_port_line_is_bounded() {
        let started = std::time::Instant::now();
        let error = failed_server(
            "import sys,time; sys.stdout.write('1'*100000); sys.stdout.flush(); time.sleep(10)",
        );
        assert!(matches!(error.problem, ServerStartupProblem::OversizedPort));
        assert!(started.elapsed() < std::time::Duration::from_secs(5));
    }

    #[test]
    fn test_http_server_premature_eof_is_distinct_from_timeout_and_nonzero() {
        let error = failed_server("import os,time; os.close(1); time.sleep(10)");
        assert!(matches!(error.problem, ServerStartupProblem::PrematureEof));
        let error = failed_server("pass");
        assert!(matches!(
            error.problem,
            ServerStartupProblem::Exited | ServerStartupProblem::PrematureEof
        ));
        assert_eq!(error.status.unwrap().code(), Some(0));
    }

    #[test]
    fn test_http_server_no_handshake_timeout_is_bounded() {
        let started = std::time::Instant::now();
        let limits = ServerStartupLimits {
            timeout: std::time::Duration::from_millis(150),
            ..server_test_limits()
        };
        let error = failed_server_with_limits("import time; time.sleep(10)", limits);
        assert!(matches!(error.problem, ServerStartupProblem::Timeout));
        assert!(started.elapsed() < std::time::Duration::from_secs(2));
    }

    #[test]
    fn test_http_server_drains_and_caps_stderr_before_protocol_failure() {
        let error = failed_server(
            "import sys,time; sys.stderr.write('diagnostic-'+'e'*200000); sys.stderr.flush(); print('invalid',flush=True); time.sleep(10)",
        );
        assert!(matches!(
            error.problem,
            ServerStartupProblem::MalformedPort(_)
        ));
        assert_eq!(error.stderr.bytes.len(), server_test_limits().stderr_bytes);
        assert!(error.stderr.bytes.starts_with(b"diagnostic-"));
        assert!(error.stderr.truncated);
        assert!(error.stderr.complete);
        assert!(error.to_string().contains("truncated"));
    }

    #[test]
    fn test_http_server_only_missing_executable_uses_fallback() {
        let directory = tempfile::tempdir().unwrap();
        let absent = directory.path().join("no-such-interpreter");
        let error = match start_server_commands([Command::new(&absent)], server_test_limits()) {
            Ok(_) => panic!("missing interpreter succeeded"),
            Err(error) => error,
        };
        assert!(error.to_string().contains("interpreter not found"));
        let (server, port) = start_server_commands(
            [
                Command::new(&absent),
                fake_server_command("import time; print(12345,flush=True); time.sleep(10)"),
            ],
            server_test_limits(),
        )
        .unwrap();
        assert_eq!(port, 12345);
        let pid = server.child.id();
        drop(server);
        assert_child_reaped(pid);
    }

    #[test]
    fn test_http_server_started_failure_never_tries_fallback() {
        let directory = tempfile::tempdir().unwrap();
        let marker = directory.path().join("fallback-ran");
        let mut fallback = fake_server_command(
            "import pathlib,sys,time; pathlib.Path(sys.argv[1]).write_text('wrong'); print(12345,flush=True); time.sleep(10)",
        );
        fallback.arg(&marker);
        let result = start_server_commands(
            [
                fake_server_command("import sys; sys.stderr.write('primary failed'); sys.exit(19)"),
                fallback,
            ],
            server_test_limits(),
        );
        let error = match result {
            Ok(_) => panic!("started failure used fallback"),
            Err(error) => error,
        };
        let error = error.downcast::<ServerStartupError>().unwrap();
        assert_eq!(error.status.unwrap().code(), Some(19));
        assert_eq!(error.stderr.bytes, b"primary failed");
        assert!(!marker.exists());
        assert_child_reaped(error.pid);
    }

    #[test]
    fn test_http_server_success_and_early_guard_drop_reap_owned_child() {
        let mut command =
            fake_server_command("import time; print(12345,flush=True); time.sleep(10)");
        command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let child = command.spawn().unwrap();
        let pid = child.id();
        let (server, receiver) = HttpServer::new(child, server_test_limits()).unwrap();
        drop(receiver); // Startup cancellation while the owner is still live.
        drop(server);
        assert_child_reaped(pid);

        let (server, port) = start_server_commands([
            fake_server_command("import sys,time; print(12345,flush=True); sys.stdout.write('x'*200000); sys.stdout.flush(); sys.stderr.write('stdout drained'); sys.stderr.flush(); time.sleep(10)"),
        ], server_test_limits()).unwrap();
        assert_eq!(port, 12345);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        loop {
            if server.stderr.lock().unwrap().bytes == b"stdout drained" {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "post-handshake stdout was not drained"
            );
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        let pid = server.child.id();
        drop(server);
        assert_child_reaped(pid);
    }

    #[cfg(unix)]
    #[test]
    fn test_http_server_permission_failure_does_not_try_fallback() {
        use std::os::unix::fs::PermissionsExt;
        let directory = tempfile::tempdir().unwrap();
        let denied = directory.path().join("not-executable");
        fs::write(&denied, b"not an executable").unwrap();
        fs::set_permissions(&denied, fs::Permissions::from_mode(0o600)).unwrap();
        let error = match start_server_commands(
            [
                Command::new(&denied),
                fake_server_command("import time; print(12345,flush=True); time.sleep(10)"),
            ],
            server_test_limits(),
        ) {
            Ok(_) => panic!("permission failure used fallback"),
            Err(error) => error,
        };
        assert_eq!(
            error.downcast_ref::<std::io::Error>().unwrap().kind(),
            std::io::ErrorKind::PermissionDenied
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_http_server_non_utf8_path_preserves_selected_directory() {
        use std::os::unix::ffi::OsStringExt;
        let parent = tempfile::tempdir().unwrap();
        let root = parent
            .path()
            .join(std::ffi::OsString::from_vec(b"native-\xff-root".to_vec()));
        fs::create_dir(&root).unwrap();
        fs::write(root.join("sentinel.txt"), b"native selected root").unwrap();
        let (server, port) = start_http_server(&root).unwrap();
        let pid = server.child.id();
        let response = registry_http_get(port, "/sentinel.txt");
        assert!(response.starts_with("HTTP/1.0 200"), "{response}");
        assert!(response.ends_with("native selected root"), "{response}");
        drop(server);
        assert_child_reaped(pid);
    }

    fn registry_http_get(port: u16, path: &str) -> String {
        let mut connection = std::net::TcpStream::connect_timeout(
            &std::net::SocketAddr::from((std::net::Ipv4Addr::LOCALHOST, port)),
            std::time::Duration::from_secs(2),
        )
        .expect("loopback registry connection");
        connection
            .set_read_timeout(Some(std::time::Duration::from_secs(2)))
            .unwrap();
        connection
            .set_write_timeout(Some(std::time::Duration::from_secs(2)))
            .unwrap();
        write!(connection, "GET {path} HTTP/1.0\r\nHost: localhost\r\n\r\n").unwrap();
        let mut response = String::new();
        connection.read_to_string(&mut response).unwrap();
        response
    }

    #[test]
    fn test_http_server_config_write_failure_allows_successful_retry() {
        for block_cargo_config in [true, false] {
            let mut registry = IsolatedRegistry::new().unwrap();
            let blocker = if block_cargo_config {
                registry.cargo_home.path().join("config.toml")
            } else {
                registry.index_dir.join("config.json")
            };
            // A directory cannot be overwritten as a regular file on either
            // Unix or Windows; this does not depend on permission/root behavior.
            fs::create_dir(&blocker).unwrap();
            let error = registry
                .activate_http_registry()
                .expect_err("blocked config file must fail");
            assert!(error.to_string().contains("writing"), "{error:#}");
            fs::remove_dir(&blocker).unwrap();

            let index_url = registry
                .activate_http_registry()
                .expect("failed configuration must not leave an active server blocking retry");
            let server = registry
                .http_server
                .as_ref()
                .expect("successful activation owns server");
            let pid = server.child.id();
            let port = index_url
                .strip_prefix("sparse+http://127.0.0.1:")
                .unwrap()
                .strip_suffix("/index/")
                .unwrap()
                .parse::<u16>()
                .unwrap();
            let response = registry_http_get(port, "/index/config.json");
            assert!(response.starts_with("HTTP/1.0 200"), "{response}");
            assert!(
                response.contains(&format!("http://127.0.0.1:{port}/crates/")),
                "{response}"
            );
            let cargo_config =
                fs::read_to_string(registry.cargo_home.path().join("config.toml")).unwrap();
            assert!(cargo_config.contains(&index_url));
            drop(registry);
            assert_child_reaped(pid);
        }
    }

    #[cfg(unix)]
    #[test]
    fn test_http_server_quote_unicode_path_serves_selected_root() {
        let parent = tempfile::tempdir().expect("temporary registry parent");
        let root = parent.path().join("registry \"quoted\" été");
        fs::create_dir(&root).expect("legal Unix path");
        fs::write(root.join("sentinel.txt"), b"selected registry root").expect("registry sentinel");

        let (child, port) = start_http_server(&root).expect("path is data, not Python source");
        let pid = child.child.id();
        let response = registry_http_get(port, "/sentinel.txt");
        assert!(response.starts_with("HTTP/1.0 200"), "{response}");
        assert!(response.ends_with("selected registry root"), "{response}");
        drop(child);
        assert_child_reaped(pid);
    }

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
    fn manifest_path_detection_flags_target_specific_path_dependency() {
        // #1000 reproduced fixture: the old line scanner never entered
        // target-specific tables.
        let manifest = r#"
[target.'cfg(windows)'.dependencies]
adze = { path = '../runtime' }
"#;
        assert!(manifest_contains_path_dependency(manifest));
    }

    #[test]
    fn manifest_path_detection_flags_compact_single_quoted_path() {
        // #1000 reproduced fixture: no spaces around `=`.
        let manifest = "[dependencies]\nadze = {path='../runtime'}\n";
        assert!(manifest_contains_path_dependency(manifest));
    }

    fn expected_versions_for(names: &[&str], version: &str) -> BTreeMap<String, String> {
        names
            .iter()
            .map(|name| ((*name).to_string(), version.to_string()))
            .collect()
    }

    #[test]
    fn test_isolation_violations_fail_closed_on_malformed_manifest() {
        let expected = expected_versions_for(&["adze"], "0.10.0");
        assert!(manifest_contains_path_dependency("[dependencies\nadze = {"));
        assert!(!manifest_isolation_violations("[dependencies\nadze = {", &expected).is_empty());
    }

    #[test]
    fn test_isolation_violations_reject_reproduced_path_fixtures() {
        // #1000 reproduced fixtures, independent of whitespace/quote style.
        let expected = expected_versions_for(&["adze"], "0.10.0");
        for manifest in [
            "[target.'cfg(windows)'.dependencies]\nadze = { path = '../runtime' }\n",
            "[dependencies]\nadze = {path='../runtime'}\n",
        ] {
            let violations = manifest_isolation_violations(manifest, &expected);
            assert!(
                violations.iter().any(|v| v.contains("path")),
                "{manifest}: {violations:?}"
            );
        }
    }

    #[test]
    fn test_isolation_violations_reject_wrong_version_pin() {
        // #1000: a registry pin at the wrong version must not satisfy
        // isolation. RED-BEFORE: the registry-only check accepted this.
        let expected = expected_versions_for(&["adze"], "0.10.0");
        let manifest = r#"
[dependencies]
adze = { version = "0.0.1", registry = "adze-local" }
"#;
        let violations = manifest_isolation_violations(manifest, &expected);
        assert!(
            violations.iter().any(|v| v.contains("0.0.1")),
            "{violations:?}"
        );
    }

    #[test]
    fn test_isolation_violations_reject_wrong_registry_and_missing_pin() {
        let expected = expected_versions_for(&["adze"], "0.10.0");
        let wrong_registry = r#"
[dependencies]
adze = { version = "0.10.0", registry = "crates-io" }
"#;
        let violations = manifest_isolation_violations(wrong_registry, &expected);
        assert!(!violations.is_empty(), "wrong registry must be rejected");
        let missing_pin = r#"
[dependencies]
adze = "0.10.0"
"#;
        assert!(!manifest_isolation_violations(missing_pin, &expected).is_empty());
    }

    #[test]
    fn test_isolation_violations_resolve_renamed_dependencies() {
        let expected = expected_versions_for(&["adze"], "0.10.0");
        let published = expected_versions_for(&["adze"], "0.10.0");
        let renamed_unpinned = r#"
[dependencies]
adze-alias = { package = "adze", version = "0.10.0" }
"#;
        assert!(!manifest_isolation_violations(renamed_unpinned, &expected).is_empty());
        // Injection pins by actual package and preserves the rename.
        let pinned =
            inject_registry_into_manifest(renamed_unpinned, &published).expect("inject renamed");
        assert!(pinned.contains(r#"package = "adze""#), "{pinned}");
        assert!(
            manifest_isolation_violations(&pinned, &expected).is_empty(),
            "{pinned}"
        );
        // A rename to an external package is not a co-release edge.
        let external_rename = r#"
[dependencies]
clap-alias = { package = "clap", version = "4.5" }
"#;
        assert!(manifest_isolation_violations(external_rename, &expected).is_empty());
    }

    #[test]
    fn test_isolation_violations_check_target_specific_dependencies() {
        let expected = expected_versions_for(&["adze"], "0.10.0");
        let published = expected_versions_for(&["adze"], "0.10.0");
        let target_unpinned = r#"
[target.'cfg(windows)'.dependencies]
adze = "0.10.0"
"#;
        assert!(!manifest_isolation_violations(target_unpinned, &expected).is_empty());
        let pinned =
            inject_registry_into_manifest(target_unpinned, &published).expect("inject target");
        assert!(
            manifest_isolation_violations(&pinned, &expected).is_empty(),
            "{pinned}"
        );
    }

    #[test]
    fn test_isolation_violations_distinguish_leaking_edge_in_graph() {
        // A complete isolated graph validates clean; one misdirected edge
        // fails the receipt gate for exactly that manifest.
        let expected = expected_versions_for(&["adze", "adze-tool"], "0.10.0");
        let clean = r#"
[dependencies]
adze = { version = "=0.10.0", registry = "adze-local" }
clap = "4.5"

[build-dependencies]
adze-tool = { version = "=0.10.0", registry = "adze-local" }
"#;
        assert!(manifest_isolation_violations(clean, &expected).is_empty());
        let leaking = r#"
[dependencies]
adze = { version = "=0.10.0", registry = "adze-local" }
clap = "4.5"

[build-dependencies]
adze-tool = { version = "0.9.0", registry = "adze-local" }
"#;
        let violations = manifest_isolation_violations(leaking, &expected);
        assert_eq!(violations.len(), 1, "{violations:?}");
        assert!(violations[0].contains("adze-tool"), "{violations:?}");
    }

    #[test]
    fn test_injection_strips_path_and_validates_clean() {
        let expected = expected_versions_for(&["adze"], "0.10.0");
        let published = expected_versions_for(&["adze"], "0.10.0");
        let with_path = r#"
[dependencies]
adze = { version = "0.10.0", path = "../runtime" }
"#;
        let pinned = inject_registry_into_manifest(with_path, &published).expect("inject");
        assert!(!pinned.contains("path"), "{pinned}");
        assert!(
            manifest_isolation_violations(&pinned, &expected).is_empty(),
            "{pinned}"
        );
    }

    #[test]
    fn sparse_index_file_uses_crates_io_layout() {
        let path = sparse_index_file(Path::new("/tmp/index"), "adze-cli");
        assert_eq!(path, Path::new("/tmp/index/ad/ze/adze-cli"));
    }

    #[test]
    fn registry_config_json_uses_http_dl_url() {
        let dl_url = "http://127.0.0.1:19876/crates/";
        let config_json = registry_config_json(dl_url);
        assert!(config_json.contains(r#""dl":"http://127.0.0.1:19876/crates/""#));
    }

    #[test]
    fn test_inject_registry_into_manifest_pins_workspace_dependencies() {
        let manifest = r#"
[dependencies]
adze = "0.10.0"
clap = "4.5"
"#;
        let published = expected_versions_for(&["adze"], "0.10.0");
        let updated = inject_registry_into_manifest(manifest, &published).expect("inject");
        assert!(updated.contains(r#"registry = "adze-local""#));
        assert!(updated.contains("adze"));
        assert!(updated.contains(r#"clap = "4.5""#));
    }

    #[test]
    fn test_isolation_violations_detect_unpinned_workspace_crate() {
        let expected = expected_versions_for(&["adze"], "0.10.0");
        let published = expected_versions_for(&["adze"], "0.10.0");
        let manifest = r#"
[dependencies]
adze = "0.10.0"
"#;
        assert!(!manifest_isolation_violations(manifest, &expected).is_empty());
        let pinned = inject_registry_into_manifest(manifest, &published).expect("inject");
        assert!(
            manifest_isolation_violations(&pinned, &expected).is_empty(),
            "{pinned}"
        );
    }

    #[test]
    fn test_index_dependency_uses_null_registry_for_same_registry_deps() {
        let dep = CargoDependency {
            name: "adze".to_string(),
            rename: None,
            req: "^0.10.0".to_string(),
            features: Vec::new(),
            optional: false,
            default_features: true,
            target: None,
            kind: None,
        };
        let published = expected_versions_for(&["adze"], "0.10.0");
        let entry = index_dependency(&dep, &published).expect("index dep");
        assert_eq!(entry["registry"], Value::Null);
        assert_eq!(entry["package"], Value::Null);
    }

    #[test]
    fn index_dependency_points_external_deps_at_crates_io() {
        let dep = CargoDependency {
            name: "clap".to_string(),
            rename: None,
            req: "^4".to_string(),
            features: Vec::new(),
            optional: false,
            default_features: true,
            target: None,
            kind: None,
        };
        let published = expected_versions_for(&["adze"], "0.10.0");
        let entry = index_dependency(&dep, &published).expect("index dep");
        assert_eq!(
            entry["registry"],
            Value::String("https://github.com/rust-lang/crates.io-index".to_string())
        );
    }

    #[test]
    fn sparse_index_file_handles_short_names() {
        assert_eq!(
            sparse_index_file(Path::new("/tmp/index"), "a"),
            Path::new("/tmp/index/1/a")
        );
        assert_eq!(
            sparse_index_file(Path::new("/tmp/index"), "abc"),
            Path::new("/tmp/index/3/a/abc")
        );
    }

    #[test]
    fn index_includes_dev_deps_so_features_can_reference_them() {
        let dev = CargoDependency {
            name: "adze-example".to_string(),
            rename: None,
            req: "*".to_string(),
            features: Vec::new(),
            optional: false,
            default_features: true,
            target: None,
            kind: Some("dev".to_string()),
        };
        assert!(dependency_counts_for_index(&dev));
    }

    #[test]
    fn split_index_features_moves_dep_syntax_to_features2() {
        let features = BTreeMap::from([
            ("default".to_string(), vec!["pure-rust".to_string()]),
            (
                "tree-sitter-standard".to_string(),
                vec!["dep:tree-sitter-runtime-standard".to_string()],
            ),
            (
                "pure-rust".to_string(),
                vec!["adze-macro/pure-rust".to_string()],
            ),
        ]);
        let (v1, v2) = split_index_features(&features);
        assert!(v1.contains_key("default"));
        assert!(v1.contains_key("pure-rust"));
        assert!(v2.contains_key("tree-sitter-standard"));
    }

    #[test]
    fn sha256_hex_matches_known_vector() {
        let digest = sha256_hex(b"abc").expect("sha256");
        assert_eq!(
            digest,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
    #[test]
    fn test_review_exact_cargo_requirement_boundary() {
        let expected = expected_versions_for(&["adze"], "0.10.0");
        let broad = "[dependencies]\nadze = {version='0.10.0', registry='adze-local'}\n";
        let exact = "[dependencies]\nadze = {version='=0.10.0', registry='adze-local'}\n";
        assert!(
            !manifest_isolation_violations(broad, &expected).is_empty(),
            "caret-compatible requirement accepted as exact"
        );
        assert!(
            manifest_isolation_violations(exact, &expected).is_empty(),
            "actual exact requirement rejected"
        );
    }
    #[test]
    fn test_review_invalid_dependency_table_fails_closed() {
        let expected = expected_versions_for(&["adze"], "0.10.0");
        assert!(
            !manifest_isolation_violations("dependencies = 'invalid Cargo table'\n", &expected)
                .is_empty()
        );
    }

    #[test]
    fn test_exact_manifest_archive_and_index_requirements_match_real_cargo_metadata() {
        let fixture = tempfile::tempdir().unwrap();
        let root = fixture.path();
        fs::create_dir_all(root.join("dep/src")).unwrap();
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(
            root.join("dep/Cargo.toml"),
            "[package]\nname='adze'\nversion='0.10.0'\nedition='2021'\n",
        )
        .unwrap();
        fs::write(root.join("dep/src/lib.rs"), "").unwrap();
        fs::write(root.join("src/lib.rs"), "").unwrap();
        let manifest = "[package]\nname='isolation-consumer'\nversion='0.1.0'\nedition='2021'\n[workspace]\nmembers=['dep']\n[dependencies]\nalias={package='adze',version='0.10.0',path='dep',default-features=false}\n[target.'cfg(unix)'.build-dependencies]\nalias={package='adze',version='^0.10.0',path='dep'}\n";
        fs::write(root.join("Cargo.toml"), manifest).unwrap();
        let metadata = |root: &Path| {
            let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
            let output = Command::new(cargo)
                .args([
                    "metadata",
                    "--offline",
                    "--no-deps",
                    "--format-version",
                    "1",
                ])
                .current_dir(root)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            serde_json::from_slice::<CargoMetadata>(&output.stdout).unwrap()
        };
        let before = metadata(root);
        let expected = expected_release_versions(&before, &["adze".to_string()]).unwrap();
        let consumer = before
            .packages
            .iter()
            .find(|p| p.name == "isolation-consumer")
            .unwrap();
        assert_eq!(consumer.dependencies.len(), 2);
        assert!(consumer.dependencies.iter().all(|d| d.req == "^0.10.0"));
        let pinned = inject_registry_into_manifest(manifest, &expected).unwrap();
        assert!(manifest_isolation_violations(&pinned, &expected).is_empty());
        let mut cargo_manifest: toml::Value = pinned.parse().unwrap();
        // Cargo supplies the independent requirement parser. Only the source
        // is substituted with an owned local package, avoiding registry I/O.
        for field in ["dependencies", "build-dependencies"] {
            let table = if field == "dependencies" {
                cargo_manifest.get_mut(field).unwrap()
            } else {
                cargo_manifest
                    .get_mut("target")
                    .unwrap()
                    .get_mut("cfg(unix)")
                    .unwrap()
                    .get_mut(field)
                    .unwrap()
            };
            let edge = table.get_mut("alias").unwrap().as_table_mut().unwrap();
            edge.remove("registry");
            edge.insert("path".into(), toml::Value::String("dep".into()));
        }
        fs::write(
            root.join("Cargo.toml"),
            toml::to_string(&cargo_manifest).unwrap(),
        )
        .unwrap();
        let after = metadata(root);
        let consumer_after = after
            .packages
            .iter()
            .find(|p| p.name == "isolation-consumer")
            .unwrap();
        for dep in &consumer.dependencies {
            let index = index_dependency(dep, &expected).unwrap();
            assert_eq!(index["req"], "=0.10.0");
            assert_eq!(index["name"], "alias");
            assert_eq!(index["package"], "adze");
            assert_eq!(index["registry"], Value::Null);
            let cargo_edge = consumer_after
                .dependencies
                .iter()
                .find(|d| d.kind == dep.kind)
                .unwrap();
            assert_eq!(index["req"], cargo_edge.req);
            assert_eq!(
                index["target"],
                serde_json::to_value(&cargo_edge.target).unwrap()
            );
            assert_eq!(index["default_features"], cargo_edge.default_features);
        }
        // Exercise the same archive rewrite/readback used by publication.
        let archive_root = root.join("isolation-consumer-0.1.0");
        fs::create_dir(&archive_root).unwrap();
        fs::write(archive_root.join("Cargo.toml"), manifest).unwrap();
        let archive = root.join("consumer.crate");
        assert!(
            Command::new("tar")
                .arg("-czf")
                .arg(&archive)
                .arg("-C")
                .arg(root)
                .arg("isolation-consumer-0.1.0")
                .status()
                .unwrap()
                .success()
        );
        inject_registry_deps_into_crate(&archive, &expected).unwrap();
        let packaged = packaged_manifest_text(&archive).unwrap();
        assert_eq!(packaged, pinned);
        assert!(manifest_isolation_violations(&packaged, &expected).is_empty());
    }

    #[test]
    fn test_invalid_dependency_shapes_and_pin_fields_refuse_isolation_and_rewrite() {
        let expected = expected_versions_for(&["adze"], "0.10.0");
        for manifest in [
            "build-dependencies=[]",
            "dev-dependencies=3",
            "target=[]",
            "[target]\nunix=3",
            "[target.'cfg(unix)']\ndependencies='not a table'",
            "[dependencies]\nadze=3",
            "[dependencies]\nadze={version='=0.10.0',registry='adze-local',package=3}",
            "[dependencies]\nadze={version=10,registry='adze-local'}",
        ] {
            assert!(
                !manifest_isolation_violations(manifest, &expected).is_empty(),
                "{manifest}"
            );
            assert!(
                inject_registry_into_manifest(manifest, &expected).is_err(),
                "{manifest}"
            );
        }
        for requirement in ["0.9.0", "=0.10.1", "not-semver"] {
            let manifest =
                format!("[dependencies]\nadze={{version='{requirement}',registry='adze-local'}}");
            assert!(
                inject_registry_into_manifest(&manifest, &expected).is_err(),
                "{manifest}"
            );
        }
        let already_local_path =
            "[dependencies]\nadze={version='=0.10.0',registry='adze-local',path='../runtime'}";
        let pinned = inject_registry_into_manifest(already_local_path, &expected).unwrap();
        assert!(manifest_isolation_violations(&pinned, &expected).is_empty());
        assert!(!pinned.contains("path"));
    }
    #[test]
    fn test_git_registry_source_conflict_matches_real_cargo_refusal() {
        let fixture = tempfile::tempdir().unwrap();
        fs::create_dir_all(fixture.path().join("src")).unwrap();
        fs::create_dir_all(fixture.path().join(".cargo")).unwrap();
        fs::write(fixture.path().join("src/lib.rs"), "").unwrap();
        fs::write(
            fixture.path().join(".cargo/config.toml"),
            "[registries.adze-local]\nindex='sparse+http://127.0.0.1:9/index/'\n",
        )
        .unwrap();
        let manifest = "[package]\nname='source-conflict-probe'\nversion='0.0.0'\nedition='2021'\n[dependencies]\nadze={version='=0.10.0',registry='adze-local',git='https://example.invalid/repo'}\n";
        fs::write(fixture.path().join("Cargo.toml"), manifest).unwrap();
        let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
        let output = Command::new(cargo)
            .args([
                "metadata",
                "--offline",
                "--no-deps",
                "--format-version",
                "1",
            ])
            .env("CARGO_NET_OFFLINE", "true")
            .current_dir(fixture.path())
            .output()
            .unwrap();
        assert!(!output.status.success());
        let error = String::from_utf8_lossy(&output.stderr);
        assert!(
            error.contains("Only one of `git` or `registry` is allowed"),
            "{error}"
        );
        let expected = expected_versions_for(&["adze"], "0.10.0");
        let violations = manifest_isolation_violations(manifest, &expected);
        let rewrite = inject_registry_into_manifest(manifest, &expected);
        assert!(
            !violations.is_empty() && rewrite.is_err(),
            "validator: {violations:?}; rewrite: {rewrite:?}"
        );
    }

    #[test]
    fn test_git_source_selectors_refused_across_corelease_dependency_tables() {
        let expected = expected_versions_for(&["adze"], "0.10.0");
        for table in [
            "dependencies",
            "build-dependencies",
            "dev-dependencies",
            "target.'cfg(unix)'.dependencies",
            "target.'cfg(unix)'.build-dependencies",
        ] {
            for (field, value) in [
                ("git", "https://example.invalid/repo"),
                ("rev", "deadbeef"),
                ("branch", "main"),
                ("tag", "v0.10.0"),
            ] {
                let manifest = format!(
                    "[{table}]\nalias={{package='adze',version='=0.10.0',registry='adze-local',{field}='{value}'}}\n"
                );
                let violations = manifest_isolation_violations(&manifest, &expected);
                assert!(
                    violations.iter().any(|v| v.contains(field)),
                    "{manifest}: {violations:?}"
                );
                let error = inject_registry_into_manifest(&manifest, &expected).unwrap_err();
                assert!(error.to_string().contains(field), "{manifest}: {error}");
            }
        }
        let external =
            "[dependencies]\nexternal={git='https://example.invalid/other',rev='deadbeef'}\n";
        assert!(manifest_isolation_violations(external, &expected).is_empty());
        let rewritten = inject_registry_into_manifest(external, &expected).unwrap();
        let before: toml::Value = external.parse().unwrap();
        let after: toml::Value = rewritten.parse().unwrap();
        assert_eq!(after, before);
    }
}
