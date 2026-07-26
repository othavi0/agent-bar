//! v9 → v10 data migration for settings and shell.json inline keys (MIG-007..016).

use std::collections::{BTreeSet, HashSet};

use serde_json::{Map, Value};
use thiserror::Error;

use super::schema::{
    DisplayMetric, DisplaySettings, NotificationSettings, ProviderIdJson, ProviderSetting, Settings,
};
use crate::cli::ProviderId;
use crate::plugin::paths::PLUGIN_ID;

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
        let (settings, unknown_keys, already_settings) = match settings_raw {
            None => (Settings::defaults(), Vec::new(), false),
            Some(raw) if raw.iter().all(|b| b.is_ascii_whitespace()) => {
                (Settings::defaults(), Vec::new(), false)
            }
            Some(raw) => {
                // Already strict v10?
                if let Ok(v10) = Settings::parse_strict(raw) {
                    (v10, Vec::new(), true)
                } else {
                    migrate_v9_settings(raw)?
                }
            }
        };

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

fn migrate_v9_settings(raw: &[u8]) -> Result<(Settings, Vec<String>, bool), MigrationError> {
    let value: Value = serde_json::from_slice(raw)
        .map_err(|e| MigrationError::msg(format!("invalid v9 settings JSON: {e}")))?;
    let obj = value
        .as_object()
        .ok_or_else(|| MigrationError::msg("v9 settings must be a JSON object"))?;

    let mut unknown = Vec::new();
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
        "fxRate",
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
