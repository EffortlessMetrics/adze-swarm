//! Regression for #1098: the real `xtask` binary entry point must run
//! `verify-local-release-install --dry-run` to completion. This exercises
//! `main()` itself (including its worker-thread dispatch), not just the
//! library `run()` function.

use std::process::Command;

#[test]
fn xtask_binary_dry_run_prints_plan_and_exits_ok() {
    let output = Command::new(env!("CARGO_BIN_EXE_xtask"))
        .args([
            "verify-local-release-install",
            "--version",
            "0.10.0",
            "--dry-run",
        ])
        .output()
        .expect("run xtask binary");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "dry-run must exit ok, status={} stderr={stderr}",
        output.status
    );
    assert!(
        stdout.contains("status: dry-run") && stdout.contains("crate count: 12"),
        "dry-run plan missing from stdout: {stdout}"
    );
    assert!(
        !stderr.contains("overflowed its stack"),
        "entry point stack-overflowed: {stderr}"
    );
}
