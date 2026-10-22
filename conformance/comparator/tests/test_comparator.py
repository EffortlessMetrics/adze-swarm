"""Unit tests for the #977 conformance comparator (fake engines)."""
import copy
import hashlib
import json
import os
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import comparator as cmp  # noqa: E402


def _case(case_id, expectation):
    return {
        "case_id": case_id, "start": "S", "terminals": ["a"], "nonterminals": ["S"],
        "productions": [{"lhs": "S", "rhs": ["a"]}],
        "expectation": expectation, "rationale": "test fixture",
    }


EXP = {"authority": "reasoned_semantic", "nullable": [],
       "first": {"S": ["a"]}, "follow": {"S": {"terminals": [], "eof": True}}}
EXP_RESULT = {"nullable": [], "first": {"S": ["a"]}, "follow": {"S": {"terminals": [], "eof": True}}}
WRONG_RESULT = {"nullable": [], "first": {"S": []}, "follow": {"S": {"terminals": [], "eof": True}}}


def make_corpus(cases):
    doc = {"profile": cmp.PROFILE, "schema_version": 1,
           "required_denominator": len(cases), "cases": cases}
    return cmp.Corpus(doc, cmp.canonical_bytes(doc))


def one_case_corpus():
    return make_corpus([_case("single_terminal", copy.deepcopy(EXP))])


class FakeEngine:
    """Returns one ok row per requested case, with configurable sabotage."""

    def __init__(self, name, result=None, mutate=None, identity=None):
        self.name = name
        self.result = EXP_RESULT if result is None else result
        self.mutate = mutate
        self.identity = identity or {"engine": f"fake-{name}", "engine_version": "1"}
        self.calls = []
        self.last_request = None

    def run(self, request, request_bytes):
        self.calls.append(request)
        self.last_request = request
        rows = [{"case_id": c["case_id"], "status": "ok", "result": copy.deepcopy(self.result)}
                for c in request["cases"]]
        envelope = {"identity": dict(self.identity),
                    "input_digest": hashlib.sha256(request_bytes).hexdigest(),
                    "input_digest_algorithm": "sha256", "results": rows}
        if self.mutate:
            envelope = self.mutate(envelope, request)
        return envelope


class RaisingEngine:
    def __init__(self, exc):
        self.exc = exc
        self.calls = []

    def run(self, request, request_bytes):
        self.calls.append(request)
        raise self.exc


def compare(corpus, rust=None, python=None, **kw):
    return cmp.run_comparison(
        corpus,
        {"rust": rust or FakeEngine("rust"), "python": python or FakeEngine("python")}, **kw)


def disposition(receipt, case_id="single_terminal"):
    return next(r["disposition"] for r in receipt["per_case"] if r["case_id"] == case_id)


class TestAgreement(unittest.TestCase):
    def test_full_agreement(self):
        receipt = compare(one_case_corpus())
        self.assertEqual(receipt["aggregate_disposition"], "agrees")
        self.assertEqual(disposition(receipt), "agrees")
        self.assertEqual(receipt["counts"]["selected"], 1)
        self.assertEqual(receipt["counts"]["rust"]["ok"], 1)
        self.assertEqual(receipt["counts"]["python"]["ok"], 1)

    def test_request_carries_no_expectation(self):
        rust = FakeEngine("rust")
        compare(one_case_corpus(), rust=rust)
        blob = json.dumps(rust.last_request)
        self.assertNotIn("expectation", blob)
        self.assertNotIn("rationale", blob)
        self.assertEqual(set(rust.last_request), {"profile", "schema_version", "cases"})

    def test_determinism_byte_compare(self):
        r1 = compare(one_case_corpus())
        r2 = compare(one_case_corpus())
        self.assertEqual(cmp.receipt_json_bytes(r1), cmp.receipt_json_bytes(r2))
        self.assertEqual(r1["semantic_digest"], r2["semantic_digest"])
        # No volatile telemetry keys anywhere in the semantic receipt.
        blob = cmp.receipt_json_bytes(r1).decode()
        for volatile in ("elapsed", "timestamp", "wall_time", "cpu_time"):
            self.assertNotIn(volatile, blob)

    def test_rust_mismatch_and_python_mismatch(self):
        r = compare(one_case_corpus(), rust=FakeEngine("rust", result=WRONG_RESULT))
        self.assertEqual(disposition(r), "rust_mismatch")
        r = compare(one_case_corpus(), python=FakeEngine("python", result=WRONG_RESULT))
        self.assertEqual(disposition(r), "python_mismatch")

    def test_inconclusive_observer(self):
        r = compare(one_case_corpus(), observer=lambda cid: "inconclusive")
        self.assertEqual(disposition(r), "inconclusive_observer")


class TestNegativeControls(unittest.TestCase):
    def test_01_omitted_rust_row(self):
        rust = FakeEngine("rust", mutate=lambda e, req: {**e, "results": []})
        receipt = compare(one_case_corpus(), rust=rust)
        self.assertEqual(disposition(receipt), "engine_incomplete")
        self.assertEqual(receipt["counts"]["rust"]["missing"], 1)

    def test_02_omitted_python_row(self):
        python = FakeEngine("python", mutate=lambda e, req: {**e, "results": []})
        receipt = compare(one_case_corpus(), python=python)
        self.assertEqual(disposition(receipt), "engine_incomplete")
        self.assertEqual(receipt["counts"]["python"]["missing"], 1)

    def test_03_duplicate_or_unknown_row(self):
        def dup(e, req):
            e["results"] = e["results"] + [dict(e["results"][0])]
            return e

        def unknown(e, req):
            e["results"] = e["results"] + [{"case_id": "nope", "status": "ok",
                                            "result": copy.deepcopy(EXP_RESULT)}]
            return e

        for mutate in (dup, unknown):
            receipt = compare(one_case_corpus(), rust=FakeEngine("rust", mutate=mutate))
            self.assertEqual(receipt["aggregate_disposition"], "harness_failure")

    def test_04_wrong_input_digest(self):
        rust = FakeEngine("rust", mutate=lambda e, req: {**e, "input_digest": "0" * 64})
        receipt = compare(one_case_corpus(), rust=rust)
        self.assertEqual(disposition(receipt), "identity_mismatch")
        self.assertEqual(receipt["aggregate_disposition"], "identity_mismatch")

    def test_05_stale_rust_adapter_identity(self):
        rust = FakeEngine("rust", identity={"engine": "adze-glr-core", "engine_version": "0.0.0-stale"})
        receipt = compare(one_case_corpus(), rust=rust,
                          expected_identities={"rust": {"engine": "adze-glr-core",
                                                        "engine_version": "1"}})
        self.assertEqual(disposition(receipt), "identity_mismatch")

    def test_06_unsupported_required_case(self):
        rust = FakeEngine("rust", mutate=lambda e, req: {
            **e, "results": [{"case_id": "single_terminal", "status": "unsupported",
                              "reason": "profile not supported"}]})
        receipt = compare(one_case_corpus(), rust=rust)
        self.assertEqual(disposition(receipt), "engine_incomplete")
        self.assertNotEqual(receipt["aggregate_disposition"], "agrees")

    def test_07_malformed_partial_json(self):
        class RawEngine:
            def run(self, request, request_bytes):
                return '{"identity": {}, "results": [{"case_id": "single_termi'

        receipt = cmp.run_comparison(one_case_corpus(),
                                     {"rust": RawEngine(), "python": FakeEngine("python")})
        self.assertEqual(receipt["aggregate_disposition"], "harness_failure")

    def test_08_both_engines_same_wrong_answer(self):
        receipt = compare(one_case_corpus(),
                          rust=FakeEngine("rust", result=WRONG_RESULT),
                          python=FakeEngine("python", result=WRONG_RESULT))
        self.assertEqual(disposition(receipt), "both_disagree_with_expectation")
        self.assertEqual(receipt["aggregate_disposition"], "both_disagree_with_expectation")

    def test_09_zero_selected_cases(self):
        doc = {"profile": cmp.PROFILE, "schema_version": 1, "required_denominator": 0, "cases": []}
        with self.assertRaises(cmp.HarnessFailure):
            cmp.Corpus(doc, b"{}")
        empty = make_corpus([_case("single_terminal", copy.deepcopy(EXP))])
        empty.selected_case_ids = []
        rust = FakeEngine("rust")
        with self.assertRaises(cmp.HarnessFailure):
            cmp.run_comparison(empty, {"rust": rust, "python": FakeEngine("python")})
        self.assertEqual(rust.calls, [])  # engine never invoked

    def test_10_engine_crash_and_timeout(self):
        for exc, status in ((cmp.EngineTimeout("too slow"), "timeout"),
                            (RuntimeError("segfault"), "engine_error")):
            rust = RaisingEngine(exc)
            receipt = compare(one_case_corpus(), rust=rust)
            self.assertEqual(disposition(receipt), "engine_incomplete")
            row = next(r for r in receipt["per_case"])
            self.assertEqual(row["engine_status"]["rust"], status)
            self.assertNotEqual(receipt["aggregate_disposition"], "agrees")

    def test_11_expectation_key_in_request(self):
        bad = {"profile": cmp.PROFILE, "schema_version": 1,
               "cases": [{"case_id": "x", "expectation": {"nullable": []}}]}
        with self.assertRaises(cmp.HarnessFailure):
            cmp.assert_no_expectation_keys(bad)
        nested = {"cases": [{"productions": [{"lhs": "S", "rhs": [], "expected": []}]}]}
        with self.assertRaises(cmp.HarnessFailure):
            cmp.assert_no_expectation_keys(nested)
        # And the real request builder strips expectations instead of sending them.
        req = cmp.build_engine_request(cmp.PROFILE, 1, one_case_corpus().cases)
        cmp.assert_no_expectation_keys(req)


class TestRowValidation(unittest.TestCase):
    def test_type_confusion_and_missing_nonterminal_row(self):
        bad = copy.deepcopy(EXP_RESULT)
        bad["follow"]["S"]["eof"] = 1  # int, not bool
        receipt = compare(one_case_corpus(), rust=FakeEngine("rust", result=bad))
        self.assertEqual(disposition(receipt), "harness_failure")
        bad2 = copy.deepcopy(EXP_RESULT)
        bad2["first"] = {}  # missing nonterminal row
        receipt = compare(one_case_corpus(), rust=FakeEngine("rust", result=bad2))
        self.assertEqual(disposition(receipt), "harness_failure")

    def test_expectation_ambiguous_or_suspect(self):
        corpus = make_corpus([_case("single_terminal", {"authority": "who_knows",
                                                        "nullable": [], "first": {"S": ["a"]},
                                                        "follow": {"S": {"terminals": [], "eof": True}}})])
        receipt = compare(corpus)
        self.assertEqual(disposition(receipt), "expectation_ambiguous_or_suspect")


@unittest.skipUnless(
    os.environ.get("ADZE_CONF_CORPUS") and os.environ.get("ADZE_CONF_PYREF")
    and os.environ.get("ADZE_CONF_RUST_DRIVER"),
    "set ADZE_CONF_CORPUS, ADZE_CONF_PYREF and ADZE_CONF_RUST_DRIVER for the end-to-end run")
class TestEndToEnd(unittest.TestCase):
    def test_full_corpus_agreement(self):
        corpus = cmp.load_corpus(os.environ["ADZE_CONF_CORPUS"])
        self.assertEqual(len(corpus.selected_case_ids), 13)
        rust = cmp.RustDriverEngine.build(os.environ["ADZE_CONF_RUST_DRIVER"])
        python = cmp.PythonReferenceEngine(os.environ["ADZE_CONF_PYREF"])
        receipt = cmp.run_comparison(corpus, {"rust": rust, "python": python})
        self.assertEqual(receipt["counts"]["rust"]["executed"], 13)
        self.assertEqual(receipt["counts"]["python"]["executed"], 13)
        self.assertEqual(receipt["aggregate_disposition"], "agrees",
                         json.dumps(receipt["per_case"], indent=1))


if __name__ == "__main__":
    unittest.main()
