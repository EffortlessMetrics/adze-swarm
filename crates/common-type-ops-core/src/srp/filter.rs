use std::collections::HashSet;

use syn::{GenericArgument, PathArguments, Type};

pub(crate) fn filter_inner_type(ty: &Type, skip_over: &HashSet<&str>) -> Type {
    let Type::Path(path) = ty else {
        return ty.clone();
    };

    let Some(type_segment) = path.path.segments.last() else {
        return ty.clone();
    };

    if !skip_over.contains(type_segment.ident.to_string().as_str()) {
        return ty.clone();
    }

    extract_and_recurse_or_original(type_segment.arguments.clone(), ty, skip_over)
}

fn extract_and_recurse_or_original(
    arguments: PathArguments,
    original: &Type,
    skip_over: &HashSet<&str>,
) -> Type {
    match arguments {
        PathArguments::AngleBracketed(arguments) => {
            if let Some(GenericArgument::Type(inner)) = arguments.args.first().cloned() {
                filter_inner_type(&inner, skip_over)
            } else {
                panic!("argument in angle brackets must be a type")
            }
        }
        _ => original.clone(),
    }
}
