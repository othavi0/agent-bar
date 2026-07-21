//! Vocabulário de ícones: Nerd Font (faixa Font Awesome, estável no NF v3)
//! com fallback Unicode universal (GlyphMode::Box).

use crate::settings::GlyphMode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    Ok,
    LoggedOut,
    Warn,
    NoToken,
}

pub fn glyph(icon: Icon, mode: GlyphMode) -> &'static str {
    match (icon, mode) {
        (Icon::Ok, GlyphMode::Nerd) => "\u{f00c}",
        (Icon::Ok, GlyphMode::Box) => "✓",
        (Icon::LoggedOut, GlyphMode::Nerd) => "\u{f00d}",
        (Icon::LoggedOut, GlyphMode::Box) => "✗",
        (Icon::Warn, GlyphMode::Nerd) => "\u{f071}",
        (Icon::Warn, GlyphMode::Box) => "!",
        (Icon::NoToken, GlyphMode::Nerd) => "\u{f023}",
        (Icon::NoToken, GlyphMode::Box) => "×",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nerd_and_box_glyphs_differ_and_are_nonempty() {
        for icon in [
            Icon::Ok,
            Icon::LoggedOut,
            Icon::Warn,
            Icon::NoToken,
        ] {
            assert!(!glyph(icon, GlyphMode::Nerd).is_empty());
            assert!(!glyph(icon, GlyphMode::Box).is_empty());
        }
        // Nerd usa PUA (>= U+E000); Box nunca:
        assert!(glyph(Icon::Ok, GlyphMode::Nerd)
            .chars()
            .all(|c| c as u32 >= 0xE000));
        assert!(glyph(Icon::Ok, GlyphMode::Box)
            .chars()
            .all(|c| (c as u32) < 0xE000));
    }
}
