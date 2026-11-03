#!/usr/bin/env python3
"""Qualify the reviewed Linux signal mutant separately from successful proof.

The red command runs only in an owned detached actual-G worktree. Parser tests
use explicitly synthetic output; only this command executes actual Cargo. A CI
watchdog/interrupt is never accepted as the expected regression or cleanup.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parent.parent
FIXTURES = ROOT / "scripts/fixtures/linux-signal"
POLICY_PATH = FIXTURES / "reviewed-source.json"
PATCH_PATH = FIXTURES / "legacy-load-red.patch"
MODULE = "cli/src/runner_process.rs"
SIGNAL_TARGET = "cli/tests/runner_linux_signal_contract.rs"
ORIGINAL_TARGET = "cli/tests/runner_process_contract.rs"
DYNAMIC_INPUTS = ("Cargo.lock", "Cargo.toml", "cli/Cargo.toml", "rust-toolchain.toml")
OUTPUT_LIMIT = 4 << 20
SOURCE_LIMIT = 16 << 20
RED_COMMAND = [
    "cargo", "test", "--locked", "--offline", "-p", "adze-cli",
    "--test", "runner_linux_signal_contract", "test_late_dispatched_",
    "--", "--nocapture", "--test-threads=1",
]
LATE = {
    "test_late_dispatched_sigint_relays_after_atomic_close": "late_int",
    "test_late_dispatched_sigterm_relays_after_atomic_close": "late_term",
}


class Refused(RuntimeError):
    """The exact qualification contract was not established."""


def require(condition: bool, reason: str) -> None:
    if not condition:
        raise Refused(reason)


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def blob(data: bytes) -> str:
    return hashlib.sha1(b"blob " + str(len(data)).encode() + b"\0" + data).hexdigest()


def bounded_read(path: Path, limit: int) -> bytes:
    with path.open("rb") as stream:
        data = stream.read(limit + 1)
    require(len(data) <= limit, f"{path}: exceeds the {limit}-byte inspection bound; raw file retained")
    return data


def unique_object(pairs: list[tuple[str, object]]) -> dict:
    result = {}
    for key, value in pairs:
        require(key not in result, f"duplicate identity field: {key}")
        result[key] = value
    return result


def load_policy() -> dict:
    policy = json.loads(bounded_read(POLICY_PATH, 64 << 10), object_pairs_hook=unique_object)
    require(policy["version"] == 1, "unsupported signal qualification identity version")
    require(set(policy["green_sources"]) == {MODULE, SIGNAL_TARGET, ORIGINAL_TARGET}, "unexpected reviewed source paths")
    require(set(policy["green"]) == {"signal", "process"}, "unexpected green targets")
    require(set(LATE).issubset(policy["green"]["signal"]["passed"]), "late-signal methods absent from reviewed target")
    require(len(policy["green"]["signal"]["passed"]) == 8, "signal target must retain eight active controls")
    require(len(policy["green"]["process"]["passed"]) == 14, "process target must retain fourteen active controls")
    require(policy["green"]["signal"]["ignored"] == "fixture_signal_controller", "changed signal fixture owner")
    require(policy["green"]["process"]["ignored"] == "fixture_controller", "changed original fixture owner")
    return policy


def decode_output(data: bytes) -> str:
    require(len(data) <= OUTPUT_LIMIT, "output exceeds the bounded parser limit")
    try:
        return data.decode("utf-8")
    except UnicodeDecodeError as error:
        raise Refused("non-UTF8 libtest output; raw channels retained") from error


def summary(text: str) -> tuple[str, tuple[int, ...]]:
    lines = [line for line in text.splitlines() if line.startswith("test result:")]
    require(len(lines) == 1, "expected one complete libtest summary")
    match = re.fullmatch(
        r"test result: (ok|FAILED)\. ([0-9]{1,6}) passed; ([0-9]{1,6}) failed; "
        r"([0-9]{1,6}) ignored; ([0-9]{1,6}) measured; ([0-9]{1,6}) filtered out; "
        r"finished in [0-9]+(?:\.[0-9]+)?s", lines[0],
    )
    require(match is not None, "malformed or truncated libtest summary")
    assert match is not None
    return match[1], tuple(int(value) for value in match.groups()[1:])


def registrations(text: str) -> tuple[list[str], list[str], list[str]]:
    rows = re.findall(r"^test ([A-Za-z0-9_:]+) \.\.\. (ok|FAILED|ignored)(?:, [^\n]+)?$", text, re.M)
    names = [name for name, _ in rows]
    require(len(names) == len(set(names)), "duplicate libtest method result")
    return tuple([name for name, status in rows if status == wanted]
                 for wanted in ("ok", "FAILED", "ignored"))


def running(text: str, count: int) -> None:
    rows = re.findall(r"^running ([0-9]{1,6}) tests?$", text, re.M)
    require(rows == [str(count)], "unexpected or absent libtest selection count")


def check_red(stdout: bytes, stderr: bytes, status: int) -> None:
    require(status == 101, f"expected actual Cargo status 101, got {status}")
    out, err = decode_output(stdout), decode_output(stderr)
    require(summary(out) == ("FAILED", (0, 2, 0, 0, 7)), "red must execute exactly two failures, zero pass/ignore, seven filtered")
    running(out, 2)
    passed, failed, ignored = registrations(out)
    require(not passed and set(failed) == set(LATE) and not ignored, "wrong red methods or results")
    headers = list(re.finditer(r"^thread '([^'\n]+)'(?: \([0-9]+\))? panicked at [^\n]+:\n", err, re.M))
    require([match[1] for match in headers] == list(LATE), "expected only the two serialized late-signal assertion failures")
    for index, match in enumerate(headers):
        block = err[match.end():headers[index + 1].start() if index + 1 < len(headers) else len(err)]
        case = LATE[match[1]]
        require(f"{case}: code=Some(95), signal=None; Output {{" in block, f"{case}: wrong controller exit or signal")
        channels = re.findall(r'\bstdout: ("(?:\\.|[^"\\])*"), stderr: ("(?:\\.|[^"\\])*")', block)
        require(len(channels) == 1, f"{case}: missing exact captured Output channels")
        try:
            child_stdout = json.loads(channels[0][0])
            child_stderr = json.loads(channels[0][1])
        except (ValueError, TypeError) as error:
            raise Refused(f"{case}: malformed captured Output channels") from error
        require("owned cleanup checked\n" in child_stdout, f"{case}: no owned-cleanup marker in that controller stdout")
        require(child_stderr == "", f"{case}: unexpected controller failure diagnostics")


def check_green(data: bytes, suite: str, policy: dict) -> None:
    text = decode_output(data)
    expected = policy["green"][suite]
    names = expected["passed"]
    require(summary(text) == ("ok", (len(names), 0, 1, 0, 0)), f"{suite}: wrong complete green counts")
    running(text, len(names) + 1)
    passed, failed, ignored = registrations(text)
    require(set(passed) == set(names) and len(passed) == len(names) and not failed
            and ignored == [expected["ignored"]], f"{suite}: wrong passed/ignored method identities")


def command(argv: list[str], cwd: Path) -> bytes:
    result = subprocess.run(argv, cwd=cwd, capture_output=True, check=False)
    require(result.returncode == 0, f"{argv!r} failed ({result.returncode}): {result.stderr.decode(errors='replace')}")
    require(len(result.stdout) <= SOURCE_LIMIT, f"{argv!r}: oversized metadata output")
    return result.stdout


def git(repo: Path, *args: str) -> bytes:
    return command(["git", *args], repo)


def source_facts(repo: Path, head: str, policy: dict, *, red: bool) -> dict:
    facts = {}
    for path in (*policy["green_sources"], *DYNAMIC_INPUTS):
        data = bounded_read(repo / path, SOURCE_LIMIT)
        expected = policy["red_module"] if red and path == MODULE else policy["green_sources"].get(path)
        if expected:
            require(blob(data) == expected["blob"] and sha256(data) == expected["sha256"], f"unreviewed source bytes: {path}")
        else:
            require(data == git(repo, "show", f"{head}:{path}"), f"{path}: actual-G bytes changed")
        if not red or path != MODULE:
            require(blob(data) == git(repo, "rev-parse", f"{head}:{path}").decode().strip(), f"{path}: worktree differs from actual G")
        facts[path] = {"blob": blob(data), "sha256": sha256(data), "bytes": len(data)}
    return facts


def checkout(repo: Path, head: str, tree: str, *, red: bool) -> None:
    require(git(repo, "rev-parse", "HEAD").decode().strip() == head, "worktree HEAD differs from actual G")
    require(git(repo, "rev-parse", "HEAD^{tree}").decode().strip() == tree, "worktree tree differs from actual G")
    status = git(repo, "status", "--porcelain=v1", "--untracked-files=all", "-z")
    require(status == (b" M " + MODULE.encode() + b"\0" if red else b""), "unexpected tracked/untracked source changes")


def write_receipt(path: Path, value: dict) -> None:
    pending = path.with_suffix(path.suffix + ".next")
    pending.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    pending.replace(path)


def run_recorded(argv: list[str], cwd: Path, env: dict[str, str], receipt: Path) -> int:
    """Record real process status without turning nonzero into positive proof.

    No deadline signal/exit is inferred to prove cleanup. An interrupt leaves
    the intent/start record and all owned paths for the CI owner to inspect.
    """
    record = {"argv": argv, "cwd": str(cwd), "phase": "intent_recorded"}
    write_receipt(receipt / "cargo-process.json", record)
    with (receipt / "cargo.stdout").open("xb") as stdout, (receipt / "cargo.stderr").open("xb") as stderr:
        child = subprocess.Popen(argv, cwd=cwd, env=env, stdout=stdout, stderr=stderr)
        record.update(phase="started", pid=child.pid)
        write_receipt(receipt / "cargo-process.json", record)
        status = child.wait()
    record.update(phase="completed", status=status)
    write_receipt(receipt / "cargo-process.json", record)
    return status


def red_proof(expected_sha: str, owned_root: Path, policy: dict) -> None:
    require(re.fullmatch(r"[0-9a-f]{40}", expected_sha) is not None, "expected G must be one full commit SHA")
    repo = ROOT.resolve()
    owned_root = owned_root.resolve()
    require(not owned_root.is_relative_to(repo) and not repo.is_relative_to(owned_root), "proof paths must be separate from the ordinary checkout")
    owned_root.mkdir(mode=0o700, parents=True, exist_ok=False)
    receipt = owned_root / "receipt"
    receipt.mkdir(mode=0o700)
    print(f"linux_signal_owned_proof_directory={owned_root}", flush=True)
    event = os.environ.get("GITHUB_EVENT_NAME", "local")
    report = {"kind": "checked-commit Linux signal legacy-load regression", "outcome": "incomplete",
              "ordinary_checkout": str(repo), "expected_sha": expected_sha,
              "event": event, "subject_kind": "merge-group G" if event == "merge_group" else "checked commit; no merge-group claim",
              "watchdog_is_never_evidence_of_expected_red_or_cleanup": True}
    report_path = receipt / "receipt.json"
    write_receipt(report_path, report)
    try:
        tree = git(repo, "rev-parse", "HEAD^{tree}").decode().strip()
        checkout(repo, expected_sha, tree, red=False)
        before = source_facts(repo, expected_sha, policy, red=False)
        patch = bounded_read(PATCH_PATH, 64 << 10)
        require(sha256(patch) == policy["red_patch_sha256"], "red overlay differs from reviewed bytes")
        (receipt / "legacy-load-red.patch").write_bytes(patch)
        (receipt / "reviewed-source.json").write_bytes(bounded_read(POLICY_PATH, 64 << 10))
        toolchain = {}
        for executable in ("rustc", "cargo"):
            text = command([executable, "-vV"], repo)
            require(b"release: 1.95.0\n" in text, f"{executable}: expected Rust/Cargo 1.95.0")
            (receipt / f"{executable}-vV.txt").write_bytes(text)
            toolchain[executable] = {"executable": shutil.which(executable), "identity_sha256": sha256(text)}
        report.update(actual_sha=expected_sha, actual_tree=tree, green_before=before,
                      patch_sha256=sha256(patch), toolchain=toolchain,
                      qualifier_sha256=sha256(bounded_read(Path(__file__), SOURCE_LIMIT)),
                      policy_sha256=sha256(bounded_read(POLICY_PATH, 64 << 10)))
        write_receipt(report_path, report)
        red_source, red_target = owned_root / "red-source", owned_root / "red-target"
        git(repo, "worktree", "add", "--detach", str(red_source), expected_sha)
        checkout(red_source, expected_sha, tree, red=False)
        require(source_facts(red_source, expected_sha, policy, red=False) == before, "detached actual-G source/lock differs")
        git(red_source, "apply", "--check", "--whitespace=error-all", str(receipt / "legacy-load-red.patch"))
        git(red_source, "apply", "--whitespace=error-all", str(receipt / "legacy-load-red.patch"))
        checkout(red_source, expected_sha, tree, red=True)
        red_before = source_facts(red_source, expected_sha, policy, red=True)
        sources = receipt / "sources"
        for path in policy["green_sources"]:
            target = sources / path
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(bounded_read(red_source / path, SOURCE_LIMIT))
        # These are actual-G inputs, not H987's historical lock or a new lock.
        for path in DYNAMIC_INPUTS:
            target = sources / path
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(bounded_read(red_source / path, SOURCE_LIMIT))
        changes = {"CARGO_TARGET_DIR": str(red_target), "CARGO_TERM_COLOR": "never", "RUST_BACKTRACE": "0"}
        report.update(red_before=red_before, command=RED_COMMAND, environment_overrides=changes)
        write_receipt(report_path, report)
        status = run_recorded(RED_COMMAND, red_source, {**os.environ, **changes}, receipt)
        stdout = bounded_read(receipt / "cargo.stdout", OUTPUT_LIMIT)
        stderr = bounded_read(receipt / "cargo.stderr", OUTPUT_LIMIT)
        report.update(cargo_status=status, stdout_sha256=sha256(stdout), stderr_sha256=sha256(stderr))
        write_receipt(report_path, report)
        print("--- actual legacy-load Cargo stdout ---", flush=True)
        sys.stdout.buffer.write(stdout)
        sys.stdout.buffer.flush()
        print("--- actual legacy-load Cargo stderr ---", flush=True)
        sys.stdout.buffer.write(stderr)
        sys.stdout.buffer.flush()
        checkout(red_source, expected_sha, tree, red=True)
        require(source_facts(red_source, expected_sha, policy, red=True) == red_before, "Cargo changed red source/lock bytes")
        checkout(repo, expected_sha, tree, red=False)
        require(source_facts(repo, expected_sha, policy, red=False) == before, "ordinary G source/lock changed")
        check_red(stdout, stderr, status)
        report.update(outcome="exact_expected_red", red_after=red_before, green_after=before,
                      controlled_fixture_result="two actual code95 failures with per-controller owned-cleanup markers",
                      production_owner_empty_receipt=False)
        write_receipt(report_path, report)
        # Only this completed, exact controlled fixture permits disposable file
        # cleanup. No PID/group cleanup, timeout inference or product receipt.
        git(repo, "worktree", "remove", "--force", str(red_source))
        if red_target.exists():
            shutil.rmtree(red_target)
        checkout(repo, expected_sha, tree, red=False)
        require(source_facts(repo, expected_sha, policy, red=False) == before, "ordinary G changed after owned file cleanup")
        report["disposable_files"] = "removed after exact completed controlled fixture; raw receipt retained"
        write_receipt(report_path, report)
        print(f"Exact legacy-load regression reproduced; raw receipt retained at {receipt}", flush=True)
    except BaseException as error:
        report.update(outcome="refused_or_interrupted", error=f"{type(error).__name__}: {error}",
                      disposable_files="retained; no process-absence inference")
        write_receipt(report_path, report)
        print(f"Linux signal regression refused; owned evidence retained at {owned_root}: {error}", file=sys.stderr, flush=True)
        raise


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    modes = parser.add_subparsers(dest="mode", required=True)
    red = modes.add_parser("red")
    red.add_argument("--expected-sha", required=True)
    red.add_argument("--owned-root", required=True, type=Path)
    green = modes.add_parser("green")
    green.add_argument("--suite", choices=("signal", "process"), required=True)
    args = parser.parse_args()
    try:
        policy = load_policy()
        if args.mode == "red":
            red_proof(args.expected_sha, args.owned_root, policy)
        else:
            check_green(sys.stdin.buffer.read(OUTPUT_LIMIT + 1), args.suite, policy)
            print(f"Exact {args.suite} green method/count check passed; calling guard owns process status.")
        return 0
    except (OSError, ValueError, KeyError, Refused) as error:
        print(f"Linux signal qualification failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
