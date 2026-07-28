# Fase 1 — Claude Completo — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Restaurar o monitoramento das janelas 5h/semanal do Claude ponta a ponta: header de auth correto, buckets completos, IDs sem colisão, resets tolerantes, retry compartilhado, expiry acionável e tier real.

**Architecture:** Toda a Fase 1 vive no helper Rust (`src/providers/`, `src/status/`). O adapter Claude (`adapters.rs`) coleta via HTTP OAuth; `v2_map.rs` faz o parse puro para `ProviderResult`; `collect.rs` mapeia para `ProviderStatus`; o coordinator aplica cache/retenção. Zero mudança de QML nesta fase — a UI já renderiza `windows`/`resetsAt` corretamente quando o dado chega.

**Tech Stack:** Rust (tokio, serde, time, reqwest via seam injetável), testes com fakes (`ScriptedHttpClient`, `MapFileSystem`, `FixedClock`).

## Global Constraints

- Rust/Cargo e QML apenas. Sem Node/npm/Bun/etc.
- Proibido `unwrap()`/`expect()` em código de produção (clippy deny já ativo no crate; escreva `match`/`?`/`is_some_and`).
- Stdout do status é exatamente um objeto JSON schema-v2 + newline; logs vão para stderr.
- Falha operacional de provider é dado tipado (`ProviderResult`), nunca falha de processo.
- Token/credencial NUNCA entra em `ProviderResult`, logs, mensagens de erro ou asserts de Debug (asserts devem provar a AUSÊNCIA do token).
- Testes usam fakes; zero rede viva, zero credencial real.
- Não mutar paths vivos do Omarchy/Hyprland fora do gate final autorizado (Task 10 para e pergunta).
- Commits: Conventional Commit com subject em INGLÊS, ≤50 caracteres.
- Gate de cada checkpoint: `cargo fmt --check && cargo test && cargo clippy --all-targets -- -D warnings && git diff --check`.
- Branch de trabalho: `claude-ajustes`.
- Read cada arquivo antes de Edit; se Edit falhar com "string not found", re-Read antes de re-tentar.

## File Structure

- `src/providers/adapters.rs` — adapters concretos; Claude em ~330-470, testes em ~506-919. Tasks 1, 5, 7.
- `src/providers/v2_map.rs` — parsers puros; Claude em ~374-598, testes em ~616-794. Tasks 2, 3, 4, 8, 9.
- `src/providers/retry.rs` — NOVO: helper de retry transiente compartilhado. Task 5.
- `src/providers/mod.rs` — declara o novo módulo. Task 5.
- `src/providers/adapter.rs` — helper `unauthenticated()` (linha ~211). Task 6.
- `src/status/schema.rs` — variante `ProviderResult::Unauthenticated` (~948) e `is_temporary_failure` (~449). Task 6.
- `src/status/collect.rs` — mapeamento `ProviderResult` → `ProviderStatus` (~48-66). Task 6.
- `src/status/coordinator.rs` — testes de retenção stale (~527-558). Task 6 (só teste novo).

---

### Task 1: Header `Bearer` no Claude

O adapter Claude envia o token OAuth cru no header `Authorization` (`adapters.rs:382`); a API responde 401 e o provider aparece "unauthenticated" com credencial válida. O Grok já faz certo (`adapters.rs:155`: `let bearer = format!("Bearer {token}");`).

**Files:**
- Modify: `src/providers/adapters.rs` (Claude collect ~linha 380-384; teste `claude_http_exact_url_and_redacts_auth_from_domain` ~linha 634)

**Interfaces:**
- Consumes: `ScriptedHttpClient.last_headers` (já existe, `providers/http.rs:80`).
- Produces: nada novo — corrige comportamento.

- [ ] **Step 1: Escrever o assert que falha**

No teste `claude_http_exact_url_and_redacts_auth_from_domain` (em `src/providers/adapters.rs`, módulo `tests`), logo após o assert de `http.last_url`, adicionar (espelhando o padrão do teste `grok_ready_from_billing_http`, linhas ~782-794):

```rust
        let headers = http.last_headers.lock().unwrap().clone();
        assert!(
            headers.iter().any(|(k, v)| {
                k == "Authorization"
                    && v.starts_with("Bearer ")
                    && v.contains("SECRET_TOKEN_VALUE")
            }),
            "Authorization Bearer header missing: got keys {:?}",
            headers.iter().map(|(k, _)| k.as_str()).collect::<Vec<_>>()
        );
        assert!(
            headers
                .iter()
                .any(|(k, v)| k == "anthropic-beta" && v == "oauth-2025-04-20"),
            "anthropic-beta header missing"
        );
```

Atenção: o segundo argumento do assert imprime só as CHAVES dos headers, nunca os valores — o token de teste não deve vazar na saída de falha.

- [ ] **Step 2: Rodar e confirmar a falha**

Run: `cargo test claude_http_exact_url_and_redacts_auth_from_domain`
Expected: FAIL com "Authorization Bearer header missing" (o header atual é o token cru, sem prefixo).

- [ ] **Step 3: Corrigir o header de produção**

Em `src/providers/adapters.rs`, no `impl ProviderAdapter for ClaudeAdapter`, trocar:

```rust
            // Never log the token. Pass only as Authorization header value.
            let headers = [
                ("Authorization", token.as_str()),
                ("anthropic-beta", "oauth-2025-04-20"),
            ];
```

por:

```rust
            // Never log the token. Pass only as Authorization header value.
            let bearer = format!("Bearer {token}");
            let headers = [
                ("Authorization", bearer.as_str()),
                ("anthropic-beta", "oauth-2025-04-20"),
            ];
```

- [ ] **Step 4: Rodar e confirmar que passa**

Run: `cargo test claude_http`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/providers/adapters.rs
git commit -m 'fix: send Bearer prefix on Claude usage auth'
```

---

### Task 2: Bucket `seven_day_oauth_apps`

A API OAuth retorna a janela semanal no bucket `seven_day_oauth_apps` para tokens OAuth (o widget nativo do Quattro o trata como fonte primária, com fallback para `seven_day`). Nossa struct `ClaudeUsageDoc` não declara o campo, então o serde o descarta em silêncio — semanal vazio mesmo com auth correto.

**Files:**
- Modify: `src/providers/v2_map.rs` (struct `ClaudeUsageDoc` ~378; `claude_from_usage_json` ~492 e ~503; testes no módulo `tests`)

**Interfaces:**
- Consumes: `ClaudeWindowRaw` (já existe, `v2_map.rs:399`).
- Produces: campo `seven_day_oauth_apps: Option<ClaudeWindowRaw>` em `ClaudeUsageDoc`.

- [ ] **Step 1: Escrever os testes que falham**

No módulo `tests` de `src/providers/v2_map.rs`:

```rust
    #[test]
    fn claude_reads_seven_day_oauth_apps_bucket() {
        let body = br#"{"five_hour":{"utilization":10.0,"resets_at":"2026-07-28T20:00:00Z"},"seven_day_oauth_apps":{"utilization":37.0,"resets_at":"2026-08-01T00:00:00Z"}}"#;
        let result =
            claude_from_usage_json(body, datetime!(2026-07-28 18:00:00 UTC), None, None, true);
        match result {
            ProviderResult::Ready { windows, .. } => {
                assert_eq!(windows.len(), 2);
                assert_eq!(windows[1].id(), "weekly");
                assert!((windows[1].used_percent() - 37.0).abs() < 0.01);
                assert!(windows[1].resets_at().is_some());
            }
            other => panic!("expected ready, got {other:?}"),
        }
    }

    #[test]
    fn claude_prefers_oauth_apps_bucket_over_seven_day() {
        let body = br#"{"seven_day_oauth_apps":{"utilization":30.0},"seven_day":{"utilization":60.0}}"#;
        let result =
            claude_from_usage_json(body, datetime!(2026-07-28 18:00:00 UTC), None, None, true);
        match result {
            ProviderResult::Ready { windows, .. } => {
                assert_eq!(windows.len(), 1);
                assert_eq!(windows[0].id(), "weekly");
                assert!((windows[0].used_percent() - 30.0).abs() < 0.01);
            }
            other => panic!("expected ready, got {other:?}"),
        }
    }
```

- [ ] **Step 2: Rodar e confirmar as falhas**

Run: `cargo test claude_reads_seven_day_oauth_apps claude_prefers_oauth_apps`
Expected: FAIL — no primeiro, `windows.len()` é 1 (bucket descartado); no segundo, `used_percent` é 60 (usa `seven_day`).

- [ ] **Step 3: Implementar**

Em `ClaudeUsageDoc`, adicionar o campo após `seven_day`:

```rust
    #[serde(default)]
    seven_day_oauth_apps: Option<ClaudeWindowRaw>,
```

Em `claude_from_usage_json`, trocar o bloco da janela semanal:

```rust
    if let Some(w) = doc.seven_day.as_ref() {
        if let Some(window) = claude_window("weekly", "Weekly", w) {
            windows.push(window);
        }
    }
```

por:

```rust
    // OAuth tokens report the weekly bucket as seven_day_oauth_apps; prefer it.
    if let Some(w) = doc.seven_day_oauth_apps.as_ref().or(doc.seven_day.as_ref()) {
        if let Some(window) = claude_window("weekly", "Weekly", w) {
            windows.push(window);
        }
    }
```

E no loop de `limits[]`, estender o skip de kinds dedicados:

```rust
        if kind == "five_hour" || kind == "seven_day" || kind == "seven_day_oauth_apps" {
            continue; // handled via dedicated fields when present
        }
```

- [ ] **Step 4: Rodar e confirmar que passa**

Run: `cargo test claude_ -p agent-bar` (ou `cargo test claude_` se single-crate)
Expected: PASS em todos os testes com prefixo `claude_`.

- [ ] **Step 5: Commit**

```bash
git add src/providers/v2_map.rs
git commit -m 'feat: read Claude seven_day_oauth_apps bucket'
```

---

### Task 3: Dedup de IDs de janela

`weekly_model_id(model_id, idx)` no caminho de `limits[]` e `weekly_model_id(suffix, 0)` no caminho legado (`seven_day_opus`/`seven_day_sonnet`) produzem o MESMO id (ex.: `weekly-model:opus`) quando `ordinal <= 1`. Payload com ambos → duas janelas com id repetido → `ensure_unique_window_ids` (`status/schema.rs:996`) rejeita → o coordinator rebaixa a linha INTEIRA do Claude para `provider_error`, apagando 5h/semanal válidos. A validação downstream está certa; o fix é a montante: nunca inserir id duplicado.

**Files:**
- Modify: `src/providers/v2_map.rs` (`claude_from_usage_json`; novo helper `push_window_unique`; testes)

**Interfaces:**
- Consumes: `UsageWindow::id()` (getter público, `status/schema.rs:300`).
- Produces: `fn push_window_unique(windows: &mut Vec<UsageWindow>, window: UsageWindow)` (privado de `v2_map.rs`).

- [ ] **Step 1: Escrever o teste que falha**

```rust
    #[test]
    fn claude_dedupes_model_window_ids_across_sources() {
        // limits[] entry AND legacy seven_day_opus for the same model must not
        // produce duplicate window ids (schema rejects the whole row otherwise).
        let body = br#"{
            "five_hour": {"utilization": 10.0},
            "limits": [{"kind": "seven_day_model", "utilization": 20.0,
                        "scope": {"model": {"id": "opus", "display_name": "Opus"}}}],
            "seven_day_opus": {"utilization": 55.0}
        }"#;
        let result =
            claude_from_usage_json(body, datetime!(2026-07-28 18:00:00 UTC), None, None, true);
        // The whole row must survive schema validation downstream (duplicate
        // window ids make ensure_unique_window_ids reject the entire status).
        let status = crate::status::collect::provider_status_from_result(result.clone());
        assert!(status.is_ok(), "row failed schema validation: {status:?}");
        match result {
            ProviderResult::Ready { windows, .. } => {
                let opus: Vec<_> = windows
                    .iter()
                    .filter(|w| w.id() == "weekly-model:opus")
                    .collect();
                assert_eq!(opus.len(), 1, "duplicate weekly-model:opus windows");
                // limits[] (dynamic) wins over the legacy field.
                assert!((opus[0].used_percent() - 20.0).abs() < 0.01);
            }
            other => panic!("expected ready, got {other:?}"),
        }
    }
```

- [ ] **Step 2: Rodar e confirmar a falha**

Run: `cargo test claude_dedupes_model_window_ids`
Expected: FAIL — hoje nascem 2 janelas `weekly-model:opus`.

- [ ] **Step 3: Implementar o helper e aplicar**

Adicionar em `v2_map.rs` (perto de `claude_window`):

```rust
/// Insert keeping window ids unique; first occurrence wins (dynamic limits[]
/// entries are pushed before legacy seven_day_* fields on purpose).
fn push_window_unique(windows: &mut Vec<UsageWindow>, window: UsageWindow) {
    if windows.iter().any(|w| w.id() == window.id()) {
        return;
    }
    windows.push(window);
}
```

Em `claude_from_usage_json`, trocar TODOS os `windows.push(window);` por `push_window_unique(&mut windows, window);` (4 sites: five_hour, weekly, loop de limits[], loop legado opus/sonnet).

- [ ] **Step 4: Rodar e confirmar que passa**

Run: `cargo test claude_`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/providers/v2_map.rs
git commit -m 'fix: dedupe Claude usage window ids'
```

---

### Task 4: `resets_at` tolerante (RFC3339 + epoch)

`claude_window` só aceita RFC3339 estrito; o endpoint real também emite epoch (o widget nativo trata epoch explicitamente, e nosso próprio Codex já parseia `resets_at` como `i64`). Timestamp não-ISO hoje derruba silenciosamente só o reset, deixando percentual sem horário.

**Files:**
- Modify: `src/providers/v2_map.rs` (novo helper `parse_reset_timestamp`; `claude_window`; testes)

**Interfaces:**
- Consumes: `time::OffsetDateTime`, `Rfc3339`, `UtcOffset` (já importados no arquivo).
- Produces: `fn parse_reset_timestamp(raw: &str) -> Option<OffsetDateTime>` (privado; Fases 3/4 poderão promovê-lo).

- [ ] **Step 1: Escrever os testes que falham**

```rust
    #[test]
    fn parse_reset_timestamp_accepts_iso_and_epoch() {
        let iso = parse_reset_timestamp("2026-08-01T00:00:00Z");
        assert!(iso.is_some());
        // Epoch seconds (10 digits).
        let secs = parse_reset_timestamp("1785272823");
        assert_eq!(secs.map(|t| t.unix_timestamp()), Some(1_785_272_823));
        // Epoch milliseconds (13 digits).
        let millis = parse_reset_timestamp("1785272823412");
        assert_eq!(millis.map(|t| t.unix_timestamp()), Some(1_785_272_823));
        // Garbage and non-positive are rejected.
        assert!(parse_reset_timestamp("soon").is_none());
        assert!(parse_reset_timestamp("0").is_none());
        assert!(parse_reset_timestamp("-5").is_none());
        assert!(parse_reset_timestamp("").is_none());
    }

    #[test]
    fn claude_window_accepts_epoch_resets_at() {
        let body = br#"{"five_hour":{"utilization":42.0,"resets_at":"1785272823"}}"#;
        let result =
            claude_from_usage_json(body, datetime!(2026-07-28 18:00:00 UTC), None, None, true);
        match result {
            ProviderResult::Ready { windows, .. } => {
                assert_eq!(windows.len(), 1);
                assert!(windows[0].resets_at().is_some(), "epoch resets_at was dropped");
            }
            other => panic!("expected ready, got {other:?}"),
        }
    }
```

- [ ] **Step 2: Rodar e confirmar as falhas**

Run: `cargo test parse_reset_timestamp claude_window_accepts_epoch`
Expected: FAIL — `parse_reset_timestamp` não existe (erro de compilação). Isso conta como falha vermelha de TDD para função nova.

- [ ] **Step 3: Implementar**

Adicionar em `v2_map.rs`:

```rust
/// Parse a reset timestamp that may be RFC3339, epoch seconds, or epoch millis.
///
/// The documented contract is RFC3339-only, but the live OAuth endpoint and
/// sibling providers also emit epochs; be liberal on input, strict on output.
fn parse_reset_timestamp(raw: &str) -> Option<OffsetDateTime> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Ok(ts) = OffsetDateTime::parse(trimmed, &Rfc3339) {
        return Some(ts.to_offset(UtcOffset::UTC));
    }
    let numeric: i128 = trimmed.parse().ok()?;
    if numeric <= 0 {
        return None;
    }
    // < 1e12 → seconds; otherwise milliseconds (mirrors the reference widget).
    let nanos = if numeric < 1_000_000_000_000 {
        numeric.checked_mul(1_000_000_000)?
    } else {
        numeric.checked_mul(1_000_000)?
    };
    OffsetDateTime::from_unix_timestamp_nanos(nanos)
        .ok()
        .map(|ts| ts.to_offset(UtcOffset::UTC))
}
```

Em `claude_window`, trocar:

```rust
    let resets = raw
        .resets_at
        .as_deref()
        .and_then(|s| OffsetDateTime::parse(s, &Rfc3339).ok())
        .map(|ts| ts.to_offset(UtcOffset::UTC));
```

por:

```rust
    let resets = raw.resets_at.as_deref().and_then(parse_reset_timestamp);
```

- [ ] **Step 4: Rodar e confirmar que passa**

Run: `cargo test parse_reset_timestamp claude_`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/providers/v2_map.rs
git commit -m 'fix: accept epoch resets_at in Claude usage'
```

---

### Task 5: Retry transiente compartilhado

O catálogo declara `RetryPolicy::OneTransient` para os 4 providers e o spec promete "one network/timeout retry", mas o loop de retry só existe manuscrito dentro do `GrokAdapter` (`adapters.rs:164-235`). Claude é single-shot. Extrair o loop para um helper e usar em Claude e Grok (Amp/Codex adotam nas Fases 3/4).

**Files:**
- Create: `src/providers/retry.rs`
- Modify: `src/providers/mod.rs` (declarar `mod retry;`)
- Modify: `src/providers/adapters.rs` (Claude: usar helper; Grok: substituir loop inline)

**Interfaces:**
- Consumes: `HttpClient`, `HttpError`, `HttpResponse` (`providers/adapter.rs`), `ProviderDescriptor::retry_delay()` (`catalog.rs:61`, retorna `Some(250ms)` para `OneTransient`).
- Produces: `pub(crate) async fn http_get_with_retry(http: &dyn HttpClient, descriptor: &ProviderDescriptor, url: &str, headers: &[(&str, &str)], max_body_bytes: usize) -> Result<HttpResponse, HttpError>` em `src/providers/retry.rs`.

- [ ] **Step 1: Criar o módulo com os testes que falham**

Criar `src/providers/retry.rs`:

```rust
//! Shared one-transient-retry loop for HTTP collection.
//!
//! The catalog declares [`RetryPolicy::OneTransient`] for every provider; this
//! is the single implementation of that promise. Only network errors retry.

use super::adapter::{HttpClient, HttpError, HttpResponse};
use super::catalog::ProviderDescriptor;

/// GET with at most one extra attempt after a transient network error,
/// honoring the descriptor's retry policy and delay.
pub(crate) async fn http_get_with_retry(
    http: &dyn HttpClient,
    descriptor: &ProviderDescriptor,
    url: &str,
    headers: &[(&str, &str)],
    max_body_bytes: usize,
) -> Result<HttpResponse, HttpError> {
    match http.get(url, headers, max_body_bytes).await {
        Err(HttpError::Network(first)) => {
            let Some(delay) = descriptor.retry_delay() else {
                return Err(HttpError::Network(first));
            };
            tokio::time::sleep(delay).await;
            http.get(url, headers, max_body_bytes).await
        }
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::catalog::CLAUDE;
    use crate::providers::http::ScriptedHttpClient;

    fn ok_response() -> HttpResponse {
        HttpResponse {
            status: 200,
            final_url: "https://example.invalid/".into(),
            body: b"{}".to_vec(),
        }
    }

    #[tokio::test]
    async fn retries_once_after_network_error() {
        // ScriptedHttpClient pops from the END: last entry is served first.
        let http = ScriptedHttpClient {
            responses: std::sync::Mutex::new(vec![
                Ok(ok_response()),
                Err(HttpError::Network("transient".into())),
            ]),
            last_url: std::sync::Mutex::new(None),
            last_headers: std::sync::Mutex::new(Vec::new()),
        };
        let result = http_get_with_retry(&http, &CLAUDE, "https://x/", &[], 1024).await;
        assert!(result.is_ok(), "expected retry to succeed: {result:?}");
        assert!(
            http.responses.lock().unwrap().is_empty(),
            "both scripted responses must be consumed (two attempts)"
        );
    }

    #[tokio::test]
    async fn non_network_errors_do_not_retry() {
        let http = ScriptedHttpClient {
            responses: std::sync::Mutex::new(vec![
                Ok(ok_response()),
                Err(HttpError::RedirectRefused("https://evil/".into())),
            ]),
            last_url: std::sync::Mutex::new(None),
            last_headers: std::sync::Mutex::new(Vec::new()),
        };
        let result = http_get_with_retry(&http, &CLAUDE, "https://x/", &[], 1024).await;
        assert!(matches!(result, Err(HttpError::RedirectRefused(_))));
        assert_eq!(
            http.responses.lock().unwrap().len(),
            1,
            "second scripted response must remain unconsumed (single attempt)"
        );
    }

    #[tokio::test]
    async fn second_network_error_is_returned() {
        let http = ScriptedHttpClient {
            responses: std::sync::Mutex::new(vec![
                Err(HttpError::Network("second".into())),
                Err(HttpError::Network("first".into())),
            ]),
            last_url: std::sync::Mutex::new(None),
            last_headers: std::sync::Mutex::new(Vec::new()),
        };
        let result = http_get_with_retry(&http, &CLAUDE, "https://x/", &[], 1024).await;
        assert!(matches!(result, Err(HttpError::Network(_))));
    }
}
```

Em `src/providers/mod.rs`, adicionar `mod retry;` junto às outras declarações de módulo (ler o arquivo primeiro para seguir a ordem existente).

- [ ] **Step 2: Rodar e confirmar que compilam e passam**

Run: `cargo test retry`
Expected: PASS (helper novo com testes próprios — o vermelho de TDD aqui é o passo 3, onde os adapters ainda não usam o helper).

- [ ] **Step 3: Escrever o teste de adoção no Claude que falha**

No módulo `tests` de `src/providers/adapters.rs`:

```rust
    #[tokio::test]
    async fn claude_retries_once_on_network_error() {
        let body = br#"{"five_hour":{"utilization":10.0,"resets_at":"2026-07-28T22:00:00Z"}}"#;
        let http = ScriptedHttpClient {
            responses: Mutex::new(vec![
                Ok(HttpResponse {
                    status: 200,
                    final_url: CLAUDE_USAGE_URL.into(),
                    body: body.to_vec(),
                }),
                Err(crate::providers::adapter::HttpError::Network("blip".into())),
            ]),
            last_url: Mutex::new(None),
            last_headers: Mutex::new(Vec::new()),
        };
        let process = empty_process();
        let mut fs = MapFileSystem::default();
        fs.files.insert(
            std::path::PathBuf::from("/home/u/.claude/.credentials.json"),
            br#"{"claudeAiOauth":{"accessToken":"tok"}}"#.to_vec(),
        );
        let env = ExecutionEnvironment {
            home: std::path::PathBuf::from("/home/u"),
            path_dirs: vec![],
            grok_home: None,
        };
        let clock = FixedClock(datetime!(2026-07-28 18:00:00 UTC));
        let ctx = CollectionContext {
            env: &env,
            clock: &clock,
            fs: &fs,
            process: &process,
            http: &http,
            plugin_root: None,
        };
        let discovery = discovery_with_exe(Path::new("/usr/bin/claude"));
        let result = CLAUDE_ADAPTER.collect(&ctx, &discovery).await;
        assert!(
            matches!(result, ProviderResult::Ready { .. }),
            "one transient network error must be retried: {result:?}"
        );
    }
```

- [ ] **Step 4: Rodar e confirmar a falha**

Run: `cargo test claude_retries_once_on_network_error`
Expected: FAIL — hoje o Claude retorna `NetworkError` na primeira falha, sem retry.

- [ ] **Step 5: Adotar o helper no Claude e no Grok**

No `ClaudeAdapter::collect`, trocar:

```rust
            match context
                .http
                .get(CLAUDE_USAGE_URL, &headers, CLAUDE.max_output_bytes)
                .await
```

por:

```rust
            match super::retry::http_get_with_retry(
                context.http,
                &CLAUDE,
                CLAUDE_USAGE_URL,
                &headers,
                CLAUDE.max_output_bytes,
            )
            .await
```

No `GrokAdapter::collect`, remover o loop manuscrito (`let mut attempts: u8 = 0; loop { ... }`) e o braço `Err(HttpError::Network)` com sleep interno, substituindo por uma única chamada + match plano (mesmos braços de status, sem `continue`):

```rust
            match super::retry::http_get_with_retry(
                context.http,
                &GROK,
                GROK_BILLING_URL,
                &headers,
                max_body,
            )
            .await
            {
                Ok(resp) if resp.status == 401 || resp.status == 403 => unauthenticated(
                    ProviderId::Grok,
                    GROK.display_name,
                    "Grok authentication was rejected.",
                    login,
                    GROK.installation_url,
                ),
                Ok(resp) if (200..300).contains(&resp.status) => {
                    let _ = resp.final_url;
                    grok_from_billing_json(&resp.body, account, now, login)
                }
                Ok(resp) if resp.status >= 500 => ProviderResult::ProviderError {
                    id: ProviderId::Grok,
                    name: GROK.display_name.to_owned(),
                    message: "Grok billing request failed.".into(),
                    retryable: true,
                },
                Ok(_) => ProviderResult::ProviderError {
                    id: ProviderId::Grok,
                    name: GROK.display_name.to_owned(),
                    message: "Grok billing request failed.".into(),
                    retryable: false,
                },
                Err(super::adapter::HttpError::Network(_)) => ProviderResult::NetworkError {
                    id: ProviderId::Grok,
                    name: GROK.display_name.to_owned(),
                    message: "Network error while contacting Grok.".into(),
                },
                Err(super::adapter::HttpError::RedirectRefused(_)) => {
                    ProviderResult::ProviderError {
                        id: ProviderId::Grok,
                        name: GROK.display_name.to_owned(),
                        message: "Grok billing redirect refused.".into(),
                        retryable: false,
                    }
                }
                Err(super::adapter::HttpError::BodyTooLarge) => ProviderResult::ProviderError {
                    id: ProviderId::Grok,
                    name: GROK.display_name.to_owned(),
                    message: "Grok billing response exceeded size limit.".into(),
                    retryable: false,
                },
                Err(super::adapter::HttpError::InvalidResponse(_)) => {
                    ProviderResult::ProviderError {
                        id: ProviderId::Grok,
                        name: GROK.display_name.to_owned(),
                        message: "Invalid Grok billing response.".into(),
                        retryable: false,
                    }
                }
            }
```

Atenção: os `return` do loop antigo viram expressões do match (sem `return`), pois o match agora é a expressão final do bloco async. Ajuste conforme o corpo real após ler o arquivo.

- [ ] **Step 6: Rodar a suíte inteira e confirmar**

Run: `cargo test`
Expected: PASS — incluindo `claude_retries_once_on_network_error` e os testes existentes do Grok (`grok_ready_from_billing_http`, `grok_billing_timeout_network_error` — este continua vendo `NetworkError` porque o client roteirizado exaure e o segundo attempt também falha).

- [ ] **Step 7: Commit**

```bash
git add src/providers/retry.rs src/providers/mod.rs src/providers/adapters.rs
git commit -m 'refactor: share one-transient HTTP retry helper'
```

---

### Task 6: `retryable` em Unauthenticated + retenção stale

Token expirado é transitório (o Claude Code renova sozinho na próxima abertura) — mas hoje qualquer `Unauthenticated` apaga as janelas e NÃO retém cache (invariante correto para token revogado, errado para expirado). Distinguir via `retryable` no erro: expirado → `retryable: true` → elegível a retenção stale; rejeitado → `retryable: false` → comportamento atual intacto.

**Files:**
- Modify: `src/status/schema.rs` (variante `ProviderResult::Unauthenticated` ~948; `is_temporary_failure` ~449)
- Modify: `src/providers/adapter.rs` (helper `unauthenticated()` ~211)
- Modify: `src/status/collect.rs` (mapeamento ~48-66)
- Modify: `src/providers/v2_map.rs` (2 construções literais: ~228 e ~468)
- Modify: `src/providers/adapters.rs` (7 chamadas do helper — o compilador lista todas)
- Modify: `src/status/coordinator.rs` (teste novo de retenção)

**Interfaces:**
- Consumes: `apply_stale_retention` (`coordinator.rs:245`) e `retain_as_stale` (`schema.rs:427`) — nenhuma mudança nelas.
- Produces: `ProviderResult::Unauthenticated` ganha campo `retryable: bool`; helper vira `unauthenticated(id, name, message, login_available, url, retryable)`.

- [ ] **Step 1: Escrever o teste de retenção que falha (não compila ainda)**

No módulo `tests` de `src/status/coordinator.rs`, ao lado de `auth_failure_does_not_retain_stale_usage`:

```rust
    #[tokio::test]
    async fn retryable_auth_failure_retains_prior_ready_as_stale() {
        // Expired-token style failure (retryable) keeps last good windows.
        let now = datetime!(2026-07-28 18:42:00 UTC);
        let prior = ready_claude(now);
        let live = ProviderStatus::unauthenticated(
            ProviderId::Claude,
            "Claude",
            ProviderError::new(ErrorCode::AuthenticationRequired, "expired", true),
            ProviderAction::login("Log in"),
        )
        .unwrap();
        let out = apply_stale_retention(live, Some(&prior)).unwrap();
        assert_eq!(out.state(), ProviderState::Stale);
        assert_eq!(out.windows().len(), 1, "prior windows must be retained");
        assert!(out.error().is_some_and(|e| e.retryable));
    }
```

- [ ] **Step 2: Rodar e confirmar a falha**

Run: `cargo test retryable_auth_failure_retains`
Expected: FAIL — `out.state()` é `Unauthenticated` (retenção não se aplica a auth hoje).

- [ ] **Step 3: Estender `is_temporary_failure`**

Em `src/status/schema.rs`, trocar:

```rust
    /// Temporary failures eligible for stale retention (not auth / missing CLI).
    pub fn is_temporary_failure(&self) -> bool {
        match self.state {
            ProviderState::NetworkError | ProviderState::RateLimited => true,
            ProviderState::ProviderError => self.error.as_ref().is_some_and(|e| e.retryable),
            _ => false,
        }
    }
```

por:

```rust
    /// Temporary failures eligible for stale retention. Rejected auth and
    /// missing CLI never retain; an expired session (retryable auth) does.
    pub fn is_temporary_failure(&self) -> bool {
        match self.state {
            ProviderState::NetworkError | ProviderState::RateLimited => true,
            ProviderState::ProviderError | ProviderState::Unauthenticated => {
                self.error.as_ref().is_some_and(|e| e.retryable)
            }
            _ => false,
        }
    }
```

- [ ] **Step 4: Rodar os dois testes de retenção**

Run: `cargo test retryable_auth_failure_retains auth_failure_does_not_retain`
Expected: PASS nos dois — o teste antigo usa `retryable: false` e continua não retendo.

- [ ] **Step 5: Adicionar `retryable` à variante e propagar**

Em `src/status/schema.rs`, na variante:

```rust
    Unauthenticated {
        id: ProviderId,
        name: String,
        message: String,
        login_available: bool,
        installation_url: String,
        retryable: bool,
    },
```

Em `src/providers/adapter.rs`, helper:

```rust
pub(crate) fn unauthenticated(
    id: ProviderId,
    name: &str,
    message: impl Into<String>,
    login_available: bool,
    url: &str,
    retryable: bool,
) -> ProviderResult {
    ProviderResult::Unauthenticated {
        id,
        name: name.to_owned(),
        message: message.into(),
        login_available,
        installation_url: url.to_owned(),
        retryable,
    }
}
```

Em `src/status/collect.rs`, no braço `Unauthenticated`, destructure o campo novo e use-o:

```rust
        ProviderResult::Unauthenticated {
            id,
            name,
            message,
            login_available,
            installation_url,
            retryable,
        } => {
            let action = if login_available {
                ProviderAction::login("Log in")
            } else {
                ProviderAction::view_installation("View installation", installation_url)?
            };
            ProviderStatus::unauthenticated(
                id,
                name,
                ProviderError::new(ErrorCode::AuthenticationRequired, message, retryable),
                action,
            )
        }
```

Compilar (`cargo build`) e corrigir TODOS os erros de campo/aridade que o compilador listar, passando `retryable: false` (ou `false` como 6º argumento do helper) em cada site existente — `adapters.rs` (Amp 1×, Grok 3×, Claude 3×) e `v2_map.rs` (2 construções literais). NENHUM site existente muda de comportamento nesta task.

- [ ] **Step 6: Rodar a suíte inteira**

Run: `cargo test`
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add src/status/schema.rs src/status/collect.rs src/providers/adapter.rs src/providers/adapters.rs src/providers/v2_map.rs src/status/coordinator.rs
git commit -m 'feat: retryable unauthenticated retains stale'
```

---

### Task 7: Expiry client-side no Claude

O adapter lê `accessToken` mas ignora `expiresAt`. Com token vencido (Claude Code fechado há horas), dispara um HTTP condenado e mostra o genérico "authentication was rejected". Fix: checar expiry antes do HTTP; vencido → `Unauthenticated` com mensagem própria e `retryable: true` (Task 6 garante retenção do último dado como Stale quando houver cache).

**Files:**
- Modify: `src/providers/adapters.rs` (`parse_claude_credentials` ~451 e `ClaudeAdapter::collect`; testes)

**Interfaces:**
- Consumes: helper `unauthenticated(..., retryable)` da Task 6; `context.clock.now_utc()`.
- Produces: `struct ClaudeCredentials { token: String, plan: Option<Plan>, account: Option<Account>, expires_at_ms: Option<i64> }` (privada de `adapters.rs`); `parse_claude_credentials(bytes) -> Option<ClaudeCredentials>`. A Task 8 consome esta struct.

- [ ] **Step 1: Escrever o teste que falha**

No módulo `tests` de `src/providers/adapters.rs`:

```rust
    #[tokio::test]
    async fn claude_expired_token_skips_http_and_is_retryable() {
        let http = ScriptedHttpClient::default(); // any HTTP call would error
        let process = empty_process();
        let mut fs = MapFileSystem::default();
        fs.files.insert(
            std::path::PathBuf::from("/home/u/.claude/.credentials.json"),
            // expiresAt in the past relative to the fixed clock below.
            br#"{"claudeAiOauth":{"accessToken":"tok","expiresAt":1690000000000}}"#.to_vec(),
        );
        let env = ExecutionEnvironment {
            home: std::path::PathBuf::from("/home/u"),
            path_dirs: vec![],
            grok_home: None,
        };
        let clock = FixedClock(datetime!(2026-07-28 18:00:00 UTC));
        let ctx = CollectionContext {
            env: &env,
            clock: &clock,
            fs: &fs,
            process: &process,
            http: &http,
            plugin_root: None,
        };
        let discovery = discovery_with_exe(Path::new("/usr/bin/claude"));
        let result = CLAUDE_ADAPTER.collect(&ctx, &discovery).await;
        assert!(
            http.last_url.lock().unwrap().is_none(),
            "expired token must not trigger an HTTP request"
        );
        match result {
            ProviderResult::Unauthenticated {
                message, retryable, ..
            } => {
                assert!(message.contains("expired"), "message: {message}");
                assert!(retryable, "expired session must be retryable");
            }
            other => panic!("expected unauthenticated, got {other:?}"),
        }
    }
```

- [ ] **Step 2: Rodar e confirmar a falha**

Run: `cargo test claude_expired_token_skips_http`
Expected: FAIL — hoje o HTTP dispara (client roteirizado vazio → `NetworkError`).

- [ ] **Step 3: Implementar**

Em `src/providers/adapters.rs`, substituir `parse_claude_credentials` (retorno em tupla) por:

```rust
struct ClaudeCredentials {
    token: String,
    plan: Option<Plan>,
    account: Option<Account>,
    expires_at_ms: Option<i64>,
}

fn parse_claude_credentials(bytes: &[u8]) -> Option<ClaudeCredentials> {
    let value: serde_json::Value = serde_json::from_slice(bytes).ok()?;
    let oauth = value.get("claudeAiOauth")?;
    let token = oauth.get("accessToken")?.as_str()?.to_owned();
    if token.is_empty() {
        return None;
    }
    let plan = oauth
        .get("subscriptionType")
        .and_then(|v| v.as_str())
        .map(|id| Plan {
            id: id.to_owned(),
            label: id.to_owned(),
        });
    let expires_at_ms = oauth.get("expiresAt").and_then(|v| v.as_i64());
    Some(ClaudeCredentials {
        token,
        plan,
        account: None,
        expires_at_ms,
    })
}
```

No `ClaudeAdapter::collect`, trocar o destructure em tupla pelo uso da struct e inserir a checagem ANTES da montagem dos headers:

```rust
            let creds = match parse_claude_credentials(&cred_bytes) {
                Some(v) => v,
                None => {
                    return unauthenticated(
                        ProviderId::Claude,
                        CLAUDE.display_name,
                        "Claude is not authenticated.",
                        login_available(discovery),
                        CLAUDE.installation_url,
                        false,
                    );
                }
            };

            // An expired session self-heals when Claude Code refreshes the
            // token; report it as retryable so prior data is retained as stale.
            let now_ms = context.clock.now_utc().unix_timestamp().saturating_mul(1000);
            if creds.expires_at_ms.is_some_and(|exp| exp <= now_ms) {
                return unauthenticated(
                    ProviderId::Claude,
                    CLAUDE.display_name,
                    "Claude session expired. Open Claude Code to refresh it.",
                    login_available(discovery),
                    CLAUDE.installation_url,
                    true,
                );
            }
```

Referências seguintes trocam `token`/`plan`/`account` por `creds.token`/`creds.plan`/`creds.account` (o `format!("Bearer {token}")` da Task 1 vira `format!("Bearer {}", creds.token)`).

- [ ] **Step 4: Rodar e confirmar que passa**

Run: `cargo test claude_`
Expected: PASS em todos.

- [ ] **Step 5: Commit**

```bash
git add src/providers/adapters.rs
git commit -m 'feat: detect expired Claude token before HTTP'
```

---

### Task 8: Tier real (`rateLimitTier`)

O plano/badge usa só `subscriptionType` cru ("max"). O arquivo de credenciais também traz `rateLimitTier` (ex.: `max_20x`), que o widget nativo formata como "Max 20x". Preferir o tier; fallback para o subscription capitalizado.

**Files:**
- Modify: `src/providers/adapters.rs` (`parse_claude_credentials` da Task 7; novo helper `claude_plan`; testes)

**Interfaces:**
- Consumes: `ClaudeCredentials` (Task 7), `Plan { id, label }` (`status/schema.rs`).
- Produces: `fn claude_plan(subscription_type: Option<&str>, rate_limit_tier: Option<&str>) -> Option<Plan>` (privada de `adapters.rs`).

- [ ] **Step 1: Escrever o teste que falha**

```rust
    #[test]
    fn claude_plan_formats_rate_limit_tier() {
        let plan = claude_plan(Some("max"), Some("max_20x"));
        assert_eq!(plan.as_ref().map(|p| p.label.as_str()), Some("Max 20x"));
        assert_eq!(plan.as_ref().map(|p| p.id.as_str()), Some("max_20x"));

        let fallback = claude_plan(Some("pro"), None);
        assert_eq!(fallback.as_ref().map(|p| p.label.as_str()), Some("Pro"));
        assert_eq!(fallback.as_ref().map(|p| p.id.as_str()), Some("pro"));

        assert!(claude_plan(None, None).is_none());
    }
```

- [ ] **Step 2: Rodar e confirmar a falha**

Run: `cargo test claude_plan_formats`
Expected: FAIL — `claude_plan` não existe (erro de compilação; vermelho de TDD para função nova).

- [ ] **Step 3: Implementar**

Em `src/providers/adapters.rs`:

```rust
/// Prefer the granular rate-limit tier ("max_20x" → "Max 20x"); fall back to
/// the capitalized subscription type. Mirrors the native widget's formatTier.
fn claude_plan(subscription_type: Option<&str>, rate_limit_tier: Option<&str>) -> Option<Plan> {
    if let Some(tier) = rate_limit_tier.filter(|t| !t.is_empty()) {
        if let Some(suffix) = tier.strip_prefix("max_") {
            return Some(Plan {
                id: tier.to_owned(),
                label: format!("Max {suffix}"),
            });
        }
        return Some(Plan {
            id: tier.to_owned(),
            label: capitalize_ascii(tier),
        });
    }
    let sub = subscription_type.filter(|s| !s.is_empty())?;
    Some(Plan {
        id: sub.to_owned(),
        label: capitalize_ascii(sub),
    })
}

fn capitalize_ascii(raw: &str) -> String {
    let mut chars = raw.chars();
    match chars.next() {
        Some(first) => first.to_ascii_uppercase().to_string() + chars.as_str(),
        None => String::new(),
    }
}
```

Em `parse_claude_credentials`, trocar a construção de `plan` por:

```rust
    let plan = claude_plan(
        oauth.get("subscriptionType").and_then(|v| v.as_str()),
        oauth.get("rateLimitTier").and_then(|v| v.as_str()),
    );
```

- [ ] **Step 4: Rodar e confirmar que passa**

Run: `cargo test claude_`
Expected: PASS. Atenção: se algum teste existente assertar o label antigo cru (ex.: "pro"), atualize o assert para o label capitalizado — mudança intencional.

- [ ] **Step 5: Commit**

```bash
git add src/providers/adapters.rs
git commit -m 'feat: format Claude rate limit tier label'
```

---

### Task 9: Remover o guard morto de percentual

`claude_window` tem um `if/else` cujos dois ramos computam a expressão idêntica — o comentário alega proteção contra payload fracionário que não existe. Remover o código morto e fixar o comportamento correto por teste (1.0 = 1%, não 100%).

**Files:**
- Modify: `src/providers/v2_map.rs` (`claude_window` ~550-563; teste novo)

**Interfaces:**
- Consumes/Produces: nada novo — refactor interno.

- [ ] **Step 1: Escrever o teste de caracterização**

```rust
    #[test]
    fn claude_utilization_one_means_one_percent() {
        // The endpoint reports percent scale: 1.0 is 1%, never 100%.
        let body = br#"{"five_hour":{"utilization":1.0,"resets_at":"2026-07-28T22:00:00Z"}}"#;
        let result =
            claude_from_usage_json(body, datetime!(2026-07-28 18:00:00 UTC), None, None, true);
        match result {
            ProviderResult::Ready { windows, .. } => {
                assert!((windows[0].used_percent() - 1.0).abs() < 0.01);
                assert!((windows[0].remaining_percent() - 99.0).abs() < 0.01);
            }
            other => panic!("expected ready, got {other:?}"),
        }
    }
```

- [ ] **Step 2: Rodar — este teste JÁ PASSA (é rede de segurança do refactor)**

Run: `cargo test claude_utilization_one`
Expected: PASS (os dois ramos do guard morto são idênticos; o teste protege o refactor seguinte).

- [ ] **Step 3: Remover o código morto**

Em `claude_window`, trocar:

```rust
    // Guard double-division: values are already percent, not 0..=1 fractions.
    let used = if raw.utilization > 0.0 && raw.utilization <= 1.0 {
        // Ambiguous tiny values: treat as percent only when clearly percent-like
        // API always sends 0..=100; if someone passes 0.42 meaning 42%, that
        // was the historical bug — we intentionally treat <=1 as percent only
        // when the fixture marks it via >100 impossible; keep as-is clamp.
        raw.utilization.clamp(0.0, 100.0)
    } else {
        raw.utilization.clamp(0.0, 100.0)
    };
```

por:

```rust
    // Utilization is already percent scale (1.0 == 1%); clamp only.
    let used = raw.utilization.clamp(0.0, 100.0);
```

- [ ] **Step 4: Rodar e confirmar que segue passando**

Run: `cargo test claude_`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/providers/v2_map.rs
git commit -m 'refactor: drop dead percent guard in Claude map'
```

---

### Task 10: Gate completo + baseline QML + checkpoint de QA ao vivo

Fecha a fase: gate integral do repo, baseline da suíte QML (nunca executada de fato segundo a auditoria — Fase 1 não toca QML, então ela DEVE passar) e parada obrigatória antes de tocar o plugin instalado.

**Files:**
- Nenhuma modificação esperada (só correções se o gate acusar).

- [ ] **Step 1: Gate Rust completo**

Run:
```bash
cargo fmt --check && cargo test && cargo clippy --all-targets -- -D warnings && git diff --check
```
Expected: tudo verde. Qualquer falha: corrigir, re-rodar, e emendar no commit da task correspondente (`git commit --fixup` NÃO — commit normal com subject descritivo).

- [ ] **Step 2: Baseline QML (sem mudanças QML nesta fase)**

Run:
```bash
find assets/omarchy -type f -name '*.qml' -exec qmllint -I /usr/share/omarchy/shell {} +
omarchy plugin validate assets/omarchy
QT_QPA_PLATFORM=offscreen qmltestrunner -input tests/qml -import /usr/share/omarchy/shell -import assets/omarchy -o -,txt
```
Expected: PASS. Se falhar em algo pré-existente, NÃO corrigir aqui — registrar o resultado literal no relatório da fase (é insumo da Fase 2).

- [ ] **Step 3: PARAR — checkpoint com o usuário para QA ao vivo**

Reportar: resultado do gate, baseline QML e o diff total da fase (`git log --oneline master..claude-ajustes`). Pedir autorização explícita para o QA ao vivo (build do bundle + atualização do plugin instalado em `~/.config/omarchy/plugins/agent-bar.usage` + verificação na barra real com credencial real). O contrato proíbe mutar paths vivos sem esse gate autorizado. NÃO prosseguir sem resposta.

---

## Fora deste plano (registrado para as próximas fases)

- Humanização de `resets_at` na UI ("em 2h 14m") — Fase 2 (está pinado por teste QML hoje).
- Enum sync JS/Rust, split do ServiceCore.js, `error.retryable` na UI — Fase 2.
- Codex freshness/sandbox/cap, dedup de schemas — Fase 3.
- Amp classifier, Grok period type/expiry, dead code Grok — Fase 4.
- Paridade com o widget nativo + emenda de contrato — Fase 5.
