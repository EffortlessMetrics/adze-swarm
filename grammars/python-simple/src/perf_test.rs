use adze::parser_v4::Parser;
use std::time::Instant;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grammar::LANGUAGE;

    // This is an experimental adapter benchmark, not the generated production
    // route. The old unified set_language used this same default registry name.
    fn adapter_parser() -> Parser {
        Parser::from_language(&LANGUAGE, "unknown".to_string())
    }

    fn require_complete_parse(parser: &mut Parser, source: &str, assignments: usize) {
        // parse() and parse_tree_with_error_count() use the same parse_internal
        // implementation. Inspect its source ranges before timing the arena view,
        // whose Node::byte_range currently does not retain those source facts.
        let (root, errors) = parser
            .parse_tree_with_error_count(source)
            .expect("experimental adapter returned an explicit parse error");
        assert_eq!(
            errors,
            0,
            "recovery is not successful throughput: {errors} errors, root {}..{}, source {} bytes",
            root.start_byte,
            root.end_byte,
            source.len()
        );
        assert_eq!(root.start_byte, 0, "complete fixture starts at byte zero");
        assert!(
            root.end_byte >= source.trim_end().len() && root.end_byte <= source.len(),
            "root must cover every non-trivia source byte"
        );
        let assignment = parser
            .grammar()
            .find_symbol_by_name("AssignmentStatement")
            .expect("fixture declares AssignmentStatement");
        let mut pending = vec![&root];
        let mut count = 0;
        while let Some(node) = pending.pop() {
            count += usize::from(node.symbol_id == assignment);
            pending.extend(&node.children);
        }
        assert_eq!(count, assignments, "complete fixture statement count");
    }

    #[test]
    #[cfg_attr(
        feature = "pure-rust",
        ignore = "#1071/#869: adapter returns 6-error recovery at 5..5; generated 1000-line input exhausts LR work guard at byte 5733; no successful throughput receipt"
    )]
    fn test_experimental_parser_v4_performance_requires_complete_parse() {
        let mut large_code = String::new();
        for i in 0..1000 {
            large_code.push_str(&format!("var{} = {} * 2 + {}\n", i, i, i + 1));
        }

        // Keep this excluded large row first: explicitly selecting the test
        // must expose its known failure before printing any throughput. The
        // generated route's separate work-budget failure is owned by #1071;
        // switching engines or accepting recovery would not repair that row.
        let mut parser = adapter_parser();
        require_complete_parse(&mut parser, &large_code, 1000);
        let start = Instant::now();
        let iterations = 100;
        for _ in 0..iterations {
            let tree = parser.parse(&large_code).unwrap();
            assert_eq!(tree.error_count(), 0, "timed parse must remain clean");
        }
        let elapsed = start.elapsed();
        let avg_time = elapsed.as_micros() / iterations as u128;
        let mb_per_sec = (large_code.len() as f64 / 1_000_000.0) / (avg_time as f64 / 1_000_000.0);
        println!("\nExperimental parser_v4 large file (1000 lines):");
        println!(
            "Time: {}µs | Size: {} chars | Throughput: {:.1} MB/sec",
            avg_time,
            large_code.len(),
            mb_per_sec
        );

        let test_cases = vec![
            ("simple", "x = 42", 1),
            ("medium", "x = 42\ny = x + 1\nz = x * y", 3),
            (
                "complex",
                "x = 42\ny = x + 1\nz = x * y + 10\nresult = z + x * 2\nfinal = result * x + y",
                5,
            ),
        ];
        println!("\nExperimental parser_v4 small fixtures:");
        println!("Code           | Time      | Chars/sec | Lines/sec");
        println!("---------------|-----------|-----------|----------");
        for (name, code, assignments) in test_cases {
            let mut parser = adapter_parser();
            require_complete_parse(&mut parser, code, assignments);
            for _ in 0..100 {
                let tree = parser.parse(code).unwrap();
                assert_eq!(tree.error_count(), 0, "warmup must remain clean");
            }
            let iterations = 10000;
            let start = Instant::now();
            for _ in 0..iterations {
                let tree = parser.parse(code).unwrap();
                assert_eq!(tree.error_count(), 0, "timed parse must remain clean");
            }
            let elapsed = start.elapsed();
            let avg_time = elapsed.as_nanos() / iterations as u128;
            let chars_per_sec = (code.len() as f64) / (avg_time as f64 / 1_000_000_000.0);
            let lines_per_sec = (code.lines().count() as f64) / (avg_time as f64 / 1_000_000_000.0);
            println!(
                "{:14} | {:7}ns | {:9.0} | {:9.0}",
                name, avg_time, chars_per_sec, lines_per_sec
            );
        }
    }
}
