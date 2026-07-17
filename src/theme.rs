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
    /// Acento de UI (#61afef) — não confundir com as séries de gráfico
    /// (Series1..6), que têm paleta própria.
    Blue,
    BrightBlue,
    Surface,
    SelBg,
    ChipBg,
    EmptyTrack,
    GreenHi,
    // séries de gráfico por modelo (One Dark Turbo, validadas p/ CVD/contraste)
    Series1, // fable/mythos
    Series2, // opus
    Series3, // sonnet
    Series4, // haiku
    Series5, // codex/gpt
    Series6, // outros
    /// Fundo geral da TUI (#282c34 — antes literal em render/mod.rs).
    Bg,
}

impl ColorToken {
    pub fn hex(self) -> &'static str {
        match self {
            ColorToken::Green => "#98c379",
            ColorToken::Yellow => "#e5c07b",
            ColorToken::Orange => "#d19a66",
            // Contraste ≥4.5:1 sobre Bg #282c34 (trilha B / WCAG AA body).
            ColorToken::Red => "#e88b93",
            ColorToken::Comment => "#8b95a5",
            ColorToken::Text => "#c0c9d4",
            ColorToken::TextBright => "#e2e8f0",
            ColorToken::Muted => "#97a1ae",
            ColorToken::Magenta => "#c678dd",
            ColorToken::Cyan => "#56b6c2",
            ColorToken::Blue => "#61afef",
            ColorToken::BrightBlue => "#528bff",
            ColorToken::Surface => "#1b202a",
            ColorToken::SelBg => "#2c333f",
            ColorToken::ChipBg => "#262d3a",
            ColorToken::EmptyTrack => "#343b49",
            ColorToken::GreenHi => "#b5e890",
            ColorToken::Series1 => "#4a9ae0",
            ColorToken::Series2 => "#d4924a",
            ColorToken::Series3 => "#c47ae0",
            ColorToken::Series4 => "#63b358",
            ColorToken::Series5 => "#3ab3c4",
            ColorToken::Series6 => "#c2a23a",
            ColorToken::Bg => "#282c34",
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
        "grok" => ColorToken::Cyan.hex(),
        _ => ColorToken::Text.hex(),
    }
}

/// Token de série de gráfico pelo slot de família (usage::model_names).
pub fn series_token(slot: u8) -> ColorToken {
    match slot {
        0 => ColorToken::Series1,
        1 => ColorToken::Series2,
        2 => ColorToken::Series3,
        3 => ColorToken::Series4,
        4 => ColorToken::Series5,
        _ => ColorToken::Series6,
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
        assert_eq!(ColorToken::Red.hex(), "#e88b93");
        assert_eq!(ColorToken::Comment.hex(), "#8b95a5");
        assert_eq!(ColorToken::BrightBlue.hex(), "#528bff");
        assert_eq!(ColorToken::TextBright.hex(), "#e2e8f0");
    }

    #[test]
    fn ansi_truecolor_format() {
        // #98c379 → 152;195;121
        assert_eq!(ColorToken::Green.ansi(), "\x1b[38;2;152;195;121m");
        assert_eq!(ansi_truecolor("#e88b93"), "\x1b[38;2;232;139;147m");
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
        assert_eq!(provider_hex("grok"), ColorToken::Cyan.hex());
        assert_eq!(provider_hex("other"), ColorToken::Text.hex());
    }

    #[test]
    fn box_chars_are_heavy_variant() {
        assert_eq!(box_chars::TL, "┏");
        assert_eq!(box_chars::V, "┃");
        assert_eq!(box_chars::DIAMOND, "◆");
    }

    #[test]
    fn new_surface_tokens_hex() {
        assert_eq!(ColorToken::Surface.hex(), "#1b202a");
        assert_eq!(ColorToken::SelBg.hex(), "#2c333f");
        assert_eq!(ColorToken::ChipBg.hex(), "#262d3a");
        assert_eq!(ColorToken::EmptyTrack.hex(), "#343b49");
        assert_eq!(ColorToken::GreenHi.hex(), "#b5e890");
    }

    #[test]
    fn series_tokens_hex() {
        assert_eq!(ColorToken::Series1.hex(), "#4a9ae0");
        assert_eq!(ColorToken::Series2.hex(), "#d4924a");
        assert_eq!(ColorToken::Series3.hex(), "#c47ae0");
        assert_eq!(ColorToken::Series4.hex(), "#63b358");
        assert_eq!(ColorToken::Series5.hex(), "#3ab3c4");
        assert_eq!(ColorToken::Series6.hex(), "#c2a23a");
        assert_eq!(ColorToken::Bg.hex(), "#282c34");
    }

    #[test]
    fn series_token_maps_slots() {
        assert_eq!(series_token(0), ColorToken::Series1);
        assert_eq!(series_token(5), ColorToken::Series6);
        assert_eq!(series_token(99), ColorToken::Series6); // clamp
    }
}
