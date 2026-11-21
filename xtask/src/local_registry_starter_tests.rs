//! Focused native controls for the starter Cargo journal.

use super::*;
use crate::local_release_command::{SourceBinding, sha256};
use serde_json::Value;
use tempfile::TempDir;

fn source() -> SourceBinding {
    SourceBinding {
        source_commit: "1".repeat(40),
        source_tree: "2".repeat(40),
        selected_version: "0.10.0".into(),
    }
}

fn json(path: &Path) -> Value {
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

fn fixture() -> (TempDir, CommandJournal) {
    let root = tempfile::Builder::new()
        .prefix("starter journal λ ")
        .tempdir()
        .unwrap();
    let j = CommandJournal::create(root.path(), source(), limits()).unwrap();
    (root, j)
}

fn child(root: &Path, script: &str, additional: &[&str]) -> ReleaseCommand {
    let program = ["python3", "python"]
        .into_iter()
        .find(|p| {
            Command::new(p)
                .arg("--version")
                .output()
                .is_ok_and(|o| o.status.success())
        })
        .expect("Python is required by the local registry harness");
    ReleaseCommand::new(
        "starter-fixture".into(),
        program.into(),
        ["-c", script]
            .into_iter()
            .chain(additional.iter().copied())
            .map(OsString::from)
            .collect(),
        root.into(),
        vec![
            EnvironmentChange::Set {
                name: "ADZE_FIXTURE".into(),
                value: "old".into(),
                record_value: true,
            },
            EnvironmentChange::Remove("ADZE_FIXTURE".into()),
            EnvironmentChange::Set {
                name: "ADZE_FIXTURE".into(),
                value: "private λ".into(),
                record_value: false,
            },
        ],
        policy(),
    )
    .unwrap()
}

#[test]
fn test_child_with_journaled_inputs_retains_binary_channels_and_natural_status() {
    let (root, mut journal) = fixture();
    let script = r#"import json, os, pathlib, sys
req = json.loads(pathlib.Path('commands/00000001/intent.json').read_bytes())['request']
assert pathlib.Path(req['cwd']['display']).samefile(pathlib.Path.cwd())
assert [v['display'] for v in req['arguments'][2:]] == sys.argv[1:] == ['', 'space λ']
assert os.environ['ADZE_FIXTURE'] == 'private λ'
sys.stdout.buffer.write(bytes([255, 0, 97]))
sys.stderr.buffer.write(bytes([254, 98]))
sys.exit(7)
"#;
    let observation = capture(&mut journal, child(root.path(), script, &["", "space λ"])).unwrap();
    assert_eq!(observation.natural_status().unwrap().code(), Some(7));
    assert_eq!(observation.stdout(), [255, 0, 97]);
    assert_eq!(observation.stderr(), [254, 98]);
    assert!(
        project(&observation)
            .unwrap_err()
            .to_string()
            .contains("not UTF-8")
    );
    let completed = json(observation.completion_path());
    assert_eq!(completed["package_first_qualified"], false);
    assert_eq!(completed["natural_status"]["code"], 7);
    assert_eq!(
        fs::read(root.path().join("commands/00000001/stdout.bin")).unwrap(),
        [255, 0, 97]
    );
    let intent = json(&root.path().join("commands/00000001/intent.json"));
    assert!(
        !intent["request"]["environment"]
            .to_string()
            .contains("private")
    );
    assert_eq!(
        intent["request"]["inherited_environment_identity"],
        "unknown"
    );
}

#[test]
fn test_capture_with_failed_intent_or_spawn_never_invents_status() {
    let (root, mut journal) = fixture();
    fs::create_dir(root.path().join("commands/00000001")).unwrap();
    let request = child(root.path(), "open('started', 'w').close()", &[]);
    assert!(capture(&mut journal, request).is_err());
    assert!(!root.path().join("started").exists());
    let (root, mut journal) = fixture();
    let req = ReleaseCommand::new(
        "missing".into(),
        root.path().join("absent").into_os_string(),
        vec![],
        root.path().into(),
        vec![],
        policy(),
    )
    .unwrap();
    let error = capture(&mut journal, req).err().unwrap();
    assert_eq!(
        error.downcast_ref::<std::io::Error>().unwrap().kind(),
        std::io::ErrorKind::NotFound
    );
    let observation = json(&root.path().join("commands/00000001/completion.json"));
    assert!(observation["natural_status"].is_null());
    assert!(observation["observed_failure"].is_string());
}

#[test]
fn test_completion_write_failure_retains_natural_status_and_rejects_incomplete_plan() {
    let (root, mut journal) = fixture();
    let req = child(
        root.path(),
        "import sys; open('commands/00000001/stdout.bin', 'wb').close(); sys.exit(23)",
        &[],
    );
    let err = capture(&mut journal, req).err().unwrap();
    assert!(format!("{err:#}").contains("23"));
    assert!(format!("{err:#}").contains("stdout.bin"));
    assert!(
        !root
            .path()
            .join("commands/00000001/completion.json")
            .exists()
    );
    assert!(project_plan(root.path(), &[]).is_err());
}

#[test]
fn test_starter_plan_with_orphan_or_changed_completion_rejects_aggregate() {
    let (root, mut journal) = fixture();
    let mut observations = Vec::new();
    for (name, _) in PLAN {
        let request = child(root.path(), "pass", &[]);
        let named = ReleaseCommand::new(
            name.into(),
            request.program().into(),
            request.arguments().to_vec(),
            request.cwd().into(),
            request.environment().to_vec(),
            request.policy(),
        )
        .unwrap();
        observations.push(capture(&mut journal, named).unwrap());
    }
    assert_eq!(project_plan(root.path(), &observations).unwrap().len(), 3);
    let orphan = root.path().join("commands/00000004");
    fs::create_dir(&orphan).unwrap();
    fs::write(orphan.join("intent.json"), b"orphan").unwrap();
    assert!(
        project_plan(root.path(), &observations)
            .unwrap_err()
            .to_string()
            .contains("unexpected entry")
    );
    fs::rename(&orphan, root.path().join("retained orphan")).unwrap();
    observations.swap(0, 1);
    assert!(
        project_plan(root.path(), &observations)
            .unwrap_err()
            .to_string()
            .contains("ordered plan")
    );
    observations.swap(0, 1);
    fs::write(observations[1].completion_path(), b"changed").unwrap();
    let error = project_plan(root.path(), &observations).unwrap_err();
    let context = error.to_string();
    assert!(context.contains("starter-parse-valid"), "{error:#}");
    assert!(
        context.contains(observations[1].completion_path().to_str().unwrap()),
        "{error:#}"
    );
}

#[cfg(windows)]
#[test]
fn test_windows_high_bit_status_is_retained_not_a_parser_refusal() {
    let (root, mut journal) = fixture();
    let req = child(
        root.path(),
        "import ctypes; ctypes.windll.kernel32.ExitProcess(ctypes.c_uint(0xc000013a))",
        &[],
    );
    let obs = capture(&mut journal, req).unwrap();
    assert_eq!(
        json(obs.completion_path())["natural_status"]["code"],
        0xc000_013a_u32
    );
    assert!(
        project(&obs)
            .unwrap_err()
            .to_string()
            .contains("not a portable starter exit code")
    );
}

#[cfg(unix)]
#[test]
fn test_unix_signal_is_retained_not_an_invalid_parse_exit_code() {
    let (root, mut journal) = fixture();
    let req = child(
        root.path(),
        "import os, signal; os.kill(os.getpid(), signal.SIGTERM)",
        &[],
    );
    let obs = capture(&mut journal, req).unwrap();
    assert!(obs.natural_status().unwrap().code().is_none());
    assert_eq!(json(obs.completion_path())["natural_status"]["signal"], 15);
    assert!(
        project(&obs)
            .unwrap_err()
            .to_string()
            .contains("without an exit code")
    );
}

#[test]
fn test_starter_plan_with_nondefault_cargo_runs_all_three_requests() {
    let root = tempfile::tempdir().unwrap();
    let project = root.path().join("project λ");
    let home = root.path().join("home");
    let target = root.path().join("target");
    for path in [&project, &home, &target] {
        fs::create_dir(path).unwrap();
    }
    fs::write(
        project.join("Cargo.toml"),
        "[package]\nname='fixture'\nversion='0.0.0'\n",
    )
    .unwrap();
    let cargo = root
        .path()
        .join(format!("cargo fixture λ{}", std::env::consts::EXE_SUFFIX));
    let shim = root.path().join("fixture.rs");
    fs::write(&shim, r#"
fn main() {
    let cwd = std::env::current_dir().unwrap();
    assert_eq!(cwd.file_name().unwrap(), "project λ");
    let parent = cwd.parent().unwrap();
    // Windows can report the working directory with a different spelling
    // of its ancestor paths (for example, long names instead of 8.3 aliases).
    for (name, leaf) in [("CARGO_HOME", "home"), ("CARGO_TARGET_DIR", "target")] {
        let actual = std::path::PathBuf::from(std::env::var_os(name).unwrap());
        assert!(actual.is_absolute(), "{name} must be absolute: {actual:?}");
        assert_eq!(
            actual.canonicalize().unwrap(),
            parent.join(leaf).canonicalize().unwrap(),
            "{name} points to the wrong directory",
        );
    }
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args == ["test"] { std::fs::write("Cargo.lock","version = 4\n[[package]]\nname='fixture'\nversion='0.0.0'\n").unwrap(); return; }
    if args == ["run","--example","parse","--","1 + 2 * 3"] {
        println!("Add(Number(1), Mul(Number(2), Number(3)))");
    } else if args == ["run","--example","parse","--","1 + @"] {
        eprintln!("expected one of: Number"); std::process::exit(23);
    } else { panic!("unexpected argv: {args:?}"); }
}
"#).unwrap();
    let rustc = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
    let built = Command::new(rustc)
        .arg("--edition=2024")
        .arg(&shim)
        .arg("-o")
        .arg(&cargo)
        .output()
        .unwrap();
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let (mut inputs, generated) = StarterInputs::create(root.path(), source(), &project).unwrap();
    inputs.expect_effective(generated.as_bytes()).unwrap();
    let results = run(&mut inputs, cargo.as_os_str(), &home, &target).unwrap();
    assert_eq!(results.len(), 3);
    assert_eq!(
        results.iter().map(|r| r.exit_code).collect::<Vec<_>>(),
        [0, 0, 23]
    );
    for (i, command) in results.iter().enumerate() {
        assert_eq!(command.argv[0], cargo.to_str().unwrap());
        let dir = root.path().join(format!("commands/{:08}", i + 1));
        let intent = json(&dir.join("intent.json"));
        assert_eq!(intent["request"]["name"], command.name);
        assert_eq!(intent["request"]["program"]["display"], command.argv[0]);
        assert_eq!(
            json(&dir.join("completion.json"))["natural_status"]["code"],
            command.exit_code
        );
    }
}

#[test]
fn test_real_cargo_outside_checkout_builds_and_runs_fixture() {
    let root = tempfile::tempdir().unwrap();
    let project = root.path().join("outside λ");
    fs::create_dir_all(project.join("examples")).unwrap();
    fs::create_dir(project.join("src")).unwrap();
    fs::write(
        project.join("Cargo.toml"),
        "[package]\nname='proof-fixture'\nversion='0.0.0'\nedition='2024'\n[workspace]\n",
    )
    .unwrap();
    fs::write(
        project.join("src/lib.rs"),
        "#[test] fn test_fixture_addition_returns_four() { assert_eq!(2+2,4); }",
    )
    .unwrap();
    fs::write(
        project.join("examples/parse.rs"),
        r#"fn main() {
        match std::env::args().nth(1).as_deref() {
            Some("1 + 2 * 3") => println!("Add(Number(1), Mul(Number(2), Number(3)))"),
            Some("1 + @") => {eprintln!("expected one of: Number"); std::process::exit(23);}
            _ => panic!("wrong argument"),
        }
    }"#,
    )
    .unwrap();
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let (mut inputs, generated) = StarterInputs::create(root.path(), source(), &project).unwrap();
    inputs.expect_effective(generated.as_bytes()).unwrap();
    let commands = run(
        &mut inputs,
        &cargo,
        &root.path().join("home"),
        &root.path().join("target"),
    )
    .unwrap();
    assert!(
        commands[0].stdout.contains("1 passed; 0 failed"),
        "{}",
        commands[0].stdout
    );
    assert_eq!(commands[2].exit_code, 23);
    let lock = fs::read(project.join("Cargo.lock")).unwrap();
    let parsed: toml::Value = toml::from_str(std::str::from_utf8(&lock).unwrap()).unwrap();
    let packages = parsed["package"].as_array().unwrap();
    assert_eq!(packages.len(), 1);
    assert_eq!(packages[0]["name"].as_str(), Some("proof-fixture"));
    assert_eq!(packages[0]["version"].as_str(), Some("0.0.0"));
    let observations = root.path().join("starter-inputs");
    assert_eq!(
        fs::read(observations.join("generated-Cargo.toml")).unwrap(),
        generated.as_bytes()
    );
    assert_eq!(
        json(&observations.join("before.json"))["lock"]["state"],
        "absent"
    );
    for id in 1..=3 {
        let record = json(&observations.join(format!("after-{id:02}.json")));
        assert_eq!(record["journal_slot"], id);
        assert_eq!(record["lock"]["state"], "present");
        assert_eq!(record["lock"]["artifact"]["bytes"], lock.len());
        assert_eq!(record["lock"]["artifact"]["sha256"], sha256(&lock));
        assert_eq!(
            fs::read(observations.join(record["lock"]["artifact"]["path"].as_str().unwrap()))
                .unwrap(),
            lock
        );
    }
    println!(
        "actual outside fixture: generated_manifest {} bytes sha256 {}; cargo_lock {} bytes sha256 {}",
        generated.len(),
        sha256(generated.as_bytes()),
        lock.len(),
        sha256(&lock)
    );
}

#[test]
fn test_starter_inputs_after_cargo_failure_retain_lock_and_three_results() {
    failed_cargo_fixture(false)
}

#[test]
fn test_cargo_failure_with_snapshot_collision_preserves_oracle_and_three_results() {
    failed_cargo_fixture(true)
}

fn failed_cargo_fixture(collision: bool) {
    // A valid local fixture with a compilation error resolves a genuine
    // lockfile, then fails without any registry dependency or network need.
    let root = tempfile::tempdir().unwrap();
    let project = root.path().join("broken");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::create_dir(project.join("examples")).unwrap();
    let original = "[package]\nname='broken'\nversion='0.1.0'\nedition='2024'\n[workspace]\n";
    fs::write(project.join("Cargo.toml"), original).unwrap();
    let (mut inputs, generated) = StarterInputs::create(root.path(), source(), &project).unwrap();
    let effective = format!("{original}\n# bounded fixture for harness manifest preparation\n");
    fs::write(project.join("Cargo.toml"), &effective).unwrap();
    assert_eq!(generated, original);
    inputs.expect_effective(effective.as_bytes()).unwrap();
    fs::write(
        project.join("src/lib.rs"),
        "compile_error!(\"fixture compile refusal\");",
    )
    .unwrap();
    fs::write(project.join("examples/parse.rs"), "fn main() {}").unwrap();
    if collision {
        fs::write(
            root.path().join("starter-inputs/after-01.json"),
            b"prior output",
        )
        .unwrap();
    }
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let err = run(
        &mut inputs,
        &cargo,
        &root.path().join("home"),
        &root.path().join("target"),
    )
    .unwrap_err();
    assert!(
        format!("{err:#}").contains("fixture compile refusal"),
        "{err:#}"
    );
    let directory = root.path().join("starter-inputs");
    assert_eq!(
        fs::read(directory.join("generated-Cargo.toml")).unwrap(),
        original.as_bytes()
    );
    let before = json(&directory.join("before.json"));
    assert_eq!(before["lock"]["state"], "absent");
    assert_eq!(
        fs::read(directory.join(before["effective_manifest"]["path"].as_str().unwrap())).unwrap(),
        effective.as_bytes()
    );
    let lock = fs::read(project.join("Cargo.lock")).unwrap();
    for id in 1..=3 {
        if collision && id == 1 {
            assert_eq!(
                fs::read(directory.join("after-01.json")).unwrap(),
                b"prior output"
            );
            assert!(format!("{err:#}").contains("after-01.json"));
        } else {
            let after = json(&directory.join(format!("after-{id:02}.json")));
            assert_eq!(after["command_outcome"], "status_unsuccessful");
            assert_eq!(after["lock"]["artifact"]["sha256"], sha256(&lock));
        }
        let completion = json(
            &root
                .path()
                .join(format!("commands/{id:08}/completion.json")),
        );
        assert_eq!(completion["natural_status"]["code"], 101);
    }
    println!(
        "failed real Cargo fixture: generated {} bytes sha256 {}; effective {} bytes sha256 {}; lock {} bytes sha256 {}",
        original.len(),
        sha256(original.as_bytes()),
        effective.len(),
        sha256(effective.as_bytes()),
        lock.len(),
        sha256(&lock)
    );
}

#[test]
fn test_starter_inputs_on_spawn_failure_record_absence_without_overwrite() {
    let root = tempfile::tempdir().unwrap();
    let project = root.path().join("absent");
    fs::create_dir(&project).unwrap();
    fs::write(
        project.join("Cargo.toml"),
        "[package]\nname='absent'\nversion='0.1.0'\n",
    )
    .unwrap();
    let (mut inputs, generated) = StarterInputs::create(root.path(), source(), &project).unwrap();
    inputs.expect_effective(generated.as_bytes()).unwrap();
    let cargo = root.path().join("no-such-cargo");
    let err = run(
        &mut inputs,
        cargo.as_os_str(),
        &root.path().join("home"),
        &root.path().join("target"),
    )
    .unwrap_err();
    assert_eq!(
        err.downcast_ref::<std::io::Error>().unwrap().kind(),
        std::io::ErrorKind::NotFound
    );
    let directory = root.path().join("starter-inputs");
    let after = json(&directory.join("after-01.json"));
    assert_eq!(after["lock"]["state"], "absent");
    assert_eq!(after["command_outcome"], "capture_or_recording_error");
    assert_eq!(after["stage"], "after_command_attempt");
    assert!(!directory.join("after-02.json").exists());
    let before = fs::read(directory.join("generated-Cargo.toml")).unwrap();
    assert!(StarterInputs::create(root.path(), source(), &project).is_err());
    assert_eq!(
        fs::read(directory.join("generated-Cargo.toml")).unwrap(),
        before
    );
}

#[test]
fn test_starter_inputs_with_changed_or_tampered_bytes_reject_verification() {
    let root = tempfile::tempdir().unwrap();
    let project = root.path().join("altered");
    fs::create_dir(&project).unwrap();
    fs::write(
        project.join("Cargo.toml"),
        "[package]\nname='changed'\nversion='0.1.0'\n",
    )
    .unwrap();
    let (mut inputs, generated) = StarterInputs::create(root.path(), source(), &project).unwrap();
    inputs.expect_effective(generated.as_bytes()).unwrap();
    inputs.observe(None, "not_started").unwrap();
    for (index, version) in ["0.1.0", "0.1.1"].iter().enumerate() {
        let lock = format!("version=4\n[[package]]\nname='changed'\nversion='{version}'\n");
        fs::write(project.join("Cargo.lock"), lock).unwrap();
        inputs
            .observe(Some((index + 1, PLAN[index].0)), "status_success")
            .unwrap();
    }
    assert!(
        inputs
            .verify(true)
            .unwrap_err()
            .to_string()
            .contains("changed during the journey")
    );
    let dir = root.path().join("starter-inputs");
    let a = json(&dir.join("after-01.json"));
    let b = json(&dir.join("after-02.json"));
    assert_ne!(
        a["lock"]["artifact"]["sha256"],
        b["lock"]["artifact"]["sha256"]
    );
    let file = dir.join(a["lock"]["artifact"]["path"].as_str().unwrap());
    let mut raw = fs::read(&file).unwrap();
    raw[0] = b'X';
    fs::write(&file, raw).unwrap();
    assert!(
        inputs
            .verify(false)
            .unwrap_err()
            .to_string()
            .contains("retained starter input changed")
    );
}

#[test]
fn test_starter_inputs_with_changed_preflight_manifest_refuse_launch_without_collecting_it() {
    let root = tempfile::tempdir().unwrap();
    let project = root.path().join("preflight");
    fs::create_dir(&project).unwrap();
    let manifest = project.join("Cargo.toml");
    fs::write(&manifest, "[package]\nname='f'\nversion='0.0.0'\n").unwrap();
    let (mut inputs, generated) = StarterInputs::create(root.path(), source(), &project).unwrap();
    inputs.expect_effective(generated.as_bytes()).unwrap();
    fs::write(&manifest, "[workspace]\n# replaced before proof\n").unwrap();
    let error = run(
        &mut inputs,
        OsStr::new("unused"),
        &root.path().join("home"),
        &root.path().join("target"),
    )
    .unwrap_err();
    assert!(
        error.to_string().contains("before the first command"),
        "{error:#}"
    );
    assert!(!root.path().join("commands").exists());
    let dir = root.path().join("starter-inputs");
    let before = json(&dir.join("before.json"));
    assert_eq!(before["effective_manifest_matches"], false);
    assert_eq!(
        fs::read(dir.join("effective-Cargo.toml")).unwrap(),
        generated.as_bytes()
    );
    for item in fs::read_dir(&dir).unwrap() {
        assert!(
            !fs::read_to_string(item.unwrap().path())
                .unwrap()
                .contains("replaced before proof")
        );
    }
}

#[test]
fn test_starter_inputs_with_failed_checkpoint_preserve_original_attempt_error() {
    let root = tempfile::tempdir().unwrap();
    let project = root.path().join("collision");
    fs::create_dir(&project).unwrap();
    fs::write(
        project.join("Cargo.toml"),
        "[package]\nname='f'\nversion='0.0.0'\n",
    )
    .unwrap();
    let (mut inputs, generated) = StarterInputs::create(root.path(), source(), &project).unwrap();
    inputs.expect_effective(generated.as_bytes()).unwrap();
    let reserved = root.path().join("starter-inputs/after-01.json");
    fs::write(&reserved, b"previous evidence").unwrap();
    let err = run(
        &mut inputs,
        root.path().join("missing-program").as_os_str(),
        &root.path().join("home"),
        &root.path().join("target"),
    )
    .unwrap_err();
    assert_eq!(
        err.downcast_ref::<std::io::Error>().unwrap().kind(),
        std::io::ErrorKind::NotFound
    );
    assert!(format!("{err:#}").contains("after-01.json"));
    assert_eq!(fs::read(reserved).unwrap(), b"previous evidence");
}

#[test]
fn test_starter_with_existing_lock_drift_before_first_slot_skips_all_commands() {
    let root = tempfile::tempdir().unwrap();
    let project = root.path().join("early");
    fs::create_dir(&project).unwrap();
    fs::write(
        project.join("Cargo.toml"),
        "[package]\nname='f'\nversion='0.0.0'\n",
    )
    .unwrap();
    let lock = project.join("Cargo.lock");
    fs::write(&lock, "version=4\n[[package]]\nname='f'\nversion='0.0.0'\n").unwrap();
    let (mut inputs, generated) = StarterInputs::create(root.path(), source(), &project).unwrap();
    inputs.expect_effective(generated.as_bytes()).unwrap();
    fs::write(&lock, "version=4\n[[package]]\nname='f'\nversion='0.1.0'\n").unwrap();
    let err = run(&mut inputs, OsStr::new("unused"), root.path(), root.path()).unwrap_err();
    assert!(format!("{err:#}").contains("before the first command"));
    let dir = root.path().join("starter-inputs");
    assert!(
        json(&dir.join("before.json"))["known_input_change"]
            .as_bool()
            .unwrap()
    );
    assert_eq!(
        json(&dir.join("stopped.json"))["not_run"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
    assert!(!root.path().join("commands").exists());
}

#[test]
fn test_real_cargo_with_manifest_drift_after_first_slot_skips_later_commands() {
    drift_after_first_slot("Cargo.toml");
}

#[test]
fn test_real_cargo_with_lock_drift_after_first_slot_skips_later_commands() {
    drift_after_first_slot("Cargo.lock");
}

fn drift_after_first_slot(filename: &str) {
    let root = tempfile::tempdir().unwrap();
    let project = root.path().join("during");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(
        project.join("Cargo.toml"),
        "[package]\nname='f'\nversion='0.0.0'\nedition='2024'\n[workspace]\n",
    )
    .unwrap();
    fs::write(project.join("src/lib.rs"),r##"#[test] fn test_fixture_after_input_change_reports_failure() {
        use std::io::Write;
        std::fs::OpenOptions::new().append(true).open("FILE_TO_CHANGE").unwrap().write_all(b"# fixture change\n").unwrap();
        eprintln!("fixture change observed");
        panic!("fixture change observed");
    }"##.replace("FILE_TO_CHANGE",filename)).unwrap();
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let generated_lock = Command::new(&cargo)
        .args(["generate-lockfile", "--offline"])
        .current_dir(&project)
        .env("CARGO_HOME", root.path().join("home"))
        .output()
        .unwrap();
    assert!(generated_lock.status.success());
    let (mut inputs, generated) = StarterInputs::create(root.path(), source(), &project).unwrap();
    inputs.expect_effective(generated.as_bytes()).unwrap();
    let err = run(
        &mut inputs,
        &cargo,
        &root.path().join("home"),
        &root.path().join("target"),
    )
    .unwrap_err();
    assert!(format!("{err:#}").contains("starter inputs changed after starter-cargo-test"));
    let dir = root.path().join("starter-inputs");
    let stopped = json(&dir.join("stopped.json"));
    assert_eq!(
        stopped["not_run"],
        serde_json::json!(["starter-parse-valid", "starter-parse-invalid"])
    );
    assert_eq!(
        json(&dir.join("after-01.json"))["effective_manifest_matches"],
        filename != "Cargo.toml"
    );
    let commands = root.path().join("commands");
    assert_eq!(
        json(&commands.join("00000001/completion.json"))["natural_status"]["code"],
        101
    );
    assert!(
        fs::read_to_string(commands.join("00000001/stdout.bin"))
            .unwrap()
            .contains("fixture change observed")
    );
    assert!(!commands.join("00000002").exists());
    assert!(!commands.join("00000003").exists());
}

#[test]
fn test_starter_with_unexpected_preflight_lock_rejects_new_baseline() {
    let root = tempfile::tempdir().unwrap();
    let project = root.path().join("unexpected-lock");
    fs::create_dir(&project).unwrap();
    fs::write(
        project.join("Cargo.toml"),
        "[package]\nname='f'\nversion='0.0.0'\n",
    )
    .unwrap();
    let (mut inputs, generated) = StarterInputs::create(root.path(), source(), &project).unwrap();
    inputs.expect_effective(generated.as_bytes()).unwrap();
    let early = b"version=4\n[[package]]\nname='f'\nversion='0.0.0'\n";
    fs::write(project.join("Cargo.lock"), early).unwrap();
    let err = run(
        &mut inputs,
        OsStr::new("unneeded-program"),
        root.path(),
        root.path(),
    )
    .unwrap_err();
    assert!(
        format!("{err:#}").contains("changed before the first command"),
        "{err:#}"
    );
    let dir = root.path().join("starter-inputs");
    assert_eq!(json(&dir.join("generated.json"))["lock"]["state"], "absent");
    let before = json(&dir.join("before.json"));
    assert_eq!(before["lock"]["state"], "present");
    assert_eq!(before["lock"]["artifact"]["sha256"], sha256(early));
    assert_eq!(
        fs::read(dir.join(before["lock"]["artifact"]["path"].as_str().unwrap())).unwrap(),
        early
    );
    assert_eq!(before["known_input_change"], true);
    assert_eq!(
        json(&dir.join("stopped.json"))["not_run"],
        serde_json::json!([
            "starter-cargo-test",
            "starter-parse-valid",
            "starter-parse-invalid"
        ])
    );
    assert!(!root.path().join("commands").exists());
}
