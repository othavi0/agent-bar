//! Bucketing temporal de UsageRecords (dado puro — sem render).

use std::collections::BTreeMap;

use time::{Date, OffsetDateTime};

use crate::usage::pricing::cost_usd_of;
use crate::usage::UsageRecord;

// ---------------------------------------------------------------------------
// Day bucketing
// ---------------------------------------------------------------------------

/// Um bucket diario: date, soma de tokens (input+output), soma de cache
/// (cache_read+cache_write, T4 rótulo duplo — alimenta o sufixo "(+X cache)"
/// de `fmt_tokens_dual`; `tokens` continua SÓ input+output), custo opcional
/// em USD.
#[derive(Debug, Clone, PartialEq)]
pub struct DayBucket {
    pub date: Date,
    pub tokens: u64,
    pub cache_tokens: u64,
    pub cost_usd: Option<f64>,
}

/// Acumulador intermediário de um bucket: (tokens io, cache_tokens, custo).
type BucketAcc = (u64, u64, Option<f64>);

/// Agrupa records por dia local (`ts.to_offset(local_offset).date()`) somando
/// tokens e custo. `local_offset` decide a fronteira de "dia" — sem ele o
/// bucket cai na data UTC, que discorda do "hoje" cortado à meia-noite local
/// usado no resto do painel.
/// Records com modelo desconhecido contribuem tokens mas nao custo (igual ao engine).
/// Retorna vec ordenado por data crescente.
pub fn bucket_by_day(records: &[UsageRecord], local_offset: time::UtcOffset) -> Vec<DayBucket> {
    let mut map: BTreeMap<Date, BucketAcc> = BTreeMap::new();

    for rec in records {
        let date = rec.ts.to_offset(local_offset).date();
        let tokens = rec.input + rec.output;
        let cache_tokens = rec.cache_read + rec.cache_write;
        let cost = cost_usd_of(rec);

        let entry = map.entry(date).or_insert((0, 0, None));
        entry.0 += tokens;
        entry.1 += cache_tokens;
        match (cost, entry.2.as_mut()) {
            (Some(c), Some(acc)) => *acc += c,
            (Some(c), None) => entry.2 = Some(c),
            (None, _) => {}
        }
    }

    map.into_iter()
        .map(|(date, (tokens, cache_tokens, cost_usd))| DayBucket {
            date,
            tokens,
            cache_tokens,
            cost_usd,
        })
        .collect()
}

/// Agrupa records por (provider, day local) para o grafico por-provider.
/// Mesma fronteira de dia de `bucket_by_day` (`local_offset`).
/// Retorna BTreeMap<provider_name, Vec<DayBucket>> ordenado por data.
pub fn bucket_by_provider_day(
    records: &[UsageRecord],
    local_offset: time::UtcOffset,
) -> BTreeMap<String, Vec<DayBucket>> {
    // (provider, date) -> (tokens, cache_tokens, cost)
    let mut map: BTreeMap<(String, Date), BucketAcc> = BTreeMap::new();

    for rec in records {
        let date = rec.ts.to_offset(local_offset).date();
        let tokens = rec.input + rec.output;
        let cache_tokens = rec.cache_read + rec.cache_write;
        let cost = cost_usd_of(rec);
        let key = (rec.provider.clone(), date);

        let entry = map.entry(key).or_insert((0, 0, None));
        entry.0 += tokens;
        entry.1 += cache_tokens;
        match (cost, entry.2.as_mut()) {
            (Some(c), Some(acc)) => *acc += c,
            (Some(c), None) => entry.2 = Some(c),
            (None, _) => {}
        }
    }

    // Reorganiza por provider
    let mut by_provider: BTreeMap<String, BTreeMap<Date, BucketAcc>> = BTreeMap::new();
    for ((provider, date), (tokens, cache_tokens, cost)) in map {
        by_provider
            .entry(provider)
            .or_default()
            .insert(date, (tokens, cache_tokens, cost));
    }

    by_provider
        .into_iter()
        .map(|(provider, date_map)| {
            let buckets = date_map
                .into_iter()
                .map(|(date, (tokens, cache_tokens, cost_usd))| DayBucket {
                    date,
                    tokens,
                    cache_tokens,
                    cost_usd,
                })
                .collect();
            (provider, buckets)
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Hour bucketing
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub struct HourBucket {
    pub hour_start: OffsetDateTime,
    pub tokens: u64,
}

fn floor_to_hour(t: OffsetDateTime) -> OffsetDateTime {
    t.replace_minute(0)
        .and_then(|t| t.replace_second(0))
        .and_then(|t| t.replace_nanosecond(0))
        .unwrap_or(t)
}

fn record_tokens(r: &UsageRecord) -> u64 {
    r.input + r.output + r.cache_read + r.cache_write
}

/// Agrupa records em `hours` buckets horarios terminando na hora de `now`
/// (janela [now - hours + 1h, now], truncada em horas cheias).
/// Sempre devolve exatamente `hours` buckets, do mais antigo ao mais novo,
/// com zeros preenchidos onde nao ha records.
pub fn bucket_by_hour(
    records: &[UsageRecord],
    now: OffsetDateTime,
    hours: usize,
) -> Vec<HourBucket> {
    let end_hour = floor_to_hour(now);
    let mut buckets: Vec<HourBucket> = (0..hours)
        .map(|i| HourBucket {
            hour_start: end_hour - time::Duration::hours((hours - 1 - i) as i64),
            tokens: 0,
        })
        .collect();
    let start = match buckets.first() {
        Some(b) => b.hour_start,
        None => return buckets,
    };
    for r in records {
        if r.ts < start || r.ts >= end_hour + time::Duration::hours(1) {
            continue;
        }
        let idx = (floor_to_hour(r.ts) - start).whole_hours() as usize;
        if let Some(b) = buckets.get_mut(idx) {
            b.tokens += record_tokens(r);
        }
    }
    buckets
}

/// Serie de 24 pontos horarios (tokens) para um provider especifico, ate `now`.
/// Usada pelo sparkline "tokens/h 24h" da aba de resumo.
pub fn provider_series_24h(
    records: &[UsageRecord],
    provider: &str,
    now: OffsetDateTime,
) -> Vec<u64> {
    let filtered: Vec<UsageRecord> = records
        .iter()
        .filter(|r| r.provider == provider)
        .cloned()
        .collect();
    bucket_by_hour(&filtered, now, 24)
        .into_iter()
        .map(|b| b.tokens)
        .collect()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::usage::UsageRecord;
    use time::macros::datetime;

    fn rec(provider: &str, ts: time::OffsetDateTime, tokens: u64) -> UsageRecord {
        UsageRecord {
            provider: provider.into(),
            model: Some("m".into()),
            input: tokens,
            output: 0,
            cache_read: 0,
            cache_write: 0,
            cache_write_1h: 0,
            fast: false,
            geo_us: false,
            ts,
        }
    }

    #[test]
    fn bucket_by_hour_fills_gaps_with_zero() {
        let now = datetime!(2026-07-01 18:30:00 UTC);
        let records = vec![
            rec("claude", datetime!(2026-07-01 18:10:00 UTC), 100), // hora atual
            rec("claude", datetime!(2026-07-01 16:59:00 UTC), 40),  // 2h atras
            rec("claude", datetime!(2026-06-30 18:45:00 UTC), 7),   // fora da janela de 3h
        ];
        let buckets = bucket_by_hour(&records, now, 3);
        assert_eq!(buckets.len(), 3);
        assert_eq!(buckets[0].hour_start, datetime!(2026-07-01 16:00:00 UTC));
        assert_eq!(buckets[0].tokens, 40);
        assert_eq!(buckets[1].tokens, 0); // 17h vazia
        assert_eq!(buckets[2].tokens, 100);
    }

    #[test]
    fn provider_series_24h_has_24_points_and_filters_provider() {
        let now = datetime!(2026-07-01 18:30:00 UTC);
        let records = vec![
            rec("claude", datetime!(2026-07-01 18:00:01 UTC), 5),
            rec("codex", datetime!(2026-07-01 18:00:01 UTC), 999),
        ];
        let series = provider_series_24h(&records, "claude", now);
        assert_eq!(series.len(), 24);
        assert_eq!(series[23], 5);
        assert!(series[..23].iter().all(|&t| t == 0));
    }

    #[test]
    fn bucket_by_hour_sums_all_token_kinds() {
        let now = datetime!(2026-07-01 10:30:00 UTC);
        let mut r = rec("claude", datetime!(2026-07-01 10:05:00 UTC), 1);
        r.output = 2;
        r.cache_read = 3;
        r.cache_write = 4;
        let buckets = bucket_by_hour(&[r], now, 1);
        assert_eq!(buckets[0].tokens, 10);
    }
}

#[cfg(test)]
mod day_bucket_tests {
    use super::*;
    use time::macros::date;

    fn rec(
        provider: &str,
        model: Option<&str>,
        ts_str: &str,
        input: u64,
        output: u64,
    ) -> UsageRecord {
        // Parseia ISO timestamp simples: "2026-06-17T10:00:00Z"
        let ts =
            time::OffsetDateTime::parse(ts_str, &time::format_description::well_known::Rfc3339)
                .expect("timestamp invalido");
        UsageRecord {
            provider: provider.to_string(),
            model: model.map(|s| s.to_string()),
            input,
            output,
            cache_read: 0,
            cache_write: 0,
            cache_write_1h: 0,
            fast: false,
            geo_us: false,
            ts,
        }
    }

    #[test]
    fn bucket_by_day_empty_input() {
        let result = bucket_by_day(&[], time::UtcOffset::UTC);
        assert!(result.is_empty());
    }

    #[test]
    fn bucket_by_day_single_record() {
        let records = vec![rec(
            "claude",
            Some("claude-sonnet-4-6"),
            "2026-06-17T10:00:00Z",
            1000,
            500,
        )];
        let buckets = bucket_by_day(&records, time::UtcOffset::UTC);
        assert_eq!(buckets.len(), 1);
        assert_eq!(buckets[0].date, date!(2026 - 06 - 17));
        assert_eq!(buckets[0].tokens, 1500);
        assert!(buckets[0].cost_usd.is_some());
    }

    #[test]
    fn bucket_by_day_three_days_correct_sums() {
        let records = vec![
            // Dia 17: 2 records de claude-sonnet → somados
            rec(
                "claude",
                Some("claude-sonnet-4-6"),
                "2026-06-17T08:00:00Z",
                1000,
                200,
            ),
            rec(
                "claude",
                Some("claude-sonnet-4-6"),
                "2026-06-17T14:00:00Z",
                500,
                100,
            ),
            // Dia 18: 1 record de codex
            rec("codex", Some("gpt-5.5"), "2026-06-18T10:00:00Z", 2000, 300),
            // Dia 19: 1 record sem modelo (sem custo)
            rec("claude", None, "2026-06-19T09:00:00Z", 800, 200),
        ];

        let buckets = bucket_by_day(&records, time::UtcOffset::UTC);

        // Deve ter 3 buckets (um por data unica)
        assert_eq!(
            buckets.len(),
            3,
            "esperado 3 buckets, obtido {}",
            buckets.len()
        );

        // Ordenados por data crescente
        assert_eq!(buckets[0].date, date!(2026 - 06 - 17));
        assert_eq!(buckets[1].date, date!(2026 - 06 - 18));
        assert_eq!(buckets[2].date, date!(2026 - 06 - 19));

        // Dia 17: tokens = (1000+200) + (500+100) = 1800
        assert_eq!(buckets[0].tokens, 1800, "dia 17 tokens incorretos");
        assert!(
            buckets[0].cost_usd.is_some(),
            "dia 17 deve ter custo (sonnet conhecido)"
        );

        // Dia 18: tokens = 2000+300 = 2300
        assert_eq!(buckets[1].tokens, 2300, "dia 18 tokens incorretos");
        assert!(
            buckets[1].cost_usd.is_some(),
            "dia 18 deve ter custo (gpt-5 conhecido)"
        );

        // Dia 19: tokens = 800+200 = 1000, custo = None (modelo None)
        assert_eq!(buckets[2].tokens, 1000, "dia 19 tokens incorretos");
        assert!(
            buckets[2].cost_usd.is_none(),
            "dia 19 nao deve ter custo (modelo None)"
        );
    }

    #[test]
    fn bucket_by_day_same_day_different_providers_merged() {
        // Records de providers diferentes no mesmo dia → somados no bucket total
        let records = vec![
            rec(
                "claude",
                Some("claude-sonnet-4-6"),
                "2026-06-17T08:00:00Z",
                1000,
                0,
            ),
            rec("codex", Some("gpt-5.5"), "2026-06-17T12:00:00Z", 500, 0),
        ];
        let buckets = bucket_by_day(&records, time::UtcOffset::UTC);
        assert_eq!(buckets.len(), 1);
        assert_eq!(
            buckets[0].tokens, 1500,
            "tokens de providers diferentes devem somar"
        );
    }

    #[test]
    fn bucket_by_day_unknown_model_contributes_tokens_not_cost() {
        let records = vec![rec("claude", None, "2026-06-17T10:00:00Z", 1000, 200)];
        let buckets = bucket_by_day(&records, time::UtcOffset::UTC);
        assert_eq!(buckets.len(), 1);
        assert_eq!(buckets[0].tokens, 1200);
        assert!(
            buckets[0].cost_usd.is_none(),
            "modelo None nao deve gerar custo"
        );
    }

    /// `cache_tokens` (T4) soma cache_read+cache_write num campo SEPARADO de
    /// `tokens` (que continua só input+output) — a distinção é o ponto: o
    /// rótulo duplo em `render/history.rs` precisa dos dois valores, nunca
    /// misturados.
    #[test]
    fn bucket_by_day_accumulates_cache_tokens_separately_from_tokens() {
        let mut r1 = rec("claude", Some("claude-sonnet-4-6"), "2026-06-17T08:00:00Z", 1000, 200);
        r1.cache_read = 500;
        r1.cache_write = 100;
        let mut r2 = rec("claude", Some("claude-sonnet-4-6"), "2026-06-17T14:00:00Z", 300, 50);
        r2.cache_read = 200;
        let buckets = bucket_by_day(&[r1, r2], time::UtcOffset::UTC);
        assert_eq!(buckets.len(), 1);
        assert_eq!(buckets[0].tokens, 1550, "tokens deve continuar so input+output");
        assert_eq!(buckets[0].cache_tokens, 800, "cache_tokens = 500+100+200");
    }

    #[test]
    fn bucket_by_provider_day_accumulates_cache_tokens_per_provider() {
        let mut r1 = rec("claude", Some("claude-sonnet-4-6"), "2026-06-17T08:00:00Z", 1000, 0);
        r1.cache_read = 400;
        let mut r2 = rec("codex", Some("gpt-5.5"), "2026-06-17T09:00:00Z", 2000, 0);
        r2.cache_write = 900;
        let by_provider = bucket_by_provider_day(&[r1, r2], time::UtcOffset::UTC);
        assert_eq!(by_provider["claude"][0].cache_tokens, 400);
        assert_eq!(by_provider["codex"][0].cache_tokens, 900);
    }
}

/// T5: fronteira de dia usa o offset local, não UTC — sem isso a tabela de
/// History discorda do "hoje" cortado à meia-noite local do resto do painel.
#[cfg(test)]
mod local_day_boundary_tests {
    use super::*;
    use time::macros::datetime;

    fn rec(provider: &str, model: &str, ts: time::OffsetDateTime, tokens: u64) -> UsageRecord {
        UsageRecord {
            provider: provider.to_string(),
            model: Some(model.to_string()),
            input: tokens,
            output: 0,
            cache_read: 0,
            cache_write: 0,
            cache_write_1h: 0,
            fast: false,
            geo_us: false,
            ts,
        }
    }

    #[test]
    fn buckets_use_local_day_not_utc() {
        // 02:00 UTC com offset -3 = 23:00 do dia ANTERIOR local.
        let r = rec(
            "claude",
            "claude-fable-5",
            datetime!(2026-07-03 02:00 UTC),
            100,
        );
        let off = time::UtcOffset::from_hms(-3, 0, 0).unwrap();
        let buckets = bucket_by_day(&[r], off);
        assert_eq!(buckets[0].date, time::macros::date!(2026 - 07 - 02));
        // Offset zero preserva o comportamento anterior (snapshots intactos).
        let r2 = rec(
            "claude",
            "claude-fable-5",
            datetime!(2026-07-03 02:00 UTC),
            100,
        );
        let utc = bucket_by_day(&[r2], time::UtcOffset::UTC);
        assert_eq!(utc[0].date, time::macros::date!(2026 - 07 - 03));
    }
}
