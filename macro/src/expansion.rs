//! Grammar macro expansion from annotated Rust types to parser code.

use std::collections::{HashMap, HashSet};

use crate::errors::IteratorExt as _;
use adze_common::*;
use proc_macro2::Span;
use quote::{ToTokens, quote};
use syn::{parse::Parse, punctuated::Punctuated, *};

/// Whether generated extraction should propagate leaf conversion failures.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ExtractMode {
    Infallible,
    Fallible,
}

/// The `transform` and `try_transform` expressions declared on one leaf.
struct LeafTransforms {
    transform: Option<Expr>,
    try_transform: Option<Expr>,
}

fn leaf_transforms(attrs: &[Attribute]) -> Result<LeafTransforms> {
    let leaf_attr = attrs
        .iter()
        .find(|attr| attr.path() == &syn::parse_quote!(adze::leaf));
    let leaf_params = leaf_attr.and_then(|attr| {
        attr.parse_args_with(Punctuated::<NameValueExpr, Token![,]>::parse_terminated)
            .ok()
    });
    let find_param = |name: &str| {
        leaf_params.as_ref().and_then(|params| {
            params
                .iter()
                .find(|param| param.path == name)
                .map(|param| param.expr.clone())
        })
    };
    let transform = find_param("transform");
    let try_transform = find_param("try_transform");
    if transform.is_some() && try_transform.is_some() {
        return Err(syn::Error::new(
            Span::call_site(),
            "cannot specify both `transform` and `try_transform` on the same #[adze::leaf] field",
        ));
    }
    Ok(LeafTransforms {
        transform,
        try_transform,
    })
}

fn field_has_try_transform(field: &Field) -> bool {
    field.attrs.iter().any(|attr| {
        attr.path() == &syn::parse_quote!(adze::leaf)
            && attr
                .parse_args_with(Punctuated::<NameValueExpr, Token![,]>::parse_terminated)
                .is_ok_and(|params| params.iter().any(|param| param.path == "try_transform"))
    })
}

fn grammar_uses_try_transform(contents: &[Item]) -> bool {
    contents.iter().any(|item| match item {
        Item::Enum(item_enum) => item_enum
            .variants
            .iter()
            .any(|variant| variant.fields.iter().any(field_has_try_transform)),
        Item::Struct(item_struct) => item_struct.fields.iter().any(field_has_try_transform),
        _ => false,
    })
}

/// Whether `path` is the `adze::WithLeaf<...>` wrapper introduced by
/// `wrap_leaf_type`.
///
/// Only this adze-resolved wrapper may be rewritten to `adze::WithTryLeaf`
/// for fallible extraction. A user type that merely shares the `WithLeaf`
/// name (bare `WithLeaf`, `my_crate::WithLeaf`, ...) must keep resolving to
/// the user's own type (issue #1086).
fn is_adze_with_leaf(path: &syn::Path) -> bool {
    path.segments.len() == 2
        && path.segments[0].ident == "adze"
        && path.segments[1].ident == "WithLeaf"
}

fn rename_with_leaf_to_try(ty: &mut Type) {
    match ty {
        Type::Path(type_path) => {
            if let Some(qself) = &mut type_path.qself {
                rename_with_leaf_to_try(&mut qself.ty);
            }
            if is_adze_with_leaf(&type_path.path)
                && let Some(last) = type_path.path.segments.last_mut()
            {
                last.ident = Ident::new("WithTryLeaf", last.ident.span());
            }
            for segment in &mut type_path.path.segments {
                match &mut segment.arguments {
                    syn::PathArguments::AngleBracketed(arguments) => {
                        for argument in &mut arguments.args {
                            if let GenericArgument::Type(inner) = argument {
                                rename_with_leaf_to_try(inner);
                            }
                        }
                    }
                    syn::PathArguments::Parenthesized(arguments) => {
                        for input in &mut arguments.inputs {
                            rename_with_leaf_to_try(input);
                        }
                        if let syn::ReturnType::Type(_, output) = &mut arguments.output {
                            rename_with_leaf_to_try(output);
                        }
                    }
                    syn::PathArguments::None => {}
                }
            }
        }
        Type::Array(inner) => rename_with_leaf_to_try(&mut inner.elem),
        Type::BareFn(inner) => {
            for input in &mut inner.inputs {
                rename_with_leaf_to_try(&mut input.ty);
            }
            if let syn::ReturnType::Type(_, output) = &mut inner.output {
                rename_with_leaf_to_try(output);
            }
        }
        Type::Group(inner) => rename_with_leaf_to_try(&mut inner.elem),
        Type::Paren(inner) => rename_with_leaf_to_try(&mut inner.elem),
        Type::Ptr(inner) => rename_with_leaf_to_try(&mut inner.elem),
        Type::Reference(inner) => rename_with_leaf_to_try(&mut inner.elem),
        Type::Slice(inner) => rename_with_leaf_to_try(&mut inner.elem),
        Type::Tuple(inner) => {
            for elem in &mut inner.elems {
                rename_with_leaf_to_try(elem);
            }
        }
        _ => {}
    }
}

fn wrap_try_leaf_type(ty: &Type, skip_over: &HashSet<&str>) -> Type {
    let mut wrapped = wrap_leaf_type(ty, skip_over);
    rename_with_leaf_to_try(&mut wrapped);
    wrapped
}

fn fallible_closure_expr(closure: Expr) -> Expr {
    // Give an untyped single closure parameter the `&str` leaf-text type so
    // method calls on it (for example `text.parse::<i32>()`) type-check
    // through the generated fallible wrapper.
    let closure = match closure {
        Expr::Closure(mut closure_expr) if closure_expr.inputs.len() == 1 => {
            for input in &mut closure_expr.inputs {
                if let Pat::Ident(pat_ident) = input {
                    let pat = Pat::Ident(pat_ident.clone());
                    *input = Pat::Type(syn::PatType {
                        attrs: Vec::new(),
                        pat: Box::new(pat),
                        colon_token: Token![:](Span::call_site()),
                        ty: Box::new(syn::parse_quote!(&str)),
                    });
                }
            }
            Expr::Closure(closure_expr)
        }
        other => other,
    };
    // Bindings introduced by the macro use `mixed_site` spans, so user code
    // at the call site can neither capture nor shadow them (and vice versa):
    // user identifiers resolve as if the generated bindings were invisible,
    // while the generated uses still resolve to the generated bindings,
    // exactly like `macro_rules!` locals (issue #1086). Interpolating the
    // `Ident`s preserves their spans; the surrounding tokens keep call-site
    // spans, which is irrelevant for name resolution.
    let hygiene = Span::mixed_site();
    let text_ident = Ident::new("__adze_text", hygiene);
    let leaf_ident = Ident::new("__adze_try_leaf", hygiene);
    let error_ident = Ident::new("__adze_error", hygiene);
    syn::parse_quote!(Some(&|#text_ident: &str| -> Result<_, String> {
        let #leaf_ident = #closure;
        #leaf_ident(#text_ident).map_err(|#error_ident| #error_ident.to_string())
    }))
}

fn is_sitter_attr(attr: &Attribute) -> bool {
    attr.path()
        .segments
        .iter()
        .next()
        .map(|segment| segment.ident == "adze")
        .unwrap_or(false)
}

/// Distinguishes between function parameters and struct fields during expansion.
pub enum ParamOrField {
    Param(Expr),
    Field(FieldValue),
}

impl ToTokens for ParamOrField {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        match self {
            ParamOrField::Param(expr) => expr.to_tokens(tokens),
            ParamOrField::Field(field) => field.to_tokens(tokens),
        }
    }
}

fn gen_field(ident_str: String, leaf: Field, mode: ExtractMode) -> Result<Expr> {
    let leaf_type = leaf.ty;
    let transforms = leaf_transforms(&leaf.attrs)?;

    let mut non_leaf = HashSet::new();
    non_leaf.insert("Spanned");
    non_leaf.insert("Box");
    non_leaf.insert("Option");
    non_leaf.insert("Vec");

    let (leaf_type, closure_expr): (Type, Expr) = match transforms {
        LeafTransforms {
            try_transform: Some(closure),
            ..
        } => (
            wrap_try_leaf_type(&leaf_type, &non_leaf),
            fallible_closure_expr(closure),
        ),
        LeafTransforms {
            transform: Some(closure),
            ..
        } => (
            wrap_leaf_type(&leaf_type, &non_leaf),
            syn::parse_quote!(Some(&#closure)),
        ),
        LeafTransforms {
            transform: None,
            try_transform: None,
        } => (leaf_type, syn::parse_quote!(None)),
    };

    Ok(match mode {
        ExtractMode::Infallible => syn::parse_quote!({
            ::adze::__private::extract_field::<#leaf_type,_>(cursor, source, last_idx, #ident_str, #closure_expr)
        }),
        ExtractMode::Fallible => syn::parse_quote!({
            ::adze::__private::extract_field_try::<#leaf_type,_>(cursor, source, last_idx, #ident_str, #closure_expr)?
        }),
    })
}

fn gen_struct_or_variant(
    fields: Fields,
    variant_ident: Option<Ident>,
    containing_type: Ident,
    container_attrs: Vec<Attribute>,
    mode: ExtractMode,
) -> Result<Expr> {
    // Special handling for single-field enum variants that might be leaf nodes
    if let (Some(variant_name), Fields::Unnamed(unnamed_fields)) = (&variant_ident, &fields)
        && unnamed_fields.unnamed.len() == 1
    {
        let field = &unnamed_fields.unnamed[0];
        // Check if this field has a leaf attribute
        let is_leaf = field
            .attrs
            .iter()
            .any(|attr| attr.path() == &syn::parse_quote!(adze::leaf));
        if is_leaf {
            // For leaf variants, extract directly from the node without navigating to children
            let leaf_type = &field.ty;
            let transforms = leaf_transforms(&field.attrs)?;

            let mut non_leaf = HashSet::new();
            non_leaf.insert("Spanned");
            non_leaf.insert("Box");
            non_leaf.insert("Option");
            non_leaf.insert("Vec");

            let (leaf_type, closure_expr): (Type, Expr) = match transforms {
                LeafTransforms {
                    try_transform: Some(closure),
                    ..
                } => (
                    wrap_try_leaf_type(leaf_type, &non_leaf),
                    fallible_closure_expr(closure),
                ),
                LeafTransforms {
                    transform: Some(closure),
                    ..
                } => (
                    wrap_leaf_type(leaf_type, &non_leaf),
                    syn::parse_quote!(Some(&#closure)),
                ),
                LeafTransforms {
                    transform: None,
                    try_transform: None,
                } => (leaf_type.clone(), syn::parse_quote!(None)),
            };

            let construct_name = quote! {
                #containing_type::#variant_name
            };

            // In both modes, 'node' should be passed appropriately.
            // For tree-sitter, it's Node by value. For pure-rust, it's &ParsedNode.
            // We use 'Some(node)' because gen_struct_or_variant is called inside extract()
            // where 'node' is already un-Optioned.
            return Ok(match mode {
                ExtractMode::Infallible => syn::parse_quote!({
                    let value = <#leaf_type as ::adze::Extract<_>>::extract(Some(node), source, 0, #closure_expr);
                    #construct_name(value)
                }),
                ExtractMode::Fallible => syn::parse_quote!({
                    let value = <#leaf_type as ::adze::Extract<_>>::try_extract(Some(node), source, 0, #closure_expr)?;
                    ::core::result::Result::Ok(#construct_name(value))
                }),
            });
        }
    }

    let children_parsed = if fields == Fields::Unit {
        let expr = {
            let dummy_field = Field {
                attrs: container_attrs,
                vis: Visibility::Inherited,
                mutability: FieldMutability::None,
                ident: None,
                colon_token: None,
                ty: Type::Verbatim(quote!(())), // unit type.
            };

            gen_field("unit".to_string(), dummy_field, mode)?
        };
        vec![ParamOrField::Param(expr)]
    } else {
        fields
            .iter()
            .enumerate()
            .map(|(i, field)| {
                let expr = if let Some(skip_attrs) = field
                    .attrs
                    .iter()
                    .find(|attr| attr.path() == &syn::parse_quote!(adze::skip))
                {
                    skip_attrs.parse_args::<syn::Expr>()?
                } else {
                    let ident_str = field
                        .ident
                        .as_ref()
                        .map(|v| v.to_string())
                        .unwrap_or(format!("{i}"));

                    gen_field(ident_str, field.clone(), mode)?
                };

                let field = if let Some(field_name) = &field.ident {
                    ParamOrField::Field(FieldValue {
                        attrs: vec![],
                        member: Member::Named(field_name.clone()),
                        colon_token: Some(Token![:](Span::call_site())),
                        expr,
                    })
                } else {
                    ParamOrField::Param(expr)
                };
                Ok(field)
            })
            .sift::<Vec<ParamOrField>>()?
    };

    let construct_name = match variant_ident {
        Some(ident) => quote! {
            #containing_type::#ident
        },
        None => quote! {
            #containing_type
        },
    };

    let construct_expr = {
        match &fields {
            Fields::Unit => {
                let ParamOrField::Param(ref expr) = children_parsed[0] else {
                    unreachable!()
                };

                quote! {
                    {
                        #expr;
                        #construct_name
                    }
                }
            }
            Fields::Named(_) => quote! {
                #construct_name {
                    #(#children_parsed),*
                }
            },
            Fields::Unnamed(_) => quote! {
                #construct_name(
                    #(#children_parsed),*
                )
            },
        }
    };

    Ok(match mode {
        ExtractMode::Infallible => syn::parse_quote!(
            ::adze::__private::extract_struct_or_variant(node, move |cursor, last_idx| #construct_expr)
        ),
        ExtractMode::Fallible => syn::parse_quote!(
            ::adze::__private::extract_struct_or_variant_try(node, move |cursor, last_idx| Ok(#construct_expr))
        ),
    })
}

/// Expands an annotated Rust module into grammar definitions and parser code.
pub fn expand_grammar(input: ItemMod) -> Result<ItemMod> {
    let grammar_name_str = input
        .attrs
        .iter()
        .find_map(|a| {
            if a.path() == &syn::parse_quote!(adze::grammar) {
                let grammar_name_expr = a.parse_args_with(Expr::parse).ok();
                if let Some(Expr::Lit(ExprLit {
                    attrs: _,
                    lit: Lit::Str(s),
                })) = grammar_name_expr
                {
                    Some(Ok(s.value()))
                } else {
                    Some(Err(syn::Error::new(
                        Span::call_site(),
                        "Expected a string literal grammar name",
                    )))
                }
            } else {
                None
            }
        })
        .transpose()?
        .ok_or_else(|| syn::Error::new(Span::call_site(), "Each grammar must have a name"))?;

    let (brace, new_contents) = input.content.ok_or_else(|| {
        syn::Error::new(
            Span::call_site(),
            "Expected the module to have inline contents (`mod my_module { .. }` syntax)",
        )
    })?;

    let root_type = new_contents
        .iter()
        .find_map(|item| match item {
            Item::Enum(ItemEnum { ident, attrs, .. })
            | Item::Struct(ItemStruct { ident, attrs, .. }) => {
                if attrs
                    .iter()
                    .any(|attr| attr.path() == &syn::parse_quote!(adze::language))
                {
                    Some(ident.clone())
                } else {
                    None
                }
            }
            _ => None,
        })
        .ok_or_else(|| {
            syn::Error::new(
                Span::call_site(),
                "Each parser must have the root type annotated with `#[adze::language]`",
            )
        })?;

    let extraction_mode = if grammar_uses_try_transform(&new_contents) {
        ExtractMode::Fallible
    } else {
        ExtractMode::Infallible
    };

    let mut transformed: Vec<Item> = new_contents
        .iter()
        .cloned()
        .map(|c| match c {
            Item::Enum(mut e) => {
                    let enum_name = &e.ident;
                    let mut variant_arity_counts = HashMap::<usize, usize>::new();
                    for variant in &e.variants {
                        *variant_arity_counts
                            .entry(variant.fields.iter().count())
                            .or_insert(0) += 1;
                    }

                    let variant_detection_logic = e.variants.iter().map(|v| {
                        let extract_expr = gen_struct_or_variant(
                            v.fields.clone(),
                            Some(v.ident.clone()),
                            e.ident.clone(),
                            v.attrs.clone(),
                            extraction_mode,
                        )?;

                        let enum_name_str = e.ident.to_string();
                        let variant_name = v.ident.to_string();
                        let expected_symbol = format!("{}_{}", enum_name_str, variant_name);

                        let detection_expr = quote! {
                            if node.kind() == #expected_symbol {
                                return #extract_expr;
                            }
                        };

                        Ok(detection_expr)
                    }).collect::<Result<Vec<_>>>()?;

                    let unique_shape_variant_detection = e.variants.iter().filter_map(|v| {
                        let arity = v.fields.iter().count();
                        let arity_is_unique = variant_arity_counts
                            .get(&arity)
                            .copied()
                            .unwrap_or_default()
                            == 1;

                        let is_leaf_variant = if let Fields::Unnamed(ref unnamed) = v.fields {
                            unnamed.unnamed.len() == 1 && unnamed.unnamed[0]
                                .attrs
                                .iter()
                                .any(|attr| attr.path() == &syn::parse_quote!(adze::leaf))
                        } else {
                            false
                        };

                        if !arity_is_unique || is_leaf_variant || arity == 0 {
                            return None;
                        }

                        let extract_expr = gen_struct_or_variant(
                            v.fields.clone(),
                            Some(v.ident.clone()),
                            e.ident.clone(),
                            v.attrs.clone(),
                            extraction_mode,
                        ).ok()?;

                        Some(quote! {
                            if unwrapped_node.kind() == stringify!(#enum_name) && non_extra_children.len() == #arity {
                                let node = unwrapped_node;
                                return #extract_expr;
                            }
                        })
                    }).collect::<Vec<_>>();

                    let leaf_variant_detection = e.variants.iter().filter_map(|v| {
                        let is_leaf_variant = if let Fields::Unnamed(ref unnamed) = v.fields {
                            unnamed.unnamed.len() == 1 && unnamed.unnamed[0]
                                .attrs
                                .iter()
                                .any(|attr| attr.path() == &syn::parse_quote!(adze::leaf))
                        } else {
                            false
                        };

                        if is_leaf_variant {
                            let extract_expr = gen_struct_or_variant(
                                v.fields.clone(),
                                Some(v.ident.clone()),
                                e.ident.clone(),
                                v.attrs.clone(),
                                extraction_mode,
                            ).ok()?;

                            Some(quote! {
                                return #extract_expr;
                            })
                        } else {
                            None
                        }
                    }).collect::<Vec<_>>();

                    e.attrs.retain(|a| !is_sitter_attr(a));
                    e.variants.iter_mut().for_each(|v| {
                        v.attrs.retain(|a| !is_sitter_attr(a));
                        v.fields.iter_mut().for_each(|f| {
                            f.attrs.retain(|a| !is_sitter_attr(a));
                        });
                    });

                    let variant_detection_logic_std = e.variants.iter().map(|v| {
                        let extract_expr = gen_struct_or_variant(
                            v.fields.clone(),
                            Some(v.ident.clone()),
                            e.ident.clone(),
                            v.attrs.clone(),
                            extraction_mode,
                        )?;

                        let enum_name_str = e.ident.to_string();
                        let variant_name = v.ident.to_string();
                        let expected_symbol = format!("{}_{}", enum_name_str, variant_name);

                        let detection_expr = quote! {
                            if child_node.kind() == #expected_symbol {
                                return #extract_expr;
                            }
                        };

                        Ok(detection_expr)
                    }).collect::<Result<Vec<_>>>()?;

                    let fallible = extraction_mode == ExtractMode::Fallible;
                    let enum_body: proc_macro2::TokenStream = if cfg!(feature = "pure-rust") {
                        let self_call = if fallible {
                            quote!(Self::try_extract)
                        } else {
                            quote!(Self::extract)
                        };
                        let tail = if fallible {
                            quote! {
                                return Err(::adze::errors::ParseError {
                                    reason: ::adze::errors::ParseErrorReason::UnexpectedToken(format!(
                                        "Could not determine enum variant from tree structure: node kind='{}', symbol={}, child_count={}",
                                        unwrapped_node.kind(), unwrapped_node.symbol, unwrapped_node.children.len()
                                    )),
                                    start: unwrapped_node.start_byte,
                                    end: unwrapped_node.end_byte,
                                    expected: Vec::new(),
                                });
                            }
                        } else {
                            quote! {
                                panic!("Could not determine enum variant from tree structure: node kind='{}', symbol={}, child_count={}",
                                    unwrapped_node.kind(), unwrapped_node.symbol, unwrapped_node.children.len())
                            }
                        };
                        quote! {
                            let node = node.expect("Extract called with None node for enum");

                            fn unwrap_hidden_rules<'a>(node: &'a ::adze::pure_parser::ParsedNode) -> &'a ::adze::pure_parser::ParsedNode {
                                let non_extra_children: Vec<_> = node.children.iter().filter(|c| !c.is_extra).collect();
                                if (node.kind() == "source_file" || node.kind().starts_with('_')) && !non_extra_children.is_empty() {
                                    return unwrap_hidden_rules(non_extra_children[0]);
                                }
                                node
                            }

                            let unwrapped_node = unwrap_hidden_rules(node);

                            // Check if this is an enum wrapper node, ignoring extras
                            let non_extra_children: Vec<_> = unwrapped_node.children.iter().filter(|c| !c.is_extra).collect();
                            if unwrapped_node.kind() == stringify!(#enum_name) && non_extra_children.len() == 1 {
                                let child_node = non_extra_children[0];
                                return #self_call(Some(child_node), source, _last_idx, _leaf_fn);
                            }

                            let node = unwrapped_node;
                            #(#variant_detection_logic)*
                            #(#unique_shape_variant_detection)*

                            if non_extra_children.is_empty() {
                                let node = unwrapped_node;
                                #(#leaf_variant_detection)*
                            }

                            #tail
                        }
                    } else {
                        let self_call = if fallible {
                            quote!(Self::try_extract)
                        } else {
                            quote!(Self::extract)
                        };
                        let tail = if fallible {
                            quote! {
                                return Err(::adze::errors::ParseError {
                                    reason: ::adze::errors::ParseErrorReason::UnexpectedToken(format!(
                                        "Could not determine enum variant from tree structure: node kind='{}', child_count={}",
                                        unwrapped_node.kind(), unwrapped_node.child_count()
                                    )),
                                    start: unwrapped_node.start_byte(),
                                    end: unwrapped_node.end_byte(),
                                    expected: Vec::new(),
                                });
                            }
                        } else {
                            quote! {
                                panic!("Could not determine enum variant from tree structure: node kind='{}', child_count={}",
                                    unwrapped_node.kind(), unwrapped_node.child_count())
                            }
                        };
                        quote! {
                            let node = node.expect("Extract called with None node for enum");

                            fn unwrap_hidden_rules(node: ::adze::tree_sitter::Node) -> ::adze::tree_sitter::Node {
                                let mut cursor = node.walk();
                                let non_extra_children: Vec<_> = node.children(&mut cursor).filter(|c| !c.is_extra()).collect();
                                if (node.kind() == "source_file" || node.kind().starts_with('_')) && !non_extra_children.is_empty() {
                                    if let Some(child) = non_extra_children.first() {
                                        return unwrap_hidden_rules(*child);
                                    }
                                }
                                node
                            }

                            let unwrapped_node = unwrap_hidden_rules(node);

                            // Check if this is an enum wrapper node, ignoring extras
                            let mut cursor = unwrapped_node.walk();
                            let non_extra_children: Vec<_> = unwrapped_node.children(&mut cursor).filter(|c| !c.is_extra()).collect();
                            if unwrapped_node.kind() == stringify!(#enum_name) && non_extra_children.len() == 1 {
                                let child = non_extra_children[0];
                                return #self_call(Some(child), source, _last_idx, _leaf_fn);
                            }

                            let child_node = unwrapped_node;
                            #(#variant_detection_logic_std)*

                            #tail
                        }
                    };

                    let extract_impl: Item = match (cfg!(feature = "pure-rust"), fallible) {
                        (true, true) => syn::parse_quote! {
                            impl ::adze::Extract<#enum_name> for #enum_name {
                                type LeafFn = ();
                                const GRAMMAR_NAME: &'static str = #grammar_name_str;

                                fn extract(node: Option<&::adze::pure_parser::ParsedNode>, source: &[u8], _last_idx: usize, _leaf_fn: Option<&Self::LeafFn>) -> Self {
                                    Self::try_extract(node, source, _last_idx, _leaf_fn)
                                        .unwrap_or_else(|error| panic!("leaf conversion failed: {error}"))
                                }

                                fn try_extract(node: Option<&::adze::pure_parser::ParsedNode>, source: &[u8], _last_idx: usize, _leaf_fn: Option<&Self::LeafFn>) -> Result<Self, ::adze::errors::ParseError> {
                                    #enum_body
                                }
                            }
                        },
                        (true, false) => syn::parse_quote! {
                            impl ::adze::Extract<#enum_name> for #enum_name {
                                type LeafFn = ();
                                const GRAMMAR_NAME: &'static str = #grammar_name_str;

                                fn extract(node: Option<&::adze::pure_parser::ParsedNode>, source: &[u8], _last_idx: usize, _leaf_fn: Option<&Self::LeafFn>) -> Self {
                                    #enum_body
                                }
                            }
                        },
                        (false, true) => syn::parse_quote! {
                            impl ::adze::Extract<#enum_name> for #enum_name {
                                type LeafFn = ();

                                fn extract(node: Option<::adze::tree_sitter::Node>, source: &[u8], _last_idx: usize, _leaf_fn: Option<&Self::LeafFn>) -> Self {
                                    Self::try_extract(node, source, _last_idx, _leaf_fn)
                                        .unwrap_or_else(|error| panic!("leaf conversion failed: {error}"))
                                }

                                fn try_extract(node: Option<::adze::tree_sitter::Node>, source: &[u8], _last_idx: usize, _leaf_fn: Option<&Self::LeafFn>) -> Result<Self, ::adze::errors::ParseError> {
                                    #enum_body
                                }
                            }
                        },
                        (false, false) => syn::parse_quote! {
                            impl ::adze::Extract<#enum_name> for #enum_name {
                                type LeafFn = ();

                                fn extract(node: Option<::adze::tree_sitter::Node>, source: &[u8], _last_idx: usize, _leaf_fn: Option<&Self::LeafFn>) -> Self {
                                    #enum_body
                                }
                            }
                        },
                    };
                    Ok(vec![Item::Enum(e), extract_impl])
            }

            Item::Struct(mut s) => {
                    let struct_name = &s.ident;
                    let extract_expr = gen_struct_or_variant(
                        s.fields.clone(),
                        None,
                        s.ident.clone(),
                        s.attrs.clone(),
                        extraction_mode,
                    )?;

                    s.attrs.retain(|a| !is_sitter_attr(a));
                    s.fields.iter_mut().for_each(|f| {
                        f.attrs.retain(|a| !is_sitter_attr(a));
                    });

                    let extract_impl: Item = match (
                        cfg!(feature = "pure-rust"),
                        extraction_mode == ExtractMode::Fallible,
                    ) {
                        (true, true) => syn::parse_quote! {
                            impl ::adze::Extract<#struct_name> for #struct_name {
                                type LeafFn = ();
                                const GRAMMAR_NAME: &'static str = #grammar_name_str;

                                fn extract(node: Option<&::adze::pure_parser::ParsedNode>, source: &[u8], last_idx: usize, _leaf_fn: Option<&Self::LeafFn>) -> Self {
                                    Self::try_extract(node, source, last_idx, _leaf_fn)
                                        .unwrap_or_else(|error| panic!("leaf conversion failed: {error}"))
                                }

                                fn try_extract(node: Option<&::adze::pure_parser::ParsedNode>, source: &[u8], last_idx: usize, _leaf_fn: Option<&Self::LeafFn>) -> Result<Self, ::adze::errors::ParseError> {
                                    let node = node.expect("Extract called with None node for struct");
                                    #extract_expr
                                }
                            }
                        },
                        (true, false) => syn::parse_quote! {
                            impl ::adze::Extract<#struct_name> for #struct_name {
                                type LeafFn = ();
                                const GRAMMAR_NAME: &'static str = #grammar_name_str;

                                fn extract(node: Option<&::adze::pure_parser::ParsedNode>, source: &[u8], last_idx: usize, _leaf_fn: Option<&Self::LeafFn>) -> Self {
                                    let node = node.expect("Extract called with None node for struct");
                                    #extract_expr
                                }
                            }
                        },
                        (false, true) => syn::parse_quote! {
                            impl ::adze::Extract<#struct_name> for #struct_name {
                                type LeafFn = ();

                                fn extract(node: Option<::adze::tree_sitter::Node>, source: &[u8], last_idx: usize, _leaf_fn: Option<&Self::LeafFn>) -> Self {
                                    Self::try_extract(node, source, last_idx, _leaf_fn)
                                        .unwrap_or_else(|error| panic!("leaf conversion failed: {error}"))
                                }

                                fn try_extract(node: Option<::adze::tree_sitter::Node>, source: &[u8], last_idx: usize, _leaf_fn: Option<&Self::LeafFn>) -> Result<Self, ::adze::errors::ParseError> {
                                    let node = node.expect("Extract called with None node for struct");
                                    #extract_expr
                                }
                            }
                        },
                        (false, false) => syn::parse_quote! {
                            impl ::adze::Extract<#struct_name> for #struct_name {
                                type LeafFn = ();

                                fn extract(node: Option<::adze::tree_sitter::Node>, source: &[u8], last_idx: usize, _leaf_fn: Option<&Self::LeafFn>) -> Self {
                                    let node = node.expect("Extract called with None node for struct");
                                    #extract_expr
                                }
                            }
                        },
                    };

                    Ok(vec![Item::Struct(s), extract_impl])
            }

            o => Ok(vec![o]),
        })
        .sift::<Vec<_>>()?.into_iter().flatten().collect();

    let tree_sitter_ident = Ident::new(
        &format!("tree_sitter_{grammar_name_str}"),
        Span::call_site(),
    );

    if cfg!(feature = "pure-rust") {
        transformed.push(syn::parse_quote! {
            include!(concat!(env!("OUT_DIR"), "/grammar_", #grammar_name_str, "/parser_", #grammar_name_str, ".rs"));
        });

        transformed.push(syn::parse_quote! {
            pub fn language() -> &'static ::adze::pure_parser::TSLanguage {
                &LANGUAGE
            }
        });

        transformed.push(syn::parse_quote! {
            pub const LANGUAGE_REF: &'static ::adze::pure_parser::TSLanguage = &LANGUAGE;
        });

        let root_type_docstr = format!("[`{root_type}`]");
        transformed.push(syn::parse_quote! {
            /// Parse an input string according to the grammar. Returns either any parsing errors that happened, or a
            #[doc = #root_type_docstr]
            /// instance containing the parsed structured data.
            pub fn parse(input: &str) -> core::result::Result<#root_type, Vec<::adze::errors::ParseError>> {
                ::adze::__private::parse::<#root_type>(input, || language())
            }
        });
    } else {
        transformed.push(syn::parse_quote! {
            unsafe extern "C" {
                fn #tree_sitter_ident() -> ::adze::tree_sitter::Language;
            }
        });

        transformed.push(syn::parse_quote! {
            pub fn language() -> ::adze::tree_sitter::Language {
                unsafe { #tree_sitter_ident() }
            }
        });

        let root_type_docstr = format!("[`{root_type}`]");
        transformed.push(syn::parse_quote! {
            /// Parse an input string according to the grammar. Returns either any parsing errors that happened, or a
            #[doc = #root_type_docstr]
            /// instance containing the parsed structured data.
            pub fn parse(input: &str) -> core::result::Result<#root_type, Vec<::adze::errors::ParseError>> {
                ::adze::__private::parse::<#root_type>(input, language)
            }
        });
    }

    let mut filtered_attrs = input.attrs;
    filtered_attrs.retain(|a| !is_sitter_attr(a));
    Ok(ItemMod {
        attrs: filtered_attrs,
        vis: input.vis,
        unsafety: None,
        mod_token: input.mod_token,
        ident: input.ident,
        content: Some((brace, transformed)),
        semi: input.semi,
    })
}

#[cfg(test)]
mod name_hygiene_1086_tests {
    use super::*;

    fn parse_ty(source: &str) -> Type {
        syn::parse_str(source).expect("test type must parse")
    }

    /// Render through the same `TokenStream` display as the assertions, so
    /// whitespace differences in the input never matter.
    fn ty_string(ty: &Type) -> String {
        ty.to_token_stream().to_string()
    }

    fn renamed(source: &str) -> String {
        let mut ty = parse_ty(source);
        rename_with_leaf_to_try(&mut ty);
        ty_string(&ty)
    }

    fn assert_renamed_to(input: &str, expected: &str) {
        assert_eq!(
            renamed(input),
            ty_string(&parse_ty(expected)),
            "rename of `{input}`"
        );
    }

    fn assert_renamed_unchanged(source: &str) {
        assert_renamed_to(source, source);
    }

    #[test]
    fn try_rename_rewrites_adze_resolved_with_leaf() {
        assert_renamed_to("adze::WithLeaf<String>", "adze::WithTryLeaf<String>");
        assert_renamed_to(
            "Vec<adze::WithLeaf<String>>",
            "Vec<adze::WithTryLeaf<String>>",
        );
        assert_renamed_to(
            "adze::WithLeaf<adze::WithLeaf<String>>",
            "adze::WithTryLeaf<adze::WithTryLeaf<String>>",
        );
        // Leading-colon spelling of the same adze wrapper is still adze's.
        assert_renamed_to("::adze::WithLeaf<String>", "::adze::WithTryLeaf<String>");
    }

    #[test]
    fn try_rename_leaves_user_with_leaf_types_untouched() {
        // Bare user type sharing the name.
        assert_renamed_unchanged("WithLeaf<String>");
        // Qualified user types sharing the name.
        assert_renamed_unchanged("my_crate::WithLeaf<String>");
        assert_renamed_unchanged("crate::WithLeaf<String>");
        // Nested inside other containers and compound types.
        assert_renamed_unchanged("Option<WithLeaf<String>>");
        assert_renamed_unchanged("Vec<my::WithLeaf<u32>>");
        assert_renamed_unchanged("Result<WithLeaf<A>, WithLeaf<B>>");
        assert_renamed_unchanged("fn(WithLeaf<String>) -> bool");
        assert_renamed_unchanged("&[WithLeaf<u8>]");
        // Mixed: only the adze-resolved wrapper is rewritten.
        assert_renamed_to(
            "Wrapper<adze::WithLeaf<A>, other::WithLeaf<B>>",
            "Wrapper<adze::WithTryLeaf<A>, other::WithLeaf<B>>",
        );
    }

    fn skip_over_for_fields() -> HashSet<&'static str> {
        ["Spanned", "Box", "Option", "Vec"].into_iter().collect()
    }

    #[test]
    fn wrap_try_leaf_type_scopes_rename_to_adze_wrapper() {
        let skip_over = skip_over_for_fields();
        let wrapped = |source: &str| ty_string(&wrap_try_leaf_type(&parse_ty(source), &skip_over));
        let expected = |source: &str| ty_string(&parse_ty(source));

        assert_eq!(
            wrapped("my::WithLeaf<String>"),
            expected("adze::WithTryLeaf<my::WithLeaf<String>>"),
        );
        assert_eq!(
            wrapped("WithLeaf<String>"),
            expected("adze::WithTryLeaf<WithLeaf<String>>"),
        );
        assert_eq!(
            wrapped("Vec<String>"),
            expected("Vec<adze::WithTryLeaf<String>>"),
        );
        assert_eq!(wrapped("String"), expected("adze::WithTryLeaf<String>"),);
    }

    #[test]
    fn grammar_expansion_preserves_colliding_user_with_leaf() {
        let expanded = expand_grammar(syn::parse_quote! {
            #[adze::grammar("hygiene_test")]
            mod grammar {
                #[adze::language]
                pub struct Root {
                    #[adze::leaf(pattern = r"\d+", try_transform = |v: &str| v.parse::<i32>())]
                    value: my_crate::WithLeaf<i32>,
                }
            }
        })
        .expect("grammar with user WithLeaf must expand");
        let text = expanded.to_token_stream().to_string();
        assert!(
            text.contains("adze :: WithTryLeaf < my_crate :: WithLeaf"),
            "fixed expansion wraps the user type instead of rewriting it: {text}"
        );
        assert!(
            !text.contains("my_crate :: WithTryLeaf"),
            "user path must survive the fallible rewrite: {text}"
        );
    }

    #[test]
    fn fallible_closure_keeps_stable_binding_names() {
        let closure: Expr = syn::parse_quote!(|v| v.parse::<i32>());
        let text = fallible_closure_expr(closure).to_token_stream().to_string();
        for binding in ["__adze_text", "__adze_try_leaf", "__adze_error"] {
            assert!(
                text.contains(binding),
                "generated wrapper must keep the `{binding}` binding name: {text}"
            );
        }
    }

    #[test]
    fn fallible_closure_preserves_adversarial_user_bindings() {
        // The user closure itself shadows every generated binding name; the
        // macro must carry it through verbatim and still emit exactly one
        // generated wrapper around it.
        let closure: Expr = syn::parse_quote!(|__adze_text| __adze_text.parse::<i32>());
        let text = fallible_closure_expr(closure).to_token_stream().to_string();
        // Generated parameter plus generated call argument, plus the user's
        // own parameter and body use.
        let uses = text.matches("__adze_text").count();
        assert_eq!(
            uses, 4,
            "user `__adze_text` must survive alongside the generated one: {text}"
        );
        assert_eq!(
            text.matches("__adze_try_leaf").count(),
            2,
            "exactly one generated `let` binding and its use: {text}"
        );
        assert!(
            text.contains("__adze_error"),
            "generated error mapping must survive adversarial input: {text}"
        );
    }
}
