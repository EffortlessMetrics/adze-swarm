# External Scanner Integration Guide

> **Status:** Experimental advanced integration surface. External scanner
> support exists and has targeted tests, but it is not part of the Stable
> product contract. Use the
> [support-tier ledger](../status/SUPPORT_TIERS.md) for current proof commands
> and limitations.

This guide walks through integrating external scanners with adze's pure-Rust parser implementation. External scanners enable complex tokenization that cannot be expressed with regular expressions, such as indentation-sensitive parsing, heredoc strings, and context-sensitive tokenization.

The APIs shown here are the current experimental surfaces, not Stable API
commitments. Treat code blocks as implementation sketches unless the text points
to a checked test or support-tier proof command.

## Table of Contents

1. [Overview](#overview)
2. [C FFI External Scanners](#c-ffi-external-scanners)
3. [Pure-Rust External Scanners](#pure-rust-external-scanners)
4. [Integration Examples](#integration-examples)
5. [Common Patterns](#common-patterns)
6. [Debugging & Testing](#debugging--testing)

## Overview

adze supports two types of external scanners:

- **C FFI Scanners**: Use existing Tree-sitter C external scanners where the
  ABI and language fixture have explicit proof.
- **Pure-Rust Scanners**: Implement scanners directly in Rust using native traits

Both approaches integrate with the pure-Rust parser through the same runtime
interface, but scanner-heavy grammars should be treated as experimental unless
their support-tier row names a repeatable proof command.

### When to Use External Scanners

External scanners are needed for:
- **Indentation-sensitive languages** (Python, YAML)
- **Context-sensitive tokens** (JavaScript template literals)
- **Complex delimited strings** (heredoc, raw strings)
- **Comment nesting** (/* /* nested */ */)
- **Whitespace significance** (where whitespace affects parsing)

## C FFI External Scanners

### Using Existing Tree-sitter Scanners

The most direct C path is to register Tree-sitter-compatible scanner callbacks
with Adze's scanner registry. The current public FFI data type is
`adze::external_scanner_ffi::TSExternalScannerData`:

```rust
use std::ffi::c_void;
use adze::external_scanner_ffi::{
    CreateFn, DeserializeFn, DestroyFn, ScanFn, SerializeFn,
    TSExternalScannerData, TSLexer,
};
use adze::scanner_registry;
use adze::SymbolId;

// Link to existing Tree-sitter scanner
extern "C" {
    fn tree_sitter_python_external_scanner_create() -> *mut c_void;
    fn tree_sitter_python_external_scanner_destroy(scanner: *mut c_void);
    fn tree_sitter_python_external_scanner_scan(
        scanner: *mut c_void, 
        lexer: *mut TSLexer, 
        valid_symbols: *const bool
    ) -> bool;
    fn tree_sitter_python_external_scanner_serialize(
        scanner: *mut c_void, 
        buffer: *mut std::os::raw::c_char
    ) -> std::os::raw::c_uint;
    fn tree_sitter_python_external_scanner_deserialize(
        scanner: *mut c_void, 
        buffer: *const std::os::raw::c_char, 
        length: std::os::raw::c_uint
    );
}

// External scanner configuration
static PYTHON_EXTERNAL_SCANNER: TSExternalScannerData = TSExternalScannerData {
    states: std::ptr::null(),
    symbol_map: PYTHON_EXTERNAL_SYMBOL_MAP.as_ptr(),
    create: Some(tree_sitter_python_external_scanner_create as CreateFn),
    destroy: Some(tree_sitter_python_external_scanner_destroy as DestroyFn),
    scan: Some(tree_sitter_python_external_scanner_scan as ScanFn),
    serialize: Some(tree_sitter_python_external_scanner_serialize as SerializeFn),
    deserialize: Some(tree_sitter_python_external_scanner_deserialize as DeserializeFn),
};

// Symbol mapping for external tokens
static PYTHON_EXTERNAL_SYMBOL_MAP: &[u16] = &[
    0,    // unused (0-index)
    100,  // NEWLINE token
    101,  // INDENT token  
    102,  // DEDENT token
    103,  // STRING_START token
    104,  // STRING_CONTENT token
    105,  // STRING_END token
];

fn register_python_scanner() {
    let external_tokens = PYTHON_EXTERNAL_SYMBOL_MAP
        .iter()
        .copied()
        .map(SymbolId)
        .collect();

    scanner_registry::register_c_scanner(
        "python",
        PYTHON_EXTERNAL_SCANNER,
        external_tokens,
    );
}
```

C scanner support depends on matching ABI data, symbol maps, and language
fixtures. Keep grammar-specific C scanner integration behind focused proof for
that grammar.

### Build Configuration

Add the external scanner library to your `build.rs`:

```rust
// build.rs
fn main() {
    // Link to Tree-sitter external scanner
    println!("cargo:rustc-link-lib=tree-sitter-python");
    println!("cargo:rustc-link-search=native=/path/to/tree-sitter-python/lib");
    
    // Build adze parser
    adze_tool::build_parsers(&["src/grammar.rs"]).expect("Failed to build parser");
}
```

## Pure-Rust External Scanners

### Implementing the ExternalScanner Trait

For pure-Rust implementations, implement the `ExternalScanner` trait:

```rust
use adze::external_scanner::{ExternalScanner, Lexer, ScanResult};

#[derive(Default, Debug)]
struct PythonIndentationScanner {
    indent_stack: Vec<usize>,
    at_line_start: bool,
    pending_dedents: usize,
}

impl ExternalScanner for PythonIndentationScanner {
    fn scan(&mut self, lexer: &mut dyn Lexer, valid_symbols: &[bool]) -> Option<ScanResult> {
        // Symbol indices (must match grammar definition)
        const NEWLINE: usize = 1;
        const INDENT: usize = 2;
        const DEDENT: usize = 3;

        // Process pending dedents first
        if self.pending_dedents > 0 && valid_symbols.get(DEDENT).copied().unwrap_or(false) {
            self.pending_dedents -= 1;
            return Some(ScanResult { symbol: DEDENT as u16, length: 0 });
        }

        // Skip whitespace and count indentation
        let mut indent_level = 0;
        let mut found_newline = false;

        while !lexer.is_eof() {
            match lexer.lookahead() {
                Some(b'\n') => {
                    found_newline = true;
                    self.at_line_start = true;
                    lexer.advance(1);
                    lexer.mark_end();
                    
                    if valid_symbols.get(NEWLINE).copied().unwrap_or(false) {
                        return Some(ScanResult { symbol: NEWLINE as u16, length: 1 });
                    }
                }
                Some(b' ') if self.at_line_start => {
                    indent_level += 1;
                    lexer.advance(1);
                }
                Some(b'\t') if self.at_line_start => {
                    indent_level += 8; // Tab = 8 spaces
                    lexer.advance(1);
                }
                _ => break,
            }
        }

        // Process indentation changes
        if self.at_line_start && !lexer.is_eof() {
            self.at_line_start = false;
            let current_indent = self.indent_stack.last().copied().unwrap_or(0);

            if indent_level > current_indent {
                // Increased indentation
                if valid_symbols.get(INDENT).copied().unwrap_or(false) {
                    self.indent_stack.push(indent_level);
                    lexer.mark_end();
                    return Some(ScanResult { symbol: INDENT as u16, length: 0 });
                }
            } else if indent_level < current_indent {
                // Decreased indentation - may need multiple DEDENTs
                let mut dedent_count = 0;
                while let Some(&stack_level) = self.indent_stack.last() {
                    if stack_level <= indent_level {
                        break;
                    }
                    self.indent_stack.pop();
                    dedent_count += 1;
                }

                if dedent_count > 0 && valid_symbols.get(DEDENT).copied().unwrap_or(false) {
                    self.pending_dedents = dedent_count - 1; // Return first, queue rest
                    lexer.mark_end();
                    return Some(ScanResult { symbol: DEDENT as u16, length: 0 });
                }
            }
        }

        None
    }

    fn serialize(&self, buffer: &mut Vec<u8>) {
        // Serialize indent stack
        let stack_len = self.indent_stack.len() as u32;
        buffer.extend_from_slice(&stack_len.to_le_bytes());
        
        for &indent in &self.indent_stack {
            buffer.extend_from_slice(&(indent as u32).to_le_bytes());
        }
        
        buffer.push(self.at_line_start as u8);
        buffer.extend_from_slice(&(self.pending_dedents as u32).to_le_bytes());
    }

    fn deserialize(&mut self, buffer: &[u8]) {
        if buffer.len() < 4 {
            return;
        }
        
        let mut offset = 0;
        let stack_len = u32::from_le_bytes([
            buffer[offset], buffer[offset + 1], 
            buffer[offset + 2], buffer[offset + 3]
        ]) as usize;
        offset += 4;

        self.indent_stack.clear();
        for _ in 0..stack_len {
            if offset + 4 <= buffer.len() {
                let indent = u32::from_le_bytes([
                    buffer[offset], buffer[offset + 1],
                    buffer[offset + 2], buffer[offset + 3]
                ]) as usize;
                self.indent_stack.push(indent);
                offset += 4;
            }
        }

        if offset < buffer.len() {
            self.at_line_start = buffer[offset] != 0;
            offset += 1;
        }

        if offset + 4 <= buffer.len() {
            self.pending_dedents = u32::from_le_bytes([
                buffer[offset], buffer[offset + 1],
                buffer[offset + 2], buffer[offset + 3]
            ]) as usize;
        }
    }
}
```

### Registering Pure-Rust Scanners

Register scanners with the scanner registry. The current registry APIs create a
fresh scanner from a `Default` implementation:

```rust
use adze::scanner_registry::{
    ExternalScannerBuilder, register_rust_scanner,
};

fn register_python_indentation() {
    register_rust_scanner::<PythonIndentationScanner>("python_indentation");

    // Builder form for generated/setup code:
    ExternalScannerBuilder::new("python_indentation")
        .register_rust::<PythonIndentationScanner>();
}
```

## Integration Examples

### Python-like Grammar Sketch

Generated grammar integration is still experimental. The sketch below shows the
intended grammar relationship between normal tokens and external tokens; it is
not a Stable, pasteable macro contract:

```rust
#[adze::grammar("python_simple")]
mod python_grammar {
    use adze::*;

    #[adze::language]
    pub struct Program {
        statements: Vec<Statement>,
    }

    pub enum Statement {
        FunctionDef(FunctionDef),
        Simple(SimpleStatement),
    }

    pub struct FunctionDef {
        #[adze::leaf(text = "def")]
        def_keyword: (),
        
        #[adze::leaf(pattern = r"[a-zA-Z_][a-zA-Z0-9_]*")]
        name: String,
        
        #[adze::leaf(text = "(")]
        open_paren: (),
        
        #[adze::leaf(text = ")")]
        close_paren: (),
        
        #[adze::leaf(text = ":")]
        colon: (),
        
        // External tokens for indentation
        #[adze::external(symbol = "NEWLINE")]
        newline: (),
        
        #[adze::external(symbol = "INDENT")]
        indent: (),
        
        body: Block,
        
        #[adze::external(symbol = "DEDENT")]
        dedent: (),
    }

    pub struct Block {
        statements: Vec<Statement>,
    }

    pub struct SimpleStatement {
        #[adze::leaf(pattern = r"[a-zA-Z_][a-zA-Z0-9_]*")]
        name: String,
    }

    // Register the scanner separately with `scanner_registry` using the
    // generated language name and a scanner type that implements
    // `adze::external_scanner::ExternalScanner + Default`.
}
```

### JavaScript Template Literals

External scanner for JavaScript template literal parsing:

```rust
#[derive(Default)]
struct TemplateScanner {
    brace_depth: usize,
    in_template: bool,
}

impl ExternalScanner for TemplateScanner {
    fn scan(&mut self, lexer: &mut dyn Lexer, valid_symbols: &[bool]) -> Option<ScanResult> {
        const TEMPLATE_START: usize = 1;      // `
        const TEMPLATE_CONTENT: usize = 2;    // template content
        const TEMPLATE_MIDDLE: usize = 3;     // }...${
        const TEMPLATE_END: usize = 4;        // `
        
        if valid_symbols.get(TEMPLATE_START).copied().unwrap_or(false) {
            if lexer.lookahead() == Some(b'`') {
                lexer.advance(1);
                lexer.mark_end();
                self.in_template = true;
                return Some(ScanResult { symbol: TEMPLATE_START as u16, length: 1 });
            }
        }

        if self.in_template && valid_symbols.get(TEMPLATE_CONTENT).copied().unwrap_or(false) {
            let mut content_length = 0;
            
            while let Some(ch) = lexer.lookahead() {
                match ch {
                    b'`' => {
                        // End of template
                        if content_length > 0 {
                            lexer.mark_end();
                            return Some(ScanResult { 
                                symbol: TEMPLATE_CONTENT as u16, 
                                length: content_length 
                            });
                        }
                        break;
                    }
                    b'$' => {
                        // Check for ${ expression start
                        lexer.advance(1);
                        if lexer.lookahead() == Some(b'{') {
                            if content_length > 0 {
                                // Return content before ${
                                return Some(ScanResult { 
                                    symbol: TEMPLATE_CONTENT as u16, 
                                    length: content_length 
                                });
                            }
                            // This is start of expression - parser will handle
                            break;
                        }
                        content_length += 1;
                    }
                    b'\\' => {
                        // Escape sequence
                        lexer.advance(1);
                        content_length += 1;
                        if !lexer.is_eof() {
                            lexer.advance(1);
                            content_length += 1;
                        }
                    }
                    _ => {
                        lexer.advance(1);
                        content_length += 1;
                    }
                }
            }
        }

        None
    }

    fn serialize(&self, buffer: &mut Vec<u8>) {
        buffer.extend_from_slice(&(self.brace_depth as u32).to_le_bytes());
        buffer.push(self.in_template as u8);
    }

    fn deserialize(&mut self, buffer: &[u8]) {
        if buffer.len() >= 4 {
            self.brace_depth = u32::from_le_bytes([
                buffer[0], buffer[1], buffer[2], buffer[3]
            ]) as usize;
        }
        if buffer.len() >= 5 {
            self.in_template = buffer[4] != 0;
        }
    }
}
```

## Common Patterns

### State Machine Pattern

For complex tokenization, use state machines:

```rust
#[derive(Default)]
struct StateMachineScanner {
    state: ScannerState,
    // ... other state ...
}

#[derive(Default)]
enum ScannerState {
    #[default]
    Normal,
    InString { quote_type: u8 },
    InComment { nesting: usize },
    InHeredoc { delimiter: String },
}

impl ExternalScanner for StateMachineScanner {
    fn scan(&mut self, lexer: &mut dyn Lexer, valid_symbols: &[bool]) -> Option<ScanResult> {
        match &mut self.state {
            ScannerState::Normal => self.scan_normal(lexer, valid_symbols),
            ScannerState::InString { quote_type } => self.scan_string(lexer, *quote_type),
            ScannerState::InComment { nesting } => self.scan_comment(lexer, nesting),
            ScannerState::InHeredoc { delimiter } => self.scan_heredoc(lexer, delimiter),
        }
    }
    
    // ... implement state-specific scanning methods ...
}
```

### Token Lookahead

Use lookahead to disambiguate tokens:

```rust
impl ExternalScanner for LookaheadScanner {
    fn scan(&mut self, lexer: &mut dyn Lexer, valid_symbols: &[bool]) -> Option<ScanResult> {
        // Look ahead to determine token type
        let mut lookahead_pos = 0;
        while let Some(ch) = lexer.lookahead() {
            match (lookahead_pos, ch) {
                (0, b'<') => lookahead_pos += 1,
                (1, b'<') => lookahead_pos += 1,
                (2, b'-') => {
                    // Found <<- heredoc
                    return self.scan_heredoc_with_indent(lexer, valid_symbols);
                }
                (2, _) if ch.is_ascii_alphabetic() => {
                    // Found << heredoc
                    return self.scan_heredoc(lexer, valid_symbols);
                }
                _ => break,
            }
            lexer.advance(1);
        }
        None
    }
}
```

## Debugging & Testing

### Debugging External Scanners

Use debug logging to trace scanner behavior:

```rust
impl ExternalScanner for DebugScanner {
    fn scan(&mut self, lexer: &mut dyn Lexer, valid_symbols: &[bool]) -> Option<ScanResult> {
        eprintln!("Scanner called with valid_symbols: {:?}", valid_symbols);
        
        if let Some(result) = self.inner_scan(lexer, valid_symbols) {
            eprintln!("Scanner returning: symbol={}, length={}", result.symbol, result.length);
            Some(result)
        } else {
            eprintln!("Scanner returning None");
            None
        }
    }
}
```

### Testing External Scanners

Create unit tests for scanner logic:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    struct TestLexer<'a> {
        input: &'a [u8],
        position: usize,
    }

    impl<'a> TestLexer<'a> {
        fn new(input: &'a str) -> Self {
            Self {
                input: input.as_bytes(),
                position: 0,
            }
        }
    }

    impl<'a> Lexer for TestLexer<'a> {
        fn lookahead(&self) -> Option<u8> {
            self.input.get(self.position).copied()
        }

        fn advance(&mut self, n: usize) {
            self.position = (self.position + n).min(self.input.len());
        }

        fn mark_end(&mut self) {}

        fn column(&self) -> usize {
            self.position
        }

        fn is_eof(&self) -> bool {
            self.position >= self.input.len()
        }
    }

    #[test]
    fn test_indentation_scanner() {
        let mut scanner = PythonIndentationScanner::default();
        let mut lexer = TestLexer::new("    def foo():\n        return 42\n");
        
        // Should recognize indentation
        let valid_symbols = [false, false, true, false]; // INDENT valid
        let result = scanner.scan(&mut lexer, &valid_symbols);
        
        assert_eq!(result, Some(ScanResult { symbol: 2, length: 0 }));
    }

    #[test]
    fn test_dedentation() {
        let mut scanner = PythonIndentationScanner::default();
        scanner.indent_stack.push(4); // Simulate existing indentation
        
        let mut lexer = TestLexer::new("def bar():\n");
        
        let valid_symbols = [false, false, false, true]; // DEDENT valid  
        let result = scanner.scan(&mut lexer, &valid_symbols);
        
        assert_eq!(result, Some(ScanResult { symbol: 3, length: 0 }));
    }

    #[test]
    fn test_serialization() {
        let mut scanner = PythonIndentationScanner::default();
        scanner.indent_stack.push(4);
        scanner.indent_stack.push(8);
        scanner.at_line_start = true;
        
        let mut buffer = Vec::new();
        scanner.serialize(&mut buffer);
        
        let mut new_scanner = PythonIndentationScanner::default();
        new_scanner.deserialize(&buffer);
        
        assert_eq!(new_scanner.indent_stack, vec![4, 8]);
        assert_eq!(new_scanner.at_line_start, true);
    }
}
```

### Integration Testing

Use the support-tier commands as the current integration receipts:

```bash
cargo test -p adze --features external_scanners
cargo test -p adze --features "pure-rust,external_scanners" parser_v4::tests::test_parser_with_external_scanner -- --exact --nocapture
cargo test -p adze --features "pure-rust,external_scanners" parser_v4::tests::test_external_scanner_rejects_token_not_in_valid_symbols -- --exact --nocapture
cargo test -p adze --features "pure-rust,external_scanners" parser_v4::tests::test_external_scanner_parse_document_bad_input_returns_diagnostic_document -- --exact --nocapture
cargo test --manifest-path example/Cargo.toml external_word_example::tests::generated_external_grammar_bad_input_matrix_returns_diagnostic_document --features pure-rust -- --exact --nocapture
```

For source examples, start with
[`runtime/src/external_scanner/tests.rs`](../../runtime/src/external_scanner/tests.rs),
[`runtime/tests/test_python_scanner.rs`](../../runtime/tests/test_python_scanner.rs),
and the parser-v4 external scanner canaries named above.

## Performance Considerations

### Minimize Lexer State

Keep scanner state minimal for better performance:

```rust
// Good: minimal state
#[derive(Default)]
struct EfficientScanner {
    mode: u8,  // Single byte for mode
    counter: u16,  // Small counter
}

// Less efficient: large state
struct HeavyScanner {
    full_stack: Vec<ComplexState>,
    history: HashMap<String, Vec<Token>>,
}
```

### Avoid Excessive Lookahead

Limit lookahead to prevent performance issues:

```rust
impl ExternalScanner for BoundedScanner {
    fn scan(&mut self, lexer: &mut dyn Lexer, valid_symbols: &[bool]) -> Option<ScanResult> {
        const MAX_LOOKAHEAD: usize = 100;
        let mut lookahead = 0;
        
        while lookahead < MAX_LOOKAHEAD && !lexer.is_eof() {
            // Process character
            lexer.advance(1);
            lookahead += 1;
        }
        
        None
    }
}
```

## Conclusion

External scanners provide powerful capabilities for handling complex
tokenization requirements that exceed the capabilities of regular expressions.
The pure-Rust implementation exposes both C FFI and native Rust scanner paths,
but both remain experimental unless a support-tier row promotes a narrower
slice with proof.

For questions or issues with external scanner integration, check the runtime
scanner tests linked above for working examples or consult the
[Parser Cookbook external scanner notes](../reference/parser-cookbook.md#external-scanners).
