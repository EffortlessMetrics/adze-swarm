use syn::{GenericArgument, PathArguments, Type, TypePath};

pub(crate) const ARGUMENT_TYPE_ERROR: &str = "argument in angle brackets must be a type";

pub(crate) fn last_type_segment(ty: &Type) -> Option<&syn::PathSegment> {
    let Type::Path(TypePath { path, .. }) = ty else {
        return None;
    };
    path.segments.last()
}

pub(crate) fn is_named_segment(segment: &syn::PathSegment, name: &str) -> bool {
    segment.ident == name
}

pub(crate) fn first_type_argument(segment: &syn::PathSegment) -> Option<Option<Type>> {
    match &segment.arguments {
        PathArguments::AngleBracketed(arguments) => {
            if let Some(argument) = arguments.args.first().cloned() {
                if let GenericArgument::Type(inner) = argument {
                    Some(Some(inner))
                } else {
                    panic!("{ARGUMENT_TYPE_ERROR}")
                }
            } else {
                Some(None)
            }
        }
        _ => None,
    }
}
