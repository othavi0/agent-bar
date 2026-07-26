//! Shared/exclusive maintenance gate for settings, cache, and plugin work.

use std::fs::{File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use fs2::FileExt;

/// Cross-process advisory lock coordinating shared status/settings work with
/// exclusive maintenance transactions.
#[derive(Debug)]
pub struct MaintenanceGate {
    path: PathBuf,
}

/// RAII guard holding a shared maintenance lock.
#[derive(Debug)]
pub struct SharedMaintenanceGuard {
    file: File,
}

/// RAII guard holding an exclusive maintenance lock.
#[derive(Debug)]
pub struct ExclusiveMaintenanceGuard {
    file: File,
}

impl Drop for SharedMaintenanceGuard {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self.file);
    }
}

impl Drop for ExclusiveMaintenanceGuard {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self.file);
    }
}

impl MaintenanceGate {
    /// Open (or create) the lock file at `path`.
    pub fn open(path: impl Into<PathBuf>) -> io::Result<Self> {
        let path = path.into();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        // Ensure the lock file exists for reopen() callers.
        let _file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)?;
        Ok(Self { path })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Block until a shared lock is available.
    pub fn lock_shared(&self) -> io::Result<SharedMaintenanceGuard> {
        let file = self.reopen()?;
        // Prefer fs2 over std::fs::File locks (Rust 1.89+) for stable io::Error.
        FileExt::lock_shared(&file)?;
        Ok(SharedMaintenanceGuard { file })
    }

    /// Non-blocking shared lock attempt.
    pub fn try_lock_shared(&self) -> io::Result<Option<SharedMaintenanceGuard>> {
        let file = self.reopen()?;
        match FileExt::try_lock_shared(&file) {
            Ok(()) => Ok(Some(SharedMaintenanceGuard { file })),
            Err(err) if err.kind() == io::ErrorKind::WouldBlock || is_lock_busy(&err) => Ok(None),
            Err(err) => Err(err),
        }
    }

    /// Block until an exclusive lock is available.
    pub fn lock_exclusive(&self) -> io::Result<ExclusiveMaintenanceGuard> {
        let file = self.reopen()?;
        FileExt::lock_exclusive(&file)?;
        Ok(ExclusiveMaintenanceGuard { file })
    }

    /// Non-blocking exclusive lock attempt.
    pub fn try_lock_exclusive(&self) -> io::Result<Option<ExclusiveMaintenanceGuard>> {
        let file = self.reopen()?;
        match FileExt::try_lock_exclusive(&file) {
            Ok(()) => Ok(Some(ExclusiveMaintenanceGuard { file })),
            Err(err) if err.kind() == io::ErrorKind::WouldBlock || is_lock_busy(&err) => Ok(None),
            Err(err) => Err(err),
        }
    }

    fn reopen(&self) -> io::Result<File> {
        OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&self.path)
    }
}

fn is_lock_busy(err: &io::Error) -> bool {
    // Linux: EAGAIN/EWOULDBLOCK == 11
    const EWOULDBLOCK: i32 = 11;
    matches!(err.raw_os_error(), Some(EWOULDBLOCK))
        || err.to_string().to_ascii_lowercase().contains("would block")
}

/// Shared ownership wrapper for process-wide gate injection in stores.
pub type SharedMaintenanceGate = Arc<MaintenanceGate>;

pub fn shared_gate(path: impl Into<PathBuf>) -> io::Result<SharedMaintenanceGate> {
    Ok(Arc::new(MaintenanceGate::open(path)?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn exclusive_blocks_shared_try_lock() {
        let dir = tempfile::tempdir().unwrap();
        let gate = MaintenanceGate::open(dir.path().join("maintenance.lock")).unwrap();
        let exclusive = gate.lock_exclusive().unwrap();
        assert!(gate.try_lock_shared().unwrap().is_none());
        drop(exclusive);
        assert!(gate.try_lock_shared().unwrap().is_some());
    }

    #[test]
    fn shared_allows_additional_shared() {
        let dir = tempfile::tempdir().unwrap();
        let gate = MaintenanceGate::open(dir.path().join("maintenance.lock")).unwrap();
        let a = gate.lock_shared().unwrap();
        let b = gate.try_lock_shared().unwrap();
        assert!(b.is_some());
        drop(a);
        drop(b);
    }

    #[test]
    fn exclusive_holder_is_released_across_threads() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("maintenance.lock");
        let gate = Arc::new(MaintenanceGate::open(&path).unwrap());
        let exclusive = gate.lock_exclusive().unwrap();

        let gate2 = Arc::clone(&gate);
        let (tx, rx) = mpsc::channel();
        let handle = thread::spawn(move || {
            // Must not obtain shared while exclusive is held.
            assert!(gate2.try_lock_shared().unwrap().is_none());
            tx.send(()).unwrap();
            let _shared = gate2.lock_shared().unwrap();
        });

        rx.recv_timeout(Duration::from_secs(2)).unwrap();
        thread::sleep(Duration::from_millis(50));
        drop(exclusive);
        handle.join().unwrap();
    }
}
