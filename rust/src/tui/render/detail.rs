use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph};
use ratatui::Frame;

use crate::theme::ColorToken;
use crate::tui::state::AppState;
use crate::tui::theme_bridge::{provider_color, to_ratatui};

/// Width (chars) of the block bar for window gauges.
const BAR_WIDTH: usize = 18;
/// Width (chars) of the block bar for model mini-gauges.
const MINI_BAR_WIDTH: usize = 12;

/// Builds a block bar string: filled (█) = remaining; empty (░) = consumed.
fn block_bar(remaining_pct: f64, width: usize) -> String {
    let remaining = remaining_pct.clamp(0.0, 100.0);
    let filled = ((remaining / 100.0) * width as f64).round() as usize;
    let filled = filled.min(width);
    let empty = width - filled;
    format!("{}{}", "\u{2588}".repeat(filled), "\u{2591}".repeat(empty))
}

/// Returns a severity color for the given remaining percentage.
fn severity_color(remaining_pct: f64) -> ratatui::style::Color {
    if remaining_pct >= 60.0 {
        to_ratatui(ColorToken::Green)
    } else if remaining_pct >= 30.0 {
        to_ratatui(ColorToken::Yellow)
    } else if remaining_pct >= 10.0 {
        to_ratatui(ColorToken::Orange)
    } else {
        to_ratatui(ColorToken::Red)
    }
}

/// Formats a reset time string from an ISO timestamp or raw string.
/// Extracts HH:MM if ISO, else returns raw string or "-".
fn fmt_reset(resets_at: Option<&str>) -> String {
    match resets_at {
        None => "-".to_string(),
        Some(s) => s
            .split('T')
            .nth(1)
            .and_then(|t| t.get(..5))
            .map(|hm| hm.to_string())
            .unwrap_or_else(|| s.to_string()),
    }
}

/// Renders the Detail view for the selected provider (Mode::Detail).
pub fn render_detail(state: &AppState, frame: &mut Frame, area: Rect) {
    let provider = match state.providers.get(state.selected) {
        Some(pv) => pv,
        None => return render_empty(frame, area),
    };
    let q = &provider.quota;
    let p_color = provider_color(&q.provider);

    // Title: "Name · Plan" or just "Name"
    let title = match &q.plan {
        Some(plan) => format!(" {} \u{b7} {} ", q.display_name, plan),
        None => format!(" {} ", q.display_name),
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Thick)
        .border_style(Style::default().fg(p_color))
        .title(Span::styled(
            title,
            Style::default()
                .fg(to_ratatui(ColorToken::TextBright))
                .add_modifier(Modifier::BOLD),
        ));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Build content lines
    let mut lines: Vec<Line<'_>> = Vec::new();

    // --- Window gauges ---
    // Primary (5h)
    if let Some(primary) = &q.primary {
        let rem = primary.remaining;
        let bar = block_bar(rem, BAR_WIDTH);
        let color = severity_color(rem);
        let pct_str = format!("{:3.0}%", rem);
        let reset_str = fmt_reset(primary.resets_at.as_deref());

        lines.push(Line::from(vec![
            Span::styled(" 5h  ", Style::default().fg(to_ratatui(ColorToken::Muted))),
            Span::styled(bar, Style::default().fg(color)),
            Span::styled(
                format!("  {}  ", pct_str),
                Style::default().fg(to_ratatui(ColorToken::TextBright)),
            ),
            Span::styled(
                format!("\u{2192} {}", reset_str),
                Style::default().fg(to_ratatui(ColorToken::Comment)),
            ),
        ]));
    }

    // Secondary (wk)
    if let Some(secondary) = &q.secondary {
        let rem = secondary.remaining;
        let bar = block_bar(rem, BAR_WIDTH);
        let color = severity_color(rem);
        let pct_str = format!("{:3.0}%", rem);
        let reset_str = fmt_reset(secondary.resets_at.as_deref());

        lines.push(Line::from(vec![
            Span::styled(" wk  ", Style::default().fg(to_ratatui(ColorToken::Muted))),
            Span::styled(bar, Style::default().fg(color)),
            Span::styled(
                format!("  {}  ", pct_str),
                Style::default().fg(to_ratatui(ColorToken::TextBright)),
            ),
            Span::styled(
                format!("\u{2192} {}", reset_str),
                Style::default().fg(to_ratatui(ColorToken::Comment)),
            ),
        ]));
    }

    // --- Models section ---
    if let Some(models) = &q.models {
        if !models.is_empty() {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                " Models",
                Style::default()
                    .fg(to_ratatui(ColorToken::TextBright))
                    .add_modifier(Modifier::BOLD),
            )));

            for (model_name, window) in models {
                let rem = window.remaining;
                let bar = block_bar(rem, MINI_BAR_WIDTH);
                let color = severity_color(rem);
                let pct_str = format!("{:3.0}%", rem);
                // Truncate model name to 8 chars for alignment
                let name_trunc: &str = if model_name.len() > 8 {
                    &model_name[..8]
                } else {
                    model_name.as_str()
                };

                lines.push(Line::from(vec![
                    Span::styled(
                        format!("   {:<8}  ", name_trunc),
                        Style::default().fg(to_ratatui(ColorToken::Text)),
                    ),
                    Span::styled(bar, Style::default().fg(color)),
                    Span::styled(
                        format!("  {}", pct_str),
                        Style::default().fg(to_ratatui(ColorToken::Muted)),
                    ),
                ]));
            }
        }
    }

    // --- Footer placeholders ---
    // Sparkline placeholder (T10) and cost placeholder (T5)
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled(
            " tokens/h \u{2581}\u{2582}\u{2583}\u{2585}\u{2587}\u{2586}\u{2584}\u{2582}\u{2581}",
            Style::default().fg(to_ratatui(ColorToken::Comment)),
        ),
        Span::styled(
            "          ~$\u{2014} hoje",
            Style::default().fg(to_ratatui(ColorToken::Muted)),
        ),
    ]));

    // Split inner area: use full width for paragraph
    let content_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0)])
        .split(inner);

    let para = Paragraph::new(lines);
    frame.render_widget(para, content_layout[0]);
}

/// Fallback when no provider is selected.
fn render_empty(frame: &mut Frame, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Thick)
        .border_style(Style::default().fg(to_ratatui(ColorToken::Comment)));

    let para = Paragraph::new(Span::styled(
        " Nenhum provider selecionado",
        Style::default().fg(to_ratatui(ColorToken::Muted)),
    ))
    .block(block);

    frame.render_widget(para, area);
}

#[cfg(test)]
mod tests {
    use indexmap::IndexMap;

    use crate::providers::types::{ProviderQuota, QuotaWindow};
    use crate::tui::render::render;
    use crate::tui::state::{AppState, FetchStatus, Mode, ProviderView};

    fn make_window(remaining: f64, resets_at: Option<&str>) -> QuotaWindow {
        QuotaWindow {
            remaining,
            resets_at: resets_at.map(|s| s.to_string()),
            window_minutes: Some(300),
            used: Some(100.0 - remaining),
        }
    }

    fn make_claude_provider() -> ProviderView {
        let mut models: IndexMap<String, QuotaWindow> = IndexMap::new();
        models.insert("Opus".to_string(), make_window(41.0, None));
        models.insert("Sonnet".to_string(), make_window(20.0, None));

        ProviderView::new(ProviderQuota {
            provider: "claude".to_string(),
            display_name: "Claude".to_string(),
            available: true,
            account: None,
            plan: Some("Max 5x".to_string()),
            plan_type: None,
            primary: Some(make_window(26.0, Some("2026-06-19T23:00:00Z"))),
            secondary: Some(make_window(12.0, Some("2026-06-22T00:00:00Z"))),
            models: Some(models),
            extra: None,
            error: None,
        })
    }

    #[test]
    fn detail_renders_provider_windows_and_models() {
        let backend = ratatui::backend::TestBackend::new(64, 20);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        let mut state = AppState::new();
        state.providers = vec![make_claude_provider()];
        state.selected = 0;
        state.mode = Mode::Detail;
        state.status = FetchStatus::Loaded;
        terminal.draw(|f| render(&state, f)).unwrap();
        insta::assert_snapshot!(terminal.backend());
    }
}
