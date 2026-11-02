//! CLI argument parsing tests for adze.
//!
//! These tests validate that clap argument parsing works correctly
//! without running the actual commands (no end-to-end execution).

use assert_cmd::cargo::cargo_bin_cmd;
use predicates::prelude::*;
use std::process::Command;

// ---------------------------------------------------------------------------
// End-to-end smoke tests.
// ---------------------------------------------------------------------------

#[test]
fn test_cli_help() {
    let mut cmd = cargo_bin_cmd!("adze");
    cmd.arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Adze CLI - Tools for grammar development",
        ));
}

#[test]
fn test_cli_help_subcommand() {
    let mut cmd = cargo_bin_cmd!("adze");
    cmd.arg("help")
        .assert()
        .success()
        .stdout(predicate::str::contains("check"))
        .stdout(predicate::str::contains("stats"))
        .stdout(predicate::str::contains("version"));
}

#[test]
fn test_cli_version_flag() {
    let mut cmd = cargo_bin_cmd!("adze");
    cmd.arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::starts_with("adze"));
}

#[test]
fn test_cli_version_subcommand() {
    let mut cmd = cargo_bin_cmd!("adze");
    cmd.arg("version")
        .assert()
        .success()
        .stdout(predicate::str::starts_with("adze"));
}

#[test]
fn test_cli_no_args_shows_help() {
    let mut cmd = cargo_bin_cmd!("adze");
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Usage"));
}

#[test]
fn test_cli_unknown_command() {
    let mut cmd = cargo_bin_cmd!("adze");
    cmd.arg("nonexistent")
        .assert()
        .failure()
        .stderr(predicate::str::contains("unrecognized subcommand"));
}

#[test]
fn test_init_generates_buildable_project() {
    let temp = tempfile::tempdir().expect("tempdir");
    let project_name = "freshlang";
    let mut init = cargo_bin_cmd!("adze");
    init.arg("init")
        .arg(project_name)
        .arg("--output")
        .arg(temp.path())
        .assert()
        .success();

    let project_dir = temp.path().join(project_name);
    assert!(project_dir.join("Cargo.toml").exists());
    assert!(project_dir.join("src/grammar.rs").exists());
    assert!(project_dir.join("tests/parse.rs").exists());
    assert!(project_dir.join("examples/parse.rs").exists());

    let status = Command::new("cargo")
        .arg("test")
        .current_dir(&project_dir)
        .status()
        .expect("run cargo test for generated project");
    assert!(
        status.success(),
        "generated project should build and pass typed parser tests"
    );

    let output = Command::new("cargo")
        .arg("run")
        .arg("--quiet")
        .arg("--example")
        .arg("parse")
        .arg("--")
        .arg("1 + 2 * 3")
        .current_dir(&project_dir)
        .output()
        .expect("run generated parse example");
    assert!(
        output.status.success(),
        "generated parse example should run\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("Add"),
        "generated parse example should print the typed AST"
    );

    let mut check = cargo_bin_cmd!("adze");
    check
        .arg("check")
        .arg(project_dir.join("src/grammar.rs"))
        .assert()
        .success();
}

fn write_tiny_project(dir: &std::path::Path, package: &str, test_name: &str, passes: bool) {
    std::fs::create_dir_all(dir.join("src")).expect("create tiny project src");
    std::fs::write(
        dir.join("Cargo.toml"),
        format!("[package]\nname = \"{package}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[workspace]\n"),
    )
    .expect("write tiny project manifest");
    let assertion = if passes {
        "assert_eq!(1 + 1, 2);"
    } else {
        "assert_eq!(1 + 1, 3, \"target failure marker\");"
    };
    std::fs::write(
        dir.join("src/lib.rs"),
        format!(
            "#[cfg(test)]\nmod tests {{\n    #[test]\n    fn {test_name}() {{ {assertion} }}\n}}\n"
        ),
    )
    .expect("write tiny project lib");
}

#[test]
fn test_cli_test_failing_target_from_passing_caller_reports_failure() {
    let temp = tempfile::tempdir().expect("tempdir");
    let caller = temp.path().join("passing caller");
    let target = temp.path().join("failing-target");
    write_tiny_project(&caller, "passing_caller", "passing_caller_test", true);
    write_tiny_project(&target, "failing_target", "failing_target_test", false);

    let mut cmd = cargo_bin_cmd!("adze");
    cmd.current_dir(&caller)
        .arg("test")
        .arg(&target)
        .assert()
        .failure()
        .stdout(predicate::str::contains("failing_target_test"));
}

#[test]
fn test_cli_test_passing_target_from_failing_caller_reports_success() {
    let temp = tempfile::tempdir().expect("tempdir");
    let caller = temp.path().join("failing-caller");
    let target = temp.path().join("passing target");
    write_tiny_project(&caller, "failing_caller", "failing_caller_test", false);
    write_tiny_project(&target, "passing_target", "passing_target_test", true);

    let mut cmd = cargo_bin_cmd!("adze");
    cmd.current_dir(&caller)
        .arg("test")
        .arg(&target)
        .assert()
        .success()
        .stdout(predicate::str::contains("All tests passed!"));
}

fn write_context_project(
    dir: &std::path::Path,
    package: &str,
    marker: &str,
    expected_marker: &str,
) {
    std::fs::create_dir_all(dir.join("src")).expect("create context project src");
    std::fs::create_dir_all(dir.join(".cargo")).expect("create context project cargo config dir");
    std::fs::write(
        dir.join("Cargo.toml"),
        format!(
            "[package]\nname = \"{package}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[workspace]\n"
        ),
    )
    .expect("write context project manifest");
    std::fs::write(
        dir.join(".cargo/config.toml"),
        format!("[env]\nADZE_AUDIT_CARGO_CONTEXT = {{ value = \"{marker}\", force = true }}\n"),
    )
    .expect("write context project cargo config");
    std::fs::write(
        dir.join("src/lib.rs"),
        format!(
            "#[test]\nfn selected_context() {{\n    assert_eq!(std::env::var(\"ADZE_AUDIT_CARGO_CONTEXT\").unwrap(), \"{expected_marker}\");\n}}\n"
        ),
    )
    .expect("write context project lib");
}

fn write_snapshot_project(dir: &std::path::Path, package: &str) {
    std::fs::create_dir_all(dir.join("src")).expect("create snapshot project src");
    std::fs::write(
        dir.join("Cargo.toml"),
        format!(
            "[package]\nname = \"{package}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[workspace]\n\n[dev-dependencies]\ninsta = \"=1.48.0\"\n"
        ),
    )
    .expect("write snapshot project manifest");
    std::fs::write(
        dir.join("src/lib.rs"),
        "#[test]\nfn greeting_snapshot() {\n    insta::assert_snapshot!(\"hello adze\");\n}\n",
    )
    .expect("write snapshot project lib");
}

/// Accepted `*.snap` files under a fixture project, excluding build output.
fn accepted_snapshots(project: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut snaps = Vec::new();
    let mut stack = vec![project.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = std::fs::read_dir(&dir).expect("read fixture project dir");
        for entry in entries {
            let path = entry.expect("read fixture dir entry").path();
            if path.is_dir() {
                if path.file_name().is_some_and(|name| name == "target") {
                    continue;
                }
                stack.push(path);
            } else if path.extension().is_some_and(|ext| ext == "snap") {
                snaps.push(path);
            }
        }
    }
    snaps.sort();
    snaps
}

#[test]
fn test_cli_test_uses_target_cargo_config_not_caller() {
    let temp = tempfile::tempdir().expect("tempdir");
    let caller = temp.path().join("caller");
    let target = temp.path().join("target");
    write_context_project(&caller, "audit_caller", "caller", "caller");
    write_context_project(&target, "audit_target", "target", "target");

    // Control: a direct cargo invocation from each project directory passes
    // against that directory's own configuration.
    for dir in [&caller, &target] {
        let status = Command::new("cargo")
            .arg("test")
            .arg("--quiet")
            .current_dir(dir)
            .status()
            .expect("run direct cargo test control");
        assert!(
            status.success(),
            "direct cargo test in {} should pass against its own config",
            dir.display()
        );
    }

    // The CLI must run the target's test under the target's configuration
    // even when invoked from the caller's directory.
    let mut cmd = cargo_bin_cmd!("adze");
    cmd.current_dir(&caller)
        .arg("test")
        .arg(&target)
        .assert()
        .success()
        .stdout(predicate::str::contains("selected_context"))
        .stdout(predicate::str::contains("All tests passed!"));
}

#[test]
fn test_cli_test_relative_and_manifest_forms_keep_target_context() {
    let temp = tempfile::tempdir().expect("tempdir");
    let caller = temp.path().join("caller");
    let target = temp.path().join("target");
    write_context_project(&caller, "rel_caller", "caller", "caller");
    write_context_project(&target, "rel_target", "target", "target");

    // Relative inputs must resolve against the caller's working directory
    // before the child adopts the target directory.
    for input in ["../target", "../target/Cargo.toml"] {
        let mut cmd = cargo_bin_cmd!("adze");
        cmd.current_dir(&caller)
            .arg("test")
            .arg(input)
            .assert()
            .success()
            .stdout(predicate::str::contains("All tests passed!"));
    }

    for input in [target.clone(), target.join("Cargo.toml")] {
        let mut cmd = cargo_bin_cmd!("adze");
        cmd.current_dir(&caller)
            .arg("test")
            .arg(&input)
            .assert()
            .success()
            .stdout(predicate::str::contains("All tests passed!"));
    }

    // Default `.` still tests the project in the current directory.
    let mut cmd = cargo_bin_cmd!("adze");
    cmd.current_dir(&target)
        .arg("test")
        .assert()
        .success()
        .stdout(predicate::str::contains("All tests passed!"));
}

#[test]
fn test_cli_test_space_and_unicode_paths_keep_target_context() {
    let temp = tempfile::tempdir().expect("tempdir");
    let caller = temp.path().join("caller with space");
    let target = temp.path().join("targét ünïcode");
    write_context_project(&caller, "space_caller", "caller", "caller");
    write_context_project(&target, "unicode_target", "target", "target");

    let mut cmd = cargo_bin_cmd!("adze");
    cmd.current_dir(&caller)
        .arg("test")
        .arg(&target)
        .assert()
        .success()
        .stdout(predicate::str::contains("All tests passed!"));

    let mut relative = cargo_bin_cmd!("adze");
    relative
        .current_dir(&caller)
        .arg("test")
        .arg("../targét ünïcode")
        .assert()
        .success()
        .stdout(predicate::str::contains("All tests passed!"));
}

#[test]
fn test_cli_test_nested_member_matches_direct_invocation() {
    let temp = tempfile::tempdir().expect("tempdir");
    let elsewhere = temp.path().join("elsewhere-caller");
    let root = temp.path().join("ws-root");
    let member = root.join("member-target");
    let sibling = root.join("member-sibling");
    for dir in [&elsewhere, &member, &sibling] {
        std::fs::create_dir_all(dir.join("src")).expect("create workspace fixture src");
    }
    std::fs::create_dir_all(elsewhere.join(".cargo")).expect("create caller config dir");
    std::fs::create_dir_all(root.join(".cargo")).expect("create workspace config dir");
    std::fs::write(
        root.join("Cargo.toml"),
        "[workspace]\nmembers = [\"member-target\", \"member-sibling\"]\nresolver = \"2\"\n",
    )
    .expect("write workspace root manifest");
    std::fs::write(
        root.join(".cargo/config.toml"),
        "[env]\nADZE_AUDIT_CARGO_CONTEXT = { value = \"ws-root\", force = true }\n",
    )
    .expect("write workspace cargo config");
    std::fs::write(
        elsewhere.join("Cargo.toml"),
        "[package]\nname = \"ws_caller\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[workspace]\n",
    )
    .expect("write caller manifest");
    std::fs::write(
        elsewhere.join(".cargo/config.toml"),
        "[env]\nADZE_AUDIT_CARGO_CONTEXT = { value = \"caller\", force = true }\n",
    )
    .expect("write caller cargo config");
    std::fs::write(
        elsewhere.join("src/lib.rs"),
        "#[cfg(test)]\nmod tests {\n    #[test]\n    fn ws_caller_test() { assert_eq!(1 + 1, 2); }\n}\n",
    )
    .expect("write caller lib");
    for (dir, package, test_name, body) in [
        (
            &member,
            "member_target",
            "member_target_test",
            "assert_eq!(std::env::var(\"ADZE_AUDIT_CARGO_CONTEXT\").unwrap(), \"ws-root\");",
        ),
        (
            &sibling,
            "member_sibling",
            "member_sibling_test",
            "assert_eq!(1 + 1, 2);",
        ),
    ] {
        std::fs::write(
            dir.join("Cargo.toml"),
            format!("[package]\nname = \"{package}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n"),
        )
        .expect("write member manifest");
        std::fs::write(
            dir.join("src/lib.rs"),
            format!("#[test]\nfn {test_name}() {{ {body} }}\n"),
        )
        .expect("write member lib");
    }

    // Baseline: the direct cargo invocation from the member directory.
    let direct = Command::new("cargo")
        .arg("test")
        .current_dir(&member)
        .output()
        .expect("run direct cargo test in member dir");
    assert!(
        direct.status.success(),
        "direct cargo test in member dir should pass\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&direct.stdout),
        String::from_utf8_lossy(&direct.stderr)
    );
    let direct_combined = format!(
        "{}{}",
        String::from_utf8_lossy(&direct.stdout),
        String::from_utf8_lossy(&direct.stderr)
    );
    assert!(direct_combined.contains("member_target_test"));
    let sibling_in_direct = direct_combined.contains("member_sibling_test");

    // The CLI must agree with that baseline instead of inventing its own
    // workspace scope or inheriting the caller's configuration.
    let mut cmd = cargo_bin_cmd!("adze");
    let output = cmd
        .current_dir(&elsewhere)
        .arg("test")
        .arg(&member)
        .assert()
        .success()
        .get_output()
        .clone();
    let cli_combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        cli_combined.contains("member_target_test"),
        "CLI should run the selected member's test: {cli_combined}"
    );
    assert_eq!(
        cli_combined.contains("member_sibling_test"),
        sibling_in_direct,
        "CLI workspace scope must match the direct cargo invocation from the member dir"
    );
}

#[test]
fn test_cli_test_update_isolated_to_target_snapshots() {
    let temp = tempfile::tempdir().expect("tempdir");
    let caller = temp.path().join("caller");
    let target = temp.path().join("target");
    write_tiny_project(&caller, "snap_caller", "snap_caller_test", true);
    write_snapshot_project(&target, "snap_target");

    // Caller files that must remain byte-identical no matter what the target
    // snapshot update does.
    let caller_sentinel = caller.join("caller-sentinel.txt");
    let caller_snapshot = caller.join("snapshots").join("caller__snap.snap");
    std::fs::create_dir_all(caller_snapshot.parent().expect("snapshot parent"))
        .expect("create caller snapshot dir");
    std::fs::write(&caller_sentinel, "caller sentinel bytes\n").expect("write caller sentinel");
    std::fs::write(&caller_snapshot, "caller snapshot bytes\n").expect("write caller snapshot");

    // Shared target cache keeps the repeated fixture builds cheap; the
    // offline flag keeps the tiny insta fixture on the local registry cache.
    let cache = temp.path().join("target-cache");
    let cli = |update: bool| {
        let mut cmd = cargo_bin_cmd!("adze");
        cmd.current_dir(&caller)
            .env("CARGO_NET_OFFLINE", "true")
            .env("CARGO_TARGET_DIR", &cache)
            .arg("test")
            .arg(&target);
        if update {
            cmd.arg("--update");
        }
        cmd
    };

    // A fresh snapshot fixture fails the normal check without accepting anything.
    cli(false)
        .assert()
        .failure()
        .stdout(predicate::str::contains("greeting_snapshot"));
    assert!(
        accepted_snapshots(&target).is_empty(),
        "normal check must not accept target snapshots"
    );

    // An INSTA_UPDATE value inherited from the parent environment must not
    // turn a normal check into a snapshot update.
    cli(false)
        .env("INSTA_UPDATE", "always")
        .assert()
        .failure()
        .stdout(predicate::str::contains("greeting_snapshot"));
    assert!(
        accepted_snapshots(&target).is_empty(),
        "normal check must not inherit INSTA_UPDATE=always from an earlier call"
    );

    // --update accepts the target snapshot and passes.
    cli(true)
        .assert()
        .success()
        .stdout(predicate::str::contains("All tests passed!"));
    let snaps = accepted_snapshots(&target);
    assert_eq!(
        snaps.len(),
        1,
        "snapshot update should accept exactly the target snapshot: {snaps:?}"
    );
    assert!(
        std::fs::read_to_string(&snaps[0])
            .expect("read accepted snapshot")
            .contains("hello adze"),
        "accepted snapshot should hold the fixture value"
    );

    // Only the intended target snapshot changed.
    assert_eq!(
        std::fs::read_to_string(&caller_sentinel).expect("read caller sentinel"),
        "caller sentinel bytes\n",
        "snapshot update must leave caller files byte-identical"
    );
    assert_eq!(
        std::fs::read_to_string(&caller_snapshot).expect("read caller snapshot"),
        "caller snapshot bytes\n",
        "snapshot update must leave caller snapshots byte-identical"
    );

    // The accepted snapshot is stable under a later normal check.
    cli(false)
        .assert()
        .success()
        .stdout(predicate::str::contains("All tests passed!"));
}

#[test]
fn test_cli_test_missing_or_nonproject_path_fails_before_cargo() {
    let temp = tempfile::tempdir().expect("tempdir");

    let mut missing = cargo_bin_cmd!("adze");
    missing
        .current_dir(temp.path())
        .arg("test")
        .arg(temp.path().join("does-not-exist"))
        .assert()
        .failure()
        .stderr(predicate::str::contains("does not exist"));

    let not_a_project = temp.path().join("not-a-project");
    std::fs::create_dir_all(&not_a_project).expect("create non-project dir");
    let mut cmd = cargo_bin_cmd!("adze");
    cmd.current_dir(temp.path())
        .arg("test")
        .arg(&not_a_project)
        .assert()
        .failure()
        .stderr(predicate::str::contains("does not contain a Cargo.toml"));
}

#[test]
fn test_init_existing_project_refusal_preserves_user_files() {
    let temp = tempfile::tempdir().expect("tempdir");
    let mut init = cargo_bin_cmd!("adze");
    init.arg("init")
        .arg("calc")
        .arg("--output")
        .arg(temp.path())
        .assert()
        .success();

    let project_dir = temp.path().join("calc");
    let manifest = project_dir.join("Cargo.toml");
    let grammar = project_dir.join("src/grammar.rs");
    let lib = project_dir.join("src/lib.rs");
    let build = project_dir.join("build.rs");
    let tests = project_dir.join("tests/parse.rs");
    let example = project_dir.join("examples/parse.rs");
    let readme = project_dir.join("README.md");
    for (path, marker) in [
        (&manifest, "sentinel manifest"),
        (&grammar, "sentinel grammar"),
        (&lib, "sentinel lib"),
        (&build, "sentinel build"),
        (&tests, "sentinel tests"),
        (&example, "sentinel example"),
        (&readme, "sentinel readme"),
    ] {
        std::fs::write(path, marker).expect("write file sentinel");
    }

    let mut rerun = cargo_bin_cmd!("adze");
    rerun
        .arg("init")
        .arg("calc")
        .arg("--output")
        .arg(temp.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("not empty"));

    for (path, marker) in [
        (&manifest, "sentinel manifest"),
        (&grammar, "sentinel grammar"),
        (&lib, "sentinel lib"),
        (&build, "sentinel build"),
        (&tests, "sentinel tests"),
        (&example, "sentinel example"),
        (&readme, "sentinel readme"),
    ] {
        assert_eq!(
            std::fs::read_to_string(path).expect("read preserved file"),
            marker,
            "refused rerun must leave {} byte-identical",
            path.display()
        );
    }
}

#[test]
fn test_init_path_like_or_invalid_names_fail_without_effects() {
    let temp = tempfile::tempdir().expect("tempdir");
    // Init through a child output directory so a path escape would land on
    // a sibling of that child (temp/escaped), not inside it.
    let output = temp.path().join("output");
    std::fs::create_dir_all(&output).expect("create output child dir");
    for name in [
        "../escaped",
        "a/b",
        "a\\b",
        "..",
        ".",
        "9bad",
        "type",
        "bad name",
        "",
        "/abs-path-probe",
        "C:\\abs-path-probe",
    ] {
        let mut cmd = cargo_bin_cmd!("adze");
        cmd.arg("init")
            .arg(name)
            .arg("--output")
            .arg(&output)
            .assert()
            .failure();
    }

    assert!(
        !temp.path().join("escaped").exists(),
        "escaped init must not create a project as a sibling of the output root"
    );
    assert!(
        !output.join("escaped").exists(),
        "escaped init must not create a project inside the output root either"
    );
    let entries = std::fs::read_dir(&output)
        .expect("read output root")
        .count();
    assert_eq!(entries, 0, "rejected names must leave no files behind");
}

#[cfg(unix)]
#[test]
fn test_init_symlinked_target_is_refused_without_effects() {
    let temp = tempfile::tempdir().expect("tempdir");
    let secret = temp.path().join("outside secret");
    std::fs::create_dir_all(&secret).expect("create secret dir");
    std::fs::write(secret.join("keep.txt"), "secret content").expect("write secret sentinel");
    std::os::unix::fs::symlink(&secret, temp.path().join("calc")).expect("link target to secret");

    let mut cmd = cargo_bin_cmd!("adze");
    cmd.arg("init")
        .arg("calc")
        .arg("--output")
        .arg(temp.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("not an empty directory"));

    assert_eq!(
        std::fs::read_to_string(secret.join("keep.txt")).expect("read secret sentinel"),
        "secret content",
        "refused init must not follow a symlinked target"
    );
    assert!(
        std::fs::symlink_metadata(temp.path().join("calc"))
            .expect("read target metadata")
            .file_type()
            .is_symlink(),
        "refused init must leave the symlink itself in place"
    );
}

#[test]
fn test_init_unusable_output_root_fails_before_filesystem_effects() {
    let temp = tempfile::tempdir().expect("tempdir");
    let blocker = temp.path().join("blocker.txt");
    std::fs::write(&blocker, "blocker").expect("write output blocker");

    let mut cmd = cargo_bin_cmd!("adze");
    cmd.arg("init")
        .arg("calc")
        .arg("--output")
        .arg(&blocker)
        .assert()
        .failure();

    assert_eq!(
        std::fs::read_to_string(&blocker).expect("read output blocker"),
        "blocker",
        "failed init must not disturb the blocking file"
    );
    assert!(
        !temp.path().join("calc").exists(),
        "failed init must not create a project directory"
    );
}

#[test]
fn test_init_leaves_no_staging_directories_behind() {
    let temp = tempfile::tempdir().expect("tempdir");
    for name in ["calc", "other"] {
        let mut cmd = cargo_bin_cmd!("adze");
        cmd.arg("init")
            .arg(name)
            .arg("--output")
            .arg(temp.path())
            .assert()
            .success();
    }
    // A refused rerun must not leave staging behind either.
    let mut rerun = cargo_bin_cmd!("adze");
    rerun
        .arg("init")
        .arg("calc")
        .arg("--output")
        .arg(temp.path())
        .assert()
        .failure();

    let leftovers: Vec<_> = std::fs::read_dir(temp.path())
        .expect("read output root")
        .filter_map(|entry| entry.ok().map(|entry| entry.file_name()))
        .filter(|name| name.to_string_lossy().starts_with(".adze-init-"))
        .collect();
    assert!(
        leftovers.is_empty(),
        "init must never leave staging directories behind: {leftovers:?}"
    );
}

#[test]
fn test_init_existing_empty_directory_is_filled() {
    let temp = tempfile::tempdir().expect("tempdir");
    let project_dir = temp.path().join("calc");
    std::fs::create_dir_all(&project_dir).expect("create empty project dir");

    let mut cmd = cargo_bin_cmd!("adze");
    cmd.arg("init")
        .arg("calc")
        .arg("--output")
        .arg(temp.path())
        .assert()
        .success();

    assert!(project_dir.join("Cargo.toml").exists());
    assert!(project_dir.join("src/grammar.rs").exists());
}

#[test]
fn test_init_hyphenated_name_generates_underscore_crate_names() {
    let temp = tempfile::tempdir().expect("tempdir");
    let mut cmd = cargo_bin_cmd!("adze");
    cmd.arg("init")
        .arg("my-lang")
        .arg("--output")
        .arg(temp.path())
        .assert()
        .success();

    let project_dir = temp.path().join("my-lang");
    let manifest = std::fs::read_to_string(project_dir.join("Cargo.toml"))
        .expect("read hyphenated project manifest");
    assert!(manifest.contains("name = \"my-lang\""));
    let grammar = std::fs::read_to_string(project_dir.join("src/grammar.rs"))
        .expect("read hyphenated project grammar");
    assert!(grammar.contains("grammar(\"my_lang\")"));
}

#[test]
fn test_init_project_supports_cli_stats_and_check_end_to_end() {
    let temp = tempfile::tempdir().expect("tempdir");
    let project_name = "statslang";

    let mut init = cargo_bin_cmd!("adze");
    init.arg("init")
        .arg(project_name)
        .arg("--output")
        .arg(temp.path())
        .assert()
        .success();

    let grammar = temp.path().join(project_name).join("src/grammar.rs");

    let mut check = cargo_bin_cmd!("adze");
    check
        .arg("check")
        .arg(&grammar)
        .assert()
        .success()
        .stdout(predicate::str::contains("Grammar syntax is valid"));

    let mut stats = cargo_bin_cmd!("adze");
    stats
        .arg("stats")
        .arg(&grammar)
        .assert()
        .success()
        .stdout(predicate::str::contains("Grammar statistics"))
        .stdout(predicate::str::contains("States:"))
        .stdout(predicate::str::contains("Conflicts:"));
}

#[test]
fn test_check_rejects_file_without_adze_grammar() {
    let temp = tempfile::tempdir().expect("tempdir");
    let not_a_grammar = temp.path().join("not_a_grammar.rs");
    std::fs::write(
        &not_a_grammar,
        "pub struct PlainRust;\nimpl PlainRust { pub fn value(&self) -> usize { 1 } }\n",
    )
    .expect("write non-grammar rust file");

    let mut cmd = cargo_bin_cmd!("adze");
    cmd.arg("check")
        .arg(&not_a_grammar)
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "No adze grammar definitions found",
        ))
        .stdout(predicate::str::contains("Grammar syntax is valid").not());
}

#[test]
fn test_check_reports_missing_grammar_path() {
    let temp = tempfile::tempdir().expect("tempdir");
    let missing_grammar = temp.path().join("missing.rs");

    let mut cmd = cargo_bin_cmd!("adze");
    cmd.arg("check")
        .arg(&missing_grammar)
        .assert()
        .failure()
        .stderr(predicate::str::contains("Grammar file does not exist"))
        .stderr(predicate::str::contains("Grammar analysis panicked").not())
        .stdout(predicate::str::contains("Grammar syntax is valid").not());
}

#[test]
fn test_check_reports_invalid_grammar_syntax() {
    let temp = tempfile::tempdir().expect("tempdir");
    let broken_grammar = temp.path().join("broken.rs");
    std::fs::write(
        &broken_grammar,
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
    .expect("write broken grammar fixture");

    let mut cmd = cargo_bin_cmd!("adze");
    cmd.arg("check")
        .arg(&broken_grammar)
        .assert()
        .failure()
        .stderr(predicate::str::contains("Grammar syntax is invalid"))
        .stderr(predicate::str::contains("Grammar analysis panicked").not())
        .stdout(predicate::str::contains("Grammar syntax is valid").not());
}

#[test]
fn test_stats_rejects_file_without_adze_grammar() {
    let temp = tempfile::tempdir().expect("tempdir");
    let not_a_grammar = temp.path().join("not_a_grammar.rs");
    std::fs::write(
        &not_a_grammar,
        "pub struct PlainRust;\nimpl PlainRust { pub fn value(&self) -> usize { 1 } }\n",
    )
    .expect("write non-grammar rust file");

    let mut cmd = cargo_bin_cmd!("adze");
    cmd.arg("stats")
        .arg(&not_a_grammar)
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "No adze grammar definitions found",
        ))
        .stdout(predicate::str::contains("Grammar Statistics").not());
}

#[test]
fn test_parse_static_tree_mode_emits_document_backed_tree() {
    let temp = tempfile::tempdir().expect("tempdir");
    let project_name = "parsetree";
    let mut init = cargo_bin_cmd!("adze");
    init.arg("init")
        .arg(project_name)
        .arg("--output")
        .arg(temp.path())
        .assert()
        .success();

    let project_dir = temp.path().join(project_name);
    let grammar = project_dir.join("src/grammar.rs");
    let input = temp.path().join("input.txt");
    std::fs::write(&input, "123").expect("write input");

    let mut cmd = cargo_bin_cmd!("adze");
    cmd.arg("parse")
        .arg(&grammar)
        .arg(&input)
        .assert()
        .success()
        .stdout(predicate::str::contains("source_file"))
        .stdout(predicate::str::contains("Expr"))
        .stdout(predicate::str::contains(r"/\d+/"))
        .stdout(predicate::str::contains("[0..3]"))
        .stdout(predicate::str::contains("Static parse mode").not());
}

#[test]
fn test_parse_init_starter_grammar_addition_succeeds() {
    let temp = tempfile::tempdir().expect("tempdir");
    let project_name = "parsestarter";
    let mut init = cargo_bin_cmd!("adze");
    init.arg("init")
        .arg(project_name)
        .arg("--output")
        .arg(temp.path())
        .assert()
        .success();

    let project_dir = temp.path().join(project_name);
    let grammar = project_dir.join("src/grammar.rs");
    let input = temp.path().join("input.txt");
    std::fs::write(&input, "1 + 2").expect("write input");

    let mut cmd = cargo_bin_cmd!("adze");
    cmd.arg("parse")
        .arg(&grammar)
        .arg(&input)
        .assert()
        .success()
        .stdout(predicate::str::contains("source_file"))
        .stdout(predicate::str::contains("Expr_Add"))
        .stdout(predicate::str::contains("[0..5]"));
}

#[test]
fn test_parse_runner_failure_surfaces_root_cause_without_warning_noise() {
    let temp = tempfile::tempdir().expect("tempdir");
    let project_name = "parsebroken";
    let mut init = cargo_bin_cmd!("adze");
    init.arg("init")
        .arg(project_name)
        .arg("--output")
        .arg(temp.path())
        .assert()
        .success();

    let project_dir = temp.path().join(project_name);
    let grammar = project_dir.join("src/grammar.rs");
    let mut grammar_source = std::fs::read_to_string(&grammar).expect("read starter grammar");
    grammar_source.push_str("\nfn broken() -> UndefinedType1099 { todo!() }\n");
    std::fs::write(&grammar, grammar_source).expect("write broken grammar");
    let input = temp.path().join("input.txt");
    std::fs::write(&input, "1 + 2").expect("write input");

    let mut cmd = cargo_bin_cmd!("adze");
    cmd.arg("parse")
        .arg(&grammar)
        .arg(&input)
        .assert()
        .failure()
        .stderr(predicate::str::contains("UndefinedType1099"))
        .stderr(predicate::str::contains("warning").not())
        .stderr(predicate::str::contains("static document projection failed").not());
}

#[test]
fn test_parse_static_json_mode_emits_document_json() {
    let temp = tempfile::tempdir().expect("tempdir");
    let project_name = "parsejsonstatic";
    let mut init = cargo_bin_cmd!("adze");
    init.arg("init")
        .arg(project_name)
        .arg("--output")
        .arg(temp.path())
        .assert()
        .success();

    let project_dir = temp.path().join(project_name);
    let grammar = project_dir.join("src/grammar.rs");
    let input = temp.path().join("input.txt");
    std::fs::write(&input, "123").expect("write input");

    let mut cmd = cargo_bin_cmd!("adze");
    let output = cmd
        .arg("parse")
        .arg(&grammar)
        .arg(&input)
        .arg("--output")
        .arg("json")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value =
        serde_json::from_slice(&output).expect("static json output should be document JSON");
    assert_eq!(json["schema"].as_str(), Some("adze.document.v1"));
    assert_eq!(json["language"]["name"].as_str(), Some("parsejsonstatic"));
    assert_eq!(json["source"]["byte_len"].as_u64(), Some(3));
    assert_eq!(json["tree"]["root"]["kind"].as_str(), Some("source_file"));
    assert!(json["diagnostics"].is_array());
}

#[test]
fn test_parse_static_dot_mode_emits_document_backed_graph() {
    let temp = tempfile::tempdir().expect("tempdir");
    let project_name = "parsedot";
    let mut init = cargo_bin_cmd!("adze");
    init.arg("init")
        .arg(project_name)
        .arg("--output")
        .arg(temp.path())
        .assert()
        .success();

    let project_dir = temp.path().join(project_name);
    let grammar = project_dir.join("src/grammar.rs");
    let input = temp.path().join("input.txt");
    std::fs::write(&input, "123").expect("write input");

    let mut cmd = cargo_bin_cmd!("adze");
    cmd.arg("parse")
        .arg(&grammar)
        .arg(&input)
        .arg("--output")
        .arg("dot")
        .assert()
        .success()
        .stdout(predicate::str::starts_with("digraph adze_tree"))
        .stdout(predicate::str::contains("source_file"))
        .stdout(predicate::str::contains("Expr"))
        .stdout(predicate::str::contains(r"/\\d+/"))
        .stdout(predicate::str::contains("->"))
        .stdout(predicate::str::contains("Static parse mode").not());
}

#[test]
fn test_parse_static_sexp_mode_emits_document_backed_sexp() {
    let temp = tempfile::tempdir().expect("tempdir");
    let project_name = "parsesexp";
    let mut init = cargo_bin_cmd!("adze");
    init.arg("init")
        .arg(project_name)
        .arg("--output")
        .arg(temp.path())
        .assert()
        .success();

    let project_dir = temp.path().join(project_name);
    let grammar = project_dir.join("src/grammar.rs");
    let input = temp.path().join("input.txt");
    std::fs::write(&input, "123").expect("write input");

    let mut cmd = cargo_bin_cmd!("adze");
    cmd.arg("parse")
        .arg(&grammar)
        .arg(&input)
        .arg("--output")
        .arg("sexp")
        .assert()
        .success()
        .stdout(predicate::str::starts_with("(source_file"))
        .stdout(predicate::str::contains("Expr"))
        .stdout(predicate::str::contains(r"/\d+/"))
        .stdout(predicate::str::contains("[0..3]").not())
        .stdout(predicate::str::contains("Static parse mode").not());
}

#[test]
fn test_parse_document_projection_modes_emit_schema_envelopes() {
    let temp = tempfile::tempdir().expect("tempdir");
    let project_name = "parsejson";
    let mut init = cargo_bin_cmd!("adze");
    init.arg("init")
        .arg(project_name)
        .arg("--output")
        .arg(temp.path())
        .assert()
        .success();

    let project_dir = temp.path().join(project_name);
    let grammar = project_dir.join("src/grammar.rs");
    let input = temp.path().join("input.txt");
    std::fs::write(&input, "123").expect("write input");

    for (mode, schema, required_field) in [
        ("document-json", "adze.document.v1", "tree"),
        ("tree-json", "adze.tree.v1", "tree"),
        ("diagnostics-json", "adze.diagnostics.v1", "diagnostics"),
        ("ambiguity-json", "adze.ambiguity.v1", "ambiguities"),
    ] {
        let mut cmd = cargo_bin_cmd!("adze");
        let output = cmd
            .arg("parse")
            .arg(&grammar)
            .arg(&input)
            .arg("--output")
            .arg(mode)
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();

        let json: serde_json::Value = serde_json::from_slice(&output)
            .unwrap_or_else(|err| panic!("{mode} output should be JSON: {err}"));
        assert_eq!(json["schema"].as_str(), Some(schema), "{mode} schema");
        assert_eq!(json["language"]["name"].as_str(), Some("parsejson"));
        assert!(
            !json[required_field].is_null(),
            "{mode} should include `{required_field}` projection facts"
        );

        if mode == "document-json" {
            assert_eq!(json["source"]["byte_len"].as_u64(), Some(3));
            assert!(json["diagnostics"].is_array());
        } else {
            assert_eq!(
                json["document_schema"].as_str(),
                Some("adze.document.v1"),
                "{mode} should declare its source document schema"
            );
        }
    }
}

#[test]
fn parse_document_json_modes_emit_recovery_diagnostics() {
    let temp = tempfile::tempdir().expect("tempdir");
    let project_name = "parsejsonrecovery";
    let mut init = cargo_bin_cmd!("adze");
    init.arg("init")
        .arg(project_name)
        .arg("--output")
        .arg(temp.path())
        .assert()
        .success();

    let project_dir = temp.path().join(project_name);
    let grammar = project_dir.join("src/grammar.rs");
    let input = temp.path().join("bad-input.txt");
    std::fs::write(&input, "1 + @").expect("write bad input");

    let document = parse_projection(&grammar, &input, "document-json");
    assert_eq!(document["schema"].as_str(), Some("adze.document.v1"));
    assert!(
        document["metadata"]["error_count"].as_u64().unwrap_or(0) > 0,
        "document-json should preserve parser recovery metadata: {document:?}"
    );
    assert_eq!(
        document["tree"]["root"]["flags"]["has_error"].as_bool(),
        Some(true),
        "document-json should preserve root error facts: {document:?}"
    );
    assert_bad_input_diagnostic(&document["diagnostics"][0]);

    let tree = parse_projection(&grammar, &input, "tree-json");
    assert_eq!(tree["schema"].as_str(), Some("adze.tree.v1"));
    assert_eq!(tree["document_schema"].as_str(), Some("adze.document.v1"));
    assert_eq!(
        tree["tree"]["root"]["flags"]["has_error"].as_bool(),
        Some(true),
        "tree-json should project selected-tree error facts from the document: {tree:?}"
    );

    let diagnostics = parse_projection(&grammar, &input, "diagnostics-json");
    assert_eq!(diagnostics["schema"].as_str(), Some("adze.diagnostics.v1"));
    assert_eq!(
        diagnostics["document_schema"].as_str(),
        Some("adze.document.v1")
    );
    assert_bad_input_diagnostic(&diagnostics["diagnostics"][0]);
}

fn parse_projection(
    grammar: &std::path::Path,
    input: &std::path::Path,
    mode: &str,
) -> serde_json::Value {
    let mut cmd = cargo_bin_cmd!("adze");
    let output = cmd
        .arg("parse")
        .arg(grammar)
        .arg(input)
        .arg("--output")
        .arg(mode)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    serde_json::from_slice(&output)
        .unwrap_or_else(|err| panic!("{mode} output should be JSON: {err}"))
}

fn assert_bad_input_diagnostic(diagnostic: &serde_json::Value) {
    assert_eq!(diagnostic["start_byte"].as_u64(), Some(4));
    assert_eq!(diagnostic["end_byte"].as_u64(), Some(5));
    assert_eq!(diagnostic["point_range"]["start"]["row"].as_u64(), Some(0));
    assert_eq!(
        diagnostic["point_range"]["start"]["column"].as_u64(),
        Some(4)
    );
    assert_eq!(diagnostic["found"].as_str(), Some("@"));
    assert!(
        diagnostic["expected"]
            .as_array()
            .is_some_and(|expected| expected
                .iter()
                .any(|token| token.as_str() == Some("number"))),
        "diagnostics projection should preserve expected-token names: {diagnostic:?}"
    );
    assert!(
        diagnostic["message"]
            .as_str()
            .is_some_and(|message| message.contains("expected one of:")),
        "diagnostics projection should preserve useful parser context: {diagnostic:?}"
    );
    assert!(
        diagnostic["message"]
            .as_str()
            .is_some_and(|message| message.starts_with("@;")),
        "diagnostics projection should start with the offending byte: {diagnostic:?}"
    );
    assert!(
        diagnostic["message"]
            .as_str()
            .is_some_and(|message| !message.starts_with("end;")),
        "diagnostics projection should name the offending byte instead of EOF: {diagnostic:?}"
    );
}

#[test]
fn test_init_generated_cargo_toml_is_valid() {
    let temp = tempfile::tempdir().expect("tempdir");
    let project_name = "validcargotoml";
    let mut init = cargo_bin_cmd!("adze");
    init.arg("init")
        .arg(project_name)
        .arg("--output")
        .arg(temp.path())
        .assert()
        .success();

    let project_dir = temp.path().join(project_name);
    let cargo_toml_path = project_dir.join("Cargo.toml");
    assert!(cargo_toml_path.exists(), "Cargo.toml should exist");

    let cargo_toml = std::fs::read_to_string(&cargo_toml_path).expect("read Cargo.toml");

    // Must declare the runtime and build-time dependencies
    assert!(
        cargo_toml.contains("adze"),
        "Cargo.toml must reference the adze runtime crate"
    );
    assert!(
        cargo_toml.contains("adze-tool"),
        "Cargo.toml must reference adze-tool as a build dependency"
    );
    assert!(
        cargo_toml.contains("[build-dependencies]"),
        "Cargo.toml must have a [build-dependencies] section"
    );
    assert!(
        cargo_toml.contains("edition = \"2024\""),
        "Cargo.toml must specify edition 2024"
    );
    assert!(
        cargo_toml.contains(&format!("name = \"{}\"", project_name)),
        "Cargo.toml package name must match project name"
    );
    assert!(
        cargo_toml.contains("[features]"),
        "Cargo.toml must define features"
    );
    assert!(
        cargo_toml.contains("pure-rust"),
        "Cargo.toml must include pure-rust feature"
    );
    assert!(
        cargo_toml.contains("[workspace]"),
        "Cargo.toml must make generated starters standalone workspace roots"
    );
}

#[test]
fn test_init_generated_grammar_uses_adze_macros() {
    let temp = tempfile::tempdir().expect("tempdir");
    let project_name = "macrocheck";
    let mut init = cargo_bin_cmd!("adze");
    init.arg("init")
        .arg(project_name)
        .arg("--output")
        .arg(temp.path())
        .assert()
        .success();

    let project_dir = temp.path().join(project_name);
    let grammar_path = project_dir.join("src/grammar.rs");
    assert!(grammar_path.exists(), "grammar.rs should exist");

    let grammar = std::fs::read_to_string(&grammar_path).expect("read grammar.rs");

    assert!(
        grammar.contains("#[adze::grammar("),
        "grammar.rs must use #[adze::grammar] macro"
    );
    assert!(
        grammar.contains("#[adze::language]"),
        "grammar.rs must declare a language entry point with #[adze::language]"
    );
    assert!(
        grammar.contains("#[adze::leaf("),
        "grammar.rs must use #[adze::leaf] for tokens"
    );
    assert!(
        grammar.contains("#[adze::extra]"),
        "grammar.rs must define whitespace handling with #[adze::extra]"
    );
}

#[test]
fn test_init_generated_project_passes_check() {
    let temp = tempfile::tempdir().expect("tempdir");
    let project_name = "checklang";
    let mut init = cargo_bin_cmd!("adze");
    init.arg("init")
        .arg(project_name)
        .arg("--output")
        .arg(temp.path())
        .assert()
        .success();

    let project_dir = temp.path().join(project_name);

    let status = Command::new("cargo")
        .arg("check")
        .current_dir(&project_dir)
        .status()
        .expect("run cargo check for generated project");
    assert!(
        status.success(),
        "generated project should pass cargo check"
    );
}

#[test]
fn test_init_generated_project_under_parent_workspace_passes() {
    let temp = tempfile::tempdir().expect("tempdir");
    let parent_dir = temp.path().join("parent-workspace");
    let output_dir = parent_dir.join("scratch");
    std::fs::create_dir_all(&output_dir).expect("create parent scratch dir");
    std::fs::write(
        parent_dir.join("Cargo.toml"),
        "[workspace]\nmembers = []\nresolver = \"3\"\n",
    )
    .expect("write parent workspace manifest");

    let project_name = "nestedlang";
    let mut init = cargo_bin_cmd!("adze");
    init.arg("init")
        .arg(project_name)
        .arg("--output")
        .arg(&output_dir)
        .assert()
        .success();

    let project_dir = output_dir.join(project_name);
    let cargo_toml =
        std::fs::read_to_string(project_dir.join("Cargo.toml")).expect("read generated Cargo.toml");
    assert!(
        cargo_toml.contains("[workspace]"),
        "generated nested starter should opt out of the parent workspace"
    );

    let output = Command::new("cargo")
        .arg("test")
        .arg("--quiet")
        .current_dir(&project_dir)
        .output()
        .expect("run cargo test for nested generated project");
    assert!(
        output.status.success(),
        "nested generated project should build outside the parent workspace\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn test_init_default_cwd_generates_buildable_project() {
    let temp = tempfile::tempdir().expect("tempdir");
    let project_name = "defaultcwdlang";
    let mut init = cargo_bin_cmd!("adze");
    init.current_dir(temp.path())
        .arg("init")
        .arg(project_name)
        .assert()
        .success();

    let project_dir = temp.path().join(project_name);
    assert!(project_dir.join("Cargo.toml").exists());
    assert!(project_dir.join("src/grammar.rs").exists());
    assert!(project_dir.join("tests/parse.rs").exists());
    assert!(project_dir.join("examples/parse.rs").exists());

    let status = Command::new("cargo")
        .arg("check")
        .current_dir(&project_dir)
        .status()
        .expect("run cargo check for generated project");
    assert!(
        status.success(),
        "default-cwd generated project should pass cargo check"
    );
}

#[test]
fn test_parse_reports_available_modes() {
    let mut cmd = cargo_bin_cmd!("adze");
    cmd.arg("parse")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("--dynamic"))
        .stdout(predicate::str::contains("--format"))
        .stdout(predicate::str::contains("tree"))
        .stdout(predicate::str::contains("json"))
        .stdout(predicate::str::contains("sexp"))
        .stdout(predicate::str::contains("dot"))
        .stdout(predicate::str::contains("document-json"))
        .stdout(predicate::str::contains("tree-json"))
        .stdout(predicate::str::contains("diagnostics-json"))
        .stdout(predicate::str::contains("ambiguity-json"))
        .stdout(predicate::str::contains("--output"));
}

#[test]
fn test_init_cargo_toml_references_adze_dependency() {
    let temp = tempfile::tempdir().expect("tempdir");
    let project_name = "deptest";
    let mut init = cargo_bin_cmd!("adze");
    init.arg("init")
        .arg(project_name)
        .arg("--output")
        .arg(temp.path())
        .assert()
        .success();

    let cargo_toml = std::fs::read_to_string(temp.path().join(project_name).join("Cargo.toml"))
        .expect("read Cargo.toml");

    // In dev builds deps use path=; in releases they use version=. Either is fine.
    let has_path_or_version = cargo_toml.contains("path =") || cargo_toml.contains("version =");
    assert!(
        cargo_toml.contains("adze = {") && has_path_or_version,
        "Cargo.toml must declare adze with a path or version dependency"
    );
    assert!(
        cargo_toml.contains("adze-tool = {"),
        "Cargo.toml must declare adze-tool as a build dependency"
    );
    assert!(
        cargo_toml.contains("pure-rust = [\"adze/pure-rust\"]"),
        "Cargo.toml must forward the pure-rust feature to the adze dependency"
    );
}

#[test]
fn test_init_cargo_toml_has_no_verbatim_path_prefix() {
    let temp = tempfile::tempdir().expect("tempdir");
    let project_name = "verbatimtest";
    let mut init = cargo_bin_cmd!("adze");
    init.arg("init")
        .arg(project_name)
        .arg("--output")
        .arg(temp.path())
        .assert()
        .success();

    let cargo_toml = std::fs::read_to_string(temp.path().join(project_name).join("Cargo.toml"))
        .expect("read Cargo.toml");

    assert!(
        !cargo_toml.contains(r"\\?"),
        "Cargo.toml must not leak Windows verbatim path prefixes: {cargo_toml}"
    );
    assert!(
        cargo_toml.contains("adze = {") && cargo_toml.contains("adze-tool = {"),
        "Cargo.toml must still declare working adze dependencies: {cargo_toml}"
    );
}

#[test]
fn test_init_readme_teaches_parse_and_parse_document() {
    let temp = tempfile::tempdir().expect("tempdir");
    let project_name = "readmepath";
    let mut init = cargo_bin_cmd!("adze");
    init.arg("init")
        .arg(project_name)
        .arg("--output")
        .arg(temp.path())
        .assert()
        .success();

    let readme = std::fs::read_to_string(temp.path().join(project_name).join("README.md"))
        .expect("read generated README");

    assert!(
        readme.contains("grammar::parse(\"1 + 2 * 3\")"),
        "generated README should teach the typed parser path"
    );
    assert!(
        readme.contains("Which API Should I Use?"),
        "generated README should make the API choice explicit"
    );
    assert!(
        readme.contains("grammar::parse_document(\"1 +\")"),
        "generated README should teach the document parser path"
    );
    assert!(
        readme.contains("document.diagnostics()"),
        "generated README should point diagnostics users at document facts"
    );
    assert!(
        readme.contains("renders parse errors"),
        "generated README should explain the runnable error example"
    );
    assert!(
        readme.contains("cargo test"),
        "generated README should keep the starter proof command visible"
    );
    assert!(
        readme.contains("cargo run --example parse -- \"1 + 2 * 3\""),
        "generated README should keep the parse example command visible"
    );
}

#[test]
fn test_parse_help_documents_available_modes() {
    let mut cmd = cargo_bin_cmd!("adze");
    cmd.arg("parse")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("tree"))
        .stdout(predicate::str::contains("json"))
        .stdout(predicate::str::contains("sexp"))
        .stdout(predicate::str::contains("dot"))
        .stdout(predicate::str::contains("document-json"))
        .stdout(predicate::str::contains("tree-json"))
        .stdout(predicate::str::contains("diagnostics-json"))
        .stdout(predicate::str::contains("ambiguity-json"))
        .stdout(predicate::str::contains("unavailable"))
        .stdout(predicate::str::contains("experimental load-check").not());
}

#[test]
fn test_parse_dynamic_rejected_before_loading_any_library() {
    // Both files exist, so the failure must be the capability rejection
    // itself: no library is loaded and no resolved symbol is ever called,
    // with or without the `dynamic` feature enabled.
    let temp = tempfile::tempdir().expect("tempdir");
    let grammar = temp.path().join("grammar.so");
    let input = temp.path().join("input.txt");
    std::fs::write(&grammar, "not a shared library").expect("write grammar");
    std::fs::write(&input, "1").expect("write input");

    let mut cmd = cargo_bin_cmd!("adze");
    cmd.arg("parse")
        .arg(&grammar)
        .arg(&input)
        .arg("--dynamic")
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "Dynamic parse mode is unavailable",
        ))
        .stderr(predicate::str::contains("no grammar library was loaded"))
        .stderr(predicate::str::contains("currently unimplemented").not())
        .stderr(predicate::str::contains("dynamic grammar not found").not());
}

#[test]
fn test_cli_build_without_out_dir_dir_and_file_forms() {
    let temp = tempfile::tempdir().expect("tempdir");
    let caller = temp.path().join("caller");
    std::fs::create_dir_all(&caller).expect("create caller dir");
    // An ancestor component contains "grammar"; discovery must use the
    // project entrypoint, not every path whose full name contains it.
    let ancestor = temp.path().join("grammar-ancestor");
    let mut init = cargo_bin_cmd!("adze");
    init.arg("init")
        .arg("buildlang")
        .arg("--output")
        .arg(&ancestor)
        .assert()
        .success();
    let project = ancestor.join("buildlang");
    let grammar = project.join("src/grammar.rs");
    let grammar_before = std::fs::read(&grammar).expect("read grammar");
    std::fs::write(project.join("src/grammar_notes.rs"), "not rust !!!\n")
        .expect("write unrelated decoy");

    let mut dir_build = cargo_bin_cmd!("adze");
    dir_build
        .current_dir(&caller)
        .env_remove("OUT_DIR")
        .env_remove("CARGO_FEATURE_PURE_RUST")
        .env_remove("ADZE_USE_PURE_RUST")
        .arg("build")
        .arg(&project)
        .assert()
        .success()
        .stdout(predicate::str::contains("Grammar built successfully"));
    let artifact = project.join("target/adze-build/grammar_buildlang/parser_buildlang.rs");
    let code = std::fs::read_to_string(&artifact).expect("read dir-form artifact");
    assert!(code.contains("GRAMMAR_NAME"), "artifact: {artifact:?}");
    assert!(!code.is_empty());

    let custom = temp.path().join("custom-out");
    let mut file_build = cargo_bin_cmd!("adze");
    file_build
        .current_dir(&caller)
        .env_remove("OUT_DIR")
        .env_remove("CARGO_FEATURE_PURE_RUST")
        .env_remove("ADZE_USE_PURE_RUST")
        .arg("build")
        .arg(&grammar)
        .arg("--output")
        .arg(&custom)
        .assert()
        .success();
    assert!(
        custom
            .join("grammar_buildlang/parser_buildlang.rs")
            .is_file(),
        "file form must emit into the declared --output directory"
    );

    // Relative directory target resolves against the caller, outputs stay
    // under the selected project.
    let mut rel_build = cargo_bin_cmd!("adze");
    rel_build
        .current_dir(temp.path())
        .env_remove("OUT_DIR")
        .arg("build")
        .arg("grammar-ancestor/buildlang")
        .assert()
        .success();
    assert_eq!(
        std::fs::read(&grammar).expect("read grammar after builds"),
        grammar_before,
        "build must preserve user grammar source"
    );
}

#[test]
fn test_cli_build_without_out_dir_errors_without_panic() {
    let temp = tempfile::tempdir().expect("tempdir");
    let empty = temp.path().join("empty");
    std::fs::create_dir_all(&empty).expect("create empty dir");
    let broken = temp.path().join("broken.rs");
    std::fs::write(&broken, "fn broken( {\n").expect("write broken fixture");
    let plain = temp.path().join("plain.rs");
    std::fs::write(&plain, "pub struct Plain;\n").expect("write plain fixture");
    let blocker = temp.path().join("blocker.txt");
    std::fs::write(&blocker, "blocker").expect("write blocker");

    for (args, extra) in [
        (vec![temp.path().join("missing")], None),
        (vec![empty.clone()], None),
        (vec![broken.clone()], None),
        (vec![plain.clone()], None),
        (vec![plain.clone()], Some(blocker.clone())),
    ] {
        let mut cmd = cargo_bin_cmd!("adze");
        cmd.current_dir(temp.path())
            .env_remove("OUT_DIR")
            .env_remove("CARGO_FEATURE_PURE_RUST")
            .env_remove("ADZE_USE_PURE_RUST")
            .arg("build")
            .args(&args);
        if let Some(output) = &extra {
            cmd.arg("--output").arg(output);
        }
        cmd.assert()
            .failure()
            .stderr(predicate::str::contains("panicked").not())
            .stdout(predicate::str::contains("built successfully").not());
    }
    assert_eq!(
        std::fs::read_to_string(&blocker).expect("read blocker"),
        "blocker",
        "unwritable-output failure must leave the blocking file untouched"
    );
}

// ---------------------------------------------------------------------------
// Unit tests for CLI argument parsing (no binary execution)
// ---------------------------------------------------------------------------

// Import the CLI types directly for parsing tests.
// The types are pub(crate) so we use `try_parse_from` on the binary's types
// via the clap trait. We re-derive a minimal mirror here to avoid exposing
// internal types outside the crate.
mod parsing {
    use clap::Parser;

    /// Minimal mirror of the real CLI struct for argument parsing tests.
    #[derive(Parser, Debug)]
    #[command(name = "adze")]
    #[command(about = "Adze CLI - Tools for grammar development")]
    #[command(author, version, long_about = None)]
    struct Cli {
        #[arg(short, long, global = true)]
        verbose: bool,

        #[command(subcommand)]
        command: Commands,
    }

    #[derive(clap::Subcommand, Debug)]
    enum Commands {
        Init {
            name: String,
            #[arg(short, long)]
            output: Option<std::path::PathBuf>,
        },
        Build {
            #[arg(default_value = ".")]
            path: std::path::PathBuf,
            #[arg(short, long)]
            watch: bool,
        },
        Parse {
            grammar: std::path::PathBuf,
            input: std::path::PathBuf,
            #[arg(short, long, visible_alias = "output", default_value = "tree")]
            format: OutputFormat,
            #[arg(long)]
            dynamic: bool,
            #[arg(long, default_value = "language")]
            symbol: String,
        },
        Test {
            #[arg(default_value = ".")]
            path: std::path::PathBuf,
            #[arg(short, long)]
            update: bool,
        },
        Doc {
            grammar: std::path::PathBuf,
            #[arg(short, long)]
            output: Option<std::path::PathBuf>,
        },
        Check {
            grammar: std::path::PathBuf,
        },
        Stats {
            grammar: std::path::PathBuf,
        },
        Version,
    }

    #[derive(clap::ValueEnum, Clone, Debug)]
    enum OutputFormat {
        Tree,
        Json,
        Sexp,
        Dot,
        #[value(name = "document-json")]
        DocumentJson,
        #[value(name = "tree-json")]
        TreeJson,
        #[value(name = "diagnostics-json")]
        DiagnosticsJson,
        #[value(name = "ambiguity-json")]
        AmbiguityJson,
    }

    // --- argument parsing unit tests ---

    #[test]
    fn parse_check_subcommand() {
        let cli = Cli::try_parse_from(["adze", "check", "grammar.rs"]).unwrap();
        assert!(!cli.verbose);
        match cli.command {
            Commands::Check { grammar } => {
                assert_eq!(grammar.to_str().unwrap(), "grammar.rs");
            }
            _ => panic!("expected Check command"),
        }
    }

    #[test]
    fn parse_stats_subcommand() {
        let cli = Cli::try_parse_from(["adze", "stats", "my_grammar.rs"]).unwrap();
        match cli.command {
            Commands::Stats { grammar } => {
                assert_eq!(grammar.to_str().unwrap(), "my_grammar.rs");
            }
            _ => panic!("expected Stats command"),
        }
    }

    #[test]
    fn parse_version_subcommand() {
        let cli = Cli::try_parse_from(["adze", "version"]).unwrap();
        assert!(matches!(cli.command, Commands::Version));
    }

    #[test]
    fn parse_verbose_flag_global() {
        let cli = Cli::try_parse_from(["adze", "-v", "version"]).unwrap();
        assert!(cli.verbose);
    }

    #[test]
    fn parse_init_with_output() {
        let cli = Cli::try_parse_from(["adze", "init", "my-lang", "-o", "/tmp/out"]).unwrap();
        match cli.command {
            Commands::Init { name, output } => {
                assert_eq!(name, "my-lang");
                assert_eq!(output.unwrap().to_str().unwrap(), "/tmp/out");
            }
            _ => panic!("expected Init command"),
        }
    }

    #[test]
    fn parse_build_defaults() {
        let cli = Cli::try_parse_from(["adze", "build"]).unwrap();
        match cli.command {
            Commands::Build { path, watch } => {
                assert_eq!(path.to_str().unwrap(), ".");
                assert!(!watch);
            }
            _ => panic!("expected Build command"),
        }
    }

    #[test]
    fn parse_build_with_watch() {
        let cli = Cli::try_parse_from(["adze", "build", "src/", "--watch"]).unwrap();
        match cli.command {
            Commands::Build { path, watch } => {
                assert_eq!(path.to_str().unwrap(), "src/");
                assert!(watch);
            }
            _ => panic!("expected Build command"),
        }
    }

    #[test]
    fn parse_parse_command_full() {
        let cli = Cli::try_parse_from([
            "adze",
            "parse",
            "gram.rs",
            "input.txt",
            "--format",
            "json",
            "--dynamic",
            "--symbol",
            "my_lang",
        ])
        .unwrap();
        match cli.command {
            Commands::Parse {
                grammar,
                input,
                dynamic,
                symbol,
                ..
            } => {
                assert_eq!(grammar.to_str().unwrap(), "gram.rs");
                assert_eq!(input.to_str().unwrap(), "input.txt");
                assert!(dynamic);
                assert_eq!(symbol, "my_lang");
            }
            _ => panic!("expected Parse command"),
        }
    }

    #[test]
    fn parse_parse_command_accepts_output_alias_and_document_json() {
        let cli = Cli::try_parse_from([
            "adze",
            "parse",
            "gram.rs",
            "input.txt",
            "--output",
            "document-json",
        ])
        .unwrap();
        match cli.command {
            Commands::Parse { format, .. } => {
                assert!(matches!(format, OutputFormat::DocumentJson));
            }
            _ => panic!("expected Parse command"),
        }
    }

    #[test]
    fn parse_test_with_update() {
        let cli = Cli::try_parse_from(["adze", "test", "--update"]).unwrap();
        match cli.command {
            Commands::Test { path, update } => {
                assert_eq!(path.to_str().unwrap(), ".");
                assert!(update);
            }
            _ => panic!("expected Test command"),
        }
    }

    #[test]
    fn parse_doc_subcommand() {
        let cli = Cli::try_parse_from(["adze", "doc", "grammar.rs", "-o", "docs.md"]).unwrap();
        match cli.command {
            Commands::Doc { grammar, output } => {
                assert_eq!(grammar.to_str().unwrap(), "grammar.rs");
                assert_eq!(output.unwrap().to_str().unwrap(), "docs.md");
            }
            _ => panic!("expected Doc command"),
        }
    }

    #[test]
    fn parse_check_missing_arg_fails() {
        assert!(Cli::try_parse_from(["adze", "check"]).is_err());
    }

    #[test]
    fn parse_stats_missing_arg_fails() {
        assert!(Cli::try_parse_from(["adze", "stats"]).is_err());
    }

    #[test]
    fn parse_unknown_subcommand_fails() {
        assert!(Cli::try_parse_from(["adze", "foobar"]).is_err());
    }

    #[test]
    fn parse_no_subcommand_fails() {
        assert!(Cli::try_parse_from(["adze"]).is_err());
    }
}
