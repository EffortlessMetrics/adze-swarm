//! Comprehensive tests for macro token processing and attribute parsing.
//!
//! This test suite covers the syn/quote/proc_macro2 ecosystem that adze-macro depends on.
//! 64 tests across 8 categories:
//! 1. Token stream processing (8 tests)
//! 2. Identifier handling (8 tests)
//! 3. Type parsing (8 tests)
//! 4. DeriveInput parsing (8 tests)
//! 5. Attribute processing (8 tests)
//! 6. Struct analysis (8 tests)
//! 7. Enum analysis (8 tests)
//! 8. Code generation (8 tests)

use proc_macro2::{Span, TokenStream};
use quote::{ToTokens, quote};
use syn::{Attribute, Data, DeriveInput, Fields, Ident, Lit, Meta, Type, parse_quote, parse_str};

// ============================================================================
// CATEGORY 1: Token Stream Processing (8 tests)
// ============================================================================

#[test]
fn token_stream_create_from_string() {
    let ts: TokenStream = "fn hello() {}".parse().expect("parse failed");
    assert!(!ts.is_empty());
}

#[test]
fn token_stream_empty() {
    let ts: TokenStream = "".parse().expect("parse failed");
    assert!(ts.is_empty());
}

#[test]
fn token_stream_with_identifiers() {
    let ts: TokenStream = "foo bar baz".parse().expect("parse failed");
    let display_str = ts.to_string();
    assert!(display_str.contains("foo"));
    assert!(display_str.contains("bar"));
    assert!(display_str.contains("baz"));
}

#[test]
fn token_stream_with_literals() {
    let ts: TokenStream = r#""hello" 42 3.14"#.parse().expect("parse failed");
    let display_str = ts.to_string();
    assert!(display_str.contains("hello"));
    assert!(display_str.contains("42"));
}

#[test]
fn token_stream_with_punctuation() {
    let ts: TokenStream = "a + b * (c - d)".parse().expect("parse failed");
    let display_str = ts.to_string();
    assert!(display_str.contains("+"));
    assert!(display_str.contains("*"));
}

#[test]
fn token_stream_composition() {
    let ts1: TokenStream = "fn".parse().expect("parse failed");
    let ts2: TokenStream = "hello".parse().expect("parse failed");
    let ts3: TokenStream = "()".parse().expect("parse failed");

    let composed = quote! {
        #ts1 #ts2 #ts3 {}
    };

    let display_str = composed.to_string();
    assert!(display_str.contains("fn"));
    assert!(display_str.contains("hello"));
}

#[test]
fn token_stream_display() {
    let ts: TokenStream = "struct Point { x : i32 , y : i32 }".parse().unwrap();
    let display_str = ts.to_string();
    assert!(!display_str.is_empty());
    // Verify it contains expected components
    assert!(display_str.contains("struct") || display_str.contains("Point"));
}

#[test]
fn token_stream_iteration() {
    let ts: TokenStream = "a b c".parse().expect("parse failed");
    let count = ts.into_iter().count();
    assert_eq!(count, 3);
}

// ============================================================================
// CATEGORY 2: Identifier Handling (8 tests)
// ============================================================================

#[test]
fn ident_parse_simple() {
    let ident: Ident = parse_str("hello").expect("parse failed");
    assert_eq!(ident.to_string(), "hello");
}

#[test]
fn ident_parse_snake_case() {
    let ident: Ident = parse_str("my_variable").expect("parse failed");
    assert_eq!(ident.to_string(), "my_variable");
}

#[test]
fn ident_parse_camel_case() {
    let ident: Ident = parse_str("MyStruct").expect("parse failed");
    assert_eq!(ident.to_string(), "MyStruct");
}

#[test]
fn ident_parse_with_underscore_prefix() {
    let ident: Ident = parse_str("_internal").expect("parse failed");
    assert_eq!(ident.to_string(), "_internal");
}

#[test]
fn ident_parse_numeric_suffix() {
    let ident: Ident = parse_str("var42").expect("parse failed");
    assert_eq!(ident.to_string(), "var42");
}

#[test]
fn ident_equality() {
    let ident1: Ident = parse_str("test").expect("parse failed");
    let ident2: Ident = parse_str("test").expect("parse failed");
    assert_eq!(ident1, ident2);
}

#[test]
fn ident_display() {
    let ident: Ident = parse_str("example").expect("parse failed");
    let display_str = ident.to_string();
    assert_eq!(display_str, "example");
}

#[test]
fn ident_span() {
    let ident: Ident = parse_str("test").expect("parse failed");
    let span = ident.span();
    // Span does not implement PartialEq; just verify we got a span
    let _ = span;
}

// ============================================================================
// CATEGORY 3: Type Parsing (8 tests)
// ============================================================================

#[test]
fn type_parse_simple() {
    let ty: Type = parse_str("i32").expect("parse failed");
    let display_str = ty.to_token_stream().to_string();
    assert!(display_str.contains("i32"));
}

#[test]
fn type_parse_generic() {
    let ty: Type = parse_str("Vec<String>").expect("parse failed");
    let display_str = ty.to_token_stream().to_string();
    assert!(display_str.contains("Vec"));
    assert!(display_str.contains("String"));
}

#[test]
fn type_parse_path() {
    let ty: Type = parse_str("std::collections::HashMap").expect("parse failed");
    let display_str = ty.to_token_stream().to_string();
    assert!(display_str.contains("HashMap"));
}

#[test]
fn type_parse_reference() {
    let ty: Type = parse_str("&str").expect("parse failed");
    let display_str = ty.to_token_stream().to_string();
    assert!(display_str.contains("str"));
}

#[test]
fn type_parse_tuple() {
    let ty: Type = parse_str("(i32, String)").expect("parse failed");
    let display_str = ty.to_token_stream().to_string();
    assert!(display_str.contains("i32"));
    assert!(display_str.contains("String"));
}

#[test]
fn type_parse_array() {
    let ty: Type = parse_str("[u8; 32]").expect("parse failed");
    let display_str = ty.to_token_stream().to_string();
    assert!(display_str.contains("u8"));
    assert!(display_str.contains("32"));
}

#[test]
fn type_parse_option() {
    let ty: Type = parse_str("Option<i32>").expect("parse failed");
    let display_str = ty.to_token_stream().to_string();
    assert!(display_str.contains("Option"));
    assert!(display_str.contains("i32"));
}

#[test]
fn type_parse_vec() {
    let ty: Type = parse_str("Vec<u8>").expect("parse failed");
    let display_str = ty.to_token_stream().to_string();
    assert!(display_str.contains("Vec"));
    assert!(display_str.contains("u8"));
}

// ============================================================================
// CATEGORY 4: DeriveInput Parsing (8 tests)
// ============================================================================

#[test]
fn derive_input_parse_struct() {
    let input: DeriveInput = parse_quote! {
        struct Point {
            x: i32,
            y: i32,
        }
    };
    assert_eq!(input.ident.to_string(), "Point");
}

#[test]
fn derive_input_parse_enum() {
    let input: DeriveInput = parse_quote! {
        enum Color {
            Red,
            Green,
            Blue,
        }
    };
    assert_eq!(input.ident.to_string(), "Color");
}

#[test]
fn derive_input_parse_struct_fields() {
    let input: DeriveInput = parse_quote! {
        struct Person {
            name: String,
            age: u32,
        }
    };
    if let Data::Struct(data) = &input.data {
        if let Fields::Named(fields) = &data.fields {
            assert_eq!(fields.named.len(), 2);
        } else {
            panic!("Expected named fields");
        }
    } else {
        panic!("Expected struct");
    }
}

#[test]
fn derive_input_parse_enum_variants() {
    let input: DeriveInput = parse_quote! {
        enum Result {
            Ok(i32),
            Err(String),
        }
    };
    if let Data::Enum(data) = &input.data {
        assert_eq!(data.variants.len(), 2);
    } else {
        panic!("Expected enum");
    }
}

#[test]
fn derive_input_parse_generics() {
    let input: DeriveInput = parse_quote! {
        struct Container<T> {
            item: T,
        }
    };
    assert_eq!(input.generics.params.len(), 1);
}

#[test]
fn derive_input_parse_attributes() {
    let input: DeriveInput = parse_quote! {
        #[derive(Debug)]
        struct Item;
    };
    assert!(!input.attrs.is_empty());
}

#[test]
fn derive_input_parse_visibility() {
    let input: DeriveInput = parse_quote! {
        pub struct Public;
    };
    let is_public = matches!(&input.vis, syn::Visibility::Public(_));
    assert!(is_public);
}

#[test]
fn derive_input_parse_where_clause() {
    let input: DeriveInput = parse_quote! {
        struct Constrained<T>
        where
            T: Clone,
        {
            data: T,
        }
    };
    assert!(input.generics.where_clause.is_some());
}

// ============================================================================
// CATEGORY 5: Attribute Processing (8 tests)
// ============================================================================

#[test]
fn attribute_parse_single() {
    let attr: Attribute = parse_quote! { #[doc = "test"] };
    assert_eq!(
        attr.path().get_ident().map(|i| i.to_string()).as_deref(),
        Some("doc")
    );
}

#[test]
fn attribute_parse_multiple() {
    let input: DeriveInput = parse_quote! {
        #[doc = "first"]
        #[deprecated]
        struct Item;
    };
    assert_eq!(input.attrs.len(), 2);
}

#[test]
fn attribute_parse_nested() {
    let attr: Attribute = parse_quote! { #[allow(dead_code)] };
    let meta = &attr.meta;
    match meta {
        Meta::List(_) => {} // Expected for nested attributes
        _ => panic!("Expected list meta for nested attribute"),
    }
}

#[test]
fn attribute_with_literal_value() {
    let attr: Attribute = parse_quote! { #[doc = "documentation"] };
    let meta = &attr.meta;
    match meta {
        Meta::NameValue(nv) => match &nv.value {
            syn::Expr::Lit(expr_lit) => match &expr_lit.lit {
                Lit::Str(lit_str) => {
                    let value = lit_str.value();
                    assert_eq!(value, "documentation");
                }
                _ => panic!("Expected string literal"),
            },
            _ => panic!("Expected literal expression"),
        },
        _ => panic!("Expected NameValue meta"),
    }
}

#[test]
fn attribute_with_path() {
    let attr: Attribute = parse_quote! { #[derive(Debug)] };
    let path = attr.path();
    assert_eq!(
        path.get_ident().map(|i| i.to_string()).as_deref(),
        Some("derive")
    );
}

#[test]
fn attribute_with_list() {
    let attr: Attribute = parse_quote! { #[attribute(a, b, c)] };
    match &attr.meta {
        Meta::List(_) => {} // Success - it's a list
        _ => panic!("Expected list meta"),
    }
}

#[test]
fn attribute_name_extraction() {
    let attr: Attribute = parse_quote! { #[my_attr = "value"] };
    let name = attr.path().get_ident().map(|i| i.to_string());
    assert_eq!(name.as_deref(), Some("my_attr"));
}

#[test]
fn attribute_value_extraction() {
    let attr: Attribute = parse_quote! { #[test_attr = "test_value"] };
    if let Meta::NameValue(nv) = &attr.meta
        && let syn::Expr::Lit(expr_lit) = &nv.value
        && let Lit::Str(lit_str) = &expr_lit.lit
    {
        assert_eq!(lit_str.value(), "test_value");
        return;
    }
    panic!("Could not extract attribute value");
}

// ============================================================================
// CATEGORY 6: Struct Analysis (8 tests)
// ============================================================================

#[test]
fn struct_analysis_named_fields() {
    let input: DeriveInput = parse_quote! {
        struct Point {
            x: i32,
            y: i32,
        }
    };
    if let Data::Struct(data) = &input.data {
        if let Fields::Named(fields) = &data.fields {
            assert_eq!(fields.named.len(), 2);
            let field_names: Vec<_> = fields
                .named
                .iter()
                .map(|f| f.ident.as_ref().unwrap().to_string())
                .collect();
            assert_eq!(field_names, vec!["x", "y"]);
        } else {
            panic!("Expected named fields");
        }
    } else {
        panic!("Expected struct");
    }
}

#[test]
fn struct_analysis_unnamed_fields() {
    let input: DeriveInput = parse_quote! {
        struct Pair(i32, String);
    };
    if let Data::Struct(data) = &input.data {
        if let Fields::Unnamed(fields) = &data.fields {
            assert_eq!(fields.unnamed.len(), 2);
        } else {
            panic!("Expected unnamed fields");
        }
    } else {
        panic!("Expected struct");
    }
}

#[test]
fn struct_analysis_unit_struct() {
    let input: DeriveInput = parse_quote! {
        struct Unit;
    };
    if let Data::Struct(data) = &input.data {
        if let Fields::Unit = &data.fields {
            // Success
        } else {
            panic!("Expected unit fields");
        }
    } else {
        panic!("Expected struct");
    }
}

#[test]
fn struct_analysis_field_types() {
    let input: DeriveInput = parse_quote! {
        struct Data {
            count: i32,
            name: String,
        }
    };
    if let Data::Struct(data) = &input.data {
        if let Fields::Named(fields) = &data.fields {
            let types: Vec<_> = fields
                .named
                .iter()
                .map(|f| f.ty.to_token_stream().to_string())
                .collect();
            assert_eq!(types.len(), 2);
            assert!(types[0].contains("i32"));
            assert!(types[1].contains("String"));
        } else {
            panic!("Expected named fields");
        }
    } else {
        panic!("Expected struct");
    }
}

#[test]
fn struct_analysis_field_names() {
    let input: DeriveInput = parse_quote! {
        struct Record {
            id: u64,
            value: i32,
        }
    };
    if let Data::Struct(data) = &input.data {
        if let Fields::Named(fields) = &data.fields {
            let names: Vec<_> = fields
                .named
                .iter()
                .map(|f| f.ident.as_ref().unwrap().to_string())
                .collect();
            assert_eq!(names, vec!["id", "value"]);
        } else {
            panic!("Expected named fields");
        }
    } else {
        panic!("Expected struct");
    }
}

#[test]
fn struct_analysis_field_attributes() {
    let input: DeriveInput = parse_quote! {
        struct Item {
            #[serde(rename = "item_id")]
            id: u32,
        }
    };
    if let Data::Struct(data) = &input.data {
        if let Fields::Named(fields) = &data.fields {
            let field = fields.named.iter().next().unwrap();
            assert!(!field.attrs.is_empty());
        } else {
            panic!("Expected named fields");
        }
    } else {
        panic!("Expected struct");
    }
}

#[test]
fn struct_analysis_generic_struct() {
    let input: DeriveInput = parse_quote! {
        struct Box<T> {
            value: T,
        }
    };
    assert_eq!(input.generics.params.len(), 1);
    if let Data::Struct(data) = &input.data {
        if let Fields::Named(fields) = &data.fields {
            assert_eq!(fields.named.len(), 1);
        } else {
            panic!("Expected named fields");
        }
    } else {
        panic!("Expected struct");
    }
}

#[test]
fn struct_analysis_with_lifetime() {
    let input: DeriveInput = parse_quote! {
        struct Reference<'a> {
            data: &'a str,
        }
    };
    assert_eq!(input.generics.params.len(), 1);
}

// ============================================================================
// CATEGORY 7: Enum Analysis (8 tests)
// ============================================================================

#[test]
fn enum_analysis_simple_enum() {
    let input: DeriveInput = parse_quote! {
        enum Status {
            Active,
            Inactive,
        }
    };
    if let Data::Enum(data) = &input.data {
        assert_eq!(data.variants.len(), 2);
        let variant_names: Vec<_> = data.variants.iter().map(|v| v.ident.to_string()).collect();
        assert_eq!(variant_names, vec!["Active", "Inactive"]);
    } else {
        panic!("Expected enum");
    }
}

#[test]
fn enum_analysis_enum_with_data() {
    let input: DeriveInput = parse_quote! {
        enum Message {
            Quit,
            Move { x: i32, y: i32 },
            Write(String),
        }
    };
    if let Data::Enum(data) = &input.data {
        assert_eq!(data.variants.len(), 3);
    } else {
        panic!("Expected enum");
    }
}

#[test]
fn enum_analysis_variant_names() {
    let input: DeriveInput = parse_quote! {
        enum Color {
            Red,
            Green,
            Blue,
        }
    };
    if let Data::Enum(data) = &input.data {
        let names: Vec<_> = data.variants.iter().map(|v| v.ident.to_string()).collect();
        assert_eq!(names, vec!["Red", "Green", "Blue"]);
    } else {
        panic!("Expected enum");
    }
}

#[test]
fn enum_analysis_variant_fields() {
    let input: DeriveInput = parse_quote! {
        enum Wrapped {
            Some(i32),
            None,
        }
    };
    if let Data::Enum(data) = &input.data {
        let variant = &data.variants[0];
        if let Fields::Unnamed(_) = &variant.fields {
            // Success - it has unnamed fields
        } else {
            panic!("Expected unnamed fields for Some variant");
        }
    } else {
        panic!("Expected enum");
    }
}

#[test]
fn enum_analysis_discriminants() {
    let input: DeriveInput = parse_quote! {
        enum Priority {
            High = 10,
            Medium = 5,
            Low = 1,
        }
    };
    if let Data::Enum(data) = &input.data {
        assert_eq!(data.variants.len(), 3);
        let has_discriminants = data.variants.iter().all(|v| v.discriminant.is_some());
        assert!(has_discriminants);
    } else {
        panic!("Expected enum");
    }
}

#[test]
fn enum_analysis_enum_with_attributes() {
    let input: DeriveInput = parse_quote! {
        #[derive(Clone)]
        enum Config {
            #[deprecated]
            OldStyle,
            NewStyle,
        }
    };
    assert!(!input.attrs.is_empty());
    if let Data::Enum(data) = &input.data {
        let has_attr_variant = data.variants.iter().any(|v| !v.attrs.is_empty());
        assert!(has_attr_variant);
    } else {
        panic!("Expected enum");
    }
}

#[test]
fn enum_analysis_generic_enum() {
    let input: DeriveInput = parse_quote! {
        enum Result<T, E> {
            Ok(T),
            Err(E),
        }
    };
    assert_eq!(input.generics.params.len(), 2);
    if let Data::Enum(data) = &input.data {
        assert_eq!(data.variants.len(), 2);
    } else {
        panic!("Expected enum");
    }
}

// ============================================================================
// CATEGORY 8: Code Generation (8 tests)
// ============================================================================

#[test]
fn code_gen_quote_simple_struct() {
    let name = Ident::new("MyStruct", Span::call_site());
    let generated = quote! {
        struct #name {
            field: i32,
        }
    };
    let output = generated.to_string();
    assert!(output.contains("struct"));
    assert!(output.contains("MyStruct"));
    assert!(output.contains("field"));
}

#[test]
fn code_gen_quote_with_interpolation() {
    let field_name = Ident::new("my_field", Span::call_site());
    let field_type: Type = parse_str("String").expect("parse failed");
    let generated = quote! {
        let #field_name: #field_type = String::new();
    };
    let output = generated.to_string();
    assert!(output.contains("my_field"));
    assert!(output.contains("String"));
}

#[test]
fn code_gen_quote_with_iteration() {
    let fields = ["x", "y", "z"];
    let field_names: Vec<Ident> = fields
        .iter()
        .map(|s| Ident::new(s, Span::call_site()))
        .collect();
    let generated = quote! {
        struct Point {
            #(#field_names: i32,)*
        }
    };
    let output = generated.to_string();
    assert!(output.contains("x"));
    assert!(output.contains("y"));
    assert!(output.contains("z"));
}

#[test]
fn code_gen_quote_nested() {
    let struct_name = Ident::new("Container", Span::call_site());
    let item_name = Ident::new("item", Span::call_site());
    let generated = quote! {
        struct #struct_name {
            #item_name: Vec<i32>,
        }
    };
    let output = generated.to_string();
    assert!(output.contains("Container"));
    assert!(output.contains("item"));
    assert!(output.contains("Vec"));
}

#[test]
fn code_gen_quote_function() {
    let func_name = Ident::new("process", Span::call_site());
    let generated = quote! {
        fn #func_name(input: &str) -> String {
            input.to_uppercase()
        }
    };
    let output = generated.to_string();
    assert!(output.contains("fn"));
    assert!(output.contains("process"));
    assert!(output.contains("input"));
}

#[test]
fn code_gen_quote_impl_block() {
    let struct_name = Ident::new("Handler", Span::call_site());
    let method_name = Ident::new("handle", Span::call_site());
    let generated = quote! {
        impl #struct_name {
            fn #method_name(&self) {
                // handler logic
            }
        }
    };
    let output = generated.to_string();
    assert!(output.contains("impl"));
    assert!(output.contains("Handler"));
    assert!(output.contains("handle"));
}

#[test]
fn code_gen_quote_matches_original() {
    let original_code = quote! {
        struct TestStruct {
            field1: i32,
            field2: String,
        }
    };

    // Re-quote the same thing
    let requoted = quote! {
        struct TestStruct {
            field1: i32,
            field2: String,
        }
    };

    assert_eq!(original_code.to_string(), requoted.to_string());
}

#[test]
fn code_gen_quote_deterministic() {
    let names: Vec<Ident> = (0..3)
        .map(|i| Ident::new(&format!("field{}", i), Span::call_site()))
        .collect();

    let gen1 = quote! {
        struct S {
            #(#names: u32,)*
        }
    };

    let gen2 = quote! {
        struct S {
            #(#names: u32,)*
        }
    };

    // Generated code should be deterministic
    assert_eq!(gen1.to_string(), gen2.to_string());
}

#[test]
fn code_gen_quote_with_conditional() {
    let include_debug = true;
    let struct_name = Ident::new("Config", Span::call_site());

    let generated = if include_debug {
        quote! {
            #[derive(Debug)]
            struct #struct_name;
        }
    } else {
        quote! {
            struct #struct_name;
        }
    };

    let output = generated.to_string();
    assert!(output.contains("Config"));
    assert!(output.contains("derive"));
}
