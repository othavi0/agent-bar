//! Severity color mapping: percent remaining → ratatui Color via theme_bridge.

use crate::config::{status_for_percent, HealthStatus};
use crate::tui::theme_bridge::to_ratatui;
use ratatui::style::Color;

/// Maps an optional remaining-percent to a ratatui Color via the canonical
/// `status_for_percent` thresholds (≥60 green / 30-59 yellow / 10-29 orange / <10 red).
/// `None` → Green (unknown = treat as healthy).
pub fn severity_color(pct: Option<f64>) -> Color {
    let status = status_for_percent(pct);
    use crate::theme::ColorToken;
    let token = match status {
        HealthStatus::Ok => ColorToken::Green,
        HealthStatus::Low => ColorToken::Yellow,
        HealthStatus::Warn => ColorToken::Orange,
        HealthStatus::Critical => ColorToken::Red,
    };
    to_ratatui(token)
}

/// Severidade com precedência da API (spec §4.1): valores conhecidos da API
/// vencem o threshold local; desconhecido/ausente cai no cálculo local.
pub fn severity_color_api(api: Option<&str>, remaining_pct: Option<f64>) -> Color {
    use crate::theme::ColorToken;
    match api.map(str::to_ascii_lowercase).as_deref() {
        Some("normal") | Some("ok") => to_ratatui(ColorToken::Green),
        Some("warning") | Some("elevated") | Some("high") => to_ratatui(ColorToken::Yellow),
        Some("critical") | Some("exceeded") | Some("blocked") => to_ratatui(ColorToken::Red),
        _ => severity_color(remaining_pct),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::ColorToken;
    use crate::tui::theme_bridge::to_ratatui;

    fn color(token: ColorToken) -> Color {
        to_ratatui(token)
    }

    #[test]
    fn severity_color_none_is_green() {
        assert_eq!(severity_color(None), color(ColorToken::Green));
    }

    #[test]
    fn severity_color_green_at_60_and_above() {
        assert_eq!(severity_color(Some(60.0)), color(ColorToken::Green));
        assert_eq!(severity_color(Some(100.0)), color(ColorToken::Green));
        assert_eq!(severity_color(Some(75.0)), color(ColorToken::Green));
    }

    #[test]
    fn severity_color_yellow_30_to_59() {
        assert_eq!(severity_color(Some(30.0)), color(ColorToken::Yellow));
        assert_eq!(severity_color(Some(45.0)), color(ColorToken::Yellow));
        assert_eq!(severity_color(Some(59.9)), color(ColorToken::Yellow));
    }

    #[test]
    fn severity_color_orange_10_to_29() {
        assert_eq!(severity_color(Some(10.0)), color(ColorToken::Orange));
        assert_eq!(severity_color(Some(26.0)), color(ColorToken::Orange));
        assert_eq!(severity_color(Some(29.9)), color(ColorToken::Orange));
    }

    #[test]
    fn severity_color_red_below_10() {
        assert_eq!(severity_color(Some(0.0)), color(ColorToken::Red));
        assert_eq!(severity_color(Some(1.0)), color(ColorToken::Red));
        assert_eq!(severity_color(Some(9.9)), color(ColorToken::Red));
    }

    #[test]
    fn api_severity_takes_precedence() {
        use crate::theme::ColorToken;
        // API diz normal mesmo com pct baixo → verde (fonte oficial vence):
        assert_eq!(
            severity_color_api(Some("normal"), Some(5.0)),
            to_ratatui(ColorToken::Green)
        );
        assert_eq!(
            severity_color_api(Some("warning"), Some(90.0)),
            to_ratatui(ColorToken::Yellow)
        );
        assert_eq!(
            severity_color_api(Some("critical"), None),
            to_ratatui(ColorToken::Red)
        );
        // Desconhecida/absent → fallback threshold local:
        assert_eq!(
            severity_color_api(Some("banana"), Some(5.0)),
            severity_color(Some(5.0))
        );
        assert_eq!(
            severity_color_api(None, Some(50.0)),
            severity_color(Some(50.0))
        );
    }
}
