// Simplified parallel parser for adze
// Uses rayon for data-parallel parsing of large files

use anyhow::Result;
use rayon::prelude::*;
use std::sync::Arc;

use crate::parser_v4::{ParseNode, Parser};
use adze_glr_core::ParseTable;
use adze_ir::Grammar;

/// Parallel parser configuration
#[derive(Debug, Clone)]
pub struct ParallelConfig {
    /// Minimum file size (bytes) to enable parallel parsing
    pub min_file_size: usize,
    /// Target chunk size for splitting
    pub chunk_size: usize,
    /// Number of worker threads (0 = use rayon default)
    pub num_threads: usize,
}

impl Default for ParallelConfig {
    fn default() -> Self {
        Self {
            min_file_size: 100_000, // 100KB
            chunk_size: 50_000,     // 50KB chunks
            num_threads: 0,         // Use all available cores
        }
    }
}

/// Parallel parser for large files
pub struct ParallelParser {
    grammar: Arc<Grammar>,
    parse_table: Arc<ParseTable>,
    config: ParallelConfig,
}

/// Chunk of input for parallel processing
#[derive(Debug)]
struct ParseChunk {
    id: usize,
    #[allow(dead_code)]
    start: usize,
    #[allow(dead_code)]
    end: usize,
    content: String,
}

/// Result of parsing a chunk
#[derive(Debug)]
struct ChunkResult {
    chunk_id: usize,
    tree: Option<ParseNode>,
    #[allow(dead_code)]
    parse_time_ms: f64,
}

impl ParallelParser {
    pub fn new(grammar: Grammar, parse_table: ParseTable, config: ParallelConfig) -> Self {
        // Configure rayon thread pool if specified
        if config.num_threads > 0 {
            rayon::ThreadPoolBuilder::new()
                .num_threads(config.num_threads)
                .build_global()
                .ok();
        }

        Self {
            grammar: Arc::new(grammar),
            parse_table: Arc::new(parse_table),
            config,
        }
    }

    /// Parse input in parallel
    pub fn parse(&self, input: &str) -> Result<ParseNode> {
        // For small files, use single-threaded parser
        if input.len() < self.config.min_file_size {
            let mut parser = Parser::new((*self.grammar).clone(), (*self.parse_table).clone());
            return parser.parse(input);
        }

        // Split input into chunks
        let chunks = self.split_into_chunks(input);

        // Parse chunks in parallel
        let chunk_results: Vec<ChunkResult> = chunks
            .into_par_iter()
            .map(|chunk| self.parse_chunk(chunk))
            .collect();

        // Merge results
        self.merge_chunk_results(chunk_results, input)
    }

    /// Split input into chunks for parallel processing
    fn split_into_chunks(&self, input: &str) -> Vec<ParseChunk> {
        let mut chunks = Vec::new();
        let chunk_size = self.config.chunk_size;

        let mut start = 0;
        let mut id = 0;

        while start < input.len() {
            let mut end = (start + chunk_size).min(input.len());
            end = align_to_char_boundary(input, end);

            // Try to find a clean boundary (newline)
            if end < input.len() {
                let search_start = end.saturating_sub(1000);
                let search_start = align_to_char_boundary(input, search_start);
                let search_bytes = &input.as_bytes()[search_start..end];

                // Look for last newline
                if let Some(pos) = search_bytes.iter().rposition(|&b| b == b'\n') {
                    end = search_start + pos + 1;
                }
            }

            chunks.push(ParseChunk {
                id,
                start,
                end,
                content: input[start..end].to_owned(),
            });

            start = end;
            id += 1;
        }

        chunks
    }

    /// Parse a single chunk
    fn parse_chunk(&self, chunk: ParseChunk) -> ChunkResult {
        use std::time::Instant;
        let start_time = Instant::now();

        // Create parser for this chunk
        let mut parser = Parser::new((*self.grammar).clone(), (*self.parse_table).clone());

        // Try to parse the chunk
        let tree = parser.parse(&chunk.content).ok();

        let parse_time_ms = start_time.elapsed().as_secs_f64() * 1000.0;

        ChunkResult {
            chunk_id: chunk.id,
            tree,
            parse_time_ms,
        }
    }

    /// Merge chunk results into final tree
    fn merge_chunk_results(&self, mut results: Vec<ChunkResult>, input: &str) -> Result<ParseNode> {
        // Sort by chunk ID to maintain order
        results.sort_by_key(|r| r.chunk_id);

        // For now, just return the first successful parse
        // In a real implementation, we would merge the trees
        for result in results {
            if let Some(tree) = result.tree {
                return Ok(tree);
            }
        }

        // If no chunks parsed successfully, parse the whole input
        let mut parser = Parser::new((*self.grammar).clone(), (*self.parse_table).clone());
        parser.parse(input)
    }
}

fn align_to_char_boundary(input: &str, mut idx: usize) -> usize {
    while idx > 0 && !input.is_char_boundary(idx) {
        idx -= 1;
    }
    idx
}

/// Parallel parsing statistics
#[derive(Debug, Default)]
pub struct ParallelStats {
    pub total_chunks: usize,
    pub successful_chunks: usize,
    pub total_parse_time_ms: f64,
    pub speedup: f64,
}

impl ParallelParser {
    /// Parse with statistics collection
    pub fn parse_with_stats(&self, input: &str) -> Result<(ParseNode, ParallelStats)> {
        use std::time::Instant;

        // Measure single-threaded baseline
        let baseline_start = Instant::now();
        let mut baseline_parser = Parser::new((*self.grammar).clone(), (*self.parse_table).clone());
        let _ = baseline_parser.parse(input);
        let baseline_time = baseline_start.elapsed().as_secs_f64() * 1000.0;

        // Measure parallel parsing
        let parallel_start = Instant::now();
        let tree = self.parse(input)?;
        let parallel_time = parallel_start.elapsed().as_secs_f64() * 1000.0;

        let stats = ParallelStats {
            total_chunks: (input.len() + self.config.chunk_size - 1) / self.config.chunk_size,
            successful_chunks: 1, // Simplified
            total_parse_time_ms: parallel_time,
            speedup: baseline_time / parallel_time,
        };

        Ok((tree, stats))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_grammar() -> (Grammar, ParseTable) {
        // Simple test grammar
        let grammar = Grammar::new("test".to_string());
        let table = ParseTable {
            action_table: vec![],
            goto_table: vec![],
            symbol_metadata: vec![],
            state_count: 1,
            symbol_count: 1,
            symbol_to_index: std::collections::BTreeMap::new(),
            external_scanner_states: vec![],
        };
        (grammar, table)
    }

    #[test]
    fn test_chunk_splitting() {
        let (grammar, table) = create_test_grammar();
        let config = ParallelConfig {
            min_file_size: 10,
            chunk_size: 20,
            ..Default::default()
        };
        let parser = ParallelParser::new(grammar, table, config);

        let input = "line1\nline2\nline3\nline4\nline5\n";
        let chunks = parser.split_into_chunks(input);

        assert!(chunks.len() >= 2);
        for chunk in &chunks {
            assert!(chunk.end > chunk.start);
            assert_eq!(&input[chunk.start..chunk.end], &chunk.content);
        }
    }

    #[test]
    fn test_small_file_handling() {
        let (grammar, table) = create_test_grammar();
        let config = ParallelConfig {
            min_file_size: 1000,
            ..Default::default()
        };
        let parser = ParallelParser::new(grammar, table, config);

        let input = "small input";
        // Should use single-threaded parser for small inputs
        let _ = parser.parse(input);
    }

    #[test]
    fn test_chunk_splitting_handles_unicode_boundaries_without_panicking() {
        let (grammar, table) = create_test_grammar();
        let config = ParallelConfig {
            min_file_size: 1,
            chunk_size: 5,
            ..Default::default()
        };
        let parser = ParallelParser::new(grammar, table, config);

        let input = "éééé\néééé\n";
        let chunks = parser.split_into_chunks(input);
        assert!(!chunks.is_empty());
        for chunk in &chunks {
            assert!(input.is_char_boundary(chunk.start));
            assert!(input.is_char_boundary(chunk.end));
            assert_eq!(&input[chunk.start..chunk.end], chunk.content);
        }
    }
}

/// Benchmark utilities
#[cfg(all(test, not(debug_assertions)))]
pub mod bench {
    use super::*;
    use std::time::Instant;

    pub struct ParallelBenchmark {
        pub file_size: usize,
        pub single_thread_ms: f64,
        pub parallel_ms: f64,
        pub speedup: f64,
        pub num_chunks: usize,
    }

    pub fn benchmark_parallel_parsing(
        grammar: Grammar,
        table: ParseTable,
        input: &str,
    ) -> ParallelBenchmark {
        // Single-threaded baseline
        let start = Instant::now();
        let mut parser = Parser::new(grammar.clone(), table.clone());
        let _ = parser.parse(input);
        let single_thread_ms = start.elapsed().as_secs_f64() * 1000.0;

        // Parallel parsing
        let config = ParallelConfig::default();
        let parallel_parser = ParallelParser::new(grammar, table, config.clone());

        let start = Instant::now();
        let _ = parallel_parser.parse(input);
        let parallel_ms = start.elapsed().as_secs_f64() * 1000.0;

        ParallelBenchmark {
            file_size: input.len(),
            single_thread_ms,
            parallel_ms,
            speedup: single_thread_ms / parallel_ms,
            num_chunks: (input.len() + config.chunk_size - 1) / config.chunk_size,
        }
    }
}
