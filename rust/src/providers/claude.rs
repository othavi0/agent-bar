//! Claude provider. Implementa `Provider` DIRETO (fluxo próprio: expiry
//! pré-request + check pós-cache). Port fiel de `src/providers/claude.ts`.

use std::path::Path;
use std::sync::OnceLock;

use async_trait::async_trait;
use indexmap::IndexMap;
use regex::Regex;
use serde::{Deserialize, Serialize};

use super::base::{get_or_fetch, quota_base};
use super::error::ClaudeError;
use super::types::{ClaudeQuotaExtra, ExtraUsage, ProviderExtra, ProviderQuota, QuotaWindow};
use super::{Ctx, Provider};

/// Resolve o plano de exibição a partir de `subscriptionType` + `rateLimitTier`
/// (o tier carrega o multiplicador, ex. `default_claude_max_5x` → `Max 5x`).
pub fn derive_claude_plan(
    subscription_type: Option<&str>,
    rate_limit_tier: Option<&str>,
) -> String {
    let sub = match subscription_type.map(str::trim).filter(|s| !s.is_empty()) {
        Some(s) => s,
        None => return "unknown".to_string(),
    };
    static RE: OnceLock<Option<Regex>> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"(?i)_(\d+)x$").ok());
    let mult = rate_limit_tier
        .and_then(|tier| re.as_ref().and_then(|r| r.captures(tier)))
        .and_then(|caps| caps.get(1).map(|m| m.as_str().to_string()));
    match mult {
        Some(m) if !sub.to_lowercase().contains(&format!("{m}x")) => format!("{sub} {m}x"),
        _ => sub.to_string(),
    }
}

// ---- Credenciais (deserialize do ~/.claude/.credentials.json) ----

#[derive(Debug, Deserialize)]
struct ClaudeCredentials {
    #[serde(rename = "claudeAiOauth")]
    claude_ai_oauth: Option<ClaudeOauth>,
}

#[derive(Debug, Deserialize)]
struct ClaudeOauth {
    #[serde(rename = "accessToken")]
    access_token: Option<String>,
    #[serde(rename = "subscriptionType")]
    subscription_type: Option<String>,
    #[serde(rename = "rateLimitTier")]
    rate_limit_tier: Option<String>,
    /// Epoch em ms.
    #[serde(rename = "expiresAt")]
    expires_at: Option<f64>,
}

// ---- Resposta da API (raw cacheável: Serialize + Deserialize) ----

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ClaudeWindowRaw {
    utilization: f64,
    #[serde(default)]
    resets_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ClaudeExtraUsageRaw {
    is_enabled: bool,
    monthly_limit: f64,
    used_credits: f64,
    utilization: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ClaudeErrorRaw {
    error_code: String,
    #[allow(dead_code)]
    message: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct ClaudeUsageResponse {
    #[serde(default)]
    five_hour: Option<ClaudeWindowRaw>,
    #[serde(default)]
    seven_day: Option<ClaudeWindowRaw>,
    #[serde(default)]
    seven_day_opus: Option<ClaudeWindowRaw>,
    #[serde(default)]
    seven_day_sonnet: Option<ClaudeWindowRaw>,
    #[serde(default)]
    seven_day_cowork: Option<ClaudeWindowRaw>,
    #[serde(default)]
    extra_usage: Option<ClaudeExtraUsageRaw>,
    #[serde(default)]
    error: Option<ClaudeErrorRaw>,
}

fn read_credentials(path: &Path) -> Option<ClaudeCredentials> {
    let bytes = std::fs::read(path).ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn window_from(raw: &ClaudeWindowRaw) -> QuotaWindow {
    let used = raw.utilization.round();
    QuotaWindow {
        remaining: 100.0 - used,
        resets_at: raw.resets_at.clone().filter(|s| !s.is_empty()),
        window_minutes: None,
        used: None,
    }
}

/// O GET cru (sem cache). `Err` mapeado p/ `ClaudeError` (não cacheado).
async fn fetch_usage(
    client: &reqwest::Client,
    url: &str,
    token: &str,
) -> Result<ClaudeUsageResponse, ClaudeError> {
    let resp = client
        .get(url)
        .bearer_auth(token)
        .send()
        .await
        .map_err(|e| {
            if e.is_timeout() {
                ClaudeError::Timeout
            } else {
                ClaudeError::Generic
            }
        })?;
    let status = resp.status();
    if !status.is_success() {
        return Err(ClaudeError::Api(status.as_u16()));
    }
    resp.json::<ClaudeUsageResponse>().await.map_err(|e| {
        if e.is_timeout() {
            ClaudeError::Timeout
        } else {
            ClaudeError::Generic
        }
    })
}

pub struct ClaudeProvider;

#[async_trait(?Send)]
impl Provider for ClaudeProvider {
    fn id(&self) -> &'static str {
        "claude"
    }
    fn name(&self) -> &'static str {
        "Claude"
    }
    fn cache_key(&self) -> &'static str {
        "claude-usage"
    }

    async fn is_available(&self, ctx: &Ctx<'_>) -> bool {
        read_credentials(&ctx.paths.claude_credentials)
            .and_then(|c| c.claude_ai_oauth)
            .and_then(|o| o.access_token)
            .is_some_and(|t| !t.is_empty())
    }

    async fn get_quota(&self, ctx: &Ctx<'_>) -> ProviderQuota {
        let base = quota_base(self.id(), self.name());
        let path = &ctx.paths.claude_credentials;

        if !path.exists() {
            return ProviderQuota {
                error: Some(ClaudeError::NotLoggedIn.to_string()),
                ..base
            };
        }
        let creds = match read_credentials(path) {
            Some(c) => c,
            None => {
                log::error!("Failed to parse Claude credentials");
                return ProviderQuota {
                    error: Some(ClaudeError::InvalidCredentials.to_string()),
                    ..base
                };
            }
        };
        let oauth = creds.claude_ai_oauth;
        let access_token = match oauth.as_ref().and_then(|o| o.access_token.clone()) {
            Some(t) if !t.is_empty() => t,
            _ => {
                return ProviderQuota {
                    error: Some(ClaudeError::NoAccessToken.to_string()),
                    ..base
                }
            }
        };

        let plan = derive_claude_plan(
            oauth.as_ref().and_then(|o| o.subscription_type.as_deref()),
            oauth.as_ref().and_then(|o| o.rate_limit_tier.as_deref()),
        );

        // Short-circuit pré-request: token já expirado → sem rede, sem cache.
        if let Some(exp) = oauth.as_ref().and_then(|o| o.expires_at) {
            if exp <= ctx.now_ms as f64 {
                return ProviderQuota {
                    plan: Some(plan),
                    error: Some(ClaudeError::TokenExpired.to_string()),
                    ..base
                };
            }
        }

        let ttl = ctx.ttl_ms("claude");
        let url = ctx.claude_usage_url.clone();
        let token = access_token;
        let client = ctx.client;
        let fetched = get_or_fetch(
            &ctx.paths.cache_dir,
            self.cache_key(),
            ttl,
            ctx.now_ms,
            || fetch_usage(client, &url, &token),
        )
        .await;

        let usage = match fetched {
            Ok(u) => u,
            Err(ClaudeError::Timeout) => {
                log::warn!("Claude API timeout");
                return ProviderQuota {
                    plan: Some(plan),
                    error: Some(ClaudeError::Timeout.to_string()),
                    ..base
                };
            }
            Err(e @ ClaudeError::Api(_)) => {
                log::warn!("Claude API error: {e}");
                return ProviderQuota {
                    plan: Some(plan),
                    error: Some(e.to_string()),
                    ..base
                };
            }
            Err(_) => {
                log::error!("Claude API fetch error");
                return ProviderQuota {
                    plan: Some(plan),
                    error: Some(ClaudeError::Generic.to_string()),
                    ..base
                };
            }
        };

        // Check pós-cache: body 200 pode trazer token_expired.
        if usage.error.as_ref().map(|e| e.error_code.as_str()) == Some("token_expired") {
            return ProviderQuota {
                plan: Some(plan),
                error: Some(ClaudeError::TokenExpired.to_string()),
                ..base
            };
        }

        let primary = usage.five_hour.as_ref().map(window_from);
        let secondary = usage.seven_day.as_ref().map(window_from);

        let mut weekly: IndexMap<String, QuotaWindow> = IndexMap::new();
        if let Some(w) = usage.seven_day_opus.as_ref() {
            weekly.insert("Opus".to_string(), window_from(w));
        }
        if let Some(w) = usage.seven_day_sonnet.as_ref() {
            weekly.insert("Sonnet".to_string(), window_from(w));
        }
        if let Some(w) = usage.seven_day_cowork.as_ref() {
            weekly.insert("Cowork".to_string(), window_from(w));
        }

        let extra_usage = usage
            .extra_usage
            .as_ref()
            .filter(|e| e.is_enabled)
            .map(|e| ExtraUsage {
                enabled: true,
                remaining: (100.0 - e.utilization).round(),
                limit: e.monthly_limit,
                used: e.used_credits.round(),
            });

        let extra = if !weekly.is_empty() || extra_usage.is_some() {
            Some(ProviderExtra::Claude(ClaudeQuotaExtra {
                weekly_models: if weekly.is_empty() {
                    None
                } else {
                    Some(weekly)
                },
                extra_usage,
            }))
        } else {
            None
        };

        ProviderQuota {
            available: true,
            plan: Some(plan),
            primary,
            secondary,
            extra,
            ..base
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::test_support::{ctx_for, settings};
    use crate::settings::Settings;
    use serde_json::json;
    use tempfile::tempdir;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    fn derive_plan_variants() {
        assert_eq!(
            derive_claude_plan(Some("max"), Some("default_claude_max_5x")),
            "max 5x"
        );
        assert_eq!(
            derive_claude_plan(Some("Max 20x"), Some("tier_20x")),
            "Max 20x"
        ); // já contém 20x
        assert_eq!(derive_claude_plan(Some("Pro"), None), "Pro");
        assert_eq!(
            derive_claude_plan(None, Some("default_claude_max_5x")),
            "unknown"
        );
        assert_eq!(derive_claude_plan(Some("  "), None), "unknown");
    }

    fn write_creds(path: &Path, body: serde_json::Value) {
        std::fs::write(path, body.to_string()).unwrap();
    }

    fn ctx_with_url<'a>(
        dir: &tempfile::TempDir,
        settings: &'a Settings,
        client: &'a reqwest::Client,
        url: String,
        now_ms: u64,
    ) -> Ctx<'a> {
        let mut ctx = ctx_for(dir.path(), settings, client, now_ms);
        ctx.claude_usage_url = url;
        ctx
    }

    #[tokio::test]
    async fn missing_credentials_yields_not_logged_in() {
        let dir = tempdir().unwrap();
        let s = settings();
        let client = reqwest::Client::new();
        let ctx = ctx_for(dir.path(), &s, &client, 0);
        // o tempdir não tem o arquivo claude.json
        let q = ClaudeProvider.get_quota(&ctx).await;
        assert!(!q.available);
        assert_eq!(
            q.error.as_deref(),
            Some("Not logged in. Open `agent-bar menu` and choose Provider login.")
        );
    }

    #[tokio::test]
    async fn expired_token_short_circuits_without_network() {
        let dir = tempdir().unwrap();
        let s = settings();
        let client = reqwest::Client::new();
        let ctx = ctx_for(dir.path(), &s, &client, 10_000);
        write_creds(
            &ctx.paths.claude_credentials,
            json!({"claudeAiOauth":{"accessToken":"t","subscriptionType":"Pro","expiresAt":5000}}),
        );
        let q = ClaudeProvider.get_quota(&ctx).await;
        assert_eq!(q.plan.as_deref(), Some("Pro"));
        assert_eq!(
            q.error.as_deref(),
            Some("Token expired. Open `agent-bar menu` and choose Provider login.")
        );
    }

    #[tokio::test]
    async fn fetches_and_parses_windows_and_weekly() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/oauth/usage"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "five_hour": {"utilization": 25.0, "resets_at": "2026-03-28T14:00:00Z"},
                "seven_day": {"utilization": 40.0, "resets_at": "2026-04-01T00:00:00Z"},
                "seven_day_opus": {"utilization": 60.0},
                "seven_day_sonnet": {"utilization": 35.0},
                "seven_day_cowork": {"utilization": 10.0},
                "extra_usage": {"is_enabled": true, "monthly_limit": 5000.0, "used_credits": 2250.4, "utilization": 45.0}
            })))
            .mount(&server)
            .await;

        let dir = tempdir().unwrap();
        let s = settings();
        let client = reqwest::Client::new();
        let url = format!("{}/api/oauth/usage", server.uri());
        let ctx = ctx_with_url(&dir, &s, &client, url, 1_000);
        write_creds(
            &ctx.paths.claude_credentials,
            json!({"claudeAiOauth":{"accessToken":"tok","subscriptionType":"max","rateLimitTier":"default_claude_max_5x"}}),
        );

        let q = ClaudeProvider.get_quota(&ctx).await;
        assert!(q.available);
        assert_eq!(q.plan.as_deref(), Some("max 5x"));
        assert_eq!(q.primary.as_ref().unwrap().remaining, 75.0);
        assert_eq!(q.secondary.as_ref().unwrap().remaining, 60.0);
        // weekly em ordem de inserção Opus, Sonnet, Cowork (IndexMap)
        let extra = match q.extra.as_ref().unwrap() {
            ProviderExtra::Claude(c) => c,
            _ => panic!("esperava Claude extra"),
        };
        let weekly = extra.weekly_models.as_ref().unwrap();
        let keys: Vec<&str> = weekly.keys().map(String::as_str).collect();
        assert_eq!(keys, vec!["Opus", "Sonnet", "Cowork"]);
        assert_eq!(weekly["Opus"].remaining, 40.0);
        let eu = extra.extra_usage.as_ref().unwrap();
        assert_eq!(eu.remaining, 55.0);
        assert_eq!(eu.used, 2250.0); // round(2250.4)
        assert_eq!(eu.limit, 5000.0);
    }

    #[tokio::test]
    async fn non_200_maps_to_api_error_and_is_not_cached() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/oauth/usage"))
            .respond_with(ResponseTemplate::new(429))
            .mount(&server)
            .await;
        let dir = tempdir().unwrap();
        let s = settings();
        let client = reqwest::Client::new();
        let url = format!("{}/api/oauth/usage", server.uri());
        let ctx = ctx_with_url(&dir, &s, &client, url, 1_000);
        write_creds(
            &ctx.paths.claude_credentials,
            json!({"claudeAiOauth":{"accessToken":"tok"}}),
        );
        let q = ClaudeProvider.get_quota(&ctx).await;
        assert!(!q.available);
        assert_eq!(q.error.as_deref(), Some("Claude API error: 429"));
        // não cacheado: o arquivo de cache não deve existir.
        let cache_file = ctx.paths.cache_dir.join("claude-usage.json");
        assert!(!cache_file.exists(), "non-200 não pode ser cacheado");
    }

    #[tokio::test]
    async fn token_expired_in_body_after_200() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/oauth/usage"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "error": {"error_code": "token_expired", "message": "expired"}
            })))
            .mount(&server)
            .await;
        let dir = tempdir().unwrap();
        let s = settings();
        let client = reqwest::Client::new();
        let url = format!("{}/api/oauth/usage", server.uri());
        let ctx = ctx_with_url(&dir, &s, &client, url, 1_000);
        write_creds(
            &ctx.paths.claude_credentials,
            json!({"claudeAiOauth":{"accessToken":"tok","subscriptionType":"Pro"}}),
        );
        let q = ClaudeProvider.get_quota(&ctx).await;
        assert_eq!(q.plan.as_deref(), Some("Pro"));
        assert_eq!(
            q.error.as_deref(),
            Some("Token expired. Open `agent-bar menu` and choose Provider login.")
        );
    }

    // ---- is_available variants ----

    #[tokio::test]
    async fn is_available_no_file_returns_false() {
        let dir = tempdir().unwrap();
        let s = settings();
        let client = reqwest::Client::new();
        let ctx = ctx_for(dir.path(), &s, &client, 0);
        // nenhum arquivo de credenciais no tempdir
        assert!(!ClaudeProvider.is_available(&ctx).await);
    }

    #[tokio::test]
    async fn is_available_valid_token_returns_true() {
        let dir = tempdir().unwrap();
        let s = settings();
        let client = reqwest::Client::new();
        let ctx = ctx_for(dir.path(), &s, &client, 0);
        write_creds(
            &ctx.paths.claude_credentials,
            json!({"claudeAiOauth":{"accessToken":"tok"}}),
        );
        assert!(ClaudeProvider.is_available(&ctx).await);
    }

    #[tokio::test]
    async fn is_available_empty_token_returns_false() {
        let dir = tempdir().unwrap();
        let s = settings();
        let client = reqwest::Client::new();
        let ctx = ctx_for(dir.path(), &s, &client, 0);
        write_creds(
            &ctx.paths.claude_credentials,
            json!({"claudeAiOauth":{"accessToken":""}}),
        );
        assert!(!ClaudeProvider.is_available(&ctx).await);
    }

    #[tokio::test]
    async fn is_available_no_oauth_section_returns_false() {
        let dir = tempdir().unwrap();
        let s = settings();
        let client = reqwest::Client::new();
        let ctx = ctx_for(dir.path(), &s, &client, 0);
        write_creds(&ctx.paths.claude_credentials, json!({}));
        assert!(!ClaudeProvider.is_available(&ctx).await);
    }

    // ---- invalid_credentials_file ----

    #[tokio::test]
    async fn invalid_credentials_file_returns_error() {
        let dir = tempdir().unwrap();
        let s = settings();
        let client = reqwest::Client::new();
        let ctx = ctx_for(dir.path(), &s, &client, 0);
        std::fs::write(&ctx.paths.claude_credentials, b"{ not json").unwrap();
        let q = ClaudeProvider.get_quota(&ctx).await;
        assert!(!q.available);
        assert_eq!(q.error.as_deref(), Some("Invalid credentials file"));
    }

    // ---- no_access_token ----

    #[tokio::test]
    async fn no_access_token_returns_error_without_plan() {
        let dir = tempdir().unwrap();
        let s = settings();
        let client = reqwest::Client::new();
        let ctx = ctx_for(dir.path(), &s, &client, 0);
        // claudeAiOauth presente mas sem accessToken
        write_creds(
            &ctx.paths.claude_credentials,
            json!({"claudeAiOauth":{"subscriptionType":"Pro"}}),
        );
        let q = ClaudeProvider.get_quota(&ctx).await;
        assert!(!q.available);
        assert_eq!(q.error.as_deref(), Some("No access token"));
        // early-return antes de derivar plano
        assert!(q.plan.is_none());
    }

    // ---- non_round_utilization ----

    #[tokio::test]
    async fn non_round_utilization_rounds_correctly() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/oauth/usage"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "five_hour": {"utilization": 33.7},
                "seven_day": {"utilization": 10.4}
            })))
            .mount(&server)
            .await;

        let dir = tempdir().unwrap();
        let s = settings();
        let client = reqwest::Client::new();
        let url = format!("{}/api/oauth/usage", server.uri());
        let ctx = ctx_with_url(&dir, &s, &client, url, 1_000);
        write_creds(
            &ctx.paths.claude_credentials,
            json!({"claudeAiOauth":{"accessToken":"tok","subscriptionType":"Pro"}}),
        );

        let q = ClaudeProvider.get_quota(&ctx).await;
        // 33.7 → round → 34 → remaining = 100 - 34 = 66
        assert_eq!(q.primary.as_ref().unwrap().remaining, 66.0);
        // 10.4 → round → 10 → remaining = 100 - 10 = 90
        assert_eq!(q.secondary.as_ref().unwrap().remaining, 90.0);
    }

    // ---- extra_usage_disabled_is_omitted ----

    #[tokio::test]
    async fn extra_usage_disabled_is_omitted() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/oauth/usage"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "five_hour": {"utilization": 10.0},
                "extra_usage": {
                    "is_enabled": false,
                    "monthly_limit": 5000.0,
                    "used_credits": 100.0,
                    "utilization": 50.0
                }
            })))
            .mount(&server)
            .await;

        let dir = tempdir().unwrap();
        let s = settings();
        let client = reqwest::Client::new();
        let url = format!("{}/api/oauth/usage", server.uri());
        let ctx = ctx_with_url(&dir, &s, &client, url, 1_000);
        write_creds(
            &ctx.paths.claude_credentials,
            json!({"claudeAiOauth":{"accessToken":"tok","subscriptionType":"Pro"}}),
        );

        let q = ClaudeProvider.get_quota(&ctx).await;
        // extra_usage desabilitado + sem weekly → extra deve ser None
        assert!(q.extra.is_none());
    }

    // ---- resets_at_empty_string_becomes_none ----

    #[tokio::test]
    async fn resets_at_empty_string_becomes_none() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/oauth/usage"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "five_hour": {"utilization": 20.0, "resets_at": ""}
            })))
            .mount(&server)
            .await;

        let dir = tempdir().unwrap();
        let s = settings();
        let client = reqwest::Client::new();
        let url = format!("{}/api/oauth/usage", server.uri());
        let ctx = ctx_with_url(&dir, &s, &client, url, 1_000);
        write_creds(
            &ctx.paths.claude_credentials,
            json!({"claudeAiOauth":{"accessToken":"tok","subscriptionType":"Pro"}}),
        );

        let q = ClaudeProvider.get_quota(&ctx).await;
        // string vazia deve ser convertida para None
        assert!(q.primary.as_ref().unwrap().resets_at.is_none());
    }
}
