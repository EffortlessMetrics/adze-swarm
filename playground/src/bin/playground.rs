// CLI entry point for the experimental Adze playground.

use adze_playground::{PlaygroundBuilder, PlaygroundFeature};
use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "adze-playground")]
#[command(
    about = "Experimental grammar playground for Adze",
    long_about = None
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Launch interactive CLI playground
    Cli {
        /// Path to grammar file
        #[arg(short, long)]
        grammar: PathBuf,

        /// Path to test file
        #[arg(short, long)]
        tests: Option<PathBuf>,
    },

    /// Launch web playground server
    Web {
        /// Path to grammar file
        #[arg(short, long)]
        grammar: PathBuf,

        /// Server port
        #[arg(short, long, default_value = "8080")]
        port: u16,

        /// Path to test file
        #[arg(short, long)]
        tests: Option<PathBuf>,
    },

    /// Run tests without interactive mode
    Test {
        /// Path to grammar file
        #[arg(short, long)]
        grammar: PathBuf,

        /// Path to test file
        #[arg(short, long)]
        tests: PathBuf,

        /// Output format (text, json)
        #[arg(short, long, default_value = "text")]
        format: String,
    },

    /// Analyze grammar
    Analyze {
        /// Path to grammar file
        #[arg(short, long)]
        grammar: PathBuf,

        /// Output format (text, json)
        #[arg(short, long, default_value = "text")]
        format: String,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Cli { grammar, tests } => {
            println!("Launching prototype CLI playground...");

            let mut builder = PlaygroundBuilder::new()
                .grammar(grammar.to_string_lossy())
                .feature(PlaygroundFeature::CliInterface);

            if let Some(test_file) = tests {
                builder = builder.tests(test_file.to_string_lossy());
            }

            builder.build()?;
        }

        Commands::Web {
            grammar,
            port,
            tests,
        } => {
            println!("Launching prototype web playground on port {port}...");

            let mut builder = PlaygroundBuilder::new()
                .grammar(grammar.to_string_lossy())
                .feature(PlaygroundFeature::WebInterface(port));

            if let Some(test_file) = tests {
                builder = builder.tests(test_file.to_string_lossy());
            }

            builder.build()?;
        }

        Commands::Test {
            grammar,
            tests,
            format,
        } => {
            println!("Running playground test prototype...");

            PlaygroundBuilder::new()
                .grammar(grammar.to_string_lossy())
                .tests(tests.to_string_lossy())
                .feature(PlaygroundFeature::TestRunner)
                .build()?;

            // Format output based on requested format
            match format.as_str() {
                "json" => {
                    // JSON output would be implemented here
                }
                _ => {
                    // Text output is default
                }
            }
        }

        Commands::Analyze { grammar, format } => {
            println!("Analyzing grammar with playground prototype...");

            PlaygroundBuilder::new()
                .grammar(grammar.to_string_lossy())
                .feature(PlaygroundFeature::Analysis)
                .build()?;

            // Format output based on requested format
            match format.as_str() {
                "json" => {
                    // JSON output would be implemented here
                }
                _ => {
                    // Text output is default
                }
            }
        }
    }

    Ok(())
}
