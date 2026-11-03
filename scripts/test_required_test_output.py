#!/usr/bin/env python3
"""Execute the required scripts with real libtest selections (#995).

Requires bash and rustc. Only Cargo's dispatch and the unrelated formatting /
clippy prerequisites are replaced; both gate scripts and their shared guard
are copied unchanged. The producer is a compiled Rust test harness, not text
that imitates a test summary. Run: python3 scripts/test_required_test_output.py
"""
from __future__ import annotations

import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parent.parent
SCRIPTS = ("ci-supported.sh", "ci-product-stable.sh")


class RequiredScriptProcesses(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.temp = tempfile.TemporaryDirectory(prefix="adze-required-test-output-")
        cls.root = Path(cls.temp.name)
        scripts = cls.root / "scripts"
        (scripts / "lib").mkdir(parents=True)
        (cls.root / "bin").mkdir()
        for script in (*SCRIPTS, "lib/required-test-output.sh"):
            shutil.copy2(ROOT / "scripts" / script, scripts / script)
        (scripts / "fmt-workspace.sh").write_text("#!/bin/sh\nexit 0\n")
        (scripts / "fmt-workspace.sh").chmod(0o755)
        harness = cls.root / "harness.rs"
        harness.write_text('''#[test] fn test_pass_case() {}
#[test] #[ignore] fn test_ignored_case() {}
#[test] fn test_fail_case() { panic!("deliberate process failure"); }
''')
        program = cls.root / "program.rs"
        program.write_text('''fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("stderr") => eprintln!("diagnostic without stdout"),
        Some("whitespace") => print!(" \\n\\t"),
        Some("exit23") => std::process::exit(23),
        _ => println!("downstream value: 17"),
    }
}
''')
        rustc = os.environ.get("RUSTC", "rustc")
        for source, target, args in [(harness, "harness", ["--test"]), (program, "program", [])]:
            subprocess.run([rustc, *args, str(source), "-o", str(cls.root / target)], check=True,
                           capture_output=True, text=True)
        cargo = cls.root / "bin" / "cargo"
        cargo.write_text('''#!/usr/bin/env bash
set -u
case "$1" in
  clippy) exit 0 ;;
  run) exec "$ADZE_GUARD_ROOT/program" "$ADZE_GUARD_CASE" ;;
  test)
    case "$ADZE_GUARD_CASE" in
      zero) exec "$ADZE_GUARD_ROOT/harness" --exact absent ;;
      ignored) exec "$ADZE_GUARD_ROOT/harness" --exact test_ignored_case ;;
      ignored_doc)
        for arg in "$@"; do
          if [[ "$arg" == --doc ]]; then
            exec "$ADZE_GUARD_ROOT/harness" --exact test_ignored_case
          fi
        done ;;
      fail) exec "$ADZE_GUARD_ROOT/harness" --exact test_fail_case ;;
      exit23) exec "$ADZE_GUARD_ROOT/program" exit23 ;;
      missing_summary) exec "$ADZE_GUARD_ROOT/program" stdout ;;
      mixed)
        "$ADZE_GUARD_ROOT/harness" --exact test_pass_case &&
          exec "$ADZE_GUARD_ROOT/harness" --exact absent ;;
      aggregate_fail)
        "$ADZE_GUARD_ROOT/harness" --exact test_pass_case || exit "$?"
        exec "$ADZE_GUARD_ROOT/harness" --exact test_fail_case ;;
    esac
    exec "$ADZE_GUARD_ROOT/harness" --exact test_pass_case ;;
  *) exit 99 ;;
esac
''')
        cargo.chmod(0o755)

    @classmethod
    def tearDownClass(cls) -> None:
        cls.temp.cleanup()

    def run_script(self, script: str, selection: str) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            ["bash", str(self.root / "scripts" / script)], cwd=self.root,
            env={**os.environ, "PATH": str(self.root / "bin") + os.pathsep + os.environ["PATH"],
                 "ADZE_GUARD_ROOT": str(self.root), "ADZE_GUARD_CASE": selection},
            capture_output=True, text=True, timeout=30,
        )

    def test_all_ignored_selection_rejected(self) -> None:
        for script in SCRIPTS:
            with self.subTest(script=script):
                result = self.run_script(script, "ignored")
                self.assertIn("0 passed; 0 failed; 1 ignored", result.stdout)
                self.assertNotEqual(0, result.returncode, result.stdout + result.stderr)

    def test_all_ignored_doctest_selection_rejected(self) -> None:
        for script in SCRIPTS:
            with self.subTest(script=script):
                result = self.run_script(script, "ignored_doc")
                self.assertIn("1 passed; 0 failed", result.stdout)
                self.assertIn("0 passed; 0 failed; 1 ignored", result.stdout)
                self.assertNotEqual(0, result.returncode, result.stdout + result.stderr)

    def test_zero_selection_and_missing_summary_rejected(self) -> None:
        for script in SCRIPTS:
            for selection in ("zero", "missing_summary"):
                with self.subTest(script=script, selection=selection):
                    result = self.run_script(script, selection)
                    self.assertNotEqual(0, result.returncode, result.stdout + result.stderr)

    def test_executed_pass_and_auxiliary_zero_targets_succeed(self) -> None:
        for script in SCRIPTS:
            for selection in ("pass", "mixed"):
                with self.subTest(script=script, selection=selection):
                    result = self.run_script(script, selection)
                    self.assertEqual(0, result.returncode, result.stdout + result.stderr)
                    self.assertIn("1 passed; 0 failed", result.stdout)
                    if selection == "mixed":
                        self.assertIn("0 passed; 0 failed", result.stdout)

    def test_failed_process_and_failed_aggregate_propagate(self) -> None:
        for script in SCRIPTS:
            for selection, status in (("fail", 101), ("aggregate_fail", 101), ("exit23", 23)):
                with self.subTest(script=script, selection=selection):
                    result = self.run_script(script, selection)
                    self.assertEqual(status if script == "ci-supported.sh" else 1, result.returncode,
                                     result.stdout + result.stderr)
                    if selection == "aggregate_fail":
                        self.assertIn("1 passed; 0 failed", result.stdout)
                        self.assertIn("0 passed; 1 failed", result.stdout)

    def test_binary_requires_non_whitespace_stdout(self) -> None:
        for selection in ("stderr", "whitespace"):
            with self.subTest(selection=selection):
                result = self.run_script("ci-product-stable.sh", selection)
                self.assertNotEqual(0, result.returncode, result.stdout + result.stderr)
                self.assertIn("FAIL (empty stdout)", result.stdout)
                if selection == "stderr":
                    self.assertIn("diagnostic without stdout", result.stderr)
        result = self.run_script("ci-product-stable.sh", "pass")
        self.assertEqual(0, result.returncode, result.stdout + result.stderr)
        self.assertIn("downstream value: 17", result.stdout)


if __name__ == "__main__":
    unittest.main(verbosity=2)
