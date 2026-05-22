//! Property-based tests for adze-macro derive infrastructure.
//!
//! Since proc-macros cannot be called directly from integration tests, these
//! tests exercise the supporting infrastructure: token stream construction,
//! `syn`/`quote` round-trips, attribute combination properties, type analysis,
//! and identifier validity — all patterns that underpin the macro expansion
//! pipeline.

use std::collections::HashSet;

use adze_common::{filter_inner_type, try_extract_inner_type, wrap_leaf_type};
use proptest::prelude::*;
use quote::{ToTokens, format_ident, quote};
use syn::{Attribute, Ident, ItemEnum, ItemMod, ItemStruct, Type, parse_quote, parse_str};

// ── Strategies ──────────────────────────────────────────────────────────────

/// Generate valid Rust identifiers that are not keywords (including 2024 edition).
fn ident_strategy() -> impl Strategy<Value = String> {
    "[a-z][a-z0-9_]{1,12}".prop_filter("no keywords", |s| {
        !matches!(
            s.as_str(),
            "type"
                | "fn"
                | "let"
                | "mut"
                | "ref"
                | "pub"
                | "mod"
                | "use"
                | "self"
                | "super"
                | "crate"
                | "struct"
                | "enum"
                | "impl"
                | "trait"
                | "where"
                | "for"
                | "loop"
                | "while"
                | "if"
                | "else"
                | "match"
                | "return"
                | "break"
                | "continue"
                | "as"
                | "in"
                | "move"
                | "box"
                | "dyn"
                | "async"
                | "await"
                | "try"
                | "yield"
                | "macro"
                | "const"
                | "static"
                | "unsafe"
                | "extern"
                | "do"
                | "gen"
                | "abstract"
                | "become"
                | "final"
                | "override"
                | "priv"
                | "typeof"
                | "unsized"
                | "virtual"
                | "true"
                | "false"
        )
    })
}

/// Generate a PascalCase type name from an identifier base.
fn type_name_strategy() -> impl Strategy<Value = String> {
    ident_strategy().prop_map(|s| {
        let mut chars = s.chars();
        match chars.next() {
            Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
            None => String::from("T"),
        }
    })
}

/// Pick an adze attribute name.
fn attr_name_strategy() -> impl Strategy<Value = &'static str> {
    prop_oneof![
        Just("language"),
        Just("extra"),
        Just("leaf"),
        Just("skip"),
        Just("prec"),
        Just("prec_left"),
        Just("prec_right"),
        Just("delimited"),
        Just("repeat"),
        Just("external"),
        Just("word"),
    ]
}

// ── Helpers ─────────────────────────────────────────────────────────────────

fn ty(s: &str) -> Type {
    parse_str::<Type>(s).unwrap()
}

fn ts(t: &Type) -> String {
    t.to_token_stream().to_string()
}

fn is_adze_attr(attr: &Attribute, name: &str) -> bool {
    let segs: Vec<_> = attr.path().segments.iter().collect();
    segs.len() == 2 && segs[0].ident == "adze" && segs[1].ident == name
}

fn skip_set<'a>(names: &'a [&'a str]) -> HashSet<&'a str> {
    names.iter().copied().collect()
}

// ═══════════════════════════════════════════════════════════════════════════
// 1. Identifier validity properties (10 tests)
// ═══════════════════════════════════════════════════════════════════════════

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Generated identifiers can be parsed by syn as valid Ident tokens.
    #[test]
    fn ident_parses_as_syn_ident(name in ident_strategy()) {
        let ident: Ident = format_ident!("{}", name);
        prop_assert_eq!(ident.to_string(), name);
    }

    /// Identifiers remain unchanged through a quote → parse round-trip.
    #[test]
    fn ident_quote_roundtrip(name in ident_strategy()) {
        let ident = format_ident!("{}", name);
        let tokens = quote! { #ident };
        let parsed: Ident = syn::parse2(tokens).unwrap();
        prop_assert_eq!(parsed.to_string(), name);
    }

    /// Two distinct identifiers never produce identical token streams.
    #[test]
    fn distinct_idents_distinct_tokens(
        a in ident_strategy(),
        b in ident_strategy().prop_filter("different", |b| b != "a_placeholder"),
    ) {
        prop_assume!(a != b);
        let ia = format_ident!("{}", a);
        let ib = format_ident!("{}", b);
        prop_assert_ne!(ia.to_string(), ib.to_string());
    }

    /// Generated identifiers always start with a lowercase letter.
    #[test]
    fn ident_starts_lowercase(name in ident_strategy()) {
        prop_assert!(name.starts_with(|c: char| c.is_ascii_lowercase()));
    }

    /// Generated identifiers only contain ASCII alphanumeric or underscore.
    #[test]
    fn ident_chars_valid(name in ident_strategy()) {
        prop_assert!(name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'));
    }

    /// Identifiers have length between 2 and 13 inclusive.
    #[test]
    fn ident_length_bounds(name in ident_strategy()) {
        prop_assert!(name.len() >= 2 && name.len() <= 13);
    }

    /// An identifier can be used as a struct field name without error.
    #[test]
    fn ident_valid_as_field_name(name in ident_strategy()) {
        let ident = format_ident!("{}", name);
        let _s: ItemStruct = parse_quote! {
            struct Test { #ident: u32 }
        };
    }

    /// An identifier can be used as a struct name when capitalised.
    #[test]
    fn ident_valid_as_type_name(name in type_name_strategy()) {
        let ident = format_ident!("{}", name);
        let _s: ItemStruct = parse_quote! {
            struct #ident { value: u32 }
        };
    }

    /// An identifier can be used as an enum variant name when capitalised.
    #[test]
    fn ident_valid_as_variant(name in type_name_strategy()) {
        let ident = format_ident!("{}", name);
        let _e: ItemEnum = parse_quote! {
            enum E { #ident(u32) }
        };
    }

    /// Two generated identifiers can coexist as fields in one struct.
    #[test]
    fn two_idents_coexist_as_fields(a in ident_strategy(), b in ident_strategy()) {
        prop_assume!(a != b);
        let ia = format_ident!("{}", a);
        let ib = format_ident!("{}", b);
        let _s: ItemStruct = parse_quote! {
            struct S { #ia: u32, #ib: String }
        };
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 2. syn::parse_quote type construction (10 tests)
// ═══════════════════════════════════════════════════════════════════════════

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// A bare type name round-trips through parse_str.
    #[test]
    fn type_bare_roundtrip(name in type_name_strategy()) {
        let t = ty(&name);
        let s = ts(&t);
        // Token stream normalises spacing, just check the name is present.
        prop_assert!(s.contains(&name));
    }

    /// Option<T> can be constructed and stringified.
    #[test]
    fn type_option_construction(inner in type_name_strategy()) {
        let src = format!("Option<{inner}>");
        let t = ty(&src);
        let s = ts(&t);
        prop_assert!(s.contains("Option"));
        prop_assert!(s.contains(&inner));
    }

    /// Vec<T> can be constructed and stringified.
    #[test]
    fn type_vec_construction(inner in type_name_strategy()) {
        let src = format!("Vec<{inner}>");
        let t = ty(&src);
        let s = ts(&t);
        prop_assert!(s.contains("Vec"));
        prop_assert!(s.contains(&inner));
    }

    /// Box<T> can be constructed and stringified.
    #[test]
    fn type_box_construction(inner in type_name_strategy()) {
        let src = format!("Box<{inner}>");
        let t = ty(&src);
        prop_assert!(ts(&t).contains("Box"));
    }

    /// Nested generics Option<Vec<T>> parse correctly.
    #[test]
    fn type_nested_generics(inner in type_name_strategy()) {
        let src = format!("Option<Vec<{inner}>>");
        let t = ty(&src);
        let s = ts(&t);
        prop_assert!(s.contains("Option"));
        prop_assert!(s.contains("Vec"));
        prop_assert!(s.contains(&inner));
    }

    /// A type used in a struct field preserves the type string.
    #[test]
    fn type_in_struct_field(name in ident_strategy(), tname in type_name_strategy()) {
        let field_ident = format_ident!("{}", name);
        let field_ty = ty(&tname);
        let s: ItemStruct = parse_quote! {
            struct S { #field_ident: #field_ty }
        };
        let ft = s.fields.iter().next().unwrap().ty.to_token_stream().to_string();
        prop_assert!(ft.contains(&tname));
    }

    /// A type constructed via quote! matches one from parse_str.
    #[test]
    fn type_quote_matches_parse(tname in type_name_strategy()) {
        let ident = format_ident!("{}", tname);
        let via_quote: Type = parse_quote! { #ident };
        let via_parse = ty(&tname);
        prop_assert_eq!(ts(&via_quote), ts(&via_parse));
    }

    /// Tuple types (T, U) parse and stringify.
    #[test]
    fn type_tuple_construction(a in type_name_strategy(), b in type_name_strategy()) {
        let src = format!("({a}, {b})");
        let t = ty(&src);
        let s = ts(&t);
        prop_assert!(s.contains(&a));
        prop_assert!(s.contains(&b));
    }

    /// Reference types &T parse correctly.
    #[test]
    fn type_reference_construction(inner in type_name_strategy()) {
        let t = ty(&format!("&{inner}"));
        prop_assert!(ts(&t).contains(&inner));
        prop_assert!(ts(&t).contains("&"));
    }

    /// Unit type () round-trips.
    #[test]
    fn type_unit_roundtrip(_dummy in 0u8..1) {
        let t = ty("()");
        prop_assert_eq!(ts(&t), "()");
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 3. Attribute combination properties (10 tests)
// ═══════════════════════════════════════════════════════════════════════════

proptest! {
    #![proptest_config(ProptestConfig::with_cases(80))]

    /// A struct with #[adze::language] parses and the attribute is present.
    #[test]
    fn attr_language_on_struct(name in type_name_strategy()) {
        let ident = format_ident!("{}", name);
        let s: ItemStruct = parse_quote! {
            #[adze::language]
            struct #ident { value: u32 }
        };
        prop_assert!(s.attrs.iter().any(|a| is_adze_attr(a, "language")));
    }

    /// A struct with #[adze::extra] parses correctly.
    #[test]
    fn attr_extra_on_struct(name in type_name_strategy()) {
        let ident = format_ident!("{}", name);
        let s: ItemStruct = parse_quote! {
            #[adze::extra]
            struct #ident { value: u32 }
        };
        prop_assert!(s.attrs.iter().any(|a| is_adze_attr(a, "extra")));
    }

    /// An enum variant can carry #[adze::prec_left(N)] where N varies.
    #[test]
    fn attr_prec_left_on_variant(prec_val in 0u32..100) {
        let lit = proc_macro2::Literal::u32_unsuffixed(prec_val);
        let e: ItemEnum = parse_quote! {
            enum E {
                #[adze::prec_left(#lit)]
                A(u32, u32),
            }
        };
        let v = &e.variants[0];
        prop_assert!(v.attrs.iter().any(|a| is_adze_attr(a, "prec_left")));
    }

    /// An enum variant can carry #[adze::prec_right(N)].
    #[test]
    fn attr_prec_right_on_variant(prec_val in 0u32..100) {
        let lit = proc_macro2::Literal::u32_unsuffixed(prec_val);
        let e: ItemEnum = parse_quote! {
            enum E {
                #[adze::prec_right(#lit)]
                B(u32, u32),
            }
        };
        let v = &e.variants[0];
        prop_assert!(v.attrs.iter().any(|a| is_adze_attr(a, "prec_right")));
    }

    /// A field with #[adze::leaf(text = "x")] preserves the attribute.
    #[test]
    fn attr_leaf_text_on_field(text in "[a-z]{1,5}") {
        let s: ItemStruct = parse_quote! {
            struct S {
                #[adze::leaf(text = #text)]
                tok: (),
            }
        };
        let field = s.fields.iter().next().unwrap();
        prop_assert!(field.attrs.iter().any(|a| is_adze_attr(a, "leaf")));
    }

    /// A field with #[adze::leaf(pattern = "...")] preserves the attribute.
    #[test]
    fn attr_leaf_pattern_on_field(pat in "[a-z]{1,8}") {
        let s: ItemStruct = parse_quote! {
            struct S {
                #[adze::leaf(pattern = #pat)]
                tok: String,
            }
        };
        let field = s.fields.iter().next().unwrap();
        prop_assert!(field.attrs.iter().any(|a| is_adze_attr(a, "leaf")));
    }

    /// Multiple adze attributes can coexist on a single field.
    #[test]
    fn attr_multiple_on_field(text in "[a-z]{1,5}") {
        let s: ItemStruct = parse_quote! {
            struct S {
                #[adze::leaf(text = #text)]
                #[adze::repeat(non_empty = true)]
                items: Vec<()>,
            }
        };
        let field = s.fields.iter().next().unwrap();
        let adze_attrs: Vec<_> = field.attrs.iter()
            .filter(|a| a.path().segments.first().is_some_and(|seg| seg.ident == "adze"))
            .collect();
        prop_assert!(adze_attrs.len() >= 2);
    }

    /// #[adze::skip(expr)] parses on a struct field.
    #[test]
    fn attr_skip_on_field(val in prop::bool::ANY) {
        let s: ItemStruct = parse_quote! {
            struct S {
                #[adze::skip(#val)]
                meta: bool,
            }
        };
        let field = s.fields.iter().next().unwrap();
        prop_assert!(field.attrs.iter().any(|a| is_adze_attr(a, "skip")));
    }

    /// #[adze::external] can be applied to a unit struct.
    #[test]
    fn attr_external_on_unit_struct(name in type_name_strategy()) {
        let ident = format_ident!("{}", name);
        let s: ItemStruct = parse_quote! {
            #[adze::external]
            struct #ident;
        };
        prop_assert!(s.attrs.iter().any(|a| is_adze_attr(a, "external")));
    }

    /// #[adze::word] can be combined with #[adze::leaf(...)].
    #[test]
    fn attr_word_combined_with_leaf(pat in "[a-z]{1,5}") {
        let s: ItemStruct = parse_quote! {
            #[adze::word]
            #[adze::leaf(pattern = #pat)]
            struct Ident(String);
        };
        prop_assert!(s.attrs.iter().any(|a| is_adze_attr(a, "word")));
        prop_assert!(s.attrs.iter().any(|a| is_adze_attr(a, "leaf")));
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 4. Token stream formatting properties (10 tests)
// ═══════════════════════════════════════════════════════════════════════════

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// A struct's token stream contains the struct keyword and its name.
    #[test]
    fn tokens_struct_contains_name(name in type_name_strategy()) {
        let ident = format_ident!("{}", name);
        let s: ItemStruct = parse_quote! { struct #ident { x: u32 } };
        let tokens = s.to_token_stream().to_string();
        prop_assert!(tokens.contains("struct"));
        prop_assert!(tokens.contains(&name));
    }

    /// An enum's token stream contains the enum keyword and its name.
    #[test]
    fn tokens_enum_contains_name(name in type_name_strategy()) {
        let ident = format_ident!("{}", name);
        let e: ItemEnum = parse_quote! { enum #ident { A, B } };
        let tokens = e.to_token_stream().to_string();
        prop_assert!(tokens.contains("enum"));
        prop_assert!(tokens.contains(&name));
    }

    /// Token stream of a module contains the `mod` keyword and module name.
    #[test]
    fn tokens_module_contains_mod(name in ident_strategy()) {
        let ident = format_ident!("{}", name);
        let m: ItemMod = parse_quote! { mod #ident { struct S; } };
        let tokens = m.to_token_stream().to_string();
        prop_assert!(tokens.contains("mod"));
        prop_assert!(tokens.contains(&name));
    }

    /// A quote! invocation with an interpolated ident contains the ident string.
    #[test]
    fn tokens_quote_interpolation(name in ident_strategy()) {
        let ident = format_ident!("{}", name);
        let tokens = quote! { let #ident = 42; };
        let s = tokens.to_string();
        prop_assert!(s.contains(&name));
    }

    /// Two struct definitions produce distinct token streams.
    #[test]
    fn tokens_distinct_structs(a in type_name_strategy(), b in type_name_strategy()) {
        prop_assume!(a != b);
        let ia = format_ident!("{}", a);
        let ib = format_ident!("{}", b);
        let sa: ItemStruct = parse_quote! { struct #ia { x: u32 } };
        let sb: ItemStruct = parse_quote! { struct #ib { x: u32 } };
        prop_assert_ne!(
            sa.to_token_stream().to_string(),
            sb.to_token_stream().to_string()
        );
    }

    /// The token stream length of a struct grows with the number of fields.
    #[test]
    fn tokens_length_grows_with_fields(
        name in type_name_strategy(),
        f1 in ident_strategy(),
        f2 in ident_strategy(),
    ) {
        prop_assume!(f1 != f2);
        let ident = format_ident!("{}", name);
        let fi1 = format_ident!("{}", f1);
        let fi2 = format_ident!("{}", f2);
        let s1: ItemStruct = parse_quote! { struct #ident { #fi1: u32 } };
        let s2: ItemStruct = parse_quote! { struct #ident { #fi1: u32, #fi2: u32 } };
        prop_assert!(
            s2.to_token_stream().to_string().len() > s1.to_token_stream().to_string().len()
        );
    }

    /// An enum variant's name appears in its parent enum's token stream.
    #[test]
    fn tokens_variant_in_enum(vname in type_name_strategy()) {
        let vi = format_ident!("{}", vname);
        let e: ItemEnum = parse_quote! { enum E { #vi(u32) } };
        prop_assert!(e.to_token_stream().to_string().contains(&vname));
    }

    /// A pub struct's token stream contains `pub`.
    #[test]
    fn tokens_pub_visibility(name in type_name_strategy()) {
        let ident = format_ident!("{}", name);
        let s: ItemStruct = parse_quote! { pub struct #ident { x: u32 } };
        prop_assert!(s.to_token_stream().to_string().contains("pub"));
    }

    /// An attribute path appears in the struct's token stream.
    #[test]
    fn tokens_attr_path_present(aname in attr_name_strategy(), sname in type_name_strategy()) {
        let si = format_ident!("{}", sname);
        let ai = format_ident!("{}", aname);
        let s: ItemStruct = parse_quote! {
            #[adze::#ai]
            struct #si;
        };
        let tokens = s.to_token_stream().to_string();
        prop_assert!(tokens.contains("adze"));
        prop_assert!(tokens.contains(aname));
    }

    /// Token stream of a field attribute contains the attribute name.
    #[test]
    fn tokens_field_attr_present(text in "[a-z]{1,5}") {
        let s: ItemStruct = parse_quote! {
            struct S {
                #[adze::leaf(text = #text)]
                tok: (),
            }
        };
        let tokens = s.to_token_stream().to_string();
        prop_assert!(tokens.contains("leaf"));
        prop_assert!(tokens.contains(&text));
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 5. Type analysis edge cases (10 tests)
// ═══════════════════════════════════════════════════════════════════════════

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// try_extract_inner_type finds the inner type of Option<T>.
    #[test]
    fn type_extract_option_inner(inner in type_name_strategy()) {
        let src = format!("Option<{inner}>");
        let t = ty(&src);
        let empty: HashSet<&str> = HashSet::new();
        let (extracted, found) = try_extract_inner_type(&t, "Option", &empty);
        prop_assert!(found, "should extract inner from Option<{inner}>");
        let extracted_str = ts(&extracted);
        prop_assert!(extracted_str.contains(&inner));
    }

    /// try_extract_inner_type finds the inner type of Vec<T>.
    #[test]
    fn type_extract_vec_inner(inner in type_name_strategy()) {
        let src = format!("Vec<{inner}>");
        let t = ty(&src);
        let empty: HashSet<&str> = HashSet::new();
        let (extracted, found) = try_extract_inner_type(&t, "Vec", &empty);
        prop_assert!(found);
        prop_assert!(ts(&extracted).contains(&inner));
    }

    /// try_extract_inner_type finds the inner type of Box<T>.
    #[test]
    fn type_extract_box_inner(inner in type_name_strategy()) {
        let src = format!("Box<{inner}>");
        let t = ty(&src);
        let empty: HashSet<&str> = HashSet::new();
        let (extracted, found) = try_extract_inner_type(&t, "Box", &empty);
        prop_assert!(found);
        prop_assert!(ts(&extracted).contains(&inner));
    }

    /// try_extract_inner_type returns false for a bare type when seeking Option.
    #[test]
    fn type_extract_none_for_bare(name in type_name_strategy()) {
        let t = ty(&name);
        let empty: HashSet<&str> = HashSet::new();
        let (_extracted, found) = try_extract_inner_type(&t, "Option", &empty);
        prop_assert!(!found);
    }

    /// try_extract_inner_type returns false when wrapper name mismatches.
    #[test]
    fn type_extract_none_mismatch(inner in type_name_strategy()) {
        let src = format!("Vec<{inner}>");
        let t = ty(&src);
        let empty: HashSet<&str> = HashSet::new();
        let (_extracted, found) = try_extract_inner_type(&t, "Option", &empty);
        prop_assert!(!found);
    }

    /// filter_inner_type with empty skip set returns all types unchanged.
    #[test]
    fn type_filter_empty_skip(name in type_name_strategy()) {
        let t = ty(&name);
        let empty: HashSet<&str> = HashSet::new();
        let filtered = filter_inner_type(&t, &empty);
        prop_assert_eq!(ts(&filtered), ts(&t));
    }

    /// filter_inner_type with "Option" in skip set unwraps Option<T> to T.
    #[test]
    fn type_filter_strips_option(inner in type_name_strategy()) {
        let src = format!("Option<{inner}>");
        let t = ty(&src);
        let skips = skip_set(&["Option"]);
        let filtered = filter_inner_type(&t, &skips);
        prop_assert!(ts(&filtered).contains(&inner));
        prop_assert!(!ts(&filtered).contains("Option"));
    }

    /// wrap_leaf_type on a bare type wraps it in adze::WithLeaf<T>.
    #[test]
    fn type_wrap_leaf_bare(name in type_name_strategy()) {
        let t = ty(&name);
        let empty: HashSet<&str> = HashSet::new();
        let wrapped = wrap_leaf_type(&t, &empty);
        let s = ts(&wrapped);
        prop_assert!(s.contains("WithLeaf"), "expected WithLeaf wrapper in: {s}");
        prop_assert!(s.contains(&name));
    }

    /// wrap_leaf_type with "Option" in skip_over wraps the inner type of Option<T>.
    #[test]
    fn type_wrap_leaf_through_option(inner in type_name_strategy()) {
        let src = format!("Option<{inner}>");
        let t = ty(&src);
        let wraps = skip_set(&["Option"]);
        let wrapped = wrap_leaf_type(&t, &wraps);
        let s = ts(&wrapped);
        prop_assert!(s.contains("Option"), "expected Option preserved in: {s}");
        prop_assert!(s.contains("WithLeaf"), "expected WithLeaf wrapper in: {s}");
        prop_assert!(s.contains(&inner));
    }

    /// Nested extraction: extracting Vec from Option<Vec<T>> via two-step.
    #[test]
    fn type_nested_extraction(inner in type_name_strategy()) {
        let src = format!("Option<Vec<{inner}>>");
        let t = ty(&src);
        let empty: HashSet<&str> = HashSet::new();
        let (opt_inner, found1) = try_extract_inner_type(&t, "Option", &empty);
        prop_assert!(found1);
        let (vec_inner, found2) = try_extract_inner_type(&opt_inner, "Vec", &empty);
        prop_assert!(found2);
        prop_assert!(ts(&vec_inner).contains(&inner));
    }
}
