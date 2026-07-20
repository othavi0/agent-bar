//! Builder do card genérico (fallback para provider sem builder dedicado).
//! Port fiel de `src/formatters/builders/generic.ts`. Cor de marca = Text.
//! Sem `vline` separadores.

use crate::formatters::clock::Clock;
use crate::formatters::segments::{
    bar_segments, color_for_display, indicator_segments, Line, Segment,
};
use crate::formatters::shared::{format_percent, to_display};
use crate::providers::types::ProviderQuota;
use crate::theme::{box_chars, ColorToken};

use super::shared::{build_footer_line, header_line, BuildOptions};

pub fn build_generic(clock: &Clock, p: &ProviderQuota, options: &BuildOptions) -> Vec<Line> {
    let mut lines: Vec<Line> = Vec::new();

    lines.push(header_line(
        &options.header_title,
        options.header_width,
        ColorToken::Text,
    ));

    if let Some(err) = p.error.as_deref() {
        lines.push(vec![
            Segment::new(box_chars::V, ColorToken::Text),
            Segment::raw_text("  "),
            Segment::new(err.to_string(), ColorToken::Red),
        ]);
    } else if let Some(primary) = p.primary.as_ref() {
        let disp = to_display(Some(primary.remaining), options.mode);
        let mut line: Line = vec![
            Segment::new(box_chars::V, ColorToken::Text),
            Segment::raw_text("  "),
        ];
        line.extend(indicator_segments(disp, options.mode));
        line.push(Segment::raw_text(" "));
        line.extend(bar_segments(disp, options.mode));
        line.push(Segment::raw_text(" "));
        line.push(Segment::new(
            format!("{:>4}", format_percent(disp)),
            color_for_display(disp, options.mode),
        ));
        lines.push(line);
    }

    lines.push(build_footer_line(
        clock,
        options.footer_fetched_at.as_deref(),
        ColorToken::Text,
    ));

    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formatters::builders::shared::{AmpLayout, BuildOptions};
    use crate::formatters::clock::Clock;
    use crate::formatters::render_pango::render_pango;
    use crate::providers::types::{ProviderQuota, QuotaWindow};
    use crate::settings::DisplayMode;
    use crate::theme::ColorToken;
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
            header_title: "Foo".into(),
            header_width: 52,
            label_color: ColorToken::Text,
            footer_fetched_at: None,
            plan_label: None,
            amp_free_tier_layout: AmpLayout::Inline,
            account_in_header: false,
        }
    }

    fn quota() -> ProviderQuota {
        ProviderQuota {
            provider: "foo".into(),
            display_name: "Foo".into(),
            available: true,
            account: None,
            plan: None,
            plan_type: None,
            primary: Some(QuotaWindow {
                remaining: 60.0,
                resets_at: None,
                window_minutes: None,
                used: None,
                severity: None,
            }),
            secondary: None,
            models: None,
            extra: None,
            error: None,
            stale_reason: None,
        }
    }

    #[test]
    fn renders_header_primary_footer() {
        let lines = build_generic(&clk(), &quota(), &opts());
        assert_eq!(lines.len(), 3); // header + primary + footer (sem vline)
        let out = render_pango(&lines);
        assert!(out.contains("Foo"));
        assert!(out.contains("60%"));
    }

    #[test]
    fn error_branch_replaces_primary() {
        let mut q = quota();
        q.error = Some("boom".into());
        q.primary = None;
        let lines = build_generic(&clk(), &q, &opts());
        assert_eq!(lines.len(), 3); // header + error + footer
        let out = render_pango(&lines);
        assert!(out.contains("boom"));
    }

    #[test]
    fn no_primary_no_error_omits_middle() {
        let mut q = quota();
        q.primary = None;
        let lines = build_generic(&clk(), &q, &opts());
        assert_eq!(lines.len(), 2); // header + footer
    }
}
