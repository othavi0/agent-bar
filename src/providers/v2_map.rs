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

/// Display labels for the shared window ids. Every provider mapper uses these;
/// QML renders them verbatim and never re-derives.
const LABEL_SESSION: &str = "Session (5h)";
const LABEL_DAILY: &str = "Daily (1d)";
const LABEL_WEEKLY: &str = "Weekly (7d)";

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
        if let Ok(window) = UsageWindow::try_new("daily", LABEL_DAILY, used, rem, resets) {
            windows.push(window);
        }
    }

    // Subscription line (2026-07-18 Amp subscriptions): two percentage buckets.
    // "orb usage" = included orb-hours allowance; "other usage" = included agent
    // usage. The plan name doubles as the Plan badge. The "Individual credits: $"
    // line is monetary and intentionally never parsed into a window (JSON-022B).
    let mut plan = None;
    if let Some(caps) = Regex::new(
        r"Subscription\s+(\S+):\s*([0-9.]+)%\s*other usage and\s*([0-9.]+)%\s*orb usage remaining",
    )
    .ok()
    .and_then(|re| re.captures(&text))
    {
        if let Some(name) = caps.get(1).map(|m| m.as_str()) {
            plan = Some(Plan {
                id: name.to_ascii_lowercase(),
                label: name.to_owned(),
            });
        }
        for (idx, id, label) in [
            (2usize, "plan-other", "Plan · other"),
            (3usize, "plan-orb", "Plan · orb"),
        ] {
            if let Some(rem) = caps.get(idx).and_then(|m| m.as_str().parse::<f64>().ok()) {
                let rem = rem.clamp(0.0, 100.0);
                let used = (100.0 - rem).clamp(0.0, 100.0);
                // No resets_at: Amp documents only "replenishes at the end of
                // each monthly period", with no timestamp exposed.
                if let Ok(w) = UsageWindow::try_new(id, label, used, rem, None) {
                    windows.push(w);
                }
            }
        }
    }

    ProviderResult::Ready {
        id: ProviderId::Amp,
        name: AMP.display_name.to_owned(),
        source: DataSource::Live,
        plan,
        account: account.map(|label| Account {
            label: sanitize_account_label(&label),
        }),
        windows,
        last_success_at: now,
        rate_limit_resets_available: None,
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
    #[serde(default, rename = "type")]
    period_type: Option<String>,
}

/// USAGE_PERIOD_TYPE_WEEKLY → ("weekly", "Weekly (7d)"); other known-shaped
/// values strip the prefix (lowercase id, title-case label); absent/foreign
/// values keep the historical weekly identity.
fn grok_window_identity(period_type: Option<&str>) -> (String, String) {
    match period_type {
        Some("USAGE_PERIOD_TYPE_WEEKLY") | None => ("weekly".into(), LABEL_WEEKLY.into()),
        Some(other) => match other.strip_prefix("USAGE_PERIOD_TYPE_") {
            Some(rest) if !rest.is_empty() => (
                rest.to_ascii_lowercase(),
                format_plan_label(&rest.to_ascii_lowercase()),
            ),
            _ => ("weekly".into(), LABEL_WEEKLY.into()),
        },
    }
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
            let (id, label) = grok_window_identity(
                doc.current_period
                    .as_ref()
                    .and_then(|p| p.period_type.as_deref()),
            );
            if let Ok(w) = UsageWindow::try_new(&id, &label, used, remaining, resets) {
                windows.push(w);
            }
        }
    }

    let plan = doc
        .subscription_tiers
        .filter(|s| !s.is_empty())
        .map(|id| Plan {
            label: format_plan_label(&id),
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
        rate_limit_resets_available: None,
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
            retryable: false,
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
        rate_limit_resets_available: None,
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
    #[serde(default, rename = "individualLimit")]
    individual_limit: Option<CodexIndividualLimitRaw>,
    #[serde(default, rename = "extraBuckets")]
    extra_buckets: Vec<CodexExtraBucketRaw>,
    #[serde(default, rename = "rateLimitResetsAvailable")]
    rate_limit_resets_available: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct CodexIndividualLimitRaw {
    #[serde(rename = "remainingPercent")]
    remaining_percent: f64,
    #[serde(default, rename = "resetsAt")]
    resets_at: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct CodexExtraBucketRaw {
    #[serde(rename = "limitId")]
    limit_id: String,
    #[serde(default)]
    primary: Option<CodexWindowRaw>,
    #[serde(default)]
    secondary: Option<CodexWindowRaw>,
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

    for bucket in &doc.extra_buckets {
        let sanitized = sanitize_bucket_id(&bucket.limit_id);
        if let Some(raw) = bucket.primary.as_ref() {
            let id = format!("codex:{sanitized}");
            let label = codex_extra_bucket_label(&sanitized, raw.window_minutes);
            if let Some(w) = codex_window(&id, &label, raw) {
                windows.push(w);
            }
        }
        if let Some(raw) = bucket.secondary.as_ref() {
            let id = format!("codex:{sanitized}:2");
            let label = codex_extra_bucket_label(&sanitized, raw.window_minutes);
            if let Some(w) = codex_window(&id, &label, raw) {
                windows.push(w);
            }
        }
    }

    if let Some(il) = doc.individual_limit.as_ref() {
        if il.remaining_percent.is_finite() {
            let rem = il.remaining_percent.clamp(0.0, 100.0);
            let used = (100.0 - rem).clamp(0.0, 100.0);
            let resets = il
                .resets_at
                .filter(|&ts| ts > 0)
                .and_then(|ts| OffsetDateTime::from_unix_timestamp(ts).ok())
                .map(|ts| ts.to_offset(UtcOffset::UTC));
            if let Ok(w) =
                UsageWindow::try_new("workspace-limit", "Workspace limit", used, rem, resets)
            {
                windows.push(w);
            }
        }
    }

    let _ = doc.credits; // discarded

    ProviderResult::Ready {
        id: ProviderId::Codex,
        name: CODEX.display_name.to_owned(),
        source: DataSource::Live,
        plan: doc.plan_type.map(|id| Plan {
            label: format_plan_label(&id),
            id,
        }),
        account: None,
        windows,
        last_success_at: now,
        rate_limit_resets_available: doc.rate_limit_resets_available,
    }
}

/// Lowercase ASCII letters/digits/hyphens; empty input falls back to `bucket`.
/// Mirrors the sanitization in [`weekly_model_id`].
fn sanitize_bucket_id(raw: &str) -> String {
    let sanitized: String = raw
        .chars()
        .flat_map(|c| c.to_lowercase())
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-')
        .collect();
    if sanitized.is_empty() {
        "bucket".into()
    } else {
        sanitized
    }
}

/// Display label for a Codex extra rate-limit bucket window.
fn codex_extra_bucket_label(sanitized: &str, window_minutes: Option<i64>) -> String {
    let name = format_plan_label(sanitized);
    match window_minutes {
        Some(10080) => format!("{name} (7d)"),
        Some(mins) => format!("{name} ({mins}m)"),
        None => name,
    }
}

/// Map a Codex rate-limit duration to window id and display label.
///
/// Known durations: 300 → session, 10080 → weekly. Other positive minutes use
/// `other:{n}:{ordinal}`. Missing duration falls back by slot ordinal (primary
/// → session, secondary → weekly) for incomplete payloads.
fn codex_window_identity(window_minutes: Option<i64>, ordinal: usize) -> (String, String) {
    match window_minutes {
        Some(10080) => ("weekly".into(), LABEL_WEEKLY.into()),
        Some(300) => ("session".into(), LABEL_SESSION.into()),
        Some(n) if n > 0 => (format!("other:{n}:{ordinal}"), format!("{n}m")),
        _ => {
            if ordinal == 1 {
                ("session".into(), LABEL_SESSION.into())
            } else {
                ("weekly".into(), LABEL_WEEKLY.into())
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
    seven_day_oauth_apps: Option<ClaudeWindowRaw>,
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
    /// Current limits[] vocabulary (captured 2026-08-01) reports the value
    /// as `percent`; older payloads used `utilization`. Both are 0..=100.
    #[serde(default)]
    percent: Option<f64>,
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
                message: "Claude session expired. Open Claude Code to refresh it.".into(),
                login_available,
                installation_url: CLAUDE.installation_url.to_owned(),
                retryable: true,
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
        if let Some(window) = claude_window("session", LABEL_SESSION, w) {
            push_window_unique(&mut windows, window);
        }
    }
    // OAuth tokens report the weekly bucket as seven_day_oauth_apps; prefer it.
    if let Some(w) = doc.seven_day_oauth_apps.as_ref().or(doc.seven_day.as_ref()) {
        if let Some(window) = claude_window("weekly", LABEL_WEEKLY, w) {
            push_window_unique(&mut windows, window);
        }
    }
    // Dynamic windows from limits[] (or legacy seven_day_* fields below).
    // Grouped entries (session/weekly_all) mirror the dedicated fields and
    // dedup against them by id — and become the source if the API retires
    // the top-level fields, as it already did with seven_day_oauth_apps.
    let mut scoped_count = 0usize;
    for limit in doc.limits.iter() {
        let Some(util) = limit.utilization.or(limit.percent) else {
            continue;
        };
        let kind = limit.kind.as_deref().unwrap_or("");
        if kind == "five_hour" || kind == "seven_day" || kind == "seven_day_oauth_apps" {
            continue; // legacy vocabulary: dedicated fields own these
        }
        let raw = ClaudeWindowRaw {
            utilization: util,
            resets_at: limit.resets_at.clone(),
        };
        let (id, label) = match kind {
            "session" => ("session".to_owned(), LABEL_SESSION.to_owned()),
            "weekly_all" => ("weekly".to_owned(), LABEL_WEEKLY.to_owned()),
            _ => {
                let model_id = limit
                    .scope
                    .as_ref()
                    .and_then(|s| s.model.as_ref())
                    .and_then(|m| m.id.as_deref().or(m.display_name.as_deref()))
                    .unwrap_or("model");
                // Ordinal counts emitted model windows, not the array
                // position: ids must not shift when the API reorders or
                // prepends grouped entries.
                let id = weekly_model_id(model_id, scoped_count);
                scoped_count += 1;
                let label = limit
                    .scope
                    .as_ref()
                    .and_then(|s| s.model.as_ref())
                    .and_then(|m| m.display_name.clone())
                    .unwrap_or_else(|| "Model".into());
                (id, label)
            }
        };
        if let Some(window) = claude_window(&id, &label, &raw) {
            push_window_unique(&mut windows, window);
        }
    }
    for (suffix, label, field) in [
        ("opus", "Opus", doc.seven_day_opus.as_ref()),
        ("sonnet", "Sonnet", doc.seven_day_sonnet.as_ref()),
    ] {
        if let Some(w) = field {
            let id = weekly_model_id(suffix, 0);
            if let Some(window) = claude_window(&id, label, w) {
                push_window_unique(&mut windows, window);
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
        rate_limit_resets_available: None,
    }
}

/// Insert keeping window ids unique; first occurrence wins (dynamic limits[]
/// entries are pushed before legacy seven_day_* fields on purpose).
fn push_window_unique(windows: &mut Vec<UsageWindow>, window: UsageWindow) {
    if windows.iter().any(|w| w.id() == window.id()) {
        return;
    }
    windows.push(window);
}

/// Parse a reset timestamp that may be RFC3339, epoch seconds, or epoch millis.
///
/// The documented contract is RFC3339-only, but the live OAuth endpoint and
/// sibling providers also emit epochs; be liberal on input, strict on output.
fn parse_reset_timestamp(raw: &str) -> Option<OffsetDateTime> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Ok(ts) = OffsetDateTime::parse(trimmed, &Rfc3339) {
        return Some(ts.to_offset(UtcOffset::UTC));
    }
    let numeric: i128 = trimmed.parse().ok()?;
    if numeric <= 0 {
        return None;
    }
    // < 1e12 → seconds; otherwise milliseconds (mirrors the reference widget).
    let nanos = if numeric < 1_000_000_000_000 {
        numeric.checked_mul(1_000_000_000)?
    } else {
        numeric.checked_mul(1_000_000)?
    };
    OffsetDateTime::from_unix_timestamp_nanos(nanos)
        .ok()
        .map(|ts| ts.to_offset(UtcOffset::UTC))
}

fn claude_window(id: &str, label: &str, raw: &ClaudeWindowRaw) -> Option<UsageWindow> {
    if !raw.utilization.is_finite() {
        return None;
    }
    // Utilization is already percent scale (1.0 == 1%); clamp only.
    let used = raw.utilization.clamp(0.0, 100.0);
    let remaining = (100.0 - used).clamp(0.0, 100.0);
    let resets = raw.resets_at.as_deref().and_then(parse_reset_timestamp);
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

/// Title-case a raw plan/tier id for display: "pro" → "Pro",
/// "self_serve_business_usage_based" → "Self Serve Business Usage Based".
///
/// Amp keeps its own label verbatim; Grok (Task 3) and Codex (PR2 Task 9)
/// consume this for their raw tier ids.
pub(crate) fn format_plan_label(raw: &str) -> String {
    raw.split('_')
        .filter(|s| !s.is_empty())
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
                assert_eq!(windows[0].label(), "Daily (1d)");
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
    fn amp_subscription_fixture_emits_plan_windows_and_plan() {
        let fixture = include_str!("../../tests/fixtures/amp/usage-subscription-pct.txt");
        let result = amp_from_usage_text(fixture, datetime!(2026-08-07 12:00:00 UTC));
        assert_no_money(&result);
        match result {
            ProviderResult::Ready { windows, plan, .. } => {
                let ids: Vec<&str> = windows.iter().map(|w| w.id()).collect();
                assert_eq!(ids, vec!["daily", "plan-other", "plan-orb"]);
                assert_eq!(windows[1].label(), "Plan · other");
                assert!((windows[1].remaining_percent() - 92.0).abs() < 0.01);
                assert!((windows[1].used_percent() - 8.0).abs() < 0.01);
                assert_eq!(windows[2].label(), "Plan · orb");
                assert!((windows[2].remaining_percent() - 100.0).abs() < 0.01);
                // Amp exposes no subscription reset timestamp ("monthly" only).
                assert!(windows[1].resets_at().is_none());
                assert!(windows[2].resets_at().is_none());
                let plan = plan.expect("plan from Subscription line");
                assert_eq!(plan.id, "megawatt");
                assert_eq!(plan.label, "Megawatt");
            }
            other => panic!("expected ready, got {other:?}"),
        }
    }

    #[test]
    fn amp_free_only_fixture_still_has_no_plan() {
        let fixture = include_str!("../../tests/fixtures/amp/usage-free-pct.txt");
        let result = amp_from_usage_text(fixture, datetime!(2026-08-07 12:00:00 UTC));
        match result {
            ProviderResult::Ready { windows, plan, .. } => {
                assert_eq!(windows.len(), 1);
                assert!(plan.is_none());
            }
            other => panic!("expected ready, got {other:?}"),
        }
    }

    #[test]
    fn amp_individual_credits_line_never_emits_window() {
        // Only the monetary line plus account: Ready with zero windows, no money.
        let text = "Signed in as user@email.com (nick)\nIndividual credits: $4.19 remaining (replenishes automatically)\n";
        let result = amp_from_usage_text(text, datetime!(2026-08-07 12:00:00 UTC));
        assert_no_money(&result);
        match result {
            ProviderResult::Ready { windows, plan, .. } => {
                assert!(windows.is_empty());
                assert!(plan.is_none());
            }
            other => panic!("expected ready, got {other:?}"),
        }
    }

    #[test]
    fn claude_token_expired_is_unauthenticated() {
        let body = br#"{"error":{"error_code":"token_expired","message":"expired"}}"#;
        let result =
            claude_from_usage_json(body, datetime!(2026-07-26 18:00:00 UTC), None, None, true);
        match result {
            ProviderResult::Unauthenticated {
                message, retryable, ..
            } => {
                assert!(retryable, "server-reported expiry must be retryable");
                assert!(message.contains("expired"), "message: {message}");
            }
            other => panic!("expected unauthenticated, got {other:?}"),
        }
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
    fn parse_reset_timestamp_accepts_iso_and_epoch() {
        let iso = parse_reset_timestamp("2026-08-01T00:00:00Z");
        assert!(iso.is_some());
        // Epoch seconds (10 digits).
        let secs = parse_reset_timestamp("1785272823");
        assert_eq!(secs.map(|t| t.unix_timestamp()), Some(1_785_272_823));
        // Epoch milliseconds (13 digits).
        let millis = parse_reset_timestamp("1785272823412");
        assert_eq!(millis.map(|t| t.unix_timestamp()), Some(1_785_272_823));
        // Garbage and non-positive are rejected.
        assert!(parse_reset_timestamp("soon").is_none());
        assert!(parse_reset_timestamp("0").is_none());
        assert!(parse_reset_timestamp("-5").is_none());
        assert!(parse_reset_timestamp("").is_none());
    }

    #[test]
    fn claude_window_accepts_epoch_resets_at() {
        let body = br#"{"five_hour":{"utilization":42.0,"resets_at":"1785272823"}}"#;
        let result =
            claude_from_usage_json(body, datetime!(2026-07-28 18:00:00 UTC), None, None, true);
        match result {
            ProviderResult::Ready { windows, .. } => {
                assert_eq!(windows.len(), 1);
                assert_eq!(windows[0].label(), "Session (5h)");
                assert!(
                    windows[0].resets_at().is_some(),
                    "epoch resets_at was dropped"
                );
            }
            other => panic!("expected ready, got {other:?}"),
        }
    }

    #[test]
    fn claude_utilization_one_means_one_percent() {
        // The endpoint reports percent scale: 1.0 is 1%, never 100%.
        let body = br#"{"five_hour":{"utilization":1.0,"resets_at":"2026-07-28T22:00:00Z"}}"#;
        let result =
            claude_from_usage_json(body, datetime!(2026-07-28 18:00:00 UTC), None, None, true);
        match result {
            ProviderResult::Ready { windows, .. } => {
                assert!((windows[0].used_percent() - 1.0).abs() < 0.01);
                assert!((windows[0].remaining_percent() - 99.0).abs() < 0.01);
            }
            other => panic!("expected ready, got {other:?}"),
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
    fn claude_reads_seven_day_oauth_apps_bucket() {
        let body = br#"{"five_hour":{"utilization":10.0,"resets_at":"2026-07-28T20:00:00Z"},"seven_day_oauth_apps":{"utilization":37.0,"resets_at":"2026-08-01T00:00:00Z"}}"#;
        let result =
            claude_from_usage_json(body, datetime!(2026-07-28 18:00:00 UTC), None, None, true);
        match result {
            ProviderResult::Ready { windows, .. } => {
                assert_eq!(windows.len(), 2);
                assert_eq!(windows[1].id(), "weekly");
                assert!((windows[1].used_percent() - 37.0).abs() < 0.01);
                assert!(windows[1].resets_at().is_some());
            }
            other => panic!("expected ready, got {other:?}"),
        }
    }

    #[test]
    fn claude_prefers_oauth_apps_bucket_over_seven_day() {
        let body =
            br#"{"seven_day_oauth_apps":{"utilization":30.0},"seven_day":{"utilization":60.0}}"#;
        let result =
            claude_from_usage_json(body, datetime!(2026-07-28 18:00:00 UTC), None, None, true);
        match result {
            ProviderResult::Ready { windows, .. } => {
                assert_eq!(windows.len(), 1);
                assert_eq!(windows[0].id(), "weekly");
                assert!((windows[0].used_percent() - 30.0).abs() < 0.01);
            }
            other => panic!("expected ready, got {other:?}"),
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
                assert_eq!(windows[0].label(), "Weekly (7d)");
                assert!((windows[0].used_percent() - 1.0).abs() < 0.01);
                assert!(plan.is_some());
            }
            other => panic!("expected ready, got {other:?}"),
        }
    }

    #[test]
    fn codex_other_duration_labels_minutes_only() {
        let body =
            br#"{"primary":{"used_percent":10.0,"window_minutes":45,"resets_at":1785628013}}"#;
        let result = codex_from_rate_limits_json(body, datetime!(2026-07-26 18:00:00 UTC));
        match result {
            ProviderResult::Ready { windows, .. } => {
                assert_eq!(windows[0].id(), "other:45:1");
                assert_eq!(windows[0].label(), "45m");
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
                assert_eq!(windows[0].label(), "Session (5h)");
                assert!((windows[0].used_percent() - 30.0).abs() < 0.01);
                assert_eq!(windows[1].id(), "weekly");
                assert_eq!(windows[1].label(), "Weekly (7d)");
                assert!((windows[1].used_percent() - 40.0).abs() < 0.01);
            }
            other => panic!("expected ready, got {other:?}"),
        }
    }

    #[test]
    fn codex_maps_extra_buckets_individual_limit_and_resets() {
        let json = serde_json::json!({
            "primary": {"usedPercent": 36.0, "windowDurationMins": 10080, "resetsAt": 1791000000},
            "plan_type": "plus",
            "individualLimit": {"remainingPercent": 40.0, "resetsAt": 1791000000},
            "extraBuckets": [{"limitId": "premium",
                "primary": {"usedPercent": 10.0, "windowDurationMins": 10080, "resetsAt": 0}}],
            "rateLimitResetsAvailable": 2
        });
        let bytes = serde_json::to_vec(&json).expect("fixture json");
        let result = codex_from_rate_limits_json(&bytes, datetime!(2026-08-07 12:00:00 UTC));
        assert_no_money(&result);
        match result {
            ProviderResult::Ready {
                windows,
                plan,
                rate_limit_resets_available,
                ..
            } => {
                let ids: Vec<&str> = windows.iter().map(|w| w.id()).collect();
                assert_eq!(ids, vec!["weekly", "codex:premium", "workspace-limit"]);
                assert_eq!(windows[1].label(), "Premium (7d)");
                assert_eq!(windows[2].label(), "Workspace limit");
                assert!((windows[2].remaining_percent() - 40.0).abs() < 0.01);
                assert_eq!(rate_limit_resets_available, Some(2));
                assert_eq!(plan.as_ref().map(|p| p.label.as_str()), Some("Plus"));
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
                assert_eq!(windows[0].label(), "Weekly (7d)");
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
    fn grok_monthly_period_type_names_the_window() {
        let json = br#"{"creditUsagePercent": 20.0,
            "currentPeriod": {"type": "USAGE_PERIOD_TYPE_MONTHLY", "end": "2026-09-01T00:00:00Z"},
            "subscriptionTiers": "pro"}"#;
        let result = grok_from_billing_json(json, None, datetime!(2026-08-07 12:00:00 UTC), true);
        match result {
            ProviderResult::Ready { windows, plan, .. } => {
                assert_eq!(windows[0].id(), "monthly");
                assert_eq!(windows[0].label(), "Monthly");
                assert_eq!(plan.as_ref().map(|p| p.label.as_str()), Some("Pro"));
            }
            other => panic!("expected ready, got {other:?}"),
        }
    }

    #[test]
    fn grok_missing_period_type_stays_weekly() {
        let json = br#"{"creditUsagePercent": 10.0}"#;
        let result = grok_from_billing_json(json, None, datetime!(2026-08-07 12:00:00 UTC), true);
        match result {
            ProviderResult::Ready { windows, .. } => {
                assert_eq!(windows[0].id(), "weekly");
                assert_eq!(windows[0].label(), "Weekly (7d)");
            }
            other => panic!("expected ready, got {other:?}"),
        }
    }

    #[test]
    fn claude_dedupes_model_window_ids_across_sources() {
        // limits[] entry AND legacy seven_day_opus for the same model must not
        // produce duplicate window ids (schema rejects the whole row otherwise).
        let body = br#"{
            "five_hour": {"utilization": 10.0},
            "limits": [{"kind": "seven_day_model", "utilization": 20.0,
                        "scope": {"model": {"id": "opus", "display_name": "Opus"}}}],
            "seven_day_opus": {"utilization": 55.0}
        }"#;
        let result =
            claude_from_usage_json(body, datetime!(2026-07-28 18:00:00 UTC), None, None, true);
        // The whole row must survive schema validation downstream (duplicate
        // window ids make ensure_unique_window_ids reject the entire status).
        let status = crate::status::collect::provider_status_from_result(result.clone());
        assert!(status.is_ok(), "row failed schema validation: {status:?}");
        match result {
            ProviderResult::Ready { windows, .. } => {
                let opus: Vec<_> = windows
                    .iter()
                    .filter(|w| w.id() == "weekly-model:opus")
                    .collect();
                assert_eq!(opus.len(), 1, "duplicate weekly-model:opus windows");
                // limits[] (dynamic) wins over the legacy field.
                assert!((opus[0].used_percent() - 20.0).abs() < 0.01);
            }
            other => panic!("expected ready, got {other:?}"),
        }
    }

    #[test]
    fn claude_reads_current_limits_shape_with_fable() {
        // Captured live 2026-08-01: limits[] entries carry `percent` (not
        // `utilization`), kinds are session/weekly_all/weekly_scoped, the
        // scoped entry names the model only via display_name, and
        // seven_day_oauth_apps is null. The Fable window must come through
        // and the grouped entries must not duplicate session/weekly.
        let body = br#"{
            "five_hour": {"utilization": 10.0, "resets_at": "2026-08-02T02:00:00+00:00"},
            "seven_day": {"utilization": 12.0, "resets_at": "2026-08-07T12:00:00+00:00"},
            "seven_day_oauth_apps": null,
            "seven_day_cowork": null,
            "limits": [
                {"kind": "session", "group": "session", "percent": 10,
                 "resets_at": "2026-08-02T02:00:00+00:00", "scope": null, "is_active": false},
                {"kind": "weekly_all", "group": "weekly", "percent": 12,
                 "resets_at": "2026-08-07T12:00:00+00:00", "scope": null, "is_active": true},
                {"kind": "weekly_scoped", "group": "weekly", "percent": 10,
                 "resets_at": "2026-08-07T12:00:00+00:00",
                 "scope": {"model": {"id": null, "display_name": "Fable"}, "surface": null},
                 "is_active": false}
            ]
        }"#;
        let result =
            claude_from_usage_json(body, datetime!(2026-08-01 21:00:00 UTC), None, None, true);
        match result {
            ProviderResult::Ready { windows, .. } => {
                let ids: Vec<_> = windows.iter().map(|w| w.id().to_owned()).collect();
                assert_eq!(
                    ids,
                    vec!["session", "weekly", "weekly-model:fable"],
                    "got {ids:?}"
                );
                let fable = &windows[2];
                assert_eq!(fable.label(), "Fable");
                assert!((fable.used_percent() - 10.0).abs() < 0.01);
                assert!(fable.resets_at().is_some(), "Fable resets_at was dropped");
            }
            other => panic!("expected ready, got {other:?}"),
        }
    }

    #[test]
    fn claude_limits_only_payload_still_yields_windows() {
        // The API already retired seven_day_oauth_apps once; if the legacy
        // top-level fields go next, limits[] must be able to carry the
        // canonical windows alone.
        let body = br#"{
            "limits": [
                {"kind": "session", "percent": 7.0, "resets_at": "2026-08-02T02:00:00+00:00"},
                {"kind": "weekly_all", "percent": 12.0, "resets_at": "2026-08-07T12:00:00+00:00"}
            ]
        }"#;
        let result =
            claude_from_usage_json(body, datetime!(2026-08-01 21:00:00 UTC), None, None, true);
        match result {
            ProviderResult::Ready { windows, .. } => {
                let ids: Vec<_> = windows.iter().map(|w| w.id().to_owned()).collect();
                assert_eq!(ids, vec!["session", "weekly"], "got {ids:?}");
                assert_eq!(windows[0].label(), LABEL_SESSION);
                assert!((windows[0].used_percent() - 7.0).abs() < 0.01);
            }
            other => panic!("expected ready, got {other:?}"),
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

    #[test]
    fn format_plan_label_title_cases_underscore_ids() {
        assert_eq!(format_plan_label("pro"), "Pro");
        assert_eq!(
            format_plan_label("self_serve_business_usage_based"),
            "Self Serve Business Usage Based"
        );
        assert_eq!(format_plan_label(""), "");
        assert_eq!(format_plan_label("_x_"), "X");
    }
}
