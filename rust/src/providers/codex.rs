//! Codex provider. Estende `QuotaSource`. Duas fontes (app-server JSON-RPC +
//! fallback session-log) normalizadas para `CodexRateLimits`. Port fiel de
//! `src/providers/codex.ts`.

use std::collections::BTreeMap;

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use super::iso_from_ms;
use super::types::{
    CodexQuotaExtra, ExtraUsage, ModelWindows, ProviderExtra, ProviderQuota, QuotaWindow,
};
use crate::formatters::shared::{classify_window, normalize_plan, WindowKind};

// ---- Formato interno (snake_case = formato do session-log; é o Raw cacheável) ----

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexWindowRaw {
    pub used_percent: f64,
    pub window_minutes: i64,
    pub resets_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexLimitBucket {
    pub limit_id: String,
    #[serde(default)]
    pub limit_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub primary: Option<CodexWindowRaw>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secondary: Option<CodexWindowRaw>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexCredits {
    pub has_credits: bool,
    pub unlimited: bool,
    pub balance: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CodexRateLimits {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub primary: Option<CodexWindowRaw>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secondary: Option<CodexWindowRaw>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credits: Option<CodexCredits>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub buckets: Option<IndexMap<String, CodexLimitBucket>>,
}

// ---- Helpers de conversão (puros) ----

/// Unix SEGUNDOS → ISO UTC; None se `<= 0`.
fn unix_to_iso(ts: i64) -> Option<String> {
    if ts <= 0 {
        None
    } else {
        Some(iso_from_ms((ts as u64) * 1000))
    }
}

/// CodexWindowRaw → QuotaWindow (remaining = 100 - round(used_percent)).
fn to_quota_window(raw: &CodexWindowRaw) -> QuotaWindow {
    QuotaWindow {
        remaining: 100.0 - raw.used_percent.round(),
        resets_at: unix_to_iso(raw.resets_at),
        window_minutes: Some(raw.window_minutes),
        used: None,
    }
}

/// `limit_name` (não-vazio) ou `limit_id`; `[_-]+`→espaço; titlecase por palavra; vazio→"Codex".
fn format_bucket_label(bucket: &CodexLimitBucket) -> String {
    let raw = bucket
        .limit_name
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(&bucket.limit_id);
    let normalized: String = raw
        .chars()
        .map(|c| if c == '_' || c == '-' { ' ' } else { c })
        .collect();
    let normalized = normalized.trim();
    if normalized.is_empty() {
        return "Codex".to_string();
    }
    normalized
        .split_whitespace()
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

/// Insere `qw` no kind certo de `windows` (fiveHour/sevenDay únicos; resto em other).
fn place_window(windows: &mut ModelWindows, raw: &CodexWindowRaw) {
    let qw = to_quota_window(raw);
    match classify_window(Some(raw.window_minutes)) {
        WindowKind::FiveHour if windows.five_hour.is_none() => windows.five_hour = Some(qw),
        WindowKind::SevenDay if windows.seven_day.is_none() => windows.seven_day = Some(qw),
        _ => windows.other.get_or_insert_with(Vec::new).push(qw),
    }
}

/// Constrói `modelsDetailed` a partir dos buckets (ou fallback legacy primary/secondary).
fn build_model_windows(limits: &CodexRateLimits) -> BTreeMap<String, ModelWindows> {
    let mut models: BTreeMap<String, ModelWindows> = BTreeMap::new();

    if let Some(buckets) = limits.buckets.as_ref().filter(|b| !b.is_empty()) {
        for bucket in buckets.values() {
            let mut windows = ModelWindows::default();
            for raw in [bucket.primary.as_ref(), bucket.secondary.as_ref()]
                .into_iter()
                .flatten()
            {
                place_window(&mut windows, raw);
            }
            // Fallback de mapeamento quando as durações não classificam limpo.
            if windows.five_hour.is_none() {
                if let Some(p) = bucket.primary.as_ref() {
                    windows.five_hour = Some(to_quota_window(p));
                }
            }
            if windows.seven_day.is_none() {
                if let Some(s) = bucket.secondary.as_ref() {
                    windows.seven_day = Some(to_quota_window(s));
                }
            }
            if windows.five_hour.is_none()
                && windows.seven_day.is_none()
                && windows.other.as_ref().map(Vec::is_empty).unwrap_or(true)
            {
                continue;
            }
            let base_name = format_bucket_label(bucket);
            let mut name = base_name.clone();
            let mut suffix = 2;
            while models.contains_key(&name) {
                name = format!("{base_name} ({suffix})");
                suffix += 1;
            }
            models.insert(name, windows);
        }
    }

    // Legacy: só primary/secondary, sem buckets.
    if models.is_empty() && (limits.primary.is_some() || limits.secondary.is_some()) {
        let mut windows = ModelWindows::default();
        for raw in [limits.primary.as_ref(), limits.secondary.as_ref()]
            .into_iter()
            .flatten()
        {
            place_window(&mut windows, raw);
        }
        if windows.five_hour.is_none() {
            if let Some(p) = limits.primary.as_ref() {
                windows.five_hour = Some(to_quota_window(p));
            }
        }
        if windows.seven_day.is_none() {
            if let Some(s) = limits.secondary.as_ref() {
                windows.seven_day = Some(to_quota_window(s));
            }
        }
        models.insert("Codex".to_string(), windows);
    }

    models
}

fn flatten_models(
    models_detailed: &BTreeMap<String, ModelWindows>,
) -> IndexMap<String, QuotaWindow> {
    let mut models: IndexMap<String, QuotaWindow> = IndexMap::new();
    for (name, w) in models_detailed {
        let selected = w
            .five_hour
            .clone()
            .or_else(|| w.seven_day.clone())
            .or_else(|| w.other.as_ref().and_then(|o| o.first().cloned()));
        if let Some(qw) = selected {
            models.insert(name.clone(), qw);
        }
    }
    models
}

fn pick_primary(
    limits: &CodexRateLimits,
    models_detailed: &BTreeMap<String, ModelWindows>,
) -> Option<QuotaWindow> {
    if let Some(p) = limits.primary.as_ref() {
        return Some(to_quota_window(p));
    }
    for m in models_detailed.values() {
        if let Some(fh) = m.five_hour.as_ref() {
            return Some(fh.clone());
        }
    }
    for m in models_detailed.values() {
        if let Some(sd) = m.seven_day.as_ref() {
            return Some(sd.clone());
        }
    }
    None
}

fn pick_secondary(
    limits: &CodexRateLimits,
    models_detailed: &BTreeMap<String, ModelWindows>,
) -> Option<QuotaWindow> {
    if let Some(s) = limits.secondary.as_ref() {
        return Some(to_quota_window(s));
    }
    for m in models_detailed.values() {
        if let Some(sd) = m.seven_day.as_ref() {
            return Some(sd.clone());
        }
    }
    None
}

/// CodexRateLimits → ProviderQuota. `error` embutido se sem janelas usáveis.
pub fn build_codex_quota(limits: &CodexRateLimits, base: ProviderQuota) -> ProviderQuota {
    let models_detailed = build_model_windows(limits);
    let models = flatten_models(&models_detailed);
    let primary = pick_primary(limits, &models_detailed);
    let secondary = pick_secondary(limits, &models_detailed);

    if primary.is_none() && secondary.is_none() && models_detailed.is_empty() {
        return ProviderQuota {
            error: Some(crate::providers::error::CodexError::NoQuotaWindows.to_string()),
            ..base
        };
    }

    // Credits → extraUsage.
    let credits_extra: Option<ExtraUsage> = limits.credits.as_ref().and_then(|c| {
        let balance: f64 = c.balance.parse().unwrap_or(0.0);
        if c.has_credits || balance > 0.0 {
            Some(ExtraUsage {
                enabled: true,
                remaining: if c.unlimited {
                    100.0
                } else {
                    100.0_f64.min(balance.round())
                },
                limit: if c.unlimited { -1.0 } else { 0.0 },
                used: 0.0,
            })
        } else {
            None
        }
    });

    let extra = if !models_detailed.is_empty() || credits_extra.is_some() {
        Some(ProviderExtra::Codex(CodexQuotaExtra {
            models_detailed: if models_detailed.is_empty() {
                None
            } else {
                Some(models_detailed)
            },
            extra_usage: credits_extra,
        }))
    } else {
        None
    };

    let plan = normalize_plan(limits.plan_type.as_deref());

    ProviderQuota {
        available: true,
        primary,
        secondary,
        models: if models.is_empty() {
            None
        } else {
            Some(models)
        },
        plan_type: limits.plan_type.clone().filter(|s| !s.is_empty()),
        plan,
        extra,
        ..base
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> ProviderQuota {
        ProviderQuota {
            provider: "codex".into(),
            display_name: "Codex".into(),
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

    fn win(used: f64, mins: i64, resets: i64) -> CodexWindowRaw {
        CodexWindowRaw {
            used_percent: used,
            window_minutes: mins,
            resets_at: resets,
        }
    }

    fn codex_extra(q: &ProviderQuota) -> &CodexQuotaExtra {
        match q.extra.as_ref() {
            Some(ProviderExtra::Codex(c)) => c,
            _ => panic!("expected Codex extra"),
        }
    }

    // Future timestamp for tests that need a non-zero resets_at
    fn future_unix() -> i64 {
        // 2030-01-01T00:00:00Z in unix seconds
        1893456000
    }

    // -----------------------------------------------------------------------
    // primary/secondary basics
    // -----------------------------------------------------------------------

    #[test]
    fn primary_used_40_remaining_60_with_window() {
        let limits = CodexRateLimits {
            primary: Some(win(40.0, 300, 0)),
            ..Default::default()
        };
        let q = build_codex_quota(&limits, base());
        assert!(q.available);
        assert_eq!(q.primary.as_ref().unwrap().remaining, 60.0);
        assert_eq!(q.primary.as_ref().unwrap().window_minutes, Some(300));
        assert!(q.primary.as_ref().unwrap().resets_at.is_none()); // resets 0
    }

    #[test]
    fn primary_used_0_remaining_100() {
        let limits = CodexRateLimits {
            primary: Some(win(0.0, 300, future_unix())),
            ..Default::default()
        };
        let q = build_codex_quota(&limits, base());
        assert_eq!(q.primary.as_ref().unwrap().remaining, 100.0);
    }

    #[test]
    fn primary_used_100_remaining_0() {
        let limits = CodexRateLimits {
            primary: Some(win(100.0, 300, future_unix())),
            ..Default::default()
        };
        let q = build_codex_quota(&limits, base());
        assert_eq!(q.primary.as_ref().unwrap().remaining, 0.0);
    }

    #[test]
    fn secondary_used_20_remaining_80() {
        let limits = CodexRateLimits {
            primary: Some(win(40.0, 300, future_unix())),
            secondary: Some(win(20.0, 10080, future_unix())),
            ..Default::default()
        };
        let q = build_codex_quota(&limits, base());
        assert_eq!(q.secondary.as_ref().unwrap().remaining, 80.0);
        assert_eq!(q.secondary.as_ref().unwrap().window_minutes, Some(10080));
    }

    #[test]
    fn resets_at_iso_string_from_unix_timestamp() {
        let ts: i64 = 1711540800; // fixed known timestamp
        let limits = CodexRateLimits {
            primary: Some(win(0.0, 300, ts)),
            ..Default::default()
        };
        let q = build_codex_quota(&limits, base());
        // 1711540800 seconds → 1711540800000 ms → iso
        let expected = iso_from_ms((ts as u64) * 1000);
        assert_eq!(
            q.primary.as_ref().unwrap().resets_at.as_deref(),
            Some(expected.as_str())
        );
    }

    #[test]
    fn resets_at_null_when_zero() {
        let limits = CodexRateLimits {
            primary: Some(win(50.0, 300, 0)),
            ..Default::default()
        };
        let q = build_codex_quota(&limits, base());
        assert!(q.primary.as_ref().unwrap().resets_at.is_none());
    }

    // -----------------------------------------------------------------------
    // No usable data → error
    // -----------------------------------------------------------------------

    #[test]
    fn no_usable_data_errors() {
        let limits = CodexRateLimits::default();
        let q = build_codex_quota(&limits, base());
        assert_eq!(q.error.as_deref(), Some("No quota windows found"));
        assert!(!q.available);
    }

    #[test]
    fn plan_type_only_no_windows_errors() {
        let limits = CodexRateLimits {
            plan_type: Some("pro".into()),
            ..Default::default()
        };
        let q = build_codex_quota(&limits, base());
        assert_eq!(q.error.as_deref(), Some("No quota windows found"));
        assert!(!q.available);
    }

    // -----------------------------------------------------------------------
    // Plan type mapping
    // -----------------------------------------------------------------------

    #[test]
    fn plan_type_enterprise_maps() {
        let limits = CodexRateLimits {
            primary: Some(win(10.0, 300, 0)),
            plan_type: Some("enterprise".into()),
            ..Default::default()
        };
        let q = build_codex_quota(&limits, base());
        assert_eq!(q.plan.as_deref(), Some("Enterprise"));
        assert_eq!(q.plan_type.as_deref(), Some("enterprise"));
    }

    #[test]
    fn plan_type_null_omitted() {
        let limits = CodexRateLimits {
            primary: Some(win(10.0, 300, 0)),
            plan_type: None,
            ..Default::default()
        };
        let q = build_codex_quota(&limits, base());
        assert!(q.plan.is_none());
        assert!(q.plan_type.is_none());
    }

    #[test]
    fn plan_type_various_cases() {
        let cases = [
            ("free", "Free"),
            ("pro", "Pro"),
            ("team", "Business"),
            ("business", "Business"),
            ("enterprise", "Enterprise"),
            ("edu", "Edu"),
            ("education", "Edu"),
            ("go", "Go"),
            ("plus", "Plus"),
            ("apikey", "API Key"),
            ("api_key", "API Key"),
        ];
        for (input, expected) in cases {
            let limits = CodexRateLimits {
                primary: Some(win(10.0, 300, 0)),
                plan_type: Some(input.into()),
                ..Default::default()
            };
            let q = build_codex_quota(&limits, base());
            assert_eq!(q.plan.as_deref(), Some(expected), "plan_type '{input}'");
            assert_eq!(q.plan_type.as_deref(), Some(input));
        }
    }

    #[test]
    fn unknown_plan_type_titlecased() {
        let limits = CodexRateLimits {
            primary: Some(win(10.0, 300, 0)),
            plan_type: Some("custom_plan_xyz".into()),
            ..Default::default()
        };
        let q = build_codex_quota(&limits, base());
        assert_eq!(q.plan.as_deref(), Some("Custom Plan Xyz"));
    }

    // -----------------------------------------------------------------------
    // Credits / extraUsage
    // -----------------------------------------------------------------------

    #[test]
    fn credits_has_credits_true_sets_extra_usage() {
        let limits = CodexRateLimits {
            primary: Some(win(20.0, 300, 0)),
            credits: Some(CodexCredits {
                has_credits: true,
                unlimited: false,
                balance: "10.50".into(),
            }),
            ..Default::default()
        };
        let q = build_codex_quota(&limits, base());
        let eu = codex_extra(&q).extra_usage.as_ref().unwrap();
        assert!(eu.enabled);
        assert_eq!(eu.remaining, 11.0); // min(100, round(10.50)) = 11
        assert_eq!(eu.limit, 0.0);
        assert_eq!(eu.used, 0.0);
    }

    #[test]
    fn credits_capped_and_unlimited() {
        let limits = CodexRateLimits {
            primary: Some(win(10.0, 300, 0)),
            credits: Some(CodexCredits {
                has_credits: true,
                unlimited: false,
                balance: "250".into(),
            }),
            ..Default::default()
        };
        let q = build_codex_quota(&limits, base());
        let eu = codex_extra(&q).extra_usage.as_ref().unwrap();
        assert_eq!(eu.remaining, 100.0); // min(100, 250)
        assert_eq!(eu.limit, 0.0);

        // TS test uses has_credits: true here (brief had has_credits: false — mismatch vs codex.test.ts:569)
        let limits2 = CodexRateLimits {
            primary: Some(win(10.0, 300, 0)),
            credits: Some(CodexCredits {
                has_credits: true,
                unlimited: true,
                balance: "0".into(),
            }),
            ..Default::default()
        };
        let eu2 = codex_extra(&build_codex_quota(&limits2, base()))
            .extra_usage
            .clone()
            .unwrap();
        assert_eq!(eu2.remaining, 100.0);
        assert_eq!(eu2.limit, -1.0);
    }

    #[test]
    fn credits_balance_gt_zero_without_has_credits() {
        let limits = CodexRateLimits {
            primary: Some(win(20.0, 300, 0)),
            credits: Some(CodexCredits {
                has_credits: false,
                unlimited: false,
                balance: "5.00".into(),
            }),
            ..Default::default()
        };
        let q = build_codex_quota(&limits, base());
        let eu = codex_extra(&q).extra_usage.as_ref().unwrap();
        assert!(eu.enabled);
        assert_eq!(eu.remaining, 5.0);
    }

    #[test]
    fn credits_no_data_omits_extra_usage() {
        let limits = CodexRateLimits {
            primary: Some(win(20.0, 300, 0)),
            ..Default::default()
        };
        let q = build_codex_quota(&limits, base());
        // extra is present (modelsDetailed), but extra_usage is None
        let ce = codex_extra(&q);
        assert!(ce.extra_usage.is_none());
    }

    #[test]
    fn credits_false_and_balance_zero_omits_extra_usage() {
        let limits = CodexRateLimits {
            primary: Some(win(20.0, 300, 0)),
            credits: Some(CodexCredits {
                has_credits: false,
                unlimited: false,
                balance: "0".into(),
            }),
            ..Default::default()
        };
        let q = build_codex_quota(&limits, base());
        let ce = codex_extra(&q);
        assert!(ce.extra_usage.is_none());
    }

    // -----------------------------------------------------------------------
    // Window classification
    // -----------------------------------------------------------------------

    #[test]
    fn window_300_classifies_as_five_hour() {
        let limits = CodexRateLimits {
            primary: Some(win(10.0, 300, future_unix())),
            ..Default::default()
        };
        let q = build_codex_quota(&limits, base());
        let md = codex_extra(&q).models_detailed.as_ref().unwrap();
        let model = md.values().next().unwrap();
        assert!(model.five_hour.is_some());
        assert_eq!(model.five_hour.as_ref().unwrap().remaining, 90.0);
    }

    #[test]
    fn window_10080_classifies_as_seven_day() {
        let limits = CodexRateLimits {
            secondary: Some(win(25.0, 10080, future_unix())),
            ..Default::default()
        };
        let q = build_codex_quota(&limits, base());
        let md = codex_extra(&q).models_detailed.as_ref().unwrap();
        let model = md.values().next().unwrap();
        assert!(model.seven_day.is_some());
        assert_eq!(model.seven_day.as_ref().unwrap().remaining, 75.0);
    }

    #[test]
    fn tolerates_five_hour_within_90min() {
        // 210 = 300 - 90 (boundary), 390 = 300 + 90 (boundary)
        let mut buckets = IndexMap::new();
        buckets.insert(
            "b1".to_string(),
            CodexLimitBucket {
                limit_id: "b1".into(),
                limit_name: None,
                primary: Some(win(10.0, 210, future_unix())),
                secondary: None,
            },
        );
        buckets.insert(
            "b2".to_string(),
            CodexLimitBucket {
                limit_id: "b2".into(),
                limit_name: None,
                primary: Some(win(20.0, 390, future_unix())),
                secondary: None,
            },
        );
        let limits = CodexRateLimits {
            buckets: Some(buckets),
            ..Default::default()
        };
        let q = build_codex_quota(&limits, base());
        let md = codex_extra(&q).models_detailed.as_ref().unwrap();
        for windows in md.values() {
            assert!(
                windows.five_hour.is_some(),
                "expected fiveHour for boundary minutes"
            );
        }
    }

    #[test]
    fn tolerates_seven_day_within_1440min() {
        // 8640 = 10080 - 1440, 11520 = 10080 + 1440
        let mut buckets = IndexMap::new();
        buckets.insert(
            "b1".to_string(),
            CodexLimitBucket {
                limit_id: "b1".into(),
                limit_name: None,
                primary: None,
                secondary: Some(win(30.0, 8640, future_unix())),
            },
        );
        buckets.insert(
            "b2".to_string(),
            CodexLimitBucket {
                limit_id: "b2".into(),
                limit_name: None,
                primary: None,
                secondary: Some(win(40.0, 11520, future_unix())),
            },
        );
        let limits = CodexRateLimits {
            buckets: Some(buckets),
            ..Default::default()
        };
        let q = build_codex_quota(&limits, base());
        let md = codex_extra(&q).models_detailed.as_ref().unwrap();
        for windows in md.values() {
            assert!(
                windows.seven_day.is_some(),
                "expected sevenDay for boundary minutes"
            );
        }
    }

    #[test]
    fn unrecognized_window_uses_fallback_mapping() {
        // 60 min = "other" from classify_window, but fallback: primary→fiveHour, secondary→sevenDay
        let mut buckets = IndexMap::new();
        buckets.insert(
            "b1".to_string(),
            CodexLimitBucket {
                limit_id: "b1".into(),
                limit_name: None,
                primary: Some(win(10.0, 60, future_unix())),
                secondary: Some(win(20.0, 60, future_unix())),
            },
        );
        let limits = CodexRateLimits {
            buckets: Some(buckets),
            ..Default::default()
        };
        let q = build_codex_quota(&limits, base());
        let md = codex_extra(&q).models_detailed.as_ref().unwrap();
        let model = md.values().next().unwrap();
        // primary (60 min → other → fallback fiveHour)
        // secondary (60 min → other initially, but place_window already put it in other;
        //   then fallback seven_day fills since five_hour was already filled by place_window fallback)
        // Actually: place_window(primary, 60) → other; place_window(secondary, 60) → other
        // then fallback: five_hour is None → fill from primary; seven_day is None → fill from secondary
        assert!(model.five_hour.is_some());
        assert!(model.seven_day.is_some());
    }

    // -----------------------------------------------------------------------
    // Multiple buckets
    // -----------------------------------------------------------------------

    #[test]
    fn multiple_buckets_create_models_detailed_entries() {
        let mut buckets = IndexMap::new();
        buckets.insert(
            "codex-mini".to_string(),
            CodexLimitBucket {
                limit_id: "codex-mini".into(),
                limit_name: Some("Codex Mini".into()),
                primary: Some(win(30.0, 300, future_unix())),
                secondary: Some(win(15.0, 10080, future_unix())),
            },
        );
        buckets.insert(
            "codex-standard".to_string(),
            CodexLimitBucket {
                limit_id: "codex-standard".into(),
                limit_name: Some("Codex Standard".into()),
                primary: Some(win(60.0, 300, future_unix())),
                secondary: Some(win(45.0, 10080, future_unix())),
            },
        );
        let limits = CodexRateLimits {
            buckets: Some(buckets),
            ..Default::default()
        };
        let q = build_codex_quota(&limits, base());
        assert!(q.available);
        let md = codex_extra(&q).models_detailed.as_ref().unwrap();
        assert_eq!(md.len(), 2);
        assert!(md.contains_key("Codex Mini"));
        assert!(md.contains_key("Codex Standard"));
        assert_eq!(md["Codex Mini"].five_hour.as_ref().unwrap().remaining, 70.0);
        assert_eq!(md["Codex Mini"].seven_day.as_ref().unwrap().remaining, 85.0);
        assert_eq!(
            md["Codex Standard"].five_hour.as_ref().unwrap().remaining,
            40.0
        );
        assert_eq!(
            md["Codex Standard"].seven_day.as_ref().unwrap().remaining,
            55.0
        );
    }

    #[test]
    fn limit_id_used_when_limit_name_null() {
        let mut buckets = IndexMap::new();
        buckets.insert(
            "my_custom_limit".to_string(),
            CodexLimitBucket {
                limit_id: "my_custom_limit".into(),
                limit_name: None,
                primary: Some(win(10.0, 300, future_unix())),
                secondary: None,
            },
        );
        let limits = CodexRateLimits {
            buckets: Some(buckets),
            ..Default::default()
        };
        let q = build_codex_quota(&limits, base());
        let md = codex_extra(&q).models_detailed.as_ref().unwrap();
        assert_eq!(md.len(), 1);
        assert!(md.contains_key("My Custom Limit"));
    }

    #[test]
    fn flatten_picks_five_hour_first() {
        let mut buckets = IndexMap::new();
        buckets.insert(
            "codex".to_string(),
            CodexLimitBucket {
                limit_id: "codex".into(),
                limit_name: Some("Codex".into()),
                primary: Some(win(25.0, 300, future_unix())),
                secondary: Some(win(50.0, 10080, future_unix())),
            },
        );
        let limits = CodexRateLimits {
            buckets: Some(buckets),
            ..Default::default()
        };
        let q = build_codex_quota(&limits, base());
        assert!(q.models.is_some());
        let models = q.models.as_ref().unwrap();
        assert_eq!(models["Codex"].remaining, 75.0); // fiveHour preferred
    }

    #[test]
    fn dedup_bucket_names_with_suffix() {
        let mut buckets = IndexMap::new();
        buckets.insert(
            "a".to_string(),
            CodexLimitBucket {
                limit_id: "a".into(),
                limit_name: Some("gpt".into()),
                primary: Some(win(20.0, 300, 0)),
                secondary: None,
            },
        );
        buckets.insert(
            "b".to_string(),
            CodexLimitBucket {
                limit_id: "b".into(),
                limit_name: Some("gpt".into()),
                primary: Some(win(30.0, 300, 0)),
                secondary: None,
            },
        );
        let limits = CodexRateLimits {
            buckets: Some(buckets),
            ..Default::default()
        };
        let q = build_codex_quota(&limits, base());
        let md = codex_extra(&q).models_detailed.as_ref().unwrap();
        assert!(md.contains_key("Gpt"));
        assert!(md.contains_key("Gpt (2)"));
    }

    #[test]
    fn dedup_with_codex_label_name_collision() {
        let mut buckets = IndexMap::new();
        buckets.insert(
            "a".to_string(),
            CodexLimitBucket {
                limit_id: "a".into(),
                limit_name: Some("Codex".into()),
                primary: Some(win(10.0, 300, 0)),
                secondary: None,
            },
        );
        buckets.insert(
            "b".to_string(),
            CodexLimitBucket {
                limit_id: "b".into(),
                limit_name: Some("Codex".into()),
                primary: Some(win(20.0, 300, 0)),
                secondary: None,
            },
        );
        let limits = CodexRateLimits {
            buckets: Some(buckets),
            ..Default::default()
        };
        let q = build_codex_quota(&limits, base());
        let md = codex_extra(&q).models_detailed.as_ref().unwrap();
        assert_eq!(md.len(), 2);
        assert!(md.contains_key("Codex"));
        assert!(md.contains_key("Codex (2)"));
    }

    // -----------------------------------------------------------------------
    // Legacy fallback (no buckets, only primary/secondary)
    // -----------------------------------------------------------------------

    #[test]
    fn legacy_single_codex_entry() {
        let limits = CodexRateLimits {
            primary: Some(win(35.0, 300, future_unix())),
            secondary: Some(win(55.0, 10080, future_unix())),
            ..Default::default()
        };
        let q = build_codex_quota(&limits, base());
        let md = codex_extra(&q).models_detailed.as_ref().unwrap();
        assert_eq!(md.len(), 1);
        assert!(md.contains_key("Codex"));
        assert_eq!(md["Codex"].five_hour.as_ref().unwrap().remaining, 65.0);
        assert_eq!(md["Codex"].seven_day.as_ref().unwrap().remaining, 45.0);
    }

    // -----------------------------------------------------------------------
    // Primary/secondary selection
    // -----------------------------------------------------------------------

    #[test]
    fn explicit_primary_secondary_preferred_over_buckets() {
        let mut buckets = IndexMap::new();
        buckets.insert(
            "codex".to_string(),
            CodexLimitBucket {
                limit_id: "codex".into(),
                limit_name: None,
                primary: Some(win(99.0, 300, future_unix())),
                secondary: Some(win(99.0, 10080, future_unix())),
            },
        );
        let limits = CodexRateLimits {
            primary: Some(win(30.0, 300, future_unix())),
            secondary: Some(win(50.0, 10080, future_unix())),
            buckets: Some(buckets),
            ..Default::default()
        };
        let q = build_codex_quota(&limits, base());
        assert_eq!(q.primary.as_ref().unwrap().remaining, 70.0);
        assert_eq!(q.secondary.as_ref().unwrap().remaining, 50.0);
    }

    #[test]
    fn falls_back_to_bucket_five_hour_seven_day() {
        let mut buckets = IndexMap::new();
        buckets.insert(
            "codex".to_string(),
            CodexLimitBucket {
                limit_id: "codex".into(),
                limit_name: None,
                primary: Some(win(40.0, 300, future_unix())),
                secondary: Some(win(60.0, 10080, future_unix())),
            },
        );
        let limits = CodexRateLimits {
            buckets: Some(buckets),
            ..Default::default()
        };
        let q = build_codex_quota(&limits, base());
        // pickPrimary → first model's fiveHour
        assert_eq!(q.primary.as_ref().unwrap().remaining, 60.0);
        // pickSecondary → first model's sevenDay
        assert_eq!(q.secondary.as_ref().unwrap().remaining, 40.0);
    }

    // -----------------------------------------------------------------------
    // Edge: empty buckets / skipped
    // -----------------------------------------------------------------------

    #[test]
    fn empty_plan_type_is_omitted() {
        let limits = CodexRateLimits {
            primary: Some(win(10.0, 300, 0)),
            plan_type: Some("".into()),
            ..Default::default()
        };
        let q = build_codex_quota(&limits, base());
        assert!(
            q.plan_type.is_none(),
            "plan_type vazio deve ser omitido (casa o TS)"
        );
        assert!(q.plan.is_none());
    }

    #[test]
    fn skips_bucket_with_no_primary_or_secondary() {
        let mut buckets = IndexMap::new();
        buckets.insert(
            "empty".to_string(),
            CodexLimitBucket {
                limit_id: "empty".into(),
                limit_name: None,
                primary: None,
                secondary: None,
            },
        );
        buckets.insert(
            "valid".to_string(),
            CodexLimitBucket {
                limit_id: "valid".into(),
                limit_name: Some("Valid Bucket".into()),
                primary: Some(win(20.0, 300, future_unix())),
                secondary: None,
            },
        );
        let limits = CodexRateLimits {
            primary: Some(win(10.0, 300, future_unix())),
            buckets: Some(buckets),
            ..Default::default()
        };
        let q = build_codex_quota(&limits, base());
        let md = codex_extra(&q).models_detailed.as_ref().unwrap();
        assert!(md.contains_key("Valid Bucket"));
        assert!(!md.contains_key("empty") && !md.contains_key("Empty"));
    }
}
