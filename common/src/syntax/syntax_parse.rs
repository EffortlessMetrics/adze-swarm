use syn::parse::Parse;
use syn::{Field, Token, parse::ParseStream, punctuated::Punctuated};

use crate::syntax::NameValueExpr;

pub(crate) fn parse_name_value_expr(input: ParseStream) -> syn::Result<NameValueExpr> {
    Ok(NameValueExpr {
        path: input.parse()?,
        eq_token: input.parse()?,
        expr: input.parse()?,
    })
}

pub(crate) fn parse_field_then_params(
    input: ParseStream,
) -> syn::Result<(
    Field,
    Option<Token![,]>,
    Punctuated<NameValueExpr, Token![,]>,
)> {
    let field = Field::parse_unnamed(input)?;
    let comma: Option<Token![,]> = input.parse()?;
    let params = if comma.is_some() {
        Punctuated::parse_terminated_with(input, NameValueExpr::parse)?
    } else {
        Punctuated::new()
    };

    Ok((field, comma, params))
}
