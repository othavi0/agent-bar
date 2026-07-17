//! Seção 3: extra usage (só Claude — spend novo).

use ratatui::style::Style;
use ratatui::text::{Line, Span};

use crate::providers::extras::get_claude_extra;
use crate::providers::types::{ExtraUsage, ProviderQuota};
use crate::theme::ColorToken;
use crate::tui::theme_bridge::to_ratatui;
use crate::tui::widgets::quota_gauge::gauge_spans;
use crate::tui::widgets::severity::severity_color as sev_color;

use super::format::{derive_bar_width, EXTRA_SUFFIX_W, LABEL_W};

/// Linha "EXTRA USAGE": `enabled=false` → texto fixo "desativado";
/// `enabled=true && limit<=0.0` → sentinel de "sem limite configurado"
/// (`extra_usage_from_spend` em `claude.rs`) → só o valor gasto, SEM gauge
/// (não há teto p/ dar proporção — gauge 100%+"de $0.00" era
/// autocontraditório); `enabled=true && limit>0.0` → gauge + "$used de
/// $limit" (largura via `derive_bar_width`, Task 9 — MESMA coluna das
/// demais seções). Só Claude tem esta seção (Amp/Codex têm extras próprios,
/// sem overlap com `ClaudeQuotaExtra.extra_usage`); providers sem extra
/// omitem a linha inteira (`extra_lines` devolve vazio).
fn extra_usage_line(eu: &ExtraUsage, content_width: u16) -> Line<'static> {
    let label = Span::styled(
        format!(" {:<LABEL_W$} ", "EXTRA USAGE"),
        Style::default().fg(to_ratatui(ColorToken::Muted)),
    );
    if !eu.enabled {
        return Line::from(vec![
            label,
            Span::styled(
                "desativado",
                Style::default().fg(to_ratatui(ColorToken::Muted)),
            ),
        ]);
    }
    if eu.limit <= 0.0 {
        return Line::from(vec![
            label,
            Span::styled(
                format!("${:.2} usado", eu.used),
                Style::default().fg(to_ratatui(ColorToken::Yellow)),
            ),
            Span::styled(
                " \u{b7} sem limite",
                Style::default().fg(to_ratatui(ColorToken::Comment)),
            ),
        ]);
    }
    // `eu.limit > 0.0` é garantido pelo early-return acima (limit <= 0.0
    // já saiu com o texto "sem limite") — divisão segura.
    let pct_used = (eu.used / eu.limit * 100.0).clamp(0.0, 100.0);
    let remaining_pct = 100.0 - pct_used;
    let color = sev_color(Some(remaining_pct));
    let mut spans = vec![label];
    spans.extend(gauge_spans(
        remaining_pct,
        derive_bar_width(content_width, EXTRA_SUFFIX_W),
        color,
    ));
    spans.push(Span::styled(
        format!("  ${:.2} de ${:.2}", eu.used, eu.limit),
        Style::default().fg(to_ratatui(ColorToken::TextBright)),
    ));
    Line::from(spans)
}

/// Seção EXTRA USAGE completa: 0 linhas se o provider não tiver extra usage
/// (chamador colapsa a seção via `Constraint::Length(0)`), 1 linha se tiver.
pub(super) fn extra_lines(q: &ProviderQuota, content_width: u16) -> Vec<Line<'static>> {
    match get_claude_extra(q).and_then(|c| c.extra_usage.as_ref()) {
        Some(eu) => vec![extra_usage_line(eu, content_width)],
        None => Vec::new(),
    }
}
