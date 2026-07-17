//! Assembly da superfície Waybar ({text,tooltip,class}). Port fiel de
//! `src/formatters/waybar.ts`. Funções puras: Settings e Clock injetados.
//! Settings são lidas do disco a cada invocação — sem cache; o hidden-module
//! short-circuit vive no CLI (Plano 5).

use serde::Serialize;

use crate::app_identity::APP_BASE_CLASS;
use crate::config::status_for_percent;
use crate::formatters::builders::amp::build_amp;
use crate::formatters::builders::claude::build_claude;
use crate::formatters::builders::codex::build_codex;
use crate::formatters::builders::generic::build_generic;
use crate::formatters::builders::shared::{AmpLayout, BuildOptions, TOOLTIP_BORDER};
use crate::formatters::clock::Clock;
use crate::formatters::render_pango::{render_pango, span};
use crate::formatters::segments::color_for_display;
use crate::formatters::shared::{format_percent, normalize_plan_label, to_window_display};
use crate::formatters::view_model::resolve_codex_view_model_from;
use crate::providers::types::{AllQuotas, ProviderQuota};
use crate::settings::{DisplayMode, Settings};
use crate::theme::ColorToken;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WaybarOutput {
    pub text: String,
    pub tooltip: String,
    pub class: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alt: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub percentage: Option<u8>,
}

/// Payload degradado quando a serialização do `WaybarOutput` falha.
/// Campos estáticos garantem que o fallback em si serializa.
pub fn waybar_error_payload(msg: &str) -> WaybarOutput {
    // Classe visível (não `agent-bar-hidden`): falha de serialize deve
    // degradar o módulo, não colapsá-lo como provider desabilitado (trilha B).
    WaybarOutput {
        text: "err".to_string(),
        tooltip: format!("agent-bar: serialize failed: {msg}"),
        class: format!("{APP_BASE_CLASS} disconnected"),
        alt: Some("disconnected".to_string()),
        percentage: None,
    }
}

/// Serializa saída Waybar. Em falha de serde, devolve payload degradado
/// (nunca string vazia) e emite o erro em `err_log`.
pub fn waybar_stdout_line(o: &WaybarOutput, err_log: &mut dyn std::io::Write) -> String {
    match serde_json::to_string(o) {
        Ok(s) => s,
        Err(e) => {
            let _ = writeln!(err_log, "waybar serialize failed: {e}");
            match serde_json::to_string(&waybar_error_payload(&e.to_string())) {
                Ok(fallback) => fallback,
                // Teoricamente impossível com strings estáticas; último recurso.
                Err(_) => {
                    r#"{"text":"err","tooltip":"agent-bar: serialize failed","class":"agent-bar disconnected"}"#
                        .to_string()
                }
            }
        }
    }
}

fn pct_colored(disp: Option<f64>, mode: DisplayMode) -> String {
    span(
        color_for_display(disp, mode).hex(),
        &format_percent(disp),
        false,
    )
}

fn header_width_waybar() -> usize {
    TOOLTIP_BORDER - 4
}

fn provider_tooltip(
    clock: &Clock,
    p: &ProviderQuota,
    fetched_at: Option<&str>,
    settings: &Settings,
    mode: DisplayMode,
) -> String {
    let fetched = fetched_at.map(|s| s.to_string());
    match p.provider.as_str() {
        "claude" => {
            let plan = normalize_plan_label(p);
            let title = if plan != "Unknown" {
                format!("Claude · {plan}")
            } else {
                "Claude".to_string()
            };
            render_pango(&build_claude(
                clock,
                p,
                &BuildOptions {
                    mode,
                    header_title: title,
                    header_width: header_width_waybar(),
                    label_color: ColorToken::Orange,
                    footer_fetched_at: fetched,
                    plan_label: None,
                    amp_free_tier_layout: AmpLayout::Inline,
                    account_in_header: false,
                },
            ))
        }
        "codex" => {
            let vm = resolve_codex_view_model_from(settings, p);
            let plan = normalize_plan_label(p);
            let title = if plan != "Unknown" {
                format!("Codex · {plan}")
            } else {
                "Codex".to_string()
            };
            render_pango(&build_codex(
                clock,
                p,
                &vm,
                &BuildOptions {
                    mode,
                    header_title: title,
                    header_width: header_width_waybar(),
                    label_color: ColorToken::Green,
                    footer_fetched_at: fetched,
                    plan_label: None,
                    amp_free_tier_layout: AmpLayout::Inline,
                    account_in_header: false,
                },
            ))
        }
        "amp" => {
            let title = match p.account.as_deref().filter(|s| !s.is_empty()) {
                Some(acc) => format!("Amp · {acc}"),
                None => "Amp".to_string(),
            };
            render_pango(&build_amp(
                clock,
                p,
                &BuildOptions {
                    mode,
                    header_title: title,
                    header_width: header_width_waybar(),
                    label_color: ColorToken::Magenta,
                    footer_fetched_at: fetched,
                    plan_label: None,
                    amp_free_tier_layout: AmpLayout::Inline,
                    account_in_header: true,
                },
            ))
        }
        _ => {
            let name = if p.display_name.is_empty() {
                p.provider.clone()
            } else {
                p.display_name.clone()
            };
            render_pango(&build_generic(
                clock,
                p,
                &BuildOptions {
                    mode,
                    header_title: name,
                    header_width: header_width_waybar(),
                    label_color: ColorToken::Text,
                    footer_fetched_at: fetched,
                    plan_label: None,
                    amp_free_tier_layout: AmpLayout::Inline,
                    account_in_header: false,
                },
            ))
        }
    }
}

fn build_text(quotas: &AllQuotas, mode: DisplayMode) -> String {
    let parts: Vec<String> = quotas
        .providers
        .iter()
        .filter(|p| p.available)
        .map(|p| pct_colored(to_window_display(p.primary.as_ref(), mode), mode))
        .collect();
    if parts.is_empty() {
        return span(ColorToken::Comment.hex(), "No Providers", false);
    }
    let sep = format!(" {} ", span(ColorToken::Comment.hex(), "│", false));
    parts.join(&sep)
}

fn build_tooltip(
    clock: &Clock,
    quotas: &AllQuotas,
    settings: &Settings,
    mode: DisplayMode,
) -> String {
    let sections: Vec<String> = quotas
        .providers
        .iter()
        .filter(|p| p.available || p.error.is_some())
        .map(|p| provider_tooltip(clock, p, Some(&quotas.fetched_at), settings, mode))
        .collect();
    sections.join("\n\n")
}

fn aggregate_class(quotas: &AllQuotas) -> String {
    let mut classes = vec![APP_BASE_CLASS.to_string()];
    for p in quotas.providers.iter().filter(|p| p.available) {
        let val = p.primary.as_ref().map(|w| w.remaining).unwrap_or(100.0);
        let status = status_for_percent(Some(val));
        classes.push(format!("{}-{}", p.provider, status.as_str()));
    }
    classes.join(" ")
}

pub fn format_for_waybar(
    clock: &Clock,
    quotas: &AllQuotas,
    settings: &Settings,
    mode: DisplayMode,
) -> WaybarOutput {
    WaybarOutput {
        text: build_text(quotas, mode),
        tooltip: build_tooltip(clock, quotas, settings, mode),
        class: aggregate_class(quotas),
        alt: None,
        percentage: None,
    }
}

pub fn format_provider_for_waybar(
    clock: &Clock,
    quota: &ProviderQuota,
    settings: &Settings,
    mode: DisplayMode,
) -> WaybarOutput {
    if !quota.available || quota.error.is_some() {
        return WaybarOutput {
            // Glyph nerd-font U+F1616 (confirmado vs src/formatters/waybar.ts:215).
            text: span(ColorToken::Red.hex(), "\u{f1616}", false),
            tooltip: provider_tooltip(clock, quota, None, settings, mode),
            class: format!("{}-{} disconnected", APP_BASE_CLASS, quota.provider),
            alt: Some("disconnected".to_string()),
            percentage: None,
        };
    }

    let rem = quota.primary.as_ref().map(|w| w.remaining);
    let disp = to_window_display(quota.primary.as_ref(), mode);
    let status = status_for_percent(Some(rem.unwrap_or(100.0)));

    let (alt, percentage) = match disp {
        Some(d) => (
            Some(status.as_str().to_string()),
            Some((d.round() as i64).clamp(0, 100) as u8),
        ),
        None => (None, None),
    };

    WaybarOutput {
        text: pct_colored(disp, mode),
        tooltip: provider_tooltip(clock, quota, None, settings, mode),
        class: format!("{}-{} {}", APP_BASE_CLASS, quota.provider, status.as_str()),
        alt,
        percentage,
    }
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
        })
    }
    fn claude(remaining: f64) -> ProviderQuota {
        ProviderQuota {
            provider: "claude".into(),
            display_name: "Claude".into(),
            available: true,
            account: None,
            plan: Some("Pro".into()),
            plan_type: None,
            primary: Some(QuotaWindow {
                remaining,
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
    fn aggregate_class_format() {
        let q = AllQuotas {
            providers: vec![claude(75.0)],
            fetched_at: "2026-03-28T12:00:00Z".into(),
        };
        let out = format_for_waybar(&clk(), &q, &settings(), DisplayMode::Remaining);
        assert_eq!(out.class, "agent-bar claude-ok");
        assert!(out.text.contains("75%"));
        assert!(out.tooltip.contains("Claude · Pro"));
        assert!(out.alt.is_none()); // agregado não tem alt
    }

    #[test]
    fn per_provider_class_and_alt() {
        let out =
            format_provider_for_waybar(&clk(), &claude(5.0), &settings(), DisplayMode::Remaining);
        assert_eq!(out.class, "agent-bar-claude critical"); // 5% < 10 → critical
        assert_eq!(out.alt.as_deref(), Some("critical"));
        assert_eq!(out.percentage, Some(5));
    }

    #[test]
    fn per_provider_disconnected() {
        let mut c = claude(50.0);
        c.available = false;
        c.error = Some("token expired".into());
        let out = format_provider_for_waybar(&clk(), &c, &settings(), DisplayMode::Remaining);
        assert_eq!(out.class, "agent-bar-claude disconnected");
        assert_eq!(out.alt.as_deref(), Some("disconnected"));
        assert!(out.percentage.is_none());
    }

    #[test]
    fn aggregate_empty_text() {
        let q = AllQuotas {
            providers: vec![],
            fetched_at: "2026-03-28T12:00:00Z".into(),
        };
        let out = format_for_waybar(&clk(), &q, &settings(), DisplayMode::Remaining);
        assert!(out.text.contains("No Providers"));
        assert_eq!(out.class, "agent-bar");
    }

    #[test]
    fn waybar_stdout_line_happy_path_matches_serde() {
        let o = WaybarOutput {
            text: "75%".to_string(),
            tooltip: "tip".to_string(),
            class: "agent-bar claude-ok".to_string(),
            alt: None,
            percentage: Some(75),
        };
        let expected = serde_json::to_string(&o).expect("serde ok");
        let mut err = Vec::new();
        let got = waybar_stdout_line(&o, &mut err);
        assert_eq!(got, expected);
        assert!(err.is_empty(), "happy path must not log");
    }

    #[test]
    fn waybar_error_payload_is_non_empty_json() {
        let o = waybar_error_payload("boom");
        let s = serde_json::to_string(&o).expect("fallback must serialize");
        assert!(!s.is_empty());
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert_eq!(
            v.get("text").and_then(|t| t.as_str()).map(|t| !t.is_empty()),
            Some(true)
        );
        assert_eq!(
            v.get("class").and_then(|c| c.as_str()),
            Some("agent-bar disconnected")
        );
        assert!(
            v.get("tooltip")
                .and_then(|t| t.as_str())
                .is_some_and(|t| t.contains("boom"))
        );
    }
}
