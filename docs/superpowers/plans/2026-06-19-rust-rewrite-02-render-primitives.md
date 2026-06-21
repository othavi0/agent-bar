# Reescrita Rust — Plano 02: Render Primitives + Data Model

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Portar a Layer 4a (primitivos de render puros + modelo de dados): o tipo `ProviderQuota`, a paleta/`ColorToken`, os `Segment`/`Line`, e os renderers Pango (boundary de escape) / ANSI / JSON — sem builders, sem providers, sem async.

**Architecture:** `ProviderQuota` é **serialize-only** (o cache guarda o *raw* do provider, não o quota normalizado — então o quota nunca é desserializado). Os atributos serde (`skip_serializing_if`, `camelCase`) reproduzem a projeção manual do `json.ts`. `ColorToken` carrega `hex()`/`ansi()`; o gate `NO_COLOR` é um **parâmetro injetado** no `render_ansi` (sem estado global). `render_pango` é o ÚNICO ponto de XML-escape.

**Tech Stack:** serde/serde_json (já deps). Nenhuma dep nova (o `time`/local-offset entra só no Plano 03, builders).

## Global Constraints

(Herdados do Plano 01 — repetidos aqui pois cada task implicitamente os inclui.)
- Linux-only. **stdout limpo**; logs → stderr.
- **Sem `unwrap()`/`expect()` em produção** (lib.rs e main.rs têm `#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]`). Em `#[cfg(test)]` é permitido.
- **Sem estado global mutável.** O gate `NO_COLOR` é passado como `bool`, não lido num `static`.
- Crate em `rust/`; todos os comandos `cargo` com `--manifest-path rust/Cargo.toml`. Não `cd`.
- Conventional Commits em português, subject ≤ 50 chars.
- **Contratos de byte exatos desta camada** (testados):
  - `span(hex, text, bold)` = `` `<span foreground='{hex}'{ weight='bold'}>{escape(text)}</span>` `` — **aspas simples**, `weight='bold'` só quando bold.
  - `escape_xml`: nesta ORDEM — `&`→`&amp;`, `<`→`&lt;`, `>`→`&gt;`, `'`→`&#39;`, `"`→`&quot;`. (`&` primeiro, senão double-escape.)
  - `ColorToken` = 12 variantes; hex One Dark exato: green `#98c379`, yellow `#e5c07b`, orange `#d19a66`, red `#e06c75`, comment `#6a7485`, text `#c0c9d4`, textBright `#e2e8f0`, muted `#97a1ae`, magenta `#c678dd`, cyan `#56b6c2`, blue `#61afef`, brightBlue `#528bff`.
  - Barra de quota = **20 chars** sempre: `█`×⌊display/5⌋ + `░`×(20−filled); `display=None` → `░`×20 (cor comment).
  - ANSI: cor = `\x1b[38;2;{r};{g};{b}m` (truecolor do hex); bold = `\x1b[1m`; reset = `\x1b[0m`. Reset anexado **só** quando a linha tem ≥1 segment não-`raw` E `!no_color`.
  - Segment `raw` → texto verbatim (sem span, sem ANSI, sem escape).
  - JSON: `schemaVersion = 1`; envelope `{schemaVersion, fetchedAt, providers[]}`; campos opcionais **omitidos** quando ausentes; **nunca** contém `<span`.

## File Structure

```
rust/src/
  providers/
    mod.rs          # pub mod types;
    types.rs        # ProviderQuota (serialize-only), QuotaWindow, ModelWindows, ProviderExtra, ExtraUsage, AllQuotas
  theme.rs          # ColorToken (+ hex/ansi), ANSI_RESET/ANSI_BOLD, ansi_truecolor, box_chars, provider_hex
  formatters/
    mod.rs          # pub mod shared; segments; render_pango; render_ansi; json;
    shared.rs       # DisplayMode reuse + to_display/to_health/to_window_display (math de exibição)
    segments.rs     # Segment/Line + color_for_display/bar_segments/indicator_segments
    render_pango.rs # escape_xml + span + render_pango (boundary de escape)
    render_ansi.rs  # render_ansi(lines, no_color)
    json.rs         # SCHEMA_VERSION + to_json_string
```

`lib.rs` final (alfabético, atributo preservado):
```rust
#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]
pub mod app_identity;
pub mod cache;
pub mod config;
pub mod formatters;
pub mod logger;
pub mod providers;
pub mod settings;
pub mod theme;
```

---

### Task 1: `providers/types.rs` — modelo de dados (serialize-only)

**Files:**
- Create: `rust/src/providers/mod.rs`
- Create: `rust/src/providers/types.rs`
- Modify: `rust/src/lib.rs` (adicionar `pub mod providers;` em ordem alfabética)

**Interfaces:**
- Produces: `agent_bar::providers::types::{ QuotaWindow, ModelWindows, ExtraUsage, ClaudeQuotaExtra, CodexQuotaExtra, AmpQuotaExtra, ProviderExtra, ProviderQuota, AllQuotas }` — todas `#[derive(Debug, Clone, PartialEq, Serialize)]` (SEM `Deserialize`).

- [ ] **Step 1: Escrever os testes falhando em `rust/src/providers/types.rs`**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn window(remaining: f64) -> QuotaWindow {
        QuotaWindow { remaining, resets_at: Some("2026-06-19T14:00:00Z".into()), window_minutes: None, used: None }
    }

    #[test]
    fn quota_window_omits_none_optionals() {
        let j = serde_json::to_value(window(75.0)).unwrap();
        assert_eq!(j["remaining"], 75.0);
        assert_eq!(j["resetsAt"], "2026-06-19T14:00:00Z");
        assert!(j.get("windowMinutes").is_none(), "windowMinutes deve ser omitido quando None");
        assert!(j.get("used").is_none(), "used deve ser omitido quando None");
    }

    #[test]
    fn quota_window_keeps_null_resets_at() {
        let w = QuotaWindow { remaining: 100.0, resets_at: None, window_minutes: Some(300), used: None };
        let j = serde_json::to_value(w).unwrap();
        assert!(j.as_object().unwrap().contains_key("resetsAt"), "resetsAt sempre presente (pode ser null)");
        assert_eq!(j["resetsAt"], serde_json::Value::Null);
        assert_eq!(j["windowMinutes"], 300);
    }

    #[test]
    fn provider_quota_omits_absent_fields() {
        let q = ProviderQuota {
            provider: "claude".into(),
            display_name: "Claude".into(),
            available: true,
            account: None, plan: None, plan_type: None,
            primary: Some(window(60.0)), secondary: None, models: None, extra: None, error: None,
        };
        let j = serde_json::to_value(q).unwrap();
        assert_eq!(j["provider"], "claude");
        assert_eq!(j["displayName"], "Claude");
        assert_eq!(j["available"], true);
        assert!(j["primary"].is_object());
        for absent in ["account", "plan", "planType", "secondary", "models", "extra", "error"] {
            assert!(j.get(absent).is_none(), "{absent} deve ser omitido quando None");
        }
    }

    #[test]
    fn provider_extra_serializes_untagged() {
        let mut weekly = BTreeMap::new();
        weekly.insert("Opus".to_string(), window(50.0));
        let extra = ProviderExtra::Claude(ClaudeQuotaExtra { weekly_models: Some(weekly), extra_usage: None });
        let j = serde_json::to_value(extra).unwrap();
        // untagged: emite os campos do struct interno diretamente (sem chave de variante)
        assert!(j["weeklyModels"]["Opus"].is_object());
        assert!(j.get("extraUsage").is_none());
    }

    #[test]
    fn all_quotas_field_names() {
        let aq = AllQuotas { providers: vec![], fetched_at: "2026-06-19T14:00:00Z".into() };
        let j = serde_json::to_value(aq).unwrap();
        assert_eq!(j["fetchedAt"], "2026-06-19T14:00:00Z");
        assert!(j["providers"].is_array());
    }
}
```

- [ ] **Step 2: Rodar (deve falhar)**

Run: `cargo test --manifest-path rust/Cargo.toml types 2>&1 | head`
Expected: FAIL de compilação.

- [ ] **Step 3: Implementar `rust/src/providers/types.rs`**

```rust
//! Modelo de quota normalizado, agnóstico de provider. SERIALIZE-ONLY: o cache
//! guarda a resposta crua do provider, não este tipo — então `ProviderQuota`
//! nunca é desserializado, o que evita a ambiguidade de enums untagged.

use std::collections::BTreeMap;

use serde::Serialize;

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
}

#[derive(Debug, Clone, PartialEq, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ModelWindows {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub five_hour: Option<QuotaWindow>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seven_day: Option<QuotaWindow>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub other: Option<Vec<QuotaWindow>>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtraUsage {
    pub enabled: bool,
    pub remaining: f64,
    pub limit: f64,
    pub used: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeQuotaExtra {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weekly_models: Option<BTreeMap<String, QuotaWindow>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra_usage: Option<ExtraUsage>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CodexQuotaExtra {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub models_detailed: Option<BTreeMap<String, ModelWindows>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra_usage: Option<ExtraUsage>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AmpQuotaExtra {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta: Option<BTreeMap<String, String>>,
}

/// Untagged: serializa apenas o conteúdo do struct interno (sem chave de variante),
/// reproduzindo a forma de `extra` do TS.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(untagged)]
pub enum ProviderExtra {
    Claude(ClaudeQuotaExtra),
    Codex(CodexQuotaExtra),
    Amp(AmpQuotaExtra),
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderQuota {
    pub provider: String,
    pub display_name: String,
    pub available: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub primary: Option<QuotaWindow>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secondary: Option<QuotaWindow>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub models: Option<BTreeMap<String, QuotaWindow>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra: Option<ProviderExtra>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AllQuotas {
    pub providers: Vec<ProviderQuota>,
    pub fetched_at: String,
}
```

- [ ] **Step 4: Criar `rust/src/providers/mod.rs`**

```rust
pub mod types;
```

- [ ] **Step 5: Wirar `rust/src/lib.rs`** — ler o arquivo atual e adicionar `pub mod providers;` em ordem alfabética (entre `logger` e `settings`), preservando o atributo `#![cfg_attr…]` na linha 1.

- [ ] **Step 6: Rodar + lint + commit**

```bash
cargo test --manifest-path rust/Cargo.toml types
cargo fmt --manifest-path rust/Cargo.toml
cargo clippy --manifest-path rust/Cargo.toml --all-targets -- -D warnings
git add rust/src/providers/ rust/src/lib.rs
git commit -m "feat(rust): modelo de dados ProviderQuota"
```
Expected: 5 testes de `types` passam; suite total 21; clippy clean.

---

### Task 2: `theme.rs` — ColorToken + paleta + ANSI

**Files:**
- Create: `rust/src/theme.rs`
- Modify: `rust/src/lib.rs` (adicionar `pub mod theme;` ao fim, ordem alfabética)

**Interfaces:**
- Produces: `agent_bar::theme::{ ColorToken (12 variantes), ANSI_RESET, ANSI_BOLD, ansi_truecolor, provider_hex, box_chars::{TL,BL,LT,H,V,DOT,DOT_O,DIAMOND} }`. `ColorToken::hex(self) -> &'static str`; `ColorToken::ansi(self) -> String`.

- [ ] **Step 1: Escrever os testes falhando em `rust/src/theme.rs`**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_values_match_one_dark() {
        assert_eq!(ColorToken::Green.hex(), "#98c379");
        assert_eq!(ColorToken::Red.hex(), "#e06c75");
        assert_eq!(ColorToken::Comment.hex(), "#6a7485");
        assert_eq!(ColorToken::BrightBlue.hex(), "#528bff");
        assert_eq!(ColorToken::TextBright.hex(), "#e2e8f0");
    }

    #[test]
    fn ansi_truecolor_format() {
        // #98c379 → 152;195;121
        assert_eq!(ColorToken::Green.ansi(), "\x1b[38;2;152;195;121m");
        assert_eq!(ansi_truecolor("#e06c75"), "\x1b[38;2;224;108;117m");
    }

    #[test]
    fn ansi_truecolor_rejects_bad_hex() {
        assert_eq!(ansi_truecolor("nope"), "");
        assert_eq!(ansi_truecolor("#12"), "");
    }

    #[test]
    fn ansi_constants() {
        assert_eq!(ANSI_RESET, "\x1b[0m");
        assert_eq!(ANSI_BOLD, "\x1b[1m");
    }

    #[test]
    fn provider_hex_mapping() {
        assert_eq!(provider_hex("claude"), ColorToken::Orange.hex());
        assert_eq!(provider_hex("codex"), ColorToken::Green.hex());
        assert_eq!(provider_hex("amp"), ColorToken::Magenta.hex());
        assert_eq!(provider_hex("other"), ColorToken::Text.hex());
    }

    #[test]
    fn box_chars_are_heavy_variant() {
        assert_eq!(box_chars::TL, "┏");
        assert_eq!(box_chars::V, "┃");
        assert_eq!(box_chars::DIAMOND, "◆");
    }
}
```

- [ ] **Step 2: Rodar (deve falhar)**

Run: `cargo test --manifest-path rust/Cargo.toml theme 2>&1 | head`
Expected: FAIL de compilação.

- [ ] **Step 3: Implementar `rust/src/theme.rs`**

```rust
//! Paleta One Dark e tokens de cor. O gate NO_COLOR NÃO vive aqui (é injetado no
//! render_ansi) — `ansi()` sempre devolve o código; o renderer decide emiti-lo ou não.

pub const ANSI_RESET: &str = "\x1b[0m";
pub const ANSI_BOLD: &str = "\x1b[1m";

/// Token de cor agnóstico de tema. Os renderers mapeiam para hex (Pango) ou ANSI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorToken {
    Green,
    Yellow,
    Orange,
    Red,
    Comment,
    Text,
    TextBright,
    Muted,
    Magenta,
    Cyan,
    Blue,
    BrightBlue,
}

impl ColorToken {
    pub fn hex(self) -> &'static str {
        match self {
            ColorToken::Green => "#98c379",
            ColorToken::Yellow => "#e5c07b",
            ColorToken::Orange => "#d19a66",
            ColorToken::Red => "#e06c75",
            ColorToken::Comment => "#6a7485",
            ColorToken::Text => "#c0c9d4",
            ColorToken::TextBright => "#e2e8f0",
            ColorToken::Muted => "#97a1ae",
            ColorToken::Magenta => "#c678dd",
            ColorToken::Cyan => "#56b6c2",
            ColorToken::Blue => "#61afef",
            ColorToken::BrightBlue => "#528bff",
        }
    }

    pub fn ansi(self) -> String {
        ansi_truecolor(self.hex())
    }
}

/// `#rrggbb` → escape ANSI truecolor `\x1b[38;2;r;g;bm`. Hex inválido → string vazia.
pub fn ansi_truecolor(hex: &str) -> String {
    let clean = hex.trim_start_matches('#');
    if clean.len() != 6 {
        return String::new();
    }
    let comp = |s: &str| u8::from_str_radix(s, 16).unwrap_or(0);
    let r = comp(&clean[0..2]);
    let g = comp(&clean[2..4]);
    let b = comp(&clean[4..6]);
    format!("\x1b[38;2;{r};{g};{b}m")
}

/// Cor de marca do provider (hex). Desconhecido → text.
pub fn provider_hex(id: &str) -> &'static str {
    match id {
        "claude" => ColorToken::Orange.hex(),
        "codex" => ColorToken::Green.hex(),
        "amp" => ColorToken::Magenta.hex(),
        _ => ColorToken::Text.hex(),
    }
}

/// Box-drawing (variante pesada) — fonte única da verdade.
pub mod box_chars {
    pub const TL: &str = "┏";
    pub const BL: &str = "┗";
    pub const LT: &str = "┣";
    pub const H: &str = "━";
    pub const V: &str = "┃";
    pub const DOT: &str = "●";
    pub const DOT_O: &str = "○";
    pub const DIAMOND: &str = "◆";
}
```

- [ ] **Step 4: Wirar `rust/src/lib.rs`** — adicionar `pub mod theme;` ao fim (ordem alfabética), preservar atributo.

- [ ] **Step 5: Rodar + lint + commit**

```bash
cargo test --manifest-path rust/Cargo.toml theme
cargo fmt --manifest-path rust/Cargo.toml
cargo clippy --manifest-path rust/Cargo.toml --all-targets -- -D warnings
git add rust/src/theme.rs rust/src/lib.rs
git commit -m "feat(rust): paleta One Dark + ColorToken"
```
Expected: 6 testes de `theme`; suite total 27; clippy clean.

---

### Task 3: `formatters/shared.rs` + `formatters/segments.rs`

**Files:**
- Create: `rust/src/formatters/mod.rs`
- Create: `rust/src/formatters/shared.rs`
- Create: `rust/src/formatters/segments.rs`
- Modify: `rust/src/lib.rs` (adicionar `pub mod formatters;` em ordem alfabética)

**Interfaces:**
- Consumes: `theme::ColorToken`, `config::{HealthStatus, status_for_percent}`, `settings::DisplayMode`.
- Produces:
  - `formatters::shared::{ to_display(Option<f64>, DisplayMode) -> Option<f64>, to_health(Option<f64>, DisplayMode) -> Option<f64>, to_window_display(Option<&QuotaWindow>, DisplayMode) -> Option<f64> }`
  - `formatters::segments::{ Segment, Line, color_for_display, bar_segments, indicator_segments }`. `Segment { text: Cow<'static,str>, color: ColorToken, bold: bool, raw: bool }` com `Segment::new`, `Segment::raw_text`, `.bold()`. `type Line = Vec<Segment>`.

- [ ] **Step 1: Escrever os testes falhando**

Em `rust/src/formatters/shared.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::DisplayMode;

    #[test]
    fn to_display_modes() {
        assert_eq!(to_display(Some(70.0), DisplayMode::Remaining), Some(70.0));
        assert_eq!(to_display(Some(70.0), DisplayMode::Used), Some(30.0));
        assert_eq!(to_display(None, DisplayMode::Used), None);
    }

    #[test]
    fn to_health_inverts_used() {
        assert_eq!(to_health(Some(30.0), DisplayMode::Used), Some(70.0));
        assert_eq!(to_health(Some(70.0), DisplayMode::Remaining), Some(70.0));
        assert_eq!(to_health(None, DisplayMode::Remaining), None);
    }
}
```

Em `rust/src/formatters/segments.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::types::QuotaWindow;
    use crate::settings::DisplayMode;
    use crate::theme::ColorToken;

    #[test]
    fn bar_is_always_20_wide() {
        let segs = bar_segments(Some(60.0), DisplayMode::Remaining);
        let total: usize = segs.iter().map(|s| s.text.chars().count()).sum();
        assert_eq!(total, 20);
        // 60/5 = 12 filled
        assert_eq!(segs[0].text.chars().count(), 12);
        assert_eq!(segs[1].text.chars().count(), 8);
    }

    #[test]
    fn bar_null_is_all_empty_comment() {
        let segs = bar_segments(None, DisplayMode::Remaining);
        assert_eq!(segs.len(), 1);
        assert_eq!(segs[0].text.chars().count(), 20);
        assert_eq!(segs[0].color, ColorToken::Comment);
    }

    #[test]
    fn bar_clamps_overage_to_20() {
        let segs = bar_segments(Some(150.0), DisplayMode::Remaining);
        let total: usize = segs.iter().map(|s| s.text.chars().count()).sum();
        assert_eq!(total, 20);
    }

    #[test]
    fn color_for_display_thresholds() {
        assert_eq!(color_for_display(Some(75.0), DisplayMode::Remaining), ColorToken::Green);
        assert_eq!(color_for_display(Some(20.0), DisplayMode::Remaining), ColorToken::Orange);
        assert_eq!(color_for_display(Some(5.0), DisplayMode::Remaining), ColorToken::Red);
        assert_eq!(color_for_display(None, DisplayMode::Remaining), ColorToken::Text);
    }

    #[test]
    fn indicator_open_dot_when_null() {
        assert_eq!(indicator_segments(None, DisplayMode::Remaining)[0].text, "○");
        assert_eq!(indicator_segments(Some(80.0), DisplayMode::Remaining)[0].text, "●");
    }

    #[test]
    fn raw_segment_constructor() {
        let s = Segment::raw_text(" │ ");
        assert!(s.raw);
        assert_eq!(s.text, " │ ");
    }
}
```

- [ ] **Step 2: Rodar (deve falhar)**

Run: `cargo test --manifest-path rust/Cargo.toml formatters:: 2>&1 | head`
Expected: FAIL de compilação.

- [ ] **Step 3: Implementar `rust/src/formatters/shared.rs`**

```rust
//! Math de exibição compartilhada (remaining vs used). DisplayMode vem de settings.

use crate::providers::types::QuotaWindow;
use crate::settings::DisplayMode;

pub fn to_display(remaining: Option<f64>, mode: DisplayMode) -> Option<f64> {
    let r = remaining?;
    Some(match mode {
        DisplayMode::Used => 100.0 - r,
        DisplayMode::Remaining => r,
    })
}

/// Valor de exibição de uma janela, honrando `used` do provider em modo `used`.
pub fn to_window_display(window: Option<&QuotaWindow>, mode: DisplayMode) -> Option<f64> {
    let w = window?;
    if let (DisplayMode::Used, Some(used)) = (mode, w.used) {
        return Some(used);
    }
    to_display(Some(w.remaining), mode)
}

pub fn to_health(display_value: Option<f64>, mode: DisplayMode) -> Option<f64> {
    let d = display_value?;
    Some(match mode {
        DisplayMode::Used => 100.0 - d,
        DisplayMode::Remaining => d,
    })
}
```

- [ ] **Step 4: Implementar `rust/src/formatters/segments.rs`**

```rust
//! Modelo intermediário de render: linhas de segments coloridos.

use std::borrow::Cow;

use crate::config::{status_for_percent, HealthStatus};
use crate::settings::DisplayMode;
use crate::theme::ColorToken;

use super::shared::to_health;

#[derive(Debug, Clone, PartialEq)]
pub struct Segment {
    pub text: Cow<'static, str>,
    pub color: ColorToken,
    pub bold: bool,
    /// Texto verbatim: sem span/ANSI/escape. Usado p/ conectores (espaços, separadores).
    pub raw: bool,
}

impl Segment {
    pub fn new(text: impl Into<Cow<'static, str>>, color: ColorToken) -> Self {
        Self { text: text.into(), color, bold: false, raw: false }
    }

    /// Segment `raw` (verbatim). A cor é estruturalmente exigida mas ignorada no render.
    pub fn raw_text(text: impl Into<Cow<'static, str>>) -> Self {
        Self { text: text.into(), color: ColorToken::Text, bold: false, raw: true }
    }

    pub fn bold(mut self) -> Self {
        self.bold = true;
        self
    }
}

pub type Line = Vec<Segment>;

fn status_to_color(s: HealthStatus) -> ColorToken {
    match s {
        HealthStatus::Ok => ColorToken::Green,
        HealthStatus::Low => ColorToken::Yellow,
        HealthStatus::Warn => ColorToken::Orange,
        HealthStatus::Critical => ColorToken::Red,
    }
}

pub fn color_for_display(display: Option<f64>, mode: DisplayMode) -> ColorToken {
    match to_health(display, mode) {
        None => ColorToken::Text,
        Some(h) => status_to_color(status_for_percent(Some(h))),
    }
}

/// Barra de quota de 20 chars. Vazia (comment) quando `display` é None.
pub fn bar_segments(display: Option<f64>, mode: DisplayMode) -> Line {
    match display {
        None => vec![Segment::new("░".repeat(20), ColorToken::Comment)],
        Some(d) => {
            let filled = ((d / 5.0).floor().max(0.0) as usize).min(20);
            vec![
                Segment::new("█".repeat(filled), color_for_display(display, mode)),
                Segment::new("░".repeat(20 - filled), ColorToken::Comment),
            ]
        }
    }
}

/// Indicador de ponto único. Ponto aberto (comment) quando `display` é None.
pub fn indicator_segments(display: Option<f64>, mode: DisplayMode) -> Line {
    match display {
        None => vec![Segment::new("○", ColorToken::Comment)],
        Some(_) => vec![Segment::new("●", color_for_display(display, mode))],
    }
}
```

- [ ] **Step 5: Criar `rust/src/formatters/mod.rs`**

```rust
pub mod render_ansi;
pub mod render_pango;
pub mod segments;
pub mod shared;
```
> Nota: `render_ansi`/`render_pango` ainda não existem (Tasks 4-5). Para esta task compilar, crie `mod.rs` listando só `segments` e `shared`, e adicione `render_pango`/`render_ansi`/`json` nas tasks seguintes. (Comece com `pub mod segments; pub mod shared;`.)

- [ ] **Step 6: Wirar `rust/src/lib.rs`** — adicionar `pub mod formatters;` em ordem alfabética (entre `config` e `logger`), preservar atributo.

- [ ] **Step 7: Rodar + lint + commit**

```bash
cargo test --manifest-path rust/Cargo.toml formatters::
cargo fmt --manifest-path rust/Cargo.toml
cargo clippy --manifest-path rust/Cargo.toml --all-targets -- -D warnings
git add rust/src/formatters/ rust/src/lib.rs
git commit -m "feat(rust): segments + math de exibição"
```
Expected: 2 testes de shared + 6 de segments; suite total 35; clippy clean.

---

### Task 4: `formatters/render_pango.rs` — boundary de escape

**Files:**
- Create: `rust/src/formatters/render_pango.rs`
- Modify: `rust/src/formatters/mod.rs` (adicionar `pub mod render_pango;`)

**Interfaces:**
- Consumes: `theme::ColorToken`, `formatters::segments::{Line, Segment}`.
- Produces: `formatters::render_pango::{ escape_xml(&str) -> String, span(hex: &str, text: &str, bold: bool) -> String, render_pango(&[Line]) -> String }`.

- [ ] **Step 1: Escrever os testes falhando em `rust/src/formatters/render_pango.rs`**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::formatters::segments::Segment;
    use crate::theme::ColorToken;

    #[test]
    fn escape_all_five_entities_in_order() {
        assert_eq!(escape_xml("a&b<c>d'e\"f"), "a&amp;b&lt;c&gt;d&#39;e&quot;f");
    }

    #[test]
    fn escape_ampersand_first_no_double_escape() {
        // se '&' não fosse primeiro, "<" viraria "&lt;" e o "&" seria re-escapado
        assert_eq!(escape_xml("<"), "&lt;");
        assert_eq!(escape_xml("&lt;"), "&amp;lt;");
    }

    #[test]
    fn span_format_single_quotes() {
        assert_eq!(span("#98c379", "hi", false), "<span foreground='#98c379'>hi</span>");
    }

    #[test]
    fn span_bold_adds_weight() {
        assert_eq!(span("#e06c75", "x", true), "<span foreground='#e06c75' weight='bold'>x</span>");
    }

    #[test]
    fn span_escapes_text() {
        assert_eq!(span("#c0c9d4", "a<b", false), "<span foreground='#c0c9d4'>a&lt;b</span>");
    }

    #[test]
    fn render_pango_raw_bypasses_span_and_escape() {
        let line = vec![
            Segment::new("ok", ColorToken::Green),
            Segment::raw_text(" <sep> "),
        ];
        let out = render_pango(&[line]);
        assert_eq!(out, "<span foreground='#98c379'>ok</span> <sep> ");
    }

    #[test]
    fn render_pango_joins_lines_with_newline() {
        let l1 = vec![Segment::new("a", ColorToken::Text)];
        let l2 = vec![Segment::new("b", ColorToken::Text)];
        let out = render_pango(&[l1, l2]);
        assert!(out.contains('\n'));
        assert_eq!(out.lines().count(), 2);
    }
}
```

- [ ] **Step 2: Rodar (deve falhar)**

Run: `cargo test --manifest-path rust/Cargo.toml render_pango 2>&1 | head`
Expected: FAIL de compilação.

- [ ] **Step 3: Implementar `rust/src/formatters/render_pango.rs`**

```rust
//! ÚNICO ponto de XML-escape do Pango. Toda string de provider que entra em markup
//! Pango passa por `span` (que escapa). Segments `raw` saem verbatim — nunca passe
//! dados não-confiáveis por `raw`.

use super::segments::{Line, Segment};

/// Escapa os 5 entities XML. ORDEM importa: `&` primeiro (senão double-escape).
pub fn escape_xml(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('\'', "&#39;")
        .replace('"', "&quot;")
}

/// `<span foreground='{hex}'[ weight='bold']>{escape(text)}</span>` — aspas simples.
pub fn span(hex: &str, text: &str, bold: bool) -> String {
    let weight = if bold { " weight='bold'" } else { "" };
    format!("<span foreground='{hex}'{weight}>{}</span>", escape_xml(text))
}

fn render_segment(seg: &Segment) -> String {
    if seg.raw {
        return seg.text.to_string();
    }
    span(seg.color.hex(), &seg.text, seg.bold)
}

fn render_line(line: &Line) -> String {
    line.iter().map(render_segment).collect()
}

/// Renderiza linhas em markup Pango multi-linha.
pub fn render_pango(lines: &[Line]) -> String {
    lines.iter().map(render_line).collect::<Vec<_>>().join("\n")
}
```

- [ ] **Step 4: `rust/src/formatters/mod.rs`** — adicionar `pub mod render_pango;` (ordem alfabética).

- [ ] **Step 5: Rodar + lint + commit**

```bash
cargo test --manifest-path rust/Cargo.toml render_pango
cargo fmt --manifest-path rust/Cargo.toml
cargo clippy --manifest-path rust/Cargo.toml --all-targets -- -D warnings
git add rust/src/formatters/render_pango.rs rust/src/formatters/mod.rs
git commit -m "feat(rust): render Pango + boundary de escape"
```
Expected: 7 testes; suite total 42; clippy clean.

---

### Task 5: `formatters/render_ansi.rs` — gate NO_COLOR injetado

**Files:**
- Create: `rust/src/formatters/render_ansi.rs`
- Modify: `rust/src/formatters/mod.rs` (adicionar `pub mod render_ansi;`)

**Interfaces:**
- Consumes: `theme::{ansi_truecolor (via ColorToken::ansi), ANSI_BOLD, ANSI_RESET}`, `formatters::segments::{Line, Segment}`.
- Produces: `formatters::render_ansi::render_ansi(lines: &[Line], no_color: bool) -> String`.

**Nota:** o NO_COLOR é injetado (`no_color: bool`), não lido de `static` — alinhado com o ban de estado global. `theme::ColorToken::ansi()` sempre devolve o código; o renderer decide emiti-lo.

- [ ] **Step 1: Escrever os testes falhando em `rust/src/formatters/render_ansi.rs`**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::formatters::segments::Segment;
    use crate::theme::ColorToken;

    #[test]
    fn colored_segment_has_truecolor_and_reset() {
        let out = render_ansi(&[vec![Segment::new("hi", ColorToken::Green)]], false);
        assert_eq!(out, "\x1b[38;2;152;195;121mhi\x1b[0m");
    }

    #[test]
    fn bold_segment_includes_bold_code() {
        let out = render_ansi(&[vec![Segment::new("x", ColorToken::Red).bold()]], false);
        assert_eq!(out, "\x1b[38;2;224;108;117m\x1b[1mx\x1b[0m");
    }

    #[test]
    fn no_color_emits_plain_text() {
        let out = render_ansi(&[vec![Segment::new("hi", ColorToken::Green)]], true);
        assert_eq!(out, "hi");
    }

    #[test]
    fn raw_only_line_has_no_reset() {
        let out = render_ansi(&[vec![Segment::raw_text("│")]], false);
        assert_eq!(out, "│"); // sem reset porque nenhum segment não-raw
    }

    #[test]
    fn mixed_line_resets_once_at_end() {
        let line = vec![Segment::new("a", ColorToken::Text), Segment::raw_text(" | ")];
        let out = render_ansi(&[line], false);
        assert!(out.ends_with("\x1b[0m"));
        assert_eq!(out.matches("\x1b[0m").count(), 1);
    }

    #[test]
    fn lines_joined_by_newline() {
        let out = render_ansi(
            &[vec![Segment::new("a", ColorToken::Text)], vec![Segment::new("b", ColorToken::Text)]],
            true,
        );
        assert_eq!(out, "a\nb");
    }
}
```

- [ ] **Step 2: Rodar (deve falhar)**

Run: `cargo test --manifest-path rust/Cargo.toml render_ansi 2>&1 | head`
Expected: FAIL de compilação.

- [ ] **Step 3: Implementar `rust/src/formatters/render_ansi.rs`**

```rust
//! Render ANSI para o terminal. NO_COLOR é injetado. Reset anexado só quando a
//! linha tem ≥1 segment não-raw e cores estão ativas.

use super::segments::{Line, Segment};
use crate::theme::{ANSI_BOLD, ANSI_RESET};

fn render_segment(seg: &Segment, no_color: bool) -> String {
    if seg.raw || no_color {
        return seg.text.to_string();
    }
    let bold = if seg.bold { ANSI_BOLD } else { "" };
    format!("{}{}{}", seg.color.ansi(), bold, seg.text)
}

fn render_line(line: &Line, no_color: bool) -> String {
    if line.is_empty() {
        return String::new();
    }
    let body: String = line.iter().map(|s| render_segment(s, no_color)).collect();
    let has_colored = line.iter().any(|s| !s.raw);
    if has_colored && !no_color {
        format!("{body}{ANSI_RESET}")
    } else {
        body
    }
}

/// Renderiza linhas em ANSI multi-linha.
pub fn render_ansi(lines: &[Line], no_color: bool) -> String {
    lines.iter().map(|l| render_line(l, no_color)).collect::<Vec<_>>().join("\n")
}
```

- [ ] **Step 4: `rust/src/formatters/mod.rs`** — adicionar `pub mod render_ansi;` (ordem alfabética, antes de render_pango).

- [ ] **Step 5: Rodar + lint + commit**

```bash
cargo test --manifest-path rust/Cargo.toml render_ansi
cargo fmt --manifest-path rust/Cargo.toml
cargo clippy --manifest-path rust/Cargo.toml --all-targets -- -D warnings
git add rust/src/formatters/render_ansi.rs rust/src/formatters/mod.rs
git commit -m "feat(rust): render ANSI com gate NO_COLOR"
```
Expected: 6 testes; suite total 48; clippy clean.

---

### Task 6: `formatters/json.rs` — envelope versionado

**Files:**
- Create: `rust/src/formatters/json.rs`
- Modify: `rust/src/formatters/mod.rs` (adicionar `pub mod json;`)

**Interfaces:**
- Consumes: `providers::types::{AllQuotas, ProviderQuota}`.
- Produces: `formatters::json::{ SCHEMA_VERSION: u32, to_json_string(&AllQuotas) -> Result<String, serde_json::Error> }`.

**Nota de design:** o `ProviderQuota` já serializa com a forma exata do `JsonProvider` do TS (graças aos atributos serde `skip_serializing_if`/`camelCase`), então NÃO precisamos das structs de projeção `Json*` do TS — o envelope só adiciona `schemaVersion`. Os providers (Plano 04) devem setar `extra = None` quando vazio (o `skip_serializing_if` omite).

- [ ] **Step 1: Escrever os testes falhando em `rust/src/formatters/json.rs`**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::types::{AllQuotas, ProviderQuota, QuotaWindow};

    fn sample() -> AllQuotas {
        AllQuotas {
            fetched_at: "2026-06-19T14:00:00Z".into(),
            providers: vec![ProviderQuota {
                provider: "claude".into(),
                display_name: "Claude".into(),
                available: true,
                account: None,
                plan: Some("Pro".into()),
                plan_type: None,
                primary: Some(QuotaWindow { remaining: 60.0, resets_at: None, window_minutes: None, used: None }),
                secondary: None,
                models: None,
                extra: None,
                error: None,
            }],
        }
    }

    #[test]
    fn envelope_has_schema_version_and_fetched_at() {
        let s = to_json_string(&sample()).unwrap();
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["schemaVersion"], 1);
        assert_eq!(v["fetchedAt"], "2026-06-19T14:00:00Z");
        assert_eq!(v["providers"][0]["provider"], "claude");
        assert_eq!(v["providers"][0]["plan"], "Pro");
    }

    #[test]
    fn omits_absent_provider_fields() {
        let s = to_json_string(&sample()).unwrap();
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        let p = &v["providers"][0];
        for absent in ["account", "planType", "secondary", "models", "extra", "error"] {
            assert!(p.get(absent).is_none(), "{absent} deve ser omitido");
        }
    }

    #[test]
    fn never_contains_pango_markup() {
        let s = to_json_string(&sample()).unwrap();
        assert!(!s.contains("<span"), "envelope JSON nunca pode conter Pango");
    }

    #[test]
    fn schema_version_is_one() {
        assert_eq!(SCHEMA_VERSION, 1);
    }
}
```

- [ ] **Step 2: Rodar (deve falhar)**

Run: `cargo test --manifest-path rust/Cargo.toml json 2>&1 | head`
Expected: FAIL de compilação.

- [ ] **Step 3: Implementar `rust/src/formatters/json.rs`**

```rust
//! Envelope JSON versionado (sem Pango) para outras barras (Quickshell/Eww).
//! Bump em mudança incompatível (remover/renomear/retipar campo estável); adicionar
//! campo opcional NÃO exige bump.

use serde::Serialize;

use crate::providers::types::{AllQuotas, ProviderQuota};

pub const SCHEMA_VERSION: u32 = 1;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonOutput<'a> {
    schema_version: u32,
    fetched_at: &'a str,
    providers: &'a [ProviderQuota],
}

/// Serializa o envelope versionado. `ProviderQuota` já tem a forma de saída correta.
pub fn to_json_string(quotas: &AllQuotas) -> Result<String, serde_json::Error> {
    let out = JsonOutput {
        schema_version: SCHEMA_VERSION,
        fetched_at: &quotas.fetched_at,
        providers: &quotas.providers,
    };
    serde_json::to_string(&out)
}
```

- [ ] **Step 4: `rust/src/formatters/mod.rs`** — adicionar `pub mod json;` (ordem alfabética, no topo).

- [ ] **Step 5: Rodar suíte inteira + lint + commit**

```bash
cargo test --manifest-path rust/Cargo.toml
cargo fmt --manifest-path rust/Cargo.toml
cargo clippy --manifest-path rust/Cargo.toml --all-targets -- -D warnings
git add rust/src/formatters/json.rs rust/src/formatters/mod.rs
git commit -m "feat(rust): envelope JSON versionado"
```
Expected: 4 testes de json; **suite total 52**; clippy clean.

---

## Próximo plano (03 — formatters c/ settings + builders)

`time` (local-offset) entra aqui p/ `format_reset_time`. Builders por-provider (`ProviderQuota` → `Line[]`), `view_model`, `waybar.rs` (+ cache 5s de settings), `terminal.rs`. Golden snapshots: usar `tests/__snapshots__/formatters-snapshot.test.ts.snap` do TS como referência byte-exact das saídas completas.

## Self-review (preenchido)

- **Cobertura:** Layer 4a do design (types/theme/segments/render_pango/render_ansi/json) — toda mapeada a uma task. Builders/waybar/terminal/view_model = Plano 03 (explícito).
- **Desvio do spec registrado:** `render_ansi` porta o truecolor manual do TS (não owo-colors) — saída ANSI é sanitizada nos snapshots (não byte-exact), e o approach manual evita o gotcha do `IsTerminal` do owo-colors e casa exatamente com a semântica de reset-por-linha do TS. owo-colors fica p/ a view `status`/help (Plano 03+).
- **Tipos consistentes:** `Segment`/`Line`/`ColorToken`/`DisplayMode`/`QuotaWindow` usados de forma idêntica entre tasks. `ColorToken` definido em `theme.rs`, consumido por segments/render_*.
- **Sem placeholder.** Todo código presente.
