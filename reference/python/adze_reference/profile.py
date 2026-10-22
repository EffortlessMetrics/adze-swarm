"""Strict validation for profile ``cfg.nullable_first_follow.v1`` (SPEC-0019).

Written from ADZE-SPEC-0019 only. Stdlib only; no subprocess, no Rust.
"""
from __future__ import annotations

import re
from dataclasses import dataclass
from enum import Enum

PROFILE = "cfg.nullable_first_follow.v1"
SCHEMA_VERSION = 1
MAX_SYMBOLS = 64
MAX_PRODUCTIONS = 256
MAX_RHS = 32
MAX_RHS_OCCURRENCES = 4096
MAX_NAME_BYTES = 64
RESERVED = frozenset({"EOF", "eof"})
_NAME_RE = re.compile(r"^[A-Za-z_][A-Za-z0-9_]*$")
_CASE_RE = re.compile(r"^[a-z][a-z0-9_]*$")
_ALLOWED_KEYS = frozenset({"profile", "schema_version", "case_id", "start", "terminals", "nonterminals", "productions"})
_EXPECTED_KEYS = frozenset({"expectation", "expected", "expected_result", "expectations"})


class Status(str, Enum):
    OK = "ok"
    INVALID = "invalid"
    UNSUPPORTED = "unsupported"
    BOUND_EXCEEDED = "bound_exceeded"


@dataclass(frozen=True)
class Problem:
    status: Status
    code: str
    message: str


class ReferenceFailure(Exception):
    def __init__(self, problem: Problem):
        super().__init__(f"{problem.status.value}:{problem.code}: {problem.message}")
        self.problem = problem


def _fail(status: Status, code: str, message: str) -> ReferenceFailure:
    return ReferenceFailure(Problem(status, code, message))


@dataclass(frozen=True)
class Production:
    lhs: str
    rhs: tuple[str, ...]


@dataclass(frozen=True)
class Grammar:
    case_id: str | None
    start: str
    terminals: tuple[str, ...]
    nonterminals: tuple[str, ...]
    productions: tuple[Production, ...]

    @property
    def terminal_set(self) -> frozenset[str]:
        return frozenset(self.terminals)

    @property
    def nonterminal_set(self) -> frozenset[str]:
        return frozenset(self.nonterminals)


def _is_str_list(v: object) -> bool:
    return isinstance(v, (list, tuple)) and all(isinstance(x, str) for x in v)


def validate_request(doc: object) -> Grammar:
    """Validate an engine request (case minus expectation). Raises ReferenceFailure."""
    if not isinstance(doc, dict):
        raise _fail(Status.INVALID, "malformed_document", "request must be a JSON object")
    extra = set(doc) - _ALLOWED_KEYS
    if extra:
        key = sorted(extra)[0]
        if key in _EXPECTED_KEYS:
            raise _fail(Status.INVALID, "expected_values_not_accepted",
                        f"engine requests never carry expected values: {key!r}")
        raise _fail(Status.INVALID, "unexpected_field", f"unexpected field: {key!r}")
    if "profile" in doc:
        if not isinstance(doc["profile"], str):
            raise _fail(Status.INVALID, "malformed_document", "profile must be a string")
        if doc["profile"] != PROFILE:
            raise _fail(Status.UNSUPPORTED, "profile_mismatch",
                        f"unsupported profile: {doc['profile']!r}")
    if "schema_version" in doc:
        v = doc["schema_version"]
        if isinstance(v, bool) or not isinstance(v, int):
            raise _fail(Status.INVALID, "malformed_document", "schema_version must be an integer")
        if v != SCHEMA_VERSION:
            raise _fail(Status.UNSUPPORTED, "schema_version_mismatch",
                        f"unsupported schema_version: {v!r}")
    for field in ("start", "terminals", "nonterminals", "productions"):
        if field not in doc:
            if field == "start":
                raise _fail(Status.INVALID, "missing_or_unknown_start", "start is required")
            raise _fail(Status.INVALID, "malformed_document", f"missing field: {field}")
    case_id = doc.get("case_id")
    if case_id is not None:
        if not isinstance(case_id, str):
            raise _fail(Status.INVALID, "malformed_document", "case_id must be a string")
        if len(case_id.encode("utf-8")) > MAX_NAME_BYTES:
            raise _fail(Status.BOUND_EXCEEDED, "bound_exceeded", "case_id exceeds 64 bytes")
        if not _CASE_RE.match(case_id):
            raise _fail(Status.INVALID, "malformed_case_id", f"malformed case_id: {case_id!r}")
    start = doc["start"]
    if not isinstance(start, str):
        raise _fail(Status.INVALID, "malformed_document", "start must be a string")
    if not _is_str_list(doc["terminals"]) or not _is_str_list(doc["nonterminals"]):
        raise _fail(Status.INVALID, "malformed_document", "terminals/nonterminals must be string arrays")
    terminals = list(doc["terminals"]); nonterminals = list(doc["nonterminals"])
    productions_raw = doc["productions"]
    if not isinstance(productions_raw, (list, tuple)):
        raise _fail(Status.INVALID, "malformed_document", "productions must be an array")
    # Bounds first among size checks so overflow is typed bound_exceeded.
    if len(terminals) + len(nonterminals) > MAX_SYMBOLS:
        raise _fail(Status.BOUND_EXCEEDED, "bound_exceeded", "more than 64 declared symbols")
    if len(productions_raw) > MAX_PRODUCTIONS:
        raise _fail(Status.BOUND_EXCEEDED, "bound_exceeded", "more than 256 productions")
    for name in terminals + nonterminals:
        if len(name.encode("utf-8")) > MAX_NAME_BYTES:
            raise _fail(Status.BOUND_EXCEEDED, "bound_exceeded", f"name exceeds 64 bytes: {name!r}")
        if name in RESERVED:
            raise _fail(Status.INVALID, "reserved_eof_name", f"reserved EOF name declared: {name!r}")
        if not _NAME_RE.match(name):
            raise _fail(Status.INVALID, "invalid_name", f"invalid symbol name: {name!r}")
    if len(set(terminals)) != len(terminals) or len(set(nonterminals)) != len(nonterminals):
        raise _fail(Status.INVALID, "duplicate_name", "duplicate terminal or nonterminal name")
    if set(terminals) & set(nonterminals):
        raise _fail(Status.INVALID, "terminal_nonterminal_collision",
                    f"name in both terminals and nonterminals: {sorted(set(terminals) & set(nonterminals))[0]!r}")
    if not nonterminals:
        raise _fail(Status.INVALID, "malformed_document", "at least one nonterminal is required")
    if start not in set(nonterminals):
        raise _fail(Status.INVALID, "missing_or_unknown_start", f"start is not a declared nonterminal: {start!r}")
    productions: list[Production] = []
    total_rhs = 0
    for i, p in enumerate(productions_raw):
        if not isinstance(p, dict) or set(p) - {"lhs", "rhs"} or "lhs" not in p or "rhs" not in p:
            raise _fail(Status.INVALID, "malformed_document", f"production {i} must be an object with lhs/rhs only")
        if not isinstance(p["lhs"], str) or not _is_str_list(p["rhs"]):
            raise _fail(Status.INVALID, "malformed_document", f"production {i} lhs/rhs have wrong types")
        if len(p["rhs"]) > MAX_RHS:
            raise _fail(Status.BOUND_EXCEEDED, "bound_exceeded", f"production {i} RHS exceeds 32 symbols")
        total_rhs += len(p["rhs"])
        productions.append(Production(p["lhs"], tuple(p["rhs"])))
    if total_rhs > MAX_RHS_OCCURRENCES:
        raise _fail(Status.BOUND_EXCEEDED, "bound_exceeded", "more than 4096 total RHS occurrences")
    declared = set(terminals) | set(nonterminals)
    for p in productions:
        if p.lhs not in set(nonterminals):
            raise _fail(Status.INVALID, "production_lhs_not_nonterminal",
                        f"production LHS is not a declared nonterminal: {p.lhs!r}")
        for sym in p.rhs:
            if sym not in declared:
                raise _fail(Status.INVALID, "undefined_symbol_reference",
                            f"undefined symbol reference: {sym!r}")
    if len(set(productions)) != len(productions):
        raise _fail(Status.INVALID, "duplicate_production", "exact duplicate production")
    with_prod = {p.lhs for p in productions}
    for nt in nonterminals:
        if nt not in with_prod:
            raise _fail(Status.INVALID, "nonterminal_without_production",
                        f"declared nonterminal has no production: {nt!r}")
    return Grammar(case_id, start, tuple(terminals), tuple(nonterminals), tuple(productions))
