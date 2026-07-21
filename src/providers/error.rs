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
    #[error("Request timeout")]
    Timeout,
    #[error("Failed to parse usage")]
    ParseFailed,
    #[error("Failed to fetch Amp usage")]
    Generic,
}

#[derive(Error, Debug)]
pub enum GrokError {
    #[error("Grok CLI not installed. Install from https://x.ai/cli or ensure ~/.grok/bin/grok is on PATH.")]
    NotInstalled,
    #[error("Not logged in. Open `agent-bar menu` and choose Provider login.")]
    NotLoggedIn,
    #[error("Failed to read Grok credentials.")]
    InvalidCredentials,
}

#[derive(Error, Debug)]
pub enum ProviderError {
    #[error(transparent)]
    Claude(#[from] ClaudeError),
    #[error(transparent)]
    Codex(#[from] CodexError),
    #[error(transparent)]
    Amp(#[from] AmpError),
    #[error(transparent)]
    Grok(#[from] GrokError),
}

impl ProviderError {
    /// Transitório = falha de infra (rede/CLI/parse) que não significa
    /// logout; o caller pode servir cache stale. Credencial/logout → false.
    pub fn is_transient(&self) -> bool {
        match self {
            ProviderError::Claude(e) => matches!(
                e,
                ClaudeError::Timeout | ClaudeError::Api(_) | ClaudeError::Generic
            ),
            ProviderError::Codex(e) => matches!(e, CodexError::Generic),
            ProviderError::Amp(e) => matches!(
                e,
                AmpError::Timeout | AmpError::Generic | AmpError::ParseFailed
            ),
            ProviderError::Grok(_) => false,
        }
    }
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
    fn grok_strings_are_verbatim() {
        assert_eq!(
            GrokError::NotInstalled.to_string(),
            "Grok CLI not installed. Install from https://x.ai/cli or ensure ~/.grok/bin/grok is on PATH."
        );
        assert_eq!(
            GrokError::NotLoggedIn.to_string(),
            format!("Not logged in. Open `{APP_NAME} menu` and choose Provider login.")
        );
        assert_eq!(
            GrokError::InvalidCredentials.to_string(),
            "Failed to read Grok credentials."
        );
    }

    #[test]
    fn provider_error_wraps_transparently() {
        let e: ProviderError = ClaudeError::NoAccessToken.into();
        assert_eq!(e.to_string(), "No access token");
    }

    #[test]
    fn amp_timeout_string_verbatim() {
        assert_eq!(AmpError::Timeout.to_string(), "Request timeout");
    }

    #[test]
    fn transient_classification() {
        assert!(ProviderError::from(ClaudeError::Timeout).is_transient());
        assert!(ProviderError::from(ClaudeError::Api(500)).is_transient());
        assert!(ProviderError::from(ClaudeError::Generic).is_transient());
        assert!(!ProviderError::from(ClaudeError::NotLoggedIn).is_transient());
        assert!(!ProviderError::from(ClaudeError::TokenExpired).is_transient());
        assert!(ProviderError::from(AmpError::Timeout).is_transient());
        assert!(ProviderError::from(AmpError::Generic).is_transient());
        assert!(ProviderError::from(AmpError::ParseFailed).is_transient());
        assert!(!ProviderError::from(AmpError::NotLoggedIn).is_transient());
        assert!(!ProviderError::from(AmpError::NotInstalled).is_transient());
        assert!(ProviderError::from(CodexError::Generic).is_transient());
        assert!(!ProviderError::from(CodexError::NotLoggedIn).is_transient());
        assert!(!ProviderError::from(CodexError::NoRateLimitData).is_transient());
        assert!(!ProviderError::from(GrokError::NotLoggedIn).is_transient());
    }
}
