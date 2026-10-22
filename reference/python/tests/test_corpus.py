import json
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from adze_reference import Status, compute, try_compute  # noqa: E402

CORPUS = (Path(__file__).resolve().parents[3]
          / "conformance" / "cases" / "cfg.nullable_first_follow.v1" / "corpus.json")
SKIP = "corpus not present in this checkout; it lands with #1050 (swarm/conformance-profile-974)"


def load():
    return json.loads(CORPUS.read_text(encoding="utf-8"))


def request_for(corpus, case):
    doc = {k: v for k, v in case.items() if k not in ("expectation", "rationale")}
    doc.setdefault("profile", corpus["profile"])
    return doc


def canonical_expectation(exp):
    return {
        "nullable": sorted(exp["nullable"]),
        "first": {k: sorted(v) for k, v in sorted(exp["first"].items())},
        "follow": {k: {"terminals": sorted(v["terminals"]), "eof": bool(v["eof"])}
                   for k, v in sorted(exp["follow"].items())},
    }


@unittest.skipUnless(CORPUS.is_file(), SKIP)
class CorpusTests(unittest.TestCase):
    def test_all_required_cases(self):
        corpus = load()
        self.assertEqual(corpus["profile"], "cfg.nullable_first_follow.v1")
        self.assertEqual(corpus["required_denominator"], 13)
        self.assertEqual(len(corpus["cases"]), 13)
        for case in corpus["cases"]:
            with self.subTest(case_id=case["case_id"]):
                result = compute(request_for(corpus, case))
                self.assertEqual(result.to_canonical_dict(),
                                 canonical_expectation(case["expectation"]))

    def test_invalid_cases_rejected_with_declared_class(self):
        corpus = load()
        self.assertTrue(corpus["invalid_cases"])
        for case in corpus["invalid_cases"]:
            with self.subTest(case_id=case["case_id"]):
                outcome = try_compute(case["document"])
                self.assertEqual(outcome.status, Status.INVALID)
                self.assertEqual(outcome.problem.code, case["reject_class"])


if __name__ == "__main__":
    unittest.main()
