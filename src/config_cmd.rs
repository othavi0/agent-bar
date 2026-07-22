//! `agent-bar config show|apply` — subset editável do settings.json (spec
//! 2026-07-21-omarchy-settings-and-cli-simplify).

use serde::{Deserialize, Serialize};

use crate::config::Paths;
use crate::settings::{normalize_provider_selection, DisplayMode, Settings};

pub const CONFIG_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigView {
    pub schema_version: u32,
    pub providers: Vec<String>,
    pub provider_order: Vec<String>,
    pub display_mode: DisplayMode,
    pub notify: NotifyView,
    pub menu_animations: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NotifyView {
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApplyError {
    Validation(String),
    Io(String),
}

impl std::fmt::Display for ApplyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApplyError::Validation(m) | ApplyError::Io(m) => write!(f, "{m}"),
        }
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConfigPatch {
    schema_version: Option<u32>,
    providers: Option<Vec<String>>,
    provider_order: Option<Vec<String>>,
    display_mode: Option<String>,
    notify: Option<NotifyPatch>,
}

#[derive(Debug, Default, Deserialize)]
struct NotifyPatch {
    enabled: Option<bool>,
}

pub fn view_from_settings(s: &Settings) -> ConfigView {
    ConfigView {
        schema_version: CONFIG_SCHEMA_VERSION,
        providers: s.waybar.providers.clone(),
        provider_order: s.waybar.provider_order.clone(),
        display_mode: s.waybar.display_mode,
        notify: NotifyView {
            enabled: s.notify.enabled,
        },
        menu_animations: s.menu.animations,
    }
}

pub fn show(paths: &Paths) -> ConfigView {
    view_from_settings(&crate::settings::load(paths))
}

pub fn apply_json(paths: &Paths, raw: &str) -> Result<ConfigView, ApplyError> {
    let patch: ConfigPatch = serde_json::from_str(raw).map_err(|e| {
        ApplyError::Validation(format!("invalid JSON: {e}"))
    })?;

    match patch.schema_version {
        Some(CONFIG_SCHEMA_VERSION) => {}
        Some(v) => {
            return Err(ApplyError::Validation(format!(
                "unsupported schemaVersion: {v} (expected {CONFIG_SCHEMA_VERSION})"
            )));
        }
        None => {
            return Err(ApplyError::Validation(
                "schemaVersion is required".into(),
            ));
        }
    }

    let mut s = crate::settings::load(paths);

    if let Some(providers) = patch.providers {
        let order_src = patch
            .provider_order
            .clone()
            .unwrap_or_else(|| s.waybar.provider_order.clone());
        let (providers, order) = normalize_provider_selection(&providers, &order_src);
        if providers.is_empty() {
            return Err(ApplyError::Validation(
                "providers must contain at least one known id".into(),
            ));
        }
        s.waybar.providers = providers;
        s.waybar.provider_order = order;
    } else if let Some(order) = patch.provider_order {
        let (providers, order) =
            normalize_provider_selection(&s.waybar.providers, &order);
        s.waybar.providers = providers;
        s.waybar.provider_order = order;
    }

    if let Some(mode) = patch.display_mode {
        s.waybar.display_mode = match mode.as_str() {
            "remaining" => DisplayMode::Remaining,
            "used" => DisplayMode::Used,
            other => {
                return Err(ApplyError::Validation(format!(
                    "invalid displayMode: '{other}' (remaining|used)"
                )));
            }
        };
    }

    if let Some(n) = patch.notify {
        if let Some(en) = n.enabled {
            s.notify.enabled = en;
        }
    }

    crate::settings::save(paths, &s).map_err(|e| ApplyError::Io(e.to_string()))?;
    Ok(view_from_settings(&s))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Paths;
    use crate::settings::{self, DisplayMode};
    use std::path::PathBuf;
    use tempfile::tempdir;

    fn paths_in(dir: &std::path::Path) -> Paths {
        Paths {
            cache_dir: dir.join("cache"),
            config_dir: dir.join("config"),
            claude_credentials: PathBuf::new(),
            codex_auth: PathBuf::new(),
            codex_sessions: PathBuf::new(),
            amp_settings: PathBuf::new(),
            amp_threads: PathBuf::new(),
            grok_home: PathBuf::new(),
            grok_auth: PathBuf::new(),
        }
    }

    #[test]
    fn show_defaults_when_no_file() {
        let dir = tempdir().unwrap();
        let v = show(&paths_in(dir.path()));
        assert_eq!(v.schema_version, 1);
        assert_eq!(v.providers, vec!["claude", "codex", "amp", "grok"]);
        assert_eq!(v.provider_order, v.providers);
        assert_eq!(v.display_mode, DisplayMode::Remaining);
        assert!(v.notify.enabled);
    }

    #[test]
    fn apply_rejects_wrong_schema() {
        let dir = tempdir().unwrap();
        let p = paths_in(dir.path());
        let err = apply_json(&p, r#"{"schemaVersion":2,"providers":["claude"]}"#).unwrap_err();
        assert!(matches!(err, ApplyError::Validation(_)));
    }

    #[test]
    fn apply_rejects_empty_providers() {
        let dir = tempdir().unwrap();
        let p = paths_in(dir.path());
        let err = apply_json(&p, r#"{"schemaVersion":1,"providers":[]}"#).unwrap_err();
        assert!(matches!(err, ApplyError::Validation(_)));
    }

    #[test]
    fn apply_partial_preserves_omitted_fields() {
        let dir = tempdir().unwrap();
        let p = paths_in(dir.path());
        // seed
        let mut s = settings::load(&p);
        s.waybar.display_mode = DisplayMode::Used;
        s.notify.enabled = false;
        settings::save(&p, &s).unwrap();

        let v = apply_json(
            &p,
            r#"{"schemaVersion":1,"providers":["claude","codex"]}"#,
        )
        .unwrap();
        assert_eq!(v.providers, vec!["claude", "codex"]);
        assert_eq!(v.display_mode, DisplayMode::Used);
        assert!(!v.notify.enabled);

        let loaded = settings::load(&p);
        assert_eq!(loaded.waybar.providers, vec!["claude", "codex"]);
        assert_eq!(loaded.waybar.display_mode, DisplayMode::Used);
        assert!(!loaded.notify.enabled);
        // separators etc. intact
        assert_eq!(loaded.waybar.separators, s.waybar.separators);
    }

    #[test]
    fn apply_display_mode_and_notify() {
        let dir = tempdir().unwrap();
        let p = paths_in(dir.path());
        let v = apply_json(
            &p,
            r#"{"schemaVersion":1,"displayMode":"used","notify":{"enabled":false}}"#,
        )
        .unwrap();
        assert_eq!(v.display_mode, DisplayMode::Used);
        assert!(!v.notify.enabled);
    }

    #[test]
    fn apply_rejects_invalid_display_mode() {
        let dir = tempdir().unwrap();
        let p = paths_in(dir.path());
        let err = apply_json(&p, r#"{"schemaVersion":1,"displayMode":"nope"}"#).unwrap_err();
        assert!(matches!(err, ApplyError::Validation(_)));
    }

    #[test]
    fn apply_drops_unknown_provider_ids() {
        let dir = tempdir().unwrap();
        let p = paths_in(dir.path());
        let v = apply_json(
            &p,
            r#"{"schemaVersion":1,"providers":["claude","nope","codex"]}"#,
        )
        .unwrap();
        assert_eq!(v.providers, vec!["claude", "codex"]);
    }

    #[test]
    fn apply_all_unknown_providers_is_error() {
        let dir = tempdir().unwrap();
        let p = paths_in(dir.path());
        let err =
            apply_json(&p, r#"{"schemaVersion":1,"providers":["nope"]}"#).unwrap_err();
        assert!(matches!(err, ApplyError::Validation(_)));
    }

    #[test]
    fn show_json_wire_format() {
        let dir = tempdir().unwrap();
        let v = show(&paths_in(dir.path()));
        let j = serde_json::to_value(&v).unwrap();
        assert_eq!(j["schemaVersion"], 1);
        assert_eq!(j["displayMode"], "remaining");
        assert_eq!(j["notify"]["enabled"], true);
        assert!(j["providers"].is_array());
        assert_eq!(
            j["menuAnimations"],
            true,
            "default do Settings.menu.animations"
        );
    }

    #[test]
    fn apply_ignores_menu_animations_field() {
        let dir = tempdir().unwrap();
        let p = paths_in(dir.path());
        let v = apply_json(
            &p,
            r#"{"schemaVersion":1,"displayMode":"used","menuAnimations":false}"#,
        )
        .unwrap();
        assert_eq!(v.display_mode, DisplayMode::Used);
        // campo ignorado: não existe em ConfigPatch, apply não falha, e o
        // valor real de menu.animations em settings.json não é tocado.
        let loaded = crate::settings::load(&p);
        assert!(
            loaded.menu.animations,
            "apply não deve mexer em menu.animations"
        );
    }
}
