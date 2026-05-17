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
