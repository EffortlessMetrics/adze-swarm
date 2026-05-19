use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};

pub fn run(
    crate_name: &str,
    bin_name: &str,
    version: Option<&str>,
    locked: bool,
    dry_run: bool,
) -> Result<()> {
    validate_args(crate_name, bin_name, version)?;

    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());

    if dry_run {
        print_plan(crate_name, bin_name, version, locked);
        return Ok(());
    }

    let install_root = tempfile::Builder::new()
        .prefix("adze-crates-io-install-root-")
        .tempdir()
        .context("creating temporary cargo install root")?;
    let cargo_home = tempfile::Builder::new()
        .prefix("adze-crates-io-cargo-home-")
        .tempdir()
        .context("creating temporary CARGO_HOME")?;
    let target_dir = tempfile::Builder::new()
        .prefix("adze-crates-io-target-")
        .tempdir()
        .context("creating temporary CARGO_TARGET_DIR")?;

    println!("crates.io install receipt");
    println!("package: {crate_name}");
    println!("binary: {bin_name}");
    println!("version: {}", version.unwrap_or("latest registry version"));
    println!("locked: {locked}");
    println!("install root: {}", install_root.path().display());
    println!();

    let mut info = Command::new(&cargo);
    info.arg("info").arg(crate_name);
    run_command(info, &cargo_home, &target_dir)
        .with_context(|| format!("checking crates.io metadata for package `{crate_name}`"))?;

    let mut install = Command::new(&cargo);
    install
        .arg("install")
        .arg(crate_name)
        .arg("--root")
        .arg(install_root.path());
    if let Some(version) = version {
        install.arg("--version").arg(version);
    }
    if locked {
        install.arg("--locked");
    }
    run_command(install, &cargo_home, &target_dir).with_context(|| {
        format!("installing `{crate_name}` from crates.io into an isolated temp root")
    })?;

    let binary_path = installed_binary_path(install_root.path(), bin_name);
    if !binary_path.is_file() {
        bail!(
            "expected installed binary `{}` was not created",
            binary_path.display()
        );
    }

    let mut version_check = Command::new(&binary_path);
    version_check.arg("--version");
    run_command(version_check, &cargo_home, &target_dir)
        .with_context(|| format!("running `{}` --version", binary_path.display()))?;

    println!();
    println!("receipt: crates.io install succeeded");
    println!("installed binary: {}", binary_path.display());
    Ok(())
}

fn print_plan(crate_name: &str, bin_name: &str, version: Option<&str>, locked: bool) {
    println!("crates.io install receipt plan");
    println!("status: dry-run");
    println!("package: {crate_name}");
    println!("binary: {bin_name}");
    println!("version: {}", version.unwrap_or("latest registry version"));
    println!("locked: {locked}");
    println!("commands:");
    println!("  cargo info {crate_name}");
    if let Some(version) = version {
        if locked {
            println!(
                "  cargo install {crate_name} --root <temp-root> --version {version} --locked"
            );
        } else {
            println!("  cargo install {crate_name} --root <temp-root> --version {version}");
        }
    } else if locked {
        println!("  cargo install {crate_name} --root <temp-root> --locked");
    } else {
        println!("  cargo install {crate_name} --root <temp-root>");
    }
    println!(
        "  <temp-root>/bin/{bin_name}{} --version",
        std::env::consts::EXE_SUFFIX
    );
    println!();
    println!("non-claim: dry-run does not contact crates.io or prove registry installation");
}

fn run_command(
    mut command: Command,
    cargo_home: &tempfile::TempDir,
    target_dir: &tempfile::TempDir,
) -> Result<()> {
    command
        .env("CARGO_HOME", cargo_home.path())
        .env("CARGO_TARGET_DIR", target_dir.path());

    let display = format!("{command:?}");
    let status = command
        .status()
        .with_context(|| format!("spawning command {display}"))?;
    if !status.success() {
        bail!("command failed with status {status}: {display}");
    }
    Ok(())
}

fn validate_args(crate_name: &str, bin_name: &str, version: Option<&str>) -> Result<()> {
    if crate_name.trim().is_empty() {
        bail!("crate package name must not be empty");
    }
    if bin_name.trim().is_empty() {
        bail!("expected binary name must not be empty");
    }
    if version.is_some_and(|version| version.trim().is_empty()) {
        bail!("version must not be empty when provided");
    }
    Ok(())
}

fn installed_binary_path(root: &Path, bin_name: &str) -> PathBuf {
    let mut binary = bin_name.to_owned();
    binary.push_str(std::env::consts::EXE_SUFFIX);
    root.join("bin").join(binary)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verify_crates_io_install_rejects_empty_crate_name() {
        let error = validate_args("  ", "adze", None).unwrap_err().to_string();

        assert!(error.contains("crate package name"));
    }

    #[test]
    fn verify_crates_io_install_rejects_empty_binary_name() {
        let error = validate_args("adze-cli", "", None).unwrap_err().to_string();

        assert!(error.contains("binary name"));
    }

    #[test]
    fn verify_crates_io_install_binary_path_uses_cargo_bin_dir() {
        let root = Path::new("/tmp/adze-install-root");
        let path = installed_binary_path(root, "adze");

        assert_eq!(
            path,
            root.join("bin")
                .join(format!("adze{}", std::env::consts::EXE_SUFFIX))
        );
    }
}
