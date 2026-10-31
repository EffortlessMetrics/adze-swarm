//! Regression contract for #1130, using the real macro expansion implementation.
//! These are expansion-level tests, not downstream rustc/runtime qualification.

#[path = "../src/errors.rs"]
mod errors;
#[path = "../src/expansion.rs"]
mod expansion;

use quote::ToTokens;
use syn::ItemMod;

fn numeric_field(attribute: &str) -> String {
    format!(
        "#[adze::grammar(\"authoring_diag\")]\nmod grammar {{\n    #[adze::language]\n    pub struct Number {{\n        {attribute}\n        pub value: u16,\n    }}\n}}"
    )
}

fn expand(source: &str) -> syn::Result<ItemMod> {
    let module = syn::parse_str(source).expect("fixture must have valid outer Rust syntax");
    expansion::expand_grammar(module)
}

fn error_on_line(source: &str, line: usize) -> syn::Error {
    let error = expand(source).expect_err("invalid leaf must fail real macro expansion");
    assert_eq!(
        error.span().start().line,
        line,
        "error must point to the user's leaf, not the grammar macro call: {error}"
    );
    error
}

#[test]
fn authoring_malformed_fallible_field_reports_its_source() {
    let source = numeric_field(r##"#[adze::leaf(pattern = "[0-9]+", try_transform =)]"##);
    error_on_line(&source, 5);
}

#[test]
fn authoring_missing_comma_is_not_silently_ignored() {
    let source = numeric_field(
        r##"#[adze::leaf(pattern = "[0-9]+" try_transform = |s| s.parse::<u16>())]"##,
    );
    error_on_line(&source, 5);
}

#[test]
fn authoring_conflicting_transforms_report_the_leaf_not_call_site() {
    let source = numeric_field(
        r##"#[adze::leaf(pattern = "[0-9]+", transform = |s: &str| s.len() as u16, try_transform = |s| s.parse::<u16>())]"##,
    );
    let error = error_on_line(&source, 5);
    assert!(error.to_string().contains("transform"));
    assert!(error.to_string().contains("try_transform"));
}

#[test]
fn authoring_malformed_unit_struct_leaf_reports_its_source() {
    let source = "#[adze::grammar(\"unit_diag\")]\nmod grammar {\n    #[adze::language]\n    #[adze::leaf(text = \"ok\", try_transform =)]\n    pub struct Token;\n}";
    error_on_line(source, 4);
}

#[test]
fn authoring_malformed_unit_variant_leaf_reports_its_source() {
    let source = "#[adze::grammar(\"variant_diag\")]\nmod grammar {\n    #[adze::language]\n    pub enum Token {\n        #[adze::leaf(text = \"ok\", try_transform =)]\n        Ok,\n    }\n}";
    error_on_line(source, 5);
}

#[test]
fn authoring_malformed_single_field_variant_leaf_reports_its_source() {
    let source = "#[adze::grammar(\"variant_value_diag\")]\nmod grammar {\n    #[adze::language]\n    pub enum Number {\n        #[adze::leaf(pattern = \"[0-9]+\", try_transform =)]\n        Value(u16),\n    }\n}";
    error_on_line(source, 5);
}

#[test]
fn authoring_valid_conversion_forms_still_expand() {
    let fallible = numeric_field(
        r##"#[adze::leaf(pattern = "[0-9]+", try_transform = |s| s.parse::<u16>())]"##,
    );
    let output = expand(&fallible).expect("valid fallible leaf");
    assert!(output.to_token_stream().to_string().contains("try_extract"));

    let infallible = numeric_field(
        r##"#[adze::leaf(pattern = "[0-9]+", transform = |s: &str| s.len() as u16)]"##,
    );
    let output = expand(&infallible).expect("valid infallible leaf");
    assert!(output.to_token_stream().to_string().contains("extract_field"));
}

#[test]
fn authoring_valid_literal_unit_leaf_still_expands() {
    let source = r##"
#[adze::grammar("literal_control")]
mod grammar {
    #[adze::language]
    pub struct Token {
        #[adze::leaf(text = "ok")]
        token: (),
    }
}
"##;
    let output = expand(source).expect("valid literal unit field");
    assert!(output.to_token_stream().to_string().contains("struct Token"));
}
