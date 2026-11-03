use std::{env, fs, path::Path};

fn main() {
    // The externally supplied counter lives outside all fixture source paths.
    // No source or generated-output watch is added by this instrumentation.
    let counter = env::var_os("ADZE_TEST_BUILD_COUNT_FILE").expect("external counter path");
    let previous = fs::read_to_string(&counter)
        .map(|count| count.parse::<u32>().expect("numeric counter"))
        .unwrap_or(0);
    fs::write(counter, (previous + 1).to_string()).expect("write invocation count");
    println!("cargo::rustc-check-cfg=cfg(adze_unsafe_attrs)");
    adze_tool::build_parsers(Path::new("src/lib.rs"));
}
