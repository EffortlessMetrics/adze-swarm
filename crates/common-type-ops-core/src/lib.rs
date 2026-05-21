//! Shared type-shape helpers for macro and tool syntax handling.
//!
//! The crate owns the narrow transformations that inspect container types and
//! wrap leaf values without depending on a parser/runtime owner.

mod srp;

pub use srp::extract::try_extract_inner_type;
pub use srp::filter::filter_inner_type;
pub use srp::wrap::wrap_leaf_type;

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use syn::{Type, parse_quote};

    fn skip_set(items: &[&'static str]) -> HashSet<&'static str> {
        items.iter().copied().collect()
    }

    fn type_to_string(ty: &Type) -> String {
        quote::ToTokens::to_token_stream(ty).to_string()
    }

    #[test]
    fn try_extract_inner_type_matches_last_segment_of_qualified_path() {
        let ty: Type = parse_quote!(std::vec::Vec<u32>);

        let (inner, extracted) = try_extract_inner_type(&ty, "Vec", &skip_set(&[]));

        assert!(extracted);
        assert_eq!(type_to_string(&inner), "u32");
    }

    #[test]
    fn try_extract_inner_type_extracts_first_type_argument_only() {
        let ty: Type = parse_quote!(std::collections::HashMap<String, usize>);

        let (inner, extracted) = try_extract_inner_type(&ty, "HashMap", &skip_set(&[]));

        assert!(extracted);
        assert_eq!(type_to_string(&inner), "String");
    }

    #[test]
    fn try_extract_inner_type_does_not_search_through_non_skipped_container() {
        let ty: Type = parse_quote!(Box<Result<Vec<u8>, Error>>);

        let (out, extracted) = try_extract_inner_type(&ty, "Vec", &skip_set(&["Box"]));

        assert!(!extracted);
        assert_eq!(
            type_to_string(&out),
            "Box < Result < Vec < u8 > , Error > >"
        );
    }

    #[test]
    fn try_extract_inner_type_finds_target_through_qualified_skipped_wrapper() {
        let ty: Type = parse_quote!(std::sync::Arc<std::boxed::Box<Vec<i64>>>);

        let (inner, extracted) = try_extract_inner_type(&ty, "Vec", &skip_set(&["Arc", "Box"]));

        assert!(extracted);
        assert_eq!(type_to_string(&inner), "i64");
    }

    #[test]
    fn try_extract_inner_type_returns_original_when_target_has_no_angle_arguments() {
        let ty: Type = parse_quote!(Vec);

        let (out, extracted) = try_extract_inner_type(&ty, "Vec", &skip_set(&[]));

        assert!(!extracted);
        assert_eq!(type_to_string(&out), "Vec");
    }

    #[test]
    #[should_panic(expected = "argument in angle brackets must be a type")]
    fn try_extract_inner_type_panics_when_matching_container_first_arg_is_not_type() {
        let ty: Type = parse_quote!(Vec<'a>);

        let _ = try_extract_inner_type(&ty, "Vec", &skip_set(&[]));
    }

    #[test]
    #[should_panic(expected = "argument in angle brackets must be a type")]
    fn try_extract_inner_type_panics_when_skipped_container_first_arg_is_not_type() {
        let ty: Type = parse_quote!(Box<'a>);

        let _ = try_extract_inner_type(&ty, "Vec", &skip_set(&["Box"]));
    }

    #[test]
    fn filter_inner_type_removes_only_contiguous_skipped_outer_wrappers() {
        let ty: Type = parse_quote!(Arc<Result<Box<u8>, Error>>);

        let out = filter_inner_type(&ty, &skip_set(&["Arc", "Box"]));

        assert_eq!(type_to_string(&out), "Result < Box < u8 > , Error >");
    }

    #[test]
    fn filter_inner_type_leaves_non_path_types_unchanged() {
        let ty: Type = parse_quote!((String, u8));

        let out = filter_inner_type(&ty, &skip_set(&["Arc", "Box"]));

        assert_eq!(type_to_string(&out), "(String , u8)");
    }

    #[test]
    fn filter_inner_type_handles_qualified_skipped_wrappers() {
        let ty: Type = parse_quote!(std::sync::Arc<std::boxed::Box<String>>);

        let out = filter_inner_type(&ty, &skip_set(&["Arc", "Box"]));

        assert_eq!(type_to_string(&out), "String");
    }

    #[test]
    #[should_panic(expected = "argument in angle brackets must be a type")]
    fn filter_inner_type_panics_when_skipped_container_first_arg_is_not_type() {
        let ty: Type = parse_quote!(Box<'a>);

        let _ = filter_inner_type(&ty, &skip_set(&["Box"]));
    }

    #[test]
    fn wrap_leaf_type_wraps_plain_type() {
        let ty: Type = parse_quote!(String);

        let out = wrap_leaf_type(&ty, &skip_set(&["Option", "Vec"]));

        assert_eq!(type_to_string(&out), "adze :: WithLeaf < String >");
    }

    #[test]
    fn wrap_leaf_type_wraps_nested_inside_skipped_containers() {
        let ty: Type = parse_quote!(Option<Vec<u8>>);

        let out = wrap_leaf_type(&ty, &skip_set(&["Option", "Vec"]));

        assert_eq!(
            type_to_string(&out),
            "Option < Vec < adze :: WithLeaf < u8 > > >"
        );
    }

    #[test]
    fn wrap_leaf_type_supports_qualified_skipped_containers() {
        let ty: Type = parse_quote!(std::option::Option<std::vec::Vec<String>>);

        let out = wrap_leaf_type(&ty, &skip_set(&["Option", "Vec"]));

        assert_eq!(
            type_to_string(&out),
            "std :: option :: Option < std :: vec :: Vec < adze :: WithLeaf < String > > >"
        );
    }

    #[test]
    fn wrap_leaf_type_leaves_non_skipped_outer_container_and_wraps_whole_type() {
        let ty: Type = parse_quote!(Result<u8, Error>);

        let out = wrap_leaf_type(&ty, &skip_set(&["Option", "Vec"]));

        assert_eq!(
            type_to_string(&out),
            "adze :: WithLeaf < Result < u8 , Error > >"
        );
    }
}
