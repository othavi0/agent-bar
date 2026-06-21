use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::tui::state::{AppState, Mode, Tab};
use crate::tui::widgets::key_hint::{hint_span, key_span, sep_span};

/// Renders the bottom status bar with contextual key hints.
pub fn render_status_bar(state: &AppState, frame: &mut Frame, area: Rect) {
    // Se o overlay de ajuda esta aberto, mostra dica de fechar.
    if state.show_help {
        let spans = vec![
            hint_span(" "),
            key_span("Esc"),
            hint_span(" / "),
            key_span("[?]"),
            hint_span(" fecha ajuda"),
        ];
        let bar = Paragraph::new(Line::from(spans));
        frame.render_widget(bar, area);
        return;
    }

    let help_hint: Vec<Span<'_>> = vec![sep_span(), key_span("[?]"), hint_span(" ajuda")];

    let spans: Vec<Span<'_>> = match (&state.tab, &state.mode) {
        // Aba Waybar (config): hints de edicao.
        (Tab::Waybar, _) => {
            let editing = state
                .config_state
                .as_ref()
                .map(|cs| cs.editing)
                .unwrap_or(false);
            if editing {
                vec![
                    hint_span(" "),
                    key_span("Enter"),
                    hint_span(" confirma"),
                    sep_span(),
                    key_span("Esc"),
                    hint_span(" cancela"),
                ]
            } else {
                let mut s = vec![
                    hint_span(" "),
                    key_span("\u{2191}\u{2193}"),
                    hint_span(" campo"),
                    sep_span(),
                    key_span("Enter"),
                    hint_span(" editar"),
                    sep_span(),
                    key_span("[s]"),
                    hint_span("alvar"),
                    sep_span(),
                    key_span("\u{2190}\u{2192}"),
                    hint_span(" aba"),
                    sep_span(),
                    key_span("[q]"),
                    hint_span("uit"),
                ];
                s.extend(help_hint);
                s
            }
        }
        // Aba Login.
        (Tab::Login, _) => {
            let mut s = vec![
                hint_span(" "),
                key_span("\u{2191}\u{2193}"),
                hint_span(" provider"),
                sep_span(),
                key_span("Enter"),
                hint_span(" login"),
                sep_span(),
                key_span("\u{2190}\u{2192}"),
                hint_span(" aba"),
                sep_span(),
                key_span("[q]"),
                hint_span("uit"),
            ];
            s.extend(help_hint);
            s
        }
        // Aba History.
        (Tab::History, _) => {
            let mut s = vec![
                hint_span(" "),
                key_span("\u{2190}\u{2192}"),
                hint_span(" aba"),
                sep_span(),
                key_span("[r]"),
                hint_span("efresh"),
                sep_span(),
                key_span("[q]"),
                hint_span("uit"),
            ];
            s.extend(help_hint);
            s
        }
        // Dashboard lista.
        (_, Mode::List) => {
            let mut s = vec![
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
            ];
            s.extend(help_hint);
            s
        }
        // Dashboard detalhe.
        (_, Mode::Detail) => {
            let mut s = vec![
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
            ];
            s.extend(help_hint);
            s
        }
    };

    let bar = Paragraph::new(Line::from(spans));
    frame.render_widget(bar, area);
}
