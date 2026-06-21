# Plano 04c — Codex provider (app-server JSON-RPC + fallback session-log)

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:subagent-driven-development. Steps usam checkbox (`- [ ]`).

**Goal:** Terceiro e mais complexo provider (Codex), estendendo `QuotaSource`: handshake JSON-RPC sobre stdio com o `codex app-server`, com fallback para o session-log `.jsonl`, normalizando para `CodexRateLimits` e construindo `ProviderQuota` byte-exact com o TS.

**Architecture:** Codex tem duas fontes: (1) `codex app-server` (JSON-RPC via stdio: `initialize`→`initialized`+`account/read`+`account/rateLimits/read`, respostas fora de ordem com grace de 200ms, timeout externo 4s); (2) fallback: maior-mtime `.jsonl` em `~/.codex/sessions/YYYY/MM/DD` (hoje/ontem), scan reverso por evento `token_count`. Ambas produzem `CodexRateLimits` (formato interno snake_case = o do session-log), que é o `QuotaSource::Raw` cacheável. `build_quota` é puro. O protocolo é escrito sobre `AsyncRead+AsyncWrite` para ser testado com `tokio::io::duplex` (sem spawn real).

**Tech Stack:** tokio (`process`, `io-util`, `time`, `select!`), serde_json, time (`local_offset`), indexmap. Port fiel de `src/providers/codex.ts`.

## Global Constraints

- **Byte-exact com o TS é SAGRADO. Autoridade = `src/providers/codex.ts` + `tests/providers/codex.test.ts` + `tests/providers/codex-appserver.test.ts`.** Rejeitar "fix" de review que divergiria do TS.
- **Sem `unwrap()`/`expect()` em produção** (deny lint). `#[cfg(test)]` permitido. Nunca `!`. `unwrap_or`/`.ok()`/`let _ =`/`?` permitidos.
- **stdout limpo** (logs só `log::*`).
- **Strings de erro CONTRATO** (já em `error.rs`): `CodexError::{NotLoggedIn, NoSessionData("No session data found"), NoRateLimitData("No rate limit data found (app-server + session log)"), NoQuotaWindows("No quota windows found"), Generic("Failed to fetch Codex usage")}`.
- `ProviderQuota` serialize-only; o cache faz round-trip de `CodexRateLimits` (Serialize+Deserialize).
- **Ordem de mapas:** `CodexRateLimits.buckets` = `IndexMap` (ordem de inserção afeta o sufixo de dedup, como o `Record` do TS); raw app-server `rate_limits_by_limit_id` = `IndexMap`. **`models_detailed` (CodexQuotaExtra) fica `BTreeMap`** (o view-model re-ordena por severity — decisão 04a). `models` (ProviderQuota, flatten) é IndexMap (04a).
- **Verificação:** `cargo test --manifest-path rust/Cargo.toml` + `cargo clippy ... --all-targets -- -D warnings`. RTK: `cargo test: N passed` (sem `test result:`; ler `... 2>&1 | tail -8`); só UM filtro posicional. `cargo fmt` ANTES de `git add`. Read antes de Edit. Commits PT ≤50 chars. NÃO tocar main.rs/docs do projeto.

---

## File Structure

- `rust/src/providers/codex.rs` (criar, ao longo de T1-T5) — types, helpers, normalize, fallback, protocol, `CodexProvider`.
- `rust/src/providers/mod.rs` (modificar) — `pub mod codex;` (T1); `registry()` ganha Codex (T5).

---

### Task 1: Types + `build_quota` (CodexRateLimits → ProviderQuota)

**Files:** Create `rust/src/providers/codex.rs`; Modify `rust/src/providers/mod.rs` (`pub mod codex;`).

**Interfaces:**
- Produces: tipos `CodexWindowRaw`/`CodexLimitBucket`/`CodexCredits`/`CodexRateLimits` (snake_case, Serialize+Deserialize); helpers `unix_to_iso`, `to_quota_window`, `format_bucket_label`, `build_model_windows`, `flatten_models`, `pick_primary`, `pick_secondary`; `build_codex_quota(limits, base) -> ProviderQuota`.

- [ ] **Step 1: Criar `codex.rs` com types + helpers de parse**

```rust
//! Codex provider. Estende `QuotaSource`. Duas fontes (app-server JSON-RPC +
//! fallback session-log) normalizadas para `CodexRateLimits`. Port fiel de
//! `src/providers/codex.ts`.

use std::collections::BTreeMap;

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use super::iso_from_ms;
use super::types::{
    CodexQuotaExtra, ExtraUsage, ModelWindows, ProviderExtra, ProviderQuota, QuotaWindow,
};
use crate::formatters::shared::{classify_window, normalize_plan, WindowKind};

// ---- Formato interno (snake_case = formato do session-log; é o Raw cacheável) ----

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexWindowRaw {
    pub used_percent: f64,
    pub window_minutes: i64,
    pub resets_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexLimitBucket {
    pub limit_id: String,
    #[serde(default)]
    pub limit_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub primary: Option<CodexWindowRaw>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secondary: Option<CodexWindowRaw>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexCredits {
    pub has_credits: bool,
    pub unlimited: bool,
    pub balance: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CodexRateLimits {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub primary: Option<CodexWindowRaw>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secondary: Option<CodexWindowRaw>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credits: Option<CodexCredits>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub buckets: Option<IndexMap<String, CodexLimitBucket>>,
}

// ---- Helpers de conversão (puros) ----

/// Unix SEGUNDOS → ISO UTC; None se `<= 0`.
fn unix_to_iso(ts: i64) -> Option<String> {
    if ts <= 0 {
        None
    } else {
        Some(iso_from_ms((ts as u64) * 1000))
    }
}

/// CodexWindowRaw → QuotaWindow (remaining = 100 - round(used_percent)).
fn to_quota_window(raw: &CodexWindowRaw) -> QuotaWindow {
    QuotaWindow {
        remaining: 100.0 - raw.used_percent.round(),
        resets_at: unix_to_iso(raw.resets_at),
        window_minutes: Some(raw.window_minutes),
        used: None,
    }
}

/// `limit_name` (não-vazio) ou `limit_id`; `[_-]+`→espaço; titlecase por palavra; vazio→"Codex".
fn format_bucket_label(bucket: &CodexLimitBucket) -> String {
    let raw = bucket
        .limit_name
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(&bucket.limit_id);
    let normalized: String = raw
        .chars()
        .map(|c| if c == '_' || c == '-' { ' ' } else { c })
        .collect();
    let normalized = normalized.trim();
    if normalized.is_empty() {
        return "Codex".to_string();
    }
    normalized
        .split_whitespace()
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Insere `qw` no kind certo de `windows` (fiveHour/sevenDay únicos; resto em other).
fn place_window(windows: &mut ModelWindows, raw: &CodexWindowRaw) {
    let qw = to_quota_window(raw);
    match classify_window(Some(raw.window_minutes)) {
        WindowKind::FiveHour if windows.five_hour.is_none() => windows.five_hour = Some(qw),
        WindowKind::SevenDay if windows.seven_day.is_none() => windows.seven_day = Some(qw),
        _ => windows.other.get_or_insert_with(Vec::new).push(qw),
    }
}

/// Constrói `modelsDetailed` a partir dos buckets (ou fallback legacy primary/secondary).
fn build_model_windows(limits: &CodexRateLimits) -> BTreeMap<String, ModelWindows> {
    let mut models: BTreeMap<String, ModelWindows> = BTreeMap::new();

    if let Some(buckets) = limits.buckets.as_ref().filter(|b| !b.is_empty()) {
        for bucket in buckets.values() {
            let mut windows = ModelWindows::default();
            for raw in [bucket.primary.as_ref(), bucket.secondary.as_ref()]
                .into_iter()
                .flatten()
            {
                place_window(&mut windows, raw);
            }
            // Fallback de mapeamento quando as durações não classificam limpo.
            if windows.five_hour.is_none() {
                if let Some(p) = bucket.primary.as_ref() {
                    windows.five_hour = Some(to_quota_window(p));
                }
            }
            if windows.seven_day.is_none() {
                if let Some(s) = bucket.secondary.as_ref() {
                    windows.seven_day = Some(to_quota_window(s));
                }
            }
            if windows.five_hour.is_none()
                && windows.seven_day.is_none()
                && windows.other.as_ref().map(Vec::is_empty).unwrap_or(true)
            {
                continue;
            }
            let base_name = format_bucket_label(bucket);
            let mut name = base_name.clone();
            let mut suffix = 2;
            while models.contains_key(&name) {
                name = format!("{base_name} ({suffix})");
                suffix += 1;
            }
            models.insert(name, windows);
        }
    }

    // Legacy: só primary/secondary, sem buckets.
    if models.is_empty() && (limits.primary.is_some() || limits.secondary.is_some()) {
        let mut windows = ModelWindows::default();
        for raw in [limits.primary.as_ref(), limits.secondary.as_ref()]
            .into_iter()
            .flatten()
        {
            place_window(&mut windows, raw);
        }
        if windows.five_hour.is_none() {
            if let Some(p) = limits.primary.as_ref() {
                windows.five_hour = Some(to_quota_window(p));
            }
        }
        if windows.seven_day.is_none() {
            if let Some(s) = limits.secondary.as_ref() {
                windows.seven_day = Some(to_quota_window(s));
            }
        }
        models.insert("Codex".to_string(), windows);
    }

    models
}

fn flatten_models(models_detailed: &BTreeMap<String, ModelWindows>) -> IndexMap<String, QuotaWindow> {
    let mut models: IndexMap<String, QuotaWindow> = IndexMap::new();
    for (name, w) in models_detailed {
        let selected = w
            .five_hour
            .clone()
            .or_else(|| w.seven_day.clone())
            .or_else(|| w.other.as_ref().and_then(|o| o.first().cloned()));
        if let Some(qw) = selected {
            models.insert(name.clone(), qw);
        }
    }
    models
}

fn pick_primary(
    limits: &CodexRateLimits,
    models_detailed: &BTreeMap<String, ModelWindows>,
) -> Option<QuotaWindow> {
    if let Some(p) = limits.primary.as_ref() {
        return Some(to_quota_window(p));
    }
    for m in models_detailed.values() {
        if let Some(fh) = m.five_hour.as_ref() {
            return Some(fh.clone());
        }
    }
    for m in models_detailed.values() {
        if let Some(sd) = m.seven_day.as_ref() {
            return Some(sd.clone());
        }
    }
    None
}

fn pick_secondary(
    limits: &CodexRateLimits,
    models_detailed: &BTreeMap<String, ModelWindows>,
) -> Option<QuotaWindow> {
    if let Some(s) = limits.secondary.as_ref() {
        return Some(to_quota_window(s));
    }
    for m in models_detailed.values() {
        if let Some(sd) = m.seven_day.as_ref() {
            return Some(sd.clone());
        }
    }
    None
}

/// CodexRateLimits → ProviderQuota. `error` embutido se sem janelas usáveis.
pub fn build_codex_quota(limits: &CodexRateLimits, base: ProviderQuota) -> ProviderQuota {
    let models_detailed = build_model_windows(limits);
    let models = flatten_models(&models_detailed);
    let primary = pick_primary(limits, &models_detailed);
    let secondary = pick_secondary(limits, &models_detailed);

    if primary.is_none() && secondary.is_none() && models_detailed.is_empty() {
        return ProviderQuota {
            error: Some(crate::providers::error::CodexError::NoQuotaWindows.to_string()),
            ..base
        };
    }

    // Credits → extraUsage.
    let credits_extra: Option<ExtraUsage> = limits.credits.as_ref().and_then(|c| {
        let balance: f64 = c.balance.parse().unwrap_or(0.0);
        if c.has_credits || balance > 0.0 {
            Some(ExtraUsage {
                enabled: true,
                remaining: if c.unlimited { 100.0 } else { 100.0_f64.min(balance.round()) },
                limit: if c.unlimited { -1.0 } else { 0.0 },
                used: 0.0,
            })
        } else {
            None
        }
    });

    let extra = if !models_detailed.is_empty() || credits_extra.is_some() {
        Some(ProviderExtra::Codex(CodexQuotaExtra {
            models_detailed: if models_detailed.is_empty() {
                None
            } else {
                Some(models_detailed)
            },
            extra_usage: credits_extra,
        }))
    } else {
        None
    };

    let plan = normalize_plan(limits.plan_type.as_deref());

    ProviderQuota {
        available: true,
        primary,
        secondary,
        models: if models.is_empty() { None } else { Some(models) },
        plan_type: limits.plan_type.clone(),
        plan,
        extra,
        ..base
    }
}
```

**Nota:** `ModelWindows` (em types.rs) tem `five_hour`/`seven_day`/`other`; é `Default`. Confirme os nomes dos campos com um `grep` em `rust/src/providers/types.rs` antes de codar; ajuste se diferir.

- [ ] **Step 2: Registrar módulo** — em `mod.rs`, `pub mod codex;` (após `pub mod claude;`).

- [ ] **Step 3: Testes de `build_codex_quota`** — porte de `tests/providers/codex.test.ts` (describe blocks: primary/secondary, window classification, multiple buckets, legacy fallback, plan mapping, credits, primary/secondary selection, edge "No quota windows found"). Construa `CodexRateLimits` direto (sem fetch). Asserções-chave (verbatim do TS):
  - used_percent 40 → primary.remaining 60; windowMinutes propagado; resets_at 0 → None; 100→0; 0→100.
  - 1 bucket (primary 300min) → models_detailed["Default"] com five_hour; flatten → models["Default"].
  - buckets com labels colidindo → "(2)" suffix; limit_id quando limit_name null.
  - legacy (só primary/secondary) → models_detailed["Codex"].
  - plan_type "enterprise"→plan "Enterprise"; null → plan/plan_type omitidos.
  - credits has_credits+balance "12.5" → extra_usage{enabled, remaining: min(100,round(12.5))=13, limit 0}; unlimited → remaining 100 limit -1; has_credits false & balance "0" → sem extra_usage.
  - sem janelas usáveis → error "No quota windows found".

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> ProviderQuota {
        ProviderQuota {
            provider: "codex".into(),
            display_name: "Codex".into(),
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

    fn win(used: f64, mins: i64, resets: i64) -> CodexWindowRaw {
        CodexWindowRaw { used_percent: used, window_minutes: mins, resets_at: resets }
    }

    fn codex_extra(q: &ProviderQuota) -> &CodexQuotaExtra {
        match q.extra.as_ref() {
            Some(ProviderExtra::Codex(c)) => c,
            _ => panic!("expected Codex extra"),
        }
    }

    #[test]
    fn primary_used_40_remaining_60_with_window() {
        let limits = CodexRateLimits { primary: Some(win(40.0, 300, 0)), ..Default::default() };
        let q = build_codex_quota(&limits, base());
        assert!(q.available);
        assert_eq!(q.primary.as_ref().unwrap().remaining, 60.0);
        assert_eq!(q.primary.as_ref().unwrap().window_minutes, Some(300));
        assert!(q.primary.as_ref().unwrap().resets_at.is_none()); // resets 0
    }

    #[test]
    fn no_usable_data_errors() {
        let limits = CodexRateLimits::default();
        let q = build_codex_quota(&limits, base());
        assert_eq!(q.error.as_deref(), Some("No quota windows found"));
    }

    #[test]
    fn plan_type_enterprise_maps() {
        let limits = CodexRateLimits {
            primary: Some(win(10.0, 300, 0)),
            plan_type: Some("enterprise".into()),
            ..Default::default()
        };
        let q = build_codex_quota(&limits, base());
        assert_eq!(q.plan.as_deref(), Some("Enterprise"));
        assert_eq!(q.plan_type.as_deref(), Some("enterprise"));
    }

    #[test]
    fn credits_capped_and_unlimited() {
        let limits = CodexRateLimits {
            primary: Some(win(10.0, 300, 0)),
            credits: Some(CodexCredits { has_credits: true, unlimited: false, balance: "250".into() }),
            ..Default::default()
        };
        let q = build_codex_quota(&limits, base());
        let eu = codex_extra(&q).extra_usage.as_ref().unwrap();
        assert_eq!(eu.remaining, 100.0); // min(100, 250)
        assert_eq!(eu.limit, 0.0);

        let limits2 = CodexRateLimits {
            primary: Some(win(10.0, 300, 0)),
            credits: Some(CodexCredits { has_credits: false, unlimited: true, balance: "0".into() }),
            ..Default::default()
        };
        let eu2 = codex_extra(&build_codex_quota(&limits2, base())).extra_usage.clone().unwrap();
        assert_eq!(eu2.remaining, 100.0);
        assert_eq!(eu2.limit, -1.0);
    }

    #[test]
    fn dedup_bucket_names_with_suffix() {
        let mut buckets = IndexMap::new();
        buckets.insert("a".to_string(), CodexLimitBucket {
            limit_id: "a".into(), limit_name: Some("gpt".into()),
            primary: Some(win(20.0, 300, 0)), secondary: None,
        });
        buckets.insert("b".to_string(), CodexLimitBucket {
            limit_id: "b".into(), limit_name: Some("gpt".into()),
            primary: Some(win(30.0, 300, 0)), secondary: None,
        });
        let limits = CodexRateLimits { buckets: Some(buckets), ..Default::default() };
        let q = build_codex_quota(&limits, base());
        let md = codex_extra(&q).models_detailed.as_ref().unwrap();
        assert!(md.contains_key("Gpt"));
        assert!(md.contains_key("Gpt (2)"));
    }
}
```
(Porte os demais casos do codex.test.ts — classificação de janela, legacy, limit_id-quando-name-null, selection.)

- [ ] **Step 4: Verificar** — `cargo test ... codex 2>&1 | tail -8`; `cargo clippy ... -D warnings`.
- [ ] **Step 5: Commit** — `cargo fmt`; `git commit -m "feat(rust): build_quota do Codex"`.

---

### Task 2: Normalização do app-server (camelCase → CodexRateLimits)

**Files:** Modify `rust/src/providers/codex.rs`.

**Interfaces:**
- Produces: tipos app-server (camelCase, Deserialize): `CodexAppServerWindow`, `CodexAppServerLimitBucket`, `CodexAppServerRateLimitsReadResult`, `CodexAppServerAccountReadResult`; `normalize_appserver_rate_limits(raw, account_plan_type) -> Option<CodexRateLimits>`.

- [ ] **Step 1: Tipos app-server + normalize** — porte de `codex.ts` `toRawWindow`/`normalizeBucket`/`normalizeAppServerRateLimits` (linhas ~37-231). Pontos: `toRawWindow(w, fallback) = {used_percent: w.usedPercent, window_minutes: w.windowDurationMins ?? fallback, resets_at: w.resetsAt ?? 0}`; `normalizeBucket(raw, fallbackId)` primary fallback 300, secondary 10080, None se ambos None; `normalizeAppServerRateLimits` constrói `buckets` (IndexMap) de `rateLimitsByLimitId` + root `rateLimits` (id fallback "codex"); primary/secondary = root ?? primeiro bucket; credits `{has_credits: hasCredits, unlimited, balance: balance ?? "0"}`; `plan_type = accountPlanType ?? raw.planType ?? root.planType ?? None`; retorna None se sem primary/secondary E buckets vazio.

```rust
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexAppServerWindow {
    pub used_percent: f64,
    #[serde(default)]
    pub window_duration_mins: Option<i64>,
    #[serde(default)]
    pub resets_at: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexAppServerLimitBucket {
    #[serde(default)]
    pub limit_id: Option<String>,
    #[serde(default)]
    pub limit_name: Option<String>,
    #[serde(default)]
    pub primary: Option<CodexAppServerWindow>,
    #[serde(default)]
    pub secondary: Option<CodexAppServerWindow>,
    #[serde(default)]
    pub plan_type: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexAppServerCredits {
    pub has_credits: bool,
    pub unlimited: bool,
    #[serde(default)]
    pub balance: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexAppServerRateLimitsReadResult {
    #[serde(default)]
    pub rate_limits: Option<CodexAppServerLimitBucket>,
    #[serde(default)]
    pub rate_limits_by_limit_id: Option<IndexMap<String, CodexAppServerLimitBucket>>,
    #[serde(default)]
    pub credits: Option<CodexAppServerCredits>,
    #[serde(default)]
    pub plan_type: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexAppServerAccount {
    #[serde(default)]
    pub plan_type: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct CodexAppServerAccountReadResult {
    #[serde(default)]
    pub account: Option<CodexAppServerAccount>,
}

fn to_raw_window(raw: &CodexAppServerWindow, fallback_minutes: i64) -> CodexWindowRaw {
    CodexWindowRaw {
        used_percent: raw.used_percent,
        window_minutes: raw.window_duration_mins.unwrap_or(fallback_minutes),
        resets_at: raw.resets_at.unwrap_or(0),
    }
}

fn normalize_bucket(raw: &CodexAppServerLimitBucket, fallback_id: Option<&str>) -> Option<CodexLimitBucket> {
    let limit_id = raw.limit_id.clone().or_else(|| fallback_id.map(str::to_string))?;
    let primary = raw.primary.as_ref().map(|w| to_raw_window(w, 300));
    let secondary = raw.secondary.as_ref().map(|w| to_raw_window(w, 10080));
    if primary.is_none() && secondary.is_none() {
        return None;
    }
    Some(CodexLimitBucket { limit_id, limit_name: raw.limit_name.clone(), primary, secondary })
}

pub fn normalize_appserver_rate_limits(
    raw: &CodexAppServerRateLimitsReadResult,
    account_plan_type: Option<&str>,
) -> Option<CodexRateLimits> {
    let mut buckets: IndexMap<String, CodexLimitBucket> = IndexMap::new();
    if let Some(by_id) = raw.rate_limits_by_limit_id.as_ref() {
        for (limit_id, bucket) in by_id {
            if let Some(n) = normalize_bucket(bucket, Some(limit_id)) {
                buckets.insert(n.limit_id.clone(), n);
            }
        }
    }
    let root = raw.rate_limits.as_ref();
    let root_bucket = root.and_then(|r| {
        let fallback = r.limit_id.as_deref().unwrap_or("codex");
        normalize_bucket(r, Some(fallback))
    });
    if let Some(rb) = root_bucket.as_ref() {
        if !buckets.contains_key(&rb.limit_id) {
            buckets.insert(rb.limit_id.clone(), rb.clone());
        }
    }

    let first = buckets.values().next();
    let primary = root_bucket.as_ref().and_then(|b| b.primary.clone())
        .or_else(|| first.and_then(|b| b.primary.clone()));
    let secondary = root_bucket.as_ref().and_then(|b| b.secondary.clone())
        .or_else(|| first.and_then(|b| b.secondary.clone()));

    if primary.is_none() && secondary.is_none() && buckets.is_empty() {
        return None;
    }

    let credits = raw.credits.as_ref().map(|c| CodexCredits {
        has_credits: c.has_credits,
        unlimited: c.unlimited,
        balance: c.balance.clone().unwrap_or_else(|| "0".to_string()),
    });

    let plan_type = account_plan_type
        .map(str::to_string)
        .or_else(|| raw.plan_type.clone())
        .or_else(|| root.and_then(|r| r.plan_type.clone()));

    Some(CodexRateLimits {
        primary,
        secondary,
        credits,
        plan_type,
        buckets: if buckets.is_empty() { None } else { Some(buckets) },
    })
}
```

- [ ] **Step 2: Testes** — normalize de um `rateLimits` root simples (usedPercent 30, windowDurationMins 300) → CodexRateLimits com primary used_percent 30 window 300; rateLimitsByLimitId com 1 bucket; credits camelCase→snake; planType prioridade account>raw>root; None quando vazio.
- [ ] **Step 3: Verificar** (`cargo test ... codex`, clippy). **Step 4: Commit** `feat(rust): normalize app-server do Codex`.

---

### Task 3: Fallback session-log (`find_latest_session_file` + `extract_rate_limits`)

**Files:** Modify `rust/src/providers/codex.rs`.

**Interfaces:**
- Produces: `find_latest_session_file(sessions_dir, now_ms, local_offset) -> Option<PathBuf>`; `extract_rate_limits(path) -> Option<CodexRateLimits>`.

- [ ] **Step 1: Implementar** — porte de `codex.ts` `findLatestSessionFile`/`extractRateLimits` (linhas ~91-152). `find_latest_session_file`: para `dayOffset` 0 e 1, data LOCAL (via `local_offset`) → `sessions_dir/YYYY/MM/DD`; `read_dir` filtra `*.jsonl`; escolhe maior `mtime`; retorna no 1º dia com arquivos. `extract_rate_limits`: lê o arquivo, split linhas, **reverso**, parseia cada linha como JSON; se `payload.type=="token_count"` E `payload.rate_limits` presente → desserializa `rate_limits` como `CodexRateLimits` e retorna.

```rust
use std::path::{Path, PathBuf};
use time::{Duration as TimeDuration, OffsetDateTime, UtcOffset};

fn find_latest_session_file(sessions_dir: &Path, now_ms: u64, offset: UtcOffset) -> Option<PathBuf> {
    let now = OffsetDateTime::from_unix_timestamp_nanos((now_ms as i128) * 1_000_000)
        .ok()?
        .to_offset(offset);
    for day_offset in 0..2 {
        let date = now - TimeDuration::days(day_offset);
        let dir = sessions_dir
            .join(format!("{:04}", date.year()))
            .join(format!("{:02}", date.month() as u8))
            .join(format!("{:02}", date.day()));
        let mut newest: Option<(std::time::SystemTime, PathBuf)> = None;
        let entries = match std::fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                continue;
            }
            let mtime = match entry.metadata().and_then(|m| m.modified()) {
                Ok(t) => t,
                Err(_) => continue,
            };
            if newest.as_ref().map(|(t, _)| mtime > *t).unwrap_or(true) {
                newest = Some((mtime, path));
            }
        }
        if let Some((_, path)) = newest {
            return Some(path);
        }
    }
    None
}

#[derive(Deserialize)]
struct SessionEvent {
    #[serde(default)]
    payload: Option<SessionPayload>,
}
#[derive(Deserialize)]
struct SessionPayload {
    #[serde(rename = "type", default)]
    kind: Option<String>,
    #[serde(default)]
    rate_limits: Option<CodexRateLimits>,
}

fn extract_rate_limits(path: &Path) -> Option<CodexRateLimits> {
    let content = std::fs::read_to_string(path).ok()?;
    for line in content.trim().lines().rev() {
        if line.trim().is_empty() {
            continue;
        }
        if let Ok(ev) = serde_json::from_str::<SessionEvent>(line) {
            if let Some(p) = ev.payload {
                if p.kind.as_deref() == Some("token_count") {
                    if let Some(rl) = p.rate_limits {
                        return Some(rl);
                    }
                }
            }
        }
    }
    None
}
```

- [ ] **Step 2: Testes** — escreva uma árvore `YYYY/MM/DD` em tempdir com `.jsonl` (uma linha `token_count` com `rate_limits`), `now_ms` fixo + `UtcOffset::UTC`; assert que `find_latest_session_file` acha e `extract_rate_limits` devolve os limits. Múltiplos arquivos → maior mtime. Linha sem token_count → ignorada.
- [ ] **Step 3: Verificar** + **Step 4: Commit** `feat(rust): fallback session-log do Codex`.

---

### Task 4: Protocolo app-server (JSON-RPC sobre AsyncRead/AsyncWrite)

**Files:** Modify `rust/src/providers/codex.rs`.

**Interfaces:**
- Produces: `run_appserver_protocol<R: AsyncRead+Unpin, W: AsyncWrite+Unpin>(reader, writer, version, timeout) -> Option<CodexRateLimits>`.

- [ ] **Step 1: Implementar o loop** — porte de `fetchRateLimitsViaAppServer` (codex.ts ~359-453) sobre streams genéricos (p/ testar com `tokio::io::duplex`). Envia `initialize`(id 0); ao receber id 0 com result → envia `initialized` + `account/read`(id 1, `params.refreshToken=false`) + `account/rateLimits/read`(id 2); id 1 → `account_plan`; id 2 com `rateLimits`||`rateLimitsByLimitId` → guarda result; se `account_plan` já chegou → normaliza e retorna; senão arma grace 200ms; timeout externo 4s → None; EOF/erro de leitura → None.

```rust
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};
use crate::app_identity::APP_NAME;

#[derive(Deserialize)]
struct AppServerResponse {
    #[serde(default)]
    id: Option<i64>,
    #[serde(default)]
    result: Option<serde_json::Value>,
}

async fn write_json<W: AsyncWrite + Unpin>(w: &mut W, v: &serde_json::Value) -> std::io::Result<()> {
    let mut s = serde_json::to_string(v).unwrap_or_default();
    s.push('\n');
    w.write_all(s.as_bytes()).await
}

pub async fn run_appserver_protocol<R, W>(
    reader: R,
    mut writer: W,
    version: &str,
    timeout: std::time::Duration,
) -> Option<CodexRateLimits>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let init = serde_json::json!({
        "method": "initialize", "id": 0,
        "params": { "clientInfo": { "name": APP_NAME, "title": APP_NAME, "version": version } }
    });
    write_json(&mut writer, &init).await.ok()?;

    let mut lines = BufReader::new(reader).lines();
    let mut account_plan: Option<Option<String>> = None; // None=não recebido
    let mut rate_limits: Option<CodexAppServerRateLimitsReadResult> = None;

    let hard = tokio::time::sleep(timeout);
    tokio::pin!(hard);
    // grace começa "no futuro" (depois do hard) p/ nunca disparar antes; resetado quando armado.
    let grace = tokio::time::sleep(timeout + std::time::Duration::from_secs(1));
    tokio::pin!(grace);
    let mut grace_armed = false;

    loop {
        tokio::select! {
            _ = &mut hard => return None,
            _ = &mut grace, if grace_armed => {
                return rate_limits.as_ref().and_then(|r| {
                    normalize_appserver_rate_limits(r, account_plan.flatten_ref())
                });
            }
            line = lines.next_line() => {
                let line = match line { Ok(Some(l)) => l, _ => return None };
                let msg: AppServerResponse = match serde_json::from_str(&line) { Ok(m) => m, Err(_) => continue };
                match msg.id {
                    Some(0) if msg.result.is_some() => {
                        let _ = write_json(&mut writer, &serde_json::json!({"method":"initialized","params":{}})).await;
                        let _ = write_json(&mut writer, &serde_json::json!({"method":"account/read","id":1,"params":{"refreshToken":false}})).await;
                        let _ = write_json(&mut writer, &serde_json::json!({"method":"account/rateLimits/read","id":2,"params":{}})).await;
                    }
                    Some(1) => {
                        let plan = msg.result.as_ref()
                            .and_then(|v| serde_json::from_value::<CodexAppServerAccountReadResult>(v.clone()).ok())
                            .and_then(|a| a.account).and_then(|a| a.plan_type);
                        account_plan = Some(plan);
                        if rate_limits.is_some() {
                            if let Some(r) = rate_limits.as_ref() {
                                return normalize_appserver_rate_limits(r, account_plan.as_ref().and_then(|o| o.as_deref()));
                            }
                        }
                    }
                    Some(2) => {
                        let parsed = msg.result.as_ref()
                            .and_then(|v| serde_json::from_value::<CodexAppServerRateLimitsReadResult>(v.clone()).ok());
                        if let Some(r) = parsed {
                            let has_data = r.rate_limits.is_some() || r.rate_limits_by_limit_id.is_some();
                            if has_data {
                                rate_limits = Some(r);
                                if account_plan.is_some() {
                                    if let Some(rr) = rate_limits.as_ref() {
                                        return normalize_appserver_rate_limits(rr, account_plan.as_ref().and_then(|o| o.as_deref()));
                                    }
                                } else {
                                    grace.as_mut().reset(tokio::time::Instant::now() + std::time::Duration::from_millis(200));
                                    grace_armed = true;
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}
```

**Notas p/ o implementer:** (a) `account_plan.flatten_ref()` é pseudocódigo — use `account_plan.as_ref().and_then(|o| o.as_deref())` (resolve `&Option<Option<String>>` → `Option<&str>`); ajuste as 3 ocorrências p/ a forma que compila sem `unwrap`. (b) `write_json` usa `to_string().unwrap_or_default()` (não `unwrap`); o `\n` espelha o TS. (c) o `select!` com `, if grace_armed` só avalia o ramo do grace quando armado — o `grace` pinado começa no futuro distante p/ não disparar antes. (d) `tokio::time::Instant` p/ `reset`. (e) Se o `select!` reclamar de borrow de `writer` em múltiplos ramos, extraia os 3 `write_json` do init num bloco sequencial (já são `.await` sequenciais).

- [ ] **Step 2: Testes com `tokio::io::duplex`** — um par bidirecional: o lado "cliente" (split em read/write) vai p/ `run_appserver_protocol`; uma task "fake server" usa o outro lado p/ ler os requests e empurrar respostas (init `{capabilities:{}}` id 0, account `{account:{planType:"pro"}}` id 1, rateLimits `{rateLimits:{limitId,primary:{usedPercent:30,windowDurationMins:300,resetsAt:...}}}` id 2). Casos (de `codex-appserver.test.ts`): sucesso → Some com primary remaining 70 e plan; autoRespond=false + timeout curto → None; EOF antes das respostas → None.

```rust
    #[tokio::test]
    async fn appserver_happy_path() {
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
        let (client, server) = tokio::io::duplex(8192);
        let (cr, cw) = tokio::io::split(client);
        tokio::spawn(async move {
            let (sr, mut sw) = tokio::io::split(server);
            let mut lines = BufReader::new(sr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let v: serde_json::Value = match serde_json::from_str(&line) { Ok(v) => v, Err(_) => continue };
                match v.get("id").and_then(|i| i.as_i64()) {
                    Some(0) => { let _ = sw.write_all(b"{\"id\":0,\"result\":{\"capabilities\":{}}}\n").await; }
                    Some(1) => { let _ = sw.write_all(b"{\"id\":1,\"result\":{\"account\":{\"planType\":\"pro\"}}}\n").await; }
                    Some(2) => { let _ = sw.write_all(b"{\"id\":2,\"result\":{\"rateLimits\":{\"limitId\":\"codex-default\",\"primary\":{\"usedPercent\":30,\"windowDurationMins\":300,\"resetsAt\":1700000000}}}}\n").await; }
                    _ => {}
                }
            }
        });
        let out = run_appserver_protocol(cr, cw, "test", std::time::Duration::from_secs(4)).await;
        let limits = out.expect("should resolve");
        assert_eq!(limits.primary.as_ref().unwrap().used_percent, 30.0);
        assert_eq!(limits.plan_type.as_deref(), Some("pro"));
    }

    #[tokio::test]
    async fn appserver_timeout_returns_none() {
        let (client, _server) = tokio::io::duplex(8192);
        let (cr, cw) = tokio::io::split(client);
        // _server nunca responde
        let out = run_appserver_protocol(cr, cw, "test", std::time::Duration::from_millis(100)).await;
        assert!(out.is_none());
    }
```

- [ ] **Step 3: Verificar** + **Step 4: Commit** `feat(rust): protocolo app-server do Codex`.

---

### Task 5: `CodexProvider` (fetch_raw = app-server || fallback) + registry

**Files:** Modify `rust/src/providers/codex.rs` + `rust/src/providers/mod.rs`.

**Interfaces:**
- Produces: `CodexProvider` impl `QuotaSource`(Raw=CodexRateLimits) + `Provider`; `registry()` ganha Codex.

- [ ] **Step 1: `fetch_via_appserver` (produção) + `CodexProvider`** — `fetch_via_appserver(version)`: spawn `codex app-server` (`tokio::process`, stdin/stdout pipe, stderr null, `kill_on_drop`), `run_appserver_protocol(stdout, stdin, version, 4s)`, kill no fim. `QuotaSource`: `Raw=CodexRateLimits`; `is_available` = `ctx.paths.codex_auth.exists()`; `fetch_raw`: tenta app-server → se Some, Ok; senão `log::warn!` + `find_latest_session_file` (None→`Err(NoSessionData)`) + `extract_rate_limits` (None→`Err(NoRateLimitData)`); `build_quota` = `build_codex_quota(&raw, base)`; `unavailable_error`=NotLoggedIn; `to_user_facing_error`: `Codex(e)` → `e.to_string()` (NoSessionData/NoRateLimitData passam a mensagem), senão Generic. `Provider` delega `get_quota` a `base_get_quota`.

```rust
use std::process::Stdio;
use super::base::{base_get_quota, quota_base, QuotaSource};
use super::error::{CodexError, ProviderError};
use super::{Ctx, Provider};
use async_trait::async_trait;

async fn fetch_via_appserver(version: &str) -> Option<CodexRateLimits> {
    let mut child = tokio::process::Command::new("codex")
        .arg("app-server")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .ok()?;
    let stdout = child.stdout.take()?;
    let stdin = child.stdin.take()?;
    let result = run_appserver_protocol(stdout, stdin, version, std::time::Duration::from_secs(4)).await;
    let _ = child.start_kill();
    result
}

pub struct CodexProvider;

#[async_trait(?Send)]
impl QuotaSource for CodexProvider {
    type Raw = CodexRateLimits;
    fn id(&self) -> &'static str { "codex" }
    fn name(&self) -> &'static str { "Codex" }
    fn cache_key(&self) -> &'static str { "codex-quota" }

    async fn is_available(&self, ctx: &Ctx<'_>) -> bool {
        ctx.paths.codex_auth.exists()
    }

    async fn fetch_raw(&self, ctx: &Ctx<'_>) -> Result<CodexRateLimits, ProviderError> {
        if let Some(limits) = fetch_via_appserver(ctx.version).await {
            return Ok(limits);
        }
        log::warn!("Codex app-server unavailable, falling back to session log");
        let session = find_latest_session_file(&ctx.paths.codex_sessions, ctx.now_ms, ctx.local_offset)
            .ok_or(CodexError::NoSessionData)?;
        extract_rate_limits(&session).ok_or(CodexError::NoRateLimitData.into())
    }

    fn build_quota(&self, raw: CodexRateLimits, base: ProviderQuota, _ctx: &Ctx<'_>) -> ProviderQuota {
        build_codex_quota(&raw, base)
    }

    fn unavailable_error(&self) -> String { CodexError::NotLoggedIn.to_string() }

    fn to_user_facing_error(&self, error: &ProviderError) -> String {
        match error {
            ProviderError::Codex(e) => e.to_string(),
            _ => CodexError::Generic.to_string(),
        }
    }
}

#[async_trait(?Send)]
impl Provider for CodexProvider {
    fn id(&self) -> &'static str { "codex" }
    fn name(&self) -> &'static str { "Codex" }
    fn cache_key(&self) -> &'static str { "codex-quota" }
    async fn is_available(&self, ctx: &Ctx<'_>) -> bool { QuotaSource::is_available(self, ctx).await }
    async fn get_quota(&self, ctx: &Ctx<'_>) -> ProviderQuota { base_get_quota(self, ctx).await }
}
```

**Nota:** o `ok_or(CodexError::NoSessionData)?` precisa que `CodexError` converta p/ `ProviderError` via `?` (o `#[from]` existe). Se o compilador reclamar do tipo, use `.ok_or(ProviderError::Codex(CodexError::NoSessionData))?`. `quota_base` pode ser import não-usado — remova se acusar.

- [ ] **Step 2: Registrar** — `registry()` = `vec![claude, amp, codex]`; atualizar o teste de registry (len 3, ids contém "codex").
- [ ] **Step 3: Testes** — `is_available` via tempdir (codex_auth existe/não); orquestração via `build_quota` direto (já coberto na T1); um teste de `fetch_raw` fallback: `codex_auth` aponta tempdir SEM `codex` no PATH... (frágil — preferir testar `build_quota`/`find_latest_session_file`/`run_appserver_protocol` isolados, já feito). Cobrir o `to_user_facing_error` (Codex(NoSessionData)→"No session data found").
- [ ] **Step 4: Verificar** (suíte completa + clippy). **Step 5: Commit** `feat(rust): CodexProvider + registry`.

---

## Self-Review (autor)

- **Cobertura do spec:** §3.4 (app-server protocol id0/1/2, refreshToken false, grace 200ms, timeout 4s, kill; fallback YYYY/MM/DD hoje/ontem mtime, scan reverso token_count; classify tolerante) → T2/T3/T4/T5. build_quota (remaining=100-round, credits cap/unlimited, dedup, legacy, plan) → T1.
- **Tipos:** `CodexRateLimits` (Raw cacheável, snake_case) — `buckets` IndexMap (ordem→sufixo); app-server camelCase separado; `models_detailed` BTreeMap (view-model re-ordena).
- **Seam de teste:** protocolo sobre `AsyncRead+AsyncWrite` → `tokio::io::duplex` (sem spawn real). Fallback testável via tempdir. build_quota puro.
- **Sem placeholders:** código completo nas peças não-óbvias; helpers mecânicos com port preciso + assinatura; o implementer pode ler `codex.ts` como autoridade.
- **DEFERIDO:** notify (04d). `registry()` completo após T5 (claude+amp+codex).
