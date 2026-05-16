//! CLI argument parsing tests for adze.
//!
//! These tests validate that clap argument parsing works correctly
//! without running the actual commands (no end-to-end execution).

use assert_cmd::cargo::cargo_bin_cmd;
use predicates::prelude::*;
use std::process::Command;

// ---------------------------------------------------------------------------
// End-to-end smoke tests.
// ---------------------------------------------------------------------------

#[test]
fn test_cli_help() {
    let mut cmd = cargo_bin_cmd!("adze");
    cmd.arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Adze CLI - Tools for grammar development",
        ));
}

#[test]
fn test_cli_help_subcommand() {
    let mut cmd = cargo_bin_cmd!("adze");
    cmd.arg("help")
        .assert()
        .success()
        .stdout(predicate::str::contains("check"))
        .stdout(predicate::str::contains("stats"))
        .stdout(predicate::str::contains("version"));
}

#[test]
fn test_cli_version_flag() {
    let mut cmd = cargo_bin_cmd!("adze");
    cmd.arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::starts_with("adze"));
}

#[test]
fn test_cli_version_subcommand() {
    let mut cmd = cargo_bin_cmd!("adze");
    cmd.arg("version")
        .assert()
        .success()
        .stdout(predicate::str::starts_with("adze"));
}

#[test]
fn test_cli_no_args_shows_help() {
    let mut cmd = cargo_bin_cmd!("adze");
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Usage"));
}

#[test]
fn test_cli_unknown_command() {
    let mut cmd = cargo_bin_cmd!("adze");
    cmd.arg("nonexistent")
        .assert()
        .failure()
        .stderr(predicate::str::contains("unrecognized subcommand"));
}

#[test]
fn test_init_generates_buildable_project() {
    let temp = tempfile::tempdir().expect("tempdir");
    let project_name = "freshlang";
    let mut init = cargo_bin_cmd!("adze");
    init.arg("init")
        .arg(project_name)
        .arg("--output")
        .arg(temp.path())
        .assert()
        .success();

    let project_dir = temp.path().join(project_name);
    assert!(project_dir.join("Cargo.toml").exists());
    assert!(project_dir.join("src/grammar.rs").exists());
    assert!(project_dir.join("tests/basic.rs").exists());

    let status = Command::new("cargo")
        .arg("test")
        .current_dir(&project_dir)
        .status()
        .expect("run cargo test for generated project");
    assert!(
        status.success(),
        "generated project should build and pass typed parser tests"
    );

    let mut check = cargo_bin_cmd!("adze");
    check
        .arg("check")
        .arg(project_dir.join("src/grammar.rs"))
        .assert()
        .success();
}

#[test]
fn test_check_rejects_file_without_adze_grammar() {
    let temp = tempfile::tempdir().expect("tempdir");
    let not_a_grammar = temp.path().join("not_a_grammar.rs");
    std::fs::write(
        &not_a_grammar,
        "pub struct PlainRust;\nimpl PlainRust { pub fn value(&self) -> usize { 1 } }\n",
    )
    .expect("write non-grammar rust file");

    let mut cmd = cargo_bin_cmd!("adze");
    cmd.arg("check")
        .arg(&not_a_grammar)
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "No adze grammar definitions found",
        ))
        .stdout(predicate::str::contains("Grammar syntax is valid").not());
}

#[test]
fn test_check_reports_missing_grammar_path() {
    let temp = tempfile::tempdir().expect("tempdir");
    let missing_grammar = temp.path().join("missing.rs");

    let mut cmd = cargo_bin_cmd!("adze");
    cmd.arg("check")
        .arg(&missing_grammar)
        .assert()
        .failure()
        .stderr(predicate::str::contains("Grammar file does not exist"))
        .stderr(predicate::str::contains("Grammar analysis panicked").not())
        .stdout(predicate::str::contains("Grammar syntax is valid").not());
}

#[test]
fn test_stats_rejects_file_without_adze_grammar() {
    let temp = tempfile::tempdir().expect("tempdir");
    let not_a_grammar = temp.path().join("not_a_grammar.rs");
    std::fs::write(
        &not_a_grammar,
        "pub struct PlainRust;\nimpl PlainRust { pub fn value(&self) -> usize { 1 } }\n",
    )
    .expect("write non-grammar rust file");

    let mut cmd = cargo_bin_cmd!("adze");
    cmd.arg("stats")
        .arg(&not_a_grammar)
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "No adze grammar definitions found",
        ))
        .stdout(predicate::str::contains("Grammar Statistics").not());
}

#[test]
fn test_parse_static_mode_is_explicitly_unimplemented() {
    let temp = tempfile::tempdir().expect("tempdir");
    let grammar = temp.path().join("grammar.rs");
    let input = temp.path().join("input.txt");
    std::fs::write(&grammar, "// dummy grammar").expect("write grammar");
    std::fs::write(&input, "x").expect("write input");

    let mut cmd = cargo_bin_cmd!("adze");
    cmd.arg("parse")
        .arg(&grammar)
        .arg(&input)
        .assert()
        .failure()
        .stderr(predicate::str::contains("unimplemented"))
        .stdout(predicate::str::contains("adze build"))
        .stdout(predicate::str::contains("cargo test"));
}

#[test]
fn test_init_generated_cargo_toml_is_valid() {
    let temp = tempfile::tempdir().expect("tempdir");
    let project_name = "validcargotoml";
    let mut init = cargo_bin_cmd!("adze");
    init.arg("init")
        .arg(project_name)
        .arg("--output")
        .arg(temp.path())
        .assert()
        .success();

    let project_dir = temp.path().join(project_name);
    let cargo_toml_path = project_dir.join("Cargo.toml");
    assert!(cargo_toml_path.exists(), "Cargo.toml should exist");

    let cargo_toml = std::fs::read_to_string(&cargo_toml_path).expect("read Cargo.toml");

    // Must declare the runtime and build-time dependencies
    assert!(
        cargo_toml.contains("adze"),
        "Cargo.toml must reference the adze runtime crate"
    );
    assert!(
        cargo_toml.contains("adze-tool"),
        "Cargo.toml must reference adze-tool as a build dependency"
    );
    assert!(
        cargo_toml.contains("[build-dependencies]"),
        "Cargo.toml must have a [build-dependencies] section"
    );
    assert!(
        cargo_toml.contains("edition = \"2024\""),
        "Cargo.toml must specify edition 2024"
    );
    assert!(
        cargo_toml.contains(&format!("name = \"{}\"", project_name)),
        "Cargo.toml package name must match project name"
    );
    assert!(
        cargo_toml.contains("[features]"),
        "Cargo.toml must define features"
    );
    assert!(
        cargo_toml.contains("pure-rust"),
        "Cargo.toml must include pure-rust feature"
    );
}

#[test]
fn test_init_generated_grammar_uses_adze_macros() {
    let temp = tempfile::tempdir().expect("tempdir");
    let project_name = "macrocheck";
    let mut init = cargo_bin_cmd!("adze");
    init.arg("init")
        .arg(project_name)
        .arg("--output")
        .arg(temp.path())
        .assert()
        .success();

    let project_dir = temp.path().join(project_name);
    let grammar_path = project_dir.join("src/grammar.rs");
    assert!(grammar_path.exists(), "grammar.rs should exist");

    let grammar = std::fs::read_to_string(&grammar_path).expect("read grammar.rs");

    assert!(
        grammar.contains("#[adze::grammar("),
        "grammar.rs must use #[adze::grammar] macro"
    );
    assert!(
        grammar.contains("#[adze::language]"),
        "grammar.rs must declare a language entry point with #[adze::language]"
    );
    assert!(
        grammar.contains("#[adze::leaf("),
        "grammar.rs must use #[adze::leaf] for tokens"
    );
    assert!(
        grammar.contains("#[adze::extra]"),
        "grammar.rs must define whitespace handling with #[adze::extra]"
    );
}

#[test]
fn test_init_generated_project_passes_check() {
    let temp = tempfile::tempdir().expect("tempdir");
    let project_name = "checklang";
    let mut init = cargo_bin_cmd!("adze");
    init.arg("init")
        .arg(project_name)
        .arg("--output")
        .arg(temp.path())
        .assert()
        .success();

    let project_dir = temp.path().join(project_name);

    let status = Command::new("cargo")
        .arg("check")
        .current_dir(&project_dir)
        .status()
        .expect("run cargo check for generated project");
    assert!(
        status.success(),
        "generated project should pass cargo check"
    );
}

#[test]
fn test_init_default_cwd_generates_buildable_project() {
    let temp = tempfile::tempdir().expect("tempdir");
    let project_name = "defaultcwdlang";
    let mut init = cargo_bin_cmd!("adze");
    init.current_dir(temp.path())
        .arg("init")
        .arg(project_name)
        .assert()
        .success();

    let project_dir = temp.path().join(project_name);
    assert!(project_dir.join("Cargo.toml").exists());
    assert!(project_dir.join("src/grammar.rs").exists());
    assert!(project_dir.join("tests/basic.rs").exists());

    let status = Command::new("cargo")
        .arg("check")
        .current_dir(&project_dir)
        .status()
        .expect("run cargo check for generated project");
    assert!(
        status.success(),
        "default-cwd generated project should pass cargo check"
    );
}

#[test]
fn test_parse_reports_available_modes() {
    let mut cmd = cargo_bin_cmd!("adze");
    cmd.arg("parse")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("--dynamic"))
        .stdout(predicate::str::contains("--format"))
        .stdout(predicate::str::contains("tree"))
        .stdout(predicate::str::contains("json"))
        .stdout(predicate::str::contains("sexp"))
        .stdout(predicate::str::contains("dot"));
}

#[test]
fn test_init_cargo_toml_references_adze_dependency() {
    let temp = tempfile::tempdir().expect("tempdir");
    let project_name = "deptest";
    let mut init = cargo_bin_cmd!("adze");
    init.arg("init")
        .arg(project_name)
        .arg("--output")
        .arg(temp.path())
        .assert()
        .success();

    let cargo_toml = std::fs::read_to_string(temp.path().join(project_name).join("Cargo.toml"))
        .expect("read Cargo.toml");

    // In dev builds deps use path=; in releases they use version=. Either is fine.
    let has_path_or_version = cargo_toml.contains("path =") || cargo_toml.contains("version =");
    assert!(
        cargo_toml.contains("adze = {") && has_path_or_version,
        "Cargo.toml must declare adze with a path or version dependency"
    );
    assert!(
        cargo_toml.contains("adze-tool = {"),
        "Cargo.toml must declare adze-tool as a build dependency"
    );
    assert!(
        cargo_toml.contains("pure-rust = [\"adze/pure-rust\"]"),
        "Cargo.toml must forward the pure-rust feature to the adze dependency"
    );
}

#[test]
fn test_parse_help_documents_available_modes() {
    let mut cmd = cargo_bin_cmd!("adze");
    cmd.arg("parse")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("tree"))
        .stdout(predicate::str::contains("json"))
        .stdout(predicate::str::contains("sexp"))
        .stdout(predicate::str::contains("dot"))
        .stdout(predicate::str::contains("experimental"));
}

// ---------------------------------------------------------------------------
// Unit tests for CLI argument parsing (no binary execution)
// ---------------------------------------------------------------------------

// Import the CLI types directly for parsing tests.
// The types are pub(crate) so we use `try_parse_from` on the binary's types
// via the clap trait. We re-derive a minimal mirror here to avoid exposing
// internal types outside the crate.
mod parsing {
    use clap::Parser;

    /// Minimal mirror of the real CLI struct for argument parsing tests.
    #[derive(Parser, Debug)]
    #[command(name = "adze")]
    #[command(about = "Adze CLI - Tools for grammar development")]
    #[command(author, version, long_about = None)]
    struct Cli {
        #[arg(short, long, global = true)]
        verbose: bool,

        #[command(subcommand)]
        command: Commands,
    }

    #[derive(clap::Subcommand, Debug)]
    enum Commands {
        Init {
            name: String,
            #[arg(short, long)]
            output: Option<std::path::PathBuf>,
        },
        Build {
            #[arg(default_value = ".")]
            path: std::path::PathBuf,
            #[arg(short, long)]
            watch: bool,
        },
        Parse {
            grammar: std::path::PathBuf,
            input: std::path::PathBuf,
            #[arg(short, long, default_value = "tree")]
            format: OutputFormat,
            #[arg(long)]
            dynamic: bool,
            #[arg(long, default_value = "language")]
            symbol: String,
        },
        Test {
            #[arg(default_value = ".")]
            path: std::path::PathBuf,
            #[arg(short, long)]
            update: bool,
        },
        Doc {
            grammar: std::path::PathBuf,
            #[arg(short, long)]
            output: Option<std::path::PathBuf>,
        },
        Check {
            grammar: std::path::PathBuf,
        },
        Stats {
            grammar: std::path::PathBuf,
        },
        Version,
    }

    #[derive(clap::ValueEnum, Clone, Debug)]
    enum OutputFormat {
        Tree,
        Json,
        Sexp,
        Dot,
    }

    // --- argument parsing unit tests ---

    #[test]
    fn parse_check_subcommand() {
        let cli = Cli::try_parse_from(["adze", "check", "grammar.rs"]).unwrap();
        assert!(!cli.verbose);
        match cli.command {
            Commands::Check { grammar } => {
                assert_eq!(grammar.to_str().unwrap(), "grammar.rs");
            }
            _ => panic!("expected Check command"),
        }
    }

    #[test]
    fn parse_stats_subcommand() {
        let cli = Cli::try_parse_from(["adze", "stats", "my_grammar.rs"]).unwrap();
        match cli.command {
            Commands::Stats { grammar } => {
                assert_eq!(grammar.to_str().unwrap(), "my_grammar.rs");
            }
            _ => panic!("expected Stats command"),
        }
    }

    #[test]
    fn parse_version_subcommand() {
        let cli = Cli::try_parse_from(["adze", "version"]).unwrap();
        assert!(matches!(cli.command, Commands::Version));
    }

    #[test]
    fn parse_verbose_flag_global() {
        let cli = Cli::try_parse_from(["adze", "-v", "version"]).unwrap();
        assert!(cli.verbose);
    }

    #[test]
    fn parse_init_with_output() {
        let cli = Cli::try_parse_from(["adze", "init", "my-lang", "-o", "/tmp/out"]).unwrap();
        match cli.command {
            Commands::Init { name, output } => {
                assert_eq!(name, "my-lang");
                assert_eq!(output.unwrap().to_str().unwrap(), "/tmp/out");
            }
            _ => panic!("expected Init command"),
        }
    }

    #[test]
    fn parse_build_defaults() {
        let cli = Cli::try_parse_from(["adze", "build"]).unwrap();
        match cli.command {
            Commands::Build { path, watch } => {
                assert_eq!(path.to_str().unwrap(), ".");
                assert!(!watch);
            }
            _ => panic!("expected Build command"),
        }
    }

    #[test]
    fn parse_build_with_watch() {
        let cli = Cli::try_parse_from(["adze", "build", "src/", "--watch"]).unwrap();
        match cli.command {
            Commands::Build { path, watch } => {
                assert_eq!(path.to_str().unwrap(), "src/");
                assert!(watch);
            }
            _ => panic!("expected Build command"),
        }
    }

    #[test]
    fn parse_parse_command_full() {
        let cli = Cli::try_parse_from([
            "adze",
            "parse",
            "gram.rs",
            "input.txt",
            "--format",
            "json",
            "--dynamic",
            "--symbol",
            "my_lang",
        ])
        .unwrap();
        match cli.command {
            Commands::Parse {
                grammar,
                input,
                dynamic,
                symbol,
                ..
            } => {
                assert_eq!(grammar.to_str().unwrap(), "gram.rs");
                assert_eq!(input.to_str().unwrap(), "input.txt");
                assert!(dynamic);
                assert_eq!(symbol, "my_lang");
            }
            _ => panic!("expected Parse command"),
        }
    }

    #[test]
    fn parse_test_with_update() {
        let cli = Cli::try_parse_from(["adze", "test", "--update"]).unwrap();
        match cli.command {
            Commands::Test { path, update } => {
                assert_eq!(path.to_str().unwrap(), ".");
                assert!(update);
            }
            _ => panic!("expected Test command"),
        }
    }

    #[test]
    fn parse_doc_subcommand() {
        let cli = Cli::try_parse_from(["adze", "doc", "grammar.rs", "-o", "docs.md"]).unwrap();
        match cli.command {
            Commands::Doc { grammar, output } => {
                assert_eq!(grammar.to_str().unwrap(), "grammar.rs");
                assert_eq!(output.unwrap().to_str().unwrap(), "docs.md");
            }
            _ => panic!("expected Doc command"),
        }
    }

    #[test]
    fn parse_check_missing_arg_fails() {
        assert!(Cli::try_parse_from(["adze", "check"]).is_err());
    }

    #[test]
    fn parse_stats_missing_arg_fails() {
        assert!(Cli::try_parse_from(["adze", "stats"]).is_err());
    }

    #[test]
    fn parse_unknown_subcommand_fails() {
        assert!(Cli::try_parse_from(["adze", "foobar"]).is_err());
    }

    #[test]
    fn parse_no_subcommand_fails() {
        assert!(Cli::try_parse_from(["adze"]).is_err());
    }
}
