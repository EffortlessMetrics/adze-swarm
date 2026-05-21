use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

const BADGE_ENDPOINT_DIR: &str = "badges";
const BADGE_ENDPOINT_TARGET_DIR: &str = "target/xtask/badges";

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct ShieldsEndpointBadge {
    #[serde(rename = "schemaVersion")]
    pub schema_version: u8,
    pub label: String,
    pub message: String,
    pub color: String,
}

pub fn run(check: bool) -> Result<()> {
    let workspace_root = crate::policy::workspace_root()?;
    let target_dir = workspace_root.join(BADGE_ENDPOINT_TARGET_DIR);
    std::fs::create_dir_all(&target_dir)
        .with_context(|| format!("creating badge target dir {}", target_dir.display()))?;

    let ripr_plus = ripr_plus_badge(&workspace_root)?;
    validate_shields_badge(&ripr_plus, Some("ripr+"))?;
    write_json_pretty(&target_dir.join("ripr-plus.json"), &ripr_plus)?;

    if check {
        let committed_dir = workspace_root.join(BADGE_ENDPOINT_DIR);
        compare_files(
            &committed_dir.join("ripr-plus.json"),
            &target_dir.join("ripr-plus.json"),
        )?;
        println!("badges: committed endpoints are current");
        return Ok(());
    }

    let committed_dir = workspace_root.join(BADGE_ENDPOINT_DIR);
    std::fs::create_dir_all(&committed_dir)
        .with_context(|| format!("creating badge endpoint dir {}", committed_dir.display()))?;
    std::fs::copy(
        target_dir.join("ripr-plus.json"),
        committed_dir.join("ripr-plus.json"),
    )
    .with_context(|| "refreshing badges/ripr-plus.json")?;

    println!("badges: refreshed public endpoint JSON under badges/");
    Ok(())
}

fn ripr_plus_badge(workspace_root: &Path) -> Result<ShieldsEndpointBadge> {
    let ripr_bin = std::env::var("RIPR_BIN").unwrap_or_else(|_| "ripr".to_string());

    let output = std::process::Command::new(&ripr_bin)
        .arg("check")
        .arg("--root")
        .arg(workspace_root)
        .arg("--format")
        .arg("repo-badge-plus-shields")
        .current_dir(workspace_root)
        .output()
        .with_context(|| format!("running {ripr_bin} for repo-scoped ripr+ badge"))?;

    if !output.status.success() {
        anyhow::bail!(
            "{ripr_bin} repo-badge-plus-shields failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    serde_json::from_slice(&output.stdout)
        .with_context(|| format!("{ripr_bin} emitted invalid Shields endpoint JSON"))
}

pub fn validate_shields_badge(
    badge: &ShieldsEndpointBadge,
    expected_label: Option<&str>,
) -> Result<()> {
    if badge.schema_version != 1 {
        anyhow::bail!("badge `{}` has unsupported schemaVersion", badge.label);
    }

    if let Some(expected_label) = expected_label
        && badge.label != expected_label
    {
        anyhow::bail!(
            "badge label drifted: got `{}`, expected `{expected_label}`",
            badge.label
        );
    }

    validate_non_empty_field(&badge.label, "message", &badge.message)?;
    validate_non_empty_field(&badge.label, "color", &badge.color)?;

    Ok(())
}

fn validate_non_empty_field(label: &str, field_name: &str, field_value: &str) -> Result<()> {
    if field_value.trim().is_empty() {
        anyhow::bail!("badge `{label}` has empty {field_name}");
    }

    Ok(())
}

fn write_json_pretty(path: &Path, badge: &ShieldsEndpointBadge) -> Result<()> {
    let json = serde_json::to_string_pretty(badge)?;
    std::fs::write(path, format!("{json}\n")).with_context(|| format!("writing {}", path.display()))
}

fn compare_files(committed: &Path, generated: &Path) -> Result<()> {
    let committed_text = std::fs::read_to_string(committed)
        .with_context(|| format!("reading committed badge {}", committed.display()))?;
    let generated_text = std::fs::read_to_string(generated)
        .with_context(|| format!("reading generated badge {}", generated.display()))?;

    if committed_text != generated_text {
        anyhow::bail!(
            "badge endpoint drift: {} differs from {} (run `cargo xtask badges`)",
            committed.display(),
            generated.display()
        );
    }

    Ok(())
}

#[allow(dead_code)]
fn _target_dir_for_tests(workspace_root: &Path) -> PathBuf {
    workspace_root.join(BADGE_ENDPOINT_TARGET_DIR)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ripr_plus_badge_shape_is_stable() {
        let badge = ShieldsEndpointBadge {
            schema_version: 1,
            label: "ripr+".to_string(),
            message: "0".to_string(),
            color: "brightgreen".to_string(),
        };

        assert!(validate_shields_badge(&badge, Some("ripr+")).is_ok());
    }

    #[test]
    fn rejects_empty_badge_message() {
        let badge = ShieldsEndpointBadge {
            schema_version: 1,
            label: "ripr+".to_string(),
            message: " ".to_string(),
            color: "brightgreen".to_string(),
        };

        assert!(validate_shields_badge(&badge, Some("ripr+")).is_err());
    }

    #[test]
    fn rejects_empty_badge_color() {
        let badge = ShieldsEndpointBadge {
            schema_version: 1,
            label: "ripr+".to_string(),
            message: "ok".to_string(),
            color: " ".to_string(),
        };

        assert!(validate_shields_badge(&badge, Some("ripr+")).is_err());
    }

    #[test]
    fn rejects_unsupported_schema_version() {
        let badge = ShieldsEndpointBadge {
            schema_version: 2,
            label: "ripr+".to_string(),
            message: "ok".to_string(),
            color: "brightgreen".to_string(),
        };

        assert!(validate_shields_badge(&badge, Some("ripr+")).is_err());
    }

    #[test]
    fn rejects_label_drift() {
        let badge = ShieldsEndpointBadge {
            schema_version: 1,
            label: "ripr".to_string(),
            message: "ok".to_string(),
            color: "brightgreen".to_string(),
        };

        assert!(validate_shields_badge(&badge, Some("ripr+")).is_err());
    }
}
