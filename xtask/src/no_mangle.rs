use anyhow::{Context, Result, bail};
use std::{
    fs,
    path::{Component, Path, PathBuf},
    process::Command,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Violation {
    pub path: PathBuf,
    pub line: usize,
    pub content: String,
}

pub fn run(files: Vec<PathBuf>) -> Result<()> {
    println!("Checking for bare #[no_mangle] attributes...");

    let files = if files.is_empty() {
        discover_rust_files()?
    } else {
        files
            .into_iter()
            .filter(|path| path.extension().is_some_and(|ext| ext == "rs"))
            .collect()
    };

    let mut violations = Vec::new();
    for path in &files {
        if !path.exists() {
            continue;
        }
        violations.extend(find_violations(path)?);
    }

    if violations.is_empty() {
        println!("No bare #[no_mangle] attributes found.");
        return Ok(());
    }

    for violation in &violations {
        println!(
            "{}:{}:{}",
            violation.path.display(),
            violation.line,
            violation.content
        );
    }
    bail!(
        "found {} bare #[no_mangle] attribute(s); use #[unsafe(no_mangle)] or the documented cfg compatibility pattern",
        violations.len()
    );
}

fn discover_rust_files() -> Result<Vec<PathBuf>> {
    let output = Command::new("git")
        .args(["ls-files", "*.rs"])
        .output()
        .context("failed to run git ls-files")?;

    if output.status.success() {
        return Ok(String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(PathBuf::from)
            .collect());
    }

    let mut files = Vec::new();
    collect_rust_files(Path::new("."), &mut files)?;
    Ok(files)
}

fn collect_rust_files(dir: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    if should_skip_dir(dir) {
        return Ok(());
    }

    for entry in fs::read_dir(dir).with_context(|| format!("read {}", dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_rust_files(&path, files)?;
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            files.push(path);
        }
    }
    Ok(())
}

fn should_skip_dir(path: &Path) -> bool {
    path.components().any(|component| {
        matches!(
            component,
            Component::Normal(name) if name == "target" || name == ".git"
        )
    })
}

pub fn find_violations(path: &Path) -> Result<Vec<Violation>> {
    let text = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    Ok(find_violations_in_text(path, &text))
}

pub fn find_violations_in_text(path: &Path, text: &str) -> Vec<Violation> {
    let lines: Vec<&str> = text.lines().collect();
    lines
        .iter()
        .enumerate()
        .filter_map(|(idx, line)| {
            if !is_bare_no_mangle(line) || has_adze_unsafe_attrs_fallback_cfg(&lines, idx) {
                return None;
            }
            Some(Violation {
                path: path.to_path_buf(),
                line: idx + 1,
                content: (*line).to_owned(),
            })
        })
        .collect()
}

fn is_bare_no_mangle(line: &str) -> bool {
    line.trim_start()
        .strip_prefix('#')
        .is_some_and(|after_hash| after_hash.trim_start().starts_with("[no_mangle]"))
}

fn has_adze_unsafe_attrs_fallback_cfg(lines: &[&str], idx: usize) -> bool {
    idx > 0 && lines[idx - 1].trim() == "#[cfg(not(adze_unsafe_attrs))]"
}

#[cfg(test)]
mod tests {
    use super::*;

    const BARE_NO_MANGLE: &str = concat!("#[", "no_mangle]");

    fn violations(text: &str) -> Vec<Violation> {
        find_violations_in_text(Path::new("src/lib.rs"), text)
    }

    #[test]
    fn no_mangle_flags_bare_attribute() {
        let source = format!(
            r#"
{BARE_NO_MANGLE}
pub extern "C" fn exported() {{}}
"#
        );
        let found = violations(&source);

        assert_eq!(found.len(), 1);
        assert_eq!(found[0].line, 2);
    }

    #[test]
    fn no_mangle_allows_adze_unsafe_attrs_fallback_pattern() {
        let source = format!(
            r#"
#[cfg(adze_unsafe_attrs)]
#[unsafe(no_mangle)]
#[cfg(not(adze_unsafe_attrs))]
{BARE_NO_MANGLE}
pub extern "C" fn exported() {{}}
"#
        );
        let found = violations(&source);

        assert!(found.is_empty());
    }

    #[test]
    fn no_mangle_allows_unsafe_no_mangle() {
        let found = violations(
            r#"
#[unsafe(no_mangle)]
pub extern "C" fn exported() {}
"#,
        );

        assert!(found.is_empty());
    }

    #[test]
    fn no_mangle_requires_fallback_cfg_to_be_immediately_before_attribute() {
        let source = format!(
            r#"
#[cfg(not(adze_unsafe_attrs))]
#[inline]
{BARE_NO_MANGLE}
pub extern "C" fn exported() {{}}
"#
        );
        let found = violations(&source);

        assert_eq!(found.len(), 1);
        assert_eq!(found[0].line, 4);
    }

    #[test]
    fn no_mangle_matches_hash_bracket_spacing() {
        let source = format!(
            r#"
  # {}
pub extern "C" fn exported() {{}}
"#,
            "[no_mangle]"
        );
        let found = violations(&source);

        assert_eq!(found.len(), 1);
        assert_eq!(found[0].line, 2);
    }
}
