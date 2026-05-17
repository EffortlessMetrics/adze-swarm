# Error Recovery

Error recovery in Adze enables robust parsing of malformed or partially
complete code. This guide covers the current product-shaped path: safe span
helpers, generated parser diagnostics, document facts, and support-tiered GLR
or incremental behavior.

## Overview

Adze provides multiple layers of error recovery:

1. **Span Error Recovery** (PR #55) - Safe span operations with comprehensive validation
2. **Generated Parser Diagnostics** - Graceful handling of syntax errors during parsing
3. **Document Diagnostics** - Structured byte/point ranges and expected-token data
4. **Incremental Lifecycle Metadata** - Honest full-reparse fallback before stable reuse claims
5. **GLR Ambiguity Summaries** - Documented selected-tree and ambiguity facts for covered grammar shapes

## Span Error Recovery

### The SpanError System

The `SpanError` system provides comprehensive error handling for span-based operations, eliminating panic-prone indexing that can crash parsers when working with malformed input.

```rust
use adze::{Spanned, SpanError};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SpanError {
    /// The span start index is greater than the span end index
    InvalidRange { start: usize, end: usize },
    /// The span extends beyond the bounds of the target string or buffer  
    OutOfBounds { span: (usize, usize), length: usize },
}
```

### Safe Span Operations

All span operations now provide safe alternatives that return `Result` instead of panicking:

```rust
// Validate spans before use
let span = Spanned::new("identifier", (10, 20));
match span.validate_for_str(source_code) {
    Ok(()) => {
        // Safe to proceed with span operations
        let text = span.try_slice_str(source_code)?;
        println!("Extracted: {}", text);
    },
    Err(SpanError::InvalidRange { start, end }) => {
        eprintln!("Invalid span: start {} > end {}", start, end);
        // Handle malformed span gracefully
    },
    Err(SpanError::OutOfBounds { span, length }) => {
        eprintln!("Span {:?} exceeds source length {}", span, length);
        // Handle truncated input gracefully  
    }
}
```

### Error Recovery Patterns

#### Pattern 1: Defensive Span Extraction
```rust
fn safe_extract_token(source: &str, span: &Spanned<()>) -> Option<String> {
    match span.try_slice_str(source) {
        Ok(text) => Some(text.to_string()),
        Err(SpanError::OutOfBounds { .. }) => {
            // Input was truncated, extract what we can
            if span.span.0 < source.len() {
                Some(source[span.span.0..].to_string())
            } else {
                None
            }
        },
        Err(SpanError::InvalidRange { .. }) => {
            // Malformed span, skip this token
            None
        }
    }
}
```

#### Pattern 2: Graceful Mutable Operations
```rust
fn safe_rename_identifier(
    source: &mut String, 
    span: &Spanned<()>, 
    new_name: &str
) -> Result<(), String> {
    match span.try_slice_str_mut(source) {
        Ok(identifier) => {
            // Safely replace the identifier
            let start = span.span.0;
            let end = span.span.1;
            source.replace_range(start..end, new_name);
            Ok(())
        },
        Err(e) => Err(format!("Cannot rename identifier: {}", e))
    }
}
```

#### Pattern 3: Error Recovery in Batch Operations
```rust
fn process_spans_with_recovery(
    source: &str, 
    spans: &[Spanned<String>]
) -> (Vec<String>, Vec<SpanError>) {
    let mut results = Vec::new();
    let mut errors = Vec::new();
    
    for span in spans {
        match span.try_slice_str(source) {
            Ok(text) => results.push(text.to_string()),
            Err(e) => {
                errors.push(e);
                // Continue processing other spans
            }
        }
    }
    
    (results, errors)
}
```

## Parser Error Recovery

### Basic Error Handling

Generated parsers expose the stable typed path through `grammar::parse()` and
the document-oriented diagnostic path through `grammar::parse_document()`.

```rust
let typed = grammar::parse("1 + 2");
assert!(typed.is_ok());

let report = grammar::parse_document("1 +");
if !report.diagnostics().is_empty() {
    for diagnostic in report.diagnostics() {
        eprintln!(
            "{} at {:?}",
            diagnostic.message,
            diagnostic.byte_span()
        );
    }
}
```

`parse()` remains the ergonomic typed-AST front door. Use `parse_document()`
when tooling needs diagnostics, source ranges, selected-tree facts, or JSON and
Tree-sitter-compatible projections.

### Error Recovery Strategies

#### Strategy 1: Diagnostic Document Recovery
```rust
let source = "1 +";
let report = grammar::parse_document(source);

for diagnostic in report.diagnostics() {
    assert!(diagnostic.byte_span().start <= diagnostic.byte_span().end);
}
```

#### Strategy 2: Typed AST Fallback
```rust
let source = "1 +";
match grammar::parse(source) {
    Ok(ast) => {
        println!("typed AST: {ast:?}");
    }
    Err(_) => {
        let report = grammar::parse_document(source);
        eprintln!("diagnostics: {:?}", report.diagnostics());
    }
}
```

Typed AST lowering may reject recovered syntax by default. That is intentional:
the document can preserve diagnostics and selected-tree facts even when the
typed semantic value is not trustworthy.

## Incremental Error Recovery

Incremental parsing is experimental. The supported guidance is to make fallback
visible instead of claiming stable subtree reuse.

```rust
use adze::document::IncrementalFallbackReason;

let document = grammar::parse_document("1 +")
    .document()
    .with_full_reparse_fallback_metadata(IncrementalFallbackReason::FullReparseOnly);

assert!(document.metadata().incremental_requested);
if document.metadata().full_reparse_fallback() {
    eprintln!("incremental reuse was requested, but this parse used a full reparse");
}
```

If an editor integration needs changed ranges, treat them as conservative until
the incremental document lifecycle is promoted in `docs/status/SUPPORT_TIERS.md`.

### Edit Repair Inputs
```rust
fn clamp_edit_range(start: usize, old_end: usize, source_len: usize) -> (usize, usize) {
    let start = start.min(source_len);
    let old_end = old_end.min(source_len).max(start);
    (start, old_end)
}
```

Until incremental reuse is promoted, validate edit offsets before handing them
to an editor integration and keep fallback metadata visible in the resulting
document.

## GLR Error Recovery

GLR parsers handle documented ambiguous grammar classes. Native Adze output
keeps the selected tree and ambiguity summary on `AdzeDocument`; full forest
export is not the default stable user contract.

### Ambiguity Resolution

```rust
let report = grammar::parse_document(ambiguous_source);
let document = report.document();

for ambiguity in document.ambiguities() {
    if let Some(selected) = ambiguity.selected {
        println!("selected alternative: {selected:?}");
    }
    println!("retained alternatives: {}", ambiguity.alternatives.len());
}
```

### Error Forest Analysis

```rust
let report = grammar::parse_document(source);

for diagnostic in report.diagnostics() {
    eprintln!("{diagnostic:?}");
}

let root = report.document().tree().root();
assert!(root.byte_range().start <= root.byte_range().end);
```

Use diagnostics and ambiguity summaries as the public facts. Raw forest
inspection remains future/experimental unless a support-tier row promotes a
specific API and proof command.

## Testing Error Recovery

### Unit Testing Error Conditions

```rust
#[cfg(test)]
mod error_recovery_tests {
    use super::*;

    #[test]
    fn test_span_out_of_bounds_recovery() {
        let source = "hello";
        let span = Spanned::new((), (0, 10)); // Extends beyond source
        
        match span.try_slice_str(source) {
            Err(SpanError::OutOfBounds { span, length }) => {
                assert_eq!(span, (0, 10));
                assert_eq!(length, 5);
            },
            _ => panic!("Expected OutOfBounds error"),
        }
    }
    
    #[test]
    fn test_invalid_range_recovery() {
        let source = "hello world";
        let span = Spanned::new((), (5, 3)); // start > end
        
        match span.validate_for_str(source) {
            Err(SpanError::InvalidRange { start, end }) => {
                assert_eq!(start, 5);
                assert_eq!(end, 3);
            },
            _ => panic!("Expected InvalidRange error"),
        }
    }
}
```

### Integration Testing

```rust
#[test]
fn test_malformed_input_recovery() {
    let malformed_inputs = vec![
        "1 +",        // Unexpected EOF
        "1 + + 2",    // Unexpected operator
        "é +",        // Multibyte bad-token span
        "",           // Empty input
    ];

    for input in malformed_inputs {
        let report = grammar::parse_document(input);

        // Bad input should produce diagnostics or a hard failure, never a panic.
        assert!(!report.diagnostics().is_empty(), "expected diagnostics for {input:?}");
    }
}
```

## Best Practices

### 1. Always Use Safe Span Operations

```rust
// ❌ Panic-prone 
let text = &source[span.0..span.1];

// ✅ Safe with error handling
let text = match span.try_slice_str(source) {
    Ok(text) => text,
    Err(e) => {
        eprintln!("Failed to extract span: {}", e);
        return Err("Invalid span".into());
    }
};
```

### 2. Validate Edits Before Applying

```rust
// ✅ Validate edit before use
fn validate_edit_range(start: usize, old_end: usize, source_len: usize) -> bool {
    start <= old_end && old_end <= source_len
}
```

### 3. Implement Progressive Recovery

```rust
fn parse_for_user_feedback(source: &str) {
    match grammar::parse(source) {
        Ok(ast) => println!("parsed: {ast:?}"),
        Err(_) => {
            let report = grammar::parse_document(source);
            for diagnostic in report.diagnostics() {
                eprintln!("{diagnostic:?}");
            }
        }
    }
}
```

### 4. Provide Rich Error Information

```rust
#[derive(Debug)]
struct DetailedParseError {
    message: String,
    byte_range: std::ops::Range<usize>,
    suggestions: Vec<String>,
}

impl DetailedParseError {
    fn with_suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestions.push(suggestion.into());
        self
    }
}
```

## Performance Considerations

Error recovery adds some overhead, so keep measurements tied to the exact
surface being proven:

- **Lazy Validation**: Spans are only validated when accessed
- **Zero-Cost Abstractions**: No overhead when not using error recovery features
- **Document Diagnostics**: Diagnostics are structured facts that can be
  rendered or serialized without reparsing
- **GLR Summaries**: Ambiguity summaries expose selected-tree facts without
  making raw forest export the default user surface

Monitor performance using the built-in instrumentation:

```rust
// Enable performance logging
std::env::set_var("ADZE_LOG_PERFORMANCE", "true");

// Monitor error recovery overhead
let start = std::time::Instant::now();
let report = grammar::parse_document(large_malformed_input);
let duration = start.elapsed();

eprintln!("diagnostics: {}", report.diagnostics().len());
println!("Recovery parse took {:?}", duration);
```

## Conclusion

Adze's error recovery system provides multiple layers of protection against malformed input:

1. **SpanError system** prevents panics and provides detailed error information
2. **Safe span operations** allow graceful handling of invalid ranges
3. **Document diagnostics** expose byte ranges, point ranges, expected tokens,
   and related parse facts when available
4. **Incremental lifecycle metadata** makes fallback visible instead of
   implying stable reuse
5. **GLR ambiguity summaries** report documented ambiguity facts without
   promoting raw forest APIs

By following the patterns and best practices in this guide, you can build
parsers that handle malformed input clearly while keeping stable, stabilizing,
and experimental claims separated.
