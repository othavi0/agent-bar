//! Golden snapshots de paridade com o TS. Os valores de referência vêm do
//! `.snap` do TS (sanitizado). Clock fixo + filters reproduzem a sanitização.
//!
//! IMPORTANTE: os goldens NUNCA são gravados via `cargo insta accept`.
//! Os valores esperados são copiados do TS snap e colados inline.

use agent_bar::config::Paths;
use agent_bar::formatters::clock::Clock;
use agent_bar::formatters::terminal::format_for_terminal;
use agent_bar::formatters::waybar::{format_for_waybar, format_provider_for_waybar};
use agent_bar::providers::types::{
    AllQuotas, AmpQuotaExtra, ClaudeQuotaExtra, CodexQuotaExtra, ExtraUsage, ModelWindows,
    ProviderExtra, ProviderQuota, QuotaWindow,
};
use agent_bar::settings::{load, DisplayMode, Settings};
use indexmap::IndexMap;
use std::collections::BTreeMap;
use std::path::PathBuf;
use tempfile::tempdir;
use time::macros::datetime;

// ---------------------------------------------------------------------------
// Constantes de tempo
// ---------------------------------------------------------------------------

const FIXED_FETCHED_AT: &str = "2026-03-28T12:00:00.000Z";
const FIXED_RESET: &str = "2026-03-28T14:00:00.000Z";

// ---------------------------------------------------------------------------
// Helpers de infra
// ---------------------------------------------------------------------------

fn clk() -> Clock {
    Clock {
        now: datetime!(2026-03-28 12:00:00 UTC),
        local_offset: time::UtcOffset::UTC,
    }
}

fn settings() -> Settings {
    let dir = tempdir().unwrap();
    load(&Paths {
        cache_dir: dir.path().join("cache"),
        config_dir: dir.path().join("config"),
        claude_credentials: PathBuf::new(),
        codex_auth: PathBuf::new(),
        codex_sessions: PathBuf::new(),
        amp_settings: PathBuf::new(),
        amp_threads: PathBuf::new(),
    })
}

fn wrap(providers: Vec<ProviderQuota>) -> AllQuotas {
    AllQuotas {
        providers,
        fetched_at: FIXED_FETCHED_AT.into(),
    }
}

fn qw(remaining: f64, resets_at: &str, window_minutes: Option<i64>) -> QuotaWindow {
    QuotaWindow {
        remaining,
        resets_at: Some(resets_at.into()),
        window_minutes,
        used: None,
        severity: None,
    }
}

// Filters de sanitização (mesmos regex/ordem do TS sanitize()).
fn with_filters<F: FnOnce()>(f: F) {
    let mut s = insta::Settings::clone_current();
    s.add_filter(r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d+)?Z", "__ISO__");
    s.add_filter(r"\d+d \d{2}h", "__DH__");
    s.add_filter(r"\d{1,2}h \d{2}m", "__HM__");
    s.add_filter(r"\(\d{2}:\d{2}\)", "(__:__)");
    s.add_filter(r"\d+[hm] ago", "__AGO__");
    s.add_filter("just now", "__AGO__");
    s.bind(f);
}

// Para terminal, strip ANSI antes das demais substituições.
fn with_filters_terminal<F: FnOnce()>(f: F) {
    let mut s = insta::Settings::clone_current();
    s.add_filter(r"\x1b\[[0-9;]*m", "");
    s.add_filter(r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d+)?Z", "__ISO__");
    s.add_filter(r"\d+d \d{2}h", "__DH__");
    s.add_filter(r"\d{1,2}h \d{2}m", "__HM__");
    s.add_filter(r"\(\d{2}:\d{2}\)", "(__:__)");
    s.add_filter(r"\d+[hm] ago", "__AGO__");
    s.add_filter("just now", "__AGO__");
    s.bind(f);
}

// ---------------------------------------------------------------------------
// Factories — espelham `tests/formatters-snapshot.test.ts`
// ---------------------------------------------------------------------------

fn claude_healthy() -> ProviderQuota {
    ProviderQuota {
        provider: "claude".into(),
        display_name: "Claude".into(),
        available: true,
        account: None,
        plan: Some("Pro".into()),
        plan_type: None,
        primary: Some(qw(75.0, FIXED_RESET, Some(300))),
        secondary: Some(qw(90.0, FIXED_RESET, Some(10080))),
        models: None,
        extra: None,
        error: None,
    }
}

fn claude_error() -> ProviderQuota {
    ProviderQuota {
        provider: "claude".into(),
        display_name: "Claude".into(),
        available: false,
        account: None,
        plan: None,
        plan_type: None,
        primary: None,
        secondary: None,
        models: None,
        extra: None,
        error: Some("Token expired. Open `agent-bar menu` and choose Provider login.".into()),
    }
}

fn codex_healthy() -> ProviderQuota {
    let mut models_detailed = BTreeMap::new();
    models_detailed.insert(
        "Codex".to_string(),
        ModelWindows {
            five_hour: Some(qw(60.0, FIXED_RESET, Some(300))),
            seven_day: Some(qw(85.0, FIXED_RESET, Some(10080))),
            other: None,
        },
    );
    let mut models = IndexMap::new();
    models.insert("Codex".to_string(), qw(60.0, FIXED_RESET, Some(300)));

    ProviderQuota {
        provider: "codex".into(),
        display_name: "Codex".into(),
        available: true,
        account: None,
        plan: Some("Pro".into()),
        plan_type: Some("pro".into()),
        primary: Some(qw(60.0, FIXED_RESET, Some(300))),
        secondary: Some(qw(85.0, FIXED_RESET, Some(10080))),
        models: Some(models),
        extra: Some(ProviderExtra::Codex(CodexQuotaExtra {
            models_detailed: Some(models_detailed),
            extra_usage: None,
        })),
        error: None,
    }
}

fn codex_error() -> ProviderQuota {
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
        error: Some("No session data found".into()),
    }
}

fn amp_healthy() -> ProviderQuota {
    let mut models = IndexMap::new();
    models.insert("Free Tier".to_string(), qw(70.0, FIXED_RESET, None));

    let mut meta = BTreeMap::new();
    meta.insert("freeRemaining".to_string(), "$3.50".to_string());
    meta.insert("freeTotal".to_string(), "$5.00".to_string());
    meta.insert("replenishRate".to_string(), "+$0.25/hr".to_string());

    ProviderQuota {
        provider: "amp".into(),
        display_name: "Amp".into(),
        available: true,
        account: None,
        plan: None,
        plan_type: None,
        primary: Some(qw(70.0, FIXED_RESET, None)),
        secondary: None,
        models: Some(models),
        extra: Some(ProviderExtra::Amp(AmpQuotaExtra { meta: Some(meta) })),
        error: None,
    }
}

fn amp_error() -> ProviderQuota {
    ProviderQuota {
        provider: "amp".into(),
        display_name: "Amp".into(),
        available: false,
        account: None,
        plan: None,
        plan_type: None,
        primary: None,
        secondary: None,
        models: None,
        extra: None,
        error: Some("Amp CLI not installed. Right-click to install and log in.".into()),
    }
}

fn amp_with_account() -> ProviderQuota {
    let mut p = amp_healthy();
    p.account = Some("user@example.com".into());
    p
}

fn claude_with_extras() -> ProviderQuota {
    let mut weekly_models = IndexMap::new();
    weekly_models.insert(
        "claude-opus-4-5".to_string(),
        qw(40.0, FIXED_RESET, Some(10080)),
    );
    weekly_models.insert(
        "claude-sonnet-4-5".to_string(),
        qw(65.0, FIXED_RESET, Some(10080)),
    );

    ProviderQuota {
        provider: "claude".into(),
        display_name: "Claude".into(),
        available: true,
        account: None,
        plan: Some("Pro".into()),
        plan_type: None,
        primary: Some(qw(60.0, FIXED_RESET, Some(300))),
        secondary: Some(qw(50.0, FIXED_RESET, Some(10080))),
        models: None,
        extra: Some(ProviderExtra::Claude(ClaudeQuotaExtra {
            weekly_models: Some(weekly_models),
            extra_usage: Some(ExtraUsage {
                enabled: true,
                remaining: 55.0,
                limit: 5000.0,
                used: 2250.0,
            }),
        })),
        error: None,
    }
}

fn amp_with_credits() -> ProviderQuota {
    let mut models = IndexMap::new();
    models.insert("Free Tier".to_string(), qw(30.0, FIXED_RESET, None));
    models.insert("Credits".to_string(), qw(75.0, FIXED_RESET, None));

    let mut meta = BTreeMap::new();
    meta.insert("freeRemaining".to_string(), "$1.50".to_string());
    meta.insert("freeTotal".to_string(), "$5.00".to_string());
    meta.insert("replenishRate".to_string(), "+$0.25/hr".to_string());
    meta.insert("creditsBalance".to_string(), "$7.50".to_string());

    ProviderQuota {
        provider: "amp".into(),
        display_name: "Amp".into(),
        available: true,
        account: None,
        plan: None,
        plan_type: None,
        primary: Some(qw(30.0, FIXED_RESET, None)),
        secondary: None,
        models: Some(models),
        extra: Some(ProviderExtra::Amp(AmpQuotaExtra { meta: Some(meta) })),
        error: None,
    }
}

fn amp_unknown_models() -> ProviderQuota {
    let mut models = IndexMap::new();
    models.insert("Custom Plan A".to_string(), qw(45.0, FIXED_RESET, None));
    models.insert("Custom Plan B".to_string(), qw(80.0, FIXED_RESET, None));

    ProviderQuota {
        provider: "amp".into(),
        display_name: "Amp".into(),
        available: true,
        account: None,
        plan: None,
        plan_type: None,
        primary: Some(qw(45.0, FIXED_RESET, None)),
        secondary: None,
        models: Some(models),
        extra: Some(ProviderExtra::Amp(AmpQuotaExtra {
            meta: Some(BTreeMap::new()),
        })),
        error: None,
    }
}

// ---------------------------------------------------------------------------
// Terminal — remaining
// ---------------------------------------------------------------------------

#[test]
fn terminal_claude_healthy() {
    with_filters_terminal(|| {
        let out = format_for_terminal(
            &clk(),
            &wrap(vec![claude_healthy()]),
            &settings(),
            DisplayMode::Remaining,
            false,
        );
        insta::assert_snapshot!(out, @r"
┏━ Claude ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
┃
┣━ ◆ 5-hour limit (shared)
┃  ● All Models           ███████████████░░░░░  75% → __HM__ (__:__)
┃
┣━ ◆ Weekly limit (shared)
┃  ● All Models           ██████████████████░░  90% → __HM__ (__:__)
┃
┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    });
}

#[test]
fn terminal_claude_error() {
    with_filters_terminal(|| {
        let out = format_for_terminal(
            &clk(),
            &wrap(vec![claude_error()]),
            &settings(),
            DisplayMode::Remaining,
            false,
        );
        insta::assert_snapshot!(out, @r"
┏━ Claude ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
┃
┃  ⚠️ Token expired. Open `agent-bar menu` and choose Provider login.
┃
┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    });
}

#[test]
fn terminal_codex_healthy() {
    with_filters_terminal(|| {
        let out = format_for_terminal(
            &clk(),
            &wrap(vec![codex_healthy()]),
            &settings(),
            DisplayMode::Remaining,
            false,
        );
        insta::assert_snapshot!(out, @r"
┏━ Codex ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
┃
┃  Plan: Pro
┃
┣━ ◆ 5-hour limit
┃  ● Codex                ████████████░░░░░░░░  60% → __HM__ (__:__)
┃
┣━ ◆ 7-day limit
┃  ● Codex                █████████████████░░░  85% → __HM__ (__:__)
┃
┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    });
}

#[test]
fn terminal_codex_error() {
    with_filters_terminal(|| {
        let out = format_for_terminal(
            &clk(),
            &wrap(vec![codex_error()]),
            &settings(),
            DisplayMode::Remaining,
            false,
        );
        insta::assert_snapshot!(out, @r"
┏━ Codex ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
┃
┃  ⚠️ No session data found
┃
┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    });
}

#[test]
fn terminal_amp_error() {
    with_filters_terminal(|| {
        let out = format_for_terminal(
            &clk(),
            &wrap(vec![amp_error()]),
            &settings(),
            DisplayMode::Remaining,
            false,
        );
        insta::assert_snapshot!(out, @r"
┏━ Amp ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
┃
┃  ⚠️ Amp CLI not installed. Right-click to install and log in.
┃
┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    });
}

#[test]
fn terminal_amp_healthy() {
    with_filters_terminal(|| {
        let out = format_for_terminal(
            &clk(),
            &wrap(vec![amp_healthy()]),
            &settings(),
            DisplayMode::Remaining,
            false,
        );
        insta::assert_snapshot!(out, @r"
┏━ Amp ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
┃
┣━ ◆ Free Tier
┃  ● ██████████████░░░░░░  70%
┃  ├─ +$0.25/hr  ( $3.50 / $5.00 )
┃  └─ Full in __HM__  (__:__)
┃
┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    });
}

#[test]
fn terminal_all_combined() {
    with_filters_terminal(|| {
        let out = format_for_terminal(
            &clk(),
            &wrap(vec![claude_healthy(), codex_healthy(), amp_healthy()]),
            &settings(),
            DisplayMode::Remaining,
            false,
        );
        insta::assert_snapshot!(out, @r"
┏━ Claude ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
┃
┣━ ◆ 5-hour limit (shared)
┃  ● All Models           ███████████████░░░░░  75% → __HM__ (__:__)
┃
┣━ ◆ Weekly limit (shared)
┃  ● All Models           ██████████████████░░  90% → __HM__ (__:__)
┃
┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

┏━ Codex ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
┃
┃  Plan: Pro
┃
┣━ ◆ 5-hour limit
┃  ● Codex                ████████████░░░░░░░░  60% → __HM__ (__:__)
┃
┣━ ◆ 7-day limit
┃  ● Codex                █████████████████░░░  85% → __HM__ (__:__)
┃
┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

┏━ Amp ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
┃
┣━ ◆ Free Tier
┃  ● ██████████████░░░░░░  70%
┃  ├─ +$0.25/hr  ( $3.50 / $5.00 )
┃  └─ Full in __HM__  (__:__)
┃
┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    });
}

#[test]
fn terminal_empty() {
    with_filters_terminal(|| {
        let out = format_for_terminal(
            &clk(),
            &wrap(vec![]),
            &settings(),
            DisplayMode::Remaining,
            false,
        );
        insta::assert_snapshot!(out, @"No providers connected");
    });
}

// ---------------------------------------------------------------------------
// Terminal — used
// ---------------------------------------------------------------------------

#[test]
fn terminal_claude_healthy_used() {
    with_filters_terminal(|| {
        let out = format_for_terminal(
            &clk(),
            &wrap(vec![claude_healthy()]),
            &settings(),
            DisplayMode::Used,
            false,
        );
        insta::assert_snapshot!(out, @r"
┏━ Claude ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
┃
┣━ ◆ 5-hour limit (shared)
┃  ● All Models           █████░░░░░░░░░░░░░░░  25% → __HM__ (__:__)
┃
┣━ ◆ Weekly limit (shared)
┃  ● All Models           ██░░░░░░░░░░░░░░░░░░  10% → __HM__ (__:__)
┃
┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    });
}

#[test]
fn terminal_codex_healthy_used() {
    with_filters_terminal(|| {
        let out = format_for_terminal(
            &clk(),
            &wrap(vec![codex_healthy()]),
            &settings(),
            DisplayMode::Used,
            false,
        );
        insta::assert_snapshot!(out, @r"
┏━ Codex ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
┃
┃  Plan: Pro
┃
┣━ ◆ 5-hour limit
┃  ● Codex                ████████░░░░░░░░░░░░  40% → __HM__ (__:__)
┃
┣━ ◆ 7-day limit
┃  ● Codex                ███░░░░░░░░░░░░░░░░░  15% → __HM__ (__:__)
┃
┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    });
}

#[test]
fn terminal_amp_healthy_used() {
    with_filters_terminal(|| {
        let out = format_for_terminal(
            &clk(),
            &wrap(vec![amp_healthy()]),
            &settings(),
            DisplayMode::Used,
            false,
        );
        insta::assert_snapshot!(out, @r"
┏━ Amp ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
┃
┣━ ◆ Free Tier
┃  ● ██████░░░░░░░░░░░░░░  30%
┃  ├─ +$0.25/hr  ( $3.50 / $5.00 )
┃  └─ Resets in __HM__  (__:__)
┃
┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    });
}

#[test]
fn terminal_all_combined_used() {
    with_filters_terminal(|| {
        let out = format_for_terminal(
            &clk(),
            &wrap(vec![claude_healthy(), codex_healthy(), amp_healthy()]),
            &settings(),
            DisplayMode::Used,
            false,
        );
        insta::assert_snapshot!(out, @r"
┏━ Claude ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
┃
┣━ ◆ 5-hour limit (shared)
┃  ● All Models           █████░░░░░░░░░░░░░░░  25% → __HM__ (__:__)
┃
┣━ ◆ Weekly limit (shared)
┃  ● All Models           ██░░░░░░░░░░░░░░░░░░  10% → __HM__ (__:__)
┃
┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

┏━ Codex ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
┃
┃  Plan: Pro
┃
┣━ ◆ 5-hour limit
┃  ● Codex                ████████░░░░░░░░░░░░  40% → __HM__ (__:__)
┃
┣━ ◆ 7-day limit
┃  ● Codex                ███░░░░░░░░░░░░░░░░░  15% → __HM__ (__:__)
┃
┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

┏━ Amp ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
┃
┣━ ◆ Free Tier
┃  ● ██████░░░░░░░░░░░░░░  30%
┃  ├─ +$0.25/hr  ( $3.50 / $5.00 )
┃  └─ Resets in __HM__  (__:__)
┃
┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    });
}

// ---------------------------------------------------------------------------
// Terminal — rich fixtures (C3)
// ---------------------------------------------------------------------------

#[test]
fn terminal_claude_with_extras() {
    with_filters_terminal(|| {
        let out = format_for_terminal(
            &clk(),
            &wrap(vec![claude_with_extras()]),
            &settings(),
            DisplayMode::Remaining,
            false,
        );
        insta::assert_snapshot!(out, @r"
┏━ Claude ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
┃
┣━ ◆ 5-hour limit (shared)
┃  ● All Models           ████████████░░░░░░░░  60% → __HM__ (__:__)
┃
┣━ ◆ Weekly per model
┃  ● claude-opus-4-5      ████████░░░░░░░░░░░░  40% → __HM__ (__:__)
┃  ● claude-sonnet-4-5    █████████████░░░░░░░  65% → __HM__ (__:__)
┃
┣━ ◆ Weekly limit (shared)
┃  ● All Models           ██████████░░░░░░░░░░  50% → __HM__ (__:__)
┃
┣━ ◆ Extra Usage
┃  ● Budget               ███████████░░░░░░░░░  55% $22.50/$50.00
┃
┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    });
}

#[test]
fn terminal_amp_with_credits() {
    with_filters_terminal(|| {
        let out = format_for_terminal(
            &clk(),
            &wrap(vec![amp_with_credits()]),
            &settings(),
            DisplayMode::Remaining,
            false,
        );
        insta::assert_snapshot!(out, @r"
┏━ Amp ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
┃
┣━ ◆ Free Tier
┃  ● ██████░░░░░░░░░░░░░░  30%
┃  ├─ +$0.25/hr  ( $1.50 / $5.00 )
┃  └─ Full in __HM__  (__:__)
┃
┣━ ◆ Credits
┃  ● $7.50
┃
┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    });
}

#[test]
fn terminal_amp_unknown_models() {
    with_filters_terminal(|| {
        let out = format_for_terminal(
            &clk(),
            &wrap(vec![amp_unknown_models()]),
            &settings(),
            DisplayMode::Remaining,
            false,
        );
        insta::assert_snapshot!(out, @r"
┏━ Amp ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
┃
┣━ ◆ Usage
┃  ● Custom Plan A        █████████░░░░░░░░░░░  45%
┃  ● Custom Plan B        ████████████████░░░░  80%
┃
┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    });
}

// ---------------------------------------------------------------------------
// Terminal — account (C1)
// ---------------------------------------------------------------------------

#[test]
fn terminal_amp_with_account() {
    with_filters_terminal(|| {
        let out = format_for_terminal(
            &clk(),
            &wrap(vec![amp_with_account()]),
            &settings(),
            DisplayMode::Remaining,
            false,
        );
        insta::assert_snapshot!(out, @r"
┏━ Amp ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
┃
┣━ ◆ Free Tier
┃  ● ██████████████░░░░░░  70%
┃  ├─ +$0.25/hr  ( $3.50 / $5.00 )
┃  └─ Full in __HM__  (__:__)
┃
┃  Account: user@example.com
┃
┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    });
}

// ---------------------------------------------------------------------------
// Waybar aggregate — remaining
// ---------------------------------------------------------------------------

#[test]
fn waybar_claude_healthy_text() {
    with_filters(|| {
        let out = format_for_waybar(
            &clk(),
            &wrap(vec![claude_healthy()]),
            &settings(),
            DisplayMode::Remaining,
        );
        insta::assert_snapshot!(out.text, @"<span foreground='#98c379'>75%</span>");
    });
}

#[test]
fn waybar_claude_healthy_tooltip() {
    with_filters(|| {
        let out = format_for_waybar(
            &clk(),
            &wrap(vec![claude_healthy()]),
            &settings(),
            DisplayMode::Remaining,
        );
        insta::assert_snapshot!(out.tooltip, @r"
<span foreground='#d19a66'>┏━</span> <span foreground='#d19a66' weight='bold'>Claude · Pro</span> <span foreground='#d19a66'>━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━</span>
<span foreground='#d19a66'>┃</span>
<span foreground='#d19a66'>┣━</span> <span foreground='#d19a66' weight='bold'>◆ 5-hour limit (shared)</span>
<span foreground='#d19a66'>┃</span>  <span foreground='#98c379'>●</span> <span foreground='#e2e8f0'>All Models          </span> <span foreground='#98c379'>███████████████</span><span foreground='#8b95a5'>░░░░░</span> <span foreground='#98c379'> 75%</span> <span foreground='#56b6c2'>→ __HM__ (__:__)</span>
<span foreground='#d19a66'>┃</span>
<span foreground='#d19a66'>┣━</span> <span foreground='#d19a66' weight='bold'>◆ Weekly limit (shared)</span>
<span foreground='#d19a66'>┃</span>  <span foreground='#98c379'>●</span> <span foreground='#e2e8f0'>All Models          </span> <span foreground='#98c379'>██████████████████</span><span foreground='#8b95a5'>░░</span> <span foreground='#98c379'> 90%</span> <span foreground='#56b6c2'>→ __HM__ (__:__)</span>
<span foreground='#d19a66'>┃</span>
<span foreground='#d19a66'>┗━━━━━━━━━━━━━━━━━━</span><span foreground='#8b95a5'> cached · __AGO__ </span><span foreground='#d19a66'>━━━━━━━━━━━━━━━━━━</span>");
    });
}

#[test]
fn waybar_claude_healthy_class() {
    let out = format_for_waybar(
        &clk(),
        &wrap(vec![claude_healthy()]),
        &settings(),
        DisplayMode::Remaining,
    );
    assert_eq!(out.class, "agent-bar claude-ok");
}

#[test]
fn waybar_claude_error_text() {
    with_filters(|| {
        let out = format_for_waybar(
            &clk(),
            &wrap(vec![claude_error()]),
            &settings(),
            DisplayMode::Remaining,
        );
        insta::assert_snapshot!(out.text, @"<span foreground='#8b95a5'>No Providers</span>");
    });
}

#[test]
fn waybar_claude_error_tooltip() {
    with_filters(|| {
        let out = format_for_waybar(
            &clk(),
            &wrap(vec![claude_error()]),
            &settings(),
            DisplayMode::Remaining,
        );
        insta::assert_snapshot!(out.tooltip, @r"
<span foreground='#d19a66'>┏━</span> <span foreground='#d19a66' weight='bold'>Claude</span> <span foreground='#d19a66'>━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━</span>
<span foreground='#d19a66'>┃</span>
<span foreground='#d19a66'>┃</span>  <span foreground='#e88b93'>⚠️ Token expired. Open `agent-bar menu` and choose Provider login.</span>
<span foreground='#d19a66'>┃</span>
<span foreground='#d19a66'>┗━━━━━━━━━━━━━━━━━━</span><span foreground='#8b95a5'> cached · __AGO__ </span><span foreground='#d19a66'>━━━━━━━━━━━━━━━━━━</span>");
    });
}

#[test]
fn waybar_claude_error_class() {
    let out = format_for_waybar(
        &clk(),
        &wrap(vec![claude_error()]),
        &settings(),
        DisplayMode::Remaining,
    );
    assert_eq!(out.class, "agent-bar");
}

#[test]
fn waybar_codex_healthy_text() {
    with_filters(|| {
        let out = format_for_waybar(
            &clk(),
            &wrap(vec![codex_healthy()]),
            &settings(),
            DisplayMode::Remaining,
        );
        insta::assert_snapshot!(out.text, @"<span foreground='#98c379'>60%</span>");
    });
}

#[test]
fn waybar_codex_healthy_tooltip() {
    with_filters(|| {
        let out = format_for_waybar(
            &clk(),
            &wrap(vec![codex_healthy()]),
            &settings(),
            DisplayMode::Remaining,
        );
        insta::assert_snapshot!(out.tooltip, @r"
<span foreground='#98c379'>┏━</span> <span foreground='#98c379' weight='bold'>Codex · Pro</span> <span foreground='#98c379'>━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━</span>
<span foreground='#98c379'>┃</span>
<span foreground='#98c379'>┃</span>
<span foreground='#98c379'>┣━</span> <span foreground='#98c379' weight='bold'>◆ 5-hour limit</span>
<span foreground='#98c379'>┃</span>  <span foreground='#98c379'>●</span> <span foreground='#e2e8f0'>Codex               </span> <span foreground='#98c379'>████████████</span><span foreground='#8b95a5'>░░░░░░░░</span> <span foreground='#98c379'> 60%</span> <span foreground='#56b6c2'>→ __HM__ (__:__)</span>
<span foreground='#98c379'>┃</span>
<span foreground='#98c379'>┣━</span> <span foreground='#98c379' weight='bold'>◆ 7-day limit</span>
<span foreground='#98c379'>┃</span>  <span foreground='#98c379'>●</span> <span foreground='#e2e8f0'>Codex               </span> <span foreground='#98c379'>█████████████████</span><span foreground='#8b95a5'>░░░</span> <span foreground='#98c379'> 85%</span> <span foreground='#56b6c2'>→ __HM__ (__:__)</span>
<span foreground='#98c379'>┃</span>
<span foreground='#98c379'>┗━━━━━━━━━━━━━━━━━━</span><span foreground='#8b95a5'> cached · __AGO__ </span><span foreground='#98c379'>━━━━━━━━━━━━━━━━━━</span>");
    });
}

#[test]
fn waybar_codex_healthy_class() {
    let out = format_for_waybar(
        &clk(),
        &wrap(vec![codex_healthy()]),
        &settings(),
        DisplayMode::Remaining,
    );
    assert_eq!(out.class, "agent-bar codex-ok");
}

#[test]
fn waybar_amp_healthy_text() {
    with_filters(|| {
        let out = format_for_waybar(
            &clk(),
            &wrap(vec![amp_healthy()]),
            &settings(),
            DisplayMode::Remaining,
        );
        insta::assert_snapshot!(out.text, @"<span foreground='#98c379'>70%</span>");
    });
}

#[test]
fn waybar_amp_healthy_tooltip() {
    with_filters(|| {
        let out = format_for_waybar(
            &clk(),
            &wrap(vec![amp_healthy()]),
            &settings(),
            DisplayMode::Remaining,
        );
        insta::assert_snapshot!(out.tooltip, @r"
<span foreground='#c678dd'>┏━</span> <span foreground='#c678dd' weight='bold'>Amp</span> <span foreground='#c678dd'>━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━</span>
<span foreground='#c678dd'>┃</span>
<span foreground='#c678dd'>┣━</span> <span foreground='#c678dd' weight='bold'>◆ Free Tier</span>
<span foreground='#c678dd'>┃</span>  <span foreground='#98c379'>●</span> <span foreground='#98c379'>██████████████</span><span foreground='#8b95a5'>░░░░░░</span> <span foreground='#98c379'> 70%</span>  <span foreground='#56b6c2'>→ Full in __HM__ (__:__)</span>
<span foreground='#c678dd'>┃</span>  <span foreground='#8b95a5'>○</span> <span foreground='#56b6c2'>+$0.25/hr</span><span foreground='#8b95a5'>  |  </span><span foreground='#c0c9d4'>$3.50 / $5.00</span>
<span foreground='#c678dd'>┃</span>
<span foreground='#c678dd'>┗━━━━━━━━━━━━━━━━━━</span><span foreground='#8b95a5'> cached · __AGO__ </span><span foreground='#c678dd'>━━━━━━━━━━━━━━━━━━</span>");
    });
}

#[test]
fn waybar_amp_healthy_class() {
    let out = format_for_waybar(
        &clk(),
        &wrap(vec![amp_healthy()]),
        &settings(),
        DisplayMode::Remaining,
    );
    assert_eq!(out.class, "agent-bar amp-ok");
}

#[test]
fn waybar_all_combined_text() {
    with_filters(|| {
        let out = format_for_waybar(
            &clk(),
            &wrap(vec![claude_healthy(), codex_healthy(), amp_healthy()]),
            &settings(),
            DisplayMode::Remaining,
        );
        insta::assert_snapshot!(out.text, @"<span foreground='#98c379'>75%</span> <span foreground='#8b95a5'>│</span> <span foreground='#98c379'>60%</span> <span foreground='#8b95a5'>│</span> <span foreground='#98c379'>70%</span>");
    });
}

#[test]
fn waybar_all_combined_tooltip() {
    with_filters(|| {
        let out = format_for_waybar(
            &clk(),
            &wrap(vec![claude_healthy(), codex_healthy(), amp_healthy()]),
            &settings(),
            DisplayMode::Remaining,
        );
        insta::assert_snapshot!(out.tooltip, @r"
<span foreground='#d19a66'>┏━</span> <span foreground='#d19a66' weight='bold'>Claude · Pro</span> <span foreground='#d19a66'>━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━</span>
<span foreground='#d19a66'>┃</span>
<span foreground='#d19a66'>┣━</span> <span foreground='#d19a66' weight='bold'>◆ 5-hour limit (shared)</span>
<span foreground='#d19a66'>┃</span>  <span foreground='#98c379'>●</span> <span foreground='#e2e8f0'>All Models          </span> <span foreground='#98c379'>███████████████</span><span foreground='#8b95a5'>░░░░░</span> <span foreground='#98c379'> 75%</span> <span foreground='#56b6c2'>→ __HM__ (__:__)</span>
<span foreground='#d19a66'>┃</span>
<span foreground='#d19a66'>┣━</span> <span foreground='#d19a66' weight='bold'>◆ Weekly limit (shared)</span>
<span foreground='#d19a66'>┃</span>  <span foreground='#98c379'>●</span> <span foreground='#e2e8f0'>All Models          </span> <span foreground='#98c379'>██████████████████</span><span foreground='#8b95a5'>░░</span> <span foreground='#98c379'> 90%</span> <span foreground='#56b6c2'>→ __HM__ (__:__)</span>
<span foreground='#d19a66'>┃</span>
<span foreground='#d19a66'>┗━━━━━━━━━━━━━━━━━━</span><span foreground='#8b95a5'> cached · __AGO__ </span><span foreground='#d19a66'>━━━━━━━━━━━━━━━━━━</span>

<span foreground='#98c379'>┏━</span> <span foreground='#98c379' weight='bold'>Codex · Pro</span> <span foreground='#98c379'>━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━</span>
<span foreground='#98c379'>┃</span>
<span foreground='#98c379'>┃</span>
<span foreground='#98c379'>┣━</span> <span foreground='#98c379' weight='bold'>◆ 5-hour limit</span>
<span foreground='#98c379'>┃</span>  <span foreground='#98c379'>●</span> <span foreground='#e2e8f0'>Codex               </span> <span foreground='#98c379'>████████████</span><span foreground='#8b95a5'>░░░░░░░░</span> <span foreground='#98c379'> 60%</span> <span foreground='#56b6c2'>→ __HM__ (__:__)</span>
<span foreground='#98c379'>┃</span>
<span foreground='#98c379'>┣━</span> <span foreground='#98c379' weight='bold'>◆ 7-day limit</span>
<span foreground='#98c379'>┃</span>  <span foreground='#98c379'>●</span> <span foreground='#e2e8f0'>Codex               </span> <span foreground='#98c379'>█████████████████</span><span foreground='#8b95a5'>░░░</span> <span foreground='#98c379'> 85%</span> <span foreground='#56b6c2'>→ __HM__ (__:__)</span>
<span foreground='#98c379'>┃</span>
<span foreground='#98c379'>┗━━━━━━━━━━━━━━━━━━</span><span foreground='#8b95a5'> cached · __AGO__ </span><span foreground='#98c379'>━━━━━━━━━━━━━━━━━━</span>

<span foreground='#c678dd'>┏━</span> <span foreground='#c678dd' weight='bold'>Amp</span> <span foreground='#c678dd'>━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━</span>
<span foreground='#c678dd'>┃</span>
<span foreground='#c678dd'>┣━</span> <span foreground='#c678dd' weight='bold'>◆ Free Tier</span>
<span foreground='#c678dd'>┃</span>  <span foreground='#98c379'>●</span> <span foreground='#98c379'>██████████████</span><span foreground='#8b95a5'>░░░░░░</span> <span foreground='#98c379'> 70%</span>  <span foreground='#56b6c2'>→ Full in __HM__ (__:__)</span>
<span foreground='#c678dd'>┃</span>  <span foreground='#8b95a5'>○</span> <span foreground='#56b6c2'>+$0.25/hr</span><span foreground='#8b95a5'>  |  </span><span foreground='#c0c9d4'>$3.50 / $5.00</span>
<span foreground='#c678dd'>┃</span>
<span foreground='#c678dd'>┗━━━━━━━━━━━━━━━━━━</span><span foreground='#8b95a5'> cached · __AGO__ </span><span foreground='#c678dd'>━━━━━━━━━━━━━━━━━━</span>");
    });
}

#[test]
fn waybar_all_combined_class() {
    let out = format_for_waybar(
        &clk(),
        &wrap(vec![claude_healthy(), codex_healthy(), amp_healthy()]),
        &settings(),
        DisplayMode::Remaining,
    );
    assert_eq!(out.class, "agent-bar claude-ok codex-ok amp-ok");
}

#[test]
fn waybar_empty_text() {
    with_filters(|| {
        let out = format_for_waybar(&clk(), &wrap(vec![]), &settings(), DisplayMode::Remaining);
        insta::assert_snapshot!(out.text, @"<span foreground='#8b95a5'>No Providers</span>");
    });
}

#[test]
fn waybar_empty_tooltip() {
    with_filters(|| {
        let out = format_for_waybar(&clk(), &wrap(vec![]), &settings(), DisplayMode::Remaining);
        insta::assert_snapshot!(out.tooltip, @"");
    });
}

#[test]
fn waybar_empty_class() {
    let out = format_for_waybar(&clk(), &wrap(vec![]), &settings(), DisplayMode::Remaining);
    assert_eq!(out.class, "agent-bar");
}

// ---------------------------------------------------------------------------
// Waybar aggregate — used
// ---------------------------------------------------------------------------

#[test]
fn waybar_claude_healthy_used_text() {
    with_filters(|| {
        let out = format_for_waybar(
            &clk(),
            &wrap(vec![claude_healthy()]),
            &settings(),
            DisplayMode::Used,
        );
        insta::assert_snapshot!(out.text, @"<span foreground='#98c379'>25%</span>");
    });
}

#[test]
fn waybar_claude_healthy_used_tooltip() {
    with_filters(|| {
        let out = format_for_waybar(
            &clk(),
            &wrap(vec![claude_healthy()]),
            &settings(),
            DisplayMode::Used,
        );
        insta::assert_snapshot!(out.tooltip, @r"
<span foreground='#d19a66'>┏━</span> <span foreground='#d19a66' weight='bold'>Claude · Pro</span> <span foreground='#d19a66'>━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━</span>
<span foreground='#d19a66'>┃</span>
<span foreground='#d19a66'>┣━</span> <span foreground='#d19a66' weight='bold'>◆ 5-hour limit (shared)</span>
<span foreground='#d19a66'>┃</span>  <span foreground='#98c379'>●</span> <span foreground='#e2e8f0'>All Models          </span> <span foreground='#98c379'>█████</span><span foreground='#8b95a5'>░░░░░░░░░░░░░░░</span> <span foreground='#98c379'> 25%</span> <span foreground='#56b6c2'>→ __HM__ (__:__)</span>
<span foreground='#d19a66'>┃</span>
<span foreground='#d19a66'>┣━</span> <span foreground='#d19a66' weight='bold'>◆ Weekly limit (shared)</span>
<span foreground='#d19a66'>┃</span>  <span foreground='#98c379'>●</span> <span foreground='#e2e8f0'>All Models          </span> <span foreground='#98c379'>██</span><span foreground='#8b95a5'>░░░░░░░░░░░░░░░░░░</span> <span foreground='#98c379'> 10%</span> <span foreground='#56b6c2'>→ __HM__ (__:__)</span>
<span foreground='#d19a66'>┃</span>
<span foreground='#d19a66'>┗━━━━━━━━━━━━━━━━━━</span><span foreground='#8b95a5'> cached · __AGO__ </span><span foreground='#d19a66'>━━━━━━━━━━━━━━━━━━</span>");
    });
}

#[test]
fn waybar_claude_healthy_used_class() {
    let out = format_for_waybar(
        &clk(),
        &wrap(vec![claude_healthy()]),
        &settings(),
        DisplayMode::Used,
    );
    assert_eq!(out.class, "agent-bar claude-ok");
}

#[test]
fn waybar_codex_healthy_used_text() {
    with_filters(|| {
        let out = format_for_waybar(
            &clk(),
            &wrap(vec![codex_healthy()]),
            &settings(),
            DisplayMode::Used,
        );
        insta::assert_snapshot!(out.text, @"<span foreground='#98c379'>40%</span>");
    });
}

#[test]
fn waybar_codex_healthy_used_tooltip() {
    with_filters(|| {
        let out = format_for_waybar(
            &clk(),
            &wrap(vec![codex_healthy()]),
            &settings(),
            DisplayMode::Used,
        );
        insta::assert_snapshot!(out.tooltip, @r"
<span foreground='#98c379'>┏━</span> <span foreground='#98c379' weight='bold'>Codex · Pro</span> <span foreground='#98c379'>━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━</span>
<span foreground='#98c379'>┃</span>
<span foreground='#98c379'>┃</span>
<span foreground='#98c379'>┣━</span> <span foreground='#98c379' weight='bold'>◆ 5-hour limit</span>
<span foreground='#98c379'>┃</span>  <span foreground='#98c379'>●</span> <span foreground='#e2e8f0'>Codex               </span> <span foreground='#98c379'>████████</span><span foreground='#8b95a5'>░░░░░░░░░░░░</span> <span foreground='#98c379'> 40%</span> <span foreground='#56b6c2'>→ __HM__ (__:__)</span>
<span foreground='#98c379'>┃</span>
<span foreground='#98c379'>┣━</span> <span foreground='#98c379' weight='bold'>◆ 7-day limit</span>
<span foreground='#98c379'>┃</span>  <span foreground='#98c379'>●</span> <span foreground='#e2e8f0'>Codex               </span> <span foreground='#98c379'>███</span><span foreground='#8b95a5'>░░░░░░░░░░░░░░░░░</span> <span foreground='#98c379'> 15%</span> <span foreground='#56b6c2'>→ __HM__ (__:__)</span>
<span foreground='#98c379'>┃</span>
<span foreground='#98c379'>┗━━━━━━━━━━━━━━━━━━</span><span foreground='#8b95a5'> cached · __AGO__ </span><span foreground='#98c379'>━━━━━━━━━━━━━━━━━━</span>");
    });
}

#[test]
fn waybar_codex_healthy_used_class() {
    let out = format_for_waybar(
        &clk(),
        &wrap(vec![codex_healthy()]),
        &settings(),
        DisplayMode::Used,
    );
    assert_eq!(out.class, "agent-bar codex-ok");
}

#[test]
fn waybar_amp_healthy_used_text() {
    with_filters(|| {
        let out = format_for_waybar(
            &clk(),
            &wrap(vec![amp_healthy()]),
            &settings(),
            DisplayMode::Used,
        );
        insta::assert_snapshot!(out.text, @"<span foreground='#98c379'>30%</span>");
    });
}

#[test]
fn waybar_amp_healthy_used_tooltip() {
    with_filters(|| {
        let out = format_for_waybar(
            &clk(),
            &wrap(vec![amp_healthy()]),
            &settings(),
            DisplayMode::Used,
        );
        insta::assert_snapshot!(out.tooltip, @r"
<span foreground='#c678dd'>┏━</span> <span foreground='#c678dd' weight='bold'>Amp</span> <span foreground='#c678dd'>━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━</span>
<span foreground='#c678dd'>┃</span>
<span foreground='#c678dd'>┣━</span> <span foreground='#c678dd' weight='bold'>◆ Free Tier</span>
<span foreground='#c678dd'>┃</span>  <span foreground='#98c379'>●</span> <span foreground='#98c379'>██████</span><span foreground='#8b95a5'>░░░░░░░░░░░░░░</span> <span foreground='#98c379'> 30%</span>  <span foreground='#56b6c2'>→ Resets in __HM__ (__:__)</span>
<span foreground='#c678dd'>┃</span>  <span foreground='#8b95a5'>○</span> <span foreground='#56b6c2'>+$0.25/hr</span><span foreground='#8b95a5'>  |  </span><span foreground='#c0c9d4'>$3.50 / $5.00</span>
<span foreground='#c678dd'>┃</span>
<span foreground='#c678dd'>┗━━━━━━━━━━━━━━━━━━</span><span foreground='#8b95a5'> cached · __AGO__ </span><span foreground='#c678dd'>━━━━━━━━━━━━━━━━━━</span>");
    });
}

// ---------------------------------------------------------------------------
// Waybar per-provider
// ---------------------------------------------------------------------------

#[test]
fn per_provider_claude_healthy_text() {
    with_filters(|| {
        let out = format_provider_for_waybar(
            &clk(),
            &claude_healthy(),
            &settings(),
            DisplayMode::Remaining,
        );
        insta::assert_snapshot!(out.text, @"<span foreground='#98c379'>75%</span>");
    });
}

#[test]
fn per_provider_claude_healthy_tooltip() {
    with_filters(|| {
        let out = format_provider_for_waybar(
            &clk(),
            &claude_healthy(),
            &settings(),
            DisplayMode::Remaining,
        );
        // per-provider tooltip tem footer SEM cached (fetched_at=None)
        insta::assert_snapshot!(out.tooltip, @r"
<span foreground='#d19a66'>┏━</span> <span foreground='#d19a66' weight='bold'>Claude · Pro</span> <span foreground='#d19a66'>━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━</span>
<span foreground='#d19a66'>┃</span>
<span foreground='#d19a66'>┣━</span> <span foreground='#d19a66' weight='bold'>◆ 5-hour limit (shared)</span>
<span foreground='#d19a66'>┃</span>  <span foreground='#98c379'>●</span> <span foreground='#e2e8f0'>All Models          </span> <span foreground='#98c379'>███████████████</span><span foreground='#8b95a5'>░░░░░</span> <span foreground='#98c379'> 75%</span> <span foreground='#56b6c2'>→ __HM__ (__:__)</span>
<span foreground='#d19a66'>┃</span>
<span foreground='#d19a66'>┣━</span> <span foreground='#d19a66' weight='bold'>◆ Weekly limit (shared)</span>
<span foreground='#d19a66'>┃</span>  <span foreground='#98c379'>●</span> <span foreground='#e2e8f0'>All Models          </span> <span foreground='#98c379'>██████████████████</span><span foreground='#8b95a5'>░░</span> <span foreground='#98c379'> 90%</span> <span foreground='#56b6c2'>→ __HM__ (__:__)</span>
<span foreground='#d19a66'>┃</span>
<span foreground='#d19a66'>┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━</span>");
    });
}

#[test]
fn per_provider_claude_healthy_class() {
    let out = format_provider_for_waybar(
        &clk(),
        &claude_healthy(),
        &settings(),
        DisplayMode::Remaining,
    );
    assert_eq!(out.class, "agent-bar-claude ok");
}

#[test]
fn per_provider_claude_disconnected_text() {
    with_filters(|| {
        let out = format_provider_for_waybar(
            &clk(),
            &claude_error(),
            &settings(),
            DisplayMode::Remaining,
        );
        // Glyph nerd-font U+F1616
        insta::assert_snapshot!(out.text, @"<span foreground='#e88b93'>\u{f1616}</span>");
    });
}

#[test]
fn per_provider_claude_disconnected_tooltip() {
    with_filters(|| {
        let out = format_provider_for_waybar(
            &clk(),
            &claude_error(),
            &settings(),
            DisplayMode::Remaining,
        );
        insta::assert_snapshot!(out.tooltip, @r"
<span foreground='#d19a66'>┏━</span> <span foreground='#d19a66' weight='bold'>Claude</span> <span foreground='#d19a66'>━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━</span>
<span foreground='#d19a66'>┃</span>
<span foreground='#d19a66'>┃</span>  <span foreground='#e88b93'>⚠️ Token expired. Open `agent-bar menu` and choose Provider login.</span>
<span foreground='#d19a66'>┃</span>
<span foreground='#d19a66'>┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━</span>");
    });
}

#[test]
fn per_provider_claude_disconnected_class() {
    let out =
        format_provider_for_waybar(&clk(), &claude_error(), &settings(), DisplayMode::Remaining);
    assert_eq!(out.class, "agent-bar-claude disconnected");
}

#[test]
fn per_provider_codex_healthy_text() {
    with_filters(|| {
        let out = format_provider_for_waybar(
            &clk(),
            &codex_healthy(),
            &settings(),
            DisplayMode::Remaining,
        );
        insta::assert_snapshot!(out.text, @"<span foreground='#98c379'>60%</span>");
    });
}

#[test]
fn per_provider_codex_healthy_tooltip() {
    with_filters(|| {
        let out = format_provider_for_waybar(
            &clk(),
            &codex_healthy(),
            &settings(),
            DisplayMode::Remaining,
        );
        insta::assert_snapshot!(out.tooltip, @r"
<span foreground='#98c379'>┏━</span> <span foreground='#98c379' weight='bold'>Codex · Pro</span> <span foreground='#98c379'>━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━</span>
<span foreground='#98c379'>┃</span>
<span foreground='#98c379'>┃</span>
<span foreground='#98c379'>┣━</span> <span foreground='#98c379' weight='bold'>◆ 5-hour limit</span>
<span foreground='#98c379'>┃</span>  <span foreground='#98c379'>●</span> <span foreground='#e2e8f0'>Codex               </span> <span foreground='#98c379'>████████████</span><span foreground='#8b95a5'>░░░░░░░░</span> <span foreground='#98c379'> 60%</span> <span foreground='#56b6c2'>→ __HM__ (__:__)</span>
<span foreground='#98c379'>┃</span>
<span foreground='#98c379'>┣━</span> <span foreground='#98c379' weight='bold'>◆ 7-day limit</span>
<span foreground='#98c379'>┃</span>  <span foreground='#98c379'>●</span> <span foreground='#e2e8f0'>Codex               </span> <span foreground='#98c379'>█████████████████</span><span foreground='#8b95a5'>░░░</span> <span foreground='#98c379'> 85%</span> <span foreground='#56b6c2'>→ __HM__ (__:__)</span>
<span foreground='#98c379'>┃</span>
<span foreground='#98c379'>┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━</span>");
    });
}

#[test]
fn per_provider_codex_healthy_class() {
    let out = format_provider_for_waybar(
        &clk(),
        &codex_healthy(),
        &settings(),
        DisplayMode::Remaining,
    );
    assert_eq!(out.class, "agent-bar-codex ok");
}

#[test]
fn per_provider_amp_healthy_text() {
    with_filters(|| {
        let out =
            format_provider_for_waybar(&clk(), &amp_healthy(), &settings(), DisplayMode::Remaining);
        insta::assert_snapshot!(out.text, @"<span foreground='#98c379'>70%</span>");
    });
}

#[test]
fn per_provider_amp_healthy_tooltip() {
    with_filters(|| {
        let out =
            format_provider_for_waybar(&clk(), &amp_healthy(), &settings(), DisplayMode::Remaining);
        insta::assert_snapshot!(out.tooltip, @r"
<span foreground='#c678dd'>┏━</span> <span foreground='#c678dd' weight='bold'>Amp</span> <span foreground='#c678dd'>━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━</span>
<span foreground='#c678dd'>┃</span>
<span foreground='#c678dd'>┣━</span> <span foreground='#c678dd' weight='bold'>◆ Free Tier</span>
<span foreground='#c678dd'>┃</span>  <span foreground='#98c379'>●</span> <span foreground='#98c379'>██████████████</span><span foreground='#8b95a5'>░░░░░░</span> <span foreground='#98c379'> 70%</span>  <span foreground='#56b6c2'>→ Full in __HM__ (__:__)</span>
<span foreground='#c678dd'>┃</span>  <span foreground='#8b95a5'>○</span> <span foreground='#56b6c2'>+$0.25/hr</span><span foreground='#8b95a5'>  |  </span><span foreground='#c0c9d4'>$3.50 / $5.00</span>
<span foreground='#c678dd'>┃</span>
<span foreground='#c678dd'>┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━</span>");
    });
}

#[test]
fn per_provider_amp_healthy_class() {
    let out =
        format_provider_for_waybar(&clk(), &amp_healthy(), &settings(), DisplayMode::Remaining);
    assert_eq!(out.class, "agent-bar-amp ok");
}

// ---------------------------------------------------------------------------
// Waybar account (C1)
// ---------------------------------------------------------------------------

#[test]
fn waybar_amp_with_account_text() {
    with_filters(|| {
        let out = format_for_waybar(
            &clk(),
            &wrap(vec![amp_with_account()]),
            &settings(),
            DisplayMode::Remaining,
        );
        insta::assert_snapshot!(out.text, @"<span foreground='#98c379'>70%</span>");
    });
}

#[test]
fn waybar_amp_with_account_tooltip() {
    with_filters(|| {
        let out = format_for_waybar(
            &clk(),
            &wrap(vec![amp_with_account()]),
            &settings(),
            DisplayMode::Remaining,
        );
        insta::assert_snapshot!(out.tooltip, @r"
<span foreground='#c678dd'>┏━</span> <span foreground='#c678dd' weight='bold'>Amp · user@example.com</span> <span foreground='#c678dd'>━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━</span>
<span foreground='#c678dd'>┃</span>
<span foreground='#c678dd'>┣━</span> <span foreground='#c678dd' weight='bold'>◆ Free Tier</span>
<span foreground='#c678dd'>┃</span>  <span foreground='#98c379'>●</span> <span foreground='#98c379'>██████████████</span><span foreground='#8b95a5'>░░░░░░</span> <span foreground='#98c379'> 70%</span>  <span foreground='#56b6c2'>→ Full in __HM__ (__:__)</span>
<span foreground='#c678dd'>┃</span>  <span foreground='#8b95a5'>○</span> <span foreground='#56b6c2'>+$0.25/hr</span><span foreground='#8b95a5'>  |  </span><span foreground='#c0c9d4'>$3.50 / $5.00</span>
<span foreground='#c678dd'>┃</span>
<span foreground='#c678dd'>┗━━━━━━━━━━━━━━━━━━</span><span foreground='#8b95a5'> cached · __AGO__ </span><span foreground='#c678dd'>━━━━━━━━━━━━━━━━━━</span>");
    });
}

#[test]
fn waybar_amp_with_account_class() {
    let out = format_for_waybar(
        &clk(),
        &wrap(vec![amp_with_account()]),
        &settings(),
        DisplayMode::Remaining,
    );
    assert_eq!(out.class, "agent-bar amp-ok");
}

#[test]
fn per_provider_amp_with_account_text() {
    with_filters(|| {
        let out = format_provider_for_waybar(
            &clk(),
            &amp_with_account(),
            &settings(),
            DisplayMode::Remaining,
        );
        insta::assert_snapshot!(out.text, @"<span foreground='#98c379'>70%</span>");
    });
}

#[test]
fn per_provider_amp_with_account_tooltip() {
    with_filters(|| {
        let out = format_provider_for_waybar(
            &clk(),
            &amp_with_account(),
            &settings(),
            DisplayMode::Remaining,
        );
        insta::assert_snapshot!(out.tooltip, @r"
<span foreground='#c678dd'>┏━</span> <span foreground='#c678dd' weight='bold'>Amp · user@example.com</span> <span foreground='#c678dd'>━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━</span>
<span foreground='#c678dd'>┃</span>
<span foreground='#c678dd'>┣━</span> <span foreground='#c678dd' weight='bold'>◆ Free Tier</span>
<span foreground='#c678dd'>┃</span>  <span foreground='#98c379'>●</span> <span foreground='#98c379'>██████████████</span><span foreground='#8b95a5'>░░░░░░</span> <span foreground='#98c379'> 70%</span>  <span foreground='#56b6c2'>→ Full in __HM__ (__:__)</span>
<span foreground='#c678dd'>┃</span>  <span foreground='#8b95a5'>○</span> <span foreground='#56b6c2'>+$0.25/hr</span><span foreground='#8b95a5'>  |  </span><span foreground='#c0c9d4'>$3.50 / $5.00</span>
<span foreground='#c678dd'>┃</span>
<span foreground='#c678dd'>┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━</span>");
    });
}

#[test]
fn per_provider_amp_with_account_class() {
    let out = format_provider_for_waybar(
        &clk(),
        &amp_with_account(),
        &settings(),
        DisplayMode::Remaining,
    );
    assert_eq!(out.class, "agent-bar-amp ok");
}

// ---------------------------------------------------------------------------
// Waybar rich fixtures (C3)
// ---------------------------------------------------------------------------

#[test]
fn waybar_claude_with_extras_text() {
    with_filters(|| {
        let out = format_for_waybar(
            &clk(),
            &wrap(vec![claude_with_extras()]),
            &settings(),
            DisplayMode::Remaining,
        );
        insta::assert_snapshot!(out.text, @"<span foreground='#98c379'>60%</span>");
    });
}

#[test]
fn waybar_claude_with_extras_tooltip() {
    with_filters(|| {
        let out = format_for_waybar(
            &clk(),
            &wrap(vec![claude_with_extras()]),
            &settings(),
            DisplayMode::Remaining,
        );
        insta::assert_snapshot!(out.tooltip, @r"
<span foreground='#d19a66'>┏━</span> <span foreground='#d19a66' weight='bold'>Claude · Pro</span> <span foreground='#d19a66'>━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━</span>
<span foreground='#d19a66'>┃</span>
<span foreground='#d19a66'>┣━</span> <span foreground='#d19a66' weight='bold'>◆ 5-hour limit (shared)</span>
<span foreground='#d19a66'>┃</span>  <span foreground='#98c379'>●</span> <span foreground='#e2e8f0'>All Models          </span> <span foreground='#98c379'>████████████</span><span foreground='#8b95a5'>░░░░░░░░</span> <span foreground='#98c379'> 60%</span> <span foreground='#56b6c2'>→ __HM__ (__:__)</span>
<span foreground='#d19a66'>┃</span>
<span foreground='#d19a66'>┣━</span> <span foreground='#d19a66' weight='bold'>◆ Weekly per model</span>
<span foreground='#d19a66'>┃</span>  <span foreground='#e5c07b'>●</span> <span foreground='#e2e8f0'>claude-opus-4-5     </span> <span foreground='#e5c07b'>████████</span><span foreground='#8b95a5'>░░░░░░░░░░░░</span> <span foreground='#e5c07b'> 40%</span> <span foreground='#56b6c2'>→ __HM__ (__:__)</span>
<span foreground='#d19a66'>┃</span>  <span foreground='#98c379'>●</span> <span foreground='#e2e8f0'>claude-sonnet-4-5   </span> <span foreground='#98c379'>█████████████</span><span foreground='#8b95a5'>░░░░░░░</span> <span foreground='#98c379'> 65%</span> <span foreground='#56b6c2'>→ __HM__ (__:__)</span>
<span foreground='#d19a66'>┃</span>
<span foreground='#d19a66'>┣━</span> <span foreground='#d19a66' weight='bold'>◆ Weekly limit (shared)</span>
<span foreground='#d19a66'>┃</span>  <span foreground='#e5c07b'>●</span> <span foreground='#e2e8f0'>All Models          </span> <span foreground='#e5c07b'>██████████</span><span foreground='#8b95a5'>░░░░░░░░░░</span> <span foreground='#e5c07b'> 50%</span> <span foreground='#56b6c2'>→ __HM__ (__:__)</span>
<span foreground='#d19a66'>┃</span>
<span foreground='#d19a66'>┣━</span> <span foreground='#d19a66' weight='bold'>◆ Extra Usage</span>
<span foreground='#d19a66'>┃</span>  <span foreground='#e5c07b'>●</span> <span foreground='#e2e8f0'>Budget              </span> <span foreground='#e5c07b'>███████████</span><span foreground='#8b95a5'>░░░░░░░░░</span> <span foreground='#e5c07b'> 55%</span> <span foreground='#56b6c2'>$22.50/$50.00</span>
<span foreground='#d19a66'>┃</span>
<span foreground='#d19a66'>┗━━━━━━━━━━━━━━━━━━</span><span foreground='#8b95a5'> cached · __AGO__ </span><span foreground='#d19a66'>━━━━━━━━━━━━━━━━━━</span>");
    });
}

#[test]
fn waybar_claude_with_extras_class() {
    let out = format_for_waybar(
        &clk(),
        &wrap(vec![claude_with_extras()]),
        &settings(),
        DisplayMode::Remaining,
    );
    assert_eq!(out.class, "agent-bar claude-ok");
}

#[test]
fn waybar_amp_with_credits_text() {
    with_filters(|| {
        let out = format_for_waybar(
            &clk(),
            &wrap(vec![amp_with_credits()]),
            &settings(),
            DisplayMode::Remaining,
        );
        insta::assert_snapshot!(out.text, @"<span foreground='#e5c07b'>30%</span>");
    });
}

#[test]
fn waybar_amp_with_credits_tooltip() {
    with_filters(|| {
        let out = format_for_waybar(
            &clk(),
            &wrap(vec![amp_with_credits()]),
            &settings(),
            DisplayMode::Remaining,
        );
        insta::assert_snapshot!(out.tooltip, @r"
<span foreground='#c678dd'>┏━</span> <span foreground='#c678dd' weight='bold'>Amp</span> <span foreground='#c678dd'>━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━</span>
<span foreground='#c678dd'>┃</span>
<span foreground='#c678dd'>┣━</span> <span foreground='#c678dd' weight='bold'>◆ Free Tier</span>
<span foreground='#c678dd'>┃</span>  <span foreground='#e5c07b'>●</span> <span foreground='#e5c07b'>██████</span><span foreground='#8b95a5'>░░░░░░░░░░░░░░</span> <span foreground='#e5c07b'> 30%</span>  <span foreground='#56b6c2'>→ Full in __HM__ (__:__)</span>
<span foreground='#c678dd'>┃</span>  <span foreground='#8b95a5'>○</span> <span foreground='#56b6c2'>+$0.25/hr</span><span foreground='#8b95a5'>  |  </span><span foreground='#c0c9d4'>$1.50 / $5.00</span>
<span foreground='#c678dd'>┃</span>
<span foreground='#c678dd'>┣━</span> <span foreground='#c678dd' weight='bold'>◆ Credits</span>
<span foreground='#c678dd'>┃</span>  <span foreground='#98c379'>●</span> <span foreground='#98c379'>$7.50 remaining</span>
<span foreground='#c678dd'>┃</span>
<span foreground='#c678dd'>┗━━━━━━━━━━━━━━━━━━</span><span foreground='#8b95a5'> cached · __AGO__ </span><span foreground='#c678dd'>━━━━━━━━━━━━━━━━━━</span>");
    });
}

#[test]
fn waybar_amp_with_credits_class() {
    let out = format_for_waybar(
        &clk(),
        &wrap(vec![amp_with_credits()]),
        &settings(),
        DisplayMode::Remaining,
    );
    assert_eq!(out.class, "agent-bar amp-low");
}

#[test]
fn waybar_amp_unknown_models_text() {
    with_filters(|| {
        let out = format_for_waybar(
            &clk(),
            &wrap(vec![amp_unknown_models()]),
            &settings(),
            DisplayMode::Remaining,
        );
        insta::assert_snapshot!(out.text, @"<span foreground='#e5c07b'>45%</span>");
    });
}

#[test]
fn waybar_amp_unknown_models_tooltip() {
    with_filters(|| {
        let out = format_for_waybar(
            &clk(),
            &wrap(vec![amp_unknown_models()]),
            &settings(),
            DisplayMode::Remaining,
        );
        insta::assert_snapshot!(out.tooltip, @r"
<span foreground='#c678dd'>┏━</span> <span foreground='#c678dd' weight='bold'>Amp</span> <span foreground='#c678dd'>━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━</span>
<span foreground='#c678dd'>┃</span>
<span foreground='#c678dd'>┣━</span> <span foreground='#c678dd' weight='bold'>◆ Usage</span>
<span foreground='#c678dd'>┃</span>  <span foreground='#e5c07b'>●</span> <span foreground='#e2e8f0'>Custom Plan A       </span> <span foreground='#e5c07b'>█████████</span><span foreground='#8b95a5'>░░░░░░░░░░░</span> <span foreground='#e5c07b'> 45%</span>
<span foreground='#c678dd'>┃</span>  <span foreground='#98c379'>●</span> <span foreground='#e2e8f0'>Custom Plan B       </span> <span foreground='#98c379'>████████████████</span><span foreground='#8b95a5'>░░░░</span> <span foreground='#98c379'> 80%</span>
<span foreground='#c678dd'>┃</span>
<span foreground='#c678dd'>┗━━━━━━━━━━━━━━━━━━</span><span foreground='#8b95a5'> cached · __AGO__ </span><span foreground='#c678dd'>━━━━━━━━━━━━━━━━━━</span>");
    });
}

#[test]
fn waybar_amp_unknown_models_class() {
    let out = format_for_waybar(
        &clk(),
        &wrap(vec![amp_unknown_models()]),
        &settings(),
        DisplayMode::Remaining,
    );
    assert_eq!(out.class, "agent-bar amp-low");
}

// ---------------------------------------------------------------------------
// Per-provider rich fixtures (C3)
// ---------------------------------------------------------------------------

#[test]
fn per_provider_claude_with_extras_text() {
    with_filters(|| {
        let out = format_provider_for_waybar(
            &clk(),
            &claude_with_extras(),
            &settings(),
            DisplayMode::Remaining,
        );
        insta::assert_snapshot!(out.text, @"<span foreground='#98c379'>60%</span>");
    });
}

#[test]
fn per_provider_claude_with_extras_tooltip() {
    with_filters(|| {
        let out = format_provider_for_waybar(
            &clk(),
            &claude_with_extras(),
            &settings(),
            DisplayMode::Remaining,
        );
        insta::assert_snapshot!(out.tooltip, @r"
<span foreground='#d19a66'>┏━</span> <span foreground='#d19a66' weight='bold'>Claude · Pro</span> <span foreground='#d19a66'>━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━</span>
<span foreground='#d19a66'>┃</span>
<span foreground='#d19a66'>┣━</span> <span foreground='#d19a66' weight='bold'>◆ 5-hour limit (shared)</span>
<span foreground='#d19a66'>┃</span>  <span foreground='#98c379'>●</span> <span foreground='#e2e8f0'>All Models          </span> <span foreground='#98c379'>████████████</span><span foreground='#8b95a5'>░░░░░░░░</span> <span foreground='#98c379'> 60%</span> <span foreground='#56b6c2'>→ __HM__ (__:__)</span>
<span foreground='#d19a66'>┃</span>
<span foreground='#d19a66'>┣━</span> <span foreground='#d19a66' weight='bold'>◆ Weekly per model</span>
<span foreground='#d19a66'>┃</span>  <span foreground='#e5c07b'>●</span> <span foreground='#e2e8f0'>claude-opus-4-5     </span> <span foreground='#e5c07b'>████████</span><span foreground='#8b95a5'>░░░░░░░░░░░░</span> <span foreground='#e5c07b'> 40%</span> <span foreground='#56b6c2'>→ __HM__ (__:__)</span>
<span foreground='#d19a66'>┃</span>  <span foreground='#98c379'>●</span> <span foreground='#e2e8f0'>claude-sonnet-4-5   </span> <span foreground='#98c379'>█████████████</span><span foreground='#8b95a5'>░░░░░░░</span> <span foreground='#98c379'> 65%</span> <span foreground='#56b6c2'>→ __HM__ (__:__)</span>
<span foreground='#d19a66'>┃</span>
<span foreground='#d19a66'>┣━</span> <span foreground='#d19a66' weight='bold'>◆ Weekly limit (shared)</span>
<span foreground='#d19a66'>┃</span>  <span foreground='#e5c07b'>●</span> <span foreground='#e2e8f0'>All Models          </span> <span foreground='#e5c07b'>██████████</span><span foreground='#8b95a5'>░░░░░░░░░░</span> <span foreground='#e5c07b'> 50%</span> <span foreground='#56b6c2'>→ __HM__ (__:__)</span>
<span foreground='#d19a66'>┃</span>
<span foreground='#d19a66'>┣━</span> <span foreground='#d19a66' weight='bold'>◆ Extra Usage</span>
<span foreground='#d19a66'>┃</span>  <span foreground='#e5c07b'>●</span> <span foreground='#e2e8f0'>Budget              </span> <span foreground='#e5c07b'>███████████</span><span foreground='#8b95a5'>░░░░░░░░░</span> <span foreground='#e5c07b'> 55%</span> <span foreground='#56b6c2'>$22.50/$50.00</span>
<span foreground='#d19a66'>┃</span>
<span foreground='#d19a66'>┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━</span>");
    });
}

#[test]
fn per_provider_claude_with_extras_class() {
    let out = format_provider_for_waybar(
        &clk(),
        &claude_with_extras(),
        &settings(),
        DisplayMode::Remaining,
    );
    assert_eq!(out.class, "agent-bar-claude ok");
}

#[test]
fn per_provider_amp_with_credits_text() {
    with_filters(|| {
        let out = format_provider_for_waybar(
            &clk(),
            &amp_with_credits(),
            &settings(),
            DisplayMode::Remaining,
        );
        insta::assert_snapshot!(out.text, @"<span foreground='#e5c07b'>30%</span>");
    });
}

#[test]
fn per_provider_amp_with_credits_tooltip() {
    with_filters(|| {
        let out = format_provider_for_waybar(
            &clk(),
            &amp_with_credits(),
            &settings(),
            DisplayMode::Remaining,
        );
        insta::assert_snapshot!(out.tooltip, @r"
<span foreground='#c678dd'>┏━</span> <span foreground='#c678dd' weight='bold'>Amp</span> <span foreground='#c678dd'>━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━</span>
<span foreground='#c678dd'>┃</span>
<span foreground='#c678dd'>┣━</span> <span foreground='#c678dd' weight='bold'>◆ Free Tier</span>
<span foreground='#c678dd'>┃</span>  <span foreground='#e5c07b'>●</span> <span foreground='#e5c07b'>██████</span><span foreground='#8b95a5'>░░░░░░░░░░░░░░</span> <span foreground='#e5c07b'> 30%</span>  <span foreground='#56b6c2'>→ Full in __HM__ (__:__)</span>
<span foreground='#c678dd'>┃</span>  <span foreground='#8b95a5'>○</span> <span foreground='#56b6c2'>+$0.25/hr</span><span foreground='#8b95a5'>  |  </span><span foreground='#c0c9d4'>$1.50 / $5.00</span>
<span foreground='#c678dd'>┃</span>
<span foreground='#c678dd'>┣━</span> <span foreground='#c678dd' weight='bold'>◆ Credits</span>
<span foreground='#c678dd'>┃</span>  <span foreground='#98c379'>●</span> <span foreground='#98c379'>$7.50 remaining</span>
<span foreground='#c678dd'>┃</span>
<span foreground='#c678dd'>┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━</span>");
    });
}

#[test]
fn per_provider_amp_with_credits_class() {
    let out = format_provider_for_waybar(
        &clk(),
        &amp_with_credits(),
        &settings(),
        DisplayMode::Remaining,
    );
    assert_eq!(out.class, "agent-bar-amp low");
}

#[test]
fn per_provider_amp_unknown_models_text() {
    with_filters(|| {
        let out = format_provider_for_waybar(
            &clk(),
            &amp_unknown_models(),
            &settings(),
            DisplayMode::Remaining,
        );
        insta::assert_snapshot!(out.text, @"<span foreground='#e5c07b'>45%</span>");
    });
}

#[test]
fn per_provider_amp_unknown_models_tooltip() {
    with_filters(|| {
        let out = format_provider_for_waybar(
            &clk(),
            &amp_unknown_models(),
            &settings(),
            DisplayMode::Remaining,
        );
        insta::assert_snapshot!(out.tooltip, @r"
<span foreground='#c678dd'>┏━</span> <span foreground='#c678dd' weight='bold'>Amp</span> <span foreground='#c678dd'>━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━</span>
<span foreground='#c678dd'>┃</span>
<span foreground='#c678dd'>┣━</span> <span foreground='#c678dd' weight='bold'>◆ Usage</span>
<span foreground='#c678dd'>┃</span>  <span foreground='#e5c07b'>●</span> <span foreground='#e2e8f0'>Custom Plan A       </span> <span foreground='#e5c07b'>█████████</span><span foreground='#8b95a5'>░░░░░░░░░░░</span> <span foreground='#e5c07b'> 45%</span>
<span foreground='#c678dd'>┃</span>  <span foreground='#98c379'>●</span> <span foreground='#e2e8f0'>Custom Plan B       </span> <span foreground='#98c379'>████████████████</span><span foreground='#8b95a5'>░░░░</span> <span foreground='#98c379'> 80%</span>
<span foreground='#c678dd'>┃</span>
<span foreground='#c678dd'>┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━</span>");
    });
}

#[test]
fn per_provider_amp_unknown_models_class() {
    let out = format_provider_for_waybar(
        &clk(),
        &amp_unknown_models(),
        &settings(),
        DisplayMode::Remaining,
    );
    assert_eq!(out.class, "agent-bar-amp low");
}
