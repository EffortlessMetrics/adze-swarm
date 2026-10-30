#[adze::grammar("boolean_expr")]
pub mod grammar {
    /// Boolean expressions with &&, ||, and ! operators.
    /// Precedence (low to high): ||, &&, !
    #[adze::language]
    #[derive(PartialEq, Eq, Debug)]
    pub enum BoolExpr {
        #[adze::leaf(text = "true", transform = |_| true)]
        Literal(bool),

        #[adze::leaf(text = "false", transform = |_| false)]
        LiteralFalse(bool),

        /// Logical OR: lowest precedence, left associative
        #[adze::prec_left(1)]
        Or(Box<BoolExpr>, #[adze::leaf(text = "||")] (), Box<BoolExpr>),

        /// Logical AND: higher than OR, left associative
        #[adze::prec_left(2)]
        And(Box<BoolExpr>, #[adze::leaf(text = "&&")] (), Box<BoolExpr>),
    }

    #[adze::extra]
    struct Whitespace {
        #[adze::leaf(pattern = r"\s")]
        _whitespace: (),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boolean_literals() {
        // `ParseError` is not `PartialEq`, so compare the `Ok` values.
        assert_eq!(
            grammar::parse("true").ok(),
            Some(grammar::BoolExpr::Literal(true))
        );
        assert_eq!(
            grammar::parse("false").ok(),
            Some(grammar::BoolExpr::LiteralFalse(false))
        );
        insta::assert_debug_snapshot!("true_literal", grammar::parse("true"));
        insta::assert_debug_snapshot!("false_literal", grammar::parse("false"));
    }

    #[test]
    fn boolean_and() {
        insta::assert_debug_snapshot!(grammar::parse("true && false"));
    }

    #[test]
    fn boolean_or() {
        insta::assert_debug_snapshot!(grammar::parse("true || false"));
    }

    #[test]
    fn boolean_precedence() {
        // && binds tighter than ||: "true || false && true" => "true || (false && true)"
        insta::assert_debug_snapshot!(grammar::parse("true || false && true"));
    }

    #[test]
    fn boolean_associativity() {
        // Left associative: "true && false && true" => "(true && false) && true"
        insta::assert_debug_snapshot!(grammar::parse("true && false && true"));
    }

    #[test]
    fn boolean_error_cases() {
        assert!(grammar::parse("").is_err());
        assert!(grammar::parse("true &&").is_err());
        assert!(grammar::parse("yes").is_err());
    }
}
