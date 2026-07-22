//! Shared format helpers for Detail sections (label width, gauges, costs).

use crate::formatters::clock::Clock;
use crate::formatters::shared::{format_eta, parse_iso};
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
pub(super) const WINDOW_SUFFIX_W: usize = 1 + 4 + 1 + 2 + 1 + 20; // pct(" NNN%"=6) + reset("  → "+countdown até "99d 23h · qua 23:59"=20)
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

/// Abreviação PT (3 letras) do weekday — usada só quando o reset cai
/// em dia local diferente de "agora" (janela de 7 dias, tipicamente).
fn weekday_pt(w: time::Weekday) -> &'static str {
    use time::Weekday::*;
    match w {
        Monday => "seg",
        Tuesday => "ter",
        Wednesday => "qua",
        Thursday => "qui",
        Friday => "sex",
        Saturday => "s\u{e1}b",
        Sunday => "dom",
    }
}

/// Countdown + horário de reset em fuso LOCAL — substitui o slice cru
/// de UTC (bug da auditoria: TUI mostrava o reset em UTC). "Xh Ym ·
/// HH:MM" no mesmo dia local; "{d}d {h}h · seg HH:MM" (weekday
/// abreviado) quando o reset cai em outro dia local (janela de 7
/// dias). `Full`/`?` (via `format_eta`) passam direto — sem hora.
pub(super) fn fmt_reset(clock: &Clock, resets_at: Option<&str>, remaining: f64) -> String {
    let eta = format_eta(clock, resets_at, remaining);
    if eta == "Full" || eta == "?" {
        return eta;
    }
    let Some(iso) = resets_at else {
        return eta;
    };
    let Some(dt) = parse_iso(iso) else {
        return eta;
    };
    let local = dt.to_offset(clock.local_offset);
    let today_local = clock.now.to_offset(clock.local_offset).date();
    if local.date() == today_local {
        format!("{eta} \u{b7} {:02}:{:02}", local.hour(), local.minute())
    } else {
        format!(
            "{eta} \u{b7} {} {:02}:{:02}",
            weekday_pt(local.weekday()),
            local.hour(),
            local.minute()
        )
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
