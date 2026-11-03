//! Isolated local-registry package-first install receipt (#856).
//!
//! Pre-release evidence only. This is not a crates.io install claim and must
//! not touch public release credentials.

use std::path::Path;

use anyhow::{Context, Result, bail};

use crate::local_registry;
use crate::local_release_receipt::{self, LocalReleaseReceipt};
use crate::policy;
use crate::release_graph;

pub const DEFAULT_CLI_CRATE: &str = "adze-cli";
pub const DEFAULT_CLI_BIN: &str = "adze";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalReleasePlan {
    pub version: String,
    pub ordered_crates: Vec<String>,
    pub cli_crate: String,
    pub cli_bin: String,
}

pub fn run(
    version: &str,
    cli_crate: &str,
    cli_bin: &str,
    dry_run: bool,
    receipt_out: Option<&Path>,
    artifacts_out: Option<&Path>,
) -> Result<()> {
    let version = version.trim();
    if version.is_empty() {
        bail!("--version is required for local-registry install receipt");
    }
    if cli_crate.trim().is_empty() {
        bail!("CLI crate name must not be empty");
    }
    if cli_bin.trim().is_empty() {
        bail!("CLI binary name must not be empty");
    }

    let root = policy::workspace_root()?;
    let ordered_crates = release_graph::ordered_crate_names(&root)?;
    if ordered_crates.is_empty() {
        bail!("release graph is empty; run `cargo xtask generate-release-graph`");
    }
    if !ordered_crates.iter().any(|name| name == cli_crate) {
        bail!("CLI crate `{cli_crate}` is not in the release graph");
    }

    let plan = LocalReleasePlan {
        version: version.to_string(),
        ordered_crates,
        cli_crate: cli_crate.to_string(),
        cli_bin: cli_bin.to_string(),
    };

    if dry_run {
        print_plan(&plan);
        return Ok(());
    }

    println!("local-registry package-first receipt");
    println!("version: {}", plan.version);
    println!("authority: {}", release_graph::ARTIFACT_PATH);
    println!();

    let mut registry = local_registry::IsolatedRegistry::new_for_release(
        &root,
        artifacts_out,
        version,
        &plan.ordered_crates,
    )?;
    let result = execute(&root, &plan, &mut registry, receipt_out);
    registry.finish_evidence(result)
}

fn execute(
    root: &Path,
    plan: &LocalReleasePlan,
    registry: &mut local_registry::IsolatedRegistry,
    receipt_out: Option<&Path>,
) -> Result<()> {
    registry.publish_release_graph(&plan.ordered_crates, &plan.version)?;

    println!();
    println!(
        "published {} crate(s) into isolated registry `{}`",
        registry.published_crates().len(),
        local_registry::REGISTRY_NAME
    );
    for published in registry.published_crates() {
        println!(
            "  - {} {} ({})",
            published.name, published.version, published.checksum
        );
    }

    println!();
    println!("install:");
    registry.evidence_stage("cargo_install_unqualified")?;
    let install = registry.install_cli(&plan.cli_crate, &plan.cli_bin, &plan.version)?;
    println!("installed binary: {}", install.binary_path.display());

    registry.evidence_stage("installed_cli_init_unqualified")?;
    let starter = registry.init_starter_project(&install.binary_path, root, "calc")?;
    println!("starter project: {}", starter.project_dir.display());

    println!();
    println!("starter proof:");
    registry.evidence_stage("installed_starter_journey_unqualified")?;
    let starter_commands = starter.run_starter_proof(registry)?;
    for command in &starter_commands {
        println!(
            "  - {}: exit={} success={}",
            command.name,
            command.exit_code,
            command.success()
        );
    }

    registry.evidence_stage("journey_receipt_write")?;
    let receipt = local_release_receipt::build_receipt(
        local_release_receipt::SourceBinding {
            commit: registry.bound_source_commit()?,
            artifacts_dir: registry.evidence_root()?,
        },
        &plan.version,
        release_graph::ARTIFACT_PATH,
        local_registry::REGISTRY_NAME,
        registry.published_crates(),
        &starter.project_dir,
        &starter_commands,
    )?;
    local_release_receipt::write_receipt(
        &registry.evidence_root()?.join("journey-observations.json"),
        &receipt,
    )?;
    emit_receipt(&receipt, receipt_out)?;

    println!();
    println!(
        "claim boundary: source/archive/index preparation and observed local journey; full package-first provenance remains unqualified"
    );
    Ok(())
}

fn emit_receipt(receipt: &LocalReleaseReceipt, receipt_out: Option<&Path>) -> Result<()> {
    let json =
        serde_json::to_string_pretty(receipt).context("serializing local release receipt")?;
    if let Some(path) = receipt_out {
        local_release_receipt::write_receipt(path, receipt)?;
        println!("receipt written: {}", path.display());
    }
    println!("receipt-json:");
    println!("{json}");
    Ok(())
}

fn print_plan(plan: &LocalReleasePlan) {
    println!("local-registry package-first receipt plan");
    println!("status: dry-run");
    println!("version: {}", plan.version);
    println!("authority: {}", release_graph::ARTIFACT_PATH);
    println!("cli package: {}", plan.cli_crate);
    println!("cli binary: {}", plan.cli_bin);
    println!("crate count: {}", plan.ordered_crates.len());
    println!();
    println!("isolated roots:");
    println!("  - temporary CARGO_HOME");
    println!("  - temporary CARGO_TARGET_DIR");
    println!("  - persistent owned source/archive/registry evidence outside the checkout");
    println!("  - generated starter project outside the workspace");
    println!();
    println!("package and local-registry preparation order:");
    for (idx, crate_name) in plan.ordered_crates.iter().enumerate() {
        println!(
            "  {}. cargo package -p {crate_name} --allow-dirty --no-verify (owned Git export)",
            idx + 1
        );
    }
    println!(
        "  preserve each original archive, rewrite a separate copy, then index its served manifest"
    );
    println!();
    println!("install:");
    println!(
        "  cargo install {} --bin {} --version {} --registry <local> --locked",
        plan.cli_crate, plan.cli_bin, plan.version
    );
    println!();
    println!("starter flow:");
    println!("  adze init calc");
    println!("  cargo test");
    println!("  cargo run --example parse -- \"1 + 2 * 3\"");
    println!("  cargo run --example parse -- \"1 + @\"");
    println!();
    println!("receipt:");
    println!(
        "  machine-readable JSON with source commit, rustc version, target, crate checksums, and command results"
    );
    println!();
    println!("claim boundary: pre-release local-registry evidence only; not crates.io proof");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dry_run_plan_uses_committed_release_graph() {
        let root = policy::workspace_root().expect("workspace root");
        let ordered = release_graph::ordered_crate_names(&root).expect("release graph");
        let plan = LocalReleasePlan {
            version: "0.10.0".to_string(),
            ordered_crates: ordered.clone(),
            cli_crate: DEFAULT_CLI_CRATE.to_string(),
            cli_bin: DEFAULT_CLI_BIN.to_string(),
        };

        assert_eq!(plan.ordered_crates.len(), 12);
        assert_eq!(
            plan.ordered_crates.last().map(String::as_str),
            Some("adze-cli")
        );
        assert!(plan.ordered_crates.contains(&"adze".to_string()));
    }

    #[test]
    fn dry_run_entry_point_prints_plan_and_returns_ok() {
        // Regression guard for #1098: the dry-run must reach print_plan
        // and exit successfully without touching any registry.
        run(
            "0.10.0",
            DEFAULT_CLI_CRATE,
            DEFAULT_CLI_BIN,
            true,
            None,
            None,
        )
        .expect("dry-run should succeed");
    }

    #[test]
    fn missing_version_is_rejected() {
        let err =
            run("", DEFAULT_CLI_CRATE, DEFAULT_CLI_BIN, true, None, None).expect_err("version");
        assert!(err.to_string().contains("--version is required"));
    }

    #[test]
    fn cli_not_in_graph_is_rejected() {
        let err = run("0.10.0", "xtask", DEFAULT_CLI_BIN, true, None, None).expect_err("cli");
        assert!(err.to_string().contains("not in the release graph"));
    }
}
