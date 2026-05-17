//! Shared type-shape helpers for macro and tool syntax handling.
//!
//! The crate owns the narrow transformations that inspect container types and
//! wrap leaf values without depending on a parser/runtime owner.

use std::collections::HashSet;

use syn::{GenericArgument, PathArguments, Type, parse_quote};

/// Extract the first generic argument from `inner_of`, optionally unwrapping
/// containers named in `skip_over` while searching.
///
/// Returns the extracted type plus `true` when a matching container is found.
/// Returns the original type plus `false` when the shape does not match.
///
/// # Panics
///
/// Panics when a matching or skipped angle-bracketed type has a first generic
/// argument that is not a type. This mirrors the existing macro/tool helper
/// contract for unsupported syntactic shapes.
pub fn try_extract_inner_type(
    ty: &Type,
    inner_of: &str,
    skip_over: &HashSet<&str>,
) -> (Type, bool) {
    if let Type::Path(path) = ty {
        let Some(type_segment) = path.path.segments.last() else {
            return (ty.clone(), false);
        };
        if type_segment.ident == inner_of {
            return match &type_segment.arguments {
                PathArguments::AngleBracketed(arguments) => {
                    if let Some(GenericArgument::Type(inner)) = arguments.args.first().cloned() {
                        (inner, true)
                    } else {
                        panic!("argument in angle brackets must be a type")
                    }
                }
                _ => (ty.clone(), false),
            };
        }

        if skip_over.contains(type_segment.ident.to_string().as_str()) {
            return match &type_segment.arguments {
                PathArguments::AngleBracketed(arguments) => {
                    if let Some(GenericArgument::Type(inner)) = arguments.args.first().cloned() {
                        let (inner, extracted) =
                            try_extract_inner_type(&inner, inner_of, skip_over);
                        if extracted {
                            (inner, true)
                        } else {
                            (ty.clone(), false)
                        }
                    } else {
                        panic!("argument in angle brackets must be a type")
                    }
                }
                _ => (ty.clone(), false),
            };
        }
    }

    (ty.clone(), false)
}

/// Remove container wrappers named in `skip_over` from the outer edge of `ty`.
///
/// The function recursively unwraps matching path types and returns `ty`
/// unchanged when the outer path segment is not listed in `skip_over`.
///
/// # Panics
///
/// Panics when a skipped angle-bracketed type has a first generic argument that
/// is not a type.
pub fn filter_inner_type(ty: &Type, skip_over: &HashSet<&str>) -> Type {
    if let Type::Path(path) = ty {
        let Some(type_segment) = path.path.segments.last() else {
            return ty.clone();
        };
        if skip_over.contains(type_segment.ident.to_string().as_str()) {
            return match &type_segment.arguments {
                PathArguments::AngleBracketed(arguments) => {
                    if let Some(GenericArgument::Type(inner)) = arguments.args.first().cloned() {
                        filter_inner_type(&inner, skip_over)
                    } else {
                        panic!("argument in angle brackets must be a type")
                    }
                }
                _ => ty.clone(),
            };
        }
    }

    ty.clone()
}

/// Wrap leaf types in `adze::WithLeaf`.
///
/// Containers listed in `skip_over` keep their outer type and recursively wrap
/// type arguments instead.
pub fn wrap_leaf_type(ty: &Type, skip_over: &HashSet<&str>) -> Type {
    let mut ty = ty.clone();
    if let Type::Path(path) = &mut ty {
        let Some(type_segment) = path.path.segments.last_mut() else {
            return parse_quote!(adze::WithLeaf<#ty>);
        };
        if skip_over.contains(type_segment.ident.to_string().as_str()) {
            return match &mut type_segment.arguments {
                PathArguments::AngleBracketed(arguments) => {
                    for argument in arguments.args.iter_mut() {
                        if let GenericArgument::Type(inner) = argument {
                            *inner = wrap_leaf_type(inner, skip_over);
                        }
                    }
                    ty
                }
                _ => ty,
            };
        }
    }

    parse_quote!(adze::WithLeaf<#ty>)
}

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
    fn wrap_leaf_type_wraps_every_type_argument_in_skipped_container() {
        let ty: Type = parse_quote!(Result<String, Vec<u8>>);

        let out = wrap_leaf_type(&ty, &skip_set(&["Result", "Vec"]));

        assert_eq!(
            type_to_string(&out),
            "Result < adze :: WithLeaf < String > , Vec < adze :: WithLeaf < u8 > > >"
        );
    }

    #[test]
    fn wrap_leaf_type_preserves_lifetime_arguments_in_skipped_container() {
        let ty: Type = parse_quote!(Cow<'a, str>);

        let out = wrap_leaf_type(&ty, &skip_set(&["Cow"]));

        assert_eq!(
            type_to_string(&out),
            "Cow < 'a , adze :: WithLeaf < str > >"
        );
    }

    #[test]
    fn wrap_leaf_type_recurses_through_qualified_skipped_container() {
        let ty: Type = parse_quote!(std::vec::Vec<std::option::Option<u16>>);

        let out = wrap_leaf_type(&ty, &skip_set(&["Vec", "Option"]));

        assert_eq!(
            type_to_string(&out),
            "std :: vec :: Vec < std :: option :: Option < adze :: WithLeaf < u16 > > >"
        );
    }
}
