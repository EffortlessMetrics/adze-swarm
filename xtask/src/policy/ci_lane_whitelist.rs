//! CI lane whitelist lint.
//!
//! Validates `policy/ci-lane-whitelist.toml` and
//! `policy/ci-whitelist-exceptions.toml` and walks `.github/workflows/`
//! looking for jobs that are not declared in the whitelist.
//!
//! Intentionally advisory: prints findings, writes JSON to
//! `target/policy/ci-lane-whitelist.json`, and never fails the build until a
//! follow-up PR flips the mode.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use super::{Mode, ensure_report_dir, workspace_root};

#[derive(Debug, Default, Deserialize)]
#[expect(
    dead_code,
    reason = "schema fields are deserialized for whitelist validation, not all are read directly"
)]
struct WhitelistFile {
    #[serde(default)]
    schema_version: Option<String>,
    #[serde(default)]
    policy: Option<String>,
    #[serde(default)]
    owner: Option<String>,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    updated: Option<String>,
    #[serde(default)]
    budget: Option<Budget>,
    #[serde(default)]
    runner_multipliers: BTreeMap<String, f64>,
    #[serde(default, rename = "lane")]
    lanes: Vec<Lane>,
}

#[derive(Debug, Default, Deserialize)]
#[expect(
    dead_code,
    reason = "schema fields are deserialized for whitelist validation, not all are read directly"
)]
struct Budget {
    preferred_default_lem: Option<u32>,
    default_limit_lem: Option<u32>,
    elevated_limit_lem: Option<u32>,
    hard_limit_lem: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
#[expect(
    dead_code,
    reason = "schema fields are deserialized for whitelist validation, not all are read directly"
)]
pub struct Lane {
    pub id: String,
    pub workflow: String,
    /// YAML job id from the workflow file, or `"multiple"` to whitelist every
    /// job in the workflow without enumerating each one.
    pub job: String,
    /// Optional human-readable label.
    #[serde(default)]
    pub display_name: Option<String>,
    pub kind: String,
    pub tier: String,
    pub default_pr: bool,
    pub blocking: bool,
    pub runner: String,
    pub base_lem: u32,
    pub owner: String,
    pub intent: Option<String>,
    pub failure_mode: Option<String>,
    pub proof_obligation: Option<String>,
    #[serde(default)]
    pub evidence: Vec<String>,
    #[serde(default)]
    pub allowed_triggers: Vec<String>,
    #[serde(default)]
    pub duplicate_of: Vec<String>,
    #[serde(default)]
    pub expensive: bool,
    #[serde(default)]
    pub default_pr_exception: Option<String>,
    pub review_after: Option<String>,
    pub expires: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct ExceptionsFile {
    #[serde(default, rename = "exception")]
    entries: Vec<Exception>,
}

#[derive(Debug, Clone, Deserialize)]
#[expect(
    dead_code,
    reason = "schema fields are deserialized for whitelist validation, not all are read directly"
)]
pub struct Exception {
    pub id: String,
    pub kind: String,
    pub lane: String,
    pub allowed: bool,
    pub owner: String,
    pub issue: Option<String>,
    pub reason: String,
    pub created: Option<String>,
    pub review_after: Option<String>,
    pub expires: Option<String>,
}

#[derive(Debug, Default, Serialize)]
pub struct WhitelistReport {
    pub mode: String,
    pub lane_count: usize,
    pub exception_count: usize,
    pub findings: Vec<Finding>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Finding {
    pub severity: &'static str,
    pub code: &'static str,
    pub lane: Option<String>,
    pub workflow: Option<String>,
    pub message: String,
}

pub fn run_check(mode: Mode) -> Result<()> {
    let root = workspace_root()?;
    let report_dir = ensure_report_dir(&root)?;

    let whitelist_path = root.join("policy/ci-lane-whitelist.toml");
    let exceptions_path = root.join("policy/ci-whitelist-exceptions.toml");

    let whitelist: WhitelistFile = read_toml(&whitelist_path)
        .with_context(|| format!("reading {}", whitelist_path.display()))?;
    let exceptions: ExceptionsFile = if exceptions_path.exists() {
        read_toml(&exceptions_path)
            .with_context(|| format!("reading {}", exceptions_path.display()))?
    } else {
        ExceptionsFile::default()
    };

    let mut report = WhitelistReport {
        mode: format!("{:?}", mode),
        lane_count: whitelist.lanes.len(),
        exception_count: exceptions.entries.len(),
        findings: Vec::new(),
    };

    let lane_ids: BTreeSet<String> = whitelist.lanes.iter().map(|l| l.id.clone()).collect();
    let exception_ids: BTreeSet<String> = exceptions.entries.iter().map(|e| e.id.clone()).collect();

    // Internal consistency.
    for lane in &whitelist.lanes {
        if lane.intent.as_deref().unwrap_or("").trim().is_empty() {
            report.findings.push(finding(
                "warning",
                "missing-intent",
                Some(&lane.id),
                Some(&lane.workflow),
                "lane has no intent",
            ));
        }
        if lane.failure_mode.as_deref().unwrap_or("").trim().is_empty() {
            report.findings.push(finding(
                "warning",
                "missing-failure-mode",
                Some(&lane.id),
                Some(&lane.workflow),
                "lane has no failure_mode",
            ));
        }
        if lane
            .proof_obligation
            .as_deref()
            .unwrap_or("")
            .trim()
            .is_empty()
        {
            report.findings.push(finding(
                "warning",
                "missing-proof-obligation",
                Some(&lane.id),
                Some(&lane.workflow),
                "lane has no proof_obligation",
            ));
        }
        if lane.review_after.is_none() || lane.expires.is_none() {
            report.findings.push(finding(
                "warning",
                "missing-review-dates",
                Some(&lane.id),
                Some(&lane.workflow),
                "lane is missing review_after or expires",
            ));
        }
        if lane.runner != "mixed" && !whitelist.runner_multipliers.contains_key(&lane.runner) {
            report.findings.push(finding(
                "warning",
                "unknown-runner",
                Some(&lane.id),
                Some(&lane.workflow),
                &format!(
                    "runner `{}` has no multiplier in [runner_multipliers]",
                    lane.runner
                ),
            ));
        }
        for dup in &lane.duplicate_of {
            // duplicate_of may reference external ids like "justfile:ci-supported";
            // only flag references that look like lane ids (no `:`) and that miss.
            if !dup.contains(':') && !lane_ids.contains(dup) {
                report.findings.push(finding(
                    "warning",
                    "unknown-duplicate-of",
                    Some(&lane.id),
                    Some(&lane.workflow),
                    &format!("duplicate_of references unknown lane `{dup}`"),
                ));
            }
        }
        if lane.expensive && lane.default_pr {
            match &lane.default_pr_exception {
                None => report.findings.push(finding(
                    "error",
                    "missing-exception",
                    Some(&lane.id),
                    Some(&lane.workflow),
                    "expensive default-PR lane requires a default_pr_exception",
                )),
                Some(eid) if !exception_ids.contains(eid) => report.findings.push(finding(
                    "error",
                    "unknown-exception",
                    Some(&lane.id),
                    Some(&lane.workflow),
                    &format!("default_pr_exception `{eid}` not found in exceptions file"),
                )),
                _ => {}
            }
        }
        let workflow_path = root.join(&lane.workflow);
        if !workflow_path.exists() {
            report.findings.push(finding(
                "warning",
                "missing-workflow-file",
                Some(&lane.id),
                Some(&lane.workflow),
                "workflow file does not exist on disk",
            ));
        }
    }

    // Walk workflows for undeclared jobs.
    let workflows_dir = root.join(".github/workflows");
    let declared: BTreeSet<(String, String)> = whitelist
        .lanes
        .iter()
        .map(|l| (l.workflow.clone(), l.job.clone()))
        .collect();
    if workflows_dir.exists() {
        for entry in walkdir::WalkDir::new(&workflows_dir)
            .min_depth(1)
            .max_depth(1)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let ext = path.extension().and_then(|e| e.to_str());
            if !matches!(ext, Some("yml") | Some("yaml")) {
                continue;
            }
            let rel = path
                .strip_prefix(&root)
                .unwrap_or(path)
                .to_string_lossy()
                .replace('\\', "/");
            let body = std::fs::read_to_string(path)
                .with_context(|| format!("reading {}", path.display()))?;
            let jobs = scan_jobs(&body);
            for job in jobs {
                let pair = (rel.clone(), job.clone());
                let multiple = (rel.clone(), "multiple".to_string());
                if !declared.contains(&pair) && !declared.contains(&multiple) {
                    report.findings.push(finding(
                        "warning",
                        "undeclared-workflow-job",
                        None,
                        Some(&rel),
                        &format!("workflow job `{job}` is not declared in the whitelist"),
                    ));
                }
            }
        }
    }

    // Print summary.
    println!(
        "ci-lane-whitelist: {} lanes, {} exceptions, {} finding(s) ({:?} mode)",
        report.lane_count,
        report.exception_count,
        report.findings.len(),
        mode
    );
    for f in &report.findings {
        let lane = f.lane.as_deref().unwrap_or("-");
        let wf = f.workflow.as_deref().unwrap_or("-");
        println!(
            "  [{}] {} ({}, {}): {}",
            f.severity, f.code, lane, wf, f.message
        );
    }

    let json_path = report_dir.join("ci-lane-whitelist.json");
    std::fs::write(&json_path, serde_json::to_string_pretty(&report)?)
        .with_context(|| format!("writing {}", json_path.display()))?;

    if matches!(mode, Mode::BlockingAllowlist | Mode::BlockingStrict) {
        let errors = report
            .findings
            .iter()
            .filter(|f| f.severity == "error")
            .count();
        if errors > 0 {
            anyhow::bail!("ci-lane-whitelist: {errors} error finding(s) in blocking mode");
        }
    }

    Ok(())
}

fn finding(
    severity: &'static str,
    code: &'static str,
    lane: Option<&str>,
    workflow: Option<&str>,
    message: &str,
) -> Finding {
    Finding {
        severity,
        code,
        lane: lane.map(str::to_string),
        workflow: workflow.map(str::to_string),
        message: message.to_string(),
    }
}

fn read_toml<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T> {
    let body =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    Ok(toml::from_str(&body)?)
}

/// Tiny line-based job scanner for GitHub Actions workflow YAML.
///
/// Looks for the top-level `jobs:` mapping and returns the keys directly
/// underneath it. Sufficient for whitelist coverage checks; not a full YAML
/// parser.
fn scan_jobs(body: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut in_jobs = false;
    let mut jobs_indent: Option<usize> = None;

    for raw in body.lines() {
        let line = raw.trim_end();
        if line.is_empty() || line.trim_start().starts_with('#') {
            continue;
        }
        let indent = line.len() - line.trim_start().len();
        let trimmed = line.trim_start();

        if !in_jobs {
            if indent == 0 && trimmed.starts_with("jobs:") {
                in_jobs = true;
            }
            continue;
        }

        if indent == 0 {
            // left the jobs mapping
            in_jobs = false;
            jobs_indent = None;
            continue;
        }

        let job_indent = *jobs_indent.get_or_insert(indent);
        if indent != job_indent {
            continue;
        }
        if let Some((key, _)) = trimmed.split_once(':') {
            let key = key.trim();
            if !key.is_empty()
                && key
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
            {
                out.push(key.to_string());
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scan_jobs_extracts_top_level_keys() {
        let yaml = r#"
name: x
on: [push]
jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - run: echo hi
  lint:
    runs-on: ubuntu-latest
    steps:
      - run: echo hi
"#;
        assert_eq!(scan_jobs(yaml), vec!["build", "lint"]);
    }

    #[test]
    fn scan_jobs_handles_no_jobs() {
        assert!(scan_jobs("name: x\non: [push]\n").is_empty());
    }
}
