//! `xtask ci plan` — testable, policy-driven CI planner.
//!
//! Reads:
//! - `policy/ci-lane-whitelist.toml` (lane registry, base_lem, runner_multipliers)
//! - `policy/ci-risk-packs.toml`     (routing map)
//!
//! Computes a plan from a base/head SHA and a label set, then emits
//! `target/ci/ci-plan.json` and (if `$GITHUB_STEP_SUMMARY` is set or
//! `--github-summary <PATH>` is passed) a Markdown summary.
//!
//! Intentionally dependency-light: no network, no cargo metadata invocation
//! today. Cargo-graph closure is a follow-up extension once we have actuals
//! to validate the simpler model.

use anyhow::{Context, Result};
use globset::{Glob, GlobSetBuilder};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Default, Deserialize)]
struct WhitelistFile {
    #[serde(default)]
    runner_multipliers: BTreeMap<String, f64>,
    #[serde(default)]
    budget: Option<Budget>,
    #[serde(default, rename = "lane")]
    lanes: Vec<Lane>,
}

#[derive(Debug, Default, Deserialize, Serialize, Clone)]
#[allow(dead_code)]
pub struct Budget {
    preferred_default_lem: Option<u32>,
    default_limit_lem: Option<u32>,
    elevated_limit_lem: Option<u32>,
    hard_limit_lem: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
struct Lane {
    id: String,
    #[allow(dead_code)]
    workflow: String,
    #[allow(dead_code)]
    job: String,
    #[allow(dead_code)]
    tier: String,
    #[allow(dead_code)]
    default_pr: bool,
    blocking: bool,
    runner: String,
    base_lem: u32,
}

#[derive(Debug, Default, Deserialize)]
struct RiskPacksFile {
    #[serde(default)]
    risk_pack: BTreeMap<String, RiskPack>,
}

#[derive(Debug, Clone, Deserialize)]
struct RiskPack {
    #[allow(dead_code)]
    description: String,
    #[serde(default)]
    paths: Vec<String>,
    #[serde(default)]
    keywords: Vec<String>,
    #[serde(default)]
    lanes: Vec<String>,
    #[serde(default)]
    deep_lanes: Vec<String>,
    #[serde(default)]
    labels: Vec<String>,
}

#[derive(Debug, Default, Serialize)]
pub struct Plan {
    pub schema_version: u32,
    pub repo: &'static str,
    pub posture: &'static str,
    pub base: String,
    pub head: String,
    pub labels: Vec<String>,
    pub changed: Changed,
    pub selection: Selection,
    pub budget: BudgetReport,
    pub warnings: Vec<String>,
    pub notes: Vec<String>,
}

#[derive(Debug, Default, Serialize)]
pub struct Changed {
    pub files: Vec<String>,
    pub crates: Vec<String>,
    pub areas: Vec<String>,
}

#[derive(Debug, Default, Serialize)]
pub struct Selection {
    pub risk_packs: Vec<String>,
    pub lanes: Vec<SelectedLane>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SelectedLane {
    pub id: String,
    pub lem: u32,
    pub blocking: bool,
    pub reason: String,
}

#[derive(Debug, Default, Serialize)]
pub struct BudgetReport {
    pub estimated_lem: u32,
    pub band: String,
    pub limits: Option<Budget>,
}

const FRONTDOOR_DEFAULTS: &[&str] = &[
    "pr-plan",
    "ci-supported",
    "ripr-advisory",
    "ci-lane-whitelist-lint",
];

/// Map an adze area to a list of path-prefix patterns that imply it.
/// Mirrors the top-level area classifier in scripts/ci/pr-plan.py so the
/// canonical Rust planner emits `changed.areas` with the same vocabulary.
const AREAS: &[(&str, &[&str])] = &[
    (
        "docs",
        &["docs/", "book/", ".adze/goals/", "README", "CHANGELOG"],
    ),
    (
        "workflow",
        &[
            ".github/workflows/",
            "policy/",
            "scripts/",
            "justfile",
            ".githooks/",
            "xtask/",
        ],
    ),
    (
        "core_runtime",
        &[
            "runtime/",
            "runtime2/",
            "common/",
            "ir/",
            "glr-core/",
            "tablegen/",
            "macro/",
            "tool/",
            "cli/",
        ],
    ),
    ("microcrate", &["crates/"]),
    (
        "parser",
        &[
            "glr-core/",
            "crates/parser-",
            "crates/grammar-",
            "crates/parsetable-metadata/",
            "crates/linecol-core/",
            "crates/error-location-core/",
        ],
    ),
    ("grammar", &["grammars/", "golden-tests/", "corpus/"]),
    ("tablegen", &["tablegen/", "crates/parsetable-metadata/"]),
    (
        "governance",
        &["crates/bdd-governance-core/", "tests/governance/"],
    ),
    ("concurrency", &["runtime/src/concurrency_caps"]),
    ("wasm", &["wasm-demo/", "runtime/wasm", "playground/"]),
    ("performance", &["benchmarks/", "baselines/"]),
    (
        "manifest",
        &[
            "Cargo.toml",
            "Cargo.lock",
            "rust-toolchain.toml",
            "deny.toml",
        ],
    ),
];

fn classify_areas(files: &[String]) -> Vec<String> {
    let mut hits: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for f in files {
        // Suffix-based docs match (any *.md or README/CHANGELOG anywhere).
        if f.ends_with(".md") || f.contains("README") || f.contains("CHANGELOG") {
            hits.insert("docs".to_string());
        }
        for (area, patterns) in AREAS {
            for p in *patterns {
                if f.starts_with(p) || f == *p {
                    hits.insert((*area).to_string());
                    break;
                }
            }
        }
    }
    hits.into_iter().collect()
}

#[derive(Debug)]
pub struct PlanArgs {
    pub workspace_root: PathBuf,
    pub base: Option<String>,
    pub head: Option<String>,
    pub labels: Vec<String>,
    pub whitelist_path: PathBuf,
    pub risk_packs_path: PathBuf,
    pub json_out: PathBuf,
    pub github_summary: Option<PathBuf>,
    /// When true, fail the command if the plan exceeds the hard ceiling
    /// without a `full-ci` or `ci-budget-override` label.
    pub enforce_hard_ceiling: bool,
}

pub fn run(args: PlanArgs) -> Result<()> {
    let whitelist: WhitelistFile = read_toml(&args.whitelist_path)
        .with_context(|| format!("reading {}", args.whitelist_path.display()))?;
    let risk_packs_file: RiskPacksFile = read_toml(&args.risk_packs_path)
        .with_context(|| format!("reading {}", args.risk_packs_path.display()))?;

    let lane_by_id: BTreeMap<&str, &Lane> =
        whitelist.lanes.iter().map(|l| (l.id.as_str(), l)).collect();

    let base = resolve_base(&args)?;
    let head = args.head.clone().unwrap_or_else(|| "HEAD".to_string());
    let files = changed_files(&args.workspace_root, &base, &head);

    let mut warnings: Vec<String> = Vec::new();

    let packs = select_packs(&risk_packs_file.risk_pack, &files, &args.labels);
    let lanes = select_lanes(
        &lane_by_id,
        &whitelist.runner_multipliers,
        &risk_packs_file.risk_pack,
        &packs,
        &args.labels,
        &mut warnings,
    );

    let total_lem: u32 = lanes.iter().map(|l| l.lem).sum();
    let limits = whitelist.budget.clone();
    let band = band_for(total_lem, &limits);

    let has_ack = args.labels.iter().any(|l| l == "ci-budget-ack");
    let has_override = args
        .labels
        .iter()
        .any(|l| l == "ci-budget-override" || l == "full-ci");
    match band {
        "elevated" if !has_ack => warnings.push(format!(
            "Plan is in the `elevated` band ({total_lem} LEM). Consider whether this PR's risk surface justifies the cost; add the `ci-budget-ack` label to acknowledge."
        )),
        "high" => warnings.push(format!(
            "Plan is in the `high` band ({total_lem} LEM). Add the `ci-budget-ack` label and explain the surface in the PR body."
        )),
        "over-ceiling" if !has_override => warnings.push(format!(
            "Plan is over the hard ceiling ({total_lem} LEM). Either remove deep lanes or add the `ci-budget-override` (or `full-ci`) label."
        )),
        _ => {}
    }

    let mut plan = Plan {
        schema_version: 1,
        repo: "adze",
        posture: "rust",
        base,
        head,
        labels: args.labels.clone(),
        changed: Changed {
            areas: classify_areas(&files),
            files,
            crates: Vec::new(),
        },
        selection: Selection {
            risk_packs: packs,
            lanes,
        },
        budget: BudgetReport {
            estimated_lem: total_lem,
            band: band.to_string(),
            limits,
        },
        warnings,
        notes: vec![
            "Static plan. Routing PRs (10–14) are not yet wired in.".to_string(),
            "Lane LEM values come from policy/ci-lane-whitelist.toml.".to_string(),
        ],
    };

    if let Some(parent) = args.json_out.parent() {
        fs::create_dir_all(parent).ok();
    }
    fs::write(&args.json_out, serde_json::to_string_pretty(&plan)? + "\n")
        .with_context(|| format!("writing {}", args.json_out.display()))?;

    if let Some(summary_path) = &args.github_summary {
        write_summary(summary_path, &plan)
            .with_context(|| format!("writing summary to {}", summary_path.display()))?;
    }

    println!(
        "ci plan: {} LEM ({}); {} lane(s); {} risk pack(s); plan -> {}",
        plan.budget.estimated_lem,
        plan.budget.band,
        plan.selection.lanes.len(),
        plan.selection.risk_packs.len(),
        args.json_out.display(),
    );
    if !plan.warnings.is_empty() {
        for w in &plan.warnings {
            eprintln!("  warning: {w}");
        }
    }
    if args.enforce_hard_ceiling && plan.budget.band == "over-ceiling" && !has_override {
        anyhow::bail!(
            "ci plan exceeds the hard ceiling ({} LEM) without a `full-ci` or `ci-budget-override` label",
            plan.budget.estimated_lem
        );
    }
    // Suppress unused after fields move into plan.
    let _ = &mut plan;
    Ok(())
}

fn resolve_base(args: &PlanArgs) -> Result<String> {
    if let Some(b) = &args.base
        && !b.is_empty()
    {
        return Ok(b.clone());
    }
    let out = Command::new("git")
        .args(["merge-base", "origin/main", "HEAD"])
        .current_dir(&args.workspace_root)
        .output();
    match out {
        Ok(o) if o.status.success() => Ok(String::from_utf8_lossy(&o.stdout).trim().to_string()),
        _ => Ok(String::new()),
    }
}

fn changed_files(root: &Path, base: &str, head: &str) -> Vec<String> {
    if base.is_empty() || head.is_empty() {
        return Vec::new();
    }
    let out = Command::new("git")
        .args(["diff", "--name-only", &format!("{base}...{head}")])
        .current_dir(root)
        .output();
    match out {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout)
            .lines()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect(),
        _ => Vec::new(),
    }
}

fn select_packs(
    packs: &BTreeMap<String, RiskPack>,
    files: &[String],
    labels: &[String],
) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for (name, pack) in packs {
        if pack
            .labels
            .iter()
            .any(|lbl| labels.iter().any(|l| l == lbl))
        {
            out.push(name.clone());
            continue;
        }

        let mut gsb = GlobSetBuilder::new();
        for p in &pack.paths {
            if let Ok(g) = Glob::new(p) {
                gsb.add(g);
            }
        }
        let matched = match gsb.build() {
            Ok(set) => files.iter().any(|f| set.is_match(f)),
            Err(_) => false,
        };
        if matched {
            out.push(name.clone());
            continue;
        }

        if pack
            .keywords
            .iter()
            .any(|kw| files.iter().any(|f| f.contains(kw)))
        {
            out.push(name.clone());
        }
    }
    out
}

fn select_lanes(
    lane_by_id: &BTreeMap<&str, &Lane>,
    runner_multipliers: &BTreeMap<String, f64>,
    packs: &BTreeMap<String, RiskPack>,
    selected_packs: &[String],
    labels: &[String],
    warnings: &mut Vec<String>,
) -> Vec<SelectedLane> {
    let mut chosen: BTreeMap<String, SelectedLane> = BTreeMap::new();

    let add = |id: &str, reason: String, chosen: &mut BTreeMap<String, SelectedLane>| {
        if chosen.contains_key(id) {
            return;
        }
        let Some(lane) = lane_by_id.get(id) else {
            return;
        };
        let lem = scaled_lem(lane, runner_multipliers);
        chosen.insert(
            id.to_string(),
            SelectedLane {
                id: id.to_string(),
                lem,
                blocking: lane.blocking,
                reason,
            },
        );
    };

    for id in FRONTDOOR_DEFAULTS {
        if !lane_by_id.contains_key(id) {
            warnings.push(format!(
                "frontdoor default `{id}` is not in the whitelist; skipping"
            ));
            continue;
        }
        add(id, "frontdoor default".to_string(), &mut chosen);
    }

    for pack_name in selected_packs {
        if let Some(pack) = packs.get(pack_name) {
            for lane in &pack.lanes {
                add(lane, format!("risk pack: {pack_name}"), &mut chosen);
            }
            if labels.iter().any(|l| l == "full-ci") {
                for lane in &pack.deep_lanes {
                    add(
                        lane,
                        format!("risk pack: {pack_name} (full-ci)"),
                        &mut chosen,
                    );
                }
            }
        }
    }

    for label in labels {
        match label.as_str() {
            "full-ci" => {
                add("pure-rust-os-matrix", "label: full-ci".into(), &mut chosen);
                add("fuzz-pr", "label: full-ci".into(), &mut chosen);
                add("benchmarks-pr", "label: full-ci".into(), &mut chosen);
            }
            "platform-matrix" => {
                add(
                    "pure-rust-os-matrix",
                    "label: platform-matrix".into(),
                    &mut chosen,
                );
            }
            "fuzz" => {
                add("fuzz-pr", "label: fuzz".into(), &mut chosen);
            }
            "ci:perf" => {
                add(
                    "performance-regression",
                    "label: ci:perf".into(),
                    &mut chosen,
                );
                add("benchmarks-pr", "label: ci:perf".into(), &mut chosen);
            }
            "ci:golden" => {
                add("golden-tests", "label: ci:golden".into(), &mut chosen);
            }
            "ci:microcrate" => {
                add("microcrate-ci", "label: ci:microcrate".into(), &mut chosen);
            }
            _ => {}
        }
    }

    chosen.into_values().collect()
}

fn scaled_lem(lane: &Lane, runner_multipliers: &BTreeMap<String, f64>) -> u32 {
    let mult = runner_multipliers.get(&lane.runner).copied().unwrap_or(1.0);
    ((lane.base_lem as f64) * mult).round() as u32
}

fn band_for(lem: u32, limits: &Option<Budget>) -> &'static str {
    let (default_limit, elevated_limit, hard_limit) = match limits {
        Some(b) => (
            b.default_limit_lem.unwrap_or(35),
            b.elevated_limit_lem.unwrap_or(75),
            b.hard_limit_lem.unwrap_or(125),
        ),
        None => (35, 75, 125),
    };
    if lem <= default_limit {
        "ordinary"
    } else if lem <= elevated_limit {
        "elevated"
    } else if lem <= hard_limit {
        "high"
    } else {
        "over-ceiling"
    }
}

fn write_summary(path: &Path, plan: &Plan) -> Result<()> {
    use std::fmt::Write;
    let mut out = String::new();
    let icon = match plan.budget.band.as_str() {
        "ordinary" => "✅",
        "elevated" => "⚠️",
        "high" => "🟠",
        "over-ceiling" => "🛑",
        _ => "·",
    };
    writeln!(out, "## CI Plan (xtask)")?;
    writeln!(out)?;
    writeln!(
        out,
        "{icon} **Estimated LEM:** {} ({})",
        plan.budget.estimated_lem, plan.budget.band
    )?;
    writeln!(out)?;
    writeln!(out, "### Risk packs")?;
    writeln!(out)?;
    if plan.selection.risk_packs.is_empty() {
        writeln!(out, "- (none)")?;
    } else {
        for p in &plan.selection.risk_packs {
            writeln!(out, "- `{p}`")?;
        }
    }
    writeln!(out)?;
    writeln!(out, "### Selected lanes")?;
    writeln!(out)?;
    writeln!(out, "| lane | LEM | blocking | reason |")?;
    writeln!(out, "| --- | ---: | :---: | --- |")?;
    for lane in &plan.selection.lanes {
        let b = if lane.blocking { "yes" } else { "no" };
        writeln!(
            out,
            "| `{}` | {} | {} | {} |",
            lane.id, lane.lem, b, lane.reason
        )?;
    }
    if !plan.warnings.is_empty() {
        writeln!(out)?;
        writeln!(out, "### Warnings")?;
        writeln!(out)?;
        for w in &plan.warnings {
            writeln!(out, "- {w}")?;
        }
    }
    let mut existing = String::new();
    if path.exists() {
        existing = fs::read_to_string(path).unwrap_or_default();
    }
    fs::write(path, existing + &out)?;
    Ok(())
}

fn read_toml<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T> {
    let body = fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    Ok(toml::from_str(&body)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    fn workspace_root_for_tests() -> PathBuf {
        // Walk up from CARGO_MANIFEST_DIR (xtask/) to the workspace root.
        let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        p.pop();
        p
    }

    fn load_real_data() -> (WhitelistFile, RiskPacksFile) {
        let root = workspace_root_for_tests();
        let wl: WhitelistFile = read_toml(&root.join("policy/ci-lane-whitelist.toml")).unwrap();
        let rp: RiskPacksFile = read_toml(&root.join("policy/ci-risk-packs.toml")).unwrap();
        (wl, rp)
    }

    #[test]
    fn whitelist_parses_and_has_frontdoor_lanes() {
        let (wl, _) = load_real_data();
        let ids: BTreeSet<&str> = wl.lanes.iter().map(|l| l.id.as_str()).collect();
        for id in FRONTDOOR_DEFAULTS {
            assert!(ids.contains(id), "missing frontdoor default lane `{id}`");
        }
    }

    #[test]
    fn risk_packs_parses_and_has_known_packs() {
        let (_, rp) = load_real_data();
        for name in [
            "core_runtime",
            "glr_core",
            "tablegen",
            "grammar_golden",
            "performance",
        ] {
            assert!(rp.risk_pack.contains_key(name), "missing pack `{name}`");
        }
    }

    #[test]
    fn docs_only_diff_picks_no_packs() {
        let (_, rp) = load_real_data();
        let files = vec!["docs/ci/ripr.md".to_string(), "README.md".to_string()];
        let packs = select_packs(&rp.risk_pack, &files, &[]);
        assert!(packs.is_empty(), "got packs: {packs:?}");
    }

    #[test]
    fn glr_change_picks_glr_pack() {
        let (_, rp) = load_real_data();
        let files = vec!["glr-core/src/lib.rs".to_string()];
        let packs = select_packs(&rp.risk_pack, &files, &[]);
        assert!(packs.contains(&"glr_core".to_string()), "got: {packs:?}");
    }

    #[test]
    fn full_ci_label_adds_deep_lanes() {
        let (wl, rp) = load_real_data();
        let lane_by_id: BTreeMap<&str, &Lane> =
            wl.lanes.iter().map(|l| (l.id.as_str(), l)).collect();
        let labels = vec!["full-ci".to_string()];
        let packs = vec!["core_runtime".to_string()];
        let mut warnings = Vec::new();
        let lanes = select_lanes(
            &lane_by_id,
            &wl.runner_multipliers,
            &rp.risk_pack,
            &packs,
            &labels,
            &mut warnings,
        );
        let ids: BTreeSet<String> = lanes.iter().map(|l| l.id.clone()).collect();
        assert!(ids.contains("pure-rust-os-matrix"));
        assert!(ids.contains("fuzz-pr"));
    }

    #[test]
    fn classify_areas_includes_docs_for_md() {
        let files = vec!["docs/ci/ripr.md".to_string(), "README.md".to_string()];
        let areas = classify_areas(&files);
        assert!(areas.contains(&"docs".to_string()), "got: {areas:?}");
    }

    #[test]
    fn classify_areas_treats_goal_manifests_as_docs() {
        let files = vec![
            ".adze/goals/active.toml".to_string(),
            ".adze/goals/archive/2026-05-16-0.9-contract-convergence.toml".to_string(),
        ];
        let areas = classify_areas(&files);
        assert_eq!(areas, vec!["docs".to_string()]);
    }

    #[test]
    fn classify_areas_includes_core_runtime_for_runtime_change() {
        let files = vec!["runtime/src/lib.rs".to_string()];
        let areas = classify_areas(&files);
        assert!(
            areas.contains(&"core_runtime".to_string()),
            "got: {areas:?}"
        );
    }

    #[test]
    fn band_thresholds() {
        let limits = Some(Budget {
            preferred_default_lem: Some(25),
            default_limit_lem: Some(35),
            elevated_limit_lem: Some(75),
            hard_limit_lem: Some(125),
        });
        assert_eq!(band_for(10, &limits), "ordinary");
        assert_eq!(band_for(35, &limits), "ordinary");
        assert_eq!(band_for(36, &limits), "elevated");
        assert_eq!(band_for(75, &limits), "elevated");
        assert_eq!(band_for(76, &limits), "high");
        assert_eq!(band_for(125, &limits), "high");
        assert_eq!(band_for(126, &limits), "over-ceiling");
    }
}
