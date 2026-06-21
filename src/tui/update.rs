use ratatui::crossterm::event::{KeyCode, KeyEvent};
use time::OffsetDateTime;

use super::action::Action;
use super::state::{AppState, ConfigField, ConfigState, FetchStatus, Mode, ProviderView, Tab};

/// Translates a raw KeyEvent into a semantic Action, if applicable.
pub fn key_to_action(key: KeyEvent) -> Option<Action> {
    match key.code {
        KeyCode::Char('j') | KeyCode::Down => Some(Action::Down),
        KeyCode::Char('k') | KeyCode::Up => Some(Action::Up),
        KeyCode::Enter => Some(Action::OpenDetail),
        KeyCode::Esc => Some(Action::Back),
        KeyCode::Left => {
            // Will be resolved in update using current tab; return sentinel via Char('<')
            // Actually we return a SwitchTab action resolved here is not possible without state.
            // So we return a raw Left action wrapped — update will handle it.
            None // handled below
        }
        _ => None,
    }
}

/// Translates a KeyEvent into a semantic Action using current tab state for
/// cyclic left/right tab switching.
fn key_to_action_with_state(key: KeyEvent, state: &AppState) -> Option<Action> {
    // Se o overlay de ajuda esta aberto, qualquer tecla fecha (Esc ou '?').
    if state.show_help {
        return match key.code {
            KeyCode::Esc | KeyCode::Char('?') | KeyCode::Char('q') => Some(Action::ToggleHelp),
            _ => None,
        };
    }

    // '?' global: abre o overlay de ajuda de qualquer contexto (exceto edicao).
    let in_config_edit = state.tab == Tab::Waybar
        && state
            .config_state
            .as_ref()
            .map(|cs| cs.editing)
            .unwrap_or(false);

    if key.code == KeyCode::Char('?') && !in_config_edit {
        return Some(Action::ToggleHelp);
    }

    // Na aba Waybar com campo em edicao, delega ao input buffer (so Esc/Enter escapam).
    if state.tab == Tab::Waybar {
        if let Some(cs) = &state.config_state {
            if cs.editing {
                return match key.code {
                    KeyCode::Esc => Some(Action::ConfigCancelEdit),
                    KeyCode::Enter => Some(Action::ConfigConfirmEdit),
                    _ => None, // o event_loop vai passar o evento cru ao Input
                };
            }
        }
        // Aba Waybar, fora do modo edicao.
        return match key.code {
            KeyCode::Char('j') | KeyCode::Down => Some(Action::ConfigDown),
            KeyCode::Char('k') | KeyCode::Up => Some(Action::ConfigUp),
            KeyCode::Enter => Some(Action::ConfigEnterEdit),
            KeyCode::Char('s') => Some(Action::SaveConfig),
            KeyCode::Esc => Some(Action::Back),
            KeyCode::Left | KeyCode::BackTab => {
                let idx = state.tab.index();
                let next = if idx == 0 { 3 } else { idx - 1 };
                Some(Action::SwitchTab(Tab::from_index(next)))
            }
            KeyCode::Right | KeyCode::Tab => {
                let idx = state.tab.index();
                let next = (idx + 1) % 4;
                Some(Action::SwitchTab(Tab::from_index(next)))
            }
            KeyCode::Char('q') => Some(Action::Quit),
            _ => None,
        };
    }

    // Aba Login: navegacao e acao de login.
    if state.tab == Tab::Login {
        return match key.code {
            KeyCode::Char('j') | KeyCode::Down => Some(Action::LoginDown),
            KeyCode::Char('k') | KeyCode::Up => Some(Action::LoginUp),
            KeyCode::Enter => {
                let id = match state.login_selected {
                    0 => "claude",
                    1 => "codex",
                    _ => "amp",
                };
                Some(Action::LoginRequested(id.to_string()))
            }
            KeyCode::Left | KeyCode::BackTab => {
                let idx = state.tab.index();
                let next = if idx == 0 { 3 } else { idx - 1 };
                Some(Action::SwitchTab(Tab::from_index(next)))
            }
            KeyCode::Right | KeyCode::Tab => {
                let idx = state.tab.index();
                let next = (idx + 1) % 4;
                Some(Action::SwitchTab(Tab::from_index(next)))
            }
            KeyCode::Char('q') => Some(Action::Quit),
            _ => None,
        };
    }

    match key.code {
        KeyCode::Char('j') | KeyCode::Down => Some(Action::Down),
        KeyCode::Char('k') | KeyCode::Up => Some(Action::Up),
        KeyCode::Enter => Some(Action::OpenDetail),
        KeyCode::Esc => Some(Action::Back),
        KeyCode::Left | KeyCode::BackTab => {
            let idx = state.tab.index();
            let next = if idx == 0 { 3 } else { idx - 1 };
            Some(Action::SwitchTab(Tab::from_index(next)))
        }
        KeyCode::Right | KeyCode::Tab => {
            let idx = state.tab.index();
            let next = (idx + 1) % 4;
            Some(Action::SwitchTab(Tab::from_index(next)))
        }
        KeyCode::Char('r') => Some(Action::Refresh),
        KeyCode::Char('q') => Some(Action::Quit),
        _ => None,
    }
}

/// Retorna a representacao em string de um campo das edit_settings.
fn field_value_string(field: ConfigField, cs: &ConfigState) -> String {
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

/// Aplica o valor textual do buffer de edicao ao campo das edit_settings.
/// Retorna Err com mensagem descritiva se o valor e invalido.
fn apply_field_edit(field: ConfigField, value: &str, cs: &mut ConfigState) -> Result<(), String> {
    let s = &mut cs.edit_settings;
    match field {
        ConfigField::Providers => {
            let providers: Vec<String> = value
                .split(',')
                .map(|p| p.trim().to_string())
                .filter(|p| !p.is_empty())
                .collect();
            let (normalized, order) =
                crate::settings::normalize_provider_selection(&providers, &s.waybar.provider_order);
            s.waybar.providers = normalized;
            s.waybar.provider_order = order;
            Ok(())
        }
        ConfigField::ProviderOrder => {
            let order: Vec<String> = value
                .split(',')
                .map(|p| p.trim().to_string())
                .filter(|p| !p.is_empty())
                .collect();
            let (normalized, new_order) =
                crate::settings::normalize_provider_selection(&s.waybar.providers, &order);
            s.waybar.providers = normalized;
            s.waybar.provider_order = new_order;
            Ok(())
        }
        ConfigField::Separators => {
            use crate::settings::SeparatorStyle;
            let sep = match value.trim() {
                "pill" => SeparatorStyle::Pill,
                "gap" => SeparatorStyle::Gap,
                "bare" => SeparatorStyle::Bare,
                "glass" => SeparatorStyle::Glass,
                "shadow" => SeparatorStyle::Shadow,
                "none" => SeparatorStyle::None,
                other => {
                    return Err(format!(
                        "separador invalido: '{other}' (pill/gap/bare/glass/shadow/none)"
                    ))
                }
            };
            s.waybar.separators = sep;
            Ok(())
        }
        ConfigField::DisplayMode => {
            use crate::settings::DisplayMode;
            let mode = match value.trim() {
                "remaining" => DisplayMode::Remaining,
                "used" => DisplayMode::Used,
                other => return Err(format!("modo invalido: '{other}' (remaining/used)")),
            };
            s.waybar.display_mode = mode;
            Ok(())
        }
        ConfigField::FxRate => {
            let rate: f64 = value
                .trim()
                .parse()
                .map_err(|_| format!("fxRate invalido: '{value}' (numero esperado)"))?;
            if !rate.is_finite() || rate <= 0.0 {
                return Err(format!("fxRate deve ser positivo: '{value}'"));
            }
            s.fx_rate = rate;
            Ok(())
        }
        ConfigField::Signal => {
            let trimmed = value.trim();
            if trimmed == "none" || trimmed.is_empty() {
                s.waybar.signal = None;
                return Ok(());
            }
            let n: i64 = trimmed
                .parse()
                .map_err(|_| format!("signal invalido: '{value}' (1-30 ou none)"))?;
            if !(1..=30).contains(&n) {
                return Err(format!("signal deve ser 1-30 ou none: '{value}'"));
            }
            s.waybar.signal = Some(n as u8);
            Ok(())
        }
        ConfigField::Interval => {
            let n: u32 = value
                .trim()
                .parse()
                .map_err(|_| format!("interval invalido: '{value}' (inteiro positivo)"))?;
            if n == 0 {
                return Err(format!("interval deve ser > 0: '{value}'"));
            }
            s.waybar.interval = n;
            Ok(())
        }
    }
}

/// Pure update function: mutates `state` based on `action`, returns follow-up actions.
/// No IO, no spawning, no clocks — fully testable.
pub fn update(state: &mut AppState, action: Action) -> Vec<Action> {
    match action {
        Action::Key(key) => {
            if let Some(semantic) = key_to_action_with_state(key, state) {
                return update(state, semantic);
            }
            vec![]
        }

        Action::Down => {
            let max = state.providers.len().saturating_sub(1);
            if state.selected < max {
                state.selected += 1;
            }
            vec![]
        }

        Action::Up => {
            if state.selected > 0 {
                state.selected -= 1;
            }
            vec![]
        }

        Action::OpenDetail => {
            state.mode = Mode::Detail;
            vec![]
        }

        Action::Back => {
            state.mode = Mode::List;
            vec![]
        }

        Action::SwitchTab(tab) => {
            let is_waybar = tab == Tab::Waybar;
            state.tab = tab;
            state.mode = Mode::List;
            // Inicializa config_state ao entrar na aba Waybar pela 1a vez.
            // Nao podemos acessar settings aqui (update e puro), entao sinalizamos.
            if is_waybar && state.config_state.is_none() {
                return vec![Action::InitConfig(
                    // Usa as edit_settings se ja existirem (re-entrada), senao placeholder.
                    // O event_loop vai enviar InitConfig com as settings reais.
                    crate::settings::Settings {
                        version: 0, // sentinela; event_loop sobrescreve com real
                        waybar: crate::settings::Waybar {
                            providers: vec![],
                            show_percentage: true,
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
                        glyph_mode: crate::settings::GlyphMode::Box,
                        fx_rate: 5.50,
                    },
                )];
            }
            vec![]
        }

        Action::Refresh => {
            state.status = FetchStatus::Loading;
            // The event loop observes Loading and fires the actual fetch.
            vec![]
        }

        Action::DataFetched(quotas) => {
            state.providers = quotas
                .providers
                .into_iter()
                .map(ProviderView::new)
                .collect();
            state.status = FetchStatus::Loaded;
            state.last_update = Some(OffsetDateTime::now_utc());
            // Clamp selection if providers list shrank.
            if !state.providers.is_empty() && state.selected >= state.providers.len() {
                state.selected = state.providers.len() - 1;
            }
            vec![]
        }

        Action::FetchFailed(msg) => {
            state.status = FetchStatus::Failed(msg);
            vec![]
        }

        Action::Quit => {
            state.should_quit = true;
            vec![]
        }

        Action::UsageComputed(summary) => {
            state.usage = Some(summary);
            vec![]
        }

        // --- Config tab actions ---
        Action::InitConfig(settings) => {
            // So inicializa se ainda nao existe (preserva edicoes em andamento).
            if state.config_state.is_none() {
                state.config_state = Some(ConfigState::new(&settings));
            } else if let Some(cs) = state.config_state.as_mut() {
                // Atualiza com as settings reais se ainda era o placeholder (version=0).
                if cs.edit_settings.version == 0 {
                    *cs = ConfigState::new(&settings);
                }
            }
            vec![]
        }

        Action::ConfigUp => {
            if let Some(cs) = state.config_state.as_mut() {
                if !cs.editing && cs.selected_field > 0 {
                    cs.selected_field -= 1;
                }
            }
            vec![]
        }

        Action::ConfigDown => {
            if let Some(cs) = state.config_state.as_mut() {
                let max = ConfigField::ALL.len().saturating_sub(1);
                if !cs.editing && cs.selected_field < max {
                    cs.selected_field += 1;
                }
            }
            vec![]
        }

        Action::ConfigEnterEdit => {
            if let Some(cs) = state.config_state.as_mut() {
                if !cs.editing {
                    let field = ConfigField::ALL[cs.selected_field];
                    let current = field_value_string(field, cs);
                    cs.input = tui_input::Input::new(current);
                    cs.editing = true;
                    cs.status_msg = None;
                }
            }
            vec![]
        }

        Action::ConfigCancelEdit => {
            if let Some(cs) = state.config_state.as_mut() {
                cs.editing = false;
                cs.input = tui_input::Input::default();
            }
            vec![]
        }

        Action::ConfigConfirmEdit => {
            if let Some(cs) = state.config_state.as_mut() {
                if cs.editing {
                    let field = ConfigField::ALL[cs.selected_field];
                    let value = cs.input.value().to_string();
                    match apply_field_edit(field, &value, cs) {
                        Ok(()) => {
                            cs.editing = false;
                            cs.input = tui_input::Input::default();
                            cs.status_msg =
                                Some("Campo atualizado. Pressione [s] para salvar.".to_string());
                        }
                        Err(e) => {
                            cs.status_msg = Some(format!("Erro: {e}"));
                            // Mantem edicao aberta para correcao.
                        }
                    }
                }
            }
            vec![]
        }

        Action::SaveConfig => {
            // Sinaliza ao event_loop que deve persistir as edit_settings.
            // O update e puro, nao faz IO.
            if state.config_state.is_some() {
                if let Some(cs) = state.config_state.as_mut() {
                    cs.status_msg = Some("Salvando...".to_string());
                }
                vec![Action::SaveConfig] // re-enfileira; event_loop intercepta
            } else {
                vec![]
            }
        }

        Action::ConfigSaveResult(result) => {
            if let Some(cs) = state.config_state.as_mut() {
                cs.status_msg = Some(match result {
                    Ok(()) => "Configuracao salva e Waybar recarregado.".to_string(),
                    Err(e) => format!("Erro ao salvar: {e}"),
                });
            }
            vec![]
        }

        // --- Aba Login ---
        Action::LoginUp => {
            if state.login_selected > 0 {
                state.login_selected -= 1;
            }
            state.login_status = None;
            vec![]
        }

        Action::LoginDown => {
            // 3 providers: indices 0, 1, 2.
            if state.login_selected < 2 {
                state.login_selected += 1;
            }
            state.login_status = None;
            vec![]
        }

        Action::LoginRequested(id) => {
            // Puro: sinaliza ao event_loop para executar o IO (RealLogin).
            // O event_loop intercepta e chama RealLogin::launch(id).
            state.login_status = Some(format!("Abrindo login para {}...", id));
            vec![Action::LoginRequested(id)]
        }

        Action::LoginResult(result) => {
            state.login_status = Some(match result {
                Ok(()) => "Login concluido. Pressione [r] para atualizar.".to_string(),
                Err(e) => format!("Erro no login: {e}"),
            });
            vec![]
        }

        Action::HistoryLoaded(records) => {
            state.history = Some(records);
            vec![]
        }

        Action::ToggleHelp => {
            state.show_help = !state.show_help;
            vec![]
        }

        Action::Tick => vec![],

        Action::AnimTick => {
            // Animação A (gauge lerp): cada provider avança display_ratio → target.
            for pv in &mut state.providers {
                let target = pv.target_ratio();
                pv.display_ratio += (target - pv.display_ratio) * 0.20;
            }
            // Animação C (throbber): avança o frame do spinner braille.
            state.throbber.advance();
            // Animação D (pulse): contador de frames para blink do ● crítico.
            state.anim_frame = state.anim_frame.wrapping_add(1);
            vec![]
        }
    }
}

#[cfg(test)]
mod tests {
    use ratatui::crossterm::event::KeyModifiers;

    use super::*;
    use crate::providers::types::{AllQuotas, ProviderQuota};

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
        }
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
    fn down_moves_selection_and_clamps() {
        let mut state = state_with_providers(3);
        assert_eq!(state.selected, 0);

        update(&mut state, Action::Down);
        assert_eq!(state.selected, 1);

        update(&mut state, Action::Down);
        assert_eq!(state.selected, 2);

        // Clamp: already at max
        update(&mut state, Action::Down);
        assert_eq!(state.selected, 2, "should clamp at providers.len()-1");
    }

    #[test]
    fn open_detail_then_back() {
        let mut state = AppState::new();
        assert_eq!(state.mode, Mode::List);

        update(&mut state, Action::OpenDetail);
        assert_eq!(state.mode, Mode::Detail);

        update(&mut state, Action::Back);
        assert_eq!(state.mode, Mode::List);
    }

    #[test]
    fn switch_tab_changes_tab_resets_mode() {
        let mut state = AppState::new();
        // Set detail mode to verify reset
        state.mode = Mode::Detail;
        state.tab = Tab::Dashboard;

        update(&mut state, Action::SwitchTab(Tab::Waybar));

        assert_eq!(state.tab, Tab::Waybar, "tab should switch to Waybar");
        assert_eq!(state.mode, Mode::List, "mode should reset to List");
    }

    #[test]
    fn data_fetched_populates_providers_and_status() {
        let mut state = AppState::new();
        assert_eq!(state.status, FetchStatus::Idle);
        assert!(state.providers.is_empty());
        assert!(state.last_update.is_none());

        let quotas = AllQuotas {
            providers: vec![fake_quota("claude"), fake_quota("codex")],
            fetched_at: "2026-06-19T12:00:00.000Z".to_string(),
        };

        update(&mut state, Action::DataFetched(quotas));

        assert_eq!(state.status, FetchStatus::Loaded);
        assert_eq!(state.providers.len(), 2);
        assert_eq!(state.providers[0].quota.provider, "claude");
        assert_eq!(state.providers[1].quota.provider, "codex");
        assert!(
            state.last_update.is_some(),
            "last_update should be Some after DataFetched"
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
    fn anim_tick_lerps_display_ratio_toward_target() {
        use crate::providers::types::QuotaWindow;

        // Cria provider com remaining=80% → target_ratio=0.80
        let mut q = fake_quota("claude");
        q.primary = Some(QuotaWindow {
            remaining: 80.0,
            resets_at: None,
            window_minutes: None,
            used: Some(20.0),
        });
        let mut state = AppState::new();
        // Inicializa com 0 (forçamos display_ratio inicial diferente do target)
        let mut pv = crate::tui::state::ProviderView::new(q);
        pv.display_ratio = 0.0; // ponto de partida artificial para testar a convergência
        state.providers = vec![pv];

        // Após 20 AnimTicks, display_ratio deve convergir próximo a 0.80
        for _ in 0..20 {
            update(&mut state, Action::AnimTick);
        }

        let display = state.providers[0].display_ratio;
        let target = 0.80_f64;
        let diff = (display - target).abs();
        assert!(
            diff < 0.01,
            "display_ratio {display:.4} deve estar próximo de {target:.2} após 20 ticks (diff={diff:.4})"
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

    #[test]
    fn display_ratio_initializes_to_target() {
        use crate::providers::types::QuotaWindow;
        let mut q = fake_quota("codex");
        q.primary = Some(QuotaWindow {
            remaining: 42.0,
            resets_at: None,
            window_minutes: None,
            used: Some(58.0),
        });
        let pv = crate::tui::state::ProviderView::new(q);
        // Na inicialização, display_ratio deve ser igual ao target (sem animação no 1º frame).
        let expected = 42.0 / 100.0;
        let diff = (pv.display_ratio - expected).abs();
        assert!(
            diff < 1e-10,
            "display_ratio={} mas esperado={expected}",
            pv.display_ratio
        );
    }

    // ---- Config tab tests ----

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
            glyph_mode: GlyphMode::Box,
            fx_rate: 5.50,
        }
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

        // Seleciona o campo FxRate (index 4)
        state.config_state.as_mut().unwrap().selected_field = 4;
        update(&mut state, Action::ConfigEnterEdit);

        let cs = state.config_state.as_ref().unwrap();
        assert!(cs.editing);
        assert_eq!(cs.input.value(), "5.50");
    }

    #[test]
    fn config_confirm_edit_updates_fx_rate() {
        let mut state = AppState::new();
        update(&mut state, Action::InitConfig(fake_settings()));
        state.config_state.as_mut().unwrap().selected_field = 4; // FxRate
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
        state.config_state.as_mut().unwrap().selected_field = 4; // FxRate
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
        state.config_state.as_mut().unwrap().selected_field = 4; // FxRate
        update(&mut state, Action::ConfigEnterEdit);
        assert!(state.config_state.as_ref().unwrap().editing);

        update(&mut state, Action::ConfigCancelEdit);
        assert!(!state.config_state.as_ref().unwrap().editing);
    }

    #[test]
    fn save_config_returns_save_config_action_and_sets_status() {
        let mut state = AppState::new();
        update(&mut state, Action::InitConfig(fake_settings()));

        let follow_ups = update(&mut state, Action::SaveConfig);
        // Deve re-enfileirar SaveConfig para o event_loop interceptar
        let has_save_config = follow_ups.iter().any(|a| matches!(a, Action::SaveConfig));
        assert!(has_save_config, "SaveConfig deve ser re-enfileirado");
        // Status msg deve ser "Salvando..."
        let msg = state.config_state.as_ref().unwrap().status_msg.as_deref();
        assert_eq!(msg, Some("Salvando..."));
    }
}
