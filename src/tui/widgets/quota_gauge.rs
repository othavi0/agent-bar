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

/// Escala os canais RGB de `c` por `s` (não-negativo), clamp em 0..=255.
/// Usado pelo pulso crítico (T16) pra modular brilho sem trocar o matiz.
/// Cores não-RGB (raras neste app) passam intactas.
pub fn scale_rgb(c: Color, s: f64) -> Color {
    match c {
        Color::Rgb(r, g, b) => {
            let scale = |x: u8| -> u8 { (f64::from(x) * s).round().clamp(0.0, 255.0) as u8 };
            Color::Rgb(scale(r), scale(g), scale(b))
        }
        other => other,
    }
}

/// Pulso crítico (T16, determinístico — sem tachyonfx): oscila o brilho de
/// `base` entre 0.75x e 1.45x num ciclo de ~1.1s (37 ticks de `AnimTick`,
/// ~30ms cada). Chamado nos call sites de `gauge_spans` onde
/// `remaining < 10.0` e `state.animations` está ligado — coexiste com o
/// blink da sidebar (Task 10, `render/sidebar.rs::item_color`), que
/// continua intacto (efeito diferente, alvo diferente: sidebar pisca
/// texto/marca, isto pulsa o gauge do card/detalhe).
pub fn pulse_color(base: Color, anim_frame: u64) -> Color {
    let phase = (anim_frame % 37) as f64 / 37.0;
    let s = 0.75 + 0.70 * (phase * std::f64::consts::TAU).sin().abs();
    scale_rgb(base, s)
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

    // ---- Motion: pulse crítico (Task 16) ----

    #[test]
    fn scale_rgb_scales_channels_and_clamps() {
        let c = Color::Rgb(100, 100, 100);
        assert_eq!(scale_rgb(c, 1.0), Color::Rgb(100, 100, 100));
        assert_eq!(scale_rgb(c, 0.5), Color::Rgb(50, 50, 50));
        // Clamp em 255, não overflow/wrap de u8.
        assert_eq!(scale_rgb(c, 3.0), Color::Rgb(255, 255, 255));
    }

    #[test]
    fn scale_rgb_passes_through_non_rgb_colors() {
        assert_eq!(scale_rgb(Color::Reset, 0.5), Color::Reset);
    }

    #[test]
    fn pulse_color_oscillates_within_bounds() {
        let base = Color::Rgb(100, 100, 100);
        // A cada frame do ciclo de 37, o canal escalado deve ficar dentro
        // de [0.75x, 1.45x] (clamp de 255 à parte).
        for frame in 0..37u64 {
            if let Color::Rgb(r, _, _) = pulse_color(base, frame) {
                assert!(
                    (75..=145).contains(&r),
                    "frame {frame}: canal {r} fora de [75,145] (base 100 escalado 0.75x-1.45x)"
                );
            } else {
                panic!("pulse_color deveria preservar Color::Rgb");
            }
        }
    }

    #[test]
    fn pulse_color_is_deterministic_for_same_frame() {
        let base = Color::Rgb(200, 50, 10);
        assert_eq!(pulse_color(base, 12), pulse_color(base, 12));
        assert_eq!(pulse_color(base, 12), pulse_color(base, 12 + 37));
    }
}
