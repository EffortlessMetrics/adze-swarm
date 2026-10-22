"""Denominator-complete Rust/Python conformance comparator (#977).

Pure standard library, Python >= 3.11. Expectations are loaded only here
and are never placed in an engine request. The full required case set is
selected before either engine is invoked; each engine then owes exactly
one terminal row per selected case.

Terminal row statuses: ok, invalid_input, unsupported, engine_error,
timeout. Only ``ok`` rows participate in semantic agreement for required
cases. Majority agreement is not truth: two engines returning the same
wrong result against an expectation is ``both_disagree_with_expectation``.

Dispositions: agrees, rust_mismatch, python_mismatch,
both_disagree_with_expectation, expectation_ambiguous_or_suspect,
engine_incomplete, identity_mismatch, harness_failure,
inconclusive_observer.
"""
from __future__ import annotations

import hashlib
import json
import platform
import subprocess
import sys
from pathlib import Path

RECEIPT_VERSION = 1
PROFILE = "cfg.nullable_first_follow.v1"
SCHEMA_VERSION = 1
TERMINAL_STATUSES = frozenset({"ok", "invalid_input", "unsupported", "engine_error", "timeout"})
EXPECTATION_KEYS = frozenset({"expectation", "expected", "expected_result", "expectations"})
KNOWN_AUTHORITIES = frozenset({"reasoned_semantic"})

# Aggregate severity: earlier wins.
_SEVERITY = [
    "harness_failure",
    "identity_mismatch",
    "engine_incomplete",
    "expectation_ambiguous_or_suspect",
    "both_disagree_with_expectation",
    "rust_mismatch",
    "python_mismatch",
    "inconclusive_observer",
    "agrees",
]


class HarnessFailure(Exception):
    """Comparator/harness defect, not an engine result."""


class EngineTimeout(Exception):
    """Engine exceeded its timeout (synthesised timeout rows follow)."""


class EngineCrash(Exception):
    """Engine process failed without a parseable envelope."""


def canonical_bytes(obj) -> bytes:
    return json.dumps(obj, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode("utf-8")


def sha256_hex(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def fnv1a64_hex(data: bytes) -> str:
    h = 0xCBF29CE484222325
    for b in data:
        h ^= b
        h = (h * 0x100000001B3) & 0xFFFFFFFFFFFFFFFF
    return format(h, "016x")


_DIGESTS = {"sha256": lambda b: sha256_hex(b), "fnv1a64": fnv1a64_hex}


def assert_no_expectation_keys(node, path: str = "$") -> None:
    """Fail harness-side if any expectation key would be sent to an engine."""
    if isinstance(node, dict):
        for key, value in node.items():
            if key in EXPECTATION_KEYS:
                raise HarnessFailure(f"expectation key {key!r} present in engine request at {path}")
            assert_no_expectation_keys(value, f"{path}.{key}")
    elif isinstance(node, list):
        for i, value in enumerate(node):
            assert_no_expectation_keys(value, f"{path}[{i}]")


def build_engine_request(profile: str, schema_version: int, cases: list[dict]) -> dict:
    """Strip everything except the engine-facing case fields, then guard."""
    stripped = [
        {
            "case_id": c["case_id"],
            "start": c["start"],
            "terminals": list(c["terminals"]),
            "nonterminals": list(c["nonterminals"]),
            "productions": [{"lhs": p["lhs"], "rhs": list(p["rhs"])} for p in c["productions"]],
        }
        for c in cases
    ]
    request = {"profile": profile, "schema_version": schema_version, "cases": stripped}
    assert_no_expectation_keys(request)
    return request


class Corpus:
    def __init__(self, doc: dict, raw_bytes: bytes):
        if not isinstance(doc, dict):
            raise HarnessFailure("corpus must be a JSON object")
        self.profile = doc.get("profile")
        self.schema_version = doc.get("schema_version")
        self.required_denominator = doc.get("required_denominator")
        self.cases = doc.get("cases")
        if not isinstance(self.cases, list) or not self.cases:
            raise HarnessFailure("zero selected cases: corpus has no cases")
        if self.required_denominator is not None and len(self.cases) != self.required_denominator:
            raise HarnessFailure(
                f"corpus denominator mismatch: required {self.required_denominator}, found {len(self.cases)}"
            )
        ids = [c.get("case_id") for c in self.cases]
        if len(set(ids)) != len(ids) or any(not isinstance(i, str) for i in ids):
            raise HarnessFailure("corpus case_ids must be unique strings")
        self.selected_case_ids = ids
        self.expectations = {c["case_id"]: c.get("expectation") for c in self.cases}
        # Corpus digest binds the canonical corpus bytes (selected cases,
        # expectations and metadata), independent of file whitespace.
        self.corpus_digest = sha256_hex(canonical_bytes(doc))
        self.expectation_digest = sha256_hex(canonical_bytes(self.expectations))
        self.raw_bytes = raw_bytes


def load_corpus(path) -> Corpus:
    raw = Path(path).read_bytes()
    try:
        doc = json.loads(raw.decode("utf-8"))
    except (ValueError, UnicodeDecodeError) as exc:
        raise HarnessFailure(f"malformed corpus JSON: {exc}") from exc
    return Corpus(doc, raw)


def _is_str_list(v) -> bool:
    return isinstance(v, list) and all(isinstance(x, str) for x in v)


def normalize_result(result, case: dict) -> dict:
    """Validate and canonicalise one semantic result; raise ValueError if malformed.

    Rejects type confusion (e.g. bool/int for ``eof``), missing/extra
    nonterminal rows, and undeclared symbols.
    """
    nonterminals = set(case["nonterminals"])
    terminals = set(case["terminals"])
    if not isinstance(result, dict) or set(result) != {"nullable", "first", "follow"}:
        raise ValueError("result must be an object with exactly nullable/first/follow")
    if not _is_str_list(result["nullable"]) or not set(result["nullable"]) <= nonterminals:
        raise ValueError("nullable must be a string list of declared nonterminals")
    for field in ("first", "follow"):
        if not isinstance(result[field], dict) or set(result[field]) != nonterminals:
            raise ValueError(f"{field} must have exactly one row per declared nonterminal")
    first = {}
    for nt, vals in result["first"].items():
        if not _is_str_list(vals) or not set(vals) <= terminals:
            raise ValueError("first rows must be string lists of declared terminals")
        first[nt] = sorted(vals)
    follow = {}
    for nt, row in result["follow"].items():
        if not isinstance(row, dict) or set(row) != {"terminals", "eof"}:
            raise ValueError("follow rows must be objects with exactly terminals/eof")
        if not _is_str_list(row["terminals"]) or not set(row["terminals"]) <= terminals:
            raise ValueError("follow terminals must be declared terminals")
        if type(row["eof"]) is not bool:  # bool check first: int is not accepted either
            raise ValueError("follow eof must be a boolean")
        follow[nt] = {"terminals": sorted(row["terminals"]), "eof": row["eof"]}
    return {"nullable": sorted(result["nullable"]), "first": first, "follow": follow}


def expectation_semantics(expectation, case: dict):
    """Return (canonical expectation, problem). problem is None when usable."""
    if not isinstance(expectation, dict):
        return None, "expectation missing or not an object"
    if expectation.get("authority") not in KNOWN_AUTHORITIES:
        return None, f"unknown expectation authority: {expectation.get('authority')!r}"
    semantic = {k: expectation.get(k) for k in ("nullable", "first", "follow")}
    try:
        return normalize_result(semantic, case), None
    except ValueError as exc:
        return None, f"malformed expectation: {exc}"


def _parse_engine_payload(payload):
    """Accept an envelope dict, or raw JSON text/bytes from a subprocess engine."""
    if isinstance(payload, (str, bytes)):
        text = payload.decode("utf-8") if isinstance(payload, bytes) else payload
        return json.loads(text)  # raises ValueError on malformed/partial JSON
    return payload


def _validate_engine_output(engine_name, payload, selected_ids, case_by_id):
    """Return (rows_by_id, global_disposition_or_None, detail, identity, counts).

    rows_by_id maps case_id -> row dict (synthetic rows included by caller
    for crash/timeout are handled before this function).
    """
    envelope = _parse_engine_payload(payload)
    if not isinstance(envelope, dict) or not isinstance(envelope.get("results"), list):
        raise HarnessFailure(f"{engine_name}: malformed/partial engine output: no results list")
    identity = envelope.get("identity") or {}
    rows = envelope["results"]
    seen: dict[str, dict] = {}
    global_disp = None
    detail = ""
    for row in rows:
        if not isinstance(row, dict) or not isinstance(row.get("case_id"), str) or "status" not in row:
            global_disp, detail = "harness_failure", f"{engine_name}: malformed row: {row!r:.200}"
            continue
        cid = row["case_id"]
        if cid not in case_by_id:
            global_disp, detail = "harness_failure", f"{engine_name}: unknown case row {cid!r}"
        elif cid in seen:
            global_disp, detail = "harness_failure", f"{engine_name}: duplicate case row {cid!r}"
        else:
            seen[cid] = row
    return seen, global_disp, detail, identity, envelope


def run_comparison(corpus: Corpus, engines: dict, expected_identities: dict | None = None,
                   components: dict | None = None, harness_controls: list | None = None,
                   observer=None) -> dict:
    """Run all engines over the full selected set and build the receipt.

    ``engines`` maps name -> engine object with ``run(request, request_bytes)``
    returning an envelope dict (or raw JSON text) or raising EngineTimeout /
    Exception. ``observer`` is an optional callable(case_id) -> 'conclusive' |
    'inconclusive'; an inconclusive observer only downgrades an otherwise
    agreeing case to ``inconclusive_observer``.
    """
    if not corpus.selected_case_ids:
        raise HarnessFailure("zero selected cases")
    # Selection happens before any engine invocation (comparator law).
    selected = list(corpus.cases)
    request = build_engine_request(corpus.profile, corpus.schema_version, selected)
    request_bytes = canonical_bytes(request)
    case_by_id = {c["case_id"]: c for c in selected}
    expected_identities = expected_identities or {}

    per_engine_rows: dict[str, dict] = {}
    per_engine_global: dict[str, tuple] = {}  # name -> (disposition, detail)
    engine_identities: dict[str, dict] = {}
    engine_counts: dict[str, dict] = {}

    for name, engine in engines.items():
        global_disp, detail = None, ""
        identity: dict = {}
        try:
            payload = engine.run(request, request_bytes)
            rows_by_id, global_disp, detail, identity, envelope = _validate_engine_output(
                name, payload, corpus.selected_case_ids, case_by_id)
            # Digest check: wrong case/input digest is an identity failure.
            digest = envelope.get("input_digest") if isinstance(envelope, dict) else None
            if digest is not None:
                algo = envelope.get("input_digest_algorithm") or "sha256"
                fn = _DIGESTS.get(algo)
                if fn is None or fn(request_bytes) != digest:
                    global_disp, detail = "identity_mismatch", f"{name}: input digest mismatch ({algo})"
            # Stale/wrong engine identity check (subset match).
            expected = expected_identities.get(name) or {}
            merged_identity = {**identity, **(envelope.get("identity") or {})} if isinstance(envelope, dict) else identity
            identity = merged_identity
            for key, want in expected.items():
                if identity.get(key) != want:
                    global_disp, detail = (
                        "identity_mismatch",
                        f"{name}: stale/wrong identity {key}={identity.get(key)!r}, expected {want!r}",
                    )
        except EngineTimeout as exc:
            rows_by_id = {cid: {"case_id": cid, "status": "timeout", "reason": str(exc)} for cid in corpus.selected_case_ids}
            global_disp, detail = "engine_incomplete", f"{name}: timeout: {exc}"
        except HarnessFailure:
            raise
        except ValueError as exc:  # malformed/partial JSON from the engine
            rows_by_id = {}
            global_disp, detail = "harness_failure", f"{name}: malformed/partial JSON: {exc}"
        except Exception as exc:  # engine crash: never an empty success
            rows_by_id = {cid: {"case_id": cid, "status": "engine_error", "reason": str(exc)} for cid in corpus.selected_case_ids}
            global_disp, detail = "engine_incomplete", f"{name}: engine crash: {exc}"
        engine_identities[name] = identity
        per_engine_rows[name] = rows_by_id
        per_engine_global[name] = (global_disp, detail)
        executed = len(rows_by_id)
        ok_count = sum(1 for r in rows_by_id.values() if r.get("status") == "ok")
        engine_counts[name] = {
            "selected": len(corpus.selected_case_ids),
            "executed": executed,
            "ok": ok_count,
            "failed": executed - ok_count,
            "missing": len(corpus.selected_case_ids) - executed,
        }

    rust_name = "rust" if "rust" in engines else (list(engines)[0] if engines else "rust")
    python_name = "python" if "python" in engines else (list(engines)[1] if len(engines) > 1 else "python")

    per_case = []
    dispositions = []
    for case in selected:
        cid = case["case_id"]
        disp, detail_parts = None, []
        results, statuses = {}, {}
        for name in engines:
            gdisp, gdetail = per_engine_global[name]
            if gdisp in ("harness_failure", "identity_mismatch") and disp is None:
                disp = gdisp
                detail_parts.append(gdetail)
            row = per_engine_rows[name].get(cid)
            if row is None:
                statuses[name] = "missing"
                if disp is None:
                    disp = "engine_incomplete"
                    detail_parts.append(f"{name}: missing terminal row for {cid}")
                continue
            status = row.get("status")
            statuses[name] = status if isinstance(status, str) else repr(status)
            if status not in TERMINAL_STATUSES:
                if disp is None:
                    disp = "harness_failure"
                detail_parts.append(f"{name}: unknown terminal status {status!r}")
                continue
            if status != "ok":
                if disp is None:
                    disp = "engine_incomplete"
                detail_parts.append(f"{name}: {status}: {row.get('reason', '')}")
                continue
            try:
                results[name] = normalize_result(row.get("result"), case)
            except ValueError as exc:
                if disp is None:
                    disp = "harness_failure"
                detail_parts.append(f"{name}: malformed result for {cid}: {exc}")
        if disp is None:
            expectation, problem = expectation_semantics(corpus.expectations.get(cid), case)
            if problem is not None:
                disp = "expectation_ambiguous_or_suspect"
                detail_parts.append(problem)
            elif len(results) == len(engines):
                rust_res = results.get(rust_name)
                python_res = results.get(python_name)
                rust_ok = rust_res == expectation
                python_ok = python_res == expectation
                if rust_ok and python_ok:
                    disp = "agrees"
                    if observer is not None and observer(cid) == "inconclusive":
                        disp = "inconclusive_observer"
                        detail_parts.append("observer inconclusive for this case")
                elif not rust_ok and not python_ok:
                    disp = "both_disagree_with_expectation"
                    detail_parts.append("both engines differ from expectation (majority is not truth)")
                elif not rust_ok:
                    disp = "rust_mismatch"
                else:
                    disp = "python_mismatch"
            else:
                disp = "engine_incomplete"
        dispositions.append(disp)
        per_case.append({
            "case_id": cid,
            "disposition": disp,
            "engine_status": {n: statuses.get(n, "missing") for n in engines},
            "detail": "; ".join(p for p in detail_parts if p),
        })

    aggregate = min(dispositions, key=_SEVERITY.index) if dispositions else "harness_failure"
    receipt = {
        "receipt_version": RECEIPT_VERSION,
        "profile": corpus.profile,
        "schema_version": corpus.schema_version,
        "corpus_digest": corpus.corpus_digest,
        "corpus_digest_algorithm": "sha256(canonical corpus JSON)",
        "expectation_digest": corpus.expectation_digest,
        "expectation_digest_algorithm": "sha256(canonical expectations JSON)",
        "selected_case_ids": list(corpus.selected_case_ids),
        "required_denominator": corpus.required_denominator,
        "components": components or {},
        "engine_identities": engine_identities,
        "counts": {"selected": len(corpus.selected_case_ids), **engine_counts},
        "per_case": per_case,
        "harness_controls_executed": harness_controls or [],
        "aggregate_disposition": aggregate,
    }
    # Semantic digest binds everything above; no timestamps/elapsed anywhere
    # in the receipt, so volatile telemetry cannot enter semantic identity.
    receipt["semantic_digest"] = sha256_hex(canonical_bytes(receipt))
    return receipt


def receipt_json_bytes(receipt: dict) -> bytes:
    return (json.dumps(receipt, indent=2, sort_keys=True, ensure_ascii=False) + "\n").encode("utf-8")


def receipt_markdown(receipt: dict) -> str:
    lines = [
        f"# Conformance receipt - {receipt['profile']}",
        "",
        f"- Receipt version: {receipt['receipt_version']}",
        f"- Schema version: {receipt['schema_version']}",
        f"- Corpus digest (sha256, canonical corpus JSON): `{receipt['corpus_digest']}`",
        f"- Expectation digest: `{receipt['expectation_digest']}`",
        f"- Semantic digest: `{receipt['semantic_digest']}`",
        f"- Aggregate disposition: **{receipt['aggregate_disposition']}**",
        f"- Selected cases ({receipt['counts']['selected']}): {', '.join(receipt['selected_case_ids'])}",
        "",
        "## Components",
        "",
    ]
    for name, comp in sorted((receipt.get("components") or {}).items()):
        lines.append(f"- {name}: {json.dumps(comp, sort_keys=True)}")
    for name, ident in sorted((receipt.get("engine_identities") or {}).items()):
        lines.append(f"- engine/{name}: {json.dumps(ident, sort_keys=True)}")
    lines += ["", "## Counts", ""]
    for name in sorted(receipt["counts"]):
        if name == "selected":
            continue
        c = receipt["counts"][name]
        lines.append(f"- {name}: selected={c['selected']} executed={c['executed']} ok={c['ok']} failed={c['failed']} missing={c['missing']}")
    lines += ["", "## Per-case dispositions", "", "| Case | Disposition | Engine statuses | Detail |", "|---|---|---|---|"]
    for row in receipt["per_case"]:
        statuses = ", ".join(f"{k}={v}" for k, v in sorted(row["engine_status"].items()))
        detail = (row["detail"] or "").replace("|", "\\|")
        lines.append(f"| {row['case_id']} | {row['disposition']} | {statuses} | {detail} |")
    lines.append("")
    return "\n".join(lines)


# ---------------------------------------------------------------- engines

def _git_commit(checkout) -> str:
    out = subprocess.run(["git", "-C", str(checkout), "rev-parse", "HEAD"],
                         capture_output=True, text=True, timeout=30)
    if out.returncode != 0:
        raise HarnessFailure(f"git rev-parse failed in {checkout}: {out.stderr.strip()}")
    return out.stdout.strip()


class RustDriverEngine:
    """Batched Rust engine: build once, one request for all cases."""

    name = "rust"

    def __init__(self, binary: Path):
        self.binary = Path(binary)
        self.binary_sha256 = sha256_hex(self.binary.read_bytes()) if self.binary.exists() else None

    @classmethod
    def build(cls, checkout) -> "RustDriverEngine":
        checkout = Path(checkout)
        proc = subprocess.run(
            ["cargo", "build", "-p", "adze-glr-core", "--example", "conformance_sets"],
            cwd=str(checkout), capture_output=True, text=True, timeout=1200)
        if proc.returncode != 0:
            raise EngineCrash(f"cargo build of conformance_sets failed: {proc.stderr[-2000:]}")
        exe = "conformance_sets.exe" if sys.platform == "win32" else "conformance_sets"
        binary = checkout / "target" / "debug" / "examples" / exe
        if not binary.exists():
            raise EngineCrash(f"built driver binary not found: {binary}")
        return cls(binary)

    def run(self, request, request_bytes):
        try:
            proc = subprocess.run([str(self.binary)], input=request_bytes,
                                  capture_output=True, timeout=300)
        except subprocess.TimeoutExpired as exc:
            raise EngineTimeout("rust driver timed out after 300s") from exc
        if proc.returncode != 0 and not proc.stdout:
            raise EngineCrash(f"rust driver exited {proc.returncode}: {proc.stderr.decode('utf-8', 'replace')[-500:]}")
        envelope = _parse_engine_payload(proc.stdout)  # ValueError -> harness_failure upstream
        if not isinstance(envelope, dict):
            raise HarnessFailure("rust driver output is not a JSON object")
        identity = {
            "engine": envelope.get("engine"),
            "engine_version": envelope.get("engine_version"),
            "driver": "glr-core/examples/conformance_sets.rs",
            "binary_sha256": self.binary_sha256,
        }
        return {
            "identity": identity,
            "input_digest": envelope.get("input_digest"),
            "input_digest_algorithm": envelope.get("input_digest_algorithm"),
            "results": envelope.get("results"),
        }


class PythonReferenceEngine:
    """Python engine: import ``adze_reference`` from the reference checkout."""

    name = "python"

    def __init__(self, pyref):
        pyref = Path(pyref)
        candidates = [pyref, pyref / "reference" / "python"]
        self.package_parent = next((c for c in candidates if (c / "adze_reference").is_dir()), None)
        if self.package_parent is None:
            raise HarnessFailure(f"adze_reference package not found under {pyref}")
        sys.path.insert(0, str(self.package_parent))
        try:
            import adze_reference  # noqa: F401  (imported from the checkout path only)
            self._mod = adze_reference
        finally:
            sys.path.remove(str(self.package_parent))
        self.identity = {
            "engine": "adze_reference",
            "package_parent": self.package_parent.name,
            "interpreter": platform.python_version(),
            "interpreter_implementation": platform.python_implementation(),
        }

    def run(self, request, request_bytes):
        sys.path.insert(0, str(self.package_parent))
        try:
            from adze_reference import try_compute
            rows = []
            for case in request["cases"]:
                doc = {"profile": request["profile"], "schema_version": request["schema_version"], **case}
                outcome = try_compute(doc)
                status_value = outcome.status.value
                if outcome.ok and outcome.result is not None:
                    rows.append({"case_id": case["case_id"], "status": "ok",
                                 "result": outcome.result.to_canonical_dict()})
                else:
                    mapped = {"invalid": "invalid_input", "bound_exceeded": "invalid_input",
                              "unsupported": "unsupported"}.get(status_value, "engine_error")
                    problem = outcome.problem
                    reason = f"{problem.code}: {problem.message}" if problem else status_value
                    rows.append({"case_id": case["case_id"], "status": mapped, "reason": reason})
            return {"identity": dict(self.identity),
                    "input_digest": sha256_hex(request_bytes),
                    "input_digest_algorithm": "sha256",
                    "results": rows}
        finally:
            sys.path.remove(str(self.package_parent))


# ---------------------------------------------------------------- CLI

def main(argv=None) -> int:
    import argparse

    parser = argparse.ArgumentParser(prog="conformance-comparator")
    parser.add_argument("--corpus", required=True, help="path to corpus.json")
    parser.add_argument("--pyref", required=True, help="Python reference checkout (or its reference/python dir)")
    parser.add_argument("--rust-checkout", required=True, help="Rust driver checkout root")
    parser.add_argument("--out-dir", required=True, help="directory for receipt.json / receipt.md")
    parser.add_argument("--no-build", action="store_true", help="reuse an already-built driver binary")
    args = parser.parse_args(argv)

    corpus_path = Path(args.corpus)
    corpus = load_corpus(corpus_path)
    rust_checkout = Path(args.rust_checkout)
    pyref = Path(args.pyref)

    corpus_commit = _git_commit(corpus_path.parent)
    python_commit = _git_commit(pyref)
    rust_commit = _git_commit(rust_checkout)

    if args.no_build:
        exe = "conformance_sets.exe" if sys.platform == "win32" else "conformance_sets"
        rust_engine = RustDriverEngine(rust_checkout / "target" / "debug" / "examples" / exe)
    else:
        rust_engine = RustDriverEngine.build(rust_checkout)
    python_engine = PythonReferenceEngine(pyref)

    components = {
        "corpus": {"commit": corpus_commit, "obtained_via": "git -C <corpus checkout> rev-parse HEAD",
                   "source": "conformance/cases/cfg.nullable_first_follow.v1/corpus.json"},
        "python_reference": {"commit": python_commit, "obtained_via": "git -C <python reference checkout> rev-parse HEAD",
                             "interpreter": platform.python_version(),
                             "interpreter_implementation": platform.python_implementation(),
                             "obtained_via_interpreter": "platform.python_version()"},
        "rust_candidate": {"commit": rust_commit, "obtained_via": "git -C <rust driver checkout> rev-parse HEAD",
                           "driver": "glr-core/examples/conformance_sets.rs",
                           "build": "cargo build -p adze-glr-core --example conformance_sets (once, batched request)"},
    }
    receipt = run_comparison(
        corpus,
        {"rust": rust_engine, "python": python_engine},
        components=components,
        harness_controls=["full required denominator selected before engine invocation",
                          "expectation keys guarded out of engine requests"],
    )
    out_dir = Path(args.out_dir)
    out_dir.mkdir(parents=True, exist_ok=True)
    (out_dir / "receipt.json").write_bytes(receipt_json_bytes(receipt))
    (out_dir / "receipt.md").write_text(receipt_markdown(receipt), encoding="utf-8")
    counts = receipt["counts"]
    print(f"aggregate_disposition={receipt['aggregate_disposition']}")
    print(f"selected={counts['selected']} rust={counts.get('rust')} python={counts.get('python')}")
    print(f"receipt written to {out_dir}")
    return 0 if receipt["aggregate_disposition"] == "agrees" else 1


if __name__ == "__main__":
    raise SystemExit(main())
