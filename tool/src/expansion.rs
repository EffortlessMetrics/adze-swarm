//! Grammar expansion from annotated Rust types to Tree-sitter JSON.

use std::collections::HashSet;

use adze_common::*;
use serde_json::{Map, Value, json};
use syn::{parse::Parse, punctuated::Punctuated, spanned::Spanned, *};

use crate::error::{Result as ToolResult, ToolError};

#[cfg(not(debug_assertions))]
macro_rules! debug_trace {
    ($($arg:tt)*) => {};
}

#[cfg(debug_assertions)]
macro_rules! debug_trace {
    ($($arg:tt)*) => {
        if std::env::var("RUST_LOG")
            .ok()
            .unwrap_or_default()
            .contains("debug")
        {
            eprintln!($($arg)*);
        }
    };
}

fn gen_field(
    path: String,
    leaf_type: Type,
    leaf_attrs: Vec<Attribute>,
    word_rule: &mut Option<String>,
    out: &mut Map<String, Value>,
) -> ToolResult<(Value, bool)> {
    let leaf_attr = leaf_attrs
        .iter()
        .find(|attr| attr.path() == &syn::parse_quote!(adze::leaf));

    if leaf_attrs
        .iter()
        .any(|attr| attr.path() == &syn::parse_quote!(adze::word))
    {
        if word_rule.is_some() {
            Err(ToolError::MultipleWordRules)?;
        }

        *word_rule = Some(path.clone());
    }

    let leaf_params = leaf_attr.and_then(|a| {
        a.parse_args_with(Punctuated::<NameValueExpr, Token![,]>::parse_terminated)
            .ok()
    });

    let pattern_param = leaf_params.as_ref().and_then(|p| {
        p.iter()
            .find(|param| param.path == "pattern")
            .map(|p| p.expr.clone())
    });

    let text_param = leaf_params.as_ref().and_then(|p| {
        p.iter()
            .find(|param| param.path == "text")
            .map(|p| p.expr.clone())
    });

    let mut skip_over = HashSet::new();
    skip_over.insert("Spanned");
    skip_over.insert("Box");

    let (inner_type_vec, is_vec) = try_extract_inner_type(&leaf_type, "Vec", &skip_over);
    let (inner_type_option, is_option) = try_extract_inner_type(&leaf_type, "Option", &skip_over);

    if !is_vec && !is_option {
        if let Some(Expr::Lit(lit)) = pattern_param {
            if let Lit::Str(s) = &lit.lit {
                // Validate that the pattern is not empty
                if s.value().is_empty() {
                    return Err(ToolError::GrammarValidation {
                        reason: format!(
                            "Empty patterns are not supported. Token '{}' has an empty pattern value.",
                            path
                        ),
                    });
                }

                out.insert(
                    path.clone(),
                    json!({
                        "type": "PATTERN",
                        "value": s.value(),
                    }),
                );

                Ok((
                    json!({
                        "type": "SYMBOL",
                        "name": path
                    }),
                    is_option,
                ))
            } else {
                Err(ToolError::ExpectedStringLiteral {
                    context: "pattern".to_string(),
                    actual: format!("{:?}", lit.lit),
                })
            }
        } else if let Some(Expr::Lit(lit)) = text_param {
            if let Lit::Str(s) = &lit.lit {
                // Allow empty strings for now - they may be used in some grammars
                // to avoid the EmptyString error

                out.insert(
                    path.clone(),
                    json!({
                        "type": "STRING",
                        "value": s.value(),
                    }),
                );

                Ok((
                    json!({
                        "type": "SYMBOL",
                        "name": path
                    }),
                    is_option,
                ))
            } else {
                Err(ToolError::ExpectedStringLiteral {
                    context: "text".to_string(),
                    actual: format!("{:?}", lit.lit),
                })
            }
        } else {
            let symbol_name = match filter_inner_type(&leaf_type, &skip_over) {
                Type::Path(p) => {
                    if p.path.segments.len() == 1 {
                        p.path.segments[0].ident.to_string()
                    } else {
                        return Err(ToolError::ExpectedSingleSegmentPath {
                            actual: format!("{}", p.path.segments.len()),
                        });
                    }
                }
                Type::Tuple(t) if t.elems.is_empty() => {
                    // Unit type () - generate a synthetic name
                    format!("{path}_unit")
                }
                _ => {
                    return Err(ToolError::ExpectedPathType {
                        actual: "non-path type".to_string(),
                    });
                }
            };

            Ok((
                json!({
                    "type": "SYMBOL",
                    "name": symbol_name,
                }),
                false,
            ))
        }
    } else if is_vec {
        // Check if we need to pass the inner element type name
        let element_path = if path.ends_with("_vec_contents") {
            // This is a recursive call - use the path as-is
            path.clone()
        } else {
            // This is the initial call - we'll generate a _vec_contents rule
            path.clone()
        };

        let (field_json, field_optional) = gen_field(
            element_path,
            inner_type_vec,
            leaf_attrs.clone(),
            word_rule,
            out,
        )?;

        let delimited_attr = leaf_attrs
            .iter()
            .find(|attr| attr.path() == &syn::parse_quote!(adze::delimited));

        let delimited_params =
            delimited_attr.and_then(|a| a.parse_args_with(FieldThenParams::parse).ok());

        let delimiter_json = if let Some(p) = delimited_params {
            Some(gen_field(
                format!("{path}_vec_delimiter"),
                p.field.ty,
                p.field.attrs,
                word_rule,
                out,
            )?)
        } else {
            None
        };

        let repeat_attr = leaf_attrs
            .iter()
            .find(|attr| attr.path() == &syn::parse_quote!(adze::repeat));

        let repeat_params = repeat_attr.and_then(|a| {
            a.parse_args_with(Punctuated::<NameValueExpr, Token![,]>::parse_terminated)
                .ok()
        });

        let repeat_non_empty = repeat_params
            .and_then(|p| {
                p.iter()
                    .find(|param| param.path == "non_empty")
                    .map(|p| p.expr.clone())
            })
            .map(|e| e == syn::parse_quote!(true))
            .unwrap_or(false);

        let field_rule_non_optional = json!({
            "type": "FIELD",
            "name": format!("{path}_vec_element"),
            "content": field_json
        });

        let field_rule = if field_optional {
            json!({
                "type": "CHOICE",
                "members": [
                    {
                        "type": "BLANK"
                    },
                    field_rule_non_optional
                ]
            })
        } else {
            field_rule_non_optional
        };

        let vec_contents = if let Some((delimiter_json, delimiter_optional)) = delimiter_json {
            let delim_made_optional = if delimiter_optional {
                json!({
                    "type": "CHOICE",
                    "members": [
                        {
                            "type": "BLANK"
                        },
                        delimiter_json
                    ]
                })
            } else {
                delimiter_json
            };

            json!({
                "type": "SEQ",
                "members": [
                    field_rule,
                    {
                        "type": if field_optional {
                            "REPEAT1"
                        } else {
                            "REPEAT"
                        },
                        "content": {
                            "type": "SEQ",
                            "members": [
                                delim_made_optional,
                                field_rule,
                            ]
                        }
                    }
                ]
            })
        } else {
            // Always use REPEAT1 for the rule definition to avoid empty string issues
            // The empty case is handled by wrapping the reference in CHOICE
            json!({
                "type": "REPEAT1",
                "content": field_rule
            })
        };

        // Always create a named rule with REPEAT1 (never empty)
        let contents_ident = format!("{path}_vec_contents");
        out.insert(contents_ident.clone(), vec_contents);

        // Return a reference to the named rule
        // If the Vec can be empty, wrap in CHOICE to make it optional
        let reference = if !repeat_non_empty {
            // Vec can be empty, so make the reference optional
            json!({
                "type": "CHOICE",
                "members": [
                    {
                        "type": "BLANK"
                    },
                    {
                        "type": "SYMBOL",
                        "name": contents_ident,
                    }
                ]
            })
        } else {
            // Vec must have at least one element
            json!({
                "type": "SYMBOL",
                "name": contents_ident,
            })
        };

        Ok((reference, false)) // Never mark as optional since we handle it in the reference
    } else {
        // is_option
        let (field_json, field_optional) =
            gen_field(path, inner_type_option, leaf_attrs, word_rule, out)?;

        if field_optional {
            return Err(ToolError::NestedOptionType);
        }

        Ok((field_json, true))
    }
}

/// Precedence attributes handler for grammar rules.
///
/// Manages the three types of precedence attributes:
/// - `#[adze::prec(n)]`: Non-associative precedence
/// - `#[adze::prec_left(n)]`: Left-associative precedence  
/// - `#[adze::prec_right(n)]`: Right-associative precedence
///
/// Only one precedence attribute can be applied per rule. The precedence value
/// must be an integer literal in the range 0 to 4294967295 (u32).
#[derive(Default)]
struct Precs<'a> {
    prec: Option<&'a Attribute>,
    prec_left: Option<&'a Attribute>,
    prec_right: Option<&'a Attribute>,
}

impl<'a> Precs<'a> {
    /// Extracts precedence attributes from a list of attributes.
    ///
    /// Searches for `prec`, `prec_left`, and `prec_right` attributes
    /// and stores references to them for later validation and application.
    fn new(attrs: &'a [Attribute]) -> Self {
        Self {
            prec: attrs
                .iter()
                .find(|attr| attr.path() == &syn::parse_quote!(adze::prec)),
            prec_left: attrs
                .iter()
                .find(|attr| attr.path() == &syn::parse_quote!(adze::prec_left)),
            prec_right: attrs
                .iter()
                .find(|attr| attr.path() == &syn::parse_quote!(adze::prec_right)),
        }
    }

    /// Applies precedence to a grammar rule, validating attribute usage.
    ///
    /// # Errors
    ///
    /// Returns a `syn::Error` in the following cases:
    /// - Multiple precedence attributes are specified on the same rule
    /// - The precedence value is not an integer literal (e.g., string, float, variable)
    /// - The precedence value is outside the valid u32 range (0 to 4294967295)
    ///
    /// Error messages include specific guidance on how to fix the issue.
    fn apply(&self, base_rule: Value) -> syn::Result<Value> {
        let count = self.prec.iter().count()
            + self.prec_left.iter().count()
            + self.prec_right.iter().count();
        if count > 1 {
            let span = self
                .prec
                .map(|a| a.span())
                .or_else(|| self.prec_left.map(|a| a.span()))
                .or_else(|| self.prec_right.map(|a| a.span()))
                .unwrap();

            // Collect which attributes were found for a better error message
            let mut found_attrs = Vec::new();
            if self.prec.is_some() {
                found_attrs.push("prec");
            }
            if self.prec_left.is_some() {
                found_attrs.push("prec_left");
            }
            if self.prec_right.is_some() {
                found_attrs.push("prec_right");
            }

            return Err(syn::Error::new(
                span,
                format!(
                    "only one of prec, prec_left, and prec_right can be specified, but found: {}",
                    found_attrs.join(", ")
                ),
            ));
        }

        if let Some(attr) = self.prec {
            let expr: Expr = attr.parse_args()?;
            if let Expr::Lit(ExprLit {
                lit: Lit::Int(i), ..
            }) = expr
            {
                let value = i.base10_parse::<u32>().map_err(|e| {
                    syn::Error::new(
                        i.span(),
                        format!(
                            "Invalid integer literal for precedence: {} (must be a valid u32)",
                            e
                        ),
                    )
                })?;
                Ok(json!({
                    "type": "PREC",
                    "value": value,
                    "content": base_rule
                }))
            } else {
                Err(syn::Error::new(
                    expr.span(),
                    "Expected integer literal for precedence. Use #[adze::prec(123)] with a positive integer (0 to 4294967295).",
                ))
            }
        } else if let Some(attr) = self.prec_left {
            let expr: Expr = attr.parse_args()?;
            if let Expr::Lit(ExprLit {
                lit: Lit::Int(i), ..
            }) = expr
            {
                let value = i
                    .base10_parse::<u32>()
                    .map_err(|e| syn::Error::new(
                        i.span(),
                        format!("Invalid integer literal for left-associative precedence: {} (must be a valid u32)", e)
                    ))?;
                Ok(json!({
                    "type": "PREC_LEFT",
                    "value": value,
                    "content": base_rule
                }))
            } else {
                Err(syn::Error::new(
                    expr.span(),
                    "Expected integer literal for left-associative precedence. Use #[adze::prec_left(123)] with a positive integer (0 to 4294967295).",
                ))
            }
        } else if let Some(attr) = self.prec_right {
            let expr: Expr = attr.parse_args()?;
            if let Expr::Lit(ExprLit {
                lit: Lit::Int(i), ..
            }) = expr
            {
                let value = i
                    .base10_parse::<u32>()
                    .map_err(|e| syn::Error::new(
                        i.span(),
                        format!("Invalid integer literal for right-associative precedence: {} (must be a valid u32)", e)
                    ))?;
                Ok(json!({
                    "type": "PREC_RIGHT",
                    "value": value,
                    "content": base_rule
                }))
            } else {
                Err(syn::Error::new(
                    expr.span(),
                    "Expected integer literal for right-associative precedence. Use #[adze::prec_right(123)] with a positive integer (0 to 4294967295).",
                ))
            }
        } else {
            Ok(base_rule)
        }
    }
}

/// Determine if an enum variant should be inlined directly into the parent enum's CHOICE
/// instead of creating an intermediate symbol.
///
/// Inlining Rules (from ADR-0003):
/// 1. Explicit opt-out: #[adze::no_inline] → do NOT inline
/// 2. Unit variants: → do NOT inline (backward compatibility)
/// 3. Precedence attributes: #[prec], #[prec_left], #[prec_right] → do NOT inline (backward compatibility)
/// 4. Single-field non-leaf wrapper variants: → do NOT inline (preserves
///    reduce/reduce variant identity)
/// 5. Default: → inline (enables GLR conflict preservation)
fn should_inline_variant(attrs: &[Attribute], fields: &Fields) -> bool {
    // Rule 1: Check for explicit no_inline attribute
    if attrs
        .iter()
        .any(|attr| attr.path() == &syn::parse_quote!(adze::no_inline))
    {
        return false;
    }

    // Rule 2: Unit variants never inline (backward compatibility)
    if matches!(fields, Fields::Unit) {
        return false;
    }

    // Rule 3: Variants with precedence never inline (backward compatibility)
    let has_precedence = attrs.iter().any(|attr| {
        attr.path() == &syn::parse_quote!(adze::prec)
            || attr.path() == &syn::parse_quote!(adze::prec_left)
            || attr.path() == &syn::parse_quote!(adze::prec_right)
    });

    if has_precedence {
        return false;
    }

    // Rule 4: Single-field non-leaf wrapper variants preserve their own
    // symbol identity. Inlining `Choice::FromA(FromA)` and
    // `Choice::FromB(FromB)` would collapse both alternatives to the same
    // terminal before table generation can preserve the reduce/reduce cell.
    if is_single_field_non_leaf_variant(attrs, fields) {
        return false;
    }

    // Rule 5: Default - inline for GLR support
    true
}

fn is_single_field_non_leaf_variant(attrs: &[Attribute], fields: &Fields) -> bool {
    if let Fields::Unnamed(fields_unnamed) = fields
        && fields_unnamed.unnamed.len() == 1
    {
        if attrs
            .iter()
            .any(|attr| attr.path() == &syn::parse_quote!(adze::leaf))
        {
            return false;
        }

        let field = &fields_unnamed.unnamed[0];
        return !field
            .attrs
            .iter()
            .any(|attr| attr.path() == &syn::parse_quote!(adze::leaf));
    }

    false
}

fn single_field_non_leaf_target_name(attrs: &[Attribute], fields: &Fields) -> Option<String> {
    let Fields::Unnamed(fields_unnamed) = fields else {
        return None;
    };
    if fields_unnamed.unnamed.len() != 1 {
        return None;
    }

    let field = &fields_unnamed.unnamed[0];
    if attrs
        .iter()
        .any(|attr| attr.path() == &syn::parse_quote!(adze::leaf))
        || field
            .attrs
            .iter()
            .any(|attr| attr.path() == &syn::parse_quote!(adze::leaf))
    {
        return None;
    }

    let mut skip_over = HashSet::new();
    skip_over.insert("Spanned");
    skip_over.insert("Box");

    match filter_inner_type(&field.ty, &skip_over) {
        Type::Path(p) if p.path.segments.len() == 1 => Some(p.path.segments[0].ident.to_string()),
        _ => None,
    }
}

fn gen_struct_or_variant(
    path: String,
    attrs: Vec<Attribute>,
    fields: Fields,
    out: &mut Map<String, Value>,
    word_rule: &mut Option<String>,
    inline: bool, // If true, return rule instead of inserting into out
) -> ToolResult<Option<Value>> {
    // Check if this is a single-leaf variant (enum variant with a single leaf field)
    if let Fields::Unnamed(fields_unnamed) = &fields
        && fields_unnamed.unnamed.len() == 1
    {
        let field = &fields_unnamed.unnamed[0];
        if let Some(leaf_attrs) = field
            .attrs
            .iter()
            .find(|attr| attr.path() == &syn::parse_quote!(adze::leaf))
        {
            // This is a single-leaf variant - return the token directly
            let params = leaf_attrs
                .parse_args_with(Punctuated::<NameValueExpr, Token![,]>::parse_terminated)
                .ok();
            if let Some(params) = params {
                if let Some(pattern) = params
                    .iter()
                    .find(|param| param.path == "pattern")
                    .map(|p| p.expr.clone())
                {
                    if let Expr::Lit(ExprLit {
                        lit: Lit::Str(s), ..
                    }) = pattern
                    {
                        // For single-leaf variants with pattern
                        let pattern_rule = json!({
                            "type": "PATTERN",
                            "value": s.value(),
                        });

                        if inline {
                            // Inline: return the pattern directly
                            return Ok(Some(pattern_rule));
                        } else {
                            // Not inline: create named rule
                            out.insert(path, pattern_rule);
                            return Ok(None);
                        }
                    }
                } else if let Some(Expr::Lit(ExprLit {
                    lit: Lit::Str(s), ..
                })) = params
                    .iter()
                    .find(|param| param.path == "text")
                    .map(|p| p.expr.clone())
                {
                    // For single-leaf variants with text
                    let string_rule = json!({
                        "type": "STRING",
                        "value": s.value(),
                    });

                    if inline {
                        // Inline: return the string directly
                        return Ok(Some(string_rule));
                    } else {
                        // Not inline: create named rule
                        out.insert(path, string_rule);
                        return Ok(None);
                    }
                }
            }
        }
    }

    // Check for precedence attributes early to determine if we should inline operators
    let has_precedence = attrs.iter().any(|attr| {
        attr.path() == &syn::parse_quote!(adze::prec)
            || attr.path() == &syn::parse_quote!(adze::prec_left)
            || attr.path() == &syn::parse_quote!(adze::prec_right)
    });

    fn gen_field_optional(
        path: &str,
        field: &Field,
        word_rule: &mut Option<String>,
        out: &mut Map<String, Value>,
        ident_str: String,
    ) -> ToolResult<Value> {
        let (field_contents, is_option) = gen_field(
            format!("{path}_{ident_str}"),
            field.ty.clone(),
            field.attrs.clone(),
            word_rule,
            out,
        )?;

        let core = json!({
            "type": "FIELD",
            "name": ident_str,
            "content": field_contents
        });

        Ok(if is_option {
            json!({
                "type": "CHOICE",
                "members": [
                    {
                        "type": "BLANK"
                    },
                    core
                ]
            })
        } else {
            core
        })
    }

    let children = fields
        .iter()
        .enumerate()
        .filter_map(|(i, field)| {
            if field
                .attrs
                .iter()
                .any(|attr| attr.path() == &syn::parse_quote!(adze::skip))
            {
                None
            } else {
                // Check if this is a leaf field with text parameter (operator)
                let is_operator_field = field.attrs.iter().any(|attr| {
                    if attr.path() == &syn::parse_quote!(adze::leaf) {
                        if let Ok(params) = attr.parse_args_with(
                            Punctuated::<NameValueExpr, Token![,]>::parse_terminated,
                        ) {
                            params.iter().any(|param| param.path == "text")
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                });

                let field_name = field
                    .attrs
                    .iter()
                    .find(|attr| attr.path() == &syn::parse_quote!(adze::field))
                    .and_then(|attr| attr.parse_args::<syn::LitStr>().ok().map(|lit| lit.value()));

                // Try to inline operator fields for precedence (only if this variant has precedence)
                let inlined_operator = if is_operator_field && has_precedence {
                    field
                        .attrs
                        .iter()
                        .find(|attr| attr.path() == &syn::parse_quote!(adze::leaf))
                        .and_then(|leaf_attr| {
                            leaf_attr
                                .parse_args_with(
                                    Punctuated::<NameValueExpr, Token![,]>::parse_terminated,
                                )
                                .ok()
                                .and_then(|params| {
                                    params
                                        .iter()
                                        .find(|param| param.path == "text")
                                        .and_then(|p| {
                                            if let Expr::Lit(ExprLit {
                                                lit: Lit::Str(s), ..
                                            }) = &p.expr
                                            {
                                                // Only inline simple operators (single chars or simple symbols)
                                                let text_val = s.value();
                                                if text_val.len() <= 2
                                                    || text_val == "&&"
                                                    || text_val == "||"
                                                    || text_val == "=="
                                                    || text_val == "!="
                                                {
                                                    Some(json!({
                                                        "type": "STRING",
                                                        "value": text_val
                                                    }))
                                                } else {
                                                    None
                                                }
                                            } else {
                                                None
                                            }
                                        })
                                })
                        })
                } else {
                    None
                };

                if let Some(operator) = inlined_operator {
                    if let Some(field_name) = field_name {
                        Some(json!({
                            "type": "FIELD",
                            "name": field_name,
                            "content": operator
                        }))
                    } else {
                        Some(operator)
                    }
                } else {
                    let ident_str = field_name.unwrap_or_else(|| {
                        field
                            .ident
                            .as_ref()
                            .map(|v| v.to_string())
                            .unwrap_or_else(|| {
                                // Generate a deterministic name based on the path and field index
                                // This ensures consistent naming across builds
                                format!("{path}_{i}")
                            })
                    });

                    let ident_str_clone = ident_str.clone();
                    match gen_field_optional(&path, field, word_rule, out, ident_str) {
                        Ok(result) => Some(result),
                        Err(e) => {
                            debug_trace!("Error generating field {}: {:?}", ident_str_clone, e);
                            None // Skip this field on error
                        }
                    }
                }
            }
        })
        .collect::<Vec<Value>>();

    let base_rule = match fields {
        Fields::Unit => {
            let dummy_field = Field {
                attrs: attrs.clone(),
                vis: Visibility::Inherited,
                mutability: FieldMutability::None,
                ident: None,
                colon_token: None,
                ty: Type::Tuple(TypeTuple {
                    paren_token: Default::default(),
                    elems: Punctuated::new(),
                }),
            };
            gen_field_optional(&path, &dummy_field, word_rule, out, "unit".to_owned())?
        }
        _ => {
            // If all children are optional, we need at least one to be present
            // to avoid the EmptyString error
            if children.is_empty() {
                return Err(ToolError::StructHasNoFields { name: path });
            } else if children.len() == 1 {
                // Single field - use it directly
                children.into_iter().next().unwrap()
            } else {
                json!({
                    "type": "SEQ",
                    "members": children
                })
            }
        }
    };

    // Check if this rule could be empty (single optional field)
    let potentially_empty = match &base_rule {
        Value::Object(obj) => {
            obj.get("type").and_then(|t| t.as_str()) == Some("FIELD")
                && obj
                    .get("content")
                    .and_then(|c| c.as_object())
                    .map(|c| {
                        c.get("type").and_then(|t| t.as_str()) == Some("CHOICE")
                            && c.get("members")
                                .and_then(|m| m.as_array())
                                .map(|m| {
                                    m.iter().any(|member| {
                                        member
                                            .as_object()
                                            .and_then(|o| o.get("type"))
                                            .and_then(|t| t.as_str())
                                            == Some("BLANK")
                                    })
                                })
                                .unwrap_or(false)
                    })
                    .unwrap_or(false)
        }
        _ => false,
    };

    if potentially_empty {
        debug_trace!(
            "Warning: Rule '{}' can match empty input. Tree-sitter requires all named rules to match at least one character. Consider adding at least one required field or using 'non_empty = true' for Vec fields.",
            path
        );
    }

    let rule = Precs::new(&attrs)
        .apply(base_rule)
        .map_err(|e| ToolError::SynError { syn_error: e })?;

    // If inlining, return the rule directly instead of inserting into map
    if inline {
        Ok(Some(rule))
    } else {
        out.insert(path, rule);
        Ok(None) // Return None for non-single-leaf variants
    }
}

pub fn generate_grammar(module: &ItemMod) -> ToolResult<Value> {
    let mut rules_map = Map::new();
    // for some reason, source_file must be the first key for things to work
    // We'll insert it after we find the root type

    let mut extras_list = vec![];
    let mut externals_list = vec![];
    let mut conflicts_list = vec![];

    let grammar_name = module
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
                    Some(s.value())
                } else {
                    panic!("Expected string literal for grammar name");
                }
            } else {
                None
            }
        })
        .expect("Each grammar must have a name");

    let (_, contents) = module.content.as_ref().unwrap();

    let root_type = contents
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
        .expect("Each parser must have the root type annotated with `#[adze::language]`")
        .to_string();

    // Insert source_file rule that references the root type
    rules_map.insert(
        "source_file".to_string(),
        json!({
            "type": "SYMBOL",
            "name": root_type.to_string()
        }),
    );

    // Optionally locate the rule annotated with `#[adze::word]`.
    let mut word_rule = None;
    for c in contents.iter() {
        let (symbol, attrs) = match c {
            Item::Enum(e) => {
                let mut members: Vec<Value> = vec![];
                let mut wrapper_conflict_set = vec![];

                for v in e.variants.iter() {
                    let variant_path = format!("{}_{}", e.ident, v.ident);

                    // Determine if this variant should be inlined (ADR-0003)
                    let inline = should_inline_variant(&v.attrs, &v.fields);
                    if !inline && is_single_field_non_leaf_variant(&v.attrs, &v.fields) {
                        wrapper_conflict_set.push(
                            single_field_non_leaf_target_name(&v.attrs, &v.fields)
                                .unwrap_or_else(|| variant_path.clone()),
                        );
                    }

                    // Generate the variant rule
                    let inline_rule = gen_struct_or_variant(
                        variant_path.clone(),
                        v.attrs.clone(),
                        v.fields.clone(),
                        &mut rules_map,
                        &mut word_rule,
                        inline, // Pass inline flag
                    )?;

                    // Add to CHOICE members
                    let variant_member = if let Some(rule) = inline_rule {
                        // Variant was inlined - use the rule directly
                        rule
                    } else {
                        // Variant created intermediate symbol - reference it
                        json!({
                            "type": "SYMBOL",
                            "name": variant_path.clone()
                        })
                    };

                    members.push(variant_member);
                }

                if wrapper_conflict_set.len() > 1 {
                    conflicts_list.push(json!(wrapper_conflict_set));
                }

                // For precedence to work correctly with the LR algorithm,
                // we need the CHOICE to be visible. This allows the parser to see
                // the operators directly and generate proper shift/reduce conflicts.

                let rule = json!({
                    "type": "CHOICE",
                    "members": members
                });

                // Insert the CHOICE rule directly (no hidden indirection)
                rules_map.insert(e.ident.to_string(), rule);

                (e.ident.to_string(), e.attrs.clone())
            }

            Item::Struct(s) => {
                // Check if this is an external token first
                let is_external = s
                    .attrs
                    .iter()
                    .any(|a| a.path() == &syn::parse_quote!(adze::external));

                // Check if this is an extra token
                let is_extra = s
                    .attrs
                    .iter()
                    .any(|a| a.path() == &syn::parse_quote!(adze::extra));

                // Check if this is the word token
                let is_word = s
                    .attrs
                    .iter()
                    .any(|a| a.path() == &syn::parse_quote!(adze::word));

                if is_word {
                    if word_rule.is_some() {
                        Err(ToolError::MultipleWordRules)?;
                    }
                    word_rule = Some(s.ident.to_string());
                }

                // Generate rules for non-external structs AND extra structs (even if they're not referenced)
                if !is_external || is_extra {
                    gen_struct_or_variant(
                        s.ident.to_string(),
                        s.attrs.clone(),
                        s.fields.clone(),
                        &mut rules_map,
                        &mut word_rule,
                        false, // Structs are never inlined (only enum variants can be inlined)
                    )?;
                }

                (s.ident.to_string(), s.attrs.clone())
            }

            _ => continue,
        };

        if attrs
            .iter()
            .any(|a| a.path() == &syn::parse_quote!(adze::extra))
        {
            // For extras, we want to reference the generated rule directly
            // The Whitespace struct generates a rule like "Whitespace" which contains the pattern
            extras_list.push(json!({
                "type": "SYMBOL",
                "name": symbol
            }));
        }

        if attrs
            .iter()
            .any(|a| a.path() == &syn::parse_quote!(adze::external))
        {
            externals_list.push(json!({
                "type": "SYMBOL",
                "name": symbol
            }));
        }
    }

    // source_file rule already inserted above - don't overwrite it!

    // Add all external tokens to the extras list as well
    // This makes them behave like implicit tokens that can appear anywhere
    for external in &externals_list {
        if let Some(name) = external.get("name") {
            extras_list.push(json!({
                "type": "SYMBOL",
                "name": name
            }));
        }
    }

    let mut grammar = json!({
        "name": grammar_name,
        "word": word_rule,
        "rules": rules_map,
        "extras": extras_list
    });

    // Only include externals if there are any
    if !externals_list.is_empty() {
        grammar
            .as_object_mut()
            .unwrap()
            .insert("externals".to_string(), json!(externals_list));
    }

    // Only include conflicts if generated wrapper variants need them.
    if !conflicts_list.is_empty() {
        grammar
            .as_object_mut()
            .unwrap()
            .insert("conflicts".to_string(), json!(conflicts_list));
    }

    Ok(grammar)
}
