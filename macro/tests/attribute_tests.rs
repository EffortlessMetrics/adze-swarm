//! Comprehensive attribute processing tests for adze macros.
//!
//! Tests attribute recognition, parameter parsing, and placement validation
//! for all `#[adze::*]` attributes using `syn` to parse token streams.

use adze_common::NameValueExpr;
use proc_macro2::TokenStream;
use quote::quote;
use syn::punctuated::Punctuated;
use syn::{Attribute, Fields, ItemEnum, ItemMod, ItemStruct, Token, parse_quote};

// ── Helpers ─────────────────────────────────────────────────────────────────

/// Check whether an attribute path matches `adze::<name>`.
fn is_adze_attr(attr: &Attribute, name: &str) -> bool {
    let segs: Vec<_> = attr.path().segments.iter().collect();
    segs.len() == 2 && segs[0].ident == "adze" && segs[1].ident == name
}

/// Collect all `adze::*` attribute names from an attribute list.
fn adze_attr_names(attrs: &[Attribute]) -> Vec<String> {
    attrs
        .iter()
        .filter_map(|a| {
            let segs: Vec<_> = a.path().segments.iter().collect();
            if segs.len() == 2 && segs[0].ident == "adze" {
                Some(segs[1].ident.to_string())
            } else {
                None
            }
        })
        .collect()
}

/// Parse a token stream as an `ItemMod`.
fn parse_mod(tokens: TokenStream) -> ItemMod {
    syn::parse2(tokens).expect("failed to parse module")
}

// ── 1–7: Attribute recognition ──────────────────────────────────────────────

#[test]
fn recognize_grammar_attribute() {
    let m = parse_mod(quote! {
        #[adze::grammar("test")]
        mod grammar {}
    });
    assert!(m.attrs.iter().any(|a| is_adze_attr(a, "grammar")));
}

#[test]
fn recognize_language_attribute() {
    let s: ItemStruct = parse_quote! {
        #[adze::language]
        pub struct Root { }
    };
    assert!(s.attrs.iter().any(|a| is_adze_attr(a, "language")));
}

#[test]
fn recognize_leaf_attribute_on_field() {
    let s: ItemStruct = parse_quote! {
        pub struct Tok {
            #[adze::leaf(pattern = r"\d+")]
            value: String,
        }
    };
    let field = s.fields.iter().next().unwrap();
    assert!(field.attrs.iter().any(|a| is_adze_attr(a, "leaf")));
}

#[test]
fn recognize_word_attribute() {
    let s: ItemStruct = parse_quote! {
        #[adze::word]
        pub struct Ident {
            name: String,
        }
    };
    assert!(s.attrs.iter().any(|a| is_adze_attr(a, "word")));
}

#[test]
fn recognize_prec_left_attribute() {
    let e: ItemEnum = parse_quote! {
        pub enum Expr {
            #[adze::prec_left(1)]
            Add(Box<Expr>, Box<Expr>),
        }
    };
    let variant = &e.variants[0];
    assert!(variant.attrs.iter().any(|a| is_adze_attr(a, "prec_left")));
}

#[test]
fn recognize_prec_right_attribute() {
    let e: ItemEnum = parse_quote! {
        pub enum Expr {
            #[adze::prec_right(2)]
            Cons(Box<Expr>, Box<Expr>),
        }
    };
    let variant = &e.variants[0];
    assert!(variant.attrs.iter().any(|a| is_adze_attr(a, "prec_right")));
}

#[test]
fn recognize_prec_attribute() {
    let e: ItemEnum = parse_quote! {
        pub enum Expr {
            #[adze::prec(3)]
            Cmp(Box<Expr>, Box<Expr>),
        }
    };
    let variant = &e.variants[0];
    assert!(variant.attrs.iter().any(|a| is_adze_attr(a, "prec")));
}

// ── 8–9: Attribute parameters ───────────────────────────────────────────────

#[test]
fn parse_precedence_value_parameter() {
    let e: ItemEnum = parse_quote! {
        pub enum Expr {
            #[adze::prec_left(42)]
            Add(Box<Expr>, Box<Expr>),
        }
    };
    let attr = e.variants[0]
        .attrs
        .iter()
        .find(|a| is_adze_attr(a, "prec_left"))
        .unwrap();
    // The precedence value should parse as a single expression
    let expr: syn::Expr = attr.parse_args().unwrap();
    if let syn::Expr::Lit(lit) = &expr
        && let syn::Lit::Int(int) = &lit.lit
    {
        assert_eq!(int.base10_parse::<i32>().unwrap(), 42);
        return;
    }
    panic!("Expected integer literal 42");
}

#[test]
fn parse_multiple_named_parameters() {
    let s: ItemStruct = parse_quote! {
        pub struct Tok {
            #[adze::leaf(pattern = r"\d+", transform = |v| v.parse().unwrap())]
            value: i32,
        }
    };
    let attr = s
        .fields
        .iter()
        .next()
        .unwrap()
        .attrs
        .iter()
        .find(|a| is_adze_attr(a, "leaf"))
        .unwrap();

    let params = attr
        .parse_args_with(Punctuated::<NameValueExpr, Token![,]>::parse_terminated)
        .unwrap();

    let names: Vec<String> = params.iter().map(|p| p.path.to_string()).collect();
    assert!(names.contains(&"pattern".to_string()));
    assert!(names.contains(&"transform".to_string()));
    assert_eq!(params.len(), 2);
}

// ── 10–13: Attribute placement on different items ───────────────────────────

#[test]
fn attribute_on_struct() {
    let s: ItemStruct = parse_quote! {
        #[adze::language]
        pub struct Program {
            expr: Box<Expr>,
        }
    };
    assert_eq!(adze_attr_names(&s.attrs), vec!["language"]);
}

#[test]
fn attribute_on_enum() {
    let e: ItemEnum = parse_quote! {
        #[adze::language]
        pub enum Expr {
            Num(i32),
        }
    };
    assert_eq!(adze_attr_names(&e.attrs), vec!["language"]);
}

#[test]
fn attribute_on_enum_variant() {
    let e: ItemEnum = parse_quote! {
        pub enum Expr {
            #[adze::prec_left(1)]
            Add(Box<Expr>, Box<Expr>),
            #[adze::prec_right(2)]
            Pow(Box<Expr>, Box<Expr>),
        }
    };
    assert_eq!(adze_attr_names(&e.variants[0].attrs), vec!["prec_left"]);
    assert_eq!(adze_attr_names(&e.variants[1].attrs), vec!["prec_right"]);
}

#[test]
fn attribute_on_field() {
    let s: ItemStruct = parse_quote! {
        pub struct Num {
            #[adze::leaf(pattern = r"\d+")]
            value: String,
            #[adze::skip(0)]
            counter: usize,
        }
    };
    let fields: Vec<_> = s.fields.iter().collect();
    assert_eq!(adze_attr_names(&fields[0].attrs), vec!["leaf"]);
    assert_eq!(adze_attr_names(&fields[1].attrs), vec!["skip"]);
}

// ── 14–15: Invalid / unknown attribute detection ────────────────────────────

#[test]
fn detect_non_adze_attribute_is_not_recognized() {
    let s: ItemStruct = parse_quote! {
        #[derive(Debug)]
        #[serde(rename_all = "camelCase")]
        #[adze::language]
        pub struct Root {}
    };
    // Only the adze attribute should be recognized
    let adze_attrs: Vec<_> = s
        .attrs
        .iter()
        .filter(|a| {
            a.path()
                .segments
                .iter()
                .next()
                .map(|seg| seg.ident == "adze")
                .unwrap_or(false)
        })
        .collect();
    assert_eq!(adze_attrs.len(), 1);
    assert!(is_adze_attr(adze_attrs[0], "language"));
}

#[test]
fn detect_unknown_adze_attribute() {
    // Parsing succeeds at the syn level; unknown attributes are detected
    // during macro expansion, not during parsing. Verify parsing works
    // and the attribute is present but unrecognized by our helper.
    let s: ItemStruct = parse_quote! {
        #[adze::nonexistent_attr]
        pub struct Foo {}
    };
    let names = adze_attr_names(&s.attrs);
    assert_eq!(names, vec!["nonexistent_attr"]);
    // Known attributes do not include "nonexistent_attr"
    let known = [
        "grammar",
        "language",
        "leaf",
        "word",
        "prec",
        "prec_left",
        "prec_right",
        "extra",
        "skip",
        "delimited",
        "repeat",
        "external",
    ];
    assert!(!known.contains(&names[0].as_str()));
}

// ── 16–20: Additional attribute recognition ─────────────────────────────────

#[test]
fn recognize_extra_attribute() {
    let s: ItemStruct = parse_quote! {
        #[adze::extra]
        struct Whitespace {
            _ws: (),
        }
    };
    assert!(s.attrs.iter().any(|a| is_adze_attr(a, "extra")));
}

#[test]
fn recognize_skip_attribute() {
    let s: ItemStruct = parse_quote! {
        pub struct Node {
            #[adze::skip(false)]
            visited: bool,
        }
    };
    let field = s.fields.iter().next().unwrap();
    assert!(field.attrs.iter().any(|a| is_adze_attr(a, "skip")));
}

#[test]
fn recognize_delimited_attribute() {
    let s: ItemStruct = parse_quote! {
        pub struct List {
            #[adze::delimited(
                #[adze::leaf(text = ",")]
                ()
            )]
            items: Vec<Item>,
        }
    };
    let field = s.fields.iter().next().unwrap();
    assert!(field.attrs.iter().any(|a| is_adze_attr(a, "delimited")));
}

#[test]
fn recognize_repeat_attribute() {
    let s: ItemStruct = parse_quote! {
        pub struct List {
            #[adze::repeat(non_empty = true)]
            items: Vec<Number>,
        }
    };
    let field = s.fields.iter().next().unwrap();
    assert!(field.attrs.iter().any(|a| is_adze_attr(a, "repeat")));
}

#[test]
fn recognize_external_attribute() {
    let s: ItemStruct = parse_quote! {
        #[adze::external]
        struct IndentToken;
    };
    assert!(s.attrs.iter().any(|a| is_adze_attr(a, "external")));
}

// ── 21–26: Combined and edge-case tests ─────────────────────────────────────

#[test]
fn multiple_attributes_on_same_item() {
    let s: ItemStruct = parse_quote! {
        #[adze::word]
        #[adze::language]
        pub struct Identifier {
            name: String,
        }
    };
    let names = adze_attr_names(&s.attrs);
    assert_eq!(names.len(), 2);
    assert!(names.contains(&"word".to_string()));
    assert!(names.contains(&"language".to_string()));
}

#[test]
fn multiple_attributes_on_same_field() {
    let s: ItemStruct = parse_quote! {
        pub struct List {
            #[adze::repeat(non_empty = true)]
            #[adze::delimited(
                #[adze::leaf(text = ",")]
                ()
            )]
            items: Vec<Number>,
        }
    };
    let field = s.fields.iter().next().unwrap();
    let names = adze_attr_names(&field.attrs);
    assert_eq!(names.len(), 2);
    assert!(names.contains(&"repeat".to_string()));
    assert!(names.contains(&"delimited".to_string()));
}

#[test]
fn leaf_with_text_parameter() {
    let e: ItemEnum = parse_quote! {
        pub enum Op {
            #[adze::leaf(text = "+")]
            Plus,
        }
    };
    let attr = e.variants[0]
        .attrs
        .iter()
        .find(|a| is_adze_attr(a, "leaf"))
        .unwrap();
    let params = attr
        .parse_args_with(Punctuated::<NameValueExpr, Token![,]>::parse_terminated)
        .unwrap();
    assert_eq!(params.len(), 1);
    assert_eq!(params[0].path.to_string(), "text");
}

#[test]
fn leaf_with_pattern_parameter() {
    let s: ItemStruct = parse_quote! {
        pub struct Num {
            #[adze::leaf(pattern = r"\d+")]
            value: String,
        }
    };
    let attr = s
        .fields
        .iter()
        .next()
        .unwrap()
        .attrs
        .iter()
        .find(|a| is_adze_attr(a, "leaf"))
        .unwrap();
    let params = attr
        .parse_args_with(Punctuated::<NameValueExpr, Token![,]>::parse_terminated)
        .unwrap();
    assert_eq!(params.len(), 1);
    assert_eq!(params[0].path.to_string(), "pattern");
}

#[test]
fn leaf_with_transform_parameter() {
    let s: ItemStruct = parse_quote! {
        pub struct Num {
            #[adze::leaf(pattern = r"\d+", transform = |v| v.parse::<i32>().unwrap())]
            value: i32,
        }
    };
    let attr = s
        .fields
        .iter()
        .next()
        .unwrap()
        .attrs
        .iter()
        .find(|a| is_adze_attr(a, "leaf"))
        .unwrap();
    let params = attr
        .parse_args_with(Punctuated::<NameValueExpr, Token![,]>::parse_terminated)
        .unwrap();
    let names: Vec<String> = params.iter().map(|p| p.path.to_string()).collect();
    assert!(names.contains(&"pattern".to_string()));
    assert!(names.contains(&"transform".to_string()));
}

#[test]
fn grammar_attribute_with_string_name() {
    let m = parse_mod(quote! {
        #[adze::grammar("arithmetic")]
        mod grammar {}
    });
    let attr = m.attrs.iter().find(|a| is_adze_attr(a, "grammar")).unwrap();
    let expr: syn::Expr = attr.parse_args().unwrap();
    if let syn::Expr::Lit(syn::ExprLit {
        lit: syn::Lit::Str(s),
        ..
    }) = expr
    {
        assert_eq!(s.value(), "arithmetic");
    } else {
        panic!("Expected string literal grammar name");
    }
}

// ── 27–30: Structural preservation and edge cases ───────────────────────────

#[test]
fn attributes_do_not_alter_struct_fields() {
    let s: ItemStruct = parse_quote! {
        #[adze::language]
        pub struct Program {
            #[adze::leaf(pattern = r"\w+")]
            name: String,
            count: usize,
        }
    };
    let field_names: Vec<_> = s
        .fields
        .iter()
        .map(|f| f.ident.as_ref().unwrap().to_string())
        .collect();
    assert_eq!(field_names, vec!["name", "count"]);
}

#[test]
fn attributes_do_not_alter_enum_variants() {
    let e: ItemEnum = parse_quote! {
        #[adze::language]
        pub enum Expr {
            #[adze::prec_left(1)]
            Add(Box<Expr>, Box<Expr>),
            #[adze::prec_right(2)]
            Pow(Box<Expr>, Box<Expr>),
            Lit(i32),
        }
    };
    let variant_names: Vec<_> = e.variants.iter().map(|v| v.ident.to_string()).collect();
    assert_eq!(variant_names, vec!["Add", "Pow", "Lit"]);
}

#[test]
fn unit_variant_with_leaf_text() {
    let e: ItemEnum = parse_quote! {
        pub enum Keyword {
            #[adze::leaf(text = "if")]
            If,
            #[adze::leaf(text = "else")]
            Else,
        }
    };
    for variant in &e.variants {
        assert!(variant.attrs.iter().any(|a| is_adze_attr(a, "leaf")));
        assert!(matches!(variant.fields, Fields::Unit));
    }
}

#[test]
fn unit_struct_with_leaf() {
    let s: ItemStruct = parse_quote! {
        #[adze::leaf(text = "9")]
        struct BigDigit;
    };
    assert!(s.attrs.iter().any(|a| is_adze_attr(a, "leaf")));
    assert!(matches!(s.fields, Fields::Unit));
}

#[test]
fn repeat_non_empty_parameter() {
    let s: ItemStruct = parse_quote! {
        pub struct List {
            #[adze::repeat(non_empty = true)]
            items: Vec<Number>,
        }
    };
    let attr = s
        .fields
        .iter()
        .next()
        .unwrap()
        .attrs
        .iter()
        .find(|a| is_adze_attr(a, "repeat"))
        .unwrap();
    let params = attr
        .parse_args_with(Punctuated::<NameValueExpr, Token![,]>::parse_terminated)
        .unwrap();
    assert_eq!(params.len(), 1);
    assert_eq!(params[0].path.to_string(), "non_empty");
}

#[test]
fn skip_attribute_with_value() {
    let s: ItemStruct = parse_quote! {
        pub struct Node {
            #[adze::skip(false)]
            visited: bool,
        }
    };
    let attr = s
        .fields
        .iter()
        .next()
        .unwrap()
        .attrs
        .iter()
        .find(|a| is_adze_attr(a, "skip"))
        .unwrap();
    let expr: syn::Expr = attr.parse_args().unwrap();
    if let syn::Expr::Lit(syn::ExprLit {
        lit: syn::Lit::Bool(b),
        ..
    }) = expr
    {
        assert!(!b.value);
    } else {
        panic!("Expected bool literal");
    }
}

#[test]
fn grammar_module_content_preserved() {
    let m = parse_mod(quote! {
        #[adze::grammar("test")]
        mod grammar {
            #[adze::language]
            pub struct Root {}

            #[adze::extra]
            struct Whitespace {}
        }
    });
    let (_, items) = m.content.unwrap();
    assert_eq!(items.len(), 2);
}

#[test]
fn mixed_adze_and_derive_attributes() {
    let s: ItemStruct = parse_quote! {
        #[derive(Debug, Clone)]
        #[adze::language]
        #[derive(PartialEq)]
        pub struct Root {
            #[adze::leaf(pattern = r"\w+")]
            name: String,
        }
    };
    // Should have exactly 1 adze attribute and 2 derive attributes
    let adze_count = s
        .attrs
        .iter()
        .filter(|a| {
            a.path()
                .segments
                .iter()
                .next()
                .map(|seg| seg.ident == "adze")
                .unwrap_or(false)
        })
        .count();
    let derive_count = s
        .attrs
        .iter()
        .filter(|a| a.path().is_ident("derive"))
        .count();
    assert_eq!(adze_count, 1);
    assert_eq!(derive_count, 2);
}

#[test]
fn all_precedence_variants_parseable() {
    let e: ItemEnum = parse_quote! {
        pub enum Expr {
            #[adze::prec(1)]
            A(i32),
            #[adze::prec_left(2)]
            B(i32),
            #[adze::prec_right(3)]
            C(i32),
        }
    };
    let attrs: Vec<String> = e
        .variants
        .iter()
        .flat_map(|v| adze_attr_names(&v.attrs))
        .collect();
    assert_eq!(attrs, vec!["prec", "prec_left", "prec_right"]);

    // Verify each precedence value parses correctly
    for (variant, expected_val) in e.variants.iter().zip([1i32, 2, 3]) {
        let attr = &variant.attrs[0];
        let expr: syn::Expr = attr.parse_args().unwrap();
        if let syn::Expr::Lit(syn::ExprLit {
            lit: syn::Lit::Int(int),
            ..
        }) = expr
        {
            assert_eq!(int.base10_parse::<i32>().unwrap(), expected_val);
        } else {
            panic!("Expected integer literal for {}", variant.ident);
        }
    }
}

#[test]
fn external_on_unit_struct() {
    let s: ItemStruct = parse_quote! {
        #[adze::external]
        struct IndentToken;
    };
    assert!(s.attrs.iter().any(|a| is_adze_attr(a, "external")));
    assert!(matches!(s.fields, Fields::Unit));
}
