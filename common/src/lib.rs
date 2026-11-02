// Common crate is pure-Rust - no unsafe needed
#![forbid(unsafe_code)]
#![cfg_attr(feature = "strict_docs", deny(missing_docs))]
#![cfg_attr(not(feature = "strict_docs"), allow(missing_docs))]

//! Shared utility entrypoint for macro and tool parsing behavior.

pub mod syntax;

pub use syntax::*;

/// Canonical public token-kind name for an inline regex leaf.
///
/// Generation and typed extraction share this name so dispatch follows the
/// selected native token identity rather than re-evaluating the regex.
pub fn pattern_token_name(pattern: &str) -> String {
    format!("/{pattern}/")
}

/// Whether a variant explicitly requires its named intermediate rule.
///
/// ADR 0003 retains wrappers for `no_inline` and precedence attributes. Tool
/// lowering and macro extraction share this policy so wrapped leaves cannot
/// fall through to a sibling's inline token dispatch.
pub fn variant_has_explicit_wrapper(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|attr| {
        attr.path() == &syn::parse_quote!(adze::no_inline)
            || attr.path() == &syn::parse_quote!(adze::prec)
            || attr.path() == &syn::parse_quote!(adze::prec_left)
            || attr.path() == &syn::parse_quote!(adze::prec_right)
    })
}
