#!/usr/bin/env python3
"""Required-result and product-path contract tests (#995, #1027).

Job outcomes are simulated as environment inputs; path fixtures execute the
workflow detector over real Git commits. No Rust build or hosted run is needed.
Run: python3 scripts/test_evaluate_required_result.py
"""

from __future__ import annotations

import os
import subprocess
import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SCRIPT = ROOT / "scripts" / "evaluate-required-result.py"

RUST_PASS = {
    "EVENT_NAME": "merge_group",
    "DETECT_RESULT": "success",
    "RUST_NEEDED": "true",
    "DETECT_REASON": "event:merge_group",
    "ROUTE_RESULT": "success",
    "TARGET": "github",
    "TARGET_REASON": "merge_group_github_hosted",
    "FALLBACK_ALLOWED": "true",
    "SUPPORTED_RESULT": "success",
    "CPX42_RESULT": "skipped",
    "CX43_RESULT": "skipped",
    "CX33_RESULT": "skipped",
    "CX53_RESULT": "skipped",
    "GITHUB_RESULT": "success",
    "PR_ADMISSION_RESULT": "skipped",
}
PRODUCT_PASS = {
    "EVENT_NAME": "merge_group",
    "DETECT_RESULT": "success",
    "RUN_STABLE": "true",
    "DETECT_REASON": "stable_surface:runtime/lib.rs",
    "STABLE_RESULT": "success",
    "WINDOWS_RESULT": "success",
    "PR_CANARY_RESULT": "skipped",
}


def run(kind: str, env: dict[str, str]) -> int:
    clean = {k: v for k, v in os.environ.items() if k not in RUST_PASS and k not in PRODUCT_PASS}
    return subprocess.run(
        [sys.executable, str(SCRIPT), kind],
        env={**clean, **env},
        capture_output=True,
        text=True,
        check=False,
    ).returncode


def rust(**overrides: str) -> int:
    return run("rust", {**RUST_PASS, **overrides})


def product(**overrides: str) -> int:
    return run("product", {**PRODUCT_PASS, **overrides})


class ValidPassRows(unittest.TestCase):
    def test_valid_no_work_passes(self) -> None:
        self.assertEqual(0, rust(RUST_NEEDED="false", DETECT_REASON="no_rust_surface_changed"))
        self.assertEqual(0, product(RUN_STABLE="false", DETECT_REASON="no_stable_product_surface_changed"))
        self.assertEqual(0, product(RUN_STABLE="false", DETECT_REASON="no_stable_product_surface_changed",
                                    STABLE_RESULT="skipped", WINDOWS_RESULT="skipped"))
        self.assertEqual(0, product(RUN_STABLE="false", DETECT_REASON="no_stable_product_surface_changed",
                                    STABLE_RESULT="", WINDOWS_RESULT=""))

    def test_selected_proof_succeeds_passes(self) -> None:
        self.assertEqual(0, rust())
        self.assertEqual(0, product())
        for target, key in [
            ("cpx42", "CPX42_RESULT"), ("cx43", "CX43_RESULT"),
            ("cx33", "CX33_RESULT"), ("cx53", "CX53_RESULT"),
        ]:
            with self.subTest(target=target):
                self.assertEqual(0, rust(TARGET=target, GITHUB_RESULT="skipped", **{key: "success"}))

    def test_pull_request_admission_passes(self) -> None:
        self.assertEqual(0, rust(EVENT_NAME="pull_request", PR_ADMISSION_RESULT="success",
                                 SUPPORTED_RESULT="skipped", GITHUB_RESULT="skipped"))
        self.assertEqual(0, product(EVENT_NAME="pull_request", PR_CANARY_RESULT="success",
                                    STABLE_RESULT="skipped", WINDOWS_RESULT="skipped"))
        self.assertEqual(0, product(EVENT_NAME="pull_request", PR_CANARY_RESULT="success",
                                    STABLE_RESULT="", WINDOWS_RESULT=""))

    def test_each_selected_non_pr_event_requires_and_accepts_windows_success(self) -> None:
        for event in ("merge_group", "workflow_dispatch", "schedule"):
            with self.subTest(event=event):
                self.assertEqual(0, product(EVENT_NAME=event))


class FailRows(unittest.TestCase):
    def test_detector_fails(self) -> None:
        for bad in ["failure", "cancelled", "skipped", ""]:
            with self.subTest(bad=bad):
                self.assertEqual(1, rust(DETECT_RESULT=bad))
                self.assertEqual(1, product(DETECT_RESULT=bad))

    def test_selector_missing_or_malformed(self) -> None:
        for bad in ["", "tru", "True", "yes", " true"]:
            with self.subTest(bad=bad):
                self.assertEqual(1, rust(RUST_NEEDED=bad))
                self.assertEqual(1, product(RUN_STABLE=bad))

    def test_pull_request_selector_validated_before_pr_branch(self) -> None:
        pr = {"EVENT_NAME": "pull_request", "PR_CANARY_RESULT": "success", "STABLE_RESULT": "skipped"}
        self.assertEqual(1, product(**pr, RUN_STABLE=""))
        self.assertEqual(1, product(**pr, RUN_STABLE="garbage"))
        self.assertEqual(1, product(**pr, RUN_STABLE="false", DETECT_REASON=""))

    def test_selected_job_skipped_or_cancelled(self) -> None:
        for bad in ["skipped", "cancelled"]:
            with self.subTest(bad=bad):
                self.assertEqual(1, rust(GITHUB_RESULT=bad))
                self.assertEqual(1, rust(SUPPORTED_RESULT=bad))
                self.assertEqual(1, product(STABLE_RESULT=bad))
                self.assertEqual(1, rust(EVENT_NAME="pull_request", PR_ADMISSION_RESULT=bad))
                self.assertEqual(1, product(EVENT_NAME="pull_request", PR_CANARY_RESULT=bad))

    def test_selected_job_fails(self) -> None:
        self.assertEqual(1, rust(GITHUB_RESULT="failure"))
        self.assertEqual(1, rust(SUPPORTED_RESULT="failure"))
        self.assertEqual(1, product(STABLE_RESULT="failure"))
        self.assertEqual(1, rust(EVENT_NAME="pull_request", PR_ADMISSION_RESULT="failure"))
        self.assertEqual(1, product(EVENT_NAME="pull_request", PR_CANARY_RESULT="failure"))

    def test_selected_windows_proof_missing_skipped_cancelled_failed_or_malformed(self) -> None:
        for event in ("merge_group", "workflow_dispatch", "schedule"):
            for bad in ("", "skipped", "cancelled", "failure", "succ", "Success", "success ", "success\n"):
                with self.subTest(event=event, windows_result=bad):
                    self.assertEqual(1, product(EVENT_NAME=event, WINDOWS_RESULT=bad))
            omitted = {key: value for key, value in PRODUCT_PASS.items() if key != "WINDOWS_RESULT"}
            omitted["EVENT_NAME"] = event
            with self.subTest(event=event, windows_result="omitted"):
                self.assertEqual(1, run("product", omitted))

    def test_route_unavailable(self) -> None:
        for bad in ["failure", "cancelled", "skipped", ""]:
            with self.subTest(route=bad):
                self.assertEqual(1, rust(ROUTE_RESULT=bad))
        self.assertEqual(1, rust(TARGET="github", FALLBACK_ALLOWED="false"))
        self.assertEqual(1, rust(TARGET="github", FALLBACK_ALLOWED=""))
        self.assertEqual(1, rust(TARGET="pr"))  # admission pseudo-target on a qualification event
        self.assertEqual(1, rust(TARGET="nowhere"))

    def test_result_output_absent_or_truncated(self) -> None:
        self.assertEqual(1, rust(RUST_NEEDED="false", DETECT_REASON=""))
        self.assertEqual(1, product(RUN_STABLE="false", DETECT_REASON=""))
        self.assertEqual(1, rust(TARGET=""))
        self.assertEqual(1, rust(TARGET="gith"))
        self.assertEqual(1, rust(GITHUB_RESULT=""))
        self.assertEqual(1, rust(GITHUB_RESULT="succ"))
        self.assertEqual(1, product(STABLE_RESULT=""))
        self.assertEqual(1, product(STABLE_RESULT="succ"))


class WorkflowWiring(unittest.TestCase):
    def _job_block(self, path: str, job: str) -> str:
        import re

        text = (ROOT / path).read_text(encoding="utf-8")
        start = text.index(f"\n  {job}:")
        rest = text[start + 1:]
        match = re.search(r"\n  (?=\S)", rest[5:])
        return rest if match is None else rest[: match.start() + 5]

    def _product_path_selection(
        self, path: str, *, event: str = "merge_group", change: str = "modify"
    ) -> dict[str, str]:
        """Execute the workflow's actual detector over one real Git change."""
        import tempfile
        import textwrap

        text = (ROOT / ".github/workflows/product-proof.yml").read_text(encoding="utf-8")
        paths = text.split("  STABLE_PRODUCT_PATHS: |\n", 1)[1].split("\nconcurrency:", 1)[0]
        block = self._job_block(".github/workflows/product-proof.yml", "detect-stable-product-paths")
        detector = textwrap.dedent(block.split("        run: |\n", 1)[1])
        with tempfile.TemporaryDirectory(prefix="adze-required-detector-") as directory:
            repo = Path(directory) / "repo"
            repo.mkdir()

            def git(*args: str) -> str:
                return subprocess.check_output(
                    ["git", "-c", "user.name=Guard fixture", "-c", "user.email=fixture@example.invalid", *args],
                    cwd=repo, text=True, stderr=subprocess.PIPE,
                ).strip()

            git("init", "--quiet")
            source = repo / path
            source.parent.mkdir(parents=True, exist_ok=True)
            source.write_text("before\n", encoding="utf-8")
            git("add", ".")
            git("commit", "--quiet", "-m", "base")
            base = git("rev-parse", "HEAD")
            if change == "modify":
                source.write_text("after\n", encoding="utf-8")
            elif change == "delete":
                source.unlink()
            elif change == "rename":
                destination = repo / "docs/retired-source.txt"
                destination.parent.mkdir(parents=True, exist_ok=True)
                git("mv", path, "docs/retired-source.txt")
            else:
                self.fail(f"unsupported fixture change: {change}")
            git("add", "-A")
            git("commit", "--quiet", "-m", f"{change} {path}")
            head = git("rev-parse", "HEAD")
            output = Path(directory) / "outputs"
            result = subprocess.run(
                ["bash", "-euo", "pipefail", "-c", detector], cwd=repo,
                env={**os.environ, "EVENT_NAME": event,
                     "MERGE_GROUP_BASE_SHA": base if event == "merge_group" else "unused-base",
                     "MERGE_GROUP_HEAD_SHA": head if event == "merge_group" else "unused-head",
                     "PR_BASE_SHA": base if event == "pull_request" else "unused-base",
                     "PR_HEAD_SHA": head if event == "pull_request" else "unused-head",
                     "STABLE_PRODUCT_PATHS": textwrap.dedent(paths), "GITHUB_OUTPUT": str(output)},
                capture_output=True, text=True,
            )
            self.assertEqual(0, result.returncode, result.stdout + result.stderr)
            return dict(line.split("=", 1) for line in output.read_text(encoding="utf-8").splitlines())

    def test_shared_guard_and_regressions_select_stable_product_lane(self) -> None:
        paths = (
            "scripts/lib/required-test-output.sh",
            "scripts/test_required_test_output.py",
        )
        for event in ("merge_group", "pull_request"):
            for change in ("modify", "delete"):
                for path in paths:
                    with self.subTest(event=event, change=change, path=path):
                        result = self._product_path_selection(path, event=event, change=change)
                        self.assertEqual("true", result["run_stable"])
                        self.assertEqual(f"stable_surface:{path}", result["reason"])

    def test_static_runner_contract_inputs_select_product_qualification(self) -> None:
        paths = (
            "cli/Cargo.toml",
            "cli/tests/cli_test.rs",
            "cli/tests/runner_output_contract.rs",
            "cli/tests/runner_process_contract.rs",
            "cli/tests/runner_linux_signal_contract.rs",
            "cli/tests/runner_protocol_contract.rs",
            "cli/tests/runner_windows_process_contract.rs",
            "cli/tests/runner_windows_output_contract.rs",
            "cli/tests/runner_windows_protocol_contract.rs",
            "cli/tests/fixtures/windows_runner_fixture.rs",
            "cli/tests/support/windows_fixture.rs",
            "cli/tests/support/windows_output.rs",
        )
        for event in ("merge_group", "pull_request"):
            for change in ("modify", "delete", "rename"):
                for path in paths:
                    with self.subTest(event=event, change=change, path=path):
                        result = self._product_path_selection(path, event=event, change=change)
                        self.assertEqual("true", result["run_stable"])
                        self.assertEqual(f"stable_surface:{path}", result["reason"])

    def test_portable_guard_and_evaluator_changes_select_product_qualification(self) -> None:
        paths = (
            "scripts/run-required-test.py",
            "scripts/test_run_required_test.py",
            "scripts/check-linux-signal-regression.py",
            "scripts/test_linux_signal_regression.py",
            "scripts/fixtures/linux-signal/reviewed-source.json",
            "scripts/fixtures/linux-signal/legacy-load-red.patch",
            "scripts/evaluate-required-result.py",
            "scripts/test_evaluate_required_result.py",
        )
        for event in ("merge_group", "pull_request"):
            for change in ("modify", "delete", "rename"):
                for path in paths:
                    with self.subTest(event=event, change=change, path=path):
                        result = self._product_path_selection(path, event=event, change=change)
                        self.assertEqual("true", result["run_stable"])
                        self.assertEqual(f"stable_surface:{path}", result["reason"])

    def test_shared_build_inputs_select_stable_product_lane(self) -> None:
        # Recorded #1027 omissions: dependencies and workspace build inputs
        # affect generated product canaries even without a runtime/ change.
        paths = (
            "common/src/lib.rs",
            "crates/common-type-ops-core/src/lib.rs",
            "crates/linecol-core/src/lib.rs",
            "crates/parsetable-metadata/src/lib.rs",
            "Cargo.toml",
            "Cargo.lock",
            "rust-toolchain.toml",
            ".cargo/config.toml",
        )
        for event in ("merge_group", "pull_request"):
            for change in ("modify", "delete"):
                for path in paths:
                    with self.subTest(event=event, change=change, path=path):
                        result = self._product_path_selection(path, event=event, change=change)
                        self.assertEqual("true", result["run_stable"])
                        self.assertEqual(f"stable_surface:{path}", result["reason"])

    def test_local_release_provenance_inputs_select_product_qualification(self) -> None:
        paths = (
            "scripts/test_local_release_archive.py",
            "xtask/Cargo.toml",
            "xtask/src/main.rs",
            "xtask/src/policy.rs",
            "xtask/src/local_registry.rs",
            "xtask/src/local_registry_provenance.rs",
            "xtask/src/local_registry_provenance_tests.rs",
            "xtask/src/local_registry_lock_fixture.rs",
            "xtask/src/local_release_archive.py",
            "xtask/src/local_release_install.rs",
            "xtask/src/local_release_receipt.rs",
            "xtask/src/local_release_command.rs",
            "xtask/tests/local_release_command_contract.rs",
            "xtask/src/release_graph.rs",
            "policy/release-graph.toml",
            "policy/package-boundary.toml",
            ".github/workflows/local-release-receipt.yml",
        )
        for event in ("merge_group", "pull_request"):
            for change in ("modify", "delete", "rename"):
                for path in paths:
                    with self.subTest(event=event, change=change, path=path):
                        result = self._product_path_selection(path, event=event, change=change)
                        self.assertEqual("true", result["run_stable"])
                        self.assertEqual(f"stable_surface:{path}", result["reason"])

    def test_local_release_controls_remain_small_guarded_selected_proof(self) -> None:
        block = self._job_block(".github/workflows/product-proof.yml", "ci-product-stable")
        self.assertIn("outputs.run_stable == 'true' && github.event_name != 'pull_request'", block)
        for selector in ("local_registry::", "local_release_install::", "local_release_receipt::"):
            self.assertIn(f"cargo test --locked -p xtask --bin xtask {selector} -- --nocapture", block)
        self.assertIn('required_test_output "$output" "$label"', block)
        self.assertIn("python3 scripts/test_local_release_archive.py", block)
        self.assertNotIn("verify-local-release-install", block)
        self.assertNotIn("continue-on-error", block)

    def test_release_command_journal_has_independent_native_target_selection(self) -> None:
        path = ".github/workflows/product-proof.yml"
        linux = self._job_block(path, "ci-product-stable")
        windows = self._job_block(path, "ci-product-windows")
        self.assertIn("cargo clippy --locked -p xtask --bin xtask --tests -- -D warnings", linux)
        self.assertIn(
            "run_release_contract 'Linux release command journal data' "
            "cargo test --locked -p xtask --test local_release_command_contract -- --nocapture",
            linux,
        )
        self.assertIn(
            "run: cargo clippy --locked -p xtask --target x86_64-pc-windows-msvc "
            "--test local_release_command_contract -- -D warnings",
            windows,
        )
        expected = (
            'run: python scripts/run-required-test.py --label "Windows release command journal data" '
            "-- cargo test --locked -p xtask --target x86_64-pc-windows-msvc "
            "--test local_release_command_contract -- --nocapture"
        )
        self.assertIn(expected, windows)
        for block in (linux, windows):
            self.assertNotIn("--test local_release_command_contract -- --ignored", block)
            self.assertNotIn("continue-on-error", block)

    def test_runtime_change_selects_stable_product_lane(self) -> None:
        for event in ("merge_group", "pull_request"):
            with self.subTest(event=event):
                result = self._product_path_selection("runtime/src/lib.rs", event=event)
                self.assertEqual("true", result["run_stable"])
                self.assertEqual("stable_surface:runtime/src/lib.rs", result["reason"])

    def test_actual_cargo_lock_fixture_is_selected_and_retains_owned_artifacts(self) -> None:
        linux = self._job_block(".github/workflows/product-proof.yml", "ci-product-stable")
        windows = self._job_block(".github/workflows/product-proof.yml", "ci-product-windows")
        self.assertEqual(1, linux.count("cargo test --locked -p xtask --bin xtask local_registry:: -- --nocapture"))
        self.assertIn("cargo clippy --locked -p xtask --bin xtask --tests -- -D warnings", linux)
        target = (
            "local_registry::provenance::tests::lock_fixture::"
            "test_actual_transformed_packaged_lock_prefetch_and_offline_install"
        )
        self.assertEqual(1, windows.count(
            'run: python scripts/run-required-test.py --label "Windows Cargo lock and offline install assumptions" '
            "-- cargo test --locked -p xtask --target x86_64-pc-windows-msvc "
            f"--bin xtask {target} -- --exact --nocapture"
        ))
        self.assertIn(
            "cargo clippy --locked -p xtask --target x86_64-pc-windows-msvc --bin xtask --tests -- -D warnings",
            windows,
        )
        for platform, block in (("linux", linux), ("windows", windows)):
            with self.subTest(platform=platform):
                marker = f"      - name: Retain {platform.title()} Cargo lock assumption artifacts\n"
                self.assertEqual(1, block.count(marker))
                upload = block.split(marker, 1)[1].split("\n      - ", 1)[0]
                self.assertIn("if: always()", upload)
                self.assertIn("uses: actions/upload-artifact@v7.0.0", upload)
                self.assertIn("path: ${{ runner.temp }}/adze-lock-fixture-*", upload)
                self.assertIn("include-hidden-files: true", upload)
                self.assertIn("if-no-files-found: error", upload)
                self.assertNotIn("continue-on-error", block)
                self.assertNotIn("verify-local-release-install", block)

    def test_unrelated_paths_do_not_select_stable_product_lane(self) -> None:
        for event in ("merge_group", "pull_request"):
            for path in ("docs/research/fixture.md", "playground/assets/fixture.txt"):
                with self.subTest(event=event, path=path):
                    result = self._product_path_selection(path, event=event)
                    self.assertEqual("false", result["run_stable"])
                    self.assertEqual("no_stable_product_surface_changed", result["reason"])

    def test_renamed_stable_path_outside_allowlist_selects_stable_product_lane(self) -> None:
        for event in ("merge_group", "pull_request"):
            with self.subTest(event=event):
                result = self._product_path_selection("runtime/src/lib.rs", event=event, change="rename")
                self.assertEqual("true", result["run_stable"])
                self.assertEqual("stable_surface:runtime/src/lib.rs", result["reason"])

    def test_contract_tests_run_before_product_path_selection(self) -> None:
        block = self._job_block(".github/workflows/product-proof.yml", "detect-stable-product-paths")
        command = "run: python3 scripts/test_evaluate_required_result.py"
        self.assertIn(command, block)
        self.assertLess(block.index(command), block.index("- id: detect"))

    def _native_dependency_preparation(self, platform: str) -> tuple[str, str, str]:
        import textwrap

        job = "ci-product-stable" if platform == "Linux" else "ci-product-windows"
        block = self._job_block(".github/workflows/product-proof.yml", job)
        name = f"Prepare locked native dependencies online for {platform} consumers"
        step = block[block.index(f"      - name: {name}\n"):]
        step = step.split("\n      - ", 1)[0]
        run_lines = []
        for line in step.split("        run: |\n", 1)[1].splitlines():
            if line and not line.startswith("          "):
                break
            run_lines.append(line)
        script = textwrap.dedent("\n".join(run_lines))
        if platform == "Linux":
            self.assertIn("shell: bash\n", step)
            self.assertTrue(script.startswith("set -euo pipefail\n"))
            program, suffix = script.split("python3 - <<'PY'\n", 1)[1].split("\nPY", 1)
            self.assertEqual("", suffix.strip())
        else:
            self.assertNotIn("shell: bash", step)
            program, suffix = script.split("@'\n", 1)[1].split("\n'@ | python -", 1)
            self.assertEqual("exit $LASTEXITCODE", suffix.strip())
        return block, step, program

    def test_native_dependency_preparation_is_finite_and_precedes_consumers(self) -> None:
        for platform, first_consumer in (
            ("Linux", "Qualify Linux static runner process and output contracts"),
            ("Windows", "Strict Clippy for native Windows runner contracts"),
        ):
            with self.subTest(platform=platform):
                block, step, _ = self._native_dependency_preparation(platform)
                self.assertIn("        timeout-minutes: 5\n", step)
                self.assertNotIn("continue-on-error", step)
                self.assertNotIn("        if:", step)
                self.assertLess(block.index("Swatinem/rust-cache@v2"), block.index(step))
                self.assertLess(block.index(step), block.index(first_consumer))

    def test_native_dependency_preparation_preserves_failures_and_lock_bytes(self) -> None:
        """Execute both workflow programs with an actual fake Cargo subprocess.

        Only Cargo's executable is substituted; the embedded hash, failure and
        lock-refusal code runs unchanged. This is not native Cargo or PowerShell
        qualification.
        """
        import hashlib
        import json
        import tempfile
        import textwrap

        fake_cargo = textwrap.dedent('''\
            import json
            import os
            import pathlib
            import sys

            pathlib.Path("cargo-argv.json").write_text(json.dumps(sys.argv[1:]))
            change = os.environ["ADZE_PREP_TEST_CHANGE"]
            if change == "changed":
                with pathlib.Path("Cargo.lock").open("ab") as lock:
                    lock.write(b"# changed\\n")
            elif change == "removed":
                pathlib.Path("Cargo.lock").unlink()
            status = int(os.environ["ADZE_PREP_TEST_STATUS"])
            if status:
                print("fixture Cargo failure", file=sys.stderr)
            raise SystemExit(status)
        ''')
        lock_bytes = b"version = 4\r\n"
        lock_hash = hashlib.sha256(lock_bytes).hexdigest()
        cases = (
            ("unchanged", 0, 0), ("unchanged", 37, 37),
            ("changed", 0, 1), ("changed", 37, 37),
            ("removed", 0, 1), ("removed", 37, 37),
            ("missing-before", 0, 1),
        )
        for platform, target in (("Linux", "host-tuple"), ("Windows", "x86_64-pc-windows-msvc")):
            _, _, program = self._native_dependency_preparation(platform)
            # This adapter preserves the workflow's exact Cargo argument list
            # and check policy while using a portable executable fake child.
            harness = textwrap.dedent('''\
                import subprocess
                import sys

                real_run = subprocess.run
                def invoke(command, *, check):
                    assert command[0] == "cargo", command
                    assert check is False
                    return real_run([sys.executable, "fake-cargo.py", *command[1:]], check=check)
                subprocess.run = invoke
            ''') + "\nexec(" + repr(program) + ")\n"
            for change, cargo_status, expected_status in cases:
                with self.subTest(platform=platform, change=change, cargo_status=cargo_status):
                    with tempfile.TemporaryDirectory(prefix="adze-dependency-preparation-") as directory:
                        fixture = Path(directory)
                        (fixture / "fake-cargo.py").write_text(fake_cargo, encoding="utf-8")
                        if change != "missing-before":
                            (fixture / "Cargo.lock").write_bytes(lock_bytes)
                        result = subprocess.run(
                            [sys.executable, "-c", harness], cwd=fixture,
                            env={**os.environ, "ADZE_PREP_TEST_CHANGE": change,
                                 "ADZE_PREP_TEST_STATUS": str(cargo_status)},
                            capture_output=True, text=True, timeout=10,
                        )
                        self.assertEqual(expected_status, result.returncode, result.stdout + result.stderr)
                        argv = fixture / "cargo-argv.json"
                        if change == "missing-before":
                            self.assertFalse(argv.exists(), "unreadable lock must prevent preparation")
                            continue
                        self.assertEqual(
                            ["fetch", "--locked", "--target", target],
                            json.loads(argv.read_text(encoding="utf-8")),
                        )
                        self.assertIn(f"dependency_preparation_lock_before_sha256={lock_hash}", result.stdout)
                        self.assertIn(f"dependency_preparation_fetch_status={cargo_status}", result.stdout)
                        if cargo_status:
                            self.assertIn("fixture Cargo failure", result.stderr)
                        if change == "unchanged":
                            self.assertEqual(lock_bytes, (fixture / "Cargo.lock").read_bytes())
                            self.assertIn(f"dependency_preparation_lock_after_sha256={lock_hash}", result.stdout)
                        elif change == "changed":
                            changed_hash = hashlib.sha256(lock_bytes + b"# changed\n").hexdigest()
                            self.assertIn(f"dependency_preparation_lock_after_sha256={changed_hash}", result.stdout)
                            self.assertIn("changed Cargo.lock bytes", result.stderr)
                        else:
                            self.assertIn("could not read Cargo.lock afterward", result.stderr)

    def test_windows_job_is_selected_with_the_existing_non_pr_product_proof(self) -> None:
        import re

        path = ".github/workflows/product-proof.yml"
        linux = self._job_block(path, "ci-product-stable")
        windows = self._job_block(path, "ci-product-windows")
        self.assertEqual(
            re.search(r"^    if: (.+)$", linux, re.M).group(1),
            re.search(r"^    if: (.+)$", windows, re.M).group(1),
        )
        self.assertIn("needs: detect-stable-product-paths", windows)
        self.assertIn("runs-on: windows-2022", windows)
        self.assertNotIn("continue-on-error", windows)
        self.assertNotIn("shell: bash", windows)
        self.assertIn("toolchain: 1.95.0", windows)
        self.assertIn("components: clippy,rustfmt", windows)
        self.assertIn("targets: x86_64-pc-windows-msvc", windows)
        self.assertIn("EXPECTED_SHA: ${{ github.sha }}", windows)
        self.assertIn("$actualSha -cne $env:EXPECTED_SHA", windows)
        self.assertEqual(2, windows.count("if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }"))
        for token in ("ImageOS", "ImageVersion", "windows_build", "rustc -vV", "lock_sha256"):
            self.assertIn(token, windows)
        self.assertIn("run: python scripts/test_run_required_test.py\n", windows)
        self.assertNotIn("test_run_required_test.py SyntheticProcessControls", windows)

    def test_windows_guards_each_native_contract_and_strict_targeted_clippy(self) -> None:
        block = self._job_block(".github/workflows/product-proof.yml", "ci-product-windows")
        commands = [line.strip() for line in block.splitlines()
                    if "run: python scripts/run-required-test.py " in line]
        self.assertEqual(9, len(commands))
        targets = [
            "--test runner_windows_process_contract -- --nocapture",
            "--test runner_windows_output_contract -- --nocapture",
            "--test runner_windows_protocol_contract -- --nocapture",
            "--test cli_test test_parse_static_tree_mode_emits_document_backed_tree -- --exact --nocapture",
            "--test cli_test test_parse_document_projection_modes_emit_schema_envelopes -- --exact --nocapture",
            "--test cli_test parse_document_json_modes_emit_recovery_diagnostics -- --exact --nocapture",
            "--bin adze generated_runner_argv_tests::test_generated_runner_protocol_args_precede_input_io -- --exact --nocapture",
        ]
        for command, target in zip(commands[:7], targets):
            self.assertIn("--label ", command)
            self.assertIn("-- cargo test --locked -p adze-cli --target x86_64-pc-windows-msvc ", command)
            self.assertTrue(command.endswith(target), command)
        self.assertEqual(
            'run: python scripts/run-required-test.py --label "Windows release command journal data" '
            "-- cargo test --locked -p xtask --target x86_64-pc-windows-msvc "
            "--test local_release_command_contract -- --nocapture",
            commands[7],
        )
        self.assertEqual(
            'run: python scripts/run-required-test.py --label "Windows Cargo lock and offline install assumptions" '
            "-- cargo test --locked -p xtask --target x86_64-pc-windows-msvc --bin xtask "
            "local_registry::provenance::tests::lock_fixture::test_actual_transformed_packaged_lock_prefetch_and_offline_install "
            "-- --exact --nocapture",
            commands[8],
        )
        clippy = (
            "cargo clippy --locked -p adze-cli --target x86_64-pc-windows-msvc --bin adze "
            "--test runner_windows_process_contract --test runner_windows_output_contract "
            "--test runner_windows_protocol_contract -- -D warnings"
        )
        self.assertIn(clippy, block)

    def test_required_product_aggregate_receives_the_windows_result(self) -> None:
        block = self._job_block(".github/workflows/product-proof.yml", "product-proof-result")
        self.assertIn(
            "needs: [detect-stable-product-paths, pr-canary-plan, ci-product-stable, ci-product-windows]",
            block,
        )
        self.assertIn("WINDOWS_RESULT: ${{ needs.ci-product-windows.result }}", block)
        self.assertIn("name: Product Proof Result", block)
        self.assertIn("always() &&", block)

    def test_ready_for_review_triggers_both_workflows(self) -> None:
        import re

        for path in (
            ".github/workflows/product-proof.yml",
            ".github/workflows/em-ci-routed-rust.yml",
        ):
            with self.subTest(workflow=path):
                text = (ROOT / path).read_text(encoding="utf-8")
                match = re.search(r"pull_request:\s*\n\s*types:\s*\[([^\]]*)\]", text)
                self.assertIsNotNone(match, f"pull_request types not found in {path}")
                assert match is not None
                self.assertIn("ready_for_review", match.group(1))

    def test_result_jobs_always_run_and_delegate_to_evaluator(self) -> None:
        cases = [
            (".github/workflows/product-proof.yml", "product-proof-result", "product"),
            (".github/workflows/em-ci-routed-rust.yml", "rust-small-result", "rust"),
        ]
        for path, job, kind in cases:
            with self.subTest(job=job):
                block = self._job_block(path, job)
                self.assertIn("always() &&", block)
                self.assertNotIn("!= 'cancelled'", block)
                self.assertNotIn('!= "cancelled"', block)
                self.assertNotIn("result != 'cancelled'", block)
                self.assertIn(f"evaluate-required-result.py {kind}", block)
                self.assertIn("actions/checkout", block)


if __name__ == "__main__":
    unittest.main(verbosity=2)
