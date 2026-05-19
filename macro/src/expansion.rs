//! Grammar macro expansion from annotated Rust types to parser code.

use std::collections::{HashMap, HashSet};

use crate::errors::IteratorExt as _;
use adze_common::*;
use proc_macro2::Span;
use quote::{ToTokens, quote};
use syn::{parse::Parse, punctuated::Punctuated, *};

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

fn gen_field(ident_str: String, leaf: Field) -> Expr {
    let leaf_type = leaf.ty;

    let leaf_attr = leaf
        .attrs
        .iter()
        .find(|attr| attr.path() == &syn::parse_quote!(adze::leaf));

    let leaf_params = leaf_attr.and_then(|a| {
        a.parse_args_with(Punctuated::<NameValueExpr, Token![,]>::parse_terminated)
            .ok()
    });

    let transform_param = leaf_params.as_ref().and_then(|p| {
        p.iter()
            .find(|param| param.path == "transform")
            .map(|p| p.expr.clone())
    });

    let (leaf_type, closure_expr): (Type, Expr) = match transform_param {
        Some(closure) => {
            let mut non_leaf = HashSet::new();
            non_leaf.insert("Spanned");
            non_leaf.insert("Box");
            non_leaf.insert("Option");
            non_leaf.insert("Vec");
            let wrapped_leaf_type = wrap_leaf_type(&leaf_type, &non_leaf);
            (wrapped_leaf_type, syn::parse_quote!(Some(&#closure)))
        }
        None => (leaf_type, syn::parse_quote!(None)),
    };

    syn::parse_quote!({
        ::adze::__private::extract_field::<#leaf_type,_>(cursor, source, last_idx, #ident_str, #closure_expr)
    })
}

fn gen_struct_or_variant(
    fields: Fields,
    variant_ident: Option<Ident>,
    containing_type: Ident,
    container_attrs: Vec<Attribute>,
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
            let leaf_attr = field
                .attrs
                .iter()
                .find(|attr| attr.path() == &syn::parse_quote!(adze::leaf));

            let leaf_params = leaf_attr.and_then(|a| {
                a.parse_args_with(Punctuated::<NameValueExpr, Token![,]>::parse_terminated)
                    .ok()
            });

            let transform_param = leaf_params.as_ref().and_then(|p| {
                p.iter()
                    .find(|param| param.path == "transform")
                    .map(|p| p.expr.clone())
            });

            let (leaf_type, closure_expr): (Type, Expr) = match transform_param {
                Some(closure) => {
                    let mut non_leaf = HashSet::new();
                    non_leaf.insert("Spanned");
                    non_leaf.insert("Box");
                    non_leaf.insert("Option");
                    non_leaf.insert("Vec");
                    let wrapped_leaf_type = wrap_leaf_type(leaf_type, &non_leaf);
                    (wrapped_leaf_type, syn::parse_quote!(Some(&#closure)))
                }
                None => (leaf_type.clone(), syn::parse_quote!(None)),
            };

            let construct_name = quote! {
                #containing_type::#variant_name
            };

            // In both modes, 'node' should be passed appropriately.
            // For tree-sitter, it's Node by value. For pure-rust, it's &ParsedNode.
            // We use 'Some(node)' because gen_struct_or_variant is called inside extract()
            // where 'node' is already un-Optioned.
            return Ok(syn::parse_quote!({
                let value = <#leaf_type as ::adze::Extract<_>>::extract(Some(node), source, 0, #closure_expr);
                #construct_name(value)
            }));
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

            gen_field("unit".to_string(), dummy_field)
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

                    gen_field(ident_str, field.clone())
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

    Ok(
        syn::parse_quote!(::adze::__private::extract_struct_or_variant(node, move |cursor, last_idx| #construct_expr)),
    )
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

                    let extract_impl: Item = if cfg!(feature = "pure-rust") {
                        syn::parse_quote! {
                            impl ::adze::Extract<#enum_name> for #enum_name {
                                type LeafFn = ();
                                const GRAMMAR_NAME: &'static str = #grammar_name_str;

                                #[allow(non_snake_case)]
                                fn extract(node: Option<&::adze::pure_parser::ParsedNode>, source: &[u8], _last_idx: usize, _leaf_fn: Option<&Self::LeafFn>) -> Self {
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
                                        return Self::extract(Some(child_node), source, _last_idx, _leaf_fn);
                                    }

                                    let node = unwrapped_node;
                                    #(#variant_detection_logic)*
                                    #(#unique_shape_variant_detection)*

                                    if non_extra_children.is_empty() {
                                        let node = unwrapped_node;
                                        #(#leaf_variant_detection)*
                                    }

                                    panic!("Could not determine enum variant from tree structure: node kind='{}', symbol={}, child_count={}",
                                        unwrapped_node.kind(), unwrapped_node.symbol, unwrapped_node.children.len())
                                }
                            }
                        }
                    } else {
                        syn::parse_quote! {
                            impl ::adze::Extract<#enum_name> for #enum_name {
                                type LeafFn = ();

                                #[allow(non_snake_case)]
                                fn extract(node: Option<::adze::tree_sitter::Node>, source: &[u8], _last_idx: usize, _leaf_fn: Option<&Self::LeafFn>) -> Self {
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
                                        return Self::extract(Some(child), source, _last_idx, _leaf_fn);
                                    }

                                    let child_node = unwrapped_node;
                                    #(#variant_detection_logic_std)*

                                    panic!("Could not determine enum variant from tree structure: node kind='{}', child_count={}",
                                        unwrapped_node.kind(), unwrapped_node.child_count())
                                }
                            }
                        }
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
                    )?;

                    s.attrs.retain(|a| !is_sitter_attr(a));
                    s.fields.iter_mut().for_each(|f| {
                        f.attrs.retain(|a| !is_sitter_attr(a));
                    });

                    let extract_impl: Item = if cfg!(feature = "pure-rust") {
                        syn::parse_quote! {
                            impl ::adze::Extract<#struct_name> for #struct_name {
                                type LeafFn = ();
                                const GRAMMAR_NAME: &'static str = #grammar_name_str;

                                #[allow(non_snake_case)]
                                fn extract(node: Option<&::adze::pure_parser::ParsedNode>, source: &[u8], last_idx: usize, _leaf_fn: Option<&Self::LeafFn>) -> Self {
                                    let node = node.expect("Extract called with None node for struct");
                                    #extract_expr
                                }
                            }
                        }
                    } else {
                        syn::parse_quote! {
                            impl ::adze::Extract<#struct_name> for #struct_name {
                                type LeafFn = ();

                                #[allow(non_snake_case)]
                                fn extract(node: Option<::adze::tree_sitter::Node>, source: &[u8], last_idx: usize, _leaf_fn: Option<&Self::LeafFn>) -> Self {
                                    let node = node.expect("Extract called with None node for struct");
                                    #extract_expr
                                }
                            }
                        }
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
