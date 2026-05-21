use std::collections::HashSet;

use syn::{GenericArgument, PathArguments, Type};

pub(crate) fn try_extract_inner_type(
    ty: &Type,
    inner_of: &str,
    skip_over: &HashSet<&str>,
) -> (Type, bool) {
    let Type::Path(path) = ty else {
        return (ty.clone(), false);
    };

    let Some(type_segment) = path.path.segments.last() else {
        return (ty.clone(), false);
    };

    if type_segment.ident == inner_of {
        return extract_first_type_argument(type_segment.arguments.clone())
            .map_or_else(|| (ty.clone(), false), |inner| (inner, true));
    }

    if skip_over.contains(type_segment.ident.to_string().as_str()) {
        return extract_first_type_argument(type_segment.arguments.clone()).map_or_else(
            || (ty.clone(), false),
            |inner| recurse_or_original(inner, ty, inner_of, skip_over),
        );
    }

    (ty.clone(), false)
}

fn recurse_or_original(
    inner: Type,
    original: &Type,
    inner_of: &str,
    skip_over: &HashSet<&str>,
) -> (Type, bool) {
    let (inner, extracted) = try_extract_inner_type(&inner, inner_of, skip_over);
    if extracted {
        (inner, true)
    } else {
        (original.clone(), false)
    }
}

fn extract_first_type_argument(arguments: PathArguments) -> Option<Type> {
    match arguments {
        PathArguments::AngleBracketed(arguments) => {
            if let Some(GenericArgument::Type(inner)) = arguments.args.first().cloned() {
                Some(inner)
            } else {
                panic!("argument in angle brackets must be a type")
            }
        }
        _ => None,
    }
}
