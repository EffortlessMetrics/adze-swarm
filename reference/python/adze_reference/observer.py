"""Bounded second observer: finite sentential-form enumeration.

Materially different from the worklist reference: it breadth-first
enumerates sentential forms instead of propagating set equations.

Envelope (conclusive only when all hold):
  * enumeration from the start symbol and from every nonterminal drains
    its queue without exceeding any budget;
  * no sentential form exceeds ``max_form_length``;
  * every declared nonterminal is reachable from the start (otherwise its
    FOLLOW row is unobservable in start-rooted forms).

Any budget exhaustion, form-length overflow, or unreachable nonterminal
yields ``inconclusive`` -- exhaustion never counts as agreement.
Recursive grammars therefore normally return inconclusive here, which is
intended: the observer only claims agreement inside its finite envelope.
"""
from __future__ import annotations

from collections import deque
from dataclasses import dataclass, field

from .engine import SemanticResult
from .profile import Grammar, validate_request


@dataclass(frozen=True)
class ObserverBudgets:
    max_depth: int = 10
    max_states: int = 8192
    max_expansions: int = 8192
    max_form_length: int = 12


@dataclass
class Observation:
    status: str  # "conclusive" | "inconclusive"
    reasons: list[str] = field(default_factory=list)
    nullable: frozenset[str] = frozenset()
    first: dict[str, frozenset[str]] = field(default_factory=dict)
    follow_terminals: dict[str, frozenset[str]] = field(default_factory=dict)
    follow_eof: dict[str, bool] = field(default_factory=dict)
    states_seen: int = 0


def _enumerate(g: Grammar, seed: tuple[str, ...], budgets: ObserverBudgets, reasons: list[str], label: str):
    """Return set of sentential forms, or None when a budget is exceeded."""
    seen: set[tuple[str, ...]] = {seed}
    queue: deque[tuple[tuple[str, ...], int]] = deque([(seed, 0)])
    expansions = 0
    by_lhs: dict[str, list[tuple[str, ...]]] = {}
    for p in g.productions:
        by_lhs.setdefault(p.lhs, []).append(p.rhs)
    while queue:
        form, depth = queue.popleft()
        if len(form) > budgets.max_form_length:
            reasons.append(f"{label}: form length exceeds {budgets.max_form_length}")
            return None
        if depth >= budgets.max_depth:
            # A form at the depth cap that still has a nonterminal was pruned.
            if any(s in g.nonterminal_set for s in form):
                reasons.append(f"{label}: depth budget {budgets.max_depth} exhausted")
                return None
            continue
        for i, sym in enumerate(form):
            if sym not in g.nonterminal_set:
                continue
            for rhs in by_lhs[sym]:
                expansions += 1
                if expansions > budgets.max_expansions:
                    reasons.append(f"{label}: expansion budget {budgets.max_expansions} exhausted")
                    return None
                new_form = form[:i] + rhs + form[i + 1:]
                if len(new_form) > budgets.max_form_length:
                    reasons.append(f"{label}: form length exceeds {budgets.max_form_length}")
                    return None
                if new_form not in seen:
                    seen.add(new_form)
                    if len(seen) > budgets.max_states:
                        reasons.append(f"{label}: state budget {budgets.max_states} exhausted")
                        return None
                    queue.append((new_form, depth + 1))
    return seen


def observe(request: object, budgets: ObserverBudgets = ObserverBudgets()) -> Observation:
    g = validate_request(request)
    reasons: list[str] = []
    per_nt: dict[str, set[tuple[str, ...]]] = {}
    states = 0
    for nt in sorted(g.nonterminals):
        forms = _enumerate(g, (nt,), budgets, reasons, f"from {nt}")
        if forms is None:
            return Observation("inconclusive", reasons, states_seen=states)
        per_nt[nt] = forms
        states += len(forms)
    start_forms = _enumerate(g, (g.start,), budgets, reasons, "from start")
    if start_forms is None:
        return Observation("inconclusive", reasons, states_seen=states)
    states += len(start_forms)
    reachable: set[str] = set()
    for form in start_forms:
        reachable.update(s for s in form if s in g.nonterminal_set)
    reachable.add(g.start)
    missing = [nt for nt in sorted(g.nonterminals) if nt not in reachable]
    if missing:
        reasons.append(f"unreachable nonterminals unobservable for FOLLOW: {missing}")
        return Observation("inconclusive", reasons, states_seen=states)
    nullable = frozenset(nt for nt in g.nonterminals if () in per_nt[nt])
    # A terminal is in FIRST(nt) iff some derivable form starts with it:
    # expansions of a leading nonterminal (including epsilon expansions of
    # nullable prefixes) are themselves in the enumerated set.
    first = {
        nt: frozenset(form[0] for form in per_nt[nt] if form and form[0] in g.terminal_set)
        for nt in g.nonterminals
    }
    follow_t: dict[str, set[str]] = {nt: set() for nt in g.nonterminals}
    follow_eof: dict[str, bool] = {nt: False for nt in g.nonterminals}
    for form in start_forms:
        for i, sym in enumerate(form):
            if sym not in g.nonterminal_set:
                continue
            reaches_end = True
            for nxt in form[i + 1:]:
                if nxt in g.terminal_set:
                    follow_t[sym].add(nxt)
                    reaches_end = False
                    break
                follow_t[sym] |= set(first[nxt])
                if nxt not in nullable:
                    reaches_end = False
                    break
            if reaches_end:
                follow_eof[sym] = True
    return Observation(
        "conclusive", [], nullable,
        first,
        {nt: frozenset(v) for nt, v in follow_t.items()},
        follow_eof, states,
    )


def verdict(request: object, reference: SemanticResult, budgets: ObserverBudgets = ObserverBudgets()) -> tuple[str, list[str]]:
    """Return (status, mismatches). Inconclusive never counts as agreement."""
    obs = observe(request, budgets)
    if obs.status != "conclusive":
        return "inconclusive", list(obs.reasons)
    mismatches: list[str] = []
    if obs.nullable != reference.nullable:
        mismatches.append(f"nullable: observer={sorted(obs.nullable)} reference={sorted(reference.nullable)}")
    for nt in sorted(reference.first):
        if obs.first.get(nt, frozenset()) != reference.first[nt]:
            mismatches.append(f"first[{nt}]: observer={sorted(obs.first.get(nt, ()))} reference={sorted(reference.first[nt])}")
        ref_follow = reference.follow[nt]
        if obs.follow_terminals.get(nt, frozenset()) != ref_follow.terminals:
            mismatches.append(f"follow[{nt}] terminals: observer={sorted(obs.follow_terminals.get(nt, ()))} reference={sorted(ref_follow.terminals)}")
        if bool(obs.follow_eof.get(nt, False)) != bool(ref_follow.eof):
            mismatches.append(f"follow[{nt}] eof: observer={obs.follow_eof.get(nt)} reference={ref_follow.eof}")
    return ("conclusive_agree" if not mismatches else "conclusive_disagree"), mismatches
