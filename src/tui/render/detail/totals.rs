//! Seção 4: Totais (hoje + 7 dias).

use ratatui::style::Style;
use ratatui::text::{Line, Span};

use crate::theme::ColorToken;
use crate::tui::state::AppState;
use crate::tui::theme_bridge::to_ratatui;
use crate::tui::widgets::column_chart::fmt_tokens_short;
use crate::usage::pricing::cost_usd_of;
use crate::usage::{ProviderUsage, UsageRecord};

use super::format::{fmt_cost_generic, provider_usage_tokens};

/// Linha de totais: "hoje" vem de `state.usage` (já agregado pelo engine);
/// "7 dias" soma `state.history` filtrado por provider (records brutos —
/// `state.usage` não cobre a janela de 7d).
pub(super) fn totals_line(
    state: &AppState,
    provider_usage: Option<&ProviderUsage>,
    provider: &str,
) -> Line<'static> {
    // Cada metade tem seu próprio sinal de loading: "hoje" vem do
    // UsageComputed (`state.usage`), "7 dias" do HistoryLoaded
    // (`state.history`) — eles chegam em momentos diferentes. Enquanto o
    // respectivo dado não chegou, a metade diz "coletando…" em vez de
    // afirmar zero (regressão "hoje 0 tok").
    let today_str = if state.usage.is_none() {
        "coletando\u{2026}".to_string()
    } else {
        let (today_tokens, today_cost) = match provider_usage {
            Some(pu) => (provider_usage_tokens(pu), fmt_cost_generic(pu)),
            None => (0, "-".to_string()),
        };
        format!(
            "{} tok \u{b7} {}",
            fmt_tokens_short(today_tokens),
            today_cost
        )
    };

    if state.history.is_none() {
        return Line::from(Span::styled(
            format!(" hoje {today_str}    7 dias coletando\u{2026}"),
            Style::default().fg(to_ratatui(ColorToken::TextBright)),
        ));
    }

    let week_records: Vec<&UsageRecord> = state
        .history
        .as_deref()
        .unwrap_or(&[])
        .iter()
        .filter(|r| r.provider == provider)
        .collect();
    let week_tokens: u64 = week_records
        .iter()
        .map(|r| r.input + r.output + r.cache_read + r.cache_write)
        .sum();
    let week_cost: Option<f64> = week_records
        .iter()
        .fold(None, |acc, r| match cost_usd_of(r) {
            Some(c) => Some(acc.unwrap_or(0.0) + c),
            None => acc,
        });
    let week_cost_str = week_cost
        .map(|c| format!("${c:.2}"))
        .unwrap_or_else(|| "-".to_string());

    Line::from(Span::styled(
        format!(
            " hoje {}    7 dias {} tok \u{b7} {}",
            today_str,
            fmt_tokens_short(week_tokens),
            week_cost_str
        ),
        Style::default().fg(to_ratatui(ColorToken::TextBright)),
    ))
}
