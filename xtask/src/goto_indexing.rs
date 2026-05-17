use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};
use std::process::Command;
use walkdir::WalkDir;

const GOTO_FIELD: &str = "goto_indexing";
const DIRECT_SYMBOL_ID: &str = "GotoIndexing::DirectSymbolId";
const NONTERMINAL_MAP: &str = "GotoIndexing::NonterminalMap";

#[derive(Debug, Clone, PartialEq, Eq)]
struct Violation {
    path: PathBuf,
    line_number: usize,
    line: String,
    kind: ViolationKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ViolationKind {
    DirectSymbolId,
    NonterminalMap,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SymbolZeroWarning {
    path: PathBuf,
    line_number: usize,
    line: String,
}

pub fn run(files: Vec<PathBuf>) -> Result<()> {
    println!("Checking GOTO indexing invariants...");

    let files = if files.is_empty() {
        tracked_rust_files().unwrap_or_else(|_| discover_rust_files())
    } else {
        files
    };

    let mut violations = Vec::new();
    let mut warnings = Vec::new();

    for path in files {
        if !path.exists() || !is_rust_file(&path) {
            continue;
        }

        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        violations.extend(find_assignment_violations(&path, &text));

        if is_runtime_test_path(&path) {
            warnings.extend(find_symbol_zero_warnings(&path, &text));
        }
    }

    if !violations.is_empty() {
        eprintln!("ERROR: found direct GOTO indexing assignment without remapping helper");
        for violation in &violations {
            let helper = match violation.kind {
                ViolationKind::DirectSymbolId => "remap_goto_to_direct_symbol_id",
                ViolationKind::NonterminalMap => "remap_goto_to_nonterminal_map",
            };
            eprintln!(
                "{}:{}: use table.{}() instead of directly setting the field",
                display_path(&violation.path),
                violation.line_number,
                helper
            );
            eprintln!("  {}", violation.line.trim());
        }
        bail!("GOTO indexing check failed");
    }

    if !warnings.is_empty() {
        eprintln!("WARNING: found SymbolId(0) in runtime tests outside known EOF contexts");
        for warning in warnings.iter().take(5) {
            eprintln!(
                "{}:{}: {}",
                display_path(&warning.path),
                warning.line_number,
                warning.line.trim()
            );
        }
    }

    println!("All GOTO indexing checks passed.");
    Ok(())
}

fn tracked_rust_files() -> Result<Vec<PathBuf>> {
    let output = Command::new("git")
        .args(["ls-files", "*.rs"])
        .output()
        .context("failed to run git ls-files")?;

    if !output.status.success() {
        bail!("git ls-files failed");
    }

    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(PathBuf::from)
        .collect())
}

fn discover_rust_files() -> Vec<PathBuf> {
    WalkDir::new(".")
        .into_iter()
        .filter_entry(|entry| {
            let name = entry.file_name().to_string_lossy();
            name != ".git" && name != "target"
        })
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
        .map(|entry| entry.into_path())
        .filter(|path| is_rust_file(path))
        .collect()
}

fn is_rust_file(path: &Path) -> bool {
    path.extension().is_some_and(|ext| ext == "rs")
}

fn find_assignment_violations(path: &Path, text: &str) -> Vec<Violation> {
    if is_allowed_goto_implementation_path(path) {
        return Vec::new();
    }

    text.lines()
        .enumerate()
        .filter_map(|(index, line)| {
            assignment_violation_kind(line).map(|kind| Violation {
                path: path.to_path_buf(),
                line_number: index + 1,
                line: line.to_string(),
                kind,
            })
        })
        .collect()
}

fn assignment_violation_kind(line: &str) -> Option<ViolationKind> {
    if line.contains("remap_goto_to_direct_symbol_id")
        || line.contains("remap_goto_to_nonterminal_map")
    {
        return None;
    }

    let compact: String = line.chars().filter(|ch| !ch.is_whitespace()).collect();
    if compact.contains(&format!("{GOTO_FIELD}={DIRECT_SYMBOL_ID}")) {
        return Some(ViolationKind::DirectSymbolId);
    }
    if compact.contains(&format!("{GOTO_FIELD}={NONTERMINAL_MAP}")) {
        return Some(ViolationKind::NonterminalMap);
    }

    None
}

fn find_symbol_zero_warnings(path: &Path, text: &str) -> Vec<SymbolZeroWarning> {
    text.lines()
        .enumerate()
        .filter(|(_, line)| {
            line.contains("SymbolId(0)")
                && !line.contains("eof_symbol")
                && !line.contains("normalize_eof_to_zero")
                && !line.contains("table.eof_symbol")
        })
        .map(|(index, line)| SymbolZeroWarning {
            path: path.to_path_buf(),
            line_number: index + 1,
            line: line.to_string(),
        })
        .collect()
}

fn is_allowed_goto_implementation_path(path: &Path) -> bool {
    matches!(
        normalized_path(path).as_str(),
        "glr-core/src/lib.rs" | "glr-core/src/parse_table.rs"
    )
}

fn is_runtime_test_path(path: &Path) -> bool {
    let normalized = normalized_path(path);
    normalized.starts_with("runtime/tests/") && normalized.ends_with(".rs")
}

fn display_path(path: &Path) -> String {
    normalized_path(path)
}

fn normalized_path(path: &Path) -> String {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn goto_indexing_flags_direct_symbol_id_assignment() {
        let line = format!(
            "table.goto_{} = GotoIndexing::{};",
            "indexing", "DirectSymbolId"
        );

        assert_eq!(
            assignment_violation_kind(&line),
            Some(ViolationKind::DirectSymbolId)
        );
    }

    #[test]
    fn goto_indexing_flags_nonterminal_map_assignment_with_spacing() {
        let line = format!(
            "table.goto_{}   =   GotoIndexing::{};",
            "indexing", "NonterminalMap"
        );

        assert_eq!(
            assignment_violation_kind(&line),
            Some(ViolationKind::NonterminalMap)
        );
    }

    #[test]
    fn goto_indexing_allows_direct_symbol_id_remap_helper() {
        let line = "table.remap_goto_to_direct_symbol_id();";

        assert_eq!(assignment_violation_kind(line), None);
    }

    #[test]
    fn goto_indexing_allows_nonterminal_map_remap_helper() {
        let line = "table.remap_goto_to_nonterminal_map();";

        assert_eq!(assignment_violation_kind(line), None);
    }

    #[test]
    fn symbol_zero_warning_ignores_known_eof_contexts() {
        let text = "\
let eof_symbol = SymbolId(0);
normalize_eof_to_zero(SymbolId(0));
assert_eq!(table.eof_symbol, SymbolId(0));
let token = SymbolId(0);
";

        let warnings = find_symbol_zero_warnings(Path::new("runtime/tests/example.rs"), text);

        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].line_number, 4);
    }

    #[test]
    fn goto_implementation_path_is_exempt() {
        let line = format!(
            "table.goto_{} = GotoIndexing::{};",
            "indexing", "DirectSymbolId"
        );
        let violations = find_assignment_violations(Path::new("glr-core/src/lib.rs"), &line);

        assert!(violations.is_empty());
    }

    #[test]
    fn split_goto_implementation_path_is_exempt() {
        let line = format!(
            "table.goto_{} = GotoIndexing::{};",
            "indexing", "NonterminalMap"
        );
        let violations =
            find_assignment_violations(Path::new("glr-core/src/parse_table.rs"), &line);

        assert!(violations.is_empty());
    }
}
