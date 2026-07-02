pub mod config;
pub mod dashboard;
pub mod detail;
pub mod history;
pub mod login;
pub mod sidebar;

use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, BorderType, Borders};
use ratatui::Frame;
use throbber_widgets_tui::{Throbber, ThrobberState, BRAILLE_SIX};
use tui_popup::Popup;

use crate::theme::ColorToken;
use crate::tui::mouse::HitMap;
use crate::tui::state::{AppState, Screen};
use crate::tui::theme_bridge::to_ratatui;

use self::config::render_config;
use self::dashboard::render_dashboard;
use self::detail::render_detail;
use self::history::render_history;
use self::login::render_login;
use self::sidebar::render_sidebar;

/// Largura abaixo da qual a sidebar colapsa pra so a coluna de marcas.
const NARROW_WIDTH: u16 = 80;

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

/// Título direito da moldura externa: spinner (quando ha fetch em voo) +
/// custo de hoje + relogio da ultima atualizacao.
fn header_status(state: &AppState) -> Line<'static> {
    let mut spans: Vec<Span<'static>> = Vec::new();

    if !state.fetch_pending.is_empty() {
        let throbber_widget = Throbber::default()
            .throbber_set(BRAILLE_SIX)
            .throbber_style(
                Style::default()
                    .fg(to_ratatui(ColorToken::Cyan))
                    .add_modifier(Modifier::BOLD),
            )
            .use_type(throbber_widgets_tui::WhichUse::Spin);
        let mut throbber_state = ThrobberState::default();
        for _ in 0..state.throbber.index {
            throbber_state.calc_next();
        }
        spans.push(throbber_widget.to_symbol_span(&throbber_state));
        spans.push(Span::raw(" \u{b7} "));
    }

    let cost = state
        .usage
        .as_ref()
        .map(|u| format!("${:.2}", u.total_cost.usd))
        .unwrap_or_else(|| "-".to_string());
    spans.push(Span::styled(
        cost,
        Style::default().fg(to_ratatui(ColorToken::TextBright)),
    ));

    if let Some(dt) = state.last_update {
        spans.push(Span::raw(" \u{b7} "));
        spans.push(Span::styled(
            format!("{:02}:{:02}", dt.hour(), dt.minute()),
            Style::default().fg(to_ratatui(ColorToken::Comment)),
        ));
    }

    spans.push(Span::raw(" "));
    Line::from(spans).right_aligned()
}

/// Top-level render: lays out the full TUI and dispatches to sub-renders.
///
/// Moldura externa unica `BorderType::Rounded` com titulo ` agent-bar `
/// (esquerda) + status (direita). Interna: `[sidebar | content]`
/// horizontal — sidebar colapsa pra so a coluna de marcas quando o
/// terminal e mais estreito que `NARROW_WIDTH`.
///
/// `hits` acumula as zonas clicaveis do frame atual (Task 9) — o event_loop
/// consulta via `HitMap::at` ao processar `MouseEvent`. O caller e
/// responsavel por `hits.clear()` antes de cada `terminal.draw` (render nao
/// limpa sozinho: um HitMap vazio silenciosamente sem clear acumularia
/// zonas obsoletas de frames anteriores).
pub fn render(state: &AppState, frame: &mut Frame, hits: &mut HitMap) {
    let area = frame.area();

    let outer = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(to_ratatui(ColorToken::Comment)))
        .title(Span::styled(
            " agent-bar ",
            Style::default()
                .fg(to_ratatui(ColorToken::Blue))
                .add_modifier(Modifier::BOLD),
        ))
        .title(header_status(state));
    let inner = outer.inner(area);
    frame.render_widget(outer, area);

    let sidebar_w: u16 = if area.width < NARROW_WIDTH { 3 } else { 17 };
    let cols = Layout::horizontal([Constraint::Length(sidebar_w), Constraint::Min(0)]).split(inner);

    render_sidebar(state, frame, cols[0], hits);
    match state.screen {
        Screen::Overview => render_dashboard(state, frame, cols[1], hits),
        Screen::Detail => render_detail(state, frame, cols[1], hits),
        Screen::History => render_history(state, frame, cols[1], hits),
        Screen::Login => render_login(state, frame, cols[1], hits),
        Screen::Waybar => render_config(state, frame, cols[1], hits),
    }

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::types::{ProviderQuota, QuotaWindow};
    use crate::tui::mouse::MouseTarget;
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
        // Terminal largo (>=80) para exercitar a sidebar cheia (17 cols) —
        // a colapsada tem teste dedicado em `sidebar_collapses_below_80_cols`.
        let backend = ratatui::backend::TestBackend::new(90, 20);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        let mut state = AppState::new();
        state.providers = vec![
            ProviderView::new(make_quota("claude", "Claude", 26.0, None)),
            ProviderView::new(make_quota("codex", "Codex", 1.0, None)),
        ];
        state.status = FetchStatus::Loaded;
        let mut hits = HitMap::default();
        terminal.draw(|f| render(&state, f, &mut hits)).unwrap();

        // Sidebar nova: TODOS os itens de sidebar_items() (Overview,
        // Provider(0), Provider(1), History, Login, Waybar) tem zona
        // clicavel 1:1 com o indice do cursor — nao so os providers como na
        // sidebar antiga. Borda ALL Rounded -> inner comeca em (1,1);
        // "VISAO" ocupa a 1a linha do inner, entao Overview cai na 2a.
        assert_eq!(hits.at(1, 2), Some(MouseTarget::Sidebar(0))); // Overview
        assert_eq!(hits.at(1, 5), Some(MouseTarget::Sidebar(1))); // claude
        assert_eq!(hits.at(1, 6), Some(MouseTarget::Sidebar(2))); // codex
        assert_eq!(hits.at(1, 9), Some(MouseTarget::Sidebar(3))); // History
        assert_eq!(hits.at(1, 10), Some(MouseTarget::Sidebar(4))); // Login
        assert_eq!(hits.at(1, 11), Some(MouseTarget::Sidebar(5))); // Waybar
        // (50, 5) cai dentro do 1º card da Overview (Task 11: cards
        // registram MouseTarget::Card) — deixou de ser "fora de qualquer
        // zona" desde que o dashboard passou a ser cards clicáveis.
        assert_eq!(hits.at(50, 5), Some(MouseTarget::Card(0)));
        // Fora do frame inteiramente continua sem zona.
        assert_eq!(hits.at(200, 5), None);
    }

    #[test]
    fn sidebar_collapses_below_80_cols() {
        // < NARROW_WIDTH (80) -> sidebar Length(3), so a coluna de marcas.
        let backend = ratatui::backend::TestBackend::new(70, 30);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        let state = AppState::new();
        let mut hits = HitMap::default();
        terminal.draw(|f| render(&state, f, &mut hits)).unwrap();

        // Overview e sempre o 1o item (1a linha do inner e o header VISAO,
        // Overview cai na linha seguinte) — estavel independente do numero
        // de providers. Borda ALL Rounded -> inner comeca em (1,1).
        assert_eq!(hits.at(1, 2), Some(MouseTarget::Sidebar(0)));
        assert_eq!(hits.at(3, 2), Some(MouseTarget::Sidebar(0))); // ultima col da sidebar colapsada (largura 3: x=1..4)
        assert_eq!(hits.at(4, 2), None); // area de conteudo comeca na coluna 4
    }
}
