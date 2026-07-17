//! Aba Waybar config: exibe e edita os campos de Settings via tui-input.

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, List, ListItem, Paragraph};
use ratatui::Frame;

use crate::theme::ColorToken;
use crate::tui::mouse::{ChipKind, HitMap};
use crate::tui::state::{AppState, ConfigField, ConfigState};
use crate::tui::theme_bridge::to_ratatui;
use crate::tui::widgets::chips::{chips_line, register_chip_hits};

/// Renders a aba Waybar config.
pub fn render_config(state: &AppState, frame: &mut Frame, area: Rect, hits: &mut HitMap) {
    // Layout: [field_list | detail_panel]
    let horiz = Layout::default()
        .direction(Direction::Horizontal)
        // Lista um pouco mais larga p/ rótulos humanos (trilha C).
        .constraints([Constraint::Length(22), Constraint::Min(0)])
        .split(area);

    let list_area = horiz[0];
    let detail_area = horiz[1];

    match &state.config_state {
        None => {
            // Ainda nao inicializado (primeira entrada na aba)
            let block = Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(to_ratatui(ColorToken::Comment)));
            let p = Paragraph::new(Span::styled(
                " Carregando config...",
                Style::default().fg(to_ratatui(ColorToken::Muted)),
            ))
            .block(block);
            frame.render_widget(p, area);
        }
        Some(cs) => {
            render_field_list(cs, frame, list_area);
            render_field_detail(cs, frame, detail_area, hits);
        }
    }
}

/// Lista os campos editaveis na coluna esquerda.
fn render_field_list(cs: &ConfigState, frame: &mut Frame, area: Rect) {
    let selected_style = Style::default()
        .fg(to_ratatui(ColorToken::TextBright))
        .add_modifier(Modifier::BOLD)
        .bg(to_ratatui(ColorToken::SelBg));

    let normal_style = Style::default().fg(to_ratatui(ColorToken::Text));

    let section_style = Style::default()
        .fg(to_ratatui(ColorToken::Comment))
        .add_modifier(Modifier::BOLD);

    // Cabecalhos de secao (T14): WAYBAR agrupa os campos que o Waybar le
    // (Providers..Interval); TUI agrupa o que so afeta este menu (FxRate,
    // por ora). Sao linhas de lista sem indice em ConfigField::ALL — o
    // highlight de selecao compara `i` (indice do campo) com
    // `cs.selected_field`, nunca a posicao visual na lista.
    let mut items: Vec<ListItem<'_>> = Vec::new();
    for (i, field) in ConfigField::ALL.iter().enumerate() {
        if i == 0 {
            items.push(ListItem::new(Line::from(Span::styled(
                " WAYBAR",
                section_style,
            ))));
        }
        if *field == ConfigField::FxRate {
            items.push(ListItem::new(Line::from(Span::styled(
                " TUI · afeta só este menu",
                section_style,
            ))));
        }

        let style = if i == cs.selected_field {
            selected_style
        } else {
            normal_style
        };
        let prefix = if i == cs.selected_field { " > " } else { "   " };
        items.push(ListItem::new(Line::from(vec![Span::styled(
            format!("{}{}", prefix, field.label()),
            style,
        )])));
    }

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(to_ratatui(ColorToken::Blue)))
        .title(Span::styled(
            " Config ",
            Style::default()
                .fg(to_ratatui(ColorToken::TextBright))
                .add_modifier(Modifier::BOLD),
        ));

    let list = List::new(items).block(block);
    frame.render_widget(list, area);
}

/// Mostra o valor atual + editor inline no painel direito.
fn render_field_detail(cs: &ConfigState, frame: &mut Frame, area: Rect, hits: &mut HitMap) {
    // Vertical split: [value_row (3), help_row (1 fill)]
    let vert = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(area);

    let value_area = vert[0];
    let help_area = vert[1];

    let field = ConfigField::ALL[cs.selected_field];

    // Calcula o valor a exibir: se editando, usa o buffer; senao o valor atual.
    let display_value = if cs.editing {
        cs.input.value().to_string()
    } else {
        field_current_value(field, cs)
    };

    let value_style = if cs.editing {
        Style::default()
            .fg(to_ratatui(ColorToken::Yellow))
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(to_ratatui(ColorToken::Green))
    };

    let border_color = if cs.editing {
        to_ratatui(ColorToken::Yellow)
    } else {
        to_ratatui(ColorToken::Blue)
    };

    let edit_indicator = if cs.editing { " [editando] " } else { "" };

    let value_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border_color))
        .title(Span::styled(
            format!(" {} {} ", field.label(), edit_indicator),
            Style::default()
                .fg(to_ratatui(ColorToken::TextBright))
                .add_modifier(Modifier::BOLD),
        ));

    let value_paragraph =
        Paragraph::new(Span::styled(format!(" {}", display_value), value_style)).block(value_block);

    frame.render_widget(value_paragraph, value_area);

    // Painel de ajuda + status
    render_help_and_status(cs, frame, help_area, hits);
}

/// Retorna o valor atual do campo como string.
fn field_current_value(field: ConfigField, cs: &ConfigState) -> String {
    let s = &cs.edit_settings;
    match field {
        ConfigField::Providers => s.waybar.providers.join(", "),
        ConfigField::ProviderOrder => s.waybar.provider_order.join(", "),
        ConfigField::Separators => format!("{:?}", s.waybar.separators).to_lowercase(),
        ConfigField::DisplayMode => format!("{:?}", s.waybar.display_mode).to_lowercase(),
        ConfigField::FxRate => format!("{:.2}", s.fx_rate),
        ConfigField::Signal => s
            .waybar
            .signal
            .map(|n| n.to_string())
            .unwrap_or_else(|| "none".to_string()),
        ConfigField::Interval => s.waybar.interval.to_string(),
    }
}

/// Painel de ajuda e mensagem de status. Rodapé: chips `[↵ editar] [s
/// salvar] [esc voltar]` substituindo o hint-text antigo (T14) — só fora do
/// modo edição (durante edição o campo tem foco do `tui_input`; os chips
/// dariam a entender que o clique funciona ali, o que não é verdade).
fn render_help_and_status(cs: &ConfigState, frame: &mut Frame, area: Rect, hits: &mut HitMap) {
    let muted = Style::default().fg(to_ratatui(ColorToken::Muted));
    let comment = Style::default().fg(to_ratatui(ColorToken::Comment));

    let mut lines: Vec<Line<'_>> = Vec::new();

    // Dica de campo
    let field = ConfigField::ALL[cs.selected_field];
    if let Some(hint) = field_hint(field) {
        lines.push(Line::from(Span::styled(format!(" {}", hint), comment)));
    }

    // Instrucoes de navegacao (só em edição — fora dela, os chips do rodapé
    // cobrem editar/salvar/voltar).
    if cs.editing {
        lines.push(Line::from(Span::styled(
            " Enter confirma   Esc cancela",
            muted,
        )));
    }

    // Mensagem de status (erro / confirmacao)
    if let Some(msg) = &cs.status_msg {
        let status_style = if msg.starts_with("Erro") || msg.starts_with("erro") {
            Style::default().fg(to_ratatui(ColorToken::Red))
        } else {
            Style::default().fg(to_ratatui(ColorToken::Green))
        };
        lines.push(Line::from(vec![]));
        lines.push(Line::from(Span::styled(format!(" {}", msg), status_style)));
    }

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(to_ratatui(ColorToken::Comment)));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let vert = Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).split(inner);
    frame.render_widget(Paragraph::new(lines), vert[0]);

    if !cs.editing {
        let chips: [(ChipKind, &str, &str); 3] = [
            (ChipKind::EnterEdit, "\u{21b5}", "editar"),
            (ChipKind::SaveConfig, "s", "salvar"),
            (ChipKind::Back, "esc", "voltar"),
        ];
        let chips_area = vert[1];
        let line = chips_line(&chips, chips_area.width);
        frame.render_widget(Paragraph::new(line), chips_area);
        register_chip_hits(&chips, chips_area, hits);
    }
}

/// Dica por campo (None = sem dica especifica). Inclui a chave técnica
/// entre parênteses pra quem edita settings.json à mão.
fn field_hint(field: ConfigField) -> Option<&'static str> {
    match field {
        ConfigField::Providers => {
            Some("Quais providers aparecem na barra. Ex: claude, codex, amp, grok (providers)")
        }
        ConfigField::ProviderOrder => {
            Some("Ordem dos módulos no Waybar. Ex: claude, codex (providerOrder)")
        }
        ConfigField::Separators => {
            Some("Estilo entre módulos: pill / gap / bare / glass / shadow / none (separators)")
        }
        ConfigField::DisplayMode => {
            Some("Na barra: remaining = % restante · used = % usado (displayMode)")
        }
        ConfigField::Signal => Some(
            "Sinal p/ refresh externo (pkill -SIGRTMIN+N waybar). \
             O agent-bar não dispara sozinho (signal)",
        ),
        ConfigField::Interval => Some("Poll da barra em segundos. Ex: 60 (interval)"),
        ConfigField::FxRate => Some("US$ → R$ nos custos da TUI. Ex: 5.75 (fxRate)"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::{
        CacheSettings, DisplayMode, GlyphMode, MenuSettings, Notify, SeparatorStyle, Settings,
        Tooltip, Waybar,
    };
    use crate::tui::state::{AppState, ConfigState, Screen};
    use std::collections::BTreeMap;

    fn fake_settings() -> Settings {
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

    fn state_on_waybar() -> AppState {
        let settings = fake_settings();
        let mut state = AppState::new();
        state.screen = Screen::Waybar;
        state.config_state = Some(ConfigState::new(&settings));
        state
    }

    #[test]
    fn config_renders_without_panic() {
        let backend = ratatui::backend::TestBackend::new(64, 20);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        let state = state_on_waybar();
        terminal
            .draw(|f| {
                render_config(&state, f, f.area(), &mut HitMap::default());
            })
            .unwrap();
        // Se chegou aqui sem panico, o render esta basico OK.
    }

    #[test]
    fn config_snapshot() {
        let backend = ratatui::backend::TestBackend::new(64, 20);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        let state = state_on_waybar();
        terminal
            .draw(|f| render_config(&state, f, f.area(), &mut HitMap::default()))
            .unwrap();
        insta::assert_snapshot!(terminal.backend());
    }

    #[test]
    fn config_snapshot_editing() {
        let backend = ratatui::backend::TestBackend::new(64, 20);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        let mut state = state_on_waybar();
        if let Some(cs) = state.config_state.as_mut() {
            // Busca por variante, nao por posicao fixa (Task 14 moveu FxRate
            // pro fim de ConfigField::ALL).
            cs.selected_field = ConfigField::ALL
                .iter()
                .position(|f| *f == ConfigField::FxRate)
                .expect("FxRate deve estar em ConfigField::ALL");
            cs.editing = true;
            cs.input = tui_input::Input::new("6.25".to_string());
        }
        terminal
            .draw(|f| render_config(&state, f, f.area(), &mut HitMap::default()))
            .unwrap();
        insta::assert_snapshot!(terminal.backend());
    }

    #[test]
    fn config_snapshot_with_status_msg() {
        let backend = ratatui::backend::TestBackend::new(64, 20);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        let mut state = state_on_waybar();
        if let Some(cs) = state.config_state.as_mut() {
            cs.status_msg = Some("Configuração salva e Waybar recarregado.".to_string());
        }
        terminal
            .draw(|f| render_config(&state, f, f.area(), &mut HitMap::default()))
            .unwrap();
        insta::assert_snapshot!(terminal.backend());
    }

    /// Achata um `Buffer` renderizado em texto puro, uma linha por row
    /// (trailing spaces cortados) — usado pra checar conteúdo textual sem
    /// depender do snapshot Pango-sensível.
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

    #[test]
    fn config_renders_waybar_and_tui_sections() {
        // Largura generosa pra o hint do signal caber inteiro na coluna de
        // detalhe (a lista de campos fica em Length(20) fixo).
        let backend = ratatui::backend::TestBackend::new(140, 32);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        let mut state = state_on_waybar();
        if let Some(cs) = state.config_state.as_mut() {
            // Seleciona o campo Signal pra expor o hint na área de ajuda.
            cs.selected_field = ConfigField::ALL
                .iter()
                .position(|f| *f == ConfigField::Signal)
                .expect("Signal deve estar em ConfigField::ALL");
        }
        terminal
            .draw(|f| render_config(&state, f, f.area(), &mut HitMap::default()))
            .unwrap();
        let text = buffer_to_string(terminal.backend().buffer());

        assert!(text.contains("WAYBAR"), "seção WAYBAR ausente:\n{text}");
        assert!(text.contains("TUI"), "seção TUI ausente:\n{text}");
        assert!(
            text.contains("refresh externo") && text.contains("agent-bar não"),
            "hint do signal ausente:\n{text}"
        );
    }
}
