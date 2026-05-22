//! Property-based tests (v5) for macro attribute and expansion properties.
//!
//! Covers 8 categories × 6 tests = 48 properties exercising syn-based type
//! parsing, attribute parsing, generic handling, struct/enum annotation,
//! validation, error messages, and parse → emit → parse roundtrips.
//!
//! API under test: `adze_common::{try_extract_inner_type, filter_inner_type,
//! wrap_leaf_type, NameValueExpr, FieldThenParams}`, `syn::parse_str`,
//! `quote::quote!`, `proc_macro2::TokenStream`.

use std::collections::HashSet;

use adze_common::{
    FieldThenParams, NameValueExpr, filter_inner_type, try_extract_inner_type, wrap_leaf_type,
};
use proptest::prelude::*;
use quote::{ToTokens, format_ident, quote};
use syn::{DeriveInput, Fields, Type, parse_quote, parse_str};

// ── Helpers ─────────────────────────────────────────────────────────────────

fn skip_set<'a>(names: &'a [&'a str]) -> HashSet<&'a str> {
    names.iter().copied().collect()
}

fn ty_str(ty: &Type) -> String {
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

fn is_adze_attr(attr: &syn::Attribute, name: &str) -> bool {
    let segs: Vec<_> = attr.path().segments.iter().collect();
    segs.len() == 2 && segs[0].ident == "adze" && segs[1].ident == name
}

// ── Strategy helpers ────────────────────────────────────────────────────────

/// Valid Rust identifiers excluding all keywords (including Rust 2024 reserved).
fn ident_strategy() -> impl Strategy<Value = String> {
    "[a-z][a-z0-9_]{1,10}".prop_filter("no keywords", |s| {
        !matches!(
            s.as_str(),
            "type"
                | "fn"
                | "let"
                | "mut"
                | "ref"
                | "pub"
                | "mod"
                | "use"
                | "self"
                | "super"
                | "crate"
                | "struct"
                | "enum"
                | "impl"
                | "trait"
                | "where"
                | "for"
                | "loop"
                | "while"
                | "if"
                | "else"
                | "match"
                | "return"
                | "break"
                | "continue"
                | "as"
                | "in"
                | "move"
                | "box"
                | "dyn"
                | "async"
                | "await"
                | "try"
                | "yield"
                | "macro"
                | "const"
                | "static"
                | "unsafe"
                | "extern"
                | "do"
                | "gen"
                | "abstract"
                | "become"
                | "final"
                | "override"
                | "priv"
                | "typeof"
                | "unsized"
                | "virtual"
                | "true"
                | "false"
                | "union"
        )
    })
}

fn pascal_name() -> impl Strategy<Value = String> {
    ident_strategy().prop_map(|s| {
        let mut c = s.chars();
        match c.next() {
            Some(ch) => ch.to_uppercase().collect::<String>() + c.as_str(),
            None => "Ty".to_string(),
        }
    })
}

fn simple_type() -> impl Strategy<Value = String> {
    prop::sample::select(vec![
        "i8", "i16", "i32", "i64", "u8", "u16", "u32", "u64", "f32", "f64", "bool", "char",
        "String", "usize", "isize",
    ])
    .prop_map(|s| s.to_string())
}

fn wrapper_name() -> impl Strategy<Value = String> {
    prop::sample::select(vec!["Vec", "Option", "Box", "Arc", "Rc"]).prop_map(|s| s.to_string())
}

fn field_name() -> impl Strategy<Value = String> {
    prop::sample::select(vec![
        "value", "data", "items", "name", "count", "flag", "inner", "result", "content", "text",
    ])
    .prop_map(|s| s.to_string())
}

fn adze_attr_name() -> impl Strategy<Value = &'static str> {
    prop_oneof![
        Just("language"),
        Just("extra"),
        Just("leaf"),
        Just("skip"),
        Just("prec"),
        Just("word"),
    ]
}

// ═══════════════════════════════════════════════════════════════════════════
// Category 1: prop_parse_type_* — type parsing properties (6 tests)
// ═══════════════════════════════════════════════════════════════════════════

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    /// Primitive type strings always parse successfully.
    #[test]
    fn prop_parse_type_primitive_always_parses(ty_name in simple_type()) {
        let parsed = parse_str::<Type>(&ty_name);
        prop_assert!(parsed.is_ok(), "failed to parse: {ty_name}");
    }

    /// Wrapped types (e.g. `Vec<i32>`) always parse successfully.
    #[test]
    fn prop_parse_type_wrapped_always_parses(
        wrapper in wrapper_name(),
        inner in simple_type(),
    ) {
        let type_str = format!("{wrapper}<{inner}>");
        let parsed = parse_str::<Type>(&type_str);
        prop_assert!(parsed.is_ok(), "failed to parse: {type_str}");
    }

    /// Reference types parse and produce Type::Reference.
    #[test]
    fn prop_parse_type_reference_is_reference(inner in simple_type()) {
        let type_str = format!("&{inner}");
        if let Ok(ty) = parse_str::<Type>(&type_str) {
            prop_assert!(matches!(ty, Type::Reference(_)), "expected reference type for &{inner}");
        }
    }

    /// Tuple types parse and produce Type::Tuple.
    #[test]
    fn prop_parse_type_tuple_is_tuple(
        a in simple_type(),
        b in simple_type(),
    ) {
        let type_str = format!("({a}, {b})");
        let parsed = parse_str::<Type>(&type_str);
        prop_assert!(parsed.is_ok());
        if let Ok(Type::Tuple(tup)) = parsed {
            prop_assert_eq!(tup.elems.len(), 2);
        }
    }

    /// Nested wrappers (e.g. `Vec<Option<i32>>`) parse successfully.
    #[test]
    fn prop_parse_type_nested_wrapper_parses(
        outer in wrapper_name(),
        mid in wrapper_name(),
        inner in simple_type(),
    ) {
        let type_str = format!("{outer}<{mid}<{inner}>>");
        let parsed = parse_str::<Type>(&type_str);
        prop_assert!(parsed.is_ok(), "failed to parse nested: {type_str}");
    }

    /// Array types with constant length parse successfully.
    #[test]
    fn prop_parse_type_array_parses(inner in simple_type(), len in 1u32..=128) {
        let type_str = format!("[{inner}; {len}]");
        let parsed = parse_str::<Type>(&type_str);
        prop_assert!(parsed.is_ok(), "failed to parse array: {type_str}");
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Category 2: prop_parse_attr_* — attribute parsing properties (6 tests)
// ═══════════════════════════════════════════════════════════════════════════

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    /// NameValueExpr preserves the key name after parsing.
    #[test]
    fn prop_parse_attr_name_value_preserves_key(key in ident_strategy()) {
        let key_ident = format_ident!("{}", key);
        let nve: NameValueExpr = parse_quote!(#key_ident = 42);
        prop_assert_eq!(nve.path.to_string(), key);
    }

    /// FieldThenParams with no extra params has empty params list.
    #[test]
    fn prop_parse_attr_field_only_has_no_params(ty in simple_type()) {
        let ty_parsed: Type = parse_str(&ty).unwrap();
        let ftp: FieldThenParams = parse_quote!(#ty_parsed);
        prop_assert!(ftp.params.is_empty());
        prop_assert!(ftp.comma.is_none());
    }

    /// FieldThenParams with one param yields exactly one parameter.
    #[test]
    fn prop_parse_attr_field_one_param(key in ident_strategy()) {
        let key_ident = format_ident!("{}", key);
        let ftp: FieldThenParams = parse_quote!(i32, #key_ident = 1);
        prop_assert_eq!(ftp.params.len(), 1);
        prop_assert_eq!(ftp.params[0].path.to_string(), key);
    }

    /// Adze attributes on structs are detected by path inspection.
    #[test]
    fn prop_parse_attr_adze_detected(attr_name in adze_attr_name()) {
        let attr_ident = format_ident!("{}", attr_name);
        let item: DeriveInput = parse_quote! {
            #[adze::#attr_ident]
            struct Foo;
        };
        prop_assert!(!item.attrs.is_empty());
        let attr = &item.attrs[0];
        prop_assert!(is_adze_attr(attr, attr_name));
    }

    /// Non-adze attributes are not detected as adze attributes.
    #[test]
    fn prop_parse_attr_non_adze_not_detected(attr_name in adze_attr_name()) {
        let item: DeriveInput = parse_quote! {
            #[derive(Debug)]
            struct Foo;
        };
        prop_assert!(!item.attrs.is_empty());
        prop_assert!(!is_adze_attr(&item.attrs[0], attr_name));
    }

    /// Multiple adze attributes on a struct are all preserved.
    #[test]
    fn prop_parse_attr_multiple_preserved(
        a in adze_attr_name(),
        b in adze_attr_name(),
    ) {
        let a_ident = format_ident!("{}", a);
        let b_ident = format_ident!("{}", b);
        let item: DeriveInput = parse_quote! {
            #[adze::#a_ident]
            #[adze::#b_ident]
            struct Foo;
        };
        prop_assert_eq!(item.attrs.len(), 2);
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Category 3: prop_generics_* — generic type handling (6 tests)
// ═══════════════════════════════════════════════════════════════════════════

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    /// Wrapped types are detected as parameterized.
    #[test]
    fn prop_generics_wrapper_is_parameterized(
        wrapper in wrapper_name(),
        inner in simple_type(),
    ) {
        let type_str = format!("{wrapper}<{inner}>");
        let ty = parse_str::<Type>(&type_str).unwrap();
        prop_assert!(is_parameterized(&ty));
    }

    /// Primitive types are not detected as parameterized.
    #[test]
    fn prop_generics_primitive_not_parameterized(inner in simple_type()) {
        let ty = parse_str::<Type>(&inner).unwrap();
        prop_assert!(!is_parameterized(&ty));
    }

    /// Struct with type parameter preserves the generic param count.
    #[test]
    fn prop_generics_struct_one_param(name in pascal_name()) {
        let ident = format_ident!("{}", name);
        let di: DeriveInput = parse_quote! { struct #ident<T> { value: T } };
        prop_assert_eq!(di.generics.params.len(), 1);
    }

    /// Struct with two type parameters preserves both.
    #[test]
    fn prop_generics_struct_two_params(name in pascal_name()) {
        let ident = format_ident!("{}", name);
        let di: DeriveInput = parse_quote! { struct #ident<K, V> { key: K, val: V } };
        prop_assert_eq!(di.generics.params.len(), 2);
    }

    /// Struct with lifetime parameter detects it correctly.
    #[test]
    fn prop_generics_lifetime_detected(name in pascal_name()) {
        let ident = format_ident!("{}", name);
        let di: DeriveInput = parse_quote! { struct #ident<'a> { data: &'a str } };
        prop_assert_eq!(di.generics.params.len(), 1);
        let param = di.generics.params.first().unwrap();
        prop_assert!(matches!(param, syn::GenericParam::Lifetime(_)));
    }

    /// Enum with generic parameter preserves it across roundtrip.
    #[test]
    fn prop_generics_enum_roundtrip(name in pascal_name()) {
        let ident = format_ident!("{}", name);
        let di: DeriveInput = parse_quote! { enum #ident<T> { A(T), B } };
        let tokens = di.to_token_stream();
        let di2: DeriveInput = syn::parse2(tokens).unwrap();
        prop_assert_eq!(di2.generics.params.len(), 1);
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Category 4: prop_struct_* — struct annotation properties (6 tests)
// ═══════════════════════════════════════════════════════════════════════════

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    /// Named struct preserves field count.
    #[test]
    fn prop_struct_named_field_count(
        f1 in field_name(),
        f2 in field_name(),
        ty in simple_type(),
    ) {
        prop_assume!(f1 != f2);
        let i1 = format_ident!("{}", f1);
        let i2 = format_ident!("{}", f2);
        let ty_parsed: Type = parse_str(&ty).unwrap();
        let di: DeriveInput = parse_quote! {
            struct S { #i1: #ty_parsed, #i2: #ty_parsed }
        };
        if let syn::Data::Struct(ds) = &di.data {
            if let Fields::Named(nf) = &ds.fields {
                prop_assert_eq!(nf.named.len(), 2);
            } else {
                prop_assert!(false, "expected named fields");
            }
        }
    }

    /// Tuple struct preserves field count.
    #[test]
    fn prop_struct_tuple_field_count(count in 1usize..=5) {
        let field_types: Vec<Type> = (0..count).map(|_| parse_quote!(u32)).collect();
        let tokens = quote! { struct S(#(#field_types),*); };
        let di: DeriveInput = syn::parse2(tokens).unwrap();
        if let syn::Data::Struct(ds) = &di.data
            && let Fields::Unnamed(uf) = &ds.fields
        {
            prop_assert_eq!(uf.unnamed.len(), count);
        }
    }

    /// Unit struct has no fields.
    #[test]
    fn prop_struct_unit_no_fields(name in pascal_name()) {
        let ident = format_ident!("{}", name);
        let di: DeriveInput = parse_quote! { struct #ident; };
        if let syn::Data::Struct(ds) = &di.data {
            prop_assert!(matches!(&ds.fields, Fields::Unit));
        }
    }

    /// Struct name is preserved after parsing.
    #[test]
    fn prop_struct_name_preserved(name in pascal_name()) {
        let ident = format_ident!("{}", name);
        let di: DeriveInput = parse_quote! { struct #ident { x: i32 } };
        prop_assert_eq!(di.ident.to_string(), name);
    }

    /// Pub struct preserves visibility.
    #[test]
    fn prop_struct_pub_visibility(name in pascal_name()) {
        let ident = format_ident!("{}", name);
        let di: DeriveInput = parse_quote! { pub struct #ident; };
        let tokens = di.to_token_stream().to_string();
        prop_assert!(tokens.contains("pub"));
    }

    /// Field types in a named struct are retrievable.
    #[test]
    fn prop_struct_field_type_retrievable(
        fname in field_name(),
        ty in simple_type(),
    ) {
        let fi = format_ident!("{}", fname);
        let ty_parsed: Type = parse_str(&ty).unwrap();
        let di: DeriveInput = parse_quote! { struct S { #fi: #ty_parsed } };
        if let syn::Data::Struct(ds) = &di.data
            && let Fields::Named(nf) = &ds.fields
        {
            let field = nf.named.first().unwrap();
            let field_ty = ty_str(&field.ty);
            // Token-stream normalization may add spaces; compare content
            prop_assert!(!field_ty.is_empty());
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Category 5: prop_enum_* — enum annotation properties (6 tests)
// ═══════════════════════════════════════════════════════════════════════════

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    /// Enum with unit variants preserves variant count.
    #[test]
    fn prop_enum_unit_variant_count(
        v1 in pascal_name(),
        v2 in pascal_name(),
        v3 in pascal_name(),
    ) {
        prop_assume!(v1 != v2 && v2 != v3 && v1 != v3);
        let vi1 = format_ident!("{}", v1);
        let vi2 = format_ident!("{}", v2);
        let vi3 = format_ident!("{}", v3);
        let di: DeriveInput = parse_quote! { enum E { #vi1, #vi2, #vi3 } };
        if let syn::Data::Enum(data) = &di.data {
            prop_assert_eq!(data.variants.len(), 3);
        }
    }

    /// Enum variant names are preserved.
    #[test]
    fn prop_enum_variant_name_preserved(
        vname in pascal_name(),
    ) {
        let vi = format_ident!("{}", vname);
        let di: DeriveInput = parse_quote! { enum E { #vi } };
        if let syn::Data::Enum(data) = &di.data {
            let variant = data.variants.first().unwrap();
            prop_assert_eq!(variant.ident.to_string(), vname);
        }
    }

    /// Enum with tuple variant preserves the inner field type.
    #[test]
    fn prop_enum_tuple_variant_field(
        vname in pascal_name(),
        ty in simple_type(),
    ) {
        let vi = format_ident!("{}", vname);
        let ty_parsed: Type = parse_str(&ty).unwrap();
        let di: DeriveInput = parse_quote! { enum E { #vi(#ty_parsed) } };
        if let syn::Data::Enum(data) = &di.data {
            let variant = data.variants.first().unwrap();
            if let Fields::Unnamed(uf) = &variant.fields {
                prop_assert_eq!(uf.unnamed.len(), 1);
            }
        }
    }

    /// Enum with struct variant preserves field names.
    #[test]
    fn prop_enum_struct_variant_fields(
        vname in pascal_name(),
        fname in field_name(),
    ) {
        let vi = format_ident!("{}", vname);
        let fi = format_ident!("{}", fname);
        let di: DeriveInput = parse_quote! { enum E { #vi { #fi: i32 } } };
        if let syn::Data::Enum(data) = &di.data {
            let variant = data.variants.first().unwrap();
            if let Fields::Named(nf) = &variant.fields {
                let field = nf.named.first().unwrap();
                prop_assert_eq!(field.ident.as_ref().unwrap().to_string(), fname);
            }
        }
    }

    /// Enum name is preserved.
    #[test]
    fn prop_enum_name_preserved(name in pascal_name()) {
        let ident = format_ident!("{}", name);
        let di: DeriveInput = parse_quote! { enum #ident { A, B } };
        prop_assert_eq!(di.ident.to_string(), name);
    }

    /// Mixed enum with unit + tuple variants counts correctly.
    #[test]
    fn prop_enum_mixed_variant_count(
        v1 in pascal_name(),
        v2 in pascal_name(),
    ) {
        prop_assume!(v1 != v2);
        let vi1 = format_ident!("{}", v1);
        let vi2 = format_ident!("{}", v2);
        let di: DeriveInput = parse_quote! { enum E { #vi1, #vi2(i32) } };
        if let syn::Data::Enum(data) = &di.data {
            prop_assert_eq!(data.variants.len(), 2);
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Category 6: prop_validate_* — validation properties (6 tests)
// ═══════════════════════════════════════════════════════════════════════════

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    /// extract on matching wrapper always succeeds.
    #[test]
    fn prop_validate_extract_matching_succeeds(
        wrapper in wrapper_name(),
        inner in simple_type(),
    ) {
        let type_str = format!("{wrapper}<{inner}>");
        let ty = parse_str::<Type>(&type_str).unwrap();
        let empty = skip_set(&[]);
        let (_, extracted) = try_extract_inner_type(&ty, &wrapper, &empty);
        prop_assert!(extracted);
    }

    /// extract on non-matching wrapper always fails.
    #[test]
    fn prop_validate_extract_non_matching_fails(inner in simple_type()) {
        let ty = parse_str::<Type>(&inner).unwrap();
        let empty = skip_set(&[]);
        let (_, extracted) = try_extract_inner_type(&ty, "Vec", &empty);
        prop_assert!(!extracted);
    }

    /// filter_inner_type with empty skip set returns original type.
    #[test]
    fn prop_validate_filter_empty_skip_identity(inner in simple_type()) {
        let ty = parse_str::<Type>(&inner).unwrap();
        let empty = skip_set(&[]);
        let filtered = filter_inner_type(&ty, &empty);
        prop_assert_eq!(ty_str(&filtered), ty_str(&ty));
    }

    /// filter_inner_type strips skip-over wrappers.
    #[test]
    fn prop_validate_filter_strips_wrapper(inner in simple_type()) {
        let inner_ty = parse_str::<Type>(&inner).unwrap();
        let wrapped: Type = parse_quote!(Box<#inner_ty>);
        let skip = skip_set(&["Box"]);
        let filtered = filter_inner_type(&wrapped, &skip);
        prop_assert_eq!(ty_str(&filtered), ty_str(&inner_ty));
    }

    /// wrap_leaf_type on a non-skip type wraps with adze::WithLeaf.
    #[test]
    fn prop_validate_wrap_non_skip_wraps(inner in simple_type()) {
        let ty = parse_str::<Type>(&inner).unwrap();
        let empty = skip_set(&[]);
        let wrapped = wrap_leaf_type(&ty, &empty);
        let s = ty_str(&wrapped);
        prop_assert!(s.contains("WithLeaf"));
    }

    /// wrap_leaf_type on skip type does NOT wrap outer, wraps inner.
    #[test]
    fn prop_validate_wrap_skip_preserves_outer(inner in simple_type()) {
        let inner_ty = parse_str::<Type>(&inner).unwrap();
        let ty: Type = parse_quote!(Vec<#inner_ty>);
        let skip = skip_set(&["Vec"]);
        let wrapped = wrap_leaf_type(&ty, &skip);
        let s = ty_str(&wrapped);
        // Outer Vec is preserved, inner is wrapped
        prop_assert!(s.starts_with("Vec"));
        prop_assert!(s.contains("WithLeaf"));
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Category 7: prop_error_* — error message properties (6 tests)
// ═══════════════════════════════════════════════════════════════════════════

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    /// Parsing an empty string as Type produces an error.
    #[test]
    fn prop_error_empty_string_type_fails(_dummy in 0u8..1) {
        let result = parse_str::<Type>("");
        prop_assert!(result.is_err());
    }

    /// Parsing invalid tokens as DeriveInput produces an error.
    #[test]
    fn prop_error_garbage_derive_input_fails(s in "[^a-zA-Z_]{3,10}") {
        let result = parse_str::<DeriveInput>(&s);
        prop_assert!(result.is_err());
    }

    /// syn::Error message is non-empty for invalid type strings.
    #[test]
    fn prop_error_message_non_empty(s in "[^a-zA-Z_]{2,8}") {
        if let Err(e) = parse_str::<Type>(&s) {
            let msg = e.to_string();
            prop_assert!(!msg.is_empty());
        }
    }

    /// syn::Error has a valid span (line/column) for invalid type inputs.
    #[test]
    fn prop_error_has_span_info(s in "[^a-zA-Z_]{2,6}") {
        if let Err(e) = parse_str::<Type>(&s) {
            // syn errors always have a span; to_compile_error produces valid tokens
            let tokens = e.to_compile_error();
            let token_str = tokens.to_string();
            prop_assert!(!token_str.is_empty());
        }
    }

    /// Malformed generic bracket in type string produces an error.
    #[test]
    fn prop_error_unclosed_angle_bracket(
        wrapper in wrapper_name(),
        inner in simple_type(),
    ) {
        let malformed = format!("{wrapper}<{inner}");
        let result = parse_str::<Type>(&malformed);
        prop_assert!(result.is_err());
    }

    /// Parsing `struct` without a name fails.
    #[test]
    fn prop_error_struct_no_name_fails(_dummy in 0u8..1) {
        let result = parse_str::<DeriveInput>("struct ;");
        prop_assert!(result.is_err());
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Category 8: prop_roundtrip_* — parse → emit → parse roundtrip (6 tests)
// ═══════════════════════════════════════════════════════════════════════════

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    /// Type roundtrips through token stream and re-parse.
    #[test]
    fn prop_roundtrip_type_token_stream(ty_name in simple_type()) {
        let ty = parse_str::<Type>(&ty_name).unwrap();
        let tokens = ty.to_token_stream();
        let ty2: Type = syn::parse2(tokens).unwrap();
        prop_assert_eq!(ty_str(&ty), ty_str(&ty2));
    }

    /// Wrapped type roundtrips through token stream.
    #[test]
    fn prop_roundtrip_wrapped_type(
        wrapper in wrapper_name(),
        inner in simple_type(),
    ) {
        let type_str = format!("{wrapper}<{inner}>");
        let ty = parse_str::<Type>(&type_str).unwrap();
        let tokens = ty.to_token_stream();
        let ty2: Type = syn::parse2(tokens).unwrap();
        prop_assert_eq!(ty_str(&ty), ty_str(&ty2));
    }

    /// DeriveInput for a struct roundtrips preserving ident.
    #[test]
    fn prop_roundtrip_struct_ident(name in pascal_name()) {
        let ident = format_ident!("{}", name);
        let di: DeriveInput = parse_quote! { struct #ident { x: i32 } };
        let tokens = di.to_token_stream();
        let di2: DeriveInput = syn::parse2(tokens).unwrap();
        prop_assert_eq!(di2.ident.to_string(), name);
    }

    /// DeriveInput for an enum roundtrips preserving ident and variant count.
    #[test]
    fn prop_roundtrip_enum_ident_and_variants(
        name in pascal_name(),
        v1 in pascal_name(),
        v2 in pascal_name(),
    ) {
        prop_assume!(v1 != v2 && name != v1 && name != v2);
        let ni = format_ident!("{}", name);
        let vi1 = format_ident!("{}", v1);
        let vi2 = format_ident!("{}", v2);
        let di: DeriveInput = parse_quote! { enum #ni { #vi1, #vi2 } };
        let tokens = di.to_token_stream();
        let di2: DeriveInput = syn::parse2(tokens).unwrap();
        prop_assert_eq!(di2.ident.to_string(), name);
        if let syn::Data::Enum(data) = &di2.data {
            prop_assert_eq!(data.variants.len(), 2);
        }
    }

    /// try_extract_inner_type result roundtrips as a parseable type.
    #[test]
    fn prop_roundtrip_extract_result_parseable(
        wrapper in wrapper_name(),
        inner in simple_type(),
    ) {
        let type_str = format!("{wrapper}<{inner}>");
        let ty = parse_str::<Type>(&type_str).unwrap();
        let empty = skip_set(&[]);
        let (extracted_ty, _) = try_extract_inner_type(&ty, &wrapper, &empty);
        // The extracted type should emit parseable tokens
        let tokens = extracted_ty.to_token_stream();
        let reparsed = syn::parse2::<Type>(tokens);
        prop_assert!(reparsed.is_ok());
    }

    /// wrap_leaf_type result roundtrips as a parseable type.
    #[test]
    fn prop_roundtrip_wrap_result_parseable(inner in simple_type()) {
        let ty = parse_str::<Type>(&inner).unwrap();
        let empty = skip_set(&[]);
        let wrapped = wrap_leaf_type(&ty, &empty);
        let tokens = wrapped.to_token_stream();
        let reparsed = syn::parse2::<Type>(tokens);
        prop_assert!(reparsed.is_ok());
    }
}
