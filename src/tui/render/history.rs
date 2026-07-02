//! Aba History: tendencia de tokens/custo por dia nos ultimos 7 dias.
//! `bucket_by_day` e uma funcao pura testavel — sem IO, sem relogio.
//! `render_history` consome `state.history` (Vec<UsageRecord>) e renderiza
//! sparklines + tabela de totais por provider.

use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Cell, Row, Table};
use ratatui::Frame;

use crate::theme::ColorToken;
use crate::tui::mouse::HitMap;
use crate::tui::state::AppState;
use crate::tui::theme_bridge::{provider_color, to_ratatui};
use crate::tui::widgets::sparkline::sparkline_str_wide;
use crate::usage::buckets::{bucket_by_day, bucket_by_provider_day, DayBucket};

// ---------------------------------------------------------------------------
// Render
// ---------------------------------------------------------------------------

/// Formata tokens em unidade legivel (K/M).
fn fmt_tokens(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.0}K", n as f64 / 1_000.0)
    } else {
        format!("{n}")
    }
}

/// Renderiza a aba History: sparklines + tabela de totais por provider.
///
/// `_hits`: repassado pelo dispatcher — usos reais nas Tasks 11-14.
pub fn render_history(state: &AppState, frame: &mut Frame, area: Rect, _hits: &mut HitMap) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Thick)
        .border_style(Style::default().fg(to_ratatui(ColorToken::Blue)))
        .title(Span::styled(
            " Historico (7 dias) ",
            Style::default()
                .fg(to_ratatui(ColorToken::TextBright))
                .add_modifier(Modifier::BOLD),
        ));

    let records = match &state.history {
        Some(r) => r,
        None => {
            // Sem dados: mostra placeholder enquanto carrega.
            let inner = block.inner(area);
            frame.render_widget(block, area);
            render_loading(frame, inner);
            return;
        }
    };

    if records.is_empty() {
        let inner = block.inner(area);
        frame.render_widget(block, area);
        render_empty(frame, inner);
        return;
    }

    let total_buckets = bucket_by_day(records);
    let provider_buckets = bucket_by_provider_day(records);

    // Total na janela (para rodape)
    let total_tokens: u64 = total_buckets.iter().map(|b| b.tokens).sum();
    let total_cost: Option<f64> = {
        let costs: Vec<f64> = total_buckets.iter().filter_map(|b| b.cost_usd).collect();
        if costs.is_empty() {
            None
        } else {
            Some(costs.iter().sum())
        }
    };

    let footer_str = match total_cost {
        Some(c) => format!(
            " Total 7d: {} tokens / ${:.2} ",
            fmt_tokens(total_tokens),
            c
        ),
        None => format!(" Total 7d: {} tokens ", fmt_tokens(total_tokens)),
    };

    let block = block.title_bottom(
        Line::from(Span::styled(
            footer_str,
            Style::default()
                .fg(to_ratatui(ColorToken::TextBright))
                .add_modifier(Modifier::BOLD),
        ))
        .alignment(Alignment::Right),
    );

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Layout vertical: [sparkline area, tabela]
    let n_providers = provider_buckets.len().max(1);
    let spark_height = (n_providers as u16 * 3)
        .min(inner.height.saturating_sub(6))
        .max(3);

    let vert = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(spark_height), Constraint::Min(0)])
        .split(inner);

    let spark_area = vert[0];
    let table_area = vert[1];

    render_sparklines(frame, spark_area, &provider_buckets);
    render_table(frame, table_area, &provider_buckets);
}

/// Renderiza sparklines empilhados por provider.
fn render_sparklines(
    frame: &mut Frame,
    area: Rect,
    provider_buckets: &std::collections::BTreeMap<String, Vec<DayBucket>>,
) {
    if area.height == 0 {
        return;
    }

    let providers: Vec<&str> = provider_buckets.keys().map(|s| s.as_str()).collect();
    let n = providers.len();
    if n == 0 {
        return;
    }

    // Divide verticalmente: 3 linhas por provider (label + spark + espaco).
    let constraints: Vec<Constraint> = (0..n).map(|_| Constraint::Length(2)).collect();
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);

    for (i, provider) in providers.iter().enumerate() {
        if i >= rows.len() {
            break;
        }
        let row_area = rows[i];
        let buckets = match provider_buckets.get(*provider) {
            Some(b) => b,
            None => continue,
        };

        let token_data: Vec<u64> = buckets.iter().map(|b| b.tokens).collect();
        // Fill available width: label is 9 chars, remainder goes to sparkline.
        let spark_width = (row_area.width as usize)
            .saturating_sub(9)
            .max(token_data.len().max(1));
        let spark = sparkline_str_wide(&token_data, spark_width);

        let p_color = provider_color(provider);
        let label = format!("{:<8}", provider);

        let line = Line::from(vec![
            Span::styled(
                label,
                Style::default().fg(p_color).add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
            Span::styled(spark, Style::default().fg(p_color)),
        ]);

        frame.render_widget(ratatui::widgets::Paragraph::new(line), row_area);
    }
}

/// Renderiza tabela com totais por provider.
fn render_table(
    frame: &mut Frame,
    area: Rect,
    provider_buckets: &std::collections::BTreeMap<String, Vec<DayBucket>>,
) {
    if area.height < 3 {
        return;
    }

    let header_style = Style::default()
        .fg(to_ratatui(ColorToken::Muted))
        .add_modifier(Modifier::BOLD);
    let muted_style = Style::default().fg(to_ratatui(ColorToken::Comment));

    let header = Row::new(vec![
        Cell::from("provider").style(header_style),
        Cell::from("tokens (7d)").style(header_style),
        Cell::from("custo (7d)").style(header_style),
        Cell::from("tendencia").style(header_style),
    ]);

    // Fixed columns: provider(9) + tokens(12) + cost(11) + spacing(3) = 35.
    // The "tendencia" column (Fill(1)) takes the remainder.
    let spark_col_width = (area.width as usize).saturating_sub(35).max(7);

    let rows: Vec<Row<'_>> = provider_buckets
        .iter()
        .map(|(provider, buckets)| {
            let total_tokens: u64 = buckets.iter().map(|b| b.tokens).sum();
            let cost_str = {
                let costs: Vec<f64> = buckets.iter().filter_map(|b| b.cost_usd).collect();
                if costs.is_empty() {
                    "-".to_string()
                } else {
                    format!("${:.2}", costs.iter().sum::<f64>())
                }
            };

            let token_data: Vec<u64> = buckets.iter().map(|b| b.tokens).collect();
            let spark = sparkline_str_wide(&token_data, spark_col_width);
            let p_color = provider_color(provider);

            Row::new(vec![
                Cell::from(provider.as_str())
                    .style(Style::default().fg(p_color).add_modifier(Modifier::BOLD)),
                Cell::from(fmt_tokens(total_tokens)).style(muted_style),
                Cell::from(cost_str).style(muted_style),
                Cell::from(spark).style(Style::default().fg(p_color)),
            ])
        })
        .collect();

    let widths = [
        Constraint::Length(9),
        Constraint::Length(12),
        Constraint::Length(11),
        Constraint::Fill(1),
    ];

    let table = Table::new(rows, widths).header(header).column_spacing(1);
    frame.render_widget(table, area);
}

/// Placeholder enquanto history ainda nao foi carregada.
fn render_loading(frame: &mut Frame, area: Rect) {
    use ratatui::widgets::Paragraph;
    let p = Paragraph::new(Span::styled(
        " Carregando historico...",
        Style::default().fg(to_ratatui(ColorToken::Muted)),
    ));
    frame.render_widget(p, area);
}

/// Placeholder quando nao ha records na janela.
fn render_empty(frame: &mut Frame, area: Rect) {
    use ratatui::widgets::Paragraph;
    let p = Paragraph::new(Span::styled(
        " Sem registros nos ultimos 7 dias.",
        Style::default().fg(to_ratatui(ColorToken::Muted)),
    ));
    frame.render_widget(p, area);
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
pub mod tests {
    use super::*;

    use crate::tui::state::AppState;
    use crate::usage::UsageRecord;

    fn rec(
        provider: &str,
        model: Option<&str>,
        ts_str: &str,
        input: u64,
        output: u64,
    ) -> UsageRecord {
        // Parseia ISO timestamp simples: "2026-06-17T10:00:00Z"
        let ts =
            time::OffsetDateTime::parse(ts_str, &time::format_description::well_known::Rfc3339)
                .expect("timestamp invalido");
        UsageRecord {
            provider: provider.to_string(),
            model: model.map(|s| s.to_string()),
            input,
            output,
            cache_read: 0,
            cache_write: 0,
            ts,
        }
    }

    // ---- snapshot test ----

    #[test]
    fn history_snapshot_three_days() {
        let backend = ratatui::backend::TestBackend::new(64, 20);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();

        let mut state = AppState::new();
        state.screen = crate::tui::state::Screen::History;
        state.history = Some(vec![
            rec(
                "claude",
                Some("claude-sonnet-4-6"),
                "2026-06-17T08:00:00Z",
                500_000,
                100_000,
            ),
            rec(
                "claude",
                Some("claude-sonnet-4-6"),
                "2026-06-18T09:00:00Z",
                300_000,
                80_000,
            ),
            rec(
                "codex",
                Some("gpt-5.5"),
                "2026-06-18T10:00:00Z",
                200_000,
                50_000,
            ),
            rec(
                "claude",
                Some("claude-opus-4-8"),
                "2026-06-19T11:00:00Z",
                1_000_000,
                200_000,
            ),
            rec(
                "codex",
                Some("gpt-5.5"),
                "2026-06-19T14:00:00Z",
                400_000,
                100_000,
            ),
        ]);

        terminal
            .draw(|f| render_history(&state, f, f.area(), &mut HitMap::default()))
            .unwrap();

        insta::assert_snapshot!(terminal.backend());
    }

    #[test]
    fn history_snapshot_empty() {
        let backend = ratatui::backend::TestBackend::new(64, 20);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();

        let mut state = AppState::new();
        state.screen = crate::tui::state::Screen::History;
        state.history = Some(vec![]);

        terminal
            .draw(|f| render_history(&state, f, f.area(), &mut HitMap::default()))
            .unwrap();

        insta::assert_snapshot!(terminal.backend());
    }

    #[test]
    fn history_snapshot_loading() {
        let backend = ratatui::backend::TestBackend::new(64, 20);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();

        let mut state = AppState::new();
        state.screen = crate::tui::state::Screen::History;
        // history = None simula ainda-nao-carregado

        terminal
            .draw(|f| render_history(&state, f, f.area(), &mut HitMap::default()))
            .unwrap();

        insta::assert_snapshot!(terminal.backend());
    }

    #[test]
    fn history_renders_wide_160() {
        let backend = ratatui::backend::TestBackend::new(160, 40);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();

        let mut state = AppState::new();
        state.screen = crate::tui::state::Screen::History;
        state.history = Some(vec![
            rec(
                "claude",
                Some("claude-sonnet-4-6"),
                "2026-06-17T08:00:00Z",
                500_000,
                100_000,
            ),
            rec(
                "claude",
                Some("claude-sonnet-4-6"),
                "2026-06-18T09:00:00Z",
                300_000,
                80_000,
            ),
            rec(
                "codex",
                Some("gpt-5.5"),
                "2026-06-18T10:00:00Z",
                200_000,
                50_000,
            ),
            rec(
                "claude",
                Some("claude-opus-4-8"),
                "2026-06-19T11:00:00Z",
                1_000_000,
                200_000,
            ),
            rec(
                "codex",
                Some("gpt-5.5"),
                "2026-06-19T14:00:00Z",
                400_000,
                100_000,
            ),
        ]);

        terminal
            .draw(|f| render_history(&state, f, f.area(), &mut HitMap::default()))
            .unwrap();

        insta::assert_snapshot!(terminal.backend());
    }
}
