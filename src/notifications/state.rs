//! Persisted notification deduplication state (schema v1).

use std::cmp::Ordering;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use fs2::FileExt;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::cli::ProviderId;
use crate::support::atomic_file::replace_atomically;
use crate::support::maintenance_gate::SharedMaintenanceGate;

pub const NOTIFICATION_STATE_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum NotificationLevel {
    Warning,
    Critical,
}

impl NotificationLevel {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Warning => "warning",
            Self::Critical => "critical",
        }
    }

    pub fn from_used_percent(used: f64) -> Option<Self> {
        if used >= 95.0 {
            Some(Self::Critical)
        } else if used >= 90.0 {
            Some(Self::Warning)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NotificationEntry {
    pub provider_id: String,
    pub window_id: String,
    #[serde(with = "time::serde::rfc3339::option")]
    pub reset_at: Option<OffsetDateTime>,
    pub level: NotificationLevel,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NotificationState {
    pub schema_version: u32,
    pub entries: Vec<NotificationEntry>,
}

impl NotificationState {
    pub fn empty() -> Self {
        Self {
            schema_version: NOTIFICATION_STATE_VERSION,
            entries: Vec::new(),
        }
    }

    pub fn validate(&self) -> Result<(), NotificationStateError> {
        if self.schema_version != NOTIFICATION_STATE_VERSION {
            return Err(NotificationStateError::Version);
        }
        let mut keys = std::collections::HashSet::new();
        for entry in &self.entries {
            if ProviderId::parse_word(&entry.provider_id).is_none() {
                return Err(NotificationStateError::UnknownProvider(
                    entry.provider_id.clone(),
                ));
            }
            let key = (
                entry.provider_id.clone(),
                entry.window_id.clone(),
                entry.reset_at.map(|t| t.unix_timestamp()),
            );
            if !keys.insert(key) {
                return Err(NotificationStateError::DuplicateKey);
            }
        }
        Ok(())
    }

    pub fn sort_entries(&mut self) {
        self.entries
            .sort_by(|a, b| match a.provider_id.cmp(&b.provider_id) {
                Ordering::Equal => match a.window_id.cmp(&b.window_id) {
                    Ordering::Equal => match (a.reset_at, b.reset_at) {
                        (None, None) => Ordering::Equal,
                        (None, Some(_)) => Ordering::Less,
                        (Some(_), None) => Ordering::Greater,
                        (Some(x), Some(y)) => x.cmp(&y),
                    },
                    other => other,
                },
                other => other,
            });
    }

    pub fn level_for(
        &self,
        provider: ProviderId,
        window_id: &str,
        reset_at: Option<OffsetDateTime>,
    ) -> Option<NotificationLevel> {
        self.entries.iter().find_map(|e| {
            if e.provider_id == provider.as_str()
                && e.window_id == window_id
                && e.reset_at == reset_at
            {
                Some(e.level)
            } else {
                None
            }
        })
    }

    pub fn upsert(&mut self, entry: NotificationEntry) {
        self.entries.retain(|e| {
            !(e.provider_id == entry.provider_id
                && e.window_id == entry.window_id
                && e.reset_at == entry.reset_at)
        });
        self.entries.push(entry);
        self.sort_entries();
    }

    pub fn remove_key(
        &mut self,
        provider: ProviderId,
        window_id: &str,
        reset_at: Option<OffsetDateTime>,
    ) {
        self.entries.retain(|e| {
            !(e.provider_id == provider.as_str()
                && e.window_id == window_id
                && e.reset_at == reset_at)
        });
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotificationStateError {
    Version,
    UnknownProvider(String),
    DuplicateKey,
    InvalidJson(String),
}

impl std::fmt::Display for NotificationStateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Version => write!(f, "notification state schemaVersion must be 1"),
            Self::UnknownProvider(id) => write!(f, "unknown notification provider '{id}'"),
            Self::DuplicateKey => write!(f, "duplicate notification key"),
            Self::InvalidJson(msg) => write!(f, "invalid notification state: {msg}"),
        }
    }
}

impl std::error::Error for NotificationStateError {}

#[derive(Debug, Clone)]
pub struct NotificationPaths {
    pub state: PathBuf,
    pub lock: PathBuf,
}

impl NotificationPaths {
    pub fn from_cache_home(cache_home: impl Into<PathBuf>) -> Self {
        let root = cache_home.into().join("agent-bar");
        Self {
            state: root.join("notification-state-v1.json"),
            lock: root.join("notification.lock"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct NotificationStateStore {
    paths: NotificationPaths,
    gate: SharedMaintenanceGate,
}

impl NotificationStateStore {
    pub fn new(paths: NotificationPaths, gate: SharedMaintenanceGate) -> Self {
        Self { paths, gate }
    }

    pub fn load(&self) -> Result<NotificationState, io::Error> {
        match fs::read(&self.paths.state) {
            Ok(bytes) => match parse_state(&bytes) {
                Ok(state) => Ok(state),
                Err(err) => {
                    quarantine(&self.paths.state, &bytes, &err.to_string())?;
                    Ok(NotificationState::empty())
                }
            },
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(NotificationState::empty()),
            Err(err) => Err(err),
        }
    }

    pub fn save(&self, state: &NotificationState) -> Result<(), io::Error> {
        let _guard = self
            .gate
            .try_lock_shared()
            .map_err(io::Error::other)?
            .ok_or_else(|| io::Error::new(io::ErrorKind::WouldBlock, "maintenance blocked"))?;
        let lock = open_lock(&self.paths.lock)?;
        FileExt::lock_exclusive(&lock)?;
        let mut sorted = state.clone();
        sorted.sort_entries();
        sorted
            .validate()
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err.to_string()))?;
        let mut bytes = serde_json::to_vec_pretty(&sorted)
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err.to_string()))?;
        if !bytes.ends_with(b"\n") {
            bytes.push(b'\n');
        }
        replace_atomically(&self.paths.state, &bytes, 0o600)?;
        FileExt::unlock(&lock)?;
        Ok(())
    }
}

fn parse_state(bytes: &[u8]) -> Result<NotificationState, NotificationStateError> {
    let state: NotificationState = serde_json::from_slice(bytes)
        .map_err(|err| NotificationStateError::InvalidJson(err.to_string()))?;
    state.validate()?;
    Ok(state)
}

fn quarantine(path: &Path, bytes: &[u8], reason: &str) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let dest = path.with_extension(format!("corrupt-{stamp}.json"));
    let _ = fs::write(&dest, bytes);
    let _ = fs::remove_file(path);
    log::warn!(
        "quarantined corrupt notification state ({reason}): {}",
        dest.display()
    );
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::support::maintenance_gate::MaintenanceGate;
    use std::sync::Arc;
    use time::macros::datetime;

    #[test]
    fn thresholds() {
        assert_eq!(NotificationLevel::from_used_percent(89.9), None);
        assert_eq!(
            NotificationLevel::from_used_percent(90.0),
            Some(NotificationLevel::Warning)
        );
        assert_eq!(
            NotificationLevel::from_used_percent(95.0),
            Some(NotificationLevel::Critical)
        );
    }

    #[test]
    fn rearm_on_reset_change() {
        let mut state = NotificationState::empty();
        state.upsert(NotificationEntry {
            provider_id: "claude".into(),
            window_id: "session".into(),
            reset_at: Some(datetime!(2026-07-26 22:00:00 UTC)),
            level: NotificationLevel::Warning,
        });
        assert!(state
            .level_for(
                ProviderId::Claude,
                "session",
                Some(datetime!(2026-07-26 23:00:00 UTC))
            )
            .is_none());
    }

    #[test]
    fn store_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let store = NotificationStateStore::new(
            NotificationPaths {
                state: dir.path().join("notification-state-v1.json"),
                lock: dir.path().join("notification.lock"),
            },
            Arc::new(MaintenanceGate::open(dir.path().join("m.lock")).unwrap()),
        );
        let mut state = NotificationState::empty();
        state.upsert(NotificationEntry {
            provider_id: "claude".into(),
            window_id: "session".into(),
            reset_at: None,
            level: NotificationLevel::Critical,
        });
        store.save(&state).unwrap();
        let loaded = store.load().unwrap();
        assert_eq!(loaded.entries.len(), 1);
        assert_eq!(loaded.entries[0].level, NotificationLevel::Critical);
    }
}
