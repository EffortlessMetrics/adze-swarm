#!/usr/bin/env python3
"""Exercise the real supported shell entrypoint with synthetic Cargo outcomes.

Only Cargo, formatting, and this test driver's recursive invocation are stubbed
inside an owned temporary checkout. The production shell and both existing
output guards run unchanged. These controls are not native Rust test evidence.
"""

from __future__ import annotations

import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
TARGET = "support_tier_proof_commands"
CORE = [
    "-p", "adze", "-p", "adze-macro", "-p", "adze-tool",
    "-p", "adze-common", "-p", "adze-ir", "-p", "adze-glr-core",
    "-p", "adze-tablegen",
]
SUMMARY = (
    "test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; "
    "0 filtered out; finished in 0.01s\n"
)
FAKE_CARGO = r'''#!/usr/bin/env python3
import json
import os
import sys
from pathlib import Path

argv = sys.argv[1:]
with Path(os.environ["QUALIFICATION_CALLS"]).open("a", encoding="utf-8") as log:
    log.write(json.dumps(argv) + "\n")
case = os.environ["QUALIFICATION_CASE"]
validator = "support_tier_proof_commands" in argv
summary = os.environ["QUALIFICATION_SUMMARY"]
if argv[0] == "clippy":
    if (validator and case == "validator_lint") or (not validator and case == "core_lint"):
        raise SystemExit(29)
    raise SystemExit(0)
if argv[0] != "test":
    raise SystemExit(99)
if validator:
    if case == "zero":
        summary = summary.replace("15 passed", "0 passed")
    elif case == "ignored":
        summary = summary.replace("15 passed", "0 passed").replace("0 ignored", "15 ignored")
    elif case == "partially_ignored":
        summary = summary.replace("15 passed", "14 passed").replace("0 ignored", "1 ignored")
    elif case == "too_few":
        summary = summary.replace("15 passed", "14 passed")
    elif case == "too_many":
        summary = summary.replace("15 passed", "16 passed")
    elif case == "ignored_extra":
        summary = summary.replace("0 ignored", "1 ignored")
    elif case == "measured_extra":
        summary = summary.replace("0 measured", "1 measured")
    elif case == "filtered_extra":
        summary = summary.replace("0 filtered out", "1 filtered out")
    elif case == "duplicate":
        summary += summary
    elif case == "split":
        summary = summary.replace("15 passed", "7 passed") + summary.replace("15 passed", "8 passed")
    elif case == "missing":
        summary = "running 15 tests\n"
    elif case == "malformed":
        summary = "test result: ok. not-a-count passed; 0 failed;\n"
    elif case == "truncated":
        summary = summary.rstrip("\n")
    elif case == "failed":
        summary = summary.replace("ok.", "FAILED.").replace("0 failed", "1 failed")
    sys.stdout.write(summary)
    raise SystemExit(37 if case == "child_nonzero" else 0)
sys.stdout.write(summary)
if case == "core_failure" and "--doc" not in argv:
    raise SystemExit(41)
if case == "doc_failure" and "--doc" in argv:
    raise SystemExit(43)
'''


class SupportedQualificationControls(unittest.TestCase):
    def run_case(self, case: str):
        with tempfile.TemporaryDirectory(prefix="adze qualification λ ") as temporary:
            root = Path(temporary)
            (root / "scripts/lib").mkdir(parents=True)
            for relative in (
                "scripts/ci-supported.sh",
                "scripts/lib/required-test-output.sh",
                "scripts/run-required-test.py",
            ):
                shutil.copyfile(ROOT / relative, root / relative)
            # Avoid recursively executing this same suite in the fixture. No
            # production environment switch or bypass is introduced.
            (root / "scripts/test_support_tier_qualification.py").write_text(
                "import os\nraise SystemExit(23 if os.environ['QUALIFICATION_CASE'] "
                "== 'contract_failure' else 0)\n", encoding="utf-8"
            )
            formatter = root / "scripts/fmt-workspace.sh"
            formatter.write_text("#!/usr/bin/env bash\nexit 0\n", encoding="utf-8")
            formatter.chmod(0o755)
            binary_dir = root / "bin"
            binary_dir.mkdir()
            cargo = binary_dir / "cargo"
            cargo.write_text(FAKE_CARGO, encoding="utf-8")
            cargo.chmod(0o755)
            calls = root / "cargo-calls.jsonl"
            result = subprocess.run(
                ["bash", "scripts/ci-supported.sh"], cwd=root,
                env={
                    **os.environ,
                    "PATH": str(binary_dir) + os.pathsep + os.environ.get("PATH", ""),
                    "RUST_TEST_THREADS": "2",
                    "QUALIFICATION_CASE": case,
                    "QUALIFICATION_SUMMARY": SUMMARY,
                    "QUALIFICATION_CALLS": str(calls),
                },
                stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=20, check=False,
            )
            recorded = [json.loads(line) for line in calls.read_text().splitlines()] if calls.exists() else []
            return result, recorded

    def test_complete_target_runs_once_and_original_core_commands_survive(self):
        result, calls = self.run_case("success")
        self.assertEqual(result.returncode, 0, result.stdout.decode(errors="replace"))
        self.assertEqual(calls, [
            ["clippy", *CORE, "--all-targets", "--", "-D", "warnings"],
            ["clippy", "--locked", "-p", "xtask", "--lib", "--test", TARGET, "--", "-D", "warnings"],
            ["test", "--locked", "-p", "xtask", "--test", TARGET, "--", "--nocapture"],
            ["test", *CORE, "--lib", "--tests", "--bins", "--", "--test-threads=2", "--quiet"],
            ["test", "-p", "adze-glr-core", "--features", "serialization", "--doc", "--", "--test-threads=2", "--quiet"],
        ])
        self.assertIn(b"support-tier validator regressions (static screen only)", result.stdout)
        self.assertIn(b"15 passed tests in 1 complete summaries", result.stdout)

    def test_validator_natural_failure_cannot_borrow_a_passing_summary(self):
        result, calls = self.run_case("child_nonzero")
        self.assertEqual(result.returncode, 37, result.stdout.decode(errors="replace"))
        self.assertEqual(calls[-1], ["test", "--locked", "-p", "xtask", "--test", TARGET, "--", "--nocapture"])
        self.assertEqual(len(calls), 3)

    def test_validator_empty_ignored_missing_malformed_truncated_and_failed_refuse(self):
        for case in ("zero", "ignored", "missing", "malformed", "truncated", "failed"):
            with self.subTest(case=case):
                result, calls = self.run_case(case)
                self.assertNotEqual(result.returncode, 0, result.stdout.decode(errors="replace"))
                self.assertEqual(len(calls), 3)
                self.assertIn(TARGET, calls[-1])

    def test_validator_requires_one_complete_fifteen_method_suite(self):
        for case in (
            "partially_ignored", "too_few", "too_many", "ignored_extra",
            "measured_extra", "filtered_extra", "duplicate", "split",
        ):
            with self.subTest(case=case):
                result, calls = self.run_case(case)
                self.assertEqual(result.returncode, 1, result.stdout.decode(errors="replace"))
                self.assertEqual(len(calls), 3)
                self.assertIn(TARGET, calls[-1])

    def test_strict_validator_lint_failure_stops_before_execution(self):
        result, calls = self.run_case("validator_lint")
        self.assertEqual(result.returncode, 29)
        self.assertEqual(len(calls), 2)
        self.assertEqual(calls[-1][0], "clippy")
        self.assertIn(TARGET, calls[-1])

    def test_contract_controls_cannot_fail_open(self):
        result, calls = self.run_case("contract_failure")
        self.assertEqual(result.returncode, 23)
        self.assertEqual(len(calls), 1)

    def test_original_core_and_doctest_failures_still_propagate(self):
        for case, status, count in (("core_lint", 29, 1), ("core_failure", 41, 4), ("doc_failure", 43, 5)):
            with self.subTest(case=case):
                result, calls = self.run_case(case)
                self.assertEqual(result.returncode, status, result.stdout.decode(errors="replace"))
                self.assertEqual(len(calls), count)


if __name__ == "__main__":
    unittest.main(verbosity=2)
