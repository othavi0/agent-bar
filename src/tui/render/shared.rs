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
    state.last_update.or_else(|| {
        state
            .history
            .as_deref()
            .and_then(|r| r.iter().map(|u| u.ts).max())
    })
}

/// Formata tokens em unidade legível ("14.2M" / "1.2K" / "500"). Escolhe a
/// MENOR unidade cujo valor arredondado a 1 casa decimal fique < 1000 (senão
/// a última, "B") — nunca a unidade "óbvia" pelo tamanho bruto de `n`, que
/// deixava a fronteira estourar (`999_950` virava "1000.0K" em vez de
/// "1.0M"; regressão pega em review, T12). Movida de `detail.rs` (T13) —
/// `render/history.rs` também precisa (labels do eixo Y + coluna "tokens"
/// da tabela).
pub fn abbrev_tokens(n: u64) -> String {
    const UNITS: [&str; 4] = ["", "K", "M", "B"];
    let last = UNITS.len() - 1;
    let mut idx = 0;
    while idx < last {
        let scale = 1000f64.powi(idx as i32);
        let rounded = ((n as f64 / scale) * 10.0).round() / 10.0;
        if rounded < 1000.0 {
            break;
        }
        idx += 1;
    }
    if idx == 0 {
        return n.to_string();
    }
    let scale = 1000f64.powi(idx as i32);
    format!("{:.1}{}", n as f64 / scale, UNITS[idx])
}

/// Rótulo duplo de tokens (decisão de produto, spec 2026-07-03): principal
/// = input+output (o que o usuário entende por "tokens que usei"); sufixo
/// = cache (read+write), presente só quando > 0. Em largura apertada, quem
/// chama pode dropar o sufixo — NUNCA o principal. Mata a divergência entre
/// `render/detail.rs` (que somava cache no total) e `render/history.rs`/
/// `render/dashboard.rs` (que só somavam input+output): as três telas agora
/// falam o mesmo vocabulário.
pub fn fmt_tokens_dual(io: u64, cache: u64) -> String {
    if cache == 0 {
        return abbrev_tokens(io);
    }
    format!("{} (+{} cache)", abbrev_tokens(io), abbrev_tokens(cache))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn abbrev_tokens_below_1000_is_raw() {
        assert_eq!(abbrev_tokens(0), "0");
        assert_eq!(abbrev_tokens(500), "500");
        assert_eq!(abbrev_tokens(999), "999");
    }

    #[test]
    fn abbrev_tokens_thousands_and_millions() {
        assert_eq!(abbrev_tokens(1_200), "1.2K");
        assert_eq!(abbrev_tokens(14_200_000), "14.2M");
        assert_eq!(abbrev_tokens(999_000), "999.0K");
    }

    /// Regressão (review T12): a unidade "óbvia" pelo tamanho bruto de `n`
    /// podia estourar a fronteira de 1000 depois do arredondamento a 1 casa
    /// decimal (999_950 virava "1000.0K"). `abbrev_tokens` tem que escolher
    /// a MENOR unidade cujo valor arredondado fique < 1000.
    #[test]
    fn abbrev_tokens_never_rounds_across_unit_boundary() {
        assert_eq!(abbrev_tokens(999), "999");
        assert_eq!(abbrev_tokens(1_000), "1.0K");
        assert_eq!(abbrev_tokens(999_950), "1.0M");
        assert_eq!(abbrev_tokens(999_999_999), "1.0B");
        assert_eq!(abbrev_tokens(5_686_100_000), "5.7B");
    }

    #[test]
    fn fmt_tokens_dual_formats_io_plus_cache() {
        assert_eq!(fmt_tokens_dual(9_900_000, 1_400_000_000), "9.9M (+1.4B cache)");
        assert_eq!(fmt_tokens_dual(9_900_000, 0), "9.9M");
        assert_eq!(fmt_tokens_dual(0, 500), "0 (+500 cache)");
    }
}
