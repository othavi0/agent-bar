pub mod config;
pub mod dashboard;
pub mod detail;
pub mod history;
pub mod login;
pub mod status_bar;

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, BorderType, Borders, List, ListItem};
use ratatui::Frame;
use throbber_widgets_tui::{Throbber, ThrobberState, BRAILLE_SIX};
use tui_popup::Popup;

use crate::theme::ColorToken;
use crate::tui::mouse::{HitMap, MouseTarget};
use crate::tui::state::{AppState, FetchStatus, Screen};
use crate::tui::theme_bridge::to_ratatui;
use crate::tui::widgets::provider_list::provider_list_item;

use self::config::render_config;
use self::dashboard::render_dashboard;
use self::detail::render_detail;
use self::history::render_history;
use self::login::render_login;
use self::status_bar::render_status_bar;

/// Constroi o conteudo do overlay de ajuda (atalhos de teclado).
fn help_text() -> Text<'static> {
    Text::from(vec![
        Line::from(" Navegacao global ").centered(),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "  [?] / Esc  ",
                Style::default()
                    .fg(to_ratatui(ColorToken::TextBright))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "abre/fecha esta ajuda",
                Style::default().fg(to_ratatui(ColorToken::Text)),
            ),
        ]),
        Line::from(vec![
            Span::styled(
                "  up/down    ",
                Style::default()
                    .fg(to_ratatui(ColorToken::TextBright))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "mover selecao na sidebar",
                Style::default().fg(to_ratatui(ColorToken::Text)),
            ),
        ]),
        Line::from(vec![
            Span::styled(
                "  Enter      ",
                Style::default()
                    .fg(to_ratatui(ColorToken::TextBright))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "ativar item selecionado",
                Style::default().fg(to_ratatui(ColorToken::Text)),
            ),
        ]),
        Line::from(vec![
            Span::styled(
                "  h / g / w  ",
                Style::default()
                    .fg(to_ratatui(ColorToken::TextBright))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "ir para Historico / Login / Waybar",
                Style::default().fg(to_ratatui(ColorToken::Text)),
            ),
        ]),
        Line::from(vec![
            Span::styled(
                "  [q]        ",
                Style::default()
                    .fg(to_ratatui(ColorToken::TextBright))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("sair", Style::default().fg(to_ratatui(ColorToken::Text))),
        ]),
        Line::from(vec![
            Span::styled(
                "  [r]        ",
                Style::default()
                    .fg(to_ratatui(ColorToken::TextBright))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "atualizar quotas",
                Style::default().fg(to_ratatui(ColorToken::Text)),
            ),
        ]),
        Line::from(""),
        Line::from(" Overview ").centered(),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "  up/down    ",
                Style::default()
                    .fg(to_ratatui(ColorToken::TextBright))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "selecionar provider",
                Style::default().fg(to_ratatui(ColorToken::Text)),
            ),
        ]),
        Line::from(vec![
            Span::styled(
                "  Enter      ",
                Style::default()
                    .fg(to_ratatui(ColorToken::TextBright))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "abrir detalhe",
                Style::default().fg(to_ratatui(ColorToken::Text)),
            ),
        ]),
        Line::from(vec![
            Span::styled(
                "  Esc        ",
                Style::default()
                    .fg(to_ratatui(ColorToken::TextBright))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "voltar para lista",
                Style::default().fg(to_ratatui(ColorToken::Text)),
            ),
        ]),
        Line::from(""),
        Line::from(" Waybar Config ").centered(),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "  up/down    ",
                Style::default()
                    .fg(to_ratatui(ColorToken::TextBright))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "selecionar campo",
                Style::default().fg(to_ratatui(ColorToken::Text)),
            ),
        ]),
        Line::from(vec![
            Span::styled(
                "  Enter      ",
                Style::default()
                    .fg(to_ratatui(ColorToken::TextBright))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "editar campo",
                Style::default().fg(to_ratatui(ColorToken::Text)),
            ),
        ]),
        Line::from(vec![
            Span::styled(
                "  [s]        ",
                Style::default()
                    .fg(to_ratatui(ColorToken::TextBright))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "salvar configuracao",
                Style::default().fg(to_ratatui(ColorToken::Text)),
            ),
        ]),
        Line::from(""),
        Line::from(" Login ").centered(),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "  up/down    ",
                Style::default()
                    .fg(to_ratatui(ColorToken::TextBright))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "selecionar provider",
                Style::default().fg(to_ratatui(ColorToken::Text)),
            ),
        ]),
        Line::from(vec![
            Span::styled(
                "  Enter      ",
                Style::default()
                    .fg(to_ratatui(ColorToken::TextBright))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "iniciar login do provider",
                Style::default().fg(to_ratatui(ColorToken::Text)),
            ),
        ]),
        Line::from(""),
    ])
}

/// Top-level render: lays out the full TUI and dispatches to sub-renders.
///
/// `hits` acumula as zonas clicaveis do frame atual (Task 9) — o event_loop
/// consulta via `HitMap::at` ao processar `MouseEvent`. O caller e
/// responsavel por `hits.clear()` antes de cada `terminal.draw` (render nao
/// limpa sozinho: um HitMap vazio silenciosamente sem clear acumularia
/// zonas obsoletas de frames anteriores).
pub fn render(state: &AppState, frame: &mut Frame, hits: &mut HitMap) {
    let area = frame.area();

    // Vertical split: [body (fill), status_bar (1)]. Sem tab bar — navegacao
    // e via sidebar unica (Task 8); o layout visual completo e a Task 10.
    let vert = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(area);

    let body_area = vert[0];
    let status_area = vert[1];

    render_body(state, frame, body_area, hits);
    render_status_bar(state, frame, status_area);

    // Overlay de ajuda: renderizado por cima de tudo quando show_help=true.
    if state.show_help {
        // Fundo One Dark (#282c34) para contraste com o conteudo da tela.
        let bg = ratatui::style::Color::Rgb(0x28, 0x2c, 0x34);
        let content = help_text();
        let popup = Popup::new(content)
            .title(Line::from(Span::styled(
                " agent-bar — atalhos ",
                Style::default()
                    .fg(to_ratatui(ColorToken::Blue))
                    .add_modifier(Modifier::BOLD),
            )))
            .style(Style::default().bg(bg).fg(to_ratatui(ColorToken::Text)))
            .border_style(Style::default().fg(to_ratatui(ColorToken::Blue)));
        frame.render_widget(popup, area);
    }
}

/// Renders the body: sidebar (providers) + content panel.
fn render_body(state: &AppState, frame: &mut Frame, area: Rect, hits: &mut HitMap) {
    // Horizontal split: [sidebar (17), content (fill)]
    let horiz = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(17), Constraint::Min(0)])
        .split(area);

    let sidebar_area = horiz[0];
    let content_area = horiz[1];

    render_sidebar(state, frame, sidebar_area, hits);
    render_content(state, frame, content_area);
}

/// Renders the provider sidebar.
///
/// Registra no HitMap uma zona por linha visivel do painel PROVIDERS (Task
/// 9). A sidebar visual atual so lista providers (nao Overview/History/
/// Login/Waybar — isso e o redesign da Task 10); o indice logico registrado
/// e `sidebar_items()[i+1]` (o item na posicao `i+1` porque `Overview`
/// ocupa a posicao 0 sem ter uma linha propria hoje), consistente com o
/// espaco de indices que `Action::Click(MouseTarget::Sidebar(_))` consome.
fn render_sidebar(state: &AppState, frame: &mut Frame, area: Rect, hits: &mut HitMap) {
    // Sidebar sempre em foco (nao ha mais painel de conteudo com foco
    // proprio nesta versao minima — Task 10 redesenha o layout completo).
    let border_color = to_ratatui(ColorToken::Blue);

    let title_style = Style::default()
        .fg(to_ratatui(ColorToken::TextBright))
        .add_modifier(Modifier::BOLD);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Thick)
        .border_style(Style::default().fg(border_color))
        .title(Span::styled("PROVIDERS", title_style));

    // Area interna (sem as bordas) — usada abaixo para calcular a linha de
    // cada zona clicavel. Calculado ANTES de `block` ser movido pro `List`.
    let inner = block.inner(area);

    // Animação D (pulse crítico): blink lento ~450ms.
    // 30ms/tick → 450ms = 15 ticks. Visível nos primeiros 7-8 ticks, dim nos seguintes.
    let blink_visible = (state.anim_frame / 15) % 2 == 0;

    let items: Vec<ListItem<'_>> = {
        let mut v: Vec<ListItem<'_>> = state
            .providers
            .iter()
            .enumerate()
            .map(|(i, pv)| {
                let remaining = pv
                    .quota
                    .primary
                    .as_ref()
                    .map(|w| w.remaining)
                    .unwrap_or(0.0);
                let is_critical = remaining < 10.0;
                provider_list_item(pv, i == state.selected, is_critical, blink_visible)
            })
            .collect();

        // Animação C (throbber): indicador de carregamento enquanto Loading.
        match &state.status {
            FetchStatus::Loading => {
                // Constrói o throbber braille com o índice do estado de animação.
                let throbber_widget = Throbber::default()
                    .throbber_set(BRAILLE_SIX)
                    .throbber_style(
                        Style::default()
                            .fg(to_ratatui(ColorToken::Cyan))
                            .add_modifier(Modifier::BOLD),
                    )
                    .use_type(throbber_widgets_tui::WhichUse::Spin);
                let mut throbber_state = ThrobberState::default();
                // Sincroniza o índice com o estado de animação do AppState.
                for _ in 0..state.throbber.index {
                    throbber_state.calc_next();
                }
                let symbol_span = throbber_widget.to_symbol_span(&throbber_state);
                // Prefixo " " para alinhamento com os itens da lista.
                let mut spans = vec![Span::raw(" "), symbol_span];
                // Progresso por provider (Task 5): lista os ids ainda em voo
                // enquanto o fetch assíncrono roda em thread própria.
                if !state.fetch_pending.is_empty() {
                    spans.push(Span::styled(
                        format!(" atualizando: {}", state.fetch_pending.join(" ")),
                        Style::default().fg(to_ratatui(ColorToken::Comment)),
                    ));
                }
                let line = Line::from(spans);
                v.push(ListItem::new(line));
            }
            FetchStatus::Failed(_) => {
                v.push(ListItem::new(Span::styled(
                    " err",
                    Style::default().fg(to_ratatui(ColorToken::Red)),
                )));
            }
            _ => {}
        }

        v
    };

    // Zonas clicaveis: uma por linha de provider visivel (a lista nao tem
    // scroll — itens alem de `inner.height` sao clipados pelo widget e nao
    // ficam clicaveis). A linha da throbber/err (acima) nao registra zona:
    // nao tem MouseTarget correspondente ainda.
    for (i, _) in state.providers.iter().enumerate() {
        if (i as u16) >= inner.height {
            break;
        }
        let row = Rect::new(inner.x, inner.y + i as u16, inner.width, 1);
        // sidebar_items() = [Overview, Provider(0), Provider(1), ..., History,
        // Login, Waybar]; a linha visual `i` do provider corresponde ao
        // indice logico `i + 1` (Overview ocupa o indice 0 sem linha propria).
        hits.push(row, MouseTarget::Sidebar(i + 1));
    }

    let list = List::new(items).block(block);
    frame.render_widget(list, area);
}

/// Dispatches content rendering based on the current screen.
fn render_content(state: &AppState, frame: &mut Frame, area: ratatui::layout::Rect) {
    match state.screen {
        Screen::Overview => render_dashboard(state, frame, area),
        Screen::Detail => render_detail(state, frame, area),
        Screen::History => render_history(state, frame, area),
        Screen::Login => render_login(state, None, frame, area),
        Screen::Waybar => render_config(state, frame, area),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::types::{ProviderQuota, QuotaWindow};
    use crate::tui::state::{FetchStatus, ProviderView};
    use crate::usage::amp::AmpDollars;
    use crate::usage::{Cost, ModelUsage, ProviderUsage, UsageSummary};

    fn make_quota(
        id: &str,
        display: &str,
        remaining: f64,
        resets_at: Option<&str>,
    ) -> ProviderQuota {
        ProviderQuota {
            provider: id.to_string(),
            display_name: display.to_string(),
            available: true,
            account: None,
            plan: None,
            plan_type: None,
            primary: Some(QuotaWindow {
                remaining,
                resets_at: resets_at.map(|s| s.to_string()),
                window_minutes: Some(300),
                used: Some(100.0 - remaining),
                severity: None,
            }),
            secondary: None,
            models: None,
            extra: None,
            error: None,
        }
    }

    /// Constroi um UsageSummary falso para testes de dashboard:
    /// - claude: $2.10 / R$11.55
    /// - codex: tokens sem custo conhecido (cost None)
    /// - amp: amp_dollars (remaining $4.19)
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
                    by_model: vec![ModelUsage {
                        model: "claude-opus-4-8".to_string(),
                        input: 800_000,
                        output: 100_000,
                        cache_read: 0,
                        cache_write: 0,
                        cost: Some(Cost {
                            usd: 1.40,
                            brl: 7.70,
                        }),
                    }],
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

    #[test]
    fn dashboard_renders_providers_table() {
        let backend = ratatui::backend::TestBackend::new(64, 20);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        let mut state = AppState::new();
        state.providers = vec![
            ProviderView::new(make_quota(
                "claude",
                "Claude",
                26.0,
                Some("2026-06-19T23:00:00Z"),
            )),
            ProviderView::new(make_quota(
                "codex",
                "Codex",
                1.0,
                Some("2026-06-20T01:28:00Z"),
            )),
            ProviderView::new(make_quota("amp", "Amp", 0.0, None)),
        ];
        state.status = FetchStatus::Loaded;
        terminal
            .draw(|f| render(&state, f, &mut HitMap::default()))
            .unwrap();
        insta::assert_snapshot!(terminal.backend());
    }

    #[test]
    fn dashboard_renders_with_real_cost() {
        let backend = ratatui::backend::TestBackend::new(64, 20);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        let mut state = AppState::new();
        state.providers = vec![
            ProviderView::new(make_quota(
                "claude",
                "Claude",
                26.0,
                Some("2026-06-19T23:00:00Z"),
            )),
            ProviderView::new(make_quota(
                "codex",
                "Codex",
                1.0,
                Some("2026-06-20T01:28:00Z"),
            )),
            ProviderView::new(make_quota("amp", "Amp", 0.0, None)),
        ];
        state.status = FetchStatus::Loaded;
        state.usage = Some(fake_usage());
        terminal
            .draw(|f| render(&state, f, &mut HitMap::default()))
            .unwrap();
        insta::assert_snapshot!(terminal.backend());
    }

    #[test]
    fn quota_bar_logic() {
        // 100% remaining = all filled (nothing consumed)
        let bar_100 = dashboard::quota_bar_pub(100.0);
        // 0% remaining = all empty (fully consumed)
        let bar_0 = dashboard::quota_bar_pub(0.0);
        assert_eq!(
            bar_100,
            "\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}"
        ); // all filled
        assert_eq!(
            bar_0,
            "\u{2592}\u{2592}\u{2592}\u{2592}\u{2592}\u{2592}\u{2592}"
        ); // all empty (trilho ▒)
    }

    #[test]
    fn help_overlay_renders_snapshot() {
        // Terminal largo para acomodar o popup centralizado sem truncamento.
        let backend = ratatui::backend::TestBackend::new(80, 35);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        let mut state = AppState::new();
        state.show_help = true;
        terminal
            .draw(|f| render(&state, f, &mut HitMap::default()))
            .unwrap();
        insta::assert_snapshot!(terminal.backend());
    }

    #[test]
    fn dashboard_renders_wide_160() {
        let backend = ratatui::backend::TestBackend::new(160, 40);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        let mut state = AppState::new();
        state.providers = vec![
            ProviderView::new(make_quota(
                "claude",
                "Claude",
                26.0,
                Some("2026-06-19T23:00:00Z"),
            )),
            ProviderView::new(make_quota(
                "codex",
                "Codex",
                1.0,
                Some("2026-06-20T01:28:00Z"),
            )),
            ProviderView::new(make_quota("amp", "Amp", 0.0, None)),
        ];
        state.status = FetchStatus::Loaded;
        state.usage = Some(fake_usage());
        terminal
            .draw(|f| render(&state, f, &mut HitMap::default()))
            .unwrap();
        insta::assert_snapshot!(terminal.backend());
    }

    #[test]
    fn render_registers_sidebar_hit_zones() {
        let backend = ratatui::backend::TestBackend::new(64, 20);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        let mut state = AppState::new();
        state.providers = vec![
            ProviderView::new(make_quota("claude", "Claude", 26.0, None)),
            ProviderView::new(make_quota("codex", "Codex", 1.0, None)),
        ];
        state.status = FetchStatus::Loaded;
        let mut hits = HitMap::default();
        terminal.draw(|f| render(&state, f, &mut hits)).unwrap();

        // Sidebar: x=0, largura 17, borda ALL -> inner comeca em (1,1).
        // Linha visual 0 (claude) = indice logico 1 (Overview ocupa 0).
        assert_eq!(hits.at(1, 1), Some(MouseTarget::Sidebar(1)));
        // Linha visual 1 (codex) = indice logico 2.
        assert_eq!(hits.at(1, 2), Some(MouseTarget::Sidebar(2)));
        // Sem 3o provider — nao ha zona ali.
        assert_eq!(hits.at(1, 3), None);
        // Fora da sidebar inteiramente (coluna do content_area).
        assert_eq!(hits.at(30, 1), None);
    }
}
