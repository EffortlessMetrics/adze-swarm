"""Nullable / FIRST / FOLLOW reference for cfg.nullable_first_follow.v1.

Symbolic names with ordinary Python sets/maps and explicit worklists.
EOF is seeded only on the explicit start symbol. Output is canonical:
every declared nonterminal exactly once, all name lists sorted.
"""
from __future__ import annotations

import json
from collections import deque
from dataclasses import dataclass

from .profile import Grammar, Problem, ReferenceFailure, Status, validate_request


@dataclass(frozen=True)
class FollowSet:
    terminals: frozenset[str]
    eof: bool


@dataclass(frozen=True)
class SemanticResult:
    nullable: frozenset[str]
    first: dict[str, frozenset[str]]
    follow: dict[str, FollowSet]

    def to_canonical_dict(self) -> dict:
        return {
            "nullable": sorted(self.nullable),
            "first": {nt: sorted(self.first[nt]) for nt in sorted(self.first)},
            "follow": {
                nt: {"terminals": sorted(self.follow[nt].terminals), "eof": bool(self.follow[nt].eof)}
                for nt in sorted(self.follow)
            },
        }

    def canonical_json(self) -> bytes:
        return json.dumps(self.to_canonical_dict(), sort_keys=True, separators=(",", ":")).encode("utf-8")


@dataclass(frozen=True)
class Outcome:
    status: Status
    result: SemanticResult | None = None
    problem: Problem | None = None

    @property
    def ok(self) -> bool:
        return self.status is Status.OK


def _nullable(g: Grammar) -> set[str]:
    # Worklist over productions: count not-yet-nullable NT occurrences;
    # a terminal anywhere in the RHS blocks nullability for that production.
    blocked: list[bool] = []
    remaining: list[int] = []
    dependents: dict[str, list[int]] = {nt: [] for nt in g.nonterminals}
    for idx, p in enumerate(g.productions):
        blocked.append(any(s in g.terminal_set for s in p.rhs))
        cnt = 0
        for s in p.rhs:
            if s in g.nonterminal_set:
                cnt += 1
                dependents[s].append(idx)
        remaining.append(cnt)
    nullable: set[str] = set()
    queue: deque[str] = deque()
    for idx, p in enumerate(g.productions):
        if not blocked[idx] and remaining[idx] == 0 and p.lhs not in nullable:
            nullable.add(p.lhs); queue.append(p.lhs)
    done_prods: set[int] = set()
    while queue:
        nt = queue.popleft()
        for idx in dependents[nt]:
            if idx in done_prods or blocked[idx]:
                continue
            remaining[idx] -= g.productions[idx].rhs.count(nt)
            if remaining[idx] <= 0:
                done_prods.add(idx)
                lhs = g.productions[idx].lhs
                if lhs not in nullable:
                    nullable.add(lhs); queue.append(lhs)
    return nullable


def _first_of_sequence(seq, nullable, first, terminals):
    out: set[str] = set()
    for sym in seq:
        if sym in terminals:
            out.add(sym)
            return out, False
        out |= first[sym]
        if sym not in nullable:
            return out, False
    return out, True


def _first(g: Grammar, nullable: set[str]) -> dict[str, set[str]]:
    first: dict[str, set[str]] = {nt: set() for nt in g.nonterminals}
    dependents: dict[str, list[int]] = {nt: [] for nt in g.nonterminals}
    for idx, p in enumerate(g.productions):
        for s in p.rhs:
            if s in g.nonterminal_set:
                dependents[s].append(idx)
    queue: deque[str] = deque(sorted(g.nonterminals))
    in_queue = set(queue)
    while queue:
        # Recompute all productions for LHS symbols whose inputs changed is
        # handled by enqueueing LHS when any RHS nonterminal grows.
        nt = queue.popleft(); in_queue.discard(nt)
        for idx in dependents[nt]:
            p = g.productions[idx]
            add, _ = _first_of_sequence(p.rhs, nullable, first, g.terminal_set)
            if not add <= first[p.lhs]:
                first[p.lhs] |= add
                if p.lhs not in in_queue:
                    queue.append(p.lhs); in_queue.add(p.lhs)
    # Initial pass: productions whose RHS starts with a terminal (or is
    # otherwise independent) were seeded above only via dependents; do a
    # final worklist sweep seeded from every LHS to cover those.
    queue = deque(sorted(g.nonterminals)); in_queue = set(queue)
    by_lhs: dict[str, list] = {nt: [] for nt in g.nonterminals}
    for p in g.productions:
        by_lhs[p.lhs].append(p)
    while queue:
        lhs = queue.popleft(); in_queue.discard(lhs)
        for p in by_lhs[lhs]:
            add, _ = _first_of_sequence(p.rhs, nullable, first, g.terminal_set)
            if not add <= first[lhs]:
                first[lhs] |= add
                for idx in dependents[lhs]:
                    dep_lhs = g.productions[idx].lhs
                    if dep_lhs not in in_queue:
                        queue.append(dep_lhs); in_queue.add(dep_lhs)
    return first


def _follow(g: Grammar, nullable, first) -> dict[str, FollowSet]:
    follow_t: dict[str, set[str]] = {nt: set() for nt in g.nonterminals}
    follow_eof: dict[str, bool] = {nt: False for nt in g.nonterminals}
    follow_eof[g.start] = True  # EOF seeded only on explicit start.
    by_lhs: dict[str, list] = {nt: [] for nt in g.nonterminals}
    users: dict[str, set[str]] = {nt: set() for nt in g.nonterminals}
    for p in g.productions:
        by_lhs[p.lhs].append(p)
        for s in p.rhs:
            if s in g.nonterminal_set:
                users[s].add(p.lhs)
    queue: deque[str] = deque(sorted(g.nonterminals))
    in_queue = set(queue)

    def propagate(lhs: str) -> list[str]:
        grown: list[str] = []
        for p in by_lhs[lhs]:
            for i, sym in enumerate(p.rhs):
                if sym not in g.nonterminal_set:
                    continue
                add, suffix_nullable = _first_of_sequence(p.rhs[i + 1:], nullable, first, g.terminal_set)
                changed = False
                if not add <= follow_t[sym]:
                    follow_t[sym] |= add; changed = True
                if suffix_nullable:
                    if not follow_t[lhs] <= follow_t[sym]:
                        follow_t[sym] |= follow_t[lhs]; changed = True
                    if follow_eof[lhs] and not follow_eof[sym]:
                        follow_eof[sym] = True; changed = True
                if changed:
                    grown.append(sym)
        return grown

    while queue:
        lhs = queue.popleft(); in_queue.discard(lhs)
        for sym in propagate(lhs):
            if sym not in in_queue:
                queue.append(sym); in_queue.add(sym)
            for user in sorted(users[sym]):
                if user not in in_queue:
                    queue.append(user); in_queue.add(user)
    return {nt: FollowSet(frozenset(follow_t[nt]), follow_eof[nt]) for nt in g.nonterminals}


def compute_grammar(g: Grammar) -> SemanticResult:
    nullable = _nullable(g)
    first = _first(g, nullable)
    follow = _follow(g, nullable, first)
    return SemanticResult(
        nullable=frozenset(nullable),
        first={nt: frozenset(first[nt]) for nt in g.nonterminals},
        follow=follow,
    )


def compute(request: object) -> SemanticResult:
    return compute_grammar(validate_request(request))


def try_compute(request: object) -> Outcome:
    """Non-raising entry point returning typed ok/invalid/unsupported/bound results."""
    try:
        return Outcome(Status.OK, result=compute(request))
    except ReferenceFailure as exc:
        return Outcome(exc.problem.status, problem=exc.problem)
