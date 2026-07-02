//! Gauge por célula: gradiente sutil no trecho preenchido + trilho.

use ratatui::style::{Color, Style};
use ratatui::text::Span;

use crate::theme::ColorToken;
use crate::tui::theme_bridge::to_ratatui;

/// Interpola linearmente entre duas cores RGB (t em 0.0..=1.0).
fn lerp_rgb(a: Color, b: Color, t: f64) -> Color {
    let ((ar, ag, ab), (br, bg, bb)) = match (a, b) {
        (Color::Rgb(r1, g1, b1), Color::Rgb(r2, g2, b2)) => ((r1, g1, b1), (r2, g2, b2)),
        _ => return b,
    };
    let mix = |x: u8, y: u8| -> u8 {
        (f64::from(x) + (f64::from(y) - f64::from(x)) * t).round() as u8
    };
    Color::Rgb(mix(ar, br), mix(ag, bg), mix(ab, bb))
}

/// Escurece a cor para o início do gradiente (60% da intensidade).
fn dimmed(c: Color) -> Color {
    lerp_rgb(Color::Rgb(0, 0, 0), c, 0.6)
}

/// Barra de quota: `remaining_pct` em 0..=100, `width` células.
/// Preenchido = `█` com gradiente dimmed→cor ao longo do trecho;
/// trilho = `▒` em EmptyTrack. Total de células == width, sempre.
pub fn gauge_spans(remaining_pct: f64, width: usize, color: Color) -> Vec<Span<'static>> {
    let pct = remaining_pct.clamp(0.0, 100.0);
    let filled = ((width as f64) * pct / 100.0).round() as usize;
    let mut spans = Vec::with_capacity(width.min(filled + 1));
    for i in 0..filled {
        let t = if filled <= 1 {
            1.0
        } else {
            i as f64 / (filled - 1) as f64
        };
        spans.push(Span::styled(
            "█".to_string(),
            Style::default().fg(lerp_rgb(dimmed(color), color, t)),
        ));
    }
    if width > filled {
        spans.push(Span::styled(
            "▒".repeat(width - filled),
            Style::default().fg(to_ratatui(ColorToken::EmptyTrack)),
        ));
    }
    spans
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gauge_spans_fill_and_track_add_up_to_width() {
        let spans = gauge_spans(50.0, 10, ratatui::style::Color::Green);
        let total: usize = spans.iter().map(|s| s.content.chars().count()).sum();
        assert_eq!(total, 10);
        let filled: usize = spans
            .iter()
            .filter(|s| s.content.contains('█'))
            .map(|s| s.content.chars().count())
            .sum();
        assert_eq!(filled, 5);
    }

    #[test]
    fn gauge_spans_zero_and_full() {
        let z = gauge_spans(0.0, 8, ratatui::style::Color::Red);
        assert!(z.iter().all(|s| !s.content.contains('█')));
        let f = gauge_spans(100.0, 8, ratatui::style::Color::Green);
        assert!(f.iter().all(|s| !s.content.contains('▒')));
    }
}
