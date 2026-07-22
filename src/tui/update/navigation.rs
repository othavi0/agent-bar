//! Navegação de teclado/mouse, sidebar e ações globais de tela.

use ratatui::crossterm::event::{KeyCode, KeyEvent};

use crate::tui::action::Action;
use crate::tui::mouse::{ChipKind, MouseTarget};
use crate::tui::state::{sidebar_items, AppState, Screen, SidebarItem};

use super::login::login_selected_id;

/// Translates a KeyEvent into a semantic Action using current screen state:
/// up/down move the sidebar cursor, Enter/h/g/w activate a sidebar item
/// (jumping directly to Detail/History/Login/Waybar). Esc goes back to
/// Detail from History/Login/Waybar (Task 11: Detail é a tela default —
/// Esc nela é no-op, ver o match genérico abaixo). Some screens (Waybar
/// editing, Login list) intercept keys with their own local semantics
/// before falling through to this navigation.
pub(super) fn key_to_action_with_state(key: KeyEvent, state: &AppState) -> Option<Action> {
    // Se o overlay de ajuda esta aberto, qualquer tecla fecha (Esc ou '?').
    if state.show_help {
        return match key.code {
            KeyCode::Esc | KeyCode::Char('?') | KeyCode::Char('q') => Some(Action::ToggleHelp),
            _ => None,
        };
    }

    // '?' global: abre o overlay de ajuda de qualquer contexto (exceto edicao).
    let in_config_edit = state.screen == Screen::Waybar
        && state
            .config_state
            .as_ref()
            .map(|cs| cs.editing)
            .unwrap_or(false);

    if key.code == KeyCode::Char('?') && !in_config_edit {
        return Some(Action::ToggleHelp);
    }

    // Na tela Waybar com campo em edicao, delega ao input buffer (so Esc/Enter escapam).
    if state.screen == Screen::Waybar {
        if let Some(cs) = &state.config_state {
            if cs.editing {
                return match key.code {
                    KeyCode::Esc => Some(Action::ConfigCancelEdit),
                    KeyCode::Enter => Some(Action::ConfigConfirmEdit),
                    _ => None, // o event_loop vai passar o evento cru ao Input
                };
            }
        }
        // Tela Waybar, fora do modo edicao. h/g/w saltam direto para outra
        // tela (substituem o antigo ←→ de troca de aba — sem conflito com
        // j/k/Enter/s/Esc/q, ja usados aqui).
        return match key.code {
            KeyCode::Char('j') | KeyCode::Down => Some(Action::ConfigDown),
            KeyCode::Char('k') | KeyCode::Up => Some(Action::ConfigUp),
            KeyCode::Enter => Some(Action::ConfigEnterEdit),
            KeyCode::Char('s') => Some(Action::SaveConfig),
            KeyCode::Esc => Some(Action::Back),
            KeyCode::Char('h') => Some(Action::Activate(SidebarItem::History)),
            KeyCode::Char('g') => Some(Action::Activate(SidebarItem::Login)),
            KeyCode::Char('w') => Some(Action::Activate(SidebarItem::Waybar)),
            KeyCode::Char('q') => Some(Action::Quit),
            _ => None,
        };
    }

    // Tela Login: navegacao e acao de login. h/w saltam direto para outra
    // tela (mesmo racional da tela Waybar acima).
    if state.screen == Screen::Login {
        return match key.code {
            KeyCode::Char('j') | KeyCode::Down => Some(Action::LoginDown),
            KeyCode::Char('k') | KeyCode::Up => Some(Action::LoginUp),
            KeyCode::Enter => Some(Action::LoginRequested(
                login_selected_id(state.login_selected).to_string(),
            )),
            KeyCode::Esc => Some(Action::Back),
            KeyCode::Char('h') => Some(Action::Activate(SidebarItem::History)),
            KeyCode::Char('g') => Some(Action::Activate(SidebarItem::Login)),
            KeyCode::Char('w') => Some(Action::Activate(SidebarItem::Waybar)),
            KeyCode::Char('q') => Some(Action::Quit),
            _ => None,
        };
    }

    // Tela History: navega a lista de dias (j/k/Enter) + alterna o range do
    // chart (t) — mesmo racional da tela Login acima (h/g/w saltam direto
    // pra outra tela; j/k/Enter aqui SUBSTITUEM a navegação genérica de
    // sidebar/Detail, exatamente como LoginUp/LoginDown/LoginRequested
    // substituem Down/Up/OpenDetail na tela Login).
    if state.screen == Screen::History {
        return match key.code {
            KeyCode::Char('j') | KeyCode::Down => Some(Action::HistoryDown),
            KeyCode::Char('k') | KeyCode::Up => Some(Action::HistoryUp),
            KeyCode::Enter => Some(Action::HistoryToggleDay),
            KeyCode::Char('t') => Some(Action::ToggleHistoryRange),
            KeyCode::Char('r') => Some(Action::Refresh),
            KeyCode::Esc => Some(Action::Back),
            KeyCode::Char('h') => Some(Action::Activate(SidebarItem::History)),
            KeyCode::Char('g') => Some(Action::Activate(SidebarItem::Login)),
            KeyCode::Char('w') => Some(Action::Activate(SidebarItem::Waybar)),
            KeyCode::Char('q') => Some(Action::Quit),
            _ => None,
        };
    }

    // Tela Detail (default, sem bloco dedicado acima): Esc é no-op de
    // propósito (Task 11) — Detail é a base da navegação, não há "voltar"
    // daqui (o overlay de ajuda já intercepta Esc antes de chegar aqui).
    match key.code {
        KeyCode::Char('j') | KeyCode::Down => Some(Action::Down),
        KeyCode::Char('k') | KeyCode::Up => Some(Action::Up),
        KeyCode::Enter => Some(Action::OpenDetail),
        KeyCode::Char('h') => Some(Action::Activate(SidebarItem::History)),
        KeyCode::Char('g') => Some(Action::Activate(SidebarItem::Login)),
        KeyCode::Char('w') => Some(Action::Activate(SidebarItem::Waybar)),
        KeyCode::Char('r') => Some(Action::Refresh),
        KeyCode::Char('q') => Some(Action::Quit),
        _ => None,
    }
}

pub(super) fn up(state: &mut AppState) -> Vec<Action> {
    state.sidebar_selected = state.sidebar_selected.saturating_sub(1);
    vec![]
}

pub(super) fn down(state: &mut AppState) -> Vec<Action> {
    let max = sidebar_items(state.providers.len()).len() - 1;
    state.sidebar_selected = (state.sidebar_selected + 1).min(max);
    vec![]
}

pub(super) fn open_detail(state: &mut AppState) -> Vec<Action> {
    // Recursa (mesmo padrao do braço Action::Key): aplica a ativacao
    // na hora e propaga os follow-ups dela (ex. InitConfig ao ativar
    // Waybar), em vez de so devolver Activate sem aplicar.
    let items = sidebar_items(state.providers.len());
    match items.get(state.sidebar_selected).copied() {
        Some(item) => super::update(state, Action::Activate(item)),
        None => vec![],
    }
}

pub(super) fn activate(state: &mut AppState, item: SidebarItem) -> Vec<Action> {
    let old_screen = state.screen;
    let follow_ups = match item {
        SidebarItem::Provider(i) => {
            state.selected = i;
            state.screen = Screen::Detail;
            vec![]
        }
        SidebarItem::History => {
            state.screen = Screen::History;
            vec![]
        }
        SidebarItem::Login => {
            state.screen = Screen::Login;
            vec![]
        }
        SidebarItem::Waybar => {
            state.screen = Screen::Waybar;
            // Inicializa config_state ao entrar na tela Waybar pela 1a vez.
            // Nao podemos acessar settings aqui (update e puro), entao sinalizamos
            // com um placeholder — o event_loop (drain) sobrescreve com as
            // settings reais ao interceptar InitConfig (mesmo mecanismo do
            // antigo SwitchTab para a aba Waybar).
            if state.config_state.is_none() {
                vec![Action::InitConfig(crate::settings::Settings {
                    version: 0, // sentinela; event_loop sobrescreve com real
                    waybar: crate::settings::Waybar {
                        providers: vec![],
                        separators: crate::settings::SeparatorStyle::Gap,
                        provider_order: vec![],
                        display_mode: crate::settings::DisplayMode::Remaining,
                        signal: None,
                        interval: 60,
                    },
                    tooltip: crate::settings::Tooltip {},
                    models: Default::default(),
                    window_policy: Default::default(),
                    notify: crate::settings::Notify { enabled: true },
                    cache: crate::settings::CacheSettings {
                        ttl: Default::default(),
                    },
                    menu: crate::settings::MenuSettings {
                        animations: true,
                        font_family: "IBM Plex Mono".to_string(),
                        font_size: 12,
                    },
                    glyph_mode: crate::settings::GlyphMode::Box,
                    fx_rate: 5.50,
                })]
            } else {
                vec![]
            }
        }
    };
    // Sincroniza o cursor da sidebar com o item ativado — cobre
    // h/g/w, Enter (via OpenDetail) e cliques futuros da Task 9.
    // Sem isso, ativar por atalho deixa sidebar_selected apontando
    // pra outro item quando a Task 10 desenhar o highlight.
    if let Some(idx) = sidebar_items(state.providers.len())
        .iter()
        .position(|i| *i == item)
    {
        state.sidebar_selected = idx;
    }
    // `state.scroll` é compartilhado entre telas (ScrollView do
    // Overview e, agora, a tabela do History) — sem reset, uma
    // posição de scroll deixada numa tela (ex. Overview rolado)
    // vaza pra outra tela sem relação nenhuma com aquele offset
    // (History abriria com linhas de dado já puladas). Reseta
    // só quando a tela de fato muda, não quando reativa a que
    // já está ativa (ex. Enter em cima de si mesmo).
    if state.screen != old_screen {
        state.scroll = 0;
    }
    follow_ups
}

pub(super) fn select_sidebar(state: &mut AppState, i: usize) -> Vec<Action> {
    state.sidebar_selected = i;
    vec![]
}

pub(super) fn back(state: &mut AppState) -> Vec<Action> {
    // Mesma regra do braço Activate: mudança de tela zera o scroll
    // (compartilhado entre Detail/History) — só quando a screen de
    // fato muda, senão Esc numa tela que já é Detail zeraria o
    // scroll do usuário à toa. Task 11: Detail (não mais Overview) é
    // o destino — Overview morreu, Detail é a tela default/home.
    if state.screen != Screen::Detail {
        state.scroll = 0;
    }
    state.screen = Screen::Detail;
    vec![]
}

pub(super) fn quit(state: &mut AppState) -> Vec<Action> {
    state.should_quit = true;
    vec![]
}

pub(super) fn toggle_help(state: &mut AppState) -> Vec<Action> {
    state.show_help = !state.show_help;
    vec![]
}

pub(super) fn tick() -> Vec<Action> {
    vec![]
}

pub(super) fn anim_tick(state: &mut AppState) -> Vec<Action> {
    // (A antiga "Animação A" — gauge lerp via display_ratio — foi
    // removida: nenhum render lia o valor.)
    // Animação C (throbber): avança o frame do spinner braille.
    state.throbber.advance();
    // Animação D (pulse): contador de frames para blink do ● crítico
    // da sidebar (Task 10). O pulse dos gauges críticos do
    // card/detalhe morreu em v8 (spec §6, `pulse_color` removido) —
    // gauge agora é sólido, sem modulação de brilho.
    state.anim_frame = state.anim_frame.wrapping_add(1);
    // Count-up do custo do header (T16): ease exponencial (~800ms
    // em ticks de 30ms, fator 0.12). animations=false → snapa
    // direto pro alvo, sem lerp visual.
    let target_cost = state
        .usage
        .as_ref()
        .map(|u| u.total_cost.usd)
        .unwrap_or(0.0);
    if state.animations {
        state.display_cost += (target_cost - state.display_cost) * 0.12;
        if (target_cost - state.display_cost).abs() < 0.01 {
            state.display_cost = target_cost;
        }
    } else {
        state.display_cost = target_cost;
    }
    vec![]
}

/// Mouse (Task 9).
/// Recursa via update() em vez de so devolver a action mapeada como
/// follow-up (mesmo padrao de Action::Key/Action::OpenDetail acima):
/// aplica o efeito NA HORA. Alem de manter clique e teclado simetricos
/// (ex. o Refresh clicado passa pelo MESMO guard anti-fetch-duplicado
/// que o Refresh da tecla [r] — devolver a action crua ignoraria o
/// guard, ja que drain() intercepta Action::Refresh direto sem
/// reentrar no update), a sincronicidade e o que os testes observam.
pub(super) fn click(state: &mut AppState, target: MouseTarget) -> Vec<Action> {
    match target {
        MouseTarget::Sidebar(i) => {
            // sidebar_items() e a unica fonte de verdade dos indices validos
            // (SelectSidebar nao tem bounds-check); ignora i fora de faixa em
            // vez de deixar sidebar_selected apontando pra um item inexistente.
            let items = sidebar_items(state.providers.len());
            match items.get(i).copied() {
                Some(item) => {
                    state.sidebar_selected = i;
                    super::update(state, Action::Activate(item))
                }
                None => vec![],
            }
        }
        // Na tela Login, os itens da lista de providers TAMBÉM registram
        // MouseTarget::Card(i) (mesma linguagem visual dos cards do
        // Overview) — mas aqui o clique so SELECIONA (mesmo efeito de
        // LoginUp/LoginDown), nunca ativa Detail. Reusar o braço
        // genérico abaixo (Activate(Provider(i))) navegaria pra fora da
        // tela Login no primeiro clique, contrariando "click seleciona;
        // ativação continua pelo Enter/chip" (T14).
        MouseTarget::Card(i) if state.screen == Screen::Login => {
            // 4 providers na lista Login (claude/codex/amp/grok).
            if i < 4 {
                state.login_selected = i;
                state.login_status = None;
            }
            vec![]
        }
        // Cards eram do Overview/dashboard (Task 11: ambos apagados) —
        // fora da tela Login (guard acima), Card não tem mais nenhum
        // efeito (a lista de providers da Detail é a sidebar, não
        // cards clicáveis).
        MouseTarget::Card(_) => vec![],
        MouseTarget::Chip(ChipKind::Open) => super::update(state, Action::OpenDetail),
        MouseTarget::Chip(ChipKind::Refresh) => super::update(state, Action::Refresh),
        MouseTarget::Chip(ChipKind::Help) => super::update(state, Action::ToggleHelp),
        MouseTarget::Chip(ChipKind::Quit) => super::update(state, Action::Quit),
        MouseTarget::Chip(ChipKind::Back) => super::update(state, Action::Back),
        MouseTarget::Chip(ChipKind::Login) => {
            super::update(state, Action::Activate(SidebarItem::Login))
        }
        MouseTarget::Chip(ChipKind::History) => {
            super::update(state, Action::Activate(SidebarItem::History))
        }
        MouseTarget::Chip(ChipKind::ToggleRange) => {
            super::update(state, Action::ToggleHistoryRange)
        }
        MouseTarget::Chip(ChipKind::ExpandDay) => super::update(state, Action::HistoryToggleDay),
        // Chip "iniciar login" da tela Login: dispara a MESMA action que
        // o Enter dispara lá (Action::LoginRequested pro provider
        // selecionado) — nunca Activate(Login), que seria no-op (a
        // tela já está ativa) e ignoraria o clique (T14).
        MouseTarget::Chip(ChipKind::StartLogin) => super::update(
            state,
            Action::LoginRequested(login_selected_id(state.login_selected).to_string()),
        ),
        MouseTarget::Chip(ChipKind::EnterEdit) => super::update(state, Action::ConfigEnterEdit),
        MouseTarget::Chip(ChipKind::SaveConfig) => super::update(state, Action::SaveConfig),
    }
}

pub(super) fn hover(state: &mut AppState, t: Option<MouseTarget>) -> Vec<Action> {
    state.hover = t;
    vec![]
}

pub(super) fn scroll(state: &mut AppState, delta: i32) -> Vec<Action> {
    // saturating_add_signed ja satura em 0 (limite inferior de u16) —
    // .max(0) e redundante (clippy::unnecessary_min_or_max).
    state.scroll = state.scroll.saturating_add_signed(delta as i16);
    vec![]
}
