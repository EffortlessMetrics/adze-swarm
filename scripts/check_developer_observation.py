#!/usr/bin/env python3
"""Validate #1140 observation structure, never parser correctness or adoption."""
from __future__ import annotations

import argparse
from datetime import datetime
import json
import math
from pathlib import Path
import re
import sys
from typing import Any

TASK_IDS = tuple(f"D{number:02}" for number in range(1, 9))
OUTCOMES = {"not_attempted", "blocked", "assisted", "independent"}
SCHEMA = "adze.developer-observation.v1"
MAX_BYTES = 1_048_576


class InvalidObservation(ValueError):
    """The observation is incomplete or contradicts its declared outcome."""


def require(condition: bool, message: str) -> None:
    if not condition:
        raise InvalidObservation(message)


def text(value: Any) -> bool:
    return isinstance(value, str) and bool(value.strip())


def strings(value: Any) -> bool:
    return isinstance(value, list) and all(text(item) for item in value)


def keys(value: Any, expected: set[str], where: str) -> None:
    require(isinstance(value, dict), f"{where}: expected object")
    require(set(value) == expected, f"{where}: missing or unknown fields")


def timestamp(value: Any, where: str) -> datetime:
    require(text(value), f"{where}: timezone-aware timestamp required")
    try:
        parsed = datetime.fromisoformat(value.replace("Z", "+00:00"))
    except ValueError as error:
        raise InvalidObservation(f"{where}: invalid timestamp") from error
    require(parsed.utcoffset() is not None, f"{where}: timezone required")
    return parsed


def validate(observation: Any) -> None:
    keys(observation, {
        "schema", "packet_version", "source_sha", "docs_sha", "participant_kind",
        "participant_code", "consent_recorded", "platform", "toolchain",
        "started_at", "finished_at", "tasks",
    }, "observation")
    require(observation["schema"] == SCHEMA, "unsupported observation schema")
    require(observation["packet_version"] == 1 and type(observation["packet_version"]) is int,
            "packet_version must be integer 1")
    for field in ("source_sha", "docs_sha"):
        require(isinstance(observation[field], str)
                and re.fullmatch(r"[0-9a-f]{40}", observation[field]) is not None,
                f"{field}: exact lowercase 40-character commit required")
    kind = observation["participant_kind"]
    require(kind in ("agent", "human"), "participant_kind must be agent or human")
    require(type(observation["consent_recorded"]) is bool, "consent_recorded must be boolean")
    require(observation["consent_recorded"] == (kind == "human"),
            "human observations require consent; agent runs must not claim human consent")
    for field in ("participant_code", "platform", "toolchain"):
        require(text(observation[field]), f"{field}: nonempty value required")
    start = timestamp(observation["started_at"], "started_at")
    end = timestamp(observation["finished_at"], "finished_at")
    require(end >= start, "finished_at precedes started_at")
    duration = (end - start).total_seconds()
    tasks = observation["tasks"]
    require(isinstance(tasks, list) and len(tasks) == len(TASK_IDS), "all eight task rows are required")
    seen: set[str] = set()
    for task in tasks:
        keys(task, {
            "id", "outcome", "assistance", "evidence", "observed_result",
            "elapsed_seconds", "blocker", "failed_commands", "confusing_messages", "owner",
        }, "task")
        task_id = task["id"]
        require(isinstance(task_id, str) and task_id in TASK_IDS, "unknown task ID")
        require(task_id not in seen, f"{task_id}: duplicate task")
        seen.add(task_id)
        outcome = task["outcome"]
        require(isinstance(outcome, str) and outcome in OUTCOMES, f"{task_id}: unknown outcome")
        for field in ("assistance", "evidence", "failed_commands", "confusing_messages"):
            require(strings(task[field]), f"{task_id}.{field}: list of nonempty strings required")
        for field in ("observed_result", "blocker", "owner"):
            require(isinstance(task[field], str), f"{task_id}.{field}: string required")
        elapsed = task["elapsed_seconds"]
        require(elapsed is None or (
            (type(elapsed) is int or (type(elapsed) is float and math.isfinite(elapsed)))
            and 0 <= elapsed <= duration
        ), f"{task_id}: elapsed time must be finite, nonnegative and within the recorded run")
        if outcome in ("independent", "assisted"):
            require(bool(task["evidence"]) and text(task["observed_result"]),
                    f"{task_id}: completed tasks need evidence references and observed values")
            require(elapsed is not None, f"{task_id}: completed task needs elapsed time")
            require(not task["blocker"].strip(), f"{task_id}: completed task cannot retain an unresolved blocker")
        if outcome == "independent":
            require(not task["assistance"], f"{task_id}: assistance contradicts independent completion")
        elif outcome == "assisted":
            require(bool(task["assistance"]), f"{task_id}: assisted completion needs an assistance log")
        elif outcome == "blocked":
            require(text(task["blocker"]) and text(task["owner"]), f"{task_id}: blocker and owning issue required")
        else:
            require(text(task["blocker"]), f"{task_id}: record why the task was not attempted")
            require(elapsed is None and not task["evidence"] and not task["observed_result"].strip()
                    and not task["assistance"] and not task["failed_commands"] and not task["confusing_messages"],
                    f"{task_id}: not_attempted cannot contain execution or completion evidence")
    require(seen == set(TASK_IDS), "task denominator changed")


def unique_object(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        require(key not in result, f"duplicate JSON key: {key}")
        result[key] = value
    return result


def load(path: Path) -> Any:
    with path.open("rb") as stream:
        data = stream.read(MAX_BYTES + 1)
    require(len(data) <= MAX_BYTES, "observation exceeds 1 MiB limit")
    return json.loads(data.decode("utf-8"), object_pairs_hook=unique_object)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("observation", type=Path)
    arguments = parser.parse_args(argv)
    try:
        validate(load(arguments.observation))
    except (OSError, UnicodeError, ValueError, RecursionError) as error:
        print(f"Invalid observation: {error}", file=sys.stderr)
        return 2
    print("Observation structure valid. Outcomes remain reported observations, not verified product proof.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
