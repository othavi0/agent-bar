pub mod config;
pub mod dashboard;
pub mod detail;
pub mod history;
pub mod login;
mod shared;
pub mod sidebar;

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;
use throbber_widgets_tui::{Throbber, ThrobberState, BRAILLE_SIX};

use crate::theme::ColorToken;
use crate::tui::mouse::HitMap;
use crate::tui::state::{AppState, Screen};
use crate::tui::theme_bridge::to_ratatui;

use self::config::render_config;
use self::dashboard::render_dashboard;
use self::detail::render_detail;
use self::history::render_history;
use self::login::render_login;
use self::sidebar::render_sidebar;

/// Largura abaixo da qual a sidebar colapsa pra so a coluna de marcas.
const NARROW_WIDTH: u16 = 80;

/// Largura da coluna "tecla" na tabela de atalhos — fixa pra alinhar a
/// coluna "ação" em todas as seções (contrato de tabela de 2 colunas, T14).
const HELP_KEY_COL: usize = 12;

/// Uma seção de atalhos: título centrado + linhas tecla/ação alinhadas em 2
/// colunas. Reutilizado por `help_text` pra cada tela (Navegação global,
/// Overview, Waybar Config, Login) — data-driven em vez de repetir a mesma
/// construção de `Line` 4x.
fn help_section(title: &str, rows: &[(&str, &str)]) -> Vec<Line<'static>> {
    let mut lines = vec![Line::from(Span::styled(
        format!(" {title} "),
        Style::default()
            .fg(to_ratatui(ColorToken::TextBright))
            .add_modifier(Modifier::BOLD),
    ))
    .centered()];
    for (key, action) in rows {
        lines.push(Line::from(vec![
            Span::styled(
                format!("  {key:<HELP_KEY_COL$}"),
                Style::default()
                    .fg(to_ratatui(ColorToken::Cyan))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                action.to_string(),
                Style::default().fg(to_ratatui(ColorToken::Text)),
            ),
        ]));
    }
    lines
}

/// Constroi o conteudo do overlay de ajuda: tabela de 2 colunas
/// (tecla/ação) por tela + dica de mouse no rodapé (T14).
fn help_text() -> Text<'static> {
    let mut lines: Vec<Line<'static>> = Vec::new();

    lines.extend(help_section(
        "Navegação global",
        &[
            ("[?] / Esc", "abre/fecha esta ajuda"),
            ("up/down", "mover seleção na sidebar"),
            ("Enter", "ativar item selecionado"),
            ("h / g / w", "Histórico / Login / Waybar"),
            ("q", "sair"),
            ("r", "atualizar quotas"),
        ],
    ));
    lines.push(Line::from(""));

    lines.extend(help_section(
        "Overview",
        &[
            ("up/down", "selecionar provider"),
            ("Enter", "abrir detalhe"),
            ("Esc", "voltar para lista"),
        ],
    ));
    lines.push(Line::from(""));

    lines.extend(help_section(
        "Waybar Config",
        &[
            ("up/down", "selecionar campo"),
            ("Enter", "editar campo"),
            ("s", "salvar configuração"),
            ("Esc", "voltar"),
        ],
    ));
    lines.push(Line::from(""));

    lines.extend(help_section(
        "Login",
        &[
            ("up/down", "selecionar provider"),
            ("Enter", "iniciar login do provider"),
            ("Esc", "voltar"),
        ],
    ));
    lines.push(Line::from(""));

    lines.extend(help_section(
        "Histórico",
        &[("t", "alterna 24h/7d \u{b7} wheel rola a tabela")],
    ));
    lines.push(Line::from(""));

    lines.push(
        Line::from(Span::styled(
            "click seleciona \u{b7} wheel rola \u{b7} shift+drag seleciona texto",
            Style::default().fg(to_ratatui(ColorToken::Muted)),
        ))
        .centered(),
    );

    Text::from(lines)
}

/// Retorna um `Rect` centralizado em `r`, ocupando `percent_x`% de largura e
/// `percent_y`% de altura. Usado pelo overlay de ajuda pra fixar o tamanho
/// do popup INDEPENDENTE do conteúdo — antes o popup (`tui_popup::Popup`) se
/// auto-dimensionava pelo texto, e um cálculo de altura menor que a área
/// disponível deixava linhas da tela por baixo (ex. a tabela do dashboard)
/// sobreviverem nas bordas do popup (bug "pr"/"sto": fragmentos truncados
/// de texto que não pertenciam ao overlay). Com área fixa + `Clear`
/// explícito ANTES de qualquer conteúdo, nenhuma célula da tela anterior
/// sobrevive dentro do popup.
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vertical[1])[1]
}

/// Renderiza o overlay de ajuda por cima de tudo: `Clear` ANTES do conteúdo
/// numa área FIXA (60%x70% do frame, `centered_rect`) — mata o clipping que
/// deixava texto da tela por baixo vazar nas bordas do popup quando o
/// tamanho era auto-calculado a partir do conteúdo (T14).
fn render_help_overlay(frame: &mut Frame) {
    let area = frame.area();
    let popup_area = centered_rect(60, 70, area);

    frame.render_widget(Clear, popup_area);

    let bg = ratatui::style::Color::Rgb(0x28, 0x2c, 0x34); // One Dark
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .style(Style::default().bg(bg))
        .border_style(Style::default().fg(to_ratatui(ColorToken::Blue)))
        .title(Span::styled(
            " agent-bar — atalhos ",
            Style::default()
                .fg(to_ratatui(ColorToken::Blue))
                .add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(popup_area);
    frame.render_widget(block, popup_area);

    let content = Paragraph::new(help_text())
        .style(Style::default().fg(to_ratatui(ColorToken::Text)))
        .wrap(Wrap { trim: false });
    frame.render_widget(content, inner);
}

/// Título direito da moldura externa: spinner (quando ha fetch em voo) +
/// custo de hoje + relogio da ultima atualizacao.
fn header_status(state: &AppState) -> Line<'static> {
    let mut spans: Vec<Span<'static>> = Vec::new();

    if !state.fetch_pending.is_empty() {
        let throbber_widget = Throbber::default()
            .throbber_set(BRAILLE_SIX)
            .throbber_style(
                Style::default()
                    .fg(to_ratatui(ColorToken::Cyan))
                    .add_modifier(Modifier::BOLD),
            )
            .use_type(throbber_widgets_tui::WhichUse::Spin);
        let mut throbber_state = ThrobberState::default();
        for _ in 0..state.throbber.index {
            throbber_state.calc_next();
        }
        spans.push(throbber_widget.to_symbol_span(&throbber_state));
        spans.push(Span::raw(" \u{b7} "));
    }

    // Count-up (T16): mostra `display_cost` (persegue `usage.total_cost.usd`
    // via lerp em AnimTick), não o valor bruto — "-" enquanto usage ainda
    // não carregou nenhuma vez (display_cost fica em 0.0 até o 1º load).
    let cost = if state.usage.is_some() {
        format!("${:.2}", state.display_cost)
    } else {
        "-".to_string()
    };
    spans.push(Span::styled(
        cost,
        Style::default().fg(to_ratatui(ColorToken::TextBright)),
    ));

    if let Some(dt) = state.last_update {
        spans.push(Span::raw(" \u{b7} "));
        spans.push(Span::styled(
            format!("{:02}:{:02}", dt.hour(), dt.minute()),
            Style::default().fg(to_ratatui(ColorToken::Comment)),
        ));
    }

    spans.push(Span::raw(" "));
    Line::from(spans).right_aligned()
}

/// Top-level render: lays out the full TUI and dispatches to sub-renders.
///
/// Moldura externa unica `BorderType::Rounded` com titulo ` agent-bar `
/// (esquerda) + status (direita). Interna: `[sidebar | content]`
/// horizontal — sidebar colapsa pra so a coluna de marcas quando o
/// terminal e mais estreito que `NARROW_WIDTH`.
///
/// `hits` acumula as zonas clicaveis do frame atual (Task 9) — o event_loop
/// consulta via `HitMap::at` ao processar `MouseEvent`. O caller e
/// responsavel por `hits.clear()` antes de cada `terminal.draw` (render nao
/// limpa sozinho: um HitMap vazio silenciosamente sem clear acumularia
/// zonas obsoletas de frames anteriores).
pub fn render(state: &AppState, frame: &mut Frame, hits: &mut HitMap) {
    let area = frame.area();

    let outer = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(to_ratatui(ColorToken::Comment)))
        .title(Span::styled(
            " agent-bar ",
            Style::default()
                .fg(to_ratatui(ColorToken::Blue))
                .add_modifier(Modifier::BOLD),
        ))
        .title(header_status(state));
    let inner = outer.inner(area);
    frame.render_widget(outer, area);

    let sidebar_w: u16 = if area.width < NARROW_WIDTH { 3 } else { 17 };
    let cols = Layout::horizontal([Constraint::Length(sidebar_w), Constraint::Min(0)]).split(inner);

    render_sidebar(state, frame, cols[0], hits);
    match state.screen {
        Screen::Overview => render_dashboard(state, frame, cols[1], hits),
        Screen::Detail => render_detail(state, frame, cols[1], hits),
        Screen::History => render_history(state, frame, cols[1], hits),
        Screen::Login => render_login(state, frame, cols[1], hits),
        Screen::Waybar => render_config(state, frame, cols[1], hits),
    }

    // Overlay de ajuda: renderizado por cima de tudo quando show_help=true.
    if state.show_help {
        render_help_overlay(frame);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::types::{ProviderQuota, QuotaWindow};
    use crate::tui::mouse::MouseTarget;
    use crate::tui::state::{FetchStatus, ProviderView};
    use crate::usage::amp::AmpDollars;
    use crate::usage::{Cost, ModelUsage, ProviderUsage, UsageSummary};

    /// Settings mínimas pra inicializar `ConfigState` (usada só pelos
    /// testes de help overlay sobre a tela Waybar — não exercita edição).
    fn fake_settings() -> crate::settings::Settings {
        use crate::settings::*;
        use std::collections::BTreeMap;
        Settings {
            version: 2,
            waybar: Waybar {
                providers: vec!["claude".to_string(), "codex".to_string()],
                show_percentage: true,
                separators: SeparatorStyle::Gap,
                provider_order: vec!["claude".to_string(), "codex".to_string()],
                display_mode: DisplayMode::Remaining,
                signal: Some(8),
                interval: 60,
            },
            tooltip: Tooltip {},
            models: BTreeMap::new(),
            window_policy: BTreeMap::new(),
            notify: Notify { enabled: true },
            cache: CacheSettings {
                ttl: BTreeMap::new(),
            },
            menu: MenuSettings {
                animations: true,
                font_family: "IBM Plex Mono".to_string(),
                font_size: 12,
            },
            glyph_mode: GlyphMode::Box,
            fx_rate: 5.50,
        }
    }

    fn make_quota(
        id: &str,
        display: &str,
        remaining: f64,
        resets_at: Option<&str>,
    ) -> ProviderQuota {
        ProviderQuota {
            provider: id.to_string(),
            display_name: display.to_string(),
            available: true,
            account: None,
            plan: None,
            plan_type: None,
            primary: Some(QuotaWindow {
                remaining,
                resets_at: resets_at.map(|s| s.to_string()),
                window_minutes: Some(300),
                used: Some(100.0 - remaining),
                severity: None,
            }),
            secondary: None,
            models: None,
            extra: None,
            error: None,
        }
    }

    /// Constroi um UsageSummary falso para testes de dashboard:
    /// - claude: $2.10 / R$11.55
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
                    by_model: vec![ModelUsage {
                        model: "claude-opus-4-8".to_string(),
                        input: 800_000,
                        output: 100_000,
                        cache_read: 0,
                        cache_write: 0,
                        cost: Some(Cost {
                            usd: 1.40,
                            brl: 7.70,
                        }),
                    }],
                    amp_dollars: None,
                },
                ProviderUsage {
                    provider: "codex".to_string(),
                    total_input: 500_000,
                    total_output: 80_000,
                    total_cache_read: 0,
                    total_cache_write: 0,
                    cost: None,
                    by_model: vec![],
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
    fn dashboard_renders_providers_table() {
        let backend = ratatui::backend::TestBackend::new(64, 20);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        let mut state = AppState::new();
        state.providers = vec![
            ProviderView::new(make_quota(
                "claude",
                "Claude",
                26.0,
                Some("2026-06-19T23:00:00Z"),
            )),
            ProviderView::new(make_quota(
                "codex",
                "Codex",
                1.0,
                Some("2026-06-20T01:28:00Z"),
            )),
            ProviderView::new(make_quota("amp", "Amp", 0.0, None)),
        ];
        state.status = FetchStatus::Loaded;
        terminal
            .draw(|f| render(&state, f, &mut HitMap::default()))
            .unwrap();
        insta::assert_snapshot!(terminal.backend());
    }

    #[test]
    fn dashboard_renders_with_real_cost() {
        let backend = ratatui::backend::TestBackend::new(64, 20);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        let mut state = AppState::new();
        state.providers = vec![
            ProviderView::new(make_quota(
                "claude",
                "Claude",
                26.0,
                Some("2026-06-19T23:00:00Z"),
            )),
            ProviderView::new(make_quota(
                "codex",
                "Codex",
                1.0,
                Some("2026-06-20T01:28:00Z"),
            )),
            ProviderView::new(make_quota("amp", "Amp", 0.0, None)),
        ];
        state.status = FetchStatus::Loaded;
        // display_cost (T16): header agora mostra o count-up, não
        // usage.total_cost.usd direto — sem isto, o header ficaria em
        // "$0.00" (default de AppState::new()) em vez do custo real.
        let usage = fake_usage();
        state.display_cost = usage.total_cost.usd;
        state.usage = Some(usage);
        terminal
            .draw(|f| render(&state, f, &mut HitMap::default()))
            .unwrap();
        insta::assert_snapshot!(terminal.backend());
    }

    #[test]
    fn quota_bar_logic() {
        // 100% remaining = all filled (nothing consumed)
        let bar_100 = dashboard::quota_bar_pub(100.0);
        // 0% remaining = all empty (fully consumed)
        let bar_0 = dashboard::quota_bar_pub(0.0);
        assert_eq!(
            bar_100,
            "\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}"
        ); // all filled
        assert_eq!(
            bar_0,
            "\u{2592}\u{2592}\u{2592}\u{2592}\u{2592}\u{2592}\u{2592}"
        ); // all empty (trilho ▒)
    }

    #[test]
    fn help_overlay_renders_snapshot() {
        // Terminal generoso (60%x70% do popup comporta as 4 seções + dica
        // de mouse sem wrap/corte).
        let backend = ratatui::backend::TestBackend::new(100, 44);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        let mut state = AppState::new();
        state.show_help = true;
        terminal
            .draw(|f| render(&state, f, &mut HitMap::default()))
            .unwrap();
        insta::assert_snapshot!(terminal.backend());
    }

    #[test]
    fn help_overlay_clear_prevents_underlying_table_from_leaking() {
        // Regressão do bug "pr"/"sto" (T14): o popup ERA auto-dimensionado
        // pelo conteúdo (`tui_popup::Popup`); se o cálculo automático desse
        // uma área menor que o necessário, fragmentos da tabela por baixo
        // sobreviviam dentro das bordas do popup. Fix: `centered_rect`
        // fixa a área (60%x70% do frame) e `Clear` roda ANTES de qualquer
        // conteúdo nessa área exata — célula a célula, nada da tabela do
        // dashboard por baixo (aqui, os textos "sessão"/"26%"/"$2.10" dos
        // cards) pode sobreviver dentro do popup.
        let backend = ratatui::backend::TestBackend::new(100, 44);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        let mut state = AppState::new();
        state.providers = vec![
            ProviderView::new(make_quota(
                "claude",
                "Claude",
                26.0,
                Some("2026-06-19T23:00:00Z"),
            )),
            ProviderView::new(make_quota(
                "codex",
                "Codex",
                1.0,
                Some("2026-06-20T01:28:00Z"),
            )),
            ProviderView::new(make_quota("amp", "Amp", 0.0, None)),
        ];
        state.status = FetchStatus::Loaded;
        // display_cost (T16): header agora mostra o count-up, não
        // usage.total_cost.usd direto — sem isto, o header ficaria em
        // "$0.00" (default de AppState::new()) em vez do custo real.
        let usage = fake_usage();
        state.display_cost = usage.total_cost.usd;
        state.usage = Some(usage);
        state.show_help = true;

        terminal
            .draw(|f| render(&state, f, &mut HitMap::default()))
            .unwrap();

        let popup_area = centered_rect(60, 70, Rect::new(0, 0, 100, 44));
        let buffer = terminal.backend().buffer();
        let mut popup_text = String::new();
        for y in popup_area.y..popup_area.y + popup_area.height {
            for x in popup_area.x..popup_area.x + popup_area.width {
                if let Some(cell) = buffer.cell((x, y)) {
                    popup_text.push_str(cell.symbol());
                }
            }
            popup_text.push('\n');
        }

        for leaked in ["sessão", "26%", "$2.10", "hoje", "23:00"] {
            assert!(
                !popup_text.contains(leaked),
                "conteúdo do dashboard por baixo vazou dentro do popup ({leaked:?} encontrado):\n{popup_text}"
            );
        }
    }

    #[test]
    fn help_overlay_clears_over_login_screen() {
        // Mesma regressão, sobre a tela Login (reskin da T14) — confirma
        // que o Clear cobre a área do popup em QUALQUER tela, não só
        // Overview.
        let backend = ratatui::backend::TestBackend::new(100, 44);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        let mut state = AppState::new();
        state.screen = Screen::Login;
        state.show_help = true;
        terminal
            .draw(|f| render(&state, f, &mut HitMap::default()))
            .unwrap();
        insta::assert_snapshot!(terminal.backend());
    }

    #[test]
    fn help_overlay_clears_over_waybar_screen() {
        let backend = ratatui::backend::TestBackend::new(100, 44);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        let mut state = AppState::new();
        state.screen = Screen::Waybar;
        state.config_state = Some(crate::tui::state::ConfigState::new(&fake_settings()));
        state.show_help = true;
        terminal
            .draw(|f| render(&state, f, &mut HitMap::default()))
            .unwrap();
        insta::assert_snapshot!(terminal.backend());
    }

    #[test]
    fn dashboard_renders_wide_160() {
        let backend = ratatui::backend::TestBackend::new(160, 40);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        let mut state = AppState::new();
        state.providers = vec![
            ProviderView::new(make_quota(
                "claude",
                "Claude",
                26.0,
                Some("2026-06-19T23:00:00Z"),
            )),
            ProviderView::new(make_quota(
                "codex",
                "Codex",
                1.0,
                Some("2026-06-20T01:28:00Z"),
            )),
            ProviderView::new(make_quota("amp", "Amp", 0.0, None)),
        ];
        state.status = FetchStatus::Loaded;
        // display_cost (T16): header agora mostra o count-up, não
        // usage.total_cost.usd direto — sem isto, o header ficaria em
        // "$0.00" (default de AppState::new()) em vez do custo real.
        let usage = fake_usage();
        state.display_cost = usage.total_cost.usd;
        state.usage = Some(usage);
        terminal
            .draw(|f| render(&state, f, &mut HitMap::default()))
            .unwrap();
        insta::assert_snapshot!(terminal.backend());
    }

    #[test]
    fn render_registers_sidebar_hit_zones() {
        // Terminal largo (>=80) para exercitar a sidebar cheia (17 cols) —
        // a colapsada tem teste dedicado em `sidebar_collapses_below_80_cols`.
        let backend = ratatui::backend::TestBackend::new(90, 20);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        let mut state = AppState::new();
        state.providers = vec![
            ProviderView::new(make_quota("claude", "Claude", 26.0, None)),
            ProviderView::new(make_quota("codex", "Codex", 1.0, None)),
        ];
        state.status = FetchStatus::Loaded;
        let mut hits = HitMap::default();
        terminal.draw(|f| render(&state, f, &mut hits)).unwrap();

        // Sidebar nova: TODOS os itens de sidebar_items() (Overview,
        // Provider(0), Provider(1), History, Login, Waybar) tem zona
        // clicavel 1:1 com o indice do cursor — nao so os providers como na
        // sidebar antiga. Borda ALL Rounded -> inner comeca em (1,1);
        // "VISAO" ocupa a 1a linha do inner, entao Overview cai na 2a.
        assert_eq!(hits.at(1, 2), Some(MouseTarget::Sidebar(0))); // Overview
        assert_eq!(hits.at(1, 5), Some(MouseTarget::Sidebar(1))); // claude
        assert_eq!(hits.at(1, 6), Some(MouseTarget::Sidebar(2))); // codex
        assert_eq!(hits.at(1, 9), Some(MouseTarget::Sidebar(3))); // History
        assert_eq!(hits.at(1, 10), Some(MouseTarget::Sidebar(4))); // Login
        assert_eq!(hits.at(1, 11), Some(MouseTarget::Sidebar(5))); // Waybar
                                                                   // (50, 5) cai dentro do 1º card da Overview (Task 11: cards
                                                                   // registram MouseTarget::Card) — deixou de ser "fora de qualquer
                                                                   // zona" desde que o dashboard passou a ser cards clicáveis.
        assert_eq!(hits.at(50, 5), Some(MouseTarget::Card(0)));
        // Fora do frame inteiramente continua sem zona.
        assert_eq!(hits.at(200, 5), None);
    }

    #[test]
    fn sidebar_collapses_below_80_cols() {
        // < NARROW_WIDTH (80) -> sidebar Length(3), so a coluna de marcas.
        let backend = ratatui::backend::TestBackend::new(70, 30);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        let state = AppState::new();
        let mut hits = HitMap::default();
        terminal.draw(|f| render(&state, f, &mut hits)).unwrap();

        // Overview e sempre o 1o item (1a linha do inner e o header VISAO,
        // Overview cai na linha seguinte) — estavel independente do numero
        // de providers. Borda ALL Rounded -> inner comeca em (1,1).
        assert_eq!(hits.at(1, 2), Some(MouseTarget::Sidebar(0)));
        assert_eq!(hits.at(3, 2), Some(MouseTarget::Sidebar(0))); // ultima col da sidebar colapsada (largura 3: x=1..4)
        assert_eq!(hits.at(4, 2), None); // area de conteudo comeca na coluna 4
    }
}
