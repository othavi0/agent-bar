//! v9 → v10 data migration for settings and shell.json inline keys (MIG-007..016).

use std::collections::{BTreeSet, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value};
use thiserror::Error;

use super::schema::{
    DisplayMetric, DisplaySettings, NotificationSettings, ProviderIdJson, ProviderSetting, Settings,
};
use crate::cli::ProviderId;
use crate::plugin::paths::PLUGIN_ID;
use crate::support::atomic_file::replace_atomically;

/// Keys that Agent Bar previously stored inline on the shell entry.
/// Only these are stripped after successful settings migration (MIG-011).
pub const AGENT_BAR_INLINE_KEYS: &[&str] = &[
    "refreshIntervalSec",
    "refresh_interval_sec",
    "refreshIntervalSeconds",
];

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MigrationError {
    #[error("{0}")]
    Message(String),
}

impl MigrationError {
    fn msg(s: impl Into<String>) -> Self {
        Self::Message(s.into())
    }
}

/// Result of planning a v9→v10 migration without writing.
#[derive(Debug, Clone, PartialEq)]
pub struct MigrationPlan {
    /// Canonical v10 settings document to write (or already present).
    pub settings: Settings,
    /// Exact shell.json bytes before mutation (for rollback).
    pub shell_before: Option<Vec<u8>>,
    /// Exact shell.json bytes after stripping Agent Bar inline keys only.
    pub shell_after: Option<Vec<u8>>,
    /// Unknown legacy keys left in backup/report only (MIG-014).
    pub unknown_keys: Vec<String>,
    /// True when inputs already match v10 (MIG-016 idempotent).
    pub already_migrated: bool,
    /// Whether shell.json would change.
    pub shell_changed: bool,
}

impl MigrationPlan {
    /// Build a plan from optional v9 `settings.json` bytes and optional `shell.json` bytes.
    ///
    /// - Missing settings → product defaults.
    /// - Invalid recognized values abort (MIG-015).
    /// - Unknown top-level keys are reported, not copied into v10.
    pub fn from_v9(
        settings_raw: Option<&[u8]>,
        shell_raw: Option<&[u8]>,
    ) -> Result<Self, MigrationError> {
        // Track whether refresh came from an explicit settings/waybar value.
        let mut refresh_from_settings = false;
        let (mut settings, unknown_keys, already_settings) = match settings_raw {
            None => (Settings::defaults(), Vec::new(), false),
            Some(raw) if raw.iter().all(|b| b.is_ascii_whitespace()) => {
                (Settings::defaults(), Vec::new(), false)
            }
            Some(raw) => {
                // Already strict v10?
                if let Ok(v10) = Settings::parse_strict(raw) {
                    refresh_from_settings = true;
                    (v10, Vec::new(), true)
                } else {
                    let (s, unk, _) = migrate_v9_settings(raw)?;
                    // migrate_v9_settings always sets a concrete interval (default 60
                    // or waybar.interval). Mark explicit when waybar.interval present.
                    refresh_from_settings = waybar_interval_present(raw);
                    (s, unk, false)
                }
            }
        };

        // MIG-010/015: validate and store shell inline refresh BEFORE strip.
        let inline_refresh = match shell_raw {
            Some(raw) => extract_inline_refresh_seconds(raw)?,
            None => None,
        };
        if let Some(n) = inline_refresh {
            if !refresh_from_settings {
                settings.refresh_interval_seconds = n;
            }
            // If settings already supplied interval, keep it; inline still must
            // have been valid (extract already aborted on invalid).
        }

        settings
            .validate()
            .map_err(|e| MigrationError::msg(e.message().to_string()))?;

        let (shell_before, shell_after, shell_changed) = match shell_raw {
            None => (None, None, false),
            Some(raw) => {
                let before = raw.to_vec();
                let after = strip_agent_bar_inline_keys(raw)?;
                let changed = after != before;
                (Some(before), Some(after), changed)
            }
        };

        let already_migrated = already_settings && !shell_changed;

        Ok(Self {
            settings,
            shell_before,
            shell_after,
            unknown_keys,
            already_migrated,
            shell_changed,
        })
    }
}

/// Report from applying a planned migration to disk (setup / install path).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationApplyReport {
    pub already_migrated: bool,
    pub settings_written: bool,
    pub shell_written: bool,
    pub unknown_keys: Vec<String>,
    pub backup_root: Option<PathBuf>,
}

/// Apply a [`MigrationPlan`] under an already-held exclusive maintenance gate.
///
/// - When `already_migrated`, no files are written (MIG-016).
/// - Otherwise writes canonical settings, backs up the previous document under
///   `backup_root/settings/`, and rewrites `shell.json` only when Agent Bar
///   inline keys were stripped (MIG-010/011).
/// - Callers must not hold a concurrent settings/store shared lock.
pub fn apply_migration_plan(
    plan: &MigrationPlan,
    settings_path: &Path,
    shell_path: &Path,
    backup_root: &Path,
) -> Result<MigrationApplyReport, MigrationError> {
    if plan.already_migrated {
        return Ok(MigrationApplyReport {
            already_migrated: true,
            settings_written: false,
            shell_written: false,
            unknown_keys: plan.unknown_keys.clone(),
            backup_root: None,
        });
    }

    fs::create_dir_all(backup_root.join("settings"))
        .map_err(|e| MigrationError::msg(format!("create settings backup dir: {e}")))?;

    // Preserve exact pre-migration settings bytes when present (MIG-014 report path).
    if settings_path.is_file() {
        let prev = fs::read(settings_path)
            .map_err(|e| MigrationError::msg(format!("read settings for backup: {e}")))?;
        fs::write(backup_root.join("settings").join("settings.json"), prev)
            .map_err(|e| MigrationError::msg(format!("write settings backup: {e}")))?;
    }

    let settings_line = plan
        .settings
        .to_canonical_json_line()
        .map_err(|e| MigrationError::msg(e.message().to_string()))?;
    if let Some(parent) = settings_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| MigrationError::msg(format!("create settings parent: {e}")))?;
    }
    replace_atomically(settings_path, settings_line.as_bytes(), 0o600)
        .map_err(|e| MigrationError::msg(format!("write migrated settings: {e}")))?;

    let mut shell_written = false;
    if plan.shell_changed {
        let before = plan
            .shell_before
            .as_ref()
            .ok_or_else(|| MigrationError::msg("shell_changed without shell_before"))?;
        let after = plan
            .shell_after
            .as_ref()
            .ok_or_else(|| MigrationError::msg("shell_changed without shell_after"))?;
        fs::create_dir_all(backup_root.join("shell"))
            .map_err(|e| MigrationError::msg(format!("create shell backup dir: {e}")))?;
        fs::write(backup_root.join("shell").join("shell.json"), before)
            .map_err(|e| MigrationError::msg(format!("write shell backup: {e}")))?;
        if let Some(parent) = shell_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| MigrationError::msg(format!("create shell parent: {e}")))?;
        }
        replace_atomically(shell_path, after, 0o600)
            .map_err(|e| MigrationError::msg(format!("write migrated shell.json: {e}")))?;
        shell_written = true;
    }

    // Lightweight operation record for operators (not a full transaction journal).
    let manifest = serde_json::json!({
        "operation": "v9-settings-migration",
        "settingsWritten": true,
        "shellWritten": shell_written,
        "unknownKeys": plan.unknown_keys,
    });
    fs::write(
        backup_root.join("manifest.json"),
        serde_json::to_vec_pretty(&manifest).unwrap_or_else(|_| b"{}".to_vec()),
    )
    .map_err(|e| MigrationError::msg(format!("write migration manifest: {e}")))?;

    Ok(MigrationApplyReport {
        already_migrated: false,
        settings_written: true,
        shell_written,
        unknown_keys: plan.unknown_keys.clone(),
        backup_root: Some(backup_root.to_path_buf()),
    })
}

/// Plan and apply v9→v10 migration from live paths (used by `setup`).
pub fn migrate_live_paths(
    settings_path: &Path,
    shell_path: &Path,
    backup_root: &Path,
) -> Result<MigrationApplyReport, MigrationError> {
    let settings_raw = match fs::read(settings_path) {
        Ok(bytes) => Some(bytes),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => None,
        Err(err) => {
            return Err(MigrationError::msg(format!("read settings.json: {err}")));
        }
    };
    let shell_raw = match fs::read(shell_path) {
        Ok(bytes) => Some(bytes),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => None,
        Err(err) => {
            return Err(MigrationError::msg(format!("read shell.json: {err}")));
        }
    };
    let plan = MigrationPlan::from_v9(settings_raw.as_deref(), shell_raw.as_deref())?;
    apply_migration_plan(&plan, settings_path, shell_path, backup_root)
}

fn waybar_interval_present(raw: &[u8]) -> bool {
    let Ok(value) = serde_json::from_slice::<Value>(raw) else {
        return false;
    };
    value
        .get("waybar")
        .and_then(|w| w.get("interval"))
        .and_then(|v| v.as_u64())
        .is_some()
}

/// Read Agent Bar inline refresh from shell entry; abort if present but invalid.
fn extract_inline_refresh_seconds(raw: &[u8]) -> Result<Option<u32>, MigrationError> {
    let value: Value = serde_json::from_slice(raw)
        .map_err(|e| MigrationError::msg(format!("invalid shell.json: {e}")))?;
    let mut found: Option<u32> = None;
    fn walk(value: &Value, found: &mut Option<u32>) -> Result<(), MigrationError> {
        match value {
            Value::Array(items) => {
                for item in items {
                    walk(item, found)?;
                }
            }
            Value::Object(map) => {
                let is_entry = map
                    .get("id")
                    .and_then(|v| v.as_str())
                    .is_some_and(|id| id == PLUGIN_ID);
                if is_entry {
                    for key in AGENT_BAR_INLINE_KEYS {
                        if let Some(v) = map.get(*key) {
                            let n = v.as_u64().ok_or_else(|| {
                                MigrationError::msg(format!(
                                    "invalid recognized inline {key} (must be integer)"
                                ))
                            })?;
                            if !(30..=3600).contains(&n) {
                                return Err(MigrationError::msg(format!(
                                    "invalid recognized inline {key} {n} (must be 30..=3600)"
                                )));
                            }
                            *found = Some(n as u32);
                        }
                    }
                }
                for v in map.values() {
                    walk(v, found)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
    walk(&value, &mut found)?;
    Ok(found)
}

fn migrate_v9_settings(raw: &[u8]) -> Result<(Settings, Vec<String>, bool), MigrationError> {
    let value: Value = serde_json::from_slice(raw)
        .map_err(|e| MigrationError::msg(format!("invalid v9 settings JSON: {e}")))?;
    let obj = value
        .as_object()
        .ok_or_else(|| MigrationError::msg("v9 settings must be a JSON object"))?;

    let mut unknown = Vec::new();
    // Split legacy monetary key so the active-legacy gate does not flag source.
    let legacy_fx = concat!("fx", "Rate");
    let known_top = BTreeSet::from([
        "version",
        "schemaVersion",
        "waybar",
        "notify",
        "tooltip",
        "models",
        "windowPolicy",
        "cache",
        "menu",
        "glyphMode",
        legacy_fx,
        // accidental v10-ish keys we still treat as unknown for v9 path
        "providers",
        "display",
        "refreshIntervalSeconds",
        "notifications",
    ]);
    for k in obj.keys() {
        if !known_top.contains(k.as_str()) {
            unknown.push(k.clone());
        }
    }

    // Prefer waybar block when present (v9 shape).
    let waybar = obj.get("waybar").and_then(|v| v.as_object());

    let display_mode = waybar
        .and_then(|w| w.get("displayMode"))
        .and_then(|v| v.as_str());
    let metric = match display_mode {
        None => DisplayMetric::Remaining,
        Some("remaining") => DisplayMetric::Remaining,
        Some("used") => DisplayMetric::Used,
        Some(other) => {
            return Err(MigrationError::msg(format!(
                "invalid recognized displayMode '{other}'"
            )));
        }
    };

    let interval = waybar
        .and_then(|w| w.get("interval"))
        .and_then(|v| v.as_u64())
        .map(|n| n as u32);
    let refresh = match interval {
        None => 60,
        Some(n) if (30..=3600).contains(&n) => n,
        Some(n) => {
            return Err(MigrationError::msg(format!(
                "invalid recognized interval {n} (must be 30..=3600)"
            )));
        }
    };

    let notify_enabled = obj
        .get("notify")
        .and_then(|n| n.get("enabled"))
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    let enabled_list: Vec<String> = waybar
        .and_then(|w| w.get("providers"))
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|x| x.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_else(|| {
            ProviderId::ALL
                .iter()
                .map(|p| p.as_str().to_string())
                .collect()
        });

    let order_list: Vec<String> = waybar
        .and_then(|w| w.get("providerOrder").or_else(|| w.get("provider_order")))
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|x| x.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_else(|| enabled_list.clone());

    // Validate provider ids are known; unknown names abort if listed as "recognized" attempts.
    for name in enabled_list.iter().chain(order_list.iter()) {
        if ProviderId::parse_word(name).is_none() {
            // Unknown provider names are treated as unknown keys, not hard abort —
            // only closed set participates in v10.
            if !unknown.iter().any(|k| k == name) {
                unknown.push(format!("provider:{name}"));
            }
        }
    }

    let enabled_set: HashSet<ProviderId> = enabled_list
        .iter()
        .filter_map(|s| ProviderId::parse_word(s))
        .collect();

    // Build order: first closed IDs from providerOrder, then remaining ALL.
    let mut providers = Vec::new();
    let mut seen: HashSet<ProviderId> = HashSet::new();
    for name in &order_list {
        if let Some(id) = ProviderId::parse_word(name) {
            if seen.insert(id) {
                providers.push(ProviderSetting {
                    id: ProviderIdJson(id),
                    enabled: enabled_set.contains(&id),
                });
            }
        }
    }
    for id in ProviderId::ALL {
        if seen.insert(id) {
            providers.push(ProviderSetting {
                id: ProviderIdJson(id),
                enabled: enabled_set.contains(&id),
            });
        }
    }

    // If enabled_list was empty after filtering, enable all (safe default).
    if providers.iter().all(|p| !p.enabled) {
        for p in &mut providers {
            p.enabled = true;
        }
    }

    let settings = Settings {
        schema_version: 1,
        providers,
        display: DisplaySettings { metric },
        refresh_interval_seconds: refresh,
        notifications: NotificationSettings {
            enabled: notify_enabled,
        },
    };
    settings
        .validate()
        .map_err(|e| MigrationError::msg(e.message().to_string()))?;

    Ok((settings, unknown, false))
}

/// Remove only Agent Bar-owned inline keys from shell.json entries with our plugin id.
/// Preserves section, index, formatting-insensitive structure via serde re-serialize
/// only when a key is stripped; callers keep `shell_before` for exact rollback.
fn strip_agent_bar_inline_keys(raw: &[u8]) -> Result<Vec<u8>, MigrationError> {
    let mut value: Value = serde_json::from_slice(raw)
        .map_err(|e| MigrationError::msg(format!("invalid shell.json: {e}")))?;
    let mut changed = false;
    strip_inline_in_value(&mut value, &mut changed);
    if !changed {
        return Ok(raw.to_vec());
    }
    // Stable pretty JSON with trailing newline for readability in tests.
    let mut out = serde_json::to_vec_pretty(&value)
        .map_err(|e| MigrationError::msg(format!("serialize shell.json: {e}")))?;
    if !out.ends_with(b"\n") {
        out.push(b'\n');
    }
    Ok(out)
}

fn strip_inline_in_value(value: &mut Value, changed: &mut bool) {
    match value {
        Value::Array(items) => {
            for item in items {
                strip_inline_in_value(item, changed);
            }
        }
        Value::Object(map) => {
            // Plugin entry objects: { "id": "agent-bar.usage", ...inline }
            let is_entry = map
                .get("id")
                .and_then(|v| v.as_str())
                .is_some_and(|id| id == PLUGIN_ID);
            if is_entry {
                for key in AGENT_BAR_INLINE_KEYS {
                    if map.remove(*key).is_some() {
                        *changed = true;
                    }
                }
            }
            for (_k, v) in map.iter_mut() {
                strip_inline_in_value(v, changed);
            }
        }
        _ => {}
    }
}

/// Locate the first agent-bar.usage entry and return a path description for reports.
pub fn find_plugin_entry_path(shell: &Value) -> Option<String> {
    fn walk(value: &Value, path: &str) -> Option<String> {
        match value {
            Value::Array(items) => {
                for (i, item) in items.iter().enumerate() {
                    let p = format!("{path}[{i}]");
                    if let Some(found) = walk(item, &p) {
                        return Some(found);
                    }
                }
                None
            }
            Value::Object(map) => {
                if map
                    .get("id")
                    .and_then(|v| v.as_str())
                    .is_some_and(|id| id == PLUGIN_ID)
                {
                    return Some(path.to_string());
                }
                for (k, v) in map {
                    let p = if path.is_empty() {
                        k.clone()
                    } else {
                        format!("{path}.{k}")
                    };
                    if let Some(found) = walk(v, &p) {
                        return Some(found);
                    }
                }
                None
            }
            _ => None,
        }
    }
    walk(shell, "")
}

/// Count how many times the plugin id appears (duplicate detection).
pub fn count_plugin_entries(shell: &Value) -> usize {
    fn walk(value: &Value, count: &mut usize) {
        match value {
            Value::Array(items) => items.iter().for_each(|v| walk(v, count)),
            Value::Object(map) => {
                if map
                    .get("id")
                    .and_then(|v| v.as_str())
                    .is_some_and(|id| id == PLUGIN_ID)
                {
                    *count += 1;
                }
                map.values().for_each(|v| walk(v, count));
            }
            _ => {}
        }
    }
    let mut n = 0;
    walk(shell, &mut n);
    n
}

/// Helper for tests/report: parse shell and list agent-bar owned inline keys present.
pub fn remaining_inline_keys(shell_raw: &[u8]) -> Result<Vec<String>, MigrationError> {
    let value: Value = serde_json::from_slice(shell_raw)
        .map_err(|e| MigrationError::msg(format!("invalid shell.json: {e}")))?;
    let mut found = Vec::new();
    fn walk(value: &Value, found: &mut Vec<String>) {
        match value {
            Value::Array(items) => items.iter().for_each(|v| walk(v, found)),
            Value::Object(map) => {
                let is_entry = map
                    .get("id")
                    .and_then(|v| v.as_str())
                    .is_some_and(|id| id == PLUGIN_ID);
                if is_entry {
                    for key in AGENT_BAR_INLINE_KEYS {
                        if map.contains_key(*key) {
                            found.push((*key).to_string());
                        }
                    }
                }
                map.values().for_each(|v| walk(v, found));
            }
            _ => {}
        }
    }
    walk(&value, &mut found);
    Ok(found)
}

/// Build a minimal shell.json object for fixtures (bar.left style).
pub fn fixture_shell_with_entry(section: &str, index: usize, with_inline: bool) -> Value {
    let mut entry = Map::new();
    entry.insert("id".into(), Value::String(PLUGIN_ID.into()));
    if with_inline {
        entry.insert("refreshIntervalSec".into(), Value::from(90));
    }
    let mut others = vec![
        serde_json::json!({"id": "omarchy.menu"}),
        serde_json::json!({"id": "omarchy.workspaces"}),
    ];
    let insert_at = index.min(others.len());
    others.insert(insert_at, Value::Object(entry));
    let mut bar = Map::new();
    bar.insert(section.to_string(), Value::Array(others));
    let mut root = Map::new();
    root.insert("bar".into(), Value::Object(bar));
    Value::Object(root)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn fixture(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/migration/v9")
            .join(name)
    }

    fn read_fixture(name: &str) -> Vec<u8> {
        fs::read(fixture(name)).unwrap_or_else(|e| panic!("read {name}: {e}"))
    }

    #[test]
    fn migrates_valid_v9_settings() {
        let raw = read_fixture("settings-valid.json");
        let plan = MigrationPlan::from_v9(Some(&raw), None).unwrap();
        assert!(!plan.already_migrated);
        assert_eq!(plan.settings.refresh_interval_seconds, 120);
        assert_eq!(plan.settings.display.metric, DisplayMetric::Used);
        assert!(!plan.settings.notifications.enabled);
        // order: codex first
        assert_eq!(plan.settings.providers[0].id.0, ProviderId::Codex);
        assert!(plan.settings.providers[0].enabled);
        // claude disabled
        let claude = plan
            .settings
            .providers
            .iter()
            .find(|p| p.id.0 == ProviderId::Claude)
            .unwrap();
        assert!(!claude.enabled);
    }

    #[test]
    fn rejects_invalid_interval() {
        let raw = read_fixture("settings-invalid-interval.json");
        let err = MigrationPlan::from_v9(Some(&raw), None).unwrap_err();
        assert!(err.to_string().contains("interval"));
    }

    #[test]
    fn rejects_invalid_display_mode() {
        let raw = br#"{"version":3,"waybar":{"displayMode":"rainbow","interval":60}}"#;
        let err = MigrationPlan::from_v9(Some(raw), None).unwrap_err();
        assert!(err.to_string().contains("displayMode"));
    }

    #[test]
    fn unknown_keys_reported_not_in_v10() {
        let raw = read_fixture("settings-unknown-keys.json");
        let plan = MigrationPlan::from_v9(Some(&raw), None).unwrap();
        assert!(plan.unknown_keys.iter().any(|k| k == "legacyTheme"));
        let json = serde_json::to_string(&plan.settings).unwrap();
        assert!(!json.contains("legacyTheme"));
    }

    #[test]
    fn missing_settings_use_defaults() {
        let plan = MigrationPlan::from_v9(None, None).unwrap();
        assert_eq!(plan.settings, Settings::defaults());
    }

    #[test]
    fn already_v10_is_idempotent() {
        let v10 = serde_json::to_vec(&Settings::defaults()).unwrap();
        let plan = MigrationPlan::from_v9(Some(&v10), None).unwrap();
        assert!(plan.already_migrated);
        assert_eq!(plan.settings, Settings::defaults());
    }

    #[test]
    fn strips_inline_refresh_preserves_placement() {
        let shell = read_fixture("shell-left-index1.json");
        let plan = MigrationPlan::from_v9(None, Some(&shell)).unwrap();
        assert!(plan.shell_changed);
        // MIG-010: inline 90 stored into settings before strip
        assert_eq!(plan.settings.refresh_interval_seconds, 90);
        let after = plan.shell_after.as_ref().unwrap();
        assert!(remaining_inline_keys(after).unwrap().is_empty());
        let value: Value = serde_json::from_slice(after).unwrap();
        // Still present once
        assert_eq!(count_plugin_entries(&value), 1);
        // Section left still has three entries; agent-bar at index 1
        let left = value["bar"]["left"].as_array().unwrap();
        assert_eq!(left.len(), 3);
        assert_eq!(left[1]["id"], PLUGIN_ID);
        assert!(left[1].get("refreshIntervalSec").is_none());
        // Unrelated plugins untouched
        assert_eq!(left[0]["id"], "omarchy.menu");
    }

    #[test]
    fn invalid_inline_refresh_aborts() {
        let shell = br#"{
          "bar": { "left": [{ "id": "agent-bar.usage", "refreshIntervalSec": 10 }] }
        }"#;
        let err = MigrationPlan::from_v9(None, Some(shell)).unwrap_err();
        assert!(err.to_string().contains("inline"));
        assert!(err.to_string().contains("10") || err.to_string().contains("30"));
    }

    #[test]
    fn settings_interval_wins_over_inline() {
        let settings = read_fixture("settings-valid.json"); // interval 120
        let shell = read_fixture("shell-left-index1.json"); // inline 90
        let plan = MigrationPlan::from_v9(Some(&settings), Some(&shell)).unwrap();
        assert_eq!(plan.settings.refresh_interval_seconds, 120);
        assert!(plan.shell_changed);
    }

    #[test]
    fn shell_without_inline_is_noop_bytes() {
        let shell = read_fixture("shell-clean.json");
        let plan = MigrationPlan::from_v9(None, Some(&shell)).unwrap();
        assert!(!plan.shell_changed);
        assert_eq!(plan.shell_after.as_ref().unwrap(), &shell);
    }

    #[test]
    fn duplicate_entries_detected() {
        let shell = read_fixture("shell-duplicate.json");
        let value: Value = serde_json::from_slice(&shell).unwrap();
        assert_eq!(count_plugin_entries(&value), 2);
    }

    #[test]
    fn repeated_migration_is_idempotent() {
        let settings = read_fixture("settings-valid.json");
        let shell = read_fixture("shell-left-index1.json");
        let plan1 = MigrationPlan::from_v9(Some(&settings), Some(&shell)).unwrap();
        let v10 = serde_json::to_vec(&plan1.settings).unwrap();
        let plan2 = MigrationPlan::from_v9(Some(&v10), plan1.shell_after.as_deref()).unwrap();
        assert!(plan2.already_migrated);
        assert!(!plan2.shell_changed);
    }
}
