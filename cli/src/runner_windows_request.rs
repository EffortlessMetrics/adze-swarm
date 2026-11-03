//! Private Windows bootstrap request; no public document or runner-output facts.
//!
//! The request is explicit because `Command` getters cannot recover env_clear,
//! raw_arg or Windows creation settings. Only the fields below are supported.

use std::ffi::{OsStr, OsString};
use std::io;
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::path::PathBuf;
use std::process::{Command, Stdio};

pub(crate) const MAX_REQUEST_BYTES: usize = 1024 * 1024;
pub(crate) const HEADER_BYTES: usize = 16;
const MAGIC: &[u8; 8] = b"ADZWRQ01";
const VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum EnvironmentPolicy {
    Inherit,
    Clear,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum EnvironmentChange {
    Set(OsString, OsString),
    Remove(OsString),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RunnerRequest {
    pub(crate) program: OsString,
    pub(crate) args: Vec<OsString>,
    pub(crate) environment: EnvironmentPolicy,
    pub(crate) changes: Vec<EnvironmentChange>,
    /// None inherits the CLI cwd; it does not select the temporary project cwd.
    pub(crate) cwd: Option<PathBuf>,
}

impl RunnerRequest {
    pub(crate) fn command(&self) -> Command {
        let mut command = Command::new(&self.program);
        command.args(&self.args);
        if self.environment == EnvironmentPolicy::Clear {
            command.env_clear();
        }
        // Preserve order, including repeated/case-variant Windows keys.
        for change in &self.changes {
            match change {
                EnvironmentChange::Set(key, value) => {
                    command.env(key, value);
                }
                EnvironmentChange::Remove(key) => {
                    command.env_remove(key);
                }
            }
        }
        if let Some(cwd) = &self.cwd {
            command.current_dir(cwd);
        }
        command
            .stdin(Stdio::null())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());
        command
    }

    pub(crate) fn encode(&self) -> io::Result<Vec<u8>> {
        let mut writer = Writer(Vec::with_capacity(HEADER_BYTES));
        writer.bytes(MAGIC)?;
        writer.number(VERSION)?;
        writer.number(0)?;
        writer.number(match self.environment {
            EnvironmentPolicy::Inherit => 0,
            EnvironmentPolicy::Clear => 1,
        })?;
        writer.number(u32::from(self.cwd.is_some()))?;
        writer.string(&self.program)?;
        writer.count(self.args.len())?;
        for arg in &self.args {
            writer.string(arg)?;
        }
        writer.count(self.changes.len())?;
        for change in &self.changes {
            match change {
                EnvironmentChange::Set(key, value) => {
                    writer.number(1)?;
                    writer.string(key)?;
                    writer.string(value)?;
                }
                EnvironmentChange::Remove(key) => {
                    writer.number(0)?;
                    writer.string(key)?;
                }
            }
        }
        if let Some(cwd) = &self.cwd {
            writer.string(cwd.as_os_str())?;
        }
        let length = u32::try_from(writer.0.len()).map_err(|_| invalid("request size overflow"))?;
        writer.0[12..16].copy_from_slice(&length.to_le_bytes());
        Ok(writer.0)
    }

    pub(crate) fn decode(bytes: &[u8]) -> io::Result<Self> {
        let length = request_length(bytes.get(..HEADER_BYTES).ok_or_else(|| invalid("short request header"))?)?;
        if length != bytes.len() {
            return Err(invalid("request extent does not match its declared length"));
        }
        let mut reader = Reader { bytes, offset: HEADER_BYTES };
        let environment = match reader.number()? {
            0 => EnvironmentPolicy::Inherit,
            1 => EnvironmentPolicy::Clear,
            _ => return Err(invalid("unknown request environment policy")),
        };
        let has_cwd = match reader.number()? {
            0 => false,
            1 => true,
            _ => return Err(invalid("unknown request cwd policy")),
        };
        let program = reader.string()?;
        let count = reader.count(4)?;
        let mut args = Vec::new();
        for _ in 0..count {
            args.push(reader.string()?);
        }
        let count = reader.count(8)?;
        let mut changes = Vec::new();
        for _ in 0..count {
            let tag = reader.number()?;
            let key = reader.string()?;
            changes.push(match tag {
                0 => EnvironmentChange::Remove(key),
                1 => EnvironmentChange::Set(key, reader.string()?),
                _ => return Err(invalid("unknown request environment change")),
            });
        }
        let cwd = if has_cwd { Some(PathBuf::from(reader.string()?)) } else { None };
        if reader.offset != bytes.len() {
            return Err(invalid("trailing request records"));
        }
        Ok(Self { program, args, environment, changes, cwd })
    }
}

/// Validate only the fixed header before the helper maps any declared extent.
pub(crate) fn request_length(header: &[u8]) -> io::Result<usize> {
    if header.len() != HEADER_BYTES || &header[..8] != MAGIC {
        return Err(invalid("invalid request header"));
    }
    let version = u32::from_le_bytes(header[8..12].try_into().expect("fixed version extent"));
    if version != VERSION {
        return Err(invalid("unsupported bootstrap request version"));
    }
    let length = u32::from_le_bytes(header[12..16].try_into().expect("fixed length extent")) as usize;
    if !(HEADER_BYTES..=MAX_REQUEST_BYTES).contains(&length) {
        return Err(invalid("bootstrap request exceeds its 1048576-byte limit or is too short"));
    }
    Ok(length)
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

struct Writer(Vec<u8>);

impl Writer {
    fn room(&self, additional: usize) -> io::Result<()> {
        if self.0.len().checked_add(additional).is_none_or(|n| n > MAX_REQUEST_BYTES) {
            return Err(invalid("bootstrap request exceeds its 1048576-byte limit"));
        }
        Ok(())
    }

    fn bytes(&mut self, bytes: &[u8]) -> io::Result<()> {
        self.room(bytes.len())?;
        self.0.extend_from_slice(bytes);
        Ok(())
    }

    fn number(&mut self, number: u32) -> io::Result<()> {
        self.bytes(&number.to_le_bytes())
    }

    fn count(&mut self, count: usize) -> io::Result<()> {
        self.number(u32::try_from(count).map_err(|_| invalid("request count overflow"))?)
    }

    fn string(&mut self, value: &OsStr) -> io::Result<()> {
        let units = value.encode_wide().count();
        let bytes = units.checked_mul(2).and_then(|n| n.checked_add(4)).ok_or_else(|| invalid("request string overflow"))?;
        self.room(bytes)?;
        if value.encode_wide().any(|unit| unit == 0) {
            return Err(invalid("request strings cannot contain NUL"));
        }
        self.count(units)?;
        for unit in value.encode_wide() {
            self.0.extend_from_slice(&unit.to_le_bytes());
        }
        Ok(())
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl Reader<'_> {
    fn take(&mut self, count: usize) -> io::Result<&[u8]> {
        let end = self.offset.checked_add(count).ok_or_else(|| invalid("request extent overflow"))?;
        let value = self.bytes.get(self.offset..end).ok_or_else(|| invalid("truncated request record"))?;
        self.offset = end;
        Ok(value)
    }

    fn number(&mut self) -> io::Result<u32> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().expect("four-byte number")))
    }

    fn count(&mut self, minimum_record_bytes: usize) -> io::Result<usize> {
        let count = self.number()? as usize;
        if count > (self.bytes.len() - self.offset) / minimum_record_bytes {
            return Err(invalid("request count exceeds remaining extent"));
        }
        Ok(count)
    }

    fn string(&mut self) -> io::Result<OsString> {
        let units = self.number()? as usize;
        let bytes = units.checked_mul(2).ok_or_else(|| invalid("request string overflow"))?;
        let data = self.take(bytes)?;
        let units: Vec<u16> = data.chunks_exact(2).map(|pair| u16::from_le_bytes([pair[0], pair[1]])).collect();
        if units.contains(&0) {
            return Err(invalid("request strings cannot contain NUL"));
        }
        // OsString preserves every Windows UTF-16 code unit, including an
        // unpaired surrogate. Unicode validity is not a new request restriction.
        Ok(OsString::from_wide(&units))
    }
}
