//! Primitivos compartilhados pelos builders por-provider (Plano 3b). Funções puras
//! `-> Line`. Layout: borda de 56 chars; box-drawing pesado.

use crate::formatters::clock::Clock;
use crate::formatters::segments::{
    bar_segments, color_for_display, indicator_segments, Line, Segment,
};
use crate::formatters::shared::{
    format_ago, format_eta, format_percent, format_reset_time, to_window_display,
};
use crate::providers::types::{ProviderQuota, QuotaWindow};
use crate::settings::DisplayMode;
use crate::theme::{box_chars, ColorToken};

pub const TOOLTIP_BORDER: usize = 56;

/// Opções resolvidas pela superfície (waybar/terminal/tui) e passadas ao builder.
#[derive(Debug, Clone)]
pub struct BuildOptions {
    pub mode: DisplayMode,
    pub header_title: String,
    pub header_width: usize,
    pub label_color: ColorToken,
    pub footer_fetched_at: Option<String>,
    pub plan_label: Option<String>,
    pub amp_free_tier_layout: AmpLayout,
    pub account_in_header: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AmpLayout {
    Sublines,
    Inline,
    Generic,
}

/// Linha vertical vazia com o accent do provider: `┃`.
pub fn vline(color: ColorToken) -> Line {
    vec![Segment::new(box_chars::V, color)]
}

/// `┣━ ◆ {text}` — connector + diamante + label em bold.
pub fn label_line(text: &str, label_color: ColorToken, connector_color: ColorToken) -> Line {
    vec![
        Segment::new(
            format!("{}{}", box_chars::LT, box_chars::H),
            connector_color,
        ),
        Segment::raw_text(" "),
        Segment::new(format!("{} {}", box_chars::DIAMOND, text), label_color).bold(),
    ]
}

/// `┏━ {title} ━…` preenchido até `header_width`.
pub fn header_line(title: &str, header_width: usize, color: ColorToken) -> Line {
    let fill = header_width.saturating_sub(title.chars().count()).max(1);
    vec![
        Segment::new(format!("{}{}", box_chars::TL, box_chars::H), color),
        Segment::raw_text(" "),
        Segment::new(title.to_string(), color).bold(),
        Segment::raw_text(" "),
        Segment::new(box_chars::H.repeat(fill), color),
    ]
}

/// Linha de aviso quando a quota veio de cache vencido (erro transitório).
/// `vline_color` deve casar com a cor usada pelos demais `┃` do builder chamador.
/// Builders nunca escapam — o escape é do render_pango.
pub fn stale_line(p: &ProviderQuota, vline_color: ColorToken) -> Option<Line> {
    let reason = p.stale_reason.as_deref()?;
    Some(vec![
        Segment::new(box_chars::V, vline_color),
        Segment::raw_text("  "),
        Segment::new(format!("⚠️ Cached data — {reason}"), ColorToken::Yellow),
    ])
}

/// `┗━…[ cached · {ago} ]…` sempre com 56 chars de largura total.
pub fn build_footer_line(clock: &Clock, fetched_at: Option<&str>, color: ColorToken) -> Line {
    match fetched_at {
        None => vec![Segment::new(
            format!("{}{}", box_chars::BL, box_chars::H.repeat(55)),
            color,
        )],
        Some(iso) => {
            let stamp = format!(" cached · {} ", format_ago(clock, iso));
            let total_dashes = (TOOLTIP_BORDER - 1).saturating_sub(stamp.chars().count());
            let left = (total_dashes / 2).max(1);
            let right = total_dashes.saturating_sub(left).max(1);
            vec![
                Segment::new(
                    format!("{}{}", box_chars::BL, box_chars::H.repeat(left)),
                    color,
                ),
                Segment::new(stamp, ColorToken::Comment),
                Segment::new(box_chars::H.repeat(right), color),
            ]
        }
    }
}

/// Linha de modelo: `┃  {indicador} {nome:<maxlen} {barra}  {pct:>4} → {eta} {reset}`.
#[allow(clippy::too_many_arguments)]
pub fn model_line(
    clock: &Clock,
    name: &str,
    window: Option<&QuotaWindow>,
    max_len: usize,
    mode: DisplayMode,
    provider_color: ColorToken,
    null_eta_text: Option<&str>,
) -> Line {
    let disp = to_window_display(window, mode);
    let reset = window.and_then(|w| w.resets_at.as_deref());
    let rem = window.map(|w| w.remaining).unwrap_or(0.0);

    let eta_text = match (null_eta_text, reset) {
        (Some(na), None) => format!("→ {na}"),
        _ => format!(
            "→ {} {}",
            format_eta(clock, reset, rem),
            format_reset_time(clock, reset, rem)
        ),
    };

    let mut line: Line = vec![
        Segment::new(box_chars::V, provider_color),
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
    line.push(Segment::new(eta_text, ColorToken::Cyan));
    line
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formatters::clock::Clock;
    use crate::formatters::render_pango::render_pango;
    use crate::providers::types::QuotaWindow;
    use crate::settings::DisplayMode;
    use crate::theme::ColorToken;
    use time::macros::datetime;

    fn clk() -> Clock {
        Clock {
            now: datetime!(2026-06-19 12:00:00 UTC),
            local_offset: time::UtcOffset::UTC,
        }
    }

    #[test]
    fn vline_is_single_accent_bar() {
        let l = vline(ColorToken::Orange);
        assert_eq!(l.len(), 1);
        assert_eq!(l[0].text, "┃");
        assert_eq!(l[0].color, ColorToken::Orange);
    }

    #[test]
    fn label_line_renders_connector_diamond_label() {
        let out = render_pango(&[label_line(
            "Models",
            ColorToken::Magenta,
            ColorToken::Magenta,
        )]);
        // ┣━ + ' ' (raw) + ◆ Models
        assert!(out.contains("┣━"));
        assert!(out.contains("◆ Models"));
    }

    #[test]
    fn header_line_pads_to_width() {
        // headerWidth 10, title "AB" → fill = 8 dashes
        let l = header_line("AB", 10, ColorToken::Orange);
        let _dashes: usize = l
            .iter()
            .filter(|s| s.text.chars().all(|c| c == '━'))
            .map(|s| s.text.chars().count())
            .sum();
        // ┏━ tem 1 ━; o fill tem 8 → total ━ = 9; aqui checamos só o fill segment
        let fill = l.last().unwrap();
        assert_eq!(fill.text.chars().count(), 8);
    }

    #[test]
    fn footer_simple_is_56_wide() {
        let l = build_footer_line(&clk(), None, ColorToken::Orange);
        let total: usize = l.iter().map(|s| s.text.chars().count()).sum();
        assert_eq!(total, 56); // ┗ + 55×━
    }

    #[test]
    fn footer_cached_is_56_wide_with_stamp() {
        // fetched 30min atrás → ' cached · 30m ago '
        let l = build_footer_line(&clk(), Some("2026-06-19T11:30:00Z"), ColorToken::Orange);
        let total: usize = l.iter().map(|s| s.text.chars().count()).sum();
        assert_eq!(total, 56);
        let rendered: String = l.iter().map(|s| s.text.as_ref()).collect();
        assert!(rendered.contains(" cached · 30m ago "));
    }

    #[test]
    fn model_line_segment_shape() {
        let w = QuotaWindow {
            remaining: 75.0,
            resets_at: Some("2026-06-19T14:00:00Z".into()),
            window_minutes: None,
            used: None,
            severity: None,
        };
        let l = model_line(
            &clk(),
            "All Models",
            Some(&w),
            20,
            DisplayMode::Remaining,
            ColorToken::Orange,
            None,
        );
        // primeiro segment = bar vertical accent; contém nome padded a 20 e '75%' e '→'
        assert_eq!(l[0].text, "┃");
        assert_eq!(l[0].color, ColorToken::Orange);
        let rendered: String = l.iter().map(|s| s.text.as_ref()).collect();
        assert!(rendered.contains("All Models")); // padEnd 20
        assert!(rendered.contains("75%"));
        assert!(rendered.contains("→ "));
    }

    #[test]
    fn model_line_null_eta_override() {
        let w = QuotaWindow {
            remaining: 50.0,
            resets_at: None,
            window_minutes: None,
            used: None,
            severity: None,
        };
        let l = model_line(
            &clk(),
            "X",
            Some(&w),
            5,
            DisplayMode::Remaining,
            ColorToken::Green,
            Some("N/A"),
        );
        let rendered: String = l.iter().map(|s| s.text.as_ref()).collect();
        assert!(rendered.contains("→ N/A"));
    }
}
