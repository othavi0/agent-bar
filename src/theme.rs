//! Paleta One Dark e tokens de cor. O gate NO_COLOR NÃO vive aqui (é injetado no
//! render_ansi) — `ansi()` sempre devolve o código; o renderer decide emiti-lo ou não.

pub const ANSI_RESET: &str = "\x1b[0m";
pub const ANSI_BOLD: &str = "\x1b[1m";

/// Token de cor agnóstico de tema. Os renderers mapeiam para hex (Pango) ou ANSI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorToken {
    Green,
    Yellow,
    Orange,
    Red,
    Comment,
    Text,
    TextBright,
    Muted,
    Magenta,
    Cyan,
    Blue,
    BrightBlue,
}

impl ColorToken {
    pub fn hex(self) -> &'static str {
        match self {
            ColorToken::Green => "#98c379",
            ColorToken::Yellow => "#e5c07b",
            ColorToken::Orange => "#d19a66",
            ColorToken::Red => "#e06c75",
            ColorToken::Comment => "#6a7485",
            ColorToken::Text => "#c0c9d4",
            ColorToken::TextBright => "#e2e8f0",
            ColorToken::Muted => "#97a1ae",
            ColorToken::Magenta => "#c678dd",
            ColorToken::Cyan => "#56b6c2",
            ColorToken::Blue => "#61afef",
            ColorToken::BrightBlue => "#528bff",
        }
    }

    pub fn ansi(self) -> String {
        ansi_truecolor(self.hex())
    }
}

/// `#rrggbb` → escape ANSI truecolor `\x1b[38;2;r;g;bm`. Hex inválido → string vazia.
pub fn ansi_truecolor(hex: &str) -> String {
    let clean = hex.trim_start_matches('#');
    if clean.len() != 6 {
        return String::new();
    }
    let comp = |s: &str| u8::from_str_radix(s, 16).ok();
    let (Some(r), Some(g), Some(b)) = (comp(&clean[0..2]), comp(&clean[2..4]), comp(&clean[4..6]))
    else {
        return String::new();
    };
    format!("\x1b[38;2;{r};{g};{b}m")
}

/// Cor de marca do provider (hex). Desconhecido → text.
pub fn provider_hex(id: &str) -> &'static str {
    match id {
        "claude" => ColorToken::Orange.hex(),
        "codex" => ColorToken::Green.hex(),
        "amp" => ColorToken::Magenta.hex(),
        _ => ColorToken::Text.hex(),
    }
}

/// Box-drawing (variante pesada) — fonte única da verdade.
pub mod box_chars {
    pub const TL: &str = "┏";
    pub const BL: &str = "┗";
    pub const LT: &str = "┣";
    pub const H: &str = "━";
    pub const V: &str = "┃";
    pub const DOT: &str = "●";
    pub const DOT_O: &str = "○";
    pub const DIAMOND: &str = "◆";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_values_match_one_dark() {
        assert_eq!(ColorToken::Green.hex(), "#98c379");
        assert_eq!(ColorToken::Red.hex(), "#e06c75");
        assert_eq!(ColorToken::Comment.hex(), "#6a7485");
        assert_eq!(ColorToken::BrightBlue.hex(), "#528bff");
        assert_eq!(ColorToken::TextBright.hex(), "#e2e8f0");
    }

    #[test]
    fn ansi_truecolor_format() {
        // #98c379 → 152;195;121
        assert_eq!(ColorToken::Green.ansi(), "\x1b[38;2;152;195;121m");
        assert_eq!(ansi_truecolor("#e06c75"), "\x1b[38;2;224;108;117m");
    }

    #[test]
    fn ansi_truecolor_rejects_bad_hex() {
        assert_eq!(ansi_truecolor("nope"), "");
        assert_eq!(ansi_truecolor("#12"), "");
        assert_eq!(ansi_truecolor("#zzzzzz"), ""); // 6 chars mas dígitos inválidos
    }

    #[test]
    fn ansi_constants() {
        assert_eq!(ANSI_RESET, "\x1b[0m");
        assert_eq!(ANSI_BOLD, "\x1b[1m");
    }

    #[test]
    fn provider_hex_mapping() {
        assert_eq!(provider_hex("claude"), ColorToken::Orange.hex());
        assert_eq!(provider_hex("codex"), ColorToken::Green.hex());
        assert_eq!(provider_hex("amp"), ColorToken::Magenta.hex());
        assert_eq!(provider_hex("other"), ColorToken::Text.hex());
    }

    #[test]
    fn box_chars_are_heavy_variant() {
        assert_eq!(box_chars::TL, "┏");
        assert_eq!(box_chars::V, "┃");
        assert_eq!(box_chars::DIAMOND, "◆");
    }
}
