//! Math de exibição compartilhada (remaining vs used). DisplayMode vem de settings.

use crate::providers::types::QuotaWindow;
use crate::settings::DisplayMode;

pub fn to_display(remaining: Option<f64>, mode: DisplayMode) -> Option<f64> {
    let r = remaining?;
    Some(match mode {
        DisplayMode::Used => 100.0 - r,
        DisplayMode::Remaining => r,
    })
}

/// Valor de exibição de uma janela, honrando `used` do provider em modo `used`.
pub fn to_window_display(window: Option<&QuotaWindow>, mode: DisplayMode) -> Option<f64> {
    let w = window?;
    if let (DisplayMode::Used, Some(used)) = (mode, w.used) {
        return Some(used);
    }
    to_display(Some(w.remaining), mode)
}

pub fn to_health(display_value: Option<f64>, mode: DisplayMode) -> Option<f64> {
    let d = display_value?;
    Some(match mode {
        DisplayMode::Used => 100.0 - d,
        DisplayMode::Remaining => d,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::DisplayMode;

    #[test]
    fn to_display_modes() {
        assert_eq!(to_display(Some(70.0), DisplayMode::Remaining), Some(70.0));
        assert_eq!(to_display(Some(70.0), DisplayMode::Used), Some(30.0));
        assert_eq!(to_display(None, DisplayMode::Used), None);
    }

    #[test]
    fn to_health_inverts_used() {
        assert_eq!(to_health(Some(30.0), DisplayMode::Used), Some(70.0));
        assert_eq!(to_health(Some(70.0), DisplayMode::Remaining), Some(70.0));
        assert_eq!(to_health(None, DisplayMode::Remaining), None);
    }
}
