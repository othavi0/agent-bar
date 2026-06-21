# Plano 04d — Notificações (notify-send em quota baixa/crítica)

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:subagent-driven-development. Steps usam checkbox (`- [ ]`).

**Goal:** Último componente do Plano 4: notificações de quota. Decisão PURA (`plan_notifications`: dispara só em escalação, re-arma em recuperação, dedup de alias) + IO best-effort (`check_and_notify`: lê/grava estado por-provider, spawn `notify-send`).

**Architecture:** `src/notify.rs` (top-level, não sob providers). Núcleo puro testável sem mock + camada IO async. Consome `AllQuotas`/`ProviderQuota` (já existem) + `get_claude_extra` (weeklyModels). Os GATES de quando chamar (stdout não-TTY, `settings.notify.enabled`, comando=waybar, !json, !watch) ficam no CALLER (Plano 5) — aqui só a lógica.

**Tech Stack:** serde (estado), tokio::process (`notify-send`). Port fiel de `src/notify.ts` (validado por `tests/notify.test.ts`).

## Global Constraints

- **Byte-exact / paridade com o TS é SAGRADO. Autoridade = `src/notify.ts` + `tests/notify.test.ts`.** Rejeitar "fix" de review que divergiria do TS.
- **Sem `unwrap()`/`expect()` em produção** (deny lint). `#[cfg(test)]` permitido. `unwrap_or`/`.ok()`/`let _ =` permitidos.
- **stdout limpo** (logs só `log::*`; `notify-send` tem stdio ignorado).
- **Thresholds são CONTRATO:** `LOW_USED=90`, `CRITICAL_USED=95` (sobre %USADO). NÃO alterar.
- **`notify-send` ausente → no-op SEM persistir estado** (p/ disparar quando o user instalar). Estado persistido **SÓ p/ os `changed` após o envio**.
- **Verificação:** `cargo test --manifest-path rust/Cargo.toml` + `cargo clippy ... --all-targets -- -D warnings`. RTK: `cargo test: N passed` (sem `test result:`; `... 2>&1 | tail -8`); um filtro posicional. `cargo fmt` ANTES de `git add`. Read antes de Edit. Commits PT ≤50 chars. NÃO tocar main.rs/docs do projeto.

---

## File Structure

- `rust/src/notify.rs` (criar T1+T2) — types + pure (T1) + IO (T2).
- `rust/src/lib.rs` (modificar T1) — `pub mod notify;`.

---

### Task 1: Núcleo puro (`plan_notifications`)

**Files:** Create `rust/src/notify.rs`; Modify `rust/src/lib.rs` (`pub mod notify;`).

**Interfaces:**
- Produces: `LOW_USED`/`CRITICAL_USED`; `NotifyLevel`; `level_for(f64)`; `ProviderNotifyState`; `NotifyFire`; `NotifyPlan`; `plan_notifications(&AllQuotas, &HashMap<String, ProviderNotifyState>) -> NotifyPlan`.

- [ ] **Step 1: Criar `rust/src/notify.rs` (núcleo puro)**

```rust
//! Notificações de quota (notify-send). Núcleo PURO (`plan_notifications`) +
//! IO best-effort (`check_and_notify`). Port fiel de `src/notify.ts`.
//! Os GATES de quando notificar (TTY, settings, comando) ficam no CALLER (Plano 5).

use std::collections::{BTreeMap, HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::providers::extras::get_claude_extra;
use crate::providers::types::{AllQuotas, ProviderQuota, QuotaWindow};

/// Thresholds sobre %USADO de uma janela (contrato — não alterar).
pub const LOW_USED: f64 = 90.0;
pub const CRITICAL_USED: f64 = 95.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotifyLevel {
    Ok,
    Low,
    Critical,
}

impl NotifyLevel {
    fn rank(self) -> u8 {
        match self {
            NotifyLevel::Ok => 0,
            NotifyLevel::Low => 1,
            NotifyLevel::Critical => 2,
        }
    }
    fn as_str(self) -> &'static str {
        match self {
            NotifyLevel::Ok => "ok",
            NotifyLevel::Low => "low",
            NotifyLevel::Critical => "critical",
        }
    }
}

pub fn level_for(used: f64) -> NotifyLevel {
    if used >= CRITICAL_USED {
        NotifyLevel::Critical
    } else if used >= LOW_USED {
        NotifyLevel::Low
    } else {
        NotifyLevel::Ok
    }
}

/// Estado persistido por-provider: maior nível já notificado por label de janela.
/// `windows` guarda strings cruas de nível (sanitizadas na leitura — stale → ok).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProviderNotifyState {
    #[serde(default)]
    pub windows: BTreeMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct NotifyFire {
    pub provider: String,
    pub display_name: String,
    pub label: String,
    pub used: f64,
    pub level: NotifyLevel, // low | critical
}

pub struct NotifyPlan {
    pub fires: Vec<NotifyFire>,
    pub next_states: HashMap<String, ProviderNotifyState>,
    pub changed: HashSet<String>,
}

fn used_of(w: &QuotaWindow) -> f64 {
    w.used.unwrap_or(100.0 - w.remaining)
}

/// Janelas distintas de um provider, com key de dedup, label e %usado. Ordem:
/// models → weeklyModels (Claude) → primary → secondary; dedup por
/// `(round(used), resetsAt)` (1º visto vence — o label amigável do model ganha
/// do alias primary/secondary).
fn windows_of(p: &ProviderQuota) -> Vec<(String, String, f64)> {
    let mut raw: Vec<(String, String, &QuotaWindow)> = Vec::new();
    if let Some(models) = p.models.as_ref() {
        for (name, w) in models {
            raw.push((format!("m:{name}"), name.clone(), w));
        }
    }
    if let Some(weekly) = get_claude_extra(p).and_then(|e| e.weekly_models.as_ref()) {
        for (name, w) in weekly {
            raw.push((format!("w:{name}"), format!("{name} (weekly)"), w));
        }
    }
    if let Some(pr) = p.primary.as_ref() {
        raw.push(("primary".to_string(), "primary".to_string(), pr));
    }
    if let Some(sec) = p.secondary.as_ref() {
        raw.push(("secondary".to_string(), "secondary".to_string(), sec));
    }

    let mut seen: HashSet<String> = HashSet::new();
    let mut out: Vec<(String, String, f64)> = Vec::new();
    for (key, label, w) in raw {
        let used = used_of(w);
        let sig = format!("{}|{}", used.round(), w.resets_at.as_deref().unwrap_or(""));
        if seen.contains(&sig) {
            continue;
        }
        seen.insert(sig);
        out.push((key, label, used));
    }
    out
}

/// Decisão pura: dado os quotas e o estado anterior, retorna o que disparar e o
/// próximo estado. Dispara SÓ quando uma janela ESCALA; re-arma (sem disparar)
/// na recuperação.
pub fn plan_notifications(
    quotas: &AllQuotas,
    prev_states: &HashMap<String, ProviderNotifyState>,
) -> NotifyPlan {
    let mut fires: Vec<NotifyFire> = Vec::new();
    let mut next_states: HashMap<String, ProviderNotifyState> = HashMap::new();
    let mut changed: HashSet<String> = HashSet::new();

    for p in &quotas.providers {
        if !p.available {
            continue;
        }
        let empty = ProviderNotifyState::default();
        let prev = prev_states.get(&p.provider).unwrap_or(&empty);
        let mut next: BTreeMap<String, String> = BTreeMap::new();

        for (key, label, used) in windows_of(p) {
            let current = level_for(used);
            // Sanitiza: valor stale/hand-edited que não é nível conhecido → ok.
            let previous = match prev.windows.get(&key).map(String::as_str) {
                Some("low") => NotifyLevel::Low,
                Some("critical") => NotifyLevel::Critical,
                _ => NotifyLevel::Ok,
            };

            if current.rank() > previous.rank() {
                fires.push(NotifyFire {
                    provider: p.provider.clone(),
                    display_name: p.display_name.clone(),
                    label,
                    used,
                    level: current,
                });
                next.insert(key, current.as_str().to_string());
                changed.insert(p.provider.clone());
            } else if current != previous {
                next.insert(key, current.as_str().to_string());
                changed.insert(p.provider.clone());
            } else if previous != NotifyLevel::Ok {
                next.insert(key, previous.as_str().to_string());
            }
        }

        next_states.insert(p.provider.clone(), ProviderNotifyState { windows: next });
    }

    NotifyPlan {
        fires,
        next_states,
        changed,
    }
}
```

- [ ] **Step 2: Registrar módulo** — em `rust/src/lib.rs`, `pub mod notify;` (ordem alfabética: depois de `logger`, antes de `providers`).

- [ ] **Step 3: Testes** — porte de `tests/notify.test.ts` (todos os 11 casos). Helpers:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::types::{ClaudeQuotaExtra, ProviderExtra, QuotaWindow};
    use indexmap::IndexMap;

    fn win(remaining: f64, resets: Option<&str>) -> QuotaWindow {
        QuotaWindow { remaining, resets_at: resets.map(str::to_string), window_minutes: None, used: None }
    }

    fn wrap(providers: Vec<ProviderQuota>) -> AllQuotas {
        AllQuotas { providers, fetched_at: "2026-06-17T00:00:00.000Z".into() }
    }

    fn claude(primary_remaining: f64) -> ProviderQuota {
        ProviderQuota {
            provider: "claude".into(), display_name: "Claude".into(), available: true,
            account: None, plan: None, plan_type: None,
            primary: Some(win(primary_remaining, None)),
            secondary: None, models: None, extra: None, error: None,
        }
    }

    #[test]
    fn level_for_classifies() {
        assert_eq!(level_for(0.0), NotifyLevel::Ok);
        assert_eq!(level_for(89.0), NotifyLevel::Ok);
        assert_eq!(level_for(90.0), NotifyLevel::Low);
        assert_eq!(level_for(94.0), NotifyLevel::Low);
        assert_eq!(level_for(95.0), NotifyLevel::Critical);
        assert_eq!(level_for(232.0), NotifyLevel::Critical);
    }

    #[test]
    fn fires_low_on_first_cross_90() {
        let plan = plan_notifications(&wrap(vec![claude(8.0)]), &HashMap::new()); // 92% used
        assert_eq!(plan.fires.len(), 1);
        assert_eq!(plan.fires[0].provider, "claude");
        assert_eq!(plan.fires[0].level, NotifyLevel::Low);
        assert_eq!(plan.fires[0].label, "primary");
        assert_eq!(plan.next_states["claude"].windows.get("primary").map(String::as_str), Some("low"));
        assert!(plan.changed.contains("claude"));
    }

    #[test]
    fn no_refire_at_same_level() {
        let mut prev = HashMap::new();
        prev.insert("claude".to_string(), ProviderNotifyState {
            windows: BTreeMap::from([("primary".to_string(), "low".to_string())]),
        });
        let plan = plan_notifications(&wrap(vec![claude(8.0)]), &prev);
        assert_eq!(plan.fires.len(), 0);
        assert!(!plan.changed.contains("claude"));
    }

    #[test]
    fn escalates_low_to_critical() {
        let mut prev = HashMap::new();
        prev.insert("claude".to_string(), ProviderNotifyState {
            windows: BTreeMap::from([("primary".to_string(), "low".to_string())]),
        });
        let plan = plan_notifications(&wrap(vec![claude(3.0)]), &prev); // 97% used
        assert_eq!(plan.fires.len(), 1);
        assert_eq!(plan.fires[0].level, NotifyLevel::Critical);
        assert_eq!(plan.next_states["claude"].windows.get("primary").map(String::as_str), Some("critical"));
    }

    #[test]
    fn rearms_on_recovery_without_firing() {
        let mut prev = HashMap::new();
        prev.insert("claude".to_string(), ProviderNotifyState {
            windows: BTreeMap::from([("primary".to_string(), "low".to_string())]),
        });
        let plan = plan_notifications(&wrap(vec![claude(80.0)]), &prev); // 20% used → ok
        assert_eq!(plan.fires.len(), 0);
        assert_eq!(plan.next_states["claude"].windows.get("primary").map(String::as_str), Some("ok"));
        assert!(plan.changed.contains("claude"));
    }

    #[test]
    fn fires_for_any_window_secondary() {
        let mut c = claude(50.0);
        c.secondary = Some(win(4.0, None)); // 96% used → critical
        let plan = plan_notifications(&wrap(vec![c]), &HashMap::new());
        assert_eq!(plan.fires.len(), 1);
        assert_eq!(plan.fires[0].label, "secondary");
        assert_eq!(plan.fires[0].level, NotifyLevel::Critical);
    }

    #[test]
    fn fires_for_model_window() {
        let mut c = claude(50.0);
        let mut models = IndexMap::new();
        models.insert("Sonnet".to_string(), win(9.0, None)); // 91% → low
        c.models = Some(models);
        let plan = plan_notifications(&wrap(vec![c]), &HashMap::new());
        assert_eq!(plan.fires.len(), 1);
        assert_eq!(plan.fires[0].label, "Sonnet");
        assert_eq!(plan.fires[0].level, NotifyLevel::Low);
    }

    #[test]
    fn honors_provider_used_over_100() {
        let mut c = claude(50.0);
        c.primary = Some(QuotaWindow { remaining: 0.0, resets_at: None, window_minutes: None, used: Some(232.0) });
        let plan = plan_notifications(&wrap(vec![c]), &HashMap::new());
        assert_eq!(plan.fires[0].label, "primary");
        assert_eq!(plan.fires[0].level, NotifyLevel::Critical);
        assert_eq!(plan.fires[0].used, 232.0);
    }

    #[test]
    fn skips_unavailable() {
        let p = ProviderQuota {
            provider: "amp".into(), display_name: "Amp".into(), available: false,
            account: None, plan: None, plan_type: None, primary: None, secondary: None,
            models: None, extra: None, error: Some("x".into()),
        };
        let plan = plan_notifications(&wrap(vec![p]), &HashMap::new());
        assert_eq!(plan.fires.len(), 0);
        assert!(!plan.next_states.contains_key("amp"));
    }

    #[test]
    fn dedups_primary_aliasing_model() {
        let mut models = IndexMap::new();
        models.insert("Free Tier".to_string(), win(8.0, Some("2026-06-17T20:00:00Z")));
        models.insert("Credits".to_string(), win(100.0, None));
        let amp = ProviderQuota {
            provider: "amp".into(), display_name: "Amp".into(), available: true,
            account: None, plan: None, plan_type: None,
            primary: Some(win(8.0, Some("2026-06-17T20:00:00Z"))),
            secondary: None, models: Some(models), extra: None, error: None,
        };
        let plan = plan_notifications(&wrap(vec![amp]), &HashMap::new());
        assert_eq!(plan.fires.len(), 1);
        assert_eq!(plan.fires[0].label, "Free Tier");
        assert_eq!(plan.fires[0].level, NotifyLevel::Low);
    }

    #[test]
    fn fires_for_claude_weekly_models() {
        let mut weekly = IndexMap::new();
        weekly.insert("Opus".to_string(), win(3.0, Some("2026-06-19T00:00:00Z"))); // 97% → critical
        let c = ProviderQuota {
            provider: "claude".into(), display_name: "Claude".into(), available: true,
            account: None, plan: None, plan_type: None,
            primary: Some(win(50.0, None)), secondary: None, models: None,
            extra: Some(ProviderExtra::Claude(ClaudeQuotaExtra { weekly_models: Some(weekly), extra_usage: None })),
            error: None,
        };
        let plan = plan_notifications(&wrap(vec![c]), &HashMap::new());
        assert!(plan.fires.iter().any(|f| f.label == "Opus (weekly)" && f.level == NotifyLevel::Critical));
    }
}
```

- [ ] **Step 4: Verificar** — `cargo test ... notify 2>&1 | tail -8`; `cargo clippy ... -D warnings`.
- [ ] **Step 5: Commit** — `cargo fmt`; `git commit -m "feat(rust): plan_notifications (núcleo puro)"`.

---

### Task 2: IO (`check_and_notify` + estado + spawn)

**Files:** Modify `rust/src/notify.rs`.

**Interfaces:**
- Produces: `check_and_notify(quotas: &AllQuotas, cache_dir: &Path)` (async, best-effort, nunca panica).

- [ ] **Step 1: IO em `notify.rs`** — port de `statePath`/`readState`/`writeState`/`fireNotification`/`checkAndNotify`.

```rust
use std::path::{Path, PathBuf};

fn state_path(cache_dir: &Path, provider: &str) -> PathBuf {
    cache_dir.join(format!("notify-{provider}.json"))
}

fn read_state(cache_dir: &Path, provider: &str) -> ProviderNotifyState {
    std::fs::read(state_path(cache_dir, provider))
        .ok()
        .and_then(|b| serde_json::from_slice::<ProviderNotifyState>(&b).ok())
        .unwrap_or_default()
}

fn write_state(cache_dir: &Path, provider: &str, state: &ProviderNotifyState) {
    use std::io::Write;
    let _ = std::fs::create_dir_all(cache_dir);
    let json = match serde_json::to_string(state) {
        Ok(j) => j,
        Err(_) => return,
    };
    if let Ok(mut tmp) = tempfile::NamedTempFile::new_in(cache_dir) {
        if tmp.write_all(json.as_bytes()).is_ok() {
            let _ = tmp.persist(state_path(cache_dir, provider));
        }
    }
}

/// Dispara `notify-send` (fire-and-forget). stdio ignorado; erros engolidos.
fn fire_notification(fire: &NotifyFire) {
    use std::process::Stdio;
    let left = (100.0 - fire.used.round()).max(0.0);
    let is_critical = fire.level == NotifyLevel::Critical;
    let title = format!(
        "{} quota {}",
        fire.display_name,
        if is_critical { "critical" } else { "low" }
    );
    let body = format!("{}: {}% used ({}% left)", fire.label, fire.used.round(), left);
    // spawn sem kill_on_drop (queremos que sobreviva ao drop do Child).
    let _ = tokio::process::Command::new("notify-send")
        .arg(format!("--app-name={}", crate::app_identity::APP_NAME))
        .arg(format!("--urgency={}", if is_critical { "critical" } else { "normal" }))
        .arg(title)
        .arg(body)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
}

/// Checa janelas e emite notificações nas escaladas. Best-effort: nunca panica.
/// Se `notify-send` ausente → no-op SEM persistir (dispara quando o user instalar).
pub async fn check_and_notify(quotas: &AllQuotas, cache_dir: &Path) {
    if crate::providers::amp_cli::which_in_path("notify-send").is_none() {
        return;
    }
    let prev_states: HashMap<String, ProviderNotifyState> = quotas
        .providers
        .iter()
        .filter(|p| p.available)
        .map(|p| (p.provider.clone(), read_state(cache_dir, &p.provider)))
        .collect();

    let plan = plan_notifications(quotas, &prev_states);

    for fire in &plan.fires {
        fire_notification(fire);
    }
    for provider in &plan.changed {
        if let Some(state) = plan.next_states.get(provider) {
            write_state(cache_dir, provider, state);
        }
    }
}
```

**Nota:** `which_in_path` reusa o helper de `providers/amp_cli.rs` (busca genérica no PATH). Se o reviewer achar o acoplamento estranho, é aceitável (helper genérico); não duplicar.

- [ ] **Step 2: Testes** — (a) `check_and_notify` com `notify-send` AUSENTE do PATH → no-op e NÃO grava estado (tempdir cache fica vazio). Como controlar a ausência? `which_in_path` lê o `$PATH` do processo — num teste `#[serial_test::serial]`, setar `PATH=""` via `temp_env` e confirmar que nenhum `notify-<provider>.json` é criado. (b) `read_state`/`write_state` round-trip num tempdir. (c) Estado stale (JSON com nível inválido) → `read_state` devolve default (não panica). NÃO teste o spawn real de `notify-send` (efeito colateral de desktop).

```rust
    use tempfile::tempdir;

    #[test]
    fn state_roundtrip() {
        let dir = tempdir().unwrap();
        let st = ProviderNotifyState {
            windows: BTreeMap::from([("primary".to_string(), "low".to_string())]),
        };
        write_state(dir.path(), "claude", &st);
        let got = read_state(dir.path(), "claude");
        assert_eq!(got.windows.get("primary").map(String::as_str), Some("low"));
    }

    #[test]
    fn read_state_missing_or_corrupt_is_default() {
        let dir = tempdir().unwrap();
        assert!(read_state(dir.path(), "nope").windows.is_empty());
        std::fs::write(dir.path().join("notify-bad.json"), b"{ not json").unwrap();
        assert!(read_state(dir.path(), "bad").windows.is_empty());
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn no_op_and_no_persist_when_notify_send_absent() {
        let dir = tempdir().unwrap();
        let cache = dir.path().join("cache");
        std::fs::create_dir_all(&cache).unwrap();
        temp_env::async_with_vars([("PATH", Some(""))], async {
            let q = wrap(vec![claude(3.0)]); // escalaria, mas notify-send ausente
            check_and_notify(&q, &cache).await;
        })
        .await;
        // nenhum estado gravado
        assert!(!cache.join("notify-claude.json").exists());
    }
```

**Nota:** `temp_env::async_with_vars` exige a feature async do `temp-env` (já é dev-dep). Se não tiver a variante async, use `#[serial_test::serial]` + `temp_env::with_var` envolvendo um `tokio::runtime::Runtime::new().unwrap().block_on(...)`, OU rode `check_and_notify` num executor manual. Ajuste conforme a API real do `temp-env` disponível; o objetivo é provar o no-op-sem-persist com PATH vazio.

- [ ] **Step 3: Verificar** — `cargo test ... notify 2>&1 | tail -8` + suíte completa + `cargo clippy --all-targets -- -D warnings`. Garanta `git status` limpo.
- [ ] **Step 4: Commit** — `cargo fmt`; `git commit -m "feat(rust): check_and_notify (IO + spawn)"`.

---

## Self-Review (autor)

- **Cobertura do spec:** §3.6 (dispara só em escalação; re-arma sem disparar; persistir SÓ após envio; estado por-provider atômico; dedup de alias por `(round(used), resetsAt)`; thresholds 90/95) → T1/T2. `notify-send` ausente → no-op sem persist → T2.
- **Pureza:** `plan_notifications` é puro (testável síncrono, sem mock). `check_and_notify` isola o IO.
- **Sem placeholders:** código completo + 11 casos do `notify.test.ts` portados + testes de IO.
- **Tipos:** `ProviderNotifyState.windows: BTreeMap<String,String>` (níveis crus, sanitizados na leitura — espelha o `prev[key] === 'low'||'critical'` do TS). `windows_of` itera models(IndexMap)→weekly→primary→secondary com dedup 1º-visto.
- **Gates DEFERIDOS p/ Plano 5:** TTY, `settings.notify.enabled`, comando=waybar, !json, !watch. **PLANO 4 COMPLETO após 04d.**
