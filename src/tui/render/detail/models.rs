//! Seção 2: Modelos hoje (tokens + custo, de provider_usage.by_model).

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

use crate::theme::ColorToken;
use crate::tui::theme_bridge::to_ratatui;
use crate::tui::widgets::column_chart::fmt_tokens_short;
use crate::tui::widgets::quota_gauge::gauge_spans;
use crate::usage::model_names::display_model_name;
use crate::usage::{ModelUsage, ProviderUsage};

use super::format::{derive_bar_width, model_tokens, truncate_name, LABEL_W, MODEL_SUFFIX_W};

/// Uma linha de "MODELOS HOJE": label(12, nome TRATADO — `display_model_name`,
/// Task 9) + gauge PROPORCIONAL a tokens (não a 100% — normalizada pelo
/// modelo de maior consumo, MESMA largura das janelas via `derive_bar_width`)
/// + tokens abreviados + custo, ambos right-aligned.
fn model_usage_line(
    mu: &ModelUsage,
    max_tokens: u64,
    brand: Color,
    content_width: u16,
) -> Line<'static> {
    let tokens = model_tokens(mu);
    let pct = if max_tokens > 0 {
        (tokens as f64 / max_tokens as f64 * 100.0).clamp(0.0, 100.0)
    } else {
        0.0
    };
    let display_name = display_model_name(&mu.model);
    let name = truncate_name(&display_name, LABEL_W);
    let cost_str = match &mu.cost {
        Some(c) => format!("${:.2}", c.usd),
        None => "\u{2014}".to_string(),
    };
    let mut spans = vec![Span::styled(
        format!(" {name:<LABEL_W$} "),
        Style::default().fg(to_ratatui(ColorToken::Text)),
    )];
    spans.extend(gauge_spans(
        pct,
        derive_bar_width(content_width, MODEL_SUFFIX_W),
        brand,
    ));
    spans.push(Span::styled(
        format!(" {:>8}", fmt_tokens_short(tokens)),
        Style::default().fg(to_ratatui(ColorToken::Muted)),
    ));
    spans.push(Span::styled(
        format!(" {cost_str:>9}"),
        Style::default().fg(to_ratatui(ColorToken::Comment)),
    ));
    Line::from(spans)
}

/// Seção MODELOS HOJE completa (título + 1 linha por `pu.by_model`) e a
/// versão colapsada (1 linha-resumo, Task 9 §2 — usada quando a área não
/// cabe tudo + chart mínimo). Vazio (sem `provider_usage` ou sem modelos
/// hoje) → ambas vazias, a seção inteira desaparece do layout.
pub(super) fn model_lines(
    provider_usage: Option<&ProviderUsage>,
    brand: Color,
    content_width: u16,
) -> (Vec<Line<'static>>, Vec<Line<'static>>) {
    let Some(pu) = provider_usage else {
        return (Vec::new(), Vec::new());
    };
    if pu.by_model.is_empty() {
        return (Vec::new(), Vec::new());
    }
    let mut full = vec![
        Line::from(""),
        Line::from(Span::styled(
            " MODELOS HOJE",
            Style::default()
                .fg(to_ratatui(ColorToken::TextBright))
                .add_modifier(Modifier::BOLD),
        )),
    ];
    let max_tokens = pu
        .by_model
        .iter()
        .map(model_tokens)
        .max()
        .unwrap_or(0)
        .max(1);
    for mu in &pu.by_model {
        full.push(model_usage_line(mu, max_tokens, brand, content_width));
    }

    let total_cost: Option<f64> = pu.by_model.iter().fold(None, |acc, mu| match &mu.cost {
        Some(c) => Some(acc.unwrap_or(0.0) + c.usd),
        None => acc,
    });
    let cost_str = total_cost
        .map(|c| format!("${c:.2}"))
        .unwrap_or_else(|| "\u{2014}".to_string());
    let collapsed = vec![Line::from(Span::styled(
        format!(
            " {} modelos hoje \u{b7} {}  \u{2026}",
            pu.by_model.len(),
            cost_str
        ),
        Style::default().fg(to_ratatui(ColorToken::Comment)),
    ))];
    (full, collapsed)
}
