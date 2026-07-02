use ratatui::layout::{Alignment, Constraint, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Cell, Row, Table};
use ratatui::Frame;

use crate::theme::ColorToken;
use crate::tui::state::AppState;
use crate::tui::theme_bridge::{provider_color, to_ratatui};
use crate::tui::widgets::quota_gauge::gauge_spans;
use crate::tui::widgets::severity::severity_color as sev_color;
use crate::usage::ProviderUsage;

/// Builds a 7-char gauge string (chars only, no color) for remaining quota.
/// Delegates to `quota_gauge::gauge_spans` with width=7.
/// Public for tests in render/mod.rs.
pub fn quota_bar_pub(remaining_pct: f64) -> String {
    gauge_spans(remaining_pct, 7, to_ratatui(ColorToken::Green))
        .iter()
        .map(|s| s.content.as_ref())
        .collect()
}

/// Derives bar width from available area width.
/// Fixed columns: provider(9) + reset(6) + cost(8) + column_spacing*3(3) + borders(2) + pct_label(5) = 33
/// At least 7 chars for the bar itself.
fn derive_bar_width(area_width: u16) -> usize {
    (area_width as usize).saturating_sub(33).max(7)
}

/// Formata o custo de um provider para a coluna custo do dashboard.
/// - Provider com cost Some(c) → ".XX"
/// - Provider Amp com amp_dollars → "cr .XX" (credito restante)
/// - Sem custo conhecido → "-"
fn fmt_provider_cost(pu: &ProviderUsage) -> String {
    // Amp: mostra saldo de credito
    if pu.provider == "amp" {
        if let Some(ref ad) = pu.amp_dollars {
            if let Some(rem) = ad.remaining {
                return format!("cr ${:.2}", rem);
            }
        }
        return "-".to_string();
    }
    // Outros providers: custo em USD
    match &pu.cost {
        Some(c) => format!("${:.2}", c.usd),
        None => "-".to_string(),
    }
}

/// Renders the Dashboard tab: a table of all providers with usage bars.
pub fn render_dashboard(state: &AppState, frame: &mut Frame, area: Rect) {
    let header_style = Style::default()
        .fg(to_ratatui(ColorToken::Muted))
        .add_modifier(Modifier::BOLD);
    let text_style = Style::default().fg(to_ratatui(ColorToken::Text));
    let muted_style = Style::default().fg(to_ratatui(ColorToken::Comment));

    let bar_width = derive_bar_width(area.width);

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
            // Animacao A (gauge lerp): usa display_ratio (animado) para a barra,
            // mas mostra o percentual bruto (remaining) no texto - so a barra desliza.
            let bar_pct = pv.display_ratio * 100.0;
            let pct_str = format!("{:3.0}%", remaining);
            let bar_color = sev_color(Some(remaining));
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

            let mut bar_spans = gauge_spans(bar_pct, bar_width, bar_color);
            bar_spans.push(Span::styled(format!(" {pct_str}"), text_style));
            let bar_cell = Cell::from(Line::from(bar_spans));

            // Custo: busca no UsageSummary pelo provider id
            let cost_str = state
                .usage
                .as_ref()
                .and_then(|s| s.providers.iter().find(|pu| pu.provider == q.provider))
                .map(fmt_provider_cost)
                .unwrap_or_else(|| "-".to_string());

            let cost_cell = Cell::from(cost_str).style(muted_style);

            Row::new(vec![
                Cell::from(q.display_name.as_str())
                    .style(Style::default().fg(p_color).add_modifier(Modifier::BOLD)),
                bar_cell,
                Cell::from(reset_str).style(text_style),
                cost_cell,
            ])
        })
        .collect();

    // Bloco com titulo no topo e, se houver custo, total no rodape.
    let mut block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Thick)
        .border_style(Style::default().fg(to_ratatui(ColorToken::Blue)))
        .title(Span::styled(
            " Todos os providers ",
            Style::default()
                .fg(to_ratatui(ColorToken::TextBright))
                .add_modifier(Modifier::BOLD),
        ));

    if let Some(s) = &state.usage {
        let total_str = format!(" Total hoje ~${:.2} ", s.total_cost.usd);
        block = block.title_bottom(
            Line::from(Span::styled(
                total_str,
                Style::default()
                    .fg(to_ratatui(ColorToken::TextBright))
                    .add_modifier(Modifier::BOLD),
            ))
            .alignment(Alignment::Right),
        );
    }

    let all_rows: Vec<Row<'_>> = rows;

    // bar column = Fill(1) so it expands with the terminal; other columns are fixed.
    let widths = [
        Constraint::Length(9),
        Constraint::Fill(1),
        Constraint::Length(6),
        Constraint::Min(6),
    ];

    let table = Table::new(all_rows, widths)
        .header(header)
        .block(block)
        .column_spacing(1);

    frame.render_widget(table, area);
}
