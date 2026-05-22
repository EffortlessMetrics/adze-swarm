//! Property-based tests (v4) for type wrapping and extraction in adze-common.
//!
//! 46 proptest property tests across 8 categories:
//! 1. prop_wrap_leaf_*   — wrap_leaf_type properties
//! 2. prop_extract_*     — try_extract_inner_type properties
//! 3. prop_filter_*      — filter_inner_type properties
//! 4. prop_param_*       — is_parameterized properties
//! 5. prop_roundtrip_*   — wrap then extract roundtrip
//! 6. prop_identity_*    — identity operations
//! 7. prop_nested_*      — nested type operations
//! 8. prop_edge_*        — edge case properties

use adze_common::{filter_inner_type, try_extract_inner_type, wrap_leaf_type};
use proptest::prelude::*;
use quote::ToTokens;
use std::collections::HashSet;
use syn::{Type, parse_str};

// ---------------------------------------------------------------------------
// Strategies
// ---------------------------------------------------------------------------

/// Valid Rust leaf type names (primitives + common types).
fn leaf_type() -> impl Strategy<Value = &'static str> {
    prop::sample::select(
        &[
            "i8", "i16", "i32", "i64", "u8", "u16", "u32", "u64", "f32", "f64", "bool", "char",
            "String", "usize", "isize",
        ][..],
    )
}

/// Common generic container names.
fn container_name() -> impl Strategy<Value = &'static str> {
    prop::sample::select(&["Option", "Vec", "Box", "Arc", "Rc"][..])
}

/// Custom identifier that avoids reserved keywords and known container names.
fn custom_ident() -> impl Strategy<Value = String> {
    "[A-Z][a-z]{2,6}".prop_filter("avoid keywords and containers", |s| {
        !matches!(
            s.as_str(),
            "Self" | "Box" | "Vec" | "Option" | "Arc" | "Rc" | "Result" | "Gen"
        )
    })
}

/// Depth-0 type string (bare leaf).
fn depth0() -> impl Strategy<Value = String> {
    leaf_type().prop_map(|s| s.to_string())
}

/// Depth-1 type string: Container<Leaf>.
fn depth1() -> impl Strategy<Value = String> {
    (container_name(), leaf_type()).prop_map(|(c, l)| format!("{c}<{l}>"))
}

/// Depth-2 type string: Container<Container<Leaf>>.
fn depth2() -> impl Strategy<Value = String> {
    (container_name(), container_name(), leaf_type())
        .prop_map(|(c1, c2, l)| format!("{c1}<{c2}<{l}>>"))
}

/// Nested type string with depth 0-2.
fn any_type_string() -> impl Strategy<Value = String> {
    prop_oneof![depth0(), depth1(), depth2()]
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

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

fn skip1(a: &str) -> HashSet<&str> {
    [a].into_iter().collect()
}

fn skip2<'a>(a: &'a str, b: &'a str) -> HashSet<&'a str> {
    [a, b].into_iter().collect()
}

fn skip_static(names: &[&'static str]) -> HashSet<&'static str> {
    names.iter().copied().collect()
}

fn empty_skip() -> HashSet<&'static str> {
    HashSet::new()
}

// ---------------------------------------------------------------------------
// 1. prop_wrap_leaf_* — wrap_leaf_type properties (6 tests)
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    /// Wrapping a bare leaf always produces adze::WithLeaf<Leaf>.
    #[test]
    fn prop_wrap_leaf_bare_produces_with_leaf(leaf in leaf_type()) {
        let ty: Type = parse_str(leaf).unwrap();
        let wrapped = wrap_leaf_type(&ty, &empty_skip());
        let s = ty_str(&wrapped);
        prop_assert!(s.contains("adze :: WithLeaf"), "expected WithLeaf wrapper, got: {s}");
        prop_assert!(s.contains(leaf), "expected leaf {leaf} in: {s}");
    }

    /// Wrapping a leaf with empty skip always yields a parameterized type.
    #[test]
    fn prop_wrap_leaf_always_parameterized(leaf in leaf_type()) {
        let ty: Type = parse_str(leaf).unwrap();
        let wrapped = wrap_leaf_type(&ty, &empty_skip());
        prop_assert!(is_parameterized(&wrapped));
    }

    /// Wrapping a container in the skip set preserves the outer container.
    #[test]
    fn prop_wrap_leaf_skip_preserves_container(
        container in container_name(),
        leaf in leaf_type(),
    ) {
        let ty: Type = parse_str(&format!("{container}<{leaf}>")).unwrap();
        let skip = skip1(container);
        let wrapped = wrap_leaf_type(&ty, &skip);
        let s = ty_str(&wrapped);
        prop_assert!(s.starts_with(container), "should start with {container}: {s}");
    }

    /// Wrapping a container in skip set wraps its inner leaf.
    #[test]
    fn prop_wrap_leaf_skip_wraps_inner(
        container in container_name(),
        leaf in leaf_type(),
    ) {
        let ty: Type = parse_str(&format!("{container}<{leaf}>")).unwrap();
        let skip = skip1(container);
        let wrapped = wrap_leaf_type(&ty, &skip);
        let s = ty_str(&wrapped);
        prop_assert!(s.contains("adze :: WithLeaf"), "inner should be wrapped: {s}");
    }

    /// Wrapping a container NOT in skip wraps the entire type.
    #[test]
    fn prop_wrap_leaf_no_skip_wraps_whole(
        container in container_name(),
        leaf in leaf_type(),
    ) {
        let ty: Type = parse_str(&format!("{container}<{leaf}>")).unwrap();
        let wrapped = wrap_leaf_type(&ty, &empty_skip());
        let s = ty_str(&wrapped);
        prop_assert!(s.starts_with("adze :: WithLeaf"), "should wrap whole type: {s}");
    }

    /// Wrapping a custom identifier wraps it entirely.
    #[test]
    fn prop_wrap_leaf_custom_ident(name in custom_ident()) {
        let ty: Type = parse_str(&name).unwrap();
        let wrapped = wrap_leaf_type(&ty, &empty_skip());
        let s = ty_str(&wrapped);
        prop_assert!(s.starts_with("adze :: WithLeaf"), "custom type should be wrapped: {s}");
    }
}

// ---------------------------------------------------------------------------
// 2. prop_extract_* — try_extract_inner_type properties (6 tests)
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    /// Extracting from Container<Leaf> with matching target succeeds.
    #[test]
    fn prop_extract_matching_succeeds(
        container in container_name(),
        leaf in leaf_type(),
    ) {
        let ty: Type = parse_str(&format!("{container}<{leaf}>")).unwrap();
        let (inner, ok) = try_extract_inner_type(&ty, container, &empty_skip());
        prop_assert!(ok);
        prop_assert_eq!(ty_str(&inner), leaf);
    }

    /// Extracting from a bare leaf with any target fails.
    #[test]
    fn prop_extract_bare_leaf_fails(
        leaf in leaf_type(),
        target in container_name(),
    ) {
        let ty: Type = parse_str(leaf).unwrap();
        let (_, ok) = try_extract_inner_type(&ty, target, &empty_skip());
        prop_assert!(!ok);
    }

    /// Extracting with mismatched target returns the original type.
    #[test]
    fn prop_extract_mismatch_returns_original(leaf in leaf_type()) {
        let ty: Type = parse_str(&format!("Vec<{leaf}>")).unwrap();
        let (inner, ok) = try_extract_inner_type(&ty, "Option", &empty_skip());
        prop_assert!(!ok);
        prop_assert_eq!(ty_str(&inner), ty_str(&ty));
    }

    /// Extracting through a skip container succeeds.
    #[test]
    fn prop_extract_through_skip(leaf in leaf_type()) {
        let ty: Type = parse_str(&format!("Box<Vec<{leaf}>>")).unwrap();
        let skip = skip_static(&["Box"]);
        let (inner, ok) = try_extract_inner_type(&ty, "Vec", &skip);
        prop_assert!(ok);
        prop_assert_eq!(ty_str(&inner), leaf);
    }

    /// Extracting with target in skip set but not matching inner fails.
    #[test]
    fn prop_extract_skip_no_inner_match(leaf in leaf_type()) {
        let ty: Type = parse_str(&format!("Box<{leaf}>")).unwrap();
        let skip = skip_static(&["Box"]);
        let (_, ok) = try_extract_inner_type(&ty, "Option", &skip);
        prop_assert!(!ok);
    }

    /// Extraction result for matching target is never parameterized (bare leaf).
    #[test]
    fn prop_extract_result_not_parameterized(
        container in container_name(),
        leaf in leaf_type(),
    ) {
        let ty: Type = parse_str(&format!("{container}<{leaf}>")).unwrap();
        let (inner, ok) = try_extract_inner_type(&ty, container, &empty_skip());
        prop_assert!(ok);
        prop_assert!(!is_parameterized(&inner), "extracted leaf should be bare");
    }
}

// ---------------------------------------------------------------------------
// 3. prop_filter_* — filter_inner_type properties (6 tests)
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    /// Filtering a container in the skip set removes it.
    #[test]
    fn prop_filter_removes_skip_container(
        container in container_name(),
        leaf in leaf_type(),
    ) {
        let ty: Type = parse_str(&format!("{container}<{leaf}>")).unwrap();
        let skip = skip1(container);
        let filtered = filter_inner_type(&ty, &skip);
        prop_assert_eq!(ty_str(&filtered), leaf);
    }

    /// Filtering with empty skip set is identity.
    #[test]
    fn prop_filter_empty_skip_identity(type_str in any_type_string()) {
        let ty: Type = parse_str(&type_str).unwrap();
        let filtered = filter_inner_type(&ty, &empty_skip());
        prop_assert_eq!(ty_str(&filtered), ty_str(&ty));
    }

    /// Filter is idempotent: filtering twice equals filtering once.
    #[test]
    fn prop_filter_idempotent(
        container in container_name(),
        leaf in leaf_type(),
    ) {
        let ty: Type = parse_str(&format!("{container}<{leaf}>")).unwrap();
        let skip = skip1(container);
        let once = filter_inner_type(&ty, &skip);
        let twice = filter_inner_type(&once, &skip);
        prop_assert_eq!(ty_str(&once), ty_str(&twice));
    }

    /// Filtering a bare leaf with any skip set returns the leaf unchanged.
    #[test]
    fn prop_filter_bare_leaf_unchanged(leaf in leaf_type()) {
        let ty: Type = parse_str(leaf).unwrap();
        let skip = skip_static(&["Box", "Arc", "Rc"]);
        let filtered = filter_inner_type(&ty, &skip);
        prop_assert_eq!(ty_str(&filtered), leaf);
    }

    /// Filtering nested Box<Arc<Leaf>> with both in skip yields leaf.
    #[test]
    fn prop_filter_double_unwrap(leaf in leaf_type()) {
        let ty: Type = parse_str(&format!("Box<Arc<{leaf}>>")).unwrap();
        let skip = skip_static(&["Box", "Arc"]);
        let filtered = filter_inner_type(&ty, &skip);
        prop_assert_eq!(ty_str(&filtered), leaf);
    }

    /// Filtering preserves a non-skip container.
    #[test]
    fn prop_filter_preserves_non_skip(leaf in leaf_type()) {
        let ty: Type = parse_str(&format!("Vec<{leaf}>")).unwrap();
        let skip = skip_static(&["Box", "Arc"]);
        let filtered = filter_inner_type(&ty, &skip);
        prop_assert_eq!(ty_str(&filtered), format!("Vec < {leaf} >"));
    }
}

// ---------------------------------------------------------------------------
// 4. prop_param_* — is_parameterized properties (6 tests)
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    /// A bare leaf is never parameterized.
    #[test]
    fn prop_param_bare_leaf_false(leaf in leaf_type()) {
        let ty: Type = parse_str(leaf).unwrap();
        prop_assert!(!is_parameterized(&ty));
    }

    /// A Container<Leaf> is always parameterized.
    #[test]
    fn prop_param_container_leaf_true(
        container in container_name(),
        leaf in leaf_type(),
    ) {
        let ty: Type = parse_str(&format!("{container}<{leaf}>")).unwrap();
        prop_assert!(is_parameterized(&ty));
    }

    /// A nested Container<Container<Leaf>> is parameterized.
    #[test]
    fn prop_param_nested_true(
        c1 in container_name(),
        c2 in container_name(),
        leaf in leaf_type(),
    ) {
        let ty: Type = parse_str(&format!("{c1}<{c2}<{leaf}>>")).unwrap();
        prop_assert!(is_parameterized(&ty));
    }

    /// A custom identifier without generics is not parameterized.
    #[test]
    fn prop_param_custom_ident_false(name in custom_ident()) {
        let ty: Type = parse_str(&name).unwrap();
        prop_assert!(!is_parameterized(&ty));
    }

    /// Wrapping a leaf always produces a parameterized result.
    #[test]
    fn prop_param_wrap_always_true(leaf in leaf_type()) {
        let ty: Type = parse_str(leaf).unwrap();
        let wrapped = wrap_leaf_type(&ty, &empty_skip());
        prop_assert!(is_parameterized(&wrapped));
    }

    /// Filtering a container in skip produces a non-parameterized result.
    #[test]
    fn prop_param_filter_leaf_result(
        container in container_name(),
        leaf in leaf_type(),
    ) {
        let ty: Type = parse_str(&format!("{container}<{leaf}>")).unwrap();
        let skip = skip1(container);
        let filtered = filter_inner_type(&ty, &skip);
        prop_assert!(!is_parameterized(&filtered), "filtered leaf should not be parameterized");
    }
}

// ---------------------------------------------------------------------------
// 5. prop_roundtrip_* — wrap then extract roundtrip (6 tests)
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    /// Wrapping a leaf then extracting WithLeaf yields the original leaf.
    #[test]
    fn prop_roundtrip_wrap_extract_leaf(leaf in leaf_type()) {
        let ty: Type = parse_str(leaf).unwrap();
        let wrapped = wrap_leaf_type(&ty, &empty_skip());
        // WithLeaf is the outer, extracting it should yield original
        let (inner, ok) = try_extract_inner_type(&wrapped, "WithLeaf", &empty_skip());
        prop_assert!(ok);
        prop_assert_eq!(ty_str(&inner), leaf);
    }

    /// Wrapping Container<Leaf> (container in skip) then extracting WithLeaf
    /// from the inner type produces the original leaf.
    #[test]
    fn prop_roundtrip_wrap_skip_extract(
        container in container_name(),
        leaf in leaf_type(),
    ) {
        let ty: Type = parse_str(&format!("{container}<{leaf}>")).unwrap();
        let skip = skip1(container);
        let wrapped = wrap_leaf_type(&ty, &skip);
        // The inner should be Container<WithLeaf<Leaf>>, extract Container first
        let (inner_of_container, ok) = try_extract_inner_type(&wrapped, container, &empty_skip());
        prop_assert!(ok);
        // inner_of_container = WithLeaf<Leaf>
        let (final_inner, ok2) = try_extract_inner_type(&inner_of_container, "WithLeaf", &empty_skip());
        prop_assert!(ok2);
        prop_assert_eq!(ty_str(&final_inner), leaf);
    }

    /// Extract then wrap: extracting then wrapping changes the type.
    #[test]
    fn prop_roundtrip_extract_then_wrap(
        container in container_name(),
        leaf in leaf_type(),
    ) {
        let ty: Type = parse_str(&format!("{container}<{leaf}>")).unwrap();
        let (inner, ok) = try_extract_inner_type(&ty, container, &empty_skip());
        prop_assert!(ok);
        let wrapped = wrap_leaf_type(&inner, &empty_skip());
        let s = ty_str(&wrapped);
        prop_assert!(s.contains("adze :: WithLeaf"), "re-wrapped should contain WithLeaf: {s}");
    }

    /// Filter then wrap: filtering and wrapping a bare leaf.
    #[test]
    fn prop_roundtrip_filter_then_wrap(
        container in container_name(),
        leaf in leaf_type(),
    ) {
        let ty: Type = parse_str(&format!("{container}<{leaf}>")).unwrap();
        let skip = skip1(container);
        let filtered = filter_inner_type(&ty, &skip);
        let wrapped = wrap_leaf_type(&filtered, &empty_skip());
        let s = ty_str(&wrapped);
        prop_assert!(s.contains("adze :: WithLeaf"), "should be wrapped: {s}");
        prop_assert!(s.contains(leaf), "should contain leaf: {s}");
    }

    /// Wrap then filter with WithLeaf not in skip is identity on wrapped.
    #[test]
    fn prop_roundtrip_wrap_then_filter_noop(leaf in leaf_type()) {
        let ty: Type = parse_str(leaf).unwrap();
        let wrapped = wrap_leaf_type(&ty, &empty_skip());
        let filtered = filter_inner_type(&wrapped, &empty_skip());
        prop_assert_eq!(ty_str(&wrapped), ty_str(&filtered));
    }

    /// Double wrapping: wrapping twice nests two WithLeaf layers.
    #[test]
    fn prop_roundtrip_double_wrap(leaf in leaf_type()) {
        let ty: Type = parse_str(leaf).unwrap();
        let once = wrap_leaf_type(&ty, &empty_skip());
        let twice = wrap_leaf_type(&once, &empty_skip());
        let s = ty_str(&twice);
        // Should have two occurrences of WithLeaf
        let count = s.matches("WithLeaf").count();
        prop_assert!(count >= 2, "double wrap should have >= 2 WithLeaf: {s}");
    }
}

// ---------------------------------------------------------------------------
// 6. prop_identity_* — identity operations (6 tests)
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    /// Extraction with non-matching target returns the original type.
    #[test]
    fn prop_identity_extract_no_match(type_str in any_type_string()) {
        let ty: Type = parse_str(&type_str).unwrap();
        let (result, ok) = try_extract_inner_type(&ty, "NonExistentType", &empty_skip());
        prop_assert!(!ok);
        prop_assert_eq!(ty_str(&result), ty_str(&ty));
    }

    /// Filter with empty skip set preserves any type.
    #[test]
    fn prop_identity_filter_empty_skip(type_str in any_type_string()) {
        let ty: Type = parse_str(&type_str).unwrap();
        let filtered = filter_inner_type(&ty, &empty_skip());
        prop_assert_eq!(ty_str(&filtered), ty_str(&ty));
    }

    /// Triple filter equals single filter.
    #[test]
    fn prop_identity_filter_triple(
        container in container_name(),
        leaf in leaf_type(),
    ) {
        let ty: Type = parse_str(&format!("{container}<{leaf}>")).unwrap();
        let skip = skip1(container);
        let once = filter_inner_type(&ty, &skip);
        let triple = filter_inner_type(&filter_inner_type(&once, &skip), &skip);
        prop_assert_eq!(ty_str(&once), ty_str(&triple));
    }

    /// Extracting the same target twice from a non-nested type: second fails.
    #[test]
    fn prop_identity_double_extract_fails(
        container in container_name(),
        leaf in leaf_type(),
    ) {
        let ty: Type = parse_str(&format!("{container}<{leaf}>")).unwrap();
        let (inner, ok1) = try_extract_inner_type(&ty, container, &empty_skip());
        prop_assert!(ok1);
        let (_, ok2) = try_extract_inner_type(&inner, container, &empty_skip());
        prop_assert!(!ok2, "second extract of same container should fail");
    }

    /// Wrap preserves the original type string inside the wrapper.
    #[test]
    fn prop_identity_wrap_preserves_inner(leaf in leaf_type()) {
        let ty: Type = parse_str(leaf).unwrap();
        let wrapped = wrap_leaf_type(&ty, &empty_skip());
        let s = ty_str(&wrapped);
        prop_assert!(s.contains(leaf), "wrapped should contain {leaf}: {s}");
    }

    /// Wrapping a custom ident with no skip produces expected format.
    #[test]
    fn prop_identity_wrap_custom_format(name in custom_ident()) {
        let ty: Type = parse_str(&name).unwrap();
        let wrapped = wrap_leaf_type(&ty, &empty_skip());
        let s = ty_str(&wrapped);
        let expected = format!("adze :: WithLeaf < {name} >");
        prop_assert_eq!(s, expected);
    }
}

// ---------------------------------------------------------------------------
// 7. prop_nested_* — nested type operations (6 tests)
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    /// Wrapping a depth-2 type with both containers in skip wraps the leaf.
    #[test]
    fn prop_nested_wrap_depth2_both_skip(
        c1 in container_name(),
        c2 in container_name(),
        leaf in leaf_type(),
    ) {
        let ty: Type = parse_str(&format!("{c1}<{c2}<{leaf}>>")).unwrap();
        let skip = skip2(c1, c2);
        let wrapped = wrap_leaf_type(&ty, &skip);
        let s = ty_str(&wrapped);
        prop_assert!(s.contains("adze :: WithLeaf"), "inner leaf should be wrapped: {s}");
        prop_assert!(s.starts_with(c1), "outer container preserved: {s}");
    }

    /// Extracting inner through two skip layers.
    #[test]
    fn prop_nested_extract_through_two_skips(leaf in leaf_type()) {
        let ty: Type = parse_str(&format!("Box<Arc<Vec<{leaf}>>>")).unwrap();
        let skip = skip_static(&["Box", "Arc"]);
        let (inner, ok) = try_extract_inner_type(&ty, "Vec", &skip);
        prop_assert!(ok);
        prop_assert_eq!(ty_str(&inner), leaf);
    }

    /// Filtering depth-2 with both containers in skip yields the leaf.
    #[test]
    fn prop_nested_filter_depth2(
        c1 in container_name(),
        c2 in container_name(),
        leaf in leaf_type(),
    ) {
        let ty: Type = parse_str(&format!("{c1}<{c2}<{leaf}>>")).unwrap();
        let skip = skip2(c1, c2);
        let filtered = filter_inner_type(&ty, &skip);
        prop_assert_eq!(ty_str(&filtered), leaf);
    }

    /// Wrapping depth-2 with only outer in skip wraps the inner Container<Leaf>.
    #[test]
    fn prop_nested_wrap_partial_skip(
        leaf in leaf_type(),
    ) {
        // Use Option as outer (in skip), Rc as inner (not in skip)
        let ty: Type = parse_str(&format!("Option<Rc<{leaf}>>")).unwrap();
        let skip = skip1("Option");
        let wrapped = wrap_leaf_type(&ty, &skip);
        let s = ty_str(&wrapped);
        // Rc<Leaf> is not in skip, so it gets wrapped entirely
        prop_assert!(s.contains("adze :: WithLeaf < Rc"), "inner Rc should be wrapped: {s}");
    }

    /// Filtering depth-2 with only outer in skip preserves the inner container.
    #[test]
    fn prop_nested_filter_partial_skip(
        c2 in container_name().prop_filter("must not be Box", |c| *c != "Box"),
        leaf in leaf_type(),
    ) {
        let ty: Type = parse_str(&format!("Box<{c2}<{leaf}>>")).unwrap();
        let skip = skip_static(&["Box"]);
        let filtered = filter_inner_type(&ty, &skip);
        let expected = format!("{c2} < {leaf} >");
        prop_assert_eq!(ty_str(&filtered), expected);
    }

    /// Nested extraction: Container<Container<Leaf>>, extract outer returns inner.
    #[test]
    fn prop_nested_extract_outer(
        outer in container_name(),
        inner_c in container_name(),
        leaf in leaf_type(),
    ) {
        let ty: Type = parse_str(&format!("{outer}<{inner_c}<{leaf}>>")).unwrap();
        let (inner, ok) = try_extract_inner_type(&ty, outer, &empty_skip());
        prop_assert!(ok);
        let expected = format!("{inner_c} < {leaf} >");
        prop_assert_eq!(ty_str(&inner), expected);
    }
}

// ---------------------------------------------------------------------------
// 8. prop_edge_* — edge case properties (4 tests)
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    /// Wrapping, then filtering with empty skip, equals wrapping.
    #[test]
    fn prop_edge_wrap_filter_empty_is_wrap(leaf in leaf_type()) {
        let ty: Type = parse_str(leaf).unwrap();
        let wrapped = wrap_leaf_type(&ty, &empty_skip());
        let filtered = filter_inner_type(&wrapped, &empty_skip());
        prop_assert_eq!(ty_str(&wrapped), ty_str(&filtered));
    }

    /// Extracting from a type string that is a valid depth-0..2 with
    /// non-matching target always fails.
    #[test]
    fn prop_edge_extract_always_fails_wrong_target(type_str in any_type_string()) {
        let ty: Type = parse_str(&type_str).unwrap();
        let (_, ok) = try_extract_inner_type(&ty, "ZzNonExistent", &empty_skip());
        prop_assert!(!ok);
    }

    /// Wrapping any parseable type always produces a non-empty token stream.
    #[test]
    fn prop_edge_wrap_nonempty(type_str in any_type_string()) {
        let ty: Type = parse_str(&type_str).unwrap();
        let wrapped = wrap_leaf_type(&ty, &empty_skip());
        let s = ty_str(&wrapped);
        prop_assert!(!s.is_empty(), "wrapped type should not be empty");
    }

    /// Filter and extract agree on single-container types.
    #[test]
    fn prop_edge_filter_extract_agreement(
        container in container_name(),
        leaf in leaf_type(),
    ) {
        let ty: Type = parse_str(&format!("{container}<{leaf}>")).unwrap();
        let filtered = filter_inner_type(&ty, &skip1(container));
        let (extracted, ok) = try_extract_inner_type(&ty, container, &empty_skip());
        prop_assert!(ok);
        prop_assert_eq!(ty_str(&filtered), ty_str(&extracted));
    }
}
