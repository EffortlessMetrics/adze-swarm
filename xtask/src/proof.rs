//! Durable executed-proof receipt writer/reader (#1058 fix item 5).
//!
//! `run_executed_proof` + `ProofReceipt` previously had no durable caller:
//! evidence lived in console logs only. This module wires one stable canary
//! (a `projection_equivalence` suite row) to a stable on-disk location and
//! provides a reader that re-checks `validate_against_output`.
//!
//! Older private xtask receipts without `raw_output_sha256` are unbound and
//! fail validation. Regenerate them from the intended checkout with
//! `cargo xtask proof run --out <fresh-dir>`; hashing an old log cannot
//! recreate its original execution binding.

use std::fs::{self, OpenOptions};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use crate::executed_proof::{ProofReceipt, ProofSpec, run_executed_proof};

/// Stable canary. The file-level cfg on `projection_equivalence` requires
/// `pure-rust + ts-compat + serialization`; omitting `ts-compat` compiles to
/// zero tests (the #1058 historical witness), so all three plus `glr` stay.
/// No `--nocapture`: the runner rejects every harness flag but `--exact`.
pub const STABLE_CANARY_COMMAND: &str = r#"cargo test -p adze --features "pure-rust,glr,ts-compat,serialization" --test projection_equivalence glr_ambiguous_expr_parse_document_ast_and_json_agree -- --exact"#;
/// Default receipt JSON file name inside the out dir.
pub const RECEIPT_FILE_NAME: &str = "proof-receipt.json";
/// Raw child-output file name inside the out dir.
pub const RAW_FILE_NAME: &str = "raw-output.log";
// Reserve an unused directory even when the attempt fails before a child starts.
const ATTEMPT_FILE_NAME: &str = ".proof-attempt";

/// Workspace root (xtask lives directly under it).
pub fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask lives directly under the workspace root")
        .to_path_buf()
}

fn occupied_destination(path: &Path, option: &str) -> anyhow::Error {
    anyhow::anyhow!(
        "proof destination already exists: {}; use a fresh {option}",
        path.display()
    )
}

fn require_absent(path: &Path, option: &str) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(_) => Err(occupied_destination(path, option)),
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(()),
        Err(err) => {
            Err(err).with_context(|| format!("checking proof destination {}", path.display()))
        }
    }
}

/// Execute `command` in `root` and retain this attempt's raw log and receipt.
/// `out_dir` may already exist, but it must not have been used for a proof
/// attempt or contain proof artifacts. `receipt_path` (`--ledger`) overrides
/// `<out_dir>/proof-receipt.json` and must also be unused.
/// A failed attempt leaves no new receipt; a child failure retains its raw
/// log. Use fresh destinations for retries, and check this run's exit status.
pub fn run_command(
    root: &Path,
    command: &str,
    out_dir: &Path,
    receipt_path: Option<&Path>,
) -> Result<PathBuf> {
    let default_receipt = out_dir.join(RECEIPT_FILE_NAME);
    let raw_output = out_dir.join(RAW_FILE_NAME);
    let attempt = out_dir.join(ATTEMPT_FILE_NAME);
    let receipt_out = receipt_path.map_or_else(|| default_receipt.clone(), Path::to_path_buf);
    let receipt_option = if receipt_path.is_some() {
        "--ledger"
    } else {
        "--out"
    };
    let receipt_parent = receipt_out
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));

    // The marker covers attempts on this version; the artifact checks also
    // protect directories created by older writers. Inspect dangling links as
    // occupied locations, not free paths.
    require_absent(&attempt, "--out")?;
    require_absent(&raw_output, "--out")?;
    require_absent(&default_receipt, "--out")?;
    if receipt_path.is_some() {
        require_absent(&receipt_out, receipt_option)?;
    }

    fs::create_dir_all(out_dir)
        .with_context(|| format!("creating proof out dir {}", out_dir.display()))?;
    fs::create_dir_all(receipt_parent)
        .with_context(|| format!("creating ledger dir {}", receipt_parent.display()))?;
    if receipt_path.is_some() {
        let receipt_name = receipt_out
            .file_name()
            .context("--ledger must name a file")?;
        let out_dir = out_dir
            .canonicalize()
            .context("resolving --out directory")?;
        let receipt_parent = receipt_parent
            .canonicalize()
            .context("resolving --ledger directory")?;
        if receipt_parent == out_dir
            && (receipt_name == RAW_FILE_NAME || receipt_name == ATTEMPT_FILE_NAME)
        {
            bail!("--ledger must not name the raw log or proof attempt marker");
        }
    }
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&attempt)
        .map_err(|err| {
            if err.kind() == ErrorKind::AlreadyExists {
                occupied_destination(&attempt, "--out")
            } else {
                anyhow::Error::new(err)
                    .context(format!("reserving proof output {}", out_dir.display()))
            }
        })?;

    let spec = ProofSpec {
        root: root.to_path_buf(),
        command: command.to_string(),
        raw_output_path: raw_output,
        include_ignored: false,
    };
    let receipt = run_executed_proof(&spec).map_err(|e| anyhow::anyhow!("{e}"))?;
    // Validate staged bytes and publish without overwriting a separately
    // chosen ledger path a concurrent attempt may have claimed.
    let mut staged = tempfile::NamedTempFile::new_in(receipt_parent)
        .with_context(|| format!("staging receipt in {}", receipt_parent.display()))?;
    serde_json::to_writer_pretty(staged.as_file_mut(), &receipt)
        .context("serializing proof receipt")?;
    validate_receipt_file(staged.path())?;
    staged.persist_noclobber(&receipt_out).map_err(|err| {
        if err.error.kind() == ErrorKind::AlreadyExists {
            occupied_destination(&receipt_out, receipt_option)
        } else {
            anyhow::Error::new(err.error)
                .context(format!("publishing receipt {}", receipt_out.display()))
        }
    })?;
    Ok(receipt_out)
}

/// Run the stable canary (`--command` overrides it) against the workspace.
pub fn run_stable_canary(
    out_dir: &Path,
    ledger: Option<&Path>,
    command: Option<&str>,
) -> Result<PathBuf> {
    run_command(
        &workspace_root(),
        command.unwrap_or(STABLE_CANARY_COMMAND),
        out_dir,
        ledger,
    )
}

/// Read a retained receipt without executing anything.
pub fn read_receipt(path: &Path) -> Result<ProofReceipt> {
    let text =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    serde_json::from_str(&text).with_context(|| format!("parsing receipt {}", path.display()))
}

/// Re-check a retained historical receipt against its raw output. This does
/// not attest that a newer attempt ran or passed. Resolves
/// `raw_output_ref` as stored, falling back to `<receipt-dir>/raw-output.log`
/// so a relocated out dir still validates.
pub fn validate_receipt_file(path: &Path) -> Result<()> {
    let receipt = read_receipt(path)?;
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    let stored = PathBuf::from(&receipt.raw_output_ref);
    let raw_path = if stored.is_file() {
        stored
    } else {
        dir.join(RAW_FILE_NAME)
    };
    let raw = std::fs::read_to_string(&raw_path)
        .with_context(|| format!("reading raw output {}", raw_path.display()))?;
    receipt
        .validate_against_output(&raw)
        .map_err(|e| anyhow::anyhow!("receipt {} invalid: {e}", path.display()))
}
