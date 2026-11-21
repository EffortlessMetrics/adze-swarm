//! Observe bytes at the expected local Cargo install path. The caller owns
//! the install TempDir and keeps it alive through these observations. These
//! samples establish neither immutable custody nor execution by handle.

use std::fs::{self, File};
use std::io::Read;
use std::path::{Component, Path};

use anyhow::{Context, Result, bail};
use serde::Serialize;
use sha2::{Digest, Sha256};

const IMAGE_LIMIT: u64 = 256 * 1024 * 1024;

pub(super) fn validate_binary_basename(name: &str) -> Result<()> {
    // Cargo receives the literal name; require one unambiguous filename on
    // both separator conventions before passing it to an installer.
    if name.contains(['/', '\\', ':'])
        || Path::new(name).file_name().and_then(|p| p.to_str()) != Some(name)
    {
        bail!("installed CLI binary name must be one unambiguous basename: {name:?}");
    }
    Ok(())
}

pub(super) fn receipt_path(relative: &Path) -> Result<String> {
    let name = relative
        .file_name()
        .and_then(|name| name.to_str())
        .context("installed CLI receipt requires a UTF-8 basename")?;
    validate_binary_basename(name)?;
    if relative.parent() != Some(Path::new("bin")) {
        bail!("installed CLI receipt requires a path directly under bin");
    }
    Ok(format!("bin/{name}"))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(super) struct InstalledImage {
    size_bytes: u64,
    sha256: String,
}

impl InstalledImage {
    pub(super) fn capture(owned_root: &Path, path: &Path) -> Result<Self> {
        Self::capture_bounded(owned_root, path, IMAGE_LIMIT)
            .with_context(|| format!("observing installed CLI image at {}", path.display()))
    }

    fn capture_bounded(owned_root: &Path, path: &Path, limit: u64) -> Result<Self> {
        let relative = path
            .strip_prefix(owned_root)
            .context("installed CLI path must be under its owned install root")?;
        let mut parts = relative.components();
        if !matches!(
            (parts.next(), parts.next(), parts.next()),
            (Some(Component::Normal(dir)), Some(Component::Normal(_)), None) if dir == "bin"
        ) {
            bail!("installed CLI path must be one ordinary file directly under bin");
        }
        let parent = path.parent().context("installed CLI has no parent")?;
        if !fs::symlink_metadata(owned_root)?.file_type().is_dir()
            || !fs::symlink_metadata(parent)?.file_type().is_dir()
            || !fs::symlink_metadata(path)?.file_type().is_file()
        {
            bail!("installed CLI must be an ordinary file in the owned bin directory");
        }
        let canonical = fs::canonicalize(path)?;
        if canonical != fs::canonicalize(owned_root)?.join(relative) {
            bail!("installed CLI path resolves through an unexpected link or directory");
        }
        let mut file = File::open(&canonical)?;
        let before = file.metadata()?;
        if !before.is_file() || before.len() == 0 || before.len() > limit {
            bail!("installed CLI image must be a nonempty regular file within {limit} bytes");
        }
        let mut reader = (&mut file).take(limit + 1);
        let mut digest = Sha256::new();
        let mut buffer = [0_u8; 16 * 1024];
        let mut size_bytes = 0;
        loop {
            let n = reader.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            size_bytes += n as u64;
            if size_bytes > limit {
                bail!("installed CLI image exceeded {limit} bytes while reading");
            }
            digest.update(&buffer[..n]);
        }
        // Compare observations, not a claim that the filesystem prevented a
        // concurrent rename/replacement or that these bytes were executed.
        let current = fs::symlink_metadata(path)?;
        if size_bytes != before.len()
            || size_bytes != file.metadata()?.len()
            || !current.file_type().is_file()
            || size_bytes != current.len()
            || canonical != fs::canonicalize(path)?
        {
            bail!("installed CLI image changed while its bytes were sampled");
        }
        Ok(Self {
            size_bytes,
            sha256: format!("{:x}", digest.finalize()),
        })
    }

    pub(super) fn verify(&self, owned_root: &Path, path: &Path) -> Result<()> {
        let actual = Self::capture(owned_root, path)?;
        if actual != *self {
            bail!(
                "installed CLI image differs from installation: expected {} bytes / SHA-256 {}, observed {} bytes / SHA-256 {}",
                self.size_bytes,
                self.sha256,
                actual.size_bytes,
                actual.sha256,
            );
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_installed_image_receipt_keeps_one_exact_bin_basename() {
        let relative = Path::new("bin").join("adze fixture.exe");
        assert_eq!(receipt_path(&relative).unwrap(), "bin/adze fixture.exe");
        let ambiguous = Path::new("bin").join(r"adze\fixture.exe");
        assert!(receipt_path(&ambiguous).is_err());
        let nested = Path::new("bin").join("adze").join("fixture.exe");
        assert!(receipt_path(&nested).is_err());
    }

    #[test]
    fn test_installed_image_bounds_and_hashes_raw_bytes() {
        let root = tempfile::tempdir().unwrap();
        let bin = root.path().join("bin");
        fs::create_dir(&bin).unwrap();
        let path = bin.join("adze");
        fs::write(&path, [0, 0xff, 1]).unwrap();
        let image = InstalledImage::capture_bounded(root.path(), &path, 3).unwrap();
        assert_eq!(image.size_bytes, 3);
        assert_eq!(
            image.sha256,
            "47ffa3ea45a70b8a41c2c0825df323c00a8b7a01c1ea06083cc41dddcc001123"
        );
        let error = InstalledImage::capture_bounded(root.path(), &path, 2).unwrap_err();
        assert!(error.to_string().contains("within 2 bytes"), "{error:#}");
        fs::write(&path, []).unwrap();
        let error = InstalledImage::capture_bounded(root.path(), &path, 3).unwrap_err();
        assert!(error.to_string().contains("nonempty"), "{error:#}");
    }

    #[cfg(unix)]
    #[test]
    fn test_installed_image_rejects_file_and_bin_directory_links() {
        use std::os::unix::fs::symlink;

        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let external = outside.path().join("adze");
        fs::write(&external, b"ordinary fixture bytes").unwrap();
        let bin = root.path().join("bin");
        fs::create_dir(&bin).unwrap();
        let link = bin.join("adze");
        symlink(&external, &link).unwrap();
        assert!(InstalledImage::capture(root.path(), &link).is_err());

        fs::remove_file(&link).unwrap();
        fs::remove_dir(&bin).unwrap();
        symlink(outside.path(), &bin).unwrap();
        assert!(InstalledImage::capture(root.path(), &link).is_err());
        assert!(InstalledImage::capture(root.path(), &external).is_err());
    }
}
