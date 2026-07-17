//! Shared format helpers for Detail sections (label width, gauges, costs).

use crate::usage::{ModelUsage, ProviderUsage};

/// Largura do rótulo (janela/modelo) — MESMA coluna em toda seção com gauge
/// (contrato do brief: "todas alinhadas na mesma coluna de gauge"). 12 =
/// o limite de `truncate_name`, então um nome truncado nunca estoura a
/// coluna do gauge.
pub(super) const LABEL_W: usize = 12;

/// Sufixo reservado após o gauge — cada seção tem o seu (pct+reset pras
/// janelas, tokens+custo pros modelos, "$usado de $limite" pro extra
/// usage), então NÃO dá pra derivar com um valor único (era o bug do
/// primeiro draft da Task 9: reusar o sufixo das janelas pros modelos
/// estourava a borda, cortando o custo no meio — "$1.4" em vez de "$1.40").
pub(super) const WINDOW_SUFFIX_W: usize = 1 + 4 + 1 + 2 + 1 + 1 + 5; // pct(" NNN%"=6) + reset("  → "+HH:MM=9)
pub(super) const MODEL_SUFFIX_W: usize = 1 + 8 + 1 + 9; // tokens(" "+8=9) + custo(" "+9=10, larguras fixas do format!)
pub(super) const EXTRA_SUFFIX_W: usize = 22; // "  $9999.99 de $9999.99" (generoso; custo real bem menor)

/// Deriva a largura do gauge a partir da área real do conteúdo e do
/// `suffix_w` de quem chama — MESMA função usada por janelas, modelos hoje
/// e extra usage (Task 9: antes só as janelas deriviam, "Modelos
/// hoje"/"extra usage" tinham `FIXED_GAUGE_W` fixo). Prefixo fixo: label("
/// "+12+" "=14). Contrato: NUNCA estourar a borda (era o off-by-1 do
/// primeiro draft, que cortava o sufixo no meio).
pub(super) fn derive_bar_width(content_width: u16, suffix_w: usize) -> usize {
    let label_w = 1 + LABEL_W + 1;
    (content_width as usize)
        .saturating_sub(label_w + suffix_w)
        .max(10)
}

/// Trunca um nome pra no máximo `max` colunas, usando `…` no lugar do
/// último caractere cortado — NUNCA corte seco (contrato do brief; era o
/// bug que produzia "Free Tie" a partir de "Free Tier").
pub(super) fn truncate_name(name: &str, max: usize) -> String {
    if name.chars().count() <= max {
        name.to_string()
    } else {
        let head: String = name.chars().take(max.saturating_sub(1)).collect();
        format!("{head}\u{2026}")
    }
}

/// Tokens totais de um `ModelUsage` (todas as 4 categorias — mesma
/// convenção do bucket horário em `usage::buckets::bucket_by_model_hour`,
/// que alimenta o chart da seção 2; mantém as duas seções coerentes entre
/// si mesmo divergindo do bucket diário de `render/history.rs`, que soma
/// só input+output).
pub(super) fn model_tokens(mu: &ModelUsage) -> u64 {
    mu.input + mu.output + mu.cache_read + mu.cache_write
}

/// Encontra um ModelUsage cujo nome contem `quota_name` (case-insensitive).
/// Necessario porque o nome no quota (ex "Opus") e curto, enquanto o nome no
/// usage engine e completo (ex "claude-opus-4-8").
pub(super) fn find_model_usage<'a>(
    by_model: &'a [ModelUsage],
    quota_name: &str,
) -> Option<&'a ModelUsage> {
    let lower = quota_name.to_lowercase();
    by_model
        .iter()
        .find(|mu| mu.model.to_lowercase().contains(&lower))
}

/// Formats a reset time string from an ISO timestamp or raw string.
/// Extracts HH:MM if ISO, else returns raw string or "-".
pub(super) fn fmt_reset(resets_at: Option<&str>) -> String {
    match resets_at {
        None => "-".to_string(),
        Some(s) => s
            .split('T')
            .nth(1)
            .and_then(|t| t.get(..5))
            .map(|hm| hm.to_string())
            .unwrap_or_else(|| s.to_string()),
    }
}

/// Custo/crédito "de hoje" de um provider (Amp mostra crédito restante, os
/// demais mostram custo acumulado — mesma convenção de `dashboard.rs`).
pub(super) fn fmt_cost_generic(pu: &ProviderUsage) -> String {
    if pu.provider == "amp" {
        return pu
            .amp_dollars
            .as_ref()
            .and_then(|ad| ad.remaining)
            .map(|r| format!("cr ${r:.2}"))
            .unwrap_or_else(|| "-".to_string());
    }
    match &pu.cost {
        Some(c) => format!("${:.2}", c.usd),
        None => "-".to_string(),
    }
}
