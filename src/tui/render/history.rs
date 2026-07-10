//! Aba History (T20): chart de colunas (`column_chart`, T8) + lista de dias
//! expansível (▸/▾, T5 `sessions_by_day`) — a antiga tabela `dia | provider
//! | tokens | custo` morre aqui. Toggle 24h/7d via tecla `t`
//! (`state.history_range`, `Action::ToggleHistoryRange`) — só o CHART
//! respeita o toggle; a lista de dias e o rodapé "7 DIAS" sempre cobrem os
//! 7 dias inteiros de `state.history` (a fonte já é `records_since(7d)`, T2).
//!
//! O chart braille ORIGINAL desta tela (`render_trend_chart`/`chart_series`/
//! `x_axis_labels`/`CHART_PROVIDERS`) morreu na Task 11 junto com
//! `dashboard.rs` (único consumidor, painel "Hoje (24h)" do extinto
//! Overview). `weekday_abbrev` sobreviveu — reaproveitada pelos rótulos de
//! dia da lista nova (`day_list_lines`).

use std::collections::BTreeSet;

use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph};
use ratatui::Frame;
use throbber_widgets_tui::{Throbber, ThrobberState, BRAILLE_SIX};

use crate::theme::ColorToken;
use crate::tui::mouse::{ChipKind, HitMap};
use crate::tui::render::shared::series_now;
use crate::tui::state::{AppState, HistoryRange};
use crate::tui::theme_bridge::{provider_color, to_ratatui};
use crate::tui::widgets::chips::{chips_line, register_chip_hits};
use crate::tui::widgets::column_chart::{self, column_chart_lines_bucketed, fmt_tokens_short};
use crate::usage::amp::AmpDollars;
use crate::usage::buckets::{bucket_by_model_hour, sessions_by_day, DaySessions, ModelHourSeries};
use crate::usage::UsageRecord;

// ---------------------------------------------------------------------------
// Formatação
// ---------------------------------------------------------------------------

/// Abreviação PT de dia-da-semana (`seg`..`dom`) — usada pelo rótulo de dia
/// da lista de dias (`day_list_lines`, dia != hoje).
fn weekday_abbrev(w: time::Weekday) -> &'static str {
    match w {
        time::Weekday::Monday => "seg",
        time::Weekday::Tuesday => "ter",
        time::Weekday::Wednesday => "qua",
        time::Weekday::Thursday => "qui",
        time::Weekday::Friday => "sex",
        time::Weekday::Saturday => "s\u{e1}b",
        time::Weekday::Sunday => "dom",
    }
}

// ---------------------------------------------------------------------------
// Dados
// ---------------------------------------------------------------------------

/// `AmpDollars` do `state.usage`, se o provider "amp" estiver presente.
/// Amp nunca aparece em `state.history` (sem `UsageRecord`), então esta é a
/// ÚNICA fonte da nota Amp na lista de dias (`amp_note_line`).
fn amp_dollars_of(state: &AppState) -> Option<&AmpDollars> {
    state
        .usage
        .as_ref()?
        .providers
        .iter()
        .find(|pu| pu.provider == "amp")
        .and_then(|pu| pu.amp_dollars.as_ref())
}

// ---------------------------------------------------------------------------
// Render principal
// ---------------------------------------------------------------------------

/// Renderiza a aba History: chart de colunas (topo) + lista de dias
/// expansível + chips. `hits` recebe as zonas clicáveis dos chips.
pub fn render_history(state: &AppState, frame: &mut Frame, area: Rect, hits: &mut HitMap) {
    // `None` = HistoryLoaded ainda não chegou (parse em voo) → skeleton.
    // `Some` = carregado (mesmo vazio) → tela real. O gate antigo derivava
    // loading de `records.is_empty() && amp_dollars.is_none()` — como o
    // `UsageComputed` (que traz o Amp) chega SEGUNDOS antes do
    // `HistoryLoaded`, a tela afirmava "sem uso de tokens" com o parse do
    // Claude/Codex ainda rodando ("hoje 0 tok" da máquina real). O caso
    // só-Amp continua coberto: `Some(vec![])` não é skeleton.
    let loading = state.history.is_none();
    let records: &[UsageRecord] = state.history.as_deref().unwrap_or(&[]);
    let amp_dollars = amp_dollars_of(state);

    let mut block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Thick)
        .border_style(Style::default().fg(to_ratatui(ColorToken::Blue)));

    // `inner.width` decide o plano do chart (downsampling horizontal da
    // semana, `week_chart_plan`) ANTES do título — o título tem que refletir
    // a janela REAL que o chart desenha (achado Critical: "(7 dias)" no
    // título enquanto o chart só cobria o que coubesse na largura, com a
    // legenda contradizendo o rodapé). `.inner()` só depende das bordas, não
    // do título, então dá pra consultar aqui e anexar o `Span` do título
    // depois, sem duplicar a conta de bordas.
    let inner = block.inner(area);
    let (chart_hours, bucket_hours) = match state.history_range {
        HistoryRange::Day => (24usize, 1usize),
        HistoryRange::Week => {
            let avail = (inner.width as usize)
                .saturating_sub(column_chart::Y_AXIS_W)
                .max(1);
            match week_chart_plan(avail) {
                Some(n) => (24 * 7, n),
                // Último recurso: nem N=24 (7 colunas de 24h) cabe — terminal
                // absurdamente estreito. Mesmo clamp horário de antes
                // (bucket_hours=1, ver `render_top_chart`), mas o título
                // abaixo não pode mais afirmar "7 dias" se a janela real
                // encolheu.
                None => (avail.min(24 * 7), 1),
            }
        }
    };
    let title = match state.history_range {
        HistoryRange::Day => " Hist\u{f3}rico (24h) ".to_string(),
        HistoryRange::Week if chart_hours < 24 * 7 => {
            format!(" Hist\u{f3}rico (\u{fa}ltimas {chart_hours}h) ")
        }
        HistoryRange::Week => " Hist\u{f3}rico (7 dias) ".to_string(),
    };
    block = block.title(Span::styled(
        title,
        Style::default()
            .fg(to_ratatui(ColorToken::TextBright))
            .add_modifier(Modifier::BOLD),
    ));

    // Sessões por dia (T5) — fonte única da lista E do rodapé "7 DIAS".
    // Computada mesmo com `loading=true` seria sempre vazia (records vazio),
    // então não custa nada calcular incondicionalmente aqui.
    let days = sessions_by_day(records, state.local_offset);

    // Rodapé "7 DIAS" some enquanto carrega e quando NÃO HÁ NADA pra
    // mostrar (nem dia nem Amp). Com Amp-only (records vazio, amp_dollars
    // presente), ainda mostra o rodapé — honesto, não esconde por causa de
    // um campo vazio.
    if !loading && (!days.is_empty() || amp_dollars.is_some()) {
        block = block.title_bottom(footer_line(&days));
    }

    frame.render_widget(block, area);

    // Skeleton com spinner enquanto o parse está em voo. NUNCA branco.
    if loading {
        render_skeleton_screen(state, frame, inner, hits);
        return;
    }

    // `now`: âncora do chart (E da comparação "hoje" da lista de dias).
    // Sem records não há nada pra bucketizar — usa uma constante
    // determinística (`UNIX_EPOCH`, NUNCA `now_utc()`); os buckets ficam
    // 100% zero de qualquer forma, então `render_top_chart` cai sozinho no
    // estado vazio embutido do `column_chart` — mesmo caminho de quando um
    // provider não tem dado no range selecionado, sem precisar de uma
    // mensagem/branch nova.
    let now = if records.is_empty() {
        time::OffsetDateTime::UNIX_EPOCH
    } else {
        // `records` não-vazio implica `series_now` Some (fallback = max ts
        // de `state.history`) — o `None` aqui é só defensivo (nunca deveria
        // disparar), mantém o mesmo skeleton em vez de propagar um `unwrap`.
        match series_now(state) {
            Some(n) => n,
            None => {
                render_skeleton_screen(state, frame, inner, hits);
                return;
            }
        }
    };

    let today_local = now.to_offset(state.local_offset).date();

    let mut list_lines = day_list_lines(
        &days,
        state.history_selected,
        &state.history_expanded,
        today_local,
        state.local_offset,
    );
    if let Some(ad) = amp_dollars {
        list_lines.push(amp_note_line(ad));
    }
    let list_len = list_lines.len() as u16;

    let vert = Layout::vertical([
        Constraint::Min(10),
        Constraint::Length(list_len),
        Constraint::Length(1),
    ])
    .split(inner);

    render_top_chart(
        frame,
        vert[0],
        records,
        now,
        chart_hours,
        bucket_hours,
        state.local_offset,
    );
    render_day_list(frame, vert[1], list_lines, state.scroll);
    render_footer_chips(frame, vert[2], hits);
}

/// Rodapé fixo do bloco: "7 DIAS $X.XX · N tok · M sessões" (right-aligned),
/// sempre sobre os 7 dias inteiros de `days` — independe do toggle 24h/7d
/// (mesmo contrato do antigo "Total 7d").
fn footer_line(days: &[DaySessions]) -> Line<'static> {
    let total_tokens: u64 = days.iter().map(|d| d.tokens).sum();
    let total_sessions: usize = days.iter().map(|d| d.sessions.len()).sum();
    let total_cost: Option<f64> = days
        .iter()
        .filter_map(|d| d.cost_usd)
        .fold(None, |acc, c| Some(acc.unwrap_or(0.0) + c));
    let cost_str = match total_cost {
        Some(c) => format!("${c:.2}"),
        None => "\u{2014}".to_string(),
    };
    let footer_str = format!(
        " 7 DIAS {cost_str} \u{b7} {} tok \u{b7} {total_sessions} sess\u{f5}es ",
        fmt_tokens_short(total_tokens)
    );
    Line::from(Span::styled(
        footer_str,
        Style::default()
            .fg(to_ratatui(ColorToken::TextBright))
            .add_modifier(Modifier::BOLD),
    ))
    .alignment(Alignment::Right)
}

// ---------------------------------------------------------------------------
// Chart novo (topo) — column_chart (T8)
// ---------------------------------------------------------------------------

/// Escolhe o fator de agregação horária (`bucket_hours`) do range Week: o
/// menor N em `{1,2,3,4,6,8,12,24}` cujas `ceil(168/N)` colunas cabem em
/// `avail` (largura do chart já sem a reserva do eixo Y,
/// `column_chart::Y_AXIS_W` — calculada por quem chama). `None` = nem N=24
/// (7 colunas de 24h) cabe — terminal absurdamente estreito; quem chama cai
/// no último recurso (clamp horário antigo + título honesto, ver
/// `render_history`).
fn week_chart_plan(avail: usize) -> Option<usize> {
    const CANDIDATES: [usize; 8] = [1, 2, 3, 4, 6, 8, 12, 24];
    CANDIDATES
        .into_iter()
        .find(|&n| (24 * 7usize).div_ceil(n) <= avail)
}

/// Chart do topo da aba History: `column_chart_lines_bucketed` (T8 + fix de
/// downsampling da semana) com séries tokens/hora concatenadas de TODOS os
/// providers presentes em `records` (não uma lista fixa como
/// `CHART_PROVIDERS` — generaliza pra qualquer provider que gere
/// `UsageRecord` no futuro). Estado vazio (nenhum provider com dado) é
/// resolvido pelo próprio `column_chart_lines_bucketed`.
///
/// `hours`/`bucket_hours` já vêm decididos por `render_history`
/// (`week_chart_plan` pro range Week; Day sempre `(24, 1)`) — aqui só
/// reclampa em resolução horária (`bucket_hours == 1`, tanto Day quanto o
/// último recurso do Week): sem downsampling, 1 hora sempre vira ≥1 coluna,
/// e pedir mais horas do que colunas disponíveis faria o `Paragraph`
/// truncar a linha à direita, cortando os dados mais RECENTES (as séries
/// vêm ordenadas do mais antigo pro mais novo). Com `bucket_hours > 1`, quem
/// chamou já garantiu que as colunas agregadas cabem — não reclampa de novo.
fn render_top_chart(
    frame: &mut Frame,
    area: Rect,
    records: &[UsageRecord],
    now: time::OffsetDateTime,
    hours: usize,
    bucket_hours: usize,
    local_offset: time::UtcOffset,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let hours = if bucket_hours <= 1 {
        let max_hours = (area.width as usize)
            .saturating_sub(column_chart::Y_AXIS_W)
            .max(1);
        hours.min(max_hours)
    } else {
        hours
    };

    let providers: BTreeSet<&str> = records.iter().map(|r| r.provider.as_str()).collect();
    let mut series: Vec<ModelHourSeries> = Vec::new();
    for provider in providers {
        series.extend(bucket_by_model_hour(records, provider, now, hours));
    }
    let lines = column_chart_lines_bucketed(
        &series,
        area.width,
        area.height,
        now,
        local_offset,
        bucket_hours,
    );
    frame.render_widget(Paragraph::new(lines), area);
}

// ---------------------------------------------------------------------------
// Lista de dias expansível (substitui a Table legada)
// ---------------------------------------------------------------------------

/// Constrói as linhas visuais da lista de dias: 1 linha por `DaySessions`
/// (desc — mais recente primeiro, já vem assim de `sessions_by_day`) +
/// linhas de sessão indentadas quando o dia está em `expanded`. NÃO inclui
/// a nota do Amp (`amp_note_line`, anexada por quem chama) — mantido
/// separado pra ficar testável sem depender de `AmpDollars`.
fn day_list_lines(
    days: &[DaySessions],
    selected: usize,
    expanded: &BTreeSet<time::Date>,
    today: time::Date,
    local_offset: time::UtcOffset,
) -> Vec<Line<'static>> {
    let mut lines = Vec::with_capacity(days.len());
    for (i, day) in days.iter().enumerate() {
        let is_selected = i == selected;
        let is_expanded = expanded.contains(&day.date);
        let arrow = if is_expanded { "\u{25be}" } else { "\u{25b8}" }; // ▾ / ▸
        let label = if day.date == today {
            "hoje".to_string()
        } else {
            weekday_abbrev(day.date.weekday()).to_string()
        };
        let tokens = fmt_tokens_short(day.tokens);
        let cost = day
            .cost_usd
            .map(|c| format!("${c:.2}"))
            .unwrap_or_else(|| "\u{2014}".to_string());

        let arrow_style = Style::default().fg(to_ratatui(if is_selected {
            ColorToken::Blue
        } else {
            ColorToken::Muted
        }));
        let text_style = Style::default().fg(to_ratatui(ColorToken::Text));

        let mut line = Line::from(vec![
            Span::styled(format!("{arrow} "), arrow_style),
            Span::styled(
                format!(
                    "{:02}/{:02} \u{b7} {label} \u{b7} {tokens} \u{b7} {cost} \u{b7} {} sess\u{f5}es",
                    day.date.month() as u8,
                    day.date.day(),
                    day.sessions.len(),
                ),
                text_style,
            ),
        ]);
        // Preenche a linha INTEIRA com o bg de seleção — mesmo padrão de
        // `render/login.rs::render_provider_list` (o bg entra "por baixo"
        // dos estilos de cada span, que só definem fg).
        if is_selected {
            line = line.style(Style::default().bg(to_ratatui(ColorToken::SelBg)));
        }
        lines.push(line);

        if is_expanded {
            for s in &day.sessions {
                let local_start = s.start.to_offset(local_offset);
                let hhmm = format!("{:02}:{:02}", local_start.hour(), local_start.minute());
                let project = s.project.as_deref().unwrap_or("\u{2014}");
                let model = s.dominant_model.as_deref().unwrap_or("\u{2014}");
                let stokens = fmt_tokens_short(s.tokens);
                let scost = s
                    .cost_usd
                    .map(|c| format!("${c:.2}"))
                    .unwrap_or_else(|| "\u{2014}".to_string());
                lines.push(Line::from(Span::styled(
                    format!("    {hhmm}  {project}  {model}  {stokens}  {scost}"),
                    Style::default().fg(to_ratatui(ColorToken::Comment)),
                )));
            }
        }
    }
    lines
}

/// Nota do Amp (crédito, sem log local de token) — mesmo conteúdo da antiga
/// linha "hoje/amp" da Table (T13), adaptada pra 1 linha de texto (a lista
/// de dias não tem mais colunas).
fn amp_note_line(ad: &AmpDollars) -> Line<'static> {
    let p_color = provider_color("amp");
    let spent = ad
        .spent
        .map(|v| format!("${v:.2}"))
        .unwrap_or_else(|| "-".to_string());
    let total = ad
        .total
        .map(|v| format!("${v:.2}"))
        .unwrap_or_else(|| "-".to_string());
    let remaining = ad
        .remaining
        .map(|v| format!("${v:.2}"))
        .unwrap_or_else(|| "-".to_string());
    Line::from(vec![
        Span::styled(
            format!("hoje \u{b7} amp \u{b7} {spent} de {total} (saldo cr {remaining})  "),
            Style::default().fg(p_color),
        ),
        Span::styled(
            "sem logs locais de token",
            Style::default().fg(to_ratatui(ColorToken::Comment)),
        ),
    ])
}

/// Desenha a lista de dias já construída (`lines`) com clamp local de
/// scroll — mesmo padrão do antigo `render_table` (T13): o layout externo
/// já entrega aqui a altura REAL concedida pelo solver de constraints (pode
/// ser menor que `list_len`, o "tamanho desejado" usado só como hint);
/// clamp local (nunca muta `state`) evita que linhas fiquem inalcançáveis
/// em terminal baixo.
fn render_day_list(frame: &mut Frame, area: Rect, lines: Vec<Line<'static>>, scroll: u16) {
    if area.height == 0 {
        return;
    }
    let total_rows = lines.len();
    let visible_rows = area.height as usize;
    let max_scroll = total_rows.saturating_sub(visible_rows);
    let scroll = (scroll as usize).min(max_scroll);
    let hidden_above = scroll;
    let visible: Vec<Line<'static>> = lines.into_iter().skip(scroll).take(visible_rows).collect();
    let visible_len = visible.len();
    let hidden_below = total_rows.saturating_sub(scroll + visible_len);

    frame.render_widget(Paragraph::new(visible), area);
    render_list_overflow(frame, area, hidden_above, hidden_below, visible_len);
}

/// Indicador de overflow (`▲ +N` acima / `▼ +N` abaixo) da lista de dias —
/// mesmo racional do antigo `render_overflow_indicators` (T13), SEM o
/// offset de header: a lista não é mais tabela, a linha 0 de `area` já é a
/// 1a linha de DADO.
fn render_list_overflow(
    frame: &mut Frame,
    area: Rect,
    hidden_above: usize,
    hidden_below: usize,
    visible_len: usize,
) {
    if visible_len == 0 {
        return;
    }
    let top = (hidden_above > 0).then(|| format!("\u{25b2} +{hidden_above}"));
    let bottom = (hidden_below > 0).then(|| format!("\u{25bc} +{hidden_below}"));
    let last_row_offset = (visible_len - 1) as u16;
    match (top, bottom) {
        (Some(t), Some(b)) if last_row_offset == 0 => {
            // Só 1 linha visível: os dois indicadores caem na mesma linha —
            // combina num único span em vez de um sobrescrever o outro.
            render_overflow_span(frame, area, 0, &format!("{t} {b}"));
        }
        (top, bottom) => {
            if let Some(t) = top {
                render_overflow_span(frame, area, 0, &t);
            }
            if let Some(b) = bottom {
                render_overflow_span(frame, area, last_row_offset, &b);
            }
        }
    }
}

fn render_overflow_span(frame: &mut Frame, area: Rect, row_offset: u16, text: &str) {
    let w = (text.chars().count() as u16).min(area.width);
    if w == 0 || row_offset >= area.height {
        return;
    }
    let rect = Rect::new(
        area.x + area.width.saturating_sub(w),
        area.y + row_offset,
        w,
        1,
    );
    frame.render_widget(
        Paragraph::new(Span::styled(
            text.to_string(),
            Style::default().fg(to_ratatui(ColorToken::Comment)),
        )),
        rect,
    );
}

// ---------------------------------------------------------------------------
// Estados especiais
// ---------------------------------------------------------------------------

/// Skeleton "coletando histórico…" com spinner — nunca tela branca. Mesmo
/// padrão do skeleton do Overview (`dashboard::render_skeleton_card`):
/// throbber braille avançado por `state.throbber.index` (Animação C).
fn render_skeleton(state: &AppState, frame: &mut Frame, area: Rect) {
    if area.height == 0 {
        return;
    }
    let throbber_widget = Throbber::default()
        .throbber_set(BRAILLE_SIX)
        .throbber_style(Style::default().fg(to_ratatui(ColorToken::Cyan)))
        .use_type(throbber_widgets_tui::WhichUse::Spin);
    let mut throbber_state = ThrobberState::default();
    for _ in 0..state.throbber.index {
        throbber_state.calc_next();
    }
    let line = Line::from(vec![
        throbber_widget.to_symbol_span(&throbber_state),
        Span::styled(
            " coletando hist\u{f3}rico\u{2026}",
            Style::default().fg(to_ratatui(ColorToken::Muted)),
        ),
    ]);
    let p = Paragraph::new(line).alignment(Alignment::Center);
    frame.render_widget(p, area);
}

/// Layout do estado sem-dados: skeleton (topo) + chips (rodapé) — os chips
/// continuam ativos mesmo sem dados (ex. `[r]` pode disparar o load).
fn render_skeleton_screen(state: &AppState, frame: &mut Frame, inner: Rect, hits: &mut HitMap) {
    let vert = Layout::vertical([Constraint::Min(3), Constraint::Length(1)]).split(inner);
    render_skeleton(state, frame, vert[0]);
    render_footer_chips(frame, vert[1], hits);
}

/// Chips centrados: `[↵ expandir] [t 24h/7d] [r atualizar] [esc voltar]`.
fn render_footer_chips(frame: &mut Frame, area: Rect, hits: &mut HitMap) {
    let chips: [(ChipKind, &str, &str); 4] = [
        (ChipKind::ExpandDay, "\u{21b5}", "expandir"),
        (ChipKind::ToggleRange, "t", "24h/7d"),
        (ChipKind::Refresh, "r", "atualizar"),
        (ChipKind::Back, "esc", "voltar"),
    ];
    let line = chips_line(&chips, area.width);
    frame.render_widget(Paragraph::new(line), area);
    register_chip_hits(&chips, area, hits);
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    use crate::tui::state::{AppState, Screen};
    use crate::usage::amp::AmpDollars;
    use crate::usage::{Cost, ProviderUsage, UsageSummary};

    fn rec(provider: &str, model: &str, ts: time::OffsetDateTime, tokens: u64) -> UsageRecord {
        UsageRecord {
            provider: provider.to_string(),
            model: Some(model.to_string()),
            input: tokens,
            output: 0,
            cache_read: 0,
            cache_write: 0,
            ts,
            session_id: None,
            project: None,
        }
    }

    /// `rec` + `session_id`/`project` — necessário pra exercitar
    /// `sessions_by_day` de verdade (sem session_id, tudo colapsa numa
    /// sessão "—" por dia).
    fn session_rec(
        provider: &str,
        model: &str,
        session_id: &str,
        project: Option<&str>,
        ts: time::OffsetDateTime,
        tokens: u64,
    ) -> UsageRecord {
        let mut r = rec(provider, model, ts, tokens);
        r.session_id = Some(session_id.to_string());
        r.project = project.map(|s| s.to_string());
        r
    }

    // -----------------------------------------------------------------
    // Unit tests: weekday_abbrev
    // -----------------------------------------------------------------

    #[test]
    fn weekday_abbrev_pt_short_names() {
        assert_eq!(weekday_abbrev(time::Weekday::Monday), "seg");
        assert_eq!(weekday_abbrev(time::Weekday::Wednesday), "qua");
        assert_eq!(weekday_abbrev(time::Weekday::Sunday), "dom");
    }

    // -----------------------------------------------------------------
    // Unit tests: amp_dollars_of
    // -----------------------------------------------------------------

    fn amp_usage(spent: f64, total: f64, remaining: f64) -> UsageSummary {
        UsageSummary {
            providers: vec![ProviderUsage {
                provider: "amp".to_string(),
                total_input: 0,
                total_output: 0,
                total_cache_read: 0,
                total_cache_write: 0,
                cost: None,
                by_model: vec![],
                amp_dollars: Some(AmpDollars {
                    spent: Some(spent),
                    remaining: Some(remaining),
                    total: Some(total),
                }),
            }],
            total_cost: Cost { usd: 0.0, brl: 0.0 },
            fx_rate: 5.50,
        }
    }

    #[test]
    fn amp_dollars_of_none_when_usage_missing() {
        let state = AppState::new();
        assert!(amp_dollars_of(&state).is_none());
    }

    #[test]
    fn amp_dollars_of_finds_amp_provider() {
        let mut state = AppState::new();
        state.usage = Some(amp_usage(0.81, 5.0, 4.19));
        let ad = amp_dollars_of(&state).expect("amp dollars presentes");
        assert_eq!(ad.remaining, Some(4.19));
    }

    // -----------------------------------------------------------------
    // Snapshots (brief Step 3)
    // -----------------------------------------------------------------

    /// Gera uma onda de records (nao uma reta) pra provar que o chart tem
    /// FORMA de verdade — mata o antigo esticamento de 7 pontos que virava
    /// blocos repetidos tipo ▂▂▂▂▅▅▅▅.
    fn synth_wave(
        provider: &str,
        model: &str,
        now: time::OffsetDateTime,
        hours_back_max: i64,
    ) -> Vec<UsageRecord> {
        (0..hours_back_max)
            .step_by(3)
            .map(|h| {
                let phase = h as f64 / 12.0;
                let tokens = ((phase.sin() + 1.2) * 40_000.0) as u64;
                rec(provider, model, now - time::Duration::hours(h), tokens)
            })
            .collect()
    }

    #[test]
    fn history_week() {
        let backend = ratatui::backend::TestBackend::new(100, 32);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        let mut state = AppState::new();
        state.screen = Screen::History;
        let now = time::macros::datetime!(2026-07-02 12:00:00 UTC);
        state.last_update = Some(now);
        let mut records = synth_wave("claude", "claude-opus-4-8", now, 24 * 7);
        records.extend(synth_wave("codex", "gpt-5.5", now, 24 * 7 - 10));
        state.history = Some(records);
        terminal
            .draw(|f| render_history(&state, f, f.area(), &mut HitMap::default()))
            .unwrap();
        insta::assert_snapshot!(terminal.backend());
    }

    #[test]
    fn history_day() {
        let backend = ratatui::backend::TestBackend::new(100, 32);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        let mut state = AppState::new();
        state.screen = Screen::History;
        state.history_range = HistoryRange::Day;
        let now = time::macros::datetime!(2026-07-02 12:00:00 UTC);
        state.last_update = Some(now);
        let mut records = synth_wave("claude", "claude-opus-4-8", now, 24 * 7);
        records.extend(synth_wave("codex", "gpt-5.5", now, 20));
        state.history = Some(records);
        terminal
            .draw(|f| render_history(&state, f, f.area(), &mut HitMap::default()))
            .unwrap();
        insta::assert_snapshot!(terminal.backend());
    }

    #[test]
    fn history_empty() {
        let backend = ratatui::backend::TestBackend::new(100, 32);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        let mut state = AppState::new();
        state.screen = Screen::History;
        state.history = Some(vec![]);
        terminal
            .draw(|f| render_history(&state, f, f.area(), &mut HitMap::default()))
            .unwrap();
        insta::assert_snapshot!(terminal.backend());
    }

    #[test]
    fn history_amp_note() {
        let backend = ratatui::backend::TestBackend::new(100, 32);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        let mut state = AppState::new();
        state.screen = Screen::History;
        let now = time::macros::datetime!(2026-07-02 12:00:00 UTC);
        state.last_update = Some(now);
        state.history = Some(synth_wave("claude", "claude-opus-4-8", now, 24 * 3));
        state.usage = Some(amp_usage(0.81, 5.0, 4.19));
        terminal
            .draw(|f| render_history(&state, f, f.area(), &mut HitMap::default()))
            .unwrap();
        insta::assert_snapshot!(terminal.backend());
    }

    /// Regressão do "hoje 0 tok" visto na máquina real: `UsageComputed`
    /// (que traz `amp_dollars`) chega SEGUNDOS antes de `HistoryLoaded`.
    /// Nessa janela, `state.history` ainda é `None` (carregando) — a tela
    /// deve continuar no skeleton "coletando histórico…", NUNCA afirmar
    /// "sem uso de tokens" (mentira: o parse só não terminou). O sinal de
    /// loading é `state.history.is_none()`, não a presença do Amp.
    #[test]
    fn history_loading_with_amp_shows_skeleton() {
        let backend = ratatui::backend::TestBackend::new(100, 32);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        let mut state = AppState::new();
        state.screen = Screen::History;
        state.history = None; // HistoryLoaded ainda não chegou
        state.usage = Some(amp_usage(0.81, 5.0, 4.19)); // UsageComputed já chegou

        terminal
            .draw(|f| render_history(&state, f, f.area(), &mut HitMap::default()))
            .unwrap();

        let buffer = terminal.backend().buffer();
        let mut screen = String::new();
        for y in 0..32u16 {
            for x in 0..100u16 {
                if let Some(cell) = buffer.cell((x, y)) {
                    screen.push_str(cell.symbol());
                }
            }
            screen.push('\n');
        }
        assert!(
            screen.contains("coletando hist\u{f3}rico"),
            "history=None deve mostrar skeleton de loading:\n{screen}"
        );
        assert!(
            !screen.contains("sem uso de tokens"),
            "history=None não pode afirmar 'sem uso' (ainda carregando):\n{screen}"
        );
    }

    /// Regressão pós-review: usuário só-Amp (`state.history` carregado mas
    /// vazio — sem log local de claude/codex — `amp_dollars` presente).
    /// Antes do fix, o early-return do skeleton rodava só olhando pra
    /// `records.is_empty()` e prendia esse usuário em "coletando
    /// histórico…" pra sempre, mesmo com o Amp já carregado.
    #[test]
    fn history_amp_only() {
        let backend = ratatui::backend::TestBackend::new(100, 32);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        let mut state = AppState::new();
        state.screen = Screen::History;
        state.history = Some(vec![]); // carregado, mas sem records (só-Amp)
        state.usage = Some(amp_usage(0.81, 5.0, 4.19));
        terminal
            .draw(|f| render_history(&state, f, f.area(), &mut HitMap::default()))
            .unwrap();
        insta::assert_snapshot!(terminal.backend());
    }

    // -----------------------------------------------------------------
    // Lista de dias expansível (Task 20)
    // -----------------------------------------------------------------

    /// Fixture com 2 dias reais (sessões com session_id/project distintos)
    /// — reusada pelo snapshot de dia expandido.
    fn fixture_history() -> AppState {
        let mut state = AppState::new();
        state.screen = Screen::History;
        let now = time::macros::datetime!(2026-07-10 18:00:00 UTC);
        state.last_update = Some(now);
        state.history = Some(vec![
            session_rec(
                "claude",
                "claude-fable-5",
                "s1",
                Some("agent-bar"),
                time::macros::datetime!(2026-07-10 09:15:00 UTC),
                12_400,
            ),
            session_rec(
                "claude",
                "claude-opus-4-8",
                "s2",
                None,
                time::macros::datetime!(2026-07-10 14:42:00 UTC),
                31_000,
            ),
            session_rec(
                "codex",
                "gpt-5.5",
                "s3",
                Some("crm"),
                time::macros::datetime!(2026-07-09 08:05:00 UTC),
                8_200,
            ),
        ]);
        state
    }

    #[test]
    fn history_snapshot_with_expanded_day() {
        let backend = ratatui::backend::TestBackend::new(100, 32);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        let mut state = fixture_history();
        state
            .history_expanded
            .insert(time::macros::date!(2026 - 07 - 10));
        terminal
            .draw(|f| render_history(&state, f, f.area(), &mut HitMap::default()))
            .unwrap();
        insta::assert_snapshot!(terminal.backend());
    }

    /// Sessões distintas em 10 dias diferentes — força o scroll da lista de
    /// dias (mesmo racional do antigo `history_table_scrolled`, T13):
    /// terminal baixo não cabe as 10 linhas de dia + chart mínimo + chips.
    fn many_day_records(now: time::OffsetDateTime) -> Vec<UsageRecord> {
        (0..10i64)
            .map(|d| {
                session_rec(
                    "claude",
                    "claude-fable-5",
                    &format!("s{d}"),
                    None,
                    now - time::Duration::days(d),
                    1_000 + d as u64,
                )
            })
            .collect()
    }

    #[test]
    fn history_day_list_scrolled() {
        let backend = ratatui::backend::TestBackend::new(100, 20);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        let mut state = AppState::new();
        state.screen = Screen::History;
        let now = time::macros::datetime!(2026-07-02 12:00:00 UTC);
        state.last_update = Some(now);
        state.history = Some(many_day_records(now));
        state.scroll = 5;
        terminal
            .draw(|f| render_history(&state, f, f.area(), &mut HitMap::default()))
            .unwrap();
        insta::assert_snapshot!(terminal.backend());
    }

    /// Clamp local: scroll absurdamente além do fim não pode panicar nem
    /// deixar a lista vazia — trava na última página (mesmo padrão do
    /// antigo `history_table_scroll_clamps_beyond_max`, T13). Na última
    /// página só o indicador ▲ (linhas acima) pode aparecer — ▼ nunca, já
    /// que não sobra nada abaixo.
    #[test]
    fn history_day_list_scroll_clamps_beyond_max() {
        let backend = ratatui::backend::TestBackend::new(100, 20);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        let mut state = AppState::new();
        state.screen = Screen::History;
        let now = time::macros::datetime!(2026-07-02 12:00:00 UTC);
        state.last_update = Some(now);
        state.history = Some(many_day_records(now));
        state.scroll = u16::MAX;
        terminal
            .draw(|f| render_history(&state, f, f.area(), &mut HitMap::default()))
            .unwrap();
        let buf = terminal.backend().buffer().clone();
        let mut lines: Vec<String> = Vec::new();
        for y in 0..buf.area.height {
            let mut line = String::new();
            for x in 0..buf.area.width {
                line.push_str(buf.cell((x, y)).unwrap().symbol());
            }
            lines.push(line);
        }
        let text = lines.join("\n");
        assert!(
            !text.contains('\u{25bc}'),
            "não pode sobrar indicador ▼ na última página:\n{text}"
        );
        assert!(
            text.contains('\u{25b2}'),
            "esperava indicador ▲ (linhas ocultas acima) na última página:\n{text}"
        );
        // A última linha de dia (mais antigo, d=9, now - 9 dias) tem que
        // estar visível na última página, senão o clamp cortou demais e
        // escondeu dado alcançável.
        let oldest = now - time::Duration::days(9);
        let oldest_str = format!("{:02}/{:02}", oldest.month() as u8, oldest.day());
        assert!(
            text.contains(&oldest_str),
            "esperava a linha do dia mais antigo ({oldest_str}) visível:\n{text}"
        );
    }
}
