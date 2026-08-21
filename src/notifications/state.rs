//! Persisted notification deduplication state (schema v2).

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use fs2::FileExt;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::cli::ProviderId;
use crate::support::atomic_file::replace_atomically;
use crate::support::maintenance_gate::SharedMaintenanceGate;

pub const NOTIFICATION_STATE_VERSION: u32 = 2;

/// How far two observed reset timestamps may drift and still describe the same
/// quota window.
///
/// The Claude usage endpoint derives `resets_at` from its own clock on every
/// response — within one envelope its three windows come back with distinct
/// microseconds — so no two collections ever agree byte-for-byte. Sixty
/// seconds swallows that drift with orders of magnitude to spare and stays
/// negligible against a 5h or 7d window.
pub const RESET_JITTER_TOLERANCE: time::Duration = time::Duration::seconds(60);

/// Severity thresholds on `usedPercent`. Duplicated in
/// `CoreView.js` because the status schema is frozen at v2 and
/// must not gain a field; `tests/severity_parity.rs` fails the build if the
/// two sides drift.
pub const CRITICAL_USED_PERCENT: f64 = 95.0;
pub const WARNING_USED_PERCENT: f64 = 90.0;

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
        if used >= CRITICAL_USED_PERCENT {
            Some(Self::Critical)
        } else if used >= WARNING_USED_PERCENT {
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
    /// Last observed reset for this window. Evidence, not identity: see
    /// `RESET_JITTER_TOLERANCE`.
    #[serde(with = "time::serde::rfc3339::option")]
    pub reset_at: Option<OffsetDateTime>,
    pub level: NotificationLevel,
    /// When the last successful dispatch happened; drives the reminder.
    #[serde(with = "time::serde::rfc3339")]
    pub notified_at: OffsetDateTime,
}

impl NotificationEntry {
    /// The one definition of a notification's identity.
    ///
    /// v1 spelled this comparison out by hand in four places; `validate`
    /// truncated the reset to whole seconds while the other three compared
    /// nanoseconds, and the disagreement blocked every write.
    pub fn key(&self) -> (&str, &str) {
        (self.provider_id.as_str(), self.window_id.as_str())
    }
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
            if !keys.insert(entry.key()) {
                return Err(NotificationStateError::DuplicateKey);
            }
        }
        Ok(())
    }

    pub fn sort_entries(&mut self) {
        self.entries.sort_by(|a, b| a.key().cmp(&b.key()));
    }

    /// True when two observed resets describe the same quota window.
    pub fn same_window(saved: Option<OffsetDateTime>, observed: Option<OffsetDateTime>) -> bool {
        match (saved, observed) {
            (None, None) => true,
            (Some(a), Some(b)) => (a - b).abs() <= RESET_JITTER_TOLERANCE,
            _ => false,
        }
    }

    pub fn entry_for(&self, provider: ProviderId, window_id: &str) -> Option<&NotificationEntry> {
        self.entries
            .iter()
            .find(|e| e.key() == (provider.as_str(), window_id))
    }

    pub fn upsert(&mut self, entry: NotificationEntry) {
        self.entries.retain(|e| e.key() != entry.key());
        self.entries.push(entry);
        self.sort_entries();
    }

    pub fn remove_key(&mut self, provider: ProviderId, window_id: &str) {
        self.entries
            .retain(|e| e.key() != (provider.as_str(), window_id));
    }

    /// Drop rows for one `Ready` provider whose window vanished from the
    /// envelope, or whose reset already elapsed on a live reading.
    ///
    /// `is_live` guards the elapsed branch specifically. A `Ready` provider
    /// can be served straight from cache for up to its TTL while still
    /// reporting the pre-reset timestamp (`for_cache_hit` in
    /// `src/status/schema.rs` clones the windows unchanged and keeps the state
    /// `Ready`), so an elapsed reset is only evidence the window restarted
    /// when this cycle actually reached the provider.
    pub fn prune_ready_provider(
        &mut self,
        provider: ProviderId,
        live_windows: &[&str],
        now: OffsetDateTime,
        is_live: bool,
    ) {
        self.entries.retain(|e| {
            if e.provider_id != provider.as_str() {
                return true;
            }
            if !live_windows.contains(&e.window_id.as_str()) {
                return false;
            }
            if !is_live {
                return true;
            }
            e.reset_at.map(|ts| ts > now).unwrap_or(true)
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
            Self::Version => write!(
                f,
                "notification state schemaVersion must be {NOTIFICATION_STATE_VERSION}"
            ),
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
            state: root.join("notification-state-v2.json"),
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
    fn sub_second_reset_jitter_is_the_same_window() {
        // The Claude usage endpoint derives resets_at from its own clock per
        // response, so the same window returns with millisecond drift. v1
        // treated that as a new key, which is what produced the notification
        // loop this test exists to prevent.
        let a = datetime!(2026-08-21 11:59:59.707742 UTC);
        let b = datetime!(2026-08-21 11:59:59.854947 UTC);
        let c = datetime!(2026-08-21 12:00:00.024238 UTC);
        assert!(NotificationState::same_window(Some(a), Some(b)));
        assert!(NotificationState::same_window(Some(a), Some(c)));
        assert!(NotificationState::same_window(None, None));
    }

    #[test]
    fn a_real_window_advance_is_not_the_same_window() {
        let now = datetime!(2026-08-21 11:59:59 UTC);
        let next_week = datetime!(2026-08-28 11:59:59 UTC);
        assert!(!NotificationState::same_window(Some(now), Some(next_week)));
        assert!(!NotificationState::same_window(Some(now), None));
        assert!(!NotificationState::same_window(None, Some(now)));
        // Just outside the tolerance, so the boundary is pinned, not implied.
        let just_past = datetime!(2026-08-21 12:01:00.001 UTC);
        assert!(!NotificationState::same_window(Some(now), Some(just_past)));
    }

    #[test]
    fn upsert_replaces_a_jittered_row_instead_of_appending() {
        let mut state = NotificationState::empty();
        state.upsert(NotificationEntry {
            provider_id: "claude".into(),
            window_id: "weekly-model:fable".into(),
            reset_at: Some(datetime!(2026-08-21 11:59:59.854947 UTC)),
            level: NotificationLevel::Warning,
            notified_at: datetime!(2026-08-21 10:31:56 UTC),
        });
        state.upsert(NotificationEntry {
            provider_id: "claude".into(),
            window_id: "weekly-model:fable".into(),
            reset_at: Some(datetime!(2026-08-21 11:59:59.707742 UTC)),
            level: NotificationLevel::Critical,
            notified_at: datetime!(2026-08-21 10:37:56 UTC),
        });
        assert_eq!(state.entries.len(), 1);
        assert_eq!(state.entries[0].level, NotificationLevel::Critical);
        // The exact document that made save() fail with "duplicate
        // notification key" on the reporting install.
        state.validate().unwrap();
    }

    #[test]
    fn prune_drops_elapsed_and_absent_windows_on_a_live_reading() {
        let now = datetime!(2026-08-21 12:00:00 UTC);
        let mut state = NotificationState::empty();
        for (window, reset) in [
            ("live", Some(datetime!(2026-08-28 12:00:00 UTC))),
            ("elapsed", Some(datetime!(2026-08-12 00:00:00 UTC))),
            ("vanished", Some(datetime!(2026-08-28 12:00:00 UTC))),
        ] {
            state.upsert(NotificationEntry {
                provider_id: "claude".into(),
                window_id: window.into(),
                reset_at: reset,
                level: NotificationLevel::Warning,
                notified_at: datetime!(2026-08-21 10:00:00 UTC),
            });
        }
        state.prune_ready_provider(ProviderId::Claude, &["live", "elapsed"], now, true);
        let kept: Vec<&str> = state.entries.iter().map(|e| e.window_id.as_str()).collect();
        assert_eq!(kept, vec!["live"]);
    }

    #[test]
    fn prune_keeps_elapsed_rows_when_the_reading_came_from_cache() {
        // A Ready provider can be served straight from cache for up to its
        // TTL (300s for Claude) while still reporting the pre-reset
        // timestamp. Treating that as proof the window restarted would rearm
        // against a reading the provider never confirmed.
        let now = datetime!(2026-08-21 12:00:00 UTC);
        let mut state = NotificationState::empty();
        state.upsert(NotificationEntry {
            provider_id: "claude".into(),
            window_id: "weekly".into(),
            reset_at: Some(datetime!(2026-08-21 11:59:59 UTC)),
            level: NotificationLevel::Critical,
            notified_at: datetime!(2026-08-21 10:00:00 UTC),
        });
        state.prune_ready_provider(ProviderId::Claude, &["weekly"], now, false);
        assert_eq!(state.entries.len(), 1, "cache reading is not evidence");
    }

    #[test]
    fn prune_leaves_other_providers_untouched() {
        let now = datetime!(2026-08-21 12:00:00 UTC);
        let mut state = NotificationState::empty();
        state.upsert(NotificationEntry {
            provider_id: "amp".into(),
            window_id: "daily".into(),
            reset_at: Some(datetime!(2026-08-12 00:00:00 UTC)),
            level: NotificationLevel::Critical,
            notified_at: datetime!(2026-08-11 23:00:00 UTC),
        });
        state.prune_ready_provider(ProviderId::Claude, &[], now, true);
        assert_eq!(state.entries.len(), 1);
    }

    #[test]
    fn entry_for_finds_a_row_regardless_of_reset_drift() {
        let mut state = NotificationState::empty();
        state.upsert(NotificationEntry {
            provider_id: "claude".into(),
            window_id: "session".into(),
            reset_at: Some(datetime!(2026-07-26 22:00:00 UTC)),
            level: NotificationLevel::Warning,
            notified_at: datetime!(2026-07-26 18:42:00 UTC),
        });
        let found = state.entry_for(ProviderId::Claude, "session").unwrap();
        assert_eq!(found.level, NotificationLevel::Warning);
        assert!(state.entry_for(ProviderId::Claude, "weekly").is_none());
    }

    #[test]
    fn store_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let store = NotificationStateStore::new(
            NotificationPaths {
                state: dir.path().join("notification-state-v2.json"),
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
            notified_at: datetime!(2026-07-26 18:42:00 UTC),
        });
        store.save(&state).unwrap();
        let loaded = store.load().unwrap();
        assert_eq!(loaded.entries.len(), 1);
        assert_eq!(loaded.entries[0].level, NotificationLevel::Critical);
    }
}
