//! Settings: schema tipado + normalização leniente (raw→typed) + load/save atômico.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::config::Paths;

pub const CURRENT_VERSION: u32 = 3;

/// Modo de glyph para a TUI.
///
/// - `Box` (padrão): box-drawing universal, funciona em qualquer terminal.
/// - `Nerd`: opt-in para glyphs nerd-font (degrada para Box se a fonte não tiver).
///
/// Em v1, `Nerd` é aceito no settings mas não altera o render (infra pronta;
/// glyphs nerd-font específicos serão adicionados em iterações futuras).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum GlyphMode {
    Box,
    Nerd,
}

fn glyph_mode_from_str(s: &str) -> Option<GlyphMode> {
    Some(match s {
        "box" => GlyphMode::Box,
        "nerd" => GlyphMode::Nerd,
        _ => return None,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SeparatorStyle {
    Pill,
    Gap,
    Bare,
    Glass,
    Shadow,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DisplayMode {
    Remaining,
    Used,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WindowPolicy {
    Both,
    FiveHour,
    SevenDay,
}

fn separator_from_str(s: &str) -> Option<SeparatorStyle> {
    Some(match s {
        "pill" => SeparatorStyle::Pill,
        "gap" => SeparatorStyle::Gap,
        "bare" => SeparatorStyle::Bare,
        "glass" => SeparatorStyle::Glass,
        "shadow" => SeparatorStyle::Shadow,
        "none" => SeparatorStyle::None,
        _ => return None,
    })
}

fn display_mode_from_str(s: &str) -> Option<DisplayMode> {
    Some(match s {
        "remaining" => DisplayMode::Remaining,
        "used" => DisplayMode::Used,
        _ => return None,
    })
}

fn window_policy_from_str(s: &str) -> Option<WindowPolicy> {
    Some(match s {
        "both" => WindowPolicy::Both,
        "five_hour" => WindowPolicy::FiveHour,
        "seven_day" => WindowPolicy::SevenDay,
        _ => return None,
    })
}

// ---- Schema tipado (serialize) ----

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub struct Tooltip {}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Waybar {
    pub providers: Vec<String>,
    pub separators: SeparatorStyle,
    pub provider_order: Vec<String>,
    pub display_mode: DisplayMode,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signal: Option<u8>,
    pub interval: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Notify {
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CacheSettings {
    pub ttl: BTreeMap<String, u32>,
}

/// Configuração da UI do menu TUI (fonte/tamanho/animações). Consumida
/// pela TUI a partir da Task 16.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MenuSettings {
    pub animations: bool,
    pub font_family: String,
    pub font_size: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub version: u32,
    pub waybar: Waybar,
    pub tooltip: Tooltip,
    pub models: BTreeMap<String, Vec<String>>,
    pub window_policy: BTreeMap<String, WindowPolicy>,
    pub notify: Notify,
    pub cache: CacheSettings,
    /// Configuração da UI do menu TUI. Default: animações on, "IBM Plex
    /// Mono", 12pt. Configurável em settings.json como "menu": {...}.
    pub menu: MenuSettings,
    /// Modo de glyph para a TUI (box-drawing universal ou nerd-font opt-in).
    /// Default: Box. Configurável em settings.json como "glyphMode": "box" | "nerd".
    pub glyph_mode: GlyphMode,
    /// Taxa de cambio US$/BRL para exibicao de custo em R$ na TUI.
    /// Default: 5.50. Configuravel em settings.json como "fxRate": <numero>.
    pub fx_rate: f64,
}

// ---- Schema bruto (deserialize leniente) ----

#[derive(Debug, Default, Deserialize)]
struct RawSettings {
    waybar: Option<RawWaybar>,
    models: Option<BTreeMap<String, Vec<String>>>,
    #[serde(rename = "windowPolicy")]
    window_policy: Option<BTreeMap<String, String>>,
    notify: Option<RawNotify>,
    cache: Option<RawCache>,
    menu: Option<RawMenu>,
    #[serde(rename = "glyphMode")]
    glyph_mode: Option<String>,
    #[serde(rename = "fxRate")]
    fx_rate: Option<f64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawWaybar {
    providers: Option<Vec<String>>,
    separators: Option<String>,
    provider_order: Option<Vec<String>>,
    display_mode: Option<String>,
    signal: Option<i64>,
    interval: Option<u32>,
}

#[derive(Debug, Default, Deserialize)]
struct RawNotify {
    enabled: Option<bool>,
}

#[derive(Debug, Default, Deserialize)]
struct RawCache {
    ttl: Option<BTreeMap<String, u32>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawMenu {
    animations: Option<bool>,
    font_family: Option<String>,
    font_size: Option<u32>,
}

fn default_providers() -> Vec<String> {
    crate::config::KNOWN_PROVIDER_IDS
        .iter()
        .map(|s| s.to_string())
        .collect()
}

fn default_ttl_map() -> BTreeMap<String, u32> {
    crate::config::KNOWN_PROVIDER_IDS
        .iter()
        .map(|p| (p.to_string(), crate::config::default_ttl_secs(p)))
        .collect()
}

/// Filtra a known ids + dedup; reconcilia `provider_order` (válidos + faltantes ao fim).
/// Espelha `normalizeProviderSelection` do TS.
pub fn normalize_provider_selection(
    providers: &[String],
    provider_order: &[String],
) -> (Vec<String>, Vec<String>) {
    let known = |p: &str| crate::config::KNOWN_PROVIDER_IDS.contains(&p);

    let mut deduped: Vec<String> = Vec::new();
    for p in providers {
        if known(p) && !deduped.contains(p) {
            deduped.push(p.clone());
        }
    }

    let mut order: Vec<String> = provider_order
        .iter()
        .filter(|p| deduped.contains(*p))
        .cloned()
        .collect();
    for p in &deduped {
        if !order.contains(p) {
            order.push(p.clone());
        }
    }

    (deduped, order)
}

fn normalize(raw: RawSettings) -> Settings {
    let rw = raw.waybar.unwrap_or_default();

    let providers = rw.providers.unwrap_or_else(default_providers);
    let provider_order = rw.provider_order.unwrap_or_else(default_providers);
    let (providers, provider_order) = normalize_provider_selection(&providers, &provider_order);

    let separators = rw
        .separators
        .as_deref()
        .and_then(separator_from_str)
        .unwrap_or(SeparatorStyle::Gap);

    let display_mode = rw
        .display_mode
        .as_deref()
        .and_then(display_mode_from_str)
        .unwrap_or(DisplayMode::Remaining);

    let signal = rw.signal.filter(|n| (1..=30).contains(n)).map(|n| n as u8);

    // window_policy: default {codex: Both}, mesclado com o raw (inválido → Both).
    let mut window_policy: BTreeMap<String, WindowPolicy> = BTreeMap::new();
    window_policy.insert("codex".to_string(), WindowPolicy::Both);
    if let Some(raw_wp) = raw.window_policy {
        for (k, v) in raw_wp {
            window_policy.insert(k, window_policy_from_str(&v).unwrap_or(WindowPolicy::Both));
        }
    }

    // cache.ttl: defaults mesclados com overrides do raw.
    let mut ttl = default_ttl_map();
    if let Some(rc) = raw.cache {
        if let Some(raw_ttl) = rc.ttl {
            ttl.extend(raw_ttl);
        }
    }

    // menu: defaults mesclados com overrides do raw (leniente por campo).
    let rm = raw.menu.unwrap_or_default();
    let menu = MenuSettings {
        animations: rm.animations.unwrap_or(true),
        font_family: rm
            .font_family
            .unwrap_or_else(|| "IBM Plex Mono".to_string()),
        font_size: rm.font_size.unwrap_or(12),
    };

    let glyph_mode = raw
        .glyph_mode
        .as_deref()
        .and_then(glyph_mode_from_str)
        .unwrap_or(GlyphMode::Box);

    // fx_rate: deve ser positivo e finito; invalido cai para o default.
    const DEFAULT_FX_RATE: f64 = 5.50;
    let fx_rate = raw
        .fx_rate
        .filter(|r| r.is_finite() && *r > 0.0)
        .unwrap_or(DEFAULT_FX_RATE);

    Settings {
        version: CURRENT_VERSION,
        waybar: Waybar {
            providers,
            separators,
            provider_order,
            display_mode,
            signal,
            interval: rw.interval.unwrap_or(crate::config::DEFAULT_INTERVAL_SECS),
        },
        tooltip: Tooltip {},
        models: raw.models.unwrap_or_default(),
        window_policy,
        notify: Notify {
            enabled: raw.notify.and_then(|n| n.enabled) != Some(false),
        },
        cache: CacheSettings { ttl },
        menu,
        glyph_mode,
        fx_rate,
    }
}

/// Carrega + normaliza. Defaults em ausência/erro. Auto-repair se o conteúdo
/// normalizado difere do arquivo bruto.
/// Chaves desconhecidas no arquivo são removidas no auto-repair (intencional).
pub fn load(paths: &Paths) -> Settings {
    let file = paths.settings_file();
    let bytes = match std::fs::read(&file) {
        Ok(b) => b,
        Err(_) => return normalize(RawSettings::default()),
    };

    let raw_value: serde_json::Value = match serde_json::from_slice(&bytes) {
        Ok(v) => v,
        Err(e) => {
            log::warn!("[agent-bar] settings parse error (using defaults): {e}");
            return normalize(RawSettings::default());
        }
    };

    let raw: RawSettings = serde_json::from_value(raw_value.clone()).unwrap_or_default();
    let normalized = normalize(raw);

    let norm_value = serde_json::to_value(&normalized).unwrap_or(serde_json::Value::Null);
    if norm_value != raw_value {
        if let Err(e) = save(paths, &normalized) {
            log::warn!("[agent-bar] settings auto-repair save failed: {e}");
        }
    }

    normalized
}

/// Grava atomicamente (tempfile + rename), pretty 2-espaços, sempre normalizado.
pub fn save(paths: &Paths, settings: &Settings) -> anyhow::Result<()> {
    use std::io::Write;

    std::fs::create_dir_all(&paths.config_dir)?;
    let json = serde_json::to_string_pretty(settings)?;

    let mut tmp = tempfile::NamedTempFile::new_in(&paths.config_dir)?;
    tmp.write_all(json.as_bytes())?;
    tmp.persist(paths.settings_file())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Paths;
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
    fn defaults_when_no_file() {
        let dir = tempdir().unwrap();
        let s = load(&paths_in(dir.path()));
        assert_eq!(s.version, 3);
        assert_eq!(s.waybar.providers, vec!["claude", "codex", "amp", "grok"]);
        assert_eq!(s.waybar.separators, SeparatorStyle::Gap);
        assert_eq!(s.waybar.display_mode, DisplayMode::Remaining);
        assert_eq!(s.waybar.interval, 60);
        assert!(s.waybar.signal.is_none());
        assert!(s.notify.enabled);
        assert_eq!(s.cache.ttl.get("claude"), Some(&300));
        assert_eq!(s.cache.ttl.get("codex"), Some(&90));
        assert_eq!(s.cache.ttl.get("grok"), Some(&90));
        assert_eq!(s.window_policy.get("codex"), Some(&WindowPolicy::Both));
    }

    #[test]
    fn coerces_invalid_enums_to_default() {
        let dir = tempdir().unwrap();
        let p = paths_in(dir.path());
        std::fs::create_dir_all(&p.config_dir).unwrap();
        std::fs::write(
            p.settings_file(),
            r#"{"waybar":{"separators":"bogus","displayMode":"weird","signal":99},
                "windowPolicy":{"codex":"nope"}}"#,
        )
        .unwrap();
        let s = load(&p);
        assert_eq!(s.waybar.separators, SeparatorStyle::Gap);
        assert_eq!(s.waybar.display_mode, DisplayMode::Remaining);
        assert!(s.waybar.signal.is_none()); // 99 fora de 1..=30
        assert_eq!(s.window_policy.get("codex"), Some(&WindowPolicy::Both));
    }

    #[test]
    fn keeps_valid_signal_and_separator() {
        let dir = tempdir().unwrap();
        let p = paths_in(dir.path());
        std::fs::create_dir_all(&p.config_dir).unwrap();
        std::fs::write(
            p.settings_file(),
            r#"{"waybar":{"separators":"glass","signal":8}}"#,
        )
        .unwrap();
        let s = load(&p);
        assert_eq!(s.waybar.separators, SeparatorStyle::Glass);
        assert_eq!(s.waybar.signal, Some(8));
    }

    #[test]
    fn v2_settings_drops_show_percentage_on_load() {
        let dir = tempdir().unwrap();
        let p = paths_in(dir.path());
        std::fs::create_dir_all(&p.config_dir).unwrap();
        std::fs::write(
            p.settings_file(),
            r#"{"version":2,"waybar":{"providers":["claude"],"showPercentage":false,
                "separators":"gap","providerOrder":["claude"],"displayMode":"remaining",
                "interval":60}}"#,
        )
        .unwrap();

        let s = load(&p);
        assert_eq!(s.version, CURRENT_VERSION);

        // Auto-repair já regravou o arquivo sem a chave legada.
        let saved = std::fs::read_to_string(p.settings_file()).unwrap();
        assert!(
            !saved.contains("showPercentage"),
            "showPercentage deveria ter sido dropada no re-save: {saved}"
        );
    }

    #[test]
    fn provider_selection_filters_dedups_and_orders() {
        let (providers, order) = normalize_provider_selection(
            &["amp".into(), "claude".into(), "amp".into(), "ghost".into()],
            &["claude".into()],
        );
        assert_eq!(providers, vec!["amp", "claude"]); // dedup, known-only, ordem de `providers`
        assert_eq!(order, vec!["claude", "amp"]); // order válido + faltantes ao fim
    }

    #[test]
    fn notify_disabled_only_when_explicit_false() {
        let dir = tempdir().unwrap();
        let p = paths_in(dir.path());
        std::fs::create_dir_all(&p.config_dir).unwrap();
        std::fs::write(p.settings_file(), r#"{"notify":{"enabled":false}}"#).unwrap();
        assert!(!load(&p).notify.enabled);
    }

    #[test]
    fn save_then_load_is_stable() {
        let dir = tempdir().unwrap();
        let p = paths_in(dir.path());
        let s1 = load(&p);
        save(&p, &s1).unwrap();
        let s2 = load(&p);
        assert_eq!(s1, s2);
    }

    #[test]
    fn glyph_mode_defaults_to_box() {
        let dir = tempdir().unwrap();
        let s = load(&paths_in(dir.path()));
        assert_eq!(s.glyph_mode, GlyphMode::Box);
    }

    #[test]
    fn glyph_mode_nerd_is_accepted() {
        let dir = tempdir().unwrap();
        let p = paths_in(dir.path());
        std::fs::create_dir_all(&p.config_dir).unwrap();
        std::fs::write(p.settings_file(), r#"{"glyphMode":"nerd"}"#).unwrap();
        let s = load(&p);
        assert_eq!(s.glyph_mode, GlyphMode::Nerd);
    }

    #[test]
    fn glyph_mode_invalid_falls_back_to_box() {
        let dir = tempdir().unwrap();
        let p = paths_in(dir.path());
        std::fs::create_dir_all(&p.config_dir).unwrap();
        std::fs::write(p.settings_file(), r#"{"glyphMode":"emoji"}"#).unwrap();
        let s = load(&p);
        assert_eq!(s.glyph_mode, GlyphMode::Box);
    }

    #[test]
    fn menu_settings_defaults() {
        let dir = tempdir().unwrap();
        let s = load(&paths_in(dir.path()));
        assert!(s.menu.animations);
        assert_eq!(s.menu.font_family, "IBM Plex Mono");
        assert_eq!(s.menu.font_size, 12);
    }

    #[test]
    fn menu_settings_from_json() {
        let dir = tempdir().unwrap();
        let p = paths_in(dir.path());
        std::fs::create_dir_all(&p.config_dir).unwrap();
        std::fs::write(
            p.settings_file(),
            r#"{"menu":{"animations":false,"fontFamily":"Geist Mono","fontSize":13}}"#,
        )
        .unwrap();
        let s = load(&p);
        assert!(!s.menu.animations);
        assert_eq!(s.menu.font_family, "Geist Mono");
        assert_eq!(s.menu.font_size, 13);
    }

    #[test]
    fn fx_rate_defaults_to_5_50() {
        let dir = tempdir().unwrap();
        let s = load(&paths_in(dir.path()));
        let diff = (s.fx_rate - 5.50_f64).abs();
        assert!(diff < 1e-10, "fx_rate esperado 5.50, obtido {}", s.fx_rate);
    }

    #[test]
    fn fx_rate_valid_override_is_accepted() {
        let dir = tempdir().unwrap();
        let p = paths_in(dir.path());
        std::fs::create_dir_all(&p.config_dir).unwrap();
        std::fs::write(p.settings_file(), r#"{"fxRate":6.25}"#).unwrap();
        let s = load(&p);
        let diff = (s.fx_rate - 6.25_f64).abs();
        assert!(diff < 1e-10, "fx_rate esperado 6.25, obtido {}", s.fx_rate);
    }

    #[test]
    fn fx_rate_invalid_values_fall_back_to_default() {
        let cases = [
            r#"{"fxRate":-1.0}"#,
            r#"{"fxRate":0.0}"#,
            r#"{"fxRate":null}"#,
        ];
        for json in &cases {
            let dir = tempdir().unwrap();
            let p = paths_in(dir.path());
            std::fs::create_dir_all(&p.config_dir).unwrap();
            std::fs::write(p.settings_file(), json).unwrap();
            let s = load(&p);
            let diff = (s.fx_rate - 5.50_f64).abs();
            assert!(
                diff < 1e-10,
                "input {json}: fx_rate esperado 5.50, obtido {}",
                s.fx_rate
            );
        }
    }
}
