pub mod amp;
pub mod amp_cli;
pub mod base;
pub mod claude;
pub mod codex;
pub mod error;
pub mod extras;
pub mod types;

use std::time::Duration;

use async_trait::async_trait;
use time::{OffsetDateTime, UtcOffset};

use crate::config::Paths;
use crate::settings::Settings;
use types::{AllQuotas, ProviderQuota};

const MAX_RETRIES: u32 = 1;
const RETRY_DELAY: Duration = Duration::from_secs(1);

/// Contexto injetado (DI): cliente HTTP, paths, settings, relógio. As funções
/// de provider são impuras (rede/disco/subprocesso) e recebem tudo daqui.
pub struct Ctx<'a> {
    pub client: &'a reqwest::Client,
    pub paths: &'a Paths,
    pub settings: &'a Settings,
    /// Epoch em ms (cache TTL, expiry do Claude, fullAt do Amp).
    pub now_ms: u64,
    /// Offset local (data hoje/ontem das sessões do Codex; o resto é UTC).
    pub local_offset: UtcOffset,
    /// URL de usage do Claude — injetável p/ testes (wiremock); default = const.
    pub claude_usage_url: String,
    /// `clientInfo.version` do app-server do Codex.
    pub version: &'static str,
    /// `$HOME` resolvido (candidatos de binário do Amp ficam sob ele).
    pub home: std::path::PathBuf,
}

impl Ctx<'_> {
    /// TTL de cache em ms para um provider (`settings.cache.ttl` ou default).
    pub fn ttl_ms(&self, provider: &str) -> u64 {
        let secs = self
            .settings
            .cache
            .ttl
            .get(provider)
            .copied()
            .unwrap_or_else(|| crate::config::default_ttl_secs(provider));
        u64::from(secs) * 1000
    }
}

#[async_trait(?Send)]
pub trait Provider {
    fn id(&self) -> &'static str;
    fn name(&self) -> &'static str;
    fn cache_key(&self) -> &'static str;
    async fn is_available(&self, ctx: &Ctx<'_>) -> bool;
    /// Sempre devolve um quota (nunca erra): falhas viram `error` embutido,
    /// igual ao `getQuota()` do TS. O boundary "cache só em sucesso" vive
    /// dentro (em `base::get_or_fetch`).
    async fn get_quota(&self, ctx: &Ctx<'_>) -> ProviderQuota;
}

/// Providers de produção. Cresce a cada plano (04a: Claude; 04b: Amp; 04c: Codex).
pub fn registry() -> Vec<Box<dyn Provider>> {
    vec![
        Box::new(claude::ClaudeProvider),
        Box::new(amp::AmpProvider),
        Box::new(codex::CodexProvider),
    ]
}

/// ISO-8601 UTC com 3 dígitos de milissegundo e sufixo `Z` — idêntico ao
/// `Date.prototype.toISOString()` do JS (o `Rfc3339` do `time` omite millis zero).
pub fn iso_from_ms(ms: u64) -> String {
    let dt = OffsetDateTime::from_unix_timestamp_nanos(i128::from(ms) * 1_000_000)
        .unwrap_or(OffsetDateTime::UNIX_EPOCH);
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z",
        dt.year(),
        u8::from(dt.month()),
        dt.day(),
        dt.hour(),
        dt.minute(),
        dt.second(),
        dt.millisecond()
    )
}

async fn fetch_one(provider: &dyn Provider, ctx: &Ctx<'_>) -> ProviderQuota {
    let timeout = Duration::from_secs(crate::config::PROVIDER_TIMEOUT_SECS);
    let mut attempt = 0u32;
    loop {
        match tokio::time::timeout(timeout, provider.get_quota(ctx)).await {
            Ok(quota) => return quota,
            Err(_elapsed) => {
                if attempt < MAX_RETRIES {
                    log::debug!(
                        "{} timeout, retrying ({}/{MAX_RETRIES})...",
                        provider.name(),
                        attempt + 1
                    );
                    attempt += 1;
                    tokio::time::sleep(RETRY_DELAY).await;
                    continue;
                }
                let msg = format!(
                    "{} timed out after {}ms",
                    provider.name(),
                    timeout.as_millis()
                );
                log::debug!("{msg}");
                return ProviderQuota {
                    provider: provider.id().to_string(),
                    display_name: provider.name().to_string(),
                    available: false,
                    account: None,
                    plan: None,
                    plan_type: None,
                    primary: None,
                    secondary: None,
                    models: None,
                    extra: None,
                    error: Some(msg),
                };
            }
        }
    }
}

/// Fan-out concorrente sobre os providers (1 thread, `join_all`).
pub async fn fetch_all(providers: &[Box<dyn Provider>], ctx: &Ctx<'_>) -> AllQuotas {
    let futures = providers.iter().map(|p| fetch_one(p.as_ref(), ctx));
    let results = futures::future::join_all(futures).await;
    AllQuotas {
        providers: results,
        fetched_at: iso_from_ms(ctx.now_ms),
    }
}

#[cfg(test)]
pub(crate) mod test_support {
    use super::*;
    use std::path::{Path, PathBuf};

    pub fn settings() -> Settings {
        // Settings default (sem arquivo) — usa o load com config dir inexistente.
        let dir = std::env::temp_dir().join("agent-bar-test-cfg-missing");
        crate::settings::load(&Paths {
            cache_dir: dir.join("cache"),
            config_dir: dir.join("config-missing-xyz"),
            claude_credentials: PathBuf::new(),
            codex_auth: PathBuf::new(),
            codex_sessions: PathBuf::new(),
            amp_settings: PathBuf::new(),
            amp_threads: PathBuf::new(),
        })
    }

    pub fn paths_in(dir: &Path) -> Paths {
        Paths {
            cache_dir: dir.join("cache"),
            config_dir: dir.join("config"),
            claude_credentials: dir.join("claude.json"),
            codex_auth: dir.join("codex-auth.json"),
            codex_sessions: dir.join("codex-sessions"),
            amp_settings: dir.join("amp-settings.json"),
            amp_threads: dir.join("amp-threads"),
        }
    }

    /// Ctx p/ testes apontando o cache num tempdir. `paths` é vazado (leak) p/
    /// viver pelo Ctx; aceitável em teste.
    pub fn ctx_for<'a>(
        dir: &Path,
        settings: &'a Settings,
        client: &'a reqwest::Client,
        now_ms: u64,
    ) -> Ctx<'a> {
        let paths: &'a Paths = Box::leak(Box::new(paths_in(dir)));
        Ctx {
            client,
            paths,
            settings,
            now_ms,
            local_offset: UtcOffset::UTC,
            claude_usage_url: "http://127.0.0.1:0/api/oauth/usage".to_string(),
            version: "0.0.0-test",
            home: dir.to_path_buf(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::{ctx_for, settings};
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn iso_from_ms_matches_to_iso_string_format() {
        assert_eq!(iso_from_ms(0), "1970-01-01T00:00:00.000Z");
        assert_eq!(iso_from_ms(1_000), "1970-01-01T00:00:01.000Z");
        assert_eq!(iso_from_ms(1_234), "1970-01-01T00:00:01.234Z");
    }

    // Provider fake p/ exercitar fetch_all (sucesso + timeout + retry).
    struct FakeProvider {
        id: &'static str,
        name: &'static str,
        hang: bool,
    }

    #[async_trait(?Send)]
    impl Provider for FakeProvider {
        fn id(&self) -> &'static str {
            self.id
        }
        fn name(&self) -> &'static str {
            self.name
        }
        fn cache_key(&self) -> &'static str {
            "fake"
        }
        async fn is_available(&self, _ctx: &Ctx<'_>) -> bool {
            true
        }
        async fn get_quota(&self, _ctx: &Ctx<'_>) -> ProviderQuota {
            if self.hang {
                // Excede o timeout de 10s (virtualizado em start_paused).
                tokio::time::sleep(Duration::from_secs(60)).await;
            }
            ProviderQuota {
                provider: self.id.to_string(),
                display_name: self.name.to_string(),
                available: true,
                account: None,
                plan: None,
                plan_type: None,
                primary: None,
                secondary: None,
                models: None,
                extra: None,
                error: None,
            }
        }
    }

    #[tokio::test(start_paused = true)]
    async fn fetch_all_returns_quota_and_iso_fetched_at() {
        let dir = tempdir().unwrap();
        let settings = settings();
        let client = reqwest::Client::new();
        let ctx = ctx_for(dir.path(), &settings, &client, 1_234);
        let providers: Vec<Box<dyn Provider>> = vec![Box::new(FakeProvider {
            id: "amp",
            name: "Amp",
            hang: false,
        })];
        let all = fetch_all(&providers, &ctx).await;
        assert_eq!(all.providers.len(), 1);
        assert!(all.providers[0].available);
        assert_eq!(all.fetched_at, "1970-01-01T00:00:01.234Z");
    }

    #[tokio::test(start_paused = true)]
    async fn fetch_all_maps_timeout_to_error_quota() {
        let dir = tempdir().unwrap();
        let settings = settings();
        let client = reqwest::Client::new();
        let ctx = ctx_for(dir.path(), &settings, &client, 0);
        let providers: Vec<Box<dyn Provider>> = vec![Box::new(FakeProvider {
            id: "claude",
            name: "Claude",
            hang: true,
        })];
        let all = fetch_all(&providers, &ctx).await;
        assert_eq!(all.providers.len(), 1);
        assert!(!all.providers[0].available);
        assert_eq!(
            all.providers[0].error.as_deref(),
            Some("Claude timed out after 10000ms")
        );
    }

    #[test]
    fn registry_has_claude_amp_and_codex() {
        let r = registry();
        assert_eq!(r.len(), 3);
        assert_eq!(r[0].id(), "claude");
        assert_eq!(r[1].id(), "amp");
        assert_eq!(r[2].id(), "codex");
        assert!(r.iter().any(|p| p.id() == "codex"));
    }
}
