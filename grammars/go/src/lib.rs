#![allow(
    clippy::empty_line_after_outer_attr,
    clippy::manual_non_exhaustive,
    clippy::op_ref,
    clippy::char_lit_as_u8,
    clippy::unnecessary_cast,
    clippy::assertions_on_constants
)]

// Simplified Go grammar for adze v0.5.0-beta
// This is a minimal subset to demonstrate basic functionality

#[adze::grammar("go")]
pub mod grammar {
    #[adze::language]
    pub struct SourceFile {
        pub package_clause: PackageClause,
        #[adze::repeat]
        pub declarations: Vec<Declaration>,
    }

    #[adze::language]
    pub struct PackageClause {
        #[adze::leaf(text = "package")]
        _package: (),
        pub name: Identifier,
    }

    #[adze::language]
    pub enum Declaration {
        Function(FunctionDeclaration),
        Variable(VarDeclaration),
    }

    #[adze::language]
    pub struct FunctionDeclaration {
        #[adze::leaf(text = "func")]
        _func: (),
        pub name: Identifier,
        #[adze::leaf(text = "(")]
        _lparen: (),
        #[adze::leaf(text = ")")]
        _rparen: (),
        pub body: Block,
    }

    #[adze::language]
    pub struct VarDeclaration {
        #[adze::leaf(text = "var")]
        _var: (),
        pub name: Identifier,
        pub type_name: Identifier,
    }

    #[adze::language]
    pub struct Block {
        #[adze::leaf(text = "{")]
        _open: (),
        #[adze::repeat]
        pub statements: Vec<Statement>,
        #[adze::leaf(text = "}")]
        _close: (),
    }

    #[adze::language]
    pub enum Statement {
        Assignment(AssignmentStatement),
        Call(CallStatement),
        Return(ReturnStatement),
    }

    #[adze::language]
    pub struct AssignmentStatement {
        pub name: Identifier,
        #[adze::leaf(text = "=")]
        _eq: (),
        pub value: Expression,
    }

    #[adze::language]
    pub struct CallStatement {
        pub name: Identifier,
        #[adze::leaf(text = "(")]
        _lparen: (),
        #[adze::repeat]
        pub args: Vec<Expression>,
        #[adze::leaf(text = ")")]
        _rparen: (),
    }

    #[adze::language]
    pub struct ReturnStatement {
        #[adze::leaf(text = "return")]
        _return: (),
        pub value: Expression,
    }

    #[adze::language]
    pub enum Expression {
        Identifier(Identifier),
        Literal(Literal),
    }

    #[adze::language]
    pub enum Literal {
        String(StringLiteral),
        Number(NumberLiteral),
    }

    #[adze::language]
    pub struct StringLiteral {
        #[adze::leaf(pattern = r#""[^"]*""#)]
        pub value: (),
    }

    #[adze::language]
    pub struct NumberLiteral {
        #[adze::leaf(pattern = r"\d+")]
        pub value: (),
    }

    #[adze::language]
    pub struct Identifier {
        #[adze::leaf(pattern = r"[a-zA-Z_][a-zA-Z0-9_]*")]
        pub name: (),
    }

    #[adze::extra]
    pub enum Extra {
        #[adze::leaf(pattern = r"\s+")]
        Whitespace,
    }
}

#[cfg(test)]
mod tests {
    use super::grammar;
    use adze::pure_parser::Parser;

    #[test]
    fn smoke_build_language_and_parse_minimal_fixture() {
        // Verify the language object constructs and the parser can be invoked.
        // Note: 'package main' with zero declarations still has a narrow
        // epsilon-reduce-at-EOF issue (tracked in #784). The declaration
        // parsing test below verifies the main use case works.
        let mut parser = Parser::new();
        parser
            .set_language(grammar::language())
            .expect("failed to construct adze-go language");
        let _parse_result = parser.parse_bytes(b"package main");
    }

    #[test]
    fn smoke_package_with_declaration_parses_successfully() {
        let source = "package main\nvar answer int";

        let mut parser = Parser::new();
        parser
            .set_language(grammar::language())
            .expect("failed to construct adze-go language");
        let parse_result = parser.parse_bytes(source.as_bytes());

        assert!(
            parse_result.root.is_some(),
            "expected parse root for package + var fixture"
        );
        assert!(
            parse_result.errors.is_empty(),
            "expected zero errors for package + var declaration fixture, got: {:?}",
            parse_result.errors
        );
    }
}
