//! Tabela de preço pública (USD por 1M tokens). Match por PREFIXO/substring de
//! família (os nomes exatos variam por versão). Modelo desconhecido → None
//! (nunca chutar custo).
//!
//! Atualizado: 2026-07-10. Fontes oficiais (verificadas + cross-check adversarial):
//! - Anthropic: <https://platform.claude.com/docs/en/about-claude/pricing>
//!   (Fable/Mythos, Sonnet 5 introdutório, Opus 4.5–4.8, Opus legado ≤4.1,
//!   Sonnet ≤4.6, Haiku 4.5; cache_read = 0.1×input, cache_write 5min = 1.25×input).
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
    pub cache_write: f64,
}

/// Preço (USD por 1M tokens) para o modelo, por família. `None` se desconhecido.
pub fn pricing_for(model: &str) -> Option<Pricing> {
    let m = model.to_ascii_lowercase();

    // --- Anthropic ---
    if m.contains("fable") || m.contains("mythos") {
        return Some(Pricing {
            input: 10.0,
            output: 50.0,
            cache_read: 1.0,
            cache_write: 12.5,
        });
    }
    if m.contains("opus") {
        // Legado (≤4.1): claude-opus-4-1 / claude-opus-4 sem minor ≥5 / opus-3.
        if m.contains("opus-4-1") || m.contains("3-opus") || m.contains("opus-3") {
            return Some(Pricing {
                input: 15.0,
                output: 75.0,
                cache_read: 1.5,
                cache_write: 18.75,
            });
        }
        return Some(Pricing {
            input: 5.0,
            output: 25.0,
            cache_read: 0.50,
            cache_write: 6.25,
        });
    }
    if m.contains("sonnet-5") {
        // Introdutório até 2026-08-31 (platform.claude.com); revisar em set/2026.
        return Some(Pricing {
            input: 2.0,
            output: 10.0,
            cache_read: 0.20,
            cache_write: 2.50,
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
            input: 1.0,
            output: 5.0,
            cache_read: 0.10,
            cache_write: 1.25,
        });
    }

    // --- OpenAI (Codex / gpt-5.x / o-series) — do mais específico p/ o mais geral ---
    // `gpt-5.3-codex` contém "codex" → cai aqui antes do prefixo `gpt-5`.
    if m.contains("codex") {
        return Some(Pricing {
            input: 1.75,
            output: 14.0,
            cache_read: 0.175,
            cache_write: 1.75, // OpenAI: sem custo de escrita de cache → = input
        });
    }
    if m.starts_with("gpt-5.5") {
        return Some(Pricing {
            input: 5.0,
            output: 30.0,
            cache_read: 0.50,
            cache_write: 5.0,
        });
    }
    if m.starts_with("gpt-5") {
        return Some(Pricing {
            input: 1.25,
            output: 10.0,
            cache_read: 0.125,
            cache_write: 1.25,
        });
    }
    if m.starts_with("o4") {
        // Proxy: o4-mini (não há "o4" standalone público). confidence baixa.
        return Some(Pricing {
            input: 1.10,
            output: 4.40,
            cache_read: 0.275,
            cache_write: 1.10,
        });
    }

    None
}

/// Modelos desconhecidos já avisados nesta execução (warn-once por modelo).
static WARNED_UNKNOWN: Mutex<Option<HashSet<String>>> = Mutex::new(None);

/// Custo em USD do record. `None` se modelo ausente/desconhecido (mostra tokens sem $).
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
            session_id: None,
            project: None,
        }
    }

    #[test]
    fn pricing_matches_by_family_prefix() {
        assert!(pricing_for("claude-opus-4-8").is_some());
        assert!(pricing_for("claude-sonnet-4-6").is_some());
        assert!(pricing_for("claude-haiku-4-5").is_some());
        assert!(pricing_for("gpt-5.5").is_some());
        assert!(pricing_for("gpt-5.3-codex").is_some());
        assert!(pricing_for("o4-mini").is_some());
    }

    #[test]
    fn verified_prices_2026_06_20() {
        // Anthropic (confirmado vs platform.claude.com, cross-check adversarial).
        let opus = pricing_for("claude-opus-4-8").unwrap();
        assert_eq!((opus.input, opus.output), (5.0, 25.0));
        let sonnet = pricing_for("claude-sonnet-4-6").unwrap();
        assert_eq!((sonnet.input, sonnet.output), (3.0, 15.0));
        let haiku = pricing_for("claude-haiku-4-5").unwrap();
        assert_eq!((haiku.input, haiku.output), (1.0, 5.0));
        // OpenAI: gpt-5.5 ($5/$30) distinto de codex ($1.75/$14, gpt-5.3-codex).
        let gpt55 = pricing_for("gpt-5.5").unwrap();
        assert_eq!((gpt55.input, gpt55.output), (5.0, 30.0));
        let codex = pricing_for("gpt-5.3-codex").unwrap();
        assert_eq!((codex.input, codex.output), (1.75, 14.0));
        // "codex" tem precedência sobre o prefixo "gpt-5".
        assert_eq!(
            pricing_for("gpt-5.3-codex"),
            pricing_for("some-codex-model")
        );
    }

    #[test]
    fn fable_and_mythos_pricing_2026_07_10() {
        let fable = pricing_for("claude-fable-5").unwrap();
        assert_eq!(
            (
                fable.input,
                fable.output,
                fable.cache_read,
                fable.cache_write
            ),
            (10.0, 50.0, 1.0, 12.5)
        );
        assert_eq!(
            pricing_for("claude-mythos-5"),
            pricing_for("claude-fable-5")
        );
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

    #[test]
    fn real_claude_3_opus_id_gets_legacy_pricing() {
        let o3 = pricing_for("claude-3-opus-20240229").unwrap();
        assert_eq!((o3.input, o3.output), (15.0, 75.0));
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
        // Opus: input 5, output 25 por 1M. 1M input + 1M output = 5 + 25 = 30.
        let c = cost_usd_of(&rec(Some("claude-opus-4-8"), 1_000_000, 1_000_000, 0, 0)).unwrap();
        assert!((c - 30.0).abs() < 1e-9, "got {c}");
        // cache_read mais barato que input.
        let p = pricing_for("claude-opus-4-8").unwrap();
        assert!(p.cache_read < p.input);
    }
}
