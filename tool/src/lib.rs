// Tool crate is mostly safe, with minimal unsafe for optimizations
#![deny(unsafe_op_in_unsafe_fn)]
#![cfg_attr(feature = "strict_docs", allow(missing_docs))]
#![cfg_attr(not(feature = "strict_docs"), allow(missing_docs))]

//! Build tool for adze parser generation

use serde_json::Value;
use syn::{Item, parse_quote};

mod expansion;
use expansion::*;

mod grammar_converter;
/// Re-exported grammar format converter.
pub use grammar_converter::GrammarConverter;

/// Grammar and parse tree visualization tools.
pub mod visualization;
/// Re-exported grammar visualizer.
pub use visualization::GrammarVisualizer;

/// JavaScript grammar.js parsing and conversion.
pub mod grammar_js;
/// Re-exported grammar.js converter and parser.
pub use grammar_js::{GrammarJsConverter, parse_grammar_js};

/// Pure-Rust parser builder bypassing C code generation.
pub mod pure_rust_builder;
/// Re-exported builder types and entry points.
pub use pure_rust_builder::{
    BuildOptions, BuildResult, build_parser, build_parser_for_crate, build_parser_from_grammar_js,
};

/// Command-line interface for the adze build tool.
pub mod cli;
/// Build system integration for external scanners.
pub mod scanner_build;

/// Error types for the build tool.
pub mod error;
/// Re-exported error types.
pub use error::{Result as ToolResult, ToolError};

// Use tree-sitter-generate's version for compatibility
// Version 0.25.1 is what we depend on in Cargo.toml
pub(crate) const GENERATED_SEMANTIC_VERSION: Option<(u8, u8, u8)> = Some((0, 25, 1));

/// Generates JSON strings defining Tree Sitter grammars for every Adze
/// grammar found in the given module and recursive submodules.
pub fn generate_grammars(root_file: &Path) -> ToolResult<Vec<Value>> {
    let root_file = syn_inline_mod::parse_and_inline_modules(root_file).items;
    let mut out = vec![];
    for i in root_file.iter() {
        generate_all_grammars(i, &mut out)?;
    }
    Ok(out)
}

fn generate_all_grammars(item: &Item, out: &mut Vec<Value>) -> ToolResult<()> {
    if let Item::Mod(m) = item {
        if let Some((_, items)) = &m.content {
            for item in items {
                generate_all_grammars(item, out)?;
            }
        }

        if m.attrs
            .iter()
            .any(|a| a.path() == &parse_quote!(adze::grammar))
        {
            out.push(generate_grammar(m)?);
        }
    }
    Ok(())
}

#[cfg(feature = "build_parsers")]
mod build_parsers;
#[cfg(feature = "build_parsers")]
pub use build_parsers::build_parsers;

use std::path::Path;
#[cfg(test)]
mod tests {
    use syn::parse_quote;

    use super::{GENERATED_SEMANTIC_VERSION, generate_grammar, generate_grammars};
    use tree_sitter_generate::generate_parser_for_grammar;

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

    #[test]
    fn generate_grammars_collects_nested_annotated_modules_only() {
        let dir = tempfile::tempdir().expect("temp dir");
        let root = dir.path().join("lib.rs");
        std::fs::write(
            &root,
            r##"
            mod ignored {
                pub struct NotAGrammar;
            }

            pub mod outer {
                #[adze::grammar("first")]
                pub mod first {
                    #[adze::language]
                    pub struct Root {
                        #[adze::leaf(text = "a")]
                        a: (),
                    }
                }

                pub mod nested {
                    #[adze::grammar("second")]
                    pub mod second {
                        #[adze::language]
                        pub struct Root {
                            #[adze::leaf(pattern = r"[b]")]
                            b: (),
                        }
                    }
                }
            }
            "##,
        )
        .expect("write root module");

        let grammars = generate_grammars(&root).expect("generate grammars");
        let names: Vec<_> = grammars
            .iter()
            .map(|grammar| grammar["name"].as_str().expect("grammar name"))
            .collect();

        assert_eq!(names, ["first", "second"]);
        assert_eq!(grammars[0]["rules"]["source_file"]["name"], "Root");
        assert_eq!(grammars[1]["rules"]["source_file"]["name"], "Root");
    }

    #[test]
    fn generate_grammars_inlines_external_modules() {
        let dir = tempfile::tempdir().expect("temp dir");
        let root = dir.path().join("lib.rs");
        let nested_dir = dir.path().join("nested");
        std::fs::create_dir(&nested_dir).expect("create nested module dir");
        std::fs::write(
            &root,
            r##"
            mod inline_grammar;
            mod nested;
            "##,
        )
        .expect("write root module");
        std::fs::write(
            dir.path().join("inline_grammar.rs"),
            r##"
            #[adze::grammar("external_file")]
            pub mod grammar {
                #[adze::language]
                pub struct Root {
                    #[adze::leaf(text = "x")]
                    x: (),
                }
            }
            "##,
        )
        .expect("write external module");
        std::fs::write(
            nested_dir.join("mod.rs"),
            r##"
            #[adze::grammar("nested_mod_rs")]
            pub mod grammar {
                #[adze::language]
                pub struct Root {
                    #[adze::leaf(text = "y")]
                    y: (),
                }
            }
            "##,
        )
        .expect("write nested module");

        let grammars = generate_grammars(&root).expect("generate grammars");
        let names: Vec<_> = grammars
            .iter()
            .map(|grammar| grammar["name"].as_str().expect("grammar name"))
            .collect();

        assert_eq!(names, ["external_file", "nested_mod_rs"]);
    }

    #[test]
    fn enum_with_named_field() {
        let m = if let syn::Item::Mod(m) = parse_quote! {
            #[adze::grammar("test")]
            mod grammar {
                #[adze::language]
                pub enum Expr {
                    Number(
                            #[adze::leaf(pattern = r"\d+", transform = |v| v.parse().unwrap())]
                            u32
                    ),
                    Neg {
                        #[adze::leaf(text = "!")]
                        _bang: (),
                        value: Box<Expr>,
                    }
                }
            }
        } {
            m
        } else {
            panic!()
        };

        let grammar = generate_grammar(&m).expect("Failed to generate grammar");
        insta::assert_snapshot!(grammar);
        generate_parser_for_grammar(
            &serde_json::to_string(&grammar).unwrap(),
            GENERATED_SEMANTIC_VERSION,
        )
        .unwrap();
    }

    #[test]
    fn enum_transformed_fields() {
        let m = if let syn::Item::Mod(m) = parse_quote! {
            #[adze::grammar("test")]
            mod grammar {
                #[adze::language]
                pub enum Expression {
                    Number(
                        #[adze::leaf(pattern = r"\d+", transform = |v: &str| v.parse::<i32>().unwrap())]
                        i32
                    ),
                }
            }
        } {
            m
        } else {
            panic!()
        };

        let grammar = generate_grammar(&m).expect("Failed to generate grammar");
        insta::assert_snapshot!(grammar);
        generate_parser_for_grammar(
            &serde_json::to_string(&grammar).unwrap(),
            GENERATED_SEMANTIC_VERSION,
        )
        .unwrap();
    }

    #[test]
    fn enum_recursive() {
        let m = if let syn::Item::Mod(m) = parse_quote! {
            #[adze::grammar("test")]
            mod grammar {
                #[adze::language]
                pub enum Expression {
                    Number(
                        #[adze::leaf(pattern = r"\d+", transform = |v: &str| v.parse::<i32>().unwrap())]
                        i32
                    ),
                    Neg(
                        #[adze::leaf(text = "-", transform = |v| ())]
                        (),
                        Box<Expression>
                    ),
                }
            }
        } {
            m
        } else {
            panic!()
        };

        let grammar = generate_grammar(&m).expect("Failed to generate grammar");
        insta::assert_snapshot!(grammar);
        generate_parser_for_grammar(
            &serde_json::to_string(&grammar).unwrap(),
            GENERATED_SEMANTIC_VERSION,
        )
        .unwrap();
    }

    #[test]
    fn enum_prec_left() {
        let m = if let syn::Item::Mod(m) = parse_quote! {
            #[adze::grammar("test")]
            mod grammar {
                #[adze::language]
                pub enum Expression {
                    Number(
                        #[adze::leaf(pattern = r"\d+", transform = |v: &str| v.parse::<i32>().unwrap())]
                        i32
                    ),
                    #[adze::prec_left(1)]
                    Sub(
                        Box<Expression>,
                        #[adze::leaf(text = "-", transform = |v| ())]
                        (),
                        Box<Expression>
                    ),
                }
            }
        } {
            m
        } else {
            panic!()
        };

        let grammar = generate_grammar(&m).expect("Failed to generate grammar");
        insta::assert_snapshot!(grammar);
        generate_parser_for_grammar(
            &serde_json::to_string(&grammar).unwrap(),
            GENERATED_SEMANTIC_VERSION,
        )
        .unwrap();
    }

    #[test]
    fn enum_prec_left_preserves_fielded_inlined_operator() {
        let m = if let syn::Item::Mod(m) = parse_quote! {
            #[adze::grammar("test")]
            mod grammar {
                #[adze::language]
                pub enum Expression {
                    Number(
                        #[adze::leaf(pattern = r"\d+", transform = |v: &str| v.parse::<i32>().unwrap())]
                        i32
                    ),
                    #[adze::prec_left(1)]
                    Add {
                        #[adze::field("left")]
                        left: Box<Expression>,
                        #[adze::field("operator")]
                        #[adze::leaf(text = "+", transform = |v| ())]
                        operator: (),
                        #[adze::field("right")]
                        right: Box<Expression>,
                    },
                }
            }
        } {
            m
        } else {
            panic!()
        };

        let grammar = generate_grammar(&m).expect("Failed to generate grammar");
        let add_rule = &grammar["rules"]["Expression_Add"];

        assert!(
            contains_fielded_string(add_rule, "operator", "+"),
            "precedence operator inlining should preserve explicit FIELD metadata"
        );
        generate_parser_for_grammar(
            &serde_json::to_string(&grammar).unwrap(),
            GENERATED_SEMANTIC_VERSION,
        )
        .unwrap();
    }

    #[test]
    fn grammar_with_extras() {
        let m = if let syn::Item::Mod(m) = parse_quote! {
            #[adze::grammar("test")]
            mod grammar {
                #[adze::language]
                pub enum Expression {
                    Number(
                        #[adze::leaf(pattern = r"\d+", transform = |v: &str| v.parse::<i32>().unwrap())]
                        i32
                    ),
                }

                #[adze::extra]
                struct Whitespace {
                    #[adze::leaf(pattern = r"\s", transform = |_v| ())]
                    _whitespace: (),
                }
            }
        } {
            m
        } else {
            panic!()
        };

        let grammar = generate_grammar(&m).expect("Failed to generate grammar");
        insta::assert_snapshot!(grammar);
        generate_parser_for_grammar(
            &serde_json::to_string(&grammar).unwrap(),
            GENERATED_SEMANTIC_VERSION,
        )
        .unwrap();
    }

    #[test]
    fn grammar_unboxed_field() {
        let m = if let syn::Item::Mod(m) = parse_quote! {
            #[adze::grammar("test")]
            mod grammar {
                #[adze::language]
                pub struct Language {
                    e: Expression,
                }

                pub enum Expression {
                    Number(
                        #[adze::leaf(pattern = r"\d+", transform = |v: &str| v.parse::<i32>().unwrap())]
                        i32
                    ),
                }
            }
        } {
            m
        } else {
            panic!()
        };

        let grammar = generate_grammar(&m).expect("Failed to generate grammar");
        insta::assert_snapshot!(grammar);
        generate_parser_for_grammar(
            &serde_json::to_string(&grammar).unwrap(),
            GENERATED_SEMANTIC_VERSION,
        )
        .unwrap();
    }

    fn contains_fielded_string(value: &serde_json::Value, field_name: &str, text: &str) -> bool {
        let Some(object) = value.as_object() else {
            return false;
        };

        if object.get("type").and_then(serde_json::Value::as_str) == Some("FIELD")
            && object.get("name").and_then(serde_json::Value::as_str) == Some(field_name)
            && object
                .get("content")
                .and_then(|content| content.get("type"))
                .and_then(serde_json::Value::as_str)
                == Some("STRING")
            && object
                .get("content")
                .and_then(|content| content.get("value"))
                .and_then(serde_json::Value::as_str)
                == Some(text)
        {
            return true;
        }

        object.values().any(|child| {
            if let Some(children) = child.as_array() {
                children
                    .iter()
                    .any(|item| contains_fielded_string(item, field_name, text))
            } else {
                contains_fielded_string(child, field_name, text)
            }
        })
    }

    #[test]
    fn grammar_repeat() {
        let m = if let syn::Item::Mod(m) = parse_quote! {
            #[adze::grammar("test")]
            pub mod grammar {
                #[adze::language]
                pub struct NumberList {
                    #[adze::repeat(non_empty = true)]
                    #[adze::delimited(
                        #[adze::leaf(text = ",")]
                        ()
                    )]
                    numbers: Vec<Number>,
                }

                pub struct Number {
                    #[adze::leaf(pattern = r"\d+", transform = |v| v.parse().unwrap())]
                    v: i32,
                }

                #[adze::extra]
                struct Whitespace {
                    #[adze::leaf(pattern = r"\s")]
                    _whitespace: (),
                }
            }
        } {
            m
        } else {
            panic!()
        };

        let grammar = generate_grammar(&m).expect("Failed to generate grammar");
        insta::assert_snapshot!(grammar);
        generate_parser_for_grammar(
            &serde_json::to_string(&grammar).unwrap(),
            GENERATED_SEMANTIC_VERSION,
        )
        .unwrap();
    }

    #[test]
    fn grammar_repeat_no_delimiter() {
        let m = if let syn::Item::Mod(m) = parse_quote! {
            #[adze::grammar("test")]
            pub mod grammar {
                #[adze::language]
                pub struct NumberList {
                    #[adze::repeat(non_empty = true)]
                    numbers: Vec<Number>,
                }

                pub struct Number {
                    #[adze::leaf(pattern = r"\d+", transform = |v| v.parse().unwrap())]
                    v: i32,
                }

                #[adze::extra]
                struct Whitespace {
                    #[adze::leaf(pattern = r"\s")]
                    _whitespace: (),
                }
            }
        } {
            m
        } else {
            panic!()
        };

        let grammar = generate_grammar(&m).expect("Failed to generate grammar");
        insta::assert_snapshot!(grammar);
        generate_parser_for_grammar(
            &serde_json::to_string(&grammar).unwrap(),
            GENERATED_SEMANTIC_VERSION,
        )
        .unwrap();
    }

    #[test]
    fn grammar_repeat1() {
        let m = if let syn::Item::Mod(m) = parse_quote! {
            #[adze::grammar("test")]
            pub mod grammar {
                #[adze::language]
                pub struct NumberList {
                    #[adze::repeat(non_empty = true)]
                    #[adze::delimited(
                        #[adze::leaf(text = ",")]
                        ()
                    )]
                    numbers: Vec<Number>,
                }

                pub struct Number {
                    #[adze::leaf(pattern = r"\d+", transform = |v| v.parse().unwrap())]
                    v: i32,
                }

                #[adze::extra]
                struct Whitespace {
                    #[adze::leaf(pattern = r"\s")]
                    _whitespace: (),
                }
            }
        } {
            m
        } else {
            panic!()
        };

        let grammar = generate_grammar(&m).expect("Failed to generate grammar");
        insta::assert_snapshot!(grammar);
        generate_parser_for_grammar(
            &serde_json::to_string(&grammar).unwrap(),
            GENERATED_SEMANTIC_VERSION,
        )
        .unwrap();
    }

    #[test]
    fn struct_optional() {
        let m = if let syn::Item::Mod(m) = parse_quote! {
            #[adze::grammar("test")]
            mod grammar {
                #[adze::language]
                pub struct Language {
                    #[adze::leaf(pattern = r"\d+", transform = |v| v.parse().unwrap())]
                    v: Option<i32>,
                    #[adze::leaf(pattern = r" ", transform = |v| ())]
                    space: (),
                    t: Option<Number>,
                }

                pub struct Number {
                    #[adze::leaf(pattern = r"\d+", transform = |v| v.parse().unwrap())]
                    v: i32
                }
            }
        } {
            m
        } else {
            panic!()
        };

        let grammar = generate_grammar(&m).expect("Failed to generate grammar");
        insta::assert_snapshot!(grammar);
        generate_parser_for_grammar(
            &serde_json::to_string(&grammar).unwrap(),
            GENERATED_SEMANTIC_VERSION,
        )
        .unwrap();
    }

    #[test]
    fn enum_with_unamed_vector() {
        let m = if let syn::Item::Mod(m) = parse_quote! {
            #[adze::grammar("test")]
            mod grammar {
                pub struct Number {
                        #[adze::leaf(pattern = r"\d+", transform = |v| v.parse().unwrap())]
                        value: u32
                }

                #[adze::language]
                pub enum Expr {
                    Numbers(
                        #[adze::repeat(non_empty = true)]
                        Vec<Number>
                    )
                }
            }
        } {
            m
        } else {
            panic!()
        };

        let grammar = generate_grammar(&m).expect("Failed to generate grammar");
        insta::assert_snapshot!(grammar);
        generate_parser_for_grammar(
            &serde_json::to_string(&grammar).unwrap(),
            GENERATED_SEMANTIC_VERSION,
        )
        .unwrap();
    }

    #[test]
    fn spanned_in_vec() {
        let m = if let syn::Item::Mod(m) = parse_quote! {
            #[adze::grammar("test")]
            mod grammar {
                use adze::Spanned;

                #[adze::language]
                pub struct NumberList {
                    #[adze::repeat(non_empty = true)]
                    #[adze::leaf(pattern = r"\d+", transform = |v| v.parse().unwrap())]
                    numbers: Vec<Spanned<i32>>,
                }

                #[adze::extra]
                struct Whitespace {
                    #[adze::leaf(pattern = r"\s")]
                    _whitespace: (),
                }
            }
        } {
            m
        } else {
            panic!()
        };

        let grammar = generate_grammar(&m).expect("Failed to generate grammar");
        insta::assert_snapshot!(grammar);
        generate_parser_for_grammar(
            &serde_json::to_string(&grammar).unwrap(),
            GENERATED_SEMANTIC_VERSION,
        )
        .unwrap();
    }

    /// CRITICAL BUG REPRODUCTION: Test Binary variant generation with inlining
    /// https://github.com/EffortlessMetrics/adze/issues/BINARY_VARIANT_MISSING
    #[test]
    fn test_binary_variant_inlined_generation() {
        // This test reproduces the critical bug where Binary variant disappears
        let m = if let syn::Item::Mod(m) = parse_quote! {
            #[adze::grammar("test_binary")]
            pub mod grammar {
                #[adze::language]
                #[derive(Debug)]
                pub enum Expr {
                    Binary(
                        Box<Expr>,
                        #[adze::leaf(pattern = r"[-+*/]")] String,
                        Box<Expr>,
                    ),
                    Number(#[adze::leaf(pattern = r"\d+")] i32),
                }

                /// Whitespace handling - match real ambiguous_expr grammar
                #[adze::extra]
                struct Whitespace {
                    #[adze::leaf(pattern = r"\s")]
                    _whitespace: (),
                }
            }
        } {
            m
        } else {
            panic!("Failed to parse test module")
        };

        debug_trace!("\n=== Testing Binary Variant Inlined Generation ===\n");

        let grammar = generate_grammar(&m).expect("Failed to generate grammar");
        debug_trace!(
            "Generated grammar:\n{}",
            serde_json::to_string_pretty(&grammar).unwrap()
        );

        // Extract rules
        let rules = grammar.get("rules").expect("No rules in grammar");
        let rules_obj = rules.as_object().expect("Rules not an object");

        debug_trace!("\n=== All Rules ===");
        for (name, _rule) in rules_obj {
            debug_trace!("  - {}", name);
        }

        // Find the Expr rule
        let expr_rule = rules_obj.get("Expr").expect("No Expr rule found!");
        debug_trace!(
            "\n=== Expr Rule ===\n{}",
            serde_json::to_string_pretty(expr_rule).unwrap()
        );

        // Expr should be a CHOICE
        let expr_type = expr_rule.get("type").and_then(serde_json::Value::as_str);
        assert_eq!(expr_type, Some("CHOICE"), "Expr should be a CHOICE");

        // Get CHOICE members
        let members = expr_rule.get("members").expect("No members in Expr CHOICE");
        let members_array = members.as_array().expect("Members not an array");

        debug_trace!("\n=== Expr CHOICE Members ({}) ===", members_array.len());
        for (i, member) in members_array.iter().enumerate() {
            debug_trace!(
                "Member {}:\n{}",
                i,
                serde_json::to_string_pretty(member).unwrap()
            );
        }

        // CRITICAL ASSERTION: Expr CHOICE should have 2 members (Binary + Number)
        assert_eq!(
            members_array.len(),
            2,
            "CONTRACT VIOLATION: Expr should have 2 CHOICE members (Binary + Number), found {}.\n\
             This indicates the Binary variant is missing from grammar generation!",
            members_array.len()
        );

        // Check first member (should be Binary - a SEQ with 3 fields)
        let binary_member = &members_array[0];
        let binary_type = binary_member
            .get("type")
            .and_then(serde_json::Value::as_str);

        assert_eq!(
            binary_type,
            Some("SEQ"),
            "Binary variant should be inlined as SEQ, got: {:?}",
            binary_type
        );

        // Binary SEQ should have 3 members (Expr, Op, Expr)
        let binary_members = binary_member
            .get("members")
            .expect("No members in Binary SEQ");
        let binary_members_array = binary_members
            .as_array()
            .expect("Binary members not an array");

        assert_eq!(
            binary_members_array.len(),
            3,
            "Binary SEQ should have 3 members (Expr, Op, Expr), found {}",
            binary_members_array.len()
        );

        // Check second member (should be Number - a PATTERN)
        let number_member = &members_array[1];
        let number_type = number_member
            .get("type")
            .and_then(serde_json::Value::as_str);

        assert_eq!(
            number_type,
            Some("PATTERN"),
            "Number variant should be inlined as PATTERN, got: {:?}",
            number_type
        );

        debug_trace!("\n✅ TEST PASSED: Binary variant generates correctly!\n");
    }

    #[test]
    fn single_field_non_leaf_variants_preserve_identity_for_reduce_reduce() {
        let m = if let syn::Item::Mod(m) = parse_quote! {
            #[adze::grammar("reduce_reduce")]
            pub mod grammar {
                #[adze::language]
                pub enum Choice {
                    FromA(FromA),
                    FromB(FromB),
                }

                #[adze::language]
                pub struct FromA(#[adze::leaf(text = "x")] ());

                #[adze::language]
                pub struct FromB(#[adze::leaf(text = "x")] ());
            }
        } {
            m
        } else {
            panic!("Failed to parse test module")
        };

        let grammar = generate_grammar(&m).expect("Failed to generate grammar");
        let rules = grammar["rules"].as_object().expect("rules object");
        let choice = rules.get("Choice").expect("Choice rule");
        let members = choice["members"].as_array().expect("Choice members");

        let member_names: Vec<_> = members
            .iter()
            .map(|member| member["name"].as_str().expect("symbol member"))
            .collect();

        assert_eq!(
            member_names,
            ["Choice_FromA", "Choice_FromB"],
            "single-field non-leaf variants must keep wrapper symbols so \
             generated reduce/reduce conflicts survive table generation"
        );
        assert!(
            rules.contains_key("Choice_FromA"),
            "Choice_FromA wrapper rule should be generated"
        );
        assert!(
            rules.contains_key("Choice_FromB"),
            "Choice_FromB wrapper rule should be generated"
        );

        generate_parser_for_grammar(
            &serde_json::to_string(&grammar).unwrap(),
            GENERATED_SEMANTIC_VERSION,
        )
        .unwrap();
    }

    #[cfg(feature = "build_parsers")]
    #[test]
    fn test_emit_artifacts_functionality() {
        use std::env;
        use std::path::Path;

        // Set up test environment
        let original_target = env::var("TARGET").ok();
        let original_out_dir = env::var("OUT_DIR").ok();
        let original_emit = env::var("ADZE_EMIT_ARTIFACTS").ok();
        let original_opt_level = env::var("OPT_LEVEL").ok();
        let original_host = env::var("HOST").ok();
        let original_profile = env::var("PROFILE").ok();

        // Set required environment variables for the current platform
        let target = if cfg!(target_os = "windows") {
            "x86_64-pc-windows-msvc"
        } else if cfg!(target_os = "macos") {
            "x86_64-apple-darwin"
        } else {
            "x86_64-unknown-linux-gnu"
        };

        unsafe {
            env::set_var("TARGET", target);
            env::set_var("OPT_LEVEL", "0");
            env::set_var("HOST", target);
            env::set_var("PROFILE", "debug");
            env::set_var("ADZE_EMIT_ARTIFACTS", "true");
        }

        let test_dir = "./test_emit_artifacts_output";
        std::fs::create_dir_all(test_dir).unwrap();
        unsafe {
            env::set_var("OUT_DIR", test_dir);
        }

        // Create a simple test grammar file
        let test_grammar = r#"
#[adze::grammar("test_emit")]
mod grammar {
    #[adze::language]
    pub enum Expression {
        Number(
            #[adze::leaf(pattern = r"\d+", transform = |v: &str| v.parse::<i32>().unwrap())]
            i32
        ),
    }
}
"#;

        let grammar_file = "test_emit_grammar.rs";
        std::fs::write(grammar_file, test_grammar).unwrap();

        // Test that build_parsers doesn't panic with ADZE_EMIT_ARTIFACTS=true
        let result = std::panic::catch_unwind(|| {
            super::build_parsers(Path::new(grammar_file));
        });

        // Clean up
        let _ = std::fs::remove_file(grammar_file);
        let _ = std::fs::remove_dir_all(test_dir);

        // Restore original environment variables
        unsafe {
            match original_target {
                Some(val) => env::set_var("TARGET", val),
                None => env::remove_var("TARGET"),
            }
        }
        unsafe {
            match original_out_dir {
                Some(val) => env::set_var("OUT_DIR", val),
                None => env::remove_var("OUT_DIR"),
            }
        }
        unsafe {
            match original_emit {
                Some(val) => env::set_var("ADZE_EMIT_ARTIFACTS", val),
                None => env::remove_var("ADZE_EMIT_ARTIFACTS"),
            }
        }
        unsafe {
            match original_opt_level {
                Some(val) => env::set_var("OPT_LEVEL", val),
                None => env::remove_var("OPT_LEVEL"),
            }
        }
        unsafe {
            match original_host {
                Some(val) => env::set_var("HOST", val),
                None => env::remove_var("HOST"),
            }
        }
        unsafe {
            match original_profile {
                Some(val) => env::set_var("PROFILE", val),
                None => env::remove_var("PROFILE"),
            }
        }

        // Assert that the function completed successfully
        assert!(
            result.is_ok(),
            "build_parsers should not panic with ADZE_EMIT_ARTIFACTS=true"
        );
    }
}
