#!/usr/bin/env python3
"""Advisory PR Plan for adze.

Computes a static, label/path-driven CI plan for the current PR and emits
``target/ci/ci-plan.json`` plus a Markdown step summary. Intentionally simple
and dependency-free (stdlib only); the testable, cargo-graph-aware planner
lives in ``xtask ci plan`` (see ``docs/ci/adze-rollout-plan.md`` PR 09).

Inputs (env / argv):
  - BASE_SHA           base of the diff (defaults to merge-base with origin/main)
  - HEAD_SHA           head of the diff (defaults to HEAD)
  - PR_LABELS_JSON     JSON array of label names (optional)
  - GITHUB_STEP_SUMMARY path to the step summary file (optional)
  - --json-out PATH    write machine plan to PATH (default target/ci/ci-plan.json)

The plan is advisory: it only reports which lanes would run if routing PRs
were live. It never blocks merge.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import shutil
import subprocess
import sys
from pathlib import Path
from typing import Iterable

# ---------------------------------------------------------------------------
# Area classifier — adze-specific.
# ---------------------------------------------------------------------------

AREAS: dict[str, list[str]] = {
    "docs": [
        r"^docs/",
        r"^book/",
        r"^\.adze/goals/",
        r"\.md$",
        r"^README",
        r"^CHANGELOG",
    ],
    "workflow": [
        r"^\.github/workflows/",
        r"^policy/",
        r"^scripts/",
        r"^justfile$",
        r"^\.githooks/",
        r"^xtask/",
    ],
    "core_runtime": [
        r"^runtime/",
        r"^runtime2/",
        r"^common/",
        r"^ir/",
        r"^glr-core/",
        r"^tablegen/",
        r"^macro/",
        r"^tool/",
        r"^cli/",
    ],
    "microcrate": [
        r"^crates/",
    ],
    "parser": [
        r"^glr-core/",
        r"^crates/parser-",
        r"^crates/grammar-",
        r"^crates/parsetable-metadata/",
        r"^crates/linecol-core/",
    ],
    "grammar": [
        r"^grammars/",
        r"^golden-tests/",
        r"^corpus/",
    ],
    "tablegen": [
        r"^tablegen/",
        r"^crates/parsetable-metadata/",
    ],
    "governance": [
        r"^crates/bdd-governance-core/",
        r"^tests/governance/",
    ],
    "concurrency": [
        r"concurrency_caps",
    ],
    "wasm": [
        r"^wasm-demo/",
        r"^runtime/wasm",
        r"^playground/",
    ],
    "performance": [
        r"^benchmarks/",
        r"^baselines/",
        r"PERFORMANCE",
    ],
    "manifest": [
        r"^Cargo\.toml$",
        r"^Cargo\.lock$",
        r"^rust-toolchain\.toml$",
        r"^deny\.toml$",
    ],
}

# ---------------------------------------------------------------------------
# Risk pack mapping. Mirrors policy/ci-risk-packs.toml at a high level so the
# Python advisory plan agrees with the canonical xtask planner. Keep in sync
# until xtask ci plan is wired in (PR 09).
# ---------------------------------------------------------------------------

RISK_PACKS: dict[str, dict[str, object]] = {
    "core_runtime": {
        "areas": ["core_runtime"],
        "lanes": ["ci-supported", "ripr-advisory"],
        "deep_lanes": ["pure-rust-os-matrix", "product-proof-advisory"],
        "labels": ["full-ci", "coverage"],
    },
    "glr_core": {
        "areas": ["parser"],
        "paths": [r"^glr-core/"],
        "lanes": ["ci-supported", "ripr-advisory"],
        "deep_lanes": ["fuzz-pr", "performance-regression"],
        "labels": ["mutation", "property-tests", "ci:perf"],
    },
    "tablegen": {
        "areas": ["tablegen"],
        "lanes": ["ci-supported", "ripr-advisory"],
        "deep_lanes": ["product-proof-advisory", "benchmarks-pr"],
        "labels": ["full-ci", "ci:perf"],
    },
    "grammar_golden": {
        "areas": ["grammar"],
        "lanes": ["golden-tests", "ripr-advisory"],
        "deep_lanes": ["product-proof-advisory"],
        "labels": ["ci:golden", "full-ci"],
    },
    "microcrate_governance": {
        "areas": ["governance"],
        "lanes": ["microcrate-ci"],
        "deep_lanes": [],
        "labels": ["ci:microcrate", "full-ci"],
    },
    "concurrency": {
        "areas": ["concurrency"],
        "lanes": ["ci-supported"],
        "deep_lanes": [],
        "labels": ["ci:concurrency", "full-ci"],
    },
    "wasm": {
        "areas": ["wasm"],
        "lanes": [],
        "deep_lanes": ["pure-rust-os-matrix"],
        "labels": ["wasm", "full-ci"],
    },
    "performance": {
        "areas": ["performance"],
        "lanes": ["criterion-smoke"],
        "deep_lanes": ["performance-regression", "benchmarks-pr"],
        "labels": ["ci:perf", "full-ci"],
    },
    "manifest_release": {
        "areas": ["manifest"],
        "lanes": ["ci-supported"],
        "deep_lanes": [],
        "labels": ["security-audit", "release-check", "full-ci"],
    },
}

# Static fallback LEM table — keep in sync with policy/ci-lane-whitelist.toml
# `base_lem` field. The xtask planner replaces this with the real table.
LANE_LEM: dict[str, int] = {
    "pr-plan": 1,
    "pr-gate-success": 1,
    "ci-supported": 20,
    "ci-lane-whitelist-lint": 2,
    "ripr-advisory": 4,
    "test-policy": 2,
    "core-tests": 18,
    "criterion-smoke": 6,
    "smoke-ts-bridge": 4,
    "ts-bridge-smoke": 4,
    "ts-bridge-parity": 8,
    "clippy-quarantine-report": 4,
    "ci-main": 22,
    "golden-tests": 20,
    "microcrate-ci": 45,
    "fuzz-pr": 45,
    "performance-regression": 35,
    "benchmarks-pr": 35,
    "pure-rust-os-matrix": 180,
    "product-proof-advisory": 25,
}

DEFAULT_LANES = [
    "pr-plan",
    "ci-supported",
    "ripr-advisory",
    "ci-lane-whitelist-lint",
]

DEEP_LABELS = {
    "full-ci": "all deep lanes",
    "platform-matrix": "pure-rust-os-matrix",
    "fuzz": "fuzz-pr",
    "ci:perf": "performance-regression / benchmarks-pr",
    "ci:golden": "golden-tests",
    "ci:microcrate": "microcrate-ci",
    "ci:concurrency": "microcrate-ci",
    "wasm": "pure-rust-os-matrix (wasm path)",
    "coverage": "coverage",
    "release-check": "release",
    "security-audit": "security",
}

BUDGET_BANDS = [
    (35, "ordinary"),
    (75, "elevated"),
    (125, "high"),
    (10**9, "over-ceiling"),
]


def run(cmd: list[str]) -> str:
    return subprocess.check_output(cmd, text=True).strip()


def changed_files(base: str, head: str) -> list[str]:
    if not base or not head:
        return []
    try:
        out = run(["git", "diff", "--name-only", f"{base}...{head}"])
    except subprocess.CalledProcessError:
        return []
    return [line for line in out.splitlines() if line]


def merge_base() -> str:
    try:
        return run(["git", "merge-base", "origin/main", "HEAD"])
    except subprocess.CalledProcessError:
        return ""


def classify_areas(files: Iterable[str]) -> set[str]:
    hits: set[str] = set()
    for f in files:
        for area, patterns in AREAS.items():
            for pat in patterns:
                if re.search(pat, f):
                    hits.add(area)
                    break
    return hits


def select_risk_packs(areas: set[str], files: list[str], labels: list[str]) -> list[str]:
    selected: list[str] = []
    for name, pack in RISK_PACKS.items():
        pack_areas = set(pack.get("areas", []))  # type: ignore[arg-type]
        if pack_areas & areas:
            selected.append(name)
            continue
        for pat in pack.get("paths", []):  # type: ignore[union-attr]
            if any(re.search(pat, f) for f in files):
                selected.append(name)
                break
        if name in selected:
            continue
        if any(lbl in pack.get("labels", []) for lbl in labels):  # type: ignore[union-attr]
            selected.append(name)
    return selected


def select_lanes(areas: set[str], packs: list[str], labels: list[str]) -> list[dict[str, object]]:
    chosen: dict[str, dict[str, object]] = {}

    def add(lane_id: str, blocking: bool, reason: str) -> None:
        if lane_id not in chosen:
            chosen[lane_id] = {
                "id": lane_id,
                "lem": LANE_LEM.get(lane_id, 5),
                "blocking": blocking,
                "reason": reason,
            }

    for lane in DEFAULT_LANES:
        add(lane, blocking=lane == "ci-supported", reason="default frontdoor")

    for pack in packs:
        for lane in RISK_PACKS[pack].get("lanes", []):  # type: ignore[union-attr]
            add(str(lane), blocking=False, reason=f"risk pack: {pack}")
        if "full-ci" in labels:
            for lane in RISK_PACKS[pack].get("deep_lanes", []):  # type: ignore[union-attr]
                add(str(lane), blocking=False, reason=f"risk pack: {pack} (full-ci)")

    if "full-ci" in labels:
        add("pure-rust-os-matrix", blocking=False, reason="label: full-ci")
        add("fuzz-pr", blocking=False, reason="label: full-ci")
        add("benchmarks-pr", blocking=False, reason="label: full-ci")

    if "platform-matrix" in labels:
        add("pure-rust-os-matrix", blocking=False, reason="label: platform-matrix")
    if "fuzz" in labels:
        add("fuzz-pr", blocking=False, reason="label: fuzz")
    if "ci:perf" in labels:
        add("performance-regression", blocking=False, reason="label: ci:perf")
        add("benchmarks-pr", blocking=False, reason="label: ci:perf")
    if "ci:golden" in labels:
        add("golden-tests", blocking=False, reason="label: ci:golden")

    return list(chosen.values())


def band_for(lem: int) -> str:
    for limit, name in BUDGET_BANDS:
        if lem <= limit:
            return name
    return "over-ceiling"


def make_plan(base: str, head: str, labels: list[str]) -> dict[str, object]:
    files = changed_files(base, head)
    areas = classify_areas(files)
    packs = select_risk_packs(areas, files, labels)
    lanes = select_lanes(areas, packs, labels)
    total = sum(int(l["lem"]) for l in lanes)  # type: ignore[arg-type]
    return {
        "schema_version": 1,
        "repo": "adze",
        "posture": "rust",
        "base": base,
        "head": head,
        "labels": labels,
        "changed": {"files": files, "areas": sorted(areas)},
        "selection": {"risk_packs": packs, "lanes": lanes},
        "budget": {"estimated_lem": total, "band": band_for(total)},
        "notes": [
            "Advisory fallback plan. The canonical planner is `cargo run -q -p xtask -- ci-plan`.",
            "Static lane LEM values are mirrored from policy/ci-lane-whitelist.toml.",
        ],
    }


def write_summary(plan: dict[str, object], summary_path: str) -> None:
    if not summary_path:
        return
    band = plan["budget"]["band"]  # type: ignore[index]
    lem = plan["budget"]["estimated_lem"]  # type: ignore[index]
    icon = {"ordinary": "✅", "elevated": "⚠️", "high": "🟠", "over-ceiling": "🛑"}.get(str(band), "·")
    lines: list[str] = [
        "## PR Plan (advisory)",
        "",
        f"{icon} **Estimated LEM:** {lem} ({band})",
        "",
        "### Changed areas",
        "",
        "- " + ", ".join(plan["changed"]["areas"]) if plan["changed"]["areas"] else "- (none)",  # type: ignore[index]
        "",
        "### Risk packs",
        "",
        "- " + ", ".join(plan["selection"]["risk_packs"]) if plan["selection"]["risk_packs"] else "- (none)",  # type: ignore[index]
        "",
        "### Selected lanes",
        "",
        "| lane | LEM | blocking | reason |",
        "| --- | ---: | :---: | --- |",
    ]
    for lane in plan["selection"]["lanes"]:  # type: ignore[index]
        blocking = "yes" if lane.get("blocking") else "no"  # type: ignore[union-attr]
        lines.append(
            f"| `{lane['id']}` | {lane['lem']} | {blocking} | {lane['reason']} |"  # type: ignore[index]
        )
    lines += ["", "_See docs/ci/cost-and-verification-policy.md._", ""]

    with open(summary_path, "a", encoding="utf-8") as fh:
        fh.write("\n".join(lines) + "\n")


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--base", default=os.environ.get("BASE_SHA", ""))
    ap.add_argument("--head", default=os.environ.get("HEAD_SHA", "HEAD"))
    ap.add_argument("--labels-json", default=os.environ.get("PR_LABELS_JSON", "[]"))
    ap.add_argument("--json-out", default="target/ci/ci-plan.json")
    ap.add_argument("--summary", default=os.environ.get("GITHUB_STEP_SUMMARY", ""))
    args = ap.parse_args()

    base = args.base or merge_base()
    try:
        labels = list(json.loads(args.labels_json or "[]"))
    except json.JSONDecodeError:
        labels = []

    plan = make_plan(base, args.head, [str(l) for l in labels])

    out_path = Path(args.json_out)
    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(json.dumps(plan, indent=2) + "\n", encoding="utf-8")

    write_summary(plan, args.summary)

    band = plan["budget"]["band"]  # type: ignore[index]
    lem = plan["budget"]["estimated_lem"]  # type: ignore[index]
    print(f"PR Plan: ~{lem} LEM ({band}); plan written to {out_path}")
    if shutil.which("jq"):
        print(f"Inspect: jq . {out_path}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
