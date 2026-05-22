//! Attribute parsing v5 tests for the adze-macro crate.
//!
//! 55+ tests across 8 categories:
//! 1. Parse struct attributes (derive, repr, doc)
//! 2. Parse field-level attributes
//! 3. adze-specific attribute detection
//! 4. Attribute argument parsing (key = value, paths)
//! 5. Multiple attributes on same item
//! 6. Enum variant attributes
//! 7. Generic type handling with attributes
//! 8. Edge cases: empty attributes, nested attributes, macro attributes

use std::collections::HashSet;

use adze_common::{
    FieldThenParams, NameValueExpr, filter_inner_type, try_extract_inner_type, wrap_leaf_type,
};
use proc_macro2::TokenStream;
use quote::{ToTokens, quote};
use syn::{Attribute, DeriveInput, Fields, ItemEnum, ItemStruct, Meta, Type, parse_quote, parse2};

// ── Helpers ──────────────────────────────────────────────────────────────────

fn parse_struct(tokens: TokenStream) -> ItemStruct {
    parse2(tokens).expect("failed to parse struct")
}

fn parse_enum(tokens: TokenStream) -> ItemEnum {
    parse2(tokens).expect("failed to parse enum")
}

fn parse_derive(code: &str) -> DeriveInput {
    syn::parse_str::<DeriveInput>(code).expect("failed to parse DeriveInput")
}

fn parse_type(code: &str) -> Type {
    syn::parse_str::<Type>(code).expect("failed to parse Type")
}

fn is_adze_attr(attr: &Attribute, name: &str) -> bool {
    let segments: Vec<_> = attr.path().segments.iter().collect();
    segments.len() == 2 && segments[0].ident == "adze" && segments[1].ident == name
}

fn adze_attr_names(attrs: &[Attribute]) -> Vec<String> {
    attrs
        .iter()
        .filter_map(|attr| {
            let segs: Vec<_> = attr.path().segments.iter().collect();
            if segs.len() == 2 && segs[0].ident == "adze" {
                Some(segs[1].ident.to_string())
            } else {
                None
            }
        })
        .collect()
}

fn ts(ty: &Type) -> String {
    ty.to_token_stream().to_string()
}

fn is_parameterized(ty: &Type) -> bool {
    if let Type::Path(p) = ty
        && let Some(seg) = p.path.segments.last()
    {
        return matches!(seg.arguments, syn::PathArguments::AngleBracketed(_));
    }
    false
}

fn named_fields(s: &ItemStruct) -> Vec<&syn::Field> {
    match &s.fields {
        Fields::Named(f) => f.named.iter().collect(),
        _ => panic!("expected named fields"),
    }
}

fn empty_skip_set() -> HashSet<&'static str> {
    HashSet::new()
}

fn box_skip_set() -> HashSet<&'static str> {
    HashSet::from(["Box", "Arc"])
}

fn vec_option_skip() -> HashSet<&'static str> {
    HashSet::from(["Vec", "Option"])
}

// ============================================================================
// 1. Parse struct attributes — derive, repr, doc (8 tests)
// ============================================================================

#[test]
fn struct_derive_debug_attribute() {
    let s = parse_struct(quote! {
        #[derive(Debug)]
        struct Foo { x: i32 }
    });
    assert_eq!(s.attrs.len(), 1);
    assert!(matches!(s.attrs[0].meta, Meta::List(_)));
}

#[test]
fn struct_derive_multiple_traits() {
    let s = parse_struct(quote! {
        #[derive(Debug, Clone, PartialEq, Eq)]
        struct Bar { name: String }
    });
    assert_eq!(s.attrs.len(), 1);
    let tokens = s.attrs[0].meta.to_token_stream().to_string();
    assert!(tokens.contains("Debug"));
    assert!(tokens.contains("Clone"));
    assert!(tokens.contains("PartialEq"));
}

#[test]
fn struct_repr_c_attribute() {
    let s = parse_struct(quote! {
        #[repr(C)]
        struct Repr { a: u32, b: u64 }
    });
    let meta_str = s.attrs[0].meta.to_token_stream().to_string();
    assert!(meta_str.contains("repr"));
    assert!(meta_str.contains("C"));
}

#[test]
fn struct_repr_packed_attribute() {
    let s = parse_struct(quote! {
        #[repr(packed)]
        struct Packed { a: u8, b: u16 }
    });
    let meta_str = s.attrs[0].meta.to_token_stream().to_string();
    assert!(meta_str.contains("packed"));
}

#[test]
fn struct_doc_attribute() {
    let s = parse_struct(quote! {
        #[doc = "A documented struct"]
        struct Documented { val: i32 }
    });
    assert_eq!(s.attrs.len(), 1);
    assert!(matches!(s.attrs[0].meta, Meta::NameValue(_)));
}

#[test]
fn struct_doc_comment_via_derive_input() {
    let d = parse_derive("/// A comment\nstruct Commented { val: i32 }");
    assert!(!d.attrs.is_empty());
    let meta_str = d.attrs[0].meta.to_token_stream().to_string();
    assert!(meta_str.contains("doc"));
}

#[test]
fn struct_cfg_attr_attribute() {
    let s = parse_struct(quote! {
        #[cfg_attr(test, derive(Debug))]
        struct Configured { x: i32 }
    });
    let meta_str = s.attrs[0].meta.to_token_stream().to_string();
    assert!(meta_str.contains("cfg_attr"));
    assert!(meta_str.contains("derive"));
}

#[test]
fn struct_cfg_attribute() {
    let s = parse_struct(quote! {
        #[cfg(test)]
        struct TestOnly { flag: bool }
    });
    let meta_str = s.attrs[0].meta.to_token_stream().to_string();
    assert!(meta_str.contains("cfg"));
    assert!(meta_str.contains("test"));
}

// ============================================================================
// 2. Parse field-level attributes (8 tests)
// ============================================================================

#[test]
fn field_with_serde_rename() {
    let s = parse_struct(quote! {
        struct Renamed {
            #[serde(rename = "other_name")]
            field: String,
        }
    });
    let field = &named_fields(&s)[0];
    assert_eq!(field.attrs.len(), 1);
    let meta_str = field.attrs[0].meta.to_token_stream().to_string();
    assert!(meta_str.contains("serde"));
}

#[test]
fn field_adze_leaf_text() {
    let s = parse_struct(quote! {
        struct Plus {
            #[adze::leaf(text = "+")]
            symbol: (),
        }
    });
    let field = &named_fields(&s)[0];
    assert!(is_adze_attr(&field.attrs[0], "leaf"));
}

#[test]
fn field_adze_leaf_pattern() {
    let s = parse_struct(quote! {
        struct Digits {
            #[adze::leaf(pattern = r"\d+")]
            raw: String,
        }
    });
    let field = &named_fields(&s)[0];
    assert!(is_adze_attr(&field.attrs[0], "leaf"));
    assert!(matches!(field.attrs[0].meta, Meta::List(_)));
}

#[test]
fn field_doc_comment() {
    let d = parse_derive("struct WithDoc { /// A field\n val: i32 }");
    match &d.data {
        syn::Data::Struct(data) => {
            let fields: Vec<_> = data.fields.iter().collect();
            assert!(!fields[0].attrs.is_empty());
        }
        _ => panic!("expected struct"),
    }
}

#[test]
fn field_cfg_attribute() {
    let s = parse_struct(quote! {
        struct Configured {
            #[cfg(test)]
            spare: u64,
        }
    });
    let field = &named_fields(&s)[0];
    assert_eq!(field.attrs.len(), 1);
    let meta_str = field.attrs[0].meta.to_token_stream().to_string();
    assert!(meta_str.contains("cfg"));
    assert!(meta_str.contains("test"));
}

#[test]
fn field_type_extraction_option() {
    let s = parse_struct(quote! {
        struct Opt { value: Option<String> }
    });
    let field = &named_fields(&s)[0];
    let (inner, extracted) = try_extract_inner_type(&field.ty, "Option", &empty_skip_set());
    assert!(extracted);
    assert_eq!(ts(&inner), "String");
}

#[test]
fn field_type_extraction_vec() {
    let s = parse_struct(quote! {
        struct Items { list: Vec<Item> }
    });
    let field = &named_fields(&s)[0];
    let (inner, extracted) = try_extract_inner_type(&field.ty, "Vec", &empty_skip_set());
    assert!(extracted);
    assert_eq!(ts(&inner), "Item");
}

#[test]
fn field_type_extraction_box_through_skip() {
    let s = parse_struct(quote! {
        struct Wrap { inner: Box<Vec<Token>> }
    });
    let field = &named_fields(&s)[0];
    let (inner, extracted) = try_extract_inner_type(&field.ty, "Vec", &box_skip_set());
    assert!(extracted);
    assert_eq!(ts(&inner), "Token");
}

// ============================================================================
// 3. adze-specific attribute detection (7 tests)
// ============================================================================

#[test]
fn detect_adze_grammar() {
    let attr: Attribute = parse_quote!(#[adze::grammar("test")]);
    assert!(is_adze_attr(&attr, "grammar"));
    assert!(!is_adze_attr(&attr, "language"));
}

#[test]
fn detect_adze_language() {
    let attr: Attribute = parse_quote!(#[adze::language]);
    assert!(is_adze_attr(&attr, "language"));
}

#[test]
fn detect_adze_leaf() {
    let attr: Attribute = parse_quote!(#[adze::leaf(text = "fn")]);
    assert!(is_adze_attr(&attr, "leaf"));
}

#[test]
fn detect_adze_extra() {
    let attr: Attribute = parse_quote!(#[adze::extra]);
    assert!(is_adze_attr(&attr, "extra"));
}

#[test]
fn detect_adze_word() {
    let attr: Attribute = parse_quote!(#[adze::word]);
    assert!(is_adze_attr(&attr, "word"));
}

#[test]
fn reject_non_adze_derive() {
    let attr: Attribute = parse_quote!(#[derive(Clone)]);
    assert!(!is_adze_attr(&attr, "grammar"));
    assert!(!is_adze_attr(&attr, "leaf"));
}

#[test]
fn reject_single_segment_attribute() {
    let attr: Attribute = parse_quote!(#[inline]);
    assert!(!is_adze_attr(&attr, "inline"));
}

// ============================================================================
// 4. Attribute argument parsing — key=value, paths (8 tests)
// ============================================================================

#[test]
fn name_value_expr_string_literal() {
    let nve: NameValueExpr = parse_quote!(text = "hello");
    assert_eq!(nve.path.to_string(), "text");
}

#[test]
fn name_value_expr_integer_literal() {
    let nve: NameValueExpr = parse_quote!(precedence = 5);
    assert_eq!(nve.path.to_string(), "precedence");
}

#[test]
fn name_value_expr_bool_value() {
    let nve: NameValueExpr = parse_quote!(non_empty = true);
    assert_eq!(nve.path.to_string(), "non_empty");
    let expr_str = nve.expr.to_token_stream().to_string();
    assert_eq!(expr_str, "true");
}

#[test]
fn name_value_expr_raw_string() {
    let nve: NameValueExpr = parse_quote!(pattern = r"\d+");
    assert_eq!(nve.path.to_string(), "pattern");
}

#[test]
fn field_then_params_type_only() {
    let ftp: FieldThenParams = parse_quote!(String);
    assert!(ftp.comma.is_none());
    assert!(ftp.params.is_empty());
}

#[test]
fn field_then_params_with_one_param() {
    let ftp: FieldThenParams = parse_quote!(String, name = "test");
    assert!(ftp.comma.is_some());
    assert_eq!(ftp.params.len(), 1);
    assert_eq!(ftp.params[0].path.to_string(), "name");
}

#[test]
fn field_then_params_with_two_params() {
    let ftp: FieldThenParams = parse_quote!(u32, min = 0, max = 100);
    assert_eq!(ftp.params.len(), 2);
    assert_eq!(ftp.params[0].path.to_string(), "min");
    assert_eq!(ftp.params[1].path.to_string(), "max");
}

#[test]
fn meta_name_value_doc_attribute() {
    let attr: Attribute = parse_quote!(#[doc = "documentation"]);
    assert!(matches!(attr.meta, Meta::NameValue(_)));
    if let Meta::NameValue(nv) = &attr.meta {
        assert_eq!(nv.path.to_token_stream().to_string(), "doc");
    }
}

// ============================================================================
// 5. Multiple attributes on same item (7 tests)
// ============================================================================

#[test]
fn multiple_attrs_derive_and_adze() {
    let s = parse_struct(quote! {
        #[derive(Debug, Clone)]
        #[adze::language]
        struct Root { value: i32 }
    });
    assert_eq!(s.attrs.len(), 2);
    let adze_names = adze_attr_names(&s.attrs);
    assert_eq!(adze_names, ["language"]);
}

#[test]
fn multiple_attrs_three_adze_on_struct() {
    let s = parse_struct(quote! {
        #[adze::language]
        #[adze::word]
        #[adze::extra]
        struct Triple {}
    });
    let names = adze_attr_names(&s.attrs);
    assert_eq!(names, ["language", "word", "extra"]);
}

#[test]
fn multiple_attrs_mixed_doc_derive_adze() {
    let s = parse_struct(quote! {
        #[doc = "some doc"]
        #[derive(Debug)]
        #[adze::language]
        #[cfg(test)]
        struct Mixed { val: u8 }
    });
    assert_eq!(s.attrs.len(), 4);
    let adze_names = adze_attr_names(&s.attrs);
    assert_eq!(adze_names, ["language"]);
}

#[test]
fn multiple_attrs_on_field() {
    let s = parse_struct(quote! {
        struct Multi {
            #[adze::repeat(non_empty = true)]
            #[adze::delimited(
                #[adze::leaf(text = ",")]
                ()
            )]
            items: Vec<Item>,
        }
    });
    let field = &named_fields(&s)[0];
    let names = adze_attr_names(&field.attrs);
    assert_eq!(names, ["repeat", "delimited"]);
}

#[test]
fn multiple_attrs_preserved_in_roundtrip() {
    let s: ItemStruct = parse_quote! {
        #[adze::language]
        #[adze::extra]
        struct Roundtrip { x: i32 }
    };
    let tokens = s.to_token_stream();
    let reparsed: ItemStruct = parse2(tokens).unwrap();
    let names = adze_attr_names(&reparsed.attrs);
    assert_eq!(names, ["language", "extra"]);
}

#[test]
fn multiple_attrs_on_enum() {
    let item = parse_enum(quote! {
        #[derive(Debug)]
        #[adze::grammar("test")]
        enum Expr { Lit(i32) }
    });
    assert_eq!(item.attrs.len(), 2);
    let adze_names = adze_attr_names(&item.attrs);
    assert_eq!(adze_names, ["grammar"]);
}

#[test]
fn multiple_fields_each_with_attr() {
    let s = parse_struct(quote! {
        struct Pair {
            #[adze::leaf(text = "(")]
            open: (),
            #[adze::leaf(text = ")")]
            close: (),
        }
    });
    let fields = named_fields(&s);
    assert!(is_adze_attr(&fields[0].attrs[0], "leaf"));
    assert!(is_adze_attr(&fields[1].attrs[0], "leaf"));
}

// ============================================================================
// 6. Enum variant attributes (7 tests)
// ============================================================================

#[test]
fn variant_adze_prec_left() {
    let item = parse_enum(quote! {
        enum Expr {
            #[adze::prec_left(1)]
            Add(Box<Expr>, Box<Expr>),
        }
    });
    assert!(is_adze_attr(&item.variants[0].attrs[0], "prec_left"));
}

#[test]
fn variant_adze_prec_right() {
    let item = parse_enum(quote! {
        enum Expr {
            #[adze::prec_right(2)]
            Pow(Box<Expr>, Box<Expr>),
        }
    });
    assert!(is_adze_attr(&item.variants[0].attrs[0], "prec_right"));
}

#[test]
fn variant_unit_with_leaf() {
    let item = parse_enum(quote! {
        enum Op {
            #[adze::leaf(text = "+")]
            Plus,
            #[adze::leaf(text = "-")]
            Minus,
        }
    });
    assert!(matches!(item.variants[0].fields, Fields::Unit));
    assert!(is_adze_attr(&item.variants[0].attrs[0], "leaf"));
    assert!(is_adze_attr(&item.variants[1].attrs[0], "leaf"));
}

#[test]
fn variant_names_extraction() {
    let item = parse_enum(quote! {
        enum Token { Ident, Number, Whitespace, Comment }
    });
    let names: Vec<_> = item.variants.iter().map(|v| v.ident.to_string()).collect();
    assert_eq!(names, ["Ident", "Number", "Whitespace", "Comment"]);
}

#[test]
fn variant_tuple_field_types() {
    let item = parse_enum(quote! {
        enum Expr {
            Binary(Box<Expr>, Op, Box<Expr>),
        }
    });
    match &item.variants[0].fields {
        Fields::Unnamed(f) => {
            assert_eq!(f.unnamed.len(), 3);
            assert_eq!(ts(&f.unnamed[1].ty), "Op");
        }
        _ => panic!("expected unnamed fields"),
    }
}

#[test]
fn variant_named_fields() {
    let item = parse_enum(quote! {
        enum Stmt {
            Assignment { target: Ident, value: Expr },
        }
    });
    match &item.variants[0].fields {
        Fields::Named(f) => {
            assert_eq!(f.named.len(), 2);
            let names: Vec<_> = f
                .named
                .iter()
                .map(|field| field.ident.as_ref().unwrap().to_string())
                .collect();
            assert_eq!(names, ["target", "value"]);
        }
        _ => panic!("expected named fields"),
    }
}

#[test]
fn variant_with_doc_and_adze_attrs() {
    let item = parse_enum(quote! {
        enum Expr {
            #[doc = "addition"]
            #[adze::prec_left(1)]
            Add(Box<Expr>, Box<Expr>),
        }
    });
    let variant = &item.variants[0];
    assert_eq!(variant.attrs.len(), 2);
    let adze_names = adze_attr_names(&variant.attrs);
    assert_eq!(adze_names, ["prec_left"]);
}

// ============================================================================
// 7. Generic type handling with attributes (7 tests)
// ============================================================================

#[test]
fn generic_vec_is_parameterized() {
    let ty = parse_type("Vec<i32>");
    assert!(is_parameterized(&ty));
}

#[test]
fn generic_option_is_parameterized() {
    let ty = parse_type("Option<String>");
    assert!(is_parameterized(&ty));
}

#[test]
fn generic_simple_not_parameterized() {
    let ty = parse_type("i32");
    assert!(!is_parameterized(&ty));
}

#[test]
fn generic_reference_not_parameterized() {
    let ty: Type = parse_quote!(&str);
    assert!(!is_parameterized(&ty));
}

#[test]
fn generic_extract_through_box_arc() {
    let ty = parse_type("Arc<Box<Vec<Token>>>");
    let skip = HashSet::from(["Arc", "Box"]);
    let (inner, extracted) = try_extract_inner_type(&ty, "Vec", &skip);
    assert!(extracted);
    assert_eq!(ts(&inner), "Token");
}

#[test]
fn generic_filter_strips_box() {
    let ty = parse_type("Box<String>");
    let filtered = filter_inner_type(&ty, &box_skip_set());
    assert_eq!(ts(&filtered), "String");
}

#[test]
fn generic_wrap_leaf_simple_type() {
    let ty = parse_type("i32");
    let wrapped = wrap_leaf_type(&ty, &empty_skip_set());
    assert_eq!(ts(&wrapped), "adze :: WithLeaf < i32 >");
}

// ============================================================================
// 8. Edge cases (11 tests)
// ============================================================================

#[test]
fn edge_empty_struct_no_attrs() {
    let s: ItemStruct = parse_quote! { struct Empty {} };
    assert!(s.attrs.is_empty());
    match &s.fields {
        Fields::Named(f) => assert!(f.named.is_empty()),
        _ => panic!("expected named fields"),
    }
}

#[test]
fn edge_unit_struct() {
    let s: ItemStruct = parse_quote! { struct Marker; };
    assert!(matches!(s.fields, Fields::Unit));
}

#[test]
fn edge_enum_no_variants() {
    let item = parse_enum(quote! { enum Void {} });
    assert!(item.variants.is_empty());
}

#[test]
fn edge_deeply_nested_generics() {
    let ty = parse_type("Vec<Option<Box<HashMap<String, Vec<i32>>>>>");
    assert!(is_parameterized(&ty));
}

#[test]
fn edge_wrap_leaf_skips_vec_wraps_inner() {
    let ty = parse_type("Vec<String>");
    let wrapped = wrap_leaf_type(&ty, &vec_option_skip());
    assert_eq!(ts(&wrapped), "Vec < adze :: WithLeaf < String > >");
}

#[test]
fn edge_wrap_leaf_nested_option_vec() {
    let ty = parse_type("Option<Vec<Expr>>");
    let wrapped = wrap_leaf_type(&ty, &vec_option_skip());
    assert_eq!(ts(&wrapped), "Option < Vec < adze :: WithLeaf < Expr > > >");
}

#[test]
fn edge_filter_nested_box_arc() {
    let ty = parse_type("Box<Arc<String>>");
    let filtered = filter_inner_type(&ty, &box_skip_set());
    assert_eq!(ts(&filtered), "String");
}

#[test]
fn edge_extract_returns_original_when_no_match() {
    let ty = parse_type("HashMap<String, i32>");
    let (inner, extracted) = try_extract_inner_type(&ty, "Vec", &empty_skip_set());
    assert!(!extracted);
    assert_eq!(ts(&inner), "HashMap < String , i32 >");
}

#[test]
fn edge_token_stream_empty() {
    let stream = TokenStream::new();
    assert!(stream.is_empty());
}

#[test]
fn edge_struct_with_lifetime_generic() {
    let s = parse_struct(quote! {
        struct Borrowed<'a> { data: &'a str }
    });
    assert_eq!(s.generics.params.len(), 1);
}

#[test]
fn edge_struct_generic_with_where_clause() {
    let s = parse_struct(quote! {
        struct Bounded<T> where T: Send + Sync { value: T }
    });
    assert!(s.generics.where_clause.is_some());
}
