//! Ledger-selected, dependency-ordered release crate graph.
//!
//! Source selection comes from `policy/package-boundary.toml` (`category =
//! "published"`). Ordering uses workspace `cargo metadata` normal and build
//! dependencies among that set, excluding dev-only edges and optional deps.

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, BinaryHeap};
use std::path::Path;

use crate::policy::{self, package_boundary};

pub const ARTIFACT_PATH: &str = "policy/release-graph.toml";
pub const RELEASE_CRATES_TXT_PATH: &str = "scripts/release-crates.txt";
const TARGET_ARTIFACT_DIR: &str = "target/xtask/release-graph";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReleaseGraph {
    pub schema_version: String,
    pub policy: String,
    pub source_ledger: String,
    pub selection: String,
    pub dependency_kinds: Vec<String>,
    pub ordered_crates: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct CargoMetadata {
    packages: Vec<CargoPackage>,
}

#[derive(Debug, Clone, Deserialize)]
struct CargoPackage {
    name: String,
    #[serde(default)]
    dependencies: Vec<CargoDependency>,
}

#[derive(Debug, Clone, Deserialize)]
struct CargoDependency {
    name: String,
    kind: Option<DependencyKind>,
    optional: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum DependencyKind {
    Single(String),
    Multiple(Vec<String>),
}

pub fn run_generate() -> Result<()> {
    let root = policy::workspace_root()?;
    let graph = compute_release_graph(&root)?;
    let target_dir = root.join(TARGET_ARTIFACT_DIR);
    std::fs::create_dir_all(&target_dir)
        .with_context(|| format!("creating {}", target_dir.display()))?;
    let target_path = target_dir.join("release-graph.toml");
    std::fs::write(&target_path, render_toml(&graph)?)
        .with_context(|| format!("writing {}", target_path.display()))?;

    let committed_path = root.join(ARTIFACT_PATH);
    if let Some(parent) = committed_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating {}", parent.display()))?;
    }
    std::fs::copy(&target_path, &committed_path)
        .with_context(|| format!("refreshing {}", committed_path.display()))?;
    write_release_crates_txt(&root, &graph.ordered_crates)?;

    println!(
        "release-graph: refreshed {} ({} crate(s))",
        ARTIFACT_PATH,
        graph.ordered_crates.len()
    );
    for crate_name in &graph.ordered_crates {
        println!("  {crate_name}");
    }
    Ok(())
}

pub fn run_check() -> Result<()> {
    let root = policy::workspace_root()?;
    let expected = compute_release_graph(&root)?;
    let committed_path = root.join(ARTIFACT_PATH);
    let committed_text = std::fs::read_to_string(&committed_path)
        .with_context(|| format!("reading {}", committed_path.display()))?;
    let committed = parse_artifact(&committed_text)
        .with_context(|| format!("parsing {}", committed_path.display()))?;

    if committed != expected {
        let target_dir = root.join(TARGET_ARTIFACT_DIR);
        std::fs::create_dir_all(&target_dir)
            .with_context(|| format!("creating {}", target_dir.display()))?;
        let target_path = target_dir.join("release-graph.toml");
        std::fs::write(&target_path, render_toml(&expected)?)
            .with_context(|| format!("writing {}", target_path.display()))?;
        bail!(
            "release-graph drift: committed {} is stale; run `cargo xtask generate-release-graph`",
            ARTIFACT_PATH
        );
    }

    println!(
        "release-graph: {} matches ledger-selected dependency order ({} crate(s))",
        ARTIFACT_PATH,
        committed.ordered_crates.len()
    );
    Ok(())
}

pub fn run_print() -> Result<()> {
    let root = policy::workspace_root()?;
    let graph = load_committed(&root)?;
    for crate_name in &graph.ordered_crates {
        println!("{crate_name}");
    }
    Ok(())
}

pub fn load_committed(root: &Path) -> Result<ReleaseGraph> {
    let committed_path = root.join(ARTIFACT_PATH);
    let committed_text = std::fs::read_to_string(&committed_path)
        .with_context(|| format!("reading {}", committed_path.display()))?;
    parse_artifact(&committed_text).with_context(|| format!("parsing {}", committed_path.display()))
}

pub fn ordered_crate_names(root: &Path) -> Result<Vec<String>> {
    Ok(load_committed(root)?.ordered_crates)
}

fn write_release_crates_txt(root: &Path, ordered_crates: &[String]) -> Result<()> {
    let path = root.join(RELEASE_CRATES_TXT_PATH);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating {}", parent.display()))?;
    }
    std::fs::write(&path, render_release_crates_txt(ordered_crates)?)
        .with_context(|| format!("writing {}", path.display()))?;
    Ok(())
}

fn render_release_crates_txt(ordered_crates: &[String]) -> Result<String> {
    let mut out = String::new();
    out.push_str("# Generated from policy/release-graph.toml. Do not hand-edit.\n");
    out.push_str("# Regenerate: cargo xtask generate-release-graph\n\n");
    for crate_name in ordered_crates {
        out.push_str(crate_name);
        out.push('\n');
    }
    Ok(out)
}

pub fn compute_release_graph(root: &Path) -> Result<ReleaseGraph> {
    let published = package_boundary::published_package_names(root)?;
    if published.is_empty() {
        bail!("ledger published scope is empty");
    }

    let metadata = load_cargo_metadata(root)?;
    let edges = release_dependency_edges(&metadata, &published)?;
    let ordered_crates = topo_sort_release(&published, &edges)?;

    Ok(ReleaseGraph {
        schema_version: "1.0".to_string(),
        policy: "release-graph".to_string(),
        source_ledger: package_boundary::POLICY_PATH.to_string(),
        selection: "ledger-published".to_string(),
        dependency_kinds: vec!["normal".to_string(), "build".to_string()],
        ordered_crates,
    })
}

fn load_cargo_metadata(root: &Path) -> Result<CargoMetadata> {
    let output = std::process::Command::new("cargo")
        .args(["metadata", "--format-version", "1", "--locked"])
        .current_dir(root)
        .output()
        .context("running cargo metadata --format-version 1 --locked")?;
    if !output.status.success() {
        bail!(
            "cargo metadata failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    serde_json::from_slice(&output.stdout).context("parsing cargo metadata output")
}

fn release_dependency_edges(
    metadata: &CargoMetadata,
    published: &BTreeSet<String>,
) -> Result<BTreeMap<String, BTreeSet<String>>> {
    let mut edges: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();

    for package in &metadata.packages {
        if !published.contains(&package.name) {
            continue;
        }
        for dependency in &package.dependencies {
            if dependency.optional.unwrap_or(false) {
                continue;
            }
            if !dependency_counts_for_release_order(dependency.kind.clone()) {
                continue;
            }
            if !published.contains(&dependency.name) || dependency.name == package.name {
                continue;
            }
            edges
                .entry(dependency.name.clone())
                .or_default()
                .insert(package.name.clone());
        }
    }

    Ok(edges)
}

fn dependency_counts_for_release_order(kind: Option<DependencyKind>) -> bool {
    let kinds = match kind {
        None => vec!["normal".to_string()],
        Some(DependencyKind::Single(kind)) => vec![kind],
        Some(DependencyKind::Multiple(kinds)) => kinds,
    };
    kinds.iter().any(|kind| kind == "normal" || kind == "build")
        && !kinds.iter().any(|kind| kind == "dev")
}

fn topo_sort_release(
    published: &BTreeSet<String>,
    edges: &BTreeMap<String, BTreeSet<String>>,
) -> Result<Vec<String>> {
    let mut indegree: BTreeMap<String, usize> =
        published.iter().map(|name| (name.clone(), 0)).collect();

    for dependents in edges.values() {
        for dependent in dependents {
            if let Some(count) = indegree.get_mut(dependent) {
                *count += 1;
            }
        }
    }

    let mut ready: BinaryHeap<std::cmp::Reverse<String>> = indegree
        .iter()
        .filter_map(|(name, count)| (*count == 0).then_some(std::cmp::Reverse(name.clone())))
        .collect();
    let mut ordered = Vec::with_capacity(published.len());

    while let Some(std::cmp::Reverse(crate_name)) = ready.pop() {
        ordered.push(crate_name.clone());
        if let Some(dependents) = edges.get(&crate_name) {
            for dependent in dependents {
                let count = indegree
                    .get_mut(dependent)
                    .with_context(|| format!("dependent crate `{dependent}` missing from graph"))?;
                *count -= 1;
                if *count == 0 {
                    ready.push(std::cmp::Reverse(dependent.clone()));
                }
            }
        }
    }

    if ordered.len() != published.len() {
        let unresolved: Vec<_> = indegree
            .into_iter()
            .filter(|(_, count)| *count > 0)
            .map(|(name, _)| name)
            .collect();
        bail!(
            "could not topologically order ledger-published crates; unresolved: {}",
            unresolved.join(", ")
        );
    }

    Ok(ordered)
}

fn render_toml(graph: &ReleaseGraph) -> Result<String> {
    let mut out = String::new();
    out.push_str("# Generated release graph. Do not hand-edit.\n");
    out.push_str("# Regenerate: cargo xtask generate-release-graph\n\n");
    out.push_str(&toml::to_string_pretty(graph)?);
    out.push('\n');
    Ok(out)
}

fn parse_artifact(text: &str) -> Result<ReleaseGraph> {
    toml::from_str(text).context("parsing release graph artifact")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn graph_from_edges(published: &[&str], edges: &[(&str, &str)]) -> Result<Vec<String>> {
        let published: BTreeSet<String> = published.iter().map(|s| (*s).to_string()).collect();
        let mut edge_map: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for (from, to) in edges {
            edge_map
                .entry((*from).to_string())
                .or_default()
                .insert((*to).to_string());
        }
        topo_sort_release(&published, &edge_map)
    }

    #[test]
    fn dependency_kind_filter_includes_normal_and_build_only() {
        assert!(dependency_counts_for_release_order(None));
        assert!(dependency_counts_for_release_order(Some(
            DependencyKind::Single("build".to_string())
        )));
        assert!(!dependency_counts_for_release_order(Some(
            DependencyKind::Single("dev".to_string())
        )));
        assert!(!dependency_counts_for_release_order(Some(
            DependencyKind::Multiple(vec!["dev".to_string()])
        )));
        assert!(dependency_counts_for_release_order(Some(
            DependencyKind::Multiple(vec!["normal".to_string(), "build".to_string()])
        )));
    }

    #[test]
    fn topo_sort_orders_dependencies_before_dependents_deterministically() {
        let ordered = graph_from_edges(&["b", "a", "c"], &[("a", "b"), ("b", "c"), ("a", "c")])
            .expect("graph should be acyclic");

        assert_eq!(ordered, vec!["a", "b", "c"]);
    }

    #[test]
    fn topo_sort_breaks_ties_alphabetically() {
        let ordered = graph_from_edges(&["beta", "alpha", "gamma"], &[])
            .expect("independent crates should order alphabetically");

        assert_eq!(ordered, vec!["alpha", "beta", "gamma"]);
    }

    #[test]
    fn topo_sort_reports_cycle_as_error() {
        let published: BTreeSet<String> = ["a".to_string(), "b".to_string()].into_iter().collect();
        let edges = BTreeMap::from([
            ("a".to_string(), BTreeSet::from(["b".to_string()])),
            ("b".to_string(), BTreeSet::from(["a".to_string()])),
        ]);

        let err = topo_sort_release(&published, &edges).expect_err("cycle should fail");
        assert!(
            err.to_string().contains("could not topologically order"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn release_dependency_edges_ignore_dev_only_workspace_deps() {
        let metadata = CargoMetadata {
            packages: vec![
                CargoPackage {
                    name: "published-a".to_string(),
                    dependencies: vec![],
                },
                CargoPackage {
                    name: "published-b".to_string(),
                    dependencies: vec![CargoDependency {
                        name: "published-a".to_string(),
                        kind: Some(DependencyKind::Single("normal".to_string())),
                        optional: Some(false),
                    }],
                },
                CargoPackage {
                    name: "published-c".to_string(),
                    dependencies: vec![CargoDependency {
                        name: "dev-only".to_string(),
                        kind: Some(DependencyKind::Single("dev".to_string())),
                        optional: Some(false),
                    }],
                },
            ],
        };
        let published: BTreeSet<String> = [
            "published-a".to_string(),
            "published-b".to_string(),
            "published-c".to_string(),
        ]
        .into_iter()
        .collect();

        let edges = release_dependency_edges(&metadata, &published).expect("edges");
        assert_eq!(
            edges.get("published-a").cloned().unwrap_or_default(),
            BTreeSet::from(["published-b".to_string()])
        );
        assert!(!edges.contains_key("dev-only"));
    }

    #[test]
    fn workspace_release_graph_matches_ledger_published_scope() {
        let root = policy::workspace_root().expect("workspace root");
        let graph = compute_release_graph(&root).expect("release graph");
        let published = package_boundary::published_package_names(&root).expect("ledger");

        assert_eq!(graph.ordered_crates.len(), published.len());
        assert_eq!(
            graph
                .ordered_crates
                .iter()
                .cloned()
                .collect::<BTreeSet<_>>(),
            published
        );
        assert!(!graph.ordered_crates.iter().any(|name| name == "xtask"));
        assert!(
            !graph
                .ordered_crates
                .iter()
                .any(|name| name == "adze-runtime")
        );
    }

    #[test]
    fn print_release_graph_matches_committed_artifact() {
        let root = policy::workspace_root().expect("workspace root");
        let graph = load_committed(&root).expect("committed release graph");
        let computed = compute_release_graph(&root).expect("computed release graph");

        assert_eq!(graph.ordered_crates, computed.ordered_crates);
        assert_eq!(graph.ordered_crates.len(), 12);
    }

    #[test]
    fn release_crates_txt_render_is_deterministic() {
        let ordered = vec!["alpha".to_string(), "beta".to_string()];
        let rendered = render_release_crates_txt(&ordered).expect("render");
        assert!(rendered.contains("# Generated from policy/release-graph.toml"));
        assert!(rendered.contains("alpha\nbeta\n"));
    }

    #[test]
    fn committed_release_crates_txt_matches_graph() {
        let root = policy::workspace_root().expect("workspace root");
        let graph = load_committed(&root).expect("committed release graph");
        let path = root.join(RELEASE_CRATES_TXT_PATH);
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|err| panic!("reading {}: {err}", path.display()));
        let listed: Vec<String> = text
            .lines()
            .filter(|line| !line.trim().is_empty() && !line.trim_start().starts_with('#'))
            .map(str::to_string)
            .collect();

        assert_eq!(listed, graph.ordered_crates);
    }

    #[test]
    fn workspace_release_graph_is_dependency_safe() {
        let root = policy::workspace_root().expect("workspace root");
        let graph = compute_release_graph(&root).expect("release graph");
        let metadata = load_cargo_metadata(&root).expect("metadata");
        let published: BTreeSet<String> = graph.ordered_crates.iter().cloned().collect();
        let edges = release_dependency_edges(&metadata, &published).expect("edges");
        let position: BTreeMap<_, _> = graph
            .ordered_crates
            .iter()
            .enumerate()
            .map(|(idx, name)| (name.clone(), idx))
            .collect();

        for (dependency, dependents) in edges {
            let dep_pos = position[&dependency];
            for dependent in dependents {
                assert!(
                    dep_pos < position[&dependent],
                    "{dependency} must precede {dependent} in release order"
                );
            }
        }
    }
}
