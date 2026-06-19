# Reescrita Rust — Plano 03c: Assembly de superfícies + golden snapshots

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development para executar este plano task-a-task. Steps usam checkbox (`- [ ]`).

**Goal:** Montar as superfícies de saída (terminal ANSI + Waybar `{text,tooltip,class}`) chamando os builders do 3b, e travar a **paridade byte-exact total** com o TS via golden snapshots (`insta`).

**Architecture:** `terminal.rs` e `waybar.rs` são funções PURAS que recebem `&AllQuotas`/`&ProviderQuota` + `&Settings` + `&Clock` injetados e produzem String (terminal) ou `WaybarOutput` (waybar). Elas resolvem o `BuildOptions` por-superfície (header title/width, labelColor, footer, ampFreeTierLayout, accountInHeader) e chamam `build_{claude,codex,amp,generic}` do 3b. O teste de paridade (`tests/golden.rs`) espelha as factories e os cenários do `formatters-snapshot.test.ts` do TS, renderiza com Clock fixo, sanitiza as partes time-dependent com os MESMOS regex do TS, e compara byte-a-byte com os valores do `.snap` TS via `insta`.

**Tech Stack:** Rust 1.95, `insta` (dev-dependency, com `filters` para sanitização). Sem deps de runtime novas.

## Global Constraints

Valores verbatim do TS-fonte (`src/formatters/waybar.ts`, `terminal.ts`, `tests/formatters-snapshot.test.ts`).

- **Contrato byte-exact do Waybar/Pango é a autoridade = saída do TS.** O golden (T4) é o juiz final. Divergência byte-a-byte = bug a corrigir, EXCETO a ordem de iteração de maps (BTreeMap vs insertion-order) que é desvio conhecido e registrado — se aparecer, documentar, não "consertar" mudando o tipo aqui.
- **Funções de assembly são PURAS:** recebem `&Settings` e `&Clock` injetados (sem leitura de disco, sem relógio de sistema, sem estado global mutável). O **cache 5s de settings** e o **hidden-module short-circuit** NÃO entram aqui — são do hot path e ficam para o Plano 5 (CLI dispatch). YAGNI no 3c.
- **stdout limpo:** estas funções retornam String/struct; quem imprime é o CLI (Plano 5). Não usar `println!` no código de produção do 3c (exceto onde o TS usa `console.log` em `outputX` — porte essas como `print`/funções finas, mas o foco do 3c são as funções `format_*` puras).
- **XML-escape só em `render_pango`/`span`.** O assembly nunca escapa manualmente; usa `span(hex, text, false)` para texto de provider.
- **Cores de marca e larguras por superfície (verbatim do TS):**
  - Waybar tooltip: `headerWidth = TOOLTIP_BORDER - 4 = 52`; footer = `Some(fetchedAt)`. Claude labelColor=`orange`, Codex=`green`, Amp=`magenta` + `ampFreeTierLayout=Inline` + `accountInHeader=true`, generic=`text`.
  - Terminal: `headerWidth = 56` (generic = `52`); footer = `None`; labelColor SEMPRE `magenta` (claude/codex/amp), generic=`text`; Amp `ampFreeTierLayout=Sublines`; Codex passa `planLabel = Some(normalize_plan_label(p))`.
  - Waybar NÃO passa planLabel (embute no headerTitle); Terminal passa planLabel ao Codex.
- **Class tokens (verbatim):** agregado (`format_for_waybar`) = `agent-bar` + ` ` + `{provider}-{status}` por provider available (ex: `agent-bar claude-ok`). Por-provider (`format_provider_for_waybar`) = `agent-bar-{provider} {status}` (ex: `agent-bar-claude ok`); disconnected = `agent-bar-{provider} disconnected`.
- **HealthStatus serializa lowercase:** `ok`/`low`/`warn`/`critical` (= TS `type HealthStatus`).
- **`status` para class/alt usa `remaining` CRU**, não o display value: `status_for_percent(Some(rem ?? 100))`.
- **Sem `unwrap()`/`expect()` em produção** (permitido em `#[cfg(test)]`). Sem `!` non-null assertion.
- **Verificação:** `cargo test --manifest-path rust/Cargo.toml` + `cargo clippy --manifest-path rust/Cargo.toml --all-targets -- -D warnings`. RTK reformata o output do cargo (`cargo test: N passed (...)`, sem `test result:`) — leia o output bruto com `tail`.
- **`cargo fmt --manifest-path rust/Cargo.toml` ANTES de `git add`.** Commits: Conventional Commits PT ≤50 chars.

### Invariante documentada (do review de branch 03b)

`ProviderQuota.error` nunca é string vazia (contrato do lado do provider, Plano 4). Os builders tratam `Some("")` como erro presente — inalcançável na prática. Não corrigir aqui; documentar no Plano 4 ao construir os quotas.

---

## File Structure

- **Modify** `rust/src/config.rs` — add `HealthStatus::as_str(&self) -> &'static str`.
- **Modify** `rust/src/app_identity.rs` — add `pub const APP_BASE_CLASS: &str = APP_NAME;`.
- **Create** `rust/src/formatters/terminal.rs` — `format_for_terminal`.
- **Create** `rust/src/formatters/waybar.rs` — `WaybarOutput`, `format_for_waybar`, `format_provider_for_waybar`, helpers.
- **Modify** `rust/src/formatters/mod.rs` — add `pub mod terminal;` e `pub mod waybar;`.
- **Modify** `rust/Cargo.toml` — add `insta` em `[dev-dependencies]`.
- **Create** `rust/tests/golden.rs` — testes de paridade com os goldens do TS.

---

## Task 1: Prep — HealthStatus::as_str + APP_BASE_CLASS

**Files:**
- Modify: `rust/src/config.rs`
- Modify: `rust/src/app_identity.rs`

**Interfaces:**
- Produces: `HealthStatus::as_str(&self) -> &'static str` (`ok`/`low`/`warn`/`critical`); `app_identity::APP_BASE_CLASS: &str` (= "agent-bar").

- [ ] **Step 1: Teste que falha** — em `config.rs`, no `mod tests`:

```rust
    #[test]
    fn health_status_as_str() {
        assert_eq!(HealthStatus::Ok.as_str(), "ok");
        assert_eq!(HealthStatus::Low.as_str(), "low");
        assert_eq!(HealthStatus::Warn.as_str(), "warn");
        assert_eq!(HealthStatus::Critical.as_str(), "critical");
    }
```

E em `app_identity.rs`, no fim (criar `mod tests` se não existir):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_base_class_is_app_name() {
        assert_eq!(APP_BASE_CLASS, "agent-bar");
        assert_eq!(APP_BASE_CLASS, APP_NAME);
    }
}
```

- [ ] **Step 2: Confirmar falha** — `cargo test --manifest-path rust/Cargo.toml as_str app_base_class 2>&1 | tail -10` → erro de compilação.

- [ ] **Step 3: Implementar** — em `config.rs`, no `impl`/após o enum `HealthStatus`:

```rust
impl HealthStatus {
    /// Token lowercase para class CSS / `alt` do Waybar (= TS `type HealthStatus`).
    pub fn as_str(&self) -> &'static str {
        match self {
            HealthStatus::Ok => "ok",
            HealthStatus::Low => "low",
            HealthStatus::Warn => "warn",
            HealthStatus::Critical => "critical",
        }
    }
}
```

Em `app_identity.rs`, após `APP_NAME`:

```rust
/// Classe CSS base do Waybar (= APP_NAME). Fonte: TS `APP_BASE_CLASS = APP_NAME`.
pub const APP_BASE_CLASS: &str = APP_NAME;
```

- [ ] **Step 4: Confirmar passa** — `cargo test --manifest-path rust/Cargo.toml as_str app_base_class 2>&1 | tail -5` → ok.

- [ ] **Step 5: fmt + clippy + commit**

```bash
cargo fmt --manifest-path rust/Cargo.toml
cargo clippy --manifest-path rust/Cargo.toml --all-targets -- -D warnings
git add rust/src/config.rs rust/src/app_identity.rs
git commit -m "feat(rust): HealthStatus::as_str + APP_BASE_CLASS"
```

---

## Task 2: terminal.rs

**Files:**
- Create: `rust/src/formatters/terminal.rs`
- Modify: `rust/src/formatters/mod.rs`

**Interfaces:**
- Consumes: builders `build_{claude,codex,amp,generic}`; `render_ansi`; `view_model::resolve_codex_view_model_from`; `shared::normalize_plan_label`; `theme::{ColorToken, ANSI_RESET}`; `settings::{Settings, DisplayMode}`; `clock::Clock`; `providers::types::{AllQuotas, ProviderQuota}`; `builders::shared::{BuildOptions, AmpLayout}`.
- Produces: `pub fn format_for_terminal(clock: &Clock, quotas: &AllQuotas, settings: &Settings, mode: DisplayMode) -> String`.

Port FIEL de `src/formatters/terminal.ts`. Cada builder por-provider monta seu `BuildOptions` (ver Global Constraints). Empty → `{comment_ansi}No providers connected{reset}`. Seções juntadas por `\n\n`.

Nota: o TS usa `resolveCodexViewModel(p)` (loadSettingsSync). Aqui injetamos `settings` e chamamos `resolve_codex_view_model_from(settings, p)`. O Clock vai aos builders (footer=None no terminal, mas `model_line`/ETAs precisam dele).

- [ ] **Step 1: Teste que falha**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Paths;
    use crate::formatters::clock::Clock;
    use crate::providers::types::{AllQuotas, ProviderQuota, QuotaWindow};
    use crate::settings::load;
    use std::path::PathBuf;
    use tempfile::tempdir;
    use time::macros::datetime;

    fn clk() -> Clock {
        Clock {
            now: datetime!(2026-03-28 12:00:00 UTC),
            local_offset: time::UtcOffset::UTC,
        }
    }

    fn settings() -> crate::settings::Settings {
        let dir = tempdir().unwrap();
        load(&Paths {
            cache_dir: dir.path().join("cache"),
            config_dir: dir.path().join("config"),
            claude_credentials: PathBuf::new(),
            codex_auth: PathBuf::new(),
            codex_sessions: PathBuf::new(),
            amp_settings: PathBuf::new(),
            amp_threads: PathBuf::new(),
        })
    }

    fn claude() -> ProviderQuota {
        ProviderQuota {
            provider: "claude".into(),
            display_name: "Claude".into(),
            available: true,
            account: None,
            plan: Some("Pro".into()),
            plan_type: None,
            primary: Some(QuotaWindow {
                remaining: 75.0,
                resets_at: Some("2026-03-28T14:00:00Z".into()),
                window_minutes: Some(300),
                used: None,
            }),
            secondary: None,
            models: None,
            extra: None,
            error: None,
        }
    }

    #[test]
    fn renders_claude_section() {
        let q = AllQuotas {
            providers: vec![claude()],
            fetched_at: "2026-03-28T12:00:00Z".into(),
        };
        let out = format_for_terminal(&clk(), &q, &settings(), DisplayMode::Remaining);
        assert!(out.contains("Claude"));
        assert!(out.contains("75%"));
    }

    #[test]
    fn empty_when_no_providers() {
        let q = AllQuotas {
            providers: vec![],
            fetched_at: "2026-03-28T12:00:00Z".into(),
        };
        let out = format_for_terminal(&clk(), &q, &settings(), DisplayMode::Remaining);
        assert!(out.contains("No providers connected"));
    }

    #[test]
    fn skips_unavailable_without_error() {
        let mut c = claude();
        c.available = false;
        c.error = None;
        let q = AllQuotas {
            providers: vec![c],
            fetched_at: "2026-03-28T12:00:00Z".into(),
        };
        let out = format_for_terminal(&clk(), &q, &settings(), DisplayMode::Remaining);
        assert!(out.contains("No providers connected"));
    }
}
```

- [ ] **Step 2: Confirmar falha** — `cargo test --manifest-path rust/Cargo.toml formatters::terminal 2>&1 | tail -10`.

- [ ] **Step 3: Implementar**

```rust
//! Assembly da superfície terminal (ANSI). Port fiel de `src/formatters/terminal.ts`.
//! Funções puras: recebem Settings e Clock injetados.

use crate::formatters::builders::amp::build_amp;
use crate::formatters::builders::claude::build_claude;
use crate::formatters::builders::codex::build_codex;
use crate::formatters::builders::generic::build_generic;
use crate::formatters::builders::shared::{AmpLayout, BuildOptions};
use crate::formatters::clock::Clock;
use crate::formatters::render_ansi::render_ansi;
use crate::formatters::shared::normalize_plan_label;
use crate::formatters::view_model::resolve_codex_view_model_from;
use crate::providers::types::{AllQuotas, ProviderQuota};
use crate::settings::{DisplayMode, Settings};
use crate::theme::{ColorToken, ANSI_RESET};

fn terminal_section(
    clock: &Clock,
    p: &ProviderQuota,
    settings: &Settings,
    mode: DisplayMode,
) -> String {
    match p.provider.as_str() {
        "claude" => render_ansi(&build_claude(
            clock,
            p,
            &BuildOptions {
                mode,
                header_title: "Claude".into(),
                header_width: 56,
                label_color: ColorToken::Magenta,
                footer_fetched_at: None,
                plan_label: None,
                amp_free_tier_layout: AmpLayout::Inline,
                account_in_header: false,
            },
        )),
        "codex" => {
            let vm = resolve_codex_view_model_from(settings, p);
            render_ansi(&build_codex(
                clock,
                p,
                &vm,
                &BuildOptions {
                    mode,
                    header_title: "Codex".into(),
                    header_width: 56,
                    label_color: ColorToken::Magenta,
                    footer_fetched_at: None,
                    plan_label: Some(normalize_plan_label(p)),
                    amp_free_tier_layout: AmpLayout::Inline,
                    account_in_header: false,
                },
            ))
        }
        "amp" => render_ansi(&build_amp(
            clock,
            p,
            &BuildOptions {
                mode,
                header_title: "Amp".into(),
                header_width: 56,
                label_color: ColorToken::Magenta,
                footer_fetched_at: None,
                plan_label: None,
                amp_free_tier_layout: AmpLayout::Sublines,
                account_in_header: false,
            },
        )),
        _ => render_ansi(&build_generic(
            clock,
            p,
            &BuildOptions {
                mode,
                header_title: if p.display_name.is_empty() {
                    p.provider.clone()
                } else {
                    p.display_name.clone()
                },
                header_width: 52,
                label_color: ColorToken::Text,
                footer_fetched_at: None,
                plan_label: None,
                amp_free_tier_layout: AmpLayout::Inline,
                account_in_header: false,
            },
        )),
    }
}

pub fn format_for_terminal(
    clock: &Clock,
    quotas: &AllQuotas,
    settings: &Settings,
    mode: DisplayMode,
) -> String {
    let sections: Vec<String> = quotas
        .providers
        .iter()
        .filter(|p| p.available || p.error.is_some())
        .map(|p| terminal_section(clock, p, settings, mode))
        .collect();

    if sections.is_empty() {
        return format!(
            "{}No providers connected{}",
            ColorToken::Comment.ansi(),
            ANSI_RESET
        );
    }
    sections.join("\n\n")
}
```

> **Nota de fidelidade:** o TS usa `p.displayName ?? p.provider` para o generic. O Rust `display_name` é `String` (não-opcional); espelha o fallback com `if is_empty()`. Para os providers conhecidos o display_name não é usado (header é hardcoded).

- [ ] **Step 4: Wire `formatters/mod.rs`** — adicionar `pub mod terminal;` (posição alfabética: entre `segments`/`shared` e `view_model` → fica entre `shared` e `view_model`; resultado: `...segments; shared; terminal; view_model;`).

- [ ] **Step 5: Confirmar passa** — `cargo test --manifest-path rust/Cargo.toml formatters::terminal 2>&1 | tail -5` → 3 passed.

- [ ] **Step 6: fmt + clippy + commit**

```bash
cargo fmt --manifest-path rust/Cargo.toml
cargo clippy --manifest-path rust/Cargo.toml --all-targets -- -D warnings
git add rust/src/formatters/terminal.rs rust/src/formatters/mod.rs
git commit -m "feat(rust): assembly terminal (format_for_terminal)"
```

---

## Task 3: waybar.rs

**Files:**
- Create: `rust/src/formatters/waybar.rs`
- Modify: `rust/src/formatters/mod.rs`

**Interfaces:**
- Consumes: builders; `render_pango::{render_pango, span}`; `segments::color_for_display`; `shared::{format_percent, normalize_plan_label, to_window_display}`; `view_model::resolve_codex_view_model_from`; `config::{status_for_percent, HealthStatus}`; `app_identity::APP_BASE_CLASS`; `theme::ColorToken`; `settings::{Settings, DisplayMode}`; `clock::Clock`; `providers::types::{AllQuotas, ProviderQuota}`.
- Produces: `WaybarOutput` (serde Serialize), `format_for_waybar(clock, &AllQuotas, &Settings, mode) -> WaybarOutput`, `format_provider_for_waybar(clock, &ProviderQuota, &Settings, mode) -> WaybarOutput`.

Port FIEL de `src/formatters/waybar.ts`. **O glyph do ícone disconnected (`text` do disconnected) deve ser COPIADO verbatim de `src/formatters/waybar.ts:215`** (`<span foreground='{red}'>…</span>`) — não digitar o codepoint nerd-font de memória.

`WaybarOutput`:
```rust
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WaybarOutput {
    pub text: String,
    pub tooltip: String,
    pub class: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alt: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub percentage: Option<u8>,
}
```

- [ ] **Step 1: Teste que falha**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Paths;
    use crate::formatters::clock::Clock;
    use crate::providers::types::{AllQuotas, ProviderQuota, QuotaWindow};
    use crate::settings::load;
    use std::path::PathBuf;
    use tempfile::tempdir;
    use time::macros::datetime;

    fn clk() -> Clock {
        Clock {
            now: datetime!(2026-03-28 12:00:00 UTC),
            local_offset: time::UtcOffset::UTC,
        }
    }
    fn settings() -> crate::settings::Settings {
        let dir = tempdir().unwrap();
        load(&Paths {
            cache_dir: dir.path().join("cache"),
            config_dir: dir.path().join("config"),
            claude_credentials: PathBuf::new(),
            codex_auth: PathBuf::new(),
            codex_sessions: PathBuf::new(),
            amp_settings: PathBuf::new(),
            amp_threads: PathBuf::new(),
        })
    }
    fn claude(remaining: f64) -> ProviderQuota {
        ProviderQuota {
            provider: "claude".into(),
            display_name: "Claude".into(),
            available: true,
            account: None,
            plan: Some("Pro".into()),
            plan_type: None,
            primary: Some(QuotaWindow {
                remaining,
                resets_at: Some("2026-03-28T14:00:00Z".into()),
                window_minutes: Some(300),
                used: None,
            }),
            secondary: None,
            models: None,
            extra: None,
            error: None,
        }
    }

    #[test]
    fn aggregate_class_format() {
        let q = AllQuotas {
            providers: vec![claude(75.0)],
            fetched_at: "2026-03-28T12:00:00Z".into(),
        };
        let out = format_for_waybar(&clk(), &q, &settings(), DisplayMode::Remaining);
        assert_eq!(out.class, "agent-bar claude-ok");
        assert!(out.text.contains("75%"));
        assert!(out.tooltip.contains("Claude · Pro"));
        assert!(out.alt.is_none()); // agregado não tem alt
    }

    #[test]
    fn per_provider_class_and_alt() {
        let out =
            format_provider_for_waybar(&clk(), &claude(5.0), &settings(), DisplayMode::Remaining);
        assert_eq!(out.class, "agent-bar-claude critical"); // 5% < 10 → critical
        assert_eq!(out.alt.as_deref(), Some("critical"));
        assert_eq!(out.percentage, Some(5));
    }

    #[test]
    fn per_provider_disconnected() {
        let mut c = claude(50.0);
        c.available = false;
        c.error = Some("token expired".into());
        let out = format_provider_for_waybar(&clk(), &c, &settings(), DisplayMode::Remaining);
        assert_eq!(out.class, "agent-bar-claude disconnected");
        assert_eq!(out.alt.as_deref(), Some("disconnected"));
        assert!(out.percentage.is_none());
    }

    #[test]
    fn aggregate_empty_text() {
        let q = AllQuotas {
            providers: vec![],
            fetched_at: "2026-03-28T12:00:00Z".into(),
        };
        let out = format_for_waybar(&clk(), &q, &settings(), DisplayMode::Remaining);
        assert!(out.text.contains("No Providers"));
        assert_eq!(out.class, "agent-bar");
    }
}
```

- [ ] **Step 2: Confirmar falha** — `cargo test --manifest-path rust/Cargo.toml formatters::waybar 2>&1 | tail -10`.

- [ ] **Step 3: Implementar**

```rust
//! Assembly da superfície Waybar ({text,tooltip,class}). Port fiel de
//! `src/formatters/waybar.ts`. Funções puras: Settings e Clock injetados.
//! O cache 5s de settings e o hidden-module short-circuit vivem no CLI (Plano 5).

use serde::Serialize;

use crate::app_identity::APP_BASE_CLASS;
use crate::config::{status_for_percent, HealthStatus};
use crate::formatters::builders::amp::build_amp;
use crate::formatters::builders::claude::build_claude;
use crate::formatters::builders::codex::build_codex;
use crate::formatters::builders::generic::build_generic;
use crate::formatters::builders::shared::{AmpLayout, BuildOptions, TOOLTIP_BORDER};
use crate::formatters::clock::Clock;
use crate::formatters::render_pango::{render_pango, span};
use crate::formatters::segments::color_for_display;
use crate::formatters::shared::{format_percent, normalize_plan_label, to_window_display};
use crate::formatters::view_model::resolve_codex_view_model_from;
use crate::providers::types::{AllQuotas, ProviderQuota};
use crate::settings::{DisplayMode, Settings};
use crate::theme::ColorToken;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WaybarOutput {
    pub text: String,
    pub tooltip: String,
    pub class: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alt: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub percentage: Option<u8>,
}

fn pct_colored(disp: Option<f64>, mode: DisplayMode) -> String {
    span(
        color_for_display(disp, mode).hex(),
        &format_percent(disp),
        false,
    )
}

fn header_width_waybar() -> usize {
    TOOLTIP_BORDER - 4
}

fn provider_tooltip(
    clock: &Clock,
    p: &ProviderQuota,
    fetched_at: Option<&str>,
    settings: &Settings,
    mode: DisplayMode,
) -> String {
    let fetched = fetched_at.map(|s| s.to_string());
    match p.provider.as_str() {
        "claude" => {
            let plan = normalize_plan_label(p);
            let title = if plan != "Unknown" {
                format!("Claude · {plan}")
            } else {
                "Claude".to_string()
            };
            render_pango(&build_claude(
                clock,
                p,
                &BuildOptions {
                    mode,
                    header_title: title,
                    header_width: header_width_waybar(),
                    label_color: ColorToken::Orange,
                    footer_fetched_at: fetched,
                    plan_label: None,
                    amp_free_tier_layout: AmpLayout::Inline,
                    account_in_header: false,
                },
            ))
        }
        "codex" => {
            let vm = resolve_codex_view_model_from(settings, p);
            let plan = normalize_plan_label(p);
            let title = if plan != "Unknown" {
                format!("Codex · {plan}")
            } else {
                "Codex".to_string()
            };
            render_pango(&build_codex(
                clock,
                p,
                &vm,
                &BuildOptions {
                    mode,
                    header_title: title,
                    header_width: header_width_waybar(),
                    label_color: ColorToken::Green,
                    footer_fetched_at: fetched,
                    plan_label: None,
                    amp_free_tier_layout: AmpLayout::Inline,
                    account_in_header: false,
                },
            ))
        }
        "amp" => {
            let title = match p.account.as_deref().filter(|s| !s.is_empty()) {
                Some(acc) => format!("Amp · {acc}"),
                None => "Amp".to_string(),
            };
            render_pango(&build_amp(
                clock,
                p,
                &BuildOptions {
                    mode,
                    header_title: title,
                    header_width: header_width_waybar(),
                    label_color: ColorToken::Magenta,
                    footer_fetched_at: fetched,
                    plan_label: None,
                    amp_free_tier_layout: AmpLayout::Inline,
                    account_in_header: true,
                },
            ))
        }
        _ => {
            let name = if p.display_name.is_empty() {
                p.provider.clone()
            } else {
                p.display_name.clone()
            };
            render_pango(&build_generic(
                clock,
                p,
                &BuildOptions {
                    mode,
                    header_title: name,
                    header_width: header_width_waybar(),
                    label_color: ColorToken::Text,
                    footer_fetched_at: fetched,
                    plan_label: None,
                    amp_free_tier_layout: AmpLayout::Inline,
                    account_in_header: false,
                },
            ))
        }
    }
}

fn build_text(quotas: &AllQuotas, mode: DisplayMode) -> String {
    let parts: Vec<String> = quotas
        .providers
        .iter()
        .filter(|p| p.available)
        .map(|p| pct_colored(to_window_display(p.primary.as_ref(), mode), mode))
        .collect();
    if parts.is_empty() {
        return span(ColorToken::Comment.hex(), "No Providers", false);
    }
    let sep = format!(" {} ", span(ColorToken::Comment.hex(), "│", false));
    parts.join(&sep)
}

fn build_tooltip(clock: &Clock, quotas: &AllQuotas, settings: &Settings, mode: DisplayMode) -> String {
    let sections: Vec<String> = quotas
        .providers
        .iter()
        .filter(|p| p.available || p.error.is_some())
        .map(|p| provider_tooltip(clock, p, Some(&quotas.fetched_at), settings, mode))
        .collect();
    sections.join("\n\n")
}

fn aggregate_class(quotas: &AllQuotas) -> String {
    let mut classes = vec![APP_BASE_CLASS.to_string()];
    for p in quotas.providers.iter().filter(|p| p.available) {
        let val = p.primary.as_ref().map(|w| w.remaining).unwrap_or(100.0);
        let status = status_for_percent(Some(val));
        classes.push(format!("{}-{}", p.provider, status.as_str()));
    }
    classes.join(" ")
}

pub fn format_for_waybar(
    clock: &Clock,
    quotas: &AllQuotas,
    settings: &Settings,
    mode: DisplayMode,
) -> WaybarOutput {
    WaybarOutput {
        text: build_text(quotas, mode),
        tooltip: build_tooltip(clock, quotas, settings, mode),
        class: aggregate_class(quotas),
        alt: None,
        percentage: None,
    }
}

pub fn format_provider_for_waybar(
    clock: &Clock,
    quota: &ProviderQuota,
    settings: &Settings,
    mode: DisplayMode,
) -> WaybarOutput {
    if !quota.available || quota.error.is_some() {
        return WaybarOutput {
            // Glyph nerd-font U+F1616 (confirmado vs src/formatters/waybar.ts); golden valida.
            text: span(ColorToken::Red.hex(), "\u{f1616}", false),
            tooltip: provider_tooltip(clock, quota, None, settings, mode),
            class: format!("{}-{} disconnected", APP_BASE_CLASS, quota.provider),
            alt: Some("disconnected".to_string()),
            percentage: None,
        };
    }

    let rem = quota.primary.as_ref().map(|w| w.remaining);
    let disp = to_window_display(quota.primary.as_ref(), mode);
    let status = status_for_percent(Some(rem.unwrap_or(100.0)));

    let (alt, percentage) = match disp {
        Some(d) => (
            Some(status.as_str().to_string()),
            Some((d.round() as i64).clamp(0, 100) as u8),
        ),
        None => (None, None),
    };

    WaybarOutput {
        text: pct_colored(disp, mode),
        tooltip: provider_tooltip(clock, quota, None, settings, mode),
        class: format!("{}-{} {}", APP_BASE_CLASS, quota.provider, status.as_str()),
        alt,
        percentage,
    }
}
```

> **Glyph disconnected:** `\u{f1616}` foi confirmado contra `src/formatters/waybar.ts` (codepoint U+F1616). O golden (T4) valida byte-a-byte de qualquer forma. Se o editor mostrar o glyph literal `󱖖`, ambos são equivalentes; prefira o escape `\u{f1616}` no código.

> **Nota:** o TS `text` do disconnected é `<span foreground='${ONE_DARK.red}'>GLYPH</span>` — `span(red_hex, GLYPH, false)` reproduz exatamente (sem bold). `ONE_DARK.red` = `ColorToken::Red.hex()` = `#e06c75`.

- [ ] **Step 4: Wire `formatters/mod.rs`** — adicionar `pub mod waybar;` (fim, após `view_model`).

- [ ] **Step 5: Confirmar passa** — `cargo test --manifest-path rust/Cargo.toml formatters::waybar 2>&1 | tail -5` → 4 passed.

- [ ] **Step 6: fmt + clippy + commit**

```bash
cargo fmt --manifest-path rust/Cargo.toml
cargo clippy --manifest-path rust/Cargo.toml --all-targets -- -D warnings
git add rust/src/formatters/waybar.rs rust/src/formatters/mod.rs
git commit -m "feat(rust): assembly waybar (text/tooltip/class)"
```

---

## Task 4: Golden snapshots de paridade

**Files:**
- Modify: `rust/Cargo.toml` (add `insta` em `[dev-dependencies]`)
- Create: `rust/tests/golden.rs`

**Interfaces:**
- Consumes: `agent_bar::formatters::{terminal::format_for_terminal, waybar::{format_for_waybar, format_provider_for_waybar}}`; tipos públicos. (Confirme que o crate expõe `formatters` em `lib.rs`; se algum item não for `pub`, torne-o `pub`.)

**Objetivo:** travar a paridade byte-exact com o TS. Espelha as factories e cenários de `tests/formatters-snapshot.test.ts` e compara o output Rust (sanitizado com os MESMOS regex do TS) contra os valores do `.snap` do TS.

**Estratégia (LEIA antes de codar):**
1. **Fonte da verdade:** `/home/othavio/Projects/agent-bar/tests/__snapshots__/formatters-snapshot.test.ts.snap`. Cada chave `exports[\`<describe> <it> N\`]` é o golden sanitizado.
2. **Factories Rust:** replicar `claudeHealthy/claudeError/codexHealthy/ampHealthy/ampWithCredits/claudeWithExtras/ampUnknownModels/ampWithAccount` (ler os valores exatos no topo de `tests/formatters-snapshot.test.ts`). Constantes: `FIXED_FETCHED_AT = "2026-03-28T12:00:00.000Z"`, `FIXED_RESET = "2026-03-28T14:00:00.000Z"`.
3. **Clock fixo:** `Clock { now: datetime!(2026-03-28 12:00:00 UTC), local_offset: UtcOffset::UTC }`. Com `now = FIXED_FETCHED_AT` e `reset = +2h`, os ETAs caem em formato `"Xh YYm"` → sanitizam para `__HM__` igual ao TS. O `cached · …` vira `just now`/`Xm ago` → `__AGO__`.
4. **Sanitização = `insta` filters** (replicam os regex do TS, na MESMA ordem):
   - `r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d+)?Z"` → `"__ISO__"`
   - `r"\d+d \d{2}h"` → `"__DH__"`  (DH antes de HM para não conflitar)
   - `r"\d{1,2}h \d{2}m"` → `"__HM__"`
   - `r"\(\d{2}:\d{2}\)"` → `"(__:__)"`
   - `r"\d+[hm] ago"` → `"__AGO__"`
   - `r"just now"` → `"__AGO__"`
   - Terminal: ANSI já é texto colorido; o snapshot TS de terminal é ANSI-STRIPPED. Use `render_ansi` SEM cor? NÃO — `render_ansi` emite ANSI. O golden TS de terminal está stripped (`/\x1b\[[0-9;]*m/g` removido). Adicione o filter `r"\x1b\[[0-9;]*m"` → `""` para o snapshot terminal. (Waybar Pango NÃO tem ANSI; não aplicar lá.)
5. **Comparar:** para CADA cenário, renderize → o `insta` aplica os filters → compare com o valor do `.snap` TS. **Como popular o snapshot insta:** copie o conteúdo de cada `exports[...]` do `.snap` TS para o snapshot insta correspondente (arquivo `rust/tests/snapshots/golden__<nome>.snap` no formato insta, OU use `assert_snapshot!` inline colando o valor TS). Rode `cargo test`. Se passar → paridade. Se falhar → o diff do insta mostra a divergência byte-a-byte: **investigue (bug real vs ordem BTreeMap conhecida)**. Para divergência de ordem BTreeMap em cenário multi-model não-alfabético, documente no ledger e ajuste o fixture para ordem alfabética (ou registre como desvio aceito).

> **Importante:** NÃO rode `cargo insta accept` cegamente — isso só gravaria o output Rust como golden, anulando a paridade. O golden DEVE vir do TS. Aceite só após confirmar visualmente que o output Rust == valor TS.

**Cenários a cobrir (mínimo — espelha o TS):**
- Terminal (remaining): Claude healthy, Claude error, Codex healthy, Amp healthy, all-combined, empty.
- Terminal (used): Claude/Codex/Amp healthy.
- Terminal rich: claudeWithExtras, ampWithCredits, ampUnknownModels.
- Waybar (remaining) — text/tooltip/class: Claude healthy, Codex healthy, Amp healthy, all-combined, empty.
- Waybar per-provider — text/tooltip/class: Claude healthy, Claude disconnected, Codex healthy, Amp healthy.
- Waybar account (C1): ampWithAccount (aggregate + per-provider).

- [ ] **Step 1: Adicionar `insta`** ao `rust/Cargo.toml` `[dev-dependencies]`:

```toml
insta = "1"
```

Rode `cargo build --manifest-path rust/Cargo.toml --tests 2>&1 | tail -5` para baixar/compilar.

- [ ] **Step 2: Criar `rust/tests/golden.rs` com as factories + um helper de sanitização via insta settings.** Estrutura (preencher os fixtures lendo o TS):

```rust
//! Golden snapshots de paridade com o TS. Os valores de referência vêm do
//! `.snap` do TS (sanitizado). Clock fixo + filters reproduzem a sanitização.

use agent_bar::config::Paths;
use agent_bar::formatters::clock::Clock;
use agent_bar::formatters::terminal::format_for_terminal;
use agent_bar::formatters::waybar::{format_for_waybar, format_provider_for_waybar};
use agent_bar::providers::types::{AllQuotas, ProviderQuota, QuotaWindow};
use agent_bar::settings::{load, DisplayMode, Settings};
use std::path::PathBuf;
use tempfile::tempdir;
use time::macros::datetime;

const FIXED_RESET: &str = "2026-03-28T14:00:00.000Z";

fn clk() -> Clock {
    Clock {
        now: datetime!(2026-03-28 12:00:00 UTC),
        local_offset: time::UtcOffset::UTC,
    }
}

fn settings() -> Settings {
    let dir = tempdir().unwrap();
    load(&Paths {
        cache_dir: dir.path().join("cache"),
        config_dir: dir.path().join("config"),
        claude_credentials: PathBuf::new(),
        codex_auth: PathBuf::new(),
        codex_sessions: PathBuf::new(),
        amp_settings: PathBuf::new(),
        amp_threads: PathBuf::new(),
    })
}

// Filters de sanitização (mesmos regex/ordem do TS sanitize()).
fn with_filters<F: FnOnce()>(f: F) {
    let mut s = insta::Settings::clone_current();
    s.add_filter(r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d+)?Z", "__ISO__");
    s.add_filter(r"\d+d \d{2}h", "__DH__");
    s.add_filter(r"\d{1,2}h \d{2}m", "__HM__");
    s.add_filter(r"\(\d{2}:\d{2}\)", "(__:__)");
    s.add_filter(r"\d+[hm] ago", "__AGO__");
    s.add_filter("just now", "__AGO__");
    s.bind(f);
}

// Para terminal, adicionalmente strip ANSI.
fn with_filters_terminal<F: FnOnce()>(f: F) {
    let mut s = insta::Settings::clone_current();
    s.add_filter(r"\x1b\[[0-9;]*m", "");
    s.add_filter(r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d+)?Z", "__ISO__");
    s.add_filter(r"\d+d \d{2}h", "__DH__");
    s.add_filter(r"\d{1,2}h \d{2}m", "__HM__");
    s.add_filter(r"\(\d{2}:\d{2}\)", "(__:__)");
    s.add_filter(r"\d+[hm] ago", "__AGO__");
    s.add_filter("just now", "__AGO__");
    s.bind(f);
}

// --- Factories (preencher lendo tests/formatters-snapshot.test.ts) ---
fn wrap(providers: Vec<ProviderQuota>) -> AllQuotas {
    AllQuotas {
        providers,
        fetched_at: "2026-03-28T12:00:00.000Z".into(),
    }
}
// claude_healthy(), codex_healthy(), amp_healthy(), claude_error(), etc.
// ... (transcrever os valores exatos do TS) ...

#[test]
fn terminal_claude_healthy() {
    with_filters_terminal(|| {
        let out = format_for_terminal(
            &clk(),
            &wrap(vec![claude_healthy()]),
            &settings(),
            DisplayMode::Remaining,
        );
        insta::assert_snapshot!(out);
    });
}
// ... demais cenários ...
```

- [ ] **Step 3: Popular os snapshots a partir do TS.** Para cada `assert_snapshot!`, copie o valor sanitizado correspondente do `.snap` TS para o snapshot insta. Rode:

`cargo test --manifest-path rust/Cargo.toml --test golden 2>&1 | tail -30`

Para cada falha, o insta mostra `expected` (TS) vs `actual` (Rust). Se a única diferença for ordem de models (BTreeMap), ajuste o fixture para ordem alfabética e registre. Se for outra coisa → bug de paridade no builder/assembly → corrigir (provavelmente no 3c; se no 3b, é uma regressão a reportar).

- [ ] **Step 4: Iterar até paridade total.** Quando todos os cenários baterem, a paridade está travada. Documente no relatório quantos cenários, e qualquer desvio aceito (ordem BTreeMap).

- [ ] **Step 5: Confirmar suíte inteira + clippy**

`cargo test --manifest-path rust/Cargo.toml 2>&1 | tail -8` (somar todas as suítes incl. `golden`)
`cargo clippy --manifest-path rust/Cargo.toml --all-targets -- -D warnings 2>&1 | tail -3`

- [ ] **Step 6: fmt + commit**

```bash
cargo fmt --manifest-path rust/Cargo.toml
git add rust/Cargo.toml rust/tests/golden.rs rust/tests/snapshots/
git commit -m "test(rust): golden snapshots de paridade com TS"
```

---

## Self-Review (do autor do plano)

- **Cobertura:** assembly terminal (T2) + waybar agregado e per-provider (T3) + paridade golden (T4) + prep (T1). As funções `outputTerminal`/`outputWaybar` (console.log) ficam para o Plano 5 (CLI/stdout) — YAGNI aqui.
- **Deferido com registro:** cache 5s de settings + hidden-module short-circuit → Plano 5; JSON `60.0` → já resolvido (f64 + compare por valor; sem golden JSON).
- **Riscos:** (a) glyph disconnected — marcado como "copiar verbatim do TS" + golden pega; (b) ordem BTreeMap — golden revela; ajustar fixture p/ alfabético ou registrar desvio; (c) o filtro ANSI no terminal — necessário porque o golden TS é ANSI-stripped; (d) `insta` deve vir como dev-dep — T4 Step 1.
- **Decisão de assinatura:** funções recebem `&Settings` + `&Clock` (DI), sem estado global — alinhado ao princípio inviolável e ao que o review de branch 03b confirmou (`resolve_codex_view_model_from` pronto).
- **Placeholder scan:** as factories da T4 estão marcadas como "preencher lendo o TS" — é intencional (os valores exatos vivem no TS-fonte; o implementer transcreve). Todo o resto tem código completo.
