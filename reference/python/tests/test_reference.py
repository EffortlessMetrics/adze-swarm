import ast
import copy
import json
import random
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from adze_reference import Status, compute, try_compute  # noqa: E402
from adze_reference.profile import PROFILE  # noqa: E402

HERE = Path(__file__).resolve().parent


def req(**kw):
    base = {"profile": PROFILE, "case_id": "sample_case", "start": "S",
            "terminals": ["a", "b"], "nonterminals": ["S", "A"],
            "productions": [{"lhs": "S", "rhs": ["A", "b"]},
                             {"lhs": "A", "rhs": ["a"]}, {"lhs": "A", "rhs": []}]}
    base.update(kw)
    return base


class ValidationTests(unittest.TestCase):
    def test_ok_result_shape_every_nonterminal_exactly_once(self):
        result = compute(req(nonterminals=["A", "S"]))
        canon = result.to_canonical_dict()
        self.assertEqual(sorted(canon["first"]), ["A", "S"])
        self.assertEqual(sorted(canon["follow"]), ["A", "S"])
        self.assertEqual(canon["nullable"], ["A"])
        self.assertEqual(canon["first"], {"A": ["a"], "S": ["a", "b"]})
        self.assertEqual(canon["follow"]["S"], {"terminals": [], "eof": True})
        self.assertEqual(canon["follow"]["A"], {"terminals": ["b"], "eof": False})

    def test_expected_values_are_never_accepted(self):
        doc = req(expectation={"nullable": []})
        outcome = try_compute(doc)
        self.assertEqual(outcome.status, Status.INVALID)
        self.assertEqual(outcome.problem.code, "expected_values_not_accepted")

    def test_unknown_field_rejected(self):
        outcome = try_compute(req(bogus=1))
        self.assertEqual((outcome.status, outcome.problem.code), (Status.INVALID, "unexpected_field"))

    def test_unsupported_profile_and_schema(self):
        self.assertEqual(try_compute(req(profile="other.v9")).status, Status.UNSUPPORTED)
        self.assertEqual(try_compute(req(profile="other.v9")).problem.code, "profile_mismatch")
        doc = req(schema_version=99)
        self.assertEqual((try_compute(doc).status, try_compute(doc).problem.code),
                         (Status.UNSUPPORTED, "schema_version_mismatch"))

    def test_invalid_classes(self):
        cases = [
            (req(start="Missing"), "missing_or_unknown_start"),
            (req(terminals=["a", "S"]), "terminal_nonterminal_collision"),
            (req(productions=[{"lhs": "S", "rhs": ["a", "Missing"]},
                              {"lhs": "A", "rhs": ["a"]}]), "undefined_symbol_reference"),
            (req(productions=[{"lhs": "S", "rhs": ["a"]}, {"lhs": "S", "rhs": ["a"]},
                              {"lhs": "A", "rhs": ["a"]}]), "duplicate_production"),
            (req(productions=[{"lhs": "S", "rhs": ["a"]}]), "nonterminal_without_production"),
            (req(case_id="BadCase"), "malformed_case_id"),
            (req(terminals=["a", "EOF"], productions=[{"lhs": "S", "rhs": ["a"]},
                              {"lhs": "A", "rhs": ["a"]}]), "reserved_eof_name"),
            (req(terminals=["a", "a"]), "duplicate_name"),
        ]
        for doc, code in cases:
            with self.subTest(code=code):
                outcome = try_compute(doc)
                self.assertEqual(outcome.status, Status.INVALID)
                self.assertEqual(outcome.problem.code, code)

    def test_bound_exceeded(self):
        many_terms = [f"t{i}" for i in range(70)]
        self.assertEqual(try_compute(req(terminals=many_terms)).status, Status.BOUND_EXCEEDED)
        long_rhs = req(productions=[{"lhs": "S", "rhs": ["a"] * 33}, {"lhs": "A", "rhs": ["a"]}])
        self.assertEqual(try_compute(long_rhs).status, Status.BOUND_EXCEEDED)
        many_prods = req(productions=[{"lhs": "S", "rhs": ["a"]}] * 300 + [{"lhs": "A", "rhs": ["a"]}])
        self.assertEqual(try_compute(many_prods).status, Status.BOUND_EXCEEDED)

    def test_malformed_types(self):
        self.assertEqual(try_compute(["not", "a", "dict"]).problem.code, "malformed_document")
        self.assertEqual(try_compute(req(terminals="ab")).problem.code, "malformed_document")


class DeterminismTests(unittest.TestCase):
    def test_reorder_invariance(self):
        generated = json.loads((HERE / "generated_cases.json").read_text(encoding="utf-8"))["cases"]
        rng = random.Random(20260214)
        for case in generated:
            expected = compute(case).canonical_json()
            for _ in range(6):
                shuffled = copy.deepcopy(case)
                rng.shuffle(shuffled["terminals"]); rng.shuffle(shuffled["nonterminals"])
                rng.shuffle(shuffled["productions"])
                self.assertEqual(compute(shuffled).canonical_json(), expected, case["case_id"])

    def test_repeated_runs_byte_identical(self):
        generated = json.loads((HERE / "generated_cases.json").read_text(encoding="utf-8"))["cases"]
        first = [compute(c).canonical_json() for c in generated]
        second = [compute(c).canonical_json() for c in generated]
        self.assertEqual(first, second)


class IndependenceTests(unittest.TestCase):
    def test_stdlib_only_and_no_subprocess_import(self):
        pkg = Path(__file__).resolve().parents[1] / "adze_reference"
        imported = set()
        for path in pkg.glob("*.py"):
            tree = ast.parse(path.read_text(encoding="utf-8"))
            for node in ast.walk(tree):
                if isinstance(node, ast.Import):
                    imported.update(a.name.split(".")[0] for a in node.names)
                elif isinstance(node, ast.ImportFrom) and node.module and not node.level:
                    imported.add(node.module.split(".")[0])
        self.assertNotIn("subprocess", imported)
        allowed = set(sys.stdlib_module_names) | {"adze_reference"}
        self.assertLessEqual(imported, allowed, f"non-stdlib imports: {imported - allowed}")


if __name__ == "__main__":
    unittest.main()
