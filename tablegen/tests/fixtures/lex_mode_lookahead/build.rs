include!("grammar.rs");
fn main() {
    println!("cargo::rustc-check-cfg=cfg(adze_unsafe_attrs)");
    println!("cargo::rustc-cfg=adze_unsafe_attrs");
    let result = adze_tool::pure_rust_builder::build_parser(
        grammar(),
        adze_tool::pure_rust_builder::BuildOptions::default(),
    )
    .expect("generate exact overlapping-lookahead native parser");
    std::fs::write(
        std::path::Path::new(&std::env::var("OUT_DIR").unwrap()).join("generated.rs"),
        result.parser_code,
    )
    .unwrap();
}
