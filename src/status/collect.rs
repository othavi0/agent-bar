//! Map temporary [`ProviderResult`] values into validated [`ProviderStatus`].

use super::schema::{
    ErrorCode, ProviderAction, ProviderError, ProviderResult, ProviderStatus, SchemaError,
};

/// Convert a typed collection result into a completed provider status row.
pub fn provider_status_from_result(result: ProviderResult) -> Result<ProviderStatus, SchemaError> {
    match result {
        ProviderResult::Ready {
            id,
            name,
            source,
            plan,
            account,
            windows,
            last_success_at,
        } => ProviderStatus::ready(id, name, source, plan, account, windows, last_success_at),
        ProviderResult::Stale {
            id,
            name,
            plan,
            account,
            windows,
            last_success_at,
            error,
        } => ProviderStatus::stale(
            id,
            name,
            plan,
            account,
            windows,
            last_success_at,
            error,
            ProviderAction::retry("Retry"),
        ),
        ProviderResult::CliMissing {
            id,
            name,
            message,
            installation_url,
        } => ProviderStatus::cli_missing(
            id,
            name,
            ProviderError::new(ErrorCode::CliNotFound, message, false),
            ProviderAction::view_installation("Install guide", installation_url)?,
        ),
        ProviderResult::Unauthenticated {
            id,
            name,
            message,
            login_available,
            installation_url,
            retryable,
        } => {
            let action = if login_available {
                ProviderAction::login("Sign in")
            } else {
                ProviderAction::view_installation("Install guide", installation_url)?
            };
            ProviderStatus::unauthenticated(
                id,
                name,
                ProviderError::new(ErrorCode::AuthenticationRequired, message, retryable),
                action,
            )
        }
        ProviderResult::RateLimited { id, name, message } => ProviderStatus::rate_limited(
            id,
            name,
            ProviderError::new(ErrorCode::RateLimited, message, true),
            ProviderAction::retry("Retry"),
        ),
        ProviderResult::NetworkError { id, name, message } => ProviderStatus::network_error(
            id,
            name,
            ProviderError::new(ErrorCode::NetworkError, message, true),
            ProviderAction::retry("Retry"),
        ),
        ProviderResult::ProviderError {
            id,
            name,
            message,
            retryable,
        } => ProviderStatus::provider_error(
            id,
            name,
            ProviderError::new(ErrorCode::ProviderError, message, retryable),
            ProviderAction::retry("Retry"),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::ProviderId;
    use crate::status::schema::{DataSource, ProviderState, UsageWindow};
    use time::macros::datetime;

    #[test]
    fn maps_ready_and_cli_missing_results() {
        let ready = provider_status_from_result(ProviderResult::Ready {
            id: ProviderId::Claude,
            name: "Claude".into(),
            source: DataSource::Live,
            plan: None,
            account: None,
            windows: vec![UsageWindow::try_new("session", "Session", 10.0, 90.0, None).unwrap()],
            last_success_at: datetime!(2026-07-26 18:42:00 UTC),
        })
        .unwrap();
        assert_eq!(ready.state(), ProviderState::Ready);

        let missing = provider_status_from_result(ProviderResult::CliMissing {
            id: ProviderId::Amp,
            name: "Amp".into(),
            message: "missing".into(),
            installation_url: "https://ampcode.com/manual".into(),
        })
        .unwrap();
        assert_eq!(missing.state(), ProviderState::CliMissing);
        assert!(missing.windows().is_empty());
        assert_eq!(
            missing.action().map(|a| a.label.as_str()),
            Some("Install guide")
        );
    }

    #[test]
    fn unauthenticated_action_label_prefers_sign_in_when_login_available() {
        let status = provider_status_from_result(ProviderResult::Unauthenticated {
            id: ProviderId::Claude,
            name: "Claude".into(),
            message: "not authenticated".into(),
            login_available: true,
            installation_url: "https://claude.ai/download".into(),
            retryable: false,
        })
        .unwrap();
        assert_eq!(status.action().map(|a| a.label.as_str()), Some("Sign in"));
    }

    #[test]
    fn unauthenticated_action_label_falls_back_to_install_guide_without_login() {
        let status = provider_status_from_result(ProviderResult::Unauthenticated {
            id: ProviderId::Codex,
            name: "Codex".into(),
            message: "not authenticated".into(),
            login_available: false,
            installation_url: "https://developers.openai.com/codex".into(),
            retryable: false,
        })
        .unwrap();
        assert_eq!(
            status.action().map(|a| a.label.as_str()),
            Some("Install guide")
        );
    }
}
