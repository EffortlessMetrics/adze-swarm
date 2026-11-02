fn main() {
    println!("cargo::rustc-check-cfg=cfg(adze_unsafe_attrs)");
    println!("cargo::rerun-if-changed=grammar.rs");
    adze_tool::build_parsers(&std::path::PathBuf::from("grammar.rs"));
}
