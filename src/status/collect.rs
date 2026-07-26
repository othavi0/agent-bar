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
            ProviderAction::view_installation("View installation", installation_url)?,
        ),
        ProviderResult::Unauthenticated {
            id,
            name,
            message,
            login_available,
            installation_url,
        } => {
            let action = if login_available {
                ProviderAction::login("Log in")
            } else {
                ProviderAction::view_installation("View installation", installation_url)?
            };
            ProviderStatus::unauthenticated(
                id,
                name,
                ProviderError::new(ErrorCode::AuthenticationRequired, message, false),
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
    }
}
