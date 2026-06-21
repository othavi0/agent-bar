use crate::theme::ColorToken;
use ratatui::style::Color;

pub fn to_ratatui(token: ColorToken) -> Color {
    let h = token.hex().trim_start_matches('#');
    let p = |s: &str| u8::from_str_radix(s, 16).unwrap_or(0);
    Color::Rgb(p(&h[0..2]), p(&h[2..4]), p(&h[4..6]))
}

pub fn provider_color(id: &str) -> Color {
    match id {
        "claude" => to_ratatui(ColorToken::Orange),
        "codex" => to_ratatui(ColorToken::Green),
        "amp" => to_ratatui(ColorToken::Magenta),
        _ => to_ratatui(ColorToken::Text),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::{provider_hex, ColorToken};
    use ratatui::style::Color;

    #[test]
    fn to_ratatui_parses_hex() {
        assert_eq!(to_ratatui(ColorToken::Green), Color::Rgb(0x98, 0xc3, 0x79));
    }

    #[test]
    fn provider_color_matches_theme_provider_hex() {
        for id in ["claude", "codex", "amp", "other"] {
            let h = provider_hex(id).trim_start_matches('#');
            let want = Color::Rgb(
                u8::from_str_radix(&h[0..2], 16).unwrap(),
                u8::from_str_radix(&h[2..4], 16).unwrap(),
                u8::from_str_radix(&h[4..6], 16).unwrap(),
            );
            assert_eq!(provider_color(id), want, "divergiu p/ {id}");
        }
    }
}
