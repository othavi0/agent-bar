//! Canonical settings schema v1 domain types and validation.

use std::collections::HashSet;
use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::cli::ProviderId;

const SCHEMA_VERSION: u32 = 1;
const MIN_REFRESH: u32 = 30;
const MAX_REFRESH: u32 = 3600;
const MIN_REMINDER_MINUTES: u32 = 15;
const MAX_REMINDER_MINUTES: u32 = 1440;
/// Two hours. Hourly was judged too frequent by the product owner; the field
/// exists so this is a default, not a floor.
const DEFAULT_REMINDER_MINUTES: u32 = 120;

pub(crate) fn default_reminder_minutes() -> u32 {
    DEFAULT_REMINDER_MINUTES
}

/// Display metric for chips and windows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DisplayMetric {
    Used,
    Remaining,
}

/// One provider membership row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderSetting {
    pub id: ProviderIdJson,
    pub enabled: bool,
}

/// Serde adapter for closed provider IDs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ProviderIdJson(pub ProviderId);

impl Serialize for ProviderIdJson {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.0.as_str())
    }
}

impl<'de> Deserialize<'de> for ProviderIdJson {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = String::deserialize(deserializer)?;
        ProviderId::parse_word(&raw)
            .map(ProviderIdJson)
            .ok_or_else(|| serde::de::Error::custom(format!("unknown provider id '{raw}'")))
    }
}

/// Display block.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DisplaySettings {
    pub metric: DisplayMetric,
}

/// Notifications block.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NotificationSettings {
    pub enabled: bool,
    /// Minutes between repeats of an alert that is still above its threshold.
    /// Optional on read so documents written before this field stay valid.
    #[serde(default = "default_reminder_minutes")]
    pub reminder_minutes: u32,
}

/// Canonical settings document (schema v1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Settings {
    pub schema_version: u32,
    pub providers: Vec<ProviderSetting>,
    pub display: DisplaySettings,
    pub refresh_interval_seconds: u32,
    pub notifications: NotificationSettings,
}

/// Settings validation / parse error (maps to helper exit code 3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingsError {
    message: String,
}

impl SettingsError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for SettingsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for SettingsError {}

impl Settings {
    /// Product defaults (missing file returns these without creating a file).
    pub fn defaults() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            providers: vec![
                ProviderSetting {
                    id: ProviderIdJson(ProviderId::Claude),
                    enabled: true,
                },
                ProviderSetting {
                    id: ProviderIdJson(ProviderId::Codex),
                    enabled: true,
                },
                ProviderSetting {
                    id: ProviderIdJson(ProviderId::Amp),
                    enabled: true,
                },
                ProviderSetting {
                    id: ProviderIdJson(ProviderId::Grok),
                    enabled: true,
                },
            ],
            display: DisplaySettings {
                metric: DisplayMetric::Remaining,
            },
            refresh_interval_seconds: 60,
            notifications: NotificationSettings {
                enabled: true,
                reminder_minutes: DEFAULT_REMINDER_MINUTES,
            },
        }
    }

    /// Parse a complete document from JSON bytes without discarding unknown keys.
    pub fn parse_strict(raw: &[u8]) -> Result<Self, SettingsError> {
        let value: Value = serde_json::from_slice(raw)
            .map_err(|err| SettingsError::new(format!("invalid settings JSON: {err}")))?;
        reject_unknown_top_level(&value)?;
        let settings: Self = serde_json::from_value(value)
            .map_err(|err| SettingsError::new(format!("invalid settings document: {err}")))?;
        settings.validate()?;
        Ok(settings)
    }

    /// Semantic validation beyond `deny_unknown_fields`.
    pub fn validate(&self) -> Result<(), SettingsError> {
        if self.schema_version != SCHEMA_VERSION {
            return Err(SettingsError::new(format!(
                "schemaVersion must be {SCHEMA_VERSION}"
            )));
        }
        if !(MIN_REFRESH..=MAX_REFRESH).contains(&self.refresh_interval_seconds) {
            return Err(SettingsError::new(format!(
                "refreshIntervalSeconds must be in {MIN_REFRESH}..={MAX_REFRESH}"
            )));
        }
        if !(MIN_REMINDER_MINUTES..=MAX_REMINDER_MINUTES)
            .contains(&self.notifications.reminder_minutes)
        {
            return Err(SettingsError::new(format!(
                "reminderMinutes must be in {MIN_REMINDER_MINUTES}..={MAX_REMINDER_MINUTES}"
            )));
        }
        if self.providers.len() != ProviderId::ALL.len() {
            return Err(SettingsError::new(
                "providers must list every supported provider exactly once",
            ));
        }
        let mut seen = HashSet::new();
        for item in &self.providers {
            if !seen.insert(item.id.0) {
                return Err(SettingsError::new(format!(
                    "duplicate provider id '{}'",
                    item.id.0
                )));
            }
        }
        for required in ProviderId::ALL {
            if !seen.contains(&required) {
                return Err(SettingsError::new(format!(
                    "missing provider id '{}'",
                    required.as_str()
                )));
            }
        }
        Ok(())
    }

    /// Canonical JSON object plus trailing newline.
    pub fn to_canonical_json_line(&self) -> Result<String, SettingsError> {
        self.validate()?;
        let mut body = serde_json::to_string(self)
            .map_err(|err| SettingsError::new(format!("settings serialization failed: {err}")))?;
        body.push('\n');
        Ok(body)
    }
}

fn reject_unknown_top_level(value: &Value) -> Result<(), SettingsError> {
    let obj = value
        .as_object()
        .ok_or_else(|| SettingsError::new("settings document must be a JSON object"))?;
    const ALLOWED: &[&str] = &[
        "schemaVersion",
        "providers",
        "display",
        "refreshIntervalSeconds",
        "notifications",
    ];
    for key in obj.keys() {
        if !ALLOWED.contains(&key.as_str()) {
            return Err(SettingsError::new(format!("unknown settings key '{key}'")));
        }
    }
    if let Some(display) = obj.get("display") {
        let display = display
            .as_object()
            .ok_or_else(|| SettingsError::new("display must be an object"))?;
        for key in display.keys() {
            if key != "metric" {
                return Err(SettingsError::new(format!("unknown display key '{key}'")));
            }
        }
    }
    if let Some(notifications) = obj.get("notifications") {
        let notifications = notifications
            .as_object()
            .ok_or_else(|| SettingsError::new("notifications must be an object"))?;
        for key in notifications.keys() {
            if key != "enabled" && key != "reminderMinutes" {
                return Err(SettingsError::new(format!(
                    "unknown notifications key '{key}'"
                )));
            }
        }
    }
    if let Some(providers) = obj.get("providers") {
        let providers = providers
            .as_array()
            .ok_or_else(|| SettingsError::new("providers must be an array"))?;
        for (idx, item) in providers.iter().enumerate() {
            let item = item
                .as_object()
                .ok_or_else(|| SettingsError::new(format!("providers[{idx}] must be an object")))?;
            for key in item.keys() {
                if key != "id" && key != "enabled" {
                    return Err(SettingsError::new(format!(
                        "unknown providers[{idx}] key '{key}'"
                    )));
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_validate_and_round_trip() {
        let defaults = Settings::defaults();
        defaults.validate().unwrap();
        let line = defaults.to_canonical_json_line().unwrap();
        let parsed = Settings::parse_strict(line.as_bytes()).unwrap();
        assert_eq!(parsed, defaults);
    }

    #[test]
    fn rejects_unknown_keys_and_duplicate_providers() {
        let unknown = br#"{"schemaVersion":1,"providers":[{"id":"claude","enabled":true},{"id":"codex","enabled":true},{"id":"amp","enabled":true},{"id":"grok","enabled":true}],"display":{"metric":"remaining"},"refreshIntervalSeconds":60,"notifications":{"enabled":true},"theme":"dark"}"#;
        assert!(Settings::parse_strict(unknown)
            .unwrap_err()
            .message()
            .contains("unknown"));

        let dup = br#"{"schemaVersion":1,"providers":[{"id":"claude","enabled":true},{"id":"claude","enabled":false},{"id":"amp","enabled":true},{"id":"grok","enabled":true}],"display":{"metric":"remaining"},"refreshIntervalSeconds":60,"notifications":{"enabled":true}}"#;
        assert!(Settings::parse_strict(dup).is_err());
    }

    #[test]
    fn reminder_minutes_defaults_when_absent() {
        // Settings reads never rewrite (SET-007), so an existing settings.json
        // predating this field must parse and take the default silently.
        let doc = br#"{"schemaVersion":1,"providers":[{"id":"claude","enabled":true},{"id":"codex","enabled":true},{"id":"amp","enabled":true},{"id":"grok","enabled":true}],"display":{"metric":"remaining"},"refreshIntervalSeconds":60,"notifications":{"enabled":true}}"#;
        let parsed = Settings::parse_strict(doc).unwrap();
        assert_eq!(
            parsed.notifications.reminder_minutes,
            DEFAULT_REMINDER_MINUTES
        );
        assert_eq!(parsed.notifications.reminder_minutes, 120);
    }

    #[test]
    fn reminder_minutes_round_trips_and_rejects_out_of_range() {
        let mut doc = Settings::defaults();
        doc.notifications.reminder_minutes = MIN_REMINDER_MINUTES;
        doc.validate().unwrap();
        doc.notifications.reminder_minutes = MAX_REMINDER_MINUTES;
        doc.validate().unwrap();

        doc.notifications.reminder_minutes = MIN_REMINDER_MINUTES - 1;
        let err = doc.validate().unwrap_err();
        assert!(err.message().contains("reminderMinutes"));

        doc.notifications.reminder_minutes = MAX_REMINDER_MINUTES + 1;
        assert!(doc.validate().is_err());
    }

    #[test]
    fn reminder_minutes_survives_the_hand_written_allowlist() {
        // deny_unknown_fields is not the only gate: reject_unknown_top_level
        // walks raw JSON before deserialization and would reject the key on
        // its own.
        let doc = br#"{"schemaVersion":1,"providers":[{"id":"claude","enabled":true},{"id":"codex","enabled":true},{"id":"amp","enabled":true},{"id":"grok","enabled":true}],"display":{"metric":"remaining"},"refreshIntervalSeconds":60,"notifications":{"enabled":true,"reminderMinutes":240}}"#;
        let parsed = Settings::parse_strict(doc).unwrap();
        assert_eq!(parsed.notifications.reminder_minutes, 240);
    }
}
