use adze_tool::grammar_js::{GrammarJsConverter, from_json};
use serde_json::json;
fn main() {
    println!("cargo::rustc-check-cfg=cfg(adze_unsafe_attrs)");
    println!("cargo::rustc-cfg=adze_unsafe_attrs");
    for (literal, leaf, text) in [
        (false, json!({"type":"PATTERN","value":"[a-z]+"}), "word"),
        (true, json!({"type":"STRING","value":"q"}), "q"),
    ] {
        for choice in [false, true] {
            let name = format!(
                "imported_{}_{}",
                if literal { "literal" } else { "pattern" },
                if choice { "choice" } else { "sequence" }
            );
            let reference = json!({"type":"SYMBOL","name":"leaf"});
            let entry = if choice {
                json!({"type":"CHOICE","members":[reference,{"type":"STRING","value":"!"}]})
            } else {
                json!({"type":"SEQ","members":[reference]})
            };
            let value = json!({"name":name,"start_symbol":"source_file","rules":{"source_file":entry,"leaf":leaf}});
            let grammar = GrammarJsConverter::new(from_json(&value).unwrap())
                .convert()
                .unwrap();
            let start = grammar.explicit_start_symbol().unwrap();
            let wrapper = grammar.find_symbol_by_name("leaf").unwrap();
            let binding = grammar.wrapper_token_for(wrapper).unwrap();
            let result =
                adze_tool::build_parser(grammar, adze_tool::BuildOptions::default()).unwrap();
            let code = format!(
                "const GRAMMAR_NAME: &str = {name:?}; pub const BINDING: u16 = {}; pub const WRAPPER: u16 = {}; pub const START: u16 = {}; pub const SOURCE: &str = {text:?}; {}",
                binding.0, wrapper.0, start.0, result.parser_code
            );
            std::fs::write(
                std::path::Path::new(&std::env::var("OUT_DIR").unwrap()).join(format!("{name}.rs")),
                code,
            )
            .unwrap();
        }
    }
}
