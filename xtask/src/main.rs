use anyhow::Result;
use clap::{Parser, Subcommand};
use xshell::Shell;

mod badges;
mod baseline;
mod bench;
mod ci_plan;
mod corpus;
mod dashboard;
mod debug_blocks;
mod doctor;
mod fixtures;
mod golden;
mod goto_indexing;
mod grammar_json;
mod lint;
mod no_mangle;
mod policy;
mod profile;
mod ripr;
mod test_grammars;
mod test_local_grammars;

#[derive(Parser)]
#[command(author, version, about = "Adze development tasks")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Generate golden test files from tree-sitter
    GenerateGolden {
        /// Grammar to generate golden files for
        #[arg(value_enum)]
        grammar: Grammar,
        /// Force regeneration even if files exist
        #[arg(short, long)]
        force: bool,
    },
    /// Compare generated output against golden files
    DiffGolden {
        /// Grammar to compare
        #[arg(value_enum)]
        grammar: Grammar,
        /// Show detailed diff output
        #[arg(short, long)]
        verbose: bool,
    },
    /// Update golden files with current output
    UpdateGolden {
        /// Grammar to update
        #[arg(value_enum)]
        grammar: Grammar,
    },
    /// Run all golden tests
    TestGolden {
        /// Show detailed output for failures
        #[arg(short, long)]
        verbose: bool,
    },
    /// Download Tree-sitter grammar corpus
    DownloadCorpus {
        /// Target directory for corpus
        #[arg(short, long, default_value = "./corpus")]
        target: String,
    },
    /// Test grammars against Tree-sitter corpus
    TestCorpus {
        /// Path to corpus directory
        #[arg(short, long, default_value = "./corpus")]
        corpus: String,
        /// Output directory for results
        #[arg(short, long, default_value = "./target/corpus-results")]
        output: String,
    },
    /// Test a specific grammar from the corpus
    TestGrammar {
        /// Grammar name (e.g., javascript, rust, python)
        grammar: String,
        /// Path to corpus directory
        #[arg(short, long, default_value = "./corpus")]
        corpus: String,
    },
    /// Generate dashboard data from test results
    DashboardData {
        /// Input directory with test results
        #[arg(short, long, default_value = "./target/corpus-results")]
        input: String,
        /// Output file for dashboard data
        #[arg(short, long, default_value = "./dashboard/data.json")]
        output: String,
    },
    /// Initialize dashboard project
    InitDashboard {
        /// Dashboard directory
        #[arg(short, long, default_value = "./dashboard")]
        dir: String,
    },
    /// Test top 20 grammars for compatibility
    TestGrammars {
        /// Output format
        #[arg(short, long, value_enum, default_value = "markdown")]
        format: OutputFormat,
    },
    /// Test local grammar examples
    TestLocal,
    /// Test fixture grammars with pure-Rust backend
    TestPureRust {
        /// Grammar to test (python, rust, c)
        #[arg(value_enum)]
        grammar: Grammar,
        /// Show detailed output
        #[arg(short, long)]
        verbose: bool,
    },
    /// Run benchmarks with optional baseline saving
    Bench {
        /// Save results as a new baseline
        #[arg(long)]
        save_baseline: bool,
        /// Baseline name (defaults to version from Cargo.toml)
        #[arg(long)]
        baseline_name: Option<String>,
    },
    /// Profile CPU or memory usage
    Profile {
        /// Profile type: cpu or memory
        #[arg(value_enum)]
        profile_type: ProfileType,
        /// Grammar to profile
        #[arg(value_enum)]
        grammar: ProfileGrammar,
        /// Fixture size
        #[arg(value_enum)]
        size: FixtureSize,
        /// Output JSON metrics
        #[arg(long)]
        json: bool,
    },
    /// Save current benchmark results as a baseline (without running benchmarks)
    SaveBaseline {
        /// Baseline version name (e.g., "v0.8.0")
        version: String,
    },
    /// Compare current benchmarks against baseline
    CompareBaseline {
        /// Baseline version to compare against (e.g., "v0.8.0")
        baseline_version: String,
        /// Regression threshold percentage (default: 5.0)
        #[arg(long, default_value = "5.0")]
        threshold: f64,
    },
    /// Run local environment doctor checks (toolchain, targets, workspace)
    Doctor,
    /// Generate or check public Shields endpoint JSON under badges/.
    Badges {
        /// Check committed endpoints for drift without updating badges/.
        #[arg(long)]
        check: bool,
    },
    /// Produce or check PR-scoped RIPR repository exposure evidence.
    RiprPr {
        /// Check target/ripr/pr output contract without running ripr.
        #[arg(long)]
        check: bool,
    },
    /// Produce or check PR-scoped RIPR review guidance.
    RiprReviewComments {
        /// Check target/ripr/review output contract without running ripr.
        #[arg(long)]
        check: bool,
    },
    /// Run all lint checks (fmt -> no-mangle -> debug-block validator -> clippy)
    ///
    /// Examples:
    ///   cargo xtask lint --fast               # 3-5s: fmt/validator/no-mangle + clippy on core crates
    ///   cargo xtask lint --changed-only       # pre-commit mirror (staged .rs)
    ///   cargo xtask lint --since origin/main  # PR-diff mirror
    ///   cargo xtask lint --fix                # auto-fix formatting and debug blocks
    Lint {
        /// Auto-fix debug blocks (adds `// );` where missing) and run `cargo fmt` write-mode
        #[arg(long)]
        fix: bool,
        /// Only scan staged .rs files (uses Git index)
        #[arg(long)]
        changed_only: bool,
        /// Scan diff since a Git rev/range (e.g. `main`, `origin/main`, `abc123..HEAD`)
        #[arg(long, value_name = "REV")]
        since: Option<String>,
        /// Fast mode: skip self-tests and limit clippy to core crates (3-5s checks)
        #[arg(long)]
        fast: bool,
        /// Extra args passed to `cargo clippy` after `--`
        #[arg(last = true)]
        clippy_args: Vec<String>,
    },
    /// Generate arithmetic expression fixtures for benchmarking
    GenerateFixtures {
        /// Output directory for fixtures
        #[arg(short, long, default_value = "benchmarks/fixtures/arithmetic")]
        output: String,
        /// Force regeneration even if files exist
        #[arg(short, long)]
        force: bool,
    },
    /// Validate existing arithmetic fixtures
    ValidateFixtures {
        /// Fixtures directory to validate
        #[arg(short = 'd', long, default_value = "benchmarks/fixtures/arithmetic")]
        dir: String,
    },
    /// Show information about generated fixtures
    FixturesInfo {
        /// Fixtures directory
        #[arg(short = 'd', long, default_value = "benchmarks/fixtures/arithmetic")]
        dir: String,
    },
    /// Check commented debug blocks in Rust files.
    CheckDebugBlocks {
        /// Auto-insert missing `// );` after commented debug blocks.
        #[arg(long)]
        fix: bool,
        /// Only check staged .rs files in the index.
        #[arg(long)]
        changed_only: bool,
        /// Only check files changed since REV.
        #[arg(long, value_name = "REV")]
        since: Option<String>,
        /// Files to check (defaults to Git-tracked .rs files).
        files: Vec<String>,
    },
    /// Check Rust source for bare #[no_mangle] attributes.
    CheckNoMangle {
        /// Files to check (defaults to Git-tracked .rs files).
        files: Vec<String>,
    },
    /// Check GOTO indexing remapping invariants.
    CheckGotoIndexing {
        /// Files to check (defaults to Git-tracked .rs files).
        files: Vec<String>,
    },
    /// Check the workspace for unreceipted panic-family debt.
    ///
    /// See docs/NO_PANIC_POLICY.md.
    CheckNoPanicFamily {
        /// Operating mode: advisory | blocking-allowlist | blocking-strict
        #[arg(long, default_value = "advisory")]
        mode: String,
    },
    /// Propose new no-panic allowlist entries for current findings.
    NoPanicPropose {
        /// Treat every finding as a new entry, ignoring existing matches.
        #[arg(long)]
        baseline: bool,
    },
    /// Verify non-Rust files against policy/non-rust-allowlist.toml.
    CheckFilePolicy {
        /// Operating mode: advisory | blocking-allowlist | blocking-strict
        #[arg(long, default_value = "advisory")]
        mode: String,
    },
    /// Verify Cargo / clippy.toml configuration matches policy/clippy-lints.toml.
    CheckLintPolicy {
        /// Operating mode: advisory | blocking-allowlist | blocking-strict
        #[arg(long, default_value = "advisory")]
        mode: String,
    },
    /// Run every policy check and emit a combined Markdown report.
    PolicyReport,
    /// Verify workspace packages against policy/package-boundary.toml.
    CheckPackageBoundary {
        /// Operating mode: advisory | blocking-allowlist | blocking-strict
        #[arg(long, default_value = "blocking-allowlist")]
        mode: String,
        /// Fail while any owner-module migration target remains.
        ///
        /// This is intended for release-candidate validation. Routine package
        /// collapse PRs should use the default transition check so the ledger
        /// can track remaining work without making main permanently red.
        #[arg(long)]
        release_gate: bool,
    },
    /// Lint workflows against policy/ci-lane-whitelist.toml.
    ///
    /// Reports undeclared workflow jobs, missing exceptions for expensive
    /// default-PR lanes, missing fields, unknown runners, and dangling
    /// duplicate-of references. Advisory by default.
    CheckCiLaneWhitelist {
        /// Operating mode: advisory | blocking-allowlist | blocking-strict
        #[arg(long, default_value = "advisory")]
        mode: String,
    },
    /// Validate policy/doc-artifacts.toml: paths exist, IDs unique, links resolve.
    CheckDocArtifacts {
        /// Operating mode: advisory (warnings only) | blocking (fail on errors).
        #[arg(long, default_value = "blocking")]
        mode: String,
    },
    /// Validate .adze/goals/active.toml: fields present, IDs unique, references valid.
    CheckActiveGoal {
        /// Operating mode: advisory (warnings only) | blocking (fail on errors).
        #[arg(long, default_value = "blocking")]
        mode: String,
    },
    /// Compute the CI plan for the current PR (LEM + lane selection).
    ///
    /// Reads policy/ci-lane-whitelist.toml and policy/ci-risk-packs.toml,
    /// classifies the changed file set, picks lanes, and emits
    /// target/ci/ci-plan.json plus an optional GitHub step summary.
    CiPlan {
        /// Base SHA (defaults to merge-base with origin/main).
        #[arg(long)]
        base: Option<String>,
        /// Head SHA (defaults to HEAD).
        #[arg(long)]
        head: Option<String>,
        /// Comma-separated label names.
        #[arg(long, default_value = "")]
        labels: String,
        /// Output JSON path.
        #[arg(long, default_value = "target/ci/ci-plan.json")]
        json_out: String,
        /// Path to append a Markdown step summary (typically $GITHUB_STEP_SUMMARY).
        #[arg(long)]
        github_summary: Option<String>,
        /// Path to whitelist TOML.
        #[arg(long, default_value = "policy/ci-lane-whitelist.toml")]
        whitelist: String,
        /// Path to risk-packs TOML.
        #[arg(long, default_value = "policy/ci-risk-packs.toml")]
        risk_packs: String,
        /// Fail when the plan exceeds the hard ceiling without an override label.
        #[arg(long)]
        enforce_hard_ceiling: bool,
    },
}

#[derive(clap::ValueEnum, Clone, Copy, Debug)]
enum OutputFormat {
    Markdown,
    Json,
    Console,
}

#[derive(clap::ValueEnum, Clone, Copy, Debug)]
pub enum Grammar {
    Arithmetic,
    Javascript,
    Rust,
    Python,
    C,
}

#[derive(clap::ValueEnum, Clone, Copy, Debug)]
enum ProfileType {
    Cpu,
    Memory,
}

impl From<ProfileType> for profile::ProfileType {
    fn from(pt: ProfileType) -> Self {
        match pt {
            ProfileType::Cpu => profile::ProfileType::Cpu,
            ProfileType::Memory => profile::ProfileType::Memory,
        }
    }
}

#[derive(clap::ValueEnum, Clone, Copy, Debug)]
enum ProfileGrammar {
    Python,
    Javascript,
    Arithmetic,
}

impl From<ProfileGrammar> for profile::ProfileGrammar {
    fn from(pg: ProfileGrammar) -> Self {
        match pg {
            ProfileGrammar::Python => profile::ProfileGrammar::Python,
            ProfileGrammar::Javascript => profile::ProfileGrammar::Javascript,
            ProfileGrammar::Arithmetic => profile::ProfileGrammar::Arithmetic,
        }
    }
}

#[derive(clap::ValueEnum, Clone, Copy, Debug)]
enum FixtureSize {
    Small,
    Medium,
    Large,
}

impl From<FixtureSize> for profile::FixtureSize {
    fn from(fs: FixtureSize) -> Self {
        match fs {
            FixtureSize::Small => profile::FixtureSize::Small,
            FixtureSize::Medium => profile::FixtureSize::Medium,
            FixtureSize::Large => profile::FixtureSize::Large,
        }
    }
}

impl Grammar {
    fn name(&self) -> &'static str {
        match self {
            Grammar::Arithmetic => "arithmetic",
            Grammar::Javascript => "javascript",
            Grammar::Rust => "rust",
            Grammar::Python => "python",
            Grammar::C => "c",
        }
    }

    fn repo_url(&self) -> Option<&'static str> {
        match self {
            Grammar::Arithmetic => None, // Local example
            Grammar::Javascript => Some("https://github.com/tree-sitter/tree-sitter-javascript"),
            Grammar::Rust => Some("https://github.com/tree-sitter/tree-sitter-rust"),
            Grammar::Python => Some("https://github.com/tree-sitter/tree-sitter-python"),
            Grammar::C => Some("https://github.com/tree-sitter/tree-sitter-c"),
        }
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let sh = Shell::new()?;

    match cli.command {
        Commands::Doctor => {
            doctor::run()?;
        }
        Commands::Badges { check } => {
            badges::run(check)?;
        }
        Commands::RiprPr { check } => {
            ripr::run_pr(check)?;
        }
        Commands::RiprReviewComments { check } => {
            ripr::run_review_comments(check)?;
        }
        Commands::GenerateGolden { grammar, force } => {
            golden::generate_golden(&sh, grammar, force)?;
        }
        Commands::DiffGolden { grammar, verbose } => {
            golden::diff_golden(&sh, grammar, verbose)?;
        }
        Commands::UpdateGolden { grammar } => {
            golden::update_golden(&sh, grammar)?;
        }
        Commands::TestGolden { verbose } => {
            golden::test_all_golden(&sh, verbose)?;
        }
        Commands::DownloadCorpus { target } => {
            corpus::download_corpus(std::path::Path::new(&target))?;
        }
        Commands::TestCorpus { corpus, output } => {
            let runner = corpus::CorpusRunner::new(corpus.into(), output.into());
            let results = runner.run_all()?;
            println!(
                "\nCorpus test complete: {:.1}% pass rate",
                results.pass_rate
            );
        }
        Commands::TestGrammar { grammar, corpus } => {
            let runner = corpus::CorpusRunner::new(corpus.into(), "./target/corpus-results".into());
            let result = runner.test_grammar(&grammar)?;
            println!("Grammar {} status: {:?}", grammar, result.status);
        }
        Commands::DashboardData { input, output } => {
            dashboard::generate_dashboard_data(
                std::path::Path::new(&input),
                std::path::Path::new(&output),
            )?;
        }
        Commands::InitDashboard { dir } => {
            dashboard::init_dashboard(std::path::Path::new(&dir))?;
        }
        Commands::TestGrammars { format: _ } => {
            test_grammars::run_corpus_tests()?;
        }
        Commands::TestLocal => {
            test_local_grammars::test_local_grammars()?;
        }
        Commands::TestPureRust { grammar, verbose } => {
            test_grammars::test_pure_rust(&sh, grammar, verbose)?;
        }
        Commands::Bench {
            save_baseline,
            baseline_name,
        } => {
            bench::run_benchmarks(&sh, save_baseline, baseline_name)?;
        }
        Commands::Profile {
            profile_type,
            grammar,
            size,
            json,
        } => {
            profile::profile(&sh, profile_type.into(), grammar.into(), size.into(), json)?;
        }
        Commands::SaveBaseline { version } => {
            baseline::save_baseline(&sh, &version)?;
        }
        Commands::CompareBaseline {
            baseline_version,
            threshold,
        } => {
            baseline::compare_baseline(&sh, &baseline_version, threshold)?;
        }
        Commands::Lint {
            fix,
            changed_only,
            since,
            fast,
            clippy_args,
        } => {
            lint::lint(&sh, fix, changed_only, since, fast, clippy_args)?;
        }
        Commands::GenerateFixtures { output, force } => {
            fixtures::generate_fixtures(&sh, &output, force)?;
        }
        Commands::CheckDebugBlocks {
            fix,
            changed_only,
            since,
            files,
        } => {
            debug_blocks::run(debug_blocks::DebugBlockOptions {
                fix,
                changed_only,
                since,
                files: files.into_iter().map(Into::into).collect(),
            })?;
        }
        Commands::CheckNoMangle { files } => {
            no_mangle::run(files.into_iter().map(Into::into).collect())?;
        }
        Commands::CheckGotoIndexing { files } => {
            goto_indexing::run(files.into_iter().map(Into::into).collect())?;
        }
        Commands::ValidateFixtures { dir } => {
            fixtures::validate_only(&sh, &dir)?;
        }
        Commands::FixturesInfo { dir } => {
            fixtures::info_fixtures(&dir)?;
        }
        Commands::CheckNoPanicFamily { mode } => {
            let mode = policy::Mode::parse(&mode)?;
            policy::no_panic::run_check(mode)?;
        }
        Commands::NoPanicPropose { baseline } => {
            policy::no_panic::run_propose(baseline)?;
        }
        Commands::CheckFilePolicy { mode } => {
            let mode = policy::Mode::parse(&mode)?;
            policy::file_policy::run_check(mode)?;
        }
        Commands::CheckLintPolicy { mode } => {
            let mode = policy::Mode::parse(&mode)?;
            policy::lint_policy::run_check(mode)?;
        }
        Commands::PolicyReport => {
            policy::report::run()?;
        }
        Commands::CheckPackageBoundary { mode, release_gate } => {
            let mode = policy::Mode::parse(&mode)?;
            policy::package_boundary::run_check(mode, release_gate)?;
        }
        Commands::CheckCiLaneWhitelist { mode } => {
            let mode = policy::Mode::parse(&mode)?;
            policy::ci_lane_whitelist::run_check(mode)?;
        }
        Commands::CheckDocArtifacts { mode } => {
            policy::doc_artifacts::run(&mode)?;
        }
        Commands::CheckActiveGoal { mode } => {
            policy::active_goal::run(&mode)?;
        }
        Commands::CiPlan {
            base,
            head,
            labels,
            json_out,
            github_summary,
            whitelist,
            risk_packs,
            enforce_hard_ceiling,
        } => {
            let workspace_root = policy::workspace_root()?;
            let labels_vec: Vec<String> = labels
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            let github_summary = github_summary
                .map(std::path::PathBuf::from)
                .or_else(|| std::env::var("GITHUB_STEP_SUMMARY").ok().map(Into::into));
            let args = ci_plan::PlanArgs {
                workspace_root: workspace_root.clone(),
                base,
                head,
                labels: labels_vec,
                whitelist_path: workspace_root.join(whitelist),
                risk_packs_path: workspace_root.join(risk_packs),
                json_out: workspace_root.join(json_out),
                github_summary,
                enforce_hard_ceiling,
            };
            ci_plan::run(args)?;
        }
    }

    Ok(())
}
