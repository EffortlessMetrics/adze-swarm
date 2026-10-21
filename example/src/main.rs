use std::io::Write;

use adze::errors::{ParseError, ParseErrorReason};
use codemap::CodeMap;
use codemap_diagnostic::{ColorConfig, Diagnostic, Emitter, Level, SpanLabel, SpanStyle};

// Import modules from the lib instead of compiling them locally
use adze_example::{
    ambiguous, arithmetic, external_word_example, optionals, performance_test, repetitions,
    test_precedence, test_whitespace, words,
};

fn convert_parse_error_to_diagnostics(
    file_span: &codemap::Span,
    error: &ParseError,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match &error.reason {
        ParseErrorReason::MissingToken(tok) => diagnostics.push(Diagnostic {
            level: Level::Error,
            message: format!("Missing token: \"{tok}\""),
            code: Some("S000".to_string()),
            spans: vec![SpanLabel {
                span: file_span.subspan(error.start as u64, error.end as u64),
                style: SpanStyle::Primary,
                label: Some(format!("missing \"{tok}\"")),
            }],
        }),

        ParseErrorReason::UnexpectedToken(tok) => diagnostics.push(Diagnostic {
            level: Level::Error,
            message: format!("Unexpected token: \"{tok}\""),
            code: Some("S000".to_string()),
            spans: vec![SpanLabel {
                span: file_span.subspan(error.start as u64, error.end as u64),
                style: SpanStyle::Primary,
                label: Some(format!("unexpected \"{tok}\"")),
            }],
        }),

        ParseErrorReason::UnexpectedEof => diagnostics.push(Diagnostic {
            level: Level::Error,
            message: "Unexpected end of input".to_string(),
            code: Some("S000".to_string()),
            spans: vec![SpanLabel {
                span: file_span.subspan(error.start as u64, error.end as u64),
                style: SpanStyle::Primary,
                label: Some("end of input".to_string()),
            }],
        }),

        ParseErrorReason::FailedNode(errors) => {
            if errors.is_empty() {
                diagnostics.push(Diagnostic {
                    level: Level::Error,
                    message: "Failed to parse node".to_string(),
                    code: Some("S000".to_string()),
                    spans: vec![SpanLabel {
                        span: file_span.subspan(error.start as u64, error.end as u64),
                        style: SpanStyle::Primary,
                        label: Some("failed".to_string()),
                    }],
                })
            } else {
                for error in errors {
                    convert_parse_error_to_diagnostics(file_span, error, diagnostics);
                }
            }
        }

        _ => diagnostics.push(Diagnostic {
            level: Level::Error,
            message: error.to_string(),
            code: Some("S000".to_string()),
            spans: vec![SpanLabel {
                span: file_span.subspan(error.start as u64, error.end as u64),
                style: SpanStyle::Primary,
                label: Some("parse error".to_string()),
            }],
        }),
    }
}

fn main() {
    // Run whitespace tests first
    test_whitespace::test_whitespace_parsing();
    println!("\n---\n");

    // Run precedence tests
    test_precedence::test_precedence();
    println!("\n---\n");

    let stdin = std::io::stdin();

    loop {
        print!("> ");
        std::io::stdout().flush().unwrap();

        let mut input = String::new();
        stdin.read_line(&mut input).unwrap();
        let input = input.trim();
        if input.is_empty() {
            break;
        }

        match arithmetic::grammar::parse(input) {
            Ok(expr) => println!("{expr:?}"),
            Err(errs) => {
                let mut codemap = CodeMap::new();
                let file_span = codemap.add_file("<input>".to_string(), input.to_string());
                let mut diagnostics = vec![];
                for error in errs {
                    convert_parse_error_to_diagnostics(&file_span.span, &error, &mut diagnostics);
                }

                let mut emitter = Emitter::stderr(ColorConfig::Always, Some(&codemap));
                emitter.emit(&diagnostics);
            }
        };
    }
}
