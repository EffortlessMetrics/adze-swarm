//! Focused native controls for the starter Cargo journal.

use super::*;
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
fn child_observes_journaled_inputs_before_returning_binary_channels_and_natural_status() {
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
fn intent_failure_never_starts_child_and_spawn_failure_records_no_status() {
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
fn completion_failure_keeps_natural_status_and_missing_plan_is_not_success() {
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
fn aggregate_refuses_orphan_entry_and_changed_completion() {
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
fn high_bit_windows_status_is_retained_but_not_a_parser_refusal() {
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
fn signal_is_not_an_invalid_parse_exit_code() {
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
fn production_starter_plan_uses_the_nondefault_program_and_verifies_all_three() {
    let root = tempfile::tempdir().unwrap();
    let project = root.path().join("project λ");
    fs::create_dir(&project).unwrap();
    let cargo = root
        .path()
        .join(format!("cargo fixture λ{}", std::env::consts::EXE_SUFFIX));
    let shim = root.path().join("fixture.rs");
    fs::write(&shim, r#"
fn main() {
    let cwd = std::env::current_dir().unwrap();
    assert_eq!(cwd.file_name().unwrap(), "project λ");
    let parent = cwd.parent().unwrap();
    assert_eq!(std::path::PathBuf::from(std::env::var_os("CARGO_HOME").unwrap()), parent.join("home"));
    assert_eq!(std::path::PathBuf::from(std::env::var_os("CARGO_TARGET_DIR").unwrap()), parent.join("target"));
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args == ["test"] { return; }
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
    let results = run(
        root.path(),
        source(),
        cargo.as_os_str(),
        &project,
        &root.path().join("home"),
        &root.path().join("target"),
    )
    .unwrap();
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
fn actual_cargo_builds_and_runs_an_outside_checkout_fixture() {
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
        "#[test] fn fixture() { assert_eq!(2+2,4); }",
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
    let commands = run(
        root.path(),
        source(),
        &cargo,
        &project,
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
    assert!(project.join("Cargo.lock").is_file());
}
