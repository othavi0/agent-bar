use ratatui::crossterm::event::KeyEvent;

use crate::providers::types::AllQuotas;
use crate::usage::UsageSummary;

use super::state::Tab;

#[derive(Debug)]
pub enum Action {
    Key(KeyEvent),
    Tick,
    AnimTick,
    DataFetched(AllQuotas),
    FetchFailed(String),
    /// Engine de custo calculou UsageSummary; armazenar em AppState.usage.
    UsageComputed(UsageSummary),
    Up,
    Down,
    OpenDetail,
    Back,
    SwitchTab(Tab),
    Refresh,
    Quit,
}
