#[adze::grammar("tiny_build_invalidation")]
pub mod grammar {
    #[adze::language]
    #[derive(Debug, PartialEq, Eq)]
    pub struct Word {
        #[adze::leaf(text = "hello")]
        pub value: (),
    }
}
