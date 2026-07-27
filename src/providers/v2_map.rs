//! Pure parsers that map provider fixtures/payloads into schema-v2 domain results.
//!
//! Adapters never serialize schema v2; they only produce [`ProviderResult`].
//! Monetary fields, credits, and arbitrary extras are discarded here.

use std::path::Path;

use regex::Regex;
use serde::Deserialize;
use serde_json::Value;
use time::format_description::well_known::Rfc3339;
use time::{OffsetDateTime, UtcOffset};

use crate::cli::ProviderId;
use crate::status::schema::{Account, DataSource, Plan, ProviderResult, UsageWindow};
use crate::support::redact::strip_ansi_and_controls;

use super::catalog::{AMP, CLAUDE, CODEX, GROK};

// ---------------------------------------------------------------------------
// Amp
// ---------------------------------------------------------------------------

/// Parse `amp usage` text into a domain result. Credits/dollar lines are ignored.
pub fn amp_from_usage_text(stdout: &str, now: OffsetDateTime) -> ProviderResult {
    let text = strip_ansi_and_controls(stdout);
    let account = Regex::new(r"Signed in as (\S+)")
        .ok()
        .and_then(|re| re.captures(&text))
        .and_then(|c| c.get(1).map(|m| m.as_str().to_owned()));

    // Prefer percentage form; never emit spend/credits windows.
    let free_pct = Regex::new(r"Amp Free:\s*([0-9.]+)%\s*remaining")
        .ok()
        .and_then(|re| re.captures(&text))
        .and_then(|c| c.get(1)?.as_str().parse::<f64>().ok());

    let dollar_pct = if free_pct.is_none() {
        Regex::new(r"Amp Free:\s*\$([0-9.]+)/\$([0-9.]+)\s*remaining")
            .ok()
            .and_then(|re| re.captures(&text))
            .and_then(|c| {
                let remaining: f64 = c.get(1)?.as_str().parse().ok()?;
                let total: f64 = c.get(2)?.as_str().parse().ok()?;
                if total > 0.0 {
                    Some(((remaining / total) * 100.0).clamp(0.0, 100.0))
                } else {
                    None
                }
            })
    } else {
        None
    };

    let remaining = free_pct.or(dollar_pct);
    let mut windows = Vec::new();
    if let Some(rem) = remaining {
        let used = (100.0 - rem).clamp(0.0, 100.0);
        let resets = if text.contains("resets daily") {
            Some(next_utc_midnight(now))
        } else {
            None
        };
        if let Ok(window) = UsageWindow::try_new("daily", "Daily", used, rem, resets) {
            windows.push(window);
        }
    }

    ProviderResult::Ready {
        id: ProviderId::Amp,
        name: AMP.display_name.to_owned(),
        source: DataSource::Live,
        plan: None,
        account: account.map(|label| Account {
            label: sanitize_account_label(&label),
        }),
        windows,
        last_success_at: now,
    }
}

fn next_utc_midnight(now: OffsetDateTime) -> OffsetDateTime {
    let date = now.date();
    let tomorrow = date.next_day().unwrap_or(date);
    tomorrow
        .with_hms(0, 0, 0)
        .map(|t| t.assume_utc())
        .unwrap_or(now)
}

// ---------------------------------------------------------------------------
// Grok
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct GrokSignals {
    #[serde(default, rename = "contextTokensUsed")]
    context_tokens_used: Option<u64>,
    #[serde(default, rename = "contextWindowTokens")]
    context_window_tokens: Option<u64>,
}

#[derive(Debug, Deserialize, Default)]
struct GrokBillingDoc {
    #[serde(default, rename = "creditUsagePercent")]
    credit_usage_percent: Option<f64>,
    #[serde(default, rename = "currentPeriod")]
    current_period: Option<GrokPeriodRaw>,
    #[serde(default, rename = "billingPeriodEnd")]
    billing_period_end: Option<String>,
    #[serde(default, rename = "subscriptionTiers")]
    subscription_tiers: Option<String>,
    /// Discarded monetary fields.
    #[serde(default, rename = "prepaidBalance")]
    prepaid_balance: Option<Value>,
    #[serde(default, rename = "onDemandCap")]
    on_demand_cap: Option<Value>,
    #[serde(default, rename = "onDemandUsed")]
    on_demand_used: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct GrokPeriodRaw {
    #[serde(default)]
    end: Option<String>,
}

/// Parse Grok billing JSON into a single weekly percentage window.
///
/// - `creditUsagePercent` → used; remaining = 100 − used
/// - `currentPeriod.end` or `billingPeriodEnd` → resetsAt
/// - money-like fields discarded; never emits a `context` window
/// - missing/invalid percent → Ready with empty windows
pub fn grok_from_billing_json(
    bytes: &[u8],
    account_label: Option<String>,
    now: OffsetDateTime,
    _login_available: bool,
) -> ProviderResult {
    // Live CLI proxy wraps fields in `{ "config": { ... } }`; fixtures may be flat.
    let doc: GrokBillingDoc = match parse_grok_billing_doc(bytes) {
        Ok(doc) => doc,
        Err(_) => {
            return ProviderResult::ProviderError {
                id: ProviderId::Grok,
                name: GROK.display_name.to_owned(),
                message: "Grok returned an invalid billing payload.".into(),
                retryable: false,
            };
        }
    };

    // prepaid_balance / on_demand_* are deserialized only to document discard.
    let _ = (
        &doc.prepaid_balance,
        &doc.on_demand_cap,
        &doc.on_demand_used,
    );

    let mut windows = Vec::new();
    if let Some(used_raw) = doc.credit_usage_percent {
        if used_raw.is_finite() {
            let used = used_raw.clamp(0.0, 100.0);
            let remaining = (100.0 - used).clamp(0.0, 100.0);
            let resets = grok_billing_resets_at(&doc);
            if let Ok(w) = UsageWindow::try_new("weekly", "Weekly", used, remaining, resets) {
                windows.push(w);
            }
        }
    }

    let plan = doc
        .subscription_tiers
        .filter(|s| !s.is_empty())
        .map(|id| Plan {
            label: id.clone(),
            id,
        });

    ProviderResult::Ready {
        id: ProviderId::Grok,
        name: GROK.display_name.to_owned(),
        source: DataSource::Live,
        plan,
        account: account_label.map(|label| Account {
            label: sanitize_account_label(&label),
        }),
        windows,
        last_success_at: now,
    }
}

fn parse_grok_billing_doc(bytes: &[u8]) -> Result<GrokBillingDoc, serde_json::Error> {
    let value: Value = serde_json::from_slice(bytes)?;
    let payload = match &value {
        Value::Object(map) => match map.get("config") {
            Some(cfg) if cfg.is_object() => cfg.clone(),
            _ => value,
        },
        _ => value,
    };
    serde_json::from_value(payload)
}

fn grok_billing_resets_at(doc: &GrokBillingDoc) -> Option<OffsetDateTime> {
    let end = doc
        .current_period
        .as_ref()
        .and_then(|p| p.end.as_deref())
        .or(doc.billing_period_end.as_deref())?;
    OffsetDateTime::parse(end, &Rfc3339)
        .ok()
        .map(|ts| ts.to_offset(UtcOffset::UTC))
}

/// Build Grok result from auth flag + optional signals JSON bytes.
///
/// Legacy helper retained for unauthenticated/auth fixtures. Product collect
/// uses [`grok_from_billing_json`] (weekly only); do not treat context as primary.
pub fn grok_from_auth_and_signals(
    logged_in: bool,
    account_label: Option<String>,
    signals_json: Option<&[u8]>,
    now: OffsetDateTime,
    login_available: bool,
) -> ProviderResult {
    if !logged_in {
        return ProviderResult::Unauthenticated {
            id: ProviderId::Grok,
            name: GROK.display_name.to_owned(),
            message: "Grok is not authenticated.".into(),
            login_available,
            installation_url: GROK.installation_url.to_owned(),
        };
    }

    let mut windows = Vec::new();
    if let Some(bytes) = signals_json {
        if let Ok(signals) = serde_json::from_slice::<GrokSignals>(bytes) {
            if let (Some(used), Some(window)) =
                (signals.context_tokens_used, signals.context_window_tokens)
            {
                if window > 0 {
                    let used_pct = ((used as f64) * 100.0 / (window as f64)).clamp(0.0, 100.0);
                    let rem = (100.0 - used_pct).clamp(0.0, 100.0);
                    if let Ok(w) = UsageWindow::try_new("context", "Context", used_pct, rem, None) {
                        windows.push(w);
                    }
                }
            }
        }
    }

    ProviderResult::Ready {
        id: ProviderId::Grok,
        name: GROK.display_name.to_owned(),
        source: DataSource::Live,
        plan: None,
        account: account_label.map(|label| Account {
            label: sanitize_account_label(&label),
        }),
        windows,
        last_success_at: now,
    }
}

// ---------------------------------------------------------------------------
// Codex
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct CodexWindowRaw {
    #[serde(rename = "usedPercent", alias = "used_percent")]
    used_percent: f64,
    #[serde(default, rename = "windowDurationMins", alias = "window_minutes")]
    window_minutes: Option<i64>,
    #[serde(default, rename = "resetsAt", alias = "resets_at")]
    resets_at: Option<i64>,
}

#[derive(Debug, Deserialize, Default)]
struct CodexRateLimitsDoc {
    #[serde(default)]
    primary: Option<CodexWindowRaw>,
    #[serde(default)]
    secondary: Option<CodexWindowRaw>,
    #[serde(default)]
    plan_type: Option<String>,
    /// Explicitly ignored monetary field.
    #[serde(default)]
    credits: Option<Value>,
}

/// Parse Codex rate-limit JSON. Credits are discarded.
///
/// Window IDs/labels come from `window_minutes` duration, not primary/secondary
/// slot order. Primary with 10080 minutes is weekly; 300 is session.
pub fn codex_from_rate_limits_json(bytes: &[u8], now: OffsetDateTime) -> ProviderResult {
    let doc: CodexRateLimitsDoc = match serde_json::from_slice(bytes) {
        Ok(doc) => doc,
        Err(_) => {
            return ProviderResult::ProviderError {
                id: ProviderId::Codex,
                name: CODEX.display_name.to_owned(),
                message: "Codex returned an invalid usage payload.".into(),
                retryable: false,
            };
        }
    };

    let mut windows = Vec::new();
    for (ordinal, raw) in [
        (1usize, doc.primary.as_ref()),
        (2usize, doc.secondary.as_ref()),
    ] {
        let Some(raw) = raw else {
            continue;
        };
        let (id, label) = codex_window_identity(raw.window_minutes, ordinal);
        if let Some(w) = codex_window(&id, &label, raw) {
            windows.push(w);
        }
    }
    let _ = doc.credits; // discarded

    ProviderResult::Ready {
        id: ProviderId::Codex,
        name: CODEX.display_name.to_owned(),
        source: DataSource::Live,
        plan: doc.plan_type.map(|id| Plan {
            label: id.clone(),
            id,
        }),
        account: None,
        windows,
        last_success_at: now,
    }
}

/// Map a Codex rate-limit duration to window id and display label.
///
/// Known durations: 300 → session, 10080 → weekly. Other positive minutes use
/// `other:{n}:{ordinal}`. Missing duration falls back by slot ordinal (primary
/// → session, secondary → weekly) for incomplete payloads.
fn codex_window_identity(window_minutes: Option<i64>, ordinal: usize) -> (String, String) {
    match window_minutes {
        Some(10080) => ("weekly".into(), "Weekly".into()),
        Some(300) => ("session".into(), "Session".into()),
        Some(n) if n > 0 => (format!("other:{n}:{ordinal}"), format!("{n}m")),
        _ => {
            if ordinal == 1 {
                ("session".into(), "Session".into())
            } else {
                ("weekly".into(), "Weekly".into())
            }
        }
    }
}

fn codex_window(id: &str, label: &str, raw: &CodexWindowRaw) -> Option<UsageWindow> {
    if !raw.used_percent.is_finite() {
        return None;
    }
    let used = raw.used_percent.clamp(0.0, 100.0);
    let remaining = (100.0 - used).clamp(0.0, 100.0);
    let resets = raw
        .resets_at
        .filter(|&ts| ts > 0)
        .and_then(|ts| OffsetDateTime::from_unix_timestamp(ts).ok())
        .map(|ts| ts.to_offset(UtcOffset::UTC));
    UsageWindow::try_new(id, label, used, remaining, resets).ok()
}

// ---------------------------------------------------------------------------
// Claude
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, Default)]
struct ClaudeUsageDoc {
    #[serde(default)]
    five_hour: Option<ClaudeWindowRaw>,
    #[serde(default)]
    seven_day: Option<ClaudeWindowRaw>,
    #[serde(default)]
    seven_day_opus: Option<ClaudeWindowRaw>,
    #[serde(default)]
    seven_day_sonnet: Option<ClaudeWindowRaw>,
    #[serde(default)]
    error: Option<ClaudeErrorRaw>,
    #[serde(default)]
    limits: Vec<ClaudeLimitRaw>,
    /// Discarded monetary block.
    #[serde(default)]
    spend: Option<Value>,
    #[serde(default)]
    extra_usage: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct ClaudeWindowRaw {
    /// Utilization is already a percentage 0..=100 (never treat as 0..=1).
    utilization: f64,
    #[serde(default)]
    resets_at: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ClaudeErrorRaw {
    error_code: String,
    #[serde(default)]
    #[allow(dead_code)]
    message: String,
}

#[derive(Debug, Deserialize)]
struct ClaudeLimitRaw {
    #[serde(default)]
    kind: Option<String>,
    #[serde(default)]
    utilization: Option<f64>,
    #[serde(default)]
    resets_at: Option<String>,
    #[serde(default)]
    scope: Option<ClaudeLimitScope>,
}

#[derive(Debug, Deserialize)]
struct ClaudeLimitScope {
    #[serde(default)]
    model: Option<ClaudeLimitModel>,
}

#[derive(Debug, Deserialize)]
struct ClaudeLimitModel {
    #[serde(default)]
    display_name: Option<String>,
    #[serde(default)]
    id: Option<String>,
}

/// Map Claude usage JSON to a domain result.
///
/// - `token_expired` → unauthenticated
/// - utilization is percent (double-division regression guard)
/// - spend/extra_usage discarded
/// - unknown limits without usable windows → ready with empty windows
pub fn claude_from_usage_json(
    bytes: &[u8],
    now: OffsetDateTime,
    plan: Option<Plan>,
    account: Option<Account>,
    login_available: bool,
) -> ProviderResult {
    let doc: ClaudeUsageDoc = match serde_json::from_slice(bytes) {
        Ok(doc) => doc,
        Err(_) => {
            return ProviderResult::ProviderError {
                id: ProviderId::Claude,
                name: CLAUDE.display_name.to_owned(),
                message: "Claude returned an invalid usage payload.".into(),
                retryable: false,
            };
        }
    };

    if let Some(err) = doc.error.as_ref() {
        if err.error_code == "token_expired" {
            return ProviderResult::Unauthenticated {
                id: ProviderId::Claude,
                name: CLAUDE.display_name.to_owned(),
                message: "Claude authentication expired.".into(),
                login_available,
                installation_url: CLAUDE.installation_url.to_owned(),
            };
        }
        return ProviderResult::ProviderError {
            id: ProviderId::Claude,
            name: CLAUDE.display_name.to_owned(),
            message: "Claude returned a provider error.".into(),
            retryable: false,
        };
    }

    let _ = (doc.spend, doc.extra_usage); // discarded

    let mut windows = Vec::new();
    if let Some(w) = doc.five_hour.as_ref() {
        if let Some(window) = claude_window("session", "Session", w) {
            windows.push(window);
        }
    }
    if let Some(w) = doc.seven_day.as_ref() {
        if let Some(window) = claude_window("weekly", "Weekly", w) {
            windows.push(window);
        }
    }
    // Dynamic model windows from limits[] or legacy seven_day_* fields.
    for (idx, limit) in doc.limits.iter().enumerate() {
        let Some(util) = limit.utilization else {
            continue;
        };
        let kind = limit.kind.as_deref().unwrap_or("");
        if kind == "five_hour" || kind == "seven_day" {
            continue; // handled via dedicated fields when present
        }
        let model_id = limit
            .scope
            .as_ref()
            .and_then(|s| s.model.as_ref())
            .and_then(|m| m.id.as_deref().or(m.display_name.as_deref()))
            .unwrap_or("model");
        let id = weekly_model_id(model_id, idx);
        let label = limit
            .scope
            .as_ref()
            .and_then(|s| s.model.as_ref())
            .and_then(|m| m.display_name.clone())
            .unwrap_or_else(|| "Weekly model".into());
        let raw = ClaudeWindowRaw {
            utilization: util,
            resets_at: limit.resets_at.clone(),
        };
        if let Some(window) = claude_window(&id, &label, &raw) {
            windows.push(window);
        }
    }
    for (suffix, field) in [
        ("opus", doc.seven_day_opus.as_ref()),
        ("sonnet", doc.seven_day_sonnet.as_ref()),
    ] {
        if let Some(w) = field {
            let id = weekly_model_id(suffix, 0);
            if let Some(window) = claude_window(&id, &format!("Weekly {suffix}"), w) {
                windows.push(window);
            }
        }
    }

    ProviderResult::Ready {
        id: ProviderId::Claude,
        name: CLAUDE.display_name.to_owned(),
        source: DataSource::Live,
        plan,
        account,
        windows,
        last_success_at: now,
    }
}

fn claude_window(id: &str, label: &str, raw: &ClaudeWindowRaw) -> Option<UsageWindow> {
    if !raw.utilization.is_finite() {
        return None;
    }
    // Guard double-division: values are already percent, not 0..=1 fractions.
    let used = if raw.utilization > 0.0 && raw.utilization <= 1.0 {
        // Ambiguous tiny values: treat as percent only when clearly percent-like
        // API always sends 0..=100; if someone passes 0.42 meaning 42%, that
        // was the historical bug — we intentionally treat <=1 as percent only
        // when the fixture marks it via >100 impossible; keep as-is clamp.
        raw.utilization.clamp(0.0, 100.0)
    } else {
        raw.utilization.clamp(0.0, 100.0)
    };
    let remaining = (100.0 - used).clamp(0.0, 100.0);
    let resets = raw
        .resets_at
        .as_deref()
        .and_then(|s| OffsetDateTime::parse(s, &Rfc3339).ok())
        .map(|ts| ts.to_offset(UtcOffset::UTC));
    UsageWindow::try_new(id, label, used, remaining, resets).ok()
}

/// Lowercase ASCII letters/digits/hyphens, prefixed with `weekly-model:`.
pub fn weekly_model_id(raw: &str, ordinal: usize) -> String {
    let mut sanitized: String = raw
        .chars()
        .flat_map(|c| c.to_lowercase())
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-')
        .collect();
    if sanitized.is_empty() {
        sanitized = "model".into();
    }
    let base = format!("weekly-model:{sanitized}");
    if ordinal <= 1 {
        base
    } else {
        format!("{base}:{ordinal}")
    }
}

fn sanitize_account_label(raw: &str) -> String {
    let cleaned = strip_ansi_and_controls(raw);
    // Drop anything that looks like a token-ish secret.
    if cleaned.len() > 64 || cleaned.contains("sk-") || cleaned.contains("eyJ") {
        return "Account".into();
    }
    cleaned
}

/// Assert a domain result never carries monetary residue in Debug form.
pub fn assert_no_money(result: &ProviderResult) {
    let text = format!("{result:?}");
    for banned in ["spend", "credits", "balance", "currency", "usd", "BRL"] {
        assert!(
            !text.to_ascii_lowercase().contains(banned),
            "domain result leaked monetary field '{banned}': {text}"
        );
    }
}

/// Walk limits for Grok/Codex filesystem discovery tests.
pub fn path_is_absolute_home(path: &Path) -> bool {
    path.is_absolute()
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::datetime;

    #[test]
    fn amp_free_pct_fixture_ready_without_credits() {
        let fixture = include_str!("../../tests/fixtures/amp/usage-free-pct.txt");
        let result = amp_from_usage_text(fixture, datetime!(2026-07-26 18:00:00 UTC));
        assert_no_money(&result);
        match result {
            ProviderResult::Ready { windows, .. } => {
                assert_eq!(windows.len(), 1);
                assert_eq!(windows[0].id(), "daily");
                assert!((windows[0].remaining_percent() - 97.0).abs() < 0.01);
            }
            other => panic!("expected ready, got {other:?}"),
        }
    }

    #[test]
    fn amp_legacy_dollars_converts_percent_and_drops_money() {
        let fixture = include_str!("../../tests/fixtures/amp/usage-legacy-dollars.txt");
        let result = amp_from_usage_text(fixture, datetime!(2026-07-26 18:00:00 UTC));
        assert_no_money(&result);
        match result {
            ProviderResult::Ready { windows, .. } => {
                assert_eq!(windows.len(), 1);
                assert!((windows[0].remaining_percent() - 70.0).abs() < 0.01);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn claude_token_expired_is_unauthenticated() {
        let body = br#"{"error":{"error_code":"token_expired","message":"expired"}}"#;
        let result =
            claude_from_usage_json(body, datetime!(2026-07-26 18:00:00 UTC), None, None, true);
        assert!(matches!(result, ProviderResult::Unauthenticated { .. }));
    }

    #[test]
    fn claude_does_not_double_divide_utilization() {
        let body = br#"{"five_hour":{"utilization":42.0,"resets_at":"2026-07-26T22:00:00Z"}}"#;
        let result =
            claude_from_usage_json(body, datetime!(2026-07-26 18:00:00 UTC), None, None, true);
        match result {
            ProviderResult::Ready { windows, .. } => {
                assert_eq!(windows.len(), 1);
                assert!((windows[0].used_percent() - 42.0).abs() < 0.01);
                assert!((windows[0].remaining_percent() - 58.0).abs() < 0.01);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn claude_unknown_limits_empty_windows() {
        let body = br#"{"limits":[{"kind":"mystery"}]}"#;
        let result =
            claude_from_usage_json(body, datetime!(2026-07-26 18:00:00 UTC), None, None, true);
        match result {
            ProviderResult::Ready { windows, .. } => assert!(windows.is_empty()),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn codex_discards_credits() {
        let body = br#"{"primary":{"usedPercent":30.0,"windowDurationMins":300,"resetsAt":1700000000},"credits":{"has_credits":true,"unlimited":false,"balance":"12.5"}}"#;
        let result = codex_from_rate_limits_json(body, datetime!(2026-07-26 18:00:00 UTC));
        assert_no_money(&result);
        match result {
            ProviderResult::Ready { windows, .. } => {
                assert_eq!(windows[0].id(), "session");
                assert!((windows[0].used_percent() - 30.0).abs() < 0.01);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn codex_primary_only_weekly_is_labeled_weekly() {
        let body = br#"{
      "primary": {"used_percent": 1.0, "window_minutes": 10080, "resets_at": 1785628013},
      "plan_type": "plus"
    }"#;
        let now = datetime!(2026-07-26 18:00:00 UTC);
        let result = codex_from_rate_limits_json(body, now);
        match result {
            ProviderResult::Ready { windows, plan, .. } => {
                assert_eq!(windows.len(), 1);
                assert_eq!(windows[0].id(), "weekly");
                assert_eq!(windows[0].label(), "Weekly");
                assert!((windows[0].used_percent() - 1.0).abs() < 0.01);
                assert!(plan.is_some());
            }
            other => panic!("expected ready, got {other:?}"),
        }
    }

    #[test]
    fn codex_dual_windows_map_session_and_weekly_by_duration() {
        let body = include_bytes!("../../tests/fixtures/providers/codex/rate-limits-ready.json");
        let result = codex_from_rate_limits_json(body, datetime!(2026-07-26 18:00:00 UTC));
        assert_no_money(&result);
        match result {
            ProviderResult::Ready { windows, .. } => {
                assert_eq!(windows.len(), 2);
                assert_eq!(windows[0].id(), "session");
                assert_eq!(windows[0].label(), "Session");
                assert!((windows[0].used_percent() - 30.0).abs() < 0.01);
                assert_eq!(windows[1].id(), "weekly");
                assert_eq!(windows[1].label(), "Weekly");
                assert!((windows[1].used_percent() - 40.0).abs() < 0.01);
            }
            other => panic!("expected ready, got {other:?}"),
        }
    }

    #[test]
    fn grok_billing_weekly_window_discards_money() {
        let body = include_bytes!("../../tests/fixtures/providers/grok/billing-weekly.json");
        let now = datetime!(2026-07-27 12:00:00 UTC);
        match grok_from_billing_json(body, Some("Ada".into()), now, true) {
            ProviderResult::Ready { windows, plan, .. } => {
                assert_eq!(windows.len(), 1);
                assert_eq!(windows[0].id(), "weekly");
                assert_eq!(windows[0].label(), "Weekly");
                assert!((windows[0].used_percent() - 33.0).abs() < 0.01);
                assert!((windows[0].remaining_percent() - 67.0).abs() < 0.01);
                assert!(windows[0].resets_at().is_some());
                assert_eq!(plan.as_ref().map(|p| p.label.as_str()), Some("SuperGrok"));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn grok_billing_ready_never_emits_context() {
        let body = include_bytes!("../../tests/fixtures/providers/grok/billing-weekly.json");
        let now = datetime!(2026-07-27 12:00:00 UTC);
        match grok_from_billing_json(body, None, now, true) {
            ProviderResult::Ready { windows, .. } => {
                assert!(windows.iter().all(|w| w.id() != "context"));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn grok_billing_accepts_cli_config_envelope() {
        let body =
            include_bytes!("../../tests/fixtures/providers/grok/billing-weekly-wrapped.json");
        let now = datetime!(2026-07-27 12:00:00 UTC);
        let result = grok_from_billing_json(body, None, now, true);
        assert_no_money(&result);
        match result {
            ProviderResult::Ready { windows, .. } => {
                assert_eq!(windows.len(), 1);
                assert_eq!(windows[0].id(), "weekly");
                assert!((windows[0].used_percent() - 96.0).abs() < 0.01);
                assert!((windows[0].remaining_percent() - 4.0).abs() < 0.01);
                assert!(windows[0].resets_at().is_some());
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn weekly_model_id_sanitizes() {
        assert_eq!(
            weekly_model_id("Claude Opus 4", 1),
            "weekly-model:claudeopus4"
        );
        assert_eq!(weekly_model_id("X", 2), "weekly-model:x:2");
    }
}
