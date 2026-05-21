//! Minimal sysroot emission for wasm C parser builds.

use std::{io::Write, path::Path};

pub(crate) fn write_wasm_sysroot_if_needed(dir: &Path) -> std::path::PathBuf {
    let sysroot_dir = dir.join("sysroot");
    let target = std::env::var("TARGET").unwrap_or_else(|_| current_target_fallback());

    if should_emit_wasm_sysroot(&target) {
        if let Err(error) = std::fs::create_dir_all(&sysroot_dir) {
            eprintln!(
                "warning: unable to create wasm sysroot directory {}: {error}",
                sysroot_dir.display()
            );
            return sysroot_dir;
        }

        let files = [
            (
                "stdint.h",
                include_bytes!("../wasm-sysroot/stdint.h").as_slice(),
            ),
            (
                "stdlib.h",
                include_bytes!("../wasm-sysroot/stdlib.h").as_slice(),
            ),
            (
                "stdio.h",
                include_bytes!("../wasm-sysroot/stdio.h").as_slice(),
            ),
            (
                "stdbool.h",
                include_bytes!("../wasm-sysroot/stdbool.h").as_slice(),
            ),
        ];

        for (name, contents) in files {
            if let Err(error) = write_sysroot_file(&sysroot_dir, name, contents) {
                eprintln!(
                    "warning: unable to write wasm sysroot file {}: {error}",
                    sysroot_dir.join(name).display()
                );
            }
        }
    }

    sysroot_dir
}

fn current_target_fallback() -> String {
    format!("{}-{}", std::env::consts::ARCH, std::env::consts::OS)
}

fn should_emit_wasm_sysroot(target: &str) -> bool {
    target.starts_with("wasm32")
}

fn write_sysroot_file(sysroot_dir: &Path, name: &str, contents: &[u8]) -> std::io::Result<()> {
    let mut file = std::fs::File::create(sysroot_dir.join(name))?;
    file.write_all(contents)
}

#[cfg(test)]
mod tests {
    use super::{current_target_fallback, should_emit_wasm_sysroot};

    #[test]
    fn should_emit_wasm_sysroot_accepts_wasm32_targets() {
        assert!(should_emit_wasm_sysroot("wasm32-unknown-unknown"));
        assert!(should_emit_wasm_sysroot("wasm32-wasip1"));
    }

    #[test]
    fn should_emit_wasm_sysroot_rejects_non_wasm_targets() {
        assert!(!should_emit_wasm_sysroot("x86_64-unknown-linux-gnu"));
        assert!(!should_emit_wasm_sysroot("aarch64-apple-darwin"));
    }

    #[test]
    fn current_target_fallback_matches_arch_os_contract() {
        assert_eq!(
            current_target_fallback(),
            format!("{}-{}", std::env::consts::ARCH, std::env::consts::OS)
        );
    }
}
