//! Helpers compartilhados entre telas de render. Hoje só `series_now`,
//! extraída de `dashboard.rs` (Task 11) e reaproveitada por `detail.rs`
//! (Task 12) — ambas as telas precisam da MESMA âncora temporal pra
//! `provider_series_24h` (sparkline real de 24h), então a lógica mora aqui
//! em vez de duplicada em cada módulo de tela.

use crate::tui::state::AppState;

/// "now" para a série 24h do sparkline: NUNCA `OffsetDateTime::now_utc()`
/// (render precisa ser puro/determinístico p/ snapshot). Fonte primária =
/// `state.last_update`; fallback = timestamp mais recente de `state.history`;
/// ambos ausentes → sem âncora, série fica vazia (sparkline vazio, ok).
pub fn series_now(state: &AppState) -> Option<time::OffsetDateTime> {
    state
        .last_update
        .or_else(|| state.history.as_deref().and_then(|r| r.iter().map(|u| u.ts).max()))
}
