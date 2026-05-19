# Adze Usage Examples

> **Doc status:** User-facing examples should start with generated
> `grammar::parse()` and move to generated `grammar::parse_document()` only
> when tooling needs document facts. Lower-level parser and Tree-sitter-style
> APIs are implementation or compatibility surfaces unless their
> support-tier rows say otherwise.

This document shows common generated-parser usage. The stable front door is a
Rust grammar module whose generated `parse()` function returns typed Rust
values. The tooling path is `parse_document()`, which returns the native
`AdzeDocument` parse product for diagnostics, syntax inspection, ambiguity
summaries, JSON, and Tree-sitter-compatible selected-tree projections.

## Table of Contents
1. [Basic Grammar Definition](#basic-grammar-definition)
2. [Simple Expression Parser](#simple-expression-parser)
3. [JSON Parser](#json-parser)
4. [Programming Language Parser](#programming-language-parser)
5. [Error Handling](#error-handling)
6. [Tree Traversal](#tree-traversal)
7. [Grammar Analysis](#grammar-analysis)
8. [Custom Transformations](#custom-transformations)
9. [Performance Optimization](#performance-optimization)
10. [Integration Examples](#integration-examples)

## Basic Grammar Definition

### Simple Calculator

```rust
#[adze::grammar("calculator")]
pub mod grammar {
    #[adze::language]
    pub enum Expression {
        Number(
            #[adze::leaf(pattern = r"-?\d+(\.\d+)?", transform = |v| v.parse().unwrap())]
            f64
        ),
        #[adze::prec_left(1)]
        Add(
            Box<Expression>,
            #[adze::leaf(text = "+")]
            (),
            Box<Expression>
        ),
        #[adze::prec_left(1)]
        Subtract(
            Box<Expression>,
            #[adze::leaf(text = "-")]
            (),
            Box<Expression>
        ),
        #[adze::prec_left(2)]
        Multiply(
            Box<Expression>,
            #[adze::leaf(text = "*")]
            (),
            Box<Expression>
        ),
        #[adze::prec_left(2)]
        Divide(
            Box<Expression>,
            #[adze::leaf(text = "/")]
            (),
            Box<Expression>
        ),
        #[adze::prec(3)]
        Parenthesized(
            #[adze::leaf(text = "(")]
            (),
            Box<Expression>,
            #[adze::leaf(text = ")")]
            ()
        ),
    }

    #[adze::extra]
    struct Whitespace {
        #[adze::leaf(pattern = r"\s+")]
        _ws: (),
    }
}

// Usage
fn main() {
    let input = "2 + 3 * (4 - 1)";
    match grammar::parse(input) {
        Ok(expr) => println!("Parsed: {:?}", expr),
        Err(e) => eprintln!("Parse error: {:?}", e),
    }
}
```

## Simple Expression Parser

### Boolean Expression Grammar

```rust
#[adze::grammar("boolean")]
pub mod bool_grammar {
    #[adze::language]
    pub enum BoolExpr {
        True(#[adze::leaf(text = "true")] ()),
        False(#[adze::leaf(text = "false")] ()),
        
        #[adze::prec_left(1)]
        Or(
            Box<BoolExpr>,
            #[adze::leaf(text = "||")]
            (),
            Box<BoolExpr>
        ),
        
        #[adze::prec_left(2)]
        And(
            Box<BoolExpr>,
            #[adze::leaf(text = "&&")]
            (),
            Box<BoolExpr>
        ),
        
        #[adze::prec(3)]
        Not(
            #[adze::leaf(text = "!")]
            (),
            Box<BoolExpr>
        ),
        
        Variable(
            #[adze::leaf(pattern = r"[a-zA-Z_]\w*")]
            String
        ),
    }

    #[adze::extra]
    struct Whitespace {
        #[adze::leaf(pattern = r"\s+")]
        _ws: (),
    }
}

// Evaluator
impl bool_grammar::BoolExpr {
    fn evaluate(&self, vars: &HashMap<String, bool>) -> bool {
        use bool_grammar::BoolExpr::*;
        match self {
            True(_) => true,
            False(_) => false,
            Or(left, _, right) => left.evaluate(vars) || right.evaluate(vars),
            And(left, _, right) => left.evaluate(vars) && right.evaluate(vars),
            Not(_, expr) => !expr.evaluate(vars),
            Variable(name) => *vars.get(name).unwrap_or(&false),
        }
    }
}
```

## JSON Parser

### Complete JSON Grammar

```rust
#[adze::grammar("json")]
pub mod json_grammar {
    use std::collections::HashMap;

    #[adze::language]
    pub enum Value {
        Null(#[adze::leaf(text = "null")] ()),
        Bool(Bool),
        Number(Number),
        String(String),
        Array(Array),
        Object(Object),
    }

    pub enum Bool {
        #[adze::leaf(text = "true")]
        True,
        #[adze::leaf(text = "false")]
        False,
    }

    #[adze::leaf(pattern = r"-?(0|[1-9]\d*)(\.\d+)?([eE][+-]?\d+)?")]
    pub struct Number(pub f64);

    impl adze::Deserialize for Number {
        fn deserialize(s: &str) -> Self {
            Number(s.parse().unwrap())
        }
    }

    #[adze::leaf(pattern = r#""([^"\\]|\\.)*""#, transform = parse_string)]
    pub struct String(pub std::string::String);

    fn parse_string(s: &str) -> std::string::String {
        // Remove quotes and handle escapes
        let content = &s[1..s.len()-1];
        content.replace("\\\"", "\"")
            .replace("\\\\", "\\")
            .replace("\\n", "\n")
            .replace("\\r", "\r")
            .replace("\\t", "\t")
    }

    pub struct Array {
        #[adze::leaf(text = "[")]
        _open: (),
        #[adze::repeat(non_empty = false)]
        #[adze::delimited(
            #[adze::leaf(text = ",")]
            ()
        )]
        pub elements: Vec<Value>,
        #[adze::leaf(text = "]")]
        _close: (),
    }

    pub struct Object {
        #[adze::leaf(text = "{")]
        _open: (),
        #[adze::repeat(non_empty = false)]
        #[adze::delimited(
            #[adze::leaf(text = ",")]
            ()
        )]
        pub members: Vec<Member>,
        #[adze::leaf(text = "}")]
        _close: (),
    }

    pub struct Member {
        pub key: String,
        #[adze::leaf(text = ":")]
        _colon: (),
        pub value: Value,
    }

    #[adze::extra]
    struct Whitespace {
        #[adze::leaf(pattern = r"[ \t\n\r]+")]
        _ws: (),
    }
}

// Convert to standard Rust types
impl json_grammar::Value {
    fn to_rust_value(&self) -> serde_json::Value {
        use json_grammar::Value::*;
        match self {
            Null(_) => serde_json::Value::Null,
            Bool(b) => match b {
                json_grammar::Bool::True => serde_json::Value::Bool(true),
                json_grammar::Bool::False => serde_json::Value::Bool(false),
            },
            Number(n) => serde_json::Value::Number(
                serde_json::Number::from_f64(n.0).unwrap()
            ),
            String(s) => serde_json::Value::String(s.0.clone()),
            Array(a) => serde_json::Value::Array(
                a.elements.iter().map(|v| v.to_rust_value()).collect()
            ),
            Object(o) => {
                let mut map = serde_json::Map::new();
                for member in &o.members {
                    map.insert(member.key.0.clone(), member.value.to_rust_value());
                }
                serde_json::Value::Object(map)
            }
        }
    }
}
```

## Programming Language Parser

### Mini C-like Language

```rust
#[adze::grammar("mini_c")]
pub mod mini_c {
    #[adze::language]
    pub struct Program {
        #[adze::repeat(non_empty = false)]
        pub declarations: Vec<Declaration>,
    }

    pub enum Declaration {
        Function(Function),
        Variable(VariableDecl),
    }

    pub struct Function {
        pub return_type: Type,
        pub name: Identifier,
        #[adze::leaf(text = "(")]
        _open: (),
        #[adze::repeat(non_empty = false)]
        #[adze::delimited(
            #[adze::leaf(text = ",")]
            ()
        )]
        pub parameters: Vec<Parameter>,
        #[adze::leaf(text = ")")]
        _close: (),
        pub body: Block,
    }

    pub struct Parameter {
        pub param_type: Type,
        pub name: Identifier,
    }

    pub struct VariableDecl {
        pub var_type: Type,
        pub name: Identifier,
        pub init: Option<Initializer>,
        #[adze::leaf(text = ";")]
        _semi: (),
    }

    pub struct Initializer {
        #[adze::leaf(text = "=")]
        _eq: (),
        pub value: Expression,
    }

    pub enum Type {
        #[adze::leaf(text = "int")]
        Int,
        #[adze::leaf(text = "float")]
        Float,
        #[adze::leaf(text = "bool")]
        Bool,
        #[adze::leaf(text = "void")]
        Void,
    }

    pub struct Block {
        #[adze::leaf(text = "{")]
        _open: (),
        #[adze::repeat(non_empty = false)]
        pub statements: Vec<Statement>,
        #[adze::leaf(text = "}")]
        _close: (),
    }

    pub enum Statement {
        Expression(ExpressionStmt),
        Return(ReturnStmt),
        If(IfStmt),
        While(WhileStmt),
        Block(Block),
        Variable(VariableDecl),
    }

    pub struct ExpressionStmt {
        pub expr: Expression,
        #[adze::leaf(text = ";")]
        _semi: (),
    }

    pub struct ReturnStmt {
        #[adze::leaf(text = "return")]
        _return: (),
        pub value: Option<Expression>,
        #[adze::leaf(text = ";")]
        _semi: (),
    }

    pub struct IfStmt {
        #[adze::leaf(text = "if")]
        _if: (),
        #[adze::leaf(text = "(")]
        _open: (),
        pub condition: Expression,
        #[adze::leaf(text = ")")]
        _close: (),
        pub then_stmt: Box<Statement>,
        pub else_stmt: Option<ElseClause>,
    }

    pub struct ElseClause {
        #[adze::leaf(text = "else")]
        _else: (),
        pub stmt: Box<Statement>,
    }

    pub struct WhileStmt {
        #[adze::leaf(text = "while")]
        _while: (),
        #[adze::leaf(text = "(")]
        _open: (),
        pub condition: Expression,
        #[adze::leaf(text = ")")]
        _close: (),
        pub body: Box<Statement>,
    }

    #[adze::prec]
    pub enum Expression {
        #[adze::prec_left(1)]
        Assign(Box<Expression>, #[adze::leaf(text = "=")] (), Box<Expression>),
        
        #[adze::prec_left(2)]
        LogicalOr(Box<Expression>, #[adze::leaf(text = "||")] (), Box<Expression>),
        
        #[adze::prec_left(3)]
        LogicalAnd(Box<Expression>, #[adze::leaf(text = "&&")] (), Box<Expression>),
        
        #[adze::prec_left(4)]
        Equal(Box<Expression>, #[adze::leaf(text = "==")] (), Box<Expression>),
        #[adze::prec_left(4)]
        NotEqual(Box<Expression>, #[adze::leaf(text = "!=")] (), Box<Expression>),
        
        #[adze::prec_left(5)]
        Less(Box<Expression>, #[adze::leaf(text = "<")] (), Box<Expression>),
        #[adze::prec_left(5)]
        Greater(Box<Expression>, #[adze::leaf(text = ">")] (), Box<Expression>),
        
        #[adze::prec_left(6)]
        Add(Box<Expression>, #[adze::leaf(text = "+")] (), Box<Expression>),
        #[adze::prec_left(6)]
        Subtract(Box<Expression>, #[adze::leaf(text = "-")] (), Box<Expression>),
        
        #[adze::prec_left(7)]
        Multiply(Box<Expression>, #[adze::leaf(text = "*")] (), Box<Expression>),
        #[adze::prec_left(7)]
        Divide(Box<Expression>, #[adze::leaf(text = "/")] (), Box<Expression>),
        
        #[adze::prec(8)]
        UnaryMinus(#[adze::leaf(text = "-")] (), Box<Expression>),
        #[adze::prec(8)]
        Not(#[adze::leaf(text = "!")] (), Box<Expression>),
        
        #[adze::prec(9)]
        Call(Box<Expression>, ArgumentList),
        
        #[adze::prec(10)]
        Primary(Primary),
    }

    pub struct ArgumentList {
        #[adze::leaf(text = "(")]
        _open: (),
        #[adze::repeat(non_empty = false)]
        #[adze::delimited(
            #[adze::leaf(text = ",")]
            ()
        )]
        pub args: Vec<Expression>,
        #[adze::leaf(text = ")")]
        _close: (),
    }

    pub enum Primary {
        Identifier(Identifier),
        Number(Number),
        True(#[adze::leaf(text = "true")] ()),
        False(#[adze::leaf(text = "false")] ()),
        Parenthesized(ParenExpr),
    }

    pub struct ParenExpr {
        #[adze::leaf(text = "(")]
        _open: (),
        pub expr: Box<Expression>,
        #[adze::leaf(text = ")")]
        _close: (),
    }

    #[adze::leaf(pattern = r"[a-zA-Z_][a-zA-Z0-9_]*", transform = |s| s.to_string())]
    pub struct Identifier(pub String);

    #[adze::leaf(pattern = r"\d+(\.\d+)?", transform = |s| s.parse().unwrap())]
    pub struct Number(pub f64);

    #[adze::extra]
    struct Whitespace {
        #[adze::leaf(pattern = r"([ \t\n\r]+|//[^\n]*\n?)")]
        _ws: (),
    }
}
```

## Error Handling

### Comprehensive Error Handling

```rust
use adze::errors::{ParseError, ParseErrorReason};

fn parse_with_diagnostics(input: &str) -> Result<(), Vec<String>> {
    match grammar::parse(input) {
        Ok(ast) => {
            println!("Successfully parsed!");
            Ok(())
        }
        Err(errors) => {
            let mut diagnostics = Vec::new();
            
            for error in errors {
                let diagnostic = format_error(&error, input);
                diagnostics.push(diagnostic);
            }
            
            Err(diagnostics)
        }
    }
}

fn format_error(error: &ParseError, input: &str) -> String {
    let line_num = input[..error.start].matches('\n').count() + 1;
    let col_num = error.start - input[..error.start].rfind('\n').unwrap_or(0);
    
    match &error.reason {
        ParseErrorReason::MissingToken(token) => {
            format!("{}:{}: Expected '{}'", line_num, col_num, token)
        }
        ParseErrorReason::UnexpectedToken(token) => {
            format!("{}:{}: Unexpected token '{}'", line_num, col_num, token)
        }
        ParseErrorReason::FailedNode(errors) => {
            format!("{}:{}: Failed to parse node ({} sub-errors)", 
                   line_num, col_num, errors.len())
        }
    }
}

// Pretty error display
fn display_error_context(input: &str, error: &ParseError) {
    let lines: Vec<&str> = input.lines().collect();
    let error_line = input[..error.start].matches('\n').count();
    
    // Show context
    for i in error_line.saturating_sub(1)..=(error_line + 1).min(lines.len() - 1) {
        println!("{:4} | {}", i + 1, lines[i]);
        
        if i == error_line {
            let col = error.start - input[..error.start].rfind('\n').unwrap_or(0);
            println!("     | {}^", " ".repeat(col - 1));
        }
    }
}
```

## Tree Traversal

### Inspecting the Document Tree

```rust
fn print_document_summary(source: &str) -> Result<(), Box<dyn std::error::Error>> {
    let document = grammar::parse_document(source)?;
    let root = document.tree().root();

    println!("root kind: {}", root.kind_name().unwrap_or("<unknown>"));
    println!("root bytes: {:?}", root.byte_range());
    println!("diagnostics: {}", document.diagnostics().len());

    for edge in root.child_edges() {
        let Some(child) = edge.child() else {
            continue;
        };
        println!(
            "child {} {:?}",
            child.kind_name().unwrap_or("<unknown>"),
            child.byte_range(),
        );
    }

    Ok(())
}
```

When a tool needs Tree-sitter-style traversal, use the selected-tree
compatibility projection instead of creating a second parser truth:

```rust
fn print_sexp(
    source: &str,
    language: std::sync::Arc<adze::ts_compat::Language>,
) -> Result<(), Box<dyn std::error::Error>> {
    let document = grammar::parse_document(source)?;
    let tree = adze::ts_compat::Tree::from_document(language, &document);

    println!("{}", tree.root_node().to_sexp());
    Ok(())
}
```

## Grammar Analysis

### Using Grammar Validation and Optimization

```rust
use adze_ir::{GrammarValidator, GrammarOptimizer};

fn analyze_grammar(grammar: &mut Grammar) {
    // Validate grammar
    let validator = GrammarValidator::new();
    let validation_result = validator.validate(grammar);
    
    println!("Grammar Validation Results:");
    println!("==========================");
    
    if validation_result.errors.is_empty() {
        println!("✓ No errors found");
    } else {
        println!("✗ {} errors found:", validation_result.errors.len());
        for error in &validation_result.errors {
            println!("  - {}", error);
        }
    }
    
    if !validation_result.warnings.is_empty() {
        println!("\n⚠ {} warnings:", validation_result.warnings.len());
        for warning in &validation_result.warnings {
            println!("  - {}", warning);
        }
    }
    
    println!("\nGrammar Statistics:");
    println!("  Total symbols: {}", validation_result.stats.total_symbols);
    println!("  Reachable symbols: {}", validation_result.stats.reachable_symbols);
    println!("  Terminal symbols: {}", validation_result.stats.terminal_symbols);
    
    // Optimize grammar
    let mut optimizer = GrammarOptimizer::new();
    optimizer.optimize_grammar(grammar);
    
    let opt_stats = optimizer.get_stats();
    println!("\nOptimization Results:");
    println!("====================");
    println!("  Removed {} unused symbols", opt_stats.removed_unused_symbols);
    println!("  Inlined {} rules", opt_stats.inlined_rules);
    println!("  Merged {} duplicate tokens", opt_stats.merged_tokens);
    println!("  Optimized {} left-recursive rules", opt_stats.left_recursion_optimized);
}
```

## Custom Transformations

### Building an Interpreter

```rust
// Calculator interpreter
impl calculator::Expression {
    fn evaluate(&self) -> f64 {
        use calculator::Expression::*;
        match self {
            Number(n) => *n,
            Add(left, _, right) => left.evaluate() + right.evaluate(),
            Subtract(left, _, right) => left.evaluate() - right.evaluate(),
            Multiply(left, _, right) => left.evaluate() * right.evaluate(),
            Divide(left, _, right) => left.evaluate() / right.evaluate(),
            Parenthesized(_, expr, _) => expr.evaluate(),
        }
    }
}

// Type checker for mini-c
#[derive(Debug, Clone, PartialEq)]
enum TypeInfo {
    Int,
    Float,
    Bool,
    Void,
    Error(String),
}

impl mini_c::Expression {
    fn type_check(&self, env: &HashMap<String, TypeInfo>) -> TypeInfo {
        use mini_c::Expression::*;
        match self {
            Primary(p) => match p {
                mini_c::Primary::Number(_) => TypeInfo::Float,
                mini_c::Primary::True(_) | mini_c::Primary::False(_) => TypeInfo::Bool,
                mini_c::Primary::Identifier(id) => {
                    env.get(&id.0).cloned()
                        .unwrap_or(TypeInfo::Error(format!("Unknown variable: {}", id.0)))
                }
                mini_c::Primary::Parenthesized(p) => p.expr.type_check(env),
            },
            Add(left, _, right) | Subtract(left, _, right) |
            Multiply(left, _, right) | Divide(left, _, right) => {
                match (left.type_check(env), right.type_check(env)) {
                    (TypeInfo::Int, TypeInfo::Int) => TypeInfo::Int,
                    (TypeInfo::Float, TypeInfo::Float) => TypeInfo::Float,
                    (TypeInfo::Int, TypeInfo::Float) | (TypeInfo::Float, TypeInfo::Int) => TypeInfo::Float,
                    _ => TypeInfo::Error("Type mismatch in arithmetic".to_string()),
                }
            }
            Equal(left, _, right) | NotEqual(left, _, right) |
            Less(left, _, right) | Greater(left, _, right) => {
                match (left.type_check(env), right.type_check(env)) {
                    (TypeInfo::Error(e), _) | (_, TypeInfo::Error(e)) => TypeInfo::Error(e),
                    _ => TypeInfo::Bool,
                }
            }
            // ... other cases
            _ => TypeInfo::Error("Not implemented".to_string()),
        }
    }
}
```

## Performance Optimization

### Keep the Hot Path Simple

```rust
use std::sync::Arc;

// Cache typed parse results when your application repeatedly parses identical
// source text. Keep cache invalidation in application code; Adze's stable
// front door remains the generated parser function.
use lru::LruCache;

struct CachedParser {
    cache: LruCache<String, Arc<grammar::Expression>>,
}

impl CachedParser {
    fn new(cache_size: usize) -> Self {
        Self {
            cache: LruCache::new(cache_size),
        }
    }
    
    fn parse(&mut self, input: &str) -> Result<Arc<grammar::Expression>, ParseError> {
        if let Some(cached) = self.cache.get(input) {
            return Ok(cached.clone());
        }
        
        let ast = grammar::parse(input)?;
        let arc_ast = Arc::new(ast);
        self.cache.put(input.to_string(), arc_ast.clone());
        Ok(arc_ast)
    }
}

// Parallel parsing
use rayon::prelude::*;

fn parse_many_files(files: Vec<(String, String)>) -> Vec<Result<grammar::Expression, ParseError>> {
    files.into_par_iter()
        .map(|(name, content)| {
            grammar::parse(&content)
                .map_err(|e| ParseError {
                    start: e.start,
                    end: e.end,
                    reason: ParseErrorReason::FailedNode(vec![format!("In file: {}", name)]),
                })
        })
        .collect()
}
```

Use `parse_document()` for diagnostics, syntax facts, JSON, or
Tree-sitter-compatible output. It is richer than `parse()`, so keep it on the
tooling path rather than making every typed-parse call pay for projections your
application does not need.

## Integration Examples

### Language Server Protocol Integration

```rust
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};

struct AdzeLanguageServer {
    client: Client,
}

#[tower_lsp::async_trait]
impl LanguageServer for AdzeLanguageServer {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                diagnostic_provider: Some(DiagnosticServerCapabilities::Options(
                    DiagnosticOptions::default(),
                )),
                ..Default::default()
            },
            ..Default::default()
        })
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        self.parse_and_diagnose(params.text_document).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        self.parse_and_diagnose(params.text_document).await;
    }
}

impl AdzeLanguageServer {
    async fn parse_and_diagnose(&self, document: TextDocumentItem) {
        let parse = grammar::parse_document(&document.text);
        let diagnostics = match parse {
            Ok(parse_document) => parse_document
                .diagnostics()
                .iter()
                .map(|diagnostic| document_diagnostic_to_lsp(diagnostic, &document.text))
                .collect(),
            Err(errors) => errors
                .into_iter()
                .map(|error| parse_error_to_lsp(error, &document.text))
                .collect(),
        };

        self.client
            .publish_diagnostics(document.uri, diagnostics, None)
            .await;
    }
}
```

### Syntax Highlighting

```rust
use adze::ts_compat::TreeCursor;

#[derive(Debug, Clone)]
struct Highlight {
    start: usize,
    end: usize,
    scope: String,
}

fn highlight_code(
    source: &str,
    language: std::sync::Arc<adze::ts_compat::Language>,
) -> Result<Vec<Highlight>, Box<dyn std::error::Error>> {
    let document = grammar::parse_document(source)?;
    let tree = adze::ts_compat::Tree::from_document(language, &document);
    let mut highlights = Vec::new();
    let mut cursor = tree.walk();
    
    highlight_node(&mut cursor, source, &mut highlights);
    Ok(highlights)
}

fn highlight_node(cursor: &mut TreeCursor, source: &str, highlights: &mut Vec<Highlight>) {
    let node = cursor.node();
    
    let scope = match node.kind() {
        "function" => Some("entity.name.function"),
        "identifier" => Some("variable"),
        "number" => Some("constant.numeric"),
        "string" => Some("string.quoted"),
        "comment" => Some("comment"),
        "if" | "else" | "while" | "return" => Some("keyword.control"),
        "int" | "float" | "bool" | "void" => Some("storage.type"),
        "true" | "false" | "null" => Some("constant.language"),
        "+" | "-" | "*" | "/" | "=" => Some("keyword.operator"),
        _ => None,
    };
    
    if let Some(scope) = scope {
        highlights.push(Highlight {
            start: node.start_byte(),
            end: node.end_byte(),
            scope: scope.to_string(),
        });
    }
    
    if cursor.goto_first_child() {
        loop {
            highlight_node(cursor, source, highlights);
            if !cursor.goto_next_sibling() {
                break;
            }
        }
        cursor.goto_parent();
    }
}

// Generate HTML with syntax highlighting
fn to_html(source: &str, highlights: &[Highlight]) -> String {
    let mut html = String::from("<pre><code>");
    let mut last_end = 0;
    
    for highlight in highlights {
        // Add unhighlighted text
        if highlight.start > last_end {
            html.push_str(&html_escape(&source[last_end..highlight.start]));
        }
        
        // Add highlighted text
        html.push_str(&format!(
            r#"<span class="{}">{}</span>"#,
            highlight.scope,
            html_escape(&source[highlight.start..highlight.end])
        ));
        
        last_end = highlight.end;
    }
    
    // Add remaining text
    if last_end < source.len() {
        html.push_str(&html_escape(&source[last_end..]));
    }
    
    html.push_str("</code></pre>");
    html
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
```

## Best Practices

1. **Grammar Design**
   - Keep rules simple and composable
   - Use meaningful names for all nodes
   - Add appropriate precedence and associativity
   - Test edge cases thoroughly

2. **Performance**
   - Start with generated `grammar::parse()`
   - Cache typed parse results when your application repeats identical inputs
   - Treat incremental parsing as experimental until its support-tier row says otherwise
   - Profile before optimizing

3. **Error Handling**
   - Always handle parse errors gracefully
   - Provide meaningful error messages
   - Show context for errors
   - Use `parse_document()` when tooling needs diagnostics from the same parse truth

4. **Integration**
   - Use `parse()` for typed semantic values
   - Use `parse_document()` for document facts and diagnostics
   - Use `ts_compat::Tree::from_document(...)` only when you need compatibility with Tree-sitter-shaped tooling
   - Leverage the type system for AST processing
   - Test with real-world inputs

This guide covers the most common usage patterns. For support status and known
limits, see [Support Tiers](../status/SUPPORT_TIERS.md). For the API ladder, see
the [API Reference](./api.md).
