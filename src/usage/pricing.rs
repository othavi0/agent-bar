//! Tabela de preço pública (USD por 1M tokens). Match por PREFIXO/substring de
//! família (os nomes exatos variam por versão). Modelo desconhecido → None
//! (nunca chutar custo).
//!
//! Atualizado: 2026-07-17. Fontes oficiais (revalidadas na trilha B):
//! - Anthropic: <https://platform.claude.com/docs/en/about-claude/pricing>
//!   (Fable/Mythos, Sonnet 5 introdutório com virada em 2026-09-01, Opus 4.5–4.8,
//!   Opus legado ≤4.1, Sonnet ≤4.6, Haiku; cache_read = 0.1×input,
//!   cache_write 5m = 1.25×input / 1h = 2×input; fast mode Opus 4.7/4.8;
//!   `inference_geo: us` = 1.1× em todas as categorias).
//! - OpenAI: <https://developers.openai.com/api/docs/models> (gpt-5.x / o-series / codex).
//!   A OpenAI NÃO cobra escrita de cache separada (Prompt Caching automático) →
//!   modelamos cache_write = input (sem sobretaxa); cache_read = "cached input".
//!
//! NOTA (revisar quando trocar de modelo): a match-key `codex` usa `gpt-5.3-codex`
//! (padrão do CLI Codex, fev/2026) = $1.75/$14. O CLI também aceita `gpt-5.5`
//! ($5/$30) e `gpt-5-codex` ($1.25/$10) — se teu Codex usa outro, ajustar aqui.

use std::collections::HashSet;
use std::sync::Mutex;

use super::UsageRecord;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pricing {
    pub input: f64,
    pub output: f64,
    pub cache_read: f64,
    pub cache_write_5m: f64,
    pub cache_write_1h: f64,
}

fn p(input: f64, output: f64, cache_read: f64, w5m: f64, w1h: f64) -> Pricing {
    Pricing {
        input,
        output,
        cache_read,
        cache_write_5m: w5m,
        cache_write_1h: w1h,
    }
}

/// Preço (USD por 1M tokens) para o modelo, por família, na data `ts` (o
/// Sonnet 5 tem preço introdutório até 2026-08-31). `None` se desconhecido.
pub fn pricing_for(model: &str, ts: time::OffsetDateTime) -> Option<Pricing> {
    let m = model.to_ascii_lowercase();

    // --- Anthropic ---
    if m.contains("fable") || m.contains("mythos") {
        return Some(p(10.0, 50.0, 1.0, 12.5, 20.0));
    }
    if m.contains("opus") {
        // Legado (≤4.1) custa 3× o 4.5+: claude-opus-4-1 / claude-opus-4 sem
        // minor ≥5 / opus-3 — checar ANTES do genérico.
        if m.contains("opus-4-1")
            || m.contains("opus-4-0")
            || m.ends_with("opus-4")
            || m.contains("3-opus")
            || m.contains("opus-3")
        {
            return Some(p(15.0, 75.0, 1.5, 18.75, 30.0));
        }
        return Some(p(5.0, 25.0, 0.5, 6.25, 10.0));
    }
    if m.contains("sonnet-5") {
        // Preço introdutório até 2026-08-31 (platform.claude.com).
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

    // --- OpenAI (Codex / gpt-5.x / o-series) — do mais específico p/ o mais geral ---
    // `gpt-5.3-codex` contém "codex" → cai aqui antes do prefixo `gpt-5`.
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
        // Proxy: o4-mini (não há "o4" standalone público). confidence baixa.
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

/// Modelos desconhecidos já avisados nesta execução (warn-once por modelo).
static WARNED_UNKNOWN: Mutex<Option<HashSet<String>>> = Mutex::new(None);

/// Custo em USD do record. `None` se modelo ausente/desconhecido (mostra tokens sem $).
pub fn cost_usd_of(rec: &UsageRecord) -> Option<f64> {
    let model = rec.model.as_deref()?;
    let mut p = match pricing_for(model, rec.ts) {
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
    if rec.fast {
        p = fast_override(&model.to_ascii_lowercase(), p);
    }
    // cache_write_1h é subconjunto de cache_write (extração pode não clampar) —
    // clampar aqui garante que 5m nunca fique negativo.
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::usage::UsageRecord;
    use time::macros::datetime;

    fn rec(model: Option<&str>, input: u64, output: u64, cr: u64, cw: u64) -> UsageRecord {
        UsageRecord {
            provider: "claude".into(),
            model: model.map(|s| s.to_string()),
            input,
            output,
            cache_read: cr,
            cache_write: cw,
            cache_write_1h: 0,
            fast: false,
            geo_us: false,
            ts: datetime!(2026-06-19 12:00 UTC),
            session_id: None,
            project: None,
        }
    }

    #[test]
    fn pricing_matches_by_family_prefix() {
        let ts = datetime!(2026-07-10 12:00 UTC);
        assert!(pricing_for("claude-opus-4-8", ts).is_some());
        assert!(pricing_for("claude-sonnet-4-6", ts).is_some());
        assert!(pricing_for("claude-haiku-4-5", ts).is_some());
        assert!(pricing_for("gpt-5.5", ts).is_some());
        assert!(pricing_for("gpt-5.3-codex", ts).is_some());
        assert!(pricing_for("o4-mini", ts).is_some());
    }

    #[test]
    fn verified_prices_2026_07_10() {
        let ts = datetime!(2026-07-10 12:00 UTC);
        // Anthropic (confirmado vs platform.claude.com, cross-check adversarial).
        let opus = pricing_for("claude-opus-4-8", ts).unwrap();
        assert_eq!(
            (opus.input, opus.output, opus.cache_write_1h),
            (5.0, 25.0, 10.0)
        );
        let sonnet = pricing_for("claude-sonnet-4-6", ts).unwrap();
        assert_eq!((sonnet.input, sonnet.output), (3.0, 15.0));
        let haiku = pricing_for("claude-haiku-4-5", ts).unwrap();
        assert_eq!((haiku.input, haiku.output), (1.0, 5.0));
        // OpenAI: gpt-5.5 ($5/$30) distinto de codex ($1.75/$14, gpt-5.3-codex).
        let gpt55 = pricing_for("gpt-5.5", ts).unwrap();
        assert_eq!((gpt55.input, gpt55.output), (5.0, 30.0));
        let codex = pricing_for("gpt-5.3-codex", ts).unwrap();
        assert_eq!((codex.input, codex.output), (1.75, 14.0));
        // "codex" tem precedência sobre o prefixo "gpt-5".
        assert_eq!(
            pricing_for("gpt-5.3-codex", ts),
            pricing_for("some-codex-model", ts)
        );
    }

    #[test]
    fn fable_and_mythos_pricing_2026_07_10() {
        let ts = datetime!(2026-07-10 12:00 UTC);
        let fable = pricing_for("claude-fable-5", ts).unwrap();
        assert_eq!(
            (
                fable.input,
                fable.output,
                fable.cache_read,
                fable.cache_write_5m,
                fable.cache_write_1h
            ),
            (10.0, 50.0, 1.0, 12.5, 20.0)
        );
        assert_eq!(
            pricing_for("claude-mythos-5", ts),
            pricing_for("claude-fable-5", ts)
        );
    }

    #[test]
    fn sonnet5_intro_pricing_flips_on_2026_09_01() {
        let intro = pricing_for("claude-sonnet-5", datetime!(2026-08-31 23:00 UTC)).unwrap();
        assert_eq!((intro.input, intro.output), (2.0, 10.0));
        let standard = pricing_for("claude-sonnet-5", datetime!(2026-09-01 01:00 UTC)).unwrap();
        assert_eq!((standard.input, standard.output), (3.0, 15.0));
        let s46 = pricing_for("claude-sonnet-4-6", datetime!(2026-07-10 0:00 UTC)).unwrap();
        assert_eq!((s46.input, s46.output), (3.0, 15.0));
    }

    #[test]
    fn legacy_opus_41_keeps_old_pricing() {
        let ts = datetime!(2026-07-10 12:00 UTC);
        let o41 = pricing_for("claude-opus-4-1", ts).unwrap();
        assert_eq!((o41.input, o41.output), (15.0, 75.0));
        let o48 = pricing_for("claude-opus-4-8", ts).unwrap();
        assert_eq!((o48.input, o48.output), (5.0, 25.0));
    }

    #[test]
    fn real_claude_3_opus_id_gets_legacy_pricing() {
        let ts = datetime!(2026-07-10 12:00 UTC);
        let o3 = pricing_for("claude-3-opus-20240229", ts).unwrap();
        assert_eq!((o3.input, o3.output), (15.0, 75.0));
    }

    #[test]
    fn unknown_model_has_no_pricing_and_no_cost() {
        let ts = datetime!(2026-07-10 12:00 UTC);
        assert!(pricing_for("totally-unknown-model", ts).is_none());
        assert_eq!(
            cost_usd_of(&rec(Some("totally-unknown-model"), 1000, 1000, 0, 0)),
            None
        );
        assert_eq!(cost_usd_of(&rec(None, 1000, 1000, 0, 0)), None);
    }

    #[test]
    fn cost_is_weighted_sum_over_million() {
        let ts = datetime!(2026-07-10 12:00 UTC);
        // Opus: input 5, output 25 por 1M. 1M input + 1M output = 5 + 25 = 30.
        let c = cost_usd_of(&rec(Some("claude-opus-4-8"), 1_000_000, 1_000_000, 0, 0)).unwrap();
        assert!((c - 30.0).abs() < 1e-9, "got {c}");
        // cache_read mais barato que input.
        let p = pricing_for("claude-opus-4-8", ts).unwrap();
        assert!(p.cache_read < p.input);
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
}
