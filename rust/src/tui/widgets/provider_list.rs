//! ProviderList: sidebar list items for the provider panel.
//!
//! Encapsulates the selected/unselected rendering logic (diamond ◆ / circle ●),
//! provider color, and percentage label.

use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::ListItem;

use crate::theme::ColorToken;
use crate::tui::state::ProviderView;
use crate::tui::theme_bridge::{provider_color, to_ratatui};

/// Builds a `ListItem` for one provider entry in the sidebar.
///
/// - Selected → ◆ (U+25C6) prefix, bold provider color, bright pct
/// - Unselected → ● (U+25CF) prefix, muted provider color, muted pct
pub fn provider_list_item(pv: &ProviderView, selected: bool) -> ListItem<'static> {
    let q = &pv.quota;
    let remaining = q.primary.as_ref().map(|w| w.remaining).unwrap_or(0.0);
    let pct = format!("{:3.0}%", remaining);
    let p_color = provider_color(&q.provider);

    // Inner width = 13 (sidebar=15 - 2 borders).
    // Layout: glyph(1) + space(1) + name(7) + pct(4) = 13
    let name_trunc: String = if q.display_name.len() > 7 {
        q.display_name[..7].to_string()
    } else {
        q.display_name.clone()
    };

    if selected {
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
}
