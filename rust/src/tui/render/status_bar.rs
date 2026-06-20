use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::tui::state::{AppState, Mode};
use crate::tui::widgets::key_hint::{hint_span, key_span, sep_span};

/// Renders the bottom status bar with contextual key hints.
pub fn render_status_bar(state: &AppState, frame: &mut Frame, area: Rect) {
    let spans: Vec<Span<'_>> = match state.mode {
        Mode::List => vec![
            hint_span(" "),
            key_span("\u{2191}\u{2193}"),
            hint_span(" provider"),
            sep_span(),
            key_span("\u{2190}\u{2192}"),
            hint_span(" aba"),
            sep_span(),
            key_span("Enter"),
            hint_span(" detalhe"),
            sep_span(),
            key_span("[q]"),
            hint_span("uit"),
        ],
        Mode::Detail => vec![
            hint_span(" "),
            key_span("\u{2191}\u{2193}"),
            hint_span(" provider"),
            sep_span(),
            key_span("Esc"),
            hint_span(" volta"),
            sep_span(),
            key_span("\u{2190}\u{2192}"),
            hint_span(" aba"),
            sep_span(),
            key_span("[r]"),
            hint_span("efresh"),
        ],
    };

    let bar = Paragraph::new(Line::from(spans));
    frame.render_widget(bar, area);
}
