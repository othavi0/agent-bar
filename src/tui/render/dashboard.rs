//! Tela Visão Geral: um card denso por provider, rolável via `tui-scrollview`.
//! Substitui a antiga tabela única — cada provider ganha um card com título
//! (nome + plano + status de login), gauges de sessão/semana, sparkline real
//! de 24h e custo do dia. Estados especiais (deslogado/skeleton/vazio) usam
//! a mesma altura de card (6 linhas) para manter a matemática de scroll simples.

use ratatui::layout::{Alignment, Constraint, Layout, Position, Rect, Size};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph};
use ratatui::Frame;
use throbber_widgets_tui::{Throbber, ThrobberState, BRAILLE_SIX};
use tui_scrollview::{ScrollView, ScrollViewState, ScrollbarVisibility};

use crate::providers::types::QuotaWindow;
use crate::settings::GlyphMode;
use crate::theme::{provider_hex, ColorToken};
use crate::tui::login_state::{login_state_for, LoginState};
use crate::tui::mouse::{ChipKind, HitMap, MouseTarget};
use crate::tui::render::shared::series_now;
use crate::tui::state::{AppState, ProviderView};
use crate::tui::theme_bridge::{hex_to_color, to_ratatui};
use crate::tui::widgets::chips::{chips_line, register_chip_hits};
use crate::tui::widgets::icons::{glyph, Icon};
use crate::tui::widgets::quota_gauge::{gauge_spans, pulse_color};
use crate::tui::widgets::severity::severity_color_api;
use crate::tui::widgets::sparkline::sparkline_str;
use crate::usage::buckets::provider_series_24h;
use crate::usage::ProviderUsage;

/// Altura fixa de todo card (título na borda + 2 gauges + sparkline/custo +
/// respiro + borda). Uniforme mesmo em estados degradados (deslogado/skeleton)
/// para manter `6 * n_cards` como matemática de scroll única fonte de verdade.
const CARD_H: u16 = 6;

/// Builds a 7-char gauge string (chars only, no color) for remaining quota.
/// Delegates to `quota_gauge::gauge_spans` with width=7.
/// Public for tests in render/mod.rs (`quota_bar_logic`).
pub fn quota_bar_pub(remaining_pct: f64) -> String {
    gauge_spans(remaining_pct, 7, to_ratatui(ColorToken::Green))
        .iter()
        .map(|s| s.content.as_ref())
        .collect()
}

/// Renders the Overview screen: um card por provider, chips de ação no rodapé.
pub fn render_dashboard(state: &AppState, frame: &mut Frame, area: Rect, hits: &mut HitMap) {
    if area.height == 0 || area.width == 0 {
        return;
    }

    let vert = Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).split(area);
    let content_area = vert[0];
    let footer_area = vert[1];

    if state.providers.is_empty() {
        if state.fetch_pending.is_empty() {
            render_empty_state(frame, content_area);
        } else {
            render_skeleton(state, frame, content_area);
        }
    } else {
        render_cards(state, frame, content_area, hits);
    }

    render_footer_chips(frame, footer_area, hits);
}

// ---------------------------------------------------------------------------
// Cards reais (ScrollView)
// ---------------------------------------------------------------------------

fn render_cards(state: &AppState, frame: &mut Frame, area: Rect, hits: &mut HitMap) {
    let n = state.providers.len() as u16;
    let content_h = CARD_H.saturating_mul(n);
    let viewport_h = area.height;
    let max_scroll = content_h.saturating_sub(viewport_h);
    let scroll = state.scroll.min(max_scroll);

    // Reserva 1 coluna p/ o trilho de scroll (sempre visível — Always, não
    // Automatic): evita que a borda direita dos cards fique atrás do trilho
    // quando o conteúdo aperta a largura exata do viewport.
    let card_w = area.width.saturating_sub(1).max(1);

    let mut sv = ScrollView::new(Size::new(card_w, content_h.max(1)))
        .vertical_scrollbar_visibility(ScrollbarVisibility::Always)
        .horizontal_scrollbar_visibility(ScrollbarVisibility::Never);

    let now = series_now(state);

    for (i, pv) in state.providers.iter().enumerate() {
        let card_rect = Rect::new(0, CARD_H * i as u16, card_w, CARD_H);
        render_provider_card(state, pv, i, card_rect, now, &mut sv);
    }

    let mut sv_state = ScrollViewState::with_offset(Position::new(0, scroll));
    frame.render_stateful_widget(&sv, area, &mut sv_state);

    // Hit-testing em espaço de tela: só cards (parcialmente) visíveis, com
    // offset do scroll aplicado — a zona nunca cobre linhas fora do viewport.
    for i in 0..n as usize {
        let top = CARD_H * i as u16;
        let bottom = top + CARD_H;
        if bottom <= scroll || top >= scroll + viewport_h {
            continue;
        }
        let visible_top = top.max(scroll);
        let visible_bottom = bottom.min(scroll + viewport_h);
        let screen_y = area.y + (visible_top - scroll);
        let h = visible_bottom - visible_top;
        hits.push(
            Rect::new(area.x, screen_y, area.width, h),
            MouseTarget::Card(i),
        );
    }
}

fn login_status_span(logged: LoginState, mode: GlyphMode) -> Span<'static> {
    match logged {
        LoginState::Ok => Span::styled(
            format!("{} ok", glyph(Icon::Ok, mode)),
            Style::default().fg(to_ratatui(ColorToken::Green)),
        ),
        // Checking nao tem Icon dedicado (estado transitorio): permanece ● literal.
        LoginState::Checking => Span::styled(
            "\u{25cf} verificando\u{2026}",
            Style::default().fg(to_ratatui(ColorToken::Yellow)),
        ),
        LoginState::NoToken => Span::styled(
            format!("{} sem token", glyph(Icon::NoToken, mode)),
            Style::default().fg(to_ratatui(ColorToken::Yellow)),
        ),
        LoginState::LoggedOut => Span::styled(
            format!("{} deslogado", glyph(Icon::LoggedOut, mode)),
            Style::default().fg(to_ratatui(ColorToken::Red)),
        ),
        // Falha nao-auth (parse/rede/API): erro real, mas nao pede re-login.
        LoginState::Error => Span::styled(
            format!("{} erro", glyph(Icon::Warn, mode)),
            Style::default().fg(to_ratatui(ColorToken::Red)),
        ),
    }
}

/// Cor da borda do card: feedback de hover. Extraída como função pura (em
/// vez de inline no render) pra ser testável direto — um snapshot de
/// `TestBackend` serializa só caracteres, nunca cor (`buffer_view` não
/// exporta estilo), então a decisão de cor teria zero cobertura sem isto.
fn card_border_color(hovered: bool) -> ratatui::style::Color {
    if hovered {
        to_ratatui(ColorToken::Text)
    } else {
        to_ratatui(ColorToken::Comment)
    }
}

/// Largura do gauge derivada da área real do card — nunca constante mágica
/// (bug do código antigo). 8 label + 6 pct + 14 reset = 28 colunas fixas.
fn derive_gauge_width(card_width: u16) -> usize {
    (card_width as usize).saturating_sub(8 + 6 + 14).max(10)
}

/// Extrai HH:MM de um timestamp ISO; fallback pra string crua ou "-".
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

/// Formata o custo do dia de um provider (Amp = crédito restante).
fn fmt_provider_cost(pu: &ProviderUsage) -> String {
    if pu.provider == "amp" {
        return match pu.amp_dollars.as_ref().and_then(|ad| ad.remaining) {
            Some(rem) => format!("cr ${:.2}", rem),
            None => "-".to_string(),
        };
    }
    match &pu.cost {
        Some(c) => format!("${:.2}", c.usd),
        None => "-".to_string(),
    }
}

/// Uma linha de gauge (sessão/semana): label fixo 8, gauge, % right-aligned, reset.
/// `anim_frame`/`animations` (Task 16): pulso crítico quando `remaining < 10.0`
/// — coexiste com o blink da sidebar (Task 10), não o substitui.
fn gauge_line(
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
    let mut spans = vec![Span::styled(
        format!("{:<8}", label),
        Style::default().fg(to_ratatui(ColorToken::Muted)),
    )];
    spans.extend(gauge_spans(w.remaining, gauge_w, color));
    spans.push(Span::styled(
        format!(" {:>4.0}%", w.remaining),
        Style::default().fg(to_ratatui(ColorToken::TextBright)),
    ));
    spans.push(Span::styled(
        format!("  \u{2192} {}", reset_str),
        Style::default().fg(to_ratatui(ColorToken::Comment)),
    ));
    Line::from(spans)
}

/// Renderiza um card de provider dentro do buffer virtual do ScrollView.
/// `card_rect` já está em coordenadas virtuais (origem 0,0 do ScrollView).
fn render_provider_card(
    state: &AppState,
    pv: &ProviderView,
    idx: usize,
    card_rect: Rect,
    now: Option<time::OffsetDateTime>,
    sv: &mut ScrollView,
) {
    let q = &pv.quota;
    let brand = hex_to_color(provider_hex(&q.provider));
    let fetch_pending = state.fetch_pending.iter().any(|p| p == &q.provider);
    let logged = login_state_for(Some(q), fetch_pending);

    let title = match &q.plan {
        Some(p) => format!(" {} \u{b7} {} ", q.display_name, p),
        None => format!(" {} ", q.display_name),
    };

    let hovered = state.hover == Some(MouseTarget::Card(idx));
    let border_color = card_border_color(hovered);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border_color))
        .title(Span::styled(
            title,
            Style::default().fg(brand).add_modifier(Modifier::BOLD),
        ))
        .title(Line::from(login_status_span(logged, state.glyph_mode)).alignment(Alignment::Right));

    let inner = block.inner(card_rect);
    sv.render_widget(block, card_rect);

    let lines: Vec<Line<'static>> = if logged == LoginState::LoggedOut {
        vec![Line::from(Span::styled(
            " sem sess\u{e3}o \u{2014} clique aqui ou g para logar",
            Style::default().fg(to_ratatui(ColorToken::Muted)),
        ))]
    } else {
        let gauge_w = derive_gauge_width(card_rect.width);
        let mut lines = Vec::with_capacity(3);
        if let Some(w) = &q.primary {
            lines.push(gauge_line(
                "sess\u{e3}o",
                w,
                gauge_w,
                state.anim_frame,
                state.animations,
            ));
        }
        if let Some(w) = &q.secondary {
            lines.push(gauge_line(
                "semana",
                w,
                gauge_w,
                state.anim_frame,
                state.animations,
            ));
        }

        let series = match now {
            Some(now) => state
                .history
                .as_deref()
                .map(|r| provider_series_24h(r, &q.provider, now))
                .unwrap_or_default(),
            None => Vec::new(),
        };
        let spark = sparkline_str(&series);
        let cost_str = state
            .usage
            .as_ref()
            .and_then(|s| s.providers.iter().find(|pu| pu.provider == q.provider))
            .map(fmt_provider_cost)
            .unwrap_or_else(|| "-".to_string());

        lines.push(Line::from(vec![
            Span::styled(
                format!(" {spark}"),
                Style::default().fg(to_ratatui(ColorToken::Comment)),
            ),
            Span::styled(
                format!("  {cost_str} hoje"),
                Style::default().fg(to_ratatui(ColorToken::Muted)),
            ),
        ]));
        lines
    };

    sv.render_widget(Paragraph::new(lines), inner);
}

// ---------------------------------------------------------------------------
// Estados especiais
// ---------------------------------------------------------------------------

/// Skeleton: 3 cards com trilhos EmptyTrack (gauge em 0%) + spinner no 1º
/// card. Mostrado enquanto `state.providers` está vazio mas há fetch em voo.
fn render_skeleton(state: &AppState, frame: &mut Frame, area: Rect) {
    const N: u16 = 3;
    let mut y = area.y;
    for i in 0..N {
        if y + CARD_H > area.y + area.height {
            break;
        }
        let card_rect = Rect::new(area.x, y, area.width, CARD_H);
        render_skeleton_card(state, frame, card_rect, i == 0);
        y += CARD_H;
    }
}

fn render_skeleton_card(state: &AppState, frame: &mut Frame, rect: Rect, show_spinner: bool) {
    let mut block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(to_ratatui(ColorToken::Comment)))
        .title(Span::styled(
            " carregando\u{2026} ",
            Style::default().fg(to_ratatui(ColorToken::Muted)),
        ));

    if show_spinner {
        let throbber_widget = Throbber::default()
            .throbber_set(BRAILLE_SIX)
            .throbber_style(Style::default().fg(to_ratatui(ColorToken::Cyan)))
            .use_type(throbber_widgets_tui::WhichUse::Spin);
        let mut throbber_state = ThrobberState::default();
        for _ in 0..state.throbber.index {
            throbber_state.calc_next();
        }
        block = block.title(
            Line::from(throbber_widget.to_symbol_span(&throbber_state)).alignment(Alignment::Right),
        );
    }

    let inner = block.inner(rect);
    frame.render_widget(block, rect);

    let gauge_w = derive_gauge_width(rect.width);
    let lines = vec![
        skeleton_gauge_line("sess\u{e3}o", gauge_w),
        skeleton_gauge_line("semana", gauge_w),
    ];
    frame.render_widget(Paragraph::new(lines), inner);
}

fn skeleton_gauge_line(label: &str, gauge_w: usize) -> Line<'static> {
    let mut spans = vec![Span::styled(
        format!("{:<8}", label),
        Style::default().fg(to_ratatui(ColorToken::Muted)),
    )];
    spans.extend(gauge_spans(0.0, gauge_w, to_ratatui(ColorToken::Comment)));
    Line::from(spans)
}

/// Zero providers habilitados e nenhum fetch em voo: linha instrutiva.
fn render_empty_state(frame: &mut Frame, area: Rect) {
    let p = Paragraph::new(Line::from(Span::styled(
        " nenhum provider habilitado \u{2014} veja a tela Waybar",
        Style::default().fg(to_ratatui(ColorToken::Muted)),
    )));
    frame.render_widget(p, area);
}

// ---------------------------------------------------------------------------
// Rodapé de chips
// ---------------------------------------------------------------------------

fn render_footer_chips(frame: &mut Frame, area: Rect, hits: &mut HitMap) {
    let chips: [(ChipKind, &str, &str); 4] = [
        (ChipKind::Open, "\u{21b5}", "abrir"),
        (ChipKind::Refresh, "r", "atualizar"),
        (ChipKind::Help, "?", "ajuda"),
        (ChipKind::Quit, "q", "sair"),
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
    use crate::providers::types::{ProviderQuota, QuotaWindow};
    use crate::tui::state::{FetchStatus, ProviderView};
    use crate::usage::amp::AmpDollars;
    use crate::usage::{Cost, ProviderUsage, UsageSummary};

    fn make_quota(
        id: &str,
        display: &str,
        remaining: f64,
        resets_at: Option<&str>,
        error: Option<&str>,
    ) -> ProviderQuota {
        ProviderQuota {
            provider: id.to_string(),
            display_name: display.to_string(),
            available: error.is_none(),
            account: None,
            plan: Some("Max 5x".to_string()),
            plan_type: None,
            primary: Some(QuotaWindow {
                remaining,
                resets_at: resets_at.map(|s| s.to_string()),
                window_minutes: Some(300),
                used: Some(100.0 - remaining),
                severity: None,
            }),
            secondary: Some(QuotaWindow {
                remaining: (remaining + 10.0).min(100.0),
                resets_at: Some("2026-06-22T00:00:00Z".to_string()),
                window_minutes: None,
                used: None,
                severity: None,
            }),
            models: None,
            extra: None,
            error: error.map(|s| s.to_string()),
        }
    }

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
                    by_model: vec![],
                    amp_dollars: None,
                },
                ProviderUsage {
                    provider: "codex".to_string(),
                    total_input: 500_000,
                    total_output: 80_000,
                    total_cache_read: 0,
                    total_cache_write: 0,
                    cost: None,
                    by_model: vec![],
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

    fn rec(provider: &str, ts: time::OffsetDateTime, tokens: u64) -> crate::usage::UsageRecord {
        crate::usage::UsageRecord {
            provider: provider.into(),
            model: Some("m".into()),
            input: tokens,
            output: 0,
            cache_read: 0,
            cache_write: 0,
            ts,
        }
    }

    #[test]
    fn overview_all_ok() {
        use time::macros::datetime;
        let backend = ratatui::backend::TestBackend::new(100, 32);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        let mut state = AppState::new();
        let now = datetime!(2026-06-19 20:00:00 UTC);
        state.last_update = Some(now);
        state.providers = vec![
            ProviderView::new(make_quota(
                "claude",
                "Claude",
                26.0,
                Some("2026-06-19T23:00:00Z"),
                None,
            )),
            ProviderView::new(make_quota(
                "codex",
                "Codex",
                55.0,
                Some("2026-06-20T01:28:00Z"),
                None,
            )),
            ProviderView::new(make_quota("amp", "Amp", 80.0, None, None)),
        ];
        state.status = FetchStatus::Loaded;
        // display_cost (T16): header agora mostra o count-up, não
        // usage.total_cost.usd direto — sem isto, o header ficaria em
        // "$0.00" (default de AppState::new()) em vez do custo real.
        let usage = fake_usage();
        state.display_cost = usage.total_cost.usd;
        state.usage = Some(usage);
        // Séries distintas por provider — sparklines não devem ficar iguais.
        state.history = Some(vec![
            rec("claude", now - time::Duration::hours(1), 900_000),
            rec("claude", now - time::Duration::hours(5), 100_000),
            rec("codex", now - time::Duration::hours(2), 50_000),
            rec("amp", now - time::Duration::hours(3), 300_000),
        ]);
        terminal
            .draw(|f| {
                let area = f.area();
                render_dashboard(&state, f, area, &mut HitMap::default());
            })
            .unwrap();
        insta::assert_snapshot!(terminal.backend());
    }

    #[test]
    fn overview_codex_logged_out() {
        let backend = ratatui::backend::TestBackend::new(100, 32);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        let mut state = AppState::new();
        state.providers = vec![
            ProviderView::new(make_quota(
                "claude",
                "Claude",
                26.0,
                Some("2026-06-19T23:00:00Z"),
                None,
            )),
            ProviderView::new(make_quota(
                "codex",
                "Codex",
                0.0,
                None,
                Some("Not logged in. Open `agent-bar menu` and choose Provider login."),
            )),
            ProviderView::new(make_quota("amp", "Amp", 80.0, None, None)),
        ];
        state.status = FetchStatus::Loaded;
        terminal
            .draw(|f| {
                let area = f.area();
                render_dashboard(&state, f, area, &mut HitMap::default());
            })
            .unwrap();
        insta::assert_snapshot!(terminal.backend());
    }

    #[test]
    fn overview_loading_skeleton() {
        let backend = ratatui::backend::TestBackend::new(100, 32);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        let mut state = AppState::new();
        state.fetch_pending = vec!["claude".to_string(), "codex".to_string(), "amp".to_string()];
        terminal
            .draw(|f| {
                let area = f.area();
                render_dashboard(&state, f, area, &mut HitMap::default());
            })
            .unwrap();
        insta::assert_snapshot!(terminal.backend());
    }

    #[test]
    fn overview_empty_no_providers_no_fetch() {
        let backend = ratatui::backend::TestBackend::new(100, 32);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        let state = AppState::new();
        terminal
            .draw(|f| {
                let area = f.area();
                render_dashboard(&state, f, area, &mut HitMap::default());
            })
            .unwrap();
        insta::assert_snapshot!(terminal.backend());
    }

    #[test]
    fn card_border_color_reflects_hover() {
        // A cor de hover é decidida por uma função pura (`card_border_color`)
        // justamente pra ser testável direto: um snapshot de TestBackend
        // serializa só caracteres (`buffer_view` não exporta estilo/cor) —
        // o snapshot abaixo (`card_hover_state_renders_without_panic`) é
        // idêntico com ou sem hover, então NÃO cobre a cor.
        assert_eq!(card_border_color(true), to_ratatui(ColorToken::Text));
        assert_eq!(card_border_color(false), to_ratatui(ColorToken::Comment));
    }

    #[test]
    fn card_hover_state_renders_without_panic() {
        // Cobertura estrutural apenas (layout/panic) — a cor de hover é
        // verificada em `card_border_color_reflects_hover`, não aqui.
        let backend = ratatui::backend::TestBackend::new(100, 32);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        let mut state = AppState::new();
        state.providers = vec![ProviderView::new(make_quota(
            "claude",
            "Claude",
            26.0,
            Some("2026-06-19T23:00:00Z"),
            None,
        ))];
        state.hover = Some(MouseTarget::Card(0));
        terminal
            .draw(|f| {
                let area = f.area();
                render_dashboard(&state, f, area, &mut HitMap::default());
            })
            .unwrap();
        insta::assert_snapshot!(terminal.backend());
    }

    #[test]
    fn card_registers_mouse_hit_zone() {
        let backend = ratatui::backend::TestBackend::new(100, 32);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        let mut state = AppState::new();
        state.providers = vec![
            ProviderView::new(make_quota("claude", "Claude", 26.0, None, None)),
            ProviderView::new(make_quota("codex", "Codex", 50.0, None, None)),
        ];
        let mut hits = HitMap::default();
        terminal
            .draw(|f| {
                let area = f.area();
                render_dashboard(&state, f, area, &mut hits);
            })
            .unwrap();
        // Primeiro card começa logo no topo da área de conteúdo (y=0).
        assert_eq!(hits.at(1, 0), Some(MouseTarget::Card(0)));
        // Segundo card começa em y=6 (CARD_H).
        assert_eq!(hits.at(1, 6), Some(MouseTarget::Card(1)));
    }

    #[test]
    fn scroll_offset_clips_cards_and_shifts_hit_zones() {
        // 5 providers (virtual: card i em [6i, 6i+6)) num viewport de 10
        // linhas com scroll=8 → visível é a janela virtual [8, 18):
        //   card0 [0,6)   totalmente acima do scroll → sem hit
        //   card1 [6,12)  parcialmente visível: tela [0,4)
        //   card2 [12,18) totalmente visível: tela [4,10)
        //   card3 [18,24) começa exatamente na borda inferior → sem hit
        //   card4 [24,30) totalmente abaixo → sem hit
        let backend = ratatui::backend::TestBackend::new(100, 11); // 10 conteúdo + 1 rodapé
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        let mut state = AppState::new();
        state.providers = (0..5)
            .map(|i| {
                ProviderView::new(make_quota(
                    &format!("p{i}"),
                    &format!("P{i}"),
                    50.0,
                    None,
                    None,
                ))
            })
            .collect();
        state.scroll = 8;
        let mut hits = HitMap::default();
        terminal
            .draw(|f| {
                let area = f.area();
                render_dashboard(&state, f, area, &mut hits);
            })
            .unwrap();

        assert_eq!(hits.at(1, 0), Some(MouseTarget::Card(1)));
        assert_eq!(hits.at(1, 3), Some(MouseTarget::Card(1)));
        assert_eq!(hits.at(1, 4), Some(MouseTarget::Card(2)));
        assert_eq!(hits.at(1, 9), Some(MouseTarget::Card(2)));
        // Nenhuma linha do viewport de conteúdo pertence a card0/3/4.
        for y in 0..10u16 {
            let t = hits.at(1, y);
            assert!(
                t == Some(MouseTarget::Card(1)) || t == Some(MouseTarget::Card(2)),
                "y={y} inesperado: {t:?}"
            );
        }
    }

    #[test]
    fn footer_registers_chip_hits() {
        let backend = ratatui::backend::TestBackend::new(100, 32);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        let state = AppState::new();
        let mut hits = HitMap::default();
        terminal
            .draw(|f| {
                let area = f.area();
                render_dashboard(&state, f, area, &mut hits);
            })
            .unwrap();
        // Chips ficam na última linha da área renderizada.
        let found = (0..100u16).any(|x| hits.at(x, 31) == Some(MouseTarget::Chip(ChipKind::Quit)));
        assert!(found, "esperava chip [q] registrado na última linha");
    }

    // quota_bar_logic (assertion de gauge 100%/0% via quota_bar_pub) já
    // existe em `render/mod.rs` — não duplicar aqui.

    // ---- Motion: pulse crítico no card (Task 16) ----

    /// Provider crítico: `remaining` < 10.0 dispara `pulse_color` no gauge
    /// de sessão do card (via `gauge_line`). Confirma o CALL SITE real
    /// (não só `pulse_color` isolado, já coberto em
    /// `widgets::quota_gauge::tests`) — o buffer inteiro do card deve
    /// diferir entre dois `anim_frame` distintos quando `animations=true`.
    #[test]
    fn critical_card_gauge_pulses_across_anim_frames_when_animations_on() {
        let backend = ratatui::backend::TestBackend::new(100, 20);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        let mut state = AppState::new();
        state.providers = vec![ProviderView::new(make_quota(
            "claude", "Claude", 5.0, None, None,
        ))];
        state.status = FetchStatus::Loaded;
        state.animations = true;

        state.anim_frame = 0;
        terminal
            .draw(|f| {
                let area = f.area();
                render_dashboard(&state, f, area, &mut HitMap::default());
            })
            .unwrap();
        let buf_frame0 = terminal.backend().buffer().clone();

        state.anim_frame = 18; // ~metade do ciclo de 37 ticks do pulso
        terminal
            .draw(|f| {
                let area = f.area();
                render_dashboard(&state, f, area, &mut HitMap::default());
            })
            .unwrap();
        let buf_frame18 = terminal.backend().buffer().clone();

        assert_ne!(
            buf_frame0, buf_frame18,
            "gauge crítico deveria pulsar (cor diferente) entre anim_frame 0 e 18"
        );
    }

    /// Self-review do brief: animations=false → zero lerp visual — o pulso
    /// não deve alterar UM ÚNICO byte do buffer entre `anim_frame`s
    /// distintos (mesmo provider crítico do teste acima).
    #[test]
    fn critical_card_gauge_stays_static_when_animations_off() {
        let backend = ratatui::backend::TestBackend::new(100, 20);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        let mut state = AppState::new();
        state.providers = vec![ProviderView::new(make_quota(
            "claude", "Claude", 5.0, None, None,
        ))];
        state.status = FetchStatus::Loaded;
        state.animations = false;

        state.anim_frame = 0;
        terminal
            .draw(|f| {
                let area = f.area();
                render_dashboard(&state, f, area, &mut HitMap::default());
            })
            .unwrap();
        let buf_frame0 = terminal.backend().buffer().clone();

        state.anim_frame = 18;
        terminal
            .draw(|f| {
                let area = f.area();
                render_dashboard(&state, f, area, &mut HitMap::default());
            })
            .unwrap();
        let buf_frame18 = terminal.backend().buffer().clone();

        assert_eq!(
            buf_frame0, buf_frame18,
            "com animations=false, o pulso não deve alterar nada entre anim_frames"
        );
    }
}
