#![allow(
    clippy::needless_range_loop,
    reason = "property and comprehensive tests use index-based loops to exercise table positions and boundary cases"
)]

//! Comprehensive tests for rule generation in adze-tool's grammar output.
//!
//! Validates that Rust type annotations are correctly converted into
//! Tree-sitter grammar rules: FIELD, SEQ, CHOICE, REPEAT, PREC, STRING, PATTERN.

use std::fs;
use tempfile::TempDir;

/// Helper: write Rust source to a temp file and extract exactly one grammar.
fn extract_one(src: &str) -> serde_json::Value {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("lib.rs");
    fs::write(&path, src).unwrap();
    let gs = adze_tool::generate_grammars(&path).unwrap();
    assert_eq!(gs.len(), 1, "expected exactly one grammar");
    gs.into_iter().next().unwrap()
}

// ---------------------------------------------------------------------------
// 1. Simple field → FIELD rule
// ---------------------------------------------------------------------------

#[test]
fn single_field_struct_generates_field_rule() {
    let g = extract_one(
        r#"
        #[adze::grammar("field_single")]
        mod grammar {
            #[adze::language]
            pub struct Root {
                #[adze::leaf(pattern = r"[a-z]+")]
                pub name: String,
            }
        }
        "#,
    );
    let root = &g["rules"]["Root"];
    assert_eq!(root["type"].as_str().unwrap(), "FIELD");
    assert_eq!(root["name"].as_str().unwrap(), "name");
    assert_eq!(root["content"]["type"].as_str().unwrap(), "SYMBOL");
    assert_eq!(root["content"]["name"].as_str().unwrap(), "Root_name");
}

#[test]
fn field_content_references_generated_symbol() {
    let g = extract_one(
        r#"
        #[adze::grammar("field_ref")]
        mod grammar {
            #[adze::language]
            pub struct Root {
                #[adze::leaf(pattern = r"\d+")]
                pub val: String,
            }
        }
        "#,
    );
    let rules = g["rules"].as_object().unwrap();
    // FIELD content references Root_val, which should exist as a PATTERN rule
    assert!(rules.contains_key("Root_val"));
    assert_eq!(rules["Root_val"]["type"].as_str().unwrap(), "PATTERN");
}

#[test]
fn field_references_named_type_as_symbol() {
    let g = extract_one(
        r#"
        #[adze::grammar("field_type_ref")]
        mod grammar {
            #[adze::language]
            pub struct Root {
                pub child: Child,
            }
            pub struct Child {
                #[adze::leaf(pattern = r"[a-z]+")]
                pub name: String,
            }
        }
        "#,
    );
    let root = &g["rules"]["Root"];
    assert_eq!(root["type"].as_str().unwrap(), "FIELD");
    // The field content should be a SYMBOL referencing "Child"
    assert_eq!(root["content"]["type"].as_str().unwrap(), "SYMBOL");
    assert_eq!(root["content"]["name"].as_str().unwrap(), "Child");
}

// ---------------------------------------------------------------------------
// 2. Multiple fields → SEQ rule
// ---------------------------------------------------------------------------

#[test]
fn two_fields_generate_seq() {
    let g = extract_one(
        r#"
        #[adze::grammar("seq_two")]
        mod grammar {
            #[adze::language]
            pub struct Root {
                #[adze::leaf(pattern = r"[a-z]+")]
                pub left: String,
                #[adze::leaf(pattern = r"\d+")]
                pub right: String,
            }
        }
        "#,
    );
    let root = &g["rules"]["Root"];
    assert_eq!(root["type"].as_str().unwrap(), "SEQ");
    let members = root["members"].as_array().unwrap();
    assert_eq!(members.len(), 2);
    assert_eq!(members[0]["type"].as_str().unwrap(), "FIELD");
    assert_eq!(members[1]["type"].as_str().unwrap(), "FIELD");
}

#[test]
fn three_fields_generate_seq_with_three_members() {
    let g = extract_one(
        r#"
        #[adze::grammar("seq_three")]
        mod grammar {
            #[adze::language]
            pub struct Triple {
                #[adze::leaf(text = "(")]
                pub open: String,
                #[adze::leaf(pattern = r"\d+")]
                pub val: String,
                #[adze::leaf(text = ")")]
                pub close: String,
            }
        }
        "#,
    );
    let rule = &g["rules"]["Triple"];
    assert_eq!(rule["type"].as_str().unwrap(), "SEQ");
    assert_eq!(rule["members"].as_array().unwrap().len(), 3);
}

#[test]
fn seq_members_preserve_field_order() {
    let g = extract_one(
        r#"
        #[adze::grammar("seq_order")]
        mod grammar {
            #[adze::language]
            pub struct Stmt {
                #[adze::leaf(text = "let")]
                pub kw: String,
                #[adze::leaf(pattern = r"[a-z]+")]
                pub name: String,
                #[adze::leaf(text = "=")]
                pub eq: String,
                #[adze::leaf(pattern = r"\d+")]
                pub val: String,
            }
        }
        "#,
    );
    let members = g["rules"]["Stmt"]["members"].as_array().unwrap();
    assert_eq!(members.len(), 4);
    assert_eq!(members[0]["name"].as_str().unwrap(), "kw");
    assert_eq!(members[1]["name"].as_str().unwrap(), "name");
    assert_eq!(members[2]["name"].as_str().unwrap(), "eq");
    assert_eq!(members[3]["name"].as_str().unwrap(), "val");
}

// ---------------------------------------------------------------------------
// 3. Optional field → CHOICE(field, BLANK)
// ---------------------------------------------------------------------------

#[test]
fn option_field_wraps_in_choice_with_blank() {
    let g = extract_one(
        r#"
        #[adze::grammar("opt_blank")]
        mod grammar {
            #[adze::language]
            pub struct Root {
                #[adze::leaf(pattern = r"[a-z]+")]
                pub required: String,
                #[adze::leaf(pattern = r"\d+")]
                pub maybe: Option<String>,
            }
        }
        "#,
    );
    let root = &g["rules"]["Root"];
    assert_eq!(root["type"].as_str().unwrap(), "SEQ");
    let members = root["members"].as_array().unwrap();
    // First member is a plain FIELD
    assert_eq!(members[0]["type"].as_str().unwrap(), "FIELD");
    // Second member is CHOICE with BLANK
    let opt = &members[1];
    assert_eq!(opt["type"].as_str().unwrap(), "CHOICE");
    let choices = opt["members"].as_array().unwrap();
    assert!(choices.iter().any(|c| c["type"].as_str() == Some("BLANK")));
    assert!(choices.iter().any(|c| c["type"].as_str() == Some("FIELD")));
}

#[test]
fn option_choice_has_exactly_two_members() {
    let g = extract_one(
        r#"
        #[adze::grammar("opt_two")]
        mod grammar {
            #[adze::language]
            pub struct Root {
                #[adze::leaf(pattern = r"[a-z]+")]
                pub required: String,
                #[adze::leaf(pattern = r"\d+")]
                pub opt_num: Option<String>,
            }
        }
        "#,
    );
    let members = g["rules"]["Root"]["members"].as_array().unwrap();
    let opt = &members[1];
    assert_eq!(opt["type"].as_str().unwrap(), "CHOICE");
    assert_eq!(opt["members"].as_array().unwrap().len(), 2);
}

#[test]
fn option_field_blank_is_first_choice_member() {
    let g = extract_one(
        r#"
        #[adze::grammar("opt_blank_first")]
        mod grammar {
            #[adze::language]
            pub struct Root {
                #[adze::leaf(pattern = r"[a-z]+")]
                pub req: String,
                #[adze::leaf(pattern = r"\d+")]
                pub opt_val: Option<String>,
            }
        }
        "#,
    );
    let members = g["rules"]["Root"]["members"].as_array().unwrap();
    let opt_choices = members[1]["members"].as_array().unwrap();
    assert_eq!(opt_choices[0]["type"].as_str().unwrap(), "BLANK");
}

// ---------------------------------------------------------------------------
// 4. Vec field → REPEAT
// ---------------------------------------------------------------------------

#[test]
fn vec_field_generates_repeat1_contents_rule() {
    let g = extract_one(
        r#"
        #[adze::grammar("vec_repeat")]
        mod grammar {
            #[adze::language]
            pub struct Root {
                #[adze::leaf(pattern = r"[a-z]+")]
                pub items: Vec<String>,
            }
        }
        "#,
    );
    let rules = g["rules"].as_object().unwrap();
    let contents = &rules["Root_items_vec_contents"];
    assert_eq!(contents["type"].as_str().unwrap(), "REPEAT1");
}

#[test]
fn vec_field_default_wraps_reference_in_choice_for_empty() {
    let g = extract_one(
        r#"
        #[adze::grammar("vec_empty")]
        mod grammar {
            #[adze::language]
            pub struct Root {
                #[adze::leaf(pattern = r"[a-z]+")]
                pub items: Vec<String>,
            }
        }
        "#,
    );
    // Root rule references vec_contents wrapped in CHOICE(BLANK, SYMBOL)
    let root = &g["rules"]["Root"];
    // The root is a FIELD whose content is a CHOICE
    let content = &root["content"];
    assert_eq!(
        content["type"].as_str().unwrap(),
        "CHOICE",
        "default Vec should wrap in CHOICE for empty, got: {}",
        serde_json::to_string_pretty(root).unwrap()
    );
    let choices = content["members"].as_array().unwrap();
    assert!(choices.iter().any(|c| c["type"].as_str() == Some("BLANK")));
    assert!(choices.iter().any(|c| c["type"].as_str() == Some("SYMBOL")));
}

#[test]
fn vec_non_empty_references_symbol_directly() {
    let g = extract_one(
        r#"
        #[adze::grammar("vec_nonempty")]
        mod grammar {
            #[adze::language]
            pub struct Root {
                #[adze::leaf(pattern = r"\d+")]
                #[adze::repeat(non_empty = true)]
                pub nums: Vec<i32>,
            }
        }
        "#,
    );
    let root = &g["rules"]["Root"];
    let content = &root["content"];
    assert_eq!(
        content["type"].as_str().unwrap(),
        "SYMBOL",
        "non_empty Vec should be direct SYMBOL, got: {}",
        serde_json::to_string_pretty(root).unwrap()
    );
}

#[test]
fn vec_repeat1_content_contains_field_element() {
    let g = extract_one(
        r#"
        #[adze::grammar("vec_elem")]
        mod grammar {
            #[adze::language]
            pub struct Root {
                #[adze::leaf(pattern = r"[a-z]+")]
                pub words: Vec<String>,
            }
        }
        "#,
    );
    let rules = g["rules"].as_object().unwrap();
    let contents = &rules["Root_words_vec_contents"];
    assert_eq!(contents["type"].as_str().unwrap(), "REPEAT1");
    // The content of REPEAT1 is a FIELD wrapping the element
    let inner = &contents["content"];
    assert_eq!(inner["type"].as_str().unwrap(), "FIELD");
    assert_eq!(inner["name"].as_str().unwrap(), "Root_words_vec_element");
}

// ---------------------------------------------------------------------------
// 5. Enum → CHOICE
// ---------------------------------------------------------------------------

#[test]
fn enum_with_two_variants_generates_choice() {
    let g = extract_one(
        r#"
        #[adze::grammar("enum_choice")]
        mod grammar {
            #[adze::language]
            pub enum Token {
                Num(#[adze::leaf(pattern = r"\d+")] i32),
                Id(#[adze::leaf(pattern = r"[a-z]+")] String),
            }
        }
        "#,
    );
    let rule = &g["rules"]["Token"];
    assert_eq!(rule["type"].as_str().unwrap(), "CHOICE");
    assert_eq!(rule["members"].as_array().unwrap().len(), 2);
}

#[test]
fn enum_four_variants_generates_four_choice_members() {
    let g = extract_one(
        r#"
        #[adze::grammar("enum_four")]
        mod grammar {
            #[adze::language]
            pub enum Tok {
                A(#[adze::leaf(text = "a")] String),
                B(#[adze::leaf(text = "b")] String),
                C(#[adze::leaf(text = "c")] String),
                D(#[adze::leaf(text = "d")] String),
            }
        }
        "#,
    );
    let members = g["rules"]["Tok"]["members"].as_array().unwrap();
    assert_eq!(members.len(), 4);
}

#[test]
fn enum_variant_with_named_fields_creates_intermediate_rule() {
    let g = extract_one(
        r#"
        #[adze::grammar("enum_named")]
        mod grammar {
            #[adze::language]
            pub enum Expr {
                #[adze::prec_left(1)]
                Add {
                    left: Box<Expr>,
                    #[adze::leaf(text = "+")]
                    op: String,
                    right: Box<Expr>,
                },
                Lit(#[adze::leaf(pattern = r"\d+")] i32),
            }
        }
        "#,
    );
    let rules = g["rules"].as_object().unwrap();
    // Named-field variant with prec creates an intermediate rule
    assert!(rules.contains_key("Expr_Add"));
}

// ---------------------------------------------------------------------------
// 6. Precedence → PREC / PREC_LEFT / PREC_RIGHT
// ---------------------------------------------------------------------------

#[test]
fn prec_generates_prec_wrapper() {
    let g = extract_one(
        r#"
        #[adze::grammar("prec_plain")]
        mod grammar {
            #[adze::language]
            pub enum Expr {
                #[adze::prec(3)]
                Paren {
                    #[adze::leaf(text = "(")]
                    open: String,
                    inner: Box<Expr>,
                    #[adze::leaf(text = ")")]
                    close: String,
                },
                Num(#[adze::leaf(pattern = r"\d+")] i32),
            }
        }
        "#,
    );
    let paren = &g["rules"]["Expr_Paren"];
    assert_eq!(paren["type"].as_str().unwrap(), "PREC");
    assert_eq!(paren["value"].as_u64().unwrap(), 3);
    assert!(paren.get("content").is_some());
}

#[test]
fn prec_left_generates_prec_left_wrapper() {
    let g = extract_one(
        r#"
        #[adze::grammar("prec_left_test")]
        mod grammar {
            #[adze::language]
            pub enum Expr {
                #[adze::prec_left(1)]
                Add {
                    left: Box<Expr>,
                    #[adze::leaf(text = "+")]
                    op: String,
                    right: Box<Expr>,
                },
                Num(#[adze::leaf(pattern = r"\d+")] i32),
            }
        }
        "#,
    );
    let add = &g["rules"]["Expr_Add"];
    assert_eq!(add["type"].as_str().unwrap(), "PREC_LEFT");
    assert_eq!(add["value"].as_u64().unwrap(), 1);
}

#[test]
fn prec_right_generates_prec_right_wrapper() {
    let g = extract_one(
        r#"
        #[adze::grammar("prec_right_test")]
        mod grammar {
            #[adze::language]
            pub enum Expr {
                #[adze::prec_right(2)]
                Assign {
                    target: Box<Expr>,
                    #[adze::leaf(text = "=")]
                    eq: String,
                    value: Box<Expr>,
                },
                Id(#[adze::leaf(pattern = r"[a-z]+")] String),
            }
        }
        "#,
    );
    let assign = &g["rules"]["Expr_Assign"];
    assert_eq!(assign["type"].as_str().unwrap(), "PREC_RIGHT");
    assert_eq!(assign["value"].as_u64().unwrap(), 2);
}

#[test]
fn prec_content_is_seq_for_multi_field_variant() {
    let g = extract_one(
        r#"
        #[adze::grammar("prec_seq")]
        mod grammar {
            #[adze::language]
            pub enum Expr {
                #[adze::prec_left(1)]
                Mul {
                    left: Box<Expr>,
                    #[adze::leaf(text = "*")]
                    op: String,
                    right: Box<Expr>,
                },
                Num(#[adze::leaf(pattern = r"\d+")] i32),
            }
        }
        "#,
    );
    let mul = &g["rules"]["Expr_Mul"];
    assert_eq!(mul["type"].as_str().unwrap(), "PREC_LEFT");
    let content = &mul["content"];
    assert_eq!(content["type"].as_str().unwrap(), "SEQ");
    let members = content["members"].as_array().unwrap();
    assert_eq!(members.len(), 3);
}

#[test]
fn prec_value_zero_is_valid() {
    let g = extract_one(
        r#"
        #[adze::grammar("prec_zero")]
        mod grammar {
            #[adze::language]
            pub enum Expr {
                #[adze::prec(0)]
                Wrap {
                    #[adze::leaf(text = "[")]
                    open: String,
                    inner: Box<Expr>,
                    #[adze::leaf(text = "]")]
                    close: String,
                },
                Lit(#[adze::leaf(pattern = r"\d+")] i32),
            }
        }
        "#,
    );
    let wrap = &g["rules"]["Expr_Wrap"];
    assert_eq!(wrap["type"].as_str().unwrap(), "PREC");
    assert_eq!(wrap["value"].as_u64().unwrap(), 0);
}

// ---------------------------------------------------------------------------
// 7. Leaf with text → STRING
// ---------------------------------------------------------------------------

#[test]
fn leaf_text_produces_string_rule() {
    let g = extract_one(
        r#"
        #[adze::grammar("string_rule")]
        mod grammar {
            #[adze::language]
            pub struct Root {
                #[adze::leaf(text = "hello")]
                pub kw: String,
            }
        }
        "#,
    );
    let rules = g["rules"].as_object().unwrap();
    let kw = &rules["Root_kw"];
    assert_eq!(kw["type"].as_str().unwrap(), "STRING");
    assert_eq!(kw["value"].as_str().unwrap(), "hello");
}

#[test]
fn leaf_text_special_chars_preserved() {
    let g = extract_one(
        r#"
        #[adze::grammar("string_special")]
        mod grammar {
            #[adze::language]
            pub struct Root {
                #[adze::leaf(text = "=>")]
                pub arrow: String,
            }
        }
        "#,
    );
    let rules = g["rules"].as_object().unwrap();
    assert_eq!(rules["Root_arrow"]["value"].as_str().unwrap(), "=>");
}

// ---------------------------------------------------------------------------
// 8. Leaf with pattern → PATTERN
// ---------------------------------------------------------------------------

#[test]
fn leaf_pattern_produces_pattern_rule() {
    let g = extract_one(
        r#"
        #[adze::grammar("pattern_rule")]
        mod grammar {
            #[adze::language]
            pub struct Root {
                #[adze::leaf(pattern = r"[0-9]+")]
                pub num: String,
            }
        }
        "#,
    );
    let rules = g["rules"].as_object().unwrap();
    let num = &rules["Root_num"];
    assert_eq!(num["type"].as_str().unwrap(), "PATTERN");
    assert_eq!(num["value"].as_str().unwrap(), "[0-9]+");
}

#[test]
fn leaf_pattern_regex_special_chars_preserved() {
    let g = extract_one(
        r#"
        #[adze::grammar("pattern_regex")]
        mod grammar {
            #[adze::language]
            pub struct Root {
                #[adze::leaf(pattern = r"[a-zA-Z_]\w*")]
                pub ident: String,
            }
        }
        "#,
    );
    let rules = g["rules"].as_object().unwrap();
    assert_eq!(
        rules["Root_ident"]["value"].as_str().unwrap(),
        r"[a-zA-Z_]\w*"
    );
}

// ---------------------------------------------------------------------------
// 9. Nested rules
// ---------------------------------------------------------------------------

#[test]
fn nested_struct_referenced_as_symbol() {
    let g = extract_one(
        r#"
        #[adze::grammar("nested_struct")]
        mod grammar {
            #[adze::language]
            pub struct Program {
                pub stmt: Statement,
            }
            pub struct Statement {
                #[adze::leaf(pattern = r"[a-z]+")]
                pub name: String,
                #[adze::leaf(text = ";")]
                pub semi: String,
            }
        }
        "#,
    );
    let rules = g["rules"].as_object().unwrap();
    // Program references Statement as a SYMBOL
    let prog = &rules["Program"];
    assert_eq!(prog["content"]["name"].as_str().unwrap(), "Statement");
    // Statement is its own rule with SEQ
    assert!(rules.contains_key("Statement"));
    assert_eq!(rules["Statement"]["type"].as_str().unwrap(), "SEQ");
}

#[test]
fn nested_enum_inside_struct() {
    let g = extract_one(
        r#"
        #[adze::grammar("nested_enum")]
        mod grammar {
            #[adze::language]
            pub struct Root {
                pub value: Value,
            }
            pub enum Value {
                Num(#[adze::leaf(pattern = r"\d+")] i32),
                Str(#[adze::leaf(pattern = r"'[^']*'")] String),
            }
        }
        "#,
    );
    let rules = g["rules"].as_object().unwrap();
    assert!(rules.contains_key("Value"));
    assert_eq!(rules["Value"]["type"].as_str().unwrap(), "CHOICE");
    // Root field references Value
    let root = &rules["Root"];
    assert_eq!(root["content"]["name"].as_str().unwrap(), "Value");
}

#[test]
fn recursive_type_via_box() {
    let g = extract_one(
        r#"
        #[adze::grammar("recursive")]
        mod grammar {
            #[adze::language]
            pub enum Expr {
                #[adze::prec_left(1)]
                Add {
                    left: Box<Expr>,
                    #[adze::leaf(text = "+")]
                    op: String,
                    right: Box<Expr>,
                },
                Num(#[adze::leaf(pattern = r"\d+")] i32),
            }
        }
        "#,
    );
    let rules = g["rules"].as_object().unwrap();
    // Recursive variant references Expr through Box
    let add = &rules["Expr_Add"];
    let content = &add["content"];
    let members = content["members"].as_array().unwrap();
    // left and right fields reference Expr
    let left = &members[0];
    assert_eq!(left["content"]["name"].as_str().unwrap(), "Expr");
    let right = &members[2];
    assert_eq!(right["content"]["name"].as_str().unwrap(), "Expr");
}

#[test]
fn deeply_nested_struct_chain() {
    let g = extract_one(
        r#"
        #[adze::grammar("deep_nest")]
        mod grammar {
            #[adze::language]
            pub struct Program {
                pub block: Block,
            }
            pub struct Block {
                pub stmt: Statement,
            }
            pub struct Statement {
                #[adze::leaf(pattern = r"[a-z]+")]
                pub name: String,
            }
        }
        "#,
    );
    let rules = g["rules"].as_object().unwrap();
    assert!(rules.contains_key("Program"));
    assert!(rules.contains_key("Block"));
    assert!(rules.contains_key("Statement"));
    assert_eq!(
        rules["Program"]["content"]["name"].as_str().unwrap(),
        "Block"
    );
    assert_eq!(
        rules["Block"]["content"]["name"].as_str().unwrap(),
        "Statement"
    );
}

#[test]
fn enum_variant_inlining_single_leaf_pattern() {
    let g = extract_one(
        r#"
        #[adze::grammar("inline_leaf")]
        mod grammar {
            #[adze::language]
            pub enum Token {
                Word(#[adze::leaf(pattern = r"[a-z]+")] String),
                Num(#[adze::leaf(pattern = r"\d+")] i32),
            }
        }
        "#,
    );
    let rule = &g["rules"]["Token"];
    assert_eq!(rule["type"].as_str().unwrap(), "CHOICE");
    let members = rule["members"].as_array().unwrap();
    // Single-leaf variants with inline should produce PATTERN directly in CHOICE
    for m in members {
        let ty = m["type"].as_str().unwrap();
        assert!(
            ty == "PATTERN" || ty == "SYMBOL",
            "expected PATTERN or SYMBOL in CHOICE, got: {}",
            ty
        );
    }
}

#[test]
fn source_file_rule_always_references_language_type() {
    let g = extract_one(
        r#"
        #[adze::grammar("srcfile")]
        mod grammar {
            #[adze::language]
            pub struct MyLang {
                #[adze::leaf(pattern = r".*")]
                pub content: String,
            }
        }
        "#,
    );
    let sf = &g["rules"]["source_file"];
    assert_eq!(sf["type"].as_str().unwrap(), "SYMBOL");
    assert_eq!(sf["name"].as_str().unwrap(), "MyLang");
}

#[test]
fn skip_field_excluded_from_seq() {
    let g = extract_one(
        r#"
        #[adze::grammar("skip_in_seq")]
        mod grammar {
            #[adze::language]
            pub struct Root {
                #[adze::leaf(pattern = r"[a-z]+")]
                pub kept: String,
                #[adze::skip]
                pub skipped: Option<()>,
                #[adze::leaf(pattern = r"\d+")]
                pub also_kept: String,
            }
        }
        "#,
    );
    let root = &g["rules"]["Root"];
    assert_eq!(root["type"].as_str().unwrap(), "SEQ");
    // Only 2 members (skipped field excluded)
    assert_eq!(root["members"].as_array().unwrap().len(), 2);
}

#[test]
fn test_vec_with_option_elements_preserves_fielded_optional_occurrence() {
    let g = extract_one(
        r#"
        #[adze::grammar("vec_opt_elem")]
        mod grammar {
            #[adze::language]
            pub struct Root {
                #[adze::leaf(pattern = r"[a-z]+")]
                pub items: Vec<Option<String>>,
            }
        }
        "#,
    );
    let rules = g["rules"].as_object().unwrap();
    let contents = &rules["Root_items_vec_contents"];
    // FIELD encloses both alternatives so a BLANK is a real list occurrence
    // with the element's identity, distinct from an empty-repeat sentinel.
    assert_eq!(
        contents,
        &serde_json::json!({
            "type": "REPEAT1",
            "content": {
                "type": "FIELD",
                "name": "Root_items_vec_element",
                "content": {
                    "type": "CHOICE",
                    "members": [
                        { "type": "BLANK" },
                        { "type": "SYMBOL", "name": "Root_items" }
                    ]
                }
            }
        })
    );
}
