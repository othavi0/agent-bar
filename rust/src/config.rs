//! Configuração estática + resolução de paths (injetada, sem estado global).

use std::env;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

// API Claude (contrato §3.10 do design — UA hardcoded, não a versão do agent-bar).
pub const CLAUDE_USAGE_URL: &str = "https://api.anthropic.com/api/oauth/usage";
pub const CLAUDE_USER_AGENT: &str = "claude-code/2.1.179";
pub const CLAUDE_BETA_HEADER: &str = "oauth-2025-04-20";

pub const HTTP_TIMEOUT_SECS: u64 = 5;
pub const PROVIDER_TIMEOUT_SECS: u64 = 10;

/// Poll interval default do módulo Waybar (era 120s hardcoded; ver design §1).
pub const DEFAULT_INTERVAL_SECS: u32 = 60;

pub const KNOWN_PROVIDER_IDS: [&str; 3] = ["claude", "codex", "amp"];

/// TTL default de cache por provider (segundos). Claude conservador (rate-limit);
/// Codex/Amp são locais e podem ser mais frescos.
pub fn default_ttl_secs(provider: &str) -> u32 {
    match provider {
        "codex" | "amp" => 90,
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
        assert_eq!(default_ttl_secs("unknown"), 300);
    }
}
