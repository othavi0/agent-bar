//! Estado de login derivado do ÚLTIMO FETCH REAL — nunca de path.exists().
//! Substitui a checagem fraca que fazia a aba Login mostrar [ok] com o
//! dashboard em erro (spec §4.3).

use crate::providers::types::ProviderQuota;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoginState {
    /// Último fetch retornou quota sem erro.
    Ok,
    /// Fonte presente mas auth inválida (erro de API/token no fetch).
    NoToken,
    /// Sem sessão (erro tipado de não-logado, ou provider nunca visto).
    LoggedOut,
    /// Fetch em voo para este provider.
    Checking,
}

pub fn login_state_for(quota: Option<&ProviderQuota>, fetch_pending: bool) -> LoginState {
    if fetch_pending {
        return LoginState::Checking;
    }
    match quota {
        None => LoginState::LoggedOut,
        Some(q) => match (&q.error, q.available) {
            (None, _) => LoginState::Ok,
            (Some(e), _) if e.starts_with("Not logged in") => LoginState::LoggedOut,
            (Some(_), true) => LoginState::NoToken,
            (Some(_), false) => LoginState::LoggedOut,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::types::ProviderQuota;

    fn quota(available: bool, error: Option<&str>) -> ProviderQuota {
        ProviderQuota {
            provider: "claude".into(),
            display_name: "Claude".into(),
            available,
            account: None,
            plan: None,
            plan_type: None,
            primary: None,
            secondary: None,
            models: None,
            extra: None,
            error: error.map(|s| s.to_string()),
        }
    }

    #[test]
    fn fetch_ok_is_logged_in() {
        assert_eq!(
            login_state_for(Some(&quota(true, None)), false),
            LoginState::Ok
        );
    }

    #[test]
    fn not_logged_in_error_is_logged_out() {
        // Mensagem padrão de não-logado é contrato (CLAUDE.md §7).
        let q = quota(
            false,
            Some("Not logged in. Open `agent-bar menu` and choose Provider login."),
        );
        assert_eq!(login_state_for(Some(&q), false), LoginState::LoggedOut);
    }

    #[test]
    fn other_error_with_source_present_is_no_token() {
        let q = quota(true, Some("Claude API error 401"));
        assert_eq!(login_state_for(Some(&q), false), LoginState::NoToken);
    }

    #[test]
    fn pending_fetch_is_checking() {
        assert_eq!(login_state_for(None, true), LoginState::Checking);
    }

    #[test]
    fn absent_quota_without_fetch_is_logged_out() {
        assert_eq!(login_state_for(None, false), LoginState::LoggedOut);
    }
}
