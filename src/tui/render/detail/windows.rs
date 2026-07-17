//! Seção 1: Janelas (sessão/semana/modelos).

use ratatui::style::Style;
use ratatui::text::{Line, Span};

use crate::providers::types::{ProviderQuota, QuotaWindow};
use crate::theme::ColorToken;
use crate::tui::theme_bridge::to_ratatui;
use crate::tui::widgets::quota_gauge::gauge_spans;
use crate::tui::widgets::severity::severity_color_api;
use crate::usage::ProviderUsage;

use super::format::{
    derive_bar_width, find_model_usage, fmt_reset, truncate_name, LABEL_W, WINDOW_SUFFIX_W,
};

/// Uma linha de janela (sessão/semana/modelo): label(12) + gauge + pct(4) +
/// reset. Cor vem da severidade da API quando presente (spec §4.1),
/// fallback pro threshold local — sem modulação de brilho (v8: gauge sólido,
/// pulso removido de propósito, spec §6).
fn window_line(label: &str, w: &QuotaWindow, gauge_w: usize) -> Line<'static> {
    let color = severity_color_api(w.severity.as_deref(), Some(w.remaining));
    let reset_str = fmt_reset(w.resets_at.as_deref());
    let name = truncate_name(label, LABEL_W);
    let mut spans = vec![Span::styled(
        format!(" {name:<LABEL_W$} "),
        Style::default().fg(to_ratatui(ColorToken::Muted)),
    )];
    spans.extend(gauge_spans(w.remaining, gauge_w, color));
    spans.push(Span::styled(
        format!(" {:>4.0}%", w.remaining),
        Style::default().fg(to_ratatui(ColorToken::TextBright)),
    ));
    spans.push(Span::styled(
        format!("  \u{2192} {reset_str}"),
        Style::default().fg(to_ratatui(ColorToken::Comment)),
    ));
    Line::from(spans)
}

/// Linha de modelo (q.models): igual a `window_line`, com custo do dia
/// anexado quando `find_model_usage` acha o modelo correspondente em
/// `provider_usage.by_model` — mas SÓ se couber em `content_width` (nunca
/// estoura a borda; opcional > alinhamento). Modelos de sessão semanal do
/// Claude (ex. "Opus") batem por substring com o id completo do usage
/// engine (ex. "claude-opus-4-8"), então isto é o caminho comum, não raro.
fn model_window_line(
    name: &str,
    w: &QuotaWindow,
    gauge_w: usize,
    content_width: u16,
    provider_usage: Option<&ProviderUsage>,
) -> Line<'static> {
    let mut line = window_line(name, w, gauge_w);
    if let Some(cost) = provider_usage
        .and_then(|pu| find_model_usage(&pu.by_model, name))
        .and_then(|mu| mu.cost.as_ref())
    {
        let cost_span = format!("  ${:.2}", cost.usd);
        let current_w: usize = line.spans.iter().map(|s| s.content.chars().count()).sum();
        if current_w + cost_span.chars().count() <= content_width as usize {
            line.spans.push(Span::styled(
                cost_span,
                Style::default().fg(to_ratatui(ColorToken::Comment)),
            ));
        }
    }
    line
}

/// Seção JANELAS completa: sessão + semana + 1 linha por `q.models` (nome
/// real da API). MESMO `bar_width` em todas — a coluna de gauge tem que
/// alinhar entre sessão/semana/modelos (contrato do brief) — e a mesma
/// `derive_bar_width` alimenta as seções 2 e 4 (Task 9), então o gauge
/// COMEÇA na mesma coluna em toda seção. A LARGURA (e portanto a coluna
/// onde termina) varia entre seções, porque cada uma tem seu próprio
/// `suffix_w` (`WINDOW_SUFFIX_W` vs `MODEL_SUFFIX_W` vs `EXTRA_SUFFIX_W`) —
/// isso é deliberado, não um bug de alinhamento (ver comentário acima de
/// `WINDOW_SUFFIX_W`).
pub(super) fn window_lines(
    q: &ProviderQuota,
    provider_usage: Option<&ProviderUsage>,
    content_width: u16,
) -> Vec<Line<'static>> {
    let bar_width = derive_bar_width(content_width, WINDOW_SUFFIX_W);
    let mut lines = Vec::new();
    if let Some(primary) = &q.primary {
        lines.push(window_line("sessão", primary, bar_width));
    }
    if let Some(secondary) = &q.secondary {
        lines.push(window_line("semana", secondary, bar_width));
    }
    if let Some(models) = &q.models {
        for (name, w) in models {
            lines.push(model_window_line(
                name,
                w,
                bar_width,
                content_width,
                provider_usage,
            ));
        }
    }
    lines
}
