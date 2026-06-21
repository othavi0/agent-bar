//! Builder do card Claude. Port fiel de `src/formatters/builders/claude.ts`.
//! Cor de marca = Orange.

use crate::formatters::clock::Clock;
use crate::formatters::segments::{
    bar_segments, color_for_display, indicator_segments, Line, Segment,
};
use crate::formatters::shared::{format_percent, to_display};
use crate::providers::extras::get_claude_extra;
use crate::providers::types::ProviderQuota;
use crate::settings::DisplayMode;
use crate::theme::{box_chars, ColorToken};

use super::shared::{build_footer_line, header_line, label_line, model_line, vline, BuildOptions};

/// Linha de Extra Usage: indicador + nome + barra + pct + texto `$used/$limit`.
fn extra_usage_line(
    name: &str,
    max_len: usize,
    disp: Option<f64>,
    mode: DisplayMode,
    used_str: &str,
) -> Line {
    let mut line: Line = vec![
        Segment::new(box_chars::V, ColorToken::Orange),
        Segment::raw_text("  "),
    ];
    line.extend(indicator_segments(disp, mode));
    line.push(Segment::raw_text(" "));
    line.push(Segment::new(
        format!("{name:<max_len$}"),
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
    line.push(Segment::new(used_str.to_string(), ColorToken::Cyan));
    line
}

pub fn build_claude(clock: &Clock, p: &ProviderQuota, options: &BuildOptions) -> Vec<Line> {
    let mode = options.mode;
    let mut lines: Vec<Line> = Vec::new();

    lines.push(header_line(
        &options.header_title,
        options.header_width,
        ColorToken::Orange,
    ));
    lines.push(vline(ColorToken::Orange));

    if let Some(err) = p.error.as_deref() {
        lines.push(vec![
            Segment::new(box_chars::V, ColorToken::Orange),
            Segment::raw_text("  "),
            Segment::new(format!("⚠️ {err}"), ColorToken::Red),
        ]);
    } else {
        let max_len = 20;

        if let Some(primary) = p.primary.as_ref() {
            lines.push(label_line(
                "5-hour limit (shared)",
                options.label_color,
                ColorToken::Orange,
            ));
            lines.push(model_line(
                clock,
                "All Models",
                Some(primary),
                max_len,
                mode,
                ColorToken::Orange,
                None,
            ));
        }

        let weekly = get_claude_extra(p).and_then(|e| e.weekly_models.as_ref());
        if let Some(weekly) = weekly.filter(|w| !w.is_empty()) {
            lines.push(vline(ColorToken::Orange));
            lines.push(label_line(
                "Weekly per model",
                options.label_color,
                ColorToken::Orange,
            ));
            let w_max_len = weekly
                .keys()
                .map(|n| n.chars().count())
                .max()
                .unwrap_or(0)
                .max(max_len);
            for (name, window) in weekly {
                lines.push(model_line(
                    clock,
                    name,
                    Some(window),
                    w_max_len,
                    mode,
                    ColorToken::Orange,
                    None,
                ));
            }
        }

        if let Some(secondary) = p.secondary.as_ref() {
            lines.push(vline(ColorToken::Orange));
            lines.push(label_line(
                "Weekly limit (shared)",
                options.label_color,
                ColorToken::Orange,
            ));
            lines.push(model_line(
                clock,
                "All Models",
                Some(secondary),
                max_len,
                mode,
                ColorToken::Orange,
                None,
            ));
        }

        if let Some(eu) = get_claude_extra(p).and_then(|e| e.extra_usage.as_ref()) {
            if eu.enabled && eu.limit > 0.0 {
                let disp = to_display(Some(eu.remaining), mode);
                lines.push(vline(ColorToken::Orange));
                lines.push(label_line(
                    "Extra Usage",
                    options.label_color,
                    ColorToken::Orange,
                ));
                let used_str = format!("${:.2}/${:.2}", eu.used / 100.0, eu.limit / 100.0);
                lines.push(extra_usage_line("Budget", max_len, disp, mode, &used_str));
            }
        }
    }

    lines.push(vline(ColorToken::Orange));
    lines.push(build_footer_line(
        clock,
        options.footer_fetched_at.as_deref(),
        ColorToken::Orange,
    ));

    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formatters::builders::shared::{AmpLayout, BuildOptions};
    use crate::formatters::clock::Clock;
    use crate::formatters::render_pango::render_pango;
    use crate::providers::types::{
        ClaudeQuotaExtra, ExtraUsage, ProviderExtra, ProviderQuota, QuotaWindow,
    };
    use crate::settings::DisplayMode;
    use crate::theme::ColorToken;
    use indexmap::IndexMap;
    use time::macros::datetime;

    fn clk() -> Clock {
        Clock {
            now: datetime!(2026-06-19 12:00:00 UTC),
            local_offset: time::UtcOffset::UTC,
        }
    }

    fn opts() -> BuildOptions {
        BuildOptions {
            mode: DisplayMode::Remaining,
            header_title: "Claude".into(),
            header_width: 52,
            label_color: ColorToken::Orange,
            footer_fetched_at: None,
            plan_label: None,
            amp_free_tier_layout: AmpLayout::Inline,
            account_in_header: false,
        }
    }

    fn win(r: f64, m: Option<i64>) -> QuotaWindow {
        QuotaWindow {
            remaining: r,
            resets_at: Some("2026-06-19T14:00:00Z".into()),
            window_minutes: m,
            used: None,
        }
    }

    fn base() -> ProviderQuota {
        ProviderQuota {
            provider: "claude".into(),
            display_name: "Claude".into(),
            available: true,
            account: None,
            plan: Some("Pro".into()),
            plan_type: None,
            primary: Some(win(60.0, Some(300))),
            secondary: Some(win(50.0, Some(10080))),
            models: None,
            extra: None,
            error: None,
        }
    }

    #[test]
    fn renders_all_sections_in_order() {
        let mut q = base();
        let mut weekly = IndexMap::new();
        weekly.insert("claude-opus-4-5".to_string(), win(40.0, Some(10080)));
        weekly.insert("claude-sonnet-4-5".to_string(), win(65.0, Some(10080)));
        q.extra = Some(ProviderExtra::Claude(ClaudeQuotaExtra {
            weekly_models: Some(weekly),
            extra_usage: Some(ExtraUsage {
                enabled: true,
                remaining: 55.0,
                limit: 5000.0,
                used: 2250.0,
            }),
        }));
        let out = render_pango(&build_claude(&clk(), &q, &opts()));
        // ordem das seções
        let i5 = out.find("5-hour limit (shared)").unwrap();
        let iw = out.find("Weekly per model").unwrap();
        let iws = out.find("Weekly limit (shared)").unwrap();
        let ie = out.find("Extra Usage").unwrap();
        assert!(i5 < iw && iw < iws && iws < ie);
        // weeklyModels em ordem de inserção (IndexMap = Object.entries do TS)
        assert!(out.find("claude-opus-4-5").unwrap() < out.find("claude-sonnet-4-5").unwrap());
        // extraUsage formata centavos
        assert!(out.contains("$22.50/$50.00"));
    }

    #[test]
    fn error_branch() {
        let mut q = base();
        q.error = Some("token expired".into());
        let out = render_pango(&build_claude(&clk(), &q, &opts()));
        assert!(out.contains("⚠️ token expired"));
        assert!(!out.contains("5-hour limit"));
    }

    #[test]
    fn extra_usage_gated_by_limit_positive() {
        let mut q = base();
        q.extra = Some(ProviderExtra::Claude(ClaudeQuotaExtra {
            weekly_models: None,
            extra_usage: Some(ExtraUsage {
                enabled: true,
                remaining: 0.0,
                limit: 0.0,
                used: 0.0,
            }),
        }));
        let out = render_pango(&build_claude(&clk(), &q, &opts()));
        assert!(!out.contains("Extra Usage")); // limit == 0 → seção omitida
    }
}
