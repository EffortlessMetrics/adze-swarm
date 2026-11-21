//! Fixed private terminal record, separate from parsed stdout and diagnostics.
//! This codec is portable so the distinction can also be tested without Win32.

use std::io;

pub(crate) const STATUS_BYTES: usize = 16;
const MAGIC: &[u8; 8] = b"ADZWST01";
const NATURAL: u32 = 1;
const SPAWN_OS: u32 = 2;
const SPAWN_KIND: u32 = 3;
pub(crate) const BOOTSTRAP_FAILURE_EXIT: u32 = 1;

#[derive(Debug)]
pub(crate) enum Terminal {
    Natural(u32),
    ExternalSpawn(io::Error),
}

fn kind_tag(kind: io::ErrorKind) -> u32 {
    match kind {
        io::ErrorKind::NotFound => 1,
        io::ErrorKind::PermissionDenied => 2,
        io::ErrorKind::InvalidInput => 3,
        io::ErrorKind::Unsupported => 4,
        _ => 5,
    }
}

fn parse_kind(tag: u32) -> Option<io::ErrorKind> {
    Some(match tag {
        1 => io::ErrorKind::NotFound,
        2 => io::ErrorKind::PermissionDenied,
        3 => io::ErrorKind::InvalidInput,
        4 => io::ErrorKind::Unsupported,
        5 => io::ErrorKind::Other,
        _ => return None,
    })
}

pub(crate) fn encode(outcome: &Terminal) -> [u8; STATUS_BYTES] {
    let (tag, value) = match outcome {
        Terminal::Natural(code) => (NATURAL, *code),
        Terminal::ExternalSpawn(error) => match error.raw_os_error().filter(|&code| code != 0) {
            Some(code) => (SPAWN_OS, code as u32),
            None => (SPAWN_KIND, kind_tag(error.kind())),
        },
    };
    let mut bytes = [0_u8; STATUS_BYTES];
    bytes[..8].copy_from_slice(MAGIC);
    bytes[8..12].copy_from_slice(&tag.to_le_bytes());
    bytes[12..].copy_from_slice(&value.to_le_bytes());
    bytes
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

pub(crate) fn decode(bytes: &[u8], helper_exit: u32) -> io::Result<Terminal> {
    if bytes.len() != STATUS_BYTES || &bytes[..8] != MAGIC {
        return Err(invalid("missing or invalid bootstrap terminal record"));
    }
    let tag = u32::from_le_bytes(bytes[8..12].try_into().expect("fixed tag extent"));
    let value = u32::from_le_bytes(bytes[12..].try_into().expect("fixed result extent"));
    match tag {
        NATURAL if value == helper_exit => Ok(Terminal::Natural(value)),
        SPAWN_OS if value != 0 && helper_exit == BOOTSTRAP_FAILURE_EXIT => Ok(
            Terminal::ExternalSpawn(io::Error::from_raw_os_error(value as i32)),
        ),
        SPAWN_KIND if helper_exit == BOOTSTRAP_FAILURE_EXIT => parse_kind(value)
            .map(|kind| Terminal::ExternalSpawn(io::Error::from(kind)))
            .ok_or_else(|| invalid("invalid bootstrap spawn error kind")),
        _ => Err(invalid("bootstrap terminal record contradicts its exit")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_natural_exit_one_has_distinct_wire_evidence_from_spawn_failure() {
        let natural = encode(&Terminal::Natural(1));
        assert_eq!(&natural[..8], b"ADZWST01");
        assert_eq!(&natural[8..], &[1, 0, 0, 0, 1, 0, 0, 0]);
        let failure = encode(&Terminal::ExternalSpawn(io::Error::from_raw_os_error(2)));
        assert_eq!(&failure[8..], &[2, 0, 0, 0, 2, 0, 0, 0]);
        assert!(matches!(decode(&natural, 1).unwrap(), Terminal::Natural(1)));
        let Terminal::ExternalSpawn(error) = decode(&failure, 1).unwrap() else {
            panic!("external NotFound was decoded as a natural exit");
        };
        assert_eq!(error.raw_os_error(), Some(2));
    }

    #[test]
    fn test_terminal_keeps_all_natural_status_bits_and_supported_error_kinds() {
        for code in [0_u32, 1, 259, 0x8000ffff, 0xc000013a, u32::MAX] {
            assert!(matches!(
                decode(&encode(&Terminal::Natural(code)), code).unwrap(),
                Terminal::Natural(actual) if actual == code
            ));
        }
        for kind in [
            io::ErrorKind::NotFound,
            io::ErrorKind::PermissionDenied,
            io::ErrorKind::InvalidInput,
            io::ErrorKind::Unsupported,
            io::ErrorKind::Other,
        ] {
            let Terminal::ExternalSpawn(error) =
                decode(&encode(&Terminal::ExternalSpawn(kind.into())), 1).unwrap()
            else {
                panic!("non-OS spawn failure became natural");
            };
            assert_eq!(error.kind(), kind);
        }
    }

    #[test]
    fn test_missing_partial_and_contradictory_terminals_cannot_supply_natural_status() {
        let natural = encode(&Terminal::Natural(1));
        let failure = encode(&Terminal::ExternalSpawn(io::Error::from_raw_os_error(2)));
        let zero_os_code = encode(&Terminal::ExternalSpawn(io::Error::from_raw_os_error(2)));
        let bad_kind = encode(&Terminal::ExternalSpawn(io::ErrorKind::InvalidInput.into()));
        let mut bad_tag = natural;
        bad_tag[8..12].copy_from_slice(&4_u32.to_le_bytes());
        let mut zero_os_code = zero_os_code;
        zero_os_code[12..].fill(0);
        let mut bad_kind = bad_kind;
        bad_kind[12..].copy_from_slice(&6_u32.to_le_bytes());
        let mut uncommitted = encode(&Terminal::Natural(0));
        uncommitted[0] = 0;
        let mut trailing = natural.to_vec();
        trailing.push(0);
        for (record, helper_exit) in [
            (&[0_u8; STATUS_BYTES][..], 1),
            (&uncommitted[..], 0),
            (&natural[..15], 1),
            (trailing.as_slice(), 1),
            (&natural[..], 0),
            (&failure[..], 2),
            (&bad_tag[..], 1),
            (&zero_os_code[..], 1),
            (&bad_kind[..], 1),
        ] {
            assert_eq!(
                decode(record, helper_exit).unwrap_err().kind(),
                io::ErrorKind::InvalidData
            );
        }
    }
}
