use std::collections::HashSet;

use syn::Type;

use super::shared::{first_generic_type, last_segment};

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
    let Some(segment) = last_segment(ty) else {
        return ty.clone();
    };

    if skip_over.contains(segment.ident.to_string().as_str()) {
        return first_generic_type(&segment.arguments)
            .map_or_else(|| ty.clone(), |inner| filter_inner_type(&inner, skip_over));
    }

    ty.clone()
}
