// Quick test with just a few grammars
use anyhow::Result;
use std::path::PathBuf;

pub fn run_quick_test() -> Result<()> {
    use crate::test_grammars::{GrammarTest, TestStatus, download_grammar, test_grammar};

    let corpus_dir = PathBuf::from("corpus");
    std::fs::create_dir_all(&corpus_dir)?;

    let quick_tests = vec![
        GrammarTest {
            name: "json".to_string(),
            repo_url: "https://github.com/tree-sitter/tree-sitter-json".to_string(),
            expected_status: TestStatus::Working,
            blocking_features: vec![],
            notes: Some("Simple grammar, should work perfectly".to_string()),
        },
        GrammarTest {
            name: "c".to_string(),
            repo_url: "https://github.com/tree-sitter/tree-sitter-c".to_string(),
            expected_status: TestStatus::LikelyWorking,
            blocking_features: vec!["precedence".to_string()],
            notes: Some("Uses precedence, should work now".to_string()),
        },
    ];

    println!(
        "Running quick test with {} grammars...\n",
        quick_tests.len()
    );

    for test in &quick_tests {
        println!("Testing {}...", test.name);

        match download_grammar(test, &corpus_dir) {
            Ok(grammar_dir) => match test_grammar(test, &grammar_dir) {
                Ok(result) => {
                    println!("  Result: {:?}", result.status);
                    if let Some(err) = &result.error_message {
                        println!("  Error: {}", err);
                    }
                }
                Err(e) => {
                    println!("  Test error: {}", e);
                }
            },
            Err(e) => {
                println!("  Download error: {}", e);
            }
        }
    }

    Ok(())
}
