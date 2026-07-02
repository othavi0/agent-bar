use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::tui::state::{AppState, Screen};
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

    let spans: Vec<Span<'_>> = match state.screen {
        // Tela Waybar (config): hints de edicao.
        Screen::Waybar => {
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
                    key_span("Esc"),
                    hint_span(" volta"),
                    sep_span(),
                    key_span("[q]"),
                    hint_span("uit"),
                ];
                s.extend(help_hint);
                s
            }
        }
        // Tela Login.
        Screen::Login => {
            let mut s = vec![
                hint_span(" "),
                key_span("\u{2191}\u{2193}"),
                hint_span(" provider"),
                sep_span(),
                key_span("Enter"),
                hint_span(" login"),
                sep_span(),
                key_span("Esc"),
                hint_span(" volta"),
                sep_span(),
                key_span("[q]"),
                hint_span("uit"),
            ];
            s.extend(help_hint);
            s
        }
        // Tela History.
        Screen::History => {
            let mut s = vec![
                hint_span(" "),
                key_span("Esc"),
                hint_span(" volta"),
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
        // Overview (lista de providers).
        Screen::Overview => {
            let mut s = vec![
                hint_span(" "),
                key_span("\u{2191}\u{2193}"),
                hint_span(" navegar"),
                sep_span(),
                key_span("Enter"),
                hint_span(" abrir"),
                sep_span(),
                key_span("[q]"),
                hint_span("uit"),
            ];
            s.extend(help_hint);
            s
        }
        // Detalhe do provider.
        Screen::Detail => {
            let mut s = vec![
                hint_span(" "),
                key_span("\u{2191}\u{2193}"),
                hint_span(" navegar"),
                sep_span(),
                key_span("Esc"),
                hint_span(" volta"),
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
