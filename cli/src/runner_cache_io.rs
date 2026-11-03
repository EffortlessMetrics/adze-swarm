//! Bounded file IO for the disabled static-runner cache.
//!
//! Limits are enforced while reading, including a single excess-byte probe;
//! a metadata length is only an early refusal, never the bound itself. Copies
//! hash the bytes actually written to an exclusively created destination.
//! These are byte/allocation bounds, not filesystem deadlines or a proof of
//! immutable compiler inputs. Callers own destination cleanup/publication and
//! must separately establish trusted ancestors and process/entry custody.

use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::Path;

const CHUNK_BYTES: usize = 8192;

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct FileDigest {
    pub(crate) len: u64,
    pub(crate) sha256: [u8; 32],
}

fn over_limit(limit: u64) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        format!("cache file exceeds its {limit}-byte limit"),
    )
}

/// Visit no more than `limit` bytes and read at most one further byte.
/// An excess byte is never passed to the sink or included in a digest.
fn bounded_chunks(
    input: &mut impl Read,
    limit: u64,
    mut sink: impl FnMut(&[u8]) -> io::Result<()>,
) -> io::Result<FileDigest> {
    let mut buffer = [0; CHUNK_BYTES];
    let mut len = 0_u64;
    let mut hasher = Sha256::new();
    loop {
        let remaining = limit - len;
        let requested = remaining.saturating_add(1).min(CHUNK_BYTES as u64) as usize;
        let count = match input.read(&mut buffer[..requested]) {
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            other => other?,
        };
        if count == 0 {
            return Ok(FileDigest {
                len,
                sha256: hasher.finalize().into(),
            });
        }
        if count as u64 > remaining {
            return Err(over_limit(limit));
        }
        sink(&buffer[..count])?;
        hasher.update(&buffer[..count]);
        len += count as u64;
    }
}

fn require_regular(metadata: &std::fs::Metadata) -> io::Result<()> {
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "cache input is not a regular file",
        ));
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        use windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT;

        if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "cache input is a reparse point",
            ));
        }
    }
    Ok(())
}

/// Check regular-file metadata before and after opening. Linux and Windows
/// additionally open without following a final-component symlink/reparse
/// point. Linux's nonblocking open avoids waiting on a substituted FIFO.
/// Other platforms retain metadata checks without claiming those race bounds.
fn open_regular(path: &Path, limit: u64) -> io::Result<File> {
    require_regular(&std::fs::symlink_metadata(path)?)?;
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        use windows_sys::Win32::Storage::FileSystem::FILE_FLAG_OPEN_REPARSE_POINT;
        options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    }
    let file = options.open(path)?;
    let metadata = file.metadata()?;
    require_regular(&metadata)?;
    if metadata.len() > limit {
        return Err(over_limit(limit));
    }
    Ok(file)
}

pub(crate) fn read_regular(path: &Path, limit: u64) -> io::Result<Vec<u8>> {
    let mut file = open_regular(path, limit)?;
    let mut bytes = Vec::new();
    bounded_chunks(&mut file, limit, |chunk| {
        bytes.try_reserve_exact(chunk.len()).map_err(io::Error::other)?;
        bytes.extend_from_slice(chunk);
        Ok(())
    })?;
    Ok(bytes)
}

pub(crate) fn digest_regular(path: &Path, limit: u64) -> io::Result<FileDigest> {
    bounded_chunks(&mut open_regular(path, limit)?, limit, |_| Ok(()))
}

/// A failed copy may leave a partial file only at this caller-owned, newly
/// created destination. It is never a completed/published artifact.
pub(crate) fn copy_regular(
    source: &Path,
    destination: &Path,
    limit: u64,
) -> io::Result<FileDigest> {
    let mut input = open_regular(source, limit)?;
    let permissions = input.metadata()?.permissions();
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut output = options.open(destination)?;
    let digest = bounded_chunks(&mut input, limit, |chunk| output.write_all(chunk))?;
    output.set_permissions(permissions)?;
    output.sync_all()?;
    Ok(digest)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_exact_and_zero_byte_limits_preserve_literal_content_and_digest() {
        let mut output = Vec::new();
        let digest = bounded_chunks(&mut Cursor::new(b"abc"), 3, |chunk| {
            output.extend_from_slice(chunk);
            Ok(())
        })
        .unwrap();
        assert_eq!(output, b"abc");
        assert_eq!(digest.len, 3);
        assert_eq!(
            format!("{:x}", Sha256::digest(&output)),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        );
        assert_eq!(digest.sha256, <[u8; 32]>::from(Sha256::digest(b"abc")));
        assert_eq!(
            bounded_chunks(&mut Cursor::new(b""), 0, |_| panic!("empty source"))
                .unwrap()
                .len,
            0,
        );
        assert_eq!(
            bounded_chunks(&mut Cursor::new(b"x"), 0, |_| {
                panic!("excess byte cannot reach the sink")
            })
            .unwrap_err()
            .kind(),
            io::ErrorKind::InvalidData,
        );
    }

    struct OneByteAtATime {
        bytes: Cursor<Vec<u8>>,
        delivered: usize,
    }

    impl Read for OneByteAtATime {
        fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
            let count = self.bytes.read(&mut output[..1])?;
            self.delivered += count;
            Ok(count)
        }
    }

    #[test]
    fn test_stream_that_continues_past_the_limit_reads_only_one_excess_byte() {
        let mut input = OneByteAtATime {
            bytes: Cursor::new(vec![b'x'; 100]),
            delivered: 0,
        };
        let mut output = Vec::new();
        let error = bounded_chunks(&mut input, 3, |chunk| {
            output.extend_from_slice(chunk);
            Ok(())
        })
        .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert_eq!(input.delivered, 4);
        assert_eq!(output, b"xxx");
    }

    #[test]
    fn test_input_and_destination_failures_are_not_successful_truncation() {
        struct FailedRead;
        impl Read for FailedRead {
            fn read(&mut self, _output: &mut [u8]) -> io::Result<usize> {
                Err(io::Error::new(io::ErrorKind::PermissionDenied, "source denied"))
            }
        }
        assert_eq!(
            bounded_chunks(&mut FailedRead, 4, |_| Ok(()))
                .unwrap_err()
                .kind(),
            io::ErrorKind::PermissionDenied,
        );
        let error = bounded_chunks(&mut Cursor::new(b"abc"), 3, |_| {
            Err(io::Error::new(io::ErrorKind::WriteZero, "destination full"))
        })
        .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::WriteZero);
    }

    #[test]
    fn test_regular_copy_is_exclusive_and_hashes_exact_owned_bytes() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("source space λ");
        let destination = directory.path().join("owned");
        std::fs::write(&source, b"abc").unwrap();
        #[cfg(target_os = "linux")]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&source, std::fs::Permissions::from_mode(0o700)).unwrap();
        }
        let digest = copy_regular(&source, &destination, 3).unwrap();
        assert_eq!(read_regular(&destination, 3).unwrap(), b"abc");
        assert_eq!(digest_regular(&destination, 3).unwrap(), digest);
        assert_eq!(digest.len, 3);
        #[cfg(target_os = "linux")]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&destination).unwrap().permissions().mode() & 0o777,
                0o700,
            );
        }
        std::fs::write(&source, b"new").unwrap();
        assert_eq!(
            copy_regular(&source, &destination, 3).unwrap_err().kind(),
            io::ErrorKind::AlreadyExists,
        );
        assert_eq!(std::fs::read(&destination).unwrap(), b"abc");
    }

    #[test]
    fn test_oversize_and_nonregular_files_refuse_before_destination_creation() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("oversize");
        let destination = directory.path().join("never-created");
        File::create(&source).unwrap().set_len(9).unwrap();
        assert_eq!(read_regular(&source, 8).unwrap_err().kind(), io::ErrorKind::InvalidData);
        assert_eq!(digest_regular(&source, 8).unwrap_err().kind(), io::ErrorKind::InvalidData);
        assert_eq!(
            copy_regular(&source, &destination, 8).unwrap_err().kind(),
            io::ErrorKind::InvalidData,
        );
        assert!(!destination.exists());
        assert!(read_regular(directory.path(), 8).is_err());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn test_linux_symlink_and_fifo_inputs_are_refused() {
        use std::os::unix::ffi::OsStrExt;
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("source");
        let link = directory.path().join("link");
        std::fs::write(&source, b"abc").unwrap();
        symlink(&source, &link).unwrap();
        assert!(read_regular(&link, 3).is_err());
        let fifo = directory.path().join("fifo");
        let c_path = std::ffi::CString::new(fifo.as_os_str().as_bytes()).unwrap();
        // SAFETY: c_path is a live NUL-terminated path, and mkfifo retains no
        // pointer. This test owns the containing temporary directory.
        assert_eq!(unsafe { libc::mkfifo(c_path.as_ptr(), 0o600) }, 0);
        assert!(read_regular(&fifo, 3).is_err());
    }
}
