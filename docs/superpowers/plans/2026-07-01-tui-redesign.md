# TUI Redesign Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implementar o redesign da TUI do `agent-bar menu` conforme
`docs/superpowers/specs/2026-07-01-tui-redesign-design.md`: dados reais
(API `limits[]`+`spend` do Claude, bucketing horário), runtime que nunca
congela (fetch/login fora do event loop), IA nova (sidebar sem tabs +
mouse), visual One Dark evoluído com motion.

**Architecture:** Mantém o MVU existente (`state.rs`/`action.rs`/`update()`
puro + `event_loop.rs` com IO). IO pesado migra para threads dedicadas que
reportam via o canal `bg_tx` já existente. Render passa a produzir um
`HitMap` de regiões clicáveis para o mouse. Telas despacham por `Screen`
(substitui `Tab`/`Mode`).

**Tech Stack:** Rust (ratatui 0.30, crossterm 0.29), tachyonfx,
tui-scrollview, throbber-widgets-tui, insta (snapshots), wiremock.

## Global Constraints

- Rust/cargo only; sem Node/bun em runtime ou testes (CLAUDE.md §1).
- **Nunca `unwrap()`/`expect()` em código de produção** — `?`, `bail!` ou guard.
- Strings de erro de provider são contrato — não alterar nenhuma existente.
- XML-escape só em `render_pango.rs`; TUI nunca escapa.
- stdout limpo é contrato Waybar — a TUI escreve no terminal alternativo; logs via `logger` (stderr).
- `cargo test --test golden` e `cargo test waybar_contract` **não podem mudar** (contrato Waybar intacto).
- Identidade via constantes de `src/app_identity.rs`.
- Gotcha RTK: **um filtro posicional por invocação** de `cargo test` (ex.: `cargo test providers::claude` ok; dois filtros não).
- Snapshots insta: setar `XDG_CONFIG_HOME`/`XDG_CACHE_HOME` antes de qualquer import que leia `config.rs`; regravar snapshot só quando o display contract muda de propósito (aqui: muda — todos os snaps de `src/tui/render/snapshots/` serão regravados nas tasks 10-14).
- Commits: Conventional Commits em PT, subject ≤50 chars.
- Não reintroduzir legado morto (qbar etc.); não converter `scripts/agent-bar-open-terminal` pra Rust.
- Deps novas permitidas SOMENTE: `tachyonfx`, `tui-scrollview`.
- Gate por task: além do teste da task, `cargo clippy --all-targets -- -D warnings`.

**Refinamento de spec registrado:** §4.2 do spec menciona "série de custo"
do Amp no History; não existe fonte local de série (o `~/.amp` não guarda
logs de token). Implementação correta (princípio "dado real ou nada"):
linha do Amp na tabela do History com saldo/gasto atuais de `amp_dollars`
e chart sem dataset Amp, com nota "sem logs locais". O card do Amp nunca
some.

---

## Fase 1 — Dados

### Task 1: Claude — parser `limits[]` + `spend`

**Files:**
- Modify: `src/providers/claude.rs` (structs raw ~linhas 59-106; montagem do quota ~linhas 280-340; testes no fim do arquivo)
- Modify: `src/providers/types.rs` (campo `severity` em `QuotaWindow`)

**Interfaces:**
- Consumes: `ClaudeUsageResponse`, `QuotaWindow`, `ExtraUsage`, `ClaudeQuotaExtra` (existentes).
- Produces:
  - `QuotaWindow.severity: Option<String>` (novo campo, `skip_serializing_if`).
  - `fn quota_from_limits(&ClaudeUsageResponse) -> Option<(Option<QuotaWindow>, Option<QuotaWindow>, IndexMap<String, QuotaWindow>)>` (privada em claude.rs).
  - `fn extra_usage_from_spend(&ClaudeSpendRaw) -> ExtraUsage` (privada).

- [ ] **Step 1: Adicionar `severity` ao `QuotaWindow`**

Em `src/providers/types.rs`, no struct `QuotaWindow`:

```rust
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuotaWindow {
    pub remaining: f64,
    /// Sempre presente no JSON (pode ser null).
    pub resets_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window_minutes: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub used: Option<f64>,
    /// Severidade vinda da API (`limits[].severity` do Claude). `None` =
    /// calcular localmente por threshold. Omitida do JSON quando ausente
    /// (mantém golden/waybar_contract intactos).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub severity: Option<String>,
}
```

Todos os construtores de `QuotaWindow` no repo ganham `severity: None`
(buscar com `grep -rn "QuotaWindow {" src/` — claude.rs, codex.rs, amp.rs,
extras.rs, types.rs tests, render tests). Não alterar nenhum outro campo.

- [ ] **Step 2: Rodar a suite de types para confirmar que nada quebrou**

Run: `cargo test providers::types`
Expected: PASS (o campo novo é omitido quando None; nenhum teste existente muda).

- [ ] **Step 3: Escrever os testes que falham (parser limits/spend)**

No `mod tests` de `src/providers/claude.rs`, adicionar (fixture = resposta
real da API em 2026-07-01, reduzida):

```rust
const LIMITS_BODY: &str = r#"{
  "five_hour": {"utilization": 55.0, "resets_at": "2026-07-02T02:39:59Z"},
  "seven_day": {"utilization": 44.0, "resets_at": "2026-07-03T22:59:59Z"},
  "limits": [
    {"kind": "session", "group": "session", "percent": 11,
     "severity": "normal", "resets_at": "2026-07-02T02:39:59.132436+00:00",
     "scope": null, "is_active": true},
    {"kind": "weekly_all", "group": "weekly", "percent": 3,
     "severity": "normal", "resets_at": "2026-07-03T22:59:59.132457+00:00",
     "scope": null, "is_active": false},
    {"kind": "weekly_scoped", "group": "weekly", "percent": 3,
     "severity": "normal", "resets_at": "2026-07-03T22:59:59.132697+00:00",
     "scope": {"model": {"id": null, "display_name": "Fable"}, "surface": null},
     "is_active": false},
    {"kind": "algum_kind_novo", "percent": 1, "severity": "normal"}
  ],
  "spend": {
    "used": {"amount_minor": 1234, "currency": "USD", "exponent": 2},
    "limit": {"amount_minor": 10000, "currency": "USD", "exponent": 2},
    "percent": 12, "severity": "normal", "enabled": true,
    "disclaimer": "x", "can_purchase_credits": false, "can_toggle": false
  }
}"#;

#[tokio::test]
async fn claude_limits_block_takes_precedence_over_legacy() {
    let (ctx_dir, settings, client) = /* mesmo setup wiremock dos testes existentes:
        copiar o padrão do teste que mocka GET /api/oauth/usage neste arquivo,
        respondendo LIMITS_BODY com status 200 */;
    // ... montar ctx com claude_usage_url do mock e credenciais válidas ...
    let q = ClaudeProvider.get_quota(&ctx).await;
    // limits[] vence os campos legados five_hour/seven_day:
    let p = q.primary.as_ref().unwrap();
    assert_eq!(p.remaining, 89.0); // 100 - 11 (limits), NÃO 45 (legacy 55)
    assert_eq!(p.severity.as_deref(), Some("normal"));
    assert_eq!(p.used, Some(11.0));
    let s = q.secondary.as_ref().unwrap();
    assert_eq!(s.remaining, 97.0);
    // weekly_scoped vira models["Fable"]:
    let models = q.models.as_ref().unwrap();
    assert_eq!(models.get("Fable").unwrap().remaining, 97.0);
    // spend vira extra_usage habilitado com $12.34 de $100.00:
    let extra = match q.extra.as_ref().unwrap() {
        crate::providers::types::ProviderExtra::Claude(c) => c,
        _ => panic!("extra deve ser Claude"),
    };
    let eu = extra.extra_usage.as_ref().unwrap();
    assert!(eu.enabled);
    assert!((eu.used - 12.34).abs() < 1e-9);
    assert!((eu.limit - 100.0).abs() < 1e-9);
    assert!((eu.remaining - 87.66).abs() < 1e-9);
}

#[tokio::test]
async fn claude_falls_back_to_legacy_when_limits_absent() {
    // Body SÓ com five_hour/seven_day (fixture já usada nos testes atuais).
    // Asserts existentes continuam: remaining = 100 - utilization etc.
    // Novo assert: severity is None (fallback não inventa severidade).
}

#[tokio::test]
async fn claude_spend_disabled_still_maps_extra_usage_off() {
    // Body com "spend": {"used": {"amount_minor": 0, ...}, "limit": null,
    //   "percent": 0, "severity": "normal", "enabled": false}
    // e "limits" vazio → extra_usage = Some(ExtraUsage{enabled:false, used:0,
    //   limit:0, remaining:0}) para a TUI renderizar "extra usage: off".
}
```

(Os testes seguem o padrão wiremock já usado no arquivo — mesma mecânica de
mock de `path("/api/oauth/usage")` vista na linha ~420.)

- [ ] **Step 4: Rodar e ver falhar**

Run: `cargo test providers::claude`
Expected: FAIL — os 3 testes novos não compilam/falham (structs `limits`/`spend` inexistentes).

- [ ] **Step 5: Implementar structs raw + mapeamento**

Em `src/providers/claude.rs`, junto aos structs raw existentes:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ClaudeLimitScopeModelRaw {
    #[serde(default)]
    display_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ClaudeLimitScopeRaw {
    #[serde(default)]
    model: Option<ClaudeLimitScopeModelRaw>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ClaudeLimitRaw {
    kind: String,
    #[serde(default)]
    percent: Option<f64>,
    #[serde(default)]
    severity: Option<String>,
    #[serde(default)]
    resets_at: Option<String>,
    #[serde(default)]
    scope: Option<ClaudeLimitScopeRaw>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ClaudeSpendMoneyRaw {
    #[serde(default)]
    amount_minor: Option<i64>,
    #[serde(default)]
    exponent: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ClaudeSpendRaw {
    #[serde(default)]
    used: Option<ClaudeSpendMoneyRaw>,
    #[serde(default)]
    limit: Option<ClaudeSpendMoneyRaw>,
    #[serde(default)]
    enabled: Option<bool>,
}
```

No `ClaudeUsageResponse`, adicionar:

```rust
    #[serde(default)]
    limits: Vec<ClaudeLimitRaw>,
    #[serde(default)]
    spend: Option<ClaudeSpendRaw>,
```

Funções de mapeamento (mesmo módulo):

```rust
fn window_from_limit(l: &ClaudeLimitRaw) -> QuotaWindow {
    let used = l.percent.unwrap_or(0.0).round();
    QuotaWindow {
        remaining: 100.0 - used,
        resets_at: l.resets_at.clone().filter(|s| !s.is_empty()),
        window_minutes: None,
        used: Some(used),
        severity: l.severity.clone(),
    }
}

/// `limits[]` → (primary, secondary, weekly_models). `None` se a lista está
/// vazia (conta antiga) → chamador usa o caminho legado.
fn quota_from_limits(
    u: &ClaudeUsageResponse,
) -> Option<(Option<QuotaWindow>, Option<QuotaWindow>, IndexMap<String, QuotaWindow>)> {
    if u.limits.is_empty() {
        return None;
    }
    let mut primary = None;
    let mut secondary = None;
    let mut weekly = IndexMap::new();
    for l in &u.limits {
        match l.kind.as_str() {
            "session" => primary = Some(window_from_limit(l)),
            "weekly_all" => secondary = Some(window_from_limit(l)),
            "weekly_scoped" => {
                let name = l
                    .scope
                    .as_ref()
                    .and_then(|s| s.model.as_ref())
                    .and_then(|m| m.display_name.clone());
                if let Some(name) = name {
                    weekly.insert(name, window_from_limit(l));
                }
            }
            other => log::debug!("Claude limits[]: kind desconhecido ignorado: {other}"),
        }
    }
    Some((primary, secondary, weekly))
}

fn money_of(m: &Option<ClaudeSpendMoneyRaw>) -> f64 {
    m.as_ref()
        .and_then(|m| {
            m.amount_minor
                .map(|a| a as f64 / 10f64.powi(m.exponent.unwrap_or(2) as i32))
        })
        .unwrap_or(0.0)
}

fn extra_usage_from_spend(s: &ClaudeSpendRaw) -> ExtraUsage {
    let used = money_of(&s.used);
    let limit = money_of(&s.limit);
    ExtraUsage {
        enabled: s.enabled.unwrap_or(false),
        remaining: (limit - used).max(0.0),
        limit,
        used,
    }
}
```

No ponto de montagem do quota (~linha 285, onde hoje se faz
`let primary = usage.five_hour.as_ref().map(window_from);`):

```rust
let (primary, secondary, weekly) = match quota_from_limits(&usage) {
    Some(t) => t,
    None => {
        // Caminho legado (código atual movido pra cá, sem mudança de lógica):
        let primary = usage.five_hour.as_ref().map(window_from);
        let secondary = usage.seven_day.as_ref().map(window_from);
        let mut weekly = IndexMap::new();
        if let Some(w) = usage.seven_day_opus.as_ref() {
            weekly.insert("Opus".to_string(), window_from(w));
        }
        if let Some(w) = usage.seven_day_sonnet.as_ref() {
            weekly.insert("Sonnet".to_string(), window_from(w));
        }
        if let Some(w) = usage.seven_day_cowork.as_ref() {
            weekly.insert("Cowork".to_string(), window_from(w));
        }
        (primary, secondary, weekly)
    }
};
// extra_usage: spend novo tem precedência; legado como fallback.
let extra_usage = match usage.spend.as_ref() {
    Some(s) => Some(extra_usage_from_spend(s)),
    None => /* mapeamento legado existente de usage.extra_usage, inalterado */,
};
```

- [ ] **Step 6: Rodar e ver passar**

Run: `cargo test providers::claude`
Expected: PASS (novos + todos os existentes — o corpo legado cai no fallback).

- [ ] **Step 7: Gate + commit**

Run: `cargo clippy --all-targets -- -D warnings` → sem warnings.
Run: `cargo test --test golden` → PASS inalterado.

```bash
git add src/providers/claude.rs src/providers/types.rs
git commit -m "feat(claude): parser limits[] e spend da API"
```

---

### Task 2: Bucketing horário em `usage/buckets.rs`

**Files:**
- Create: `src/usage/buckets.rs`
- Modify: `src/usage/mod.rs` (adicionar `pub mod buckets;`)
- Modify: `src/tui/render/history.rs` (mover `DayBucket`, `bucket_by_day`, `bucket_by_provider_day` para `buckets.rs`; re-importar de lá)

**Interfaces:**
- Consumes: `UsageRecord { provider, model, input, output, cache_read, cache_write, ts }`.
- Produces (em `crate::usage::buckets`):
  - `pub struct DayBucket` e fns `bucket_by_day`, `bucket_by_provider_day` (movidos, assinaturas idênticas às atuais de `render/history.rs:25-110`).
  - `pub struct HourBucket { pub hour_start: time::OffsetDateTime, pub tokens: u64 }`
  - `pub fn bucket_by_hour(records: &[UsageRecord], now: time::OffsetDateTime, hours: usize) -> Vec<HourBucket>` — sempre devolve exatamente `hours` buckets (zeros preenchidos), do mais antigo ao mais novo; tokens = `input + output + cache_read + cache_write` (mesma soma do `bucket_by_day` atual).
  - `pub fn provider_series_24h(records: &[UsageRecord], provider: &str, now: time::OffsetDateTime) -> Vec<u64>` — 24 pontos do provider.

- [ ] **Step 1: Escrever testes que falham**

Em `src/usage/buckets.rs` (arquivo novo, testes no próprio módulo):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::usage::UsageRecord;
    use time::macros::datetime;

    fn rec(provider: &str, ts: time::OffsetDateTime, tokens: u64) -> UsageRecord {
        UsageRecord {
            provider: provider.into(),
            model: Some("m".into()),
            input: tokens,
            output: 0,
            cache_read: 0,
            cache_write: 0,
            ts,
        }
    }

    #[test]
    fn bucket_by_hour_fills_gaps_with_zero() {
        let now = datetime!(2026-07-01 18:30:00 UTC);
        let records = vec![
            rec("claude", datetime!(2026-07-01 18:10:00 UTC), 100), // hora atual
            rec("claude", datetime!(2026-07-01 16:59:00 UTC), 40),  // 2h atrás
            rec("claude", datetime!(2026-06-30 18:45:00 UTC), 7),   // fora da janela de 3h
        ];
        let buckets = bucket_by_hour(&records, now, 3);
        assert_eq!(buckets.len(), 3);
        assert_eq!(buckets[0].hour_start, datetime!(2026-07-01 16:00:00 UTC));
        assert_eq!(buckets[0].tokens, 40);
        assert_eq!(buckets[1].tokens, 0); // 17h vazia
        assert_eq!(buckets[2].tokens, 100);
    }

    #[test]
    fn provider_series_24h_has_24_points_and_filters_provider() {
        let now = datetime!(2026-07-01 18:30:00 UTC);
        let records = vec![
            rec("claude", datetime!(2026-07-01 18:00:01 UTC), 5),
            rec("codex", datetime!(2026-07-01 18:00:01 UTC), 999),
        ];
        let series = provider_series_24h(&records, "claude", now);
        assert_eq!(series.len(), 24);
        assert_eq!(series[23], 5);
        assert!(series[..23].iter().all(|&t| t == 0));
    }

    #[test]
    fn bucket_by_hour_sums_all_token_kinds() {
        let now = datetime!(2026-07-01 10:30:00 UTC);
        let mut r = rec("claude", datetime!(2026-07-01 10:05:00 UTC), 1);
        r.output = 2;
        r.cache_read = 3;
        r.cache_write = 4;
        let buckets = bucket_by_hour(&[r], now, 1);
        assert_eq!(buckets[0].tokens, 10);
    }
}
```

- [ ] **Step 2: Rodar e ver falhar**

Run: `cargo test usage::buckets`
Expected: FAIL (módulo não existe).

- [ ] **Step 3: Implementar**

```rust
//! Bucketing temporal de UsageRecords (dado puro — sem render).

use time::OffsetDateTime;

use crate::usage::UsageRecord;

#[derive(Debug, Clone, PartialEq)]
pub struct HourBucket {
    pub hour_start: OffsetDateTime,
    pub tokens: u64,
}

fn floor_to_hour(t: OffsetDateTime) -> OffsetDateTime {
    t.replace_minute(0)
        .and_then(|t| t.replace_second(0))
        .and_then(|t| t.replace_nanosecond(0))
        .unwrap_or(t)
}

fn record_tokens(r: &UsageRecord) -> u64 {
    r.input + r.output + r.cache_read + r.cache_write
}

pub fn bucket_by_hour(
    records: &[UsageRecord],
    now: OffsetDateTime,
    hours: usize,
) -> Vec<HourBucket> {
    let end_hour = floor_to_hour(now);
    let mut buckets: Vec<HourBucket> = (0..hours)
        .map(|i| HourBucket {
            hour_start: end_hour - time::Duration::hours((hours - 1 - i) as i64),
            tokens: 0,
        })
        .collect();
    let start = match buckets.first() {
        Some(b) => b.hour_start,
        None => return buckets,
    };
    for r in records {
        if r.ts < start || r.ts >= end_hour + time::Duration::hours(1) {
            continue;
        }
        let idx = ((floor_to_hour(r.ts) - start).whole_hours()) as usize;
        if let Some(b) = buckets.get_mut(idx) {
            b.tokens += record_tokens(r);
        }
    }
    buckets
}

pub fn provider_series_24h(
    records: &[UsageRecord],
    provider: &str,
    now: OffsetDateTime,
) -> Vec<u64> {
    let filtered: Vec<UsageRecord> = records
        .iter()
        .filter(|r| r.provider == provider)
        .cloned()
        .collect();
    bucket_by_hour(&filtered, now, 24)
        .into_iter()
        .map(|b| b.tokens)
        .collect()
}
```

Mover `DayBucket`, `bucket_by_day` e `bucket_by_provider_day` de
`src/tui/render/history.rs:25-110` para este arquivo **sem mudar lógica**
(junto com seus testes), e em `history.rs` importar
`use crate::usage::buckets::{bucket_by_day, bucket_by_provider_day, DayBucket};`.

- [ ] **Step 4: Rodar e ver passar**

Run: `cargo test usage::buckets` → PASS.
Run: `cargo test formatters` → PASS (nada de formatter mudou).
Run: `cargo test --test golden` → PASS.

- [ ] **Step 5: Commit**

```bash
git add src/usage/buckets.rs src/usage/mod.rs src/tui/render/history.rs
git commit -m "feat(usage): bucketing horário em usage::buckets"
```

---

### Task 3: Estado de login unificado

**Files:**
- Create: `src/tui/login_state.rs`
- Modify: `src/tui/mod.rs` (declarar módulo)
- Modify: `src/tui/render/login.rs` (remover `is_logged_in` fraco; consumir `LoginState`)

**Interfaces:**
- Consumes: `ProviderQuota { available, error }`, `FetchStatus`.
- Produces:
  - `pub enum LoginState { Ok, NoToken, LoggedOut, Checking }`
  - `pub fn login_state_for(quota: Option<&ProviderQuota>, fetch_pending: bool) -> LoginState`

- [ ] **Step 1: Testes que falham**

Em `src/tui/login_state.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::types::ProviderQuota;

    fn quota(available: bool, error: Option<&str>) -> ProviderQuota {
        ProviderQuota {
            provider: "claude".into(),
            display_name: "Claude".into(),
            available,
            account: None,
            plan: None,
            plan_type: None,
            primary: None,
            secondary: None,
            models: None,
            extra: None,
            error: error.map(|s| s.to_string()),
        }
    }

    #[test]
    fn fetch_ok_is_logged_in() {
        assert_eq!(login_state_for(Some(&quota(true, None)), false), LoginState::Ok);
    }

    #[test]
    fn not_logged_in_error_is_logged_out() {
        // Mensagem padrão de não-logado é contrato (CLAUDE.md §7).
        let q = quota(false, Some("Not logged in. Open `agent-bar menu` and choose Provider login."));
        assert_eq!(login_state_for(Some(&q), false), LoginState::LoggedOut);
    }

    #[test]
    fn other_error_with_source_present_is_no_token() {
        let q = quota(true, Some("Claude API error 401"));
        assert_eq!(login_state_for(Some(&q), false), LoginState::NoToken);
    }

    #[test]
    fn pending_fetch_is_checking() {
        assert_eq!(login_state_for(None, true), LoginState::Checking);
    }

    #[test]
    fn absent_quota_without_fetch_is_logged_out() {
        assert_eq!(login_state_for(None, false), LoginState::LoggedOut);
    }
}
```

- [ ] **Step 2: Rodar e ver falhar**

Run: `cargo test tui::login_state`
Expected: FAIL (módulo não existe).

- [ ] **Step 3: Implementar**

```rust
//! Estado de login derivado do ÚLTIMO FETCH REAL — nunca de path.exists().
//! Substitui a checagem fraca que fazia a aba Login mostrar [ok] com o
//! dashboard em erro (spec §4.3).

use crate::providers::types::ProviderQuota;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoginState {
    /// Último fetch retornou quota sem erro.
    Ok,
    /// Fonte presente mas auth inválida (erro de API/token no fetch).
    NoToken,
    /// Sem sessão (erro tipado de não-logado, ou provider nunca visto).
    LoggedOut,
    /// Fetch em voo para este provider.
    Checking,
}

pub fn login_state_for(quota: Option<&ProviderQuota>, fetch_pending: bool) -> LoginState {
    if fetch_pending {
        return LoginState::Checking;
    }
    match quota {
        None => LoginState::LoggedOut,
        Some(q) => match (&q.error, q.available) {
            (None, _) => LoginState::Ok,
            (Some(e), _) if e.starts_with("Not logged in") => LoginState::LoggedOut,
            (Some(_), true) => LoginState::NoToken,
            (Some(_), false) => LoginState::LoggedOut,
        },
    }
}
```

Em `src/tui/render/login.rs`: apagar `is_logged_in` (linhas 19-29) e todo
uso; a lista da tela passa a chamar
`login_state_for(state.providers.iter().find(|pv| pv.quota.provider == id).map(|pv| &pv.quota), state.fetch_pending.iter().any(|p| p == id))`
(o campo `fetch_pending` chega na Task 5; até lá, passar `false` literal
com comentário `// Task 5 liga o pending real`). Labels:
`Ok → "ok"`, `NoToken → "sem token"`, `LoggedOut → "deslogado"`,
`Checking → "verificando…"`.

- [ ] **Step 4: Rodar e ver passar**

Run: `cargo test tui::login_state` → PASS.
Run: `INSTA_UPDATE=always cargo test render::login` → regrava o snapshot da
tela de login; inspecionar o `.snap` novo: os labels devem refletir o
estado derivado (sem `[ok]` para provider com erro).

- [ ] **Step 5: Commit**

```bash
git add src/tui/login_state.rs src/tui/mod.rs src/tui/render/login.rs src/tui/render/snapshots
git commit -m "feat(tui): estado de login derivado do fetch real"
```

---

## Fase 2 — Runtime assíncrono

### Task 4: `OwnedCtx` (contexto clonável cross-thread)

**Files:**
- Modify: `src/providers/mod.rs`

**Interfaces:**
- Consumes: `Ctx<'a>`, `Paths` (já `Clone`), `Settings` (já `Clone`).
- Produces:
  - `pub struct OwnedCtx { pub client: reqwest::Client, pub paths: Paths, pub settings: Settings, pub local_offset: UtcOffset, pub claude_usage_url: String, pub version: &'static str, pub home: PathBuf }`
  - `impl OwnedCtx { pub fn as_ctx(&self, now_ms: u64) -> Ctx<'_>; pub fn now_ms() -> u64; pub fn from_ctx(ctx: &Ctx<'_>) -> OwnedCtx }`
  - `pub(crate) async fn fetch_one(...)` (visibilidade sobe de privada para `pub(crate)`).

- [ ] **Step 1: Teste que falha**

No `mod tests` de `src/providers/mod.rs`:

```rust
#[test]
fn owned_ctx_round_trips_to_ctx() {
    let dir = tempdir().unwrap();
    let settings = settings();
    let client = reqwest::Client::new();
    let ctx = ctx_for(dir.path(), &settings, &client, 42);
    let owned = OwnedCtx::from_ctx(&ctx);
    let back = owned.as_ctx(43);
    assert_eq!(back.now_ms, 43);
    assert_eq!(back.claude_usage_url, ctx.claude_usage_url);
    assert_eq!(back.home, ctx.home);
    // Clonável (requisito para cruzar thread):
    let _second = owned.clone();
}
```

- [ ] **Step 2: Rodar e ver falhar**

Run: `cargo test providers::tests::owned_ctx`
Expected: FAIL (`OwnedCtx` não existe).

- [ ] **Step 3: Implementar**

Em `src/providers/mod.rs`, após `Ctx`:

```rust
/// Ctx com ownership — cruza threads (fetch fora do event loop, Task 5).
#[derive(Clone)]
pub struct OwnedCtx {
    pub client: reqwest::Client,
    pub paths: Paths,
    pub settings: Settings,
    pub local_offset: UtcOffset,
    pub claude_usage_url: String,
    pub version: &'static str,
    pub home: std::path::PathBuf,
}

impl OwnedCtx {
    pub fn from_ctx(ctx: &Ctx<'_>) -> Self {
        Self {
            client: ctx.client.clone(),
            paths: ctx.paths.clone(),
            settings: ctx.settings.clone(),
            local_offset: ctx.local_offset,
            claude_usage_url: ctx.claude_usage_url.clone(),
            version: ctx.version,
            home: ctx.home.clone(),
        }
    }

    pub fn as_ctx(&self, now_ms: u64) -> Ctx<'_> {
        Ctx {
            client: &self.client,
            paths: &self.paths,
            settings: &self.settings,
            now_ms,
            local_offset: self.local_offset,
            claude_usage_url: self.claude_usage_url.clone(),
            version: self.version,
            home: self.home.clone(),
        }
    }

    /// Epoch ms do relógio real (o fetch em thread não tem `ctx.now_ms` fresco).
    pub fn now_ms() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }
}
```

E trocar `async fn fetch_one` → `pub(crate) async fn fetch_one`.

- [ ] **Step 4: Rodar, gate e commit**

Run: `cargo test providers` → PASS.
Run: `cargo clippy --all-targets -- -D warnings` → limpo.

```bash
git add src/providers/mod.rs
git commit -m "feat(providers): OwnedCtx clonável cross-thread"
```

---

### Task 5: Fetch assíncrono com progresso por provider

**Files:**
- Create: `src/tui/fetch.rs`
- Modify: `src/tui/mod.rs` (declarar módulo)
- Modify: `src/tui/action.rs`, `src/tui/state.rs`, `src/tui/update.rs`, `src/tui/event_loop.rs`

**Interfaces:**
- Consumes: `OwnedCtx`, `fetch_one`, `registry()`, canal `bg_tx` existente.
- Produces:
  - Actions novas: `FetchStarted(Vec<String>)`, `ProviderFetched(ProviderQuota)`, `FetchCompleted { fetched_at: String }`, `ReloadUsage`.
  - Action removida: `DataFetched(AllQuotas)` (substituída pelas três acima; `FetchFailed(String)` permanece para erro de runtime).
  - `AppState.fetch_pending: Vec<String>` (ids em voo).
  - `pub fn spawn_fetch(tx: &tokio::sync::mpsc::UnboundedSender<Action>, octx: OwnedCtx, only: Option<String>)` em `tui::fetch`.
  - `run()` muda assinatura: `pub async fn run(octx: OwnedCtx, terminal: &mut DefaultTerminal) -> anyhow::Result<()>` (caller em `cli.rs`/`main.rs` constrói `OwnedCtx::from_ctx`).

- [ ] **Step 1: Testes de `update()` que falham**

No `mod tests` de `src/tui/update.rs` (seguir o padrão dos testes existentes
nas linhas 511-859):

```rust
#[test]
fn fetch_started_sets_loading_and_pending() {
    let mut state = AppState::new();
    let fu = update(&mut state, Action::FetchStarted(vec!["claude".into(), "amp".into()]));
    assert!(fu.is_empty());
    assert_eq!(state.status, FetchStatus::Loading);
    assert_eq!(state.fetch_pending, vec!["claude".to_string(), "amp".to_string()]);
}

#[test]
fn provider_fetched_merges_by_id_and_clears_pending() {
    let mut state = AppState::new();
    update(&mut state, Action::FetchStarted(vec!["claude".into()]));
    let q = test_quota("claude", 80.0); // helper existente dos testes; senão criar igual ao de login_state
    update(&mut state, Action::ProviderFetched(q.clone()));
    assert!(state.fetch_pending.is_empty());
    assert_eq!(state.providers.len(), 1);
    assert_eq!(state.providers[0].quota.provider, "claude");
    // Segundo fetch do mesmo provider substitui (não duplica):
    update(&mut state, Action::ProviderFetched(q));
    assert_eq!(state.providers.len(), 1);
}

#[test]
fn fetch_completed_sets_loaded_and_requests_usage_reload() {
    let mut state = AppState::new();
    update(&mut state, Action::FetchStarted(vec!["claude".into()]));
    update(&mut state, Action::ProviderFetched(test_quota("claude", 80.0)));
    let fu = update(&mut state, Action::FetchCompleted {
        fetched_at: "2026-07-01T18:00:00.000Z".into(),
    });
    assert_eq!(state.status, FetchStatus::Loaded);
    assert!(state.last_update.is_some());
    assert!(matches!(fu.as_slice(), [Action::ReloadUsage]));
}
```

- [ ] **Step 2: Rodar e ver falhar**

Run: `cargo test tui::update`
Expected: FAIL (Actions inexistentes).

- [ ] **Step 3: Implementar Actions + state + update**

`action.rs`: remover `DataFetched(AllQuotas)` e adicionar:

```rust
    /// Fetch iniciou para estes provider ids (spinner/progresso).
    FetchStarted(Vec<String>),
    /// Um provider terminou (merge incremental — a tela atualiza aos poucos).
    ProviderFetched(ProviderQuota),
    /// Todos terminaram. `fetched_at` ISO (mesmo formato do AllQuotas).
    FetchCompleted { fetched_at: String },
    /// Pede ao event_loop para redisparar o parse de usage (interceptada).
    ReloadUsage,
```

`state.rs`: adicionar `pub fetch_pending: Vec<String>` em `AppState`
(inicializar `Vec::new()` em `new()`).

`update.rs` (no `match` principal):

```rust
Action::FetchStarted(ids) => {
    state.status = FetchStatus::Loading;
    state.fetch_pending = ids;
    vec![]
}
Action::ProviderFetched(q) => {
    state.fetch_pending.retain(|id| id != &q.provider);
    match state
        .providers
        .iter_mut()
        .find(|pv| pv.quota.provider == q.provider)
    {
        Some(pv) => pv.quota = q,
        None => state.providers.push(ProviderView::new(q)),
    }
    vec![]
}
Action::FetchCompleted { fetched_at } => {
    state.fetch_pending.clear();
    state.status = FetchStatus::Loaded;
    // Mesmo parse de timestamp usado hoje pelo DataFetched (copiar de lá).
    state.last_update = time::OffsetDateTime::parse(
        &fetched_at,
        &time::format_description::well_known::Rfc3339,
    )
    .ok();
    vec![Action::ReloadUsage]
}
Action::ReloadUsage => vec![], // interceptada no event_loop; no update é no-op
```

O braço atual de `DataFetched` é removido; a lógica de mapear `AllQuotas` →
`providers` morre com ele (o merge incremental cobre).

`fetch.rs` (novo):

```rust
//! Fetch de quotas em thread dedicada — o event loop NUNCA espera rede.

use tokio::sync::mpsc::UnboundedSender;

use crate::providers::{fetch_one, iso_from_ms, registry, OwnedCtx, Provider};

use super::action::Action;

/// Dispara o fetch (todos os providers, ou só `only`) numa thread própria com
/// runtime tokio current_thread. Resultados chegam via `tx` como Actions.
pub fn spawn_fetch(tx: &UnboundedSender<Action>, octx: OwnedCtx, only: Option<String>) {
    let tx = tx.clone();
    std::thread::spawn(move || {
        let rt = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(rt) => rt,
            Err(e) => {
                let _ = tx.send(Action::FetchFailed(format!("fetch runtime: {e}")));
                return;
            }
        };
        rt.block_on(async move {
            let providers: Vec<Box<dyn Provider>> = registry()
                .into_iter()
                .filter(|p| only.as_deref().is_none_or(|id| p.id() == id))
                .collect();
            let ids: Vec<String> = providers.iter().map(|p| p.id().to_string()).collect();
            let _ = tx.send(Action::FetchStarted(ids));
            let now = OwnedCtx::now_ms();
            let ctx = octx.as_ctx(now);
            // Sequencial dentro da thread é aceitável (cada provider já tem
            // timeout de 10s + retry 1); mas mantemos o join concorrente:
            let futs = providers.iter().map(|p| fetch_one(p.as_ref(), &ctx));
            let mut stream = futures::stream::FuturesUnordered::from_iter(futs);
            use futures::StreamExt as _;
            while let Some(q) = stream.next().await {
                let _ = tx.send(Action::ProviderFetched(q));
            }
            let _ = tx.send(Action::FetchCompleted {
                fetched_at: iso_from_ms(now),
            });
        });
    });
}
```

- [ ] **Step 4: Reescrever o event_loop**

`event_loop.rs` — `run` vira:

```rust
pub async fn run(octx: OwnedCtx, terminal: &mut DefaultTerminal) -> anyhow::Result<()> {
    let mut state = AppState::new();
    let (bg_tx, mut bg_rx) = tokio::sync::mpsc::unbounded_channel::<Action>();
    let mut events = EventStream::new();
    let mut data_tick = tokio::time::interval_at(
        tokio::time::Instant::now() + Duration::from_secs(60),
        Duration::from_secs(60),
    );
    let mut anim_tick = interval(Duration::from_millis(30));

    // Fetch inicial: dispara e segue — o select! serve teclado/animação já.
    super::fetch::spawn_fetch(&bg_tx, octx.clone(), None);

    loop {
        terminal.draw(|f| render(&state, f))?;
        if state.should_quit {
            break;
        }
        tokio::select! {
            maybe_ev = events.next() => { /* bloco de teclado atual, inalterado */ }
            _ = data_tick.tick() => {
                super::fetch::spawn_fetch(&bg_tx, octx.clone(), None);
            }
            bg = bg_rx.recv() => {
                if let Some(action) = bg {
                    let follow_ups = update(&mut state, action);
                    drain(&mut state, &octx, &bg_tx, follow_ups);
                }
            }
            _ = anim_tick.tick() => { update(&mut state, Action::AnimTick); }
        }
    }
    Ok(())
}
```

`drain` ganha `&OwnedCtx` e `&bg_tx` (para interceptar `ReloadUsage` →
`spawn_usage_load`, que passa a receber `&OwnedCtx` e derivar
`claude_dir`/`codex_dir`/`fx_rate`/cutoffs a partir dele — mesma lógica,
trocando `ctx.` por `octx.` e `ctx.now_ms` por `OwnedCtx::now_ms()`):

```rust
Action::ReloadUsage => {
    spawn_usage_load(bg_tx, octx, state);
}
```

Callers: em `cli.rs`/`main.rs`, onde hoje se chama `tui::run(&ctx, ...)`,
construir `let octx = OwnedCtx::from_ctx(&ctx);` e passar `octx`.

- [ ] **Step 5: Header com progresso (spinner que agora VIVE)**

Em `src/tui/render/mod.rs`, na área de status do header (onde o throbber já
é renderizado — linhas ~335-356): quando `!state.fetch_pending.is_empty()`,
renderizar spinner (throbber existente) + `state.fetch_pending.join(" ")`
como "atualizando: claude …". Nenhum snapshot novo aqui (Task 10 regrava).

- [ ] **Step 6: Rodar tudo e ver passar**

Run: `cargo test tui::update` → PASS.
Run: `cargo test providers` → PASS.
Run: `cargo build` → compila (todos os call sites migrados).
Smoke manual: `tmux new -d -s smoke -x 100 -y 30 'target/debug/agent-bar menu'`;
`tmux capture-pane -t smoke -p` DEVE mostrar a moldura instantaneamente
(mesmo com rede lenta) e o spinner de fetch; teclas respondem durante o
fetch. `tmux kill-session -t smoke`.

- [ ] **Step 7: Gate + commit**

Run: `cargo clippy --all-targets -- -D warnings` → limpo.

```bash
git add src/tui src/providers/mod.rs src/cli.rs src/main.rs
git commit -m "feat(tui): fetch assíncrono fora do event loop"
```

---

### Task 6: Login/save não-bloqueantes + refetch pós-login

**Files:**
- Modify: `src/tui/event_loop.rs`, `src/tui/update.rs`, `src/tui/action.rs`, `src/tui/state.rs`

**Interfaces:**
- Consumes: `RealLogin::launch`, `spawn_fetch(only: Some(id))` (Task 5).
- Produces:
  - Action nova: `LoginFinished(String)` (id do provider).
  - `AppState.pending_login: Option<String>`, `AppState.pending_save: bool`.

- [ ] **Step 1: Testes que falham**

Em `src/tui/update.rs` tests:

```rust
#[test]
fn login_requested_sets_pending_and_status() {
    let mut state = AppState::new();
    let fu = update(&mut state, Action::LoginRequested("codex".into()));
    assert!(fu.is_empty()); // não re-enfileira mais: o event_loop lê pending_login
    assert_eq!(state.pending_login.as_deref(), Some("codex"));
    assert!(state.login_status.as_deref().unwrap_or("").contains("codex"));
}

#[test]
fn login_finished_success_requests_single_refetch() {
    let mut state = AppState::new();
    let fu = update(&mut state, Action::LoginFinished("codex".into()));
    // O event_loop intercepta e chama spawn_fetch(only=Some("codex")).
    assert!(matches!(fu.as_slice(), [Action::LoginFinished(id)] if id == "codex"));
}
```

(Nota: `LoginFinished` segue o padrão re-enfileirar-para-interceptar já
usado por `SaveConfig` — o update devolve a própria action UMA vez, marcada
por um campo `state.login_refetch_dispatched: bool` para não loopar;
detalhe abaixo.)

- [ ] **Step 2: Rodar e ver falhar**

Run: `cargo test tui::update` → FAIL.

- [ ] **Step 3: Implementar**

`state.rs`:

```rust
    /// Login pendente: o event_loop desenha 1 frame com o status e então
    /// suspende o terminal para o CLI de login.
    pub pending_login: Option<String>,
    /// Save pendente: mesmo padrão (frame "Salvando…" antes do IO).
    pub pending_save: bool,
```

`update.rs`:

```rust
Action::LoginRequested(id) => {
    state.login_status = Some(format!("Abrindo login para {id}…"));
    state.pending_login = Some(id);
    vec![]
}
Action::LoginFinished(id) => vec![Action::LoginFinished(id)], // interceptada
```

O braço `Action::SaveConfig` deixa de re-enfileirar: seta
`state.pending_save = true` e o status "Salvando…" (como hoje), devolve
`vec![]`.

`event_loop.rs` — no topo do loop, depois do `terminal.draw` (o frame com o
status JÁ FOI pintado neste ponto — é exatamente o fix):

```rust
        // IO pendente que exige frame prévio: o draw acima já pintou o status.
        if let Some(id) = state.pending_login.take() {
            handle_login(&mut state, id.clone());
            let follow_ups = update(&mut state, Action::LoginFinished(id));
            drain(&mut state, &octx, &bg_tx, follow_ups);
            continue;
        }
        if state.pending_save {
            state.pending_save = false;
            handle_save_config(&mut state, &octx);
            continue;
        }
```

`drain` intercepta `LoginFinished`:

```rust
Action::LoginFinished(id) => {
    super::fetch::spawn_fetch(bg_tx, octx.clone(), Some(id));
}
```

(Com a interceptação no drain, o guard anti-loop do Step 1 é desnecessário —
a action nunca re-entra no update; ajustar o teste para validar via drain se
preferir manter update 100% puro.)

Remover de `drain` a interceptação antiga de `LoginRequested` (agora o
fluxo é via `pending_login`); `handle_login` deixa de chamar `update`
internamente e passa a devolver o resultado para o caller aplicar via
`Action::LoginResult(result)` — assinatura:
`fn handle_login(state: &mut AppState, provider_id: String)` mantém, mas o
corpo vira:

```rust
fn handle_login(state: &mut AppState, provider_id: String) {
    use crate::tui::login_spawn::ProviderLogin as _;
    let login = RealLogin;
    let result = login.launch(&provider_id).map_err(|e| e.to_string());
    for a in update(state, Action::LoginResult(result)) {
        update(state, a);
    }
}
```

`Action::LoginResult` atual (update.rs:476-482) troca a mensagem "aperte r"
por "login concluído — atualizando quota…" (o refetch agora é automático).

- [ ] **Step 4: Rodar e ver passar + smoke**

Run: `cargo test tui::update` → PASS.
Run: `cargo test tui` → PASS (MockLogin nos testes de event_loop, se existirem, seguem o novo fluxo).
Smoke tmux (como na Task 5): na tela de Login, `Enter` deve mostrar
"Abrindo login para claude…" por um frame antes do REPL abrir; ao sair do
REPL, o status do provider atualiza sozinho (sem apertar `r`).

- [ ] **Step 5: Gate + commit**

```bash
git add src/tui
git commit -m "feat(tui): login e save com frame de feedback"
```

---

## Fase 3 — Navegação e telas

### Task 7: Theme tokens novos + severidade da API + gauge com gradiente

**Files:**
- Modify: `src/theme.rs` (5 tokens novos)
- Modify: `src/tui/widgets/severity.rs` (função com precedência da API)
- Modify: `src/tui/widgets/quota_gauge.rs` (reescrito: spans por célula)

**Interfaces:**
- Produces:
  - `ColorToken::{Surface, SelBg, ChipBg, EmptyTrack, GreenHi}` com hex `#1b202a`, `#2c333f`, `#262d3a`, `#343b49`, `#b5e890`.
  - `pub fn severity_color_api(api: Option<&str>, remaining_pct: Option<f64>) -> Color` em severity.rs.
  - `pub fn gauge_spans(remaining_pct: f64, width: usize, color: Color) -> Vec<Span<'static>>` em quota_gauge.rs — preenchido `█` por célula (gradiente do 60% da cor à cor plena ao longo do trecho preenchido), trilho `▒` em EmptyTrack.
- Consumes: `QuotaWindow.severity` (Task 1).

- [ ] **Step 1: Testes que falham**

`src/theme.rs` tests:

```rust
#[test]
fn new_surface_tokens_hex() {
    assert_eq!(ColorToken::Surface.hex(), "#1b202a");
    assert_eq!(ColorToken::SelBg.hex(), "#2c333f");
    assert_eq!(ColorToken::ChipBg.hex(), "#262d3a");
    assert_eq!(ColorToken::EmptyTrack.hex(), "#343b49");
    assert_eq!(ColorToken::GreenHi.hex(), "#b5e890");
}
```

`src/tui/widgets/severity.rs` tests:

```rust
#[test]
fn api_severity_takes_precedence() {
    use crate::theme::ColorToken;
    // API diz normal mesmo com pct baixo → verde (fonte oficial vence):
    assert_eq!(severity_color_api(Some("normal"), Some(5.0)), to_ratatui(ColorToken::Green));
    assert_eq!(severity_color_api(Some("warning"), Some(90.0)), to_ratatui(ColorToken::Yellow));
    assert_eq!(severity_color_api(Some("critical"), None), to_ratatui(ColorToken::Red));
    // Desconhecida/absent → fallback threshold local:
    assert_eq!(severity_color_api(Some("banana"), Some(5.0)), severity_color(Some(5.0)));
    assert_eq!(severity_color_api(None, Some(50.0)), severity_color(Some(50.0)));
}
```

`src/tui/widgets/quota_gauge.rs` tests:

```rust
#[test]
fn gauge_spans_fill_and_track_add_up_to_width() {
    let spans = gauge_spans(50.0, 10, ratatui::style::Color::Green);
    let total: usize = spans.iter().map(|s| s.content.chars().count()).sum();
    assert_eq!(total, 10);
    let filled: usize = spans
        .iter()
        .filter(|s| s.content.contains('█'))
        .map(|s| s.content.chars().count())
        .sum();
    assert_eq!(filled, 5);
}

#[test]
fn gauge_spans_zero_and_full() {
    let z = gauge_spans(0.0, 8, ratatui::style::Color::Red);
    assert!(z.iter().all(|s| !s.content.contains('█')));
    let f = gauge_spans(100.0, 8, ratatui::style::Color::Green);
    assert!(f.iter().all(|s| !s.content.contains('▒')));
}
```

- [ ] **Step 2: Rodar e ver falhar**

Run: `cargo test theme` → FAIL. Depois `cargo test widgets` → FAIL.

- [ ] **Step 3: Implementar**

`theme.rs`: adicionar os 5 variants ao enum e ao `match` de `hex()`.

`severity.rs`:

```rust
/// Severidade com precedência da API (spec §4.1): valores conhecidos da API
/// vencem o threshold local; desconhecido/ausente cai no cálculo local.
pub fn severity_color_api(api: Option<&str>, remaining_pct: Option<f64>) -> Color {
    match api.map(str::to_ascii_lowercase).as_deref() {
        Some("normal") | Some("ok") => to_ratatui(ColorToken::Green),
        Some("warning") | Some("elevated") | Some("high") => to_ratatui(ColorToken::Yellow),
        Some("critical") | Some("exceeded") | Some("blocked") => to_ratatui(ColorToken::Red),
        _ => severity_color(remaining_pct),
    }
}
```

`quota_gauge.rs` (substitui `block_bar` e os builders mortos
`quota_gauge_line`/`window_gauge_line`/`model_gauge_line` — apagá-los):

```rust
//! Gauge por célula: gradiente sutil no trecho preenchido + trilho.

use ratatui::style::{Color, Style};
use ratatui::text::Span;

use crate::theme::ColorToken;
use crate::tui::theme_bridge::to_ratatui;

/// Interpola linearmente entre duas cores RGB (t em 0.0..=1.0).
fn lerp_rgb(a: Color, b: Color, t: f64) -> Color {
    let ((ar, ag, ab), (br, bg, bb)) = match (a, b) {
        (Color::Rgb(r1, g1, b1), Color::Rgb(r2, g2, b2)) => ((r1, g1, b1), (r2, g2, b2)),
        _ => return b,
    };
    let mix = |x: u8, y: u8| -> u8 {
        (f64::from(x) + (f64::from(y) - f64::from(x)) * t).round() as u8
    };
    Color::Rgb(mix(ar, br), mix(ag, bg), mix(ab, bb))
}

/// Escurece a cor para o início do gradiente (60% da intensidade).
fn dimmed(c: Color) -> Color {
    lerp_rgb(Color::Rgb(0, 0, 0), c, 0.6)
}

/// Barra de quota: `remaining_pct` em 0..=100, `width` células.
/// Preenchido = `█` com gradiente dimmed→cor ao longo do trecho;
/// trilho = `▒` em EmptyTrack. Total de células == width, sempre.
pub fn gauge_spans(remaining_pct: f64, width: usize, color: Color) -> Vec<Span<'static>> {
    let pct = remaining_pct.clamp(0.0, 100.0);
    let filled = ((width as f64) * pct / 100.0).round() as usize;
    let mut spans = Vec::with_capacity(width.min(filled + 1));
    for i in 0..filled {
        let t = if filled <= 1 { 1.0 } else { i as f64 / (filled - 1) as f64 };
        spans.push(Span::styled(
            "█".to_string(),
            Style::default().fg(lerp_rgb(dimmed(color), color, t)),
        ));
    }
    if width > filled {
        spans.push(Span::styled(
            "▒".repeat(width - filled),
            Style::default().fg(to_ratatui(ColorToken::EmptyTrack)),
        ));
    }
    spans
}
```

Call sites de `block_bar` (dashboard.rs, detail.rs) migram para
`gauge_spans` nas Tasks 11-12; nesta task, para compilar, trocar cada
`Span::styled(block_bar(rem, w), Style::default().fg(color))` por
`Line::from(gauge_spans(rem, w, color))`-style composição inline (mesma
largura — os snapshots serão regravados de qualquer forma nas tasks 11-12;
aqui rodar `INSTA_UPDATE=always cargo test render` e inspecionar que só o
caractere de trilho mudou `░`→`▒`).

- [ ] **Step 4: Rodar e ver passar**

Run: `cargo test theme` → PASS. `cargo test widgets` → PASS.
Run: `INSTA_UPDATE=always cargo test render` → snapshots regravados; diff
esperado: trilho `▒` e (nos snapshots sem cor) nada mais.

- [ ] **Step 5: Gate + commit**

```bash
git add src/theme.rs src/tui
git commit -m "feat(tui): tokens novos, severidade da API e gauge"
```

---

### Task 8: Navegação `Screen`/`SidebarItem` (state + teclado)

**Files:**
- Modify: `src/tui/state.rs`, `src/tui/action.rs`, `src/tui/update.rs`
- Modify: `src/tui/render/mod.rs` (dispatch por `Screen` — mínimo pra compilar; layout completo é a Task 10)

**Interfaces:**
- Produces:
  - `pub enum Screen { Overview, Detail, History, Login, Waybar }` (substitui `Tab` + `Mode`).
  - `pub enum SidebarItem { Overview, Provider(usize), History, Login, Waybar }` com `pub fn sidebar_items(n_providers: usize) -> Vec<SidebarItem>` = `[Overview, Provider(0..n), History, Login, Waybar]`.
  - `AppState.screen: Screen`, `AppState.sidebar_selected: usize`, `AppState.scroll: u16`. Campos `tab`, `mode`, `focus` removidos.
  - Actions: `SelectSidebar(usize)`, `Activate(SidebarItem)`; `SwitchTab(Tab)` removida (e o enum `Tab`).
- Consumes: `state.providers` (ordem = ordem da sidebar).

- [ ] **Step 1: Testes que falham (update de navegação)**

```rust
#[test]
fn sidebar_items_order() {
    let items = sidebar_items(2);
    assert_eq!(items, vec![
        SidebarItem::Overview,
        SidebarItem::Provider(0),
        SidebarItem::Provider(1),
        SidebarItem::History,
        SidebarItem::Login,
        SidebarItem::Waybar,
    ]);
}

#[test]
fn up_down_move_sidebar_and_enter_activates() {
    let mut state = AppState::new();
    state.providers = vec![ProviderView::new(test_quota("claude", 80.0))];
    update(&mut state, Action::Down); // Overview → Provider(0)
    assert_eq!(state.sidebar_selected, 1);
    update(&mut state, Action::OpenDetail); // Enter
    assert_eq!(state.screen, Screen::Detail);
    assert_eq!(state.selected, 0);
    update(&mut state, Action::Back); // Esc
    assert_eq!(state.screen, Screen::Overview);
}

#[test]
fn activate_history_login_waybar() {
    let mut state = AppState::new();
    update(&mut state, Action::Activate(SidebarItem::History));
    assert_eq!(state.screen, Screen::History);
    update(&mut state, Action::Activate(SidebarItem::Login));
    assert_eq!(state.screen, Screen::Login);
    let fu = update(&mut state, Action::Activate(SidebarItem::Waybar));
    assert_eq!(state.screen, Screen::Waybar);
    // Entrar na Waybar inicializa o config (comportamento atual do SwitchTab):
    assert!(matches!(fu.as_slice(), [Action::InitConfig(_)]));
}
```

- [ ] **Step 2: Rodar e ver falhar**

Run: `cargo test tui::update` → FAIL.

- [ ] **Step 3: Implementar**

`state.rs`: remover `Tab`, `Mode`, `Panel`; adicionar:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Overview,
    Detail,
    History,
    Login,
    Waybar,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidebarItem {
    Overview,
    Provider(usize),
    History,
    Login,
    Waybar,
}

pub fn sidebar_items(n_providers: usize) -> Vec<SidebarItem> {
    let mut v = vec![SidebarItem::Overview];
    v.extend((0..n_providers).map(SidebarItem::Provider));
    v.extend([SidebarItem::History, SidebarItem::Login, SidebarItem::Waybar]);
    v
}
```

`AppState`: `pub screen: Screen` (init `Screen::Overview`),
`pub sidebar_selected: usize` (0), `pub scroll: u16` (0). Manter `selected`
(índice do provider em Detail).

`update.rs` — teclado (em `key_to_action_with_state`) e braços:

```rust
Action::Up => {
    state.sidebar_selected = state.sidebar_selected.saturating_sub(1);
    vec![]
}
Action::Down => {
    let max = sidebar_items(state.providers.len()).len() - 1;
    state.sidebar_selected = (state.sidebar_selected + 1).min(max);
    vec![]
}
Action::OpenDetail => {
    let items = sidebar_items(state.providers.len());
    match items.get(state.sidebar_selected).copied() {
        Some(item) => vec![Action::Activate(item)],
        None => vec![],
    }
}
Action::Activate(item) => match item {
    SidebarItem::Overview => {
        state.screen = Screen::Overview;
        vec![]
    }
    SidebarItem::Provider(i) => {
        state.selected = i;
        state.screen = Screen::Detail;
        vec![]
    }
    SidebarItem::History => {
        state.screen = Screen::History;
        vec![]
    }
    SidebarItem::Login => {
        state.screen = Screen::Login;
        vec![]
    }
    SidebarItem::Waybar => {
        state.screen = Screen::Waybar;
        // Placeholder: o drain injeta as settings reais (padrão atual).
        vec![Action::InitConfig(crate::providers::test_default_settings_placeholder())]
    }
},
Action::SelectSidebar(i) => {
    state.sidebar_selected = i;
    vec![]
}
Action::Back => {
    state.screen = Screen::Overview;
    vec![]
}
```

(Para o placeholder de `InitConfig`: usar o mesmo mecanismo que o código
atual usa ao trocar pra aba Waybar — inspecionar o braço `SwitchTab`
existente e replicar; se hoje passa `Settings::default()`-like via clone,
manter idêntico.)

Atalhos: `h` → `Activate(History)`, `g` → `Activate(Login)`, `w` →
`Activate(Waybar)`, `Esc` → `Back`, `?` → `ToggleHelp`, `q` → `Quit`,
`r` → `Refresh` (inalterado). Remover `←`/`→` de troca de aba.

`render/mod.rs`: trocar o dispatch `(Tab, Mode)` por `state.screen`
(mapear Overview→render_dashboard, Detail→render_detail,
History→render_history, Login→render_login, Waybar→render_config); a tab
bar deixa de renderizar (a Task 10 constrói o layout novo). Ajustar todos
os usos de `state.tab`/`state.mode`/`state.focus` no crate
(`grep -rn "state.tab\|state.mode\|state.focus\|Tab::" src/`).

- [ ] **Step 4: Rodar e ver passar**

Run: `cargo test tui::update` → PASS.
Run: `INSTA_UPDATE=always cargo test render` → regrava (sem tab bar);
inspecionar os `.snap`: telas seguem íntegras, sem a linha de tabs.

- [ ] **Step 5: Gate + commit**

```bash
git add src/tui
git commit -m "feat(tui): navegação Screen/SidebarItem sem tabs"
```

---

### Task 9: Mouse — HitMap, captura e eventos

**Files:**
- Create: `src/tui/mouse.rs`
- Modify: `src/tui/mod.rs`, `src/tui/action.rs`, `src/tui/update.rs`, `src/tui/event_loop.rs`, `src/tui/render/mod.rs`
- Modify: ponto de init do terminal do menu (onde `ratatui::init`/terminal setup do comando menu acontece — localizar com `grep -rn "ratatui::init\|DefaultTerminal" src/main.rs src/cli.rs src/runtime.rs`)

**Interfaces:**
- Produces:
  - `pub enum MouseTarget { Sidebar(usize), Card(usize), Chip(ChipKind) }` e `pub enum ChipKind { Open, Refresh, Help, Quit, Back, Login, History }` (ambos `Clone, Copy, Debug, PartialEq`).
  - `pub struct HitMap` com `pub fn clear(&mut self)`, `pub fn push(&mut self, rect: Rect, t: MouseTarget)`, `pub fn at(&self, x: u16, y: u16) -> Option<MouseTarget>` (último registrado vence — z-order).
  - `render` muda assinatura: `pub fn render(state: &AppState, frame: &mut Frame, hits: &mut HitMap)`.
  - Actions: `Click(MouseTarget)`, `Hover(Option<MouseTarget>)`, `Scroll(i32)`.
  - `AppState.hover: Option<MouseTarget>`.

- [ ] **Step 1: Testes que falham**

`src/tui/mouse.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::layout::Rect;

    #[test]
    fn hitmap_last_registered_wins() {
        let mut h = HitMap::default();
        h.push(Rect::new(0, 0, 10, 10), MouseTarget::Card(0));
        h.push(Rect::new(2, 2, 3, 3), MouseTarget::Chip(ChipKind::Refresh));
        assert_eq!(h.at(3, 3), Some(MouseTarget::Chip(ChipKind::Refresh)));
        assert_eq!(h.at(0, 0), Some(MouseTarget::Card(0)));
        assert_eq!(h.at(50, 50), None);
        h.clear();
        assert_eq!(h.at(3, 3), None);
    }
}
```

`update.rs` tests:

```rust
#[test]
fn click_sidebar_selects_and_activates() {
    let mut state = AppState::new();
    state.providers = vec![ProviderView::new(test_quota("claude", 80.0))];
    update(&mut state, Action::Click(MouseTarget::Sidebar(1)));
    assert_eq!(state.sidebar_selected, 1);
    assert_eq!(state.screen, Screen::Detail); // Provider(0) ativado
}

#[test]
fn hover_and_scroll_update_state() {
    let mut state = AppState::new();
    update(&mut state, Action::Hover(Some(MouseTarget::Card(0))));
    assert_eq!(state.hover, Some(MouseTarget::Card(0)));
    state.scroll = 2;
    update(&mut state, Action::Scroll(-1));
    assert_eq!(state.scroll, 1);
    update(&mut state, Action::Scroll(-5));
    assert_eq!(state.scroll, 0); // saturating
}
```

- [ ] **Step 2: Rodar e ver falhar** — `cargo test tui` → FAIL.

- [ ] **Step 3: Implementar**

`mouse.rs`:

```rust
//! Hit-testing de mouse: o render registra regiões clicáveis; o event_loop
//! consulta no MouseEvent. update() permanece puro.

use ratatui::layout::Rect;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChipKind {
    Open,
    Refresh,
    Help,
    Quit,
    Back,
    Login,
    History,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseTarget {
    Sidebar(usize),
    Card(usize),
    Chip(ChipKind),
}

#[derive(Debug, Default)]
pub struct HitMap {
    zones: Vec<(Rect, MouseTarget)>,
}

impl HitMap {
    pub fn clear(&mut self) {
        self.zones.clear();
    }

    pub fn push(&mut self, rect: Rect, t: MouseTarget) {
        self.zones.push((rect, t));
    }

    pub fn at(&self, x: u16, y: u16) -> Option<MouseTarget> {
        self.zones
            .iter()
            .rev()
            .find(|(r, _)| {
                x >= r.x && x < r.x + r.width && y >= r.y && y < r.y + r.height
            })
            .map(|(_, t)| *t)
    }
}
```

`action.rs`: adicionar `Click(MouseTarget)`, `Hover(Option<MouseTarget>)`,
`Scroll(i32)`. `state.rs`: `pub hover: Option<MouseTarget>` (None).

`update.rs`:

```rust
Action::Click(target) => match target {
    MouseTarget::Sidebar(i) => {
        state.sidebar_selected = i;
        let items = sidebar_items(state.providers.len());
        match items.get(i).copied() {
            Some(item) => vec![Action::Activate(item)],
            None => vec![],
        }
    }
    MouseTarget::Card(i) => vec![Action::Activate(SidebarItem::Provider(i))],
    MouseTarget::Chip(ChipKind::Open) => vec![Action::OpenDetail],
    MouseTarget::Chip(ChipKind::Refresh) => vec![Action::Refresh],
    MouseTarget::Chip(ChipKind::Help) => vec![Action::ToggleHelp],
    MouseTarget::Chip(ChipKind::Quit) => vec![Action::Quit],
    MouseTarget::Chip(ChipKind::Back) => vec![Action::Back],
    MouseTarget::Chip(ChipKind::Login) => vec![Action::Activate(SidebarItem::Login)],
    MouseTarget::Chip(ChipKind::History) => vec![Action::Activate(SidebarItem::History)],
},
Action::Hover(t) => {
    state.hover = t;
    vec![]
}
Action::Scroll(delta) => {
    state.scroll = state.scroll.saturating_add_signed(delta as i16).max(0);
    vec![]
}
```

(`u16::saturating_add_signed(i16)` existe desde Rust 1.66; converter
`delta` com `as i16` é seguro para os valores ±1/±3 do wheel.)

`event_loop.rs`:

```rust
let mut hits = super::mouse::HitMap::default();
// ...
terminal.draw(|f| {
    hits.clear();
    render(&state, f, &mut hits)
})?;
// ... no branch de eventos:
if let Event::Mouse(m) = &ev {
    use ratatui::crossterm::event::{MouseButton, MouseEventKind};
    let action = match m.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            hits.at(m.column, m.row).map(Action::Click)
        }
        MouseEventKind::Moved => Some(Action::Hover(hits.at(m.column, m.row))),
        MouseEventKind::ScrollUp => Some(Action::Scroll(-1)),
        MouseEventKind::ScrollDown => Some(Action::Scroll(1)),
        _ => None,
    };
    if let Some(a) = action {
        let follow_ups = update(&mut state, a);
        drain(&mut state, &octx, &bg_tx, follow_ups);
    }
}
```

Captura: no init do terminal do menu,
`crossterm::execute!(std::io::stdout(), crossterm::event::EnableMouseCapture)?`
após entrar no alternate screen, e `DisableMouseCapture` na restauração
(incl. o caminho do login que suspende o terminal — `login_spawn.rs`
restaura/reinicializa: desabilitar antes do spawn, reabilitar depois).

`render/mod.rs`: assinatura nova; por ora registrar zonas da sidebar
(uma por item, `Rect` de cada linha) — os cards/chips registram nas
Tasks 10-11.

- [ ] **Step 4: Rodar e ver passar + smoke**

Run: `cargo test tui` → PASS.
Smoke tmux: `tmux send-keys` não testa mouse; usar
`tmux new -d ... 'target/debug/agent-bar menu'` e verificar via capture que
nada regrediu no teclado. Teste manual de mouse fica no gate da Task 11.

- [ ] **Step 5: Gate + commit**

```bash
git add src/tui
git commit -m "feat(tui): mouse com HitMap e captura crossterm"
```

---

### Task 10: Render base — moldura, sidebar, chips centrados, responsivo

**Files:**
- Modify: `src/tui/render/mod.rs` (layout raiz novo)
- Create: `src/tui/render/sidebar.rs`
- Create: `src/tui/widgets/chips.rs`
- Modify: `src/tui/render/status_bar.rs` (remover — o rodapé vira chips por tela; deletar arquivo e referências)

**Interfaces:**
- Produces:
  - `pub fn render_sidebar(state: &AppState, frame: &mut Frame, area: Rect, hits: &mut HitMap)` — seções VISÃO/PROVIDERS/MAIS, item selecionado com bg `SelBg` + bold, hover com bg `Surface`, % de relance por provider, deslogado dim.
  - `pub fn chips_line(chips: &[(ChipKind, &str, &str)], width: u16) -> Line<'static>` em `widgets/chips.rs` — `(kind, tecla, label)`; chips com bg `ChipBg`, tecla em Cyan bold, **linha centralizada** na largura.
  - `pub fn register_chip_hits(...)` — registra os `Rect`s de cada chip no `HitMap` (mesma matemática de centralização; assinatura: `(chips: &[(ChipKind, &str, &str)], area: Rect, hits: &mut HitMap)`).
  - Layout raiz: bloco externo `BorderType::Rounded` com título ` agent-bar ` (Blue bold) na borda; interna: horizontal `[sidebar Length(17) | content Min(0)]`; sidebar colapsa para `Length(3)` (só marcas ◆●●) quando `frame.area().width < 80`.
- Consumes: `HitMap`, tokens Task 7, `Screen` Task 8.

- [ ] **Step 1: Teste do chips_line (unit, sem snapshot)**

`src/tui/widgets/chips.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::mouse::ChipKind;

    #[test]
    fn chips_line_is_centered() {
        let chips = [(ChipKind::Quit, "q", "sair")];
        let line = chips_line(&chips, 40);
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        // ' q sair ' = 8 células visíveis → 16 de padding em cada lado.
        assert_eq!(text.chars().count(), 40 - 16); // padding direito não é emitido
        let leading: usize = text.chars().take_while(|c| *c == ' ').count();
        assert_eq!(leading, 16);
    }
}
```

- [ ] **Step 2: Rodar e ver falhar** — `cargo test widgets` → FAIL.

- [ ] **Step 3: Implementar chips + sidebar + layout**

`widgets/chips.rs`:

```rust
//! Chips de ação (rodapé): sempre centralizados — contrato de alinhamento.

use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

use crate::theme::ColorToken;
use crate::tui::mouse::{ChipKind, HitMap, MouseTarget};
use crate::tui::theme_bridge::to_ratatui;

const GAP: &str = "   ";

fn chip_width(key: &str, label: &str) -> u16 {
    // ' key ' + 'label ' → key+2 + label+1
    (key.chars().count() + 2 + label.chars().count() + 1) as u16
}

fn total_width(chips: &[(ChipKind, &str, &str)]) -> u16 {
    let sum: u16 = chips.iter().map(|(_, k, l)| chip_width(k, l)).sum();
    sum + (GAP.chars().count() as u16) * (chips.len().saturating_sub(1) as u16)
}

pub fn chips_line(chips: &[(ChipKind, &str, &str)], width: u16) -> Line<'static> {
    let pad = width.saturating_sub(total_width(chips)) / 2;
    let mut spans = vec![Span::raw(" ".repeat(pad as usize))];
    for (i, (_, key, label)) in chips.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw(GAP));
        }
        spans.push(Span::styled(
            format!(" {key} "),
            Style::default()
                .fg(to_ratatui(ColorToken::Cyan))
                .bg(to_ratatui(ColorToken::ChipBg))
                .add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::styled(
            format!("{label} "),
            Style::default()
                .fg(to_ratatui(ColorToken::Muted))
                .bg(to_ratatui(ColorToken::ChipBg)),
        ));
    }
    Line::from(spans)
}

pub fn register_chip_hits(chips: &[(ChipKind, &str, &str)], area: Rect, hits: &mut HitMap) {
    let mut x = area.x + area.width.saturating_sub(total_width(chips)) / 2;
    for (i, (kind, key, label)) in chips.iter().enumerate() {
        if i > 0 {
            x += GAP.chars().count() as u16;
        }
        let w = chip_width(key, label);
        hits.push(Rect::new(x, area.y, w, 1), MouseTarget::Chip(*kind));
        x += w;
    }
}
```

`render/sidebar.rs` — estrutura (rows fixas; cada linha de item registra
`MouseTarget::Sidebar(i)` no HitMap):

```rust
//! Sidebar: VISÃO / PROVIDERS / MAIS. Sem tabs — este é o hub de navegação.

use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::theme::{provider_hex, ColorToken};
use crate::tui::mouse::{HitMap, MouseTarget};
use crate::tui::state::{sidebar_items, AppState, SidebarItem};
use crate::tui::theme_bridge::to_ratatui;

fn item_label(state: &AppState, item: SidebarItem) -> Line<'static> {
    match item {
        SidebarItem::Overview => Line::from(" ▸ Geral".to_string()),
        SidebarItem::Provider(i) => {
            let pv = &state.providers[i];
            let mark = if pv.quota.provider == "claude" { "◆" } else { "●" };
            let pct = pv
                .quota
                .primary
                .as_ref()
                .map(|w| format!("{:>3.0}%", w.remaining))
                .unwrap_or_else(|| "  – ".to_string());
            Line::from(format!(" {mark} {:<7}{pct}", pv.quota.display_name))
        }
        SidebarItem::History => Line::from("   Histórico".to_string()),
        SidebarItem::Login => Line::from("   Login".to_string()),
        SidebarItem::Waybar => Line::from("   Waybar".to_string()),
    }
}

pub fn render_sidebar(state: &AppState, frame: &mut Frame, area: Rect, hits: &mut HitMap) {
    let items = sidebar_items(state.providers.len());
    let mut lines: Vec<Line> = Vec::new();
    let mut row_of_item: Vec<u16> = Vec::new();

    for (i, item) in items.iter().enumerate() {
        // Cabeçalhos de seção antes do primeiro item de cada grupo:
        match item {
            SidebarItem::Overview => lines.push(section(" VISÃO")),
            SidebarItem::Provider(0) => {
                lines.push(Line::from(""));
                lines.push(section(" PROVIDERS"));
            }
            SidebarItem::History => {
                lines.push(Line::from(""));
                lines.push(section(" MAIS"));
            }
            _ => {}
        }
        let mut line = item_label(state, *item);
        let selected = state.sidebar_selected == i;
        let hovered = state.hover == Some(MouseTarget::Sidebar(i));
        let style = if selected {
            Style::default()
                .bg(to_ratatui(ColorToken::SelBg))
                .add_modifier(Modifier::BOLD)
        } else if hovered {
            Style::default().bg(to_ratatui(ColorToken::Surface))
        } else {
            Style::default()
        };
        line = line.style(style.fg(item_color(state, *item)));
        row_of_item.push(area.y + lines.len() as u16);
        lines.push(line);
    }

    for (i, row) in row_of_item.iter().enumerate() {
        hits.push(Rect::new(area.x, *row, area.width, 1), MouseTarget::Sidebar(i));
    }
    frame.render_widget(Paragraph::new(lines), area);
}

fn section(label: &str) -> Line<'static> {
    Line::from(Span::styled(
        label.to_string(),
        Style::default()
            .fg(to_ratatui(ColorToken::Comment))
            .add_modifier(Modifier::BOLD),
    ))
}

fn item_color(state: &AppState, item: SidebarItem) -> ratatui::style::Color {
    match item {
        SidebarItem::Provider(i) => {
            let pv = &state.providers[i];
            if pv.quota.error.is_some() {
                to_ratatui(ColorToken::Muted) // deslogado/erro: dim
            } else {
                crate::tui::theme_bridge::hex_to_color(provider_hex(&pv.quota.provider))
            }
        }
        _ => to_ratatui(ColorToken::Text),
    }
}
```

(`hex_to_color`: se `theme_bridge.rs` ainda não expõe um conversor
hex→`Color::Rgb`, adicionar `pub fn hex_to_color(hex: &str) -> Color`
parseando os 6 dígitos — mesma lógica do `ansi_truecolor` de `theme.rs`,
sem `unwrap`.)

`render/mod.rs` — layout raiz:

```rust
pub fn render(state: &AppState, frame: &mut Frame, hits: &mut HitMap) {
    let area = frame.area();
    let outer = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(to_ratatui(ColorToken::Comment)))
        .title(Span::styled(
            " agent-bar ",
            Style::default()
                .fg(to_ratatui(ColorToken::Blue))
                .add_modifier(Modifier::BOLD),
        ))
        .title(header_status(state)); // canto direito: custo hoje + relógio + spinner
    let inner = outer.inner(area);
    frame.render_widget(outer, area);

    let sidebar_w: u16 = if area.width < 80 { 3 } else { 17 };
    let cols = Layout::horizontal([Constraint::Length(sidebar_w), Constraint::Min(0)])
        .split(inner);
    render_sidebar(state, frame, cols[0], hits);
    match state.screen {
        Screen::Overview => render_dashboard(state, frame, cols[1], hits),
        Screen::Detail => render_detail(state, frame, cols[1], hits),
        Screen::History => render_history(state, frame, cols[1], hits),
        Screen::Login => render_login(state, frame, cols[1], hits),
        Screen::Waybar => render_config(state, frame, cols[1], hits),
    }
    if state.show_help {
        render_help_overlay(state, frame); // Task 14 corrige o Clear
    }
}
```

`header_status(state)`: título à direita
(`Title::from(...).alignment(Alignment::Right)`) com
`{spinner se fetch_pending} · ${custo_hoje} · HH:MM` — custo de
`state.usage.total_cost.usd`, relógio de `state.last_update`. As telas
(`render_dashboard` etc.) ganham o parâmetro `hits` (repassar; usos reais
nas próximas tasks). Sidebar colapsada (width 3): renderizar só a coluna de
marcas `◆●●` (uma por provider, sem labels) — `item_label` ganha variante
curta quando `area.width < 6`.

- [ ] **Step 4: Snapshots novos**

Run: `INSTA_UPDATE=always cargo test render`
Inspecionar `.snap`s: moldura arredondada única, sidebar com 3 seções,
sem tab bar, sem status_bar antigo. Adicionar teste de largura estreita:

```rust
#[test]
fn sidebar_collapses_below_80_cols() {
    // render num TestBackend 70x30; assert que a área de conteúdo começa
    // na coluna 4 (sidebar de 3) — usar o padrão de teste de render existente.
}
```

- [ ] **Step 5: Gate + commit**

Run: `cargo test tui` → PASS. `cargo clippy --all-targets -- -D warnings`.

```bash
git add src/tui
git commit -m "feat(tui): layout raiz com sidebar e chips"
```

---

### Task 11: Tela Visão Geral (cards + scroll + estados)

**Files:**
- Modify: `src/tui/render/dashboard.rs` (reescrito como Overview)
- Modify: `Cargo.toml` (adicionar `tui-scrollview = "0.6"`)

**Interfaces:**
- Consumes: `gauge_spans`, `severity_color_api`, `sparkline_str` (existente), `provider_series_24h` (Task 2), `login_state_for` (Task 3), `chips_line`/`register_chip_hits` (Task 10), `HitMap`.
- Produces: `pub fn render_dashboard(state: &AppState, frame: &mut Frame, area: Rect, hits: &mut HitMap)` — um card por provider (altura 6: título/2 gauges/sparkline+custo/borda), rolável via `state.scroll` com `tui_scrollview::ScrollView`; card registra `MouseTarget::Card(i)`; rodapé chips `[↵ abrir] [r atualizar] [? ajuda] [q sair]`.

- [ ] **Step 1: Estrutura do card (código de referência)**

Cada card (área `Rect` de 6 linhas dentro do ScrollView):

```rust
fn render_provider_card(
    state: &AppState,
    pv: &ProviderView,
    idx: usize,
    buf_area: Rect,
    sv: &mut tui_scrollview::ScrollView,
) {
    let q = &pv.quota;
    let brand = hex_to_color(provider_hex(&q.provider));
    let logged = login_state_for(Some(q), state.fetch_pending.iter().any(|p| *p == q.provider));
    let status = match logged {
        LoginState::Ok => Span::styled("● ok", Style::default().fg(to_ratatui(ColorToken::Green))),
        LoginState::Checking => Span::styled("● verificando…", Style::default().fg(to_ratatui(ColorToken::Yellow))),
        LoginState::NoToken => Span::styled("● sem token", Style::default().fg(to_ratatui(ColorToken::Yellow))),
        LoginState::LoggedOut => Span::styled("○ deslogado", Style::default().fg(to_ratatui(ColorToken::Red))),
    };
    let title = match &q.plan {
        Some(p) => format!(" {} · {} ", q.display_name, p),
        None => format!(" {} ", q.display_name),
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(to_ratatui(ColorToken::Comment)))
        .title(Span::styled(title, Style::default().fg(brand).add_modifier(Modifier::BOLD)))
        .title(Title::from(Line::from(status)).alignment(Alignment::Right));
    // corpo:
    //  - deslogado → 1 linha: "sem sessão — clique aqui ou g para logar"
    //  - senão → linha por janela: label 8 + gauge_spans(rem, W, severity_color_api(...))
    //            + pct right-aligned 4 + reset; depois sparkline real:
    //    let series = state.history.as_deref()
    //        .map(|r| provider_series_24h(r, &q.provider, now))
    //        .unwrap_or_default();
    //    sparkline_str(&series) + custo do dia (state.usage por provider).
    //  Alinhamento: TODOS os gauges do card na mesma coluna (label fixo 8).
}
```

Largura do gauge: `let gauge_w = (area.width as usize).saturating_sub(8 + 6 + 14).max(10);`
(8 label, 6 pct, 14 reset) — derivada da área real, nunca constante mágica
(corrige o bug de largura do código antigo).

ScrollView: conteúdo `Rect` de altura `6 * n_cards`; `state.scroll` limita
em `content_h.saturating_sub(viewport_h)`; wheel já emite `Action::Scroll`
(Task 9). Card hit: `hits.push(rect_do_card_no_viewport, MouseTarget::Card(idx))`
(só para cards visíveis; calcular offset do scroll).

Estados: `state.providers` vazio + `fetch_pending` não-vazio → skeleton
(3 cards com trilhos `▒` em EmptyTrack e spinner); ambos vazios → linha
"nenhum provider habilitado — veja a tela Waybar".

- [ ] **Step 2: Snapshots (3 estados)**

Testes de snapshot no padrão do arquivo (TestBackend 100x32):
`overview_all_ok` (3 providers com dados), `overview_codex_logged_out`,
`overview_loading_skeleton`. Run: `INSTA_UPDATE=always cargo test render::dashboard`
e inspecionar os três `.snap`.

- [ ] **Step 3: Smoke tmux**

`tmux new -d -s ov -x 100 -y 32 'target/debug/agent-bar menu'`; capture:
cards com bordas arredondadas, sparkline do Claude ≠ sparkline do Codex
(dado real), Codex deslogado com CTA. Mouse manual: click num card abre o
detalhe. `tmux kill-session -t ov`.

- [ ] **Step 4: Gate + commit**

Run: `cargo test tui` → PASS; `cargo test --test golden` → PASS;
`cargo clippy --all-targets -- -D warnings`.

```bash
git add src/tui Cargo.toml Cargo.lock
git commit -m "feat(tui): tela visão geral com cards densos"
```

---

### Task 12: Tela Detalhe

**Files:**
- Modify: `src/tui/render/detail.rs` (reescrito)

**Interfaces:**
- Consumes: `q.models` (agora populado via `limits[]` — nomes da API), `q.extra` (`ClaudeQuotaExtra.extra_usage` novo), `provider_series_24h`, `ProviderUsage.by_model`, `gauge_spans`, `chips_line`.
- Produces: `pub fn render_detail(state: &AppState, frame: &mut Frame, area: Rect, hits: &mut HitMap)`.

- [ ] **Step 1: Reescrever o render**

Seções (todas alinhadas na mesma coluna de gauge, números à direita):

1. **Janelas**: `sessão`/`semana` de `q.primary`/`q.secondary` +
   uma linha por entrada de `q.models` (nome vindo da API, ex. "Fable").
   Cor: `severity_color_api(w.severity.as_deref(), Some(w.remaining))`.
2. **Modelos hoje** (se `provider_usage.by_model` não-vazio): por modelo,
   barra proporcional de tokens (`gauge_spans(tokens_pct, 20, brand)`),
   tokens abreviados (`14.2M`), custo `$X.XX` right-aligned. Nome truncado
   com `…` se >12 chars (`format!("{:.11}…", name)` quando necessário —
   NUNCA corte seco).
3. **tokens/h 24h**: sparkline larga
   (`sparkline_str_wide(&series, area.width - 24)`) + "pico HHh: N"
   (índice do max da série → hora local).
4. **extra usage**: de `ClaudeQuotaExtra.extra_usage`:
   `enabled=false` → "extra usage  desativado"; `enabled=true` →
   `gauge_spans(100.0 - pct_usado, 20, sev)` + "$used de $limit".
   Providers sem extra → omitir a linha.
5. **Totais**: "hoje 14.2M tok · $429.97    7 dias 29.1M tok · $3,930"
   (hoje de `state.usage`; 7d somando `state.history` do provider).
6. **Chips centrados**: `[esc voltar] [r atualizar] [g login] [h histórico]`
   via `chips_line` + `register_chip_hits`.

O placeholder hardcoded ` tokens/h ▁▂▃▅▇▆▄▂▁` (linhas 209-218 atuais)
MORRE nesta task — apagar junto com o comentário "(T10)".

- [ ] **Step 2: Snapshots**

`detail_claude_full` (models + extra), `detail_amp_credits`,
`detail_codex_logged_out`, `detail_narrow_80`. Run:
`INSTA_UPDATE=always cargo test render::detail`; inspecionar: nenhum
`▁▂▃▅▇▆▄▂▁` idêntico entre providers (era o placeholder), "Free Tie"
não existe mais (truncagem com `…`).

- [ ] **Step 3: Gate + commit**

Run: `cargo test tui` → PASS; `grep -rn "T10" src/` → vazio.

```bash
git add src/tui
git commit -m "feat(tui): detalhe com dados reais e extra usage"
```

---

### Task 13: Tela Histórico (chart braille + tabela)

**Files:**
- Modify: `src/tui/render/history.rs` (reescrito no miolo; buckets já vêm de `usage::buckets`)
- Modify: `src/tui/state.rs`, `src/tui/update.rs` (toggle 24h/7d)

**Interfaces:**
- Consumes: `bucket_by_hour`, `bucket_by_day`, `bucket_by_provider_day`, `AmpDollars` (via `state.usage`), `Chart`/`Dataset`/`Axis` do ratatui.
- Produces:
  - `AppState.history_range: HistoryRange` com `pub enum HistoryRange { Day, Week }` (default Week; tecla `t` alterna → `Action::ToggleHistoryRange`).
  - `pub fn render_history(state: &AppState, frame: &mut Frame, area: Rect, hits: &mut HitMap)`.

- [ ] **Step 1: Teste de update (toggle)**

```rust
#[test]
fn toggle_history_range_flips() {
    let mut state = AppState::new();
    assert_eq!(state.history_range, HistoryRange::Week);
    update(&mut state, Action::ToggleHistoryRange);
    assert_eq!(state.history_range, HistoryRange::Day);
}
```

Run: `cargo test tui::update` → FAIL → implementar enum+action+braço → PASS.

- [ ] **Step 2: Render**

Layout vertical: `[chart Min(10) | tabela Length(n_dias+3) | chips Length(1)]`.

Chart (metade superior):

```rust
let datasets: Vec<Dataset> = ["claude", "codex"] // amp: sem logs locais (nota abaixo)
    .iter()
    .filter_map(|pid| {
        let series: Vec<(f64, f64)> = match state.history_range {
            HistoryRange::Day => bucket_by_hour(records_do_provider, now, 24),
            HistoryRange::Week => bucket_by_hour(records_do_provider, now, 24 * 7),
        }
        .iter()
        .enumerate()
        .map(|(i, b)| (i as f64, b.tokens as f64))
        .collect();
        (!series.iter().all(|(_, y)| *y == 0.0)).then(|| /* leak-free: guardar as séries num Vec fora do closure */ Dataset::default()
            .name(*pid)
            .marker(ratatui::symbols::Marker::Braille)
            .graph_type(GraphType::Area)
            .style(Style::default().fg(hex_to_color(provider_hex(pid))))
            .data(/* &series armazenada */))
    })
    .collect();
```

(Atenção lifetime: `Dataset::data` recebe `&[(f64, f64)]` — materializar as
séries num `Vec<Vec<(f64,f64)>>` no escopo do render antes de montar os
datasets.)

Eixos: X com labels de hora (`00h`, `12h`, `hoje` / dias `seg`…`dom`), Y com
`0` e max abreviado (`fn abbrev_tokens(n: u64) -> String` — "1.2K", "14.2M";
criar em `render/shared` local do arquivo com teste unit). Sem dados → tela
skeleton "coletando histórico…" com spinner (nunca branco).

Tabela (metade inferior) — colunas
`dia | provider | tokens | custo`, uma linha por (dia × provider) dos
`bucket_by_provider_day`, cor da linha = cor de marca do provider; linha
final do Amp: `hoje | amp | – | $spent de $total (saldo cr $remaining)` com
nota `sem logs locais de token` em Comment. Rodapé: total 7d
`Total 7d: 29.1M tokens / $3930.31` right-aligned (formato atual mantido).

Chips: `[t 24h/7d] [r atualizar] [esc voltar]` centrados.

- [ ] **Step 3: Snapshots + smoke**

Snapshots: `history_week`, `history_day`, `history_empty`,
`history_amp_note`. Run `INSTA_UPDATE=always cargo test render::history`;
inspecionar: chart ocupa a metade superior (braille), tabela embaixo, zero
área morta. Smoke tmux com dado real: o chart do Claude deve ter forma
(não blocos repetidos ▂▂▂▂▅▅▅▅ — isso era o esticamento de 7 pontos).

- [ ] **Step 4: Gate + commit**

```bash
git add src/tui
git commit -m "feat(tui): histórico com chart braille e tabela"
```

---

### Task 14: Login + Waybar reskin + help overlay com Clear

**Files:**
- Modify: `src/tui/render/login.rs`, `src/tui/render/config.rs`, `src/tui/render/mod.rs` (help overlay)

**Interfaces:**
- Consumes: `LoginState` (Task 3), `chips_line`, `tui_popup` (dep existente) ou `ratatui::widgets::Clear`.
- Produces: mesmas assinaturas `render_login`/`render_config` com `hits`.

- [ ] **Step 1: Login**

Lista à esquerda (dentro do content — a sidebar global continua): um item
por provider com `LoginState` real:
`◆ Claude   ● ok` / `● Codex   ○ deslogado` / etc. Painel direito:
instrução do provider selecionado (texto atual mantido) + chips
`[↵ iniciar login] [esc voltar]` centrados. Cada item registra
`MouseTarget::Card(i)` (click seleciona; segundo click = Enter).

- [ ] **Step 2: Waybar config**

Manter a lógica de edição (ConfigState/tui-input) intacta; trocar só o skin:
bordas Rounded, título embutido, chips `[↵ editar] [s salvar] [esc voltar]`
centrados no rodapé (substituem o hint-text atual). Nenhuma mudança em
`settings::save`/integração.

- [ ] **Step 3: Help overlay — fix do clipping**

No `render_help_overlay`, ANTES de renderizar o popup:

```rust
let popup_area = centered_rect(60, 70, frame.area()); // helper existente ou criar
frame.render_widget(ratatui::widgets::Clear, popup_area);
```

Conteúdo: atalhos por tela (tabela de 2 colunas tecla/ação) + dica de mouse
("click seleciona · wheel rola · shift+drag seleciona texto").

- [ ] **Step 4: Snapshots + gate + commit**

`INSTA_UPDATE=always cargo test render` — inspecionar
`help_overlay_*.snap`: a tabela por baixo NÃO aparece cortada dentro do
popup (era o bug "pr"/"sto"). Run `cargo test tui` → PASS.

```bash
git add src/tui
git commit -m "feat(tui): login/waybar reskin e help com Clear"
```

---

## Fase 4 — Settings, ícones, motion e fonte

### Task 15: Settings `menu.*` + módulo de ícones

**Files:**
- Modify: `src/settings.rs`
- Create: `src/tui/widgets/icons.rs`
- Modify: `src/tui/widgets/mod.rs`

**Interfaces:**
- Produces:
  - `pub struct MenuSettings { pub animations: bool, pub font_family: String, pub font_size: u32 }` (defaults: `true`, `"IBM Plex Mono"`, `12`); campo `Settings.menu: MenuSettings`; raw deserialize leniente `"menu": {"animations": bool, "fontFamily": str, "fontSize": u32}`.
  - `pub enum Icon { Ok, LoggedOut, Warn, NoToken, Reset, Cost, History, Peak, Refresh, Login, Waybar }` e `pub fn glyph(icon: Icon, mode: GlyphMode) -> &'static str` (reusa `GlyphMode` box/nerd existente).

- [ ] **Step 1: Testes que falham**

`settings.rs` tests (padrão dos testes existentes de load leniente):

```rust
#[test]
fn menu_settings_defaults() {
    let s = load_from_str("{}"); // usar o helper de teste existente do arquivo
    assert!(s.menu.animations);
    assert_eq!(s.menu.font_family, "IBM Plex Mono");
    assert_eq!(s.menu.font_size, 12);
}

#[test]
fn menu_settings_from_json() {
    let s = load_from_str(r#"{"menu":{"animations":false,"fontFamily":"Geist Mono","fontSize":13}}"#);
    assert!(!s.menu.animations);
    assert_eq!(s.menu.font_family, "Geist Mono");
    assert_eq!(s.menu.font_size, 13);
}
```

`icons.rs` tests:

```rust
#[test]
fn nerd_and_box_glyphs_differ_and_are_nonempty() {
    for icon in [Icon::Ok, Icon::LoggedOut, Icon::Warn, Icon::Reset, Icon::Cost,
                 Icon::History, Icon::Peak, Icon::Refresh, Icon::Login, Icon::Waybar, Icon::NoToken] {
        assert!(!glyph(icon, GlyphMode::Nerd).is_empty());
        assert!(!glyph(icon, GlyphMode::Box).is_empty());
    }
    // Nerd usa PUA (>= U+E000); Box nunca:
    assert!(glyph(Icon::Ok, GlyphMode::Nerd).chars().all(|c| c as u32 >= 0xE000));
    assert!(glyph(Icon::Ok, GlyphMode::Box).chars().all(|c| (c as u32) < 0xE000));
}
```

- [ ] **Step 2: Rodar e ver falhar** — `cargo test settings` e `cargo test widgets` → FAIL.

- [ ] **Step 3: Implementar**

`settings.rs`: structs + default + merge no `load` (mesmo padrão leniente
de `RawWaybar`); `save` serializa automático (Serialize derive).

`icons.rs`:

```rust
//! Vocabulário de ícones: Nerd Font (faixa Font Awesome, estável no NF v3)
//! com fallback Unicode universal (GlyphMode::Box).

use crate::settings::GlyphMode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    Ok,
    LoggedOut,
    Warn,
    NoToken,
    Reset,
    Cost,
    History,
    Peak,
    Refresh,
    Login,
    Waybar,
}

pub fn glyph(icon: Icon, mode: GlyphMode) -> &'static str {
    match (icon, mode) {
        (Icon::Ok, GlyphMode::Nerd) => "\u{f00c}",
        (Icon::Ok, GlyphMode::Box) => "✓",
        (Icon::LoggedOut, GlyphMode::Nerd) => "\u{f00d}",
        (Icon::LoggedOut, GlyphMode::Box) => "✗",
        (Icon::Warn, GlyphMode::Nerd) => "\u{f071}",
        (Icon::Warn, GlyphMode::Box) => "!",
        (Icon::NoToken, GlyphMode::Nerd) => "\u{f023}",
        (Icon::NoToken, GlyphMode::Box) => "×",
        (Icon::Reset, GlyphMode::Nerd) => "\u{f017}",
        (Icon::Reset, GlyphMode::Box) => "↻",
        (Icon::Cost, GlyphMode::Nerd) => "\u{f155}",
        (Icon::Cost, GlyphMode::Box) => "$",
        (Icon::History, GlyphMode::Nerd) => "\u{f201}",
        (Icon::History, GlyphMode::Box) => "≡",
        (Icon::Peak, GlyphMode::Nerd) => "\u{f0e7}",
        (Icon::Peak, GlyphMode::Box) => "▲",
        (Icon::Refresh, GlyphMode::Nerd) => "\u{f021}",
        (Icon::Refresh, GlyphMode::Box) => "↻",
        (Icon::Login, GlyphMode::Nerd) => "\u{f090}",
        (Icon::Login, GlyphMode::Box) => "→",
        (Icon::Waybar, GlyphMode::Nerd) => "\u{f013}",
        (Icon::Waybar, GlyphMode::Box) => "⚙",
    }
}
```

Substituir nos renders (Tasks 11-14 files) os literais `↻`/`✓`/`○`/`●` de
STATUS por `glyph(...)` com o `GlyphMode` de `octx.settings.glyph_mode`
(passado ao render via `AppState` — adicionar `pub glyph_mode: GlyphMode`
setado na criação do estado em `run()`). Marcas de provider `◆●` ficam
como estão (identidade, não ícone).

- [ ] **Step 4: Rodar, snapshots, commit**

`cargo test settings` → PASS; `cargo test widgets` → PASS;
`INSTA_UPDATE=always cargo test render` (glyphs box nos snapshots — testes
usam default Box). `cargo test settings` de novo após snapshots.

```bash
git add src/settings.rs src/tui
git commit -m "feat(settings): bloco menu e vocabulário de ícones"
```

---

### Task 16: Motion — tachyonfx + lerps

**Files:**
- Modify: `Cargo.toml` (adicionar `tachyonfx = "0.25"`)
- Create: `src/tui/effects.rs`
- Modify: `src/tui/event_loop.rs`, `src/tui/update.rs`, `src/tui/state.rs`, `src/tui/render/dashboard.rs` (pulse), `src/tui/render/mod.rs` (count-up no header)

**Interfaces:**
- Produces:
  - `pub enum FxEvent { ScreenChanged, FetchLanded }` e `AppState.fx_queue: Vec<FxEvent>` (update empurra; event_loop drena — update segue puro).
  - `pub struct Effects` em `effects.rs` com `pub fn new(enabled: bool) -> Self`, `pub fn on_event(&mut self, ev: FxEvent, content_area: Rect)`, `pub fn process(&mut self, elapsed: std::time::Duration, buf: &mut Buffer, area: Rect)`.
  - `AppState.display_cost: f64` (count-up lerp do custo do header).
- Consumes: `settings.menu.animations`.

- [ ] **Step 1: Testes que falham (update: fila de eventos + lerp)**

```rust
#[test]
fn screen_change_pushes_fx_event() {
    let mut state = AppState::new();
    update(&mut state, Action::Activate(SidebarItem::History));
    assert!(state.fx_queue.contains(&FxEvent::ScreenChanged));
}

#[test]
fn fetch_completed_pushes_fetch_landed() {
    let mut state = AppState::new();
    update(&mut state, Action::FetchCompleted { fetched_at: "2026-07-01T18:00:00.000Z".into() });
    assert!(state.fx_queue.contains(&FxEvent::FetchLanded));
}

#[test]
fn anim_tick_lerps_display_cost_toward_target() {
    let mut state = AppState::new();
    state.usage = Some(test_usage_summary_with_cost(100.0)); // helper: UsageSummary com total_cost.usd
    state.display_cost = 0.0;
    update(&mut state, Action::AnimTick);
    assert!(state.display_cost > 0.0 && state.display_cost < 100.0);
    for _ in 0..400 { update(&mut state, Action::AnimTick); }
    assert!((state.display_cost - 100.0).abs() < 0.5);
}
```

- [ ] **Step 2: Rodar e ver falhar** — `cargo test tui::update` → FAIL.

- [ ] **Step 3: Implementar**

`state.rs`: `pub fx_queue: Vec<FxEvent>`, `pub display_cost: f64`.
`update.rs`: nos braços `Activate` (quando `screen` muda) push
`ScreenChanged`; em `FetchCompleted` push `FetchLanded`; em `AnimTick`:

```rust
Action::AnimTick => {
    state.anim_frame = state.anim_frame.wrapping_add(1);
    state.throbber.advance();
    // lerp existente de display_ratio dos gauges permanece;
    // count-up do custo (ease exponencial ~800ms em ticks de 30ms):
    let target = state.usage.as_ref().map(|u| u.total_cost.usd).unwrap_or(0.0);
    state.display_cost += (target - state.display_cost) * 0.12;
    if (target - state.display_cost).abs() < 0.01 {
        state.display_cost = target;
    }
    vec![]
}
```

`effects.rs`:

```rust
//! Efeitos tachyonfx: coalesce na troca de tela, sweep no fetch.
//! `enabled=false` (menu.animations) → tudo vira no-op.

use std::time::Duration;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use tachyonfx::{fx, EffectManager, Interpolation, Motion};

use super::state::FxEvent;

pub struct Effects {
    manager: EffectManager<()>,
    enabled: bool,
}

impl Effects {
    pub fn new(enabled: bool) -> Self {
        Self {
            manager: EffectManager::default(),
            enabled,
        }
    }

    pub fn on_event(&mut self, ev: FxEvent, _content_area: Rect) {
        if !self.enabled {
            return;
        }
        match ev {
            FxEvent::ScreenChanged => self
                .manager
                .add_effect(fx::coalesce((280, Interpolation::SineOut))),
            FxEvent::FetchLanded => self.manager.add_effect(fx::sweep_in(
                Motion::LeftToRight,
                10,
                0,
                ratatui::style::Color::Black,
                (900, Interpolation::QuadOut),
            )),
        }
    }

    pub fn process(&mut self, elapsed: Duration, buf: &mut Buffer, area: Rect) {
        if !self.enabled {
            return;
        }
        self.manager.process_effects(elapsed.into(), buf, area);
    }
}
```

(Conferir assinaturas exatas contra a doc do tachyonfx 0.25 —
`ctx7 docs /ratatui/tachyonfx "fx::coalesce fx::sweep_in signature"` — os
params de sweep_in são (motion, depth, randomness, color, timer); ajustar
se a minor divergir.)

`event_loop.rs`: criar `let mut effects = Effects::new(octx.settings.menu.animations);`
+ `let mut last_frame = std::time::Instant::now();`; no draw:

```rust
terminal.draw(|f| {
    hits.clear();
    render(&state, f, &mut hits);
    let area = f.area();
    let elapsed = last_frame.elapsed();
    effects.process(elapsed, f.buffer_mut(), area);
})?;
last_frame = std::time::Instant::now();
for ev in state.fx_queue.drain(..) {
    effects.on_event(ev, terminal_content_area);
}
```

Pulse crítico (sem tachyonfx — determinístico p/ snapshot): em
`gauge_spans` call sites onde `remaining < 10.0` e `animations` on, modular
a cor com `anim_frame`:

```rust
/// Pulso: oscila brilho 0.75→1.45 num ciclo de ~1.1s (37 ticks de 30ms).
fn pulse_color(base: Color, anim_frame: u64) -> Color {
    let phase = (anim_frame % 37) as f64 / 37.0;
    let s = 0.75 + 0.70 * (phase * std::f64::consts::TAU).sin().abs();
    scale_rgb(base, s) // multiplica os canais, clamp 255 — helper no quota_gauge.rs
}
```

Header count-up: render usa `state.display_cost` em vez de
`usage.total_cost.usd` direto.

Snapshots: testes de render usam `animations=false`/`anim_frame=0` →
determinísticos (sem mudança pelos efeitos, que agem só no buffer em
runtime).

- [ ] **Step 4: Rodar e ver passar + smoke**

`cargo test tui` → PASS. Smoke tmux: abrir menu → sweep no load; navegar
Geral↔Histórico → coalesce; setar `"menu":{"animations":false}` num
`XDG_CONFIG_HOME` temporário → estático.

- [ ] **Step 5: Gate + commit**

```bash
git add src/tui Cargo.toml Cargo.lock
git commit -m "feat(tui): motion tachyonfx e count-up"
```

---

### Task 17: Fonte do menu — CLI `menu-font` + helper

**Files:**
- Modify: `src/cli.rs` (subcomando oculto `menu-font`)
- Modify: `src/main.rs` (dispatch)
- Modify: `scripts/agent-bar-open-terminal` (Bash — permanece Bash!)

**Interfaces:**
- Produces:
  - `agent-bar menu-font` imprime `IBM Plex Mono<TAB>12\n` em stdout (família e size das settings) e sai 0. Não aparece no help (comando interno do helper).
- Consumes: `settings.menu.font_family` / `font_size` (Task 15).

- [ ] **Step 1: Teste de CLI que falha**

No padrão dos testes de `cli.rs`:

```rust
#[test]
fn menu_font_parses_as_command() {
    let opts = parse_args(&args(&["menu-font"])).unwrap();
    assert_eq!(opts.command, Command::MenuFont);
}
```

E teste de integração (assert_cmd, padrão dos testes de binário existentes):

```rust
#[test]
fn menu_font_prints_family_and_size() {
    // XDG_CONFIG_HOME temp com settings {"menu":{"fontFamily":"Geist Mono","fontSize":13}}
    // agent-bar menu-font → stdout == "Geist Mono\t13\n", exit 0.
}
```

- [ ] **Step 2: Rodar e ver falhar** — `cargo test cli` → FAIL.

- [ ] **Step 3: Implementar CLI**

`Command::MenuFont` no enum; parse da string `"menu-font"`; no dispatch:

```rust
Command::MenuFont => {
    let settings = settings::load(&paths);
    println!("{}\t{}", settings.menu.font_family, settings.menu.font_size);
}
```

(stdout limpo: exatamente uma linha TSV — é contrato do helper.)

- [ ] **Step 4: Helper Bash**

Em `scripts/agent-bar-open-terminal`, antes da cascata de terminais:

```bash
# Fonte do menu (settings menu.fontFamily/fontSize) — best-effort:
# se o binário não responder, cai na fonte padrão do terminal.
font_family=""
font_size=""
if command -v agent-bar >/dev/null 2>&1; then
  if font_spec="$(agent-bar menu-font 2>/dev/null)"; then
    font_family="${font_spec%%	*}"
    font_size="${font_spec##*	}"
  fi
fi
```

E na cascata (ANTES do caminho uwsm/xdg genérico — alacritty direto
preserva o float do Hyprland via `--class` e ganha a fonte):

```bash
# 1) Alacritty direto (Omarchy incluso): fonte configurável + classe p/ float.
if command -v alacritty >/dev/null 2>&1; then
  font_opts=()
  if [ -n "$font_family" ]; then
    font_opts+=(--option "font.normal.family=\"$font_family\"")
    [ -n "$font_size" ] && font_opts+=(--option "font.size=$font_size")
  fi
  exec setsid alacritty --class org.omarchy.terminal --title "Agent Bar" \
    "${font_opts[@]}" -e bash -lc "$cmd"
fi

# 2) kitty / foot / ghostty / wezterm com flag de fonte:
if command -v kitty >/dev/null 2>&1; then
  font_opts=()
  [ -n "$font_family" ] && font_opts+=(-o "font_family=$font_family")
  [ -n "$font_size" ] && font_opts+=(-o "font_size=$font_size")
  exec setsid kitty --title "Agent Bar" "${font_opts[@]}" bash -lc "$cmd"
fi
if command -v foot >/dev/null 2>&1; then
  if [ -n "$font_family" ]; then
    exec setsid foot -T "Agent Bar" -o "main.font=$font_family:size=${font_size:-12}" bash -lc "$cmd"
  fi
  exec setsid foot -T "Agent Bar" bash -lc "$cmd"
fi
if command -v ghostty >/dev/null 2>&1; then
  font_opts=()
  [ -n "$font_family" ] && font_opts+=("--font-family=$font_family")
  [ -n "$font_size" ] && font_opts+=("--font-size=${font_size:-12}")
  exec setsid ghostty --title="Agent Bar" "${font_opts[@]}" -e bash -lc "$cmd"
fi

# 3) uwsm/xdg genérico (sem suporte a fonte — fallback documentado):
#    ... cascata atual inalterada ...
```

(O bloco atual de alacritty/kitty/foot/wezterm é SUBSTITUÍDO pelos acima;
wezterm mantém o launch atual sem fonte — `--config font=...` tem sintaxe
Lua frágil; documentar como não-suportado.)

- [ ] **Step 5: Validar**

Run: `cargo test cli` → PASS.
Run: `bash -n scripts/agent-bar-open-terminal` → sintaxe ok.
Run: `shellcheck scripts/agent-bar-open-terminal 2>/dev/null | head -20` →
sem erro novo (warnings pré-existentes toleram).
Smoke manual (NÃO roda `setup`/`update`): executar
`scripts/agent-bar-open-terminal agent-bar menu` — abre janela flutuante
com IBM Plex Mono (se instalada: `fc-list | grep -i "IBM Plex Mono"`).

- [ ] **Step 6: Commit**

```bash
git add src/cli.rs src/main.rs scripts/agent-bar-open-terminal
git commit -m "feat(menu): fonte configurável via menu-font"
```

---

### Task 18: Limpeza de código morto + gate integral

**Files:**
- Modify: `src/tui/update.rs` (apagar `key_to_action` morto, linhas 8-22)
- Modify: `src/tui/widgets/sparkline.rs` (doc-comment mente sobre "wraps ratatui Sparkline" — corrigir para descrever a implementação manual)
- Modify: `src/tui/login_spawn.rs` (comentário na linha 10 referencia `login.ts` inexistente — trocar por referência ao spec; verificar comando `codex auth login` contra `codex --help` real e corrigir se divergir)
- Delete: snapshots órfãos de telas antigas (`src/tui/render/snapshots/*.snap` sem teste correspondente — `cargo insta test --unreferenced=delete` se cargo-insta disponível; senão comparar nomes de snap × testes na mão)
- Modify: `docs/commands.md` e `docs/runtime.md` (seção do menu: navegação por sidebar/mouse, settings `menu.*`)

**Steps:**

- [ ] **Step 1: Apagar mortos e corrigir comentários**

`grep -rn "key_to_action\b" src/` → só a definição → apagar. Confirmar
`grep -rn "block_bar\|quota_gauge_line\|window_gauge_line\|model_gauge_line" src/`
→ vazio (Task 7 os removeu). Corrigir os dois comentários citados.
Verificação do login do codex: `codex auth login --help 2>&1 | head -3`
(local, read-only) — se o subcomando divergir, corrigir a string em
`login_spawn.rs:77-78` E anotar no CHANGELOG interno da PR (não no
CHANGELOG.md — só em release).

- [ ] **Step 2: Docs**

`docs/commands.md`: atualizar a descrição de `menu` (sidebar, mouse,
teclas, `menu-font`). `docs/runtime.md`: settings novas
(`menu.animations`, `menu.fontFamily`, `menu.fontSize`) + nota do fallback
de fonte no caminho xdg.

- [ ] **Step 3: Gate integral (matriz completa do CLAUDE.md §2)**

Run (cada um separado, RTK-safe):
- `cargo test` → PASS integral.
- `cargo clippy --all-targets -- -D warnings` → limpo.
- `cargo test --test golden` → PASS (contrato Waybar intacto).
- `cargo test waybar_contract` → PASS.
- `git diff --check` → limpo.
Smoke final tmux 120x35 + 78x24 (sidebar colapsada): dashboard, detalhe,
histórico, login, waybar, help — capturar e inspecionar cada um.

- [ ] **Step 4: Commit final**

```bash
git add -A
git commit -m "chore(tui): limpeza de código morto e docs"
```

---

## Self-Review (executado na escrita do plano)

1. **Spec coverage:** §3 runtime → Tasks 4-6; §4.1 limits/spend → Task 1;
   §4.2 histórico horário → Task 2 (Amp: refinamento registrado no topo);
   §4.3 login → Tasks 3+6; §5 navegação/mouse → Tasks 8-9; §6 telas →
   Tasks 10-14; §7 visual/ícones/fonte → Tasks 7, 15, 17; §8 motion →
   Task 16; §10 estados → distribuído (skeleton Task 11, narrow Task 10,
   NO_COLOR preservado); §11 testes → por task; limpeza §1/§13 → Task 18.
2. **Placeholders:** nenhum TBD; os dois pontos "copiar padrão existente"
   (wiremock setup Task 1, InitConfig placeholder Task 8) apontam para
   código concreto existente no mesmo arquivo — aceitável para implementer
   com acesso ao repo.
3. **Type consistency:** `gauge_spans(f64, usize, Color)` usado em 11/12/16;
   `login_state_for(Option<&ProviderQuota>, bool)` em 3/11/14;
   `spawn_fetch(&UnboundedSender<Action>, OwnedCtx, Option<String>)` em
   5/6; `chips_line(&[(ChipKind, &str, &str)], u16)` em 10/12/13/14;
   `HitMap.at(u16,u16) -> Option<MouseTarget>` em 9/10/11. Consistentes.
