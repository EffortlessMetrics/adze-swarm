//! Private framing for an existing public static-runner projection (#1066).
//!
//! A single JSON header line precedes exactly `payload_bytes` bytes. The
//! header is at most 4096 bytes including its LF; the capture budget covers
//! both header and payload. Public JSON whitespace, trailing newline and deep
//! payload bytes are forwarded unchanged to the existing output interpreter.
//! `terminal: "output"` means serialization completed, not syntax success:
//! the public document remains the only authority for parse outcomes.

use anyhow::{Context, Result, ensure};
use serde::Deserialize;

pub(crate) const PROTOCOL_NAME: &str = "adze.static-runner";
// The earlier unframed shape is version 1 in #1160's cache key. That cache
// remains disabled and must compose this version before any future admission.
pub(crate) const PROTOCOL_VERSION: u32 = 2;
const MAX_HEADER_BYTES: usize = 4096;

pub(crate) fn request_token() -> String {
    format!("--adze-static-runner-protocol={PROTOCOL_VERSION}")
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Header {
    protocol: String,
    version: u32,
    operation: String,
    terminal: String,
    payload_bytes: u64,
}

/// Validate and remove one complete private frame from a zero-exit response.
/// Callers retain actual nonzero status precedence before invoking this step.
/// Payload admission stays in `runner_output`, including its UTF-8 and public
/// schema checks; this shallow header introduces no recursive document model.
pub(crate) fn unframe(bytes: &mut Vec<u8>, requested_operation: &str) -> Result<()> {
    let header_end = bytes
        .iter()
        .take(MAX_HEADER_BYTES)
        .position(|byte| *byte == b'\n')
        .context("missing complete private header within 4096 bytes including LF")?;
    let header_text =
        std::str::from_utf8(&bytes[..header_end]).context("private runner header is not UTF-8")?;
    let header: Header = serde_json::from_str(header_text)
        .context("private runner header is not one complete typed JSON object")?;
    ensure!(
        header.protocol == PROTOCOL_NAME,
        "private runner protocol name mismatch"
    );
    ensure!(
        header.version == PROTOCOL_VERSION,
        "private runner version mismatch: expected {PROTOCOL_VERSION}, received {}",
        header.version
    );
    ensure!(
        header.operation == requested_operation,
        "private runner operation mismatch: expected `{requested_operation}`, received `{}`",
        header.operation
    );
    ensure!(
        header.terminal == "output",
        "zero-exit private runner response has no output terminal"
    );
    let payload_bytes = usize::try_from(header.payload_bytes)
        .context("private runner payload length does not fit this platform")?;
    let payload_start = header_end + 1;
    ensure!(
        bytes.len() - payload_start == payload_bytes,
        "private runner payload length mismatch or extra terminal response"
    );
    // The exact payload is moved in-place: no recursive JSON reserialization,
    // extra whole-payload allocation, whitespace trimming or added newline.
    drop(bytes.drain(..payload_start));
    Ok(())
}
