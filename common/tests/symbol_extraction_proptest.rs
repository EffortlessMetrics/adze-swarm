#![allow(
    clippy::needless_range_loop,
    reason = "property and comprehensive tests use index-based loops to exercise table positions and boundary cases"
)]

//! Property-based tests for symbol extraction from Rust types in adze-common.
//!
//! Verifies that `try_extract_inner_type`, `filter_inner_type`, and
//! `wrap_leaf_type` correctly extract, unwrap, and transform type symbols
//! across struct fields, enum variants, Option/Vec/Box wrappers, nested
//! generics, and naming conventions.

use adze_common::{filter_inner_type, try_extract_inner_type, wrap_leaf_type};
use proptest::prelude::*;
mod support;
use quote::ToTokens;
use std::collections::HashSet;
use support::{container_name_with_rc, distinct_lower_idents, symbol_leaf_type_name};
use syn::{Type, parse_str};

// ---------------------------------------------------------------------------
// Strategies
// ---------------------------------------------------------------------------

fn leaf_type_name() -> impl Strategy<Value = &'static str> {
    symbol_leaf_type_name()
}

fn distinct_idents(max: usize) -> impl Strategy<Value = Vec<String>> {
    distinct_lower_idents(max, 7)
}

fn container_name() -> impl Strategy<Value = &'static str> {
    container_name_with_rc()
}

fn skip_set_strategy() -> impl Strategy<Value = HashSet<&'static str>> {
    prop::collection::hash_set(container_name(), 0..=5)
}

/// Type strings of varying nesting depth.
fn type_string_strategy() -> impl Strategy<Value = String> {
    prop_oneof![
        leaf_type_name().prop_map(|s| s.to_string()),
        (container_name(), leaf_type_name()).prop_map(|(c, l)| format!("{c}<{l}>")),
        (container_name(), container_name(), leaf_type_name())
            .prop_map(|(c1, c2, l)| format!("{c1}<{c2}<{l}>>")),
    ]
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn ty_str(ty: &Type) -> String {
    ty.to_token_stream().to_string()
}

fn parse_ty(s: &str) -> Type {
    parse_str::<Type>(s).unwrap()
}

/// Build a struct source string with the given named fields and types.
fn struct_source(name: &str, fields: &[(&str, &str)]) -> String {
    let body: Vec<String> = fields
        .iter()
        .map(|(n, t)| format!("    {n}: {t},"))
        .collect();
    format!("struct {name} {{\n{}\n}}", body.join("\n"))
}

/// Build an enum source string with tuple variants.
fn enum_source(name: &str, variants: &[(&str, &str)]) -> String {
    let body: Vec<String> = variants
        .iter()
        .map(|(n, t)| format!("    {n}({t}),"))
        .collect();
    format!("enum {name} {{\n{}\n}}", body.join("\n"))
}

/// Extract field types from a parsed struct item.
fn struct_field_types(src: &str) -> Vec<Type> {
    let item: syn::Item = parse_str(src).unwrap();
    if let syn::Item::Struct(s) = item {
        s.fields.iter().map(|f| f.ty.clone()).collect()
    } else {
        panic!("expected struct");
    }
}

/// Extract variant inner types from a parsed enum item.
fn enum_variant_types(src: &str) -> Vec<Type> {
    let item: syn::Item = parse_str(src).unwrap();
    if let syn::Item::Enum(e) = item {
        e.variants
            .iter()
            .filter_map(|v| v.fields.iter().next().map(|f| f.ty.clone()))
            .collect()
    } else {
        panic!("expected enum");
    }
}

// ---------------------------------------------------------------------------
// Property tests
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(80))]

    // ===== Extract symbols from struct fields =====

    /// 1. Extracting Option from struct field types recovers the inner leaf.
    #[test]
    fn struct_field_option_extraction(leaf in leaf_type_name()) {
        let src = struct_source("Foo", &[("value", &format!("Option<{leaf}>"))]);
        let types = struct_field_types(&src);
        let skip: HashSet<&str> = HashSet::new();
        let (inner, ok) = try_extract_inner_type(&types[0], "Option", &skip);
        prop_assert!(ok);
        prop_assert_eq!(ty_str(&inner), leaf);
    }

    /// 2. Extracting Vec from struct field types recovers the inner leaf.
    #[test]
    fn struct_field_vec_extraction(leaf in leaf_type_name()) {
        let src = struct_source("Bar", &[("items", &format!("Vec<{leaf}>"))]);
        let types = struct_field_types(&src);
        let skip: HashSet<&str> = HashSet::new();
        let (inner, ok) = try_extract_inner_type(&types[0], "Vec", &skip);
        prop_assert!(ok);
        prop_assert_eq!(ty_str(&inner), leaf);
    }

    /// 3. Non-container struct field types are not extracted.
    #[test]
    fn struct_field_plain_not_extracted(
        leaf in leaf_type_name(),
        target in container_name(),
    ) {
        let src = struct_source("Plain", &[("x", leaf)]);
        let types = struct_field_types(&src);
        let skip: HashSet<&str> = HashSet::new();
        let (_, ok) = try_extract_inner_type(&types[0], target, &skip);
        prop_assert!(!ok);
    }

    /// 4. Struct with multiple fields — each field extracts independently.
    #[test]
    fn struct_multi_field_independent_extraction(
        a in leaf_type_name(),
        b in leaf_type_name(),
    ) {
        let src = struct_source("Multi", &[
            ("first", &format!("Option<{a}>")),
            ("second", &format!("Vec<{b}>")),
        ]);
        let types = struct_field_types(&src);
        let skip: HashSet<&str> = HashSet::new();
        let (inner_a, ok_a) = try_extract_inner_type(&types[0], "Option", &skip);
        let (inner_b, ok_b) = try_extract_inner_type(&types[1], "Vec", &skip);
        prop_assert!(ok_a);
        prop_assert!(ok_b);
        prop_assert_eq!(ty_str(&inner_a), a);
        prop_assert_eq!(ty_str(&inner_b), b);
    }

    /// 5. Field ordering is preserved after extraction.
    #[test]
    fn struct_field_order_preserved(names in distinct_idents(4)) {
        if names.len() < 2 { return Ok(()); }
        let fields: Vec<(&str, &str)> = names.iter().map(|n| (n.as_str(), "i32")).collect();
        let src = struct_source("Ordered", &fields);
        let item: syn::Item = parse_str(&src).unwrap();
        if let syn::Item::Struct(s) = item {
            let extracted_names: Vec<String> = s.fields.iter()
                .map(|f| f.ident.as_ref().unwrap().to_string())
                .collect();
            for i in 0..names.len().min(extracted_names.len()) {
                prop_assert_eq!(&extracted_names[i], &names[i]);
            }
        }
    }

    // ===== Extract symbols from enum variants =====

    /// 6. Extracting Option from enum variant types recovers the inner leaf.
    #[test]
    fn enum_variant_option_extraction(leaf in leaf_type_name()) {
        let src = enum_source("MyEnum", &[("Var", &format!("Option<{leaf}>"))]);
        let types = enum_variant_types(&src);
        let skip: HashSet<&str> = HashSet::new();
        let (inner, ok) = try_extract_inner_type(&types[0], "Option", &skip);
        prop_assert!(ok);
        prop_assert_eq!(ty_str(&inner), leaf);
    }

    /// 7. Extracting Vec from enum variant types recovers the inner leaf.
    #[test]
    fn enum_variant_vec_extraction(leaf in leaf_type_name()) {
        let src = enum_source("MyEnum", &[("Items", &format!("Vec<{leaf}>"))]);
        let types = enum_variant_types(&src);
        let skip: HashSet<&str> = HashSet::new();
        let (inner, ok) = try_extract_inner_type(&types[0], "Vec", &skip);
        prop_assert!(ok);
        prop_assert_eq!(ty_str(&inner), leaf);
    }

    /// 8. Plain type in enum variant is not extracted.
    #[test]
    fn enum_variant_plain_not_extracted(
        leaf in leaf_type_name(),
        target in container_name(),
    ) {
        let src = enum_source("E", &[("V", leaf)]);
        let types = enum_variant_types(&src);
        let skip: HashSet<&str> = HashSet::new();
        let (_, ok) = try_extract_inner_type(&types[0], target, &skip);
        prop_assert!(!ok);
    }

    /// 9. Multiple enum variants — each extracts independently.
    #[test]
    fn enum_multi_variant_extraction(
        a in leaf_type_name(),
        b in leaf_type_name(),
    ) {
        let src = enum_source("E", &[
            ("A", &format!("Option<{a}>")),
            ("B", &format!("Vec<{b}>")),
        ]);
        let types = enum_variant_types(&src);
        let skip: HashSet<&str> = HashSet::new();
        let (inner_a, ok_a) = try_extract_inner_type(&types[0], "Option", &skip);
        let (inner_b, ok_b) = try_extract_inner_type(&types[1], "Vec", &skip);
        prop_assert!(ok_a);
        prop_assert!(ok_b);
        prop_assert_eq!(ty_str(&inner_a), a);
        prop_assert_eq!(ty_str(&inner_b), b);
    }

    // ===== Symbol naming conventions =====

    /// 10. Extracted symbol name matches the original leaf type identifier.
    #[test]
    fn symbol_name_matches_leaf(leaf in leaf_type_name()) {
        let ty = parse_ty(&format!("Option<{leaf}>"));
        let skip: HashSet<&str> = HashSet::new();
        let (inner, ok) = try_extract_inner_type(&ty, "Option", &skip);
        prop_assert!(ok);
        let name = ty_str(&inner);
        prop_assert_eq!(name, leaf);
    }

    /// 11. Container type name is never present in extracted output.
    #[test]
    fn container_name_absent_after_extraction(
        container in container_name(),
        leaf in leaf_type_name(),
    ) {
        let ty = parse_ty(&format!("{container}<{leaf}>"));
        let skip: HashSet<&str> = HashSet::new();
        let (inner, ok) = try_extract_inner_type(&ty, container, &skip);
        prop_assert!(ok);
        prop_assert!(!ty_str(&inner).contains(container));
    }

    /// 12. Filtered output never contains the skipped container name.
    #[test]
    fn filter_removes_container_name(
        container in container_name(),
        leaf in leaf_type_name(),
    ) {
        let skip: HashSet<&str> = [container].into_iter().collect();
        let ty = parse_ty(&format!("{container}<{leaf}>"));
        let filtered = filter_inner_type(&ty, &skip);
        prop_assert!(!ty_str(&filtered).contains(container));
    }

    /// 13. Wrapped symbol always contains "WithLeaf" in its name.
    #[test]
    fn wrap_always_introduces_with_leaf(leaf in leaf_type_name()) {
        let ty = parse_ty(leaf);
        let wrapped = wrap_leaf_type(&ty, &HashSet::new());
        prop_assert!(ty_str(&wrapped).contains("WithLeaf"));
    }

    // ===== Symbol from Option type =====

    /// 14. Option<T> extraction yields exactly T.
    #[test]
    fn option_extracts_exact_inner(leaf in leaf_type_name()) {
        let ty = parse_ty(&format!("Option<{leaf}>"));
        let skip: HashSet<&str> = HashSet::new();
        let (inner, ok) = try_extract_inner_type(&ty, "Option", &skip);
        prop_assert!(ok);
        prop_assert_eq!(ty_str(&inner), leaf);
    }

    /// 15. Option<T> filtered with Option in skip set yields T.
    #[test]
    fn option_filter_yields_inner(leaf in leaf_type_name()) {
        let skip: HashSet<&str> = ["Option"].into_iter().collect();
        let ty = parse_ty(&format!("Option<{leaf}>"));
        let filtered = filter_inner_type(&ty, &skip);
        prop_assert_eq!(ty_str(&filtered), leaf);
    }

    /// 16. Option<T> wrapped with Option in skip wraps only the inner T.
    #[test]
    fn option_wrap_wraps_inner_only(leaf in leaf_type_name()) {
        let skip: HashSet<&str> = ["Option"].into_iter().collect();
        let ty = parse_ty(&format!("Option<{leaf}>"));
        let wrapped = wrap_leaf_type(&ty, &skip);
        let s = ty_str(&wrapped);
        prop_assert!(s.starts_with("Option <"), "starts with Option: {s}");
        prop_assert!(s.contains("WithLeaf"), "inner wrapped: {s}");
    }

    // ===== Symbol from Vec type =====

    /// 17. Vec<T> extraction yields exactly T.
    #[test]
    fn vec_extracts_exact_inner(leaf in leaf_type_name()) {
        let ty = parse_ty(&format!("Vec<{leaf}>"));
        let skip: HashSet<&str> = HashSet::new();
        let (inner, ok) = try_extract_inner_type(&ty, "Vec", &skip);
        prop_assert!(ok);
        prop_assert_eq!(ty_str(&inner), leaf);
    }

    /// 18. Vec<T> filtered with Vec in skip set yields T.
    #[test]
    fn vec_filter_yields_inner(leaf in leaf_type_name()) {
        let skip: HashSet<&str> = ["Vec"].into_iter().collect();
        let ty = parse_ty(&format!("Vec<{leaf}>"));
        let filtered = filter_inner_type(&ty, &skip);
        prop_assert_eq!(ty_str(&filtered), leaf);
    }

    /// 19. Vec<T> wrapped with Vec in skip wraps only the inner T.
    #[test]
    fn vec_wrap_wraps_inner_only(leaf in leaf_type_name()) {
        let skip: HashSet<&str> = ["Vec"].into_iter().collect();
        let ty = parse_ty(&format!("Vec<{leaf}>"));
        let wrapped = wrap_leaf_type(&ty, &skip);
        let s = ty_str(&wrapped);
        prop_assert!(s.starts_with("Vec <"), "starts with Vec: {s}");
        prop_assert!(s.contains("WithLeaf"), "inner wrapped: {s}");
    }

    // ===== Symbol from Box type =====

    /// 20. Box<T> extraction through skip set yields T when target is inside.
    #[test]
    fn box_skip_extraction(leaf in leaf_type_name()) {
        let skip: HashSet<&str> = ["Box"].into_iter().collect();
        let ty = parse_ty(&format!("Box<Vec<{leaf}>>"));
        let (inner, ok) = try_extract_inner_type(&ty, "Vec", &skip);
        prop_assert!(ok);
        prop_assert_eq!(ty_str(&inner), leaf);
    }

    /// 21. Box<T> filtered with Box in skip set yields T directly.
    #[test]
    fn box_filter_yields_inner(leaf in leaf_type_name()) {
        let skip: HashSet<&str> = ["Box"].into_iter().collect();
        let ty = parse_ty(&format!("Box<{leaf}>"));
        let filtered = filter_inner_type(&ty, &skip);
        prop_assert_eq!(ty_str(&filtered), leaf);
    }

    /// 22. Box<T> wrapped without skip wraps the entire Box<T>.
    #[test]
    fn box_wrap_without_skip_wraps_entirely(leaf in leaf_type_name()) {
        let ty = parse_ty(&format!("Box<{leaf}>"));
        let wrapped = wrap_leaf_type(&ty, &HashSet::new());
        let s = ty_str(&wrapped);
        prop_assert!(s.starts_with("adze :: WithLeaf"), "entire type wrapped: {s}");
    }

    // ===== Symbol from nested generics =====

    /// 23. Option<Vec<T>> — extract Option yields Vec<T>.
    #[test]
    fn nested_option_vec_extract_outer(leaf in leaf_type_name()) {
        let ty = parse_ty(&format!("Option<Vec<{leaf}>>"));
        let skip: HashSet<&str> = HashSet::new();
        let (inner, ok) = try_extract_inner_type(&ty, "Option", &skip);
        prop_assert!(ok);
        prop_assert_eq!(ty_str(&inner), format!("Vec < {leaf} >"));
    }

    /// 24. Box<Option<T>> — extract through Box skip yields T.
    #[test]
    fn nested_box_option_skip_extraction(leaf in leaf_type_name()) {
        let skip: HashSet<&str> = ["Box"].into_iter().collect();
        let ty = parse_ty(&format!("Box<Option<{leaf}>>"));
        let (inner, ok) = try_extract_inner_type(&ty, "Option", &skip);
        prop_assert!(ok);
        prop_assert_eq!(ty_str(&inner), leaf);
    }

    /// 25. Nested filter: Box<Arc<T>> with both in skip yields T.
    #[test]
    fn nested_filter_double_skip(leaf in leaf_type_name()) {
        let skip: HashSet<&str> = ["Box", "Arc"].into_iter().collect();
        let ty = parse_ty(&format!("Box<Arc<{leaf}>>"));
        let filtered = filter_inner_type(&ty, &skip);
        prop_assert_eq!(ty_str(&filtered), leaf);
    }

    /// 26. Triple nesting: Box<Arc<Vec<T>>> — extract Vec through skip yields T.
    #[test]
    fn triple_nested_skip_extraction(leaf in leaf_type_name()) {
        let skip: HashSet<&str> = ["Box", "Arc"].into_iter().collect();
        let ty = parse_ty(&format!("Box<Arc<Vec<{leaf}>>>"));
        let (inner, ok) = try_extract_inner_type(&ty, "Vec", &skip);
        prop_assert!(ok);
        prop_assert_eq!(ty_str(&inner), leaf);
    }

    /// 27. Wrap nested Option<Vec<T>> with both in skip wraps only the leaf.
    #[test]
    fn nested_wrap_option_vec_wraps_leaf(leaf in leaf_type_name()) {
        let skip: HashSet<&str> = ["Option", "Vec"].into_iter().collect();
        let ty = parse_ty(&format!("Option<Vec<{leaf}>>"));
        let wrapped = wrap_leaf_type(&ty, &skip);
        let s = ty_str(&wrapped);
        let count = s.matches("WithLeaf").count();
        prop_assert!(count == 1, "exactly one WithLeaf in: {}", s);
        prop_assert!(s.starts_with("Option <"), "outer preserved: {s}");
    }

    // ===== Symbol extraction determinism =====

    /// 28. try_extract_inner_type is deterministic across repeated calls.
    #[test]
    fn extraction_deterministic(
        ty_s in type_string_strategy(),
        target in container_name(),
        skip in skip_set_strategy(),
    ) {
        let ty = parse_ty(&ty_s);
        let (r1, e1) = try_extract_inner_type(&ty, target, &skip);
        let (r2, e2) = try_extract_inner_type(&ty, target, &skip);
        prop_assert_eq!(e1, e2);
        prop_assert_eq!(ty_str(&r1), ty_str(&r2));
    }

    /// 29. filter_inner_type is deterministic across repeated calls.
    #[test]
    fn filter_deterministic(
        ty_s in type_string_strategy(),
        skip in skip_set_strategy(),
    ) {
        let ty = parse_ty(&ty_s);
        let f1 = ty_str(&filter_inner_type(&ty, &skip));
        let f2 = ty_str(&filter_inner_type(&ty, &skip));
        prop_assert_eq!(f1, f2);
    }

    /// 30. wrap_leaf_type is deterministic across repeated calls.
    #[test]
    fn wrap_deterministic(
        ty_s in type_string_strategy(),
        skip in skip_set_strategy(),
    ) {
        let ty = parse_ty(&ty_s);
        let w1 = ty_str(&wrap_leaf_type(&ty, &skip));
        let w2 = ty_str(&wrap_leaf_type(&ty, &skip));
        prop_assert_eq!(w1, w2);
    }

    /// 31. Struct field extraction is deterministic.
    #[test]
    fn struct_field_extraction_deterministic(leaf in leaf_type_name()) {
        let src = struct_source("Det", &[("a", &format!("Option<{leaf}>"))]);
        let types1 = struct_field_types(&src);
        let types2 = struct_field_types(&src);
        let skip: HashSet<&str> = HashSet::new();
        let (r1, _) = try_extract_inner_type(&types1[0], "Option", &skip);
        let (r2, _) = try_extract_inner_type(&types2[0], "Option", &skip);
        prop_assert_eq!(ty_str(&r1), ty_str(&r2));
    }

    /// 32. Enum variant extraction is deterministic.
    #[test]
    fn enum_variant_extraction_deterministic(leaf in leaf_type_name()) {
        let src = enum_source("Det", &[("V", &format!("Vec<{leaf}>"))]);
        let types1 = enum_variant_types(&src);
        let types2 = enum_variant_types(&src);
        let skip: HashSet<&str> = HashSet::new();
        let (r1, _) = try_extract_inner_type(&types1[0], "Vec", &skip);
        let (r2, _) = try_extract_inner_type(&types2[0], "Vec", &skip);
        prop_assert_eq!(ty_str(&r1), ty_str(&r2));
    }

    /// 33. Extraction output is always parseable as a valid Type.
    #[test]
    fn extraction_output_always_parseable(
        ty_s in type_string_strategy(),
        target in container_name(),
        skip in skip_set_strategy(),
    ) {
        let ty = parse_ty(&ty_s);
        let (result, _) = try_extract_inner_type(&ty, target, &skip);
        let s = ty_str(&result);
        prop_assert!(parse_str::<Type>(&s).is_ok(), "unparseable: {s}");
    }

    /// 34. Filter output is always parseable as a valid Type.
    #[test]
    fn filter_output_always_parseable(
        ty_s in type_string_strategy(),
        skip in skip_set_strategy(),
    ) {
        let ty = parse_ty(&ty_s);
        let s = ty_str(&filter_inner_type(&ty, &skip));
        prop_assert!(parse_str::<Type>(&s).is_ok(), "unparseable: {s}");
    }

    /// 35. Wrap output is always parseable as a valid Type.
    #[test]
    fn wrap_output_always_parseable(
        ty_s in type_string_strategy(),
        skip in skip_set_strategy(),
    ) {
        let ty = parse_ty(&ty_s);
        let s = ty_str(&wrap_leaf_type(&ty, &skip));
        prop_assert!(parse_str::<Type>(&s).is_ok(), "unparseable: {s}");
    }
}
