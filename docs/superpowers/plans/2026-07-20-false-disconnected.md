# Falso "disconnected" — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Parar de mostrar "disconnected" na Waybar quando o usuário está logado — token expirado renovável e erro transitório não são logout.

**Architecture:** Três frentes: (1) Grok considera logado quem tem `key` no auth.json (o provider é zero-rede e nem usa o token; o access token de 6h é renovado pelo Grok CLI via refresh_token); (2) infra de stale-cache — `cache::get_stale` ignora TTL, `ProviderQuota.stale_reason` marca dado servido de cache vencido, `ProviderError::is_transient()` classifica erros; `base_get_quota` e o fluxo inline do Claude servem stale em erro transitório em vez de erro; (3) timeout do `amp usage` vira `AmpError::Timeout` (transitório) em vez de `NotLoggedIn`. O tooltip ganha linha "⚠️ Cached data — {reason}"; o render do módulo fica normal (percentuais) porque quota stale tem `error=None` — `disconnected` sobra só para logout real ou erro sem cache algum.

**Tech Stack:** Rust, tokio, serde, insta (snapshots), tempfile.

## Global Constraints

- Rust/cargo only; sem rede/CLIs vivas em teste; mocks via seams.
- Nunca `unwrap()`/`expect()` em código de produção (testes podem).
- Error strings são contrato — testes assertam verbatim (`src/providers/error.rs`).
- stdout limpo (logs via `log::` → stderr).
- Identificadores/UI em inglês; commits em PT, Conventional Commits, subject ≤50 chars.
- `ClaudeProvider` implementa `Provider` direto (cache inline); Codex/Amp/Grok usam `base_get_quota`.
- XML-escape só em `render_pango.rs` — builders nunca escapam.
- Rodar `cargo fmt` antes de cada commit; clippy `-D warnings` no fim.
- **Gotcha RTK:** hook reformata output do cargo; usar no máximo um filtro posicional por invocação de `cargo test`.

---

### Task 1: Grok — logado por presença de `key`, não por `expires_at`

Contexto: `~/.grok/auth.json` tem access token de 6h renovado só quando o Grok CLI roda. `parse_auth_entries` hoje faz `logged_in = expires_at > now` → usuário logado (com refresh_token válido) aparece "Not logged in" na barra. O provider é zero-rede (lê só `sessions/**/signals.json`) — a expiração é irrelevante.

**Files:**
- Modify: `src/providers/grok.rs` (fn `parse_auth_entries`, ~linhas 83-130; testes `parse_auth_expired` ~529, `not_logged_in_expired` ~627)

**Interfaces:**
- Produces: `parse_auth_entries(bytes, _now) -> Result<AuthView, GrokError>` — `logged_in = true` sse existe entry com `key` não vazio. Assinatura inalterada (param `now` vira `_now`).

- [ ] **Step 1: Ajustar testes existentes para a nova semântica (failing)**

Em `src/providers/grok.rs`, trocar os dois testes:

```rust
#[test]
fn parse_auth_expired_token_still_logged_in() {
    let now = datetime!(2026-07-17 12:00:00 UTC);
    let view = parse_auth_entries(&fixture_bytes("auth-expired.json"), now).unwrap();
    // Access token vencido ≠ logout: o Grok CLI renova via refresh_token.
    assert!(view.logged_in);
    assert_eq!(view.account.as_deref(), Some("Test User"));
}

#[test]
fn parse_auth_empty_key_not_logged_in() {
    let now = datetime!(2026-07-17 12:00:00 UTC);
    let json = br#"{"https://auth.x.ai::c": {"key": "", "first_name": "X"}}"#;
    let view = parse_auth_entries(json, now).unwrap();
    assert!(!view.logged_in);
    assert!(view.account.is_none());
}
```

E substituir o teste `not_logged_in_expired` (~linha 627) por:

```rust
#[tokio::test]
async fn expired_access_token_still_serves_quota() {
    let dir = tempdir().unwrap();
    write_auth(dir.path(), "auth-expired.json");
    write_signals(
        dir.path(),
        "proj/sid/signals.json",
        &fixture_str("signals-recent.json"),
    );
    let s = settings();
    let client = reqwest::Client::new();
    let ctx = ctx_for(dir.path(), &s, &client, 1_720_000_000_000);
    let q = GrokProvider.get_quota(&ctx).await;
    assert!(q.available, "err={:?}", q.error);
    assert_eq!(q.account.as_deref(), Some("Test User"));
    assert_eq!(q.primary.as_ref().unwrap().remaining, 90.0);
}
```

- [ ] **Step 2: Rodar e ver falhar**

Run: `cargo test providers::grok`
Expected: FAIL em `parse_auth_expired_token_still_logged_in` e `expired_access_token_still_serves_quota` (logged_in=false hoje).

- [ ] **Step 3: Implementar**

Substituir o corpo de `parse_auth_entries` (o doc-comment também):

```rust
/// Parse de `auth.json`. JSON inválido → `InvalidCredentials`.
/// Logado = existe entry com `key` não vazio. `expires_at` NÃO desloga:
/// o access token (6h) é renovado pelo Grok CLI via refresh_token, e este
/// provider é zero-rede — nem usa o token, só lê signals.json.
pub(crate) fn parse_auth_entries(
    bytes: &[u8],
    _now: OffsetDateTime,
) -> Result<AuthView, GrokError> {
    let map: HashMap<String, AuthEntry> =
        serde_json::from_slice(bytes).map_err(|_| GrokError::InvalidCredentials)?;

    // Entre entries com key não vazio, preferir a de expires_at mais distante
    // (desempate estável quando o CLI mantém múltiplas entradas).
    let mut best: Option<(OffsetDateTime, AuthEntry)> = None;
    for (_k, entry) in map {
        let key = entry.key.as_deref().unwrap_or("").trim();
        if key.is_empty() {
            continue;
        }
        let exp = entry
            .expires_at
            .as_deref()
            .and_then(parse_expires_at)
            .unwrap_or(OffsetDateTime::UNIX_EPOCH);
        match &best {
            Some((prev_exp, _)) if exp <= *prev_exp => {}
            _ => best = Some((exp, entry)),
        }
    }

    let Some((_exp, entry)) = best else {
        return Ok(AuthView {
            account: None,
            logged_in: false,
        });
    };

    Ok(AuthView {
        account: Some(account_label(&entry)),
        logged_in: true,
    })
}
```

Nota: a flag `any_with_key` some (o `best` já cobre); o caminho `logged_in=false` em `build_quota` (grok.rs:374) continua existindo para key vazio/ausente — não tocar.

- [ ] **Step 4: Rodar e ver passar**

Run: `cargo test providers::grok`
Expected: PASS (todos).

- [ ] **Step 5: fmt + commit**

```bash
cargo fmt
git add src/providers/grok.rs
git commit -m "fix: grok logado por key, não por expires_at"
```

---

### Task 2: Infra stale — `cache::get_stale`, `stale_reason`, `is_transient`

**Files:**
- Modify: `src/cache.rs` (nova fn `get_stale` + teste)
- Modify: `src/providers/types.rs` (`ProviderQuota.stale_reason` + teste)
- Modify: `src/providers/error.rs` (`ProviderError::is_transient` + variante `AmpError::Timeout` + testes verbatim)
- Modify: `src/providers/base.rs` (`quota_base` ganha o campo)

**Interfaces:**
- Produces: `cache::get_stale<T: DeserializeOwned>(cache_dir: &Path, key: &str) -> Option<T>` — lê ignorando TTL.
- Produces: `ProviderQuota.stale_reason: Option<String>` — `#[serde(skip_serializing_if = "Option::is_none")]`, camelCase `staleReason`.
- Produces: `ProviderError::is_transient(&self) -> bool`.
- Produces: `AmpError::Timeout` com string verbatim `"Request timeout"`.

- [ ] **Step 1: Testes failing**

Em `src/cache.rs` (mod tests):

```rust
#[test]
fn get_stale_ignores_expiry() {
    let dir = tempdir().unwrap();
    set(dir.path(), "k", &"v".to_string(), 5_000, 1_000).unwrap();
    // now (10_000) > expires_at (6_000): get normal → None, stale → Some
    let fresh: Option<String> = get(dir.path(), "k", 10_000);
    assert_eq!(fresh, None);
    let stale: Option<String> = get_stale(dir.path(), "k");
    assert_eq!(stale, Some("v".to_string()));
}

#[test]
fn get_stale_none_on_missing_or_corrupt() {
    let dir = tempdir().unwrap();
    let missing: Option<String> = get_stale(dir.path(), "nope");
    assert_eq!(missing, None);
    std::fs::write(dir.path().join("bad.json"), b"{ not json").unwrap();
    let corrupt: Option<String> = get_stale(dir.path(), "bad");
    assert_eq!(corrupt, None);
}
```

Em `src/providers/types.rs` (mod tests):

```rust
#[test]
fn stale_reason_omitted_when_none_present_when_some() {
    let mut q = ProviderQuota {
        provider: "amp".into(),
        display_name: "Amp".into(),
        available: true,
        account: None,
        plan: None,
        plan_type: None,
        primary: None,
        secondary: None,
        models: None,
        extra: None,
        error: None,
        stale_reason: None,
    };
    let j = serde_json::to_value(&q).unwrap();
    assert!(j.get("staleReason").is_none());
    q.stale_reason = Some("Request timeout".into());
    let j = serde_json::to_value(&q).unwrap();
    assert_eq!(j["staleReason"], "Request timeout");
}
```

Em `src/providers/error.rs` (mod tests):

```rust
#[test]
fn amp_timeout_string_verbatim() {
    assert_eq!(AmpError::Timeout.to_string(), "Request timeout");
}

#[test]
fn transient_classification() {
    assert!(ProviderError::from(ClaudeError::Timeout).is_transient());
    assert!(ProviderError::from(ClaudeError::Api(500)).is_transient());
    assert!(ProviderError::from(ClaudeError::Generic).is_transient());
    assert!(!ProviderError::from(ClaudeError::NotLoggedIn).is_transient());
    assert!(!ProviderError::from(ClaudeError::TokenExpired).is_transient());
    assert!(ProviderError::from(AmpError::Timeout).is_transient());
    assert!(ProviderError::from(AmpError::Generic).is_transient());
    assert!(ProviderError::from(AmpError::ParseFailed).is_transient());
    assert!(!ProviderError::from(AmpError::NotLoggedIn).is_transient());
    assert!(!ProviderError::from(AmpError::NotInstalled).is_transient());
    assert!(ProviderError::from(CodexError::Generic).is_transient());
    assert!(!ProviderError::from(CodexError::NotLoggedIn).is_transient());
    assert!(!ProviderError::from(CodexError::NoRateLimitData).is_transient());
    assert!(!ProviderError::from(GrokError::NotLoggedIn).is_transient());
}
```

- [ ] **Step 2: Rodar e ver falhar (erro de compilação = fail esperado)**

Run: `cargo test cache`
Expected: FAIL — `get_stale` não existe; depois `cargo test settings` nem rodará até o structs compilar.

- [ ] **Step 3: Implementar**

`src/cache.rs` — após `get`:

```rust
/// Lê o cache IGNORANDO o TTL (`None` só em miss/corrompido/key inválida).
/// Para fallback em erro transitório: dado velho identificado no tooltip
/// é mais honesto que ícone de desconectado quando o usuário está logado.
pub fn get_stale<T: DeserializeOwned>(cache_dir: &Path, key: &str) -> Option<T> {
    let path = cache_path(cache_dir, key).ok()?;
    let bytes = std::fs::read(&path).ok()?;
    let entry: CacheEntryOwned<T> = serde_json::from_slice(&bytes).ok()?;
    Some(entry.data)
}
```

`src/providers/types.rs` — em `ProviderQuota`, após `error`:

```rust
    /// Motivo de dado stale: erro transitório respondido com cache vencido.
    /// `None` = dado fresco (ou erro real em `error`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stale_reason: Option<String>,
```

`src/providers/error.rs` — variante em `AmpError` (antes de `ParseFailed`):

```rust
    #[error("Request timeout")]
    Timeout,
```

e o impl no fim do bloco de enums (antes de `#[cfg(test)]`):

```rust
impl ProviderError {
    /// Transitório = falha de infra (rede/CLI/parse) que não significa
    /// logout; o caller pode servir cache stale. Credencial/logout → false.
    pub fn is_transient(&self) -> bool {
        match self {
            ProviderError::Claude(e) => matches!(
                e,
                ClaudeError::Timeout | ClaudeError::Api(_) | ClaudeError::Generic
            ),
            ProviderError::Codex(e) => matches!(e, CodexError::Generic),
            ProviderError::Amp(e) => matches!(
                e,
                AmpError::Timeout | AmpError::Generic | AmpError::ParseFailed
            ),
            ProviderError::Grok(_) => false,
        }
    }
}
```

`src/providers/base.rs` — `quota_base` ganha `stale_reason: None`. Depois, deixar o compilador guiar: `cargo build` aponta todo struct literal completo de `ProviderQuota` (produção e testes, ex.: `shared.rs` tests, `types.rs` tests, `formatters/*` tests) — adicionar `stale_reason: None` em cada um. NÃO tocar em construções via `..base`/`..q`.

- [ ] **Step 4: Rodar e ver passar**

Run: `cargo test cache` — Expected: PASS.
Run: `cargo test providers` — Expected: PASS (compila tudo; erros verbatim ok).

- [ ] **Step 5: fmt + commit**

```bash
cargo fmt
git add -A src/
git commit -m "feat: cache stale + stale_reason na quota"
```

---

### Task 3: `base_get_quota` — fallback stale em erro transitório

**Files:**
- Modify: `src/providers/base.rs` (match final de `base_get_quota`, linhas 87-99; testes)

**Interfaces:**
- Consumes: `cache::get_stale`, `ProviderError::is_transient`, `ProviderQuota.stale_reason` (Task 2).
- Produces: contrato — erro transitório + cache (mesmo vencido) presente ⇒ quota construída normal com `stale_reason = Some(user_facing_error)`; sem cache ⇒ comportamento atual (quota de erro).

- [ ] **Step 1: Teste failing**

No mod tests de `base.rs` (o `Fake` com `fail: true` retorna `AmpError::ParseFailed`, que é transitório):

```rust
#[tokio::test]
async fn transient_error_with_stale_cache_serves_stale() {
    let dir = tempdir().unwrap();
    let calls = Cell::new(0);
    let settings = crate::providers::test_support::settings();
    let client = reqwest::Client::new();
    let f = Fake {
        available: true,
        fail: true,
        calls: &calls,
    };
    // Cache vencido: escrito em t=0 com ttl=1ms; ctx roda em t=1_000.
    crate::cache::set(dir.path(), "fake-key", &"OLD".to_string(), 1, 0).unwrap();
    let ctx = ctx_for(dir.path(), &settings, &client, 1_000);
    let q = base_get_quota(&f, &ctx).await;
    assert!(q.available, "stale deve construir quota normal");
    assert_eq!(q.account.as_deref(), Some("OLD"));
    assert_eq!(q.error, None);
    assert_eq!(q.stale_reason.as_deref(), Some("Failed to parse usage"));
}
```

Atenção: `ctx_for(dir.path(), ...)` precisa apontar `cache_dir` para `dir.path()` — conferir em `src/providers/test_support.rs` como `cache_dir` é derivado do home de teste e escrever o cache::set no path certo (se `cache_dir = home.join(...)`, usar esse join no teste).

- [ ] **Step 2: Rodar e ver falhar**

Run: `cargo test providers::base`
Expected: FAIL — hoje retorna quota de erro (`available=false`).

- [ ] **Step 3: Implementar**

Substituir o braço `Err(e)` de `base_get_quota`:

```rust
        Err(e) => {
            log::error!(
                "Provider quota fetch error: provider={} error={e}",
                source.id()
            );
            // Erro transitório não é logout: servir cache mesmo vencido,
            // marcado como stale, em vez de derrubar pra "disconnected".
            if e.is_transient() {
                if let Some(raw) =
                    cache::get_stale::<S::Raw>(&ctx.paths.cache_dir, source.cache_key())
                {
                    let mut q = source.build_quota(raw, base.clone(), ctx);
                    q.stale_reason = Some(source.to_user_facing_error(&e));
                    return q;
                }
            }
            ProviderQuota {
                error: Some(source.to_user_facing_error(&e)),
                ..base
            }
        }
```

(`base` passa a precisar de `.clone()` — `ProviderQuota` já deriva `Clone`.)

- [ ] **Step 4: Rodar e ver passar**

Run: `cargo test providers::base`
Expected: PASS (incluindo `error_is_not_cached_and_maps_message`, que não tem cache seedado e segue no caminho de erro).

- [ ] **Step 5: fmt + commit**

```bash
cargo fmt
git add src/providers/base.rs
git commit -m "feat: fallback stale em erro transitório"
```

---

### Task 4: Claude — token expirado/timeout servem cache stale

Contexto: `claude.rs` tem fluxo inline (não usa `base_get_quota`). Hoje `expiresAt <= now` → short-circuit `TokenExpired` → disconnected, mesmo com `refreshToken` no arquivo (Claude Code renova sozinho na próxima execução) e mesmo com cache utilizável no disco.

**Files:**
- Modify: `src/providers/claude.rs` (get_quota, linhas ~348-398 + extração do bloco usage→quota ~400-520; testes)

**Interfaces:**
- Consumes: `cache::get`, `cache::get_stale` (Task 2).
- Produces: `fn quota_from_usage(usage: <tipo do fetch>, plan: String, base: ProviderQuota, stale_reason: Option<String>) -> ProviderQuota` — função privada do módulo; contém TODO o bloco atual pós-fetch (do check `usage.error == token_expired` até o `ProviderQuota` final, movido verbatim), com o literal final recebendo `stale_reason`.

- [ ] **Step 1: Testes failing**

Seguir o padrão dos testes existentes do módulo (ver `missing_credentials_yields_not_logged_in` e o helper que escreve `.credentials.json` com `expiresAt`). Adicionar:

```rust
#[tokio::test]
async fn token_expired_with_stale_cache_serves_stale() {
    // credentials com expiresAt no passado + cache claude-usage vencido no disco
    // → quota disponível, stale_reason = TokenExpired, error = None.
    // Montagem: escrever credentials como nos testes vizinhos (expiresAt: 5000,
    // now_ms do ctx > 5000); escrever cache com cache::set(cache_dir,
    // "claude-usage", &usage_json_fixture, ttl_ms=1, now_ms=0).
    // Asserts:
    //   assert!(q.available);
    //   assert_eq!(q.error, None);
    //   assert_eq!(
    //       q.stale_reason.as_deref(),
    //       Some("Token expired. Open `agent-bar menu` and choose Provider login.")
    //   );
    //   assert!(q.primary.is_some());
}

#[tokio::test]
async fn token_expired_without_cache_keeps_error() {
    // credentials expiradas, sem cache → comportamento atual:
    //   q.error == Some(TokenExpired verbatim), available=false.
}
```

O corpo exato segue os helpers do módulo (fixture de usage: reutilizar o JSON dos testes de sucesso existentes do claude.rs — há fixture/`json!` com `five_hour`/`limits`). O tipo cacheado é o mesmo `T` que `get_or_fetch` já roundtripa hoje (o retorno de `fetch_usage`).

- [ ] **Step 2: Rodar e ver falhar**

Run: `cargo test providers::claude`
Expected: FAIL no primeiro teste novo (hoje retorna erro TokenExpired direto).

- [ ] **Step 3: Extrair `quota_from_usage` e implementar fallback**

1. Extrair de `get_quota` o bloco inteiro que começa em `// Check pós-cache: body 200 pode trazer token_expired.` (linha ~400) até o `ProviderQuota` final da função, para:

```rust
fn quota_from_usage(
    usage: ClaudeUsageResponse,
    plan: String,
    base: ProviderQuota,
    stale_reason: Option<String>,
) -> ProviderQuota {
    // [bloco movido verbatim: check token_expired no body, limits/legado,
    //  extra_usage, models/extra…]
    // No literal final, acrescentar:
    //     stale_reason,
    // (as saídas de erro internas — ex. token_expired no body — mantêm
    //  `..base`, que já tem stale_reason: None do quota_base)
}
```

(Se o nome do tipo de usage for outro, usar o tipo real do retorno de `fetch_usage`.)

2. No caminho de sucesso atual, substituir o bloco movido por:

```rust
        quota_from_usage(usage, plan, base, None)
```

3. Substituir o short-circuit de token expirado (linhas ~348-357) por:

```rust
        // Token expirado: sem rede (o refresh é do Claude Code, não nosso).
        // Cache válido → serve normal; cache vencido → serve como stale;
        // sem cache → erro (disconnected honesto).
        if let Some(exp) = oauth.as_ref().and_then(|o| o.expires_at) {
            if exp <= ctx.now_ms as f64 {
                if let Some(usage) = cache::get::<ClaudeUsageResponse>(
                    &ctx.paths.cache_dir,
                    self.cache_key(),
                    ctx.now_ms,
                ) {
                    return quota_from_usage(usage, plan, base, None);
                }
                if let Some(usage) = cache::get_stale::<ClaudeUsageResponse>(
                    &ctx.paths.cache_dir,
                    self.cache_key(),
                ) {
                    return quota_from_usage(
                        usage,
                        plan,
                        base,
                        Some(ClaudeError::TokenExpired.to_string()),
                    );
                }
                return ProviderQuota {
                    plan: Some(plan),
                    error: Some(ClaudeError::TokenExpired.to_string()),
                    ..base
                };
            }
        }
```

4. Nos braços de erro do fetch (`Timeout`, `Api`, `Generic`, linhas ~374-397), antes de retornar o erro, tentar stale:

```rust
            Err(e) => {
                log::warn!("Claude API error: {e}");
                if let Some(usage) = cache::get_stale::<ClaudeUsageResponse>(
                    &ctx.paths.cache_dir,
                    self.cache_key(),
                ) {
                    return quota_from_usage(usage, plan, base, Some(e.to_string()));
                }
                return ProviderQuota {
                    plan: Some(plan),
                    error: Some(e.to_string()),
                    ..base
                };
            }
```

Consolidar os três braços num só como acima SÓ se as mensagens resultantes ficarem idênticas às atuais (`ClaudeError::Timeout/Api/Generic` têm `Display` próprio; o braço atual de `Generic` loga com `log::error!` e converte `e` → manter os níveis de log atuais por braço se divergirem).

- [ ] **Step 4: Rodar e ver passar**

Run: `cargo test providers::claude`
Expected: PASS (novos + existentes — nenhuma string de erro mudou).

- [ ] **Step 5: fmt + commit**

```bash
cargo fmt
git add src/providers/claude.rs
git commit -m "feat: claude serve cache stale sem rede"
```

---

### Task 5: Amp — timeout não é logout

**Files:**
- Modify: `src/providers/amp.rs` (linha ~199 + comentário; `to_user_facing_error` ~249; testes do módulo)
- Modify: `src/action_right.rs` (teste novo)

**Interfaces:**
- Consumes: `AmpError::Timeout` (Task 2).
- Produces: timeout do `amp usage` → `AmpError::Timeout` (transitório ⇒ Task 3 serve stale automaticamente).

- [ ] **Step 1: Testes failing**

Em `src/action_right.rs` (mod tests, junto dos vizinhos):

```rust
#[test]
fn request_timeout_is_not_disconnected_for_amp() {
    assert!(!looks_disconnected("amp", Some("Request timeout")));
}
```

Em `src/providers/amp.rs`: localizar o teste que cobre timeout→NotLoggedIn (rg por `NotLoggedIn` no mod tests) e atualizar a expectativa para `"Request timeout"`. Se não existir teste direto de timeout (o seam pode não simular timeout), garantir pelo menos o mapeamento:

```rust
#[test]
fn timeout_maps_to_own_message() {
    let e: ProviderError = AmpError::Timeout.into();
    assert_eq!(AmpProvider.to_user_facing_error(&e), "Request timeout");
}
```

- [ ] **Step 2: Rodar e ver falhar**

Run: `cargo test providers::amp`
Expected: FAIL (mapeamento ainda cai no braço default/NotLoggedIn).

- [ ] **Step 3: Implementar**

Em `amp.rs` linha ~198-199, trocar:

```rust
        // timeout: CLI viva demais não é logout — erro transitório; a base
        // serve cache stale. (Divergência consciente do TS, que deslogava.)
        Err(_) => return Err(AmpError::Timeout.into()),
```

Em `to_user_facing_error` (~249), garantir que `AmpError::Timeout` passa a própria mensagem (`e.to_string()` no braço `ProviderError::Amp(e)`), preservando as strings atuais dos demais casos.

- [ ] **Step 4: Rodar e ver passar**

Run: `cargo test providers::amp`
Expected: PASS.
Run: `cargo test action_right`
Expected: PASS.

- [ ] **Step 5: fmt + commit**

```bash
cargo fmt
git add src/providers/amp.rs src/action_right.rs
git commit -m "fix: timeout do amp não é logout"
```

---

### Task 6: Tooltip — aviso "Cached data" para quota stale

Contexto: quota stale tem `available=true, error=None` ⇒ o módulo Waybar já renderiza percentuais normais (nenhuma mudança de classe CSS). Falta o aviso no tooltip para o dado velho ser identificável.

**Files:**
- Modify: `src/formatters/builders/shared.rs` (nova `stale_line`)
- Modify: `src/formatters/builders/{generic,claude,codex,amp,grok}.rs` (chamada após o header)
- Test: teste unitário em `generic.rs` + teste de integração em `src/formatters/waybar.rs`

**Interfaces:**
- Consumes: `ProviderQuota.stale_reason` (Task 2).
- Produces: `pub fn stale_line(p: &ProviderQuota) -> Option<Line>` em `builders/shared.rs`; linha `⚠️ Cached data — {reason}` em `ColorToken::Yellow` logo após o header de cada builder.

- [ ] **Step 1: Testes failing**

Em `src/formatters/builders/generic.rs` (mod tests, seguindo o padrão de `error_branch_replaces_primary`):

```rust
#[test]
fn stale_reason_adds_warning_line() {
    let mut q = quota();
    q.stale_reason = Some("Request timeout".into());
    let lines = build_generic(&clk(), &q, &opts());
    // header + stale + primary + footer
    assert_eq!(lines.len(), 4);
    let rendered = render_pango(&lines);
    assert!(rendered.contains("Cached data — Request timeout"));
}
```

Em `src/formatters/waybar.rs` (mod tests, seguindo `per_provider_disconnected`):

```rust
#[test]
fn per_provider_stale_renders_normal_with_warning() {
    let mut q = quota_ok(); // usar o helper existente dos testes vizinhos
    q.stale_reason = Some("Request timeout".into());
    let out = format_provider_for_waybar(&clk(), &q, &settings(), DisplayMode::Remaining);
    assert!(!out.class.contains("disconnected"));
    assert!(out.tooltip.contains("Cached data — Request timeout"));
}
```

(Adaptar nomes de helpers `quota_ok`/`clk`/`settings` aos que o mod tests de waybar.rs realmente tem.)

- [ ] **Step 2: Rodar e ver falhar**

Run: `cargo test formatters`
Expected: FAIL nos dois novos.

- [ ] **Step 3: Implementar**

Em `src/formatters/builders/shared.rs`:

```rust
/// Linha de aviso quando a quota veio de cache vencido (erro transitório).
/// Builders nunca escapam — o escape é do render_pango.
pub fn stale_line(p: &ProviderQuota) -> Option<Line> {
    let reason = p.stale_reason.as_deref()?;
    Some(vec![
        Segment::new(box_chars::V, ColorToken::Text),
        Segment::raw_text("  "),
        Segment::new(format!("⚠️ Cached data — {reason}"), ColorToken::Yellow),
    ])
}
```

(Ajustar imports do arquivo: `Line`, `Segment`, `box_chars`, `ColorToken`, `ProviderQuota` — mesmos usados em `generic.rs`.)

Em cada builder (`generic.rs`, `claude.rs`, `codex.rs`, `amp.rs`, `grok.rs`), logo após o push do `header_line(...)`:

```rust
    if let Some(l) = stale_line(p) {
        lines.push(l);
    }
```

(Em builders cujo parâmetro não se chama `p`, usar o nome local. Import de `stale_line` junto dos demais de `super::shared`.)

- [ ] **Step 4: Rodar e ver passar + snapshots**

Run: `cargo test formatters`
Expected: PASS. Snapshots insta existentes NÃO devem mudar (stale_reason=None em todos os fixtures atuais) — se algum snapshot mudar, é regressão: investigar, não aceitar.

Run: `cargo test --test golden`
Expected: PASS sem mudanças.

- [ ] **Step 5: fmt + commit**

```bash
cargo fmt
git add src/formatters/
git commit -m "feat: aviso de dado em cache no tooltip"
```

---

### Task 7: Gate final

- [ ] **Step 1: Suíte completa + clippy**

Run: `cargo test`
Expected: PASS.
Run: `cargo clippy --all-targets -- -D warnings`
Expected: sem warnings.

- [ ] **Step 2: Smoke real (read-only)**

Run: `cargo run -- --provider grok` (stdout JSON)
Expected: com o auth.json real do usuário (access token vencido de 19/07, key presente), `"class":"agent-bar-grok ..."` SEM `disconnected`, tooltip com dados de sessão.

- [ ] **Step 3: Commit final se houver sobras (fmt/docs)**

```bash
git status
```

Somente arquivos do plano; nada de commit espontâneo além dos previstos.

## Limitações aceitas (documentar no PR)

- Erro transitório SEM nenhum cache no disco continua renderizando `disconnected` (primeira execução offline). Estado neutro novo exigiria mudança no contrato CSS do Waybar — fora de escopo.
- `CodexError::NoRateLimitData` segue não-transitório: no Codex esse erro costuma significar auth quebrada (o `action_right` o trata como desconexão); reclassificar exigiria evidência de falso positivo.
- O check de `token_expired` no body 200 do Claude (dentro de `quota_from_usage`) mantém o comportamento atual (erro), pois o dado que chegou junto não é confiável.
