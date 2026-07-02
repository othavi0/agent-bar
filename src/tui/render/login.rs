//! Aba Login: lista os 3 providers com status de autenticacao e acao de login.
//! Sem emoji. Status vem do `LoginState` derivado do ULTIMO FETCH REAL (ver
//! `crate::tui::login_state`) — nunca de path.exists() ou binario no PATH.

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, List, ListItem, Paragraph};
use ratatui::Frame;

use crate::config::Paths;
use crate::theme::ColorToken;
use crate::tui::login_state::{login_state_for, LoginState};
use crate::tui::state::AppState;
use crate::tui::theme_bridge::{provider_color, to_ratatui};

/// Constantes dos providers da aba Login (id, nome de exibicao).
const PROVIDERS: [(&str, &str); 3] = [("claude", "Claude"), ("codex", "Codex"), ("amp", "Amp")];

/// Renderiza a aba Login completa. `_paths_opt` fica no signature so por
/// compat com o dispatcher (`render/mod.rs`); o status de cada provider nao
/// depende mais de paths de credencial — vem do `LoginState` (ultimo fetch).
pub fn render_login(state: &AppState, _paths_opt: Option<&Paths>, frame: &mut Frame, area: Rect) {
    // Layout: [lista de providers | painel de detalhe/instrucao]
    // 28 cols cobre o pior caso sem truncar: prefixo " > " (3) + "Claude" (6)
    // + " [verificando…]" (15, o label mais longo) + 2 de borda = 26; +2 de
    // folga. Labels antigos ("[ok]"/"[--]") cabiam em 22, mas os novos labels
    // em PT-BR ("deslogado", "sem token", "verificando…") sao mais longos.
    let horiz = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(28), Constraint::Min(0)])
        .split(area);

    let list_area = horiz[0];
    let detail_area = horiz[1];

    render_provider_list(state, frame, list_area);
    render_detail_panel(state, frame, detail_area);
}

/// Coluna esquerda: lista os providers com status derivado do ultimo fetch.
fn render_provider_list(state: &AppState, frame: &mut Frame, area: Rect) {
    let selected_bg = ratatui::style::Color::Rgb(45, 53, 65);

    let items: Vec<ListItem<'_>> = PROVIDERS
        .iter()
        .enumerate()
        .map(|(i, &(id, name))| {
            let quota = state
                .providers
                .iter()
                .find(|pv| pv.quota.provider == id)
                .map(|pv| &pv.quota);
            // Task 5 liga o pending real; ate la, nunca "Checking" por aqui.
            let fetch_pending = false;
            let login_state = login_state_for(quota, fetch_pending);
            let selected = i == state.login_selected;

            let prefix = if selected { " > " } else { "   " };

            let name_style = if selected {
                Style::default()
                    .fg(provider_color(id))
                    .add_modifier(Modifier::BOLD)
                    .bg(selected_bg)
            } else {
                Style::default().fg(provider_color(id))
            };

            let prefix_style = if selected {
                Style::default()
                    .fg(to_ratatui(ColorToken::TextBright))
                    .bg(selected_bg)
            } else {
                Style::default().fg(to_ratatui(ColorToken::Comment))
            };

            let (status_text, status_fg, bold) = match login_state {
                LoginState::Ok => (" [ok]", to_ratatui(ColorToken::Green), true),
                LoginState::NoToken => (" [sem token]", to_ratatui(ColorToken::Yellow), false),
                LoginState::LoggedOut => (" [deslogado]", to_ratatui(ColorToken::Muted), false),
                // Falha nao-auth (parse/rede/API): erro real, mas nao pede
                // re-login — cor de atencao distinta de "deslogado".
                LoginState::Error => (" [erro]", to_ratatui(ColorToken::Red), false),
                LoginState::Checking => (" [verificando…]", to_ratatui(ColorToken::Cyan), false),
            };
            let status_style = if selected {
                Style::default()
                    .fg(status_fg)
                    .add_modifier(if bold {
                        Modifier::BOLD
                    } else {
                        Modifier::empty()
                    })
                    .bg(selected_bg)
            } else {
                Style::default().fg(status_fg).add_modifier(if bold {
                    Modifier::BOLD
                } else {
                    Modifier::empty()
                })
            };

            ListItem::new(Line::from(vec![
                Span::styled(prefix, prefix_style),
                Span::styled(name, name_style),
                Span::styled(status_text, status_style),
            ]))
        })
        .collect();

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

    let list = List::new(items).block(block);
    frame.render_widget(list, area);
}

/// Painel direito: instrucoes e status de feedback.
fn render_detail_panel(state: &AppState, frame: &mut Frame, area: Rect) {
    let (id, name) = PROVIDERS[state.login_selected];

    let hint = match id {
        "claude" => "Abre a REPL do Claude. Digite /login e siga as instrucoes.",
        "codex" => "Executa `codex auth login` (fluxo OAuth no browser).",
        "amp" => "Executa `amp login` (autenticacao no browser).",
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
        Line::from(""),
        Line::from(Span::styled(
            " [Enter] iniciar login    [j/k] navegar    [q] sair",
            Style::default().fg(to_ratatui(ColorToken::Comment)),
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

    let p = Paragraph::new(lines).block(block);
    frame.render_widget(p, area);
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
        use crate::tui::state::Tab;
        state.tab = Tab::Login;

        let backend = ratatui::backend::TestBackend::new(64, 16);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                let area = f.area();
                render_login(&state, None, f, area);
            })
            .unwrap();
        insta::assert_snapshot!(terminal.backend());
    }

    #[test]
    fn render_login_snapshot() {
        // Sem providers no state (fetch nunca rodou): todos deslogados.
        let mut state = AppState::new();
        state.login_selected = 1; // Codex selecionado
        use crate::tui::state::Tab;
        state.tab = Tab::Login;

        let backend = ratatui::backend::TestBackend::new(64, 16);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                let area = f.area();
                render_login(&state, None, f, area);
            })
            .unwrap();
        insta::assert_snapshot!(terminal.backend());
    }

    #[test]
    fn render_login_snapshot_with_status() {
        let mut state = AppState::new();
        state.login_selected = 0;
        state.login_status = Some("Erro no login: claude nao encontrado".to_string());
        use crate::tui::state::Tab;
        state.tab = Tab::Login;

        let backend = ratatui::backend::TestBackend::new(64, 16);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                let area = f.area();
                render_login(&state, None, f, area);
            })
            .unwrap();
        insta::assert_snapshot!(terminal.backend());
    }
}
