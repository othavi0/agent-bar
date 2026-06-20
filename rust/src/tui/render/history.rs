//! Aba History: tendencia de tokens/custo por dia nos ultimos 7 dias.
//! `bucket_by_day` e uma funcao pura testavel — sem IO, sem relogio.
//! `render_history` consome `state.history` (Vec<UsageRecord>) e renderiza
//! sparklines + tabela de totais por provider.

use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Cell, Row, Table};
use ratatui::Frame;
use time::Date;

use crate::theme::ColorToken;
use crate::tui::state::AppState;
use crate::tui::theme_bridge::{provider_color, to_ratatui};
use crate::tui::widgets::sparkline::{sparkline_str, sparkline_str_wide};
use crate::usage::{pricing::cost_usd_of, UsageRecord};

// ---------------------------------------------------------------------------
// Bucketing (pure, testable)
// ---------------------------------------------------------------------------

/// Um bucket diario: date, soma de tokens (input+output), custo opcional em USD.
#[derive(Debug, Clone, PartialEq)]
pub struct DayBucket {
    pub date: Date,
    pub tokens: u64,
    pub cost_usd: Option<f64>,
}

/// Agrupa records por dia (ts.date()) somando tokens e custo.
/// Records com modelo desconhecido contribuem tokens mas nao custo (igual ao engine).
/// Retorna vec ordenado por data crescente.
pub fn bucket_by_day(records: &[UsageRecord]) -> Vec<DayBucket> {
    use std::collections::BTreeMap;

    let mut map: BTreeMap<Date, (u64, Option<f64>)> = BTreeMap::new();

    for rec in records {
        let date = rec.ts.date();
        let tokens = rec.input + rec.output;
        let cost = cost_usd_of(rec);

        let entry = map.entry(date).or_insert((0, None));
        entry.0 += tokens;
        match (cost, entry.1.as_mut()) {
            (Some(c), Some(acc)) => *acc += c,
            (Some(c), None) => entry.1 = Some(c),
            (None, _) => {}
        }
    }

    map.into_iter()
        .map(|(date, (tokens, cost_usd))| DayBucket {
            date,
            tokens,
            cost_usd,
        })
        .collect()
}

/// Agrupa records por (provider, day) para o grafico por-provider.
/// Retorna BTreeMap<provider_name, Vec<DayBucket>> ordenado por data.
pub fn bucket_by_provider_day(
    records: &[UsageRecord],
) -> std::collections::BTreeMap<String, Vec<DayBucket>> {
    use std::collections::BTreeMap;

    // (provider, date) -> (tokens, cost)
    let mut map: BTreeMap<(String, Date), (u64, Option<f64>)> = BTreeMap::new();

    for rec in records {
        let date = rec.ts.date();
        let tokens = rec.input + rec.output;
        let cost = cost_usd_of(rec);
        let key = (rec.provider.clone(), date);

        let entry = map.entry(key).or_insert((0, None));
        entry.0 += tokens;
        match (cost, entry.1.as_mut()) {
            (Some(c), Some(acc)) => *acc += c,
            (Some(c), None) => entry.1 = Some(c),
            (None, _) => {}
        }
    }

    // Reorganiza por provider
    let mut by_provider: BTreeMap<String, BTreeMap<Date, (u64, Option<f64>)>> = BTreeMap::new();
    for ((provider, date), (tokens, cost)) in map {
        by_provider
            .entry(provider)
            .or_default()
            .insert(date, (tokens, cost));
    }

    by_provider
        .into_iter()
        .map(|(provider, date_map)| {
            let buckets = date_map
                .into_iter()
                .map(|(date, (tokens, cost_usd))| DayBucket {
                    date,
                    tokens,
                    cost_usd,
                })
                .collect();
            (provider, buckets)
        })
        .collect()
}

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
pub fn render_history(state: &AppState, frame: &mut Frame, area: Rect) {
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
            let spark = sparkline_str(&token_data);
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
    use time::macros::date;

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

    // ---- bucket_by_day unit tests ----

    #[test]
    fn bucket_by_day_empty_input() {
        let result = bucket_by_day(&[]);
        assert!(result.is_empty());
    }

    #[test]
    fn bucket_by_day_single_record() {
        let records = vec![rec(
            "claude",
            Some("claude-sonnet-4-6"),
            "2026-06-17T10:00:00Z",
            1000,
            500,
        )];
        let buckets = bucket_by_day(&records);
        assert_eq!(buckets.len(), 1);
        assert_eq!(buckets[0].date, date!(2026 - 06 - 17));
        assert_eq!(buckets[0].tokens, 1500);
        assert!(buckets[0].cost_usd.is_some());
    }

    #[test]
    fn bucket_by_day_three_days_correct_sums() {
        let records = vec![
            // Dia 17: 2 records de claude-sonnet → somados
            rec(
                "claude",
                Some("claude-sonnet-4-6"),
                "2026-06-17T08:00:00Z",
                1000,
                200,
            ),
            rec(
                "claude",
                Some("claude-sonnet-4-6"),
                "2026-06-17T14:00:00Z",
                500,
                100,
            ),
            // Dia 18: 1 record de codex
            rec("codex", Some("gpt-5.5"), "2026-06-18T10:00:00Z", 2000, 300),
            // Dia 19: 1 record sem modelo (sem custo)
            rec("claude", None, "2026-06-19T09:00:00Z", 800, 200),
        ];

        let buckets = bucket_by_day(&records);

        // Deve ter 3 buckets (um por data unica)
        assert_eq!(
            buckets.len(),
            3,
            "esperado 3 buckets, obtido {}",
            buckets.len()
        );

        // Ordenados por data crescente
        assert_eq!(buckets[0].date, date!(2026 - 06 - 17));
        assert_eq!(buckets[1].date, date!(2026 - 06 - 18));
        assert_eq!(buckets[2].date, date!(2026 - 06 - 19));

        // Dia 17: tokens = (1000+200) + (500+100) = 1800
        assert_eq!(buckets[0].tokens, 1800, "dia 17 tokens incorretos");
        assert!(
            buckets[0].cost_usd.is_some(),
            "dia 17 deve ter custo (sonnet conhecido)"
        );

        // Dia 18: tokens = 2000+300 = 2300
        assert_eq!(buckets[1].tokens, 2300, "dia 18 tokens incorretos");
        assert!(
            buckets[1].cost_usd.is_some(),
            "dia 18 deve ter custo (gpt-5 conhecido)"
        );

        // Dia 19: tokens = 800+200 = 1000, custo = None (modelo None)
        assert_eq!(buckets[2].tokens, 1000, "dia 19 tokens incorretos");
        assert!(
            buckets[2].cost_usd.is_none(),
            "dia 19 nao deve ter custo (modelo None)"
        );
    }

    #[test]
    fn bucket_by_day_same_day_different_providers_merged() {
        // Records de providers diferentes no mesmo dia → somados no bucket total
        let records = vec![
            rec(
                "claude",
                Some("claude-sonnet-4-6"),
                "2026-06-17T08:00:00Z",
                1000,
                0,
            ),
            rec("codex", Some("gpt-5.5"), "2026-06-17T12:00:00Z", 500, 0),
        ];
        let buckets = bucket_by_day(&records);
        assert_eq!(buckets.len(), 1);
        assert_eq!(
            buckets[0].tokens, 1500,
            "tokens de providers diferentes devem somar"
        );
    }

    #[test]
    fn bucket_by_day_unknown_model_contributes_tokens_not_cost() {
        let records = vec![rec("claude", None, "2026-06-17T10:00:00Z", 1000, 200)];
        let buckets = bucket_by_day(&records);
        assert_eq!(buckets.len(), 1);
        assert_eq!(buckets[0].tokens, 1200);
        assert!(
            buckets[0].cost_usd.is_none(),
            "modelo None nao deve gerar custo"
        );
    }

    // ---- snapshot test ----

    #[test]
    fn history_snapshot_three_days() {
        let backend = ratatui::backend::TestBackend::new(64, 20);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();

        let mut state = AppState::new();
        state.tab = crate::tui::state::Tab::History;
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
            .draw(|f| render_history(&state, f, f.area()))
            .unwrap();

        insta::assert_snapshot!(terminal.backend());
    }

    #[test]
    fn history_snapshot_empty() {
        let backend = ratatui::backend::TestBackend::new(64, 20);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();

        let mut state = AppState::new();
        state.tab = crate::tui::state::Tab::History;
        state.history = Some(vec![]);

        terminal
            .draw(|f| render_history(&state, f, f.area()))
            .unwrap();

        insta::assert_snapshot!(terminal.backend());
    }

    #[test]
    fn history_snapshot_loading() {
        let backend = ratatui::backend::TestBackend::new(64, 20);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();

        let mut state = AppState::new();
        state.tab = crate::tui::state::Tab::History;
        // history = None simula ainda-nao-carregado

        terminal
            .draw(|f| render_history(&state, f, f.area()))
            .unwrap();

        insta::assert_snapshot!(terminal.backend());
    }

    #[test]
    fn history_renders_wide_160() {
        let backend = ratatui::backend::TestBackend::new(160, 40);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();

        let mut state = AppState::new();
        state.tab = crate::tui::state::Tab::History;
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
            .draw(|f| render_history(&state, f, f.area()))
            .unwrap();

        insta::assert_snapshot!(terminal.backend());
    }
}
