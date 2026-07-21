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
/// "999M ┤" — reserva de colunas do eixo Y. `pub(crate)` porque
/// `history.rs::render_top_chart` precisa da mesma largura pro cálculo de
/// fit (downsampling da semana) sem manter um número mágico espelhado.
pub(crate) const Y_AXIS_W: usize = 6;

/// Abreviação PT de dia-da-semana p/ os labels de eixo X do modo bucketed
/// (`bucket_hours > 1`) — cópia pequena e autocontida de
/// `history.rs::weekday_abbrev` (módulos não devem depender um do outro só
/// por isso; o mapeamento é trivial e estável).
fn weekday_abbrev_pt(w: time::Weekday) -> &'static str {
    match w {
        time::Weekday::Monday => "seg",
        time::Weekday::Tuesday => "ter",
        time::Weekday::Wednesday => "qua",
        time::Weekday::Thursday => "qui",
        time::Weekday::Friday => "sex",
        time::Weekday::Saturday => "sáb",
        time::Weekday::Sunday => "dom",
    }
}

/// Valor agregado da série `tokens` na coluna `i` do chart. Com
/// `bucket_hours == 1` é `tokens[i]` direto (resolução horária, idêntico ao
/// comportamento antigo). Com `bucket_hours > 1`, soma as `bucket_hours`
/// horas consecutivas `[i*bucket_hours, (i+1)*bucket_hours)` — o último
/// grupo pode ser parcial se `tokens.len()` não for múltiplo exato.
fn bucketed_value(tokens: &[u64], i: usize, bucket_hours: usize) -> u64 {
    if bucket_hours <= 1 {
        return tokens.get(i).copied().unwrap_or(0);
    }
    let lo = i.saturating_mul(bucket_hours);
    if lo >= tokens.len() {
        return 0;
    }
    let hi = (lo + bucket_hours).min(tokens.len());
    tokens[lo..hi].iter().sum()
}

/// Chart de colunas em resolução horária (`bucket_hours = 1`) — delegação
/// fina pra `column_chart_lines_bucketed`; assinatura preservada pro
/// `detail.rs` e os testes existentes.
pub fn column_chart_lines(
    series: &[ModelHourSeries],
    width: u16,
    height: u16,
    now: OffsetDateTime,
    local_offset: UtcOffset,
) -> Vec<Line<'static>> {
    column_chart_lines_bucketed(series, width, height, now, local_offset, 1)
}

/// Chart de colunas com downsampling horizontal opcional. `series[].tokens`
/// chega SEMPRE em resolução 1h; com `bucket_hours > 1`, cada grupo de N
/// horas consecutivas vira 1 coluna (soma), permitindo cobrir uma janela
/// maior (ex. 168h/semana) sem estourar a largura do terminal. Os totais da
/// legenda continuam a soma de TODAS as horas de `series[].total` — o
/// downsampling só afeta o desenho das colunas, nunca os totais.
pub fn column_chart_lines_bucketed(
    series: &[ModelHourSeries],
    width: u16,
    height: u16,
    now: OffsetDateTime,
    local_offset: UtcOffset,
    bucket_hours: usize,
) -> Vec<Line<'static>> {
    let bucket_hours = bucket_hours.max(1);
    let width = width as usize;
    let height = height as usize;
    if series.is_empty() {
        let height = height.max(1);
        let msg = Line::from(Span::styled(
            " sem uso de tokens no período".to_string(),
            Style::default().fg(to_ratatui(ColorToken::Comment)),
        ));
        // Centraliza verticalmente: preenche até a metade com linhas em
        // branco, emite a mensagem, e completa até `height` — em vez de
        // jogar a mensagem no topo e deixar um bloco em branco embaixo.
        let top_pad = height.saturating_sub(1) / 2;
        let mut out = vec![Line::default(); top_pad];
        out.push(msg);
        out.resize(height, Line::default());
        return out;
    }

    let hourly_len = series.iter().map(|s| s.tokens.len()).max().unwrap_or(0);
    // Nº de COLUNAS do chart: em resolução horária é `hourly_len` direto; com
    // downsampling, cada grupo de `bucket_hours` horas vira 1 coluna (o
    // último grupo pode ser parcial — `div_ceil` conta essa coluna também).
    let buckets = if bucket_hours <= 1 {
        hourly_len
    } else {
        hourly_len.div_ceil(bucket_hours)
    };
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
                .map(|s| bucketed_value(&s.tokens, i, bucket_hours))
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
            // `h_total` já é ≤ cap (fração √ ∈ [0,1]), mas usamos `.min(cap)`
            // como meta defensiva mesmo assim — ver invariante abaixo.
            let target = h_total.min(cap);
            let mut used = 0usize;
            let active: Vec<&ModelHourSeries> = series
                .iter()
                .filter(|s| bucketed_value(&s.tokens, i, bucket_hours) > 0)
                .collect();
            let mut heights: Vec<(usize, u8)> = Vec::with_capacity(active.len());
            for (k, s) in active.iter().enumerate() {
                let v = bucketed_value(&s.tokens, i, bucket_hours);
                let mut h = if k + 1 == active.len() {
                    target.saturating_sub(used) // último leva o resto (soma exata)
                } else {
                    ((v as f64 / total as f64) * target as f64).round() as usize
                };
                h = h.max(1); // uso >0 nunca invisível
                used += h;
                heights.push((h, s.slot));
            }
            // Invariante pós-loop: soma(heights) == min(h_total, cap) E toda
            // série com tokens>0 mantém altura ≥1. O `.max(1)` acima pode
            // empurrar a soma além de `target`/`cap` — oitavos acima do teto
            // nunca são desenhados por NENHUMA linha do plot, então a série
            // minoritária ficaria invisível de novo. Rouba o excesso da MAIOR
            // série (a única que tem folga pra ceder sem zerar ninguém).
            let sum: usize = heights.iter().map(|(h, _)| *h).sum();
            let mut excess = sum.saturating_sub(target);
            while excess > 0 {
                match heights
                    .iter()
                    .enumerate()
                    .filter(|(_, (h, _))| *h > 1)
                    .max_by_key(|(_, (h, _))| *h)
                    .map(|(idx, _)| idx)
                {
                    Some(idx) => {
                        heights[idx].0 -= 1;
                        excess -= 1;
                    }
                    // Não há mais de onde roubar sem derrubar alguma série
                    // abaixo de 1 oitavo (mais séries ativas do que oitavos
                    // disponíveis nesta faixa) — situação degenerada e
                    // DOCUMENTADA: para aqui em vez de violar o mínimo de
                    // visibilidade de outra série pra manter este. O resgate
                    // de linha em `cell_grid` (abaixo) tem o mesmo tipo de
                    // teto honesto quando sobram mais séries ativas do que
                    // linhas do plot — não é bug, é o limite físico da
                    // resolução escolhida.
                    None => break,
                }
            }
            cols.extend(heights);
        }
        stacks.push(cols);
    }

    // Pré-computa, por coluna, (fill, slot) de CADA linha do plot.
    // Por que precisa disso além do fix de soma acima: mesmo com a soma das
    // alturas correta (≤ cap), a votação por maioria dentro de uma janela de
    // 8 oitavos pode enterrar uma série minoritária que nunca tem folego pra
    // vencer nenhuma linha (ex.: série dominante ocupa 7/8 oitavos da linha-
    // limite, minoritária fica com 1/8 e perde SEMPRE) — ela ficaria com
    // altura>0 nos dados mas invisível na tela, violando o mesmo contrato do
    // header. Resgate: se uma série com altura>0 nunca vence a votação em
    // nenhuma linha, força a cor dela na linha onde teve a MAIOR sobreposição
    // (só recolore aquela linha; não mexe no fill/glyph de ninguém).
    //
    // Colisão entre resgates (achado de review): com 3+ séries num bucket é
    // possível DUAS séries minoritárias terem a MESMA única linha de overlap
    // (ex.: A=1 oitavo e B=1 oitavo, ambos inteiramente dentro da linha-base,
    // enquanto C domina as linhas de cima). Um resgate ingênuo processa em
    // ordem de stack e sobrescreve cegamente — a segunda série resgatada
    // reenterra a primeira, violando o mesmo contrato que o resgate deveria
    // proteger. A correção usa reserva: linhas já concedidas nesta passada,
    // ou que são a ÚNICA linha visível de outra série, não podem ser
    // roubadas de novo. Quando nem a melhor linha de overlap real está livre,
    // um último recurso rouba uma linha de QUALQUER série que ainda tenha
    // folga (visible_count > 1) — mesmo princípio de "rouba de quem pode
    // ceder sem sumir" do excess-steal acima — preferindo quem tem MAIS
    // folga. Isso troca precisão geométrica (a linha recolorida pode não ter
    // overlap real com a série resgatada) pelo contrato "uso>0 nunca
    // invisível", e só se aplica quando ainda cabe: se o número de séries
    // ativas exceder o de linhas do plot, sobra série sem resgate possível —
    // degenerado e documentado, mesmo bound do excess-steal.
    let mut cell_grid: Vec<Vec<(usize, Option<u8>)>> = Vec::with_capacity(stacks.len());
    for cols in &stacks {
        let h_total: usize = cols.iter().map(|(h, _)| *h).sum();
        let mut rows: Vec<(usize, Option<u8>)> = Vec::with_capacity(plot_rows);
        // occ_by_row[row] = overlaps (slot, oitavos) de TODA série que tocou
        // essa linha, mesmo perdendo a votação — usado só no resgate abaixo.
        let mut occ_by_row: Vec<Vec<(u8, usize)>> = vec![Vec::new(); plot_rows];
        for (row, occ_row) in occ_by_row.iter_mut().enumerate() {
            let lo = row * 8;
            let fill = h_total.saturating_sub(lo).min(8);
            if fill == 0 {
                rows.push((0, None));
                continue;
            }
            let mut base = 0usize;
            let mut best: Option<(usize, u8)> = None; // (oitavos ocupados, slot)
            for (h, slot) in cols {
                let top = base + h;
                let overlap_lo = base.max(lo);
                let overlap_hi = top.min(lo + fill);
                if overlap_hi > overlap_lo {
                    let occ = overlap_hi - overlap_lo;
                    occ_row.push((*slot, occ));
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
            rows.push((fill, best.map(|(_, s)| s)));
        }
        // Dono atual de cada linha + quantas linhas cada slot ainda detém
        // (usado pra saber se uma linha é a ÚNICA visível de alguém).
        let mut row_owner: Vec<Option<u8>> = rows.iter().map(|(_, s)| *s).collect();
        let mut visible_count: std::collections::HashMap<u8, usize> =
            std::collections::HashMap::new();
        for owner in row_owner.iter().flatten() {
            *visible_count.entry(*owner).or_insert(0) += 1;
        }

        // Séries com tokens>0 que nunca venceram nenhuma linha, com as linhas
        // onde REALMENTE têm overlap (tier 1), ordenadas por overlap DESC.
        struct Invisible {
            slot: u8,
            tier1: Vec<usize>,
        }
        let mut invisible: Vec<Invisible> = Vec::new();
        for (h, slot) in cols {
            if *h == 0 {
                continue;
            }
            if row_owner.contains(&Some(*slot)) {
                continue;
            }
            if invisible.iter().any(|iv| iv.slot == *slot) {
                continue; // slot duplicado no mesmo bucket não deveria ocorrer
            }
            let mut cands: Vec<(usize, usize)> = occ_by_row
                .iter()
                .enumerate()
                .filter_map(|(row, occs)| {
                    occs.iter()
                        .find(|(s, _)| s == slot)
                        .map(|(_, occ)| (row, *occ))
                })
                .collect();
            cands.sort_by_key(|(_, occ)| std::cmp::Reverse(*occ)); // overlap DESC
            invisible.push(Invisible {
                slot: *slot,
                tier1: cands.into_iter().map(|(row, _)| row).collect(),
            });
        }
        // Menos opções primeiro; empate mantém ordem de stack (`cols`).
        invisible.sort_by_key(|iv| iv.tier1.len());

        // Tenta mover a linha `row` pro `slot`. Recusa se a linha já é a
        // ÚNICA visível de quem a detém hoje (isso reenterraria essa série —
        // exatamente o bug de "resgate cego" que este pass substitui).
        fn try_reserve(
            row: usize,
            slot: u8,
            row_owner: &mut [Option<u8>],
            visible_count: &mut std::collections::HashMap<u8, usize>,
        ) -> bool {
            let Some(owner) = row_owner[row] else {
                return false;
            };
            if owner == slot {
                return true;
            }
            if visible_count.get(&owner).copied().unwrap_or(0) <= 1 {
                return false;
            }
            if let Some(c) = visible_count.get_mut(&owner) {
                *c -= 1;
            }
            *visible_count.entry(slot).or_insert(0) += 1;
            row_owner[row] = Some(slot);
            true
        }

        for iv in &invisible {
            let mut assigned = false;
            // Tier 1: linha(s) onde a série tem overlap geométrico real.
            for &row in &iv.tier1 {
                if try_reserve(row, iv.slot, &mut row_owner, &mut visible_count) {
                    assigned = true;
                    break;
                }
            }
            // Tier 2 (último recurso, documentado no comentário acima de
            // `cell_grid`): nenhuma linha de overlap real ficou livre — rouba
            // uma linha de quem ainda tem folga (visible_count > 1),
            // preferindo quem tem MAIS folga pra ceder.
            // A cor resgatada aqui não tem fidelidade posicional — pode
            // aparecer fora da ordem real da stack (raro, só em colisões
            // com 3+ séries).
            if !assigned {
                let steal_from = (0..plot_rows)
                    .filter_map(|row| {
                        let owner = row_owner[row]?;
                        let cnt = visible_count.get(&owner).copied().unwrap_or(0);
                        (cnt > 1).then_some((row, cnt))
                    })
                    .max_by_key(|(_, cnt)| *cnt)
                    .map(|(row, _)| row);
                if let Some(row) = steal_from {
                    assigned = try_reserve(row, iv.slot, &mut row_owner, &mut visible_count);
                }
            }
            // Degenerado real (mais séries ativas do que linhas do plot):
            // nenhuma linha assinalável sobrou nem no último recurso — a
            // série fica sem oitavo visível. Documentado, não é bug (mesmo
            // teto do excess-steal acima).
            let _ = assigned;
        }

        for (row, owner) in row_owner.into_iter().enumerate() {
            rows[row].1 = owner;
        }
        cell_grid.push(rows);
    }

    let mut out: Vec<Line<'static>> = Vec::with_capacity(height);

    // Plot, de cima pra baixo.
    for row in (0..plot_rows).rev() {
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

        for grid in &cell_grid {
            // `fill` = quantos oitavos desta célula [lo, lo+8) estão dentro da
            // pilha total. Linhas abaixo do topo da pilha são SEMPRE cheias
            // (█), mesmo que duas séries se encontrem no meio da célula (uma
            // termina, outra começa) — só a linha do TOPO da pilha é parcial.
            // A cor de cada linha já veio decidida em `cell_grid` (maioria,
            // com resgate de visibilidade pra série minoritária que nunca
            // vence votação nenhuma — ver comentário acima de `cell_grid`).
            let (fill, slot_opt) = grid[row];
            let (glyph, style) = if fill == 0 {
                (" ".to_string(), Style::default())
            } else {
                let slot = slot_opt.unwrap_or(0);
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

    // Labels do eixo X: hora (a cada 3 buckets) em resolução horária, ou
    // fronteira de dia local (seg/ter/.../dom) quando bucketed.
    let mut xl = String::with_capacity(width);
    xl.push_str(&" ".repeat(Y_AXIS_W));
    if bucket_hours <= 1 {
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
    } else {
        // Uma coluna ganha label se contém a PRIMEIRA hora de um novo dia
        // local (meia-noite local) — varre as horas originais (não as
        // colunas já agregadas) pra achar a fronteira exata. Guard de
        // colisão/overflow: só escreve o label se a posição de início ainda
        // não foi ultrapassada pelo conteúdo já escrito (evita sobrepor um
        // label no anterior) E se cabe inteiro em `width` — mesmo espírito
        // do guard do ramo horário acima, só que sem bunching: aqui prefere
        // OMITIR o label a desenhá-lo fora de posição.
        let mut prev_date: Option<time::Date> = None;
        let mut last_labeled_col: Option<usize> = None;
        for j in 0..hourly_len {
            let t =
                (now - time::Duration::hours((hourly_len - 1 - j) as i64)).to_offset(local_offset);
            let d = t.date();
            let is_new_day = matches!(prev_date, Some(p) if p != d);
            prev_date = Some(d);
            if !is_new_day {
                continue;
            }
            let col = j / bucket_hours;
            if last_labeled_col == Some(col) {
                continue;
            }
            let lab = weekday_abbrev_pt(d.weekday());
            let pos = Y_AXIS_W + col * (col_w + gap);
            if xl.chars().count() <= pos && pos + lab.chars().count() <= width {
                while xl.chars().count() < pos {
                    xl.push(' ');
                }
                xl.push_str(lab);
                last_labeled_col = Some(col);
            }
        }
    }
    out.push(Line::from(Span::styled(
        xl,
        Style::default().fg(to_ratatui(ColorToken::Comment)),
    )));

    // Legenda. Guard de largura (espelha o guard dos labels X): para de
    // acrescentar séries assim que a próxima entrada não couber em `width`.
    // Séries omitidas → sufixo ` …+N` se ainda couber (trilha B).
    let mut legend: Vec<Span<'static>> = Vec::new();
    let mut legend_w = 0usize;
    let mut shown = 0usize;
    for s in series {
        let marker = "  ● ".to_string();
        let text = format!("{} {}", s.label, fmt_tokens_short(s.total));
        let entry_w = marker.chars().count() + text.chars().count();
        if legend_w + entry_w > width {
            break;
        }
        legend_w += entry_w;
        shown += 1;
        legend.push(Span::styled(
            marker,
            Style::default().fg(to_ratatui(series_token(s.slot))),
        ));
        legend.push(Span::styled(
            text,
            Style::default().fg(to_ratatui(ColorToken::Text)),
        ));
    }
    let omitted = series.len().saturating_sub(shown);
    if omitted > 0 {
        let more = format!(" \u{2026}+{omitted}");
        if legend_w + more.chars().count() <= width {
            legend.push(Span::styled(
                more,
                Style::default().fg(to_ratatui(ColorToken::Comment)),
            ));
        }
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

/// Rótulo de tokens com dual opcional: principal = input+output; sufixo de
/// cache quando `cache > 0`. Charts/gauges de intensidade continuam
/// cache-inclusive no plot — só o texto do rótulo usa este helper
/// (spec confianca + trilha B).
///
/// Exemplos: `9,9M` · `9,9M (+1,4B cache)`.
pub fn fmt_tokens_dual(io: u64, cache: u64) -> String {
    let main = fmt_tokens_short(io);
    if cache == 0 {
        main
    } else {
        format!("{main} (+{} cache)", fmt_tokens_short(cache))
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
    fn fmt_tokens_dual_omits_cache_when_zero() {
        assert_eq!(fmt_tokens_dual(1_200_000, 0), "1,2M");
        assert_eq!(
            fmt_tokens_dual(1_200_000, 1_400_000_000),
            "1,2M (+1,4B cache)"
        );
    }

    #[test]
    fn legend_shows_omitted_count_when_narrow() {
        // Muitas séries com labels longos → só cabem poucas em width=40;
        // o sufixo …+N deve aparecer.
        let s: Vec<_> = (0..6u8)
            .map(|i| series(&format!("ModelNameVeryLong{i}"), i, vec![1000; 24]))
            .collect();
        let lines = column_chart_lines(
            &s,
            40,
            10,
            datetime!(2026-07-10 12:00:00 UTC),
            time::UtcOffset::UTC,
        );
        let legend = plain(&lines).last().cloned().unwrap_or_default();
        assert!(
            legend.contains('\u{2026}') || legend.contains("…"),
            "legenda estreita deveria indicar omissões: {legend:?}"
        );
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
        // Só as linhas do PLOT contam — a legenda SEMPRE tem um ● na cor da
        // série (mesmo se a coluna some), então incluí-la daria falso-positivo.
        // As últimas 3 linhas são eixo X / labels X / legenda.
        let plot_lines = &lines[..lines.len().saturating_sub(3)];
        let series2 = crate::tui::theme_bridge::to_ratatui(crate::theme::series_token(1));
        let has_opus_cell = plot_lines.iter().any(|l| {
            l.spans
                .iter()
                .any(|sp| sp.style.fg == Some(series2) && !sp.content.trim().is_empty())
        });
        assert!(has_opus_cell, "série minúscula não pode sumir do chart");
    }

    #[test]
    fn chart_empty_series_shows_empty_state() {
        let height = 8u16;
        let lines = column_chart_lines(
            &[],
            40,
            height,
            datetime!(2026-07-10 12:00:00 UTC),
            time::UtcOffset::UTC,
        );
        assert_eq!(lines.len(), height as usize);
        let text = plain(&lines);
        let msg_row = text
            .iter()
            .position(|l| l.contains("sem uso"))
            .expect("estado vazio desenhado");
        // Centralizado verticalmente: não pode ficar na primeira linha (era o
        // bug — mensagem no topo + bloco em branco embaixo) e deve ficar
        // perto do meio da altura.
        assert_ne!(msg_row, 0, "mensagem não pode ficar na primeira linha");
        let mid = height as usize / 2;
        assert!(
            msg_row.abs_diff(mid) <= 1,
            "mensagem deveria estar perto do meio (linha {mid}), ficou na linha {msg_row}"
        );
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

        // 4+ séries com largura estreita: sem guard, a legenda sozinha
        // ("  ● label total" × 4) estouraria `width`.
        let many = vec![
            series("Fable 5", 0, vec![100, 200]),
            series("Opus 4.8", 1, vec![50, 60]),
            series("Codex Mini", 2, vec![30, 10]),
            series("Amp Turbo", 3, vec![20, 5]),
        ];
        for w in [30u16, 60, 100] {
            let lines = column_chart_lines(
                &many,
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

    #[test]
    fn chart_three_series_all_visible_in_plot() {
        // A=50, B=50, C=1000 no mesmo bucket: com plot_rows pequeno (altura 6
        // → plot_rows 3, cap 24), A e B ficam com 1 oitavo cada e SÓ tem
        // overlap na row0 (a base da pilha) — resgate ingênuo (last-writer-
        // wins) faz B sobrescrever a cor de A na row0, reenterrando A.
        let s = vec![
            series("A", 0, vec![50]),
            series("B", 1, vec![50]),
            series("C", 2, vec![1000]),
        ];
        let lines = column_chart_lines(
            &s,
            30,
            6,
            datetime!(2026-07-10 12:00:00 UTC),
            time::UtcOffset::UTC,
        );
        let plot = &lines[..lines.len() - 3]; // exclui eixo X, labels e legenda
        for slot in 0..3u8 {
            let color = crate::tui::theme_bridge::to_ratatui(crate::theme::series_token(slot));
            let visible = plot.iter().any(|l| {
                l.spans
                    .iter()
                    .any(|sp| sp.style.fg == Some(color) && !sp.content.trim().is_empty())
            });
            assert!(visible, "série slot {slot} invisível no plot");
        }
    }

    #[test]
    fn chart_bucketed_aggregates_hours_into_columns() {
        // 168 horas sequenciais (valor = índice) agregadas em blocos de 6h:
        // coluna i = soma de tokens[i*6..i*6+6] = 36*i + 15 (soma de 6
        // inteiros consecutivos começando em 6*i).
        let tokens: Vec<u64> = (0..168u64).collect();
        for i in 0..28usize {
            let expected = 36 * i as u64 + 15;
            assert_eq!(
                bucketed_value(&tokens, i, 6),
                expected,
                "coluna {i} deveria somar as 6 horas [{}, {})",
                i * 6,
                i * 6 + 6
            );
        }
        // Grupo parcial: 5 horas com bucket_hours=3 → 2 colunas, última
        // parcial (só 2 horas em vez de 3).
        let partial: Vec<u64> = vec![1, 2, 3, 4, 5];
        assert_eq!(bucketed_value(&partial, 0, 3), 1 + 2 + 3);
        assert_eq!(bucketed_value(&partial, 1, 3), 4 + 5);
        assert_eq!(bucketed_value(&partial, 2, 3), 0); // fora do range

        // Legenda: total continua a soma de TODAS as 168 horas, independente
        // do downsampling em colunas de 6h (contrato do achado: totais da
        // semana não podem ficar presos à janela visível).
        let total: u64 = tokens.iter().sum();
        let s = vec![series("Fable 5", 0, tokens)];
        let lines = column_chart_lines_bucketed(
            &s,
            100,
            12,
            datetime!(2026-07-10 12:00:00 UTC),
            time::UtcOffset::UTC,
            6,
        );
        let text = plain(&lines);
        let legend = text.last().unwrap();
        assert!(
            legend.contains(&fmt_tokens_short(total)),
            "legenda deveria mostrar o total das 168h ({}): {legend}",
            fmt_tokens_short(total)
        );
    }

    #[test]
    fn chart_bucketed_shows_day_labels_and_fits_width() {
        let s = vec![series(
            "Fable 5",
            0,
            (0..168u64).map(|h| (h % 5) * 1000).collect(),
        )];
        for w in [80u16, 100, 160] {
            let lines = column_chart_lines_bucketed(
                &s,
                w,
                12,
                datetime!(2026-07-10 12:00:00 UTC),
                time::UtcOffset::UTC,
                6,
            );
            let text = plain(&lines);
            for l in &text {
                assert!(l.chars().count() <= w as usize, "linha estourou {w}: {l:?}");
            }
            // Linhas, de baixo pra cima: legenda, labels X, eixo X, plot...
            let xl = &text[text.len() - 2];
            let has_weekday = ["seg", "ter", "qua", "qui", "sex", "sáb", "dom"]
                .iter()
                .any(|d| xl.contains(d));
            assert!(has_weekday, "largura {w}: eixo X sem label de dia: {xl:?}");
        }
    }
}
