#![allow(
    clippy::needless_range_loop,
    reason = "property and comprehensive tests use index-based loops to exercise table positions and boundary cases"
)]

//! Property-based tests for field extraction in adze-common.
//!
//! Exercises extracting fields from structs and enum variants, verifying
//! field name preservation, type processing correctness, empty/many-field
//! structs, mixed annotated/plain fields, and field ordering stability.

use adze_common::{FieldThenParams, filter_inner_type, try_extract_inner_type, wrap_leaf_type};
use proptest::prelude::*;
mod support;
use quote::ToTokens;
use std::collections::HashSet;
use support::{container_name_with_rc, distinct_lower_idents, field_leaf_type_name, lower_ident};
use syn::{Fields, Item, Type, parse_str};

// ---------------------------------------------------------------------------
// Strategies
// ---------------------------------------------------------------------------

/// Valid Rust identifiers (lowercase start, suitable for field names).
fn ident_strategy() -> impl Strategy<Value = String> {
    lower_ident(8)
}

/// Produce a vector of distinct identifiers.
fn distinct_idents(max: usize) -> impl Strategy<Value = Vec<String>> {
    distinct_lower_idents(max, 8)
}

/// Simple leaf type names that are never container names.
fn leaf_type_name() -> impl Strategy<Value = &'static str> {
    field_leaf_type_name()
}

/// Container type names used for wrapping.
fn container_name() -> impl Strategy<Value = &'static str> {
    container_name_with_rc()
}

/// Known adze attribute names.
fn adze_attr_name() -> impl Strategy<Value = &'static str> {
    prop::sample::select(&["leaf", "skip", "prec", "word", "extra", "delimited"][..])
}

/// Random subsets of container names for skip-over sets.
fn skip_set_strategy() -> impl Strategy<Value = HashSet<&'static str>> {
    prop::collection::hash_set(container_name(), 0..=5)
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

/// Capitalize the first letter to create a valid type name.
fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => {
            let upper: String = c.to_uppercase().collect();
            upper + chars.as_str()
        }
    }
}

/// Build a struct source string with named fields.
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

/// Build an enum source string where each variant has a single named field.
fn build_enum_with_struct_variants(name: &str, variants: &[(&str, &str, &str)]) -> String {
    let body: String = variants
        .iter()
        .map(|(vname, fname, ftype)| format!("    {vname} {{ {fname}: {ftype} }},\n"))
        .collect();
    format!("pub enum {name} {{\n{body}}}")
}

/// Extract named fields from a parsed struct item.
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

/// Extract fields from the first variant of an enum.
fn extract_first_variant_fields(item: &Item) -> Vec<(String, String)> {
    if let Item::Enum(e) = item
        && let Some(v) = e.variants.first()
        && let Fields::Named(ref named) = v.fields
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

// ===========================================================================
// 1. Extract fields from struct
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(80))]

    // 1. A struct with a single field yields exactly one extracted field.
    #[test]
    fn struct_single_field_extraction(
        name in ident_strategy(),
        field_name in ident_strategy(),
        field_type in leaf_type_name(),
    ) {
        let src = build_struct(&capitalize(&name), &[(&field_name, field_type)]);
        let item: Item = parse_str(&src).unwrap();
        let fields = extract_struct_fields(&item);
        prop_assert_eq!(fields.len(), 1);
        prop_assert_eq!(&fields[0].0, &field_name);
        prop_assert_eq!(fields[0].1.as_str(), field_type);
    }

    // 2. A struct with two fields yields exactly two extracted fields.
    #[test]
    fn struct_two_fields_extraction(
        name in ident_strategy(),
        f1 in ident_strategy(),
        f2 in ident_strategy(),
        t1 in leaf_type_name(),
        t2 in leaf_type_name(),
    ) {
        prop_assume!(f1 != f2);
        let src = build_struct(&capitalize(&name), &[(&f1, t1), (&f2, t2)]);
        let item: Item = parse_str(&src).unwrap();
        let fields = extract_struct_fields(&item);
        prop_assert_eq!(fields.len(), 2);
    }

    // 3. Extracted field count matches input field count for variable-size structs.
    #[test]
    fn struct_field_count_matches(
        name in ident_strategy(),
        idents in distinct_idents(6),
        ty in leaf_type_name(),
    ) {
        prop_assume!(!idents.is_empty());
        let pairs: Vec<(&str, &str)> = idents.iter().map(|id| (id.as_str(), ty)).collect();
        let src = build_struct(&capitalize(&name), &pairs);
        let item: Item = parse_str(&src).unwrap();
        let fields = extract_struct_fields(&item);
        prop_assert_eq!(fields.len(), idents.len());
    }

    // 4. Fields with generic container types are extracted with correct type string.
    #[test]
    fn struct_field_generic_type(
        name in ident_strategy(),
        field_name in ident_strategy(),
        container in container_name(),
        inner in leaf_type_name(),
    ) {
        let ftype = format!("{container}<{inner}>");
        let src = build_struct(&capitalize(&name), &[(&field_name, &ftype)]);
        let item: Item = parse_str(&src).unwrap();
        let fields = extract_struct_fields(&item);
        prop_assert_eq!(fields.len(), 1);
        prop_assert!(fields[0].1.contains(container));
        prop_assert!(fields[0].1.contains(inner));
    }
}

// ===========================================================================
// 2. Extract fields from enum variant
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(80))]

    // 5. A single enum variant with a named field yields one field.
    #[test]
    fn enum_variant_single_field(
        enum_name in ident_strategy(),
        variant_name in ident_strategy(),
        field_name in ident_strategy(),
        field_type in leaf_type_name(),
    ) {
        let en = capitalize(&enum_name);
        let vn = capitalize(&variant_name);
        let src = build_enum_with_struct_variants(&en, &[(&vn, &field_name, field_type)]);
        let item: Item = parse_str(&src).unwrap();
        let fields = extract_first_variant_fields(&item);
        prop_assert_eq!(fields.len(), 1);
        prop_assert_eq!(&fields[0].0, &field_name);
        prop_assert_eq!(fields[0].1.as_str(), field_type);
    }

    // 6. Enum variant field type includes container when generic.
    #[test]
    fn enum_variant_generic_field(
        enum_name in ident_strategy(),
        field_name in ident_strategy(),
        container in container_name(),
        inner in leaf_type_name(),
    ) {
        let en = capitalize(&enum_name);
        let ftype = format!("{container}<{inner}>");
        let src = build_enum_with_struct_variants(&en, &[("V", &field_name, &ftype)]);
        let item: Item = parse_str(&src).unwrap();
        let fields = extract_first_variant_fields(&item);
        prop_assert_eq!(fields.len(), 1);
        prop_assert!(fields[0].1.contains(container));
    }

    // 7. Multiple enum variants each have independent fields.
    #[test]
    fn enum_multiple_variants_independent(
        enum_name in ident_strategy(),
        f1 in ident_strategy(),
        f2 in ident_strategy(),
        t1 in leaf_type_name(),
        t2 in leaf_type_name(),
    ) {
        let en = capitalize(&enum_name);
        let src = format!(
            "pub enum {en} {{ A {{ {f1}: {t1} }}, B {{ {f2}: {t2} }} }}"
        );
        let item: Item = parse_str(&src).unwrap();
        if let Item::Enum(e) = &item {
            prop_assert_eq!(e.variants.len(), 2);
            if let Fields::Named(ref na) = e.variants[0].fields {
                prop_assert_eq!(na.named.len(), 1);
            }
            if let Fields::Named(ref nb) = e.variants[1].fields {
                prop_assert_eq!(nb.named.len(), 1);
            }
        }
    }
}

// ===========================================================================
// 3. Field names preserved
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(80))]

    // 8. Field name survives parse-then-serialize roundtrip.
    #[test]
    fn field_name_roundtrip(
        name in ident_strategy(),
        field_name in ident_strategy(),
        ty in leaf_type_name(),
    ) {
        let src = build_struct(&capitalize(&name), &[(&field_name, ty)]);
        let item: Item = parse_str(&src).unwrap();
        let tokens = item.to_token_stream().to_string();
        let reparsed: Item = parse_str(&tokens).unwrap();
        let fields = extract_struct_fields(&reparsed);
        prop_assert_eq!(fields.len(), 1);
        prop_assert_eq!(&fields[0].0, &field_name);
    }

    // 9. All distinct field names are preserved in extraction order.
    #[test]
    fn all_field_names_preserved(
        name in ident_strategy(),
        idents in distinct_idents(5),
        ty in leaf_type_name(),
    ) {
        prop_assume!(!idents.is_empty());
        let pairs: Vec<(&str, &str)> = idents.iter().map(|id| (id.as_str(), ty)).collect();
        let src = build_struct(&capitalize(&name), &pairs);
        let item: Item = parse_str(&src).unwrap();
        let fields = extract_struct_fields(&item);
        for i in 0..idents.len() {
            prop_assert_eq!(&fields[i].0, &idents[i]);
        }
    }

    // 10. FieldThenParams preserves field type as string through parsing.
    #[test]
    fn ftp_field_name_preserved(ty in leaf_type_name()) {
        let parsed: FieldThenParams = syn::parse_str(ty).unwrap();
        let token_str = ty_str(&parsed.field.ty);
        prop_assert_eq!(token_str.as_str(), ty);
    }

    // 11. Enum variant field name survives roundtrip.
    #[test]
    fn enum_field_name_roundtrip(
        enum_name in ident_strategy(),
        field_name in ident_strategy(),
        ty in leaf_type_name(),
    ) {
        let en = capitalize(&enum_name);
        let src = build_enum_with_struct_variants(&en, &[("V", &field_name, ty)]);
        let item: Item = parse_str(&src).unwrap();
        let tokens = item.to_token_stream().to_string();
        let reparsed: Item = parse_str(&tokens).unwrap();
        let fields = extract_first_variant_fields(&reparsed);
        prop_assert_eq!(fields.len(), 1);
        prop_assert_eq!(&fields[0].0, &field_name);
    }
}

// ===========================================================================
// 4. Field types processed correctly
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(80))]

    // 12. try_extract_inner_type on a struct field's type succeeds for matching container.
    #[test]
    fn field_type_extract_matching_container(
        field_name in ident_strategy(),
        container in container_name(),
        inner in leaf_type_name(),
    ) {
        let ftype = format!("{container}<{inner}>");
        let src = build_struct("S", &[(&field_name, &ftype)]);
        let item: Item = parse_str(&src).unwrap();
        let fields = extract_struct_fields(&item);
        let (extracted, ok) = try_extract_inner_type(
            &parse_str::<Type>(&fields[0].1).unwrap(),
            container,
            &skip(&[]),
        );
        prop_assert!(ok);
        prop_assert_eq!(ty_str(&extracted), inner);
    }

    // 13. filter_inner_type strips container from field type.
    #[test]
    fn field_type_filter_strips_container(
        field_name in ident_strategy(),
        container in container_name(),
        inner in leaf_type_name(),
    ) {
        let ftype = format!("{container}<{inner}>");
        let src = build_struct("S", &[(&field_name, &ftype)]);
        let item: Item = parse_str(&src).unwrap();
        let fields = extract_struct_fields(&item);
        let ty: Type = parse_str(&fields[0].1).unwrap();
        let skip_arr = [container];
        let filtered = filter_inner_type(&ty, &skip(&skip_arr));
        prop_assert_eq!(ty_str(&filtered), inner);
    }

    // 14. wrap_leaf_type wraps a plain field type in WithLeaf.
    #[test]
    fn field_type_wrap_plain(
        field_name in ident_strategy(),
        leaf in leaf_type_name(),
    ) {
        let src = build_struct("S", &[(&field_name, leaf)]);
        let item: Item = parse_str(&src).unwrap();
        let fields = extract_struct_fields(&item);
        let ty: Type = parse_str(&fields[0].1).unwrap();
        let wrapped = wrap_leaf_type(&ty, &HashSet::new());
        let s = ty_str(&wrapped);
        prop_assert!(s.contains("adze :: WithLeaf"), "expected WithLeaf: {s}");
        prop_assert!(s.contains(leaf), "expected inner type: {s}");
    }

    // 15. Full pipeline: extract + filter + wrap on a field type.
    #[test]
    fn field_type_full_pipeline(
        container in container_name(),
        inner in leaf_type_name(),
    ) {
        let ftype = format!("{container}<{inner}>");
        let src = build_struct("S", &[("field", &ftype)]);
        let item: Item = parse_str(&src).unwrap();
        let fields = extract_struct_fields(&item);
        let ty: Type = parse_str(&fields[0].1).unwrap();

        let (extracted, ok) = try_extract_inner_type(&ty, container, &skip(&[]));
        prop_assert!(ok);
        let wrapped = wrap_leaf_type(&extracted, &HashSet::new());
        prop_assert_eq!(ty_str(&wrapped), format!("adze :: WithLeaf < {inner} >"));
    }
}

// ===========================================================================
// 5. Empty struct
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    // 16. Empty struct yields zero fields.
    #[test]
    fn empty_struct_no_fields(name in ident_strategy()) {
        let src = build_struct(&capitalize(&name), &[]);
        let item: Item = parse_str(&src).unwrap();
        let fields = extract_struct_fields(&item);
        prop_assert!(fields.is_empty());
    }

    // 17. Unit struct yields zero fields.
    #[test]
    fn unit_struct_no_fields(name in ident_strategy()) {
        let src = format!("pub struct {};", capitalize(&name));
        let item: Item = parse_str(&src).unwrap();
        let fields = extract_struct_fields(&item);
        prop_assert!(fields.is_empty());
    }

    // 18. Empty struct roundtrips without gaining fields.
    #[test]
    fn empty_struct_roundtrip(name in ident_strategy()) {
        let src = build_struct(&capitalize(&name), &[]);
        let item: Item = parse_str(&src).unwrap();
        let tokens = item.to_token_stream().to_string();
        let reparsed: Item = parse_str(&tokens).unwrap();
        let fields = extract_struct_fields(&reparsed);
        prop_assert!(fields.is_empty());
    }
}

// ===========================================================================
// 6. Struct with many fields
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(40))]

    // 19. Struct with up to 8 distinct fields parses and extracts them all.
    #[test]
    fn many_fields_all_extracted(
        name in ident_strategy(),
        idents in distinct_idents(8),
        ty in leaf_type_name(),
    ) {
        prop_assume!(idents.len() >= 3);
        let pairs: Vec<(&str, &str)> = idents.iter().map(|id| (id.as_str(), ty)).collect();
        let src = build_struct(&capitalize(&name), &pairs);
        let item: Item = parse_str(&src).unwrap();
        let fields = extract_struct_fields(&item);
        prop_assert_eq!(fields.len(), idents.len());
    }

    // 20. All field types in a many-field struct match the input type.
    #[test]
    fn many_fields_types_match(
        name in ident_strategy(),
        idents in distinct_idents(8),
        ty in leaf_type_name(),
    ) {
        prop_assume!(idents.len() >= 3);
        let pairs: Vec<(&str, &str)> = idents.iter().map(|id| (id.as_str(), ty)).collect();
        let src = build_struct(&capitalize(&name), &pairs);
        let item: Item = parse_str(&src).unwrap();
        let fields = extract_struct_fields(&item);
        for i in 0..fields.len() {
            prop_assert_eq!(fields[i].1.as_str(), ty);
        }
    }

    // 21. Many-field struct roundtrips correctly.
    #[test]
    fn many_fields_roundtrip(
        name in ident_strategy(),
        idents in distinct_idents(6),
        ty in leaf_type_name(),
    ) {
        prop_assume!(idents.len() >= 2);
        let pairs: Vec<(&str, &str)> = idents.iter().map(|id| (id.as_str(), ty)).collect();
        let src = build_struct(&capitalize(&name), &pairs);
        let item: Item = parse_str(&src).unwrap();
        let tokens = item.to_token_stream().to_string();
        let reparsed: Item = parse_str(&tokens).unwrap();
        let orig = extract_struct_fields(&item);
        let rt = extract_struct_fields(&reparsed);
        prop_assert_eq!(orig.len(), rt.len());
        for i in 0..orig.len() {
            prop_assert_eq!(&orig[i].0, &rt[i].0);
            prop_assert_eq!(&orig[i].1, &rt[i].1);
        }
    }
}

// ===========================================================================
// 7. Mixed annotated/plain fields
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    // 22. Annotated fields preserve their attributes alongside plain fields.
    #[test]
    fn mixed_annotated_plain_fields(
        name in ident_strategy(),
        f1 in ident_strategy(),
        f2 in ident_strategy(),
        t1 in leaf_type_name(),
        t2 in leaf_type_name(),
        attr_name in adze_attr_name(),
    ) {
        prop_assume!(f1 != f2);
        let src = format!(
            "pub struct {} {{\n    #[adze::{attr_name}]\n    pub {f1}: {t1},\n    pub {f2}: {t2},\n}}",
            capitalize(&name)
        );
        let item: Item = parse_str(&src).unwrap();
        let fields = extract_struct_fields(&item);
        prop_assert_eq!(fields.len(), 2);
        prop_assert_eq!(&fields[0].0, &f1);
        prop_assert_eq!(&fields[1].0, &f2);

        // Verify the attribute is on the first field only.
        if let Item::Struct(s) = &item
            && let Fields::Named(ref named) = s.fields
        {
            prop_assert_eq!(named.named[0].attrs.len(), 1);
            prop_assert!(named.named[1].attrs.is_empty());
        }
    }

    // 23. Annotated field count matches expectations.
    #[test]
    fn annotated_field_count(
        name in ident_strategy(),
        idents in distinct_idents(4),
        ty in leaf_type_name(),
        annotate_idx in 0usize..4,
    ) {
        prop_assume!(idents.len() >= 2);
        let idx = annotate_idx % idents.len();
        let body: String = idents
            .iter()
            .enumerate()
            .map(|(i, id)| {
                if i == idx {
                    format!("    #[adze::leaf]\n    pub {id}: {ty},\n")
                } else {
                    format!("    pub {id}: {ty},\n")
                }
            })
            .collect();
        let src = format!("pub struct {} {{\n{body}}}", capitalize(&name));
        let item: Item = parse_str(&src).unwrap();

        if let Item::Struct(s) = &item
            && let Fields::Named(ref named) = s.fields
        {
            let annotated_count = named.named.iter().filter(|f| !f.attrs.is_empty()).count();
            prop_assert_eq!(annotated_count, 1);
        }
    }

    // 24. FieldThenParams with and without params both parse correctly.
    #[test]
    fn ftp_mixed_params(
        ty in leaf_type_name(),
        key in ident_strategy(),
        val in 0i64..100,
        has_params in prop::bool::ANY,
    ) {
        let src = if has_params {
            format!("{ty}, {key} = {val}")
        } else {
            ty.to_string()
        };
        let parsed: FieldThenParams = syn::parse_str(&src).unwrap();
        if has_params {
            prop_assert!(parsed.comma.is_some());
            prop_assert_eq!(parsed.params.len(), 1);
        } else {
            prop_assert!(parsed.comma.is_none());
            prop_assert!(parsed.params.is_empty());
        }
    }

    // 25. Enum variant with mix of annotated and plain fields.
    #[test]
    fn enum_variant_mixed_annotated(
        enum_name in ident_strategy(),
        f1 in ident_strategy(),
        f2 in ident_strategy(),
        t1 in leaf_type_name(),
        t2 in leaf_type_name(),
    ) {
        prop_assume!(f1 != f2);
        let en = capitalize(&enum_name);
        let src = format!(
            "pub enum {en} {{ V {{ #[adze::leaf] {f1}: {t1}, {f2}: {t2} }} }}"
        );
        let item: Item = parse_str(&src).unwrap();
        if let Item::Enum(e) = &item
            && let Fields::Named(ref named) = e.variants[0].fields
        {
            prop_assert_eq!(named.named.len(), 2);
            prop_assert_eq!(named.named[0].attrs.len(), 1);
            prop_assert!(named.named[1].attrs.is_empty());
        }
    }
}

// ===========================================================================
// 8. Field ordering stability
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(80))]

    // 26. Field extraction order matches declaration order.
    #[test]
    fn field_order_matches_declaration(
        name in ident_strategy(),
        idents in distinct_idents(5),
        ty in leaf_type_name(),
    ) {
        prop_assume!(idents.len() >= 2);
        let pairs: Vec<(&str, &str)> = idents.iter().map(|id| (id.as_str(), ty)).collect();
        let src = build_struct(&capitalize(&name), &pairs);
        let item: Item = parse_str(&src).unwrap();
        let fields = extract_struct_fields(&item);
        for i in 0..idents.len() {
            prop_assert_eq!(&fields[i].0, &idents[i]);
        }
    }

    // 27. Re-parsing preserves field order.
    #[test]
    fn field_order_stable_after_reparse(
        name in ident_strategy(),
        idents in distinct_idents(5),
        ty in leaf_type_name(),
    ) {
        prop_assume!(idents.len() >= 2);
        let pairs: Vec<(&str, &str)> = idents.iter().map(|id| (id.as_str(), ty)).collect();
        let src = build_struct(&capitalize(&name), &pairs);
        let item: Item = parse_str(&src).unwrap();
        let tokens = item.to_token_stream().to_string();
        let reparsed: Item = parse_str(&tokens).unwrap();
        let orig = extract_struct_fields(&item);
        let rt = extract_struct_fields(&reparsed);
        for i in 0..orig.len() {
            prop_assert_eq!(&orig[i].0, &rt[i].0);
        }
    }

    // 28. Extraction is deterministic: two parses of same source yield same fields.
    #[test]
    fn field_extraction_deterministic(
        name in ident_strategy(),
        idents in distinct_idents(4),
        ty in leaf_type_name(),
    ) {
        prop_assume!(!idents.is_empty());
        let pairs: Vec<(&str, &str)> = idents.iter().map(|id| (id.as_str(), ty)).collect();
        let src = build_struct(&capitalize(&name), &pairs);
        let item1: Item = parse_str(&src).unwrap();
        let item2: Item = parse_str(&src).unwrap();
        let f1 = extract_struct_fields(&item1);
        let f2 = extract_struct_fields(&item2);
        prop_assert_eq!(f1, f2);
    }

    // 29. FieldThenParams param ordering is stable.
    #[test]
    fn ftp_param_ordering_stable(
        ty in leaf_type_name(),
        keys in distinct_idents(3),
    ) {
        prop_assume!(keys.len() >= 2);
        let params: Vec<String> = keys.iter().enumerate()
            .map(|(i, k)| format!("{k} = {i}"))
            .collect();
        let src = format!("{ty}, {}", params.join(", "));
        let parsed: FieldThenParams = syn::parse_str(&src).unwrap();
        for i in 0..keys.len() {
            prop_assert_eq!(parsed.params[i].path.to_string(), keys[i].as_str());
        }
    }

    // 30. Type processing on extracted fields preserves ordering.
    #[test]
    fn type_processing_preserves_field_order(
        idents in distinct_idents(4),
        container in container_name(),
        inner in leaf_type_name(),
    ) {
        prop_assume!(idents.len() >= 2);
        let ftype = format!("{container}<{inner}>");
        let pairs: Vec<(&str, &str)> = idents.iter().map(|id| (id.as_str(), ftype.as_str())).collect();
        let src = build_struct("S", &pairs);
        let item: Item = parse_str(&src).unwrap();
        let fields = extract_struct_fields(&item);

        let processed: Vec<(String, String)> = fields
            .iter()
            .map(|(name, ty_s)| {
                let ty: Type = parse_str(ty_s).unwrap();
                let (extracted, _) = try_extract_inner_type(&ty, container, &skip(&[]));
                (name.clone(), ty_str(&extracted))
            })
            .collect();

        for i in 0..idents.len() {
            prop_assert_eq!(&processed[i].0, &idents[i]);
        }
    }

    // 31. Field types from struct fields are always parseable as Type.
    #[test]
    fn extracted_field_types_parseable(
        name in ident_strategy(),
        field_name in ident_strategy(),
        container in container_name(),
        inner in leaf_type_name(),
        skip_s in skip_set_strategy(),
    ) {
        let ftype = format!("{container}<{inner}>");
        let src = build_struct(&capitalize(&name), &[(&field_name, &ftype)]);
        let item: Item = parse_str(&src).unwrap();
        let fields = extract_struct_fields(&item);
        let ty: Type = parse_str(&fields[0].1).unwrap();
        let filtered = filter_inner_type(&ty, &skip_s);
        let s = ty_str(&filtered);
        prop_assert!(parse_str::<Type>(&s).is_ok(), "unparseable: {s}");
    }

    // 32. wrap_leaf_type on each extracted field yields parseable types.
    #[test]
    fn wrapped_field_types_parseable(
        idents in distinct_idents(3),
        ty in leaf_type_name(),
        skip_s in skip_set_strategy(),
    ) {
        prop_assume!(!idents.is_empty());
        let pairs: Vec<(&str, &str)> = idents.iter().map(|id| (id.as_str(), ty)).collect();
        let src = build_struct("S", &pairs);
        let item: Item = parse_str(&src).unwrap();
        let fields = extract_struct_fields(&item);
        for (_, ty_s) in &fields {
            let fty: Type = parse_str(ty_s).unwrap();
            let wrapped = wrap_leaf_type(&fty, &skip_s);
            let s = ty_str(&wrapped);
            prop_assert!(parse_str::<Type>(&s).is_ok(), "unparseable: {s}");
        }
    }

    // 33. Enum variant fields maintain order across multiple variants.
    #[test]
    fn enum_variant_field_order(
        f1 in ident_strategy(),
        f2 in ident_strategy(),
        t1 in leaf_type_name(),
        t2 in leaf_type_name(),
    ) {
        prop_assume!(f1 != f2);
        let src = format!("pub enum E {{ V {{ {f1}: {t1}, {f2}: {t2} }} }}");
        let item: Item = parse_str(&src).unwrap();
        let fields = extract_first_variant_fields(&item);
        prop_assert_eq!(fields.len(), 2);
        prop_assert_eq!(&fields[0].0, &f1);
        prop_assert_eq!(&fields[1].0, &f2);
    }

    // 34. Mixed container/leaf field types all process without panic.
    #[test]
    fn mixed_field_types_no_panic(
        f1 in ident_strategy(),
        f2 in ident_strategy(),
        leaf in leaf_type_name(),
        container in container_name(),
        inner in leaf_type_name(),
    ) {
        prop_assume!(f1 != f2);
        let ftype = format!("{container}<{inner}>");
        let src = build_struct("S", &[(&f1, leaf), (&f2, &ftype)]);
        let item: Item = parse_str(&src).unwrap();
        let fields = extract_struct_fields(&item);
        for (_, ty_s) in &fields {
            let fty: Type = parse_str(ty_s).unwrap();
            let _ = try_extract_inner_type(&fty, container, &skip(&[]));
            let _ = filter_inner_type(&fty, &skip(&[container]));
            let _ = wrap_leaf_type(&fty, &skip(&[container]));
        }
    }

    // 35. Annotated fields don't affect ordering of extracted (name, type) pairs.
    #[test]
    fn annotations_dont_affect_field_order(
        idents in distinct_idents(4),
        ty in leaf_type_name(),
        attr_name in adze_attr_name(),
    ) {
        prop_assume!(idents.len() >= 2);

        // Build struct without annotations.
        let plain_pairs: Vec<(&str, &str)> = idents.iter().map(|id| (id.as_str(), ty)).collect();
        let plain_src = build_struct("S", &plain_pairs);
        let plain_item: Item = parse_str(&plain_src).unwrap();
        let plain_fields = extract_struct_fields(&plain_item);

        // Build struct with annotation on first field.
        let body: String = idents
            .iter()
            .enumerate()
            .map(|(i, id)| {
                if i == 0 {
                    format!("    #[adze::{attr_name}]\n    pub {id}: {ty},\n")
                } else {
                    format!("    pub {id}: {ty},\n")
                }
            })
            .collect();
        let annotated_src = format!("pub struct S {{\n{body}}}");
        let annotated_item: Item = parse_str(&annotated_src).unwrap();
        let annotated_fields = extract_struct_fields(&annotated_item);

        prop_assert_eq!(plain_fields.len(), annotated_fields.len());
        for i in 0..plain_fields.len() {
            prop_assert_eq!(&plain_fields[i].0, &annotated_fields[i].0);
            prop_assert_eq!(&plain_fields[i].1, &annotated_fields[i].1);
        }
    }
}
