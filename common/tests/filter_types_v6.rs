//! Tests for `filter_inner_type`, `try_extract_inner_type`, `wrap_leaf_type`,
//! and `is_parameterized` covering eight categories × eight tests = 64 tests.

use std::collections::HashSet;

use adze_common::{filter_inner_type, try_extract_inner_type, wrap_leaf_type};
use quote::ToTokens;
use syn::{Type, parse_quote};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn skip<'a>(names: &'a [&'a str]) -> HashSet<&'a str> {
    names.iter().copied().collect()
}

fn ty_str(ty: &Type) -> String {
    ty.to_token_stream().to_string()
}

/// Returns `true` when the outermost type carries angle-bracketed generic
/// arguments (i.e. is a parameterized `Type::Path`).
fn is_parameterized(ty: &Type) -> bool {
    if let Type::Path(p) = ty
        && let Some(seg) = p.path.segments.last()
    {
        return matches!(seg.arguments, syn::PathArguments::AngleBracketed(_));
    }
    false
}

// ===========================================================================
// 1. filter_vec_* — filtering Vec types (8 tests)
// ===========================================================================

#[test]
fn filter_vec_extracts_inner_string() {
    let ty: Type = parse_quote!(Vec<String>);
    let result = filter_inner_type(&ty, &skip(&["Vec"]));
    assert_eq!(ty_str(&result), "String");
}

#[test]
fn filter_vec_extracts_inner_u8() {
    let ty: Type = parse_quote!(Vec<u8>);
    let result = filter_inner_type(&ty, &skip(&["Vec"]));
    assert_eq!(ty_str(&result), "u8");
}

#[test]
fn filter_vec_extracts_inner_bool() {
    let ty: Type = parse_quote!(Vec<bool>);
    let result = filter_inner_type(&ty, &skip(&["Vec"]));
    assert_eq!(ty_str(&result), "bool");
}

#[test]
fn filter_vec_extracts_inner_f64() {
    let ty: Type = parse_quote!(Vec<f64>);
    let result = filter_inner_type(&ty, &skip(&["Vec"]));
    assert_eq!(ty_str(&result), "f64");
}

#[test]
fn filter_vec_extracts_inner_isize() {
    let ty: Type = parse_quote!(Vec<isize>);
    let result = filter_inner_type(&ty, &skip(&["Vec"]));
    assert_eq!(ty_str(&result), "isize");
}

#[test]
fn filter_vec_extracts_inner_custom() {
    let ty: Type = parse_quote!(Vec<MyNode>);
    let result = filter_inner_type(&ty, &skip(&["Vec"]));
    assert_eq!(ty_str(&result), "MyNode");
}

#[test]
fn filter_vec_qualified_path_uses_last_segment() {
    let ty: Type = parse_quote!(std::vec::Vec<i32>);
    let result = filter_inner_type(&ty, &skip(&["Vec"]));
    assert_eq!(ty_str(&result), "i32");
}

#[test]
fn filter_vec_wraps_inner_with_leaf() {
    let ty: Type = parse_quote!(Vec<u32>);
    let wrapped = wrap_leaf_type(&ty, &skip(&["Vec"]));
    assert_eq!(ty_str(&wrapped), "Vec < adze :: WithLeaf < u32 > >");
}

// ===========================================================================
// 2. filter_option_* — filtering Option types (8 tests)
// ===========================================================================

#[test]
fn filter_option_extracts_inner_string() {
    let ty: Type = parse_quote!(Option<String>);
    let result = filter_inner_type(&ty, &skip(&["Option"]));
    assert_eq!(ty_str(&result), "String");
}

#[test]
fn filter_option_extracts_inner_i64() {
    let ty: Type = parse_quote!(Option<i64>);
    let result = filter_inner_type(&ty, &skip(&["Option"]));
    assert_eq!(ty_str(&result), "i64");
}

#[test]
fn filter_option_extracts_inner_bool() {
    let ty: Type = parse_quote!(Option<bool>);
    let result = filter_inner_type(&ty, &skip(&["Option"]));
    assert_eq!(ty_str(&result), "bool");
}

#[test]
fn filter_option_extracts_inner_usize() {
    let ty: Type = parse_quote!(Option<usize>);
    let result = filter_inner_type(&ty, &skip(&["Option"]));
    assert_eq!(ty_str(&result), "usize");
}

#[test]
fn filter_option_extracts_inner_char() {
    let ty: Type = parse_quote!(Option<char>);
    let result = filter_inner_type(&ty, &skip(&["Option"]));
    assert_eq!(ty_str(&result), "char");
}

#[test]
fn filter_option_extracts_inner_custom() {
    let ty: Type = parse_quote!(Option<Token>);
    let result = filter_inner_type(&ty, &skip(&["Option"]));
    assert_eq!(ty_str(&result), "Token");
}

#[test]
fn filter_option_try_extract_succeeds() {
    let ty: Type = parse_quote!(Option<u16>);
    let (inner, ok) = try_extract_inner_type(&ty, "Option", &skip(&[]));
    assert!(ok);
    assert_eq!(ty_str(&inner), "u16");
}

#[test]
fn filter_option_is_parameterized() {
    let ty: Type = parse_quote!(Option<f32>);
    assert!(is_parameterized(&ty));
}

// ===========================================================================
// 3. filter_box_* — filtering Box types (8 tests)
// ===========================================================================

#[test]
fn filter_box_extracts_inner_string() {
    let ty: Type = parse_quote!(Box<String>);
    let result = filter_inner_type(&ty, &skip(&["Box"]));
    assert_eq!(ty_str(&result), "String");
}

#[test]
fn filter_box_extracts_inner_i32() {
    let ty: Type = parse_quote!(Box<i32>);
    let result = filter_inner_type(&ty, &skip(&["Box"]));
    assert_eq!(ty_str(&result), "i32");
}

#[test]
fn filter_box_extracts_inner_bool() {
    let ty: Type = parse_quote!(Box<bool>);
    let result = filter_inner_type(&ty, &skip(&["Box"]));
    assert_eq!(ty_str(&result), "bool");
}

#[test]
fn filter_box_extracts_inner_u64() {
    let ty: Type = parse_quote!(Box<u64>);
    let result = filter_inner_type(&ty, &skip(&["Box"]));
    assert_eq!(ty_str(&result), "u64");
}

#[test]
fn filter_box_extracts_inner_f32() {
    let ty: Type = parse_quote!(Box<f32>);
    let result = filter_inner_type(&ty, &skip(&["Box"]));
    assert_eq!(ty_str(&result), "f32");
}

#[test]
fn filter_box_extracts_inner_custom() {
    let ty: Type = parse_quote!(Box<AstExpr>);
    let result = filter_inner_type(&ty, &skip(&["Box"]));
    assert_eq!(ty_str(&result), "AstExpr");
}

#[test]
fn filter_box_try_extract_succeeds() {
    let ty: Type = parse_quote!(Box<u8>);
    let (inner, ok) = try_extract_inner_type(&ty, "Box", &skip(&[]));
    assert!(ok);
    assert_eq!(ty_str(&inner), "u8");
}

#[test]
fn filter_box_wrap_leaf_wraps_inner() {
    let ty: Type = parse_quote!(Box<i16>);
    let wrapped = wrap_leaf_type(&ty, &skip(&["Box"]));
    assert_eq!(ty_str(&wrapped), "Box < adze :: WithLeaf < i16 > >");
}

// ===========================================================================
// 4. filter_result_* — filtering Result types (8 tests)
// ===========================================================================

#[test]
fn filter_result_skipped_extracts_first_arg() {
    let ty: Type = parse_quote!(Result<String, i32>);
    let result = filter_inner_type(&ty, &skip(&["Result"]));
    assert_eq!(ty_str(&result), "String");
}

#[test]
fn filter_result_not_skipped_unchanged() {
    let ty: Type = parse_quote!(Result<String, i32>);
    let result = filter_inner_type(&ty, &skip(&["Box"]));
    assert_eq!(ty_str(&result), "Result < String , i32 >");
}

#[test]
fn filter_result_try_extract_ok_arg() {
    let ty: Type = parse_quote!(Result<bool, String>);
    let (inner, ok) = try_extract_inner_type(&ty, "Result", &skip(&[]));
    assert!(ok);
    assert_eq!(ty_str(&inner), "bool");
}

#[test]
fn filter_result_try_extract_mismatch() {
    let ty: Type = parse_quote!(Result<u8, String>);
    let (inner, ok) = try_extract_inner_type(&ty, "Option", &skip(&[]));
    assert!(!ok);
    assert_eq!(ty_str(&inner), "Result < u8 , String >");
}

#[test]
fn filter_result_is_parameterized() {
    let ty: Type = parse_quote!(Result<(), String>);
    assert!(is_parameterized(&ty));
}

#[test]
fn filter_result_wrap_leaf_wraps_both_args() {
    let ty: Type = parse_quote!(Result<String, i32>);
    let wrapped = wrap_leaf_type(&ty, &skip(&["Result"]));
    assert_eq!(
        ty_str(&wrapped),
        "Result < adze :: WithLeaf < String > , adze :: WithLeaf < i32 > >"
    );
}

#[test]
fn filter_result_extract_through_box() {
    let ty: Type = parse_quote!(Box<Result<u16, String>>);
    let (inner, ok) = try_extract_inner_type(&ty, "Result", &skip(&["Box"]));
    assert!(ok);
    assert_eq!(ty_str(&inner), "u16");
}

#[test]
fn filter_result_nested_box_result_filter_strips_both() {
    let ty: Type = parse_quote!(Box<Result<f64, String>>);
    let result = filter_inner_type(&ty, &skip(&["Box", "Result"]));
    assert_eq!(ty_str(&result), "f64");
}

// ===========================================================================
// 5. filter_nested_* — filtering nested types (8 tests)
// ===========================================================================

#[test]
fn filter_nested_option_vec_strips_option_only() {
    let ty: Type = parse_quote!(Option<Vec<String>>);
    let result = filter_inner_type(&ty, &skip(&["Option"]));
    assert_eq!(ty_str(&result), "Vec < String >");
}

#[test]
fn filter_nested_box_arc_strips_both() {
    let ty: Type = parse_quote!(Box<Arc<i32>>);
    let result = filter_inner_type(&ty, &skip(&["Box", "Arc"]));
    assert_eq!(ty_str(&result), "i32");
}

#[test]
fn filter_nested_option_option_strips_both() {
    let ty: Type = parse_quote!(Option<Option<bool>>);
    let result = filter_inner_type(&ty, &skip(&["Option"]));
    assert_eq!(ty_str(&result), "bool");
}

#[test]
fn filter_nested_vec_vec_strips_both() {
    let ty: Type = parse_quote!(Vec<Vec<u8>>);
    let result = filter_inner_type(&ty, &skip(&["Vec"]));
    assert_eq!(ty_str(&result), "u8");
}

#[test]
fn filter_nested_box_box_box_strips_all() {
    let ty: Type = parse_quote!(Box<Box<Box<char>>>);
    let result = filter_inner_type(&ty, &skip(&["Box"]));
    assert_eq!(ty_str(&result), "char");
}

#[test]
fn filter_nested_arc_rc_cell_strips_all() {
    let ty: Type = parse_quote!(Arc<Rc<Cell<f64>>>);
    let result = filter_inner_type(&ty, &skip(&["Arc", "Rc", "Cell"]));
    assert_eq!(ty_str(&result), "f64");
}

#[test]
fn filter_nested_try_extract_through_two_skips() {
    let ty: Type = parse_quote!(Arc<Box<Vec<u32>>>);
    let (inner, ok) = try_extract_inner_type(&ty, "Vec", &skip(&["Arc", "Box"]));
    assert!(ok);
    assert_eq!(ty_str(&inner), "u32");
}

#[test]
fn filter_nested_wrap_option_vec_recursive() {
    let ty: Type = parse_quote!(Option<Vec<i32>>);
    let wrapped = wrap_leaf_type(&ty, &skip(&["Option", "Vec"]));
    assert_eq!(
        ty_str(&wrapped),
        "Option < Vec < adze :: WithLeaf < i32 > > >"
    );
}

// ===========================================================================
// 6. filter_custom_* — filtering custom wrapper types (8 tests)
// ===========================================================================

#[test]
fn filter_custom_wrapper_strips_outer() {
    let ty: Type = parse_quote!(Wrapper<String>);
    let result = filter_inner_type(&ty, &skip(&["Wrapper"]));
    assert_eq!(ty_str(&result), "String");
}

#[test]
fn filter_custom_arc_strips_inner() {
    let ty: Type = parse_quote!(Arc<u32>);
    let result = filter_inner_type(&ty, &skip(&["Arc"]));
    assert_eq!(ty_str(&result), "u32");
}

#[test]
fn filter_custom_rc_strips_inner() {
    let ty: Type = parse_quote!(Rc<bool>);
    let result = filter_inner_type(&ty, &skip(&["Rc"]));
    assert_eq!(ty_str(&result), "bool");
}

#[test]
fn filter_custom_cell_strips_inner() {
    let ty: Type = parse_quote!(Cell<i64>);
    let result = filter_inner_type(&ty, &skip(&["Cell"]));
    assert_eq!(ty_str(&result), "i64");
}

#[test]
fn filter_custom_refcell_strips_inner() {
    let ty: Type = parse_quote!(RefCell<f32>);
    let result = filter_inner_type(&ty, &skip(&["RefCell"]));
    assert_eq!(ty_str(&result), "f32");
}

#[test]
fn filter_custom_mutex_strips_inner() {
    let ty: Type = parse_quote!(Mutex<String>);
    let result = filter_inner_type(&ty, &skip(&["Mutex"]));
    assert_eq!(ty_str(&result), "String");
}

#[test]
fn filter_custom_try_extract_cow() {
    let ty: Type = parse_quote!(Cow<u8>);
    let (inner, ok) = try_extract_inner_type(&ty, "Cow", &skip(&[]));
    assert!(ok);
    assert_eq!(ty_str(&inner), "u8");
}

#[test]
fn filter_custom_wrap_leaf_pin() {
    let ty: Type = parse_quote!(Pin<i32>);
    let wrapped = wrap_leaf_type(&ty, &skip(&["Pin"]));
    assert_eq!(ty_str(&wrapped), "Pin < adze :: WithLeaf < i32 > >");
}

// ===========================================================================
// 7. filter_negative_* — types that don't match filter (8 tests)
// ===========================================================================

#[test]
fn filter_negative_vec_when_skip_is_box() {
    let ty: Type = parse_quote!(Vec<String>);
    let result = filter_inner_type(&ty, &skip(&["Box"]));
    assert_eq!(ty_str(&result), "Vec < String >");
}

#[test]
fn filter_negative_option_when_skip_is_vec() {
    let ty: Type = parse_quote!(Option<i32>);
    let result = filter_inner_type(&ty, &skip(&["Vec"]));
    assert_eq!(ty_str(&result), "Option < i32 >");
}

#[test]
fn filter_negative_box_when_skip_is_option() {
    let ty: Type = parse_quote!(Box<bool>);
    let result = filter_inner_type(&ty, &skip(&["Option"]));
    assert_eq!(ty_str(&result), "Box < bool >");
}

#[test]
fn filter_negative_plain_type_returns_unchanged() {
    let ty: Type = parse_quote!(String);
    let result = filter_inner_type(&ty, &skip(&["Vec", "Option", "Box"]));
    assert_eq!(ty_str(&result), "String");
}

#[test]
fn filter_negative_empty_skip_set() {
    let ty: Type = parse_quote!(Option<String>);
    let result = filter_inner_type(&ty, &skip(&[]));
    assert_eq!(ty_str(&result), "Option < String >");
}

#[test]
fn filter_negative_try_extract_mismatch_returns_original() {
    let ty: Type = parse_quote!(Vec<u8>);
    let (inner, ok) = try_extract_inner_type(&ty, "Option", &skip(&[]));
    assert!(!ok);
    assert_eq!(ty_str(&inner), "Vec < u8 >");
}

#[test]
fn filter_negative_plain_not_parameterized() {
    let ty: Type = parse_quote!(i32);
    assert!(!is_parameterized(&ty));
}

#[test]
fn filter_negative_wrap_plain_wraps_entire_type() {
    let ty: Type = parse_quote!(usize);
    let wrapped = wrap_leaf_type(&ty, &skip(&[]));
    assert_eq!(ty_str(&wrapped), "adze :: WithLeaf < usize >");
}

// ===========================================================================
// 8. filter_edge_* — edge cases (8 tests)
// ===========================================================================

#[test]
fn filter_edge_reference_type_unchanged() {
    let ty: Type = parse_quote!(&str);
    let result = filter_inner_type(&ty, &skip(&["Box", "Vec"]));
    assert_eq!(ty_str(&result), "& str");
}

#[test]
fn filter_edge_array_type_unchanged() {
    let ty: Type = parse_quote!([u8; 4]);
    let result = filter_inner_type(&ty, &skip(&["Vec"]));
    assert_eq!(ty_str(&result), "[u8 ; 4]");
}

#[test]
fn filter_edge_tuple_type_unchanged() {
    let ty: Type = parse_quote!((i32, u32));
    let result = filter_inner_type(&ty, &skip(&["Box"]));
    assert_eq!(ty_str(&result), "(i32 , u32)");
}

#[test]
fn filter_edge_tuple_not_parameterized() {
    let ty: Type = parse_quote!((bool, char));
    assert!(!is_parameterized(&ty));
}

#[test]
fn filter_edge_reference_try_extract_fails() {
    let ty: Type = parse_quote!(&str);
    let (inner, ok) = try_extract_inner_type(&ty, "Option", &skip(&[]));
    assert!(!ok);
    assert_eq!(ty_str(&inner), "& str");
}

#[test]
fn filter_edge_wrap_reference_wraps_entirely() {
    let ty: Type = parse_quote!(&str);
    let wrapped = wrap_leaf_type(&ty, &skip(&[]));
    assert_eq!(ty_str(&wrapped), "adze :: WithLeaf < & str >");
}

#[test]
fn filter_edge_wrap_array_wraps_entirely() {
    let ty: Type = parse_quote!([u8; 4]);
    let wrapped = wrap_leaf_type(&ty, &skip(&[]));
    assert_eq!(ty_str(&wrapped), "adze :: WithLeaf < [u8 ; 4] >");
}

#[test]
fn filter_edge_qualified_path_extract_uses_last_segment() {
    let ty: Type = parse_quote!(std::option::Option<i32>);
    let (inner, ok) = try_extract_inner_type(&ty, "Option", &skip(&[]));
    assert!(ok);
    assert_eq!(ty_str(&inner), "i32");
}
