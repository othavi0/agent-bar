//! Configuração estática + resolução de paths (injetada, sem estado global).

use std::env;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::app_identity::APP_NAME;

// API Claude (contrato §3.10 do design — UA hardcoded, não a versão do agent-bar).
pub const CLAUDE_USAGE_URL: &str = "https://api.anthropic.com/api/oauth/usage";
pub const CLAUDE_USER_AGENT: &str = "claude-code/2.1.179";
pub const CLAUDE_BETA_HEADER: &str = "oauth-2025-04-20";

pub const HTTP_TIMEOUT_SECS: u64 = 5;
pub const PROVIDER_TIMEOUT_SECS: u64 = 10;

/// Poll interval default do módulo Waybar (era 120s hardcoded; ver design §1).
pub const DEFAULT_INTERVAL_SECS: u32 = 60;

pub const KNOWN_PROVIDER_IDS: [&str; 4] = ["claude", "codex", "amp", "grok"];

/// TTL default de cache por provider (segundos). Claude conservador (rate-limit);
/// Codex/Amp/Grok são locais e podem ser mais frescos.
pub fn default_ttl_secs(provider: &str) -> u32 {
    match provider {
        "codex" | "amp" | "grok" => 90,
        _ => 300,
    }
}

// Thresholds de saúde (% restante).
pub const THRESHOLD_GREEN: f64 = 60.0;
pub const THRESHOLD_YELLOW: f64 = 30.0;
pub const THRESHOLD_ORANGE: f64 = 10.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthStatus {
    Ok,
    Low,
    Warn,
    Critical,
}

impl HealthStatus {
    /// Token lowercase para class CSS / `alt` do Waybar (= TS `type HealthStatus`).
    pub fn as_str(&self) -> &'static str {
        match self {
            HealthStatus::Ok => "ok",
            HealthStatus::Low => "low",
            HealthStatus::Warn => "warn",
            HealthStatus::Critical => "critical",
        }
    }
}

/// Bucket de saúde a partir do % restante cru. `None` → Ok (desconhecido).
pub fn status_for_percent(pct: Option<f64>) -> HealthStatus {
    match pct {
        None => HealthStatus::Ok,
        Some(p) if p < THRESHOLD_ORANGE => HealthStatus::Critical,
        Some(p) if p < THRESHOLD_YELLOW => HealthStatus::Warn,
        Some(p) if p < THRESHOLD_GREEN => HealthStatus::Low,
        Some(_) => HealthStatus::Ok,
    }
}

/// Diretório de dados (`icons/`, `scripts/`) compartilhado entre o
/// self-update (`agent-bar update`, via `update::default_data_dir`) e a
/// resolução de assets standalone (`agent-bar setup`, via
/// `waybar_contract::standalone_data_asset_dir`) — hotfix 7.0.1: antes cada
/// um resolvia essa pasta de um jeito diferente (`update` ignorava
/// `XDG_DATA_HOME`), causando split-brain pra quem só setasse
/// `XDG_DATA_HOME` (self-update escrevia num lugar, setup lia de outro).
///
/// Mesma convenção do `install.sh`, com um fallback extra via
/// `XDG_DATA_HOME` que o script não tem (só `AGENT_BAR_DATA`/`$HOME` fixo).
///
/// Prioridade: `AGENT_BAR_DATA` env (usado como está — já é o diretório
/// final, sem sufixo), senão `${XDG_DATA_HOME:-<home>/.local/share}/agent-bar`.
pub fn agent_bar_data_dir(home: &Path) -> PathBuf {
    if let Some(v) = env::var_os("AGENT_BAR_DATA").filter(|v| !v.is_empty()) {
        return PathBuf::from(v);
    }

    let data_home = env::var_os("XDG_DATA_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .filter(|p| p.is_absolute()) // XDG spec: ignore relative paths
        .unwrap_or_else(|| home.join(".local").join("share"));

    data_home.join(APP_NAME)
}

/// Epoch em milissegundos. Os módulos puros recebem `now_ms` por parâmetro
/// (testáveis); só os call sites de produção chamam esta função.
pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Paths resolvidos de XDG/HOME. Injetado pela call-chain (sem singleton global).
#[derive(Debug, Clone)]
pub struct Paths {
    pub cache_dir: PathBuf,
    pub config_dir: PathBuf,
    pub claude_credentials: PathBuf,
    pub codex_auth: PathBuf,
    pub codex_sessions: PathBuf,
    pub amp_settings: PathBuf,
    pub amp_threads: PathBuf,
    pub grok_home: PathBuf,
    pub grok_auth: PathBuf,
}

impl Paths {
    pub fn from_env() -> anyhow::Result<Self> {
        let home = env::var_os("HOME")
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
            .ok_or_else(|| anyhow::anyhow!("HOME não está definido"))?;

        let xdg_dir = |var: &str, fallback: &str| -> PathBuf {
            env::var_os(var)
                .filter(|v| !v.is_empty())
                .map(PathBuf::from)
                .filter(|p| p.is_absolute()) // XDG spec: ignore relative paths
                .unwrap_or_else(|| home.join(fallback))
        };

        let xdg_cache = xdg_dir("XDG_CACHE_HOME", ".cache");
        let xdg_config = xdg_dir("XDG_CONFIG_HOME", ".config");

        let grok_home = env::var_os("GROK_HOME")
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".grok"));

        Ok(Self {
            cache_dir: xdg_cache.join("agent-bar"),
            config_dir: xdg_config.join("agent-bar"),
            claude_credentials: home.join(".claude").join(".credentials.json"),
            codex_auth: home.join(".codex").join("auth.json"),
            codex_sessions: home.join(".codex").join("sessions"),
            amp_settings: xdg_config.join("amp").join("settings.json"),
            amp_threads: home
                .join(".local")
                .join("share")
                .join("amp")
                .join("threads"),
            grok_home: grok_home.clone(),
            grok_auth: grok_home.join("auth.json"),
        })
    }

    /// Caminho do settings.json sob o config dir.
    pub fn settings_file(&self) -> PathBuf {
        self.config_dir.join("settings.json")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    #[serial_test::serial]
    fn paths_from_env_uses_xdg_when_set() {
        temp_env::with_vars(
            [
                ("HOME", Some("/home/u")),
                ("XDG_CACHE_HOME", Some("/x/cache")),
                ("XDG_CONFIG_HOME", Some("/x/config")),
            ],
            || {
                let p = Paths::from_env().unwrap();
                assert_eq!(p.cache_dir, PathBuf::from("/x/cache/agent-bar"));
                assert_eq!(p.config_dir, PathBuf::from("/x/config/agent-bar"));
                assert_eq!(
                    p.claude_credentials,
                    PathBuf::from("/home/u/.claude/.credentials.json")
                );
            },
        );
    }

    #[test]
    #[serial_test::serial]
    fn paths_from_env_falls_back_to_home_when_xdg_unset() {
        temp_env::with_vars(
            [
                ("HOME", Some("/home/u")),
                ("XDG_CACHE_HOME", None::<&str>),
                ("XDG_CONFIG_HOME", None::<&str>),
            ],
            || {
                let p = Paths::from_env().unwrap();
                assert_eq!(p.cache_dir, PathBuf::from("/home/u/.cache/agent-bar"));
                assert_eq!(p.config_dir, PathBuf::from("/home/u/.config/agent-bar"));
            },
        );
    }

    #[test]
    #[serial_test::serial]
    fn agent_bar_data_dir_prefers_env_override() {
        temp_env::with_vars(
            [
                ("AGENT_BAR_DATA", Some("/custom/data")),
                ("XDG_DATA_HOME", Some("/xdg/data")),
            ],
            || {
                assert_eq!(
                    agent_bar_data_dir(Path::new("/home/u")),
                    PathBuf::from("/custom/data")
                );
            },
        );
    }

    #[test]
    #[serial_test::serial]
    fn agent_bar_data_dir_honors_xdg_data_home() {
        temp_env::with_vars(
            [
                ("AGENT_BAR_DATA", None::<&str>),
                ("XDG_DATA_HOME", Some("/xdg/data")),
            ],
            || {
                assert_eq!(
                    agent_bar_data_dir(Path::new("/home/u")),
                    PathBuf::from("/xdg/data/agent-bar")
                );
            },
        );
    }

    #[test]
    #[serial_test::serial]
    fn agent_bar_data_dir_falls_back_to_home() {
        temp_env::with_vars(
            [
                ("AGENT_BAR_DATA", None::<&str>),
                ("XDG_DATA_HOME", None::<&str>),
            ],
            || {
                assert_eq!(
                    agent_bar_data_dir(Path::new("/home/u")),
                    PathBuf::from("/home/u/.local/share/agent-bar")
                );
            },
        );
    }

    #[test]
    fn status_thresholds() {
        assert_eq!(status_for_percent(None), HealthStatus::Ok);
        assert_eq!(status_for_percent(Some(75.0)), HealthStatus::Ok);
        assert_eq!(status_for_percent(Some(59.9)), HealthStatus::Low);
        assert_eq!(status_for_percent(Some(29.9)), HealthStatus::Warn);
        assert_eq!(status_for_percent(Some(9.9)), HealthStatus::Critical);
        assert_eq!(status_for_percent(Some(0.0)), HealthStatus::Critical);
    }

    #[test]
    fn ttl_defaults_per_provider() {
        assert_eq!(default_ttl_secs("claude"), 300);
        assert_eq!(default_ttl_secs("codex"), 90);
        assert_eq!(default_ttl_secs("amp"), 90);
        assert_eq!(default_ttl_secs("grok"), 90);
        assert_eq!(default_ttl_secs("unknown"), 300);
    }

    #[test]
    fn health_status_as_str() {
        assert_eq!(HealthStatus::Ok.as_str(), "ok");
        assert_eq!(HealthStatus::Low.as_str(), "low");
        assert_eq!(HealthStatus::Warn.as_str(), "warn");
        assert_eq!(HealthStatus::Critical.as_str(), "critical");
    }
}
