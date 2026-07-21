//! Builder do card Grok (Grok Build CLI). Cor de marca = Cyan.
//! Copy deixa explícito que o % é **contexto de sessão**, não cota de plano.

use crate::formatters::clock::Clock;
use crate::formatters::segments::{
    bar_segments, color_for_display, indicator_segments, Line, Segment,
};
use crate::formatters::shared::{format_percent, to_display};
use crate::providers::extras::get_grok_extra;
use crate::providers::types::ProviderQuota;
use crate::theme::{box_chars, ColorToken};

use super::shared::{build_footer_line, header_line, vline, BuildOptions};

const BRAND: ColorToken = ColorToken::Cyan;

/// Compacta tokens para tooltip: 39000 → "39k", 500000 → "500k".
fn fmt_tokens_compact(n: u64) -> String {
    if n >= 1_000_000 {
        let m = (n as f64 / 1_000_000.0).round() as u64;
        format!("{m}M")
    } else if n >= 1_000 {
        let k = (n as f64 / 1_000.0).round() as u64;
        format!("{k}k")
    } else {
        format!("{n}")
    }
}

/// Linha da barra de contexto: `┃  ● contexto ████  87%`.
fn contexto_bar_line(disp: Option<f64>, mode: crate::settings::DisplayMode) -> Line {
    let mut line: Line = vec![Segment::new(box_chars::V, BRAND), Segment::raw_text("  ")];
    line.extend(indicator_segments(disp, mode));
    line.push(Segment::raw_text(" "));
    line.push(Segment::new("contexto", ColorToken::TextBright));
    line.push(Segment::raw_text(" "));
    line.extend(bar_segments(disp, mode));
    line.push(Segment::raw_text(" "));
    line.push(Segment::new(
        format!("{:>4}", format_percent(disp)),
        color_for_display(disp, mode),
    ));
    line
}

pub fn build_grok(clock: &Clock, p: &ProviderQuota, options: &BuildOptions) -> Vec<Line> {
    let mode = options.mode;
    let mut lines: Vec<Line> = Vec::new();

    lines.push(header_line(
        &options.header_title,
        options.header_width,
        BRAND,
    ));
    lines.push(vline(BRAND));

    if let Some(err) = p.error.as_deref() {
        lines.push(vec![
            Segment::new(box_chars::V, BRAND),
            Segment::raw_text("  "),
            Segment::new(format!("⚠️ {err}"), ColorToken::Red),
        ]);
    } else if let Some(primary) = p.primary.as_ref() {
        let disp = to_display(Some(primary.remaining), mode);
        lines.push(contexto_bar_line(disp, mode));

        // Tokens da sessão recente (quando o extra os traz).
        if let Some(extra) = get_grok_extra(p) {
            if let (Some(used), Some(window)) =
                (extra.context_tokens_used, extra.context_window_tokens)
            {
                let text = format!(
                    "{} / {} tokens · sessão recente",
                    fmt_tokens_compact(used),
                    fmt_tokens_compact(window)
                );
                lines.push(vec![
                    Segment::new(box_chars::V, BRAND),
                    Segment::raw_text("  "),
                    Segment::new(text, ColorToken::Comment),
                ]);
            }
        }
    } else {
        lines.push(vec![
            Segment::new(box_chars::V, BRAND),
            Segment::raw_text("  "),
            Segment::new("sem sessões locais ainda", ColorToken::Muted),
        ]);
    }

    // Contagem do dia: `hoje  N sessões · M turns`.
    if p.error.is_none() {
        if let Some(extra) = get_grok_extra(p) {
            if extra.sessions_today.is_some() || extra.turns_today.is_some() {
                let sessions = extra.sessions_today.unwrap_or(0);
                let turns = extra.turns_today.unwrap_or(0);
                let text = format!("hoje  {sessions} sessões · {turns} turns");
                lines.push(vec![
                    Segment::new(box_chars::V, BRAND),
                    Segment::raw_text("  "),
                    Segment::new(text, ColorToken::Comment),
                ]);
            }
        }
    }

    if let Some(account) = p.account.as_deref().filter(|s| !s.is_empty()) {
        if !options.account_in_header {
            lines.push(vline(BRAND));
            lines.push(vec![
                Segment::new(box_chars::V, BRAND),
                Segment::raw_text("  "),
                Segment::new(format!("Account: {account}"), ColorToken::Comment),
            ]);
        }
    }

    lines.push(vline(BRAND));
    lines.push(build_footer_line(
        clock,
        options.footer_fetched_at.as_deref(),
        BRAND,
    ));

    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formatters::builders::shared::{AmpLayout, BuildOptions};
    use crate::formatters::clock::Clock;
    use crate::formatters::render_pango::render_pango;
    use crate::providers::types::{GrokQuotaExtra, ProviderExtra, ProviderQuota, QuotaWindow};
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
            header_title: "Grok · Grok 4.5".into(),
            header_width: 52,
            label_color: ColorToken::Cyan,
            footer_fetched_at: None,
            plan_label: None,
            amp_free_tier_layout: AmpLayout::Inline,
            account_in_header: false,
        }
    }

    fn quota_with_primary() -> ProviderQuota {
        ProviderQuota {
            provider: "grok".into(),
            display_name: "Grok".into(),
            available: true,
            account: Some("Test".into()),
            plan: Some("Grok 4.5".into()),
            plan_type: None,
            primary: Some(QuotaWindow {
                remaining: 87.0,
                resets_at: None,
                window_minutes: None,
                used: Some(13.0),
                severity: None,
            }),
            secondary: None,
            models: None,
            extra: Some(ProviderExtra::Grok(GrokQuotaExtra {
                sessions_today: Some(3),
                turns_today: Some(12),
                context_tokens_used: Some(39_000),
                context_window_tokens: Some(500_000),
                recent_model: Some("grok-4.5".into()),
            })),
            error: None,
        }
    }

    #[test]
    fn builder_mentions_context_not_plan_quota() {
        let out = render_pango(&build_grok(&clk(), &quota_with_primary(), &opts()));
        assert!(
            out.contains("contexto"),
            "must name session context, got:\n{out}"
        );
        assert!(!out.to_lowercase().contains("plano"));
        assert!(!out.to_lowercase().contains("quota de plano"));
        assert!(out.contains("87%"));
        assert!(out.contains("39k / 500k tokens"));
        assert!(out.contains("hoje  3 sessões · 12 turns"));
        assert!(out.contains("Account: Test"));
    }

    #[test]
    fn error_branch_is_red_text() {
        let mut q = quota_with_primary();
        q.error = Some("Not logged in. Open `agent-bar menu` and choose Provider login.".into());
        q.primary = None;
        let out = render_pango(&build_grok(&clk(), &q, &opts()));
        assert!(out.contains("Not logged in"));
        assert!(!out.contains("contexto"));
        assert!(!out.contains("sem sessões"));
    }

    #[test]
    fn no_primary_shows_empty_sessions() {
        let mut q = quota_with_primary();
        q.primary = None;
        q.extra = Some(ProviderExtra::Grok(GrokQuotaExtra {
            sessions_today: Some(0),
            turns_today: Some(0),
            context_tokens_used: None,
            context_window_tokens: None,
            recent_model: None,
        }));
        let out = render_pango(&build_grok(&clk(), &q, &opts()));
        assert!(out.contains("sem sessões locais ainda"));
        assert!(out.contains("hoje  0 sessões · 0 turns"));
    }

    #[test]
    fn fmt_tokens_compact_rounds_thousands() {
        assert_eq!(fmt_tokens_compact(0), "0");
        assert_eq!(fmt_tokens_compact(999), "999");
        assert_eq!(fmt_tokens_compact(39_000), "39k");
        assert_eq!(fmt_tokens_compact(500_000), "500k");
        assert_eq!(fmt_tokens_compact(1_500_000), "2M");
    }
}
