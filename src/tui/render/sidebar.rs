//! Sidebar: VISÃO / PROVIDERS / MAIS. Sem tabs — este é o hub de navegação.

use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::theme::{provider_hex, ColorToken};
use crate::tui::mouse::{HitMap, MouseTarget};
use crate::tui::state::{sidebar_items, AppState, SidebarItem};
use crate::tui::theme_bridge::to_ratatui;

/// Largura mínima pra caber o rótulo completo (mark + nome + %). Abaixo
/// disso (sidebar colapsada em Length(3)) só a marca ◆●● é exibida.
const NARROW_THRESHOLD: u16 = 6;

fn item_label(state: &AppState, item: SidebarItem, narrow: bool) -> Line<'static> {
    if narrow {
        return match item {
            SidebarItem::Overview => Line::from(" ▸".to_string()),
            SidebarItem::Provider(i) => {
                let pv = &state.providers[i];
                let mark = if pv.quota.provider == "claude" { "◆" } else { "●" };
                Line::from(format!(" {mark}"))
            }
            // ratatui pula `buf.set_style` inteiro quando a Line tem
            // largura 0 (ver `ratatui_core::text::Line::render_with_alignment`)
            // — uma Line vazia nunca pinta o bg de seleção. Por isso cada
            // item ganha ao menos 1 glifo em vez de string vazia: senão o
            // cursor fica invisível ao navegar até History/Login/Waybar
            // com a sidebar colapsada.
            SidebarItem::History => Line::from(" H".to_string()),
            SidebarItem::Login => Line::from(" L".to_string()),
            SidebarItem::Waybar => Line::from(" W".to_string()),
        };
    }
    match item {
        SidebarItem::Overview => Line::from(" ▸ Geral".to_string()),
        SidebarItem::Provider(i) => {
            let pv = &state.providers[i];
            let mark = if pv.quota.provider == "claude" { "◆" } else { "●" };
            let pct = pv
                .quota
                .primary
                .as_ref()
                .map(|w| format!("{:>3.0}%", w.remaining))
                .unwrap_or_else(|| "  – ".to_string());
            Line::from(format!(" {mark} {:<7}{pct}", pv.quota.display_name))
        }
        SidebarItem::History => Line::from("   Histórico".to_string()),
        SidebarItem::Login => Line::from("   Login".to_string()),
        SidebarItem::Waybar => Line::from("   Waybar".to_string()),
    }
}

pub fn render_sidebar(state: &AppState, frame: &mut Frame, area: Rect, hits: &mut HitMap) {
    let narrow = area.width < NARROW_THRESHOLD;
    let items = sidebar_items(state.providers.len());
    let mut lines: Vec<Line> = Vec::new();
    let mut row_of_item: Vec<u16> = Vec::new();

    for (i, item) in items.iter().enumerate() {
        // Cabeçalhos de seção antes do primeiro item de cada grupo. Em modo
        // estreito viram linha em branco (sem texto legível em 3 colunas) —
        // preserva a contagem de linhas (e portanto o offset de cada item)
        // idêntica ao modo largo.
        match item {
            SidebarItem::Overview => lines.push(section(" VISÃO", narrow)),
            SidebarItem::Provider(0) => {
                lines.push(Line::from(""));
                lines.push(section(" PROVIDERS", narrow));
            }
            SidebarItem::History => {
                lines.push(Line::from(""));
                lines.push(section(" MAIS", narrow));
            }
            _ => {}
        }
        let mut line = item_label(state, *item, narrow);
        let selected = state.sidebar_selected == i;
        let hovered = state.hover == Some(MouseTarget::Sidebar(i));
        let style = if selected {
            Style::default()
                .bg(to_ratatui(ColorToken::SelBg))
                .add_modifier(Modifier::BOLD)
        } else if hovered {
            Style::default().bg(to_ratatui(ColorToken::Surface))
        } else {
            Style::default()
        };
        line = line.style(style.fg(item_color(state, *item)));
        row_of_item.push(area.y + lines.len() as u16);
        lines.push(line);
    }

    for (i, row) in row_of_item.iter().enumerate() {
        hits.push(Rect::new(area.x, *row, area.width, 1), MouseTarget::Sidebar(i));
    }
    frame.render_widget(Paragraph::new(lines), area);
}

fn section(label: &str, narrow: bool) -> Line<'static> {
    if narrow {
        return Line::from("");
    }
    Line::from(Span::styled(
        label.to_string(),
        Style::default()
            .fg(to_ratatui(ColorToken::Comment))
            .add_modifier(Modifier::BOLD),
    ))
}

fn item_color(state: &AppState, item: SidebarItem) -> ratatui::style::Color {
    match item {
        SidebarItem::Provider(i) => {
            let pv = &state.providers[i];
            if pv.quota.error.is_some() {
                to_ratatui(ColorToken::Muted) // deslogado/erro: dim
            } else {
                crate::tui::theme_bridge::hex_to_color(provider_hex(&pv.quota.provider))
            }
        }
        _ => to_ratatui(ColorToken::Text),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::types::{ProviderQuota, QuotaWindow};
    use crate::tui::state::ProviderView;

    fn make_provider(id: &str, display: &str, remaining: f64) -> ProviderView {
        ProviderView::new(ProviderQuota {
            provider: id.to_string(),
            display_name: display.to_string(),
            available: true,
            account: None,
            plan: None,
            plan_type: None,
            primary: Some(QuotaWindow {
                remaining,
                resets_at: None,
                window_minutes: Some(300),
                used: Some(100.0 - remaining),
                severity: None,
            }),
            secondary: None,
            models: None,
            extra: None,
            error: None,
        })
    }

    #[test]
    fn render_sidebar_registers_all_items_1to1_with_cursor() {
        let backend = ratatui::backend::TestBackend::new(17, 20);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        let mut state = AppState::new();
        state.providers = vec![
            make_provider("claude", "Claude", 26.0),
            make_provider("codex", "Codex", 1.0),
        ];
        let mut hits = HitMap::default();
        let items_len = sidebar_items(state.providers.len()).len();
        terminal
            .draw(|f| {
                let area = f.area();
                render_sidebar(&state, f, area, &mut hits);
            })
            .unwrap();

        // Cada indice logico 0..items_len tem uma zona registrada (1:1).
        for i in 0..items_len {
            let found = (0..20u16).any(|y| hits.at(0, y) == Some(MouseTarget::Sidebar(i)));
            assert!(found, "faltou hit-zone para indice {i}");
        }
    }

    #[test]
    fn narrow_sidebar_does_not_panic_and_keeps_same_row_count() {
        let backend = ratatui::backend::TestBackend::new(3, 20);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        let mut state = AppState::new();
        state.providers = vec![make_provider("claude", "Claude", 26.0)];
        let mut hits = HitMap::default();
        let items_len = sidebar_items(state.providers.len()).len();
        terminal
            .draw(|f| {
                let area = f.area();
                render_sidebar(&state, f, area, &mut hits);
            })
            .unwrap();
        for i in 0..items_len {
            let found = (0..20u16).any(|y| hits.at(0, y) == Some(MouseTarget::Sidebar(i)));
            assert!(found, "faltou hit-zone (narrow) para indice {i}");
        }
    }
}
