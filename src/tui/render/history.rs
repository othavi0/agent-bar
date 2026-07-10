//! Aba History (T13): chart nativo (braille, área preenchida) + tabela por
//! dia/provider. Toggle 24h/7d via tecla `t` (`state.history_range`,
//! `Action::ToggleHistoryRange`) — só o CHART respeita o toggle; a tabela e
//! o rodapé "Total 7d" sempre cobrem os 7 dias inteiros de `state.history`
//! (a fonte já é `records_since(7d)`, T2).
//!
//! Mata o gráfico de 7 pontos esticados (sparkline diário virando blocos
//! repetidos tipo ▂▂▂▂▅▅▅▅): o Claude sozinho já gera dezenas de records/dia,
//! e `bucket_by_hour` com 24 ou 168 pontos dá resolução suficiente pro
//! `Chart` braille desenhar uma curva de verdade.

use std::collections::BTreeMap;

use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::symbols::Marker;
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Axis, Block, BorderType, Borders, Cell, Chart, Dataset, GraphType, Paragraph, Row, Table,
};
use ratatui::Frame;
use throbber_widgets_tui::{Throbber, ThrobberState, BRAILLE_SIX};

use crate::theme::{provider_hex, ColorToken};
use crate::tui::mouse::{ChipKind, HitMap};
use crate::tui::render::shared::{abbrev_tokens, series_now};
use crate::tui::state::{AppState, HistoryRange};
use crate::tui::theme_bridge::{hex_to_color, provider_color, to_ratatui};
use crate::tui::widgets::chips::{chips_line, register_chip_hits};
use crate::usage::amp::AmpDollars;
use crate::usage::buckets::{bucket_by_day, bucket_by_hour, bucket_by_provider_day, DayBucket};
use crate::usage::UsageRecord;

/// Providers com log local de token — só estes ganham `Dataset` no chart.
/// Amp não gera `UsageRecord` (sem tracking local de token); ele só aparece
/// na tabela, via `amp_dollars` (ver `render_table`/`amp_dollars_of`).
const CHART_PROVIDERS: [&str; 2] = ["claude", "codex"];

// ---------------------------------------------------------------------------
// Formatação
// ---------------------------------------------------------------------------

/// Formata tokens do rodapé "Total 7d" — formato PRÉ-EXISTENTE (0 casas
/// decimais em K, 1 em M), mantido de propósito (contrato do brief: "formato
/// atual mantido"). Diferente de `abbrev_tokens` (shared.rs — 1 casa decimal
/// em toda unidade, usado pelo chart/tabela, elementos NOVOS desta task).
/// As duas funções coexistem por design, não por descuido.
fn fmt_tokens(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.0}K", n as f64 / 1_000.0)
    } else {
        format!("{n}")
    }
}

/// Abreviação PT de dia-da-semana (`seg`..`dom`) para o eixo X do chart no
/// range Week.
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

/// Labels do eixo X: exatamente 3 pontos (mais que isso quebra o
/// posicionamento das labels do meio — ver doc de `ratatui::widgets::Axis::
/// labels`). `now` é a âncora determinística (`series_now`, NUNCA
/// `OffsetDateTime::now_utc()`), convertida para `offset` (`state.
/// local_offset`) ANTES de extrair hora/dia-da-semana — mesmo contrato de
/// `spark_line` em `detail.rs` (T12): um `OffsetDateTime` não carrega "hora
/// local" por si só, tem que converter explicitamente.
fn x_axis_labels(
    now: time::OffsetDateTime,
    hours: usize,
    range: HistoryRange,
    offset: time::UtcOffset,
) -> Vec<Line<'static>> {
    let local_now = now.to_offset(offset);
    let oldest = local_now - time::Duration::hours((hours - 1) as i64);
    let mid = local_now - time::Duration::hours(((hours - 1) / 2) as i64);
    match range {
        HistoryRange::Day => vec![
            Line::from(format!("{:02}h", oldest.hour())),
            Line::from(format!("{:02}h", mid.hour())),
            Line::from("agora"),
        ],
        HistoryRange::Week => vec![
            Line::from(weekday_abbrev(oldest.weekday())),
            Line::from(weekday_abbrev(mid.weekday())),
            Line::from("hoje"),
        ],
    }
}

// ---------------------------------------------------------------------------
// Dados
// ---------------------------------------------------------------------------

/// Série (x, y) de tokens/hora de um provider, pronta para `Dataset::data`.
/// Pura/testável sem depender de `Frame` — separada de `render_chart` de
/// propósito (a montagem do widget não pode ser unit-testada diretamente).
fn chart_series(
    records: &[UsageRecord],
    provider: &str,
    now: time::OffsetDateTime,
    hours: usize,
) -> Vec<(f64, f64)> {
    let filtered: Vec<UsageRecord> = records
        .iter()
        .filter(|r| r.provider == provider)
        .cloned()
        .collect();
    bucket_by_hour(&filtered, now, hours)
        .iter()
        .enumerate()
        .map(|(i, b)| (i as f64, b.tokens as f64))
        .collect()
}

/// `AmpDollars` do `state.usage`, se o provider "amp" estiver presente.
/// Amp nunca aparece em `state.history` (sem `UsageRecord`), então esta é a
/// ÚNICA fonte da linha Amp na tabela.
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

/// Renderiza a aba History: chart braille (claude/codex) + tabela por
/// dia/provider + chips. `hits` recebe as zonas clicáveis dos chips.
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

    let title = match state.history_range {
        HistoryRange::Day => " Hist\u{f3}rico (24h) ",
        HistoryRange::Week => " Hist\u{f3}rico (7 dias) ",
    };
    let mut block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Thick)
        .border_style(Style::default().fg(to_ratatui(ColorToken::Blue)))
        .title(Span::styled(
            title,
            Style::default()
                .fg(to_ratatui(ColorToken::TextBright))
                .add_modifier(Modifier::BOLD),
        ));

    // Rodapé "Total 7d" some enquanto carrega e quando NÃO HÁ NADA pra
    // mostrar (nem token nem Amp). Com Amp-only (records vazio,
    // amp_dollars presente), ainda mostra "Total 7d: 0 tokens" — honesto,
    // não esconde o rodapé por causa de um campo vazio.
    if !loading && (!records.is_empty() || amp_dollars.is_some()) {
        block = block.title_bottom(footer_line(records));
    }

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Skeleton com spinner enquanto o parse está em voo. NUNCA branco.
    if loading {
        render_skeleton_screen(state, frame, inner, hits);
        return;
    }

    // `now`: âncora do chart. Sem records não há nada pra bucketizar — usa
    // uma constante determinística (`UNIX_EPOCH`, NUNCA `now_utc()`); os
    // buckets ficam 100% zero de qualquer forma (bucket_by_hour(&[], ..)),
    // então `render_chart` cai sozinho no texto "sem uso de tokens..." —
    // mesmo caminho de quando um provider não tem dado no range
    // selecionado, sem precisar de uma mensagem/branch nova.
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

    let hours = match state.history_range {
        HistoryRange::Day => 24,
        HistoryRange::Week => 24 * 7,
    };

    let provider_buckets = bucket_by_provider_day(records);

    let mut n_rows: u16 = provider_buckets.values().map(|v| v.len() as u16).sum();
    if amp_dollars.is_some() {
        n_rows += 1;
    }
    let table_len = n_rows.saturating_add(1).max(2); // +1 header, mínimo 2

    let vert = Layout::vertical([
        Constraint::Min(10),
        Constraint::Length(table_len),
        Constraint::Length(1),
    ])
    .split(inner);

    render_chart(state, frame, vert[0], records, now, hours);
    render_table(frame, vert[1], &provider_buckets, amp_dollars, state.scroll);
    render_footer_chips(frame, vert[2], hits);
}

/// Rodapé fixo do bloco: "Total 7d: X tokens / $Y" (right-aligned), sempre
/// sobre os 7 dias inteiros de `records` — independe do toggle 24h/7d.
fn footer_line(records: &[UsageRecord]) -> Line<'static> {
    let total_buckets = bucket_by_day(records);
    let total_tokens: u64 = total_buckets.iter().map(|b| b.tokens).sum();
    let total_cost: Option<f64> = {
        let costs: Vec<f64> = total_buckets.iter().filter_map(|b| b.cost_usd).collect();
        if costs.is_empty() {
            None
        } else {
            Some(costs.iter().sum())
        }
    };
    let footer_str = match total_cost {
        Some(c) => format!(" Total 7d: {} tokens / ${c:.2} ", fmt_tokens(total_tokens)),
        None => format!(" Total 7d: {} tokens ", fmt_tokens(total_tokens)),
    };
    Line::from(Span::styled(
        footer_str,
        Style::default()
            .fg(to_ratatui(ColorToken::TextBright))
            .add_modifier(Modifier::BOLD),
    ))
    .alignment(Alignment::Right)
}

// ---------------------------------------------------------------------------
// Chart (metade superior)
// ---------------------------------------------------------------------------

/// Chart nativo `ratatui` (braille, área preenchida) por provider. Amp fica
/// de fora (nunca tem `UsageRecord`); claude/codex sem dados no range
/// selecionado também ficam de fora (dataset totalmente zero é ruído, não
/// curva). Sem NENHUM provider com dado → placeholder textual.
fn render_chart(
    state: &AppState,
    frame: &mut Frame,
    area: Rect,
    records: &[UsageRecord],
    now: time::OffsetDateTime,
    hours: usize,
) {
    render_trend_chart(
        frame,
        area,
        records,
        now,
        hours,
        state.history_range,
        state.local_offset,
        " sem uso de tokens no per\u{ed}odo selecionado",
    );
}

/// Corpo reusável do chart (History E painel "Hoje (24h)" do Overview):
/// mesma visualização, parametrizada em vez de ler `AppState` direto —
/// `range` decide o estilo dos labels do eixo X, `empty_msg` é o
/// placeholder quando nenhum provider tem dado no range (o texto muda
/// conforme a tela: o History tem seletor de período, o Overview não).
#[allow(clippy::too_many_arguments)]
pub(super) fn render_trend_chart(
    frame: &mut Frame,
    area: Rect,
    records: &[UsageRecord],
    now: time::OffsetDateTime,
    hours: usize,
    range: HistoryRange,
    local_offset: time::UtcOffset,
    empty_msg: &str,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }

    // Materializa as séries ANTES de montar os `Dataset`s — `Dataset::data`
    // recebe `&[(f64, f64)]`; um `Vec` criado dentro do `.map()` de um
    // iterador não sobreviveria ao escopo (borrow pendurado). `series_by_
    // provider` é o dono; `datasets` só empresta dele, e os dois ficam
    // vivos até `frame.render_widget` consumir o `Chart`.
    let mut series_by_provider: Vec<(&'static str, Vec<(f64, f64)>)> = Vec::new();
    for pid in CHART_PROVIDERS {
        let series = chart_series(records, pid, now, hours);
        if series.iter().any(|&(_, y)| y > 0.0) {
            series_by_provider.push((pid, series));
        }
    }

    if series_by_provider.is_empty() {
        let p = Paragraph::new(Span::styled(
            empty_msg.to_string(),
            Style::default().fg(to_ratatui(ColorToken::Muted)),
        ));
        frame.render_widget(p, area);
        return;
    }

    let y_max: f64 = series_by_provider
        .iter()
        .flat_map(|(_, s)| s.iter().map(|&(_, y)| y))
        .fold(0.0_f64, f64::max)
        .max(1.0);

    let datasets: Vec<Dataset> = series_by_provider
        .iter()
        .map(|(pid, series)| {
            Dataset::default()
                .name(*pid)
                .marker(Marker::Braille)
                .graph_type(GraphType::Area)
                .style(Style::default().fg(hex_to_color(provider_hex(pid))))
                .data(series)
        })
        .collect();

    let x_labels = x_axis_labels(now, hours, range, local_offset);
    let y_labels = vec![Line::from("0"), Line::from(abbrev_tokens(y_max as u64))];

    let chart = Chart::new(datasets)
        .x_axis(
            Axis::default()
                .style(Style::default().fg(to_ratatui(ColorToken::Comment)))
                .bounds([0.0, (hours - 1) as f64])
                .labels(x_labels),
        )
        .y_axis(
            Axis::default()
                .style(Style::default().fg(to_ratatui(ColorToken::Comment)))
                .bounds([0.0, y_max])
                .labels(y_labels),
        );

    frame.render_widget(chart, area);
}

// ---------------------------------------------------------------------------
// Tabela (metade inferior)
// ---------------------------------------------------------------------------

/// Tabela `dia | provider | tokens | custo`: uma linha por (provider, dia)
/// de `bucket_by_provider_day`, cor da linha = marca do provider. Linha
/// final do Amp (se `amp_dollars` presente): tokens "–", custo com o resumo
/// de crédito + nota "sem logs locais de token" em Comment.
///
/// `scroll` (`state.scroll`, compartilhado com o ScrollView do Overview):
/// offset vertical nas LINHAS DE DADOS — o header nunca rola (sempre ocupa
/// a 1a linha de `area`). O layout externo (`render_history`) já entrega
/// aqui a altura REAL concedida pelo solver de constraints (pode ser menor
/// que `table_len`, o "tamanho desejado" usado só como hint); antes deste
/// fix, `Table` sempre desenhava a partir do início do Vec e as linhas além
/// da altura concedida ficavam inalcançáveis em terminal baixo — clamp
/// local (nunca muta `state`) segue o mesmo padrão do `render_cards` do
/// dashboard.
fn render_table(
    frame: &mut Frame,
    area: Rect,
    provider_buckets: &BTreeMap<String, Vec<DayBucket>>,
    amp_dollars: Option<&AmpDollars>,
    scroll: u16,
) {
    if area.height == 0 {
        return;
    }

    let header_style = Style::default()
        .fg(to_ratatui(ColorToken::Muted))
        .add_modifier(Modifier::BOLD);
    let header = Row::new(vec![
        Cell::from("dia").style(header_style),
        Cell::from("provider").style(header_style),
        Cell::from("tokens").style(header_style),
        Cell::from("custo").style(header_style),
    ]);

    let mut rows: Vec<Row<'_>> = Vec::new();
    for (provider, buckets) in provider_buckets {
        let p_color = provider_color(provider);
        for b in buckets {
            let dia = format!("{:02}/{:02}", b.date.month() as u8, b.date.day());
            let tokens = abbrev_tokens(b.tokens);
            let custo = match b.cost_usd {
                Some(c) => format!("${c:.2}"),
                None => "-".to_string(),
            };
            rows.push(
                Row::new(vec![
                    Cell::from(dia),
                    Cell::from(provider.as_str()),
                    Cell::from(tokens),
                    Cell::from(custo),
                ])
                .style(Style::default().fg(p_color)),
            );
        }
    }

    if let Some(ad) = amp_dollars {
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
        let custo_line = Line::from(vec![
            Span::raw(format!("{spent} de {total} (saldo cr {remaining})  ")),
            Span::styled(
                "sem logs locais de token",
                Style::default().fg(to_ratatui(ColorToken::Comment)),
            ),
        ]);
        rows.push(
            Row::new(vec![
                Cell::from("hoje"),
                Cell::from("amp"),
                Cell::from("\u{2013}"),
                Cell::from(custo_line),
            ])
            .style(Style::default().fg(p_color)),
        );
    }

    let widths = [
        Constraint::Length(6),
        Constraint::Length(9),
        Constraint::Length(8),
        Constraint::Fill(1),
    ];

    // Clamp local: header reserva a 1a linha, o resto é viewport de dados.
    let total_rows = rows.len();
    let visible_rows = area.height.saturating_sub(1) as usize;
    let max_scroll = total_rows.saturating_sub(visible_rows);
    let scroll = (scroll as usize).min(max_scroll);
    let hidden_above = scroll;
    let visible: Vec<Row<'_>> = rows.into_iter().skip(scroll).take(visible_rows).collect();
    let visible_len = visible.len();
    let hidden_below = total_rows.saturating_sub(scroll + visible_len);

    let table = Table::new(visible, widths).header(header).column_spacing(1);
    frame.render_widget(table, area);

    render_overflow_indicators(frame, area, hidden_above, hidden_below, visible_len);
}

/// Indicador de overflow (`▲ +N` acima / `▼ +N` abaixo) quando o clamp de
/// `render_table` esconde linhas fora do viewport — derive do clamp, nunca
/// de um cálculo paralelo. Desenhado por cima da última coluna (Comment),
/// alinhado à direita: só ocupa a largura do próprio texto, não a linha
/// inteira (senão apagaria as células de dado daquela linha).
fn render_overflow_indicators(
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
    // Linha 0 de `area` é o header; a 1a linha de dados visível é a linha 1,
    // a última é `visible_len` (1-indexado a partir do header).
    let last_row_offset = visible_len as u16;
    match (top, bottom) {
        (Some(t), Some(b)) if last_row_offset == 1 => {
            // Só 1 linha visível: os dois indicadores caem na mesma linha —
            // combina num único span em vez de um sobrescrever o outro.
            render_overflow_span(frame, area, 1, &format!("{t} {b}"));
        }
        (top, bottom) => {
            if let Some(t) = top {
                render_overflow_span(frame, area, 1, &t);
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

/// Chips centrados: `[t 24h/7d] [r atualizar] [esc voltar]`.
fn render_footer_chips(frame: &mut Frame, area: Rect, hits: &mut HitMap) {
    let chips: [(ChipKind, &str, &str); 3] = [
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

    // -----------------------------------------------------------------
    // Unit tests: fmt_tokens / weekday_abbrev / x_axis_labels / chart_series
    // -----------------------------------------------------------------

    #[test]
    fn fmt_tokens_footer_format_differs_from_abbrev_in_k_scale() {
        // fmt_tokens (rodapé, formato preservado) usa 0 casas em K —
        // abbrev_tokens (shared.rs, chart/tabela) usaria "1.2K" pro mesmo n.
        assert_eq!(fmt_tokens(1_200), "1K");
        assert_eq!(fmt_tokens(29_100_000), "29.1M");
        assert_eq!(fmt_tokens(500), "500");
    }

    #[test]
    fn weekday_abbrev_pt_short_names() {
        assert_eq!(weekday_abbrev(time::Weekday::Monday), "seg");
        assert_eq!(weekday_abbrev(time::Weekday::Wednesday), "qua");
        assert_eq!(weekday_abbrev(time::Weekday::Sunday), "dom");
    }

    #[test]
    fn x_axis_labels_day_ends_in_agora_and_uses_local_hours() {
        let now = time::macros::datetime!(2026-07-02 05:00:00 UTC);
        let labels = x_axis_labels(now, 24, HistoryRange::Day, time::UtcOffset::UTC);
        assert_eq!(labels.len(), 3);
        let text =
            |l: &Line<'_>| -> String { l.spans.iter().map(|s| s.content.as_ref()).collect() };
        assert_eq!(
            text(&labels[0]),
            "06h",
            "24h atras de 05h (offset UTC) = 06h do dia anterior"
        );
        assert_eq!(text(&labels[1]), "18h");
        assert_eq!(text(&labels[2]), "agora");
    }

    #[test]
    fn x_axis_labels_day_uses_local_offset_not_utc() {
        // Regressao no espirito do fix T12 (spark_line): o eixo tem que
        // converter pro offset local ANTES de extrair a hora, nunca vazar
        // a hora UTC crua rotulada como "local".
        let now = time::macros::datetime!(2026-07-02 02:00:00 UTC);
        let offset = time::UtcOffset::from_hms(-3, 0, 0).unwrap();
        let labels = x_axis_labels(now, 24, HistoryRange::Day, offset);
        let text =
            |l: &Line<'_>| -> String { l.spans.iter().map(|s| s.content.as_ref()).collect() };
        // now em -03:00 = 23h do dia anterior; 23h atras disso = 00h local
        // (NAO "03h", que seria o resultado se a hora UTC vazasse crua).
        assert_eq!(text(&labels[0]), "00h");
    }

    #[test]
    fn x_axis_labels_week_ends_in_hoje() {
        let now = time::macros::datetime!(2026-07-02 12:00:00 UTC);
        let hours = 24 * 7;
        let labels = x_axis_labels(now, hours, HistoryRange::Week, time::UtcOffset::UTC);
        let text =
            |l: &Line<'_>| -> String { l.spans.iter().map(|s| s.content.as_ref()).collect() };
        assert_eq!(text(&labels[2]), "hoje");
        let expected_oldest =
            weekday_abbrev((now - time::Duration::hours((hours - 1) as i64)).weekday());
        assert_eq!(text(&labels[0]), expected_oldest);
    }

    #[test]
    fn chart_series_has_hours_points_and_filters_provider() {
        let now = time::macros::datetime!(2026-07-02 12:00:00 UTC);
        let records = vec![
            rec("claude", "m", now - time::Duration::hours(1), 500),
            rec("codex", "m", now - time::Duration::hours(1), 999),
        ];
        let series = chart_series(&records, "claude", now, 24);
        assert_eq!(series.len(), 24);
        // Indice 22 = "1h atras" (indice 23 = hora atual/"now").
        assert_eq!(series[22].1, 500.0);
        assert!(series
            .iter()
            .enumerate()
            .all(|(i, &(_, y))| i == 22 || y == 0.0));
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
    /// blocos repetidos.
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
    // Fix: `state.scroll` na tabela (dados inalcançáveis em terminal baixo)
    // -----------------------------------------------------------------

    /// Muitos providers/dias sintéticos numa área curta (100x20 — mesmo
    /// terminal do smoke manual da spec) força a tabela a ter menos altura
    /// do que `total_rows`. Antes do fix, a tabela sempre desenhava a
    /// partir do topo do Vec e as linhas abaixo ficavam permanentemente
    /// inalcançáveis; `state.scroll=5` prova que a janela desliza e os
    /// indicadores ▲/▼ aparecem.
    fn many_provider_day_records(now: time::OffsetDateTime) -> Vec<UsageRecord> {
        let mut records = vec![];
        for provider in ["claude", "codex", "amp2"] {
            for d in 0..10 {
                records.push(rec(
                    provider,
                    "m",
                    now - time::Duration::days(d),
                    1000 + d as u64,
                ));
            }
        }
        records
    }

    #[test]
    fn history_table_scrolled() {
        let backend = ratatui::backend::TestBackend::new(100, 20);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        let mut state = AppState::new();
        state.screen = Screen::History;
        let now = time::macros::datetime!(2026-07-02 12:00:00 UTC);
        state.last_update = Some(now);
        state.history = Some(many_provider_day_records(now));
        state.scroll = 5;
        terminal
            .draw(|f| render_history(&state, f, f.area(), &mut HitMap::default()))
            .unwrap();
        insta::assert_snapshot!(terminal.backend());
    }

    /// Clamp local: scroll absurdamente além do fim não pode panicar nem
    /// deixar a tabela vazia — trava na última página (mesmo padrão do
    /// `render_cards` do dashboard, que faz `state.scroll.min(max_scroll)`
    /// sem mutar `state`). Na última página só o indicador ▲ (linhas
    /// acima) pode aparecer — ▼ nunca, já que não sobra nada abaixo.
    #[test]
    fn history_table_scroll_clamps_beyond_max() {
        let backend = ratatui::backend::TestBackend::new(100, 20);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        let mut state = AppState::new();
        state.screen = Screen::History;
        let now = time::macros::datetime!(2026-07-02 12:00:00 UTC);
        state.last_update = Some(now);
        state.history = Some(many_provider_day_records(now));
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
        // A última linha de dados sintética é a de "d=0" (mais recente,
        // 07/02 = `now`, último provider em ordem alfabética "codex") —
        // tem que estar visível na última página, senão o clamp cortou
        // demais e escondeu dado alcançável.
        assert!(
            text.contains("07/02"),
            "esperava a última linha de dados (d=0, 07/02) visível:\n{text}"
        );
    }
}

