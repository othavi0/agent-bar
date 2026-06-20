use ratatui::crossterm::event::KeyEvent;

use crate::providers::types::AllQuotas;

use super::state::Tab;

#[derive(Debug)]
pub enum Action {
    Key(KeyEvent),
    Tick,
    AnimTick,
    DataFetched(AllQuotas),
    FetchFailed(String),
    Up,
    Down,
    OpenDetail,
    Back,
    SwitchTab(Tab),
    Refresh,
    Quit,
}
