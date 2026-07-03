pub mod config;
pub mod dashboard;
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
        "Config do Waybar",
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

/// Uma linha é "de atalho" (não título, não separador) se tiver mais de um
/// span — `help_section` monta títulos como Line de 1 span centrado e
/// linhas de atalho como Line de 2 spans (tecla + ação); os separadores são
/// `Line::from("")`, também 1 span (vazio). Usado por `help_text_fitting`
/// pra truncar sem cortar um título no meio e pra contar `n_ocultos` só de
/// atalhos de verdade.
fn is_shortcut_line(line: &Line<'static>) -> bool {
    line.spans.len() > 1
}

/// Monta o conteúdo do popup de ajuda que CABE em `max_rows` linhas,
/// em 3 níveis progressivos (T-10, corte mudo era o bug: terminal baixo
/// <30 linhas cortava as últimas seções sem avisar):
///
/// (a) `help_text()` completo já cabe (`height() <= max_rows`) → retorna
///     como está — é o caso comum (>=30 linhas de terminal).
/// (b) não cabe: reconstrói removendo os separadores `Line::from("")` entre
///     seções (compactação). Se o resultado cabe, retorna — nenhum atalho
///     é perdido, só o respiro visual.
/// (c) ainda não cabe: trunca a versão compactada em `max_rows - 1` linhas
///     e acrescenta uma linha final `… (+N atalhos)` em `Muted`. `N` =
///     quantidade de linhas de ATALHO (`is_shortcut_line`, não título/
///     separador) que ficaram de fora do corte — títulos de seção não
///     entram na contagem porque por si só não são um atalho perdido.
fn help_text_fitting(max_rows: usize) -> Text<'static> {
    let full = help_text();
    if full.height() <= max_rows {
        return full;
    }

    let compact: Vec<Line<'static>> = full
        .lines
        .into_iter()
        .filter(|line| !line.spans.is_empty())
        .collect();
    if compact.len() <= max_rows {
        return Text::from(compact);
    }

    let keep = max_rows.saturating_sub(1);
    let n_ocultos = compact[keep..]
        .iter()
        .filter(|l| is_shortcut_line(l))
        .count();
    let mut truncated: Vec<Line<'static>> = compact.into_iter().take(keep).collect();
    truncated.push(
        Line::from(Span::styled(
            format!("… (+{n_ocultos} atalhos)"),
            Style::default().fg(to_ratatui(ColorToken::Muted)),
        ))
        .centered(),
    );
    Text::from(truncated)
}

/// Área do popup de ajuda: dimensionada pelo CONTEÚDO de `text` (altura =
/// linhas + 2 bordas; largura = linha mais longa + bordas e respiro),
/// clampada ao frame e centralizada. A área era um percentual fixo do frame
/// (60%x70%) — em terminais reais menores que o dos snapshots (ex.
/// 110x32), 70% dava menos linhas que o conteúdo e as seções finais
/// (Login/Histórico) morriam cortadas na borda de baixo.
/// Recebe `text` (em vez de chamar `help_text()`/`help_text_fitting()`
/// internamente) pra garantir que área e conteúdo derivem da MESMA `Text` —
/// duas chamadas independentes de `help_text_fitting` podiam, em teoria,
/// divergir se o cálculo dependesse de algo além do `max_rows` (T-10).
/// O `Clear` explícito ANTES do conteúdo (em `render_help_overlay`)
/// continua obrigatório: sem ele, células da tela por baixo sobrevivem
/// dentro do popup (bug "pr"/"sto" da T14).
fn help_popup_area(frame_area: Rect, text: &Text<'static>) -> Rect {
    let width = (text.width() as u16).saturating_add(6).min(frame_area.width);
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
    // max_rows = altura do frame menos as 2 bordas do popup (topo/base) —
    // é o número de linhas de CONTEÚDO que cabem dentro do inner.
    let max_rows = area.height.saturating_sub(2) as usize;
    let text = help_text_fitting(max_rows);
    let popup_area = help_popup_area(area, &text);

    frame.render_widget(
        Block::default().style(Style::default().add_modifier(Modifier::DIM)),
        area,
    );
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

    let content = Paragraph::new(text)
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
    // via lerp em AnimTick), não o valor bruto — fallback enquanto usage
    // ainda não carregou nenhuma vez (display_cost fica em 0.0 até o 1º
    // load). Fix (review final): o fallback era `!fetch_pending.is_empty()`,
    // que é o fetch de QUOTA — mas o parse de usage roda em outra thread e,
    // quando a quota resolve via cache antes do parse terminar, o header
    // mostrava "-" por segundos no 1º boot. `usage.is_none()` já basta como
    // condição do "…": o parse inicial sempre dispara no boot, então não há
    // estado real de "vazio" distinto de "ainda carregando" pro custo.
    let cost = if state.usage.is_some() {
        format!("${:.2}", state.display_cost)
    } else {
        "…".to_string()
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
    fn help_overlay_clear_prevents_underlying_table_from_leaking() {
        // Regressão do bug "pr"/"sto" (T14): fragmentos da tabela por
        // baixo sobreviviam dentro das bordas do popup quando o Clear não
        // cobria a área inteira. `Clear` roda ANTES de qualquer conteúdo
        // na área exata de `help_popup_area` — célula a célula, nada da
        // tabela do dashboard por baixo (aqui, os textos "sessão"/"26%"/
        // "$2.10" dos cards) pode sobreviver dentro do popup.
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

        let frame_area = Rect::new(0, 0, 100, 44);
        let text = help_text_fitting(frame_area.height.saturating_sub(2) as usize);
        let popup_area = help_popup_area(frame_area, &text);
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
            "alterna 24h/7d",              // linha da seção Histórico
            "iniciar login do provider",   // linha da seção Login
            "shift+drag seleciona texto",  // dica de mouse (última linha)
        ] {
            assert!(
                screen.contains(expected),
                "conteúdo do help cortado ({expected:?} ausente):\n{screen}"
            );
        }
    }

    #[test]
    fn help_overlay_compacts_then_truncates_at_78x24() {
        let backend = ratatui::backend::TestBackend::new(78, 24);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        let mut state = AppState::new();
        state.show_help = true;
        terminal
            .draw(|f| render(&state, f, &mut HitMap::default()))
            .unwrap();
        let buffer = terminal.backend().buffer();
        let mut screen = String::new();
        for y in 0..24u16 {
            for x in 0..78u16 {
                if let Some(cell) = buffer.cell((x, y)) {
                    screen.push_str(cell.symbol());
                }
            }
            screen.push('\n');
        }
        // 22 linhas úteis: compactação (23 linhas de conteúdo) não basta →
        // trunca com indicador. NUNCA corte mudo.
        assert!(
            screen.contains("atalhos)"),
            "corte deve ser anunciado com '… (+N atalhos)':\n{screen}"
        );
    }

    #[test]
    fn help_text_fitting_compacts_without_truncating_at_level_2() {
        // Nível 2 (b): `help_text()` completa (28 linhas) não cabe, mas a
        // versão compactada (23 linhas, sem os 5 separadores em branco)
        // cabe. Faixa de `max_rows` que cai aqui é 23..=27 — 24 fica no
        // meio, longe dos limites já cobertos por
        // `help_overlay_shows_all_sections_at_110x32` (nível 1, >=28) e
        // `help_overlay_compacts_then_truncates_at_78x24` (nível 3, =22).
        // Sem este teste, uma regressão no filtro
        // `!line.spans.is_empty()` (ex. trocar por
        // `line.width() == 0`, que também classificaria títulos/atalhos
        // vazios de forma diferente) passaria despercebida.
        let text = help_text_fitting(24);

        // Exatamente 23 linhas: 28 - 5 separadores. Nem uma a mais
        // (indicaria que o filtro não removeu algum separador) nem uma a
        // menos (indicaria que o filtro comeu uma linha de conteúdo).
        assert_eq!(
            text.lines.len(),
            23,
            "nível 2 deve compactar pra exatamente 23 linhas, sem truncar"
        );

        // Nenhuma linha vazia (separador) sobrou entre seções — é
        // exatamente o que o filtro `!line.spans.is_empty()` garante.
        assert!(
            text.lines.iter().all(|line| !line.spans.is_empty()),
            "compactação não deve deixar linha com spans vazios (separador sobrevivente)"
        );

        let screen: String = text
            .lines
            .iter()
            .map(|line| line.to_string())
            .collect::<Vec<_>>()
            .join("\n");

        // Títulos das 5 seções presentes — se o filtro engolisse um
        // título por engano (ex. um título viesse a ter spans vazios em
        // alguma reescrita futura de `help_section`), este assert pega.
        for title in [
            "Navegação global",
            "Overview",
            "Config do Waybar",
            "Login",
            "Histórico",
        ] {
            assert!(
                screen.contains(title),
                "título de seção ausente na compactação: {title:?}\n{screen}"
            );
        }

        // Atalhos representativos de cada seção, íntegros (nada truncado
        // — nível 2 nunca corta conteúdo, só remove respiro visual).
        for shortcut in [
            "alterna 24h/7d",
            "iniciar login do provider",
            "shift+drag seleciona texto",
        ] {
            assert!(
                screen.contains(shortcut),
                "atalho ausente/truncado na compactação: {shortcut:?}\n{screen}"
            );
        }

        // Nível 2 nunca trunca — não deve haver indicador de corte.
        assert!(
            !screen.contains("atalhos)"),
            "nível 2 não deve truncar (achou indicador de corte '… (+N atalhos)'):\n{screen}"
        );
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

    #[test]
    fn header_first_load_shows_ellipsis_not_dash() {
        let mut state = AppState::new();
        state.fetch_pending = vec!["claude".to_string()];
        let line = header_status(&state);
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(!text.contains('-'), "primeiro load não mostra '-': {text:?}");
        assert!(text.contains('…'), "primeiro load mostra reticências: {text:?}");
    }

    #[test]
    fn header_shows_ellipsis_even_with_no_fetch_pending() {
        // Regressão do review final: quota resolve por cache (fetch_pending
        // fica vazio) mas o parse de usage — outra thread — ainda não
        // terminou. `usage.is_none()` sozinho já basta como condição do
        // "…"; não deve sobrar caminho pro "-" nesse cenário.
        let state = AppState::new();
        assert!(state.fetch_pending.is_empty());
        assert!(state.usage.is_none());
        let line = header_status(&state);
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(!text.contains('-'), "sem fetch pendente não mostra '-': {text:?}");
        assert!(text.contains('…'), "sem fetch pendente mostra reticências: {text:?}");
    }
}
