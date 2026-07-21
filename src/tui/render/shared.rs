//! Helpers compartilhados entre telas de render. `series_now` calcula a
//! âncora temporal da série real de 24h; `history.rs` é quem consome hoje
//! (`render_history`, que alimenta o `render_top_chart` do chart de
//! History).
//!
//! `abbrev_tokens` (formatador de tokens com ponto decimal) morou aqui até
//! o fix gate de dados reais — removido: `detail.rs` unificou pra
//! `column_chart::fmt_tokens_short` (vírgula decimal), a mesma usada pelas
//! legendas do chart e por `history.rs`, pra não ter dois formatos de
//! número na mesma tela (`719.6M` vs `719,6M`).

use crate::tui::state::AppState;

/// "now" para a série 24h do sparkline: NUNCA `OffsetDateTime::now_utc()`
/// (render precisa ser puro/determinístico p/ snapshot). Fonte primária =
/// `state.last_update`; fallback = timestamp mais recente de `state.history`;
/// ambos ausentes → sem âncora, série fica vazia (sparkline vazio, ok).
pub fn series_now(state: &AppState) -> Option<time::OffsetDateTime> {
    state.last_update.or_else(|| {
        state
            .history
            .as_deref()
            .and_then(|r| r.iter().map(|u| u.ts).max())
    })
}
