//! Tests for derive-related attribute and type processing in adze-macro.
//!
//! 64 tests across 8 categories (8 tests each):
//!   - derive_struct_*    : struct DeriveInput parsing
//!   - derive_enum_*      : enum DeriveInput parsing
//!   - derive_field_*     : field type and attribute inspection
//!   - derive_generic_*   : generic parameter extraction
//!   - derive_attr_*      : attribute path and argument processing
//!   - derive_lifetime_*  : lifetime parameter handling
//!   - derive_complex_*   : nested / multi-layer type structures
//!   - derive_edge_*      : edge cases and unusual forms

use quote::ToTokens;
use syn::{
    Data, DeriveInput, Fields, GenericArgument, GenericParam, ItemStruct, PathArguments, Type,
    TypePath,
};

// ── Helpers ──────────────────────────────────────────────────────────────────

fn type_str(ty: &Type) -> String {
    ty.to_token_stream().to_string()
}

fn parse_derive_str(s: &str) -> DeriveInput {
    syn::parse_str::<DeriveInput>(s).expect("failed to parse DeriveInput from str")
}

fn parse_type_str(s: &str) -> Type {
    syn::parse_str::<Type>(s).expect("failed to parse Type from str")
}

fn parse_struct_str(s: &str) -> ItemStruct {
    syn::parse_str::<ItemStruct>(s).expect("failed to parse ItemStruct from str")
}

fn named_field_names(fields: &Fields) -> Vec<String> {
    fields
        .iter()
        .filter_map(|f| f.ident.as_ref().map(|id| id.to_string()))
        .collect()
}

fn field_type_strings(fields: &Fields) -> Vec<String> {
    fields.iter().map(|f| type_str(&f.ty)).collect()
}

fn extract_inner_type<'a>(ty: &'a Type, wrapper: &str) -> Option<&'a Type> {
    if let Type::Path(TypePath { path, .. }) = ty
        && let Some(seg) = path.segments.last()
        && seg.ident == wrapper
        && let PathArguments::AngleBracketed(args) = &seg.arguments
        && let Some(GenericArgument::Type(inner)) = args.args.first()
    {
        return Some(inner);
    }
    None
}

// ═══════════════════════════════════════════════════════════════════════════════
// Section 1: derive_struct_* — struct DeriveInput parsing (8 tests)
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn derive_struct_named_fields() {
    let di = parse_derive_str("struct Foo { x: i32, y: String }");
    assert_eq!(di.ident, "Foo");
    assert!(matches!(di.data, Data::Struct(ref s) if matches!(s.fields, Fields::Named(_))));
    if let Data::Struct(ref s) = di.data {
        assert_eq!(s.fields.len(), 2);
    }
}

#[test]
fn derive_struct_unit() {
    let di = parse_derive_str("struct Unit;");
    assert_eq!(di.ident, "Unit");
    if let Data::Struct(ref s) = di.data {
        assert!(matches!(s.fields, Fields::Unit));
    }
}

#[test]
fn derive_struct_tuple() {
    let di = parse_derive_str("struct Pair(i32, String);");
    assert_eq!(di.ident, "Pair");
    if let Data::Struct(ref s) = di.data {
        assert!(matches!(s.fields, Fields::Unnamed(_)));
        assert_eq!(s.fields.len(), 2);
    }
}

#[test]
fn derive_struct_with_derive_attr() {
    let di = parse_derive_str("#[derive(Debug, Clone)] struct Tagged { v: u8 }");
    assert_eq!(di.ident, "Tagged");
    assert_eq!(di.attrs.len(), 1);
    let path_str = di.attrs[0].path().to_token_stream().to_string();
    assert_eq!(path_str, "derive");
}

#[test]
fn derive_struct_pub_visibility() {
    let item = parse_struct_str("pub struct Visible { pub name: String }");
    assert_eq!(item.ident, "Visible");
    assert!(matches!(item.vis, syn::Visibility::Public(_)));
}

#[test]
fn derive_struct_empty_named() {
    let di = parse_derive_str("struct Empty {}");
    assert_eq!(di.ident, "Empty");
    if let Data::Struct(ref s) = di.data {
        assert_eq!(s.fields.len(), 0);
        assert!(matches!(s.fields, Fields::Named(_)));
    }
}

#[test]
fn derive_struct_field_names() {
    let item = parse_struct_str("struct Record { alpha: u32, beta: f64, gamma: bool }");
    let names = named_field_names(&item.fields);
    assert_eq!(names, vec!["alpha", "beta", "gamma"]);
}

#[test]
fn derive_struct_field_types() {
    let item = parse_struct_str("struct Types { a: Vec<u8>, b: Option<String> }");
    let types = field_type_strings(&item.fields);
    assert!(types[0].contains("Vec"));
    assert!(types[1].contains("Option"));
}

// ═══════════════════════════════════════════════════════════════════════════════
// Section 2: derive_enum_* — enum DeriveInput parsing (8 tests)
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn derive_enum_simple_variants() {
    let di = parse_derive_str("enum Color { Red, Green, Blue }");
    assert_eq!(di.ident, "Color");
    if let Data::Enum(ref e) = di.data {
        assert_eq!(e.variants.len(), 3);
        let names: Vec<_> = e.variants.iter().map(|v| v.ident.to_string()).collect();
        assert_eq!(names, vec!["Red", "Green", "Blue"]);
    }
}

#[test]
fn derive_enum_tuple_variant() {
    let di = parse_derive_str("enum Expr { Num(i32), Str(String) }");
    if let Data::Enum(ref e) = di.data {
        for v in &e.variants {
            assert!(matches!(v.fields, Fields::Unnamed(_)));
            assert_eq!(v.fields.len(), 1);
        }
    }
}

#[test]
fn derive_enum_struct_variant() {
    let di = parse_derive_str(
        "enum Node { Leaf { value: i32 }, Branch { left: Box<u8>, right: Box<u8> } }",
    );
    if let Data::Enum(ref e) = di.data {
        assert_eq!(e.variants.len(), 2);
        assert!(matches!(e.variants[0].fields, Fields::Named(_)));
        assert_eq!(e.variants[0].fields.len(), 1);
        assert_eq!(e.variants[1].fields.len(), 2);
    }
}

#[test]
fn derive_enum_mixed_variants() {
    let di = parse_derive_str("enum Mixed { A, B(u32), C { x: f64 } }");
    if let Data::Enum(ref e) = di.data {
        assert!(matches!(e.variants[0].fields, Fields::Unit));
        assert!(matches!(e.variants[1].fields, Fields::Unnamed(_)));
        assert!(matches!(e.variants[2].fields, Fields::Named(_)));
    }
}

#[test]
fn derive_enum_with_derive_attr() {
    let di = parse_derive_str("#[derive(Debug, PartialEq)] enum Token { Plus, Minus }");
    assert_eq!(di.attrs.len(), 1);
    if let Data::Enum(ref e) = di.data {
        assert_eq!(e.variants.len(), 2);
    }
}

#[test]
fn derive_enum_single_variant() {
    let di = parse_derive_str("enum Wrapper { Only(String) }");
    if let Data::Enum(ref e) = di.data {
        assert_eq!(e.variants.len(), 1);
        assert_eq!(e.variants[0].ident, "Only");
    }
}

#[test]
fn derive_enum_variant_with_multiple_fields() {
    let di = parse_derive_str("enum Op { Binary(Box<u8>, String, Box<u8>) }");
    if let Data::Enum(ref e) = di.data {
        assert_eq!(e.variants[0].fields.len(), 3);
    }
}

#[test]
fn derive_enum_discriminant() {
    let di = parse_derive_str("enum Status { Active = 1, Inactive = 0 }");
    if let Data::Enum(ref e) = di.data {
        assert!(e.variants[0].discriminant.is_some());
        assert!(e.variants[1].discriminant.is_some());
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Section 3: derive_field_* — field type and attribute inspection (8 tests)
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn derive_field_option_type() {
    let ty = parse_type_str("Option<i32>");
    let inner = extract_inner_type(&ty, "Option").expect("should extract inner");
    assert_eq!(type_str(inner), "i32");
}

#[test]
fn derive_field_vec_type() {
    let ty = parse_type_str("Vec<String>");
    let inner = extract_inner_type(&ty, "Vec").expect("should extract inner");
    assert_eq!(type_str(inner), "String");
}

#[test]
fn derive_field_box_type() {
    let ty = parse_type_str("Box<Expr>");
    let inner = extract_inner_type(&ty, "Box").expect("should extract inner");
    assert_eq!(type_str(inner), "Expr");
}

#[test]
fn derive_field_nested_option_vec() {
    let ty = parse_type_str("Option<Vec<u8>>");
    let vec_ty = extract_inner_type(&ty, "Option").expect("outer Option");
    let inner = extract_inner_type(vec_ty, "Vec").expect("inner Vec");
    assert_eq!(type_str(inner), "u8");
}

#[test]
fn derive_field_reference_type() {
    let ty = parse_type_str("&str");
    assert!(matches!(ty, Type::Reference(_)));
}

#[test]
fn derive_field_tuple_type() {
    let ty = parse_type_str("(i32, String, bool)");
    if let Type::Tuple(ref t) = ty {
        assert_eq!(t.elems.len(), 3);
    } else {
        panic!("expected tuple type");
    }
}

#[test]
fn derive_field_unit_type() {
    let ty = parse_type_str("()");
    if let Type::Tuple(ref t) = ty {
        assert!(t.elems.is_empty());
    } else {
        panic!("expected unit/empty tuple");
    }
}

#[test]
fn derive_field_fn_pointer() {
    let ty = parse_type_str("fn(i32) -> bool");
    assert!(matches!(ty, Type::BareFn(_)));
}

// ═══════════════════════════════════════════════════════════════════════════════
// Section 4: derive_generic_* — generic parameter extraction (8 tests)
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn derive_generic_single_type_param() {
    let di = parse_derive_str("struct Container<T> { value: T }");
    assert_eq!(di.generics.params.len(), 1);
    assert!(matches!(di.generics.params[0], GenericParam::Type(_)));
}

#[test]
fn derive_generic_multiple_type_params() {
    let di = parse_derive_str("struct Pair<A, B> { first: A, second: B }");
    assert_eq!(di.generics.params.len(), 2);
    let names: Vec<_> = di
        .generics
        .type_params()
        .map(|tp| tp.ident.to_string())
        .collect();
    assert_eq!(names, vec!["A", "B"]);
}

#[test]
fn derive_generic_with_bound() {
    let di = parse_derive_str("struct Bounded<T: Clone> { value: T }");
    let tp = di.generics.type_params().next().expect("one type param");
    assert_eq!(tp.ident, "T");
    assert!(!tp.bounds.is_empty());
}

#[test]
fn derive_generic_where_clause() {
    let di = parse_derive_str("struct W<T> where T: Send + Sync { v: T }");
    assert!(di.generics.where_clause.is_some());
}

#[test]
fn derive_generic_const_param() {
    let di = parse_derive_str("struct Array<const N: usize> { data: [u8; N] }");
    assert_eq!(di.generics.params.len(), 1);
    assert!(matches!(di.generics.params[0], GenericParam::Const(_)));
}

#[test]
fn derive_generic_default_type() {
    let di = parse_derive_str("struct WithDefault<T = String> { value: T }");
    let tp = di.generics.type_params().next().unwrap();
    assert!(tp.default.is_some());
}

#[test]
fn derive_generic_mixed_params() {
    let di = parse_derive_str("struct Mix<'a, T: Clone, const N: usize> { r: &'a T, arr: [T; N] }");
    assert_eq!(di.generics.params.len(), 3);
    assert!(matches!(di.generics.params[0], GenericParam::Lifetime(_)));
    assert!(matches!(di.generics.params[1], GenericParam::Type(_)));
    assert!(matches!(di.generics.params[2], GenericParam::Const(_)));
}

#[test]
fn derive_generic_no_params() {
    let di = parse_derive_str("struct Plain { x: i32 }");
    assert!(di.generics.params.is_empty());
    assert!(di.generics.where_clause.is_none());
}

// ═══════════════════════════════════════════════════════════════════════════════
// Section 5: derive_attr_* — attribute path and argument processing (8 tests)
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn derive_attr_single_derive() {
    let di = parse_derive_str("#[derive(Debug)] struct S;");
    assert_eq!(di.attrs.len(), 1);
    assert!(di.attrs[0].path().is_ident("derive"));
}

#[test]
fn derive_attr_multiple_derives() {
    let di = parse_derive_str("#[derive(Debug)] #[derive(Clone)] struct S;");
    assert_eq!(di.attrs.len(), 2);
    assert!(di.attrs.iter().all(|a| a.path().is_ident("derive")));
}

#[test]
fn derive_attr_namespaced_path() {
    let di = parse_derive_str("#[serde(rename_all = \"camelCase\")] struct S { x: i32 }");
    assert_eq!(di.attrs.len(), 1);
    assert!(di.attrs[0].path().is_ident("serde"));
}

#[test]
fn derive_attr_cfg_condition() {
    let di = parse_derive_str("#[cfg(test)] struct TestOnly;");
    assert!(di.attrs[0].path().is_ident("cfg"));
}

#[test]
fn derive_attr_repr() {
    let di = parse_derive_str("#[repr(C)] struct CLayout { a: u32, b: u32 }");
    assert!(di.attrs[0].path().is_ident("repr"));
}

#[test]
fn derive_attr_allow() {
    let di = parse_derive_str("#[allow(dead_code)] struct Unused;");
    assert!(di.attrs[0].path().is_ident("allow"));
}

#[test]
fn derive_attr_doc_comment() {
    // Doc comments are parsed as #[doc = "..."] attributes
    let di = parse_derive_str("/// A documented struct\nstruct Documented;");
    assert!(!di.attrs.is_empty());
    assert!(di.attrs[0].path().is_ident("doc"));
}

#[test]
fn derive_attr_mixed_outer_attrs() {
    let di = parse_derive_str(
        "#[derive(Debug)] #[allow(dead_code)] #[cfg(feature = \"test\")] struct Multi;",
    );
    assert_eq!(di.attrs.len(), 3);
    let paths: Vec<String> = di
        .attrs
        .iter()
        .map(|a| a.path().to_token_stream().to_string())
        .collect();
    assert_eq!(paths, vec!["derive", "allow", "cfg"]);
}

// ═══════════════════════════════════════════════════════════════════════════════
// Section 6: derive_lifetime_* — lifetime parameter handling (8 tests)
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn derive_lifetime_single() {
    let di = parse_derive_str("struct Ref<'a> { data: &'a str }");
    assert_eq!(di.generics.lifetimes().count(), 1);
    let lt = di.generics.lifetimes().next().unwrap();
    assert_eq!(lt.lifetime.ident, "a");
}

#[test]
fn derive_lifetime_multiple() {
    let di = parse_derive_str("struct Multi<'a, 'b> { x: &'a str, y: &'b str }");
    let names: Vec<_> = di
        .generics
        .lifetimes()
        .map(|lt| lt.lifetime.ident.to_string())
        .collect();
    assert_eq!(names, vec!["a", "b"]);
}

#[test]
fn derive_lifetime_bound() {
    let di = parse_derive_str("struct Bounded<'a, 'b: 'a> { x: &'a str, y: &'b str }");
    let params: Vec<_> = di.generics.lifetimes().collect();
    assert_eq!(params.len(), 2);
    // 'b: 'a means 'b has bounds
    assert!(!params[1].bounds.is_empty());
}

#[test]
fn derive_lifetime_with_type_param() {
    let di = parse_derive_str("struct Combo<'a, T> { r: &'a T }");
    assert_eq!(di.generics.lifetimes().count(), 1);
    assert_eq!(di.generics.type_params().count(), 1);
}

#[test]
fn derive_lifetime_static_field() {
    let ty = parse_type_str("&'static str");
    if let Type::Reference(ref r) = ty {
        assert!(r.lifetime.is_some());
        assert_eq!(r.lifetime.as_ref().unwrap().ident, "static");
    } else {
        panic!("expected reference type");
    }
}

#[test]
fn derive_lifetime_in_enum() {
    let di = parse_derive_str("enum Token<'src> { Word(&'src str), Num(i32) }");
    assert_eq!(di.generics.lifetimes().count(), 1);
    let lt = di.generics.lifetimes().next().unwrap();
    assert_eq!(lt.lifetime.ident, "src");
}

#[test]
fn derive_lifetime_in_where_clause() {
    let di = parse_derive_str("struct Constrained<'a, T> where T: 'a { val: &'a T }");
    assert!(di.generics.where_clause.is_some());
    let wc = di.generics.where_clause.as_ref().unwrap();
    assert!(!wc.predicates.is_empty());
}

#[test]
fn derive_lifetime_none_present() {
    let di = parse_derive_str("struct NoLifetime { x: i32 }");
    assert_eq!(di.generics.lifetimes().count(), 0);
}

// ═══════════════════════════════════════════════════════════════════════════════
// Section 7: derive_complex_* — nested / multi-layer type structures (8 tests)
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn derive_complex_vec_of_option() {
    let ty = parse_type_str("Vec<Option<i32>>");
    let opt = extract_inner_type(&ty, "Vec").expect("Vec inner");
    let inner = extract_inner_type(opt, "Option").expect("Option inner");
    assert_eq!(type_str(inner), "i32");
}

#[test]
fn derive_complex_result_type() {
    let ty = parse_type_str("Result<String, Box<dyn std::error::Error>>");
    if let Type::Path(ref tp) = ty {
        let seg = tp.path.segments.last().unwrap();
        assert_eq!(seg.ident, "Result");
        if let PathArguments::AngleBracketed(ref args) = seg.arguments {
            assert_eq!(args.args.len(), 2);
        }
    }
}

#[test]
fn derive_complex_hashmap_type() {
    let ty = parse_type_str("std::collections::HashMap<String, Vec<u8>>");
    if let Type::Path(ref tp) = ty {
        let seg = tp.path.segments.last().unwrap();
        assert_eq!(seg.ident, "HashMap");
    }
}

#[test]
fn derive_complex_nested_generics_struct() {
    let di = parse_derive_str("struct Deep<T> { inner: Option<Vec<Box<T>>> }");
    assert_eq!(di.ident, "Deep");
    if let Data::Struct(ref s) = di.data {
        let ft = type_str(&s.fields.iter().next().unwrap().ty);
        assert!(ft.contains("Option"));
        assert!(ft.contains("Vec"));
        assert!(ft.contains("Box"));
    }
}

#[test]
fn derive_complex_enum_recursive() {
    let di = parse_derive_str("enum Tree { Leaf(i32), Node(Box<Tree>, Box<Tree>) }");
    if let Data::Enum(ref e) = di.data {
        assert_eq!(e.variants.len(), 2);
        assert_eq!(e.variants[1].fields.len(), 2);
    }
}

#[test]
fn derive_complex_tuple_of_tuples() {
    let ty = parse_type_str("((i32, i32), (String, bool))");
    if let Type::Tuple(ref t) = ty {
        assert_eq!(t.elems.len(), 2);
        for elem in &t.elems {
            assert!(matches!(elem, Type::Tuple(_)));
        }
    }
}

#[test]
fn derive_complex_fn_returning_result() {
    let ty = parse_type_str("fn(String) -> Result<i32, String>");
    if let Type::BareFn(ref f) = ty {
        assert_eq!(f.inputs.len(), 1);
        assert!(f.output.to_token_stream().to_string().contains("Result"));
    } else {
        panic!("expected bare fn");
    }
}

#[test]
fn derive_complex_multi_attr_struct() {
    let di = parse_derive_str(
        "#[derive(Debug, Clone, PartialEq, Eq, Hash)] #[repr(C)] struct Multi { a: u32, b: u64 }",
    );
    assert_eq!(di.attrs.len(), 2);
    assert_eq!(di.ident, "Multi");
    if let Data::Struct(ref s) = di.data {
        assert_eq!(s.fields.len(), 2);
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Section 8: derive_edge_* — edge cases and unusual forms (8 tests)
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn derive_edge_empty_enum() {
    // An enum with no variants (like `!`-like types)
    let di = parse_derive_str("enum Never {}");
    if let Data::Enum(ref e) = di.data {
        assert!(e.variants.is_empty());
    }
}

#[test]
fn derive_edge_single_field_tuple_struct() {
    let di = parse_derive_str("struct Wrapper(String);");
    if let Data::Struct(ref s) = di.data {
        assert!(matches!(s.fields, Fields::Unnamed(_)));
        assert_eq!(s.fields.len(), 1);
    }
}

#[test]
fn derive_edge_many_fields() {
    let di = parse_derive_str(
        "struct Wide { a: u8, b: u16, c: u32, d: u64, e: i8, f: i16, g: i32, h: i64 }",
    );
    if let Data::Struct(ref s) = di.data {
        assert_eq!(s.fields.len(), 8);
    }
}

#[test]
fn derive_edge_type_alias_path() {
    let ty = parse_type_str("std::vec::Vec<u8>");
    if let Type::Path(ref tp) = ty {
        assert_eq!(tp.path.segments.len(), 3);
        assert_eq!(tp.path.segments.last().unwrap().ident, "Vec");
    }
}

#[test]
fn derive_edge_slice_type() {
    let ty = parse_type_str("[u8]");
    assert!(matches!(ty, Type::Slice(_)));
}

#[test]
fn derive_edge_array_type() {
    let ty = parse_type_str("[u8; 32]");
    assert!(matches!(ty, Type::Array(_)));
}

#[test]
fn derive_edge_raw_pointer() {
    let ty = parse_type_str("*const u8");
    assert!(matches!(ty, Type::Ptr(_)));
}

#[test]
fn derive_edge_never_type() {
    let ty = parse_type_str("!");
    assert!(matches!(ty, Type::Never(_)));
}
