use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::theme::ColorToken;
use crate::tui::state::{AppState, Mode};
use crate::tui::theme_bridge::to_ratatui;

/// Renders the bottom status bar with contextual key hints.
pub fn render_status_bar(state: &AppState, frame: &mut Frame, area: Rect) {
    let hint_style = Style::default().fg(to_ratatui(ColorToken::Muted));
    let key_style = Style::default()
        .fg(to_ratatui(ColorToken::TextBright))
        .add_modifier(Modifier::BOLD);

    let spans: Vec<Span<'_>> = match state.mode {
        Mode::List => vec![
            Span::styled(" ", hint_style),
            Span::styled("\u{2191}\u{2193}", key_style),
            Span::styled(" provider", hint_style),
            Span::styled(" \u{b7} ", hint_style),
            Span::styled("\u{2190}\u{2192}", key_style),
            Span::styled(" aba", hint_style),
            Span::styled(" \u{b7} ", hint_style),
            Span::styled("Enter", key_style),
            Span::styled(" detalhe", hint_style),
            Span::styled(" \u{b7} ", hint_style),
            Span::styled("[q]", key_style),
            Span::styled("uit", hint_style),
        ],
        Mode::Detail => vec![
            Span::styled(" ", hint_style),
            Span::styled("\u{2191}\u{2193}", key_style),
            Span::styled(" provider", hint_style),
            Span::styled(" \u{b7} ", hint_style),
            Span::styled("Esc", key_style),
            Span::styled(" volta", hint_style),
            Span::styled(" \u{b7} ", hint_style),
            Span::styled("\u{2190}\u{2192}", key_style),
            Span::styled(" aba", hint_style),
            Span::styled(" \u{b7} ", hint_style),
            Span::styled("[r]", key_style),
            Span::styled("efresh", hint_style),
        ],
    };

    let bar = Paragraph::new(Line::from(spans));
    frame.render_widget(bar, area);
}
