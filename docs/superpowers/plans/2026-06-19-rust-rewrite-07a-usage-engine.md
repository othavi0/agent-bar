# Plano 07a — Engine de Usage/Custo (parsers de session log + pricing + agregação)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recomendado) ou superpowers:executing-plans, task-by-task. Steps usam checkbox (`- [ ]`).

**Goal:** Construir um subsistema PURO que lê os session logs locais (Claude/Codex) + o `amp usage`, normaliza em `UsageRecord`, cruza com uma tabela de preços estática e agrega tokens/custo (US$/R$) por provider/modelo/tempo — a base de dados da feature de monitoramento de custo da TUI (Plano 7b).

**Architecture:** Módulos puros e testáveis sob `rust/src/usage/`. Parsers recebem linhas (`&str`) → `Vec<UsageRecord>` (sem IO no núcleo de parsing). `pricing.rs` é uma tabela estática + `cost_of`. `cache.rs` faz índice incremental por `(path, size, mtime)` (o log do Claude tem ~12.8MB/sessão; só re-parseia arquivos novos/alterados). `mod.rs::aggregate` orquestra: descobre arquivos → cache → parse → pricing → `UsageSummary`. NENHUMA dependência de ratatui/TUI (consumido depois). Síncrono (não está no hot-path async).

**Tech Stack:** Rust 1.95, `serde_json` (parse JSONL linha-a-linha), `time` 0.3 (parse ISO8601 via `Rfc3339`), `std::fs` (descoberta + stat). Sem crates novas.

## Global Constraints

- **Fonte da verdade do design:** `docs/superpowers/specs/2026-06-19-tui-design.md` **§4b** (Engine de Usage/Custo). Os formatos de log foram verificados em disco (2026-06-19). Este é código NOVO (não há equivalente TS — o TS nunca parseou logs pra custo).
- **Formatos REAIS dos session logs (não inventar campos):**
  - **Claude** `~/.claude/projects/<hash>/<uuid>.jsonl`: linhas `{"type":"assistant", "timestamp":"<ISO>", "message":{"model":"claude-opus-4-8","usage":{"input_tokens":N,"output_tokens":N,"cache_creation_input_tokens":N,"cache_read_input_tokens":N}}}`. Tokens por chamada; modelo na mesma linha.
  - **Codex** `~/.codex/sessions/YYYY/MM/DD/rollout-*.jsonl`: modelo em `{"type":"session_meta",...,"model":"gpt-5.5"}` (1×) ou `{"type":"turn_context",...,"model":"..."}` (N×); tokens em `{"type":"event_msg","timestamp":"<ISO>","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":N,"cached_input_tokens":N,"output_tokens":N,"reasoning_output_tokens":N,"total_tokens":N},"last_token_usage":{...}}}}`. **`total_token_usage` é acumulado da sessão; `last_token_usage` é o delta da chamada.**
  - **Amp**: `amp usage` (CLI) → só $ (sem tokens). Reusar o que o provider Amp já parseia.
- **Modelo desconhecido na tabela de preço → custo `None`** (mostra tokens, omite $). **NUNCA chutar preço.** (Mesmo princípio "não reportar ok sem dado" do Waybar.)
- **US$ é exato (da tabela); R$ = US$ × `fx_rate`** configurável (default 5.50; sem fetch de câmbio ao vivo na v1).
- **Sem `unwrap()`/`expect()` em produção** (enforçado pelo `deny` em `lib.rs`; permitido em `#[cfg(test)]`).
- **Testes com fixtures SINTÉTICAS** no formato real (linhas inline nos testes) — NUNCA ler `~/.claude`/`~/.codex` reais; o `aggregate` recebe os diretórios por parâmetro (DI) → testes injetam `tempdir`.
- **Verificação por task:** `cargo test --manifest-path rust/Cargo.toml <filtro>` (UM filtro posicional) + `cargo clippy --manifest-path rust/Cargo.toml --all-targets -- -D warnings`. **RTK:** sem `test result:`; some os `passed`. Clean = `cargo clippy: No issues found`. **`cargo fmt` ANTES de `git add`.**
- **Commits:** Conventional Commits PT ≤50 chars. **Read antes de Edit** (`cat`/`sed` não contam); re-Read se Edit falhar com "string not found".

## Mapa de Arquivos

| Arquivo | Responsabilidade |
| --- | --- |
| `rust/src/usage/mod.rs` | tipos núcleo (`UsageRecord`, `Cost`, `ModelUsage`, `ProviderUsage`, `UsageSummary`, `TimeWindow`) + `aggregate(opts) -> UsageSummary` + `pub mod` dos submódulos |
| `rust/src/usage/pricing.rs` | `Pricing` + tabela estática + `pricing_for(model)` + `cost_usd_of(record)` |
| `rust/src/usage/claude.rs` | `parse_claude_lines(lines) -> Vec<UsageRecord>` |
| `rust/src/usage/codex.rs` | `parse_codex_lines(lines) -> Vec<UsageRecord>` |
| `rust/src/usage/amp.rs` | `amp_dollar_usage(...)` — saldo $ (reusa o parse do provider Amp) |
| `rust/src/usage/cache.rs` | índice incremental `(path,size,mtime) -> totais cacheados` |
| `rust/src/lib.rs` | `pub mod usage;` |

Ordem: **U1 (mod tipos + pricing) → U2 (claude) → U3 (codex) → U4 (cache + amp + aggregate)**.

---

### Task 1: `usage/mod.rs` tipos núcleo + `usage/pricing.rs`  (bloco U1)

**Files:**
- Create: `rust/src/usage/mod.rs`
- Create: `rust/src/usage/pricing.rs`
- Modify: `rust/src/lib.rs` (`pub mod usage;`, ordem alfabética — depois de `update`, antes de `watch`)
- Test: inline em ambos.

**Interfaces:**
- Produces:
  - `UsageRecord { provider: String, model: Option<String>, input: u64, output: u64, cache_read: u64, cache_write: u64, ts: time::OffsetDateTime }` (derive Debug, Clone, PartialEq)
  - `pricing::Pricing { input: f64, output: f64, cache_read: f64, cache_write: f64 }` (USD por 1M tokens)
  - `pricing::pricing_for(model: &str) -> Option<Pricing>`
  - `pricing::cost_usd_of(rec: &UsageRecord) -> Option<f64>` (None se modelo None ou desconhecido)

- [ ] **Step 1: Write failing tests (`pricing.rs`)**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::usage::UsageRecord;
    use time::macros::datetime;

    fn rec(model: Option<&str>, input: u64, output: u64, cr: u64, cw: u64) -> UsageRecord {
        UsageRecord {
            provider: "claude".into(),
            model: model.map(|s| s.to_string()),
            input, output, cache_read: cr, cache_write: cw,
            ts: datetime!(2026-06-19 12:00 UTC),
        }
    }

    #[test]
    fn pricing_matches_by_family_prefix() {
        assert!(pricing_for("claude-opus-4-8").is_some());
        assert!(pricing_for("claude-sonnet-4-6").is_some());
        assert!(pricing_for("claude-haiku-4-5").is_some());
        assert!(pricing_for("gpt-5.5").is_some());
    }

    #[test]
    fn unknown_model_has_no_pricing_and_no_cost() {
        assert!(pricing_for("totally-unknown-model").is_none());
        assert_eq!(cost_usd_of(&rec(Some("totally-unknown-model"), 1000, 1000, 0, 0)), None);
        assert_eq!(cost_usd_of(&rec(None, 1000, 1000, 0, 0)), None);
    }

    #[test]
    fn cost_is_weighted_sum_over_million() {
        // Opus: input 15, output 75 por 1M. 1M input + 1M output = 15 + 75 = 90.
        let c = cost_usd_of(&rec(Some("claude-opus-4-8"), 1_000_000, 1_000_000, 0, 0)).unwrap();
        assert!((c - 90.0).abs() < 1e-9, "got {c}");
        // cache_read mais barato que input.
        let p = pricing_for("claude-opus-4-8").unwrap();
        assert!(p.cache_read < p.input);
    }
}
```

- [ ] **Step 2: Run → fail.** `cargo test --manifest-path rust/Cargo.toml pricing 2>&1 | tail -6`

- [ ] **Step 3: Implement `usage/mod.rs` (tipos) + `usage/pricing.rs`**

`usage/mod.rs` (por enquanto só os tipos + declaração dos submódulos; `aggregate` vem na U4):
```rust
//! Engine de usage/custo: lê session logs locais → tokens → custo (US$/R$).
//! Subsistema PURO (sem TUI/ratatui). Ver spec §4b.

pub mod amp;
pub mod cache;
pub mod claude;
pub mod codex;
pub mod pricing;

use time::OffsetDateTime;

/// Uma chamada de API normalizada, extraída de um session log.
#[derive(Debug, Clone, PartialEq)]
pub struct UsageRecord {
    pub provider: String,
    pub model: Option<String>,
    pub input: u64,
    pub output: u64,
    pub cache_read: u64,
    pub cache_write: u64,
    pub ts: OffsetDateTime,
}
```
*(Os tipos de agregação — `Cost`, `ModelUsage`, `ProviderUsage`, `UsageSummary`, `TimeWindow` — entram na U4 junto com `aggregate`. Esta task entrega `UsageRecord` + pricing.)*

`usage/pricing.rs`:
```rust
//! Tabela de preço pública (USD por 1M tokens). VERIFICAR/atualizar com os preços
//! públicos vigentes — comentar a data. Match por PREFIXO de família (os nomes
//! exatos variam por versão). Modelo desconhecido → None (nunca chutar custo).

use super::UsageRecord;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pricing {
    pub input: f64,
    pub output: f64,
    pub cache_read: f64,
    pub cache_write: f64,
}

/// Preços públicos por 1M tokens (USD). Atualizado: 2026-06-19 (VERIFICAR antes de release).
/// Anthropic: cache_read ~= 0.1×input, cache_write ~= 1.25×input.
pub fn pricing_for(model: &str) -> Option<Pricing> {
    let m = model.to_ascii_lowercase();
    // Anthropic (Claude)
    if m.contains("opus") {
        return Some(Pricing { input: 15.0, output: 75.0, cache_read: 1.50, cache_write: 18.75 });
    }
    if m.contains("sonnet") {
        return Some(Pricing { input: 3.0, output: 15.0, cache_read: 0.30, cache_write: 3.75 });
    }
    if m.contains("haiku") {
        return Some(Pricing { input: 0.80, output: 4.0, cache_read: 0.08, cache_write: 1.00 });
    }
    // OpenAI (Codex / gpt-5.x) — PREÇOS PLACEHOLDER, VERIFICAR público OpenAI.
    if m.starts_with("gpt-5") || m.starts_with("o4") || m.contains("codex") {
        return Some(Pricing { input: 1.25, output: 10.0, cache_read: 0.125, cache_write: 1.25 });
    }
    None
}

/// Custo em USD do record. `None` se modelo ausente/desconhecido (mostra tokens sem $).
pub fn cost_usd_of(rec: &UsageRecord) -> Option<f64> {
    let model = rec.model.as_deref()?;
    let p = pricing_for(model)?;
    let cost = (rec.input as f64 * p.input
        + rec.output as f64 * p.output
        + rec.cache_read as f64 * p.cache_read
        + rec.cache_write as f64 * p.cache_write)
        / 1_000_000.0;
    Some(cost)
}
```

**Nota:** os submódulos `amp`/`cache`/`claude`/`codex` são declarados em `mod.rs` mas só ganham conteúdo nas tasks U2-U4. Para a U1 compilar, crie os arquivos `usage/{amp,cache,claude,codex}.rs` VAZIOS (ou com um comentário de doc) — senão `pub mod` falha. Conteúdo real entra nas tasks seguintes.

- [ ] **Step 4: Run → pass.** `cargo test --manifest-path rust/Cargo.toml pricing 2>&1 | tail -6` + `cargo clippy ...`.

- [ ] **Step 5: `cargo fmt` + commit**

```bash
cargo fmt --manifest-path rust/Cargo.toml
git add rust/src/usage/ rust/src/lib.rs
git commit -m "feat(rust): usage — tipos núcleo + tabela de preço"
```

---

### Task 2: `usage/claude.rs` — parser do session log do Claude  (bloco U2)

**Files:**
- Modify: `rust/src/usage/claude.rs`
- Test: inline.

**Interfaces:**
- Consumes: `UsageRecord` (U1), `serde_json`, `time`.
- Produces: `parse_claude_lines<'a>(lines: impl Iterator<Item = &'a str>) -> Vec<UsageRecord>`

**Regras (formato Claude):** linhas JSONL; só interessam `type == "assistant"` com `message.usage`. Extrai `message.model`, `message.usage.{input_tokens, output_tokens, cache_creation_input_tokens, cache_read_input_tokens}` (ausente → 0), `timestamp` (ISO → `OffsetDateTime::parse(.., &Rfc3339)`; falha → pula a linha). Linha não-JSON ou sem os campos → pula (parse leniente). `provider = "claude"`. `cache_write = cache_creation_input_tokens`, `cache_read = cache_read_input_tokens`.

- [ ] **Step 1: Write failing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    const LINE: &str = r#"{"type":"assistant","timestamp":"2026-06-19T11:22:19.163Z","message":{"model":"claude-opus-4-8","usage":{"input_tokens":8285,"output_tokens":2481,"cache_creation_input_tokens":15031,"cache_read_input_tokens":16291}}}"#;

    #[test]
    fn parses_assistant_usage_line() {
        let recs = parse_claude_lines([LINE].into_iter());
        assert_eq!(recs.len(), 1);
        let r = &recs[0];
        assert_eq!(r.provider, "claude");
        assert_eq!(r.model.as_deref(), Some("claude-opus-4-8"));
        assert_eq!(r.input, 8285);
        assert_eq!(r.output, 2481);
        assert_eq!(r.cache_write, 15031);
        assert_eq!(r.cache_read, 16291);
        assert_eq!(r.ts.year(), 2026);
    }

    #[test]
    fn skips_non_assistant_and_malformed() {
        let lines = [
            r#"{"type":"user","timestamp":"2026-06-19T11:00:00Z","message":{}}"#,
            "not json at all",
            r#"{"type":"assistant","message":{"model":"x"}}"#, // sem usage nem ts → pula
            LINE,
        ];
        let recs = parse_claude_lines(lines.into_iter());
        assert_eq!(recs.len(), 1); // só a LINE válida
    }

    #[test]
    fn missing_cache_fields_default_zero() {
        let line = r#"{"type":"assistant","timestamp":"2026-06-19T11:22:19Z","message":{"model":"claude-sonnet-4-6","usage":{"input_tokens":100,"output_tokens":50}}}"#;
        let recs = parse_claude_lines([line].into_iter());
        assert_eq!(recs[0].cache_read, 0);
        assert_eq!(recs[0].cache_write, 0);
    }
}
```

- [ ] **Step 2: Run → fail.** `cargo test --manifest-path rust/Cargo.toml claude 2>&1 | tail -6` *(cuidado: o filtro `claude` também casa `providers::claude` — confira que os novos testes de `usage::claude` aparecem; pode filtrar por `usage::claude` se preciso, mas `cargo test` aceita 1 filtro só — use o nome do teste, ex `parses_assistant_usage_line`)*.

- [ ] **Step 3: Implement**

```rust
//! Parser do session log do Claude (`~/.claude/projects/**/*.jsonl`). Ver spec §4b.

use serde_json::Value;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use super::UsageRecord;

/// Extrai `UsageRecord`s de linhas JSONL do Claude. Linhas sem `type:"assistant"`,
/// sem `message.usage`, sem timestamp parseável, ou não-JSON → puladas.
pub fn parse_claude_lines<'a>(lines: impl Iterator<Item = &'a str>) -> Vec<UsageRecord> {
    let mut out = Vec::new();
    for line in lines {
        let v: Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if v.get("type").and_then(Value::as_str) != Some("assistant") {
            continue;
        }
        let msg = match v.get("message") {
            Some(m) => m,
            None => continue,
        };
        let usage = match msg.get("usage") {
            Some(u) => u,
            None => continue,
        };
        let ts = match v.get("timestamp").and_then(Value::as_str) {
            Some(s) => match OffsetDateTime::parse(s, &Rfc3339) {
                Ok(t) => t,
                Err(_) => continue,
            },
            None => continue,
        };
        let u = |k: &str| usage.get(k).and_then(Value::as_u64).unwrap_or(0);
        out.push(UsageRecord {
            provider: "claude".to_string(),
            model: msg.get("model").and_then(Value::as_str).map(|s| s.to_string()),
            input: u("input_tokens"),
            output: u("output_tokens"),
            cache_read: u("cache_read_input_tokens"),
            cache_write: u("cache_creation_input_tokens"),
            ts,
        });
    }
    out
}
```

- [ ] **Step 4: Run → pass** + clippy.

- [ ] **Step 5: `cargo fmt` + commit**

```bash
cargo fmt --manifest-path rust/Cargo.toml
git add rust/src/usage/claude.rs
git commit -m "feat(rust): usage — parser do log do Claude"
```

---

### Task 3: `usage/codex.rs` — parser do session log do Codex  (bloco U3)

**Files:**
- Modify: `rust/src/usage/codex.rs`
- Test: inline.

**Interfaces:**
- Produces: `parse_codex_lines<'a>(lines: impl Iterator<Item = &'a str>) -> Vec<UsageRecord>`

**Regras (formato Codex — 2 passes na MESMA sessão):**
1. O **modelo** vem de `type == "session_meta"` (campo `model`) ou `type == "turn_context"` (campo `model`); pode mudar ao longo da sessão. Estratégia: rastrear o "modelo corrente" (atualiza ao ver `session_meta`/`turn_context`); aplicar aos `token_count` seguintes.
2. Os **tokens** vêm de `type == "event_msg"` com `payload.type == "token_count"`. Usar `payload.info.last_token_usage` (DELTA da chamada — evita double-count do acumulado). Campos: `input_tokens`, `cached_input_tokens`, `output_tokens` (+ `reasoning_output_tokens` somado ao output). `cache_read = cached_input_tokens`; `cache_write = 0` (Codex não separa). `timestamp` no topo do evento.
3. `provider = "codex"`. Modelo corrente `None` no início → record com `model: None` (custo omitido).

**IMPORTANTE — input vs cached:** no Codex, `input_tokens` JÁ inclui os `cached_input_tokens` (o cached é um subconjunto)? OU são disjuntos? **Decisão conservadora (documentar):** tratar `input` = `input_tokens` e `cache_read` = `cached_input_tokens` como o log reporta, **sem subtrair** (se houver double-count, é do formato; a tabela de preço aplica cache_read mais barato ao cached e input cheio ao input — pode superestimar levemente). Deixar um comentário no código sinalizando essa premissa pra revisitar se os números saírem altos.

- [ ] **Step 1: Write failing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    const META: &str = r#"{"type":"session_meta","timestamp":"2026-06-16T14:35:51Z","model":"gpt-5.5"}"#;
    const TOKENS: &str = r#"{"type":"event_msg","timestamp":"2026-06-16T14:36:00Z","payload":{"type":"token_count","info":{"last_token_usage":{"input_tokens":26016,"cached_input_tokens":2432,"output_tokens":582,"reasoning_output_tokens":175,"total_tokens":26598}}}}"#;

    #[test]
    fn associates_model_from_meta_with_token_events() {
        let recs = parse_codex_lines([META, TOKENS].into_iter());
        assert_eq!(recs.len(), 1);
        let r = &recs[0];
        assert_eq!(r.provider, "codex");
        assert_eq!(r.model.as_deref(), Some("gpt-5.5"));
        assert_eq!(r.input, 26016);
        assert_eq!(r.output, 582 + 175); // output + reasoning
        assert_eq!(r.cache_read, 2432);
        assert_eq!(r.cache_write, 0);
    }

    #[test]
    fn token_event_before_any_model_has_none_model() {
        let recs = parse_codex_lines([TOKENS].into_iter());
        assert_eq!(recs.len(), 1);
        assert_eq!(recs[0].model, None); // sem session_meta antes
    }

    #[test]
    fn turn_context_updates_current_model() {
        let turn = r#"{"type":"turn_context","timestamp":"2026-06-16T14:40:00Z","model":"gpt-5.5-codex"}"#;
        let recs = parse_codex_lines([META, TOKENS, turn, TOKENS].into_iter());
        assert_eq!(recs[0].model.as_deref(), Some("gpt-5.5"));
        assert_eq!(recs[1].model.as_deref(), Some("gpt-5.5-codex"));
    }

    #[test]
    fn skips_non_token_events_and_malformed() {
        let recs = parse_codex_lines(["garbage", r#"{"type":"event_msg","payload":{"type":"agent_message"}}"#].into_iter());
        assert_eq!(recs.len(), 0);
    }
}
```

- [ ] **Step 2: Run → fail.** `cargo test --manifest-path rust/Cargo.toml associates_model_from_meta_with_token_events 2>&1 | tail -6`

- [ ] **Step 3: Implement**

```rust
//! Parser do session log do Codex (`~/.codex/sessions/**/*.jsonl`). Ver spec §4b.
//! Modelo vem de session_meta/turn_context; tokens de event_msg/token_count.

use serde_json::Value;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use super::UsageRecord;

pub fn parse_codex_lines<'a>(lines: impl Iterator<Item = &'a str>) -> Vec<UsageRecord> {
    let mut out = Vec::new();
    let mut current_model: Option<String> = None;

    for line in lines {
        let v: Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        match v.get("type").and_then(Value::as_str) {
            Some("session_meta") | Some("turn_context") => {
                if let Some(m) = v.get("model").and_then(Value::as_str) {
                    current_model = Some(m.to_string());
                }
            }
            Some("event_msg") => {
                let payload = match v.get("payload") {
                    Some(p) => p,
                    None => continue,
                };
                if payload.get("type").and_then(Value::as_str) != Some("token_count") {
                    continue;
                }
                let last = match payload.get("info").and_then(|i| i.get("last_token_usage")) {
                    Some(l) => l,
                    None => continue,
                };
                let ts = match v.get("timestamp").and_then(Value::as_str) {
                    Some(s) => match OffsetDateTime::parse(s, &Rfc3339) {
                        Ok(t) => t,
                        Err(_) => continue,
                    },
                    None => continue,
                };
                let u = |k: &str| last.get(k).and_then(Value::as_u64).unwrap_or(0);
                // input_tokens reportado como vem (cached é subconjunto contado à parte
                // no preço de cache_read — premissa conservadora, ver spec §4b).
                out.push(UsageRecord {
                    provider: "codex".to_string(),
                    model: current_model.clone(),
                    input: u("input_tokens"),
                    output: u("output_tokens") + u("reasoning_output_tokens"),
                    cache_read: u("cached_input_tokens"),
                    cache_write: 0,
                    ts,
                });
            }
            _ => {}
        }
    }
    out
}
```

- [ ] **Step 4: Run → pass** + clippy.

- [ ] **Step 5: `cargo fmt` + commit**

```bash
cargo fmt --manifest-path rust/Cargo.toml
git add rust/src/usage/codex.rs
git commit -m "feat(rust): usage — parser do log do Codex"
```

---

### Task 4: `usage/cache.rs` + `usage/amp.rs` + `aggregate` em `mod.rs`  (bloco U4)

**Files:**
- Modify: `rust/src/usage/cache.rs`, `rust/src/usage/amp.rs`, `rust/src/usage/mod.rs`
- Test: inline (cache + aggregate end-to-end com tempdir).

**Interfaces:**
- Produces (em `mod.rs`):
  - `Cost { usd: f64, brl: f64 }`
  - `ModelUsage { model: String, input: u64, output: u64, cache_read: u64, cache_write: u64, cost: Option<Cost> }`
  - `ProviderUsage { provider: String, total_input: u64, total_output: u64, total_cache_read: u64, total_cache_write: u64, cost: Option<Cost>, by_model: Vec<ModelUsage>, amp_dollars: Option<AmpDollars> }`
  - `AmpDollars { spent: Option<f64>, remaining: Option<f64>, total: Option<f64> }` (de `amp.rs`)
  - `UsageSummary { providers: Vec<ProviderUsage>, total_cost: Cost, fx_rate: f64 }`
  - `AggregateOptions<'a> { claude_dir: &'a Path, codex_dir: &'a Path, fx_rate: f64, /* amp via seam opcional */ }`
  - `aggregate(opts: AggregateOptions) -> UsageSummary`
  - `records(opts: AggregateOptions) -> Vec<UsageRecord>` — os records crus de Claude+Codex ORDENADOS por `ts` (asc). Consumido pela aba History do Plano 7b pra montar a tendência no tempo (bucketing por dia/janela é responsabilidade do consumidor). `aggregate` pode chamar `records` internamente e então agrupar. Adicionar 1 teste: 2 arquivos com ts fora de ordem → `records` retorna ordenado.
- Em `cache.rs`: `UsageCache` (índice `path -> (size, mtime_unix, Vec<UsageRecord>)`) + `cached_or_parse(path, parser) -> Vec<UsageRecord>` (re-parseia só se size/mtime mudou). Persistência do índice em disco = OPCIONAL (pode ser em memória nesta task; persistir em `cache_dir/usage-index.json` é polish — se fizer, atômico).

**Decisões:**
- `aggregate` descobre os `.jsonl` recursivamente sob `claude_dir`/`codex_dir` (`walkdir`? NÃO — usar `std::fs::read_dir` recursivo manual, sem dep nova), parseia cada (via `cache::cached_or_parse`), junta todos os `UsageRecord`, agrupa por `(provider, model)`, soma tokens, calcula `cost` por modelo (`pricing::cost_usd_of` somado; `brl = usd * fx_rate`); agrega por provider; soma o total. Amp entra como `ProviderUsage` com `amp_dollars` (sem records de token).
- **Modelo desconhecido:** `cost: None` no `ModelUsage`; no `ProviderUsage.cost`, somar só os modelos com custo conhecido (custo parcial é melhor que nenhum; documentar).
- **Incremental:** o ganho real é não re-ler sessões antigas; o `cache` indexa por `(size, mtime)`. Nesta task pode ser em memória (o `UsageCache` é criado por `aggregate` e poderia persistir — deixar persistência como TODO comentado se faltar tempo, mas a interface `cached_or_parse` deve existir e ser usada).
- **Amp:** `amp.rs::amp_dollar_usage` pode receber um seam (closure que retorna o texto do `amp usage`, ou reusar `crate::providers::amp::parse_usage`) — para testar sem CLI. Manter simples: extrair `spent/remaining/total` do que o provider Amp já parseia (`meta.freeRemaining`/`freeTotal`). Se o provider Amp não expõe isso reutilizável, parsear o `$` do meta. **Investigar o que `providers::amp` já oferece e reusar; não duplicar regex.**

- [ ] **Step 1: Write failing test (`aggregate` end-to-end com tempdir)**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use tempfile::tempdir;

    fn write(dir: &Path, rel: &str, content: &str) {
        let p = dir.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, content).unwrap();
    }

    #[test]
    fn aggregate_sums_tokens_and_cost_per_provider() {
        let claude = tempdir().unwrap();
        let codex = tempdir().unwrap();
        write(claude.path(), "proj/sess.jsonl",
            r#"{"type":"assistant","timestamp":"2026-06-19T11:00:00Z","message":{"model":"claude-opus-4-8","usage":{"input_tokens":1000000,"output_tokens":1000000,"cache_creation_input_tokens":0,"cache_read_input_tokens":0}}}"#);
        write(codex.path(), "2026/06/19/rollout-x.jsonl",
            "{\"type\":\"session_meta\",\"model\":\"gpt-5.5\"}\n{\"type\":\"event_msg\",\"timestamp\":\"2026-06-19T11:05:00Z\",\"payload\":{\"type\":\"token_count\",\"info\":{\"last_token_usage\":{\"input_tokens\":1000000,\"cached_input_tokens\":0,\"output_tokens\":0,\"reasoning_output_tokens\":0,\"total_tokens\":1000000}}}}");

        let s = aggregate(AggregateOptions {
            claude_dir: claude.path(),
            codex_dir: codex.path(),
            fx_rate: 5.0,
        });

        let cl = s.providers.iter().find(|p| p.provider == "claude").unwrap();
        assert_eq!(cl.total_input, 1_000_000);
        // Opus 1M in + 1M out = $90; brl = 90*5 = 450.
        let c = cl.cost.as_ref().unwrap();
        assert!((c.usd - 90.0).abs() < 1e-6);
        assert!((c.brl - 450.0).abs() < 1e-6);

        let cx = s.providers.iter().find(|p| p.provider == "codex").unwrap();
        assert_eq!(cx.total_input, 1_000_000);
        assert!(cx.cost.is_some());

        assert!(s.total_cost.usd > 90.0); // claude + codex
        assert_eq!(s.fx_rate, 5.0);
    }

    #[test]
    fn unknown_model_contributes_tokens_but_not_cost() {
        let claude = tempdir().unwrap();
        let codex = tempdir().unwrap();
        write(claude.path(), "p/s.jsonl",
            r#"{"type":"assistant","timestamp":"2026-06-19T11:00:00Z","message":{"model":"mystery-model","usage":{"input_tokens":500,"output_tokens":500}}}"#);
        let s = aggregate(AggregateOptions { claude_dir: claude.path(), codex_dir: codex.path(), fx_rate: 5.0 });
        let cl = s.providers.iter().find(|p| p.provider == "claude").unwrap();
        assert_eq!(cl.total_input, 500);
        // modelo desconhecido → sem custo
        let m = cl.by_model.iter().find(|m| m.model == "mystery-model").unwrap();
        assert!(m.cost.is_none());
    }
}
```

- [ ] **Step 2: Run → fail.** `cargo test --manifest-path rust/Cargo.toml aggregate_sums_tokens_and_cost_per_provider 2>&1 | tail -6`

- [ ] **Step 3: Implement** `cache.rs` (cached_or_parse por size+mtime), `amp.rs` (reusa `providers::amp` parse — investigar a API antes), e em `mod.rs` os tipos de agregação + `aggregate`:
  - Descoberta recursiva de `.jsonl`: fn helper `collect_jsonl(dir) -> Vec<PathBuf>` (recursivo via `read_dir`; ignora erros de IO).
  - Para cada arquivo: `let lines = read_to_string(path)?; parse_*_lines(lines.lines())` (via cache).
  - Agrupar por `(provider, model_or_"unknown")`; somar; `cost` por modelo via `cost_usd_of` (somar os records do modelo) — atenção: `cost_usd_of` é por-record; somar os custos dos records do mesmo modelo. Modelo `None`/desconhecido → `cost: None`.
  - `ProviderUsage.cost` = soma dos `ModelUsage.cost` conhecidos (Some) → se nenhum conhecido, `None`; senão Some(soma). `brl = usd * fx_rate`.
  - `UsageSummary.total_cost` = soma de todos os providers.
  - Amp: `ProviderUsage { provider:"amp", tokens 0, cost: None, by_model: [], amp_dollars: Some(...) }`.

- [ ] **Step 4: Run → pass** + clippy + **suíte inteira** (`cargo test --manifest-path rust/Cargo.toml 2>&1 | tail -6` — zero regressão).

- [ ] **Step 5: `cargo fmt` + commit**

```bash
cargo fmt --manifest-path rust/Cargo.toml
git add rust/src/usage/cache.rs rust/src/usage/amp.rs rust/src/usage/mod.rs
git commit -m "feat(rust): usage — cache incremental + agregação"
```

---

## Self-Review (autor)

**1. Spec coverage (§4b):** fontes Claude/Codex/Amp → U2/U3/U4 ✓; UsageRecord → U1 ✓; pricing US$ + modelo-desconhecido→None → U1 ✓; R$ via fx_rate → U4 ✓; agregação por modelo/provider/tempo → U4 ✓ (tempo: os `ts` estão nos records; o bucketing por dia/janela é consumido pela History no Plano 7b — a U4 entrega os records com ts + agregados por modelo/provider; bucketing temporal fino pode ser uma função extra na U4 ou no 7b — **nota:** a History do 7b precisa de agregação por tempo; ou a U4 expõe os `Vec<UsageRecord>` crus + helpers de bucket, ou o 7b agrega. **Decisão:** a U4 expõe também `pub fn records(opts) -> Vec<UsageRecord>` (os crus, ordenados por ts) pro 7b fazer o bucketing temporal das sparklines. Adicionar isso à U4.); cache incremental → U4 ✓.

**2. Placeholder scan:** preços do OpenAI são PLACEHOLDER explicitamente marcados (VERIFICAR) — não é "TODO escondido", é um valor real-aproximado com nota; o maintainer atualiza. Sem outros placeholders.

**3. Type consistency:** `UsageRecord` (U1) usado por U2/U3/U4; `cost_usd_of`/`pricing_for` (U1) por U4; `Cost`/`ProviderUsage`/`UsageSummary` definidos na U4 e consumidos pelo 7b. `parse_claude_lines`/`parse_codex_lines` assinaturas batem com o uso na U4. ✓

**Riscos p/ o reviewer:**
- **R1 — premissa input-vs-cached do Codex** (U3): trata `input_tokens` e `cached_input_tokens` como o log dá, sem subtrair; pode superestimar custo se forem sobrepostos. Documentado; revisitar se os números saírem altos vs a realidade.
- **R2 — preços OpenAI placeholder** (U1): marcar claramente; custo do Codex é aproximado até verificar o preço público real dos modelos.
- **R3 — `last_token_usage` vs `total_token_usage`** (U3): uso `last` (delta) pra somar sem double-count; se um log só tiver `total` (sem `last`), esses eventos são pulados — aceitável (o formato real tem ambos).
- **R4 — bucketing temporal** (U4 self-review acima): a U4 deve expor os records crus ordenados por ts (`records()`) além dos agregados, senão o 7b (History) não tem como montar a tendência no tempo.
