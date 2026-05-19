use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[test]
fn readme_arithmetic_quickstart_builds_and_runs() {
    let temp = tempfile::tempdir().expect("tempdir");
    let project_dir = temp.path().join("adze_readme_quickstart");
    fs::create_dir_all(project_dir.join("src")).expect("create src");
    fs::create_dir_all(project_dir.join("tests")).expect("create tests");

    let repo_root = repo_root();
    let runtime_path = toml_path(repo_root.join("runtime"));
    let tool_path = toml_path(repo_root.join("tool"));
    let readme = include_str!("../../README.md");
    let manifest_snippet = fenced_block_after(readme, "## Install", "toml")
        .expect("README install section should include a TOML dependency block");
    let build_rs = fenced_block_after(readme, "Add a `build.rs`", "rust")
        .expect("README install section should include a build.rs block");
    let grammar_snippet = fenced_block_starting_with(readme, "rust", "#[adze::grammar")
        .expect("README should include the arithmetic grammar quickstart block");
    assert!(
        grammar_snippet.contains(r#"let expr = grammar::parse("1 + 2 * 3")?;"#),
        "README grammar block should show the documented parser call"
    );

    fs::write(
        project_dir.join("Cargo.toml"),
        downstream_manifest(manifest_snippet, &runtime_path, &tool_path),
    )
    .expect("write Cargo.toml");

    fs::write(project_dir.join("build.rs"), build_rs).expect("write build.rs");

    fs::write(
        project_dir.join("src/lib.rs"),
        grammar_module_from_readme(grammar_snippet),
    )
    .expect("write lib.rs");

    fs::write(
        project_dir.join("tests/readme_quickstart.rs"),
        r#"use adze_readme_quickstart::grammar::{self, Expr};

#[test]
fn readme_expression_respects_precedence() {
    let expr = grammar::parse("1 + 2 * 3").expect("README expression should parse");

    assert_eq!(
        expr,
        Expr::Add(
            Box::new(Expr::Number(1)),
            (),
            Box::new(Expr::Mul(
                Box::new(Expr::Number(2)),
                (),
                Box::new(Expr::Number(3)),
            )),
        )
    );
}

#[test]
fn readme_bad_input_reports_useful_diagnostic() {
    let source = "1 + @";
    let errors = grammar::parse(source).expect_err("bad README input should fail clearly");
    let first = errors
        .first()
        .expect("bad README input should produce at least one parse error");

    assert_eq!(
        first.byte_span(),
        4..5,
        "diagnostic should point at the invalid token"
    );
    assert!(
        !first.expected.is_empty(),
        "diagnostic should report expected tokens"
    );
    assert!(
        first.expected.iter().any(|name| name == r"/\d+/"),
        "diagnostic should name the expected number token, got {:?}",
        first.expected
    );

    let rendered = first.display_with_source(source).to_string();
    assert!(
        rendered.contains("bytes 4..5"),
        "rendered diagnostic should include the byte span: {rendered}"
    );
    assert!(
        rendered.contains("expected one of:"),
        "rendered diagnostic should include expected-token context: {rendered}"
    );
    assert!(
        rendered.contains("    ^"),
        "rendered diagnostic should place a caret under the invalid token: {rendered}"
    );
}
"#,
    )
    .expect("write quickstart test");

    let output = Command::new("cargo")
        .arg("test")
        .arg("--quiet")
        .current_dir(&project_dir)
        .output()
        .expect("run cargo test in README quickstart crate");

    assert!(
        output.status.success(),
        "README quickstart crate should build and parse into the documented typed AST\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn getting_started_quickstart_builds_parses_and_reports_diagnostics() {
    let temp = tempfile::tempdir().expect("tempdir");
    let project_dir = temp.path().join("adze_getting_started_quickstart");
    fs::create_dir_all(project_dir.join("src")).expect("create src");
    fs::create_dir_all(project_dir.join("tests")).expect("create tests");

    let repo_root = repo_root();
    let runtime_path = toml_path(repo_root.join("runtime"));
    let tool_path = toml_path(repo_root.join("tool"));
    let tutorial = include_str!("../../docs/tutorials/getting-started.md");
    let manifest_snippet = fenced_block_after(tutorial, "### Installation", "toml")
        .expect("Getting Started tutorial should include a TOML dependency block");
    let build_rs = fenced_block_after(tutorial, "Create `build.rs`", "rust")
        .expect("Getting Started tutorial should include a build.rs block");
    let lib_rs = fenced_block_after(tutorial, "Create `src/lib.rs`", "rust")
        .expect("Getting Started tutorial should include a src/lib.rs block");

    fs::write(
        project_dir.join("Cargo.toml"),
        tutorial_downstream_manifest(manifest_snippet, &runtime_path, &tool_path),
    )
    .expect("write Cargo.toml");

    fs::write(project_dir.join("build.rs"), build_rs).expect("write build.rs");
    fs::write(project_dir.join("src/lib.rs"), lib_rs).expect("write lib.rs");
    fs::write(
        project_dir.join("tests/getting_started_quickstart.rs"),
        r#"use adze_getting_started_quickstart::grammar;

#[test]
fn getting_started_expression_parses_into_typed_value() {
    let program = grammar::parse("42").expect("documented tutorial expression should parse");

    assert_eq!(program.number, "42");
}

#[test]
fn getting_started_bad_input_reports_useful_diagnostic() {
    let source = "@";
    let errors = match grammar::parse(source) {
        Ok(_) => panic!("bad tutorial input should fail clearly"),
        Err(errors) => errors,
    };
    let first = errors
        .first()
        .expect("bad tutorial input should produce at least one parse error");

    assert_eq!(
        first.byte_span(),
        0..1,
        "diagnostic should point at the invalid token"
    );
    assert!(
        !first.expected.is_empty(),
        "diagnostic should report expected tokens"
    );
    assert!(
        first.expected.iter().any(|name| name == r"/\d+/"),
        "diagnostic should name the expected number token, got {:?}",
        first.expected
    );

    let rendered = first.display_with_source(source).to_string();
    assert!(
        rendered.contains("bytes 0..1"),
        "rendered diagnostic should include the byte span: {rendered}"
    );
    assert!(
        rendered.contains("expected one of:"),
        "rendered diagnostic should include expected-token context: {rendered}"
    );
    assert!(
        rendered.contains("^"),
        "rendered diagnostic should place a caret under the invalid token: {rendered}"
    );
}
"#,
    )
    .expect("write Getting Started quickstart tests");

    let output = Command::new("cargo")
        .arg("test")
        .arg("--quiet")
        .current_dir(&project_dir)
        .output()
        .expect("run cargo test in Getting Started quickstart crate");

    assert!(
        output.status.success(),
        "Getting Started quickstart crate should build, parse, and report diagnostics through the documented public API\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn book_quickstart_builds_parses_and_reports_diagnostics() {
    let temp = tempfile::tempdir().expect("tempdir");
    let project_dir = temp.path().join("adze_book_quickstart");
    fs::create_dir_all(project_dir.join("src")).expect("create src");
    fs::create_dir_all(project_dir.join("tests")).expect("create tests");

    let repo_root = repo_root();
    let runtime_path = toml_path(repo_root.join("runtime"));
    let tool_path = toml_path(repo_root.join("tool"));
    let quickstart = include_str!("../../book/src/getting-started/quickstart.md");
    let manifest_snippet = fenced_block_after(quickstart, "## Installation", "toml")
        .expect("Book quickstart should include a TOML dependency block");
    let build_rs = fenced_block_after(quickstart, "Create `build.rs`", "rust")
        .expect("Book quickstart should include a build.rs block");
    let lib_rs = fenced_block_after(quickstart, "Create `src/lib.rs`", "rust")
        .expect("Book quickstart should include a src/lib.rs block");

    fs::write(
        project_dir.join("Cargo.toml"),
        book_downstream_manifest(manifest_snippet, &runtime_path, &tool_path),
    )
    .expect("write Cargo.toml");

    fs::write(project_dir.join("build.rs"), build_rs).expect("write build.rs");
    fs::write(project_dir.join("src/lib.rs"), lib_rs).expect("write lib.rs");
    fs::write(
        project_dir.join("tests/book_quickstart.rs"),
        r#"use adze_book_quickstart::grammar::{self, Expr};

#[test]
fn book_expression_respects_precedence() {
    let expr = grammar::parse("1 + 2 * 3").expect("book expression should parse");

    assert_eq!(
        expr,
        Expr::Add(
            Box::new(Expr::Number(1)),
            (),
            Box::new(Expr::Mul(
                Box::new(Expr::Number(2)),
                (),
                Box::new(Expr::Number(3)),
            )),
        )
    );
}

#[test]
fn book_bad_input_reports_useful_diagnostic() {
    let source = "1 + @";
    let errors = grammar::parse(source).expect_err("bad book input should fail clearly");
    let first = errors
        .first()
        .expect("bad book input should produce at least one parse error");

    assert_eq!(
        first.byte_span(),
        4..5,
        "diagnostic should point at the invalid token"
    );
    assert!(
        first.expected.iter().any(|name| name == r"/\d+/"),
        "diagnostic should name the expected number token, got {:?}",
        first.expected
    );

    let rendered = first.display_with_source(source).to_string();
    assert!(
        rendered.contains("bytes 4..5"),
        "rendered diagnostic should include the byte span: {rendered}"
    );
    assert!(
        rendered.contains("expected one of:"),
        "rendered diagnostic should include expected-token context: {rendered}"
    );
    assert!(
        rendered.contains("    ^"),
        "rendered diagnostic should place a caret under the invalid token: {rendered}"
    );
}
"#,
    )
    .expect("write book quickstart tests");

    let output = Command::new("cargo")
        .arg("test")
        .arg("--quiet")
        .current_dir(&project_dir)
        .output()
        .expect("run cargo test in Book quickstart crate");

    assert!(
        output.status.success(),
        "Book quickstart crate should build, parse, and report diagnostics through the documented public API\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn readme_stable_claims_are_in_stable_product_lane() {
    let readme = include_str!("../../README.md");
    let support_tiers = include_str!("../../docs/status/SUPPORT_TIERS.md");
    let stable_lane = include_str!("../../scripts/ci-product-stable.sh");
    let stable_rows = readme_stable_capability_rows(readme);

    assert!(
        !stable_rows.is_empty(),
        "README capability table should include Stable proof commands"
    );

    for row in stable_rows {
        assert!(
            support_tiers_has_stable_surface_row(support_tiers, &row.surface),
            "README Stable surface must map to a Stable row in docs/status/SUPPORT_TIERS.md:\n{}",
            row.surface
        );

        for command in row.proof_commands {
            assert!(
                support_tiers.contains(&command),
                "README Stable proof command must be documented in docs/status/SUPPORT_TIERS.md:\n{command}"
            );

            if is_required_gate(&command) {
                continue;
            }

            assert!(
                stable_lane.contains(&command),
                "README Stable proof command must be present in scripts/ci-product-stable.sh:\n{command}"
            );
        }
    }
}

#[test]
fn cargo_install_adze_cli_claims_stay_release_surface_bounded() {
    let docs = [
        ("README.md", include_str!("../../README.md")),
        ("cli/README.md", include_str!("../README.md")),
        (
            "docs/tutorials/quickstart-10-minutes.md",
            include_str!("../../docs/tutorials/quickstart-10-minutes.md"),
        ),
        (
            "docs/tutorials/getting-started.md",
            include_str!("../../docs/tutorials/getting-started.md"),
        ),
        (
            "book/src/getting-started/quickstart.md",
            include_str!("../../book/src/getting-started/quickstart.md"),
        ),
        (
            "docs/product/ACCEPTANCE_MATRIX.md",
            include_str!("../../docs/product/ACCEPTANCE_MATRIX.md"),
        ),
        (
            "docs/specs/ADZE-SPEC-0012-glr-toolkit-product-contract.md",
            include_str!("../../docs/specs/ADZE-SPEC-0012-glr-toolkit-product-contract.md"),
        ),
        (
            "docs/status/KNOWN_RED.md",
            include_str!("../../docs/status/KNOWN_RED.md"),
        ),
        (
            "docs/status/PRODUCT_OBJECTIVE_AUDIT.md",
            include_str!("../../docs/status/PRODUCT_OBJECTIVE_AUDIT.md"),
        ),
        (
            "docs/status/PRODUCT_PROOF_MAP.md",
            include_str!("../../docs/status/PRODUCT_PROOF_MAP.md"),
        ),
    ];

    for (path, text) in docs {
        for (idx, line) in text.lines().enumerate() {
            if !line.contains("cargo install adze-cli") {
                continue;
            }

            let context = surrounding_context(text, idx, 6).to_ascii_lowercase();
            assert!(
                install_claim_context_is_bounded(&context),
                "`cargo install adze-cli` must stay explicitly bounded as a release-surface claim until a crates.io receipt exists.\nfile: {path}\nline: {}\ncontext:\n{}",
                idx + 1,
                surrounding_context(text, idx, 6)
            );
        }
    }
}

#[derive(Debug)]
struct StableCapabilityRow {
    surface: String,
    proof_commands: Vec<String>,
}

fn support_tiers_has_stable_surface_row(support_tiers: &str, readme_surface: &str) -> bool {
    let normalized_readme_surface = stable_surface_lookup_key(readme_surface);

    support_tiers.lines().any(|line| {
        if !line.starts_with('|') || !line.contains("| **Stable") {
            return false;
        }

        let columns: Vec<&str> = line.split('|').collect();
        let Some(surface) = columns.get(1) else {
            return false;
        };

        stable_surface_lookup_key(surface.trim()) == normalized_readme_surface
    })
}

fn stable_surface_lookup_key(surface: &str) -> String {
    surface
        .split(" (")
        .next()
        .unwrap_or(surface)
        .trim()
        .trim_matches('`')
        .to_ascii_lowercase()
}

fn readme_stable_capability_rows(readme: &str) -> Vec<StableCapabilityRow> {
    let mut rows = Vec::new();
    let mut in_capability_table = false;

    for line in readme.lines() {
        if line == "### Capability table" {
            in_capability_table = true;
            continue;
        }

        if in_capability_table && line.starts_with("##") {
            break;
        }

        if !in_capability_table {
            continue;
        }

        if !line.starts_with('|') || !line.contains("| **Stable** |") {
            continue;
        }

        let columns: Vec<&str> = line.split('|').collect();
        assert!(
            columns.len() >= 4,
            "README Stable capability row should have a surface and proof column: {line}"
        );

        let surface = columns[1].trim().to_string();
        let proof = columns[3];
        let proof_commands = inline_code_spans(proof);
        assert!(
            !proof_commands.is_empty(),
            "README Stable capability row should name at least one proof command: {line}"
        );

        rows.push(StableCapabilityRow {
            surface,
            proof_commands,
        });
    }

    rows
}

fn book_downstream_manifest(readme_toml: &str, runtime_path: &str, tool_path: &str) -> String {
    assert!(
        readme_toml.contains(r#"adze = { version = "0.8.0-dev", default-features = false }"#),
        "Book quickstart install block should document the adze runtime dependency"
    );
    assert!(
        readme_toml.contains(r#"adze-tool = "0.8.0-dev""#),
        "Book quickstart install block should document the adze-tool build dependency"
    );

    let dependencies = readme_toml
        .replace(
            r#"adze = { version = "0.8.0-dev", default-features = false }"#,
            &format!(r#"adze = {{ path = "{runtime_path}", default-features = false }}"#),
        )
        .replace(
            r#"adze-tool = "0.8.0-dev""#,
            &format!(r#"adze-tool = {{ path = "{tool_path}" }}"#),
        );

    format!(
        r#"[package]
name = "adze_book_quickstart"
version = "0.1.0"
edition = "2024"

{dependencies}
"#
    )
}

fn tutorial_downstream_manifest(readme_toml: &str, runtime_path: &str, tool_path: &str) -> String {
    assert!(
        readme_toml.contains(r#"adze = { version = "0.8.0-dev", default-features = false }"#),
        "Getting Started install block should document the adze runtime dependency"
    );
    assert!(
        readme_toml.contains(r#"adze-tool = "0.8.0-dev""#),
        "Getting Started install block should document the adze-tool build dependency"
    );

    let dependencies = readme_toml
        .replace(
            r#"adze = { version = "0.8.0-dev", default-features = false }"#,
            &format!(r#"adze = {{ path = "{runtime_path}", default-features = false }}"#),
        )
        .replace(
            r#"adze-tool = "0.8.0-dev""#,
            &format!(r#"adze-tool = {{ path = "{tool_path}" }}"#),
        );

    format!(
        r#"[package]
name = "adze_getting_started_quickstart"
version = "0.1.0"
edition = "2024"

{dependencies}
"#
    )
}

fn downstream_manifest(readme_toml: &str, runtime_path: &str, tool_path: &str) -> String {
    assert!(
        readme_toml.contains(r#"adze = { version = "0.8", default-features = false }"#),
        "README install block should document the adze runtime dependency"
    );
    assert!(
        readme_toml.contains(r#"adze-tool = "0.8""#),
        "README install block should document the adze-tool build dependency"
    );

    let dependencies = readme_toml
        .replace(
            r#"adze = { version = "0.8", default-features = false }"#,
            &format!(r#"adze = {{ path = "{runtime_path}", default-features = false }}"#),
        )
        .replace(
            r#"adze-tool = "0.8""#,
            &format!(r#"adze-tool = {{ path = "{tool_path}" }}"#),
        );

    format!(
        r#"[package]
name = "adze_readme_quickstart"
version = "0.1.0"
edition = "2024"

{dependencies}
"#
    )
}

fn inline_code_spans(text: &str) -> Vec<String> {
    let mut spans = Vec::new();
    let mut rest = text;

    while let Some(start) = rest.find('`') {
        let after_start = &rest[start + 1..];
        let Some(end) = after_start.find('`') else {
            break;
        };

        spans.push(after_start[..end].to_string());
        rest = &after_start[end + 1..];
    }

    spans
}

fn is_required_gate(command: &str) -> bool {
    matches!(command, "just ci-supported" | "CI / ci-supported")
}

fn install_claim_context_is_bounded(context: &str) -> bool {
    [
        "intended published",
        "only after",
        "until `adze-cli` is published",
        "until adze-cli is published",
        "crates.io install receipt",
        "release-surface",
        "not prove crates.io",
        "not a crates.io install claim",
        "not a stable cli",
    ]
    .iter()
    .any(|marker| context.contains(marker))
}

fn surrounding_context(text: &str, line_idx: usize, radius: usize) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let start = line_idx.saturating_sub(radius);
    let end = (line_idx + radius + 1).min(lines.len());
    lines[start..end].join("\n")
}

fn grammar_module_from_readme(snippet: &str) -> String {
    let parser_call = "\nlet expr = grammar::parse";
    let grammar = snippet
        .split(parser_call)
        .next()
        .expect("README grammar snippet should have a grammar module before the parser call")
        .trim_end();

    format!("{grammar}\n")
}

fn fenced_block_after<'a>(text: &'a str, marker: &str, language: &str) -> Option<&'a str> {
    let start = text.find(marker)?;
    fenced_blocks(&text[start..], language).into_iter().next()
}

fn fenced_block_starting_with<'a>(text: &'a str, language: &str, prefix: &str) -> Option<&'a str> {
    fenced_blocks(text, language)
        .into_iter()
        .find(|block| block.trim_start().starts_with(prefix))
}

fn fenced_blocks<'a>(text: &'a str, language: &str) -> Vec<&'a str> {
    let fence = format!("```{language}");
    let mut blocks = Vec::new();
    let mut rest = text;

    while let Some(idx) = rest.find(&fence) {
        let after_fence = &rest[idx + fence.len()..];
        let Some(line_end) = after_fence.find('\n') else {
            break;
        };
        let body_start = idx + fence.len() + line_end + 1;
        let body = &rest[body_start..];
        let Some(body_end) = body.find("\n```") else {
            break;
        };
        blocks.push(body[..body_end].trim_end_matches('\r'));
        rest = &body[body_end + "\n```".len()..];
    }

    blocks
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("cli crate has workspace parent")
        .to_path_buf()
}

fn toml_path(path: PathBuf) -> String {
    path.display().to_string().replace('\\', "/")
}
