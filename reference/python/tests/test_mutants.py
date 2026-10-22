"""Faulty variants: each mutant must be caught against correct expectations."""
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from adze_reference import compute  # noqa: E402
from adze_reference.engine import SemanticResult, FollowSet, compute_grammar  # noqa: E402
from adze_reference.profile import PROFILE, Grammar, Production, validate_request  # noqa: E402


def doc(start, terminals, nonterminals, productions, case_id="mutant_case"):
    return {"profile": PROFILE, "case_id": case_id, "start": start,
            "terminals": terminals, "nonterminals": nonterminals,
            "productions": [{"lhs": lhs, "rhs": rhs} for lhs, rhs in productions]}


def canon(result):
    return result.to_canonical_dict()


def expected(nullable, first, follow):
    return {"nullable": sorted(nullable),
            "first": {k: sorted(v) for k, v in sorted(first.items())},
            "follow": {k: {"terminals": sorted(v[0]), "eof": v[1]} for k, v in sorted(follow.items())}}


def mutant_omit_eof(request):
    r = compute(request)
    return SemanticResult(r.nullable, r.first,
                          {nt: FollowSet(f.terminals, False) for nt, f in r.follow.items()})


def mutant_first_stops_at_nullable_prefix(request):
    g = validate_request(request)
    first = {nt: set() for nt in g.nonterminals}
    changed = True
    while changed:
        changed = False
        for p in g.productions:
            if not p.rhs:
                continue
            sym = p.rhs[0]  # fault: never look past the first symbol
            add = {sym} if sym in g.terminal_set else set(first[sym])
            if not add <= first[p.lhs]:
                first[p.lhs] |= add; changed = True
    r = compute(request)
    return SemanticResult(r.nullable, {nt: frozenset(first[nt]) for nt in g.nonterminals}, r.follow)


def _flawed_follow(request, *, through_terminal=False, first_occurrence_only=False,
                   seed_first_declared=False, stop_at_terminal=True):
    g = validate_request(request)
    ref = compute(request)
    nullable, first = ref.nullable, ref.first
    follow_t = {nt: set() for nt in g.nonterminals}
    follow_eof = {nt: False for nt in g.nonterminals}
    follow_eof[g.nonterminals[0] if seed_first_declared else g.start] = True

    def first_seq(seq):
        out, all_null = set(), True
        for sym in seq:
            if sym in g.terminal_set:
                out.add(sym)
                if through_terminal:
                    continue  # fault: a terminal does not stop propagation
                return out, False
            out |= set(first[sym])
            if sym not in nullable:
                return out, False
        return out, all_null

    changed = True
    while changed:
        changed = False
        for p in g.productions:
            seen = set()
            for i, sym in enumerate(p.rhs):
                if sym not in g.nonterminal_set:
                    continue
                if first_occurrence_only and sym in seen:
                    continue
                seen.add(sym)
                add, suffix_null = first_seq(p.rhs[i + 1:])
                if not add <= follow_t[sym]:
                    follow_t[sym] |= add; changed = True
                if suffix_null:
                    if not follow_t[p.lhs] <= follow_t[sym]:
                        follow_t[sym] |= follow_t[p.lhs]; changed = True
                    if follow_eof[p.lhs] and not follow_eof[sym]:
                        follow_eof[sym] = True; changed = True
    return SemanticResult(ref.nullable, ref.first,
                          {nt: FollowSet(frozenset(follow_t[nt]), follow_eof[nt]) for nt in g.nonterminals})


def mutant_mishandles_epsilon(request):
    g = validate_request(request)
    flawed = Grammar(g.case_id, g.start, g.terminals, g.nonterminals,
                     tuple(p for p in g.productions if p.rhs != ()))
    # Nonterminals left without any production keep empty rows in the mutant.
    base = compute_grammar(flawed) if flawed.productions else compute(request)
    first = {nt: base.first.get(nt, frozenset()) for nt in g.nonterminals}
    follow = {nt: base.follow.get(nt, FollowSet(frozenset(), nt == g.start)) for nt in g.nonterminals}
    return SemanticResult(frozenset(), first, follow)


class MutantTests(unittest.TestCase):
    def assert_caught(self, case, exp, mutant):
        self.assertEqual(canon(compute(case)), exp)
        self.assertNotEqual(canon(mutant(case)), exp, f"{mutant.__name__} was not caught")

    def test_omit_eof_caught(self):
        case = doc("S", ["a"], ["S"], [("S", ["a"])])
        exp = expected([], {"S": ["a"]}, {"S": ([], True)})
        self.assert_caught(case, exp, mutant_omit_eof)

    def test_first_stopping_at_nullable_prefix_caught(self):
        case = doc("S", ["a", "b"], ["S", "A"], [("S", ["A", "b"]), ("A", ["a"]), ("A", [])])
        exp = expected(["A"], {"S": ["a", "b"], "A": ["a"]},
                       {"S": ([], True), "A": (["b"], False)})
        self.assert_caught(case, exp, mutant_first_stops_at_nullable_prefix)

    def test_follow_propagating_through_terminal_caught(self):
        case = doc("S", ["a", "b"], ["S", "A"], [("S", ["A", "a"]), ("A", ["b"])])
        exp = expected([], {"S": ["b"], "A": ["b"]},
                       {"S": ([], True), "A": (["a"], False)})
        mutant = lambda r: _flawed_follow(r, through_terminal=True)  # noqa: E731
        self.assert_caught(case, exp, mutant)

    def test_losing_repeated_nullable_occurrence_caught(self):
        case = doc("S", ["a", "b"], ["S", "A"],
                   [("S", ["A", "a", "A"]), ("A", ["b"]), ("A", [])])
        exp = expected(["A"], {"S": ["a", "b"], "A": ["b"]},
                       {"S": ([], True), "A": (["a"], True)})
        mutant = lambda r: _flawed_follow(r, first_occurrence_only=True)  # noqa: E731
        self.assert_caught(case, exp, mutant)

    def test_inferring_start_from_ordering_caught(self):
        case = doc("S", ["a", "b"], ["A", "S"], [("A", ["a"]), ("S", ["A", "b"])])
        exp = expected([], {"S": ["a"], "A": ["a"]},
                       {"S": ([], True), "A": (["b"], False)})
        mutant = lambda r: _flawed_follow(r, seed_first_declared=True)  # noqa: E731
        self.assert_caught(case, exp, mutant)

    def test_mishandling_epsilon_caught(self):
        case = doc("S", [], ["S"], [("S", [])])
        exp = expected(["S"], {"S": []}, {"S": ([], True)})
        self.assert_caught(case, exp, mutant_mishandles_epsilon)


if __name__ == "__main__":
    unittest.main()
