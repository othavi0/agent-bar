use ratatui::crossterm::event::{KeyCode, KeyEvent};
use time::OffsetDateTime;

use super::action::Action;
use super::state::{AppState, FetchStatus, Mode, ProviderView, Tab};

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
            state.tab = tab;
            state.mode = Mode::List;
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
}
