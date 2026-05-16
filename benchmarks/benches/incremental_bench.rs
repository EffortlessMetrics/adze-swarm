//! Classification: real_parser
//! Status: active
//! CI coverage: ci.yml Benchmark Compilation (compile-only incremental_bench),
//!   criterion-smoke.yml benchmark compile check (compile-only),
//!   pure-rust-ci.yml dispatch-only benchmark compile check (compile-only)
//!
//! Incremental GLR parser benchmark. Measures full-reparse vs incremental
//! reparse across different edit patterns and file sizes, plus fork
//! preservation and subtree reuse efficiency.

use adze::glr_incremental::{GLREdit, GLRToken, IncrementalGLRParser};
use adze::glr_parser::GLRParser;
use adze_benchmarks::test_grammars::{load_arithmetic_grammar, tokenize_arithmetic};
use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};

/// Common edit patterns in programming
#[derive(Debug, Clone)]
enum EditPattern {
    /// Single character insertion (e.g., typing)
    CharacterInsertion,
    /// Word replacement (e.g., renaming a variable)
    WordReplacement,
    /// Line insertion (e.g., adding a new statement)
    LineInsertion,
    /// Block deletion (e.g., removing a function)
    BlockDeletion,
    /// Multiple scattered edits (e.g., refactoring)
    MultipleEdits,
}

impl EditPattern {
    fn apply(&self, text: &str) -> (String, Vec<GLREdit>) {
        match self {
            EditPattern::CharacterInsertion => {
                // Insert a character in the middle
                let pos = text.len() / 2;
                let mut new_text = text.to_string();
                new_text.insert(pos, 'x');

                let edit = GLREdit {
                    old_range: pos..pos,
                    new_text: b"x".to_vec(),
                    old_token_range: 0..0, // Would be computed from lexer
                    new_tokens: vec![],
                    old_tokens: vec![],
                    old_forest: None,
                };

                (new_text, vec![edit])
            }

            EditPattern::WordReplacement => {
                // Replace a word (simulate variable rename)
                let words: Vec<&str> = text.split_whitespace().collect();
                if words.len() > 2 {
                    let target = words[1];
                    let start = text.find(target).unwrap_or(0);
                    let end = start + target.len();

                    let new_word = "renamed_var";
                    let mut new_text = text.to_string();
                    new_text.replace_range(start..end, new_word);

                    let edit = GLREdit {
                        old_range: start..end,
                        new_text: new_word.as_bytes().to_vec(),
                        old_token_range: 1..2,
                        new_tokens: vec![],
                        old_tokens: vec![],
                        old_forest: None,
                    };

                    (new_text, vec![edit])
                } else {
                    (text.to_string(), vec![])
                }
            }

            EditPattern::LineInsertion => {
                // Insert a new line
                let lines: Vec<&str> = text.lines().collect();
                let insert_pos = lines.len() / 2;

                let mut new_lines = lines.clone();
                new_lines.insert(insert_pos, "    // New comment line");
                let new_text = new_lines.join("\n");

                let line_start: usize = lines[..insert_pos].iter().map(|l| l.len() + 1).sum();
                let edit = GLREdit {
                    old_range: line_start..line_start,
                    new_text: b"    // New comment line\n".to_vec(),
                    old_token_range: 0..0,
                    new_tokens: vec![],
                    old_tokens: vec![],
                    old_forest: None,
                };

                (new_text, vec![edit])
            }

            EditPattern::BlockDeletion => {
                // Delete a block of text
                let block_size = text.len() / 4;
                let start = text.len() / 3;
                let end = (start + block_size).min(text.len());

                let mut new_text = text.to_string();
                new_text.drain(start..end);

                let edit = GLREdit {
                    old_range: start..end,
                    new_text: vec![],
                    old_token_range: 0..0,
                    new_tokens: vec![],
                    old_tokens: vec![],
                    old_forest: None,
                };

                (new_text, vec![edit])
            }

            EditPattern::MultipleEdits => {
                // Apply multiple scattered edits
                let mut edits = vec![];
                let mut new_text = text.to_string();

                // Edit 1: Insert at 25%
                let pos1 = text.len() / 4;
                new_text.insert_str(pos1, "/* edit1 */");
                edits.push(GLREdit {
                    old_range: pos1..pos1,
                    new_text: b"/* edit1 */".to_vec(),
                    old_token_range: 0..0,
                    new_tokens: vec![],
                    old_tokens: vec![],
                    old_forest: None,
                });

                // Edit 2: Delete at 50%
                let pos2 = text.len() / 2;
                let delete_len = 10.min(text.len() - pos2);
                edits.push(GLREdit {
                    old_range: pos2..pos2 + delete_len,
                    new_text: vec![],
                    old_token_range: 0..0,
                    new_tokens: vec![],
                    old_tokens: vec![],
                    old_forest: None,
                });

                // Edit 3: Replace at 75%
                let pos3 = (text.len() * 3) / 4;
                let replace_len = 5.min(text.len() - pos3);
                edits.push(GLREdit {
                    old_range: pos3..pos3 + replace_len,
                    new_text: b"REPLACED".to_vec(),
                    old_token_range: 0..0,
                    new_tokens: vec![],
                    old_tokens: vec![],
                    old_forest: None,
                });

                (new_text, edits)
            }
        }
    }
}

/// Generate sample arithmetic expressions for benchmarking
fn generate_sample_code(size: usize) -> String {
    // Generate arithmetic expressions with the given number of operations
    let mut expr = String::from("1");
    for i in 0..size {
        if i % 2 == 0 {
            expr.push_str(&format!(" - {}", i + 2));
        } else {
            expr.push_str(&format!(" * {}", i + 2));
        }
    }
    expr
}

/// Tokenize source code using real grammar tokenizer
fn tokenize(text: &str) -> Vec<GLRToken> {
    // Convert from TestToken to GLRToken
    tokenize_arithmetic(text)
        .into_iter()
        .map(|t| GLRToken {
            symbol: t.symbol,
            text: t.text,
            start_byte: t.start_byte,
            end_byte: t.end_byte,
        })
        .collect()
}

fn benchmark_incremental_parsing(c: &mut Criterion) {
    let mut group = c.benchmark_group("incremental_parsing");

    // Test different file sizes
    let file_sizes = vec![100, 500, 1000];
    let edit_patterns = vec![
        EditPattern::CharacterInsertion,
        EditPattern::WordReplacement,
        EditPattern::LineInsertion,
        EditPattern::BlockDeletion,
        EditPattern::MultipleEdits,
    ];

    for size in file_sizes {
        let code = generate_sample_code(size);
        let tokens = tokenize(&code);

        // Load real grammar and parse table
        let (grammar, table) = load_arithmetic_grammar();

        for pattern in &edit_patterns {
            let bench_name = format!("{:?}_size_{}", pattern, size);

            // Pre-compute edit and tokens outside benchmark loop
            let (new_code, edits) = pattern.apply(&code);
            let new_tokens = tokenize(&new_code);

            group.bench_function(BenchmarkId::new("full_reparse", &bench_name), |b| {
                b.iter(|| {
                    // Full reparse with pre-computed tokens
                    let mut parser = GLRParser::new(table.clone(), grammar.clone());
                    for token in &new_tokens {
                        let text_str = String::from_utf8_lossy(&token.text);
                        parser.process_token(token.symbol, &text_str, token.start_byte);
                    }
                    let total_bytes = new_tokens
                        .last()
                        .map(|t| t.start_byte + t.text.len())
                        .unwrap_or(0);
                    parser.process_eof(total_bytes);
                    parser.finish()
                });
            });

            group.bench_function(BenchmarkId::new("incremental", &bench_name), |b| {
                // Initial parse outside benchmark loop
                let mut inc_parser = IncrementalGLRParser::new(grammar.clone(), table.clone());
                let _ = inc_parser.parse_incremental(&tokens, &[]);

                b.iter(|| {
                    // Incremental reparse with pre-computed edits
                    inc_parser.parse_incremental(&tokens, &edits)
                });
            });
        }
    }

    group.finish();
}

fn benchmark_fork_preservation(c: &mut Criterion) {
    let mut group = c.benchmark_group("fork_preservation");

    // Test how well we preserve forks across edits
    // Arithmetic expression that could have multiple parse trees
    let ambiguous_code = "1 - 2 * 3 - 4 * 5";

    let tokens = tokenize(ambiguous_code);
    let (grammar, table) = load_arithmetic_grammar();

    group.bench_function("fork_tracking_overhead", |b| {
        let mut parser = IncrementalGLRParser::new(grammar.clone(), table.clone());
        let _ = parser.parse_incremental(&tokens, &[]);

        b.iter(|| {
            // Small edit that shouldn't affect most forks
            let edit = GLREdit {
                old_range: 10..11,
                new_text: b"X".to_vec(),
                old_token_range: 2..3,
                new_tokens: vec![],
                old_tokens: vec![],
                old_forest: None,
            };

            parser.parse_incremental(&tokens, &[edit])
        });
    });

    group.finish();
}

fn benchmark_reuse_efficiency(c: &mut Criterion) {
    let mut group = c.benchmark_group("subtree_reuse");

    // Test subtree reuse efficiency
    let code_sizes = vec![1000, 5000, 10000];

    for size in code_sizes {
        let code = generate_sample_code(size);
        let tokens = tokenize(&code);
        let (grammar, table) = load_arithmetic_grammar();

        group.bench_function(BenchmarkId::new("reuse_ratio", size), |b| {
            let mut parser = IncrementalGLRParser::new(grammar.clone(), table.clone());
            let _ = parser.parse_incremental(&tokens, &[]);

            b.iter(|| {
                // Small localized edit
                let edit = GLREdit {
                    old_range: size / 2..size / 2 + 10,
                    new_text: b"EDITED".to_vec(),
                    old_token_range: 0..0,
                    new_tokens: vec![],
                    old_tokens: vec![],
                    old_forest: None,
                };

                parser.parse_incremental(&tokens, &[edit])
            });
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    benchmark_incremental_parsing,
    benchmark_fork_preservation,
    benchmark_reuse_efficiency
);
criterion_main!(benches);
