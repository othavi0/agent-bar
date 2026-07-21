//! Tela Detalhe: dados reais por provider (Task 12, redesenhada na Task 9
//! do plano v8). `render_full` orquestra 5 seções empilhadas por
//! `Layout::vertical`, todas alinhadas na mesma coluna de gauge: janelas
//! (sessão/semana/modelos), chart de tokens/hora por modelo (`Min(9)` —
//! absorve a altura extra, substitui a antiga sparkline de 1 linha que
//! deixava ~20 linhas em branco), modelos hoje (tokens+custo), extra usage
//! (spend novo do Claude) e totais (hoje/7 dias). Chips de ação ficam fora
//! de `render_full` (`render_detail` já reserva a última linha antes de
//! chamá-la). Quando a área é curta demais pro conteúdo + chart mínimo, o
//! colapso é progressivo: EXTRA USAGE some primeiro, depois MODELOS HOJE
//! vira 1 linha-resumo.

mod chart;
mod extra;
mod format;
mod models;
mod states;
mod totals;
mod windows;

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph};
use ratatui::Frame;
use throbber_widgets_tui::{Throbber, ThrobberState, BRAILLE_SIX};

use crate::providers::types::ProviderQuota;
use crate::theme::ColorToken;
use crate::tui::login_state::{login_state_for, LoginState};
use crate::tui::mouse::{ChipKind, HitMap};
use crate::tui::state::AppState;
use crate::tui::theme_bridge::{provider_color, to_ratatui};
use crate::tui::widgets::chips::{chips_line, register_chip_hits};
use crate::tui::widgets::quota_gauge::gauge_spans;
use crate::usage::ProviderUsage;

use chart::render_chart_section;
use extra::extra_lines;
use format::{derive_bar_width, truncate_name, LABEL_W, WINDOW_SUFFIX_W};
use models::model_lines;
use states::{render_error, render_logged_out};
use totals::totals_line;
use windows::window_lines;

// ---------------------------------------------------------------------------
// Render principal
// ---------------------------------------------------------------------------

/// Corpo completo (estado Ok/Checking): orquestra as seções em
/// `Layout::vertical` (Task 9 §2) — JANELAS / GRÁFICO (`Min(9)`, absorve a
/// altura extra) / MODELOS HOJE / EXTRA USAGE / TOTAIS. Quando a área não
/// cabe tudo + o chart mínimo, colapsa progressivamente: EXTRA USAGE some
/// primeiro, depois MODELOS HOJE vira 1 linha-resumo. Chips ficam fora
/// (`render_detail` já reserva a última linha antes de chamar esta fn).
fn render_full(
    state: &AppState,
    q: &ProviderQuota,
    brand: Color,
    provider_usage: Option<&ProviderUsage>,
    frame: &mut Frame,
    area: Rect,
) {
    let windows = window_lines(q, provider_usage, area.width);
    let (models_full, models_collapsed) = model_lines(provider_usage, brand, area.width);
    let extra_full = extra_lines(q, area.width);
    let totals = totals_line(state, provider_usage, &q.provider);

    // Progressive disclosure (trilha C): em painel estreito (~80 cols de
    // terminal com sidebar) o chart cede altura; em largo mantém Min(9).
    let chart_min: u16 = if area.width < 72 { 6 } else { 9 };
    const TOTALS_LEN: u16 = 1;
    let windows_len = windows.len() as u16;
    let models_full_len = models_full.len() as u16;
    let extra_full_len = extra_full.len() as u16;

    // Colapso progressivo (spec §2): tenta tudo, depois sem EXTRA USAGE,
    // depois com MODELOS HOJE também colapsado pra 1 linha-resumo.
    let with_extra = windows_len + models_full_len + extra_full_len + TOTALS_LEN + chart_min;
    let without_extra = windows_len + models_full_len + TOTALS_LEN + chart_min;
    let (extra, models) = if area.height >= with_extra {
        (extra_full, models_full)
    } else if area.height >= without_extra {
        (Vec::new(), models_full)
    } else {
        (Vec::new(), models_collapsed)
    };

    let chunks = Layout::vertical([
        Constraint::Length(windows_len),
        Constraint::Min(chart_min),
        Constraint::Length(models.len() as u16),
        Constraint::Length(extra.len() as u16),
        Constraint::Length(TOTALS_LEN),
    ])
    .split(area);

    frame.render_widget(Paragraph::new(windows), chunks[0]);
    render_chart_section(state, frame, chunks[1], q);
    frame.render_widget(Paragraph::new(models), chunks[2]);
    if !extra.is_empty() {
        frame.render_widget(Paragraph::new(extra), chunks[3]);
    }
    frame.render_widget(Paragraph::new(vec![totals]), chunks[4]);
}

/// Chips centrados: `[esc voltar] [r atualizar] [g login] [h histórico]`.
/// As 4 teclas já são globais (`update.rs`) — os chips só tornam a ação
/// clicável/visível, não introduzem comportamento novo.
fn render_footer_chips(frame: &mut Frame, area: Rect, hits: &mut HitMap) {
    // "?" no rodapé do Detail: discoverability do help (trilha C).
    let chips: [(ChipKind, &str, &str); 5] = [
        (ChipKind::Back, "esc", "voltar"),
        (ChipKind::Refresh, "r", "atualizar"),
        (ChipKind::Login, "g", "login"),
        (ChipKind::History, "h", "hist\u{f3}rico"),
        (ChipKind::Help, "?", "ajuda"),
    ];
    let line = chips_line(&chips, area.width);
    frame.render_widget(Paragraph::new(line), area);
    register_chip_hits(&chips, area, hits);
}

/// Renders the Detail view for the selected provider (Screen::Detail).
/// `Detail` é a tela default do boot (Task 11: Overview morreu) — enquanto
/// `state.providers` está vazio, NUNCA uma tela em branco: fetch em voo
/// (`fetch_pending`) ou foco pendente (`pending_focus`) → skeleton com
/// throbber; genuinamente sem providers habilitados → mensagem instrutiva.
pub fn render_detail(state: &AppState, frame: &mut Frame, area: Rect, hits: &mut HitMap) {
    let provider = match state.providers.get(state.selected) {
        Some(pv) => pv,
        None => {
            return if state.fetch_pending.is_empty() && state.pending_focus.is_none() {
                render_empty(frame, area, hits)
            } else {
                render_skeleton(state, frame, area, hits)
            };
        }
    };
    let q = &provider.quota;
    let p_color = provider_color(&q.provider);
    let fetch_pending = state.fetch_pending.iter().any(|p| p == &q.provider);
    let logged = login_state_for(Some(q), fetch_pending);

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

    // [conteúdo | chips] — chips sempre presentes, em qualquer estado.
    let vert = Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).split(inner);
    let content_area = vert[0];
    let footer_area = vert[1];

    match logged {
        // Deslogado: CTA em tela cheia (nunca a lista de seções vazia).
        LoginState::LoggedOut => render_logged_out(q, frame, content_area),
        // Falha não-auth (parse/rede/API): mensagem tipada + ícone — nunca
        // branco, nunca induz re-login (spec §10).
        LoginState::NoToken | LoginState::Error => {
            render_error(q, state.glyph_mode, frame, content_area)
        }
        LoginState::Ok | LoginState::Checking => {
            let provider_usage: Option<&ProviderUsage> = state
                .usage
                .as_ref()
                .and_then(|s| s.providers.iter().find(|pu| pu.provider == q.provider));
            render_full(state, q, p_color, provider_usage, frame, content_area);
        }
    }

    render_footer_chips(frame, footer_area, hits);
}

/// Fallback quando não há NENHUM provider habilitado e nenhum fetch em voo
/// (`settings.waybar.providers` vazio — caso raro; distinto de
/// `LoginState::LoggedOut`, que é POR provider). Chips continuam ativos
/// (ex. `[h]`/`[g]` ainda abrem Histórico/Login) — nunca uma tela sem
/// nenhuma ação possível.
fn render_empty(frame: &mut Frame, area: Rect, hits: &mut HitMap) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Thick)
        .border_style(Style::default().fg(to_ratatui(ColorToken::Comment)));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let vert = Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).split(inner);

    let para = Paragraph::new(Span::styled(
        " nenhum provider habilitado \u{2014} veja a tela Config",
        Style::default().fg(to_ratatui(ColorToken::Muted)),
    ));
    frame.render_widget(para, vert[0]);

    render_footer_chips(frame, vert[1], hits);
}

/// Skeleton do boot (Task 11): `state.providers` ainda vazio mas há fetch em
/// voo (`fetch_pending`) ou foco pendente (`pending_focus`) aguardando
/// resolução — NUNCA tela vazia enquanto isso. Título usa o nome real do
/// provider de `pending_focus` quando conhecido (via
/// `providers::get_provider`), senão "carregando…" genérico. Throbber
/// (Animação C) no canto — mesmo padrão do antigo skeleton do Overview
/// (`dashboard::render_skeleton_card`, órfão desta task, apagado).
fn render_skeleton(state: &AppState, frame: &mut Frame, area: Rect, hits: &mut HitMap) {
    let title = match state
        .pending_focus
        .as_deref()
        .and_then(crate::providers::get_provider)
    {
        Some(p) => format!(" {} \u{b7} carregando\u{2026} ", p.name()),
        None => " carregando\u{2026} ".to_string(),
    };

    let mut block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Thick)
        .border_style(Style::default().fg(to_ratatui(ColorToken::Comment)))
        .title(Span::styled(
            title,
            Style::default()
                .fg(to_ratatui(ColorToken::Muted))
                .add_modifier(Modifier::BOLD),
        ));

    let throbber_widget = Throbber::default()
        .throbber_set(BRAILLE_SIX)
        .throbber_style(Style::default().fg(to_ratatui(ColorToken::Cyan)))
        .use_type(throbber_widgets_tui::WhichUse::Spin);
    let mut throbber_state = ThrobberState::default();
    for _ in 0..state.throbber.index {
        throbber_state.calc_next();
    }
    block =
        block.title(Line::from(throbber_widget.to_symbol_span(&throbber_state)).right_aligned());

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let vert = Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).split(inner);
    let content_area = vert[0];
    let footer_area = vert[1];

    let gauge_w = derive_bar_width(content_area.width, WINDOW_SUFFIX_W);
    let lines = vec![
        skeleton_gauge_line("sess\u{e3}o", gauge_w),
        skeleton_gauge_line("semana", gauge_w),
    ];
    frame.render_widget(Paragraph::new(lines), content_area);

    render_footer_chips(frame, footer_area, hits);
}

/// Uma linha de gauge vazio (trilho ░, sem % nem reset — não há dado ainda)
/// do skeleton do boot. Mesmo `LABEL_W` das demais seções (coluna alinhada).
fn skeleton_gauge_line(label: &str, gauge_w: usize) -> Line<'static> {
    let name = truncate_name(label, LABEL_W);
    let mut spans = vec![Span::styled(
        format!(" {name:<LABEL_W$} "),
        Style::default().fg(to_ratatui(ColorToken::Muted)),
    )];
    spans.extend(gauge_spans(0.0, gauge_w, to_ratatui(ColorToken::Comment)));
    Line::from(spans)
}

#[cfg(test)]
mod tests {
    use indexmap::IndexMap;

    use crate::providers::types::{
        AmpQuotaExtra, ClaudeQuotaExtra, ExtraUsage, ProviderExtra, ProviderQuota, QuotaWindow,
    };
    use crate::tui::mouse::HitMap;
    use crate::tui::render::render;
    use crate::tui::state::{AppState, FetchStatus, ProviderView, Screen};
    use crate::usage::amp::AmpDollars;
    use crate::usage::{Cost, ModelUsage, ProviderUsage, UsageRecord, UsageSummary};

    use super::truncate_name;

    fn window(remaining: f64, resets_at: Option<&str>, severity: Option<&str>) -> QuotaWindow {
        QuotaWindow {
            remaining,
            resets_at: resets_at.map(|s| s.to_string()),
            window_minutes: Some(300),
            used: Some(100.0 - remaining),
            severity: severity.map(|s| s.to_string()),
        }
    }

    fn rec(provider: &str, model: &str, ts: time::OffsetDateTime, tokens: u64) -> UsageRecord {
        UsageRecord {
            provider: provider.into(),
            model: Some(model.into()),
            input: tokens,
            output: 0,
            cache_read: 0,
            cache_write: 0,
            cache_write_1h: 0,
            fast: false,
            geo_us: false,
            ts,
            session_id: None,
            project: None,
        }
    }

    // -----------------------------------------------------------------
    // Unit tests: truncate_name
    // -----------------------------------------------------------------

    #[test]
    fn truncate_name_keeps_short_names_intact() {
        // Regressão do bug "Free Tie": "Free Tier" (9 chars) cabe em 12 —
        // NUNCA deve ser cortado.
        assert_eq!(truncate_name("Free Tier", 12), "Free Tier");
        assert_eq!(truncate_name("Fable", 12), "Fable");
    }

    #[test]
    fn truncate_name_ellipsizes_long_names() {
        let out = truncate_name("claude-opus-4-8-extended", 12);
        assert_eq!(out.chars().count(), 12);
        assert!(out.ends_with('\u{2026}'));
    }

    /// `ProviderQuota` mínima só pra exercitar `render_chart_section`
    /// isoladamente (sem janelas/modelos/extra — não é o que está sob teste
    /// aqui).
    fn minimal_quota(provider: &str) -> ProviderQuota {
        ProviderQuota {
            provider: provider.to_string(),
            display_name: provider.to_string(),
            available: true,
            account: None,
            plan: None,
            plan_type: None,
            primary: None,
            secondary: None,
            models: None,
            extra: None,
            error: None,
            stale_reason: None,
        }
    }

    // -----------------------------------------------------------------
    // Unit tests: render_chart_section usa state.local_offset (regressão
    // T12, antes coberta por `spark_line` — morta na Task 9). A conversão
    // de fuso em si mora em `column_chart_lines` (T8); isto verifica que
    // `render_chart_section` de fato REPASSA `state.local_offset`, e não um
    // `UtcOffset::UTC` hardcoded ou similar.
    // -----------------------------------------------------------------

    #[test]
    fn chart_section_uses_local_offset_for_hour_labels() {
        let backend = ratatui::backend::TestBackend::new(60, 12);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        let mut state = AppState::new();
        let now = time::macros::datetime!(2026-07-02 02:00:00 UTC);
        state.last_update = Some(now);
        state.local_offset = time::UtcOffset::from_hms(-3, 0, 0).unwrap();
        state.history = Some(vec![rec(
            "claude",
            "claude-opus-4-8",
            now - time::Duration::hours(1),
            900_000,
        )]);
        let q = minimal_quota("claude");
        terminal
            .draw(|f| super::render_chart_section(&state, f, f.area(), &q))
            .unwrap();
        let text = buffer_to_string(terminal.backend().buffer());
        // Mesma fórmula de rótulo do eixo X que `column_chart_lines` usa
        // (T8) — se `render_chart_section` passasse UTC em vez de
        // `state.local_offset`, os rótulos calculados aqui (com -03:00)
        // divergiriam dos realmente desenhados.
        for i in (0..24usize).step_by(3) {
            let bucket_time = now - time::Duration::hours((24 - 1 - i) as i64);
            let h = bucket_time.to_offset(state.local_offset).hour();
            let label = format!("{h:02}h");
            assert!(
                text.contains(&label),
                "r\u{f3}tulo local esperado {label:?} ausente:\n{text}"
            );
        }
    }

    // -----------------------------------------------------------------
    // Loading vs zero (regressão "hoje 0 tok"): enquanto o parse dos
    // session logs não terminou (`state.usage`/`state.history` = None),
    // as linhas de uso devem dizer "coletando", nunca afirmar zero.
    // -----------------------------------------------------------------

    #[test]
    fn chart_section_history_none_shows_coletando_not_sem_uso() {
        let backend = ratatui::backend::TestBackend::new(40, 10);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        let mut state = AppState::new();
        state.last_update = Some(time::macros::datetime!(2026-07-02 12:00:00 UTC));
        state.history = None; // HistoryLoaded ainda não chegou
        let q = minimal_quota("claude");
        terminal
            .draw(|f| super::render_chart_section(&state, f, f.area(), &q))
            .unwrap();
        let text = buffer_to_string(terminal.backend().buffer());
        assert!(
            text.contains("coletando"),
            "history=None deve mostrar loading, obtido:\n{text}"
        );
        assert!(
            !text.contains("sem uso"),
            "history=None não pode afirmar 'sem uso':\n{text}"
        );
    }

    #[test]
    fn totals_line_loading_shows_coletando_not_zero() {
        let state = AppState::new(); // usage=None, history=None
        let line = super::totals_line(&state, None, "claude");
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(
            text.contains("coletando"),
            "usage/history=None deve mostrar loading, obtido: {text:?}"
        );
        assert!(
            !text.contains("0 tok"),
            "usage/history=None não pode afirmar '0 tok': {text:?}"
        );
    }

    // -----------------------------------------------------------------
    // Fixtures
    // -----------------------------------------------------------------

    fn make_claude_full() -> ProviderView {
        let mut models: IndexMap<String, QuotaWindow> = IndexMap::new();
        // "Opus" bate por substring com "claude-opus-4-8" (by_model) —
        // exercita o custo opcional anexado em `model_window_line`.
        models.insert("Opus".to_string(), window(60.0, None, Some("normal")));
        models.insert(
            "Fable".to_string(),
            window(97.0, Some("2026-07-03T22:59:59Z"), Some("normal")),
        );

        ProviderView::new(ProviderQuota {
            provider: "claude".to_string(),
            display_name: "Claude".to_string(),
            available: true,
            account: None,
            plan: Some("Max 5x".to_string()),
            plan_type: None,
            primary: Some(window(89.0, Some("2026-07-02T02:39:59Z"), Some("normal"))),
            secondary: Some(window(97.0, Some("2026-07-03T22:59:59Z"), Some("normal"))),
            models: Some(models),
            extra: Some(ProviderExtra::Claude(ClaudeQuotaExtra {
                weekly_models: None,
                extra_usage: Some(ExtraUsage {
                    enabled: true,
                    remaining: 87.66,
                    limit: 100.0,
                    used: 12.34,
                }),
            })),
            error: None,
            stale_reason: None,
        })
    }

    fn make_amp_full() -> ProviderView {
        let mut models: IndexMap<String, QuotaWindow> = IndexMap::new();
        models.insert("Free Tier".to_string(), window(70.0, None, None));
        models.insert("Credits".to_string(), window(100.0, None, None));

        ProviderView::new(ProviderQuota {
            provider: "amp".to_string(),
            display_name: "Amp".to_string(),
            available: true,
            account: Some("user@x.com".to_string()),
            plan: None,
            plan_type: None,
            primary: Some(window(70.0, None, None)),
            secondary: None,
            models: Some(models),
            extra: Some(ProviderExtra::Amp(AmpQuotaExtra { meta: None })),
            error: None,
            stale_reason: None,
        })
    }

    fn make_codex_logged_out() -> ProviderView {
        ProviderView::new(ProviderQuota {
            provider: "codex".to_string(),
            display_name: "Codex".to_string(),
            available: false,
            account: None,
            plan: None,
            plan_type: None,
            primary: None,
            secondary: None,
            models: None,
            extra: None,
            error: Some(
                "Not logged in. Open `agent-bar menu` and choose Provider login.".to_string(),
            ),
            stale_reason: None,
        })
    }

    /// UsageSummary com claude (2 modelos, 1 com custo conhecido) + amp
    /// (crédito, sem tokens).
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

    /// Fixture completa de `AppState` pro Detail do Claude com 2 modelos
    /// (Task 9): reaproveitada pelo snapshot pré-existente `detail_claude_full`
    /// e pelos 2 testes novos de layout (`render_detail` chamado direto).
    fn fixture_claude_full() -> AppState {
        let mut state = AppState::new();
        let now = time::macros::datetime!(2026-07-02 02:00:00 UTC);
        state.providers = vec![make_claude_full()];
        state.selected = 0;
        state.screen = Screen::Detail;
        state.status = FetchStatus::Loaded;
        // display_cost (T16): header agora mostra o count-up, não
        // usage.total_cost.usd direto — sem isto, o header ficaria em
        // "$0.00" (default de AppState::new()) em vez do custo real.
        let usage = fake_usage();
        state.display_cost = usage.total_cost.usd;
        state.usage = Some(usage);
        state.last_update = Some(now);
        state.history = Some(vec![
            rec(
                "claude",
                "claude-opus-4-8",
                now - time::Duration::hours(1),
                900_000,
            ),
            rec(
                "claude",
                "claude-opus-4-8",
                now - time::Duration::hours(5),
                100_000,
            ),
            rec(
                "claude",
                "claude-sonnet-4-6",
                now - time::Duration::hours(9),
                50_000,
            ),
            rec("codex", "gpt-5.5", now - time::Duration::hours(2), 700_000),
        ]);
        state
    }

    /// Mantém snapshots em `src/tui/render/snapshots/` (mesmo path de quando
    /// o módulo era `detail.rs`) — split para `detail/` não deve mover nem
    /// reescrever os arquivos `.snap`.
    fn with_render_snapshots<F: FnOnce()>(f: F) {
        insta::with_settings!({ snapshot_path => "../snapshots" }, {
            f();
        });
    }

    /// Achata um `Buffer` renderizado em texto puro, uma linha por row
    /// (trailing spaces cortados) — usado pelos asserts de conteúdo e pelo
    /// snapshot textual dos testes de layout (Task 9).
    fn buffer_to_string(buf: &ratatui::buffer::Buffer) -> String {
        (0..buf.area.height)
            .map(|y| {
                (0..buf.area.width)
                    .map(|x| {
                        buf.cell((x, y))
                            .map(|c| c.symbol())
                            .unwrap_or(" ")
                            .to_string()
                    })
                    .collect::<String>()
                    .trim_end()
                    .to_string()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Maior sequência de linhas "em branco" (só espaço/borda vertical)
    /// entre o título e os chips — Task 9: antes do chart absorver a
    /// altura extra, sobravam ~20 linhas assim (bug que este helper
    /// existe pra pegar). Ignora a 1ª/última linha (bordas horizontais).
    fn max_blank_run(buf: &ratatui::buffer::Buffer) -> usize {
        let s = buffer_to_string(buf);
        let mut max = 0;
        let mut cur = 0;
        let lines: Vec<&str> = s.lines().collect();
        for l in &lines[1..lines.len().saturating_sub(1)] {
            if l.trim_matches(|c: char| c == ' ' || c == '\u{2503}' || c == '\u{2502}')
                .is_empty()
            {
                cur += 1;
                max = max.max(cur);
            } else {
                cur = 0;
            }
        }
        max
    }

    // -----------------------------------------------------------------
    // Snapshots (brief Step 2)
    // -----------------------------------------------------------------

    #[test]
    fn detail_claude_full() {
        let backend = ratatui::backend::TestBackend::new(100, 32);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        let state = fixture_claude_full();
        terminal
            .draw(|f| render(&state, f, &mut HitMap::default()))
            .unwrap();
        with_render_snapshots(|| insta::assert_snapshot!(terminal.backend()));
    }

    // -----------------------------------------------------------------
    // Task 9: layout por seção — chart absorve a altura extra (sem gap em
    // branco) e "Modelos hoje" mostra nomes TRATADOS (nunca o id raw).
    // -----------------------------------------------------------------

    #[test]
    fn detail_chart_absorbs_extra_height_no_blank_gap() {
        // 100x40: antes do v8 sobravam ~20 linhas em branco. Agora o chart
        // estica (`Min(9)` no orquestrador) pra absorver o espaço livre.
        let state = fixture_claude_full();
        let mut term = ratatui::Terminal::new(ratatui::backend::TestBackend::new(100, 40)).unwrap();
        term.draw(|f| {
            let mut hits = HitMap::default();
            super::render_detail(&state, f, f.area(), &mut hits);
        })
        .unwrap();
        let buf = term.backend().buffer().clone();
        let blank_run = max_blank_run(&buf);
        assert!(
            blank_run < 5,
            "gap de {blank_run} linhas em branco — chart deveria absorver"
        );
        with_render_snapshots(|| insta::assert_snapshot!(buffer_to_string(&buf)));
    }

    #[test]
    fn detail_models_today_shows_treated_names_and_cost() {
        let state = fixture_claude_full();
        let mut term = ratatui::Terminal::new(ratatui::backend::TestBackend::new(100, 32)).unwrap();
        term.draw(|f| {
            let mut hits = HitMap::default();
            super::render_detail(&state, f, f.area(), &mut hits);
        })
        .unwrap();
        let text = buffer_to_string(term.backend().buffer());
        assert!(text.contains("Opus 4.8"), "nome tratado ausente:\n{text}");
        assert!(!text.contains("claude-opus"), "id raw vazou:\n{text}");
    }

    #[test]
    fn detail_collapse_short_terminal() {
        // 100x20: overhead do outer block de `render()` (2) + block do
        // Detail (2) + linha de chips (1) = 5 linhas, então `content_area`
        // fica com 15 — abaixo de `without_extra` (18, ver `render_full`) —
        // colapso completo: EXTRA USAGE some e MODELOS HOJE vira 1
        // linha-resumo. Altura escolhida pra deixar o chart EXATAMENTE no
        // `CHART_MIN` (9), provando que o colapso protege o mínimo do
        // chart mesmo no pior caso.
        let backend = ratatui::backend::TestBackend::new(100, 20);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        let state = fixture_claude_full();
        terminal
            .draw(|f| render(&state, f, &mut HitMap::default()))
            .unwrap();
        let buf = terminal.backend().buffer().clone();
        let text = buffer_to_string(&buf);
        assert!(
            !text.contains("EXTRA USAGE"),
            "extra usage deveria colapsar (sumir) no terminal curto:\n{text}"
        );
        assert!(
            !text.contains(" MODELOS HOJE"),
            "t\u{ed}tulo completo de modelos hoje n\u{e3}o deveria aparecer colapsado:\n{text}"
        );
        assert!(
            text.contains("modelos hoje \u{b7}"),
            "linha-resumo de modelos hoje ausente:\n{text}"
        );
        assert!(
            text.contains("TOKENS/HORA"),
            "chart deveria continuar vis\u{ed}vel mesmo colapsado:\n{text}"
        );
        with_render_snapshots(|| insta::assert_snapshot!(text));
    }

    #[test]
    fn detail_amp_credits() {
        let backend = ratatui::backend::TestBackend::new(100, 32);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        let mut state = AppState::new();
        let now = time::macros::datetime!(2026-07-02 02:00:00 UTC);
        state.providers = vec![make_amp_full()];
        state.selected = 0;
        state.screen = Screen::Detail;
        state.status = FetchStatus::Loaded;
        // display_cost (T16): header agora mostra o count-up, não
        // usage.total_cost.usd direto — sem isto, o header ficaria em
        // "$0.00" (default de AppState::new()) em vez do custo real.
        let usage = fake_usage();
        state.display_cost = usage.total_cost.usd;
        state.usage = Some(usage);
        state.last_update = Some(now);
        // Amp nunca gera UsageRecord (sem tracking de token) — history só
        // tem claude, então o sparkline do Amp deve cair no placeholder
        // "sem uso", DIFERENTE do sparkline real do Claude.
        state.history = Some(vec![rec(
            "claude",
            "claude-opus-4-8",
            now - time::Duration::hours(1),
            900_000,
        )]);
        terminal
            .draw(|f| render(&state, f, &mut HitMap::default()))
            .unwrap();
        with_render_snapshots(|| insta::assert_snapshot!(terminal.backend()));
    }

    #[test]
    fn detail_codex_logged_out() {
        let backend = ratatui::backend::TestBackend::new(100, 32);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        let mut state = AppState::new();
        state.providers = vec![make_codex_logged_out()];
        state.selected = 0;
        state.screen = Screen::Detail;
        state.status = FetchStatus::Loaded;
        terminal
            .draw(|f| render(&state, f, &mut HitMap::default()))
            .unwrap();
        with_render_snapshots(|| insta::assert_snapshot!(terminal.backend()));
    }

    #[test]
    fn detail_narrow_80() {
        // Largura 80 == NARROW_WIDTH: sidebar ainda expandida (colapsa só
        // abaixo de 80), então o conteúdo do Detail fica bem apertado —
        // verifica que gauges/sparkline/totais não colidem nem estouram.
        let backend = ratatui::backend::TestBackend::new(80, 32);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        let mut state = AppState::new();
        let now = time::macros::datetime!(2026-07-02 02:00:00 UTC);
        state.providers = vec![make_claude_full()];
        state.selected = 0;
        state.screen = Screen::Detail;
        state.status = FetchStatus::Loaded;
        // display_cost (T16): header agora mostra o count-up, não
        // usage.total_cost.usd direto — sem isto, o header ficaria em
        // "$0.00" (default de AppState::new()) em vez do custo real.
        let usage = fake_usage();
        state.display_cost = usage.total_cost.usd;
        state.usage = Some(usage);
        state.last_update = Some(now);
        state.history = Some(vec![rec(
            "claude",
            "claude-opus-4-8",
            now - time::Duration::hours(3),
            900_000,
        )]);
        terminal
            .draw(|f| render(&state, f, &mut HitMap::default()))
            .unwrap();
        with_render_snapshots(|| insta::assert_snapshot!(terminal.backend()));
    }

    // -----------------------------------------------------------------
    // Cobertura extra: extra usage desativado + estado de erro tipado
    // -----------------------------------------------------------------

    #[test]
    fn detail_extra_usage_disabled() {
        let mut pv = make_claude_full();
        pv.quota.extra = Some(ProviderExtra::Claude(ClaudeQuotaExtra {
            weekly_models: None,
            extra_usage: Some(ExtraUsage {
                enabled: false,
                remaining: 0.0,
                limit: 0.0,
                used: 0.0,
            }),
        }));

        let backend = ratatui::backend::TestBackend::new(100, 32);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        let mut state = AppState::new();
        state.providers = vec![pv];
        state.selected = 0;
        state.screen = Screen::Detail;
        state.status = FetchStatus::Loaded;
        terminal
            .draw(|f| render(&state, f, &mut HitMap::default()))
            .unwrap();
        with_render_snapshots(|| insta::assert_snapshot!(terminal.backend()));
    }

    #[test]
    fn detail_extra_usage_no_limit() {
        // Sentinel: enabled:true + limit<=0.0 ("sem teto configurado" — ver
        // extra_usage_from_spend em claude.rs). Sem gauge, só o valor gasto.
        let mut pv = make_claude_full();
        pv.quota.extra = Some(ProviderExtra::Claude(ClaudeQuotaExtra {
            weekly_models: None,
            extra_usage: Some(ExtraUsage {
                enabled: true,
                remaining: 0.0,
                limit: 0.0,
                used: 12.34,
            }),
        }));

        let backend = ratatui::backend::TestBackend::new(100, 32);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        let mut state = AppState::new();
        state.providers = vec![pv];
        state.selected = 0;
        state.screen = Screen::Detail;
        state.status = FetchStatus::Loaded;
        terminal
            .draw(|f| render(&state, f, &mut HitMap::default()))
            .unwrap();
        with_render_snapshots(|| insta::assert_snapshot!(terminal.backend()));
    }

    #[test]
    fn detail_provider_error_shows_icon_and_message() {
        let pv = ProviderView::new(ProviderQuota {
            provider: "codex".to_string(),
            display_name: "Codex".to_string(),
            available: false,
            account: None,
            plan: None,
            plan_type: None,
            primary: None,
            secondary: None,
            models: None,
            extra: None,
            error: Some("Failed to fetch Codex usage".to_string()),
            stale_reason: None,
        });

        let backend = ratatui::backend::TestBackend::new(100, 32);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        let mut state = AppState::new();
        state.providers = vec![pv];
        state.selected = 0;
        state.screen = Screen::Detail;
        state.status = FetchStatus::Loaded;
        terminal
            .draw(|f| render(&state, f, &mut HitMap::default()))
            .unwrap();
        with_render_snapshots(|| insta::assert_snapshot!(terminal.backend()));
    }

    // Pulso crítico (Task 16) removido em v8 (spec §6): `window_line` usa a
    // cor de severidade direto, sem modulação de brilho — os testes que
    // confirmavam o pulso saíram junto.
}
