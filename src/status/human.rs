//! Terminal-safe English human status formatting (CLI-012).

use super::schema::{DataSource, ProviderState, StatusEnvelope};

/// Format a validated envelope as plain English text for `status format human`.
pub fn format_human(envelope: &StatusEnvelope) -> String {
    let mut out = String::new();
    out.push_str("Agent Bar status\n");
    out.push_str(&format!("helper {}\n", envelope.helper_version()));
    if envelope.providers().is_empty() {
        out.push_str("No providers.\n");
        return out;
    }
    for provider in envelope.providers() {
        out.push('\n');
        out.push_str(&format!("{} ({})\n", provider.name(), provider.id()));
        out.push_str(&format!("  state: {}\n", provider.state().as_str()));
        match provider.source() {
            Some(DataSource::Live) => out.push_str("  source: live\n"),
            Some(DataSource::Cache) => out.push_str("  source: cache\n"),
            None => out.push_str("  source: —\n"),
        }
        if provider.windows().is_empty() {
            if matches!(
                provider.state(),
                ProviderState::Ready | ProviderState::Stale
            ) {
                out.push_str("  windows: —\n");
            }
        } else {
            for window in provider.windows() {
                out.push_str(&format!(
                    "  {}: used {:.1}% remaining {:.1}%\n",
                    window.label(),
                    window.used_percent(),
                    window.remaining_percent()
                ));
            }
        }
        if let Some(error) = provider.error() {
            out.push_str(&format!("  error: {}\n", error.message));
        }
        if let Some(action) = provider.action() {
            out.push_str(&format!("  action: {}\n", action.label));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::{CacheMode, ProviderId};
    use crate::status::schema::{
        Account, DataSource, Plan, ProviderStatus, StatusRequest, UsageWindow,
    };
    use time::macros::datetime;

    #[test]
    fn human_output_is_plain_text_without_json() {
        let provider = ProviderStatus::ready(
            ProviderId::Claude,
            "Claude",
            DataSource::Live,
            Some(Plan {
                id: "max".into(),
                label: "Max".into(),
            }),
            Some(Account {
                label: "Personal".into(),
            }),
            vec![UsageWindow::try_new("session", "Session", 42.0, 58.0, None).unwrap()],
            datetime!(2026-07-26 18:42:00 UTC),
        )
        .unwrap();
        let envelope = StatusEnvelope::try_new_for_package(
            datetime!(2026-07-26 18:42:00 UTC),
            StatusRequest {
                provider: None,
                cache: CacheMode::Use,
            },
            vec![provider],
        )
        .unwrap();
        let text = format_human(&envelope);
        assert!(text.contains("Claude"));
        assert!(text.contains("42.0%"));
        assert!(!text.contains('{'));
        assert!(!text.contains("schemaVersion"));
    }
}
