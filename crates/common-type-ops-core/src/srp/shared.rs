use syn::{GenericArgument, PathArguments, Type, TypePath};

pub(super) fn last_segment(ty: &Type) -> Option<&syn::PathSegment> {
    match ty {
        Type::Path(TypePath { path, .. }) => path.segments.last(),
        _ => None,
    }
}

pub(super) fn last_segment_mut(ty: &mut Type) -> Option<&mut syn::PathSegment> {
    match ty {
        Type::Path(TypePath { path, .. }) => path.segments.last_mut(),
        _ => None,
    }
}

pub(super) fn first_generic_type(arguments: &PathArguments) -> Option<Type> {
    match arguments {
        PathArguments::AngleBracketed(angle) => {
            if let Some(GenericArgument::Type(inner)) = angle.args.first().cloned() {
                Some(inner)
            } else {
                panic!("argument in angle brackets must be a type")
            }
        }
        _ => None,
    }
}
