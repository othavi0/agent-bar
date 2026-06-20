use crate::providers::types::ProviderQuota;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Tab {
    Dashboard,
    Waybar,
    History,
    Login,
}

impl Tab {
    /// Tabs in display order.
    pub const ALL: [Tab; 4] = [Tab::Dashboard, Tab::Waybar, Tab::History, Tab::Login];

    pub fn index(&self) -> usize {
        match self {
            Tab::Dashboard => 0,
            Tab::Waybar => 1,
            Tab::History => 2,
            Tab::Login => 3,
        }
    }

    pub fn from_index(i: usize) -> Self {
        match i % 4 {
            0 => Tab::Dashboard,
            1 => Tab::Waybar,
            2 => Tab::History,
            _ => Tab::Login,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Panel {
    Sidebar,
    Content,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mode {
    List,
    Detail,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FetchStatus {
    Idle,
    Loading,
    Loaded,
    Failed(String),
}

#[derive(Debug, Clone)]
pub struct ProviderView {
    pub quota: ProviderQuota,
    // gauge animation state added in T6
}

impl ProviderView {
    pub fn new(quota: ProviderQuota) -> Self {
        Self { quota }
    }
}

#[derive(Debug)]
pub struct AppState {
    pub tab: Tab,
    pub providers: Vec<ProviderView>,
    pub selected: usize,
    pub mode: Mode,
    pub focus: Panel,
    pub status: FetchStatus,
    pub last_update: Option<time::OffsetDateTime>,
    pub should_quit: bool,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            tab: Tab::Dashboard,
            providers: Vec::new(),
            selected: 0,
            mode: Mode::List,
            focus: Panel::Sidebar,
            status: FetchStatus::Idle,
            last_update: None,
            should_quit: false,
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
