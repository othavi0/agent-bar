# Confiança e Performance — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Números de tokens/custo corretos e consistentes em toda a TUI, cold-load do histórico ~instantâneo via cache persistente, estados de fetch estáveis, e limpeza de miudezas (help, CI, URLs, flake).

**Architecture:** O parser do Claude passa a deduplicar entradas de streaming por request e a extrair o breakdown de cache 5m/1h + modificadores (`speed`, `inference_geo`); a tabela de preços é reescrita com os valores oficiais de 2026-07-03. O cache de parse vira redb (KV embedded) com valores postcard, keyed por (path, size, mtime) e versionado. A TUI unifica o rótulo de tokens (input+output com sufixo de cache) e a fronteira de dia (meia-noite local).

**Tech Stack:** Rust, ratatui, redb (novo), postcard (novo), serde no `time` (feature nova), insta snapshots.

**Spec:** `docs/superpowers/specs/2026-07-03-confianca-e-performance-design.md`

## Global Constraints

- Rust/cargo only; deps novas precisam ser puro-Rust (build musl estático via cargo-zigbuild).
- `cargo clippy --all-targets -- -D warnings` limpo em todo commit; nunca `unwrap()`/`expect()` em código de produção.
- Provider error strings são contrato (testes assertam verbatim).
- XML-escape só em `render_pango.rs`; nunca round-trip de config Waybar via serde_json.
- Snapshots insta: atualizar SÓ quando o display contract muda de propósito; aceitar via `mv <nome>.snap.new <nome>.snap` (não há cargo-insta instalado).
- Gotcha RTK: 1 filtro posicional por invocação de `cargo test`; confiar no exit code.
- Testes: `XDG_CONFIG_HOME`/`XDG_CACHE_HOME` setados ANTES de qualquer import que leia `src/config.rs`; restaurar env em drop.
- Comunicação de repo/commits em português; Conventional Commits, subject ≤50 chars.
- NUNCA rodar `agent-bar setup/update/uninstall` (muta o desktop); TUI real só via tmux (`tmux new-session -d -x 110 -y 32 '<bin> menu'`).
- Read cada arquivo antes de Edit; se Edit falhar com `string not found`, re-Read antes de re-tentar.

---

### Task 1: Dedup de streaming no parser do Claude

O Claude Code escreve N entradas JSONL por request durante o streaming (mesmo
`requestId`, `output_tokens` crescendo). Somar todas conta a mesma request N
vezes. Manter só a ÚLTIMA entrada de cada request.

**Files:**
- Modify: `src/usage/claude.rs`
- Test: `src/usage/claude.rs` (mod tests inline)

**Interfaces:**
- Produces: `parse_claude_lines(lines) -> Vec<UsageRecord>` (assinatura
  inalterada; semântica nova: 1 record por request).

- [ ] **Step 1: Teste que falha** — em `src/usage/claude.rs` mod tests:

```rust
#[test]
fn streaming_entries_same_request_dedupe_to_last() {
    // 3 entradas do MESMO requestId com output_tokens crescendo (streaming
    // real do Claude Code) — só a última pode contar, senão a request é
    // somada 3x (bug documentado em claude-devtools#74).
    let lines = [
        r#"{"type":"assistant","requestId":"req_1","timestamp":"2026-07-03T10:00:00.000Z","message":{"id":"msg_1","model":"claude-fable-5","usage":{"input_tokens":100,"output_tokens":5}}}"#,
        r#"{"type":"assistant","requestId":"req_1","timestamp":"2026-07-03T10:00:01.000Z","message":{"id":"msg_1","model":"claude-fable-5","usage":{"input_tokens":100,"output_tokens":50}}}"#,
        r#"{"type":"assistant","requestId":"req_1","timestamp":"2026-07-03T10:00:02.000Z","message":{"id":"msg_1","model":"claude-fable-5","usage":{"input_tokens":100,"output_tokens":90}}}"#,
        r#"{"type":"assistant","requestId":"req_2","timestamp":"2026-07-03T10:01:00.000Z","message":{"id":"msg_2","model":"claude-fable-5","usage":{"input_tokens":10,"output_tokens":1}}}"#,
    ];
    let recs = parse_claude_lines(lines.into_iter());
    assert_eq!(recs.len(), 2, "1 record por request, não por linha");
    let req1 = recs.iter().find(|r| r.output == 90).expect("última entrada de req_1");
    assert_eq!(req1.input, 100);
    assert_eq!(recs.iter().map(|r| r.output).sum::<u64>(), 91);
}

#[test]
fn lines_without_request_id_fall_back_to_message_id_then_standalone() {
    // Sem requestId: dedup por message.id. Sem ambos: linha vale sozinha.
    let lines = [
        r#"{"type":"assistant","timestamp":"2026-07-03T10:00:00Z","message":{"id":"msg_a","model":"claude-fable-5","usage":{"input_tokens":1,"output_tokens":1}}}"#,
        r#"{"type":"assistant","timestamp":"2026-07-03T10:00:01Z","message":{"id":"msg_a","model":"claude-fable-5","usage":{"input_tokens":1,"output_tokens":7}}}"#,
        r#"{"type":"assistant","timestamp":"2026-07-03T10:00:02Z","message":{"model":"claude-fable-5","usage":{"input_tokens":3,"output_tokens":3}}}"#,
    ];
    let recs = parse_claude_lines(lines.into_iter());
    assert_eq!(recs.len(), 2);
    assert!(recs.iter().any(|r| r.output == 7), "msg_a dedupado pra última");
    assert!(recs.iter().any(|r| r.output == 3), "linha sem id vale sozinha");
}
```

- [ ] **Step 2: Rodar e ver falhar** — `cargo test streaming_entries` →
  FAIL (len == 4). E `cargo test lines_without_request_id` → FAIL.

- [ ] **Step 3: Implementar dedup** — em `parse_claude_lines`, acumular por
  chave em vez de push direto:

```rust
use std::collections::HashMap;

pub fn parse_claude_lines<'a>(lines: impl Iterator<Item = &'a str>) -> Vec<UsageRecord> {
    // Dedup de streaming: o Claude Code grava várias entradas por request
    // (mesmo requestId, output_tokens crescendo). A ÚLTIMA entrada vista
    // (ordem do arquivo) é o estado final da request — as anteriores são
    // parciais e somá-las multiplicaria tokens/custo (claude-devtools#74).
    // Chave: requestId; fallback message.id; sem ambos → índice da linha
    // (nunca colide: linha vale sozinha).
    let mut by_request: HashMap<String, UsageRecord> = HashMap::new();
    let mut order: Vec<String> = Vec::new();

    for (i, line) in lines.enumerate() {
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
        let key = v
            .get("requestId")
            .and_then(Value::as_str)
            .or_else(|| msg.get("id").and_then(Value::as_str))
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("__line_{i}"));

        let u = |k: &str| usage.get(k).and_then(Value::as_u64).unwrap_or(0);
        let rec = UsageRecord {
            provider: "claude".to_string(),
            model: msg.get("model").and_then(Value::as_str).map(|s| s.to_string()),
            input: u("input_tokens"),
            output: u("output_tokens"),
            cache_read: u("cache_read_input_tokens"),
            cache_write: u("cache_creation_input_tokens"),
            ts,
        };
        if by_request.insert(key.clone(), rec).is_none() {
            order.push(key);
        }
    }
    order.into_iter().filter_map(|k| by_request.remove(&k)).collect()
}
```

- [ ] **Step 4: Verificar verde** — `cargo test providers::claude` e
  `cargo test usage` (o teste antigo `parses_assistant_usage_line` continua
  passando: 1 linha → 1 record).

- [ ] **Step 5: Commit** — `git add src/usage/claude.rs && git commit -m "fix: dedup de streaming no parser do Claude"`

---

### Task 2: Campos novos no UsageRecord (cache 1h, fast, geo)

O JSONL real traz `usage.cache_creation.ephemeral_1h_input_tokens` /
`ephemeral_5m_input_tokens`, `usage.speed` e `usage.inference_geo` — dados
necessários pro preço correto (Task 3).

**Files:**
- Modify: `src/usage/mod.rs` (struct `UsageRecord`)
- Modify: `src/usage/claude.rs` (extração)
- Modify: todos os construtores de `UsageRecord` em testes (compilador guia;
  os novos campos têm default semanticamente neutro: `0`/`false`)
- Test: `src/usage/claude.rs`

**Interfaces:**
- Produces: `UsageRecord` ganha `cache_write_1h: u64` (subconjunto de
  `cache_write` gravado no tier 1h; 5m = `cache_write - cache_write_1h`),
  `fast: bool` (`usage.speed == "fast"`), `geo_us: bool`
  (`usage.inference_geo == "us"`). `cache_write` continua sendo o TOTAL
  (display não muda).

- [ ] **Step 1: Teste que falha** — em `src/usage/claude.rs`:

```rust
#[test]
fn extracts_cache_tiers_speed_and_geo() {
    let line = r#"{"type":"assistant","requestId":"r9","timestamp":"2026-07-03T10:00:00Z","message":{"model":"claude-opus-4-8","usage":{"input_tokens":10,"output_tokens":5,"cache_creation_input_tokens":300,"cache_read_input_tokens":0,"cache_creation":{"ephemeral_5m_input_tokens":100,"ephemeral_1h_input_tokens":200},"speed":"fast","inference_geo":"us"}}}"#;
    let recs = parse_claude_lines([line].into_iter());
    let r = &recs[0];
    assert_eq!(r.cache_write, 300, "total continua o campo agregado");
    assert_eq!(r.cache_write_1h, 200);
    assert!(r.fast);
    assert!(r.geo_us);
}

#[test]
fn missing_breakdown_defaults_to_zero_1h_and_standard() {
    let line = r#"{"type":"assistant","requestId":"r8","timestamp":"2026-07-03T10:00:00Z","message":{"model":"claude-opus-4-8","usage":{"input_tokens":10,"output_tokens":5,"cache_creation_input_tokens":300}}}"#;
    let r = &parse_claude_lines([line].into_iter())[0];
    // Fallback documentado da spec: sem breakdown, tratar tudo como 5m.
    assert_eq!(r.cache_write_1h, 0);
    assert!(!r.fast);
    assert!(!r.geo_us);
}
```

- [ ] **Step 2: Rodar e ver falhar** — `cargo test extracts_cache_tiers` →
  erro de compilação (campos inexistentes). É o red esperado.

- [ ] **Step 3: Implementar** — em `src/usage/mod.rs`:

```rust
pub struct UsageRecord {
    pub provider: String,
    pub model: Option<String>,
    pub input: u64,
    pub output: u64,
    pub cache_read: u64,
    /// TOTAL de cache write (5m + 1h) — é o que o display soma.
    pub cache_write: u64,
    /// Subconjunto de `cache_write` gravado no tier 1h (2× o input base).
    /// 5m = `cache_write - cache_write_1h`. 0 quando o log não traz o
    /// breakdown `usage.cache_creation` (fallback: tudo 5m, conservador).
    pub cache_write_1h: u64,
    /// `usage.speed == "fast"` (fast mode, preço premium em Opus 4.7/4.8).
    pub fast: bool,
    /// `usage.inference_geo == "us"` (multiplicador 1.1× em tudo).
    pub geo_us: bool,
    pub ts: OffsetDateTime,
}
```

Em `src/usage/claude.rs`, no lugar da construção atual do rec:

```rust
        let cache_creation = usage.get("cache_creation");
        let tier = |k: &str| {
            cache_creation
                .and_then(|c| c.get(k))
                .and_then(Value::as_u64)
                .unwrap_or(0)
        };
        let rec = UsageRecord {
            provider: "claude".to_string(),
            model: msg.get("model").and_then(Value::as_str).map(|s| s.to_string()),
            input: u("input_tokens"),
            output: u("output_tokens"),
            cache_read: u("cache_read_input_tokens"),
            cache_write: u("cache_creation_input_tokens"),
            cache_write_1h: tier("ephemeral_1h_input_tokens"),
            fast: usage.get("speed").and_then(Value::as_str) == Some("fast"),
            geo_us: usage.get("inference_geo").and_then(Value::as_str) == Some("us"),
            ts,
        };
```

O parser do Codex (`src/usage/codex.rs`) e todo construtor de teste ganham
`cache_write_1h: 0, fast: false, geo_us: false` (o compilador lista os
pontos; são ~15 construtores em testes de buckets/render/pricing).

- [ ] **Step 4: Verificar verde** — `cargo test usage` e depois
  `cargo test` completo (construtores de teste em render/* também compilam).

- [ ] **Step 5: Commit** — `git add -A src/ && git commit -m "feat: tiers de cache, fast e geo no UsageRecord"`

---

### Task 3: Tabela de preços 2026-07-03 + custo com tiers/modificadores

Valores oficiais capturados de
<https://platform.claude.com/docs/en/about-claude/pricing> em 2026-07-03
(USD por 1M tokens):

| Família | input | output | cache read | write 5m | write 1h |
|---|---|---|---|---|---|
| Fable 5 / Mythos 5 | 10 | 50 | 1.00 | 12.50 | 20 |
| Opus 4.5–4.8 | 5 | 25 | 0.50 | 6.25 | 10 |
| Opus 4.1 / Opus 4 (legado) | 15 | 75 | 1.50 | 18.75 | 30 |
| Sonnet 5 (até 2026-08-31) | 2 | 10 | 0.20 | 2.50 | 4 |
| Sonnet 5 (após) e Sonnet 4.x | 3 | 15 | 0.30 | 3.75 | 6 |
| Haiku 4.5 | 1 | 5 | 0.10 | 1.25 | 2 |

Modificadores: fast mode Opus 4.8 = $10/$50 e Opus 4.7 = $30/$150 (input/
output; cache multiplica sobre o preço fast); `inference_geo: us` = 1.1× em
todas as categorias. OpenAI: manter os valores verificados em 2026-06-20 e
re-verificar <https://developers.openai.com/api/docs/models> via WebFetch
antes do commit — se divergirem, atualizar tabela E teste juntos.

**Files:**
- Modify: `src/usage/pricing.rs`
- Test: `src/usage/pricing.rs`

**Interfaces:**
- Produces: `Pricing { input, output, cache_read, cache_write_5m, cache_write_1h }`
  (campo `cache_write` RENOMEADO para `cache_write_5m` + novo `cache_write_1h`);
  `pricing_for(model: &str, ts: OffsetDateTime) -> Option<Pricing>` (ganha
  `ts` pro preço introdutório do Sonnet 5); `cost_usd_of(rec)` inalterado
  na assinatura.

- [ ] **Step 1: Testes que falham** — substituir `verified_prices_2026_06_20`
  por:

```rust
#[test]
fn verified_prices_2026_07_03() {
    let ts = datetime!(2026-07-03 12:00 UTC);
    let fable = pricing_for("claude-fable-5", ts).unwrap();
    assert_eq!(
        (fable.input, fable.output, fable.cache_read, fable.cache_write_5m, fable.cache_write_1h),
        (10.0, 50.0, 1.0, 12.5, 20.0)
    );
    let opus = pricing_for("claude-opus-4-8", ts).unwrap();
    assert_eq!((opus.input, opus.output, opus.cache_write_1h), (5.0, 25.0, 10.0));
    let opus_legacy = pricing_for("claude-opus-4-1", ts).unwrap();
    assert_eq!((opus_legacy.input, opus_legacy.output), (15.0, 75.0));
    let haiku = pricing_for("claude-haiku-4-5", ts).unwrap();
    assert_eq!((haiku.input, haiku.output), (1.0, 5.0));
}

#[test]
fn sonnet5_intro_pricing_flips_on_2026_09_01() {
    let intro = pricing_for("claude-sonnet-5", datetime!(2026-08-31 23:00 UTC)).unwrap();
    assert_eq!((intro.input, intro.output), (2.0, 10.0));
    let standard = pricing_for("claude-sonnet-5", datetime!(2026-09-01 01:00 UTC)).unwrap();
    assert_eq!((standard.input, standard.output), (3.0, 15.0));
    let s46 = pricing_for("claude-sonnet-4-6", datetime!(2026-07-03 0:00 UTC)).unwrap();
    assert_eq!((s46.input, s46.output), (3.0, 15.0));
}

#[test]
fn cost_prices_cache_tiers_separately() {
    // Opus 4.8: 100 5m (6.25) + 200 1h (10.0) por 1M.
    let mut r = rec(Some("claude-opus-4-8"), 0, 0, 0, 300);
    r.cache_write_1h = 200;
    let c = cost_usd_of(&r).unwrap();
    let expected = (100.0 * 6.25 + 200.0 * 10.0) / 1_000_000.0;
    assert!((c - expected).abs() < 1e-12, "got {c}, want {expected}");
}

#[test]
fn fast_mode_reprices_opus48_and_geo_us_multiplies() {
    let mut r = rec(Some("claude-opus-4-8"), 1_000_000, 1_000_000, 0, 0);
    r.fast = true;
    let c = cost_usd_of(&r).unwrap();
    assert!((c - 60.0).abs() < 1e-9, "fast opus 4.8 = 10+50, got {c}");
    r.fast = false;
    r.geo_us = true;
    let c = cost_usd_of(&r).unwrap();
    assert!((c - 33.0).abs() < 1e-9, "geo us = 30 * 1.1, got {c}");
    // fast em modelo sem fast mode → preço padrão (sem panic, sem chute).
    let mut f = rec(Some("claude-fable-5"), 1_000_000, 0, 0, 0);
    f.fast = true;
    assert!((cost_usd_of(&f).unwrap() - 10.0).abs() < 1e-9);
}
```

(O helper `rec` do mod tests ganha os campos novos zerados.)

- [ ] **Step 2: Rodar e ver falhar** — `cargo test pricing` → erros de
  compilação (assinatura/campos). Red esperado.

- [ ] **Step 3: Implementar** — reescrever a tabela:

```rust
//! Atualizado: 2026-07-03. Fonte: platform.claude.com/docs/en/about-claude/pricing
//! (tabela completa incl. Fable/Mythos, tiers de cache 5m=1.25×/1h=2×,
//! fast mode Opus 4.7/4.8, inference_geo us=1.1×). OpenAI: developers.openai.com,
//! re-verificado nesta data.

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pricing {
    pub input: f64,
    pub output: f64,
    pub cache_read: f64,
    pub cache_write_5m: f64,
    pub cache_write_1h: f64,
}

fn p(input: f64, output: f64, cache_read: f64, w5m: f64, w1h: f64) -> Pricing {
    Pricing { input, output, cache_read, cache_write_5m: w5m, cache_write_1h: w1h }
}

pub fn pricing_for(model: &str, ts: time::OffsetDateTime) -> Option<Pricing> {
    let m = model.to_ascii_lowercase();
    // --- Anthropic ---
    if m.contains("fable") || m.contains("mythos") {
        return Some(p(10.0, 50.0, 1.0, 12.5, 20.0));
    }
    // Opus legado (4.1 e 4.0) custa 3× o 4.5+ — checar ANTES do genérico.
    if m.contains("opus-4-1") || m.ends_with("opus-4") || m.contains("opus-4-0") {
        return Some(p(15.0, 75.0, 1.5, 18.75, 30.0));
    }
    if m.contains("opus") {
        return Some(p(5.0, 25.0, 0.5, 6.25, 10.0));
    }
    if m.contains("sonnet-5") {
        // Preço introdutório até 2026-08-31 (docs oficiais).
        let intro_end = time::macros::datetime!(2026-09-01 00:00 UTC);
        return Some(if ts < intro_end {
            p(2.0, 10.0, 0.2, 2.5, 4.0)
        } else {
            p(3.0, 15.0, 0.3, 3.75, 6.0)
        });
    }
    if m.contains("sonnet") {
        return Some(p(3.0, 15.0, 0.3, 3.75, 6.0));
    }
    if m.contains("haiku-3") {
        return Some(p(0.8, 4.0, 0.08, 1.0, 1.6));
    }
    if m.contains("haiku") {
        return Some(p(1.0, 5.0, 0.1, 1.25, 2.0));
    }
    // --- OpenAI (verificar developers.openai.com antes do commit) ---
    if m.contains("codex") {
        return Some(p(1.75, 14.0, 0.175, 1.75, 1.75));
    }
    if m.starts_with("gpt-5.5") {
        return Some(p(5.0, 30.0, 0.5, 5.0, 5.0));
    }
    if m.starts_with("gpt-5") {
        return Some(p(1.25, 10.0, 0.125, 1.25, 1.25));
    }
    if m.starts_with("o4") {
        return Some(p(1.10, 4.40, 0.275, 1.10, 1.10));
    }
    None
}

/// Fast mode (research preview): Opus 4.8 = $10/$50, Opus 4.7 = $30/$150.
/// Cache multiplica sobre o preço fast (docs: multipliers stack). Modelos
/// sem fast mode ignoram o flag (billed standard, docs 2026-06-29).
fn fast_override(m: &str, base: Pricing) -> Pricing {
    let (input, output) = if m.contains("opus-4-8") {
        (10.0, 50.0)
    } else if m.contains("opus-4-7") {
        (30.0, 150.0)
    } else {
        return base;
    };
    let scale = input / base.input;
    Pricing {
        input,
        output,
        cache_read: base.cache_read * scale,
        cache_write_5m: base.cache_write_5m * scale,
        cache_write_1h: base.cache_write_1h * scale,
    }
}

pub fn cost_usd_of(rec: &UsageRecord) -> Option<f64> {
    let model = rec.model.as_deref()?;
    let mut p = pricing_for(model, rec.ts)?;
    if rec.fast {
        p = fast_override(&model.to_ascii_lowercase(), p);
    }
    let w1h = rec.cache_write_1h.min(rec.cache_write);
    let w5m = rec.cache_write - w1h;
    let mut cost = (rec.input as f64 * p.input
        + rec.output as f64 * p.output
        + rec.cache_read as f64 * p.cache_read
        + w5m as f64 * p.cache_write_5m
        + w1h as f64 * p.cache_write_1h)
        / 1_000_000.0;
    if rec.geo_us {
        cost *= 1.1; // inference_geo "us" (docs: multiplica todas as categorias)
    }
    Some(cost)
}
```

Atualizar os DOIS callers de `pricing_for` fora do módulo (rg
`pricing_for\(` — testes e possíveis usos em formatters) para passar `ts`.

- [ ] **Step 4: Re-verificar OpenAI online** — WebFetch em
  `https://developers.openai.com/api/docs/models` conferindo gpt-5.x/codex;
  divergiu → atualizar tabela + asserts no mesmo commit.

- [ ] **Step 5: Verificar verde** — `cargo test pricing`, depois
  `cargo test usage`, depois `cargo test --test golden` e
  `cargo test formatters` (custos aparecem no tooltip do Waybar — se um
  golden mudar, é mudança DELIBERADA de contrato: revisar o diff e aceitar).

- [ ] **Step 6: Commit** — `git add -A src/ && git commit -m "feat: preços 2026-07 c/ tiers, fast e geo"`

---

### Task 4: Rótulo duplo de tokens em toda a TUI

Decisão do dono: rótulo principal = input+output; sufixo `(+X cache)` onde
couber. Vale pra Detail, History (tabela + rodapé) e painel do Overview.

**Files:**
- Modify: `src/tui/render/shared.rs` (novo helper)
- Modify: `src/tui/render/detail.rs` (`model_tokens`, `provider_usage_tokens`,
  `totals_line` e a linha por-modelo)
- Modify: `src/tui/render/history.rs` (`DayBucket` consumo — tabela e
  `footer_line`)
- Modify: `src/tui/render/dashboard.rs` (`trend_totals_line`)
- Modify: `src/usage/buckets.rs` (`DayBucket` ganha `cache_tokens: u64`)
- Test: `src/tui/render/shared.rs`, snapshots dos 3 arquivos de render

**Interfaces:**
- Produces: `pub fn fmt_tokens_dual(io: u64, cache: u64) -> String` em
  `shared.rs` → `"9.9M (+1.4B cache)"`; cache 0 → `"9.9M"`.
- Produces: `DayBucket { date, tokens, cache_tokens, cost_usd }` (`tokens`
  continua input+output; `cache_tokens` = cache_read+cache_write do dia).

- [ ] **Step 1: Teste que falha** — em `shared.rs`:

```rust
#[test]
fn fmt_tokens_dual_formats_io_plus_cache() {
    assert_eq!(fmt_tokens_dual(9_900_000, 1_400_000_000), "9.9M (+1.4B cache)");
    assert_eq!(fmt_tokens_dual(9_900_000, 0), "9.9M");
    assert_eq!(fmt_tokens_dual(0, 500), "0 (+500 cache)");
}
```

(Requer `abbrev_tokens` suportar bilhões: verificar; se não formata `B`,
estender `abbrev_tokens` com o caso `>= 1_000_000_000` no mesmo estilo dos
casos K/M existentes, com teste próprio.)

- [ ] **Step 2: Ver falhar** — `cargo test fmt_tokens_dual` → função não existe.

- [ ] **Step 3: Implementar helper** — em `shared.rs`:

```rust
/// Rótulo duplo de tokens (decisão de produto, spec 2026-07-03): principal
/// = input+output (o que o usuário entende por "tokens que usei"); sufixo
/// = cache (read+write), presente só quando > 0. Em largura apertada, quem
/// chama pode dropar o sufixo — NUNCA o principal.
pub fn fmt_tokens_dual(io: u64, cache: u64) -> String {
    if cache == 0 {
        return abbrev_tokens(io);
    }
    format!("{} (+{} cache)", abbrev_tokens(io), abbrev_tokens(cache))
}
```

- [ ] **Step 4: Aplicar nos consumidores** — mudanças por arquivo:
  - `buckets.rs`: `DayBucket` ganha `cache_tokens`; `bucket_by_day`/
    `bucket_by_provider_day` acumulam `rec.cache_read + rec.cache_write`
    nesse campo (o campo `tokens` NÃO muda).
  - `detail.rs`: `model_tokens`/`provider_usage_tokens` viram pares —
    substituir por `fn model_tokens_split(mu) -> (u64, u64)` (io, cache) e
    `fn provider_tokens_split(pu) -> (u64, u64)`; as linhas por-modelo e
    `totals_line` usam `fmt_tokens_dual`. REMOVER o comentário de
    "convenção divergente" em `model_tokens` (a divergência morre aqui).
  - `history.rs`: coluna `tokens` da tabela usa
    `fmt_tokens_dual(b.tokens, b.cache_tokens)` quando a largura da coluna
    comporta (>= 20 chars), senão só `abbrev_tokens(b.tokens)`;
    `footer_line` usa o dual sempre.
  - `dashboard.rs` (`trend_totals_line`): hoje/7d com `fmt_tokens_dual`
    (hoje: io = total_input+total_output, cache = total_cache_read+
    total_cache_write do `state.usage`; 7d: somas dos records).

- [ ] **Step 5: Suíte + snapshots** — `cargo test` → snapshots de detail/
  history/dashboard mudam; revisar cada `.snap.new` (o rótulo novo deve
  aparecer, nada mais deve mudar) e aceitar com
  `cd src/tui/render/snapshots && for f in *.snap.new; do mv "$f" "${f%.new}"; done`.
  Re-rodar `cargo test` → verde.

- [ ] **Step 6: Commit** — `git add -A src/ && git commit -m "feat: rótulo duplo de tokens (io + cache)"`

---

### Task 5: Fronteira de dia local nos buckets

**Files:**
- Modify: `src/usage/buckets.rs` (`bucket_by_day`, `bucket_by_provider_day`)
- Modify: `src/tui/render/history.rs` (callers passam `state.local_offset`)
- Test: `src/usage/buckets.rs`

**Interfaces:**
- Produces: `bucket_by_day(records: &[UsageRecord], local_offset: time::UtcOffset) -> Vec<DayBucket>`
  e `bucket_by_provider_day(records, local_offset) -> BTreeMap<String, Vec<DayBucket>>`.

- [ ] **Step 1: Teste que falha:**

```rust
#[test]
fn buckets_use_local_day_not_utc() {
    // 02:00 UTC com offset -3 = 23:00 do dia ANTERIOR local.
    let r = rec("claude", "claude-fable-5", datetime!(2026-07-03 02:00 UTC), 100);
    let off = time::UtcOffset::from_hms(-3, 0, 0).unwrap();
    let buckets = bucket_by_day(&[r], off);
    assert_eq!(buckets[0].date, time::macros::date!(2026-07-02));
    // Offset zero preserva o comportamento anterior (snapshots intactos).
    let r2 = rec("claude", "claude-fable-5", datetime!(2026-07-03 02:00 UTC), 100);
    let utc = bucket_by_day(&[r2], time::UtcOffset::UTC);
    assert_eq!(utc[0].date, time::macros::date!(2026-07-03));
}
```

- [ ] **Step 2: Ver falhar** — `cargo test buckets_use_local_day` → erro de
  assinatura. Red.

- [ ] **Step 3: Implementar** — nas duas funções, trocar
  `let date = rec.ts.date();` por
  `let date = rec.ts.to_offset(local_offset).date();` e propagar o parâmetro.
  Callers: `render/history.rs` (tabela, `footer_line` — `footer_line` ganha
  o offset como parâmetro) passam `state.local_offset`; testes existentes
  passam `time::UtcOffset::UTC` (comportamento idêntico ao atual — nenhum
  snapshot muda nesta task).

- [ ] **Step 4: Verde** — `cargo test usage` e `cargo test render`.

- [ ] **Step 5: Commit** — `git add -A src/ && git commit -m "fix: buckets de dia usam fuso local"`

---

### Task 6: Cache persistente redb + postcard

**Files:**
- Modify: `Cargo.toml` (deps: `redb`, `postcard`; feature `serde` no `time`;
  `serde` derive já existe no repo)
- Modify: `src/usage/mod.rs` (`UsageRecord` deriva Serialize/Deserialize;
  `AggregateOptions` ganha `cache_db: Option<&'a Path>`)
- Rewrite: `src/usage/cache.rs`
- Modify: `src/tui/event_loop.rs` (passa `octx.paths.cache_dir.join("usage.redb")`)
- Test: `src/usage/cache.rs` (tempdir)

**Interfaces:**
- Produces: `UsageCache::open(db_path: Option<&Path>) -> UsageCache` (None =
  só memória, comportamento atual — usado pelos testes existentes);
  `cached_or_parse(&mut self, path, parse) -> Vec<UsageRecord>`;
  `gc(&mut self, live: &HashSet<PathBuf>)`; const `CACHE_VERSION: u64`.
- Consumes: `UsageRecord` com os campos da Task 2.

- [ ] **Step 1: Adicionar deps** — `cargo add redb postcard --features postcard/alloc`
  e habilitar `serde` no time: em `Cargo.toml`, adicionar `"serde"` às
  features existentes de `time`. Derivar em `UsageRecord`:
  `#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]`
  com `#[serde(with = "time::serde::timestamp")] pub ts: OffsetDateTime`
  (unix seconds — granularidade suficiente pra buckets por hora/dia).
  `cargo build` verde antes de seguir.

- [ ] **Step 2: Testes que falham** — em `src/usage/cache.rs` (substituem os
  três atuais, mantendo os cenários):

```rust
#[test]
fn persistent_cache_survives_reopen() {
    let dir = tempdir().unwrap();
    let db = dir.path().join("usage.redb");
    let f = dir.path().join("s.jsonl");
    std::fs::write(&f, "conteudo").unwrap();

    let mut c1 = UsageCache::open(Some(&db));
    let r1 = c1.cached_or_parse(&f, |_| vec![dummy_record()]);
    assert_eq!(r1.len(), 1);
    drop(c1);

    // Reabrir: mesmo (size, mtime) → NÃO re-parseia.
    let mut c2 = UsageCache::open(Some(&db));
    let r2 = c2.cached_or_parse(&f, |_| panic!("não deveria reparsear"));
    assert_eq!(r2, r1);
}

#[test]
fn changed_file_reparses_and_version_bump_drops_everything() {
    let dir = tempdir().unwrap();
    let db = dir.path().join("usage.redb");
    let f = dir.path().join("s.jsonl");
    std::fs::write(&f, "v1").unwrap();
    let mut c = UsageCache::open(Some(&db));
    let _ = c.cached_or_parse(&f, |_| vec![dummy_record()]);
    std::fs::write(&f, "v2 maior").unwrap(); // size muda → key muda
    let r = c.cached_or_parse(&f, |_| vec![]);
    assert!(r.is_empty(), "arquivo mudado re-parseia");
    drop(c);
    // Simular bump de versão: gravar meta antiga e reabrir.
    let mut c = UsageCache::open_with_version(Some(&db), CACHE_VERSION + 1);
    let r = c.cached_or_parse(&f, |_| vec![dummy_record(), dummy_record()]);
    assert_eq!(r.len(), 2, "versão nova invalida tudo");
}

#[test]
fn corrupted_db_is_rebuilt_not_fatal() {
    let dir = tempdir().unwrap();
    let db = dir.path().join("usage.redb");
    std::fs::write(&db, b"isto nao e um redb").unwrap();
    let mut c = UsageCache::open(Some(&db)); // não pode panicar
    let f = dir.path().join("s.jsonl");
    std::fs::write(&f, "x").unwrap();
    assert_eq!(c.cached_or_parse(&f, |_| vec![dummy_record()]).len(), 1);
}

#[test]
fn gc_removes_dead_paths() {
    let dir = tempdir().unwrap();
    let db = dir.path().join("usage.redb");
    let f = dir.path().join("s.jsonl");
    std::fs::write(&f, "x").unwrap();
    let mut c = UsageCache::open(Some(&db));
    let _ = c.cached_or_parse(&f, |_| vec![dummy_record()]);
    c.gc(&std::collections::HashSet::new()); // nenhum path vivo
    assert_eq!(c.persisted_len(), 0);
}
```

- [ ] **Step 3: Ver falhar** — `cargo test cache` → red (API nova).

- [ ] **Step 4: Implementar** — estrutura do novo `cache.rs`:

```rust
//! Cache persistente de UsageRecord por arquivo (redb + postcard).
//! Chave: path canônico; valor: postcard de (size, mtime, Vec<UsageRecord>).
//! O cache é DERIVADO: qualquer erro (corrupção, versão velha, IO) degrada
//! pra re-parse — nunca panica, nunca é fonte de verdade.

use redb::{Database, ReadableTable, TableDefinition};

const FILES: TableDefinition<&str, &[u8]> = TableDefinition::new("files");
const META: TableDefinition<&str, u64> = TableDefinition::new("meta");
/// Bump SEMPRE que UsageRecord ou a semântica do parse mudar (dedup, campos
/// novos) — força re-parse geral e corrige o histórico inteiro.
pub const CACHE_VERSION: u64 = 2;

pub struct UsageCache {
    memory: HashMap<PathBuf, (FileKey, Vec<UsageRecord>)>, // L1, por processo
    db: Option<Database>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct Entry {
    size: u64,
    mtime: i64,
    records: Vec<UsageRecord>,
}
```

Comportamentos (cada um coberto pelos testes do Step 2):
- `open(path)` → `open_with_version(path, CACHE_VERSION)`. Abre/cria via
  `Database::create`; erro → deleta o arquivo e tenta 1x de novo; erro de
  novo → `db: None` (memória só) + `log::warn!` no stderr. Depois compara
  `META["version"]` com a esperada; diferente → dropa a tabela FILES numa
  write txn e grava a versão nova.
- `cached_or_parse`: L1 hit → clone. Senão, read txn em FILES; postcard
  decode ok E (size, mtime) do stat batem → popula L1 e retorna. Senão
  parse + write txn upsert + L1. Falha de decode = miss silencioso.
- `gc(live)`: write txn removendo chaves fora de `live`.
- `persisted_len()`: `#[cfg(test)]`, conta chaves em FILES.
- Em `records()` (`usage/mod.rs`): construir `UsageCache::open(opts.cache_db)`,
  coletar os paths vivos num `HashSet` e chamar `gc` ao final.
- `event_loop.rs`: `AggregateOptions { cache_db: Some(&octx.paths.cache_dir.join("usage.redb")), .. }`
  nos dois pontos (spawn_usage_load). Testes existentes de `records()` usam
  `cache_db: None`.

- [ ] **Step 5: Verde + medição** — `cargo test cache`, `cargo test usage`,
  `cargo test` completo. Medir na máquina real (leitura, sem mutar nada):
  `time target/debug/agent-bar menu-font` não exercita usage — medir via
  tmux: abrir `agent-bar menu`, cronometrar até o painel "Hoje (24h)"
  popular (1ª run ~igual ao atual; 2ª run deve ser <2s).

- [ ] **Step 6: Commit** — `git add -A && git commit -m "feat: cache persistente de parse (redb)"`

---

### Task 7: Sidebar/Overview estáveis durante fetch inicial

**Files:**
- Modify: `src/tui/event_loop.rs` (seed antes do loop)
- Modify: `src/tui/state.rs` (helper `skeleton_quota`)
- Test: `src/tui/update.rs` (mod tests)

**Interfaces:**
- Produces: `pub fn skeleton_quota(id: &str) -> ProviderQuota` em `state.rs`
  (display_name: claude→"Claude", codex→"Codex", amp→"Amp", outro→id;
  `available: false`, `error: None`, demais campos `None`/default).
- Consumes: braço `Action::ProviderFetched` existente (substitui in-place
  por id — já funciona; o seed só garante que o slot existe desde o boot).

- [ ] **Step 1: Teste que falha** — em `update.rs` mod tests:

```rust
#[test]
fn provider_fetched_replaces_seeded_slot_in_place() {
    let mut state = AppState::new();
    state.providers = vec![
        ProviderView::new(crate::tui::state::skeleton_quota("claude")),
        ProviderView::new(crate::tui::state::skeleton_quota("codex")),
        ProviderView::new(crate::tui::state::skeleton_quota("amp")),
    ];
    state.fetch_pending = vec!["claude".into(), "codex".into(), "amp".into()];
    // Codex chega PRIMEIRO (ordem de chegada ≠ ordem configurada).
    update(&mut state, Action::ProviderFetched(Box::new(test_quota("codex", 55.0))));
    assert_eq!(state.providers.len(), 3, "nenhum slot novo — substitui in-place");
    assert_eq!(state.providers[1].quota.provider, "codex", "posição estável");
    assert!(state.providers[1].quota.available);
    assert_eq!(state.providers[0].quota.provider, "claude", "slot ainda skeleton");
}
```

- [ ] **Step 2: Ver falhar** — `cargo test provider_fetched_replaces_seeded` →
  `skeleton_quota` não existe. Red.

- [ ] **Step 3: Implementar** — `skeleton_quota` em `state.rs`:

```rust
/// Quota-esqueleto pro seed do boot: slot visível desde o 1º frame, na
/// ordem configurada — sem isto a sidebar/Overview crescem conforme cada
/// ProviderFetched chega e os itens pulam de posição na frente do usuário.
/// `login_state_for(Some(q), fetch_pending=true)` mapeia isto pra
/// "verificando…" no card.
pub fn skeleton_quota(id: &str) -> ProviderQuota {
    let display_name = match id {
        "claude" => "Claude",
        "codex" => "Codex",
        "amp" => "Amp",
        other => other,
    }
    .to_string();
    ProviderQuota {
        provider: id.to_string(),
        display_name,
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
```

No `event_loop.rs`, logo após montar o `AppState` inicial (antes do 1º
draw): semear `state.providers` com um slot por provider habilitado em
`octx.settings.waybar.providers` (na ordem configurada) e
`state.fetch_pending` com os mesmos ids — verificar o comportamento de
`render_skeleton` (dashboard): com providers semeados, `render_cards` roda
em vez do skeleton de 3 cards fixos; cada card semeado renderiza o estado
"verificando…" via `login_state_for`. Confirmar no snapshot.

- [ ] **Step 4: Verde + snapshot** — `cargo test tui`; rodar snapshot do
  dashboard com estado semeado (adicionar caso se nenhum cobre) e smoke
  tmux 110x32: abrir menu e capturar aos 1s — sidebar já deve listar os 3
  providers na ordem final.

- [ ] **Step 5: Commit** — `git add -A src/ && git commit -m "fix: slots de provider estáveis no boot"`

---

### Task 8: Header sem "-" no primeiro load

**Files:**
- Modify: `src/tui/render/mod.rs` (`header_status`)
- Test: `src/tui/render/mod.rs`

**Interfaces:**
- Consumes: `state.usage`, `state.last_update`, `state.fetch_pending`.

- [ ] **Step 1: Teste que falha:**

```rust
#[test]
fn header_first_load_shows_ellipsis_not_dash() {
    let mut state = AppState::new();
    state.fetch_pending = vec!["claude".to_string()];
    let line = header_status(&state);
    let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
    assert!(!text.contains('-'), "primeiro load não mostra '-': {text:?}");
    assert!(text.contains('…'), "primeiro load mostra reticências: {text:?}");
}
```

- [ ] **Step 2: Ver falhar** — `cargo test header_first_load` → red.

- [ ] **Step 3: Implementar** — em `header_status`, os dois fallbacks `"-"`
  (custo com `usage: None` e relógio com `last_update: None`) viram `"…"`
  QUANDO `!state.fetch_pending.is_empty()` (carregando de verdade) e
  continuam `"-"` caso contrário (estado genuinamente vazio, ex. erro).
  Durante refresh com dado velho presente, nada muda (custo/hora
  conhecidos continuam — comportamento atual já correto).

- [ ] **Step 4: Verde + snapshots** — `cargo test render`; snapshots que
  capturam o header em estado inicial mudam `-`→`…`: revisar e aceitar.

- [ ] **Step 5: Commit** — `git add -A src/ && git commit -m "fix: header mostra … no primeiro load"`

---

### Task 9: Diagnóstico do Codex "! erro" (máquina real)

Task INVESTIGATIVA — root cause antes de qualquer fix (systematic-debugging).
Não assumir a hipótese; colher evidência.

**Files:**
- Leitura: `src/providers/codex.rs`, `src/providers/base.rs`
- Possível fix: onde a evidência apontar (+ teste de contrato correspondente)

- [ ] **Step 1: Capturar o erro verbatim** — na máquina real (leitura, não
  muta nada): `target/debug/agent-bar refresh codex 2>&1 | head -20` (ou o
  comando de fetch avulso equivalente — ver `src/cli.rs` p/ o subcomando
  exato) e o tooltip/Detail do Codex na TUI via tmux. Anotar a string de
  erro EXATA.

- [ ] **Step 2: Reproduzir a fonte** — conferir o que o provider chama:
  `codex --version`, existência/formato de `~/.codex/auth.json` (só stat,
  não abrir conteúdo além do necessário), e o endpoint/CLI que
  `providers/codex.rs` usa. Comparar com o que a CLI real instalada faz.

- [ ] **Step 3: Formar hipótese única e escrever teste que falha** — com a
  causa identificada, escrever o teste de contrato que reproduz (mock do
  seam correspondente em `providers::codex`), ver falhar, corrigir, ver
  passar. Se a mensagem de erro do provider mudar, é mudança de CONTRATO:
  atualizar os asserts verbatim junto e dizer isso no commit.

- [ ] **Step 4: Verde** — `cargo test providers::codex` e smoke tmux: card
  do Codex sai de "! erro" (ou mostra o erro CORRETO e acionável, se a
  causa for externa — ex. não logado → mensagem padrão de login).

- [ ] **Step 5: Commit** — `git add -A src/ && git commit -m "fix: <causa real do erro do codex>"`

---

### Task 10: Help popup compacta em terminal baixo

**Files:**
- Modify: `src/tui/render/mod.rs` (`help_text`, `help_popup_area`,
  `render_help_overlay`)
- Test: `src/tui/render/mod.rs`

**Interfaces:**
- Produces: `fn help_text_fitting(max_rows: usize) -> Text<'static>` —
  conteúdo integral quando cabe; sem linhas em branco entre seções quando
  não cabe; truncado com linha final `… (+N atalhos)` em último caso.

- [ ] **Step 1: Teste que falha:**

```rust
#[test]
fn help_overlay_compacts_then_truncates_at_78x24() {
    let backend = ratatui::backend::TestBackend::new(78, 24);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    let mut state = AppState::new();
    state.show_help = true;
    terminal
        .draw(|f| render(&state, f, &mut HitMap::default()))
        .unwrap();
    let buffer = terminal.backend().buffer();
    let mut screen = String::new();
    for y in 0..24u16 {
        for x in 0..78u16 {
            if let Some(cell) = buffer.cell((x, y)) {
                screen.push_str(cell.symbol());
            }
        }
        screen.push('\n');
    }
    // 22 linhas úteis: compactação (23 linhas de conteúdo) não basta →
    // trunca com indicador. NUNCA corte mudo.
    assert!(
        screen.contains("atalhos)"),
        "corte deve ser anunciado com '… (+N atalhos)':\n{screen}"
    );
}
```

- [ ] **Step 2: Ver falhar** — `cargo test help_overlay_compacts` → red
  (corte mudo atual).

- [ ] **Step 3: Implementar** — `help_text_fitting(max_rows)`:
  (a) `help_text()` cabe (`height() <= max_rows`) → retorna como está;
  (b) reconstruir sem os `Line::from("")` separadores; coube → retorna;
  (c) truncar em `max_rows - 1` linhas + linha final
  `… (+{n_ocultos} atalhos)` estilizada `Muted`, onde `n_ocultos` = linhas
  de atalho (não-título) removidas. `help_popup_area` e
  `render_help_overlay` derivam `max_rows = frame.height - 2` e usam a
  MESMA `Text` para dimensionar e renderizar (uma chamada, passada adiante
  — não recomputar).

- [ ] **Step 4: Verde** — `cargo test render` (o teste de 110x32 da task
  anterior continua passando — conteúdo integral quando cabe) e smoke tmux
  78x24.

- [ ] **Step 5: Commit** — `git add -A src/ && git commit -m "fix: help compacta e anuncia corte em tela baixa"`

---

### Task 11: History carregado-vazio sem buraco

**Files:**
- Modify: `src/tui/render/history.rs` (branch vazio de `render_trend_chart`)
- Test: `src/tui/render/history.rs`

- [ ] **Step 1: Teste que falha:**

```rust
#[test]
fn empty_chart_message_is_vertically_centered() {
    let backend = ratatui::backend::TestBackend::new(100, 32);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    let mut state = AppState::new();
    state.screen = Screen::History;
    state.history = Some(vec![]);
    state.usage = Some(amp_usage(0.81, 5.0, 4.19)); // só-Amp: tabela de 1 linha
    terminal
        .draw(|f| render_history(&state, f, f.area(), &mut HitMap::default()))
        .unwrap();
    let buffer = terminal.backend().buffer();
    let mut msg_row = None;
    for y in 0..32u16 {
        let row: String = (0..100u16)
            .filter_map(|x| buffer.cell((x, y)).map(|c| c.symbol().to_string()))
            .collect();
        if row.contains("sem uso de tokens") {
            msg_row = Some(y);
        }
    }
    let y = msg_row.expect("mensagem presente");
    assert!(
        (8..=20).contains(&y),
        "mensagem deve estar centrada na área do chart, não colada no topo (y={y})"
    );
}
```

- [ ] **Step 2: Ver falhar** — `cargo test empty_chart_message` → y == 1. Red.

- [ ] **Step 3: Implementar** — no branch `series_by_provider.is_empty()`
  de `render_trend_chart`, centralizar vertical e horizontalmente:

```rust
    if series_by_provider.is_empty() {
        let y = area.y + area.height / 2;
        let line_area = Rect::new(area.x, y.min(area.y + area.height.saturating_sub(1)), area.width, 1);
        let p = Paragraph::new(Span::styled(
            empty_msg.to_string(),
            Style::default().fg(to_ratatui(ColorToken::Muted)),
        ))
        .alignment(Alignment::Center);
        frame.render_widget(p, line_area);
        return;
    }
```

- [ ] **Step 4: Verde + snapshots** — `cargo test history`; `history_empty`
  e `history_amp_only` snapshots mudam (mensagem desce/centraliza) —
  revisar e aceitar. O painel do Overview usa o mesmo branch (herda o fix).

- [ ] **Step 5: Commit** — `git add -A src/ && git commit -m "fix: mensagem de chart vazio centralizada"`

---

### Task 12: Flake de PATH nos testes

**Files:**
- Investigar: `rg -n "set_var" src/ --type rust` (enumerar TODOS os testes
  que mutam env de processo — PATH em particular)
- Modify: os arquivos apontados + `src/install.rs`
- Test: os já existentes (o deliverable é a serialização, não teste novo)

- [ ] **Step 1: Enumerar mutadores** — rodar o rg acima. Sintoma observado:
  `install::tests::ensure_command_true_when_present` (lê PATH via
  `which_in_path`) falhou 1x em paralelo com a suíte completa e passou
  isolado — algum teste zera/troca PATH concorrentemente.

- [ ] **Step 2: Implementar lock compartilhado** — criar em
  `src/test_support.rs` (novo, `#[cfg(test)]`-only via
  `#[cfg(test)] pub mod test_support` em `main.rs`/`lib.rs` conforme o
  layout) um mutex de env:

```rust
/// Serializa testes que MUTAM ou LEEM env sensível a mutação (PATH).
/// `std::env::set_var` é process-wide; dois testes em paralelo — um
/// mutando, outro lendo — flakam. Envenenamento é ignorado de propósito
/// (um teste que panicou não deve derrubar os vizinhos).
pub static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

pub fn env_guard() -> std::sync::MutexGuard<'static, ()> {
    ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner())
}
```

Todo teste enumerado no Step 1 que muta PATH E os testes de
`install.rs`/`providers/amp_cli.rs` que leem PATH começam com
`let _env = crate::test_support::env_guard();`.

- [ ] **Step 3: Verificar** — `cargo test` completo 3x seguidas → 3x verde
  (o flake era intermitente; 3 runs limpos + a análise causal é o critério).

- [ ] **Step 4: Commit** — `git add -A src/ && git commit -m "test: serializa testes que tocam PATH"`

---

### Task 13: CI node24 + URLs othavi0

**Files:**
- Modify: `.github/workflows/publish.yml`
- Modify: `packaging/aur/PKGBUILD`, `packaging/aur/.SRCINFO`
- Verificar: `rg -ni "othavioquiliao" --hidden -g '!.git'` (CHANGELOG e
  docs/superpowers históricos FICAM; código/packaging/README mudam)

- [ ] **Step 1: Bump das actions** — em `publish.yml`:
  `actions/checkout@v4` → `actions/checkout@v6`; `mlugg/setup-zig@v2` →
  verificar a major mais recente em
  `https://github.com/mlugg/setup-zig/releases` via WebFetch (existir v3+ →
  usar; senão manter v2 e registrar que o warning é do runner). Manter o
  pin `version: 0.16.0` do zig.

- [ ] **Step 2: URLs** — `url=` do PKGBUILD e `.SRCINFO` (linhas `url =` e
  `source =`) → `https://github.com/othavi0/agent-bar`. Rodar o rg do
  cabeçalho e atualizar qualquer ocorrência restante em código/README
  (NÃO em CHANGELOG/docs históricos).

- [ ] **Step 3: Validar** — `bash -n` não se aplica; validar YAML com
  `python3 -c "import yaml,sys; yaml.safe_load(open('.github/workflows/publish.yml'))"`.
  `./scripts/check-version v7.1.0` continua OK (pkgver não mudou).
  A validação REAL do workflow acontece no próximo release — anotar isso
  no commit.

- [ ] **Step 4: Commit** — `git add -A && git commit -m "chore: actions node24 e URLs othavi0"`

---

### Task 14: Verificação integrada final

- [ ] **Step 1: Suíte + lint** — `cargo test` (completo) e
  `cargo clippy --all-targets -- -D warnings` → verdes.

- [ ] **Step 2: Smoke real (tmux, leitura)** — build e abrir
  `agent-bar menu` em 110x32 E 78x24:
  - Boot: sidebar já com os 3 providers na ordem configurada (Task 7);
    header com `…` (Task 8).
  - 2ª abertura: painel "Hoje (24h)" populado em <2s (Task 6).
  - Rótulos duplos coerentes entre Detail / History / painel (Task 4) — o
    MESMO número principal nas três telas pro mesmo dia (Tasks 1+5).
  - Help em 78x24 com indicador de corte (Task 10).
  - Codex sem "! erro" ou com erro acionável (Task 9).

- [ ] **Step 3: Sanidade de custo** — comparar o custo de "hoje" exibido
  com uma conta manual: pegar 1 arquivo JSONL de hoje, somar via python3 os
  usage finais por requestId com a tabela nova, conferir ordem de grandeza
  (tolerância: diferenças por arquivos de outros projetos). Registrar o
  resultado no report final.

- [ ] **Step 4: Commit de fechamento se sobrar ajuste** — mensagens
  `fix:`/`chore:` conforme o caso. NÃO editar CHANGELOG nem cortar release
  (decisão do dono ao final).

---

## Self-review (feito na escrita)

- **Cobertura da spec:** S1 → Tasks 1-5; S2 → Task 6; S3 → Tasks 7-9;
  S4 → Tasks 10-13; testes/verificação → Task 14. Preços atualizados
  (pedido extra do dono) → Task 3 com valores reais capturados.
- **Placeholders:** nenhum TBD; os dois pontos "verificar online" (OpenAI,
  setup-zig) têm URL, critério e fallback explícitos.
- **Consistência de tipos:** `fmt_tokens_dual(u64, u64) -> String` (T4);
  `pricing_for(&str, OffsetDateTime)` (T3) casa com `cost_usd_of` usando
  `rec.ts`; `UsageRecord` novo (T2) é o serializado no cache (T6, versão 2);
  `bucket_by_day(records, UtcOffset)` (T5) casa com callers listados;
  `skeleton_quota` (T7) usa os campos reais de `ProviderQuota` (types.rs).
- **Ordem:** T1→T2→T3 sequenciais (parser→campos→preço); T4 depende de T2
  (cache split) e T5 é independente; T6 depende de T2 (serialização do
  record novo); T7-T13 independentes entre si; T14 fecha.
