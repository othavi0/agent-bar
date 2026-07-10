//! Gauge sólido: fill na cor plena com precisão de ⅛ de célula + trilho.
//! (v8: gradiente lerp e pulso de brilho removidos de propósito — spec §6.)

use ratatui::style::{Color, Style};
use ratatui::text::Span;

use crate::theme::ColorToken;
use crate::tui::theme_bridge::to_ratatui;

/// Oitavos de célula, do vazio (índice 0 = nada) ao quase-cheio (7 = ▉).
const EIGHTHS: [&str; 8] = ["", "▏", "▎", "▍", "▌", "▋", "▊", "▉"];

/// Barra de quota: `remaining_pct` em 0..=100, `width` células.
/// Fill = `█` sólido + célula parcial de ⅛; trilho = `░` em EmptyTrack.
/// Total de células == width, sempre.
pub fn gauge_spans(remaining_pct: f64, width: usize, color: Color) -> Vec<Span<'static>> {
    let pct = remaining_pct.clamp(0.0, 100.0);
    let eighths = ((width as f64) * pct / 100.0 * 8.0).round() as usize;
    let full = (eighths / 8).min(width);
    let partial = if full < width { eighths % 8 } else { 0 };

    let mut spans = Vec::with_capacity(3);
    let mut used = 0;
    if full > 0 {
        spans.push(Span::styled("█".repeat(full), Style::default().fg(color)));
        used += full;
    }
    if partial > 0 && used < width {
        spans.push(Span::styled(
            EIGHTHS[partial].to_string(),
            Style::default().fg(color),
        ));
        used += 1;
    }
    if used < width {
        spans.push(Span::styled(
            "░".repeat(width - used),
            Style::default().fg(to_ratatui(ColorToken::EmptyTrack)),
        ));
    }
    spans
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(spans: &[Span<'_>]) -> String {
        spans.iter().map(|s| s.content.as_ref()).collect()
    }

    #[test]
    fn gauge_total_width_is_exact() {
        let spans = gauge_spans(50.0, 10, ratatui::style::Color::Green);
        let total: usize = spans.iter().map(|s| s.content.chars().count()).sum();
        assert_eq!(total, 10);
    }

    #[test]
    fn gauge_uses_single_solid_color_no_gradient() {
        let spans = gauge_spans(80.0, 10, ratatui::style::Color::Green);
        let fills: Vec<_> = spans
            .iter()
            .filter(|s| s.content.contains('█') || "▏▎▍▌▋▊▉".chars().any(|c| s.content.contains(c)))
            .collect();
        // Fill inteiro em NO MÁXIMO 2 spans (cheio + parcial), mesma cor.
        assert!(
            fills.len() <= 2,
            "fill deve ser sólido, não célula-a-célula: {} spans",
            fills.len()
        );
        let colors: std::collections::HashSet<_> =
            fills.iter().map(|s| format!("{:?}", s.style.fg)).collect();
        assert_eq!(colors.len(), 1);
    }

    #[test]
    fn gauge_has_eighth_precision() {
        // 64% de 24 células = 15.36 células → 15 cheias + parcial de 3/8 (▍).
        let spans = gauge_spans(64.0, 24, ratatui::style::Color::Green);
        let t = text(&spans);
        assert_eq!(t.chars().filter(|c| *c == '█').count(), 15);
        assert!(t.contains('▍'), "esperava célula parcial ▍ em: {t}");
    }

    #[test]
    fn gauge_zero_and_full() {
        let z = gauge_spans(0.0, 8, ratatui::style::Color::Red);
        assert_eq!(text(&z), "░".repeat(8));
        let f = gauge_spans(100.0, 8, ratatui::style::Color::Green);
        assert_eq!(text(&f), "█".repeat(8));
    }
}
