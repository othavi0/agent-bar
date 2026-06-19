# Reescrita Rust — Plano 03b: Builders por-provider

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development para executar este plano task-a-task. Steps usam checkbox (`- [ ]`).

**Goal:** Portar os 4 builders por-provider (`claude`, `codex`, `amp`, `generic`) + os helpers de derivação que eles consomem (extras getters, codex model-prep, CodexViewModel, normalize_plan_label), produzindo `Vec<Line>` byte-fiéis ao TS.

**Architecture:** Cada builder é uma função PURA `build_X(clock, p, [view_model,] options) -> Vec<Line>` que compõe os primitivos do Plano 3a (`header_line`, `vline`, `label_line`, `model_line`, `build_footer_line`) + segments (`bar_segments`, `indicator_segments`, `color_for_display`) + shared (`format_percent`, `to_display`, `eta_label`, `format_eta`, `format_reset_time`). Sem I/O, sem leitura de settings, sem markup/escape. O `Clock` é injetado (substitui `Date.now()`/`new Date()` do TS) porque o footer e os ETAs precisam de tempo.

**Tech Stack:** Rust 1.95, `time` 0.3 (Clock já existe), `serde` (tipos já existem). Sem deps novas.

## Global Constraints

Toda task herda implicitamente esta seção. Valores verbatim do TS-fonte (`src/formatters/builders/*.ts`, `src/formatters/codex-helpers.ts`).

- **Contrato byte-exact do Waybar/Pango é a autoridade = saída do TS.** Os builders são port FIEL. **REJEITAR** qualquer "melhoria"/simplificação que divirja do TS (já aconteceu: span vazio nas bordas da barra, `max(1)` no footer — ambos mantidos fiéis). Conferir o comportamento real do TS antes de aceitar qualquer finding de review.
- **Builders são PUROS:** zero I/O, zero leitura de settings (recebem `BuildOptions`/`CodexViewModel` já resolvidos), zero markup/escape. Só compõem `Segment`s via primitivos existentes.
- **Clock injetado:** todo builder recebe `&Clock` (para `build_footer_line` e `model_line`/ETAs). Nunca usar relógio do sistema dentro do builder.
- **XML-escape acontece SÓ em `render_pango`.** Builders nunca escapam; segments `raw` pulam color-wrap E escape.
- **Cor de marca hardcoded por builder:** claude=`Orange`, codex=`Green`, amp=`Magenta`, generic=`Text`. (O `label_color` da seção `◆` vem do `options`, separado da cor de marca.)
- **Ordem das seções é contrato.** Manter EXATAMENTE a ordem de `lines.push(...)` do TS, incluindo os `vline` separadores.
- **Sem `unwrap()`/`expect()` em produção** (deny lint em `lib.rs`/`main.rs`). Em `#[cfg(test)]` é permitido. `unwrap_or`/`unwrap_or_else`/`unwrap_or_default` NÃO são banidos.
- **Sem `!` non-null assertion** (regra do projeto): estreitar com guard que `throw`a/`return`a, não com cast.
- **Verificação:** `cargo test --manifest-path rust/Cargo.toml` + `cargo clippy --manifest-path rust/Cargo.toml --all-targets -- -D warnings`. RTK trunca output multi-suíte — somar suítes com `grep -Eo "[0-9]+ passed"` ou conferir cada `test result:`.
- **`cargo fmt --manifest-path rust/Cargo.toml` ANTES de `git add`** (fmt órfão suja a tree do próximo implementer).
- **Commits:** Conventional Commits em PT, subject ≤50 chars.

### Nota de fidelidade (ordem de iteração de maps)

`ProviderQuota.models` / `ClaudeQuotaExtra.weekly_models` / `CodexQuotaExtra.models_detailed` são `BTreeMap` (ordem alfabética por chave), enquanto o TS itera `Object.entries` (ordem de inserção). Onde o resultado é re-ordenado depois (Codex: sort por severity+name) isso é irrelevante. Onde NÃO é re-ordenado e itera direto — **Claude `weeklyModels`** e **Amp `p.models`** (loop genérico/fallback) — a ordem pode divergir de um dado real não-alfabético. Os snapshots/fixtures atuais usam chaves já alfabéticas (`claude-opus-4-5` < `claude-sonnet-4-5`; Amp acessa `Free Tier`/`Credits` por chave), então BTreeMap bate. **Não mudar o tipo neste plano.** Validar contra os golden snapshots no Plano 3c; se um caso real divergir, a correção é trocar por `IndexMap` no tipo (Plano 02/04), não no builder.

---

## File Structure

- **Create** `rust/src/providers/extras.rs` — getters `get_claude_extra`/`get_codex_extra`/`get_amp_extra`.
- **Modify** `rust/src/providers/mod.rs` — add `pub mod extras;`.
- **Create** `rust/src/formatters/codex_helpers.rs` — `CodexModelEntry`, `codex_models_from_quota`, `apply_codex_model_filter`.
- **Create** `rust/src/formatters/view_model.rs` — `CodexViewModel`, `resolve_codex_view_model_from`.
- **Modify** `rust/src/formatters/shared.rs` — add `normalize_plan_label`.
- **Modify** `rust/src/formatters/mod.rs` — add `pub mod codex_helpers;` e `pub mod view_model;`.
- **Create** `rust/src/formatters/builders/generic.rs` — `build_generic`.
- **Create** `rust/src/formatters/builders/claude.rs` — `build_claude` (+ `extra_usage_line`).
- **Create** `rust/src/formatters/builders/codex.rs` — `build_codex`.
- **Create** `rust/src/formatters/builders/amp.rs` — `build_amp` (+ `free_tier_bar_line`).
- **Modify** `rust/src/formatters/builders/mod.rs` — add `pub mod {generic,claude,codex,amp};`.

---

## Task 1: Extras getters

**Files:**
- Create: `rust/src/providers/extras.rs`
- Modify: `rust/src/providers/mod.rs`

**Interfaces:**
- Consumes: `providers::types::{ProviderQuota, ProviderExtra, ClaudeQuotaExtra, CodexQuotaExtra, AmpQuotaExtra}`.
- Produces: `pub fn get_claude_extra(&ProviderQuota) -> Option<&ClaudeQuotaExtra>`; idem `get_codex_extra -> Option<&CodexQuotaExtra>`, `get_amp_extra -> Option<&AmpQuotaExtra>`.

Port de `src/providers/extras.ts`. No TS o gate é por `q.provider === 'claude'`. No Rust o enum `ProviderExtra` já discrimina, mas mantemos o gate por `provider` (string) E pela variante para fidelidade total: uma quota com `provider="claude"` mas `extra=ProviderExtra::Codex(_)` deve devolver `None`.

- [ ] **Step 1: Escrever o teste que falha**

Em `rust/src/providers/extras.rs`, no fim do arquivo:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::types::{
        ClaudeQuotaExtra, CodexQuotaExtra, ProviderExtra, ProviderQuota,
    };

    fn base(provider: &str, extra: Option<ProviderExtra>) -> ProviderQuota {
        ProviderQuota {
            provider: provider.into(),
            display_name: "X".into(),
            available: true,
            account: None,
            plan: None,
            plan_type: None,
            primary: None,
            secondary: None,
            models: None,
            extra,
            error: None,
        }
    }

    #[test]
    fn claude_getter_returns_payload() {
        let q = base(
            "claude",
            Some(ProviderExtra::Claude(ClaudeQuotaExtra::default())),
        );
        assert!(get_claude_extra(&q).is_some());
        assert!(get_codex_extra(&q).is_none());
        assert!(get_amp_extra(&q).is_none());
    }

    #[test]
    fn getter_gated_by_provider_string() {
        // provider diz "codex" mas o payload é Claude → todos None (fidelidade ao gate do TS).
        let q = base(
            "codex",
            Some(ProviderExtra::Claude(ClaudeQuotaExtra::default())),
        );
        assert!(get_claude_extra(&q).is_none());
        assert!(get_codex_extra(&q).is_none());
    }

    #[test]
    fn codex_getter_returns_payload() {
        let q = base("codex", Some(ProviderExtra::Codex(CodexQuotaExtra::default())));
        assert!(get_codex_extra(&q).is_some());
    }

    #[test]
    fn none_extra_returns_none() {
        let q = base("amp", None);
        assert!(get_amp_extra(&q).is_none());
    }
}
```

- [ ] **Step 2: Rodar o teste e confirmar que falha**

Run: `cargo test --manifest-path rust/Cargo.toml extras 2>&1 | tail -20`
Expected: erro de compilação (`get_claude_extra` não existe).

- [ ] **Step 3: Implementar**

No topo de `rust/src/providers/extras.rs`:

```rust
//! Getters para o payload `extra` específico de cada provider. O enum
//! `ProviderExtra` já discrimina, mas mantemos o gate por `provider` (string)
//! para reproduzir exatamente o comportamento do TS (`q.provider === '...'`).

use super::types::{AmpQuotaExtra, ClaudeQuotaExtra, CodexQuotaExtra, ProviderExtra, ProviderQuota};

/// Payload Claude-específico, ou None para outros providers.
pub fn get_claude_extra(q: &ProviderQuota) -> Option<&ClaudeQuotaExtra> {
    match (q.provider == "claude", &q.extra) {
        (true, Some(ProviderExtra::Claude(e))) => Some(e),
        _ => None,
    }
}

/// Payload Codex-específico, ou None para outros providers.
pub fn get_codex_extra(q: &ProviderQuota) -> Option<&CodexQuotaExtra> {
    match (q.provider == "codex", &q.extra) {
        (true, Some(ProviderExtra::Codex(e))) => Some(e),
        _ => None,
    }
}

/// Payload Amp-específico, ou None para outros providers.
pub fn get_amp_extra(q: &ProviderQuota) -> Option<&AmpQuotaExtra> {
    match (q.provider == "amp", &q.extra) {
        (true, Some(ProviderExtra::Amp(e))) => Some(e),
        _ => None,
    }
}
```

Em `rust/src/providers/mod.rs`, adicionar a linha (ordem alfabética):

```rust
pub mod extras;
pub mod types;
```

- [ ] **Step 4: Rodar e confirmar que passa**

Run: `cargo test --manifest-path rust/Cargo.toml extras 2>&1 | grep "test result"`
Expected: `test result: ok. 4 passed`.

- [ ] **Step 5: fmt + clippy + commit**

```bash
cargo fmt --manifest-path rust/Cargo.toml
cargo clippy --manifest-path rust/Cargo.toml --all-targets -- -D warnings
git add rust/src/providers/extras.rs rust/src/providers/mod.rs
git commit -m "feat(rust): getters de extra por-provider"
```

---

## Task 2: Codex model-prep + CodexViewModel + normalize_plan_label

**Files:**
- Create: `rust/src/formatters/codex_helpers.rs`
- Create: `rust/src/formatters/view_model.rs`
- Modify: `rust/src/formatters/shared.rs` (adicionar `normalize_plan_label`)
- Modify: `rust/src/formatters/mod.rs`

**Interfaces:**
- Consumes: `formatters::shared::{classify_window, WindowKind, normalize_plan}`, `providers::extras::get_codex_extra`, `providers::types::{ModelWindows, ProviderQuota, QuotaWindow}`, `settings::{Settings, WindowPolicy}`.
- Produces:
  - `pub struct CodexModelEntry { pub name: String, pub windows: ModelWindows, pub severity: f64 }`
  - `pub fn codex_models_from_quota(&ProviderQuota) -> Vec<CodexModelEntry>`
  - `pub fn apply_codex_model_filter(Vec<CodexModelEntry>, Option<&[String]>) -> Vec<CodexModelEntry>`
  - `pub struct CodexViewModel { pub models: Vec<CodexModelEntry>, pub policy: WindowPolicy }`
  - `pub fn resolve_codex_view_model_from(&Settings, &ProviderQuota) -> CodexViewModel`
  - `pub fn normalize_plan_label(&ProviderQuota) -> String` (em `shared.rs`)

Port de `src/formatters/codex-helpers.ts` + `src/formatters/view-model.ts` + `normalizePlanLabel` de `src/formatters/shared.ts:85`.

Notas de port:
- **Agrupamento:** `modelsDetailed` primeiro (clona janelas), depois `p.models` classificados via `classify_window(window_minutes)`. Regra do TS: se `fiveHour` já preenchido, a próxima janela fiveHour cai em `other` (idem sevenDay). Fallback `Codex` só quando o map ficou vazio E há `primary`/`secondary`.
- **Severity:** menor `remaining` entre as janelas presentes; `101.0` se nenhuma. Sort: `severity` asc, desempate por `name` (codepoint order). ⚠️ O TS usa `localeCompare` (locale-aware) — para nomes ASCII lowercase a ordem é idêntica ao `str::cmp`; divergência só em maiúsculas/acentos (não ocorre em nomes de model). Manter `str::cmp`; registrar como Minor.
- `WindowPolicy` deriva `Copy` (confirmar em `settings.rs`; se não, usar `.cloned()` em vez de `.copied()`).

- [ ] **Step 1: Escrever os testes que falham**

`rust/src/formatters/codex_helpers.rs` (módulo de teste):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::types::{CodexQuotaExtra, ProviderExtra, ProviderQuota, QuotaWindow};
    use std::collections::BTreeMap;

    fn win(remaining: f64, minutes: Option<i64>) -> QuotaWindow {
        QuotaWindow {
            remaining,
            resets_at: Some("2026-06-19T14:00:00Z".into()),
            window_minutes: minutes,
            used: None,
        }
    }

    fn quota_with_models(models: BTreeMap<String, QuotaWindow>) -> ProviderQuota {
        ProviderQuota {
            provider: "codex".into(),
            display_name: "Codex".into(),
            available: true,
            account: None,
            plan: None,
            plan_type: None,
            primary: None,
            secondary: None,
            models: Some(models),
            extra: None,
            error: None,
        }
    }

    #[test]
    fn classifies_and_sorts_by_severity() {
        let mut m = BTreeMap::new();
        m.insert("gpt-5".to_string(), win(80.0, Some(300))); // fiveHour, sev 80
        m.insert("o3".to_string(), win(20.0, Some(10080))); // sevenDay, sev 20
        let entries = codex_models_from_quota(&quota_with_models(m));
        assert_eq!(entries.len(), 2);
        // menor remaining (mais severo) primeiro
        assert_eq!(entries[0].name, "o3");
        assert_eq!(entries[0].windows.seven_day.as_ref().unwrap().remaining, 20.0);
        assert_eq!(entries[1].name, "gpt-5");
        assert_eq!(entries[1].windows.five_hour.as_ref().unwrap().remaining, 80.0);
    }

    #[test]
    fn fallback_to_codex_from_primary_secondary() {
        let mut q = quota_with_models(BTreeMap::new());
        q.models = None;
        q.primary = Some(win(60.0, Some(300)));
        q.secondary = Some(win(50.0, Some(10080)));
        let entries = codex_models_from_quota(&q);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "Codex");
        assert_eq!(entries[0].windows.five_hour.as_ref().unwrap().remaining, 60.0);
        assert_eq!(entries[0].windows.seven_day.as_ref().unwrap().remaining, 50.0);
    }

    #[test]
    fn second_five_hour_goes_to_other() {
        // modelsDetailed dá fiveHour; p.models tenta outro fiveHour → vai p/ other
        let detailed = {
            let mut md = BTreeMap::new();
            let mut mw = crate::providers::types::ModelWindows::default();
            mw.five_hour = Some(win(90.0, Some(300)));
            md.insert("gpt-5".to_string(), mw);
            md
        };
        let mut q = quota_with_models({
            let mut m = BTreeMap::new();
            m.insert("gpt-5".to_string(), win(40.0, Some(300)));
            m
        });
        q.extra = Some(ProviderExtra::Codex(CodexQuotaExtra {
            models_detailed: Some(detailed),
            extra_usage: None,
        }));
        let entries = codex_models_from_quota(&q);
        assert_eq!(entries.len(), 1);
        let w = &entries[0].windows;
        assert_eq!(w.five_hour.as_ref().unwrap().remaining, 90.0);
        assert_eq!(w.other.as_ref().unwrap()[0].remaining, 40.0);
        // severity = min(90, 40) = 40
        assert_eq!(entries[0].severity, 40.0);
    }

    #[test]
    fn filter_keeps_only_allowed() {
        let mut m = BTreeMap::new();
        m.insert("gpt-5".to_string(), win(80.0, Some(300)));
        m.insert("o3".to_string(), win(20.0, Some(300)));
        let all = codex_models_from_quota(&quota_with_models(m));
        let allowed = vec!["gpt-5".to_string()];
        let filtered = apply_codex_model_filter(all, Some(&allowed));
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].name, "gpt-5");
    }

    #[test]
    fn empty_filter_is_passthrough() {
        let mut m = BTreeMap::new();
        m.insert("gpt-5".to_string(), win(80.0, Some(300)));
        let all = codex_models_from_quota(&quota_with_models(m));
        let n = all.len();
        assert_eq!(apply_codex_model_filter(all, Some(&[])).len(), n);
    }
}
```

`rust/src/formatters/view_model.rs` (módulo de teste). Usa `settings::load` com um tempdir vazio → produz os defaults (`window_policy["codex"] = Both`, `models` vazio), exatamente como o módulo de teste de `settings.rs` faz. Não cria helper de produção:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Paths;
    use crate::providers::types::{ProviderQuota, QuotaWindow};
    use crate::settings::{load, WindowPolicy};
    use std::collections::BTreeMap;
    use std::path::PathBuf;
    use tempfile::tempdir;

    fn paths_in(dir: &std::path::Path) -> Paths {
        Paths {
            cache_dir: dir.join("cache"),
            config_dir: dir.join("config"),
            claude_credentials: PathBuf::new(),
            codex_auth: PathBuf::new(),
            codex_sessions: PathBuf::new(),
            amp_settings: PathBuf::new(),
            amp_threads: PathBuf::new(),
        }
    }

    fn codex_quota() -> ProviderQuota {
        let mut m = BTreeMap::new();
        m.insert(
            "gpt-5".to_string(),
            QuotaWindow {
                remaining: 80.0,
                resets_at: None,
                window_minutes: Some(300),
                used: None,
            },
        );
        ProviderQuota {
            provider: "codex".into(),
            display_name: "Codex".into(),
            available: true,
            account: None,
            plan: None,
            plan_type: None,
            primary: None,
            secondary: None,
            models: Some(m),
            extra: None,
            error: None,
        }
    }

    #[test]
    fn resolves_policy_and_models() {
        let dir = tempdir().unwrap();
        let settings = load(&paths_in(dir.path())); // defaults: window_policy[codex]=Both
        let vm = resolve_codex_view_model_from(&settings, &codex_quota());
        assert_eq!(vm.policy, WindowPolicy::Both);
        assert_eq!(vm.models.len(), 1);
        assert_eq!(vm.models[0].name, "gpt-5");
    }
}
```

- [ ] **Step 2: Rodar e confirmar que falha**

Run: `cargo test --manifest-path rust/Cargo.toml codex_helpers view_model 2>&1 | tail -20`
Expected: erro de compilação (símbolos não existem).

- [ ] **Step 3: Implementar `codex_helpers.rs`**

```rust
//! Derivação dos models do Codex a partir do ProviderQuota: agrupa janelas por
//! model (modelsDetailed + p.models classificados + fallback primary/secondary)
//! e ordena por severidade (menor remaining primeiro), desempate por nome.

use std::collections::BTreeMap;

use crate::formatters::shared::{classify_window, WindowKind};
use crate::providers::extras::get_codex_extra;
use crate::providers::types::{ModelWindows, ProviderQuota, QuotaWindow};

#[derive(Debug, Clone, PartialEq)]
pub struct CodexModelEntry {
    pub name: String,
    pub windows: ModelWindows,
    pub severity: f64,
}

/// Coloca `window` no bucket certo de `mw` seguindo a regra do TS:
/// fiveHour/sevenDay só se ainda vazio; senão cai em `other`.
fn place_window(mw: &mut ModelWindows, window: &QuotaWindow) {
    match classify_window(window.window_minutes) {
        WindowKind::FiveHour if mw.five_hour.is_none() => mw.five_hour = Some(window.clone()),
        WindowKind::SevenDay if mw.seven_day.is_none() => mw.seven_day = Some(window.clone()),
        _ => mw.other.get_or_insert_with(Vec::new).push(window.clone()),
    }
}

pub fn codex_models_from_quota(p: &ProviderQuota) -> Vec<CodexModelEntry> {
    let mut models: BTreeMap<String, ModelWindows> = BTreeMap::new();

    if let Some(detailed) = get_codex_extra(p).and_then(|e| e.models_detailed.as_ref()) {
        for (name, windows) in detailed {
            models.insert(name.clone(), windows.clone());
        }
    }

    if let Some(pm) = p.models.as_ref() {
        for (name, window) in pm {
            let entry = models.entry(name.clone()).or_default();
            place_window(entry, window);
        }
    }

    if models.is_empty() && (p.primary.is_some() || p.secondary.is_some()) {
        let mut fallback = ModelWindows::default();
        for window in [p.primary.as_ref(), p.secondary.as_ref()].into_iter().flatten() {
            place_window(&mut fallback, window);
        }
        models.insert("Codex".to_string(), fallback);
    }

    let mut entries: Vec<CodexModelEntry> = models
        .into_iter()
        .map(|(name, windows)| {
            let mut values: Vec<f64> = Vec::new();
            if let Some(w) = &windows.five_hour {
                values.push(w.remaining);
            }
            if let Some(w) = &windows.seven_day {
                values.push(w.remaining);
            }
            if let Some(others) = &windows.other {
                values.extend(others.iter().map(|w| w.remaining));
            }
            let severity = if values.is_empty() {
                101.0
            } else {
                values.iter().copied().fold(f64::INFINITY, f64::min)
            };
            CodexModelEntry {
                name,
                windows,
                severity,
            }
        })
        .collect();

    entries.sort_by(|a, b| {
        a.severity
            .partial_cmp(&b.severity)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.name.cmp(&b.name))
    });
    entries
}

/// Filtra para os models permitidos (lista de settings). Lista ausente/vazia → passthrough.
pub fn apply_codex_model_filter(
    models: Vec<CodexModelEntry>,
    allowed: Option<&[String]>,
) -> Vec<CodexModelEntry> {
    match allowed {
        Some(a) if !a.is_empty() => models
            .into_iter()
            .filter(|m| a.iter().any(|x| x == &m.name))
            .collect(),
        _ => models,
    }
}
```

- [ ] **Step 4: Implementar `view_model.rs`**

```rust
//! View model do Codex resolvido a partir de settings já carregadas (puro). A
//! variante que carrega settings frescas vive na superfície (Plano 3c).

use crate::providers::types::ProviderQuota;
use crate::settings::{Settings, WindowPolicy};

use super::codex_helpers::{apply_codex_model_filter, codex_models_from_quota, CodexModelEntry};

/// Dados que o builder do Codex precisa: models filtrados + window policy.
#[derive(Debug, Clone, PartialEq)]
pub struct CodexViewModel {
    pub models: Vec<CodexModelEntry>,
    pub policy: WindowPolicy,
}

/// Deriva o view model a partir de settings já carregadas.
pub fn resolve_codex_view_model_from(settings: &Settings, p: &ProviderQuota) -> CodexViewModel {
    let policy = settings
        .window_policy
        .get(&p.provider)
        .copied()
        .unwrap_or(WindowPolicy::Both);
    let allowed = settings.models.get(&p.provider).map(|v| v.as_slice());
    let models = apply_codex_model_filter(codex_models_from_quota(p), allowed);
    CodexViewModel { models, policy }
}
```

- [ ] **Step 5: Adicionar `normalize_plan_label` em `shared.rs`**

Logo após `normalize_plan` (e antes de `titlecase_plan` ou no fim do bloco de helpers de plano), adicionar:

```rust
/// Label de plano para um quota: normaliza `plan` (ou `plan_type` como fallback);
/// "Unknown" quando nenhum resolve.
pub fn normalize_plan_label(p: &crate::providers::types::ProviderQuota) -> String {
    normalize_plan(p.plan.as_deref().or(p.plan_type.as_deref()))
        .unwrap_or_else(|| "Unknown".to_string())
}
```

E um teste no módulo `helper_tests` de `shared.rs`:

```rust
    #[test]
    fn normalize_plan_label_prefers_plan_then_type_then_unknown() {
        use crate::providers::types::ProviderQuota;
        let mut q = ProviderQuota {
            provider: "codex".into(),
            display_name: "Codex".into(),
            available: true,
            account: None,
            plan: Some("pro".into()),
            plan_type: Some("ignored".into()),
            primary: None,
            secondary: None,
            models: None,
            extra: None,
            error: None,
        };
        assert_eq!(normalize_plan_label(&q), "Pro");
        q.plan = None;
        assert_eq!(normalize_plan_label(&q), "Ignored"); // titlecase do plan_type
        q.plan_type = None;
        assert_eq!(normalize_plan_label(&q), "Unknown");
    }
```

- [ ] **Step 6: Wire `formatters/mod.rs`**

Inserir nas posições alfabéticas:

```rust
pub mod builders;
pub mod clock;
pub mod codex_helpers;
pub mod json;
pub mod render_ansi;
pub mod render_pango;
pub mod segments;
pub mod shared;
pub mod view_model;
```

- [ ] **Step 7: Rodar e confirmar que passa**

Run: `cargo test --manifest-path rust/Cargo.toml codex_helpers view_model normalize_plan_label 2>&1 | grep "test result"`
Expected: todas as suítes `ok`, somando ≥7 testes novos (5 codex_helpers + 1 view_model + 1 shared).

- [ ] **Step 8: fmt + clippy + commit**

```bash
cargo fmt --manifest-path rust/Cargo.toml
cargo clippy --manifest-path rust/Cargo.toml --all-targets -- -D warnings
git add rust/src/formatters/codex_helpers.rs rust/src/formatters/view_model.rs rust/src/formatters/shared.rs rust/src/formatters/mod.rs
git commit -m "feat(rust): codex model-prep + CodexViewModel + plan label"
```

---

## Task 3: Builder generic

**Files:**
- Create: `rust/src/formatters/builders/generic.rs`
- Modify: `rust/src/formatters/builders/mod.rs`

**Interfaces:**
- Consumes: `builders::shared::{BuildOptions, header_line, build_footer_line}`, `segments::{bar_segments, color_for_display, indicator_segments, Line, Segment}`, `shared::{format_percent, to_display}`, `theme::ColorToken`, `clock::Clock`, `providers::types::ProviderQuota`.
- Produces: `pub fn build_generic(clock: &Clock, p: &ProviderQuota, options: &BuildOptions) -> Vec<Line>`.

Port FIEL de `src/formatters/builders/generic.ts`. Cor de marca = `Text`. Estrutura: header → (error | primary) → footer. **Sem `vline`** em nenhum lugar. Header usa `header_line` (idêntico ao header inline do TS).

- [ ] **Step 1: Escrever os testes que falham**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::formatters::builders::shared::{AmpLayout, BuildOptions};
    use crate::formatters::clock::Clock;
    use crate::formatters::render_pango::render_pango;
    use crate::providers::types::{ProviderQuota, QuotaWindow};
    use crate::settings::DisplayMode;
    use crate::theme::ColorToken;
    use time::macros::datetime;

    fn clk() -> Clock {
        Clock {
            now: datetime!(2026-06-19 12:00:00 UTC),
            local_offset: time::UtcOffset::UTC,
        }
    }

    fn opts() -> BuildOptions {
        BuildOptions {
            mode: DisplayMode::Remaining,
            header_title: "Foo".into(),
            header_width: 52,
            label_color: ColorToken::Text,
            footer_fetched_at: None,
            plan_label: None,
            amp_free_tier_layout: AmpLayout::Inline,
            account_in_header: false,
        }
    }

    fn quota() -> ProviderQuota {
        ProviderQuota {
            provider: "foo".into(),
            display_name: "Foo".into(),
            available: true,
            account: None,
            plan: None,
            plan_type: None,
            primary: Some(QuotaWindow {
                remaining: 60.0,
                resets_at: None,
                window_minutes: None,
                used: None,
            }),
            secondary: None,
            models: None,
            extra: None,
            error: None,
        }
    }

    #[test]
    fn renders_header_primary_footer() {
        let lines = build_generic(&clk(), &quota(), &opts());
        assert_eq!(lines.len(), 3); // header + primary + footer (sem vline)
        let out = render_pango(&lines);
        assert!(out.contains("Foo"));
        assert!(out.contains("60%"));
    }

    #[test]
    fn error_branch_replaces_primary() {
        let mut q = quota();
        q.error = Some("boom".into());
        q.primary = None;
        let lines = build_generic(&clk(), &q, &opts());
        assert_eq!(lines.len(), 3); // header + error + footer
        let out = render_pango(&lines);
        assert!(out.contains("boom"));
    }

    #[test]
    fn no_primary_no_error_omits_middle() {
        let mut q = quota();
        q.primary = None;
        let lines = build_generic(&clk(), &q, &opts());
        assert_eq!(lines.len(), 2); // header + footer
    }
}
```

- [ ] **Step 2: Rodar e confirmar que falha**

Run: `cargo test --manifest-path rust/Cargo.toml builders::generic 2>&1 | tail -15`
Expected: erro de compilação.

- [ ] **Step 3: Implementar**

```rust
//! Builder do card genérico (fallback para provider sem builder dedicado).
//! Port fiel de `src/formatters/builders/generic.ts`. Cor de marca = Text.
//! Sem `vline` separadores.

use crate::formatters::clock::Clock;
use crate::formatters::segments::{
    bar_segments, color_for_display, indicator_segments, Line, Segment,
};
use crate::formatters::shared::{format_percent, to_display};
use crate::providers::types::ProviderQuota;
use crate::theme::ColorToken;

use super::shared::{build_footer_line, header_line, BuildOptions};

pub fn build_generic(clock: &Clock, p: &ProviderQuota, options: &BuildOptions) -> Vec<Line> {
    let mut lines: Vec<Line> = Vec::new();

    lines.push(header_line(
        &options.header_title,
        options.header_width,
        ColorToken::Text,
    ));

    if let Some(err) = p.error.as_deref() {
        lines.push(vec![
            Segment::new(crate::theme::box_chars::V, ColorToken::Text),
            Segment::raw_text("  "),
            Segment::new(err.to_string(), ColorToken::Red),
        ]);
    } else if let Some(primary) = p.primary.as_ref() {
        let disp = to_display(Some(primary.remaining), options.mode);
        let mut line: Line = vec![
            Segment::new(crate::theme::box_chars::V, ColorToken::Text),
            Segment::raw_text("  "),
        ];
        line.extend(indicator_segments(disp, options.mode));
        line.push(Segment::raw_text(" "));
        line.extend(bar_segments(disp, options.mode));
        line.push(Segment::raw_text(" "));
        line.push(Segment::new(
            format!("{:>4}", format_percent(disp)),
            color_for_display(disp, options.mode),
        ));
        lines.push(line);
    }

    lines.push(build_footer_line(
        clock,
        options.footer_fetched_at.as_deref(),
        ColorToken::Text,
    ));

    lines
}
```

> **Nota:** o TS usa `error` (sem `⚠️`) no generic, mas `⚠️ {error}` em claude/codex/amp. Manter essa assimetria fiel (generic = só o texto).

- [ ] **Step 4: Wire `builders/mod.rs`**

```rust
pub mod amp;
pub mod claude;
pub mod codex;
pub mod generic;
pub mod shared;
```

> Adicionar só `pub mod generic;` nesta task; as outras linhas (`amp`/`claude`/`codex`) serão adicionadas pelas respectivas tasks. Se o módulo ainda não existir, manter só `pub mod generic;` + `pub mod shared;` por ora. (O estado final acima é o objetivo ao fim do plano.)

- [ ] **Step 5: Rodar e confirmar que passa**

Run: `cargo test --manifest-path rust/Cargo.toml builders::generic 2>&1 | grep "test result"`
Expected: `test result: ok. 3 passed`.

- [ ] **Step 6: fmt + clippy + commit**

```bash
cargo fmt --manifest-path rust/Cargo.toml
cargo clippy --manifest-path rust/Cargo.toml --all-targets -- -D warnings
git add rust/src/formatters/builders/generic.rs rust/src/formatters/builders/mod.rs
git commit -m "feat(rust): builder generic"
```

---

## Task 4: Builder claude

**Files:**
- Create: `rust/src/formatters/builders/claude.rs`
- Modify: `rust/src/formatters/builders/mod.rs`

**Interfaces:**
- Consumes: `builders::shared::{BuildOptions, header_line, vline, label_line, model_line, build_footer_line}`, `segments::{bar_segments, color_for_display, indicator_segments, Line, Segment}`, `shared::{format_percent, to_display}`, `providers::extras::get_claude_extra`, `theme::{box_chars, ColorToken}`, `clock::Clock`, `providers::types::ProviderQuota`.
- Produces: `pub fn build_claude(clock: &Clock, p: &ProviderQuota, options: &BuildOptions) -> Vec<Line>`.

Port FIEL de `src/formatters/builders/claude.ts`. Cor de marca = `Orange`. `max_len = 20`. Ordem: header → vline → (error | seções) → vline → footer. Seções (cada uma precedida do label e, exceto a 1ª `primary`, de um `vline`): `5-hour limit (shared)` (primary), `Weekly per model` (weeklyModels não-vazio), `Weekly limit (shared)` (secondary), `Extra Usage` (extraUsage enabled && limit>0).

**Detalhe weeklyModels:** `w_max_len = max(maior nome, 20)`. **Detalhe extraUsage:** `$used/100/$limit/100` com 2 casas (`format!("${:.2}/${:.2}", used/100.0, limit/100.0)`); `used`/`limit` são centavos inteiros → sem arredondamento real.

- [ ] **Step 1: Escrever os testes que falham**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::formatters::builders::shared::{AmpLayout, BuildOptions};
    use crate::formatters::clock::Clock;
    use crate::formatters::render_pango::render_pango;
    use crate::providers::types::{
        ClaudeQuotaExtra, ExtraUsage, ProviderExtra, ProviderQuota, QuotaWindow,
    };
    use crate::settings::DisplayMode;
    use crate::theme::ColorToken;
    use std::collections::BTreeMap;
    use time::macros::datetime;

    fn clk() -> Clock {
        Clock {
            now: datetime!(2026-06-19 12:00:00 UTC),
            local_offset: time::UtcOffset::UTC,
        }
    }

    fn opts() -> BuildOptions {
        BuildOptions {
            mode: DisplayMode::Remaining,
            header_title: "Claude".into(),
            header_width: 52,
            label_color: ColorToken::Orange,
            footer_fetched_at: None,
            plan_label: None,
            amp_free_tier_layout: AmpLayout::Inline,
            account_in_header: false,
        }
    }

    fn win(r: f64, m: Option<i64>) -> QuotaWindow {
        QuotaWindow {
            remaining: r,
            resets_at: Some("2026-06-19T14:00:00Z".into()),
            window_minutes: m,
            used: None,
        }
    }

    fn base() -> ProviderQuota {
        ProviderQuota {
            provider: "claude".into(),
            display_name: "Claude".into(),
            available: true,
            account: None,
            plan: Some("Pro".into()),
            plan_type: None,
            primary: Some(win(60.0, Some(300))),
            secondary: Some(win(50.0, Some(10080))),
            models: None,
            extra: None,
            error: None,
        }
    }

    #[test]
    fn renders_all_sections_in_order() {
        let mut q = base();
        let mut weekly = BTreeMap::new();
        weekly.insert("claude-opus-4-5".to_string(), win(40.0, Some(10080)));
        weekly.insert("claude-sonnet-4-5".to_string(), win(65.0, Some(10080)));
        q.extra = Some(ProviderExtra::Claude(ClaudeQuotaExtra {
            weekly_models: Some(weekly),
            extra_usage: Some(ExtraUsage {
                enabled: true,
                remaining: 55.0,
                limit: 5000.0,
                used: 2250.0,
            }),
        }));
        let out = render_pango(&build_claude(&clk(), &q, &opts()));
        // ordem das seções
        let i5 = out.find("5-hour limit (shared)").unwrap();
        let iw = out.find("Weekly per model").unwrap();
        let iws = out.find("Weekly limit (shared)").unwrap();
        let ie = out.find("Extra Usage").unwrap();
        assert!(i5 < iw && iw < iws && iws < ie);
        // weeklyModels em ordem alfabética (BTreeMap)
        assert!(out.find("claude-opus-4-5").unwrap() < out.find("claude-sonnet-4-5").unwrap());
        // extraUsage formata centavos
        assert!(out.contains("$22.50/$50.00"));
    }

    #[test]
    fn error_branch() {
        let mut q = base();
        q.error = Some("token expired".into());
        let out = render_pango(&build_claude(&clk(), &q, &opts()));
        assert!(out.contains("⚠️ token expired"));
        assert!(!out.contains("5-hour limit"));
    }

    #[test]
    fn extra_usage_gated_by_limit_positive() {
        let mut q = base();
        q.extra = Some(ProviderExtra::Claude(ClaudeQuotaExtra {
            weekly_models: None,
            extra_usage: Some(ExtraUsage {
                enabled: true,
                remaining: 0.0,
                limit: 0.0,
                used: 0.0,
            }),
        }));
        let out = render_pango(&build_claude(&clk(), &q, &opts()));
        assert!(!out.contains("Extra Usage")); // limit == 0 → seção omitida
    }
}
```

- [ ] **Step 2: Rodar e confirmar que falha**

Run: `cargo test --manifest-path rust/Cargo.toml builders::claude 2>&1 | tail -15`
Expected: erro de compilação.

- [ ] **Step 3: Implementar**

```rust
//! Builder do card Claude. Port fiel de `src/formatters/builders/claude.ts`.
//! Cor de marca = Orange.

use crate::formatters::clock::Clock;
use crate::formatters::segments::{
    bar_segments, color_for_display, indicator_segments, Line, Segment,
};
use crate::formatters::shared::{format_percent, to_display};
use crate::providers::extras::get_claude_extra;
use crate::providers::types::ProviderQuota;
use crate::settings::DisplayMode;
use crate::theme::{box_chars, ColorToken};

use super::shared::{build_footer_line, header_line, label_line, model_line, vline, BuildOptions};

/// Linha de Extra Usage: indicador + nome + barra + pct + texto `$used/$limit`.
fn extra_usage_line(
    name: &str,
    max_len: usize,
    disp: Option<f64>,
    mode: DisplayMode,
    used_str: &str,
) -> Line {
    let mut line: Line = vec![
        Segment::new(box_chars::V, ColorToken::Orange),
        Segment::raw_text("  "),
    ];
    line.extend(indicator_segments(disp, mode));
    line.push(Segment::raw_text(" "));
    line.push(Segment::new(format!("{name:<max_len$}"), ColorToken::TextBright));
    line.push(Segment::raw_text(" "));
    line.extend(bar_segments(disp, mode));
    line.push(Segment::raw_text(" "));
    line.push(Segment::new(
        format!("{:>4}", format_percent(disp)),
        color_for_display(disp, mode),
    ));
    line.push(Segment::raw_text(" "));
    line.push(Segment::new(used_str.to_string(), ColorToken::Cyan));
    line
}

pub fn build_claude(clock: &Clock, p: &ProviderQuota, options: &BuildOptions) -> Vec<Line> {
    let mode = options.mode;
    let mut lines: Vec<Line> = Vec::new();

    lines.push(header_line(
        &options.header_title,
        options.header_width,
        ColorToken::Orange,
    ));
    lines.push(vline(ColorToken::Orange));

    if let Some(err) = p.error.as_deref() {
        lines.push(vec![
            Segment::new(box_chars::V, ColorToken::Orange),
            Segment::raw_text("  "),
            Segment::new(format!("⚠️ {err}"), ColorToken::Red),
        ]);
    } else {
        let max_len = 20;

        if let Some(primary) = p.primary.as_ref() {
            lines.push(label_line(
                "5-hour limit (shared)",
                options.label_color,
                ColorToken::Orange,
            ));
            lines.push(model_line(
                clock,
                "All Models",
                Some(primary),
                max_len,
                mode,
                ColorToken::Orange,
                None,
            ));
        }

        let weekly = get_claude_extra(p).and_then(|e| e.weekly_models.as_ref());
        if let Some(weekly) = weekly.filter(|w| !w.is_empty()) {
            lines.push(vline(ColorToken::Orange));
            lines.push(label_line(
                "Weekly per model",
                options.label_color,
                ColorToken::Orange,
            ));
            let w_max_len = weekly
                .keys()
                .map(|n| n.chars().count())
                .max()
                .unwrap_or(0)
                .max(max_len);
            for (name, window) in weekly {
                lines.push(model_line(
                    clock,
                    name,
                    Some(window),
                    w_max_len,
                    mode,
                    ColorToken::Orange,
                    None,
                ));
            }
        }

        if let Some(secondary) = p.secondary.as_ref() {
            lines.push(vline(ColorToken::Orange));
            lines.push(label_line(
                "Weekly limit (shared)",
                options.label_color,
                ColorToken::Orange,
            ));
            lines.push(model_line(
                clock,
                "All Models",
                Some(secondary),
                max_len,
                mode,
                ColorToken::Orange,
                None,
            ));
        }

        if let Some(eu) = get_claude_extra(p).and_then(|e| e.extra_usage.as_ref()) {
            if eu.enabled && eu.limit > 0.0 {
                let disp = to_display(Some(eu.remaining), mode);
                lines.push(vline(ColorToken::Orange));
                lines.push(label_line("Extra Usage", options.label_color, ColorToken::Orange));
                let used_str =
                    format!("${:.2}/${:.2}", eu.used / 100.0, eu.limit / 100.0);
                lines.push(extra_usage_line("Budget", max_len, disp, mode, &used_str));
            }
        }
    }

    lines.push(vline(ColorToken::Orange));
    lines.push(build_footer_line(
        clock,
        options.footer_fetched_at.as_deref(),
        ColorToken::Orange,
    ));

    lines
}
```

- [ ] **Step 4: Wire `builders/mod.rs`** — adicionar `pub mod claude;`.

- [ ] **Step 5: Rodar e confirmar que passa**

Run: `cargo test --manifest-path rust/Cargo.toml builders::claude 2>&1 | grep "test result"`
Expected: `test result: ok. 3 passed`.

- [ ] **Step 6: fmt + clippy + commit**

```bash
cargo fmt --manifest-path rust/Cargo.toml
cargo clippy --manifest-path rust/Cargo.toml --all-targets -- -D warnings
git add rust/src/formatters/builders/claude.rs rust/src/formatters/builders/mod.rs
git commit -m "feat(rust): builder claude"
```

---

## Task 5: Builder codex

**Files:**
- Create: `rust/src/formatters/builders/codex.rs`
- Modify: `rust/src/formatters/builders/mod.rs`

**Interfaces:**
- Consumes: tudo de `builders::shared`, `segments`, `shared::{format_percent, to_display}`, `view_model::CodexViewModel`, `providers::extras::get_codex_extra`, `settings::WindowPolicy`, `theme::{box_chars, ColorToken}`, `clock::Clock`, `providers::types::ProviderQuota`.
- Produces: `pub fn build_codex(clock: &Clock, p: &ProviderQuota, view_model: &CodexViewModel, options: &BuildOptions) -> Vec<Line>`.

Port FIEL de `src/formatters/builders/codex.ts`. Cor de marca = `Green`. `max_len = 20`, `null_eta_text = Some("N/A")` em todos os `model_line`. Ordem: header → vline → (error | conteúdo) → vline → footer.

Conteúdo (sem error): Plan line (se `plan_label` Some, ANTES de tudo); se `models` vazio → seção `Available Models` + "No models selected"; senão `model_len = max(maior nome, 20)`, depois seção `5-hour limit` (se policy != SevenDay) e `7-day limit` (se policy != FiveHour). Por fim Credits (se `extraUsage.enabled`): `limit == -1.0 → "Unlimited"`, senão `"Balance"`; linha de Balance com nome "Balance" padded.

- [ ] **Step 1: Escrever os testes que falham**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::formatters::builders::shared::{AmpLayout, BuildOptions};
    use crate::formatters::clock::Clock;
    use crate::formatters::codex_helpers::CodexModelEntry;
    use crate::formatters::render_pango::render_pango;
    use crate::formatters::view_model::CodexViewModel;
    use crate::providers::types::{ModelWindows, ProviderQuota, QuotaWindow};
    use crate::settings::{DisplayMode, WindowPolicy};
    use crate::theme::ColorToken;
    use time::macros::datetime;

    fn clk() -> Clock {
        Clock {
            now: datetime!(2026-06-19 12:00:00 UTC),
            local_offset: time::UtcOffset::UTC,
        }
    }

    fn opts(plan_label: Option<&str>) -> BuildOptions {
        BuildOptions {
            mode: DisplayMode::Remaining,
            header_title: "Codex".into(),
            header_width: 52,
            label_color: ColorToken::Magenta,
            footer_fetched_at: None,
            plan_label: plan_label.map(|s| s.to_string()),
            amp_free_tier_layout: AmpLayout::Inline,
            account_in_header: false,
        }
    }

    fn quota() -> ProviderQuota {
        ProviderQuota {
            provider: "codex".into(),
            display_name: "Codex".into(),
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

    fn entry(name: &str, five: f64, seven: f64) -> CodexModelEntry {
        let w = |r: f64| QuotaWindow {
            remaining: r,
            resets_at: Some("2026-06-19T14:00:00Z".into()),
            window_minutes: None,
            used: None,
        };
        CodexModelEntry {
            name: name.into(),
            windows: ModelWindows {
                five_hour: Some(w(five)),
                seven_day: Some(w(seven)),
                other: None,
            },
            severity: five.min(seven),
        }
    }

    #[test]
    fn both_policy_renders_both_sections() {
        let vm = CodexViewModel {
            models: vec![entry("gpt-5", 80.0, 50.0)],
            policy: WindowPolicy::Both,
        };
        let out = render_pango(&build_codex(&clk(), &quota(), &vm, &opts(Some("Pro"))));
        assert!(out.contains("Plan: Pro"));
        let i5 = out.find("5-hour limit").unwrap();
        let i7 = out.find("7-day limit").unwrap();
        assert!(i5 < i7);
        // null_eta_text "N/A" aparece quando não há resets... aqui há resets, então ETA real.
    }

    #[test]
    fn five_hour_policy_hides_seven_day() {
        let vm = CodexViewModel {
            models: vec![entry("gpt-5", 80.0, 50.0)],
            policy: WindowPolicy::FiveHour,
        };
        let out = render_pango(&build_codex(&clk(), &quota(), &vm, &opts(None)));
        assert!(out.contains("5-hour limit"));
        assert!(!out.contains("7-day limit"));
        assert!(!out.contains("Plan:")); // plan_label None → sem linha
    }

    #[test]
    fn empty_models_shows_placeholder() {
        let vm = CodexViewModel {
            models: vec![],
            policy: WindowPolicy::Both,
        };
        let out = render_pango(&build_codex(&clk(), &quota(), &vm, &opts(None)));
        assert!(out.contains("Available Models"));
        assert!(out.contains("No models selected"));
    }
}
```

- [ ] **Step 2: Rodar e confirmar que falha**

Run: `cargo test --manifest-path rust/Cargo.toml builders::codex 2>&1 | tail -15`
Expected: erro de compilação.

- [ ] **Step 3: Implementar**

```rust
//! Builder do card Codex. Port fiel de `src/formatters/builders/codex.ts`.
//! Cor de marca = Green. Recebe o CodexViewModel já resolvido pela superfície.

use crate::formatters::clock::Clock;
use crate::formatters::segments::{
    bar_segments, color_for_display, indicator_segments, Line, Segment,
};
use crate::formatters::shared::{format_percent, to_display};
use crate::formatters::view_model::CodexViewModel;
use crate::providers::extras::get_codex_extra;
use crate::providers::types::ProviderQuota;
use crate::settings::WindowPolicy;
use crate::theme::{box_chars, ColorToken};

use super::shared::{build_footer_line, header_line, label_line, model_line, vline, BuildOptions};

pub fn build_codex(
    clock: &Clock,
    p: &ProviderQuota,
    view_model: &CodexViewModel,
    options: &BuildOptions,
) -> Vec<Line> {
    let mode = options.mode;
    let mut lines: Vec<Line> = Vec::new();

    lines.push(header_line(
        &options.header_title,
        options.header_width,
        ColorToken::Green,
    ));
    lines.push(vline(ColorToken::Green));

    if let Some(err) = p.error.as_deref() {
        lines.push(vec![
            Segment::new(box_chars::V, ColorToken::Green),
            Segment::raw_text("  "),
            Segment::new(format!("⚠️ {err}"), ColorToken::Red),
        ]);
    } else {
        let models = &view_model.models;
        let policy = view_model.policy;
        let max_len = 20;

        if let Some(plan_label) = options.plan_label.as_deref() {
            lines.push(vec![
                Segment::new(box_chars::V, ColorToken::Green),
                Segment::raw_text("  "),
                Segment::new(format!("Plan: {plan_label}"), ColorToken::Muted),
            ]);
        }

        if models.is_empty() {
            lines.push(vline(ColorToken::Green));
            lines.push(label_line("Available Models", options.label_color, ColorToken::Green));
            lines.push(vec![
                Segment::new(box_chars::V, ColorToken::Green),
                Segment::raw_text("  "),
                Segment::new("No models selected", ColorToken::Comment),
            ]);
        } else {
            let model_len = models
                .iter()
                .map(|m| m.name.chars().count())
                .max()
                .unwrap_or(0)
                .max(max_len);

            if policy != WindowPolicy::SevenDay {
                lines.push(vline(ColorToken::Green));
                lines.push(label_line("5-hour limit", options.label_color, ColorToken::Green));
                for model in models {
                    lines.push(model_line(
                        clock,
                        &model.name,
                        model.windows.five_hour.as_ref(),
                        model_len,
                        mode,
                        ColorToken::Green,
                        Some("N/A"),
                    ));
                }
            }

            if policy != WindowPolicy::FiveHour {
                lines.push(vline(ColorToken::Green));
                lines.push(label_line("7-day limit", options.label_color, ColorToken::Green));
                for model in models {
                    lines.push(model_line(
                        clock,
                        &model.name,
                        model.windows.seven_day.as_ref(),
                        model_len,
                        mode,
                        ColorToken::Green,
                        Some("N/A"),
                    ));
                }
            }
        }

        if let Some(eu) = get_codex_extra(p).and_then(|e| e.extra_usage.as_ref()) {
            if eu.enabled {
                let disp = to_display(Some(eu.remaining), mode);
                lines.push(vline(ColorToken::Green));
                lines.push(label_line("Credits", options.label_color, ColorToken::Green));
                let limit_text = if eu.limit == -1.0 { "Unlimited" } else { "Balance" };
                let mut line: Line = vec![
                    Segment::new(box_chars::V, ColorToken::Green),
                    Segment::raw_text("  "),
                ];
                line.extend(indicator_segments(disp, mode));
                line.push(Segment::raw_text(" "));
                line.push(Segment::new(
                    format!("{:<max_len$}", "Balance"),
                    ColorToken::TextBright,
                ));
                line.push(Segment::raw_text(" "));
                line.extend(bar_segments(disp, mode));
                line.push(Segment::raw_text(" "));
                line.push(Segment::new(
                    format!("{:>4}", format_percent(disp)),
                    color_for_display(disp, mode),
                ));
                line.push(Segment::raw_text(" "));
                line.push(Segment::new(limit_text.to_string(), ColorToken::Cyan));
                lines.push(line);
            }
        }
    }

    lines.push(vline(ColorToken::Green));
    lines.push(build_footer_line(
        clock,
        options.footer_fetched_at.as_deref(),
        ColorToken::Green,
    ));

    lines
}
```

- [ ] **Step 4: Wire `builders/mod.rs`** — adicionar `pub mod codex;`.

- [ ] **Step 5: Rodar e confirmar que passa**

Run: `cargo test --manifest-path rust/Cargo.toml builders::codex 2>&1 | grep "test result"`
Expected: `test result: ok. 3 passed`.

- [ ] **Step 6: fmt + clippy + commit**

```bash
cargo fmt --manifest-path rust/Cargo.toml
cargo clippy --manifest-path rust/Cargo.toml --all-targets -- -D warnings
git add rust/src/formatters/builders/codex.rs rust/src/formatters/builders/mod.rs
git commit -m "feat(rust): builder codex"
```

---

## Task 6: Builder amp

**Files:**
- Create: `rust/src/formatters/builders/amp.rs`
- Modify: `rust/src/formatters/builders/mod.rs`

**Interfaces:**
- Consumes: tudo de `builders::shared`, `segments`, `shared::{format_percent, to_display, eta_label, format_eta, format_reset_time}`, `providers::extras::get_amp_extra`, `theme::{box_chars, ColorToken}`, `clock::Clock`, `providers::types::ProviderQuota`, `builders::shared::AmpLayout`.
- Produces: `pub fn build_amp(clock: &Clock, p: &ProviderQuota, options: &BuildOptions) -> Vec<Line>`.

Port FIEL de `src/formatters/builders/amp.ts`. Cor de marca = `Magenta`. Três layouts via `options.amp_free_tier_layout`: `Generic` (TUI: loop simples de `p.models`), `Sublines` (Terminal: barra sem ETA + sub-linhas `├─`/`└─`), `Inline` (Waybar: barra com ETA inline + linha `○` de dólares). Depois Credits (sublines+inline), fallback Usage, Account line (se `account` && !`account_in_header`), vline, footer.

**Semântica de `meta` (truthy do TS):** `replenishRate`/`freeRemaining`/`freeTotal`/`bonus` usam truthy (presente E não-vazio). `creditsBalance` usa `?? '$0'` (nullish): mantém `""` se presente, só cai em `$0` se ausente. Helper `meta_get` para truthy; acesso direto + `unwrap_or("$0")` para creditsBalance.

**Dólares:** `[freeRemaining, freeTotal]` filtrados truthy, juntados por `" / "`.

- [ ] **Step 1: Escrever os testes que falham**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::formatters::builders::shared::{AmpLayout, BuildOptions};
    use crate::formatters::clock::Clock;
    use crate::formatters::render_pango::render_pango;
    use crate::providers::types::{AmpQuotaExtra, ProviderExtra, ProviderQuota, QuotaWindow};
    use crate::settings::DisplayMode;
    use crate::theme::ColorToken;
    use std::collections::BTreeMap;
    use time::macros::datetime;

    fn clk() -> Clock {
        Clock {
            now: datetime!(2026-06-19 12:00:00 UTC),
            local_offset: time::UtcOffset::UTC,
        }
    }

    fn opts(layout: AmpLayout) -> BuildOptions {
        BuildOptions {
            mode: DisplayMode::Remaining,
            header_title: "Amp".into(),
            header_width: 52,
            label_color: ColorToken::Magenta,
            footer_fetched_at: None,
            plan_label: None,
            amp_free_tier_layout: layout,
            account_in_header: false,
        }
    }

    fn win(r: f64) -> QuotaWindow {
        QuotaWindow {
            remaining: r,
            resets_at: Some("2026-06-19T14:00:00Z".into()),
            window_minutes: None,
            used: None,
        }
    }

    fn amp_with_free_and_credits() -> ProviderQuota {
        let mut models = BTreeMap::new();
        models.insert("Free Tier".to_string(), win(30.0));
        models.insert("Credits".to_string(), win(75.0));
        let mut meta = BTreeMap::new();
        meta.insert("freeRemaining".to_string(), "$1.50".to_string());
        meta.insert("freeTotal".to_string(), "$5.00".to_string());
        meta.insert("replenishRate".to_string(), "+$0.10/h".to_string());
        meta.insert("creditsBalance".to_string(), "$12".to_string());
        ProviderQuota {
            provider: "amp".into(),
            display_name: "Amp".into(),
            available: true,
            account: Some("me@x.com".into()),
            plan: None,
            plan_type: None,
            primary: Some(win(30.0)),
            secondary: None,
            models: Some(models),
            extra: Some(ProviderExtra::Amp(AmpQuotaExtra { meta: Some(meta) })),
            error: None,
        }
    }

    #[test]
    fn inline_layout_has_free_tier_and_credits_and_account() {
        let out = render_pango(&build_amp(&clk(), &amp_with_free_and_credits(), &opts(AmpLayout::Inline)));
        assert!(out.contains("Free Tier"));
        assert!(out.contains("Credits"));
        assert!(out.contains("$12 remaining")); // inline anexa " remaining"
        assert!(out.contains("Account: me@x.com"));
        // ○ line de dólares (inline usa separador "  |  ")
        assert!(out.contains("$1.50 / $5.00"));
    }

    #[test]
    fn sublines_layout_uses_tree_connectors() {
        let out = render_pango(&build_amp(&clk(), &amp_with_free_and_credits(), &opts(AmpLayout::Sublines)));
        assert!(out.contains("Free Tier"));
        // sublines: dólares entre parênteses + último connector └─
        assert!(out.contains("( $1.50 / $5.00 )"));
        assert!(out.contains("└─"));
        // sublines NÃO anexa " remaining" ao balance
        assert!(out.contains("$12"));
    }

    #[test]
    fn generic_layout_loops_models() {
        // generic ignora special-casing de Free Tier/Credits e itera p.models.
        let q = amp_with_free_and_credits();
        let out = render_pango(&build_amp(&clk(), &q, &opts(AmpLayout::Generic)));
        assert!(out.contains("Usage"));
        // Account line é independente do layout → presente também no generic.
        assert!(out.contains("Account: me@x.com"));
    }

    #[test]
    fn account_omitted_when_in_header() {
        let mut o = opts(AmpLayout::Inline);
        o.account_in_header = true;
        let out = render_pango(&build_amp(&clk(), &amp_with_free_and_credits(), &o));
        assert!(!out.contains("Account:"));
    }

    #[test]
    fn error_branch() {
        let mut q = amp_with_free_and_credits();
        q.error = Some("rate limited".into());
        let out = render_pango(&build_amp(&clk(), &q, &opts(AmpLayout::Inline)));
        assert!(out.contains("⚠️ rate limited"));
        assert!(!out.contains("Free Tier"));
    }
}
```

- [ ] **Step 2: Rodar e confirmar que falha**

Run: `cargo test --manifest-path rust/Cargo.toml builders::amp 2>&1 | tail -15`
Expected: erro de compilação.

- [ ] **Step 3: Implementar**

```rust
//! Builder do card Amp. Port fiel de `src/formatters/builders/amp.ts`.
//! Cor de marca = Magenta. Três layouts de Free Tier: Generic/Sublines/Inline.

use std::collections::BTreeMap;

use crate::formatters::clock::Clock;
use crate::formatters::segments::{
    bar_segments, color_for_display, indicator_segments, Line, Segment,
};
use crate::formatters::shared::{eta_label, format_eta, format_percent, format_reset_time, to_display};
use crate::providers::extras::get_amp_extra;
use crate::providers::types::ProviderQuota;
use crate::settings::DisplayMode;
use crate::theme::{box_chars, ColorToken};

use super::shared::{build_footer_line, header_line, label_line, vline, AmpLayout, BuildOptions};

/// Valor de meta com semântica truthy do JS: Some só se presente E não-vazio.
fn meta_get<'a>(m: &'a BTreeMap<String, String>, k: &str) -> Option<&'a str> {
    m.get(k).map(|s| s.as_str()).filter(|s| !s.is_empty())
}

/// Junta freeRemaining/freeTotal (truthy) com " / ".
fn dollars(m: &BTreeMap<String, String>) -> String {
    [meta_get(m, "freeRemaining"), meta_get(m, "freeTotal")]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" / ")
}

/// Linha da barra do Free Tier (compartilhada por sublines/inline). `eta_segments`
/// é anexado ao fim (inline anexa ETA; sublines passa vazio).
fn free_tier_bar_line(disp: Option<f64>, mode: DisplayMode, eta_segments: Line) -> Line {
    let mut line: Line = vec![
        Segment::new(box_chars::V, ColorToken::Magenta),
        Segment::raw_text("  "),
    ];
    line.extend(indicator_segments(disp, mode));
    line.push(Segment::raw_text(" "));
    line.extend(bar_segments(disp, mode));
    line.push(Segment::raw_text(" "));
    line.push(Segment::new(
        format!("{:>4}", format_percent(disp)),
        color_for_display(disp, mode),
    ));
    line.extend(eta_segments);
    line
}

/// Linha genérica de model (indicador + nome + barra + pct), sem ETA.
fn generic_model_line(name: &str, max_len: usize, disp: Option<f64>, mode: DisplayMode) -> Line {
    let mut line: Line = vec![
        Segment::new(box_chars::V, ColorToken::Magenta),
        Segment::raw_text("  "),
    ];
    line.extend(indicator_segments(disp, mode));
    line.push(Segment::raw_text(" "));
    line.push(Segment::new(format!("{name:<max_len$}"), ColorToken::TextBright));
    line.push(Segment::raw_text(" "));
    line.extend(bar_segments(disp, mode));
    line.push(Segment::raw_text(" "));
    line.push(Segment::new(
        format!("{:>4}", format_percent(disp)),
        color_for_display(disp, mode),
    ));
    line
}

pub fn build_amp(clock: &Clock, p: &ProviderQuota, options: &BuildOptions) -> Vec<Line> {
    let mode = options.mode;
    let layout = options.amp_free_tier_layout;
    let empty_meta = BTreeMap::new();
    let m: &BTreeMap<String, String> = get_amp_extra(p)
        .and_then(|e| e.meta.as_ref())
        .unwrap_or(&empty_meta);

    let mut lines: Vec<Line> = Vec::new();

    lines.push(header_line(
        &options.header_title,
        options.header_width,
        ColorToken::Magenta,
    ));
    lines.push(vline(ColorToken::Magenta));

    if let Some(err) = p.error.as_deref() {
        lines.push(vec![
            Segment::new(box_chars::V, ColorToken::Magenta),
            Segment::raw_text("  "),
            Segment::new(format!("⚠️ {err}"), ColorToken::Red),
        ]);
    } else if layout == AmpLayout::Generic {
        // TUI: loop genérico, sem special-casing de Free Tier/Credits.
        match p.models.as_ref().filter(|mm| !mm.is_empty()) {
            None => lines.push(vec![
                Segment::new(box_chars::V, ColorToken::Magenta),
                Segment::raw_text("  "),
                Segment::new("No usage data", ColorToken::Muted),
            ]),
            Some(models) => {
                let max_len = models
                    .keys()
                    .map(|n| n.chars().count())
                    .max()
                    .unwrap_or(0)
                    .max(20);
                lines.push(label_line("Usage", options.label_color, ColorToken::Magenta));
                for (name, window) in models {
                    let disp = to_display(Some(window.remaining), mode);
                    lines.push(generic_model_line(name, max_len, disp, mode));
                }
            }
        }
    } else {
        // Terminal (sublines) e Waybar (inline).
        let free = p.models.as_ref().and_then(|mm| mm.get("Free Tier"));

        if let Some(free) = free {
            let rem = free.remaining;
            let disp = to_display(Some(rem), mode);
            lines.push(label_line("Free Tier", options.label_color, ColorToken::Magenta));

            if layout == AmpLayout::Sublines {
                lines.push(free_tier_bar_line(disp, mode, Vec::new()));

                let mut subs: Vec<Line> = Vec::new();

                // Sub-linha de dólares: replenishRate  ( freeRemaining / freeTotal )  bonus
                let mut dollar_parts: Line = Vec::new();
                if let Some(rate) = meta_get(m, "replenishRate") {
                    dollar_parts.push(Segment::new(rate.to_string(), ColorToken::Cyan));
                }
                let d = dollars(m);
                if !d.is_empty() {
                    if !dollar_parts.is_empty() {
                        dollar_parts.push(Segment::raw_text("  "));
                    }
                    dollar_parts.push(Segment::new(format!("( {d} )"), ColorToken::Text));
                }
                if let Some(bonus) = meta_get(m, "bonus") {
                    if !dollar_parts.is_empty() {
                        dollar_parts.push(Segment::raw_text("  "));
                    }
                    dollar_parts.push(Segment::new(bonus.to_string(), ColorToken::Cyan));
                }
                if !dollar_parts.is_empty() {
                    subs.push(dollar_parts);
                }

                // Sub-linha de ETA (só com resets e não cheio).
                if free.resets_at.is_some() && rem != 100.0 {
                    let eta_text = format!(
                        "{} {}  {}",
                        eta_label(mode),
                        format_eta(clock, free.resets_at.as_deref(), rem),
                        format_reset_time(clock, free.resets_at.as_deref(), rem)
                    );
                    subs.push(vec![Segment::new(eta_text, ColorToken::Cyan)]);
                }

                let last = subs.len().saturating_sub(1);
                for (i, sub) in subs.into_iter().enumerate() {
                    let conn = if i == last { "└─" } else { "├─" };
                    let mut line: Line = vec![
                        Segment::new(box_chars::V, ColorToken::Magenta),
                        Segment::raw_text("  "),
                        Segment::new(conn, ColorToken::Comment),
                        Segment::raw_text(" "),
                    ];
                    line.extend(sub);
                    lines.push(line);
                }
            } else {
                // Inline (Waybar): barra com ETA anexado; dólares na linha ○.
                let eta_segs: Line = if free.resets_at.is_some() && rem != 100.0 {
                    vec![
                        Segment::raw_text("  "),
                        Segment::new(
                            format!(
                                "→ {} {} {}",
                                eta_label(mode),
                                format_eta(clock, free.resets_at.as_deref(), rem),
                                format_reset_time(clock, free.resets_at.as_deref(), rem)
                            ),
                            ColorToken::Cyan,
                        ),
                    ]
                } else {
                    Vec::new()
                };
                lines.push(free_tier_bar_line(disp, mode, eta_segs));

                let mut info_parts: Line = Vec::new();
                if let Some(rate) = meta_get(m, "replenishRate") {
                    info_parts.push(Segment::new(rate.to_string(), ColorToken::Cyan));
                }
                let d = dollars(m);
                if !d.is_empty() {
                    if !info_parts.is_empty() {
                        info_parts.push(Segment::new("  |  ", ColorToken::Comment));
                    }
                    info_parts.push(Segment::new(d, ColorToken::Text));
                }
                if let Some(bonus) = meta_get(m, "bonus") {
                    if !info_parts.is_empty() {
                        info_parts.push(Segment::new("  |  ", ColorToken::Comment));
                    }
                    info_parts.push(Segment::new(bonus.to_string(), ColorToken::Cyan));
                }
                if !info_parts.is_empty() {
                    let mut line: Line = vec![
                        Segment::new(box_chars::V, ColorToken::Magenta),
                        Segment::raw_text("  "),
                        Segment::new(box_chars::DOT_O, ColorToken::Comment),
                        Segment::raw_text(" "),
                    ];
                    line.extend(info_parts);
                    lines.push(line);
                }
            }
        }

        // Credits (terminal + waybar).
        let credits = p.models.as_ref().and_then(|mm| mm.get("Credits"));
        if let Some(credits) = credits {
            lines.push(vline(ColorToken::Magenta));
            let balance = m.get("creditsBalance").map(|s| s.as_str()).unwrap_or("$0");
            let credit_color = if credits.remaining > 0.0 {
                ColorToken::Green
            } else {
                ColorToken::Comment
            };
            lines.push(label_line("Credits", options.label_color, ColorToken::Magenta));
            let credit_disp = to_display(Some(credits.remaining), mode);
            let balance_text = if layout == AmpLayout::Inline {
                format!("{balance} remaining")
            } else {
                balance.to_string()
            };
            let mut line: Line = vec![
                Segment::new(box_chars::V, ColorToken::Magenta),
                Segment::raw_text("  "),
            ];
            line.extend(indicator_segments(credit_disp, mode));
            line.push(Segment::raw_text(" "));
            line.push(Segment::new(balance_text, credit_color));
            lines.push(line);
        }

        // Fallback p/ models desconhecidos (nem Free Tier nem Credits).
        if free.is_none() && credits.is_none() {
            if let Some(models) = p.models.as_ref().filter(|mm| !mm.is_empty()) {
                let max_len = models
                    .keys()
                    .map(|n| n.chars().count())
                    .max()
                    .unwrap_or(0)
                    .max(20);
                lines.push(label_line("Usage", options.label_color, ColorToken::Magenta));
                for (name, window) in models {
                    let disp = to_display(Some(window.remaining), mode);
                    lines.push(generic_model_line(name, max_len, disp, mode));
                }
            }
        }
    }

    // Account line — omitida quando a superfície já mostra a conta no header.
    if let Some(account) = p.account.as_deref().filter(|s| !s.is_empty()) {
        if !options.account_in_header {
            lines.push(vline(ColorToken::Magenta));
            lines.push(vec![
                Segment::new(box_chars::V, ColorToken::Magenta),
                Segment::raw_text("  "),
                Segment::new(format!("Account: {account}"), ColorToken::Comment),
            ]);
        }
    }

    lines.push(vline(ColorToken::Magenta));
    lines.push(build_footer_line(
        clock,
        options.footer_fetched_at.as_deref(),
        ColorToken::Magenta,
    ));

    lines
}
```

- [ ] **Step 4: Wire `builders/mod.rs`** — adicionar `pub mod amp;`. Estado final do arquivo:

```rust
pub mod amp;
pub mod claude;
pub mod codex;
pub mod generic;
pub mod shared;
```

- [ ] **Step 5: Rodar e confirmar que passa**

Run: `cargo test --manifest-path rust/Cargo.toml builders::amp 2>&1 | grep "test result"`
Expected: `test result: ok. 5 passed`.

- [ ] **Step 6: Rodar a suíte inteira (regressão) + clippy**

Run: `cargo test --manifest-path rust/Cargo.toml 2>&1 | grep -E "test result|error\[" ` (somar `passed` de TODAS as suítes — RTK trunca; conferir cada `test result:`).
Run: `cargo clippy --manifest-path rust/Cargo.toml --all-targets -- -D warnings 2>&1 | tail -5`
Expected: tudo `ok`, zero warnings.

- [ ] **Step 7: fmt + commit**

```bash
cargo fmt --manifest-path rust/Cargo.toml
git add rust/src/formatters/builders/amp.rs rust/src/formatters/builders/mod.rs
git commit -m "feat(rust): builder amp (sublines/inline/generic)"
```

---

## Self-Review (do autor do plano)

- **Cobertura do spec:** os 4 builders + extras getters + codex model-prep + CodexViewModel + normalize_plan_label estão cobertos (1 task cada, exceto helpers Codex+VM+plan-label agrupados na Task 2). ✓
- **Placeholders:** nenhum — todo código está completo e transcrito do TS-fonte. ✓
- **Consistência de tipos:** `BuildOptions`/`AmpLayout` consumidos de `builders::shared` (já existem, 3a); `model_line`/`vline`/`label_line`/`header_line`/`build_footer_line` com as assinaturas reais lidas de `shared.rs`; `Clock` injetado em todos. ✓
- **Riscos registrados:** ordem de iteração BTreeMap vs insertion-order (nota de fidelidade — validar no 3c); `localeCompare` vs `str::cmp` (Minor); `toFixed` vs `{:.2}` (sem impacto: centavos inteiros); `.length` vs `chars().count()` (escolha do projeto desde 3a). Nenhum bloqueia; todos a confirmar contra golden no 3c.
- **Decisão pendente herdada:** JSON `60.0`/`60` continua para o Plano 3c (não toca este plano).
