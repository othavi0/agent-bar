//! Handlers da tela History (range, lista de dias, expand).

use crate::tui::action::Action;
use crate::tui::state::{AppState, HistoryRange};

pub(super) fn history_loaded(
    state: &mut AppState,
    records: Vec<crate::usage::UsageRecord>,
) -> Vec<Action> {
    state.history = Some(records);
    vec![]
}

pub(super) fn toggle_history_range(state: &mut AppState) -> Vec<Action> {
    state.history_range = match state.history_range {
        HistoryRange::Day => HistoryRange::Week,
        HistoryRange::Week => HistoryRange::Day,
    };
    vec![]
}

pub(super) fn history_down(state: &mut AppState) -> Vec<Action> {
    // `sessions_by_day` é barato o bastante pra recomputar por tecla
    // (7 dias de records — YAGNI cachear no state por ora). Clamp
    // aqui, não no render: `history_selected` nunca aponta pra fora
    // da lista real.
    let n_days = state
        .history
        .as_deref()
        .map(|r| crate::usage::buckets::sessions_by_day(r, state.local_offset).len())
        .unwrap_or(0);
    if n_days > 0 {
        state.history_selected = (state.history_selected + 1).min(n_days - 1);
    }
    vec![]
}

pub(super) fn history_up(state: &mut AppState) -> Vec<Action> {
    state.history_selected = state.history_selected.saturating_sub(1);
    vec![]
}

pub(super) fn history_toggle_day(state: &mut AppState) -> Vec<Action> {
    if let Some(records) = state.history.as_deref() {
        let days = crate::usage::buckets::sessions_by_day(records, state.local_offset);
        if let Some(day) = days.get(state.history_selected) {
            if !state.history_expanded.remove(&day.date) {
                state.history_expanded.insert(day.date);
            }
        }
    }
    vec![]
}
