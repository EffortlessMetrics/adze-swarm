#![no_main]

use std::collections::HashSet;

use adze_common_type_ops_core::{filter_inner_type, try_extract_inner_type, wrap_leaf_type};
use libfuzzer_sys::fuzz_target;
use quote::ToTokens;
use syn::{Type, parse_str};

fn as_bool(raw: u8) -> bool {
    raw % 2 == 0
}

fn synthetic_type_from_data(data: &[u8]) -> Type {
    let leaf = match data.first().copied().unwrap_or(0) % 7 {
        0 => "String",
        1 => "Node",
        2 => "u8",
        3 => "bool",
        4 => "std::collections::HashMap<String, usize>",
        5 => "Result<u16, Error>",
        _ => "std::sync::Arc<str>",
    };

    let mut source = leaf.to_string();
    for i in (1..data.len()).rev().take(5) {
        let outer = match data[i] % 6 {
            0 => "Vec",
            1 => "Option",
            2 => "Box",
            3 => "Arc",
            4 => "std::rc::Rc",
            _ => "std::borrow::Cow<'static, str>",
        };
        source = format!("{outer}<{source}>");
    }

    parse_str::<Type>(&source)
        .unwrap_or_else(|_| parse_str::<Type>("String").expect("valid fallback type"))
}

fuzz_target!(|data: &[u8]| {
    let parsed = synthetic_type_from_data(data);
    let mut skip_over = HashSet::new();
    if as_bool(data.first().copied().unwrap_or(0)) {
        skip_over.insert("Vec");
    }
    if as_bool(data.get(1).copied().unwrap_or(0)) {
        skip_over.insert("Option");
    }
    if as_bool(data.get(2).copied().unwrap_or(0)) {
        skip_over.insert("Box");
    }
    if as_bool(data.get(3).copied().unwrap_or(0)) {
        skip_over.insert("Arc");
    }
    if as_bool(data.get(4).copied().unwrap_or(0)) {
        skip_over.insert("Rc");
    }
    if as_bool(data.get(5).copied().unwrap_or(0)) {
        skip_over.insert("Cow");
    }

    let target = match data.first().copied().unwrap_or(0) % 6 {
        0 => "Vec",
        1 => "Option",
        2 => "Box",
        3 => "Arc",
        4 => "Rc",
        _ => "Cow",
    };

    let extracted = try_extract_inner_type(&parsed, target, &skip_over);
    let filtered = filter_inner_type(&parsed, &skip_over);
    let wrapped = wrap_leaf_type(&parsed, &skip_over);

    assert!(!extracted.0.to_token_stream().to_string().is_empty());
    assert!(!filtered.to_token_stream().to_string().is_empty());
    assert!(!wrapped.to_token_stream().to_string().is_empty());
});
