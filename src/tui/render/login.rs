//! Aba Login: lista os 3 providers com status de autenticacao e acao de login.
//! Sem emoji. Status determinado pelos credential paths e disponibilidade do binario.

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, List, ListItem, Paragraph};
use ratatui::Frame;

use crate::config::Paths;
use crate::theme::ColorToken;
use crate::tui::state::AppState;
use crate::tui::theme_bridge::{provider_color, to_ratatui};

/// Determina se o provider esta autenticado (verificacao local, sem rede).
/// - claude: credentials file existe
/// - codex: auth file existe
/// - amp: binario `amp` acessivel no PATH ou em candidatos conhecidos
pub fn is_logged_in(provider_id: &str, paths: &Paths) -> bool {
    match provider_id {
        "claude" => paths.claude_credentials.exists(),
        "codex" => paths.codex_auth.exists(),
        "amp" => {
            let home = std::env::var("HOME").unwrap_or_default();
            crate::providers::amp_cli::find_amp_bin(&home).is_some()
        }
        _ => false,
    }
}

/// Constantes dos providers da aba Login (id, nome de exibicao).
const PROVIDERS: [(&str, &str); 3] = [("claude", "Claude"), ("codex", "Codex"), ("amp", "Amp")];

/// Renderiza a aba Login completa. Aceita paths opcional; se `None`, resolve
/// via `Paths::from_env()` (aceitavel: stat de arquivo, nao chamada de rede).
pub fn render_login(state: &AppState, paths_opt: Option<&Paths>, frame: &mut Frame, area: Rect) {
    let resolved;
    let paths: &Paths = match paths_opt {
        Some(p) => p,
        None => {
            match Paths::from_env() {
                Ok(p) => {
                    resolved = p;
                    &resolved
                }
                Err(_) => {
                    // Sem HOME: exibe mensagem de erro minima.
                    let block = ratatui::widgets::Block::default()
                        .borders(Borders::ALL)
                        .border_type(BorderType::Thick)
                        .border_style(Style::default().fg(to_ratatui(ColorToken::Comment)));
                    let p = ratatui::widgets::Paragraph::new(Span::styled(
                        " HOME nao definido; nao foi possivel resolver paths.",
                        Style::default().fg(to_ratatui(ColorToken::Red)),
                    ))
                    .block(block);
                    frame.render_widget(p, area);
                    return;
                }
            }
        }
    };

    // Layout: [lista de providers | painel de detalhe/instrucao]
    let horiz = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(22), Constraint::Min(0)])
        .split(area);

    let list_area = horiz[0];
    let detail_area = horiz[1];

    render_provider_list(state, paths, frame, list_area);
    render_detail_panel(state, frame, detail_area);
}

/// Coluna esquerda: lista os providers com status (logado / nao logado).
fn render_provider_list(state: &AppState, paths: &Paths, frame: &mut Frame, area: Rect) {
    let selected_bg = ratatui::style::Color::Rgb(45, 53, 65);

    let items: Vec<ListItem<'_>> = PROVIDERS
        .iter()
        .enumerate()
        .map(|(i, &(id, name))| {
            let logged_in = is_logged_in(id, paths);
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

            let (status_text, status_fg) = if logged_in {
                (" [ok]", to_ratatui(ColorToken::Green))
            } else {
                (" [--]", to_ratatui(ColorToken::Muted))
            };
            let status_style = if selected {
                Style::default()
                    .fg(status_fg)
                    .add_modifier(if logged_in {
                        Modifier::BOLD
                    } else {
                        Modifier::empty()
                    })
                    .bg(selected_bg)
            } else {
                Style::default().fg(status_fg).add_modifier(if logged_in {
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
    use crate::tui::state::AppState;

    fn make_paths_tmp() -> (tempfile::TempDir, Paths) {
        let dir = tempfile::tempdir().unwrap();
        let h = dir.path();
        let paths = Paths {
            cache_dir: h.join("cache/agent-bar"),
            config_dir: h.join("config/agent-bar"),
            claude_credentials: h.join(".claude/.credentials.json"),
            codex_auth: h.join(".codex/auth.json"),
            codex_sessions: h.join(".codex/sessions"),
            amp_settings: h.join("config/amp/settings.json"),
            amp_threads: h.join(".local/share/amp/threads"),
        };
        (dir, paths)
    }

    #[test]
    fn is_logged_in_claude_absent() {
        let (_tmp, paths) = make_paths_tmp();
        assert!(!is_logged_in("claude", &paths));
    }

    #[test]
    fn is_logged_in_claude_present() {
        let (_tmp, paths) = make_paths_tmp();
        std::fs::create_dir_all(paths.claude_credentials.parent().unwrap()).unwrap();
        std::fs::write(&paths.claude_credentials, b"{}").unwrap();
        assert!(is_logged_in("claude", &paths));
    }

    #[test]
    fn is_logged_in_codex_absent() {
        let (_tmp, paths) = make_paths_tmp();
        assert!(!is_logged_in("codex", &paths));
    }

    #[test]
    fn is_logged_in_codex_present() {
        let (_tmp, paths) = make_paths_tmp();
        std::fs::create_dir_all(paths.codex_auth.parent().unwrap()).unwrap();
        std::fs::write(&paths.codex_auth, b"{}").unwrap();
        assert!(is_logged_in("codex", &paths));
    }

    #[test]
    fn is_logged_in_unknown_always_false() {
        let (_tmp, paths) = make_paths_tmp();
        assert!(!is_logged_in("unknown_xyz", &paths));
    }

    #[test]
    fn render_login_snapshot() {
        let (_tmp, paths) = make_paths_tmp();
        // Sem credenciais: todos [--]
        let mut state = AppState::new();
        state.login_selected = 1; // Codex selecionado
        use crate::tui::state::Tab;
        state.tab = Tab::Login;

        let backend = ratatui::backend::TestBackend::new(64, 16);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                let area = f.area();
                render_login(&state, Some(&paths), f, area);
            })
            .unwrap();
        insta::assert_snapshot!(terminal.backend());
    }

    #[test]
    fn render_login_snapshot_with_status() {
        let (_tmp, paths) = make_paths_tmp();
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
                render_login(&state, Some(&paths), f, area);
            })
            .unwrap();
        insta::assert_snapshot!(terminal.backend());
    }
}
