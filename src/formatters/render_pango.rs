//! ÚNICO ponto de XML-escape do Pango. Toda string de provider que entra em markup
//! Pango passa por `span` (que escapa). Segments `raw` saem verbatim — nunca passe
//! dados não-confiáveis por `raw`.

use super::segments::{Line, Segment};

/// Escapa os 5 entities XML. ORDEM importa: `&` primeiro (senão double-escape).
pub fn escape_xml(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('\'', "&#39;")
        .replace('"', "&quot;")
}

/// `<span foreground='{hex}'[ weight='bold']>{escape(text)}</span>` — aspas simples.
pub fn span(hex: &str, text: &str, bold: bool) -> String {
    let weight = if bold { " weight='bold'" } else { "" };
    format!(
        "<span foreground='{hex}'{weight}>{}</span>",
        escape_xml(text)
    )
}

fn render_segment(seg: &Segment) -> String {
    if seg.raw {
        return seg.text.to_string();
    }
    span(seg.color.hex(), &seg.text, seg.bold)
}

fn render_line(line: &Line) -> String {
    line.iter().map(render_segment).collect()
}

/// Renderiza linhas em markup Pango multi-linha.
pub fn render_pango(lines: &[Line]) -> String {
    lines.iter().map(render_line).collect::<Vec<_>>().join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formatters::segments::Segment;
    use crate::theme::ColorToken;

    #[test]
    fn escape_all_five_entities_in_order() {
        assert_eq!(escape_xml("a&b<c>d'e\"f"), "a&amp;b&lt;c&gt;d&#39;e&quot;f");
    }

    #[test]
    fn escape_ampersand_first_no_double_escape() {
        // se '&' não fosse primeiro, "<" viraria "&lt;" e o "&" seria re-escapado
        assert_eq!(escape_xml("<"), "&lt;");
        assert_eq!(escape_xml("&lt;"), "&amp;lt;");
    }

    #[test]
    fn span_format_single_quotes() {
        assert_eq!(
            span("#98c379", "hi", false),
            "<span foreground='#98c379'>hi</span>"
        );
    }

    #[test]
    fn span_bold_adds_weight() {
        assert_eq!(
            span("#e06c75", "x", true),
            "<span foreground='#e06c75' weight='bold'>x</span>"
        );
    }

    #[test]
    fn span_escapes_text() {
        assert_eq!(
            span("#c0c9d4", "a<b", false),
            "<span foreground='#c0c9d4'>a&lt;b</span>"
        );
    }

    #[test]
    fn render_pango_raw_bypasses_span_and_escape() {
        let line = vec![
            Segment::new("ok", ColorToken::Green),
            Segment::raw_text(" <sep> "),
        ];
        let out = render_pango(&[line]);
        assert_eq!(out, "<span foreground='#98c379'>ok</span> <sep> ");
    }

    #[test]
    fn render_pango_joins_lines_with_newline() {
        let l1 = vec![Segment::new("a", ColorToken::Text)];
        let l2 = vec![Segment::new("b", ColorToken::Text)];
        let out = render_pango(&[l1, l2]);
        assert!(out.contains('\n'));
        assert_eq!(out.lines().count(), 2);
    }
}
