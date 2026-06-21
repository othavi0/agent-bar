//! Render ANSI para o terminal. NO_COLOR é injetado. Reset anexado só quando a
//! linha tem ≥1 segment não-raw e cores estão ativas.

use super::segments::{Line, Segment};
use crate::theme::{ANSI_BOLD, ANSI_RESET};

fn render_segment(seg: &Segment, no_color: bool) -> String {
    if seg.raw || no_color {
        return seg.text.to_string();
    }
    let bold = if seg.bold { ANSI_BOLD } else { "" };
    format!("{}{}{}", seg.color.ansi(), bold, seg.text)
}

fn render_line(line: &Line, no_color: bool) -> String {
    if line.is_empty() {
        return String::new();
    }
    let body: String = line.iter().map(|s| render_segment(s, no_color)).collect();
    let has_colored = line.iter().any(|s| !s.raw);
    if has_colored && !no_color {
        format!("{body}{ANSI_RESET}")
    } else {
        body
    }
}

/// Renderiza linhas em ANSI multi-linha.
pub fn render_ansi(lines: &[Line], no_color: bool) -> String {
    lines
        .iter()
        .map(|l| render_line(l, no_color))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formatters::segments::Segment;
    use crate::theme::ColorToken;

    #[test]
    fn colored_segment_has_truecolor_and_reset() {
        let out = render_ansi(&[vec![Segment::new("hi", ColorToken::Green)]], false);
        assert_eq!(out, "\x1b[38;2;152;195;121mhi\x1b[0m");
    }

    #[test]
    fn bold_segment_includes_bold_code() {
        let out = render_ansi(&[vec![Segment::new("x", ColorToken::Red).bold()]], false);
        assert_eq!(out, "\x1b[38;2;224;108;117m\x1b[1mx\x1b[0m");
    }

    #[test]
    fn no_color_emits_plain_text() {
        let out = render_ansi(&[vec![Segment::new("hi", ColorToken::Green)]], true);
        assert_eq!(out, "hi");
    }

    #[test]
    fn raw_only_line_has_no_reset() {
        let out = render_ansi(&[vec![Segment::raw_text("│")]], false);
        assert_eq!(out, "│"); // sem reset porque nenhum segment não-raw
    }

    #[test]
    fn mixed_line_resets_once_at_end() {
        let line = vec![
            Segment::new("a", ColorToken::Text),
            Segment::raw_text(" | "),
        ];
        let out = render_ansi(&[line], false);
        assert!(out.ends_with("\x1b[0m"));
        assert_eq!(out.matches("\x1b[0m").count(), 1);
    }

    #[test]
    fn lines_joined_by_newline() {
        let out = render_ansi(
            &[
                vec![Segment::new("a", ColorToken::Text)],
                vec![Segment::new("b", ColorToken::Text)],
            ],
            true,
        );
        assert_eq!(out, "a\nb");
    }
}
