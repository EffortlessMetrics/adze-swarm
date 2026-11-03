fn main() {
    println!("cargo::rustc-check-cfg=cfg(adze_unsafe_attrs)");
    println!("cargo::rerun-if-changed=src/lib.rs");
    let source = std::path::PathBuf::from("src/lib.rs");
    adze_tool::build_parsers(&source);
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());

    // These two sibling controls deliberately select a non-first alternative.
    // Preserve the source-extracted grammar and make lexical priority explicit,
    // rather than assuming insertion/declaration order resolves equal regexes.
    for value in adze_tool::generate_grammars(&source).expect("extract actual annotated grammars") {
        let name = value["name"].as_str().expect("grammar name");
        if !matches!(
            name,
            "shared_pattern_identity" | "same_regex_wrapped_siblings"
        ) {
            continue;
        }
        let grammar_js = adze_tool::grammar_js::from_json(&value)
            .expect("convert source metadata to grammar.js IR");
        let mut grammar = adze_tool::GrammarJsConverter::new(grammar_js)
            .convert()
            .expect("convert source grammar preserving wrapper identities");
        let selected = if name == "shared_pattern_identity" {
            grammar
                .tokens
                .iter()
                .find_map(|(id, token)| (token.name == r"/\d+/").then_some(*id))
        } else {
            grammar
                .find_symbol_by_name("Choice_ASecond")
                .and_then(|wrapper| grammar.wrapper_token_for(wrapper))
        };
        // Missing inline identity is retained as a runtime assertion failure on
        // the old producer, not hidden by a fixture setup failure.
        if let Some(token) = selected {
            grammar.boost_token_lexical_priority(token, 10);
        }
        let ordinary = out.join(format!("grammar_{name}/parser_{name}.rs"));
        let ordinary_bytes = std::fs::read(&ordinary).expect("ordinary public-route parser");
        adze_tool::pure_rust_builder::build_parser(
            grammar,
            adze_tool::pure_rust_builder::BuildOptions {
                out_dir: out.join("controlled").to_str().unwrap().to_owned(),
                ..adze_tool::pure_rust_builder::BuildOptions::default()
            },
        )
        .expect("generate priority-controlled native sibling parser");
        assert_eq!(
            std::fs::read(&ordinary).unwrap(),
            ordinary_bytes,
            "controlled priority build must not overwrite ordinary parser output"
        );
    }
}
