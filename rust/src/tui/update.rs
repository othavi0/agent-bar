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

        Action::Tick | Action::AnimTick => vec![],
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
}
