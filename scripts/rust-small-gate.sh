#!/usr/bin/env bash
# Shared Rust Small gate: workspace check used by every routed lane
# (self-hosted cpx42/cx33 and the GitHub-hosted fallback).
set -euo pipefail

rustc --version
cargo --version

cargo fetch --locked
cargo check --workspace --locked
