use std::collections::HashSet;

use syn::{GenericArgument, PathArguments, Type, parse_quote};

pub(crate) fn wrap_leaf_type(ty: &Type, skip_over: &HashSet<&str>) -> Type {
    let mut ty = ty.clone();
    let Type::Path(path) = &mut ty else {
        return wrap_as_leaf(&ty);
    };

    let Some(type_segment) = path.path.segments.last_mut() else {
        return wrap_as_leaf(&ty);
    };

    if !skip_over.contains(type_segment.ident.to_string().as_str()) {
        return wrap_as_leaf(&ty);
    }

    wrap_within_skipped_container(&mut type_segment.arguments, skip_over);
    ty
}

fn wrap_within_skipped_container(arguments: &mut PathArguments, skip_over: &HashSet<&str>) {
    if let PathArguments::AngleBracketed(arguments) = arguments {
        for argument in arguments.args.iter_mut() {
            if let GenericArgument::Type(inner) = argument {
                *inner = wrap_leaf_type(inner, skip_over);
            }
        }
    }
}

fn wrap_as_leaf(ty: &Type) -> Type {
    parse_quote!(adze::WithLeaf<#ty>)
}
