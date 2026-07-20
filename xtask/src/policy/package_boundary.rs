//! Workspace package-boundary policy checker.
//!
//! Verifies that every Cargo workspace package is classified in
//! `policy/package-boundary.toml` as a published crate, dev-only crate, or
//! owner-module migration target, and that each manifest's explicit Cargo
//! `publish` stance matches the ledger category.
//!
//! Production/public scope is derived from the ledger (not from Cargo alone):
//! - `public_publish_scope`: packages with `category = "published"` (intended
//!   crates.io or equivalent public surfaces);
//! - `production_scope`: packages with `production_use = true` (runtime or
//!   support surfaces used by production code paths).

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use super::{Mode, ensure_report_dir, workspace_root};

const POLICY_PATH: &str = "policy/package-boundary.toml";

#[derive(Debug, Default, Deserialize)]
struct PackageBoundaryFile {
    #[serde(default)]
    schema_version: String,
    #[serde(default)]
    policy: String,
    #[serde(default)]
    owner: String,
    #[serde(default)]
    status: String,
    #[serde(default)]
    updated: String,
    #[serde(default, rename = "package")]
    packages: Vec<PackageEntry>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct PackageEntry {
    name: String,
    path: String,
    category: String,
    owner: String,
    production_use: Option<bool>,
    publish_intent: String,
    support_tier_impact: String,
    ci_impact: String,
    #[serde(default)]
    migration_target: Option<String>,
    removal_or_promotion_condition: String,
    last_changed: String,
    last_changed_pr: String,
}

#[derive(Debug, Clone, Deserialize)]
struct CargoMetadata {
    packages: Vec<CargoPackage>,
    workspace_members: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct CargoPackage {
    id: String,
    name: String,
    manifest_path: PathBuf,
    /// `None` (JSON null) means unrestricted publish; `Some([])` means
    /// `publish = false`; `Some(["crates.io"])` means registry-scoped publish.
    publish: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize)]
struct WorkspacePackage {
    path: String,
    explicit_publish_declared: bool,
    cargo_publishable: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Category {
    Published,
    DevOnly,
    OwnerModuleMigrationTarget,
}

impl Category {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "published" => Some(Self::Published),
            "dev-only" => Some(Self::DevOnly),
            "owner-module-migration-target" => Some(Self::OwnerModuleMigrationTarget),
            _ => None,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Published => "published",
            Self::DevOnly => "dev-only",
            Self::OwnerModuleMigrationTarget => "owner-module-migration-target",
        }
    }
}

#[derive(Debug, Default, Serialize)]
struct PackageBoundaryReport {
    mode: String,
    release_gate: bool,
    workspace_packages: usize,
    ledger_packages: usize,
    by_category: BTreeMap<String, usize>,
    /// Ledger-derived public publish surfaces (`category = published`).
    public_publish_scope: Vec<String>,
    /// Ledger-derived production-used packages (`production_use = true`).
    production_scope: Vec<String>,
    findings: Vec<Finding>,
}

#[derive(Debug, Clone, Serialize)]
struct Finding {
    severity: &'static str,
    code: &'static str,
    package: Option<String>,
    field: Option<String>,
    message: String,
}

pub fn run_check(mode: Mode, release_gate: bool) -> Result<()> {
    let root = workspace_root()?;
    let report_dir = ensure_report_dir(&root)?;
    let ledger = load_ledger(&root)?;
    let workspace_packages = load_workspace_packages(&root)?;

    let report = validate(&ledger, &workspace_packages, mode, release_gate);
    write_reports(&report_dir, &report)?;
    print_summary(&report);

    if matches!(mode, Mode::BlockingAllowlist | Mode::BlockingStrict) {
        let errors = report
            .findings
            .iter()
            .filter(|f| f.severity == "error")
            .count();
        if errors > 0 {
            anyhow::bail!("package-boundary: {errors} error finding(s) in blocking mode");
        }
    }

    Ok(())
}

fn load_ledger(root: &Path) -> Result<PackageBoundaryFile> {
    let path = root.join(POLICY_PATH);
    let text =
        std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))
}

fn load_workspace_packages(root: &Path) -> Result<BTreeMap<String, WorkspacePackage>> {
    let output = std::process::Command::new("cargo")
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .current_dir(root)
        .output()
        .context("running cargo metadata --format-version 1 --no-deps")?;
    if !output.status.success() {
        anyhow::bail!(
            "cargo metadata failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let metadata: CargoMetadata =
        serde_json::from_slice(&output.stdout).context("parsing cargo metadata output")?;
    let members: BTreeSet<&str> = metadata
        .workspace_members
        .iter()
        .map(String::as_str)
        .collect();
    let mut packages = BTreeMap::new();
    for package in metadata
        .packages
        .into_iter()
        .filter(|package| members.contains(package.id.as_str()))
    {
        let manifest_dir = package
            .manifest_path
            .parent()
            .with_context(|| format!("manifest has no parent for {}", package.name))?;
        let path = relative_path(root, manifest_dir)?;
        let explicit_publish_declared = manifest_declares_explicit_publish(&package.manifest_path)?;
        let cargo_publishable = cargo_metadata_publishable(package.publish.as_ref());
        packages.insert(
            package.name.clone(),
            WorkspacePackage {
                path,
                explicit_publish_declared,
                cargo_publishable,
            },
        );
    }
    Ok(packages)
}

fn manifest_declares_explicit_publish(manifest_path: &Path) -> Result<bool> {
    let text = std::fs::read_to_string(manifest_path)
        .with_context(|| format!("reading {}", manifest_path.display()))?;
    let doc: toml::Table =
        toml::from_str(&text).with_context(|| format!("parsing {}", manifest_path.display()))?;
    Ok(doc
        .get("package")
        .and_then(toml::Value::as_table)
        .is_some_and(|package| package.contains_key("publish")))
}

fn cargo_metadata_publishable(publish: Option<&Vec<String>>) -> bool {
    match publish {
        None => true,
        Some(registries) => !registries.is_empty(),
    }
}

fn expected_cargo_publishable(category: Category) -> bool {
    match category {
        Category::Published => true,
        Category::DevOnly | Category::OwnerModuleMigrationTarget => false,
    }
}

fn relative_path(root: &Path, path: &Path) -> Result<String> {
    let rel = path
        .strip_prefix(root)
        .with_context(|| format!("{} is not under {}", path.display(), root.display()))?;
    let rel = rel.to_string_lossy().replace('\\', "/");
    if rel.is_empty() {
        Ok(".".to_string())
    } else {
        Ok(rel)
    }
}

fn validate(
    ledger: &PackageBoundaryFile,
    workspace_packages: &BTreeMap<String, WorkspacePackage>,
    mode: Mode,
    release_gate: bool,
) -> PackageBoundaryReport {
    let mut report = PackageBoundaryReport {
        mode: format!("{mode:?}"),
        release_gate,
        workspace_packages: workspace_packages.len(),
        ledger_packages: ledger.packages.len(),
        ..Default::default()
    };

    validate_header(ledger, &mut report);

    let mut seen = BTreeSet::new();
    let mut ledger_names = BTreeSet::new();
    for entry in &ledger.packages {
        if !seen.insert(entry.name.clone()) {
            report.findings.push(finding(
                "error",
                "duplicate-package",
                Some(&entry.name),
                Some("name"),
                "package appears more than once in policy/package-boundary.toml",
            ));
        }
        ledger_names.insert(entry.name.clone());

        validate_required_fields(entry, &mut report);
        let Some(category) = Category::parse(&entry.category) else {
            report.findings.push(finding(
                "error",
                "invalid-category",
                Some(&entry.name),
                Some("category"),
                "category must be published, dev-only, or owner-module-migration-target",
            ));
            continue;
        };
        *report
            .by_category
            .entry(category.as_str().to_string())
            .or_default() += 1;

        match workspace_packages.get(&entry.name) {
            Some(package) => {
                if entry.path != package.path {
                    report.findings.push(finding(
                        "error",
                        "path-mismatch",
                        Some(&entry.name),
                        Some("path"),
                        &format!(
                            "ledger path `{}` does not match workspace path `{}`",
                            entry.path, package.path
                        ),
                    ));
                }
                validate_publish_alignment(entry, package, category, &mut report);
            }
            None => report.findings.push(finding(
                "error",
                "unknown-package",
                Some(&entry.name),
                Some("name"),
                "package is not a current workspace member",
            )),
        }

        validate_category_contract(entry, category, release_gate, &mut report);
        record_scope_entry(entry, category, &mut report);
    }

    for package in workspace_packages.keys() {
        if !ledger_names.contains(package) {
            report.findings.push(finding(
                "error",
                "missing-package",
                Some(package),
                Some("name"),
                "workspace package is missing from policy/package-boundary.toml",
            ));
        }
    }

    report
}

fn validate_header(ledger: &PackageBoundaryFile, report: &mut PackageBoundaryReport) {
    required_header("schema_version", &ledger.schema_version, report);
    required_header("policy", &ledger.policy, report);
    required_header("owner", &ledger.owner, report);
    required_header("status", &ledger.status, report);
    required_header("updated", &ledger.updated, report);

    if !ledger.policy.is_empty() && ledger.policy != "package-boundary" {
        report.findings.push(finding(
            "error",
            "wrong-policy",
            None,
            Some("policy"),
            "policy must be package-boundary",
        ));
    }
}

fn required_header(field: &'static str, value: &str, report: &mut PackageBoundaryReport) {
    if value.trim().is_empty() {
        report.findings.push(finding(
            "error",
            "missing-header-field",
            None,
            Some(field),
            "policy header field is required",
        ));
    }
}

fn validate_required_fields(entry: &PackageEntry, report: &mut PackageBoundaryReport) {
    required_entry_field(&entry.name, "name", &entry.name, report);
    required_entry_field(&entry.name, "path", &entry.path, report);
    required_entry_field(&entry.name, "category", &entry.category, report);
    required_entry_field(&entry.name, "owner", &entry.owner, report);
    required_entry_field(&entry.name, "publish_intent", &entry.publish_intent, report);
    required_entry_field(
        &entry.name,
        "support_tier_impact",
        &entry.support_tier_impact,
        report,
    );
    required_entry_field(&entry.name, "ci_impact", &entry.ci_impact, report);
    required_entry_field(
        &entry.name,
        "removal_or_promotion_condition",
        &entry.removal_or_promotion_condition,
        report,
    );
    required_entry_field(&entry.name, "last_changed", &entry.last_changed, report);
    required_entry_field(
        &entry.name,
        "last_changed_pr",
        &entry.last_changed_pr,
        report,
    );
    if entry.production_use.is_none() {
        report.findings.push(finding(
            "error",
            "missing-production-use",
            Some(&entry.name),
            Some("production_use"),
            "production_use must be true or false",
        ));
    }
}

fn required_entry_field(
    package: &str,
    field_name: &'static str,
    value: &str,
    report: &mut PackageBoundaryReport,
) {
    if value.trim().is_empty() {
        report.findings.push(finding(
            "error",
            "missing-package-field",
            Some(package),
            Some(field_name),
            "package ledger field is required",
        ));
    }
}

fn validate_publish_alignment(
    entry: &PackageEntry,
    package: &WorkspacePackage,
    category: Category,
    report: &mut PackageBoundaryReport,
) {
    if !package.explicit_publish_declared {
        report.findings.push(finding(
            "error",
            "missing-explicit-publish",
            Some(&entry.name),
            Some("publish"),
            "manifest must declare an explicit Cargo publish key (no omission/inheritance-only stance)",
        ));
    }

    let expected = expected_cargo_publishable(category);
    if package.cargo_publishable != expected {
        report.findings.push(finding(
            "error",
            "publish-stance-mismatch",
            Some(&entry.name),
            Some("publish"),
            &format!(
                "ledger category `{}` requires cargo publishable={}, but cargo metadata resolves to publishable={}",
                category.as_str(),
                expected,
                package.cargo_publishable
            ),
        ));
    }
}

fn record_scope_entry(
    entry: &PackageEntry,
    category: Category,
    report: &mut PackageBoundaryReport,
) {
    if category == Category::Published {
        report.public_publish_scope.push(entry.name.clone());
    }
    if entry.production_use == Some(true) {
        report.production_scope.push(entry.name.clone());
    }
}

fn validate_category_contract(
    entry: &PackageEntry,
    category: Category,
    release_gate: bool,
    report: &mut PackageBoundaryReport,
) {
    let production_use = entry.production_use.unwrap_or(false);

    if production_use && category == Category::DevOnly {
        report.findings.push(finding(
            "error",
            "production-dev-only",
            Some(&entry.name),
            Some("category"),
            "production-used packages must be published crates or owner-module migration targets",
        ));
    }

    match category {
        Category::Published => {
            if entry.publish_intent.trim().eq_ignore_ascii_case("none") {
                report.findings.push(finding(
                    "error",
                    "published-without-publish-intent",
                    Some(&entry.name),
                    Some("publish_intent"),
                    "published packages must declare a non-none publish intent",
                ));
            }
        }
        Category::DevOnly => {
            if entry
                .support_tier_impact
                .to_ascii_lowercase()
                .contains("stable")
            {
                report.findings.push(finding(
                    "error",
                    "dev-only-stable-claim",
                    Some(&entry.name),
                    Some("support_tier_impact"),
                    "dev-only packages cannot be stable product proof surfaces",
                ));
            }
        }
        Category::OwnerModuleMigrationTarget => {
            let target = entry.migration_target.as_deref().unwrap_or_default().trim();
            if target.is_empty() {
                report.findings.push(finding(
                    "error",
                    "missing-migration-target",
                    Some(&entry.name),
                    Some("migration_target"),
                    "owner-module migration targets must name the target owner/module",
                ));
            }
            if !production_use {
                report.findings.push(finding(
                    "warning",
                    "migration-target-not-production-used",
                    Some(&entry.name),
                    Some("production_use"),
                    "migration targets usually represent production surface debt; confirm false is intentional",
                ));
            }
            if release_gate {
                report.findings.push(finding(
                    "error",
                    "release-blocking-migration-target",
                    Some(&entry.name),
                    Some("category"),
                    "owner-module migration targets are pre-release transition states; move into the SRP owner submodule, remove, or reclassify with an accepted ADR before release",
                ));
            }
        }
    }
}

fn write_reports(dir: &Path, report: &PackageBoundaryReport) -> Result<()> {
    let json_path = dir.join("package-boundary.json");
    std::fs::write(&json_path, serde_json::to_string_pretty(report)?)
        .with_context(|| format!("writing {}", json_path.display()))?;

    let mut md = String::new();
    md.push_str("# Package boundary report\n\n");
    md.push_str(&format!("- mode: `{}`\n", report.mode));
    md.push_str(&format!("- release gate: `{}`\n", report.release_gate));
    md.push_str(&format!(
        "- workspace packages: {}\n",
        report.workspace_packages
    ));
    md.push_str(&format!("- ledger packages: {}\n", report.ledger_packages));
    md.push_str(&format!(
        "- public publish scope: {} package(s)\n",
        report.public_publish_scope.len()
    ));
    md.push_str(&format!(
        "- production scope: {} package(s)\n",
        report.production_scope.len()
    ));
    md.push_str(&format!(
        "- findings: {} error(s), {} warning(s)\n",
        count_severity(report, "error"),
        count_severity(report, "warning")
    ));
    md.push_str("\n## Categories\n\n");
    md.push_str("| category | packages |\n|---|---:|\n");
    for (category, count) in &report.by_category {
        md.push_str(&format!("| `{category}` | {count} |\n"));
    }
    if !report.findings.is_empty() {
        md.push_str("\n## Findings\n\n");
        md.push_str("| severity | code | package | field | message |\n|---|---|---|---|---|\n");
        for finding in &report.findings {
            md.push_str(&format!(
                "| {} | `{}` | `{}` | `{}` | {} |\n",
                finding.severity,
                finding.code,
                finding.package.as_deref().unwrap_or("-"),
                finding.field.as_deref().unwrap_or("-"),
                finding.message.replace('|', "\\|")
            ));
        }
    }
    let md_path = dir.join("package-boundary.md");
    std::fs::write(&md_path, md).with_context(|| format!("writing {}", md_path.display()))?;
    Ok(())
}

fn print_summary(report: &PackageBoundaryReport) {
    println!("package-boundary check ({})", report.mode);
    println!("  release gate:      {}", report.release_gate);
    println!("  workspace packages: {}", report.workspace_packages);
    println!("  ledger packages:    {}", report.ledger_packages);
    println!(
        "  public publish scope: {}",
        report.public_publish_scope.len()
    );
    println!("  production scope:     {}", report.production_scope.len());
    println!("  errors:             {}", count_severity(report, "error"));
    println!(
        "  warnings:           {}",
        count_severity(report, "warning")
    );
    for finding in &report.findings {
        let package = finding.package.as_deref().unwrap_or("-");
        let field = finding.field.as_deref().unwrap_or("-");
        println!(
            "  [{}] {} ({}, {}): {}",
            finding.severity, finding.code, package, field, finding.message
        );
    }
}

fn count_severity(report: &PackageBoundaryReport, severity: &str) -> usize {
    report
        .findings
        .iter()
        .filter(|finding| finding.severity == severity)
        .count()
}

fn finding(
    severity: &'static str,
    code: &'static str,
    package: Option<&str>,
    field: Option<&'static str>,
    message: &str,
) -> Finding {
    Finding {
        severity,
        code,
        package: package.map(str::to_string),
        field: field.map(str::to_string),
        message: message.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn workspace(path: &str, explicit_publish: bool, cargo_publishable: bool) -> WorkspacePackage {
        WorkspacePackage {
            path: path.to_string(),
            explicit_publish_declared: explicit_publish,
            cargo_publishable,
        }
    }

    fn entry(name: &str, category: &str) -> PackageEntry {
        let publish_intent = if category == "dev-only" {
            "none"
        } else {
            "public"
        };
        PackageEntry {
            name: name.to_string(),
            path: name.to_string(),
            category: category.to_string(),
            owner: "test/owner".to_string(),
            production_use: Some(category != "dev-only"),
            publish_intent: publish_intent.to_string(),
            support_tier_impact: "test impact".to_string(),
            ci_impact: "test ci".to_string(),
            migration_target: (category == "owner-module-migration-target")
                .then(|| "test/owner-module".to_string()),
            removal_or_promotion_condition: "test condition".to_string(),
            last_changed: "2026-05-12".to_string(),
            last_changed_pr: "test".to_string(),
        }
    }

    #[test]
    fn category_parse_accepts_contract_terms() {
        assert_eq!(Category::parse("published"), Some(Category::Published));
        assert_eq!(Category::parse("dev-only"), Some(Category::DevOnly));
        assert_eq!(
            Category::parse("owner-module-migration-target"),
            Some(Category::OwnerModuleMigrationTarget)
        );
        assert_eq!(Category::parse("unpublished-production"), None);
    }

    #[test]
    fn validation_reports_missing_unknown_duplicate_and_migration_fields() {
        let ledger = PackageBoundaryFile {
            schema_version: "1.0".to_string(),
            policy: "package-boundary".to_string(),
            owner: "test".to_string(),
            status: "advisory".to_string(),
            updated: "2026-05-12".to_string(),
            packages: vec![
                entry("known", "published"),
                entry("known", "published"),
                entry("unknown", "dev-only"),
                PackageEntry {
                    migration_target: None,
                    ..entry("migration", "owner-module-migration-target")
                },
            ],
        };
        let workspace = BTreeMap::from([
            ("known".to_string(), workspace("known", true, true)),
            ("missing".to_string(), workspace("missing", true, false)),
            ("migration".to_string(), workspace("migration", true, false)),
        ]);

        let report = validate(&ledger, &workspace, Mode::BlockingAllowlist, false);
        let codes: BTreeSet<_> = report.findings.iter().map(|f| f.code).collect();

        assert!(codes.contains("duplicate-package"));
        assert!(codes.contains("unknown-package"));
        assert!(codes.contains("missing-package"));
        assert!(codes.contains("missing-migration-target"));
    }

    #[test]
    fn release_gate_reports_remaining_migration_targets() {
        let ledger = PackageBoundaryFile {
            schema_version: "1.0".to_string(),
            policy: "package-boundary".to_string(),
            owner: "test".to_string(),
            status: "advisory".to_string(),
            updated: "2026-05-12".to_string(),
            packages: vec![entry("migration", "owner-module-migration-target")],
        };
        let workspace =
            BTreeMap::from([("migration".to_string(), workspace("migration", true, false))]);

        let report = validate(&ledger, &workspace, Mode::BlockingAllowlist, true);
        let codes: BTreeSet<_> = report.findings.iter().map(|f| f.code).collect();

        assert!(codes.contains("release-blocking-migration-target"));
    }

    #[test]
    fn publish_alignment_accepts_matching_stances() {
        let ledger = PackageBoundaryFile {
            schema_version: "1.0".to_string(),
            policy: "package-boundary".to_string(),
            owner: "test".to_string(),
            status: "advisory".to_string(),
            updated: "2026-05-12".to_string(),
            packages: vec![
                entry("published", "published"),
                entry("dev-only", "dev-only"),
            ],
        };
        let workspace = BTreeMap::from([
            ("published".to_string(), workspace("published", true, true)),
            ("dev-only".to_string(), workspace("dev-only", true, false)),
        ]);

        let report = validate(&ledger, &workspace, Mode::BlockingAllowlist, false);
        let codes: BTreeSet<_> = report.findings.iter().map(|f| f.code).collect();

        assert!(!codes.contains("publish-stance-mismatch"));
        assert!(!codes.contains("missing-explicit-publish"));
        assert_eq!(report.public_publish_scope, vec!["published".to_string()]);
        assert!(report.production_scope.contains(&"published".to_string()));
        assert!(!report.production_scope.contains(&"dev-only".to_string()));
    }

    #[test]
    fn publish_alignment_reports_category_cargo_mismatch() {
        let ledger = PackageBoundaryFile {
            schema_version: "1.0".to_string(),
            policy: "package-boundary".to_string(),
            owner: "test".to_string(),
            status: "advisory".to_string(),
            updated: "2026-05-12".to_string(),
            packages: vec![entry("drift", "published")],
        };
        let workspace = BTreeMap::from([("drift".to_string(), workspace("drift", true, false))]);

        let report = validate(&ledger, &workspace, Mode::BlockingAllowlist, false);
        let codes: BTreeSet<_> = report.findings.iter().map(|f| f.code).collect();

        assert!(codes.contains("publish-stance-mismatch"));
    }

    #[test]
    fn publish_alignment_reports_missing_explicit_publish() {
        let ledger = PackageBoundaryFile {
            schema_version: "1.0".to_string(),
            policy: "package-boundary".to_string(),
            owner: "test".to_string(),
            status: "advisory".to_string(),
            updated: "2026-05-12".to_string(),
            packages: vec![entry("implicit", "dev-only")],
        };
        let workspace =
            BTreeMap::from([("implicit".to_string(), workspace("implicit", false, false))]);

        let report = validate(&ledger, &workspace, Mode::BlockingAllowlist, false);
        let codes: BTreeSet<_> = report.findings.iter().map(|f| f.code).collect();

        assert!(codes.contains("missing-explicit-publish"));
    }

    #[test]
    fn cargo_metadata_publishable_interprets_null_empty_and_registry_lists() {
        assert!(cargo_metadata_publishable(None));
        assert!(!cargo_metadata_publishable(Some(&vec![])));
        assert!(cargo_metadata_publishable(Some(&vec![
            "crates.io".to_string()
        ])));
    }
}
