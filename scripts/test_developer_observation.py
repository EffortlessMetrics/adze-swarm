#!/usr/bin/env python3
"""Synthetic validator fixtures, not developer sessions or usability evidence."""
import copy
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

SCRIPT = Path(__file__).with_name("check_developer_observation.py")
spec = importlib.util.spec_from_file_location("observation", SCRIPT)
validator = importlib.util.module_from_spec(spec)
spec.loader.exec_module(validator)


def fixture():
    return {
        "schema": validator.SCHEMA, "packet_version": 1,
        "source_sha": "a" * 40, "docs_sha": "b" * 40,
        "participant_kind": "agent", "participant_code": "synthetic-unit-fixture",
        "consent_recorded": False, "platform": "synthetic", "toolchain": "synthetic",
        "started_at": "2026-10-31T01:00:00Z", "finished_at": "2026-10-31T02:00:00Z",
        "tasks": [{
            "id": task_id, "outcome": "not_attempted", "assistance": [], "evidence": [],
            "observed_result": "", "elapsed_seconds": None, "blocker": "Synthetic unattempted task",
            "failed_commands": [], "confusing_messages": [], "owner": "",
        } for task_id in validator.TASK_IDS],
    }


def completed():
    data = fixture()
    data["tasks"][0].update(outcome="independent", evidence=["synthetic-log#value"],
                            observed_result="Synthetic expected value", elapsed_seconds=10, blocker="")
    return data


class ObservationTests(unittest.TestCase):
    def rejects(self, data):
        with self.assertRaises(validator.InvalidObservation):
            validator.validate(data)

    def test_valid_unattempted_agent_record(self):
        validator.validate(fixture())

    def test_completed_task_requires_observed_value_and_artifact(self):
        validator.validate(completed())
        for field, value in (("evidence", []), ("observed_result", " "), ("elapsed_seconds", None)):
            data = completed()
            data["tasks"][0][field] = value
            self.rejects(data)

    def test_assistance_cannot_be_labeled_independent(self):
        data = completed()
        data["tasks"][0]["assistance"] = ["Evaluator supplied command"]
        self.rejects(data)
        data["tasks"][0]["outcome"] = "assisted"
        validator.validate(data)

    def test_assisted_without_assistance_rejected(self):
        data = completed()
        data["tasks"][0]["outcome"] = "assisted"
        self.rejects(data)

    def test_all_task_rows_required(self):
        data = fixture()
        data["tasks"].pop()
        self.rejects(data)

    def test_duplicate_task_cannot_replace_missing_task(self):
        data = fixture()
        data["tasks"][-1] = copy.deepcopy(data["tasks"][0])
        self.rejects(data)

    def test_unknown_task_rejected(self):
        data = fixture()
        data["tasks"][0]["id"] = "D99"
        self.rejects(data)

    def test_short_or_mutable_source_identity_rejected(self):
        for value in ("main", "c7d3845", "A" * 40, None):
            data = fixture()
            data["source_sha"] = value
            self.rejects(data)

    def test_human_requires_consent(self):
        data = fixture()
        data["participant_kind"] = "human"
        self.rejects(data)
        data["consent_recorded"] = True
        validator.validate(data)

    def test_agent_cannot_claim_human_consent(self):
        data = fixture()
        data["consent_recorded"] = True
        self.rejects(data)

    def test_invalid_participant_kind_rejected(self):
        data = fixture()
        data["participant_kind"] = "agent-counted-as-human"
        self.rejects(data)

    def test_elapsed_time_rejects_bool_nan_negative_and_excess(self):
        for elapsed in (True, float("nan"), float("inf"), -1, 3601, "10", 10 ** 1000):
            data = completed()
            data["tasks"][0]["elapsed_seconds"] = elapsed
            self.rejects(data)

    def test_completed_task_cannot_retain_unresolved_blocker(self):
        data = completed()
        data["tasks"][0]["blocker"] = "Build still fails"
        self.rejects(data)

    def test_blocked_task_requires_owner(self):
        data = fixture()
        data["tasks"][0]["outcome"] = "blocked"
        self.rejects(data)
        data["tasks"][0]["owner"] = "#874"
        validator.validate(data)

    def test_unattempted_task_cannot_hide_execution(self):
        for field, value in (("evidence", ["log"]), ("assistance", ["hint"]),
                             ("observed_result", "done"), ("failed_commands", ["cargo test"]),
                             ("confusing_messages", ["message"]), ("elapsed_seconds", 0)):
            data = fixture()
            data["tasks"][0][field] = value
            self.rejects(data)

    def test_naive_and_reverse_timestamps_rejected(self):
        for field, value in (("started_at", "2026-10-31T01:00:00"),
                             ("finished_at", "2026-10-31T00:00:00Z")):
            data = fixture()
            data[field] = value
            self.rejects(data)

    def test_unknown_fields_and_boolean_version_rejected(self):
        data = fixture()
        data["satisfaction_score"] = 100
        self.rejects(data)
        data = fixture()
        data["packet_version"] = True
        self.rejects(data)

    def test_unknown_outcome_rejected(self):
        data = fixture()
        data["tasks"][0]["outcome"] = "passed"
        self.rejects(data)

    def test_actual_cli_reads_only_and_disclaims_product_proof(self):
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder) / "observation.json"
            original = json.dumps(completed()).encode()
            path.write_bytes(original)
            result = subprocess.run([sys.executable, str(SCRIPT), str(path)], capture_output=True, timeout=10)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertIn(b"not verified product proof", result.stdout)
            self.assertEqual(path.read_bytes(), original)
            self.assertEqual([item.name for item in Path(folder).iterdir()], ["observation.json"])

    def test_actual_cli_rejects_invalid_json_duplicate_keys_and_missing_file(self):
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder) / "observation.json"
            for content in ('{', '{"schema": "one", "schema": "two"}', '\xff'):
                path.write_bytes(content.encode("latin1"))
                result = subprocess.run([sys.executable, str(SCRIPT), str(path)], capture_output=True, timeout=10)
                self.assertEqual(result.returncode, 2)
                self.assertEqual(result.stdout, b"")
            path.unlink()
            result = subprocess.run([sys.executable, str(SCRIPT), str(path)], capture_output=True, timeout=10)
            self.assertEqual(result.returncode, 2)

    def test_unfilled_template_cannot_be_a_session_receipt(self):
        path = SCRIPT.parent.parent / "docs/product/developer-tasks/observation-template.json"
        self.rejects(validator.load(path))

    def test_task_packet_denominator_matches_instructions_and_rubric(self):
        import re
        folder = SCRIPT.parent.parent / "docs/product/developer-tasks"
        instructions = (folder / "tasks.md").read_text(encoding="utf-8")
        rubric = (folder / "evaluator.md").read_text(encoding="utf-8")
        self.assertEqual(tuple(re.findall(r"^## (D[0-9]{2}) ", instructions, re.MULTILINE)), validator.TASK_IDS)
        self.assertEqual(tuple(re.findall(r"^\| (D[0-9]{2}) \|", rubric, re.MULTILINE)), validator.TASK_IDS)
        self.assertIsNone(re.search(r"#[0-9]{3,}", instructions))

    def test_oversized_input_rejected(self):
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder) / "large.json"
            path.write_bytes(b" " * (validator.MAX_BYTES + 1))
            with self.assertRaises(validator.InvalidObservation):
                validator.load(path)


if __name__ == "__main__":
    unittest.main()
