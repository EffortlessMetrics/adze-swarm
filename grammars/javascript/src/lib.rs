// JavaScript grammar for adze
// Simplified version for v0.5.0-beta

#![allow(
    clippy::manual_non_exhaustive,
    reason = "grammar enum shape is intentionally kept compatible with generated parser metadata"
)]

#[adze::grammar("javascript")]
pub mod grammar {
    #[adze::language]
    pub struct Program {
        #[adze::repeat(non_empty = true)]
        pub statements: Vec<Statement>,
    }

    #[adze::language]
    pub enum Statement {
        Expression(ExpressionStatement),
        Variable(VariableDeclaration),
        Function(FunctionDeclaration),
        Return(ReturnStatement),
        If(IfStatement),
        Block(BlockStatement),
        Empty(EmptyStatement),
    }

    #[adze::language]
    pub struct ExpressionStatement {
        pub expression: Expression,
        #[adze::leaf(text = ";")]
        _semicolon: (),
    }

    #[adze::language]
    pub struct Semicolon {
        #[adze::leaf(text = ";")]
        _semi: (),
    }

    #[adze::language]
    pub struct VariableDeclaration {
        pub kind: VarKind,
        #[adze::repeat(non_empty = true)]
        pub declarations: Vec<VariableDeclarator>,
        #[adze::leaf(text = ";")]
        _semicolon: (),
    }

    #[adze::language]
    pub enum VarKind {
        Var(VarKeyword),
        Let(LetKeyword),
        Const(ConstKeyword),
    }

    #[adze::language]
    pub struct VarKeyword {
        #[adze::leaf(text = "var")]
        _var: (),
    }

    #[adze::language]
    pub struct LetKeyword {
        #[adze::leaf(text = "let")]
        _let: (),
    }

    #[adze::language]
    pub struct ConstKeyword {
        #[adze::leaf(text = "const")]
        _const: (),
    }

    #[adze::language]
    pub struct VariableDeclarator {
        pub id: Identifier,
        pub init: Option<VariableInit>,
    }

    #[adze::language]
    pub struct VariableInit {
        #[adze::leaf(text = "=")]
        _equals: (),
        pub expression: Expression,
    }

    #[adze::language]
    pub struct FunctionDeclaration {
        #[adze::leaf(text = "function")]
        _function: (),
        pub name: Identifier,
        pub parameters: FormalParameters,
        pub body: BlockStatement,
    }

    #[adze::language]
    pub struct FormalParameters {
        #[adze::leaf(text = "(")]
        _open: (),
        #[adze::repeat]
        pub params: Vec<FormalParameter>,
        #[adze::leaf(text = ")")]
        _close: (),
    }

    #[adze::language]
    pub struct FormalParameter {
        pub name: Identifier,
    }

    #[adze::language]
    pub struct Comma {
        #[adze::leaf(text = ",")]
        _comma: (),
    }

    #[adze::language]
    pub struct ReturnStatement {
        #[adze::leaf(text = "return")]
        _return: (),
        pub expression: Option<Expression>,
        #[adze::leaf(text = ";")]
        _semicolon: (),
    }

    #[adze::language]
    pub struct IfStatement {
        #[adze::leaf(text = "if")]
        _if: (),
        #[adze::leaf(text = "(")]
        _open: (),
        pub condition: Expression,
        #[adze::leaf(text = ")")]
        _close: (),
        pub then_statement: Box<Statement>,
        pub else_clause: Option<ElseClause>,
    }

    #[adze::language]
    pub struct ElseClause {
        #[adze::leaf(text = "else")]
        _else: (),
        pub statement: Box<Statement>,
    }

    #[adze::language]
    pub struct BlockStatement {
        #[adze::leaf(text = "{")]
        _open: (),
        #[adze::repeat]
        pub statements: Vec<Statement>,
        #[adze::leaf(text = "}")]
        _close: (),
    }

    #[adze::language]
    pub struct EmptyStatement {
        #[adze::leaf(text = ";")]
        _semicolon: (),
    }

    #[adze::language]
    pub enum Expression {
        Binary(Box<BinaryExpression>),
        Unary(Box<UnaryExpression>),
        Call(Box<CallExpression>),
        Member(Box<MemberExpression>),
        Primary(PrimaryExpression),
    }

    #[adze::language]
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
        Equal(EqOp),
        NotEqual(NeOp),
        Less(LtOp),
        Greater(GtOp),
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
    pub struct UnaryExpression {
        pub operator: UnaryOperator,
        pub argument: Expression,
    }

    #[adze::language]
    pub enum UnaryOperator {
        Not(NotOp),
        Minus(MinusOp),
    }

    #[adze::language]
    pub struct NotOp {
        #[adze::leaf(text = "!")]
        _op: (),
    }

    #[adze::language]
    pub struct MinusOp {
        #[adze::leaf(text = "-")]
        _op: (),
    }

    #[adze::language]
    pub struct CallExpression {
        pub callee: Expression,
        pub arguments: Arguments,
    }

    #[adze::language]
    pub struct Arguments {
        #[adze::leaf(text = "(")]
        _open: (),
        #[adze::repeat]
        pub args: Vec<Argument>,
        #[adze::leaf(text = ")")]
        _close: (),
    }

    #[adze::language]
    pub struct Argument {
        pub expression: Expression,
    }

    #[adze::language]
    pub struct MemberExpression {
        pub object: Expression,
        pub property: MemberProperty,
    }

    #[adze::language]
    pub enum MemberProperty {
        Dot(DotProperty),
        Bracket(BracketProperty),
    }

    #[adze::language]
    pub struct DotProperty {
        #[adze::leaf(text = ".")]
        _dot: (),
        pub property: Identifier,
    }

    #[adze::language]
    pub struct BracketProperty {
        #[adze::leaf(text = "[")]
        _open: (),
        pub property: Expression,
        #[adze::leaf(text = "]")]
        _close: (),
    }

    #[adze::language]
    pub enum PrimaryExpression {
        Identifier(Identifier),
        Literal(Literal),
        Parenthesized(ParenthesizedExpression),
    }

    #[adze::language]
    pub struct Identifier {
        #[adze::leaf(pattern = r"[a-zA-Z_$][a-zA-Z0-9_$]*")]
        pub name: String,
    }

    #[adze::language]
    pub enum Literal {
        String(StringLiteral),
        Number(NumberLiteral),
        Boolean(BooleanLiteral),
        Null(NullLiteral),
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
        #[adze::leaf(text = "true")]
        _true: (),
    }

    #[adze::language]
    pub struct FalseLiteral {
        #[adze::leaf(text = "false")]
        _false: (),
    }

    #[adze::language]
    pub struct NullLiteral {
        #[adze::leaf(text = "null")]
        _null: (),
    }

    #[adze::language]
    pub struct ParenthesizedExpression {
        #[adze::leaf(text = "(")]
        _open: (),
        pub expression: Box<Expression>,
        #[adze::leaf(text = ")")]
        _close: (),
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
    use adze::pure_parser::Parser;

    #[test]
    fn test_simple_program() {
        // Grammar builds successfully - this test just ensures the grammar compiles
        let _language = super::grammar::language();
    }

    /// Whitespace is declared as an extra, so spacing around tokens must not
    /// produce lexical errors and must not be absorbed into token spans
    /// (span contract: the `;` token text stays `";"`, covered by the
    /// golden canary `canary_expression.js` = `";\n"`).
    #[test]
    fn test_whitespace_variants_parse_without_errors() {
        for source in [";\n", "; ;", ";\n", "  ;  ", "; ;\n", ";;", "; ;;"] {
            let mut parser = Parser::new();
            parser
                .set_language(super::grammar::language())
                .expect("failed to construct adze-javascript language");
            let result = parser.parse_bytes(source.as_bytes());
            assert!(result.root.is_some(), "expected parse root for {source:?}");
            assert!(
                result.errors.is_empty(),
                "expected zero errors for {source:?}, got: {:?}",
                result.errors
            );
        }
    }
}
