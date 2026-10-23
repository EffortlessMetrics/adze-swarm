// Proc-macro crate is safe code only
#![forbid(unsafe_code)]
#![cfg_attr(feature = "strict_docs", deny(missing_docs))]
#![cfg_attr(not(feature = "strict_docs"), allow(missing_docs))]

//! Procedural macros for adze grammar definition

use quote::ToTokens;
use syn::{ItemMod, parse_macro_input};

mod errors;
mod expansion;
use expansion::*;

#[proc_macro_attribute]
/// Marks the top-level AST node where parsing should start.
///
/// Exactly one type inside an [`macro@grammar`] module must carry this attribute.
/// It can be applied to either a `struct` or an `enum`.
///
/// ## Examples
///
/// As a struct (single production):
/// ```ignore
/// #[adze::language]
/// pub struct Program {
///     statements: Vec<Statement>,
/// }
/// ```
///
/// As an enum (multiple alternatives):
/// ```ignore
/// #[adze::language]
/// pub enum Expr {
///     Number(
///         #[adze::leaf(pattern = r"\d+", try_transform = |v| v.parse::<i32>())]
///         i32
///     ),
///     #[adze::prec_left(1)]
///     Add(Box<Expr>, #[adze::leaf(text = "+")] (), Box<Expr>),
/// }
/// ```
pub fn language(
    _attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    item
}

#[proc_macro_attribute]
/// This annotation marks a node as extra, which can safely be skipped while parsing.
/// This is useful for handling whitespace/newlines/comments.
///
/// ## Example
/// ```ignore
/// #[adze::extra]
/// struct Whitespace {
///     #[adze::leaf(pattern = r"\s")]
///     _whitespace: (),
/// }
/// ```
pub fn extra(
    _attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    item
}

#[proc_macro_attribute]
/// Defines a field which matches a specific token in the source string.
/// The token can be defined by passing one of two arguments
/// - `text`: a string literal that will be exactly matched
/// - `pattern`: a regular expression that will be matched against the source string
///
/// If the resulting token needs to be converted into a richer type at runtime,
/// such as a number, then the `transform` argument can be used to specify a function
/// that will be called with the token's text.
///
/// When the conversion can fail (for example, a digit token that overflows
/// the target integer type), use `try_transform` instead: it returns
/// `Result<T, E>`, and a failure becomes a structured parse diagnostic over
/// the token's byte range instead of panicking. Specifying both `transform`
/// and `try_transform` on one leaf is a compile-time error.
///
/// The attribute can also be applied to a struct or enum variant with no fields.
///
/// ## Examples
///
/// Using the `leaf` attribute on a field:
/// ```ignore
/// Number(
///     #[adze::leaf(pattern = r"\d+", try_transform = |v| v.parse::<u32>())]
///     u32
/// )
/// ```
///
/// Using the attribute on a unit struct or unit enum variant:
/// ```ignore
/// #[adze::leaf(text = "9")]
/// struct BigDigit;
///
/// enum SmallDigit {
///     #[adze::leaf(text = "0")]
///     Zero,
///     #[adze::leaf(text = "1")]
///     One,
/// }
/// ```
///
pub fn leaf(
    _attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    item
}

#[proc_macro_attribute]
/// Defines a field that does not correspond to anything in the input string,
/// such as some metadata. Takes a single, unnamed argument, which is the value
/// used to populate the field at runtime.
///
/// ## Example
/// ```ignore
/// struct MyNode {
///    ...,
///    #[adze::skip(false)]
///    node_visited: bool
/// }
/// ```
pub fn skip(
    _attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    item
}

#[proc_macro_attribute]
/// Defines a precedence level for a non-terminal that has no associativity.
///
/// This annotation takes a single, unnamed parameter, which specifies the precedence level.
/// This is used to resolve conflicts with other non-terminals, so that the one with the higher
/// precedence will bind more tightly (appear lower in the parse tree).
///
/// ## Example
/// ```ignore
/// #[adze::prec(1)]
/// PriorityExpr(Box<Expr>, Box<Expr>)
/// ```
pub fn prec(
    _attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    item
}

#[proc_macro_attribute]
/// Defines a precedence level for a non-terminal that should be left-associative.
/// For example, with subtraction we expect 1 - 2 - 3 to be parsed as (1 - 2) - 3,
/// which corresponds to a left-associativity.
///
/// This annotation takes a single, unnamed parameter, which specifies the precedence level.
/// This is used to resolve conflicts with other non-terminals, so that the one with the higher
/// precedence will bind more tightly (appear lower in the parse tree).
///
/// ## Example
/// ```ignore
/// #[adze::prec_left(1)]
/// Subtract(Box<Expr>, Box<Expr>)
/// ```
pub fn prec_left(
    _attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    item
}

#[proc_macro_attribute]
/// Defines a precedence level for a non-terminal that should be right-associative.
/// For example, with cons we could have 1 :: 2 :: 3 to be parsed as 1 :: (2 :: 3),
/// which corresponds to a right-associativity.
///
/// This annotation takes a single, unnamed parameter, which specifies the precedence level.
/// This is used to resolve conflicts with other non-terminals, so that the one with the higher
/// precedence will bind more tightly (appear lower in the parse tree).
///
/// ## Example
/// ```ignore
/// #[adze::prec_right(1)]
/// Cons(Box<Expr>, Box<Expr>)
/// ```
pub fn prec_right(
    _attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    item
}

#[proc_macro_attribute]
/// On `Vec<_>` typed fields, specifies a non-terminal that should be parsed in between the elements.
/// The `#[adze::repeat]` annotation must be used on the field as well.
///
/// This annotation takes a single, unnamed argument, which specifies a field type to parse. This can
/// either be a reference to another type, or can be defined as a `leaf` field. Generally, the argument
/// is parsed using the same rules as an unnamed field of an enum variant.
///
/// ## Example
/// ```ignore
/// #[adze::delimited(
///     #[adze::leaf(text = ",")]
///     ()
/// )]
/// numbers: Vec<Number>
/// ```
pub fn delimited(
    _attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    item
}

#[proc_macro_attribute]
/// On `Vec<_>` typed fields, specifies additional config for how the repeated elements should
/// be parsed. In particular, this annotation takes the following named arguments:
/// - `non_empty` - if this argument is `true`, then there must be at least one element parsed
///
/// ## Example
/// ```ignore
/// #[adze::repeat(non_empty = true)]
/// numbers: Vec<Number>
/// ```
pub fn repeat(
    _attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    item
}

/// Marks a rule as an external scanner token. External scanners are implemented in separate
/// code and handle context-sensitive tokens like indentation or heredocs.
///
/// ## Example
/// ```ignore
/// #[adze::external]
/// struct IndentToken;
/// ```
#[proc_macro_attribute]
pub fn external(
    _attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    item
}

/// Marks a token as the word token for the grammar. Word tokens are used to handle
/// keywords vs identifiers disambiguation.
///
/// ## Example
/// ```ignore
/// #[adze::word]
/// #[adze::leaf(pattern = r"[a-zA-Z_]\w*")]
/// struct Identifier(String);
/// ```
#[proc_macro_attribute]
pub fn word(
    _attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    item
}

/// Mark a module to be analyzed for an Adze grammar. Takes a single, unnamed argument, which
/// specifies the name of the grammar. This name must be unique across all Adze grammars within
/// a compilation unit.
///
/// The module must contain exactly one type annotated with [`macro@language`] to serve as the
/// parse entry point. Other types in the module define the remaining grammar rules.
///
/// ## Example
/// ```ignore
/// #[adze::grammar("arithmetic")]
/// mod grammar {
///     #[adze::language]
///     pub enum Expr {
///         Number(
///             #[adze::leaf(pattern = r"\d+", try_transform = |v| v.parse::<i32>())]
///             i32
///         ),
///         #[adze::prec_left(1)]
///         Add(
///             Box<Expr>,
///             #[adze::leaf(text = "+")]
///             (),
///             Box<Expr>,
///         ),
///     }
///
///     #[adze::extra]
///     struct Whitespace {
///         #[adze::leaf(pattern = r"\s")]
///         _whitespace: (),
///     }
/// }
/// ```
#[proc_macro_attribute]
pub fn grammar(
    attr: proc_macro::TokenStream,
    input: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let attr_tokens: proc_macro2::TokenStream = attr.into();
    let module: ItemMod = parse_macro_input!(input);
    let expanded = expand_grammar(syn::parse_quote! {
        #[adze::grammar[#attr_tokens]]
        #module
    })
    .map(ToTokens::into_token_stream)
    .unwrap_or_else(syn::Error::into_compile_error);
    proc_macro::TokenStream::from(expanded)
}

#[cfg(test)]
mod tests {
    use std::fs::File;
    use std::io::{Read, Write};
    use std::process::Command;

    use quote::ToTokens;
    use syn::{Result, parse_quote};
    use tempfile::tempdir;

    use super::expand_grammar;

    fn snapshot_name(base: &str) -> String {
        if cfg!(feature = "pure-rust") {
            format!("{base}__pure_rust")
        } else {
            base.to_owned()
        }
    }

    macro_rules! assert_feature_snapshot {
        ($name:literal, $expr:expr $(,)?) => {
            insta::assert_snapshot!(snapshot_name($name), $expr);
        };
    }

    fn rustfmt_code(code: &str) -> String {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("temp.rs");
        let mut file = File::create(file_path.clone()).unwrap();

        writeln!(file, "{code}").unwrap();
        drop(file);

        Command::new("rustfmt")
            .arg(file_path.to_str().unwrap())
            .spawn()
            .unwrap()
            .wait()
            .unwrap();

        let mut file = File::open(file_path).unwrap();
        let mut data = String::new();
        file.read_to_string(&mut data).unwrap();
        drop(file);
        dir.close().unwrap();
        data
    }

    #[test]
    fn enum_transformed_fields() -> Result<()> {
        assert_feature_snapshot!("enum_transformed_fields", rustfmt_code(
            &expand_grammar(parse_quote! {
                #[adze::grammar("test")]
                mod grammar {
                    #[adze::language]
                    pub enum Expression {
                        Number(
                            #[adze::leaf(pattern = r"\d+", transform = |v| v.parse::<i32>().unwrap())]
                            i32
                        ),
                    }
                }
            })?
            .to_token_stream()
            .to_string()
        ));

        Ok(())
    }

    #[test]
    fn leaf_transform_and_try_transform_conflict_is_error() {
        let error = expand_grammar(parse_quote! {
            #[adze::grammar("test")]
            mod grammar {
                #[adze::language]
                pub enum Expression {
                    Number(
                        #[adze::leaf(pattern = r"\d+", transform = |v| v.parse::<i32>().unwrap(), try_transform = |v| v.parse::<i32>())]
                        i32
                    ),
                }
            }
        })
        .expect_err("transform + try_transform must be rejected at expansion time");
        assert!(
            error.to_string().contains("cannot specify both"),
            "conflict error should name both attributes: {error}"
        );
    }

    #[test]
    fn enum_recursive() -> Result<()> {
        assert_feature_snapshot!(
            "enum_recursive",
            rustfmt_code(
                &expand_grammar(parse_quote! {
                    #[adze::grammar("test")]
                    mod grammar {
                        #[adze::language]
                        pub enum Expression {
                            Number(
                                #[adze::leaf(pattern = r"\d+", transform = |v| v.parse().unwrap())]
                                i32
                            ),
                            Neg(
                                #[adze::leaf(text = "-")]
                                (),
                                Box<Expression>
                            ),
                        }
                    }
                })?
                .to_token_stream()
                .to_string()
            )
        );

        Ok(())
    }

    #[test]
    fn enum_prec_left() -> Result<()> {
        assert_feature_snapshot!(
            "enum_prec_left",
            rustfmt_code(
                &expand_grammar(parse_quote! {
                    #[adze::grammar("test")]
                    mod grammar {
                        #[adze::language]
                        pub enum Expression {
                            Number(
                                #[adze::leaf(pattern = r"\d+", transform = |v| v.parse().unwrap())]
                                i32
                            ),
                            #[adze::prec_left(1)]
                            Sub(
                                Box<Expression>,
                                #[adze::leaf(text = "-")]
                                (),
                                Box<Expression>
                            ),
                        }
                    }
                })?
                .to_token_stream()
                .to_string()
            )
        );

        Ok(())
    }

    #[test]
    fn struct_extra() -> Result<()> {
        assert_feature_snapshot!("struct_extra", rustfmt_code(
            &expand_grammar(parse_quote! {
                #[adze::grammar("test")]
                mod grammar {
                    #[adze::language]
                    pub enum Expression {
                        Number(
                            #[adze::leaf(pattern = r"\d+", transform = |v| v.parse().unwrap())] i32,
                        ),
                    }

                    #[adze::extra]
                    struct Whitespace {
                        #[adze::leaf(pattern = r"\s")]
                        _whitespace: (),
                    }
                }
            })?
            .to_token_stream()
            .to_string()
        ));

        Ok(())
    }

    #[test]
    fn grammar_unboxed_field() -> Result<()> {
        assert_feature_snapshot!("grammar_unboxed_field", rustfmt_code(
            &expand_grammar(parse_quote! {
                #[adze::grammar("test")]
                mod grammar {
                    #[adze::language]
                    pub struct Language {
                        e: Expression,
                    }

                    pub enum Expression {
                        Number(
                            #[adze::leaf(pattern = r"\d+", transform = |v: &str| v.parse::<i32>().unwrap())]
                            i32
                        ),
                    }
                }
            })?
            .to_token_stream()
            .to_string()
        ));

        Ok(())
    }

    #[test]
    fn struct_repeat() -> Result<()> {
        assert_feature_snapshot!(
            "struct_repeat",
            rustfmt_code(
                &expand_grammar(parse_quote! {
                    #[adze::grammar("test")]
                    mod grammar {
                        #[adze::language]
                        pub struct NumberList {
                            numbers: Vec<Number>,
                        }

                        pub struct Number {
                            #[adze::leaf(pattern = r"\d+", transform = |v| v.parse().unwrap())]
                            v: i32
                        }

                        #[adze::extra]
                        struct Whitespace {
                            #[adze::leaf(pattern = r"\s")]
                            _whitespace: (),
                        }
                    }
                })?
                .to_token_stream()
                .to_string()
            )
        );

        Ok(())
    }

    #[test]
    fn struct_optional() -> Result<()> {
        assert_feature_snapshot!(
            "struct_optional",
            rustfmt_code(
                &expand_grammar(parse_quote! {
                    #[adze::grammar("test")]
                    mod grammar {
                        #[adze::language]
                        pub struct Language {
                            #[adze::leaf(pattern = r"\d+", transform = |v| v.parse().unwrap())]
                            v: Option<i32>,
                            t: Option<Number>,
                        }

                        pub struct Number {
                            #[adze::leaf(pattern = r"\d+", transform = |v| v.parse().unwrap())]
                            v: i32
                        }
                    }
                })?
                .to_token_stream()
                .to_string()
            )
        );

        Ok(())
    }

    #[test]
    fn enum_with_unamed_vector() -> Result<()> {
        assert_feature_snapshot!(
            "enum_with_unamed_vector",
            rustfmt_code(
                &expand_grammar(parse_quote! {
                    #[adze::grammar("test")]
                    mod grammar {
                        pub struct Number {
                                #[adze::leaf(pattern = r"\d+", transform = |v| v.parse().unwrap())]
                                value: u32
                        }

                        #[adze::language]
                        pub enum Expr {
                            Numbers(
                                #[adze::repeat(non_empty = true)]
                                Vec<Number>
                            )
                        }
                    }
                })?
                .to_token_stream()
                .to_string()
            )
        );

        Ok(())
    }

    #[test]
    fn enum_with_named_field() -> Result<()> {
        assert_feature_snapshot!("enum_with_named_field", rustfmt_code(
            &expand_grammar(parse_quote! {
                #[adze::grammar("test")]
                mod grammar {
                    #[adze::language]
                    pub enum Expr {
                        Number(
                                #[adze::leaf(pattern = r"\d+", transform = |v| v.parse().unwrap())]
                                u32
                        ),
                        Neg {
                            #[adze::leaf(text = "!")]
                            _bang: (),
                            value: Box<Expr>,
                        }
                    }
                }
            })?
            .to_token_stream()
            .to_string()
        ));

        Ok(())
    }

    #[test]
    fn spanned_in_vec() -> Result<()> {
        assert_feature_snapshot!(
            "spanned_in_vec",
            rustfmt_code(
                &expand_grammar(parse_quote! {
                    #[adze::grammar("test")]
                    mod grammar {
                        use adze::Spanned;

                        #[adze::language]
                        pub struct NumberList {
                            numbers: Vec<Spanned<Number>>,
                        }

                        pub struct Number {
                            #[adze::leaf(pattern = r"\d+", transform = |v| v.parse().unwrap())]
                            v: i32
                        }

                        #[adze::extra]
                        struct Whitespace {
                            #[adze::leaf(pattern = r"\s")]
                            _whitespace: (),
                        }
                    }
                })?
                .to_token_stream()
                .to_string()
            )
        );

        Ok(())
    }

    // === Error case tests ===

    #[test]
    fn error_grammar_missing_name() {
        let result = expand_grammar(parse_quote! {
            #[adze::grammar]
            mod grammar {
                #[adze::language]
                pub enum Expr {
                    Number(#[adze::leaf(pattern = r"\d+")] String),
                }
            }
        });
        let err = result.unwrap_err();
        assert!(
            err.to_string().contains("grammar name"),
            "Expected 'grammar name' error, got: {err}"
        );
    }

    #[test]
    fn error_grammar_non_string_name() {
        let result = expand_grammar(parse_quote! {
            #[adze::grammar(42)]
            mod grammar {
                #[adze::language]
                pub enum Expr {
                    Number(#[adze::leaf(pattern = r"\d+")] String),
                }
            }
        });
        let err = result.unwrap_err();
        assert!(
            err.to_string().contains("string literal"),
            "Expected 'string literal' error, got: {err}"
        );
    }

    #[test]
    fn error_grammar_missing_language_attr() {
        let result = expand_grammar(parse_quote! {
            #[adze::grammar("test")]
            mod grammar {
                pub enum Expr {
                    Number(#[adze::leaf(pattern = r"\d+")] String),
                }
            }
        });
        let err = result.unwrap_err();
        assert!(
            err.to_string().contains("adze::language"),
            "Expected 'adze::language' error, got: {err}"
        );
    }

    #[test]
    fn error_grammar_on_non_module() {
        // expand_grammar expects an ItemMod; using parse_quote with a module
        // that has no body simulates the semicolon-only module case
        let result = expand_grammar(parse_quote! {
            #[adze::grammar("test")]
            mod grammar;
        });
        let err = result.unwrap_err();
        assert!(
            err.to_string().contains("inline contents"),
            "Expected 'inline contents' error, got: {err}"
        );
    }

    // === Valid attribute variation tests ===

    #[test]
    fn enum_prec_right() -> Result<()> {
        assert_feature_snapshot!(
            "enum_prec_right",
            rustfmt_code(
                &expand_grammar(parse_quote! {
                    #[adze::grammar("test")]
                    mod grammar {
                        #[adze::language]
                        pub enum Expression {
                            Number(
                                #[adze::leaf(pattern = r"\d+", transform = |v| v.parse().unwrap())]
                                i32
                            ),
                            #[adze::prec_right(1)]
                            Cons(
                                Box<Expression>,
                                #[adze::leaf(text = "::")]
                                (),
                                Box<Expression>
                            ),
                        }
                    }
                })?
                .to_token_stream()
                .to_string()
            )
        );
        Ok(())
    }

    #[test]
    fn enum_prec_no_assoc() -> Result<()> {
        assert_feature_snapshot!(
            "enum_prec_no_assoc",
            rustfmt_code(
                &expand_grammar(parse_quote! {
                    #[adze::grammar("test")]
                    mod grammar {
                        #[adze::language]
                        pub enum Expression {
                            Number(
                                #[adze::leaf(pattern = r"\d+", transform = |v| v.parse().unwrap())]
                                i32
                            ),
                            #[adze::prec(2)]
                            Compare(
                                Box<Expression>,
                                #[adze::leaf(text = "==")]
                                (),
                                Box<Expression>
                            ),
                        }
                    }
                })?
                .to_token_stream()
                .to_string()
            )
        );
        Ok(())
    }

    #[test]
    fn struct_delimited_repeat() -> Result<()> {
        assert_feature_snapshot!(
            "struct_delimited_repeat",
            rustfmt_code(
                &expand_grammar(parse_quote! {
                    #[adze::grammar("test")]
                    mod grammar {
                        #[adze::language]
                        pub struct NumberList {
                            #[adze::delimited(
                                #[adze::leaf(text = ",")]
                                ()
                            )]
                            numbers: Vec<Number>,
                        }

                        pub struct Number {
                            #[adze::leaf(pattern = r"\d+", transform = |v| v.parse().unwrap())]
                            v: i32
                        }
                    }
                })?
                .to_token_stream()
                .to_string()
            )
        );
        Ok(())
    }

    #[test]
    fn struct_with_skip_field() -> Result<()> {
        assert_feature_snapshot!(
            "struct_with_skip_field",
            rustfmt_code(
                &expand_grammar(parse_quote! {
                    #[adze::grammar("test")]
                    mod grammar {
                        #[adze::language]
                        pub struct MyNode {
                            #[adze::leaf(pattern = r"\d+", transform = |v| v.parse().unwrap())]
                            value: i32,
                            #[adze::skip(false)]
                            visited: bool,
                        }
                    }
                })?
                .to_token_stream()
                .to_string()
            )
        );
        Ok(())
    }

    #[test]
    fn struct_repeat_non_empty() -> Result<()> {
        assert_feature_snapshot!(
            "struct_repeat_non_empty",
            rustfmt_code(
                &expand_grammar(parse_quote! {
                    #[adze::grammar("test")]
                    mod grammar {
                        #[adze::language]
                        pub struct NumberList {
                            #[adze::repeat(non_empty = true)]
                            numbers: Vec<Number>,
                        }

                        pub struct Number {
                            #[adze::leaf(pattern = r"\d+", transform = |v| v.parse().unwrap())]
                            v: i32
                        }
                    }
                })?
                .to_token_stream()
                .to_string()
            )
        );
        Ok(())
    }

    #[test]
    fn leaf_text_literal() -> Result<()> {
        assert_feature_snapshot!(
            "leaf_text_literal",
            rustfmt_code(
                &expand_grammar(parse_quote! {
                    #[adze::grammar("test")]
                    mod grammar {
                        #[adze::language]
                        pub enum Token {
                            #[adze::leaf(text = "+")]
                            Plus,
                            #[adze::leaf(text = "-")]
                            Minus,
                        }
                    }
                })?
                .to_token_stream()
                .to_string()
            )
        );
        Ok(())
    }

    #[test]
    fn leaf_pattern_only() -> Result<()> {
        assert_feature_snapshot!(
            "leaf_pattern_only",
            rustfmt_code(
                &expand_grammar(parse_quote! {
                    #[adze::grammar("test")]
                    mod grammar {
                        #[adze::language]
                        pub struct Identifier {
                            #[adze::leaf(pattern = r"[a-zA-Z_]\w*")]
                            name: String,
                        }
                    }
                })?
                .to_token_stream()
                .to_string()
            )
        );
        Ok(())
    }

    #[test]
    fn grammar_with_word_attr() -> Result<()> {
        assert_feature_snapshot!(
            "grammar_with_word_attr",
            rustfmt_code(
                &expand_grammar(parse_quote! {
                    #[adze::grammar("test")]
                    mod grammar {
                        #[adze::language]
                        pub struct Code {
                            ident: Identifier,
                        }

                        #[adze::word]
                        pub struct Identifier {
                            #[adze::leaf(pattern = r"[a-zA-Z_]\w*")]
                            name: String,
                        }
                    }
                })?
                .to_token_stream()
                .to_string()
            )
        );
        Ok(())
    }

    #[test]
    fn grammar_with_external_attr() -> Result<()> {
        assert_feature_snapshot!(
            "grammar_with_external_attr",
            rustfmt_code(
                &expand_grammar(parse_quote! {
                    #[adze::grammar("test")]
                    mod grammar {
                        #[adze::language]
                        pub struct Code {
                            #[adze::leaf(pattern = r"\w+")]
                            token: String,
                        }

                        #[adze::external]
                        struct IndentToken {
                            #[adze::leaf(pattern = r"\t+")]
                            _indent: (),
                        }
                    }
                })?
                .to_token_stream()
                .to_string()
            )
        );
        Ok(())
    }

    #[test]
    fn enum_unit_variant_leaf() -> Result<()> {
        // Unit variants with leaf attributes are a special code path
        assert_feature_snapshot!(
            "enum_unit_variant_leaf",
            rustfmt_code(
                &expand_grammar(parse_quote! {
                    #[adze::grammar("test")]
                    mod grammar {
                        #[adze::language]
                        pub enum Keyword {
                            #[adze::leaf(text = "if")]
                            If,
                            #[adze::leaf(text = "else")]
                            Else,
                            #[adze::leaf(text = "while")]
                            While,
                        }
                    }
                })?
                .to_token_stream()
                .to_string()
            )
        );
        Ok(())
    }

    #[test]
    fn multiple_extra_types() -> Result<()> {
        assert_feature_snapshot!(
            "multiple_extra_types",
            rustfmt_code(
                &expand_grammar(parse_quote! {
                    #[adze::grammar("test")]
                    mod grammar {
                        #[adze::language]
                        pub struct Code {
                            #[adze::leaf(pattern = r"\w+")]
                            token: String,
                        }

                        #[adze::extra]
                        struct Whitespace {
                            #[adze::leaf(pattern = r"\s")]
                            _ws: (),
                        }

                        #[adze::extra]
                        struct Comment {
                            #[adze::leaf(pattern = r"//[^\n]*")]
                            _comment: (),
                        }
                    }
                })?
                .to_token_stream()
                .to_string()
            )
        );
        Ok(())
    }
}
