// Python grammar for adze
// Simplified version for v0.5.0-beta (without indentation handling)

#![allow(
    clippy::manual_non_exhaustive,
    reason = "grammar enum shape is intentionally kept compatible with generated parser metadata"
)]

pub mod scanner;

// Include the generated parser
pub mod grammar_python {
    include!(concat!(env!("OUT_DIR"), "/grammar_python/parser_python.rs"));
}

/// Parse Python source code into a syntax tree
pub fn parse(source: &str) -> Result<adze::pure_parser::ParsedNode, Box<dyn std::error::Error>> {
    use adze::pure_parser::Parser;

    let language = get_language();
    let mut parser = Parser::new();
    parser.set_language(language)?;
    let result = parser.parse_string(source);
    result.root.ok_or_else(|| "Parsing failed".into())
}

// Function to register the scanner - call this from build.rs or when loading the grammar
pub fn register_scanner() {
    adze::scanner_registry::register_rust_scanner::<scanner::PythonScanner>("python");
}

// Expose the generated LANGUAGE struct for external use (e.g., benchmarks)
pub fn get_language() -> &'static adze::pure_parser::TSLanguage {
    &grammar_python::LANGUAGE
}

#[adze::grammar("python")]
pub mod grammar {
    // External scanner tokens for Python's indentation
    #[adze::external]
    pub struct Newline;

    #[adze::external]
    pub struct Indent;

    #[adze::external]
    pub struct Dedent;

    #[adze::external]
    pub struct StringStart;

    #[adze::external]
    pub struct StringContent;

    #[adze::external]
    pub struct StringEnd;

    #[adze::external]
    pub struct Comment;

    #[adze::external]
    pub struct LineJoining;

    #[adze::external]
    pub struct ErrorSentinel;

    #[adze::language]
    pub struct Module {
        // Allow empty modules - this is crucial for GLR compatibility
        // Empty Python files are valid and should parse successfully
        #[adze::repeat]
        pub statements: Vec<Statement>,
    }

    #[adze::language]
    pub enum Statement {
        Simple(SimpleStatement),
        Compound(CompoundStatement),
    }

    #[adze::language]
    pub struct SimpleStatement {
        pub statement: SimpleStmt,
        #[adze::leaf(pattern = r"\n")]
        _newline: String,
    }

    #[adze::language]
    pub enum SimpleStmt {
        Expression(ExpressionStatement),
        Assignment(Assignment),
        Return(ReturnStatement),
        Pass(PassStatement),
        Break(BreakStatement),
        Continue(ContinueStatement),
        Import(ImportStatement),
    }

    #[adze::language]
    pub enum CompoundStatement {
        Function(FunctionDefinition),
        Class(ClassDefinition),
        If(IfStatement),
        While(WhileStatement),
        For(ForStatement),
    }

    #[adze::language]
    pub struct ExpressionStatement {
        pub expression: Expression,
    }

    #[adze::language]
    pub struct Assignment {
        pub target: Expression,
        #[adze::leaf(text = "=")]
        _equals: (),
        pub value: Expression,
    }

    #[adze::language]
    pub struct ReturnStatement {
        #[adze::leaf(text = "return")]
        _return: (),
        pub value: Option<Expression>,
    }

    #[adze::language]
    pub struct PassStatement {
        #[adze::leaf(text = "pass")]
        _pass: (),
    }

    #[adze::language]
    pub struct BreakStatement {
        #[adze::leaf(text = "break")]
        _break: (),
    }

    #[adze::language]
    pub struct ContinueStatement {
        #[adze::leaf(text = "continue")]
        _continue: (),
    }

    #[adze::language]
    pub struct ImportStatement {
        #[adze::leaf(text = "import")]
        _import: (),
        pub module: DottedName,
    }

    #[adze::language]
    pub enum DottedName {
        // Single identifier like "os"
        Single(Identifier),
        // Dotted name like "os.path"
        Dotted {
            first: Identifier,
            #[adze::repeat(non_empty = true)]
            rest: Vec<DottedNamePart>,
        },
    }

    #[adze::language]
    pub struct DottedNamePart {
        #[adze::leaf(text = ".")]
        _dot: (),
        pub name: Identifier,
    }

    #[adze::language]
    pub struct FunctionDefinition {
        #[adze::leaf(text = "def")]
        _def: (),
        pub name: Identifier,
        pub parameters: Parameters,
        #[adze::leaf(text = ":")]
        _colon: (),
        #[adze::leaf(pattern = r"\n")]
        _newline: String,
        pub body: Block,
    }

    #[adze::language]
    pub struct Parameters {
        #[adze::leaf(text = "(")]
        _open: (),
        // Allow optional whitespace to prevent empty rule
        #[adze::leaf(pattern = r"\s*")]
        _ws1: (),
        #[adze::repeat]
        #[adze::delimited(#[adze::leaf(text = ",")] ())]
        pub params: Vec<Parameter>,
        #[adze::leaf(pattern = r"\s*")]
        _ws2: (),
        #[adze::leaf(text = ")")]
        _close: (),
    }

    #[adze::language]
    pub struct Parameter {
        pub name: Identifier,
    }

    #[adze::language]
    #[adze::prec_left(1)]
    pub struct Block {
        #[adze::repeat(non_empty = true)]
        pub statements: Vec<Statement>,
    }

    #[adze::language]
    pub struct ClassDefinition {
        #[adze::leaf(text = "class")]
        _class: (),
        pub name: Identifier,
        pub bases: Option<ClassBases>,
        #[adze::leaf(text = ":")]
        _colon: (),
        #[adze::leaf(pattern = r"\n")]
        _newline: String,
        pub body: Block,
    }

    #[adze::language]
    pub struct ClassBases {
        #[adze::leaf(text = "(")]
        _open: (),
        #[adze::leaf(pattern = r"\s*")]
        _ws1: (),
        #[adze::repeat]
        #[adze::delimited(#[adze::leaf(text = ",")] ())]
        pub bases: Vec<Expression>,
        #[adze::leaf(pattern = r"\s*")]
        _ws2: (),
        #[adze::leaf(text = ")")]
        _close: (),
    }

    #[adze::language]
    pub struct IfStatement {
        #[adze::leaf(text = "if")]
        _if: (),
        pub condition: Expression,
        #[adze::leaf(text = ":")]
        _colon: (),
        #[adze::leaf(pattern = r"\n")]
        _newline: String,
        pub body: Block,
        pub else_clause: Option<ElseClause>,
    }

    #[adze::language]
    pub struct ElseClause {
        #[adze::leaf(text = "else")]
        _else: (),
        #[adze::leaf(text = ":")]
        _colon: (),
        #[adze::leaf(pattern = r"\n")]
        _newline: String,
        pub body: Block,
    }

    #[adze::language]
    pub struct WhileStatement {
        #[adze::leaf(text = "while")]
        _while: (),
        pub condition: Expression,
        #[adze::leaf(text = ":")]
        _colon: (),
        #[adze::leaf(pattern = r"\n")]
        _newline: String,
        pub body: Block,
    }

    #[adze::language]
    pub struct ForStatement {
        #[adze::leaf(text = "for")]
        _for: (),
        pub target: Identifier,
        #[adze::leaf(text = "in")]
        _in: (),
        pub iter: Expression,
        #[adze::leaf(text = ":")]
        _colon: (),
        #[adze::leaf(pattern = r"\n")]
        _newline: String,
        pub body: Block,
    }

    #[adze::language]
    pub enum Expression {
        #[adze::prec_left(2)]
        Binary(Box<BinaryExpression>),
        #[adze::prec(1)]
        Unary(Box<UnaryExpression>),
        #[adze::prec(10)]
        Call(Box<CallExpression>),
        #[adze::prec(10)]
        Attribute(Box<AttributeExpression>),
        #[adze::prec(10)]
        Subscript(Box<SubscriptExpression>),
        Primary(PrimaryExpression),
    }

    #[adze::language]
    #[adze::prec_left(2)]
    pub struct BinaryExpression {
        pub left: Expression,
        pub operator: BinaryOperator,
        pub right: Expression,
    }

    #[adze::language]
    pub enum BinaryOperator {
        Add(AddOp),
        Subtract(SubOp),
        Multiply(MulOp),
        Divide(DivOp),
        Modulo(ModOp),
        Power(PowerOp),
        Equal(EqOp),
        NotEqual(NeOp),
        Less(LtOp),
        Greater(GtOp),
        And(AndOp),
        Or(OrOp),
    }

    #[adze::language]
    pub struct AddOp {
        #[adze::leaf(text = "+")]
        _op: (),
    }

    #[adze::language]
    pub struct SubOp {
        #[adze::leaf(text = "-")]
        _op: (),
    }

    #[adze::language]
    pub struct MulOp {
        #[adze::leaf(text = "*")]
        _op: (),
    }

    #[adze::language]
    pub struct DivOp {
        #[adze::leaf(text = "/")]
        _op: (),
    }

    #[adze::language]
    pub struct ModOp {
        #[adze::leaf(text = "%")]
        _op: (),
    }

    #[adze::language]
    pub struct PowerOp {
        #[adze::leaf(text = "**")]
        _op: (),
    }

    #[adze::language]
    pub struct EqOp {
        #[adze::leaf(text = "==")]
        _op: (),
    }

    #[adze::language]
    pub struct NeOp {
        #[adze::leaf(text = "!=")]
        _op: (),
    }

    #[adze::language]
    pub struct LtOp {
        #[adze::leaf(text = "<")]
        _op: (),
    }

    #[adze::language]
    pub struct GtOp {
        #[adze::leaf(text = ">")]
        _op: (),
    }

    #[adze::language]
    pub struct AndOp {
        #[adze::leaf(text = "and")]
        _op: (),
    }

    #[adze::language]
    pub struct OrOp {
        #[adze::leaf(text = "or")]
        _op: (),
    }

    #[adze::language]
    #[adze::prec(1)]
    pub struct UnaryExpression {
        pub operator: UnaryOperator,
        pub operand: Expression,
    }

    #[adze::language]
    pub enum UnaryOperator {
        Not(NotOp),
        Minus(MinusOp),
    }

    #[adze::language]
    pub struct NotOp {
        #[adze::leaf(text = "not")]
        _op: (),
    }

    #[adze::language]
    pub struct MinusOp {
        #[adze::leaf(text = "-")]
        _op: (),
    }

    #[adze::language]
    #[adze::prec(10)]
    pub struct CallExpression {
        pub function: Expression,
        pub arguments: Arguments,
    }

    #[adze::language]
    pub struct Arguments {
        #[adze::leaf(text = "(")]
        _open: (),
        #[adze::leaf(pattern = r"\s*")]
        _ws1: (),
        #[adze::repeat]
        #[adze::delimited(#[adze::leaf(text = ",")] ())]
        pub args: Vec<Expression>,
        #[adze::leaf(pattern = r"\s*")]
        _ws2: (),
        #[adze::leaf(text = ")")]
        _close: (),
    }

    #[adze::language]
    #[adze::prec(10)]
    pub struct AttributeExpression {
        pub value: Expression,
        #[adze::leaf(text = ".")]
        _dot: (),
        pub attr: Identifier,
    }

    #[adze::language]
    pub struct SubscriptExpression {
        pub value: Expression,
        #[adze::leaf(text = "[")]
        _open: (),
        pub index: Expression,
        #[adze::leaf(text = "]")]
        _close: (),
    }

    #[adze::language]
    pub enum PrimaryExpression {
        Identifier(Identifier),
        Literal(Literal),
        List(ListExpression),
        Tuple(TupleExpression),
        Dict(DictExpression),
    }

    #[adze::language]
    pub struct Identifier {
        #[adze::leaf(pattern = r"[a-zA-Z_][a-zA-Z0-9_]*")]
        pub name: String,
    }

    #[adze::language]
    pub enum Literal {
        String(StringLiteral),
        Number(NumberLiteral),
        Boolean(BooleanLiteral),
        None(NoneLiteral),
    }

    #[adze::language]
    pub enum StringLiteral {
        SingleQuoted(SingleQuotedString),
        DoubleQuoted(DoubleQuotedString),
    }

    #[adze::language]
    pub struct SingleQuotedString {
        #[adze::leaf(pattern = r"'([^'\\]|\\.)*'")]
        pub value: String,
    }

    #[adze::language]
    pub struct DoubleQuotedString {
        #[adze::leaf(pattern = r#""([^"\\]|\\.)*""#)]
        pub value: String,
    }

    #[adze::language]
    pub struct NumberLiteral {
        #[adze::leaf(pattern = r"(\d+\.?\d*|\.\d+)([eE][+-]?\d+)?")]
        pub value: String,
    }

    #[adze::language]
    pub enum BooleanLiteral {
        True(TrueLiteral),
        False(FalseLiteral),
    }

    #[adze::language]
    pub struct TrueLiteral {
        #[adze::leaf(text = "True")]
        _true: (),
    }

    #[adze::language]
    pub struct FalseLiteral {
        #[adze::leaf(text = "False")]
        _false: (),
    }

    #[adze::language]
    pub struct NoneLiteral {
        #[adze::leaf(text = "None")]
        _none: (),
    }

    #[adze::language]
    pub struct ListExpression {
        #[adze::leaf(text = "[")]
        _open: (),
        #[adze::leaf(pattern = r"\s*")]
        _ws1: (),
        #[adze::repeat]
        #[adze::delimited(#[adze::leaf(text = ",")] ())]
        pub elements: Vec<Expression>,
        #[adze::leaf(pattern = r"\s*")]
        _ws2: (),
        #[adze::leaf(text = "]")]
        _close: (),
    }

    #[adze::language]
    pub struct TupleExpression {
        #[adze::leaf(text = "(")]
        _open: (),
        #[adze::leaf(pattern = r"\s*")]
        _ws1: (),
        #[adze::repeat]
        #[adze::delimited(#[adze::leaf(text = ",")] ())]
        pub elements: Vec<Expression>,
        #[adze::leaf(pattern = r"\s*")]
        _ws2: (),
        #[adze::leaf(text = ")")]
        _close: (),
    }

    #[adze::language]
    pub struct DictExpression {
        #[adze::leaf(text = "{")]
        _open: (),
        #[adze::leaf(pattern = r"\s*")]
        _ws1: (),
        #[adze::repeat]
        #[adze::delimited(#[adze::leaf(text = ",")] ())]
        pub items: Vec<DictItem>,
        #[adze::leaf(pattern = r"\s*")]
        _ws2: (),
        #[adze::leaf(text = "}")]
        _close: (),
    }

    #[adze::language]
    pub struct DictItem {
        pub key: Expression,
        #[adze::leaf(text = ":")]
        _colon: (),
        pub value: Expression,
    }

    #[adze::extra]
    #[allow(dead_code)]
    struct Whitespace {
        #[adze::leaf(pattern = r"\s")]
        _whitespace: (),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_simple_program() {
        // Grammar builds successfully
        // Test placeholder - replaced with actual assertion
    }
}
