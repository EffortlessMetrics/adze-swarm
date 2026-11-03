#!/usr/bin/env python3
"""Manual real-Cargo input invalidation controls; one tiny generated grammar."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import tempfile
import time


def main():
    if os.name != "posix":
        raise SystemExit("Initial owned-process-group controls require POSIX; Windows is unproved.")
    options = argparse.ArgumentParser()
    options.add_argument("--target-dir", type=Path, help="explicitly owned reusable target")
    args = options.parse_args()
    template = Path(__file__).resolve().parent
    workspace = template.parents[3]
    receipt_dir = Path(tempfile.mkdtemp(prefix="adze-build-inputs-proof-"))
    project = receipt_dir / "project"
    target = args.target_dir.resolve() if args.target_dir else receipt_dir / "target"
    shutil.copytree(template, project, ignore=shutil.ignore_patterns("__pycache__"))
    manifest = project / "Cargo.toml"
    text = manifest.read_text().replace('"../../../../runtime"', json.dumps(str(workspace / "runtime")))
    manifest.write_text(text.replace('"../../.."', json.dumps(str(workspace / "tool"))))
    counter = receipt_dir / "invocations.txt"
    env = os.environ.copy()
    for name in ["ADZE_USE_PURE_RUST", "ADZE_EMIT_ARTIFACTS", "ADZE_DEBUG_FILE", "ADZE_REQUIRE_TS_CLI", "RUST_LOG"]:
        env.pop(name, None)
    env.update(CARGO_BUILD_JOBS="2", CARGO_INCREMENTAL="0", CARGO_PROFILE_DEV_DEBUG="0",
               CARGO_PROFILE_TEST_DEBUG="0", CARGO_TARGET_DIR=str(target),
               ADZE_TEST_BUILD_COUNT_FILE=str(counter), CARGO_LOG="cargo::core::compiler::fingerprint=info")
    cargo = env.get("CARGO", "cargo")
    base = [cargo, "run", "--offline", "--locked", "--quiet", "--message-format=json", "--manifest-path", str(manifest)]
    # Runtime argv changes the independent oracle, not the build-script inputs.
    (project / "src/main.rs").write_text('''use adze_build_invalidation_fixture::grammar::{Word, parse};
fn main() {
    let expected = std::env::args().nth(1).unwrap_or_else(|| "hello".to_owned());
    assert_eq!(parse(&expected).expect("independent exact value"), Word { value: () });
    if expected != "hello" { assert!(parse("hello").is_err()); }
    println!("tiny-fixture-ok");
}
''')
    receipt = {"schema": "adze.build-input-controls.v1", "runs": [],
               "control_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
               "per_run_timeout_seconds": 180,
               "total_timeout_seconds": 600, "source_production_hashes": {
                   name: hashlib.sha256((workspace/name).read_bytes()).hexdigest() for name in
                   ["tool/src/lib.rs", "tool/src/pure_rust_builder.rs", "tool/src/build_parsers/pure_rust.rs"]}}
    deadline = time.monotonic() + 600
    expected_count = 0
    problem = None

    def run(label, command):
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            raise AssertionError("total control budget exhausted")
        stdout_path, stderr_path = receipt_dir/(label+".stdout"), receipt_dir/(label+".stderr")
        files = [manifest, project/"Cargo.lock", project/"build.rs", *(project/"src").rglob("*.rs")]
        files += [path for path in [receipt_dir/"root.rs", receipt_dir/"authored.rs", receipt_dir/"external/authored.rs"] if path.exists()]
        inputs = {str(path.relative_to(receipt_dir)): hashlib.sha256(path.read_bytes()).hexdigest() for path in files}
        termination = "exit"
        with stdout_path.open("wb") as stdout, stderr_path.open("wb") as stderr:
            child = subprocess.Popen(command, cwd=project, env=env, stdout=stdout, stderr=stderr, start_new_session=True)
            try:
                code = child.wait(timeout=min(180, remaining))
            except subprocess.TimeoutExpired:
                termination = "timeout"
                os.killpg(child.pid, signal.SIGKILL)
                code = child.wait()
        out, err = stdout_path.read_text(errors="replace"), stderr_path.read_text(errors="replace")
        count = int(counter.read_text()) if counter.exists() else 0
        row = {"label": label, "command": command, "exit_code": code, "termination": termination,
               "correctness_marker": "tiny-fixture-ok" in out, "build_script_invocations": count,
               "input_hashes": inputs, "stdout": str(stdout_path), "stderr": str(stderr_path)}
        for line in out.splitlines():
            try:
                message = json.loads(line)
            except json.JSONDecodeError:
                continue
            if message.get("reason") == "build-script-executed" and "adze-build-invalidation-fixture" in message.get("package_id", ""):
                row["build_script_out_dir"] = message["out_dir"]
        receipt["runs"].append(row)
        print(json.dumps({key: row[key] for key in ["label", "exit_code", "correctness_marker", "build_script_invocations"]}), flush=True)
        assert termination == "exit", f"{label}: timeout is not qualification"
        return code, out, err, count

    def parse_case(label, value="hello", *, rebuild=True, failure=None, flags=()):
        nonlocal expected_count
        result = run(label, base + list(flags) + ["--", value])
        code, out, err, count = result
        if rebuild:
            expected_count += 1
        assert count == expected_count, f"{label}: expected build count{expected_count}, got{count}"
        if failure is None:
            assert code == 0 and "tiny-fixture-ok" in out, f"{label}: correct parse not established: {err[-3000:]}"
        else:
            assert code != 0 and "tiny-fixture-ok" not in out and failure in out + err, f"{label}: expected explicit {failure} failure: {err[-3000:]}"
        return result

    try:
        parse_case("initial")
        parse_case("unchanged", rebuild=False)
        root = project / "src/lib.rs"
        root.write_text(root.read_text()+"\n// root input mutation\n")
        parse_case("root_changed")
        child_source = project / "src/authored.rs"
        child_source.write_text(child_source.read_text().replace('text = "hello"', 'text = "world"'))
        parse_case("child_changed", "world")
        parse_case("unchanged_after_child", "world", rebuild=False)
        root.write_text(root.read_text()+"\nmod absent;\n")
        parse_case("missing_module", "world", failure="E0583")
        (project / "src/absent.rs").write_text("// newly available module\n")
        parse_case("missing_module_added", "world")
        alternate = project / "src/authored/mod.rs"
        alternate.parent.mkdir()
        alternate.write_text(child_source.read_text())
        parse_case("alternative_added", "world", failure="E0761")
        alternate.unlink()
        parse_case("alternative_removed", "world")
        child_source.rename(alternate)
        parse_case("selected_module_moved", "world")
        external = receipt_dir / "external/authored.rs"
        external.parent.mkdir()
        shutil.copyfile(alternate, external)
        root.write_text('#[path = "../../external/authored.rs"]\nmod authored;\npub use authored::grammar;\n')
        parse_case("external_path_selected", "world")
        external.write_text(external.read_text().replace('text = "world"', 'text = "hello"'))
        parse_case("external_child_changed")
        parse_case("external_unchanged", rebuild=False)
        env["ADZE_EMIT_ARTIFACTS"] = "true"
        parse_case("debug_option_changed")
        outputs = [Path(receipt["runs"][-1]["build_script_out_dir"]) / "grammar_tiny_build_invalidation/parser_tiny_build_invalidation.rs"]
        assert len(outputs) == 1, "one active default-profile generated output"
        assert (outputs[0].parent / "NODE_TYPES.json").is_file()
        assert (outputs[0].parent / "grammar.ir.json").is_file()
        parse_case("debug_option_unchanged", rebuild=False)
        outputs[0].unlink()  # Only this control's exact owned generated module.
        code, out, err, count = run("generated_output_removed", base + ["--", "hello"])
        if code == 0:
            expected_count += 1
            assert count == expected_count and "tiny-fixture-ok" in out, "missing output must regenerate, not run stale binary"
            receipt["output_removal"] = "regenerated"
        else:
            assert count == expected_count and "tiny-fixture-ok" not in out
            assert "couldn't read" in out + err and "parser_tiny_build_invalidation.rs" in out + err
            receipt["output_removal"] = "explicit_compiler_refusal"
            clean = [cargo, "clean", "--offline", "--locked", "--manifest-path", str(manifest), "-p", "adze-build-invalidation-fixture"]
            code, _, err, count = run("owned_package_clean", clean)
            assert code == 0 and count == expected_count, err
            parse_case("removed_output_clean_recovery")
        env["ADZE_USE_PURE_RUST"] = "1"
        parse_case("backend_env_selected", flags=("--no-default-features",))
        env["ADZE_USE_PURE_RUST"] = "0"  # Backend selector uses presence, not text.
        parse_case("consulted_backend_env_changed", flags=("--no-default-features",))
        parse_case("backend_env_unchanged", rebuild=False, flags=("--no-default-features",))
        # Qualify the ordinary backend switch independently of the separately
        # reproduced mixed-backend + debug-artifact deletion defect.
        env.pop("ADZE_EMIT_ARTIFACTS")
        parse_case("debug_option_disabled", flags=("--no-default-features",))
        env.pop("ADZE_USE_PURE_RUST")
        parse_case("backend_env_removed_selects_legacy_c", flags=("--no-default-features",))
        env["ADZE_USE_PURE_RUST"] = "1"
        parse_case("backend_env_restores_pure_rust", flags=("--no-default-features",))
        # An admitted external root can contain OUT_DIR in its parent tree.
        # It remains correct but explicitly opts for conservative regeneration.
        # Keep this oracle genuinely overlapping even with an explicitly
        # reused target outside the new proof directory. Both files are owned.
        overlap_root = target / ("overlap-" + receipt_dir.name + ".rs")
        shutil.copyfile(external, receipt_dir / "authored.rs")
        overlap_root.write_text("#[path = " + json.dumps(str(receipt_dir / "authored.rs")) + "]\nmod authored;\npub use authored::grammar;\n")
        manifest.write_text(manifest.read_text() + '\n[lib]\npath = ' + json.dumps(str(overlap_root)) + '\n')
        build_script = project / "build.rs"
        build_script.write_text(build_script.read_text().replace('"src/lib.rs"', json.dumps(str(overlap_root))))
        parse_case("overlapping_source_output_layout", flags=("--no-default-features",))
        parse_case("overlap_conservative_rerun", flags=("--no-default-features",))
        build_outputs = [Path(receipt["runs"][-1]["build_script_out_dir"]).parent / "output"]
        assert any("source tracking is incomplete or overlaps output; retaining conservative regeneration" in path.read_text() for path in build_outputs)
    except Exception as error:
        problem = str(error)
    finally:
        receipt["result"] = "pass" if problem is None else "not_qualified"
        receipt["problem"] = problem
        path = receipt_dir / "receipt.json"
        path.write_text(json.dumps(receipt, indent=2)+"\n")
        print("receipt="+str(path), flush=True)
    if problem is not None:
        raise SystemExit(problem)


if __name__ == "__main__":
    main()
