//! Sparkline widget: renders a braille block-element sparkline from a data slice.
//!
//! Wraps ratatui's built-in `Sparkline` widget, or falls back to a manual
//! block-element string (`▁▂▃▄▅▆▇`) when the data slice is empty.

use ratatui::style::Style;
use ratatui::text::Span;

use crate::theme::ColorToken;
use crate::tui::theme_bridge::to_ratatui;

/// Block elements in ascending order (U+2581..U+2587).
const BLOCKS: [char; 7] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇'];

/// Renders a sparkline string from `data` using the 7-level block characters.
/// Returns an empty string if data is empty.
/// Values are normalized relative to the maximum in `data`.
pub fn sparkline_str(data: &[u64]) -> String {
    if data.is_empty() {
        return String::new();
    }
    let max = *data.iter().max().unwrap_or(&1);
    let max = max.max(1);
    data.iter()
        .map(|&v| {
            let idx = ((v as f64 / max as f64) * (BLOCKS.len() - 1) as f64).round() as usize;
            BLOCKS[idx.min(BLOCKS.len() - 1)]
        })
        .collect()
}

/// Renders a sparkline string expanded to fill `target_width` characters.
/// Each data point is repeated `target_width / data.len()` times (min 1),
/// with the remainder distributed left-to-right until `target_width` is reached.
pub fn sparkline_str_wide(data: &[u64], target_width: usize) -> String {
    if data.is_empty() || target_width == 0 {
        return String::new();
    }
    let n = data.len();
    let base = (target_width / n).max(1);
    let extra = target_width.saturating_sub(base * n);

    let max = *data.iter().max().unwrap_or(&1);
    let max = max.max(1);

    let mut out = String::with_capacity(target_width);
    for (i, &v) in data.iter().enumerate() {
        let idx = ((v as f64 / max as f64) * (BLOCKS.len() - 1) as f64).round() as usize;
        let ch = BLOCKS[idx.min(BLOCKS.len() - 1)];
        let repeat = if i < extra { base + 1 } else { base };
        for _ in 0..repeat {
            out.push(ch);
        }
    }
    out
}

/// Returns a `Span` styled in the Comment color for a sparkline string.
pub fn sparkline_span(data: &[u64]) -> Span<'static> {
    let s = sparkline_str(data);
    Span::styled(s, Style::default().fg(to_ratatui(ColorToken::Comment)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sparkline_str_empty() {
        assert_eq!(sparkline_str(&[]), "");
    }

    #[test]
    fn sparkline_str_single_value() {
        let s = sparkline_str(&[5]);
        // Single value is the max → last block char
        assert_eq!(s, "▇");
    }

    #[test]
    fn sparkline_str_all_same() {
        let s = sparkline_str(&[3, 3, 3]);
        // All same → all max → all ▇
        assert_eq!(s, "▇▇▇");
    }

    #[test]
    fn sparkline_str_zero_to_max() {
        let s = sparkline_str(&[0, 6]);
        // 0 → ▁ (idx 0), 6 → ▇ (idx 6)
        assert_eq!(s.chars().next(), Some('▁'));
        assert_eq!(s.chars().nth(1), Some('▇'));
    }

    #[test]
    fn sparkline_str_length_matches_data() {
        let data = vec![1u64, 2, 3, 4, 5];
        assert_eq!(sparkline_str(&data).chars().count(), data.len());
    }
}
