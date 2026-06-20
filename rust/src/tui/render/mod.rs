pub mod config;
pub mod dashboard;
pub mod detail;
pub mod history;
pub mod login;
pub mod status_bar;

use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, List, ListItem, Tabs};
use ratatui::Frame;
use throbber_widgets_tui::{Throbber, ThrobberState, BRAILLE_SIX};

use crate::theme::ColorToken;
use crate::tui::state::{AppState, FetchStatus, Panel};
use crate::tui::theme_bridge::to_ratatui;
use crate::tui::widgets::provider_list::provider_list_item;

use self::config::render_config;
use self::dashboard::render_dashboard;
use self::detail::render_detail;
use self::history::render_history;
use self::login::render_login;
use self::status_bar::render_status_bar;

/// Top-level render: lays out the full TUI and dispatches to sub-renders.
pub fn render(state: &AppState, frame: &mut Frame) {
    let area = frame.area();

    // Vertical split: [tab_bar (3), body (fill), status_bar (1)]
    let vert = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(area);

    let tab_area = vert[0];
    let body_area = vert[1];
    let status_area = vert[2];

    render_tab_bar(state, frame, tab_area);
    render_body(state, frame, body_area);
    render_status_bar(state, frame, status_area);
}

/// Renders the tab bar at the top.
fn render_tab_bar(state: &AppState, frame: &mut Frame, area: ratatui::layout::Rect) {
    let active_style = Style::default()
        .fg(to_ratatui(ColorToken::TextBright))
        .add_modifier(Modifier::BOLD | Modifier::UNDERLINED);
    let inactive_style = Style::default().fg(to_ratatui(ColorToken::Muted));

    let tab_titles: Vec<Line<'_>> = ["Dashboard", "Waybar", "History", "Login"]
        .iter()
        .enumerate()
        .map(|(i, &name)| {
            if i == state.tab.index() {
                Line::from(Span::styled(name, active_style))
            } else {
                Line::from(Span::styled(name, inactive_style))
            }
        })
        .collect();

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Thick)
        .border_style(Style::default().fg(to_ratatui(ColorToken::Comment)))
        .title(Span::styled(
            " agent-bar ",
            Style::default()
                .fg(to_ratatui(ColorToken::Blue))
                .add_modifier(Modifier::BOLD),
        ));

    let tabs = Tabs::new(tab_titles)
        .select(state.tab.index())
        .block(block)
        .highlight_style(active_style)
        .style(inactive_style)
        // Use heavy vertical bar as divider (no VS16 selector)
        .divider(Span::raw("\u{2503}"));

    frame.render_widget(tabs, area);
}

/// Renders the body: sidebar (providers) + content panel.
fn render_body(state: &AppState, frame: &mut Frame, area: ratatui::layout::Rect) {
    // Horizontal split: [sidebar (15), content (fill)]
    let horiz = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(15), Constraint::Min(0)])
        .split(area);

    let sidebar_area = horiz[0];
    let content_area = horiz[1];

    render_sidebar(state, frame, sidebar_area);
    render_content(state, frame, content_area);
}

/// Renders the provider sidebar.
fn render_sidebar(state: &AppState, frame: &mut Frame, area: ratatui::layout::Rect) {
    let sidebar_focused = matches!(state.focus, Panel::Sidebar);
    let border_color = if sidebar_focused {
        to_ratatui(ColorToken::Blue)
    } else {
        to_ratatui(ColorToken::Comment)
    };

    let title_style = Style::default()
        .fg(to_ratatui(ColorToken::TextBright))
        .add_modifier(Modifier::BOLD);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Thick)
        .border_style(Style::default().fg(border_color))
        .title(Span::styled("PROVIDERS", title_style));

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
                let line = Line::from(vec![Span::raw(" "), symbol_span]);
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

    let list = List::new(items).block(block);
    frame.render_widget(list, area);
}

/// Dispatches content rendering based on current tab and mode.
fn render_content(state: &AppState, frame: &mut Frame, area: ratatui::layout::Rect) {
    use crate::tui::state::{Mode, Tab};

    match (&state.tab, &state.mode) {
        (Tab::Dashboard, Mode::Detail) => render_detail(state, frame, area),
        (Tab::Dashboard, _) => render_dashboard(state, frame, area),
        (Tab::Waybar, _) => render_config(state, frame, area),
        (Tab::History, _) => render_history(state, frame, area),
        (Tab::Login, _) => render_login(state, None, frame, area),
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
        terminal.draw(|f| render(&state, f)).unwrap();
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
        terminal.draw(|f| render(&state, f)).unwrap();
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
            "\u{2591}\u{2591}\u{2591}\u{2591}\u{2591}\u{2591}\u{2591}"
        ); // all empty
    }
}
