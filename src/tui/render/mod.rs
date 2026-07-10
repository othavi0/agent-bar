pub mod config;
pub mod detail;
pub mod history;
pub mod login;
mod shared;
pub mod sidebar;

use ratatui::layout::{Constraint, Layout, Rect};
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
/// Config, Login, Histórico) — data-driven em vez de repetir a mesma
/// construção de `Line` várias vezes. (Task 11: a seção "Overview" morreu
/// junto com a tela — Detail já é coberto pela Navegação global.)
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
            ("h / g / w", "Histórico / Login / Config"),
            ("q", "sair"),
            ("r", "atualizar quotas"),
        ],
    ));
    lines.push(Line::from(""));

    lines.extend(help_section(
        "Config",
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

/// Área do popup de ajuda: dimensionada pelo CONTEÚDO de `help_text()`
/// (altura = linhas + 2 bordas; largura = linha mais longa + bordas e
/// respiro), clampada ao frame e centralizada. A área era um percentual
/// fixo do frame (60%x70%) — em terminais reais menores que o dos
/// snapshots (ex. 110x32), 70% dava menos linhas que o conteúdo e as
/// seções finais (Login/Histórico) morriam cortadas na borda de baixo.
/// O `Clear` explícito ANTES do conteúdo (em `render_help_overlay`)
/// continua obrigatório: sem ele, células da tela por baixo sobrevivem
/// dentro do popup (bug "pr"/"sto" da T14).
fn help_popup_area(frame_area: Rect) -> Rect {
    let text = help_text();
    let width = (text.width() as u16)
        .saturating_add(6)
        .min(frame_area.width);
    let height = (text.height() as u16)
        .saturating_add(2)
        .min(frame_area.height);
    let x = frame_area.x + (frame_area.width - width) / 2;
    let y = frame_area.y + (frame_area.height - height) / 2;
    Rect::new(x, y, width, height)
}

/// Renderiza o overlay de ajuda por cima de tudo. A tela inteira por baixo
/// é escurecida (DIM) antes do popup — sem isso, texto vivo encostado na
/// borda (ex. a cauda de uma linha longa da tela de baixo) lê como
/// vazamento/sujeira do popup.
fn render_help_overlay(frame: &mut Frame) {
    let area = frame.area();
    let popup_area = help_popup_area(area);

    frame.render_widget(
        Block::default().style(Style::default().add_modifier(Modifier::DIM)),
        area,
    );
    frame.render_widget(Clear, popup_area);

    let bg = to_ratatui(ColorToken::Bg);
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

    #[test]
    fn help_overlay_renders_snapshot() {
        // Terminal generoso (o popup dimensionado pelo conteúdo cabe com
        // folga, sem wrap/corte).
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
    fn help_overlay_clear_prevents_underlying_content_from_leaking() {
        // Regressão do bug "pr"/"sto" (T14): fragmentos da tela por baixo
        // sobreviviam dentro das bordas do popup quando o Clear não cobria
        // a área inteira. `Clear` roda ANTES de qualquer conteúdo na área
        // exata de `help_popup_area` — célula a célula. Migrado do
        // Overview/dashboard (T11, apagados): a tela por baixo agora é o
        // skeleton de boot do Detail (providers vazio + fetch em voo) —
        // mesma garantia, fixture nova.
        let backend = ratatui::backend::TestBackend::new(100, 44);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        let mut state = AppState::new();
        state.pending_focus = Some("claude".to_string());
        state.fetch_pending = vec!["claude".to_string()];
        state.show_help = true;

        terminal
            .draw(|f| render(&state, f, &mut HitMap::default()))
            .unwrap();

        let popup_area = help_popup_area(Rect::new(0, 0, 100, 44));
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

        for leaked in ["carregando", "sess\u{e3}o", "semana", "Claude"] {
            assert!(
                !popup_text.contains(leaked),
                "conteúdo do skeleton por baixo vazou dentro do popup ({leaked:?} encontrado):\n{popup_text}"
            );
        }
    }

    #[test]
    fn help_overlay_shows_all_sections_at_110x32() {
        // Regressão do corte visto na máquina real (110x32): com a área do
        // popup fixada em 60%x70% do frame, o inner ficava com 20 linhas
        // para 28 linhas de conteúdo — as seções finais (Login/Histórico)
        // e a dica de mouse morriam na borda de baixo. O popup deve se
        // dimensionar pelo CONTEÚDO (clampado ao frame), então tudo
        // precisa estar visível.
        let backend = ratatui::backend::TestBackend::new(110, 32);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        let mut state = AppState::new();
        state.show_help = true;
        terminal
            .draw(|f| render(&state, f, &mut HitMap::default()))
            .unwrap();

        let buffer = terminal.backend().buffer();
        let mut screen = String::new();
        for y in 0..32u16 {
            for x in 0..110u16 {
                if let Some(cell) = buffer.cell((x, y)) {
                    screen.push_str(cell.symbol());
                }
            }
            screen.push('\n');
        }
        for expected in [
            "alterna 24h/7d",             // linha da seção Histórico
            "iniciar login do provider",  // linha da seção Login
            "shift+drag seleciona texto", // dica de mouse (última linha)
        ] {
            assert!(
                screen.contains(expected),
                "conteúdo do help cortado ({expected:?} ausente):\n{screen}"
            );
        }
    }

    #[test]
    fn help_overlay_clears_over_login_screen() {
        // Mesma regressão, sobre a tela Login (reskin da T14) — confirma
        // que o Clear cobre a área do popup em QUALQUER tela, não só
        // Detail.
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

        // Sidebar sem Overview (Task 11): TODOS os itens de sidebar_items()
        // (Provider(0), Provider(1), History, Login, Waybar) tem zona
        // clicavel 1:1 com o indice do cursor. Borda ALL Rounded -> inner
        // comeca em (1,1); PROVEDORES ganha 1 linha em branco + o header
        // antes do 1o provider, entao claude cai na 3a linha do inner.
        assert_eq!(hits.at(1, 3), Some(MouseTarget::Sidebar(0))); // claude
        assert_eq!(hits.at(1, 4), Some(MouseTarget::Sidebar(1))); // codex
        assert_eq!(hits.at(1, 7), Some(MouseTarget::Sidebar(2))); // History
        assert_eq!(hits.at(1, 8), Some(MouseTarget::Sidebar(3))); // Login
        assert_eq!(hits.at(1, 9), Some(MouseTarget::Sidebar(4))); // Waybar
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

        // Sem providers, sidebar_items(0) = [History, Login, Waybar] (Task
        // 11: sem Overview) — a seção MAIS ganha 1 linha em branco antes
        // (mesmo padrão de PROVEDORES), então History cai na 3a linha do
        // inner. Borda ALL Rounded -> inner comeca em (1,1).
        assert_eq!(hits.at(1, 3), Some(MouseTarget::Sidebar(0)));
        assert_eq!(hits.at(3, 3), Some(MouseTarget::Sidebar(0))); // ultima col da sidebar colapsada (largura 3: x=1..4)
        assert_eq!(hits.at(4, 3), None); // area de conteudo comeca na coluna 4
    }
}
