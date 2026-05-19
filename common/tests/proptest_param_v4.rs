//! Property-based tests (v4) for parameterized type detection and analysis.
//!
//! 46 proptest properties across 8 categories covering: is_parameterized detection,
//! extraction from parameterized types, filtering, wrapping, nesting, roundtrips,
//! identity properties, and edge cases.

use adze_common::{filter_inner_type, try_extract_inner_type, wrap_leaf_type};
use proptest::prelude::*;
use quote::ToTokens;
use std::collections::HashSet;
use syn::{Type, parse_str};

// ---------------------------------------------------------------------------
// Strategies
// ---------------------------------------------------------------------------

/// Leaf primitives that are always valid, non-keyword Rust types.
#[allow(dead_code)]
fn leaf_type() -> impl Strategy<Value = &'static str> {
    prop::sample::select(
        &[
            "i8", "i16", "i32", "i64", "u8", "u16", "u32", "u64", "f32", "f64", "bool", "char",
            "String", "usize", "isize",
        ][..],
    )
}

/// Common single-arg generic containers.
#[allow(dead_code)]
fn container_name() -> impl Strategy<Value = &'static str> {
    prop::sample::select(&["Option", "Vec", "Box", "Arc", "Rc"][..])
}

/// Containers suitable for skip sets (not typically extraction targets).
#[allow(dead_code)]
fn skip_member() -> impl Strategy<Value = &'static str> {
    prop::sample::select(&["Box", "Arc", "Rc"][..])
}

/// Containers typically used as extraction targets.
#[allow(dead_code)]
fn target_container() -> impl Strategy<Value = &'static str> {
    prop::sample::select(&["Option", "Vec"][..])
}

/// Depth-1 parameterized type string.
#[allow(dead_code)]
fn param_type_string() -> impl Strategy<Value = String> {
    (container_name(), leaf_type()).prop_map(|(c, l)| format!("{c}<{l}>"))
}

/// Depth-2 nested parameterized type string.
#[allow(dead_code)]
fn nested_param_string() -> impl Strategy<Value = String> {
    (container_name(), container_name(), leaf_type())
        .prop_map(|(c1, c2, l)| format!("{c1}<{c2}<{l}>>"))
}

/// Depth-3 deeply nested parameterized type string.
#[allow(dead_code)]
fn deep_nested_string() -> impl Strategy<Value = String> {
    (
        container_name(),
        container_name(),
        container_name(),
        leaf_type(),
    )
        .prop_map(|(c1, c2, c3, l)| format!("{c1}<{c2}<{c3}<{l}>>>"))
}

/// Any type string from depth 0–2.
#[allow(dead_code)]
fn any_type_string() -> impl Strategy<Value = String> {
    prop_oneof![
        leaf_type().prop_map(|s| s.to_string()),
        param_type_string(),
        nested_param_string(),
    ]
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

#[allow(dead_code)]
fn to_str(ty: &Type) -> String {
    ty.to_token_stream().to_string()
}

#[allow(dead_code)]
fn is_parameterized(ty: &Type) -> bool {
    if let Type::Path(p) = ty
        && let Some(seg) = p.path.segments.last()
    {
        return matches!(seg.arguments, syn::PathArguments::AngleBracketed(_));
    }
    false
}

#[allow(dead_code)]
fn empty_skip() -> HashSet<&'static str> {
    HashSet::new()
}

#[allow(dead_code)]
fn skip_set<'a>(items: &'a [&'a str]) -> HashSet<&'a str> {
    items.iter().copied().collect()
}

#[allow(dead_code)]
fn has_angle_brackets(s: &str) -> bool {
    s.contains('<') && s.contains('>')
}

// ===========================================================================
// 1. prop_param_detect_* — is_parameterized detection (6 tests)
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    /// Plain leaf types are never parameterized.
    #[test]
    fn prop_param_detect_leaf_is_not_parameterized(leaf in leaf_type()) {
        let ty: Type = parse_str(leaf).unwrap();
        prop_assert!(!is_parameterized(&ty));
    }

    /// Container<Leaf> is always parameterized.
    #[test]
    fn prop_param_detect_container_leaf_is_parameterized(
        container in container_name(),
        leaf in leaf_type(),
    ) {
        let ty: Type = parse_str(&format!("{container}<{leaf}>")).unwrap();
        prop_assert!(is_parameterized(&ty));
    }

    /// Nested Container<Container<Leaf>> is parameterized.
    #[test]
    fn prop_param_detect_nested_is_parameterized(
        c1 in container_name(),
        c2 in container_name(),
        leaf in leaf_type(),
    ) {
        let ty: Type = parse_str(&format!("{c1}<{c2}<{leaf}>>")).unwrap();
        prop_assert!(is_parameterized(&ty));
    }

    /// Triple-nested types are parameterized.
    #[test]
    fn prop_param_detect_deep_nested_is_parameterized(
        c1 in container_name(),
        c2 in container_name(),
        c3 in container_name(),
        leaf in leaf_type(),
    ) {
        let ty: Type = parse_str(&format!("{c1}<{c2}<{c3}<{leaf}>>>")).unwrap();
        prop_assert!(is_parameterized(&ty));
    }

    /// Extraction result from parameterized type is a leaf (not parameterized).
    #[test]
    fn prop_param_detect_extracted_inner_is_leaf(
        container in container_name(),
        leaf in leaf_type(),
    ) {
        let ty: Type = parse_str(&format!("{container}<{leaf}>")).unwrap();
        let (inner, extracted) = try_extract_inner_type(&ty, container, &empty_skip());
        prop_assert!(extracted);
        prop_assert!(!is_parameterized(&inner));
    }

    /// Reference types are not parameterized.
    #[test]
    fn prop_param_detect_reference_not_parameterized(leaf in leaf_type()) {
        let ty: Type = parse_str(&format!("&{leaf}")).unwrap();
        prop_assert!(!is_parameterized(&ty));
    }
}

// ===========================================================================
// 2. prop_param_extract_* — extraction from parameterized (6 tests)
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    /// Extracting with matching container always succeeds and yields the leaf.
    #[test]
    fn prop_param_extract_matching_yields_leaf(
        container in container_name(),
        leaf in leaf_type(),
    ) {
        let ty: Type = parse_str(&format!("{container}<{leaf}>")).unwrap();
        let (inner, extracted) = try_extract_inner_type(&ty, container, &empty_skip());
        prop_assert!(extracted);
        prop_assert_eq!(to_str(&inner), leaf);
    }

    /// Extracting from a non-parameterized leaf always fails.
    #[test]
    fn prop_param_extract_from_leaf_fails(
        leaf in leaf_type(),
        target in container_name(),
    ) {
        let ty: Type = parse_str(leaf).unwrap();
        let (_result, extracted) = try_extract_inner_type(&ty, target, &empty_skip());
        prop_assert!(!extracted);
    }

    /// Extracting with wrong target name fails and returns original.
    #[test]
    fn prop_param_extract_wrong_target_returns_original(leaf in leaf_type()) {
        let ty: Type = parse_str(&format!("Vec<{leaf}>")).unwrap();
        let (result, extracted) = try_extract_inner_type(&ty, "Option", &empty_skip());
        prop_assert!(!extracted);
        prop_assert_eq!(to_str(&result), format!("Vec < {leaf} >"));
    }

    /// Extracting through skip layer reaches the inner target.
    #[test]
    fn prop_param_extract_through_skip(
        skipper in skip_member(),
        leaf in leaf_type(),
    ) {
        let ty: Type = parse_str(&format!("{skipper}<Vec<{leaf}>>")).unwrap();
        let arr = [skipper];
        let s = skip_set(&arr);
        let (inner, extracted) = try_extract_inner_type(&ty, "Vec", &s);
        prop_assert!(extracted);
        prop_assert_eq!(to_str(&inner), leaf);
    }

    /// Extracting through double skip layers works correctly.
    #[test]
    fn prop_param_extract_through_double_skip(leaf in leaf_type()) {
        let ty: Type = parse_str(&format!("Box<Arc<Option<{leaf}>>>")).unwrap();
        let s = skip_set(&["Box", "Arc"]);
        let (inner, extracted) = try_extract_inner_type(&ty, "Option", &s);
        prop_assert!(extracted);
        prop_assert_eq!(to_str(&inner), leaf);
    }

    /// Skip layer without matching inner target returns original type.
    #[test]
    fn prop_param_extract_skip_no_target_returns_original(
        skipper in skip_member(),
        leaf in leaf_type(),
    ) {
        let ty: Type = parse_str(&format!("{skipper}<{leaf}>")).unwrap();
        let arr = [skipper];
        let s = skip_set(&arr);
        let (result, extracted) = try_extract_inner_type(&ty, "Option", &s);
        prop_assert!(!extracted);
        prop_assert_eq!(to_str(&result), format!("{skipper} < {leaf} >"));
    }
}

// ===========================================================================
// 3. prop_param_filter_* — filtering parameterized types (6 tests)
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    /// Filtering a skip-set container unwraps to the inner type.
    #[test]
    fn prop_param_filter_skip_unwraps(
        skipper in skip_member(),
        leaf in leaf_type(),
    ) {
        let ty: Type = parse_str(&format!("{skipper}<{leaf}>")).unwrap();
        let arr = [skipper];
        let s = skip_set(&arr);
        let filtered = filter_inner_type(&ty, &s);
        prop_assert_eq!(to_str(&filtered), leaf);
    }

    /// Filtering a non-skip container returns it unchanged.
    #[test]
    fn prop_param_filter_non_skip_unchanged(
        target in target_container(),
        leaf in leaf_type(),
    ) {
        let ty: Type = parse_str(&format!("{target}<{leaf}>")).unwrap();
        let s = skip_set(&["Box", "Arc", "Rc"]);
        let filtered = filter_inner_type(&ty, &s);
        prop_assert_eq!(to_str(&filtered), format!("{target} < {leaf} >"));
    }

    /// Filtering a plain leaf with any skip set returns it unchanged.
    #[test]
    fn prop_param_filter_leaf_unchanged(
        leaf in leaf_type(),
        skipper in skip_member(),
    ) {
        let ty: Type = parse_str(leaf).unwrap();
        let arr = [skipper];
        let s = skip_set(&arr);
        let filtered = filter_inner_type(&ty, &s);
        prop_assert_eq!(to_str(&filtered), leaf);
    }

    /// Filtering with empty skip set always returns original.
    #[test]
    fn prop_param_filter_empty_skip_identity(ts in any_type_string()) {
        let ty: Type = parse_str(&ts).unwrap();
        let filtered = filter_inner_type(&ty, &empty_skip());
        prop_assert_eq!(to_str(&filtered), to_str(&ty));
    }

    /// Filtering Box<Arc<Leaf>> with both in skip yields the leaf.
    #[test]
    fn prop_param_filter_double_skip_peels_both(leaf in leaf_type()) {
        let ty: Type = parse_str(&format!("Box<Arc<{leaf}>>")).unwrap();
        let s = skip_set(&["Box", "Arc"]);
        let filtered = filter_inner_type(&ty, &s);
        prop_assert_eq!(to_str(&filtered), leaf);
    }

    /// Filtering is idempotent: filter(filter(x)) == filter(x).
    #[test]
    fn prop_param_filter_idempotent(
        container in container_name(),
        leaf in leaf_type(),
    ) {
        let ty: Type = parse_str(&format!("{container}<{leaf}>")).unwrap();
        let s = skip_set(&["Box", "Arc", "Rc"]);
        let once = filter_inner_type(&ty, &s);
        let twice = filter_inner_type(&once, &s);
        prop_assert_eq!(to_str(&once), to_str(&twice));
    }
}

// ===========================================================================
// 4. prop_param_wrap_* — wrapping operations (6 tests)
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    /// Wrapping a leaf type with empty skip always produces WithLeaf wrapper.
    #[test]
    fn prop_param_wrap_leaf_wraps(leaf in leaf_type()) {
        let ty: Type = parse_str(leaf).unwrap();
        let wrapped = wrap_leaf_type(&ty, &empty_skip());
        prop_assert_eq!(to_str(&wrapped), format!("adze :: WithLeaf < {leaf} >"));
    }

    /// Wrapping a container in skip set preserves the container, wraps the inner.
    #[test]
    fn prop_param_wrap_skip_preserves_container(
        container in container_name(),
        leaf in leaf_type(),
    ) {
        let ty: Type = parse_str(&format!("{container}<{leaf}>")).unwrap();
        let arr = [container];
        let s = skip_set(&arr);
        let wrapped = wrap_leaf_type(&ty, &s);
        let expected = format!("{container} < adze :: WithLeaf < {leaf} > >");
        prop_assert_eq!(to_str(&wrapped), expected);
    }

    /// Wrapping a container NOT in skip set wraps the entire thing.
    #[test]
    fn prop_param_wrap_non_skip_wraps_entirely(
        target in target_container(),
        leaf in leaf_type(),
    ) {
        let ty: Type = parse_str(&format!("{target}<{leaf}>")).unwrap();
        // skip set does not contain the target
        let s = skip_set(&["Box", "Arc", "Rc"]);
        let wrapped = wrap_leaf_type(&ty, &s);
        let expected = format!("adze :: WithLeaf < {target} < {leaf} > >");
        prop_assert_eq!(to_str(&wrapped), expected);
    }

    /// Wrapping with empty skip always produces a parameterized result.
    #[test]
    fn prop_param_wrap_result_is_parameterized(leaf in leaf_type()) {
        let ty: Type = parse_str(leaf).unwrap();
        let wrapped = wrap_leaf_type(&ty, &empty_skip());
        prop_assert!(is_parameterized(&wrapped));
    }

    /// Wrapping a skip-set container keeps the outer container parameterized.
    #[test]
    fn prop_param_wrap_skip_container_stays_parameterized(
        container in container_name(),
        leaf in leaf_type(),
    ) {
        let ty: Type = parse_str(&format!("{container}<{leaf}>")).unwrap();
        let arr = [container];
        let s = skip_set(&arr);
        let wrapped = wrap_leaf_type(&ty, &s);
        prop_assert!(is_parameterized(&wrapped));
    }

    /// Wrapping is deterministic: two calls produce the same output.
    #[test]
    fn prop_param_wrap_is_deterministic(
        container in container_name(),
        leaf in leaf_type(),
    ) {
        let ty: Type = parse_str(&format!("{container}<{leaf}>")).unwrap();
        let arr = [container];
        let s = skip_set(&arr);
        let w1 = wrap_leaf_type(&ty, &s);
        let w2 = wrap_leaf_type(&ty, &s);
        prop_assert_eq!(to_str(&w1), to_str(&w2));
    }
}

// ===========================================================================
// 5. prop_param_nested_* — nested parameterized types (6 tests)
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    /// Extracting from outer container of nested type yields inner parameterized type.
    #[test]
    fn prop_param_nested_extract_outer_yields_inner(
        c1 in container_name(),
        c2 in container_name(),
        leaf in leaf_type(),
    ) {
        let ty: Type = parse_str(&format!("{c1}<{c2}<{leaf}>>")).unwrap();
        let (inner, extracted) = try_extract_inner_type(&ty, c1, &empty_skip());
        prop_assert!(extracted);
        prop_assert_eq!(to_str(&inner), format!("{c2} < {leaf} >"));
    }

    /// Extracted inner from nested type is itself parameterized.
    #[test]
    fn prop_param_nested_inner_is_parameterized(
        c1 in container_name(),
        c2 in container_name(),
        leaf in leaf_type(),
    ) {
        let ty: Type = parse_str(&format!("{c1}<{c2}<{leaf}>>")).unwrap();
        let (inner, extracted) = try_extract_inner_type(&ty, c1, &empty_skip());
        prop_assert!(extracted);
        prop_assert!(is_parameterized(&inner));
    }

    /// Filter through nested skip containers reaches the leaf.
    #[test]
    fn prop_param_nested_filter_multi_skip(leaf in leaf_type()) {
        let ty: Type = parse_str(&format!("Box<Arc<Rc<{leaf}>>>")).unwrap();
        let s = skip_set(&["Box", "Arc", "Rc"]);
        let filtered = filter_inner_type(&ty, &s);
        prop_assert_eq!(to_str(&filtered), leaf);
        prop_assert!(!is_parameterized(&filtered));
    }

    /// Wrapping nested skip containers wraps only the innermost leaf.
    #[test]
    fn prop_param_nested_wrap_inner_leaf(leaf in leaf_type()) {
        let ty: Type = parse_str(&format!("Vec<Option<{leaf}>>")).unwrap();
        let s = skip_set(&["Vec", "Option"]);
        let wrapped = wrap_leaf_type(&ty, &s);
        let expected = format!("Vec < Option < adze :: WithLeaf < {leaf} > > >");
        prop_assert_eq!(to_str(&wrapped), expected);
    }

    /// Nested extraction: extract outer, then extract inner from result.
    #[test]
    fn prop_param_nested_double_extraction(
        c1 in container_name(),
        c2 in container_name(),
        leaf in leaf_type(),
    ) {
        let ty: Type = parse_str(&format!("{c1}<{c2}<{leaf}>>")).unwrap();
        let (mid, ok1) = try_extract_inner_type(&ty, c1, &empty_skip());
        prop_assert!(ok1);
        let (inner, ok2) = try_extract_inner_type(&mid, c2, &empty_skip());
        prop_assert!(ok2);
        prop_assert_eq!(to_str(&inner), leaf);
    }

    /// Triple-nested: filter peels all three layers when all are in skip set.
    #[test]
    fn prop_param_nested_triple_filter(leaf in leaf_type()) {
        let ty: Type = parse_str(&format!("Rc<Arc<Box<{leaf}>>>")).unwrap();
        let s = skip_set(&["Rc", "Arc", "Box"]);
        let filtered = filter_inner_type(&ty, &s);
        prop_assert_eq!(to_str(&filtered), leaf);
    }
}

// ===========================================================================
// 6. prop_param_roundtrip_* — roundtrip properties (6 tests)
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    /// Extract then re-check: extracted inner from Container<Leaf> is the leaf string.
    #[test]
    fn prop_param_roundtrip_extract_matches_leaf(
        container in container_name(),
        leaf in leaf_type(),
    ) {
        let ty: Type = parse_str(&format!("{container}<{leaf}>")).unwrap();
        let (inner, ok) = try_extract_inner_type(&ty, container, &empty_skip());
        prop_assert!(ok);
        let reparsed: Type = parse_str(&to_str(&inner)).unwrap();
        prop_assert_eq!(to_str(&reparsed), leaf);
    }

    /// Filter then reparse: filtered output parses back to the same type.
    #[test]
    fn prop_param_roundtrip_filter_reparse(
        skipper in skip_member(),
        leaf in leaf_type(),
    ) {
        let ty: Type = parse_str(&format!("{skipper}<{leaf}>")).unwrap();
        let arr = [skipper];
        let s = skip_set(&arr);
        let filtered = filter_inner_type(&ty, &s);
        let reparsed: Type = parse_str(&to_str(&filtered)).unwrap();
        prop_assert_eq!(to_str(&reparsed), to_str(&filtered));
    }

    /// Wrap then check: wrapped output always contains "adze :: WithLeaf".
    #[test]
    fn prop_param_roundtrip_wrap_contains_with_leaf(leaf in leaf_type()) {
        let ty: Type = parse_str(leaf).unwrap();
        let wrapped = wrap_leaf_type(&ty, &empty_skip());
        let s = to_str(&wrapped);
        prop_assert!(s.contains("WithLeaf"));
    }

    /// Extraction roundtrip: extract Container<Leaf>, reparse leaf, compare.
    #[test]
    fn prop_param_roundtrip_extraction_reparse_stable(
        container in container_name(),
        leaf in leaf_type(),
    ) {
        let ty: Type = parse_str(&format!("{container}<{leaf}>")).unwrap();
        let (inner, _) = try_extract_inner_type(&ty, container, &empty_skip());
        let s = to_str(&inner);
        let reparsed: Type = parse_str(&s).unwrap();
        prop_assert_eq!(to_str(&reparsed), s);
    }

    /// Filter and extract agree on the same container/leaf.
    #[test]
    fn prop_param_roundtrip_filter_extract_agree(
        container in container_name(),
        leaf in leaf_type(),
    ) {
        let ty: Type = parse_str(&format!("{container}<{leaf}>")).unwrap();
        let arr = [container];
        let filtered = filter_inner_type(&ty, &skip_set(&arr));
        let (extracted, ok) = try_extract_inner_type(&ty, container, &empty_skip());
        prop_assert!(ok);
        prop_assert_eq!(to_str(&filtered), to_str(&extracted));
    }

    /// Wrap roundtrip: wrapping then reparsing produces same token stream.
    #[test]
    fn prop_param_roundtrip_wrap_reparse_stable(
        container in container_name(),
        leaf in leaf_type(),
    ) {
        let ty: Type = parse_str(&format!("{container}<{leaf}>")).unwrap();
        let arr = [container];
        let s = skip_set(&arr);
        let wrapped = wrap_leaf_type(&ty, &s);
        let ws = to_str(&wrapped);
        let reparsed: Type = parse_str(&ws).unwrap();
        prop_assert_eq!(to_str(&reparsed), ws);
    }
}

// ===========================================================================
// 7. prop_param_identity_* — identity properties (6 tests)
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    /// Filtering with empty skip set is identity for any type.
    #[test]
    fn prop_param_identity_filter_empty_skip(ts in any_type_string()) {
        let ty: Type = parse_str(&ts).unwrap();
        let filtered = filter_inner_type(&ty, &empty_skip());
        prop_assert_eq!(to_str(&filtered), to_str(&ty));
    }

    /// Extraction with mismatched target preserves original.
    #[test]
    fn prop_param_identity_extract_mismatch_preserves(
        leaf in leaf_type(),
    ) {
        let ty: Type = parse_str(&format!("Vec<{leaf}>")).unwrap();
        let (result, extracted) = try_extract_inner_type(&ty, "Option", &empty_skip());
        prop_assert!(!extracted);
        prop_assert_eq!(to_str(&result), to_str(&ty));
    }

    /// Extracting a leaf preserves type identity.
    #[test]
    fn prop_param_identity_extract_leaf_preserves(
        leaf in leaf_type(),
        target in container_name(),
    ) {
        let ty: Type = parse_str(leaf).unwrap();
        let (result, extracted) = try_extract_inner_type(&ty, target, &empty_skip());
        prop_assert!(!extracted);
        prop_assert_eq!(to_str(&result), leaf);
    }

    /// Filtering a leaf is identity regardless of skip set contents.
    #[test]
    fn prop_param_identity_filter_leaf(
        leaf in leaf_type(),
    ) {
        let ty: Type = parse_str(leaf).unwrap();
        let s = skip_set(&["Box", "Arc", "Rc", "Vec", "Option"]);
        let filtered = filter_inner_type(&ty, &s);
        prop_assert_eq!(to_str(&filtered), leaf);
    }

    /// Double-filtering a parameterized type is same as single filter.
    #[test]
    fn prop_param_identity_double_filter(ts in any_type_string()) {
        let ty: Type = parse_str(&ts).unwrap();
        let s = skip_set(&["Box", "Arc", "Rc"]);
        let once = filter_inner_type(&ty, &s);
        let twice = filter_inner_type(&once, &s);
        prop_assert_eq!(to_str(&once), to_str(&twice));
    }

    /// Extraction is deterministic across repeated calls.
    #[test]
    fn prop_param_identity_extraction_deterministic(
        container in container_name(),
        leaf in leaf_type(),
    ) {
        let ty: Type = parse_str(&format!("{container}<{leaf}>")).unwrap();
        let (r1, ok1) = try_extract_inner_type(&ty, container, &empty_skip());
        let (r2, ok2) = try_extract_inner_type(&ty, container, &empty_skip());
        prop_assert_eq!(ok1, ok2);
        prop_assert_eq!(to_str(&r1), to_str(&r2));
    }
}

// ===========================================================================
// 8. prop_param_edge_* — edge cases (4 tests)
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    /// Wrapping a type that is already the inner of a container still works.
    #[test]
    fn prop_param_edge_wrap_after_extract(
        container in container_name(),
        leaf in leaf_type(),
    ) {
        let ty: Type = parse_str(&format!("{container}<{leaf}>")).unwrap();
        let (inner, ok) = try_extract_inner_type(&ty, container, &empty_skip());
        prop_assert!(ok);
        let wrapped = wrap_leaf_type(&inner, &empty_skip());
        prop_assert_eq!(to_str(&wrapped), format!("adze :: WithLeaf < {leaf} >"));
    }

    /// Filter then wrap: filter removes skip layer, wrap adds WithLeaf.
    #[test]
    fn prop_param_edge_filter_then_wrap(
        skipper in skip_member(),
        leaf in leaf_type(),
    ) {
        let ty: Type = parse_str(&format!("{skipper}<{leaf}>")).unwrap();
        let arr = [skipper];
        let s = skip_set(&arr);
        let filtered = filter_inner_type(&ty, &s);
        let wrapped = wrap_leaf_type(&filtered, &empty_skip());
        prop_assert_eq!(to_str(&wrapped), format!("adze :: WithLeaf < {leaf} >"));
    }


    /// Skip set order does not affect filtering results.
    #[test]
    fn prop_param_edge_filter_skip_order_invariant(leaf in leaf_type()) {
        let ty: Type = parse_str(&format!("Box<Arc<Rc<{leaf}>>>")).unwrap();
        let s1 = skip_set(&["Box", "Arc", "Rc"]);
        let s2 = skip_set(&["Rc", "Box", "Arc"]);
        let f1 = filter_inner_type(&ty, &s1);
        let f2 = filter_inner_type(&ty, &s2);
        prop_assert_eq!(to_str(&f1), to_str(&f2));
        prop_assert_eq!(to_str(&f1), leaf);
    }

    /// Filtering and direct wrapping have intentionally different semantics:
    /// filtering strips skipped containers, while direct wrapping preserves them.
    #[test]
    fn prop_param_edge_filter_then_wrap_differs_from_direct_wrap(
        skipper in skip_member(),
        leaf in leaf_type(),
    ) {
        let ty: Type = parse_str(&format!("{skipper}<Vec<{leaf}>>")).unwrap();
        let arr = [skipper, "Vec"];
        let s = skip_set(&arr);

        let filtered_then_wrapped = wrap_leaf_type(&filter_inner_type(&ty, &s), &empty_skip());
        let direct_wrapped = wrap_leaf_type(&ty, &s);

        let filtered_s = to_str(&filtered_then_wrapped);
        let direct_s = to_str(&direct_wrapped);
        let expected_prefix = format!("{skipper} < Vec <");
        prop_assert_ne!(&filtered_s, &direct_s);
        prop_assert!(filtered_s.starts_with("adze :: WithLeaf <"));
        prop_assert!(direct_s.starts_with(&expected_prefix));
    }
    /// Parameterized detection agrees with angle bracket presence in string.
    #[test]
    fn prop_param_edge_detect_agrees_with_syntax(ts in any_type_string()) {
        let ty: Type = parse_str(&ts).unwrap();
        let param = is_parameterized(&ty);
        let has_angles = has_angle_brackets(&ts);
        prop_assert_eq!(param, has_angles);
    }

    /// Wrapping then filtering back: if wrap adds WithLeaf and we skip WithLeaf,
    /// we should not reach the original (WithLeaf is not in standard skip sets).
    #[test]
    fn prop_param_edge_wrap_is_not_reversible_by_filter(leaf in leaf_type()) {
        let ty: Type = parse_str(leaf).unwrap();
        let wrapped = wrap_leaf_type(&ty, &empty_skip());
        // Standard skip set does not include "WithLeaf"
        let s = skip_set(&["Box", "Arc", "Rc"]);
        let filtered = filter_inner_type(&wrapped, &s);
        // WithLeaf is not stripped, so filtered == wrapped
        prop_assert_eq!(to_str(&filtered), to_str(&wrapped));
    }
}
