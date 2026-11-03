#!/usr/bin/env python3
"""Manual tiny real-Cargo invalidation control; no Python grammar generation."""
import hashlib
import json
import os
from pathlib import Path
import signal
import subprocess
import tempfile


def main():
    if os.name != "posix":
        raise SystemExit("This initial process-group-bounded control requires POSIX; Windows is unproved.")
    fixture = Path(__file__).resolve().parent
    receipt_dir = Path(tempfile.mkdtemp(prefix="adze-build-invalidation-proof-"))
    counter = receipt_dir / "invocations.txt"
    target = receipt_dir / "target"
    inputs = ["Cargo.toml", "Cargo.lock", "build.rs", "src/lib.rs", "src/authored.rs", "src/main.rs"]

    def identity():
        return {name: hashlib.sha256((fixture / name).read_bytes()).hexdigest() for name in inputs}

    before = identity()
    env = os.environ.copy()
    env.update(CARGO_BUILD_JOBS="2", CARGO_INCREMENTAL="0", CARGO_PROFILE_DEV_DEBUG="0",
               CARGO_PROFILE_TEST_DEBUG="0", CARGO_TARGET_DIR=str(target),
               ADZE_TEST_BUILD_COUNT_FILE=str(counter),
               CARGO_LOG="cargo::core::compiler::fingerprint=info")
    command = [env.get("CARGO", "cargo"), "run", "--offline", "--locked", "--quiet",
               "--manifest-path", str(fixture / "Cargo.toml")]
    receipt = {"schema": "adze.build-invalidation-control.v1", "command": command,
               "fixture_inputs": before, "target": str(target), "runs": [],
               "per_run_timeout_seconds": 180, "expected_build_script_invocations": 1,
               "control_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest()}
    problem = None
    try:
        for label in ["first", "unchanged_second"]:
            stdout_path, stderr_path = receipt_dir / (label + ".stdout"), receipt_dir / (label + ".stderr")
            termination = "exit"
            with stdout_path.open("wb") as stdout, stderr_path.open("wb") as stderr:
                child = subprocess.Popen(command, cwd=fixture, env=env, stdout=stdout,
                                         stderr=stderr, start_new_session=True)
                try:
                    code = child.wait(timeout=180)
                except subprocess.TimeoutExpired:
                    termination = "timeout"
                    os.killpg(child.pid, signal.SIGKILL)
                    code = child.wait()
            run = {"label": label, "exit_code": code, "termination": termination,
                   "correctness_marker": "tiny-fixture-ok" in stdout_path.read_text(errors="replace"),
                   "build_script_invocations": int(counter.read_text()) if counter.exists() else None,
                   "stdout": str(stdout_path), "stderr": str(stderr_path)}
            receipt["runs"].append(run)
            print(json.dumps(run), flush=True)
            if code != 0 or not run["correctness_marker"]:
                raise AssertionError(f"{label}: build/parse setup did not establish correct generated parsing")
        if before != identity():
            raise AssertionError("fixture inputs changed during the experiment")
        if receipt["runs"][0]["build_script_invocations"] != 1:
            raise AssertionError("initial build must execute exactly once")
        if receipt["runs"][1]["build_script_invocations"] != 1:
            raise AssertionError("unchanged second Cargo run must not regenerate the parser")
    except Exception as error:
        problem = str(error)
    finally:
        receipt["inputs_unchanged"] = before == identity()
        receipt["result"] = "pass" if problem is None else "not_qualified"
        receipt["problem"] = problem
        receipt_path = receipt_dir / "receipt.json"
        receipt_path.write_text(json.dumps(receipt, indent=2) + "\n")
        print("receipt=" + str(receipt_path), flush=True)
    if problem is not None:
        raise SystemExit(problem)


if __name__ == "__main__":
    main()
