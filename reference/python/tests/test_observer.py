import json
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from adze_reference import compute  # noqa: E402
from adze_reference.observer import ObserverBudgets, observe, verdict  # noqa: E402
from adze_reference.profile import PROFILE  # noqa: E402

HERE = Path(__file__).resolve().parent


def doc(start, terminals, nonterminals, productions):
    return {"profile": PROFILE, "case_id": "observer_case", "start": start,
            "terminals": terminals, "nonterminals": nonterminals,
            "productions": [{"lhs": lhs, "rhs": rhs} for lhs, rhs in productions]}


class ObserverTests(unittest.TestCase):
    def test_generated_acyclic_cases_agree_over_complete_envelope(self):
        cases = json.loads((HERE / "generated_cases.json").read_text(encoding="utf-8"))["cases"]
        self.assertEqual(len(cases), 12)
        for case in cases:
            with self.subTest(case_id=case["case_id"]):
                status, mismatches = verdict(case, compute(case))
                self.assertEqual(status, "conclusive_agree", mismatches)

    def test_hand_acyclic_cases_agree(self):
        cases = [
            doc("S", ["a", "b"], ["S", "A"],
                [("S", ["A", "b"]), ("A", ["a"]), ("A", [])]),
            doc("S", ["a", "b"], ["S", "A", "B"],
                [("S", ["A", "B"]), ("A", ["a"]), ("A", []), ("B", ["b"]), ("B", [])]),
            doc("S", ["a", "b"], ["S", "A"],
                [("S", ["A", "a", "A"]), ("A", ["b"]), ("A", [])]),
        ]
        for case in cases:
            status, mismatches = verdict(case, compute(case))
            self.assertEqual(status, "conclusive_agree", mismatches)

    def test_recursive_grammar_is_inconclusive_never_agreement(self):
        case = doc("S", ["a", "b"], ["S"], [("S", ["S", "a"]), ("S", ["b"])])
        status, _ = verdict(case, compute(case))
        self.assertEqual(status, "inconclusive")

    def test_tiny_budgets_are_inconclusive(self):
        case = doc("S", ["c"], ["S", "A", "B"],
                   [("S", ["A"]), ("A", ["B"]), ("B", ["c"]), ("B", [])])
        status, reasons = verdict(case, compute(case), ObserverBudgets(max_depth=1))
        self.assertEqual(status, "inconclusive")
        self.assertTrue(reasons)
        self.assertEqual(observe(case, ObserverBudgets(max_states=2)).status, "inconclusive")

    def test_unreachable_nonterminal_is_inconclusive(self):
        case = doc("S", ["a", "b", "c"], ["S", "U", "V"],
                   [("S", ["a"]), ("U", ["V", "b"]), ("V", ["c"]), ("V", [])])
        status, reasons = verdict(case, compute(case))
        self.assertEqual(status, "inconclusive")
        self.assertTrue(any("unreachable" in r for r in reasons))


if __name__ == "__main__":
    unittest.main()
