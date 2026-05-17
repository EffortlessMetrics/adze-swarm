#![allow(
    dead_code,
    reason = "shared integration-test support is compiled per test binary"
)]

//! Shared property-test strategies for `adze-common` integration tests.
//!
//! Keeping these generators in one place avoids every broad test module
//! re-declaring the same identifier, type-name, and container-name strategies.

use proptest::prelude::*;
use std::collections::HashSet;

/// Primitive and grammar-ish leaf type names used by field-processing tests.
pub const FIELD_LEAF_TYPE_NAMES: &[&str] = &[
    "i8", "i16", "i32", "i64", "u8", "u16", "u32", "u64", "f32", "f64", "bool", "char", "String",
    "usize", "isize", "Token", "Expr", "Stmt", "Node", "Leaf",
];

/// Primitive and grammar-ish leaf type names used by symbol extraction tests.
pub const SYMBOL_LEAF_TYPE_NAMES: &[&str] = &[
    "i32", "u32", "i64", "u64", "f32", "f64", "bool", "char", "String", "usize", "isize", "Token",
    "Expr", "Stmt", "Node", "Ident", "Literal",
];

/// Container type names that field and symbol tests treat as wrappers.
pub const CONTAINER_TYPE_NAMES_WITH_RC: &[&str] = &["Box", "Vec", "Option", "Arc", "Rc"];

/// Builds a strategy over a static list of string slices.
pub fn select_static(values: &'static [&'static str]) -> impl Strategy<Value = &'static str> {
    prop::sample::select(values)
}

/// Valid lowercase-starting Rust identifiers with a configurable tail length.
pub fn lower_ident(max_tail_len: usize) -> impl Strategy<Value = String> {
    prop::string::string_regex(&format!("[a-z][a-z0-9_]{{0,{max_tail_len}}}"))
        .expect("lowercase identifier regex should be valid")
        .prop_filter("must be valid ident", |s| {
            !s.is_empty() && syn::parse_str::<syn::Ident>(s).is_ok()
        })
}

/// Produce a vector of distinct lowercase identifiers.
pub fn distinct_lower_idents(
    max: usize,
    max_tail_len: usize,
) -> impl Strategy<Value = Vec<String>> {
    prop::collection::vec(lower_ident(max_tail_len), 1..=max).prop_map(|v| {
        let mut seen = HashSet::new();
        v.into_iter().filter(|s| seen.insert(s.clone())).collect()
    })
}

/// Shared field-oriented leaf type-name strategy.
pub fn field_leaf_type_name() -> impl Strategy<Value = &'static str> {
    select_static(FIELD_LEAF_TYPE_NAMES)
}

/// Shared symbol-oriented leaf type-name strategy.
pub fn symbol_leaf_type_name() -> impl Strategy<Value = &'static str> {
    select_static(SYMBOL_LEAF_TYPE_NAMES)
}

/// Shared container type-name strategy for wrappers, including `Rc`.
pub fn container_name_with_rc() -> impl Strategy<Value = &'static str> {
    select_static(CONTAINER_TYPE_NAMES_WITH_RC)
}
