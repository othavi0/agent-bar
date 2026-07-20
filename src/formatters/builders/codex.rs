//! Builder do card Codex. Port fiel de `src/formatters/builders/codex.ts`.
//! Cor de marca = Green. Recebe o CodexViewModel já resolvido pela superfície.

use crate::formatters::clock::Clock;
use crate::formatters::segments::{
    bar_segments, color_for_display, indicator_segments, Line, Segment,
};
use crate::formatters::shared::{format_percent, to_display};
use crate::formatters::view_model::CodexViewModel;
use crate::providers::extras::get_codex_extra;
use crate::providers::types::ProviderQuota;
use crate::settings::WindowPolicy;
use crate::theme::{box_chars, ColorToken};

use super::shared::{build_footer_line, header_line, label_line, model_line, vline, BuildOptions};

pub fn build_codex(
    clock: &Clock,
    p: &ProviderQuota,
    view_model: &CodexViewModel,
    options: &BuildOptions,
) -> Vec<Line> {
    let mode = options.mode;
    let mut lines: Vec<Line> = Vec::new();

    lines.push(header_line(
        &options.header_title,
        options.header_width,
        ColorToken::Green,
    ));
    lines.push(vline(ColorToken::Green));

    if let Some(err) = p.error.as_deref() {
        lines.push(vec![
            Segment::new(box_chars::V, ColorToken::Green),
            Segment::raw_text("  "),
            Segment::new(format!("⚠️ {err}"), ColorToken::Red),
        ]);
    } else {
        let models = &view_model.models;
        let policy = view_model.policy;
        let max_len = 20;

        if let Some(plan_label) = options.plan_label.as_deref() {
            lines.push(vec![
                Segment::new(box_chars::V, ColorToken::Green),
                Segment::raw_text("  "),
                Segment::new(format!("Plan: {plan_label}"), ColorToken::Muted),
            ]);
        }

        if models.is_empty() {
            lines.push(vline(ColorToken::Green));
            lines.push(label_line(
                "Available Models",
                options.label_color,
                ColorToken::Green,
            ));
            lines.push(vec![
                Segment::new(box_chars::V, ColorToken::Green),
                Segment::raw_text("  "),
                Segment::new("No models selected", ColorToken::Comment),
            ]);
        } else {
            let model_len = models
                .iter()
                .map(|m| m.name.chars().count())
                .max()
                .unwrap_or(0)
                .max(max_len);

            if policy != WindowPolicy::SevenDay {
                lines.push(vline(ColorToken::Green));
                lines.push(label_line(
                    "5-hour limit",
                    options.label_color,
                    ColorToken::Green,
                ));
                for model in models {
                    lines.push(model_line(
                        clock,
                        &model.name,
                        model.windows.five_hour.as_ref(),
                        model_len,
                        mode,
                        ColorToken::Green,
                        Some("N/A"),
                    ));
                }
            }

            if policy != WindowPolicy::FiveHour {
                lines.push(vline(ColorToken::Green));
                lines.push(label_line(
                    "7-day limit",
                    options.label_color,
                    ColorToken::Green,
                ));
                for model in models {
                    lines.push(model_line(
                        clock,
                        &model.name,
                        model.windows.seven_day.as_ref(),
                        model_len,
                        mode,
                        ColorToken::Green,
                        Some("N/A"),
                    ));
                }
            }
        }

        if let Some(eu) = get_codex_extra(p).and_then(|e| e.extra_usage.as_ref()) {
            if eu.enabled {
                let disp = to_display(Some(eu.remaining), mode);
                lines.push(vline(ColorToken::Green));
                lines.push(label_line(
                    "Credits",
                    options.label_color,
                    ColorToken::Green,
                ));
                let limit_text = if eu.limit == -1.0 {
                    "Unlimited"
                } else {
                    "Balance"
                };
                let mut line: Line = vec![
                    Segment::new(box_chars::V, ColorToken::Green),
                    Segment::raw_text("  "),
                ];
                line.extend(indicator_segments(disp, mode));
                line.push(Segment::raw_text(" "));
                line.push(Segment::new(
                    format!("{:<max_len$}", "Balance"),
                    ColorToken::TextBright,
                ));
                line.push(Segment::raw_text(" "));
                line.extend(bar_segments(disp, mode));
                line.push(Segment::raw_text(" "));
                line.push(Segment::new(
                    format!("{:>4}", format_percent(disp)),
                    color_for_display(disp, mode),
                ));
                line.push(Segment::raw_text(" "));
                line.push(Segment::new(limit_text.to_string(), ColorToken::Cyan));
                lines.push(line);
            }
        }
    }

    lines.push(vline(ColorToken::Green));
    lines.push(build_footer_line(
        clock,
        options.footer_fetched_at.as_deref(),
        ColorToken::Green,
    ));

    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formatters::builders::shared::{AmpLayout, BuildOptions};
    use crate::formatters::clock::Clock;
    use crate::formatters::codex_helpers::CodexModelEntry;
    use crate::formatters::render_pango::render_pango;
    use crate::formatters::view_model::CodexViewModel;
    use crate::providers::types::{ModelWindows, ProviderQuota, QuotaWindow};
    use crate::settings::{DisplayMode, WindowPolicy};
    use crate::theme::ColorToken;
    use time::macros::datetime;

    fn clk() -> Clock {
        Clock {
            now: datetime!(2026-06-19 12:00:00 UTC),
            local_offset: time::UtcOffset::UTC,
        }
    }

    fn opts(plan_label: Option<&str>) -> BuildOptions {
        BuildOptions {
            mode: DisplayMode::Remaining,
            header_title: "Codex".into(),
            header_width: 52,
            label_color: ColorToken::Magenta,
            footer_fetched_at: None,
            plan_label: plan_label.map(|s| s.to_string()),
            amp_free_tier_layout: AmpLayout::Inline,
            account_in_header: false,
        }
    }

    fn quota() -> ProviderQuota {
        ProviderQuota {
            provider: "codex".into(),
            display_name: "Codex".into(),
            available: true,
            account: None,
            plan: None,
            plan_type: None,
            primary: None,
            secondary: None,
            models: None,
            extra: None,
            error: None,
            stale_reason: None,
        }
    }

    fn entry(name: &str, five: f64, seven: f64) -> CodexModelEntry {
        let w = |r: f64| QuotaWindow {
            remaining: r,
            resets_at: Some("2026-06-19T14:00:00Z".into()),
            window_minutes: None,
            used: None,
            severity: None,
        };
        CodexModelEntry {
            name: name.into(),
            windows: ModelWindows {
                five_hour: Some(w(five)),
                seven_day: Some(w(seven)),
                other: None,
            },
            severity: five.min(seven),
        }
    }

    #[test]
    fn both_policy_renders_both_sections() {
        let vm = CodexViewModel {
            models: vec![entry("gpt-5", 80.0, 50.0)],
            policy: WindowPolicy::Both,
        };
        let out = render_pango(&build_codex(&clk(), &quota(), &vm, &opts(Some("Pro"))));
        assert!(out.contains("Plan: Pro"));
        let i5 = out.find("5-hour limit").unwrap();
        let i7 = out.find("7-day limit").unwrap();
        assert!(i5 < i7);
        // null_eta_text "N/A" aparece quando não há resets... aqui há resets, então ETA real.
    }

    #[test]
    fn five_hour_policy_hides_seven_day() {
        let vm = CodexViewModel {
            models: vec![entry("gpt-5", 80.0, 50.0)],
            policy: WindowPolicy::FiveHour,
        };
        let out = render_pango(&build_codex(&clk(), &quota(), &vm, &opts(None)));
        assert!(out.contains("5-hour limit"));
        assert!(!out.contains("7-day limit"));
        assert!(!out.contains("Plan:")); // plan_label None → sem linha
    }

    #[test]
    fn empty_models_shows_placeholder() {
        let vm = CodexViewModel {
            models: vec![],
            policy: WindowPolicy::Both,
        };
        let out = render_pango(&build_codex(&clk(), &quota(), &vm, &opts(None)));
        assert!(out.contains("Available Models"));
        assert!(out.contains("No models selected"));
    }
}
