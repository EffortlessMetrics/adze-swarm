//! Panic-safe grammar analysis helpers for CLI commands.

use adze_tool::pure_rust_builder::{BuildOptions, BuildResult, build_parser_for_crate};
use anyhow::{Context, Result};
use std::panic::AssertUnwindSafe;
use std::{fs, path::Path};

/// Analyze an adze grammar file and return parser-build metadata.
///
/// This operation runs parser generation in a panic boundary, returning an
/// error if analysis panics or if no grammar definitions are found.
pub(crate) fn analyze_grammar_file(
    grammar: &Path,
    compress_tables: bool,
) -> Result<Vec<BuildResult>> {
    if !grammar.exists() {
        anyhow::bail!("Grammar file does not exist: {}", grammar.display());
    }
    if !grammar.is_file() {
        anyhow::bail!("Grammar path is not a file: {}", grammar.display());
    }
    let content = fs::read_to_string(grammar)
        .with_context(|| format!("Could not read grammar file: {}", grammar.display()))?;
    syn::parse_file(&content)
        .with_context(|| format!("Grammar syntax is invalid: {}", grammar.display()))?;

    let temp_dir = tempfile::tempdir()?;
    let options = BuildOptions {
        out_dir: temp_dir.path().to_string_lossy().to_string(),
        emit_artifacts: false,
        compress_tables,
    };

    let grammar_path = grammar.to_owned();
    let build_result = std::panic::catch_unwind(AssertUnwindSafe(move || {
        build_parser_for_crate(&grammar_path, options)
    }))
    .map_err(|_| anyhow::anyhow!("Grammar analysis panicked"))?;

    let results = build_result?;
    if results.is_empty() {
        anyhow::bail!("No adze grammar definitions found in {}", grammar.display());
    }

    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::analyze_grammar_file;
    use std::fs;

    #[test]
    fn returns_error_when_no_grammars_present() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let path = temp_dir.path().join("not_a_grammar.rs");
        fs::write(&path, "fn helper() {}\n").expect("write fixture");

        let err = analyze_grammar_file(&path, false).expect_err("should fail");
        let message = format!("{err:#}");
        assert!(
            message.contains("No adze grammar definitions found")
                || message.contains("Could not find grammar file"),
            "unexpected error: {message}"
        );
    }

    #[test]
    fn returns_error_when_grammar_file_is_missing() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let path = temp_dir.path().join("missing.rs");

        let err = analyze_grammar_file(&path, false).expect_err("should fail");
        let message = format!("{err:#}");
        assert!(
            message.contains("Grammar file does not exist"),
            "unexpected error: {message}"
        );
    }

    #[test]
    fn returns_error_when_grammar_file_has_invalid_rust_syntax() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let path = temp_dir.path().join("broken.rs");
        fs::write(
            &path,
            r#"
#[adze::grammar("broken")]
pub mod grammar {
    #[adze::language]
    pub struct Program {
        #[adze::leaf(pattern = r"\d+", text = true)]
        pub number: String
    }
"#,
        )
        .expect("write broken fixture");

        let err = analyze_grammar_file(&path, false).expect_err("should fail");
        let message = format!("{err:#}");
        assert!(
            message.contains("Grammar syntax is invalid"),
            "unexpected error: {message}"
        );
        assert!(
            !message.contains("Grammar analysis panicked"),
            "invalid syntax should fail before parser generation panic boundary: {message}"
        );
    }
}
