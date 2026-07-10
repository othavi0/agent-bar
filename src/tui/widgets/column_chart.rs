//! Chart de colunas horárias empilhadas por modelo (v8, spec §2).
//! Escala √ no eixo Y; uso >0 nunca fica invisível (≥1 oitavo).

use ratatui::style::Style;
use ratatui::text::{Line, Span};
use time::{OffsetDateTime, UtcOffset};

use crate::theme::{series_token, ColorToken};
use crate::tui::theme_bridge::to_ratatui;
use crate::usage::buckets::ModelHourSeries;

/// Blocos parciais por oitavos (índice 1..=7); célula cheia é `█`.
const EIGHTHS: [&str; 8] = ["", "▁", "▂", "▃", "▄", "▅", "▆", "▇"];
const Y_AXIS_W: usize = 6; // "999M ┤"

pub fn column_chart_lines(
    series: &[ModelHourSeries],
    width: u16,
    height: u16,
    now: OffsetDateTime,
    local_offset: UtcOffset,
) -> Vec<Line<'static>> {
    let width = width as usize;
    let height = height as usize;
    if series.is_empty() {
        let mut out = vec![Line::from(Span::styled(
            " sem uso de tokens no período".to_string(),
            Style::default().fg(to_ratatui(ColorToken::Comment)),
        ))];
        out.resize(height.max(1), Line::default());
        return out;
    }

    let buckets = series.iter().map(|s| s.tokens.len()).max().unwrap_or(0);
    // Reserva: 1 linha eixo X, 1 linha labels X, 1 linha legenda.
    let plot_rows = height.saturating_sub(3).max(3);

    // Largura de coluna: tenta 2+1 gap, senão 1+1, senão 1+0.
    let avail = width.saturating_sub(Y_AXIS_W);
    let (col_w, gap) = if buckets * 3 <= avail {
        (2usize, 1usize)
    } else if buckets * 2 <= avail {
        (1, 1)
    } else {
        (1, 0)
    };

    // Total por bucket e máximo (pra escala √).
    let totals: Vec<u64> = (0..buckets)
        .map(|i| {
            series
                .iter()
                .map(|s| s.tokens.get(i).copied().unwrap_or(0))
                .sum()
        })
        .collect();
    let max_total = totals.iter().copied().max().unwrap_or(0).max(1);
    let cap = plot_rows * 8; // resolução em oitavos

    // Altura empilhada por (bucket, série), em oitavos.
    let mut stacks: Vec<Vec<(usize, u8)>> = Vec::with_capacity(buckets); // (altura, slot)
    for (i, &total) in totals.iter().enumerate() {
        let mut cols = Vec::new();
        if total > 0 {
            let h_total = (((total as f64 / max_total as f64).sqrt()) * cap as f64)
                .round()
                .max(1.0) as usize;
            let mut used = 0usize;
            let active: Vec<&ModelHourSeries> = series
                .iter()
                .filter(|s| s.tokens.get(i).copied().unwrap_or(0) > 0)
                .collect();
            for (k, s) in active.iter().enumerate() {
                let v = s.tokens.get(i).copied().unwrap_or(0);
                let mut h = if k + 1 == active.len() {
                    h_total.saturating_sub(used) // último leva o resto (soma exata)
                } else {
                    ((v as f64 / total as f64) * h_total as f64).round() as usize
                };
                h = h.max(1); // uso >0 nunca invisível
                cols.push((h, s.slot));
                used += h;
            }
        }
        stacks.push(cols);
    }

    let mut out: Vec<Line<'static>> = Vec::with_capacity(height);

    // Plot, de cima pra baixo.
    for row in (0..plot_rows).rev() {
        let lo = row * 8;
        let mut spans: Vec<Span<'static>> = Vec::new();
        // Rótulo Y: topo = max, meio = valor da escala √ na metade, base tratada no eixo.
        let label = if row + 1 == plot_rows {
            fmt_tokens_short(max_total)
        } else if row == plot_rows / 2 {
            // valor cuja √-fração corresponde à metade da altura
            fmt_tokens_short(((0.5f64 * 0.5) * max_total as f64) as u64)
        } else {
            String::new()
        };
        let axis = if label.is_empty() {
            format!("{:>5}│", "")
        } else {
            format!("{label:>5}┤")
        };
        spans.push(Span::styled(
            axis,
            Style::default().fg(to_ratatui(ColorToken::Comment)),
        ));

        for cols in &stacks {
            // Descobre o que esta célula (linha `row`) mostra nesta coluna.
            // `fill` = quantos oitavos desta célula [lo, lo+8) estão dentro
            // da pilha total (h_total). Linhas abaixo do topo da pilha são
            // SEMPRE cheias (█), mesmo que duas séries se encontrem no meio
            // da célula (uma termina, outra começa) — só a linha do TOPO da
            // pilha é parcial. Cor: a série que ocupa a MAIOR parte da faixa
            // visível [lo, lo+fill) pinta a célula (desempate: primeira
            // encontrada = slot mais baixo, já que `cols` vem slot-ordenada).
            let h_total: usize = cols.iter().map(|(h, _)| *h).sum();
            let fill = h_total.saturating_sub(lo).min(8);
            let (glyph, style) = if fill == 0 {
                (" ".to_string(), Style::default())
            } else {
                let mut base = 0usize;
                let mut best: Option<(usize, u8)> = None; // (oitavos ocupados, slot)
                for (h, slot) in cols {
                    let top = base + h;
                    let overlap_lo = base.max(lo);
                    let overlap_hi = top.min(lo + fill);
                    if overlap_hi > overlap_lo {
                        let occ = overlap_hi - overlap_lo;
                        let better = match &best {
                            None => true,
                            Some((best_occ, _)) => occ > *best_occ,
                        };
                        if better {
                            best = Some((occ, *slot));
                        }
                    }
                    base = top;
                }
                let slot = best.map(|(_, s)| s).unwrap_or(0);
                let g = if fill == 8 {
                    "█".to_string()
                } else {
                    EIGHTHS[fill].to_string()
                };
                (g, Style::default().fg(to_ratatui(series_token(slot))))
            };
            spans.push(Span::styled(glyph.repeat(col_w), style));
            if gap > 0 {
                spans.push(Span::raw(" ".repeat(gap)));
            }
        }
        out.push(Line::from(spans));
    }

    // Eixo X.
    let plot_w = buckets * (col_w + gap);
    out.push(Line::from(Span::styled(
        format!(
            "{:>5}┴{}",
            "0",
            "─".repeat(plot_w.min(width.saturating_sub(Y_AXIS_W)))
        ),
        Style::default().fg(to_ratatui(ColorToken::Comment)),
    )));

    // Labels de hora (a cada 3 buckets).
    let mut xl = String::with_capacity(width);
    xl.push_str(&" ".repeat(Y_AXIS_W));
    for i in 0..buckets {
        if i % 3 == 0 {
            let bucket_time = now - time::Duration::hours((buckets - 1 - i) as i64);
            let h = bucket_time.to_offset(local_offset).hour();
            let lab = format!("{h:02}h");
            let pos = Y_AXIS_W + i * (col_w + gap);
            while xl.chars().count() < pos {
                xl.push(' ');
            }
            if xl.chars().count() + lab.len() <= width {
                xl.push_str(&lab);
            }
        }
    }
    out.push(Line::from(Span::styled(
        xl,
        Style::default().fg(to_ratatui(ColorToken::Comment)),
    )));

    // Legenda.
    let mut legend: Vec<Span<'static>> = Vec::new();
    for s in series {
        legend.push(Span::styled(
            "  ● ".to_string(),
            Style::default().fg(to_ratatui(series_token(s.slot))),
        ));
        legend.push(Span::styled(
            format!("{} {}", s.label, fmt_tokens_short(s.total)),
            Style::default().fg(to_ratatui(ColorToken::Text)),
        ));
    }
    out.push(Line::from(legend));

    // Garante exatamente `height` linhas (corta plot excedente já evitado acima).
    out.truncate(height);
    while out.len() < height {
        out.push(Line::default());
    }
    out
}

/// "264,7M", "1,5B", "980k", "42" — formato curto pt-BR (vírgula decimal).
pub fn fmt_tokens_short(t: u64) -> String {
    let f = t as f64;
    if f >= 1e9 {
        format!("{:.1}B", f / 1e9).replace('.', ",")
    } else if f >= 1e6 {
        format!("{:.1}M", f / 1e6).replace('.', ",")
    } else if f >= 1e3 {
        format!("{:.0}k", f / 1e3)
    } else {
        format!("{t}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::usage::buckets::ModelHourSeries;
    use time::macros::datetime;

    fn series(label: &str, slot: u8, tokens: Vec<u64>) -> ModelHourSeries {
        let total = tokens.iter().sum();
        ModelHourSeries {
            label: label.into(),
            slot,
            tokens,
            total,
        }
    }

    fn plain(lines: &[ratatui::text::Line<'_>]) -> Vec<String> {
        lines
            .iter()
            .map(|l| {
                l.spans
                    .iter()
                    .map(|s| s.content.as_ref())
                    .collect::<String>()
            })
            .collect()
    }

    #[test]
    fn chart_has_requested_height_and_legend() {
        let s = vec![
            series("Fable 5", 0, vec![0, 10, 100, 50]),
            series("Opus 4.8", 1, vec![0, 5, 0, 0]),
        ];
        let lines = column_chart_lines(
            &s,
            60,
            10,
            datetime!(2026-07-10 12:00:00 UTC),
            time::UtcOffset::UTC,
        );
        assert_eq!(lines.len(), 10);
        let text = plain(&lines);
        let legend = text.last().unwrap();
        assert!(legend.contains("Fable 5"), "legenda: {legend}");
        assert!(legend.contains("Opus 4.8"));
    }

    #[test]
    fn chart_nonzero_bucket_is_never_invisible() {
        // Fable enorme + Opus minúsculo no mesmo bucket: Opus ganha ≥1 oitavo.
        let s = vec![
            series("Fable 5", 0, vec![1_000_000]),
            series("Opus 4.8", 1, vec![1]),
        ];
        let lines = column_chart_lines(
            &s,
            30,
            8,
            datetime!(2026-07-10 12:00:00 UTC),
            time::UtcOffset::UTC,
        );
        // Alguma célula do plot deve usar a cor do slot 1 (Series2).
        let series2 = crate::tui::theme_bridge::to_ratatui(crate::theme::series_token(1));
        let has_opus_cell = lines.iter().any(|l| {
            l.spans
                .iter()
                .any(|sp| sp.style.fg == Some(series2) && !sp.content.trim().is_empty())
        });
        assert!(has_opus_cell, "série minúscula não pode sumir do chart");
    }

    #[test]
    fn chart_empty_series_shows_empty_state() {
        let lines = column_chart_lines(
            &[],
            40,
            8,
            datetime!(2026-07-10 12:00:00 UTC),
            time::UtcOffset::UTC,
        );
        let text = plain(&lines).join("\n");
        assert!(text.contains("sem uso"), "estado vazio desenhado: {text}");
    }

    #[test]
    fn chart_lines_never_exceed_width() {
        let s = vec![series("Fable 5", 0, (0..24).map(|i| i * 1000).collect())];
        for w in [30u16, 60, 100] {
            let lines = column_chart_lines(
                &s,
                w,
                9,
                datetime!(2026-07-10 12:00:00 UTC),
                time::UtcOffset::UTC,
            );
            for l in plain(&lines) {
                assert!(l.chars().count() <= w as usize, "linha estourou {w}: {l:?}");
            }
        }
    }

    #[test]
    fn chart_snapshot_two_series() {
        let s = vec![
            series(
                "Fable 5",
                0,
                vec![
                    0, 0, 17, 46, 75, 19, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 8, 140,
                ],
            ),
            series(
                "Opus 4.8",
                1,
                vec![
                    0, 0, 9, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                ],
            ),
        ];
        let lines = column_chart_lines(
            &s,
            84,
            12,
            datetime!(2026-07-10 12:00:00 UTC),
            time::UtcOffset::UTC,
        );
        let text: Vec<String> = plain(&lines);
        insta::assert_snapshot!(text.join("\n"));
    }
}
