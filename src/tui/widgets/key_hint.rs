//! KeyHint: status bar key hint line builder.
//!
//! Encapsulates styled hint/key spans for the bottom status bar.

use ratatui::style::{Modifier, Style};
use ratatui::text::Span;

use crate::theme::ColorToken;
use crate::tui::theme_bridge::to_ratatui;

/// Returns a styled `Span` for a key label (bright + bold).
pub fn key_span(text: &'static str) -> Span<'static> {
    Span::styled(
        text,
        Style::default()
            .fg(to_ratatui(ColorToken::TextBright))
            .add_modifier(Modifier::BOLD),
    )
}

/// Returns a styled `Span` for a hint description (muted).
pub fn hint_span(text: &'static str) -> Span<'static> {
    Span::styled(text, Style::default().fg(to_ratatui(ColorToken::Muted)))
}

/// Returns a muted separator span (` · `).
pub fn sep_span() -> Span<'static> {
    hint_span(" \u{b7} ")
}
