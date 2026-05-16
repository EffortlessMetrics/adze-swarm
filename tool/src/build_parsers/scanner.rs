//! External scanner discovery for generated C parsers.

use std::path::{Path, PathBuf};

pub(crate) fn add_first_available_scanner(c_config: &mut cc::Build, generated_dir: &Path) {
    for (path, is_cpp) in scanner_candidates(generated_dir) {
        if path.exists() {
            if is_cpp {
                c_config.cpp(true);
            }
            c_config.file(path);
            break;
        }
    }
}

fn scanner_candidates(generated_dir: &Path) -> Vec<(PathBuf, bool)> {
    let mut paths = generated_scanner_candidates(generated_dir);

    if let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") {
        let src_dir = Path::new(&manifest_dir).join("src");
        let scanner_subdir = src_dir.join("scanner");
        paths.extend([
            (src_dir.join("scanner.c"), false),
            (src_dir.join("scanner.cc"), true),
            (src_dir.join("scanner.cpp"), true),
            (scanner_subdir.join("scanner.c"), false),
            (scanner_subdir.join("scanner.cc"), true),
            (scanner_subdir.join("scanner.cpp"), true),
        ]);
    }

    paths
}

fn generated_scanner_candidates(generated_dir: &Path) -> Vec<(PathBuf, bool)> {
    vec![
        (generated_dir.join("scanner.c"), false),
        (generated_dir.join("scanner.cc"), true),
        (generated_dir.join("scanner.cpp"), true),
    ]
}
