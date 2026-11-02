//! Command-line interface for the adze build tool.

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::{Path, PathBuf};

mod parse;
mod test;

/// Tree-sitter compatible CLI for adze
#[derive(Parser)]
#[command(name = "adze")]
#[command(about = "A pure-Rust implementation of Tree-sitter", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Generate a parser from a grammar.js file
    Generate {
        /// Path to grammar.js file
        #[arg(short, long, default_value = "grammar.js")]
        grammar: PathBuf,

        /// Output directory for generated files
        #[arg(short, long, default_value = "src")]
        output: PathBuf,

        /// Emit debug artifacts
        #[arg(long)]
        debug: bool,

        /// Use pure-rust implementation
        #[arg(long, default_value_t = true)]
        pure_rust: bool,
    },

    /// Parse a file and output the syntax tree
    Parse {
        /// Path to file to parse
        file: PathBuf,

        /// Path to parser crate (MVP) or compiled library (future)
        /// Example: --parser ./my-parser-crate
        #[arg(short, long)]
        parser: Option<PathBuf>,

        /// Output format
        #[arg(short, long, value_enum, default_value = "tree")]
        format: OutputFormat,

        /// Show field names
        #[arg(long)]
        fields: bool,

        /// Show statistics
        #[arg(short, long)]
        stats: bool,
    },

    /// Test a parser against a corpus
    Test {
        /// Path to test directory
        #[arg(default_value = "test")]
        path: PathBuf,

        /// Filter tests by name
        #[arg(short, long)]
        filter: Option<String>,

        /// Update test expectations
        #[arg(short, long)]
        update: bool,
    },

    /// Retired: use the canonical `adze init` command instead
    Init {
        /// Name of the language
        name: String,

        /// Create in current directory
        #[arg(long)]
        in_place: bool,
    },

    /// Show information about a grammar
    Info {
        /// Path to grammar.js or compiled parser
        #[arg(default_value = "grammar.js")]
        path: PathBuf,

        /// Show node types
        #[arg(long)]
        node_types: bool,

        /// Show grammar rules
        #[arg(long)]
        rules: bool,
    },
}

/// Output format for parse result display.
#[derive(clap::ValueEnum, Clone, Debug)]
pub enum OutputFormat {
    Tree,
    Sexp,
    Json,
    Dot,
}

pub fn run_generate(
    grammar: &PathBuf,
    output: &PathBuf,
    debug: bool,
    pure_rust: bool,
) -> Result<()> {
    use crate::pure_rust_builder::{BuildOptions, build_parser_from_grammar_js};

    println!("Generating parser from {:?}...", grammar);

    if !pure_rust {
        anyhow::bail!(
            "Only pure-rust implementation is currently supported. Remove --no-pure-rust flag."
        );
    }

    let Some(out_dir) = output.to_str() else {
        anyhow::bail!("--output path is not valid UTF-8: {}", output.display());
    };

    let options = BuildOptions {
        out_dir: out_dir.to_string(),
        emit_artifacts: debug,
        compress_tables: !debug,
    };

    let result = build_parser_from_grammar_js(grammar, options)?;

    println!("✓ Generated parser for '{}'", result.grammar_name);
    println!("  Parser: {}", result.parser_path);
    if debug {
        println!("  Debug artifacts emitted to: {:?}", output);
    }

    Ok(())
}

pub fn run_parse(
    file: &Path,
    parser: &Option<PathBuf>,
    format: &OutputFormat,
    fields: bool,
    stats: bool,
) -> Result<()> {
    use parse::OutputFormat as ParseOutputFormat;

    let parse_format = match format {
        OutputFormat::Tree | OutputFormat::Sexp => ParseOutputFormat::Sexp,
        OutputFormat::Json => ParseOutputFormat::Json,
        OutputFormat::Dot => ParseOutputFormat::Dot,
    };

    parse::parse_file(file, parser.as_deref(), parse_format, fields, stats)
}

pub fn run_test(path: &Path, filter: &Option<String>, update: bool) -> Result<()> {
    test::run_tests(Some(path), None, filter.as_deref(), update, true)
}

/// Retired initializer (#1080).
///
/// The former implementation unconditionally overwrote `grammar.js`,
/// `Cargo.toml`, `build.rs`, and `src/lib.rs`, interpolated the name into a
/// filesystem path, and emitted a stale starter. It is disabled before any
/// filesystem effect; the canonical initializer is `adze init`.
pub fn run_init(_name: &str, _in_place: bool) -> Result<()> {
    anyhow::bail!(
        "adze-gen init is retired and no files were created or modified; \
         use the canonical `adze init` command from the adze CLI instead"
    )
}

pub fn run_info(path: &PathBuf, node_types: bool, rules: bool) -> Result<()> {
    use crate::grammar_js::parse_grammar_js_v2;
    use std::fs;

    if path.extension().is_some_and(|ext| ext == "js") {
        // Parse grammar.js
        let content = fs::read_to_string(path)?;
        let grammar = parse_grammar_js_v2(&content)?;

        println!("Grammar: {}", grammar.name);
        println!("Rules: {}", grammar.rules.len());

        if rules {
            println!("\nRules:");
            for name in grammar.rules.keys() {
                println!("  - {}", name);
            }
        }

        if node_types {
            println!("\nNode types information requires a compiled parser.");
        }
    } else {
        anyhow::bail!("Info command for compiled parsers not yet implemented.");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(any(unix, windows))]
    fn non_utf8_path(parent: &Path) -> PathBuf {
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStringExt;
            parent.join(std::ffi::OsString::from_vec(vec![
                0xff, 0xfe, b'o', b'u', b't',
            ]))
        }
        #[cfg(windows)]
        {
            use std::os::windows::ffi::OsStringExt;
            parent.join(std::ffi::OsString::from_wide(&[0xD800]))
        }
    }

    #[test]
    #[cfg(any(unix, windows))]
    fn test_run_generate_non_utf8_output_returns_exact_path_error_without_writing() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let output = non_utf8_path(directory.path());
        assert!(output.to_str().is_none());
        let grammar = directory.path().join("missing grammar.js");
        let result = std::panic::catch_unwind(|| run_generate(&grammar, &output, false, true));
        assert!(
            matches!(&result, Ok(Err(_))),
            "must return a structured error without unwinding: {result:?}"
        );
        if let Ok(Err(error)) = result {
            assert_eq!(
                error.to_string(),
                format!("--output path is not valid UTF-8: {}", output.display())
            );
        }
        assert!(!output.exists());
        assert_eq!(directory.path().read_dir()?.count(), 0);
        Ok(())
    }

    #[test]
    #[cfg(any(unix, windows))]
    fn test_run_generate_disabled_pure_rust_precedes_output_validation() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let output = non_utf8_path(directory.path());
        assert!(output.to_str().is_none());
        let result = run_generate(
            &directory.path().join("missing grammar.js"),
            &output,
            false,
            false,
        );
        assert!(result.is_err());
        if let Err(error) = result {
            assert_eq!(
                error.to_string(),
                "Only pure-rust implementation is currently supported. Remove --no-pure-rust flag."
            );
        }
        assert_eq!(directory.path().read_dir()?.count(), 0);
        Ok(())
    }

    // Creating an invalid-byte filename requires filesystem support. This
    // positive fixture is qualified on Linux; the rejection controls above
    // only construct paths and remain available on Unix and Windows.
    #[test]
    #[cfg(target_os = "linux")]
    fn test_run_generate_native_non_utf8_input_with_utf8_output_succeeds() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let grammar = non_utf8_path(directory.path());
        assert!(grammar.to_str().is_none());
        std::fs::write(
            &grammar,
            "module.exports = grammar({ name: 'cli_native_path', rules: { source_file: $ => 'x' } });",
        )?;
        let output = directory.path().join("generated é with spaces");
        assert!(output.to_str().is_some());
        run_generate(&grammar, &output, false, true)?;
        let parser = output.join("grammar_cli_native_path/parser_cli_native_path.rs");
        assert!(
            parser.is_file(),
            "expected generated parser at {}",
            parser.display()
        );
        let source = std::fs::read_to_string(parser)?;
        assert!(source.contains("static LANGUAGE"));
        assert!(source.contains("cli_native_path"));
        Ok(())
    }
}
