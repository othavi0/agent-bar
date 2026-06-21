use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph};
use ratatui::Frame;

use crate::theme::ColorToken;
use crate::tui::state::AppState;
use crate::tui::theme_bridge::{provider_color, to_ratatui};
use crate::tui::widgets::quota_gauge::block_bar;
use crate::tui::widgets::severity::severity_color as sev_color;
use crate::usage::{ModelUsage, ProviderUsage};

/// Derives bar width from available area width for window gauges.
/// Fixed prefix: " 5h  " (5) + "  PCT%  " (8) + "-> HH:MM" (8) + borders(2) = 23
/// At least 18 chars for the bar.
fn derive_bar_width(area_width: u16) -> usize {
    (area_width as usize).saturating_sub(23).max(18)
}

/// Derives mini bar width (model gauges) as 2/3 of bar_width, at least 12.
fn derive_mini_bar_width(bar_width: usize) -> usize {
    (bar_width * 2 / 3).max(12)
}

/// Encontra um ModelUsage cujo nome contem `quota_name` (case-insensitive).
/// Necessario porque o nome no quota (ex "Opus") e curto, enquanto o nome no
/// usage engine e completo (ex "claude-opus-4-8").
fn find_model_usage<'a>(by_model: &'a [ModelUsage], quota_name: &str) -> Option<&'a ModelUsage> {
    let lower = quota_name.to_lowercase();
    by_model
        .iter()
        .find(|mu| mu.model.to_lowercase().contains(&lower))
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
    let bar_width = derive_bar_width(area.width);
    let mini_bar_width = derive_mini_bar_width(bar_width);

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
        let bar = block_bar(rem, bar_width);
        let color = sev_color(Some(rem));
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
        let bar = block_bar(rem, bar_width);
        let color = sev_color(Some(rem));
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

    // Lookup usage data for this provider
    let provider_usage: Option<&ProviderUsage> = state
        .usage
        .as_ref()
        .and_then(|s| s.providers.iter().find(|pu| pu.provider == q.provider));

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
                let bar = block_bar(rem, mini_bar_width);
                let color = sev_color(Some(rem));
                let pct_str = format!("{:3.0}%", rem);
                // Truncate model name to 8 chars for alignment
                let name_trunc: &str = if model_name.len() > 8 {
                    &model_name[..8]
                } else {
                    model_name.as_str()
                };

                // Lookup custo por modelo no usage engine
                let model_cost_str: String = provider_usage
                    .and_then(|pu| {
                        // Match por prefixo: model_name do quota pode ser curto (ex "Opus"),
                        // enquanto o model do usage e o nome completo (ex "claude-opus-4-8").
                        // Estrategia: encontra o ModelUsage cujo nome contem o model_name (case-insensitive).
                        find_model_usage(&pu.by_model, model_name)
                    })
                    .map(|mu| match &mu.cost {
                        Some(c) => format!("  ${:.2}", c.usd),
                        None => "  -".to_string(),
                    })
                    .unwrap_or_default();

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
                    Span::styled(
                        model_cost_str,
                        Style::default().fg(to_ratatui(ColorToken::Comment)),
                    ),
                ]));
            }
        }
    }

    // --- Footer: sparkline placeholder (T10) + custo real (T5) ---
    let cost_str: String = match provider_usage {
        Some(pu) if pu.provider == "amp" => {
            // Amp: mostra saldo de credito
            pu.amp_dollars
                .as_ref()
                .and_then(|ad| ad.remaining)
                .map(|r| format!("cr ${:.2} hoje", r))
                .unwrap_or_else(|| "-".to_string())
        }
        Some(pu) => match &pu.cost {
            Some(c) => format!("~${:.2} hoje", c.usd),
            None => "-".to_string(),
        },
        None => "-".to_string(),
    };

    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled(
            " tokens/h \u{2581}\u{2582}\u{2583}\u{2585}\u{2587}\u{2586}\u{2584}\u{2582}\u{2581}",
            Style::default().fg(to_ratatui(ColorToken::Comment)),
        ),
        Span::styled(
            format!("          {}", cost_str),
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
    use crate::usage::amp::AmpDollars;
    use crate::usage::{Cost, ModelUsage, ProviderUsage, UsageSummary};

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

    /// Constroi um UsageSummary falso para testes:
    /// - claude: $2.10 / R$11.55; modelos opus($1.40) e sonnet(cost None)
    /// - codex: tokens sem custo conhecido (cost None)
    /// - amp: amp_dollars (remaining $4.19)
    fn fake_usage() -> UsageSummary {
        UsageSummary {
            providers: vec![
                ProviderUsage {
                    provider: "claude".to_string(),
                    total_input: 1_000_000,
                    total_output: 200_000,
                    total_cache_read: 0,
                    total_cache_write: 0,
                    cost: Some(Cost {
                        usd: 2.10,
                        brl: 11.55,
                    }),
                    by_model: vec![
                        ModelUsage {
                            model: "claude-opus-4-8".to_string(),
                            input: 800_000,
                            output: 100_000,
                            cache_read: 0,
                            cache_write: 0,
                            cost: Some(Cost {
                                usd: 1.40,
                                brl: 7.70,
                            }),
                        },
                        ModelUsage {
                            model: "claude-sonnet-4-6".to_string(),
                            input: 200_000,
                            output: 100_000,
                            cache_read: 0,
                            cache_write: 0,
                            cost: None,
                        },
                    ],
                    amp_dollars: None,
                },
                ProviderUsage {
                    provider: "codex".to_string(),
                    total_input: 500_000,
                    total_output: 80_000,
                    total_cache_read: 0,
                    total_cache_write: 0,
                    cost: None,
                    by_model: vec![ModelUsage {
                        model: "gpt-5.5".to_string(),
                        input: 500_000,
                        output: 80_000,
                        cache_read: 0,
                        cache_write: 0,
                        cost: None,
                    }],
                    amp_dollars: None,
                },
                ProviderUsage {
                    provider: "amp".to_string(),
                    total_input: 0,
                    total_output: 0,
                    total_cache_read: 0,
                    total_cache_write: 0,
                    cost: None,
                    by_model: vec![],
                    amp_dollars: Some(AmpDollars {
                        spent: Some(0.81),
                        remaining: Some(4.19),
                        total: Some(5.0),
                    }),
                },
            ],
            total_cost: Cost {
                usd: 2.10,
                brl: 11.55,
            },
            fx_rate: 5.50,
        }
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

    #[test]
    fn detail_renders_with_real_cost() {
        let backend = ratatui::backend::TestBackend::new(64, 20);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        let mut state = AppState::new();
        state.providers = vec![make_claude_provider()];
        state.selected = 0;
        state.mode = Mode::Detail;
        state.status = FetchStatus::Loaded;
        state.usage = Some(fake_usage());
        terminal.draw(|f| render(&state, f)).unwrap();
        insta::assert_snapshot!(terminal.backend());
    }

    #[test]
    fn detail_renders_wide_160() {
        let backend = ratatui::backend::TestBackend::new(160, 40);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        let mut state = AppState::new();
        state.providers = vec![make_claude_provider()];
        state.selected = 0;
        state.mode = Mode::Detail;
        state.status = FetchStatus::Loaded;
        state.usage = Some(fake_usage());
        terminal.draw(|f| render(&state, f)).unwrap();
        insta::assert_snapshot!(terminal.backend());
    }
}
