//! Machine-readable receipt for #856 local-registry package-first proof.

use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail};
use serde::Serialize;

use crate::local_registry::{CapturedCommand, PublishedCrate};

pub const SCHEMA_VERSION: &str = "1";
pub const RECEIPT_KIND: &str = "local-registry-package-first";
pub const CLAIM_BOUNDARY: &str = "pre-release local-registry evidence only; not crates.io proof";

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct LocalReleaseReceipt {
    pub schema_version: String,
    pub kind: String,
    pub claim_boundary: String,
    pub source_commit: String,
    pub rust_version: String,
    pub target: String,
    pub version: String,
    pub authority: String,
    pub registry: String,
    pub crates: Vec<ReceiptCrate>,
    pub starter_project: String,
    pub commands: Vec<ReceiptCommand>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ReceiptCrate {
    pub publish_order: usize,
    pub name: String,
    pub version: String,
    pub checksum: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ReceiptCommand {
    pub name: String,
    pub argv: Vec<String>,
    pub exit_code: i32,
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stdout_excerpt: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stderr_excerpt: Option<String>,
}

pub fn build_receipt(
    workspace_root: &Path,
    version: &str,
    authority: &str,
    registry: &str,
    published: &[PublishedCrate],
    starter_project: &Path,
    commands: &[CapturedCommand],
) -> Result<LocalReleaseReceipt> {
    let crates = published
        .iter()
        .enumerate()
        .map(|(idx, published)| ReceiptCrate {
            publish_order: idx + 1,
            name: published.name.clone(),
            version: published.version.clone(),
            checksum: published.checksum.clone(),
        })
        .collect();

    Ok(LocalReleaseReceipt {
        schema_version: SCHEMA_VERSION.to_string(),
        kind: RECEIPT_KIND.to_string(),
        claim_boundary: CLAIM_BOUNDARY.to_string(),
        source_commit: source_commit(workspace_root)?,
        rust_version: rust_version()?,
        target: host_target()?,
        version: version.to_string(),
        authority: authority.to_string(),
        registry: registry.to_string(),
        crates,
        starter_project: starter_project.display().to_string(),
        commands: commands.iter().map(receipt_command_from_capture).collect(),
    })
}

pub fn write_receipt(path: &Path, receipt: &LocalReleaseReceipt) -> Result<()> {
    let json =
        serde_json::to_string_pretty(receipt).context("serializing local release receipt")?;
    std::fs::write(path, json).with_context(|| format!("writing receipt to {}", path.display()))?;
    Ok(())
}

pub fn parse_precedence_output_matches(stdout: &str) -> bool {
    stdout.contains("Add(")
        && stdout.contains("Mul(")
        && stdout.contains("Number(1)")
        && stdout.contains("Number(2)")
        && stdout.contains("Number(3)")
        && stdout
            .find("Number(1)")
            .is_some_and(|left| stdout.find("Mul(").is_some_and(|right| left < right))
}

pub fn invalid_input_stderr_contains_diagnostics(stderr: &str) -> bool {
    stderr.contains("expected one of:") || stderr.contains("bytes ")
}

pub fn validate_starter_proof(commands: &[CapturedCommand]) -> Result<()> {
    let test = commands
        .iter()
        .find(|command| command.name == "starter-cargo-test")
        .context("starter proof missing cargo test command")?;
    if !test.success() {
        bail!(
            "starter cargo test failed with status {}: {}",
            test.exit_code,
            excerpt(&test.stderr, 400)
        );
    }

    let parse_ok = commands
        .iter()
        .find(|command| command.name == "starter-parse-valid")
        .context("starter proof missing valid parse example command")?;
    if !parse_ok.success() {
        bail!(
            "starter valid parse example failed with status {}",
            parse_ok.exit_code
        );
    }
    if !parse_precedence_output_matches(&parse_ok.stdout) {
        bail!(
            "starter valid parse output did not match expected precedence shape: {}",
            excerpt(&parse_ok.stdout, 240)
        );
    }

    let parse_bad = commands
        .iter()
        .find(|command| command.name == "starter-parse-invalid")
        .context("starter proof missing invalid parse example command")?;
    if parse_bad.success() {
        bail!("starter invalid parse example should fail with a non-zero status");
    }
    if !invalid_input_stderr_contains_diagnostics(&parse_bad.stderr) {
        bail!(
            "starter invalid parse stderr missing structured diagnostics: {}",
            excerpt(&parse_bad.stderr, 240)
        );
    }

    Ok(())
}

fn receipt_command_from_capture(command: &CapturedCommand) -> ReceiptCommand {
    ReceiptCommand {
        name: command.name.clone(),
        argv: command.argv.clone(),
        exit_code: command.exit_code,
        success: command.success(),
        stdout_excerpt: excerpt_option(&command.stdout),
        stderr_excerpt: excerpt_option(&command.stderr),
    }
}

fn excerpt_option(text: &str) -> Option<String> {
    if text.trim().is_empty() {
        None
    } else {
        Some(excerpt(text, 240))
    }
}

fn excerpt(text: &str, max_chars: usize) -> String {
    let trimmed = text.trim();
    if trimmed.chars().count() <= max_chars {
        trimmed.to_string()
    } else {
        let mut end = 0;
        for (idx, _) in trimmed.char_indices() {
            if idx >= max_chars {
                break;
            }
            end = idx + 1;
        }
        format!("{}…", &trimmed[..end])
    }
}

fn source_commit(workspace_root: &Path) -> Result<String> {
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(workspace_root)
        .output()
        .context("running git rev-parse HEAD")?;
    if !output.status.success() {
        bail!(
            "git rev-parse HEAD failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let commit = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if commit.is_empty() {
        bail!("git rev-parse HEAD returned an empty commit");
    }
    Ok(commit)
}

fn rust_version() -> Result<String> {
    let output = Command::new("rustc")
        .arg("--version")
        .output()
        .context("running rustc --version")?;
    if !output.status.success() {
        bail!(
            "rustc --version failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn host_target() -> Result<String> {
    let output = Command::new("rustc")
        .args(["-vV"])
        .output()
        .context("running rustc -vV")?;
    if !output.status.success() {
        bail!(
            "rustc -vV failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        if let Some(host) = line.strip_prefix("host: ") {
            return Ok(host.trim().to_string());
        }
    }
    bail!("rustc -vV did not report host target");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_precedence_output_accepts_expected_debug_shape() {
        let stdout = "Add(Number(1), (), Mul(Number(2), (), Number(3)))\n";
        assert!(parse_precedence_output_matches(stdout));
    }

    #[test]
    fn parse_precedence_output_rejects_flat_add_chain() {
        let stdout = "Add(Add(Number(1), (), Number(2)), (), Number(3))\n";
        assert!(!parse_precedence_output_matches(stdout));
    }

    #[test]
    fn invalid_input_stderr_requires_structured_diagnostics() {
        assert!(invalid_input_stderr_contains_diagnostics(
            "error at bytes 4..5: expected one of: /\\d+/"
        ));
        assert!(!invalid_input_stderr_contains_diagnostics("panic: boom"));
    }

    #[test]
    fn receipt_serializes_required_fields() {
        let receipt = LocalReleaseReceipt {
            schema_version: SCHEMA_VERSION.to_string(),
            kind: RECEIPT_KIND.to_string(),
            claim_boundary: CLAIM_BOUNDARY.to_string(),
            source_commit: "abc123".to_string(),
            rust_version: "rustc 1.95.0".to_string(),
            target: "x86_64-unknown-linux-gnu".to_string(),
            version: "0.10.0".to_string(),
            authority: "policy/release-graph.toml".to_string(),
            registry: "adze-local".to_string(),
            crates: vec![ReceiptCrate {
                publish_order: 1,
                name: "adze".to_string(),
                version: "0.10.0".to_string(),
                checksum: "deadbeef".to_string(),
            }],
            starter_project: "/tmp/calc".to_string(),
            commands: vec![ReceiptCommand {
                name: "starter-cargo-test".to_string(),
                argv: vec!["cargo".to_string(), "test".to_string()],
                exit_code: 0,
                success: true,
                stdout_excerpt: None,
                stderr_excerpt: None,
            }],
        };

        let json = serde_json::to_string(&receipt).expect("serialize receipt");
        assert!(json.contains("\"schema_version\":\"1\""));
        assert!(json.contains("\"source_commit\":\"abc123\""));
        assert!(json.contains("\"checksum\":\"deadbeef\""));
        assert!(json.contains("\"starter-cargo-test\""));
    }
}
