//! QuotaGauge: block-bar helpers for rendering remaining-quota percentages.
//!
//! These are pure functions, not Widget impls, to keep ownership simple and
//! allow callers to compose them into Spans inside larger Lines/Paragraphs.

use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

use crate::theme::ColorToken;
use crate::tui::theme_bridge::to_ratatui;
use crate::tui::widgets::severity::severity_color;

/// Builds a block-bar string of `width` characters.
/// Filled chars (█) represent remaining quota; empty (░) represent consumed.
/// 100% remaining → all filled; 0% remaining → all empty.
pub fn block_bar(remaining_pct: f64, width: usize) -> String {
    let remaining = remaining_pct.clamp(0.0, 100.0);
    let filled = ((remaining / 100.0) * width as f64).round() as usize;
    let filled = filled.min(width);
    let empty = width - filled;
    format!("{}{}", "\u{2588}".repeat(filled), "\u{2591}".repeat(empty))
}

/// Builds a `Line` containing `bar + " PCT%"` styled by severity.
/// `width` controls the bar width in characters; `label_style` is the style
/// applied to the percentage label.
pub fn quota_gauge_line(remaining_pct: f64, width: usize) -> Line<'static> {
    let bar = block_bar(remaining_pct, width);
    let color = severity_color(Some(remaining_pct));
    let pct_str = format!("{:3.0}%", remaining_pct);
    let text_style = Style::default().fg(to_ratatui(ColorToken::Text));

    Line::from(vec![
        Span::styled(bar, Style::default().fg(color)),
        Span::styled(format!(" {pct_str}"), text_style),
    ])
}

/// Builds a `Line` for a window gauge row (label + bar + pct + reset arrow).
/// Used in detail view for primary/secondary windows.
pub fn window_gauge_line<'a>(
    label: &'a str,
    remaining_pct: f64,
    width: usize,
    reset_str: &'a str,
) -> Line<'a> {
    let bar = block_bar(remaining_pct, width);
    let color = severity_color(Some(remaining_pct));
    let pct_str = format!("{:3.0}%", remaining_pct);

    Line::from(vec![
        Span::styled(
            format!(" {:<4} ", label),
            Style::default().fg(to_ratatui(ColorToken::Muted)),
        ),
        Span::styled(bar, Style::default().fg(color)),
        Span::styled(
            format!("  {}  ", pct_str),
            Style::default()
                .fg(to_ratatui(ColorToken::TextBright))
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("\u{2192} {}", reset_str),
            Style::default().fg(to_ratatui(ColorToken::Comment)),
        ),
    ])
}

/// Builds a `Line` for a model mini-gauge row (name + bar + pct + cost).
pub fn model_gauge_line<'a>(
    name_trunc: &'a str,
    remaining_pct: f64,
    width: usize,
    cost_str: &'a str,
) -> Line<'a> {
    let bar = block_bar(remaining_pct, width);
    let color = severity_color(Some(remaining_pct));
    let pct_str = format!("{:3.0}%", remaining_pct);

    Line::from(vec![
        Span::styled(
            format!("   {:<8}  ", name_trunc),
            Style::default().fg(to_ratatui(ColorToken::Text)),
        ),
        Span::styled(bar, Style::default().fg(color)),
        Span::styled(
            format!("  {}", pct_str),
            Style::default().fg(to_ratatui(ColorToken::Muted)),
        ),
        Span::styled(
            cost_str.to_string(),
            Style::default().fg(to_ratatui(ColorToken::Comment)),
        ),
    ])
}
