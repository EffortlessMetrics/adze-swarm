//! Owned test-only Cargo observer for the runner-cache integration canaries.
//! This binary forwards an actual Cargo invocation, not its mocked results.

use std::env;
use std::ffi::OsString;
use std::fs::OpenOptions;
use std::io::{self, Write};
use std::process::{Command, ExitStatus};

const REAL_CARGO: &str = "ADZE_TEST_FORWARD_CARGO";
const ORIGINAL_PATH: &str = "ADZE_TEST_FORWARD_PATH";
const COUNT_LOG: &str = "ADZE_TEST_FORWARD_LOG";

fn required(name: &str) -> io::Result<OsString> {
    env::var_os(name)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, format!("missing {name}")))
}

fn forward() -> io::Result<ExitStatus> {
    let mut command = Command::new(required(REAL_CARGO)?);
    let count_log = required(COUNT_LOG)?;
    command
        .args(env::args_os().skip(1))
        .env_remove(REAL_CARGO)
        .env_remove(ORIGINAL_PATH)
        .env_remove(COUNT_LOG);
    // Do not observe Cargo launched as part of the forwarded Cargo's work.
    match env::var_os(ORIGINAL_PATH) {
        Some(path) => command.env("PATH", path),
        None => command.env_remove("PATH"),
    };
    let mut child = command.spawn()?;
    let counted = OpenOptions::new()
        .create(true)
        .append(true)
        .open(count_log)
        .and_then(|mut file| file.write_all(b"cargo\n"));
    if let Err(error) = counted {
        let _ = child.kill();
        let _ = child.wait();
        return Err(error);
    }
    child.wait()
}

fn main() {
    match forward() {
        Ok(status) => {
            if let Some(code) = status.code() {
                std::process::exit(code);
            }
            eprintln!("test Cargo child did not report a natural exit code: {status}");
        }
        Err(error) => eprintln!("test Cargo forwarding fixture failed: {error}"),
    }
    std::process::exit(1);
}
