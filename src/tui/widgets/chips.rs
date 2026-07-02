//! Chips de ação (rodapé): sempre centralizados — contrato de alinhamento.

use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

use crate::theme::ColorToken;
use crate::tui::mouse::{ChipKind, HitMap, MouseTarget};
use crate::tui::theme_bridge::to_ratatui;

const GAP: &str = "   ";

fn chip_width(key: &str, label: &str) -> u16 {
    // ' key ' + 'label ' → key+2 + label+1
    (key.chars().count() + 2 + label.chars().count() + 1) as u16
}

fn total_width(chips: &[(ChipKind, &str, &str)]) -> u16 {
    let sum: u16 = chips.iter().map(|(_, k, l)| chip_width(k, l)).sum();
    sum + (GAP.chars().count() as u16) * (chips.len().saturating_sub(1) as u16)
}

pub fn chips_line(chips: &[(ChipKind, &str, &str)], width: u16) -> Line<'static> {
    let pad = width.saturating_sub(total_width(chips)) / 2;
    let mut spans = vec![Span::raw(" ".repeat(pad as usize))];
    for (i, (_, key, label)) in chips.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw(GAP));
        }
        spans.push(Span::styled(
            format!(" {key} "),
            Style::default()
                .fg(to_ratatui(ColorToken::Cyan))
                .bg(to_ratatui(ColorToken::ChipBg))
                .add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::styled(
            format!("{label} "),
            Style::default()
                .fg(to_ratatui(ColorToken::Muted))
                .bg(to_ratatui(ColorToken::ChipBg)),
        ));
    }
    Line::from(spans)
}

pub fn register_chip_hits(chips: &[(ChipKind, &str, &str)], area: Rect, hits: &mut HitMap) {
    let mut x = area.x + area.width.saturating_sub(total_width(chips)) / 2;
    for (i, (kind, key, label)) in chips.iter().enumerate() {
        if i > 0 {
            x += GAP.chars().count() as u16;
        }
        let w = chip_width(key, label);
        hits.push(Rect::new(x, area.y, w, 1), MouseTarget::Chip(*kind));
        x += w;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::mouse::ChipKind;

    #[test]
    fn chips_line_is_centered() {
        let chips = [(ChipKind::Quit, "q", "sair")];
        let line = chips_line(&chips, 40);
        // Span[0] é o pad puro (sem estilo) — checagem inequívoca de que o
        // pad esquerdo é exatamente 16. Contar espaços via flatten de todos
        // os spans seria ambíguo aqui: o badge da tecla (" q ") também
        // começa com um espaço (estilizado ChipBg), indistinguível de um
        // espaço de pad depois que os spans são concatenados em string.
        assert_eq!(line.spans[0].content.as_ref(), " ".repeat(16));
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        // ' q sair ' = 8 células visíveis → 16 de padding em cada lado
        // (o pad direito não é emitido, por isso 40 - 16 e não 40 - 32).
        assert_eq!(text.chars().count(), 40 - 16);
    }
}
