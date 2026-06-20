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
        return Some(Pricing {
            input: 15.0,
            output: 75.0,
            cache_read: 1.50,
            cache_write: 18.75,
        });
    }
    if m.contains("sonnet") {
        return Some(Pricing {
            input: 3.0,
            output: 15.0,
            cache_read: 0.30,
            cache_write: 3.75,
        });
    }
    if m.contains("haiku") {
        return Some(Pricing {
            input: 0.80,
            output: 4.0,
            cache_read: 0.08,
            cache_write: 1.00,
        });
    }
    // OpenAI (Codex / gpt-5.x) — PREÇOS PLACEHOLDER, VERIFICAR público OpenAI.
    if m.starts_with("gpt-5") || m.starts_with("o4") || m.contains("codex") {
        return Some(Pricing {
            input: 1.25,
            output: 10.0,
            cache_read: 0.125,
            cache_write: 1.25,
        });
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
        assert_eq!(
            cost_usd_of(&rec(Some("totally-unknown-model"), 1000, 1000, 0, 0)),
            None
        );
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
