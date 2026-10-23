#[adze::grammar("typed_ast_contract")]
pub mod grammar {
    #[derive(Debug, PartialEq, Eq)]
    #[adze::language]
    pub enum Expr {
        Number(#[adze::leaf(pattern = r"\d+", try_transform = |s| s.parse::<i32>())] i32),

        #[adze::prec_left(1)]
        Add(Box<Expr>, #[adze::leaf(text = "+")] (), Box<Expr>),
    }

    #[adze::extra]
    struct Whitespace {
        #[adze::leaf(pattern = r"\s")]
        _ws: (),
    }
}
