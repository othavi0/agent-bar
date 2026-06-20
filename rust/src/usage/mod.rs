//! Engine de usage/custo: lê session logs locais → tokens → custo (US$/R$).
//! Subsistema PURO (sem TUI/ratatui). Ver spec §4b.

pub mod amp;
pub mod cache;
pub mod claude;
pub mod codex;
pub mod pricing;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use time::OffsetDateTime;

use crate::usage::amp::{amp_dollar_usage, AmpDollars};
use crate::usage::cache::UsageCache;
use crate::usage::claude::parse_claude_lines;
use crate::usage::codex::parse_codex_lines;
use crate::usage::pricing::cost_usd_of;

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

/// Custo em USD e BRL.
#[derive(Debug, Clone, PartialEq)]
pub struct Cost {
    pub usd: f64,
    pub brl: f64,
}

/// Uso agregado por modelo, dentro de um provider.
/// `cost` é `None` quando o modelo não tem preço conhecido.
#[derive(Debug, Clone, PartialEq)]
pub struct ModelUsage {
    pub model: String,
    pub input: u64,
    pub output: u64,
    pub cache_read: u64,
    pub cache_write: u64,
    /// `None` se modelo desconhecido (sem preço). Custo parcial (só modelos
    /// com preço) é melhor que nenhum; modelos sem preço contribuem só tokens.
    pub cost: Option<Cost>,
}

/// Uso agregado por provider.
/// `cost` = soma dos `ModelUsage.cost` conhecidos; `None` se nenhum conhecido.
/// `amp_dollars` presente apenas no provider "amp" (sem records de token).
#[derive(Debug, Clone, PartialEq)]
pub struct ProviderUsage {
    pub provider: String,
    pub total_input: u64,
    pub total_output: u64,
    pub total_cache_read: u64,
    pub total_cache_write: u64,
    pub cost: Option<Cost>,
    pub by_model: Vec<ModelUsage>,
    pub amp_dollars: Option<AmpDollars>,
}

/// Resumo global de uso: todos os providers + custo total + taxa de câmbio usada.
#[derive(Debug, Clone, PartialEq)]
pub struct UsageSummary {
    pub providers: Vec<ProviderUsage>,
    pub total_cost: Cost,
    pub fx_rate: f64,
}

/// Opções para `aggregate` / `records`.
pub struct AggregateOptions<'a> {
    pub claude_dir: &'a Path,
    pub codex_dir: &'a Path,
    pub fx_rate: f64,
    /// Meta do provider Amp (já parseado por `providers::amp`). Se `Some`,
    /// adiciona um `ProviderUsage { provider:"amp", amp_dollars: Some(...) }`.
    pub amp_meta: Option<&'a BTreeMap<String, String>>,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Coleta todos os `.jsonl` recursivamente sob `dir`.
/// Ignora erros de IO (dir inexistente → vec vazio).
fn collect_jsonl(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    collect_jsonl_inner(dir, &mut out);
    out
}

fn collect_jsonl_inner(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_jsonl_inner(&path, out);
        } else if path.extension().map(|e| e == "jsonl").unwrap_or(false) {
            out.push(path);
        }
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Retorna todos os `UsageRecord` de Claude + Codex, ordenados por `ts` (asc).
/// Consumido pela aba History para montagem de tendência temporal; o bucketing
/// por dia/janela é responsabilidade do consumidor.
pub fn records(opts: AggregateOptions) -> Vec<UsageRecord> {
    let mut cache = UsageCache::new();
    let mut all: Vec<UsageRecord> = Vec::new();

    for path in collect_jsonl(opts.claude_dir) {
        let recs = cache.cached_or_parse(&path, |content| parse_claude_lines(content.lines()));
        all.extend(recs);
    }

    for path in collect_jsonl(opts.codex_dir) {
        let recs = cache.cached_or_parse(&path, |content| parse_codex_lines(content.lines()));
        all.extend(recs);
    }

    all.sort_by_key(|r| r.ts);
    all
}

/// Como [`records`], mas só os com `ts >= cutoff` (ordenados por `ts` asc).
/// Atalho de ergonomia pra a aba History fazer janelas (hoje / 5h / 7d) sem
/// reimplementar o filtro temporal no consumidor.
pub fn records_since(opts: AggregateOptions, cutoff: OffsetDateTime) -> Vec<UsageRecord> {
    records(opts)
        .into_iter()
        .filter(|r| r.ts >= cutoff)
        .collect()
}

/// Agrega TODOS os records em `UsageSummary` (sem filtragem temporal — agrega o
/// histórico inteiro; pra visões por janela, filtre antes com [`records_since`]).
/// - Records agrupados por `(provider, model_or_"unknown")`.
/// - `cost` por modelo = soma de `cost_usd_of` dos records (modelo desconhecido → `None`).
/// - `ProviderUsage.cost` = soma dos `ModelUsage.cost` conhecidos; `None` se nenhum.
/// - `brl = usd * fx_rate`.
/// - Se `amp_meta` Some → adiciona `ProviderUsage { provider:"amp", amp_dollars: Some(...) }`.
pub fn aggregate(opts: AggregateOptions) -> UsageSummary {
    let fx_rate = opts.fx_rate;
    let amp_meta = opts.amp_meta;

    let all = records(AggregateOptions {
        claude_dir: opts.claude_dir,
        codex_dir: opts.codex_dir,
        fx_rate,
        amp_meta: None, // amp não tem records de token
    });

    // Agrupa por (provider, model_key).
    // BTreeMap garante ordem determinística nos testes.
    use std::collections::BTreeMap as Map;

    // key = (provider, model_label); value = (tokens acumulados, custo_usd acumulado ou None)
    type GroupKey = (String, String);
    struct Group {
        input: u64,
        output: u64,
        cache_read: u64,
        cache_write: u64,
        cost_usd: Option<f64>, // None = modelo desconhecido
    }

    let mut groups: Map<GroupKey, Group> = Map::new();

    for rec in &all {
        let model_label = rec.model.clone().unwrap_or_else(|| "unknown".to_string());
        let key = (rec.provider.clone(), model_label);
        let cost_delta = cost_usd_of(rec); // None se modelo desconhecido

        let g = groups.entry(key).or_insert(Group {
            input: 0,
            output: 0,
            cache_read: 0,
            cache_write: 0,
            cost_usd: None,
        });
        g.input += rec.input;
        g.output += rec.output;
        g.cache_read += rec.cache_read;
        g.cache_write += rec.cache_write;
        // Acumula custo só se o modelo é conhecido.
        match (cost_delta, g.cost_usd.as_mut()) {
            (Some(d), Some(acc)) => *acc += d,
            (Some(d), None) => {
                // Primeira contribuição conhecida para este grupo.
                g.cost_usd = Some(d);
            }
            // modelo desconhecido ou grupo ainda sem custo inicial com None: nada.
            (None, _) => {}
        }
    }

    // Agrupa os grupos por provider.
    type ProviderBucket = Vec<((String, String), Group)>;
    let mut by_provider: Map<String, ProviderBucket> = Map::new();
    for (key, g) in groups {
        by_provider.entry(key.0.clone()).or_default().push((key, g));
    }

    let mut providers: Vec<ProviderUsage> = Vec::new();
    let mut total_usd = 0.0_f64;

    for (provider, model_groups) in by_provider {
        let mut by_model: Vec<ModelUsage> = Vec::new();
        let mut provider_usd: Option<f64> = None;
        let mut provider_input = 0_u64;
        let mut provider_output = 0_u64;
        let mut provider_cache_read = 0_u64;
        let mut provider_cache_write = 0_u64;

        for ((_prov, model_label), g) in model_groups {
            let model_cost = g.cost_usd.map(|usd| Cost {
                usd,
                brl: usd * fx_rate,
            });

            // Soma ao custo do provider só se este modelo tem custo conhecido.
            if let Some(usd) = g.cost_usd {
                match provider_usd.as_mut() {
                    Some(acc) => *acc += usd,
                    None => provider_usd = Some(usd),
                }
            }

            provider_input += g.input;
            provider_output += g.output;
            provider_cache_read += g.cache_read;
            provider_cache_write += g.cache_write;

            by_model.push(ModelUsage {
                model: model_label,
                input: g.input,
                output: g.output,
                cache_read: g.cache_read,
                cache_write: g.cache_write,
                cost: model_cost,
            });
        }

        let provider_cost = provider_usd.map(|usd| {
            total_usd += usd;
            Cost {
                usd,
                brl: usd * fx_rate,
            }
        });

        providers.push(ProviderUsage {
            provider,
            total_input: provider_input,
            total_output: provider_output,
            total_cache_read: provider_cache_read,
            total_cache_write: provider_cache_write,
            cost: provider_cost,
            by_model,
            amp_dollars: None,
        });
    }

    // Amp: sem records de token, apenas os dados monetários do meta.
    if let Some(meta) = amp_meta {
        let dollars = amp_dollar_usage(meta);
        providers.push(ProviderUsage {
            provider: "amp".to_string(),
            total_input: 0,
            total_output: 0,
            total_cache_read: 0,
            total_cache_write: 0,
            cost: None,
            by_model: vec![],
            amp_dollars: Some(dollars),
        });
    }

    UsageSummary {
        providers,
        total_cost: Cost {
            usd: total_usd,
            brl: total_usd * fx_rate,
        },
        fx_rate,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

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

    // --- Tests from the brief (Step 1) ---

    #[test]
    fn aggregate_sums_tokens_and_cost_per_provider() {
        let claude = tempdir().unwrap();
        let codex = tempdir().unwrap();
        write(
            claude.path(),
            "proj/sess.jsonl",
            r#"{"type":"assistant","timestamp":"2026-06-19T11:00:00Z","message":{"model":"claude-opus-4-8","usage":{"input_tokens":1000000,"output_tokens":1000000,"cache_creation_input_tokens":0,"cache_read_input_tokens":0}}}"#,
        );
        write(
            codex.path(),
            "2026/06/19/rollout-x.jsonl",
            "{\"type\":\"session_meta\",\"model\":\"gpt-5.5\"}\n{\"type\":\"event_msg\",\"timestamp\":\"2026-06-19T11:05:00Z\",\"payload\":{\"type\":\"token_count\",\"info\":{\"last_token_usage\":{\"input_tokens\":1000000,\"cached_input_tokens\":0,\"output_tokens\":0,\"reasoning_output_tokens\":0,\"total_tokens\":1000000}}}}",
        );

        let s = aggregate(AggregateOptions {
            claude_dir: claude.path(),
            codex_dir: codex.path(),
            fx_rate: 5.0,
            amp_meta: None,
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
        write(
            claude.path(),
            "p/s.jsonl",
            r#"{"type":"assistant","timestamp":"2026-06-19T11:00:00Z","message":{"model":"mystery-model","usage":{"input_tokens":500,"output_tokens":500}}}"#,
        );
        let s = aggregate(AggregateOptions {
            claude_dir: claude.path(),
            codex_dir: codex.path(),
            fx_rate: 5.0,
            amp_meta: None,
        });
        let cl = s.providers.iter().find(|p| p.provider == "claude").unwrap();
        assert_eq!(cl.total_input, 500);
        // modelo desconhecido → sem custo
        let m = cl
            .by_model
            .iter()
            .find(|m| m.model == "mystery-model")
            .unwrap();
        assert!(m.cost.is_none());
    }

    // --- Test: records ordered by ts ---

    #[test]
    fn records_are_sorted_by_ts_ascending() {
        let claude = tempdir().unwrap();
        let codex = tempdir().unwrap();

        // Arquivo 1: timestamp mais recente (11:30)
        write(
            claude.path(),
            "a/later.jsonl",
            r#"{"type":"assistant","timestamp":"2026-06-19T11:30:00Z","message":{"model":"claude-sonnet-4-6","usage":{"input_tokens":200,"output_tokens":100,"cache_creation_input_tokens":0,"cache_read_input_tokens":0}}}"#,
        );
        // Arquivo 2: timestamp mais antigo (10:00)
        write(
            claude.path(),
            "b/earlier.jsonl",
            r#"{"type":"assistant","timestamp":"2026-06-19T10:00:00Z","message":{"model":"claude-sonnet-4-6","usage":{"input_tokens":100,"output_tokens":50,"cache_creation_input_tokens":0,"cache_read_input_tokens":0}}}"#,
        );

        let recs = records(AggregateOptions {
            claude_dir: claude.path(),
            codex_dir: codex.path(),
            fx_rate: 5.0,
            amp_meta: None,
        });

        assert_eq!(recs.len(), 2);
        assert!(
            recs[0].ts <= recs[1].ts,
            "records should be sorted ascending by ts: {:?} > {:?}",
            recs[0].ts,
            recs[1].ts
        );
        // O primeiro deve ser o de 10:00
        assert_eq!(recs[0].input, 100);
        // O segundo deve ser o de 11:30
        assert_eq!(recs[1].input, 200);
    }

    #[test]
    fn records_since_filters_by_cutoff() {
        use time::macros::datetime;
        let claude = tempdir().unwrap();
        let codex = tempdir().unwrap();
        write(
            claude.path(),
            "a/later.jsonl",
            r#"{"type":"assistant","timestamp":"2026-06-19T11:30:00Z","message":{"model":"claude-sonnet-4-6","usage":{"input_tokens":200,"output_tokens":100}}}"#,
        );
        write(
            claude.path(),
            "b/earlier.jsonl",
            r#"{"type":"assistant","timestamp":"2026-06-19T10:00:00Z","message":{"model":"claude-sonnet-4-6","usage":{"input_tokens":100,"output_tokens":50}}}"#,
        );

        let cutoff = datetime!(2026-06-19 11:00 UTC);
        let recs = records_since(
            AggregateOptions {
                claude_dir: claude.path(),
                codex_dir: codex.path(),
                fx_rate: 5.0,
                amp_meta: None,
            },
            cutoff,
        );
        assert_eq!(recs.len(), 1); // só o de 11:30 (>= cutoff)
        assert_eq!(recs[0].input, 200);
    }

    // --- Test: amp_meta adds amp ProviderUsage ---

    #[test]
    fn aggregate_with_amp_meta_adds_amp_provider() {
        let claude = tempdir().unwrap();
        let codex = tempdir().unwrap();

        let mut meta = BTreeMap::new();
        meta.insert("freeRemaining".to_string(), "$3.5".to_string());
        meta.insert("freeTotal".to_string(), "$5".to_string());

        let s = aggregate(AggregateOptions {
            claude_dir: claude.path(),
            codex_dir: codex.path(),
            fx_rate: 5.0,
            amp_meta: Some(&meta),
        });

        let amp = s.providers.iter().find(|p| p.provider == "amp").unwrap();
        assert_eq!(amp.total_input, 0);
        assert!(amp.cost.is_none());
        assert!(amp.by_model.is_empty());
        let dollars = amp.amp_dollars.as_ref().unwrap();
        assert_eq!(dollars.remaining, Some(3.5));
        assert_eq!(dollars.total, Some(5.0));
    }

    // --- Test: empty dirs yield empty summary ---

    #[test]
    fn empty_dirs_yield_empty_summary() {
        let claude = tempdir().unwrap();
        let codex = tempdir().unwrap();
        let s = aggregate(AggregateOptions {
            claude_dir: claude.path(),
            codex_dir: codex.path(),
            fx_rate: 5.0,
            amp_meta: None,
        });
        assert!(s.providers.is_empty());
        assert_eq!(s.total_cost.usd, 0.0);
        assert_eq!(s.fx_rate, 5.0);
    }

    // --- Test: nonexistent dirs don't panic ---

    #[test]
    fn nonexistent_dirs_return_empty() {
        let s = aggregate(AggregateOptions {
            claude_dir: Path::new("/nonexistent/claude"),
            codex_dir: Path::new("/nonexistent/codex"),
            fx_rate: 5.0,
            amp_meta: None,
        });
        assert!(s.providers.is_empty());
    }
}
