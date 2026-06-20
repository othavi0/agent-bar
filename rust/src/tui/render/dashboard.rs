use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Cell, Row, Table};
use ratatui::Frame;

use crate::theme::ColorToken;
use crate::tui::state::AppState;
use crate::tui::theme_bridge::{provider_color, to_ratatui};

/// Builds a 7-char block bar string for remaining quota.
/// filled (█) = remaining; empty (░) = consumed.
/// 100% remaining → all filled; 0% remaining → all empty.
pub fn quota_bar_pub(remaining_pct: f64) -> String {
    quota_bar(remaining_pct)
}

fn quota_bar(remaining_pct: f64) -> String {
    let total = 7usize;
    let remaining = remaining_pct.clamp(0.0, 100.0);
    let filled = ((remaining / 100.0) * total as f64).round() as usize;
    let filled = filled.min(total);
    let empty = total - filled;
    format!("{}{}", "\u{2588}".repeat(filled), "\u{2591}".repeat(empty))
}

/// Selects a severity color based on remaining percentage.
fn severity_color(remaining_pct: f64) -> ratatui::style::Color {
    if remaining_pct >= 60.0 {
        to_ratatui(ColorToken::Green)
    } else if remaining_pct >= 30.0 {
        to_ratatui(ColorToken::Yellow)
    } else if remaining_pct >= 10.0 {
        to_ratatui(ColorToken::Orange)
    } else {
        to_ratatui(ColorToken::Red)
    }
}

/// Renders the Dashboard tab: a table of all providers with usage bars.
pub fn render_dashboard(state: &AppState, frame: &mut Frame, area: Rect) {
    let header_style = Style::default()
        .fg(to_ratatui(ColorToken::Muted))
        .add_modifier(Modifier::BOLD);
    let text_style = Style::default().fg(to_ratatui(ColorToken::Text));

    let header = Row::new(vec![
        Cell::from("provider").style(header_style),
        Cell::from("uso").style(header_style),
        Cell::from("reset").style(header_style),
        Cell::from("custo").style(header_style),
    ])
    .bottom_margin(0);

    let rows: Vec<Row<'_>> = state
        .providers
        .iter()
        .map(|pv| {
            let q = &pv.quota;
            let remaining = q.primary.as_ref().map(|w| w.remaining).unwrap_or(0.0);
            let bar = quota_bar(remaining);
            let pct_str = format!("{:3.0}%", remaining);
            let bar_color = severity_color(remaining);
            let p_color = provider_color(&q.provider);

            let reset_str = q
                .primary
                .as_ref()
                .and_then(|w| w.resets_at.as_ref())
                .map(|r| {
                    // Extract HH:MM from ISO timestamp if possible
                    r.split('T')
                        .nth(1)
                        .and_then(|t| t.get(..5))
                        .unwrap_or(r.as_str())
                        .to_string()
                })
                .unwrap_or_else(|| "-".to_string());

            let bar_cell = Cell::from(Line::from(vec![
                Span::styled(bar, Style::default().fg(bar_color)),
                Span::styled(format!(" {pct_str}"), text_style),
            ]));

            Row::new(vec![
                Cell::from(q.display_name.as_str())
                    .style(Style::default().fg(p_color).add_modifier(Modifier::BOLD)),
                bar_cell,
                Cell::from(reset_str).style(text_style),
                Cell::from("-").style(Style::default().fg(to_ratatui(ColorToken::Comment))),
            ])
        })
        .collect();

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Thick)
        .border_style(Style::default().fg(to_ratatui(ColorToken::Blue)))
        .title(Span::styled(
            " Todos os providers ",
            Style::default()
                .fg(to_ratatui(ColorToken::TextBright))
                .add_modifier(Modifier::BOLD),
        ));

    let widths = [
        Constraint::Length(9),
        Constraint::Length(14),
        Constraint::Length(6),
        Constraint::Min(6),
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .block(block)
        .column_spacing(1);

    frame.render_widget(table, area);
}
