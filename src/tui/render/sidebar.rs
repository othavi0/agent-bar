//! Sidebar: PROVEDORES / MAIS. Sem tabs — este é o hub de navegação.
//! (Task 11: a seção VISÃO/Overview morreu — Detail agora é a tela default.)

use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::theme::{provider_hex, ColorToken};
use crate::tui::mouse::{HitMap, MouseTarget};
use crate::tui::state::{sidebar_items, AppState, SidebarItem};
use crate::tui::theme_bridge::to_ratatui;

/// Largura mínima pra caber o rótulo completo (mark + nome + %). Abaixo
/// disso (sidebar colapsada em Length(3)) só a marca ◆●● é exibida.
const NARROW_THRESHOLD: u16 = 6;

fn item_label(state: &AppState, item: SidebarItem, narrow: bool) -> Line<'static> {
    if narrow {
        return match item {
            SidebarItem::Provider(i) => {
                let pv = &state.providers[i];
                let mark = if pv.quota.provider == "claude" {
                    "◆"
                } else {
                    "●"
                };
                Line::from(format!(" {mark}"))
            }
            // ratatui pula `buf.set_style` inteiro quando a Line tem
            // largura 0 (ver `ratatui_core::text::Line::render_with_alignment`)
            // — uma Line vazia nunca pinta o bg de seleção. Por isso cada
            // item ganha ao menos 1 glifo em vez de string vazia: senão o
            // cursor fica invisível ao navegar até History/Login/Waybar
            // com a sidebar colapsada.
            SidebarItem::History => Line::from(" H".to_string()),
            SidebarItem::Login => Line::from(" L".to_string()),
            SidebarItem::Waybar => Line::from(" C".to_string()),
        };
    }
    match item {
        SidebarItem::Provider(i) => {
            let pv = &state.providers[i];
            let mark = if pv.quota.provider == "claude" {
                "◆"
            } else {
                "●"
            };
            let pct = pv
                .quota
                .primary
                .as_ref()
                .map(|w| format!("{:>3.0}%", w.remaining))
                .unwrap_or_else(|| "  – ".to_string());
            // Coluna de nome tem largura fixa 7 — nunca corte seco: nomes
            // maiores truncam com "…" em vez de estourar a coluna.
            let name_trunc = truncate_name(&pv.quota.display_name, 7);
            Line::from(format!(" {mark} {:<7}{pct}", name_trunc))
        }
        SidebarItem::History => Line::from("   Histórico".to_string()),
        SidebarItem::Login => Line::from("   Login".to_string()),
        SidebarItem::Waybar => Line::from("   Config".to_string()),
    }
}

/// Trunca `name` para no máximo `width` células, terminando em "…" quando
/// corta — contrato de alinhamento: nunca corte seco.
fn truncate_name(name: &str, width: usize) -> String {
    if name.chars().count() <= width {
        return name.to_string();
    }
    let head: String = name.chars().take(width.saturating_sub(1)).collect();
    format!("{head}…")
}

pub fn render_sidebar(state: &AppState, frame: &mut Frame, area: Rect, hits: &mut HitMap) {
    let narrow = area.width < NARROW_THRESHOLD;
    let items = sidebar_items(state.providers.len());
    let mut lines: Vec<Line> = Vec::new();
    let mut row_of_item: Vec<u16> = Vec::new();

    for (i, item) in items.iter().enumerate() {
        // Cabeçalhos de seção antes do primeiro item de cada grupo. Em modo
        // estreito viram linha em branco (sem texto legível em 3 colunas) —
        // preserva a contagem de linhas (e portanto o offset de cada item)
        // idêntica ao modo largo.
        match item {
            SidebarItem::Provider(0) => {
                lines.push(Line::from(""));
                lines.push(section(" PROVEDORES", narrow));
            }
            SidebarItem::History => {
                lines.push(Line::from(""));
                lines.push(section(" MAIS", narrow));
            }
            _ => {}
        }
        let mut line = item_label(state, *item, narrow);
        let selected = state.sidebar_selected == i;
        let hovered = state.hover == Some(MouseTarget::Sidebar(i));
        let style = if selected {
            Style::default()
                .bg(to_ratatui(ColorToken::SelBg))
                .add_modifier(Modifier::BOLD)
        } else if hovered {
            Style::default().bg(to_ratatui(ColorToken::Surface))
        } else {
            Style::default()
        };
        line = line.style(style.fg(item_color(state, *item)));
        row_of_item.push(area.y + lines.len() as u16);
        lines.push(line);
    }

    // Guard de altura: itens além de `area.height` são clipados pelo
    // Paragraph e não ficam visíveis — não registrar zona de clique pra
    // linha que não está na tela (regressão vs a sidebar antiga, que já
    // parava em `inner.height`).
    let max_row = area.y + area.height;
    for (i, row) in row_of_item.iter().enumerate() {
        if *row >= max_row {
            continue;
        }
        hits.push(
            Rect::new(area.x, *row, area.width, 1),
            MouseTarget::Sidebar(i),
        );
    }
    frame.render_widget(Paragraph::new(lines), area);
}

fn section(label: &str, narrow: bool) -> Line<'static> {
    if narrow {
        return Line::from("");
    }
    Line::from(Span::styled(
        label.to_string(),
        Style::default()
            .fg(to_ratatui(ColorToken::Comment))
            .add_modifier(Modifier::BOLD),
    ))
}

fn item_color(state: &AppState, item: SidebarItem) -> ratatui::style::Color {
    match item {
        SidebarItem::Provider(i) => {
            let pv = &state.providers[i];
            if pv.quota.error.is_some() {
                to_ratatui(ColorToken::Muted) // deslogado/erro: dim
            } else {
                let remaining = pv
                    .quota
                    .primary
                    .as_ref()
                    .map(|w| w.remaining)
                    .unwrap_or(0.0);
                if remaining < 10.0 {
                    // Animação D: blink crítico da MARCA da sidebar. Migrado
                    // de widgets/provider_list.rs (órfão desta task,
                    // apagado): mesma cadência de ~450ms (ticks de 30ms →
                    // 15 ticks por fase). O pulso dos GAUGES do card/detalhe
                    // (Task 16, `widgets::quota_gauge::pulse_color`) morreu
                    // em v8 (spec §6, gauge sólido) — este blink da sidebar
                    // não foi substituído e continua intacto.
                    // spec §8: `animations=false` desativa TUDO.
                    // Com animações off, cor estática Red (não Muted: o
                    // crítico não pode "sumir" só porque não pisca).
                    if !state.animations {
                        to_ratatui(ColorToken::Red)
                    } else {
                        let blink_visible = (state.anim_frame / 15).is_multiple_of(2);
                        if blink_visible {
                            to_ratatui(ColorToken::Red)
                        } else {
                            to_ratatui(ColorToken::Muted)
                        }
                    }
                } else {
                    crate::tui::theme_bridge::hex_to_color(provider_hex(&pv.quota.provider))
                }
            }
        }
        _ => to_ratatui(ColorToken::Text),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::types::{ProviderQuota, QuotaWindow};
    use crate::tui::state::ProviderView;

    fn make_provider(id: &str, display: &str, remaining: f64) -> ProviderView {
        ProviderView::new(ProviderQuota {
            provider: id.to_string(),
            display_name: display.to_string(),
            available: true,
            account: None,
            plan: None,
            plan_type: None,
            primary: Some(QuotaWindow {
                remaining,
                resets_at: None,
                window_minutes: Some(300),
                used: Some(100.0 - remaining),
                severity: None,
            }),
            secondary: None,
            models: None,
            extra: None,
            error: None,
        })
    }

    #[test]
    fn render_sidebar_registers_all_items_1to1_with_cursor() {
        let backend = ratatui::backend::TestBackend::new(17, 20);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        let mut state = AppState::new();
        state.providers = vec![
            make_provider("claude", "Claude", 26.0),
            make_provider("codex", "Codex", 1.0),
        ];
        let mut hits = HitMap::default();
        let items_len = sidebar_items(state.providers.len()).len();
        terminal
            .draw(|f| {
                let area = f.area();
                render_sidebar(&state, f, area, &mut hits);
            })
            .unwrap();

        // Cada indice logico 0..items_len tem uma zona registrada (1:1).
        for i in 0..items_len {
            let found = (0..20u16).any(|y| hits.at(0, y) == Some(MouseTarget::Sidebar(i)));
            assert!(found, "faltou hit-zone para indice {i}");
        }
    }

    #[test]
    fn narrow_sidebar_does_not_panic_and_keeps_same_row_count() {
        let backend = ratatui::backend::TestBackend::new(3, 20);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        let mut state = AppState::new();
        state.providers = vec![make_provider("claude", "Claude", 26.0)];
        let mut hits = HitMap::default();
        let items_len = sidebar_items(state.providers.len()).len();
        terminal
            .draw(|f| {
                let area = f.area();
                render_sidebar(&state, f, area, &mut hits);
            })
            .unwrap();
        for i in 0..items_len {
            let found = (0..20u16).any(|y| hits.at(0, y) == Some(MouseTarget::Sidebar(i)));
            assert!(found, "faltou hit-zone (narrow) para indice {i}");
        }
    }

    #[test]
    fn critical_quota_blinks_red_then_muted() {
        // Animação D: remaining < 10% pisca — migrado de provider_list.rs
        // (órfão, apagado). anim_frame=0 -> fase visível (Red); anim_frame=15
        // -> fase apagada (Muted). Cadência: 15 ticks de 30ms por fase.
        let mut state = AppState::new();
        state.providers = vec![make_provider("claude", "Claude", 5.0)];

        state.anim_frame = 0;
        let visible = item_color(&state, SidebarItem::Provider(0));
        assert_eq!(visible, to_ratatui(ColorToken::Red));

        state.anim_frame = 15;
        let dim = item_color(&state, SidebarItem::Provider(0));
        assert_eq!(dim, to_ratatui(ColorToken::Muted));

        assert_ne!(visible, dim);
    }

    #[test]
    fn critical_quota_static_when_animations_off() {
        // spec §8: animations=false desativa TUDO — o blink crítico da
        // sidebar precisa respeitar o mesmo gate do pulso dos gauges
        // (dashboard.rs/detail.rs). Cor estática Red (não Muted: o
        // crítico não pode sumir só porque a animação está off).
        let mut state = AppState::new();
        state.providers = vec![make_provider("claude", "Claude", 5.0)];
        state.animations = false;

        state.anim_frame = 0;
        let frame0 = item_color(&state, SidebarItem::Provider(0));
        state.anim_frame = 15;
        let frame15 = item_color(&state, SidebarItem::Provider(0));

        assert_eq!(frame0, to_ratatui(ColorToken::Red));
        assert_eq!(frame15, to_ratatui(ColorToken::Red));
        assert_eq!(frame0, frame15, "sem animação, a cor não pode variar");

        // Com animations=true (comportamento atual), os frames continuam
        // diferentes — não quebrar o blink existente.
        state.animations = true;
        state.anim_frame = 0;
        let anim_frame0 = item_color(&state, SidebarItem::Provider(0));
        state.anim_frame = 15;
        let anim_frame15 = item_color(&state, SidebarItem::Provider(0));
        assert_ne!(anim_frame0, anim_frame15);
    }

    #[test]
    fn non_critical_quota_uses_provider_color_regardless_of_anim_frame() {
        let mut state = AppState::new();
        state.providers = vec![make_provider("claude", "Claude", 50.0)];
        state.anim_frame = 0;
        let color = item_color(&state, SidebarItem::Provider(0));
        assert_eq!(
            color,
            crate::tui::theme_bridge::hex_to_color(provider_hex("claude"))
        );
    }

    #[test]
    fn hit_zones_respect_height_guard() {
        // area.height=3: só cabe o header PROVEDORES (2 linhas: em branco +
        // rótulo) + Provider(0) — o resto (History/Login/Waybar) fica fora
        // da tela — não deve registrar zona de clique pra linha invisível.
        // (Task 11: sem Overview, Provider(0) é o 1º item da sidebar.)
        let backend = ratatui::backend::TestBackend::new(17, 3);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        let mut state = AppState::new();
        state.providers = vec![make_provider("claude", "Claude", 26.0)];
        let mut hits = HitMap::default();
        terminal
            .draw(|f| {
                let area = f.area();
                render_sidebar(&state, f, area, &mut hits);
            })
            .unwrap();

        // Provider(0) (indice 0) cai na linha 2, dentro da area de altura 3.
        assert_eq!(hits.at(0, 2), Some(MouseTarget::Sidebar(0)));
        // History (indice 1) cairia na linha 5 — fora da area (0..3).
        for y in 0..3u16 {
            assert_ne!(hits.at(0, y), Some(MouseTarget::Sidebar(1)));
        }
        // Nenhuma zona registrada em y >= height.
        for y in 3..20u16 {
            assert_eq!(hits.at(0, y), None);
        }
    }

    #[test]
    fn long_display_name_truncates_with_ellipsis() {
        let mut state = AppState::new();
        state.providers = vec![make_provider("claude", "VeryLongProviderName", 50.0)];
        let line = item_label(&state, SidebarItem::Provider(0), false);
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(text.contains('…'), "esperava elipse na truncagem: {text}");
        assert!(
            !text.contains("VeryLongProviderName"),
            "nome nao deveria aparecer inteiro: {text}"
        );
    }

    #[test]
    fn render_sidebar_snapshot_critical_blink_visible() {
        // Snapshot deterministico: anim_frame=0 -> fase visivel do blink.
        let backend = ratatui::backend::TestBackend::new(17, 12);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        let mut state = AppState::new();
        state.providers = vec![make_provider("claude", "Claude", 5.0)];
        state.anim_frame = 0;
        let mut hits = HitMap::default();
        terminal
            .draw(|f| {
                let area = f.area();
                render_sidebar(&state, f, area, &mut hits);
            })
            .unwrap();
        insta::assert_snapshot!(terminal.backend());
    }
}
