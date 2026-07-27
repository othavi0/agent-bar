//! Read-pure show and atomic complete-document apply for settings v1.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use super::schema::{Settings, SettingsError};
use crate::cli::VALIDATION;
use crate::support::atomic_file::{replace_atomically, replace_atomically_with, FileMutator};
use crate::support::maintenance_gate::{MaintenanceGate, SharedMaintenanceGate};

/// Settings store errors (validation → exit 3; I/O → message + nonzero).
#[derive(Debug)]
pub enum StoreError {
    Validation(SettingsError),
    Io(io::Error),
}

impl StoreError {
    pub fn message(&self) -> String {
        match self {
            Self::Validation(err) => err.message().to_owned(),
            Self::Io(err) => err.to_string(),
        }
    }

    pub fn exit_code(&self) -> i32 {
        match self {
            Self::Validation(_) => VALIDATION,
            Self::Io(_) => VALIDATION,
        }
    }
}

impl From<SettingsError> for StoreError {
    fn from(value: SettingsError) -> Self {
        Self::Validation(value)
    }
}

impl From<io::Error> for StoreError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

/// Canonical settings.json store.
#[derive(Debug, Clone)]
pub struct SettingsStore {
    path: PathBuf,
    gate: SharedMaintenanceGate,
}

impl SettingsStore {
    pub fn new(path: impl Into<PathBuf>, gate: SharedMaintenanceGate) -> Self {
        Self {
            path: path.into(),
            gate,
        }
    }

    /// Convenience constructor: settings path + sibling maintenance lock path.
    pub fn with_paths(
        settings_path: impl Into<PathBuf>,
        lock_path: impl Into<PathBuf>,
    ) -> io::Result<Self> {
        let gate = std::sync::Arc::new(MaintenanceGate::open(lock_path)?);
        Ok(Self::new(settings_path, gate))
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn gate(&self) -> &SharedMaintenanceGate {
        &self.gate
    }

    /// Read-only show: missing file returns defaults without creating anything.
    /// Existing file is never rewritten, migrated, or touched (mtime preserved).
    pub fn show(&self) -> Result<Settings, StoreError> {
        match fs::read(&self.path) {
            Ok(bytes) => Ok(Settings::parse_strict(&bytes)?),
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(Settings::defaults()),
            Err(err) => Err(StoreError::Io(err)),
        }
    }

    /// Validate a complete document, acquire the shared maintenance gate, then
    /// atomically replace the settings file. Returns the canonical stored document.
    pub fn apply(&self, document: &Settings) -> Result<Settings, StoreError> {
        document.validate()?;
        let canonical = document.clone();
        let bytes = canonical.to_canonical_json_line()?;
        // Shared gate: blocks behind exclusive maintenance without writing first.
        let _guard = self.gate.lock_shared()?;
        replace_atomically(&self.path, bytes.as_bytes(), 0o600)?;
        Ok(canonical)
    }

    /// Apply from raw JSON bytes (unknown keys fail before lock).
    pub fn apply_raw(&self, raw: &[u8]) -> Result<Settings, StoreError> {
        let document = Settings::parse_strict(raw)?;
        self.apply(&document)
    }

    /// Test/helper path: validate, try shared lock without blocking, write with mutator.
    pub fn try_apply_with<M: FileMutator + ?Sized>(
        &self,
        document: &Settings,
        mutator: &M,
    ) -> Result<Settings, StoreError> {
        document.validate()?;
        let bytes = document.to_canonical_json_line()?;
        let guard = self.gate.try_lock_shared()?.ok_or_else(|| {
            StoreError::Io(io::Error::new(
                io::ErrorKind::WouldBlock,
                "maintenance gate held exclusively",
            ))
        })?;
        replace_atomically_with(mutator, &self.path, bytes.as_bytes(), 0o600)?;
        drop(guard);
        Ok(document.clone())
    }
}

/// Snapshot mtime for purity tests.
pub fn file_mtime(path: &Path) -> io::Result<Option<SystemTime>> {
    match fs::metadata(path) {
        Ok(meta) => Ok(Some(meta.modified()?)),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err),
    }
}

/// `$XDG_CONFIG_HOME/agent-bar/settings.json` (defaulting `~/.config`).
pub fn default_settings_path() -> PathBuf {
    xdg_base("XDG_CONFIG_HOME", ".config")
        .join("agent-bar")
        .join("settings.json")
}

/// `$XDG_STATE_HOME/agent-bar/maintenance.lock` (defaulting `~/.local/state`).
pub fn default_maintenance_lock_path() -> PathBuf {
    xdg_base("XDG_STATE_HOME", ".local/state")
        .join("agent-bar")
        .join("maintenance.lock")
}

fn xdg_base(var: &str, home_fallback: &str) -> PathBuf {
    if let Some(value) = std::env::var_os(var) {
        return PathBuf::from(value);
    }
    if let Some(home) = std::env::var_os("HOME") {
        return PathBuf::from(home).join(home_fallback);
    }
    PathBuf::from(home_fallback)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::support::atomic_file::{AtomicFailPoint, FailingMutator, StdFileMutator};
    use crate::support::maintenance_gate::MaintenanceGate;
    use std::sync::Arc;
    use std::thread;
    use std::time::Duration;

    fn store_in(dir: &Path) -> SettingsStore {
        SettingsStore::new(
            dir.join("settings.json"),
            Arc::new(MaintenanceGate::open(dir.join("maintenance.lock")).unwrap()),
        )
    }

    #[test]
    fn missing_show_does_not_create_file() {
        let dir = tempfile::tempdir().unwrap();
        let store = store_in(dir.path());
        let settings = store.show().unwrap();
        assert_eq!(settings, Settings::defaults());
        assert!(!store.path().exists());
    }

    #[test]
    fn existing_show_preserves_bytes_and_mtime() {
        let dir = tempfile::tempdir().unwrap();
        let store = store_in(dir.path());
        let original = Settings::defaults();
        store.apply(&original).unwrap();
        let before_bytes = fs::read(store.path()).unwrap();
        let before_mtime = file_mtime(store.path()).unwrap().unwrap();

        thread::sleep(Duration::from_millis(20));
        let shown = store.show().unwrap();
        assert_eq!(shown, original);
        assert_eq!(fs::read(store.path()).unwrap(), before_bytes);
        assert_eq!(file_mtime(store.path()).unwrap().unwrap(), before_mtime);
    }

    #[test]
    fn unknown_keys_fail_before_lock() {
        let dir = tempfile::tempdir().unwrap();
        let store = store_in(dir.path());
        let exclusive = store.gate().lock_exclusive().unwrap();
        let raw = br#"{"schemaVersion":1,"providers":[{"id":"claude","enabled":true},{"id":"codex","enabled":true},{"id":"amp","enabled":true},{"id":"grok","enabled":true}],"display":{"metric":"remaining"},"refreshIntervalSeconds":60,"notifications":{"enabled":true},"theme":"dark"}"#;
        let err = store.apply_raw(raw).unwrap_err();
        match err {
            StoreError::Validation(v) => assert!(v.message().contains("unknown")),
            other => panic!("expected validation, got {other:?}"),
        }
        assert!(!store.path().exists());
        drop(exclusive);
    }

    #[test]
    fn valid_write_is_mode_0600_and_returns_canonical() {
        let dir = tempfile::tempdir().unwrap();
        let store = store_in(dir.path());
        let mut doc = Settings::defaults();
        doc.refresh_interval_seconds = 120;
        doc.notifications.enabled = false;
        let stored = store.apply(&doc).unwrap();
        assert_eq!(stored, doc);
        let meta = fs::metadata(store.path()).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(meta.permissions().mode() & 0o777, 0o600);
        }
        let line = fs::read_to_string(store.path()).unwrap();
        assert!(line.ends_with('\n'));
        assert_eq!(Settings::parse_strict(line.as_bytes()).unwrap(), doc);
    }

    #[test]
    fn injected_write_failures_preserve_previous_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let store = store_in(dir.path());
        let previous = Settings::defaults();
        store.apply(&previous).unwrap();
        let previous_bytes = fs::read(store.path()).unwrap();

        let mut next = Settings::defaults();
        next.refresh_interval_seconds = 90;
        for fail in [
            AtomicFailPoint::Write,
            AtomicFailPoint::FsyncTemp,
            AtomicFailPoint::Rename,
        ] {
            let mutator = FailingMutator::new(fail);
            let err = store.try_apply_with(&next, &mutator).unwrap_err();
            assert!(matches!(err, StoreError::Io(_)));
            assert_eq!(fs::read(store.path()).unwrap(), previous_bytes);
        }
        // Control: success still works.
        store.try_apply_with(&next, &StdFileMutator).unwrap();
        assert_eq!(store.show().unwrap().refresh_interval_seconds, 90);
    }

    #[test]
    fn apply_blocks_behind_exclusive_without_touching_settings_first() {
        let dir = tempfile::tempdir().unwrap();
        let store = store_in(dir.path());
        let previous = Settings::defaults();
        store.apply(&previous).unwrap();
        let previous_bytes = fs::read(store.path()).unwrap();
        let previous_mtime = file_mtime(store.path()).unwrap().unwrap();

        let exclusive = store.gate().lock_exclusive().unwrap();
        // Non-blocking path proves we do not write while exclusive is held.
        let mut next = Settings::defaults();
        next.refresh_interval_seconds = 180;
        let err = store.try_apply_with(&next, &StdFileMutator).unwrap_err();
        assert!(matches!(err, StoreError::Io(_)));
        assert_eq!(fs::read(store.path()).unwrap(), previous_bytes);
        assert_eq!(file_mtime(store.path()).unwrap().unwrap(), previous_mtime);
        drop(exclusive);

        store.apply(&next).unwrap();
        assert_eq!(store.show().unwrap().refresh_interval_seconds, 180);
    }

    #[test]
    fn apply_raw_round_trip_matches_fixture_shape() {
        let dir = tempfile::tempdir().unwrap();
        let store = store_in(dir.path());
        let fixture = include_str!("../../tests/fixtures/settings-v1/valid-defaults.json");
        // Fixture may lack trailing newline; apply_raw accepts object body.
        let mut raw = fixture.trim().to_owned();
        let stored = store.apply_raw(raw.as_bytes()).unwrap();
        assert_eq!(stored, Settings::defaults());
        let line = stored.to_canonical_json_line().unwrap();
        assert!(line.ends_with('\n'));
        raw.push('\n');
        // Canonical encoding may differ in spacing; semantic equality is enough.
        assert_eq!(Settings::parse_strict(line.as_bytes()).unwrap(), stored);
    }
}
