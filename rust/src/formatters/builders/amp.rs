//! Builder do card Amp. Port fiel de `src/formatters/builders/amp.ts`.
//! Cor de marca = Magenta. Três layouts de Free Tier: Generic/Sublines/Inline.

use std::collections::BTreeMap;

use crate::formatters::clock::Clock;
use crate::formatters::segments::{
    bar_segments, color_for_display, indicator_segments, Line, Segment,
};
use crate::formatters::shared::{
    eta_label, format_eta, format_percent, format_reset_time, to_display,
};
use crate::providers::extras::get_amp_extra;
use crate::providers::types::ProviderQuota;
use crate::settings::DisplayMode;
use crate::theme::{box_chars, ColorToken};

use super::shared::{build_footer_line, header_line, label_line, vline, AmpLayout, BuildOptions};

/// Valor de meta com semântica truthy do JS: Some só se presente E não-vazio.
fn meta_get<'a>(m: &'a BTreeMap<String, String>, k: &str) -> Option<&'a str> {
    m.get(k).map(|s| s.as_str()).filter(|s| !s.is_empty())
}

/// Junta freeRemaining/freeTotal (truthy) com " / ".
fn dollars(m: &BTreeMap<String, String>) -> String {
    [meta_get(m, "freeRemaining"), meta_get(m, "freeTotal")]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" / ")
}

/// Linha da barra do Free Tier (compartilhada por sublines/inline). `eta_segments`
/// é anexado ao fim (inline anexa ETA; sublines passa vazio).
fn free_tier_bar_line(disp: Option<f64>, mode: DisplayMode, eta_segments: Line) -> Line {
    let mut line: Line = vec![
        Segment::new(box_chars::V, ColorToken::Magenta),
        Segment::raw_text("  "),
    ];
    line.extend(indicator_segments(disp, mode));
    line.push(Segment::raw_text(" "));
    line.extend(bar_segments(disp, mode));
    line.push(Segment::raw_text(" "));
    line.push(Segment::new(
        format!("{:>4}", format_percent(disp)),
        color_for_display(disp, mode),
    ));
    line.extend(eta_segments);
    line
}

/// Linha genérica de model (indicador + nome + barra + pct), sem ETA.
fn generic_model_line(name: &str, max_len: usize, disp: Option<f64>, mode: DisplayMode) -> Line {
    let mut line: Line = vec![
        Segment::new(box_chars::V, ColorToken::Magenta),
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
    line
}

pub fn build_amp(clock: &Clock, p: &ProviderQuota, options: &BuildOptions) -> Vec<Line> {
    let mode = options.mode;
    let layout = options.amp_free_tier_layout;
    let empty_meta = BTreeMap::new();
    let m: &BTreeMap<String, String> = get_amp_extra(p)
        .and_then(|e| e.meta.as_ref())
        .unwrap_or(&empty_meta);

    let mut lines: Vec<Line> = Vec::new();

    lines.push(header_line(
        &options.header_title,
        options.header_width,
        ColorToken::Magenta,
    ));
    lines.push(vline(ColorToken::Magenta));

    if let Some(err) = p.error.as_deref() {
        lines.push(vec![
            Segment::new(box_chars::V, ColorToken::Magenta),
            Segment::raw_text("  "),
            Segment::new(format!("⚠️ {err}"), ColorToken::Red),
        ]);
    } else if layout == AmpLayout::Generic {
        // TUI: loop genérico, sem special-casing de Free Tier/Credits.
        match p.models.as_ref().filter(|mm| !mm.is_empty()) {
            None => lines.push(vec![
                Segment::new(box_chars::V, ColorToken::Magenta),
                Segment::raw_text("  "),
                Segment::new("No usage data", ColorToken::Muted),
            ]),
            Some(models) => {
                let max_len = models
                    .keys()
                    .map(|n| n.chars().count())
                    .max()
                    .unwrap_or(0)
                    .max(20);
                lines.push(label_line(
                    "Usage",
                    options.label_color,
                    ColorToken::Magenta,
                ));
                for (name, window) in models {
                    let disp = to_display(Some(window.remaining), mode);
                    lines.push(generic_model_line(name, max_len, disp, mode));
                }
            }
        }
    } else {
        // Terminal (sublines) e Waybar (inline).
        let free = p.models.as_ref().and_then(|mm| mm.get("Free Tier"));

        if let Some(free) = free {
            let rem = free.remaining;
            let disp = to_display(Some(rem), mode);
            lines.push(label_line(
                "Free Tier",
                options.label_color,
                ColorToken::Magenta,
            ));

            if layout == AmpLayout::Sublines {
                lines.push(free_tier_bar_line(disp, mode, Vec::new()));

                let mut subs: Vec<Line> = Vec::new();

                // Sub-linha de dólares: replenishRate  ( freeRemaining / freeTotal )  bonus
                let mut dollar_parts: Line = Vec::new();
                if let Some(rate) = meta_get(m, "replenishRate") {
                    dollar_parts.push(Segment::new(rate.to_string(), ColorToken::Cyan));
                }
                let d = dollars(m);
                if !d.is_empty() {
                    if !dollar_parts.is_empty() {
                        dollar_parts.push(Segment::raw_text("  "));
                    }
                    dollar_parts.push(Segment::new(format!("( {d} )"), ColorToken::Text));
                }
                if let Some(bonus) = meta_get(m, "bonus") {
                    if !dollar_parts.is_empty() {
                        dollar_parts.push(Segment::raw_text("  "));
                    }
                    dollar_parts.push(Segment::new(bonus.to_string(), ColorToken::Cyan));
                }
                if !dollar_parts.is_empty() {
                    subs.push(dollar_parts);
                }

                // Sub-linha de ETA (só com resets e não cheio).
                if free.resets_at.is_some() && rem != 100.0 {
                    let eta_text = format!(
                        "{} {}  {}",
                        eta_label(mode),
                        format_eta(clock, free.resets_at.as_deref(), rem),
                        format_reset_time(clock, free.resets_at.as_deref(), rem)
                    );
                    subs.push(vec![Segment::new(eta_text, ColorToken::Cyan)]);
                }

                let last = subs.len().saturating_sub(1);
                for (i, sub) in subs.into_iter().enumerate() {
                    let conn = if i == last { "└─" } else { "├─" };
                    let mut line: Line = vec![
                        Segment::new(box_chars::V, ColorToken::Magenta),
                        Segment::raw_text("  "),
                        Segment::new(conn, ColorToken::Comment),
                        Segment::raw_text(" "),
                    ];
                    line.extend(sub);
                    lines.push(line);
                }
            } else {
                // Inline (Waybar): barra com ETA anexado; dólares na linha ○.
                let eta_segs: Line = if free.resets_at.is_some() && rem != 100.0 {
                    vec![
                        Segment::raw_text("  "),
                        Segment::new(
                            format!(
                                "→ {} {} {}",
                                eta_label(mode),
                                format_eta(clock, free.resets_at.as_deref(), rem),
                                format_reset_time(clock, free.resets_at.as_deref(), rem)
                            ),
                            ColorToken::Cyan,
                        ),
                    ]
                } else {
                    Vec::new()
                };
                lines.push(free_tier_bar_line(disp, mode, eta_segs));

                let mut info_parts: Line = Vec::new();
                if let Some(rate) = meta_get(m, "replenishRate") {
                    info_parts.push(Segment::new(rate.to_string(), ColorToken::Cyan));
                }
                let d = dollars(m);
                if !d.is_empty() {
                    if !info_parts.is_empty() {
                        info_parts.push(Segment::new("  |  ", ColorToken::Comment));
                    }
                    info_parts.push(Segment::new(d, ColorToken::Text));
                }
                if let Some(bonus) = meta_get(m, "bonus") {
                    if !info_parts.is_empty() {
                        info_parts.push(Segment::new("  |  ", ColorToken::Comment));
                    }
                    info_parts.push(Segment::new(bonus.to_string(), ColorToken::Cyan));
                }
                if !info_parts.is_empty() {
                    let mut line: Line = vec![
                        Segment::new(box_chars::V, ColorToken::Magenta),
                        Segment::raw_text("  "),
                        Segment::new(box_chars::DOT_O, ColorToken::Comment),
                        Segment::raw_text(" "),
                    ];
                    line.extend(info_parts);
                    lines.push(line);
                }
            }
        }

        // Credits (terminal + waybar).
        let credits = p.models.as_ref().and_then(|mm| mm.get("Credits"));
        if let Some(credits) = credits {
            lines.push(vline(ColorToken::Magenta));
            let balance = m.get("creditsBalance").map(|s| s.as_str()).unwrap_or("$0");
            let credit_color = if credits.remaining > 0.0 {
                ColorToken::Green
            } else {
                ColorToken::Comment
            };
            lines.push(label_line(
                "Credits",
                options.label_color,
                ColorToken::Magenta,
            ));
            let credit_disp = to_display(Some(credits.remaining), mode);
            let balance_text = if layout == AmpLayout::Inline {
                format!("{balance} remaining")
            } else {
                balance.to_string()
            };
            let mut line: Line = vec![
                Segment::new(box_chars::V, ColorToken::Magenta),
                Segment::raw_text("  "),
            ];
            line.extend(indicator_segments(credit_disp, mode));
            line.push(Segment::raw_text(" "));
            line.push(Segment::new(balance_text, credit_color));
            lines.push(line);
        }

        // Fallback p/ models desconhecidos (nem Free Tier nem Credits).
        if free.is_none() && credits.is_none() {
            if let Some(models) = p.models.as_ref().filter(|mm| !mm.is_empty()) {
                let max_len = models
                    .keys()
                    .map(|n| n.chars().count())
                    .max()
                    .unwrap_or(0)
                    .max(20);
                lines.push(label_line(
                    "Usage",
                    options.label_color,
                    ColorToken::Magenta,
                ));
                for (name, window) in models {
                    let disp = to_display(Some(window.remaining), mode);
                    lines.push(generic_model_line(name, max_len, disp, mode));
                }
            }
        }
    }

    // Account line — omitida quando a superfície já mostra a conta no header.
    if let Some(account) = p.account.as_deref().filter(|s| !s.is_empty()) {
        if !options.account_in_header {
            lines.push(vline(ColorToken::Magenta));
            lines.push(vec![
                Segment::new(box_chars::V, ColorToken::Magenta),
                Segment::raw_text("  "),
                Segment::new(format!("Account: {account}"), ColorToken::Comment),
            ]);
        }
    }

    lines.push(vline(ColorToken::Magenta));
    lines.push(build_footer_line(
        clock,
        options.footer_fetched_at.as_deref(),
        ColorToken::Magenta,
    ));

    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formatters::builders::shared::{AmpLayout, BuildOptions};
    use crate::formatters::clock::Clock;
    use crate::formatters::render_pango::render_pango;
    use crate::providers::types::{AmpQuotaExtra, ProviderExtra, ProviderQuota, QuotaWindow};
    use crate::settings::DisplayMode;
    use crate::theme::ColorToken;
    use std::collections::BTreeMap;
    use time::macros::datetime;

    fn clk() -> Clock {
        Clock {
            now: datetime!(2026-06-19 12:00:00 UTC),
            local_offset: time::UtcOffset::UTC,
        }
    }

    fn opts(layout: AmpLayout) -> BuildOptions {
        BuildOptions {
            mode: DisplayMode::Remaining,
            header_title: "Amp".into(),
            header_width: 52,
            label_color: ColorToken::Magenta,
            footer_fetched_at: None,
            plan_label: None,
            amp_free_tier_layout: layout,
            account_in_header: false,
        }
    }

    fn win(r: f64) -> QuotaWindow {
        QuotaWindow {
            remaining: r,
            resets_at: Some("2026-06-19T14:00:00Z".into()),
            window_minutes: None,
            used: None,
        }
    }

    fn amp_with_free_and_credits() -> ProviderQuota {
        let mut models = BTreeMap::new();
        models.insert("Free Tier".to_string(), win(30.0));
        models.insert("Credits".to_string(), win(75.0));
        let mut meta = BTreeMap::new();
        meta.insert("freeRemaining".to_string(), "$1.50".to_string());
        meta.insert("freeTotal".to_string(), "$5.00".to_string());
        meta.insert("replenishRate".to_string(), "+$0.10/h".to_string());
        meta.insert("creditsBalance".to_string(), "$12".to_string());
        ProviderQuota {
            provider: "amp".into(),
            display_name: "Amp".into(),
            available: true,
            account: Some("me@x.com".into()),
            plan: None,
            plan_type: None,
            primary: Some(win(30.0)),
            secondary: None,
            models: Some(models),
            extra: Some(ProviderExtra::Amp(AmpQuotaExtra { meta: Some(meta) })),
            error: None,
        }
    }

    #[test]
    fn inline_layout_has_free_tier_and_credits_and_account() {
        let out = render_pango(&build_amp(
            &clk(),
            &amp_with_free_and_credits(),
            &opts(AmpLayout::Inline),
        ));
        assert!(out.contains("Free Tier"));
        assert!(out.contains("Credits"));
        assert!(out.contains("$12 remaining")); // inline anexa " remaining"
        assert!(out.contains("Account: me@x.com"));
        // ○ line de dólares (inline usa separador "  |  ")
        assert!(out.contains("$1.50 / $5.00"));
    }

    #[test]
    fn sublines_layout_uses_tree_connectors() {
        let out = render_pango(&build_amp(
            &clk(),
            &amp_with_free_and_credits(),
            &opts(AmpLayout::Sublines),
        ));
        assert!(out.contains("Free Tier"));
        // sublines: dólares entre parênteses + último connector └─
        assert!(out.contains("( $1.50 / $5.00 )"));
        assert!(out.contains("└─"));
        // sublines NÃO anexa " remaining" ao balance
        assert!(out.contains("$12"));
    }

    #[test]
    fn generic_layout_loops_models() {
        // generic ignora special-casing de Free Tier/Credits e itera p.models.
        let q = amp_with_free_and_credits();
        let out = render_pango(&build_amp(&clk(), &q, &opts(AmpLayout::Generic)));
        assert!(out.contains("Usage"));
        // Account line é independente do layout → presente também no generic.
        assert!(out.contains("Account: me@x.com"));
    }

    #[test]
    fn account_omitted_when_in_header() {
        let mut o = opts(AmpLayout::Inline);
        o.account_in_header = true;
        let out = render_pango(&build_amp(&clk(), &amp_with_free_and_credits(), &o));
        assert!(!out.contains("Account:"));
    }

    #[test]
    fn error_branch() {
        let mut q = amp_with_free_and_credits();
        q.error = Some("rate limited".into());
        let out = render_pango(&build_amp(&clk(), &q, &opts(AmpLayout::Inline)));
        assert!(out.contains("⚠️ rate limited"));
        assert!(!out.contains("Free Tier"));
    }
}
