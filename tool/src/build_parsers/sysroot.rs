//! Minimal sysroot emission for wasm C parser builds.

use std::{io::Write, path::Path};

pub(crate) fn write_wasm_sysroot_if_needed(dir: &Path) -> std::path::PathBuf {
    let sysroot_dir = dir.join("sysroot");
    let target = std::env::var("TARGET").unwrap_or_else(|_| {
        // Fallback to the current target if TARGET is not set.
        std::env::consts::ARCH.to_string() + "-" + std::env::consts::OS
    });

    if target.starts_with("wasm32") {
        std::fs::create_dir(&sysroot_dir).unwrap();
        write_sysroot_file(
            &sysroot_dir,
            "stdint.h",
            include_bytes!("../wasm-sysroot/stdint.h"),
        );
        write_sysroot_file(
            &sysroot_dir,
            "stdlib.h",
            include_bytes!("../wasm-sysroot/stdlib.h"),
        );
        write_sysroot_file(
            &sysroot_dir,
            "stdio.h",
            include_bytes!("../wasm-sysroot/stdio.h"),
        );
        write_sysroot_file(
            &sysroot_dir,
            "stdbool.h",
            include_bytes!("../wasm-sysroot/stdbool.h"),
        );
    }

    sysroot_dir
}

fn write_sysroot_file(sysroot_dir: &Path, name: &str, contents: &[u8]) {
    let mut file = std::fs::File::create(sysroot_dir.join(name)).unwrap();
    file.write_all(contents).unwrap();
}
