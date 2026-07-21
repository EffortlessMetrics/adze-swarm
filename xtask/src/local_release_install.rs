//! Isolated local-registry package-first install receipt (#856).
//!
//! Pre-release evidence only. This is not a crates.io install claim and must
//! not touch public release credentials.

use anyhow::{Result, bail};

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

pub fn run(version: &str, cli_crate: &str, cli_bin: &str, dry_run: bool) -> Result<()> {
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

    bail!("local-registry install receipt execution is not implemented yet (#856); use --dry-run");
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
    println!("  - temporary local registry directory");
    println!("  - generated starter project outside the workspace");
    println!();
    println!("publish order:");
    for (idx, crate_name) in plan.ordered_crates.iter().enumerate() {
        println!(
            "  {}. cargo publish -p {crate_name} --registry <local>",
            idx + 1
        );
    }
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
    println!("  cargo run --example invalid_input");
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
    fn missing_version_is_rejected() {
        let err = run("", DEFAULT_CLI_CRATE, DEFAULT_CLI_BIN, true).expect_err("version");
        assert!(err.to_string().contains("--version is required"));
    }

    #[test]
    fn cli_not_in_graph_is_rejected() {
        let err = run("0.10.0", "xtask", DEFAULT_CLI_BIN, true).expect_err("cli");
        assert!(err.to_string().contains("not in the release graph"));
    }
}
