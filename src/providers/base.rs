//! Orquestração compartilhada (template "BaseProvider" do TS) + cache async.
//! Codex/Amp usam `base_get_quota`; Claude tem fluxo próprio (cache inline).

use std::future::Future;
use std::path::Path;

use async_trait::async_trait;
use serde::{de::DeserializeOwned, Serialize};

use super::error::ProviderError;
use super::types::ProviderQuota;
use super::Ctx;
use crate::cache;

/// Base mínima de um quota antes do fetch (`available=false`).
pub fn quota_base(id: &str, name: &str) -> ProviderQuota {
    ProviderQuota {
        provider: id.to_string(),
        display_name: name.to_string(),
        available: false,
        account: None,
        plan: None,
        plan_type: None,
        primary: None,
        secondary: None,
        models: None,
        extra: None,
        error: None,
        stale_reason: None,
    }
}

/// Cache-wrapper: devolve o cache se válido; senão chama `fetcher` e **só
/// cacheia em sucesso** (`Err` propaga sem `set`). Espelha `cache.getOrFetch`.
pub async fn get_or_fetch<T, E, F, Fut>(
    cache_dir: &Path,
    key: &str,
    ttl_ms: u64,
    now_ms: u64,
    fetcher: F,
) -> Result<T, E>
where
    T: Serialize + DeserializeOwned,
    F: FnOnce() -> Fut,
    Fut: Future<Output = Result<T, E>>,
{
    if let Some(cached) = cache::get::<T>(cache_dir, key, now_ms) {
        return Ok(cached);
    }
    let data = fetcher().await?;
    let _ = cache::set(cache_dir, key, &data, ttl_ms, now_ms);
    Ok(data)
}

/// Fonte de quota no estilo template (Codex/Amp). Implementa só o que difere.
#[async_trait(?Send)]
pub trait QuotaSource {
    type Raw: Serialize + DeserializeOwned;
    fn id(&self) -> &'static str;
    fn name(&self) -> &'static str;
    fn cache_key(&self) -> &'static str;
    async fn is_available(&self, ctx: &Ctx<'_>) -> bool;
    /// Dado cru cacheável; `Err` nunca é cacheado.
    async fn fetch_raw(&self, ctx: &Ctx<'_>) -> Result<Self::Raw, ProviderError>;
    fn build_quota(&self, raw: Self::Raw, base: ProviderQuota, ctx: &Ctx<'_>) -> ProviderQuota;
    fn unavailable_error(&self) -> String;
    fn to_user_facing_error(&self, error: &ProviderError) -> String;
}

/// Orquestração: gate de disponibilidade → cache (só sucesso) → build.
pub async fn base_get_quota<S: QuotaSource>(source: &S, ctx: &Ctx<'_>) -> ProviderQuota {
    let base = quota_base(source.id(), source.name());
    if !source.is_available(ctx).await {
        return ProviderQuota {
            error: Some(source.unavailable_error()),
            ..base
        };
    }
    let ttl = ctx.ttl_ms(source.id());
    let result = get_or_fetch(
        &ctx.paths.cache_dir,
        source.cache_key(),
        ttl,
        ctx.now_ms,
        || source.fetch_raw(ctx),
    )
    .await;
    match result {
        Ok(raw) => source.build_quota(raw, base, ctx),
        Err(e) => {
            log::error!(
                "Provider quota fetch error: provider={} error={e}",
                source.id()
            );
            ProviderQuota {
                error: Some(source.to_user_facing_error(&e)),
                ..base
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::error::AmpError;
    use crate::providers::test_support::ctx_for;
    use std::cell::Cell;
    use tempfile::tempdir;

    // QuotaSource fake p/ exercitar base_get_quota + get_or_fetch.
    struct Fake<'a> {
        available: bool,
        fail: bool,
        calls: &'a Cell<u32>,
    }

    #[async_trait(?Send)]
    impl QuotaSource for Fake<'_> {
        type Raw = String;
        fn id(&self) -> &'static str {
            "amp"
        }
        fn name(&self) -> &'static str {
            "Amp"
        }
        fn cache_key(&self) -> &'static str {
            "fake-key"
        }
        async fn is_available(&self, _ctx: &Ctx<'_>) -> bool {
            self.available
        }
        async fn fetch_raw(&self, _ctx: &Ctx<'_>) -> Result<String, ProviderError> {
            self.calls.set(self.calls.get() + 1);
            if self.fail {
                Err(AmpError::ParseFailed.into())
            } else {
                Ok("RAW".to_string())
            }
        }
        fn build_quota(&self, raw: String, base: ProviderQuota, _ctx: &Ctx<'_>) -> ProviderQuota {
            ProviderQuota {
                available: true,
                account: Some(raw),
                ..base
            }
        }
        fn unavailable_error(&self) -> String {
            AmpError::NotInstalled.to_string()
        }
        fn to_user_facing_error(&self, _e: &ProviderError) -> String {
            AmpError::ParseFailed.to_string()
        }
    }

    #[tokio::test]
    async fn unavailable_yields_error_quota() {
        let dir = tempdir().unwrap();
        let calls = Cell::new(0);
        let f = Fake {
            available: false,
            fail: false,
            calls: &calls,
        };
        let (settings, client) = (
            crate::providers::test_support::settings(),
            reqwest::Client::new(),
        );
        let ctx = ctx_for(dir.path(), &settings, &client, 1_000);
        let q = base_get_quota(&f, &ctx).await;
        assert!(!q.available);
        assert_eq!(
            q.error.as_deref(),
            Some("Amp CLI not installed. Right-click to install and log in.")
        );
        assert_eq!(calls.get(), 0, "não deve fazer fetch quando indisponível");
    }

    #[tokio::test]
    async fn success_builds_and_caches() {
        let dir = tempdir().unwrap();
        let calls = Cell::new(0);
        let settings = crate::providers::test_support::settings();
        let client = reqwest::Client::new();
        let f = Fake {
            available: true,
            fail: false,
            calls: &calls,
        };
        let ctx = ctx_for(dir.path(), &settings, &client, 1_000);
        let q = base_get_quota(&f, &ctx).await;
        assert!(q.available);
        assert_eq!(q.account.as_deref(), Some("RAW"));
        // segunda chamada: serve do cache, sem novo fetch.
        let _ = base_get_quota(&f, &ctx).await;
        assert_eq!(calls.get(), 1, "fetch só uma vez (cache hit no 2º)");
    }

    #[tokio::test]
    async fn error_is_not_cached_and_maps_message() {
        let dir = tempdir().unwrap();
        let calls = Cell::new(0);
        let settings = crate::providers::test_support::settings();
        let client = reqwest::Client::new();
        let f = Fake {
            available: true,
            fail: true,
            calls: &calls,
        };
        let ctx = ctx_for(dir.path(), &settings, &client, 1_000);
        let q = base_get_quota(&f, &ctx).await;
        assert!(!q.available);
        assert_eq!(q.error.as_deref(), Some("Failed to parse usage"));
        // erro não foi cacheado → segundo fetch ocorre.
        let _ = base_get_quota(&f, &ctx).await;
        assert_eq!(calls.get(), 2, "erro nunca é cacheado");
    }
}
