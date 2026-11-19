#!/usr/bin/env python3
"""Synthetic parser/process and real-Git isolation controls; not Rust proof."""
from __future__ import annotations

import importlib.util
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import textwrap
import unittest

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parent.parent
SPEC = importlib.util.spec_from_file_location("signal_regression", ROOT / "scripts/check-linux-signal-regression.py")
assert SPEC is not None and SPEC.loader is not None
PROOF = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(PROOF)
POLICY = PROOF.load_policy()


def synthetic_red() -> tuple[bytes, bytes]:
    out = "running 2 tests\n"
    err = ""
    for name, case in PROOF.LATE.items():
        out += f"test {name} ... FAILED\n"
        child = json.dumps("running 1 test\ntest fixture_signal_controller ... owned cleanup checked\n")
        err += (
            f"\nthread '{name}' (123) panicked at cli/tests/runner_linux_signal_contract.rs:46:5:\n"
            f"{case}: code=Some(95), signal=None; Output {{ status: ExitStatus(unix_wait_status(24320)), "
            f"stdout: {child}, stderr: \"\" }}\n"
        )
    out += "\nfailures:\n" + "".join(f"    {name}\n" for name in PROOF.LATE)
    out += "\ntest result: FAILED. 0 passed; 2 failed; 0 ignored; 0 measured; 7 filtered out; finished in 0.42s\n"
    err += "error: test failed, to rerun pass `-p adze-cli --test runner_linux_signal_contract`\n"
    return out.encode(), err.encode()


def synthetic_green(suite: str) -> bytes:
    expected = POLICY["green"][suite]
    out = f"running {len(expected['passed']) + 1} tests\n"
    out += f"test {expected['ignored']} ... ignored, explicitly selected subprocess owner\n"
    out += "".join(f"test {name} ... ok\n" for name in expected["passed"])
    out += f"\ntest result: ok. {len(expected['passed'])} passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.42s\n"
    return out.encode()


class SyntheticOutputControls(unittest.TestCase):
    def test_exact_synthetic_red_is_distinct_from_success(self) -> None:
        stdout, stderr = synthetic_red()
        PROOF.check_red(stdout, stderr, 101)
        for status in (0, 1, 37, 95, 124, 130, -9, -15):
            with self.subTest(status=status), self.assertRaises(PROOF.Refused):
                PROOF.check_red(stdout, stderr, status)

    def test_compile_empty_truncated_and_zero_selection_are_not_red(self) -> None:
        stdout, stderr = synthetic_red()
        cases = [
            (b"", b"error[E0001]: failed to compile"),
            (b"", b""),
            (stdout[:-20], stderr),
            (stdout.replace(b"0 passed; 2 failed", b"0 passed; 0 failed"), stderr),
            (stdout.replace(b"running 2 tests", b"running 0 tests"), stderr),
            (stdout.replace(b"7 filtered out", b"8 filtered out"), stderr),
            (stdout.replace(b"0 ignored", b"2 ignored"), stderr),
            (stdout.replace(b"0 passed", b"2 passed"), stderr),
            (stdout + stdout[stdout.index(b"test result:"):], stderr),
            (stdout.replace(b"0 passed", b"9999999 passed"), stderr),
        ]
        for out, err in cases:
            with self.subTest(out=out[-120:]), self.assertRaises(PROOF.Refused):
                PROOF.check_red(out, err, 101)

    def test_wrong_or_duplicate_method_failures_are_not_red(self) -> None:
        stdout, stderr = synthetic_red()
        first, second = [name.encode() for name in PROOF.LATE]
        for out, err in [
            (stdout.replace(first, b"other_test"), stderr),
            (stdout.replace(first, second), stderr),
            (stdout, stderr.replace(first, b"other_test")),
            (stdout, stderr + "\nthread 'unrelated' panicked at other.rs:1:1:\nunrelated failure\n".encode()),
        ]:
            with self.subTest(out=out[:100]), self.assertRaises(PROOF.Refused):
                PROOF.check_red(out, err, 101)

    def test_each_controller_must_supply_its_own_status_and_stdout_marker(self) -> None:
        stdout, stderr = synthetic_red()
        cases = [
            stderr.replace(b"code=Some(95)", b"code=Some(96)", 1),
            stderr.replace(b"signal=None", b"signal=Some(15)", 1),
            stderr.replace(b"owned cleanup checked", b"cleanup missing", 1),
            stderr.replace(b"owned cleanup checked", b"cleanup missing", 1) + b"owned cleanup checked\n",
            stderr.replace(b'stderr: ""', b'stderr: "owned cleanup checked\\n"', 1),
            stderr.replace(b"stdout:", b"unrelated:", 1),
            stderr.replace(b"owned cleanup checked\\n", b"owned cleanup checked", 1),
            stderr.replace(b"late_int:", b"wrong_case:", 1),
        ]
        for err in cases:
            with self.subTest(err=err[:160]), self.assertRaises(PROOF.Refused):
                PROOF.check_red(stdout, err, 101)

    def test_parser_bounds_and_encoding_fail_closed(self) -> None:
        stdout, stderr = synthetic_red()
        for out, err in [(stdout + b"x" * PROOF.OUTPUT_LIMIT, stderr), (stdout, stderr + b"\xff")]:
            with self.assertRaises(PROOF.Refused):
                PROOF.check_red(out, err, 101)
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "raw.log"
            path.write_bytes(b"12345")
            with self.assertRaises(PROOF.Refused):
                PROOF.bounded_read(path, 4)
            self.assertEqual(b"12345", path.read_bytes())
            self.assertEqual(b"12345", PROOF.bounded_read(path, 5))

    def test_exact_green_methods_and_explicit_ignored_owner(self) -> None:
        for suite in ("signal", "process"):
            output = synthetic_green(suite)
            PROOF.check_green(output, suite, POLICY)
            for bad in [
                b"",
                output.replace(b"0 failed", b"1 failed"),
                output.replace(b"1 ignored", b"0 ignored"),
                output.replace(b"0 filtered out", b"1 filtered out"),
                output.replace(POLICY["green"][suite]["ignored"].encode(), b"different_fixture"),
                output.replace(POLICY["green"][suite]["passed"][0].encode(), b"different_test"),
                output.replace(b"... ok", b"... ignored"),
            ]:
                with self.subTest(suite=suite), self.assertRaises(PROOF.Refused):
                    PROOF.check_green(bad, suite, POLICY)


class SyntheticProcessControls(unittest.TestCase):
    def test_real_python_subprocess_preserves_argv_channels_and_nonzero_status(self) -> None:
        # A real Python child, explicitly synthetic: it cannot qualify Rust.
        args = ["space value", "λ", "$(touch must-not-exist)", "`literal`", "quote'\""]
        program = (
            "import json,pathlib,sys; "
            "pathlib.Path('argv.json').write_text(json.dumps(sys.argv[1:])); "
            "sys.stdout.buffer.write(b'raw stdout\\x00\\xff'); "
            "sys.stderr.buffer.write(b'raw stderr\\n'); raise SystemExit(37)"
        )
        with tempfile.TemporaryDirectory(prefix="signal synthetic λ ") as directory:
            root = Path(directory)
            receipt = root / "receipt"
            receipt.mkdir()
            status = PROOF.run_recorded([sys.executable, "-c", program, *args], root, dict(os.environ), receipt)
            self.assertEqual(37, status)
            self.assertEqual(args, json.loads((root / "argv.json").read_text()))
            self.assertFalse((root / "must-not-exist").exists())
            self.assertEqual(b"raw stdout\x00\xff", (receipt / "cargo.stdout").read_bytes())
            self.assertEqual(b"raw stderr\n", (receipt / "cargo.stderr").read_bytes())
            record = json.loads((receipt / "cargo-process.json").read_text())
            self.assertEqual("completed", record["phase"])
            self.assertEqual(37, record["status"])
            self.assertEqual([sys.executable, "-c", program, *args], record["argv"])

    def test_failed_creation_leaves_intent_without_a_completed_status(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            with self.assertRaises(OSError):
                PROOF.run_recorded([str(root / "missing program")], root, dict(os.environ), root)
            record = json.loads((root / "cargo-process.json").read_text())
            self.assertEqual("intent_recorded", record["phase"])
            self.assertNotIn("status", record)


class RealGitIsolationControls(unittest.TestCase):
    def _fixture(self, root: Path) -> tuple[Path, str, str, dict]:
        repo = root / "ordinary source λ"
        repo.mkdir()
        PROOF.git(repo, "init", "--quiet")
        policy = {"green_sources": {}}
        for index, path in enumerate((PROOF.MODULE, PROOF.SIGNAL_TARGET, PROOF.ORIGINAL_TARGET)):
            data = f"synthetic source {index}\n".encode()
            target = repo / path
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(data)
            policy["green_sources"][path] = {"blob": PROOF.blob(data), "sha256": PROOF.sha256(data)}
        for path in PROOF.DYNAMIC_INPUTS:
            target = repo / path
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_text(f"synthetic actual-G {path}\n")
        PROOF.git(repo, "add", "-A")
        PROOF.git(repo, "-c", "user.name=Signal fixture", "-c", "user.email=fixture@example.invalid", "commit", "--quiet", "-m", "synthetic actual G")
        head = PROOF.git(repo, "rev-parse", "HEAD").decode().strip()
        tree = PROOF.git(repo, "rev-parse", "HEAD^{tree}").decode().strip()
        return repo, head, tree, policy

    def test_detached_worktree_is_exact_and_only_reviewed_mutation_is_allowed(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            repo, head, tree, policy = self._fixture(root)
            PROOF.checkout(repo, head, tree, red=False)
            before = PROOF.source_facts(repo, head, policy, red=False)
            red = root / "red source"
            PROOF.git(repo, "worktree", "add", "--detach", str(red), head)
            PROOF.checkout(red, head, tree, red=False)
            self.assertEqual(before, PROOF.source_facts(red, head, policy, red=False))
            changed = b"synthetic legacy mutant\n"
            (red / PROOF.MODULE).write_bytes(changed)
            policy["red_module"] = {"blob": PROOF.blob(changed), "sha256": PROOF.sha256(changed)}
            PROOF.checkout(red, head, tree, red=True)
            PROOF.source_facts(red, head, policy, red=True)
            self.assertEqual(before, PROOF.source_facts(repo, head, policy, red=False))
            (red / "unrelated-source.rs").write_text("unexpected\n")
            with self.assertRaises(PROOF.Refused):
                PROOF.checkout(red, head, tree, red=True)
            PROOF.git(repo, "worktree", "remove", "--force", str(red))
            PROOF.checkout(repo, head, tree, red=False)

    def test_lock_is_bound_to_each_actual_commit_and_post_checkout_changes_refuse(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            repo, head, tree, policy = self._fixture(Path(directory))
            first = PROOF.source_facts(repo, head, policy, red=False)
            (repo / "Cargo.lock").write_bytes(b"legitimate composed-G lock\n")
            with self.assertRaises(PROOF.Refused):
                PROOF.source_facts(repo, head, policy, red=False)
            PROOF.git(repo, "add", "Cargo.lock")
            PROOF.git(repo, "-c", "user.name=Signal fixture", "-c", "user.email=fixture@example.invalid", "commit", "--quiet", "-m", "composed actual G")
            new_head = PROOF.git(repo, "rev-parse", "HEAD").decode().strip()
            new_tree = PROOF.git(repo, "rev-parse", "HEAD^{tree}").decode().strip()
            PROOF.checkout(repo, new_head, new_tree, red=False)
            second = PROOF.source_facts(repo, new_head, policy, red=False)
            self.assertNotEqual(first["Cargo.lock"], second["Cargo.lock"])
            with self.assertRaises(PROOF.Refused):
                PROOF.checkout(repo, head, tree, red=False)
            (repo / PROOF.SIGNAL_TARGET).write_text("changed oracle\n")
            with self.assertRaises(PROOF.Refused):
                PROOF.source_facts(repo, new_head, policy, red=False)


class WorkflowControls(unittest.TestCase):
    def _linux(self) -> str:
        text = (ROOT / ".github/workflows/product-proof.yml").read_text()
        return text.split("  ci-product-stable:\n", 1)[1].split("\n  ci-product-windows:", 1)[0]

    def test_exact_command_order_pins_and_receipt_retention(self) -> None:
        block = self._linux()
        self.assertIn("toolchain: 1.95.0", block)
        self.assertIn("run: python3 scripts/test_linux_signal_regression.py", block)
        self.assertIn(
            "cargo clippy --locked -p adze-cli --bin adze "
            "--test runner_process_contract --test runner_output_contract "
            "--test runner_protocol_contract --test runner_cache_process_contract "
            "--test runner_linux_signal_contract -- -D warnings",
            block,
        )
        red = 'python3 scripts/check-linux-signal-regression.py red --expected-sha "$EXPECTED_SHA" --owned-root "$OWNED_ROOT"'
        green = "run_contract 'Linux signal finalization' cargo test --locked -p adze-cli --test runner_linux_signal_contract -- --nocapture --test-threads=1"
        self.assertIn(red, block)
        self.assertIn(green, block)
        self.assertLess(block.index("Prepare locked native dependencies online"), block.index(red))
        self.assertLess(block.index(red), block.index(green))
        self.assertIn("EXPECTED_SHA: ${{ github.sha }}", block)
        self.assertIn("      - name: Retain Linux signal regression receipt\n        if: always()", block)
        self.assertIn("uses: actions/upload-artifact@v7.0.0", block)
        self.assertIn("if-no-files-found: error", block)
        self.assertNotIn("continue-on-error", block)
        self.assertEqual([
            "cargo", "test", "--locked", "--offline", "-p", "adze-cli", "--test",
            "runner_linux_signal_contract", "test_late_dispatched_", "--", "--nocapture", "--test-threads=1",
        ], PROOF.RED_COMMAND)

    def test_actual_workflow_guard_preserves_status_and_requires_exact_green_counts(self) -> None:
        block = self._linux()
        start = block.index("          run_contract() {")
        end = block.index("\n          }", start) + len("\n          }")
        function = textwrap.dedent(block[start:end])
        harness = (
            "set -euo pipefail\nsource scripts/lib/required-test-output.sh\n" + function +
            '\nrun_contract "$SYNTHETIC_LABEL" "$SYNTHETIC_PYTHON" "$SYNTHETIC_CHILD"\n'
        )
        child_source = (
            "import os,sys\n"
            "sys.stdout.buffer.write(bytes.fromhex(os.environ['SYNTHETIC_OUTPUT']))\n"
            "print('synthetic child stderr',file=sys.stderr)\n"
            "raise SystemExit(int(os.environ['SYNTHETIC_STATUS']))\n"
        )
        cases = [
            ("signal", synthetic_green("signal"), 0, 0),
            ("process", synthetic_green("process"), 0, 0),
            ("signal", synthetic_green("signal"), 37, 37),
            ("signal", synthetic_green("signal"), 101, 101),
            ("signal", b"", 0, 1),
            ("signal", synthetic_green("signal").replace(b"8 passed", b"0 passed"), 0, 1),
            ("signal", synthetic_green("signal").replace(b"1 ignored", b"9 ignored"), 0, 1),
            ("process", synthetic_green("process").replace(b"14 passed", b"13 passed"), 0, 1),
        ]
        with tempfile.TemporaryDirectory(prefix="signal guard λ ") as directory:
            child = Path(directory) / "synthetic child.py"
            child.write_text(child_source)
            for suite, output, status, expected in cases:
                with self.subTest(suite=suite, status=status, expected=expected):
                    result = subprocess.run(
                        ["bash", "-c", harness], cwd=ROOT,
                        env={**os.environ, "SYNTHETIC_LABEL": "Linux signal finalization" if suite == "signal" else "Linux process custody",
                             "SYNTHETIC_PYTHON": sys.executable, "SYNTHETIC_CHILD": str(child),
                             "SYNTHETIC_OUTPUT": output.hex(), "SYNTHETIC_STATUS": str(status)},
                        capture_output=True, timeout=10,
                    )
                    self.assertEqual(expected, result.returncode, result.stdout + result.stderr)
                    self.assertIn(b"synthetic child stderr", result.stdout)


if __name__ == "__main__":
    unittest.main(verbosity=2)
