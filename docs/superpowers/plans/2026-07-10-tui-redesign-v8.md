# Redesign da TUI v8 — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implementar a spec `docs/superpowers/specs/2026-07-10-tui-redesign-v8-design.md`: TUI sem Visão Geral (boot direto no provider, right-click abre a TUI focada), Detail com layout real e gráfico tokens/h por modelo, Histórico com dias expandíveis/sessões/gastos, fix do `interval` do Waybar, tema One Dark Turbo com gauges sólidos.

**Architecture:** Camada de dados primeiro (nomes de modelo, pricing, session/project no `UsageRecord`, novos buckets), depois primitivos visuais (tokens de tema, gauge sólido, widget de chart de colunas), depois telas (Detail, History, navegação/boot, action-right, Config), por fim docs e gate final. Cada task compila e passa testes sozinha.

**Tech Stack:** Rust + ratatui + insta (snapshots) + tokio (testes async). Sem dependências novas.

## Global Constraints

- Rust/cargo only; sem Node/npm/bun (CLAUDE.md §1).
- **Nunca `unwrap()`/`expect()` em código de produção** — `?`, guards ou `unwrap_or*` (CLAUDE.md §3). Em `#[cfg(test)]` é permitido.
- Provider error strings são contrato — não alterar nenhuma (CLAUDE.md §3).
- XML-escape só em `render_pango.rs`; este plano não toca render Pango.
- Usar constantes de `src/app_identity.rs`; nunca hardcode de nomes.
- stdout limpo: logs via `log::` (vão pro stderr).
- Não mutar desktop ao vivo: nunca rodar `agent-bar setup/update/uninstall`; testes só com temp dirs + `XDG_*`.
- `XDG_CONFIG_HOME`/`XDG_CACHE_HOME` setados ANTES de import que leia `src/config.rs` em testes.
- Conventional Commits em PT, subject ≤ 50 chars.
- Gotcha RTK: um único filtro posicional por invocação de `cargo test`.
- Snapshots insta: Waybar é byte-for-byte; TUI snapshots atualizam SÓ porque o contrato de display mudou de propósito (este redesign). Use `cargo insta review` ou `INSTA_UPDATE=auto` conscientemente.
- Implementers: Read cada arquivo antes de Edit; se Edit falhar com `string not found`, re-Read; após retorno de outro agente o Read anterior está morto — re-Read.

## Ordem e dependências

```
T1 model_names ─┬─► T4 bucket_by_model_hour ─► T8 column_chart ─► T9 Detail ─► T10 History
T2 pricing ─────┤                                                    ▲
T3 session/proj ┴─► T5 sessions_by_day ─────────────────────────────┘
T6 theme tokens ─► T7 gauge sólido ─► T9/T10
T9+T10 ─► T11 navegação/boot ─► T12 action-right
T13 interval fix (independente; depois de T12 p/ evitar conflito em main.rs)
T14 config sections (depois de T13)
T15 docs · T16 gate final
```

---

### Task 1: Nomes de modelo tratados (`model_names.rs`)

**Files:**
- Create: `src/usage/model_names.rs`
- Modify: `src/usage/mod.rs:4-9` (declarar `pub mod model_names;`)

**Interfaces:**
- Produces: `pub fn display_model_name(model: &str) -> String` e
  `pub fn series_slot_for_model(model: &str) -> u8` (0..=5; 0=fable, 1=opus,
  2=sonnet, 3=haiku, 4=codex/gpt, 5=outros). Consumidos por T4, T8, T9, T10.

- [ ] **Step 1: Write the failing tests**

Criar `src/usage/model_names.rs` só com os testes (sem implementação ainda compila? não — escreva o arquivo completo do Step 3 SEM os corpos, ou aceite escrever teste+impl no mesmo arquivo e verificar via teste vermelho por assert errado. Fluxo prático em Rust: escreva o arquivo com `todo!()` nos corpos):

```rust
//! Humanização de ids de modelo pra display ("claude-opus-4-8" → "Opus 4.8")
//! e slot de série de gráfico por família (cor segue a entidade, nunca o rank).

/// Nome tratado pra display. Fallback: id original (o truncamento com `…`
/// continua sendo responsabilidade do render).
pub fn display_model_name(model: &str) -> String {
    todo!()
}

/// Slot de série (0..=5) por família — mapeia pra ColorToken::Series1..6:
/// 0=fable/mythos, 1=opus, 2=sonnet, 3=haiku, 4=codex/gpt/o-series, 5=outros.
pub fn series_slot_for_model(model: &str) -> u8 {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn treats_current_claude_models() {
        assert_eq!(display_model_name("claude-fable-5"), "Fable 5");
        assert_eq!(display_model_name("claude-opus-4-8"), "Opus 4.8");
        assert_eq!(display_model_name("claude-sonnet-5"), "Sonnet 5");
        assert_eq!(display_model_name("claude-haiku-4-5"), "Haiku 4.5");
        assert_eq!(display_model_name("claude-opus-4-5-20260101"), "Opus 4.5");
    }

    #[test]
    fn treats_codex_models() {
        assert_eq!(display_model_name("gpt-5.5-codex"), "GPT-5.5 Codex");
        assert_eq!(display_model_name("gpt-5.5"), "GPT-5.5");
        assert_eq!(display_model_name("gpt-5.3-codex"), "GPT-5.3 Codex");
    }

    #[test]
    fn unknown_model_falls_back_to_id() {
        assert_eq!(display_model_name("mystery-model-9"), "mystery-model-9");
        assert_eq!(display_model_name(""), "");
    }

    #[test]
    fn slots_follow_family() {
        assert_eq!(series_slot_for_model("claude-fable-5"), 0);
        assert_eq!(series_slot_for_model("claude-mythos-5"), 0);
        assert_eq!(series_slot_for_model("claude-opus-4-8"), 1);
        assert_eq!(series_slot_for_model("claude-sonnet-5"), 2);
        assert_eq!(series_slot_for_model("claude-haiku-4-5"), 3);
        assert_eq!(series_slot_for_model("gpt-5.5-codex"), 4);
        assert_eq!(series_slot_for_model("gpt-5.5"), 4);
        assert_eq!(series_slot_for_model("o4-mini"), 4);
        assert_eq!(series_slot_for_model("mystery"), 5);
    }
}
```

Adicionar em `src/usage/mod.rs` (junto dos outros `pub mod`): `pub mod model_names;`

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test usage::model_names`
Expected: FAIL/panic com `not yet implemented` (todo!)

- [ ] **Step 3: Write minimal implementation**

Substituir os `todo!()`:

```rust
pub fn display_model_name(model: &str) -> String {
    let m = model.to_ascii_lowercase();

    // Claude: "claude-<família>-<maj>[-<min>][-<data YYYYMMDD>]"
    if let Some(rest) = m.strip_prefix("claude-") {
        let mut parts: Vec<&str> = rest.split('-').collect();
        // Descarta sufixo de data (8+ dígitos) se presente.
        if parts.last().is_some_and(|p| p.len() >= 8 && p.chars().all(|c| c.is_ascii_digit())) {
            parts.pop();
        }
        if let Some((family, version)) = parts.split_first() {
            let fam = capitalize(family);
            if version.is_empty() {
                return fam;
            }
            return format!("{} {}", fam, version.join("."));
        }
    }

    // OpenAI/Codex: "gpt-5.5-codex" → "GPT-5.5 Codex"; "gpt-5.5" → "GPT-5.5".
    if let Some(rest) = m.strip_prefix("gpt-") {
        let (ver, suffix) = match rest.split_once('-') {
            Some((v, s)) => (v, Some(s)),
            None => (rest, None),
        };
        return match suffix {
            Some(s) => format!("GPT-{} {}", ver, capitalize(s)),
            None => format!("GPT-{ver}"),
        };
    }

    model.to_string()
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

pub fn series_slot_for_model(model: &str) -> u8 {
    let m = model.to_ascii_lowercase();
    if m.contains("fable") || m.contains("mythos") {
        0
    } else if m.contains("opus") {
        1
    } else if m.contains("sonnet") {
        2
    } else if m.contains("haiku") {
        3
    } else if m.contains("codex") || m.starts_with("gpt-") || m.starts_with("o4") {
        4
    } else {
        5
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test usage::model_names`
Expected: PASS (4 testes)

- [ ] **Step 5: Commit**

```bash
git add src/usage/model_names.rs src/usage/mod.rs
git commit -m "feat: humanização de nomes de modelo"
```

---

### Task 2: Pricing — Fable/Mythos, Sonnet 5, Opus legado, warn

**Files:**
- Modify: `src/usage/pricing.rs`

**Interfaces:**
- Produces: `pricing_for`/`cost_usd_of` inalterados na assinatura. Novas
  entradas de tabela. `cost_usd_of` de modelo desconhecido continua `None`,
  mas agora loga `log::warn!` UMA vez por modelo por execução.

**Fonte (verificada 2026-07-10, platform.claude.com/docs/en/about-claude/pricing),
USD/MTok — input / output / cache_read / cache_write(5m):**
- fable, mythos: 10 / 50 / 1.00 / 12.50
- opus 4.5–4.8 (já correto): 5 / 25 / 0.50 / 6.25
- opus 4.1 e anteriores (legado): 15 / 75 / 1.50 / 18.75
- sonnet-5 (introdutório até 2026-08-31): 2 / 10 / 0.20 / 2.50
- sonnet ≤4.6 (já correto): 3 / 15 / 0.30 / 3.75
- haiku 4.5 (já correto): 1 / 5 / 0.10 / 1.25

- [ ] **Step 1: Write the failing tests**

Adicionar ao `mod tests` de `src/usage/pricing.rs`:

```rust
#[test]
fn fable_and_mythos_pricing_2026_07_10() {
    let fable = pricing_for("claude-fable-5").unwrap();
    assert_eq!(
        (fable.input, fable.output, fable.cache_read, fable.cache_write),
        (10.0, 50.0, 1.0, 12.5)
    );
    assert_eq!(pricing_for("claude-mythos-5"), pricing_for("claude-fable-5"));
}

#[test]
fn sonnet_5_intro_pricing_differs_from_sonnet_4x() {
    let s5 = pricing_for("claude-sonnet-5").unwrap();
    assert_eq!((s5.input, s5.output), (2.0, 10.0));
    let s46 = pricing_for("claude-sonnet-4-6").unwrap();
    assert_eq!((s46.input, s46.output), (3.0, 15.0));
}

#[test]
fn legacy_opus_41_keeps_old_pricing() {
    let o41 = pricing_for("claude-opus-4-1").unwrap();
    assert_eq!((o41.input, o41.output), (15.0, 75.0));
    let o48 = pricing_for("claude-opus-4-8").unwrap();
    assert_eq!((o48.input, o48.output), (5.0, 25.0));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test usage::pricing`
Expected: FAIL — `fable` retorna None; `sonnet-5` retorna 3.0/15.0; `opus-4-1` retorna 5.0/25.0

- [ ] **Step 3: Implementation**

Em `pricing_for`, o bloco Anthropic vira (do mais específico pro mais geral —
**a ordem importa**):

```rust
    // --- Anthropic ---
    if m.contains("fable") || m.contains("mythos") {
        return Some(Pricing { input: 10.0, output: 50.0, cache_read: 1.0, cache_write: 12.5 });
    }
    if m.contains("opus") {
        // Legado (≤4.1): claude-opus-4-1 / claude-opus-4 sem minor ≥5 / opus-3.
        if m.contains("opus-4-1") || m.contains("opus-3") {
            return Some(Pricing { input: 15.0, output: 75.0, cache_read: 1.5, cache_write: 18.75 });
        }
        return Some(Pricing { input: 5.0, output: 25.0, cache_read: 0.50, cache_write: 6.25 });
    }
    if m.contains("sonnet-5") {
        // Introdutório até 2026-08-31 (platform.claude.com); revisar em set/2026.
        return Some(Pricing { input: 2.0, output: 10.0, cache_read: 0.20, cache_write: 2.50 });
    }
    if m.contains("sonnet") {
        return Some(Pricing { input: 3.0, output: 15.0, cache_read: 0.30, cache_write: 3.75 });
    }
    if m.contains("haiku") {
        return Some(Pricing { input: 1.0, output: 5.0, cache_read: 0.10, cache_write: 1.25 });
    }
```

Atualizar o comentário de cabeçalho do arquivo: `Atualizado: 2026-07-10` +
fable/mythos/sonnet-5/opus-legado na lista de fontes.

Warn-once para modelo desconhecido — em `cost_usd_of`, antes do `None` final
de `pricing_for`, não dá (função pura consultada N vezes). Implementar no
próprio `cost_usd_of`:

```rust
use std::collections::HashSet;
use std::sync::Mutex;

/// Modelos desconhecidos já avisados nesta execução (warn-once por modelo).
static WARNED_UNKNOWN: Mutex<Option<HashSet<String>>> = Mutex::new(None);

pub fn cost_usd_of(rec: &UsageRecord) -> Option<f64> {
    let model = rec.model.as_deref()?;
    let p = match pricing_for(model) {
        Some(p) => p,
        None => {
            if let Ok(mut guard) = WARNED_UNKNOWN.lock() {
                let set = guard.get_or_insert_with(HashSet::new);
                if set.insert(model.to_string()) {
                    log::warn!("modelo sem preço conhecido: {model} (custo aparecerá como —)");
                }
            }
            return None;
        }
    };
    let cost = (rec.input as f64 * p.input
        + rec.output as f64 * p.output
        + rec.cache_read as f64 * p.cache_read
        + rec.cache_write as f64 * p.cache_write)
        / 1_000_000.0;
    Some(cost)
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test usage::pricing`
Expected: PASS (todos, incl. os pré-existentes `verified_prices_2026_06_20`)

Run: `cargo test usage`
Expected: PASS (o teste `aggregate_sums_tokens_and_cost_per_provider` usa opus 4.8 = $30 — inalterado)

- [ ] **Step 5: Commit**

```bash
git add src/usage/pricing.rs
git commit -m "feat: preços fable/mythos, sonnet 5 e opus legado"
```

---

### Task 3: `session_id` + `project` no `UsageRecord`

**Files:**
- Modify: `src/usage/mod.rs` (struct + `records()`)
- Modify: `src/usage/claude.rs` (extrair `cwd` → project)
- Modify: `src/usage/codex.rs`, `src/usage/buckets.rs`, `src/usage/cache.rs`,
  `src/usage/pricing.rs` (construtores de teste ganham os campos novos)

**Interfaces:**
- Produces: `UsageRecord { …, pub session_id: Option<String>, pub project: Option<String> }`.
  - `session_id` = file stem do `.jsonl` (Claude e Codex), preenchido em `records()`.
  - `project` = basename do campo `cwd` do JSONL do Claude (preenchido no parser);
    Codex fica `None`.
- Consumed by: T5 (`sessions_by_day`).

- [ ] **Step 1: Write the failing tests**

Em `src/usage/mod.rs`, `mod tests`:

```rust
#[test]
fn records_carry_session_id_from_file_stem() {
    let claude = tempdir().unwrap();
    let codex = tempdir().unwrap();
    write(
        claude.path(),
        "-home-user-proj/abc-123.jsonl",
        r#"{"type":"assistant","timestamp":"2026-07-10T10:00:00Z","cwd":"/home/user/proj","message":{"model":"claude-fable-5","usage":{"input_tokens":10,"output_tokens":5}}}"#,
    );
    let recs = records(AggregateOptions {
        claude_dir: claude.path(),
        codex_dir: codex.path(),
        fx_rate: 5.0,
        amp_meta: None,
    });
    assert_eq!(recs.len(), 1);
    assert_eq!(recs[0].session_id.as_deref(), Some("abc-123"));
    assert_eq!(recs[0].project.as_deref(), Some("proj"));
}
```

Em `src/usage/claude.rs`, `mod tests`:

```rust
#[test]
fn extracts_project_from_cwd() {
    let line = r#"{"type":"assistant","timestamp":"2026-07-10T10:00:00Z","cwd":"/home/o/Projects/agent-bar","message":{"model":"claude-fable-5","usage":{"input_tokens":1,"output_tokens":1}}}"#;
    let recs = parse_claude_lines([line].into_iter());
    assert_eq!(recs[0].project.as_deref(), Some("agent-bar"));
}

#[test]
fn missing_cwd_yields_none_project() {
    let recs = parse_claude_lines([LINE].into_iter());
    assert_eq!(recs[0].project, None);
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test usage`
Expected: erro de compilação — `UsageRecord` sem os campos (os testes novos não compilam)

- [ ] **Step 3: Implementation**

1. `src/usage/mod.rs` — struct:

```rust
pub struct UsageRecord {
    pub provider: String,
    pub model: Option<String>,
    pub input: u64,
    pub output: u64,
    pub cache_read: u64,
    pub cache_write: u64,
    pub ts: OffsetDateTime,
    /// File stem do session log de origem (arquivo = sessão). Preenchido
    /// por `records()`; parsers deixam `None`.
    pub session_id: Option<String>,
    /// Basename do `cwd` do log (só Claude). Preenchido pelo parser.
    pub project: Option<String>,
}
```

2. `records()` — anotar session_id após o cache (sempre, mesmo em cache hit,
para não depender do que foi cacheado):

```rust
    for path in collect_jsonl(opts.claude_dir) {
        let mut recs = cache.cached_or_parse(&path, |content| parse_claude_lines(content.lines()));
        let stem = path.file_stem().and_then(|s| s.to_str()).map(str::to_string);
        for r in &mut recs {
            r.session_id = stem.clone();
        }
        all.extend(recs);
    }
```

(idem no loop do codex.)

3. `src/usage/claude.rs` — capturar `cwd` no topo do loop e usar no push:

```rust
        let project = v
            .get("cwd")
            .and_then(Value::as_str)
            .and_then(|c| std::path::Path::new(c).file_name())
            .and_then(|b| b.to_str())
            .map(str::to_string);
```

e no `out.push(UsageRecord { …, session_id: None, project, … })`.

4. `src/usage/codex.rs` — push ganha `session_id: None, project: None`.

5. Construtores de teste: TODOS os literais `UsageRecord { … }` nos arquivos
   listados ganham `session_id: None, project: None` (compile errors guiam:
   `cargo build` lista os pontos — `buckets.rs` fns `rec()`, `cache.rs`
   `dummy_record()`, `pricing.rs` `rec()`, `mod.rs` teste
   `aggregate_records_sums_tokens_and_cost_directly`). Fora de `src/usage/`,
   `rg -n "UsageRecord \{" src tests` e completar os que faltarem (há
   construtores em `src/tui/render/dashboard.rs` testes e possivelmente em
   `history.rs`/`event_loop.rs` — o compilador é a lista canônica).

- [ ] **Step 4: Run tests**

Run: `cargo test usage`
Expected: PASS

Run: `cargo build --all-targets`
Expected: compila sem erro (nenhum literal esquecido)

- [ ] **Step 5: Commit**

```bash
git add -A src
git commit -m "feat: session_id e project no UsageRecord"
```

---

### Task 4: `bucket_by_model_hour`

**Files:**
- Modify: `src/usage/buckets.rs`

**Interfaces:**
- Consumes: `series_slot_for_model`, `display_model_name` (T1).
- Produces:

```rust
pub struct ModelHourSeries {
    /// Nome já tratado pra display (ex. "Fable 5").
    pub label: String,
    /// Slot de cor 0..=5 (theme::ColorToken::Series1..6).
    pub slot: u8,
    /// Exatamente `hours` pontos, mais antigo → mais novo.
    pub tokens: Vec<u64>,
    pub total: u64,
}

pub fn bucket_by_model_hour(
    records: &[UsageRecord],
    provider: &str,
    now: OffsetDateTime,
    hours: usize,
) -> Vec<ModelHourSeries>
```

Ordenação: slot asc, depois total desc (estável pros snapshots). Modelos com
mesmo display name agregam juntos. `model=None` → label "—", slot 5.
Séries com total 0 são omitidas. Consumido por T8/T9/T10.

- [ ] **Step 1: Write the failing tests**

No `mod tests` de buckets.rs (usar o helper `rec` existente, com um wrapper
que seta modelo):

```rust
fn mrec(provider: &str, model: &str, ts: time::OffsetDateTime, tokens: u64) -> UsageRecord {
    let mut r = rec(provider, ts, tokens);
    r.model = Some(model.into());
    r
}

#[test]
fn model_hour_series_split_by_model_and_slot_order() {
    let now = datetime!(2026-07-10 12:30:00 UTC);
    let records = vec![
        mrec("claude", "claude-opus-4-8", datetime!(2026-07-10 12:05:00 UTC), 50),
        mrec("claude", "claude-fable-5", datetime!(2026-07-10 12:10:00 UTC), 100),
        mrec("claude", "claude-fable-5", datetime!(2026-07-10 11:10:00 UTC), 30),
        mrec("codex", "gpt-5.5", datetime!(2026-07-10 12:00:00 UTC), 999), // outro provider: fora
    ];
    let series = bucket_by_model_hour(&records, "claude", now, 3);
    assert_eq!(series.len(), 2);
    // Slot 0 (fable) vem antes do slot 1 (opus).
    assert_eq!(series[0].label, "Fable 5");
    assert_eq!(series[0].slot, 0);
    assert_eq!(series[0].tokens, vec![0, 30, 100]);
    assert_eq!(series[0].total, 130);
    assert_eq!(series[1].label, "Opus 4.8");
    assert_eq!(series[1].tokens, vec![0, 0, 50]);
}

#[test]
fn model_hour_series_merges_same_display_name_and_skips_empty() {
    let now = datetime!(2026-07-10 12:30:00 UTC);
    let records = vec![
        mrec("claude", "claude-opus-4-8", datetime!(2026-07-10 12:05:00 UTC), 10),
        mrec("claude", "claude-opus-4-8-20260101", datetime!(2026-07-10 12:06:00 UTC), 5),
        mrec("claude", "claude-haiku-4-5", datetime!(2026-07-01 12:00:00 UTC), 7), // fora da janela
    ];
    let series = bucket_by_model_hour(&records, "claude", now, 2);
    assert_eq!(series.len(), 1); // haiku fora da janela → total 0 → omitido
    assert_eq!(series[0].label, "Opus 4.8");
    assert_eq!(series[0].total, 15);
}
```

- [ ] **Step 2: Run to verify fail**

Run: `cargo test usage::buckets`
Expected: erro de compilação (fn/struct inexistentes)

- [ ] **Step 3: Implementation**

Em buckets.rs (após `provider_series_24h`; reusa `floor_to_hour`/`record_tokens`):

```rust
use crate::usage::model_names::{display_model_name, series_slot_for_model};

#[derive(Debug, Clone, PartialEq)]
pub struct ModelHourSeries {
    pub label: String,
    pub slot: u8,
    pub tokens: Vec<u64>,
    pub total: u64,
}

/// Séries tokens/h por modelo (display name) de um provider, `hours` buckets
/// terminando na hora de `now`. Ordenadas por slot asc, total desc.
pub fn bucket_by_model_hour(
    records: &[UsageRecord],
    provider: &str,
    now: OffsetDateTime,
    hours: usize,
) -> Vec<ModelHourSeries> {
    let end_hour = floor_to_hour(now);
    let start = end_hour - time::Duration::hours(hours.saturating_sub(1) as i64);

    // label → (slot, tokens por bucket)
    let mut map: BTreeMap<String, (u8, Vec<u64>)> = BTreeMap::new();
    for r in records {
        if r.provider != provider || r.ts < start || r.ts >= end_hour + time::Duration::hours(1) {
            continue;
        }
        let raw = r.model.as_deref().unwrap_or("—");
        let label = if raw == "—" { "—".to_string() } else { display_model_name(raw) };
        let slot = if raw == "—" { 5 } else { series_slot_for_model(raw) };
        let idx = (floor_to_hour(r.ts) - start).whole_hours() as usize;
        let entry = map.entry(label).or_insert_with(|| (slot, vec![0; hours]));
        if let Some(b) = entry.1.get_mut(idx) {
            *b += record_tokens(r);
        }
    }

    let mut out: Vec<ModelHourSeries> = map
        .into_iter()
        .map(|(label, (slot, tokens))| {
            let total = tokens.iter().sum();
            ModelHourSeries { label, slot, tokens, total }
        })
        .filter(|s| s.total > 0)
        .collect();
    out.sort_by(|a, b| a.slot.cmp(&b.slot).then(b.total.cmp(&a.total)));
    out
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test usage::buckets`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/usage/buckets.rs
git commit -m "feat: buckets tokens/h por modelo"
```

---

### Task 5: `sessions_by_day`

**Files:**
- Modify: `src/usage/buckets.rs`

**Interfaces:**
- Consumes: `UsageRecord.session_id/project` (T3), `cost_usd_of` (T2),
  `display_model_name` (T1).
- Produces:

```rust
pub struct SessionAgg {
    pub session_id: String,
    pub start: OffsetDateTime,       // menor ts da sessão NO dia
    pub project: Option<String>,
    /// Display name do modelo com mais tokens na sessão (ex. "Fable 5").
    pub dominant_model: Option<String>,
    pub tokens: u64,                 // input+output+cache_read+cache_write
    pub cost_usd: Option<f64>,
}

pub struct DaySessions {
    pub date: Date,
    pub tokens: u64,
    pub cost_usd: Option<f64>,
    pub sessions: Vec<SessionAgg>,   // desc por start
}

pub fn sessions_by_day(records: &[UsageRecord], offset: time::UtcOffset) -> Vec<DaySessions>
```

Dia = `r.ts.to_offset(offset).date()` (dia LOCAL — o usuário pediu "por dia"
no fuso dele). Sessão que cruza meia-noite conta em cada dia com os records
daquele dia. `session_id=None` → agrupa em session_id "—". Retorno desc por
data. Consumido por T10.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn sessions_by_day_groups_and_orders_desc() {
    use time::UtcOffset;
    let mut a = mrec("claude", "claude-fable-5", datetime!(2026-07-10 10:00:00 UTC), 100);
    a.session_id = Some("s1".into());
    a.project = Some("crm".into());
    let mut b = mrec("claude", "claude-opus-4-8", datetime!(2026-07-10 11:00:00 UTC), 40);
    b.session_id = Some("s2".into());
    let mut c = mrec("claude", "claude-fable-5", datetime!(2026-07-09 09:00:00 UTC), 7);
    c.session_id = Some("s3".into());

    let days = sessions_by_day(&[a, b, c], UtcOffset::UTC);
    assert_eq!(days.len(), 2);
    assert_eq!(days[0].date, time::macros::date!(2026 - 07 - 10)); // desc
    assert_eq!(days[0].sessions.len(), 2);
    // sessões desc por start: s2 (11:00) antes de s1 (10:00)
    assert_eq!(days[0].sessions[0].session_id, "s2");
    assert_eq!(days[0].sessions[1].project.as_deref(), Some("crm"));
    assert_eq!(days[0].tokens, 140);
    assert!(days[0].cost_usd.is_some());
}

#[test]
fn session_dominant_model_is_treated_name_of_biggest() {
    use time::UtcOffset;
    let mut a = mrec("claude", "claude-fable-5", datetime!(2026-07-10 10:00:00 UTC), 100);
    a.session_id = Some("s1".into());
    let mut b = mrec("claude", "claude-opus-4-8", datetime!(2026-07-10 10:05:00 UTC), 30);
    b.session_id = Some("s1".into());
    let days = sessions_by_day(&[a, b], UtcOffset::UTC);
    assert_eq!(days[0].sessions[0].dominant_model.as_deref(), Some("Fable 5"));
    assert_eq!(days[0].sessions[0].tokens, 130);
}

#[test]
fn sessions_by_day_uses_local_offset_for_date() {
    // 2026-07-10 01:00 UTC = 2026-07-09 22:00 em UTC-3.
    let offset = time::UtcOffset::from_hms(-3, 0, 0).unwrap();
    let mut a = mrec("claude", "claude-fable-5", datetime!(2026-07-10 01:00:00 UTC), 10);
    a.session_id = Some("s1".into());
    let days = sessions_by_day(&[a], offset);
    assert_eq!(days[0].date, time::macros::date!(2026 - 07 - 09));
}
```

- [ ] **Step 2: Run to verify fail**

Run: `cargo test usage::buckets`
Expected: erro de compilação

- [ ] **Step 3: Implementation**

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct SessionAgg {
    pub session_id: String,
    pub start: OffsetDateTime,
    pub project: Option<String>,
    pub dominant_model: Option<String>,
    pub tokens: u64,
    pub cost_usd: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DaySessions {
    pub date: Date,
    pub tokens: u64,
    pub cost_usd: Option<f64>,
    pub sessions: Vec<SessionAgg>,
}

/// Sessões agrupadas por dia LOCAL (desc). Sessão = session_id do record;
/// records sem session_id agrupam em "—".
pub fn sessions_by_day(records: &[UsageRecord], offset: time::UtcOffset) -> Vec<DaySessions> {
    struct SessAcc {
        start: OffsetDateTime,
        project: Option<String>,
        by_model: BTreeMap<String, u64>,
        tokens: u64,
        cost_usd: Option<f64>,
    }
    // (date, session_id) → acumulador
    let mut map: BTreeMap<(Date, String), SessAcc> = BTreeMap::new();

    for r in records {
        let local = r.ts.to_offset(offset);
        let key = (local.date(), r.session_id.clone().unwrap_or_else(|| "—".into()));
        let acc = map.entry(key).or_insert(SessAcc {
            start: r.ts,
            project: None,
            by_model: BTreeMap::new(),
            tokens: 0,
            cost_usd: None,
        });
        acc.start = acc.start.min(r.ts);
        if acc.project.is_none() {
            acc.project = r.project.clone();
        }
        let t = record_tokens(r);
        acc.tokens += t;
        if let Some(m) = &r.model {
            *acc.by_model.entry(m.clone()).or_insert(0) += t;
        }
        match (cost_usd_of(r), acc.cost_usd.as_mut()) {
            (Some(c), Some(sum)) => *sum += c,
            (Some(c), None) => acc.cost_usd = Some(c),
            (None, _) => {}
        }
    }

    let mut by_day: BTreeMap<Date, Vec<SessionAgg>> = BTreeMap::new();
    for ((date, session_id), acc) in map {
        let dominant_model = acc
            .by_model
            .iter()
            .max_by_key(|(_, t)| **t)
            .map(|(m, _)| crate::usage::model_names::display_model_name(m));
        by_day.entry(date).or_default().push(SessionAgg {
            session_id,
            start: acc.start,
            project: acc.project,
            dominant_model,
            tokens: acc.tokens,
            cost_usd: acc.cost_usd,
        });
    }

    let mut out: Vec<DaySessions> = by_day
        .into_iter()
        .map(|(date, mut sessions)| {
            sessions.sort_by(|a, b| b.start.cmp(&a.start));
            let tokens = sessions.iter().map(|s| s.tokens).sum();
            let cost_usd = sessions.iter().filter_map(|s| s.cost_usd).fold(None, |acc, c| {
                Some(acc.unwrap_or(0.0) + c)
            });
            DaySessions { date, tokens, cost_usd, sessions }
        })
        .collect();
    out.reverse(); // BTreeMap é asc; queremos desc por data
    out
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test usage::buckets`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/usage/buckets.rs
git commit -m "feat: agregação de sessões por dia"
```

---

### Task 6: Tokens de tema (séries, acento, literais promovidos)

**Files:**
- Modify: `src/theme.rs`
- Modify: `src/tui/theme_bridge.rs`
- Modify: `src/tui/render/mod.rs:164` (bg literal → token)
- Modify: `src/tui/render/login.rs:47`, `src/tui/render/config.rs:52` (sel bg literal → token)

**Interfaces:**
- Produces: `ColorToken::{Series1, Series2, Series3, Series4, Series5, Series6, Bg}`.
  Acento de UI = `ColorToken::Blue` existente (#61afef) — documentar no enum.
  `pub fn series_token(slot: u8) -> ColorToken` em theme.rs.
  `theme_bridge::provider_color` passa a delegar `hex_to_color(provider_hex(id))`.
- Séries (validadas p/ CVD/contraste sobre #282c34):
  Series1 `#3f8fd6` · Series2 `#cb7e30` · Series3 `#b562d6` · Series4 `#55a34a` · Series5 `#2ba3b4` · Series6 `#af8f2c` · Bg `#282c34`.

- [ ] **Step 1: Write the failing tests**

Em `src/theme.rs` `mod tests`:

```rust
#[test]
fn series_tokens_hex() {
    assert_eq!(ColorToken::Series1.hex(), "#3f8fd6");
    assert_eq!(ColorToken::Series2.hex(), "#cb7e30");
    assert_eq!(ColorToken::Series3.hex(), "#b562d6");
    assert_eq!(ColorToken::Series4.hex(), "#55a34a");
    assert_eq!(ColorToken::Series5.hex(), "#2ba3b4");
    assert_eq!(ColorToken::Series6.hex(), "#af8f2c");
    assert_eq!(ColorToken::Bg.hex(), "#282c34");
}

#[test]
fn series_token_maps_slots() {
    assert_eq!(series_token(0), ColorToken::Series1);
    assert_eq!(series_token(5), ColorToken::Series6);
    assert_eq!(series_token(99), ColorToken::Series6); // clamp
}
```

- [ ] **Step 2: Run to verify fail**

Run: `cargo test theme`
Expected: erro de compilação (variantes inexistentes)

- [ ] **Step 3: Implementation**

theme.rs — adicionar variantes ao enum + hex() + fn:

```rust
    // séries de gráfico por modelo (One Dark Turbo, validadas p/ CVD/contraste)
    Series1, // fable/mythos
    Series2, // opus
    Series3, // sonnet
    Series4, // haiku
    Series5, // codex/gpt
    Series6, // outros
    /// Fundo geral da TUI (#282c34 — antes literal em render/mod.rs).
    Bg,
```

```rust
            ColorToken::Series1 => "#3f8fd6",
            ColorToken::Series2 => "#cb7e30",
            ColorToken::Series3 => "#b562d6",
            ColorToken::Series4 => "#55a34a",
            ColorToken::Series5 => "#2ba3b4",
            ColorToken::Series6 => "#af8f2c",
            ColorToken::Bg => "#282c34",
```

```rust
/// Token de série de gráfico pelo slot de família (usage::model_names).
pub fn series_token(slot: u8) -> ColorToken {
    match slot {
        0 => ColorToken::Series1,
        1 => ColorToken::Series2,
        2 => ColorToken::Series3,
        3 => ColorToken::Series4,
        4 => ColorToken::Series5,
        _ => ColorToken::Series6,
    }
}
```

theme_bridge.rs — `provider_color` vira delegação (remove o match duplicado):

```rust
pub fn provider_color(id: &str) -> Color {
    hex_to_color(crate::theme::provider_hex(id))
}
```

render/mod.rs:164 — `Color::Rgb(0x28, 0x2c, 0x34)` → `to_ratatui(ColorToken::Bg)`.
login.rs:47 e config.rs:52 — `Color::Rgb(45, 53, 65)` → `to_ratatui(ColorToken::SelBg)`
(consolidação intencional; snapshots afetados atualizam nesta task).

- [ ] **Step 4: Run tests**

Run: `cargo test theme`
Expected: PASS

Run: `cargo test tui`
Expected: snapshots de login/config podem divergir pela cor de seleção — revisar com `cargo insta review` (mudança intencional) e re-rodar até PASS

- [ ] **Step 5: Commit**

```bash
git add -A src
git commit -m "feat: tokens de série e acento One Dark Turbo"
```

---

### Task 7: Gauge sólido com precisão ⅛ (sem gradiente, sem pulso)

**Files:**
- Modify: `src/tui/widgets/quota_gauge.rs` (reescrita)
- Modify: `src/tui/render/detail.rs` (call sites de `pulse_color` — remover a
  modulação; manter o resto do layout desta task intocado)

**Interfaces:**
- Produces: `gauge_spans(remaining_pct: f64, width: usize, color: Color) -> Vec<Span<'static>>`
  (assinatura idêntica; fill sólido `█` + célula parcial `▏▎▍▌▋▊▉` + trilho `░`
  em EmptyTrack). **Removidos**: `lerp_rgb` (privada), `dimmed`, `pulse_color`,
  `scale_rgb`. Consumido por T9/T10 e call sites existentes.

- [ ] **Step 1: Write the failing tests**

Substituir o `mod tests` de quota_gauge.rs por:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn text(spans: &[Span<'_>]) -> String {
        spans.iter().map(|s| s.content.as_ref()).collect()
    }

    #[test]
    fn gauge_total_width_is_exact() {
        let spans = gauge_spans(50.0, 10, ratatui::style::Color::Green);
        let total: usize = spans.iter().map(|s| s.content.chars().count()).sum();
        assert_eq!(total, 10);
    }

    #[test]
    fn gauge_uses_single_solid_color_no_gradient() {
        let spans = gauge_spans(80.0, 10, ratatui::style::Color::Green);
        let fills: Vec<_> = spans
            .iter()
            .filter(|s| s.content.contains('█') || "▏▎▍▌▋▊▉".chars().any(|c| s.content.contains(c)))
            .collect();
        // Fill inteiro em NO MÁXIMO 2 spans (cheio + parcial), mesma cor.
        assert!(fills.len() <= 2, "fill deve ser sólido, não célula-a-célula: {} spans", fills.len());
        let colors: std::collections::HashSet<_> =
            fills.iter().map(|s| format!("{:?}", s.style.fg)).collect();
        assert_eq!(colors.len(), 1);
    }

    #[test]
    fn gauge_has_eighth_precision() {
        // 64% de 24 células = 15.36 células → 15 cheias + parcial de 3/8 (▍).
        let spans = gauge_spans(64.0, 24, ratatui::style::Color::Green);
        let t = text(&spans);
        assert_eq!(t.chars().filter(|c| *c == '█').count(), 15);
        assert!(t.contains('▍'), "esperava célula parcial ▍ em: {t}");
    }

    #[test]
    fn gauge_zero_and_full() {
        let z = gauge_spans(0.0, 8, ratatui::style::Color::Red);
        assert_eq!(text(&z), "░".repeat(8));
        let f = gauge_spans(100.0, 8, ratatui::style::Color::Green);
        assert_eq!(text(&f), "█".repeat(8));
    }
}
```

- [ ] **Step 2: Run to verify fail**

Run: `cargo test quota_gauge`
Expected: FAIL (`gauge_uses_single_solid_color_no_gradient` e `gauge_has_eighth_precision` falham; trilho ainda é `▒`)

- [ ] **Step 3: Implementation**

Reescrever quota_gauge.rs:

```rust
//! Gauge sólido: fill na cor plena com precisão de ⅛ de célula + trilho.
//! (v8: gradiente lerp e pulso de brilho removidos de propósito — spec §6.)

use ratatui::style::{Color, Style};
use ratatui::text::Span;

use crate::theme::ColorToken;
use crate::tui::theme_bridge::to_ratatui;

/// Oitavos de célula, do vazio (índice 0 = nada) ao quase-cheio (7 = ▉).
const EIGHTHS: [&str; 8] = ["", "▏", "▎", "▍", "▌", "▋", "▊", "▉"];

/// Barra de quota: `remaining_pct` em 0..=100, `width` células.
/// Fill = `█` sólido + célula parcial de ⅛; trilho = `░` em EmptyTrack.
/// Total de células == width, sempre.
pub fn gauge_spans(remaining_pct: f64, width: usize, color: Color) -> Vec<Span<'static>> {
    let pct = remaining_pct.clamp(0.0, 100.0);
    let eighths = ((width as f64) * pct / 100.0 * 8.0).round() as usize;
    let full = (eighths / 8).min(width);
    let partial = if full < width { eighths % 8 } else { 0 };

    let mut spans = Vec::with_capacity(3);
    let mut used = 0;
    if full > 0 {
        spans.push(Span::styled("█".repeat(full), Style::default().fg(color)));
        used += full;
    }
    if partial > 0 && used < width {
        spans.push(Span::styled(EIGHTHS[partial].to_string(), Style::default().fg(color)));
        used += 1;
    }
    if used < width {
        spans.push(Span::styled(
            "░".repeat(width - used),
            Style::default().fg(to_ratatui(ColorToken::EmptyTrack)),
        ));
    }
    spans
}
```

Call sites do pulso: `rg -n "pulse_color|scale_rgb" src/` e remover a
modulação em `src/tui/render/detail.rs` (window_line usa a cor de severidade
direto). `src/tui/render/dashboard.rs` também referencia gauge/pulse — ajuste
mínimo pra compilar (o arquivo morre na T11; não polir).

- [ ] **Step 4: Run tests**

Run: `cargo test quota_gauge`
Expected: PASS

Run: `cargo test tui`
Expected: snapshots com gauges divergem (▒→░, sem gradiente) — `cargo insta review`, aceitar, re-rodar até PASS

- [ ] **Step 5: Commit**

```bash
git add -A src
git commit -m "feat: gauge sólido com precisão de oitavos"
```

---

### Task 8: Widget de chart de colunas empilhadas (`column_chart.rs`)

**Files:**
- Create: `src/tui/widgets/column_chart.rs`
- Modify: `src/tui/widgets/mod.rs` (declarar módulo)

**Interfaces:**
- Consumes: `ModelHourSeries` (T4), `series_token` (T6), `to_ratatui`.
- Produces:

```rust
/// Linhas prontas do chart (altura total = `height`: plot + eixo X + labels + legenda).
/// `height` mínimo útil: 6 (3 de plot). `width` = células disponíveis.
pub fn column_chart_lines(
    series: &[crate::usage::buckets::ModelHourSeries],
    width: u16,
    height: u16,
    now: time::OffsetDateTime,
    local_offset: time::UtcOffset,
) -> Vec<ratatui::text::Line<'static>>
```

Contrato visual (spec §2): escala √ (rótulos Y calculados), colunas = 1 bucket/h
(largura de coluna 2+1 gap se couber, senão 1+1, senão 1+0), stack por série na
ordem do slice (já vem slot-ordenada), série com uso >0 no bucket garante ≥1
oitavo, eixo X com horas locais a cada 3 buckets, última linha = legenda
`● Label total` por série. Séries vazias (slice vazio) → linha única
"sem uso de tokens no período" em Comment. Consumido por T9/T10.

- [ ] **Step 1: Write the failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::usage::buckets::ModelHourSeries;
    use time::macros::datetime;

    fn series(label: &str, slot: u8, tokens: Vec<u64>) -> ModelHourSeries {
        let total = tokens.iter().sum();
        ModelHourSeries { label: label.into(), slot, tokens, total }
    }

    fn plain(lines: &[ratatui::text::Line<'_>]) -> Vec<String> {
        lines
            .iter()
            .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect::<String>())
            .collect()
    }

    #[test]
    fn chart_has_requested_height_and_legend() {
        let s = vec![
            series("Fable 5", 0, vec![0, 10, 100, 50]),
            series("Opus 4.8", 1, vec![0, 5, 0, 0]),
        ];
        let lines = column_chart_lines(&s, 60, 10, datetime!(2026-07-10 12:00:00 UTC), time::UtcOffset::UTC);
        assert_eq!(lines.len(), 10);
        let text = plain(&lines);
        let legend = text.last().unwrap();
        assert!(legend.contains("Fable 5"), "legenda: {legend}");
        assert!(legend.contains("Opus 4.8"));
    }

    #[test]
    fn chart_nonzero_bucket_is_never_invisible() {
        // Fable enorme + Opus minúsculo no mesmo bucket: Opus ganha ≥1 oitavo.
        let s = vec![
            series("Fable 5", 0, vec![1_000_000]),
            series("Opus 4.8", 1, vec![1]),
        ];
        let lines = column_chart_lines(&s, 30, 8, datetime!(2026-07-10 12:00:00 UTC), time::UtcOffset::UTC);
        // Alguma célula do plot deve usar a cor do slot 1 (Series2).
        let series2 = crate::tui::theme_bridge::to_ratatui(crate::theme::series_token(1));
        let has_opus_cell = lines.iter().any(|l| {
            l.spans.iter().any(|sp| sp.style.fg == Some(series2) && !sp.content.trim().is_empty())
        });
        assert!(has_opus_cell, "série minúscula não pode sumir do chart");
    }

    #[test]
    fn chart_empty_series_shows_empty_state() {
        let lines = column_chart_lines(&[], 40, 8, datetime!(2026-07-10 12:00:00 UTC), time::UtcOffset::UTC);
        let text = plain(&lines).join("\n");
        assert!(text.contains("sem uso"), "estado vazio desenhado: {text}");
    }

    #[test]
    fn chart_lines_never_exceed_width() {
        let s = vec![series("Fable 5", 0, (0..24).map(|i| i * 1000).collect())];
        for w in [30u16, 60, 100] {
            let lines = column_chart_lines(&s, w, 9, datetime!(2026-07-10 12:00:00 UTC), time::UtcOffset::UTC);
            for l in plain(&lines) {
                assert!(l.chars().count() <= w as usize, "linha estourou {w}: {l:?}");
            }
        }
    }
}
```

- [ ] **Step 2: Run to verify fail**

Run: `cargo test column_chart`
Expected: erro de compilação (módulo inexistente)

- [ ] **Step 3: Implementation**

`src/tui/widgets/column_chart.rs` completo:

```rust
//! Chart de colunas horárias empilhadas por modelo (v8, spec §2).
//! Escala √ no eixo Y; uso >0 nunca fica invisível (≥1 oitavo).

use ratatui::style::Style;
use ratatui::text::{Line, Span};
use time::{OffsetDateTime, UtcOffset};

use crate::theme::{series_token, ColorToken};
use crate::tui::theme_bridge::to_ratatui;
use crate::usage::buckets::ModelHourSeries;

/// Blocos parciais por oitavos (índice 1..=7); célula cheia é `█`.
const EIGHTHS: [&str; 8] = ["", "▁", "▂", "▃", "▄", "▅", "▆", "▇"];
const Y_AXIS_W: usize = 6; // "999M ┤"

pub fn column_chart_lines(
    series: &[ModelHourSeries],
    width: u16,
    height: u16,
    now: OffsetDateTime,
    local_offset: UtcOffset,
) -> Vec<Line<'static>> {
    let width = width as usize;
    let height = height as usize;
    if series.is_empty() {
        let mut out = vec![Line::from(Span::styled(
            " sem uso de tokens no período".to_string(),
            Style::default().fg(to_ratatui(ColorToken::Comment)),
        ))];
        out.resize(height.max(1), Line::default());
        return out;
    }

    let buckets = series.iter().map(|s| s.tokens.len()).max().unwrap_or(0);
    // Reserva: 1 linha eixo X, 1 linha labels X, 1 linha legenda.
    let plot_rows = height.saturating_sub(3).max(3);

    // Largura de coluna: tenta 2+1 gap, senão 1+1, senão 1+0.
    let avail = width.saturating_sub(Y_AXIS_W);
    let (col_w, gap) = if buckets * 3 <= avail {
        (2usize, 1usize)
    } else if buckets * 2 <= avail {
        (1, 1)
    } else {
        (1, 0)
    };

    // Total por bucket e máximo (pra escala √).
    let totals: Vec<u64> = (0..buckets)
        .map(|i| series.iter().map(|s| s.tokens.get(i).copied().unwrap_or(0)).sum())
        .collect();
    let max_total = totals.iter().copied().max().unwrap_or(0).max(1);
    let cap = plot_rows * 8; // resolução em oitavos

    // Altura empilhada por (bucket, série), em oitavos.
    let mut stacks: Vec<Vec<(usize, u8)>> = Vec::with_capacity(buckets); // (altura, slot)
    for i in 0..buckets {
        let total = totals[i];
        let mut cols = Vec::new();
        if total > 0 {
            let h_total = (((total as f64 / max_total as f64).sqrt()) * cap as f64)
                .round()
                .max(1.0) as usize;
            let mut used = 0usize;
            let active: Vec<&ModelHourSeries> =
                series.iter().filter(|s| s.tokens.get(i).copied().unwrap_or(0) > 0).collect();
            for (k, s) in active.iter().enumerate() {
                let v = s.tokens.get(i).copied().unwrap_or(0);
                let mut h = if k + 1 == active.len() {
                    h_total.saturating_sub(used) // último leva o resto (soma exata)
                } else {
                    ((v as f64 / total as f64) * h_total as f64).round() as usize
                };
                h = h.max(1); // uso >0 nunca invisível
                cols.push((h, s.slot));
                used += h;
            }
        }
        stacks.push(cols);
    }

    let mut out: Vec<Line<'static>> = Vec::with_capacity(height);

    // Plot, de cima pra baixo.
    for row in (0..plot_rows).rev() {
        let lo = row * 8;
        let mut spans: Vec<Span<'static>> = Vec::new();
        // Rótulo Y: topo = max, meio = valor da escala √ na metade, base tratada no eixo.
        let label = if row + 1 == plot_rows {
            fmt_tokens_short(max_total)
        } else if row == plot_rows / 2 {
            // valor cuja √-fração corresponde à metade da altura
            fmt_tokens_short(((0.5f64 * 0.5) * max_total as f64) as u64)
        } else {
            String::new()
        };
        let axis = if label.is_empty() { format!("{:>5}│", "") } else { format!("{label:>5}┤") };
        spans.push(Span::styled(axis, Style::default().fg(to_ratatui(ColorToken::Comment))));

        for cols in &stacks {
            // Descobre o que esta célula (linha `row`) mostra nesta coluna.
            let mut base = 0usize;
            let mut cell: Option<(String, u8)> = None;
            for (h, slot) in cols {
                let top = base + h;
                if top >= lo + 8 && base < lo + 8 && top > lo {
                    // célula coberta inteira por esta série (ou até acima)
                    if base <= lo {
                        cell = Some(("█".to_string(), *slot));
                        break;
                    }
                }
                if top > lo && top < lo + 8 && base <= lo {
                    cell = Some((EIGHTHS[top - lo].to_string(), *slot));
                    break;
                }
                base = top;
            }
            let (glyph, style) = match cell {
                Some((g, slot)) => (g, Style::default().fg(to_ratatui(series_token(slot)))),
                None => (" ".to_string(), Style::default()),
            };
            spans.push(Span::styled(glyph.repeat(col_w), style));
            if gap > 0 {
                spans.push(Span::raw(" ".repeat(gap)));
            }
        }
        out.push(Line::from(spans));
    }

    // Eixo X.
    let plot_w = buckets * (col_w + gap);
    out.push(Line::from(Span::styled(
        format!("{:>5}┴{}", "0", "─".repeat(plot_w.min(width.saturating_sub(Y_AXIS_W)))),
        Style::default().fg(to_ratatui(ColorToken::Comment)),
    )));

    // Labels de hora (a cada 3 buckets).
    let mut xl = String::with_capacity(width);
    xl.push_str(&" ".repeat(Y_AXIS_W));
    for i in 0..buckets {
        if i % 3 == 0 {
            let bucket_time = now - time::Duration::hours((buckets - 1 - i) as i64);
            let h = bucket_time.to_offset(local_offset).hour();
            let lab = format!("{h:02}h");
            let pos = Y_AXIS_W + i * (col_w + gap);
            while xl.chars().count() < pos {
                xl.push(' ');
            }
            if xl.chars().count() + lab.len() <= width {
                xl.push_str(&lab);
            }
        }
    }
    out.push(Line::from(Span::styled(
        xl,
        Style::default().fg(to_ratatui(ColorToken::Comment)),
    )));

    // Legenda.
    let mut legend: Vec<Span<'static>> = Vec::new();
    for s in series {
        legend.push(Span::styled(
            "  ● ".to_string(),
            Style::default().fg(to_ratatui(series_token(s.slot))),
        ));
        legend.push(Span::styled(
            format!("{} {}", s.label, fmt_tokens_short(s.total)),
            Style::default().fg(to_ratatui(ColorToken::Text)),
        ));
    }
    out.push(Line::from(legend));

    // Garante exatamente `height` linhas (corta plot excedente já evitado acima).
    out.truncate(height);
    while out.len() < height {
        out.push(Line::default());
    }
    out
}

/// "264,7M", "1,5B", "980k", "42" — formato curto pt-BR (vírgula decimal).
pub fn fmt_tokens_short(t: u64) -> String {
    let f = t as f64;
    if f >= 1e9 {
        format!("{:.1}B", f / 1e9).replace('.', ",")
    } else if f >= 1e6 {
        format!("{:.1}M", f / 1e6).replace('.', ",")
    } else if f >= 1e3 {
        format!("{:.0}k", f / 1e3)
    } else {
        format!("{t}")
    }
}
```

Declarar em `src/tui/widgets/mod.rs`: `pub mod column_chart;`

**Nota pro implementer:** o algoritmo de célula acima é a referência de
contrato (empilha de baixo pra cima; célula parcial usa `EIGHTHS[top-lo]` na
cor da série que termina ali). Se encontrar edge cases nos testes (p.ex.
célula onde uma série termina e outra começa no meio), a regra é: a série que
OCUPA a maior parte da célula pinta a célula. Ajuste a implementação, nunca o
contrato dos testes.

- [ ] **Step 4: Run tests**

Run: `cargo test column_chart`
Expected: PASS (4 testes)

- [ ] **Step 5: Snapshot de referência**

Adicionar teste de snapshot insta no mesmo arquivo:

```rust
    #[test]
    fn chart_snapshot_two_series() {
        let s = vec![
            series("Fable 5", 0, vec![0, 0, 17, 46, 75, 19, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 8, 140]),
            series("Opus 4.8", 1, vec![0, 0, 9, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
        ];
        let lines = column_chart_lines(&s, 84, 12, datetime!(2026-07-10 12:00:00 UTC), time::UtcOffset::UTC);
        let text: Vec<String> = plain(&lines);
        insta::assert_snapshot!(text.join("\n"));
    }
```

Run: `cargo test column_chart`
Expected: PASS (snapshot novo criado; inspecionar visualmente o .snap — colunas devem crescer, Opus visível no bucket 2)

- [ ] **Step 6: Commit**

```bash
git add -A src
git commit -m "feat: widget de chart de colunas por modelo"
```

---

### Task 9: Detail — layout por seção + chart grande + custos

**Files:**
- Modify: `src/tui/render/detail.rs` (reescrita do `render_full` e remoção de
  `spark_line`/`FIXED_GAUGE_W`)
- Test: snapshots em `src/tui/render/snapshots/` (regenerados de propósito)

**Interfaces:**
- Consumes: `column_chart_lines` (T8), `bucket_by_model_hour` (T4),
  `display_model_name` (T1), `gauge_spans` (T7), `series_token` (T6).
- Produces: mesma entrada pública `render_detail(state, frame, area, hits)`.

**Contrato de layout (spec §2)** — dentro do painel do provider:

```
Layout::vertical([
    Constraint::Length(janelas + 2),      // JANELAS (título + linhas)
    Constraint::Min(9),                   // GRÁFICO (título + chart ≥8) — absorve altura extra
    Constraint::Length(modelos + 2),      // MODELOS HOJE (título + header + linhas)
    Constraint::Length(extra),            // EXTRA USAGE (0 se ausente)
    Constraint::Length(1),                // TOTAIS
    Constraint::Length(1),                // chips
])
```

`Min` do gráfico é o que estica — o espaço vazio morre aqui. Se
`area.height` < soma dos mínimos: EXTRA USAGE some primeiro, depois MODELOS
HOJE colapsa pra 1 linha-resumo `N modelos · $X.XX`, e o título ganha `…`.

- [ ] **Step 1: Estudar o arquivo atual**

Read `src/tui/render/detail.rs` INTEIRO antes de editar. Mapear: `render_detail`
(entry, :603), `render_full` (:471), `window_line`/`model_window_line`,
`model_usage_line` (:199), `spark_line` (:275), `totals_line`, `extra_usage_line`,
`find_model_usage` (:84), constantes `LABEL_W`/`FIXED_GAUGE_W` (:36-40).

- [ ] **Step 2: Write the failing snapshot tests**

Substituir/adicionar no `mod tests` do detail.rs (aproveitar os fixtures
existentes de `detail_claude_full`; criar fixture com usage de 2 modelos):

```rust
#[test]
fn detail_chart_absorbs_extra_height_no_blank_gap() {
    // 100x40: antes do v8 sobravam ~20 linhas em branco. Agora o chart estica.
    let (state, _) = fixture_claude_full(); // fixture existente do arquivo
    let mut term = ratatui::Terminal::new(ratatui::backend::TestBackend::new(100, 40)).unwrap();
    term.draw(|f| {
            let mut hits = crate::tui::mouse::HitMap::default();
            super::render_detail(&state, f, f.area(), &mut hits);
        })
        .unwrap();
    let buf = term.backend().buffer().clone();
    // Nenhuma sequência de 5+ linhas totalmente vazias entre o título e os chips.
    let blank_run = max_blank_run(&buf);
    assert!(blank_run < 5, "gap de {blank_run} linhas em branco — chart deveria absorver");
    insta::assert_snapshot!(buffer_to_string(&buf));
}

#[test]
fn detail_models_today_shows_treated_names_and_cost() {
    let (state, _) = fixture_claude_full();
    let mut term = ratatui::Terminal::new(ratatui::backend::TestBackend::new(100, 32)).unwrap();
    term.draw(|f| {
            let mut hits = crate::tui::mouse::HitMap::default();
            super::render_detail(&state, f, f.area(), &mut hits);
        })
        .unwrap();
    let text = buffer_to_string(term.backend().buffer());
    assert!(text.contains("Opus 4.8"), "nome tratado ausente:\n{text}");
    assert!(!text.contains("claude-opus"), "id raw vazou:\n{text}");
}
```

Helpers `buffer_to_string`/`max_blank_run` (adicionar no mod tests):

```rust
fn buffer_to_string(buf: &ratatui::buffer::Buffer) -> String {
    let area = buf.area();
    (0..area.height)
        .map(|y| {
            (0..area.width)
                .map(|x| buf[(x, y)].symbol().to_string())
                .collect::<String>()
                .trim_end()
                .to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn max_blank_run(buf: &ratatui::buffer::Buffer) -> usize {
    let s = buffer_to_string(buf);
    let mut max = 0;
    let mut cur = 0;
    // Ignora a 1ª e última linha (bordas).
    let lines: Vec<&str> = s.lines().collect();
    for l in &lines[1..lines.len().saturating_sub(1)] {
        // linha "vazia" = só borda ┃ e espaços
        if l.trim_matches(|c: char| c == ' ' || c == '┃' || c == '│').is_empty() {
            cur += 1;
            max = max.max(cur);
        } else {
            cur = 0;
        }
    }
    max
}
```

**Nota:** adaptar `fixture_claude_full` ao nome real do helper de fixture do
arquivo (Read do Step 1 revela). O fixture precisa de `state.usage` com
`by_model` de 2 modelos (`claude-fable-5`, `claude-opus-4-8`) e
`state.history` com records nas últimas 24h pros buckets.

- [ ] **Step 3: Run to verify fail**

Run: `cargo test detail`
Expected: FAIL (`blank_run` ≥ 5 no layout atual; "claude-opus" raw presente)

- [ ] **Step 4: Implementation**

Reescrever `render_full` como orquestrador de seções:

```rust
fn render_full(
    state: &AppState,
    frame: &mut Frame,
    inner: Rect, // área interna do painel (sem borda)
    q: &ProviderQuota,
) {
    use ratatui::layout::{Constraint, Layout};

    let windows = window_lines(state, q, inner.width);          // Vec<Line> (JANELAS, com título)
    let (models, models_collapsed) = model_lines(state, q, inner.width); // MODELOS HOJE
    let extra = extra_lines(state, q, inner.width);              // EXTRA USAGE (vazio se N/A)
    let totals = totals_line(state, q, inner.width);             // 1 Line

    let fixed = windows.len() as u16 + models.len() as u16 + extra.len() as u16 + 2; // +totais +chips
    let chart_min: u16 = 9; // título + 8 de chart

    // Colapso progressivo quando não cabe (spec §2).
    let (extra, models) = if inner.height < fixed + chart_min {
        let no_extra: Vec<Line> = Vec::new();
        if inner.height < fixed - extra.len() as u16 + chart_min {
            (no_extra, models_collapsed) // colapsa MODELOS HOJE pra 1 linha-resumo
        } else {
            (no_extra, models)
        }
    } else {
        (extra, models)
    };

    let chunks = Layout::vertical([
        Constraint::Length(windows.len() as u16),
        Constraint::Min(chart_min),
        Constraint::Length(models.len() as u16),
        Constraint::Length(extra.len() as u16),
        Constraint::Length(1),
    ])
    .split(inner);

    frame.render_widget(Paragraph::new(windows), chunks[0]);
    render_chart_section(state, frame, chunks[1], q);
    frame.render_widget(Paragraph::new(models), chunks[2]);
    if !extra.is_empty() {
        frame.render_widget(Paragraph::new(extra), chunks[3]);
    }
    frame.render_widget(Paragraph::new(vec![totals]), chunks[4]);
}

fn render_chart_section(state: &AppState, frame: &mut Frame, area: Rect, q: &ProviderQuota) {
    let mut lines = vec![section_title("TOKENS/HORA · 24H", "escala √", area.width)];
    let records = state.history.as_deref().unwrap_or(&[]);
    let now = state.last_update.unwrap_or(time::OffsetDateTime::UNIX_EPOCH);
    let series = crate::usage::buckets::bucket_by_model_hour(records, &q.provider, now, 24);
    lines.extend(crate::tui::widgets::column_chart::column_chart_lines(
        &series,
        area.width,
        area.height.saturating_sub(1),
        now,
        state.local_offset,
    ));
    frame.render_widget(Paragraph::new(lines), area);
}
```

Regras de detalhe (o implementer preenche `window_lines`/`model_lines`/
`extra_lines`/`section_title` reaproveitando o código atual de
`window_line`/`model_usage_line`/`extra_usage_line`/`totals_line`):

1. `model_usage_line`: label vira `display_model_name(&mu.model)` (truncado
   pra `LABEL_W` só se não couber); gauge com `derive_bar_width` no lugar de
   `FIXED_GAUGE_W`; coluna custo `$X.XX` ou `—` (de `mu.cost`). **Deletar a
   constante `FIXED_GAUGE_W`.**
2. `models_collapsed` = 1 linha `N modelos hoje · $X.XX  …` em Comment.
3. `spark_line` e `provider_series_24h` no detail morrem (o import de
   `provider_series_24h` sai do arquivo; a fn continua em buckets.rs pro
   que restar de uso — se `rg -n "provider_series_24h" src/` mostrar que
   ninguém mais usa após T10/T11, deletá-la na T11).
4. `section_title(left, right, width)`: Line com left em Comment bold e right
   alinhado à direita em Comment.
5. `now`: `state.last_update` é `Option<OffsetDateTime>` — quando None (boot),
   o chart recebe records vazios e desenha o empty state; não usar
   `OffsetDateTime::now_utc()` no render (snapshots determinísticos).

- [ ] **Step 5: Run tests + review snapshots**

Run: `cargo test detail`
Expected: os 2 testes novos PASS; snapshots antigos do detail divergem — `cargo insta review`, aceitar os novos frames (verificar visualmente: chart alto, nomes tratados, custo por modelo, sem gap)

Run: `cargo test formatters`
Expected: PASS (nada do terminal/waybar formatters mudou)

- [ ] **Step 6: Commit**

```bash
git add -A src
git commit -m "feat: detail com layout por seção e chart por modelo"
```

---

### Task 10: History — dias expandíveis com sessões

**Files:**
- Modify: `src/tui/render/history.rs` (tabela → lista de dias expandíveis;
  chart do topo → column_chart)
- Modify: `src/tui/state.rs` (novos campos), `src/tui/action.rs` (novas
  actions), `src/tui/update.rs` (teclas da tela History)
- Test: snapshots regenerados + unit tests de update

**Interfaces:**
- Consumes: `sessions_by_day` (T5), `column_chart_lines` (T8),
  `bucket_by_model_hour` (T4), `fmt_tokens_short` (T8).
- Produces (state):

```rust
// state.rs (AppState)
pub history_selected: usize,                        // índice na lista de dias
pub history_expanded: std::collections::BTreeSet<time::Date>,
```

```rust
// action.rs
HistoryUp,
HistoryDown,
HistoryToggleDay,
```

- [ ] **Step 1: Estudar os arquivos**

Read `src/tui/render/history.rs` INTEIRO + as regiões de `update.rs` que
tratam History (tecla `t` → `ToggleHistoryRange`, scroll) + `action.rs`.

- [ ] **Step 2: Write the failing tests**

Em `update.rs` `mod tests` (seguir o padrão dos testes existentes de update):

```rust
#[test]
fn history_keys_select_and_toggle_days() {
    let mut state = AppState::new();
    state.screen = Screen::History;
    state.history = Some(vec![]); // carregado
    // j/k mapeiam pra HistoryDown/Up na tela History
    let a = key_to_action(&state, KeyCode::Char('j'));
    assert_eq!(a, Some(Action::HistoryDown));
    let a = key_to_action(&state, KeyCode::Enter);
    assert_eq!(a, Some(Action::HistoryToggleDay));
}

#[test]
fn history_toggle_day_flips_expanded_set() {
    let mut state = AppState::new();
    state.screen = Screen::History;
    // 2 dias de records → sessions_by_day produz 2 dias
    state.history = Some(vec![
        test_record("claude", "claude-fable-5", "s1", "2026-07-10T10:00:00Z", 10),
        test_record("claude", "claude-fable-5", "s2", "2026-07-09T10:00:00Z", 10),
    ]);
    state.history_selected = 0;
    update(&mut state, Action::HistoryToggleDay);
    assert_eq!(state.history_expanded.len(), 1);
    update(&mut state, Action::HistoryToggleDay);
    assert!(state.history_expanded.is_empty());
}
```

(`key_to_action`/`test_record`: adaptar aos nomes reais do arquivo — o padrão
de teste de update já existe; `test_record` é um helper novo local que monta
`UsageRecord` com session_id.)

- [ ] **Step 3: Run to verify fail**

Run: `cargo test tui::update`
Expected: erro de compilação (actions/campos inexistentes)

- [ ] **Step 4: Implementation**

1. `state.rs`: adicionar os 2 campos (default `0`/`BTreeSet::new()` em
   `AppState::new`).
2. `action.rs`: 3 variantes novas.
3. `update.rs`:
   - Na interceptação de teclas da tela History (criar braço como o de Login):
     `j`/`Down` → `HistoryDown`, `k`/`Up` → `HistoryUp`, `Enter` →
     `HistoryToggleDay`, mantém `t`/`r`/`Esc`/`h`/`g`/`w`/`q`/`?`.
   - Handlers:

```rust
Action::HistoryDown => {
    let n_days = state
        .history
        .as_deref()
        .map(|r| crate::usage::buckets::sessions_by_day(r, state.local_offset).len())
        .unwrap_or(0);
    if n_days > 0 {
        state.history_selected = (state.history_selected + 1).min(n_days - 1);
    }
}
Action::HistoryUp => {
    state.history_selected = state.history_selected.saturating_sub(1);
}
Action::HistoryToggleDay => {
    if let Some(records) = state.history.as_deref() {
        let days = crate::usage::buckets::sessions_by_day(records, state.local_offset);
        if let Some(day) = days.get(state.history_selected) {
            if !state.history_expanded.remove(&day.date) {
                state.history_expanded.insert(day.date);
            }
        }
    }
}
```

   (Se recomputar `sessions_by_day` por tecla pesar no futuro, cachear no
   state — YAGNI por ora: 7 dias de records é pequeno.)

4. `render_history` (reescrita da metade de baixo):
   - Chart do topo: substituir o Chart braille + CHART_PROVIDERS pelo
     `column_chart_lines` com `bucket_by_model_hour` agregando TODOS os
     providers com records (concatenar séries de claude e codex — chamar a fn
     uma vez por provider presente e juntar os Vec). `hours` = 24 (`Day`) ou
     168 (`Week`) conforme `state.history_range`. **Deletar
     `CHART_PROVIDERS` e a legend box antiga.**
   - Lista de dias (substitui a Table): para cada `DaySessions` (desc),
     1 linha-dia: `▸/▾ dd/mm · rótulo · tokens · $custo · N sessões`
     (rótulo: "hoje" se date == hoje local, senão dia da semana pt curto —
     seg/ter/qua/qui/sex/sáb/dom). Dia selecionado: fundo `SelBg` + seta em
     `Blue`. Expandido: linhas de sessão indentadas
     `hh:mm  projeto  modelo  tokens  $custo` (projeto `—` se None; modelo é
     `dominant_model` já tratado; custo `—` se None).
   - Linha extra do Amp por dia (AmpDollars) preservada como está hoje.
   - Scroll: manter `state.scroll` com o clamp local existente; overflow
     indicators mantidos.
   - Rodapé: `7 DIAS  $X.XX · N tok · M sessões` + chips
     `[Enter] expandir · [t] range · [r] atualizar · [esc] voltar`.
5. Snapshot tests do arquivo: regenerar; adicionar um caso com dia expandido:

```rust
#[test]
fn history_snapshot_with_expanded_day() {
    let mut state = fixture_history(); // fixture existente adaptado
    state.history_expanded.insert(time::macros::date!(2026 - 07 - 10));
    // render em 100x32 e assert_snapshot como os testes vizinhos
}
```

- [ ] **Step 5: Run tests + review**

Run: `cargo test tui::update`
Expected: PASS

Run: `cargo test history`
Expected: snapshots divergem — `cargo insta review`, verificar: dias com seta, dia expandido com sessões (hora/projeto/modelo/custo), chart de colunas no topo. Aceitar e re-rodar até PASS.

- [ ] **Step 6: Commit**

```bash
git add -A src
git commit -m "feat: histórico com dias expandíveis e sessões"
```

---

### Task 11: Navegação — Overview morre, boot no provider

**Files:**
- Delete: `src/tui/render/dashboard.rs` (+ snapshots `dashboard__*` e
  `tests__dashboard_*` em render/snapshots/)
- Modify: `src/tui/state.rs` (Screen/SidebarItem/sidebar_items/pending_focus)
- Modify: `src/tui/mod.rs` (assinatura run_tui), `src/tui/event_loop.rs`,
  `src/tui/update.rs`, `src/tui/render/mod.rs`, `src/tui/render/sidebar.rs`
- Modify: `src/main.rs:573-580,674-681` (chamadas run_tui)

**Interfaces:**
- Produces:

```rust
// tui/mod.rs
pub enum InitialFocus {
    Provider(String),
    Login(String),
}
pub async fn run_tui(ctx: &Ctx<'_>, focus: Option<InitialFocus>) -> anyhow::Result<()>

// state.rs
pub enum Screen { Detail, History, Login, Waybar }          // sem Overview
pub enum SidebarItem { Provider(usize), History, Login, Waybar }
pub fn sidebar_items(n_providers: usize) -> Vec<SidebarItem> // Provider(0..n) + MAIS
// AppState:
pub pending_focus: Option<String>,   // provider id aguardando fetch pra focar
```

- Consumed by: T12 (action-right passa `InitialFocus`).

- [ ] **Step 1: Estudar os arquivos**

Read `src/tui/event_loop.rs` INTEIRO, `src/tui/update.rs` (braços Activate/
Back/OpenDetail/Click/ProviderFetched), `src/tui/render/mod.rs` (dispatch +
help overlay) e `src/tui/render/sidebar.rs`.

- [ ] **Step 2: Write the failing tests**

Em `state.rs` `mod tests` (ou onde os testes de sidebar_items vivem — `rg -n "sidebar_items" src/`):

```rust
#[test]
fn sidebar_has_no_overview() {
    let items = sidebar_items(2);
    assert_eq!(
        items,
        vec![
            SidebarItem::Provider(0),
            SidebarItem::Provider(1),
            SidebarItem::History,
            SidebarItem::Login,
            SidebarItem::Waybar,
        ]
    );
}
```

Em `update.rs` `mod tests`:

```rust
#[test]
fn provider_fetched_resolves_pending_focus_by_id() {
    let mut state = AppState::new();
    state.pending_focus = Some("codex".into());
    // Chega claude primeiro: NÃO rouba o foco.
    update(&mut state, Action::ProviderFetched(quota_fixture("claude")));
    assert_eq!(state.pending_focus.as_deref(), Some("codex"));
    // Chega codex: resolve — Detail + selected no índice recém-inserido (1).
    update(&mut state, Action::ProviderFetched(quota_fixture("codex")));
    assert_eq!(state.screen, Screen::Detail);
    assert_eq!(state.selected, 1);
    assert_eq!(state.pending_focus, None);
}

#[test]
fn boot_state_is_detail_skeleton_not_overview() {
    let state = AppState::new();
    assert_eq!(state.screen, Screen::Detail);
}

#[test]
fn esc_from_history_returns_to_selected_provider_detail() {
    let mut state = AppState::new();
    update(&mut state, Action::ProviderFetched(quota_fixture("claude")));
    state.screen = Screen::History;
    update(&mut state, Action::Back);
    assert_eq!(state.screen, Screen::Detail);
}
```

(`quota_fixture(id)`: helper de teste existente ou novo que monta
`ProviderQuota` mínimo com `provider: id` — `rg -n "ProviderQuota \{" src/tui`
mostra o padrão dos testes vizinhos.)

- [ ] **Step 3: Run to verify fail**

Run: `cargo test tui`
Expected: erro de compilação (Overview referenciado por toda parte) — a lista de erros é o mapa da task

- [ ] **Step 4: Implementation**

Ordem sugerida (o compilador guia):

1. `state.rs`: remover `Screen::Overview` e `SidebarItem::Overview`;
   `sidebar_items` sem o Overview; `AppState::new` →
   `screen: Screen::Detail`, novo campo `pending_focus: None`.
2. `update.rs`:
   - `Action::ProviderFetched`: após o push/replace existente, resolver foco:

```rust
    if let Some(target) = state.pending_focus.clone() {
        if let Some(idx) = state.providers.iter().position(|p| p.quota.provider == target) {
            state.selected = idx;
            state.screen = Screen::Detail;
            state.sidebar_selected = idx; // Provider(i) é o i-ésimo item da sidebar
            state.pending_focus = None;
        }
    }
```

   - `Action::Back`: `state.screen = Screen::Detail` (mantém `selected`);
     zera scroll como hoje.
   - `Action::Activate(item)`: remover braço Overview; `Activate(Provider(i))`
     inalterado.
   - Braço de teclas: `Esc` na tela Detail → nenhuma action (remover Back do
     Detail); overlay de help fecha com Esc como hoje (precedência do overlay
     mantida).
3. `render/mod.rs`: dispatch sem Overview (`Screen::Detail` quando
   `state.providers.is_empty()` ou fetch pendente → render skeleton: painel
   com título do provider de `pending_focus` ou "carregando…" + throbber
   existente). Help overlay: atualizar textos (sem "Visão Geral").
4. `render/sidebar.rs`: seções `PROVEDORES`/`MAIS` (remover seção VISÃO);
   item de provider ganha sufixo de % quando `quota.windows` tiver janela de
   sessão (reusar o formato dos cards antigos — olhar dashboard.rs antes de
   deletar). Largura colapsada (<80): mostrar `●` colorido por provider (sem
   letra solta).
5. `tui/mod.rs`: `run_tui(ctx, focus: Option<InitialFocus>)` repassa pra
   `event_loop::run(octx, terminal, focus)`.
6. `event_loop.rs`: `run` ganha o parâmetro; no boot:

```rust
    match focus {
        Some(InitialFocus::Provider(id)) => state.pending_focus = Some(id),
        Some(InitialFocus::Login(id)) => {
            state.screen = Screen::Login;
            state.login_selected = login_index_of(&id); // 0=claude,1=codex,2=amp (ordem da tela Login)
        }
        None => {
            // primeiro provider habilitado (ordem do registry ∩ settings.waybar.providers)
            let first = crate::providers::registered_provider_ids()
                .iter()
                .find(|id| octx.settings.waybar.providers.iter().any(|p| p == *id))
                .map(|s| s.to_string());
            state.pending_focus = first;
        }
    }
```

7. `main.rs`: as 2 chamadas viram `tui::run_tui(&ctx, None).await`.
8. Deletar `src/tui/render/dashboard.rs`, remover `mod dashboard` e os
   snapshots `*dashboard*` (`rm src/tui/render/snapshots/*dashboard*`).
   Testes em render/mod.rs que renderizavam dashboard: migrar os que testavam
   sidebar/help pro novo boot (Detail skeleton), deletar os demais.
9. `mouse.rs`/`MouseTarget::Card`: cards eram do dashboard — remover o braço
   `Click(Card(i))` de update.rs FORA da tela Login (Login mantém). Se
   `MouseTarget::Card` ficar sem uso fora do Login, manter (Login usa).

- [ ] **Step 5: Run tests + review**

Run: `cargo test tui`
Expected: unit tests novos PASS; snapshots de sidebar/help/render divergem — `cargo insta review` (verificar: sidebar sem Visão Geral, com %, boot em Detail skeleton) e re-rodar até PASS

Run: `cargo build --all-targets && cargo clippy --all-targets -- -D warnings`
Expected: limpo (sem `dead_code` de dashboard; se `provider_series_24h` ficou órfã, deletar agora)

- [ ] **Step 6: Commit**

```bash
git add -A src
git commit -m "feat: boot direto no provider, overview removida"
```

---

### Task 12: action-right abre a TUI focada

**Files:**
- Modify: `src/action_right.rs`
- Modify: `src/main.rs:582-592` (braço ActionRight)
- Modify: docs em T15 (não aqui)

**Interfaces:**
- Consumes: `InitialFocus` (T11), `looks_disconnected` (existente).
- Produces: `pub async fn action_right_focus(provider_id: &str, ctx: &Ctx<'_>) -> Option<crate::tui::InitialFocus>`
  — decide o foco; `None` = provider desconhecido (erro já logado).
  `handle_action_right` (print estático + wait_enter) MORRE.

- [ ] **Step 1: Write the failing tests**

O roteamento é IO-dependente (is_available/get_quota); testar a parte pura:
manter os testes de `looks_disconnected` intactos e testar o mapeamento
decisão→foco com uma fn pura nova:

```rust
/// Decisão pura de foco a partir do estado do provider (testável sem IO).
pub fn focus_for(provider_id: &str, available: bool, quota_error: Option<&str>) -> crate::tui::InitialFocus {
    if !available || looks_disconnected(provider_id, quota_error) {
        crate::tui::InitialFocus::Login(provider_id.to_string())
    } else {
        crate::tui::InitialFocus::Provider(provider_id.to_string())
    }
}
```

```rust
#[test]
fn focus_routes_disconnected_to_login() {
    match focus_for("claude", true, Some("Token expired")) {
        crate::tui::InitialFocus::Login(id) => assert_eq!(id, "claude"),
        other => panic!("esperava Login, veio {other:?}"),
    }
}

#[test]
fn focus_routes_connected_to_provider_detail() {
    match focus_for("claude", true, None) {
        crate::tui::InitialFocus::Provider(id) => assert_eq!(id, "claude"),
        other => panic!("esperava Provider, veio {other:?}"),
    }
}

#[test]
fn focus_routes_unavailable_to_login() {
    assert!(matches!(
        focus_for("amp", false, None),
        crate::tui::InitialFocus::Login(_)
    ));
}
```

(`InitialFocus` precisa de `#[derive(Debug, Clone, PartialEq, Eq)]`.)

- [ ] **Step 2: Run to verify fail**

Run: `cargo test action_right`
Expected: erro de compilação (fn inexistente)

- [ ] **Step 3: Implementation**

`action_right.rs` — substituir `handle_action_right`/`login_stub`/`wait_enter`
por:

```rust
/// Resolve o foco inicial da TUI pro right-click. `None` = provider inválido.
pub async fn action_right_focus(
    provider_id: &str,
    ctx: &Ctx<'_>,
) -> Option<crate::tui::InitialFocus> {
    if provider_id.is_empty() {
        log::error!("Usage: {APP_NAME} action-right <provider>");
        return None;
    }
    let provider = match get_provider(provider_id) {
        Some(p) => p,
        None => {
            log::error!("Unknown provider: {provider_id}");
            return None;
        }
    };
    let available = provider.is_available(ctx).await;
    let error = if available {
        provider.get_quota(ctx).await.error
    } else {
        None
    };
    Some(focus_for(provider_id, available, error.as_deref()))
}
```

Imports que morrem junto: `BufRead`, `format_for_terminal`, `AllQuotas`,
`get_quota_for`, `iso_from_ms`, `Clock`, `invalidate` (checar com o compilador).

`main.rs` braço ActionRight:

```rust
    if matches!(opts.command, Command::ActionRight) {
        let provider = opts.provider.as_deref().unwrap_or("");
        match agent_bar::action_right::action_right_focus(provider, &ctx).await {
            Some(focus) => {
                if let Err(e) = tui::run_tui(&ctx, Some(focus)).await {
                    log::error!("TUI encerrou com erro: {e}");
                    std::process::exit(1);
                }
            }
            None => std::process::exit(1),
        }
        std::process::exit(0);
    }
```

- [ ] **Step 4: Run tests**

Run: `cargo test action_right`
Expected: PASS (3 novos + looks_disconnected antigos)

Run: `cargo test cli`
Expected: PASS (parsing de action-right inalterado)

- [ ] **Step 5: Commit**

```bash
git add -A src
git commit -m "feat: right-click abre a TUI no provider"
```

---

### Task 13: Fix do `interval` órfão no export do Waybar

**Files:**
- Modify: `src/waybar_contract.rs:50-98` (`module_definition`, `export_waybar_modules`)
- Modify: `src/waybar_integration.rs:528-612` (`apply_waybar_integration` passa o valor)
- Test: `tests/` goldens + testes de waybar_contract/waybar_integration

**Interfaces:**
- Produces: `module_definition(provider, terminal_script, app_bin, signal, interval: u32)`
  e `export_waybar_modules(..., interval: u32)` (tipo exato: o MESMO de
  `settings.waybar.interval` — Read `src/settings.rs` confirma; ajustar aqui
  se for u64).

- [ ] **Step 1: Write the failing test**

Em `src/waybar_contract.rs` `mod tests` (seguir padrão dos testes existentes
de module_definition):

```rust
#[test]
fn module_definition_uses_settings_interval() {
    let m = module_definition("claude", "term.sh", "agent-bar", 8, 60);
    assert_eq!(m.interval, 60);
    let m2 = module_definition("claude", "term.sh", "agent-bar", 8, 120);
    assert_eq!(m2.interval, 120);
}
```

Em `src/waybar_integration.rs` `mod tests` (padrão dos testes de apply com
temp dirs existentes): localizar o teste que verifica o modules.jsonc gerado
e adicionar asserção de que settings com `interval: 60` produz `"interval": 60`
no arquivo.

- [ ] **Step 2: Run to verify fail**

Run: `cargo test waybar_contract`
Expected: erro de compilação (aridade da fn)

- [ ] **Step 3: Implementation**

- `module_definition`: parâmetro novo `interval`, usar no campo (era `120`
  hardcoded na linha ~59).
- `export_waybar_modules`: recebe e repassa `interval`.
- `apply_waybar_integration`: passa `settings.waybar.interval`.
- Callers restantes: o compilador lista (setup/main) — todos têm `settings`
  em mãos.

- [ ] **Step 4: Run tests + goldens**

Run: `cargo test waybar_contract`
Expected: PASS

Run: `cargo test waybar_integration`
Expected: PASS

Run: `cargo test --test golden`
Expected: goldens de modules.jsonc divergem SE o default de settings ≠ 120 — inspecionar: o default de `settings.waybar.interval` deve permanecer 120 (compat), então goldens NÃO devem mudar. Se mudarem, o default foi alterado por engano — corrigir.

- [ ] **Step 5: Commit**

```bash
git add -A src tests
git commit -m "fix: interval das settings chega ao waybar"
```

---

### Task 14: Tela Config — seções WAYBAR/TUI + hint do signal

**Files:**
- Modify: `src/tui/render/config.rs`
- Test: snapshots do config regenerados

**Interfaces:**
- Consumes: `ConfigField::ALL` (state.rs, ordem mantida).
- Produces: render agrupado — cabeçalho de seção `WAYBAR` antes de
  Providers/ProviderOrder/Separators/DisplayMode/Signal/Interval e `TUI (só
  este menu)` antes de FxRate. **Sem mudança de ConfigField/ordem de navegação**
  (j/k continuam percorrendo a lista linear) — mudança é só visual. Descrição
  do campo Signal ganha o hint.

- [ ] **Step 1: Estudar + reordenar**

Read `src/tui/render/config.rs` e `src/tui/state.rs:76-110`. Para as seções
ficarem contíguas, mover `FxRate` pro FIM de `ConfigField::ALL` (Providers,
ProviderOrder, Separators, DisplayMode, Signal, Interval, FxRate) — a ordem
do enum é só display/navegação; nenhuma persistência depende dela.

- [ ] **Step 2: Write the failing snapshot expectation**

Nos testes de config.rs existentes, adicionar:

```rust
#[test]
fn config_renders_waybar_and_tui_sections() {
    // fixture/render iguais aos testes vizinhos, 100x32
    let text = /* buffer_to_string do frame renderizado */;
    assert!(text.contains("WAYBAR"), "seção WAYBAR ausente:\n{text}");
    assert!(text.contains("TUI"), "seção TUI ausente:\n{text}");
    let hint_ok = text.contains("não é disparado") || text.contains("disparo externo");
    assert!(hint_ok, "hint do signal ausente:\n{text}");
}
```

- [ ] **Step 3: Run to verify fail**

Run: `cargo test config`
Expected: FAIL (seções inexistentes)

- [ ] **Step 4: Implementation**

- `state.rs`: reordenar `ConfigField::ALL` (FxRate por último).
- `config.rs`: ao iterar os campos, emitir linha de cabeçalho de seção antes
  do primeiro campo de cada grupo:
  - antes de `Providers`: `WAYBAR` (Comment, bold);
  - antes de `FxRate`: `TUI · afeta só este menu` (Comment, bold).
- Descrição/help do campo Signal (onde o config.rs mostra a descrição do campo
  selecionado — se não houver área de descrição, acrescentar a linha de hint
  abaixo do campo quando selecionado):
  `sinal para refresh externo (pkill -SIGRTMIN+<n> waybar); o agent-bar não o dispara sozinho`.
- Título do painel: `Config` (era `Waybar`); item da sidebar continua "Waybar"?
  NÃO — renomear o label do item da sidebar pra `Config` também
  (`render/sidebar.rs`), mantendo a tecla `w`.

- [ ] **Step 5: Run tests + review**

Run: `cargo test config`
Expected: novo teste PASS; snapshots divergem — `cargo insta review`, aceitar.

Run: `cargo test settings`
Expected: PASS (nenhuma mudança de persistência)

- [ ] **Step 6: Commit**

```bash
git add -A src
git commit -m "feat: config com seções waybar/tui e hint do signal"
```

---

### Task 15: Docs e CLAUDE.md

**Files:**
- Modify: `CLAUDE.md` (§3: remover a regra do cache 5s fantasma)
- Modify: `docs/commands.md` (action-right agora abre a TUI focada; menu boota
  no primeiro provider), `docs/architecture.md` (diagrama/descrição do
  right-click), `docs/waybar-contract.md` (interval agora vem das settings),
  `docs/runtime.md` (se mencionar Overview — `rg -n "Overview|Visão Geral" docs/`)

**Interfaces:** nenhuma (docs).

- [ ] **Step 1: Editar CLAUDE.md**

Remover a linha:
```
- **`waybar_contract.rs` cacheia settings 5s** porque Waybar pulla em interval
  apertado.
```
(verificado: nenhuma ocorrência de cache em waybar_contract.rs — regra é
resíduo do port TS e induz agentes a erro.)

- [ ] **Step 2: Atualizar docs**

`rg -n "action-right|Overview|Visão Geral|interval" docs/*.md` e atualizar
cada menção ao comportamento antigo: right-click → "abre o menu TUI já no
provider clicado (login se desconectado)"; interval → "configurável via
`agent-bar menu` → Config"; remover menções à tela Visão Geral.

- [ ] **Step 3: Verificar**

Run: `git diff --check`
Expected: sem whitespace errors

Run: `rg -n "cacheia settings" CLAUDE.md docs/`
Expected: nenhuma ocorrência

- [ ] **Step 4: Commit**

```bash
git add CLAUDE.md docs
git commit -m "docs: TUI v8 e remoção da regra fantasma"
```

---

### Task 16: Gate final — suíte completa + verificação perceptual

**Files:** nenhum novo (verificação).

- [ ] **Step 1: Suíte completa**

Run: `cargo test`
Expected: PASS integral. Qualquer snapshot pendente de review = falha — resolver antes.

Run: `cargo clippy --all-targets -- -D warnings`
Expected: limpo.

Run: `cargo fmt --check`
Expected: limpo.

- [ ] **Step 2: Verificação funcional real (sem mutar o desktop)**

Rodar a TUI real em terminal interativo com os dados REAIS do usuário
(read-only — a TUI não escreve nada sem `s` na tela Config):

```bash
cargo build && ./target/debug/agent-bar menu
```

Checklist manual (ou via screenshot do terminal):
- boota direto no Detail do Claude (sem Visão Geral);
- gráfico tokens/h alto, com séries por modelo e legenda "Fable 5"/"Opus 4.8";
- nomes tratados e custo por modelo em MODELOS HOJE (Fable com $, não —);
- `h` → Histórico: dias com contagem de sessões; Enter expande mostrando
  hora/projeto/modelo/custo;
- `w` → Config: seções WAYBAR/TUI, hint do signal;
- sem bloco de linhas em branco em nenhuma altura de janela (testar
  redimensionando o terminal).

- [ ] **Step 3: Verificação perceptual**

Screenshot do terminal lado a lado com o mockup aprovado
(`scratchpad/tui-direcoes.html`, seção C · One Dark Turbo). Divergências de
layout/cor são finding, não “interpretação”.

- [ ] **Step 4: Reportar**

Reportar: o que mudou, o que foi verificado (comandos + resultado), risco
não-verificado (ex.: comportamento do action-right no popup real do Waybar —
requer clique real do usuário; NÃO rodar `agent-bar setup`/`update` pra
testar).
