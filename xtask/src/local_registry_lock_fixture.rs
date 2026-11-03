//! Two literal fixture crates test Cargo's transformed-lock/install assumptions.
//!
//! These are hand-built archive fixtures, not `cargo package` or release-graph
//! qualification. The existing extractor, manifest interpretation/index writer
//! and HTTP owner remain the authorities used here. This is not the production
//! command journal or a process-custody backend. Native execution is required.

use super::super::*;
use super::super::super::{
    HttpServer, REGISTRY_NAME, inject_registry_into_manifest, installed_binary_path,
    sparse_index_file, start_http_server, validate_served_metadata, write_registry_config,
    write_sparse_index_entry,
};
use anyhow::ensure;
use serde_json::{Value, json};
use std::ffi::{OsStr, OsString};
use std::io::Read;
use std::process::{Child, ExitStatus, Stdio};
use std::time::{Duration, Instant};

const DEP: &str = "adze-provenance-fixture-dep";
const CLI: &str = "adze-provenance-fixture-cli";
const BIN: &str = "adze-provenance-fixture";
const VERSION: &str = "1.0.0";
const CHANNEL_LIMIT: u64 = 4 * 1024 * 1024;
const CONTROLLER_BUDGET: Duration = Duration::from_secs(180);
const COMMAND_BUDGET: Duration = Duration::from_secs(60);
const STOP_BUDGET: Duration = Duration::from_secs(2);
const TARGET_ENTRY_LIMIT: usize = 512;
const TARGET_FILE_LIMIT: u64 = 64 * 1024 * 1024;
const TARGET_TOTAL_LIMIT: u64 = 256 * 1024 * 1024;

fn retained_directory(prefix: &str) -> Result<PathBuf> {
    let mut builder = tempfile::Builder::new();
    builder.prefix(prefix);
    let directory = if let Some(root) = std::env::var_os("RUNNER_TEMP") {
        builder.tempdir_in(root)?
    } else {
        builder.tempdir()?
    };
    Ok(directory.keep())
}

/// One immutable request is used for invocation and the fixture intent record.
/// Only explicit, nonsecret fixture environment changes are serialized.
struct Request {
    name: &'static str,
    program: OsString,
    args: Vec<OsString>,
    cwd: PathBuf,
    environment: Vec<(OsString, OsString)>,
}

impl Request {
    fn execution_identity(&self) -> Value {
        json!({
            "program": native_string(&self.program),
            "arguments": self.args.iter().map(|v| native_string(v)).collect::<Vec<_>>(),
            "cwd": native_string(self.cwd.as_os_str()),
            "environment": self.environment.iter().map(|(k, v)| {
                json!({"name": native_string(k), "value": native_string(v)})
            }).collect::<Vec<_>>(),
            "inherited_environment_identity": "unknown; values not recorded",
        })
    }

    fn record(&self) -> Value {
        let mut record = self.execution_identity();
        record["name"] = json!(self.name);
        record["state"] = json!("intent_recorded");
        record["custody_and_capture_enforcement"] = json!("unknown; fixture direct-child observation only");
        record
    }

    fn spawn(&self, stdout: File, stderr: File) -> std::io::Result<Child> {
        Command::new(&self.program)
            .args(&self.args)
            .current_dir(&self.cwd)
            .envs(self.environment.iter().map(|(k, v)| (k, v)))
            .stdin(Stdio::null())
            .stdout(stdout)
            .stderr(stderr)
            .spawn()
    }
}

fn native_string(value: &OsStr) -> Value {
    #[cfg(unix)]
    let (kind, bytes) = {
        use std::os::unix::ffi::OsStrExt;
        ("unix-bytes", value.as_bytes().to_vec())
    };
    #[cfg(windows)]
    let (kind, bytes) = {
        use std::os::windows::ffi::OsStrExt;
        ("windows-utf16le", value.encode_wide().flat_map(u16::to_le_bytes).collect::<Vec<_>>())
    };
    json!({"encoding": kind, "hex": bytes.iter().map(|v| format!("{v:02x}")).collect::<String>(), "text": value.to_str()})
}

struct Observation {
    status: ExitStatus,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

impl Observation {
    fn success(self, stage: &str) -> Result<Self> {
        ensure!(self.status.success(), "{stage}: status {:?}; {}", self.status, String::from_utf8_lossy(&self.stderr));
        Ok(self)
    }
}

/// Finite fixture cleanup observes only its direct child. It cannot establish
/// arbitrary descendant emptiness, signal relay or production lease release.
struct FixtureChild(Option<Child>);

impl FixtureChild {
    fn stop(&mut self) -> Result<Option<ExitStatus>> {
        let Some(child) = self.0.as_mut() else { return Ok(None); };
        if let Some(status) = child.try_wait()? {
            self.0.take();
            return Ok(Some(status));
        }
        let kill_error = child.kill().err();
        let deadline = Instant::now() + STOP_BUDGET;
        loop {
            if let Some(status) = child.try_wait()? {
                self.0.take();
                return Ok(Some(status));
            }
            if Instant::now() >= deadline {
                bail!("fixture direct child did not finish after kill: {kill_error:?}");
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

impl Drop for FixtureChild {
    fn drop(&mut self) {
        if self.0.is_some() {
            let _ = self.stop();
        }
    }
}

struct Probe {
    root: PathBuf,
    cwd: PathBuf,
    cargo_home: PathBuf,
    target: PathBuf,
    cargo: OsString,
    server: Option<HttpServer>,
    index_url: String,
    deadline: Instant,
    stage: String,
    sequence: usize,
    complete: bool,
}

impl Probe {
    fn new() -> Result<Self> {
        let root = retained_directory("adze-lock-fixture-λ with spaces-")?;
        // The persistent path is printed before any fallible setup. Every
        // subsequent failure leaves its owned files available for inspection.
        println!("ADZE_LOCK_FIXTURE_ARTIFACTS={}", root.display());
        let cwd = root.join("cwd");
        let target = root.join("target");
        let mut probe = Self {
            cargo_home: root.join("cargo-home-not-created"), root, cwd, target,
            cargo: std::env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo")),
            server: None, index_url: String::new(), deadline: Instant::now() + CONTROLLER_BUDGET,
            stage: "create_owned_directories".into(), sequence: 0, complete: false,
        };
        probe.checkpoint("incomplete", None)?;
        if let Err(error) = probe.prepare() {
            let retained = probe.checkpoint("failed", Some(format!("{error:#}")));
            probe.complete = true;
            return Err(error).with_context(|| format!(
                "fixture setup failed at {}; checkpoint={retained:?}; retained {}",
                probe.stage, probe.root.display()
            ));
        }
        Ok(probe)
    }

    fn prepare(&mut self) -> Result<()> {
        for path in [&self.cwd, &self.target, &self.root.join("registry/index"), &self.root.join("registry/crates"), &self.root.join("commands")] {
            fs::create_dir_all(path)?;
        }
        ensure!(!self.cwd.ancestors().any(|p| p.join("Cargo.toml").exists()), "fixture cwd must not inherit a workspace manifest");
        let cargo_home = tempfile::Builder::new().prefix("cargo-home-").tempdir_in(&self.root)?;
        self.cargo_home = cargo_home.path().to_owned();
        self.stage("starting_registry")?;
        let started = start_http_server(&self.root.join("registry"));
        let result = match started {
            Ok((server, port)) => {
                self.server = Some(server);
                self.index_url = format!("sparse+http://127.0.0.1:{port}/index/");
                write_registry_config(&cargo_home, &self.index_url,
                    &self.root.join("registry/index"), &format!("http://127.0.0.1:{port}/crates/"))
            }
            Err(error) => Err(error),
        };
        // Retain even an incomplete home before propagating setup failure.
        self.cargo_home = cargo_home.keep();
        result
    }

    fn checkpoint(&self, state: &str, error: Option<String>) -> Result<()> {
        fs::write(self.root.join("fixture-status.json"), serde_json::to_vec_pretty(&json!({
            "kind": "two-crate Cargo assumption fixture", "state": state, "stage": self.stage,
            "error": error, "last_intent_number": self.sequence, "index_url": self.index_url,
            "package_first_qualified": false, "custody_and_capture_enforcement": "unknown",
            "limits": {"controller_seconds": CONTROLLER_BUDGET.as_secs(), "command_seconds": COMMAND_BUDGET.as_secs(),
                "output_read_bytes_per_channel": CHANNEL_LIMIT, "direct_child_stop_seconds": STOP_BUDGET.as_secs()},
            "archive_inputs": "literal fixture archives, not cargo package output or the release graph",
        }))?)?;
        Ok(())
    }

    fn stage(&mut self, stage: &str) -> Result<()> {
        self.stage = stage.to_owned();
        self.checkpoint("incomplete", None)
    }

    fn request(&self, name: &'static str, program: OsString, args: Vec<OsString>, cwd: &Path) -> Request {
        Request {
            name, program, args, cwd: cwd.to_owned(), environment: vec![
                ("CARGO_HOME".into(), self.cargo_home.as_os_str().to_owned()),
                ("CARGO_TARGET_DIR".into(), self.target.as_os_str().to_owned()),
                ("CARGO_REGISTRIES_ADZE_LOCAL_INDEX".into(), self.index_url.clone().into()),
                ("CARGO_NET_OFFLINE".into(), "false".into()),
                ("CARGO_TERM_COLOR".into(), "never".into()),
                // The fixture cwd is outside the repository's toolchain
                // override. Select the reviewed installed toolchain explicitly.
                ("RUSTUP_TOOLCHAIN".into(), "1.95.0".into()),
                ("CARGO_HTTP_TIMEOUT".into(), "10".into()),
                ("CARGO_NET_RETRY".into(), "0".into()),
            ],
        }
    }

    fn run(&mut self, request: Request) -> Result<Observation> {
        self.stage(request.name)?;
        self.sequence += 1;
        self.checkpoint("incomplete", None)?;
        let directory = self.root.join("commands").join(format!("{:04}", self.sequence));
        fs::create_dir(&directory)?;
        execute_request(&request, &directory, self.deadline.min(Instant::now() + COMMAND_BUDGET))
    }

    fn cargo(&mut self, name: &'static str, args: Vec<OsString>, cwd: &Path) -> Result<Observation> {
        self.run(self.request(name, self.cargo.clone(), args, cwd))
    }
}

impl Drop for Probe {
    fn drop(&mut self) {
        if !self.complete {
            let _ = self.checkpoint("incomplete_or_unwound", None);
        }
    }
}

fn execute_request(request: &Request, directory: &Path, deadline: Instant) -> Result<Observation> {
    fs::write(directory.join("intent.json"), serde_json::to_vec_pretty(&request.record())?)?;
    let out_path = directory.join("stdout.bin");
    let err_path = directory.join("stderr.bin");
    let stdout = fs::OpenOptions::new().write(true).create_new(true).open(&out_path)?;
    let stderr = fs::OpenOptions::new().write(true).create_new(true).open(&err_path)?;
    ensure!(Instant::now() < deadline, "fixture controller deadline expired before launch");
    let child = match request.spawn(stdout, stderr) {
        Ok(child) => child,
        Err(error) => {
            fs::write(directory.join("observation.json"), serde_json::to_vec_pretty(&json!({
                "state": "spawn_failed", "error": error.to_string(), "custody": "unknown"
            }))?)?;
            return Err(error.into());
        }
    };
    let pid = child.id();
    let mut owned = FixtureChild(Some(child));
    let (status, intervention) = loop {
        if let Some(status) = owned.0.as_mut().expect("owned fixture child").try_wait()? {
            owned.0.take();
            break (status, None);
        }
        let over_limit = fs::metadata(&out_path)?.len() > CHANNEL_LIMIT || fs::metadata(&err_path)?.len() > CHANNEL_LIMIT;
        if over_limit || Instant::now() >= deadline {
            let reason = if over_limit { "output_file_limit_observed" } else { "absolute_fixture_deadline" };
            let stop = owned.stop();
            fs::write(directory.join("intervention.json"), serde_json::to_vec_pretty(&json!({
                "reason": reason, "direct_child_pid": pid,
                "stop_result": format!("{stop:?}"), "descendant_custody": "unknown",
            }))?)?;
            break (stop?.context("stopped fixture child had no status")?, Some(reason));
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    let mut observation = json!({
        "state": "direct_child_observed", "direct_child_pid": pid,
        "exit_code": status.code(), "status_debug": format!("{status:?}"),
        "natural_status": intervention.is_none(), "intervention": intervention,
        "stdout_spool": {"path": native_string(out_path.as_os_str()), "observed_bytes": fs::metadata(&out_path)?.len()},
        "stderr_spool": {"path": native_string(err_path.as_os_str()), "observed_bytes": fs::metadata(&err_path)?.len()},
        "bounded_output_snapshot": "not yet read",
        "custody_and_capture_enforcement": "unknown; no process-group or Job-object proof",
    });
    // Preserve actual status before a cap/read failure can stop interpretation.
    fs::write(directory.join("observation.json"), serde_json::to_vec_pretty(&observation)?)?;
    ensure!(intervention.is_none(), "fixture command {} refused after {intervention:?}", request.name);
    let capture = bounded_read(&out_path).and_then(|stdout| bounded_read(&err_path).map(|stderr| (stdout, stderr)));
    let (stdout, stderr) = match capture {
        Ok(capture) => capture,
        Err(error) => {
            observation["bounded_output_snapshot"] = json!({"error": format!("{error:#}")});
            fs::write(directory.join("observation.json"), serde_json::to_vec_pretty(&observation)?)?;
            return Err(error);
        }
    };
    // The parsed bytes have their own retained snapshots. An unowned writer
    // could still alter the original spools; this fixture does not claim EOF
    // or descendant cleanup from observing its direct child.
    let stdout_snapshot = directory.join("stdout-observed.bin");
    let stderr_snapshot = directory.join("stderr-observed.bin");
    fs::write(&stdout_snapshot, &stdout)?;
    fs::write(&stderr_snapshot, &stderr)?;
    observation["bounded_output_snapshot"] = json!({
        "stdout": file_fact(&stdout_snapshot)?, "stderr": file_fact(&stderr_snapshot)?
    });
    fs::write(directory.join("observation.json"), serde_json::to_vec_pretty(&observation)?)?;
    Ok(Observation { status, stdout, stderr })
}

fn bounded_read(path: &Path) -> Result<Vec<u8>> {
    let file = File::open(path)?;
    ensure!(file.metadata()?.len() <= CHANNEL_LIMIT, "fixture output exceeds bounded read: {}", path.display());
    let mut bytes = Vec::new();
    file.take(CHANNEL_LIMIT + 1).read_to_end(&mut bytes)?;
    ensure!(bytes.len() as u64 <= CHANNEL_LIMIT, "fixture output grew during bounded read");
    Ok(bytes)
}

fn file_fact(path: &Path) -> Result<Value> {
    let metadata = fs::symlink_metadata(path)?;
    ensure!(metadata.is_file() && !metadata.file_type().is_symlink(), "fixture artifact is not a regular file");
    Ok(json!({"path": native_string(path.as_os_str()), "bytes": metadata.len(), "sha256": sha256_hex_file(path)?}))
}

fn same_fixture_binary_bytes(left: &Path, right: &Path) -> Result<bool> {
    let mut left = File::open(left)?;
    let mut right = File::open(right)?;
    let length = left.metadata()?.len();
    ensure!(length <= TARGET_FILE_LIMIT, "fixture byte-comparison bound");
    if length != right.metadata()?.len() {
        return Ok(false);
    }
    let mut remaining = length;
    let mut a = [0_u8; 8192];
    let mut b = [0_u8; 8192];
    while remaining != 0 {
        let count = remaining.min(a.len() as u64) as usize;
        left.read_exact(&mut a[..count])?;
        right.read_exact(&mut b[..count])?;
        if a[..count] != b[..count] {
            return Ok(false);
        }
        remaining -= count as u64;
    }
    ensure!(left.read(&mut a[..1])? == 0 && right.read(&mut b[..1])? == 0, "fixture binary grew during comparison");
    Ok(true)
}

fn args(values: &[&str]) -> Vec<OsString> {
    values.iter().map(|value| OsString::from(*value)).collect()
}

fn pack(probe: &mut Probe, name: &'static str, directory: &Path, archive: &Path) -> Result<()> {
    let mut argv = args(&["-czf"]);
    argv.push(archive.as_os_str().to_owned());
    argv.push("-C".into());
    argv.push(directory.parent().context("package directory parent")?.as_os_str().to_owned());
    argv.push("--".into());
    argv.push(directory.file_name().context("package directory name")?.to_owned());
    let request = probe.request(name, "tar".into(), argv, &probe.cwd);
    probe.run(request)?.success(name)?;
    Ok(())
}

fn extract_retained(probe: &mut Probe, archive: &Path, label: &str, package: &str) -> Result<PathBuf> {
    probe.stage(&format!("extract_{label}"))?;
    let destination = probe.root.join("extracted").join(label);
    fs::create_dir_all(&destination)?;
    let archive_root = format!("{package}-{VERSION}");
    extract_package_to(archive, &destination, &archive_root)?;
    Ok(destination.join(archive_root))
}

fn no_deps_metadata(probe: &mut Probe, label: &'static str, manifest: &Path) -> Result<Observation> {
    let mut argv = args(&["metadata", "--no-deps", "--offline", "--format-version", "1", "--manifest-path"]);
    argv.push(manifest.as_os_str().to_owned());
    probe.cargo(label, argv, manifest.parent().context("fixture manifest parent")?)?.success(label)
}

fn full_metadata_args(manifest: &Path) -> Vec<OsString> {
    let mut argv = args(&["metadata", "--locked", "--offline", "--format-version", "1", "--manifest-path"]);
    argv.push(manifest.as_os_str().to_owned());
    argv
}

fn only_named_package<'a>(metadata: &'a Value, name: &str) -> Result<&'a Value> {
    let matches = metadata["packages"].as_array().context("metadata packages")?.iter()
        .filter(|package| package["name"].as_str() == Some(name)).collect::<Vec<_>>();
    ensure!(matches.len() == 1, "exactly one metadata package named {name}");
    Ok(matches[0])
}

fn require_resolve_graph(raw: &[u8], manifest: &Path, index_url: &str) -> Result<Value> {
    let metadata: Value = serde_json::from_slice(raw)?;
    ensure!(metadata["packages"].as_array().context("packages")?.len() == 2, "fixture graph must contain exactly its two packages");
    let root = only_named_package(&metadata, CLI)?;
    let dependency = only_named_package(&metadata, DEP)?;
    ensure!(root["version"] == VERSION && dependency["version"] == VERSION, "exact selected versions");
    ensure!(root["source"].is_null(), "metadata root is the exact extracted path, not an install source claim");
    ensure!(dependency["source"].as_str() == Some(index_url), "dependency source must be the selected sparse registry");
    ensure!(fs::canonicalize(root["manifest_path"].as_str().context("root manifest")?)? == fs::canonicalize(manifest)?, "exact resolved root manifest");
    ensure!(metadata["workspace_members"] == json!([root["id"]]), "single workspace root");
    ensure!(metadata["resolve"]["root"] == root["id"], "non-null root resolve identity");
    let nodes = metadata["resolve"]["nodes"].as_array().context("non-null resolve nodes")?;
    ensure!(nodes.len() == 2, "two actual resolve nodes");
    let root_node = nodes.iter().find(|node| node["id"] == root["id"]).context("root resolve node")?;
    let dependency_node = nodes.iter().find(|node| node["id"] == dependency["id"]).context("dependency resolve node")?;
    ensure!(root_node["dependencies"] == json!([dependency["id"]]), "root resolves the one selected dependency");
    ensure!(dependency_node["dependencies"] == json!([]), "literal dependency has no transitive crates");
    let edges = root_node["deps"].as_array().context("dependency edges")?;
    ensure!(edges.len() == 1 && edges[0]["name"] == "chosen" && edges[0]["pkg"] == dependency["id"], "renamed dependency edge must retain its actual package");
    Ok(metadata)
}

fn cached_entry(home: &Path, bucket: &str, name: &str, directory: bool) -> Result<PathBuf> {
    let parent = home.join("registry").join(bucket);
    let mut matches = Vec::new();
    if parent.exists() {
        let sources = fs::read_dir(parent)?.collect::<std::io::Result<Vec<_>>>()?;
        ensure!(sources.len() <= 8, "unexpected registry cache fanout in fresh fixture home");
        for source in sources {
            let metadata = source.file_type()?;
            ensure!(metadata.is_dir() && !metadata.is_symlink(), "unexpected registry cache entry type");
            let path = source.path().join(name);
            if let Ok(metadata) = fs::symlink_metadata(&path) {
                ensure!(!metadata.file_type().is_symlink(), "cache candidate cannot be a link");
                ensure!(if directory { metadata.is_dir() } else { metadata.is_file() }, "cache candidate has wrong type");
                matches.push(path);
            }
        }
    }
    ensure!(matches.len() == 1, "expected exactly one actual cached {bucket}/{name}, found {}", matches.len());
    Ok(matches.remove(0))
}

fn verify_cached_package(home: &Path, name: &str, archive: &Path, expected_root: &Path) -> Result<PathBuf> {
    let cache = cached_entry(home, "cache", &format!("{name}-{VERSION}.crate"), false)?;
    ensure!(sha256_hex_file(&cache)? == sha256_hex_file(archive)?, "Cargo cached archive must be byte-identical to final served image");
    let source = cached_entry(home, "src", &format!("{name}-{VERSION}"), true)?;
    let expected = file_inventory(expected_root)?;
    let mut actual = file_inventory(&source)?;
    // Pinned Cargo1.95 creates this one completion marker after extraction.
    // It is not a member of the literal archive and is recorded separately.
    ensure!(actual.remove(".cargo-ok").is_some(), "Cargo extraction completion marker");
    let marker: Value = serde_json::from_slice(&fs::read(source.join(".cargo-ok"))?)?;
    ensure!(marker == json!({"v": 1}), "pinned Cargo extraction marker version");
    ensure!(actual.keys().eq(expected.keys()), "unexpected/missing cached package members");
    for (path, wanted) in expected {
        let found = &actual[&path];
        ensure!(found.bytes == wanted.bytes && found.sha256 == wanted.sha256, "cached member changed: {path}");
    }
    Ok(source)
}

fn compiler_artifacts(raw: &[u8], cached_cli: &Path, cached_dependency: &Path, target: &Path) -> Result<(PathBuf, Value)> {
    let text = std::str::from_utf8(raw).context("Cargo JSON output must be UTF-8")?;
    let mut selected = None;
    let mut artifacts = Vec::new();
    let mut dependency_count = 0;
    let mut completed = 0;
    for line in text.lines().filter(|line| !line.is_empty()) {
        let message: Value = serde_json::from_str(line).context("every nonempty Cargo stdout line must be JSON")?;
        if message["reason"] == "build-finished" {
            ensure!(message["success"] == true, "Cargo reported unsuccessful build");
            completed += 1;
        }
        if message["reason"] != "compiler-artifact" {
            continue;
        }
        ensure!(message["profile"]["test"] == false && message["profile"]["opt_level"] == "0", "actual dev compilation, not a test harness");
        ensure!(message["fresh"] == false, "fresh owned target must compile each selected fixture unit");
        ensure!(message["package_id"].as_str().is_some_and(|id| !id.is_empty()), "retain actual opaque Cargo package identity");
        let manifest = message["manifest_path"].as_str().context("compiler artifact manifest")?;
        if message["target"]["name"] == BIN {
            ensure!(selected.is_none(), "root binary compiler artifact must be unique");
            ensure!(message["target"]["kind"] == json!(["bin"]), "selected target must be binary");
            ensure!(fs::canonicalize(manifest)? == fs::canonicalize(cached_cli.join("Cargo.toml"))?, "root artifact must name the verified Cargo extraction");
            ensure!(fs::canonicalize(message["target"]["src_path"].as_str().context("root artifact source")?)? == fs::canonicalize(cached_cli.join("src/main.rs"))?, "root artifact must compile the verified literal source");
            let executable = PathBuf::from(message["executable"].as_str().context("binary artifact executable")?);
            ensure!(executable.is_absolute(), "Cargo binary artifact path must be absolute");
            ensure!(executable.file_name() == Some(OsStr::new(&format!("{BIN}{}", std::env::consts::EXE_SUFFIX))), "exact reported binary filename");
            // For non-path installs Cargo1.95 may rename this reported,
            // uplifted executable into the install root. Only its parent must
            // still exist; the retained deps image is checked separately.
            ensure!(fs::canonicalize(executable.parent().context("artifact parent")?)?.starts_with(fs::canonicalize(target)?), "reported executable parent must be in the owned target");
            ensure!(message["filenames"].as_array().context("artifact filenames")?.iter().any(|value| value.as_str() == executable.to_str()), "executable must also be a reported artifact file");
            selected = Some(executable);
        } else {
            ensure!(fs::canonicalize(manifest)? == fs::canonicalize(cached_dependency.join("Cargo.toml"))?, "only the verified dependency may supply another artifact");
            ensure!(message["target"]["kind"] == json!(["lib"]) && message["executable"].is_null(), "dependency artifact must be the literal library");
            ensure!(fs::canonicalize(message["target"]["src_path"].as_str().context("dependency artifact source")?)? == fs::canonicalize(cached_dependency.join("src/lib.rs"))?, "dependency artifact must compile the verified literal source");
            dependency_count += 1;
        }
        artifacts.push(message);
    }
    ensure!(completed == 1 && dependency_count == 1 && artifacts.len() == 2, "one successful build completion and the exact two selected fixture units");
    Ok((selected.context("root binary compiler artifact")?, json!(artifacts)))
}

/// This inventories only the tiny fixture's fresh build directory. Archive
/// member interpretation continues to use the existing archive authority.
fn target_regular_files(target: &Path) -> Result<BTreeMap<PathBuf, u64>> {
    let target = fs::canonicalize(target)?;
    let mut result = BTreeMap::new();
    let mut entries = 0;
    let mut total = 0_u64;
    for entry in walkdir::WalkDir::new(&target).follow_links(false) {
        let entry = entry?;
        entries += 1;
        ensure!(entries <= TARGET_ENTRY_LIMIT && entry.depth() <= 16, "fixture target entry/depth bound");
        ensure!(!entry.file_type().is_symlink(), "fixture target cannot contain symbolic links");
        if entry.file_type().is_dir() {
            continue;
        }
        ensure!(entry.file_type().is_file(), "fixture target contains a special file");
        let bytes = entry.metadata()?.len();
        ensure!(bytes <= TARGET_FILE_LIMIT, "fixture target file bound");
        total = total.checked_add(bytes).context("fixture target byte count overflow")?;
        ensure!(total <= TARGET_TOTAL_LIMIT, "fixture target total bound");
        result.insert(entry.path().to_owned(), bytes);
    }
    Ok(result)
}

fn retained_build_image(
    reported: &Path,
    installed: &Path,
    target: &Path,
    before: &BTreeMap<PathBuf, u64>,
) -> Result<(PathBuf, Value)> {
    let metadata = fs::symlink_metadata(installed)?;
    ensure!(metadata.is_file() && !metadata.file_type().is_symlink(), "installed fixture binary must be regular");
    ensure!(metadata.len() > 0 && metadata.len() <= TARGET_FILE_LIMIT, "installed fixture binary size bound");
    let installed_fact = file_fact(installed)?;
    let installed_bytes = installed_fact["bytes"].as_u64().context("installed binary size")?;
    let installed_digest = installed_fact["sha256"].as_str().context("installed binary digest")?;
    let deps = fs::canonicalize(reported.parent().context("reported executable parent")?.join("deps"))?;
    ensure!(deps.starts_with(fs::canonicalize(target)?), "retained deps directory must be in the owned target");
    let after = target_regular_files(target)?;
    let mut matches = Vec::new();
    for (path, bytes) in &after {
        if path.parent() == Some(deps.as_path()) && !before.contains_key(path)
            && *bytes == installed_bytes && sha256_hex_file(path)? == installed_digest
            && same_fixture_binary_bytes(path, installed)?
        {
            matches.push(path.clone());
        }
    }
    ensure!(matches.len() == 1, "exactly one new retained deps image must match installed bytes, found {}", matches.len());
    let image = matches.remove(0);
    let reported_after = match fs::symlink_metadata(reported) {
        Ok(metadata) => {
            ensure!(metadata.is_file() && !metadata.file_type().is_symlink(), "surviving reported artifact must be regular");
            ensure!(metadata.len() == installed_bytes && sha256_hex_file(reported)? == installed_digest, "surviving reported artifact must match installed bytes");
            ensure!(same_fixture_binary_bytes(reported, installed)?, "surviving reported artifact byte comparison");
            "present_and_byte_identical"
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => "absent_after_install",
        Err(error) => return Err(error.into()),
    };
    // Pinned Cargo1.95 compiler/mod.rs uplifts output.path from deps by
    // link_or_copy, and cargo_install.rs can move the uplifted path for a
    // registry install. This bounded inference is tied to those sources and
    // the observed unique new image; it is not a universal Cargo output rule.
    Ok((image.clone(), json!({
        "reported_executable": native_string(reported.as_os_str()),
        "reported_path_after_install": reported_after,
        "retained_deps_image": file_fact(&image)?,
        "selection": "exactly one newly created regular deps image byte-identical to the installed binary in the fresh owned target",
        "source_basis": [
            "https://github.com/rust-lang/cargo/blob/rust-1.95.0/src/cargo/core/compiler/mod.rs",
            "https://github.com/rust-lang/cargo/blob/rust-1.95.0/src/cargo/ops/cargo_install.rs"
        ],
        "scope": "pinned Cargo1.95 two-crate fixture; not a universal artifact-path guarantee",
        "target_regular_files_before": before.len(), "target_regular_files_after": after.len(),
    })))
}

fn wrong_fixture_dependency_checksum(prepared: &[u8], checksum: &str, index: &str) -> Result<Vec<u8>> {
    // Deliberately mutate one literal fixture lock entry. Cargo remains the
    // resolver; this does not interpret dependency/feature selection itself.
    let mut lock: toml::Value = std::str::from_utf8(prepared)?.parse()?;
    let packages = lock.get_mut("package").and_then(toml::Value::as_array_mut).context("prepared lock packages")?;
    ensure!(packages.len() == 2, "literal two-package prepared lock");
    let mut changed = 0;
    for package in packages {
        if package.get("name").and_then(toml::Value::as_str) == Some(DEP) {
            let package = package.as_table_mut().context("dependency lock entry")?;
            ensure!(package.get("version").and_then(toml::Value::as_str) == Some(VERSION), "exact locked dependency version");
            ensure!(package.get("source").and_then(toml::Value::as_str) == Some(index), "exact locked dependency registry");
            ensure!(package.get("checksum").and_then(toml::Value::as_str) == Some(checksum), "prepared lock binds served dependency checksum");
            let wrong = if checksum.bytes().all(|byte| byte == b'0') { "1".repeat(64) } else { "0".repeat(64) };
            package.insert("checksum".into(), toml::Value::String(wrong));
            changed += 1;
        }
    }
    ensure!(changed == 1, "one exact dependency checksum mutation");
    Ok(toml::to_string(&lock)?.into_bytes())
}

fn run_assumptions(probe: &mut Probe) -> Result<Value> {
    let cwd = probe.cwd.clone();
    let rust = probe.run(probe.request("actual_rustc_identity", "rustc".into(), args(&["-vV"]), &cwd))?.success("rustc identity")?;
    let rust_identity = std::str::from_utf8(&rust.stdout)?;
    ensure!(rust_identity.lines().any(|line| line == "release: 1.95.0"), "fixture requires pinned Rust1.95.0");
    let hosts = rust_identity.lines().filter_map(|line| line.strip_prefix("host: ")).collect::<Vec<_>>();
    ensure!(hosts.len() == 1 && !hosts[0].is_empty(), "one actual native compiler host");
    let host = hosts[0].to_owned();
    let cargo_identity = probe.cargo("actual_cargo_identity", args(&["-V"]), &cwd)?.success("Cargo identity")?;
    ensure!(std::str::from_utf8(&cargo_identity.stdout)?.starts_with("cargo 1.95.0 "), "fixture requires pinned Cargo1.95.0");
    let versions = BTreeMap::from([(DEP.to_owned(), VERSION.to_owned()), (CLI.to_owned(), VERSION.to_owned())]);

    probe.stage("literal_original_archives")?;
    let original_dir = probe.root.join("original");
    fs::create_dir(&original_dir)?;
    let dep_root = probe.root.join("literal").join(format!("{DEP}-{VERSION}"));
    let cli_root = probe.root.join("literal").join(format!("{CLI}-{VERSION}"));
    for package in [&dep_root, &cli_root] { fs::create_dir_all(package.join("src"))?; }
    fs::write(dep_root.join("Cargo.toml"), format!("[package]\nname = '{DEP}'\nversion = '{VERSION}'\nedition = '2021'\n"))?;
    fs::write(dep_root.join("src/lib.rs"), "pub fn answer() -> u32 { 42 }\n")?;
    fs::write(cli_root.join("Cargo.toml"), format!("[package]\nname = '{CLI}'\nversion = '{VERSION}'\nedition = '2021'\n[[bin]]\nname = '{BIN}'\npath = 'src/main.rs'\n[dependencies]\nchosen = {{ package = '{DEP}', version = '>=0.9.0, <2.0.0' }}\n"))?;
    fs::write(cli_root.join("src/main.rs"), "fn main() { println!(\"{}\", chosen::answer()); }\n")?;
    let stale_lock = format!("version = 4\n\n[[package]]\nname = '{CLI}'\nversion = '{VERSION}'\ndependencies = ['{DEP}']\n\n[[package]]\nname = '{DEP}'\nversion = '0.9.0'\nsource = 'registry+https://github.com/rust-lang/crates.io-index'\nchecksum = '{}'\n", "0".repeat(64));
    fs::write(cli_root.join("Cargo.lock"), &stale_lock)?;
    let dep_original = original_dir.join(format!("{DEP}-{VERSION}.crate"));
    let cli_original = original_dir.join(format!("{CLI}-{VERSION}.crate"));
    pack(probe, "pack_literal_dependency", &dep_root, &dep_original)?;
    pack(probe, "pack_literal_cli_with_stale_lock", &cli_root, &cli_original)?;
    let original_facts = json!([file_fact(&dep_original)?, file_fact(&cli_original)?]);

    let dependency = extract_retained(probe, &dep_original, "dependency", DEP)?;
    let dep_manifest = dependency.join("Cargo.toml");
    let dep_output = no_deps_metadata(probe, "interpret_dependency_manifest", &dep_manifest)?;
    let dep_package = validate_served_metadata(&dep_output.stdout, &dep_manifest, DEP, VERSION, &versions, &probe.index_url)?;
    let dep_download = probe.root.join("registry/crates").join(DEP).join(VERSION).join("download");
    copy_new(&dep_original, &dep_download)?;
    write_sparse_index_entry(&probe.root.join("registry/index"), &dep_package, &versions, &sha256_hex_file(&dep_download)?)?;

    probe.stage("manifest_only_transformation")?;
    let transformed = extract_retained(probe, &cli_original, "manifest-transform", CLI)?;
    let original_members = file_inventory(&transformed)?;
    let manifest = transformed.join("Cargo.toml");
    let rewritten = inject_registry_into_manifest(&fs::read_to_string(&manifest)?, &versions)?;
    fs::write(&manifest, rewritten)?;
    let before_lock_members = file_inventory(&transformed)?;
    let changed = manifest_only_changes(&original_members, &before_lock_members)?;
    ensure!(changed.len() == 1 && changed[0].member == "Cargo.toml", "only manifest changes in first stage");
    let staged_archive = probe.root.join("manifest-transformed.crate");
    pack(probe, "pack_manifest_transformation_with_original_lock", &transformed, &staged_archive)?;
    let manifest_output = no_deps_metadata(probe, "interpret_transformed_manifest", &manifest)?;
    validate_served_metadata(&manifest_output.stdout, &manifest, CLI, VERSION, &versions, &probe.index_url)?;

    // Prepare only the dependency's actual Cargo cache. The final root image
    // does not exist yet, so it cannot be silently reused from an earlier image.
    probe.cargo("prefetch_dependency", args(&["info", "--registry", REGISTRY_NAME, &format!("{DEP}@{VERSION}")]), &cwd)?.success("dependency prefetch")?;
    let cached_dependency = verify_cached_package(&probe.cargo_home, DEP, &dep_download, &dependency)?;
    let stale = probe.cargo("stale_lock_refused", full_metadata_args(&manifest), &transformed)?;
    let stale_lock_command_number = probe.sequence;
    ensure!(stale.status.code() == Some(101), "stale lock must fail through actual Cargo");
    let stale_error = std::str::from_utf8(&stale.stderr)?;
    ensure!(stale_error.contains("--locked") && stale_error.contains("lock file"), "negative must fail for lock update refusal: {stale_error}");
    ensure!(fs::read(transformed.join("Cargo.lock"))? == stale_lock.as_bytes(), "failed locked command must preserve original lock bytes");

    let mut generate = args(&["generate-lockfile", "--manifest-path"]);
    generate.push(manifest.as_os_str().to_owned());
    probe.cargo("generate_actual_transformed_lock", generate, &transformed)?.success("generate lock")?;
    let prepared_lock = fs::read(transformed.join("Cargo.lock"))?;
    ensure!(!prepared_lock.is_empty() && prepared_lock != stale_lock.as_bytes(), "actual Cargo must prepare a different nonempty lock");
    let mut fetch = args(&["fetch", "--locked", "--manifest-path"]);
    fetch.push(manifest.as_os_str().to_owned());
    probe.cargo("fetch_exact_prepared_lock", fetch, &transformed)?.success("locked dependency fetch")?;
    let graph_output = probe.cargo("resolved_graph_separate_from_install", full_metadata_args(&manifest), &transformed)?.success("full locked offline metadata")?;
    let graph = require_resolve_graph(&graph_output.stdout, &manifest, &probe.index_url)?;
    let after_lock_members = file_inventory(&transformed)?;
    ensure!(before_lock_members.keys().eq(after_lock_members.keys()), "lock preparation cannot add unrelated package files");
    for (path, before) in &before_lock_members {
        if path != "Cargo.lock" {
            ensure!(after_lock_members[path] == *before, "lock preparation changed unapproved member {path}");
        }
    }
    ensure!(fs::read(transformed.join("Cargo.lock"))? == prepared_lock, "fetch/metadata must not change prepared lock");
    let locks = probe.root.join("locks"); fs::create_dir(&locks)?;
    fs::write(locks.join("original.Cargo.lock"), &stale_lock)?;
    fs::write(locks.join("prepared.Cargo.lock"), &prepared_lock)?;
    let final_archive = probe.root.join("final-cli.crate");
    pack(probe, "pack_final_cli_with_prepared_lock", &transformed, &final_archive)?;
    let final_root = extract_retained(probe, &final_archive, "final-cli", CLI)?;
    ensure!(file_inventory(&final_root)? == after_lock_members, "final archive must retain exactly prepared members");
    let final_manifest = final_root.join("Cargo.toml");
    let final_output = no_deps_metadata(probe, "interpret_final_archive_manifest", &final_manifest)?;
    let final_package = validate_served_metadata(&final_output.stdout, &final_manifest, CLI, VERSION, &versions, &probe.index_url)?;
    let cli_download = probe.root.join("registry/crates").join(CLI).join(VERSION).join("download");
    copy_new(&final_archive, &cli_download)?;
    let final_checksum = sha256_hex_file(&cli_download)?;
    write_sparse_index_entry(&probe.root.join("registry/index"), &final_package, &versions, &final_checksum)?;

    let install_root = probe.root.join("installed");
    let mut install_args = args(&["install", "--locked", "--offline", "--registry", REGISTRY_NAME,
        "--version", VERSION, "--bin", BIN, "--profile", "dev", "--target", &host,
        "--message-format", "json", "--root"]);
    install_args.push(install_root.as_os_str().to_owned());
    install_args.push(CLI.into());
    ensure!(cached_entry(&probe.cargo_home, "cache", &format!("{CLI}-{VERSION}.crate"), false).is_err(), "fresh home must not already contain final CLI archive");
    let before_prefetch = probe.cargo("offline_install_before_root_prefetch_refused", install_args.clone(), &cwd)?;
    let before_prefetch_command_number = probe.sequence;
    ensure!(before_prefetch.status.code() == Some(101), "offline install without cached root must fail");
    ensure!(!installed_binary_path(&install_root, BIN).exists(), "failed prefetch control cannot install a binary");

    probe.cargo("prefetch_final_root_archive", args(&["info", "--registry", REGISTRY_NAME, &format!("{CLI}@{VERSION}")]), &cwd)?.success("final root prefetch")?;
    let cached_cli = verify_cached_package(&probe.cargo_home, CLI, &cli_download, &final_root)?;
    ensure!(fs::read(cached_cli.join("Cargo.lock"))? == prepared_lock, "actual Cargo extraction must contain the prepared packaged lock");
    probe.stage("stop_registry_after_verified_root_prefetch")?;
    let mut server = probe.server.take().context("owned fixture registry server")?;
    let (server_status, server_error) = server.stop_and_reap();
    fs::write(probe.root.join("server-stop.json"), serde_json::to_vec_pretty(&json!({
        "status": format!("{server_status:?}"), "error": server_error, "claim": "existing HTTP direct-child owner only",
    }))?)?;
    ensure!(server_status.is_some() && server_error.is_none(), "registry must be stopped and reaped before offline install");
    drop(server);

    probe.stage("deliberately_mutate_verified_cached_packaged_lock")?;
    let wrong_lock = wrong_fixture_dependency_checksum(&prepared_lock, &sha256_hex_file(&dep_download)?, &probe.index_url)?;
    let wrong_lock_path = locks.join("wrong-checksum.Cargo.lock");
    fs::write(&wrong_lock_path, &wrong_lock)?;
    fs::write(cached_cli.join("Cargo.lock"), &wrong_lock)?;
    let negative_request = probe.request("actual_offline_install_refuses_wrong_packaged_checksum", probe.cargo.clone(), install_args.clone(), &cwd);
    let install_request_identity = negative_request.execution_identity();
    fs::write(probe.root.join("install-request-identity.json"), serde_json::to_vec_pretty(&install_request_identity)?)?;
    let refused = probe.run(negative_request)?;
    let negative_command_number = probe.sequence;
    ensure!(refused.status.code() == Some(101), "actual install must reject the wrong cached packaged checksum");
    let refusal = std::str::from_utf8(&refused.stderr)?;
    ensure!(refusal.contains(DEP) && refusal.contains("checksum for") && refusal.contains("changed between lock files"), "actual install must refuse for the specific locked checksum mismatch: {refusal}");
    ensure!(!installed_binary_path(&install_root, BIN).exists(), "bad-lock install cannot leave an installed binary");
    ensure!(fs::read(cached_cli.join("Cargo.lock"))? == wrong_lock, "refused install must not silently repair the altered lock");
    probe.stage("restore_exact_prepared_packaged_lock")?;
    fs::write(cached_cli.join("Cargo.lock"), &prepared_lock)?;
    verify_cached_package(&probe.cargo_home, CLI, &cli_download, &final_root)?;
    verify_cached_package(&probe.cargo_home, DEP, &dep_download, &dependency)?;

    let target_before = target_regular_files(&probe.target)?;
    fs::write(probe.root.join("preinstall-target-files.json"), serde_json::to_vec_pretty(&json!(
        target_before.iter().map(|(path, bytes)| json!({"path": native_string(path.as_os_str()), "bytes": bytes})).collect::<Vec<_>>()
    ))?)?;
    let positive_request = probe.request("actual_locked_offline_install_with_server_stopped", probe.cargo.clone(), install_args, &cwd);
    ensure!(positive_request.execution_identity() == install_request_identity, "negative and positive actual install requests must have identical program/argv/cwd/explicit environment");
    let installed = probe.run(positive_request)?.success("locked offline install")?;
    let positive_command_number = probe.sequence;
    probe.stage("verify_actual_compiler_artifacts_and_installed_bytes")?;
    let (reported, artifacts) = compiler_artifacts(&installed.stdout, &cached_cli, &cached_dependency, &probe.target)?;
    let binary = installed_binary_path(&install_root, BIN);
    let (built, build_image) = retained_build_image(&reported, &binary, &probe.target, &target_before)?;
    let binary_identity = file_fact(&binary)?;
    let retained_binary = probe.root.join(format!("retained-fixture-binary{}", std::env::consts::EXE_SUFFIX));
    copy_new(&built, &retained_binary)?;
    ensure!(sha256_hex_file(&retained_binary)? == sha256_hex_file(&binary)?, "retained build image and installed binary identity");
    ensure!(same_fixture_binary_bytes(&retained_binary, &binary)?, "retained build image and installed binary byte comparison");
    probe.stage("verify_actual_install_tracking")?;
    let tracking: Value = serde_json::from_slice(&fs::read(install_root.join(".crates2.json"))?)?;
    let tracked = tracking["installs"].as_object().context("actual install tracking map")?;
    ensure!(tracked.len() == 1, "fresh install root has exactly one installed package");
    let tracked_record = tracked.values().next().context("installed record")?;
    ensure!(tracked_record["bins"] == json!([format!("{BIN}{}", std::env::consts::EXE_SUFFIX)]) && tracked_record["version_req"] == format!("={VERSION}"), "actual tracking binary/version request");
    ensure!(tracked_record["profile"] == "dev" && tracked_record["target"] == host, "actual selected install profile/native target");
    ensure!(tracked_record["rustc"].as_str() == Some(rust_identity), "Cargo install compiler identity must match observed pinned compiler");
    let output = probe.run(probe.request("installed_binary_literal42", binary.as_os_str().to_owned(), Vec::new(), &cwd))?.success("installed binary")?;
    ensure!(output.stdout == b"42\n" && output.stderr.is_empty(), "independent literal installed-bytecode outcome");
    probe.stage("verify_final_unchanged_archives_locks_and_binary")?;
    ensure!(file_fact(&binary)? == binary_identity, "installed binary changed across invocation");
    verify_cached_package(&probe.cargo_home, CLI, &cli_download, &final_root)?;
    verify_cached_package(&probe.cargo_home, DEP, &dep_download, &dependency)?;
    ensure!(fs::read(cached_cli.join("Cargo.lock"))? == prepared_lock, "actual locked install must preserve the cached packaged lock");
    ensure!(json!([file_fact(&dep_original)?, file_fact(&cli_original)?]) == original_facts, "original archives remain byte-identical");
    ensure!(sha256_hex_file(&cli_download)? == final_checksum, "served root image must remain unchanged");

    Ok(json!({
        "kind": "Cargo1.95 two-crate assumption probe", "package_first_qualified": false,
        "actual_rustc_identity": rust_identity, "actual_cargo_identity": std::str::from_utf8(&cargo_identity.stdout)?,
        "original_archives": original_facts, "manifest_transformed_archive": file_fact(&staged_archive)?,
        "manifest_member_change": changed, "prepared_lock": file_fact(&locks.join("prepared.Cargo.lock"))?,
        "original_lock": file_fact(&locks.join("original.Cargo.lock"))?, "final_archive": file_fact(&final_archive)?,
        "metadata_stale_lock_negative": {"command_number": stale_lock_command_number, "actual_status": stale.status.code(),
            "classification": "actual full metadata refused the required lock update under --locked"},
        "offline_root_prefetch_negative": {"command_number": before_prefetch_command_number, "actual_status": before_prefetch.status.code(),
            "scope": "identical install argv failed with no root archive cached and no installed binary; no lock-consumption claim from this failure"},
        "install_lock_negative": {"mutated_cached_lock": file_fact(&wrong_lock_path)?, "actual_status": refused.status.code(),
            "negative_command_number": negative_command_number, "positive_command_number": positive_command_number,
            "identical_actual_request": install_request_identity,
            "scope": "actual offline install rejects a deliberate checksum mutation in the verified cached packaged lock; exact original bytes restored before positive install"},
        "served_archive": file_fact(&cli_download)?, "served_index": file_fact(&sparse_index_file(&probe.root.join("registry/index"), CLI))?,
        "served_dependency_archive": file_fact(&dep_download)?, "served_dependency_index": file_fact(&sparse_index_file(&probe.root.join("registry/index"), DEP))?,
        "full_metadata_graph_claim": "actual extracted-root resolve graph; not install feature/unit selection",
        "full_metadata_root": graph["resolve"]["root"], "actual_install_compiler_artifacts": artifacts,
        "actual_retained_build_image": build_image,
        "actual_install_tracking": tracking, "installed_binary": binary_identity,
        "retained_binary": file_fact(&retained_binary)?, "literal_stdout_hex": "34320a",
        "commands_observed": probe.sequence, "custody_and_capture_enforcement": "unknown",
        "request_journal_scope": "fixture Rust/Cargo/tar/installed-binary requests; existing Python archive helper and HTTP owner remain outside this test recorder",
        "production_executor_wired": false, "release_graph_or_journey_qualified": false,
    }))
}

#[test]
fn test_actual_transformed_packaged_lock_prefetch_and_offline_install() {
    let root = retained_directory("adze-lock-fixture-controller-").unwrap();
    println!("ADZE_LOCK_FIXTURE_CONTROLLER_ARTIFACTS={}", root.display());
    let request = Request {
        name: "isolated_actual_cargo_assumption_controller",
        program: std::env::current_exe().unwrap().into_os_string(),
        args: args(&["--exact", "local_registry::provenance::tests::lock_fixture::fixture_cargo_lock_controller", "--ignored", "--nocapture"]),
        cwd: root.clone(), environment: vec![("ADZE_LOCK_FIXTURE_CONTROLLER".into(), "1".into())],
    };
    let result = execute_request(&request, &root, Instant::now() + Duration::from_secs(210))
        .and_then(|observation| observation.success("isolated actual Cargo assumption controller"))
        .unwrap_or_else(|error| panic!("{error:#}; retained fixture controller: {}", root.display()));
    let text = std::str::from_utf8(&result.stdout).expect("fixture report UTF-8");
    let begin = "ADZE_LOCK_FIXTURE_PROOF_BEGIN ";
    let end = "\nADZE_LOCK_FIXTURE_PROOF_END\n";
    assert_eq!(text.matches(begin).count(), 1, "positive proof frame count: {text}");
    assert_eq!(text.matches(end).count(), 1, "positive proof terminal count: {text}");
    let (_, framed) = text.split_once(begin).unwrap();
    let (header, body) = framed.split_once('\n').expect("positive proof header");
    let (proof, _) = body.split_once(end).expect("positive proof terminal");
    let fields = header.split_whitespace().collect::<Vec<_>>();
    assert_eq!(fields.len(), 2, "exact proof length/digest fields");
    let length: usize = fields[0].strip_prefix("bytes=").unwrap().parse().unwrap();
    let digest = fields[1].strip_prefix("sha256=").unwrap();
    assert_eq!(proof.len(), length, "proof byte length");
    let retained_proof = root.join("observed-proof.json");
    fs::write(&retained_proof, proof.as_bytes()).unwrap();
    assert_eq!(sha256_hex_file(&retained_proof).unwrap(), digest, "proof byte digest");
    let value: Value = serde_json::from_str(proof).expect("proof JSON");
    assert_eq!(value["kind"], "Cargo1.95 two-crate assumption probe");
    assert_eq!(value["package_first_qualified"], false);
    // Emit only the proof frame. The retained raw controller stdout includes
    // its nested libtest summary, which must not inflate outer test counts.
    print!("{begin}{header}\n{proof}{end}");
}

/// Ignored controller plumbing; the one active method launches this exact test.
#[test]
#[ignore = "actual two-crate Cargo subprocess selected by the active lock fixture"]
fn fixture_cargo_lock_controller() {
    assert_eq!(std::env::var("ADZE_LOCK_FIXTURE_CONTROLLER").as_deref(), Ok("1"));
    let mut probe = Probe::new().expect("create retained Cargo fixture");
    match run_assumptions(&mut probe) {
        Ok(proof) => {
            let bytes = serde_json::to_vec_pretty(&proof).unwrap();
            let path = probe.root.join("fixture-proof.json");
            fs::write(&path, &bytes).unwrap();
            probe.stage = "assumption_fixture_passed".into();
            probe.checkpoint("assumption_fixture_passed", None).unwrap();
            probe.complete = true;
            println!("ADZE_LOCK_FIXTURE_PROOF_BEGIN bytes={} sha256={}", bytes.len(), sha256_hex_file(&path).unwrap());
            println!("{}", std::str::from_utf8(&bytes).unwrap());
            println!("ADZE_LOCK_FIXTURE_PROOF_END");
        }
        Err(error) => {
            let retained = probe.checkpoint("failed", Some(format!("{error:#}")));
            probe.complete = true;
            panic!("{error:#}; checkpoint={retained:?}; retained fixture: {}", probe.root.display());
        }
    }
}
