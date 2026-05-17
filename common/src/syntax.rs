//! Shared syntax helpers for parsing macro/tool attributes.

pub use adze_common_type_ops_core::{filter_inner_type, try_extract_inner_type, wrap_leaf_type};

use syn::{
    Expr, Field, Ident, Token,
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
};

/// Name-value expression for attribute parameters.
///
/// Represents a key-value pair in attribute syntax, such as `param = "value"`.
/// This is commonly used when parsing macro or tool attributes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NameValueExpr {
    /// The parameter name.
    pub path: Ident,
    /// The equals token.
    pub eq_token: Token![=],
    /// The parameter value expression.
    pub expr: Expr,
}

impl Parse for NameValueExpr {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        Ok(NameValueExpr {
            path: input.parse()?,
            eq_token: input.parse()?,
            expr: input.parse()?,
        })
    }
}

/// Field declaration followed by optional parameters.
///
/// Represents a struct field declaration optionally followed by a comma and additional
/// named parameters. Used in parsing attribute syntax that includes field definitions with extra metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldThenParams {
    /// The field declaration.
    pub field: Field,
    /// Optional comma separator before params.
    pub comma: Option<Token![,]>,
    /// Additional named parameters.
    pub params: Punctuated<NameValueExpr, Token![,]>,
}

impl Parse for FieldThenParams {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let field = Field::parse_unnamed(input)?;
        let comma: Option<Token![,]> = input.parse()?;
        let params = if comma.is_some() {
            Punctuated::parse_terminated_with(input, NameValueExpr::parse)?
        } else {
            Punctuated::new()
        };

        Ok(FieldThenParams {
            field,
            comma,
            params,
        })
    }
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
    fn test_parse_name_value_expr() {
        let input: NameValueExpr = parse_quote!(key = "value");
        assert_eq!(input.path.to_string(), "key");

        let input: NameValueExpr = parse_quote!(precedence = 5);
        assert_eq!(input.path.to_string(), "precedence");
    }

    #[test]
    fn test_parse_field_then_params() {
        let input: FieldThenParams = parse_quote!(Type);
        assert!(input.comma.is_none());
        assert!(input.params.is_empty());

        let input: FieldThenParams = parse_quote!(Type, name = "test", value = 42);
        assert!(input.comma.is_some());
        assert_eq!(input.params.len(), 2);
        assert_eq!(input.params[0].path.to_string(), "name");
        assert_eq!(input.params[1].path.to_string(), "value");
    }

    #[test]
    fn try_extract_inner_type_extracts_target() {
        let ty: Type = parse_quote!(Vec<u32>);
        let (inner, extracted) = try_extract_inner_type(&ty, "Vec", &skip_set(&[]));
        assert!(extracted);
        assert_eq!(type_to_string(&inner), "u32");
    }

    #[test]
    fn try_extract_inner_type_not_a_match_returns_original() {
        let ty: Type = parse_quote!(Option<String>);
        let (out, extracted) = try_extract_inner_type(&ty, "Vec", &skip_set(&[]));
        assert!(!extracted);
        assert_eq!(type_to_string(&out), "Option < String >");
    }

    #[test]
    fn try_extract_inner_type_non_path_returns_original() {
        let ty: Type = parse_quote!([u8; 4]);
        let (out, extracted) = try_extract_inner_type(&ty, "Vec", &skip_set(&[]));
        assert!(!extracted);
        assert_eq!(type_to_string(&out), "[u8 ; 4]");
    }

    #[test]
    fn try_extract_inner_type_skip_over_unwraps_wrapper() {
        let ty: Type = parse_quote!(Box<Vec<u32>>);
        let (inner, extracted) = try_extract_inner_type(&ty, "Vec", &skip_set(&["Box"]));
        assert!(extracted);
        assert_eq!(type_to_string(&inner), "u32");
    }

    #[test]
    fn try_extract_inner_type_skip_over_with_no_target_returns_original() {
        let ty: Type = parse_quote!(Box<String>);
        let (out, extracted) = try_extract_inner_type(&ty, "Vec", &skip_set(&["Box"]));
        assert!(!extracted);
        assert_eq!(type_to_string(&out), "Box < String >");
    }

    #[test]
    fn try_extract_inner_type_target_without_generics_returns_original() {
        let ty: Type = parse_quote!(Vec);
        let (out, extracted) = try_extract_inner_type(&ty, "Vec", &skip_set(&[]));
        assert!(!extracted);
        assert_eq!(type_to_string(&out), "Vec");
    }

    #[test]
    fn try_extract_inner_type_handles_nested_skip_chain() {
        let ty: Type = parse_quote!(Arc<Box<Vec<i64>>>);
        let (inner, extracted) = try_extract_inner_type(&ty, "Vec", &skip_set(&["Arc", "Box"]));
        assert!(extracted);
        assert_eq!(type_to_string(&inner), "i64");
    }

    #[test]
    fn filter_inner_type_unwraps_single_layer() {
        let ty: Type = parse_quote!(Box<String>);
        let out = filter_inner_type(&ty, &skip_set(&["Box"]));
        assert_eq!(type_to_string(&out), "String");
    }

    #[test]
    fn filter_inner_type_unwraps_nested_layers() {
        let ty: Type = parse_quote!(Arc<Box<u32>>);
        let out = filter_inner_type(&ty, &skip_set(&["Arc", "Box"]));
        assert_eq!(type_to_string(&out), "u32");
    }

    #[test]
    fn filter_inner_type_no_match_returns_original() {
        let ty: Type = parse_quote!(Vec<u32>);
        let out = filter_inner_type(&ty, &skip_set(&["Box"]));
        assert_eq!(type_to_string(&out), "Vec < u32 >");
    }

    #[test]
    fn filter_inner_type_skip_without_generics_returns_original() {
        let ty: Type = parse_quote!(Box);
        let out = filter_inner_type(&ty, &skip_set(&["Box"]));
        assert_eq!(type_to_string(&out), "Box");
    }

    #[test]
    fn filter_inner_type_non_path_returns_original() {
        let ty: Type = parse_quote!([u8; 8]);
        let out = filter_inner_type(&ty, &skip_set(&["Box"]));
        assert_eq!(type_to_string(&out), "[u8 ; 8]");
    }

    #[test]
    fn wrap_leaf_type_wraps_plain_path() {
        let ty: Type = parse_quote!(String);
        let out = wrap_leaf_type(&ty, &skip_set(&["Vec", "Option"]));
        assert_eq!(type_to_string(&out), "adze :: WithLeaf < String >");
    }

    #[test]
    fn wrap_leaf_type_wraps_non_path() {
        let ty: Type = parse_quote!([u8; 4]);
        let out = wrap_leaf_type(&ty, &skip_set(&["Vec"]));
        assert_eq!(type_to_string(&out), "adze :: WithLeaf < [u8 ; 4] >");
    }

    #[test]
    fn wrap_leaf_type_recurses_into_skipped_container() {
        let ty: Type = parse_quote!(Vec<String>);
        let out = wrap_leaf_type(&ty, &skip_set(&["Vec"]));
        assert_eq!(type_to_string(&out), "Vec < adze :: WithLeaf < String > >");
    }

    #[test]
    fn wrap_leaf_type_skipped_container_without_generics_returns_unchanged() {
        let ty: Type = parse_quote!(Vec);
        let out = wrap_leaf_type(&ty, &skip_set(&["Vec"]));
        assert_eq!(type_to_string(&out), "Vec");
    }

    #[test]
    fn wrap_leaf_type_recurses_through_nested_skipped_containers() {
        let ty: Type = parse_quote!(Option<Vec<u32>>);
        let out = wrap_leaf_type(&ty, &skip_set(&["Vec", "Option"]));
        assert_eq!(
            type_to_string(&out),
            "Option < Vec < adze :: WithLeaf < u32 > > >"
        );
    }
}
