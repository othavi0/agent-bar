//! Estados especiais (deslogado / erro).

use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};
use ratatui::Frame;

use crate::providers::types::ProviderQuota;
use crate::settings::GlyphMode;
use crate::theme::ColorToken;
use crate::tui::theme_bridge::to_ratatui;
use crate::tui::widgets::icons::{glyph, Icon};

/// CTA em tela cheia (provider sem sessão) — igual em espírito ao card do
/// Overview, mas com instrução maior (a tela toda é deste provider, não
/// precisa caber em 1 linha).
pub(super) fn render_logged_out(q: &ProviderQuota, frame: &mut Frame, area: Rect) {
    let lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            format!(" {} \u{2014} sem sess\u{e3}o", q.display_name),
            Style::default()
                .fg(to_ratatui(ColorToken::TextBright))
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            " Nenhuma credencial v\u{e1}lida encontrada para este provider.",
            Style::default().fg(to_ratatui(ColorToken::Muted)),
        )),
        Line::from(Span::styled(
            " Pressione [g] ou clique no chip \"login\" abaixo para autenticar.",
            Style::default().fg(to_ratatui(ColorToken::Muted)),
        )),
    ];
    let p = Paragraph::new(lines).wrap(Wrap { trim: false });
    frame.render_widget(p, area);
}

/// Mensagem de erro tipado (falha não-auth: parse/rede/API) com ícone —
/// NUNCA tela branca. `q.error` é a string verbatim do provider (contrato,
/// ver `providers::error`).
pub(super) fn render_error(q: &ProviderQuota, mode: GlyphMode, frame: &mut Frame, area: Rect) {
    let msg = q.error.as_deref().unwrap_or("Erro desconhecido");
    let lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            format!(" {} Erro ao carregar dados", glyph(Icon::Warn, mode)),
            Style::default()
                .fg(to_ratatui(ColorToken::Red))
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            format!(" {msg}"),
            Style::default().fg(to_ratatui(ColorToken::Muted)),
        )),
    ];
    let p = Paragraph::new(lines).wrap(Wrap { trim: false });
    frame.render_widget(p, area);
}
