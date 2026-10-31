#!/usr/bin/env python3
"""Truth-table tests for scripts/evaluate-required-result.py (#995).

Job outcomes are simulated as environment inputs; no GitHub-hosted run
is needed. Run: python3 scripts/test_evaluate_required_result.py
"""

from __future__ import annotations

import os
import subprocess
import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SCRIPT = ROOT / "scripts" / "evaluate-required-result.py"

RUST_PASS = {
    "EVENT_NAME": "merge_group",
    "DETECT_RESULT": "success",
    "RUST_NEEDED": "true",
    "DETECT_REASON": "event:merge_group",
    "ROUTE_RESULT": "success",
    "TARGET": "github",
    "TARGET_REASON": "merge_group_github_hosted",
    "FALLBACK_ALLOWED": "true",
    "SUPPORTED_RESULT": "success",
    "CPX42_RESULT": "skipped",
    "CX43_RESULT": "skipped",
    "CX33_RESULT": "skipped",
    "CX53_RESULT": "skipped",
    "GITHUB_RESULT": "success",
    "PR_ADMISSION_RESULT": "skipped",
}
PRODUCT_PASS = {
    "EVENT_NAME": "merge_group",
    "DETECT_RESULT": "success",
    "RUN_STABLE": "true",
    "DETECT_REASON": "stable_surface:runtime/lib.rs",
    "STABLE_RESULT": "success",
    "PR_CANARY_RESULT": "skipped",
}


def run(kind: str, env: dict[str, str]) -> int:
    clean = {k: v for k, v in os.environ.items() if k not in RUST_PASS and k not in PRODUCT_PASS}
    return subprocess.run(
        [sys.executable, str(SCRIPT), kind],
        env={**clean, **env},
        capture_output=True,
        text=True,
        check=False,
    ).returncode


def rust(**overrides: str) -> int:
    return run("rust", {**RUST_PASS, **overrides})


def product(**overrides: str) -> int:
    return run("product", {**PRODUCT_PASS, **overrides})


class ValidPassRows(unittest.TestCase):
    def test_valid_no_work_passes(self) -> None:
        self.assertEqual(0, rust(RUST_NEEDED="false", DETECT_REASON="no_rust_surface_changed"))
        self.assertEqual(0, product(RUN_STABLE="false", DETECT_REASON="no_stable_product_surface_changed"))

    def test_selected_proof_succeeds_passes(self) -> None:
        self.assertEqual(0, rust())
        self.assertEqual(0, product())
        for target, key in [
            ("cpx42", "CPX42_RESULT"), ("cx43", "CX43_RESULT"),
            ("cx33", "CX33_RESULT"), ("cx53", "CX53_RESULT"),
        ]:
            with self.subTest(target=target):
                self.assertEqual(0, rust(TARGET=target, GITHUB_RESULT="skipped", **{key: "success"}))

    def test_pull_request_admission_passes(self) -> None:
        self.assertEqual(0, rust(EVENT_NAME="pull_request", PR_ADMISSION_RESULT="success",
                                 SUPPORTED_RESULT="skipped", GITHUB_RESULT="skipped"))
        self.assertEqual(0, product(EVENT_NAME="pull_request", PR_CANARY_RESULT="success",
                                    STABLE_RESULT="skipped"))


class FailRows(unittest.TestCase):
    def test_detector_fails(self) -> None:
        for bad in ["failure", "cancelled", "skipped", ""]:
            with self.subTest(bad=bad):
                self.assertEqual(1, rust(DETECT_RESULT=bad))
                self.assertEqual(1, product(DETECT_RESULT=bad))

    def test_selector_missing_or_malformed(self) -> None:
        for bad in ["", "tru", "True", "yes", " true"]:
            with self.subTest(bad=bad):
                self.assertEqual(1, rust(RUST_NEEDED=bad))
                self.assertEqual(1, product(RUN_STABLE=bad))

    def test_pull_request_selector_validated_before_pr_branch(self) -> None:
        pr = {"EVENT_NAME": "pull_request", "PR_CANARY_RESULT": "success", "STABLE_RESULT": "skipped"}
        self.assertEqual(1, product(**pr, RUN_STABLE=""))
        self.assertEqual(1, product(**pr, RUN_STABLE="garbage"))
        self.assertEqual(1, product(**pr, RUN_STABLE="false", DETECT_REASON=""))

    def test_selected_job_skipped_or_cancelled(self) -> None:
        for bad in ["skipped", "cancelled"]:
            with self.subTest(bad=bad):
                self.assertEqual(1, rust(GITHUB_RESULT=bad))
                self.assertEqual(1, rust(SUPPORTED_RESULT=bad))
                self.assertEqual(1, product(STABLE_RESULT=bad))
                self.assertEqual(1, rust(EVENT_NAME="pull_request", PR_ADMISSION_RESULT=bad))
                self.assertEqual(1, product(EVENT_NAME="pull_request", PR_CANARY_RESULT=bad))

    def test_selected_job_fails(self) -> None:
        self.assertEqual(1, rust(GITHUB_RESULT="failure"))
        self.assertEqual(1, rust(SUPPORTED_RESULT="failure"))
        self.assertEqual(1, product(STABLE_RESULT="failure"))
        self.assertEqual(1, rust(EVENT_NAME="pull_request", PR_ADMISSION_RESULT="failure"))
        self.assertEqual(1, product(EVENT_NAME="pull_request", PR_CANARY_RESULT="failure"))

    def test_route_unavailable(self) -> None:
        for bad in ["failure", "cancelled", "skipped", ""]:
            with self.subTest(route=bad):
                self.assertEqual(1, rust(ROUTE_RESULT=bad))
        self.assertEqual(1, rust(TARGET="github", FALLBACK_ALLOWED="false"))
        self.assertEqual(1, rust(TARGET="github", FALLBACK_ALLOWED=""))
        self.assertEqual(1, rust(TARGET="pr"))  # admission pseudo-target on a qualification event
        self.assertEqual(1, rust(TARGET="nowhere"))

    def test_result_output_absent_or_truncated(self) -> None:
        self.assertEqual(1, rust(RUST_NEEDED="false", DETECT_REASON=""))
        self.assertEqual(1, product(RUN_STABLE="false", DETECT_REASON=""))
        self.assertEqual(1, rust(TARGET=""))
        self.assertEqual(1, rust(TARGET="gith"))
        self.assertEqual(1, rust(GITHUB_RESULT=""))
        self.assertEqual(1, rust(GITHUB_RESULT="succ"))
        self.assertEqual(1, product(STABLE_RESULT=""))
        self.assertEqual(1, product(STABLE_RESULT="succ"))


class WorkflowWiring(unittest.TestCase):
    def _job_block(self, path: str, job: str) -> str:
        import re

        text = (ROOT / path).read_text(encoding="utf-8")
        start = text.index(f"\n  {job}:")
        rest = text[start + 1:]
        match = re.search(r"\n  (?=\S)", rest[5:])
        return rest if match is None else rest[: match.start() + 5]

    def test_result_jobs_always_run_and_delegate_to_evaluator(self) -> None:
        cases = [
            (".github/workflows/product-proof.yml", "product-proof-result", "product"),
            (".github/workflows/em-ci-routed-rust.yml", "rust-small-result", "rust"),
        ]
        for path, job, kind in cases:
            with self.subTest(job=job):
                block = self._job_block(path, job)
                self.assertIn("always() &&", block)
                self.assertNotIn("!= 'cancelled'", block)
                self.assertNotIn('!= "cancelled"', block)
                self.assertNotIn("result != 'cancelled'", block)
                self.assertIn(f"evaluate-required-result.py {kind}", block)
                self.assertIn("actions/checkout", block)


if __name__ == "__main__":
    unittest.main(verbosity=2)
