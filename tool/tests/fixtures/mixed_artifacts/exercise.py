#!/usr/bin/env python3
"""Build C artifacts and execute the opportunistic Rust parser together.

The consumer has no local pure-rust feature; the runtime enables pure-rust.
Run: python3 tool/tests/fixtures/mixed_artifacts/exercise.py
All Cargo operations are offline; retained inputs/logs/receipt are printed.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile

REPO = Path(__file__).resolve().parents[4]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--target-dir", type=Path, default=Path("/tmp/adze-nov2-mixed-artifacts-target"))
args = parser.parse_args()
PROOF = Path(tempfile.mkdtemp(prefix="adze-mixed-artifacts-"))
(PROOF / "src").mkdir()
(PROOF / "Cargo.toml").write_text(f'''[package]
name = "mixed-artifact-consumer"
version = "0.0.0"
edition = "2024"
publish = false
[workspace]
[dependencies]
adze = {{ path = {json.dumps(str(REPO / "runtime"))}, default-features = false, features = ["pure-rust"] }}
[build-dependencies]
adze-tool = {{ path = {json.dumps(str(REPO / "tool"))} }}
''')
(PROOF / "build.rs").write_text('''fn main() {
    assert!(std::env::var_os("CARGO_FEATURE_PURE_RUST").is_none());
    assert!(std::env::var_os("ADZE_USE_PURE_RUST").is_none());
    println!("cargo::rustc-check-cfg=cfg(adze_unsafe_attrs)");
    adze_tool::build_parsers(std::path::Path::new("src/lib.rs"));
}
''')
(PROOF / "src/lib.rs").write_text('''#[adze::grammar("mixed_artifact_fixture")]
pub mod grammar {
    #[adze::language]
    #[derive(Debug, PartialEq, Eq)]
    pub struct Word {
        #[adze::leaf(text = "hello")]
        pub value: (),
    }
}
''')
(PROOF / "src/main.rs").write_text('''use mixed_artifact_consumer::grammar::{Word, parse};
fn main() {
    assert_eq!(parse("hello").expect("exact selected value"), Word { value: () });
    assert!(parse("goodbye").is_err());
    println!("mixed-artifact-word=hello");
}
''')
env = {k: v for k, v in os.environ.items() if k not in ("ADZE_USE_PURE_RUST", "CARGO_FEATURE_PURE_RUST")}
env.update(CARGO_NET_OFFLINE="true", CARGO_TARGET_DIR=str(args.target_dir), CARGO_INCREMENTAL="0",
           CARGO_BUILD_JOBS="2", CARGO_PROFILE_DEV_DEBUG="0", ADZE_EMIT_ARTIFACTS="true")
receipt = {"source": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=REPO, text=True).strip(),
           "proof": str(PROOF), "target": str(args.target_dir), "offline": True,
           "caller_features": [], "runtime_features": ["pure-rust"], "emit_artifacts": True}
print(PROOF, flush=True)
for stage, argv in [("lock", ["cargo", "generate-lockfile", "--offline"]),
                    ("build", ["cargo", "build", "--offline", "--locked", "--message-format=json"])]:
    result = subprocess.run(argv, cwd=PROOF, env=env, capture_output=True, text=True, timeout=240)
    (PROOF / (stage + ".stdout")).write_text(result.stdout)
    (PROOF / (stage + ".stderr")).write_text(result.stderr)
    receipt[stage] = {"argv": argv, "exit": result.returncode}
    (PROOF / "receipt.json").write_text(json.dumps(receipt, indent=2))
    if result.returncode:
        print(result.stderr, flush=True)
        for line in result.stdout.splitlines():
            try:
                record = json.loads(line)
            except ValueError:
                continue
            if record.get("reason") == "compiler-message" and record["message"]["level"] == "error":
                print(record["message"].get("rendered", record["message"]["message"]), flush=True)
        raise SystemExit(result.returncode)
records = [json.loads(line) for line in result.stdout.splitlines() if line.startswith("{")]
executable = next(r["executable"] for r in records if r.get("executable") and r["target"]["name"] == "mixed-artifact-consumer")
out_dir = next(Path(r["out_dir"]) for r in records if r.get("reason") == "build-script-executed" and "mixed-artifact-consumer" in r["package_id"])
grammar = out_dir / "grammar_mixed_artifact_fixture"
owned = ["parser.c", "mixed_artifact_fixture.json", "tree_sitter/parser.h",
         "parser_mixed_artifact_fixture.rs", "grammar.ir.json", "NODE_TYPES.json"]
receipt["artifacts"] = {name: hashlib.sha256((grammar / name).read_bytes()).hexdigest() for name in owned}
result = subprocess.run([executable], cwd=PROOF, env=env, capture_output=True, text=True, timeout=30)
(PROOF / "run.stdout").write_text(result.stdout); (PROOF / "run.stderr").write_text(result.stderr)
receipt["run"] = {"exit": result.returncode, "stdout": result.stdout.strip()}
assert result.returncode == 0 and result.stdout.strip() == "mixed-artifact-word=hello", receipt["run"]
(PROOF / "receipt.json").write_text(json.dumps(receipt, indent=2))
print(PROOF / "receipt.json", flush=True)
