# Plano 04a — Provider infra + Claude (async/tokio entra aqui)

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:subagent-driven-development. Steps usam checkbox (`- [ ]`).

**Goal:** Erguer a camada de providers async (tokio + reqwest) com o trait `Provider`, o cache async (`get_or_fetch`), o fan-out (`fetch_all`), os erros verbatim, e o **primeiro provider real (Claude)** produzindo `ProviderQuota`/`AllQuotas` byte-exact com o TS — alimentando a camada de formatação já pronta (03c).

**Architecture:** Camada impura sob DI. `main` (Plano 5) resolve `Paths`/relógio e injeta via `Ctx`. Trait `Provider { is_available, get_quota }` (async-trait, `?Send` — runtime `current_thread`). `get_quota` **nunca erra** (falhas viram `error` embutido no quota, igual ao `getQuota()` do TS); o boundary "cache só em sucesso" vive dentro, em `base::get_or_fetch`. Claude implementa `Provider` **direto** (fluxo próprio: expiry pré-request + check pós-cache). `fetch_all` faz `join_all` + `timeout(10s)` + 1 retry, mapeando timeout → quota de erro.

**Tech Stack:** tokio (`current_thread`, lazy), reqwest (rustls), async-trait, futures (`join_all`), regex, indexmap, serde. Testes: wiremock (HTTP), `#[tokio::test(start_paused)]` (timeout virtual), tempdir (`Paths`).

## Global Constraints

Toda task herda estas (copiadas verbatim do spec/CLAUDE.md/resume — são o contrato):

- **Contrato byte-exact do Waybar/Pango é SAGRADO. A autoridade é a saída do TS.** Rejeitar qualquer "fix" de review que divergiria do TS — validar contra a fonte TS, não aceitar cego.
- **Sem `unwrap()`/`expect()` em produção** (`#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]` em lib.rs E main.rs). Em `#[cfg(test)]` é permitido. Nunca `!` non-null. Estreitar com guard que `throw`a / fallback explícito (`unwrap_or`/`unwrap_or_else`/`ok()` são permitidos).
- **stdout limpo.** Logs → `log::{warn,error,debug}` (stderr). NUNCA `eprintln!`/`println!` na camada de provider.
- **Strings de erro de provider são CONTRATO** — verbatim (testes assertam a string exata). Mensagem padrão não-logado: `` Not logged in. Open `agent-bar menu` and choose Provider login. ``
- **Usar constantes de identidade** (`APP_NAME` etc. em `app_identity.rs`) em testes que verificam strings com `agent-bar`, não hardcode solto (ver T3).
- `ProviderQuota` é **serialize-only**; o cache faz round-trip do **raw por-provider** (structs `Serialize + Deserialize`), nunca do `ProviderQuota`.
- **Verificação:** `cargo test --manifest-path rust/Cargo.toml` + `cargo clippy --manifest-path rust/Cargo.toml --all-targets -- -D warnings`. **RTK reformata cargo** → o output é `cargo test: N passed (K suites)`; **NÃO existe `test result:`** (ler bruto com `... 2>&1 | tail -6`). `cargo test` aceita **só UM** filtro posicional.
- **`cargo fmt --manifest-path rust/Cargo.toml` ANTES de `git add`** (evita commit de fmt órfão).
- **Read antes de Edit** (`cat`/`sed`/`head` NÃO contam p/ o harness); se Edit falhar com `string not found`, re-Read antes de re-tentar — nunca editar de memória.
- Commits: Conventional Commits PT, subject ≤50 chars. Comentários/identificadores em inglês.
- **NÃO tocar main.rs** (continua sync; `#[tokio::main]` é Plano 5). A camada de provider é biblioteca, testada com `#[tokio::test]`.
- **NÃO tocar docs do projeto** (README/architecture descrevem o TS vigente) — só `docs/superpowers/` + `rust/`.

---

## File Structure

- `rust/Cargo.toml` — +deps (tokio, reqwest, async-trait, futures, regex, indexmap; dev: wiremock, tokio test-util).
- `rust/src/http.rs` (criar) — `static CLIENT: OnceLock<reqwest::Client>` + `client()`.
- `rust/src/lib.rs` (modificar) — `pub mod http;`.
- `rust/src/providers/types.rs` (modificar) — `models`/`weekly_models` → `IndexMap` (fidelidade de ordem).
- `rust/src/formatters/builders/claude.rs`, `rust/tests/golden.rs`, demais sites de teste (modificar) — ajustar construção de fixtures p/ `IndexMap` (compile-driven).
- `rust/src/providers/error.rs` (criar) — `ClaudeError`/`CodexError`/`AmpError` + `ProviderError`.
- `rust/src/providers/base.rs` (criar) — `quota_base`, `get_or_fetch`, trait `QuotaSource` + `base_get_quota`.
- `rust/src/providers/mod.rs` (modificar) — declara submódulos; `Ctx`; trait `Provider`; `registry()`; `fetch_all()`; `iso_from_ms()`.
- `rust/src/providers/claude.rs` (criar) — `derive_claude_plan` + `ClaudeProvider`.

---

### Task 1: Cargo deps + http.rs (cliente compartilhado)

**Files:**
- Modify: `rust/Cargo.toml`
- Create: `rust/src/http.rs`
- Modify: `rust/src/lib.rs`

**Interfaces:**
- Produces: `crate::http::client() -> &'static reqwest::Client` (UA `claude-code/2.1.179` + default header `anthropic-beta: oauth-2025-04-20` + timeout 5s).

- [ ] **Step 1: Adicionar deps no Cargo.toml**

Em `[dependencies]` (após `env_logger`):

```toml
tokio = { version = "1", features = ["rt", "macros", "time", "process", "io-util"] }
reqwest = { version = "0.12", default-features = false, features = ["rustls-tls", "json"] }
async-trait = "0.1"
futures = "0.3"
regex = "1"
indexmap = { version = "2", features = ["serde"] }
```

Em `[dev-dependencies]` (após `insta`):

```toml
wiremock = "0.6"
tokio = { version = "1", features = ["test-util", "macros", "rt"] }
```

(Cargo unifica as features das duas declarações de `tokio` — `test-util` fica disponível nos testes.)

- [ ] **Step 2: Criar `rust/src/http.rs`**

```rust
//! Cliente HTTP compartilhado. Um único `reqwest::Client` (pool de conexões +
//! init do rustls) reusado em todo o processo via `OnceLock` — construir um por
//! request desperdiça o pool. Só o Claude faz HTTP, então os headers default
//! (UA + anthropic-beta) são seguros como default do cliente.

use std::sync::OnceLock;
use std::time::Duration;

use crate::config::{CLAUDE_BETA_HEADER, CLAUDE_USER_AGENT, HTTP_TIMEOUT_SECS};

static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

/// Constrói (uma vez) e devolve o cliente compartilhado.
pub fn client() -> &'static reqwest::Client {
    CLIENT.get_or_init(|| {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            "anthropic-beta",
            reqwest::header::HeaderValue::from_static(CLAUDE_BETA_HEADER),
        );
        reqwest::Client::builder()
            .use_rustls_tls()
            .user_agent(CLAUDE_USER_AGENT)
            .default_headers(headers)
            .timeout(Duration::from_secs(HTTP_TIMEOUT_SECS))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_is_a_reused_singleton() {
        let a = client();
        let b = client();
        assert!(std::ptr::eq(a, b), "client() deve devolver a mesma instância");
    }
}
```

- [ ] **Step 3: Registrar o módulo em `lib.rs`**

Adicionar `pub mod http;` na lista de módulos (ordem alfabética: depois de `formatters`, antes de `logger`).

- [ ] **Step 4: Verificar**

Run: `cargo test --manifest-path rust/Cargo.toml http 2>&1 | tail -6` → o teste `client_is_a_reused_singleton` passa; build compila com as deps novas.
Run: `cargo clippy --manifest-path rust/Cargo.toml --all-targets -- -D warnings 2>&1 | tail -4` → sem issues.

- [ ] **Step 5: Commit**

```bash
cargo fmt --manifest-path rust/Cargo.toml
git add rust/Cargo.toml rust/Cargo.lock rust/src/http.rs rust/src/lib.rs
git commit -m "feat(rust): cliente HTTP compartilhado + deps async"
```

---

### Task 2: Fidelidade de ordem — `models`/`weekly_models` → IndexMap

**Contexto (por que):** O builder Claude **itera** `weekly_models` (`builders/claude.rs:97`). O producer TS gera keys `Opus`/`Sonnet`/`Cowork` em ordem de inserção; `Object.entries` preserva isso. `BTreeMap` re-ordena alfabético (`Cowork` primeiro) → **divergência byte-exact** do tooltip Pango quando `Cowork` existe. O golden atual só testou keys alfabéticas, mascarando a divergência. A resume note #42 pré-autorizou trocar p/ `IndexMap` exatamente neste gatilho. `models` (ProviderQuota) também é iterado no layout genérico do Amp → mesma correção. `models_detailed` (Codex) é re-ordenado por severity (ordem do mapa irrelevante) e `meta` (Amp) é acesso por chave → **ficam BTreeMap**.

**Files:**
- Modify: `rust/src/providers/types.rs` (campos `models` e `weekly_models` → `IndexMap`)
- Modify: sites de construção que o compilador apontar (`rust/src/formatters/builders/*.rs` tests, `rust/tests/golden.rs`, `rust/src/providers/types.rs` tests, etc.)

**Interfaces:**
- Produces: `ProviderQuota.models: Option<IndexMap<String, QuotaWindow>>` e `ClaudeQuotaExtra.weekly_models: Option<IndexMap<String, QuotaWindow>>` (ordem de inserção = ordem do `Object.entries` do TS).

- [ ] **Step 1: Trocar os tipos em `types.rs`**

Adicionar import no topo: `use indexmap::IndexMap;` (manter `use std::collections::BTreeMap;` — `models_detailed`/`meta`/`ModelWindows` continuam BTreeMap).

Trocar em `ProviderQuota`:
```rust
    #[serde(skip_serializing_if = "Option::is_none")]
    pub models: Option<IndexMap<String, QuotaWindow>>,
```
Trocar em `ClaudeQuotaExtra`:
```rust
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weekly_models: Option<IndexMap<String, QuotaWindow>>,
```
**NÃO** trocar `CodexQuotaExtra.models_detailed` nem `AmpQuotaExtra.meta` (continuam BTreeMap).

- [ ] **Step 2: Compile-driven — corrigir todos os sites**

Run: `cargo build --manifest-path rust/Cargo.toml --tests 2>&1 | tail -30`
Para CADA erro de tipo (`expected IndexMap, found BTreeMap`) que envolve `models` ou `weekly_models`: trocar aquele `BTreeMap::new()` por `IndexMap::new()` e adicionar `use indexmap::IndexMap;` no escopo de teste. **Deixar intactos** os `BTreeMap` que alimentam `models_detailed`/`meta` (e o `BTreeMap` interno de `codex_helpers.rs`, que lê `models_detailed`). Repetir até compilar limpo.

- [ ] **Step 3: Verificar paridade byte-exact PRESERVADA**

Run: `cargo test --manifest-path rust/Cargo.toml 2>&1 | tail -8`
Esperado: **todos** passam (181+). Os golden existentes têm fixtures com keys alfabéticas (= ordem de inserção), então `IndexMap` rende idêntico ao `BTreeMap`.
**SE algum golden divergir:** significa que a fixture inseriu em ordem ≠ ordem do `.snap` TS → corrigir a **ORDEM DE INSERÇÃO DA FIXTURE** (o `.snap` TS é a autoridade), **NUNCA** `cargo insta accept`.

Run: `cargo clippy --manifest-path rust/Cargo.toml --all-targets -- -D warnings 2>&1 | tail -4` → sem issues.

- [ ] **Step 4: Atualizar o comentário do golden**

Em `rust/src/formatters/builders/claude.rs`, achar o comentário `// weeklyModels em ordem alfabética (BTreeMap)` e trocar por `// weeklyModels em ordem de inserção (IndexMap = Object.entries do TS)`.

- [ ] **Step 5: Commit**

```bash
cargo fmt --manifest-path rust/Cargo.toml
git add rust/src/providers/types.rs rust/src/formatters/builders rust/tests/golden.rs
git commit -m "fix(rust): IndexMap em models/weekly p/ ordem do TS"
```

---

### Task 3: Erros por-provider (verbatim)

**Files:**
- Create: `rust/src/providers/error.rs`
- Modify: `rust/src/providers/mod.rs` (`pub mod error;`)

**Interfaces:**
- Produces: `ClaudeError`, `CodexError`, `AmpError` (todos `thiserror::Error + Debug`, `Display` = string verbatim), `ProviderError` (`#[from]` dos três).

- [ ] **Step 1: Criar `rust/src/providers/error.rs`**

```rust
//! Erros por-provider com mensagens VERBATIM (são contrato — testes assertam a
//! string exata). `ProviderError` agrega via `#[from]`. Só a camada de provider
//! usa estes tipos; comandos usam `anyhow`.

use thiserror::Error;

#[derive(Error, Debug)]
pub enum ClaudeError {
    #[error("Not logged in. Open `agent-bar menu` and choose Provider login.")]
    NotLoggedIn,
    #[error("Invalid credentials file")]
    InvalidCredentials,
    #[error("No access token")]
    NoAccessToken,
    #[error("Token expired. Open `agent-bar menu` and choose Provider login.")]
    TokenExpired,
    #[error("Request timeout")]
    Timeout,
    #[error("Claude API error: {0}")]
    Api(u16),
    #[error("Failed to fetch Claude usage")]
    Generic,
}

#[derive(Error, Debug)]
pub enum CodexError {
    #[error("Not logged in. Open `agent-bar menu` and choose Provider login.")]
    NotLoggedIn,
    #[error("No session data found")]
    NoSessionData,
    #[error("No rate limit data found (app-server + session log)")]
    NoRateLimitData,
    #[error("No quota windows found")]
    NoQuotaWindows,
    #[error("Failed to fetch Codex usage")]
    Generic,
}

#[derive(Error, Debug)]
pub enum AmpError {
    #[error("Amp CLI not installed. Right-click to install and log in.")]
    NotInstalled,
    #[error("Not logged in. Open `agent-bar menu` and choose Provider login.")]
    NotLoggedIn,
    #[error("Failed to parse usage")]
    ParseFailed,
    #[error("Failed to fetch Amp usage")]
    Generic,
}

#[derive(Error, Debug)]
pub enum ProviderError {
    #[error(transparent)]
    Claude(#[from] ClaudeError),
    #[error(transparent)]
    Codex(#[from] CodexError),
    #[error(transparent)]
    Amp(#[from] AmpError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_identity::APP_NAME;

    #[test]
    fn claude_strings_are_verbatim() {
        assert_eq!(
            ClaudeError::NotLoggedIn.to_string(),
            format!("Not logged in. Open `{APP_NAME} menu` and choose Provider login.")
        );
        assert_eq!(ClaudeError::InvalidCredentials.to_string(), "Invalid credentials file");
        assert_eq!(ClaudeError::NoAccessToken.to_string(), "No access token");
        assert_eq!(
            ClaudeError::TokenExpired.to_string(),
            format!("Token expired. Open `{APP_NAME} menu` and choose Provider login.")
        );
        assert_eq!(ClaudeError::Timeout.to_string(), "Request timeout");
        assert_eq!(ClaudeError::Api(404).to_string(), "Claude API error: 404");
        assert_eq!(ClaudeError::Generic.to_string(), "Failed to fetch Claude usage");
    }

    #[test]
    fn codex_strings_are_verbatim() {
        assert_eq!(
            CodexError::NotLoggedIn.to_string(),
            format!("Not logged in. Open `{APP_NAME} menu` and choose Provider login.")
        );
        assert_eq!(CodexError::NoSessionData.to_string(), "No session data found");
        assert_eq!(
            CodexError::NoRateLimitData.to_string(),
            "No rate limit data found (app-server + session log)"
        );
        assert_eq!(CodexError::NoQuotaWindows.to_string(), "No quota windows found");
        assert_eq!(CodexError::Generic.to_string(), "Failed to fetch Codex usage");
    }

    #[test]
    fn amp_strings_are_verbatim() {
        assert_eq!(
            AmpError::NotInstalled.to_string(),
            "Amp CLI not installed. Right-click to install and log in."
        );
        assert_eq!(
            AmpError::NotLoggedIn.to_string(),
            format!("Not logged in. Open `{APP_NAME} menu` and choose Provider login.")
        );
        assert_eq!(AmpError::ParseFailed.to_string(), "Failed to parse usage");
        assert_eq!(AmpError::Generic.to_string(), "Failed to fetch Amp usage");
    }

    #[test]
    fn provider_error_wraps_transparently() {
        let e: ProviderError = ClaudeError::NoAccessToken.into();
        assert_eq!(e.to_string(), "No access token");
    }
}
```

(Os testes amarram a string literal ao `APP_NAME` via `format!`, satisfazendo a regra de constantes de identidade sem perder a verificação verbatim.)

- [ ] **Step 2: Registrar o módulo**

Em `rust/src/providers/mod.rs`, adicionar `pub mod error;` (no topo, antes de `extras`).

- [ ] **Step 3: Verificar**

Run: `cargo test --manifest-path rust/Cargo.toml error 2>&1 | tail -6` → 4 testes passam.

- [ ] **Step 4: Commit**

```bash
cargo fmt --manifest-path rust/Cargo.toml
git add rust/src/providers/error.rs rust/src/providers/mod.rs
git commit -m "feat(rust): erros por-provider verbatim"
```

---

### Task 4: Trait Provider + Ctx + cache async + fan-out

**Files:**
- Create: `rust/src/providers/base.rs`
- Modify: `rust/src/providers/mod.rs` (Ctx, trait Provider, registry, fetch_all, iso_from_ms, `pub mod base;`)

**Interfaces:**
- Consumes: `error::ProviderError`, `types::{ProviderQuota, AllQuotas}`, `config::Paths`, `settings::Settings`, `cache::{get,set}`.
- Produces:
  - `Ctx<'a> { client, paths, settings, now_ms: u64, local_offset: UtcOffset, claude_usage_url: String, version: &'static str }` + `ttl_ms(provider) -> u64`.
  - `trait Provider { id/name/cache_key -> &'static str; async is_available(&Ctx) -> bool; async get_quota(&Ctx) -> ProviderQuota }` (`#[async_trait(?Send)]`).
  - `base::quota_base(id,name) -> ProviderQuota`; `base::get_or_fetch<T,E,F,Fut>(...) -> Result<T,E>` (só cacheia Ok); `base::QuotaSource` (template) + `base::base_get_quota`.
  - `registry() -> Vec<Box<dyn Provider>>` (vazio neste plano; Claude entra na T5); `fetch_all(&[Box<dyn Provider>], &Ctx) -> AllQuotas`; `iso_from_ms(u64) -> String`.

- [ ] **Step 1: Criar `rust/src/providers/base.rs`**

```rust
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
    fn build_quota(&self, raw: Self::Raw, base: ProviderQuota) -> ProviderQuota;
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
        Ok(raw) => source.build_quota(raw, base),
        Err(e) => {
            log::error!("Provider quota fetch error: provider={} error={e}", source.id());
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
        fn id(&self) -> &'static str { "amp" }
        fn name(&self) -> &'static str { "Amp" }
        fn cache_key(&self) -> &'static str { "fake-key" }
        async fn is_available(&self, _ctx: &Ctx<'_>) -> bool { self.available }
        async fn fetch_raw(&self, _ctx: &Ctx<'_>) -> Result<String, ProviderError> {
            self.calls.set(self.calls.get() + 1);
            if self.fail {
                Err(AmpError::ParseFailed.into())
            } else {
                Ok("RAW".to_string())
            }
        }
        fn build_quota(&self, raw: String, base: ProviderQuota) -> ProviderQuota {
            ProviderQuota { available: true, account: Some(raw), ..base }
        }
        fn unavailable_error(&self) -> String { AmpError::NotInstalled.to_string() }
        fn to_user_facing_error(&self, _e: &ProviderError) -> String {
            AmpError::ParseFailed.to_string()
        }
    }

    #[tokio::test]
    async fn unavailable_yields_error_quota() {
        let dir = tempdir().unwrap();
        let calls = Cell::new(0);
        let f = Fake { available: false, fail: false, calls: &calls };
        let (settings, client) = (crate::providers::test_support::settings(), reqwest::Client::new());
        let ctx = ctx_for(dir.path(), &settings, &client, 1_000);
        let q = base_get_quota(&f, &ctx).await;
        assert!(!q.available);
        assert_eq!(q.error.as_deref(), Some("Amp CLI not installed. Right-click to install and log in."));
        assert_eq!(calls.get(), 0, "não deve fazer fetch quando indisponível");
    }

    #[tokio::test]
    async fn success_builds_and_caches() {
        let dir = tempdir().unwrap();
        let calls = Cell::new(0);
        let settings = crate::providers::test_support::settings();
        let client = reqwest::Client::new();
        let f = Fake { available: true, fail: false, calls: &calls };
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
        let f = Fake { available: true, fail: true, calls: &calls };
        let ctx = ctx_for(dir.path(), &settings, &client, 1_000);
        let q = base_get_quota(&f, &ctx).await;
        assert!(!q.available);
        assert_eq!(q.error.as_deref(), Some("Failed to parse usage"));
        // erro não foi cacheado → segundo fetch ocorre.
        let _ = base_get_quota(&f, &ctx).await;
        assert_eq!(calls.get(), 2, "erro nunca é cacheado");
    }
}
```

- [ ] **Step 2: Reescrever `rust/src/providers/mod.rs`**

```rust
pub mod base;
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
    Vec::new()
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
                let msg = format!("{} timed out after {}ms", provider.name(), timeout.as_millis());
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
        fn id(&self) -> &'static str { self.id }
        fn name(&self) -> &'static str { self.name }
        fn cache_key(&self) -> &'static str { "fake" }
        async fn is_available(&self, _ctx: &Ctx<'_>) -> bool { true }
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
        let providers: Vec<Box<dyn Provider>> =
            vec![Box::new(FakeProvider { id: "amp", name: "Amp", hang: false })];
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
        let providers: Vec<Box<dyn Provider>> =
            vec![Box::new(FakeProvider { id: "claude", name: "Claude", hang: true })];
        let all = fetch_all(&providers, &ctx).await;
        assert_eq!(all.providers.len(), 1);
        assert!(!all.providers[0].available);
        assert_eq!(
            all.providers[0].error.as_deref(),
            Some("Claude timed out after 10000ms")
        );
    }

    #[test]
    fn registry_is_empty_in_this_plan() {
        assert_eq!(registry().len(), 0);
    }
}
```

**Nota p/ o implementer:** `Box::leak` no `test_support::ctx_for` é só p/ teste (vaza um `Paths` pequeno por teste). Se o clippy reclamar de `clippy::let_underscore_future` ou similar, ajustar; o `let _ = cache::set(...)` em `base.rs` é intencional (best-effort).

- [ ] **Step 3: Verificar**

Run: `cargo test --manifest-path rust/Cargo.toml 2>&1 | tail -8` → todos passam; os novos (`base::tests`, `mod::tests`) inclusos.
Run: `cargo clippy --manifest-path rust/Cargo.toml --all-targets -- -D warnings 2>&1 | tail -4` → sem issues.

- [ ] **Step 4: Commit**

```bash
cargo fmt --manifest-path rust/Cargo.toml
git add rust/src/providers/base.rs rust/src/providers/mod.rs
git commit -m "feat(rust): trait Provider + Ctx + cache async + fan-out"
```

---

### Task 5: ClaudeProvider (HTTP, expiry, parse)

**Files:**
- Create: `rust/src/providers/claude.rs`
- Modify: `rust/src/providers/mod.rs` (`pub mod claude;` + `registry()` retorna `vec![Box::new(claude::ClaudeProvider)]`)

**Interfaces:**
- Consumes: `Ctx`, `Provider`, `base::{quota_base, get_or_fetch}`, `error::ClaudeError`, `http::client` (via `ctx.client`), `types::{ProviderQuota, QuotaWindow, ProviderExtra, ClaudeQuotaExtra, ExtraUsage}`.
- Produces: `derive_claude_plan(Option<&str>, Option<&str>) -> String`; `ClaudeProvider` (unit struct) impl `Provider`.

**Contrato (do TS `claude.ts`), na ordem exata de `get_quota`:**
1. arquivo de credenciais não existe → `NotLoggedIn` (sem rede/cache).
2. parse falha → log error + `InvalidCredentials`.
3. sem `accessToken` (ausente OU vazio) → `NoAccessToken`.
4. `plan = derive_claude_plan(subscriptionType, rateLimitTier)`.
5. `expiresAt` (epoch **ms**) presente e `<= now_ms` → `{plan, error: TokenExpired}` (sem rede/cache).
6. `get_or_fetch` em volta do GET (UA+beta já no client default; `bearer_auth(token)`); non-200 → `Err(Api(status))` (não cacheado); timeout do reqwest → `Err(Timeout)`.
7. body com `error.error_code == "token_expired"` → `{plan, error: TokenExpired}` (esse body **é** cacheado; check roda pós-`get_or_fetch`).
8. parse: `remaining = 100 - round(util)`; `resetsAt = resets_at || null` (vazio→None); weekly `Opus`/`Sonnet`/`Cowork` (inserção nessa ordem); `extra_usage` só se `is_enabled` → `{enabled:true, remaining: round(100-util), limit: monthly_limit, used: round(used_credits)}`.
9. `extra` só se tiver weekly OU extra_usage. `available=true`, `plan`, `primary`, `secondary`.

- [ ] **Step 1: Criar `rust/src/providers/claude.rs`**

```rust
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
pub fn derive_claude_plan(subscription_type: Option<&str>, rate_limit_tier: Option<&str>) -> String {
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
        .map_err(|e| if e.is_timeout() { ClaudeError::Timeout } else { ClaudeError::Generic })?;
    let status = resp.status();
    if !status.is_success() {
        return Err(ClaudeError::Api(status.as_u16()));
    }
    resp.json::<ClaudeUsageResponse>()
        .await
        .map_err(|e| if e.is_timeout() { ClaudeError::Timeout } else { ClaudeError::Generic })
}

pub struct ClaudeProvider;

#[async_trait(?Send)]
impl Provider for ClaudeProvider {
    fn id(&self) -> &'static str { "claude" }
    fn name(&self) -> &'static str { "Claude" }
    fn cache_key(&self) -> &'static str { "claude-usage" }

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
            return ProviderQuota { error: Some(ClaudeError::NotLoggedIn.to_string()), ..base };
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
            _ => return ProviderQuota { error: Some(ClaudeError::NoAccessToken.to_string()), ..base },
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
        let fetched = get_or_fetch(&ctx.paths.cache_dir, self.cache_key(), ttl, ctx.now_ms, || {
            fetch_usage(client, &url, &token)
        })
        .await;

        let usage = match fetched {
            Ok(u) => u,
            Err(ClaudeError::Timeout) => {
                log::warn!("Claude API timeout");
                return ProviderQuota { plan: Some(plan), error: Some(ClaudeError::Timeout.to_string()), ..base };
            }
            Err(e @ ClaudeError::Api(_)) => {
                log::warn!("Claude API error: {e}");
                return ProviderQuota { plan: Some(plan), error: Some(e.to_string()), ..base };
            }
            Err(_) => {
                log::error!("Claude API fetch error");
                return ProviderQuota { plan: Some(plan), error: Some(ClaudeError::Generic.to_string()), ..base };
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

        let extra_usage = usage.extra_usage.as_ref().filter(|e| e.is_enabled).map(|e| ExtraUsage {
            enabled: true,
            remaining: (100.0 - e.utilization).round(),
            limit: e.monthly_limit,
            used: e.used_credits.round(),
        });

        let extra = if !weekly.is_empty() || extra_usage.is_some() {
            Some(ProviderExtra::Claude(ClaudeQuotaExtra {
                weekly_models: if weekly.is_empty() { None } else { Some(weekly) },
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
```

- [ ] **Step 2: Testes em `rust/src/providers/claude.rs`** (append `#[cfg(test)] mod tests`)

Cobrir: `derive_claude_plan` (Max 5x; sem dup; unknown; Pro intacto); `is_available`; e o fluxo `get_quota` via wiremock + tempdir. Porte as asserções de `tests/providers/claude.test.ts` (utilization 0-100, weekly Opus/Sonnet/Cowork em ordem, extra_usage gate, token_expired pré e pós, non-200, missing/invalid creds).

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::test_support::{ctx_for, settings};
    use serde_json::json;
    use tempfile::tempdir;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    fn derive_plan_variants() {
        assert_eq!(derive_claude_plan(Some("max"), Some("default_claude_max_5x")), "max 5x");
        assert_eq!(derive_claude_plan(Some("Max 20x"), Some("tier_20x")), "Max 20x"); // já contém 20x
        assert_eq!(derive_claude_plan(Some("Pro"), None), "Pro");
        assert_eq!(derive_claude_plan(None, Some("default_claude_max_5x")), "unknown");
        assert_eq!(derive_claude_plan(Some("  "), None), "unknown");
    }

    fn write_creds(path: &Path, body: serde_json::Value) {
        std::fs::write(path, body.to_string()).unwrap();
    }

    fn ctx_with_url<'a>(
        dir: &Path,
        settings: &'a Settings,
        client: &'a reqwest::Client,
        url: String,
        now_ms: u64,
    ) -> Ctx<'a> {
        let mut ctx = ctx_for(dir.path(), settings, client, now_ms);
        ctx.claude_usage_url = url;
        ctx
    }
    use crate::settings::Settings;

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
        let ctx = ctx_with_url(dir, &s, &client, url, 1_000);
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
        let ctx = ctx_with_url(dir, &s, &client, url, 1_000);
        write_creds(&ctx.paths.claude_credentials, json!({"claudeAiOauth":{"accessToken":"tok"}}));
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
        let ctx = ctx_with_url(dir, &s, &client, url, 1_000);
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
}
```

**Nota p/ o implementer:** se o clippy reclamar de `ctx.now_ms as f64` (cast precision), manter — é a comparação correta (epoch ms cabe exato em f64 até ~2^53). Se reclamar de `clippy::unwrap_used` em algum teste, ok (testes permitem). Os `.unwrap()`/`panic!` acima estão em `#[cfg(test)]`.

- [ ] **Step 3: Registrar Claude no registry**

Em `rust/src/providers/mod.rs`: adicionar `pub mod claude;` (após `pub mod base;`) e trocar `registry()`:
```rust
pub fn registry() -> Vec<Box<dyn Provider>> {
    vec![Box::new(claude::ClaudeProvider)]
}
```
E atualizar o teste `registry_is_empty_in_this_plan` → `registry_has_claude` (assert len 1 + `registry()[0].id() == "claude"`).

- [ ] **Step 4: Verificar**

Run: `cargo test --manifest-path rust/Cargo.toml 2>&1 | tail -8` → todos passam (incl. claude tests via wiremock).
Run: `cargo clippy --manifest-path rust/Cargo.toml --all-targets -- -D warnings 2>&1 | tail -4` → sem issues.

- [ ] **Step 5: Commit**

```bash
cargo fmt --manifest-path rust/Cargo.toml
git add rust/src/providers/claude.rs rust/src/providers/mod.rs
git commit -m "feat(rust): ClaudeProvider (HTTP, expiry, parse)"
```

---

## Self-Review (autor)

- **Cobertura do spec:** §3.3 (Claude headers/expiry/post-check/util 0-100/weekly keys/extra_usage gate) → T5. §3.2 cache (atômico, erro não-cacheado, TTL per-provider) → T4 `get_or_fetch` + `ctx.ttl_ms`. §5.2 fan-out (join_all + timeout 10s + 1 retry) → T4 `fetch_one`/`fetch_all`. §5.3 trait → T4. §5.4 erros verbatim → T3. HTTP client compartilhado (§4) → T1.
- **Fidelidade de ordem:** weekly_models/models → IndexMap (T2) p/ casar `Object.entries` do TS; models_detailed (Codex re-ordena por severity) e meta (acesso por chave) ficam BTreeMap.
- **Sem placeholders:** todo passo tem código real. Testes têm asserções concretas.
- **Consistência de tipos:** `Ctx`/`Provider`/`QuotaSource`/`get_or_fetch` assinaturas casam entre T4 e T5. `ProviderExtra::Claude(ClaudeQuotaExtra{...})` casa com types.rs.
- **DEFERIDO p/ planos seguintes:** Amp (04b), Codex (04c), notify (04d). `registry()` cresce a cada um.
