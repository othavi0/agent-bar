# Reescrita Rust — Plano 03a: Format Helpers + Builder Primitives

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Portar os helpers de formatação restantes (percent/eta/reset-time/ago/classify/plan) e os primitivos de builder (`vline`/`label_line`/`model_line`/`build_footer_line`/`header_line`) que todos os builders por-provider (Plano 3b) consomem.

**Architecture:** Tempo é injetado via um `Clock { now, local_offset }` resolvido uma vez no `main` (startup single-thread) — `format_reset_time` usa timezone LOCAL sem chamar `current_local_offset()` em runtime (mata a armadilha de CI + multi-thread). Os primitivos de builder são funções puras `-> Line` que consomem o que já foi portado (segments, render, theme, shared math).

**Tech Stack:** `time 0.3` (parse RFC3339 + local offset — já em deps desde o scaffold). serde já presente. Nenhuma dep nova.

## Global Constraints

- Linux-only; stdout limpo; logs → stderr.
- **Sem `unwrap()`/`expect()` em produção** (lib.rs + main.rs têm o `deny`). Em `#[cfg(test)]` permitido.
- **Sem estado global mutável.** Tempo via `Clock` injetado — NÃO chamar `OffsetDateTime::now_*`/`current_local_offset()` dentro dos formatters; eles recebem `&Clock`.
- Crate em `rust/`; `cargo` com `--manifest-path rust/Cargo.toml`. Não `cd`. Rodar `cargo fmt` ANTES de cada `git add` (tree limpa).
- Conventional Commits PT ≤50.
- **Contratos exatos** (do TS `src/formatters/shared.ts` + `builders/shared.ts`; a autoridade byte-exact da saída COMPLETA são os golden snapshots, validados no Plano 3c — aqui os testes asseram os helpers isoladamente):
  - `format_percent(None)`=`?%`; `Some(v)`=`{round(v)}%`.
  - `format_eta`: `remaining==100`→`Full`; sem iso→`?`; diff<0→`0h 00m`; d>0→`{d}d {hh}h`; senão `{h}h {mm}m`.
  - `format_reset_time`: `remaining==100`→``(vazio); sem iso→`(??:??)`; senão `({HH}:{MM})` em **LOCAL time** (2 dígitos).
  - `format_ago`: <60s→`just now`; <60min→`{m}m ago`; senão `{h}h ago`.
  - `classify_window`: `None`/`<=0`→Other; `|min-300|<=90`→FiveHour; `|min-10080|<=1440`→SevenDay; senão Other.
  - `normalize_plan`: mapa (free→Free, go→Go, plus→Plus, pro→Pro, business/team→Business, enterprise→Enterprise, edu/education→Edu, apikey/api_key→API Key); fallback titlecase (substitui `_`/`-` por espaço, capitaliza cada palavra). `None`/vazio→None.
  - `eta_label`: used→`Resets in`; senão `Full in`.
  - `TOOLTIP_BORDER = 56`. `vline` = 1 segment `┃` na cor accent. `label_line` = `┣━`(connector) + raw(` `) + `◆ {text}`(labelColor, bold) → "┣━ ◆ Label".
  - `build_footer_line` sem fetched_at: `┗` + `━`×55 (total 56, cor accent). Com fetched: stamp=` cached · {ago} ` (1 espaço cada lado); total_dashes=55-stamp.chars().count(); left=max(1, total_dashes/2); right=max(1, total_dashes-left) → `┗`+`━`×left (accent) + stamp (comment) + `━`×right (accent). **Char count via `.chars().count()`** (┗/━ são multi-byte).
  - `header_line(title, headerWidth, color)`: `┏━`(color) + raw(` `) + `{title}`(color, bold) + raw(` `) + `━`×max(1, headerWidth-title.chars().count()) (color).
  - `model_line` — 11 segments nesta ordem (ver Task 3).

## File Structure

```
rust/src/formatters/
  clock.rs        # NOVO: Clock { now: OffsetDateTime, local_offset: UtcOffset }, Clock::from_env()
  shared.rs       # estende: format_percent, WindowKind+classify_window, normalize_plan(+_label), eta_label (Task 1); format_eta, format_reset_time, format_ago (Task 2, usam &Clock)
  builders/
    mod.rs        # NOVO: pub mod shared;
    shared.rs     # NOVO: TOOLTIP_BORDER, BuildOptions, vline, label_line, model_line, build_footer_line, header_line
  mod.rs          # estende: pub mod builders; pub mod clock;
```

---

### Task 1: `shared.rs` — helpers puros de formatação

**Files:**
- Modify: `rust/src/formatters/shared.rs` (append; já tem `to_display`/`to_health`/`to_window_display`)
- Test: dentro de `shared.rs`

**Interfaces:**
- Consumes: nada novo.
- Produces: `formatters::shared::{ format_percent(Option<f64>)->String, WindowKind (FiveHour/SevenDay/Other), classify_window(Option<i64>)->WindowKind, normalize_plan(Option<&str>)->Option<String>, eta_label(DisplayMode)->&'static str }`. (`normalize_plan_label` vem no Plano 3b junto do tipo que precisa dele.)

- [ ] **Step 1: Escrever os testes falhando (append em `shared.rs`)**

```rust
#[cfg(test)]
mod helper_tests {
    use super::*;

    #[test]
    fn format_percent_rounds_and_handles_none() {
        assert_eq!(format_percent(None), "?%");
        assert_eq!(format_percent(Some(74.6)), "75%");
        assert_eq!(format_percent(Some(0.0)), "0%");
    }

    #[test]
    fn classify_window_tolerances() {
        assert_eq!(classify_window(Some(300)), WindowKind::FiveHour);
        assert_eq!(classify_window(Some(390)), WindowKind::FiveHour); // 300+90
        assert_eq!(classify_window(Some(391)), WindowKind::Other);
        assert_eq!(classify_window(Some(10080)), WindowKind::SevenDay);
        assert_eq!(classify_window(Some(11520)), WindowKind::SevenDay); // 10080+1440
        assert_eq!(classify_window(Some(0)), WindowKind::Other);
        assert_eq!(classify_window(None), WindowKind::Other);
    }

    #[test]
    fn normalize_plan_map_and_titlecase() {
        assert_eq!(normalize_plan(Some("pro")).as_deref(), Some("Pro"));
        assert_eq!(normalize_plan(Some("TEAM")).as_deref(), Some("Business"));
        assert_eq!(normalize_plan(Some("api_key")).as_deref(), Some("API Key"));
        assert_eq!(normalize_plan(Some("custom_plan")).as_deref(), Some("Custom Plan"));
        assert_eq!(normalize_plan(Some("  ")), None);
        assert_eq!(normalize_plan(None), None);
    }

    #[test]
    fn eta_label_by_mode() {
        use crate::settings::DisplayMode;
        assert_eq!(eta_label(DisplayMode::Used), "Resets in");
        assert_eq!(eta_label(DisplayMode::Remaining), "Full in");
    }
}
```

- [ ] **Step 2: Rodar (deve falhar)** — `cargo test --manifest-path rust/Cargo.toml helper_tests 2>&1 | head` → FAIL de compilação.

- [ ] **Step 3: Implementar (append em `shared.rs`, antes do `#[cfg(test)]` existente ou em novo bloco)**

```rust
use crate::settings::DisplayMode;

/// `?%` quando None; senão `{arredondado}%`.
pub fn format_percent(val: Option<f64>) -> String {
    match val {
        None => "?%".to_string(),
        Some(v) => format!("{}%", v.round() as i64),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowKind {
    FiveHour,
    SevenDay,
    Other,
}

/// fiveHour se |min-300|<=90; sevenDay se |min-10080|<=1440; senão other. None/<=0 → other.
pub fn classify_window(minutes: Option<i64>) -> WindowKind {
    match minutes {
        Some(m) if m > 0 => {
            if (m - 300).abs() <= 90 {
                WindowKind::FiveHour
            } else if (m - 10080).abs() <= 1440 {
                WindowKind::SevenDay
            } else {
                WindowKind::Other
            }
        }
        _ => WindowKind::Other,
    }
}

/// Normaliza o nome do plano (mapa conhecido ou titlecase). None/vazio → None.
pub fn normalize_plan(raw: Option<&str>) -> Option<String> {
    let raw = raw?;
    let key = raw.trim().to_lowercase();
    if key.is_empty() {
        return None;
    }
    let mapped = match key.as_str() {
        "free" => "Free",
        "go" => "Go",
        "plus" => "Plus",
        "pro" => "Pro",
        "business" | "team" => "Business",
        "enterprise" => "Enterprise",
        "edu" | "education" => "Edu",
        "apikey" | "api_key" => "API Key",
        _ => return Some(titlecase_plan(raw)),
    };
    Some(mapped.to_string())
}

/// Substitui `_`/`-` por espaço e capitaliza a 1ª letra de cada palavra.
fn titlecase_plan(raw: &str) -> String {
    raw.split(|c| c == '_' || c == '-')
        .filter(|w| !w.is_empty())
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

pub fn eta_label(mode: DisplayMode) -> &'static str {
    match mode {
        DisplayMode::Used => "Resets in",
        DisplayMode::Remaining => "Full in",
    }
}
```
> Nota: o TS titlecase usa regex `\b\w` (capitaliza após qualquer boundary, incluindo após dígitos). Esta versão capitaliza por palavra split em `_`/`-`. Para os planos reais (mapa conhecido + nomes simples) o resultado é idêntico; o golden de plano no Plano 3c valida.

- [ ] **Step 4: Rodar (deve passar)** — `cargo test --manifest-path rust/Cargo.toml helper_tests` → 4 PASS.

- [ ] **Step 5: fmt + clippy + commit**

```bash
cargo fmt --manifest-path rust/Cargo.toml
cargo clippy --manifest-path rust/Cargo.toml --all-targets -- -D warnings
git add rust/src/formatters/shared.rs
git commit -m "feat(rust): helpers de formatacao puros"
```

---

### Task 2: `clock.rs` + helpers de tempo

**Files:**
- Create: `rust/src/formatters/clock.rs`
- Modify: `rust/src/formatters/shared.rs` (append: `format_eta`, `format_reset_time`, `format_ago`)
- Modify: `rust/src/formatters/mod.rs` (adicionar `pub mod clock;`)

**Interfaces:**
- Consumes: `time::{OffsetDateTime, UtcOffset}`.
- Produces:
  - `formatters::clock::Clock { now: OffsetDateTime, local_offset: UtcOffset }` com `Clock::from_env() -> Clock` (resolve agora UTC + offset local; offset falha→UTC). `Clock` deriva `Debug, Clone, Copy`.
  - `formatters::shared::{ format_eta(&Clock, Option<&str>, f64)->String, format_reset_time(&Clock, Option<&str>, f64)->String, format_ago(&Clock, &str)->String }`.

- [ ] **Step 1: Escrever os testes falhando**

Em `rust/src/formatters/clock.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_env_does_not_panic() {
        let c = Clock::from_env();
        // smoke: campos acessíveis
        let _ = c.now.unix_timestamp();
        let _ = c.local_offset.whole_hours();
    }
}
```

Em `shared.rs` (append; usa um Clock fixo determinístico):
```rust
#[cfg(test)]
mod time_tests {
    use super::*;
    use crate::formatters::clock::Clock;
    use time::macros::datetime;

    // Clock fixo: agora = 2026-06-19 12:00:00 UTC, offset local = +03:00.
    fn fixed_clock() -> Clock {
        Clock {
            now: datetime!(2026-06-19 12:00:00 UTC),
            local_offset: time::UtcOffset::from_hms(3, 0, 0).unwrap(),
        }
    }

    #[test]
    fn format_eta_cases() {
        let c = fixed_clock();
        assert_eq!(format_eta(&c, None, 50.0), "?");
        assert_eq!(format_eta(&c, Some("2026-06-19T14:05:00Z"), 50.0), "2h 05m");
        assert_eq!(format_eta(&c, Some("2026-06-21T13:00:00Z"), 50.0), "2d 01h");
        assert_eq!(format_eta(&c, Some("2026-06-19T11:00:00Z"), 50.0), "0h 00m"); // passado
        assert_eq!(format_eta(&c, Some("2026-06-19T14:00:00Z"), 100.0), "Full");
    }

    #[test]
    fn format_reset_time_uses_local_offset() {
        let c = fixed_clock();
        // 14:05 UTC + offset +03:00 = 17:05 local
        assert_eq!(format_reset_time(&c, Some("2026-06-19T14:05:00Z"), 50.0), "(17:05)");
        assert_eq!(format_reset_time(&c, None, 50.0), "(??:??)");
        assert_eq!(format_reset_time(&c, Some("2026-06-19T14:00:00Z"), 100.0), "");
    }

    #[test]
    fn format_ago_cases() {
        let c = fixed_clock();
        assert_eq!(format_ago(&c, "2026-06-19T11:59:30Z"), "just now"); // 30s
        assert_eq!(format_ago(&c, "2026-06-19T11:30:00Z"), "30m ago");
        assert_eq!(format_ago(&c, "2026-06-19T09:00:00Z"), "3h ago");
    }
}
```

- [ ] **Step 2: Rodar (deve falhar)** — `cargo test --manifest-path rust/Cargo.toml -- clock time_tests 2>&1 | head` → FAIL de compilação.

- [ ] **Step 3: Implementar `rust/src/formatters/clock.rs`**

```rust
//! Relógio injetável. Resolvido uma vez no `main` (startup single-thread) e passado
//! adiante — evita chamar `current_local_offset()` em runtime (frágil em multi-thread
//! e não-determinístico em CI). `format_reset_time` usa `local_offset` p/ HH:MM local.

use time::{OffsetDateTime, UtcOffset};

#[derive(Debug, Clone, Copy)]
pub struct Clock {
    pub now: OffsetDateTime,
    pub local_offset: UtcOffset,
}

impl Clock {
    /// Resolve agora (UTC) + offset local do SO. Se o offset não puder ser determinado
    /// (ex.: chamado após spawn de threads), cai para UTC.
    pub fn from_env() -> Self {
        let local_offset = UtcOffset::current_local_offset().unwrap_or(UtcOffset::UTC);
        Self {
            now: OffsetDateTime::now_utc(),
            local_offset,
        }
    }
}
```

- [ ] **Step 4: Implementar os helpers de tempo (append em `shared.rs`)**

```rust
use crate::formatters::clock::Clock;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

fn parse_iso(iso: &str) -> Option<OffsetDateTime> {
    OffsetDateTime::parse(iso, &Rfc3339).ok()
}

/// `Full` se remaining==100; `?` sem iso; `0h 00m` se já passou; `{d}d {hh}h` ou `{h}h {mm}m`.
pub fn format_eta(clock: &Clock, iso: Option<&str>, remaining: f64) -> String {
    if remaining == 100.0 {
        return "Full".to_string();
    }
    let Some(iso) = iso else {
        return "?".to_string();
    };
    let Some(dt) = parse_iso(iso) else {
        return "?".to_string();
    };
    let diff = dt - clock.now;
    if diff.is_negative() {
        return "0h 00m".to_string();
    }
    let secs = diff.whole_seconds();
    let d = secs / 86_400;
    let h = (secs % 86_400) / 3_600;
    let m = (secs % 3_600) / 60;
    if d > 0 {
        format!("{d}d {h:02}h")
    } else {
        format!("{h}h {m:02}m")
    }
}

/// `` se remaining==100; `(??:??)` sem iso; senão `({HH}:{MM})` em horário LOCAL.
pub fn format_reset_time(clock: &Clock, iso: Option<&str>, remaining: f64) -> String {
    if remaining == 100.0 {
        return String::new();
    }
    let Some(iso) = iso else {
        return "(??:??)".to_string();
    };
    let Some(dt) = parse_iso(iso) else {
        return "(??:??)".to_string();
    };
    let local = dt.to_offset(clock.local_offset);
    format!("({:02}:{:02})", local.hour(), local.minute())
}

/// `just now` (<60s); `{m}m ago` (<60min); senão `{h}h ago`.
pub fn format_ago(clock: &Clock, iso: &str) -> String {
    let Some(dt) = parse_iso(iso) else {
        return "?".to_string();
    };
    let diff = clock.now - dt;
    if diff.whole_milliseconds() < 60_000 {
        return "just now".to_string();
    }
    let mins = diff.whole_minutes();
    if mins < 60 {
        format!("{mins}m ago")
    } else {
        format!("{}h ago", mins / 60)
    }
}
```
> Nota: requer a feature `macros` do `time` p/ os testes (`datetime!`). Se não estiver habilitada no Cargo.toml, adicione `"macros"` às features do `time` (já tem `parsing`/`formatting`/`local-offset`/`serde-well-known`).

- [ ] **Step 5: Wirar `rust/src/formatters/mod.rs`** — adicionar `pub mod clock;` (ordem alfabética, após `?` — fica antes de `json`? alfabético: clock, json, render_ansi, render_pango, segments, shared). Ler o mod.rs atual e inserir `pub mod clock;` no topo (antes de `json`).

- [ ] **Step 6: Rodar + fmt + clippy + commit**

```bash
cargo test --manifest-path rust/Cargo.toml -- clock time_tests
cargo fmt --manifest-path rust/Cargo.toml
cargo clippy --manifest-path rust/Cargo.toml --all-targets -- -D warnings
git add rust/src/formatters/clock.rs rust/src/formatters/shared.rs rust/src/formatters/mod.rs rust/Cargo.toml rust/Cargo.lock
git commit -m "feat(rust): Clock injetavel + helpers de tempo"
```
Expected: clock + time_tests passam; suite cresce; clippy clean.

---

### Task 3: `builders/shared.rs` — primitivos de builder

**Files:**
- Create: `rust/src/formatters/builders/mod.rs`
- Create: `rust/src/formatters/builders/shared.rs`
- Modify: `rust/src/formatters/mod.rs` (adicionar `pub mod builders;`)

**Interfaces:**
- Consumes: `theme::{ColorToken, box_chars}`, `segments::{Line, Segment, bar_segments, indicator_segments, color_for_display}`, `shared::{to_window_display, format_percent, format_eta, format_reset_time, format_ago}`, `clock::Clock`, `settings::DisplayMode`, `providers::types::QuotaWindow`.
- Produces: `formatters::builders::shared::{ TOOLTIP_BORDER: usize=56, BuildOptions, vline(ColorToken)->Line, label_line(&str, ColorToken, ColorToken)->Line, header_line(&str, usize, ColorToken)->Line, model_line(&Clock, &str, Option<&QuotaWindow>, usize, DisplayMode, ColorToken, Option<&str>)->Line, build_footer_line(&Clock, Option<&str>, ColorToken)->Line }`.

- [ ] **Step 1: Escrever os testes falhando em `builders/shared.rs`**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::formatters::clock::Clock;
    use crate::formatters::render_pango::render_pango;
    use crate::providers::types::QuotaWindow;
    use crate::settings::DisplayMode;
    use crate::theme::ColorToken;
    use time::macros::datetime;

    fn clk() -> Clock {
        Clock { now: datetime!(2026-06-19 12:00:00 UTC), local_offset: time::UtcOffset::UTC }
    }

    #[test]
    fn vline_is_single_accent_bar() {
        let l = vline(ColorToken::Orange);
        assert_eq!(l.len(), 1);
        assert_eq!(l[0].text, "┃");
        assert_eq!(l[0].color, ColorToken::Orange);
    }

    #[test]
    fn label_line_renders_connector_diamond_label() {
        let out = render_pango(&[label_line("Models", ColorToken::Magenta, ColorToken::Magenta)]);
        // ┣━ + ' ' (raw) + ◆ Models
        assert!(out.contains("┣━"));
        assert!(out.contains("◆ Models"));
    }

    #[test]
    fn header_line_pads_to_width() {
        // headerWidth 10, title "AB" → fill = 8 dashes
        let l = header_line("AB", 10, ColorToken::Orange);
        let dashes: usize = l.iter().filter(|s| s.text.chars().all(|c| c == '━')).map(|s| s.text.chars().count()).sum();
        // ┏━ tem 1 ━; o fill tem 8 → total ━ = 9; aqui checamos só o fill segment
        let fill = l.last().unwrap();
        assert_eq!(fill.text.chars().count(), 8);
    }

    #[test]
    fn footer_simple_is_56_wide() {
        let l = build_footer_line(&clk(), None, ColorToken::Orange);
        let total: usize = l.iter().map(|s| s.text.chars().count()).sum();
        assert_eq!(total, 56); // ┗ + 55×━
    }

    #[test]
    fn footer_cached_is_56_wide_with_stamp() {
        // fetched 30min atrás → ' cached · 30m ago '
        let l = build_footer_line(&clk(), Some("2026-06-19T11:30:00Z"), ColorToken::Orange);
        let total: usize = l.iter().map(|s| s.text.chars().count()).sum();
        assert_eq!(total, 56);
        let rendered: String = l.iter().map(|s| s.text.as_ref()).collect();
        assert!(rendered.contains(" cached · 30m ago "));
    }

    #[test]
    fn model_line_segment_shape() {
        let w = QuotaWindow { remaining: 75.0, resets_at: Some("2026-06-19T14:00:00Z".into()), window_minutes: None, used: None };
        let l = model_line(&clk(), "All Models", Some(&w), 20, DisplayMode::Remaining, ColorToken::Orange, None);
        // primeiro segment = bar vertical accent; contém nome padded a 20 e '75%' e '→'
        assert_eq!(l[0].text, "┃");
        assert_eq!(l[0].color, ColorToken::Orange);
        let rendered: String = l.iter().map(|s| s.text.as_ref()).collect();
        assert!(rendered.contains("All Models")); // padEnd 20
        assert!(rendered.contains("75%"));
        assert!(rendered.contains("→ "));
    }

    #[test]
    fn model_line_null_eta_override() {
        let w = QuotaWindow { remaining: 50.0, resets_at: None, window_minutes: None, used: None };
        let l = model_line(&clk(), "X", Some(&w), 5, DisplayMode::Remaining, ColorToken::Green, Some("N/A"));
        let rendered: String = l.iter().map(|s| s.text.as_ref()).collect();
        assert!(rendered.contains("→ N/A"));
    }
}
```

- [ ] **Step 2: Rodar (deve falhar)** — `cargo test --manifest-path rust/Cargo.toml builders 2>&1 | head` → FAIL de compilação.

- [ ] **Step 3: Implementar `rust/src/formatters/builders/shared.rs`**

```rust
//! Primitivos compartilhados pelos builders por-provider (Plano 3b). Funções puras
//! `-> Line`. Layout: borda de 56 chars; box-drawing pesado.

use crate::formatters::clock::Clock;
use crate::formatters::segments::{bar_segments, color_for_display, indicator_segments, Line, Segment};
use crate::formatters::shared::{format_ago, format_eta, format_percent, format_reset_time, to_window_display};
use crate::providers::types::QuotaWindow;
use crate::settings::DisplayMode;
use crate::theme::{box_chars, ColorToken};

pub const TOOLTIP_BORDER: usize = 56;

/// Opções resolvidas pela superfície (waybar/terminal/tui) e passadas ao builder.
#[derive(Debug, Clone)]
pub struct BuildOptions {
    pub mode: DisplayMode,
    pub header_title: String,
    pub header_width: usize,
    pub label_color: ColorToken,
    pub footer_fetched_at: Option<String>,
    pub plan_label: Option<String>,
    pub amp_free_tier_layout: AmpLayout,
    pub account_in_header: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AmpLayout {
    Sublines,
    Inline,
    Generic,
}

/// Linha vertical vazia com o accent do provider: `┃`.
pub fn vline(color: ColorToken) -> Line {
    vec![Segment::new(box_chars::V, color)]
}

/// `┣━ ◆ {text}` — connector + diamante + label em bold.
pub fn label_line(text: &str, label_color: ColorToken, connector_color: ColorToken) -> Line {
    vec![
        Segment::new(format!("{}{}", box_chars::LT, box_chars::H), connector_color),
        Segment::raw_text(" "),
        Segment::new(format!("{} {}", box_chars::DIAMOND, text), label_color).bold(),
    ]
}

/// `┏━ {title} ━…` preenchido até `header_width`.
pub fn header_line(title: &str, header_width: usize, color: ColorToken) -> Line {
    let fill = header_width.saturating_sub(title.chars().count()).max(1);
    vec![
        Segment::new(format!("{}{}", box_chars::TL, box_chars::H), color),
        Segment::raw_text(" "),
        Segment::new(title.to_string(), color).bold(),
        Segment::raw_text(" "),
        Segment::new(box_chars::H.repeat(fill), color),
    ]
}

/// `┗━…[ cached · {ago} ]…` sempre com 56 chars de largura total.
pub fn build_footer_line(clock: &Clock, fetched_at: Option<&str>, color: ColorToken) -> Line {
    match fetched_at {
        None => vec![Segment::new(
            format!("{}{}", box_chars::BL, box_chars::H.repeat(55)),
            color,
        )],
        Some(iso) => {
            let stamp = format!(" cached · {} ", format_ago(clock, iso));
            let total_dashes = (TOOLTIP_BORDER - 1).saturating_sub(stamp.chars().count());
            let left = (total_dashes / 2).max(1);
            let right = total_dashes.saturating_sub(left).max(1);
            vec![
                Segment::new(format!("{}{}", box_chars::BL, box_chars::H.repeat(left)), color),
                Segment::new(stamp, ColorToken::Comment),
                Segment::new(box_chars::H.repeat(right), color),
            ]
        }
    }
}

/// Linha de modelo: `┃  {indicador} {nome:<maxlen} {barra}  {pct:>4} → {eta} {reset}`.
#[allow(clippy::too_many_arguments)]
pub fn model_line(
    clock: &Clock,
    name: &str,
    window: Option<&QuotaWindow>,
    max_len: usize,
    mode: DisplayMode,
    provider_color: ColorToken,
    null_eta_text: Option<&str>,
) -> Line {
    let disp = to_window_display(window, mode);
    let reset = window.and_then(|w| w.resets_at.as_deref());
    let rem = window.map(|w| w.remaining).unwrap_or(0.0);

    let eta_text = match (null_eta_text, reset) {
        (Some(na), None) => format!("→ {na}"),
        _ => format!(
            "→ {} {}",
            format_eta(clock, reset, rem),
            format_reset_time(clock, reset, rem)
        ),
    };

    let mut line: Line = vec![
        Segment::new(box_chars::V, provider_color),
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
    line.push(Segment::new(eta_text, ColorToken::Cyan));
    line
}
```
> Nota sobre `eta_text` quando `format_reset_time` retorna `` (remaining==100): o TS produz `→ {eta} ` (com espaço final). Aqui `format!("→ {} {}", eta, "")` = `→ Full ` (espaço final). Idêntico ao TS — o golden do Plano 3c confirma. Não trimar.

- [ ] **Step 4: Criar `rust/src/formatters/builders/mod.rs`**

```rust
pub mod shared;
```

- [ ] **Step 5: Wirar `rust/src/formatters/mod.rs`** — adicionar `pub mod builders;` (ordem alfabética, no topo, antes de `clock`).

- [ ] **Step 6: Rodar + fmt + clippy + commit**

```bash
cargo test --manifest-path rust/Cargo.toml builders
cargo fmt --manifest-path rust/Cargo.toml
cargo clippy --manifest-path rust/Cargo.toml --all-targets -- -D warnings
git add rust/src/formatters/builders/ rust/src/formatters/mod.rs
git commit -m "feat(rust): primitivos de builder (modelLine etc)"
```
Expected: 7 testes de builders passam; suite total cresce; clippy clean.

---

## Self-review (preenchido)

- **Cobertura:** helpers de `formatters/shared.ts` (format_percent/eta/reset_time/classify/normalize_plan/eta_label) + `builders/shared.ts` (TOOLTIP_BORDER/vline/label_line/header/footer/model_line + formatAgo) — todos mapeados a uma task. `normalize_plan_label` e os builders por-provider = Plano 3b. waybar/terminal/view-model + golden = Plano 3c.
- **Armadilha de tempo tratada:** `Clock` injetado; `format_reset_time` usa `local_offset` do Clock (não `current_local_offset()` em runtime); testes usam Clock fixo → determinístico independente do TZ do CI.
- **Char-count multibyte:** footer/header usam `.chars().count()` (┗/━/┃ são multi-byte) — não `.len()`.
- **Sem placeholder.** Todo código presente. `#[allow(clippy::too_many_arguments)]` em `model_line` (7 args espelham o TS; refatorar p/ struct seria over-engineering agora).
- **Dep:** Task 2 pode precisar adicionar a feature `"macros"` ao `time` no Cargo.toml (p/ `datetime!` nos testes) — incluído no git add da Task 2.
