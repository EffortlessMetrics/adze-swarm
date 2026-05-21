use std::collections::HashSet;

use syn::{GenericArgument, Type, parse_quote};

use super::shared::last_segment_mut;

/// Wrap leaf types in `adze::WithLeaf`.
///
/// Containers listed in `skip_over` keep their outer type and recursively wrap
/// type arguments instead.
pub fn wrap_leaf_type(ty: &Type, skip_over: &HashSet<&str>) -> Type {
    let mut ty = ty.clone();
    let Some(segment) = last_segment_mut(&mut ty) else {
        return parse_quote!(adze::WithLeaf<#ty>);
    };

    if skip_over.contains(segment.ident.to_string().as_str()) {
        return match &mut segment.arguments {
            syn::PathArguments::AngleBracketed(arguments) => {
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

    parse_quote!(adze::WithLeaf<#ty>)
}
