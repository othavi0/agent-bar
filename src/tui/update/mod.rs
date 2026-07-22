//! Pure update: mutates AppState from Action, returns follow-up actions.
//!
//! Split por domínio (navigation / config / login / fetch / history).
//! `pub fn update` é o único entry point público.

mod config;
mod fetch;
mod history;
mod login;
mod navigation;

use crate::tui::action::Action;
use crate::tui::state::AppState;

/// Pure update function: mutates `state` based on `action`, returns follow-up actions.
/// No IO, no spawning, no clocks — fully testable.
pub fn update(state: &mut AppState, action: Action) -> Vec<Action> {
    match action {
        Action::Key(key) => {
            if let Some(semantic) = navigation::key_to_action_with_state(key, state) {
                return update(state, semantic);
            }
            vec![]
        }

        Action::Up => navigation::up(state),
        Action::Down => navigation::down(state),
        Action::OpenDetail => navigation::open_detail(state),
        Action::Activate(item) => navigation::activate(state, item),
        Action::SelectSidebar(i) => navigation::select_sidebar(state, i),
        Action::Back => navigation::back(state),
        Action::Quit => navigation::quit(state),
        Action::ToggleHelp => navigation::toggle_help(state),
        Action::Tick => navigation::tick(),
        Action::AnimTick => navigation::anim_tick(state),
        Action::Click(target) => navigation::click(state, target),
        Action::Hover(t) => navigation::hover(state, t),
        Action::Scroll(delta) => navigation::scroll(state, delta),

        Action::Refresh => fetch::refresh(state),
        Action::FetchStarted(ids) => fetch::fetch_started(state, ids),
        Action::ProviderFetched(q) => fetch::provider_fetched(state, q),
        Action::FetchCompleted { fetched_at, silent } => {
            fetch::fetch_completed(state, fetched_at, silent)
        }
        Action::ReloadUsage => fetch::reload_usage(),
        Action::FetchFailed(msg) => fetch::fetch_failed(state, msg),
        Action::UsageComputed(summary) => fetch::usage_computed(state, summary),

        Action::InitConfig(settings) => config::init_config(state, settings),
        Action::ConfigUp => config::config_up(state),
        Action::ConfigDown => config::config_down(state),
        Action::ConfigEnterEdit => config::config_enter_edit(state),
        Action::ConfigCancelEdit => config::config_cancel_edit(state),
        Action::ConfigConfirmEdit => config::config_confirm_edit(state),
        Action::SaveConfig => config::save_config(state),
        Action::ConfigSaveResult(result) => config::config_save_result(state, result),

        Action::LoginUp => login::login_up(state),
        Action::LoginDown => login::login_down(state),
        Action::LoginRequested(id) => login::login_requested(state, id),
        Action::LoginResult(result) => login::login_result(state, result),
        Action::LoginFinished(id) => login::login_finished(id),

        Action::HistoryLoaded(records) => history::history_loaded(state, records),
        Action::ToggleHistoryRange => history::toggle_history_range(state),
        Action::HistoryDown => history::history_down(state),
        Action::HistoryUp => history::history_up(state),
        Action::HistoryToggleDay => history::history_toggle_day(state),
    }
}

#[cfg(test)]
mod tests {
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    use super::update;
    use crate::providers::types::ProviderQuota;
    use crate::tui::action::Action;
    use crate::tui::mouse::{ChipKind, MouseTarget};
    use crate::tui::state::{
        sidebar_items, AppState, ConfigField, FetchStatus, HistoryRange, ProviderView, Screen,
        SidebarItem,
    };

    fn fake_quota(id: &str) -> ProviderQuota {
        ProviderQuota {
            provider: id.to_string(),
            display_name: id.to_string(),
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

    /// Quota com `primary.remaining` preenchido — usado pelos testes do fluxo
    /// de fetch assincrono (Task 5).
    fn test_quota(id: &str, remaining: f64) -> ProviderQuota {
        use crate::providers::types::QuotaWindow;
        let mut q = fake_quota(id);
        q.primary = Some(QuotaWindow {
            remaining,
            resets_at: None,
            window_minutes: None,
            used: Some(100.0 - remaining),
            severity: None,
            window_kind: None,
        });
        q
    }

    fn state_with_providers(n: usize) -> AppState {
        let mut s = AppState::new();
        s.providers = (0..n)
            .map(|i| ProviderView::new(fake_quota(&format!("p{i}"))))
            .collect();
        s
    }

    fn key_event(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::empty())
    }

    #[test]
    fn down_moves_sidebar_selected_and_clamps() {
        let mut state = state_with_providers(3);
        assert_eq!(state.sidebar_selected, 0);

        update(&mut state, Action::Down);
        assert_eq!(state.sidebar_selected, 1);

        update(&mut state, Action::Down);
        assert_eq!(state.sidebar_selected, 2);

        // sidebar_items(3) = [Provider(0..3), History, Login, Waybar] = 6 itens (indices 0..=5).
        // Overview morreu na Task 11 — sem o item extra do início.
        for _ in 0..10 {
            update(&mut state, Action::Down);
        }
        assert_eq!(
            state.sidebar_selected, 5,
            "should clamp at sidebar_items.len()-1"
        );
    }

    #[test]
    fn sidebar_has_no_overview() {
        let items = sidebar_items(2);
        assert_eq!(
            items,
            vec![
                SidebarItem::Provider(0),
                SidebarItem::Provider(1),
                SidebarItem::History,
                SidebarItem::Login,
                SidebarItem::Waybar,
            ]
        );
    }

    #[test]
    fn up_down_move_sidebar_and_enter_activates() {
        let mut state = AppState::new();
        state.providers = vec![ProviderView::new(test_quota("claude", 80.0))];
        // Provider(0) já é o primeiro item da sidebar (Task 11: sem Overview
        // na frente) — sidebar_selected começa em 0, apontando pra ele.
        assert_eq!(state.sidebar_selected, 0);
        update(&mut state, Action::Down); // Provider(0) → History
        assert_eq!(state.sidebar_selected, 1);
        update(&mut state, Action::Up); // History → Provider(0)
        assert_eq!(state.sidebar_selected, 0);
        update(&mut state, Action::OpenDetail); // Enter
        assert_eq!(state.screen, Screen::Detail);
        assert_eq!(state.selected, 0);
    }

    #[test]
    fn activate_history_login_waybar() {
        let mut state = AppState::new();
        update(&mut state, Action::Activate(SidebarItem::History));
        assert_eq!(state.screen, Screen::History);
        update(&mut state, Action::Activate(SidebarItem::Login));
        assert_eq!(state.screen, Screen::Login);
        let fu = update(&mut state, Action::Activate(SidebarItem::Waybar));
        assert_eq!(state.screen, Screen::Waybar);
        // Entrar na Waybar inicializa o config (comportamento atual do SwitchTab):
        assert!(matches!(fu.as_slice(), [Action::InitConfig(_)]));
    }

    #[test]
    fn activate_screen_change_resets_scroll() {
        // `state.scroll` e compartilhado entre telas (ScrollView do Overview
        // e a tabela do History) — sem reset, uma posicao de scroll deixada
        // numa tela vaza pra outra sem relacao nenhuma com aquele offset.
        let mut state = AppState::new();
        state.scroll = 12;
        update(&mut state, Action::Activate(SidebarItem::History));
        assert_eq!(state.screen, Screen::History);
        assert_eq!(state.scroll, 0, "trocar de tela deve zerar o scroll");

        // Reativar a MESMA tela (ex. atalho 'h' de novo em cima de History)
        // nao deve mexer no scroll do usuario ali dentro.
        state.scroll = 5;
        update(&mut state, Action::Activate(SidebarItem::History));
        assert_eq!(state.screen, Screen::History);
        assert_eq!(
            state.scroll, 5,
            "reativar a tela ja ativa nao deve reset o scroll"
        );
    }

    #[test]
    fn back_resets_scroll() {
        // Mesmo gap do Activate (state.scroll compartilhado entre telas):
        // Esc a partir de History tambem tem que zerar o scroll ao voltar
        // pro Detail (Task 11: Overview morreu, Detail é o destino), senao
        // o offset de uma tela vaza pra outra.
        let mut state = AppState::new();
        state.screen = Screen::History;
        state.scroll = 5;
        update(&mut state, Action::Back);
        assert_eq!(state.screen, Screen::Detail);
        assert_eq!(state.scroll, 0, "Esc pra outra tela deve zerar o scroll");
    }

    #[test]
    fn provider_fetched_growth_shifts_sidebar_cursor_to_keep_same_item() {
        // Cursor comeca em History (indice 0 com 0 providers: [History,
        // Login, Waybar] — Task 11: sem Overview). Um provider novo chega
        // via ProviderFetched (nao havia nenhum antes) — a sidebar cresce
        // pra [Provider(0), History, Login, Waybar] e History passa pro
        // indice 1. O cursor deve seguir o item logico (History), nao ficar
        // parado num indice que agora aponta pra outra coisa.
        let mut state = AppState::new();
        update(&mut state, Action::Activate(SidebarItem::History));
        assert_eq!(state.screen, Screen::History);
        assert_eq!(state.sidebar_selected, 0);

        update(
            &mut state,
            Action::ProviderFetched(Box::new(test_quota("claude", 80.0))),
        );

        assert_eq!(
            state.sidebar_selected, 1,
            "cursor deve seguir History (indice deslocado pelo provider novo)"
        );
        assert_eq!(
            sidebar_items(state.providers.len())[state.sidebar_selected],
            SidebarItem::History,
            "indice apos o shift deve continuar apontando pra History"
        );
    }

    #[test]
    fn activate_via_shortcut_syncs_sidebar_selected() {
        // Simula h/g/w: Activate chamado diretamente (nao via Down repetido)
        // deve sincronizar sidebar_selected pro indice do item ativado.
        let mut state = state_with_providers(2);
        assert_eq!(state.sidebar_selected, 0);

        update(&mut state, Action::Activate(SidebarItem::History));

        let expected = sidebar_items(2)
            .iter()
            .position(|i| *i == SidebarItem::History)
            .unwrap();
        assert_eq!(state.sidebar_selected, expected);
    }

    #[test]
    fn activate_provider_syncs_sidebar_selected() {
        let mut state = state_with_providers(2);
        // Cursor comeca em outro item (History) para provar que Activate
        // move o cursor, nao so o `selected` do Detail.
        update(&mut state, Action::Activate(SidebarItem::History));
        assert_ne!(state.sidebar_selected, 1);

        update(&mut state, Action::Activate(SidebarItem::Provider(0)));

        assert_eq!(state.screen, Screen::Detail);
        assert_eq!(state.selected, 0);
        assert_eq!(
            state.sidebar_selected, 0,
            "cursor deve sincronizar pro indice de Provider(0) na sidebar"
        );
    }

    #[test]
    fn fetch_started_sets_loading_and_pending() {
        let mut state = AppState::new();
        let fu = update(
            &mut state,
            Action::FetchStarted(vec!["claude".into(), "amp".into()]),
        );
        assert!(fu.is_empty());
        assert_eq!(state.status, FetchStatus::Loading);
        assert_eq!(
            state.fetch_pending,
            vec!["claude".to_string(), "amp".to_string()]
        );
    }

    #[test]
    fn fetch_started_unions_without_duplicating_across_overlapping_waves() {
        // Onda 1 (tick de 60s) resolve so "claude"; antes dela terminar, uma
        // onda 2 (Refresh) comeca com "claude" (de novo) + "codex". O
        // fetch_pending resultante deve ter cada id 1x, nao duplicado.
        let mut state = AppState::new();
        update(
            &mut state,
            Action::FetchStarted(vec!["claude".into(), "amp".into()]),
        );
        update(
            &mut state,
            Action::ProviderFetched(Box::new(test_quota("claude", 50.0))),
        );
        // "claude" ja saiu do pending; "amp" ainda esta em voo quando a onda 2 comeca.
        assert_eq!(state.fetch_pending, vec!["amp".to_string()]);

        update(
            &mut state,
            Action::FetchStarted(vec!["claude".into(), "codex".into()]),
        );

        // "amp" (da onda 1) + "claude"/"codex" (da onda 2), sem duplicar "amp".
        let mut pending = state.fetch_pending.clone();
        pending.sort();
        assert_eq!(
            pending,
            vec!["amp".to_string(), "claude".to_string(), "codex".to_string()]
        );
        assert_eq!(state.status, FetchStatus::Loading);
    }

    #[test]
    fn provider_fetched_merges_by_id_and_clears_pending() {
        let mut state = AppState::new();
        update(&mut state, Action::FetchStarted(vec!["claude".into()]));
        let q = test_quota("claude", 80.0);
        update(&mut state, Action::ProviderFetched(Box::new(q.clone())));
        assert!(state.fetch_pending.is_empty());
        assert_eq!(state.providers.len(), 1);
        assert_eq!(state.providers[0].quota.provider, "claude");
        // Segundo fetch do mesmo provider substitui (nao duplica):
        update(&mut state, Action::ProviderFetched(Box::new(q)));
        assert_eq!(state.providers.len(), 1);
    }

    #[test]
    fn fetch_completed_sets_loaded_and_requests_usage_reload() {
        let mut state = AppState::new();
        update(&mut state, Action::FetchStarted(vec!["claude".into()]));
        update(
            &mut state,
            Action::ProviderFetched(Box::new(test_quota("claude", 80.0))),
        );
        let fu = update(
            &mut state,
            Action::FetchCompleted {
                fetched_at: "2026-07-01T18:00:00.000Z".into(),
                silent: false,
            },
        );
        assert_eq!(state.status, FetchStatus::Loaded);
        assert!(state.last_update.is_some());
        assert!(matches!(fu.as_slice(), [Action::ReloadUsage]));
    }

    #[test]
    fn fetch_completed_with_pending_from_another_wave_stays_loading() {
        // Onda 1 ("claude"+"amp") e onda 2 ("codex") se sobrepoem. A onda 1
        // termina primeiro (FetchCompleted) mas "codex" (da onda 2) ainda
        // esta em voo: status deve permanecer Loading, e o pending restante
        // NAO pode ser apagado (senao a onda 2 nunca fecha o loop).
        let mut state = AppState::new();
        update(
            &mut state,
            Action::FetchStarted(vec!["claude".into(), "amp".into()]),
        );
        update(
            &mut state,
            Action::ProviderFetched(Box::new(test_quota("claude", 80.0))),
        );
        update(
            &mut state,
            Action::ProviderFetched(Box::new(test_quota("amp", 60.0))),
        );
        // Onda 2 comeca antes da onda 1 completar.
        update(&mut state, Action::FetchStarted(vec!["codex".into()]));

        let fu = update(
            &mut state,
            Action::FetchCompleted {
                fetched_at: "2026-07-01T18:00:00.000Z".into(),
                silent: false,
            },
        );

        assert_eq!(
            state.status,
            FetchStatus::Loading,
            "status deve permanecer Loading — a onda 2 (codex) ainda esta em voo"
        );
        assert_eq!(
            state.fetch_pending,
            vec!["codex".to_string()],
            "pending da onda 2 nao pode ser apagado pelo FetchCompleted da onda 1"
        );
        assert!(matches!(fu.as_slice(), [Action::ReloadUsage]));
    }

    #[test]
    fn fetch_completed_never_regresses_last_update() {
        let mut state = AppState::new();
        update(&mut state, Action::FetchStarted(vec!["claude".into()]));
        update(
            &mut state,
            Action::ProviderFetched(Box::new(test_quota("claude", 80.0))),
        );
        update(
            &mut state,
            Action::FetchCompleted {
                fetched_at: "2026-07-01T18:00:00.000Z".into(),
                silent: false,
            },
        );
        let after_first = state.last_update;
        assert!(after_first.is_some());

        // Uma 2a onda (mais lenta) termina com um fetched_at MAIS ANTIGO
        // (ex.: comecou antes, mas so completou depois) — last_update nao
        // pode regredir.
        update(&mut state, Action::FetchStarted(vec!["amp".into()]));
        update(
            &mut state,
            Action::ProviderFetched(Box::new(test_quota("amp", 60.0))),
        );
        update(
            &mut state,
            Action::FetchCompleted {
                fetched_at: "2026-07-01T17:00:00.000Z".into(), // 1h antes
                silent: false,
            },
        );

        assert_eq!(
            state.last_update, after_first,
            "last_update nao deve regredir para um fetched_at mais antigo"
        );
    }

    #[test]
    fn fetch_failed_clears_pending() {
        let mut state = AppState::new();
        update(
            &mut state,
            Action::FetchStarted(vec!["claude".into(), "amp".into()]),
        );
        assert!(!state.fetch_pending.is_empty());

        update(
            &mut state,
            Action::FetchFailed("fetch runtime: boom".into()),
        );

        assert!(
            state.fetch_pending.is_empty(),
            "FetchFailed deve limpar fetch_pending (senao o spinner gira pra sempre)"
        );
        assert_eq!(
            state.status,
            FetchStatus::Failed("fetch runtime: boom".to_string())
        );
    }

    #[test]
    fn fetch_flow_populates_providers_and_status() {
        // Migrado de `data_fetched_populates_providers_and_status` (Task 5):
        // o fluxo FetchStarted->ProviderFetched->FetchCompleted substitui o
        // antigo Action::DataFetched(AllQuotas), preservando as mesmas
        // validacoes (providers populados, status Loaded, last_update parseado).
        let mut state = AppState::new();
        assert_eq!(state.status, FetchStatus::Idle);
        assert!(state.providers.is_empty());
        assert!(state.last_update.is_none());

        update(
            &mut state,
            Action::FetchStarted(vec!["claude".into(), "codex".into()]),
        );
        update(
            &mut state,
            Action::ProviderFetched(Box::new(fake_quota("claude"))),
        );
        update(
            &mut state,
            Action::ProviderFetched(Box::new(fake_quota("codex"))),
        );
        update(
            &mut state,
            Action::FetchCompleted {
                fetched_at: "2026-06-19T12:00:00.000Z".to_string(),
                silent: false,
            },
        );

        assert_eq!(state.status, FetchStatus::Loaded);
        assert_eq!(state.providers.len(), 2);
        assert_eq!(state.providers[0].quota.provider, "claude");
        assert_eq!(state.providers[1].quota.provider, "codex");
        assert!(
            state.last_update.is_some(),
            "last_update should be Some after FetchCompleted"
        );
    }

    #[test]
    fn key_q_sets_should_quit() {
        let mut state = AppState::new();
        assert!(!state.should_quit);

        // Key('q') → translated to Quit → should_quit = true
        update(&mut state, Action::Key(key_event(KeyCode::Char('q'))));

        assert!(
            state.should_quit,
            "should_quit should be true after Key('q')"
        );
    }

    #[test]
    fn anim_tick_increments_anim_frame_and_throbber() {
        let mut state = AppState::new();
        assert_eq!(state.anim_frame, 0);
        assert_eq!(state.throbber.index, 0);

        update(&mut state, Action::AnimTick);
        assert_eq!(state.anim_frame, 1);
        assert_eq!(state.throbber.index, 1);

        for _ in 0..5 {
            update(&mut state, Action::AnimTick);
        }
        // throbber wraps at 6: 1+5 = 6 → 6 % 6 = 0
        assert_eq!(
            state.throbber.index, 0,
            "throbber deve voltar a 0 após 6 ticks"
        );
        assert_eq!(state.anim_frame, 6);
    }

    // ---- Motion: tachyonfx + lerps (Task 16) ----

    /// UsageSummary com `total_cost.usd` fixo — helper pro teste de count-up
    /// de `display_cost` (não interessa o resto do summary).
    fn test_usage_summary_with_cost(usd: f64) -> crate::usage::UsageSummary {
        crate::usage::UsageSummary {
            providers: vec![],
            total_cost: crate::usage::Cost {
                usd,
                brl: usd * 5.50,
            },
            fx_rate: 5.50,
        }
    }

    #[test]
    fn screen_change_pushes_no_fx_event() {
        // O coalesce de troca de tela foi removido (reprovado em uso real) —
        // navegar entre telas não deve enfileirar efeito nenhum.
        let mut state = AppState::new();
        update(&mut state, Action::Activate(SidebarItem::History));
        assert!(
            state.fx_queue.is_empty(),
            "troca de tela não deve empurrar FxEvent (coalesce removido)"
        );
    }

    #[test]
    fn fetch_completed_pushes_fetch_landed() {
        let mut state = AppState::new();
        update(
            &mut state,
            Action::FetchCompleted {
                fetched_at: "2026-07-01T18:00:00.000Z".into(),
                silent: false,
            },
        );
        assert!(state
            .fx_queue
            .contains(&crate::tui::state::FxEvent::FetchLanded));
    }

    /// Fix pós-review (T16): `silent=true` (poll de 60s do `data_tick`) NÃO
    /// deve disparar o sweep — spec §8 quer o efeito só em ondas pedidas
    /// pelo usuário (load inicial, `r`/chip Refresh, LoginFinished).
    #[test]
    fn fetch_completed_silent_does_not_push_fetch_landed() {
        let mut state = AppState::new();
        update(
            &mut state,
            Action::FetchCompleted {
                fetched_at: "2026-07-01T18:00:00.000Z".into(),
                silent: true,
            },
        );
        assert!(
            !state
                .fx_queue
                .contains(&crate::tui::state::FxEvent::FetchLanded),
            "onda silenciosa (poll de 60s) não deve empurrar FetchLanded"
        );
    }

    #[test]
    fn anim_tick_lerps_display_cost_toward_target() {
        let mut state = AppState::new();
        // usage já presente (não é o 1º load) para exercitar o lerp, não o
        // snap de UsageComputed — display_cost começa em 0.0 (default).
        state.usage = Some(test_usage_summary_with_cost(100.0));
        state.display_cost = 0.0;

        update(&mut state, Action::AnimTick);
        assert!(state.display_cost > 0.0 && state.display_cost < 100.0);

        for _ in 0..400 {
            update(&mut state, Action::AnimTick);
        }
        assert!((state.display_cost - 100.0).abs() < 0.5);
    }

    #[test]
    fn anim_tick_snaps_display_cost_when_animations_off() {
        // Self-review: animations=false → zero lerp visual (snap direto).
        let mut state = AppState::new();
        state.animations = false;
        state.usage = Some(test_usage_summary_with_cost(42.0));
        state.display_cost = 0.0;

        update(&mut state, Action::AnimTick);

        assert_eq!(
            state.display_cost, 42.0,
            "com animations=false, 1 único AnimTick já deve snapar pro alvo"
        );
    }

    #[test]
    fn usage_computed_first_load_snaps_display_cost_without_animating() {
        // O 1º load não deve animar a partir de zero.
        let mut state = AppState::new();
        assert!(state.usage.is_none());
        assert_eq!(state.display_cost, 0.0);

        update(
            &mut state,
            Action::UsageComputed(test_usage_summary_with_cost(7.5)),
        );

        assert_eq!(state.display_cost, 7.5);
    }

    #[test]
    fn usage_computed_second_load_does_not_reset_display_cost() {
        // Loads seguintes (usage já Some) preservam display_cost — o
        // AnimTick é quem faz o count-up até o novo alvo, não o snap.
        let mut state = AppState::new();
        update(
            &mut state,
            Action::UsageComputed(test_usage_summary_with_cost(10.0)),
        );
        assert_eq!(state.display_cost, 10.0);

        update(
            &mut state,
            Action::UsageComputed(test_usage_summary_with_cost(50.0)),
        );

        assert_eq!(
            state.display_cost, 10.0,
            "2º load não deve resetar display_cost — AnimTick faz o count-up"
        );
    }

    // ---- Config tab tests ----

    fn fake_settings() -> crate::settings::Settings {
        use crate::settings::*;
        use std::collections::BTreeMap;
        Settings {
            version: CURRENT_VERSION,
            waybar: Waybar {
                providers: vec!["claude".to_string(), "codex".to_string()],
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

    /// Indice de `ConfigField::FxRate` em `ConfigField::ALL` — busca por
    /// variante, nao por posicao fixa (Task 14 moveu FxRate pro fim).
    fn fx_rate_index() -> usize {
        ConfigField::ALL
            .iter()
            .position(|f| *f == ConfigField::FxRate)
            .expect("FxRate deve estar em ConfigField::ALL")
    }

    #[test]
    fn init_config_creates_config_state() {
        let mut state = AppState::new();
        assert!(state.config_state.is_none());

        update(&mut state, Action::InitConfig(fake_settings()));

        assert!(state.config_state.is_some());
        let cs = state.config_state.as_ref().unwrap();
        let diff = (cs.edit_settings.fx_rate - 5.50_f64).abs();
        assert!(
            diff < 1e-10,
            "fx_rate esperado 5.50, obtido {}",
            cs.edit_settings.fx_rate
        );
    }

    #[test]
    fn config_navigate_down_and_up() {
        let mut state = AppState::new();
        update(&mut state, Action::InitConfig(fake_settings()));

        update(&mut state, Action::ConfigDown);
        assert_eq!(state.config_state.as_ref().unwrap().selected_field, 1);

        update(&mut state, Action::ConfigDown);
        assert_eq!(state.config_state.as_ref().unwrap().selected_field, 2);

        update(&mut state, Action::ConfigUp);
        assert_eq!(state.config_state.as_ref().unwrap().selected_field, 1);
    }

    #[test]
    fn config_navigate_clamps_at_bounds() {
        let mut state = AppState::new();
        update(&mut state, Action::InitConfig(fake_settings()));

        // Ja em 0, Up nao deve subtrair
        update(&mut state, Action::ConfigUp);
        assert_eq!(state.config_state.as_ref().unwrap().selected_field, 0);

        // Vai ate o ultimo campo
        let max = crate::tui::state::ConfigField::ALL.len() - 1;
        for _ in 0..max + 5 {
            update(&mut state, Action::ConfigDown);
        }
        assert_eq!(state.config_state.as_ref().unwrap().selected_field, max);
    }

    #[test]
    fn config_enter_edit_sets_input_to_current_value() {
        let mut state = AppState::new();
        update(&mut state, Action::InitConfig(fake_settings()));

        // Seleciona o campo FxRate (por variante, nao por posicao — a ordem
        // de ConfigField::ALL mudou na Task 14).
        state.config_state.as_mut().unwrap().selected_field = fx_rate_index();
        update(&mut state, Action::ConfigEnterEdit);

        let cs = state.config_state.as_ref().unwrap();
        assert!(cs.editing);
        assert_eq!(cs.input.value(), "5.50");
    }

    #[test]
    fn config_confirm_edit_updates_fx_rate() {
        let mut state = AppState::new();
        update(&mut state, Action::InitConfig(fake_settings()));
        state.config_state.as_mut().unwrap().selected_field = fx_rate_index(); // FxRate
        update(&mut state, Action::ConfigEnterEdit);

        // Simula o usuario digitando "6.25" no buffer
        state.config_state.as_mut().unwrap().input = tui_input::Input::new("6.25".to_string());
        update(&mut state, Action::ConfigConfirmEdit);

        let cs = state.config_state.as_ref().unwrap();
        assert!(!cs.editing, "edicao deve fechar apos confirmacao valida");
        let diff = (cs.edit_settings.fx_rate - 6.25_f64).abs();
        assert!(
            diff < 1e-10,
            "fx_rate deveria ser 6.25, obtido {}",
            cs.edit_settings.fx_rate
        );
    }

    #[test]
    fn config_confirm_edit_invalid_fx_rate_keeps_editing() {
        let mut state = AppState::new();
        update(&mut state, Action::InitConfig(fake_settings()));
        state.config_state.as_mut().unwrap().selected_field = fx_rate_index(); // FxRate
        update(&mut state, Action::ConfigEnterEdit);
        state.config_state.as_mut().unwrap().input = tui_input::Input::new("negativo".to_string());
        update(&mut state, Action::ConfigConfirmEdit);

        let cs = state.config_state.as_ref().unwrap();
        assert!(
            cs.editing,
            "edicao deve permanecer aberta apos valor invalido"
        );
        assert!(
            cs.status_msg
                .as_ref()
                .map(|m| m.starts_with("Erro"))
                .unwrap_or(false),
            "status_msg deve conter 'Erro'"
        );
    }

    #[test]
    fn config_cancel_edit_clears_editing() {
        let mut state = AppState::new();
        update(&mut state, Action::InitConfig(fake_settings()));
        state.config_state.as_mut().unwrap().selected_field = fx_rate_index(); // FxRate
        update(&mut state, Action::ConfigEnterEdit);
        assert!(state.config_state.as_ref().unwrap().editing);

        update(&mut state, Action::ConfigCancelEdit);
        assert!(!state.config_state.as_ref().unwrap().editing);
    }

    #[test]
    fn save_config_sets_pending_save_and_status() {
        let mut state = AppState::new();
        update(&mut state, Action::InitConfig(fake_settings()));

        let follow_ups = update(&mut state, Action::SaveConfig);
        // Nao re-enfileira mais: o event_loop le pending_save no topo do loop.
        assert!(follow_ups.is_empty());
        assert!(state.pending_save, "pending_save deve ser sinalizado");
        // Status msg deve ser "Salvando..."
        let msg = state.config_state.as_ref().unwrap().status_msg.as_deref();
        assert_eq!(msg, Some("Salvando..."));
    }

    // ---- Aba Login ----

    #[test]
    fn login_requested_sets_pending_and_status() {
        let mut state = AppState::new();
        let fu = update(&mut state, Action::LoginRequested("codex".into()));
        assert!(fu.is_empty()); // nao re-enfileira mais: o event_loop le pending_login
        assert_eq!(state.pending_login.as_deref(), Some("codex"));
        assert!(state
            .login_status
            .as_deref()
            .unwrap_or("")
            .contains("codex"));
    }

    #[test]
    fn login_finished_success_requests_single_refetch() {
        let mut state = AppState::new();
        let fu = update(&mut state, Action::LoginFinished("codex".into()));
        // O drain intercepta esta action diretamente (spawn_fetch(only=Some(id)))
        // sem re-entrar no update — nao ha guard anti-loop porque o update
        // nunca ve esta action de volta.
        assert!(matches!(fu.as_slice(), [Action::LoginFinished(id)] if id == "codex"));
    }

    // ---- Refresh (tecla [r]) ----

    #[test]
    fn refresh_with_no_pending_fetch_reenqueues_once() {
        let mut state = AppState::new();
        assert!(state.fetch_pending.is_empty());

        let fu = update(&mut state, Action::Refresh);

        assert!(matches!(fu.as_slice(), [Action::Refresh]));
    }

    #[test]
    fn refresh_with_pending_fetch_is_noop() {
        let mut state = AppState::new();
        update(&mut state, Action::FetchStarted(vec!["claude".into()]));
        assert!(!state.fetch_pending.is_empty());

        let fu = update(&mut state, Action::Refresh);

        assert!(
            fu.is_empty(),
            "Refresh deve ser no-op quando ja ha fetch em voo (evita duplicar spawn_fetch)"
        );
    }

    // ---- Mouse (Task 9) ----

    #[test]
    fn click_sidebar_selects_and_activates() {
        let mut state = AppState::new();
        state.providers = vec![ProviderView::new(test_quota("claude", 80.0))];
        // Provider(0) é o item 0 da sidebar (Task 11: sem Overview na frente).
        update(&mut state, Action::Click(MouseTarget::Sidebar(0)));
        assert_eq!(state.sidebar_selected, 0);
        assert_eq!(state.screen, Screen::Detail); // Provider(0) ativado
    }

    #[test]
    fn click_sidebar_out_of_range_is_noop() {
        // sidebar_items(0) = [History, Login, Waybar] (3 itens, indices 0..=2).
        let mut state = AppState::new();
        let fu = update(&mut state, Action::Click(MouseTarget::Sidebar(99)));
        assert_eq!(
            state.sidebar_selected, 0,
            "indice fora de faixa e ignorado — cursor nao se move"
        );
        assert!(
            fu.is_empty(),
            "indice fora de sidebar_items() nao deve gerar Activate"
        );
        assert_eq!(state.screen, Screen::Detail, "tela nao muda sem Activate");
    }

    // ---- Aba Login: reskin (Task 14) ----

    #[test]
    fn click_card_on_login_screen_only_selects_never_activates_detail() {
        // Regressão do bug que esta task evita: MouseTarget::Card(i) já
        // significa "Activate(Provider(i))" no Overview (ativa Detail). A
        // lista de providers da tela Login reusa Card(i) pro mesmo visual,
        // mas o clique deve APENAS selecionar (mesmo efeito de
        // LoginUp/LoginDown) — nunca navegar pra fora da tela Login.
        let mut state = AppState::new();
        state.screen = Screen::Login;
        state.login_selected = 0;
        state.login_status = Some("Abrindo login para claude...".to_string());

        let fu = update(&mut state, Action::Click(MouseTarget::Card(1)));

        assert_eq!(state.login_selected, 1, "clique deve selecionar o item 1");
        assert_eq!(
            state.screen,
            Screen::Login,
            "clique num item da lista NUNCA deve navegar pra Detail"
        );
        assert_eq!(
            state.login_status, None,
            "seleção limpa o status (mesmo comportamento de LoginUp/LoginDown)"
        );
        assert!(fu.is_empty());
    }

    #[test]
    fn click_card_on_login_screen_ignores_out_of_range_index() {
        let mut state = AppState::new();
        state.screen = Screen::Login;
        state.login_selected = 0;

        update(&mut state, Action::Click(MouseTarget::Card(99)));

        assert_eq!(
            state.login_selected, 0,
            "índice fora de faixa (só 4 providers) é ignorado"
        );
    }

    #[test]
    fn click_card_outside_login_screen_is_noop() {
        // Task 11: Card(i) era do Overview/dashboard (ambos apagados) —
        // fora da tela Login, o clique não faz mais nada (a lista de
        // providers da Detail é a sidebar, não cards clicáveis).
        let mut state = AppState::new();
        state.providers = vec![ProviderView::new(test_quota("claude", 80.0))];
        let fu = update(&mut state, Action::Click(MouseTarget::Card(0)));
        assert_eq!(state.screen, Screen::Detail, "screen default, sem mudança");
        assert!(fu.is_empty());
    }

    #[test]
    fn click_start_login_chip_fires_same_action_as_enter_for_selected_provider() {
        // O chip "iniciar login" tem que disparar a MESMA action que o
        // Enter dispara na tela Login (Action::LoginRequested pro provider
        // selecionado) — nunca Action::Activate(Login) (ChipKind::Login
        // seria no-op ali, a tela já está ativa; T14 introduz
        // ChipKind::StartLogin exatamente pra evitar essa armadilha).
        let mut state = AppState::new();
        state.screen = Screen::Login;
        state.login_selected = 1; // codex

        update(
            &mut state,
            Action::Click(MouseTarget::Chip(ChipKind::StartLogin)),
        );

        assert_eq!(state.pending_login.as_deref(), Some("codex"));
        assert!(state
            .login_status
            .as_deref()
            .unwrap_or("")
            .contains("codex"));
    }

    // ---- Tela Waybar: reskin (Task 14) ----

    #[test]
    fn click_enter_edit_chip_starts_editing_selected_field() {
        let mut state = AppState::new();
        update(&mut state, Action::InitConfig(fake_settings()));

        update(
            &mut state,
            Action::Click(MouseTarget::Chip(ChipKind::EnterEdit)),
        );

        let cs = state.config_state.as_ref().unwrap();
        assert!(cs.editing, "chip 'editar' deve entrar em modo de edição");
    }

    #[test]
    fn click_save_config_chip_sets_pending_save() {
        let mut state = AppState::new();
        update(&mut state, Action::InitConfig(fake_settings()));

        update(
            &mut state,
            Action::Click(MouseTarget::Chip(ChipKind::SaveConfig)),
        );

        assert!(
            state.pending_save,
            "chip 'salvar' deve sinalizar pending_save (mesmo efeito da tecla [s])"
        );
    }

    // ---- Aba History (Task 13) ----

    #[test]
    fn toggle_history_range_flips() {
        let mut state = AppState::new();
        assert_eq!(state.history_range, HistoryRange::Week);
        update(&mut state, Action::ToggleHistoryRange);
        assert_eq!(state.history_range, HistoryRange::Day);
        update(&mut state, Action::ToggleHistoryRange);
        assert_eq!(state.history_range, HistoryRange::Week);
    }

    #[test]
    fn click_toggle_range_chip_flips_range() {
        let mut state = AppState::new();
        assert_eq!(state.history_range, HistoryRange::Week);
        update(
            &mut state,
            Action::Click(MouseTarget::Chip(ChipKind::ToggleRange)),
        );
        assert_eq!(state.history_range, HistoryRange::Day);
    }

    #[test]
    fn key_t_on_history_screen_toggles_range() {
        let mut state = AppState::new();
        state.screen = Screen::History;
        update(&mut state, Action::Key(key_event(KeyCode::Char('t'))));
        assert_eq!(state.history_range, HistoryRange::Day);
    }

    #[test]
    fn key_t_outside_history_screen_is_noop() {
        let mut state = AppState::new();
        assert_eq!(state.screen, Screen::Detail);
        let fu = update(&mut state, Action::Key(key_event(KeyCode::Char('t'))));
        assert!(fu.is_empty());
        assert_eq!(
            state.history_range,
            HistoryRange::Week,
            "'t' fora da tela History nao deve alternar o range"
        );
    }

    // ---- Aba History: lista de dias expansível (Task 20) ----

    /// `UsageRecord` com `session_id` — helper local (mirror de
    /// `usage::buckets::tests::mrec`, mas com session_id, que os testes de
    /// `update.rs` precisam pra exercitar `sessions_by_day` de verdade).
    fn test_record(
        provider: &str,
        model: &str,
        session_id: &str,
        ts: &str,
        tokens: u64,
    ) -> crate::usage::UsageRecord {
        let parsed =
            time::OffsetDateTime::parse(ts, &time::format_description::well_known::Rfc3339)
                .expect("timestamp de teste invalido");
        crate::usage::UsageRecord {
            provider: provider.to_string(),
            model: Some(model.to_string()),
            input: tokens,
            output: 0,
            cache_read: 0,
            cache_write: 0,
            cache_write_1h: 0,
            fast: false,
            geo_us: false,
            ts: parsed,
            session_id: Some(session_id.to_string()),
            project: None,
        }
    }

    #[test]
    fn history_keys_select_and_toggle_days() {
        let mut state = AppState::new();
        state.screen = Screen::History;
        state.history = Some(vec![
            test_record("claude", "claude-fable-5", "s1", "2026-07-10T10:00:00Z", 10),
            test_record("claude", "claude-fable-5", "s2", "2026-07-09T10:00:00Z", 10),
        ]);

        // j/Down -> HistoryDown: avança do dia mais recente (índice 0) pro
        // segundo (índice 1).
        update(&mut state, Action::Key(key_event(KeyCode::Char('j'))));
        assert_eq!(
            state.history_selected, 1,
            "'j' deve mapear para HistoryDown"
        );

        // k/Up -> HistoryUp: volta pro índice 0.
        update(&mut state, Action::Key(key_event(KeyCode::Char('k'))));
        assert_eq!(state.history_selected, 0, "'k' deve mapear para HistoryUp");

        // Enter -> HistoryToggleDay: expande o dia selecionado.
        update(&mut state, Action::Key(key_event(KeyCode::Enter)));
        assert_eq!(
            state.history_expanded.len(),
            1,
            "Enter deve mapear para HistoryToggleDay"
        );
    }

    #[test]
    fn history_toggle_day_flips_expanded_set() {
        let mut state = AppState::new();
        state.screen = Screen::History;
        // 2 dias de records → sessions_by_day produz 2 dias.
        state.history = Some(vec![
            test_record("claude", "claude-fable-5", "s1", "2026-07-10T10:00:00Z", 10),
            test_record("claude", "claude-fable-5", "s2", "2026-07-09T10:00:00Z", 10),
        ]);
        state.history_selected = 0;
        update(&mut state, Action::HistoryToggleDay);
        assert_eq!(state.history_expanded.len(), 1);
        update(&mut state, Action::HistoryToggleDay);
        assert!(state.history_expanded.is_empty());
    }

    #[test]
    fn history_down_clamps_at_last_day() {
        let mut state = AppState::new();
        state.screen = Screen::History;
        state.history = Some(vec![test_record(
            "claude",
            "claude-fable-5",
            "s1",
            "2026-07-10T10:00:00Z",
            10,
        )]);
        // Só 1 dia: HistoryDown não pode avançar além do índice 0.
        update(&mut state, Action::HistoryDown);
        assert_eq!(state.history_selected, 0);
    }

    #[test]
    fn history_up_saturates_at_zero() {
        let mut state = AppState::new();
        state.screen = Screen::History;
        state.history_selected = 0;
        update(&mut state, Action::HistoryUp);
        assert_eq!(state.history_selected, 0);
    }

    #[test]
    fn click_expand_day_chip_fires_toggle() {
        let mut state = AppState::new();
        state.screen = Screen::History;
        state.history = Some(vec![test_record(
            "claude",
            "claude-fable-5",
            "s1",
            "2026-07-10T10:00:00Z",
            10,
        )]);
        update(
            &mut state,
            Action::Click(MouseTarget::Chip(ChipKind::ExpandDay)),
        );
        assert_eq!(state.history_expanded.len(), 1);
    }

    #[test]
    fn history_screen_keeps_other_key_bindings() {
        // Regressão: a tela History ganhou um braço de match dedicado (j/k/
        // Enter) — as demais teclas (t/r/Esc/h/g/w/q) não podem se perder
        // no meio da reescrita (contrato do brief: "mantém t/r/Esc/h/g/w/q/?").
        let mut state = AppState::new();
        state.screen = Screen::History;

        let fu = update(&mut state, Action::Key(key_event(KeyCode::Char('r'))));
        assert!(
            matches!(fu.as_slice(), [Action::Refresh]),
            "'r' deve continuar disparando Refresh"
        );

        update(&mut state, Action::Key(key_event(KeyCode::Esc)));
        assert_eq!(
            state.screen,
            Screen::Detail,
            "Esc deve continuar voltando pro Detail (Task 11: Overview morreu)"
        );

        state.screen = Screen::History;
        update(&mut state, Action::Key(key_event(KeyCode::Char('q'))));
        assert!(state.should_quit, "'q' deve continuar saindo");
    }

    #[test]
    fn hover_and_scroll_update_state() {
        let mut state = AppState::new();
        update(&mut state, Action::Hover(Some(MouseTarget::Card(0))));
        assert_eq!(state.hover, Some(MouseTarget::Card(0)));
        state.scroll = 2;
        update(&mut state, Action::Scroll(-1));
        assert_eq!(state.scroll, 1);
        update(&mut state, Action::Scroll(-5));
        assert_eq!(state.scroll, 0); // saturating
    }

    // ---- Navegação: Overview morre, boot no provider (Task 11) ----

    #[test]
    fn boot_state_is_detail_skeleton_not_overview() {
        let state = AppState::new();
        assert_eq!(state.screen, Screen::Detail);
    }

    #[test]
    fn provider_fetched_resolves_pending_focus_by_id() {
        let mut state = AppState::new();
        state.pending_focus = Some("codex".into());
        // Chega claude primeiro: NÃO rouba o foco.
        update(
            &mut state,
            Action::ProviderFetched(Box::new(fake_quota("claude"))),
        );
        assert_eq!(state.pending_focus.as_deref(), Some("codex"));
        // Chega codex: resolve — Detail + selected no índice recém-inserido (1).
        update(
            &mut state,
            Action::ProviderFetched(Box::new(fake_quota("codex"))),
        );
        assert_eq!(state.screen, Screen::Detail);
        assert_eq!(state.selected, 1);
        assert_eq!(state.pending_focus, None);
    }

    #[test]
    fn esc_from_history_returns_to_selected_provider_detail() {
        let mut state = AppState::new();
        update(
            &mut state,
            Action::ProviderFetched(Box::new(fake_quota("claude"))),
        );
        state.screen = Screen::History;
        update(&mut state, Action::Back);
        assert_eq!(state.screen, Screen::Detail);
    }
}
