//! Bucketing temporal de UsageRecords (dado puro — sem render).

use std::collections::BTreeMap;

use time::{Date, OffsetDateTime};

use crate::usage::model_names::{display_model_name, series_slot_for_model};
use crate::usage::pricing::cost_usd_of;
use crate::usage::UsageRecord;

// ---------------------------------------------------------------------------
// Day bucketing
// ---------------------------------------------------------------------------

/// Um bucket diario: date, soma de tokens (input+output), custo opcional em USD.
#[derive(Debug, Clone, PartialEq)]
pub struct DayBucket {
    pub date: Date,
    pub tokens: u64,
    pub cost_usd: Option<f64>,
}

/// Agrupa records por dia (ts.date()) somando tokens e custo.
/// Records com modelo desconhecido contribuem tokens mas nao custo (igual ao engine).
/// Retorna vec ordenado por data crescente.
pub fn bucket_by_day(records: &[UsageRecord]) -> Vec<DayBucket> {
    let mut map: BTreeMap<Date, (u64, Option<f64>)> = BTreeMap::new();

    for rec in records {
        let date = rec.ts.date();
        let tokens = rec.input + rec.output;
        let cost = cost_usd_of(rec);

        let entry = map.entry(date).or_insert((0, None));
        entry.0 += tokens;
        match (cost, entry.1.as_mut()) {
            (Some(c), Some(acc)) => *acc += c,
            (Some(c), None) => entry.1 = Some(c),
            (None, _) => {}
        }
    }

    map.into_iter()
        .map(|(date, (tokens, cost_usd))| DayBucket {
            date,
            tokens,
            cost_usd,
        })
        .collect()
}

/// Agrupa records por (provider, day) para o grafico por-provider.
/// Retorna BTreeMap<provider_name, Vec<DayBucket>> ordenado por data.
pub fn bucket_by_provider_day(records: &[UsageRecord]) -> BTreeMap<String, Vec<DayBucket>> {
    // (provider, date) -> (tokens, cost)
    let mut map: BTreeMap<(String, Date), (u64, Option<f64>)> = BTreeMap::new();

    for rec in records {
        let date = rec.ts.date();
        let tokens = rec.input + rec.output;
        let cost = cost_usd_of(rec);
        let key = (rec.provider.clone(), date);

        let entry = map.entry(key).or_insert((0, None));
        entry.0 += tokens;
        match (cost, entry.1.as_mut()) {
            (Some(c), Some(acc)) => *acc += c,
            (Some(c), None) => entry.1 = Some(c),
            (None, _) => {}
        }
    }

    // Reorganiza por provider
    let mut by_provider: BTreeMap<String, BTreeMap<Date, (u64, Option<f64>)>> = BTreeMap::new();
    for ((provider, date), (tokens, cost)) in map {
        by_provider
            .entry(provider)
            .or_default()
            .insert(date, (tokens, cost));
    }

    by_provider
        .into_iter()
        .map(|(provider, date_map)| {
            let buckets = date_map
                .into_iter()
                .map(|(date, (tokens, cost_usd))| DayBucket {
                    date,
                    tokens,
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

fn floor_to_hour(t: OffsetDateTime) -> OffsetDateTime {
    t.replace_minute(0)
        .and_then(|t| t.replace_second(0))
        .and_then(|t| t.replace_nanosecond(0))
        .unwrap_or(t)
}

fn record_tokens(r: &UsageRecord) -> u64 {
    r.input + r.output + r.cache_read + r.cache_write
}

#[derive(Debug, Clone, PartialEq)]
pub struct ModelHourSeries {
    /// Nome já tratado pra display (ex. "Fable 5").
    pub label: String,
    /// Slot de cor 0..=5 (theme::ColorToken::Series1..6).
    pub slot: u8,
    /// Exatamente `hours` pontos, mais antigo → mais novo.
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
        let raw = r.model.as_deref();
        let (label, slot) = match raw {
            Some(m) => (display_model_name(m), series_slot_for_model(m)),
            None => ("—".to_string(), 5),
        };
        let idx = (floor_to_hour(r.ts) - start).whole_hours() as usize;
        let entry = map
            .entry(label)
            .or_insert_with(|| (slot, vec![0; hours]));
        if let Some(b) = entry.1.get_mut(idx) {
            *b += record_tokens(r);
        }
    }

    let mut out: Vec<ModelHourSeries> = map
        .into_iter()
        .map(|(label, (slot, tokens))| {
            let total = tokens.iter().sum();
            ModelHourSeries {
                label,
                slot,
                tokens,
                total,
            }
        })
        .filter(|s| s.total > 0)
        .collect();
    out.sort_by(|a, b| a.slot.cmp(&b.slot).then(b.total.cmp(&a.total)));
    out
}

// ---------------------------------------------------------------------------
// Session bucketing
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub struct SessionAgg {
    pub session_id: String,
    pub start: OffsetDateTime,
    pub project: Option<String>,
    /// Display name do modelo com mais tokens na sessão (ex. "Fable 5").
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
        let key = (
            local.date(),
            r.session_id.clone().unwrap_or_else(|| "—".into()),
        );
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
            .map(|(m, _)| display_model_name(m));
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
            sessions.sort_by_key(|s| std::cmp::Reverse(s.start));
            let tokens = sessions.iter().map(|s| s.tokens).sum();
            let cost_usd = sessions
                .iter()
                .filter_map(|s| s.cost_usd)
                .fold(None, |acc, c| Some(acc.unwrap_or(0.0) + c));
            DaySessions {
                date,
                tokens,
                cost_usd,
                sessions,
            }
        })
        .collect();
    out.reverse(); // BTreeMap é asc; queremos desc por data
    out
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
            ts,
            session_id: None,
            project: None,
        }
    }

    fn mrec(provider: &str, model: &str, ts: time::OffsetDateTime, tokens: u64) -> UsageRecord {
        let mut r = rec(provider, ts, tokens);
        r.model = Some(model.into());
        r
    }

    #[test]
    fn model_hour_series_split_by_model_and_slot_order() {
        let now = datetime!(2026-07-10 12:30:00 UTC);
        let records = vec![
            mrec(
                "claude",
                "claude-opus-4-8",
                datetime!(2026-07-10 12:05:00 UTC),
                50,
            ),
            mrec(
                "claude",
                "claude-fable-5",
                datetime!(2026-07-10 12:10:00 UTC),
                100,
            ),
            mrec(
                "claude",
                "claude-fable-5",
                datetime!(2026-07-10 11:10:00 UTC),
                30,
            ),
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
            mrec(
                "claude",
                "claude-opus-4-8",
                datetime!(2026-07-10 12:05:00 UTC),
                10,
            ),
            mrec(
                "claude",
                "claude-opus-4-8-20260101",
                datetime!(2026-07-10 12:06:00 UTC),
                5,
            ),
            mrec(
                "claude",
                "claude-haiku-4-5",
                datetime!(2026-07-01 12:00:00 UTC),
                7,
            ), // fora da janela
        ];
        let series = bucket_by_model_hour(&records, "claude", now, 2);
        assert_eq!(series.len(), 1); // haiku fora da janela → total 0 → omitido
        assert_eq!(series[0].label, "Opus 4.8");
        assert_eq!(series[0].total, 15);
    }

    #[test]
    fn sessions_by_day_groups_and_orders_desc() {
        use time::UtcOffset;
        let mut a = mrec(
            "claude",
            "claude-fable-5",
            datetime!(2026-07-10 10:00:00 UTC),
            100,
        );
        a.session_id = Some("s1".into());
        a.project = Some("crm".into());
        let mut b = mrec(
            "claude",
            "claude-opus-4-8",
            datetime!(2026-07-10 11:00:00 UTC),
            40,
        );
        b.session_id = Some("s2".into());
        let mut c = mrec(
            "claude",
            "claude-fable-5",
            datetime!(2026-07-09 09:00:00 UTC),
            7,
        );
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
        let mut a = mrec(
            "claude",
            "claude-fable-5",
            datetime!(2026-07-10 10:00:00 UTC),
            100,
        );
        a.session_id = Some("s1".into());
        let mut b = mrec(
            "claude",
            "claude-opus-4-8",
            datetime!(2026-07-10 10:05:00 UTC),
            30,
        );
        b.session_id = Some("s1".into());
        let days = sessions_by_day(&[a, b], UtcOffset::UTC);
        assert_eq!(days[0].sessions[0].dominant_model.as_deref(), Some("Fable 5"));
        assert_eq!(days[0].sessions[0].tokens, 130);
    }

    #[test]
    fn sessions_by_day_uses_local_offset_for_date() {
        // 2026-07-10 01:00 UTC = 2026-07-09 22:00 em UTC-3.
        let offset = time::UtcOffset::from_hms(-3, 0, 0).unwrap();
        let mut a = mrec(
            "claude",
            "claude-fable-5",
            datetime!(2026-07-10 01:00:00 UTC),
            10,
        );
        a.session_id = Some("s1".into());
        let days = sessions_by_day(&[a], offset);
        assert_eq!(days[0].date, time::macros::date!(2026 - 07 - 09));
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
            ts,
            session_id: None,
            project: None,
        }
    }

    #[test]
    fn bucket_by_day_empty_input() {
        let result = bucket_by_day(&[]);
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
        let buckets = bucket_by_day(&records);
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

        let buckets = bucket_by_day(&records);

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
        let buckets = bucket_by_day(&records);
        assert_eq!(buckets.len(), 1);
        assert_eq!(
            buckets[0].tokens, 1500,
            "tokens de providers diferentes devem somar"
        );
    }

    #[test]
    fn bucket_by_day_unknown_model_contributes_tokens_not_cost() {
        let records = vec![rec("claude", None, "2026-06-17T10:00:00Z", 1000, 200)];
        let buckets = bucket_by_day(&records);
        assert_eq!(buckets.len(), 1);
        assert_eq!(buckets[0].tokens, 1200);
        assert!(
            buckets[0].cost_usd.is_none(),
            "modelo None nao deve gerar custo"
        );
    }
}
