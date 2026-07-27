//! Atomic load/store for the normalized status-v2 cache.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use fs2::FileExt;
use time::OffsetDateTime;

use super::schema::{CacheDocument, CacheSchemaError, CachedProvider};
use crate::cli::ProviderId;
use crate::status::schema::ProviderStatus;
use crate::support::atomic_file::replace_atomically;
use crate::support::maintenance_gate::SharedMaintenanceGate;

/// Cache filesystem + lock paths.
#[derive(Debug, Clone)]
pub struct CachePaths {
    pub document: PathBuf,
    pub lock: PathBuf,
}

impl CachePaths {
    pub fn from_cache_home(cache_home: impl Into<PathBuf>) -> Self {
        let root = cache_home.into().join("agent-bar");
        Self {
            document: root.join("status-v2.json"),
            lock: root.join("status.lock"),
        }
    }

    pub fn default_xdg() -> Self {
        let base = std::env::var_os("XDG_CACHE_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))
            .unwrap_or_else(|| PathBuf::from(".cache"));
        Self::from_cache_home(base)
    }
}

#[derive(Debug)]
pub enum CacheStoreError {
    Schema(CacheSchemaError),
    Io(io::Error),
    MaintenanceBlocked,
}

impl std::fmt::Display for CacheStoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Schema(err) => write!(f, "{err}"),
            Self::Io(err) => write!(f, "{err}"),
            Self::MaintenanceBlocked => write!(f, "maintenance gate held exclusively"),
        }
    }
}

impl std::error::Error for CacheStoreError {}

impl From<io::Error> for CacheStoreError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<CacheSchemaError> for CacheStoreError {
    fn from(value: CacheSchemaError) -> Self {
        Self::Schema(value)
    }
}

/// Persistent cache store with quarantine on corruption.
#[derive(Debug, Clone)]
pub struct CacheStore {
    paths: CachePaths,
    gate: SharedMaintenanceGate,
}

impl CacheStore {
    pub fn new(paths: CachePaths, gate: SharedMaintenanceGate) -> Self {
        Self { paths, gate }
    }

    pub fn paths(&self) -> &CachePaths {
        &self.paths
    }

    /// Load cache document. Missing file → empty. Corrupt → quarantine + empty.
    pub fn load(&self) -> Result<CacheDocument, CacheStoreError> {
        match fs::read(&self.paths.document) {
            Ok(bytes) => match parse_document(&bytes) {
                Ok(doc) => Ok(doc),
                Err(err) => {
                    self.quarantine(&bytes, &err.to_string())?;
                    Ok(CacheDocument::empty())
                }
            },
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(CacheDocument::empty()),
            Err(err) => Err(CacheStoreError::Io(err)),
        }
    }

    /// Merge one provider entry, increment revision, preserve siblings.
    pub fn merge_provider(
        &self,
        id: ProviderId,
        entry: CachedProvider,
        now: OffsetDateTime,
    ) -> Result<CacheDocument, CacheStoreError> {
        let _guard = self
            .gate
            .try_lock_shared()?
            .ok_or(CacheStoreError::MaintenanceBlocked)?;
        let file_lock = open_lock(&self.paths.lock)?;
        FileExt::lock_exclusive(&file_lock)?;
        let mut doc = self.load()?;
        doc.providers.insert(id.as_str().to_owned(), entry);
        doc.revision = doc.revision.saturating_add(1);
        doc.validate()?;
        // Expiry is caller-provided; `now` used only for logging/tests.
        let _ = now;
        let bytes = serde_json::to_vec_pretty(&doc).map_err(|err| {
            CacheStoreError::Schema(CacheSchemaError::InvalidJson(err.to_string()))
        })?;
        let mut with_nl = bytes;
        if !with_nl.ends_with(b"\n") {
            with_nl.push(b'\n');
        }
        replace_atomically(&self.paths.document, &with_nl, 0o600)?;
        FileExt::unlock(&file_lock)?;
        Ok(doc)
    }

    /// Replace the full document (tests / rebuild).
    pub fn replace_all(&self, doc: &CacheDocument) -> Result<(), CacheStoreError> {
        let _guard = self
            .gate
            .try_lock_shared()?
            .ok_or(CacheStoreError::MaintenanceBlocked)?;
        let file_lock = open_lock(&self.paths.lock)?;
        FileExt::lock_exclusive(&file_lock)?;
        doc.validate()?;
        let mut bytes = serde_json::to_vec_pretty(doc).map_err(|err| {
            CacheStoreError::Schema(CacheSchemaError::InvalidJson(err.to_string()))
        })?;
        if !bytes.ends_with(b"\n") {
            bytes.push(b'\n');
        }
        replace_atomically(&self.paths.document, &bytes, 0o600)?;
        FileExt::unlock(&file_lock)?;
        Ok(())
    }

    fn quarantine(&self, bytes: &[u8], reason: &str) -> Result<(), CacheStoreError> {
        if let Some(parent) = self.paths.document.parent() {
            fs::create_dir_all(parent)?;
        }
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let dest = self
            .paths
            .document
            .with_extension(format!("corrupt-{stamp}.json"));
        let _ = fs::write(&dest, bytes);
        let _ = fs::remove_file(&self.paths.document);
        log::warn!("quarantined corrupt cache ({}): {}", reason, dest.display());
        Ok(())
    }
}

fn parse_document(bytes: &[u8]) -> Result<CacheDocument, CacheSchemaError> {
    let doc: CacheDocument = serde_json::from_slice(bytes)
        .map_err(|err| CacheSchemaError::InvalidJson(err.to_string()))?;
    doc.validate()?;
    Ok(doc)
}

fn open_lock(path: &Path) -> io::Result<fs::File> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
}

/// Build a cache entry from a live provider status and TTL.
pub fn entry_from_status(
    status: ProviderStatus,
    started_at: OffsetDateTime,
    completed_at: OffsetDateTime,
    ttl: std::time::Duration,
) -> CachedProvider {
    let expires_at = completed_at + ttl;
    CachedProvider {
        started_at,
        completed_at,
        expires_at,
        status,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::status::schema::{DataSource, ProviderStatus};
    use crate::support::maintenance_gate::MaintenanceGate;
    use std::sync::Arc;
    use time::macros::datetime;

    fn store_in(dir: &Path) -> CacheStore {
        CacheStore::new(
            CachePaths {
                document: dir.join("status-v2.json"),
                lock: dir.join("status.lock"),
            },
            Arc::new(MaintenanceGate::open(dir.join("maintenance.lock")).unwrap()),
        )
    }

    fn ready_status() -> ProviderStatus {
        ProviderStatus::ready(
            ProviderId::Claude,
            "Claude",
            DataSource::Live,
            None,
            None,
            vec![],
            datetime!(2026-07-26 18:42:00 UTC),
        )
        .unwrap()
    }

    #[test]
    fn missing_load_is_empty() {
        let dir = tempfile::tempdir().unwrap();
        let store = store_in(dir.path());
        let doc = store.load().unwrap();
        assert_eq!(doc.revision, 0);
        assert!(doc.providers.is_empty());
    }

    #[test]
    fn expiry_at_or_after_expires_at() {
        let now = datetime!(2026-07-26 18:47:01 UTC);
        let mut doc = CacheDocument::empty();
        doc.providers.insert(
            "claude".into(),
            CachedProvider {
                started_at: datetime!(2026-07-26 18:42:00 UTC),
                completed_at: datetime!(2026-07-26 18:42:01 UTC),
                expires_at: datetime!(2026-07-26 18:47:01 UTC),
                status: ready_status(),
            },
        );
        assert!(!doc.is_fresh(ProviderId::Claude, now));
        assert!(doc.is_fresh(ProviderId::Claude, datetime!(2026-07-26 18:47:00 UTC)));
    }

    #[test]
    fn merge_preserves_siblings_and_increments_revision() {
        let dir = tempfile::tempdir().unwrap();
        let store = store_in(dir.path());
        let now = datetime!(2026-07-26 18:42:00 UTC);
        let claude = entry_from_status(
            ready_status(),
            now,
            now,
            std::time::Duration::from_secs(300),
        );
        store
            .merge_provider(ProviderId::Claude, claude, now)
            .unwrap();
        let codex_status = ProviderStatus::ready(
            ProviderId::Codex,
            "Codex",
            DataSource::Live,
            None,
            None,
            vec![],
            now,
        )
        .unwrap();
        let codex = entry_from_status(codex_status, now, now, std::time::Duration::from_secs(90));
        let doc = store.merge_provider(ProviderId::Codex, codex, now).unwrap();
        assert_eq!(doc.revision, 2);
        assert!(doc.providers.contains_key("claude"));
        assert!(doc.providers.contains_key("codex"));
    }

    #[test]
    fn corrupt_cache_is_quarantined() {
        let dir = tempfile::tempdir().unwrap();
        let store = store_in(dir.path());
        fs::create_dir_all(store.paths.document.parent().unwrap()).unwrap();
        fs::write(&store.paths.document, b"{not-json").unwrap();
        let doc = store.load().unwrap();
        assert!(doc.providers.is_empty());
        let corrupt: Vec<_> = fs::read_dir(store.paths.document.parent().unwrap())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().contains("corrupt-"))
            .collect();
        assert!(!corrupt.is_empty());
    }

    #[test]
    fn exclusive_maintenance_blocks_merge() {
        let dir = tempfile::tempdir().unwrap();
        let store = store_in(dir.path());
        let exclusive = store.gate.lock_exclusive().unwrap();
        let now = datetime!(2026-07-26 18:42:00 UTC);
        let entry = entry_from_status(
            ready_status(),
            now,
            now,
            std::time::Duration::from_secs(300),
        );
        let err = store
            .merge_provider(ProviderId::Claude, entry, now)
            .unwrap_err();
        assert!(matches!(err, CacheStoreError::MaintenanceBlocked));
        drop(exclusive);
    }
}
