// CLI interface for the adze playground

use crate::{PlaygroundSession, TestCase};
use anyhow::Result;
use colored::*;
use std::io::{self, Write};

/// Run interactive CLI session
pub fn run_interactive(mut session: PlaygroundSession) -> Result<()> {
    println!("{}", "🎮 Adze Grammar Playground".bright_green().bold());
    println!("{}", "Type 'help' for commands, 'quit' to exit".dimmed());
    println!();

    loop {
        print!("{} ", ">".bright_blue());
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim();

        match input {
            "quit" | "exit" => break,
            "help" => print_help(),
            cmd if cmd.starts_with("parse ") => {
                let code = &cmd[6..];
                handle_parse(&session, code)?;
            }
            cmd if cmd.starts_with("test ") => {
                let parts: Vec<&str> = cmd[5..].splitn(2, ' ').collect();
                if parts.len() == 2 {
                    handle_test(&mut session, parts[0], parts[1])?;
                } else {
                    println!("{}", "Usage: test <name> <input>".red());
                }
            }
            "run" => handle_run(&session)?,
            "analyze" => handle_analyze(&mut session)?,
            cmd if cmd.starts_with("load ") => {
                let path = &cmd[5..];
                handle_load(&mut session, path)?;
            }
            cmd if cmd.starts_with("save ") => {
                let path = &cmd[5..];
                handle_save(&session, path)?;
            }
            "stats" => handle_stats(&mut session)?,
            "" => continue,
            _ => println!("{} {}", "Unknown command:".red(), input),
        }
    }

    println!("{}", "Goodbye!".green());
    Ok(())
}

fn print_help() {
    println!("{}", "Available commands:".bright_yellow());
    println!(
        "  {}  <code>         - Parse code and show tree",
        "parse".bright_cyan()
    );
    println!(
        "  {}   <name> <code>  - Add test case",
        "test".bright_cyan()
    );
    println!("  {}                  - Run all tests", "run".bright_cyan());
    println!(
        "  {}               - Analyze grammar",
        "analyze".bright_cyan()
    );
    println!(
        "  {}   <file>        - Load test cases",
        "load".bright_cyan()
    );
    println!("  {}   <file>        - Save session", "save".bright_cyan());
    println!(
        "  {}                - Show grammar statistics",
        "stats".bright_cyan()
    );
    println!(
        "  {}                 - Show this help",
        "help".bright_cyan()
    );
    println!(
        "  {}                 - Exit playground",
        "quit".bright_cyan()
    );
}

fn handle_parse(session: &PlaygroundSession, code: &str) -> Result<()> {
    println!("{}", "Parsing...".dimmed());

    match session.parse(code) {
        Ok(result) => {
            if result.success {
                println!("{} {}", "✓".green(), "Parse successful".green());
                if let Some(tree) = result.tree {
                    println!("{}", "Tree:".bright_yellow());
                    print_tree(&tree, 0);
                }
                println!("{}: {:.2}ms", "Time".dimmed(), result.timing.total_ms);
            } else {
                println!("{} {}", "✗".red(), "Parse failed".red());
                for error in result.errors {
                    println!(
                        "  {} at line {}, col {}: {}",
                        "Error".red(),
                        error.line,
                        error.column,
                        error.message
                    );
                }
            }
        }
        Err(e) => {
            println!("{} {}", "Error:".red(), e);
        }
    }

    Ok(())
}

fn handle_test(session: &mut PlaygroundSession, name: &str, input: &str) -> Result<()> {
    session.add_test_case(TestCase {
        name: name.to_string(),
        input: input.to_string(),
        expected_tree: None,
        should_pass: true,
        tags: vec![],
    });

    println!("{} Test '{}' added", "✓".green(), name);
    Ok(())
}

fn handle_run(session: &PlaygroundSession) -> Result<()> {
    let results = session.run_tests();

    if results.is_empty() {
        println!("{}", "No tests to run".yellow());
        return Ok(());
    }

    let mut passed = 0;
    let mut failed = 0;

    println!("{}", "Running tests...".dimmed());
    println!();

    for (test, result) in results {
        if result.unavailable {
            // No backend ran: never count this as passing (or as a genuine
            // parse failure); surface it as its own terminal state.
            println!("{} {} (backend unavailable)", "✗".red(), test.name);
            failed += 1;
        } else if result.success == test.should_pass {
            println!("{} {}", "✓".green(), test.name);
            passed += 1;
        } else {
            println!("{} {}", "✗".red(), test.name);
            if !result.errors.is_empty() {
                for error in result.errors {
                    println!("    {}", error.message.dimmed());
                }
            }
            failed += 1;
        }
    }

    println!();
    println!(
        "{}: {} passed, {} failed",
        "Summary".bright_yellow(),
        passed.to_string().green(),
        failed.to_string().red()
    );

    Ok(())
}

fn handle_analyze(session: &mut PlaygroundSession) -> Result<()> {
    println!("{}", "Analyzing grammar...".dimmed());

    match session.analyze_grammar() {
        Ok(analysis) => {
            println!("{}", "Grammar Statistics:".bright_yellow());
            println!("  Rules: {}", analysis.grammar_stats.rule_count);
            println!("  Terminals: {}", analysis.grammar_stats.terminal_count);
            println!(
                "  Non-terminals: {}",
                analysis.grammar_stats.nonterminal_count
            );
            println!(
                "  Avg rule length: {:.1}",
                analysis.grammar_stats.avg_rule_length
            );

            if !analysis.conflicts.is_empty() {
                println!();
                println!("{}", "Conflicts:".bright_red());
                for conflict in &analysis.conflicts {
                    println!(
                        "  {} conflict in state {}: {}",
                        match conflict.kind {
                            crate::ConflictKind::ShiftReduce => "Shift/Reduce",
                            crate::ConflictKind::ReduceReduce => "Reduce/Reduce",
                        },
                        conflict.state,
                        conflict.description
                    );
                }
            }

            if !analysis.suggestions.is_empty() {
                println!();
                println!("{}", "Suggestions:".bright_cyan());
                for suggestion in &analysis.suggestions {
                    let icon = match suggestion.level {
                        crate::SuggestionLevel::Info => "ℹ".blue(),
                        crate::SuggestionLevel::Warning => "⚠".yellow(),
                        crate::SuggestionLevel::Error => "✗".red(),
                    };
                    println!("  {} {}", icon, suggestion.message);
                }
            }
        }
        Err(e) => {
            println!("{} {}", "Error:".red(), e);
        }
    }

    Ok(())
}

fn handle_load(session: &mut PlaygroundSession, path: &str) -> Result<()> {
    match std::fs::read_to_string(path) {
        Ok(data) => match session.import(&data) {
            Ok(_) => println!("{} Loaded from {}", "✓".green(), path),
            Err(e) => println!("{} Failed to load: {}", "✗".red(), e),
        },
        Err(e) => println!("{} Cannot read file: {}", "✗".red(), e),
    }
    Ok(())
}

fn handle_save(session: &PlaygroundSession, path: &str) -> Result<()> {
    match session.export() {
        Ok(data) => match std::fs::write(path, data) {
            Ok(_) => println!("{} Saved to {}", "✓".green(), path),
            Err(e) => println!("{} Cannot write file: {}", "✗".red(), e),
        },
        Err(e) => println!("{} Failed to export: {}", "✗".red(), e),
    }
    Ok(())
}

fn handle_stats(session: &mut PlaygroundSession) -> Result<()> {
    match session.analyze_grammar() {
        Ok(analysis) => {
            let stats = &analysis.grammar_stats;
            println!("{}", "Grammar Statistics:".bright_yellow());
            println!("┌─────────────────────┬─────────┐");
            println!("│ {:19} │ {:7} │", "Metric", "Value");
            println!("├─────────────────────┼─────────┤");
            println!("│ {:19} │ {:7} │", "Rules", stats.rule_count);
            println!("│ {:19} │ {:7} │", "Terminals", stats.terminal_count);
            println!("│ {:19} │ {:7} │", "Non-terminals", stats.nonterminal_count);
            println!("│ {:19} │ {:7} │", "Nullable rules", stats.nullable_rules);
            println!(
                "│ {:19} │ {:7} │",
                "Left recursive", stats.left_recursive_rules
            );
            println!(
                "│ {:19} │ {:7} │",
                "Right recursive", stats.right_recursive_rules
            );
            println!("└─────────────────────┴─────────┘");
        }
        Err(e) => {
            println!("{} {}", "Error:".red(), e);
        }
    }
    Ok(())
}

fn print_tree(tree: &str, indent: usize) {
    // Simple tree pretty-printer
    let spaces = " ".repeat(indent);
    println!("{}{}", spaces, tree);
}
