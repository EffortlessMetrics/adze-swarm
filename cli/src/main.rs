use adze_tool::{build_parsers, pure_rust_builder::BuildResult};
use anyhow::Result;
use clap::{Parser, Subcommand};
use colored::Colorize;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

mod grammar_analysis;

use grammar_analysis::analyze_grammar_file;

/// Adze CLI
#[derive(Parser, Debug)]
#[command(name = "adze")]
#[command(about = "Adze CLI - Tools for grammar development")]
#[command(author, version, long_about = None)]
pub(crate) struct Cli {
    /// Enable verbose output
    #[arg(short, long, global = true)]
    pub verbose: bool,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub(crate) enum Commands {
    /// Initialize a new adze grammar project
    Init {
        /// Name of the grammar
        name: String,
        /// Output directory (defaults to current directory)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Build grammar parsers
    Build {
        /// Path to the grammar file or directory
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Watch for changes and rebuild automatically
        #[arg(short, long)]
        watch: bool,
    },

    /// Parse a file using the grammar
    Parse {
        /// Grammar file (or .so/.dylib path when using --dynamic)
        grammar: PathBuf,
        /// Input file to parse
        input: PathBuf,
        /// Output format
        #[arg(short, long, visible_alias = "output", default_value = "tree")]
        format: OutputFormat,
        /// Use dynamic loader to load compiled grammar from shared library (experimental; requires --features dynamic)
        #[arg(long)]
        dynamic: bool,
        /// Optional exported symbol (default: "language")
        #[arg(long, default_value = "language")]
        symbol: String,
    },

    /// Test grammar against test files
    Test {
        /// Path to grammar directory
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Update test snapshots
        #[arg(short, long)]
        update: bool,
    },

    /// Generate grammar documentation
    Doc {
        /// Path to grammar file
        grammar: PathBuf,
        /// Output file (defaults to stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Validate grammar syntax
    Check {
        /// Path to grammar file
        grammar: PathBuf,
    },

    /// Show grammar statistics
    Stats {
        /// Path to grammar file
        grammar: PathBuf,
    },

    /// Show version information
    Version,
}

#[derive(clap::ValueEnum, Clone, Debug)]
pub(crate) enum OutputFormat {
    Tree,
    Json,
    Sexp,
    Dot,
    #[value(name = "document-json")]
    DocumentJson,
    #[value(name = "tree-json")]
    TreeJson,
    #[value(name = "diagnostics-json")]
    DiagnosticsJson,
    #[value(name = "ambiguity-json")]
    AmbiguityJson,
}

impl OutputFormat {
    fn cli_name(&self) -> &'static str {
        match self {
            Self::Tree => "tree",
            Self::Json => "json",
            Self::Sexp => "sexp",
            Self::Dot => "dot",
            Self::DocumentJson => "document-json",
            Self::TreeJson => "tree-json",
            Self::DiagnosticsJson => "diagnostics-json",
            Self::AmbiguityJson => "ambiguity-json",
        }
    }

    fn is_document_projection(&self) -> bool {
        matches!(
            self,
            Self::DocumentJson | Self::TreeJson | Self::DiagnosticsJson | Self::AmbiguityJson
        )
    }
}

fn main() -> Result<()> {
    env_logger::init();
    let cli = Cli::parse();

    if cli.verbose {
        log::set_max_level(log::LevelFilter::Debug);
    }

    match cli.command {
        Commands::Init { name, output } => init_grammar(&name, output)?,
        Commands::Build { path, watch } => {
            if watch {
                watch_and_build(&path)?;
            } else {
                build_grammar(&path)?;
            }
        }
        Commands::Parse {
            grammar,
            input,
            format,
            dynamic,
            symbol,
        } => parse_file(&grammar, &input, format, dynamic, &symbol)?,
        Commands::Test { path, update } => test_grammar(&path, update)?,
        Commands::Doc { grammar, output } => generate_docs(&grammar, output)?,
        Commands::Check { grammar } => check_grammar(&grammar)?,
        Commands::Stats { grammar } => show_stats(&grammar)?,
        Commands::Version => print_version(),
    }

    Ok(())
}

fn init_grammar(name: &str, output: Option<PathBuf>) -> Result<()> {
    let dir = output.unwrap_or_else(|| PathBuf::from("."));
    let project_dir = dir.join(name);

    println!(
        "{} Creating new grammar project: {}",
        "✨".green(),
        name.bright_blue()
    );

    // Create project structure
    fs::create_dir_all(&project_dir)?;
    fs::create_dir_all(project_dir.join("src"))?;
    fs::create_dir_all(project_dir.join("tests"))?;
    fs::create_dir_all(project_dir.join("examples"))?;

    // Create Cargo.toml
    let dependency_block = scaffold_dependency_block()?;
    let cargo_toml = format!(
        r#"[package]
name = "{}"
version = "0.1.0"
edition = "2024"

{}

[features]
default = ["pure-rust"]
pure-rust = ["adze/pure-rust"]

[dev-dependencies]
insta = "1.40"
"#,
        name, dependency_block
    );

    fs::write(project_dir.join("Cargo.toml"), cargo_toml)?;

    // Create build.rs
    let build_rs = r#"use std::path::PathBuf;

fn main() {
    println!("cargo::rustc-check-cfg=cfg(adze_unsafe_attrs)");
    println!("cargo:rerun-if-changed=src/lib.rs");
    println!("cargo:rerun-if-changed=src/grammar.rs");
    adze_tool::build_parsers(&PathBuf::from("src/lib.rs"));
}
"#;

    fs::write(project_dir.join("build.rs"), build_rs)?;

    // Create example grammar
    let grammar_name = name.replace('-', "_");
    let grammar_rs = format!(
        r#"//! {} grammar definition

#[adze::grammar("{}")]
pub mod grammar {{
    /// Root node of the grammar
    #[adze::language]
    #[derive(Debug, PartialEq, Eq)]
    pub struct Program {{
        #[adze::leaf(pattern = r"\d+", text = true)]
        pub number: String,
    }}

    /// Whitespace ignored between tokens.
    #[adze::extra]
    #[allow(dead_code)]
    struct Whitespace {{
        #[adze::leaf(pattern = r"\s+")]
        _ws: (),
    }}
}}
"#,
        name, grammar_name
    );

    fs::write(project_dir.join("src/grammar.rs"), grammar_rs)?;

    // Create lib.rs
    let lib_rs = r#"#[path = "grammar.rs"]
mod grammar_file;

pub use grammar_file::grammar;
"#;

    fs::write(project_dir.join("src/lib.rs"), lib_rs)?;

    // Create example test
    let crate_name = name.replace('-', "_");
    let test_rs = format!(
        r#"use {}::grammar;

#[test]
fn test_generated_parser_returns_number_ast() {{
    let program = grammar::parse("42").expect("number program should parse");

    assert_eq!(program.number, "42");
}}
"#,
        crate_name
    );

    fs::write(project_dir.join("tests/basic.rs"), test_rs)?;

    // Create README
    let readme = format!(
        r#"# {}

A adze grammar for {}.

## Usage

```rust
let program = {}::grammar::parse("42")?;
```

## Development

Build the grammar:
```bash
cargo build
```

Run tests:
```bash
cargo test
```

## License

MIT
"#,
        name, name, crate_name
    );

    fs::write(project_dir.join("README.md"), readme)?;

    println!(
        "{} Project created at {}",
        "✅".green(),
        project_dir.display().to_string().bright_blue()
    );
    println!("\n{}", "Next steps:".bright_yellow());
    println!("  cd {}", name);
    println!("  cargo build");
    println!("  cargo test");

    Ok(())
}

fn build_grammar(path: &Path) -> Result<()> {
    println!("{} Building grammar...", "🔨".blue());

    if path.is_file() {
        build_parsers(path);
        println!("{} Grammar built successfully!", "✅".green());
    } else {
        // Find all grammar files in directory
        let grammar_files: Vec<_> = WalkDir::new(path)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path().extension().is_some_and(|ext| ext == "rs")
                    && e.path().to_str().is_some_and(|s| s.contains("grammar"))
            })
            .collect();

        if grammar_files.is_empty() {
            anyhow::bail!("No grammar files found in {}", path.display());
        }

        for entry in grammar_files {
            println!("  {} {}", "Building".bright_black(), entry.path().display());
            build_parsers(entry.path());
        }

        println!("{} All grammars built successfully!", "✅".green());
    }

    Ok(())
}

fn watch_and_build(path: &Path) -> Result<()> {
    use notify::{Event, RecursiveMode, Watcher};
    use std::sync::mpsc::channel;
    use std::time::Duration;

    println!("{} Watching for changes...", "👀".blue());

    let (tx, rx) = channel();
    let mut watcher = notify::recommended_watcher(move |res: Result<Event, _>| {
        if let Ok(event) = res {
            let _ = tx.send(event);
        }
    })?;

    watcher.watch(path, RecursiveMode::Recursive)?;

    // Initial build
    build_grammar(path)?;

    loop {
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(event) => {
                if event
                    .paths
                    .iter()
                    .any(|p| p.extension().is_some_and(|ext| ext == "rs"))
                {
                    println!("{} Change detected, rebuilding...", "🔄".yellow());
                    if let Err(e) = build_grammar(path) {
                        eprintln!("{} Build failed: {}", "❌".red(), e);
                    }
                }
            }
            Err(_) => continue,
        }
    }
}

fn parse_file(
    grammar: &Path,
    input: &Path,
    format: OutputFormat,
    dynamic: bool,
    _symbol: &str,
) -> Result<()> {
    if dynamic {
        #[cfg(feature = "dynamic")]
        {
            return parse_file_dynamic(grammar, input, format, _symbol);
        }
        #[cfg(not(feature = "dynamic"))]
        {
            anyhow::bail!(
                "{}",
                "Dynamic parse mode is experimental and requires building adze-cli with --features dynamic."
                    .red()
            );
        }
    }
    println!("{} Parsing file: {}", "📄".blue(), input.display());

    let input_content = fs::read_to_string(input)?;
    println!(
        "  Grammar: {}\n  Input: {} ({} bytes)\n  Output: {}",
        grammar.display(),
        input.display(),
        input_content.len(),
        format.cli_name()
    );
    println!(
        "{} Static parse mode is not yet available in adze-cli.",
        "⚠️ ".yellow()
    );
    if format.is_document_projection() {
        println!(
            "   `{}` is reserved for the ADZE-SPEC-0008 document projection surface.",
            format.cli_name()
        );
        println!(
            "   Until CLI schema output lands, use generated `parse_document()` from Rust code."
        );
    }
    println!(
        "   To parse files from Rust code, use `adze build` + `cargo test` in your grammar project."
    );
    println!("   To validate a grammar without parsing, use `adze check <grammar.rs>`.");
    println!(
        "   To load a compiled grammar at runtime, pass --dynamic (experimental; requires --features dynamic)."
    );

    anyhow::bail!(
        "static parse mode is currently unimplemented — use `adze build` + `cargo test` instead"
    )
}

#[cfg(feature = "dynamic")]
fn parse_file_dynamic(
    grammar: &Path,
    input: &Path,
    format: OutputFormat,
    symbol: &str,
) -> Result<()> {
    use libloading::Library;

    println!(
        "{} Loading dynamic grammar: {}",
        "🔧".blue(),
        grammar.display()
    );
    let input_content = fs::read_to_string(input)?;

    unsafe {
        // Check if file exists
        if !grammar.exists() {
            anyhow::bail!("dynamic grammar not found: {}", grammar.display());
        }

        let lib = Library::new(grammar)?;
        // Build symbol name with null terminator
        let sym_name = {
            let mut s = symbol.as_bytes().to_vec();
            if !s.ends_with(b"\0") {
                s.push(0);
            }
            s
        };
        let get_language: libloading::Symbol<unsafe extern "C" fn() -> *const u8> =
            lib.get(&sym_name)?;
        let _lang_ptr = get_language();

        println!(
            "{} Loaded language symbol from: {}",
            "✓".green(),
            grammar.display()
        );
        println!("Input size: {} bytes", input_content.len());
        println!("Requested output: {}", format.cli_name());
        if format.is_document_projection() {
            println!(
                "{} Document projection output is reserved but not implemented for dynamic parse mode yet.",
                "⚠️ ".yellow()
            );
        }
        println!(
            "{} Dynamic parse mode is experimental: loading works, but AST/output parsing is not implemented yet.",
            "⚠️ ".yellow()
        );
    }

    anyhow::bail!("dynamic parse mode is currently unimplemented")
}

fn scaffold_dependency_block() -> Result<String> {
    let version = env!("CARGO_PKG_VERSION");
    if version.contains("dev") {
        let cli_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let runtime_path = cli_dir.join("../runtime").canonicalize()?;
        let tool_path = cli_dir.join("../tool").canonicalize()?;
        let runtime_path = toml_basic_string_literal(&runtime_path.display().to_string());
        let tool_path = toml_basic_string_literal(&tool_path.display().to_string());
        Ok(format!(
            r#"[dependencies]
adze = {{ path = {} }}

[build-dependencies]
adze-tool = {{ path = {} }}"#,
            runtime_path, tool_path
        ))
    } else {
        Ok(format!(
            r#"[dependencies]
adze = {{ version = "{}" }}

[build-dependencies]
adze-tool = {{ version = "{}" }}"#,
            version, version
        ))
    }
}

fn toml_basic_string_literal(value: &str) -> String {
    use std::fmt::Write as _;

    let mut escaped = String::with_capacity(value.len() + 2);
    escaped.push('"');

    for ch in value.chars() {
        match ch {
            '\u{08}' => escaped.push_str("\\b"),
            '\t' => escaped.push_str("\\t"),
            '\n' => escaped.push_str("\\n"),
            '\u{0C}' => escaped.push_str("\\f"),
            '\r' => escaped.push_str("\\r"),
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\u{00}'..='\u{1F}' | '\u{7F}' => {
                write!(escaped, "\\u{:04X}", ch as u32).expect("write to string");
            }
            _ => escaped.push(ch),
        }
    }

    escaped.push('"');
    escaped
}

fn test_grammar(_path: &Path, update: bool) -> Result<()> {
    println!("{} Testing grammar...", "🧪".blue());

    if update {
        println!("  {} Updating snapshots", "📸".yellow());
    }

    // Run cargo test
    let mut cmd = std::process::Command::new("cargo");
    cmd.arg("test");
    if update {
        cmd.env("INSTA_UPDATE", "always");
    }

    let status = cmd.status()?;

    if status.success() {
        println!("{} All tests passed!", "✅".green());
    } else {
        anyhow::bail!("Tests failed");
    }

    Ok(())
}

fn generate_docs(grammar: &Path, output: Option<PathBuf>) -> Result<()> {
    println!("{} Generating documentation...", "📚".blue());

    let content = fs::read_to_string(grammar)?;

    // Simple doc generation - extract doc comments
    let mut docs = String::from("# Grammar Documentation\n\n");

    for line in content.lines() {
        if line.trim().starts_with("///") {
            docs.push_str(line.trim_start_matches("///").trim());
            docs.push('\n');
        }
    }

    if let Some(output) = output {
        fs::write(output, docs)?;
        println!("{} Documentation written to file", "✅".green());
    } else {
        println!("{}", docs);
    }

    Ok(())
}

fn check_grammar(grammar: &Path) -> Result<()> {
    println!("{} Checking grammar syntax...", "🔍".blue());

    let results = analyze_grammar_file(grammar, false)?;
    println!(
        "{} Grammar syntax is valid ({})!",
        "✅".green(),
        if results.len() == 1 {
            "1 grammar definition".to_string()
        } else {
            format!("{} grammar definitions", results.len())
        }
    );

    Ok(())
}

fn show_stats(grammar: &Path) -> Result<()> {
    let results = analyze_grammar_file(grammar, false)?;
    println!("{} Grammar statistics:", "📊".blue());

    for result in results {
        print_stats_summary(&result);
    }

    Ok(())
}

fn print_stats_summary(result: &BuildResult) {
    println!(
        "  {} {}",
        "Grammar:".bright_black(),
        result.grammar_name.bright_green()
    );
    println!(
        "    {} {}",
        "States:".bright_black(),
        result.build_stats.state_count
    );
    println!(
        "    {} {}",
        "Symbols:".bright_black(),
        result.build_stats.symbol_count
    );
    println!(
        "    {} {}",
        "Conflicts:".bright_black(),
        result.build_stats.conflict_cells
    );
}

fn print_version() {
    println!("adze {}", env!("CARGO_PKG_VERSION"));
}

#[cfg(test)]
mod tests {
    use super::toml_basic_string_literal;

    #[test]
    fn toml_basic_string_literal_escapes_windows_paths() {
        let literal = toml_basic_string_literal(r"\\?\D:\repo\adze\runtime");

        assert_eq!(literal, r#""\\\\?\\D:\\repo\\adze\\runtime""#);
    }
}
