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

/// Label de plano para um quota: normaliza `plan` (ou `plan_type` como fallback);
/// "Unknown" quando nenhum resolve.
pub fn normalize_plan_label(p: &crate::providers::types::ProviderQuota) -> String {
    normalize_plan(p.plan.as_deref().or(p.plan_type.as_deref()))
        .unwrap_or_else(|| "Unknown".to_string())
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
    fn normalize_plan_label_prefers_plan_then_type_then_unknown() {
        use crate::providers::types::ProviderQuota;
        let mut q = ProviderQuota {
            provider: "codex".into(),
            display_name: "Codex".into(),
            available: true,
            account: None,
            plan: Some("pro".into()),
            plan_type: Some("ignored".into()),
            primary: None,
            secondary: None,
            models: None,
            extra: None,
            error: None,
            stale_reason: None,
        };
        assert_eq!(normalize_plan_label(&q), "Pro");
        q.plan = None;
        assert_eq!(normalize_plan_label(&q), "Ignored"); // titlecase do plan_type
        q.plan_type = None;
        assert_eq!(normalize_plan_label(&q), "Unknown");
    }

    #[test]
    fn eta_label_by_mode() {
        use crate::settings::DisplayMode;
        assert_eq!(eta_label(DisplayMode::Used), "Resets in");
        assert_eq!(eta_label(DisplayMode::Remaining), "Full in");
    }
}

use crate::formatters::clock::Clock;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

fn parse_iso(iso: &str) -> Option<OffsetDateTime> {
    OffsetDateTime::parse(iso, &Rfc3339).ok()
}

/// `Full` se remaining==100; `?` sem iso; `0h 00m` se já passou; `{d}d {hh}h` ou `{h}h {mm}m`.
pub fn format_eta(clock: &Clock, iso: Option<&str>, remaining: f64) -> String {
    if remaining == 100.0 {
        return "Full".to_string();
    }
    let Some(iso) = iso else {
        return "?".to_string();
    };
    let Some(dt) = parse_iso(iso) else {
        return "?".to_string();
    };
    let diff = dt - clock.now;
    if diff.is_negative() {
        return "0h 00m".to_string();
    }
    let secs = diff.whole_seconds();
    let d = secs / 86_400;
    let h = (secs % 86_400) / 3_600;
    let m = (secs % 3_600) / 60;
    if d > 0 {
        format!("{d}d {h:02}h")
    } else {
        format!("{h}h {m:02}m")
    }
}

/// `` se remaining==100; `(??:??)` sem iso; senão `({HH}:{MM})` em horário LOCAL.
pub fn format_reset_time(clock: &Clock, iso: Option<&str>, remaining: f64) -> String {
    if remaining == 100.0 {
        return String::new();
    }
    let Some(iso) = iso else {
        return "(??:??)".to_string();
    };
    let Some(dt) = parse_iso(iso) else {
        return "(??:??)".to_string();
    };
    let local = dt.to_offset(clock.local_offset);
    format!("({:02}:{:02})", local.hour(), local.minute())
}

/// `just now` (<60s); `{m}m ago` (<60min); senão `{h}h ago`.
pub fn format_ago(clock: &Clock, iso: &str) -> String {
    let Some(dt) = parse_iso(iso) else {
        return "?".to_string();
    };
    let diff = clock.now - dt;
    if diff.whole_milliseconds() < 60_000 {
        return "just now".to_string();
    }
    let mins = diff.whole_minutes();
    if mins < 60 {
        format!("{mins}m ago")
    } else {
        format!("{}h ago", mins / 60)
    }
}

#[cfg(test)]
mod time_tests {
    use super::*;
    use crate::formatters::clock::Clock;
    use time::macros::datetime;

    // Clock fixo: agora = 2026-06-19 12:00:00 UTC, offset local = +03:00.
    fn fixed_clock() -> Clock {
        Clock {
            now: datetime!(2026-06-19 12:00:00 UTC),
            local_offset: time::UtcOffset::from_hms(3, 0, 0).unwrap(),
        }
    }

    #[test]
    fn format_eta_cases() {
        let c = fixed_clock();
        assert_eq!(format_eta(&c, None, 50.0), "?");
        assert_eq!(format_eta(&c, Some("2026-06-19T14:05:00Z"), 50.0), "2h 05m");
        assert_eq!(format_eta(&c, Some("2026-06-21T13:00:00Z"), 50.0), "2d 01h");
        assert_eq!(format_eta(&c, Some("2026-06-19T11:00:00Z"), 50.0), "0h 00m"); // passado
        assert_eq!(format_eta(&c, Some("2026-06-19T14:00:00Z"), 100.0), "Full");
    }

    #[test]
    fn format_reset_time_uses_local_offset() {
        let c = fixed_clock();
        // 14:05 UTC + offset +03:00 = 17:05 local
        assert_eq!(
            format_reset_time(&c, Some("2026-06-19T14:05:00Z"), 50.0),
            "(17:05)"
        );
        assert_eq!(format_reset_time(&c, None, 50.0), "(??:??)");
        assert_eq!(
            format_reset_time(&c, Some("2026-06-19T14:00:00Z"), 100.0),
            ""
        );
    }

    #[test]
    fn format_ago_cases() {
        let c = fixed_clock();
        assert_eq!(format_ago(&c, "2026-06-19T11:59:30Z"), "just now"); // 30s
        assert_eq!(format_ago(&c, "2026-06-19T11:30:00Z"), "30m ago");
        assert_eq!(format_ago(&c, "2026-06-19T09:00:00Z"), "3h ago");
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
            severity: None,
        };
        assert_eq!(to_window_display(Some(&w), DisplayMode::Used), Some(70.0));
        assert_eq!(
            to_window_display(Some(&w), DisplayMode::Remaining),
            Some(30.0)
        );
        assert_eq!(to_window_display(None, DisplayMode::Remaining), None);
    }
}
