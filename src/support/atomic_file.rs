//! Same-filesystem atomic file replacement with restrictive permissions.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

/// Failure injection points for atomic replacement tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AtomicFailPoint {
    Write,
    FsyncTemp,
    Rename,
    FsyncDir,
}

/// Mutable filesystem operations used by [`replace_atomically_with`].
pub trait FileMutator: Send + Sync {
    fn create_temp(&self, dir: &Path) -> io::Result<(PathBuf, File)>;
    fn write_all(&self, file: &mut File, bytes: &[u8]) -> io::Result<()>;
    fn sync_data(&self, file: &File) -> io::Result<()>;
    fn set_mode(&self, path: &Path, mode: u32) -> io::Result<()>;
    fn rename(&self, from: &Path, to: &Path) -> io::Result<()>;
    fn sync_dir(&self, dir: &Path) -> io::Result<()>;
}

/// Production mutator backed by `std::fs`.
#[derive(Debug, Default, Clone, Copy)]
pub struct StdFileMutator;

impl FileMutator for StdFileMutator {
    fn create_temp(&self, dir: &Path) -> io::Result<(PathBuf, File)> {
        fs::create_dir_all(dir)?;
        let mut opts = OpenOptions::new();
        opts.write(true).create_new(true).read(true);
        #[cfg(unix)]
        {
            opts.mode(0o600);
        }
        // Unique temp name on the same filesystem as the target directory.
        let name = format!(
            ".agent-bar-{}.tmp",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        );
        let path = dir.join(name);
        let file = opts.open(&path)?;
        Ok((path, file))
    }

    fn write_all(&self, file: &mut File, bytes: &[u8]) -> io::Result<()> {
        file.write_all(bytes)?;
        Ok(())
    }

    fn sync_data(&self, file: &File) -> io::Result<()> {
        file.sync_data()
    }

    fn set_mode(&self, path: &Path, mode: u32) -> io::Result<()> {
        #[cfg(unix)]
        {
            let perms = fs::Permissions::from_mode(mode);
            fs::set_permissions(path, perms)
        }
        #[cfg(not(unix))]
        {
            let _ = (path, mode);
            Ok(())
        }
    }

    fn rename(&self, from: &Path, to: &Path) -> io::Result<()> {
        fs::rename(from, to)
    }

    fn sync_dir(&self, dir: &Path) -> io::Result<()> {
        let dir_file = File::open(dir)?;
        dir_file.sync_data()
    }
}

/// Atomically replace `target` with `bytes`, then set `mode` (Unix `0600`).
///
/// Writes a temporary file in the same directory, syncs, renames over the
/// target, and syncs the parent directory. On any failure after a prior file
/// existed, the previous target bytes remain intact.
pub fn replace_atomically(target: &Path, bytes: &[u8], mode: u32) -> io::Result<()> {
    replace_atomically_with(&StdFileMutator, target, bytes, mode)
}

/// Atomic replacement with an injectable filesystem mutator.
pub fn replace_atomically_with<M: FileMutator + ?Sized>(
    mutator: &M,
    target: &Path,
    bytes: &[u8],
    mode: u32,
) -> io::Result<()> {
    let parent = target.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "atomic replace target must have a parent directory",
        )
    })?;
    let (temp_path, mut temp_file) = mutator.create_temp(parent)?;
    let cleanup = |temp: &Path| {
        let _ = fs::remove_file(temp);
    };

    if let Err(err) = mutator.write_all(&mut temp_file, bytes) {
        cleanup(&temp_path);
        return Err(err);
    }
    if let Err(err) = mutator.sync_data(&temp_file) {
        cleanup(&temp_path);
        return Err(err);
    }
    // Drop the handle before rename on platforms that require it.
    drop(temp_file);

    if let Err(err) = mutator.set_mode(&temp_path, mode) {
        cleanup(&temp_path);
        return Err(err);
    }
    if let Err(err) = mutator.rename(&temp_path, target) {
        cleanup(&temp_path);
        return Err(err);
    }
    mutator.sync_dir(parent)?;
    Ok(())
}

/// Test mutator that fails at a configured step after optionally writing.
#[cfg(test)]
#[derive(Debug)]
pub struct FailingMutator {
    inner: StdFileMutator,
    fail: AtomicFailPoint,
}

#[cfg(test)]
impl FailingMutator {
    pub fn new(fail: AtomicFailPoint) -> Self {
        Self {
            inner: StdFileMutator,
            fail,
        }
    }
}

#[cfg(test)]
impl FileMutator for FailingMutator {
    fn create_temp(&self, dir: &Path) -> io::Result<(PathBuf, File)> {
        self.inner.create_temp(dir)
    }

    fn write_all(&self, file: &mut File, bytes: &[u8]) -> io::Result<()> {
        if self.fail == AtomicFailPoint::Write {
            return Err(io::Error::other("injected write failure"));
        }
        self.inner.write_all(file, bytes)
    }

    fn sync_data(&self, file: &File) -> io::Result<()> {
        if self.fail == AtomicFailPoint::FsyncTemp {
            return Err(io::Error::other("injected fsync failure"));
        }
        self.inner.sync_data(file)
    }

    fn set_mode(&self, path: &Path, mode: u32) -> io::Result<()> {
        self.inner.set_mode(path, mode)
    }

    fn rename(&self, from: &Path, to: &Path) -> io::Result<()> {
        if self.fail == AtomicFailPoint::Rename {
            return Err(io::Error::other("injected rename failure"));
        }
        self.inner.rename(from, to)
    }

    fn sync_dir(&self, dir: &Path) -> io::Result<()> {
        if self.fail == AtomicFailPoint::FsyncDir {
            return Err(io::Error::other("injected dir fsync failure"));
        }
        self.inner.sync_dir(dir)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn replace_atomically_writes_mode_0600() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("settings.json");
        replace_atomically(&target, b"{\"ok\":true}\n", 0o600).unwrap();
        let meta = fs::metadata(&target).unwrap();
        #[cfg(unix)]
        {
            assert_eq!(meta.permissions().mode() & 0o777, 0o600);
        }
        assert_eq!(fs::read(&target).unwrap(), b"{\"ok\":true}\n");
    }

    #[test]
    fn injected_failures_preserve_previous_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("settings.json");
        let previous = b"{\"schemaVersion\":1,\"kept\":true}\n";
        replace_atomically(&target, previous, 0o600).unwrap();

        for fail in [
            AtomicFailPoint::Write,
            AtomicFailPoint::FsyncTemp,
            AtomicFailPoint::Rename,
        ] {
            let mutator = FailingMutator::new(fail);
            let err = replace_atomically_with(&mutator, &target, b"NEW\n", 0o600).unwrap_err();
            assert!(!err.to_string().is_empty());
            assert_eq!(fs::read(&target).unwrap(), previous);
        }
    }
}
