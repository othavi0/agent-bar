pub mod dashboard;
pub mod status_bar;

use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, List, ListItem, Tabs};
use ratatui::Frame;

use crate::theme::ColorToken;
use crate::tui::state::{AppState, FetchStatus, Panel};
use crate::tui::theme_bridge::{provider_color, to_ratatui};

use self::dashboard::render_dashboard;
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

    let items: Vec<ListItem<'_>> = {
        let mut v: Vec<ListItem<'_>> = state
            .providers
            .iter()
            .enumerate()
            .map(|(i, pv)| {
                let q = &pv.quota;
                let remaining = q.primary.as_ref().map(|w| w.remaining).unwrap_or(0.0);
                let pct = format!("{:3.0}%", remaining);
                let p_color = provider_color(&q.provider);

                // Inner width = 13 (sidebar=15 - 2 borders).
                // Layout: glyph(1) + space(1) + name(7) + pct(4) = 13
                let name_trunc: &str = if q.display_name.len() > 7 {
                    &q.display_name[..7]
                } else {
                    &q.display_name
                };

                if i == state.selected {
                    // Selected: diamond prefix + bold
                    let name_part = format!("\u{25c6} {:<7}", name_trunc);
                    ListItem::new(Line::from(vec![
                        Span::styled(
                            name_part,
                            Style::default().fg(p_color).add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(pct, Style::default().fg(to_ratatui(ColorToken::TextBright))),
                    ]))
                } else {
                    let name_part = format!("\u{25cf} {:<7}", name_trunc);
                    ListItem::new(Line::from(vec![
                        Span::styled(name_part, Style::default().fg(p_color)),
                        Span::styled(pct, Style::default().fg(to_ratatui(ColorToken::Muted))),
                    ]))
                }
            })
            .collect();

        // Loading/error status indicator
        match &state.status {
            FetchStatus::Loading => {
                v.push(ListItem::new(Span::styled(
                    " ...",
                    Style::default().fg(to_ratatui(ColorToken::Cyan)),
                )));
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

/// Dispatches content rendering based on current tab.
fn render_content(state: &AppState, frame: &mut Frame, area: ratatui::layout::Rect) {
    use crate::tui::state::Tab;

    match state.tab {
        Tab::Dashboard => render_dashboard(state, frame, area),
        _ => render_placeholder(state, frame, area),
    }
}

/// Placeholder for tabs not yet implemented.
fn render_placeholder(_state: &AppState, frame: &mut Frame, area: ratatui::layout::Rect) {
    use ratatui::widgets::Paragraph;

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Thick)
        .border_style(Style::default().fg(to_ratatui(ColorToken::Comment)));

    let p = Paragraph::new(Span::styled(
        " Em breve",
        Style::default().fg(to_ratatui(ColorToken::Muted)),
    ))
    .block(block);

    frame.render_widget(p, area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::types::{ProviderQuota, QuotaWindow};
    use crate::tui::state::{FetchStatus, ProviderView};

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
