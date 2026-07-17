//! Handlers da aba Login e helper de id do provider selecionado.

use crate::tui::action::Action;
use crate::tui::state::AppState;

/// Mapeia o índice selecionado da aba Login pro id do provider — mesma
/// ordem de `render/login.rs::PROVIDERS`. Compartilhado entre o Enter
/// (`key_to_action_with_state`) e o clique no chip `StartLogin` (T14):
/// os dois precisam disparar a MESMA action pro provider selecionado.
pub(super) fn login_selected_id(idx: usize) -> &'static str {
    match idx {
        0 => "claude",
        1 => "codex",
        _ => "amp",
    }
}

pub(super) fn login_up(state: &mut AppState) -> Vec<Action> {
    if state.login_selected > 0 {
        state.login_selected -= 1;
    }
    state.login_status = None;
    vec![]
}

pub(super) fn login_down(state: &mut AppState) -> Vec<Action> {
    // 3 providers: indices 0, 1, 2.
    if state.login_selected < 2 {
        state.login_selected += 1;
    }
    state.login_status = None;
    vec![]
}

pub(super) fn login_requested(state: &mut AppState, id: String) -> Vec<Action> {
    // Puro: sinaliza pending_login. O event_loop pinta o status no
    // frame atual e SO ENTAO suspende o terminal para o CLI de login
    // (fix: "Abrindo login..." nunca era pintado antes desta task).
    state.login_status = Some(format!("Abrindo login para {}...", id));
    state.pending_login = Some(id);
    vec![]
}

pub(super) fn login_result(state: &mut AppState, result: Result<(), String>) -> Vec<Action> {
    state.login_status = Some(match result {
        // Refetch agora e automatico (LoginFinished), sem precisar de [r].
        Ok(()) => "Login concluido. Atualizando quota...".to_string(),
        Err(e) => format!("Erro no login: {e}"),
    });
    vec![]
}

pub(super) fn login_finished(id: String) -> Vec<Action> {
    // Re-enfileira UMA vez para o event_loop interceptar (mesmo
    // padrao de Refresh/ReloadUsage): o drain chama spawn_fetch(only)
    // direto, sem re-entrar no update com esta action.
    vec![Action::LoginFinished(id)]
}
