#![allow(
    clippy::needless_range_loop,
    reason = "property and comprehensive tests use index-based loops to exercise table positions and boundary cases"
)]

//! Property-based tests for field processing in adze-common.
//!
//! Exercises field type extraction from structs, field attribute parsing,
//! Option/Vec/Box wrapper handling, field ordering preservation,
//! field name to rule name mapping, and processing determinism.

use adze_common::{
    FieldThenParams, NameValueExpr, filter_inner_type, try_extract_inner_type, wrap_leaf_type,
};
use proptest::prelude::*;
mod support;
use quote::ToTokens;
use std::collections::HashSet;
use support::{container_name_with_rc, distinct_lower_idents, field_leaf_type_name, lower_ident};
use syn::{Fields, Item, Type, parse_str};

// ---------------------------------------------------------------------------
// Strategies
// ---------------------------------------------------------------------------

fn ident_strategy() -> impl Strategy<Value = String> {
    lower_ident(8)
}

fn distinct_idents(max: usize) -> impl Strategy<Value = Vec<String>> {
    distinct_lower_idents(max, 8)
}

fn leaf_type_name() -> impl Strategy<Value = &'static str> {
    field_leaf_type_name()
}

fn container_name() -> impl Strategy<Value = &'static str> {
    container_name_with_rc()
}

fn pascal_case_strategy() -> impl Strategy<Value = String> {
    prop::string::string_regex("[A-Z][a-z]{1,6}([A-Z][a-z]{1,6}){0,2}")
        .unwrap()
        .prop_filter("must be valid ident", |s| {
            !s.is_empty() && syn::parse_str::<syn::Ident>(s).is_ok()
        })
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn ty_str(ty: &Type) -> String {
    ty.to_token_stream().to_string()
}

fn skip<'a>(names: &[&'a str]) -> HashSet<&'a str> {
    names.iter().copied().collect()
}

fn _capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => {
            let upper: String = c.to_uppercase().collect();
            upper + chars.as_str()
        }
    }
}

fn build_struct(name: &str, fields: &[(&str, &str)]) -> String {
    if fields.is_empty() {
        return format!("pub struct {name} {{}}");
    }
    let body: String = fields
        .iter()
        .map(|(fname, ftype)| format!("    pub {fname}: {ftype},\n"))
        .collect();
    format!("pub struct {name} {{\n{body}}}")
}

fn extract_struct_fields(item: &Item) -> Vec<(String, String)> {
    if let Item::Struct(s) = item
        && let Fields::Named(ref named) = s.fields
    {
        return named
            .named
            .iter()
            .map(|f| {
                let name = f.ident.as_ref().unwrap().to_string();
                let ty = ty_str(&f.ty);
                (name, ty)
            })
            .collect();
    }
    vec![]
}

fn to_snake_case(name: &str) -> String {
    let mut result = String::new();
    for (i, ch) in name.chars().enumerate() {
        if ch.is_uppercase() && i > 0 {
            let prev = name.chars().nth(i - 1).unwrap_or('_');
            if prev.is_lowercase() || prev.is_ascii_digit() {
                result.push('_');
            } else if let Some(next) = name.chars().nth(i + 1)
                && next.is_lowercase()
            {
                result.push('_');
            }
        }
        result.push(ch.to_lowercase().next().unwrap());
    }
    result
}

// ===========================================================================
// 1. Field type extraction from struct
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(80))]

    // 1. Extracting a matching container from a struct field yields the inner type.
    #[test]
    fn field_extract_matching_container(
        field_name in ident_strategy(),
        container in container_name(),
        inner in leaf_type_name(),
    ) {
        let ftype = format!("{container}<{inner}>");
        let src = build_struct("S", &[(&field_name, &ftype)]);
        let item: Item = parse_str(&src).unwrap();
        let fields = extract_struct_fields(&item);
        let ty: Type = parse_str(&fields[0].1).unwrap();
        let (extracted, ok) = try_extract_inner_type(&ty, container, &skip(&[]));
        prop_assert!(ok);
        prop_assert_eq!(ty_str(&extracted), inner);
    }

    // 2. Extracting a non-matching container from a struct field returns unchanged.
    #[test]
    fn field_extract_non_matching_returns_original(
        field_name in ident_strategy(),
        inner in leaf_type_name(),
    ) {
        let ftype = format!("Vec<{inner}>");
        let src = build_struct("S", &[(&field_name, &ftype)]);
        let item: Item = parse_str(&src).unwrap();
        let fields = extract_struct_fields(&item);
        let ty: Type = parse_str(&fields[0].1).unwrap();
        let (result, ok) = try_extract_inner_type(&ty, "Option", &skip(&[]));
        prop_assert!(!ok);
        prop_assert_eq!(ty_str(&result), ty_str(&ty));
    }

    // 3. Extracting from a plain (non-generic) field type returns unchanged.
    #[test]
    fn field_extract_plain_type_returns_original(
        field_name in ident_strategy(),
        leaf in leaf_type_name(),
        target in container_name(),
    ) {
        let src = build_struct("S", &[(&field_name, leaf)]);
        let item: Item = parse_str(&src).unwrap();
        let fields = extract_struct_fields(&item);
        let ty: Type = parse_str(&fields[0].1).unwrap();
        let (result, ok) = try_extract_inner_type(&ty, target, &skip(&[]));
        prop_assert!(!ok);
        prop_assert_eq!(ty_str(&result), leaf);
    }

    // 4. Extracting from multi-field struct processes each field independently.
    #[test]
    fn field_extract_multi_field_independent(
        inner1 in leaf_type_name(),
        inner2 in leaf_type_name(),
    ) {
        let src = build_struct("S", &[("a", &format!("Vec<{inner1}>")), ("b", &format!("Option<{inner2}>"))]);
        let item: Item = parse_str(&src).unwrap();
        let fields = extract_struct_fields(&item);
        let ty_a: Type = parse_str(&fields[0].1).unwrap();
        let ty_b: Type = parse_str(&fields[1].1).unwrap();
        let (ext_a, ok_a) = try_extract_inner_type(&ty_a, "Vec", &skip(&[]));
        let (ext_b, ok_b) = try_extract_inner_type(&ty_b, "Option", &skip(&[]));
        prop_assert!(ok_a);
        prop_assert!(ok_b);
        prop_assert_eq!(ty_str(&ext_a), inner1);
        prop_assert_eq!(ty_str(&ext_b), inner2);
    }
}

// ===========================================================================
// 2. Field attribute parsing
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(80))]

    // 5. FieldThenParams with no params has empty params list.
    #[test]
    fn ftp_no_params_empty(ty in leaf_type_name()) {
        let parsed: FieldThenParams = syn::parse_str(ty).unwrap();
        prop_assert!(parsed.comma.is_none());
        prop_assert!(parsed.params.is_empty());
    }

    // 6. FieldThenParams with one named param has exactly one entry.
    #[test]
    fn ftp_single_param_count(ty in leaf_type_name()) {
        let input = format!("{ty}, rename = \"x\"");
        let parsed: FieldThenParams = syn::parse_str(&input).unwrap();
        prop_assert!(parsed.comma.is_some());
        prop_assert_eq!(parsed.params.len(), 1);
        prop_assert_eq!(parsed.params[0].path.to_string(), "rename");
    }

    // 7. FieldThenParams with two params preserves both param names.
    #[test]
    fn ftp_two_params_preserved(ty in leaf_type_name()) {
        let input = format!("{ty}, precedence = 3, assoc = \"left\"");
        let parsed: FieldThenParams = syn::parse_str(&input).unwrap();
        prop_assert_eq!(parsed.params.len(), 2);
        prop_assert_eq!(parsed.params[0].path.to_string(), "precedence");
        prop_assert_eq!(parsed.params[1].path.to_string(), "assoc");
    }

    // 8. FieldThenParams preserves field type regardless of params.
    #[test]
    fn ftp_field_type_preserved_with_params(
        container in container_name(),
        inner in leaf_type_name(),
    ) {
        let input = format!("{container}<{inner}>, key = 42");
        let parsed: FieldThenParams = syn::parse_str(&input).unwrap();
        let s = ty_str(&parsed.field.ty);
        prop_assert!(s.contains(container));
        prop_assert!(s.contains(inner));
    }

    // 9. NameValueExpr preserves the key name for arbitrary valid identifiers.
    #[test]
    fn nve_key_preserved(key in ident_strategy()) {
        let input = format!("{key} = 42");
        let parsed: NameValueExpr = syn::parse_str(&input).unwrap();
        prop_assert_eq!(parsed.path.to_string(), key);
    }
}

// ===========================================================================
// 3. Field with Option wrapper
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(80))]

    // 10. Option<T> extracts T when target is "Option".
    #[test]
    fn option_field_extracts_inner(
        field_name in ident_strategy(),
        inner in leaf_type_name(),
    ) {
        let ftype = format!("Option<{inner}>");
        let src = build_struct("S", &[(&field_name, &ftype)]);
        let item: Item = parse_str(&src).unwrap();
        let fields = extract_struct_fields(&item);
        let ty: Type = parse_str(&fields[0].1).unwrap();
        let (extracted, ok) = try_extract_inner_type(&ty, "Option", &skip(&[]));
        prop_assert!(ok);
        prop_assert_eq!(ty_str(&extracted), inner);
    }

    // 11. Option<T> does not extract when target is "Vec".
    #[test]
    fn option_field_no_extract_for_vec(
        field_name in ident_strategy(),
        inner in leaf_type_name(),
    ) {
        let ftype = format!("Option<{inner}>");
        let src = build_struct("S", &[(&field_name, &ftype)]);
        let item: Item = parse_str(&src).unwrap();
        let fields = extract_struct_fields(&item);
        let ty: Type = parse_str(&fields[0].1).unwrap();
        let (_result, ok) = try_extract_inner_type(&ty, "Vec", &skip(&[]));
        prop_assert!(!ok);
    }

    // 12. filter_inner_type strips Option when in skip set.
    #[test]
    fn option_field_filter_strips(
        field_name in ident_strategy(),
        inner in leaf_type_name(),
    ) {
        let ftype = format!("Option<{inner}>");
        let src = build_struct("S", &[(&field_name, &ftype)]);
        let item: Item = parse_str(&src).unwrap();
        let fields = extract_struct_fields(&item);
        let ty: Type = parse_str(&fields[0].1).unwrap();
        let filtered = filter_inner_type(&ty, &skip(&["Option"]));
        prop_assert_eq!(ty_str(&filtered), inner);
    }
}

// ===========================================================================
// 4. Field with Vec wrapper
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(80))]

    // 13. Vec<T> extracts T when target is "Vec".
    #[test]
    fn vec_field_extracts_inner(
        field_name in ident_strategy(),
        inner in leaf_type_name(),
    ) {
        let ftype = format!("Vec<{inner}>");
        let src = build_struct("S", &[(&field_name, &ftype)]);
        let item: Item = parse_str(&src).unwrap();
        let fields = extract_struct_fields(&item);
        let ty: Type = parse_str(&fields[0].1).unwrap();
        let (extracted, ok) = try_extract_inner_type(&ty, "Vec", &skip(&[]));
        prop_assert!(ok);
        prop_assert_eq!(ty_str(&extracted), inner);
    }

    // 14. Vec<T> is not extracted when target is "Option".
    #[test]
    fn vec_field_no_extract_for_option(
        field_name in ident_strategy(),
        inner in leaf_type_name(),
    ) {
        let ftype = format!("Vec<{inner}>");
        let src = build_struct("S", &[(&field_name, &ftype)]);
        let item: Item = parse_str(&src).unwrap();
        let fields = extract_struct_fields(&item);
        let ty: Type = parse_str(&fields[0].1).unwrap();
        let (_result, ok) = try_extract_inner_type(&ty, "Option", &skip(&[]));
        prop_assert!(!ok);
    }

    // 15. wrap_leaf_type on Vec<T> with Vec in skip set wraps only the inner T.
    #[test]
    fn vec_field_wrap_skips_container(
        field_name in ident_strategy(),
        inner in leaf_type_name(),
    ) {
        let ftype = format!("Vec<{inner}>");
        let src = build_struct("S", &[(&field_name, &ftype)]);
        let item: Item = parse_str(&src).unwrap();
        let fields = extract_struct_fields(&item);
        let ty: Type = parse_str(&fields[0].1).unwrap();
        let wrapped = wrap_leaf_type(&ty, &skip(&["Vec"]));
        let s = ty_str(&wrapped);
        prop_assert!(s.starts_with("Vec <"), "outer Vec preserved: {s}");
        prop_assert!(s.contains("adze :: WithLeaf"), "inner wrapped: {s}");
    }
}

// ===========================================================================
// 5. Field with Box wrapper
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(80))]

    // 16. Box<T> extracts T when target is "Box".
    #[test]
    fn box_field_extracts_inner(
        field_name in ident_strategy(),
        inner in leaf_type_name(),
    ) {
        let ftype = format!("Box<{inner}>");
        let src = build_struct("S", &[(&field_name, &ftype)]);
        let item: Item = parse_str(&src).unwrap();
        let fields = extract_struct_fields(&item);
        let ty: Type = parse_str(&fields[0].1).unwrap();
        let (extracted, ok) = try_extract_inner_type(&ty, "Box", &skip(&[]));
        prop_assert!(ok);
        prop_assert_eq!(ty_str(&extracted), inner);
    }

    // 17. filter_inner_type strips Box when in skip set.
    #[test]
    fn box_field_filter_strips(
        field_name in ident_strategy(),
        inner in leaf_type_name(),
    ) {
        let ftype = format!("Box<{inner}>");
        let src = build_struct("S", &[(&field_name, &ftype)]);
        let item: Item = parse_str(&src).unwrap();
        let fields = extract_struct_fields(&item);
        let ty: Type = parse_str(&fields[0].1).unwrap();
        let filtered = filter_inner_type(&ty, &skip(&["Box"]));
        prop_assert_eq!(ty_str(&filtered), inner);
    }

    // 18. Box<Option<T>> extracts T when Box is in skip set and target is Option.
    #[test]
    fn box_option_field_extracts_through_skip(
        field_name in ident_strategy(),
        inner in leaf_type_name(),
    ) {
        let ftype = format!("Box<Option<{inner}>>");
        let src = build_struct("S", &[(&field_name, &ftype)]);
        let item: Item = parse_str(&src).unwrap();
        let fields = extract_struct_fields(&item);
        let ty: Type = parse_str(&fields[0].1).unwrap();
        let (extracted, ok) = try_extract_inner_type(&ty, "Option", &skip(&["Box"]));
        prop_assert!(ok);
        prop_assert_eq!(ty_str(&extracted), inner);
    }
}

// ===========================================================================
// 6. Field ordering preservation
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(60))]

    // 19. Extracted fields preserve declaration order.
    #[test]
    fn field_order_preserved(
        idents in distinct_idents(6),
        ty in leaf_type_name(),
    ) {
        prop_assume!(idents.len() >= 2);
        let pairs: Vec<(&str, &str)> = idents.iter().map(|id| (id.as_str(), ty)).collect();
        let src = build_struct("S", &pairs);
        let item: Item = parse_str(&src).unwrap();
        let fields = extract_struct_fields(&item);
        prop_assert_eq!(fields.len(), idents.len());
        for i in 0..idents.len() {
            prop_assert_eq!(&fields[i].0, &idents[i]);
        }
    }

    // 20. Roundtripping through token stream preserves field order.
    #[test]
    fn field_order_roundtrip(
        idents in distinct_idents(5),
        ty in leaf_type_name(),
    ) {
        prop_assume!(idents.len() >= 2);
        let pairs: Vec<(&str, &str)> = idents.iter().map(|id| (id.as_str(), ty)).collect();
        let src = build_struct("S", &pairs);
        let item: Item = parse_str(&src).unwrap();
        let tokens = item.to_token_stream().to_string();
        let reparsed: Item = parse_str(&tokens).unwrap();
        let fields = extract_struct_fields(&reparsed);
        for i in 0..idents.len() {
            prop_assert_eq!(&fields[i].0, &idents[i]);
        }
    }

    // 21. Mixed container and plain field types preserve order.
    #[test]
    fn field_order_mixed_types(
        f1 in ident_strategy(),
        f2 in ident_strategy(),
        f3 in ident_strategy(),
        inner in leaf_type_name(),
    ) {
        prop_assume!(f1 != f2 && f2 != f3 && f1 != f3);
        let src = build_struct("S", &[
            (&f1, inner),
            (&f2, &format!("Vec<{inner}>")),
            (&f3, &format!("Option<{inner}>")),
        ]);
        let item: Item = parse_str(&src).unwrap();
        let fields = extract_struct_fields(&item);
        prop_assert_eq!(fields.len(), 3);
        prop_assert_eq!(&fields[0].0, &f1);
        prop_assert_eq!(&fields[1].0, &f2);
        prop_assert_eq!(&fields[2].0, &f3);
    }
}

// ===========================================================================
// 7. Field name to rule name mapping
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(80))]

    // 22. Struct name maps to a snake_case rule name.
    #[test]
    fn struct_name_to_rule_name(
        struct_name in pascal_case_strategy(),
        field_name in ident_strategy(),
        ty in leaf_type_name(),
    ) {
        let src = build_struct(&struct_name, &[(&field_name, ty)]);
        let item: Item = parse_str(&src).unwrap();
        if let Item::Struct(s) = &item {
            let rule = to_snake_case(&s.ident.to_string());
            prop_assert!(
                rule.chars().all(|c| c.is_lowercase() || c == '_' || c.is_ascii_digit()),
                "rule name should be snake_case: {rule}"
            );
        }
    }

    // 23. Field type extraction does not alter the struct's rule name.
    #[test]
    fn extraction_does_not_alter_rule_name(
        struct_name in pascal_case_strategy(),
        container in container_name(),
        inner in leaf_type_name(),
    ) {
        let ftype = format!("{container}<{inner}>");
        let src = build_struct(&struct_name, &[("field", &ftype)]);
        let item: Item = parse_str(&src).unwrap();
        if let Item::Struct(s) = &item {
            let rule_before = to_snake_case(&s.ident.to_string());
            let field = s.fields.iter().next().unwrap();
            let _ = try_extract_inner_type(&field.ty, container, &skip(&[]));
            let rule_after = to_snake_case(&s.ident.to_string());
            prop_assert_eq!(rule_before, rule_after);
        }
    }

    // 24. Distinct struct names produce distinct rule names.
    #[test]
    fn distinct_structs_distinct_rules(
        name1 in pascal_case_strategy(),
        name2 in pascal_case_strategy(),
        ty in leaf_type_name(),
    ) {
        prop_assume!(name1 != name2);
        let src1 = build_struct(&name1, &[("v", ty)]);
        let src2 = build_struct(&name2, &[("v", ty)]);
        let item1: Item = parse_str(&src1).unwrap();
        let item2: Item = parse_str(&src2).unwrap();
        if let (Item::Struct(s1), Item::Struct(s2)) = (&item1, &item2) {
            let r1 = to_snake_case(&s1.ident.to_string());
            let r2 = to_snake_case(&s2.ident.to_string());
            prop_assert_ne!(r1, r2);
        }
    }

    // 25. Rule name from a field-bearing struct is non-empty.
    #[test]
    fn rule_name_non_empty(
        struct_name in pascal_case_strategy(),
        field_name in ident_strategy(),
        ty in leaf_type_name(),
    ) {
        let src = build_struct(&struct_name, &[(&field_name, ty)]);
        let item: Item = parse_str(&src).unwrap();
        if let Item::Struct(s) = &item {
            let rule = to_snake_case(&s.ident.to_string());
            prop_assert!(!rule.is_empty());
        }
    }
}

// ===========================================================================
// 8. Field processing determinism
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(80))]

    // 26. Extracting the same field type twice yields identical results.
    #[test]
    fn extract_deterministic(
        container in container_name(),
        inner in leaf_type_name(),
    ) {
        let ftype = format!("{container}<{inner}>");
        let src = build_struct("S", &[("f", &ftype)]);
        let item: Item = parse_str(&src).unwrap();
        let fields = extract_struct_fields(&item);
        let ty: Type = parse_str(&fields[0].1).unwrap();
        let (r1, e1) = try_extract_inner_type(&ty, container, &skip(&[]));
        let (r2, e2) = try_extract_inner_type(&ty, container, &skip(&[]));
        prop_assert_eq!(e1, e2);
        prop_assert_eq!(ty_str(&r1), ty_str(&r2));
    }

    // 27. Filtering the same field type twice yields identical results.
    #[test]
    fn filter_deterministic(
        container in container_name(),
        inner in leaf_type_name(),
    ) {
        let ftype = format!("{container}<{inner}>");
        let src = build_struct("S", &[("f", &ftype)]);
        let item: Item = parse_str(&src).unwrap();
        let fields = extract_struct_fields(&item);
        let ty: Type = parse_str(&fields[0].1).unwrap();
        let f1 = filter_inner_type(&ty, &skip(&[container]));
        let f2 = filter_inner_type(&ty, &skip(&[container]));
        prop_assert_eq!(ty_str(&f1), ty_str(&f2));
    }

    // 28. Wrapping the same field type twice yields identical results.
    #[test]
    fn wrap_deterministic(
        container in container_name(),
        inner in leaf_type_name(),
    ) {
        let ftype = format!("{container}<{inner}>");
        let src = build_struct("S", &[("f", &ftype)]);
        let item: Item = parse_str(&src).unwrap();
        let fields = extract_struct_fields(&item);
        let ty: Type = parse_str(&fields[0].1).unwrap();
        let w1 = wrap_leaf_type(&ty, &skip(&[container]));
        let w2 = wrap_leaf_type(&ty, &skip(&[container]));
        prop_assert_eq!(ty_str(&w1), ty_str(&w2));
    }

    // 29. Full pipeline (extract + filter + wrap) on a field type is deterministic.
    #[test]
    fn full_pipeline_deterministic(
        container in container_name(),
        inner in leaf_type_name(),
    ) {
        let ftype = format!("{container}<{inner}>");
        let ty: Type = parse_str(&ftype).unwrap();
        let pipeline = || {
            let (ext, ok) = try_extract_inner_type(&ty, container, &skip(&[]));
            let filtered = filter_inner_type(&ext, &skip(&[container]));
            let wrapped = wrap_leaf_type(&filtered, &HashSet::new());
            (ok, ty_str(&wrapped))
        };
        let (ok1, s1) = pipeline();
        let (ok2, s2) = pipeline();
        prop_assert_eq!(ok1, ok2);
        prop_assert_eq!(s1, s2);
    }

    // 30. Struct field extraction count is deterministic.
    #[test]
    fn field_count_deterministic(
        idents in distinct_idents(5),
        ty in leaf_type_name(),
    ) {
        prop_assume!(!idents.is_empty());
        let pairs: Vec<(&str, &str)> = idents.iter().map(|id| (id.as_str(), ty)).collect();
        let src = build_struct("S", &pairs);
        let item1: Item = parse_str(&src).unwrap();
        let item2: Item = parse_str(&src).unwrap();
        let fields1 = extract_struct_fields(&item1);
        let fields2 = extract_struct_fields(&item2);
        prop_assert_eq!(fields1.len(), fields2.len());
        for i in 0..fields1.len() {
            prop_assert_eq!(&fields1[i].0, &fields2[i].0);
            prop_assert_eq!(&fields1[i].1, &fields2[i].1);
        }
    }

    // 31. FieldThenParams parsing is deterministic.
    #[test]
    fn ftp_parsing_deterministic(ty in leaf_type_name()) {
        let input = format!("{ty}, key = 99");
        let p1: FieldThenParams = syn::parse_str(&input).unwrap();
        let p2: FieldThenParams = syn::parse_str(&input).unwrap();
        prop_assert_eq!(ty_str(&p1.field.ty), ty_str(&p2.field.ty));
        prop_assert_eq!(p1.params.len(), p2.params.len());
        for i in 0..p1.params.len() {
            prop_assert_eq!(p1.params[i].path.to_string(), p2.params[i].path.to_string());
        }
    }

    // 32. Filter is idempotent on field types from structs.
    #[test]
    fn filter_idempotent_on_field(
        container in container_name(),
        inner in leaf_type_name(),
    ) {
        let ftype = format!("{container}<{inner}>");
        let src = build_struct("S", &[("f", &ftype)]);
        let item: Item = parse_str(&src).unwrap();
        let fields = extract_struct_fields(&item);
        let ty: Type = parse_str(&fields[0].1).unwrap();
        let once = filter_inner_type(&ty, &skip(&[container]));
        let twice = filter_inner_type(&once, &skip(&[container]));
        prop_assert_eq!(ty_str(&once), ty_str(&twice));
    }

    // 33. Wrapping a plain field type produces parseable output.
    #[test]
    fn wrap_field_produces_parseable(
        field_name in ident_strategy(),
        leaf in leaf_type_name(),
    ) {
        let src = build_struct("S", &[(&field_name, leaf)]);
        let item: Item = parse_str(&src).unwrap();
        let fields = extract_struct_fields(&item);
        let ty: Type = parse_str(&fields[0].1).unwrap();
        let wrapped = wrap_leaf_type(&ty, &HashSet::new());
        let s = ty_str(&wrapped);
        prop_assert!(parse_str::<Type>(&s).is_ok(), "unparseable: {s}");
    }

    // 34. Filtering then wrapping a container field yields adze::WithLeaf<inner>.
    #[test]
    fn filter_then_wrap_yields_with_leaf(
        container in container_name(),
        inner in leaf_type_name(),
    ) {
        let ftype = format!("{container}<{inner}>");
        let ty: Type = parse_str(&ftype).unwrap();
        let filtered = filter_inner_type(&ty, &skip(&[container]));
        let wrapped = wrap_leaf_type(&filtered, &HashSet::new());
        prop_assert_eq!(ty_str(&wrapped), format!("adze :: WithLeaf < {inner} >"));
    }

    // 35. Extracted field types are always parseable as syn::Type.
    #[test]
    fn extracted_field_types_parseable(
        container in container_name(),
        inner in leaf_type_name(),
    ) {
        let ftype = format!("{container}<{inner}>");
        let src = build_struct("S", &[("f", &ftype)]);
        let item: Item = parse_str(&src).unwrap();
        let fields = extract_struct_fields(&item);
        let ty: Type = parse_str(&fields[0].1).unwrap();
        let (extracted, _) = try_extract_inner_type(&ty, container, &skip(&[]));
        let s = ty_str(&extracted);
        prop_assert!(parse_str::<Type>(&s).is_ok(), "unparseable: {s}");
    }
}

// ===========================================================================
// 9. Visibility handling (pub, pub(crate), private)
// ===========================================================================

fn build_struct_vis(name: &str, fields: &[(&str, &str, &str)]) -> String {
    if fields.is_empty() {
        return format!("pub struct {name} {{}}");
    }
    let body: String = fields
        .iter()
        .map(|(vis, fname, ftype)| {
            if vis.is_empty() {
                format!("    {fname}: {ftype},\n")
            } else {
                format!("    {vis} {fname}: {ftype},\n")
            }
        })
        .collect();
    format!("pub struct {name} {{\n{body}}}")
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(60))]

    // 36. Private field types can be extracted from parsed struct.
    #[test]
    fn private_field_type_extractable(
        inner in leaf_type_name(),
        container in container_name(),
    ) {
        let ftype = format!("{container}<{inner}>");
        let src = build_struct_vis("S", &[("", "field", &ftype)]);
        let item: Item = parse_str(&src).unwrap();
        let fields = extract_struct_fields(&item);
        prop_assert_eq!(fields.len(), 1);
        let ty: Type = parse_str(&fields[0].1).unwrap();
        let (extracted, ok) = try_extract_inner_type(&ty, container, &skip(&[]));
        prop_assert!(ok);
        prop_assert_eq!(ty_str(&extracted), inner);
    }

    // 37. pub(crate) field types can be extracted identically to pub fields.
    #[test]
    fn pub_crate_field_type_matches_pub(
        inner in leaf_type_name(),
        container in container_name(),
    ) {
        let ftype = format!("{container}<{inner}>");
        let src_pub = build_struct_vis("S", &[("pub", "f", &ftype)]);
        let src_crate = build_struct_vis("S", &[("pub(crate)", "f", &ftype)]);
        let item_pub: Item = parse_str(&src_pub).unwrap();
        let item_crate: Item = parse_str(&src_crate).unwrap();
        let fields_pub = extract_struct_fields(&item_pub);
        let fields_crate = extract_struct_fields(&item_crate);
        let ty_pub: Type = parse_str(&fields_pub[0].1).unwrap();
        let ty_crate: Type = parse_str(&fields_crate[0].1).unwrap();
        let (ext_pub, _) = try_extract_inner_type(&ty_pub, container, &skip(&[]));
        let (ext_crate, _) = try_extract_inner_type(&ty_crate, container, &skip(&[]));
        prop_assert_eq!(ty_str(&ext_pub), ty_str(&ext_crate));
    }

    // 38. Visibility does not affect filter_inner_type result.
    #[test]
    fn visibility_does_not_affect_filter(
        inner in leaf_type_name(),
        container in container_name(),
    ) {
        let ftype = format!("{container}<{inner}>");
        let src_priv = build_struct_vis("S", &[("", "f", &ftype)]);
        let src_pub = build_struct_vis("S", &[("pub", "f", &ftype)]);
        let item_priv: Item = parse_str(&src_priv).unwrap();
        let item_pub: Item = parse_str(&src_pub).unwrap();
        let ty_priv: Type = parse_str(&extract_struct_fields(&item_priv)[0].1).unwrap();
        let ty_pub: Type = parse_str(&extract_struct_fields(&item_pub)[0].1).unwrap();
        let f_priv = filter_inner_type(&ty_priv, &skip(&[container]));
        let f_pub = filter_inner_type(&ty_pub, &skip(&[container]));
        prop_assert_eq!(ty_str(&f_priv), ty_str(&f_pub));
    }

    // 39. Visibility does not affect wrap_leaf_type result.
    #[test]
    fn visibility_does_not_affect_wrap(
        inner in leaf_type_name(),
        container in container_name(),
    ) {
        let ftype = format!("{container}<{inner}>");
        let src_priv = build_struct_vis("S", &[("", "f", &ftype)]);
        let src_pub = build_struct_vis("S", &[("pub", "f", &ftype)]);
        let item_priv: Item = parse_str(&src_priv).unwrap();
        let item_pub: Item = parse_str(&src_pub).unwrap();
        let ty_priv: Type = parse_str(&extract_struct_fields(&item_priv)[0].1).unwrap();
        let ty_pub: Type = parse_str(&extract_struct_fields(&item_pub)[0].1).unwrap();
        let w_priv = wrap_leaf_type(&ty_priv, &skip(&[container]));
        let w_pub = wrap_leaf_type(&ty_pub, &skip(&[container]));
        prop_assert_eq!(ty_str(&w_priv), ty_str(&w_pub));
    }

    // 40. Mixed-visibility struct preserves field ordering.
    #[test]
    fn mixed_visibility_preserves_order(
        f1 in ident_strategy(),
        f2 in ident_strategy(),
        f3 in ident_strategy(),
        ty in leaf_type_name(),
    ) {
        prop_assume!(f1 != f2 && f2 != f3 && f1 != f3);
        let src = build_struct_vis("S", &[
            ("pub", &f1, ty),
            ("pub(crate)", &f2, ty),
            ("", &f3, ty),
        ]);
        let item: Item = parse_str(&src).unwrap();
        let fields = extract_struct_fields(&item);
        prop_assert_eq!(fields.len(), 3);
        prop_assert_eq!(&fields[0].0, &f1);
        prop_assert_eq!(&fields[1].0, &f2);
        prop_assert_eq!(&fields[2].0, &f3);
    }
}

// ===========================================================================
// 10. FieldThenParams advanced parsing
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(60))]

    // 41. FTP with three params preserves all param names in order.
    #[test]
    fn ftp_three_params_order(ty in leaf_type_name()) {
        let input = format!("{ty}, alpha = 1, beta = 2, gamma = 3");
        let parsed: FieldThenParams = syn::parse_str(&input).unwrap();
        prop_assert_eq!(parsed.params.len(), 3);
        prop_assert_eq!(parsed.params[0].path.to_string(), "alpha");
        prop_assert_eq!(parsed.params[1].path.to_string(), "beta");
        prop_assert_eq!(parsed.params[2].path.to_string(), "gamma");
    }

    // 42. FTP with string literal param value.
    #[test]
    fn ftp_string_literal_value(ty in leaf_type_name()) {
        let input = format!("{ty}, label = \"hello\"");
        let parsed: FieldThenParams = syn::parse_str(&input).unwrap();
        prop_assert_eq!(parsed.params.len(), 1);
        prop_assert_eq!(parsed.params[0].path.to_string(), "label");
    }

    // 43. FTP with nested container type preserves full type.
    #[test]
    fn ftp_nested_container_type(
        outer in container_name(),
        inner_container in container_name(),
        leaf in leaf_type_name(),
    ) {
        let input = format!("{outer}<{inner_container}<{leaf}>>, depth = 2");
        let parsed: FieldThenParams = syn::parse_str(&input).unwrap();
        let s = ty_str(&parsed.field.ty);
        prop_assert!(s.contains(outer), "outer {outer} in {s}");
        prop_assert!(s.contains(inner_container), "inner {inner_container} in {s}");
        prop_assert!(s.contains(leaf), "leaf {leaf} in {s}");
        prop_assert_eq!(parsed.params.len(), 1);
    }

    // 44. FTP field type roundtrips through token stream.
    #[test]
    fn ftp_type_roundtrip_token_stream(ty in leaf_type_name()) {
        let input = format!("{ty}, key = 1");
        let parsed: FieldThenParams = syn::parse_str(&input).unwrap();
        let tokens = parsed.field.ty.to_token_stream().to_string();
        let reparsed: Type = parse_str(&tokens).unwrap();
        prop_assert_eq!(ty_str(&reparsed), ty_str(&parsed.field.ty));
    }

    // 45. FTP with param names matching ident_strategy are preserved.
    #[test]
    fn ftp_arbitrary_param_names(
        ty in leaf_type_name(),
        pname in ident_strategy(),
    ) {
        let input = format!("{ty}, {pname} = 0");
        let parsed: FieldThenParams = syn::parse_str(&input).unwrap();
        prop_assert_eq!(parsed.params[0].path.to_string(), pname);
    }
}

// ===========================================================================
// 11. Multiple fields composition
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(60))]

    // 46. All fields in a struct can be independently filtered.
    #[test]
    fn all_fields_independently_filterable(
        inner1 in leaf_type_name(),
        inner2 in leaf_type_name(),
        inner3 in leaf_type_name(),
    ) {
        let src = build_struct("S", &[
            ("a", &format!("Box<{inner1}>")),
            ("b", &format!("Option<{inner2}>")),
            ("c", &format!("Vec<{inner3}>")),
        ]);
        let item: Item = parse_str(&src).unwrap();
        let fields = extract_struct_fields(&item);
        let ty_a: Type = parse_str(&fields[0].1).unwrap();
        let ty_b: Type = parse_str(&fields[1].1).unwrap();
        let ty_c: Type = parse_str(&fields[2].1).unwrap();
        prop_assert_eq!(ty_str(&filter_inner_type(&ty_a, &skip(&["Box"]))), inner1);
        prop_assert_eq!(ty_str(&filter_inner_type(&ty_b, &skip(&["Option"]))), inner2);
        prop_assert_eq!(ty_str(&filter_inner_type(&ty_c, &skip(&["Vec"]))), inner3);
    }

    // 47. All fields in a struct can be independently wrapped.
    #[test]
    fn all_fields_independently_wrappable(
        inner1 in leaf_type_name(),
        inner2 in leaf_type_name(),
    ) {
        let src = build_struct("S", &[
            ("a", &format!("Vec<{inner1}>")),
            ("b", &format!("Option<{inner2}>")),
        ]);
        let item: Item = parse_str(&src).unwrap();
        let fields = extract_struct_fields(&item);
        let ty_a: Type = parse_str(&fields[0].1).unwrap();
        let ty_b: Type = parse_str(&fields[1].1).unwrap();
        let w_a = wrap_leaf_type(&ty_a, &skip(&["Vec"]));
        let w_b = wrap_leaf_type(&ty_b, &skip(&["Option"]));
        prop_assert!(ty_str(&w_a).contains("adze :: WithLeaf"));
        prop_assert!(ty_str(&w_b).contains("adze :: WithLeaf"));
    }

    // 48. Processing one field does not affect another field's extraction.
    #[test]
    fn processing_field_does_not_affect_sibling(
        inner1 in leaf_type_name(),
        inner2 in leaf_type_name(),
    ) {
        let src = build_struct("S", &[
            ("a", &format!("Vec<{inner1}>")),
            ("b", &format!("Option<{inner2}>")),
        ]);
        let item: Item = parse_str(&src).unwrap();
        let fields = extract_struct_fields(&item);
        let ty_a: Type = parse_str(&fields[0].1).unwrap();
        let ty_b: Type = parse_str(&fields[1].1).unwrap();
        // Extract and wrap field a
        let _ = try_extract_inner_type(&ty_a, "Vec", &skip(&[]));
        let _ = wrap_leaf_type(&ty_a, &skip(&["Vec"]));
        // Field b should be unaffected
        let (ext_b, ok_b) = try_extract_inner_type(&ty_b, "Option", &skip(&[]));
        prop_assert!(ok_b);
        prop_assert_eq!(ty_str(&ext_b), inner2);
    }

    // 49. Struct with duplicate types but different names extracts all.
    #[test]
    fn duplicate_types_different_names(inner in leaf_type_name()) {
        let ftype = format!("Vec<{inner}>");
        let src = build_struct("S", &[("x", &ftype), ("y", &ftype), ("z", &ftype)]);
        let item: Item = parse_str(&src).unwrap();
        let fields = extract_struct_fields(&item);
        prop_assert_eq!(fields.len(), 3);
        for i in 0..3 {
            let ty: Type = parse_str(&fields[i].1).unwrap();
            let (ext, ok) = try_extract_inner_type(&ty, "Vec", &skip(&[]));
            prop_assert!(ok);
            prop_assert_eq!(ty_str(&ext), inner);
        }
    }

    // 50. Struct field count matches input count for various sizes.
    #[test]
    fn field_count_matches_input(
        idents in distinct_idents(8),
        ty in leaf_type_name(),
    ) {
        prop_assume!(!idents.is_empty());
        let pairs: Vec<(&str, &str)> = idents.iter().map(|id| (id.as_str(), ty)).collect();
        let src = build_struct("S", &pairs);
        let item: Item = parse_str(&src).unwrap();
        let fields = extract_struct_fields(&item);
        prop_assert_eq!(fields.len(), idents.len());
    }
}

// ===========================================================================
// 12. Nested container processing
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(60))]

    // 51. Vec<Option<T>> extracts Option<T> when target is Vec.
    #[test]
    fn nested_vec_option_extracts_outer(inner in leaf_type_name()) {
        let ftype = format!("Vec<Option<{inner}>>");
        let ty: Type = parse_str(&ftype).unwrap();
        let (extracted, ok) = try_extract_inner_type(&ty, "Vec", &skip(&[]));
        prop_assert!(ok);
        let s = ty_str(&extracted);
        prop_assert!(s.contains("Option"), "expected Option in {s}");
        prop_assert!(s.contains(inner), "expected {inner} in {s}");
    }

    // 52. Option<Vec<T>> extracts Vec<T> when target is Option.
    #[test]
    fn nested_option_vec_extracts_outer(inner in leaf_type_name()) {
        let ftype = format!("Option<Vec<{inner}>>");
        let ty: Type = parse_str(&ftype).unwrap();
        let (extracted, ok) = try_extract_inner_type(&ty, "Option", &skip(&[]));
        prop_assert!(ok);
        let s = ty_str(&extracted);
        prop_assert!(s.contains("Vec"), "expected Vec in {s}");
        prop_assert!(s.contains(inner), "expected {inner} in {s}");
    }

    // 53. filter_inner_type strips multiple nested containers in skip set.
    #[test]
    fn filter_strips_multiple_nested(inner in leaf_type_name()) {
        let ftype = format!("Box<Arc<{inner}>>");
        let ty: Type = parse_str(&ftype).unwrap();
        let filtered = filter_inner_type(&ty, &skip(&["Box", "Arc"]));
        prop_assert_eq!(ty_str(&filtered), inner);
    }

    // 54. wrap_leaf_type wraps through nested skip containers.
    #[test]
    fn wrap_through_nested_skip_containers(inner in leaf_type_name()) {
        let ftype = format!("Vec<Option<{inner}>>");
        let ty: Type = parse_str(&ftype).unwrap();
        let wrapped = wrap_leaf_type(&ty, &skip(&["Vec", "Option"]));
        let s = ty_str(&wrapped);
        prop_assert!(s.starts_with("Vec <"), "outer Vec preserved: {s}");
        prop_assert!(s.contains("Option <"), "middle Option preserved: {s}");
        prop_assert!(s.contains("adze :: WithLeaf"), "innermost wrapped: {s}");
    }

    // 55. Box<Vec<T>> with Box in skip extracts T when target is Vec.
    #[test]
    fn skip_through_box_to_vec(inner in leaf_type_name()) {
        let ftype = format!("Box<Vec<{inner}>>");
        let ty: Type = parse_str(&ftype).unwrap();
        let (extracted, ok) = try_extract_inner_type(&ty, "Vec", &skip(&["Box"]));
        prop_assert!(ok);
        prop_assert_eq!(ty_str(&extracted), inner);
    }
}

// ===========================================================================
// 13. Field type analysis (container vs plain)
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(60))]

    // 56. Plain type never extracts for any container target.
    #[test]
    fn plain_type_never_extracts(
        leaf in leaf_type_name(),
        container in container_name(),
    ) {
        let ty: Type = parse_str(leaf).unwrap();
        let (result, ok) = try_extract_inner_type(&ty, container, &skip(&[]));
        prop_assert!(!ok);
        prop_assert_eq!(ty_str(&result), leaf);
    }

    // 57. Empty skip set means only direct container match works.
    #[test]
    fn empty_skip_set_only_direct_match(
        inner in leaf_type_name(),
        container in container_name(),
    ) {
        let ftype = format!("{container}<{inner}>");
        let ty: Type = parse_str(&ftype).unwrap();
        let (_, ok) = try_extract_inner_type(&ty, container, &skip(&[]));
        prop_assert!(ok, "direct match should work with empty skip set");
    }

    // 58. filter_inner_type on a plain type is identity.
    #[test]
    fn filter_plain_type_is_identity(leaf in leaf_type_name()) {
        let ty: Type = parse_str(leaf).unwrap();
        let filtered = filter_inner_type(&ty, &skip(&["Box", "Vec", "Option", "Arc", "Rc"]));
        prop_assert_eq!(ty_str(&filtered), leaf);
    }

    // 59. wrap_leaf_type on a plain type wraps it directly.
    #[test]
    fn wrap_plain_type_wraps_directly(leaf in leaf_type_name()) {
        let ty: Type = parse_str(leaf).unwrap();
        let wrapped = wrap_leaf_type(&ty, &skip(&[]));
        prop_assert_eq!(ty_str(&wrapped), format!("adze :: WithLeaf < {leaf} >"));
    }

    // 60. Container not in skip set is treated as a leaf for wrapping.
    #[test]
    fn container_not_in_skip_treated_as_leaf(
        container in container_name(),
        inner in leaf_type_name(),
    ) {
        let ftype = format!("{container}<{inner}>");
        let ty: Type = parse_str(&ftype).unwrap();
        // skip set is empty, so the container type itself gets wrapped
        let wrapped = wrap_leaf_type(&ty, &skip(&[]));
        let s = ty_str(&wrapped);
        prop_assert!(s.starts_with("adze :: WithLeaf <"), "whole type wrapped: {s}");
    }
}
