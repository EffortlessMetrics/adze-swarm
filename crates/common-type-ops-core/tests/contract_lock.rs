//! Contract lock tests for the public type helper API.

use std::collections::HashSet;

use adze_common_type_ops_core::{filter_inner_type, try_extract_inner_type, wrap_leaf_type};
use quote::ToTokens;
use syn::{Type, parse_quote};

#[test]
fn contract_lock_public_function_signatures_remain_stable() {
    let _extract: fn(&Type, &str, &HashSet<&str>) -> (Type, bool) = try_extract_inner_type;
    let _filter: fn(&Type, &HashSet<&str>) -> Type = filter_inner_type;
    let _wrap: fn(&Type, &HashSet<&str>) -> Type = wrap_leaf_type;
}

#[test]
fn contract_lock_extracts_target_through_supported_wrappers() {
    let skip_over: HashSet<&str> = HashSet::from(["Box", "Arc"]);
    let input: Type = parse_quote!(Box<Arc<Vec<String>>>);

    let (extracted, found) = try_extract_inner_type(&input, "Vec", &skip_over);

    assert!(found);
    assert_eq!(extracted.to_token_stream().to_string(), "String");
}

#[test]
fn contract_lock_filters_only_configured_outer_wrappers() {
    let skip_over: HashSet<&str> = HashSet::from(["Box", "Arc"]);
    let input: Type = parse_quote!(Box<Arc<Option<String>>>);

    let filtered = filter_inner_type(&input, &skip_over);

    assert_eq!(filtered.to_token_stream().to_string(), "Option < String >");
}

#[test]
fn contract_lock_wraps_leafs_inside_configured_containers() {
    let skip_over: HashSet<&str> = HashSet::from(["Vec", "Option"]);
    let input: Type = parse_quote!(Vec<Option<String>>);

    let wrapped = wrap_leaf_type(&input, &skip_over);

    assert_eq!(
        wrapped.to_token_stream().to_string(),
        "Vec < Option < adze :: WithLeaf < String > > >"
    );
}
