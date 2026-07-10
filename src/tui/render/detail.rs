//! Tela Detalhe: dados reais por provider (Task 12). Substitui o antigo
//! placeholder hardcoded `tokens/h ▁▂▃▅▇▆▄▂▁` (idêntico entre providers,
//! nunca refletia dado real) por 6 seções alinhadas na mesma coluna de
//! gauge: janelas (sessão/semana/modelos), modelos hoje (tokens+custo),
//! sparkline real de 24h, extra usage (spend novo do Claude), totais
//! (hoje/7 dias) e chips de ação.

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph, Wrap};
use ratatui::Frame;

use crate::providers::extras::get_claude_extra;
use crate::providers::types::{ExtraUsage, ProviderQuota, QuotaWindow};
use crate::settings::GlyphMode;
use crate::theme::ColorToken;
use crate::tui::login_state::{login_state_for, LoginState};
use crate::tui::mouse::{ChipKind, HitMap};
use crate::tui::render::shared::{abbrev_tokens, series_now};
use crate::tui::state::AppState;
use crate::tui::theme_bridge::{provider_color, to_ratatui};
use crate::tui::widgets::chips::{chips_line, register_chip_hits};
use crate::tui::widgets::icons::{glyph, Icon};
use crate::tui::widgets::quota_gauge::{gauge_spans, pulse_color};
use crate::tui::widgets::severity::{severity_color as sev_color, severity_color_api};
use crate::tui::widgets::sparkline::sparkline_str_wide;
use crate::usage::buckets::provider_series_24h;
use crate::usage::pricing::cost_usd_of;
use crate::usage::{ModelUsage, ProviderUsage, UsageRecord};

/// Largura do rótulo (janela/modelo) — MESMA coluna em toda seção com gauge
/// (contrato do brief: "todas alinhadas na mesma coluna de gauge"). 12 =
/// o limite de `truncate_name`, então um nome truncado nunca estoura a
/// coluna do gauge.
const LABEL_W: usize = 12;

/// Largura fixa dos gauges de "Modelos hoje" e "extra usage" (contrato do
/// brief — ao contrário da janela de sessão/semana, não deriva da área).
const FIXED_GAUGE_W: usize = 20;

/// Deriva a largura do gauge de janelas a partir da área real do conteúdo.
/// Prefixo fixo: label(" "+12+" "=14) + pct(" "+4+"%"=6) + reset("  → "+HH:MM=9) = 29.
/// Contrato: NUNCA estourar a borda (era o off-by-1 do primeiro draft, que
/// cortava o reset no meio: "→ 02:3" em vez de "→ 02:39").
fn derive_bar_width(content_width: u16) -> usize {
    let label_w = 1 + LABEL_W + 1;
    let pct_w = 1 + 4 + 1;
    let reset_w = 2 + 1 + 1 + 5;
    (content_width as usize)
        .saturating_sub(label_w + pct_w + reset_w)
        .max(10)
}

/// Trunca um nome pra no máximo `max` colunas, usando `…` no lugar do
/// último caractere cortado — NUNCA corte seco (contrato do brief; era o
/// bug que produzia "Free Tie" a partir de "Free Tier").
fn truncate_name(name: &str, max: usize) -> String {
    if name.chars().count() <= max {
        name.to_string()
    } else {
        let head: String = name.chars().take(max.saturating_sub(1)).collect();
        format!("{head}\u{2026}")
    }
}

/// Tokens totais de um `ModelUsage` (todas as 4 categorias — mesma
/// convenção do bucket horário em `usage::buckets::bucket_by_hour`, que
/// alimenta o sparkline da seção 3; mantém as duas seções coerentes entre
/// si mesmo divergindo do bucket diário de `render/history.rs`, que soma
/// só input+output).
fn model_tokens(mu: &ModelUsage) -> u64 {
    mu.input + mu.output + mu.cache_read + mu.cache_write
}

/// Tokens totais de um `ProviderUsage` (mesma convenção de `model_tokens`).
fn provider_usage_tokens(pu: &ProviderUsage) -> u64 {
    pu.total_input + pu.total_output + pu.total_cache_read + pu.total_cache_write
}

/// Encontra um ModelUsage cujo nome contem `quota_name` (case-insensitive).
/// Necessario porque o nome no quota (ex "Opus") e curto, enquanto o nome no
/// usage engine e completo (ex "claude-opus-4-8").
fn find_model_usage<'a>(by_model: &'a [ModelUsage], quota_name: &str) -> Option<&'a ModelUsage> {
    let lower = quota_name.to_lowercase();
    by_model
        .iter()
        .find(|mu| mu.model.to_lowercase().contains(&lower))
}

/// Formats a reset time string from an ISO timestamp or raw string.
/// Extracts HH:MM if ISO, else returns raw string or "-".
fn fmt_reset(resets_at: Option<&str>) -> String {
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
fn fmt_cost_generic(pu: &ProviderUsage) -> String {
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

// ---------------------------------------------------------------------------
// Seção 1: Janelas (sessão/semana/modelos)
// ---------------------------------------------------------------------------

/// Uma linha de janela (sessão/semana/modelo): label(12) + gauge + pct(4) +
/// reset. Cor vem da severidade da API quando presente (spec §4.1),
/// fallback pro threshold local. `anim_frame`/`animations` (Task 16): pulso
/// crítico quando `remaining < 10.0` — coexiste com o blink da sidebar
/// (Task 10), não o substitui.
fn window_line(
    label: &str,
    w: &QuotaWindow,
    gauge_w: usize,
    anim_frame: u64,
    animations: bool,
) -> Line<'static> {
    let mut color = severity_color_api(w.severity.as_deref(), Some(w.remaining));
    if animations && w.remaining < 10.0 {
        color = pulse_color(color, anim_frame);
    }
    let reset_str = fmt_reset(w.resets_at.as_deref());
    let name = truncate_name(label, LABEL_W);
    let mut spans = vec![Span::styled(
        format!(" {name:<LABEL_W$} "),
        Style::default().fg(to_ratatui(ColorToken::Muted)),
    )];
    spans.extend(gauge_spans(w.remaining, gauge_w, color));
    spans.push(Span::styled(
        format!(" {:>4.0}%", w.remaining),
        Style::default().fg(to_ratatui(ColorToken::TextBright)),
    ));
    spans.push(Span::styled(
        format!("  \u{2192} {reset_str}"),
        Style::default().fg(to_ratatui(ColorToken::Comment)),
    ));
    Line::from(spans)
}

/// Linha de modelo (q.models): igual a `window_line`, com custo do dia
/// anexado quando `find_model_usage` acha o modelo correspondente em
/// `provider_usage.by_model` — mas SÓ se couber em `content_width` (nunca
/// estoura a borda; opcional > alinhamento). Modelos de sessão semanal do
/// Claude (ex. "Opus") batem por substring com o id completo do usage
/// engine (ex. "claude-opus-4-8"), então isto é o caminho comum, não raro.
fn model_window_line(
    name: &str,
    w: &QuotaWindow,
    gauge_w: usize,
    content_width: u16,
    provider_usage: Option<&ProviderUsage>,
    anim_frame: u64,
    animations: bool,
) -> Line<'static> {
    let mut line = window_line(name, w, gauge_w, anim_frame, animations);
    if let Some(cost) = provider_usage
        .and_then(|pu| find_model_usage(&pu.by_model, name))
        .and_then(|mu| mu.cost.as_ref())
    {
        let cost_span = format!("  ${:.2}", cost.usd);
        let current_w: usize = line.spans.iter().map(|s| s.content.chars().count()).sum();
        if current_w + cost_span.chars().count() <= content_width as usize {
            line.spans.push(Span::styled(
                cost_span,
                Style::default().fg(to_ratatui(ColorToken::Comment)),
            ));
        }
    }
    line
}

// ---------------------------------------------------------------------------
// Seção 2: Modelos hoje (tokens + custo, de provider_usage.by_model)
// ---------------------------------------------------------------------------

/// Uma linha de "Modelos hoje": label(12) + barra PROPORCIONAL a tokens
/// (não a 100% — normalizada pelo modelo de maior consumo) + tokens
/// abreviados + custo, ambos right-aligned.
fn model_usage_line(mu: &ModelUsage, max_tokens: u64, brand: Color) -> Line<'static> {
    let tokens = model_tokens(mu);
    let pct = if max_tokens > 0 {
        (tokens as f64 / max_tokens as f64 * 100.0).clamp(0.0, 100.0)
    } else {
        0.0
    };
    let name = truncate_name(&mu.model, LABEL_W);
    let cost_str = match &mu.cost {
        Some(c) => format!("${:.2}", c.usd),
        None => "-".to_string(),
    };
    let mut spans = vec![Span::styled(
        format!(" {name:<LABEL_W$} "),
        Style::default().fg(to_ratatui(ColorToken::Text)),
    )];
    spans.extend(gauge_spans(pct, FIXED_GAUGE_W, brand));
    spans.push(Span::styled(
        format!(" {:>8}", abbrev_tokens(tokens)),
        Style::default().fg(to_ratatui(ColorToken::Muted)),
    ));
    spans.push(Span::styled(
        format!(" {cost_str:>9}"),
        Style::default().fg(to_ratatui(ColorToken::Comment)),
    ));
    Line::from(spans)
}

// ---------------------------------------------------------------------------
// Seção 3: tokens/h 24h (sparkline real — mata o placeholder hardcoded)
// ---------------------------------------------------------------------------

/// Linha "tokens/h 24h": sparkline real (`provider_series_24h`) + hora de
/// pico (índice do máximo → hora LOCAL). `now` (de `series_now`) carrega
/// qualquer offset que sua fonte tiver (ex. `state.history[].ts`, tipicamente
/// UTC) — `now.hour()` sozinho mentiria "local" enquanto devolve UTC. Fix:
/// converte pro offset do relógio local (`state.local_offset`, T12) ANTES de
/// extrair a hora; a subtração de `hours_back` é invariante a essa conversão
/// (mesmo instante, offset fixo, sem DST neste app). `now` NUNCA é
/// `OffsetDateTime::now_utc()` — vem de `series_now` (mesma âncora do
/// Overview, Task 11), garantindo render puro/determinístico p/ snapshot.
/// Sem âncora OU sem uso na janela → placeholder textual (nunca inventa
/// pico sobre dado inexistente).
fn spark_line(state: &AppState, provider: &str, content_width: u16) -> Line<'static> {
    // `history=None` = parse dos session logs em voo — dizer "sem uso"
    // aqui seria afirmar zero sobre dado que só não chegou (regressão
    // "hoje 0 tok" da máquina real).
    if state.history.is_none() {
        return Line::from(Span::styled(
            " tokens/h  coletando hist\u{f3}rico\u{2026}",
            Style::default().fg(to_ratatui(ColorToken::Muted)),
        ));
    }
    let now = series_now(state);
    let series: Vec<u64> = match now {
        Some(n) => state
            .history
            .as_deref()
            .map(|r| provider_series_24h(r, provider, n))
            .unwrap_or_default(),
        None => Vec::new(),
    };

    let has_data = series.iter().any(|&v| v > 0);
    if let Some(now) = now {
        if has_data {
            if let Some((idx, &peak)) = series.iter().enumerate().max_by_key(|&(_, &v)| v) {
                let hours_back = (series.len() - 1 - idx) as i64;
                let local_now = now.to_offset(state.local_offset);
                let peak_hour = (local_now.hour() as i64 - hours_back).rem_euclid(24);
                let prefix = " tokens/h  ";
                let suffix = format!("  pico {peak_hour:02}h: {}", abbrev_tokens(peak));
                // Largura derivada do prefixo/sufixo REAIS (não uma constante
                // mágica) — um valor de pico maior (ex. "999.9M") nunca pode
                // estourar a borda, contrato geral desta tela (T12).
                let reserved = prefix.chars().count() + suffix.chars().count();
                let spark_width = (content_width as usize).saturating_sub(reserved).max(1);
                return Line::from(vec![
                    Span::styled(prefix, Style::default().fg(to_ratatui(ColorToken::Muted))),
                    Span::styled(
                        sparkline_str_wide(&series, spark_width),
                        Style::default().fg(to_ratatui(ColorToken::Comment)),
                    ),
                    Span::styled(suffix, Style::default().fg(to_ratatui(ColorToken::Muted))),
                ]);
            }
        }
    }
    Line::from(Span::styled(
        " tokens/h  sem uso nas \u{fa}ltimas 24h",
        Style::default().fg(to_ratatui(ColorToken::Muted)),
    ))
}

// ---------------------------------------------------------------------------
// Seção 4: extra usage (só Claude — spend novo, Task 1)
// ---------------------------------------------------------------------------

/// Linha "extra usage": `enabled=false` → texto fixo "desativado";
/// `enabled=true && limit<=0.0` → sentinel de "sem limite configurado"
/// (`extra_usage_from_spend` em `claude.rs`) → só o valor gasto, SEM gauge
/// (não há teto p/ dar proporção — gauge 100%+"de $0.00" era
/// autocontraditório); `enabled=true && limit>0.0` → gauge(20) + "$used de
/// $limit". Mesma coluna de label das demais seções. Só Claude tem esta
/// seção (Amp/Codex têm extras próprios, sem overlap com
/// `ClaudeQuotaExtra.extra_usage`); providers sem extra omitem a linha
/// inteira (chamador não invoca esta função).
fn extra_usage_line(eu: &ExtraUsage) -> Line<'static> {
    let label = Span::styled(
        format!(" {:<LABEL_W$} ", "extra usage"),
        Style::default().fg(to_ratatui(ColorToken::Muted)),
    );
    if !eu.enabled {
        return Line::from(vec![
            label,
            Span::styled(
                "desativado",
                Style::default().fg(to_ratatui(ColorToken::Muted)),
            ),
        ]);
    }
    if eu.limit <= 0.0 {
        return Line::from(vec![
            label,
            Span::styled(
                format!("${:.2} usado", eu.used),
                Style::default().fg(to_ratatui(ColorToken::Yellow)),
            ),
            Span::styled(
                " \u{b7} sem limite",
                Style::default().fg(to_ratatui(ColorToken::Comment)),
            ),
        ]);
    }
    // `eu.limit > 0.0` é garantido pelo early-return acima (limit <= 0.0
    // já saiu com o texto "sem limite") — divisão segura.
    let pct_used = (eu.used / eu.limit * 100.0).clamp(0.0, 100.0);
    let remaining_pct = 100.0 - pct_used;
    let color = sev_color(Some(remaining_pct));
    let mut spans = vec![label];
    spans.extend(gauge_spans(remaining_pct, FIXED_GAUGE_W, color));
    spans.push(Span::styled(
        format!("  ${:.2} de ${:.2}", eu.used, eu.limit),
        Style::default().fg(to_ratatui(ColorToken::TextBright)),
    ));
    Line::from(spans)
}

// ---------------------------------------------------------------------------
// Seção 5: Totais (hoje + 7 dias)
// ---------------------------------------------------------------------------

/// Linha de totais: "hoje" vem de `state.usage` (já agregado pelo engine);
/// "7 dias" soma `state.history` filtrado por provider (records brutos —
/// `state.usage` não cobre a janela de 7d).
fn totals_line(
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
        format!("{} tok \u{b7} {}", abbrev_tokens(today_tokens), today_cost)
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
            abbrev_tokens(week_tokens),
            week_cost_str
        ),
        Style::default().fg(to_ratatui(ColorToken::TextBright)),
    ))
}

// ---------------------------------------------------------------------------
// Estados especiais (deslogado / erro)
// ---------------------------------------------------------------------------

/// CTA em tela cheia (provider sem sessão) — igual em espírito ao card do
/// Overview, mas com instrução maior (a tela toda é deste provider, não
/// precisa caber em 1 linha).
fn render_logged_out(q: &ProviderQuota, frame: &mut Frame, area: Rect) {
    let lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            format!(" {} \u{2014} sem sess\u{e3}o", q.display_name),
            Style::default()
                .fg(to_ratatui(ColorToken::TextBright))
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            " Nenhuma credencial v\u{e1}lida encontrada para este provider.",
            Style::default().fg(to_ratatui(ColorToken::Muted)),
        )),
        Line::from(Span::styled(
            " Pressione [g] ou clique no chip \"login\" abaixo para autenticar.",
            Style::default().fg(to_ratatui(ColorToken::Muted)),
        )),
    ];
    let p = Paragraph::new(lines).wrap(Wrap { trim: false });
    frame.render_widget(p, area);
}

/// Mensagem de erro tipado (falha não-auth: parse/rede/API) com ícone —
/// NUNCA tela branca. `q.error` é a string verbatim do provider (contrato,
/// ver `providers::error`).
fn render_error(q: &ProviderQuota, mode: GlyphMode, frame: &mut Frame, area: Rect) {
    let msg = q.error.as_deref().unwrap_or("Erro desconhecido");
    let lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            format!(" {} Erro ao carregar dados", glyph(Icon::Warn, mode)),
            Style::default()
                .fg(to_ratatui(ColorToken::Red))
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            format!(" {msg}"),
            Style::default().fg(to_ratatui(ColorToken::Muted)),
        )),
    ];
    let p = Paragraph::new(lines).wrap(Wrap { trim: false });
    frame.render_widget(p, area);
}

// ---------------------------------------------------------------------------
// Render principal
// ---------------------------------------------------------------------------

/// Corpo completo (estado Ok/Checking): as 6 seções do brief, nesta ordem.
fn render_full(
    state: &AppState,
    q: &ProviderQuota,
    brand: Color,
    provider_usage: Option<&ProviderUsage>,
    frame: &mut Frame,
    area: Rect,
) {
    let bar_width = derive_bar_width(area.width);
    let mut lines: Vec<Line<'_>> = Vec::new();

    // 1. Janelas: sessão/semana + 1 linha por q.models (nome real da API).
    // MESMO bar_width em todas — a coluna de gauge tem que alinhar entre
    // sessão/semana/modelos (contrato do brief).
    if let Some(primary) = &q.primary {
        lines.push(window_line(
            "sessão",
            primary,
            bar_width,
            state.anim_frame,
            state.animations,
        ));
    }
    if let Some(secondary) = &q.secondary {
        lines.push(window_line(
            "semana",
            secondary,
            bar_width,
            state.anim_frame,
            state.animations,
        ));
    }
    if let Some(models) = &q.models {
        for (name, w) in models {
            lines.push(model_window_line(
                name,
                w,
                bar_width,
                area.width,
                provider_usage,
                state.anim_frame,
                state.animations,
            ));
        }
    }

    // 2. Modelos hoje: barra proporcional a tokens (não a 100%) + custo.
    if let Some(pu) = provider_usage {
        if !pu.by_model.is_empty() {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                " Modelos hoje",
                Style::default()
                    .fg(to_ratatui(ColorToken::TextBright))
                    .add_modifier(Modifier::BOLD),
            )));
            let max_tokens = pu
                .by_model
                .iter()
                .map(model_tokens)
                .max()
                .unwrap_or(0)
                .max(1);
            for mu in &pu.by_model {
                lines.push(model_usage_line(mu, max_tokens, brand));
            }
        }
    }

    // 3. tokens/h 24h: sparkline real (NUNCA o placeholder morto — T12).
    lines.push(Line::from(""));
    lines.push(spark_line(state, &q.provider, area.width));

    // 4. extra usage (só Claude — spend novo da Task 1; demais omitem).
    if let Some(eu) = get_claude_extra(q).and_then(|c| c.extra_usage.as_ref()) {
        lines.push(extra_usage_line(eu));
    }

    // 5. Totais: hoje (state.usage) + 7 dias (state.history somado).
    lines.push(Line::from(""));
    lines.push(totals_line(state, provider_usage, &q.provider));

    frame.render_widget(Paragraph::new(lines), area);
}

/// Chips centrados: `[esc voltar] [r atualizar] [g login] [h histórico]`.
/// As 4 teclas já são globais (`update.rs`) — os chips só tornam a ação
/// clicável/visível, não introduzem comportamento novo.
fn render_footer_chips(frame: &mut Frame, area: Rect, hits: &mut HitMap) {
    let chips: [(ChipKind, &str, &str); 4] = [
        (ChipKind::Back, "esc", "voltar"),
        (ChipKind::Refresh, "r", "atualizar"),
        (ChipKind::Login, "g", "login"),
        (ChipKind::History, "h", "hist\u{f3}rico"),
    ];
    let line = chips_line(&chips, area.width);
    frame.render_widget(Paragraph::new(line), area);
    register_chip_hits(&chips, area, hits);
}

/// Renders the Detail view for the selected provider (Screen::Detail).
pub fn render_detail(state: &AppState, frame: &mut Frame, area: Rect, hits: &mut HitMap) {
    let provider = match state.providers.get(state.selected) {
        Some(pv) => pv,
        None => return render_empty(frame, area),
    };
    let q = &provider.quota;
    let p_color = provider_color(&q.provider);
    let fetch_pending = state.fetch_pending.iter().any(|p| p == &q.provider);
    let logged = login_state_for(Some(q), fetch_pending);

    // Title: "Name · Plan" or just "Name"
    let title = match &q.plan {
        Some(plan) => format!(" {} \u{b7} {} ", q.display_name, plan),
        None => format!(" {} ", q.display_name),
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Thick)
        .border_style(Style::default().fg(p_color))
        .title(Span::styled(
            title,
            Style::default()
                .fg(to_ratatui(ColorToken::TextBright))
                .add_modifier(Modifier::BOLD),
        ));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // [conteúdo | chips] — chips sempre presentes, em qualquer estado.
    let vert = Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).split(inner);
    let content_area = vert[0];
    let footer_area = vert[1];

    match logged {
        // Deslogado: CTA em tela cheia (nunca a lista de seções vazia).
        LoginState::LoggedOut => render_logged_out(q, frame, content_area),
        // Falha não-auth (parse/rede/API): mensagem tipada + ícone — nunca
        // branco, nunca induz re-login (spec §10).
        LoginState::NoToken | LoginState::Error => {
            render_error(q, state.glyph_mode, frame, content_area)
        }
        LoginState::Ok | LoginState::Checking => {
            let provider_usage: Option<&ProviderUsage> = state
                .usage
                .as_ref()
                .and_then(|s| s.providers.iter().find(|pu| pu.provider == q.provider));
            render_full(state, q, p_color, provider_usage, frame, content_area);
        }
    }

    render_footer_chips(frame, footer_area, hits);
}

/// Fallback when no provider is selected (state.providers vazio — nunca
/// aconteceu fetch algum, distinto de LoginState::LoggedOut).
fn render_empty(frame: &mut Frame, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Thick)
        .border_style(Style::default().fg(to_ratatui(ColorToken::Comment)));

    let para = Paragraph::new(Span::styled(
        " Nenhum provider selecionado",
        Style::default().fg(to_ratatui(ColorToken::Muted)),
    ))
    .block(block);

    frame.render_widget(para, area);
}

#[cfg(test)]
mod tests {
    use indexmap::IndexMap;

    use crate::providers::types::{
        AmpQuotaExtra, ClaudeQuotaExtra, ExtraUsage, ProviderExtra, ProviderQuota, QuotaWindow,
    };
    use crate::tui::mouse::HitMap;
    use crate::tui::render::render;
    use crate::tui::state::{AppState, FetchStatus, ProviderView, Screen};
    use crate::usage::amp::AmpDollars;
    use crate::usage::{Cost, ModelUsage, ProviderUsage, UsageRecord, UsageSummary};

    use super::{render_detail, truncate_name};

    fn window(remaining: f64, resets_at: Option<&str>, severity: Option<&str>) -> QuotaWindow {
        QuotaWindow {
            remaining,
            resets_at: resets_at.map(|s| s.to_string()),
            window_minutes: Some(300),
            used: Some(100.0 - remaining),
            severity: severity.map(|s| s.to_string()),
        }
    }

    fn rec(provider: &str, model: &str, ts: time::OffsetDateTime, tokens: u64) -> UsageRecord {
        UsageRecord {
            provider: provider.into(),
            model: Some(model.into()),
            input: tokens,
            output: 0,
            cache_read: 0,
            cache_write: 0,
            ts,
            session_id: None,
            project: None,
        }
    }

    // -----------------------------------------------------------------
    // Unit tests: truncate_name (abbrev_tokens moveu para render/shared.rs, T13)
    // -----------------------------------------------------------------

    #[test]
    fn truncate_name_keeps_short_names_intact() {
        // Regressão do bug "Free Tie": "Free Tier" (9 chars) cabe em 12 —
        // NUNCA deve ser cortado.
        assert_eq!(truncate_name("Free Tier", 12), "Free Tier");
        assert_eq!(truncate_name("Fable", 12), "Fable");
    }

    #[test]
    fn truncate_name_ellipsizes_long_names() {
        let out = truncate_name("claude-opus-4-8-extended", 12);
        assert_eq!(out.chars().count(), 12);
        assert!(out.ends_with('\u{2026}'));
    }

    // -----------------------------------------------------------------
    // Unit tests: spark_line usa state.local_offset (review T12) — antes
    // `now.hour()` devolvia o offset que `now` já carregasse (tipicamente
    // UTC), rotulado como "hora local" sem nunca converter de fato.
    // -----------------------------------------------------------------

    fn spark_line_text(state: &AppState) -> String {
        let line = super::spark_line(state, "claude", 100);
        line.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    #[test]
    fn spark_line_peak_hour_uses_local_offset_not_utc() {
        let mut state = AppState::new();
        let now = time::macros::datetime!(2026-07-02 02:00:00 UTC);
        state.last_update = Some(now);
        state.local_offset = time::UtcOffset::from_hms(-3, 0, 0).unwrap();
        state.history = Some(vec![rec(
            "claude",
            "claude-opus-4-8",
            now - time::Duration::hours(1),
            900_000,
        )]);
        let text = spark_line_text(&state);
        // now (02:00 UTC) em -03:00 é 23h do dia anterior; o pico está 1h
        // atrás → 22h local (NÃO "01h", que seria o resultado se o offset
        // local fosse ignorado e a hora UTC crua vazasse pro label).
        assert!(
            text.contains("pico 22h:"),
            "esperava hora local (UTC-3 → 22h) na linha, obtido: {text:?}"
        );
    }

    #[test]
    fn spark_line_peak_hour_default_utc_offset_matches_snapshots() {
        let mut state = AppState::new();
        let now = time::macros::datetime!(2026-07-02 02:00:00 UTC);
        state.last_update = Some(now);
        // local_offset default (UTC, de AppState::new()) — confirma que os
        // snapshots existentes (todos com offset default) continuam
        // corretos depois do fix de conversão.
        state.history = Some(vec![rec(
            "claude",
            "claude-opus-4-8",
            now - time::Duration::hours(1),
            900_000,
        )]);
        let text = spark_line_text(&state);
        assert!(
            text.contains("pico 01h:"),
            "esperava hora UTC (offset default) 01h na linha, obtido: {text:?}"
        );
    }

    // -----------------------------------------------------------------
    // Loading vs zero (regressão "hoje 0 tok"): enquanto o parse dos
    // session logs não terminou (`state.usage`/`state.history` = None),
    // as linhas de uso devem dizer "coletando", nunca afirmar zero.
    // -----------------------------------------------------------------

    #[test]
    fn spark_line_loading_shows_coletando_not_sem_uso() {
        let mut state = AppState::new();
        state.last_update = Some(time::macros::datetime!(2026-07-02 12:00:00 UTC));
        state.history = None; // HistoryLoaded ainda não chegou
        let text = spark_line_text(&state);
        assert!(
            text.contains("coletando"),
            "history=None deve mostrar loading, obtido: {text:?}"
        );
        assert!(
            !text.contains("sem uso"),
            "history=None não pode afirmar 'sem uso': {text:?}"
        );
    }

    #[test]
    fn totals_line_loading_shows_coletando_not_zero() {
        let state = AppState::new(); // usage=None, history=None
        let line = super::totals_line(&state, None, "claude");
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(
            text.contains("coletando"),
            "usage/history=None deve mostrar loading, obtido: {text:?}"
        );
        assert!(
            !text.contains("0 tok"),
            "usage/history=None não pode afirmar '0 tok': {text:?}"
        );
    }

    // -----------------------------------------------------------------
    // Fixtures
    // -----------------------------------------------------------------

    fn make_claude_full() -> ProviderView {
        let mut models: IndexMap<String, QuotaWindow> = IndexMap::new();
        // "Opus" bate por substring com "claude-opus-4-8" (by_model) —
        // exercita o custo opcional anexado em `model_window_line`.
        models.insert("Opus".to_string(), window(60.0, None, Some("normal")));
        models.insert(
            "Fable".to_string(),
            window(97.0, Some("2026-07-03T22:59:59Z"), Some("normal")),
        );

        ProviderView::new(ProviderQuota {
            provider: "claude".to_string(),
            display_name: "Claude".to_string(),
            available: true,
            account: None,
            plan: Some("Max 5x".to_string()),
            plan_type: None,
            primary: Some(window(89.0, Some("2026-07-02T02:39:59Z"), Some("normal"))),
            secondary: Some(window(97.0, Some("2026-07-03T22:59:59Z"), Some("normal"))),
            models: Some(models),
            extra: Some(ProviderExtra::Claude(ClaudeQuotaExtra {
                weekly_models: None,
                extra_usage: Some(ExtraUsage {
                    enabled: true,
                    remaining: 87.66,
                    limit: 100.0,
                    used: 12.34,
                }),
            })),
            error: None,
        })
    }

    fn make_amp_full() -> ProviderView {
        let mut models: IndexMap<String, QuotaWindow> = IndexMap::new();
        models.insert("Free Tier".to_string(), window(70.0, None, None));
        models.insert("Credits".to_string(), window(100.0, None, None));

        ProviderView::new(ProviderQuota {
            provider: "amp".to_string(),
            display_name: "Amp".to_string(),
            available: true,
            account: Some("user@x.com".to_string()),
            plan: None,
            plan_type: None,
            primary: Some(window(70.0, None, None)),
            secondary: None,
            models: Some(models),
            extra: Some(ProviderExtra::Amp(AmpQuotaExtra { meta: None })),
            error: None,
        })
    }

    fn make_codex_logged_out() -> ProviderView {
        ProviderView::new(ProviderQuota {
            provider: "codex".to_string(),
            display_name: "Codex".to_string(),
            available: false,
            account: None,
            plan: None,
            plan_type: None,
            primary: None,
            secondary: None,
            models: None,
            extra: None,
            error: Some(
                "Not logged in. Open `agent-bar menu` and choose Provider login.".to_string(),
            ),
        })
    }

    /// UsageSummary com claude (2 modelos, 1 com custo conhecido) + amp
    /// (crédito, sem tokens).
    fn fake_usage() -> UsageSummary {
        UsageSummary {
            providers: vec![
                ProviderUsage {
                    provider: "claude".to_string(),
                    total_input: 1_000_000,
                    total_output: 200_000,
                    total_cache_read: 0,
                    total_cache_write: 0,
                    cost: Some(Cost {
                        usd: 2.10,
                        brl: 11.55,
                    }),
                    by_model: vec![
                        ModelUsage {
                            model: "claude-opus-4-8".to_string(),
                            input: 800_000,
                            output: 100_000,
                            cache_read: 0,
                            cache_write: 0,
                            cost: Some(Cost {
                                usd: 1.40,
                                brl: 7.70,
                            }),
                        },
                        ModelUsage {
                            model: "claude-sonnet-4-6".to_string(),
                            input: 200_000,
                            output: 100_000,
                            cache_read: 0,
                            cache_write: 0,
                            cost: None,
                        },
                    ],
                    amp_dollars: None,
                },
                ProviderUsage {
                    provider: "amp".to_string(),
                    total_input: 0,
                    total_output: 0,
                    total_cache_read: 0,
                    total_cache_write: 0,
                    cost: None,
                    by_model: vec![],
                    amp_dollars: Some(AmpDollars {
                        spent: Some(0.81),
                        remaining: Some(4.19),
                        total: Some(5.0),
                    }),
                },
            ],
            total_cost: Cost {
                usd: 2.10,
                brl: 11.55,
            },
            fx_rate: 5.50,
        }
    }

    // -----------------------------------------------------------------
    // Snapshots (brief Step 2)
    // -----------------------------------------------------------------

    #[test]
    fn detail_claude_full() {
        let backend = ratatui::backend::TestBackend::new(100, 32);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        let mut state = AppState::new();
        let now = time::macros::datetime!(2026-07-02 02:00:00 UTC);
        state.providers = vec![make_claude_full()];
        state.selected = 0;
        state.screen = Screen::Detail;
        state.status = FetchStatus::Loaded;
        // display_cost (T16): header agora mostra o count-up, não
        // usage.total_cost.usd direto — sem isto, o header ficaria em
        // "$0.00" (default de AppState::new()) em vez do custo real.
        let usage = fake_usage();
        state.display_cost = usage.total_cost.usd;
        state.usage = Some(usage);
        state.last_update = Some(now);
        state.history = Some(vec![
            rec(
                "claude",
                "claude-opus-4-8",
                now - time::Duration::hours(1),
                900_000,
            ),
            rec(
                "claude",
                "claude-opus-4-8",
                now - time::Duration::hours(5),
                100_000,
            ),
            rec(
                "claude",
                "claude-sonnet-4-6",
                now - time::Duration::hours(9),
                50_000,
            ),
            rec("codex", "gpt-5.5", now - time::Duration::hours(2), 700_000),
        ]);
        terminal
            .draw(|f| render(&state, f, &mut HitMap::default()))
            .unwrap();
        insta::assert_snapshot!(terminal.backend());
    }

    #[test]
    fn detail_amp_credits() {
        let backend = ratatui::backend::TestBackend::new(100, 32);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        let mut state = AppState::new();
        let now = time::macros::datetime!(2026-07-02 02:00:00 UTC);
        state.providers = vec![make_amp_full()];
        state.selected = 0;
        state.screen = Screen::Detail;
        state.status = FetchStatus::Loaded;
        // display_cost (T16): header agora mostra o count-up, não
        // usage.total_cost.usd direto — sem isto, o header ficaria em
        // "$0.00" (default de AppState::new()) em vez do custo real.
        let usage = fake_usage();
        state.display_cost = usage.total_cost.usd;
        state.usage = Some(usage);
        state.last_update = Some(now);
        // Amp nunca gera UsageRecord (sem tracking de token) — history só
        // tem claude, então o sparkline do Amp deve cair no placeholder
        // "sem uso", DIFERENTE do sparkline real do Claude.
        state.history = Some(vec![rec(
            "claude",
            "claude-opus-4-8",
            now - time::Duration::hours(1),
            900_000,
        )]);
        terminal
            .draw(|f| render(&state, f, &mut HitMap::default()))
            .unwrap();
        insta::assert_snapshot!(terminal.backend());
    }

    #[test]
    fn detail_codex_logged_out() {
        let backend = ratatui::backend::TestBackend::new(100, 32);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        let mut state = AppState::new();
        state.providers = vec![make_codex_logged_out()];
        state.selected = 0;
        state.screen = Screen::Detail;
        state.status = FetchStatus::Loaded;
        terminal
            .draw(|f| render(&state, f, &mut HitMap::default()))
            .unwrap();
        insta::assert_snapshot!(terminal.backend());
    }

    #[test]
    fn detail_narrow_80() {
        // Largura 80 == NARROW_WIDTH: sidebar ainda expandida (colapsa só
        // abaixo de 80), então o conteúdo do Detail fica bem apertado —
        // verifica que gauges/sparkline/totais não colidem nem estouram.
        let backend = ratatui::backend::TestBackend::new(80, 32);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        let mut state = AppState::new();
        let now = time::macros::datetime!(2026-07-02 02:00:00 UTC);
        state.providers = vec![make_claude_full()];
        state.selected = 0;
        state.screen = Screen::Detail;
        state.status = FetchStatus::Loaded;
        // display_cost (T16): header agora mostra o count-up, não
        // usage.total_cost.usd direto — sem isto, o header ficaria em
        // "$0.00" (default de AppState::new()) em vez do custo real.
        let usage = fake_usage();
        state.display_cost = usage.total_cost.usd;
        state.usage = Some(usage);
        state.last_update = Some(now);
        state.history = Some(vec![rec(
            "claude",
            "claude-opus-4-8",
            now - time::Duration::hours(3),
            900_000,
        )]);
        terminal
            .draw(|f| render(&state, f, &mut HitMap::default()))
            .unwrap();
        insta::assert_snapshot!(terminal.backend());
    }

    // -----------------------------------------------------------------
    // Cobertura extra: extra usage desativado + estado de erro tipado
    // -----------------------------------------------------------------

    #[test]
    fn detail_extra_usage_disabled() {
        let mut pv = make_claude_full();
        pv.quota.extra = Some(ProviderExtra::Claude(ClaudeQuotaExtra {
            weekly_models: None,
            extra_usage: Some(ExtraUsage {
                enabled: false,
                remaining: 0.0,
                limit: 0.0,
                used: 0.0,
            }),
        }));

        let backend = ratatui::backend::TestBackend::new(100, 32);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        let mut state = AppState::new();
        state.providers = vec![pv];
        state.selected = 0;
        state.screen = Screen::Detail;
        state.status = FetchStatus::Loaded;
        terminal
            .draw(|f| render(&state, f, &mut HitMap::default()))
            .unwrap();
        insta::assert_snapshot!(terminal.backend());
    }

    #[test]
    fn detail_extra_usage_no_limit() {
        // Sentinel: enabled:true + limit<=0.0 ("sem teto configurado" — ver
        // extra_usage_from_spend em claude.rs). Sem gauge, só o valor gasto.
        let mut pv = make_claude_full();
        pv.quota.extra = Some(ProviderExtra::Claude(ClaudeQuotaExtra {
            weekly_models: None,
            extra_usage: Some(ExtraUsage {
                enabled: true,
                remaining: 0.0,
                limit: 0.0,
                used: 12.34,
            }),
        }));

        let backend = ratatui::backend::TestBackend::new(100, 32);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        let mut state = AppState::new();
        state.providers = vec![pv];
        state.selected = 0;
        state.screen = Screen::Detail;
        state.status = FetchStatus::Loaded;
        terminal
            .draw(|f| render(&state, f, &mut HitMap::default()))
            .unwrap();
        insta::assert_snapshot!(terminal.backend());
    }

    #[test]
    fn detail_provider_error_shows_icon_and_message() {
        let pv = ProviderView::new(ProviderQuota {
            provider: "codex".to_string(),
            display_name: "Codex".to_string(),
            available: false,
            account: None,
            plan: None,
            plan_type: None,
            primary: None,
            secondary: None,
            models: None,
            extra: None,
            error: Some("Failed to fetch Codex usage".to_string()),
        });

        let backend = ratatui::backend::TestBackend::new(100, 32);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        let mut state = AppState::new();
        state.providers = vec![pv];
        state.selected = 0;
        state.screen = Screen::Detail;
        state.status = FetchStatus::Loaded;
        terminal
            .draw(|f| render(&state, f, &mut HitMap::default()))
            .unwrap();
        insta::assert_snapshot!(terminal.backend());
    }

    // ---- Motion: pulse crítico na janela de sessão (Task 16) ----

    /// Janela `sessão` crítica (`remaining` < 10.0) dispara `pulse_color`
    /// via `window_line`. Confirma o CALL SITE real (não só `pulse_color`
    /// isolado, já coberto em `widgets::quota_gauge::tests`) — o buffer
    /// inteiro deve diferir entre dois `anim_frame` quando `animations=true`.
    /// Chama `render_detail` direto (não `render()` completo): o mesmo
    /// provider crítico também dispara o blink da MARCA da sidebar (Task
    /// 10), que não é gated por `animations` (gap pré-existente, fora do
    /// escopo desta task) — misturaria os dois efeitos no mesmo buffer.
    #[test]
    fn critical_window_gauge_pulses_across_anim_frames_when_animations_on() {
        let mut pv = make_claude_full();
        pv.quota.primary = Some(window(5.0, Some("2026-07-02T02:39:59Z"), Some("critical")));

        let backend = ratatui::backend::TestBackend::new(100, 32);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        let mut state = AppState::new();
        state.providers = vec![pv];
        state.selected = 0;
        state.screen = Screen::Detail;
        state.status = FetchStatus::Loaded;
        state.animations = true;

        state.anim_frame = 0;
        terminal
            .draw(|f| {
                let area = f.area();
                render_detail(&state, f, area, &mut HitMap::default());
            })
            .unwrap();
        let buf_frame0 = terminal.backend().buffer().clone();

        state.anim_frame = 18; // ~metade do ciclo de 37 ticks do pulso
        terminal
            .draw(|f| {
                let area = f.area();
                render_detail(&state, f, area, &mut HitMap::default());
            })
            .unwrap();
        let buf_frame18 = terminal.backend().buffer().clone();

        assert_ne!(
            buf_frame0, buf_frame18,
            "janela de sessão crítica deveria pulsar (cor diferente) entre anim_frame 0 e 18"
        );
    }

    /// Self-review do brief: animations=false → zero lerp visual — o pulso
    /// não deve alterar UM ÚNICO byte do buffer entre `anim_frame`s
    /// distintos (mesmo provider crítico do teste acima). `render_detail`
    /// direto pelo mesmo motivo do teste acima (isola do blink da sidebar).
    #[test]
    fn critical_window_gauge_stays_static_when_animations_off() {
        let mut pv = make_claude_full();
        pv.quota.primary = Some(window(5.0, Some("2026-07-02T02:39:59Z"), Some("critical")));

        let backend = ratatui::backend::TestBackend::new(100, 32);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        let mut state = AppState::new();
        state.providers = vec![pv];
        state.selected = 0;
        state.screen = Screen::Detail;
        state.status = FetchStatus::Loaded;
        state.animations = false;

        state.anim_frame = 0;
        terminal
            .draw(|f| {
                let area = f.area();
                render_detail(&state, f, area, &mut HitMap::default());
            })
            .unwrap();
        let buf_frame0 = terminal.backend().buffer().clone();

        state.anim_frame = 18;
        terminal
            .draw(|f| {
                let area = f.area();
                render_detail(&state, f, area, &mut HitMap::default());
            })
            .unwrap();
        let buf_frame18 = terminal.backend().buffer().clone();

        assert_eq!(
            buf_frame0, buf_frame18,
            "com animations=false, o pulso não deve alterar nada entre anim_frames"
        );
    }
}
