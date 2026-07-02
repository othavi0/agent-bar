use crate::theme::ColorToken;
use ratatui::style::Color;

pub fn to_ratatui(token: ColorToken) -> Color {
    let h = token.hex().trim_start_matches('#');
    let p = |s: &str| u8::from_str_radix(s, 16).unwrap_or(0);
    Color::Rgb(p(&h[0..2]), p(&h[2..4]), p(&h[4..6]))
}

/// `#rrggbb` → `Color::Rgb`. Mesma lógica de `theme::ansi_truecolor`, mas
/// devolvendo um `Color` do ratatui em vez de um escape ANSI. Hex inválido
/// (tamanho errado ou dígito não-hex) → preto, sem panic.
pub fn hex_to_color(hex: &str) -> Color {
    let h = hex.trim_start_matches('#');
    if h.len() != 6 {
        return Color::Rgb(0, 0, 0);
    }
    let seg = |i: usize| {
        h.get(i..i + 2)
            .and_then(|s| u8::from_str_radix(s, 16).ok())
            .unwrap_or(0)
    };
    Color::Rgb(seg(0), seg(2), seg(4))
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
    fn hex_to_color_parses_hex() {
        assert_eq!(hex_to_color("#98c379"), Color::Rgb(0x98, 0xc3, 0x79));
        assert_eq!(hex_to_color("98c379"), Color::Rgb(0x98, 0xc3, 0x79));
    }

    #[test]
    fn hex_to_color_rejects_bad_hex() {
        assert_eq!(hex_to_color("nope"), Color::Rgb(0, 0, 0));
        assert_eq!(hex_to_color("#zzzzzz"), Color::Rgb(0, 0, 0));
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
