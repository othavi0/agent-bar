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

/// `?%` quando None; senão `{arredondado}%`.
pub fn format_percent(val: Option<f64>) -> String {
    match val {
        None => "?%".to_string(),
        Some(v) => format!("{}%", v.round() as i64),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowKind {
    FiveHour,
    SevenDay,
    Other,
}

/// fiveHour se |min-300|<=90; sevenDay se |min-10080|<=1440; senão other. None/<=0 → other.
pub fn classify_window(minutes: Option<i64>) -> WindowKind {
    match minutes {
        Some(m) if m > 0 => {
            if (m - 300).abs() <= 90 {
                WindowKind::FiveHour
            } else if (m - 10080).abs() <= 1440 {
                WindowKind::SevenDay
            } else {
                WindowKind::Other
            }
        }
        _ => WindowKind::Other,
    }
}

/// Normaliza o nome do plano (mapa conhecido ou titlecase). None/vazio → None.
pub fn normalize_plan(raw: Option<&str>) -> Option<String> {
    let raw = raw?;
    let key = raw.trim().to_lowercase();
    if key.is_empty() {
        return None;
    }
    let mapped = match key.as_str() {
        "free" => "Free",
        "go" => "Go",
        "plus" => "Plus",
        "pro" => "Pro",
        "business" | "team" => "Business",
        "enterprise" => "Enterprise",
        "edu" | "education" => "Edu",
        "apikey" | "api_key" => "API Key",
        _ => return Some(titlecase_plan(raw)),
    };
    Some(mapped.to_string())
}

/// Substitui `_`/`-` por espaço e capitaliza a 1ª letra de cada palavra.
fn titlecase_plan(raw: &str) -> String {
    raw.split(['_', '-'])
        .filter(|w| !w.is_empty())
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn eta_label(mode: DisplayMode) -> &'static str {
    match mode {
        DisplayMode::Used => "Resets in",
        DisplayMode::Remaining => "Full in",
    }
}

#[cfg(test)]
mod helper_tests {
    use super::*;

    #[test]
    fn format_percent_rounds_and_handles_none() {
        assert_eq!(format_percent(None), "?%");
        assert_eq!(format_percent(Some(74.6)), "75%");
        assert_eq!(format_percent(Some(0.0)), "0%");
    }

    #[test]
    fn classify_window_tolerances() {
        assert_eq!(classify_window(Some(300)), WindowKind::FiveHour);
        assert_eq!(classify_window(Some(390)), WindowKind::FiveHour); // 300+90
        assert_eq!(classify_window(Some(391)), WindowKind::Other);
        assert_eq!(classify_window(Some(10080)), WindowKind::SevenDay);
        assert_eq!(classify_window(Some(11520)), WindowKind::SevenDay); // 10080+1440
        assert_eq!(classify_window(Some(0)), WindowKind::Other);
        assert_eq!(classify_window(None), WindowKind::Other);
    }

    #[test]
    fn normalize_plan_map_and_titlecase() {
        assert_eq!(normalize_plan(Some("pro")).as_deref(), Some("Pro"));
        assert_eq!(normalize_plan(Some("TEAM")).as_deref(), Some("Business"));
        assert_eq!(normalize_plan(Some("api_key")).as_deref(), Some("API Key"));
        assert_eq!(
            normalize_plan(Some("custom_plan")).as_deref(),
            Some("Custom Plan")
        );
        assert_eq!(normalize_plan(Some("  ")), None);
        assert_eq!(normalize_plan(None), None);
    }

    #[test]
    fn eta_label_by_mode() {
        use crate::settings::DisplayMode;
        assert_eq!(eta_label(DisplayMode::Used), "Resets in");
        assert_eq!(eta_label(DisplayMode::Remaining), "Full in");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::types::QuotaWindow;
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

    #[test]
    fn to_window_display_honours_provider_used() {
        let w = QuotaWindow {
            remaining: 30.0,
            resets_at: None,
            window_minutes: None,
            used: Some(70.0),
        };
        assert_eq!(to_window_display(Some(&w), DisplayMode::Used), Some(70.0));
        assert_eq!(
            to_window_display(Some(&w), DisplayMode::Remaining),
            Some(30.0)
        );
        assert_eq!(to_window_display(None, DisplayMode::Remaining), None);
    }
}
