//! Modelo intermediário de render: linhas de segments coloridos.

use std::borrow::Cow;

use crate::config::{status_for_percent, HealthStatus};
use crate::settings::DisplayMode;
use crate::theme::ColorToken;

use super::shared::to_health;

#[derive(Debug, Clone, PartialEq)]
pub struct Segment {
    pub text: Cow<'static, str>,
    pub color: ColorToken,
    pub bold: bool,
    /// Texto verbatim: sem span/ANSI/escape. Usado p/ conectores (espaços, separadores).
    pub raw: bool,
}

impl Segment {
    pub fn new(text: impl Into<Cow<'static, str>>, color: ColorToken) -> Self {
        Self {
            text: text.into(),
            color,
            bold: false,
            raw: false,
        }
    }

    /// Segment `raw` (verbatim). A cor é estruturalmente exigida mas ignorada no render.
    pub fn raw_text(text: impl Into<Cow<'static, str>>) -> Self {
        Self {
            text: text.into(),
            color: ColorToken::Text,
            bold: false,
            raw: true,
        }
    }

    pub fn bold(mut self) -> Self {
        self.bold = true;
        self
    }
}

pub type Line = Vec<Segment>;

fn status_to_color(s: HealthStatus) -> ColorToken {
    match s {
        HealthStatus::Ok => ColorToken::Green,
        HealthStatus::Low => ColorToken::Yellow,
        HealthStatus::Warn => ColorToken::Orange,
        HealthStatus::Critical => ColorToken::Red,
    }
}

pub fn color_for_display(display: Option<f64>, mode: DisplayMode) -> ColorToken {
    match to_health(display, mode) {
        None => ColorToken::Text,
        Some(h) => status_to_color(status_for_percent(Some(h))),
    }
}

/// Barra de quota de 20 chars. Vazia (comment) quando `display` é None.
pub fn bar_segments(display: Option<f64>, mode: DisplayMode) -> Line {
    match display {
        None => vec![Segment::new("░".repeat(20), ColorToken::Comment)],
        Some(d) => {
            let filled = ((d / 5.0).floor().max(0.0) as usize).min(20);
            vec![
                Segment::new("█".repeat(filled), color_for_display(display, mode)),
                Segment::new("░".repeat(20 - filled), ColorToken::Comment),
            ]
        }
    }
}

/// Indicador de ponto único. Ponto aberto (comment) quando `display` é None.
pub fn indicator_segments(display: Option<f64>, mode: DisplayMode) -> Line {
    match display {
        None => vec![Segment::new("○", ColorToken::Comment)],
        Some(_) => vec![Segment::new("●", color_for_display(display, mode))],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::DisplayMode;
    use crate::theme::ColorToken;

    #[test]
    fn bar_is_always_20_wide() {
        let segs = bar_segments(Some(60.0), DisplayMode::Remaining);
        let total: usize = segs.iter().map(|s| s.text.chars().count()).sum();
        assert_eq!(total, 20);
        // 60/5 = 12 filled
        assert_eq!(segs[0].text.chars().count(), 12);
        assert_eq!(segs[1].text.chars().count(), 8);
    }

    #[test]
    fn bar_null_is_all_empty_comment() {
        let segs = bar_segments(None, DisplayMode::Remaining);
        assert_eq!(segs.len(), 1);
        assert_eq!(segs[0].text.chars().count(), 20);
        assert_eq!(segs[0].color, ColorToken::Comment);
    }

    #[test]
    fn bar_clamps_overage_to_20() {
        let segs = bar_segments(Some(150.0), DisplayMode::Remaining);
        let total: usize = segs.iter().map(|s| s.text.chars().count()).sum();
        assert_eq!(total, 20);
    }

    #[test]
    fn color_for_display_thresholds() {
        assert_eq!(
            color_for_display(Some(75.0), DisplayMode::Remaining),
            ColorToken::Green
        );
        assert_eq!(
            color_for_display(Some(20.0), DisplayMode::Remaining),
            ColorToken::Orange
        );
        assert_eq!(
            color_for_display(Some(5.0), DisplayMode::Remaining),
            ColorToken::Red
        );
        assert_eq!(
            color_for_display(None, DisplayMode::Remaining),
            ColorToken::Text
        );
    }

    #[test]
    fn color_for_display_used_mode_inverts_via_health() {
        // 30% usado → 70% restante → Ok/Green. Exercita to_health dentro de color_for_display.
        assert_eq!(
            color_for_display(Some(30.0), DisplayMode::Used),
            ColorToken::Green
        );
        // 95% usado → 5% restante → Critical/Red.
        assert_eq!(
            color_for_display(Some(95.0), DisplayMode::Used),
            ColorToken::Red
        );
        // bar_segments em used mode pega a cor pela saúde invertida.
        assert_eq!(
            bar_segments(Some(30.0), DisplayMode::Used)[0].color,
            ColorToken::Green
        );
    }

    #[test]
    fn indicator_open_dot_when_null() {
        assert_eq!(
            indicator_segments(None, DisplayMode::Remaining)[0].text,
            "○"
        );
        assert_eq!(
            indicator_segments(Some(80.0), DisplayMode::Remaining)[0].text,
            "●"
        );
    }

    #[test]
    fn raw_segment_constructor() {
        let s = Segment::raw_text(" │ ");
        assert!(s.raw);
        assert_eq!(s.text, " │ ");
    }
}
