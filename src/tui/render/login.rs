//! Aba Login: lista os 3 providers com status de autenticacao e acao de login.
//! Sem emoji. Status vem do `LoginState` derivado do ULTIMO FETCH REAL (ver
//! `crate::tui::login_state`) — nunca de path.exists() ou binario no PATH.

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, List, ListItem, Paragraph};
use ratatui::Frame;

use crate::theme::ColorToken;
use crate::tui::login_state::{login_state_for, LoginState};
use crate::tui::mouse::{ChipKind, HitMap, MouseTarget};
use crate::tui::state::AppState;
use crate::tui::theme_bridge::{provider_color, to_ratatui};
use crate::tui::widgets::chips::{chips_line, register_chip_hits};
use crate::tui::widgets::icons::{glyph, Icon};

/// Constantes dos providers da aba Login (id, nome de exibicao).
const PROVIDERS: [(&str, &str); 3] = [("claude", "Claude"), ("codex", "Codex"), ("amp", "Amp")];

/// Renderiza a aba Login completa. O status de cada provider nao depende de
/// paths de credencial — vem do `LoginState` (ultimo fetch real).
pub fn render_login(state: &AppState, frame: &mut Frame, area: Rect, hits: &mut HitMap) {
    // Layout: [lista de providers | painel de detalhe/instrucao]
    // 30 cols cobre o pior caso sem truncar: cursor " > " (3) + marca (1) +
    // " Claude " (8, nome com folga de 6 + 2 espaços) + marca de status (1)
    // + " verificando…" (13, o label mais longo) = 26; + 2 de borda + 2 de
    // folga = 30.
    let horiz = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(30), Constraint::Min(0)])
        .split(area);

    let list_area = horiz[0];
    let detail_area = horiz[1];

    render_provider_list(state, frame, list_area, hits);
    render_detail_panel(state, frame, detail_area, hits);
}

/// Coluna esquerda: lista os providers com status derivado do ultimo fetch.
/// Cada linha registra `MouseTarget::Card(i)` — clique SELECIONA (mesmo
/// efeito de LoginUp/LoginDown); a ativação do login continua exclusiva do
/// Enter/chip (braço dedicado em `update.rs`, T14).
fn render_provider_list(state: &AppState, frame: &mut Frame, area: Rect, hits: &mut HitMap) {
    let selected_bg = to_ratatui(ColorToken::SelBg);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Thick)
        .border_style(Style::default().fg(to_ratatui(ColorToken::Blue)))
        .title(Span::styled(
            " Login ",
            Style::default()
                .fg(to_ratatui(ColorToken::TextBright))
                .add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(area);

    let items: Vec<ListItem<'_>> = PROVIDERS
        .iter()
        .enumerate()
        .map(|(i, &(id, name))| {
            let quota = state
                .providers
                .iter()
                .find(|pv| pv.quota.provider == id)
                .map(|pv| &pv.quota);
            let fetch_pending = state.fetch_pending.iter().any(|p| p == id);
            let login_state = login_state_for(quota, fetch_pending);
            let selected = i == state.login_selected;

            let cursor = if selected { " > " } else { "   " };
            // Marca colorida — mesma linguagem visual da sidebar
            // (`render/sidebar.rs::item_label`): ◆ pra Claude, ● pros
            // demais, sempre na cor de marca do provider.
            let mark = if id == "claude" {
                "\u{25c6}"
            } else {
                "\u{25cf}"
            };

            // Marca de status: ícone semântico (`tui::widgets::icons`) por
            // LoginState — Ok/NoToken/LoggedOut mapeiam 1:1; Error (falha
            // nao-auth: parse/rede/API — erro real, mas nao pede re-login)
            // usa Warn; Checking nao tem Icon dedicado (estado transitorio,
            // permanece ◐ literal).
            let (dot, label, status_fg, bold) = match login_state {
                LoginState::Ok => (
                    glyph(Icon::Ok, state.glyph_mode),
                    "ok",
                    to_ratatui(ColorToken::Green),
                    true,
                ),
                LoginState::NoToken => (
                    glyph(Icon::NoToken, state.glyph_mode),
                    "sem token",
                    to_ratatui(ColorToken::Yellow),
                    false,
                ),
                LoginState::LoggedOut => (
                    glyph(Icon::LoggedOut, state.glyph_mode),
                    "deslogado",
                    to_ratatui(ColorToken::Muted),
                    false,
                ),
                LoginState::Error => (
                    glyph(Icon::Warn, state.glyph_mode),
                    "erro",
                    to_ratatui(ColorToken::Red),
                    false,
                ),
                LoginState::Checking => (
                    "\u{25d0}",
                    "verificando\u{2026}",
                    to_ratatui(ColorToken::Cyan),
                    false,
                ),
            };

            let mark_style = Style::default().fg(provider_color(id));
            let mut name_style = Style::default().fg(to_ratatui(ColorToken::Text));
            if selected {
                name_style = name_style.add_modifier(Modifier::BOLD);
            }
            let mut status_style = Style::default().fg(status_fg);
            if bold {
                status_style = status_style.add_modifier(Modifier::BOLD);
            }

            let mut line = Line::from(vec![
                Span::raw(cursor),
                Span::styled(mark, mark_style),
                Span::styled(format!(" {name:<6} "), name_style),
                Span::styled(dot, status_style),
                Span::styled(format!(" {label}"), status_style),
            ]);
            // Preenche a linha INTEIRA (largura da coluna, não só o texto)
            // com o bg de seleção — estilo de linha entra "por baixo" dos
            // estilos de cada span (que só definem fg), então as cores de
            // marca/status sobrevivem por cima do highlight.
            if selected {
                line = line.style(Style::default().bg(selected_bg));
            }
            ListItem::new(line)
        })
        .collect();

    for i in 0..PROVIDERS.len() {
        let row_y = inner.y + i as u16;
        if row_y < inner.y + inner.height {
            hits.push(
                Rect::new(inner.x, row_y, inner.width, 1),
                MouseTarget::Card(i),
            );
        }
    }

    let list = List::new(items).block(block);
    frame.render_widget(list, area);
}

/// Painel direito: instrucoes, chips de ação e status de feedback.
fn render_detail_panel(state: &AppState, frame: &mut Frame, area: Rect, hits: &mut HitMap) {
    let (id, name) = PROVIDERS[state.login_selected];

    let hint = match id {
        "claude" => "Abre a REPL do Claude. Digite /login e siga as instruções.",
        "codex" => "Executa `codex login` (fluxo OAuth no browser).",
        "amp" => "Executa `amp login` (autenticação no browser).",
        _ => "",
    };

    let mut lines: Vec<Line<'_>> = vec![
        Line::from(Span::styled(
            format!(" {name}"),
            Style::default()
                .fg(provider_color(id))
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            format!(" {hint}"),
            Style::default().fg(to_ratatui(ColorToken::Text)),
        )),
    ];

    // Exibe status de feedback se houver (erro ou confirmacao de sucesso).
    if let Some(msg) = &state.login_status {
        lines.push(Line::from(""));
        let style = if msg.starts_with("Erro") {
            Style::default().fg(to_ratatui(ColorToken::Red))
        } else {
            Style::default().fg(to_ratatui(ColorToken::Cyan))
        };
        lines.push(Line::from(Span::styled(format!(" {msg}"), style)));
    }

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Thick)
        .border_style(Style::default().fg(to_ratatui(ColorToken::Comment)));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let vert = Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).split(inner);
    frame.render_widget(Paragraph::new(lines), vert[0]);

    let chips: [(ChipKind, &str, &str); 2] = [
        (ChipKind::StartLogin, "\u{21b5}", "iniciar login"),
        (ChipKind::Back, "esc", "voltar"),
    ];
    let chips_area = vert[1];
    let line = chips_line(&chips, chips_area.width);
    frame.render_widget(Paragraph::new(line), chips_area);
    register_chip_hits(&chips, chips_area, hits);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::types::ProviderQuota;
    use crate::tui::state::{AppState, ProviderView};

    fn quota(provider: &str, available: bool, error: Option<&str>) -> ProviderQuota {
        ProviderQuota {
            provider: provider.to_string(),
            display_name: provider.to_string(),
            available,
            account: None,
            plan: None,
            plan_type: None,
            primary: None,
            secondary: None,
            models: None,
            extra: None,
            error: error.map(|s| s.to_string()),
        }
    }

    #[test]
    fn render_login_snapshot_mixed_login_states() {
        // Regressão do bug que esta task mata: aba Login refletia path.exists()
        // /binário no PATH, contradizendo o dashboard (fetch real). Aqui claude
        // = Ok, codex = NoToken (erro com fonte presente), amp = Error (falha
        // não-auth — parse/rede/API — nunca rotulada "deslogado", spec §10) —
        // tudo derivado de state.providers. LoggedOut fica coberto pelo
        // snapshot com providers vazios (`render_login_snapshot`).
        let mut state = AppState::new();
        state.providers = vec![
            ProviderView::new(quota("claude", true, None)),
            ProviderView::new(quota("codex", true, Some("Codex API error 401"))),
            ProviderView::new(quota("amp", false, Some("Failed to parse usage"))),
        ];
        use crate::tui::state::Screen;
        state.screen = Screen::Login;

        let backend = ratatui::backend::TestBackend::new(64, 16);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                let area = f.area();
                render_login(&state, f, area, &mut HitMap::default());
            })
            .unwrap();
        insta::assert_snapshot!(terminal.backend());
    }

    #[test]
    fn render_login_snapshot() {
        // Sem providers no state (fetch nunca rodou): todos deslogados.
        let mut state = AppState::new();
        state.login_selected = 1; // Codex selecionado
        use crate::tui::state::Screen;
        state.screen = Screen::Login;

        let backend = ratatui::backend::TestBackend::new(64, 16);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                let area = f.area();
                render_login(&state, f, area, &mut HitMap::default());
            })
            .unwrap();
        insta::assert_snapshot!(terminal.backend());
    }

    #[test]
    fn render_login_snapshot_with_status() {
        let mut state = AppState::new();
        state.login_selected = 0;
        state.login_status = Some("Erro no login: claude nao encontrado".to_string());
        use crate::tui::state::Screen;
        state.screen = Screen::Login;

        let backend = ratatui::backend::TestBackend::new(64, 16);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                let area = f.area();
                render_login(&state, f, area, &mut HitMap::default());
            })
            .unwrap();
        insta::assert_snapshot!(terminal.backend());
    }
}
