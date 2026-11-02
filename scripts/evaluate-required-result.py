#!/usr/bin/env python3
"""Fail-closed evaluation for the two required result checks (issue #995).

GitHub treats a skipped required check as satisfied, so the result jobs
must run for every admitted candidate and decide Pass/Fail themselves.

Truth table (both kinds):
  explicit valid no-work decision (detector success + selector ``false``
  + non-empty reason)                            -> Pass
  selected proof succeeds                        -> Pass
  detector fails / is skipped / cancelled / absent -> Fail
  selector missing or malformed (not ``true``/``false``) -> Fail
  selected job skipped / cancelled / absent      -> Fail
  selected job fails                             -> Fail
  route unavailable (router not successful, no/unknown target,
  hosted target without fallback allowance)      -> Fail
  result output absent or truncated (empty/short selector, target,
  reason, or job result)                         -> Fail

Inputs arrive only via environment variables (job results and outputs
passed as data, never shell-interpolated). Exit 0 = Pass, 1 = Fail.
"""

from __future__ import annotations

import os
import sys

SUCCESS = "success"


def env(name: str) -> str:
    return os.environ.get(name, "")


def fail(message: str) -> int:
    print(f"::error::{message}")
    return 1


def passes(message: str) -> int:
    print(message)
    return 0


def check_detector() -> str | None:
    """Return an error message unless detection succeeded."""
    detect = env("DETECT_RESULT")
    print(f"detect_result={detect}")
    if detect != SUCCESS:
        return f"Path detection did not succeed (result={detect!r}); required proof cannot be verified."
    return None


def evaluate_rust() -> int:
    event = env("EVENT_NAME")
    rust_needed = env("RUST_NEEDED")
    reason = env("DETECT_REASON")
    print(f"mode={'pull-request-admission' if event == 'pull_request' else 'full-qualification'}")
    print(f"rust_needed={rust_needed}")
    print(f"reason={reason}")

    if (err := check_detector()) is not None:
        return fail(err)

    # Explicit valid no-work is the only selector value other than
    # ``true`` that may pass, and only with a recorded reason.
    if rust_needed == "false":
        if not reason.strip():
            return fail("No-work decision has no reason output; selector output is absent or truncated.")
        return passes(f"::notice::Rust lanes skipped by explicit no-work decision: {reason}")
    if rust_needed != "true":
        return fail(f"Rust selector is missing or malformed (rust_needed={rust_needed!r}).")

    if event == "pull_request":
        admission = env("PR_ADMISSION_RESULT")
        print(f"pr_admission_result={admission}")
        if admission != SUCCESS:
            return fail(f"Selected PR admission job did not succeed (result={admission!r}).")
        return passes("PR admission proof succeeded.")

    route_result = env("ROUTE_RESULT")
    target = env("TARGET")
    fallback_allowed = env("FALLBACK_ALLOWED")
    supported = env("SUPPORTED_RESULT")
    print(f"route_result={route_result}")
    print(f"target={target}")
    print(f"target_reason={env('TARGET_REASON')}")
    print(f"fallback_allowed={fallback_allowed}")
    print(f"supported_result={supported}")
    if route_result != SUCCESS:
        return fail(f"Route unavailable: router did not succeed (result={route_result!r}).")

    lanes = {
        "cpx42": env("CPX42_RESULT"),
        "cx43": env("CX43_RESULT"),
        "cx33": env("CX33_RESULT"),
        "cx53": env("CX53_RESULT"),
        "github": env("GITHUB_RESULT"),
    }
    for name, result in lanes.items():
        print(f"{name}={result}")

    if target not in lanes:
        return fail(f"Route unavailable: missing or unknown routing target ({target!r}).")
    if target == "github" and fallback_allowed != "true":
        return fail(
            "Route unavailable: GitHub-hosted target selected but fallback "
            "was not allowed; the required gate cannot verify this candidate."
        )

    selected = lanes[target]
    if selected != SUCCESS:
        return fail(f"Selected Rust lane {target!r} did not succeed (result={selected!r}).")
    if supported != SUCCESS:
        return fail(f"Selected supported proof did not succeed (result={supported!r}).")
    return passes(f"Selected Rust lane {target!r} and supported proof succeeded.")


def evaluate_product() -> int:
    event = env("EVENT_NAME")
    run_stable = env("RUN_STABLE")
    reason = env("DETECT_REASON")
    print(f"mode={'pull-request-admission' if event == 'pull_request' else 'full-qualification'}")
    print(f"run_stable={run_stable}")
    print(f"reason={reason}")

    if (err := check_detector()) is not None:
        return fail(err)

    # Explicit valid no-work is the only selector value other than
    # ``true`` that may pass, and only with a recorded reason.
    if run_stable == "false":
        if not reason.strip():
            return fail("No-work decision has no reason output; selector output is absent or truncated.")
        return passes(f"::notice::Stable Product Proof canaries skipped by explicit no-work decision: {reason}")
    if run_stable != "true":
        return fail(f"Product selector is missing or malformed (run_stable={run_stable!r}).")

    if event == "pull_request":
        plan = env("PR_CANARY_RESULT")
        print(f"pr_canary_result={plan}")
        if plan != SUCCESS:
            return fail(f"Selected PR canary plan did not succeed (result={plan!r}).")
        return passes("PR canary plan proof succeeded.")

    stable = env("STABLE_RESULT")
    print(f"stable_result={stable}")
    if stable != SUCCESS:
        return fail(f"Selected stable product proof did not succeed (result={stable!r}).")
    return passes("Selected stable product proof succeeded.")


def main(argv: list[str]) -> int:
    if len(argv) != 2 or argv[1] not in {"rust", "product"}:
        print("usage: evaluate-required-result.py {rust|product}", file=sys.stderr)
        return 1
    return evaluate_rust() if argv[1] == "rust" else evaluate_product()


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
