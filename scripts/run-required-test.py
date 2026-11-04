#!/usr/bin/env python3
"""Run one native test command and require executed passing libtest evidence.

The child receives an argv list without a shell. Its merged stdout/stderr is
streamed byte-for-byte, including on failure. A successful process must provide
complete libtest summaries, no failed tests, and at least one passed test.
Ignored, measured, filtered-out and zero-selection rows do not count as passes.
An optional --expected-tests count requires one complete, unfiltered suite with
exactly that many passes and no ignored or measured tests. This checks the
reported denominator, not compiled test identities or source provenance.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys

SUMMARY = re.compile(
    r"test result: (?P<result>ok|FAILED)\. "
    r"(?P<passed>[0-9]+) passed; (?P<failed>[0-9]+) failed; "
    r"(?P<ignored>[0-9]+) ignored; (?P<measured>[0-9]+) measured; "
    r"(?P<filtered>[0-9]+) filtered out; "
    r"finished in [0-9]+(?:\.[0-9]+)?s"
)


def message(text: str) -> None:
    sys.stdout.buffer.write((text + "\n").encode("utf-8"))
    sys.stdout.buffer.flush()


class TestEvidence:
    """Inspect complete output lines while the original bytes are forwarded."""

    def __init__(self, expected_tests: int | None = None) -> None:
        self.expected_tests = expected_tests
        self.pending = b""
        self.summaries = 0
        self.passed = 0
        self.error: str | None = None

    def reject(self, reason: str) -> None:
        if self.error is None:
            self.error = reason

    def line(self, raw: bytes, *, terminated: bool) -> None:
        text = raw.removesuffix(b"\r").decode("utf-8", errors="replace")
        if re.search(r"\bFAILED\b", text):
            self.reject("output contains a FAILED result")
        if not text.startswith("test result") and "test result:" not in text:
            return
        match = SUMMARY.fullmatch(text)
        if match is None or not terminated:
            self.reject("missing, truncated or malformed libtest summary")
            return
        try:
            passed, failed = int(match["passed"]), int(match["failed"])
            excluded = (
                tuple(int(match[field]) for field in ("ignored", "measured", "filtered"))
                if self.expected_tests is not None else ()
            )
        except ValueError:
            self.reject("malformed libtest summary count")
            return
        self.summaries += 1
        if match["result"] != "ok" or failed != 0:
            self.reject("libtest reported failed tests")
        self.passed += passed
        if self.expected_tests is not None:
            if passed != self.expected_tests:
                self.reject(f"expected exactly {self.expected_tests} passed tests, observed {passed}")
            if any(excluded):
                self.reject("exact suite must have zero ignored, measured and filtered-out tests")

    def feed(self, data: bytes) -> None:
        self.pending += data
        while b"\n" in self.pending:
            line, self.pending = self.pending.split(b"\n", 1)
            self.line(line, terminated=True)

    def finish(self) -> str | None:
        if self.pending:
            self.line(self.pending, terminated=False)
        if self.error is not None:
            return self.error
        if self.expected_tests is not None and self.summaries != 1:
            return f"exact suite requires one complete summary, observed {self.summaries}"
        if self.summaries == 0:
            return "no complete libtest summary was produced"
        if self.passed == 0:
            return "no passing tests executed (zero selection or ignored-only output)"
        return None


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--label", required=True)
    parser.add_argument(
        "--expected-tests", type=int, metavar="N",
        help="require one summary with exactly N passes and zero ignored/measured/filtered tests",
    )
    parser.add_argument("command", nargs=argparse.REMAINDER, help="-- executable [arguments...]")
    arguments = parser.parse_args(argv)
    if arguments.expected_tests is not None and arguments.expected_tests <= 0:
        parser.error("--expected-tests must be a positive integer")
    command = arguments.command
    if command[:1] == ["--"]:
        command = command[1:]
    if not command:
        parser.error("a native test command is required after --")

    message(f"[required test] {arguments.label}")
    message("argv=" + json.dumps(command, ensure_ascii=False))
    environment = {**os.environ, "CARGO_TERM_COLOR": "never"}
    evidence = TestEvidence(arguments.expected_tests)
    try:
        with subprocess.Popen(
            command, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, env=environment
        ) as process:
            assert process.stdout is not None
            while chunk := process.stdout.read1(65536):
                sys.stdout.buffer.write(chunk)
                sys.stdout.buffer.flush()
                evidence.feed(chunk)
            status = process.wait()
    except OSError as error:
        message(f"[required test] {arguments.label}: could not run child: {error}")
        return 2

    problem = evidence.finish()
    if status != 0:
        message(f"[required test] {arguments.label}: child exited with status {status}")
        return status
    if problem is not None:
        message(f"[required test] {arguments.label}: {problem}")
        return 1
    message(
        f"[required test] {arguments.label}: {evidence.passed} passed tests "
        f"in {evidence.summaries} complete summaries"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
