#!/usr/bin/env python3
"""Real Cargo consumers for default C and minimal Rust scanner builds.

Run with the repository toolchain and cached dependencies:
  python3 tool/tests/fixtures/scanner_profiles/exercise.py
Each profile has its own target; all Cargo operations are offline. Retained
sources, build logs, program output, and the receipt live in the printed path.
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
PROOF = Path(tempfile.mkdtemp(prefix="adze-scanner-profiles-"))
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--target-root", type=Path, help="optional owned parent for separate per-profile Cargo targets")
TARGET_ROOT = parser.parse_args().target_root or PROOF
ENV = {**os.environ, "CARGO_NET_OFFLINE": "true", "CARGO_INCREMENTAL": "0",
       "CARGO_BUILD_JOBS": "2", "CARGO_PROFILE_DEV_DEBUG": "0"}
ROWS = []
BUILD = '''fn main() {
    let source = std::path::PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("src");
    let output = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    adze_tool::scanner_build::ScannerBuilder::new("fixture", source, output).build().unwrap();
}
'''
SCANNER = '''use adze::external_scanner::{ExternalScanner, Lexer, ScanResult};
#[derive(Default)]
pub struct FixtureScanner;
impl ExternalScanner for FixtureScanner {
    fn scan(&mut self, _: &mut dyn Lexer, _: &[bool]) -> Option<ScanResult> { None }
    fn serialize(&self, buffer: &mut Vec<u8>) { buffer.push(17); }
    fn deserialize(&mut self, _: &[u8]) {}
}
'''


def command(name: str, args: list[str], cwd: Path, env: dict[str, str]) -> subprocess.CompletedProcess[str]:
    result = subprocess.run(args, cwd=cwd, env=env, capture_output=True, text=True, timeout=240)
    (cwd / (name + ".stdout")).write_text(result.stdout)
    (cwd / (name + ".stderr")).write_text(result.stderr)
    if result.returncode:
        raise RuntimeError(f"{args}: exit {result.returncode}; see {cwd}/{name}.stderr")
    return result


for profile in ("default-c", "minimal-rust"):
    root = PROOF / profile
    (root / "src").mkdir(parents=True)
    minimal = profile == "minimal-rust"
    tool = json.dumps(str(REPO / "tool"))
    runtime = json.dumps(str(REPO / "runtime"))
    manifest = f'''[package]
name = "scanner-profile-{profile}"
version = "0.0.0"
edition = "2024"
publish = false
[workspace]
[build-dependencies]
adze-tool = {{ path = {tool}, default-features = {str(not minimal).lower()} }}
'''
    if minimal:
        manifest += f"[dependencies]\nadze = {{ path = {runtime} }}\n"
        (root / "src/scanner.rs").write_text(SCANNER)
        main = '''include!(concat!(env!("OUT_DIR"), "/fixture_scanner_registration.rs"));
fn main() {
    register_scanner();
    let registry = adze::scanner_registry::get_global_registry();
    let guard = registry.lock().unwrap();
    let scanner = guard.create_scanner("fixture").expect("generated Rust registration must work");
    let mut state = Vec::new(); scanner.serialize(&mut state);
    assert_eq!(state, vec![17]);
    println!("minimal-rust-scanner=17");
}
'''
        marker = "minimal-rust-scanner=17"
    else:
        (root / "src/scanner.c").write_text("int scanner_marker(void) { return 37; }\n")
        main = '''unsafe extern "C" { fn scanner_marker() -> i32; }
fn main() {
    assert_eq!(unsafe { scanner_marker() }, 37);
    println!("default-c-scanner=37");
}
'''
        marker = "default-c-scanner=37"
    (root / "Cargo.toml").write_text(manifest)
    (root / "build.rs").write_text(BUILD)
    (root / "src/main.rs").write_text(main)
    env = {**ENV, "CARGO_TARGET_DIR": str(TARGET_ROOT / profile / "target")}
    command("lock", ["cargo", "generate-lockfile", "--offline"], root, env)
    build = command("build", ["cargo", "build", "--offline", "--locked", "--message-format=json"], root, env)
    records = [json.loads(line) for line in build.stdout.splitlines() if line.startswith("{")]
    tool_features = [r["features"] for r in records if r.get("reason") == "compiler-artifact" and r["target"]["name"] == "adze_tool"]
    assert tool_features == ([[]] if minimal else [["build_parsers", "default"]]), tool_features
    executable = next(r["executable"] for r in records if r.get("executable") and r["target"]["name"] == f"scanner-profile-{profile}")
    output = command("run", [executable], root, env)
    assert output.stdout.strip() == marker, output.stdout
    ROWS.append({"profile": profile, "tool_features": tool_features[0], "build_exit": build.returncode,
                 "run_exit": output.returncode, "stdout": output.stdout.strip(), "root": str(root)})
    print(json.dumps(ROWS[-1]), flush=True)
receipt = {"source": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=REPO, text=True).strip(),
           "scanner_source_sha256": hashlib.sha256((REPO / "tool/src/scanner_build.rs").read_bytes()).hexdigest(),
           "offline": True, "profiles": ROWS}
(PROOF / "receipt.json").write_text(json.dumps(receipt, indent=2))
print(PROOF / "receipt.json", flush=True)
