#!/usr/bin/env python3
"""Process-level controls for the portable required-test guard (#1066).

The default suite includes a real rustc --test harness and fails if rustc is
missing. SyntheticProcessControls deliberately use Python subprocesses; they
qualify process/summary handling and are not native Rust proof. An environment
without Rust may run only that labelled class:

  python scripts/test_run_required_test.py SyntheticProcessControls
"""

from __future__ import annotations

import json
import os
import shutil
import subprocess
import sys
import tempfile
import threading
import unittest
from pathlib import Path

WRAPPER = Path(__file__).with_name("run-required-test.py")
PASS = (
    b"test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; "
    b"0 filtered out; finished in 0.00s\n"
)


def guarded(command: list[str], *, label: str = "synthetic process control") -> subprocess.CompletedProcess:
    return subprocess.run(
        [sys.executable, str(WRAPPER), "--label", label, "--", *command],
        stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
        env={**os.environ, "CARGO_TERM_COLOR": "always"},
        timeout=30, check=False,
    )


def synthetic(stdout: bytes, *, stderr: bytes = b"", status: int = 0) -> subprocess.CompletedProcess:
    code = (
        "import sys; "
        f"sys.stdout.buffer.write({stdout!r}); sys.stdout.buffer.flush(); "
        f"sys.stderr.buffer.write({stderr!r}); sys.stderr.buffer.flush(); "
        f"sys.exit({status})"
    )
    return guarded([sys.executable, "-c", code])


class SyntheticProcessControls(unittest.TestCase):
    def test_complete_success_and_crlf_summaries_pass(self) -> None:
        for summary in (PASS, PASS.replace(b"\n", b"\r\n")):
            with self.subTest(summary=summary):
                result = synthetic(summary)
                self.assertEqual(0, result.returncode, result.stdout)
                self.assertIn(summary, result.stdout)

    def test_zero_ignored_measured_and_filtered_rows_are_not_executed_passes(self) -> None:
        summaries = (
            PASS.replace(b"1 passed", b"0 passed"),
            PASS.replace(b"1 passed", b"0 passed").replace(b"0 ignored", b"1 ignored"),
            PASS.replace(b"1 passed", b"0 passed").replace(b"0 measured", b"1 measured"),
            PASS.replace(b"1 passed", b"0 passed").replace(b"0 filtered out", b"9 filtered out"),
        )
        for summary in summaries:
            with self.subTest(summary=summary):
                result = synthetic(summary)
                self.assertEqual(1, result.returncode, result.stdout)
                self.assertIn(b"no passing tests executed", result.stdout)

    def test_failed_marker_and_nonzero_failed_count_are_rejected(self) -> None:
        for output in (
            PASS.replace(b"ok.", b"FAILED."),
            PASS.replace(b"0 failed", b"1 failed"),
            b"test actual_control ... FAILED\n" + PASS,
        ):
            with self.subTest(output=output):
                result = synthetic(output)
                self.assertEqual(1, result.returncode, result.stdout)
                self.assertIn(output, result.stdout)

    def test_missing_malformed_and_truncated_summaries_are_rejected(self) -> None:
        for output in (
            b"ordinary child output\n",
            b"test result\n",
            PASS.replace(b"1 passed", b"one passed"),
            PASS.replace(b"; 0 measured", b"; unknown measured"),
            b"prefix " + PASS,
            PASS.replace(b"0.00s\n", b"0.00s unexpected\n"),
            PASS.rstrip(b"\n"),
            PASS[:70],
            PASS + PASS[:70],
        ):
            with self.subTest(output=output):
                result = synthetic(output)
                self.assertEqual(1, result.returncode, result.stdout)

    def test_child_status_37_is_preserved_even_with_a_passing_summary(self) -> None:
        result = synthetic(PASS, stderr=b"stderr before status 37\n", status=37)
        self.assertEqual(37, result.returncode, result.stdout)
        self.assertIn(PASS, result.stdout)
        self.assertIn(b"stderr before status 37\n", result.stdout)
        self.assertIn(b"child exited with status 37", result.stdout)

    def test_unparseable_count_does_not_mask_child_status_or_output(self) -> None:
        summary = PASS.replace(b"1 passed", b"9" * 641 + b" passed")
        for child_status, expected in ((0, 1), (37, 37)):
            with self.subTest(child_status=child_status):
                code = f"import sys; sys.stdout.buffer.write({summary!r}); sys.exit({child_status})"
                result = subprocess.run(
                    [sys.executable, str(WRAPPER), "--label", "synthetic oversized count", "--",
                     sys.executable, "-c", code],
                    stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=30, check=False,
                    env={**os.environ, "PYTHONINTMAXSTRDIGITS": "640"},
                )
                self.assertEqual(expected, result.returncode, result.stdout)
                self.assertIn(summary, result.stdout)
                self.assertNotIn(b"Traceback", result.stdout)

    def test_multiple_summaries_require_real_passes_and_reject_any_failure(self) -> None:
        zero = PASS.replace(b"1 passed", b"0 passed")
        self.assertEqual(0, synthetic(zero + PASS).returncode)
        self.assertEqual(1, synthetic(zero + zero).returncode)
        self.assertEqual(1, synthetic(PASS + PASS.replace(b"0 failed", b"1 failed")).returncode)

    def test_argv_spaces_unicode_empty_and_shell_characters_are_literal(self) -> None:
        with tempfile.TemporaryDirectory(prefix="adze guard paths π ") as directory:
            helper = Path(directory) / "argv helper λ.py"
            helper.write_text(
                "import json, os, sys\n"
                "row = json.dumps(sys.argv[1:], ensure_ascii=False).encode('utf-8')\n"
                "sys.stdout.buffer.write(b'ACTUAL_ARGS=' + row + b'\\n')\n"
                "sys.stdout.buffer.write(('COLOR=' + os.environ['CARGO_TERM_COLOR'] + '\\n').encode())\n"
                f"sys.stdout.buffer.write({PASS!r})\n",
                encoding="utf-8",
            )
            arguments = ["two words", "snowman ☃", "", "$(not-a-shell)", "semi;literal", 'quote"', "tail\\"]
            result = guarded([sys.executable, str(helper), *arguments])
            self.assertEqual(0, result.returncode, result.stdout)
            actual = next(line for line in result.stdout.splitlines() if line.startswith(b"ACTUAL_ARGS="))
            self.assertEqual(arguments, json.loads(actual.removeprefix(b"ACTUAL_ARGS=").decode("utf-8")))
            self.assertIn(b"COLOR=never\n", result.stdout)

    def test_large_merged_stdout_stderr_bytes_are_retained(self) -> None:
        stdout = ("stdout π\n".encode("utf-8") * 20000) + b"non-utf8=\xff\n"
        stderr = ("stderr 雪\n".encode("utf-8") * 20000) + PASS
        # A file avoids command-line length limits on native Windows.
        with tempfile.TemporaryDirectory(prefix="adze guard retention ") as directory:
            helper = Path(directory) / "retention.py"
            helper.write_text(
                "import sys\n"
                f"sys.stdout.buffer.write({stdout!r}); sys.stdout.buffer.flush()\n"
                f"sys.stderr.buffer.write({stderr!r}); sys.stderr.buffer.flush()\n",
                encoding="utf-8",
            )
            result = guarded([sys.executable, str(helper)])
        self.assertEqual(0, result.returncode, result.stdout[-1000:])
        self.assertIn(stdout + stderr, result.stdout)

    def test_partial_line_is_streamed_before_the_child_exits(self) -> None:
        with tempfile.TemporaryDirectory(prefix="adze guard streaming ") as directory:
            release = Path(directory) / "release"
            helper = Path(directory) / "streaming.py"
            prefix = b"ACTUAL_CHILD_PARTIAL_LINE_WITHOUT_NEWLINE"
            helper.write_text(
                "import pathlib, sys, time\n"
                f"sys.stdout.buffer.write({prefix!r}); sys.stdout.buffer.flush()\n"
                "release = pathlib.Path(sys.argv[1])\n"
                "deadline = time.monotonic() + 15\n"
                "while not release.exists():\n"
                "    if time.monotonic() > deadline: sys.exit(29)\n"
                "    time.sleep(0.01)\n"
                f"sys.stdout.buffer.write(b'\\n' + {PASS!r}); sys.stdout.buffer.flush()\n",
                encoding="utf-8",
            )
            with subprocess.Popen(
                [sys.executable, str(WRAPPER), "--label", "synthetic streaming", "--",
                 sys.executable, str(helper), str(release)],
                stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
            ) as process:
                self.assertIsNotNone(process.stdout)
                assert process.stdout is not None
                chunks: list[bytes] = []
                observed = threading.Event()

                def read_output() -> None:
                    seen = b""
                    while chunk := process.stdout.read1(4096):
                        chunks.append(chunk)
                        seen = (seen + chunk)[-4096:]
                        if prefix in seen:
                            observed.set()

                reader = threading.Thread(target=read_output, daemon=True)
                reader.start()
                try:
                    self.assertTrue(observed.wait(10), "partial child output was not streamed")
                    self.assertIsNone(process.poll(), "the child must still await the release file")
                finally:
                    release.touch()
                    status = process.wait(timeout=20)
                    reader.join(timeout=10)
                self.assertFalse(reader.is_alive())
                self.assertEqual(0, status, b"".join(chunks))
                self.assertIn(prefix + b"\n" + PASS, b"".join(chunks))

    def test_missing_child_command_fails_explicitly(self) -> None:
        with tempfile.TemporaryDirectory(prefix="adze missing child ") as directory:
            result = guarded([str(Path(directory) / "does-not-exist")])
        self.assertEqual(2, result.returncode, result.stdout)
        self.assertIn(b"could not run child", result.stdout)


class NativeRustHarnessControls(unittest.TestCase):
    """Use the installed native compiler; absence is a failure, never a skip."""

    @classmethod
    def setUpClass(cls) -> None:
        rustc = shutil.which("rustc")
        if rustc is None:
            raise RuntimeError("Native guard self-test requires rustc; native controls must not be skipped")
        cls.directory = tempfile.TemporaryDirectory(prefix="adze native guard π ")
        cls.addClassCleanup(cls.directory.cleanup)
        root = Path(cls.directory.name)
        source = root / "native harness.rs"
        cls.binary = root / ("native harness.exe" if os.name == "nt" else "native harness")
        source.write_text(
            '#[test]\nfn positive() { println!("native stdout π"); eprintln!("native stderr 雪"); }\n'
            '#[test]\n#[ignore]\nfn ignored() {}\n'
            '#[test]\nfn failure() { panic!("intentional native guard failure"); }\n',
            encoding="utf-8",
        )
        compiled = subprocess.run(
            [rustc, "--edition=2024", "--crate-name", "required_guard_harness", "--test",
             str(source), "-o", str(cls.binary)],
            stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=120, check=False,
        )
        if compiled.returncode != 0:
            raise RuntimeError(
                f"Native rustc --test compilation failed ({compiled.returncode}):\n"
                + compiled.stdout.decode("utf-8", errors="replace")
            )

    def native(self, name: str) -> subprocess.CompletedProcess:
        return guarded([str(self.binary), name, "--exact", "--nocapture"], label="actual native Rust harness")

    def test_native_positive_executes_and_retains_both_streams(self) -> None:
        result = self.native("positive")
        self.assertEqual(0, result.returncode, result.stdout)
        self.assertIn("native stdout π".encode("utf-8"), result.stdout)
        self.assertIn("native stderr 雪".encode("utf-8"), result.stdout)
        self.assertIn(b"1 passed; 0 failed; 0 ignored;", result.stdout)

    def test_native_zero_selection_is_rejected(self) -> None:
        result = self.native("no_such_native_test")
        self.assertEqual(1, result.returncode, result.stdout)
        self.assertIn(b"0 passed; 0 failed; 0 ignored;", result.stdout)

    def test_native_ignored_only_is_rejected(self) -> None:
        result = self.native("ignored")
        self.assertEqual(1, result.returncode, result.stdout)
        self.assertIn(b"0 passed; 0 failed; 1 ignored;", result.stdout)

    def test_native_failure_preserves_real_harness_status(self) -> None:
        direct = subprocess.run(
            [str(self.binary), "failure", "--exact", "--nocapture"],
            stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=30, check=False,
        )
        self.assertNotEqual(0, direct.returncode, direct.stdout)
        result = self.native("failure")
        self.assertEqual(direct.returncode, result.returncode, result.stdout)
        self.assertIn(b"intentional native guard failure", result.stdout)
        self.assertIn(b"test result: FAILED.", result.stdout)


if __name__ == "__main__":
    unittest.main(verbosity=2)
