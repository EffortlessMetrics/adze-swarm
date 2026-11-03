#![cfg_attr(feature = "strict_docs", allow(missing_docs))]
//! Typed CST wrapper code generation.
//!
//! This module is an alpha generator target for Rust-native typed CST handles.
//! It emits wrappers over `AdzeDocument` node IDs and checked canonical field
//! projection. The syntax module is co-generated next to its native `LANGUAGE`
//! object; casts validate that immutable language binding before local IDs.

use adze_ir::{Grammar, Symbol, SymbolId, TokenPattern};
use proc_macro2::{Ident, Span, TokenStream};
use quote::quote;
use std::collections::{BTreeMap, BTreeSet};

/// Generate typed CST wrapper code from grammar metadata.
pub struct TypedCstGenerator<'a> {
    grammar: &'a Grammar,
}

#[derive(Clone, Debug)]
struct WrapperSpec {
    ident: Ident,
    kind_name: String,
}

#[derive(Clone, Debug)]
struct FieldAccessor {
    method_ident: Ident,
    field_name: String,
    target_ident: Ident,
    target_id: SymbolId,
    multiple: bool,
}

impl<'a> TypedCstGenerator<'a> {
    /// Create a typed CST generator for one grammar.
    pub fn new(grammar: &'a Grammar) -> Self {
        Self { grammar }
    }

    /// Generate a `syntax` module containing typed CST wrappers.
    #[must_use]
    pub fn generate(&self) -> TokenStream {
        // Single derivation: field facts come from the canonical schema, and
        // wrapper retention follows the same facts (no locally re-derived
        // cardinality or target walk).
        let schema = crate::field_schema::field_semantics_for_grammar(self.grammar);
        let specs = self.wrapper_specs(&schema);
        let accessors = self.field_accessors(&specs, &schema);
        let root_fn = self.root_constructor(&specs);

        let wrappers = specs.iter().map(|(symbol_id, spec)| {
            let symbol_id = symbol_id.0;
            let ident = &spec.ident;
            let required_fields: BTreeSet<_> = accessors.get(&spec.ident.to_string())
                .into_iter().flatten().map(|accessor| accessor.field_name.as_str()).collect();
            // Leaf handles still validate the checked view on cast, but
            // only owners with generated accessors need to retain it.
            let retains_projection = !required_fields.is_empty();
            let projection_field = retains_projection.then(|| quote! {
                projection: ::adze::document::FieldProjectionView<'doc>,
            });
            let checked_projection = if retains_projection {
                quote! { let projection = document.field_projection_view(id, &[#(#required_fields),*])?; }
            } else {
                quote! { let _ = document.field_projection_view(id, &[])?; }
            };
            let cast_fields = if retains_projection {
                quote! { document, id, projection }
            } else {
                quote! { document, id }
            };
            let wrapper_accessors = accessors
                .get(&spec.ident.to_string())
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .map(|accessor| {
                    let method_ident = accessor.method_ident;
                    let field_name = accessor.field_name;
                    let target_ident = accessor.target_ident;
                    let target_id = accessor.target_id.0;
                    let target_projection = accessors
                        .get(&target_ident.to_string())
                        .is_some_and(|fields| !fields.is_empty())
                        .then(|| quote! { projection: target.view(), });
                    let targets = quote! {
                        self.projection.nodes(#field_name)
                            .filter(|target| target.grammar_id().map(|id| id.0) == Some(#target_id))
                            .map(|target| #target_ident {
                                document: self.document, id: target.node_id(), #target_projection
                            })
                    };
                    if accessor.multiple {
                        quote! {
                            pub fn #method_ident(&self) -> Vec<#target_ident<'doc>> {
                                #targets.collect()
                            }
                        }
                    } else {
                        quote! {
                            pub fn #method_ident(&self) -> Option<#target_ident<'doc>> {
                                #targets.next()
                            }
                        }
                    }
                });

            quote! {
                #[derive(Clone, Copy, Debug)]
                pub struct #ident<'doc> {
                    document: &'doc AdzeDocument,
                    id: NodeId,
                    #projection_field
                }

                impl<'doc> #ident<'doc> {
                    pub fn cast(document: &'doc AdzeDocument, id: NodeId) -> Option<Self> {
                        Self::try_cast(document, id).ok().flatten()
                    }

                    pub fn try_cast(document: &'doc AdzeDocument, id: NodeId) -> Result<Option<Self>, FieldProjectionError> {
                        if !node_has_kind(document, id, #symbol_id) { return Ok(None); }
                        #checked_projection
                        Ok(Some(Self { #cast_fields }))
                    }

                    #(#wrapper_accessors)*
                }

                impl<'doc> SyntaxNode<'doc> for #ident<'doc> {
                    fn document(&self) -> &'doc AdzeDocument {
                        self.document
                    }

                    fn node_id(&self) -> NodeId {
                        self.id
                    }
                }
            }
        });

        quote! {
            pub mod syntax {
                use ::adze::document::{AdzeDocument, NodeId, SyntaxNode, FieldProjectionError};

                #root_fn

                #(#wrappers)*

                fn node_has_kind(document: &AdzeDocument, id: NodeId, expected: u16) -> bool {
                    ::adze::__private::document_matches_language(document, &super::LANGUAGE)
                        && document.tree().node(id).is_some_and(|node| node.grammar_id().0 == expected)
                }
            }
        }
    }

    fn root_constructor(&self, specs: &BTreeMap<SymbolId, WrapperSpec>) -> TokenStream {
        let source_file_id = self.grammar.find_symbol_by_name("source_file");
        let Some(spec) = source_file_id.and_then(|id| specs.get(&id)) else {
            return quote! {};
        };

        let root_ident = &spec.ident;
        quote! {
            pub fn source_file(document: &AdzeDocument) -> Option<#root_ident<'_>> {
                #root_ident::cast(document, document.tree().root_id())
            }
        }
    }

    fn wrapper_specs(
        &self,
        schema: &BTreeMap<SymbolId, BTreeMap<String, crate::field_schema::FieldSemantics>>,
    ) -> BTreeMap<SymbolId, WrapperSpec> {
        let mut specs = BTreeMap::new();
        let mut used_idents = BTreeSet::new();
        let field_target_terminals = schema_field_terminals(schema);

        for (symbol_id, name) in &self.grammar.rule_names {
            if name.starts_with('_') {
                continue;
            }

            specs.insert(
                *symbol_id,
                WrapperSpec {
                    ident: unique_type_ident(name, "SyntaxNode", *symbol_id, &mut used_idents),
                    kind_name: name.clone(),
                },
            );
        }

        for (symbol_id, token) in &self.grammar.tokens {
            if token.name.starts_with('_') && !field_target_terminals.contains(symbol_id) {
                continue;
            }

            let kind_name = match &token.pattern {
                TokenPattern::String(text) if token.name == format!("\"{text}\"") => {
                    token.name.clone()
                }
                TokenPattern::String(text) => text.clone(),
                TokenPattern::Regex(_) => token.name.clone(),
            };

            specs.insert(
                *symbol_id,
                WrapperSpec {
                    ident: unique_type_ident(
                        &format!("{}_token", token.name),
                        "Token",
                        *symbol_id,
                        &mut used_idents,
                    ),
                    kind_name,
                },
            );
        }

        specs
    }

    fn field_accessors(
        &self,
        specs: &BTreeMap<SymbolId, WrapperSpec>,
        schema: &BTreeMap<SymbolId, BTreeMap<String, crate::field_schema::FieldSemantics>>,
    ) -> BTreeMap<String, Vec<FieldAccessor>> {
        let mut by_wrapper: BTreeMap<String, Vec<FieldAccessor>> = BTreeMap::new();
        let mut used_method_names: BTreeSet<(String, String)> = BTreeSet::new();

        for (owner, fields) in schema {
            let Some(owner_spec) = specs.get(owner) else {
                continue;
            };
            let owner_name = owner_spec.ident.to_string();
            for (field_name, semantics) in fields {
                // Map every admitted target kind to its wrapper. Targets
                // without a wrapper keep the historical skip behavior.
                let mut targets: Vec<(Ident, String, SymbolId)> = Vec::new();
                for target in &semantics.targets {
                    let (Some(target_ident), Some(kind_name)) = (
                        symbol_wrapper_ident(target, specs),
                        wrapper_kind_name(target, specs),
                    ) else {
                        continue;
                    };
                    let id = match target {
                        Symbol::Terminal(id) | Symbol::NonTerminal(id) | Symbol::External(id) => {
                            *id
                        }
                        _ => continue,
                    };
                    targets.push((target_ident, kind_name, id));
                }
                if targets.is_empty() {
                    continue;
                }
                if targets.len() == 1 {
                    // Single admitted kind keeps the historical method name.
                    let (target_ident, _, target_id) =
                        targets.into_iter().next().expect("one target");
                    let method_ident = unique_method_name(
                        &owner_name,
                        &method_ident(field_name, "field").to_string(),
                        &mut used_method_names,
                    );
                    by_wrapper
                        .entry(owner_name.clone())
                        .or_default()
                        .push(FieldAccessor {
                            method_ident,
                            field_name: field_name.clone(),
                            target_ident,
                            target_id,
                            multiple: semantics.multiple,
                        });
                    continue;
                }
                // Multiple admitted kinds: one accessor per kind so every
                // alternative is reachable without a first-type shortcut.
                for (target_ident, kind_name, target_id) in targets {
                    let method_ident = unique_method_name(
                        &owner_name,
                        &method_ident(&format!("{field_name}_{kind_name}"), "field").to_string(),
                        &mut used_method_names,
                    );
                    by_wrapper
                        .entry(owner_name.clone())
                        .or_default()
                        .push(FieldAccessor {
                            method_ident,
                            field_name: field_name.clone(),
                            target_ident,
                            target_id,
                            multiple: semantics.multiple,
                        });
                }
            }
        }

        by_wrapper
    }
}

/// Display name used for a target kind, matching the wrapper cast check.
fn wrapper_kind_name(symbol: &Symbol, specs: &BTreeMap<SymbolId, WrapperSpec>) -> Option<String> {
    match symbol {
        Symbol::Terminal(id) | Symbol::NonTerminal(id) | Symbol::External(id) => {
            specs.get(id).map(|spec| spec.kind_name.clone())
        }
        _ => None,
    }
}

/// Claim a method name for one wrapper, disambiguating sanitization
/// collisions deterministically.
fn unique_method_name(owner: &str, base: &str, used: &mut BTreeSet<(String, String)>) -> Ident {
    if used.insert((owner.to_string(), base.to_string())) {
        return Ident::new(base, Span::call_site());
    }
    let mut counter = 0u32;
    loop {
        counter += 1;
        let candidate = format!("{base}_{counter}");
        if used.insert((owner.to_string(), candidate.clone())) {
            return Ident::new(&candidate, Span::call_site());
        }
    }
}

/// Terminal symbols admitted as field targets by the canonical schema.
///
/// Single derivation for hidden-token wrapper retention: a `_`-hidden token
/// keeps its wrapper exactly when the schema names it as a field target, so
/// retention agrees with node-types and accessor generation even after
/// normalization replaces wrappers with helper non-terminals.
fn schema_field_terminals(
    schema: &BTreeMap<SymbolId, BTreeMap<String, crate::field_schema::FieldSemantics>>,
) -> BTreeSet<SymbolId> {
    let mut terminals = BTreeSet::new();
    for fields in schema.values() {
        for semantics in fields.values() {
            for target in &semantics.targets {
                if let Symbol::Terminal(id) = target {
                    terminals.insert(*id);
                }
            }
        }
    }
    terminals
}

fn symbol_wrapper_ident(symbol: &Symbol, specs: &BTreeMap<SymbolId, WrapperSpec>) -> Option<Ident> {
    // Inputs are canonical schema targets, which are leaves by contract:
    // helper wrappers are resolved away before they reach this mapping, so
    // composite shapes (and the old first-choice shortcut) have no mapping.
    match symbol {
        Symbol::Terminal(id) | Symbol::NonTerminal(id) | Symbol::External(id) => {
            specs.get(id).map(|spec| spec.ident.clone())
        }
        Symbol::Optional(_)
        | Symbol::Repeat(_)
        | Symbol::RepeatOne(_)
        | Symbol::Choice(_)
        | Symbol::Sequence(_)
        | Symbol::Epsilon => None,
    }
}

fn unique_type_ident(
    name: &str,
    fallback: &str,
    symbol_id: SymbolId,
    used: &mut BTreeSet<String>,
) -> Ident {
    let base = pascal_case_ident(name, fallback);
    let mut candidate = base.clone();
    if !used.insert(candidate.clone()) {
        candidate = format!("{base}Symbol{}", symbol_id.0);
        let mut collision = 1usize;
        while !used.insert(candidate.clone()) {
            candidate = format!("{base}Symbol{}_{collision}", symbol_id.0);
            collision += 1;
        }
    }

    Ident::new(&candidate, Span::call_site())
}

fn method_ident(name: &str, fallback: &str) -> Ident {
    Ident::new(&snake_case_ident(name, fallback), Span::call_site())
}

fn pascal_case_ident(name: &str, fallback: &str) -> String {
    let mut out = String::new();
    let mut at_word_start = true;

    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            if at_word_start {
                out.push(ch.to_ascii_uppercase());
                at_word_start = false;
            } else {
                out.push(ch);
            }
        } else {
            at_word_start = true;
        }
    }

    if out.is_empty() {
        out.push_str(fallback);
    }

    if out
        .chars()
        .next()
        .map(|ch| ch.is_ascii_digit())
        .unwrap_or(true)
    {
        out.insert(0, 'N');
    }

    if is_reserved_ident(&out) {
        out.push_str("Node");
    }

    out
}

fn snake_case_ident(name: &str, fallback: &str) -> String {
    let mut out = String::new();
    let mut previous_was_separator = true;

    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            if ch.is_ascii_uppercase() && !previous_was_separator && !out.ends_with('_') {
                out.push('_');
            }
            out.push(ch.to_ascii_lowercase());
            previous_was_separator = false;
        } else if !previous_was_separator && !out.is_empty() {
            out.push('_');
            previous_was_separator = true;
        }
    }

    while out.ends_with('_') {
        out.pop();
    }

    if out.is_empty() {
        out.push_str(fallback);
    }

    if out
        .chars()
        .next()
        .map(|ch| ch.is_ascii_digit())
        .unwrap_or(true)
    {
        out.insert(0, '_');
    }

    if is_reserved_ident(&out) {
        out.push('_');
    }

    out
}

fn is_reserved_ident(ident: &str) -> bool {
    matches!(
        ident,
        "as" | "break"
            | "const"
            | "continue"
            | "crate"
            | "else"
            | "enum"
            | "extern"
            | "false"
            | "fn"
            | "for"
            | "if"
            | "impl"
            | "in"
            | "let"
            | "loop"
            | "match"
            | "mod"
            | "move"
            | "mut"
            | "pub"
            | "ref"
            | "return"
            | "self"
            | "Self"
            | "static"
            | "struct"
            | "super"
            | "trait"
            | "true"
            | "type"
            | "unsafe"
            | "use"
            | "where"
            | "while"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use adze_ir::{FieldId, ProductionId, Rule, Token};

    #[test]
    fn test_typed_cst_generator_emits_valid_arithmetic_wrapper_module() {
        let grammar = arithmetic_grammar();
        let generated = TypedCstGenerator::new(&grammar).generate();

        syn::parse2::<syn::File>(generated.clone())
            .expect("typed CST generator should emit valid Rust syntax");

        let code = generated.to_string();
        assert!(code.contains("pub mod syntax"));
        assert!(code.contains("pub struct SourceFile"));
        assert!(code.contains("pub struct Expression"));
        assert!(code.contains("pub struct MinusToken"));
        assert!(code.contains("pub struct NumberToken"));
        assert!(code.contains("pub fn source_file"));
    }

    #[test]
    fn test_typed_cst_generator_uniquifies_colliding_wrapper_names() {
        let grammar = duplicate_punctuation_token_grammar();
        let generated = TypedCstGenerator::new(&grammar).generate();

        syn::parse2::<syn::File>(generated.clone())
            .expect("duplicate token wrapper names should still emit valid Rust syntax");

        let code = generated.to_string();
        assert_eq!(code.matches("pub struct Token <").count(), 1);
        assert!(code.contains("pub struct TokenSymbol1"));
        assert!(code.contains("node_has_kind (document , id , 0u16)"));
        assert!(code.contains("node_has_kind (document , id , 1u16)"));
    }

    #[test]
    fn test_typed_cst_generator_projects_fields_through_checked_view() {
        let grammar = arithmetic_grammar();
        let generated = TypedCstGenerator::new(&grammar).generate().to_string();

        assert!(generated.contains("pub fn left"));
        assert!(generated.contains("pub fn operator"));
        assert!(generated.contains("pub fn right"));
        assert!(generated.contains("nodes (\"left\")"));
        assert!(generated.contains("nodes (\"operator\")"));
        assert!(generated.contains("nodes (\"right\")"));
        assert!(generated.contains("Expression { document : self . document"));
        assert!(generated.contains("MinusToken { document : self . document"));
    }

    #[test]
    fn test_typed_cst_generator_keeps_hidden_tokens_when_they_are_field_targets() {
        let grammar = hidden_field_token_grammar();
        let generated = TypedCstGenerator::new(&grammar).generate().to_string();

        assert!(generated.contains("pub struct NumberToken"));
        assert!(generated.contains("pub fn left"));
        assert!(generated.contains("nodes (\"left\")"));
        assert!(generated.contains("NumberToken { document : self . document"));
    }

    #[test]
    fn test_typed_cst_generator_casts_quoted_inline_string_field_targets() {
        let grammar = quoted_inline_string_field_grammar();
        let generated = TypedCstGenerator::new(&grammar).generate().to_string();

        assert!(generated.contains("pub fn operator"));
        assert!(generated.contains("nodes (\"operator\")"));
        assert!(generated.contains("Token { document : self . document"));
        assert!(generated.contains("node_has_kind (document , id , 0u16)"));
    }

    fn arithmetic_grammar() -> Grammar {
        let mut grammar = Grammar::new("arithmetic".to_string());

        let number = SymbolId(0);
        let minus = SymbolId(1);
        let source_file = SymbolId(2);
        let expression = SymbolId(3);

        grammar.tokens.insert(
            number,
            Token {
                name: "number".to_string(),
                pattern: TokenPattern::Regex(r"\d+".to_string()),
                fragile: false,
            },
        );
        grammar.tokens.insert(
            minus,
            Token {
                name: "minus".to_string(),
                pattern: TokenPattern::String("-".to_string()),
                fragile: false,
            },
        );

        grammar
            .rule_names
            .insert(source_file, "source_file".to_string());
        grammar
            .rule_names
            .insert(expression, "expression".to_string());

        grammar.fields.insert(FieldId(0), "left".to_string());
        grammar.fields.insert(FieldId(1), "operator".to_string());
        grammar.fields.insert(FieldId(2), "right".to_string());

        grammar.add_rule(Rule {
            lhs: source_file,
            rhs: vec![Symbol::NonTerminal(expression)],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(0),
        });
        grammar.add_rule(Rule {
            lhs: expression,
            rhs: vec![
                Symbol::NonTerminal(expression),
                Symbol::Terminal(minus),
                Symbol::NonTerminal(expression),
            ],
            precedence: None,
            associativity: None,
            fields: vec![(FieldId(0), 0), (FieldId(1), 1), (FieldId(2), 2)],
            production_id: ProductionId(1),
        });
        grammar.add_rule(Rule {
            lhs: expression,
            rhs: vec![Symbol::Terminal(number)],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(2),
        });

        grammar
    }

    fn repeat_grammar() -> Grammar {
        let mut grammar = Grammar::new("repeat_fixture".to_string());

        let number = SymbolId(1);
        let list = SymbolId(2);
        let source_file = SymbolId(3);

        grammar.tokens.insert(
            number,
            Token {
                name: "number".to_string(),
                pattern: TokenPattern::Regex(r"\d+".to_string()),
                fragile: false,
            },
        );

        grammar
            .rule_names
            .insert(source_file, "source_file".to_string());
        grammar.rule_names.insert(list, "list".to_string());

        grammar.fields.insert(FieldId(0), "items".to_string());

        grammar.add_rule(Rule {
            lhs: source_file,
            rhs: vec![Symbol::NonTerminal(list)],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(0),
        });
        grammar.add_rule(Rule {
            lhs: list,
            rhs: vec![Symbol::Repeat(Box::new(Symbol::Terminal(number)))],
            precedence: None,
            associativity: None,
            fields: vec![(FieldId(0), 0)],
            production_id: ProductionId(1),
        });

        grammar
    }

    fn choice_field_grammar() -> Grammar {
        let mut grammar = Grammar::new("choice_fixture".to_string());

        let num = SymbolId(1);
        let word = SymbolId(2);
        let pick = SymbolId(3);
        let source_file = SymbolId(4);

        for (id, name, pattern) in [
            (num, "num", TokenPattern::Regex(r"\d+".to_string())),
            (word, "word", TokenPattern::Regex(r"[a-z]+".to_string())),
        ] {
            grammar.tokens.insert(
                id,
                Token {
                    name: name.to_string(),
                    pattern,
                    fragile: false,
                },
            );
        }

        grammar
            .rule_names
            .insert(source_file, "source_file".to_string());
        grammar.rule_names.insert(pick, "pick".to_string());

        grammar.fields.insert(FieldId(0), "value".to_string());

        grammar.add_rule(Rule {
            lhs: source_file,
            rhs: vec![Symbol::NonTerminal(pick)],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(0),
        });
        grammar.add_rule(Rule {
            lhs: pick,
            rhs: vec![Symbol::Choice(vec![
                Symbol::Terminal(num),
                Symbol::Terminal(word),
            ])],
            precedence: None,
            associativity: None,
            fields: vec![(FieldId(0), 0)],
            production_id: ProductionId(1),
        });

        grammar
    }

    #[test]
    fn test_typed_cst_repeated_field_generates_vec_accessor() {
        let grammar = repeat_grammar();
        let generated = TypedCstGenerator::new(&grammar).generate().to_string();

        // A repeated single-target field collects every child in order:
        // one Vec accessor, no single-child shortcut.
        assert!(
            generated.contains("pub fn items"),
            "items accessor must exist"
        );
        assert!(
            generated.contains("-> Vec <"),
            "repeated field must return a vector, not a single Option"
        );
        assert!(
            generated.contains("nodes (\"items\")"),
            "repeated accessor must consume the shared checked projection"
        );
    }

    #[test]
    fn test_typed_cst_multi_target_field_generates_per_target_accessors() {
        let grammar = choice_field_grammar();
        let generated = TypedCstGenerator::new(&grammar).generate().to_string();

        // Every admitted target kind gets its own accessor.
        assert!(
            generated.contains("pub fn value_num"),
            "num accessor must exist"
        );
        assert!(
            generated.contains("pub fn value_word"),
            "word accessor must exist"
        );
        assert!(
            !generated.contains("pub fn value("),
            "no bare first-type accessor may remain for a multi-target field"
        );
    }

    fn duplicate_punctuation_token_grammar() -> Grammar {
        let mut grammar = Grammar::new("duplicate_punctuation_tokens".to_string());

        let plus = SymbolId(0);
        let minus = SymbolId(1);
        let source_file = SymbolId(2);

        grammar.tokens.insert(
            plus,
            Token {
                name: "+".to_string(),
                pattern: TokenPattern::String("+".to_string()),
                fragile: false,
            },
        );
        grammar.tokens.insert(
            minus,
            Token {
                name: "-".to_string(),
                pattern: TokenPattern::String("-".to_string()),
                fragile: false,
            },
        );
        grammar
            .rule_names
            .insert(source_file, "source_file".to_string());

        grammar.add_rule(Rule {
            lhs: source_file,
            rhs: vec![Symbol::Terminal(plus)],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(0),
        });

        grammar
    }

    fn hidden_field_token_grammar() -> Grammar {
        let mut grammar = Grammar::new("hidden_field_token".to_string());

        let number = SymbolId(0);
        let source_file = SymbolId(1);
        let pair = SymbolId(2);

        grammar.tokens.insert(
            number,
            Token {
                name: "_number".to_string(),
                pattern: TokenPattern::Regex(r"\d+".to_string()),
                fragile: false,
            },
        );

        grammar
            .rule_names
            .insert(source_file, "source_file".to_string());
        grammar.rule_names.insert(pair, "pair".to_string());
        grammar.fields.insert(FieldId(0), "left".to_string());

        grammar.add_rule(Rule {
            lhs: source_file,
            rhs: vec![Symbol::NonTerminal(pair)],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(0),
        });
        grammar.add_rule(Rule {
            lhs: pair,
            rhs: vec![Symbol::Terminal(number)],
            precedence: None,
            associativity: None,
            fields: vec![(FieldId(0), 0)],
            production_id: ProductionId(1),
        });

        grammar
    }

    fn quoted_inline_string_field_grammar() -> Grammar {
        let mut grammar = Grammar::new("quoted_inline_string_field".to_string());

        let plus = SymbolId(0);
        let source_file = SymbolId(1);
        let expression = SymbolId(2);

        grammar.tokens.insert(
            plus,
            Token {
                name: "\"+\"".to_string(),
                pattern: TokenPattern::String("+".to_string()),
                fragile: false,
            },
        );

        grammar
            .rule_names
            .insert(source_file, "source_file".to_string());
        grammar
            .rule_names
            .insert(expression, "expression".to_string());
        grammar.fields.insert(FieldId(0), "operator".to_string());

        grammar.add_rule(Rule {
            lhs: source_file,
            rhs: vec![Symbol::NonTerminal(expression)],
            precedence: None,
            associativity: None,
            fields: vec![],
            production_id: ProductionId(0),
        });
        grammar.add_rule(Rule {
            lhs: expression,
            rhs: vec![Symbol::Terminal(plus)],
            precedence: None,
            associativity: None,
            fields: vec![(FieldId(0), 0)],
            production_id: ProductionId(1),
        });

        grammar
    }
}
