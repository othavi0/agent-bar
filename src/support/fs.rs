//! Injectable filesystem seam for deterministic tests and plugin transactions.

use std::io;
use std::path::Path;
use std::time::SystemTime;

/// Metadata subset required by settings, cache, and plugin transactions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileMetadata {
    pub len: u64,
    pub modified: Option<SystemTime>,
    pub is_dir: bool,
    pub is_symlink: bool,
}

/// Read-only filesystem operations used by pure domain code.
pub trait FileSystem: Send + Sync {
    fn read(&self, path: &Path) -> io::Result<Vec<u8>>;
    fn metadata(&self, path: &Path) -> io::Result<FileMetadata>;
    fn exists(&self, path: &Path) -> bool {
        path.exists()
    }
}

/// Production filesystem backed by `std::fs`.
#[derive(Debug, Default, Clone, Copy)]
pub struct RealFileSystem;

impl FileSystem for RealFileSystem {
    fn read(&self, path: &Path) -> io::Result<Vec<u8>> {
        std::fs::read(path)
    }

    fn metadata(&self, path: &Path) -> io::Result<FileMetadata> {
        let meta = std::fs::symlink_metadata(path)?;
        Ok(FileMetadata {
            len: meta.len(),
            modified: meta.modified().ok(),
            is_dir: meta.is_dir(),
            is_symlink: meta.file_type().is_symlink(),
        })
    }

    fn exists(&self, path: &Path) -> bool {
        path.exists()
    }
}
