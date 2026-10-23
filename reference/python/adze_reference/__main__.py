"""Thin runner: read one engine-request JSON file, print canonical result JSON."""
from __future__ import annotations

import json
import sys
from pathlib import Path

from .engine import try_compute


def main(argv: list[str]) -> int:
    if len(argv) != 2:
        print("usage: python -m adze_reference <request.json>", file=sys.stderr)
        return 2
    doc = json.loads(Path(argv[1]).read_text(encoding="utf-8"))
    outcome = try_compute(doc)
    if outcome.ok and outcome.result is not None:
        sys.stdout.write(outcome.result.canonical_json().decode("utf-8") + "\n")
        return 0
    assert outcome.problem is not None
    sys.stdout.write(json.dumps(
        {"status": outcome.problem.status.value, "code": outcome.problem.code,
         "message": outcome.problem.message}, sort_keys=True) + "\n")
    return 1


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
