//! Pure-Rust parser builder integration for build scripts.

use std::path::Path;

pub(crate) fn run_pure_rust_builder(root_file: &Path, strict: bool) {
    use crate::pure_rust_builder::{BuildOptions, build_parser_for_crate_with_inputs};
    let options = BuildOptions::default();
    // These are read by BuildParserOptions, BuildOptions, logging, or the
    // legacy backend. Cargo itself owns OUT_DIR, target, package and feature
    // identity; cc emits its own compiler-environment dependencies.
    for variable in [
        "ADZE_USE_PURE_RUST",
        "ADZE_EMIT_ARTIFACTS",
        "ADZE_DEBUG_FILE",
        "RUST_LOG",
        "ADZE_REQUIRE_TS_CLI",
    ] {
        println!("cargo:rerun-if-env-changed={variable}");
    }
    let built = build_parser_for_crate_with_inputs(root_file, options.clone(), |inputs| {
        if let Err(error) = emit_input_watches(inputs, Path::new(&options.out_dir)) {
            // Optional generation may still fall through to C. Never admit an
            // untracked success if a discovered path cannot be represented.
            emit_revalidation_watch(Path::new(&options.out_dir))?;
            return Err(error);
        }
        Ok(())
    });
    match built {
        Ok(results) => {
            for result in results {
                if std::env::var("RUST_LOG")
                    .ok()
                    .unwrap_or_default()
                    .contains("debug")
                {
                    println!("Built pure-Rust parser for {}", result.grammar_name);
                }
            }
        }
        Err(e) if strict => {
            eprintln!("Failed to build pure-Rust parser: {}", e);
            let mut source = e.source();
            while let Some(err) = source {
                eprintln!("  Caused by: {}", err);
                source = err.source();
            }
            panic!("FATAL: Pure-Rust parser generation failed: {:#}", e);
        }
        Err(e) => {
            println!("cargo:warning=Failed to build optional pure-Rust parser modules: {e}");
        }
    }
}

/// Source-directory watches observe the existing resolver's candidate topology
/// without implementing a second Rust module resolver. Output-overlapping source
/// layouts retain conservative regeneration instead of a false freshness claim.
fn emit_input_watches(inputs: &crate::GrammarInputs, out_dir: &Path) -> anyhow::Result<()> {
    let output = std::fs::canonicalize(out_dir)?;
    let mut overlap = inputs.incomplete;
    for directory in &inputs.directories {
        let directory = std::fs::canonicalize(directory)?;
        if output.starts_with(&directory) {
            overlap = true;
        } else {
            emit_file_watch(&directory)?;
        }
    }
    for file in &inputs.files {
        emit_file_watch(file)?;
    }
    if overlap {
        emit_revalidation_watch(&output)?;
        println!(
            "cargo:warning=adze source tracking is incomplete or overlaps output; retaining conservative regeneration for this layout"
        );
    }
    Ok(())
}

fn emit_revalidation_watch(output: &Path) -> anyhow::Result<()> {
    // Do not recursively watch Cargo's target tree. An owned removed path
    // forces fresh validation when tracking cannot support a no-change claim.
    let trigger = tempfile::Builder::new()
        .prefix("adze-revalidate-")
        .tempfile_in(output)?;
    let trigger_path = trigger.path().to_path_buf();
    drop(trigger);
    emit_file_watch(&trigger_path)
}

fn emit_file_watch(path: &Path) -> anyhow::Result<()> {
    let path = path.to_str().ok_or_else(|| {
        anyhow::anyhow!(
            "cannot represent grammar input path in Cargo's UTF-8 build-script protocol: {path:?}"
        )
    })?;
    anyhow::ensure!(
        !path.contains(['\n', '\r']),
        "grammar input path contains a line break and cannot be tracked by Cargo: {path:?}"
    );
    println!("cargo:rerun-if-changed={path}");
    Ok(())
}
