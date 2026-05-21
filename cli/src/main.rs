use adze_tool::{build_parsers, pure_rust_builder::BuildResult};
use anyhow::Result;
use clap::{Parser, Subcommand};
use colored::Colorize;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
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
    /// Arithmetic expression grammar.
    #[adze::language]
    #[derive(Debug, PartialEq)]
    pub enum Expr {{
        Number(
            #[adze::leaf(pattern = r"\d+", transform = |v| v.parse().unwrap())]
            i32,
        ),

        #[adze::prec_left(1)]
        Add(Box<Expr>, #[adze::leaf(text = "+")] (), Box<Expr>),

        #[adze::prec_left(2)]
        Mul(Box<Expr>, #[adze::leaf(text = "*")] (), Box<Expr>),
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
        r#"use {}::grammar::{{self, Expr}};

#[test]
fn test_generated_parser_respects_precedence() {{
    let expr = grammar::parse("1 + 2 * 3").expect("expression should parse");

    assert_eq!(
        expr,
        Expr::Add(
            Box::new(Expr::Number(1)),
            (),
            Box::new(Expr::Mul(
                Box::new(Expr::Number(2)),
                (),
                Box::new(Expr::Number(3)),
            )),
        )
    );
}}

#[test]
fn test_bad_input_reports_diagnostics() {{
    let source = "1 + @";
    let errors = grammar::parse(source).expect_err("bad input should fail");
    let first = errors
        .first()
        .expect("bad input should produce at least one parse error");

    assert_eq!(first.byte_span(), 4..5);
    assert!(
        first.expected.iter().any(|name| name == r"/\d+/"),
        "diagnostic should name the expected number token, got {{:?}}",
        first.expected
    );

    let rendered = first.display_with_source(source).to_string();
    assert!(rendered.contains("bytes 4..5"));
    assert!(rendered.contains("expected one of:"));
}}

#[test]
fn test_parse_document_exposes_recovered_document() {{
    let document = grammar::parse_document("1 +")
        .expect("parse_document should return partial document facts for recoverable input");

    assert!(document.tree().has_errors());
    assert!(!document.diagnostics().is_empty());
}}
"#,
        crate_name
    );

    fs::write(project_dir.join("tests/parse.rs"), test_rs)?;

    let example_rs = format!(
        r#"use {}::grammar;

fn main() {{
    let source = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "1 + 2 * 3".to_string());

    match grammar::parse(&source) {{
        Ok(expr) => println!("{{expr:?}}"),
        Err(errors) => {{
            for error in &errors {{
                eprintln!("{{}}", error.display_with_source(&source));
            }}
            std::process::exit(1);
        }}
    }}
}}
"#,
        crate_name
    );

    fs::write(project_dir.join("examples/parse.rs"), example_rs)?;

    // Create README
    let readme = format!(
        r#"# {}

An Adze arithmetic grammar for {}. This starter project keeps the default
user path small:

```text
Rust grammar types -> generated parser -> grammar::parse(...) -> typed Expr
```

## First Run

```bash
cargo test
cargo run --example parse -- "1 + 2 * 3"
```

## Which API Should I Use?

Use the typed parser when your application wants Rust values:

```rust
let expr = {}::grammar::parse("1 + 2 * 3")?;
```

Use the document parser when tooling needs diagnostics, ranges, syntax facts,
or recoverable parse data:

```rust
let document = {}::grammar::parse_document("1 +")?;
for diagnostic in document.diagnostics() {{
    eprintln!("{{diagnostic}}");
}}
```

The generated tests cover both paths:

```bash
cargo test
```

The generated example prints the typed parse result and renders parse errors
with source excerpts:

```bash
cargo run --example parse -- "1 + 2 * 3"
cargo run --example parse -- "1 + @"
```

## Project Layout

```text
build.rs          build-time parser generation
src/grammar.rs   annotated Rust grammar types
src/lib.rs       public generated parser module export
tests/parse.rs   typed parser and document diagnostics checks
examples/parse.rs runnable parse example
```

## License

MIT
"#,
        name, name, crate_name, crate_name
    );

    fs::write(project_dir.join("README.md"), readme)?;

    println!(
        "{} Project created at {}",
        "✅".green(),
        project_dir.display().to_string().bright_blue()
    );
    println!("\n{}", "Next steps:".bright_yellow());
    println!("  cd {}", name);
    println!("  cargo test");
    println!("  cargo run --example parse -- \"1 + 2 * 3\"");

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

    if matches!(format, OutputFormat::Tree) {
        return parse_file_static_tree(grammar, input);
    }

    if matches!(format, OutputFormat::Sexp) {
        return parse_file_static_sexp(grammar, input);
    }

    if format.is_document_projection() {
        return parse_file_static_document_projection(grammar, input, format);
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
        "{} Static parse output format `{}` is not yet available in adze-cli.",
        "⚠️ ".yellow(),
        format.cli_name()
    );
    println!(
        "   To parse files from Rust code, use `adze build` + `cargo test` in your grammar project."
    );
    println!("   To validate a grammar without parsing, use `adze check <grammar.rs>`.");
    println!(
        "   To load a compiled grammar at runtime, pass --dynamic (experimental; requires --features dynamic)."
    );

    anyhow::bail!(
        "static parse output format `{}` is currently unimplemented — use `tree` or a document projection mode instead",
        format.cli_name()
    )
}

fn parse_file_static_tree(grammar: &Path, input: &Path) -> Result<()> {
    let output = run_static_parse_runner(grammar, input, "document-json")?;
    let document: serde_json::Value = serde_json::from_str(&output).map_err(|err| {
        anyhow::anyhow!(
            "static tree output could not read generated document JSON for {}: {}",
            grammar.display(),
            err
        )
    })?;

    let root = &document["tree"]["root"];
    if root.is_null() {
        anyhow::bail!(
            "static tree output could not find document tree root for {}",
            grammar.display()
        );
    }

    let mut rendered = String::new();
    render_selected_tree(root, 0, None, &mut rendered);
    print!("{rendered}");
    Ok(())
}

fn parse_file_static_sexp(grammar: &Path, input: &Path) -> Result<()> {
    let output = run_static_parse_runner(grammar, input, "document-json")?;
    let document: serde_json::Value = serde_json::from_str(&output).map_err(|err| {
        anyhow::anyhow!(
            "static sexp output could not read generated document JSON for {}: {}",
            grammar.display(),
            err
        )
    })?;

    let root = &document["tree"]["root"];
    if root.is_null() {
        anyhow::bail!(
            "static sexp output could not find document tree root for {}",
            grammar.display()
        );
    }

    let mut rendered = String::new();
    render_selected_sexp(root, None, &mut rendered);
    rendered.push('\n');
    print!("{rendered}");
    Ok(())
}

fn parse_file_static_document_projection(
    grammar: &Path,
    input: &Path,
    format: OutputFormat,
) -> Result<()> {
    let output = run_static_parse_runner(grammar, input, format.cli_name())?;
    print!("{output}");
    Ok(())
}

fn run_static_parse_runner(grammar: &Path, input: &Path, output_format: &str) -> Result<String> {
    let grammar = absolute_path(grammar)?;
    let input = absolute_path(input)?;
    if !input.is_file() {
        anyhow::bail!("Input file does not exist: {}", input.display());
    }

    let module = single_top_level_grammar_module(&grammar)?;
    let runner = tempfile::tempdir()?;
    let src_dir = runner.path().join("src");
    fs::create_dir_all(&src_dir)?;
    fs::write(runner.path().join("Cargo.toml"), parse_runner_cargo_toml()?)?;
    fs::write(
        runner.path().join("build.rs"),
        parse_runner_build_rs(&grammar),
    )?;
    fs::write(
        src_dir.join("main.rs"),
        parse_runner_main_rs(&grammar, &module),
    )?;

    let output = Command::new("cargo")
        .arg("run")
        .arg("--quiet")
        .arg("--manifest-path")
        .arg(runner.path().join("Cargo.toml"))
        .arg("--")
        .arg(&input)
        .arg(output_format)
        .env("ADZE_USE_PURE_RUST", "1")
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        anyhow::bail!(
            "static document projection failed for {} with status {}\nstdout:\n{}\nstderr:\n{}",
            grammar.display(),
            output.status,
            stdout.trim_end(),
            stderr.trim_end()
        );
    }

    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn render_selected_tree(
    node: &serde_json::Value,
    depth: usize,
    field_name: Option<&str>,
    out: &mut String,
) {
    let indent = "  ".repeat(depth);
    let kind = node["kind"].as_str().unwrap_or("<unknown>");
    let start = node["range"]["start_byte"].as_u64().unwrap_or(0);
    let end = node["range"]["end_byte"].as_u64().unwrap_or(start);

    out.push_str(&indent);
    if let Some(field_name) = field_name {
        out.push_str(field_name);
        out.push_str(": ");
    }
    out.push_str(kind);
    out.push_str(" [");
    out.push_str(&start.to_string());
    out.push_str("..");
    out.push_str(&end.to_string());
    out.push(']');

    if node["flags"]["has_error"].as_bool().unwrap_or(false) {
        out.push_str(" has_error");
    }
    if node["flags"]["missing"].as_bool().unwrap_or(false) {
        out.push_str(" missing");
    }
    if node["flags"]["error"].as_bool().unwrap_or(false) {
        out.push_str(" error");
    }
    if let Some(text) = node["text"].as_str() {
        out.push(' ');
        out.push_str(&format!("{text:?}"));
    }
    out.push('\n');

    if let Some(children) = node["children"].as_array() {
        for edge in children {
            let field_name = edge["field_name"].as_str();
            render_selected_tree(&edge["node"], depth + 1, field_name, out);
        }
    }
}

fn render_selected_sexp(node: &serde_json::Value, field_name: Option<&str>, out: &mut String) {
    if let Some(field_name) = field_name {
        write_sexp_atom(field_name, out);
        out.push_str(": ");
    }

    let kind = node["kind"].as_str().unwrap_or("<unknown>");
    let children = node["children"].as_array();

    if children.is_none_or(|children| children.is_empty()) {
        write_sexp_atom(kind, out);
        return;
    }

    out.push('(');
    write_sexp_atom(kind, out);
    if let Some(children) = children {
        for edge in children {
            out.push(' ');
            render_selected_sexp(&edge["node"], edge["field_name"].as_str(), out);
        }
    }
    out.push(')');
}

fn write_sexp_atom(atom: &str, out: &mut String) {
    if is_plain_sexp_atom(atom) {
        out.push_str(atom);
        return;
    }

    out.push('"');
    for ch in atom.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ => out.push(ch),
        }
    }
    out.push('"');
}

fn is_plain_sexp_atom(atom: &str) -> bool {
    !atom.is_empty()
        && atom.chars().all(|ch| {
            ch.is_ascii_alphanumeric()
                || matches!(
                    ch,
                    '_' | '-'
                        | '+'
                        | '*'
                        | '/'
                        | '\\'
                        | '.'
                        | '?'
                        | '!'
                        | '@'
                        | '#'
                        | '$'
                        | '%'
                        | '^'
                        | '&'
                        | '='
                        | '<'
                        | '>'
                        | ':'
                )
        })
}

fn absolute_path(path: &Path) -> Result<PathBuf> {
    if path.is_absolute() {
        Ok(path.to_owned())
    } else {
        Ok(std::env::current_dir()?.join(path))
    }
}

fn single_top_level_grammar_module(grammar: &Path) -> Result<String> {
    let content = fs::read_to_string(grammar)
        .map_err(|e| anyhow::anyhow!("Could not read grammar file {}: {}", grammar.display(), e))?;
    let file = syn::parse_file(&content)
        .map_err(|e| anyhow::anyhow!("Grammar syntax is invalid: {}: {}", grammar.display(), e))?;

    let modules = file
        .items
        .iter()
        .filter_map(|item| match item {
            syn::Item::Mod(module) if has_adze_grammar_attr(&module.attrs) => {
                Some(module.ident.to_string())
            }
            _ => None,
        })
        .collect::<Vec<_>>();

    match modules.as_slice() {
        [module] => Ok(module.clone()),
        [] => anyhow::bail!(
            "No top-level adze grammar module found in {}",
            grammar.display()
        ),
        _ => anyhow::bail!(
            "Static CLI parse supports exactly one top-level grammar module; found {} in {}",
            modules.len(),
            grammar.display()
        ),
    }
}

fn has_adze_grammar_attr(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|attr| {
        attr.path()
            .segments
            .last()
            .is_some_and(|segment| segment.ident == "grammar")
    })
}

fn parse_runner_cargo_toml() -> Result<String> {
    let version = env!("CARGO_PKG_VERSION");
    let adze_dependency = if let Some((runtime_path, _)) = local_workspace_dependency_paths() {
        format!(
            "{{ path = {}, features = [\"serialization\", \"glr\"] }}",
            toml_basic_string_literal(&runtime_path.display().to_string())
        )
    } else {
        format!("{{ version = \"{version}\", features = [\"serialization\", \"glr\"] }}")
    };
    let tool_dependency = if let Some((_, tool_path)) = local_workspace_dependency_paths() {
        format!(
            "{{ path = {} }}",
            toml_basic_string_literal(&tool_path.display().to_string())
        )
    } else {
        format!("{{ version = \"{version}\" }}")
    };

    Ok(format!(
        r#"[package]
name = "adze-cli-parse-runner"
version = "0.0.0"
edition = "2024"
publish = false

[dependencies]
adze = {adze_dependency}
serde_json = "1"

[build-dependencies]
adze-tool = {tool_dependency}
"#
    ))
}

fn parse_runner_build_rs(grammar: &Path) -> String {
    let grammar = rust_string_literal(&grammar.display().to_string());
    format!(
        r#"fn main() {{
    let grammar = std::path::PathBuf::from({grammar});
    println!("cargo::rustc-check-cfg=cfg(adze_unsafe_attrs)");
    println!("cargo:rerun-if-changed={{}}", grammar.display());
    adze_tool::build_parsers(&grammar);
}}
"#
    )
}

fn parse_runner_main_rs(grammar: &Path, module: &str) -> String {
    let grammar = rust_string_literal(&grammar.display().to_string());
    format!(
        r#"#[path = {grammar}]
mod grammar_input;

fn main() -> Result<(), Box<dyn std::error::Error>> {{
    let mut args = std::env::args_os().skip(1);
    let input = args.next().ok_or("missing input path")?;
    let output = args.next().ok_or("missing output format")?;
    let source = std::fs::read_to_string(input)?;

    let document = grammar_input::{module}::parse_document(&source)
        .map_err(|errors| format!("parse_document failed: {{errors:?}}"))?;
    let document_json = document.to_json_value();
    let output = match output.to_str().ok_or("output format must be UTF-8")? {{
        "document-json" => document_json,
        "tree-json" => serde_json::json!({{
            "schema": "adze.tree.v1",
            "document_schema": document_json["schema"].clone(),
            "language": document_json["language"].clone(),
            "tree": document_json["tree"].clone(),
        }}),
        "diagnostics-json" => serde_json::json!({{
            "schema": "adze.diagnostics.v1",
            "document_schema": document_json["schema"].clone(),
            "language": document_json["language"].clone(),
            "diagnostics": document_json["diagnostics"].clone(),
        }}),
        "ambiguity-json" => serde_json::json!({{
            "schema": "adze.ambiguity.v1",
            "document_schema": document_json["schema"].clone(),
            "language": document_json["language"].clone(),
            "ambiguities": document_json["ambiguities"].clone(),
        }}),
        other => return Err(format!("unsupported output format: {{other}}").into()),
    }};

    println!("{{}}", serde_json::to_string_pretty(&output)?);
    Ok(())
}}
"#
    )
}

fn rust_string_literal(value: &str) -> String {
    format!("{value:?}")
}

#[cfg(feature = "dynamic")]
fn parse_file_dynamic(
    grammar: &Path,
    input: &Path,
    format: OutputFormat,
    symbol: &str,
) -> Result<()> {
    use libloading::Library;

    log_dynamic_grammar_load(grammar);
    let input_content = read_dynamic_input(input)?;

    unsafe {
        ensure_dynamic_grammar_exists(grammar)?;

        let lib = Library::new(grammar)?;
        let sym_name = symbol_name_with_null_terminator(symbol);
        let get_language: libloading::Symbol<unsafe extern "C" fn() -> *const u8> =
            lib.get(&sym_name)?;
        let _lang_ptr = get_language();

        log_dynamic_parse_mode_status(grammar, input_content.len(), format);
    }

    anyhow::bail!("dynamic parse mode is currently unimplemented")
}

#[cfg(feature = "dynamic")]
fn log_dynamic_grammar_load(grammar: &Path) {
    println!(
        "{} Loading dynamic grammar: {}",
        "🔧".blue(),
        grammar.display()
    );
}

#[cfg(feature = "dynamic")]
fn read_dynamic_input(input: &Path) -> Result<String> {
    Ok(fs::read_to_string(input)?)
}

#[cfg(feature = "dynamic")]
fn ensure_dynamic_grammar_exists(grammar: &Path) -> Result<()> {
    if !grammar.exists() {
        anyhow::bail!("dynamic grammar not found: {}", grammar.display());
    }
    Ok(())
}

#[cfg(feature = "dynamic")]
fn symbol_name_with_null_terminator(symbol: &str) -> Vec<u8> {
    let mut bytes = symbol.as_bytes().to_vec();
    if !bytes.ends_with(b"\0") {
        bytes.push(0);
    }
    bytes
}

#[cfg(feature = "dynamic")]
fn log_dynamic_parse_mode_status(grammar: &Path, input_len: usize, format: OutputFormat) {
    println!(
        "{} Loaded language symbol from: {}",
        "✓".green(),
        grammar.display()
    );
    println!("Input size: {input_len} bytes");
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

fn scaffold_dependency_block() -> Result<String> {
    if let Some((runtime_path, tool_path)) = local_workspace_dependency_paths() {
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
        let version = env!("CARGO_PKG_VERSION");
        Ok(format!(
            r#"[dependencies]
adze = {{ version = "{}" }}

[build-dependencies]
adze-tool = {{ version = "{}" }}"#,
            version, version
        ))
    }
}

fn local_workspace_dependency_paths() -> Option<(PathBuf, PathBuf)> {
    let cli_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let runtime_path = cli_dir.join("../runtime").canonicalize().ok()?;
    let tool_path = cli_dir.join("../tool").canonicalize().ok()?;

    if runtime_path.join("Cargo.toml").is_file() && tool_path.join("Cargo.toml").is_file() {
        Some((runtime_path, tool_path))
    } else {
        None
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
