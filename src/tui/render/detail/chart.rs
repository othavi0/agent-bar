//! Seção 2b: chart de tokens/hora por modelo.

use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::providers::types::ProviderQuota;
use crate::theme::ColorToken;
use crate::tui::state::AppState;
use crate::tui::theme_bridge::to_ratatui;
use crate::tui::widgets::column_chart::column_chart_lines;
use crate::usage::buckets::bucket_by_model_hour;

/// Título de seção: `left` em negrito (Comment) alinhado à esquerda, `right`
/// alinhado à direita (também Comment) — reaproveitado por qualquer seção
/// que precise de um cabeçalho (hoje só o chart usa `right`).
fn section_title(left: &str, right: &str, width: u16) -> Line<'static> {
    let left_text = format!(" {left}");
    let left_span = Span::styled(
        left_text.clone(),
        Style::default()
            .fg(to_ratatui(ColorToken::Comment))
            .add_modifier(Modifier::BOLD),
    );
    if right.is_empty() {
        return Line::from(left_span);
    }
    let right_text = format!("{right} ");
    let pad =
        (width as usize).saturating_sub(left_text.chars().count() + right_text.chars().count());
    Line::from(vec![
        left_span,
        Span::raw(" ".repeat(pad)),
        Span::styled(
            right_text,
            Style::default().fg(to_ratatui(ColorToken::Comment)),
        ),
    ])
}

/// Seção do chart: título + `column_chart_lines` (largura/altura ganhas do
/// `Min(9)` do orquestrador — Task 9 §2). `now` vem de `state.last_update`
/// (NUNCA `OffsetDateTime::now_utc()` — render precisa ser puro/
/// determinístico p/ snapshot); `None` (boot, sem fetch ainda) → registros
/// vazios, o chart desenha o próprio estado vazio (`column_chart_lines` já
/// cobre isso) em vez de inventar uma âncora temporal fake.
pub(super) fn render_chart_section(
    state: &AppState,
    frame: &mut Frame,
    area: Rect,
    q: &ProviderQuota,
) {
    let mut lines = vec![section_title(
        "TOKENS/HORA \u{b7} 24H",
        "escala \u{221a}",
        area.width,
    )];
    // `history=None` = parse dos session logs em voo — desenhar o estado
    // vazio do chart aqui seria afirmar zero sobre dado que só não chegou
    // ainda (regressão "hoje 0 tok"; mesma guarda que a extinta
    // `spark_line` já fazia). `last_update=None` (boot, sem fetch algum) É
    // o caso coberto pela regra do brief — aí o chart recebe série vazia
    // (via `now` sentinel abaixo) e desenha o próprio estado vazio.
    if state.history.is_none() {
        lines.push(Line::from(Span::styled(
            " coletando hist\u{f3}rico\u{2026}",
            Style::default().fg(to_ratatui(ColorToken::Muted)),
        )));
        lines.resize(area.height.max(1) as usize, Line::default());
        frame.render_widget(Paragraph::new(lines), area);
        return;
    }
    let now = state
        .last_update
        .unwrap_or(time::OffsetDateTime::UNIX_EPOCH);
    let records = state.history.as_deref().unwrap_or(&[]);
    let series = bucket_by_model_hour(records, &q.provider, now, 24);
    lines.extend(column_chart_lines(
        &series,
        area.width,
        area.height.saturating_sub(1),
        now,
        state.local_offset,
    ));
    frame.render_widget(Paragraph::new(lines), area);
}
