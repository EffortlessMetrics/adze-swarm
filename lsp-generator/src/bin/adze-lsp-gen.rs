// Prototype CLI tool for experimenting with LSP server generation from Adze grammars.

use adze_lsp_generator::{LspBuilder, LspConfig};
use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "adze-lsp-gen")]
#[command(
    about = "Prototype LSP server generator for Adze grammars",
    long_about = None
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Generate a prototype LSP server
    Generate {
        /// Name of the language server
        #[arg(short, long)]
        name: String,

        /// Path to the grammar file
        #[arg(short, long)]
        grammar: PathBuf,

        /// Output directory
        #[arg(short, long, default_value = "./lsp-output")]
        output: PathBuf,

        /// Version of the server
        #[arg(short, long, default_value = "0.1.0")]
        version: String,

        /// Enable completion support
        #[arg(long)]
        completion: bool,

        /// Enable hover support
        #[arg(long)]
        hover: bool,

        /// Enable diagnostics support
        #[arg(long)]
        diagnostics: bool,

        /// Enable all features
        #[arg(long)]
        all_features: bool,
    },

    /// Create a prototype LSP server from a config file
    FromConfig {
        /// Path to config file
        #[arg(short, long)]
        config: PathBuf,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Generate {
            name,
            grammar,
            output,
            version,
            completion,
            hover,
            diagnostics,
            all_features,
        } => {
            println!("Generating prototype LSP server for {name}...");

            let mut builder = LspBuilder::new(name)
                .version(version)
                .grammar_path(grammar)
                .output_dir(output);

            if all_features {
                builder = builder.feature("all");
            } else {
                if completion {
                    builder = builder.feature("completion");
                }
                if hover {
                    builder = builder.feature("hover");
                }
                if diagnostics {
                    builder = builder.feature("diagnostics");
                }
            }

            builder.build()?;

            println!("Prototype LSP server generated successfully.");
            println!("To build and run:");
            println!("   cd <output-dir>");
            println!("   cargo build --release");
            println!("   ./target/release/<name>-lsp");
        }

        Commands::FromConfig { config } => {
            println!("Loading config from: {}", config.display());

            // Load and parse config file
            let config_str = std::fs::read_to_string(&config)?;
            let lsp_config: LspConfig = serde_json::from_str(&config_str)?;

            println!("Generating prototype LSP server: {}", lsp_config.name);

            // Create builder from config
            let builder = LspBuilder::new(lsp_config.name.clone())
                .version(lsp_config.version)
                .grammar_path("path/to/grammar") // Would be in config
                .output_dir("./lsp-output")
                .feature("all");

            builder.build()?;

            println!("Prototype LSP server generated successfully.");
        }
    }

    Ok(())
}
