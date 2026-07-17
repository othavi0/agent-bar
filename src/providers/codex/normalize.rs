//! Helpers de conversão CodexRateLimits → ProviderQuota.

use std::collections::BTreeMap;

use indexmap::IndexMap;

use super::types::{CodexLimitBucket, CodexRateLimits, CodexWindowRaw};
use crate::formatters::shared::{classify_window, normalize_plan, WindowKind};
use crate::providers::iso_from_ms;
use crate::providers::types::{
    CodexQuotaExtra, ExtraUsage, ModelWindows, ProviderExtra, ProviderQuota, QuotaWindow,
};

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
        severity: None,
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
