#!/usr/bin/env python3
"""Bounded PR planning workflow contracts for #1027.

Execute the workflow's actual Bash aggregate with fixture job outcomes, evaluate
its simple Docs Gate condition, and exercise the unchanged telemetry consumer.
These are local Python/Bash contracts, not hosted or Rust qualification receipts.
Run: python3 scripts/test_pr_planning_gate.py
"""

from __future__ import annotations

import ast
import importlib.util
import json
import os
import re
import subprocess
import tempfile
import tomllib
import unittest
from pathlib import Path
from unittest import mock

ROOT = Path(__file__).resolve().parent.parent
WORKFLOW = ROOT / ".github/workflows/pr-gate.yml"
BASH_INVOCATIONS = 0


def job_block(path: Path, job: str) -> str:
    text = path.read_text(encoding="utf-8")
    match = re.search(rf"(?ms)^  {re.escape(job)}:\n(.*?)(?=^  [\w-]+:|\Z)", text)
    if match is None:
        raise AssertionError(f"missing job {job} in {path}")
    return match.group(1)


def step_block(job: str, name: str) -> str:
    match = re.search(rf"(?ms)^      - name: {re.escape(name)}\n(.*?)(?=^      - |\Z)", job)
    if match is None:
        raise AssertionError(f"missing step {name}")
    return match.group(1)


def aggregate_script() -> str:
    block = step_block(job_block(WORKFLOW, "success"), "Aggregate gate result")
    lines = block.splitlines()
    start = lines.index("        run: |") + 1
    body = []
    for line in lines[start:]:
        if line and not line.startswith("          "):
            break
        body.append(line[10:] if line else "")
    return "\n".join(body)


def job_condition(job: str) -> str:
    lines = job.splitlines()
    for index, line in enumerate(lines):
        if not line.startswith("    if: "):
            continue
        value = line.removeprefix("    if: ")
        if value not in {"|", ">", ">-", "|-"}:
            return value.strip()
        body = []
        for line in lines[index + 1:]:
            if not line.startswith("      "):
                break
            body.append(line.strip())
        return " ".join(body)
    raise AssertionError("job has no explicit condition")


def docs_selected(plan: str, docs_only: str) -> bool:
    expression = job_condition(job_block(WORKFLOW, "docs-gate"))
    expression = expression.replace("always()", "True")
    expression = expression.replace("needs.plan.result", repr(plan))
    expression = expression.replace("needs.plan.outputs.docs_only", repr(docs_only))
    expression = expression.replace("&&", " and ").replace("||", " or ")
    tree = ast.parse(expression, mode="eval")
    allowed = (ast.Expression, ast.BoolOp, ast.And, ast.Or, ast.Compare,
               ast.Eq, ast.NotEq, ast.Constant, ast.UnaryOp, ast.Not)
    if any(not isinstance(node, allowed) for node in ast.walk(tree)):
        raise AssertionError("Docs Gate condition exceeds this bounded fixture evaluator")
    return bool(eval(compile(tree, "<docs-gate condition>", "eval"), {"__builtins__": {}}))


def run_aggregate(plan: str = "success", docs_only: str = "false",
                  docs: str = "skipped", event: str = "merge_group") -> subprocess.CompletedProcess[str]:
    global BASH_INVOCATIONS
    values = {"PLAN_RESULT": plan, "DOCS_ONLY": docs_only, "DOCS_RESULT": docs,
              "EVENT_NAME": event}
    script = aggregate_script()
    # Retain executable original-source controls: the old aggregate directly
    # interpolated these fixed job-result values. The replacement uses env data.
    original = {"needs.plan.result": plan, "needs.docs-gate.result": docs,
                "needs.supported.result": "skipped" if docs_only == "true" else "success"}
    for expression, value in original.items():
        script = script.replace("${{ " + expression + " }}", value)
    if "${{" in script:
        raise AssertionError("unresolved workflow expression in aggregate")
    BASH_INVOCATIONS += 1
    return subprocess.run(["bash", "-c", script], env={**os.environ, **values},
                          capture_output=True, text=True, check=False)


class PlanningTruthTable(unittest.TestCase):
    def test_valid_planning_and_selected_format_pass(self) -> None:
        for event in ("merge_group", "workflow_dispatch"):
            for docs_only, docs in (("false", "skipped"), ("true", "success")):
                with self.subTest(event=event, docs_only=docs_only):
                    result = run_aggregate(docs_only=docs_only, docs=docs, event=event)
                    self.assertEqual(0, result.returncode, result.stdout + result.stderr)

    def test_plan_must_succeed_even_when_docs_result_looks_valid(self) -> None:
        for plan in ("failure", "cancelled", "skipped", "", "unknown"):
            for docs_only, docs in (("false", "skipped"), ("true", "success")):
                with self.subTest(plan=plan, docs_only=docs_only):
                    self.assertNotEqual(0, run_aggregate(plan, docs_only, docs).returncode)

    def test_docs_selector_must_be_present_and_exact(self) -> None:
        for selector in ("", "tru", "False", "unknown", " true "):
            with self.subTest(selector=selector):
                self.assertNotEqual(0, run_aggregate(docs_only=selector).returncode)

    def test_selected_format_must_succeed(self) -> None:
        for docs in ("failure", "cancelled", "skipped", "", "unknown"):
            with self.subTest(docs=docs):
                self.assertNotEqual(0, run_aggregate(docs_only="true", docs=docs).returncode)

    def test_unselected_format_must_be_skipped(self) -> None:
        for docs in ("success", "failure", "cancelled", "", "unknown"):
            with self.subTest(docs=docs):
                self.assertNotEqual(0, run_aggregate(docs=docs).returncode)


class WorkflowContracts(unittest.TestCase):
    def test_events_preserved_and_automatic_supported_duplicate_retired(self) -> None:
        text = WORKFLOW.read_text(encoding="utf-8")
        event_block = re.search(r"(?ms)^on:\n(.*?)(?=^permissions:)", text)
        self.assertIsNotNone(event_block)
        events = set(re.findall(r"^  ([\w_]+):", event_block.group(1), re.M))
        for event in ("merge_group", "workflow_dispatch", "pull_request", "push", "schedule"):
            with self.subTest(event=event):
                self.assertEqual(event in {"merge_group", "workflow_dispatch"}, event in events)
        self.assertEqual({"plan", "docs-gate", "success"},
                         set(re.findall(r"^  ([\w-]+):", text.split("jobs:\n", 1)[1], re.M)))
        self.assertNotIn("just ci-supported", text)
        self.assertNotIn("scripts/ci-supported.sh", text)
        self.assertNotIn("cargo clippy", text)
        self.assertNotIn("cargo test", text)
        self.assertIn("uses: ./.github/workflows/pr-plan.yml", job_block(WORKFLOW, "plan"))

    def test_docs_gate_still_selects_only_successful_docs_plan(self) -> None:
        for plan in ("success", "failure", "cancelled", "skipped", ""):
            for selector in ("true", "false", "", "unknown"):
                with self.subTest(plan=plan, selector=selector):
                    self.assertEqual(plan == "success" and selector == "true",
                                     docs_selected(plan, selector))
        self.assertIn("run: cargo fmt --all -- --check", job_block(WORKFLOW, "docs-gate"))

    def test_optional_result_always_evaluates_and_names_its_scope(self) -> None:
        block = job_block(WORKFLOW, "success")
        self.assertIn("name: PR Planning Result", block)
        self.assertIn("needs: [plan, docs-gate]", block)
        self.assertEqual("always()", job_condition(block))
        aggregate = step_block(block, "Aggregate gate result")
        for name, expression in {"PLAN_RESULT": "needs.plan.result",
                                 "DOCS_ONLY": "needs.plan.outputs.docs_only",
                                 "DOCS_RESULT": "needs.docs-gate.result"}.items():
            self.assertIn(name + ": ${{ " + expression + " }}", aggregate)
        self.assertIn("python3 scripts/test_pr_planning_gate.py", block)

    def test_canonical_supported_lane_points_to_retained_required_producer(self) -> None:
        ledger = tomllib.loads((ROOT / "policy/ci-lane-whitelist.toml").read_text())
        supported = [lane for lane in ledger["lane"] if lane["id"] == "ci-supported"]
        self.assertEqual(1, len(supported))
        self.assertEqual((".github/workflows/em-ci-routed-rust.yml", "supported-proof"),
                         (supported[0]["workflow"], supported[0]["job"]))
        remaining = [lane for lane in ledger["lane"] if lane["workflow"].endswith("pr-gate.yml")]
        self.assertEqual({"plan", "docs-gate", "success"}, {lane["job"] for lane in remaining})
        self.assertTrue(all(not lane["blocking"] for lane in remaining))
        producer = job_block(ROOT / ".github/workflows/em-ci-routed-rust.yml", "supported-proof")
        self.assertIn("run: just ci-supported", producer)
        aggregate = job_block(ROOT / ".github/workflows/em-ci-routed-rust.yml", "rust-small-result")
        self.assertIn("- supported-proof", aggregate)
        self.assertIn("evaluate-required-result.py rust", aggregate)
        product = job_block(ROOT / ".github/workflows/product-proof.yml", "ci-product-stable")
        self.assertIn("run: scripts/ci-product-stable.sh", product)
        settings = (ROOT / ".github/settings.yml").read_text()
        self.assertIn('- "Rust Small Result"', settings)
        self.assertIn('- "Product Proof Result"', settings)
        self.assertNotIn("PR Planning Result", settings)

    def test_telemetry_steps_and_consumer_preserve_plan_and_remaining_jobs(self) -> None:
        block = job_block(WORKFLOW, "success")
        for name in ("Download ci-plan artifact", "Emit ci-actuals.json"):
            step = step_block(block, name)
            self.assertIn("if: always()", step)
            self.assertIn("continue-on-error: true", step)
        self.assertIn("run: python3 scripts/ci/emit-ci-actuals.py",
                      step_block(block, "Emit ci-actuals.json"))
        self.assertIn("name: ci-plan", block)
        self.assertIn("name: ci-actuals", block)
        path = ROOT / "scripts/ci/emit-ci-actuals.py"
        spec = importlib.util.spec_from_file_location("ci_actuals_fixture", path)
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        names = ["PR Plan / PR Plan", "Docs Gate", "PR Planning Result"]
        jobs = [{"name": name, "conclusion": "success", "labels": ["ubuntu-latest"],
                 "started_at": "2026-11-03T00:00:00Z", "completed_at": "2026-11-03T00:00:02Z"}
                for name in names]
        with tempfile.TemporaryDirectory() as directory:
            previous = Path.cwd()
            try:
                os.chdir(directory)
                Path("target/ci").mkdir(parents=True)
                plan = {"fixture": "retained planning artifact", "selection": {"lanes": []}}
                Path("target/ci/ci-plan.json").write_text(json.dumps(plan))
                env = {"GITHUB_REPOSITORY": "fixture/repo", "GITHUB_RUN_ID": "123",
                       "GITHUB_TOKEN": "synthetic-fixture", "GITHUB_EVENT_NAME": "merge_group"}
                with mock.patch.dict(os.environ, env, clear=True), \
                     mock.patch.object(module, "gh_get", return_value={"jobs": jobs}):
                    self.assertEqual(0, module.main())
                actuals = json.loads(Path("target/ci/ci-actuals.json").read_text())
                self.assertEqual("ok", actuals["status"])
                self.assertEqual(plan, actuals["plan"])
                self.assertEqual(names, [job["name"] for job in actuals["jobs"]])
                self.assertEqual([2, 2, 2], [job["actual_seconds"] for job in actuals["jobs"]])
            finally:
                os.chdir(previous)


if __name__ == "__main__":
    result = unittest.main(verbosity=2, exit=False).result
    print(f"Planning aggregate Bash invocations: {BASH_INVOCATIONS}")
    raise SystemExit(0 if result.wasSuccessful() else 1)
