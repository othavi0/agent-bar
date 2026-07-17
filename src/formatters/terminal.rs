//! Assembly da superfície terminal (ANSI). Port fiel de `src/formatters/terminal.ts`.
//! Funções puras: Settings, Clock e o gate `no_color` são injetados (o CLI lê
//! NO_COLOR do ambiente no Plano 5 e repassa) — sem leitura de ambiente aqui.

use crate::formatters::builders::amp::build_amp;
use crate::formatters::builders::claude::build_claude;
use crate::formatters::builders::codex::build_codex;
use crate::formatters::builders::generic::build_generic;
use crate::formatters::builders::grok::build_grok;
use crate::formatters::builders::shared::{AmpLayout, BuildOptions};
use crate::formatters::clock::Clock;
use crate::formatters::render_ansi::render_ansi;
use crate::formatters::shared::normalize_plan_label;
use crate::formatters::view_model::resolve_codex_view_model_from;
use crate::providers::types::{AllQuotas, ProviderQuota};
use crate::settings::{DisplayMode, Settings};
use crate::theme::{ColorToken, ANSI_RESET};

fn terminal_section(
    clock: &Clock,
    p: &ProviderQuota,
    settings: &Settings,
    mode: DisplayMode,
    no_color: bool,
) -> String {
    match p.provider.as_str() {
        "claude" => render_ansi(
            &build_claude(
                clock,
                p,
                &BuildOptions {
                    mode,
                    header_title: "Claude".into(),
                    header_width: 56,
                    label_color: ColorToken::Magenta,
                    footer_fetched_at: None,
                    plan_label: None,
                    amp_free_tier_layout: AmpLayout::Inline,
                    account_in_header: false,
                },
            ),
            no_color,
        ),
        "codex" => {
            let vm = resolve_codex_view_model_from(settings, p);
            render_ansi(
                &build_codex(
                    clock,
                    p,
                    &vm,
                    &BuildOptions {
                        mode,
                        header_title: "Codex".into(),
                        header_width: 56,
                        label_color: ColorToken::Magenta,
                        footer_fetched_at: None,
                        plan_label: Some(normalize_plan_label(p)),
                        amp_free_tier_layout: AmpLayout::Inline,
                        account_in_header: false,
                    },
                ),
                no_color,
            )
        }
        "amp" => render_ansi(
            &build_amp(
                clock,
                p,
                &BuildOptions {
                    mode,
                    header_title: "Amp".into(),
                    header_width: 56,
                    label_color: ColorToken::Magenta,
                    footer_fetched_at: None,
                    plan_label: None,
                    amp_free_tier_layout: AmpLayout::Sublines,
                    account_in_header: false,
                },
            ),
            no_color,
        ),
        "grok" => render_ansi(
            &build_grok(
                clock,
                p,
                &BuildOptions {
                    mode,
                    header_title: "Grok".into(),
                    header_width: 56,
                    label_color: ColorToken::Cyan,
                    footer_fetched_at: None,
                    plan_label: None,
                    amp_free_tier_layout: AmpLayout::Inline,
                    account_in_header: false,
                },
            ),
            no_color,
        ),
        _ => render_ansi(
            &build_generic(
                clock,
                p,
                &BuildOptions {
                    mode,
                    header_title: if p.display_name.is_empty() {
                        p.provider.clone()
                    } else {
                        p.display_name.clone()
                    },
                    header_width: 52,
                    label_color: ColorToken::Text,
                    footer_fetched_at: None,
                    plan_label: None,
                    amp_free_tier_layout: AmpLayout::Inline,
                    account_in_header: false,
                },
            ),
            no_color,
        ),
    }
}

pub fn format_for_terminal(
    clock: &Clock,
    quotas: &AllQuotas,
    settings: &Settings,
    mode: DisplayMode,
    no_color: bool,
) -> String {
    let sections: Vec<String> = quotas
        .providers
        .iter()
        .filter(|p| p.available || p.error.is_some())
        .map(|p| terminal_section(clock, p, settings, mode, no_color))
        .collect();

    if sections.is_empty() {
        return format!(
            "{}No providers connected{}",
            ColorToken::Comment.ansi(),
            ANSI_RESET
        );
    }
    sections.join("\n\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Paths;
    use crate::formatters::clock::Clock;
    use crate::providers::types::{AllQuotas, ProviderQuota, QuotaWindow};
    use crate::settings::load;
    use std::path::PathBuf;
    use tempfile::tempdir;
    use time::macros::datetime;

    fn clk() -> Clock {
        Clock {
            now: datetime!(2026-03-28 12:00:00 UTC),
            local_offset: time::UtcOffset::UTC,
        }
    }

    fn settings() -> crate::settings::Settings {
        let dir = tempdir().unwrap();
        load(&Paths {
            cache_dir: dir.path().join("cache"),
            config_dir: dir.path().join("config"),
            claude_credentials: PathBuf::new(),
            codex_auth: PathBuf::new(),
            codex_sessions: PathBuf::new(),
            amp_settings: PathBuf::new(),
            amp_threads: PathBuf::new(),
            grok_home: PathBuf::new(),
            grok_auth: PathBuf::new(),
        })
    }

    fn claude() -> ProviderQuota {
        ProviderQuota {
            provider: "claude".into(),
            display_name: "Claude".into(),
            available: true,
            account: None,
            plan: Some("Pro".into()),
            plan_type: None,
            primary: Some(QuotaWindow {
                remaining: 75.0,
                resets_at: Some("2026-03-28T14:00:00Z".into()),
                window_minutes: Some(300),
                used: None,
                severity: None,
            }),
            secondary: None,
            models: None,
            extra: None,
            error: None,
        }
    }

    #[test]
    fn renders_claude_section() {
        let q = AllQuotas {
            providers: vec![claude()],
            fetched_at: "2026-03-28T12:00:00Z".into(),
        };
        let out = format_for_terminal(&clk(), &q, &settings(), DisplayMode::Remaining, false);
        assert!(out.contains("Claude"));
        assert!(out.contains("75%"));
    }

    #[test]
    fn empty_when_no_providers() {
        let q = AllQuotas {
            providers: vec![],
            fetched_at: "2026-03-28T12:00:00Z".into(),
        };
        let out = format_for_terminal(&clk(), &q, &settings(), DisplayMode::Remaining, false);
        assert!(out.contains("No providers connected"));
    }

    #[test]
    fn skips_unavailable_without_error() {
        let mut c = claude();
        c.available = false;
        c.error = None;
        let q = AllQuotas {
            providers: vec![c],
            fetched_at: "2026-03-28T12:00:00Z".into(),
        };
        let out = format_for_terminal(&clk(), &q, &settings(), DisplayMode::Remaining, false);
        assert!(out.contains("No providers connected"));
    }
}
