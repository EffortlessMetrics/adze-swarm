//! Durable executed-proof receipt writer/reader (#1058 fix item 5).
//!
//! `run_executed_proof` + `ProofReceipt` previously had no durable caller:
//! evidence lived in console logs only. This module wires one stable canary
//! (a `projection_equivalence` suite row) to a stable on-disk location and
//! provides a reader that re-checks `validate_against_output`.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

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

/// Workspace root (xtask lives directly under it).
pub fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask lives directly under the workspace root")
        .to_path_buf()
}

/// Execute `command` in `root`, retain receipt JSON + raw log, and return the
/// receipt JSON path. `receipt_path` (`--ledger`) overrides the default
/// `<out_dir>/proof-receipt.json`; the raw log always lands in `out_dir`.
/// Zero-selection, ignored-only, failing-child and tampered outputs error out
/// (no valid receipt is retained); the written artifact is re-validated from
/// disk before success is reported.
pub fn run_command(
    root: &Path,
    command: &str,
    out_dir: &Path,
    receipt_path: Option<&Path>,
) -> Result<PathBuf> {
    std::fs::create_dir_all(out_dir)
        .with_context(|| format!("creating proof out dir {}", out_dir.display()))?;
    let receipt_out =
        receipt_path.map_or_else(|| out_dir.join(RECEIPT_FILE_NAME), Path::to_path_buf);
    if let Some(parent) = receipt_out.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating ledger dir {}", parent.display()))?;
    }
    let spec = ProofSpec {
        root: root.to_path_buf(),
        command: command.to_string(),
        raw_output_path: out_dir.join(RAW_FILE_NAME),
        include_ignored: false,
    };
    let receipt = run_executed_proof(&spec).map_err(|e| anyhow::anyhow!("{e}"))?;
    let json = serde_json::to_string_pretty(&receipt).context("serializing proof receipt")?;
    std::fs::write(&receipt_out, json)
        .with_context(|| format!("writing receipt {}", receipt_out.display()))?;
    validate_receipt_file(&receipt_out)?;
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

/// Re-check a retained receipt against its raw output. Resolves
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
