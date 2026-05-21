use std::collections::HashSet;

use syn::Type;

use super::shared::{first_generic_type, last_segment};

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
    let Some(segment) = last_segment(ty) else {
        return (ty.clone(), false);
    };

    if segment.ident == inner_of {
        return first_generic_type(&segment.arguments)
            .map_or_else(|| (ty.clone(), false), |inner| (inner, true));
    }

    if skip_over.contains(segment.ident.to_string().as_str()) {
        return first_generic_type(&segment.arguments).map_or_else(
            || (ty.clone(), false),
            |inner| {
                let (inner, extracted) = try_extract_inner_type(&inner, inner_of, skip_over);
                if extracted {
                    (inner, true)
                } else {
                    (ty.clone(), false)
                }
            },
        );
    }

    (ty.clone(), false)
}
