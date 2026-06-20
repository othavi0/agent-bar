//! Handler do right-click do Waybar (`agent-bar action-right <provider>`).
//! Port fiel de `src/action-right.ts:28-90`.
//!
//! Roteamento:
//! - Provider indisponível ou desconectado → **login stub** (TUI não portado no Plano 5).
//! - Conectado → invalida cache, busca quota fresca e imprime view terminal.
//!
//! `looks_disconnected` espelha as 2 regexes de action-right.ts:52-56 (case-insensitive).

use std::io::BufRead as _;
use std::sync::OnceLock;

use regex::Regex;

use crate::app_identity::APP_NAME;
use crate::formatters::clock::Clock;
use crate::formatters::terminal::format_for_terminal;
use crate::providers::types::AllQuotas;
use crate::providers::{get_provider, get_quota_for, iso_from_ms, Ctx};

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

// ---- Utilitários -------------------------------------------------------

/// Lê uma linha de stdin (bloqueante). Mantém o popup do Waybar aberto até
/// o usuário pressionar Enter — espelha `waitEnter` do TS.
fn wait_enter() {
    let stdin = std::io::stdin();
    let mut line = String::new();
    let _ = stdin.lock().read_line(&mut line);
}

/// Lanca o login interativo para o provider, sem TUI ativa (contexto right-click).
/// Usa `launch_login_no_tui` de `tui::login_spawn` para executar o CLI com
/// stdio herdado. Em caso de erro, loga e aguarda Enter.
fn login_stub(provider_id: &str) {
    if let Err(e) = crate::tui::login_spawn::launch_login_no_tui(provider_id) {
        log::error!("Falha no login de '{}': {}", provider_id, e);
    }
    wait_enter();
}

// ---- Handler principal -------------------------------------------------

/// Handler do right-click do Waybar. Espelha `handleActionRight` do TS.
///
/// Fluxo:
/// 1. `provider_id` vazio → erro + exit(1).
/// 2. Provider desconhecido → erro + wait_enter + return.
/// 3. Provider indisponível → login stub + return.
/// 4. Quota indica desconexão → login stub + return.
/// 5. Conectado → refresh + imprime view terminal + wait_enter.
pub async fn handle_action_right(provider_id: &str, ctx: &Ctx<'_>, clock: &Clock, no_color: bool) {
    // 1. Argumento vazio.
    if provider_id.is_empty() {
        log::error!("Usage: {APP_NAME} action-right <provider>");
        std::process::exit(1);
    }

    // 2. Provider desconhecido.
    let provider = match get_provider(provider_id) {
        Some(p) => p,
        None => {
            log::error!("Unknown provider: {provider_id}");
            wait_enter();
            return;
        }
    };

    // 3. Provider indisponível (sem credenciais ou binário ausente).
    let available = provider.is_available(ctx).await;
    if !available {
        login_stub(provider_id);
        return;
    }

    // 4. Quota indica desconexão (token expirado, sessão inválida, etc.).
    let quota = provider.get_quota(ctx).await;
    if looks_disconnected(provider_id, quota.error.as_deref()) {
        login_stub(provider_id);
        return;
    }

    // 5. Conectado: força refresh ignorando TTL e imprime view terminal.
    crate::cache::invalidate(&ctx.paths.cache_dir, provider.cache_key());

    match get_quota_for(provider_id, ctx).await {
        Some(fresh) => {
            let quotas = AllQuotas {
                providers: vec![fresh],
                fetched_at: iso_from_ms(ctx.now_ms),
            };
            println!(
                "{}",
                format_for_terminal(
                    clock,
                    &quotas,
                    ctx.settings,
                    ctx.settings.waybar.display_mode,
                    no_color,
                )
            );
        }
        None => {
            log::error!("Failed to fetch {} quota", provider.name());
        }
    }

    wait_enter();
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
}
