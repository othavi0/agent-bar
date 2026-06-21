//! Erros por-provider com mensagens VERBATIM (são contrato — testes assertam a
//! string exata). `ProviderError` agrega via `#[from]`. Só a camada de provider
//! usa estes tipos; comandos usam `anyhow`.

use thiserror::Error;

#[derive(Error, Debug)]
pub enum ClaudeError {
    #[error("Not logged in. Open `agent-bar menu` and choose Provider login.")]
    NotLoggedIn,
    #[error("Invalid credentials file")]
    InvalidCredentials,
    #[error("No access token")]
    NoAccessToken,
    #[error("Token expired. Open `agent-bar menu` and choose Provider login.")]
    TokenExpired,
    #[error("Request timeout")]
    Timeout,
    #[error("Claude API error: {0}")]
    Api(u16),
    #[error("Failed to fetch Claude usage")]
    Generic,
}

#[derive(Error, Debug)]
pub enum CodexError {
    #[error("Not logged in. Open `agent-bar menu` and choose Provider login.")]
    NotLoggedIn,
    #[error("No session data found")]
    NoSessionData,
    #[error("No rate limit data found (app-server + session log)")]
    NoRateLimitData,
    #[error("No quota windows found")]
    NoQuotaWindows,
    #[error("Failed to fetch Codex usage")]
    Generic,
}

#[derive(Error, Debug)]
pub enum AmpError {
    #[error("Amp CLI not installed. Right-click to install and log in.")]
    NotInstalled,
    #[error("Not logged in. Open `agent-bar menu` and choose Provider login.")]
    NotLoggedIn,
    #[error("Failed to parse usage")]
    ParseFailed,
    #[error("Failed to fetch Amp usage")]
    Generic,
}

#[derive(Error, Debug)]
pub enum ProviderError {
    #[error(transparent)]
    Claude(#[from] ClaudeError),
    #[error(transparent)]
    Codex(#[from] CodexError),
    #[error(transparent)]
    Amp(#[from] AmpError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_identity::APP_NAME;

    #[test]
    fn claude_strings_are_verbatim() {
        assert_eq!(
            ClaudeError::NotLoggedIn.to_string(),
            format!("Not logged in. Open `{APP_NAME} menu` and choose Provider login.")
        );
        assert_eq!(
            ClaudeError::InvalidCredentials.to_string(),
            "Invalid credentials file"
        );
        assert_eq!(ClaudeError::NoAccessToken.to_string(), "No access token");
        assert_eq!(
            ClaudeError::TokenExpired.to_string(),
            format!("Token expired. Open `{APP_NAME} menu` and choose Provider login.")
        );
        assert_eq!(ClaudeError::Timeout.to_string(), "Request timeout");
        assert_eq!(ClaudeError::Api(404).to_string(), "Claude API error: 404");
        assert_eq!(
            ClaudeError::Generic.to_string(),
            "Failed to fetch Claude usage"
        );
    }

    #[test]
    fn codex_strings_are_verbatim() {
        assert_eq!(
            CodexError::NotLoggedIn.to_string(),
            format!("Not logged in. Open `{APP_NAME} menu` and choose Provider login.")
        );
        assert_eq!(
            CodexError::NoSessionData.to_string(),
            "No session data found"
        );
        assert_eq!(
            CodexError::NoRateLimitData.to_string(),
            "No rate limit data found (app-server + session log)"
        );
        assert_eq!(
            CodexError::NoQuotaWindows.to_string(),
            "No quota windows found"
        );
        assert_eq!(
            CodexError::Generic.to_string(),
            "Failed to fetch Codex usage"
        );
    }

    #[test]
    fn amp_strings_are_verbatim() {
        assert_eq!(
            AmpError::NotInstalled.to_string(),
            "Amp CLI not installed. Right-click to install and log in."
        );
        assert_eq!(
            AmpError::NotLoggedIn.to_string(),
            format!("Not logged in. Open `{APP_NAME} menu` and choose Provider login.")
        );
        assert_eq!(AmpError::ParseFailed.to_string(), "Failed to parse usage");
        assert_eq!(AmpError::Generic.to_string(), "Failed to fetch Amp usage");
    }

    #[test]
    fn provider_error_wraps_transparently() {
        let e: ProviderError = ClaudeError::NoAccessToken.into();
        assert_eq!(e.to_string(), "No access token");
    }
}
