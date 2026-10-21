// Interactive grammar playground for adze
// Provides web-based and CLI interfaces for testing grammars

use adze_glr_core::ParseTable;
use adze_ir::Grammar;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub mod analyzer;
pub mod cli;
pub mod visualizer;
pub mod web;

/// Playground session for interactive grammar testing
#[derive(Debug, Clone)]
pub struct PlaygroundSession {
    grammar: Grammar,
    #[allow(dead_code)]
    parse_table: Option<ParseTable>,
    test_cases: Vec<TestCase>,
    analysis_cache: HashMap<String, AnalysisResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestCase {
    pub name: String,
    pub input: String,
    pub expected_tree: Option<String>,
    pub should_pass: bool,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParseResult {
    pub success: bool,
    pub tree: Option<String>,
    pub errors: Vec<ParseError>,
    pub timing: ParseTiming,
    pub visualization: Option<String>,
    /// True when no parser backend executed at all. An unavailable result
    /// must never be counted as a passing (or genuinely failing) parse.
    #[serde(default)]
    pub unavailable: bool,
}

/// Message reported when a parse is requested but no parser backend is
/// connected to the session.
pub const BACKEND_UNAVAILABLE_MESSAGE: &str =
    "playground parser backend unavailable: no generated parser is connected to this session";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParseError {
    pub message: String,
    pub line: usize,
    pub column: usize,
    pub offset: usize,
    pub length: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParseTiming {
    pub lexing_ms: f64,
    pub parsing_ms: f64,
    pub total_ms: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisResult {
    pub grammar_stats: GrammarStats,
    pub conflicts: Vec<Conflict>,
    pub ambiguities: Vec<Ambiguity>,
    pub suggestions: Vec<Suggestion>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrammarStats {
    pub rule_count: usize,
    pub terminal_count: usize,
    pub nonterminal_count: usize,
    pub max_rule_length: usize,
    pub avg_rule_length: f64,
    pub nullable_rules: usize,
    pub left_recursive_rules: usize,
    pub right_recursive_rules: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conflict {
    pub kind: ConflictKind,
    pub state: usize,
    pub symbol: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConflictKind {
    ShiftReduce,
    ReduceReduce,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ambiguity {
    pub rule: String,
    pub example: String,
    pub parse_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Suggestion {
    pub level: SuggestionLevel,
    pub message: String,
    pub rule: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SuggestionLevel {
    Info,
    Warning,
    Error,
}

impl PlaygroundSession {
    /// Create a new playground session
    pub fn new(grammar: Grammar) -> Self {
        Self {
            grammar,
            parse_table: None,
            test_cases: Vec::new(),
            analysis_cache: HashMap::new(),
        }
    }

    /// Initialize the parse table
    pub fn initialize(&mut self) -> Result<()> {
        // TODO: Implement parse table building when API is stable
        // For now, we'll use a placeholder implementation
        Ok(())
    }

    /// Parse input text.
    ///
    /// No generated parser backend is connected to this session yet, so this
    /// returns an explicit unavailable error instead of fabricating a
    /// successful parse. Callers must not report the input as parsed.
    pub fn parse(&self, _input: &str) -> Result<ParseResult> {
        anyhow::bail!(BACKEND_UNAVAILABLE_MESSAGE)
    }

    /// Add a test case
    pub fn add_test_case(&mut self, test_case: TestCase) {
        self.test_cases.push(test_case);
    }

    /// Run all test cases
    pub fn run_tests(&self) -> Vec<(TestCase, ParseResult)> {
        self.test_cases
            .iter()
            .map(|test| {
                let result = self.parse(&test.input).unwrap_or_else(|e| ParseResult {
                    success: false,
                    tree: None,
                    errors: vec![ParseError {
                        message: e.to_string(),
                        line: 0,
                        column: 0,
                        offset: 0,
                        length: 0,
                    }],
                    timing: ParseTiming {
                        lexing_ms: 0.0,
                        parsing_ms: 0.0,
                        total_ms: 0.0,
                    },
                    visualization: None,
                    unavailable: true,
                });
                (test.clone(), result)
            })
            .collect()
    }

    /// Analyze the grammar
    pub fn analyze_grammar(&mut self) -> Result<&AnalysisResult> {
        let cache_key = format!("{:?}", self.grammar);

        if !self.analysis_cache.contains_key(&cache_key) {
            let analysis = analyzer::analyze_grammar(&self.grammar)?;
            self.analysis_cache.insert(cache_key.clone(), analysis);
        }

        Ok(self.analysis_cache.get(&cache_key).unwrap())
    }

    /// Generate visualization
    pub fn visualize_tree(&self, tree: &str) -> Result<String> {
        visualizer::generate_tree_svg(tree)
    }

    /// Export session data
    pub fn export(&self) -> Result<String> {
        let data = PlaygroundExport {
            grammar_name: self.grammar.name.clone(),
            test_cases: self.test_cases.clone(),
            analysis: self.analysis_cache.values().cloned().collect(),
        };

        Ok(serde_json::to_string_pretty(&data)?)
    }

    /// Import session data
    pub fn import(&mut self, data: &str) -> Result<()> {
        let export: PlaygroundExport = serde_json::from_str(data)?;
        self.test_cases = export.test_cases;
        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct PlaygroundExport {
    grammar_name: String,
    test_cases: Vec<TestCase>,
    analysis: Vec<AnalysisResult>,
}

/// Interactive grammar playground builder
pub struct PlaygroundBuilder {
    grammar_path: Option<String>,
    test_file: Option<String>,
    output_dir: Option<String>,
    features: Vec<PlaygroundFeature>,
}

#[derive(Debug, Clone)]
pub enum PlaygroundFeature {
    WebInterface(u16), // port
    CliInterface,
    Visualization,
    Analysis,
    TestRunner,
}

impl Default for PlaygroundBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl PlaygroundBuilder {
    pub fn new() -> Self {
        Self {
            grammar_path: None,
            test_file: None,
            output_dir: None,
            features: Vec::new(),
        }
    }

    pub fn grammar(mut self, path: impl Into<String>) -> Self {
        self.grammar_path = Some(path.into());
        self
    }

    pub fn tests(mut self, path: impl Into<String>) -> Self {
        self.test_file = Some(path.into());
        self
    }

    pub fn output(mut self, dir: impl Into<String>) -> Self {
        self.output_dir = Some(dir.into());
        self
    }

    pub fn feature(mut self, feature: PlaygroundFeature) -> Self {
        self.features.push(feature);
        self
    }

    pub fn build(self) -> Result<()> {
        // Load grammar
        let grammar = self.load_grammar()?;
        let mut session = PlaygroundSession::new(grammar);
        session.initialize()?;

        // Load test cases if provided
        if let Some(ref test_file) = self.test_file {
            let tests = self.load_tests(test_file)?;
            for test in tests {
                session.add_test_case(test);
            }
        }

        // Launch features
        for feature in self.features {
            match feature {
                PlaygroundFeature::WebInterface(port) => {
                    web::launch_server(session.clone(), port)?;
                }
                PlaygroundFeature::CliInterface => {
                    cli::run_interactive(session.clone())?;
                }
                PlaygroundFeature::Visualization => {
                    // Enable visualization in session
                }
                PlaygroundFeature::Analysis => {
                    let analysis = session.analyze_grammar()?;
                    println!("Grammar Analysis: {:?}", analysis);
                }
                PlaygroundFeature::TestRunner => {
                    let results = session.run_tests();
                    for (test, result) in results {
                        println!(
                            "{}: {}",
                            test.name,
                            if result.unavailable {
                                "UNAVAILABLE"
                            } else if result.success {
                                "PASS"
                            } else {
                                "FAIL"
                            }
                        );
                    }
                }
            }
        }

        Ok(())
    }

    fn load_grammar(&self) -> Result<Grammar> {
        // Placeholder - would load from file
        Ok(Grammar::default())
    }

    fn load_tests(&self, _path: &str) -> Result<Vec<TestCase>> {
        // Placeholder - would load from file
        Ok(vec![])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_playground_session() {
        let grammar = Grammar::default();
        let mut session = PlaygroundSession::new(grammar);

        session.add_test_case(TestCase {
            name: "simple".to_string(),
            input: "1 + 2".to_string(),
            expected_tree: Some("(expr (num 1) + (num 2))".to_string()),
            should_pass: true,
            tags: vec!["arithmetic".to_string()],
        });

        let results = session.run_tests();
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_parse_without_backend_reports_unavailable_not_success() {
        let session = PlaygroundSession::new(Grammar::default());

        for input in ["1 + 2", "this is not valid !!!"] {
            let error = session
                .parse(input)
                .expect_err("no backend is connected, so parse must not report success");
            assert!(
                error.to_string().contains("backend unavailable"),
                "error should name the unavailable backend: {error}"
            );
        }
    }

    #[test]
    fn test_run_tests_marks_unavailable_results_distinctly() {
        let mut session = PlaygroundSession::new(Grammar::default());
        session.add_test_case(TestCase {
            name: "expects-pass".to_string(),
            input: "1 + 2".to_string(),
            expected_tree: None,
            should_pass: true,
            tags: vec![],
        });
        session.add_test_case(TestCase {
            name: "expects-fail".to_string(),
            input: "!!!".to_string(),
            expected_tree: None,
            should_pass: false,
            tags: vec![],
        });

        let results = session.run_tests();
        assert_eq!(results.len(), 2);
        for (_, result) in &results {
            assert!(result.unavailable, "no backend ran for any test case");
            assert!(!result.success, "unavailable must not look like success");
            assert!(result.tree.is_none(), "unavailable must not invent a tree");
        }
    }

    #[test]
    fn test_test_case_creation() {
        let test_case = TestCase {
            name: "test-case".to_string(),
            input: "let x = 42".to_string(),
            expected_tree: Some("(program (let_stmt))".to_string()),
            should_pass: true,
            tags: vec!["statement".to_string(), "variable".to_string()],
        };

        assert_eq!(test_case.name, "test-case");
        assert_eq!(test_case.input, "let x = 42");
        assert!(test_case.should_pass);
        assert_eq!(test_case.tags.len(), 2);
        assert!(test_case.tags.contains(&"statement".to_string()));
    }

    #[test]
    fn test_parse_result_success() {
        let result = ParseResult {
            success: true,
            tree: Some("(program)".to_string()),
            errors: vec![],
            timing: ParseTiming {
                lexing_ms: 0.5,
                parsing_ms: 1.0,
                total_ms: 1.5,
            },
            visualization: None,
            unavailable: false,
        };

        assert!(result.success);
        assert_eq!(result.tree, Some("(program)".to_string()));
        assert!(result.errors.is_empty());
        assert_eq!(result.timing.total_ms, 1.5);
    }

    #[test]
    fn test_parse_result_failure() {
        let result = ParseResult {
            success: false,
            tree: None,
            errors: vec![ParseError {
                message: "Unexpected token at position 5".to_string(),
                line: 1,
                column: 5,
                offset: 5,
                length: 1,
            }],
            timing: ParseTiming {
                lexing_ms: 0.2,
                parsing_ms: 0.3,
                total_ms: 0.5,
            },
            visualization: None,
            unavailable: false,
        };

        assert!(!result.success);
        assert!(result.tree.is_none());
        assert_eq!(result.errors.len(), 1);
        assert_eq!(result.errors[0].message, "Unexpected token at position 5");
    }

    #[test]
    fn test_playground_session_multiple_tests() {
        let grammar = Grammar::default();
        let mut session = PlaygroundSession::new(grammar);

        // Add multiple test cases
        session.add_test_case(TestCase {
            name: "test1".to_string(),
            input: "1 + 2".to_string(),
            expected_tree: None,
            should_pass: true,
            tags: vec![],
        });

        session.add_test_case(TestCase {
            name: "test2".to_string(),
            input: "invalid syntax".to_string(),
            expected_tree: None,
            should_pass: false,
            tags: vec![],
        });

        session.add_test_case(TestCase {
            name: "test3".to_string(),
            input: "x = y".to_string(),
            expected_tree: Some("(assignment)".to_string()),
            should_pass: true,
            tags: vec!["assignment".to_string()],
        });

        let results = session.run_tests();
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn test_playground_builder() {
        let builder = PlaygroundBuilder::new()
            .grammar("path/to/grammar.rs")
            .tests("path/to/tests.yaml")
            .output("output/dir")
            .feature(PlaygroundFeature::CliInterface)
            .feature(PlaygroundFeature::Analysis);

        assert_eq!(builder.grammar_path, Some("path/to/grammar.rs".to_string()));
        assert_eq!(builder.test_file, Some("path/to/tests.yaml".to_string()));
        assert_eq!(builder.output_dir, Some("output/dir".to_string()));
        assert_eq!(builder.features.len(), 2);
    }

    #[test]
    fn test_playground_features() {
        let features = [
            PlaygroundFeature::WebInterface(8080),
            PlaygroundFeature::CliInterface,
            PlaygroundFeature::Visualization,
            PlaygroundFeature::Analysis,
            PlaygroundFeature::TestRunner,
        ];

        assert_eq!(features.len(), 5);

        // Test feature matching
        match features[0] {
            PlaygroundFeature::WebInterface(port) => assert_eq!(port, 8080),
            _ => panic!("Expected WebInterface"),
        }
    }

    #[test]
    fn test_parse_timing() {
        let timing = ParseTiming {
            lexing_ms: 0.5,
            parsing_ms: 1.5,
            total_ms: 2.0,
        };

        assert_eq!(timing.lexing_ms, 0.5);
        assert_eq!(timing.parsing_ms, 1.5);
        assert_eq!(timing.total_ms, 2.0);
    }

    #[test]
    fn test_parse_error() {
        let error = ParseError {
            message: "Syntax error".to_string(),
            line: 5,
            column: 10,
            offset: 42,
            length: 3,
        };

        assert_eq!(error.message, "Syntax error");
        assert_eq!(error.line, 5);
        assert_eq!(error.column, 10);
        assert_eq!(error.offset, 42);
        assert_eq!(error.length, 3);
    }

    #[test]
    fn test_analysis_result() {
        let result = AnalysisResult {
            grammar_stats: GrammarStats {
                rule_count: 20,
                terminal_count: 15,
                nonterminal_count: 10,
                max_rule_length: 5,
                avg_rule_length: 3.2,
                nullable_rules: 2,
                left_recursive_rules: 1,
                right_recursive_rules: 0,
            },
            conflicts: vec![],
            ambiguities: vec![],
            suggestions: vec![],
        };

        assert_eq!(result.grammar_stats.rule_count, 20);
        assert_eq!(result.grammar_stats.terminal_count, 15);
        assert_eq!(result.grammar_stats.nonterminal_count, 10);
        assert_eq!(result.grammar_stats.nullable_rules, 2);
        assert!(result.conflicts.is_empty());
    }

    #[test]
    fn test_playground_session_initialization() {
        let grammar = Grammar::default();
        let session = PlaygroundSession::new(grammar.clone());

        // Verify initial state
        let results = session.run_tests();
        assert!(results.is_empty());
    }

    #[test]
    fn test_playground_builder_default() {
        let builder = PlaygroundBuilder::new();

        assert!(builder.grammar_path.is_none());
        assert!(builder.test_file.is_none());
        assert!(builder.output_dir.is_none());
        assert!(builder.features.is_empty());
    }

    #[test]
    fn test_test_case_with_empty_tags() {
        let test_case = TestCase {
            name: "empty-tags".to_string(),
            input: "".to_string(),
            expected_tree: None,
            should_pass: false,
            tags: vec![],
        };

        assert!(test_case.tags.is_empty());
        assert!(!test_case.should_pass);
        assert!(test_case.input.is_empty());
    }

    #[test]
    fn test_parse_result_with_multiple_errors() {
        let result = ParseResult {
            success: false,
            tree: None,
            errors: vec![
                ParseError {
                    message: "Error 1".to_string(),
                    line: 1,
                    column: 1,
                    offset: 0,
                    length: 1,
                },
                ParseError {
                    message: "Error 2".to_string(),
                    line: 2,
                    column: 5,
                    offset: 10,
                    length: 2,
                },
                ParseError {
                    message: "Error 3".to_string(),
                    line: 3,
                    column: 10,
                    offset: 20,
                    length: 3,
                },
            ],
            timing: ParseTiming {
                lexing_ms: 0.05,
                parsing_ms: 0.05,
                total_ms: 0.1,
            },
            visualization: None,
            unavailable: false,
        };

        assert!(!result.success);
        assert_eq!(result.errors.len(), 3);
        assert_eq!(result.timing.total_ms, 0.1);
    }

    #[test]
    fn test_parse_result_with_visualization() {
        let result = ParseResult {
            success: true,
            tree: Some("(program)".to_string()),
            errors: vec![],
            timing: ParseTiming {
                lexing_ms: 0.5,
                parsing_ms: 1.0,
                total_ms: 1.5,
            },
            visualization: Some("<svg>...</svg>".to_string()),
            unavailable: false,
        };

        assert!(result.success);
        assert!(result.visualization.is_some());
        assert_eq!(result.visualization.unwrap(), "<svg>...</svg>");
    }
}
