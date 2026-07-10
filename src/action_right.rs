//! Handler do right-click do Waybar (`agent-bar action-right <provider>`).
//!
//! Roteamento (Task 12): resolve o foco inicial da TUI a partir do estado do
//! provider e devolve pro caller abrir `tui::run_tui` já focada — sem login
//! stub nem view terminal (essa era a UX pré-TUI do Plano 5).
//!
//! `looks_disconnected` espelha as 2 regexes de action-right.ts:52-56 (case-insensitive).

use std::sync::OnceLock;

use regex::Regex;

use crate::app_identity::APP_NAME;
use crate::providers::{get_provider, Ctx};

// ---- Detecção de desconexão ----------------------------------------

/// Pattern base: expirado, não logado, ou pedindo re-login.
const PATTERN_BASE: &str = r"(?i)expired|not logged in|login again|please login";

/// Pattern extra do Codex: sem sessão, sem rate-limit, auth genérico, token.
const PATTERN_CODEX: &str = r"(?i)no session data|no rate limit data|auth|token";

/// Compila a regex base uma vez, retornando `None` apenas se o pattern for
/// inválido (impossível com literal; tratado como não-match).
fn re_base() -> Option<&'static Regex> {
    static RE: OnceLock<Option<Regex>> = OnceLock::new();
    RE.get_or_init(|| Regex::new(PATTERN_BASE).ok()).as_ref()
}

/// Compila a regex Codex uma vez, idem.
fn re_codex() -> Option<&'static Regex> {
    static RE: OnceLock<Option<Regex>> = OnceLock::new();
    RE.get_or_init(|| Regex::new(PATTERN_CODEX).ok()).as_ref()
}

/// `true` se o campo `error` de uma quota indica desconexão/expiração.
///
/// Espelha `action-right.ts:52-56`:
/// ```ts
/// const looksDisconnected =
///   !!quota.error &&
///   (baseDisconnect.test(quota.error) ||
///    (providerId === 'codex' && codexDisconnect.test(quota.error)));
/// ```
///
/// `None` → `false`. `Some("")` → `false` (string vazia é falsy no TS).
pub fn looks_disconnected(provider_id: &str, error: Option<&str>) -> bool {
    let e = match error {
        Some(s) if !s.is_empty() => s,
        _ => return false,
    };
    let base_match = re_base().is_some_and(|r| r.is_match(e));
    let codex_match = provider_id == "codex" && re_codex().is_some_and(|r| r.is_match(e));
    base_match || codex_match
}

// ---- Roteamento de foco -------------------------------------------------

/// Decisão pura de foco a partir do estado do provider (testável sem IO).
pub fn focus_for(
    provider_id: &str,
    available: bool,
    quota_error: Option<&str>,
) -> crate::tui::InitialFocus {
    if !available || looks_disconnected(provider_id, quota_error) {
        crate::tui::InitialFocus::Login(provider_id.to_string())
    } else {
        crate::tui::InitialFocus::Provider(provider_id.to_string())
    }
}

/// Resolve o foco inicial da TUI pro right-click. `None` = provider inválido
/// (argumento vazio ou desconhecido; erro já logado).
pub async fn action_right_focus(
    provider_id: &str,
    ctx: &Ctx<'_>,
) -> Option<crate::tui::InitialFocus> {
    if provider_id.is_empty() {
        log::error!("Usage: {APP_NAME} action-right <provider>");
        return None;
    }
    let provider = match get_provider(provider_id) {
        Some(p) => p,
        None => {
            log::error!("Unknown provider: {provider_id}");
            return None;
        }
    };
    let available = provider.is_available(ctx).await;
    let error = if available {
        provider.get_quota(ctx).await.error
    } else {
        None
    };
    Some(focus_for(provider_id, available, error.as_deref()))
}

// ---- Testes -----------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // --- looks_disconnected ---

    #[test]
    fn expired_is_disconnected_for_claude() {
        // "Token expired" → base regex "expired" (case-insensitive).
        assert!(looks_disconnected("claude", Some("Token expired")));
    }

    #[test]
    fn please_login_is_disconnected_for_claude() {
        assert!(looks_disconnected("claude", Some("please login")));
    }

    #[test]
    fn no_rate_limit_data_is_not_disconnected_for_amp() {
        // pattern codex-only; provider ≠ "codex" → false.
        assert!(!looks_disconnected("amp", Some("no rate limit data")));
    }

    #[test]
    fn no_rate_limit_data_is_disconnected_for_codex() {
        assert!(looks_disconnected("codex", Some("no rate limit data")));
    }

    #[test]
    fn auth_failed_is_disconnected_for_codex() {
        assert!(looks_disconnected("codex", Some("auth failed")));
    }

    #[test]
    fn network_blip_is_not_disconnected() {
        assert!(!looks_disconnected("claude", Some("network blip")));
    }

    #[test]
    fn none_error_is_not_disconnected() {
        assert!(!looks_disconnected("claude", None));
    }

    #[test]
    fn empty_error_is_not_disconnected() {
        // string vazia é falsy no TS (!!quota.error === false).
        assert!(!looks_disconnected("claude", Some("")));
    }

    // --- focus_for ---

    #[test]
    fn focus_routes_disconnected_to_login() {
        match focus_for("claude", true, Some("Token expired")) {
            crate::tui::InitialFocus::Login(id) => assert_eq!(id, "claude"),
            other => panic!("esperava Login, veio {other:?}"),
        }
    }

    #[test]
    fn focus_routes_connected_to_provider_detail() {
        match focus_for("claude", true, None) {
            crate::tui::InitialFocus::Provider(id) => assert_eq!(id, "claude"),
            other => panic!("esperava Provider, veio {other:?}"),
        }
    }

    #[test]
    fn focus_routes_unavailable_to_login() {
        assert!(matches!(
            focus_for("amp", false, None),
            crate::tui::InitialFocus::Login(_)
        ));
    }
}
